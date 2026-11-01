//! One-shot effects past the plain operations: what repeats, what a resolving
//! player picks between, and what each of several players does.
use super::*;

/// One `Instruction::ForEachPlayer`, with what comes after it in its program.
pub(super) struct EachPlayer<'a> {
    pub players: &'a PlayerSet,
    pub binding: &'a Binding,
    pub starting_with: Option<&'a PlayerRef>,
    pub in_chosen_direction: bool,
    pub body: &'a Program,
    pub rest: &'a [Instruction],
}

/// Whether a part of a per-player body reads the player it is for.
fn names(binding: &Binding, value: &impl std::fmt::Debug) -> bool {
    format!("{value:?}").contains(&format!("Bound(Binding({}))", binding.0))
}
fn is_bound(player: &PlayerRef, binding: &Binding) -> bool {
    matches!(player, PlayerRef::Bound(b) if b.0 == binding.0)
}
fn one_bound(players: &PlayerSet, binding: &Binding) -> bool {
    matches!(players, PlayerSet::One(player) if is_bound(player, binding))
}
/// Every result the instructions bind, however deep.
fn bound_results(instruction: &Instruction, found: &mut BTreeSet<u32>) {
    let mut text = format!("{instruction:?}");
    while let Some(at) = text.find("result: ResultId(") {
        let rest = &text[at + "result: ResultId(".len()..];
        let end = rest.find(')').unwrap_or(0);
        if let Ok(id) = rest[..end].parse() {
            found.insert(id);
        }
        text = rest.to_string();
    }
}
/// A walk over the players gives each one's step its own batch of events. These
/// are the steps where that is what the rules do, or where nothing tells it from
/// one batch: a choice or a payment, or what happens to each player on its own.
fn walked(op: &Operation) -> bool {
    matches!(
        op,
        Operation::ChooseObjects { .. }
            | Operation::ChooseValue { .. }
            | Operation::ChooseValueWith { .. }
            | Operation::ChooseAtRandom { .. }
            | Operation::ChoosePowerToughness { .. }
            | Operation::ChooseAction { .. }
            | Operation::BindValue { .. }
            | Operation::CreateContinuousEffect { .. }
            | Operation::CreateEmblem { .. }
            | Operation::CreateDelayedTrigger { .. }
            | Operation::CreateTrigger { .. }
            | Operation::Conjure { .. }
            | Operation::Draw { .. }
            | Operation::DrawUpTo { .. }
            | Operation::FlipCoin { .. }
            | Operation::GainLife { .. }
            | Operation::Keyword { .. }
            | Operation::LookAt { .. }
            | Operation::LoseLife { .. }
            | Operation::Note { .. }
            | Operation::PayAnyAmount { .. }
            | Operation::PayCost { .. }
            | Operation::PayRepeatedly { .. }
            | Operation::Reveal { .. }
            | Operation::RevealFromHand { .. }
            | Operation::RevealUntil { .. }
            | Operation::RevealUntilGroup { .. }
            | Operation::ReorderLibrary { .. }
            | Operation::Scry { .. }
            | Operation::Search { .. }
            | Operation::SetLifeTotal { .. }
            | Operation::Shuffle { .. }
            | Operation::Surveil { .. }
            | Operation::Vote { .. }
    )
}
/// Whether an instruction held whole inside one player's turn of a walk does
/// only what `walked` allows.
fn walks_whole(instruction: &Instruction) -> bool {
    let all = |program: &Program| program.instructions.iter().all(walks_whole);
    match instruction {
        Instruction::Effect { operation, .. } => walked(operation),
        Instruction::Group(body)
        | Instruction::Repeatable(body)
        | Instruction::Times { body, .. }
        | Instruction::Optional { body, .. }
        | Instruction::ForEachPlayer { body, .. }
        | Instruction::ForEachObject { body, .. } => all(body),
        Instruction::If {
            then, otherwise, ..
        } => all(then) && all(otherwise),
        Instruction::ChooseOne { options, .. } => options.iter().all(all),
    }
}
/// What opens one player's part of a step: "may", or "if".
#[derive(Clone)]
enum Guard {
    May,
    If(Condition),
}
/// One step of a per-player body. Every player finishes a step before the next
/// begins: "each player does this, then that" is all of this, then all of that.
#[derive(Clone)]
struct Step {
    at: usize,
    /// Which guard it sits under.
    guard: Option<usize>,
}
/// How the players' parts of one step come together.
enum Part {
    /// Once, for all of them, reading the picks of an earlier step.
    Once(Instruction),
    /// Each player's turn at it, in order, and that is the instruction.
    Walk,
    /// Each player's turn adds to a list that one batch acts on after.
    Gather(Gathered),
    /// `Walk`, though the rules would have it one batch: `feature` says so.
    Unbatched(String),
}
impl Gathered {
    /// What one thing gathered is.
    fn row(&self) -> &'static str {
        match self {
            Gathered::Objects { .. } | Gathered::Cards { .. } => "ObjectId",
            Gathered::Sacrifice { .. } => "(PlayerId, ObjectId)",
            Gathered::Mill { .. } => "(PlayerId, u32)",
            Gathered::Damage { .. } => "(ObjectId, Target, u16)",
            Gathered::Tokens { .. } => "(CardId, PlayerId)",
        }
    }
}
enum Gathered {
    Objects {
        objects: ObjectSet,
        then: Box<dyn Fn(ObjectSet) -> Operation>,
    },
    Cards {
        amount: Expr,
        result: ResultId,
    },
    Sacrifice {
        objects: ObjectSet,
        choose: Option<Count>,
        result: ResultId,
    },
    Mill {
        amount: Expr,
        result: ResultId,
    },
    Damage {
        source: ObjectRef,
        recipients: Recipients,
        amount: Expr,
    },
    Tokens {
        player: PlayerRef,
        token: TokenSpec,
        definition: TokenDefinition,
        tapped: bool,
        result: ResultId,
    },
}
/// A list a walk filled, as each player's own and as all of them.
struct View {
    kind: ViewKind,
    result: u32,
    each: String,
    all: Option<String>,
}
#[derive(Clone, Copy)]
enum ViewKind {
    Selected,
    Moved,
    Created,
    Before,
    Matches,
    Performed,
    Value,
    Flip,
}

impl Compiler {
    /// The zone of objects a player's own, so entering "under their control" is
    /// under their owner's.
    fn own_cards(&self, objects: &ObjectSet, binding: &Binding, earlier: &[Instruction]) -> bool {
        match objects {
            ObjectSet::Zone { scope, .. } => matches!(
                scope,
                ZoneScope::Players { players, zone }
                    if one_bound(players, binding)
                        && matches!(zone, EventZone::Hand | EventZone::Library | EventZone::Graveyard)
            ),
            ObjectSet::Matching { objects, .. } | ObjectSet::Take { objects, .. } => {
                self.own_cards(objects, binding, earlier)
            }
            ObjectSet::Selected(result) | ObjectSet::Observed(result) => {
                earlier.iter().any(|instruction| match instruction {
                    Instruction::Effect {
                        result: made,
                        operation,
                    } if made.0 == result.0 => match operation {
                        Operation::Search {
                            library,
                            also,
                            outside: false,
                            ..
                        } => is_bound(library, binding) && also.is_empty(),
                        Operation::ChooseObjects { objects, .. } => {
                            self.own_cards(objects, binding, earlier)
                        }
                        Operation::RevealUntil { library, .. } => is_bound(library, binding),
                        _ => false,
                    },
                    _ => false,
                })
            }
            _ => false,
        }
    }
    /// Where a step falls in the phases of a per-player body.
    fn part(&self, each: &EachPlayer, flat: &[Instruction], step: usize, earlier: &BTreeSet<u32>) -> Part {
        let instruction = &flat[step];
        let binding = each.binding;
        let bound = names(binding, instruction);
        let text = format!("{instruction:?}");
        let reads_earlier = earlier
            .iter()
            .any(|r| text.contains(&format!("ResultId({r})")));
        let Instruction::Effect { operation, result } = instruction else {
            if !bound && reads_earlier {
                return Part::Once(instruction.clone());
            }
            return match walks_whole(instruction) {
                true => Part::Walk,
                false => Part::Unbatched("per-player instruction with several steps".into()),
            };
        };
        let result = result.clone();
        match operation {
            Operation::Move {
                objects,
                destination,
                entry,
                face_down,
            } => {
                let mut destination = destination.clone();
                if let Destination::Battlefield(player) = &destination {
                    if is_bound(player, binding) {
                        if !self.own_cards(objects, binding, &flat[..step]) {
                            return Part::Unbatched(
                                "per-player move under each player's control".into(),
                            );
                        }
                        destination = Destination::OwnersBattlefield;
                    }
                }
                if names(binding, &destination) || names(binding, entry) {
                    return Part::Unbatched("per-player Move".into());
                }
                let (entry, face_down, moved) = (entry.clone(), *face_down, result.clone());
                let then = move |objects: ObjectSet| Operation::Move {
                    objects,
                    destination: destination.clone(),
                    entry: entry.clone(),
                    face_down,
                };
                if names(binding, objects) {
                    return Part::Gather(Gathered::Objects {
                        objects: objects.clone(),
                        then: Box::new(then),
                    });
                }
                Part::Once(Instruction::Effect {
                    result: moved,
                    operation: then(objects.clone()),
                })
            }
            Operation::DiscardObjects { objects } if names(binding, objects) => {
                Part::Gather(Gathered::Objects {
                    objects: objects.clone(),
                    then: Box::new(|objects| Operation::DiscardObjects { objects }),
                })
            }
            Operation::Discard {
                players,
                amount,
                random: false,
            } if one_bound(players, binding) => Part::Gather(Gathered::Cards {
                amount: amount.clone(),
                result,
            }),
            Operation::Sacrifice {
                players,
                objects,
                choose,
            } if one_bound(players, binding) => Part::Gather(Gathered::Sacrifice {
                objects: objects.clone(),
                choose: choose.clone(),
                result,
            }),
            Operation::Mill { players, amount } if one_bound(players, binding) => {
                Part::Gather(Gathered::Mill {
                    amount: amount.clone(),
                    result,
                })
            }
            Operation::DealDamage {
                source,
                recipients,
                amount,
                preventable: true,
            } => Part::Gather(Gathered::Damage {
                source: source.clone(),
                recipients: recipients.clone(),
                amount: amount.clone(),
            }),
            Operation::CreateTokens {
                players: PlayerSet::One(player),
                tokens,
                entry,
            } if entry.counters.is_empty() && entry.states.is_empty() => match tokens.as_slice() {
                [TokenSpec {
                    recipe: TokenRecipe::Defined(definition),
                    ..
                }] => Part::Gather(Gathered::Tokens {
                    player: player.clone(),
                    token: tokens[0].clone(),
                    definition: definition.clone(),
                    tapped: entry.tapped,
                    result,
                }),
                _ => Part::Unbatched(format!("per-player {}", form(operation))),
            },
            _ if !bound && reads_earlier => Part::Once(instruction.clone()),
            _ if walked(operation) => Part::Walk,
            _ => Part::Unbatched(format!("per-player {}", form(operation))),
        }
    }
    /// "Each player does this" (101.4): each one's choices come first and the
    /// actions happen together, step by step. A step goes round the players, and
    /// a step of what they picked happens once, for all of it.
    pub(super) fn each_player(&mut self, each: EachPlayer, out: &mut String) -> Result<()> {
        let id = self.fresh();
        let read = self.players(each.players).map(|p| self.fetch(p));
        let players = planned::planned_read(read, "Vec::<PlayerId>::new()", each.players);
        writeln!(out, "let mut players_{id}: Vec<PlayerId> = {players};").unwrap();
        if let Some(first) = each.starting_with {
            let first = self.fetch(self.player(first)?);
            writeln!(out, "let first_{id} = {first}; if let Some(at) = players_{id}.iter().position(|&p| Some(p) == first_{id}) {{ players_{id}.rotate_left(at); }}").unwrap();
        }
        if each.in_chosen_direction {
            self.plan_structure("per-player in a chosen direction", "ForEachPlayer", out);
        }
        // Flat: a Group is one step's words, and what a "may" or an "if" holds
        // are steps under it.
        let mut flat: Vec<Instruction> = Vec::new();
        let mut steps: Vec<Step> = Vec::new();
        let mut guards: Vec<Guard> = Vec::new();
        self.flatten(&each, &each.body.instructions, None, &mut flat, &mut steps, &mut guards);
        let count = flat.len();
        flat.extend_from_slice(each.rest);
        let mut program = Program { instructions: flat };
        for (i, _) in guards.iter().enumerate() {
            writeln!(out, "let mut guard_{id}_{i} = vec![false; players_{id}.len()];").unwrap();
        }
        self.bound_players.insert(each.binding.0);
        let mut views: Vec<View> = Vec::new();
        let mut earlier = BTreeSet::new();
        let mut asked = vec![false; guards.len()];
        for step in steps {
            let at = step.at;
            let part = self.part(&each, &program.instructions, at, &earlier);
            bound_results(&program.instructions[at], &mut earlier);
            // What is left of a list after a guard is only that guard's players' if
            // the list was made under that guard, and a "may".
            let part = match (part, step.guard) {
                (Part::Once(_), Some(guard))
                    if !asked[guard] || matches!(guards[guard], Guard::If(_)) =>
                {
                    Part::Unbatched("per-player instruction with several steps".into())
                }
                (part, _) => part,
            };
            match part {
                Part::Once(instruction) => {
                    self.view_all(&views);
                    program.instructions[at] = instruction;
                    self.step(&program, at, out);
                }
                Part::Walk => {
                    self.walk(id, &each, &program, &step, &guards, &mut asked, &mut views, None, out);
                }
                Part::Unbatched(feature) => {
                    self.plan_structure(&feature, "ForEachPlayer", out);
                    self.walk(id, &each, &program, &step, &guards, &mut asked, &mut views, None, out);
                }
                Part::Gather(gathered) => {
                    let gather = self.fresh();
                    writeln!(out, "let mut gathered_{gather}: Vec<{}> = Vec::new();", gathered.row()).unwrap();
                    self.walk(id, &each, &program, &step, &guards, &mut asked, &mut views, Some((gather, &gathered)), out);
                    self.view_all(&views);
                    self.after_gathering(gather, gathered, &mut program, at, out)?;
                }
            }
            if at + 1 < count && self.pending == Pending::Nothing {
                writeln!(out, "g.end_sentence();").unwrap();
            }
        }
        self.bound_players.remove(&each.binding.0);
        self.view_all(&views);
        Ok(())
    }
    fn flatten(
        &self,
        each: &EachPlayer,
        instructions: &[Instruction],
        guard: Option<usize>,
        flat: &mut Vec<Instruction>,
        steps: &mut Vec<Step>,
        guards: &mut Vec<Guard>,
    ) {
        for instruction in instructions {
            match instruction {
                Instruction::Group(group) => {
                    self.flatten(each, &group.instructions, guard, flat, steps, guards);
                    continue;
                }
                Instruction::Optional { decider, body }
                    if guard.is_none()
                        && is_bound(decider, each.binding)
                        && !effects::asks_by_paying(decider, body)
                        && !playing::may_only_play(decider, body) =>
                {
                    guards.push(Guard::May);
                    let inner = guards.len() - 1;
                    self.flatten(each, &body.instructions, Some(inner), flat, steps, guards);
                    continue;
                }
                Instruction::If {
                    condition,
                    then,
                    otherwise,
                } if guard.is_none() && otherwise.instructions.is_empty() => {
                    guards.push(Guard::If(condition.clone()));
                    let inner = guards.len() - 1;
                    self.flatten(each, &then.instructions, Some(inner), flat, steps, guards);
                    continue;
                }
                _ => {}
            }
            flat.push(instruction.clone());
            steps.push(Step {
                at: flat.len() - 1,
                guard,
            });
        }
    }
    /// Reads in a walk see the player's own list, and what's after see them all.
    fn view_each(&mut self, views: &[View]) {
        for view in views {
            self.view_set(view, Some(view.each.clone()));
        }
    }
    fn view_all(&mut self, views: &[View]) {
        for view in views {
            self.view_set(view, view.all.clone());
        }
    }
    fn view_set(&mut self, view: &View, local: Option<String>) {
        let map = match view.kind {
            ViewKind::Selected => &mut self.selections,
            ViewKind::Moved => &mut self.moved,
            ViewKind::Created => &mut self.creations,
            ViewKind::Before => &mut self.before,
            ViewKind::Matches => &mut self.reveal_matches,
            ViewKind::Performed => &mut self.performed,
            ViewKind::Value => &mut self.values,
            ViewKind::Flip => &mut self.flips,
        };
        match local {
            Some(local) => {
                map.insert(view.result, local);
            }
            None => {
                map.remove(&view.result);
            }
        }
    }
    /// What opens a guard, for each player's turn at it.
    fn guard_open(
        &mut self,
        id: usize,
        each: &EachPlayer,
        guards: &[Guard],
        guard: usize,
    ) -> String {
        let player = format!("bound_{}", each.binding.0);
        let test = match &guards[guard] {
            Guard::May => format!("cantrip_mechanics::effects::ask_may(g, {player}, asker.source, Purpose::Optional)?"),
            Guard::If(condition) => {
                let test = self.condition(condition).map(|c| self.fetch(c));
                planned::planned_read(test, "false", condition)
            }
        };
        format!("if {test} {{ guard_{id}_{guard}[index_{id}] = true;")
    }
    /// One round of the players at a step.
    #[allow(clippy::too_many_arguments)]
    fn walk(
        &mut self,
        id: usize,
        each: &EachPlayer,
        program: &Program,
        step: &Step,
        guards: &[Guard],
        asked: &mut [bool],
        views: &mut Vec<View>,
        gather: Option<(usize, &Gathered)>,
        out: &mut String,
    ) {
        self.view_each(views);
        let before = self.results();
        let mut body = String::new();
        let mut closing = String::new();
        if let Some(guard) = step.guard {
            if std::mem::replace(&mut asked[guard], true) {
                writeln!(body, "if guard_{id}_{guard}[index_{id}] {{").unwrap();
            } else {
                let open = self.guard_open(id, each, guards, guard);
                writeln!(body, "{open}").unwrap();
            }
            closing.push_str("}\n");
        }
        match gather {
            None => self.step(program, step.at, &mut body),
            Some((local, gathered)) => {
                let mark = body.len();
                let scope = self.results();
                let path = format!("instructions[{}]", step.at);
                let failure = self.attempt(path, |c| c.gather(local, gathered, each, &mut body));
                if let Some(feature) = failure {
                    body.truncate(mark);
                    self.restore_results(scope);
                    let failed = &program.instructions[step.at];
                    if let Instruction::Effect { result, .. } = failed {
                        self.failed.insert(result.0);
                    }
                    self.plan_instruction(&feature, failed, &mut body);
                }
            }
        }
        let mut tail = String::new();
        let mut declared = String::new();
        let scoped: [(ViewKind, &BTreeMap<u32, String>, &BTreeMap<u32, String>); 8] = [
            (ViewKind::Selected, &self.selections, &before.selections),
            (ViewKind::Moved, &self.moved, &before.moved),
            (ViewKind::Created, &self.creations, &before.creations),
            (ViewKind::Before, &self.before, &before.before),
            (ViewKind::Matches, &self.reveal_matches, &before.reveal_matches),
            (ViewKind::Performed, &self.performed, &before.performed),
            (ViewKind::Value, &self.values, &before.values),
            (ViewKind::Flip, &self.flips, &before.flips),
        ];
        let mut fresh = Vec::new();
        for (kind, now, then) in scoped {
            for (result, local) in now {
                if then.get(result) != Some(local) {
                    fresh.push((kind, *result, local.clone()));
                }
            }
        }
        for (kind, result, local) in fresh {
            let k = self.fresh();
            let list = matches!(
                kind,
                ViewKind::Selected
                    | ViewKind::Moved
                    | ViewKind::Created
                    | ViewKind::Before
                    | ViewKind::Matches
            );
            let all = list.then(|| format!("all_{k}"));
            if list {
                let read = match kind {
                    ViewKind::Before => self.fetch(&local),
                    _ => format!("{local}.clone()"),
                };
                writeln!(declared, "let mut each_{k}: Vec<Vec<ObjectId>> = vec![Vec::new(); players_{id}.len()]; let mut all_{k}: Vec<ObjectId> = Vec::new();").unwrap();
                writeln!(tail, "each_{k}[index_{id}] = {read}; all_{k}.extend(each_{k}[index_{id}].iter().copied());").unwrap();
            } else {
                writeln!(declared, "let mut each_{k} = vec![Default::default(); players_{id}.len()];").unwrap();
                writeln!(tail, "each_{k}[index_{id}] = {local};").unwrap();
            }
            let view = View {
                kind,
                result,
                each: match kind {
                    ViewKind::Before => format!("each_{k}[index_{id}].clone()"),
                    _ => format!("each_{k}[index_{id}]"),
                },
                all: match kind {
                    ViewKind::Before => all.map(|a| format!("{a}.clone()")),
                    _ => all,
                },
            };
            views.retain(|v| v.result != result || !same_kind(v.kind, kind));
            views.push(view);
        }
        // A value chosen in a turn is that turn's, and a local of it.
        self.chosen = before.chosen;
        out.push_str(&declared);
        let binding = each.binding.0;
        let quiet = matches!(
            &program.instructions[step.at],
            Instruction::Effect {
                operation: Operation::CreateContinuousEffect { .. },
                ..
            }
        );
        let (open, close) = match quiet {
            true => ("g.one_instruction(&mut |g| {", "Ok(()) })?;"),
            false => ("", ""),
        };
        writeln!(out, "{open} for (index_{id}, bound_{binding}) in players_{id}.iter().copied().enumerate() {{ {body} {tail} {closing} }} {close}").unwrap();
    }
    /// A player's turn at a step that adds to a batch.
    fn gather(
        &mut self,
        local: usize,
        gathered: &Gathered,
        each: &EachPlayer,
        out: &mut String,
    ) -> Result<()> {
        let bound = format!("bound_{}", each.binding.0);
        let id = self.fresh();
        match gathered {
            Gathered::Objects { objects, .. } => {
                writeln!(out, "gathered_{local}.extend({});", self.fetch(self.objects(objects)?)).unwrap();
            }
            Gathered::Cards { amount, .. } => {
                let number = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "let amount_{id} = {}; gathered_{local}.extend(cantrip_mechanics::choose_from_hand(g, {bound}, asker.source, amount_{id})?);", self.fetch(number)).unwrap();
            }
            Gathered::Sacrifice {
                objects, choose, ..
            } => {
                let player = PlayerSet::One(PlayerRef::Bound(each.binding.clone()));
                self.sacrifice_picks(id, &player, objects, choose.as_ref(), out)?;
                writeln!(out, "gathered_{local}.extend(picks_{id}.iter().copied());").unwrap();
            }
            Gathered::Mill { amount, .. } => {
                let number = self.number(amount)?.amount(u32::MAX.into(), "u32")?;
                writeln!(out, "gathered_{local}.push(({bound}, {}));", self.fetch(number)).unwrap();
            }
            Gathered::Damage {
                source,
                recipients,
                amount,
            } => {
                let amount = self.number(amount)?.amount(u16::MAX.into(), "u16")?;
                writeln!(out, "let source_{id} = {}; let amount_{id} = {}; if let Some(source) = source_{id} {{ for t in {} {{ gathered_{local}.push((source, t, amount_{id})); }} }}", self.fetch(self.object(source)?), self.fetch(amount), self.fetch(self.recipients(recipients)?)).unwrap();
            }
            Gathered::Tokens {
                player,
                token,
                definition,
                ..
            } => {
                let amount = self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?;
                let definition = self.token_definition(definition)?;
                writeln!(out, "let def_{id} = g.card_id(&{definition}); let count_{id} = {}; let player_{id} = {}; gathered_{local}.extend(std::iter::repeat((def_{id}, player_{id})).take(count_{id} as usize));", self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
            }
        }
        Ok(())
    }
    /// `then` on the objects gathered, its result the step's own.
    fn act_on_gathered(
        &mut self,
        local: usize,
        then: &dyn Fn(ObjectSet) -> Operation,
        result: ResultId,
        program: &mut Program,
        at: usize,
        out: &mut String,
    ) {
        let held = u32::MAX - local as u32;
        self.selections.insert(held, format!("gathered_{local}"));
        program.instructions[at] = Instruction::Effect {
            result,
            operation: then(ObjectSet::Selected(ResultId(held))),
        };
        self.step(program, at, out);
        self.selections.remove(&held);
    }
    /// The one batch of what the players' turns gathered.
    fn after_gathering(
        &mut self,
        local: usize,
        gathered: Gathered,
        program: &mut Program,
        at: usize,
        out: &mut String,
    ) -> Result<()> {
        match gathered {
            Gathered::Objects { then, .. } => {
                let Instruction::Effect { result, .. } = program.instructions[at].clone() else {
                    return Err(gap("gathered step that is no operation"));
                };
                self.act_on_gathered(local, then.as_ref(), result, program, at, out);
            }
            Gathered::Cards { result, .. } => {
                let discard = |objects| Operation::DiscardObjects { objects };
                self.act_on_gathered(local, &discard, result, program, at, out);
            }
            Gathered::Sacrifice { result, .. } => {
                writeln!(out, "let sacrificed_{local} = g.sacrifice_each(&gathered_{local});").unwrap();
                self.moved.insert(result.0, format!("sacrificed_{local}"));
                let objects = format!("gathered_{local}.iter().map(|pick| pick.1).collect::<Vec<ObjectId>>()");
                self.left_behind(&result, effects::gone(&objects));
            }
            Gathered::Mill { result, .. } => {
                writeln!(out, "let milled_{local} = g.mill_together(&gathered_{local});").unwrap();
                self.moved.insert(result.0, format!("milled_{local}"));
                self.left_behind(&result, format!("milled_{local}.clone()"));
            }
            Gathered::Damage { .. } => {
                writeln!(out, "g.damage_together(&gathered_{local});").unwrap();
            }
            Gathered::Tokens {
                token,
                tapped,
                result,
                ..
            } => {
                let words = match token.words {
                    TokenWords::Rules => "Words::NONE",
                    TokenWords::CreatingEffect => "g.words(asker.source)",
                };
                writeln!(out, "let words_{local} = {words}; let created_{local} = g.create_tokens_worded(&gathered_{local}, {tapped}, None, words_{local})?;").unwrap();
                self.creations.insert(result.0, format!("created_{local}"));
            }
        }
        Ok(())
    }
    /// "Repeat this process": the body runs, and runs again if it reached a
    /// `RepeatProcess`. What it reports holds what every run did.
    pub(super) fn repeat(
        &mut self,
        amount: Option<&Expr>,
        body: &Program,
        out: &mut String,
    ) -> Result<()> {
        let id = self.fresh();
        match amount {
            Some(amount) => {
                let runs = self
                    .number(amount)
                    .and_then(|n| n.amount(u32::MAX.into(), "u32"))
                    .map(|n| self.fetch(n));
                let runs = planned::planned_read(runs, "1u32", amount);
                writeln!(out, "let mut runs_{id}: u32 = {runs}; while runs_{id} > 0 {{ runs_{id} -= 1; let mut again_{id} = false;").unwrap();
            }
            None => writeln!(out, "loop {{ let mut again_{id} = false;").unwrap(),
        }
        let enclosing = self.repeating.replace(id);
        let compiled = self.block(body, out, true);
        self.repeating = enclosing;
        compiled?;
        writeln!(out, "if !again_{id} {{ break; }} }}").unwrap();
        Ok(())
    }
    /// "Do this or do that", asked as it resolves. The options' words are the
    /// card's, which the IR no longer has: the question waits on them, and each
    /// option is real.
    pub(super) fn choose_one(
        &mut self,
        decider: &PlayerRef,
        options: &[Program],
        out: &mut String,
    ) -> Result<()> {
        let id = self.fresh();
        let player = self.certain_player(decider).map(|p| self.fetch(p));
        let decider = planned::planned_read(player, "asker.controller", decider);
        let labels: Vec<String> = (0..options.len())
            .map(|i| format!("Label::Card(card_{id}, {i})"))
            .collect();
        let pick = planned::planned_value(
            "instruction ChooseOne",
            "0u8",
            &format!(
                "cantrip_mechanics::choose::one_of(g, decider_{id}, asker.source, vec![{}])?",
                labels.join(", ")
            ),
        );
        writeln!(out, "let decider_{id} = {decider}; match {pick} {{").unwrap();
        for (i, option) in options.iter().enumerate() {
            match i + 1 == options.len() {
                true => writeln!(out, "_ => {{").unwrap(),
                false => writeln!(out, "{i} => {{").unwrap(),
            }
            self.block(option, out, true)?;
            writeln!(out, "}}").unwrap();
        }
        writeln!(out, "}}").unwrap();
        Ok(())
    }
    /// What `operation` has no arm for of its own.
    pub(super) fn one_shot(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        match op {
            Operation::RepeatProcess => {
                let process = self
                    .repeating
                    .ok_or_else(|| gap("repeat process outside its body"))?;
                writeln!(out, "again_{process} = true;").unwrap();
            }
            Operation::JoinResults { results } => {
                writeln!(out, "let mut joined_{id}: Vec<ObjectId> = Vec::new();").unwrap();
                for joined in results {
                    let list = if self.selections.contains_key(&joined.0) {
                        self.selected(joined)?
                    } else if self.creations.contains_key(&joined.0) {
                        self.created(joined)?
                    } else {
                        self.moved(joined)?
                    };
                    writeln!(out, "joined_{id}.extend({list});").unwrap();
                }
                self.selections.insert(result.0, format!("joined_{id}"));
            }
            Operation::CreateEmblem { actor, abilities } => {
                let card = Card {
                    schema_version: SCHEMA_VERSION,
                    name: format!("{} emblem", self.card_name),
                    characteristics: Characteristics {
                        supertypes: Vec::new(),
                        card_types: Vec::new(),
                        subtypes: Vec::new(),
                        mana_cost: None,
                        color_indicator: Vec::new(),
                        power_toughness: None,
                        loyalty: None,
                        defense: None,
                    },
                    abilities: abilities.clone(),
                };
                let name = TokenName::Explicit(card.name.clone());
                let child = emit_module(&card, Some(&name))?;
                self.gaps
                    .extend(child.gaps.iter().map(|g| g.clone().at("emblem")));
                writeln!(self.functions, "pub mod emblem_{id} {{\n{}\n}}", child.rust).unwrap();
                let path = format!("emblem_{id}::CARD");
                self.extras.push(path.clone());
                self.extras
                    .extend(child.extras.into_iter().map(|p| format!("emblem_{id}::{p}")));
                writeln!(out, "for player in {} {{ cantrip_mechanics::tokens::create_emblem(g, player, &{path}); }}", self.fetch(self.players(actor)?)).unwrap();
            }
            other => return Err(gap(format!("operation {}", form(other)))),
        }
        Ok(())
    }
}

fn same_kind(a: ViewKind, b: ViewKind) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}
