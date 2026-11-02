//! What a resolving effect does past the arms of `effects` and `lane_i`: the
//! operations the first round left, each the code Cantrip runs today, or the code
//! it will run once the requirement it names exists.
use super::*;

const M: &str = "cantrip_mechanics";

/// What "do this only once each turn" offers, under a "may" that already asks it.
pub(super) fn once_offered(body: &Program) -> Option<&Program> {
    match body.instructions.as_slice() {
        [Instruction::Effect {
            operation:
                Operation::PlayersDo {
                    players: PlayerSet::One(PlayerRef::AbilityController),
                    any_one: false,
                    optional: true,
                    once_each_turn: true,
                    body,
                    otherwise,
                },
            ..
        }] if otherwise.is_empty() => match body.as_slice() {
            [Operand::Program(offered)] => Some(offered),
            _ => None,
        },
        _ => None,
    }
}

/// A cost offered for an object and what stands for "no one paid for it": `body`'s
/// one payment, or the first willing of `players` where each is offered it.
pub(super) fn paid_for_each(body: &Program) -> Option<(ResultId, Instruction)> {
    let [only] = body.instructions.as_slice() else {
        return None;
    };
    match only {
        Instruction::Effect {
            result,
            operation: Operation::PayCost { optional: true, .. },
        } => Some((result.clone(), only.clone())),
        Instruction::ForEachPlayer {
            players,
            binding,
            starting_with: None,
            in_chosen_direction: false,
            body,
        } => {
            let [Instruction::Effect {
                result,
                operation:
                    Operation::PayCost {
                        player: PlayerRef::Bound(payer),
                        cost,
                        optional: true,
                    },
            }] = body.instructions.as_slice()
            else {
                return None;
            };
            (payer.0 == binding.0).then(|| {
                let first = PlayerRef::FirstWilling(Box::new(players.clone()));
                let offered = Instruction::Effect {
                    result: result.clone(),
                    operation: Operation::PayCost {
                        player: first,
                        cost: cost.clone(),
                        optional: true,
                    },
                };
                (result.clone(), offered)
            })
        }
        _ => None,
    }
}

/// The one instruction `player` is had to do, said of them. The reduction leaves
/// the doer as "you" in `body`, which only these forms read back without doubt.
fn had_to_do(player: &PlayerRef, body: &[Operand]) -> Option<Program> {
    let [Operand::Program(program)] = body else {
        return None;
    };
    let [Instruction::Effect { result, operation }] = program.instructions.as_slice() else {
        return None;
    };
    let them = || PlayerSet::One(player.clone());
    let you = |players: &PlayerSet| matches!(players, PlayerSet::One(PlayerRef::AbilityController));
    let operation = match operation {
        Operation::Mill { players, amount } if you(players) => Operation::Mill {
            players: them(),
            amount: amount.clone(),
        },
        Operation::Shuffle {
            players,
            excluding: None,
        } if you(players) => Operation::Shuffle {
            players: them(),
            excluding: None,
        },
        _ => return None,
    };
    let mut program = program.clone();
    program.instructions = vec![Instruction::Effect {
        result: result.clone(),
        operation,
    }];
    Some(program)
}

/// The prompt of a choice that nothing does anything to by name: "chooses three
/// permanents, then sacrifices the rest" keeps what it chooses, and any other
/// choice a later instruction reads is a plain choice.
pub(super) fn plain_purpose(result: &ResultId, rest: &[Instruction]) -> Option<&'static str> {
    let unselected = format!("Unselected(ResultId({}))", result.0);
    let earlier = "Earlier { what: Selected";
    rest.iter().find_map(|instruction| {
        let text = format!("{instruction:?}");
        if text.contains(&unselected) {
            Some("Keep")
        } else if text.contains(earlier) {
            Some("ChooseAsEnters")
        } else {
            None
        }
    })
}

/// What a target slot asks of its targets past their count and filter.
#[derive(Default)]
pub(super) struct SlotRule {
    /// The slot's rule of its targets together, as code.
    pub(super) together: String,
    /// A further test of each target, past the slot's filter.
    pub(super) test: Option<String>,
    /// What waits on Cantrip: the requirement and the code it adds to the slot.
    pub(super) planned: Vec<(String, String)>,
}

impl SlotRule {
    /// The slot, with what waits on Cantrip around it.
    pub(super) fn finish(&self, slot: String) -> String {
        self.planned.iter().fold(slot, |slot, (feature, code)| {
            planned::planned_value(feature, &slot, &format!("{slot}{code}"))
        })
    }
}

/// "One that hasn't been chosen": the engine's history of `Event::ModesChosen`.
pub(super) fn mode_history(unchosen: &Option<ModeHistory>) -> String {
    match unchosen {
        Some(ModeHistory::ThisTurn) => format!(".allowed({M}::history::mode_not_chosen_this_turn)"),
        Some(ModeHistory::ThisGame) => format!(".allowed({M}::history::mode_not_chosen_before)"),
        // Waits on Cantrip: `modes_to_come`.
        Some(ModeHistory::LastCombat(_)) | None => String::new(),
    }
}

/// How `Operation::AttachEach` pairs what is attached with what it goes on.
#[derive(Clone, Copy)]
enum AttachShape {
    /// All of them, on the one chosen before.
    AllToTheChosen,
    /// "For each of those tokens, you may attach an Equipment you control to it."
    OneToEach,
    /// "For each Aura and Equipment you control, you may attach it to a creature."
    EachToOne,
}

/// `Game::attach`, which leaves in place what can't go there (CR 701.3b). What
/// goes where is the actor's as the instruction is followed, one at a time.
fn attach_each(id: usize, player: &str, attachments: &str, hosts: &str, shape: AttachShape, out: &mut String) {
    let one = format!("{M}::select::Selection::of({M}::select::up_to(1), &[])");
    let ask = |cards: &str| format!("{M}::choose::select(g, player_{id}, asker.source, Purpose::Attach, {cards}, {one})?");
    write!(out, "let player_{id} = {player}; let attachments_{id} = {attachments}; let hosts_{id} = {hosts}; ").unwrap();
    match shape {
        AttachShape::AllToTheChosen => writeln!(out, "let _ = player_{id}; if let Some(&host) = hosts_{id}.first() {{ cantrip_api::instruction(g, |g| {{ for &attachment in &attachments_{id} {{ g.attach(attachment, host); }} }}); }}"),
        AttachShape::OneToEach => writeln!(out, "for &host in &hosts_{id} {{ let chosen = {}; if let Some(&attachment) = chosen.first() {{ g.attach(attachment, host); }} }}", ask(&format!("attachments_{id}.clone()"))),
        AttachShape::EachToOne => writeln!(out, "for &attachment in &attachments_{id} {{ let chosen = {}; if let Some(&host) = chosen.first() {{ g.attach(attachment, host); }} }}", ask(&format!("hosts_{id}.clone()"))),
    }
    .unwrap();
}

/// A rule that no two of a slot's targets share a key.
fn distinct_rule(functions: &mut String, name: &str, kind: &str, key: &str, words: &str) -> String {
    if !functions.contains(&format!("fn {name}(")) {
        writeln!(functions, "fn {name}_key(q: &dyn Query, id: ObjectId) -> {kind} {{ {key} }}\nfn {name}(q: &dyn Query, choice: &Choice, _: u32) -> Verdict {{ let used: Vec<{kind}> = choice.selected.iter().map(|&c| {name}_key(q, c)).collect(); let apart = used.iter().enumerate().all(|(i, k)| !used[..i].contains(k)); Verdict {{ complete: apart, addable: Bits::of(choice.candidates.len(), |i| !used.contains(&{name}_key(q, choice.candidates[i]))) }} }}\nfn largest_{name}(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> {{ cantrip_api::select::largest_distinct(choice, |c| {name}_key(q, c)) }}").unwrap();
    }
    format!("cantrip_api::select::set_part(&({name} as SelectFn), &(largest_{name} as cantrip_api::select::LargestFn), 0, {words:?})")
}

/// "With total power N or less": a rule of the set, each value read as it is.
fn total_rule(functions: &mut String, name: &str, read: &str, words: &str, most: &str) -> String {
    if !functions.contains(&format!("fn {name}(")) {
        writeln!(functions, "fn {name}_of(q: &dyn Query, id: ObjectId) -> u64 {{ i64::from({read}).max(0) as u64 }}\nfn {name}(q: &dyn Query, choice: &Choice, most: u32) -> Verdict {{ let total: u64 = choice.selected.iter().map(|&c| {name}_of(q, c)).sum(); Verdict {{ complete: total <= u64::from(most), addable: Bits::of(choice.candidates.len(), |i| total + {name}_of(q, choice.candidates[i]) <= u64::from(most)) }} }}\nfn largest_{name}(q: &dyn Query, choice: &Choice, most: u32) -> Vec<ObjectId> {{ cantrip_api::select::largest_within(choice, |c| {name}_of(q, c), u64::from(most)) }}").unwrap();
    }
    format!("cantrip_api::select::set_part(&({name} as SelectFn), &(largest_{name} as cantrip_api::select::LargestFn), {most}, {words:?})")
}

/// The instructions of one option of a choice between actions: a program, the
/// programs of a list one after another, or a named action's.
pub(super) fn action_program(option: &Operand) -> Option<Program> {
    match option {
        Operand::Program(program) => Some(program.clone()),
        Operand::List(items) => {
            let mut instructions = Vec::new();
            for item in items {
                instructions.extend(action_program(item)?.instructions);
            }
            Some(Program { instructions })
        }
        Operand::Term { form, operands, .. } if form == "NamedAction" => {
            match operands.as_slice() {
                [Operand::Text(_), list] => action_program(list),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Whether `count` is what a reflexive ability's slot was told.
pub(super) fn sizes_slot(sized: &Option<String>, count: &Expr) -> bool {
    sized.as_deref() == Some(format!("{count:?}").as_str())
}

/// A target rule of a reflexive ability that reads what its ability did: no
/// local reaches a target filter, and the engine tells the ability one count.
pub(super) fn reads_as_a_target(failed: Diagnostic) -> Diagnostic {
    let read = matches!(
        failed.feature.as_str(),
        "transition this instruction does not report"
            | "result this instruction does not report"
            | "selection result outside generated scope"
            | "token result outside generated scope"
            | "event object this trigger does not bind"
            | "event player this trigger does not bind"
            | "event amount this trigger does not bind"
    );
    if !read || !failed.path.starts_with("targets") {
        return failed;
    }
    let path = failed.path.clone();
    gap("target of a reflexive ability reading what its ability did").at(path)
}

/// A cost a player pays at resolution by picking what pays it, any number of
/// them: done as the instruction it is, so what picked is known. `may_pay` has
/// no way to say which objects paid.
pub(super) fn chooses_what_pays(cost: &Cost) -> bool {
    match cost.parts.as_slice() {
        [CostPart::Sacrifice {
            choose: Some(Count::AnyNumber),
            ..
        }]
        | [CostPart::Reveal {
            count: Count::AnyNumber,
            random: false,
            ..
        }] => true,
        [CostPart::Move {
            selections,
            face_down: false,
            ..
        }]
        | [CostPart::ChangeTapState {
            selections,
            tapped: true,
            symbol: false,
        }] => matches!(
            selections.as_slice(),
            [CostObjectSelection {
                choose: Some(Count::AnyNumber),
                ..
            }]
        ),
        _ => false,
    }
}

/// A search for several things at once (CR 701.23h): cards fit when each can be
/// one of the things searched for, none more than that thing's number, and any of
/// them may go unfound (CR 701.23b).
pub(super) const CLAUSES: &str = "fn clauses_fit(cards: &[ObjectId], clauses: &mut [(u32, &dyn Fn(ObjectId) -> bool)]) -> bool { let Some((&card, rest)) = cards.split_first() else { return true; }; for i in 0..clauses.len() { if clauses[i].0 > 0 && (clauses[i].1)(card) { clauses[i].0 -= 1; let fits = clauses_fit(rest, clauses); clauses[i].0 += 1; if fits { return true; } } } false }\nfn clauses_verdict(choice: &Choice, clauses: &mut [(u32, &dyn Fn(ObjectId) -> bool)]) -> Verdict { let complete = clauses_fit(choice.selected, clauses); let addable = Bits::of(choice.candidates.len(), |i| { let card = choice.candidates[i]; !choice.selected.contains(&card) && { let with: Vec<ObjectId> = choice.selected.iter().copied().chain([card]).collect(); clauses_fit(&with, clauses) } }); Verdict { complete, addable } }\n";

impl Compiler {
    /// A count made by the ability that makes a reflexive one, which that reflexive
    /// ability reads: the engine tells it one count, kept as `item.x`.
    pub(super) fn told_amount(
        &self,
        performed: &ResultId,
        body: &Program,
        announcement: &Announcement,
    ) -> Option<String> {
        let local = self.performed.get(&performed.0)?;
        let text = format!("{body:?} {announcement:?}");
        let reads = [
            format!("Performed(ResultId({}))", performed.0),
            format!("CountersRemoved(ResultId({}))", performed.0),
        ];
        let counts = local.starts_with("removed_") || local.starts_with("done_");
        (counts && reads.iter().any(|read| text.contains(read)))
            .then(|| format!("({local} as i64)"))
    }
    /// The first slot's count, if the ability making it reads it now: "up to that
    /// many target creatures". The code of the count goes with what it is, so the
    /// slot is `COUNTED` and the ability is told the number (CR 603.3d, 601.2c).
    pub(super) fn counted_slot(&self, announcement: &Announcement) -> Option<(String, String)> {
        let (Count::Exactly(count) | Count::UpTo(count)) = &announcement.targets.first()?.count
        else {
            return None;
        };
        if matches!(count, Expr::Constant(_) | Expr::AnnouncedX) {
            return None;
        }
        let code = self.number(count).ok()?.code;
        Some((
            format!("{count:?}"),
            format!("({} as i64)", self.fetch(code)),
        ))
    }
    /// Counters put on `to` by what is on `from`, which keeps its own. The kinds
    /// are read before any goes on; "choose a counter" is a choice of a kind.
    fn duplicate_counters(
        &mut self,
        id: usize,
        from: &Recipients,
        to: &Recipients,
        copies: CounterCopies,
        kind: Option<&crate::counter::CounterKind>,
        out: &mut String,
    ) -> Result<()> {
        use CounterCopies as C;
        if let (Recipients::CountersTheEventPut, Recipients::Objects(to)) = (from, to) {
            return self.duplicate_what_the_event_put(id, to, copies, out);
        }
        let (Recipients::Objects(from), Recipients::Objects(to)) = (from, to) else {
            let (Recipients::Players(from), Recipients::Players(to)) = (from, to) else {
                return Err(gap("operation DuplicateCounters"));
            };
            let from = self.fetch(self.players(from)?);
            let to = self.fetch(self.players(to)?);
            let call = format!("let from: Vec<PlayerId> = {from}; let to: Vec<PlayerId> = {to}; {M}::counters::duplicate_on_players(g, asker, &from, &to, {})?;", quoted(&format!("{copies:?}")));
            self.plan_code("counters a player has, by kind", &call, out);
            return Ok(());
        };
        let same = format!("{from:?}") == format!("{to:?}");
        let sources = self.fetch(self.objects(from)?);
        let recipients = self.fetch(self.objects(to)?);
        let only = match kind {
            Some(kind) => format!("kind == {}", counter_kind(kind)?),
            None => "true".to_string(),
        };
        writeln!(out, "let from_{id}: Vec<ObjectId> = {sources}; let to_{id}: Vec<ObjectId> = {recipients}; let kinds_{id}: Vec<CounterKind> = CounterKind::ALL.into_iter().filter(|&kind| {only} && from_{id}.iter().any(|&o| g.obj(o).counter_array()[kind as usize] > 0)).collect();").unwrap();
        let chosen = format!("kinds_{id}[{M}::choose::one_of(g, asker.controller, asker.source, kinds_{id}.iter().map(|&k| Label::Counter(k)).collect())? as usize]");
        let each = |lacking: bool| {
            let lacks = match lacking {
                true => ".filter(|&o| g.obj(o).counter_array()[kind as usize] == 0)",
                false => "",
            };
            format!("let puts: Vec<(ObjectId, CounterKind, i16)> = to_{id}.iter().copied(){lacks}.map(|o| (o, kind, 1i16)).collect(); g.add_counters_together(&puts);")
        };
        let code = match copies {
            C::All if same => format!("let puts: Vec<(ObjectId, CounterKind, i16)> = to_{id}.iter().flat_map(|&o| {{ let have = g.obj(o).counter_array(); kinds_{id}.iter().copied().filter(move |&kind| have[kind as usize] > 0).map(move |kind| (o, kind, have[kind as usize])).collect::<Vec<_>>() }}).collect(); g.add_counters_together(&puts);"),
            C::All => format!("let puts: Vec<(ObjectId, CounterKind, i16)> = to_{id}.iter().flat_map(|&o| kinds_{id}.iter().map(|&kind| (o, kind, from_{id}.iter().map(|&f| g.obj(f).counter_array()[kind as usize]).sum::<i16>())).collect::<Vec<_>>()).collect(); g.add_counters_together(&puts);"),
            C::OneOfEachKind => format!("let puts: Vec<(ObjectId, CounterKind, i16)> = to_{id}.iter().flat_map(|&o| kinds_{id}.iter().map(move |&kind| (o, kind, 1i16)).collect::<Vec<_>>()).collect(); g.add_counters_together(&puts);"),
            C::OneOfEachKindDistributed => format!("for kind in kinds_{id}.clone() {{ if let Some(object) = {M}::choose::card(g, asker.controller, asker.source, Purpose::ChooseAsEnters, to_{id}.clone())? {{ g.add_counters(object, kind, 1); }} }}"),
            C::One => format!("if !kinds_{id}.is_empty() {{ let kind = {chosen}; {} }}", each(false)),
            C::OneMissing => format!("if !kinds_{id}.is_empty() {{ let kind = {chosen}; {} }}", each(true)),
            C::UpToOne => format!("if !kinds_{id}.is_empty() {{ let kind = {chosen}; if {M}::effects::ask_may(g, asker.controller, asker.source, Purpose::Optional)? {{ {} }} }}", each(false)),
        };
        writeln!(out, "{code}").unwrap();
        Ok(())
    }
    /// "As this enters, choose another creature you control": the permanent keeps
    /// it for the abilities of its text (CR 607.2d). Only a choice of one object,
    /// and only one nothing after it in this replacement reads.
    pub(super) fn object_as_it_enters(
        &mut self,
        objects: &ObjectSet,
        count: &Count,
        rest: &[Instruction],
        result: &ResultId,
        out: &mut String,
    ) -> Result<bool> {
        let one = matches!(
            count,
            Count::Exactly(Expr::Constant(1))
                | Count::AsManyAsCan(Expr::Constant(1))
                | Count::UpTo(Expr::Constant(1))
        );
        let read = format!("ResultId({})", result.0);
        let reads = rest.iter().any(|i| format!("{i:?}").contains(&read));
        if !one || reads || self.event_at != EventAt::Entering {
            return Ok(false);
        }
        let pool = self.fetch(self.objects(objects)?);
        let count = self.fetch(self.select_count(count)?);
        writeln!(out, "let permanents = {pool}; let count = {count}; {M}::choose::object_as_enters(g, entering, permanents, {M}::select::Selection::of(count, &[]))?;").unwrap();
        Ok(true)
    }
    /// "You may tap any number of ...".
    pub(super) fn tap_any_number(
        &mut self,
        id: usize,
        player: &PlayerRef,
        objects: &ObjectSet,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        self.choose(player, objects, &Count::AnyNumber, result, "Tap", out)?;
        let tap = Operation::Tap {
            objects: ObjectSet::Selected(result.clone()),
            tapped: true,
        };
        self.operation(&tap, result, out)?;
        writeln!(out, "let paid_{id} = !{}.is_empty();", self.moved(result)?).unwrap();
        Ok(())
    }
    /// "Those that paid", where `may_pay` paid them: it does not say which.
    pub(super) fn objects_that_paid(&mut self, id: usize, result: &ResultId, out: &mut String) {
        let mark = format!("Resolution(ResultId({}))", result.0);
        if !self.after.iter().any(|i| format!("{i:?}").contains(&mark)) {
            return;
        }
        let paid = planned::planned_value(
            "objects that paid a cost at resolution",
            "Vec::<ObjectId>::new()",
            "g.objects_that_paid()",
        );
        writeln!(out, "let paid_objects_{id}: Vec<ObjectId> = {paid};").unwrap();
        self.moved.insert(result.0, format!("paid_objects_{id}"));
        self.before
            .insert(result.0, format!("paid_objects_{id}.clone()"));
    }
    /// What an instruction did, for the instructions after it that ask, when it
    /// compiled and left no word of it. If its code waits on Cantrip, so does the
    /// answer, and that code's hole says so.
    pub(super) fn report(
        &mut self,
        op: &Operation,
        result: &ResultId,
        waits: bool,
        out: &mut String,
    ) {
        let reported = self.selections.contains_key(&result.0)
            || self.moved.contains_key(&result.0)
            || self.creations.contains_key(&result.0)
            || self.performed.contains_key(&result.0)
            || self.values.contains_key(&result.0)
            || self.flips.contains_key(&result.0);
        if reported || !self.read_later(result) {
            return;
        }
        if waits {
            self.failed.insert(result.0);
            return;
        }
        if self.report_objects(op, result, out) {
            return;
        }
        let happened = match op {
            // Each of these is done whatever the state of the game.
            Operation::Scry { .. }
            | Operation::Surveil { .. }
            | Operation::Shuffle { .. }
            | Operation::AddMana { .. }
            | Operation::ChooseAtRandom { .. } => "true".to_string(),
            // Countered, the card is a new object where the spell went.
            Operation::Counter { objects, .. } => {
                let Ok(spells) = self.objects(objects) else {
                    return;
                };
                let id = self.fresh();
                writeln!(out, "let countered_{id}: Vec<ObjectId> = {}.into_iter().filter_map(|o: ObjectId| g.state().obj(o).became).collect();", self.fetch(spells)).unwrap();
                self.moved.insert(result.0, format!("countered_{id}"));
                return;
            }
            Operation::Keyword { object: None, .. } => "true".to_string(),
            Operation::Attach { host: object, .. }
            | Operation::Keyword {
                object: Some(object),
                ..
            } => match self.object(object) {
                Ok(code) => format!("({}).is_some()", self.fetch(code)),
                Err(_) => return,
            },
            _ => return,
        };
        let id = self.fresh();
        writeln!(out, "let happened_{id}: bool = {happened};").unwrap();
        self.performed.insert(result.0, format!("happened_{id}"));
    }

    /// What a slot's constraints ask, one at a time: a slot has one rule together.
    pub(super) fn slot_rule(
        &mut self,
        constraints: &[SelectionConstraint],
        slot: usize,
    ) -> Result<SlotRule> {
        let mut rule = SlotRule::default();
        for constraint in constraints {
            let feature = format!("target constraint {}", form(constraint));
            let together = match constraint {
                SelectionConstraint::DifferentFromTarget(earlier) => {
                    let chosen = self.chosen_in_slot(earlier, slot)?;
                    let test = format!("!{chosen}.contains(&t)");
                    rule.test = Some(match rule.test.take() {
                        Some(other) => format!("{other} && {test}"),
                        None => test,
                    });
                    None
                }
                SelectionConstraint::AtRandom => {
                    rule.planned.push((feature, ".at_random()".to_string()));
                    None
                }
                SelectionConstraint::OneForEach(filters) => {
                    let filters = filters
                        .iter()
                        .map(|f| {
                            let test = self.filter(f, &Candidate::of("id"))?;
                            Ok(format!(
                                "|q: &dyn Query, asker: &Asker, id: ObjectId| {test}"
                            ))
                        })
                        .collect::<Result<Vec<_>>>()?;
                    rule.planned.push((
                        feature,
                        format!(
                            ".together({M}::select::one_for_each(&[{}]))",
                            filters.join(", ")
                        ),
                    ));
                    None
                }
                other => match self.set_rule(other) {
                    Some(part) => Some(format!(".together({part})")),
                    None => {
                        let stated = self.stated_constraints(std::slice::from_ref(other))?;
                        rule.planned
                            .push((feature, format!(".with_rules({stated})")));
                        None
                    }
                },
            };
            if let Some(together) = together {
                if !rule.together.is_empty() {
                    return Err(gap("target slot with two rules together"));
                }
                rule.together = together;
            }
        }
        Ok(rule)
    }
    /// The targets an earlier slot chose, read while a later slot is chosen. A slot
    /// of no fixed size has to be the last before this one: what follows it in
    /// `chosen_targets` is this slot's own, which a target can't repeat anyway.
    pub(super) fn chosen_in_slot(&self, earlier: &Slot, choosing: usize) -> Result<String> {
        let referred = match self.slots.get(&earlier.0) {
            Some((i, _)) if *i < choosing => *i,
            _ => return Err(gap("target reference to no earlier slot")),
        };
        let size = |spec: &TargetSpec| match &spec.count {
            Count::Exactly(Expr::Constant(n)) => Some(*n as usize),
            _ => None,
        };
        let in_slot = || {
            planned::planned_value(
                "target reference past a slot of no fixed size",
                "Vec::<Target>::new()",
                &format!("q.chosen_targets_of({referred})"),
            )
        };
        let mut skip = 0;
        for (i, spec) in self.slots.values() {
            if *i < referred {
                match size(spec) {
                    Some(n) => skip += n,
                    None => return Ok(in_slot()),
                }
            }
        }
        let (_, spec) = &self.slots[&earlier.0];
        let take = match size(spec) {
            Some(n) => format!(".iter().skip({skip}).take({n})"),
            None if referred + 1 == choosing => format!(".iter().skip({skip})"),
            None => return Ok(in_slot()),
        };
        Ok(format!(
            "q.chosen_targets(){take}.copied().collect::<Vec<Target>>()"
        ))
    }
    /// A slot's test, with the rule's further test of each target.
    pub(super) fn slot_test(&self, spec: &TargetSpec, rule: &SlotRule) -> Result<String> {
        let any = spec.kind == TargetKind::DamageRecipient
            && spec.players.is_none()
            && matches!(spec.filter, ObjectFilter::Any);
        let test = if any {
            "(cantrip_mechanics::targets::ANY.filter)(q, asker, t)".to_string()
        } else {
            self.target_test(spec)?
        };
        Ok(match &rule.test {
            Some(more) => format!("({test}) && {more}"),
            None => test,
        })
    }
    /// "Target creature of an opponent's choice": someone else picks the slot.
    pub(super) fn chosen_by_another(
        &mut self,
        chooser: &PlayerRef,
        rule: &mut SlotRule,
    ) -> Result<()> {
        if self.every_slot_chosen && self.chosen_by_subject(chooser) {
            return Ok(());
        }
        if let PlayerRef::OneOf(players) = chooser {
            rule.planned.push((
                "targets chosen by another player".to_string(),
                format!(
                    ".chosen_by_one_of(|q: &dyn Query, asker: &Asker| {})",
                    self.players(players)?
                ),
            ));
            return Ok(());
        }
        let Ok(player) = self.certain_player(chooser) else {
            rule.planned.push((
                "targets chosen by another player".to_string(),
                format!(
                    ".chosen_by_one_of(|q: &dyn Query, asker: &Asker| {}.into_iter().collect::<Vec<PlayerId>>())",
                    self.player(chooser)?
                ),
            ));
            return Ok(());
        };
        rule.planned.push((
            "targets chosen by another player".to_string(),
            format!(".chosen_by(|q: &dyn Query, asker: &Asker| {player})"),
        ));
        Ok(())
    }
    /// A slot's count no plain slot has.
    pub(super) fn target_count_rest(&self, count: &Count) -> Result<String> {
        let read = |n: &Expr| -> Result<String> {
            let n = self.number(n)?.amount(u32::MAX.into(), "u32")?;
            Ok(format!(
                "TargetSpec::read_as_chosen(|q: &dyn Query, asker: &Asker| {n})"
            ))
        };
        Ok(match count {
            Count::AtLeast(Expr::Constant(n)) if *n >= 0 => {
                format!(".between({n}, cantrip_mechanics::ANY_NUMBER)")
            }
            Count::Exactly(n) => {
                let n = planned::planned_value("target cardinality Exactly", "1u32", &read(n)?);
                format!(".between({n}, {n})")
            }
            Count::AtLeast(n) => {
                let n = planned::planned_value("target cardinality AtLeast", "1u32", &read(n)?);
                format!(".between({n}, cantrip_mechanics::ANY_NUMBER)")
            }
            other => return Err(gap(format!("target cardinality {}", form(other)))),
        })
    }
    /// What `ModalDef` has no word for yet.
    pub(super) fn modes_to_come(&self, selection: &ModeSelection, modal: String) -> Result<String> {
        let mut planned: Vec<(&str, String)> = Vec::new();
        if selection.implied {
            planned.push((
                "modes implied by the target chosen",
                ".implied_by_target()".to_string(),
            ));
        }
        if let Some(ModeHistory::LastCombat(player)) = &selection.unchosen {
            let player = self.certain_player(player)?;
            planned.push((
                "modes limited by earlier choices",
                format!(".allowed({M}::history::mode_not_chosen_last_combat(|q: &dyn Query, asker: &Asker| {player}))"),
            ));
        }
        if selection.distinct_targets {
            planned.push((
                "modes with distinct targets",
                ".distinct_targets()".to_string(),
            ));
        }
        if let Some(chooser) = &selection.chooser {
            let players = self.players(chooser)?;
            planned.push((
                "modes chosen by another player",
                format!(".chosen_by(|q: &dyn Query, asker: &Asker| {players})"),
            ));
        }
        if selection.random {
            planned.push(("modes chosen at random", ".at_random()".to_string()));
        }
        if let Some(budget) = &selection.budget {
            let budget = self.number(budget)?.amount(u32::MAX.into(), "u32")?;
            let weights: Vec<String> = selection
                .modes
                .iter()
                .map(|m| m.weight.to_string())
                .collect();
            planned.push((
                "modes with a budget",
                format!(".budget({budget}, &[{}])", weights.join(", ")),
            ));
        }
        Ok(planned.into_iter().fold(modal, |modal, (feature, code)| {
            planned::planned_value(feature, &modal, &format!("{modal}{code}"))
        }))
    }
    /// The one player an instruction names, or the gap `feature`.
    fn one_actor(&self, actor: &PlayerSet, feature: &str) -> Result<String> {
        let PlayerSet::One(player) = actor else {
            return Err(gap(feature));
        };
        Ok(self.fetch(self.certain_player(player)?))
    }
    /// What `operation` has no arm for of its own.
    pub(super) fn resolving(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        match op {
            // "Those creatures": what the set holds now, whatever happens to it.
            Operation::NoteObjects { objects } => {
                writeln!(
                    out,
                    "let noted_{id}: Vec<ObjectId> = {};",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.selections.insert(result.0, format!("noted_{id}"));
            }
            Operation::DealDamageFromEach {
                sources,
                recipients,
                amount,
            } => self.damage_from_each(id, sources, recipients, amount, result, out)?,
            Operation::RevealFromHand {
                actor,
                cards,
                count,
                at_random,
            } => {
                self.reveal_from_hand(id, actor, cards, count.as_ref(), *at_random, result, out)?
            }
            Operation::ChooseAtRandom { kind, options } => {
                self.choose_at_random(id, *kind, options, out)?
            }
            Operation::Exchange {
                kind,
                first,
                second,
                until: None,
            } if matches!(
                (kind, &**first, &**second),
                (
                    ExchangeKind::LifeTotals,
                    Operand::Player(_),
                    Operand::Player(_)
                ) | (
                    ExchangeKind::Control,
                    Operand::Object(_),
                    Operand::Object(_)
                )
            ) =>
            {
                self.exchange(id, kind, first, second, None, out)?
            }
            Operation::Wish {
                actor,
                filter,
                reveal,
            } => {
                let player = self.one_actor(actor, "operation Wish")?;
                let test = self.filter(filter, &Candidate::of("id"))?;
                let wished = self.fetch(format!("q.outside_the_game(player_{id}).into_iter().filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
                let bring = if *reveal {
                    "bring_into_hand_revealed"
                } else {
                    "bring_into_hand"
                };
                writeln!(out, "let player_{id} = {player}; let wished_{id} = {wished}; let chosen_{id} = {M}::choose::cards(g, player_{id}, asker.source, Purpose::PutIntoHand, wished_{id}, 1, true)?; let brought_{id} = g.{bring}(&chosen_{id});").unwrap();
                self.moved.insert(result.0, format!("brought_{id}"));
                self.left_behind(result, format!("chosen_{id}.clone()"));
            }
            Operation::SeparatePiles {
                separator,
                chooser,
                objects,
            } => {
                let cards = self.fetch(self.objects(objects)?);
                let separator = self.fetch(self.certain_player(separator)?);
                self.name_even_the_one();
                let chooser = self.fetch(self.certain_player(chooser)?);
                writeln!(out, "let cards_{id} = {cards}; let separator_{id} = {separator}; let chooser_{id} = {chooser}; let (first_{id}, second_{id}) = {M}::choose::separate(g, Some(separator_{id}), asker.source, cards_{id})?; let (pile_{id}, other_{id}) = {M}::choose::pile(g, chooser_{id}, asker.source, first_{id}, second_{id})?;").unwrap();
                self.selections.insert(result.0, format!("pile_{id}"));
                self.unchosen.insert(result.0, format!("other_{id}"));
            }
            Operation::DrawUpTo { players, amount } => {
                let most = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let most_{id} = {}; let mut done_{id}: i64 = 0; for player in {} {{ let n = {M}::choose::number(g, player, asker.source, 0, most_{id}, \"Draw\")?; let before = g.history().drawn(player) as i64; g.draw(player, n); done_{id} += g.history().drawn(player) as i64 - before; }}", self.fetch(most), self.fetch(self.players(players)?)).unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
            }
            Operation::SacrificeAllBut {
                players,
                objects,
                keep,
            } => {
                let keep = self.number(keep)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let objects_{id} = {}; let keep_{id} = {}; let mut picks_{id}: Vec<(PlayerId, ObjectId)> = Vec::new(); for player in {} {{ let own: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.controller(o) == player).collect(); if own.len() as u32 <= keep_{id} {{ continue; }} let kept = if keep_{id} == 0 {{ Vec::new() }} else {{ {M}::choose::select(g, player, asker.source, Purpose::Keep, own.clone(), {M}::select::Selection::of({M}::select::Count::AsManyAsCan(keep_{id}), &[]))? }}; for o in own {{ if !kept.contains(&o) && g.can_sacrifice(player, o) {{ picks_{id}.push((player, o)); }} }} }} let sacrificed_{id} = g.sacrifice_each(&picks_{id});", self.fetch(self.objects(objects)?), self.fetch(keep), self.fetch(self.players(players)?)).unwrap();
                self.moved.insert(result.0, format!("sacrificed_{id}"));
                self.left_behind(result, effects::gone(&format!("objects_{id}")));
            }
            Operation::DiscardAllBut { players, keep } => {
                let keep = self.number(keep)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let keep_{id} = {}; let mut cards_{id}: Vec<ObjectId> = Vec::new(); for player in {} {{ let hand = g.state().zone(Zone::hand(player)).to_vec(); let extra = (hand.len() as u32).saturating_sub(keep_{id}); cards_{id}.extend({M}::choose_from_hand(g, player, asker.source, extra)?); }} let discarded_{id} = g.discard_together(&cards_{id});", self.fetch(keep), self.fetch(self.players(players)?)).unwrap();
                self.moved.insert(result.0, format!("discarded_{id}"));
                self.left_behind(result, format!("discarded_{id}.clone()"));
            }
            Operation::DrawThenDiscard {
                players,
                draw,
                discard,
            } => {
                let draw = self.number(draw)?.amount(u32::MAX.into(), "u32")?;
                let discard = self.number(discard)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let draw_{id} = {}; let discard_{id} = {}; for player in {} {{ g.draw(player, draw_{id}); let cards = {M}::choose_from_hand(g, player, asker.source, discard_{id})?; g.discard_together(&cards); }}", self.fetch(draw), self.fetch(discard), self.fetch(self.players(players)?)).unwrap();
            }
            Operation::PutCountersEach {
                amount,
                kind,
                objects,
                up_to: false,
            } => {
                let kind = counter_kind(kind)?;
                let saved = self.affected_object.replace("object");
                let amount = self
                    .number(amount)
                    .and_then(|n| n.amount(i16::MAX.into(), "i16"));
                self.affected_object = saved;
                let amount = self.fetch(amount?);
                writeln!(out, "let objects_{id} = {}; let mut counters_{id}: Vec<(ObjectId, CounterKind, i16)> = Vec::new(); for &object in &objects_{id} {{ let amount = {amount}; counters_{id}.push((object, {kind}, amount)); }} g.add_counters_together(&counters_{id});", self.acted_on(objects)?).unwrap();
                self.selections.insert(result.0, format!("objects_{id}"));
            }
            Operation::PutCountersCapped {
                amount,
                kind,
                object,
                at_most,
            } => {
                let kind = counter_kind(kind)?;
                let amount = self.number(amount)?.amount(i16::MAX.into(), "i16")?;
                let cap = self.number(at_most)?.amount(i16::MAX.into(), "i16")?;
                writeln!(out, "let object_{id} = {}; let amount_{id} = {}; let cap_{id} = {}; if let Some(object) = object_{id} {{ let room = cap_{id}.saturating_sub(g.state().obj(object).counters({kind})).max(0); g.add_counters(object, {kind}, amount_{id}.min(room)); }}", self.fetch(self.object(object)?), self.fetch(amount), self.fetch(cap)).unwrap();
            }
            Operation::Repeat { times, body } => {
                let runs = self.number(times)?.amount(u32::MAX.into(), "u32")?;
                writeln!(
                    out,
                    "let runs_{id} = {}; for _ in 0..runs_{id} {{",
                    self.fetch(runs)
                )
                .unwrap();
                let scope = self.results();
                let done = self.operation(body, result, out);
                self.restore_results(scope);
                done?;
                writeln!(out, "}}").unwrap();
            }
            Operation::AttachChosen {
                actor,
                attachments,
                count,
                hosts: ObjectSet::One(host),
            } => {
                let player = self.one_actor(actor, "operation AttachChosen")?;
                let count = self.offered_count(count);
                let count = self.fetch(self.select_count(&count)?);
                writeln!(out, "let player_{id} = {player}; let eligible_{id} = {}; let host_{id} = {}; let eligible_{id} = if host_{id}.is_some() {{ eligible_{id} }} else {{ Vec::new() }}; let count_{id} = {count}; let chosen_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::Attach, eligible_{id}, {M}::select::Selection::of(count_{id}, &[]))?; if let Some(host) = host_{id} {{ cantrip_api::instruction(g, |g| {{ for &attachment in &chosen_{id} {{ g.attach(attachment, host); }} }}); }}", self.acted_on(attachments)?, self.fetch(self.object(host)?)).unwrap();
                self.selections.insert(result.0, format!("chosen_{id}"));
                self.open_guard(&format!("chosen_{id}"), out);
            }
            Operation::FightEach { fighters, others } => {
                writeln!(out, "let fighters_{id} = {}; let others_{id} = {}; for &first in &fighters_{id} {{ for &second in &others_{id} {{ g.fight(first, second); }} }}", self.fetch(self.objects(fighters)?), self.fetch(self.objects(others)?)).unwrap();
            }
            Operation::FlipCoins { actor, count } => {
                let player = self.one_actor(actor, "operation FlipCoins")?;
                let count = self.number(count)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let player_{id} = {player}; let count_{id} = {}; let mut done_{id}: i64 = 0; for _ in 0..count_{id} {{ if g.flip_coin(player_{id}) {{ done_{id} += 1; }} }}", self.fetch(count)).unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
            }
            Operation::EachObject { objects, action } => {
                writeln!(
                    out,
                    "for each_object in {} {{",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                let saved = self.affected_object.replace("each_object");
                let scope = self.results();
                let done = self.operation(action, result, out);
                self.restore_results(scope);
                self.affected_object = saved;
                done?;
                writeln!(out, "}}").unwrap();
            }
            Operation::ChooseValueWith { actor, kind, terms } => match terms {
                ChoiceTerms {
                    options,
                    except,
                    count: None,
                    secret: false,
                    unchosen: false,
                    random: false,
                    ordered: false,
                    up_to: false,
                } if except.is_empty() => {
                    let mut code = String::new();
                    match self.choose_value(id, actor, *kind, 1, options, &mut code) {
                        Ok(()) => out.push_str(&code),
                        Err(_) => self.choose_value_terms(id, actor, *kind, terms, out)?,
                    }
                }
                terms => {
                    self.choose_value_terms(id, actor, *kind, terms, out)?;
                    self.chosen_number_performed(*kind, result);
                }
            },
            Operation::CopyCards {
                actor,
                objects,
                times,
            } => {
                let player = self.one_actor(actor, "operation CopyCards")?;
                let times = self.number(times)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let player_{id} = {player}; let originals_{id} = {}; let times_{id} = {}; let mut copies_{id}: Vec<ObjectId> = Vec::new(); for _ in 0..times_{id} {{ for &card in &originals_{id} {{ copies_{id}.push(g.create_copy_in_exile(card, player_{id})); }} }}", self.fetch(self.objects(objects)?), self.fetch(times)).unwrap();
                self.selections.insert(result.0, format!("copies_{id}"));
            }
            Operation::PayAnyAmount { actor, resource } => {
                self.pay_any_amount(id, actor, resource, result, out)?
            }
            other => return self.resolving_rest(id, other, result, out),
        }
        Ok(())
    }
    /// "Pay any amount of ...": the amount is the result, for each who pays.
    fn pay_any_amount(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        resource: &PaidResource,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let pay = match resource {
            // 107.14: paying energy is giving up that many counters.
            PaidResource::Energy => format!("let mut paid = 0i64; while g.state().player(player).energy() > 0 && {M}::effects::ask_may(g, player, asker.source, Purpose::Optional)? {{ g.change_energy(player, -1); paid += 1; }} paid"),
            PaidResource::Mana => {
                self.helpers.insert("pay_question");
                "i64::from(g.may_pay_x(player, asker.source, asker.source, cost(\"{X}\"), &PAY_QUESTION)?.unwrap_or(0))".to_string()
            }
            // `life` holds a byte: no one payment passes 255 life.
            PaidResource::Life => {
                self.helpers.insert("pay_question");
                format!("let most = g.state().player(player).life.clamp(0, 255) as u32; let paid = {M}::choose::number(g, player, asker.source, 0, most, \"Pay\")?; if paid > 0 && g.may_pay(player, asker.source, asker.source, life(paid as u8), &PAY_QUESTION)? {{ i64::from(paid) }} else {{ 0 }}")
            }
        };
        writeln!(
            out,
            "let mut done_{id}: i64 = 0; for player in {} {{ done_{id} += {{ {pay} }}; }}",
            self.fetch(self.players(actor)?)
        )
        .unwrap();
        self.performed.insert(result.0, format!("done_{id}"));
        self.values.insert(result.0, format!("done_{id}"));
        Ok(())
    }
    /// The requirements: code that waits on Cantrip.
    fn resolving_rest(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let feature = format!("operation {}", form(op));
        match op {
            Operation::Clash {
                actor,
                opponents,
                won,
                lost,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let opponents = self.fetch(self.players(opponents)?);
                writeln!(
                    out,
                    "let player_{id} = {player}; let opponents_{id}: Vec<PlayerId> = {opponents};"
                )
                .unwrap();
                let clash = planned::planned_value(
                    &feature,
                    "false",
                    &format!("{M}::clash::clash(g, player_{id}, asker.source, opponents_{id})?"),
                );
                writeln!(out, "let won_{id}: bool = {clash}; if won_{id} {{").unwrap();
                for program in won {
                    self.block(program, out, true)?;
                }
                writeln!(out, "}} else {{").unwrap();
                for program in lost {
                    self.block(program, out, true)?;
                }
                writeln!(out, "}}").unwrap();
            }
            Operation::Vote {
                actor,
                ballot,
                secret,
            } => {
                let voters = self.fetch(self.players(actor)?);
                let ballot = match ballot {
                    Ballot::Words(words) => {
                        let words: Vec<String> = words.iter().map(|w| format!("{w:?}")).collect();
                        format!("{M}::vote::Ballot::Words(&[{}])", words.join(", "))
                    }
                    Ballot::Players(players) => format!(
                        "{M}::vote::Ballot::Players({})",
                        self.fetch(self.players(players)?)
                    ),
                    Ballot::Objects(objects) => format!(
                        "{M}::vote::Ballot::Objects({})",
                        self.fetch(self.objects(objects)?)
                    ),
                    Ballot::UpToOne(objects) => format!(
                        "{M}::vote::Ballot::UpToOne({})",
                        self.fetch(self.objects(objects)?)
                    ),
                    Ballot::GraveyardCards(filter) => {
                        let test = self.filter(filter, &Candidate::of("id"))?;
                        format!("{M}::vote::Ballot::GraveyardCards(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})")
                    }
                };
                writeln!(out, "let voters_{id}: Vec<PlayerId> = {voters};").unwrap();
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::vote::ballot(g, asker.source, &voters_{id}, {ballot}, {secret})?"
                    ),
                    out,
                );
            }
            Operation::PutSticker {
                actor,
                objects,
                name_only,
            } => {
                let player = self.one_actor(actor, &feature)?;
                writeln!(
                    out,
                    "let player_{id} = {player}; let objects_{id} = {};",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                let call = if *name_only {
                    "name_sticker"
                } else {
                    "sticker"
                };
                self.plan_code(
                    &feature,
                    &format!("{M}::stickers::{call}(g, player_{id}, asker.source, &objects_{id})?"),
                    out,
                );
            }
            Operation::Intensify { cards, owner } => {
                let test = self.filter(cards, &Candidate::of("id"))?;
                let owner = self.fetch(self.certain_player(owner)?);
                self.plan_code(&feature, &format!("{M}::intensify::intensify(g, {owner}, |q: &dyn Query, asker: &Asker, id: ObjectId| {test})"), out);
            }
            Operation::AbandonScheme { actor } => {
                let player = self.one_actor(actor, &feature)?;
                self.plan_code(
                    &feature,
                    &format!("{M}::scheme::abandon(g, {player}, asker.source)"),
                    out,
                );
            }
            Operation::RevealChoices { kind } => {
                self.plan_code(
                    &feature,
                    &format!("{M}::secret::reveal_choices(g, asker.source, ChoiceKind::{kind:?})"),
                    out,
                );
            }
            Operation::ChangeSpell { subject, changes } => {
                let changes = changes
                    .iter()
                    .map(|change| self.spell_change(change))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                match subject {
                    SpellSubject::One(spell) => {
                        let spell = self.fetch(self.object(spell)?);
                        self.plan_code(&feature, &format!("if let Some(spell) = {spell} {{ {M}::spell_changes::change(g, asker, spell, &[{changes}]); }}"), out);
                    }
                    SpellSubject::Next {
                        spells,
                        player,
                        this_turn,
                        from_hand,
                    } => {
                        let test = self.filter(spells, &Candidate::of("id"))?;
                        let player = self.fetch(self.certain_player(player)?);
                        self.plan_code(&feature, &format!("{M}::spell_changes::next_spell(g, asker, {player}, |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, NextSpell {{ this_turn: {this_turn}, from_hand: {from_hand} }}, &[{changes}])"), out);
                    }
                }
            }
            Operation::TowardPlayer {
                actor,
                action,
                player,
            } => {
                let actor = self.one_actor(actor, &feature)?;
                let player = self.fetch(self.certain_player(player)?);
                let call = match action {
                    PlayerPairAction::Heist => {
                        format!("{M}::heist::heist(g, {actor}, {player}, asker.source)?")
                    }
                    PlayerPairAction::ControlNextTurn => {
                        format!("{M}::control_turn::next_turn(g, asker.source, {actor}, {player})")
                    }
                    PlayerPairAction::ControlNextCombat => format!(
                        "{M}::control_turn::next_combat(g, asker.source, {actor}, {player})"
                    ),
                    PlayerPairAction::ExchangeLife => {
                        format!("{M}::exchange::life_totals(g, {actor}, {player})")
                    }
                };
                self.plan_code(&feature, &call, out);
            }
            Operation::Exchange {
                kind,
                first,
                second,
                until,
            } => {
                let read = |this: &Self, side: &Operand| -> Result<String> {
                    Ok(match side {
                        Operand::Object(object) => this.fetch(this.object(object)?),
                        Operand::Objects(objects) => this.fetch(this.objects(objects)?),
                        Operand::Player(player) => this.fetch(this.player(player)?),
                        Operand::Players(players) => this.fetch(this.players(players)?),
                        _ => return Err(gap(format!("operation {}", form(op)))),
                    })
                };
                let (first, second) = (read(self, first)?, read(self, second)?);
                let until = match until {
                    Some(until) => format!("Some({})", self.duration(until)?),
                    None => "None".to_string(),
                };
                self.plan_code(&feature, &format!("{M}::exchange::exchange(g, asker.source, Exchange::{kind:?}, {first}, {second}, {until})"), out);
            }
            Operation::CopyOnStack {
                actor,
                object,
                times,
                targets,
                effects: _,
                card: None,
            } => {
                let copier = self.one_actor(actor, &feature)?;
                let object = self.fetch(self.object(object)?);
                let times = self.fetch(self.number(times)?.amount(u32::MAX.into(), "u32")?);
                let targets = self.new_targets(targets)?;
                self.plan_code(&feature, &format!("if let Some(spell) = {object} {{ {M}::copy::on_stack(g, {copier}, spell, {times}, {targets})?; }}"), out);
            }
            Operation::TakeSpell {
                actor,
                object,
                targets,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let object = self.fetch(self.object(object)?);
                let targets = self.new_targets(targets)?;
                self.plan_code(&feature, &format!("if let Some(spell) = {object} {{ g.change_controller(spell, {player}); {M}::copy::retarget(g, {player}, spell, {targets})?; }}"), out);
            }
            Operation::Retarget {
                actor,
                object,
                targets,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let object = self.fetch(self.object(object)?);
                let targets = self.new_targets(targets)?;
                self.plan_code(&feature, &format!("if let Some(spell) = {object} {{ {M}::copy::retarget(g, {player}, spell, {targets})?; }}"), out);
            }
            Operation::CopyEach {
                actor,
                objects,
                targets,
            }
            | Operation::RetargetEach {
                actor,
                objects,
                targets,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let test = self.filter(objects, &Candidate::of("id"))?;
                let targets = self.new_targets(targets)?;
                let call = if matches!(op, Operation::CopyEach { .. }) {
                    "on_stack_each"
                } else {
                    "retarget_each"
                };
                self.plan_code(&feature, &format!("{M}::copy::{call}(g, {player}, |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, {targets})?"), out);
            }
            Operation::Note(kind) => {
                let note = match kind {
                    NoteKind::Number(n) => {
                        format!("Note::Number({})", self.fetch(self.number(n)?.code))
                    }
                    NoteKind::Player(player) => {
                        format!("Note::Player({})", self.fetch(self.player(player)?))
                    }
                    NoteKind::LifeTotal => "Note::LifeTotal".to_string(),
                    NoteKind::CountersOn(object) => {
                        format!("Note::CountersOn({})", self.fetch(self.object(object)?))
                    }
                    NoteKind::ManaSpent { amount } => {
                        format!("Note::ManaSpent {{ amount: {amount} }}")
                    }
                    NoteKind::Numbered(objects) => {
                        format!("Note::Numbered({})", self.fetch(self.objects(objects)?))
                    }
                    NoteKind::MostCommonCreatureType(players) => format!(
                        "Note::MostCommonCreatureType({})",
                        self.fetch(self.players(players)?)
                    ),
                };
                self.plan_code(
                    &feature,
                    &format!("{M}::notes::note(g, asker, {note})"),
                    out,
                );
            }
            Operation::DealDamage {
                source,
                recipients,
                amount,
                preventable: false,
            } => {
                let source = self.fetch(self.object(source)?);
                let targets = self.fetch(self.recipients(recipients)?);
                let amount = self.fetch(self.number(amount)?.amount(u16::MAX.into(), "u16")?);
                self.plan_code(&feature, &format!("if let Some(source) = {source} {{ {M}::damage::unpreventable(g, source, &{targets}, {amount}); }}"), out);
            }
            Operation::DealDamageWith {
                source,
                recipients,
                amount,
                rider,
            } => {
                let source = self.fetch(self.object(source)?);
                let targets = self.fetch(self.recipients(recipients)?);
                let amount = self.fetch(self.number(amount)?.amount(u16::MAX.into(), "u16")?);
                match rider {
                    DamageRider::DividedEvenly(rounding) => {
                        let share = match rounding {
                            Rounding::Down => "amount_{id} / shares",
                            Rounding::Up => "amount_{id}.div_ceil(shares)",
                        }
                        .replace("{id}", &id.to_string());
                        writeln!(out, "let source_{id} = {source}; let targets_{id} = {targets}; let amount_{id}: u16 = {amount}; if let Some(source) = source_{id} {{ let shares = (targets_{id}.len() as u16).max(1); let share = {share}; let batch: Vec<(ObjectId, Target, u16)> = targets_{id}.iter().map(|&t| (source, t, share)).collect(); g.damage_together(&batch); }}").unwrap();
                    }
                    // What isn't lethal (120.10) goes to the player at the same
                    // time: the creature is never dealt the excess.
                    DamageRider::ExcessTo(player) => {
                        let player = self.fetch(self.certain_player(player)?);
                        writeln!(out, "let source_{id} = {source}; let targets_{id} = {targets}; let amount_{id}: u16 = {amount}; let player_{id} = {player}; if let (Some(source), Some(&target)) = (source_{id}, targets_{id}.first()) {{ let lethal_{id} = {{ let q: &dyn Query = g; target.object().map_or(amount_{id}, |o: ObjectId| {{ let chars = q.chars(o); let left = u16::try_from(chars.toughness.unwrap_or(0).max(0)).unwrap_or(0).saturating_sub(q.state().obj(o).damage); match q.chars(source).has(Keyword::Deathtouch) {{ true => left.min(1), false => left }} }}) }}; let dealt_{id} = amount_{id}.min(lethal_{id}); g.damage_together(&[(source, target, dealt_{id}), (source, Target::Player(player_{id}), amount_{id} - dealt_{id})]); }}").unwrap();
                    }
                    DamageRider::Prevented(prevented) => {
                        let prevented =
                            self.fetch(self.number(prevented)?.amount(u16::MAX.into(), "u16")?);
                        self.plan_code(&feature, &format!("if let Some(source) = {source} {{ {M}::damage::partly_prevented(g, source, &{targets}, {amount}, {prevented}); }}"), out);
                    }
                    DamageRider::DividedBy(player) => {
                        let player = self.fetch(self.certain_player(player)?);
                        self.plan_code(&feature, &format!("if let Some(source) = {source} {{ {M}::damage::divided_by(g, source, &{targets}, {amount}, {player})?; }}"), out);
                    }
                }
            }
            // "Gets +1/-1 or -1/+1": its words, as the card prints them, and a pair
            // for what follows to read.
            Operation::ChoosePowerToughness { player, options } if self.stack_item => {
                let player = self.fetch(self.certain_player(player)?);
                let labels: Vec<String> =
                    options.iter().map(|(p, t)| format!("{p:+}/{t:+}")).collect();
                let pairs: Vec<String> =
                    options.iter().map(|(p, t)| format!("({p}i64, {t}i64)")).collect();
                let base = self.choice_labels.len();
                let end = base + labels.len();
                self.choice_labels.extend(labels);
                let words: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", words.join(", ")));
                writeln!(out, "let player_{id} = {player}; let picked_{id} = {M}::choose::option(g, player_{id}, asker.source, item.ability.card, {base}..{end})? - {base}; let chosen_pt_{}: (i64, i64) = [{}][picked_{id} as usize];", result.0, pairs.join(", ")).unwrap();
            }
            Operation::ChoosePowerToughness { player, options } => {
                let player = self.fetch(self.certain_player(player)?);
                let options: Vec<String> =
                    options.iter().map(|(p, t)| format!("({p}, {t})")).collect();
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::choose::power_toughness(g, {player}, asker.source, &[{}])?",
                        options.join(", ")
                    ),
                    out,
                );
            }
            Operation::Guess {
                actor,
                object,
                filter,
                player,
            } => {
                let actor = self.one_actor(actor, &feature)?;
                let object = match object {
                    Some(object) => self.fetch(self.object(object)?),
                    None => "None".to_string(),
                };
                let test = match filter {
                    Some(filter) => {
                        let test = self.filter(filter, &Candidate::of("id"))?;
                        format!("Some(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})")
                    }
                    None => "None".to_string(),
                };
                let player = match player {
                    Some(player) => self.fetch(self.player(player)?),
                    None => "None".to_string(),
                };
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::guess::guess(g, {actor}, asker.source, {object}, {test}, {player})?"
                    ),
                    out,
                );
            }
            Operation::Bid { players, start } => {
                let players = self.fetch(self.players(players)?);
                let start = match start {
                    Some(start) => format!("Some({})", self.fetch(self.number(start)?.code)),
                    None => "None".to_string(),
                };
                self.plan_code(
                    &feature,
                    &format!("{M}::bid::bid(g, asker.source, {players}, {start})?"),
                    out,
                );
            }
            Operation::Seek {
                actor,
                cards,
                count,
                top,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let tests = cards
                    .iter()
                    .map(|filter| {
                        let test = self.filter(filter, &Candidate::of("id"))?;
                        Ok(format!(
                            "|q: &dyn Query, asker: &Asker, id: ObjectId| {test}"
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                let count = self.fetch(self.number(count)?.amount(u32::MAX.into(), "u32")?);
                let top = match top {
                    Some(top) => format!(
                        "Some({})",
                        self.fetch(self.number(top)?.amount(u32::MAX.into(), "u32")?)
                    ),
                    None => "None".to_string(),
                };
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::seek::seek(g, {player}, asker.source, &[{tests}], {count}, {top})?"
                    ),
                    out,
                );
            }
            Operation::PickFromHand {
                actor,
                player,
                filter,
                count,
                then,
            } => {
                let actor = self.one_actor(actor, &feature)?;
                let player = self.fetch(self.certain_player(player)?);
                let test = self.filter(filter, &Candidate::of("id"))?;
                let count = self.fetch(self.select_count(count)?);
                self.plan_code(&feature, &format!("{M}::hand::pick_from_hand(g, {actor}, {player}, asker.source, |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, {count}, HandPick::{then:?})?"), out);
            }
            Operation::RedistributeLife(players) => {
                let players = self.fetch(self.players(players)?);
                self.plan_code(
                    &feature,
                    &format!("{M}::exchange::redistribute_life(g, asker.source, {players})?"),
                    out,
                );
            }
            Operation::ReduceSpeed {
                player,
                amount,
                floor,
            } => {
                let player = self.fetch(self.certain_player(player)?);
                let amount = self.fetch(self.number(amount)?.amount(255, "u8")?);
                let floor = self.fetch(self.number(floor)?.amount(255, "u8")?);
                self.plan_code(
                    &feature,
                    &format!("g.reduce_speed({player}, {amount}, {floor})"),
                    out,
                );
            }
            Operation::LimitX { comparison, value } => {
                let value = self.fetch(self.number(value)?.code);
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::announce::limit_x(g, item, Comparison::{comparison:?}, {value})"
                    ),
                    out,
                );
            }
            Operation::RestartGame { keep, then } => {
                let keep = self.fetch(self.objects(keep)?);
                let then = self.planned_operation(then, result)?;
                self.plan_code(
                    &feature,
                    &format!("g.restart_game(&{keep}, |g| {{ {then} Ok(()) }})?"),
                    out,
                );
            }
            Operation::ReturnAfter { objects, duration } => {
                let objects = self.fetch(self.objects(objects)?);
                let until = self.duration(duration)?;
                writeln!(out, "let returning_{id}: Vec<ObjectId> = {objects};").unwrap();
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::exile_until::return_when(g, asker.source, &returning_{id}, {until})"
                    ),
                    out,
                );
                self.moved.insert(result.0, format!("returning_{id}"));
            }
            Operation::TurnFaceDownAs { objects, .. } => {
                let objects = self.fetch(self.objects(objects)?);
                self.plan_code(
                    &feature,
                    &format!("{M}::manifest_dread::turn_face_down(g, asker.source, &{objects})"),
                    out,
                );
            }
            // "Reveals their hand and discards a card": no two things at once, the
            // one word "and" of an instruction done in order.
            Operation::Together(operations)
                if matches!(
                    operations.as_slice(),
                    [
                        Operation::Reveal { .. } | Operation::RevealFromHand { .. },
                        Operation::DiscardObjects { .. }
                    ]
                ) =>
            {
                for operation in operations {
                    self.operation(operation, result, out)?;
                }
            }
            Operation::Together(operations) => {
                self.plan_code(&feature, "g.begin_simultaneous()", out);
                for operation in operations {
                    self.operation(operation, result, out)?;
                }
                self.plan_code(&feature, "g.end_simultaneous()", out);
            }
            Operation::GameEffect { until, effect } => {
                let until = self.duration(until)?;
                let rule = self.operand_code(effect);
                self.plan_code(
                    &feature,
                    &format!("{M}::game_rules::for_a_time(g, asker, {until}, {rule})"),
                    out,
                );
            }
            Operation::ChangeCombat { change } => {
                let change = match change {
                    CombatChange::Reselect {
                        objects,
                        players_only,
                        optional,
                    } => format!("CombatChange::Reselect {{ objects: {}, players_only: {players_only}, optional: {optional} }}", self.fetch(self.objects(objects)?)),
                    CombatChange::AssignAttack { objects, to } => {
                        let to = self.operand_code(to);
                        format!("CombatChange::AssignAttack {{ objects: {}, to: {to} }}", self.fetch(self.objects(objects)?))
                    }
                    CombatChange::Block { blocker, attackers } => format!("CombatChange::Block {{ blocker: {}, attackers: {} }}", self.fetch(self.object(blocker)?), self.fetch(self.objects(attackers)?)),
                    CombatChange::SwapBlocks {
                        first,
                        second,
                        attacking,
                    } => format!("CombatChange::SwapBlocks {{ first: {}, second: {}, attacking: {attacking} }}", self.fetch(self.object(first)?), self.fetch(self.object(second)?)),
                    CombatChange::OnlyAttackers { objects, during } => format!("CombatChange::OnlyAttackers {{ objects: {}, added_phase: {} }}", self.fetch(self.objects(objects)?), during.is_some()),
                    CombatChange::OnlyBlockers(objects) => format!("CombatChange::OnlyBlockers({})", self.fetch(self.objects(objects)?)),
                };
                self.plan_code(
                    &feature,
                    &format!("{M}::combat::change(g, asker, {change})?"),
                    out,
                );
            }
            Operation::DuplicateCounters {
                from,
                to,
                copies,
                kind,
            } => self.duplicate_counters(id, from, to, *copies, kind.as_ref(), out)?,
            Operation::ChooseAction {
                actor,
                options,
                villainous,
            } => {
                let programs = options
                    .iter()
                    .map(|option| action_program(option).ok_or_else(|| gap(feature.clone())))
                    .collect::<Result<Vec<Program>>>()?;
                let PlayerSet::One(decider) = actor else {
                    return Err(gap(feature));
                };
                if *villainous {
                    let decider = self.fetch(self.certain_player(decider)?);
                    let labels: Vec<String> = programs
                        .iter()
                        .map(|program| quoted(&ability_text(program)))
                        .collect();
                    let code = format!(
                        "{M}::choose::villainous(g, {decider}, asker.source, &[{}])?",
                        labels.join(", ")
                    );
                    self.plan_code("villainous choice", &code, out);
                }
                self.choose_one(decider, &programs, out)?;
            }
            // "You may have target player mill ...": you are asked, and they do it.
            Operation::PlayersDo {
                players: PlayerSet::One(player),
                any_one: false,
                optional: true,
                once_each_turn: false,
                body,
                otherwise,
            } if otherwise.is_empty() && had_to_do(player, body).is_some() => {
                let then = had_to_do(player, body).expect("just matched");
                let mut offered = then.clone();
                offered.instructions = vec![Instruction::Optional {
                    decider: PlayerRef::AbilityController,
                    body: then,
                }];
                self.block(&offered, out, true)?;
            }
            Operation::PlayersDo {
                players,
                any_one,
                optional,
                once_each_turn,
                body,
                otherwise,
            } => {
                let players = self.fetch(self.players(players)?);
                let body: Vec<String> = body.iter().map(|o| self.operand_code(o)).collect();
                let otherwise: Vec<String> =
                    otherwise.iter().map(|o| self.operand_code(o)).collect();
                self.plan_code(&feature, &format!("{M}::players::do_this(g, asker, {players}, PlayersDo {{ any_one: {any_one}, optional: {optional}, once_each_turn: {once_each_turn} }}, &[{}], &[{}])?", body.join(", "), otherwise.join(", ")), out);
            }
            Operation::PlayWith {
                actor,
                action,
                cards,
                count,
                terms,
                group,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let cards = self.fetch(self.objects(cards)?);
                let count = self.fetch(self.select_count(count)?);
                let group = group.as_ref().map(|g| self.operand_code(g));
                let terms = serde_json::to_string(terms).unwrap_or_default();
                self.plan_code(&feature, &format!("{M}::play::with_terms(g, {player}, asker.source, CardPlay::{}, {cards}, {count}, {group:?}, {terms:?})?", form(action)), out);
            }
            Operation::WithReflexive { action, when, body } => {
                let done = self.fresh();
                let scope = ResultId(u32::MAX - done as u32);
                self.operation(action, &scope, out)?;
                let when = match when {
                    ReflexiveWhen::YouDo => "ReflexiveWhen::YouDo".to_string(),
                    ReflexiveWhen::ExcessDamage(_) => "ReflexiveWhen::ExcessDamage".to_string(),
                    ReflexiveWhen::Revealed(filter) => {
                        let test = self.filter(filter, &Candidate::of("id"))?;
                        format!("ReflexiveWhen::Revealed(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})")
                    }
                    ReflexiveWhen::Regenerates => "ReflexiveWhen::Regenerates".to_string(),
                };
                let body = self.operand_code(body);
                self.plan_code(
                    &feature,
                    &format!("{M}::delayed::reflexive_on(g, item, {when}, {body})"),
                    out,
                );
            }
            Operation::CreateBoon {
                actor,
                event,
                condition,
                times,
                body,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let times = self.fetch(self.number(times)?.amount(u32::MAX.into(), "u32")?);
                self.create_boon(
                    &feature,
                    player,
                    times,
                    event,
                    condition.as_deref(),
                    body,
                    out,
                )?;
            }
            Operation::DelayedUnlessPaid {
                event,
                body,
                payer,
                mana,
            } => {
                let payer = self.fetch(self.player(payer)?);
                let (event, body, mana) = (
                    self.operand_code(event),
                    self.operand_code(body),
                    self.operand_code(mana),
                );
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::delayed::unless_paid(g, item, {event}, {payer}, {mana}, {body})"
                    ),
                    out,
                );
            }
            // "A face-down pile and a face-up pile" (700.3): the separator chooses
            // the first, and the second is shown to everyone.
            Operation::Piles {
                actor,
                objects,
                count: Some(_),
                names,
                chooser: None,
            } if names == &["face up", "face down"] => {
                let player = self.one_actor(actor, &feature)?;
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let separator_{id} = {player}; let cards_{id} = {objects}; let (face_down_{id}, face_up_{id}) = {M}::choose::separate_face_down_and_face_up(g, separator_{id}, asker.source, cards_{id})?;").unwrap();
                self.face_piles = Some(id);
            }
            Operation::Piles {
                actor,
                objects,
                count,
                names,
                chooser,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let objects = self.fetch(self.objects(objects)?);
                let count = match count {
                    Some(count) => format!(
                        "Some({})",
                        self.fetch(self.number(count)?.amount(u32::MAX.into(), "u32")?)
                    ),
                    None => "None".to_string(),
                };
                let chooser = match chooser {
                    Some(chooser) => format!("Some({})", self.fetch(self.players(chooser)?)),
                    None => "None".to_string(),
                };
                self.plan_code(&feature, &format!("{M}::choose::into_piles(g, {player}, asker.source, {objects}, {count}, &{names:?}, {chooser})?"), out);
            }
            // The chooser sees the face-up pile, and how many the other holds. Its
            // cards go on revealed, the other's never shown.
            Operation::ChoosePile {
                actor,
                from: None,
                random: false,
                names,
                object: None,
                then: Some(Destination::OwnersHand),
            } if names.is_empty() && self.face_piles.is_some() => {
                let piles = self.face_piles.take().unwrap_or_default();
                let chooser = match actor {
                    PlayerSet::One(player) => format!(
                        "let chooser_{id} = {};",
                        self.fetch(self.certain_player(player)?)
                    ),
                    PlayerSet::OpponentsOf(_) => {
                        const VERB: &str = "decides which of the two piles you put into your hand; the other goes into your graveyard";
                        let among = self.fetch(self.players(actor)?);
                        format!("let among_{id} = {among}; let Some(chooser_{id}) = {M}::choose::player(g, asker.controller, asker.source, among_{id}, {VERB:?}, 0)? else {{ return Ok(()) }};")
                    }
                    _ => return Err(gap(feature)),
                };
                writeln!(out, "if !(face_down_{piles}.is_empty() && face_up_{piles}.is_empty()) {{ {chooser} let (keep_{id}, _) = {M}::choose::face_down_or_face_up_pile(g, chooser_{id}, asker.source, face_down_{piles}, face_up_{piles}.clone())?; if keep_{id} == face_up_{piles} {{ g.return_to_hand_revealed(&keep_{id}); }} else {{ g.return_to_hand(&keep_{id}); }} }}").unwrap();
            }
            Operation::ChoosePile {
                actor,
                from,
                random,
                names,
                object,
                then,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let from = match from {
                    Some(from) => format!("Some({})", self.fetch(self.players(from)?)),
                    None => "None".to_string(),
                };
                let object = match object {
                    Some(object) => self.fetch(self.object(object)?),
                    None => "None".to_string(),
                };
                let then = then.as_ref().map(|d| form(d).to_string());
                self.plan_code(&feature, &format!("{M}::choose::of_piles(g, {player}, asker.source, {from}, {random}, &{names:?}, {object}, {then:?})?"), out);
            }
            Operation::PutStickers {
                actor,
                kind,
                objects,
                amount,
                any_number,
                ticket_cost,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let objects = self.fetch(self.objects(objects)?);
                let amount = match amount {
                    Some(amount) => format!(
                        "Some({})",
                        self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?)
                    ),
                    None => "None".to_string(),
                };
                let ticket = match ticket_cost {
                    Some((comparison, cost)) => format!(
                        "Some((Comparison::{comparison:?}, {}))",
                        self.fetch(self.number(cost)?.code)
                    ),
                    None => "None".to_string(),
                };
                self.plan_code(&feature, &format!("{M}::stickers::stickers(g, {player}, asker.source, {:?}, &{objects}, {amount}, {any_number}, {ticket})?", format!("{kind:?}")), out);
            }
            Operation::Populate { actor, entry } => {
                let player = self.one_actor(actor, &feature)?;
                let entry: Vec<String> = entry.iter().map(|o| self.operand_code(o)).collect();
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::tokens::populate(g, {player}, asker.source, &[{}])?",
                        entry.join(", ")
                    ),
                    out,
                );
            }
            Operation::AnyPlayerMayPay {
                players,
                mana,
                unpaid,
                paid,
            } => {
                let players = self.fetch(self.players(players)?);
                let parts = |this: &mut Self, list: &[Operand]| -> String {
                    list.iter()
                        .map(|o| this.operand_code(o))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let (mana, unpaid, paid) =
                    (parts(self, mana), parts(self, unpaid), parts(self, paid));
                self.plan_code(&feature, &format!("{M}::players::any_may_pay(g, asker, {players}, &[{mana}], &[{unpaid}], &[{paid}])?"), out);
            }
            Operation::PayRepeatedly {
                player,
                cost,
                at_most,
            } => {
                let player = self.fetch(self.certain_player(player)?);
                let most = self.fetch(self.number(at_most)?.amount(u16::MAX.into(), "u16")?);
                let cost = quoted(&format!("{cost:?}"));
                let times = planned::planned_value(
                    &feature,
                    "0i64",
                    &format!(
                        "{M}::players::pay_repeatedly(g, {player}, asker.source, {cost}, {most})?"
                    ),
                );
                writeln!(
                    out,
                    "let bound_{id}: i64 = {times}; let done_{id}: i64 = bound_{id};"
                )
                .unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
                self.values.insert(result.0, format!("bound_{id}"));
            }
            Operation::PlayersPayRepeatedly {
                players,
                cost,
                at_most,
            } => {
                let players = self.fetch(self.players(players)?);
                let most = self.fetch(self.number(at_most)?.amount(u16::MAX.into(), "u16")?);
                let cost = quoted(&format!("{cost:?}"));
                self.plan_code(&feature, &format!("{M}::players::each_pays_repeatedly(g, {players}, asker.source, {cost}, {most})?"), out);
            }
            // "May choose any number of ... and pay {4} for each": the choice first,
            // then one payment for all of it, or none chosen after all.
            Operation::ChooseAndPay {
                actor,
                objects,
                cost,
            } if matches!(cost.parts.as_slice(), [CostPart::Mana(_)]) => {
                self.helpers.insert("pay_question");
                let player = self.one_actor(actor, &feature)?;
                let objects = self.fetch(self.objects(objects)?);
                let one = self.cost(cost, super::lane_g::PAID)?;
                writeln!(out, "let payer_{id} = {player}; let among_{id}: Vec<ObjectId> = {objects}; let picked_{id} = {M}::choose::select(g, payer_{id}, asker.source, Purpose::Untap, among_{id}, {M}::select::cards({M}::select::ANY_NUMBER))?; let price_{id} = ({one}).mana.times(picked_{id}.len() as u32); let chosen_{id}: Vec<ObjectId> = match price_{id} {{ Some(mana) if !picked_{id}.is_empty() && g.may_pay(payer_{id}, asker.source, asker.source, Cost::mana(mana), &PAY_QUESTION)? => picked_{id}, _ => Vec::new() }};").unwrap();
                self.selections.insert(result.0, format!("chosen_{id}"));
                self.moved.insert(result.0, format!("chosen_{id}"));
            }
            Operation::ChooseAndPay {
                actor,
                objects,
                cost,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let objects = self.fetch(self.objects(objects)?);
                let cost = quoted(&format!("{cost:?}"));
                let chosen = planned::planned_value(&feature, "Vec::<ObjectId>::new()", &format!("{M}::players::choose_and_pay(g, {player}, asker.source, {objects}, {cost})?"));
                writeln!(out, "let chosen_{id}: Vec<ObjectId> = {chosen};").unwrap();
                self.selections.insert(result.0, format!("chosen_{id}"));
                self.moved.insert(result.0, format!("chosen_{id}"));
            }
            Operation::EachCounterKind {
                actor,
                among,
                options,
                optional,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let among = self.fetch(self.objects(among)?);
                let options = quoted(&format!("{options:?}"));
                self.plan_code(&feature, &format!("{M}::counters::each_kind(g, {player}, asker.source, {among}, {options}, {optional})?"), out);
            }
            Operation::PutChosenCounters { objects, choice } => {
                let objects = self.fetch(self.objects(objects)?);
                let choice = quoted(&format!("{choice:?}"));
                self.plan_code(
                    &feature,
                    &format!("{M}::counters::chosen(g, asker, {objects}, {choice})?"),
                    out,
                );
            }
            Operation::AddPhases {
                actor,
                phases,
                count,
                at_start,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let count = self.number(count)?.amount(u32::MAX.into(), "u32")?;
                let count = self.fetch(count);
                match (phases, at_start) {
                    (AddedPhases::Combat, None) => {
                        writeln!(out, "for _ in 0..({count}) {{ g.add_combat_phase(); }}").unwrap()
                    }
                    (phases, at_start) => {
                        let at_start = at_start.as_ref().map(|o| self.operand_code(o));
                        self.plan_code(&feature, &format!("g.add_phases({player}, AddedPhases::{phases:?}, {count}, {at_start:?})"), out);
                    }
                }
            }
            Operation::CounterWith {
                object,
                when,
                destination,
                entry,
            } => {
                let spell = self.fetch(self.object(object)?);
                let test = match when {
                    Some(filter) => {
                        let test = self.filter(filter, &Candidate::of("id"))?;
                        format!("Some(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})")
                    }
                    None => "None".to_string(),
                };
                let entry: Vec<String> = entry.iter().map(|o| self.operand_code(o)).collect();
                self.plan_code(&feature, &format!("if let Some(spell) = {spell} {{ {M}::counter::to(g, spell, {test}, Destination::{}, &[{}]); }}", form(destination), entry.join(", ")), out);
            }
            Operation::GiveControl {
                players,
                objects,
                until,
            } => {
                let players = self.fetch(self.players(players)?);
                let objects = self.fetch(self.objects(objects)?);
                let until = match until {
                    Some(until) => format!("Some({})", self.duration(until)?),
                    None => "None".to_string(),
                };
                self.plan_code(
                    &feature,
                    &format!("{M}::control::give(g, asker.source, {players}, {objects}, {until})"),
                    out,
                );
            }
            Operation::AttachEach {
                actor,
                attachments,
                count,
                hosts,
                each_host,
            } => {
                let player = self.one_actor(actor, &feature)?;
                if let Recipients::Objects(on) = hosts {
                    let shape = (count, *each_host, matches!(on, ObjectSet::Selected(_)));
                    let one = |n: &Expr| matches!(n, Expr::Constant(1));
                    let mine = match shape {
                        (None, false, true) => Some(AttachShape::AllToTheChosen),
                        (Some(Count::UpTo(n)), true, _) if one(n) => Some(AttachShape::OneToEach),
                        (Some(Count::AnyNumber), false, false) => Some(AttachShape::EachToOne),
                        _ => None,
                    };
                    if let Some(shape) = mine {
                        let attachments = self.fetch(self.objects(attachments)?);
                        let on = self.fetch(self.objects(on)?);
                        attach_each(id, &player, &attachments, &on, shape, out);
                        return Ok(());
                    }
                }
                let attachments = self.fetch(self.objects(attachments)?);
                let hosts = self.fetch(self.recipients(hosts)?);
                let count = match count {
                    Some(count) => format!("Some({})", self.fetch(self.select_count(count)?)),
                    None => "None".to_string(),
                };
                self.plan_code(&feature, &format!("{M}::attach::each(g, {player}, asker.source, {attachments}, {hosts}, {count}, {each_host})?"), out);
            }
            Operation::AttachChosen {
                actor,
                attachments,
                count,
                hosts,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let attachments = self.fetch(self.objects(attachments)?);
                let hosts = self.fetch(self.objects(hosts)?);
                let count = self.fetch(self.select_count(count)?);
                self.plan_code(&feature, &format!("{M}::attach::chosen(g, {player}, asker.source, {attachments}, {count}, {hosts})?"), out);
            }
            Operation::CoinFlips {
                actor,
                times,
                each,
                until_lost,
                may_stop,
                called,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let times = match (times, each) {
                    (Some(times), None) => {
                        let n = self.number(times)?.amount(u32::MAX.into(), "u32")?;
                        format!("Some({})", self.fetch(n))
                    }
                    (None, Some(each)) => {
                        format!("Some({}.len() as u32)", self.fetch(self.objects(each)?))
                    }
                    (None, None) => "None".to_string(),
                    (Some(_), Some(_)) => return Err(gap(feature)),
                };
                if *may_stop || *called {
                    self.plan_code(&feature, &format!("{M}::coins::flips(g, {player}, {times}, {until_lost}, {may_stop}, {called})?"), out);
                    return Ok(());
                }
                writeln!(out, "let player_{id} = {player}; let times_{id}: Option<u32> = {times}; let mut done_{id}: i64 = 0; let mut flips_{id}: u32 = 0; while times_{id}.map_or(true, |n| flips_{id} < n) {{ flips_{id} += 1; if g.flip_coin(player_{id}) {{ done_{id} += 1; }} else if {until_lost} {{ break; }} }}").unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
                self.coins = Some((
                    format!("done_{id}"),
                    format!("(i64::from(flips_{id}) - done_{id})"),
                ));
            }
            Operation::MillUntil {
                actor,
                cards,
                at_most,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let most = self.fetch(self.number(at_most)?.amount(u32::MAX.into(), "u32")?);
                let test = self.filter(cards, &Candidate::of("id"))?;
                writeln!(out, "let player_{id} = {player}; let most_{id} = {most}; let mut milled_{id}: Vec<ObjectId> = Vec::new(); while (milled_{id}.len() as u32) < most_{id} {{ let one = g.mill(player_{id}, 1); if one.is_empty() {{ break; }} let found = {{ let q: &dyn Query = g; one.iter().copied().any(|id| {test}) }}; milled_{id}.extend(one); if found {{ break; }} }}").unwrap();
                self.moved.insert(result.0, format!("milled_{id}"));
                self.left_behind(result, format!("milled_{id}.clone()"));
            }
            Operation::DistributeCounters {
                amount,
                kind,
                objects,
                any_number,
            } => {
                let kind = counter_kind(kind)?;
                let amount = self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?);
                let least = u32::from(!*any_number);
                writeln!(out, "let objects_{id} = {}; let mut left_{id}: u32 = {amount}; let mut puts_{id}: Vec<(ObjectId, CounterKind, i16)> = Vec::new(); for (i, &object) in objects_{id}.iter().enumerate() {{ let after = (objects_{id}.len() - i - 1) as u32; let n = if after == 0 {{ left_{id} }} else {{ let most = left_{id}.saturating_sub(after * {least}); {M}::choose::number(g, asker.controller, asker.source, {least}.min(most), most, \"Distribute\")? }}; left_{id} -= n.min(left_{id}); if n > 0 {{ puts_{id}.push((object, {kind}, n as i16)); }} }} g.add_counters_together(&puts_{id});", self.acted_on(objects)?).unwrap();
            }
            _ => return Err(gap(feature)),
        }
        Ok(())
    }
    /// What a spell gets, as the word `spell_changes` will have for it.
    fn spell_change(&mut self, change: &Change) -> Result<String> {
        Ok(match change {
            Change::EntersWithCounters { kind, amount } => format!(
                "SpellChange::EntersWithCounters({}, {})",
                counter_kind(kind)?,
                self.number(amount)?.amount(i16::MAX.into(), "i16")?
            ),
            Change::CantBeCountered => "SpellChange::CantBeCountered".to_string(),
            Change::SpellCost(modifier) => {
                format!("SpellChange::Cost({})", self.cost_delta(modifier)?)
            }
            Change::Continuing { changes, until } => {
                let changes = changes
                    .iter()
                    .map(|change| self.spell_change(change))
                    .collect::<Result<Vec<_>>>()?;
                format!(
                    "SpellChange::Continuing(&[{}], {})",
                    changes.join(", "),
                    self.duration(until)?
                )
            }
            Change::GrantAbility(ability) => {
                format!(
                    "SpellChange::Gains({})",
                    quoted(&form(&**ability).to_string())
                )
            }
            other => {
                let kinds = self.effect_kinds(other)?;
                let kinds: Vec<String> = kinds
                    .iter()
                    .map(|k| format!("SpellChange::Effect({k})"))
                    .collect();
                kinds.join(", ")
            }
        })
    }
    /// New targets, as a copy or a change of targets will word them.
    fn new_targets(&self, targets: &NewTargets) -> Result<String> {
        Ok(match targets {
            NewTargets::Same => "NewTargets::Same".to_string(),
            NewTargets::May => "NewTargets::May".to_string(),
            NewTargets::Must => "NewTargets::Must".to_string(),
            NewTargets::To(objects) => {
                format!("NewTargets::To({})", self.fetch(self.objects(objects)?))
            }
            NewTargets::Each(objects) => {
                format!("NewTargets::Each({})", self.fetch(self.objects(objects)?))
            }
            NewTargets::ToPlayers(players) => format!(
                "NewTargets::ToPlayers({})",
                self.fetch(self.players(players)?)
            ),
            NewTargets::Random(_) => "NewTargets::Random".to_string(),
            NewTargets::EachLegal => "NewTargets::EachLegal".to_string(),
            NewTargets::EachPlayer(players) => planned::planned_value(
                "new targets EachPlayer",
                "NewTargets::EachLegal",
                &format!(
                    "NewTargets::EachPlayer({})",
                    self.fetch(self.players(players)?)
                ),
            ),
            #[allow(unreachable_patterns)]
            other => return Err(gap(format!("new targets {}", form(other)))),
        })
    }
    /// `op`, compiled for planned code to hold.
    fn planned_operation(&mut self, op: &Operation, result: &ResultId) -> Result<String> {
        let mut body = String::new();
        let scope = self.results();
        let done = self.operation(op, result, &mut body);
        self.restore_results(scope);
        done?;
        Ok(body)
    }
    /// An operand, in planned code: what the generator can read, read, and the
    /// words of the rest. None of this runs until a requirement lands.
    pub(super) fn operand_code(&mut self, operand: &Operand) -> String {
        let read = |r: Result<String>, operand: &Operand| match r {
            Ok(code) => code,
            Err(_) => quoted(&format!("{operand:?}")),
        };
        match operand {
            Operand::Absent => "None".to_string(),
            Operand::Bool(value) => value.to_string(),
            Operand::Integer(value) => value.to_string(),
            Operand::Text(text) => quoted(text),
            Operand::List(list) => {
                let list: Vec<String> = list.iter().map(|o| self.operand_code(o)).collect();
                format!("&[{}]", list.join(", "))
            }
            Operand::Vocabulary { family, form } => format!("{family}::{form}"),
            Operand::Term {
                family,
                form,
                operands,
            } => {
                let operands: Vec<String> = operands.iter().map(|o| self.operand_code(o)).collect();
                match operands.is_empty() {
                    true => format!("{family}::{form}"),
                    false => format!("{family}::{form}({})", operands.join(", ")),
                }
            }
            Operand::Object(object) => read(self.object(object).map(|c| self.fetch(c)), operand),
            Operand::Objects(objects) => {
                read(self.objects(objects).map(|c| self.fetch(c)), operand)
            }
            Operand::Player(player) => read(self.player(player).map(|c| self.fetch(c)), operand),
            Operand::Players(players) => {
                read(self.players(players).map(|c| self.fetch(c)), operand)
            }
            Operand::Number(number) => {
                read(self.number(number).map(|n| self.fetch(n.code)), operand)
            }
            Operand::Condition(condition) => {
                read(self.condition(condition).map(|c| self.fetch(c)), operand)
            }
            Operand::Duration(duration) => match self.duration(duration) {
                Ok(code) => code,
                Err(_) => quoted(&format!("{operand:?}")),
            },
            Operand::Filter(filter) => read(
                self.filter(filter, &Candidate::of("id"))
                    .map(|test| format!("|q: &dyn Query, asker: &Asker, id: ObjectId| {test}")),
                operand,
            ),
            Operand::Program(program) => {
                let mut body = String::new();
                match self.block(program, &mut body, true) {
                    Ok(()) => {
                        format!("|g: &mut dyn Game| -> Result<(), Suspend> {{ {body} Ok(()) }}")
                    }
                    Err(_) => quoted(&format!("{operand:?}")),
                }
            }
            other => quoted(&format!("{other:?}")),
        }
    }
    /// A rule of chosen cards together, as a `Part`, or nothing if `select` has
    /// no such rule and none can be written here.
    pub(super) fn set_rule(&mut self, constraint: &SelectionConstraint) -> Option<String> {
        Some(match constraint {
            SelectionConstraint::DifferentNames => format!("{M}::select::DIFFERENT_NAMES"),
            SelectionConstraint::DifferentProperty(Property::ManaValue) => {
                format!("{M}::select::DIFFERENT_MANA_VALUES")
            }
            SelectionConstraint::DifferentProperty(Property::Power) => distinct_rule(
                &mut self.functions,
                "different_powers",
                "i16",
                "q.chars(id).power.unwrap_or(0)",
                "with different powers",
            ),
            SelectionConstraint::CommonProperty(DistinctProperty::CardTypes) => {
                format!("{M}::select::SHARE_A_CARD_TYPE")
            }
            SelectionConstraint::DifferentControllers => distinct_rule(
                &mut self.functions,
                "different_controllers",
                "PlayerId",
                "q.controller(id)",
                "controlled by different players",
            ),
            SelectionConstraint::DifferentOwners => distinct_rule(
                &mut self.functions,
                "different_owners",
                "PlayerId",
                "q.state().obj(id).owner",
                "with different owners",
            ),
            SelectionConstraint::TotalProperty {
                property,
                comparison: Comparison::LessOrEqual,
                amount,
            } => {
                let most = match amount {
                    Expr::Constant(n) if *n >= 0 => n.to_string(),
                    Expr::AnnouncedX => "TargetSpec::COUNTED".to_string(),
                    _ => return None,
                };
                match property {
                    Property::ManaValue => {
                        format!("{M}::select::total_mana_value_at_most({most})")
                    }
                    Property::Power => total_rule(
                        &mut self.functions,
                        "total_power",
                        "q.chars(id).power.unwrap_or(0)",
                        "with total power {n} or less",
                        &most,
                    ),
                    _ => return None,
                }
            }
            _ => return None,
        })
    }
    /// `.with(part)` for the rule a move keeps its chosen `cards` to, if `select`
    /// has it. The source's "from your graveyard" has to be what `cards` reads.
    pub(super) fn group_rule(&mut self, group: &Operand, cards: &ObjectSet) -> Option<String> {
        let from_graveyard = self.line_text().is_some_and(|line| line.contains("from your graveyard"));
        if from_graveyard && !self.objects(cards).ok()?.contains("Zone::graveyard") {
            return None;
        }
        let (form, operands): (&str, &[Operand]) = match group {
            Operand::Vocabulary { family, form } if family == "GroupFilter" => (form, &[]),
            Operand::Term { family, form, operands } if family == "GroupFilter" => (form, operands),
            Operand::Form(named) if named.source_type == "GroupFilter" => {
                (&named.form, &named.operands)
            }
            _ => return None,
        };
        let constraint = match (form, operands) {
            ("DifferentNames", []) => SelectionConstraint::DifferentNames,
            ("DifferentManaValues", []) => {
                SelectionConstraint::DifferentProperty(Property::ManaValue)
            }
            ("DifferentPowers", []) => SelectionConstraint::DifferentProperty(Property::Power),
            ("TotalManaValueIs", [Operand::Comparison(comparison, amount @ Expr::Constant(_))]) => {
                SelectionConstraint::TotalProperty {
                    property: Property::ManaValue,
                    comparison: comparison.clone(),
                    amount: amount.clone(),
                }
            }
            _ => return None,
        };
        Some(format!(".with({})", self.set_rule(&constraint)?))
    }
    /// `.with(part)` for each rule of a search.
    pub(super) fn search_rules(&mut self, constraints: &[SelectionConstraint]) -> Result<String> {
        let mut rules = String::new();
        for constraint in constraints {
            match self.set_rule(constraint) {
                Some(part) => write!(rules, ".with({part})").unwrap(),
                None => return Err(gap(format!("search constraint {}", form(constraint)))),
            }
        }
        Ok(rules)
    }
    /// One rule over the cards of a search for several things: the most it can
    /// take, and a judge that tells them off among the clauses, each with its own
    /// quality and number. `None` where a clause is more than that.
    fn clauses_judge(
        &self,
        id: usize,
        choices: &[SearchChoice],
    ) -> Result<Option<(String, String)>> {
        let mut tests = Vec::new();
        let mut counts = Vec::new();
        for choice in choices {
            let Count::UpTo(n) = &choice.count else {
                return Ok(None);
            };
            if !choice.constraints.is_empty() {
                return Ok(None);
            }
            let test = self.filter(&choice.filter, &Candidate::of("id"))?;
            let n = self.number(n)?.amount(u32::MAX.into(), "u32")?;
            tests.push(format!("&|id: ObjectId| {test}"));
            counts.push(n);
        }
        let total = counts.join(" + ");
        let rows: Vec<String> = counts
            .iter()
            .zip(&tests)
            .map(|(n, test)| format!("({n}, {test})"))
            .collect();
        let judge = format!(
            "#[allow(unused_variables)] fn clauses_{id}(q: &dyn Query, choice: &Choice, _: u32) -> Verdict {{ let asker = &Asker::new(choice.source, q.controller(choice.source)); clauses_verdict(choice, &mut [{}]) }}",
            rows.join(", ")
        );
        Ok(Some((total, judge)))
    }
    /// One search with several find clauses (CR 701.23h): the requirement is one
    /// episode with one result, so what reads the result stays real.
    pub(super) fn search_several(
        &mut self,
        player: &PlayerRef,
        library: &PlayerRef,
        choices: &[SearchChoice],
        result: &ResultId,
        destination: &str,
        out: &mut String,
    ) -> Result<()> {
        let id = self.fresh();
        let searcher = self.fetch(self.certain_player(player)?);
        let owner = self.fetch(self.certain_player(library)?);
        if let Some((total, judge)) = self.clauses_judge(id, choices)? {
            self.helpers.insert("clauses");
            writeln!(self.functions, "{judge}").unwrap();
            writeln!(out, "let searcher_{id} = {searcher}; let owner_{id} = {owner}; let found_{id} = {M}::library::search(g, searcher_{id}, owner_{id}, asker.source, SearchDestination::{destination}, {M}::select::Selection::new({M}::select::up_to({total})).with(cantrip_api::select::part(&(clauses_{id} as SelectFn), 0, \"one for each thing searched for\")))?;").unwrap();
            self.selections.insert(result.0, format!("found_{id}"));
            self.searched.insert(result.0, "true".into());
            return Ok(());
        }
        let mut clauses = Vec::new();
        for choice in choices {
            let test = self.filter(&choice.filter, &Candidate::of("id"))?;
            let count = self.fetch(self.select_count(&choice.count)?);
            let rules = self.search_rules(&choice.constraints)?;
            clauses.push(format!("(Selection::of({count}, &[]){rules}, |q: &dyn Query, asker: &Asker, id: ObjectId| {test})"));
        }
        let found = planned::planned_value(
            "search with several find clauses",
            "Vec::<ObjectId>::new()",
            &format!(
                "{M}::library::search_clauses(g, searcher_{id}, owner_{id}, asker.source, &[{}])?",
                clauses.join(", ")
            ),
        );
        writeln!(out, "let searcher_{id} = {searcher}; let owner_{id} = {owner}; let found_{id}: Vec<ObjectId> = {found};").unwrap();
        self.selections.insert(result.0, format!("found_{id}"));
        self.searched.insert(result.0, "true".into());
        Ok(())
    }
    /// A choice among objects held to a rule of them together.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose_with_rule(
        &mut self,
        player: &PlayerRef,
        objects: &ObjectSet,
        count: &Count,
        constraint: &SelectionConstraint,
        result: &ResultId,
        rest: &[Instruction],
        out: &mut String,
    ) -> Result<()> {
        let purpose = effects::purpose(result, rest)
            .or_else(|| plain_purpose(result, rest))
            .unwrap_or("ChooseAsEnters");
        let id = self.fresh();
        let chooser = self.fetch(self.certain_player(player)?);
        let pool = self.fetch(self.objects(objects)?);
        if let SelectionConstraint::OneForEach(filters) = constraint {
            let up_to = !matches!(count, Count::Exactly(_) | Count::AsManyAsCan(_));
            writeln!(out, "let player_{id} = {chooser}; let pool_{id} = {pool}; let mut picked_{id}: Vec<ObjectId> = Vec::new();").unwrap();
            for filter in filters {
                let test = self.filter(filter, &Candidate::of("id"))?;
                let each_type =
                    lane_k::printed_otherwise(self.line_text(), "of each permanent type");
                let again = match each_type {
                    true => String::new(),
                    false => format!(".filter(|id| !picked_{id}.contains(id))"),
                };
                let open = self.fetch(format!("pool_{id}.iter().copied(){again}.filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
                writeln!(out, "{{ let open = {open}; let one = {M}::choose::cards(g, player_{id}, asker.source, Purpose::{purpose}, open, 1, {up_to})?; for o in one {{ if !picked_{id}.contains(&o) {{ picked_{id}.push(o); }} }} }}").unwrap();
            }
            self.selections.insert(result.0, format!("picked_{id}"));
            return Ok(());
        }
        let Some(part) = self.set_rule(constraint) else {
            return Err(gap(format!("selection constraint {}", form(constraint))));
        };
        let count = self.fetch(self.select_count(count)?);
        let pool = match constraint {
            SelectionConstraint::TotalProperty {
                property: Property::ManaValue,
                comparison: Comparison::LessOrEqual,
                amount: Expr::Constant(most),
            } => format!(
                "{pool}.into_iter().filter(|&c| i64::from(g.mana_value(c)) <= {most}).collect::<Vec<ObjectId>>()"
            ),
            _ => pool,
        };
        writeln!(out, "let player_{id} = {chooser}; let pool_{id} = {pool}; let count_{id} = {count}; let picked_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::{purpose}, pool_{id}, {M}::select::Selection::of(count_{id}, &[]).with({part}))?;").unwrap();
        self.selections.insert(result.0, format!("picked_{id}"));
        Ok(())
    }
    /// The bounds of a choice of a number: `Comparison`s each number must meet.
    fn number_bounds(&self, options: &[Operand]) -> Result<(String, String)> {
        let unsupported = || gap("a choice of Number");
        let read = |n: &Expr| -> Result<String> {
            Ok(self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?))
        };
        // "A number between 0 and 7".
        if let [Operand::Integer(least), Operand::Integer(most)] = options {
            return Ok((format!("{least}u32"), format!("{most}u32")));
        }
        let (mut least, mut most) = ("0u32".to_string(), "i32::MAX as u32".to_string());
        for option in options {
            match option {
                Operand::Comparison(Comparison::GreaterOrEqual, n) => least = read(n)?,
                Operand::Comparison(Comparison::Greater, n) => least = format!("{} + 1", read(n)?),
                Operand::Comparison(Comparison::LessOrEqual, n) => most = read(n)?,
                Operand::Comparison(Comparison::Less, n) => {
                    most = format!("{}.saturating_sub(1)", read(n)?)
                }
                _ => return Err(unsupported()),
            }
        }
        Ok((least, most))
    }
    /// What `choose_value` has no arm of its own for.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn choose_value_rest(
        &mut self,
        kind: ValueKind,
        player: &str,
        source: &str,
        options: &[Operand],
        local: &str,
        id: usize,
        out: &mut String,
    ) -> Result<()> {
        let entering = self.pending == Pending::Entering;
        match kind {
            // CR 607.2f: words with no rules meaning, as the card's `labels`.
            ValueKind::Word => {
                let words = options
                    .iter()
                    .map(|option| match option {
                        Operand::Text(word) => Ok(word.clone()),
                        _ => Err(gap("a choice of Word")),
                    })
                    .collect::<Result<Vec<String>>>()?;
                if !self.word_options.is_empty() || self.entering_labels.is_some() {
                    return Err(gap("two choices of labels as it enters"));
                }
                let count = words.len();
                let labels: Vec<String> = words.iter().map(|w| quoted(w)).collect();
                self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
                self.word_options = words;
                match entering {
                    true => writeln!(out, "{M}::choose::option_as_enters(g, entering, &CARD, 0..{count})?; let {local}: u8 = entering.chosen_option.map_or(0, |chosen| chosen.index);").unwrap(),
                    false => writeln!(out, "let player_{id} = {player}; let card_{id} = g.card_id(&CARD); let {local}: u8 = {M}::choose::option(g, player_{id}, {source}, card_{id}, 0..{count})?;").unwrap(),
                }
            }
            ValueKind::Number => {
                let (least, most) = self.number_bounds(options)?;
                match entering {
                    true => writeln!(out, "let least_{id} = {least}; let most_{id} = {most}; {M}::choose::number_as_enters(g, entering, least_{id}, most_{id}, \"Choose a number\")?; let {local}: u32 = entering.chosen_number.unwrap_or(0);").unwrap(),
                    false => writeln!(out, "let player_{id} = {player}; let least_{id} = {least}; let most_{id} = {most}; let {local}: u32 = {M}::choose::number(g, player_{id}, {source}, least_{id}, most_{id}, \"Choose a number\")?;").unwrap(),
                }
            }
            other => return self.choose_value_p(other, player, source, options, local, id, out),
        }
        Ok(())
    }
    /// The controller's choice of one of the words for each of `players`.
    pub(super) fn word_for_each_player(
        &mut self,
        id: usize,
        players: &PlayerSet,
        options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        let words = options
            .iter()
            .map(|option| match option {
                Operand::Text(word) => Ok(word.clone()),
                _ => Err(gap("a choice of Word")),
            })
            .collect::<Result<Vec<String>>>()?;
        if !self.word_options.is_empty() || self.entering_labels.is_some() {
            return Err(gap("two choices of labels as it enters"));
        }
        let count = words.len();
        let labels: Vec<String> = words.iter().map(|w| quoted(w)).collect();
        self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
        self.word_options = words;
        let players = self.fetch(self.players(players)?);
        let local = format!("words_{id}");
        writeln!(out, "let card_{id} = g.card_id(&CARD); let mut {local}: Vec<(PlayerId, u8)> = Vec::new(); for player in {players} {{ let word = {M}::choose::option(g, asker.controller, asker.source, card_{id}, 0..{count})?; {local}.push((player, word)); }}").unwrap();
        self.player_words = Some(local);
        Ok(())
    }
    /// `Condition::ChosenWord`: this body's choice, or what the permanent keeps.
    pub(super) fn chosen_word(&self, word: &str) -> Result<String> {
        let index = self
            .word_options
            .iter()
            .position(|w| w == word)
            .ok_or_else(|| gap("condition ChosenWord"))?;
        if let Ok(local) = self.chosen_local(ValueKind::Word) {
            return Ok(format!("{local} == {index}"));
        }
        if !self.enters_choosing.contains(&ValueKind::Word) {
            return Err(gap("a choice of Word outside its body"));
        }
        Ok(match self.reader {
            Reader::Layer => format!(
                "q.state().chosen_option(q.linked_to(asker.source)).filter(|c| std::ptr::eq(q.cards().get(c.card), &CARD)).map(|c| c.index) == Some({index})"
            ),
            _ => format!("{M}::choose::chosen_option(q, asker, &CARD) == Some({index})"),
        })
    }
    /// "A choice of" on terms past one plain value: secret, several, up to, not
    /// what was chosen before. Later reads see the local it will be.
    pub(super) fn choose_value_terms(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        kind: ValueKind,
        terms: &ChoiceTerms,
        out: &mut String,
    ) -> Result<()> {
        let players = self.fetch(self.players(actor)?);
        let options: Vec<String> = terms.options.iter().map(|o| self.operand_code(o)).collect();
        let except: Vec<String> = terms.except.iter().map(|o| self.operand_code(o)).collect();
        let count = match &terms.count {
            Some(count) => format!(
                "Some({})",
                self.fetch(self.number(count)?.amount(u32::MAX.into(), "u32")?)
            ),
            None => "None".to_string(),
        };
        let ChoiceTerms {
            secret,
            unchosen,
            random,
            ordered,
            up_to,
            ..
        } = terms;
        if let (ValueKind::CreatureType, PlayerSet::One(player), [_, ..]) =
            (kind, actor, except.as_slice())
        {
            let plain = terms.options.is_empty()
                && terms.count.is_none()
                && !(*secret || *unchosen || *random || *ordered || *up_to)
                && except.iter().all(|word| word.starts_with("CreatureType::"))
                && !self.chosen.iter().any(|(k, _)| *k == kind);
            if plain {
                let player = self.fetch(self.certain_player(player)?);
                let except: Vec<String> = except
                    .iter()
                    .map(|word| {
                        let word = word.replacen("CreatureType::", "Subtype::", 1);
                        format!("g.words(asker.source).subtype({word})")
                    })
                    .collect();
                let local = format!("chosen_{id}");
                writeln!(out, "let player_{id} = {player}; let except_{id} = [{}]; let ask_{id} = RequestKind::ChooseSubtype {{ source: asker.source, choices: Subtype::CREATURE_TYPES.iter().copied().filter(|t| !except_{id}.contains(t)).collect(), purpose: WordUse::Chosen }}; let Answer::Subtype({local}) = g.ask(player_{id}, ask_{id})? else {{ unreachable!(\"the core answers a subtype choice with a subtype\") }};", except.join(", ")).unwrap();
                self.chosen.push((kind, local));
                return Ok(());
            }
        }
        let code = format!("{M}::choose::value(g, {players}, asker.source, ChoiceKind::{kind:?}, ChoiceTerms {{ options: &[{}], except: &[{}], count: {count}, secret: {secret}, unchosen: {unchosen}, random: {random}, ordered: {ordered}, up_to: {up_to} }})?", options.join(", "), except.join(", "));
        self.value_to_come("operation ChooseValueWith", id, kind, &code, out);
        Ok(())
    }
    /// A chosen value Cantrip can't yet ask for: bound, where the kind has a
    /// placeholder, so what reads it stays real.
    pub(super) fn value_to_come(
        &mut self,
        feature: &str,
        id: usize,
        kind: ValueKind,
        code: &str,
        out: &mut String,
    ) {
        let placeholder = match kind {
            ValueKind::Number => Some("0u32"),
            ValueKind::Color => Some("Color::White"),
            ValueKind::CreatureType | ValueKind::BasicLandType | ValueKind::LandType => {
                Some("Subtype::Plains")
            }
            ValueKind::Player => Some("asker.controller"),
            _ => None,
        };
        match placeholder {
            Some(placeholder) if !self.chosen.iter().any(|(k, _)| *k == kind) => {
                let local = format!("chosen_{id}");
                let value = planned::planned_value(feature, placeholder, code);
                writeln!(out, "let {local} = {value};").unwrap();
                self.chosen.push((kind, local));
            }
            _ => self.plan_code(feature, code, out),
        }
    }
    /// Objects exiled with counters: the counters come with them, as they arrive.
    pub(super) fn exile_with_counters(&mut self, id: usize, entry: &EntrySpec) -> Result<String> {
        let mut counters = Vec::new();
        for (kind, n) in &entry.counters {
            let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
            counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
        }
        Ok(format!("{{ let counters: Vec<(CounterKind, i16)> = vec![{}]; let exiled = g.exile_all(&objects_{id}); let puts: Vec<(ObjectId, CounterKind, i16)> = exiled.iter().flat_map(|&o| counters.iter().map(move |&(kind, n)| (o, kind, n))).collect(); g.add_counters_together(&puts); exiled }}", counters.join(", ")))
    }
    /// What `arrival_in_state` has no arm for: one object, for `moved`.
    pub(super) fn arrival_rest(&mut self, entry: &EntrySpec, counters: &str) -> Result<String> {
        let tapped = entry.tapped;
        Ok(match entry.states.as_slice() {
            [EntryState::AttachedTo(host)] if counters.is_empty() && !tapped => {
                let host = self.fetch(self.object(host)?);
                format!("if let Some(host) = {host} {{ moved.extend(g.put_onto_battlefield_attached(o, controller, host)?); }}")
            }
            states => {
                let states = quoted(&format!("{states:?}"));
                let call = planned::planned_value(
                    "move arrival state",
                    "None::<ObjectId>",
                    &format!("{M}::enter::in_state(g, o, controller, item, {states}, &[{counters}], {tapped})?"),
                );
                format!("moved.extend({call});")
            }
        })
    }
    /// What `effect_on` has no arm for: effects on what is there, on spells as
    /// they are cast, or on players.
    pub(super) fn effect_on_rest(
        &mut self,
        subject: &EffectSubject,
        effects: &[Operand],
        until: Option<&crate::ir::Duration>,
        out: &mut String,
    ) -> Result<()> {
        let subject = match subject {
            EffectSubject::Objects(objects) => {
                format!("EffectOn::Objects({})", self.fetch(self.objects(objects)?))
            }
            EffectSubject::Players(players) => {
                format!("EffectOn::Players({})", self.fetch(self.players(players)?))
            }
            EffectSubject::EachSpell(filter) => {
                let test = self.filter(filter, &Candidate::of("id"))?;
                format!("EffectOn::EachSpell(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})")
            }
            EffectSubject::NextSpell {
                spells,
                caster,
                this_turn,
                from,
            } => {
                let test = self.filter(spells, &Candidate::of("id"))?;
                let caster = self.fetch(self.certain_player(caster)?);
                format!("EffectOn::NextSpell {{ spells: |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, caster: {caster}, this_turn: {this_turn}, from: {:?} }}", from.as_ref().map(|z| format!("{z:?}")))
            }
        };
        let until = match until {
            Some(until) => format!("Some({})", self.duration(until)?),
            None => "None".to_string(),
        };
        let effects: Vec<String> = effects.iter().map(|o| self.operand_code(o)).collect();
        self.plan_code(
            "operation EffectOn",
            &format!(
                "{M}::effects::effect_on(g, asker, {subject}, &[{}], {until})?",
                effects.join(", ")
            ),
            out,
        );
        Ok(())
    }
    /// "Each of them deals damage": one event, with what each deals read for it.
    fn damage_from_each(
        &mut self,
        id: usize,
        sources: &ObjectSet,
        recipients: &Recipients,
        amount: &Expr,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let saved = self.affected_object.replace("source");
        let amount = self
            .number(amount)
            .and_then(|n| n.amount(u16::MAX.into(), "u16"));
        let targets = self.recipients(recipients);
        self.affected_object = saved;
        let (amount, targets) = (self.fetch(amount?), self.fetch(targets?));
        writeln!(out, "let sources_{id} = {}; let mut batch_{id}: Vec<(ObjectId, Target, u16)> = Vec::new(); for &source in &sources_{id} {{ let amount = {amount}; for t in {targets} {{ batch_{id}.push((source, t, amount)); }} }} let dealt_{id} = g.damage_together(&batch_{id}); let done_{id}: i64 = dealt_{id}.iter().map(|&n| i64::from(n)).sum();", self.fetch(self.objects(sources)?)).unwrap();
        self.performed.insert(result.0, format!("done_{id}"));
        Ok(())
    }
    /// "Reveals a card from their hand": at random, of their choice, or all.
    #[allow(clippy::too_many_arguments)]
    fn reveal_from_hand(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        cards: &ObjectFilter,
        count: Option<&Expr>,
        at_random: bool,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let player = self.one_actor(actor, "operation RevealFromHand")?;
        let test = self.filter(cards, &Candidate::of("id"))?;
        let hand = self.fetch(format!("q.state().zone(Zone::hand(player_{id})).iter().copied().filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
        writeln!(
            out,
            "let player_{id} = {player}; let mut pool_{id}: Vec<ObjectId> = {hand};"
        )
        .unwrap();
        match (count, at_random) {
            (None, _) => writeln!(out, "let shown_{id} = pool_{id};").unwrap(),
            (Some(n), true) => {
                let n = self.fetch(self.number(n)?.amount(u32::MAX.into(), "usize")?);
                writeln!(out, "let count_{id} = {n}; let mut shown_{id}: Vec<ObjectId> = Vec::new(); while shown_{id}.len() < count_{id} && !pool_{id}.is_empty() {{ let at = g.choose_at_random(pool_{id}.len()); shown_{id}.push(pool_{id}.remove(at)); }}").unwrap();
            }
            (Some(n), false) => {
                let n = self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?);
                writeln!(out, "let count_{id} = {n}; let shown_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::Reveal, pool_{id}, {M}::select::Selection::of({M}::select::Count::AsManyAsCan(count_{id}), &[]))?;").unwrap();
            }
        }
        writeln!(out, "g.reveal(player_{id}, &shown_{id});").unwrap();
        self.revealed = true;
        if matches!(count, Some(Expr::Constant(1))) {
            self.single.insert(result.0);
        }
        self.selections.insert(result.0, format!("shown_{id}"));
        Ok(())
    }
    /// "Chosen at random", which the engine records as one (`Event::ChosenAtRandom`).
    fn choose_at_random(
        &mut self,
        id: usize,
        kind: ValueKind,
        options: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        let entering = self.pending == Pending::Entering;
        let local = format!("chosen_{id}");
        match (kind, options) {
            (ValueKind::Player, [Operand::Players(players)]) => {
                let players = self.fetch(self.players(players)?);
                writeln!(out, "let pool_{id}: Vec<PlayerId> = {players}; if pool_{id}.is_empty() {{ return Ok(()); }} let {local} = pool_{id}[g.choose_at_random(pool_{id}.len())];").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_player = Some({local});").unwrap();
                }
            }
            (ValueKind::Number, options) => {
                let number = match options {
                    [Operand::Comparison(Comparison::GreaterOrEqual, least), Operand::Comparison(Comparison::LessOrEqual, most)] =>
                    {
                        let least = self.number(least)?.amount(u32::MAX.into(), "u32")?;
                        let most = self.number(most)?.amount(u32::MAX.into(), "u32")?;
                        format!("{{ let least = {}; let most = {}; least + g.choose_at_random((most.saturating_sub(least) + 1) as usize) as u32 }}", self.fetch(least), self.fetch(most))
                    }
                    numbers => {
                        let numbers = numbers
                            .iter()
                            .map(|n| match n {
                                Operand::Integer(n) if *n >= 0 => Ok(format!("{n}u32")),
                                _ => Err(gap("operation ChooseAtRandom")),
                            })
                            .collect::<Result<Vec<_>>>()?;
                        format!(
                            "{{ let pool = [{}]; pool[g.choose_at_random(pool.len())] }}",
                            numbers.join(", ")
                        )
                    }
                };
                writeln!(out, "let {local}: u32 = {number};").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_number = Some({local});").unwrap();
                }
            }
            _ => return Err(gap("operation ChooseAtRandom")),
        }
        self.chosen.push((kind, local));
        Ok(())
    }
    /// Two things trade places (CR 701.12).
    fn exchange(
        &mut self,
        id: usize,
        kind: &ExchangeKind,
        first: &Operand,
        second: &Operand,
        until: Option<&crate::ir::Duration>,
        out: &mut String,
    ) -> Result<()> {
        let unsupported = || gap("operation Exchange");
        match (kind, first, second, until) {
            // Each gains or loses what makes the two equal (701.12c, 119.5).
            (ExchangeKind::LifeTotals, Operand::Player(a), Operand::Player(b), None) => {
                writeln!(out, "let (first_{id}, second_{id}) = ({}, {}); if let (Some(first), Some(second)) = (first_{id}, second_{id}) {{ let (a, b) = (g.state().player(first).life, g.state().player(second).life); cantrip_api::instruction(g, |g| {{ g.set_life_total(first, b); g.set_life_total(second, a); }}); }}", self.fetch(self.player(a)?), self.fetch(self.player(b)?)).unwrap();
            }
            // Control of two permanents, for good (701.12b).
            (ExchangeKind::Control, Operand::Object(a), Operand::Object(b), None)
                if self.exchanges_a_spell(a, b).is_some() =>
            {
                let (spell, permanent) = self.exchanges_a_spell(a, b).expect("just asked");
                writeln!(out, "let (spell_{id}, permanent_{id}) = ({}, {}); if let (Some(spell), Some(permanent)) = (spell_{id}, permanent_{id}) {{ if g.target_legal(item, {spell}) && g.target_legal(item, {permanent}) {{ let by = g.state().stack.iter().find(|i| i.kind == StackKind::Spell && i.object == spell).map(|i| i.controller); let other = g.controller(permanent); if let Some(by) = by {{ if by != other {{ cantrip_api::instruction(g, |g| {{ g.change_controller(spell, other); g.add_effect(asker.source, permanent, EffectKind::Control(by), Duration::Indefinite); }}); }} }} }} }}", self.fetch(self.object(a)?), self.fetch(self.object(b)?)).unwrap();
            }
            (ExchangeKind::Control, Operand::Object(a), Operand::Object(b), None) => {
                writeln!(out, "let (first_{id}, second_{id}) = ({}, {}); if let (Some(first), Some(second)) = (first_{id}, second_{id}) {{ g.exchange_control(asker.source, first, second); }}", self.fetch(self.object(a)?), self.fetch(self.object(b)?)).unwrap();
            }
            _ => return Err(unsupported()),
        }
        Ok(())
    }
}
