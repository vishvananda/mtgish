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
            if !step.constraints.is_empty() {
                return Err(gap("constrained local selection"));
            }
            let local = match &step.chooser {
                SelectionChooser::Chance => {
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
                    self.choose_local(&chooser, &step.objects, &step.count, purpose, out)?
                }
            };
            self.bind_prepared_metadata(step, local);
        }
        Ok(())
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

pub(super) fn selection_purpose(action: &Operation) -> &'static str {
    let objects = match action.action() {
        Operation::Move { objects, .. } | Operation::Tap { objects, .. } => Some(objects),
        Operation::ChangeZone { cards, .. } => Some(cards),
        _ => None,
    };
    match objects {
        Some(ObjectSet::Difference { excluding, .. })
            if matches!(excluding.as_ref(), ObjectSet::Prepared(_)) =>
        {
            "Keep"
        }
        _ => "Choose",
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
