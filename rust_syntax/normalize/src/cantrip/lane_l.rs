//! What a resolving effect does past the arms of `effects` and `lane_i`: the
//! operations the first round left, each the code Cantrip runs today, or the code
//! it will run once the requirement it names exists.
use super::*;

const M: &str = "cantrip_mechanics";

/// The player and what they sacrifice one of each, of "sacrifices an artifact and a
/// land of their choice".
pub(super) fn sacrifice_slots(operations: &[Operation]) -> Option<(&PlayerRef, Vec<ObjectSet>)> {
    let mut player: Option<&PlayerRef> = None;
    let mut slots = Vec::new();
    for operation in operations {
        let Operation::Sacrifice {
            players: PlayerSet::One(one),
            objects,
            choose: Some(Count::Exactly(Expr::Constant(1))),
        } = operation
        else {
            return None;
        };
        if player.is_some_and(|player| format!("{player:?}") != format!("{one:?}")) {
            return None;
        }
        player = Some(one);
        slots.push(objects.clone());
    }
    Some((player?, slots))
}
/// How many of `slots` can each have a permanent of its own, none of `taken`: what
/// "as many as they can" is, and so which choice of one still allows it (Release's
/// ruling: an artifact creature must go as the artifact if a creature can go too).
pub(super) const MOST_ASSIGNED: &str = "fn most_assigned(slots: &[Vec<ObjectId>], taken: &mut Vec<ObjectId>) -> usize {\n    let Some((first, rest)) = slots.split_first() else { return 0 };\n    let mut best = most_assigned(rest, taken);\n    for &object in first {\n        if !taken.contains(&object) {\n            taken.push(object);\n            best = best.max(1 + most_assigned(rest, taken));\n            taken.pop();\n        }\n    }\n    best\n}\n";
/// The N of a price of only `{N}`.
fn generic_mana(mana: &[Operand]) -> Option<i64> {
    match mana {
        [Operand::Term { form, operands, .. }] if form == "ManaCostGeneric" => {
            match operands.as_slice() {
                [Operand::Integer(n)] => Some(*n),
                _ => None,
            }
        }
        _ => None,
    }
}

/// The N of "cards with total mana value N or greater".
fn total_mana_value_at_least(constraints: &[SelectionConstraint]) -> Option<i64> {
    match constraints {
        [SelectionConstraint::TotalProperty {
            property: Property::ManaValue,
            comparison: Comparison::GreaterOrEqual,
            amount: Expr::Constant(n),
        }] => Some(*n),
        _ => None,
    }
}

/// A body where "you" is only who gains control: then "you" is the doer. One that
/// also says "a creature you control" (Infernal Denizen) can't tell the doer from
/// the ability's controller.
fn you_only_gains_control(body: &[Operand]) -> bool {
    let text = format!("{body:?}");
    let gains = text.matches("SetController(AbilityController)").count();
    gains > 0 && text.matches("AbilityController").count() == gains
}

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

/// "Have target player reveal their hand. If you do, choose a card from it and ...":
/// the hand is theirs, and the choice stays yours.
fn had_to_show_hand(player: &PlayerRef, program: &Program) -> Option<Program> {
    let [Instruction::Group(group)] = program.instructions.as_slice() else {
        return None;
    };
    let [first, second, rest @ ..] = group.instructions.as_slice() else {
        return None;
    };
    let their_hand = |objects: &ObjectSet| -> Option<ObjectSet> {
        let ObjectSet::Zone {
            scope: ZoneScope::Players { zone: EventZone::Hand, players: PlayerSet::One(PlayerRef::AbilityController) },
            kind,
            filter,
        } = objects
        else {
            return None;
        };
        Some(ObjectSet::Zone {
            scope: ZoneScope::Players { zone: EventZone::Hand, players: PlayerSet::One(player.clone()) },
            kind: kind.clone(),
            filter: filter.clone(),
        })
    };
    let Instruction::Effect { result: shown, operation: Operation::Reveal { objects } } = first else {
        return None;
    };
    let Instruction::Effect {
        result: picked,
        operation: Operation::ChooseObjects { player: PlayerRef::AbilityController, objects: pool, count, constraints },
    } = second
    else {
        return None;
    };
    let mut steps = vec![
        Instruction::Effect {
            result: shown.clone(),
            operation: Operation::Reveal { objects: their_hand(objects)? },
        },
        Instruction::Effect {
            result: picked.clone(),
            operation: Operation::ChooseObjects {
                player: PlayerRef::AbilityController,
                objects: their_hand(pool)?,
                count: count.clone(),
                constraints: constraints.clone(),
            },
        },
    ];
    steps.extend(rest.iter().cloned());
    let mut program = program.clone();
    program.instructions = steps;
    Some(program)
}

/// The one instruction `player` is had to do, said of them. The reduction leaves
/// the doer as "you" in `body`, which only these forms read back without doubt.
fn had_to_do(player: &PlayerRef, body: &[Operand]) -> Option<Program> {
    let [Operand::Program(program)] = body else {
        return None;
    };
    if let Some(shown) = had_to_show_hand(player, program) {
        return Some(shown);
    }
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

/// A cost asked again and again as a spell resolves.
fn pays_by_the_time(cost: &Cost) -> bool {
    matches!(
        cost.parts.as_slice(),
        [CostPart::Mana(_) | CostPart::Life(_) | CostPart::Discard { .. } | CostPart::Sacrifice { .. }]
    )
}

/// What `in_state` would have to do that Cantrip has no call for.
fn arrival_hole(states: &[EntryState]) -> &'static str {
    match states {
        [EntryState::Transformed, EntryState::AttachedTo(_) | EntryState::AttachedToPlayer(_)] => {
            "returns transformed and attached"
        }
        [EntryState::Blocking(_)] => "put onto the battlefield blocking",
        [EntryState::Flipped] => "returns flipped",
        [EntryState::May(_)] => "may enter face down",
        [EntryState::FaceDownAs(_)] => "put onto the battlefield face down as a stated creature",
        [EntryState::FaceDownWith { .. }] => "put onto the battlefield face down with abilities",
        [EntryState::FaceDownLand(_)] => "put onto the battlefield face down as a land",
        [EntryState::Perpetual(_)] => "a perpetual change as it enters",
        _ => "move arrival state",
    }
}

/// A rule of at most `most` total power or mana value, `most` being code.
pub(super) fn total_at_most_part(functions: &mut String, form: &str, most: &str) -> String {
    if form == "TotalPowerIs" {
        return total_rule(
            functions,
            "total_power",
            "q.chars(id).power.unwrap_or(0)",
            "with total power {n} or less",
            most,
        );
    }
    format!("{M}::select::total_mana_value_at_most({most})")
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
            // A player's counters are poison, energy and experience (CR 122.1), each
            // given by this ability's controller: no placement of them is one event.
            let more = match copies {
                C::OneOfEachKind => Some(("1u32", "1i32")),
                C::All => Some(("n", "n as i32")),
                _ => None,
            };
            if let (Some((more, energy)), true, None) = (more, from == to, kind) {
                let poison_more = Quantity::PoisonEffect.from_stored(more);
                writeln!(out, "for player in {from} {{ let (poison, energy, experience) = {{ let counters = g.state().player(player); (u32::from(counters.poison()), u32::from(counters.energy()), u32::from(counters.experience())) }}; let n = poison; if n > 0 {{ g.give_poison(player, {poison_more}); }} let n = energy; if n > 0 {{ g.change_energy(player, {energy}); }} let n = experience; if n > 0 {{ g.give_experience(player, {more}); }} }}").unwrap();
                return Ok(());
            }
            // "Choose a counter on target player. Give that player another counter
            // of that kind" (Animation Module): the kind is one of the three a
            // player can have, asked of this ability's controller among those
            // the player has, as a permanent's kind is asked.
            if let (C::One, true, None, None) = (copies, from == to, kind, &self.wanted_labels) {
                self.wanted_labels = Some("&[\"Poison counter\", \"Energy counter\", \"Experience counter\"]".into());
                writeln!(out, "for player in {from} {{ let have = {{ let counters = g.state().player(player); [counters.poison() > 0, counters.energy() > 0, counters.experience() > 0] }}; let offered: Vec<u8> = (0..3u8).filter(|&i| have[usize::from(i)]).collect(); if !offered.is_empty() {{ let card = g.card_id(&CARD); let pick = offered[{M}::choose::one_of(g, asker.controller, asker.source, offered.iter().map(|&i| Label::Card(card, i)).collect())? as usize]; match pick {{ 0 => g.give_poison(player, 1), 1 => g.change_energy(player, 1), _ => g.give_experience(player, 1) }} }} }}").unwrap();
                return Ok(());
            }
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
            format!("let puts: Vec<(ObjectId, CounterKind, {counter_ty})> = to_{id}.iter().copied(){lacks}.map(|o| (o, kind, 1{counter_ty})).collect(); g.add_counters_together(&puts);", counter_ty = Quantity::CounterPlacement.kind())
        };
        let code = match copies {
            C::All if same => format!("let puts: Vec<(ObjectId, CounterKind, {counter_ty})> = to_{id}.iter().flat_map(|&o| {{ let have = g.obj(o).counter_array(); kinds_{id}.iter().copied().filter(move |&kind| have[kind as usize] > 0).map(move |kind| (o, kind, {counter_read})).collect::<Vec<_>>() }}).collect(); g.add_counters_together(&puts);", counter_ty = Quantity::CounterPlacement.kind(), counter_read = Quantity::CounterPlacement.from_stored("have[kind as usize]")),
            C::All => {
                let counter_ty = Quantity::CounterPlacement.kind();
                let sum = if planned::switched_on(quantity::BOUNDED_32) {
                    "fold(0i32, i32::saturating_add)".to_string()
                } else { format!("sum::<{counter_ty}>()") };
                format!("let puts: Vec<(ObjectId, CounterKind, {counter_ty})> = to_{id}.iter().flat_map(|&o| kinds_{id}.iter().map(|&kind| (o, kind, from_{id}.iter().map(|&f| {copy_counter_read}).{sum})).collect::<Vec<_>>()).collect(); g.add_counters_together(&puts);", copy_counter_read = Quantity::CounterPlacement.from_stored("g.obj(f).counter_array()[kind as usize]"))
            },
            C::OneOfEachKind => format!("let puts: Vec<(ObjectId, CounterKind, {counter_ty})> = to_{id}.iter().flat_map(|&o| kinds_{id}.iter().map(move |&kind| (o, kind, 1{counter_ty})).collect::<Vec<_>>()).collect(); g.add_counters_together(&puts);", counter_ty = Quantity::CounterPlacement.kind()),
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
        self.chose_object_as_enters = true;
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
    /// Whether what comes later asks which objects paid, not only how much.
    pub(super) fn reads_paid_objects(&self, result: &ResultId) -> bool {
        let mark = format!("CostObjects {{ payment: Resolution(ResultId({}))", result.0);
        self.after.iter().any(|i| format!("{i:?}").contains(&mark))
    }
    /// The discard of a cost whose other parts are mana, if later instructions ask
    /// for what it took: `may_pay` doesn't say, its records do. The whole is paid
    /// after the mana is made (CR 601.2g-h), so a discard a mana ability cost comes
    /// earlier than the last ones.
    pub(super) fn paid_discard(&self, cost: &Cost, result: &ResultId) -> Option<u32> {
        let mut parts = cost.parts.iter().filter(|p| !matches!(p, CostPart::Mana(_)));
        match (parts.next(), parts.next()) {
            (
                Some(CostPart::Discard {
                    count: Expr::Constant(n),
                    random: false,
                    ..
                }),
                None,
            ) if self.reads_paid_objects(result) => u32::try_from(*n).ok(),
            _ => None,
        }
    }
    /// The cards `paid_discard` found, from the records since `mark_{id}`.
    pub(super) fn discards_paid(&mut self, id: usize, n: u32, result: &ResultId, out: &mut String) {
        writeln!(out, "let paid_objects_{id}: Vec<ObjectId> = if paid_{id} {{ let q: &dyn Query = g; let mut cards: Vec<ObjectId> = q.history().all()[mark_{id}..].iter().rev().filter_map(|r: &Record| match r.event {{ Event::Discarded {{ card, .. }} => Some(card), _ => None }}).take({n}).collect(); cards.reverse(); cards }} else {{ Vec::new() }};").unwrap();
        self.moved.insert(result.0, format!("paid_objects_{id}"));
        self.before
            .insert(result.0, format!("paid_objects_{id}.clone()"));
    }
    /// "Those that paid", where `may_pay` paid them: it does not say which.
    pub(super) fn objects_that_paid(&mut self, id: usize, result: &ResultId, out: &mut String) {
        if !self.reads_paid_objects(result) {
            return;
        }
        // A payment its own hole stands for (a cost part, a cost of moving cards
        // Cantrip has no call for): what paid it is known only once it is paid.
        let asked = format!("let paid_{id} = ");
        let start = out.rfind(&asked).or_else(|| out.rfind("g.end_sentence();"));
        let sentence = &out[start.unwrap_or(0)..];
        let feature = match sentence.contains("planned") {
            true => FOLLOWS_GAP,
            false => "objects that paid a cost at resolution",
        };
        let paid = planned::planned_value(feature, "Vec::<ObjectId>::new()", "g.objects_that_paid()");
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
                    let tests = filters
                        .iter()
                        .map(|f| self.filter(f, &Candidate::of("id")))
                        .collect::<Result<Vec<_>>>()?;
                    let together = format!(".together({})", self.one_for_each(&tests));
                    // "Choose up to two" of target modes: the card has modes, which
                    // one slot isn't (CR 700.2).
                    match lane_k::printed_otherwise(self.line_text(), "Choose up to") {
                        true => {
                            rule.planned.push((feature, together));
                            None
                        }
                        false => Some(together),
                    }
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
    /// "For each color, up to one target card of that color": the targets are one
    /// each of the filters, a target that fits several being any one of them. What
    /// a matching allows is a matroid, so taking what fits finds a largest set.
    fn one_for_each(&mut self, tests: &[String]) -> String {
        let name = format!("one_for_each_{}", self.fresh());
        let (n, bits) = (tests.len(), tests.iter().enumerate());
        let bits: String = bits
            .map(|(i, test)| format!("if {test} {{ bits |= 1 << {i}; }} "))
            .collect();
        writeln!(self.functions, "#[allow(unused_variables)] fn {name}_bits(q: &dyn Query, asker: &Asker, id: ObjectId) -> u32 {{ let mut bits = 0u32; {bits}bits }}\nfn {name}_fits(q: &dyn Query, choice: &Choice, set: &[ObjectId]) -> bool {{ let asker = &Asker::new(choice.source, choice.chooser); let mut used = vec![0u32]; for &id in set {{ let bits = {name}_bits(q, asker, id); let mut next: Vec<u32> = Vec::new(); for &taken in &used {{ for each in 0..{n} {{ let one = 1u32 << each; if bits & one != 0 && taken & one == 0 && !next.contains(&(taken | one)) {{ next.push(taken | one); }} }} }} used = next; }} !used.is_empty() }}\nfn {name}(q: &dyn Query, choice: &Choice, _: u32) -> Verdict {{ Verdict {{ complete: {name}_fits(q, choice, choice.selected), addable: Bits::of(choice.candidates.len(), |i| {{ let mut set = choice.selected.to_vec(); set.push(choice.candidates[i]); {name}_fits(q, choice, &set) }}) }} }}\nfn largest_{name}(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> {{ let mut kept: Vec<ObjectId> = Vec::new(); for &id in choice.candidates {{ kept.push(id); if !{name}_fits(q, choice, &kept) {{ kept.pop(); }} }} kept }}").unwrap();
        format!("cantrip_api::select::set_part(&({name} as SelectFn), &(largest_{name} as cantrip_api::select::LargestFn), 0, \"one for each\")")
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
        // Who chooses, asked as the slot is chosen: a `fn` the slot holds.
        const FEATURE: &str = "targets chosen by another player";
        const ASKED: &str = "q: &dyn Query, asker: &Asker";
        let (method, ret, body) = match chooser {
            PlayerRef::OneOf(players) => ("chosen_by_one_of", "Vec<PlayerId>", self.players(players)?),
            _ => match self.certain_player(chooser) {
                Ok(player) => ("chosen_by", "PlayerId", player),
                Err(_) => (
                    "chosen_by_one_of",
                    "Vec<PlayerId>",
                    format!("{}.into_iter().collect::<Vec<PlayerId>>()", self.player(chooser)?),
                ),
            },
        };
        let function = self.rule_fn(FEATURE, "target_chooser", ASKED, ret, &body);
        rule.planned.push((FEATURE.to_string(), format!(".{method}({function})")));
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
                format!(".between({n}, cantrip_api::TargetSpec::ANY_NUMBER)")
            }
            Count::Exactly(n) => {
                // A spell's slot counts only `TargetSpec::COUNTED`: its X, or else the
                // times it was kicked (CR 601.2c). Comet Storm's "another target for
                // each time this spell was kicked" beside its X, and Primitive Justice's
                // counts of each additional cost paid, are no such count.
                let code = read(n)?;
                if code.contains(".kicked") {
                    return Err(gap("a spell's target count of times kicked, beside an X or added to"));
                }
                if code.contains("additional_cost_paid") {
                    return Err(gap("a spell's target count of an additional cost paid any number of times"));
                }
                let n = planned::planned_value("target cardinality Exactly", "1u32", &code);
                format!(".between({n}, {n})")
            }
            Count::AtLeast(n) => {
                let n = planned::planned_value("target cardinality AtLeast", "1u32", &read(n)?);
                format!(".between({n}, cantrip_api::TargetSpec::ANY_NUMBER)")
            }
            other => return Err(gap(format!("target cardinality {}", form(other)))),
        })
    }
    /// What `ModalDef` has no word for yet.
    pub(super) fn modes_to_come(&mut self, selection: &ModeSelection, modal: String) -> Result<String> {
        let mut planned: Vec<(&str, String)> = Vec::new();
        // Modes each open only under a requirement are told apart by it
        // (`ModalDef::allowed`), not by a target.
        let flat = lane_p::flat_modes(selection);
        let told_by_requirements =
            flat.as_ref().unwrap_or(&selection.modes).iter().all(|mode| mode.requires.is_some());
        if selection.implied && !told_by_requirements {
            planned.push((
                "modes implied by the target chosen",
                ".implied_by_target()".to_string(),
            ));
        }
        // The combat before this one of that player's: from its beginning, while
        // the steps are combat's, a later combat of the turn being another.
        if let Some(ModeHistory::LastCombat(player)) = &selection.unchosen {
            let player = self.certain_player(player)?;
            return Ok(format!(
                "{modal}.allowed(|q: &dyn Query, asker: &Asker, mode: u8| {{ let you = {player}; let records = q.history().all(); let combat = |r: &Record| matches!(r.step, Step::BeginCombat | Step::DeclareAttackers | Step::DeclareBlockers | Step::CombatDamage | Step::EndOfCombat); let began = |r: &Record| matches!(r.event, Event::StepBegan {{ step: Step::BeginCombat }}); let starts: Vec<usize> = records.iter().enumerate().filter(|(_, r)| r.active == you && began(r)).map(|(at, _)| at).collect(); let at = starts.len().saturating_sub(2); starts.get(at).filter(|_| starts.len() >= 2).map_or(true, |&start| !records[start + 1..].iter().take_while(|r| combat(r) && !began(r)).any(|r| matches!(r.event, Event::ModesChosen {{ source, modes, .. }} if source == asker.source && modes & (1 << mode) != 0))) }})"
            ));
        }
        if selection.distinct_targets {
            planned.push((
                "modes with distinct targets",
                ".distinct_targets()".to_string(),
            ));
        }
        if let Some(chooser) = &selection.chooser {
            const FEATURE: &str = "modes chosen by another player";
            let players = self.players(chooser)?;
            let params = "q: &dyn Query, asker: &Asker";
            let function = self.rule_fn(FEATURE, "mode_choosers", params, "Vec<PlayerId>", &players);
            planned.push((FEATURE, format!(".chosen_by({function})")));
        }
        if selection.random {
            planned.push(("modes chosen at random", ".at_random()".to_string()));
        }
        if let Some(budget) = &selection.budget {
            let budget = self.number(budget)?.constant(u32::MAX.into(), "u32")?;
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
            // "Choose a player or planeswalker that opponent is attacking": among what
            // that player's attacking creatures attack (CR 506.2, 508.1b), asked only
            // when there is more than one.
            Operation::ChooseDefender {
                actor: PlayerSet::One(chooser),
                attacker,
            } => {
                let chooser = self.fetch(self.certain_player(chooser)?);
                let attacker = self.fetch(self.certain_player(attacker)?);
                writeln!(out, "let chooser_{id} = {chooser}; let attacker_{id} = {attacker}; let candidates_{id}: Vec<Target> = {{ let q: &dyn Query = g; let mut seen: Vec<Target> = Vec::new(); for a in q.state().attackers.iter() {{ let attacked = match a.target {{ Target::Player(_) => true, Target::Object(o) => q.chars(o).is(CardType::Planeswalker), _ => false }}; if attacked && q.controller(a.id) == attacker_{id} && !seen.contains(&a.target) {{ seen.push(a.target); }} }} seen }}; let defender_{id}: Option<Target> = {M}::combat::choose_among(g, chooser_{id}, asker.source, &candidates_{id})?;").unwrap();
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
                    .and_then(|n| n.quantity(Quantity::CounterPlacement));
                self.affected_object = saved;
                let amount = self.fetch(amount?);
                writeln!(out, "let objects_{id} = {}; let mut counters_{id}: Vec<(ObjectId, CounterKind, {counter_ty})> = Vec::new(); for &object in &objects_{id} {{ let amount = {amount}; counters_{id}.push((object, {kind}, amount)); }} g.add_counters_together(&counters_{id});", self.acted_on(objects)?, counter_ty = Quantity::CounterPlacement.kind()).unwrap();
                self.selections.insert(result.0, format!("objects_{id}"));
            }
            Operation::PutCountersCapped {
                amount,
                kind,
                object,
                at_most,
            } => {
                let kind = counter_kind(kind)?;
                let amount = self.number(amount)?;
                let cap = self.number(at_most)?;
                if !planned::switched_on("counter-placement-ceiling") {
                    return Err(gap("post-replacement counter placement ceiling"));
                }
                // The choice is up to X, even if replacements would reduce it.
                // Only the native placement authority applies the total ceiling.
                if amount.max > Quantity::CounterPlacement.max() {
                    return Err(gap("counter placement cannot represent the full chosen range"));
                }
                let amount = amount.choice_amount()?;
                let cap = cap.quantity(Quantity::CounterPlacement)?;
                writeln!(out, "let object_{id} = {}; let amount_{id} = {}; let cap_{id} = {}; if let Some(object) = object_{id}.filter(|&object| is_live(g, object)) {{ let chosen = {M}::choose::number(g, asker.controller, asker.source, 0, amount_{id}, \"Put counters\")? as {counter_ty}; g.add_counters_with_limit(object, {kind}, chosen, cap_{id}); }}", self.fetch(self.object(object)?), self.fetch(amount), self.fetch(cap), counter_ty = Quantity::CounterPlacement.kind()).unwrap();
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
            // "Attach an Equipment to it", of a token that may be more than one, and
            // "attach one of them to a Samurai you control": the attachments are
            // chosen, then the host among those there are (CR 701.3a).
            Operation::AttachChosen {
                actor,
                attachments,
                count,
                hosts,
            } if !matches!(hosts, ObjectSet::One(_)) => {
                let player = self.one_actor(actor, "operation AttachChosen")?;
                let count = self.offered_count(count);
                let count = self.fetch(self.select_count(&count)?);
                writeln!(out, "let player_{id} = {player}; let hosts_{id}: Vec<ObjectId> = {}; let eligible_{id} = if hosts_{id}.is_empty() {{ Vec::new() }} else {{ {} }}; let count_{id} = {count}; let chosen_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::Attach, eligible_{id}, {M}::select::Selection::of(count_{id}, &[]))?; let host_{id} = match (chosen_{id}.is_empty(), hosts_{id}.as_slice()) {{ (true, _) | (_, []) => None, (_, [one]) => Some(*one), _ => {M}::choose::card(g, player_{id}, asker.source, Purpose::Attach, hosts_{id}.clone())? }}; if let Some(host) = host_{id} {{ cantrip_api::instruction(g, |g| {{ for &attachment in &chosen_{id} {{ g.attach(attachment, host); }} }}); }}", self.fetch(self.objects(hosts)?), self.acted_on(attachments)?).unwrap();
                self.selections.insert(result.0, format!("chosen_{id}"));
                self.open_guard(&format!("chosen_{id}"), out);
            }
            Operation::FightEach { fighters, others } => {
                writeln!(out, "let fighters_{id} = {}; let others_{id} = {}; for &first in &fighters_{id} {{ for &second in &others_{id} {{ g.fight(first, second); }} }}", self.fetch(self.objects(fighters)?), self.fetch(self.objects(others)?)).unwrap();
            }
            Operation::FlipCoins { actor, count } => {
                let player = self.one_actor(actor, "operation FlipCoins")?;
                let count = self.number(count)?.amount(u32::MAX.into(), "u32")?;
                let call = self.flip_call()?;
                writeln!(out, "let player_{id} = {player}; let count_{id} = {}; let mut done_{id}: i64 = 0; for _ in 0..count_{id} {{ if g.{call}(player_{id}) {{ done_{id} += 1; }} }}", self.fetch(count)).unwrap();
                self.performed.insert(result.0, format!("done_{id}"));
                self.coins = Some((
                    format!("done_{id}"),
                    format!("(i64::from(count_{id}) - done_{id})"),
                ));
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
    pub(super) fn pay_any_amount(
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
                // What may be voted for is read as it resolves and handed to the
                // vote as `Target`s, a player or an object; the ballot is data.
                let none = "Vec::<Target>::new()".to_string();
                let players_as = |code: String| format!("({code}).into_iter().map(Target::Player).collect::<Vec<Target>>()");
                let objects_as = |code: String| format!("({code}).into_iter().map(Target::Object).collect::<Vec<Target>>()");
                let (ballot, candidates) = match ballot {
                    Ballot::Words(words) => {
                        let words: Vec<String> = words.iter().map(|w| format!("{w:?}")).collect();
                        (format!("{M}::vote::Ballot::Words(&[{}])", words.join(", ")), none)
                    }
                    Ballot::Players(players) => (
                        format!("{M}::vote::Ballot::Players"),
                        players_as(self.fetch(self.players(players)?)),
                    ),
                    Ballot::Objects(objects) => (
                        format!("{M}::vote::Ballot::Objects"),
                        objects_as(self.fetch(self.objects(objects)?)),
                    ),
                    Ballot::UpToOne(objects) => (
                        format!("{M}::vote::Ballot::UpToOne"),
                        objects_as(self.fetch(self.objects(objects)?)),
                    ),
                    Ballot::GraveyardCards(filter) => {
                        let test = self.filter(filter, &Candidate::of("id"))?;
                        (format!("{M}::vote::Ballot::GraveyardCards(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})"), none)
                    }
                };
                writeln!(out, "let voters_{id}: Vec<PlayerId> = {voters};").unwrap();
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::vote::ballot(g, asker.source, &voters_{id}, {ballot}, &{candidates}, {secret})?"
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
                // A characteristic change on a spell is an ordinary layered
                // effect. Cantrip carries it onto the permanent (CR 400.7a).
                // Cost, entry and casting changes still need their own API.
                if let SpellSubject::One(spell) = subject {
                    if changes.iter().all(|change| matches!(change, Change::Colors { mode: ChangeMode::Set, .. })) {
                        let spell = self.fetch(self.object(spell)?);
                        let mut effects = Vec::new();
                        for change in changes {
                            for kind in self.effect_kinds(change)? {
                                effects.push(format!("let kind = {{ let q: &dyn Query = g; {kind} }}; g.add_effect(asker.source, spell, kind, Duration::Indefinite);"));
                            }
                        }
                        writeln!(out, "if let Some(spell) = {spell} {{ cantrip_api::instruction(g, |g| {{ {} }}); }}", effects.join(" ")).unwrap();
                        return Ok(());
                    }
                }
                let changes = changes
                    .iter()
                    .map(|change| self.spell_change(change))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                match subject {
                    SpellSubject::One(spell) => {
                        let spell = self.fetch(self.object(spell)?);
                        // The changes are read as it resolves ("the color of your choice").
                        let changes = self.fetch(format!("[{changes}]"));
                        self.plan_code(&feature, &format!("if let Some(spell) = {spell} {{ let changes = {changes}; {M}::spell_changes::change(g, asker, spell, &changes); }}"), out);
                    }
                    SpellSubject::Next {
                        spells,
                        player,
                        this_turn,
                        from_hand,
                    } => {
                        let test = self.filter(spells, &Candidate::of("id"))?;
                        let player = self.fetch(self.certain_player(player)?);
                        let changes = self.fetch(format!("[{changes}]"));
                        self.plan_code(&feature, &format!("{{ let changes = {changes}; {M}::spell_changes::next_spell(g, asker, {player}, |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, NextSpell {{ this_turn: {this_turn}, from_hand: {from_hand} }}, &changes); }}"), out);
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
                // The zones exchanged are `ZoneKind`s.
                let kind = format!("{kind:?}");
                let kind = match kind.strip_prefix("Zones(").and_then(|k| k.strip_suffix(')')).and_then(|k| k.split_once(", ")) {
                    Some((a, b)) => format!("Zones(ZoneKind::{a}, ZoneKind::{b})"),
                    None => kind,
                };
                self.plan_code(&feature, &format!("{M}::exchange::exchange(g, asker.source, Exchange::{kind}, {first}, {second}, {until})"), out);
            }
            Operation::CopyOnStack {
                actor,
                object,
                times,
                targets,
                effects: _,
                card: None,
            } => {
                let feature = match targets {
                    NewTargets::Each(_) | NewTargets::EachLegal => {
                        "a copy of a spell for each object it could target".to_string()
                    }
                    NewTargets::Random(_) => "new targets chosen at random".to_string(),
                    _ => feature,
                };
                let copier = self.one_actor(actor, &feature)?;
                let times = self.fetch(self.number(times)?.amount(u32::MAX.into(), "u32")?);
                let (targets, among) = self.new_targets(targets)?;
                // "Copy that ability" (Agrus Kos): an ability on the stack is no
                // object; Cantrip names it by its serial, as `Target::Ability`.
                if matches!(object, ObjectRef::EventStackObject)
                    && self.frame.stack_object.is_none()
                    && self.frame.activated.is_some()
                {
                    let ability = self.carried_ability()?;
                    self.plan_code(&feature, &format!("{M}::copy::ability_on_stack(g, {copier}, {ability}, {times}, {targets}, &{among})?;"), out);
                } else {
                    let object = self.fetch(self.object(object)?);
                    self.plan_code(&feature, &format!("if let Some(spell) = {object} {{ {M}::copy::on_stack(g, {copier}, spell, {times}, {targets}, &{among})?; }}"), out);
                }
            }
            // The change of control comes first, so that "you" in the spell is its
            // new controller as the targets are chosen again.
            Operation::TakeSpell {
                actor,
                object,
                targets: NewTargets::May,
            } => {
                let player = self.one_actor(actor, &feature)?;
                let object = self.fetch(self.object(object)?);
                writeln!(out, "let player_{id} = {player}; if let Some(spell) = {object} {{ g.change_controller(spell, player_{id}); g.choose_new_targets(spell, player_{id})?; }}").unwrap();
            }
            Operation::TakeSpell {
                actor,
                object,
                targets,
            } => {
                let feature = match targets {
                    NewTargets::Random(_) => "new targets chosen at random".to_string(),
                    _ => feature,
                };
                let player = self.one_actor(actor, &feature)?;
                let object = self.fetch(self.object(object)?);
                let (targets, among) = self.new_targets(targets)?;
                self.plan_code(&feature, &format!("if let Some(spell) = {object} {{ g.change_controller(spell, {player}); {M}::copy::retarget(g, {player}, spell, {targets}, &{among})?; }}"), out);
            }
            Operation::Retarget {
                actor,
                object,
                targets,
            } => {
                let feature = match targets {
                    NewTargets::ToPlayers(_) => "a spell's target changed to a player".to_string(),
                    NewTargets::Random(_) => "new targets chosen at random".to_string(),
                    _ => feature,
                };
                let player = self.one_actor(actor, &feature)?;
                let object = self.fetch(self.object(object)?);
                let (targets, among) = self.new_targets(targets)?;
                self.plan_code(&feature, &format!("if let Some(spell) = {object} {{ {M}::copy::retarget(g, {player}, spell, {targets}, &{among})?; }}"), out);
            }
            // The second of "copy each of those spells twice": the first put them all
            // on the stack.
            Operation::CopyEach { .. } if self.notes.copied_twice => {
                self.notes.copied_twice = false;
            }
            // "In any order" (Display of Power's ruling): the copier says which next,
            // and each copy goes on the stack as it is made. Twice over is one pool,
            // since the two copies of a spell need not be stacked apart.
            Operation::CopyEach {
                actor,
                objects,
                targets: targets @ (NewTargets::May | NewTargets::Same),
            } => {
                // "Copy all other activated and triggered abilities you control"
                // (Ulalek, Fused Atrocity): copies made in the order the copier
                // chooses, of stack items an object doesn't name apart (one source's
                // several abilities); `choose` picks among objects only.
                let asked = format!("{objects:?}");
                if asked.contains("OnStack(Activated)") || asked.contains("OnStack(Triggered)") {
                    return Err(gap("copies of each of several abilities on the stack"));
                }
                let player = self.one_actor(actor, &feature)?;
                let test = self.filter(objects, &Candidate::of("id"))?;
                let spells = self.fetch(format!("q.state().stack.iter().filter(|i| i.kind == StackKind::Spell).map(|i| i.object).filter(|&id| {test}).collect::<Vec<ObjectId>>()"));
                let new_targets = matches!(targets, NewTargets::May);
                let twice = matches!(
                    self.after.first(),
                    Some(Instruction::Effect { operation, .. })
                        if format!("{operation:?}") == format!("{op:?}")
                );
                self.notes.copied_twice = twice;
                let doubled = if twice {
                    format!("open_{id}.extend(open_{id}.clone()); ")
                } else {
                    String::new()
                };
                writeln!(out, "let player_{id} = {player}; let mut open_{id}: Vec<ObjectId> = {spells}; {doubled}while let Some(&first) = open_{id}.first() {{ let spell = if open_{id}.iter().all(|&o| o == first) {{ Some(first) }} else {{ let mut distinct = open_{id}.clone(); distinct.sort(); distinct.dedup(); {M}::choose::card(g, player_{id}, asker.source, Purpose::CopyWhich, distinct)? }}; let Some(spell) = spell else {{ break; }}; if let Some(at) = open_{id}.iter().position(|&o| o == spell) {{ open_{id}.remove(at); }} g.copy_spell(spell, player_{id}, {new_targets})?; }}").unwrap();
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
                let (targets, among) = self.new_targets(targets)?;
                let call = if matches!(op, Operation::CopyEach { .. }) {
                    "on_stack_each"
                } else {
                    "retarget_each"
                };
                self.plan_code(&feature, &format!("{M}::copy::{call}(g, {player}, |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, {targets}, &{among})?"), out);
            }
            // Numbering objects and naming each player's most common creature
            // type note one value per object or player: the list they are about
            // is the call's, and what is noted is kept the way remembered values
            // are.
            Operation::Note(NoteKind::Numbered(objects)) => {
                let objects = self.fetch(self.objects(objects)?);
                self.plan_code(&feature, &format!("{M}::notes::number_each(g, asker, &{objects})"), out);
            }
            Operation::Note(NoteKind::MostCommonCreatureType(players)) => {
                let players = self.fetch(self.players(players)?);
                self.plan_code(&feature, &format!("{M}::notes::most_common_creature_type(g, asker, &{players})"), out);
            }
            Operation::Note(kind) => {
                // Kept by the ability that notes it, as a chosen name is
                // (`remember_chosen_name`), for the abilities linked to it: the
                // game keeps data, never the `Asker` that asked.
                let by = match self.stack_item {
                    true => "g.linked(item)",
                    false => "g.linked_to(asker)",
                };
                // Objects or players are a list read as the call runs.
                let listed = match kind {
                    NoteKind::Numbered(objects) => {
                        Some(("note_numbered", self.fetch(self.objects(objects)?)))
                    }
                    NoteKind::MostCommonCreatureType(players) => Some((
                        "note_most_common_creature_type",
                        self.fetch(self.players(players)?),
                    )),
                    _ => None,
                };
                if let Some((call, list)) = listed {
                    self.plan_code(
                        &feature,
                        &format!("{{ let by = {by}; let listed = {list}; {M}::notes::{call}(g, by, &listed); }}"),
                        out,
                    );
                    return Ok(());
                }
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
                    NoteKind::Numbered(_) | NoteKind::MostCommonCreatureType(_) => {
                        unreachable!("noted above")
                    }
                };
                self.plan_code(
                    &feature,
                    &format!("{{ let by = {by}; {M}::notes::note(g, by, {note}); }}"),
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
                let amount = self.fetch(self.number(amount)?.quantity(Quantity::Damage)?);
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
                let amount = self.fetch(self.number(amount)?.quantity(Quantity::Damage)?);
                match rider {
                    DamageRider::DividedEvenly(rounding) => {
                        let share = match rounding {
                            Rounding::Down => "amount_{id} / shares",
                            Rounding::Up => "amount_{id}.div_ceil(shares)",
                        }
                        .replace("{id}", &id.to_string());
                        writeln!(out, "let source_{id} = {source}; let targets_{id} = {targets}; let amount_{id}: {damage_ty} = {amount}; if let Some(source) = source_{id} {{ let shares = (targets_{id}.len() as {damage_ty}).max(1); let share = {share}; let batch: Vec<(ObjectId, Target, {damage_ty})> = targets_{id}.iter().map(|&t| (source, t, share)).collect(); g.damage_together(&batch); }}", damage_ty = Quantity::Damage.kind()).unwrap();
                    }
                    // What isn't lethal (120.10) goes to the player at the same
                    // time: the creature is never dealt the excess.
                    DamageRider::ExcessTo(player) => {
                        let player = self.fetch(self.certain_player(player)?);
                        writeln!(out, "let source_{id} = {source}; let targets_{id} = {targets}; let amount_{id}: {damage_ty} = {amount}; let player_{id} = {player}; if let (Some(source), Some(&target)) = (source_{id}, targets_{id}.first()) {{ let lethal_{id} = {{ let q: &dyn Query = g; target.object().map_or(amount_{id}, |o: ObjectId| {{ let chars = q.chars(o); let left = {lethal_left}; match q.chars(source).has(Keyword::Deathtouch) {{ true => left.min(1), false => left }} }}) }}; let dealt_{id} = amount_{id}.min(lethal_{id}); g.damage_together(&[(source, target, dealt_{id}), (source, Target::Player(player_{id}), amount_{id} - dealt_{id})]); }}", damage_ty = Quantity::Damage.kind(), lethal_left = if planned::switched_on(quantity::FEATURE) { "i64::from(chars.toughness.unwrap_or(0).max(0)).saturating_sub(i64::from(q.state().obj(o).damage)).max(0)" } else if planned::switched_on(quantity::BOUNDED_32) { "(chars.toughness.unwrap_or(0).max(0) as u32).saturating_sub(q.state().obj(o).damage)" } else { "u16::try_from(chars.toughness.unwrap_or(0).max(0)).unwrap_or(0).saturating_sub(q.state().obj(o).damage)" }).unwrap();
                    }
                    DamageRider::Prevented(prevented) => {
                        let prevented =
                            self.fetch(self.number(prevented)?.quantity(Quantity::Prevention)?);
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
            // Each of those players gets one life total back: a player left out
            // gets their own, so every total is handed to someone, and the
            // changes are one instruction (CR 119.5).
            Operation::RedistributeLife(players) => {
                let players = self.fetch(self.players(players)?);
                writeln!(out, "let players_{id}: Vec<PlayerId> = {players}; let totals_{id}: Vec<i32> = players_{id}.iter().map(|&p| g.state().player(p).life).collect(); let mut open_{id} = players_{id}.clone(); let mut sets_{id}: Vec<(PlayerId, i32)> = Vec::new(); for &total in &totals_{id} {{ let who = match open_{id}.as_slice() {{ [one] => Some(*one), _ => {M}::choose::player(g, asker.controller, asker.source, open_{id}.clone(), \"GetLifeTotal\", total.clamp(0, {prompt_max}) as {prompt_ty})? }}; if let Some(who) = who {{ open_{id}.retain(|&p| p != who); sets_{id}.push((who, total)); }} }} cantrip_api::instruction(g, |g| {{ for &(who, total) in &sets_{id} {{ g.set_life_total(who, total); }} }});", prompt_ty = quantity::count_kind(), prompt_max = if planned::switched_on(quantity::BOUNDED_32) { "i32::MAX" } else { "65535" }).unwrap();
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
            // "Sacrifices an artifact and a land of their choice": a permanent for
            // each, all together.
            Operation::Together(operations) if sacrifice_slots(operations).is_some() => {
                let (player, slots) = sacrifice_slots(operations).expect("just seen");
                let player = self.fetch(self.certain_player(player)?);
                self.sacrifice_slots(id, &player, &slots, out)?;
                writeln!(out, "let sacrificed_{id} = g.sacrifice(player_{id}, &chosen_{id});").unwrap();
                self.moved.insert(result.0, format!("sacrificed_{id}"));
                self.left_behind(result, effects::gone(&format!("chosen_{id}")));
            }
            // "Simultaneously untap all tapped creatures and tap all untapped
            // creatures": who is which is read first, and it is one instruction.
            Operation::Together(operations)
                if operations
                    .iter()
                    .all(|operation| matches!(operation, Operation::Tap { .. })) =>
            {
                let (mut tapping, mut untapping) = (Vec::new(), Vec::new());
                for (at, operation) in operations.iter().enumerate() {
                    let Operation::Tap { objects, tapped } = operation else {
                        return Err(gap(feature));
                    };
                    writeln!(out, "let turned_{id}_{at}: Vec<ObjectId> = ({}).into_iter().filter(|&o| g.state().obj(o).tapped != {tapped}).collect();", self.acted_on(objects)?).unwrap();
                    let group = if *tapped { &mut tapping } else { &mut untapping };
                    group.push(format!("turned_{id}_{at}"));
                }
                let mut turn = String::new();
                for list in tapping {
                    write!(turn, "g.tap_together(&{list}); ").unwrap();
                }
                for list in untapping {
                    write!(turn, "for &object in &{list} {{ g.untap(object); }} ").unwrap();
                }
                writeln!(out, "cantrip_api::instruction(g, |g| {{ {turn}}});").unwrap();
            }
            // "That player simultaneously sacrifices the artifact and returns the
            // artifact card", with both targets still legal.
            Operation::Together(operations) if self.welds(operations) => {
                let [Operation::Sacrifice {
                    players: PlayerSet::One(player),
                    objects: ObjectSet::One(ObjectRef::Target { slot: gone, index: gone_at }),
                    ..
                }, Operation::Move {
                    objects: ObjectSet::One(ObjectRef::Target { slot: back, index: back_at }),
                    ..
                }] = operations.as_slice()
                else {
                    return Err(gap(feature));
                };
                let (gone, back) = (self.slot(gone)?, self.slot(back)?);
                let player = self.fetch(self.certain_player(player)?);
                writeln!(out, "let player_{id} = {player}; let gone_{id} = t_{gone}_raw.get({gone_at}).copied().filter(|t| t_{gone}.contains(t)).and_then(|t: Target| t.object()); let back_{id} = t_{back}_raw.get({back_at}).copied().filter(|t| t_{back}.contains(t)).and_then(|t: Target| t.object()); if let (Some(gone), Some(back)) = (gone_{id}, back_{id}) {{ g.sacrifice_and_return(player_{id}, &[gone], &[(back, player_{id})])?; }}").unwrap();
            }
            // Two calls are two batches of what dies, and no call takes both.
            Operation::Together(operations)
                if matches!(
                    operations.as_slice(),
                    [Operation::Destroy { regeneration: false, .. }, Operation::Destroy { regeneration: true, .. }]
                ) =>
            {
                return Err(gap("destroyed together, some that can't be regenerated"));
            }
            // "Choose a land type and a basic land type": choices change nothing, so
            // made one after the other they are made together.
            Operation::Together(operations)
                if operations
                    .iter()
                    .all(|operation| matches!(operation, Operation::ChooseValue { terms, .. } if terms.ordinary_count().is_some())) =>
            {
                for operation in operations {
                    self.operation(operation, result, out)?;
                }
            }
            Operation::Together(operations) => {
                let mut parts = String::new();
                let before = self.gaps.len();
                for operation in operations {
                    self.operation(operation, result, &mut parts)?;
                }
                // A part that can't be written yet already names what the file waits
                // on; doing the parts at once is asked of them only once they can be.
                if self.gaps.len() > before || parts.contains("planned") {
                    let follows = planned::requirement(FOLLOWS_GAP);
                    writeln!(out, "planned!({follows:?} => g.begin_simultaneous());").unwrap();
                    out.push_str(&parts);
                    writeln!(out, "planned!({follows:?} => g.end_simultaneous());").unwrap();
                } else if operations
                    .iter()
                    .all(|operation| matches!(operation, Operation::DealDamage { .. }))
                    && parts.matches("g.damage_together(&batch);").count() == operations.len()
                {
                    // "Each of those creatures deals damage equal to its toughness to the
                    // other": every amount read first, then one event of damage (CR 120.3).
                    writeln!(out, "let mut together_{id}: Vec<(ObjectId, Target, {damage_ty})> = Vec::new();", damage_ty = Quantity::Damage.kind()).unwrap();
                    out.push_str(&parts.replace(
                        "g.damage_together(&batch);",
                        &format!("together_{id}.extend(batch);"),
                    ));
                    writeln!(out, "g.damage_together(&together_{id});").unwrap();
                } else {
                    self.plan_code(&feature, "g.begin_simultaneous()", out);
                    out.push_str(&parts);
                    self.plan_code(&feature, "g.end_simultaneous()", out);
                }
            }
            // The rule is data; the objects and players it names as it resolves
            // are the call's, as `effect_on_objects` takes them.
            Operation::GameEffect { until, effect } => {
                let until = self.duration(until)?;
                let (mut objects, mut players) = (None, None);
                let rule = self.term_apart(effect, &mut objects, &mut players);
                let objects = objects.unwrap_or_else(|| "Vec::<ObjectId>::new()".to_string());
                let players = players.unwrap_or_else(|| "Vec::<PlayerId>::new()".to_string());
                self.plan_code(
                    &feature,
                    &format!("{M}::game_rules::for_a_time(g, asker, {until}, &{objects}, &{players}, {rule})"),
                    out,
                );
            }
            Operation::ChangeCombat { change } => {
                // The objects it is about are the call's, read as it resolves
                // (`&[ObjectId]`, as `exile_all` takes them); the change is data.
                let none = "Vec::<ObjectId>::new()".to_string();
                let (objects, change) = match change {
                    CombatChange::Reselect {
                        objects,
                        players_only,
                        optional,
                    } => (self.fetch(self.objects(objects)?), format!("CombatChange::Reselect {{ players_only: {players_only}, optional: {optional} }}")),
                    CombatChange::AssignAttack { objects, to } => {
                        let to = self.operand_code(to);
                        (self.fetch(self.objects(objects)?), format!("CombatChange::AssignAttack {{ to: {to} }}"))
                    }
                    CombatChange::Block { blocker, attackers } => (self.fetch(self.objects(attackers)?), format!("CombatChange::Block {{ blocker: {} }}", self.fetch(self.object(blocker)?))),
                    CombatChange::SwapBlocks {
                        first,
                        second,
                        attacking,
                    } => (none, format!("CombatChange::SwapBlocks {{ first: {}, second: {}, attacking: {attacking} }}", self.fetch(self.object(first)?), self.fetch(self.object(second)?))),
                    CombatChange::OnlyAttackers { objects, during } => (self.fetch(self.objects(objects)?), format!("CombatChange::OnlyAttackers {{ added_phase: {} }}", during.is_some())),
                    CombatChange::OnlyBlockers(objects) => (self.fetch(self.objects(objects)?), "CombatChange::OnlyBlockers".to_string()),
                };
                self.plan_code(
                    &feature,
                    &format!("{M}::combat::change(g, asker, &{objects}, {change})?"),
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
            // "An opponent creates two Treasure tokens and they scry 2": you choose
            // which (its ruling), and in `body` the doer is "you". "Another player
            // of your choice may gain control of it" (Stardew Valley): the one you
            // chose decides.
            Operation::PlayersDo {
                players,
                any_one: true,
                optional,
                once_each_turn: false,
                body,
                otherwise,
            } if otherwise.is_empty()
                && body.iter().all(|step| matches!(step, Operand::Program(_)))
                && (!*optional || you_only_gains_control(body)) =>
            {
                let optional = *optional;
                let among = self.fetch(self.players(players)?);
                writeln!(out, "let among_{id}: Vec<PlayerId> = {among}; let Some(doer_{id}) = (if among_{id}.len() > 1 {{ {M}::choose::player(g, asker.controller, asker.source, among_{id}, \"Choose\", 0)? }} else {{ among_{id}.first().copied() }}) else {{ return Ok(()); }}; {{ let asker = &Asker::new(asker.source, doer_{id});").unwrap();
                for step in body {
                    let Operand::Program(program) = step else {
                        unreachable!("just matched");
                    };
                    if optional {
                        let mut offered = program.clone();
                        offered.instructions = vec![Instruction::Optional {
                            decider: PlayerRef::AbilityController,
                            body: program.clone(),
                        }];
                        self.block(&offered, out, true)?;
                    } else {
                        self.block(program, out, true)?;
                    }
                }
                writeln!(out, "}}").unwrap();
            }
            // "Repeat the following process for each opponent in turn order ...
            // That player may pay": the walk names the player, who is "that
            // player" in the body, and "you" stays you (Protection Racket).
            Operation::PlayersDo {
                players: PlayerSet::One(PlayerRef::Bound(binding)),
                any_one: false,
                optional: false,
                once_each_turn: false,
                body,
                otherwise,
            } if otherwise.is_empty()
                && self.bound_players.contains(&binding.0)
                && body.iter().all(|step| matches!(step, Operand::Program(_))) =>
            {
                for step in body {
                    let Operand::Program(program) = step else {
                        unreachable!("just matched");
                    };
                    self.block(program, out, true)?;
                }
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
            // "Separates them into two piles" (CR 700.3a): the separator puts each
            // pile together from what is left, so any of them may be empty (CR
            // 700.3d). The piles are kept as rows for what chooses among them and
            // what reads "the chosen pile"; face-down piles are shown to no one else.
            Operation::Piles {
                actor,
                objects,
                count: Some(Expr::Constant(n)),
                names,
                chooser,
            } if *n >= 2
                && (names.is_empty() || names.iter().all(|name| name == "face down"))
                && (self.piles_walk.is_some() || chooser.is_none()) =>
            {
                let player = self.one_actor(actor, &feature)?;
                let objects = self.fetch(self.objects(objects)?);
                let choosers = match chooser {
                    Some(chooser) => self.fetch(self.players(chooser)?),
                    None => "Vec::<PlayerId>::new()".to_string(),
                };
                writeln!(out, "let separator_{id} = {player}; let mut rest_{id}: Vec<ObjectId> = {objects}; let mut piles_{id}: Vec<Vec<ObjectId>> = Vec::new(); for _ in 1..{n} {{ let (first, second) = {M}::choose::separate(g, Some(separator_{id}), asker.source, rest_{id})?; piles_{id}.push(first); rest_{id} = second; }} piles_{id}.push(rest_{id}); let choosers_{id}: Vec<PlayerId> = {choosers};").unwrap();
                match self.piles_walk.clone() {
                    Some(rows) => writeln!(out, "{rows}.push((separator_{id}, piles_{id}, None, choosers_{id}));").unwrap(),
                    None => {
                        writeln!(out, "let mut rows_{id}: Vec<(PlayerId, Vec<Vec<ObjectId>>, Option<usize>, Vec<PlayerId>)> = vec![(separator_{id}, piles_{id}, None, choosers_{id})];").unwrap();
                        self.piles = Some(format!("rows_{id}"));
                    }
                }
                self.pile_shape = (*n, !names.is_empty());
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
                let feature = match labeled_piles(names) {
                    true => LABELED_PILES.to_string(),
                    false => feature,
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
            // "For each opponent, you choose one of their piles", "each player
            // chooses one of their piles at random", "exile one of those piles": a
            // pile of each row of the separators named. A face-down pile shows the
            // chooser only how many cards it holds.
            Operation::ChoosePile {
                actor: PlayerSet::One(chooser),
                from,
                random,
                names,
                object: None,
                then: then @ (None | Some(Destination::Exile)),
            } if names.is_empty() && self.piles.is_some() => {
                let rows = self.piles.clone().unwrap_or_default();
                let chooser = self.fetch(self.certain_player(chooser)?);
                let from = match from {
                    Some(from) => format!("Some({})", self.fetch(self.players(from)?)),
                    None => "None::<Vec<PlayerId>>".to_string(),
                };
                let (count, face_down) = self.pile_shape;
                let pick = if *random {
                    "g.choose_at_random(row.1.len())".to_string()
                } else if face_down {
                    format!("usize::from({M}::choose::one_of(g, chooser_{id}, asker.source, row.1.iter().map(|pile| Label::FaceDownPile {{ count: pile.len() as u16 }}).collect())?)")
                } else if count == 2 {
                    format!("{{ let (chosen, _) = {M}::choose::pile(g, chooser_{id}, asker.source, row.1[0].clone(), row.1[1].clone())?; usize::from(chosen != row.1[0]) }}")
                } else {
                    return Err(gap(feature));
                };
                let chooser = if *random { String::new() } else { format!("let chooser_{id} = {chooser}; ") };
                writeln!(out, "{chooser}let from_{id} = {from}; let mut chosen_{id}: Vec<ObjectId> = Vec::new(); for row in {rows}.iter_mut() {{ if from_{id}.as_ref().is_some_and(|from| !from.contains(&row.0)) {{ continue; }} let at = {pick}; row.2 = Some(at); chosen_{id}.extend(row.1[at].iter().copied()); }}").unwrap();
                if let Some(Destination::Exile) = then {
                    writeln!(out, "let moved_{id} = g.exile_all(&chosen_{id});").unwrap();
                    self.moved.insert(result.0, format!("moved_{id}"));
                }
                self.selections.insert(result.0, format!("chosen_{id}"));
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
                // "Exile the top four cards of your library in a face-down pile, then
                // exile the top four cards of your library in a face-up pile. An
                // opponent chooses one of those piles" (Abstract Performance): the
                // chooser sees the face-up pile's cards and how many the face-down one
                // holds (CR 700.3b-d); the chosen pile is this choice's result, the
                // other is "the other pile".
                if let (true, None, [(down, true), (up, false)] | [(up, false), (down, true)]) = (
                    names.is_empty() && self.piles.is_none() && from == "None" && !*random && object == "None",
                    then,
                    self.exile_piles_made.as_slice(),
                ) {
                    let (down, up) = (down.clone(), up.clone());
                    writeln!(out, "let chooser_{id} = {player}; let (chosen_{id}, other_{id}) = {M}::choose::face_down_or_face_up_pile(g, chooser_{id}, asker.source, {down}.clone(), {up}.clone())?;").unwrap();
                    self.selections.insert(result.0, format!("chosen_{id}"));
                    self.pile_not_chosen.insert(result.0, format!("other_{id}"));
                    return Ok(());
                }
                let then = then.as_ref().map(|d| form(d).to_string());
                let code = format!("{M}::choose::of_piles(g, {player}, asker.source, {from}, {random}, &{names:?}, {object}, {then:?})?");
                // "An opponent chooses one of those piles" of piles exiled by an
                // instruction with a hole of its own (Abstract Performance,
                // Hostile Negotiations, The Celestial Toymaker): the arm above
                // writes the choice once that instruction keeps its piles.
                if names.is_empty() && self.piles.is_none() && self.exiled_piles {
                    let follows = planned::requirement(FOLLOWS_GAP);
                    writeln!(out, "planned!({follows:?} => {code});").unwrap();
                    // What reads the chosen pile follows the same hole.
                    self.failed.insert(result.0);
                    return Ok(());
                }
                let feature = match labeled_piles(names) {
                    true => LABELED_PILES.to_string(),
                    false => feature,
                };
                self.plan_code(&feature, &code, out);
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
                let entry: Vec<String> = self.entry_operands(entry)?.iter().map(|o| self.operand_code(o)).collect();
                self.plan_code(
                    &feature,
                    &format!(
                        "{M}::tokens::populate(g, {player}, asker.source, &[{}])?",
                        entry.join(", ")
                    ),
                    out,
                );
            }
            // "Unless any player pays {3}": each is offered it in turn, and the
            // first who pays ends the offer.
            Operation::AnyPlayerMayPay {
                players,
                mana,
                unpaid,
                paid,
            } if generic_mana(mana).is_some()
                && matches!(
                    (unpaid.as_slice(), paid.as_slice()),
                    ([Operand::Program(_)], [Operand::Program(_)])
                ) =>
            {
                let price = generic_mana(mana).unwrap_or_default();
                let ([Operand::Program(unpaid)], [Operand::Program(paid)]) =
                    (unpaid.as_slice(), paid.as_slice())
                else {
                    return Err(gap(feature));
                };
                self.helpers.insert("pay_question");
                let players = self.fetch(self.players(players)?);
                writeln!(out, "let mut paid_{id} = false; for player in {players} {{ if g.may_pay(player, asker.source, asker.source, cost(\"{{{price}}}\"), &PAY_QUESTION)? {{ paid_{id} = true; break; }} }} if paid_{id} {{").unwrap();
                self.block(paid, out, true)?;
                writeln!(out, "}} else {{").unwrap();
                self.block(unpaid, out, true)?;
                writeln!(out, "}}").unwrap();
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
            // "You may pay {1} up to three times", "for each, lose 1 life unless you
            // pay {1}{W}": one payment, its number the payer's. A cost that isn't mana
            // goes one at a time, until they stop: which times they stopped at is
            // nothing the rest can tell.
            Operation::PayRepeatedly {
                player,
                cost,
                at_most,
            } if pays_by_the_time(cost) => {
                let count_ty = quantity::count_kind();
                let most = self.fetch(self.number(at_most)?.amount(quantity::x_max(), count_ty)?);
                if let [CostPart::Mana(symbols)] = cost.parts.as_slice() {
                    self.helpers.insert("pay_question");
                    let payer = self.fetch(self.certain_player(player)?);
                    let mana = mana(symbols)?;
                    writeln!(out, "let payer_{id} = {payer}; let most_{id}: {count_ty} = {most}; let bound_{id}: i64 = if most_{id} == 0 {{ 0 }} else {{ i64::from(g.may_pay_times(payer_{id}, asker.source, asker.source, {mana}, most_{id}, &PAY_QUESTION)?) }}; let done_{id}: i64 = bound_{id};").unwrap();
                } else {
                    let ask = self.ask_to_pay(id, player, cost)?;
                    writeln!(out, "let most_{id}: {count_ty} = {most}; let mut bound_{id}: i64 = 0; for _ in 0..most_{id} {{ {ask} if paid_{id} {{ bound_{id} += 1; }} else {{ break; }} }} let done_{id}: i64 = bound_{id};").unwrap();
                }
                self.performed.insert(result.0, format!("done_{id}"));
                self.values.insert(result.0, format!("bound_{id}"));
            }
            Operation::PayRepeatedly {
                player,
                cost,
                at_most,
            } => {
                let player = self.fetch(self.certain_player(player)?);
                let count_ty = quantity::count_kind();
                let most = self.fetch(self.number(at_most)?.amount(quantity::x_max(), count_ty)?);
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
            // "For each creature, its controller ... unless they pay {1}": each
            // player in turn pays as many times as they like, up to that many (CR
            // 118.12), and what they left unpaid is kept for the step that reads it.
            Operation::PlayersPayRepeatedly {
                players: PlayerSet::One(player),
                cost,
                at_most,
            } if self.unpaid_rows.contains(&result.0)
                && matches!(cost.parts.as_slice(), [CostPart::Mana(_)]) =>
            {
                self.helpers.insert("pay_question");
                let player = self.fetch(self.certain_player(player)?);
                let count_ty = quantity::count_kind();
                let most = self.fetch(self.number(at_most)?.amount(quantity::x_max(), count_ty)?);
                let one = self.cost(cost, super::lane_g::PAID)?;
                writeln!(out, "let payer_{id} = {player}; let most_{id}: {count_ty} = {most}; let paid_{id} = if most_{id} == 0 {{ 0 }} else {{ g.may_pay_times(payer_{id}, asker.source, asker.source, ({one}).mana, most_{id}, &PAY_QUESTION)? }}; unpaid_{}.extend(std::iter::repeat(payer_{id}).take(most_{id}.saturating_sub(paid_{id}) as usize));", result.0).unwrap();
            }
            Operation::PlayersPayRepeatedly {
                players,
                cost,
                at_most,
            } => {
                let players = self.fetch(self.players(players)?);
                let count_ty = quantity::count_kind();
                let most = self.fetch(self.number(at_most)?.amount(quantity::x_max(), count_ty)?);
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
            // The kinds are read once, as this resolves. "Choose none" of the one
            // object is the other option: the question names the kind.
            Operation::EachCounterKind {
                actor,
                among,
                options,
                optional,
            } if matches!(
                (options.as_slice(), optional),
                ([CounterStep::PutThatKind(_), CounterStep::RemoveThatKind(_)], false)
                    | ([CounterStep::Put(..), CounterStep::PutThatKind(_)], true)
            ) =>
            {
                let player = self.one_actor(actor, &feature)?;
                let among = self.fetch(self.objects(among)?);
                let offer = |kind: &str| format!("{M}::choose::cards(g, player_{id}, asker.source, Purpose::PutCounters({kind}, 1), Vec::from([object]), 1, true)?");
                let (object, step) = match options.as_slice() {
                    [CounterStep::PutThatKind(object), CounterStep::RemoveThatKind(_)] => {
                        let put = offer("kind");
                        (object, format!("if {put}.is_empty() {{ g.remove_counters(object, kind, 1); }} else {{ g.add_counters(object, kind, 1); }}"))
                    }
                    [CounterStep::Put(other, _), CounterStep::PutThatKind(object)] => {
                        let other = counter_kind(other)?;
                        let (put, plain) = (offer("kind"), offer(&other));
                        (object, format!("if kind != {other} && !{put}.is_empty() {{ g.add_counters(object, kind, 1); continue; }} if !{plain}.is_empty() {{ g.add_counters(object, {other}, 1); }}"))
                    }
                    _ => return Err(gap(feature)),
                };
                let object = self.fetch(self.object(object)?);
                writeln!(out, "let player_{id} = {player}; let among_{id}: Vec<ObjectId> = {among}; let kinds_{id}: Vec<CounterKind> = CounterKind::ALL.into_iter().filter(|&kind| among_{id}.iter().any(|&o| g.state().obj(o).counters(kind) > 0)).collect(); if let Some(object) = {object} {{ for kind in kinds_{id} {{ {step} }} }}").unwrap();
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
            // "Put up to three lore counters on it": how many is asked of each.
            Operation::PutCountersEach {
                amount,
                kind,
                objects,
                up_to: true,
            } => {
                let kind = counter_kind(kind)?;
                let choice = self.number(amount)?;
                let amount = if planned::switched_on(quantity::FEATURE) {
                    self.fetch(choice.choice_amount()?)
                } else { self.fetch(choice.quantity(Quantity::CounterPlacement)?) };
                writeln!(out, "let objects_{id} = {}; let most_{id} = {amount}; for &object in &objects_{id} {{ let many = {M}::choose::number(g, asker.controller, asker.source, 0, most_{id}.max(0) as u32, \"Put how many counters\")?; g.add_counters(object, {kind}, many as {counter_ty}); }}", self.fetch(self.objects(objects)?), counter_ty = Quantity::CounterPlacement.kind()).unwrap();
                self.selections.insert(result.0, format!("objects_{id}"));
            }
            // "Move any number of counters": from what, of what kind, onto what and
            // how many, until the player stops or there is nothing left to move.
            Operation::MoveCounters {
                actor: PlayerSet::One(player),
                from,
                to,
                kind,
                amount: MovedCounters::AnyNumber,
            } => {
                let player = self.fetch(self.certain_player(player)?);
                let from = self.fetch(self.objects(from)?);
                let to = self.fetch(self.objects(to)?);
                let (holds, read_kind) = match kind {
                    Some(kind) => {
                        let kind = counter_kind(kind)?;
                        (format!("g.state().obj(o).counters({kind}) > 0"), format!("Some({kind})"))
                    }
                    None => (
                        "CounterKind::ALL.into_iter().any(|kind| g.state().obj(o).counters(kind) > 0)".to_string(),
                        format!("g.choose_counter_kind(player_{id}, asker.source, holder)?"),
                    ),
                };
                writeln!(out, "let player_{id} = {player}; let from_{id}: Vec<ObjectId> = {from}; let to_{id}: Vec<ObjectId> = {to}; let mut moved_{id}: i64 = 0; loop {{ let holders: Vec<ObjectId> = from_{id}.iter().copied().filter(|&o| to_{id}.iter().any(|&t| t != o) && {holds}).collect(); let holder = match holders.as_slice() {{ [] => None, [one] => Some(*one), _ => {M}::choose::cards(g, player_{id}, asker.source, Purpose::Choose, holders.clone(), 1, true)?.first().copied() }}; let Some(holder) = holder else {{ break; }}; let Some(kind) = {read_kind} else {{ break; }}; let hosts: Vec<ObjectId> = to_{id}.iter().copied().filter(|&t| t != holder).collect(); let host = match hosts.as_slice() {{ [one] => Some(*one), _ => {M}::choose::cards(g, player_{id}, asker.source, Purpose::PutCounters(kind, 1), hosts.clone(), 1, true)?.first().copied() }}; let Some(host) = host else {{ break; }}; let most = g.state().obj(holder).counters(kind).max(0) as u32; let many = {M}::choose::number(g, player_{id}, asker.source, 0, most, \"Move how many counters\")?; if many == 0 {{ break; }} let done = g.move_counters(holder, host, kind, many as {counter_ty}); moved_{id} += i64::from(done); if done == 0 {{ break; }} }}", counter_ty = Quantity::StoredCounter.kind()).unwrap();
                self.performed.insert(result.0, format!("moved_{id}"));
            }
            // "Exiles cards from the top until they have exiled cards with total
            // mana value 5 or greater": one at a time, and a card a replacement
            // sends elsewhere is not counted. The move after it is done by then.
            Operation::RevealUntilGroup {
                library,
                constraints,
            } if self.exiled_as_it_goes(result) && total_mana_value_at_least(constraints).is_some() => {
                let least = total_mana_value_at_least(constraints).unwrap_or_default();
                let owner = self.fetch(self.certain_player(library)?);
                writeln!(out, "let owner_{id} = {owner}; let mut total_{id}: i64 = 0; let mut objects_{id}: Vec<ObjectId> = Vec::new(); while total_{id} < {least} {{ let Some(&top) = g.state().zone(Zone::library(owner_{id})).first() else {{ break; }}; let card = g.exile(top); if g.obj(card).zone.is(ZoneKind::Exile) {{ total_{id} += i64::from(g.mana_value(card)); objects_{id}.push(card); }} }}").unwrap();
                self.selections.insert(result.0, format!("objects_{id}"));
                self.notes.exiled_going.insert(result.0);
            }
            Operation::PutChosenCounters { objects, choice }
                if self.chosen_counters(id, objects, choice, result, out)? => {}
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
                        // What happens at the start of the added phase is an
                        // ability of the card's the engine triggers then.
                        let at_start = match at_start.as_deref() {
                            None => "None".to_string(),
                            Some(Operand::Program(body)) => {
                                let (gaps, functions) = (self.gaps.len(), self.functions.len());
                                let delayed = self.delayed.len();
                                match self.delayed_program(body) {
                                    Ok(ability) => format!("Some({ability})"),
                                    Err(_) => {
                                        self.gaps.truncate(gaps);
                                        self.functions.truncate(functions);
                                        self.delayed.truncate(delayed);
                                        format!("Some({})", quoted(&format!("{body:?}")))
                                    }
                                }
                            }
                            Some(other) => format!("Some({})", self.operand_code(other)),
                        };
                        self.plan_code(&feature, &format!("g.add_phases({player}, AddedPhases::{phases:?}, {count}, {at_start})"), out);
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
                let entry: Vec<String> = self.entry_operands(entry)?.iter().map(|o| self.operand_code(o)).collect();
                // "Onto the battlefield under your control": the player is the
                // destination's, as a move's is.
                let destination = self.destination_code(destination)?;
                self.plan_code(&feature, &format!("let destination = {destination}; if let Some(spell) = {spell} {{ {M}::counter::to(g, spell, {test}, destination, &[{}]); }}", entry.join(", ")), out);
            }
            // "An opponent gains control": the controller picks one, unasked if
            // there is only one. The new controller keeps it (CR 611.2), or has it
            // for as long as it says ("until end of turn").
            Operation::GiveControl {
                players,
                objects,
                until,
            } if matches!(players, PlayerSet::One(_) | PlayerSet::OpponentsOf(_)) => {
                let until = match until {
                    Some(until) => self.duration(until)?,
                    None => "Duration::Indefinite".to_string(),
                };
                let players = self.fetch(self.players(players)?);
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let players_{id}: Vec<PlayerId> = {players}; let objects_{id} = {objects}; let who_{id} = match players_{id}.as_slice() {{ [one] => Some(*one), _ => {M}::choose::player(g, asker.controller, asker.source, players_{id}.clone(), \"GainControl\", 0)? }}; if let Some(who) = who_{id} {{ cantrip_api::instruction(g, |g| {{ for &object in &objects_{id} {{ g.gain_control(asker.source, object, who, {until}); }} }}); }}").unwrap();
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
                    let selected=matches!(on,ObjectSet::Selected(_))
                        || matches!(on,ObjectSet::Prepared(_)) && self.one_object(on);
                    let shape = (count, *each_host, selected);
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
                if let (None, Some(each), false, false, false) =
                    (times, each, *until_lost, *may_stop, *called)
                {
                    let call = self.flip_call()?;
                    let each = self.fetch(self.objects(each)?);
                    writeln!(out, "let player_{id} = {player}; let mut flips_{id}: Vec<(ObjectId, bool)> = Vec::new(); let mut done_{id}: i64 = 0; for object in {each} {{ let flip = g.{call}(player_{id}); if flip {{ done_{id} += 1; }} flips_{id}.push((object, flip)); }}").unwrap();
                    self.performed.insert(result.0, format!("done_{id}"));
                    self.coins = Some((
                        format!("done_{id}"),
                        format!("(flips_{id}.len() as i64 - done_{id})"),
                    ));
                    self.coin_objects = Some(format!("flips_{id}"));
                    return Ok(());
                }
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
                // "Flip a coin until you lose a flip or choose to stop flipping":
                // after each flip won, the flipper may stop (CR 705).
                if *may_stop && *until_lost && !*called && times == "None" {
                    let call = self.flip_call()?;
                    writeln!(out, "let player_{id} = {player}; let mut done_{id}: i64 = 0; let mut flips_{id}: u32 = 0; loop {{ static ASK: Question = Question::new(\"Flip\", \"flip again?\", \"\", \"Flip\", \"Stop\"); flips_{id} += 1; if !g.{call}(player_{id}) {{ break; }} done_{id} += 1; if !{M}::effects::ask_may(g, player_{id}, asker.source, Purpose::Ask(&ASK))? {{ break; }} }}").unwrap();
                    self.performed.insert(result.0, format!("done_{id}"));
                    self.coins = Some((
                        format!("done_{id}"),
                        format!("(i64::from(flips_{id}) - done_{id})"),
                    ));
                    return Ok(());
                }
                if *may_stop {
                    self.plan_code(&feature, &format!("{M}::coins::flips(g, {player}, {times}, {until_lost}, {may_stop}, {called})?"), out);
                    return Ok(());
                }
                // A called flip is won or lost (CR 705.2), whatever its line says of
                // heads or tails.
                let call = if *called { "flip_coin" } else { self.flip_call()? };
                writeln!(out, "let player_{id} = {player}; let times_{id}: Option<u32> = {times}; let mut done_{id}: i64 = 0; let mut flips_{id}: u32 = 0; while times_{id}.map_or(true, |n| flips_{id} < n) {{ flips_{id} += 1; if g.{call}(player_{id}) {{ done_{id} += 1; }} else if {until_lost} {{ break; }} }}").unwrap();
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
                let amount = self.fetch(self.number(amount)?.choice_amount()?);
                let least = u32::from(!*any_number);
                writeln!(out, "let objects_{id} = {}; let mut left_{id}: u32 = {amount}; let mut puts_{id}: Vec<(ObjectId, CounterKind, {counter_ty})> = Vec::new(); for (i, &object) in objects_{id}.iter().enumerate() {{ let after = (objects_{id}.len() - i - 1) as u32; let n = if after == 0 {{ left_{id} }} else {{ let most = left_{id}.saturating_sub(after * {least}); {M}::choose::number(g, asker.controller, asker.source, {least}.min(most), most, \"Distribute\")? }}; left_{id} -= n.min(left_{id}); if n > 0 {{ puts_{id}.push((object, {kind}, n as {counter_ty})); }} }} g.add_counters_together(&puts_{id});", self.acted_on(objects)?, counter_ty = Quantity::CounterPlacement.kind()).unwrap();
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
                self.number(amount)?.quantity(Quantity::CounterPlacement)?
            ),
            Change::CantBeCountered => "SpellChange::CantBeCountered".to_string(),
            Change::SpellCost(modifier) => {
                format!("SpellChange::Cost({})", self.cost_delta(modifier)?)
            }
            // An effect on the spell until a time (Summon: Brynhildr's haste
            // until end of turn): the effect and its duration, each one, as the
            // game keeps an effect, with no list of them to borrow.
            Change::Continuing { changes, until }
                if changes.iter().all(|c| {
                    !matches!(
                        c,
                        Change::EntersWithCounters { .. }
                            | Change::CantBeCountered
                            | Change::SpellCost(_)
                            | Change::Continuing { .. }
                            | Change::GrantAbility(_)
                    )
                }) =>
            {
                let until = self.duration(until)?;
                let mut each = Vec::new();
                for change in changes {
                    for kind in self.effect_kinds(change)? {
                        each.push(format!("SpellChange::EffectUntil({kind}, {until})"));
                    }
                }
                each.join(", ")
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
    /// How a copy or a retargeted spell gets its targets, as data, and the
    /// objects or players it names to choose among (empty when it names none).
    fn new_targets(&self, targets: &NewTargets) -> Result<(String, String)> {
        let none = "Vec::<Target>::new()".to_string();
        let objects = |objects: &ObjectSet| -> Result<String> {
            Ok(format!(
                "{}.into_iter().map(Target::Object).collect::<Vec<Target>>()",
                self.fetch(self.objects(objects)?)
            ))
        };
        let players = |players: &PlayerSet| -> Result<String> {
            Ok(format!(
                "{}.into_iter().map(Target::Player).collect::<Vec<Target>>()",
                self.fetch(self.players(players)?)
            ))
        };
        Ok(match targets {
            NewTargets::Same => ("NewTargets::Same".to_string(), none),
            NewTargets::May => ("NewTargets::May".to_string(), none),
            NewTargets::Must => ("NewTargets::Must".to_string(), none),
            NewTargets::To(those) => ("NewTargets::To".to_string(), objects(those)?),
            NewTargets::Each(those) => ("NewTargets::Each".to_string(), objects(those)?),
            NewTargets::ToPlayers(those) => {
                ("NewTargets::ToPlayers".to_string(), players(those)?)
            }
            NewTargets::Random(_) => ("NewTargets::Random".to_string(), none),
            NewTargets::EachLegal => ("NewTargets::EachLegal".to_string(), none),
            NewTargets::EachPlayer(those) => (
                planned::planned_value(
                    "new targets EachPlayer",
                    "NewTargets::EachLegal",
                    "NewTargets::EachPlayer",
                ),
                players(those)?,
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
                // A variant of one list holds the list ("And" of any number of
                // filters), not as many fields.
                let listed = crate::lower::LIST_VARIANTS.contains(&(family.as_str(), form.as_str()));
                match operands.is_empty() {
                    _ if listed => format!("{family}::{form}(&[{}])", operands.join(", ")),
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
    /// A term the game keeps, as Copy data: a set of permanents it names by what
    /// they are becomes a test of each (a rule applies to permanents to come as
    /// well), and a set of objects it names otherwise, or of players, is taken
    /// out into `objects` or `players` for the call to be given. One of each at
    /// most: a second stays where it is.
    pub(super) fn term_apart(
        &mut self,
        operand: &Operand,
        objects: &mut Option<String>,
        players: &mut Option<String>,
    ) -> String {
        let Operand::Term {
            family,
            form,
            operands,
        } = operand
        else {
            return self.operand_code(operand);
        };
        let mut parts = Vec::new();
        for o in operands {
            match o {
                Operand::Objects(crate::battlefield_query!(filter)) => {
                    match self.filter(filter, &Candidate::of("id")) {
                        Ok(test) => parts.push(format!(
                            "|q: &dyn Query, asker: &Asker, id: ObjectId| -> bool {{ {test} }}"
                        )),
                        Err(_) => parts.push(self.operand_code(o)),
                    }
                }
                Operand::Objects(set) if objects.is_none() => match self.objects(set) {
                    Ok(code) => *objects = Some(self.fetch(code)),
                    Err(_) => parts.push(self.operand_code(o)),
                },
                Operand::Players(set) if players.is_none() => match self.players(set) {
                    Ok(code) => *players = Some(self.fetch(code)),
                    Err(_) => parts.push(self.operand_code(o)),
                },
                Operand::Term { .. } => parts.push(self.term_apart(o, objects, players)),
                other => parts.push(self.operand_code(other)),
            }
        }
        match parts.is_empty() {
            true => format!("{family}::{form}"),
            false => format!("{family}::{form}({})", parts.join(", ")),
        }
    }
    /// `term_apart` where the call has no list to be given: a set of permanents
    /// named by what they are is a test of each; one named otherwise stays.
    pub(super) fn kept_term(&mut self, operand: &Operand) -> String {
        let (mut objects, mut players) = (None, None);
        match self.term_apart(operand, &mut objects, &mut players) {
            code if objects.is_none() && players.is_none() => code,
            _ => self.operand_code(operand),
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
                if planned::switched_on(quantity::BOUNDED_32) { "i32" } else { "i16" },
                "q.chars(id).power.unwrap_or(0)",
                "with different powers",
            ),
            SelectionConstraint::CommonProperty(DistinctProperty::CardTypes) => {
                format!("{M}::select::SHARE_A_CARD_TYPE")
            }
            // "That share a land type" (CR 205.3i): some one land type every card
            // chosen has, as SHARE_A_CARD_TYPE asks of card types (Myriad Landscape).
            SelectionConstraint::CommonProperty(DistinctProperty::LandTypes) => {
                const NAME: &str = "share_a_land_type";
                if !self.functions.contains(&format!("fn {NAME}(")) {
                    writeln!(self.functions, "fn {NAME}(q: &dyn Query, choice: &Choice, _: u32) -> Verdict {{ let all_have = |t: Subtype| choice.selected.iter().all(|&c| q.chars(c).subtypes.has(t)); let complete = choice.selected.is_empty() || SubtypeSet::LAND_TYPES.iter().any(all_have); let addable = Bits::of(choice.candidates.len(), |i| {{ let candidate = q.chars(choice.candidates[i]); choice.selected.is_empty() || SubtypeSet::LAND_TYPES.iter().any(|t| all_have(t) && candidate.subtypes.has(t)) }}); Verdict {{ complete, addable }} }}\nfn largest_{NAME}(q: &dyn Query, choice: &Choice, _: u32) -> Vec<ObjectId> {{ SubtypeSet::LAND_TYPES.iter().map(|t| choice.candidates.iter().copied().filter(|&c| q.chars(c).subtypes.has(t)).collect::<Vec<ObjectId>>()).max_by_key(Vec::len).unwrap_or_default() }}").unwrap();
                }
                format!("cantrip_api::select::set_part(&({NAME} as SelectFn), &(largest_{NAME} as cantrip_api::select::LargestFn), 0, \"that share a land type\")")
            }
            SelectionConstraint::NoCommonProperty(DistinctProperty::CreatureTypes) => {
                self.helpers.insert("no_common_creature_type");
                "cantrip_api::select::set_part(&(no_common_creature_type as SelectFn), &(largest_no_common_creature_type as cantrip_api::select::LargestFn), 0, \"that share no creature types\")".to_string()
            }
            // "For each color pair, choose a card that's exactly those colors": each
            // card two colors, no two the same two.
            SelectionConstraint::NoCommonProperty(DistinctProperty::ColorPairs) => {
                if !self.functions.contains("fn one_of_each_color_pair(") {
                    writeln!(self.functions, "fn color_pair_of(q: &dyn Query, id: ObjectId) -> Option<ColorSet> {{ let colors = q.chars(id).colors; (colors.iter().count() == 2).then_some(colors) }}\nfn one_of_each_color_pair(q: &dyn Query, choice: &Choice, _: u32) -> Verdict {{ let used: Vec<Option<ColorSet>> = choice.selected.iter().map(|&c| color_pair_of(q, c)).collect(); let complete = used.iter().enumerate().all(|(i, k)| k.is_some() && !used[..i].contains(k)); Verdict {{ complete, addable: Bits::of(choice.candidates.len(), |i| {{ let k = color_pair_of(q, choice.candidates[i]); k.is_some() && !used.contains(&k) }}) }} }}").unwrap();
                }
                "cantrip_api::select::part(&(one_of_each_color_pair as SelectFn), 0, \"one for each color pair\")".to_string()
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
                    other if self.counts_own_x(other) => "TargetSpec::COUNTED".to_string(),
                    // "When you do, ... with total mana value X or less, where X is
                    // the result": the number a reflexive ability was given, which
                    // its `COUNTED` reads (CR 603.12; Fire Lord Sozin).
                    Expr::Performed(result) | Expr::CountersRemoved(result) | Expr::DieResult(result)
                        if self.counted == Some(result.0)
                            && self.slot_counts == target_counts::SlotCounts::Delayed =>
                    {
                        "TargetSpec::COUNTED".to_string()
                    }
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
    pub(super) fn clauses_judge(
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
        if self.from_outside(objects) {
            self.notes.outside.insert(result.0);
        }
        let id = self.fresh();
        let chooser = self.fetch(self.certain_player(player)?);
        let pool = self.fetch(self.objects(objects)?);
        if let SelectionConstraint::OneForEach(filters) = constraint {
            let picked = self.choose_filter_parts_local(id, &chooser, &pool, count, filters, purpose, out)?;
            self.selections.insert(result.0, picked);
            return Ok(());
        }
        // "Each opponent chooses a different card": not one an earlier turn of the
        // walk picked, which the walk keeps (`unpicked_{walk}`).
        if let (SelectionConstraint::Unpicked, Some((_, players))) =
            (constraint, self.notes.walking.as_ref())
        {
            let walk = players.trim_start_matches("players_").to_string();
            let count = self.fetch(self.select_count(count)?);
            writeln!(out, "let player_{id} = {chooser}; let pool_{id}: Vec<ObjectId> = {pool}.into_iter().filter(|o| !unpicked_{walk}.contains(o)).collect(); let count_{id} = {count}; let picked_{id} = {M}::choose::select(g, player_{id}, asker.source, Purpose::{purpose}, pool_{id}, {M}::select::Selection::of(count_{id}, &[]))?; unpicked_{walk}.extend(picked_{id}.iter().copied());").unwrap();
            self.selections.insert(result.0, format!("picked_{id}"));
            return Ok(());
        }
        // "With total mana value less than or equal to the sacrificed artifact's":
        // the bound is read as the effect resolves, before the choice.
        let dynamic = match constraint {
            SelectionConstraint::TotalProperty {
                property: Property::ManaValue,
                comparison: Comparison::LessOrEqual,
                amount,
            } if !matches!(amount, Expr::Constant(_) | Expr::AnnouncedX)
                && self.reader == Reader::Resolution =>
            {
                let most = self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?);
                writeln!(out, "let most_{id}: u32 = {most};").unwrap();
                Some(format!("{M}::select::total_mana_value_at_most(most_{id})"))
            }
            _ => None,
        };
        let Some(part) = dynamic.or_else(|| self.set_rule(constraint)) else {
            return Err(gap(format!("selection constraint {}", form(constraint))));
        };
        // A card for each color pair there is one of: as many as can be, of the ten.
        let pairs = Count::AsManyAsCan(Expr::Constant(10));
        let count = match (constraint, count) {
            (SelectionConstraint::NoCommonProperty(DistinctProperty::ColorPairs), Count::AnyNumber) => &pairs,
            _ => count,
        };
        let exactly = match count {
            Count::Exactly(Expr::Constant(n)) if self.line_text().is_some_and(|line| line.contains(" exactly ")) => Some(*n),
            _ => None,
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
        // "Reveal exactly two cards ... with different names": fewer is not doing
        // it at all, where a plain "two" does as much as it can (CR 609.3).
        if let Some(n) = exactly {
            writeln!(out, "let picked_{id}: Vec<ObjectId> = if picked_{id}.len() == {n} {{ picked_{id} }} else {{ Vec::new() }};").unwrap();
        }
        self.selections.insert(result.0, format!("picked_{id}"));
        Ok(())
    }
    /// Whether these are "sacrifices that target and returns that target card".
    fn welds(&self, operations: &[Operation]) -> bool {
        matches!(
            operations,
            [Operation::Sacrifice {
                players: PlayerSet::One(_),
                objects: ObjectSet::One(ObjectRef::Target { .. }),
                choose: None,
            }, Operation::Move {
                objects: ObjectSet::One(ObjectRef::Target { .. }),
                destination: Destination::OwnersBattlefield,
                face_down: false,
                entry,
                terms,
            }] if effects::plain_entry(entry) && terms.is_default()
        )
    }
    /// `chosen_{id}`: a permanent `player_{id}` controls for each of `slots`, the
    /// chooser's own but always as many as there can be.
    pub(super) fn sacrifice_slots(
        &mut self,
        id: usize,
        player: &str,
        slots: &[ObjectSet],
        out: &mut String,
    ) -> Result<()> {
        self.helpers.insert("most_assigned");
        writeln!(out, "let player_{id}: PlayerId = {player}; let mut slots_{id}: Vec<Vec<ObjectId>> = Vec::new();").unwrap();
        for slot in slots {
            let list = self.fetch(self.objects(slot)?);
            writeln!(out, "let slot: Vec<ObjectId> = {list}; slots_{id}.push(slot.into_iter().filter(|&o| g.controller(o) == player_{id} && g.can_sacrifice(player_{id}, o)).collect());").unwrap();
        }
        writeln!(out, "let mut chosen_{id}: Vec<ObjectId> = Vec::new(); for at in 0..slots_{id}.len() {{ let goal = most_assigned(&slots_{id}[at..], &mut chosen_{id}.clone()); let keeps: Vec<ObjectId> = slots_{id}[at].iter().copied().filter(|o| !chosen_{id}.contains(o)).filter(|&o| {{ let mut taken = chosen_{id}.clone(); taken.push(o); 1 + most_assigned(&slots_{id}[at + 1..], &mut taken) == goal }}).collect(); let picked = match keeps.as_slice() {{ [] => None, [one] => Some(*one), _ => {M}::choose::card(g, player_{id}, asker.source, Purpose::Sacrifice, keeps.clone())? }}; if let Some(o) = picked {{ chosen_{id}.push(o); }} }}").unwrap();
        Ok(())
    }
    /// Whether the next instruction exiles what `result` reveals, and nothing else.
    fn exiled_as_it_goes(&self, result: &ResultId) -> bool {
        matches!(
            self.after.first(),
            Some(Instruction::Effect {
                operation: Operation::Move {
                    objects: ObjectSet::Observed(seen),
                    destination: Destination::Exile,
                    entry,
                    face_down: false,
                    terms,
                },
                ..
            }) if terms.is_default() && seen.0 == result.0 && !entry.tapped && entry.counters.is_empty() && entry.states.is_empty()
        )
    }
    /// "Your choice of a reach, vigilance or trample counter", "a counter of a
    /// kind it doesn't have, at random", "for each color", "for each such ability".
    /// Whether it wrote the code; each kind is judged in turn (CR 608.2c).
    fn chosen_counters(
        &mut self,
        id: usize,
        objects: &ObjectSet,
        choice: &CounterChoice,
        result: &ResultId,
        out: &mut String,
    ) -> Result<bool> {
        let objects = self.fetch(self.objects(objects)?);
        let list = |kinds: &[crate::counter::CounterKind]| -> Result<String> {
            let kinds = kinds.iter().map(counter_kind).collect::<Result<Vec<String>>>()?;
            Ok(format!("[{}]", kinds.join(", ")))
        };
        match choice {
            CounterChoice::RandomMissing(kinds) => {
                let kinds = list(kinds)?;
                writeln!(out, "let objects_{id} = {objects}; for object in objects_{id} {{ let missing: Vec<CounterKind> = {kinds}.into_iter().filter(|&kind| g.state().obj(object).counters(kind) == 0).collect(); if !missing.is_empty() {{ let at = g.choose_at_random(missing.len()); g.add_counters(object, missing[at], 1); }} }}").unwrap();
            }
            CounterChoice::OneOf(kinds) if self.stack_item => {
                let words: Vec<String> = kinds
                    .iter()
                    .map(|kind| {
                        let code = counter_kind(kind)?;
                        let word = code.trim_start_matches("CounterKind::").to_ascii_lowercase();
                        Ok(format!("{word} counter"))
                    })
                    .collect::<Result<_>>()?;
                let base = self.choice_labels.len();
                let end = base + words.len();
                self.choice_labels.extend(words);
                let labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
                self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
                let kinds = list(kinds)?;
                writeln!(out, "let objects_{id} = {objects}; let mut rows_{id}: Vec<(ObjectId, CounterKind, {counter_ty})> = Vec::new(); for object in objects_{id} {{ let picked = {M}::choose::option(g, asker.controller, asker.source, item.ability.card, {base}..{end})? - {base}; rows_{id}.push((object, {kinds}[picked as usize], 1)); }} g.add_counters_together(&rows_{id});", counter_ty = Quantity::CounterPlacement.kind()).unwrap();
            }
            CounterChoice::OneOf(_) => return Ok(false),
            CounterChoice::OnePerColor(kind) => {
                let kind = counter_kind(kind)?;
                writeln!(out, "let objects_{id} = {objects}; let mut rows_{id}: Vec<(ObjectId, CounterKind, {counter_ty})> = Vec::new(); for color in Color::ALL {{ let eligible: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.chars(o).colors.has(color)).collect(); if let Some(object) = {M}::choose::card(g, asker.controller, asker.source, Purpose::PutCounters({kind}, 1), eligible)? {{ rows_{id}.push((object, {kind}, 1)); }} }} let mut before_{id}: Vec<(ObjectId, {stored_ty})> = rows_{id}.iter().map(|row| (row.0, g.state().obj(row.0).counters({kind}))).collect(); before_{id}.sort(); before_{id}.dedup(); g.add_counters_together(&rows_{id}); let placed_{id}: Vec<ObjectId> = before_{id}.iter().filter(|row| g.state().obj(row.0).counters({kind}) > row.1).map(|row| row.0).collect();", counter_ty = Quantity::CounterPlacement.kind(), stored_ty = Quantity::StoredCounter.kind()).unwrap();
                self.selections.insert(result.0, format!("placed_{id}"));
            }
            CounterChoice::PerAbility {
                of,
                kinds,
                missing_only,
            } => {
                let [Operand::List(abilities), about] = kinds.as_slice() else {
                    return Ok(false);
                };
                let mut pairs = Vec::new();
                for ability in abilities {
                    let Operand::Term { form, .. } = ability else {
                        return Ok(false);
                    };
                    let word = form.strip_prefix("Any").unwrap_or(form);
                    pairs.push(format!("(Keyword::{word}, CounterKind::{word})"));
                }
                let pairs = format!("[{}]", pairs.join(", "));
                // A "hexproof from [quality]" ability is a hexproof ability (CR 702.11d).
                let has = "(c.keywords.has(keyword) || (keyword == Keyword::Hexproof && c.hexproof_from != HexproofFrom::NONE))";
                match (about, of, missing_only) {
                    // "If that creature has it and this one doesn't, put a counter on this."
                    (Operand::Object(other), Some(this), true) => {
                        let other = self.fetch(self.object(other)?);
                        let this = self.fetch(self.object(this)?);
                        writeln!(out, "let objects_{id} = {objects}; if let (Some(other), Some(this)) = ({other}, {this}) {{ for (keyword, kind) in {pairs} {{ let (c, o) = (g.chars(other), g.chars(this)); let lacks = !o.keywords.has(keyword); if {has} && lacks {{ for &object in &objects_{id} {{ g.add_counters(object, kind, 1); }} }} }} }}").unwrap();
                    }
                    // "Put a flying counter on any creature you control if a card
                    // in your graveyard has flying. Repeat this for ..."
                    (Operand::Filter(ObjectFilter::All(parts)), None, false) => {
                        let scope = parts.iter().find_map(|part| match part {
                            ObjectFilter::InZone(scope) => Some(scope.clone()),
                            _ => None,
                        });
                        let Some(scope) = scope else {
                            return Ok(false);
                        };
                        let rest: Vec<ObjectFilter> = parts
                            .iter()
                            .filter(|part| !matches!(part, ObjectFilter::InZone(_)))
                            .cloned()
                            .collect();
                        let pool = ObjectSet::Zone {
                            scope,
                            kind: ObjectKind::Any,
                            filter: ObjectFilter::All(rest),
                        };
                        let pool = self.fetch(self.objects(&pool)?);
                        writeln!(out, "let pool_{id} = {pool}; let mut done_{id}: i64 = 0; for (keyword, kind) in {pairs} {{ if pool_{id}.iter().any(|&card| {{ let c = g.chars(card); {has} }}) {{ let among = {objects}; if let Some(object) = {M}::choose::card(g, asker.controller, asker.source, Purpose::PutCounters(kind, 1), among)? {{ let before = g.state().obj(object).counters(kind); g.add_counters(object, kind, 1); done_{id} += i64::from(g.state().obj(object).counters(kind) - before).max(0); }} }} }}").unwrap();
                        self.performed.insert(result.0, format!("done_{id}"));
                    }
                    _ => return Ok(false),
                }
            }
        }
        Ok(true)
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
    /// "As this enters, choose two abilities from among first strike, vigilance,
    /// and lifelink" (Greymond): choosing `k` of them is choosing one set of `k`,
    /// each set one of the card's words (CR 607.2f), kept as `chosen_option`.
    fn abilities_as_enters(&mut self, k: usize, options: &[Operand], out: &mut String) -> Result<()> {
        let failure = || gap("a choice of several abilities as it enters");
        if self.entering_labels.is_some() || !self.choice_labels.is_empty() {
            return Err(failure());
        }
        let mut abilities = Vec::new();
        let mut names = Vec::new();
        for option in options {
            let Operand::Ability(ability) = option else { return Err(failure()) };
            let Ability::Keyword(word) = &**ability else { return Err(failure()) };
            keyword(word)?;
            names.push(super::lane_h::keyword_words(word).ok_or_else(failure)?);
            abilities.push((**ability).clone());
        }
        let mut sets: Vec<Vec<usize>> = vec![Vec::new()];
        for _ in 0..k {
            sets = sets
                .into_iter()
                .flat_map(|set| {
                    let from = set.last().map_or(0, |&i| i + 1);
                    (from..abilities.len()).map(move |i| [set.clone(), vec![i]].concat())
                })
                .collect();
        }
        let labels: Vec<String> = sets
            .iter()
            .map(|set| {
                let words: Vec<&str> = set.iter().map(|&i| names[i].as_str()).collect();
                quoted(&words.join(" and "))
            })
            .collect();
        self.entering_labels = Some(format!("&[{}]", labels.join(", ")));
        writeln!(out, "{M}::choose::option_as_enters(g, entering, &CARD, 0..{})?;", sets.len()).unwrap();
        self.ability_choice = abilities;
        self.ability_sets = sets;
        Ok(())
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
    pub(super) fn choose_on_terms(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        kind: ValueKind,
        terms: &ChoiceTerms,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        if let Some(count) = terms.ordinary_count() {
            if count == 1 {
                // Restricted card types are not the unrestricted card-type menu.
                if kind == ValueKind::CardType
                    && terms
                        .options
                        .iter()
                        .any(|o| matches!(o, Operand::Objects(_)))
                {
                    self.as_event_chooses = true;
                    return Err(gap("a card type shared among the cards used to craft it"));
                }
                let mut code = String::new();
                match self.choose_value(id, actor, kind, count, &terms.options, &mut code) {
                    Ok(()) => {
                        out.push_str(&code);
                        return Ok(());
                    }
                    // These native menus validate their operands precisely. Keep
                    // that diagnostic instead of hiding it behind a generic hole.
                    Err(error)
                        if matches!(
                            kind,
                            ValueKind::Ability | ValueKind::CardName | ValueKind::Word
                        ) =>
                    {
                        return Err(error)
                    }
                    Err(_) => return self.choose_value_terms(id, actor, kind, terms, out),
                }
            }
            if kind == ValueKind::Ability
                && self.pending == Pending::Entering
                && count > 1
                && (count as usize) < terms.options.len()
            {
                return self.abilities_as_enters(count as usize, &terms.options, out);
            }
            // The scalar multi-choice helper has no option restrictions. Word
            // replacement and entering pairs consume their options themselves.
            if terms.options.is_empty()
                || matches!(kind, ValueKind::ColorWord | ValueKind::BasicLandType)
                || (self.pending == Pending::Entering && self.enters_two == Some(kind))
            {
                return self.choose_value(id, actor, kind, count, &terms.options, out);
            }
        }
        if kind == ValueKind::CardType && self.pending == Pending::Entering {
            if let Some(options) = lane_p::entering_card_types_but(terms) {
                return self.choose_value(id, actor, kind, 1, &options, out);
            }
        }
        self.choose_value_terms(id, actor, kind, terms, out)?;
        self.chosen_number_performed(kind, result);
        if kind == ValueKind::Object && out.contains(&format!("let chosen_{id} =")) {
            self.selections.insert(result.0, format!("chosen_{id}"));
        }
        Ok(())
    }
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
            // The planned helper uses None only for unbounded cardinality.
            // A source default of one is already Exactly(1) in the IR.
            ChoiceCount::AnyNumber => "None".to_string(),
            ChoiceCount::Exactly(count) | ChoiceCount::UpTo(count) => format!(
                "Some({})",
                self.fetch(self.number(count)?.amount(u32::MAX.into(), "u32")?)
            ),
        };
        let up_to = matches!(terms.count, ChoiceCount::UpTo(_));
        let ChoiceTerms {
            secret,
            unchosen,
            random,
            ordered,
            ..
        } = terms;
        if let (ValueKind::CreatureType, PlayerSet::One(player), [_, ..]) =
            (kind, actor, except.as_slice())
        {
            let plain = terms.options.is_empty()
                && terms.exactly_one()
                && !(*secret || *unchosen || *random || *ordered || up_to)
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
        // Cantrip's choices are all in the open, and a secret one stays hidden until
        // what reveals it: nothing keeps it from the other players.
        let feature = match terms.secret {
            true => "a value chosen in secret",
            false => "operation ChooseValueWith",
        };
        let code = format!("{M}::choose::value(g, {players}, asker.source, ChoiceKind::{kind:?}, ChoiceTerms {{ options: &[{}], except: &[{}], count: {count}, secret: {secret}, unchosen: {unchosen}, random: {random}, ordered: {ordered}, up_to: {up_to} }})?", options.join(", "), except.join(", "));
        self.value_to_come(feature, id, kind, &code, out);
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
            ValueKind::Object => Some("Vec::<ObjectId>::new()"),
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
        // "With a number of time counters on it equal to its mana value": each
        // card's own, read as it arrives.
        self.moving_one = Some("moved_o".into());
        let each: Result<Vec<String>> = entry
            .counters
            .iter()
            .map(|(kind, n)| {
                let n = self.number(n)?.quantity(Quantity::CounterPlacement)?;
                Ok(format!("({}, {n})", counter_kind(kind)?))
            })
            .collect();
        self.moving_one = None;
        let each = each?;
        if each.iter().any(|c| c.contains("moved_o")) {
            return Ok(format!("{{ let exiled = g.exile_all(&objects_{id}); let puts: Vec<(ObjectId, CounterKind, {counter_ty})> = {{ let q: &dyn Query = g; exiled.iter().flat_map(|&moved_o| [{}].into_iter().map(move |(kind, n): (CounterKind, {counter_ty})| (moved_o, kind, n))).collect() }}; g.add_counters_together(&puts); exiled }}", each.join(", "), counter_ty = Quantity::CounterPlacement.kind()));
        }
        let mut counters = Vec::new();
        for (kind, n) in &entry.counters {
            let n = self.number(n)?.quantity(Quantity::CounterPlacement)?;
            counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
        }
        Ok(format!("{{ let counters: Vec<(CounterKind, {counter_ty})> = vec![{}]; let exiled = g.exile_all(&objects_{id}); let puts: Vec<(ObjectId, CounterKind, {counter_ty})> = exiled.iter().flat_map(|&o| counters.iter().map(move |&(kind, n)| (o, kind, n))).collect(); g.add_counters_together(&puts); exiled }}", counters.join(", "), counter_ty = Quantity::CounterPlacement.kind()))
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
                let feature = arrival_hole(states);
                // As a move of several objects in a state says it (lane_p): one
                // here. A state the generator can't read yet is its words.
                let mut each = Vec::new();
                for state in states {
                    let (gaps, functions) = (self.gaps.len(), self.functions.len());
                    match self.arrival_state(state, feature) {
                        Ok(code) => each.push(code),
                        Err(_) => {
                            self.gaps.truncate(gaps);
                            self.functions.truncate(functions);
                            each.push(quoted(&format!("{state:?}")));
                        }
                    }
                }
                let arrival = format!(
                    "Arrival {{ tapped: {tapped}, counters: &[{counters}], states: &[{}] }}",
                    each.join(", ")
                );
                let call = planned::planned_value(
                    feature,
                    "Vec::<ObjectId>::new()",
                    &format!("{M}::enter::in_state(g, asker, &[o], Destination::Battlefield(Some(controller)), {arrival}, false)?"),
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
        let on_objects = matches!(subject, EffectSubject::Objects(_));
        // Objects and players named as it resolves are the call's to read, as
        // `exile_all` takes them, not a list the effect keeps; a set of spells
        // to come is asked of each, so it is a rule (`EffectOn`).
        let (call, subject) = match subject {
            EffectSubject::Objects(objects) => (
                "effect_on_objects",
                format!("&{}", self.fetch(self.objects(objects)?)),
            ),
            EffectSubject::Players(players) => (
                "effect_on_players",
                format!("&{}", self.fetch(self.players(players)?)),
            ),
            EffectSubject::EachSpell(filter) => {
                let test = self.filter(filter, &Candidate::of("id"))?;
                ("effect_on", format!("EffectOn::EachSpell(|q: &dyn Query, asker: &Asker, id: ObjectId| {test})"))
            }
            EffectSubject::NextSpell {
                spells,
                caster,
                this_turn,
                from,
            } => {
                let test = self.filter(spells, &Candidate::of("id"))?;
                let caster = self.fetch(self.certain_player(caster)?);
                ("effect_on", format!("EffectOn::NextSpell {{ spells: |q: &dyn Query, asker: &Asker, id: ObjectId| {test}, caster: {caster}, this_turn: {this_turn}, from: {:?} }}", from.as_ref().map(|z| format!("{z:?}"))))
            }
        };
        let until = match until {
            Some(until) => format!("Some({})", self.duration(until)?),
            None => "None".to_string(),
        };
        // "Exile that card instead ... as it resolves" (Feather): what an effect
        // on objects does is done to the object it is on, which it is handed.
        let saved = match on_objects {
            true => self.affected_object.replace("affected"),
            false => self.affected_object,
        };
        let effects: Vec<String> = effects
            .iter()
            .map(|o| {
                let code = self.kept_term(o);
                match on_objects {
                    true => code.replace(
                        "|g: &mut dyn Game| -> Result<(), Suspend>",
                        "|g: &mut dyn Game, affected: ObjectId| -> Result<(), Suspend>",
                    ),
                    false => code,
                }
            })
            .collect();
        self.affected_object = saved;
        self.plan_code(
            "operation EffectOn",
            &format!(
                "{M}::effects::{call}(g, asker, {subject}, &[{}], {until})?",
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
            .and_then(|n| n.quantity(Quantity::Damage));
        let targets = self.recipients(recipients);
        self.affected_object = saved;
        let (amount, targets) = (self.fetch(amount?), self.fetch(targets?));
        let deal = if self.read_later(result) {
            self.report_quantity(result.0, format!("done_{id}"), Quantity::Damage);
            format!("let dealt_{id} = g.damage_together(&batch_{id}); let done_{id}: i64 = dealt_{id}.iter().map(|&n| i64::from(n)).{damage_sum};", damage_sum = if planned::switched_on(quantity::FEATURE) { Quantity::Damage.sum_i64() } else { "sum()" })
        } else { format!("g.damage_together(&batch_{id});") };
        writeln!(out, "let sources_{id} = {}; let mut batch_{id}: Vec<(ObjectId, Target, {damage_ty})> = Vec::new(); for &source in &sources_{id} {{ let amount = {amount}; for t in {targets} {{ batch_{id}.push((source, t, amount)); }} }} {deal}", self.fetch(self.objects(sources)?), damage_ty = Quantity::Damage.kind()).unwrap();
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
        self.notes.earlier.push(("Revealed", result.clone()));
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
            // "A random color that it isn't": one of the colors none of them has, each
            // as likely; none when they are all colors.
            (ValueKind::Color, [Operand::Term { family, form, operands }])
                if family == "ChoosableColor" && form == "NotColorAmoungPermanents" =>
            {
                let [Operand::Objects(objects)] = operands.as_slice() else {
                    return Err(gap("operation ChooseAtRandom"));
                };
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let held_{id} = {{ let q: &dyn Query = g; ({objects}).into_iter().fold(ColorSet::NONE, |set, o: ObjectId| ColorSet(set.0 | q.chars(o).colors.0)) }}; let pool_{id}: Vec<Color> = Color::ALL.into_iter().filter(|&c| !held_{id}.has(c)).collect(); if pool_{id}.is_empty() {{ return Ok(()); }} let {local} = pool_{id}[g.choose_at_random(pool_{id}.len())];").unwrap();
                if entering {
                    writeln!(out, "entering.chosen_color = Some({local});").unwrap();
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
/// "A 'left' pile and a 'right' pile" (Raging River): piles told apart by the
/// names a later choice and a rule that lasts past the resolution read.
const LABELED_PILES: &str = "a pile's name kept on its objects for a lasting rule";
fn labeled_piles(names: &[String]) -> bool {
    !names.is_empty() && names.iter().all(|n| !n.starts_with("face "))
}
