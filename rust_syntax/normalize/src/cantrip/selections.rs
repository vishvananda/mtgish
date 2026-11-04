//! Resolution selections have lexical IDs, separate from instruction results.
use super::*;
#[derive(Clone)]
pub(super) struct PreparedSelection {
    pub(super) local: String,
    pub(super) pool: ObjectSet,
    pub(super) one: bool,
    pub(super) outside: bool,
    pub(super) outside_shown: bool,
}
impl Compiler {
    pub(super) fn move_with_terms(
        &mut self,
        id: usize,
        operation: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let Operation::Move {
            objects,
            destination,
            entry,
            face_down,
            terms,
        } = operation
        else {
            return Err(gap("typed movement terms"));
        };
        if !terms.qualifiers.is_empty() {
            return Err(gap("a movement qualification without a selection"));
        }
        if let MovementPlacement::By(chooser) = &terms.placement {
            let (
                PlayerSet::One(player),
                Destination::OwnersLibraryTopOrBottom,
                MovementOrder::DestinationDefault,
            ) = (chooser, destination, &terms.order)
            else {
                return Err(gap("a stated movement placement chooser"));
            };
            if !effects::plain_entry(entry) || *face_down {
                return Err(gap("arrival terms with a placement choice"));
            }
            return self.chooses_top_or_bottom(id, player, objects, result, out);
        }
        match (&terms.order, destination) {
            (MovementOrder::DestinationDefault, _) => {
                self.move_objects_p(id, objects, destination, entry, *face_down, result, out)
            }
            (MovementOrder::Random, Destination::OwnersLibraryTop)
                if effects::plain_entry(entry) && !*face_down =>
            {
                let cards = self.fetch(self.objects(objects)?);
                writeln!(out,"let mut order_{id}: Vec<ObjectId> = {cards}; for i in (1..order_{id}.len()).rev() {{ let j = g.random(i as u32 + 1) as usize; order_{id}.swap(i, j); }} let moved_{id} = g.put_into_library(&order_{id}, LibraryPlace::Top);").unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                Ok(())
            }
            (
                MovementOrder::Random,
                Destination::OwnersLibraryBottom | Destination::OwnersLibraryBottomRandom,
            ) => self.move_objects_p(
                id,
                objects,
                &Destination::OwnersLibraryBottomRandom,
                entry,
                *face_down,
                result,
                out,
            ),
            _ => Err(gap("a random movement order to this destination")),
        }
    }
    pub(super) fn prepared_selection(&self, binding: &SelectionId) -> Result<&PreparedSelection> {
        self.prepared_selections
            .iter()
            .rev()
            .find_map(|scope| scope.get(&binding.0))
            .ok_or_else(|| gap("selection outside its lexical preparation scope"))
    }
    pub(super) fn selection_metadata(&self, binding: &SelectionId) -> Result<&PreparedSelection> {
        self.prepared_selection(binding).or_else(|_| {
            self.carrying()
                .ok_or_else(|| gap("selection metadata outside its scope"))?
                .compiler
                .selection_metadata(binding)
        })
    }
    pub(super) fn prepare_selections(
        &mut self,
        steps: &[SelectionStep],
        purpose: &str,
        out: &mut String,
    ) -> Result<()> {
        for step in steps {
            let local = match &step.chooser {
                SelectionChooser::Chance => {
                    if !step.constraints.is_empty() {
                        return Err(gap("a constrained random selection"));
                    }
                    let count = match &step.count {
                        Count::Exactly(n) | Count::AsManyAsCan(n) => {
                            self.fetch(self.number(n)?.code)
                        }
                        _ => return Err(gap("a random selection count")),
                    };
                    let id = self.fresh();
                    let pool = self.fetch(self.objects(&step.objects)?);
                    writeln!(out,"let mut pool_{id}: Vec<ObjectId> = {pool}; let count_{id} = {count}; let mut random_{id}: Vec<ObjectId> = Vec::new(); for _ in 0..count_{id} {{ let Some(one) = cantrip_mechanics::library::random_of(g, &pool_{id}) else {{ break }}; pool_{id}.retain(|&o| o != one); random_{id}.push(one); }}").unwrap();
                    format!("random_{id}")
                }
                SelectionChooser::Players(players) => {
                    let chooser = match players {
                        PlayerSet::One(player) => self.fetch(self.certain_player(player)?),
                        PlayerSet::OpponentsOf(_) => {
                            let id = self.fresh();
                            let players = self.fetch(self.players(players)?);
                            writeln!(out,"let choosers_{id}: Vec<PlayerId> = {players}; let chooser_{id}: PlayerId = match choosers_{id}.len() {{ 0 => asker.controller, 1 => choosers_{id}[0], _ => cantrip_mechanics::choose::player(g, asker.controller, asker.source, choosers_{id}.clone(), \"Choose\", 0)?.unwrap_or(choosers_{id}[0]) }};").unwrap();
                            format!("chooser_{id}")
                        }
                        _ => return Err(gap("object collection Chosen")),
                    };
                    self.prepare_player_selection(&chooser, step, purpose, out)?
                }
            };
            self.bind_prepared_metadata(step, local);
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose_filter_parts_local(
        &mut self,
        id: usize,
        chooser: &str,
        pool: &str,
        count: &Count,
        filters: &[ObjectFilter],
        purpose: &str,
        out: &mut String,
    ) -> Result<String> {
        const M: &str = "cantrip_mechanics";
        let up_to = !matches!(count, Count::Exactly(_) | Count::AsManyAsCan(_));
        writeln!(out, "let player_{id} = {chooser}; let pool_{id} = {pool}; let mut picked_{id}: Vec<ObjectId> = Vec::new();").unwrap();
        // One object may be picked for each of its types: an artifact creature
        // is the artifact and the creature (the rulings of Cataclysm, Tragic
        // Arrogance and Ajani, Nacatl Avenger), a dual land two basic land types
        // (Global Ruin, Sundering Titan). A party's members are distinct (CR
        // 700.8), and so is a card picked for one keyword among several
        // (Selective Adaptation) or for one color.
        let distinct = ["party", "a card with ", "color"]
            .iter()
            .any(|words| lane_k::printed_otherwise(self.line_text(), words));
        for filter in filters {
            let test = self.filter(filter, &Candidate::of("id"))?;
            let again = match distinct {
                false => String::new(),
                true => format!(".filter(|id| !picked_{id}.contains(id))"),
            };
            let open = self.fetch(format!("pool_{id}.iter().copied(){again}.filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
            writeln!(out, "{{ let open = {open}; let one = {M}::choose::cards(g, player_{id}, asker.source, Purpose::{purpose}, open, 1, {up_to})?; for o in one {{ if !picked_{id}.contains(&o) {{ picked_{id}.push(o); }} }} }}").unwrap();
        }
        Ok(format!("picked_{id}"))
    }
    fn prepare_player_selection(
        &mut self,
        chooser: &str,
        step: &SelectionStep,
        purpose: &str,
        out: &mut String,
    ) -> Result<String> {
        let inferred = if step.constraints.is_empty() {
            and_or_parts(&step.objects, &step.count, self.line_text())
        } else { None };
        let filters = match step.constraints.as_slice() {
            [SelectionConstraint::OneForEach(filters)] => Some(filters.as_slice()),
            [] => inferred.as_deref(),
            _ => None,
        };
        if let Some(filters) = filters {
            let id = self.fresh();
            let pool = self.fetch(self.objects(&step.objects)?);
            return self.choose_filter_parts_local(id, chooser, &pool, &step.count, filters, purpose, out);
        }
        if matches!(step.count, Count::AnyNumber) {
            match step.constraints.as_slice() {
                [SelectionConstraint::OneForEachCardType(domain)] => {
                    let types = self.selection_type_domain(domain)?;
                    let id = self.fresh();
                    let pool = self.fetch(self.objects(&step.objects)?);
                    writeln!(out,"let chooser_{id} = {chooser}; let pool_{id}: Vec<ObjectId> = {pool}; let mut picked_{id}: Vec<ObjectId> = Vec::new(); let types_{id} = {types}; for t in types_{id} {{ let eligible: Vec<ObjectId> = pool_{id}.iter().copied().filter(|&c| !picked_{id}.contains(&c) && g.chars(c).is(t)).collect(); if eligible.is_empty() {{ continue; }} picked_{id}.extend(cantrip_mechanics::choose::cards_from(g, chooser_{id}, asker.source, Purpose::{purpose}, pool_{id}.clone(), eligible, 1, true)?); }}").unwrap();
                    return Ok(format!("picked_{id}"));
                }
                [SelectionConstraint::AllWithChosenName] => {
                    let id = self.fresh();
                    let pool = self.fetch(self.objects(&step.objects)?);
                    writeln!(out,"let chooser_{id} = {chooser}; let pool_{id}: Vec<ObjectId> = {pool}; let mut picked_{id}: Vec<ObjectId> = Vec::new(); let one_{id} = cantrip_mechanics::choose::cards(g, chooser_{id}, asker.source, Purpose::{purpose}, pool_{id}.clone(), 1, false)?; if let Some(&one) = one_{id}.first() {{ let q: &dyn Query = g; picked_{id} = pool_{id}.iter().copied().filter(|&c| cantrip_mechanics::same_name(q, c, one)).collect(); }}").unwrap();
                    return Ok(format!("picked_{id}"));
                }
                _ => {}
            }
        }
        let mut rules = String::new();
        for constraint in &step.constraints {
            let part = match constraint {
                SelectionConstraint::TotalProperty {
                    property,
                    comparison: Comparison::LessOrEqual,
                    amount,
                } if !matches!(amount, Expr::Constant(_)) => {
                    let form = match property {
                        Property::Power => "TotalPowerIs",
                        Property::ManaValue => "TotalManaValueIs",
                        _ => return Err(gap("a local selection total property")),
                    };
                    let id = self.fresh();
                    let amount = self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?);
                    writeln!(out, "let most_{id} = {amount};").unwrap();
                    lane_l::total_at_most_part(&mut self.functions, form, &format!("most_{id}"))
                }
                constraint => {
                    // Preserve the existing reference guard: a named earlier
                    // subset (notably only the cards just milled) is not the
                    // whole graveyard printed by the card. Inspect typed pools,
                    // never the spelling of generated locals or quoted text.
                    if self.line_text().is_some_and(|line|line.contains("from your graveyard"))
                        && !queries_graveyard(&step.objects) {
                        return Err(gap("a narrowed choice pool where the card says from your graveyard"));
                    }
                    self.set_rule(constraint).ok_or_else(|| {
                        gap(format!("local selection constraint {}", form(constraint)))
                    })?
                },
            };
            write!(rules, ".with({part})").unwrap();
        }
        self.choice_rule = rules;
        self.choose_local(chooser, &step.objects, &step.count, purpose, out)
    }
    fn selection_type_domain(&self, domain: &CardTypeDomain) -> Result<String> {
        Ok(match domain {
            CardTypeDomain::All => "CardType::ALL.to_vec()".into(),
            CardTypeDomain::Permanent => {
                "cantrip_mechanics::targets::PERMANENT_CARD_TYPES.to_vec()".into()
            }
            CardTypeDomain::AmongCasts {
                by: PlayerSet::One(by),
                filter: ObjectFilter::All(filters),
                window: HistoryWindow::ThisTurn,
            } => {
                let [ObjectFilter::Not(not)] = filters.as_slice() else {
                    return Err(gap("a local selection cast type domain"));
                };
                let ObjectFilter::CardType(not) = not.as_ref() else {
                    return Err(gap("a local selection cast type domain"));
                };
                let by = self.fetch(self.certain_player(by)?);
                let not = card_type(not)?;
                format!(
                    "{{ let by = {by}; let q: &dyn Query = g; CardType::ALL.into_iter().filter(|&t| q.history().casts_by(by).any(|c| !c.is(CardType::{not}) && c.is(t))).collect::<Vec<CardType>>() }}"
                )
            }
            CardTypeDomain::AmongObjects(objects) => {
                let objects = self.fetch(self.objects(objects)?);
                format!(
                    "{{ let objects: Vec<ObjectId> = {objects}; let q: &dyn Query = g; CardType::ALL.into_iter().filter(|&t| objects.iter().any(|&o| q.chars(o).is(t))).collect::<Vec<CardType>>() }}"
                )
            }
            _ => return Err(gap("a local selection cast type domain")),
        })
    }
    pub(super) fn bind_prepared_metadata(&mut self, step: &SelectionStep, local: String) {
        use crate::visit::{VisitorMut, WalkMut};
        struct Expand<'a>(&'a Compiler);
        impl VisitorMut for Expand<'_> {
            fn ability(&mut self, _: &mut Ability) -> bool {
                false
            }
            fn cost(&mut self, _: &mut Cost) -> bool {
                false
            }
            fn instruction(&mut self, _: &mut Instruction) -> bool {
                false
            }
            fn announcement(&mut self, _: &mut Announcement) -> bool {
                false
            }
            fn target_spec(&mut self, _: &mut TargetSpec) -> bool {
                false
            }
            fn event_spec(&mut self, _: &mut EventSpec) -> bool {
                false
            }
            fn object_set_end(&mut self, objects: &mut ObjectSet) {
                if let ObjectSet::Prepared(binding) = objects {
                    if let Ok(prepared) = self.0.selection_metadata(binding) {
                        *objects = prepared.pool.clone();
                    }
                }
            }
        }
        let outside = self.from_outside(&step.objects);
        let outside_shown = outside && matches!(step.objects, ObjectSet::Union(_))
            || self.outside_shown(&step.objects);
        let mut pool = step.objects.clone();
        pool.walk_mut(&mut Expand(self));
        self.prepared_selections
            .last_mut()
            .expect("preparation scope")
            .insert(
                step.binding.0,
                PreparedSelection {
                    local,
                    pool,
                    one: effects::one_at_most(&step.count),
                    outside,
                    outside_shown,
                },
            );
    }
    pub(super) fn with_selections(
        &mut self,
        steps: &[SelectionStep],
        action: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        self.prepared_selections.push(BTreeMap::new());
        let done = self
            .prepare_selections(steps, selection_purpose(action), out)
            .and_then(|()| {
                if let Some(last) = steps
                    .iter()
                    .rev()
                    .find(|s| matches!(s.chooser, SelectionChooser::Players(_)))
                {
                    let local = self.prepared_selection(&last.binding)?.local.clone();
                    self.implicit_selections.insert(result.0, local);
                    self.notes.implicit_each.remove(&result.0);
                    if effects::one_at_most(&last.count) {
                        self.notes.implicit_single.insert(result.0);
                    } else {
                        self.notes.implicit_single.remove(&result.0);
                    }
                    self.notes.earlier.push(("Selected", result.clone()));
                }
                let gaps = self.gaps.len();
                self.operation(action, result, out)?;
                self.report(action, result, self.gaps.len() > gaps, out);
                Ok(())
            });
        self.prepared_selections.pop();
        done
    }
}

fn queries_graveyard(objects: &ObjectSet) -> bool {
    use crate::visit::{Visitor,Walk};
    struct Query(bool);
    impl Visitor for Query {
        fn ability(&mut self,_:&Ability)->bool {false}
        fn object_set(&mut self,objects:&ObjectSet)->bool {
            self.0 |= matches!(objects,ObjectSet::Zone {scope:ZoneScope::Players {zone:EventZone::Graveyard,..},..});
            !self.0
        }
        fn object_filter(&mut self,filter:&ObjectFilter)->bool {
            self.0 |= matches!(filter,ObjectFilter::InZone(ZoneScope::Players {zone:EventZone::Graveyard,..}));
            !self.0
        }
    }
    let mut query=Query(false);objects.walk(&mut query);query.0
}
#[cfg(test)]
mod transfer_reference_tests {
    use super::*;
    #[test]
    fn an_earlier_milled_subset_and_literal_text_do_not_become_the_whole_graveyard() {
        assert!(!queries_graveyard(&ObjectSet::Named(ResultId(0))));
        assert!(!queries_graveyard(&ObjectSet::Primitive {name:"contains Zone::graveyard".into(),operands:vec![Operand::Text("Zone::graveyard".into())]}));
        assert!(queries_graveyard(&ObjectSet::Zone {scope:ZoneScope::Players {players:PlayerSet::One(PlayerRef::AbilityController),zone:EventZone::Graveyard},kind:ObjectKind::Card,filter:ObjectFilter::Any}));
    }
}

pub(super) fn selection_purpose(action: &Operation) -> &'static str {
    let objects = match action.action() {
        Operation::Move { objects, .. } | Operation::Tap { objects, .. } => Some(objects),
        _ => None,
    };
    match objects {
        Some(ObjectSet::Difference { excluding, .. })
            if matches!(excluding.as_ref(), ObjectSet::Prepared(_)) =>
        {
            "Keep"
        }
        _ => match action.action() {
            Operation::Move { destination, .. } => match destination {
                Destination::OwnersHand | Destination::Hand(_) => "PutIntoHand",
                Destination::OwnersGraveyard => "PutIntoGraveyard",
                Destination::Exile => "Exile",
                Destination::OwnersLibraryTop => "PutOnTop",
                Destination::OwnersLibraryBottom | Destination::OwnersLibraryBottomRandom => {
                    "PutOnBottom"
                }
                Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                    "PutOntoBattlefield"
                }
                _ => "PutOnLibrary",
            },
            _ => "Choose",
        },
    }
}

/// An input to the current operation, rather than a later declared program.
/// Only bindings introduced here prevent one common entry/destination batch;
/// a fixed selection captured from an enclosing scope remains usable.
pub(super) fn reads_local<T: crate::visit::Walk>(value: &T, locals: &BTreeSet<u32>) -> bool {
    use crate::visit::Visitor;
    struct Read<'a> {
        locals: &'a BTreeSet<u32>,
        found: bool,
    }
    impl Visitor for Read<'_> {
        fn ability(&mut self, _: &Ability) -> bool {
            false
        }
        fn instruction(&mut self, _: &Instruction) -> bool {
            false
        }
        fn cost(&mut self, _: &Cost) -> bool {
            false
        }
        fn announcement(&mut self, _: &Announcement) -> bool {
            false
        }
        fn target_spec(&mut self, _: &TargetSpec) -> bool {
            false
        }
        fn event_spec(&mut self, _: &EventSpec) -> bool {
            false
        }
        fn operation(&mut self, _: &Operation) -> bool {
            false
        }
        fn object_set(&mut self, objects: &ObjectSet) -> bool {
            if let ObjectSet::Prepared(binding) = objects {
                self.found |= self.locals.contains(&binding.0);
            }
            true
        }
    }
    let mut read = Read {
        locals,
        found: false,
    };
    value.walk(&mut read);
    read.found
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_entry_dependency_reads_only_this_scopes_local_bindings() {
        let locals = BTreeSet::from([1]);
        let current = Expr::CountObjects(Box::new(ObjectSet::Prepared(SelectionId(1))));
        let outer = Expr::CountObjects(Box::new(ObjectSet::Prepared(SelectionId(0))));
        assert!(reads_local(&current, &locals));
        assert!(!reads_local(&outer, &locals));
        let ability = Ability::Spell {
            announcement: Announcement::empty(),
            body: Program {
                instructions: vec![Instruction::Effect {
                    result: ResultId(0),
                    operation: Operation::Draw {
                        players: PlayerSet::One(PlayerRef::AbilityController),
                        amount: current.clone(),
                    },
                }],
            },
        };
        assert!(!reads_local(&ability, &locals));
        assert!(!reads_local(
            &Cost {
                parts: vec![CostPart::Life(current)]
            },
            &locals
        ));
    }
}
impl Compiler {
    pub(super) fn single_carried_selection(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::Created(result) | ObjectSet::Selected(result) => {
                self.single.contains(&result.0)
            }
            ObjectSet::Prepared(binding) => self.selection_metadata(binding).is_ok_and(|p| p.one),
            ObjectSet::Earlier {
                what: EarlierObjects::Selected,
                by: None,
            } => self
                .notes
                .earlier
                .iter()
                .rev()
                .find(|(word, _)| *word == "Selected")
                .is_some_and(|(_, result)| self.notes.implicit_single.contains(&result.0)),
            _ => false,
        }
    }
}

/// "A creature card and/or a land card": up to one of each, not two of either.
fn and_or_parts(cards: &ObjectSet, count: &Count, line: Option<&str>) -> Option<Vec<ObjectFilter>> {
    let (ObjectSet::Zone { filter, .. } | ObjectSet::Matching { filter, .. }) = cards else {
        return None;
    };
    let (ObjectFilter::Either(parts), Count::UpTo(Expr::Constant(most))) = (filter, count) else {
        return None;
    };
    (parts.len() as i64 == *most && line?.contains("and/or")).then(|| parts.clone())
}
