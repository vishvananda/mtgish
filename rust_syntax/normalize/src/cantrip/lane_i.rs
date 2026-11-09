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

/// "Each player who chose fame, ... that player": a step the data words as each of
/// one player, the one walked, doing it is that player's own turn at it, where
/// `PlayerRef::Acting` is the player walked.
pub(super) fn own_turns(body: &Program, binding: &Binding) -> Option<Program> {
    let own = |i: &Instruction| matches!(i, Instruction::Effect { operation: Operation::EachPlayer { players, .. }, .. } if one_bound(players, binding));
    if !body.instructions.iter().any(own) {
        return None;
    }
    let instructions = body
        .instructions
        .iter()
        .map(|i| match i {
            Instruction::Effect {
                result,
                operation: Operation::EachPlayer { players, action },
            } if one_bound(players, binding) => Instruction::Effect {
                result: result.clone(),
                operation: (**action).clone(),
            },
            other => other.clone(),
        })
        .collect();
    Some(Program { instructions })
}
/// Whether a part of a per-player body reads the player it is for.
fn names(binding: &Binding, value: &impl crate::visit::Walk) -> bool {
    use crate::visit::{Visitor, Walk};
    struct Reads { binding: u32, found: bool }
    impl Visitor for Reads {
        fn ability(&mut self, _: &Ability) -> bool { false }
        fn player_ref(&mut self, player: &PlayerRef) -> bool {
            self.found |= matches!(player,PlayerRef::Bound(binding) if binding.0==self.binding);
            !self.found
        }
        fn instruction(&mut self, instruction: &Instruction) -> bool {
            if let Instruction::ForEachPlayer { players,binding,starting_with,.. }=instruction {
                if binding.0==self.binding {
                    // The range and start read the outer scope; the body binds
                    // a new player, even if it reuses this numeric ID.
                    players.walk(self);
                    starting_with.iter().for_each(|player|player.walk(self));
                    return false;
                }
            }
            !self.found
        }
    }
    let mut reads=Reads {binding:binding.0,found:false};
    value.walk(&mut reads);
    reads.found
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
        op.action(),
        // Mana goes into a pool, which no permanent sees (CR 106.4): each in turn
        // is the same as all at once.
        Operation::AddMana { .. }
            // Separating into piles and picking one are choices, made in turn.
            | Operation::Piles { .. }
            | Operation::ChoosePile { then: None, .. }
            | Operation::ChooseObjects { .. }
            | Operation::ChooseValue { .. }
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
            // One spell after another, the next player's on top (101.4c).
            | Operation::PlayCards { .. }
            | Operation::PlayWith { .. }
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
/// A walk whose player may pay a cost, and whose later step does something
/// unless they did, all of it about one thing done for them: each player's turn
/// is the whole process.
fn punished_in_turn(flat: &[Instruction], binding: &Binding) -> bool {
    let paid: Vec<u32> = flat
        .iter()
        .filter_map(|step| match step {
            Instruction::Effect {
                result,
                operation:
                    Operation::PayCost {
                        player: PlayerRef::Bound(payer),
                        optional: true,
                        ..
                    },
            } if payer.0 == binding.0 => Some(result.0),
            _ => None,
        })
        .collect();
    // Something done for that player first, by another: "you mill a card". A
    // walk of "each opponent sacrifices unless they pay" is all the choices,
    // then all that happens (CR 101.4), as its steps are.
    let done_for = flat.iter().any(|step| match step {
        Instruction::Effect {
            operation: Operation::Mill { players, .. },
            ..
        } => !names(binding, players),
        Instruction::Effect {
            operation: Operation::Reveal { objects },
            ..
        } => !names(binding, objects),
        _ => false,
    });
    flat.len() <= 3
        && done_for
        && paid.len() == 1
        && flat.iter().any(|step| {
            matches!(step, Instruction::If { condition, .. }
                if format!("{condition:?}").contains(&format!("PaymentChosen(ResultId({}))", paid[0])))
        })
        && flat.iter().all(|step| match step {
            Instruction::Effect { operation, .. } => {
                matches!(operation, Operation::Mill { .. } | Operation::Reveal { .. } | Operation::PayCost { .. })
            }
            Instruction::If { .. } => true,
            _ => false,
        })
}
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
    /// A "may" that draws its player this many cards, not offered to one who can't (CR 121.2b).
    MayDraw(i64),
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
    fn has_choice(&self) -> bool {
        match self {
            Self::WithSelections { steps, inner, .. } => {
                steps
                    .iter()
                    .any(|s| matches!(s.chooser, SelectionChooser::Players(_)))
                    || inner.has_choice()
            }
            _ => false,
        }
    }
    fn prepare_choice_views(&self, local: usize, players:&str, out: &mut String) {
        if let Self::WithSelections {
            steps,
            depth,
            inner,
            ..
        } = self
        {
            if steps
                .iter()
                .any(|s| matches!(s.chooser, SelectionChooser::Players(_)))
            {
                writeln!(
                    out,
                    "let mut implicit_{local}_{depth}: Vec<ObjectId> = Vec::new(); let mut implicit_each_{local}_{depth}: Vec<Vec<ObjectId>> = vec![Vec::new(); {players}.len()];"
                )
                .unwrap();
            }
            inner.prepare_choice_views(local, players, out);
        }
    }

    fn purpose(&self) -> &'static str {
        match self {
            Gathered::WithSelections { inner, .. } => inner.purpose(),
            Gathered::Objects { objects, .. } if matches!(objects,ObjectSet::Difference {excluding,..} if matches!(excluding.as_ref(),ObjectSet::Prepared(_))) => {
                "Keep"
            }
            Gathered::Objects {then,..}=>match then(ObjectSet::Union(vec![])) {
                Operation::Move {destination:Destination::Exile,..}=>"Exile",
                Operation::Move {destination:Destination::OwnersHand|Destination::Hand(_),..}=>"PutIntoHand",
                Operation::Move {destination:Destination::OwnersGraveyard,..}=>"PutIntoGraveyard",
                Operation::Move {destination:Destination::Battlefield(_)|Destination::OwnersBattlefield,..}=>"PutOntoBattlefield",
                _=>"Choose",
            },
            _ => "Choose",
        }
    }
    /// What one thing gathered is.
    fn row(&self) -> &'static str {
        match self {
            Gathered::WithSelections { inner, .. } => inner.row(),
            Gathered::Objects { .. } | Gathered::Offered { .. } | Gathered::Cards { .. } => {
                "ObjectId"
            }
            Gathered::Sacrifice { .. }
            | Gathered::SacrificeEach { .. }
            | Gathered::SacrificeRest { .. } => "(PlayerId, ObjectId)",
            Gathered::Counters { .. } if planned::switched_on(quantity::FEATURE) => "(PlayerId, ObjectId, CounterKind, i64)",
            Gathered::Counters { .. } if planned::switched_on(quantity::BOUNDED_32) => "(PlayerId, ObjectId, CounterKind, i32)",
            Gathered::Counters { .. } => "(PlayerId, ObjectId, CounterKind, i16)",
            Gathered::Mill { .. } => "(PlayerId, u32)",
            Gathered::Damage { .. } if planned::switched_on(quantity::FEATURE) => "(ObjectId, Target, i64)",
            Gathered::Damage { .. } if planned::switched_on(quantity::BOUNDED_32) => "(ObjectId, Target, u32)",
            Gathered::Damage { .. } => "(ObjectId, Target, u16)",
            Gathered::Tokens { .. } => "(CardId, PlayerId)",
            Gathered::TokensAt { .. } => "(CardId, PlayerId, Target)",
            Gathered::TokenCopies { .. } => "(Copiable, PlayerId)",
            Gathered::TokenCopyAt { .. } => "Target",
        }
    }
}
/// What each player's tokens are made to do to their player or planeswalker.
#[derive(Clone, Copy, PartialEq)]
enum TokensAt {
    /// "That's attacking that player" (CR 508.4).
    Attacking,
    /// "That attacks that player this turn if able" (CR 508.1d).
    MustAttack,
}
enum Gathered {
    WithSelections {
        steps: Vec<SelectionStep>,
        inner: Box<Gathered>,
        result: ResultId,
        depth: usize,
    },
    SacrificeEach {
        slots: Vec<ObjectSet>,
        result: ResultId,
    },
    Counters {
        operation: Operation,
        result: ResultId,
    },
    SacrificeRest {
        objects: ObjectSet,
        keep: Expr,
        result: ResultId,
    },
    Objects {
        objects: ObjectSet,
        then: Box<dyn Fn(ObjectSet) -> Operation>,
    },
    /// "For each opponent, you may put up to one target creature card from that
    /// player's graveyard onto the battlefield": each is offered in turn, and
    /// what was taken moves at once (Sepulchral Primordial).
    Offered {
        decider: PlayerRef,
        objects: ObjectSet,
        then: Box<dyn Fn(ObjectSet) -> Operation>,
        result: ResultId,
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
    /// "For each opponent, create a token that's attacking that player": what
    /// each attacks is known, or asked, before any token is made.
    TokensAt {
        player: PlayerRef,
        token: TokenSpec,
        definition: TokenDefinition,
        tapped: bool,
        defender: PlayerRef,
        or_planeswalker: bool,
        at: TokensAt,
        result: ResultId,
    },
    /// "For each creature token, its controller creates a copy of it."
    TokenCopies {
        player: PlayerRef,
        recipe: TokenRecipe,
        result: ResultId,
    },
    /// "For each opponent, create a tapped token that's a copy of it attacking
    /// that player": one creation, as myriad's, and each goes to its defender.
    TokenCopyAt {
        player: PlayerRef,
        object: ObjectRef,
        adjustments: Vec<Operand>,
        defender: PlayerRef,
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
    implicit_single: bool,
}
#[derive(Clone, Copy)]
enum ViewKind {
    Selected,
    Implicit,
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
    let EventSpec::Damage(damage) = event else {
        return None;
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
/// `STACK_ITEM` for a slot that takes only abilities, or only spells: an item of the other kind is never its target, so
/// no arm reads one (Repudiate, Stifle; Counterspell).
const ABILITY_ITEM: &str = "matches!(t, Target::Ability(serial) if e.kind != StackKind::Spell && e.serial == serial)";
const SPELL_ITEM: &str = "matches!(t, Target::Object(id) if e.kind == StackKind::Spell && e.object == id)";

/// Whether a stack item that passes `filter` can be a spell, and whether it can be an ability.
fn stack_kinds(filter: &ObjectFilter) -> (bool, bool) {
    match filter {
        ObjectFilter::OnStack(StackTrait::Spell) => (true, false),
        ObjectFilter::OnStack(StackTrait::Activated | StackTrait::Triggered | StackTrait::Loyalty)
        | ObjectFilter::AbilityOf(_) => (false, true),
        ObjectFilter::All(filters) => filters
            .iter()
            .map(stack_kinds)
            .fold((true, true), |(s, a), (fs, fa)| (s && fs, a && fa)),
        ObjectFilter::Either(filters) => filters
            .iter()
            .map(stack_kinds)
            .fold((false, false), |(s, a), (fs, fa)| (s || fs, a || fa)),
        ObjectFilter::Any
        | ObjectFilter::Not(_)
        | ObjectFilter::OnStack(StackTrait::SingleTarget)
        | ObjectFilter::ControlledBy(_)
        | ObjectFilter::TargetsAny(_) => (true, true),
        // A word about a card reads a spell (`stack_filter`).
        _ => (true, false),
    }
}

/// The test of whether stack item `e` is target `t`, for a slot of `domain` whose targets pass `filter`.
pub(super) fn stack_item(domain: &ObjectDomain, filter: &ObjectFilter) -> &'static str {
    let (spells, abilities) = stack_kinds(filter);
    match (spells && !matches!(domain.kind, ObjectKind::Ability), abilities) {
        (false, true) => ABILITY_ITEM,
        (true, false) => SPELL_ITEM,
        _ => STACK_ITEM,
    }
}

pub(super) fn on_the_stack(domain: &ObjectDomain) -> bool {
    domain.zones == [EventZone::Stack]
        && matches!(domain.kind, ObjectKind::Ability | ObjectKind::Any)
}

impl Compiler {
    /// The zone of objects a player's own, so entering "under their control" is
    /// under their owner's.
    fn own_cards(&self, objects: &ObjectSet, binding: &Binding, earlier: &[Instruction]) -> bool {
        if let ObjectSet::Prepared(id) = objects {
            return self
                .selection_metadata(id)
                .is_ok_and(|p| self.own_cards(&p.pool, binding, earlier));
        }
        if self.moved_their_own(objects, binding) {
            return true;
        }
        if self.turned_up.as_ref().is_some_and(|(walked, _)| *walked == binding.0)
            && format!("{objects:?}").contains("Earlier { what: It")
        {
            return true;
        }
        let theirs = |scope: &ZoneScope| {
            matches!(
                scope,
                ZoneScope::Players { players, zone }
                    if one_bound(players, binding)
                        && matches!(zone, EventZone::Hand | EventZone::Library | EventZone::Graveyard)
            )
        };
        match objects {
            // "All cards in their graveyard", said of the set of all graveyards.
            ObjectSet::Zone { scope, filter, .. } => {
                let parts = match filter {
                    ObjectFilter::All(parts) => parts.as_slice(),
                    one => std::slice::from_ref(one),
                };
                theirs(scope)
                    || parts.iter().any(|part| matches!(part, ObjectFilter::InZone(zone) if theirs(zone)))
            }
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
        self.notes.own.contains(&id) && names(binding, objects)
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
            if let Instruction::Optional { decider, body } = instruction {
                if let [Instruction::Effect {
                    result,
                    operation:
                        Operation::Move {
                            objects,
                            destination,
                            entry,
                            face_down,
                            terms,
                        },
                }] = body.instructions.as_slice()
                {
                    if !names(binding, decider)
                        && names(binding, objects)
                        && !names(binding, destination)
                        && !names(binding, entry)
                        && !names(binding, terms)
                    {
                        let (destination, entry, face_down, terms) =
                            (destination.clone(), entry.clone(), *face_down, terms.clone());
                        return Part::Gather(Gathered::Offered {
                            decider: decider.clone(),
                            objects: objects.clone(),
                            then: Box::new(move |objects| Operation::Move {
                                objects,
                                destination: destination.clone(),
                                entry: entry.clone(),
                                face_down,
                                terms: terms.clone(),
                            }),
                            result: result.clone(),
                        });
                    }
                }
            }
            // CR 701.55d: "the entire process ... one at a time in APNAP order".
            if matches!(instruction, Instruction::ChooseOne { .. })
                && self.line_text().is_some_and(|line| line.contains("villainous choice"))
            {
                return Part::Walk;
            }
            // "Each opponent exiles a card from their hand or a permanent they
            // control": each picks an action, then they all happen at once (CR
            // 101.4). Exiles of either kind would be one exile of all picked; a
            // discard beside a sacrifice is two kinds of event no one call makes.
            if let Instruction::ChooseOne { options, .. } = instruction {
                let exiles = |option: &Program| {
                    let steps: Vec<&Instruction> = option
                        .instructions
                        .iter()
                        .flat_map(|i| match i {
                            Instruction::Group(group) => group.instructions.iter().collect(),
                            other => vec![other],
                        })
                        .collect();
                    steps.iter().any(|i| {
                        matches!(i, Instruction::Effect { operation: Operation::Move { destination: Destination::Exile, .. }, .. })
                    }) && steps.iter().all(|i| match i {
                        Instruction::Effect { operation: Operation::Move { destination, .. }, .. } => {
                            matches!(destination, Destination::Exile)
                        }
                        Instruction::Effect { operation: Operation::ChooseObjects { .. }, .. } => true,
                        _ => false,
                    })
                };
                if !walks_whole(instruction) {
                    return Part::Unbatched(match options.iter().all(exiles) {
                        true => "per-player choice of what to exile, exiled together".into(),
                        false => "per-player choice among different actions, done at once".into(),
                    });
                }
            }
            return match walks_whole(instruction) {
                true => Part::Walk,
                false => Part::Unbatched("per-player instruction with several steps".into()),
            };
        };
        let result = result.clone();
        if let Operation::WithSelections { steps, action } = operation {
            let locals=steps.iter().map(|step|step.binding.0).collect::<BTreeSet<_>>();
            let dependent=match action.action() {
                Operation::Move {entry,destination,terms,..}=>selections::reads_local(entry,&locals) || selections::reads_local(destination,&locals) || selections::reads_local(terms,&locals),
                _=>false,
            };
            if dependent {return Part::Unbatched("per-player entry or destination with a local selection".into());}
            // A local preparation must run once for each player, at this step.
            // Classify the pure action independently; never move the preparation
            // outside the walk, even if the original pool mentions no bound player.
            let mut nested = flat.to_vec();
            nested[step] = Instruction::Effect {
                result: result.clone(),
                operation: (**action).clone(),
            };
            let depth = self.prepared_selections.len();
            let mut scope = self.clone();
            scope.prepared_selections.push(BTreeMap::new());
            for step in steps {
                scope.bind_prepared_metadata(step, String::new());
            }
            let classified = scope.part(each, &nested, step, earlier);
            let gathered = match classified {
                Part::Walk => return Part::Walk,
                Part::Unbatched(feature) => return Part::Unbatched(feature),
                Part::Gather(inner) => Some(inner),
                Part::Once(Instruction::Effect {
                    operation:
                        Operation::Move {
                            objects,
                            destination,
                            entry,
                            face_down,
                            terms,
                        },
                    ..
                }) => Some(Gathered::Objects {
                    objects,
                    then: Box::new(move |objects| Operation::Move {
                        objects,
                        destination: destination.clone(),
                        entry: entry.clone(),
                        face_down,
                        terms: terms.clone(),
                    }),
                }),
                _ => None,
            };
            return match gathered {
                Some(inner) => Part::Gather(Gathered::WithSelections {
                    steps: steps.clone(),
                    inner: Box::new(inner),
                    result: result.clone(),
                    depth,
                }),
                None => Part::Unbatched("per-player operation with local selections".into()),
            };
        }
        match operation {
            Operation::Move {
                objects,
                destination,
                entry,
                face_down,
                terms,
            } => {
                let mut destination = destination.clone();
                if let Destination::Battlefield(player) = &destination {
                    if is_bound(player, binding) {
                        // Thieves' Auction: picks round the table until every
                        // card is chosen, one that can't enter staying exiled.
                        if self.line_text().is_some_and(|line| {
                            line.contains("until all cards exiled this way have been chosen")
                        }) {
                            return Part::Unbatched(
                                "auction of the exiled cards until each is chosen".into(),
                            );
                        }
                        if !self.own_cards(objects, binding, &flat[..step]) {
                            return Part::Unbatched(
                                "per-player move under each player's control".into(),
                            );
                        }
                        destination = Destination::OwnersBattlefield;
                    }
                }
                if names(binding, &destination) || names(binding, entry) || names(binding, terms) {
                    return Part::Unbatched("per-player Move".into());
                }
                let (entry, face_down, moved, terms) = (entry.clone(), *face_down, result.clone(), terms.clone());
                let then = move |objects: ObjectSet| Operation::Move {
                    objects,
                    destination: destination.clone(),
                    entry: entry.clone(),
                    face_down,
                    terms: terms.clone(),
                };
                let among_turned = self.turned_up.as_ref().is_some_and(|(walked, _)| *walked == binding.0) && format!("{objects:?}").contains("Earlier { what: It");
                if names(binding, objects) || among_turned {
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
            // "For each opponent in turn order. Reveal ... That player may pay":
            // one process after another, each seeing the last (Protection Racket).
            Operation::PlayersDo {
                players: PlayerSet::One(PlayerRef::Bound(walked)),
                any_one: false,
                optional: false,
                once_each_turn: false,
                otherwise,
                ..
            } if walked.0 == binding.0 && otherwise.is_empty() => Part::Walk,
            // "Each player turns face up all cards they own exiled with this":
            // nothing sees a card turned face up in exile one at a time.
            Operation::KeywordEach {
                actor: PlayerSet::One(PlayerRef::Bound(actor)),
                action: KeywordAction::TurnFaceUp,
                until: None,
                ..
            } if actor.0 == binding.0 => Part::Walk,
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
            // "Each opponent sacrifices an artifact creature and a nonartifact
            // creature": each chooses in turn, then all of it goes together.
            Operation::Together(operations)
                if lane_l::sacrifice_slots(operations)
                    .is_some_and(|(player, _)| is_bound(player, binding)) =>
            {
                let (_, slots) = lane_l::sacrifice_slots(operations).expect("just seen");
                Part::Gather(Gathered::SacrificeEach { slots, result })
            }
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
            // "Each player chooses up to two creatures, then sacrifices the rest."
            Operation::SacrificeAllBut {
                players,
                objects,
                keep,
            } if one_bound(players, binding) && !names(binding, objects) && !names(binding, keep) => {
                Part::Gather(Gathered::SacrificeRest {
                    objects: objects.clone(),
                    keep: keep.clone(),
                    result,
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
            // "Each player puts a counter on ...": each chooses in turn, then they all
            // go on together (101.4), each put by its player (122.6).
            Operation::PutCounters { placements }
                if bound
                    && placements
                        .iter()
                        .all(|p| matches!(p.recipients, Recipients::Objects(_))) =>
            {
                Part::Gather(Gathered::Counters { operation: operation.clone(), result })
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
                [TokenSpec {
                    amount: Expr::Constant(1),
                    recipe:
                        recipe @ (TokenRecipe::Copy { .. }
                        | TokenRecipe::CopyEach { .. }
                        | TokenRecipe::CopyOne { .. }),
                    ..
                }] if !entry.tapped => Part::Gather(Gathered::TokenCopies {
                    player: player.clone(),
                    recipe: recipe.clone(),
                    result,
                }),
                _ => Part::Unbatched(format!("per-player {}", form(operation))),
            },
            Operation::CreateTokens {
                players: PlayerSet::One(player),
                tokens,
                entry,
            } if entry.counters.is_empty() && !is_bound(player, binding) => {
                // Two creations no gather writes, for want of an engine call that
                // makes them as one batch: tapped copies of several originals
                // (`create_token_copies_together` makes none tapped), and tokens
                // entering attached (no token is created attached).
                let several_tapped = entry.tapped
                    && matches!(tokens.as_slice(), [TokenSpec { recipe: TokenRecipe::CopyEach { .. }, .. }]);
                let attached = entry.states.iter().any(|s| {
                    matches!(s, EntryState::AttachedTo(_) | EntryState::AttachedToChosen(_) | EntryState::AttachedToPlayer(_))
                });
                let unbatched = || match () {
                    _ if several_tapped => {
                        Part::Unbatched("per-player tapped token copies of several objects".into())
                    }
                    _ if attached => Part::Unbatched("per-player tokens entering attached".into()),
                    _ => Part::Unbatched(format!("per-player {}", form(operation))),
                };
                let [token] = tokens.as_slice() else {
                    return unbatched();
                };
                let (defender, or_planeswalker, at) = match entry.states.as_slice() {
                    [EntryState::Attacking {
                        player: Some(defender),
                        or_planeswalker,
                    }] => (defender.clone(), *or_planeswalker, TokensAt::Attacking),
                    [EntryState::RulesUntil {
                        rules,
                        until: Duration::EndOfTurn,
                    }] => match rules.as_slice() {
                        [Operand::Term { form, operands, .. }] if form == "MustAttackPlayer" => {
                            match operands.as_slice() {
                                [Operand::Player(defender)] => {
                                    (defender.clone(), false, TokensAt::MustAttack)
                                }
                                _ => return unbatched(),
                            }
                        }
                        _ => return unbatched(),
                    },
                    _ => return unbatched(),
                };
                if !is_bound(&defender, binding) {
                    return unbatched();
                }
                match &token.recipe {
                    TokenRecipe::Defined(definition) => Part::Gather(Gathered::TokensAt {
                        player: player.clone(),
                        token: token.clone(),
                        definition: definition.clone(),
                        tapped: entry.tapped,
                        defender,
                        or_planeswalker,
                        at,
                        result,
                    }),
                    TokenRecipe::Copy {
                        object,
                        adjustments,
                    } if !or_planeswalker
                        && at == TokensAt::Attacking
                        && matches!(token.amount, Expr::Constant(1)) =>
                    {
                        Part::Gather(Gathered::TokenCopyAt {
                            player: player.clone(),
                            object: object.clone(),
                            adjustments: adjustments.clone(),
                            defender,
                            tapped: entry.tapped,
                            result,
                        })
                    }
                    _ => unbatched(),
                }
            }
            // "Each player separates ... into piles" (CR 700.3): each in turn, and
            // the piles are kept for the steps that choose among them and read them.
            Operation::Piles {
                actor: PlayerSet::One(player),
                count: Some(Expr::Constant(n)),
                names,
                chooser,
                ..
            } if is_bound(player, binding)
                && *n >= 2
                && names.is_empty()
                && (chooser.is_none() || *n == 2) =>
            {
                Part::Walk
            }
            Operation::PlayersPayRepeatedly {
                players: PlayerSet::One(player),
                cost,
                ..
            } if is_bound(player, binding)
                && matches!(cost.parts.as_slice(), [CostPart::Mana(_)]) =>
            {
                Part::Walk
            }
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
        // Opponents come in seat order; the walk goes in turn order (CR 101.4).
        if players.contains(".opponents(") {
            writeln!(out, "{{ let order: Vec<PlayerId> = g.state().apnap().collect(); players_{id}.sort_by_key(|p| order.iter().position(|o| o == p)); }}").unwrap();
        }
        let walking = (self.notes.walking).replace((each.binding.0, format!("players_{id}")));
        for step in &each.body.instructions {
            if let Instruction::Effect {
                operation:
                    Operation::KeywordEach {
                        actor: PlayerSet::One(PlayerRef::Bound(actor)),
                        action: KeywordAction::TurnFaceUp,
                        objects,
                        until: None,
                    },
                ..
            } = step
            {
                if actor.0 == each.binding.0 {
                    self.turned_up = Some((actor.0, objects.clone()));
                }
            }
        }
        self.note_own(&each.body);
        if let Some(first) = each.starting_with {
            let first = self.fetch(self.player(first)?);
            writeln!(out, "let first_{id} = {first}; if let Some(at) = players_{id}.iter().position(|&p| Some(p) == first_{id}) {{ players_{id}.rotate_left(at); }}").unwrap();
        }
        if let (true, Ok(direction)) = (
            each.in_chosen_direction,
            self.chosen_local(ValueKind::Direction),
        ) {
            // "Starting with you and proceeding in the chosen direction": turn
            // order to the left, its reverse after the first to the right.
            writeln!(out, "if {direction} == 1 && players_{id}.len() > 1 {{ players_{id}[1..].reverse(); }}").unwrap();
        } else if each.in_chosen_direction {
            self.plan_code(
                "per-player in a chosen direction",
                &format!("players_{id} = cantrip_mechanics::players::in_chosen_direction(g, players_{id}, asker)?;"),
                out,
            );
        }
        // "For each opponent, you mill a card, then return that card unless that
        // player pays 3 life" (Sivriss): one opponent's whole process, then the
        // next's, and nothing after reads what it did.
        let mut made = BTreeSet::new();
        for step in &each.body.instructions {
            bound_results(step, &mut made);
        }
        let read_after = format!("{:?}", each.rest);
        if punished_in_turn(&each.body.instructions, each.binding)
            && !made.iter().any(|r| read_after.contains(&format!("ResultId({r})")))
        {
            let b = each.binding.0;
            self.bound_players.insert(b);
            writeln!(out, "for bound_{b} in players_{id}.iter().copied() {{").unwrap();
            let walked = self.block(each.body, out, true);
            writeln!(out, "}}").unwrap();
            self.bound_players.remove(&b);
            self.notes.walking = walking;
            return walked;
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
        // "Each opponent who copied the spell this way": who said yes.
        for step in &steps {
            let (Some(guard), Instruction::Effect { result, operation: Operation::CopyOnStack { .. } }) =
                (step.guard, &program.instructions[step.at])
            else {
                continue;
            };
            if matches!(guards[guard], Guard::May | Guard::MayDraw(_)) {
                let took = (format!("players_{id}"), format!("guard_{id}_{guard}[at]"));
                self.notes.took.entry(result.0).or_insert(took);
            }
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
                    let piles = match &program.instructions[at] {
                        Instruction::Effect {
                            result,
                            operation: Operation::Piles { chooser, .. },
                        } => Some((format!("piles_{}", result.0), chooser.is_some())),
                        _ => None,
                    };
                    // "Each player gains control of all permanents controlled by the
                    // next player": which permanents is read for every player before
                    // any of them changes control, all of it one instruction.
                    let mut snapshot = None;
                    if let Instruction::Effect {
                        result,
                        operation:
                            Operation::CreateContinuousEffect {
                                source,
                                affected: EffectSubjects::Objects(objects),
                                changes,
                                duration,
                                evaluation,
                            },
                    } = &program.instructions[at]
                    {
                        if names(&each.binding, objects)
                            && changes.iter().any(|c| matches!(c, Change::SetController(_)))
                        {
                            let k = self.fresh();
                            let read = self.objects(objects).map(|o| self.fetch(o));
                            if let Ok(read) = read {
                                let b = each.binding.0;
                                writeln!(out, "let snap_{k}: Vec<Vec<ObjectId>> = players_{id}.iter().copied().map(|bound_{b}| {read}).collect();").unwrap();
                                let fake = u32::MAX - k as u32;
                                self.selections.insert(fake, format!("snap_{k}[index_{id}]"));
                                snapshot = Some(fake);
                                program.instructions[at] = Instruction::Effect {
                                    result: result.clone(),
                                    operation: Operation::CreateContinuousEffect {
                                        source: source.clone(),
                                        affected: EffectSubjects::Objects(ObjectSet::Selected(ResultId(fake))),
                                        changes: changes.clone(),
                                        duration: duration.clone(),
                                        evaluation: evaluation.clone(),
                                    },
                                };
                            }
                        }
                    }
                    if let Instruction::Effect {
                        result,
                        operation: Operation::PlayersPayRepeatedly { cost, .. },
                    } = &program.instructions[at]
                    {
                        if matches!(cost.parts.as_slice(), [CostPart::Mana(_)]) {
                            writeln!(out, "let mut unpaid_{}: Vec<PlayerId> = Vec::new();", result.0).unwrap();
                            self.unpaid_rows.insert(result.0);
                        }
                    }
                    if let Some((rows, _)) = &piles {
                        writeln!(out, "let mut {rows}: Vec<(PlayerId, Vec<Vec<ObjectId>>, Option<usize>, Vec<PlayerId>)> = Vec::new();").unwrap();
                        self.piles_walk = Some(rows.clone());
                    }
                    self.walk(
                        id, &each, &program, &step, &guards, &mut asked, &mut views, None, out,
                    );
                    if let Some(fake) = snapshot {
                        self.selections.remove(&fake);
                    }
                    if let Some((rows, chooser)) = piles {
                        self.piles_walk = None;
                        self.piles = Some(rows.clone());
                        // "One of their piles is chosen by one of their opponents of
                        // their choice": once every player has separated.
                        if chooser {
                            const VERB: &str = "chooses one of your piles";
                            writeln!(out, "for row in {rows}.iter_mut() {{ if let Some(by) = cantrip_mechanics::choose::player(g, row.0, asker.source, row.3.clone(), {VERB:?}, 0)? {{ let (chosen, _) = cantrip_mechanics::choose::pile(g, by, asker.source, row.1[0].clone(), row.1[1].clone())?; row.2 = Some(usize::from(chosen != row.1[0])); }} }}").unwrap();
                        }
                    }
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
                    gathered.prepare_choice_views(gather, &format!("players_{id}"), out);
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
                    if let Gathered::Sacrifice { result, .. } | Gathered::SacrificeEach { result, .. } = &gathered {
                        let test = format!("gathered_{gather}.iter().any(|(who, _)| *who == players_{id}[at])");
                        self.notes.took.insert(result.0, (format!("players_{id}"), test));
                    }
                    // "For each opponent who does": who took the offer, whatever it came to.
                    if let (Gathered::Counters { result, .. }, Some(guard)) = (&gathered, step.guard) {
                        let test = format!("guard_{id}_{guard}[at]");
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
                        false => match faces::may_draws(&body.instructions, decider) {
                            Some(n) => Guard::MayDraw(n),
                            None => Guard::May,
                        },
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
                // "For each flip you win, this. For each flip you lose, that": the
                // flips stay as they fell, so the two are steps of their own.
                Instruction::If {
                    condition: condition @ Condition::CoinFlip { .. },
                    then,
                    otherwise,
                } if guard.is_none() => {
                    guards.push(Guard::If(condition.clone()));
                    let inner = guards.len() - 1;
                    self.flatten(each, &then.instructions, Some(inner), flat, steps, guards);
                    guards.push(Guard::If(Condition::Not(Box::new(condition.clone()))));
                    let inner = guards.len() - 1;
                    self.flatten(each, &otherwise.instructions, Some(inner), flat, steps, guards);
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
            self.view_set(view, Some(view.each.clone()),true);
        }
    }
    fn view_all(&mut self, views: &[View]) {
        for view in views {
            self.view_set(view, view.all.clone(),false);
        }
    }
    fn view_set(&mut self, view: &View, local: Option<String>, each:bool) {
        if matches!(view.kind,ViewKind::Implicit) {
            if each && view.implicit_single {self.notes.implicit_single.insert(view.result);} else {self.notes.implicit_single.remove(&view.result);}
        }
        let map = match view.kind {
            ViewKind::Selected => &mut self.selections,
            ViewKind::Implicit => &mut self.implicit_selections,
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
            Guard::MayDraw(n) => format!("!{} && cantrip_mechanics::effects::ask_may(g, {player}, asker.source, Purpose::Optional)?", planned::cant_draw(&player, n)),
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
        let scoped: [(ViewKind, &BTreeMap<u32, String>, &BTreeMap<u32, String>); 9] = [
            (ViewKind::Selected, &self.selections, &before.selections),
            (ViewKind::Implicit,&self.implicit_selections,&before.implicit_selections),
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
                    | ViewKind::Implicit
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
                if matches!(kind,ViewKind::Implicit) {
                    self.notes.implicit_each.insert(result,(format!("players_{id}"),format!("each_{k}")));
                }
                if matches!(kind, ViewKind::Selected | ViewKind::Moved | ViewKind::Created) {
                    let lists = (format!("players_{id}"), format!("each_{k}"));
                    self.notes.each.entry(result).or_insert(lists);
                }
            } else {
                writeln!(
                    declared,
                    "let mut each_{k} = vec![Default::default(); players_{id}.len()];"
                )
                .unwrap();
                writeln!(tail, "each_{k}[index_{id}] = {local};").unwrap();
                if matches!(kind, ViewKind::Performed) {
                    let lists = (format!("players_{id}"), format!("each_{k}"));
                    self.notes.each_done.insert(result, lists);
                }
            }
            // How much of something: all of them together is its sum. A reader
            // one player at a time may mean their own or the total; the AST
            // does not say which, so nothing is read for it.
            let by_player = lane_n::read_player_by_player(each.rest, &result)
                && !self.line_text().is_some_and(|line| line.contains("the total amount"));
            let total = (matches!(kind, ViewKind::Performed)
                && local.starts_with("done_")
                && !by_player)
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
                if by_player {
                    self.notes.read_as_own.insert(result);
                }
            }
            let view = View {
                implicit_single: matches!(kind,ViewKind::Implicit) && self.notes.implicit_single.contains(&result),
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
            if matches!(kind, ViewKind::Flip) {
                self.coin_turns = Some((format!("players_{id}"), format!("each_{k}")));
                self.coins = Some((
                    format!("(each_{k}.iter().filter(|&&flip| flip).count() as i64)"),
                    format!("(each_{k}.iter().filter(|&&flip| !flip).count() as i64)"),
                ));
            }
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
        // A value chosen in a turn is that turn's, and a local of it; what "chosen
        // this way" reads after them all is any of them.
        let each_chose: Vec<(ValueKind, String)> = self
            .chosen
            .iter()
            .filter(|(kind, local)| {
                !before.chosen.iter().any(|(k, _)| k == kind) && local.starts_with("chosen_")
            })
            .cloned()
            .collect();
        for (kind, local) in each_chose {
            let list = format!("each_{local}");
            let (declare, push) = (
                format!("let mut {list} = Vec::new();\n"),
                format!("{list}.push({local});\n"),
            );
            declared.push_str(&declare);
            tail.push_str(&push);
            self.unread.push((list.clone(), vec![declare, push]));
            // Whose each one is, where a later turn reads a player's own.
            let who = format!("{list}_who");
            let (declare, push) = (
                format!("let mut {who}: Vec<PlayerId> = Vec::new();\n"),
                format!("{who}.push(bound_{});\n", each.binding.0),
            );
            declared.push_str(&declare);
            tail.push_str(&push);
            self.unread.push((who, vec![declare, push]));
            self.chosen_lists.push((kind, list));
        }
        self.chosen = before.chosen;
        // What the walk's turns have picked, for a choice of "a different" one.
        let unpicked = format!("unpicked_{id}");
        if body.contains(&unpicked) && !out.contains(&format!("let mut {unpicked}:")) {
            writeln!(declared, "let mut {unpicked}: Vec<ObjectId> = Vec::new();").unwrap();
        }
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
        // "And you draw a card" for each of them reads none of them.
        let walked = format!("bound_{binding}");
        let walked = match body.contains(&walked) || tail.contains(&walked) || closing.contains(&walked) {
            true => walked,
            false => "_".to_string(),
        };
        writeln!(out, "{open} for (index_{id}, {walked}) in players_{id}.iter().copied().enumerate() {{ {body} {tail} {shut} {closing} }} {close}").unwrap();
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
            Gathered::WithSelections {
                steps,
                inner,
                depth,
                ..
            } => {
                self.prepared_selections.push(BTreeMap::new());
                let done = self
                    .prepare_selections(steps, inner.purpose(), out)
                    .and_then(|()| {
                        if let Some(last) = steps
                            .iter()
                            .rev()
                            .find(|s| matches!(s.chooser, SelectionChooser::Players(_)))
                        {
                            let picked = &self.prepared_selection(&last.binding)?.local;
                            let players=&self.notes.walking.as_ref().expect("player collection scope").1;
                            writeln!(
                                out,
                                "implicit_{local}_{depth}.extend({picked}.iter().copied()); if let Some(at) = {players}.iter().position(|&p| p == {bound}) {{ implicit_each_{local}_{depth}[at] = {picked}.clone(); }}"
                            )
                            .unwrap();
                        }
                        self.gather(local, inner, each, out)
                    });
                self.prepared_selections.pop();
                done?;
            }
            Gathered::Objects { objects, .. } => {
                writeln!(
                    out,
                    "gathered_{local}.extend({});",
                    self.fetch(self.objects(objects)?)
                )
                .unwrap();
            }
            Gathered::Offered { decider, objects, .. } => {
                let decider = self.fetch(self.certain_player(decider)?);
                let objects = self.fetch(self.objects(objects)?);
                writeln!(out, "let offered_{id}: Vec<ObjectId> = {objects}; if !offered_{id}.is_empty() {{ let decider_{id} = {decider}; if cantrip_mechanics::effects::ask_may(g, decider_{id}, asker.source, Purpose::Optional)? {{ gathered_{local}.extend(offered_{id}); }} }}").unwrap();
            }
            Gathered::Cards { amount, .. }
                if whole_hand(amount, &PlayerSet::One(PlayerRef::Bound(each.binding.clone()))) =>
            {
                writeln!(out, "gathered_{local}.extend(g.state().zone(Zone::hand({bound})).to_vec());").unwrap();
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
            Gathered::Counters { operation, result } => {
                // "Puts counters on it": the token made for all of them is each one's own.
                let own = matches!(operation, Operation::PutCounters { placements }
                    if placements.iter().all(|p| !names(&each.binding, &p.recipients)
                        && format!("{:?}", p.recipients).contains("Named(")));
                self.notes.gathering_counters = Some((local, bound.clone(), own));
                let done = self.operation(operation, result, out);
                self.notes.gathering_counters = None;
                done?;
            }
            Gathered::SacrificeRest { objects, keep, .. } => {
                let keep = self.number(keep)?.amount(u32::MAX.into(), "u32")?;
                let objects = self.fetch(self.objects(objects)?);
                let keep = self.fetch(keep);
                const M: &str = "cantrip_mechanics";
                writeln!(out, "let own_{id}: Vec<ObjectId> = {objects}.into_iter().filter(|&o| g.controller(o) == {bound}).collect(); let keep_{id} = {keep}; let kept_{id} = {M}::choose::select(g, {bound}, asker.source, Purpose::Keep, own_{id}.clone(), {M}::select::Selection::of({M}::select::up_to(keep_{id}), &[]))?; gathered_{local}.extend(own_{id}.iter().filter(|o| !kept_{id}.contains(o)).map(|&o| ({bound}, o)));").unwrap();
            }
            Gathered::SacrificeEach { slots, .. } => {
                self.sacrifice_slots(id, &bound, slots, out)?;
                writeln!(out, "gathered_{local}.extend(chosen_{id}.iter().map(|&o| ({bound}, o)));").unwrap();
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
                let amount = self.number(amount)?.quantity(Quantity::Prevention)?;
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
            Gathered::TokensAt {
                player,
                token,
                definition,
                defender,
                or_planeswalker,
                ..
            } => {
                let amount = self.number(&token.amount)?.amount(u32::MAX.into(), "u32")?;
                let definition = self.token_definition(definition)?;
                let defender = self.fetch(self.certain_player(defender)?);
                writeln!(out, "let def_{id} = g.card_id(&{definition}); let count_{id} = {}; let player_{id} = {}; let defender_{id} = {defender};", self.fetch(amount), self.fetch(self.certain_player(player)?)).unwrap();
                match or_planeswalker {
                    true => writeln!(out, "let candidates_{id} = cantrip_mechanics::combat::player_or_their_planeswalkers(g, defender_{id}); for _ in 0..count_{id} {{ if let Some(target) = cantrip_mechanics::combat::choose_among(g, player_{id}, asker.source, &candidates_{id})? {{ gathered_{local}.push((def_{id}, player_{id}, target)); }} }}").unwrap(),
                    false => writeln!(out, "gathered_{local}.extend(std::iter::repeat((def_{id}, player_{id}, Target::Player(defender_{id}))).take(count_{id} as usize));").unwrap(),
                }
            }
            Gathered::TokenCopies { player, recipe, .. } => {
                let player = self.fetch(self.certain_player(player)?);
                let (originals, adjustments) = match recipe {
                    TokenRecipe::Copy {
                        object,
                        adjustments,
                    } => (
                        format!(
                            "{}.into_iter().collect::<Vec<ObjectId>>()",
                            self.fetch(self.object(object)?)
                        ),
                        adjustments,
                    ),
                    TokenRecipe::CopyEach {
                        objects,
                        adjustments,
                    } => (self.fetch(self.objects(objects)?), adjustments),
                    // "A token that's a copy of a creature they control": each chooses
                    // one in turn, and the tokens are all made together.
                    TokenRecipe::CopyOne {
                        objects,
                        adjustments,
                    } => (
                        format!(
                            "{{ let among = {}; cantrip_mechanics::choose::select(g, player_{id}, asker.source, Purpose::CopyWhich, among, cantrip_mechanics::select::cards(cantrip_mechanics::select::exactly(1)))? }}",
                            self.fetch(self.objects(objects)?)
                        ),
                        adjustments,
                    ),
                    _ => return Err(gap("per-player CreateTokens")),
                };
                let values = self.copy_values("Some(original)", adjustments)?;
                writeln!(out, "let player_{id} = {player}; let originals_{id} = {originals}; for original in originals_{id} {{ if let Some(values) = {values} {{ gathered_{local}.push((values, player_{id})); }} }}").unwrap();
            }
            Gathered::TokenCopyAt { defender, .. } => {
                let defender = self.fetch(self.certain_player(defender)?);
                writeln!(out, "gathered_{local}.push(Target::Player({defender}));").unwrap();
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
            Gathered::WithSelections {
                steps,
                inner,
                result,
                depth,
            } => {
                let inner_choice = inner.has_choice();
                self.after_gathering(local, *inner, program, at, out)?;
                if !inner_choice
                    && steps
                        .iter()
                        .any(|s| matches!(s.chooser, SelectionChooser::Players(_)))
                {
                    self.implicit_selections
                        .insert(result.0, format!("implicit_{local}_{depth}"));
                    self.notes.implicit_single.remove(&result.0);
                    let players=self.notes.walking.as_ref().expect("player collection scope").1.clone();
                    self.notes.implicit_each.insert(result.0,(players,format!("implicit_each_{local}_{depth}")));
                    self.notes.earlier.push(("Selected", result));
                }
                return Ok(());
            }
            Gathered::Objects { then, .. } => {
                let Instruction::Effect { result, .. } = program.instructions[at].clone() else {
                    return Err(gap("gathered step that is no operation"));
                };
                self.act_on_gathered(local, then.as_ref(), result, program, at, out);
            }
            Gathered::Offered { then, result, .. } => {
                self.act_on_gathered(local, then.as_ref(), result, program, at, out);
            }
            Gathered::Cards { result, .. } => {
                let discard = |objects| Operation::DiscardObjects { objects };
                self.act_on_gathered(local, &discard, result, program, at, out);
            }
            Gathered::Counters { result, .. } => {
                writeln!(out, "let had_{local}: Vec<_> = gathered_{local}.iter().map(|c| g.state().obj(c.1).counters(c.2)).collect(); g.add_counters_together_by(&gathered_{local}); let applied_{local}: Vec<ObjectId> = gathered_{local}.iter().zip(had_{local}).filter(|(c, had)| g.state().obj(c.1).counters(c.2) > *had).map(|(c, _)| c.1).collect();").unwrap();
                self.selections.insert(result.0, format!("applied_{local}"));
            }
            Gathered::Sacrifice { result, .. }
            | Gathered::SacrificeEach { result, .. }
            | Gathered::SacrificeRest { result, .. } => {
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
                    writeln!(out, "let done_{local}: i64 = g.damage_together(&gathered_{local}).iter().map(|&n| i64::from(n)).{damage_sum};", damage_sum = if planned::switched_on(quantity::FEATURE) { Quantity::Damage.sum_i64() } else { "sum()" }).unwrap();
                    self.report_quantity(result.0, format!("done_{local}"), Quantity::Damage);
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
            Gathered::TokensAt {
                token,
                tapped,
                at,
                result,
                ..
            } => {
                let words = match token.words {
                    TokenWords::Rules => "Words::NONE",
                    TokenWords::CreatingEffect => "g.words(asker.source)",
                };
                writeln!(out, "let words_{local} = {words}; let mut created_{local}: Vec<ObjectId> = Vec::new();").unwrap();
                match at {
                    // The tokens that attack the same thing enter together, as
                    // `combat::create_attacking` has them.
                    TokensAt::Attacking => writeln!(out, "let mut done_{local}: Vec<Target> = Vec::new(); for &(_, _, target) in &gathered_{local} {{ if done_{local}.contains(&target) {{ continue; }} done_{local}.push(target); let group: Vec<(CardId, PlayerId)> = gathered_{local}.iter().filter(|row| row.2 == target).map(|row| (row.0, row.1)).collect(); created_{local}.extend(g.create_tokens_worded(&group, {tapped}, Some(target), words_{local})?); }}").unwrap(),
                    // Each player's share of what was made, however many that is
                    // (CR 614.1a), as myriad shares its tokens out.
                    TokensAt::MustAttack => writeln!(out, "let rows_{local}: Vec<(CardId, PlayerId)> = gathered_{local}.iter().map(|row| (row.0, row.1)).collect(); created_{local}.extend(g.create_tokens_worded(&rows_{local}, {tapped}, None, words_{local})?); if !gathered_{local}.is_empty() {{ let per_{local} = created_{local}.len().div_ceil(gathered_{local}.len()); g.one_instruction(&mut |g| {{ for (at, &token) in created_{local}.iter().enumerate() {{ let target = gathered_{local}[(at / per_{local}).min(gathered_{local}.len() - 1)].2; g.add_effect(asker.source, token, EffectKind::MustAttackAt(target), Duration::EndOfTurn); }} Ok(()) }})?; }}").unwrap(),
                }
                self.creations.insert(result.0, format!("created_{local}"));
            }
            Gathered::TokenCopies { result, .. } => {
                writeln!(out, "let created_{local} = g.create_token_copies_together(&gathered_{local})?;").unwrap();
                self.creations.insert(result.0, format!("created_{local}"));
            }
            Gathered::TokenCopyAt {
                player,
                object,
                adjustments,
                tapped,
                result,
                ..
            } => {
                let values = self.copy_values(&format!("original_{local}"), &adjustments)?;
                writeln!(out, "let original_{local} = {}; let values_{local} = {values}; let player_{local} = {}; let created_{local} = match values_{local} {{ Some(values) if !gathered_{local}.is_empty() => g.create_token_copies_tapped(values, player_{local}, gathered_{local}.len() as u32, {tapped})?, _ => Vec::new() }}; if !gathered_{local}.is_empty() {{ let (share_{local}, extra_{local}) = (created_{local}.len() / gathered_{local}.len(), created_{local}.len() % gathered_{local}.len()); let mut from_{local} = 0; for (at, &target) in gathered_{local}.iter().enumerate() {{ let upto = from_{local} + share_{local} + usize::from(at < extra_{local}); g.enter_attacking(&created_{local}[from_{local}..upto], target); from_{local} = upto; }} }}", self.fetch(self.object(&object)?), self.fetch(self.certain_player(&player)?)).unwrap();
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
        // Winter's Chill: "its controller may pay {1} or {2}. If that player
        // doesn't, ...": each payment is a may of its own (CR 118.12), asked in
        // turn, and the last option is what happens when none is paid.
        if let Some((last, paying)) = options.split_last() {
            let pays = |option: &Program| match option.instructions.first() {
                Some(Instruction::Effect {
                    operation: operation @ Operation::PayCost { optional: false, .. },
                    result,
                }) => Some((operation.clone(), result.clone())),
                _ => None,
            };
            if paying.len() >= 2
                && paying.iter().all(|option| pays(option).is_some())
                && pays(last).is_none()
            {
                let mut close = String::new();
                for option in paying {
                    let (operation, result) = pays(option).expect("checked");
                    let Operation::PayCost { player, cost, .. } = operation else {
                        unreachable!("a payment");
                    };
                    let asked = Operation::PayCost { player, cost, optional: true };
                    self.operation(&asked, &result, out)?;
                    let paid = self.performed[&result.0].clone();
                    writeln!(out, "if {paid} {{").unwrap();
                    let rest = Program { instructions: option.instructions[1..].to_vec() };
                    self.block(&rest, out, true)?;
                    writeln!(out, "}} else {{").unwrap();
                    close.push('}');
                }
                self.block(last, out, true)?;
                writeln!(out, "{close}").unwrap();
                return Ok(());
            }
        }
        let Some(base) = self.option_labels_of(options) else {
            // Cloak and Dagger: "until Cloak and Dagger leave the battlefield" is
            // gone from the data's exile, and its two exiles are one choice there.
            if self.lines_said().iter().any(|line| line.contains(" until ") && line.contains(" leave")) {
                return Err(gap("an exile until this leaves the battlefield dropped from a choice, in the data"));
            }
            let labels: Vec<String> = (0..options.len())
                .map(|i| format!("Label::Card(card_{id}, {i})"))
                .collect();
            let mut arms = String::new();
            for (i, option) in options.iter().enumerate() {
                match i + 1 == options.len() {
                    true => writeln!(arms, "_ => {{").unwrap(),
                    false => writeln!(arms, "{i} => {{").unwrap(),
                }
                self.block(option, &mut arms, true)?;
                writeln!(arms, "}}").unwrap();
            }
            // "You may instead ..." in what a replacement does (Flitwing, Jinnie
            // Fay, Mirrormind Crown, Retriever Phoenix): Cantrip asks it as the
            // effect applies (`ReplacementDef::optional`, CR 614.1a), not as a
            // choice in its steps, so the choice is written with the change it
            // offers, whose own hole already says what the file waits on.
            let feature = match self.pending == Pending::Reshaped && arms.contains("planned!(") {
                true => FOLLOWS_GAP,
                // Winter's Chill: "may pay {1} or {2}" is a choice whose options
                // each ask again whether to pay, so a player who picks one and
                // declines has neither paid nor taken the option that says so.
                false if arms.matches("g.may_pay(").count() + 1 >= options.len() && options.len() > 2 => {
                    return Err(gap("a choice among costs to pay, each one asked again as a may"));
                }
                false => "instruction ChooseOne",
            };
            let card = match self.stack_item {
                true => "item.ability.card",
                false => "g.card_id(&CARD)",
            };
            let pick = planned::planned_value(
                feature,
                "0u8",
                &format!(
                    "{{ let card_{id} = {card}; cantrip_mechanics::choose::one_of(g, decider_{id}, asker.source, vec![{}])? }}",
                    labels.join(", ")
                ),
            );
            writeln!(out, "match {pick} {{\n{arms}}}").unwrap();
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
        let mut labels =
            option_words::printed_options(&self.lines_said(), printed).or_else(|| {
                (declines == 0)
                    .then(|| lane_i_option_labels(options))
                    .flatten()
            })?;
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
    pub(super) fn stack_target_test(&self, domain: &ObjectDomain, filter: &ObjectFilter) -> Result<String> {
        Ok(format!(
            "q.state().stack.iter().any(|e| {} && ({}))",
            stack_item(domain, filter),
            self.stack_filter(filter)?
        ))
    }
    /// `slot_item` for the slot `objects` names.
    pub(super) fn objects_item(&self, objects: &ObjectSet) -> &'static str {
        match objects {
            ObjectSet::One(ObjectRef::Target { slot, .. }) | ObjectSet::Targets(slot) => {
                self.slot_item(slot)
            }
            _ => STACK_ITEM,
        }
    }
    /// `stack_item` for the slot `slot`, or `STACK_ITEM` when it is not known.
    pub(super) fn slot_item(&self, slot: &Slot) -> &'static str {
        match self.slots.get(&slot.0) {
            Some((_, spec)) => match &spec.kind {
                TargetKind::Object(domain) => stack_item(domain, &spec.filter),
                _ => STACK_ITEM,
            },
            None => STACK_ITEM,
        }
    }
    /// Whether stack item `e` passes `filter`. A word about a card reads a
    /// spell, and "from a [type] source" reads what an ability came from.
    pub(super) fn stack_filter(&self, filter: &ObjectFilter) -> Result<String> {
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
        let item = self.slot_item(slot);
        writeln!(out, "let {local}: Vec<Option<PlayerId>> = t_{at}_raw.iter().map(|&t: &Target| g.state().stack.iter().find(|e| {item}).map(|e| e.controller)).collect();").unwrap();
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
        let item = self.slot_item(slot);
        Ok(format!("t_{at}_raw.get({index}).copied().and_then(|t: Target| q.state().stack.iter().find(|e| {item}).map(|e| e.controller))"))
    }
    /// "Counter target activated or triggered ability" (CR 701.6): with spells
    /// among them, one instruction counters them all.
    pub(super) fn counter_on_stack(&self, id: usize, objects: &ObjectSet, out: &mut String) {
        let targets = self.stack_targets(objects).expect("a slot of the stack");
        let item = self.objects_item(objects);
        writeln!(out, "let items_{id}: Vec<(ObjectId, usize)> = {targets}.into_iter().filter_map(|t| {{ let stack = &g.state().stack; stack.iter().position(|e| {item}).map(|at| (stack[at].object, at)) }}).collect(); g.counter_items(&items_{id});").unwrap();
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
    /// "Change the target to another creature": the chooser says which, and one the
    /// spell can't take leaves it as it was (115.7b), so the next is asked for. The
    /// spell's target changes if any of them can have it.
    fn retarget_to(
        &mut self,
        id: usize,
        player: &PlayerRef,
        object: &ObjectRef,
        to: &ObjectSet,
        out: &mut String,
    ) -> Result<()> {
        let chooser = self.fetch(self.certain_player(player)?);
        let spell = self.fetch(self.object(object)?);
        let to = self.fetch(self.objects(to)?);
        writeln!(out, "let chooser_{id} = {chooser}; let spell_{id}: Option<ObjectId> = {spell}; let mut open_{id}: Vec<ObjectId> = {to}; if let Some(spell) = spell_{id} {{ while !open_{id}.is_empty() {{ let to = match open_{id}.as_slice() {{ [one] => Some(*one), _ => cantrip_mechanics::choose::card(g, chooser_{id}, asker.source, Purpose::Choose, open_{id}.clone())? }}; let Some(to) = to else {{ break; }}; open_{id}.retain(|&o| o != to); g.change_a_target_to(Target::Object(spell), to, chooser_{id})?; let changed = {{ let q: &dyn Query = g; q.state().stack.iter().find(|i| i.kind == StackKind::Spell && i.object == spell).is_some_and(|i| q.state().targets(i).contains(&Target::Object(to))) }}; if changed {{ break; }} }} }}").unwrap();
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
        let stored = Quantity::StoredCounter.kind();
        let most_read = if planned::switched_on(quantity::BOUNDED_32) { "max(0)" } else { "clamp(0, 255)" };
        let moving = format!("done_{id} += i64::from(g.move_counters(from, to, kind, amount));");
        let step = match (amount, kind) {
            (MovedCounters::One, Some(_)) => format!("let amount = 1{stored}; {moving}"),
            (MovedCounters::Exactly(n), Some(_)) => {
                let n = self.number(n)?.quantity(Quantity::StoredCounter)?;
                writeln!(out, "let amount_{id}: {stored} = {};", self.fetch(n)).unwrap();
                format!("let amount = amount_{id}; {moving}")
            }
            (MovedCounters::All, _) => format!("let amount = {stored}::MAX; {moving}"),
            (MovedCounters::AnyNumber, Some(_)) => format!("let most = g.state().obj(from).counters(kind).{most_read} as u32; if most > 0 {{ let amount = cantrip_mechanics::choose::number(g, actor_{id}, asker.source, 0, most, \"Move counters\")? as {stored}; {moving} }}"),
            (MovedCounters::OneOfEachKindMissing, _) => format!("if g.state().obj(to).counters(kind) == 0 {{ let amount = 1{stored}; {moving} }}"),
            // One question for each kind, as much of it as the mover likes.
            (MovedCounters::AnyNumber, None) => format!("let most = g.state().obj(from).counters(kind).{most_read} as u32; if most > 0 {{ let amount = cantrip_mechanics::choose::number(g, actor_{id}, asker.source, 0, most, \"Move counters\")? as {stored}; {moving} }}"),
            // The kind is chosen as this resolves (608.2d), among those it has.
            (MovedCounters::One, None) => {
                writeln!(out, "let mut done_{id}: i64 = 0; let actor_{id} = {actor}; let sources_{id} = {sources}; let to_{id}: Option<ObjectId> = ({recipient}).first().copied(); if let (Some(&from), Some(to)) = (sources_{id}.first(), to_{id}) {{ if let Some(kind) = g.choose_counter_kind(actor_{id}, asker.source, from)? {{ let amount = 1{stored}; {moving} }} }}").unwrap();
                if self.read_later(result) {
                    self.performed.insert(result.0, format!("done_{id}"));
                }
                return Ok(());
            }
            _ => {
                let least = match amount {
                    MovedCounters::One => "1".to_string(),
                    MovedCounters::AtLeast(n) => self.fetch(self.number(n)?.quantity(Quantity::StoredCounter)?),
                    _ => "0".to_string(),
                };
                let most = if matches!(amount, MovedCounters::One) { "1".to_string() } else { format!("{stored}::MAX") };
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
        writeln!(out, "let sources_{id}: Vec<[{stored_ty}; CounterKind::COUNT]> = ({sources}).iter().map(|&from| g.obj(from).counter_array()).collect(); let recipients_{id}: Vec<ObjectId> = {recipients}; let puts_{id}: Vec<(ObjectId, CounterKind, {counter_ty})> = recipients_{id}.iter().flat_map(|&to| sources_{id}.iter().flat_map(move |counts| CounterKind::ALL.into_iter().map(move |kind| (to, kind, {copied_counter_read})))){only}.collect(); g.add_counters_together(&puts_{id});", stored_ty = Quantity::StoredCounter.kind(), counter_ty = Quantity::CounterPlacement.kind(), copied_counter_read = Quantity::CounterPlacement.from_stored("counts[kind as usize]")).unwrap();
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
        result: &ResultId,
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
        // The copies, for "those spells" (Spinerock Tyrant) after.
        let kept = lane_n::read_as_objects(&self.after, result) || format!("{:?}", self.after).contains("what: CopiedSpell");
        let copies = match kept {
            true => format!(" let mut copies_{id}: Vec<ObjectId> = Vec::new();"),
            false => String::new(),
        };
        let keep = |call: String| match kept {
            true => format!("copies_{id}.extend({call});"),
            false => format!("{call};"),
        };
        writeln!(out, "let copier_{id} = {copier}; let spell_{id}: Option<ObjectId> = ({list}).first().copied();{copies}").unwrap();
        if effects.is_empty() {
            writeln!(
                out,
                "if let Some(spell) = spell_{id} {{ {open}{}{close} }}",
                keep(format!("g.copy_spell(spell, copier_{id}, {new_targets})?"))
            )
            .unwrap();
        } else {
            let values = self.copy_values(&format!("spell_{id}"), effects)?;
            writeln!(out, "let values_{id} = {values}; if let (Some(spell), Some(values)) = (spell_{id}, values_{id}) {{ {open}{}{close} }}", keep(format!("g.copy_spell_as(spell, copier_{id}, {new_targets}, values)?"))).unwrap();
        }
        if kept {
            self.moved.insert(result.0, format!("copies_{id}"));
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
            // Ertai's Meddling: "puts it onto the stack as a copy of the original
            // spell": the card itself goes, not a copy made on the stack.
            Operation::CopyOnStack { card: Some(_), .. } => {
                return Err(gap("a card put onto the stack as a copy of the spell it was"))
            }
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
                self.copy_on_stack(id, player, object, times, new_targets, effects, result, out)?;
            }
            Operation::Retarget {
                actor: PlayerSet::One(player),
                object,
                targets:
                    targets @ (NewTargets::Must
                    | NewTargets::May
                    | NewTargets::To(ObjectSet::One(ObjectRef::Source))),
            } => self.retarget(id, player, object, targets, out)?,
            Operation::Retarget {
                actor: PlayerSet::One(player),
                object,
                targets: NewTargets::To(to),
            } => self.retarget_to(id, player, object, to, out)?,
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
                // "Choose a creature type. You get an emblem with '... of the chosen
                // type'": the emblem keeps the type for its abilities.
                let kept = self
                    .chosen_local(ValueKind::CreatureType)
                    .ok()
                    .filter(|local| !local.ends_with(chosen::KEPT))
                    .filter(|_| format!("{abilities:?}").contains("Chosen(CreatureType)"));
                let mut place = self.quoted_text();
                if kept.is_some() {
                    place.keeps.push(ValueKind::CreatureType);
                }
                let child = emit_face(&card, Some(&name), place)?;
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
                match kept {
                    Some(chosen) => writeln!(out, "for player in {} {{ cantrip_mechanics::tokens::create_emblem_choosing_subtype(g, player, &{path}, {chosen}); }}", self.fetch(self.players(actor)?)).unwrap(),
                    None => writeln!(out, "for player in {} {{ cantrip_mechanics::tokens::create_emblem(g, player, &{path}); }}", self.fetch(self.players(actor)?)).unwrap(),
                }
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
             .. } if effects::plain_entry(entry) => {
                let by = match (self.links_exile, self.stack_item) {
                    (true, true) => "Some(g.linked(item))".to_string(),
                    // A replacement's `then`: what linked abilities know its text by.
                    (true, false) if self.pending != Pending::Entering => {
                        "Some(g.text_linked(asker.source))".to_string()
                    }
                    (true, false) => planned::planned_value(
                        "exile outside a stack item that an ability reads as exiled with its source",
                        "None",
                        "Some(g.linked_to(asker))",
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
             .. } => self.move_objects_p(id, objects, destination, entry, true, result, out)?,
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
                Ability::Triggered { .. }
                | Ability::Replacement { .. }
                | Ability::TriggerRule { .. } => {
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
                    // `?` asks the player as the resolution does, outside a closure.
                    &format!("match {player} {{ Some(player) => g.put_onto_battlefield_enchanting(o, controller, player)?, None => None::<ObjectId> }}"),
                );
                format!("moved.extend({call});")
            }
            _ => self.arrival_rest(entry, counters)?,
        })
    }
    /// Entry has a typed enchant restriction instead of an ordinary delayed
    /// ability slot. Both single/batch entry paths use the same extraction.
    pub(super) fn entry_ability(&mut self, ability: &Ability) -> Result<Option<usize>> {
        match ability {
            Ability::Enchant { hosts } => {
                self.aura_enchant(hosts)?;
                Ok(None)
            }
            other => self.grant(other).map(Some),
        }
    }
    /// Visit erased nonkeyword definitions in their printed order, so a later
    /// retained grant still receives its own quoted text and delayed identity.
    /// Their references are omitted from EntersAs. Enchant extraction has no
    /// quotation cursor and must not install a discarded restriction.
    pub(super) fn entry_discarded_abilities(&mut self, changes: &[Change]) -> Result<()> {
        if let Some(last) = changes
            .iter()
            .rposition(|change| matches!(change, Change::RemoveAllAbilities))
        {
            for change in &changes[..last] {
                if let Change::GrantAbility(ability) = change {
                    if !matches!(ability.as_ref(), Ability::Enchant { .. }) {
                        self.grant(ability)?;
                    }
                }
            }
        }
        Ok(())
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
        if !self.stack_item {
            return Err(gap("move arrival state"));
        }
        if entry.tapped {
            return Err(gap("enters tapped as something else"));
        }
        self.entry_discarded_abilities(changes)?;
        let mut fields = Vec::new();
        let mut adds = Vec::new();
        let mut subtypes = Vec::new();
        let mut keywords = Vec::new();
        let mut abilities = Vec::new();
        for change in crate::change_lists::entering_changes(changes) {
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
                    if let Some(index) = self.entry_ability(ability)? { abilities.push(index.to_string()); }
                }
                Change::GrantKeyword(k)
                    if !matches!(k, Keyword::Protection(_) | Keyword::Ward(_)) =>
                {
                    keywords.push(keyword(k)?)
                }
                Change::RemoveAllAbilities => {
                    fields.push("loses_other_abilities: true".to_string())
                }
                // "It's an Aura enchantment with enchant [quality] and [ability], and
                // it loses all other abilities."
                Change::AddEnchantmentType(kind) if matches!(kind.as_ref(), Operand::Vocabulary {family,form} if family == "EnchantmentType" && form == "Aura") => {
                    fields.push("aura: true".to_string())
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
            let n = self.number(n)?.quantity(Quantity::CounterPlacement)?;
            counters.push(format!("({}, {})", counter_kind(kind)?, self.fetch(n)));
        }
        Ok(format!("{{ let rows: Vec<(ObjectId, PlayerId)> = objects_{id}.iter().map(|&o| (o, {controller})).collect(); let counters: Vec<(CounterKind, {counter_ty})> = vec![{}]; g.put_all_onto_battlefield_becoming(&rows, item, &ENTERS_{id}, &counters)? }}", counters.join(", "), counter_ty = Quantity::CounterPlacement.kind()))
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
        let tapped = entry.simple_tapped().ok_or_else(|| gap("operation Conjure"))?;
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
    pub(super) fn chooses_top_or_bottom(
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
        let tapped = entry.simple_tapped().ok_or_else(|| gap("operation Manifest"))?;
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
    /// "If it's a land card, you may reveal it and put it into your hand", "If it's a
    /// land card, you may put it onto the battlefield tapped" (Risen Reef, Nissa,
    /// Steward of Elements): asked of such a card alone. `out` gets the look; the
    /// result opens the question.
    pub(super) fn top_card_offered(
        &mut self,
        id: usize,
        body: &Program,
        out: &mut String,
    ) -> Result<String> {
        let [Instruction::Effect {
            operation:
                Operation::MoveTopCard {
                    actor: PlayerSet::One(player),
                    cards,
                    ..
                },
            ..
        }] = body.instructions.as_slice()
        else {
            return Ok(String::new());
        };
        if matches!(cards, ObjectFilter::Any) {
            return Ok(String::new());
        }
        let filter = self.filter(cards, &Candidate::of("id"))?;
        writeln!(out, "let looked_{id} = cantrip_mechanics::top_of_library(g, {}, 1); let offered_{id} = {{ let q: &dyn Query = g; looked_{id}.iter().copied().any(|id| {filter}) }};", self.fetch(self.certain_player(player)?)).unwrap();
        Ok(format!("offered_{id} && "))
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
            arrival_at(entry, destination).ok_or_else(|| gap("operation MoveTopCard"))?;
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
    /// One search shown one list (701.23a): "the top third of your library", "your
    /// library and graveyard", and "your graveyard, hand, and/or library", where the
    /// searcher first says whether the library is among them (`library::
    /// search_zone_and_or_library`, of two zones). Whether this was such a search.
    fn search_as_one(
        &mut self,
        id: usize,
        op: &Operation,
        result: &ResultId,
        out: &mut String,
    ) -> Result<bool> {
        const LIBRARY: &str = "cantrip_mechanics::library";
        const SELECTION: &str = "cantrip_mechanics::select::Selection";
        let Operation::Search {
            player,
            library,
            top,
            choices,
            also,
            either,
            outside: false,
        } = op
        else {
            return Ok(false);
        };
        if format!("{player:?}") != format!("{library:?}") {
            return Ok(false);
        }
        // "A card named Magnifying Glass and/or a card named Thinking Cap" from
        // "your graveyard, hand and/or library" (Agency Outfitter): one choice
        // over the zones searched, each card told off among the clauses.
        let clauses = match choices.as_slice() {
            [choice] if choice.constraints.is_empty() => None,
            [_, _, ..] if *either && top.is_none() && also.len() == 2 => {
                match self.clauses_judge(id, choices)? {
                    Some(clauses) => Some(clauses),
                    None => return Ok(false),
                }
            }
            _ => return Ok(false),
        };
        let choice = &choices[0];
        let zone_of = |zone: &EventZone, owner: &str| match zone {
            EventZone::Graveyard => Ok(format!("Zone::graveyard({owner})")),
            EventZone::Hand => Ok(format!("Zone::hand({owner})")),
            _ => Err(gap("operation Search")),
        };
        let owner = format!("owner_{id}");
        let shown = match (top, also.as_slice(), *either) {
            (Some(n), [], false) => {
                let n = self.fetch(self.number(n)?.code);
                format!("if may_{id} {{ g.state().zone(Zone::library({owner})).iter().copied().take(({n}).max(0) as usize).collect::<Vec<ObjectId>>() }} else {{ Vec::new() }}")
            }
            (None, [zone], false)
                if matches!(choice.count, Count::AsManyAsCan(_) | Count::Exactly(_)) =>
            {
                if format!("{:?}", self.after).contains("ReorderLibrary") {
                    return Err(gap("cards found in two zones put on top of a library"));
                }
                let zone = zone_of(zone, &owner)?;
                format!("(if may_{id} {{ {LIBRARY}::searched_cards(g, {owner}) }} else {{ Vec::new() }}).into_iter().chain(g.state().zone({zone}).iter().copied()).collect::<Vec<ObjectId>>()")
            }
            (None, [first, second], true) => {
                let (first, second) = (zone_of(first, &owner)?, zone_of(second, &owner)?);
                format!("(if may_{id} {{ {LIBRARY}::searched_cards(g, {owner}) }} else {{ Vec::new() }}).into_iter().chain(g.state().zone({first}).iter().copied()).chain(g.state().zone({second}).iter().copied()).collect::<Vec<ObjectId>>()")
            }
            _ => return Ok(false),
        };
        if clauses.is_none() && effects::one_at_most(&choice.count) {
            self.single.insert(result.0);
        }
        let destination = effects::search_destination(result, &self.after);
        let purpose = match destination {
            "Battlefield" | "BattlefieldTapped" => "PutOntoBattlefield",
            "Graveyard" => "PutIntoGraveyard",
            "Exile" => "Exile",
            "TopOfLibrary" => "PutOnTop",
            _ => "PutIntoHand",
        };
        let searcher = self.fetch(self.certain_player(player)?);
        let (rule_whole, rule_zones) = match clauses {
            Some((total, judge)) => {
                self.helpers.insert("clauses");
                writeln!(self.functions, "{judge}").unwrap();
                writeln!(out, "let searcher_{id} = {searcher}; let owner_{id} = searcher_{id}; let may_{id} = g.may_search(searcher_{id}, owner_{id}); let shown_{id}: Vec<ObjectId> = {shown};").unwrap();
                let rule = format!("{SELECTION}::new(cantrip_mechanics::select::up_to({total})).with(cantrip_api::select::part(&(clauses_{id} as SelectFn), 0, \"one for each thing searched for\"))");
                (rule.clone(), rule)
            }
            None => {
                let filter = self.filter(&choice.filter, &Candidate::of("id"))?;
                let count = self.fetch(self.select_count(&choice.count)?);
                writeln!(out, "let searcher_{id} = {searcher}; let owner_{id} = searcher_{id}; let may_{id} = g.may_search(searcher_{id}, owner_{id}); let rule_count_{id} = {count}; let shown_{id}: Vec<ObjectId> = {shown}; let can_{id}: Vec<ObjectId> = {{ let q: &dyn Query = g; shown_{id}.iter().copied().filter(|&id| {filter}).collect() }};").unwrap();
                (
                    format!("{SELECTION}::listed(rule_count_{id}, &shown_{id}, can_{id}.clone())"),
                    format!("{{ let in_zones: Vec<ObjectId> = can_{id}.iter().copied().filter(|o| zones_{id}.contains(o)).collect(); {SELECTION}::listed(rule_count_{id}, &zones_{id}, in_zones) }}"),
                )
            }
        };
        let whole = format!("{{ let rule = {rule_whole}; let found = if shown_{id}.is_empty() {{ Vec::new() }} else {{ match g.ask(searcher_{id}, RequestKind::SearchLibrary {{ source: asker.source, library: shown_{id}.clone(), rule: Box::new(rule), destination: SearchDestination::{destination} }})? {{ Answer::Cards(cards) => cards, _ => Vec::new() }} }}; if may_{id} {{ g.searched(searcher_{id}, owner_{id}); }} found }}");
        if *either {
            let [first, second] = also.as_slice() else {
                return Ok(false);
            };
            let (first, second) = (zone_of(first, &owner)?, zone_of(second, &owner)?);
            writeln!(self.functions, "static ASK_{id}: Question = Question::new(\"SearchLibrary\", \"search your library as well?\", \"Then shuffle. If you don't, you search only the other zones\", \"Search it too\", \"Not the library\");").unwrap();
            writeln!(out, "let zones_{id}: Vec<ObjectId> = g.state().zone({first}).iter().chain(g.state().zone({second}).iter()).copied().collect(); let searched_{id} = {LIBRARY}::ask_may_search(g, searcher_{id}, asker.source, &ASK_{id})?; let found_{id}: Vec<ObjectId> = if searched_{id} {{ {whole} }} else {{ let rule = {rule_zones}; let any = cantrip_api::select::first_answer(&*g, &rule, searcher_{id}, asker.source, &zones_{id}, true).is_some_and(|taken| !taken.is_empty()); if any {{ cantrip_mechanics::choose::select(g, searcher_{id}, asker.source, Purpose::{purpose}, zones_{id}.clone(), rule)? }} else {{ Vec::new() }} }};").unwrap();
            self.searched.insert(result.0, format!("searched_{id}"));
        } else {
            writeln!(out, "let found_{id}: Vec<ObjectId> = {whole};").unwrap();
            self.searched.insert(result.0, "true".to_string());
        }
        self.selections.insert(result.0, format!("found_{id}"));
        Ok(true)
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
        if self.search_as_one(id, op, result, out)? {
            return Ok(());
        }
        if matches!(op, Operation::Search { outside: true, .. }) {
            // Turtles Forever: "shuffle the rest into your library", where the rest
            // may be cards from outside the game, is a plain shuffle in the data.
            if self
                .line_text()
                .is_some_and(|line| line.contains("shuffle the rest into your library"))
            {
                return Err(gap(
                    "the rest of a search outside the game shuffled into the library, read as a shuffle in the data",
                ));
            }
            return Err(gap("search with cards from outside the game"));
        }
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
        // "Search your library and graveyard for a basic land card and a card named
        // Jiang Yanggu": one choice over the cards of every zone searched, each
        // card told off among the clauses, as Doomsday searches both zones.
        // Ecological Appreciation: "Shuffle the chosen cards into your library",
        // where cards may have been found in the graveyard, is a plain shuffle of
        // the library in the data, which leaves those cards where they are.
        if !also.is_empty()
            && self
                .line_text()
                .is_some_and(|line| line.contains("Shuffle the chosen cards into your library"))
        {
            return Err(gap(
                "the chosen cards of a search of more than the library shuffled into it, read as a shuffle in the data",
            ));
        }
        let constrained = matches!(choices.as_slice(), [choice] if !choice.constraints.is_empty());
        if (choices.len() > 1 || constrained) && !*either && !also.is_empty() {
            let zones: Vec<&str> = also
                .iter()
                .map(|zone| match zone {
                    EventZone::Graveyard => Ok("Zone::graveyard"),
                    EventZone::Hand => Ok("Zone::hand"),
                    _ => Err(gap("operation Search")),
                })
                .collect::<Result<_>>()?;
            if format!("{player:?}") != format!("{library:?}") {
                return Err(gap("operation Search"));
            }
            const M: &str = "cantrip_mechanics";
            // What may be taken: the clauses told apart, or the one clause's
            // quality with its rules ("with different names").
            let selection = match constrained {
                false => {
                    let Some((total, judge)) = self.clauses_judge(id, choices)? else {
                        return Err(gap("search with several find clauses"));
                    };
                    self.helpers.insert("clauses");
                    writeln!(self.functions, "{judge}").unwrap();
                    format!("{M}::select::Selection::new({M}::select::up_to({total})).with(cantrip_api::select::part(&(clauses_{id} as SelectFn), 0, \"one for each thing searched for\"))")
                }
                true => {
                    let choice = &choices[0];
                    let test = self.filter(&choice.filter, &Candidate::of("id"))?;
                    let rules = self.search_rules(&choice.constraints)?;
                    let count = self.fetch(self.select_count(&choice.count)?);
                    format!("{M}::select::Selection::listed({count}, &shown_{id}, {{ let q: &dyn Query = g; shown_{id}.iter().copied().filter(|&id| {test}).collect::<Vec<ObjectId>>() }}){rules}")
                }
            };
            let purpose = match effects::search_destination(result, &self.after) {
                "Battlefield" | "BattlefieldTapped" => "PutOntoBattlefield",
                "Graveyard" => "PutIntoGraveyard",
                "Exile" => "Exile",
                "TopOfLibrary" => "PutOnTop",
                _ => "PutIntoHand",
            };
            let searcher = self.fetch(self.certain_player(player)?);
            let others: String = zones
                .iter()
                .map(|zone| {
                    format!(
                        "shown_{id}.extend(g.state().zone({zone}(searcher_{id})).iter().copied());"
                    )
                })
                .collect();
            writeln!(out, "let searcher_{id} = {searcher}; let searches_{id} = g.may_search(searcher_{id}, searcher_{id}); let mut shown_{id}: Vec<ObjectId> = match searches_{id} {{ true => {M}::library::searched_cards(g, searcher_{id}), false => Vec::new() }}; {others} if searches_{id} {{ g.searched(searcher_{id}, searcher_{id}); }} let rule_{id} = {selection}; let found_{id}: Vec<ObjectId> = match shown_{id}.is_empty() {{ true => Vec::new(), false => {M}::choose::select(g, searcher_{id}, asker.source, Purpose::{purpose}, shown_{id}.clone(), rule_{id})? }};").unwrap();
            self.selections.insert(result.0, format!("found_{id}"));
            self.searched.insert(result.0, "true".into());
            return Ok(());
        }
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

/// Whether `TargetSpec::EACH_OPPONENT`'s slots, one for each seat but the
/// controller's, are for `players`, or for all of them but the controller's own,
/// which `own_slot` is.
pub(super) fn seats_have_slots(players: &PlayerSet) -> bool {
    matches!(
        players,
        PlayerSet::All
            | PlayerSet::OpponentsOf(PlayerRef::AbilityController)
            | PlayerSet::OthersThan(PlayerRef::AbilityController)
    )
}

/// Words for the options of a choice whose line doesn't list them ("you may put it
/// on the bottom of its owner's library instead"): where each option puts it, or
/// which word it changes. Options these don't tell apart have no labels.
fn lane_i_option_labels(options: &[Program]) -> Option<Vec<String>> {
    let mut labels: Vec<String> = Vec::new();
    for option in options {
        let mut steps = Vec::new();
        lane_m::flat_groups(&option.instructions, &mut steps);
        // What a replacement does to the event says nothing of the option.
        let first = steps.iter().find(|step| {
            !matches!(step, Instruction::Effect { operation: Operation::ModifyEvent { .. }, .. })
        })?;
        let land = |filter: &ObjectFilter| matches!(filter, ObjectFilter::CardType(t) if form(t) == "Land");
        let label = match first {
            // "Choose land or nonland and reveal cards until you reveal one".
            Instruction::Effect {
                operation: Operation::RevealUntil { filter, .. },
                ..
            } if land(filter) => "Land",
            Instruction::Effect {
                operation: Operation::RevealUntil { filter: ObjectFilter::Not(filter), .. },
                ..
            } if land(filter) => "Nonland",
            Instruction::Effect {
                operation: Operation::Move {
                    objects, destination, ..
                },
                ..
            } => match (objects, destination) {
                (ObjectSet::Take { .. }, _) => "Top card of library",
                (_, Destination::Battlefield(_)) => "Battlefield",
                (_, Destination::OwnersHand) => "Hand",
                (_, Destination::OwnersLibraryTop) => "Top of library",
                (_, Destination::OwnersLibraryBottom) => "Bottom of library",
                (_, Destination::OwnersGraveyard) => "Graveyard",
                (_, Destination::Exile) => "Exile",
                _ => return None,
            },
            Instruction::Effect {
                operation: Operation::ChooseObjects {
                    objects: ObjectSet::Zone { scope: ZoneScope::Players { zone: EventZone::Hand, .. }, .. },
                    ..
                },
                ..
            } => "Card from hand",
            _ => return None,
        };
        if labels.iter().any(|seen| seen == label) {
            return None;
        }
        labels.push(label.to_string());
    }
    Some(labels)
}
/// A target test in a slot of its own for each player: that player controls it.
pub(super) fn per_player_target(count: &Count, test: String) -> String {
    let Count::PerPlayer { players, .. } = count else {
        return test;
    };
    let controls = match seats_have_slots(players) {
        true => "cantrip_mechanics::targets::controlled_by_slot_opponent(q, id)".to_string(),
        false => planned::planned_value(
            "target cardinality PerPlayer",
            "true",
            "cantrip_mechanics::targets::controlled_by_slot_player(q, id)",
        ),
    };
    format!("matches!(t, Target::Object(id) if {controls}) && ({test})")
}

/// The test of the slot for the controller in "for each player, up to one target
/// [permanent] that player controls" (CR 601.2c): the engine's slots for each
/// seat leave the controller's out, so it is a slot of its own, after the printed
/// ones so that theirs keep their places. `None` for any other count.
pub(super) fn own_slot(count: &Count, test: &str) -> Option<String> {
    matches!(
        count,
        Count::PerPlayer {
            players: PlayerSet::All,
            ..
        }
    )
    .then(|| format!("matches!(t, Target::Object(id) if cantrip_mechanics::targets::permanent(q, id) && q.controller(id) == asker.controller) && ({test})"))
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

/// Resolve a typed controller override alongside an action's destination.
pub(super) fn arrival_at(entry: &EntrySpec, destination: &Destination) -> Option<(EntrySpec, Destination)> {
    let mut entry = entry.clone();
    let destination = match entry.controller.take() {
        Some(EntryController::Owner) => Destination::OwnersBattlefield,
        Some(EntryController::Player(player)) => Destination::Battlefield(player),
        None => destination.clone(),
    };
    Some((entry, destination))
}

fn same_kind(a: ViewKind, b: ViewKind) -> bool {
    std::mem::discriminant(&a) == std::mem::discriminant(&b)
}

/// "Discard your hand": nobody is asked, a whole hand leaving nothing to choose.
pub(super) fn whole_hand(amount: &Expr, players: &PlayerSet) -> bool {
    let Expr::CountObjects(objects) = amount else {
        return false;
    };
    let ObjectSet::Zone {
        scope:
            ZoneScope::Players {
                zone: EventZone::Hand,
                players: holders,
            },
        filter: ObjectFilter::Any,
        ..
    } = &**objects
    else {
        return false;
    };
    format!("{holders:?}") == format!("{players:?}")
}

/// "A tapped creature you control" with its "tapped" apart: the cost that says so
/// is paid once the mana is, as what's tapped for that mana can be chosen (601.2g-h).
pub(super) fn without_tapped(filter: &ObjectFilter) -> Option<ObjectFilter> {
    let tapped = |part: &ObjectFilter| matches!(part, ObjectFilter::Tapped);
    let ObjectFilter::All(parts) = filter else {
        return None;
    };
    if !parts.iter().any(tapped) {
        return None;
    }
    let mut rest: Vec<ObjectFilter> = parts.iter().filter(|p| !tapped(p)).cloned().collect();
    Some(match rest.len() {
        1 => rest.remove(0),
        _ => ObjectFilter::All(rest),
    })
}

/// "Becomes an artifact creature" keeps the object's other types (205.1b).
pub(super) fn artifact_creature(types: &[crate::mtg_types::CardType]) -> bool {
    use crate::mtg_types::CardType as T;
    matches!(types, [T::Artifact, T::Creature] | [T::Creature, T::Artifact])
}

#[cfg(test)]
mod entry_ability_tests {
    use super::*;
    #[test]
    fn player_dependencies_distinguish_object_ids_literals_shadowing_and_captures() {
        let binding=Binding(7);
        let read=||Instruction::Effect {result:ResultId(0),operation:Operation::Draw {players:PlayerSet::One(PlayerRef::Bound(Binding(7))),amount:Expr::Constant(1)}};
        assert!(names(&binding,&PlayerRef::Bound(Binding(7))));
        assert!(!names(&binding,&ObjectRef::Bound(Binding(7))));
        assert!(!names(&binding,&Operand::Text("Bound(Binding(7))".into())));
        let shadow=Instruction::ForEachPlayer {players:PlayerSet::All,binding:Binding(7),starting_with:None,in_chosen_direction:false,body:Program {instructions:vec![read()]}};
        assert!(!names(&binding,&shadow));
        let ability=Ability::Spell {announcement:Announcement::empty(),body:Program {instructions:vec![shadow.clone()]}};
        assert!(!names(&binding,&ability));
        let capture=Operation::CreateDelayedTrigger {event:EventSpec::Turn {point:TurnPoint::EndStepBegins,players:PlayerSet::All},start:DelayedStart::Immediately,expires:None,intervening_if:None,body:Program {instructions:vec![read()]}};
        assert!(names(&binding,&capture));
        let shadowed_capture=Operation::CreateDelayedTrigger {event:EventSpec::Turn {point:TurnPoint::EndStepBegins,players:PlayerSet::All},start:DelayedStart::Immediately,expires:None,intervening_if:None,body:Program {instructions:vec![shadow]}};
        assert!(!names(&binding,&shadowed_capture));
    }
    fn enchant() -> Ability {
        Ability::Enchant {
            hosts: EnchantHosts::Permanents(ObjectFilter::All(vec![
                ObjectFilter::CardType(crate::mtg_types::CardType::Creature),
                ObjectFilter::Not(Box::new(ObjectFilter::SameObject(ObjectRef::Source))),
            ])),
        }
    }
    fn activated() -> Ability {
        Ability::Activated {
            origin: ActivationOrigin::Ordinary,
            cost: Cost {
                parts: vec![CostPart::Mana(vec![ManaSymbol::Generic(1)])],
            },
            limits: ActivationLimits::unrestricted(),
            announcement: Announcement {
                targets: vec![],
                modes: None,
            },
            body: Program::empty(),
        }
    }
    fn entry(compiler: &mut Compiler, changes: Vec<Change>, batch: bool) -> Result<String> {
        compiler.stack_item = true;
        let spec = EntrySpec {
            states: vec![EntryState::Changes(changes.clone())],
            ..Default::default()
        };
        if batch {
            compiler.arriving_together(7, &spec, "asker.controller")
        } else {
            compiler.entering_as(7, &spec, &changes, "asker.controller")
        }
    }
    #[test]
    fn both_entry_paths_extract_enchant_and_keep_the_ordinary_ability_identity() {
        for batch in [false, true] {
            let mut compiler = Compiler::new();
            let mut changes = vec![
                Change::SetCardTypes(vec![crate::mtg_types::CardType::Enchantment]),
                Change::AddEnchantmentType(Box::new(Operand::Vocabulary {
                    family: "EnchantmentType".into(),
                    form: "Aura".into(),
                })),
            ];
            changes.extend(crate::change_lists::replace_abilities(vec![
                enchant(),
                activated(),
            ]));
            let code = entry(&mut compiler, changes, batch).unwrap();
            assert!(code.contains("put_all_onto_battlefield_becoming"));
            assert!(compiler.aura.is_some());
            assert_eq!(compiler.delayed.len(), 1);
            assert!(compiler.functions.contains("CardType::Creature"));
            assert!(compiler.functions.contains("asker.source"));
            assert!(compiler.functions.contains("aura: true"));
            assert!(compiler.functions.contains("loses_other_abilities: true"));
            assert!(compiler.functions.contains("abilities: &[0]"));
            syn::parse_file(&compiler.functions).unwrap();
        }
    }
    #[test]
    fn a_later_loss_discards_earlier_entry_grants_and_enchant_metadata() {
        for batch in [false, true] {
            let mut compiler = Compiler::new();
            let changes = vec![
                Change::GrantAbility(Box::new(enchant())),
                Change::GrantKeyword(Keyword::Flying),
                Change::RemoveAllAbilities,
                Change::GrantAbility(Box::new(activated())),
                Change::RemoveAllAbilities,
                Change::GrantKeyword(Keyword::Haste),
            ];
            entry(&mut compiler, changes, batch).unwrap();
            assert!(compiler.aura.is_none());
            // The discarded definition still consumes its own printed quote;
            // it is never included in the final entry ability references.
            assert_eq!(compiler.delayed.len(), 1);
            assert!(!compiler.functions.contains("abilities: &[0]"));
            assert_eq!(
                compiler
                    .functions
                    .matches("loses_other_abilities: true")
                    .count(),
                1
            );
            assert!(compiler.functions.contains("Keyword::Haste"));
            assert!(!compiler.functions.contains("Keyword::Flying"));
            syn::parse_file(&compiler.functions).unwrap();
        }
    }
    #[test]
    fn discarded_grants_do_not_steal_a_retained_grants_quoted_text_or_identity() {
        for batch in [false, true] {
            let mut compiler = Compiler::new();
            let outer = Ability::Static {
                kind: StaticKind::Ordinary,
                affected: EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),
                changes: vec![],
            };
            compiler.printed =
                lane_k::Printed::read("It has \"{1}: Draw a card.\" and \"{1}: Gain 1 life.\".")
                    .aligned(&[outer]);
            compiler.on_ability(0);
            let changes = vec![
                Change::GrantAbility(Box::new(activated())),
                Change::RemoveAllAbilities,
                Change::GrantAbility(Box::new(activated())),
            ];
            entry(&mut compiler, changes, batch).unwrap();
            assert_eq!(compiler.delayed.len(), 2);
            assert!(
                compiler.delayed[0].contains("Draw a card."),
                "{:?}",
                compiler.delayed
            );
            assert!(
                compiler.delayed[1].contains("Gain 1 life."),
                "{:?}",
                compiler.delayed
            );
            assert!(!compiler.delayed[1].contains("Draw a card."));
            assert!(compiler.functions.contains("abilities: &[1]"));
            assert!(!compiler.functions.contains("abilities: &[0]"));
        }
    }
    #[test]
    fn copied_enchant_text_and_fake_aura_words_do_not_enter_the_entry_extraction_path() {
        let mut compiler = Compiler::new();
        assert!(compiler.grant(&enchant()).is_err());
        assert!(entry(
            &mut compiler,
            vec![Change::Copiable(vec![Change::GrantAbility(Box::new(
                enchant()
            ))])],
            false
        )
        .is_err());
        assert!(compiler.aura.is_none());
        for (family, form) in [
            ("EnchantmentType", "NotAura"),
            ("NotAnEnchantmentType", "Aura"),
        ] {
            let mut compiler = Compiler::new();
            assert!(entry(
                &mut compiler,
                vec![Change::AddEnchantmentType(Box::new(Operand::Vocabulary {
                    family: family.into(),
                    form: form.into()
                }))],
                false
            )
            .is_err());
            assert!(compiler.aura.is_none());
        }
    }
}

#[cfg(test)]
mod wide_quantity_receipt_tests {
    use super::*;
    #[test]
    fn gathered_damage_registers_its_semantic_numeric_receipt() {
        planned::with_features(&[quantity::FEATURE], || {
            let mut compiler = Compiler::new();
            compiler.after = vec![Instruction::Effect { result: ResultId(1), operation: Operation::GainLife {
                players: PlayerSet::One(PlayerRef::AbilityController), amount: Expr::Performed(ResultId(0)) } }];
            let source = ObjectRef::Source;
            let recipients = Recipients::Players(PlayerSet::One(PlayerRef::AbilityController));
            let amount = Expr::Constant(i64::MAX);
            let mut program = Program { instructions: vec![Instruction::Effect { result: ResultId(0), operation: Operation::DealDamage {
                source: source.clone(), recipients: recipients.clone(), amount: amount.clone(), preventable: true } }] };
            let mut body = String::new();
            compiler.after_gathering(0, Gathered::Damage { source, recipients, amount }, &mut program, 0, &mut body).unwrap();
            assert!(body.contains("fold(0i64, i64::saturating_add)"), "{body}");
            assert_eq!(compiler.quantity_results.get(&0), Some(&Quantity::Damage));
            let doubled = compiler.number(&Expr::multiply(Expr::Performed(ResultId(0)), Expr::Constant(2))).unwrap();
            assert!(doubled.code.contains("saturating_mul"), "{}", doubled.code);
        });
    }
}
