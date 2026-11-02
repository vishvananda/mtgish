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
                objects: cards,
                destination,
                ..
            }
            | Operation::ChangeZone {
                cards,
                count: None,
                destination,
                ..
            } if chosen(cards) => match destination {
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

/// The one permanent a cost taps, named: "you may tap it".
/// What a card must be for `cost` to be "put a card like this from your hand onto
/// the battlefield tapped and attacking", paid by the player whose hand it is.
fn arrives_attacking_from_hand<'a>(player: &PlayerRef, cost: &'a Cost) -> Option<&'a ObjectFilter> {
    let [CostPart::Move {
        selections,
        destination,
        entry,
        face_down: false,
    }] = cost.parts.as_slice()
    else {
        return None;
    };
    let [CostObjectSelection {
        objects:
            ObjectSet::Zone {
                scope:
                    ZoneScope::Players {
                        zone: EventZone::Hand,
                        players: PlayerSet::One(owner),
                    },
                kind: ObjectKind::Card,
                filter,
            },
        choose: Some(Count::Exactly(Expr::Constant(1))),
    }] = selections.as_slice()
    else {
        return None;
    };
    let same = format!("{owner:?}") == format!("{player:?}")
        && format!("{destination:?}") == format!("Battlefield({player:?})");
    let attacking = format!("{entry:?}")
        == "EntrySpec { tapped: true, counters: [], states: [Attacking { player: None, or_planeswalker: false }] }";
    (same && attacking).then_some(filter)
}

fn taps_one_object(cost: &Cost) -> Option<&ObjectRef> {
    match cost.parts.as_slice() {
        [CostPart::ChangeTapState {
            selections,
            tapped: true,
            symbol: false,
        }] => match selections.as_slice() {
            [CostObjectSelection {
                objects: ObjectSet::One(object),
                choose: None,
            }] if !matches!(object, ObjectRef::AttachedTo(_)) => Some(object),
            _ => None,
        },
        _ => None,
    }
}

/// Whether `decider`'s "you may" is that they pay something the engine can ask
/// about as one payment.
/// `body` with a leading group of payments by one player made one payment of all
/// their parts ("pay {W}{B} and 2 life" is one cost, CR 601.2h), and the result
/// that stands for it with the results it replaces.
fn one_payment(body: &Program) -> (Program, Option<(ResultId, Vec<ResultId>)>) {
    let Some(Instruction::Group(group)) = body.instructions.first() else {
        return (body.clone(), None);
    };
    let mut parts = Vec::new();
    let mut results = Vec::new();
    let mut payer: Option<&PlayerRef> = None;
    for instruction in &group.instructions {
        let Instruction::Effect {
            operation: Operation::PayCost { player, cost, optional: false },
            result,
        } = instruction
        else {
            break;
        };
        if payer.is_some_and(|p| format!("{p:?}") != format!("{player:?}")) {
            break;
        }
        payer = Some(player);
        parts.extend(cost.parts.iter().cloned());
        results.push(result.clone());
    }
    let taken = results.len();
    let (Some(player), Some(last)) = (payer, results.pop()) else {
        return (body.clone(), None);
    };
    if results.is_empty() {
        return (body.clone(), None);
    }
    let mut instructions = vec![Instruction::Effect {
        operation: Operation::PayCost {
            player: player.clone(),
            cost: Cost { parts },
            optional: false,
        },
        result: last.clone(),
    }];
    instructions.extend(group.instructions[taken..].iter().cloned());
    let mut merged = body.clone();
    merged.instructions[0] = Instruction::Group(Program { instructions });
    (merged, Some((last, results)))
}

/// "[decider] may tap N untapped [objects]": what to choose from, how many, the
/// payment's result, and what follows it.
fn may_tap(decider: &PlayerRef, body: &Program) -> Option<(ObjectSet, Expr, ResultId, Program)> {
    let (first, rest) = match body.instructions.as_slice() {
        [Instruction::Group(group), after @ ..] => {
            let (first, inner) = group.instructions.split_first()?;
            let mut rest = inner.to_vec();
            rest.extend(after.iter().cloned());
            (first, rest)
        }
        [first, rest @ ..] => (first, rest.to_vec()),
        [] => return None,
    };
    let Instruction::Effect {
        operation: Operation::PayCost { player, cost, optional: false },
        result,
    } = first
    else {
        return None;
    };
    let [CostPart::ChangeTapState { selections, tapped: true, symbol: false }] = cost.parts.as_slice() else {
        return None;
    };
    let [CostObjectSelection { objects, choose: Some(Count::Exactly(n)) }] = selections.as_slice() else {
        return None;
    };
    (format!("{player:?}") == format!("{decider:?}"))
        .then(|| (objects.clone(), n.clone(), result.clone(), Program { instructions: rest }))
}

impl Compiler {
    /// "[decider] may [pay a cost]" whose cost the engine takes whole, so
    /// `may_pay` offers it (CR 118.12).
    fn pays_whole(&self, decider: &PlayerRef, body: &Program) -> bool {
        if let Some(Instruction::Group(group)) = body.instructions.first() {
            return self.pays_whole(decider, group);
        }
        let Some(Instruction::Effect {
            operation: Operation::PayCost { player, cost, optional: false },
            ..
        }) = body.instructions.first()
        else {
            return false;
        };
        format!("{player:?}") == format!("{decider:?}")
            && cost.parts.iter().all(|part| {
                matches!(part, CostPart::Loyalty(_))
            })
            && self.clone().cost(cost, lane_g::PAID).is_ok()
    }
}

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
/// "Until it leaves the battlefield" names the source when the trigger's event
/// is about the source itself (Lumbering Battlement's "when this enters").
fn returns_with_source(result: &ResultId, rest: &[Instruction], subject_is_source: bool) -> bool {
    matches!(
        rest.first(),
        Some(Instruction::Effect {
            operation: Operation::ReturnAfter {
                objects: ObjectSet::Transition { result: moved, phase: ObjectPhase::After },
                duration: crate::ir::Duration::WhileOnBattlefield(holder),
            },
            ..
        }) if moved.0 == result.0
            && (matches!(holder, ObjectRef::Source)
                || (subject_is_source && matches!(holder, ObjectRef::EventSubject)))
    )
}

/// The target whose leaving the battlefield sends a move back, when the next
/// thing after the move says so: "until target enchantment you control leaves
/// the battlefield".
fn returns_with_holder<'a>(result: &ResultId, rest: &'a [Instruction]) -> Option<&'a ObjectRef> {
    match rest.first() {
        Some(Instruction::Effect {
            operation:
                Operation::ReturnAfter {
                    objects: ObjectSet::Transition { result: moved, phase: ObjectPhase::After },
                    duration: crate::ir::Duration::WhileOnBattlefield(holder @ ObjectRef::Target { .. }),
                },
            ..
        }) if moved.0 == result.0 => Some(holder),
        _ => None,
    }
}

/// The players whose becoming the monarch sends a move back, when the next thing
/// after the move says so: "until an opponent becomes the monarch".
fn returns_when_crowned<'a>(result: &ResultId, rest: &'a [Instruction]) -> Option<&'a PlayerSet> {
    match rest.first() {
        Some(Instruction::Effect {
            operation:
                Operation::ReturnAfter {
                    objects: ObjectSet::Transition { result: moved, phase: ObjectPhase::After },
                    duration: crate::ir::Duration::UntilEvent(event),
                },
            ..
        }) if moved.0 == result.0 => match event.as_ref() {
            EventSpec::Player { event: PlayerEvent::BecomesMonarch, players, objects: None } => Some(players),
            _ => None,
        },
        _ => None,
    }
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
        self.tested_effect(test?, body, label)
    }
    /// `effect` that does nothing unless `test` holds as it resolves.
    pub(super) fn tested_effect(
        &mut self,
        test: String,
        body: &Program,
        label: &str,
    ) -> Result<String> {
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
        let slots: std::collections::BTreeSet<usize> = self.slots.values().map(|(i, _)| *i).collect();
        for i in &slots {
            let own = self.own_slots.get(i);
            match self.second_of_two == Some(*i) {
                true => writeln!(
                    out,
                    "let t_{i}_raw: Vec<Target> = announced(g, item, 0, false).into_iter().skip(1).collect(); let t_{i} = announced(g, item, 0, true);"
                ),
                false if own.is_some() => {
                    let own = own.unwrap();
                    writeln!(
                        out,
                        "let t_{i}_raw: Vec<Target> = [announced(g, item, {i}, false), announced(g, item, {own}, false)].concat(); let t_{i}: Vec<Target> = [announced(g, item, {i}, true), announced(g, item, {own}, true)].concat();"
                    )
                }
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
        let unread = std::mem::take(&mut self.unread);
        let lists = std::mem::take(&mut self.chosen_lists);
        let compiled = self.program(body, &mut out, true);
        self.chosen_lists = lists;
        let kept = std::mem::replace(&mut self.unread, unread);
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
        for (local, lines) in kept {
            let rest = lines.iter().fold(out.clone(), |rest, line| rest.replace(line, ""));
            if !rest.contains(&local) {
                out = rest;
            } else if local.ends_with("_kept") {
                out.insert_str(0, &format!("let mut {local} = None;\n"));
            }
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
    /// Exile `objects` until `holder` leaves the battlefield (CR 610.3): each
    /// card exiled waits on a delayed "until" ability whose source is the
    /// holder, and returns under its owner's control when the holder leaves.
    /// A holder already gone, or no longer a legal target, has no leaving to
    /// wait for, and nothing is exiled (CR 610.3c, 608.2b).
    fn exile_until_holder_leaves(
        &mut self,
        objects: &ObjectSet,
        holder: &ObjectRef,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let ObjectRef::Target { slot, index } = holder else {
            return Err(gap("an exile until another object leaves"));
        };
        let slot = self.slot(slot)?;
        let objects = self.fetch(self.objects(objects)?);
        let id = self.fresh();
        let (_, controller, card) = self.maker();
        let words = self
            .line_text()
            .map(|line| match line.split_once(": ") {
                // A loyalty ability's cost is not its sentence.
                Some((cost, rest)) if cost.trim_start_matches(['[', '+', '−', '-']).trim_end_matches(']').chars().all(|c| c.is_ascii_digit() || c == 'X') => rest.to_string(),
                _ => line.to_string(),
            })
            .and_then(|line| {
                line.split_inclusive(". ")
                    .find(|sentence| sentence.contains(" until "))
                    .map(|sentence| sentence.trim().to_string())
            })
            .unwrap_or_else(|| "Exile it until that permanent leaves the battlefield.".into());
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn until_{id}_leaves(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ match event {{ Event::ZoneChange {{ old, from, .. }} if *old == asker.source && from.is(ZoneKind::Battlefield) => Fired::PLAIN, _ => None }} }}"
        )
        .unwrap();
        writeln!(
            self.functions,
            "fn until_{id}_return(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ if let Target::Object(card) = item.subject {{ let owner = g.obj(card).owner; g.put_onto_battlefield(card, owner, false)?; }} Ok(()) }}"
        )
        .unwrap();
        self.delayed.push(format!(
            "triggered(when_asked(EventMask::ZONE_CHANGE, until_{id}_leaves), until_{id}_return, {})",
            quoted(&words)
        ));
        let at = self.delayed.len() - 1;
        writeln!(
            out,
            "let objects_{id} = {objects}; let holder_{id} = t_{slot}_raw.get({index}).copied().filter(|t| t_{slot}.contains(t)).and_then(|t: Target| t.object()).filter(|&h: &ObjectId| g.state().obj(h).zone.is(ZoneKind::Battlefield)); \
             let moved_{id} = match holder_{id} {{ Some(_) => {{ let by_{id} = g.linked(item); g.exile_with(&objects_{id}, by_{id}, false) }} None => Vec::new() }}; \
             if let Some(holder) = holder_{id} {{ for &card in &moved_{id} {{ g.delayed_trigger(DelayedWhen::UntilEvent, holder, {controller}, AbilityRef::delayed({card}, {at}), Target::Object(card)); }} }}"
        )
        .unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
        self.left_behind(result, gone(&format!("objects_{id}")));
        self.returning.insert(result.0);
        Ok(())
    }
    /// The exile and the wait of "until an opponent becomes the monarch". If
    /// one already did after the source entered, the duration is over before
    /// the exile would begin and nothing moves (CR 610.3b).
    fn exile_until_crowned(
        &mut self,
        objects: &ObjectSet,
        players: &PlayerSet,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let (who, crowned) = match players {
            PlayerSet::OpponentsOf(PlayerRef::AbilityController) => (
                "q.state().opponents(asker.controller).any(|o| o == *player)",
                "g.state().opponents(item.controller).any(|o| o == player)",
            ),
            PlayerSet::All => ("true", "true"),
            _ => return Err(gap("an exile until some players become the monarch")),
        };
        let objects = self.fetch(self.objects(objects)?);
        let id = self.fresh();
        let (_, controller, card) = self.maker();
        let words = self
            .line_text()
            .and_then(|line| line.split_once(", ").map(|(_, rest)| rest.to_string()))
            .map(|rest| rest.get(..1).map_or(String::new(), str::to_uppercase) + rest.get(1..).unwrap_or(""))
            .unwrap_or_else(|| "Exile it until an opponent becomes the monarch.".into());
        writeln!(
            self.functions,
            "#[allow(unused_variables)] fn until_{id}_crowned(q: &dyn Query, asker: &Asker, event: &Event) -> Option<Fired> {{ match event {{ Event::BecameMonarch {{ player }} if {who} => Fired::PLAIN, _ => None }} }}"
        )
        .unwrap();
        writeln!(
            self.functions,
            "fn until_{id}_return(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ if let Target::Object(card) = item.subject {{ let owner = g.obj(card).owner; g.put_onto_battlefield(card, owner, false)?; }} Ok(()) }}"
        )
        .unwrap();
        self.delayed.push(format!(
            "triggered(when_asked(EventMask::BECAME_MONARCH, until_{id}_crowned), until_{id}_return, {})",
            quoted(&words)
        ));
        let at = self.delayed.len() - 1;
        writeln!(
            out,
            "let objects_{id} = {objects}; \
             let entered_{id} = g.history().all().iter().find(|r| matches!(r.event, Event::ZoneChange {{ new, to, .. }} if new == item.object && to.is(ZoneKind::Battlefield))).map(|r| r.serial); \
             let over_{id} = entered_{id}.is_some_and(|since| g.history().all().iter().any(|r| r.serial > since && matches!(r.event, Event::BecameMonarch {{ player }} if {crowned}))); \
             let moved_{id} = match over_{id} {{ false => {{ let by_{id} = g.linked(item); g.exile_with(&objects_{id}, by_{id}, false) }} true => Vec::new() }}; \
             for &card in &moved_{id} {{ g.delayed_trigger(DelayedWhen::UntilEvent, item.object, {controller}, AbilityRef::delayed({card}, {at}), Target::Object(card)); }}"
        )
        .unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
        self.left_behind(result, gone(&format!("objects_{id}")));
        self.returning.insert(result.0);
        Ok(())
    }
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
            if waits && self.failed.contains(&result.0) {
                planned::bind_hole_result(out, mark, result);
            }
        }
        let read_later =
            matches!(instruction, Instruction::Effect { result, .. } if self.read_later(result));
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
            if let (true, Instruction::Effect { result, .. }) = (read_later, instruction) {
                planned::bind_hole_result(out, mark, result);
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
        self.hoist_choices(&inner.chosen, out);
        Ok(())
    }
    /// A value a block chose stays readable past it, in one local for its kind, so
    /// a read is of the latest choice whichever block ran (`choose_value`).
    fn hoist_choices(&mut self, inner: &[(ValueKind, String)], out: &mut String) {
        for (kind, local) in inner.iter().rev() {
            let plain_before = |(k, l): &(ValueKind, String)| k == kind && !l.ends_with(chosen::KEPT);
            if self.hoisted.is_none() || self.chosen.iter().any(plain_before) {
                continue;
            }
            if local.ends_with(chosen::KEPT) {
                self.chosen.retain(|(k, _)| k != kind);
                self.chosen.insert(0, (*kind, local.clone()));
                continue;
            }
            let prefix = local.split('_').next().unwrap_or("chosen");
            let kept = format!("{prefix}_{}_kept", format!("{kind:?}").to_lowercase());
            let line = format!("{kept} = Some({local});\n");
            out.push_str(&line);
            match self.unread.iter_mut().find(|(l, _)| *l == kept) {
                Some((_, lines)) => lines.push(line),
                None => self.unread.push((kept.clone(), vec![line])),
            }
            self.chosen.retain(|(k, _)| k != kind);
            self.chosen.insert(0, (*kind, format!("{kept}{}", chosen::KEPT)));
        }
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
        self.keep_countered(body, i, instruction, out);
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
                // CR 609.7a: what the source does later is not what it is chosen for.
                let purpose = lane_n::chooses_a_source(objects)
                    .or_else(|| purpose(result, rest))
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
                // "A land card of each basic land type": one card for each, a card
                // counted for one type alone (Gaea's Balance), the clauses of one
                // search told apart as several find clauses are.
                if let (
                    [SelectionConstraint::OneForEach(each)],
                    Count::AnyNumber | Count::UpTo(_),
                ) = (choice.constraints.as_slice(), &choice.count)
                {
                    let clauses: Vec<SearchChoice> = each
                        .iter()
                        .map(|filter| SearchChoice {
                            filter: ObjectFilter::All(vec![choice.filter.clone(), filter.clone()]),
                            count: Count::UpTo(Expr::Constant(1)),
                            constraints: Vec::new(),
                        })
                        .collect();
                    let destination = search_destination(result, &body.instructions[i + 1..]);
                    return self.search_several(player, library, &clauses, result, destination, out);
                }
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
                // Or the next instructions take them out of the order at once.
                let them = format!("{excluding:?}");
                let moved = match body.instructions.get(i + 1) {
                    Some(Instruction::If { then, otherwise, .. }) => [then, otherwise]
                        .iter()
                        .all(|branch| format!("{branch:?}").contains(&them)),
                    Some(Instruction::Effect {
                        operation:
                            Operation::Move { objects, .. }
                            | Operation::ChangeZone { cards: objects, .. },
                        ..
                    }) => format!("{objects:?}") == them,
                    _ => false,
                };
                if format!("{next:?}") != format!("{:?}", Some(excluding)) && !moved {
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
                && returns_with_source(result, &body.instructions[i + 1..], matches!(self.event_object, Some(ObjectRef::Source))) =>
            {
                let id = self.fresh();
                writeln!(out, "let objects_{id} = {}; let by_{id} = g.linked(item); let moved_{id} = g.exile_with(&objects_{id}, by_{id}, true);", self.fetch(self.objects(objects)?)).unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                self.left_behind(result, gone(&format!("objects_{id}")));
                self.returning.insert(result.0);
                Ok(())
            }
            // "Exile it until that other permanent leaves the battlefield" (Calix,
            // Destiny's Hand): an "until" wait on that permanent (CR 610.3), as
            // Palace Jailer's is on an opponent becoming the monarch.
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
                && returns_with_holder(result, &body.instructions[i + 1..]).is_some() =>
            {
                let holder = returns_with_holder(result, &body.instructions[i + 1..]).expect("matched");
                self.exile_until_holder_leaves(objects, holder, result, out)
            }
            // "Exile target creature an opponent controls until an opponent
            // becomes the monarch" (Palace Jailer): an "until" wait on that
            // event, which outlives the source (CR 610.3).
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
                && self.source_entered
                && returns_when_crowned(result, &body.instructions[i + 1..]).is_some() =>
            {
                let players = returns_when_crowned(result, &body.instructions[i + 1..]).expect("matched");
                self.exile_until_crowned(objects, players, result, out)
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
            // The trigger's "may" and the "may" of "do this only once each turn" are
            // one question, which the first asks and spends.
            Instruction::Optional { decider, body: then }
                if self.do_once.is_some() && lane_l::once_offered(then).is_some() =>
            {
                let offered = Instruction::Optional {
                    decider: decider.clone(),
                    body: lane_l::once_offered(then).expect("just matched").clone(),
                };
                self.instruction(body, i, &offered, out)
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
                if self
                    .line_text()
                    .is_some_and(|line| line.contains("this turn without paying"))
                {
                    return Err(gap("a play for the turn read as a play now"));
                }
                let once = self.do_once.take();
                self.play_cards(operation, true, out)?;
                if let (Some(key), Some(played)) = (once, &self.played) {
                    writeln!(out, "if !{played}.is_empty() {{ {} }}", lane_k::spends_once(key)).unwrap();
                }
                self.report_played(result);
                Ok(())
            }
            // "You may choose new targets" (115.7d) already leaves any of them.
            Instruction::Optional { body, .. } if lane_i::may_retarget(body).is_some() => {
                let (player, object) = lane_i::may_retarget(body).expect("just matched");
                let id = self.fresh();
                self.retarget(id, player, object, &NewTargets::May, out)
            }
            // "You may tap another untapped Merfolk you control": up to that many
            // chosen and tapped, and "if you do" is that one was (CR 608.2c).
            Instruction::Optional { decider, body } if may_tap(decider, body).is_some() => {
                let (objects, n, result, rest) = may_tap(decider, body).expect("just matched");
                let id = self.fresh();
                writeln!(out, "'optional_{id}: {{").unwrap();
                self.choose(decider, &objects, &Count::UpTo(n), &result, "Tap", out)?;
                let tap = Operation::Tap { objects: ObjectSet::Selected(result.clone()), tapped: true };
                self.operation(&tap, &result, out)?;
                writeln!(out, "let paid_{id} = !{}.is_empty(); if !paid_{id} {{ break 'optional_{id}; }}", self.moved(&result)?).unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
                self.block(&rest, out, true)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::Optional { decider, body }
                if asks_by_paying(decider, &one_payment(body).0)
                    || self.pays_whole(decider, &one_payment(body).0) =>
            {
                // "You may pay {W}{B} and 2 life", "you may tap another untapped
                // Merfolk you control": one cost, offered whole (CR 118.12, 601.2h).
                let (body, merged) = one_payment(body);
                let id = self.fresh();
                writeln!(out, "'optional_{id}: {{").unwrap();
                self.asking = Some(id);
                self.block(&body, out, true)?;
                writeln!(out, "}}").unwrap();
                if let Some((paid, others)) = merged {
                    if let Some(code) = self.performed.get(&paid.0).cloned() {
                        for other in others {
                            self.performed.insert(other.0, code.clone());
                        }
                    }
                }
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
                // "You may return one of them ... Do this only once each turn"
                // (Rinoa, Angel Wing): what is chosen spends the turn's once.
                let (open, spent) = match self.do_once.take() {
                    Some(key) => (
                        format!("if {{ let q: &dyn Query = g; {} }} ", lane_k::not_yet_done(key)),
                        lane_k::spends_once(key),
                    ),
                    None => (String::new(), String::new()),
                };
                writeln!(out, "{open}{{").unwrap();
                self.block_choosing_first(body, spent, out)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::Optional { decider, body } => {
                let player = self.certain_player(decider).map(|p| self.fetch(p));
                let player_ref = decider;
                let decider = planned::planned_read(player, "asker.controller", decider);
                let id = self.fresh();
                let (open, spent) = lane_k::once_a_turn(self.do_once.take(), id);
                let open = open
                    + &lane_k::asked_if(player_ref, id, body)
                    + &self.asked_if_there(body)
                    + &self.top_card_offered(id, body, out)?;
                writeln!(out, "let decider_{id} = {decider}; if {open}cantrip_mechanics::effects::ask_may(g, decider_{id}, asker.source, Purpose::Optional)? {{{spent}").unwrap();
                self.block(body, out, true)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            // "For each land, destroy that land unless any player pays 1 life": all
            // the payments come first, and what no one paid for is then done at once.
            Instruction::ForEachObject {
                objects,
                binding,
                body: each,
            } if lane_l::paid_for_each(each).is_some_and(|(paid, _)| {
                format!("{:?}", &body.instructions[i + 1..])
                    .contains(&format!("Unpaid(ResultId({}))", paid.0))
            }) =>
            {
                let (paid, offered) = lane_l::paid_for_each(each).expect("just matched");
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let mut unpaid_{}: Vec<ObjectId> = Vec::new(); for bound_{} in {objects} {{", paid.0, binding.0).unwrap();
                self.bound_objects.insert(binding.0);
                let scope = self.results();
                let mut asked = each.clone();
                asked.instructions = vec![offered];
                let compiled = self.program(&asked, out, false);
                let spent = self.performed.get(&paid.0).cloned();
                self.restore_results(scope);
                self.bound_objects.remove(&binding.0);
                compiled?;
                let spent = spent.ok_or_else(|| gap("object collection Unpaid"))?;
                writeln!(out, "if !{spent} {{ unpaid_{}.push(bound_{}); }} }}", paid.0, binding.0).unwrap();
                self.unpaid.insert(paid.0);
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
                // "Each opponent discards a card, then each who didn't discard ...":
                // together (CR 101.4), and what they discarded read by whose it was.
                let asked = |i: &Instruction| format!("{i:?}").contains("Earlier { what: Discarded");
                let together = self
                    .after
                    .iter()
                    .any(asked)
                    .then(|| lane_m::each_discards(players, binding, steps))
                    .flatten();
                if let Some(together) = together {
                    self.note_step(&together);
                    if let Instruction::Effect { operation, result } = &together {
                        return self.operation(operation, result, out);
                    }
                }
                let split = lane_m::chosen_then_moved(steps, 1_000_000 + i as u32);
                let own = lane_i::own_turns(split.as_ref().unwrap_or(steps), binding);
                let each = lane_i::EachPlayer {
                    players,
                    binding,
                    starting_with: starting_with.as_ref(),
                    in_chosen_direction: *in_chosen_direction,
                    body: own.as_ref().or(split.as_ref()).unwrap_or(steps),
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
        let rule = std::mem::take(&mut self.choice_rule);
        if let Count::PerPlayer { players, each } = count {
            if !rule.is_empty() {
                return Err(gap("operation ChangeZone"));
            }
            return self.choose_from_each(player, objects, players, each, result, purpose, out);
        }
        if self.from_outside(objects) {
            self.notes.outside.insert(result.0);
            if matches!(objects, ObjectSet::Union(_)) {
                self.notes.outside_shown.insert(result.0);
            }
        }
        // "Choose exactly two": fewer can't be chosen, and nothing is.
        let exactly = match count {
            Count::Exactly(Expr::Constant(n)) | Count::AsManyAsCan(Expr::Constant(n))
                if self
                    .line_text()
                    .is_some_and(|line| line.contains("hoose exactly")) =>
            {
                Some(*n)
            }
            _ => None,
        };
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
        let lone = match exactly {
            Some(n) => format!("if ({local}_candidates.len() as i64) < {n} {{ Vec::new() }} else "),
            None => lone,
        };
        writeln!(out, "let {local}_player = {}; let {local}_candidates = {}; let {local}_count = {}; let {local} = {lone}{{ cantrip_mechanics::choose::select(g, {local}_player, asker.source, Purpose::{purpose}, {local}_candidates, cantrip_mechanics::select::Selection::of({local}_count, &[]){rule})? }};", self.fetch(self.certain_player(player)?), self.fetch(self.objects(objects)?), self.fetch(count)).unwrap();
        self.open_guard(&local, out);
        if matches!(player, PlayerRef::OneOf(_)) {
            self.notes.performers.insert(result.0, format!("Some({local}_player)"));
        }
        if let Some(quality) = lane_n::quality_chosen(objects, self.line_text()) {
            self.damage_sources.insert(local.clone(), quality.clone());
        }
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
        if matches!(op, Operation::Move { .. } | Operation::ChangeZone { .. }) {
            let words = format!("{op:?}");
            self.exiled_piles |= words.contains("ExilePile") || words.contains("FaceUpPile") || words.contains("FaceDownPile");
        }
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
            // "For each ..., add one mana of any type that land produced": each mana its own type, in any combination of
            // the types the tap produced (Sasaya's Essence's rulings), whatever the data's choice timing says.
            Operation::AddMana {
                players: _,
                mana: ManaProduction::Repeat { amount, production, .. },
                restriction: None,
            } if self.stack_item && matches!(**production, ManaProduction::SameTypeAsProduced) => {
                let times = self.fetch(self.number(amount)?.amount(u32::MAX.into(), "u32")?);
                writeln!(out, "for _ in 0..({times}) {{").unwrap();
                self.same_type_as_produced(out);
                writeln!(out, "}}").unwrap();
            }
            Operation::AddMana {
                players,
                mana: ManaProduction::ColorsAmong { objects, each },
                restriction: None,
            } => self.colors_among(id, players, objects, *each, None, out)?,
            // "Add three mana in any combination of its colors" (Omnath, Locus of
            // All): each a color among them, chosen one at a time (CR 106.1a).
            Operation::AddMana {
                players,
                mana:
                    ManaProduction::Repeat {
                        amount,
                        production,
                        choice: ManaChoiceTiming::EachUnit,
                    },
                restriction: None,
            } if matches!(**production, ManaProduction::ColorsAmong { each: false, .. }) => {
                let ManaProduction::ColorsAmong { objects, .. } = &**production else {
                    unreachable!()
                };
                let times = self.fetch(self.number(amount)?.code);
                self.colors_among(id, players, objects, false, Some(times), out)?
            }
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
                let several = !matches!(players, PlayerSet::One(_));
                let players = self.fetch(self.players(players)?);
                writeln!(
                    out,
                    "let amount_{id} = {amount}; let players_{id} = {players};"
                )
                .unwrap();
                // More than one draws: in turn order from the active player (121.2c).
                if several && matches!(op, Operation::Draw { .. }) && !players.contains("apnap") {
                    writeln!(out, "let drawing_{id}: Vec<PlayerId> = players_{id}.into_iter().collect(); let players_{id}: Vec<PlayerId> = g.state().apnap().filter(|p| drawing_{id}.contains(p)).collect();").unwrap();
                }
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
                if !*random && lane_i::whole_hand(amount, players) {
                    writeln!(out, "let players_{id} = {}; let mut cards_{id} = Vec::new(); for player in players_{id} {{ cards_{id}.extend(g.state().zone(Zone::hand(player)).to_vec()); }} let discarded_{id} = g.discard_together(&cards_{id});", self.fetch(self.players(players)?)).unwrap();
                    self.moved.insert(result.0, format!("discarded_{id}"));
                    self.shares
                        .insert(result.0, (format!("discarded_{id}"), Share::Owner));
                    self.left_behind(result, format!("discarded_{id}.clone()"));
                } else if *random {
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
                    // Each chooses in turn order from the active player (101.4).
                    let order = match players {
                        PlayerSet::One(_) => format!("players_{id}"),
                        _ => format!("g.state().apnap().filter(|p| players_{id}.contains(p)).collect::<Vec<PlayerId>>()"),
                    };
                    writeln!(out,"let amount_{id} = {}; let players_{id}: Vec<PlayerId> = {}.into_iter().collect(); let mut cards_{id} = Vec::new(); for player in {order} {{ cards_{id}.extend({choice}); }} let discarded_{id} = g.discard_together(&cards_{id});",self.fetch(number),self.fetch(self.players(players)?)).unwrap();
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
                // "If it dies this way": destroyed and sent elsewhere it didn't die (700.4).
                let died = self.line_text().is_some_and(|line| {
                    line.contains("dies this way") || line.contains("die this way")
                });
                self.left_behind(result, match died {
                    true => format!("objects_{id}.iter().copied().filter(|&o| q.history().all().iter().any(|r| matches!(r.event, Event::ZoneChange {{ old, to, .. }} if old == o && to.is(ZoneKind::Graveyard)))).collect::<Vec<ObjectId>>()"),
                    false => gone(&format!("objects_{id}")),
                });
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
                    self.hurt_players.insert(result.0, format!("hurt_{id}"));
                    (
                        format!("let mut done_{id}: i64 = 0; let mut dealt_{id}: Vec<ObjectId> = Vec::new(); let mut hurt_{id}: Vec<PlayerId> = Vec::new(); let before_{id} = g.history().head() as usize; "),
                        format!("done_{id} += g.damage_together(&batch).iter().map(|&n| i64::from(n)).sum::<i64>(); dealt_{id}.extend(cantrip_mechanics::history::permanents_dealt_damage_by(g.history(), source, before_{id})); for r in &g.history().all()[before_{id}..] {{ if let Event::Damage {{ source: s, target: Target::Player(p), amount, .. }} = r.event {{ if s == source && amount > 0 && !hurt_{id}.contains(&p) {{ hurt_{id}.push(p); }} }} }}"),
                    )
                } else if lane_n::read_as_objects(&self.after, result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                    self.moved.insert(result.0, format!("dealt_{id}"));
                    (
                        format!("let mut done_{id}: i64 = 0; let mut dealt_{id}: Vec<ObjectId> = Vec::new(); "),
                        format!("let dealt = g.damage_together(&batch); done_{id} += dealt.iter().map(|&n| i64::from(n)).sum::<i64>(); dealt_{id}.extend(batch.iter().zip(&dealt).filter(|(_, n)| **n > 0).filter_map(|(d, _)| d.1.object()));"),
                    )
                } else if self.read_later(result)
                    && self.after.iter().any(|i| format!("{i:?}").contains(&format!("ClampedDamage(ResultId({}))", result.0)))
                {
                    // "But not more life than the player's life total before the
                    // damage was dealt, the planeswalker's loyalty before the damage
                    // was dealt, or the creature's toughness" (Drain Life): each
                    // recipient's damage, no more than that.
                    self.performed.insert(result.0, format!("done_{id}"));
                    let clamped = format!("clamped_damage_{}", result.0);
                    (
                        format!("let mut done_{id}: i64 = 0; let mut {clamped}: i64 = 0; "),
                        format!("let caps: Vec<i64> = batch.iter().map(|&(_, t, _)| match t {{ Target::Player(p) => i64::from(g.state().player(p).life).max(0), Target::Object(o) if g.chars(o).is(CardType::Planeswalker) => i64::from(g.state().obj(o).counters(CounterKind::Loyalty)).max(0), Target::Object(o) => i64::from(g.chars(o).toughness.unwrap_or(0)).max(0), _ => 0 }}).collect(); let dealt = g.damage_together(&batch); done_{id} += dealt.iter().map(|&n| i64::from(n)).sum::<i64>(); {clamped} += dealt.iter().zip(&caps).map(|(&n, &cap)| i64::from(n).min(cap)).sum::<i64>();"),
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
                // CR 120.10, read from the damage events: what was dealt, to whom
                // it was dealt, a redirection and a prevention included.
                let (declare, deal) = if lane_n::reads_excess(&self.after, result) {
                    let aimed = match recipients {
                        Recipients::Objects(ObjectSet::One(object)) => Some(format!("{object:?}")),
                        _ => None,
                    };
                    self.excess.insert(result.0, (format!("excess_{id}"), format!("recipients_{id}"), aimed));
                    (
                        format!("{declare}let mut excess_{id}: Vec<(Target, u32)> = Vec::new(); let since_{id} = g.history().head() as usize; "),
                        format!("{deal} excess_{id}.extend(g.history().all()[since_{id}..].iter().filter_map(|r| match r.event {{ Event::Damage {{ source: s, target, recipient_excess, .. }} if s == source => Some((target, recipient_excess)), _ => None }}));"),
                    )
                } else {
                    (declare, deal)
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
            // "For each opponent who does, put a counter on each": the instruction
            // again for each, not one instruction of more counters.
            Operation::PutCounters { placements }
                if matches!(placements.as_slice(), [CounterPlacement {
                    amount: Expr::CountPlayers(PlayerSet::Matching { condition, .. }),
                    selection: None,
                    ..
                }] if matches!(**condition, Condition::PlayerTook { .. })) =>
            {
                let times = self.number(&placements[0].amount)?.amount(u32::MAX.into(), "u32")?;
                let mut once = placements[0].clone();
                once.amount = Expr::Constant(1);
                writeln!(out, "let times_{id} = {}; for _ in 0..times_{id} {{", self.fetch(times)).unwrap();
                self.operation(&Operation::PutCounters { placements: vec![once] }, result, out)?;
                writeln!(out, "}}").unwrap();
                self.selections.remove(&result.0);
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
                if let Some((local, by, own)) = self.notes.gathering_counters.clone() {
                    let own = match own {
                        true => format!(".filter(|c| g.state().obj(c.0).controller == {by})"),
                        false => String::new(),
                    };
                    writeln!(out, "gathered_{local}.extend(counters_{id}.iter(){own}.map(|c| ({by}, c.0, c.1, c.2)));").unwrap();
                    return Ok(());
                }
                writeln!(out, "let had_{id}: Vec<_> = counters_{id}.iter().map(|c| g.state().obj(c.0).counters(c.1)).collect(); g.add_counters_together(&counters_{id}); let applied_{id}: Vec<ObjectId> = counters_{id}.iter().zip(had_{id}).filter(|(c, had)| g.state().obj(c.0).counters(c.1) > *had).map(|(c, _)| c.0).collect();").unwrap();
                self.selections.insert(result.0, format!("applied_{id}"));
            }
            Operation::RemoveCounters { removals } => {
                self.remove_counters(id, removals, result, out)?
            }
            // A host no longer a legal target has nothing attached to it (608.2b), and
            // what's already on it doesn't become attached (701.3b).
            Operation::Attach { attachments, host } => {
                let attachments = self.acted_on(attachments)?;
                let host = self.fetch(self.damage_source(host)?);
                if self.aura_puts_its_host() && attachments.starts_with("{ let q: &dyn Query = g; (Some(asker.source)") && host.contains("moved_") && !self.read_later(result) {
                    writeln!(out, "// Attached as it entered with this Aura.").unwrap();
                    return Ok(());
                }
                let (flag, attach) = if self.read_later(result) {
                    self.performed.insert(result.0, format!("attached_{id}"));
                    (
                        format!("let mut attached_{id} = false; "),
                        format!("attached_{id} |= g.attach(attachment, host);"),
                    )
                } else {
                    (String::new(), "g.attach(attachment, host);".to_string())
                };
                writeln!(out, "{flag}let attachments_{id} = {attachments}; let host_{id} = {host}; if let Some(host) = host_{id} {{ cantrip_api::instruction(g, |g| {{ for attachment in attachments_{id} {{ {attach} }} }}); }}").unwrap();
            }
            Operation::CreateRegenerationShields { objects } => {
                writeln!(out, "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ g.add_effect(asker.source, object, EffectKind::RegenerationShield, Duration::EndOfTurn); }} }});", self.acted_on(objects)?).unwrap();
            }
            Operation::Reveal { objects } => {
                self.revealed = true;
                if self.from_outside(objects) {
                    self.notes.outside.insert(result.0);
                }
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
            } if self.stack_targets(objects).is_some() => {
                let read = self.read_later(result)
                    || format!("{:?}", self.after).contains("Earlier { what: Countered");
                match read {
                    true => self.counter_reporting(id, objects, result, out),
                    false => self.counter_on_stack(id, objects, out),
                }
            }
            Operation::Counter {
                objects:
                    ObjectSet::Zone {
                        scope: ZoneScope::Shared(EventZone::Stack),
                        kind: ObjectKind::Any,
                        filter: ObjectFilter::ControlledBy(players),
                    },
                destination: Destination::OwnersGraveyard,
            } => self.counter_all_on_stack(id, players, result, out)?,
            // "On your choice of the top or bottom": the question has no one in the data
            // to ask, and the card says who.
            Operation::Counter {
                objects,
                destination: Destination::OwnersLibraryTopOrBottom,
            } if self.line_text().is_some_and(|line| line.contains("your choice of the top or bottom")) => {
                writeln!(self.functions, "static TOP_OR_BOTTOM_{id}: Question = Question::new(\"PutOnTop\", \"put it on top of its owner's library?\", \"If you don't, it goes on the bottom\", \"Top\", \"Bottom\");").unwrap();
                writeln!(out, "let objects_{id} = {}; let top_{id} = cantrip_mechanics::effects::ask_may(g, asker.controller, asker.source, Purpose::Ask(&TOP_OR_BOTTOM_{id}))?; cantrip_api::instruction(g, |g| {{ for spell in objects_{id} {{ g.counter_to(spell, if top_{id} {{ CounterTo::LibraryTop }} else {{ CounterTo::LibraryBottom }}); }} }});", self.fetch(self.objects(objects)?)).unwrap();
            }
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
                let read = format!("{:?}", self.after).contains("Earlier { what: Countered");
                let keep = match read {
                    true => format!("let before_{id} = objects_{id}.clone(); "),
                    false => String::new(),
                };
                writeln!(out, "let objects_{id} = {}; {keep}cantrip_api::instruction(g, |g| {{ for spell in objects_{id} {{ {call} }} }});", self.fetch(self.objects(objects)?)).unwrap();
                if read {
                    self.left_behind(result, format!("before_{id}.clone()"));
                }
            }
            Operation::CreateContinuousEffect {
                source,
                affected,
                changes,
                duration,
                evaluation,
            } => {
                if self.flashback_until(id, affected, changes, duration, out)?
                    || self.play_this_until(id, affected, changes, duration, out)?
                {
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
                ) || matches!(
                    // "Each creature dealt damage this way": done, so it gains no one.
                    affected,
                    EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::All(filters)))
                        if filters.iter().any(|f| matches!(f, ObjectFilter::DamagedBy(dealt) if self.moved.contains_key(&dealt.0)))
                );
                // "Each attacking creature doesn't untap during its controller's
                // next untap step": a set with no one controller has no one untap
                // step to wait for, and the words are about each object, so they
                // are the ones named as it resolves (Tangle's ruling).
                let closed = closed || lane_o::untaps_by_their_controllers(affected, duration);
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
                let call = self.flip_call()?;
                writeln!(
                    out,
                    "let player_{id} = {}; let flip_{id} = g.{call}(player_{id});",
                    self.fetch(self.certain_player(player)?)
                )
                .unwrap();
                self.flips.insert(result.0, format!("flip_{id}"));
                self.coins = Some((
                    format!("i64::from(flip_{id})"),
                    format!("i64::from(!flip_{id})"),
                ));
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
                let mut each_out = String::new();
                let each = match &mut event {
                    EventSpec::ZoneChange { objects, .. } => {
                        self.about_each_damaged(objects, true, &mut each_out)
                    }
                    EventSpec::Object { objects, .. } => {
                        self.about_each_damaged(objects, false, &mut each_out)
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
                    &mut each_out,
                );
                self.end_each_damaged(each, &mut each_out);
                out.push_str(&lane_n::one_instruction_for_each(each, each_out));
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
                let mut each_out = String::new();
                let each = match &mut event {
                    ReplacementEvent::ZoneChange { objects, .. } => {
                        self.about_each_damaged(objects, true, &mut each_out)
                    }
                    ReplacementEvent::Object { objects, .. } => {
                        self.about_each_damaged(objects, false, &mut each_out)
                    }
                    _ => None,
                };
                let made = self.create_replacement(&event, limit, duration, body, &mut each_out);
                self.end_each_damaged(each, &mut each_out);
                out.push_str(&lane_n::one_instruction_for_each(each, each_out));
                made?
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::PlayerAction { .. }]) => {
                self.player_action(id, player, &cost.parts[0], true, result, out)?
            }
            // "You may put a creature card from your hand onto the battlefield tapped
            // and attacking" (CR 508.4): it's asked after the card what it attacks.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if self.asking.is_none() && arrives_attacking_from_hand(player, cost).is_some() => {
                let filter = arrives_attacking_from_hand(player, cost).expect("just seen");
                let payer = self.fetch(self.certain_player(player)?);
                let test = self.filter(filter, &Candidate::of("id"))?;
                writeln!(out, "let payer_{id} = {payer}; let put_{id}: Vec<ObjectId> = cantrip_mechanics::hand::may_put_onto_battlefield_attacking(g, payer_{id}, asker.source, |g, id| {{ let q: &dyn Query = g; {test} }})?.into_iter().collect(); let paid_{id} = !put_{id}.is_empty();").unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
                self.moved.insert(result.0, format!("put_{id}"));
            }
            // "You may tap it": what can't be tapped can't be paid (CR 118.3), and
            // "if you don't" holds of that too.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if self.asking.is_none() && taps_one_object(cost).is_some() => {
                let object = taps_one_object(cost).expect("just seen");
                let payer = self.fetch(self.certain_player(player)?);
                let object = self.fetch(self.object(object)?);
                writeln!(
                    out,
                    "let payer_{id} = {payer}; let paid_{id} = match {object}.filter(|&o| g.state().obj(o).zone.is(ZoneKind::Battlefield) && !g.state().obj(o).tapped) {{ Some(o) if cantrip_mechanics::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)? => {{ g.tap(o); true }} _ => false }};"
                )
                .unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
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
                self.notes.acting = Some(format!("Some(payer_{id})"));
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
            // "You may exile all cards from your hand face down": a cost only in that
            // "if you do" reads it, and "all other cards" leaves out what it took.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::Move {
                selections,
                destination: Destination::Exile,
                entry,
                face_down: true,
            }] if entry.states.is_empty() && entry.counters.is_empty() && !entry.tapped
                && matches!(selections.as_slice(), [CostObjectSelection { choose: None, .. }])) =>
            {
                let [CostPart::Move { selections, .. }] = cost.parts.as_slice() else {
                    return Err(gap("cost part Move"));
                };
                let payer = self.fetch(self.certain_player(player)?);
                let cards = self.fetch(self.objects(&selections[0].objects)?);
                let by = self.fetch(self.linked().to_string());
                writeln!(out, "let payer_{id} = {payer}; let cards_{id}: Vec<ObjectId> = {cards}; let paid_{id} = cantrip_mechanics::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)?; let moved_{id}: Vec<ObjectId> = if paid_{id} {{ let by = {by}; g.exile_face_down(&cards_{id}, Some(by)) }} else {{ Vec::new() }};").unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                self.before.insert(result.0, format!("moved_{id}.clone()"));
                self.performed.insert(result.0, format!("paid_{id}"));
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
                let owed = self.fetch(self.number(x)?.amount(u32::MAX.into(), "u32")?);
                self.helpers.insert("pay_question");
                let payer = self.fetch(self.certain_player(player)?);
                writeln!(out, "let payer_{id} = {payer}; let owed_{id}: u32 = {owed}; let base_{id} = {base}; let price_{id} = ManaCost {{ generic: base_{id}.generic.saturating_add(owed_{id}.saturating_mul(u32::from(base_{id}.x))), x: 0, ..base_{id} }}; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, Cost::mana(price_{id}), &PAY_QUESTION)?;").unwrap();
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
            } if lane_g::exile_beside_a_price(cost).is_some() => {
                self.pay_and_exile(id, player, cost, result, out)?
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if lane_g::discards_hand_and_draws(cost) => {
                self.discard_hand_and_draw(id, player, cost, result, out)?
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if lane_g::untaps_named_objects(cost).is_some() => {
                let objects = lane_g::untaps_named_objects(cost).expect("just seen");
                self.may_untap(id, player, objects, result, out)?
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::EnergyOneOrMore]) => {
                self.may_pay_energy(id, player, result, out)?
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(
                cost.parts.as_slice(),
                [CostPart::ManaXBounded { mana, comparison: Comparison::LessOrEqual, .. }]
                    if matches!(mana.as_slice(), [ManaSymbol::X])
            ) =>
            {
                let [CostPart::ManaXBounded { value, .. }] = cost.parts.as_slice() else {
                    return Err(gap("cost part ManaXBounded"));
                };
                self.may_pay_bounded_x(id, player, value, result, out)?
            }
            // "You may unattach an Equipment from a creature you control. If you
            // do, tap that creature": the payer picks one attached to such a host,
            // or none, and it is unattached (CR 701.3d); "that creature" is the host.
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(cost.parts.as_slice(), [CostPart::UnattachFrom { .. }]) => {
                let [CostPart::UnattachFrom { attachments, hosts }] = cost.parts.as_slice() else {
                    unreachable!("an unattach");
                };
                let payer = self.fetch(self.certain_player(player)?);
                let attachments = self.fetch(self.objects(attachments)?);
                let hosts = self.fetch(self.objects(hosts)?);
                writeln!(out, "let payer_{id} = {payer}; let hosts_{id}: Vec<ObjectId> = {hosts}; let offered_{id}: Vec<ObjectId> = {attachments}.into_iter().filter(|&e| g.state().obj(e).attached_to.is_some_and(|h| hosts_{id}.contains(&h))).collect(); let picked_{id} = cantrip_mechanics::choose::cards(g, payer_{id}, asker.source, Purpose::Optional, offered_{id}, 1, true)?; let mut unattached_{id}: Vec<ObjectId> = Vec::new(); for e in picked_{id} {{ if let Some(h) = g.state().obj(e).attached_to {{ g.unattach(e); unattached_{id}.push(h); }} }} let paid_{id} = !unattached_{id}.is_empty();").unwrap();
                self.selections.insert(result.0, format!("unattached_{id}"));
                self.performed.insert(result.0, format!("paid_{id}"));
            }
            // "You may suspect one of the other creatures. If you do" (Frantic
            // Scapegoat): the payer picks one or none, and it becomes suspected
            // (CR 701.60a).
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } if matches!(
                cost.parts.as_slice(),
                [CostPart::ObjectAction { action: CostObjectAction::Suspect, .. }]
            ) =>
            {
                let [CostPart::ObjectAction { objects, .. }] = cost.parts.as_slice() else {
                    unreachable!("a suspect");
                };
                let payer = self.fetch(self.certain_player(player)?);
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let payer_{id} = {payer}; let offered_{id}: Vec<ObjectId> = {objects}.into_iter().filter(|&o| g.state().obj(o).zone.kind() == ZoneKind::Battlefield).collect(); let suspected_{id} = cantrip_mechanics::choose::cards(g, payer_{id}, asker.source, Purpose::Optional, offered_{id}, 1, true)?; for &o in &suspected_{id} {{ g.suspect(o); }} let paid_{id} = !suspected_{id}.is_empty();").unwrap();
                self.selections.insert(result.0, format!("suspected_{id}"));
                self.performed.insert(result.0, format!("paid_{id}"));
            }
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } => {
                let discard = self.paid_discard(cost, result);
                if discard.is_some() {
                    writeln!(out, "let mark_{id} = {{ let q: &dyn Query = g; q.history().all().len() }};").unwrap();
                }
                let asked = self.ask_to_pay(id, player, cost)?;
                writeln!(out, "{asked}").unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
                match discard {
                    Some(n) => self.discards_paid(id, n, result, out),
                    // "You may reveal that card ... add three mana in any combination
                    // of its colors" (Omnath, Locus of All): what was revealed is the
                    // card still on top.
                    None if matches!(cost.parts.as_slice(), [CostPart::RevealTop(_)])
                        && lane_n::read_as_objects(&self.after, result) =>
                    {
                        writeln!(out, "let revealed_{id}: Vec<ObjectId> = match paid_{id} {{ true => g.state().zone(Zone::library(payer_{id})).first().copied().into_iter().collect(), false => Vec::new() }};").unwrap();
                        self.moved.insert(result.0, format!("revealed_{id}"));
                        self.before.insert(result.0, format!("revealed_{id}.clone()"));
                    }
                    None => self.objects_that_paid(id, result, out),
                }
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
                // A cost of nothing (Tectonic Giant's "choose one of them", which
                // the data words as a cost and the choice before it pays) is paid.
                if cost.parts.is_empty() {
                    writeln!(out, "let paid_{id} = true;").unwrap();
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
                            (Destination::Battlefield(_) | Destination::OwnersBattlefield, Ok(moved)) => writeln!(out, "let paid_{id} = !{moved}.is_empty();").unwrap(),
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
            self.performed.insert(result.0, format!("paid_{id}"));
            // "The blighted creature": the effect says which.
            if self.reads_paid_objects(result) {
                writeln!(out, "let payer_{id} = {payer}; let blighted_{id}: Vec<ObjectId> = cantrip_mechanics::blight::may_in_effect(g, payer_{id}, asker.source, {n}, &PAY_QUESTION)?.into_iter().collect(); let paid_{id} = !blighted_{id}.is_empty();").unwrap();
                self.moved.insert(result.0, format!("blighted_{id}"));
                self.before.insert(result.0, format!("blighted_{id}.clone()"));
                return Ok(());
            }
            writeln!(out, "let payer_{id} = {payer}; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, cantrip_mechanics::blight::cost({n}), &PAY_QUESTION)?;").unwrap();
            self.objects_that_paid(id, result, out);
            return Ok(());
        }
        // "You may pay one or more": paying none is declining, and the amount paid
        // is what the rest reads, not whether it was.
        if let (true, [Instruction::Effect { operation: Operation::PayAnyAmount { actor, resource }, .. }]) =
            (optional, body.instructions.as_slice())
        {
            return self.pay_any_amount(id, actor, resource, result, out);
        }
        let willing = if optional {
            writeln!(
                out,
                "let payer_{id} = {};",
                self.fetch(self.certain_player(player)?)
            )
            .unwrap();
            let offered = self.top_card_offered(id, body, out)? + &faces::reveal_offered(self, body);
            format!("{offered}cantrip_mechanics::effects::ask_may(g, payer_{id}, asker.source, Purpose::Optional)?")
        } else {
            "true".to_string()
        };
        // "You may cast it. If you don't": willing isn't cast, as a card that can't be.
        let plays = optional && playing::may_only_play(player, body);
        let binding = if plays { "let mut" } else { "let" };
        let mark = out.len();
        writeln!(out, "{binding} paid_{id} = {willing}; if paid_{id} {{").unwrap();
        let mut only = body;
        while let [Instruction::Group(group)] = only.instructions.as_slice() {
            only = group;
        }
        match only.instructions.as_slice() {
            [Instruction::Effect { operation, result: played }] if plays => {
                self.play_cards(operation, false, out)?;
                let list = self.played.clone().expect("just played");
                writeln!(out, "paid_{id} = !{list}.is_empty();").unwrap();
                self.report_played(played);
            }
            _ => self.block(body, out, true)?,
        }
        writeln!(out, "}}").unwrap();
        // "Choose an opponent at random that ... If you can't choose an opponent
        // this way": a choice at random among no one is a cost not paid, and the
        // rest of the ability still happens (Territorial Hellkite).
        let none_left = ".is_empty() { return Ok(()); } let chosen_";
        let head = format!("let paid_{id} = true; if paid_{id} {{");
        if !optional && out[mark..].starts_with(&head) && out[mark..].matches(none_left).count() == 1 {
            let tail = out.split_off(mark);
            let tail = tail
                .replacen(&head, &format!("let mut paid_{id} = false; 'paid_{id}: {{"), 1)
                .replacen(none_left, &format!(".is_empty() {{ break 'paid_{id}; }} let chosen_"), 1);
            let tail = tail.strip_suffix("}\n").expect("the block just closed");
            write!(out, "{tail}paid_{id} = true;\n}}\n").unwrap();
        }
        self.performed.insert(result.0, format!("paid_{id}"));
        self.action_paid(body, result);
        Ok(())
    }
    pub(super) fn left_behind(&mut self, result: &ResultId, list: String) {
        self.before.insert(result.0, list);
    }
    pub(super) fn reset_results(&mut self) {
        self.countered.clear();
        self.selections.clear();
        self.unchosen.clear();
        self.reveal_matches.clear();
        self.known_one.clear();
        self.revealed = false;
        self.creations.clear();
        self.moved.clear();
        self.before.clear();
        self.performed.clear();
        self.excess.clear();
        self.unpaid.clear();
        self.hurt_players.clear();
        self.coins = None;
        self.coin_turns = None;
        self.coin_objects = None;
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
                recipe: TokenRecipe::Copy { .. } | TokenRecipe::CopyOne { .. },
                ..
            }]
        );
        // `create_token_copies_together` makes no tapped token, and a call for each
        // original would make a batch of each (CR 603.2c).
        let several = matches!(
            tokens,
            [TokenSpec {
                recipe: TokenRecipe::CopyEach { .. },
                ..
            }]
        );
        if several && entry.counters.is_empty() && entry.tapped {
            return Err(gap("tapped token copies of several objects at once"));
        }
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
        // A text change reaches a token's colors and creature types (612.2a), and
        // a Treasure has none: it takes the card's words, unchanged, beside the
        // tokens the card's own text makes.
        let unworded = mixed
            && tokens.iter().all(|t| {
                matches!(t.words, TokenWords::CreatingEffect)
                    || matches!(&t.recipe, TokenRecipe::Defined(definition)
                        if definition.colors.is_empty()
                            && !definition.chosen_color
                            && !definition.chosen_creature_type
                            && !definition.card_types.iter().any(|t| matches!(t, crate::mtg_types::CardType::Creature)))
            });
        let words_of = |words: &TokenWords| match words {
            TokenWords::Rules => "Words::NONE",
            TokenWords::CreatingEffect => "g.words(asker.source)",
        };
        let words = match unworded {
            true => words_of(&TokenWords::CreatingEffect),
            false => words_of(first),
        };
        // "A copy of a nonlegendary enchantment you control": the one is chosen
        // as this resolves (608.2d).
        if let (
            [TokenSpec {
                amount,
                recipe:
                    TokenRecipe::CopyOne {
                        objects,
                        adjustments,
                    },
                ..
            }],
            PlayerSet::One(player),
        ) = (tokens, players)
        {
            if !entry.counters.is_empty() {
                return Err(gap("token copy entering with counters"));
            }
            let amount = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
            writeln!(out, "let player_{id} = {}; let among_{id} = {}; let chosen_{id} = cantrip_mechanics::choose::select(g, player_{id}, asker.source, Purpose::CopyWhich, among_{id}, cantrip_mechanics::select::cards(cantrip_mechanics::select::exactly(1)))?.first().copied();", self.fetch(self.certain_player(player)?), self.fetch(self.objects(objects)?)).unwrap();
            let values = self.copy_values(&format!("chosen_{id}"), adjustments)?;
            writeln!(out, "let values_{id} = {values}; let count_{id} = {}; let created_{id} = match values_{id} {{ Some(values) => g.create_token_copies_tapped(values, player_{id}, count_{id}, {})?, None => Vec::new() }};", self.fetch(amount), entry.tapped).unwrap();
            self.token_state(id, entry, out)?;
            self.creations.insert(result.0, format!("created_{id}"));
            return Ok(());
        }
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
                writeln!(out, "let original_{id} = {}; let values_{id} = {values}; let count_{id} = {}; let player_{id} = {}; let created_{id} = match values_{id} {{ Some(values) => g.create_token_copies_tapped(values, player_{id}, count_{id}, {})?, None => Vec::new() }};", self.fetch(self.damage_source(object)?), self.fetch(amount), self.fetch(self.certain_player(player)?), entry.tapped).unwrap();
                self.token_state(id, entry, out)?;
                self.creations.insert(result.0, format!("created_{id}"));
                return Ok(());
            }
            writeln!(out, "let original_{id} = {}; let count_{id} = {}; let player_{id} = {}; let created_{id} = match original_{id} {{ Some(original) => g.create_token_copies(original, player_{id}, count_{id})?, None => Vec::new() }};", self.fetch(self.damage_source(object)?), self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
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
            if !entry.counters.is_empty() || entry.tapped || !entry.states.is_empty() {
                return Err(gap("adjusted token copy"));
            }
            let PlayerSet::One(player) = players else {
                if adjustments.iter().any(adjusts) {
                    return Err(gap("adjusted token copy"));
                }
                return self.token_copies_for_each(id, objects, players, result, out);
            };
            let values = self.copy_values("Some(original)", adjustments)?;
            writeln!(out, "let originals_{id} = {}; let player_{id} = {}; let mut copies_{id}: Vec<(Copiable, PlayerId)> = Vec::new(); if let Some(player) = player_{id} {{ for &original in &originals_{id} {{ if let Some(values) = {values} {{ copies_{id}.push((values, player)); }} }} }} let created_{id} = g.create_token_copies_together(&copies_{id})?;", self.fetch(self.objects(objects)?), self.fetch(self.player(player)?)).unwrap();
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
        let creation = if mixed && !unworded {
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
        if let (ObjectSet::Observed(seen), Destination::Exile) = (objects, destination) {
            if self.notes.exiled_going.contains(&seen.0) {
                writeln!(out, "let moved_{id}: Vec<ObjectId> = {};", self.fetch(self.objects(objects)?)).unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                self.left_behind(result, format!("moved_{id}.clone()"));
                return Ok(());
            }
        }
        let alone = self.one_object(objects);
        let shown = self.outside_shown(objects);
        let revealed = self.revealed;
        let chosen = matches!(objects, ObjectSet::Selected(_));
        let targeted = matches!(objects, ObjectSet::Targets(_));
        // Cards of one player's library, which a move to its bottom takes nowhere: "put the rest on the bottom of your
        // library in a random order" of cards revealed from it changes no zone (CR 401.4), and a move would carry them
        // hidden though all players saw them (CR 701.20a).
        let in_library = match objects {
            ObjectSet::Zone {
                scope: ZoneScope::Players { zone: EventZone::Library, players: PlayerSet::One(player) },
                ..
            } => Some(player.clone()),
            _ => None,
        };
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
                    _ if self.line_text().is_some_and(|line| {
                        line.contains(" die")
                            && line.contains("return them to the battlefield")
                            && !line.contains("owner")
                    }) =>
                    {
                        return Err(gap("a return that prints no owner under its owner's control"));
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
                } else if entry.counters.is_empty() && self.aura_puts_its_host() && !entry.tapped {
                    // It enters with this Aura attached (CR 303.4f), so the enchant
                    // ability "put onto the battlefield with [this]" names it.
                    format!("{{ let mut came_{id}: Vec<ObjectId> = Vec::new(); for &o in &objects_{id} {{ came_{id}.push(g.put_onto_battlefield_with_aura(o, {controller}, asker.source)?); }} came_{id}.into_iter().filter(|&o| g.state().obj(o).zone.is(ZoneKind::Battlefield)).collect::<Vec<ObjectId>>() }}")
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
            // "Exile target spell": not countered, so one that can't be goes too (701.6a).
            Destination::Exile
                if self.links_exile
                    && self.stack_item
                    && self.line_text().is_some_and(|line| line.contains("exile target spell")) =>
            {
                format!("{{ let by = g.linked(item); let mut moved: Vec<ObjectId> = Vec::new(); for &spell in &objects_{id} {{ g.remove_spell(spell, CounterTo::Exile); if let Some(exiled) = g.obj(spell).became {{ g.link_exiled(&[exiled], by); moved.push(exiled); }} }} moved }}")
            }
            Destination::Exile if self.links_exile && self.stack_item => {
                format!("{{ let by = g.linked(item); g.exile_with(&objects_{id}, by, false) }}")
            }
            Destination::Exile if self.links_exile && self.pending != Pending::Entering => {
                format!("{{ let by = g.text_linked(asker.source); g.exile_with(&objects_{id}, by, false) }}")
            }
            // As it enters: `Entering::object` is the permanent it will be.
            Destination::Exile if self.links_exile => format!("{{ let by = g.text_linked(asker.source); g.exile_with(&objects_{id}, by, false) }}"),
            Destination::Exile => format!("g.exile_all(&objects_{id})"),
            // `put_into_graveyard` refuses permanents, so name each destination.
            Destination::OwnersGraveyard => format!("{{ let moves: Vec<(ObjectId, Zone)> = objects_{id}.iter().map(|&o| (o, Zone::graveyard(g.state().obj(o).owner))).collect(); let moved = g.move_together(&moves, MoveReason::Other); moved.into_iter().filter(|&o| g.state().obj(o).zone.kind() == ZoneKind::Graveyard).collect::<Vec<ObjectId>>() }}"),
            // Chosen cards put on the bottom together are ordered too (CR 401.4; Valakut Awakening).
            Destination::OwnersLibraryBottom if chosen => lane_k::in_an_order_asked(id, destination),
            Destination::OwnersLibraryTop | Destination::OwnersLibraryBottom if targeted => {
                lane_k::in_an_order_asked(id, destination)
            }
            Destination::OwnersLibraryTop => library("LibraryPlace::Top".into()),
            Destination::OwnersLibraryBottom => library("LibraryPlace::Bottom".into()),
            Destination::OwnersLibraryBottomRandom if in_library.is_some() => {
                let player = self.fetch(self.certain_player(in_library.as_ref().expect("a library"))?);
                format!("{{ g.bottom_in_random_order({player}, &objects_{id}); objects_{id}.clone() }}")
            }
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
            [EntryState::AttachedToChosen(filter)]
                if counters.is_empty() && !entry.tapped =>
            {
                let test = self.filter(filter, &Candidate::of("host"))?;
                format!("let aura = g.chars(o).has_subtype(Subtype::Aura); let hosts: Vec<ObjectId> = {{ let q: &dyn Query = g; q.state().battlefield().iter().copied().filter(|&host| {test} && (!aura || could_enchant(q, o, controller, host))).collect() }}; match cantrip_mechanics::choose::card(g, controller, asker.source, Purpose::Attach, hosts)? {{ Some(host) => moved.extend(g.put_onto_battlefield_attached(o, controller, host)?), None if !aura => moved.push(g.put_onto_battlefield(o, controller, false)?), None => {{}} }}")
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
            // "If that card is an enchantment card, it enters tapped and attacking."
            [EntryState::When { filter, entry: then }]
                if counters.is_empty()
                    && !entry.tapped
                    && then.counters.is_empty()
                    && matches!(then.states.as_slice(), [EntryState::Attacking { .. }]) =>
            {
                let test = self.filter(filter, &Candidate::of("o"))?;
                let then = self.arrival(then)?;
                let plain = EntrySpec { tapped: false, counters: Vec::new(), states: Vec::new() };
                let otherwise = self.arrival(&plain)?;
                format!("if {{ let q: &dyn Query = g; {test} }} {{ {then} }} else {{ {otherwise} }}")
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
                    // "You may remove": each is offered, and any number is those taken.
                    if matches!(selection.count, Count::AnyNumber)
                        && lane_k::printed_otherwise(self.line_text(), "you may remove")
                    {
                        writeln!(out, "let among_{id}_{i} = {objects}; let chooser_{id}_{i} = {player}; let mut objects_{id}_{i}: Vec<ObjectId> = Vec::new(); for object in among_{id}_{i} {{ if cantrip_mechanics::effects::ask_may(g, chooser_{id}_{i}, object, Purpose::Optional)? {{ objects_{id}_{i}.push(object); }} }}").unwrap();
                    } else {
                        let count = self.select_count(&selection.count)?;
                        let count = self.fetch(count);
                        writeln!(out, "let among_{id}_{i} = {objects}; let chooser_{id}_{i} = {player}; let count_{id}_{i} = {count}; let objects_{id}_{i} = cantrip_mechanics::choose::select(g, chooser_{id}_{i}, asker.source, Purpose::Choose, among_{id}_{i}, cantrip_mechanics::select::Selection::of(count_{id}_{i}, &[]))?;").unwrap();
                    }
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
        // Kept with the text that read it (CR 607.2d).
        if self.enters_sacrifices && self.pending == Pending::Entering {
            writeln!(out, "entering.chosen_number = Some(sacrificed_{id}.len() as u32);").unwrap();
        }
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
                // Only "exactly n" (asked as "as many as can") takes what is eligible without asking when there are no
                // more than n: a count that is "up to" or "any number" always asks, so its test is not written.
                let can_take_all = matches!(count, Count::Exactly(_) | Count::AsManyAsCan(_) | Count::PerPlayer { .. });
                let count = self.fetch(self.select_count(&count)?);
                // However many they choose, they go in battlefield order.
                let order = match count.contains("AnyNumber") {
                    true => format!("objects_{id}.iter().copied().filter(|o| taken.contains(o)).collect::<Vec<ObjectId>>()"),
                    false => "taken".to_string(),
                };
                let asked = format!("cantrip_mechanics::choose::select(g, player, asker.source, Purpose::Sacrifice, eligible, cantrip_mechanics::select::Selection::of(count_{id}.clone(), &[]))?");
                let taken = match can_take_all {
                    true => format!("match count_{id}.clone() {{ cantrip_mechanics::select::Count::Exactly(n) | cantrip_mechanics::select::Count::AsManyAsCan(n) if eligible.len() <= n as usize => eligible, _ => {asked} }}"),
                    false => asked,
                };
                writeln!(out, "let count_{id} = {count}; for player in players_{id} {{ let eligible: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.controller(o) == player && g.can_sacrifice(player, o)).collect(); let taken = {taken}; for object in {order} {{ picks_{id}.push((player, object)); }} }}").unwrap();
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
            // "Until its controller's next untap step": the untap step is the
            // first step of a turn (CR 502.1), so it ends as that turn begins.
            D::UntilNextUntapStep(player) => format!(
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
    /// "Becomes a Kithkin Scout" replaces its creature types (205.1a), whatever
    /// the source says, unless the card says it keeps them: the types added, as
    /// one set of them where the first was.
    fn becoming_replaces(&self, changes: &[Change]) -> Option<Vec<Change>> {
        let says = |line: &String| {
            line.contains(" becomes a")
                && !line.contains("in addition to")
                && !line.contains("still")
        };
        if !self.lines_said().iter().any(says) {
            return None;
        }
        let mut merged: Vec<Change> = Vec::new();
        let mut at = None;
        for change in changes {
            let Change::CreatureTypes {
                mode: ChangeMode::Add,
                types: CreatureTypeSet::Listed(types),
            } = change
            else {
                merged.push(change.clone());
                continue;
            };
            let first = *at.get_or_insert(merged.len());
            if first == merged.len() {
                merged.push(Change::CreatureTypes {
                    mode: ChangeMode::Set,
                    types: CreatureTypeSet::Listed(types.clone()),
                });
            } else if let Change::CreatureTypes {
                types: CreatureTypeSet::Listed(all),
                ..
            } = &mut merged[first]
            {
                all.extend(types.iter().cloned());
            }
        }
        at.map(|_| merged)
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
        let becoming = self.becoming_replaces(changes);
        let changes = becoming.as_deref().unwrap_or(changes);
        // "Doesn't untap during its controller's next untap step" is no exert
        // (CR 701.43a: "your next untap step", the exerting player's): under a
        // change of control it waits on the new controller's, as the Tamiyo, the
        // Moon Sage ruling has it. `EffectKind::DoesntUntap`, a hole until
        // Cantrip has `lasting-rules`.
        let duration = self.duration(&lane_h::duration_on(affected, duration))?;
        writeln!(
            out,
            "let objects_{id} = {}; let source_{id} = {}; let duration_{id} = {duration};",
            self.acted_on(affected)?,
            self.fetch(self.object(source)?),
        )
        .unwrap();
        let joined = lane_n::power_with_toughness(changes);
        let changes = joined.as_deref().unwrap_or(changes);
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
        let mut each_player = String::new();
        for change in changes {
            // Protection from each of these players, as many as they are (702.16i).
            if let Change::GrantKeyword(Keyword::Protection(quality)) = change {
                if let Protection::Players(players) = &**quality {
                    let local = format!("players_{id}_{}", kinds.len());
                    writeln!(out, "let {local} = {};", self.fetch(self.players(players)?)).unwrap();
                    write!(each_player, "for &player in &{local} {{ g.add_effect(source, object, EffectKind::ProtectionFromPlayer(player), duration_{id}); }} ").unwrap();
                    continue;
                }
            }
            // "If it doesn't have [this ability], it gains that ability": asked of
            // each object as the effect begins, the same ability from any source
            // being the one it has (Musician).
            if let Change::GrantAbilityIfAbsent(ability) = change {
                for (i, reference) in self.granted_refs(ability)?.into_iter().enumerate() {
                    let local = format!("kind_{id}_{}_{i}", kinds.len());
                    writeln!(out, "let {local} = {};", self.fetch(format!("EffectKind::AddAbility({reference})"))).unwrap();
                    write!(each_player, "if !matches!({local}, EffectKind::AddAbility(a) if g.chars(object).abilities.contains(&Some(a))) {{ g.add_effect(source, object, {local}, duration_{id}); }} ").unwrap();
                }
                continue;
            }
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
        writeln!(out, "if let Some(source) = source_{id} {{ cantrip_api::instruction(g, |g| {{ for &object in &objects_{id} {{ {adds}{each_player} }} }}); }}").unwrap();
        // "It gains 'When this token deals combat damage to a player, ... return the
        // exiled card'": the card is linked to the ability as the token has it, by the
        // effect just made (CR 607.5), for that ability's `exiled_with` to find.
        if changes.iter().any(|c| matches!(c, Change::GrantAbility(_))) {
            if let Some(exiled) = self.notes.exiled_for_grant.take() {
                writeln!(out, "if source_{id}.is_some() {{ let exiled_{id}: Vec<ObjectId> = {exiled}; for &object in &objects_{id} {{ let given = g.state().effects.iter().rev().find_map(|e| match e.kind {{ EffectKind::AddAbility(a) if e.affected == object => Some((a.card, e.timestamp)), _ => None }}); if let Some((card, stamp)) = given {{ let text = g.linked_card(card); g.link_exiled(&exiled_{id}, Linked {{ object, text, acquired: Acquired {{ by: AcquiredBy::Grant, stamp, part: 0 }} }}); }} }} }}").unwrap();
            }
        }
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
            // "Landwalk of each of the land types of the sacrificed land": the
            // landwalks its types name when the effect is made, as Cantrip has a
            // landwalk keyword for each basic land type (CR 702.14a).
            Change::GrantKeyword(Keyword::Landwalk(land))
                if matches!(
                    &**land,
                    ObjectFilter::Shares {
                        property: DistinctProperty::LandTypes | DistinctProperty::BasicLandTypes,
                        ..
                    }
                ) =>
            {
                let ObjectFilter::Shares { with, .. } = &**land else {
                    return Err(gap("landwalk of Shares"));
                };
                vec![format!(
                    "{{ let walks: Vec<Keyword> = {}.into_iter().flat_map(|o: ObjectId| q.chars(o).subtypes.iter()).filter_map(Keyword::landwalk).collect(); EffectKind::AddKeywords(Keywords::of(&walks)) }}",
                    self.object(with)?
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
                let keeps = lane_i::artifact_creature(types);
                let types = types
                    .iter()
                    .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                    .collect::<Result<Vec<_>>>()?;
                let kind = if keeps { "AddTypes" } else { "SetTypes" };
                vec![format!(
                    "EffectKind::{kind}(TypeSet::of(&[{}]))",
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
                // "Becomes all colors" names no color word, so no text change
                // touches it (CR 612.2; Tam, Mindful First-Year).
                let all = mask == 31 && self.lines_said().iter().any(|line| line.contains("all colors"));
                let set = match all {
                    true => "ColorSet(31)".to_string(),
                    false => lane_k::word_colors(mask),
                };
                vec![format!("EffectKind::SetColors({set})")]
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
