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
            | Operation::CopyOnStack { .. }
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
    /// A "may" that is its choice: choosing nothing is declining.
    Chooses,
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

/// Whether `e`, an item on the stack, is what target `t` names.
pub(super) const STACK_ITEM: &str = "(match t { Target::Object(id) => e.kind == StackKind::Spell && e.object == id, Target::Ability(serial) => e.kind != StackKind::Spell && e.serial == serial, _ => false })";

/// `filter` with the one object it picks out named as the source, if it does:
/// an event watching for "that creature" made by an ability whose matcher
/// reaches nothing else can have that creature as the watcher's source.
fn watched_filter(filter: &mut ObjectFilter) -> Option<ObjectRef> {
    match filter {
        ObjectFilter::SameObject(object) if !matches!(object, ObjectRef::Source) => {
            Some(std::mem::replace(object, ObjectRef::Source))
        }
        ObjectFilter::All(filters) => filters.iter_mut().find_map(watched_filter),
        // "A creature dealt damage by that creature".
        ObjectFilter::Condition(condition) => match &mut **condition {
            Condition::Compare { left, .. } => match left {
                Expr::EventHistory { event, .. } => match &mut **event {
                    EventSpec::Damage(damage) => watched_filter(&mut damage.sources),
                    _ => None,
                },
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

/// The object `event` is about, moved to the watcher's source.
pub(super) fn watched(event: &mut EventSpec) -> Option<ObjectRef> {
    match event {
        EventSpec::Damage(damage) => {
            watched_filter(&mut damage.sources).or_else(|| match &mut damage.recipients {
                DamageRecipients::Objects(objects) => watched_filter(objects),
                _ => None,
            })
        }
        EventSpec::Attack { attackers, .. } => watched_filter(attackers),
        EventSpec::ZoneChange { objects, .. } | EventSpec::Object { objects, .. } => {
            watched_filter(objects)
        }
        // One object, however it does it.
        EventSpec::Either(events) => {
            let mut parts = events.clone();
            let mut seen = None;
            for part in &mut parts {
                let object = watched(part)?;
                if seen.as_ref().is_some_and(|s| format!("{s:?}") != format!("{object:?}")) {
                    return None;
                }
                seen = Some(object);
            }
            *events = parts;
            seen
        }
        _ => None,
    }
}

/// The targets of a slot `event` is about each of, moved to the watcher's source:
/// what they each do is an event of their own, so each has its own watcher.
pub(super) fn watched_each(event: &mut EventSpec) -> Option<ObjectSet> {
    // "When a creature dealt damage this way dies this turn".
    fn dealt(filter: &mut ObjectFilter) -> Option<ObjectSet> {
        match filter {
            ObjectFilter::DamagedBy(result) => {
                let list = ObjectSet::Named(result.clone());
                *filter = ObjectFilter::SameObject(ObjectRef::Source);
                Some(list)
            }
            ObjectFilter::All(filters) => filters.iter_mut().find_map(dealt),
            _ => None,
        }
    }
    let damage = match event {
        EventSpec::Damage(damage) => damage,
        EventSpec::ZoneChange { objects, .. } | EventSpec::Object { objects, .. } => {
            return dealt(objects)
        }
        _ => return None,
    };
    let ObjectFilter::InSet(set) = &damage.sources else {
        return None;
    };
    if !matches!(**set, ObjectSet::Targets(_)) {
        return None;
    }
    let list = (**set).clone();
    damage.sources = ObjectFilter::SameObject(ObjectRef::Source);
    Some(list)
}

/// Who may choose new targets, and for what, if that is all `body` does.
pub(super) fn may_retarget(body: &Program) -> Option<(&PlayerRef, &ObjectRef)> {
    match body.instructions.as_slice() {
        [Instruction::Effect {
            operation:
                Operation::Retarget {
                    actor: PlayerSet::One(player),
                    object,
                    targets: NewTargets::Must | NewTargets::May,
                },
            ..
        }] => Some((player, object)),
        _ => None,
    }
}

/// Whether a target of this domain may be an ability on the stack.
pub(super) fn on_the_stack(domain: &ObjectDomain) -> bool {
    domain.zones == [EventZone::Stack]
        && matches!(domain.kind, ObjectKind::Ability | ObjectKind::Any)
}

impl Compiler {
    /// The zone of objects a player's own, so entering "under their control" is
    /// under their owner's.
    fn own_cards(&self, objects: &ObjectSet, binding: &Binding, earlier: &[Instruction]) -> bool {
        if self.moved_their_own(objects, binding) {
            return true;
        }
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
    /// "Cards they own", or "cards they exiled this way" of a step that moved
    /// what each player owns.
    fn moved_their_own(&self, objects: &ObjectSet, binding: &Binding) -> bool {
        let text = format!("{objects:?}");
        if text.contains(&format!("OwnedBy(One(Bound(Binding({}))))", binding.0)) {
            return true;
        }
        let Some(at) = text.find("MovedBy { result: ResultId(") else {
            return false;
        };
        let rest = &text[at + "MovedBy { result: ResultId(".len()..];
        let id = rest[..rest.find(')').unwrap_or(0)].parse().unwrap_or(u32::MAX);
        self.notes.own.contains(&id) && names(binding, &text)
    }
    /// Where a step falls in the phases of a per-player body.
    fn part(
        &self,
        each: &EachPlayer,
        flat: &[Instruction],
        step: usize,
        earlier: &BTreeSet<u32>,
    ) -> Part {
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
            // "Each player chooses five lands and sacrifices the rest": the
            // operation asks all of them, and they sacrifice together. It keeps
            // as many as it can, which "up to two" is not.
            Operation::SacrificeAllBut {
                players,
                objects,
                keep,
            } if one_bound(players, binding)
                && !names(binding, objects)
                && !names(binding, keep)
                && each.starting_with.is_none()
                && !each.in_chosen_direction
                && !self.line_text().is_some_and(|line| line.contains("chooses up to")) =>
            {
                Part::Once(Instruction::Effect {
                    result,
                    operation: Operation::SacrificeAllBut {
                        players: each.players.clone(),
                        objects: objects.clone(),
                        keep: keep.clone(),
                    },
                })
            }
            Operation::DiscardAllBut { players, keep }
                if one_bound(players, binding)
                    && !names(binding, keep)
                    && each.starting_with.is_none()
                    && !each.in_chosen_direction =>
            {
                Part::Once(Instruction::Effect {
                    result,
                    operation: Operation::DiscardAllBut {
                        players: each.players.clone(),
                        keep: keep.clone(),
                    },
                })
            }
            Operation::Tap { objects, tapped } if names(binding, objects) => {
                let tapped = *tapped;
                Part::Gather(Gathered::Objects {
                    objects: objects.clone(),
                    then: Box::new(move |objects| Operation::Tap { objects, tapped }),
                })
            }
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
        let walking = (self.notes.walking).replace((each.binding.0, format!("players_{id}")));
        self.note_own(&each.body);
        if let Some(first) = each.starting_with {
            let first = self.fetch(self.player(first)?);
            writeln!(out, "let first_{id} = {first}; if let Some(at) = players_{id}.iter().position(|&p| Some(p) == first_{id}) {{ players_{id}.rotate_left(at); }}").unwrap();
        }
        if each.in_chosen_direction {
            self.plan_code(
                "per-player in a chosen direction",
                &format!("players_{id} = cantrip_mechanics::players::in_chosen_direction(g, players_{id}, asker)?;"),
                out,
            );
        }
        // Flat: a Group is one step's words, and what a "may" or an "if" holds
        // are steps under it.
        let mut flat: Vec<Instruction> = Vec::new();
        let mut steps: Vec<Step> = Vec::new();
        let mut guards: Vec<Guard> = Vec::new();
        self.flatten(
            &each,
            &each.body.instructions,
            None,
            &mut flat,
            &mut steps,
            &mut guards,
        );
        let count = flat.len();
        flat.extend_from_slice(each.rest);
        let mut program = Program { instructions: flat };
        for (i, _) in guards.iter().enumerate() {
            writeln!(
                out,
                "let mut guard_{id}_{i} = vec![false; players_{id}.len()];"
            )
            .unwrap();
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
                    // Each who chose cards is each who moved them together.
                    if let Instruction::Effect {
                        result,
                        operation: Operation::Move { objects: ObjectSet::Selected(chosen), .. },
                    } = &instruction
                    {
                        if let Some(took) = self.notes.took.get(&chosen.0).cloned() {
                            self.notes.took.insert(result.0, took);
                        }
                    }
                    program.instructions[at] = instruction;
                    self.step(&program, at, out);
                }
                Part::Walk => {
                    self.walk(
                        id, &each, &program, &step, &guards, &mut asked, &mut views, None, out,
                    );
                }
                Part::Unbatched(feature) => {
                    self.plan_code(&feature, "g.begin_batch();", out);
                    self.walk(
                        id, &each, &program, &step, &guards, &mut asked, &mut views, None, out,
                    );
                    self.plan_code(&feature, "g.end_batch();", out);
                }
                Part::Gather(gathered) => {
                    let gather = self.fresh();
                    writeln!(
                        out,
                        "let mut gathered_{gather}: Vec<{}> = Vec::new();",
                        gathered.row()
                    )
                    .unwrap();
                    self.walk(
                        id,
                        &each,
                        &program,
                        &step,
                        &guards,
                        &mut asked,
                        &mut views,
                        Some((gather, &gathered)),
                        out,
                    );
                    self.view_all(&views);
                    if let Gathered::Sacrifice { result, .. } = &gathered {
                        let test = format!("gathered_{gather}.iter().any(|(who, _)| *who == players_{id}[at])");
                        self.notes.took.insert(result.0, (format!("players_{id}"), test));
                    }
                    self.after_gathering(gather, gathered, &mut program, at, out)?;
                }
            }
            if at + 1 < count && self.pending == Pending::Nothing {
                writeln!(out, "g.end_sentence();").unwrap();
            }
        }
        self.bound_players.remove(&each.binding.0);
        self.view_all(&views);
        self.note_walked(&each.body);
        self.notes.walking = walking;
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
                    guards.push(match lane_k::chooses_first(decider, body) {
                        true => Guard::Chooses,
                        false => Guard::May,
                    });
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
            Guard::Chooses => {
                self.may_choose = true;
                "true".to_string()
            }
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
        let guards_around = std::mem::take(&mut self.guard_opened);
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
            (
                ViewKind::Matches,
                &self.reveal_matches,
                &before.reveal_matches,
            ),
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
                writeln!(
                    declared,
                    "let mut each_{k} = vec![Default::default(); players_{id}.len()];"
                )
                .unwrap();
                writeln!(tail, "each_{k}[index_{id}] = {local};").unwrap();
            }
            // How much of something: all of them together is its sum. A reader
            // one player at a time may mean their own or the total; the AST
            // does not say which, so nothing is read for it.
            let total = (matches!(kind, ViewKind::Performed)
                && local.starts_with("done_")
                && !lane_n::read_player_by_player(each.rest, &result))
            .then(|| format!("done_total_{k}"));
            if let Some(total) = &total {
                writeln!(declared, "let mut {total}: i64 = 0;").unwrap();
                writeln!(tail, "{total} += {local};").unwrap();
            }
            if let (
                Some(total),
                Instruction::Effect {
                    operation:
                        Operation::PayAnyAmount {
                            resource: PaidResource::Life,
                            ..
                        },
                    ..
                },
            ) = (&total, &program.instructions[step.at])
            {
                self.notes.life_paid = Some(total.clone());
            }
            if matches!(kind, ViewKind::Performed) {
                let bound = format!("bound_{}", each.binding.0);
                let rest = each.rest;
                self.tally_share(result, &local, &bound, rest, &mut declared, &mut tail);
                if lane_n::read_player_by_player(rest, &result) {
                    self.notes.read_as_own.insert(result);
                }
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
                    _ => all.or(total),
                },
            };
            views.retain(|v| v.result != result || !same_kind(v.kind, kind));
            views.push(view);
            let took = match kind {
                ViewKind::Performed => Some(format!("(each_{k}[at] as i64) != 0")),
                ViewKind::Moved | ViewKind::Selected | ViewKind::Created => {
                    Some(format!("!each_{k}[at].is_empty()"))
                }
                _ => None,
            };
            if let Some(test) = took {
                let performed = matches!(kind, ViewKind::Performed);
                if performed || !self.notes.took.contains_key(&result) {
                    self.notes
                        .took
                        .insert(result, (format!("players_{id}"), test));
                }
            }
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
        // A "you may" that opened a guard in this turn closes it in this turn.
        let shut = "}".repeat(std::mem::replace(&mut self.guard_opened, guards_around));
        writeln!(out, "{open} for (index_{id}, bound_{binding}) in players_{id}.iter().copied().enumerate() {{ {body} {tail} {shut} {closing} }} {close}").unwrap();
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
                writeln!(
                    out,
                    "gathered_{local}.extend({});",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
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
                writeln!(
                    out,
                    "gathered_{local}.push(({bound}, {}));",
                    self.fetch(number)
                )
                .unwrap();
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
                writeln!(
                    out,
                    "let sacrificed_{local} = g.sacrifice_each(&gathered_{local});"
                )
                .unwrap();
                self.moved.insert(result.0, format!("sacrificed_{local}"));
                let objects = format!(
                    "gathered_{local}.iter().map(|pick| pick.1).collect::<Vec<ObjectId>>()"
                );
                self.left_behind(&result, effects::gone(&objects));
            }
            Gathered::Mill { result, .. } => {
                writeln!(
                    out,
                    "let milled_{local} = g.mill_together(&gathered_{local});"
                )
                .unwrap();
                self.moved.insert(result.0, format!("milled_{local}"));
                self.left_behind(&result, format!("milled_{local}.clone()"));
            }
            Gathered::Damage { .. } => match &program.instructions[at] {
                Instruction::Effect { result, .. } if self.read_later(result) => {
                    writeln!(out, "let done_{local}: i64 = g.damage_together(&gathered_{local}).iter().map(|&n| i64::from(n)).sum();").unwrap();
                    self.performed.insert(result.0, format!("done_{local}"));
                }
                _ => writeln!(out, "g.damage_together(&gathered_{local});").unwrap(),
            },
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
    /// card's (`option_words`): one whose list can't be read waits on them, and
    /// each option is real. An option that can't be taken is not offered, and
    /// with only one left nothing is asked (CR 608.2d).
    pub(super) fn choose_one(
        &mut self,
        decider: &PlayerRef,
        options: &[Program],
        out: &mut String,
    ) -> Result<()> {
        let id = self.fresh();
        let player = self.certain_player(decider).map(|p| self.fetch(p));
        let decider_code = planned::planned_read(player, "asker.controller", decider);
        writeln!(out, "let decider_{id} = {decider_code};").unwrap();
        let Some(base) = self.option_labels_of(options) else {
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
            writeln!(out, "match {pick} {{").unwrap();
            for (i, option) in options.iter().enumerate() {
                match i + 1 == options.len() {
                    true => writeln!(out, "_ => {{").unwrap(),
                    false => writeln!(out, "{i} => {{").unwrap(),
                }
                self.block(option, out, true)?;
                writeln!(out, "}}").unwrap();
            }
            writeln!(out, "}}").unwrap();
            return Ok(());
        };
        let card = match self.stack_item {
            true => "item.ability.card".to_string(),
            false => "g.card_id(&CARD)".to_string(),
        };
        let offered: Vec<String> = options
            .iter()
            .enumerate()
            .map(|(i, option)| format!("({}u8, {})", base + i, self.option_possible(option)))
            .collect();
        writeln!(
            out,
            "let offered_{id}: Vec<u8> = [{}].into_iter().filter(|option| option.1).map(|option| option.0).collect(); let card_{id} = {card}; let picked_{id} = if offered_{id}.len() > 1 {{ offered_{id}[cantrip_mechanics::choose::one_of(g, decider_{id}, asker.source, offered_{id}.iter().map(|&option| Label::Card(card_{id}, option)).collect())? as usize] }} else {{ offered_{id}.first().copied().unwrap_or(u8::MAX) }}; match picked_{id} {{",
            offered.join(", ")
        )
        .unwrap();
        for (i, option) in options.iter().enumerate() {
            writeln!(out, "{} => {{", base + i).unwrap();
            self.block(option, out, true)?;
            writeln!(out, "}}").unwrap();
        }
        writeln!(out, "_ => {{}}\n}}").unwrap();
        Ok(())
    }
    /// Where the options' words start in the card's `labels`, if the card's own
    /// words say what they are. An option that does nothing is "Neither".
    fn option_labels_of(&mut self, options: &[Program]) -> Option<usize> {
        if self.entering_labels.is_some() && self.choice_labels.is_empty() {
            return None;
        }
        let empty = |option: &Program| {
            let mut steps = Vec::new();
            lane_m::flat_groups(&option.instructions, &mut steps);
            steps.is_empty()
        };
        let printed = options.iter().filter(|option| !empty(option)).count();
        let declines = options.len() - printed;
        if declines > 1 || (declines == 1 && !empty(options.last()?)) {
            return None;
        }
        let mut labels = option_words::printed_options(&self.lines_said(), printed)?;
        // "From that player's graveyard or hand" is printed the other way round.
        if labels[0] == "Graveyard" && labels[1] == "Hand" {
            labels.swap(0, 1);
            if !format!("{:?}", options[0]).contains("Hand") {
                return None;
            }
        }
        if declines == 1 {
            labels.push(if printed == 2 { "Neither" } else { "None" }.to_string());
        }
        let base = (0..=self.choice_labels.len().saturating_sub(labels.len()))
            .find(|&at| self.choice_labels[at..].starts_with(&labels))
            .unwrap_or_else(|| {
                let at = self.choice_labels.len();
                self.choice_labels.extend(labels);
                at
            });
        self.option_labels = true;
        let quoted_labels: Vec<String> = self.choice_labels.iter().map(|l| quoted(l)).collect();
        self.entering_labels = Some(format!("&[{}]", quoted_labels.join(", ")));
        Some(base)
    }
    /// `bool`: whether an option can be taken at all, by the step it starts with.
    fn option_possible(&self, option: &Program) -> String {
        let mut steps = Vec::new();
        lane_m::flat_groups(&option.instructions, &mut steps);
        let Some(Instruction::Effect { operation, .. }) = steps.first() else {
            return "true".to_string();
        };
        let some = |objects: &ObjectSet| match self.objects(objects) {
            Ok(code) => format!("!{}.is_empty()", self.fetch(code)),
            Err(_) => "true".to_string(),
        };
        let hand = |players: &PlayerSet| match self.players(players) {
            Ok(code) => self.fetch(format!(
                "({code}).into_iter().any(|p: PlayerId| !q.state().zone(Zone::hand(p)).is_empty())"
            )),
            Err(_) => "true".to_string(),
        };
        match operation {
            Operation::ChooseObjects { objects, .. } | Operation::Move { objects, .. } => {
                some(objects)
            }
            Operation::Discard { players, .. } => hand(players),
            Operation::RevealFromHand { actor, .. } => hand(actor),
            Operation::Sacrifice {
                objects, players, ..
            } => match (self.objects(objects), self.players(players)) {
                (Ok(pool), Ok(players)) => self.fetch(format!(
                    "{{ let players: Vec<PlayerId> = {players}; ({pool}).into_iter().any(|o: ObjectId| players.contains(&q.obj(o).controller)) }}"
                )),
                _ => "true".to_string(),
            },
            Operation::RemoveCounters { removals } => match removals.as_slice() {
                [CounterRemoval {
                    recipients: Recipients::Objects(objects),
                    kind: Some(kind),
                    ..
                }] => match (self.objects(objects), counter_kind(kind)) {
                    (Ok(from), Ok(kind)) => self.fetch(format!(
                        "({from}).into_iter().any(|o: ObjectId| q.state().obj(o).counters({kind}) > 0)"
                    )),
                    _ => "true".to_string(),
                },
                _ => "true".to_string(),
            },
            _ => "true".to_string(),
        }
    }
    /// What `operation` has no arm for of its own.
    /// Whether targets of `slot` are items on the stack, abilities among them.
    pub(super) fn stack_slot(&self, slot: &Slot) -> bool {
        self.slots.get(&slot.0).is_some_and(
            |(_, spec)| matches!(&spec.kind, TargetKind::Object(domain) if on_the_stack(domain)),
        )
    }
    /// `Vec<Target>`: the legal targets `objects` names, if it names a slot of
    /// items on the stack.
    pub(super) fn stack_targets(&self, objects: &ObjectSet) -> Option<String> {
        match objects {
            ObjectSet::One(ObjectRef::Target { slot, index }) if self.stack_slot(slot) => {
                let i = self.slot(slot).ok()?;
                Some(format!("t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t)).into_iter().collect::<Vec<Target>>()"))
            }
            ObjectSet::Targets(slot) if self.stack_slot(slot) => {
                let i = self.slot(slot).ok()?;
                Some(format!("t_{i}.iter().copied().collect::<Vec<Target>>()"))
            }
            _ => None,
        }
    }
    /// "Target activated or triggered ability", with or without spells: the
    /// item on the stack that `t` names passes `filter`.
    pub(super) fn stack_target_test(&self, filter: &ObjectFilter) -> Result<String> {
        Ok(format!(
            "q.state().stack.iter().any(|e| {STACK_ITEM} && ({}))",
            self.stack_filter(filter)?
        ))
    }
    /// Whether stack item `e` passes `filter`. A word about a card reads a
    /// spell, and "from a [type] source" reads what an ability came from.
    fn stack_filter(&self, filter: &ObjectFilter) -> Result<String> {
        let join = |c: &Self, filters: &[ObjectFilter], with: &str| -> Result<String> {
            let tests = filters
                .iter()
                .map(|f| c.stack_filter(f))
                .collect::<Result<Vec<_>>>()?;
            Ok(format!("({})", tests.join(with)))
        };
        let source = Candidate::of("e.object");
        Ok(match filter {
            ObjectFilter::Any => "true".into(),
            ObjectFilter::All(filters) => join(self, filters, " && ")?,
            ObjectFilter::Either(filters) => join(self, filters, " || ")?,
            ObjectFilter::Not(inner) => format!("!({})", self.stack_filter(inner)?),
            ObjectFilter::OnStack(StackTrait::Spell) => "e.kind == StackKind::Spell".into(),
            ObjectFilter::OnStack(StackTrait::Activated) => "e.kind == StackKind::Activated".into(),
            ObjectFilter::OnStack(StackTrait::Triggered) => "e.kind == StackKind::Triggered".into(),
            ObjectFilter::OnStack(StackTrait::Loyalty) => {
                "q.activated_ability(e.ability).is_some_and(|d| d.is_loyalty_ability())".into()
            }
            ObjectFilter::OnStack(StackTrait::SingleTarget) => "e.target_count == 1".into(),
            ObjectFilter::ControlledBy(players) => self.player_in(players, "e.controller")?,
            ObjectFilter::AbilityOf(inner) => format!(
                "(e.kind != StackKind::Spell && {})",
                self.filter(inner, &source)?
            ),
            ObjectFilter::TargetsAny(recipients) => format!(
                "q.state().targets(e).iter().any(|t| {})",
                self.targeted(recipients)?
            ),
            other => format!(
                "(e.kind == StackKind::Spell && {})",
                self.filter(other, &source)?
            ),
        })
    }
    /// "Counter target spell. Its controller loses life": the item is gone by
    /// then, so who controlled it is read first.
    pub(super) fn keep_countered(
        &mut self,
        body: &Program,
        i: usize,
        instruction: &Instruction,
        out: &mut String,
    ) {
        let Instruction::Effect {
            operation: Operation::Counter { objects, .. },
            ..
        } = instruction
        else {
            return;
        };
        let slot = match objects {
            ObjectSet::One(ObjectRef::Target { slot, .. }) | ObjectSet::Targets(slot)
                if self.stack_slot(slot) =>
            {
                slot
            }
            _ => return,
        };
        let Ok(at) = self.slot(slot) else {
            return;
        };
        let read = format!("ControllerOf(Target {{ slot: {slot:?}");
        if !format!("{:?}", &body.instructions[i + 1..]).contains(&read) {
            return;
        }
        let local = format!("countered_{}", self.fresh());
        writeln!(out, "let {local}: Vec<Option<PlayerId>> = t_{at}_raw.iter().map(|&t: &Target| g.state().stack.iter().find(|e| {STACK_ITEM}).map(|e| e.controller)).collect();").unwrap();
        self.countered.insert(slot.0, local);
    }
    /// `Option<PlayerId>`: who controls the spell or ability a slot targets: an
    /// ability's own controller, which its source's need not be (113.8).
    pub(super) fn stack_controller(
        &self,
        slot: &Slot,
        index: impl std::fmt::Display,
    ) -> Result<String> {
        if let Some(kept) = self.countered.get(&slot.0) {
            return Ok(format!("{kept}.get({index}).copied().flatten()"));
        }
        let at = self.slot(slot)?;
        Ok(format!("t_{at}_raw.get({index}).copied().and_then(|t: Target| q.state().stack.iter().find(|e| {STACK_ITEM}).map(|e| e.controller))"))
    }
    /// "Counter target activated or triggered ability" (CR 701.6): with spells
    /// among them, one instruction counters them all.
    pub(super) fn counter_on_stack(&self, id: usize, objects: &ObjectSet, out: &mut String) {
        let targets = self.stack_targets(objects).expect("a slot of the stack");
        writeln!(out, "let items_{id}: Vec<(ObjectId, usize)> = {targets}.into_iter().filter_map(|t| {{ let stack = &g.state().stack; stack.iter().position(|e| {STACK_ITEM}).map(|at| (stack[at].object, at)) }}).collect(); g.counter_items(&items_{id});").unwrap();
    }
    /// "Change the target of", "choose new targets for" and "change a target
    /// of target spell or ability to this creature" (115.7a, 115.7d, 115.7b).
    pub(super) fn retarget(
        &mut self,
        id: usize,
        player: &PlayerRef,
        object: &ObjectRef,
        targets: &NewTargets,
        out: &mut String,
    ) -> Result<()> {
        let chooser = self.fetch(self.certain_player(player)?);
        let (target, ability) = match object {
            ObjectRef::Target { slot, index } => {
                let i = self.slot(slot)?;
                let target =
                    format!("t_{i}_raw.get({index}).copied().filter(|t| t_{i}.contains(t))");
                (target, self.stack_slot(slot))
            }
            other => (
                self.fetch(format!("{}.map(Target::Object)", self.object(other)?)),
                false,
            ),
        };
        writeln!(out, "let chooser_{id} = {chooser}; let retarget_{id}: Option<Target> = {target}; if let Some(t) = retarget_{id} {{").unwrap();
        match targets {
            NewTargets::May => {
                writeln!(out, "if let Target::Object(spell) = t {{ g.choose_new_targets(spell, chooser_{id})?; }}").unwrap();
                if ability {
                    writeln!(out, "else {{").unwrap();
                    self.plan_code(
                        "choose new targets for an ability",
                        &format!("g.choose_new_targets_of(t, chooser_{id})?"),
                        out,
                    );
                    writeln!(out, "}}").unwrap();
                }
            }
            NewTargets::Must => writeln!(out, "g.change_target(t, chooser_{id})?;").unwrap(),
            _ => writeln!(out, "g.change_a_target_to(t, asker.source, chooser_{id})?;").unwrap(),
        }
        writeln!(out, "}}").unwrap();
        Ok(())
    }
    /// Whether counters move in a way `move_counters` writes: a set number
    /// comes from one object, and "all" from each.
    fn moves_counters(
        &self,
        from: &ObjectSet,
        kind: Option<&crate::counter::CounterKind>,
        amount: &MovedCounters,
    ) -> bool {
        matches!(amount, MovedCounters::All)
            || matches!(from, ObjectSet::One(_))
            || (kind.is_some() && matches!(amount, MovedCounters::AnyNumber))
    }
    /// "Move a +1/+1 counter from this onto target creature" (CR 122.5): each
    /// kind by itself, which is how `move_counters` takes them.
    #[allow(clippy::too_many_arguments)]
    fn move_counters(
        &mut self,
        id: usize,
        result: &ResultId,
        player: &PlayerRef,
        from: &ObjectSet,
        to: &ObjectSet,
        kind: Option<&crate::counter::CounterKind>,
        amount: &MovedCounters,
        out: &mut String,
    ) -> Result<()> {
        let kinds = match kind {
            Some(kind) => format!("[{}]", read::counter_kind(kind)?),
            None => "CounterKind::ALL".to_string(),
        };
        let actor = self.fetch(self.certain_player(player)?);
        let sources = self.acted_on(from)?;
        let recipient = self.acted_on(to)?;
        let moving = format!("done_{id} += i64::from(g.move_counters(from, to, kind, amount));");
        let step = match (amount, kind) {
            (MovedCounters::One, Some(_)) => format!("let amount = 1i16; {moving}"),
            (MovedCounters::Exactly(n), Some(_)) => {
                let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
                writeln!(out, "let amount_{id}: i16 = {};", self.fetch(n)).unwrap();
                format!("let amount = amount_{id}; {moving}")
            }
            (MovedCounters::All, _) => format!("let amount = i16::MAX; {moving}"),
            (MovedCounters::AnyNumber, Some(_)) => format!("let most = g.state().obj(from).counters(kind).clamp(0, 255) as u32; if most > 0 {{ let amount = cantrip_mechanics::choose::number(g, actor_{id}, asker.source, 0, most, \"Move counters\")? as i16; {moving} }}"),
            (MovedCounters::OneOfEachKindMissing, _) => format!("if g.state().obj(to).counters(kind) == 0 {{ let amount = 1i16; {moving} }}"),
            // One question for each kind, as much of it as the mover likes.
            (MovedCounters::AnyNumber, None) => format!("let most = g.state().obj(from).counters(kind).clamp(0, 255) as u32; if most > 0 {{ let amount = cantrip_mechanics::choose::number(g, actor_{id}, asker.source, 0, most, \"Move counters\")? as i16; {moving} }}"),
            // The kind is chosen as this resolves (608.2d), among those it has.
            (MovedCounters::One, None) => {
                writeln!(out, "let mut done_{id}: i64 = 0; let actor_{id} = {actor}; let sources_{id} = {sources}; let to_{id}: Option<ObjectId> = ({recipient}).first().copied(); if let (Some(&from), Some(to)) = (sources_{id}.first(), to_{id}) {{ if let Some(kind) = g.choose_counter_kind(actor_{id}, asker.source, from)? {{ let amount = 1i16; {moving} }} }}").unwrap();
                if self.read_later(result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                }
                return Ok(());
            }
            _ => {
                let least = match amount {
                    MovedCounters::One => "1".to_string(),
                    MovedCounters::AtLeast(n) => self.fetch(self.number(n)?.amount(i16::MAX.into(), "i16")?),
                    _ => "0".to_string(),
                };
                let most = if matches!(amount, MovedCounters::One) { "1" } else { "i16::MAX" };
                writeln!(out, "let mut done_{id}: i64 = 0; let actor_{id} = {actor}; let sources_{id} = {sources}; let to_{id}: Option<ObjectId> = ({recipient}).first().copied(); if let (Some(&from), Some(to)) = (sources_{id}.first(), to_{id}) {{").unwrap();
                self.plan_code(
                    "move counters of kinds the mover chooses",
                    &format!("done_{id} += cantrip_mechanics::counters::move_chosen(g, actor_{id}, from, to, {least}, {most})?"),
                    out,
                );
                writeln!(out, "}}").unwrap();
                if self.read_later(result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                }
                return Ok(());
            }
        };
        writeln!(out, "let mut done_{id}: i64 = 0; let actor_{id} = {actor}; let sources_{id} = {sources}; let to_{id}: Option<ObjectId> = ({recipient}).first().copied(); if let Some(to) = to_{id} {{ for from in sources_{id} {{ for kind in {kinds} {{ {step} }} }} }}").unwrap();
        if self.read_later(result) {
            self.performed.insert(result.0, format!("done_{id}"));
        }
        Ok(())
    }
    /// "Put its counters on target creature": what it has, or last had, of
    /// each kind goes on together, and none comes off it (CR 122.6, 608.2h).
    fn put_counters_of(
        &mut self,
        id: usize,
        from: &ObjectSet,
        to: &ObjectSet,
        kind: Option<&crate::counter::CounterKind>,
        out: &mut String,
    ) -> Result<()> {
        let sources = self.fetch(self.objects(from)?);
        let recipients = self.acted_on(to)?;
        let only = match kind {
            Some(kind) => format!(".filter(|put| put.1 == {})", read::counter_kind(kind)?),
            None => String::new(),
        };
        writeln!(out, "let sources_{id}: Vec<[i16; CounterKind::COUNT]> = ({sources}).iter().map(|&from| g.obj(from).counter_array()).collect(); let recipients_{id}: Vec<ObjectId> = {recipients}; let puts_{id}: Vec<(ObjectId, CounterKind, i16)> = recipients_{id}.iter().flat_map(|&to| sources_{id}.iter().flat_map(move |counts| CounterKind::ALL.into_iter().map(move |kind| (to, kind, counts[kind as usize])))){only}.collect(); g.add_counters_together(&puts_{id});").unwrap();
        Ok(())
    }
    /// "Copy it" (CR 707.10), a spell's copy with what it is excepted in
    /// (CR 707.9). A copy of a spell with an X has that X, read from the spell
    /// when a trigger of its own makes the copies.
    #[allow(clippy::too_many_arguments)]
    fn copy_on_stack(
        &mut self,
        id: usize,
        player: &PlayerRef,
        object: &ObjectRef,
        times: &Expr,
        new_targets: bool,
        effects: &[Operand],
        out: &mut String,
    ) -> Result<()> {
        let copier = self.fetch(self.certain_player(player)?);
        let again = match times {
            Expr::Constant(1) => None,
            Expr::AnnouncedX if matches!(object, ObjectRef::Source) => {
                Some(self.fetch("u32::try_from(i64::from(q.obj(asker.source).x)).unwrap_or(0)"))
            }
            times => {
                let runs = self.number(times)?.amount(u32::MAX.into(), "u32")?;
                Some(self.fetch(runs))
            }
        };
        if let Some(runs) = &again {
            writeln!(out, "let runs_{id}: u32 = {runs};").unwrap();
        }
        let (open, close) = match again {
            Some(_) => (format!("for _ in 0..runs_{id} {{ "), " }"),
            None => (String::new(), ""),
        };
        let copy_ability = format!("g.copy_ability(t, copier_{id}, {new_targets})?;");
        let slot = match object {
            ObjectRef::Target { slot, .. } if self.stack_slot(slot) => {
                self.stack_targets(&ObjectSet::One(object.clone()))
            }
            _ => None,
        };
        let plain = format!("g.copy_spell(spell, copier_{id}, {new_targets})?;");
        if let Some(listed) = slot {
            if !effects.is_empty() {
                return Err(gap("operation CopyOnStack"));
            }
            writeln!(out, "let copier_{id} = {copier}; if let Some(t) = ({listed}).first().copied() {{ {open}match t {{ Target::Object(spell) => {{ {plain} }} t => {{ {copy_ability} }} }}{close} }}").unwrap();
            return Ok(());
        }
        if matches!(object, ObjectRef::EventStackObject) && self.frame.spell_or_ability {
            if !effects.is_empty() {
                return Err(gap("operation CopyOnStack"));
            }
            writeln!(
                out,
                "let copier_{id} = {copier}; {open}match item.subject {{ Target::Object(spell) => {{ {plain} }} _ => {{ let t = Target::Ability(item.data[1]); {copy_ability} }} }}{close}"
            )
            .unwrap();
            return Ok(());
        }
        if matches!(object, ObjectRef::EventStackObject) && self.frame.activated.is_some() {
            if !effects.is_empty() {
                return Err(gap("operation CopyOnStack"));
            }
            let ability = self.carried_ability()?;
            writeln!(
                out,
                "let copier_{id} = {copier}; let t = {ability}; {open}{copy_ability}{close}"
            )
            .unwrap();
            return Ok(());
        }
        let mut list = self.acted_on(&ObjectSet::One(object.clone()))?;
        if list.contains("item.subject.object()") {
            list = list.replace(
                ".into_iter().filter(|&o| q.state().obj(o).became.is_none()).collect::<Vec<ObjectId>>()",
                "",
            );
        }
        writeln!(out, "let copier_{id} = {copier}; let spell_{id}: Option<ObjectId> = ({list}).first().copied();").unwrap();
        if effects.is_empty() {
            writeln!(
                out,
                "if let Some(spell) = spell_{id} {{ {open}{plain}{close} }}"
            )
            .unwrap();
        } else {
            let values = self.copy_values(&format!("spell_{id}"), effects)?;
            writeln!(out, "let values_{id} = {values}; if let (Some(spell), Some(values)) = (spell_{id}, values_{id}) {{ {open}g.copy_spell_as(spell, copier_{id}, {new_targets}, values)?;{close} }}").unwrap();
        }
        Ok(())
    }
    pub(super) fn one_shot(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        match op {
            // "The copy targets [this]": copied whatever it can target, and what it
            // can't stays as the original's (115.7b), every instance of it.
            Operation::CopyOnStack {
                actor: PlayerSet::One(player),
                object,
                times: Expr::Constant(1),
                targets: NewTargets::To(to) | NewTargets::Each(to @ ObjectSet::Created(_)),
                effects,
                card: None,
            } if effects.is_empty() => {
                let copier = self.fetch(self.certain_player(player)?);
                let spell = self.fetch(self.object(object)?);
                let to = self.fetch(self.objects(to)?);
                writeln!(out, "let copier_{id} = {copier}; let spell_{id} = {spell}; let to_{id}: Vec<ObjectId> = {to}; if let Some(spell) = spell_{id} {{ if let Some(copy) = g.copy_spell(spell, copier_{id}, false)? {{ if let Some(&to) = to_{id}.first() {{ let count = {{ let q: &dyn Query = g; q.state().stack.iter().find(|i| i.kind == StackKind::Spell && i.object == copy).map_or(0, |i| q.state().targets(i).len()) }}; for _ in 0..count {{ g.change_a_target_to(Target::Object(copy), to, copier_{id})?; }} }} }} }}").unwrap();
            }
            Operation::CopyOnStack {
                actor: PlayerSet::One(player),
                object,
                times,
                targets: targets @ (NewTargets::Same | NewTargets::May),
                effects,
                card: None,
            } => {
                let new_targets = matches!(targets, NewTargets::May);
                self.copy_on_stack(id, player, object, times, new_targets, effects, out)?;
            }
            Operation::Retarget {
                actor: PlayerSet::One(player),
                object,
                targets:
                    targets @ (NewTargets::Must
                    | NewTargets::May
                    | NewTargets::To(ObjectSet::One(ObjectRef::Source))),
            } => self.retarget(id, player, object, targets, out)?,
            Operation::MoveCounters {
                actor: PlayerSet::One(player),
                from,
                to: to @ ObjectSet::One(_),
                kind,
                amount,
            } if self.moves_counters(from, kind.as_ref(), amount) => {
                self.move_counters(id, result, player, from, to, kind.as_ref(), amount, out)?;
            }
            Operation::DuplicateCounters {
                from: Recipients::Objects(from),
                to: Recipients::Objects(to),
                copies: CounterCopies::All,
                kind,
            } => self.put_counters_of(id, from, to, kind.as_ref(), out)?,
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
                let child = emit_face(&card, Some(&name), self.quoted_text())?;
                self.gaps
                    .extend(child.gaps.iter().map(|g| g.clone().at("emblem")));
                writeln!(self.functions, "pub mod emblem_{id} {{\n{}\n}}", child.rust).unwrap();
                let path = format!("emblem_{id}::CARD");
                self.extras.push(path.clone());
                self.extras.extend(
                    child
                        .extras
                        .into_iter()
                        .map(|p| format!("emblem_{id}::{p}")),
                );
                writeln!(out, "for player in {} {{ cantrip_mechanics::tokens::create_emblem(g, player, &{path}); }}", self.fetch(self.players(actor)?)).unwrap();
            }
            Operation::KeywordEach {
                actor,
                action,
                objects,
                until,
            } => self.keyword_each(id, actor, action, objects, until.as_ref(), result, out)?,
            Operation::Move {
                objects,
                destination: Destination::Exile,
                entry,
                face_down: true,
            } if effects::plain_entry(entry) => {
                let by = match (self.links_exile, self.stack_item) {
                    (true, true) => "Some(g.linked(item))".to_string(),
                    // A replacement's `then`: what linked abilities know its text by.
                    (true, false) if self.pending != Pending::Entering => {
                        "Some(g.text_linked(asker.source))".to_string()
                    }
                    (true, false) => planned::planned_value(
                        "exile outside a stack item that an ability reads as exiled with its source",
                        "None",
                        "Some(g.linked_to(asker.source))",
                    ),
                    (false, _) => "None".to_string(),
                };
                writeln!(out, "let objects_{id} = {}; let by_{id} = {by}; let moved_{id} = g.exile_face_down(&objects_{id}, by_{id});", self.fetch(self.objects(objects)?)).unwrap();
                self.moved.insert(result.0, format!("moved_{id}"));
                self.left_behind(result, effects::gone(&format!("objects_{id}")));
            }
            Operation::Move {
                objects,
                destination,
                entry,
                face_down: true,
            } => self.move_objects_p(id, objects, destination, entry, true, result, out)?,
            Operation::Conjure { .. } => self.conjure(id, op, result, out)?,
            Operation::DraftFromSpellbook {
                actor,
                spellbook,
                times,
            } => {
                let times = self.number(times)?.amount(u32::MAX.into(), "u32")?;
                let call = planned::planned_value(
                    "operation DraftFromSpellbook",
                    "Vec::<ObjectId>::new()",
                    &format!("cantrip_mechanics::conjure::draft_from_spellbook(g, player, asker.source, {spellbook:?}, times_{id})?"),
                );
                writeln!(out, "let times_{id}: u32 = {}; let mut drafted_{id}: Vec<ObjectId> = Vec::new(); for player in {} {{ drafted_{id}.extend({call}); }}", self.fetch(times), self.fetch(self.players(actor)?)).unwrap();
                self.creations.insert(result.0, format!("drafted_{id}"));
                self.moved.insert(result.0, format!("drafted_{id}"));
            }
            Operation::Search { .. } => self.search_zones_p(id, op, result, out)?,
            Operation::ChangeZone { .. } => self.change_zone_p(id, op, result, out)?,
            Operation::Manifest { .. } => self.manifest(id, op, result, out)?,
            Operation::MoveTopCard { .. } => self.move_top_card(id, op, result, out)?,
            Operation::EffectOn { .. } => self.effect_on(id, op, out)?,
            other => self.resolving(id, other, result, out)?,
        }
        Ok(())
    }
    /// `then` under a condition: the same abilities, or those of its otherwise
    /// with the condition turned over.
    pub(super) fn conditional(
        &mut self,
        condition: &Condition,
        then: &[Ability],
        label: &str,
        parts: &mut Parts,
        scope: Option<costs::Scope>,
    ) -> Result<()> {
        self.static_guard = Some(condition.clone());
        let mut compiled = Ok(());
        for (i, inner) in then.iter().enumerate() {
            let label = format!("{label}_{i}");
            let step = match inner {
                Ability::Static { .. } => self.ability(inner, &label, parts, scope),
                Ability::Keyword(k) => self.static_ability(
                    &label,
                    &StaticKind::Ordinary,
                    &EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                    &[Change::GrantKeyword(k.clone())],
                    parts,
                ),
                Ability::Triggered { .. } | Ability::Replacement { .. } => {
                    self.ability_guard = Some(condition.clone());
                    self.static_guard = None;
                    let step = self.ability(inner, &label, parts, scope);
                    self.static_guard = Some(condition.clone());
                    self.ability_guard = None;
                    step
                }
                // As long as it holds it can be activated, and only then.
                Ability::Activated { .. } => {
                    let mut gated = inner.clone();
                    if let Ability::Activated { limits, .. } = &mut gated {
                        limits.condition = Some(match limits.condition.take() {
                            Some(own) => Condition::All(vec![condition.clone(), own]),
                            None => condition.clone(),
                        });
                    }
                    self.static_guard = None;
                    let step = self.ability(&gated, &label, parts, scope);
                    self.static_guard = Some(condition.clone());
                    step
                }
                Ability::AttackCap { defender, amount } => {
                    self.conditional_cap(condition, defender, amount, &label, parts)
                }
                other => Err(gap(format!("conditional {}", form(other)))),
            };
            if step.is_err() {
                compiled = step;
                break;
            }
        }
        self.static_guard = None;
        compiled
    }
    /// Whether anything after this instruction in its ability reads `result`.
    pub(super) fn read_later(&self, result: &ResultId) -> bool {
        let mark = format!("ResultId({})", result.0);
        self.after
            .iter()
            .any(|instruction| format!("{instruction:?}").contains(&mark))
    }
    /// A keyword action on each of a set. What the rules do to them together is
    /// one call; the others go through the action on one object.
    #[allow(clippy::too_many_arguments)]
    fn keyword_each(
        &mut self,
        id: usize,
        actor: &PlayerSet,
        action: &KeywordAction,
        objects: &ObjectSet,
        until: Option<&crate::ir::Duration>,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        use KeywordAction as K;
        const M: &str = "cantrip_mechanics";
        let controller = matches!(actor, PlayerSet::One(PlayerRef::AbilityController));
        if lane_n::read_as_objects(&self.after, result) {
            let acted = self.acted_on(objects)?;
            writeln!(out, "let acted_{id}: Vec<ObjectId> = {acted};").unwrap();
            self.moved.insert(result.0, format!("acted_{id}"));
            self.selections.insert(result.0, format!("acted_{id}"));
        }
        // Actions with no call yet: what the card will say once there is one.
        let planned = match action {
            K::Detain => Some((
                "keyword action Detain",
                format!("{M}::detain::detain(g, item, object)"),
            )),
            K::Unsuspect => Some((
                "keyword action Unsuspect",
                "g.unsuspect(object)".to_string(),
            )),
            K::Unprepare => Some((
                "keyword action Unprepare",
                "g.unprepare(object)".to_string(),
            )),
            K::Ungoad => Some((
                "keyword action Ungoad",
                format!("{M}::goad::ungoad(g, object)"),
            )),
            K::BecomeBlocked => Some((
                "keyword action BecomeBlocked",
                "g.become_blocked(object)".to_string(),
            )),
            K::TurnFaceDown => Some((
                "keyword action TurnFaceDown",
                "g.turn_face_down(object)".to_string(),
            )),
            K::RemoveFromCombat => Some((
                "engine: no effect removes from combat",
                "g.remove_from_combat(object)".to_string(),
            )),
            _ => None,
        };
        match (action, until) {
            (_, None) if planned.is_some() => {
                let (feature, call) = planned.expect("checked");
                let objects = self.acted_on(objects)?;
                self.plan_code(
                    feature,
                    &format!("for object in {objects} {{ {call}; }}"),
                    out,
                );
            }
            (K::PhaseOut, None) => writeln!(
                out,
                "let objects_{id} = {}; g.phase_out(&objects_{id});",
                self.acted_on(objects)?
            )
            .unwrap(),
            (K::Airbend, None) => writeln!(
                out,
                "let objects_{id} = {}; {M}::airbend_all(g, &objects_{id});",
                self.acted_on(objects)?
            )
            .unwrap(),
            (K::Cipher, None) if controller && self.stack_item => {
                const TRIGGER: &str = "cantrip_mechanics::cipher::TRIGGER";
                match self.delayed.first().map(String::as_str) {
                    Some(TRIGGER) => {}
                    None => self.delayed.push(TRIGGER.into()),
                    Some(_) => {
                        // Cantrip reads the trigger from the first delayed ability.
                        let at = self.delayed.len();
                        self.delayed.push(TRIGGER.into());
                        self.plan_code(
                            "cipher beside another first delayed ability",
                            &format!("{M}::cipher::encode_with_trigger_at(g, item, {at})?"),
                            out,
                        );
                        return Ok(());
                    }
                }
                self.cipher = true;
                writeln!(out, "{M}::cipher::encode(g, item)?;").unwrap();
            }
            (K::Goad, Some(until)) if controller && self.stack_item => {
                let duration = self.duration(until)?;
                writeln!(out, "let objects_{id} = {}; let duration_{id} = {duration}; for object in objects_{id} {{ {M}::goad::goad_for(g, item, object, duration_{id}); }}", self.acted_on(objects)?).unwrap();
            }
            (K::PhaseOut, Some(until)) => {
                let duration = self.stated_until(until)?;
                let code = format!(
                    "let objects_{id} = {}; for object in objects_{id} {{ g.phase_out_until(object, {duration}); }}",
                    self.acted_on(objects)?
                );
                self.plan_code("keyword action with a duration of its own", &code, out);
            }
            (_, Some(_)) => return Err(gap("keyword action with a duration of its own")),
            _ => {
                let binding = u32::MAX - id as u32;
                let each = ObjectRef::Bound(Binding(binding));
                writeln!(
                    out,
                    "for bound_{binding} in {} {{",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.bound_objects.insert(binding);
                let one = keywords::Action {
                    actor,
                    action: action.clone(),
                    object: Some(&each),
                    other: None,
                    amount: None,
                    subtype: None,
                };
                let inner = self.fresh();
                let compiled = self.keyword_action(inner, one, result, out);
                self.bound_objects.remove(&binding);
                compiled?;
                writeln!(out, "}}").unwrap();
            }
        }
        Ok(())
    }
    /// A move's arrival past what `arrival` knows, as one object's `each`.
    pub(super) fn arrival_in_state(&mut self, entry: &EntrySpec, counters: &str) -> Result<String> {
        let tapped = entry.tapped;
        Ok(match entry.states.as_slice() {
            [EntryState::Transformed] => {
                let call = match counters.is_empty() {
                    true => format!("g.put_onto_battlefield_transformed(o, controller, {tapped})?"),
                    false => format!("g.put_onto_battlefield_transformed_with_counters(o, controller, {tapped}, &[{counters}])?"),
                };
                format!("let entered = {call}; if g.is_on_battlefield(entered) {{ moved.push(entered); }}")
            }
            [EntryState::FaceDown] if counters.is_empty() => {
                let call = planned::planned_value(
                    "move arrival FaceDown",
                    "None::<ObjectId>",
                    &format!("g.put_onto_battlefield_face_down(o, controller, FaceDown::Bare, {tapped})?"),
                );
                format!("moved.extend({call});")
            }
            [EntryState::AttachedToPlayer(player)] if counters.is_empty() && !tapped => {
                let player = self.fetch(self.player(player)?);
                let call = planned::planned_value(
                    "move arrival AttachedToPlayer",
                    "None::<ObjectId>",
                    &format!("{player}.and_then(|player| g.put_onto_battlefield_enchanting(o, controller, player)?)"),
                );
                format!("moved.extend({call});")
            }
            _ => self.arrival_rest(entry, counters)?,
        })
    }
    /// Objects that enter together as `changes` says (CR 614.12): one static
    /// `EntersAs`, read by the engine as they enter.
    pub(super) fn entering_as(
        &mut self,
        id: usize,
        entry: &EntrySpec,
        changes: &[Change],
        controller: &str,
    ) -> Result<String> {
        if !self.stack_item || entry.tapped {
            return Err(gap("move arrival state"));
        }
        let mut fields = Vec::new();
        let mut adds = Vec::new();
        let mut subtypes = Vec::new();
        let mut keywords = Vec::new();
        let mut abilities = Vec::new();
        for change in changes {
            match change {
                Change::SetCardTypes(types) => {
                    let types = types
                        .iter()
                        .map(|t| card_type(t).map(|t| format!("CardType::{t}")))
                        .collect::<Result<Vec<_>>>()?;
                    fields.push(format!("types: Some(TypeSet::of(&[{}]))", types.join(", ")));
                }
                Change::AddCardType(t) => adds.push(format!("CardType::{}", card_type(t)?)),
                Change::CreatureTypes {
                    mode: ChangeMode::Add,
                    types: CreatureTypeSet::Listed(types),
                } => {
                    for t in types {
                        let word = name(t);
                        if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                            return Err(gap(format!("subtype {word}")));
                        }
                        subtypes.push(format!("Subtype::{word}"));
                    }
                }
                // "It's a Spirit Detective": the creature types it had are gone.
                Change::CreatureTypes {
                    mode: ChangeMode::Set,
                    types: CreatureTypeSet::Listed(types),
                } => {
                    for t in types {
                        let word = name(t);
                        if !crate::cantrip_vocab::SUBTYPES.contains(&word.as_str()) {
                            return Err(gap(format!("subtype {word}")));
                        }
                        subtypes.push(format!("Subtype::{word}"));
                    }
                    fields.push("loses_subtypes_of: &[CardType::Creature]".to_string());
                }
                Change::SetBasePowerToughness {
                    power: Expr::Constant(power),
                    toughness: Expr::Constant(toughness),
                } => fields.push(format!("power_toughness: Some(({power}, {toughness}))")),
                Change::GrantAbility(ability) => {
                    abilities.push(self.grant(ability)?.to_string());
                }
                Change::GrantKeyword(k)
                    if !matches!(k, Keyword::Protection(_) | Keyword::Ward(_)) =>
                {
                    keywords.push(keyword(k)?)
                }
                Change::RemoveAllAbilities => {
                    fields.push("loses_other_abilities: true".to_string())
                }
                Change::Colors { .. } => return Err(gap("move arrival with a color")),
                _ => return Err(gap("move arrival state")),
            }
        }
        if !adds.is_empty() {
            fields.push(format!("adds_types: TypeSet::of(&[{}])", adds.join(", ")));
        }
        if !subtypes.is_empty() {
            fields.push(format!("subtypes: &[{}]", subtypes.join(", ")));
        }
        if !keywords.is_empty() {
            fields.push(format!(
                "keywords: Keywords::of(&[{}])",
                keywords.join(", ")
            ));
        }
        if !abilities.is_empty() {
            fields.push(format!("abilities: &[{}]", abilities.join(", ")));
        }
        writeln!(
            self.functions,
            "static ENTERS_{id}: EntersAs = EntersAs {{ {}, ..EntersAs::NONE }};",
            fields.join(", ")
        )
        .unwrap();
        let mut counters = Vec::new();
        for (kind, n) in &entry.counters {
            let n = self.number(n)?.amount(i16::MAX.into(), "i16")?;
            counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
        }
        Ok(format!("{{ let rows: Vec<(ObjectId, PlayerId)> = objects_{id}.iter().map(|&o| (o, {controller})).collect(); let counters: Vec<(CounterKind, i16)> = vec![{}]; g.put_all_onto_battlefield_becoming(&rows, item, &ENTERS_{id}, &counters)? }}", counters.join(", ")))
    }
    /// A conjured card, in the form it will have once Cantrip conjures: its
    /// `what` and `to` are the operation's own, and what it conjured is real.
    fn conjure(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let Operation::Conjure {
            actor,
            card,
            count,
            zone,
            owner,
            spot,
            entry,
        } = op
        else {
            return Err(gap("operation Conjure"));
        };
        let what = match card {
            ConjuredCard::Named(name) => format!("Conjured::Named({name:?})"),
            ConjuredCard::OneOf(names) => format!("Conjured::OneOf(&{names:?})"),
            ConjuredCard::Each(names) => format!("Conjured::Each(&{names:?})"),
            ConjuredCard::Spellbook { book, pick } => {
                format!("Conjured::FromSpellbook({book:?}, SpellbookPick::{pick:?})")
            }
            ConjuredCard::Duplicate(objects) => {
                writeln!(
                    out,
                    "let duplicated_{id} = {};",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                format!("Conjured::DuplicateOf(&duplicated_{id})")
            }
            ConjuredCard::Random(cards) => {
                format!("Conjured::AtRandom({})", self.operand_code(cards))
            }
            ConjuredCard::SpellbookMatching { book, cards } => format!(
                "Conjured::FromSpellbookMatching({book:?}, {})",
                self.operand_code(cards)
            ),
        };
        let tapped = conjured_tapped(entry).ok_or_else(|| gap("operation Conjure"))?;
        let place = |c: &Self, n: &Expr| -> Result<String> {
            Ok(c.fetch(c.number(n)?.amount(u32::MAX.into(), "usize")?))
        };
        let to = match (zone, spot) {
            (EventZone::Hand, None) => "ConjureTo::Hand".to_string(),
            (EventZone::Graveyard, None) => "ConjureTo::Graveyard".to_string(),
            (EventZone::Exile, None) => "ConjureTo::Exile".to_string(),
            (EventZone::Battlefield, None) => {
                format!("ConjureTo::Battlefield {{ tapped: {tapped} }}")
            }
            (EventZone::Library, None) => "ConjureTo::Library(ConjureSpot::Random)".to_string(),
            (EventZone::Library, Some(ConjureSpot::Shuffled)) => {
                "ConjureTo::Library(ConjureSpot::Shuffled)".to_string()
            }
            (EventZone::Library, Some(ConjureSpot::Top)) => {
                "ConjureTo::Library(ConjureSpot::Top)".to_string()
            }
            (EventZone::Library, Some(ConjureSpot::AmongTop(n))) => {
                format!(
                    "ConjureTo::Library(ConjureSpot::AmongTop({}))",
                    place(self, n)?
                )
            }
            (EventZone::Library, Some(ConjureSpot::FromTop(n))) => {
                format!(
                    "ConjureTo::Library(ConjureSpot::FromTop({}))",
                    place(self, n)?
                )
            }
            _ => return Err(gap("operation Conjure")),
        };
        let owner = match owner {
            Some(owner) => self.fetch(self.certain_player(owner)?),
            None => "player".to_string(),
        };
        let count = self.number(count)?.amount(u32::MAX.into(), "u32")?;
        let call = planned::planned_value(
            "operation Conjure",
            "Vec::<ObjectId>::new()",
            &format!("cantrip_mechanics::conjure::conjure(g, player, {owner}, asker.source, {what}, count_{id}, {to})?"),
        );
        writeln!(out, "let count_{id}: u32 = {}; let mut conjured_{id}: Vec<ObjectId> = Vec::new(); for player in {} {{ conjured_{id}.extend({call}); }}", self.fetch(count), self.fetch(self.players(actor)?)).unwrap();
        self.creations.insert(result.0, format!("conjured_{id}"));
        self.moved.insert(result.0, format!("conjured_{id}"));
        Ok(())
    }
    /// Cards change zones: all of them, or those the actor or chance picks.
    pub(super) fn change_zone(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let unsupported = || gap("operation ChangeZone");
        let Operation::ChangeZone {
            actor,
            cards,
            count,
            random,
            group,
            destination,
            entry,
            face_down: None,
        } = op
        else {
            return Err(unsupported());
        };
        let rule = match group {
            None => None,
            Some(group) if count.is_some() && !random => {
                Some(self.group_rule(group, cards).ok_or_else(unsupported)?)
            }
            Some(_) => return Err(unsupported()),
        };
        let (entry, mut destination) = entry_of(entry, destination).ok_or_else(unsupported)?;
        let held = ResultId(u32::MAX - id as u32);
        let picked = ObjectSet::Selected(held.clone());
        let objects = match (count, random) {
            (None, false) => cards.clone(),
            (None, true) => {
                match destination {
                    Destination::OwnersLibraryBottom | Destination::OwnersLibraryBottomRandom => {}
                    _ => return Err(unsupported()),
                }
                destination = Destination::OwnersLibraryBottomRandom;
                cards.clone()
            }
            (Some(Count::Exactly(n) | Count::AsManyAsCan(n)), true) => {
                let pool = self.fetch(self.objects(cards)?);
                let n = self.fetch(self.number(n)?.amount(u32::MAX.into(), "usize")?);
                writeln!(out, "let mut pool_{id} = {pool}; let count_{id} = {n}; let mut picked_{id}: Vec<ObjectId> = Vec::new(); while picked_{id}.len() < count_{id} && !pool_{id}.is_empty() {{ let at = g.choose_at_random(pool_{id}.len()); picked_{id}.push(pool_{id}.remove(at)); }}").unwrap();
                self.selections.insert(held.0, format!("picked_{id}"));
                picked.clone()
            }
            (Some(_), true) => return Err(unsupported()),
            (Some(count), false) => {
                let PlayerSet::One(player) = actor else {
                    return Err(unsupported());
                };
                let rest = [Instruction::Effect {
                    result: result.clone(),
                    operation: Operation::Move {
                        objects: picked.clone(),
                        destination: destination.clone(),
                        entry: entry.clone(),
                        face_down: false,
                    },
                }];
                let purpose = effects::purpose(&held, &rest).ok_or_else(unsupported)?;
                self.choice_rule = rule.unwrap_or_default();
                self.choose(player, cards, count, &held, purpose, out)?;
                picked.clone()
            }
        };
        if count.as_ref().is_some_and(effects::one_at_most) {
            self.single.insert(held.0);
        }
        // "Your choice of the top or bottom": `move_objects` asks the owner.
        let moved = if let (Destination::OwnersLibraryTopOrBottom, PlayerSet::One(player)) =
            (&destination, actor)
        {
            self.chooses_top_or_bottom(id, player, &objects, result, out)
        } else {
            let inner = self.fresh();
            self.move_objects(inner, &objects, &destination, &entry, result, out)
        };
        self.selections.remove(&held.0);
        self.single.remove(&held.0);
        moved
    }
    fn chooses_top_or_bottom(
        &mut self,
        id: usize,
        player: &PlayerRef,
        objects: &ObjectSet,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let player = self.fetch(self.certain_player(player)?);
        let objects = self.fetch(self.objects(objects)?);
        writeln!(self.functions, "static TOP_OR_BOTTOM_{id}: Question = Question::new(\"PutOnTop\", \"put it on top of its owner's library?\", \"If you don't, it goes on the bottom\", \"Top\", \"Bottom\");").unwrap();
        writeln!(out, "let objects_{id} = {objects}; let player_{id} = {player}; let top_{id} = cantrip_mechanics::effects::ask_may(g, player_{id}, asker.source, Purpose::Ask(&TOP_OR_BOTTOM_{id}))?; let moved_{id} = g.put_into_library(&objects_{id}, if top_{id} {{ LibraryPlace::Top }} else {{ LibraryPlace::Bottom }});").unwrap();
        self.moved.insert(result.0, format!("moved_{id}"));
        self.left_behind(result, effects::gone(&format!("objects_{id}")));
        Ok(())
    }
    /// CR 701.40, 701.58: face down as a 2/2. Cards of a library go one at a time
    /// (701.40c), and each sees the one before it enter.
    fn manifest(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let Operation::Manifest {
            actor: PlayerSet::One(actor),
            source,
            cloak,
            entry,
        } = op
        else {
            return Err(gap("operation Manifest"));
        };
        let tapped = conjured_tapped(entry).ok_or_else(|| gap("operation Manifest"))?;
        let how = if *cloak {
            "FaceDown::Cloaked"
        } else {
            "FaceDown::Manifested"
        };
        let controller = self.fetch(self.certain_player(actor)?);
        writeln!(out, "let controller_{id} = {controller};").unwrap();
        match source {
            ManifestSource::TopOfLibrary { player, count } => {
                let owner = self.fetch(self.certain_player(player)?);
                let count = self.fetch(self.number(count)?.amount(u32::MAX.into(), "usize")?);
                writeln!(out, "let owner_{id} = {owner}; let count_{id} = {count}; let mut entered_{id}: Vec<ObjectId> = Vec::new(); for _ in 0..count_{id} {{ let Some(&card) = cantrip_mechanics::top_of_library(g, owner_{id}, 1).first() else {{ break; }}; entered_{id}.extend(g.put_onto_battlefield_face_down(card, controller_{id}, {how}, {tapped})?); }}").unwrap();
                self.moved.insert(result.0, format!("entered_{id}"));
            }
            ManifestSource::Cards(cards) => {
                writeln!(out, "let cards_{id} = {}; let entered_{id}: Vec<ObjectId> = g.put_all_onto_battlefield_face_down(&cards_{id}, controller_{id}, {how}, {tapped})?.into_iter().flatten().collect();", self.fetch(self.objects(cards)?)).unwrap();
                self.moved.insert(result.0, format!("entered_{id}"));
                self.left_behind(result, effects::gone(&format!("cards_{id}")));
            }
        }
        Ok(())
    }
    /// "Reveal the top card. If it's a land, put it into your hand."
    fn move_top_card(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let Operation::MoveTopCard {
            actor: PlayerSet::One(player),
            cards,
            reveal,
            destination,
            entry,
        } = op
        else {
            return Err(gap("operation MoveTopCard"));
        };
        let (entry, destination) =
            entry_of(entry, destination).ok_or_else(|| gap("operation MoveTopCard"))?;
        let filter = self.filter(cards, &Candidate::of("id"))?;
        writeln!(out, "let player_{id} = {}; let top_{id} = cantrip_mechanics::top_of_library(g, player_{id}, 1);", self.fetch(self.certain_player(player)?)).unwrap();
        if *reveal {
            writeln!(out, "g.reveal(player_{id}, &top_{id});").unwrap();
        }
        writeln!(out, "let matching_{id}: Vec<ObjectId> = {{ let q: &dyn Query = g; top_{id}.iter().copied().filter(|&id| {filter}).collect() }};").unwrap();
        let held = ResultId(u32::MAX - id as u32);
        self.selections.insert(held.0, format!("matching_{id}"));
        self.single.insert(held.0);
        let revealed = std::mem::replace(&mut self.revealed, *reveal);
        let inner = self.fresh();
        let objects = ObjectSet::Selected(held.clone());
        let moved = self.move_objects(inner, &objects, &destination, &entry, result, out);
        self.revealed = revealed;
        self.selections.remove(&held.0);
        self.single.remove(&held.0);
        moved
    }
    /// What the objects get: what the engine has words for, and what it waits on.
    fn effect_on(&mut self, id: usize, op: &Operation, out: &mut String) -> Result<()> {
        let unsupported = || gap("operation EffectOn");
        let Operation::EffectOn {
            subject,
            effects,
            until,
        } = op
        else {
            return Err(unsupported());
        };
        if effects
            .iter()
            .any(|e| matches!(e, Operand::Term { family, .. } if family == "PerpetualEffect"))
        {
            return self.perpetual(id, subject, effects, out);
        }
        let (EffectSubject::Objects(objects), None) = (subject, until) else {
            return self.effect_on_rest(subject, effects, until.as_ref(), out);
        };
        let [Operand::Term {
            family,
            form,
            operands,
        }] = effects.as_slice()
        else {
            return self.effect_on_rest(subject, effects, until.as_ref(), out);
        };
        match (family.as_str(), form.as_str()) {
            ("ExiledCardEffect", "IsPlotted") => {
                writeln!(
                    out,
                    "for card in {} {{ cantrip_mechanics::plot::become_plotted(g, card); }}",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
            }
            ("ExiledCardEffect", "AddAbility" | "AddAbilityIfItDoesntHaveIt")
                if format!("{operands:?}").contains("BasicSuspend") =>
            {
                writeln!(
                    out,
                    "let cards_{id} = {};",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
                self.plan_code(
                    "exiled card gains suspend",
                    &format!("for card in cards_{id} {{ cantrip_mechanics::suspend::gains_suspend(g, card); }}"),
                    out,
                );
            }
            _ => return self.effect_on_rest(subject, effects, until.as_ref(), out),
        }
        Ok(())
    }
    /// A search past one library: "and/or" (the searcher picks whether the library
    /// is searched), or a list of zones searched one by one, the library last.
    pub(super) fn search_zones(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<()> {
        let Operation::Search {
            player,
            library,
            top: None,
            choices,
            also,
            either,
            outside: false,
        } = op
        else {
            return Err(gap("operation Search"));
        };
        let [choice] = choices.as_slice() else {
            return Err(gap("search with several find clauses"));
        };
        if let Some(constraint) = choice.constraints.first() {
            return Err(gap(format!("search constraint {}", form(constraint))));
        }
        if also.is_empty() {
            return Err(gap("operation Search"));
        }
        let zone_of = |zone: &EventZone, owner: &str| match zone {
            EventZone::Graveyard => Ok((
                format!("Zone::graveyard({owner})"),
                "graveyard",
                "Graveyard",
            )),
            EventZone::Hand => Ok((format!("Zone::hand({owner})"), "hand", "Hand")),
            _ => Err(gap("operation Search")),
        };
        if effects::one_at_most(&choice.count) {
            self.single.insert(result.0);
        }
        let filter = self.filter(&choice.filter, &Candidate::of("id"))?;
        let destination = effects::search_destination(result, &self.after);
        let purpose = match destination {
            "Battlefield" | "BattlefieldTapped" => "PutOntoBattlefield",
            "Graveyard" => "PutIntoGraveyard",
            "Exile" => "Exile",
            "TopOfLibrary" => "PutOnTop",
            _ => "PutIntoHand",
        };
        let searcher = self.fetch(self.certain_player(player)?);
        let owner = self.fetch(self.certain_player(library)?);
        writeln!(
            out,
            "let searcher_{id} = {searcher}; let owner_{id} = {owner};"
        )
        .unwrap();
        const LIBRARY: &str = "cantrip_mechanics::library";
        const SELECTION: &str = "cantrip_mechanics::select::Selection";
        if *either {
            if format!("{player:?}") != format!("{library:?}") {
                return Err(gap("operation Search"));
            }
            let [zone] = also.as_slice() else {
                return Err(gap("operation Search"));
            };
            let (zone, word, title) = zone_of(zone, &format!("searcher_{id}"))?;
            writeln!(self.functions, "static ASK_{id}: Question = Question::new(\"SearchLibrary\", \"search your library as well as your {word}?\", \"Then shuffle. If you don't, you search only your {word}\", \"Search both\", \"{title} only\");").unwrap();
            let count = self.fetch(self.select_count(&choice.count)?);
            writeln!(out, "let rule_count_{id} = {count}; let zone_{id} = {zone}; let mut shown_{id} = {LIBRARY}::searched_cards(g, searcher_{id}); shown_{id}.extend(g.state().zone(zone_{id}).iter().copied()); let can_{id}: Vec<ObjectId> = {{ let q: &dyn Query = g; shown_{id}.iter().copied().filter(|&id| {filter}).collect() }}; let (found_{id}, searched_{id}) = {LIBRARY}::search_zone_and_or_library(g, searcher_{id}, asker.source, zone_{id}, SearchDestination::{destination}, Purpose::{purpose}, &ASK_{id}, {SELECTION}::listed(rule_count_{id}, &shown_{id}, can_{id}))?;").unwrap();
            self.searched.insert(result.0, format!("searched_{id}"));
        } else {
            // One search, with the cards of each zone chosen in the order named:
            // "up to" holds across them, and the library is searched last.
            let limit = match &choice.count {
                Count::AnyNumber => None,
                Count::UpTo(n) => Some(self.fetch(self.number(n)?.amount(u32::MAX.into(), "u32")?)),
                _ => return Err(gap("operation Search")),
            };
            let count = |id: usize| match limit {
                Some(_) => format!("cantrip_mechanics::select::Count::UpTo(left_{id})"),
                None => "cantrip_mechanics::select::Count::AnyNumber".to_string(),
            };
            let spend = |id: usize| match limit {
                Some(_) => format!("left_{id} -= taken.len() as u32;"),
                None => String::new(),
            };
            if let Some(limit) = &limit {
                writeln!(out, "let mut left_{id}: u32 = {limit};").unwrap();
            }
            writeln!(out, "let mut found_{id}: Vec<ObjectId> = Vec::new();").unwrap();
            for zone in also {
                let (zone, ..) = zone_of(zone, &format!("owner_{id}"))?;
                writeln!(out, "{{ let shown = g.state().zone({zone}).to_vec(); let can: Vec<ObjectId> = {{ let q: &dyn Query = g; shown.iter().copied().filter(|&id| {filter}).collect() }}; if !can.is_empty() {{ let taken = cantrip_mechanics::choose::select(g, searcher_{id}, asker.source, Purpose::{purpose}, shown.clone(), {SELECTION}::listed({}, &shown, can))?; {} found_{id}.extend(taken); }} }}", count(id), spend(id)).unwrap();
            }
            writeln!(out, "{{ let shown = {LIBRARY}::searched_cards(g, owner_{id}); let can: Vec<ObjectId> = {{ let q: &dyn Query = g; shown.iter().copied().filter(|&id| {filter}).collect() }}; let taken = {LIBRARY}::search(g, searcher_{id}, owner_{id}, asker.source, SearchDestination::{destination}, {SELECTION}::listed({}, &shown, can))?; found_{id}.extend(taken); }}", count(id)).unwrap();
            self.searched.insert(result.0, "true".to_string());
        }
        self.selections.insert(result.0, format!("found_{id}"));
        Ok(())
    }
}

/// A target test in a slot of its own for each player: that player controls it.
pub(super) fn per_player_target(count: &Count, test: String) -> String {
    let Count::PerPlayer { players, .. } = count else {
        return test;
    };
    let controls = match players {
        PlayerSet::OpponentsOf(PlayerRef::AbilityController) => {
            "cantrip_mechanics::targets::controlled_by_slot_opponent(q, id)".to_string()
        }
        _ => planned::planned_value(
            "target cardinality PerPlayer",
            "true",
            "cantrip_mechanics::targets::controlled_by_slot_player(q, id)",
        ),
    };
    format!("matches!(t, Target::Object(id) if {controls}) && ({test})")
}

/// `PlayWith` on no terms past what `PlayCards` says, as that.
pub(super) fn plain_play(op: &Operation) -> Option<Operation> {
    let Operation::PlayWith {
        actor,
        action,
        cards,
        count,
        terms,
        group: None,
    } = op
    else {
        return None;
    };
    let PlayTerms {
        limit: None,
        during: None,
        cost: cost @ (PlayCost::Free | PlayCost::Normal),
        spend_as: None,
        into_exile,
        condition: None,
        effects,
        flash: false,
        may_look: false,
        then: None,
        face_down: None,
    } = terms
    else {
        return None;
    };
    effects.is_empty().then(|| Operation::PlayCards {
        actor: actor.clone(),
        action: action.clone(),
        cards: cards.clone(),
        count: count.clone(),
        cost: cost.clone(),
        into_exile: *into_exile,
    })
}

/// A move's flags as a move's entry and destination.
fn entry_of(operands: &[Operand], destination: &Destination) -> Option<(EntrySpec, Destination)> {
    fn read(
        operands: &[Operand],
        entry: &mut EntrySpec,
        destination: &mut Destination,
    ) -> Option<()> {
        for operand in operands {
            match operand {
                Operand::List(flags) => read(flags, entry, destination)?,
                Operand::Term { form, operands, .. } => {
                    match (form.as_str(), operands.as_slice()) {
                        ("EntersNormally", []) => {}
                        ("EntersTapped", []) => entry.tapped = true,
                        ("EntersUnderPlayersControl", [Operand::Player(player)]) => {
                            if !matches!(destination, Destination::OwnersBattlefield) {
                                return None;
                            }
                            *destination = Destination::Battlefield(player.clone());
                        }
                        ("EntersAttachedToPermanent", [Operand::Object(host)]) => {
                            entry.states.push(EntryState::AttachedTo(host.clone()))
                        }
                        _ => return None,
                    }
                }
                _ => return None,
            }
        }
        Some(())
    }
    let mut entry = EntrySpec {
        tapped: false,
        counters: Vec::new(),
        states: Vec::new(),
    };
    let mut destination = destination.clone();
    read(operands, &mut entry, &mut destination)?;
    Some((entry, destination))
}

/// Whether a conjured permanent enters tapped, if that is all its entry says.
fn conjured_tapped(entry: &[Operand]) -> Option<bool> {
    let mut tapped = false;
    for operand in entry {
        match operand {
            Operand::List(flags) => tapped |= conjured_tapped(flags)?,
            Operand::Term { form, .. } if form == "EntersNormally" => {}
            Operand::Term { form, .. } if form == "EntersTapped" => tapped = true,
            _ => return None,
        }
    }
    Some(tapped)
}

fn same_kind(a: ViewKind, b: ViewKind) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}
