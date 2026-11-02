//! Resolution: instructions and operations, as the body of an `Effect`.
//! Reads are `fetch`ed into locals before anything changes, so one instruction
//! sees one game state, and each batch goes to the API that keeps it one batch.
use super::*;

/// The prompt a choice asks with, from what its result is for. It is wording: a
/// choice nothing consumes in sight stays a gap rather than asking something odd.
pub(super) fn purpose(result: &ResultId, rest: &[Instruction]) -> Option<&'static str> {
    let picked = |object: &ObjectRef| matches!(object, ObjectRef::Result { result: r, .. } if r.0 == result.0);
    let chosen = |objects: &ObjectSet| match objects {
        ObjectSet::Selected(r) => r.0 == result.0,
        ObjectSet::One(object) => picked(object),
        _ => false,
    };
    rest.iter().find_map(|instruction| {
        let operation = match instruction {
            Instruction::Effect { operation, .. } => operation,
            Instruction::Group(body)
            | Instruction::Optional { body, .. }
            | Instruction::Repeatable(body)
            | Instruction::Times { body, .. }
            | Instruction::ForEachPlayer { body, .. }
            | Instruction::ForEachObject { body, .. } => {
                return purpose(result, &body.instructions)
            }
            Instruction::If {
                then, otherwise, ..
            } => {
                return purpose(result, &then.instructions)
                    .or_else(|| purpose(result, &otherwise.instructions))
            }
            Instruction::ChooseOne { options, .. } => {
                return options
                    .iter()
                    .find_map(|option| purpose(result, &option.instructions))
            }
        };
        Some(match operation {
            Operation::Tap { objects, tapped } if chosen(objects) => {
                if *tapped {
                    "Tap"
                } else {
                    "Untap"
                }
            }
            Operation::Destroy { objects, .. } if chosen(objects) => "Destroy",
            Operation::DiscardObjects { objects } if chosen(objects) => "Discard",
            Operation::Sacrifice { objects, .. } if chosen(objects) => "Sacrifice",
            Operation::Reveal { objects } if chosen(objects) => "Reveal",
            Operation::Attach { attachments, host } if chosen(attachments) || picked(host) => {
                "Attach"
            }
            Operation::ReorderLibrary { objects, end, .. } if chosen(objects) => match end {
                SequenceEnd::Top => "PutOnTop",
                SequenceEnd::Bottom => "PutOnBottom",
            },
            Operation::Move {
                objects,
                destination,
                ..
            } if chosen(objects) => match destination {
                Destination::OwnersHand => "PutIntoHand",
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
            _ => return None,
        })
    })
}

/// Whether a later instruction reads the objects `result` chose: chosen, then
/// referred to, with nothing happening to them by being chosen.
fn referred_to(result: &ResultId, rest: &[Instruction]) -> bool {
    let (selected, picked) = (
        format!("Selected(ResultId({}))", result.0),
        format!("Result {{ result: ResultId({}),", result.0),
    );
    rest.iter().any(|instruction| {
        let text = format!("{instruction:?}");
        text.contains(&selected) || text.contains(&picked)
    })
}

fn reveals_one(count: &Count) -> bool {
    matches!(count, Count::Exactly(Expr::Constant(1)))
}

/// Whether a cost is one instruction a player either does or doesn't: drawing,
/// revealing a card, or moving objects no one chooses among.
fn performed_by_itself(cost: &Cost) -> bool {
    match cost.parts.as_slice() {
        [CostPart::Draw { .. } | CostPart::Mill { .. }] => true,
        [CostPart::Sacrifice {
            choose: Some(Count::Exactly(Expr::Constant(1))),
            ..
        }] => true,
        [CostPart::Discard {
            count: Expr::Constant(1),
            random: false,
            ..
        }] => true,
        [CostPart::Discard {
            filter: ObjectFilter::Any,
            random: true,
            ..
        }] => true,
        [CostPart::Reveal {
            count,
            random: false,
            ..
        }] => reveals_one(count),
        [CostPart::Move {
            selections,
            face_down: false,
            ..
        }] => matches!(
            selections.as_slice(),
            [CostObjectSelection {
                choose: None | Some(Count::Exactly(_)),
                ..
            }]
        ),
        _ => false,
    }
}

/// Whether `decider`'s "you may" is that they pay something the engine can ask
/// about as one payment.
pub(super) fn asks_by_paying(decider: &PlayerRef, body: &Program) -> bool {
    match body.instructions.first() {
        Some(Instruction::Group(group)) => asks_by_paying(decider, group),
        Some(Instruction::Effect {
            operation:
                Operation::PayCost {
                    player,
                    cost,
                    optional: false,
                },
            ..
        }) => {
            format!("{player:?}") == format!("{decider:?}")
                && cost.parts.iter().all(|part| {
                    matches!(part, CostPart::Mana(_) | CostPart::Life(_))
                        || lane_g::payer_sets_the_size(part)
                })
        }
        _ => false,
    }
}

/// Whether the next thing after a move is to send it back when the source goes.
fn returns_with_source(result: &ResultId, rest: &[Instruction]) -> bool {
    matches!(
        rest.first(),
        Some(Instruction::Effect {
            operation: Operation::ReturnAfter {
                objects: ObjectSet::Transition { result: moved, phase: ObjectPhase::After },
                duration: crate::ir::Duration::WhileOnBattlefield(ObjectRef::Source),
            },
            ..
        }) if moved.0 == result.0
    )
}

/// Whether a token copy's adjustment changes anything about the copy.
fn adjusts(adjustment: &Operand) -> bool {
    !matches!(adjustment, Operand::Term { form, .. } if form == "NoTokenCopyEffects")
}

/// `list` less what stayed: a transition is of what moved, not what was asked to.
pub(super) fn gone(list: &str) -> String {
    format!("{list}.iter().copied().filter(|&o| q.state().obj(o).became.is_some()).collect::<Vec<ObjectId>>()")
}

/// What a result of the search that found it is about to become, for the search
/// prompt. The move itself stays an ordinary later instruction.
pub(super) fn search_destination(result: &ResultId, rest: &[Instruction]) -> &'static str {
    let chosen = |objects: &ObjectSet| matches!(objects, ObjectSet::Selected(r) if r.0 == result.0);
    for instruction in rest {
        if let Instruction::Effect {
            operation:
                Operation::Move {
                    objects,
                    destination,
                    entry,
                    ..
                },
            ..
        } = instruction
        {
            if chosen(objects) {
                return match destination {
                    Destination::OwnersHand => "Hand",
                    Destination::OwnersGraveyard => "Graveyard",
                    Destination::Exile => "Exile",
                    Destination::OwnersLibraryTop => "TopOfLibrary",
                    Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                        if entry.tapped {
                            "BattlefieldTapped"
                        } else {
                            "Battlefield"
                        }
                    }
                    _ => "Hand",
                };
            }
        }
    }
    "Hand"
}

pub(super) fn plain_entry(entry: &EntrySpec) -> bool {
    !entry.tapped && entry.counters.is_empty() && entry.states.is_empty()
}

/// The locals earlier instructions bound, keyed by the result that names them.
pub(super) struct Results {
    pub(super) selections: BTreeMap<u32, String>,
    pub(super) unchosen: BTreeMap<u32, String>,
    pub(super) reveal_matches: BTreeMap<u32, String>,
    pub(super) creations: BTreeMap<u32, String>,
    pub(super) moved: BTreeMap<u32, String>,
    pub(super) before: BTreeMap<u32, String>,
    pub(super) performed: BTreeMap<u32, String>,
    pub(super) values: BTreeMap<u32, String>,
    pub(super) flips: BTreeMap<u32, String>,
    pub(super) chosen: Vec<(ValueKind, String)>,
    pub(super) searched: BTreeMap<u32, String>,
    failed: BTreeSet<u32>,
    pub(super) notes: lane_m::Notes,
}

/// Which of a block's results `Compiler::hoist` keeps.
#[derive(Clone, Copy)]
enum Hoist {
    Moved,
    Selected,
    Created,
    Before,
}

/// A count that picks one object, or none.
pub(super) fn one_at_most(count: &Count) -> bool {
    matches!(
        count,
        Count::Exactly(Expr::Constant(1))
            | Count::UpTo(Expr::Constant(1))
            | Count::AsManyAsCan(Expr::Constant(1))
    )
}

impl Compiler {
    /// `Vec<Target>`.
    pub(super) fn recipients(&self, recipients: &Recipients) -> Result<String> {
        if let Some(carried) = self.outside_recipients(recipients)? {
            return Ok(carried);
        }
        Ok(match recipients {
            Recipients::Objects(objects) => format!(
                "({}).into_iter().map(Target::Object).collect::<Vec<Target>>()",
                self.objects(objects)?
            ),
            Recipients::Players(players) => format!(
                "({}).into_iter().map(Target::Player).collect::<Vec<Target>>()",
                self.players(players)?
            ),
            Recipients::Targets(slot) => format!("t_{}.clone()", self.target_slot(slot)?),
            Recipients::Target { slot, index } => format!(
                "t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t)).into_iter().collect::<Vec<Target>>()",
                i = self.target_slot(slot)?
            ),
            Recipients::Union(parts) => {
                let mut code = String::from("{ let mut all: Vec<Target> = Vec::new(); ");
                for part in parts {
                    write!(
                        code,
                        "for t in {} {{ if !all.contains(&t) {{ all.push(t); }} }} ",
                        self.recipients(part)?
                    )
                    .unwrap();
                }
                code.push_str("all }");
                code
            }
            Recipients::Chosen => self
                .chosen_local(ValueKind::Recipient)
                .map(|chosen| format!("vec![{chosen}]"))
                .map_err(|_| gap("chosen damage recipient"))?,
            Recipients::DamagedByEvent => match self.frame {
                Frame {
                    recipient: Some(Carried::Data(i)),
                    ..
                } => {
                    let (_, data) = self.carried(Carried::Data(i))?;
                    format!("vec![{{ let r = {data}[{i}]; if r >= u32::MAX - 1 - u32::from(u8::MAX) {{ Target::Player(PlayerId((u32::MAX - 1 - r) as u8)) }} else {{ Target::Object(ObjectId(r)) }} }}]")
                }
                Frame {
                    damage_recipient: Some(carried),
                    ..
                } => format!(
                    "{}.map(Target::Object).into_iter().collect::<Vec<Target>>()",
                    self.carried_object(Some(carried))?
                ),
                Frame {
                    player: Some(carried),
                    ..
                } => format!(
                    "{}.map(Target::Player).into_iter().collect::<Vec<Target>>()",
                    self.carried_player(Some(carried))?
                ),
                _ => return Err(gap("recipient of event damage")),
            },
            Recipients::CountersTheEventPut => return Err(gap("counters as a recipient")),
            Recipients::AttackedBy(object) => {
                let carried = match self.frame.player {
                    Some(_) => format!(
                        ".or({}.map(Target::Player))",
                        self.carried_player(self.frame.player)?
                    ),
                    None => String::new(),
                };
                let battle = match lane_k::printed_otherwise(self.line_text(), "player or planeswalker it's attacking") {
                    true => ".filter(|t| !matches!(t, Target::Object(o) if q.state().obj(*o).became.is_some() || !q.chars(*o).is(CardType::Planeswalker)))",
                    false => "",
                };
                format!(
                    "{{ let attacker = {}; q.state().attackers.iter().find(|a| Some(a.id) == attacker).map(|a| a.target).filter(|t| *t != Target::None).or_else(|| q.history().this_turn().iter().rev().find_map(|r| match r.event {{ Event::Attacks {{ attacker: a, defender, .. }} if Some(a) == attacker => Some(defender), _ => None }})){carried}{battle}.into_iter().collect::<Vec<Target>>() }}",
                    self.object(object)?
                )
            }
        })
    }
    fn target_slot(&self, slot: &Slot) -> Result<usize> {
        if self.reader != Reader::Resolution || !self.stack_item {
            return Err(gap("target reference outside a resolving ability"));
        }
        self.slots
            .get(&slot.0)
            .map(|(i, _)| *i)
            .ok_or_else(|| gap("target reference"))
    }
    pub(super) fn effect(&mut self, body: &Program, label: &str) -> Result<String> {
        self.reset_results();
        let out = self.effect_body(body);
        self.chosen.clear();
        let out = out?;
        writeln!(self.functions,"fn {label}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ {out} Ok(()) }}").unwrap();
        Ok(label.into())
    }
    /// `effect` for an intervening "if" the engine cannot ask (CR 603.4): the
    /// ability does nothing if the condition no longer holds as it resolves.
    pub(super) fn guarded_effect(
        &mut self,
        condition: &Condition,
        body: &Program,
        label: &str,
    ) -> Result<String> {
        self.reset_results();
        let reader = std::mem::replace(&mut self.reader, Reader::Resolution);
        let at = std::mem::replace(&mut self.event_at, EventAt::Item);
        let test = self.condition(condition).map(|c| self.fetch(c));
        self.reader = reader;
        self.event_at = at;
        let test = test?;
        let out = self.effect_body(body);
        self.chosen.clear();
        let out = out?;
        writeln!(self.functions,"fn {label}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let asker = &Asker::new(item.object, item.controller); if !{test} {{ return Ok(()); }} {out} Ok(()) }}").unwrap();
        Ok(label.into())
    }
    fn effect_body(&mut self, body: &Program) -> Result<String> {
        let mut out = String::new();
        writeln!(
            out,
            "let asker = &Asker::new(item.object, item.controller);"
        )
        .unwrap();
        if !self.slots.is_empty() {
            self.helpers.insert("announced");
        }
        for (i, _) in self.slots.values() {
            match self.second_of_two == Some(*i) {
                true => writeln!(
                    out,
                    "let t_{i}_raw: Vec<Target> = announced(g, item, 0, false).into_iter().skip(1).collect(); let t_{i} = announced(g, item, 0, true);"
                ),
                false => writeln!(
                    out,
                    "let t_{i}_raw = announced(g, item, {i}, false); let t_{i} = announced(g, item, {i}, true);"
                ),
            }
            .unwrap();
        }
        let reader = std::mem::replace(&mut self.reader, Reader::Resolution);
        let at = std::mem::replace(&mut self.event_at, EventAt::Item);
        let hoisted = self.hoisted.replace(Vec::new());
        let around = std::mem::take(&mut self.after);
        let compiled = self.program(body, &mut out, true);
        self.after = around;
        let declared = std::mem::replace(&mut self.hoisted, hoisted);
        self.reader = reader;
        self.event_at = at;
        compiled?;
        for local in declared.unwrap_or_default() {
            out.insert_str(
                0,
                &format!("let mut {local}: Vec<ObjectId> = Vec::new();\n"),
            );
        }
        Ok(out)
    }
    pub(super) fn program(
        &mut self,
        body: &Program,
        out: &mut String,
        boundaries: bool,
    ) -> Result<()> {
        // Each instruction here answers for its own players.
        let present = self.present.borrow_mut().take();
        let picked = self.picked.borrow().named.len();
        self.note_unless(body);
        let mut i = 0;
        while i < body.instructions.len() {
            match self.one_question(&body.instructions[i..], out) {
                true => i += 2,
                false => {
                    self.step(body, i, out);
                    i += 1;
                }
            }
            if boundaries && self.pending == Pending::Nothing {
                writeln!(out, "g.end_sentence();").unwrap();
            }
        }
        *self.present.borrow_mut() = present;
        self.picked.borrow_mut().named.truncate(picked);
        Ok(())
    }
    /// One instruction of `body`, or its hole. What it half bound goes, and a
    /// later read of its result stands in for it: see `planned::unreported`.
    pub(super) fn step(&mut self, body: &Program, i: usize, out: &mut String) {
        let instruction = &body.instructions[i];
        let mark = out.len();
        let scope = self.results();
        let mut after = body.instructions[i + 1..].to_vec();
        after.extend_from_slice(&self.after);
        let around = std::mem::replace(&mut self.after, after);
        let planned = self.gaps.len();
        let failure = self.attempt(format!("instructions[{i}]"), |c| {
            let gaps = c.gaps.len();
            let may_choose = c.may_choose;
            match c.instruction(body, i, instruction, out) {
                Err(failed) if lane_j::wants_a_player(&failed.feature) => {
                    out.truncate(mark);
                    c.gaps.truncate(gaps);
                    c.may_choose = may_choose;
                    c.if_present(body, i, instruction, out)
                }
                compiled => compiled,
            }
        });
        if let (None, Instruction::Effect { result, operation }) = (&failure, instruction) {
            let waits = self.gaps.len() > planned;
            self.report(operation, result, waits, out);
        }
        self.after = around;
        if let Some(feature) = failure {
            out.truncate(mark);
            self.restore_results(scope);
            self.asking = None;
            if let Instruction::Effect { result, .. } = instruction {
                self.failed.insert(result.0);
            }
            if !self.stated_instruction(&feature, instruction, out) {
                self.plan_instruction(&feature, instruction, out);
            }
        }
        self.note_step(instruction);
    }
    pub(super) fn results(&self) -> Results {
        Results {
            selections: self.selections.clone(),
            unchosen: self.unchosen.clone(),
            reveal_matches: self.reveal_matches.clone(),
            creations: self.creations.clone(),
            moved: self.moved.clone(),
            before: self.before.clone(),
            performed: self.performed.clone(),
            values: self.values.clone(),
            flips: self.flips.clone(),
            chosen: self.chosen.clone(),
            searched: self.searched.clone(),
            failed: self.failed.clone(),
            notes: self.notes.clone(),
        }
    }
    pub(super) fn restore_results(&mut self, results: Results) {
        let Results {
            selections,
            unchosen,
            reveal_matches,
            creations,
            moved,
            before,
            performed,
            values,
            flips,
            chosen,
            searched,
            failed,
            notes,
        } = results;
        self.selections = selections;
        self.unchosen = unchosen;
        self.reveal_matches = reveal_matches;
        self.creations = creations;
        self.moved = moved;
        self.before = before;
        self.performed = performed;
        self.values = values;
        self.flips = flips;
        self.chosen = chosen;
        self.searched = searched;
        self.failed = failed;
        self.notes = notes;
    }
    /// A nested block: what it binds is out of Rust's scope once it closes. The
    /// objects it reports stay readable through a local of the whole effect;
    /// anything else reports a gap instead of generating a compile error.
    pub(super) fn block(
        &mut self,
        body: &Program,
        out: &mut String,
        boundaries: bool,
    ) -> Result<()> {
        let scope = self.results();
        self.program(body, out, boundaries)?;
        let inner = self.results();
        self.restore_results(scope);
        self.failed.extend(inner.failed.iter());
        self.notes.earlier = inner.notes.earlier;
        self.hoist(&inner.moved, Hoist::Moved, out);
        self.hoist(&inner.selections, Hoist::Selected, out);
        self.hoist(&inner.creations, Hoist::Created, out);
        self.hoist(&inner.before, Hoist::Before, out);
        Ok(())
    }
    /// Keep the objects a block just reported. `out` is at the block's end, so
    /// what didn't run leaves the list empty, as "those" then is.
    fn hoist(&mut self, inner: &BTreeMap<u32, String>, kind: Hoist, out: &mut String) {
        for (result, code) in inner {
            let known = match kind {
                Hoist::Moved => &self.moved,
                Hoist::Selected => &self.selections,
                Hoist::Created => &self.creations,
                Hoist::Before => &self.before,
            };
            if known.contains_key(result) || self.hoisted.is_none() {
                continue;
            }
            let local = format!("hoisted_{}", self.fresh());
            let read = match kind {
                Hoist::Before => self.fetch(code.clone()),
                _ => format!("{code}.clone()"),
            };
            writeln!(out, "{local}.extend({read});").unwrap();
            self.hoisted.as_mut().expect("checked").push(local.clone());
            // `before` holds an expression, the others a local to clone.
            let (known, local) = match kind {
                Hoist::Moved => (&mut self.moved, local),
                Hoist::Selected => (&mut self.selections, local),
                Hoist::Created => (&mut self.creations, local),
                Hoist::Before => (&mut self.before, format!("{local}.clone()")),
            };
            known.insert(*result, local);
        }
    }
    pub(super) fn instruction(
        &mut self,
        body: &Program,
        i: usize,
        instruction: &Instruction,
        out: &mut String,
    ) -> Result<()> {
        if let Some(each) = self.one_at_a_time(instruction) {
            return self.instruction(body, i, &each, out);
        }
        if let Some(picked) = self.picked_first(instruction, body, i, out)? {
            return self.instruction(body, i, &picked, out);
        }
        match instruction {
            Instruction::Effect {
                operation: operation @ Operation::PlayCards { .. },
                result,
            } => {
                self.play_cards(operation, false, out)?;
                self.report_played(result);
                Ok(())
            }
            Instruction::Effect { operation, result }
                if lane_i::plain_play(operation).is_some() =>
            {
                self.play_cards(operation, false, out)?;
                self.report_played(result);
                Ok(())
            }
            Instruction::Effect {
                operation:
                    Operation::ChooseObjects {
                        player,
                        objects,
                        count,
                        constraints,
                    },
                result,
            } => {
                // The move refuses what can't go there (608.2d); nothing reads
                // that sooner, so such a card can be picked and stays put.
                let constraint = constraints.iter().find(|c| {
                    !matches!(
                        c,
                        SelectionConstraint::CanMoveTo { .. } | SelectionConstraint::AtRandom
                    )
                });
                if let Some(constraint) = constraint {
                    let rest = &body.instructions[i + 1..];
                    return self
                        .choose_with_rule(player, objects, count, constraint, result, rest, out);
                }
                if one_at_most(count) {
                    self.single.insert(result.0);
                }
                if matches!(constraints.as_slice(), [SelectionConstraint::AtRandom]) {
                    let (Count::Exactly(count) | Count::AsManyAsCan(count)) = count else {
                        return Err(gap("random choice of an open count"));
                    };
                    let id = self.fresh();
                    let pool = self.fetch(self.objects(objects)?);
                    let count = self.fetch(self.number(count)?.amount(u32::MAX.into(), "usize")?);
                    writeln!(out, "let mut pool_{id} = {pool}; let count_{id} = {count}; let mut picked_{id}: Vec<ObjectId> = Vec::new(); while picked_{id}.len() < count_{id} && !pool_{id}.is_empty() {{ let at = g.choose_at_random(pool_{id}.len()); picked_{id}.push(pool_{id}.remove(at)); }}").unwrap();
                    self.selections.insert(result.0, format!("picked_{id}"));
                    return Ok(());
                }
                let rest = &body.instructions[i + 1..];
                if self.object_as_it_enters(objects, count, rest, result, out)? {
                    return Ok(());
                }
                let purpose = purpose(result, rest)
                    .or_else(|| purpose(result, &self.after))
                    .or_else(|| referred_to(result, rest).then_some("ChooseAsEnters"))
                    .or_else(|| referred_to(result, &self.after).then_some("ChooseAsEnters"))
                    .or_else(|| lane_l::plain_purpose(result, rest))
                    .or_else(|| lane_l::plain_purpose(result, &self.after))
                    .or_else(|| self.read_later(result).then_some("ChooseAsEnters"))
                    // Nothing it says yet happens to them: the prompt's plain word.
                    .unwrap_or("Choose");
                self.choose(player, objects, count, result, purpose, out)
            }
            Instruction::Effect {
                operation:
                    Operation::Search {
                        player,
                        library,
                        top: None,
                        choices,
                        also,
                        either: false,
                        outside: false,
                    },
                result,
            } if also.is_empty() => {
                let [choice] = choices.as_slice() else {
                    let destination = search_destination(result, &body.instructions[i + 1..]);
                    return self.search_several(
                        player,
                        library,
                        choices,
                        result,
                        destination,
                        out,
                    );
                };
                let rules = self.search_rules(&choice.constraints)?;
                if one_at_most(&choice.count) {
                    self.single.insert(result.0);
                }
                let count = self.select_count(&choice.count)?;
                let id = self.fresh();
                let filter = self.filter(&choice.filter, &Candidate::of("id"))?;
                let destination = search_destination(result, &body.instructions[i + 1..]);
                let searcher = self.fetch(self.certain_player(player)?);
                let owner = self.fetch(self.certain_player(library)?);
                let count = self.fetch(count);
                writeln!(out, "let searcher_{id} = {searcher}; let owner_{id} = {owner}; let rule_count_{id} = {count}; let shown_{id} = cantrip_mechanics::library::searched_cards(g, owner_{id}); let can_{id}: Vec<ObjectId> = {{ let q: &dyn Query = g; shown_{id}.iter().copied().filter(|&id| {filter}).collect() }}; let found_{id} = cantrip_mechanics::library::search(g, searcher_{id}, owner_{id}, asker.source, SearchDestination::{destination}, cantrip_mechanics::select::Selection::listed(rule_count_{id}, &shown_{id}, can_{id}){rules})?;").unwrap();
                self.selections.insert(result.0, format!("found_{id}"));
                self.searched.insert(result.0, "true".into());
                Ok(())
            }
            // The cards left out go somewhere chosen at once, so a whole shuffle
            // then that is the same library.
            Instruction::Effect {
                operation:
                    Operation::Shuffle {
                        players,
                        excluding: Some(excluding),
                    },
                ..
            } => {
                let next = body.instructions[i + 1..]
                    .iter()
                    .find_map(|next| match next {
                        Instruction::Effect {
                            operation: Operation::ReorderLibrary { objects, .. },
                            ..
                        } => Some(objects),
                        _ => None,
                    });
                if format!("{next:?}") != format!("{:?}", Some(excluding)) {
                    return Err(gap("shuffle around cards that stay where they are"));
                }
                writeln!(
                    out,
                    "for player in {} {{ g.shuffle(player); }}",
                    self.fetch(self.players(players)?)
                )
                .unwrap();
                Ok(())
            }
            // "Exile it until this leaves the battlefield" is one effect (610.3).
            Instruction::Effect {
                operation:
                    Operation::Move {
                        objects,
                        destination: Destination::Exile,
                        entry,
                        face_down: false,
                    },
                result,
            } if plain_entry(entry)
                && self.stack_item
                && returns_with_source(result, &body.instructions[i + 1..]) =>
            {
                let id = self.fresh();
                writeln!(out, "let objects_{id} = {}; let by_{id} = g.linked(item); let moved_{id} = g.exile_with(&objects_{id}, by_{id}, true);", self.fetch(self.objects(objects)?)).unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                self.left_behind(result, gone(&format!("objects_{id}")));
                self.returning.insert(result.0);
                Ok(())
            }
            Instruction::Effect {
                operation:
                    Operation::ReturnAfter {
                        objects: ObjectSet::Transition { result, .. },
                        ..
                    },
                result: returned,
            } if self.returning.contains(&result.0) => {
                if let Some(moved) = self.moved.get(&result.0).cloned() {
                    self.moved.insert(returned.0, moved);
                }
                Ok(())
            }
            Instruction::Effect { operation, result } => self.operation(operation, result, out),
            // One instruction of the card's text: no sentence boundary inside,
            // and its results stay in reach of what follows it.
            Instruction::Group(group) => match self.dig(group, out)? {
                true => Ok(()),
                false => self.program(group, out, false),
            },
            Instruction::If {
                condition,
                then,
                otherwise,
            } => {
                let test = self.condition(condition).map(|c| self.fetch(c));
                let test = planned::planned_read(test, "false", condition);
                writeln!(out, "if {test} {{").unwrap();
                self.block(then, out, true)?;
                if otherwise.instructions.is_empty() {
                    writeln!(out, "}}").unwrap();
                } else {
                    writeln!(out, "}} else {{").unwrap();
                    self.block(otherwise, out, true)?;
                    writeln!(out, "}}").unwrap();
                }
                Ok(())
            }
            Instruction::Optional { decider, body } if playing::may_only_play(decider, body) => {
                let mut plays = body;
                while let [Instruction::Group(group)] = plays.instructions.as_slice() {
                    plays = group;
                }
                let [Instruction::Effect { operation, result }] = plays.instructions.as_slice()
                else {
                    return Err(gap("may play"));
                };
                self.play_cards(operation, true, out)?;
                self.report_played(result);
                Ok(())
            }
            // "You may choose new targets" (115.7d) already leaves any of them.
            Instruction::Optional { body, .. } if lane_i::may_retarget(body).is_some() => {
                let (player, object) = lane_i::may_retarget(body).expect("just matched");
                let id = self.fresh();
                self.retarget(id, player, object, &NewTargets::May, out)
            }
            Instruction::Optional { decider, body } if asks_by_paying(decider, body) => {
                let id = self.fresh();
                writeln!(out, "'optional_{id}: {{").unwrap();
                self.asking = Some(id);
                self.block(body, out, true)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::Optional { decider, body }
                if self.pending == Pending::Entering && lane_h::asks_to_copy(body) =>
            {
                self.copy_as_enters(decider, body, out)
            }
            Instruction::Optional { decider, body }
                if self.certain_player(decider).is_err() && self.player(decider).is_ok() =>
            {
                let id = self.fresh();
                let decider = self.fetch(self.player(decider)?);
                writeln!(out, "if let Some(decider_{id}) = {decider} {{ if cantrip_mechanics::effects::ask_may(g, decider_{id}, asker.source, Purpose::Optional)? {{").unwrap();
                self.block(body, out, true)?;
                writeln!(out, "}} }}").unwrap();
                Ok(())
            }
            Instruction::Optional { decider, body } if lane_k::tap_or_untap_only(body) => {
                self.certain_player(decider)?;
                self.may_tap_or_untap = true;
                self.block(body, out, true)?;
                self.may_tap_or_untap = false;
                Ok(())
            }
            Instruction::Optional { decider, body }
                if lane_k::chooses_first(decider, body)
                    && !lane_k::printed_otherwise(self.line_text(), "discard one or more") =>
            {
                writeln!(out, "{{").unwrap();
                self.block_choosing_first(body, String::new(), out)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::Optional { decider, body } => {
                let player = self.certain_player(decider).map(|p| self.fetch(p));
                let player_ref = decider;
                let decider = planned::planned_read(player, "asker.controller", decider);
                let id = self.fresh();
                let (open, spent) = lane_k::once_a_turn(self.do_once.take(), id);
                let open =
                    open + &lane_k::asked_if(player_ref, id, body) + &self.asked_if_there(body);
                writeln!(out, "let decider_{id} = {decider}; if {open}cantrip_mechanics::effects::ask_may(g, decider_{id}, asker.source, Purpose::Optional)? {{{spent}").unwrap();
                self.block(body, out, true)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::ForEachObject {
                objects,
                binding,
                body,
            } => {
                let read = self.objects(objects).map(|o| self.fetch(o));
                let objects = planned::planned_read(read, NO_OBJECTS, objects);
                writeln!(out, "for bound_{} in {objects} {{", binding.0).unwrap();
                self.bound_objects.insert(binding.0);
                let compiled = self.block(body, out, true);
                self.bound_objects.remove(&binding.0);
                compiled?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::ForEachPlayer {
                players,
                binding,
                starting_with,
                in_chosen_direction,
                body: steps,
            } => {
                if self.each_discards_two(players, binding, steps, out) {
                    return Ok(());
                }
                let each = lane_i::EachPlayer {
                    players,
                    binding,
                    starting_with: starting_with.as_ref(),
                    in_chosen_direction: *in_chosen_direction,
                    body: steps,
                    rest: &body.instructions[i + 1..],
                };
                self.each_player(each, out)
            }
            Instruction::Repeatable(inner) => {
                self.hoist_shares(&body.instructions[i + 1..], out);
                self.repeat(None, inner, out)
            }
            Instruction::Times { amount, body } => self.repeat(Some(amount), body, out),
            Instruction::ChooseOne {
                decider,
                options,
                offered: None,
            } => self.choose_one(decider, options, out),
            Instruction::ChooseOne {
                offered: Some(_), ..
            } => Err(gap("a choice among options offered at random")),
        }
    }
    /// The count of a resolution choice. One that cannot be filled is filled as
    /// far as it can be, so "exactly" asks for as many as can be chosen.
    pub(super) fn select_count(&self, count: &Count) -> Result<String> {
        Ok(match count {
            Count::AnyNumber => "cantrip_mechanics::select::Count::AnyNumber".into(),
            Count::UpTo(n) => format!(
                "cantrip_mechanics::select::Count::UpTo({})",
                self.number(n)?.amount(u32::MAX.into(), "u32")?
            ),
            Count::Exactly(n) | Count::AsManyAsCan(n) => format!(
                "cantrip_mechanics::select::Count::AsManyAsCan({})",
                self.number(n)?.amount(u32::MAX.into(), "u32")?
            ),
            Count::AtLeast(_) => return Err(gap("at-least resolution choice")),
            Count::Between { minimum, maximum } => {
                let most = self.fetch(self.number(maximum)?.code);
                let least = self.fetch(self.number(minimum)?.code);
                planned::planned_value(
                    "bounded resolution choice",
                    &format!("cantrip_mechanics::select::Count::UpTo(({most}).max(0) as u32)"),
                    &format!("cantrip_mechanics::select::Count::Between(({least}).max(0) as u32, ({most}).max(0) as u32)"),
                )
            }
            Count::PerPlayer { players, each } => {
                let each = self.select_count(each)?;
                let players = self.fetch(self.players(players)?);
                planned::planned_value(
                    "a move of so many cards from each player's zone",
                    "cantrip_mechanics::select::Count::AnyNumber",
                    &format!("cantrip_mechanics::select::Count::per_player({players}, {each})"),
                )
            }
        })
    }
    pub(super) fn choose(
        &mut self,
        player: &PlayerRef,
        objects: &ObjectSet,
        count: &Count,
        result: &ResultId,
        purpose: &str,
        out: &mut String,
    ) -> Result<()> {
        if let Count::PerPlayer { players, each } = count {
            return self.choose_from_each(player, objects, players, each, result, purpose, out);
        }
        if self.from_outside(objects) {
            self.notes.outside.insert(result.0);
            if matches!(objects, ObjectSet::Union(_)) {
                self.notes.outside_shown.insert(result.0);
            }
        }
        let count = self.offered_count(count);
        let count = self.select_count(&count)?;
        let local = format!("selected_{}", self.fresh());
        let lone = match matches!(objects, ObjectSet::Created(_))
            || lane_k::printed_otherwise(
                self.line_text(),
                "put one onto the battlefield tapped and the other",
            ) {
            true => format!("if {local}_candidates.len() <= 1 {{ {local}_candidates }} else "),
            false => String::new(),
        };
        writeln!(out, "let {local}_player = {}; let {local}_candidates = {}; let {local}_count = {}; let {local} = {lone}{{ cantrip_mechanics::choose::select(g, {local}_player, asker.source, Purpose::{purpose}, {local}_candidates, cantrip_mechanics::select::Selection::of({local}_count, &[]))? }};", self.fetch(self.certain_player(player)?), self.fetch(self.objects(objects)?), self.fetch(count)).unwrap();
        self.open_guard(&local, out);
        self.selections.insert(result.0, local);
        Ok(())
    }
    pub(super) fn fresh(&mut self) -> usize {
        self.next += 1;
        self.next - 1
    }
    /// `Vec<ObjectId>` of what can still be changed: an object that left is
    /// gone, and its last-known information is not a thing to act on (400.7).
    pub(super) fn acted_on(&self, objects: &ObjectSet) -> Result<String> {
        Ok(self.fetch(format!(
            "({}).into_iter().filter(|&o| q.state().obj(o).became.is_none()).collect::<Vec<ObjectId>>()",
            self.objects(objects)?
        )))
    }
    pub(super) fn operation(
        &mut self,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let id = self.fresh();
        match op {
            Operation::PayCost {
                player: PlayerRef::FirstWilling(players),
                cost,
                optional,
            } => self.first_willing(id, players, cost, *optional, result, out)?,
            // The ability an activation or an event put on the stack.
            Operation::Counter {
                objects: ObjectSet::One(ObjectRef::EventStackObject),
                destination: Destination::OwnersGraveyard,
            } if self.frame.activated.is_some() => {
                let ability = self.carried_ability()?;
                writeln!(out, "let items_{id}: Vec<(ObjectId, usize)> = {{ let t = {ability}; let stack = &g.state().stack; stack.iter().position(|e| Target::Ability(e.serial) == t && e.kind != StackKind::Spell).map(|at| (stack[at].object, at)).into_iter().collect() }}; g.counter_items(&items_{id});").unwrap();
            }
            // What targeted this, spell or ability, and where it is on the stack.
            Operation::Counter {
                objects: ObjectSet::One(ObjectRef::EventStackObject),
                destination: Destination::OwnersGraveyard,
            } if self.event_at == EventAt::Item
                && self.frame.stack_controller == Some(Carried::Data(1)) =>
            {
                writeln!(out, "cantrip_api::instruction(g, |g| g.counter_item(ObjectId(item.data[0]), (item.data[1] >> 8) as usize));").unwrap();
            }
            Operation::AddMana {
                players: _,
                mana: ManaProduction::SameTypeAsProduced,
                restriction: None,
            } if self.stack_item => self.same_type_as_produced(out),
            Operation::AddMana {
                players,
                mana: ManaProduction::ColorsAmong { objects, each },
                restriction: None,
            } => self.colors_among(id, players, objects, *each, out)?,
            Operation::AddMana {
                players,
                mana,
                restriction,
            } => match mana_plan(mana) {
                Ok(plan) => self.emit_mana(&plan, players, restriction.as_ref(), None, out)?,
                Err(failed) => {
                    self.stated_mana(players, mana, restriction.as_ref(), failed, out)?
                }
            },
            Operation::ChooseValue {
                actor,
                kind,
                count,
                options,
            } => self.choose_value(id, actor, *kind, *count, options, out)?,
            Operation::Keyword {
                actor,
                action,
                object,
                other,
                amount,
                subtype,
            } => {
                let action = keywords::Action {
                    actor,
                    action: *action,
                    object: object.as_ref(),
                    other: other.as_ref(),
                    amount: amount.as_ref(),
                    subtype: subtype.as_deref(),
                };
                self.keyword_action(id, action, result, out)?;
            }
            Operation::CreateTokens {
                players: PlayerSet::All,
                tokens,
                entry,
            } if self.reads_own_share(tokens) => self.own_share_tokens(tokens, entry, out)?,
            Operation::CreateTokens {
                players,
                tokens,
                entry,
            } => self.create_tokens_p(id, players, tokens, entry, result, out)?,
            Operation::Draw { players, amount }
            | Operation::GainLife { players, amount }
            | Operation::LoseLife { players, amount } => {
                let number = self.number(amount)?;
                let (method, max, ty) = match op {
                    Operation::Draw { .. } => ("draw", u32::MAX.into(), "u32"),
                    Operation::GainLife { .. } => ("gain_life", i32::MAX.into(), "i32"),
                    _ => ("lose_life", i32::MAX.into(), "i32"),
                };
                let amount = self.fetch(number.amount(max, ty)?);
                let players = self.fetch(self.players(players)?);
                writeln!(
                    out,
                    "let amount_{id} = {amount}; let players_{id} = {players};"
                )
                .unwrap();
                if matches!(op, Operation::Draw { .. }) && self.read_later(result) {
                    writeln!(out, "let mut done_{id}: i64 = 0; let mut drawn_{id}: Vec<ObjectId> = Vec::new(); for player in players_{id} {{ let before = g.history().drawn(player) as i64; let held = g.state().zone(Zone::hand(player)).to_vec(); g.draw(player, amount_{id}); done_{id} += g.history().drawn(player) as i64 - before; drawn_{id}.extend(g.state().zone(Zone::hand(player)).iter().copied().filter(|c| !held.contains(c))); }}").unwrap();
                    self.performed.insert(result.0, format!("done_{id}"));
                    self.moved.insert(result.0, format!("drawn_{id}"));
                } else if self.read_later(result) {
                    let measure = "i64::from(g.state().player(player).life)";
                    writeln!(out, "let mut done_{id}: i64 = 0; for player in players_{id} {{ let before = {measure}; g.{method}(player, amount_{id}); done_{id} += ({measure} - before).abs(); }}").unwrap();
                    self.performed.insert(result.0, format!("done_{id}"));
                } else {
                    writeln!(
                        out,
                        "for player in players_{id} {{ g.{method}(player, amount_{id}); }}"
                    )
                    .unwrap();
                }
            }
            Operation::Mill { players, amount } => {
                let number = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out,"let amount_{id} = {}; let mills_{id}: Vec<(PlayerId, u32)> = {}.into_iter().map(|p| (p, amount_{id})).collect(); let milled_{id} = g.mill_together(&mills_{id});",self.fetch(number),self.fetch(self.players(players)?)).unwrap();
                self.moved.insert(result.0, format!("milled_{id}"));
                if let Expr::Constant(n) = amount {
                    self.whole_mills
                        .insert(format!("milled_{id}"), n.to_string());
                }
                self.left_behind(result, format!("milled_{id}.clone()"));
            }
            Operation::Discard {
                players,
                amount,
                random,
            } => {
                let number = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                if *random {
                    let PlayerSet::One(player) = players else {
                        return self.random_discards(id, players, number, result, out);
                    };
                    let amount = self.fetch(number);
                    let player = self.fetch(self.certain_player(player)?);
                    writeln!(
                        out,
                        "let amount_{id} = {amount}; let player_{id} = {player};"
                    )
                    .unwrap();
                    if self.read_later(result) {
                        writeln!(out, "let mut hand_{id} = g.state().zone(Zone::hand(player_{id})).to_vec(); let mut picked_{id}: Vec<ObjectId> = Vec::new(); while (picked_{id}.len() as u32) < amount_{id} && !hand_{id}.is_empty() {{ let at = g.choose_at_random(hand_{id}.len()); picked_{id}.push(hand_{id}.remove(at)); }} let discarded_{id} = g.discard_together(&picked_{id});").unwrap();
                        self.moved.insert(result.0, format!("discarded_{id}"));
                        self.left_behind(result, format!("discarded_{id}.clone()"));
                    } else {
                        writeln!(out, "cantrip_mechanics::discard_at_random(g, player_{id}, amount_{id} as usize);").unwrap();
                    }
                } else {
                    let choice = match std::mem::take(&mut self.may_choose) {
                        true => format!("{{ let hand = g.state().zone(Zone::hand(player)).to_vec(); cantrip_mechanics::choose::cards(g, player, asker.source, Purpose::Discard, hand, amount_{id}, true)? }}"),
                        false => format!("cantrip_mechanics::choose_from_hand(g, player, asker.source, amount_{id})?"),
                    };
                    writeln!(out,"let amount_{id} = {}; let players_{id} = {}; let mut cards_{id} = Vec::new(); for player in players_{id} {{ cards_{id}.extend({choice}); }} let discarded_{id} = g.discard_together(&cards_{id});",self.fetch(number),self.fetch(self.players(players)?)).unwrap();
                    self.open_guard(&format!("cards_{id}"), out);
                    self.moved.insert(result.0, format!("discarded_{id}"));
                    self.shares
                        .insert(result.0, (format!("discarded_{id}"), Share::Owner));
                    self.left_behind(result, format!("discarded_{id}.clone()"));
                }
            }
            Operation::DiscardObjects { objects } => {
                writeln!(
                    out,
                    "let cards_{id} = {}; let discarded_{id} = g.discard_together(&cards_{id});",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.moved.insert(result.0, format!("discarded_{id}"));
                self.left_behind(result, format!("discarded_{id}.clone()"));
            }
            Operation::Surveil { .. }
                if lane_k::printed_otherwise(
                    self.line_text(),
                    "nonland card into your graveyard",
                ) =>
            {
                return Err(gap(
                    "what a surveil put into a graveyard, of a kind the card says",
                ));
            }
            Operation::Scry { players, amount } | Operation::Surveil { players, amount } => {
                let PlayerSet::One(player) = players else {
                    return Err(gap(format!("{} by several players", form(op))));
                };
                let method = if matches!(op, Operation::Scry { .. }) {
                    "scry"
                } else {
                    "surveil"
                };
                let amount = self.number(amount)?.amount(u32::MAX.into(), "usize")?;
                let reported =
                    method == "surveil" && lane_n::reads_graveyard_part(&self.after, result);
                let kept = lane_q::reads_kept_on_top(&self.after, result);
                let (method, bind) = match reported {
                    true => ("surveil_into_graveyard", format!("let moved_{id} = ")),
                    false => (method, String::new()),
                };
                let (look, keep) = lane_q::looked_and_kept(id, kept);
                writeln!(out, "let amount_{id} = {}; let player_{id} = {}; {look}{bind}cantrip_mechanics::{method}(g, player_{id}, asker.source, amount_{id})?;{keep}", self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
                if reported {
                    self.moved.insert(result.0, format!("moved_{id}"));
                }
                if kept {
                    self.performed.insert(result.0, format!("kept_{id}"));
                }
            }
            Operation::SetLifeTotal { players, amount } => {
                let number = self.number(amount)?.amount(i32::MAX.into(), "i32")?;
                writeln!(out,"let amount_{id} = {}; let players_{id} = {}; for player in players_{id} {{ g.set_life_total(player, amount_{id}); }}",self.fetch(number),self.fetch(self.players(players)?)).unwrap();
            }
            Operation::Destroy {
                objects,
                regeneration,
            } => {
                writeln!(
                    out,
                    "let objects_{id} = {}; let destroyed_{id} = g.{}(&objects_{id});",
                    self.fetch(self.objects(objects)?),
                    if *regeneration {
                        "destroy_all"
                    } else {
                        "destroy_all_no_regen"
                    }
                )
                .unwrap();
                self.moved.insert(result.0, format!("destroyed_{id}"));
                self.left_behind(result, gone(&format!("objects_{id}")));
            }
            Operation::Move {
                objects,
                destination,
                entry,
                face_down: false,
            } => self.move_objects_p(id, objects, destination, entry, false, result, out)?,
            Operation::Tap { objects, tapped } => {
                let turn = match tapped {
                    true => format!("g.tap_together(&turned_{id});"),
                    false => format!("cantrip_api::instruction(g, |g| {{ for &object in &turned_{id} {{ g.untap(object); }} }});"),
                };
                writeln!(
                    out,
                    "let objects_{id} = {}; let turned_{id}: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.state().obj(o).tapped != {tapped}).collect(); {turn}",
                    self.acted_on(objects)?,
                )
                .unwrap();
                self.moved.insert(result.0, format!("turned_{id}"));
                // "Those creatures": all of them, whatever state each was in.
                self.selections.insert(result.0, format!("objects_{id}"));
                self.left_behind(result, format!("turned_{id}.clone()"));
            }
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } => self.sacrifice(id, players, objects, choose.as_ref(), result, out)?,
            // "When this creature enters, it deals ... divided": the event's subject
            // here is the source, which is the only way a card says it.
            Operation::DealDamage {
                source: ObjectRef::Source | ObjectRef::EventSubject,
                recipients:
                    Recipients::Objects(ObjectSet::Targets(slot)) | Recipients::Targets(slot),
                amount: Expr::DistributedShare(share),
                preventable: true,
            } if slot.0 == share.0 && self.stack_item => {
                writeln!(out, "cantrip_mechanics::damage_divided(g, item);").unwrap();
            }
            Operation::DealDamage {
                source,
                recipients,
                amount,
                preventable: true,
            } => {
                let amount = self.number(amount)?.amount(u16::MAX.into(), "u16")?;
                let (declare, deal) = if lane_n::read_as_damaged(&self.after, result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                    self.moved.insert(result.0, format!("dealt_{id}"));
                    (
                        format!("let mut done_{id}: i64 = 0; let mut dealt_{id}: Vec<ObjectId> = Vec::new(); let before_{id} = g.history().head() as usize; "),
                        format!("done_{id} += g.damage_together(&batch).iter().map(|&n| i64::from(n)).sum::<i64>(); dealt_{id}.extend(cantrip_mechanics::history::permanents_dealt_damage_by(g.history(), source, before_{id}));"),
                    )
                } else if lane_n::read_as_objects(&self.after, result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                    self.moved.insert(result.0, format!("dealt_{id}"));
                    (
                        format!("let mut done_{id}: i64 = 0; let mut dealt_{id}: Vec<ObjectId> = Vec::new(); "),
                        format!("let dealt = g.damage_together(&batch); done_{id} += dealt.iter().map(|&n| i64::from(n)).sum::<i64>(); dealt_{id}.extend(batch.iter().zip(&dealt).filter(|(_, n)| **n > 0).filter_map(|(d, _)| d.1.object()));"),
                    )
                } else if self.read_later(result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                    (
                        format!("let mut done_{id}: i64 = 0; "),
                        format!("done_{id} += g.damage_together(&batch).iter().map(|&n| i64::from(n)).sum::<i64>();"),
                    )
                } else {
                    (String::new(), "g.damage_together(&batch);".to_string())
                };
                writeln!(out, "{declare}let source_{id} = {}; let amount_{id} = {}; let recipients_{id} = {}; if let Some(source) = source_{id} {{ let batch: Vec<(ObjectId, Target, u16)> = recipients_{id}.iter().map(|t| (source, *t, amount_{id})).collect(); {deal} }}", self.fetch(self.damage_source(source)?), self.fetch(amount), self.fetch(self.recipients(recipients)?)).unwrap();
            }
            Operation::DealDamageParts {
                source,
                parts,
                preventable: true,
            } => {
                writeln!(out, "let source_{id} = {}; let mut batch_{id}: Vec<(ObjectId, Target, u16)> = Vec::new();", self.fetch(self.object(source)?)).unwrap();
                for (i, part) in parts.iter().enumerate() {
                    let amount = self.number(&part.amount)?.amount(u16::MAX.into(), "u16")?;
                    writeln!(out, "let amount_{id}_{i} = {}; if let Some(source) = source_{id} {{ for t in {} {{ batch_{id}.push((source, t, amount_{id}_{i})); }} }}", self.fetch(amount), self.fetch(self.recipients(&part.recipients)?)).unwrap();
                }
                if lane_n::read_as_damaged(&self.after, result) {
                    self.moved.insert(result.0, format!("dealt_{id}"));
                    writeln!(out, "let before_{id} = g.history().head() as usize; g.damage_together(&batch_{id}); let dealt_{id} = source_{id}.map_or(Vec::new(), |source| cantrip_mechanics::history::permanents_dealt_damage_by(g.history(), source, before_{id}));").unwrap();
                } else {
                    writeln!(out, "g.damage_together(&batch_{id});").unwrap();
                }
            }
            Operation::PutCounters { placements }
                if matches!(
                    placements.as_slice(),
                    [CounterPlacement {
                        recipients: Recipients::Objects(ObjectSet::Targets(_))
                            | Recipients::Targets(_),
                        amount: Expr::DistributedShare(_),
                        selection: None,
                        ..
                    }]
                ) && self.stack_item =>
            {
                writeln!(
                    out,
                    "cantrip_mechanics::counters_divided(g, item, {});",
                    counter_kind(&placements[0].kind)?
                )
                .unwrap();
            }
            Operation::PutCounters { placements } => {
                writeln!(
                    out,
                    "let mut counters_{id}: Vec<(ObjectId, CounterKind, i16)> = Vec::new();"
                )
                .unwrap();
                for placement in placements {
                    if let Some(selection) = &placement.selection {
                        self.counters_by_choice(id, placement, selection, out)?;
                        continue;
                    }
                    if let Recipients::Players(players) = &placement.recipients {
                        let method = match &placement.kind {
                            crate::counter::CounterKind::Named(
                                crate::counter::CounterName::PoisonCounter,
                            ) => "give_poison",
                            crate::counter::CounterKind::Named(
                                crate::counter::CounterName::ExperienceCounter,
                            ) => "give_experience",
                            crate::counter::CounterKind::Named(
                                crate::counter::CounterName::RadCounter,
                            ) => {
                                let amount = self
                                    .number(&placement.amount)?
                                    .amount(u32::MAX.into(), "u32")?;
                                let code = format!(
                                    "let amount = {}; for player in {} {{ cantrip_mechanics::rad::give(g, player, amount); }}",
                                    self.fetch(amount),
                                    self.fetch(self.players(players)?)
                                );
                                self.plan_code("rad counters", &code, out);
                                continue;
                            }
                            other => {
                                let kind = counter_kind(other)?;
                                let amount = self
                                    .number(&placement.amount)?
                                    .amount(i16::MAX.into(), "i16")?;
                                let code = format!(
                                    "let amount = {}; for player in {} {{ g.give_player_counters(player, {kind}, amount); }}",
                                    self.fetch(amount),
                                    self.fetch(self.players(players)?)
                                );
                                self.plan_code("counters on a player", &code, out);
                                continue;
                            }
                        };
                        let amount = self
                            .number(&placement.amount)?
                            .amount(u32::MAX.into(), "u32")?;
                        writeln!(
                            out,
                            "let amount = {}; for player in {} {{ g.{method}(player, amount); }}",
                            self.fetch(amount),
                            self.fetch(self.players(players)?)
                        )
                        .unwrap();
                        continue;
                    }
                    let amount = self
                        .number(&placement.amount)?
                        .amount(i16::MAX.into(), "i16")?;
                    let kind = counter_kind(&placement.kind)?;
                    writeln!(out, "let amount = {}; for t in {} {{ if let Target::Object(object) = t {{ counters_{id}.push((object, {kind}, amount)); }} }}", self.fetch(amount), self.fetch(self.recipients(&placement.recipients)?)).unwrap();
                }
                writeln!(out, "let had_{id}: Vec<_> = counters_{id}.iter().map(|c| g.state().obj(c.0).counters(c.1)).collect(); g.add_counters_together(&counters_{id}); let applied_{id}: Vec<ObjectId> = counters_{id}.iter().zip(had_{id}).filter(|(c, had)| g.state().obj(c.0).counters(c.1) > *had).map(|(c, _)| c.0).collect();").unwrap();
                self.selections.insert(result.0, format!("applied_{id}"));
            }
            Operation::RemoveCounters { removals } => {
                self.remove_counters(id, removals, result, out)?
            }
            Operation::Attach { attachments, host } => {
                writeln!(out, "let attachments_{id} = {}; let host_{id} = {}; if let Some(host) = host_{id} {{ cantrip_api::instruction(g, |g| {{ for attachment in attachments_{id} {{ g.attach(attachment, host); }} }}); }}", self.acted_on(attachments)?, self.fetch(self.object(host)?)).unwrap();
            }
            Operation::CreateRegenerationShields { objects } => {
                writeln!(out, "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ g.add_effect(asker.source, object, EffectKind::RegenerationShield, Duration::EndOfTurn); }} }});", self.acted_on(objects)?).unwrap();
            }
            Operation::Reveal { objects } => {
                self.revealed = true;
                self.selections.insert(result.0, format!("objects_{id}"));
                writeln!(out, "let objects_{id} = {}; let mut owners_{id}: Vec<PlayerId> = objects_{id}.iter().map(|o| g.state().obj(*o).owner).collect(); owners_{id}.dedup(); for owner in owners_{id} {{ let cards: Vec<ObjectId> = objects_{id}.iter().copied().filter(|o| g.state().obj(*o).owner == owner).collect(); g.reveal(owner, &cards); }}", self.fetch(self.objects(objects)?)).unwrap();
            }
            // The cards stay in the library until a later instruction moves them (701.20a), so
            // this names every card shown, the qualifying ones and the others alike.
            Operation::RevealUntil {
                library,
                filter,
                count,
                at_most: None,
            } => {
                let owner = self.fetch(self.certain_player(library)?);
                if matches!(count, Expr::Constant(1)) {
                    self.known_one.insert(result.0);
                }
                let count = self.fetch(self.number(count)?.amount(u32::MAX.into(), "usize")?);
                let filter = self.filter(filter, &Candidate::of("id"))?;
                self.selections.insert(result.0, format!("objects_{id}"));
                self.reveal_matches.insert(result.0, format!("found_{id}"));
                self.revealed = true;
                writeln!(out, "let owner_{id} = {owner}; let count_{id} = {count}; let (found_{id}, rest_{id}) = cantrip_mechanics::library::reveal_until_n(g, owner_{id}, count_{id}, &|q: &dyn Query, id: ObjectId| {filter}); let shown_{id} = found_{id}.len() + rest_{id}.len(); let objects_{id}: Vec<ObjectId> = g.state().zone(Zone::library(owner_{id})).iter().copied().take(shown_{id}).collect();").unwrap();
            }
            Operation::CreatePerpetualEffect { objects, changes } => {
                self.perpetual_effect(objects, changes, out)?
            }
            // Each count is read before any counter goes on, and all go on as one
            // event (701.10e, 603.2c).
            Operation::DoubleCounters { objects, kind } => {
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let objects_{id}: Vec<ObjectId> = {objects};").unwrap();
                match kind {
                    Some(kind) => writeln!(
                        out,
                        "cantrip_mechanics::double::counters(g, &objects_{id}, {});",
                        counter_kind(kind)?
                    )
                    .unwrap(),
                    None => writeln!(out, "let puts_{id}: Vec<(ObjectId, CounterKind, i16)> = objects_{id}.iter().flat_map(|&o| {{ let have = g.obj(o).counter_array(); CounterKind::ALL.into_iter().filter(move |&kind| have[kind as usize] > 0).map(move |kind| (o, kind, have[kind as usize])).collect::<Vec<_>>() }}).collect(); g.add_counters_together(&puts_{id});").unwrap(),
                }
            }
            Operation::LookAt { player, objects } => {
                writeln!(
                    out,
                    "let player_{id} = {}; let objects_{id} = {}; g.look_at(player_{id}, &objects_{id});",
                    self.fetch(self.certain_player(player)?),
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.selections.insert(result.0, format!("objects_{id}"));
            }
            Operation::ReorderLibrary {
                player,
                library,
                objects,
                end,
                order,
            } => {
                let library = self.fetch(self.certain_player(library)?);
                let chooser = self.fetch(self.certain_player(player)?);
                let cards = self.fetch(self.objects(objects)?);
                // Only what is still in that library moves (701.20e).
                writeln!(out, "let library_{id} = {library}; let cards_{id}: Vec<ObjectId> = {cards}.into_iter().filter(|&o| g.state().obj(o).became.is_none() && g.state().obj(o).zone == Zone::library(library_{id})).collect(); let placed_{id} = cards_{id}.clone();").unwrap();
                self.moved.insert(result.0, format!("placed_{id}"));
                self.left_behind(result, format!("placed_{id}.clone()"));
                let (kind, place) = match end {
                    SequenceEnd::Top => (
                        "LibraryTop",
                        format!("g.arrange_library(library_{id}, &order_{id}, &[]);"),
                    ),
                    SequenceEnd::Bottom => (
                        "LibraryBottom",
                        format!("g.arrange_library(library_{id}, &[], &order_{id});"),
                    ),
                };
                match (order, end) {
                    (CardOrder::Chosen, _) => writeln!(out, "let chooser_{id} = {chooser}; let order_{id} = cantrip_mechanics::library::ask_library_order(g, chooser_{id}, asker.source, &cards_{id}, DestinationKind::{kind})?; {place}").unwrap(),
                    (CardOrder::Preserve, _) => writeln!(out, "let order_{id} = cards_{id}; {place}").unwrap(),
                    (CardOrder::Random, SequenceEnd::Bottom) => writeln!(out, "g.bottom_in_random_order(library_{id}, &cards_{id});").unwrap(),
                    (CardOrder::Random, SequenceEnd::Top) => return Err(gap("cards on top in a random order")),
                }
            }
            Operation::Shuffle {
                players,
                excluding: None,
            } => {
                writeln!(
                    out,
                    "for player in {} {{ g.shuffle(player); }}",
                    self.fetch(self.players(players)?)
                )
                .unwrap();
            }
            Operation::Counter {
                objects,
                destination: Destination::OwnersGraveyard,
            } if self.stack_targets(objects).is_some() => match self.read_later(result) {
                true => self.counter_reporting(id, objects, result, out),
                false => self.counter_on_stack(id, objects, out),
            },
            Operation::Counter {
                objects:
                    ObjectSet::Zone {
                        scope: ZoneScope::Shared(EventZone::Stack),
                        kind: ObjectKind::Any,
                        filter: ObjectFilter::ControlledBy(players),
                    },
                destination: Destination::OwnersGraveyard,
            } => self.counter_all_on_stack(id, players, result, out)?,
            Operation::Counter {
                objects,
                destination,
            } => {
                let to = match destination {
                    Destination::OwnersGraveyard => None,
                    Destination::OwnersHand => Some("Hand"),
                    Destination::OwnersLibraryTop => Some("LibraryTop"),
                    Destination::OwnersLibraryBottom => Some("LibraryBottom"),
                    Destination::Exile => Some("Exile"),
                    other => return Err(gap(format!("counter into {}", form(other)))),
                };
                let call = match to {
                    Some(to) => format!("g.counter_to(spell, CounterTo::{to});"),
                    None => "g.counter(spell);".into(),
                };
                writeln!(out, "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for spell in objects_{id} {{ {call} }} }});", self.fetch(self.objects(objects)?)).unwrap();
            }
            Operation::CreateContinuousEffect {
                source,
                affected,
                changes,
                duration,
                evaluation,
            } => {
                if self.flashback_until(id, affected, changes, duration, out)? {
                    return Ok(());
                }
                if let EffectSubjects::Players(players) = affected {
                    self.note_own_turn(players, duration);
                    let made =
                        self.continuous_on_players(id, source, players, changes, duration, out);
                    self.notes.until_own_turn = None;
                    return made;
                }
                // "Live" subjects differ from read ones only where the set
                // could gain a member, which names of objects never do, nor
                // what an earlier instruction already picked out.
                let closed = matches!(
                    affected,
                    EffectSubjects::Objects(
                        ObjectSet::One(_)
                            | ObjectSet::Targets(_)
                            | ObjectSet::Selected(_)
                            | ObjectSet::Affected(_)
                            | ObjectSet::Unselected(_)
                            | ObjectSet::Created(_)
                            | ObjectSet::Transition { .. }
                    )
                ) || matches!(
                    affected,
                    EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::Either(names)))
                        if names.iter().all(|name| matches!(name, ObjectFilter::SameObject(_)))
                );
                match evaluation {
                    EffectEvaluation::LiveObjects if !closed => {
                        return self.open_rule(id, source, affected, changes, duration, out)
                    }
                    EffectEvaluation::Live => return Err(gap("live continuous effect")),
                    _ => {
                        self.rule_on_event_object = matches!(
                            affected,
                            EffectSubjects::Objects(ObjectSet::One(ObjectRef::EventSubject))
                        );
                        let made = self.continuous(id, source, affected, changes, duration, out);
                        self.rule_on_event_object = false;
                        made?;
                        self.selections.insert(result.0, format!("objects_{id}"));
                    }
                }
            }
            Operation::BindValue { value } => {
                let value = self.number(value)?;
                writeln!(out, "let value_{id}: i64 = {};", self.fetch(value.code)).unwrap();
                self.values.insert(result.0, format!("value_{id}"));
            }
            Operation::FlipCoin { player } => {
                writeln!(
                    out,
                    "let player_{id} = {}; let flip_{id} = g.flip_coin(player_{id});",
                    self.fetch(self.certain_player(player)?)
                )
                .unwrap();
                self.flips.insert(result.0, format!("flip_{id}"));
            }
            Operation::ModifyEvent { change } => self.modify_entering(change, out)?,
            Operation::CreateTrigger {
                event,
                duration,
                announcement,
                body,
                once,
                intervening_if,
            } => {
                let mut event = event.clone();
                let each = match &mut event {
                    EventSpec::ZoneChange { objects, .. } => {
                        self.about_each_damaged(objects, true, out)
                    }
                    EventSpec::Object { objects, .. } => {
                        self.about_each_damaged(objects, false, out)
                    }
                    _ => None,
                };
                let made = self.create_trigger(
                    &event,
                    duration,
                    announcement,
                    body,
                    *once,
                    intervening_if,
                    out,
                );
                self.end_each_damaged(each, out);
                made?
            }
            Operation::CreateDelayedTrigger {
                event,
                start,
                expires,
                intervening_if,
                body,
            } => self.create_delayed(event, start, expires, intervening_if, body, out)?,
            Operation::CreateReplacement {
                event,
                limit,
                duration,
                body,
            } => {
                self.prevented.extend(lane_n::preventions(body));
                let mut event = event.clone();
                let each = match &mut event {
                    ReplacementEvent::ZoneChange { objects, .. } => {
                        self.about_each_damaged(objects, true, out)
                    }
                    ReplacementEvent::Object { objects, .. } => {
                        self.about_each_damaged(objects, false, out)
                    }
                    _ => None,
                };
                let made = self.create_replacement(&event, limit, duration, body, out);
                self.end_each_damaged(each, out);
                made?
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::PlayerAction { .. }]) => {
                self.player_action(id, player, &cost.parts[0], true, result, out)?
            }
            // These parts have no cost of the engine's: the instruction itself is
            // what the player may do, and "if you do" reads whether it happened.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if self.asking.is_none()
                && (performed_by_itself(cost)
                    || ((lane_l::chooses_what_pays(cost)
                        || lane_n::moves_named_objects(cost))
                        && self.read_later(result))) =>
            {
                let mandatory = Operation::PayCost {
                    player: player.clone(),
                    cost: cost.clone(),
                    optional: false,
                };
                let mut paying = String::new();
                let chooses =
                    lane_k::paid_by_a_choice(cost) && !self.unless_paid.contains(&result.0);
                self.may_choose = chooses;
                self.operation(&mandatory, result, &mut paying)?;
                // A choice that may come to nothing is asked once, not twice.
                let asked = match chooses && !std::mem::take(&mut self.may_choose) {
                    true => "true".to_string(),
                    false => format!("{}cantrip_mechanics::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)?", self.can_pay(id, cost)),
                };
                writeln!(
                    out,
                    "let payer_{id} = {}; let (paid_{id}, after_{id}, before_{id}, picked_{id}): (bool, Vec<ObjectId>, Vec<ObjectId>, Vec<ObjectId>) = if {asked} {{",
                    self.fetch(self.certain_player(player)?)
                )
                .unwrap();
                out.push_str(&paying);
                let inner = self.performed.insert(result.0, format!("paid_{id}"));
                let none = "Vec::new()".to_string();
                let after = self.moved.get(&result.0);
                let before = self.before.get(&result.0);
                let picked = self.selections.get(&result.0);
                let kept = |list: Option<&String>| {
                    list.map_or(none.clone(), |l| self.fetch(format!("({l}).clone()")))
                };
                writeln!(
                    out,
                    "({}, {}, {}, {}) }} else {{ (false, Vec::new(), Vec::new(), Vec::new()) }};",
                    inner.expect("a paid result"),
                    kept(after),
                    kept(before),
                    kept(picked),
                )
                .unwrap();
                // What it moved is what paid, or nothing if it was not paid.
                if self.moved.contains_key(&result.0) {
                    self.moved.insert(result.0, format!("after_{id}"));
                }
                if self.before.contains_key(&result.0) {
                    self.before.insert(result.0, format!("before_{id}.clone()"));
                }
                if self.selections.contains_key(&result.0) {
                    self.selections.insert(result.0, format!("picked_{id}"));
                }
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::ManaAnyNumberOfTimes(_)]) => {
                self.pay_any_number_of_times(id, player, cost, result, out)?
            }
            // "You may pay {X}": the payer says what X is as they pay (107.3f), and
            // paying it, whatever X, is doing it.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::ManaWithX { x: None, .. }]) => {
                let [CostPart::ManaWithX { mana, .. }] = cost.parts.as_slice() else {
                    return Err(gap("pay X"));
                };
                let plain = Cost {
                    parts: vec![CostPart::Mana(mana.clone())],
                };
                let cost = self.cost(&plain, "paid")?;
                self.helpers.insert("pay_question");
                let payer = self.fetch(self.certain_player(player)?);
                writeln!(out, "let payer_{id} = {payer}; let x_{id} = g.may_pay_x(payer_{id}, asker.source, asker.source, {cost}, &PAY_QUESTION)?; let paid_{id} = x_{id}.is_some(); let bound_{id}: i64 = i64::from(x_{id}.unwrap_or(0));").unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
                self.values.insert(result.0, format!("bound_{id}"));
            }
            // X is set here (107.3b): generic mana worked out as this resolves.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(
                cost.parts.as_slice(),
                [CostPart::ManaWithX { x: Some(_), .. }]
            ) =>
            {
                let [CostPart::ManaWithX {
                    mana: symbols,
                    x: Some(x),
                }] = cost.parts.as_slice()
                else {
                    return Err(gap("pay a set X"));
                };
                let base = mana(symbols)?;
                let owed = self.fetch(self.number(x)?.amount(255, "u8")?);
                self.helpers.insert("pay_question");
                let payer = self.fetch(self.certain_player(player)?);
                writeln!(out, "let payer_{id} = {payer}; let owed_{id}: u8 = {owed}; let base_{id} = {base}; let price_{id} = ManaCost {{ generic: base_{id}.generic.saturating_add(owed_{id}.saturating_mul(base_{id}.x)), x: 0, ..base_{id} }}; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, Cost::mana(price_{id}), &PAY_QUESTION)?;").unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if lane_j::counters_on_a_target(player, cost, result).is_some() => {
                let action = lane_j::counters_on_a_target(player, cost, result).expect("just seen");
                return self.player_action(id, player, &action, true, result, out);
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } => {
                let asked = self.ask_to_pay(id, player, cost)?;
                writeln!(out, "{asked}").unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
                self.objects_that_paid(id, result, out);
            }
            Operation::PayCost {
                player,
                cost,
                optional: false,
            } => {
                if let Some(scope) = self.asking.take() {
                    if cost.parts.iter().any(lane_g::payer_sets_the_size) {
                        let chosen = Operation::PayCost {
                            player: player.clone(),
                            cost: cost.clone(),
                            optional: true,
                        };
                        self.operation(&chosen, result, out)?;
                        let paid = self.performed[&result.0].clone();
                        writeln!(out, "if !{paid} {{ break 'optional_{scope}; }}").unwrap();
                        return Ok(());
                    }
                    let asked = self.ask_to_pay(id, player, cost)?;
                    writeln!(out, "{asked} if !paid_{id} {{ break 'optional_{scope}; }}").unwrap();
                    self.performed.insert(result.0, format!("paid_{id}"));
                    return Ok(());
                }
                let [part] = cost.parts.as_slice() else {
                    self.stated_payment(
                        "resolution cost with several parts",
                        id,
                        player,
                        cost,
                        out,
                    )?;
                    self.objects_that_paid(id, result, out);
                    self.performed.insert(result.0, format!("paid_{id}"));
                    return Ok(());
                };
                if let CostPart::PlayerAction { .. } = part {
                    return self.player_action(id, player, part, false, result, out);
                }
                let payer = PlayerSet::One(player.clone());
                match part {
                    CostPart::Sacrifice { objects, choose } => {
                        let mark = out.len();
                        self.sacrifice(id, &payer, objects, choose.as_ref(), result, out)?;
                        lane_k::asks_even_of_one(out, mark);
                        writeln!(out, "let paid_{id} = !picks_{id}.is_empty();").unwrap();
                    }
                    // Paid only with the cards to pay it (CR 118.3).
                    CostPart::Discard {
                        count,
                        filter: ObjectFilter::Any,
                        random: true,
                    } => {
                        let owed = self.fetch(self.number(count)?.amount(u32::MAX.into(), "u32")?);
                        let who = self.fetch(self.certain_player(player)?);
                        writeln!(out, "let owed_{id} = {owed} as usize; let player_{id} = {who}; let paid_{id} = g.state().zone(Zone::hand(player_{id})).len() >= owed_{id}; let hand_{id} = g.state().zone(Zone::hand(player_{id})).to_vec(); if paid_{id} {{ cantrip_mechanics::discard_at_random(g, player_{id}, owed_{id}); }} let discarded_{id}: Vec<ObjectId> = hand_{id}.iter().filter_map(|&o| g.state().obj(o).became).collect();").unwrap();
                        self.moved.insert(result.0, format!("discarded_{id}"));
                        self.left_behind(result, format!("discarded_{id}.clone()"));
                    }
                    CostPart::Discard {
                        count,
                        filter: ObjectFilter::Any,
                        random: false,
                    } => {
                        self.operation(
                            &Operation::Discard {
                                players: payer,
                                amount: count.clone(),
                                random: false,
                            },
                            result,
                            out,
                        )?;
                        writeln!(out, "let paid_{id} = !{}.is_empty();", self.moved(result)?)
                            .unwrap();
                    }
                    CostPart::Reveal {
                        count,
                        filter,
                        random: false,
                    } if reveals_one(count) || matches!(count, Count::AnyNumber) => {
                        let hand = ObjectSet::Zone {
                            scope: ZoneScope::Players {
                                zone: EventZone::Hand,
                                players: payer,
                            },
                            kind: ObjectKind::Card,
                            filter: filter.clone(),
                        };
                        self.choose(player, &hand, count, result, "Reveal", out)?;
                        let picked = self.selections[&result.0].clone();
                        writeln!(out, "let revealer_{id} = {}; g.reveal(revealer_{id}, &{picked}); let paid_{id} = !{picked}.is_empty();", self.fetch(self.certain_player(player)?)).unwrap();
                    }
                    CostPart::Draw { count } => {
                        self.operation(
                            &Operation::Draw {
                                players: payer,
                                amount: count.clone(),
                            },
                            result,
                            out,
                        )?;
                        let drew = self.performed.get(&result.0);
                        let paid = drew.map_or("true".to_string(), |n| format!("{n} > 0"));
                        writeln!(out, "let paid_{id} = {paid};").unwrap();
                    }
                    CostPart::Mill { count } => {
                        self.operation(
                            &Operation::Mill {
                                players: payer,
                                amount: count.clone(),
                            },
                            result,
                            out,
                        )?;
                        writeln!(out, "let paid_{id} = true;").unwrap();
                    }
                    CostPart::Move {
                        selections,
                        destination,
                        entry,
                        face_down: false,
                    } => {
                        let [CostObjectSelection { objects, choose }] = selections.as_slice()
                        else {
                            return Err(gap("resolution cost moving a choice of objects"));
                        };
                        match choose {
                            None => {
                                self.move_objects(id, objects, destination, entry, result, out)?
                            }
                            Some(count @ Count::Exactly(_)) if self.may_choose => {
                                let purpose = lane_g::move_purpose(destination);
                                self.choose(player, objects, count, result, purpose, out)?;
                                let chosen = ObjectSet::Selected(result.clone());
                                self.move_objects(id, &chosen, destination, entry, result, out)?;
                            }
                            Some(Count::Exactly(n)) => {
                                self.select_whole(player, objects, n, destination, result, out)?;
                                let chosen = ObjectSet::Selected(result.clone());
                                self.move_objects(id, &chosen, destination, entry, result, out)?;
                            }
                            Some(count @ Count::AnyNumber) => {
                                let purpose = lane_g::move_purpose(destination);
                                self.choose(player, objects, count, result, purpose, out)?;
                                let chosen = ObjectSet::Selected(result.clone());
                                self.move_objects(id, &chosen, destination, entry, result, out)?;
                            }
                            Some(_) => {
                                return Err(gap("resolution cost moving a choice of objects"))
                            }
                        }
                        // "If you do" is that it got there (608.2c).
                        match (destination, self.moved(result)) {
                            (Destination::Exile, Ok(moved)) => writeln!(out, "let paid_{id} = {moved}.iter().any(|&o| g.obj(o).zone.is(ZoneKind::Exile));").unwrap(),
                            _ => writeln!(out, "let paid_{id} = !objects_{id}.is_empty();").unwrap(),
                        }
                    }
                    other => self.resolution_cost_beyond(id, player, cost, other, result, out)?,
                }
                self.performed.insert(result.0, format!("paid_{id}"));
            }
            Operation::ReflexiveTrigger {
                performed,
                condition,
                announcement,
                body,
            } => self.create_reflexive(performed, condition, announcement, body, out)?,
            Operation::Primitive { name, .. } => {
                return Err(gap(format!("primitive operation {name}")))
            }
            Operation::Meld { objects, into, .. } => self.plan_meld(objects, into, out)?,
            other => self.one_shot(id, other, result, out)?,
        }
        Ok(())
    }
    /// "You may have that player do something": `player` decides, and what they
    /// have done is the cost.
    fn player_action(
        &mut self,
        id: usize,
        player: &PlayerRef,
        part: &CostPart,
        optional: bool,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let CostPart::PlayerAction { body, .. } = part else {
            unreachable!("a player action");
        };
        if optional && lane_k::chooses_first(player, body) {
            writeln!(out, "let mut paid_{id} = false; {{").unwrap();
            self.block_choosing_first(body, format!("paid_{id} = true;"), out)?;
            writeln!(out, "}}").unwrap();
            self.performed.insert(result.0, format!("paid_{id}"));
            self.action_paid(body, result);
            return Ok(());
        }
        if let (true, Some(n)) = (optional, lane_k::blights(player, body)) {
            self.helpers.insert("pay_question");
            let payer = self.fetch(self.certain_player(player)?);
            writeln!(out, "let payer_{id} = {payer}; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, cantrip_mechanics::blight::cost({n}), &PAY_QUESTION)?;").unwrap();
            self.performed.insert(result.0, format!("paid_{id}"));
            self.objects_that_paid(id, result, out);
            return Ok(());
        }
        let willing = if optional {
            writeln!(
                out,
                "let payer_{id} = {};",
                self.fetch(self.certain_player(player)?)
            )
            .unwrap();
            format!("cantrip_mechanics::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)?")
        } else {
            "true".to_string()
        };
        writeln!(out, "let paid_{id} = {willing}; if paid_{id} {{").unwrap();
        self.block(body, out, true)?;
        writeln!(out, "}}").unwrap();
        self.performed.insert(result.0, format!("paid_{id}"));
        self.action_paid(body, result);
        Ok(())
    }
    pub(super) fn left_behind(&mut self, result: &ResultId, list: String) {
        self.before.insert(result.0, list);
    }
    pub(super) fn reset_results(&mut self) {
        self.selections.clear();
        self.unchosen.clear();
        self.reveal_matches.clear();
        self.known_one.clear();
        self.revealed = false;
        self.creations.clear();
        self.moved.clear();
        self.before.clear();
        self.performed.clear();
        self.values.clear();
        self.flips.clear();
        self.chosen.clear();
        self.searched.clear();
        self.failed.clear();
        self.notes = Default::default();
    }
    pub(super) fn create_tokens(
        &mut self,
        id: usize,
        players: &PlayerSet,
        tokens: &[TokenSpec],
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        if tokens
            .iter()
            .all(|token| matches!(token.recipe, TokenRecipe::Named(_)))
        {
            return self.tokens_of_a_card(id, players, tokens, entry, result, out);
        }
        let defined = tokens
            .iter()
            .all(|t| matches!(t.recipe, TokenRecipe::Defined(_)));
        let copy = matches!(
            tokens,
            [TokenSpec {
                recipe: TokenRecipe::Copy { .. },
                ..
            }]
        );
        if !entry.states.is_empty() && (!(defined || copy) || !entry.counters.is_empty()) {
            return Err(gap("tokens entering in a special state"));
        }
        if self.sized_tokens(id, players, tokens, entry, result, out)? {
            return Ok(());
        }
        let first = &tokens[0].words;
        let mixed = tokens
            .iter()
            .any(|t| std::mem::discriminant(&t.words) != std::mem::discriminant(first));
        if mixed && (!defined || !entry.counters.is_empty()) {
            return Err(gap("mixed token text origins in one creation batch"));
        }
        let words_of = |words: &TokenWords| match words {
            TokenWords::Rules => "Words::NONE",
            TokenWords::CreatingEffect => "g.words(asker.source)",
        };
        let words = words_of(first);
        if let [TokenSpec {
            amount,
            recipe:
                TokenRecipe::Copy {
                    object,
                    adjustments,
                },
            ..
        }] = tokens
        {
            if !entry.counters.is_empty() {
                return Err(gap("token copy entering with counters"));
            }
            let PlayerSet::One(player) = players else {
                return self.token_copy_for_each(
                    id,
                    object,
                    adjustments,
                    amount,
                    players,
                    entry,
                    result,
                    out,
                );
            };
            let amount = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
            if adjustments.iter().any(adjusts) || entry.tapped {
                let values = self.copy_values(&format!("original_{id}"), adjustments)?;
                writeln!(out, "let original_{id} = {}; let values_{id} = {values}; let count_{id} = {}; let player_{id} = {}; let created_{id} = match values_{id} {{ Some(values) => g.create_token_copies_tapped(values, player_{id}, count_{id}, {})?, None => Vec::new() }};", self.fetch(self.object(object)?), self.fetch(amount), self.fetch(self.certain_player(player)?), entry.tapped).unwrap();
                self.token_state(id, entry, out)?;
                self.creations.insert(result.0, format!("created_{id}"));
                return Ok(());
            }
            writeln!(out, "let original_{id} = {}; let count_{id} = {}; let player_{id} = {}; let created_{id} = match original_{id} {{ Some(original) => g.create_token_copies(original, player_{id}, count_{id})?, None => Vec::new() }};", self.fetch(self.object(object)?), self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
            self.token_state(id, entry, out)?;
            self.creations.insert(result.0, format!("created_{id}"));
            return Ok(());
        }
        if !entry.counters.is_empty() {
            let ([token], PlayerSet::One(player)) = (tokens, players) else {
                return Err(gap("several kinds of token entering with counters"));
            };
            let TokenRecipe::Defined(definition) = &token.recipe else {
                return Err(gap(format!("token recipe {}", form(&token.recipe))));
            };
            let amount = self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?;
            let definition = self.token_definition(definition)?;
            let mut counters = Vec::new();
            for (kind, n) in &entry.counters {
                let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
                counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
            }
            writeln!(out, "let count_{id} = {}; let player_{id} = {}; let def_{id} = g.card_id(&{definition}); let words_{id} = {words}; let counters_{id} = [{}]; let created_{id} = g.create_tokens_with_counters(def_{id}, player_{id}, count_{id}, {}, &counters_{id}, words_{id})?;", self.fetch(amount), self.fetch(self.certain_player(player)?), counters.join(", "), entry.tapped).unwrap();
            self.creations.insert(result.0, format!("created_{id}"));
            return Ok(());
        }
        if let [TokenSpec {
            amount: Expr::Constant(1),
            recipe:
                TokenRecipe::CopyEach {
                    objects,
                    adjustments,
                },
            ..
        }] = tokens
        {
            if adjustments.iter().any(adjusts) || !entry.counters.is_empty() || entry.tapped {
                return Err(gap("adjusted token copy"));
            }
            let PlayerSet::One(player) = players else {
                return self.token_copies_for_each(id, objects, players, result, out);
            };
            writeln!(out, "let originals_{id} = {}; let player_{id} = {}; let created_{id} = if let Some(player) = player_{id} {{ let copies: Vec<(Copiable, PlayerId)> = originals_{id}.iter().map(|&original| (g.copiable_values(original), player)).collect(); g.create_token_copies_together(&copies)? }} else {{ Vec::new() }};", self.fetch(self.objects(objects)?), self.fetch(self.player(player)?)).unwrap();
            self.creations.insert(result.0, format!("created_{id}"));
            return Ok(());
        }
        writeln!(
            out,
            "let players_{id} = {}; let mut tokens_{id} = Vec::new();",
            self.fetch(self.players(players)?)
        )
        .unwrap();
        for (i, token) in tokens.iter().enumerate() {
            let amount = self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?;
            let TokenRecipe::Defined(definition) = &token.recipe else {
                return Err(gap(format!("token recipe {}", form(&token.recipe))));
            };
            let definition = self
                .token_definition(definition)
                .map_err(|e| e.at(format!("tokens[{i}].definition")))?;
            writeln!(
                out,
                "let count_{id}_{i} = {}; let def_{id}_{i} = g.card_id(&{definition});",
                self.fetch(amount)
            )
            .unwrap();
        }
        writeln!(out, "for player in players_{id} {{").unwrap();
        for i in 0..tokens.len() {
            writeln!(out,"tokens_{id}.extend(std::iter::repeat((def_{id}_{i},player)).take(count_{id}_{i} as usize));").unwrap();
        }
        writeln!(out, "}} let words_{id} = {words};").unwrap();
        let tapped = entry.tapped;
        let creation = format!("g.create_tokens_worded(&tokens_{id}, {tapped}, None, words_{id})?");
        // Each definition's words are its own: a Treasure's are the rules', a
        // named creature's the card's (CR 111.10, 612.2a).
        let creation = if mixed {
            let each: Vec<String> = tokens
                .iter()
                .enumerate()
                .map(|(i, t)| format!("(def_{id}_{i}, {})", words_of(&t.words)))
                .collect();
            planned::planned_value(
                "mixed token text origins in one creation batch",
                &creation,
                &format!(
                    "g.create_tokens_each_worded(&tokens_{id}, {tapped}, None, &[{}])?",
                    each.join(", ")
                ),
            )
        } else {
            creation
        };
        match entry.states.as_slice() {
            [] => writeln!(out, "let created_{id} = {creation};").unwrap(),
            [EntryState::AttachedTo(host)] if !tapped => {
                writeln!(out, "let host_{id} = {}; let mut created_{id}: Vec<ObjectId> = Vec::new(); if let Some(host) = host_{id} {{ for &(def, player) in &tokens_{id} {{ created_{id}.extend(g.create_token_attached(def, player, host)?); }} }}", self.fetch(self.object(host)?)).unwrap();
            }
            [EntryState::Attacking {
                player: Some(player),
                or_planeswalker: false,
            }] => {
                writeln!(out, "let defender_{id} = Target::Player({}); let created_{id} = g.create_tokens_worded(&tokens_{id}, {tapped}, Some(defender_{id}), words_{id})?;", self.fetch(self.certain_player(player)?)).unwrap();
            }
            // Each token's defender is its own choice (508.4); those that
            // attack the same one enter together, as `create_attacking` does.
            [EntryState::Attacking {
                player: None,
                or_planeswalker: false,
            }] => {
                writeln!(out, "let mut chosen_{id}: Vec<Target> = Vec::new(); for &(_, player) in &tokens_{id} {{ chosen_{id}.push(cantrip_mechanics::combat::choose_attack_target(g, player, asker.source)?.unwrap_or(Target::None)); }} let mut created_{id}: Vec<ObjectId> = Vec::new(); let mut done_{id}: Vec<Target> = Vec::new(); for &target in &chosen_{id} {{ if done_{id}.contains(&target) {{ continue; }} done_{id}.push(target); let group: Vec<_> = tokens_{id}.iter().zip(&chosen_{id}).filter(|&(_, &t)| t == target).map(|(&token, _)| token).collect(); created_{id}.extend(g.create_tokens_worded(&group, {tapped}, Some(target), words_{id})?); }}").unwrap();
            }
            _ => {
                writeln!(out, "let created_{id} = {creation};").unwrap();
                self.token_state(id, entry, out)?;
            }
        }
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(())
    }
    pub(super) fn move_objects(
        &mut self,
        id: usize,
        objects: &ObjectSet,
        destination: &Destination,
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let alone = self.one_object(objects);
        let shown = self.outside_shown(objects);
        let revealed = self.revealed;
        let chosen = matches!(objects, ObjectSet::Selected(_));
        let targeted = matches!(objects, ObjectSet::Targets(_));
        let objects = lane_k::shuffled_in_though_gone(objects, destination);
        let outside = self.from_outside(objects);
        let objects = self.fetch(self.objects(objects)?);
        let objects = lane_k::returning(objects, destination);
        let objects = lane_k::on_top_in_turn(objects, chosen, destination);
        let library = |place: String| format!("g.put_into_library(&objects_{id}, {place})");
        let call = match destination {
            Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                let controller = match destination {
                    Destination::Battlefield(player) => {
                        let player = self.certain_player(player)?;
                        writeln!(out, "let player_{id} = {};", self.fetch(player)).unwrap();
                        format!("player_{id}")
                    }
                    _ => "g.state().obj(o).owner".into(),
                };
                let rows = format!("let rows_{id}: Vec<(ObjectId, PlayerId, bool)> = objects_{id}.iter().map(|&o| (o, {controller}, {})).collect();", entry.tapped);
                if !entry.states.is_empty() && !alone {
                    self.arriving_together(id, entry, &controller)?
                } else if let [EntryState::Changes(changes)] = entry.states.as_slice() {
                    self.entering_as(id, entry, changes, &controller)?
                } else if !entry.states.is_empty() {
                    let each = self.arrival(entry)?;
                    format!("{{ let mut moved: Vec<ObjectId> = Vec::new(); for &o in &objects_{id} {{ let controller = {controller}; {each} }} moved }}")
                } else if entry.counters.is_empty() {
                    format!("{{ {rows} let came_{id} = g.put_all_onto_battlefield_as(&rows_{id})?; came_{id}.into_iter().filter(|&o| g.state().obj(o).zone.is(ZoneKind::Battlefield)).collect::<Vec<ObjectId>>() }}")
                } else {
                    let mut counters = Vec::new();
                    for (kind, n) in &entry.counters {
                        let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
                        counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
                    }
                    format!("{{ {rows} let counters = [{}]; g.put_all_onto_battlefield_with_counters(&rows_{id}, &counters)? }}", counters.join(", "))
                }
            }
            Destination::Exile if !entry.counters.is_empty() && entry.states.is_empty() => {
                self.exile_with_counters(id, entry)?
            }
            _ if !plain_entry(entry) => return Err(gap("move arrival flags")),
            // Into the game: CR 400.11b.
            Destination::OwnersHand if outside => lane_p::bring_into_hand(id, revealed || shown),
            // All players can still see the card as it moves (603.10a, 701.20a).
            Destination::OwnersHand if revealed => {
                format!("g.return_to_hand_revealed(&objects_{id})")
            }
            Destination::OwnersHand => format!("g.return_to_hand(&objects_{id})"),
            Destination::Exile if self.links_exile && self.stack_item => {
                format!("{{ let by = g.linked(item); g.exile_with(&objects_{id}, by, false) }}")
            }
            Destination::Exile if self.links_exile => planned::planned_value(
                "exile outside a stack item that an ability reads as exiled with its source",
                &format!("g.exile_all(&objects_{id})"),
                &format!("{{ let by = g.linked_to(asker.source); g.exile_with(&objects_{id}, by, false) }}"),
            ),
            Destination::Exile => format!("g.exile_all(&objects_{id})"),
            // `put_into_graveyard` refuses permanents, so name each destination.
            Destination::OwnersGraveyard => format!("{{ let moves: Vec<(ObjectId, Zone)> = objects_{id}.iter().map(|&o| (o, Zone::graveyard(g.state().obj(o).owner))).collect(); let moved = g.move_together(&moves, MoveReason::Other); moved.into_iter().filter(|&o| g.state().obj(o).zone.kind() == ZoneKind::Graveyard).collect::<Vec<ObjectId>>() }}"),
            Destination::OwnersLibraryTop | Destination::OwnersLibraryBottom if targeted => {
                lane_k::in_an_order_asked(id, destination)
            }
            Destination::OwnersLibraryTop => library("LibraryPlace::Top".into()),
            Destination::OwnersLibraryBottom => library("LibraryPlace::Bottom".into()),
            Destination::OwnersLibraryBottomRandom => {
                library("LibraryPlace::BottomInRandomOrder".into())
            }
            Destination::OwnersLibraryShuffled => library("LibraryPlace::ShuffledIn".into()),
            Destination::OwnersLibraryFromTop(position) => {
                let position = self.number(position)?.amount(u32::MAX.into(), "usize")?;
                let position = self.fetch(position);
                library(format!("LibraryPlace::FromTop(({position}).saturating_sub(1))"))
            }
            Destination::OwnersLibraryTopOrBottom => format!("{{ let mut moved: Vec<ObjectId> = Vec::new(); for &o in &objects_{id} {{ cantrip_mechanics::library::owner_puts_on_top_or_bottom(g, asker.source, o)?; moved.extend(g.state().obj(o).became); }} moved }}"),
            Destination::Ante => planned::planned_value(
                "move to Ante",
                NO_OBJECTS,
                &format!("g.ante(&objects_{id})"),
            ),
            Destination::CommandZone => planned::planned_value(
                "move to CommandZone",
                NO_OBJECTS,
                &format!("g.put_into_command_zone(&objects_{id})"),
            ),
            Destination::OwnersPlanarDeck(end) => planned::planned_value(
                "move to OwnersPlanarDeck",
                NO_OBJECTS,
                &format!("g.put_on_planar_deck(&objects_{id}, SequenceEnd::{end:?})"),
            ),
            other => return Err(gap(format!("move to {}", form(other)))),
        };
        writeln!(
            out,
            "let objects_{id} = {objects}; let moved_{id} = {call};"
        )
        .unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
        if alone {
            self.known_one.insert(result.0);
        }
        self.left_behind(result, gone(&format!("objects_{id}")));
        Ok(())
    }
    /// The one object of a `one_object` move, entering as `entry` says.
    fn arrival(&mut self, entry: &EntrySpec) -> Result<String> {
        let mut counters = Vec::new();
        for (kind, n) in &entry.counters {
            let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
            counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
        }
        let counters = counters.join(", ");
        Ok(match entry.states.as_slice() {
            [EntryState::AttachedTo(host)] if !entry.tapped && counters.is_empty() => format!(
                "if let Some(host) = {} {{ moved.extend(g.put_onto_battlefield_attached(o, controller, host)?); }}",
                self.fetch(self.object(host)?)
            ),
            [EntryState::Attacking {
                player,
                or_planeswalker,
            }, rest @ ..]
                if rest.is_empty() || lane_n::keyword_until_end_of_turn(rest).is_some() =>
            {
                let defender = self.entering_defender(player.as_ref(), *or_planeswalker)?;
                // `put_onto_battlefield_attacking` taps: "tapped and attacking".
                let enter = match entry.tapped {
                    true => format!("let entered = g.put_onto_battlefield_attacking(o, controller, defender, &[{counters}])?; let came = g.is_on_battlefield(entered);"),
                    false => format!("let entered = g.put_onto_battlefield_with_counters(o, controller, false, &[{counters}])?; let came = g.is_on_battlefield(entered); if came {{ g.enter_attacking(&[entered], defender); }}"),
                };
                let gains = match lane_n::keyword_until_end_of_turn(rest) {
                    Some(k) => format!(" cantrip_mechanics::effects::gain_until_end_of_turn(g, asker.source, entered, {});", keyword(k)?),
                    None => String::new(),
                };
                format!("let defender = {defender}; {enter} if came {{ moved.push(entered);{gains} }}")
            }
            [EntryState::When { filter, entry: then }]
                if counters.is_empty()
                    && !entry.tapped
                    && !then.tapped
                    && then.states.is_empty()
                    && !then.counters.is_empty()
                    && self.stack_item
                    && !self.granting =>
            {
                let index = self.arrival_counters(filter, &then.counters)?;
                format!("g.add_effect(item.object, o, EffectKind::Replacement {{ card: item.ability.card, index: {index} }}, Duration::EndOfTurn); moved.extend(g.put_all_onto_battlefield_as(&[(o, controller, false)])?.into_iter().filter(|&o| g.state().obj(o).zone.is(ZoneKind::Battlefield)));")
            }
            _ => return self.arrival_in_state(entry, &counters),
        })
    }
    /// Whether a set is known to hold one object at most when this runs.
    pub(super) fn one_object(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::One(_) => true,
            ObjectSet::Take {
                count: Expr::Constant(1),
                ..
            } => true,
            ObjectSet::Targets(slot) => self
                .slots
                .get(&slot.0)
                .is_some_and(|(_, spec)| one_at_most(&spec.count)),
            ObjectSet::InZone { objects, .. } => self.one_object(objects),
            ObjectSet::Selected(result) => self.single.contains(&result.0),
            ObjectSet::Matching { objects, .. } => match &**objects {
                ObjectSet::Observed(result) => self.known_one.contains(&result.0),
                objects => self.one_object(objects),
            },
            ObjectSet::Transition { result, .. } => self.known_one.contains(&result.0),
            _ => false,
        }
    }
    pub(super) fn remove_counters(
        &mut self,
        id: usize,
        removals: &[CounterRemoval],
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        writeln!(out, "let mut removed_{id}: i64 = 0;").unwrap();
        let mut counted = true;
        for (i, removal) in removals.iter().enumerate() {
            let Recipients::Objects(objects) = &removal.recipients else {
                self.removed_from_players(id, removal, out)?;
                continue;
            };
            let shares = self.outside_shares(removal)?;
            let objects = match &shares {
                Some(shares) => self.fetch(shares),
                None => self.fetch(self.objects(objects)?),
            };
            match &removal.selection {
                // "Remove a counter from each of any number of them": those chosen.
                Some(selection) => {
                    let player = self.fetch(self.certain_player(&selection.player)?);
                    let count = self.select_count(&selection.count)?;
                    let count = self.fetch(count);
                    writeln!(out, "let among_{id}_{i} = {objects}; let chooser_{id}_{i} = {player}; let count_{id}_{i} = {count}; let objects_{id}_{i} = cantrip_mechanics::choose::select(g, chooser_{id}_{i}, asker.source, Purpose::Choose, among_{id}_{i}, cantrip_mechanics::select::Selection::of(count_{id}_{i}, &[]))?;").unwrap();
                }
                None => writeln!(out, "let objects_{id}_{i} = {objects};").unwrap(),
            }
            match (&removal.kind, &removal.amount) {
                (Some(kind), amount) => {
                    let amount = match amount {
                        _ if shares.is_some() => "1i16".to_string(),
                        CounterAmount::All => "i16::MAX".to_string(),
                        CounterAmount::Count(Count::Exactly(n)) => {
                            self.fetch(self.number(n)?.amount(i16::MAX.into(), "i16")?)
                        }
                        other @ CounterAmount::Count(Count::UpTo(n))
                            if !matches!(removal.selection, Some(_)) =>
                        {
                            let _ = other;
                            let n = self.fetch(self.number(n)?.amount(i16::MAX.into(), "i16")?);
                            let kind = counter_kind(kind)?;
                            let player = self.fetch(self.certain_player(&removal.player)?);
                            writeln!(out, "let player_{id}_{i} = {player}; let most_{id}_{i} = {n}; for object in objects_{id}_{i} {{ let most = g.state().obj(object).counters({kind}).min(most_{id}_{i}); let amount = cantrip_mechanics::choose::number(g, player_{id}_{i}, asker.source, 0, most.max(0) as u32, \"Remove\")?; removed_{id} += i64::from(g.remove_counters(object, {kind}, amount as i16)); }}").unwrap();
                            continue;
                        }
                        _ => {
                            self.removal_rest(id, i, removal, out)?;
                            continue;
                        }
                    };
                    writeln!(out, "let amount_{id}_{i} = {amount}; cantrip_api::instruction(g, |g| {{ for object in objects_{id}_{i} {{ removed_{id} += i64::from(g.remove_counters(object, {}, amount_{id}_{i})); }} }});", counter_kind(kind)?).unwrap();
                }
                (None, CounterAmount::All) if self.read_later(result) => {
                    writeln!(out, "cantrip_api::instruction(g, |g| {{ for object in objects_{id}_{i} {{ removed_{id} += CounterKind::ALL.iter().map(|k| i64::from(g.state().obj(object).counters(*k))).sum::<i64>(); cantrip_mechanics::effects::remove_all_counters(g, object); }} }});").unwrap();
                }
                (None, CounterAmount::All) => {
                    counted = false;
                    writeln!(out, "cantrip_api::instruction(g, |g| {{ for object in objects_{id}_{i} {{ cantrip_mechanics::effects::remove_all_counters(g, object); }} }});").unwrap();
                }
                (None, CounterAmount::Count(Count::UpTo(n))) => {
                    let n = self.fetch(self.number(n)?.amount(u8::MAX.into(), "u8")?);
                    let player = self.fetch(self.certain_player(&removal.player)?);
                    writeln!(out, "let player_{id}_{i} = {player}; for object in objects_{id}_{i} {{ removed_{id} += i64::from(g.remove_counters_up_to(player_{id}_{i}, asker.source, object, {n})?); }}").unwrap();
                }
                // "Remove a counter from target permanent": the one who does chooses
                // which of its kinds (CR 122.1, 608.2d).
                (None, CounterAmount::Count(Count::Exactly(Expr::Constant(1)))) => {
                    let player = self.fetch(self.certain_player(&removal.player)?);
                    writeln!(out, "let player_{id}_{i} = {player}; for object in objects_{id}_{i} {{ if g.remove_a_counter_from_among(player_{id}_{i}, asker.source, &[object])? {{ removed_{id} += 1; }} }}").unwrap();
                }
                (None, _) => self.removal_rest(id, i, removal, out)?,
            }
        }
        if counted {
            self.performed.insert(result.0, format!("removed_{id}"));
        }
        Ok(())
    }
    fn sacrifice(
        &mut self,
        id: usize,
        players: &PlayerSet,
        objects: &ObjectSet,
        choose: Option<&Count>,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        self.sacrifice_picks(id, players, objects, choose, out)?;
        writeln!(out, "let sacrificed_{id} = g.sacrifice_each(&picks_{id});").unwrap();
        self.moved.insert(result.0, format!("sacrificed_{id}"));
        self.shares.insert(
            result.0,
            (
                format!("sacrificed_{id}"),
                Share::Picks(format!("picks_{id}")),
            ),
        );
        self.left_behind(result, gone(&format!("objects_{id}")));
        Ok(())
    }
    /// `picks_{id}`: who sacrifices what, chosen but not yet done.
    pub(super) fn sacrifice_picks(
        &mut self,
        id: usize,
        players: &PlayerSet,
        objects: &ObjectSet,
        choose: Option<&Count>,
        out: &mut String,
    ) -> Result<()> {
        let objects = self.fetch(self.objects(objects)?);
        let players = self.fetch(self.players(players)?);
        writeln!(
            out,
            "let objects_{id} = {objects}; let players_{id} = {players}; let mut picks_{id}: Vec<(PlayerId, ObjectId)> = Vec::new();"
        )
        .unwrap();
        match choose {
            None => writeln!(out, "for &object in &objects_{id} {{ let controller = g.controller(object); if players_{id}.contains(&controller) && g.can_sacrifice(controller, object) {{ picks_{id}.push((controller, object)); }} }}").unwrap(),
            Some(count) => {
                let count = self.offered_count(count);
                let count = self.fetch(self.select_count(&count)?);
                writeln!(out, "let count_{id} = {count}; for player in players_{id} {{ let eligible: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.controller(o) == player && g.can_sacrifice(player, o)).collect(); let taken = match count_{id}.clone() {{ cantrip_mechanics::select::Count::Exactly(n) | cantrip_mechanics::select::Count::AsManyAsCan(n) if eligible.len() <= n as usize => eligible, _ => cantrip_mechanics::choose::select(g, player, asker.source, Purpose::Sacrifice, eligible, cantrip_mechanics::select::Selection::of(count_{id}.clone(), &[]))? }}; for object in taken {{ picks_{id}.push((player, object)); }} }}").unwrap();
            }
        }
        Ok(())
    }
    pub(super) fn duration(&mut self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        Ok(match duration {
            D::EndOfTurn => "Duration::EndOfTurn".into(),
            D::EndOfCombat => "Duration::EndOfCombat".into(),
            D::Indefinite => "Duration::Indefinite".into(),
            D::WhileOnBattlefield(ObjectRef::Source) => "Duration::WhileSourceOnBattlefield".into(),
            D::UntilNextTurn(player)
                if self.notes.until_own_turn.as_deref() == Some(format!("{player:?}").as_str()) =>
            {
                "Duration::UntilNextTurnOf(player)".into()
            }
            D::UntilNextTurn(player) => format!(
                "Duration::UntilNextTurnOf({})",
                self.fetch(self.certain_player(player)?)
            ),
            D::UntilEndOfNextTurn(player) => format!(
                "Duration::EndOfNextTurnOf({})",
                self.fetch(self.certain_player(player)?)
            ),
            other => return self.more_duration(other),
        })
    }
    /// One instruction of the card's text: the IR keeps what one instruction
    /// does in one payload, and `instruction` says so to the engine.
    /// A resolving rule on a set that can still gain members (CR 611.2c). Cantrip
    /// keeps exactly two of them, both "can't block this turn". Marking the set's
    /// members now would let a creature that arrives later do what the rule
    /// forbids, so anything else is the engine's gap and never a snapshot.
    fn open_rule(
        &mut self,
        id: usize,
        source: &ObjectRef,
        affected: &EffectSubjects,
        changes: &[Change],
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<()> {
        use crate::mtg_types::CardType;
        let no_blocks = matches!(
            changes,
            [Change::Rule(RuleChange::Prohibit(ObjectAction::Block(
                ObjectFilter::Any
            )))]
        );
        if no_blocks && matches!(duration, crate::ir::Duration::EndOfTurn) {
            let creature =
                |f: &ObjectFilter| matches!(f, ObjectFilter::CardType(CardType::Creature));
            match affected {
                EffectSubjects::Objects(ObjectSet::Battlefield(f)) if creature(f) => {
                    writeln!(
                        out,
                        "cantrip_api::instruction(g, |g| g.creatures_cant_block());"
                    )
                    .unwrap();
                    return Ok(());
                }
                EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::All(filters)))
                    if filters.len() == 2 && filters.iter().any(creature) =>
                {
                    if let Some(players) = filters.iter().find_map(|f| match f {
                        ObjectFilter::ControlledBy(players) => Some(players),
                        _ => None,
                    }) {
                        writeln!(
                            out,
                            "let players_{id} = {}; let source_{id} = {}; if let Some(source) = source_{id} {{ cantrip_api::instruction(g, |g| {{ for player in players_{id} {{ g.add_effect(source, source, EffectKind::CreaturesCantBlock(player), Duration::EndOfTurn); }} }}); }}",
                            self.fetch(self.players(players)?),
                            self.fetch(self.object(source)?),
                        )
                        .unwrap();
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
        let EffectSubjects::Objects(affected) = affected else {
            return Err(gap("continuous effect on players"));
        };
        self.open_rule_stated(source, affected, changes, duration, out)
    }
    fn continuous(
        &mut self,
        id: usize,
        source: &ObjectRef,
        affected: &EffectSubjects,
        changes: &[Change],
        duration: &crate::ir::Duration,
        out: &mut String,
    ) -> Result<()> {
        let EffectSubjects::Objects(affected) = affected else {
            return Err(gap("continuous effect on players"));
        };
        // Exerting is exactly this (CR 701.43a), and no event tells them apart.
        if let (
            [Change::Rule(RuleChange::UntapStep {
                behavior: UntapBehavior::Skip,
                ..
            })],
            crate::ir::Duration::DuringControllersNextUntapStep,
        ) = (changes, duration)
        {
            writeln!(
                out,
                "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for &object in &objects_{id} {{ g.exert(object); }} }});",
                self.acted_on(affected)?
            )
            .unwrap();
            return Ok(());
        }
        let duration = self.duration(&lane_h::duration_on(affected, duration))?;
        writeln!(
            out,
            "let objects_{id} = {}; let source_{id} = {}; let duration_{id} = {duration};",
            self.acted_on(affected)?,
            self.fetch(self.object(source)?),
        )
        .unwrap();
        if let [Change::CopyOf { object, effects }] = changes {
            let original = self.fetch(self.objects(&ObjectSet::One(object.clone()))?);
            let values = self.copy_values(
                &format!("original_{id}.first().copied()"),
                std::slice::from_ref(effects),
            )?;
            writeln!(out, "let original_{id} = {original}; let values_{id} = {values}; if let (Some(source), Some(values)) = (source_{id}, values_{id}) {{ cantrip_api::instruction(g, |g| {{ for &object in &objects_{id} {{ g.become_copy(source, object, values, duration_{id}); }} }}); }}").unwrap();
            return Ok(());
        }
        // Each object has its own value, all read before any is applied.
        if read::reads_affected(changes) {
            self.affected_object = Some("object");
            let kinds = changes
                .iter()
                .map(|change| self.effect_kinds(change))
                .collect::<Result<Vec<_>>>();
            self.affected_object = None;
            let pushes: String = kinds?
                .into_iter()
                .flatten()
                .map(|kind| {
                    format!(
                        "let kind = {}; all.push((object, kind)); ",
                        self.fetch(kind)
                    )
                })
                .collect();
            writeln!(out, "let kinds_{id} = {{ let mut all = Vec::new(); for &object in &objects_{id} {{ {pushes} }} all }}; if let Some(source) = source_{id} {{ cantrip_api::instruction(g, |g| {{ for &(object, kind) in &kinds_{id} {{ g.add_effect(source, object, kind, duration_{id}); }} }}); }}").unwrap();
            return Ok(());
        }
        let mut kinds = Vec::new();
        for change in changes {
            for (i, kind) in self.effect_kinds(change)?.into_iter().enumerate() {
                let local = format!("kind_{id}_{}_{i}", kinds.len());
                writeln!(out, "let {local} = {};", self.fetch(kind)).unwrap();
                kinds.push(local);
            }
        }
        let adds: String = kinds
            .iter()
            .map(|k| format!("g.add_effect(source, object, {k}, duration_{id}); "))
            .collect();
        writeln!(out, "if let Some(source) = source_{id} {{ cantrip_api::instruction(g, |g| {{ for &object in &objects_{id} {{ {adds} }} }}); }}").unwrap();
        Ok(())
    }
    /// What a resolving effect's change is in the engine's vocabulary. Each
    /// value is read when the effect is made, as `EffectEvaluation::Snapshot` says.
    pub(super) fn effect_kinds(&mut self, change: &Change) -> Result<Vec<String>> {
        if let Change::GrantKeyword(
            ward @ (Keyword::Ward(_) | Keyword::Flanking | Keyword::Prowess),
        ) = change
        {
            let change = Change::GrantAbility(Box::new(Ability::Keyword(ward.clone())));
            return self.effect_kinds(&change);
        }
        if let Change::GrantAbility(ability) = change {
            if let Some(keyword) = self.grants_keyword(ability) {
                return Ok(vec![format!(
                    "EffectKind::AddKeywords(Keywords::of(&[{keyword}]))"
                )]);
            }
            return self.grant_kinds(ability);
        }
        let i16_of = |n: &Expr| -> Result<String> {
            self.number(n)?.fit(i16::MIN.into(), i16::MAX.into(), "i16")
        };
        Ok(match change {
            Change::Primitive { name, .. } => {
                return Err(gap(format!("primitive continuous change {name}")))
            }
            Change::Pump { power, toughness } => vec![format!(
                "EffectKind::Pump({}, {})",
                self.pump_amount(power, i16_of(power)?),
                self.pump_amount(toughness, i16_of(toughness)?)
            )],
            Change::SetBasePowerToughness { power, toughness } => vec![format!(
                "EffectKind::SetPt({}, {})",
                i16_of(power)?,
                i16_of(toughness)?
            )],
            Change::GrantKeyword(Keyword::Protection(quality)) => {
                if let Some((kind, _)) = self.chosen_protection(quality)? {
                    return Ok(vec![kind]);
                }
                let mut grants = Vec::new();
                self.grant_protection(quality, &mut grants)?;
                grants.into_iter().map(|(kind, _)| kind).collect()
            }
            Change::ChosenColor { .. } | Change::ChosenType { .. } => self.chosen_effect(change)?,
            Change::ChosenAbility => self.chosen_ability()?,
            Change::GrantKeyword(Keyword::HexproofFrom(quality)) => {
                vec![format!(
                    "EffectKind::HexproofFrom({})",
                    self.hexproof_of(quality)?
                )]
            }
            Change::GrantKeyword(k) => {
                vec![format!(
                    "EffectKind::AddKeywords(Keywords::of(&[{}]))",
                    keyword(k)?
                )]
            }
            Change::RemoveAllAbilities => vec!["EffectKind::LoseAllAbilities".into()],
            Change::SwitchPowerToughness => vec!["EffectKind::SwitchPt".into()],
            Change::LosesAbility(lost) => match statics::lost_keyword(lost) {
                Ok(word) => vec![format!(
                    "EffectKind::LoseKeywords(Keywords::of(&[Keyword::{word}]))"
                )],
                Err(failed) => return self.lost_kinds(lost, failed),
            },
            Change::AddLandType(t) => vec![format!(
                "EffectKind::AddSubtype({})",
                lane_k::word_subtype(&name(t))
            )],
            // A land type set takes the land's rules text with it (CR 305.7),
            // which only `sets_land_subtypes`, a static ability, does.
            Change::SetLandType(land) => {
                let word = match &**land {
                    Operand::Vocabulary { form: word, .. } => format!("Subtype::{word}"),
                    other => lane_q::second_chosen_land_type(other)
                        .ok_or_else(|| gap(format!("land type {}", form(other))))?,
                };
                vec![lane_g::planned_kind(
                    "engine: no resolving effect sets land types",
                    format!("EffectKind::SetLandType({word})"),
                )]
            }
            Change::SetPower(n) => vec![lane_g::planned_kind(
                "engine: no effect sets power or toughness alone",
                format!("EffectKind::SetPower({})", i16_of(n)?),
            )],
            Change::SetToughness(n) => vec![lane_g::planned_kind(
                "engine: no effect sets power or toughness alone",
                format!("EffectKind::SetToughness({})", i16_of(n)?),
            )],
            Change::AddSupertype(t) => vec![lane_g::planned_kind(
                "engine: no effect changes supertypes",
                format!("EffectKind::AddSupertype(Supertype::{t:?})"),
            )],
            Change::RemoveSupertype(t) => vec![lane_g::planned_kind(
                "engine: no effect changes supertypes",
                format!("EffectKind::LoseSupertype(Supertype::{t:?})"),
            )],
            Change::RemoveCardType(t) => vec![lane_g::planned_kind(
                "engine: no effect removes a card type",
                format!("EffectKind::LoseType(CardType::{})", card_type(t)?),
            )],
            Change::Colors {
                mode: ChangeMode::Add,
                colors,
            } => vec![lane_g::planned_kind(
                "engine: no effect adds colors",
                format!("EffectKind::AddColors(ColorSet({}))", color_mask(colors)?),
            )],
            Change::AddCardType(t) => vec![format!(
                "EffectKind::AddTypes(TypeSet::of(&[CardType::{}]))",
                card_type(t)?
            )],
            Change::SetCardTypes(types) => {
                let types = types
                    .iter()
                    .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                    .collect::<Result<Vec<_>>>()?;
                vec![format!(
                    "EffectKind::SetTypes(TypeSet::of(&[{}]))",
                    types.join(", ")
                )]
            }
            Change::Colors {
                mode: ChangeMode::Set,
                colors,
            } => {
                let mut mask = 0u8;
                for c in colors {
                    color(c)?;
                    mask |= 1 << color_index(c)?;
                }
                vec![format!(
                    "EffectKind::SetColors({})",
                    lane_k::word_colors(mask)
                )]
            }
            // Set to none: "loses all creature types".
            Change::CreatureTypes {
                mode: ChangeMode::Set,
                types: CreatureTypeSet::Listed(types),
            } if types.is_empty() => vec!["EffectKind::LoseAllCreatureTypes".into()],
            Change::CreatureTypes {
                mode,
                types: CreatureTypeSet::Listed(types),
            } => {
                let mut kinds = Vec::new();
                for (i, t) in types.iter().enumerate() {
                    let word = name(t);
                    if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                        return Err(gap(format!("subtype {word}")));
                    }
                    kinds.push(if i == 0 && matches!(mode, ChangeMode::Set) {
                        format!(
                            "EffectKind::SetCreatureType({})",
                            lane_k::word_subtype(&word)
                        )
                    } else {
                        format!("EffectKind::AddSubtype({})", lane_k::word_subtype(&word))
                    });
                }
                kinds
            }
            // Being all of them leaves nothing for "set" to take away.
            Change::CreatureTypes {
                types: CreatureTypeSet::All,
                ..
            } => vec!["EffectKind::AddAllCreatureTypes".into()],
            Change::Rule(RuleChange::Prohibit(ObjectAction::Attack(AttackDefenders::Any))) => {
                vec!["EffectKind::CantAttack".into()]
            }
            Change::Rule(RuleChange::Prohibit(ObjectAction::Block(ObjectFilter::Any))) => {
                vec!["EffectKind::CantBlock".into()]
            }
            Change::Rule(RuleChange::Prohibit(ObjectAction::BeBlocked(ObjectFilter::Any))) => {
                vec!["EffectKind::CantBeBlocked".into()]
            }
            Change::Rule(RuleChange::RequireCombat {
                action: ObjectAction::Attack(AttackDefenders::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            }) => vec!["EffectKind::MustAttack".into()],
            Change::Rule(RuleChange::RequireCombat {
                action: ObjectAction::Block(ObjectFilter::Any),
                amount: RequirementAmount::AtLeast(Expr::Constant(1)),
            }) => vec!["EffectKind::MustBlock".into()],
            Change::SetController(player) => vec![format!(
                "EffectKind::Control({})",
                self.certain_player(player)?
            )],
            Change::Rule(rule) => return self.continuous_rule(rule),
            other => return self.more_kinds(other),
        })
    }
}

pub(super) fn rule_name(rule: &RuleChange) -> String {
    match rule {
        RuleChange::Primitive { name, .. } => format!("primitive {name}"),
        RuleChange::Prohibit(action) => format!("Prohibit {}", form(action)),
        RuleChange::RequireCombat { action, .. } => format!("RequireCombat {}", form(action)),
        other => form(other),
    }
}
