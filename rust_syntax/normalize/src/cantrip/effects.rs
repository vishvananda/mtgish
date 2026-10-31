//! Resolution: instructions and operations, as the body of an `Effect`.
//! Reads are `fetch`ed into locals before anything changes, so one instruction
//! sees one game state, and each batch goes to the API that keeps it one batch.
use super::*;

/// The prompt a choice asks with, from what its result is for. It is wording: a
/// choice nothing consumes in sight stays a gap rather than asking something odd.
fn purpose(result: &ResultId, rest: &[Instruction]) -> Option<&'static str> {
    let chosen = |objects: &ObjectSet| matches!(objects, ObjectSet::Selected(r) if r.0 == result.0);
    rest.iter().find_map(|instruction| {
        let Instruction::Effect { operation, .. } = instruction else {
            return None;
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
            Operation::Attach { attachments, .. } if chosen(attachments) => "Attach",
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

/// What a result of the search that found it is about to become, for the search
/// prompt. The move itself stays an ordinary later instruction.
fn search_destination(result: &ResultId, rest: &[Instruction]) -> &'static str {
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

fn plain_entry(entry: &EntrySpec) -> bool {
    !entry.tapped && entry.counters.is_empty() && entry.states.is_empty()
}

/// The locals earlier instructions bound, keyed by the result that names them.
pub(super) struct Results {
    selections: BTreeMap<u32, String>,
    creations: BTreeMap<u32, String>,
    moved: BTreeMap<u32, String>,
    performed: BTreeMap<u32, String>,
    values: BTreeMap<u32, String>,
    flips: BTreeMap<u32, String>,
}

impl Compiler {
    /// `Vec<Target>`.
    pub(super) fn recipients(&self, recipients: &Recipients) -> Result<String> {
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
            Recipients::AttackedBy(object) => format!(
                "{{ let attacker = {}; q.state().attackers.iter().find(|a| Some(a.id) == attacker).map(|a| a.target).filter(|t| *t != Target::None).into_iter().collect::<Vec<Target>>() }}",
                self.object(object)?
            ),
        })
    }
    fn target_slot(&self, slot: &Slot) -> Result<usize> {
        self.slots
            .get(&slot.0)
            .map(|(i, _)| *i)
            .ok_or_else(|| gap("target reference"))
    }
    pub(super) fn effect(&mut self, body: &Program, label: &str) -> Result<String> {
        self.reset_results();
        let out = self.effect_body(body)?;
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
        let out = self.effect_body(body)?;
        writeln!(self.functions,"fn {label}(g: &mut dyn Game, item: &StackItem) -> Result<(), Suspend> {{ let asker = &Asker::new(item.object, item.controller); if !{test} {{ return Ok(()); }} {out} Ok(()) }}").unwrap();
        Ok(label.into())
    }
    fn effect_body(&mut self, body: &Program) -> Result<String> {
        let mut out = String::new();
        writeln!(out, "let asker = &Asker::new(item.object, item.controller);").unwrap();
        if !self.slots.is_empty() {
            self.helpers.insert("announced");
        }
        for (i, _) in self.slots.values() {
            writeln!(
                out,
                "let t_{i}_raw = announced(g, item, {i}, false); let t_{i} = announced(g, item, {i}, true);"
            )
            .unwrap();
        }
        let reader = std::mem::replace(&mut self.reader, Reader::Resolution);
        let at = std::mem::replace(&mut self.event_at, EventAt::Item);
        self.program(body, &mut out, true)?;
        self.reader = reader;
        self.event_at = at;
        Ok(out)
    }
    pub(super) fn program(
        &mut self,
        body: &Program,
        out: &mut String,
        boundaries: bool,
    ) -> Result<()> {
        for (i, instruction) in body.instructions.iter().enumerate() {
            self.attempt(format!("instructions[{i}]"), |c| {
                let compiled = c.instruction(body, i, instruction, out);
                if compiled.is_err() {
                    if let Instruction::Effect { result, .. } = instruction {
                        c.failed.insert(result.0);
                    }
                }
                compiled
            });
            if boundaries && self.pending == Pending::Nothing {
                writeln!(out, "g.end_sentence();").unwrap();
            }
        }
        Ok(())
    }
    pub(super) fn results(&self) -> Results {
        Results {
            selections: self.selections.clone(),
            creations: self.creations.clone(),
            moved: self.moved.clone(),
            performed: self.performed.clone(),
            values: self.values.clone(),
            flips: self.flips.clone(),
        }
    }
    pub(super) fn restore_results(&mut self, results: Results) {
        let Results {
            selections,
            creations,
            moved,
            performed,
            values,
            flips,
        } = results;
        self.selections = selections;
        self.creations = creations;
        self.moved = moved;
        self.performed = performed;
        self.values = values;
        self.flips = flips;
    }
    /// A nested block: what it binds is out of Rust's scope once it closes, so
    /// a later reference reports a gap instead of generating a compile error.
    fn block(&mut self, body: &Program, out: &mut String, boundaries: bool) -> Result<()> {
        let scope = self.results();
        self.program(body, out, boundaries)?;
        self.restore_results(scope);
        Ok(())
    }
    fn instruction(
        &mut self,
        body: &Program,
        i: usize,
        instruction: &Instruction,
        out: &mut String,
    ) -> Result<()> {
        match instruction {
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
                if let Some(constraint) = constraints.first() {
                    return Err(gap(format!("selection constraint {}", form(constraint))));
                }
                let purpose = purpose(result, &body.instructions[i + 1..])
                    .ok_or_else(|| gap("resolution choice prompt purpose"))?;
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
                    },
                result,
            } if also.is_empty() => {
                let [choice] = choices.as_slice() else {
                    return Err(gap("search with several find clauses"));
                };
                if let Some(constraint) = choice.constraints.first() {
                    return Err(gap(format!("search constraint {}", form(constraint))));
                }
                let count = self.select_count(&choice.count)?;
                let id = self.fresh();
                let filter = self.filter(&choice.filter, &Candidate::of("id"))?;
                let destination = search_destination(result, &body.instructions[i + 1..]);
                let searcher = self.fetch(self.certain_player(player)?);
                let owner = self.fetch(self.certain_player(library)?);
                let count = self.fetch(count);
                writeln!(out, "let searcher_{id} = {searcher}; let owner_{id} = {owner}; let rule_count_{id} = {count}; let shown_{id} = cantrip_mechanics::library::searched_cards(g, owner_{id}); let can_{id}: Vec<ObjectId> = {{ let q: &dyn Query = g; shown_{id}.iter().copied().filter(|&id| {filter}).collect() }}; let found_{id} = cantrip_mechanics::library::search(g, searcher_{id}, owner_{id}, asker.source, SearchDestination::{destination}, cantrip_mechanics::select::Selection::listed(rule_count_{id}, &shown_{id}, can_{id}))?;").unwrap();
                self.selections.insert(result.0, format!("found_{id}"));
                Ok(())
            }
            Instruction::Effect { operation, result } => self.operation(operation, result, out),
            // One instruction of the card's text: no sentence boundary inside,
            // and its results stay in reach of what follows it.
            Instruction::Group(group) => self.program(group, out, false),
            Instruction::If {
                condition,
                then,
                otherwise,
            } => {
                let condition = self.condition(condition)?;
                writeln!(out, "if {} {{", self.fetch(condition)).unwrap();
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
            Instruction::Optional { decider, body } => {
                let decider = self.fetch(self.certain_player(decider)?);
                let id = self.fresh();
                writeln!(out, "let decider_{id} = {decider}; if cantrip_mechanics::effects::ask_may(g, decider_{id}, asker.source, Purpose::Optional)? {{").unwrap();
                self.block(body, out, true)?;
                writeln!(out, "}}").unwrap();
                Ok(())
            }
            Instruction::ForEachObject {
                objects,
                binding,
                body,
            } => {
                let objects = self.fetch(self.objects(objects)?);
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
                starting_with: None,
                body,
            } => self.for_each_player(players, binding, body, out),
            other => Err(gap(format!("instruction {}", form(other)))),
        }
    }
    /// "Each player does this": each one's choices come first and the actions
    /// happen together (101.4). Only a body whose one action a plain walk in
    /// APNAP order reproduces is generated.
    fn for_each_player(
        &mut self,
        players: &PlayerSet,
        binding: &Binding,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        let [Instruction::Effect { operation, .. }] = body.instructions.as_slice() else {
            return Err(gap("per-player instruction with several steps"));
        };
        if !matches!(
            operation,
            Operation::Draw { .. } | Operation::GainLife { .. } | Operation::LoseLife { .. }
        ) {
            return Err(gap(format!("per-player {}", form(operation))));
        }
        let players = self.fetch(self.players(players)?);
        writeln!(out, "for bound_{} in {players} {{", binding.0).unwrap();
        self.bound_players.insert(binding.0);
        let compiled = self.block(body, out, false);
        self.bound_players.remove(&binding.0);
        compiled?;
        writeln!(out, "}}").unwrap();
        Ok(())
    }
    /// The count of a resolution choice. One that cannot be filled is filled as
    /// far as it can be, so "exactly" asks for as many as can be chosen.
    fn select_count(&self, count: &Count) -> Result<String> {
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
            Count::Between { .. } => return Err(gap("bounded resolution choice")),
            Count::PerPlayer { .. } => return Err(gap("per-player resolution choice")),
        })
    }
    fn choose(
        &mut self,
        player: &PlayerRef,
        objects: &ObjectSet,
        count: &Count,
        result: &ResultId,
        purpose: &str,
        out: &mut String,
    ) -> Result<()> {
        let count = self.select_count(count)?;
        let local = format!("selected_{}", self.fresh());
        writeln!(out, "let {local}_player = {}; let {local}_candidates = {}; let {local}_count = {}; let {local} = cantrip_mechanics::choose::select(g, {local}_player, asker.source, Purpose::{purpose}, {local}_candidates, cantrip_mechanics::select::Selection::of({local}_count, &[]))?;", self.fetch(self.certain_player(player)?), self.fetch(self.objects(objects)?), self.fetch(count)).unwrap();
        self.selections.insert(result.0, local);
        Ok(())
    }
    pub(super) fn fresh(&mut self) -> usize {
        self.next += 1;
        self.next - 1
    }
    /// `Vec<ObjectId>` of what can still be changed: an object that left is
    /// gone, and its last-known information is not a thing to act on (400.7).
    fn acted_on(&self, objects: &ObjectSet) -> Result<String> {
        Ok(self.fetch(format!(
            "({}).into_iter().filter(|&o| q.state().obj(o).became.is_none()).collect::<Vec<ObjectId>>()",
            self.objects(objects)?
        )))
    }
    fn operation(&mut self, op: &Operation, result: &ResultId, out: &mut String) -> Result<()> {
        let id = self.fresh();
        match op {
            Operation::AddMana {
                players,
                mana,
                restriction,
            } => {
                let plan = mana_plan(mana)?;
                self.emit_mana(&plan, players, restriction.as_ref(), None, out)?;
            }
            Operation::CreateTokens {
                players,
                tokens,
                entry,
            } => self.create_tokens(id, players, tokens, entry, result, out)?,
            Operation::Draw { players, amount }
            | Operation::GainLife { players, amount }
            | Operation::LoseLife { players, amount } => {
                let number = self.number(amount)?;
                let (method, max, ty) = match op {
                    Operation::Draw { .. } => ("draw", u32::MAX.into(), "u32"),
                    Operation::GainLife { .. } => ("gain_life", i32::MAX.into(), "i32"),
                    _ => ("lose_life", i32::MAX.into(), "i32"),
                };
                writeln!(out,"let amount_{id} = {}; let players_{id} = {}; for player in players_{id} {{ g.{method}(player, amount_{id}); }}",self.fetch(number.amount(max,ty)?),self.fetch(self.players(players)?)).unwrap();
            }
            Operation::Mill { players, amount } => {
                let number = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out,"let amount_{id} = {}; let mills_{id}: Vec<(PlayerId, u32)> = {}.into_iter().map(|p| (p, amount_{id})).collect(); let milled_{id} = g.mill_together(&mills_{id});",self.fetch(number),self.fetch(self.players(players)?)).unwrap();
                self.moved.insert(result.0, format!("milled_{id}"));
            }
            Operation::Discard {
                players,
                amount,
                random,
            } => {
                let number = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                if *random {
                    let PlayerSet::One(player) = players else {
                        return Err(gap("random discard by several players"));
                    };
                    writeln!(out, "let amount_{id} = {}; let player_{id} = {}; cantrip_mechanics::discard_at_random(g, player_{id}, amount_{id} as usize);", self.fetch(number), self.fetch(self.certain_player(player)?)).unwrap();
                } else {
                    writeln!(out,"let amount_{id} = {}; let players_{id} = {}; let mut cards_{id} = Vec::new(); for player in players_{id} {{ cards_{id}.extend(cantrip_mechanics::choose_from_hand(g, player, asker.source, amount_{id})?); }} let discarded_{id} = g.discard_together(&cards_{id});",self.fetch(number),self.fetch(self.players(players)?)).unwrap();
                    self.moved.insert(result.0, format!("discarded_{id}"));
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
                writeln!(out, "let amount_{id} = {}; let player_{id} = {}; cantrip_mechanics::{method}(g, player_{id}, asker.source, amount_{id})?;", self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
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
            }
            Operation::Move {
                objects,
                destination,
                entry,
                face_down: false,
            } => self.move_objects(id, objects, destination, entry, result, out)?,
            Operation::Tap { objects, tapped } => {
                writeln!(
                    out,
                    "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ g.{}(object); }} }});",
                    self.acted_on(objects)?,
                    if *tapped { "tap" } else { "untap" }
                )
                .unwrap();
            }
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } => self.sacrifice(id, players, objects, choose.as_ref(), result, out)?,
            Operation::DealDamage {
                source,
                recipients,
                amount,
                preventable: true,
            } => {
                let amount = self.number(amount)?.amount(u16::MAX.into(), "u16")?;
                writeln!(out, "let source_{id} = {}; let amount_{id} = {}; let recipients_{id} = {}; if let Some(source) = source_{id} {{ let batch: Vec<(ObjectId, Target, u16)> = recipients_{id}.iter().map(|t| (source, *t, amount_{id})).collect(); g.damage_together(&batch); }}", self.fetch(self.object(source)?), self.fetch(amount), self.fetch(self.recipients(recipients)?)).unwrap();
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
                writeln!(out, "g.damage_together(&batch_{id});").unwrap();
            }
            Operation::PutCounters { placements } => {
                writeln!(
                    out,
                    "let mut counters_{id}: Vec<(ObjectId, CounterKind, i16)> = Vec::new();"
                )
                .unwrap();
                for placement in placements {
                    if placement.selection.is_some() {
                        return Err(gap("counters placed by a resolution choice"));
                    }
                    if matches!(placement.recipients, Recipients::Players(_)) {
                        return Err(gap("counters on a player"));
                    }
                    let amount = self
                        .number(&placement.amount)?
                        .amount(i16::MAX.into(), "i16")?;
                    let kind = counter_kind(&placement.kind)?;
                    writeln!(out, "let amount = {}; for t in {} {{ if let Target::Object(object) = t {{ counters_{id}.push((object, {kind}, amount)); }} }}", self.fetch(amount), self.fetch(self.recipients(&placement.recipients)?)).unwrap();
                }
                writeln!(out, "g.add_counters_together(&counters_{id});").unwrap();
            }
            Operation::Attach { attachments, host } => {
                writeln!(out, "let attachments_{id} = {}; let host_{id} = {}; if let Some(host) = host_{id} {{ cantrip_api::instruction(g, |g| {{ for attachment in attachments_{id} {{ g.attach(attachment, host); }} }}); }}", self.acted_on(attachments)?, self.fetch(self.object(host)?)).unwrap();
            }
            Operation::CreateRegenerationShields { objects } => {
                writeln!(out, "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ g.add_effect(asker.source, object, EffectKind::RegenerationShield, Duration::EndOfTurn); }} }});", self.acted_on(objects)?).unwrap();
            }
            Operation::Reveal { objects } => {
                writeln!(out, "let objects_{id} = {}; let mut owners_{id}: Vec<PlayerId> = objects_{id}.iter().map(|o| g.state().obj(*o).owner).collect(); owners_{id}.dedup(); for owner in owners_{id} {{ let cards: Vec<ObjectId> = objects_{id}.iter().copied().filter(|o| g.state().obj(*o).owner == owner).collect(); g.reveal(owner, &cards); }}", self.fetch(self.objects(objects)?)).unwrap();
            }
            Operation::LookAt { player, objects } => {
                writeln!(
                    out,
                    "let player_{id} = {}; let objects_{id} = {}; g.look_at(player_{id}, &objects_{id});",
                    self.fetch(self.certain_player(player)?),
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.selections
                    .insert(result.0, format!("objects_{id}"));
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
                // "Live" subjects differ from read ones only where the set
                // could gain a member, which names of one object never do.
                let closed = matches!(
                    affected,
                    EffectSubjects::Objects(ObjectSet::One(_) | ObjectSet::Targets(_))
                );
                match evaluation {
                    EffectEvaluation::LiveObjects if !closed => {
                        return Err(gap("resolving rule effect on an open set of objects"))
                    }
                    EffectEvaluation::Live => return Err(gap("live continuous effect")),
                    _ => self.continuous(id, source, affected, changes, duration, out)?,
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
            Operation::PayCost {
                player,
                cost,
                optional: true,
            } => {
                let cost = self.cost(cost, "paid")?;
                self.helpers.insert("pay_question");
                writeln!(out, "let payer_{id} = {}; let paid_{id} = g.may_pay(payer_{id}, asker.source, asker.source, {cost}, &PAY_QUESTION)?;", self.fetch(self.certain_player(player)?)).unwrap();
                self.performed.insert(result.0, format!("paid_{id}"));
            }
            Operation::Primitive { name, .. } => {
                return Err(gap(format!("primitive operation {name}")))
            }
            _ => return Err(gap(format!("operation {}", form(op)))),
        }
        Ok(())
    }
    pub(super) fn reset_results(&mut self) {
        self.selections.clear();
        self.creations.clear();
        self.moved.clear();
        self.performed.clear();
        self.values.clear();
        self.flips.clear();
    }
    fn create_tokens(
        &mut self,
        id: usize,
        players: &PlayerSet,
        tokens: &[TokenSpec],
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        if !entry.states.is_empty() {
            return Err(gap("tokens entering in a special state"));
        }
        let first = &tokens[0].words;
        if tokens
            .iter()
            .any(|t| std::mem::discriminant(&t.words) != std::mem::discriminant(first))
        {
            return Err(gap("mixed token text origins in one creation batch"));
        }
        let words = match first {
            TokenWords::Rules => "Words::NONE",
            TokenWords::CreatingEffect => "g.words(asker.source)",
        };
        if let [TokenSpec {
            amount,
            recipe: TokenRecipe::Copy {
                object,
                adjustments,
            },
            ..
        }] = tokens
        {
            if !adjustments.is_empty() || !entry.counters.is_empty() || entry.tapped {
                return Err(gap("adjusted token copy"));
            }
            let PlayerSet::One(player) = players else {
                return Err(gap("token copies for several players"));
            };
            let amount = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
            writeln!(out, "let original_{id} = {}; let count_{id} = {}; let player_{id} = {}; let created_{id} = match original_{id} {{ Some(original) => g.create_token_copies(original, player_{id}, count_{id})?, None => Vec::new() }};", self.fetch(self.object(object)?), self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
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
        writeln!(out,"}} let words_{id} = {words}; let created_{id} = g.create_tokens_worded(&tokens_{id}, {}, None, words_{id})?;",entry.tapped).unwrap();
        self.creations.insert(result.0, format!("created_{id}"));
        Ok(())
    }
    fn move_objects(
        &mut self,
        id: usize,
        objects: &ObjectSet,
        destination: &Destination,
        entry: &EntrySpec,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let objects = self.fetch(self.objects(objects)?);
        let library = |place: String| format!("g.put_into_library(&objects_{id}, {place})");
        let call = match destination {
            Destination::Battlefield(_) | Destination::OwnersBattlefield => {
                if !entry.states.is_empty() {
                    return Err(gap("move arrival state"));
                }
                let controller = match destination {
                    Destination::Battlefield(player) => {
                        let player = self.certain_player(player)?;
                        writeln!(out, "let player_{id} = {};", self.fetch(player)).unwrap();
                        format!("player_{id}")
                    }
                    _ => "g.state().obj(o).owner".into(),
                };
                let rows = format!("let rows_{id}: Vec<(ObjectId, PlayerId, bool)> = objects_{id}.iter().map(|&o| (o, {controller}, {})).collect();", entry.tapped);
                if entry.counters.is_empty() {
                    format!("{{ {rows} g.put_all_onto_battlefield_as(&rows_{id})? }}")
                } else {
                    let mut counters = Vec::new();
                    for (kind, n) in &entry.counters {
                        let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
                        counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
                    }
                    format!("{{ {rows} let counters = [{}]; g.put_all_onto_battlefield_with_counters(&rows_{id}, &counters)? }}", counters.join(", "))
                }
            }
            _ if !plain_entry(entry) => return Err(gap("move arrival flags")),
            Destination::OwnersHand => format!("g.return_to_hand(&objects_{id})"),
            Destination::Exile => format!("g.exile_all(&objects_{id})"),
            // `put_into_graveyard` refuses permanents, so name each destination.
            Destination::OwnersGraveyard => format!("{{ let moves: Vec<(ObjectId, Zone)> = objects_{id}.iter().map(|&o| (o, Zone::graveyard(g.state().obj(o).owner))).collect(); g.move_together(&moves, MoveReason::Other) }}"),
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
            other => return Err(gap(format!("move to {}", form(other)))),
        };
        writeln!(out, "let objects_{id} = {objects}; let moved_{id} = {call};").unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
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
                let count = self.fetch(self.select_count(count)?);
                writeln!(out, "let count_{id} = {count}; for player in players_{id} {{ let eligible: Vec<ObjectId> = objects_{id}.iter().copied().filter(|&o| g.controller(o) == player && g.can_sacrifice(player, o)).collect(); for object in cantrip_mechanics::choose::select(g, player, asker.source, Purpose::Sacrifice, eligible, cantrip_mechanics::select::Selection::of(count_{id}.clone(), &[]))? {{ picks_{id}.push((player, object)); }} }}").unwrap();
            }
        }
        writeln!(out, "let sacrificed_{id} = g.sacrifice_each(&picks_{id});").unwrap();
        self.moved.insert(result.0, format!("sacrificed_{id}"));
        Ok(())
    }
    fn duration(&self, duration: &crate::ir::Duration) -> Result<String> {
        use crate::ir::Duration as D;
        Ok(match duration {
            D::EndOfTurn => "Duration::EndOfTurn".into(),
            D::EndOfCombat => "Duration::EndOfCombat".into(),
            D::Indefinite => "Duration::Indefinite".into(),
            D::WhileOnBattlefield(ObjectRef::Source) => "Duration::WhileSourceOnBattlefield".into(),
            D::UntilNextTurn(player) => format!(
                "Duration::UntilNextTurnOf({})",
                self.fetch(self.certain_player(player)?)
            ),
            D::UntilEndOfNextTurn(player) => format!(
                "Duration::EndOfNextTurnOf({})",
                self.fetch(self.certain_player(player)?)
            ),
            other => return Err(gap(format!("duration {}", form(other)))),
        })
    }
    /// One instruction of the card's text: the IR keeps what one instruction
    /// does in one payload, and `instruction` says so to the engine.
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
                "let objects_{id} = {}; cantrip_api::instruction(g, |g| {{ for object in objects_{id} {{ g.exert(object); }} }});",
                self.acted_on(affected)?
            )
            .unwrap();
            return Ok(());
        }
        let duration = self.duration(duration)?;
        writeln!(
            out,
            "let objects_{id} = {}; let source_{id} = {}; let duration_{id} = {duration};",
            self.acted_on(affected)?,
            self.fetch(self.object(source)?),
        )
        .unwrap();
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
    fn effect_kinds(&mut self, change: &Change) -> Result<Vec<String>> {
        if let Change::GrantAbility(ability) = change {
            let index = self.grant(ability)?;
            return Ok(vec![format!(
                "EffectKind::AddAbility(AbilityRef::delayed({}, {index}))",
                read::SOURCE_CARD
            )]);
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
                i16_of(power)?,
                i16_of(toughness)?
            )],
            Change::SetBasePowerToughness { power, toughness } => vec![format!(
                "EffectKind::SetPt({}, {})",
                i16_of(power)?,
                i16_of(toughness)?
            )],
            Change::GrantKeyword(k) => {
                vec![format!("EffectKind::AddKeywords(Keywords::of(&[{}]))", keyword(k)?)]
            }
            Change::RemoveAllAbilities => vec!["EffectKind::LoseAllAbilities".into()],
            Change::AddCardType(t) => vec![format!(
                "EffectKind::AddTypes(TypeSet::of(&[CardType::{}]))",
                card_type(t)?
            )],
            Change::SetCardTypes(types) => {
                let types = types
                    .iter()
                    .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                    .collect::<Result<Vec<_>>>()?;
                vec![format!("EffectKind::SetTypes(TypeSet::of(&[{}]))", types.join(", "))]
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
                vec![format!("EffectKind::SetColors(ColorSet({mask}))")]
            }
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
                        format!("EffectKind::SetCreatureType(Subtype::{word})")
                    } else {
                        format!("EffectKind::AddSubtype(Subtype::{word})")
                    });
                }
                kinds
            }
            Change::CreatureTypes {
                mode: ChangeMode::Add,
                types: CreatureTypeSet::All,
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
            Change::Rule(rule) => {
                return Err(gap(format!("continuous rule {}", rule_name(rule))))
            }
            other => return Err(gap(format!("continuous change {}", form(other)))),
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
