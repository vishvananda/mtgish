//! Rules-derived properties of complete typed programs. Keep these separate
//! from Cantrip capabilities and avoid a serialized flag that can become stale.
use crate::ir::*;
use serde::Serialize;

/// Structural equality of typed references, without imposing Eq on the entire
/// recursive query/expression language. This compares data, never source text.
pub(crate) fn same_player(a: &PlayerRef, b: &PlayerRef) -> bool {
    serde_json::to_value(a).expect("typed player") == serde_json::to_value(b).expect("typed player")
}
pub(crate) fn shuffle_closes(library: &PlayerRef, players: &PlayerSet) -> bool {
    match players {
        PlayerSet::One(player) => same_player(library, player),
        PlayerSet::All => true,
        PlayerSet::Union(sets) => sets.iter().any(|s| shuffle_closes(library, s)),
        _ => false,
    }
}

/// References guaranteed by every possible occurrence of an event matcher.
/// Derived from the typed matcher rather than serialized as independent flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventBindings {
    pub object: bool,
    pub permanent: bool,
    pub spell: bool,
    pub player: bool,
    pub amount: bool,
    pub moved: bool,
    pub damage: bool,
    pub damage_source: bool,
    pub damage_source_permanent: bool,
    pub damage_recipient: bool,
    pub damage_objects: bool,
    pub damage_players: bool,
    pub damage_subject: Option<DamageSubject>,
    pub discard: bool,
    pub discarded: bool,
    pub activation_x: bool,
    pub block_pair: bool,
    pub group: bool,
    pub stack_object: bool,
    pub other: bool,
    pub other_player: bool,
    pub moved_owner: bool,
}
impl EventBindings {
    /// What every alternative of an either-event still binds.
    fn shared(a: Self, b: Self) -> Self {
        Self {
            object: a.object && b.object,
            permanent: a.permanent && b.permanent,
            spell: a.spell && b.spell,
            player: a.player && b.player,
            amount: a.amount && b.amount,
            moved: a.moved && b.moved,
            block_pair: a.block_pair && b.block_pair,
            group: a.group && b.group,
            stack_object: a.stack_object && b.stack_object,
            other: a.other && b.other,
            other_player: a.other_player && b.other_player,
            moved_owner: a.moved_owner && b.moved_owner,
            damage: a.damage && b.damage,
            damage_source: a.damage_source && b.damage_source,
            damage_source_permanent: a.damage_source_permanent && b.damage_source_permanent,
            damage_recipient: a.damage_recipient && b.damage_recipient,
            damage_objects: a.damage_objects && b.damage_objects,
            damage_players: a.damage_players && b.damage_players,
            damage_subject: if a.damage_subject == b.damage_subject {
                a.damage_subject
            } else {
                None
            },
            discard: a.discard && b.discard,
            discarded: a.discarded && b.discarded,
            activation_x: a.activation_x && b.activation_x,
        }
    }
}
/// References available in the body of a replacement on this event.
pub fn replacement_bindings(event: &ReplacementEvent) -> EventBindings {
    match event {
        ReplacementEvent::ZoneChange { objects, from, to } => {
            event_bindings(&EventSpec::ZoneChange {
                objects: objects.clone(),
                from: from.clone(),
                to: to.clone(),
            })
        }
        ReplacementEvent::Primitive { name, operands } => event_bindings(&EventSpec::Primitive {
            name: name.clone(),
            operands: operands.clone(),
        }),
        ReplacementEvent::Enter { .. } => EventBindings {
            object: true,
            permanent: true,
            ..Default::default()
        },
        ReplacementEvent::Measured { event, .. } | ReplacementEvent::EachPoint(event) => {
            replacement_bindings(event)
        }
        ReplacementEvent::Damage(damage) => event_bindings(&EventSpec::Damage(damage.clone())),
        ReplacementEvent::Player { event, .. } => EventBindings {
            player: true,
            amount: matches!(
                event,
                ReplacedPlayerEvent::GainLife
                    | ReplacedPlayerEvent::LoseLife
                    | ReplacedPlayerEvent::GetEnergy
                    | ReplacedPlayerEvent::Mill
                    | ReplacedPlayerEvent::Scry
            ),
            ..Default::default()
        },
        ReplacementEvent::Object { .. } => EventBindings {
            object: true,
            permanent: true,
            ..Default::default()
        },
        ReplacementEvent::Tokens { .. } => EventBindings {
            amount: true,
            ..EventBindings::default()
        },
        ReplacementEvent::Counters { players, .. } => EventBindings {
            object: players.is_none(),
            permanent: players.is_none(),
            amount: true,
            ..Default::default()
        },
        ReplacementEvent::ProduceMana { .. } => EventBindings {
            amount: true,
            ..Default::default()
        },
        ReplacementEvent::FirstEachTurn(event) => replacement_bindings(event),
        ReplacementEvent::Either(events) => events
            .iter()
            .map(replacement_bindings)
            .reduce(EventBindings::shared)
            .unwrap_or_default(),
    }
}
/// Whether an operation affects at most one object, given which earlier results do.
pub fn singular_operation(operation: &Operation, single: impl Fn(&ResultId) -> bool) -> bool {
    let one = |count: &Count| {
        matches!(
            count,
            Count::Exactly(Expr::Constant(1))
                | Count::AsManyAsCan(Expr::Constant(1))
                | Count::UpTo(Expr::Constant(1))
        )
    };
    let chosen = |objects: &ObjectSet| match objects {
        ObjectSet::One(_) => true,
        ObjectSet::Selected(result) => single(result),
        _ => false,
    };
    match operation {
        Operation::ChooseObjects { count, .. } => one(count),
        Operation::Sacrifice {
            objects, choose, ..
        } => choose.as_ref().map_or_else(|| chosen(objects), one),
        Operation::Tap { objects, .. } | Operation::Destroy { objects, .. } => chosen(objects),
        Operation::Discard {
            players: PlayerSet::One(_),
            amount: Expr::Constant(1),
            ..
        }
        | Operation::Draw {
            players: PlayerSet::One(_),
            amount: Expr::Constant(1),
        } => true,
        Operation::DiscardObjects { objects } => chosen(objects),
        Operation::CreateTokens {
            players: PlayerSet::One(_),
            tokens,
            ..
        } => matches!(
            tokens.as_slice(),
            [TokenSpec {
                amount: Expr::Constant(1),
                ..
            }]
        ),
        _ => matches!(
            yields(operation),
            Some(
                "observed"
                    | "exiled"
                    | "entered"
                    | "hand"
                    | "extra_turn"
                    | "created"
                    | "amassed"
                    | "paid"
                    | "graveyard"
            )
        ),
    }
}

/// Manifesting puts one card onto the battlefield (701.40a), as a reviewed
/// primitive with no ordinary movement to read the permanent from.
/// What a reviewed action row leaves for a later "this way" to name.
pub fn yields(operation: &Operation) -> Option<&'static str> {
    if let Operation::ChangeZone { destination, .. } = operation {
        return Some(match destination {
            Destination::OwnersHand | Destination::Hand(_) => "hand",
            Destination::OwnersGraveyard => "graveyard",
            Destination::Exile => "exiled",
            Destination::Battlefield(_) | Destination::OwnersBattlefield => "entered",
            _ => "moved",
        });
    }
    let name = match operation {
        Operation::Primitive { name, .. } => name.as_str(),
        Operation::Reveal {
            objects: ObjectSet::One(_) | ObjectSet::LibraryTops(_) | ObjectSet::Take { .. },
        } => "Action::RevealFaceDownPermanent",
        Operation::Exchange {
            kind: ExchangeKind::Control,
            until: Some(_),
            ..
        } => "Action::ExchangeControlOfEachPermanentWithPlayerUntil",
        Operation::ChooseValueWith {
            kind: ValueKind::Object,
            terms,
            ..
        } if terms.secret && matches!(terms.options.as_slice(), [Operand::Filter(_)]) => {
            "Action::SecretlyChooseAGraveyardCard"
        }
        Operation::Keyword { action, .. } => {
            KEYWORD_ACTIONS.iter().find(|(_, row)| row == action)?.0
        }
        Operation::Conjure {
            zone: EventZone::Hand,
            ..
        } => "Action::ConjureCardIntoHand",
        Operation::Conjure {
            zone: EventZone::Battlefield,
            ..
        } => "Action::ConjureCardOntoBattlefield",
        Operation::KeywordEach {
            action: KeywordAction::Goad,
            ..
        } => "Action::GoadEachCreature",
        Operation::PayAnyAmount {
            resource: PaidResource::Energy,
            ..
        } => return None,
        Operation::PayAnyAmount { .. } => "Action::PayAnyAmountOfMana",
        Operation::DrawUpTo { .. } => "Action::DrawUptoNumberCards",
        Operation::DiscardAllBut { .. } => "Action::DiscardAllButNumberCards",
        Operation::RevealFromHand {
            count: Some(Expr::Constant(1)),
            ..
        } => "Action::RevealACardFromHand",
        Operation::DraftFromSpellbook { .. } => "Action::DraftACardFromSpellBook",
        Operation::TowardPlayer {
            action: PlayerPairAction::ExchangeLife,
            ..
        } => "Action::ExchangeLifeTotalWithPlayer",
        Operation::Manifest {
            source:
                ManifestSource::TopOfLibrary {
                    count: Expr::Constant(1),
                    ..
                },
            ..
        } => "Action::ManifestTheTopCardOfPlayersLibrary",
        Operation::Manifest {
            source: ManifestSource::Cards(ObjectSet::One(_)),
            ..
        } => "Action::CloakCardFromHand",
        _ => return None,
    };
    crate::primitive_spec::lookup(name)?.yields.as_deref()
}

/// Action rows that are one keyword action. Each keeps its reviewed `yields`.
const KEYWORD_ACTIONS: &[(&str, KeywordAction)] = &[
    ("Action::TransformPermanent", KeywordAction::Transform),
    ("Action::Fight", KeywordAction::Fight),
    ("Action::CollectEvidence", KeywordAction::CollectEvidence),
    ("Action::Investigate", KeywordAction::Investigate),
    ("Action::Proliferate", KeywordAction::Proliferate),
    ("Action::Amass", KeywordAction::Amass),
    ("Action::ExploreWithPermanent", KeywordAction::Explore),
    ("Action::TemptWithRing", KeywordAction::RingTempts),
    ("Action::CreatureConnives", KeywordAction::Connive),
    ("Action::GoadCreature", KeywordAction::Goad),
    ("Action::Earthbend", KeywordAction::Earthbend),
    ("Action::VentureIntoTheDungeon", KeywordAction::Venture),
    ("Action::ManifestDread", KeywordAction::ManifestDread),
    ("Action::Recruit", KeywordAction::Recruit),
    ("Action::Epic", KeywordAction::Epic),
    ("Action::Paradigm", KeywordAction::Paradigm),
    (
        "Action::TakeANumberOfExtraTurns",
        KeywordAction::TakeExtraTurn,
    ),
    ("Action::Monstrosity", KeywordAction::Monstrosity),
    ("Action::Discover", KeywordAction::Discover),
    ("Action::GetEnergy", KeywordAction::GainEnergy),
    ("Action::BecomeTheMonarch", KeywordAction::BecomeMonarch),
    ("Action::LoseTheGame", KeywordAction::LoseGame),
    ("Action::WinTheGame", KeywordAction::WinGame),
    ("Action::TakeAnExtraTurn", KeywordAction::TakeExtraTurn),
    ("Action::PhaseOutPermanent", KeywordAction::PhaseOut),
    ("Action::PreparePermanent", KeywordAction::Prepare),
    ("Action::TapOrUntapPermanent", KeywordAction::TapOrUntap),
    ("Action::ExchangeControl", KeywordAction::ExchangeControl),
    ("Action::RollAD20", KeywordAction::RollDie(20)),
    ("Action::RollAD10", KeywordAction::RollDie(10)),
    ("Action::RollAD6", KeywordAction::RollDie(6)),
    ("Action::RollNumberD6", KeywordAction::RollDie(6)),
    ("Action::RollAD4", KeywordAction::RollDie(4)),
    ("Action::RollNumberD4", KeywordAction::RollDie(4)),
    ("Action::EmpowerJace", KeywordAction::EmpowerJace),
    ("Action::Incubate", KeywordAction::Incubate),
    ("Action::Adapt", KeywordAction::Adapt),
    ("Action::Populate", KeywordAction::Populate),
    ("Action::InvestigateTimes", KeywordAction::Investigate),
    ("Action::ProliferateNumberTimes", KeywordAction::Proliferate),
    ("Action::CreatureConnivesNumber", KeywordAction::Connive),
    ("Action::Bolster", KeywordAction::Bolster),
    ("Action::Learn", KeywordAction::Learn),
    ("Action::TakeTheInitiative", KeywordAction::TakeInitiative),
    ("Action::OpenAnAttraction", KeywordAction::OpenAttraction),
    ("Action::GetNumberTickets", KeywordAction::GetTickets),
    (
        "Action::RemoveCreatureFromCombat",
        KeywordAction::RemoveFromCombat,
    ),
    ("Action::FlipPermanent", KeywordAction::Flip),
    ("Action::AwakenPermanent", KeywordAction::Awaken),
    ("Action::Planeswalk", KeywordAction::Planeswalk),
    ("Action::ConvertPermanent", KeywordAction::Convert),
    ("Action::SuspectPermanent", KeywordAction::Suspect),
    ("Action::EndureWithPermanent", KeywordAction::Endure),
    ("Action::Blight", KeywordAction::Blight),
    ("Action::TurnPermanentFaceUp", KeywordAction::TurnFaceUp),
    ("Action::TurnPermanentFaceDown", KeywordAction::TurnFaceDown),
    ("Action::DetainPermanent", KeywordAction::Detain),
    ("Action::EndTheTurn", KeywordAction::EndTheTurn),
    ("Action::TimeTravel", KeywordAction::TimeTravel),
    ("Action::IntensifyPermanent", KeywordAction::Intensify),
    ("Action::Forage", KeywordAction::Forage),
    ("Action::Ascend", KeywordAction::Ascend),
    ("Action::BecomeDay", KeywordAction::BecomeDay),
    ("Action::BecomeNight", KeywordAction::BecomeNight),
    ("Action::ChaosEnsues", KeywordAction::ChaosEnsues),
    ("Action::AirbendPermanent", KeywordAction::Airbend),
    ("Action::SaddlePermanent", KeywordAction::Saddle),
    ("Action::HarnessPermanent", KeywordAction::Harness),
    ("Action::UnattachPermanent", KeywordAction::Unattach),
    ("Action::Fateseal", KeywordAction::Fateseal),
    (
        "Action::SkipNextTurn",
        KeywordAction::Skip(SkippedPart::Turn),
    ),
    (
        "Action::SkipNextUntapStep",
        KeywordAction::Skip(SkippedPart::UntapStep),
    ),
    (
        "Action::SkipNextDrawStep",
        KeywordAction::Skip(SkippedPart::DrawStep),
    ),
    (
        "Action::SkipNextCombatPhase",
        KeywordAction::Skip(SkippedPart::CombatPhase),
    ),
    (
        "Action::SkipNextNumberTurns",
        KeywordAction::Skip(SkippedPart::Turn),
    ),
    (
        "Action::SkipNextCombatPhaseThisTurn",
        KeywordAction::Skip(SkippedPart::CombatPhaseThisTurn),
    ),
    (
        "Action::SkipAllCombatPhasesTheirNextTurn",
        KeywordAction::Skip(SkippedPart::AllCombatPhasesNextTurn),
    ),
    ("Action::RollAD8", KeywordAction::RollDie(8)),
    ("Action::RollAD12", KeywordAction::RollDie(12)),
    ("Action::AirbendSpell", KeywordAction::Airbend),
    ("Action::Assimilate", KeywordAction::Assimilate),
    ("Action::PopulateNumberTimes", KeywordAction::Populate),
    ("Action::TimeTravelNumberTimes", KeywordAction::TimeTravel),
    (
        "Action::ManifestDreadNumberTimes",
        KeywordAction::ManifestDread,
    ),
    (
        "Action::OpenNumberAttractions",
        KeywordAction::OpenAttraction,
    ),
    (
        "Action::ExploreWithPermanentNumberTimes",
        KeywordAction::Explore,
    ),
    ("Action::PhaseInPermanent", KeywordAction::PhaseIn),
    ("Action::UnlockADoorOfPermanent", KeywordAction::UnlockDoor),
    (
        "Action::LockOrUnlockADoorOfPermanent",
        KeywordAction::LockOrUnlockDoor,
    ),
    ("Action::SetCreatureAsBlocked", KeywordAction::BecomeBlocked),
    (
        "Action::RemoveCreatureFromCombatAndUnblockBlockers",
        KeywordAction::RemoveFromCombatUnblocking,
    ),
    ("Action::ReverseTurnOrder", KeywordAction::ReverseTurnOrder),
    ("Action::LoseUnspentMana", KeywordAction::LoseUnspentMana),
    (
        "Action::DoubleEachTypeOfUnspentMana",
        KeywordAction::DoubleUnspentMana,
    ),
    ("Action::DrawTheGame", KeywordAction::DrawGame),
    ("Action::ShuffleGraveyard", KeywordAction::ShuffleGraveyard),
    ("Action::RollThePlanarDie", KeywordAction::RollPlanarDie),
    (
        "Action::RollToVisitAttractions",
        KeywordAction::VisitAttractions,
    ),
    ("Action::EndTheCombatPhase", KeywordAction::EndCombat),
    (
        "Action::RollNumberD20AndIgnoreAllButHighest",
        KeywordAction::RollKeepHighest(20),
    ),
    (
        "Action::RollTwoD4AndChooseAnOrder",
        KeywordAction::RollTwoInOrder(4),
    ),
    (
        "Action::RollTwoD6AndChooseAnOrder",
        KeywordAction::RollTwoInOrder(6),
    ),
    (
        "Action::RollTwoD8AndChooseAnOrder",
        KeywordAction::RollTwoInOrder(8),
    ),
    (
        "Action::RollTwoD10AndChooseAnOrder",
        KeywordAction::RollTwoInOrder(10),
    ),
    (
        "Action::RollTwoD12AndChooseAnOrder",
        KeywordAction::RollTwoInOrder(12),
    ),
    (
        "Action::TakeAnExtraTurnAfterNextTurn",
        KeywordAction::TakeExtraTurnAfterNext,
    ),
    (
        "Action::TakeAnExtraTurnAndSkipUntapStepOfThatTurn",
        KeywordAction::TakeExtraTurnSkippingUntap,
    ),
    ("Action::PlayAMagicSubgame", KeywordAction::PlaySubgame),
    (
        "Action::ThereIsAnAdditionalCombatPhase",
        KeywordAction::AdditionalPhase(AddedPhases::Combat),
    ),
    (
        "Action::ThereIsAnAdditionalCombatPhaseAndAnAdditionalMainPhase",
        KeywordAction::AdditionalPhase(AddedPhases::CombatAndMain),
    ),
    (
        "Action::ThereIsAnAdditionalBeginningPhase",
        KeywordAction::AdditionalPhase(AddedPhases::Beginning),
    ),
];

/// Rows that choose a value, and how many, for an action or an entering object.
const VALUE_CHOICES: &[(&str, ValueKind, u32)] = &[
    ("ChooseAColor", ValueKind::Color, 1),
    ("ChoosePermanentFilter", ValueKind::Filter, 1),
    ("ChooseLibraryFilter", ValueKind::Filter, 1),
    ("SecretlyChooseLibraryFilter", ValueKind::Filter, 1),
    ("ChooseTwoColors", ValueKind::Color, 2),
    ("ChooseAColorOrColorless", ValueKind::ColorOrColorless, 1),
    ("ChooseACreatureType", ValueKind::CreatureType, 1),
    ("ChooseACreatureTypeFromList", ValueKind::CreatureType, 1),
    ("ChooseALandType", ValueKind::LandType, 1),
    ("ChooseLandType", ValueKind::LandType, 1),
    ("ChooseABasicLandType", ValueKind::BasicLandType, 1),
    ("ChooseTwoBasicLandTypes", ValueKind::BasicLandType, 2),
    ("ChooseACardtype", ValueKind::CardType, 1),
    ("ChooseACardtypeFromList", ValueKind::CardType, 1),
    ("ChooseACardName", ValueKind::CardName, 1),
    ("ChooseANumber", ValueKind::Number, 1),
    ("ChooseANumberBetween", ValueKind::Number, 1),
    ("ChooseEvenOrOdd", ValueKind::Parity, 1),
    ("ChooseLeftOrRight", ValueKind::Direction, 1),
    ("ChooseADirection", ValueKind::Direction, 1),
    ("ChooseWord", ValueKind::Word, 1),
    ("ChooseAPlayer", ValueKind::Player, 1),
    ("ChooseTwoPlayers", ValueKind::Player, 2),
    ("ChooseAnAbility", ValueKind::Ability, 1),
    ("ChooseColors", ValueKind::Color, 0),
    ("ChooseTwoColorWords", ValueKind::ColorWord, 2),
    ("ChooseAPlaneswalkerType", ValueKind::PlaneswalkerType, 1),
    ("ChooseADamageRecipient", ValueKind::Recipient, 1),
    ("ChooseACheckableAbility", ValueKind::Ability, 1),
];

/// One counter move as a table: how many, of what, from where, to where.
fn reduce_move_counters(actor: &PlayerSet, operands: &[Operand]) -> Option<Operation> {
    let [Operand::Term {
        family,
        form,
        operands,
    }] = operands
    else {
        return None;
    };
    if family != "MoveCountersAction" {
        return None;
    }
    let (amount, kind, from, to) = match operands.as_slice() {
        [Operand::Number(n), kind, from, Operand::Object(to)] => (
            MovedCounters::Exactly(n.clone()),
            Some(counter_kind(kind)?),
            from,
            to,
        ),
        [kind, from, Operand::Object(to)] => {
            (moved_counters(form)?, Some(counter_kind(kind)?), from, to)
        }
        [from, Operand::Object(to)] => (moved_counters(form)?, None, from, to),
        _ => return None,
    };
    if form.contains("Each") && !form.contains("FromEach") || form.starts_with("Distribute") {
        return None;
    }
    let from = match from {
        Operand::Object(object) => ObjectSet::One(object.clone()),
        Operand::Objects(objects) => objects.clone(),
        _ => return None,
    };
    Some(Operation::MoveCounters {
        actor: actor.clone(),
        from,
        to: to.clone(),
        kind,
        amount,
    })
}

fn moved_counters(form: &str) -> Option<MovedCounters> {
    if form.starts_with("ACounter") {
        Some(MovedCounters::One)
    } else if form.starts_with("AllCounters") {
        Some(MovedCounters::All)
    } else if form.starts_with("AnyNumberOfCounters") {
        Some(MovedCounters::AnyNumber)
    } else {
        None
    }
}

/// Counters on exiled cards, and counters copied from one thing to another.
fn reduce_placed_counters(form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Number, Object, Objects, Player};
    let one = |object: &ObjectRef| Recipients::Objects(ObjectSet::One(object.clone()));
    let duplicate = |from, to, copies| Some(Operation::DuplicateCounters { from, to, copies });
    let player = |who: &PlayerRef| Recipients::Players(PlayerSet::One(who.clone()));
    let (recipients, kind, amount) = match (form, operands) {
        ("DuplicateCountersOfPermanentOnPermanent", [Object(from), Object(to)]) => {
            return duplicate(one(from), one(to), CounterCopies::All)
        }
        ("ADuplicateOfEachKindOfCounterOnPermanentOnPermanent", [Object(from), Object(to)]) => {
            return duplicate(one(from), one(to), CounterCopies::OneOfEachKind)
        }
        ("ADuplicateOfEachKindOfCounterOnPlayerOnPlayer", [Player(from), Player(to)]) => {
            return duplicate(player(from), player(to), CounterCopies::OneOfEachKind)
        }
        ("DuplicateACounterOfPermanentOnPermanent", [Object(from), Object(to)]) => {
            return duplicate(one(from), one(to), CounterCopies::One)
        }
        ("ACounterOfTypeOnCardInExile", [kind, Object(card)]) => {
            (one(card), kind, Expr::Constant(1))
        }
        ("ACounterOfTypeOnEachCardInExile", [kind, Objects(cards)]) => {
            (Recipients::Objects(cards.clone()), kind, Expr::Constant(1))
        }
        ("NumberCountersOfTypeOnCardInExile", [Number(amount), kind, Object(card)]) => {
            (one(card), kind, amount.clone())
        }
        ("NumberCountersOfTypeOnEachCardInExile", [Number(amount), kind, Objects(cards)]) => {
            (Recipients::Objects(cards.clone()), kind, amount.clone())
        }
        _ => return None,
    };
    Some(Operation::PutCounters {
        placements: vec![CounterPlacement {
            recipients,
            kind: counter_kind(kind)?,
            amount,
            selection: None,
        }],
    })
}

fn reduce_double_counters(form: &str, operands: &[Operand]) -> Option<Operation> {
    let (kind, objects) = match (form, operands) {
        ("DoubleCountersOfTypeOnPermanent", [kind, Operand::Object(object)]) => {
            (counter_kind(kind)?, ObjectSet::One(object.clone()))
        }
        ("DoubleCountersOfTypeOnEachPermanent", [kind, Operand::Objects(objects)]) => {
            (counter_kind(kind)?, objects.clone())
        }
        ("DoubleAllCountersOnPermanent", [Operand::Object(object)]) => {
            return Some(Operation::DoubleCounters {
                objects: ObjectSet::One(object.clone()),
                kind: None,
            })
        }
        ("DoubleAllCountersOnEachPermanent", [Operand::Objects(objects)]) => {
            return Some(Operation::DoubleCounters {
                objects: objects.clone(),
                kind: None,
            })
        }
        _ => return None,
    };
    Some(Operation::DoubleCounters {
        objects,
        kind: Some(kind),
    })
}

/// What an entering object does, from a reviewed row.
pub fn reduce_entry(name: &str, operands: &[Operand]) -> Option<EntryState> {
    match (
        name.strip_prefix("ReplacementActionWouldEnter::")?,
        operands,
    ) {
        ("EntersPrepared", []) => Some(EntryState::Prepared),
        ("EnterAsACopyOfAPermanent", [Operand::Objects(objects), effects]) => {
            Some(EntryState::CopyOf {
                objects: objects.clone(),
                effects: Box::new(effects.clone()),
            })
        }
        ("EnterAsACopyOfPermanent", [Operand::Object(object), effects]) => {
            Some(EntryState::CopyOf {
                objects: ObjectSet::One(object.clone()),
                effects: Box::new(effects.clone()),
            })
        }
        _ => None,
    }
}

/// A row's action in generic form, or the row itself when it has none.
pub fn reduce_action(actor: PlayerSet, name: String, operands: Vec<Operand>) -> Operation {
    if name == "Action::MoveCounters" {
        if let Some(operation) = reduce_move_counters(&actor, &operands) {
            return operation;
        }
    }
    if let Some(operation) = name.strip_prefix("PutCountersAction::").and_then(|form| {
        reduce_double_counters(form, &operands).or_else(|| reduce_placed_counters(form, &operands))
    }) {
        return operation;
    }
    match (name.as_str(), operands.as_slice()) {
        ("Action::GetAnEmblem", abilities)
            if abilities.iter().all(|a| matches!(a, Operand::Ability(_))) =>
        {
            return Operation::CreateEmblem {
                actor,
                abilities: abilities
                    .iter()
                    .filter_map(|a| match a {
                        Operand::Ability(ability) => Some((**ability).clone()),
                        _ => None,
                    })
                    .collect(),
            }
        }
        _ => {}
    }
    if let Some(operation) =
        reduce_play(&actor, &name, &operands).or_else(|| reduce_copy(&actor, &name, &operands))
    {
        return operation;
    }
    if let Some(operation) = reduce_named_action(&actor, &name, &operands)
        .or_else(|| reduce_conjure(&actor, &name, &operands))
        .or_else(|| reduce_stack_copy(&actor, &name, &operands))
    {
        return operation;
    }
    if let Some(operation) = name.strip_prefix("Action::").and_then(|form| {
        reduce_effect_on(form, &operands)
            .or_else(|| reduce_change_zone(&actor, form, &operands))
            .or_else(|| reduce_cast(&actor, form, &operands))
            .or_else(|| reduce_set_action(&actor, form, &operands))
            .or_else(|| reduce_exchange(&actor, form, &operands))
            .or_else(|| reduce_stack_action(&actor, form, &operands))
            .or_else(|| reduce_choice(&actor, form, &operands))
            .or_else(|| reduce_coins(&actor, form, &operands))
            .or_else(|| reduce_combat_change(form, &operands))
            .or_else(|| reduce_player_does(&actor, form, &operands))
    }) {
        return operation;
    }
    if let ("Action::CounterAbility", [Operand::Object(object)]) =
        (name.as_str(), operands.as_slice())
    {
        return Operation::Counter {
            objects: ObjectSet::One(object.clone()),
            destination: Destination::OwnersGraveyard,
        };
    }
    let choice = name
        .strip_prefix("Action::")
        .or_else(|| name.strip_prefix("ReplacementActionWouldEnter::"))
        .and_then(|form| VALUE_CHOICES.iter().find(|(row, ..)| *row == form));
    if let Some((_, kind, count)) = choice {
        return Operation::ChooseValue {
            actor,
            kind: *kind,
            count: *count,
            options: operands,
        };
    }
    let action = KEYWORD_ACTIONS
        .iter()
        .find(|(row, _)| *row == name)
        .map(|(_, action)| *action);
    let (object, other, amount, subtype) = match operands.as_slice() {
        [] => (None, None, None, None),
        [Operand::Object(object)] => (Some(object.clone()), None, None, None),
        [Operand::Number(amount)] => (None, None, Some(amount.clone()), None),
        [Operand::Object(object), Operand::Object(other)] => {
            (Some(object.clone()), Some(other.clone()), None, None)
        }
        [Operand::Object(object), Operand::Number(amount)] => {
            (Some(object.clone()), None, Some(amount.clone()), None)
        }
        [Operand::Number(amount), Operand::Vocabulary { form, .. }] => {
            (None, None, Some(amount.clone()), Some(form.clone()))
        }
        [Operand::Number(amount), Operand::Object(object)] => {
            (Some(object.clone()), None, Some(amount.clone()), None)
        }
        _ => {
            return Operation::Primitive {
                actor,
                name,
                operands,
            }
        }
    };
    match action {
        Some(action) => Operation::Keyword {
            actor,
            action,
            object,
            other,
            amount,
            subtype,
        },
        None => Operation::Primitive {
            actor,
            name,
            operands,
        },
    }
}

/// The ordered steps of a list of programs, when every entry is one.
fn program_list(operands: &[Operand]) -> Option<Vec<Program>> {
    operands
        .iter()
        .map(|operand| match operand {
            Operand::Program(program) => Some(program.clone()),
            _ => None,
        })
        .collect()
}

/// Named actions with one meaning, each its own operation or an ordinary one.
/// A vote in generic form.
fn reduce_vote(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Objects, Players, Text};
    let vote = |ballot, secret| Operation::Vote {
        actor: actor.clone(),
        ballot,
        secret,
    };
    let words = |operands: &[Operand]| -> Option<Ballot> {
        operands
            .iter()
            .map(|operand| match operand {
                Text(word) => Some(word.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>()
            .map(Ballot::Words)
    };
    match (name, operands) {
        ("Action::VoteForAWord", options) => return Some(vote(words(options)?, false)),
        ("Action::SecretlyVoteForAWord", options) => return Some(vote(words(options)?, true)),
        ("Action::VoteForAPermanent", [Objects(objects)]) => {
            return Some(vote(Ballot::Objects(objects.clone()), false))
        }
        ("Action::SecretlyVoteForAPermanent", [Objects(objects)]) => {
            return Some(vote(Ballot::Objects(objects.clone()), true))
        }
        ("Action::SecretlyVoteForUptoOnePermanent", [Objects(objects)]) => {
            return Some(vote(Ballot::UpToOne(objects.clone()), true))
        }
        ("Action::SecretlyVoteForAPlayer", [Players(players)]) => {
            return Some(vote(Ballot::Players(players.clone()), true))
        }
        ("Action::VoteForACardInGraveyard", [Operand::Filter(cards)]) => {
            return Some(vote(Ballot::GraveyardCards(cards.clone()), false))
        }
        _ => {}
    }
    None
}

/// Manifest or cloak in generic form.
fn reduce_manifest(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    let form = name.strip_prefix("Action::")?;
    let cloak = form.starts_with("Cloak");
    let top = |player: &PlayerRef, count| ManifestSource::TopOfLibrary {
        player: player.clone(),
        count,
    };
    let source = match (form, operands) {
        (
            "ManifestTheTopCardOfPlayersLibrary" | "CloakTheTopCardOfPlayersLibrary",
            [Operand::Player(player)],
        ) => top(player, Expr::Constant(1)),
        (
            "ManifestTheTopNumberCardsOfPlayersLibrary",
            [Operand::Number(count), Operand::Player(player)],
        ) => top(player, count.clone()),
        ("ManifestEachExiledCard" | "CloakEachExiledCard", [Operand::Objects(cards)]) => {
            ManifestSource::Cards(cards.clone())
        }
        ("ManifestCardFromHand" | "CloakCardFromHand", [Operand::Object(card)]) => {
            ManifestSource::Cards(ObjectSet::One(card.clone()))
        }
        _ => return None,
    };
    Some(Operation::Manifest {
        actor: actor.clone(),
        source,
        cloak,
    })
}

fn reduce_named_action(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    if let Some(vote) = reduce_vote(actor, name, operands) {
        return Some(vote);
    }
    if let Some(manifest) = reduce_manifest(actor, name, operands) {
        return Some(manifest);
    }
    use Operand::{Duration, List, Number, Object, Objects};
    match (name, operands) {
        ("Action::ChooseAPlayerAtRandom", [players]) => {
            return Some(Operation::ChooseAtRandom {
                kind: ValueKind::Player,
                options: vec![players.clone()],
            })
        }
        ("Action::PlayerChooseAnAction", [Operand::Player(player), Operand::List(options)]) => {
            return Some(Operation::ChooseAction {
                actor: PlayerSet::One(player.clone()),
                options: options.clone(),
                villainous: false,
            })
        }
        ("Action::FaceAVillianousChoice", [first, second]) => {
            return Some(Operation::ChooseAction {
                actor: actor.clone(),
                options: vec![first.clone(), second.clone()],
                villainous: true,
            })
        }
        _ => {}
    }
    match (name, operands) {
        (
            "Action::CreateExiledCardEffect" | "Action::CreateSpellEffect",
            [Object(object), List(effects)],
        ) => {
            return Some(Operation::EffectOn {
                subject: EffectSubject::Objects(ObjectSet::One(object.clone())),
                effects: effects.clone(),
                until: None,
            })
        }
        ("Action::CreateEachExiledCardEffect", [Objects(objects), List(effects)]) => {
            return Some(Operation::EffectOn {
                subject: EffectSubject::Objects(objects.clone()),
                effects: effects.clone(),
                until: None,
            })
        }
        (
            "Action::CreateEachSpellEffect",
            [Operand::Filter(spells), List(effects), Duration(until)],
        ) => {
            return Some(Operation::EffectOn {
                subject: EffectSubject::EachSpell(spells.clone()),
                effects: effects.clone(),
                until: Some(until.clone()),
            })
        }
        (
            "Action::CreateSpellOrPermanentEffect",
            [Duration(until), Object(object), List(effects)],
        ) => {
            return Some(Operation::EffectOn {
                subject: EffectSubject::Objects(ObjectSet::One(object.clone())),
                effects: effects.clone(),
                until: Some(until.clone()),
            })
        }
        ("Action::CreateGameEffect", [Duration(until), effect]) => {
            return Some(Operation::GameEffect {
                until: until.clone(),
                effect: Box::new(effect.clone()),
            })
        }
        (
            "Action::AttachAPermanentToPermanent"
            | "Action::AttachAnyNumberOfPermanentsToPermanent",
            [Objects(attachments), Object(host)],
        ) => {
            return Some(Operation::AttachChosen {
                actor: actor.clone(),
                attachments: attachments.clone(),
                count: if name.contains("AnyNumber") {
                    Count::AnyNumber
                } else {
                    Count::Exactly(Expr::Constant(1))
                },
                hosts: ObjectSet::One(host.clone()),
            })
        }
        ("Action::AttachAPermanentToAPermanent", [Objects(attachments), Objects(hosts)]) => {
            return Some(Operation::AttachChosen {
                actor: actor.clone(),
                attachments: attachments.clone(),
                count: Count::Exactly(Expr::Constant(1)),
                hosts: hosts.clone(),
            })
        }
        ("Action::DrawUptoNumberCards", [Number(amount)]) => {
            return Some(Operation::DrawUpTo {
                players: actor.clone(),
                amount: amount.clone(),
            })
        }
        ("Action::CounterEachSpell", [Operand::Filter(spells)]) => {
            return Some(Operation::Counter {
                objects: ObjectSet::Zone {
                    scope: ZoneScope::Shared(EventZone::Stack),
                    kind: ObjectKind::Spell,
                    filter: spells.clone(),
                },
                destination: Destination::OwnersGraveyard,
            })
        }
        ("Action::ControllersSacrificeEachPermanent", [Objects(objects)]) => {
            return Some(Operation::Sacrifice {
                players: PlayerSet::All,
                objects: objects.clone(),
                choose: None,
            })
        }
        ("Action::DiscardAllButNumberCards", [Number(keep)]) => {
            return Some(Operation::DiscardAllBut {
                players: actor.clone(),
                keep: keep.clone(),
            })
        }
        ("Action::SacrificeAllPermanentsExceptForNum", [Objects(objects), Number(keep)]) => {
            return Some(Operation::SacrificeAllBut {
                players: actor.clone(),
                objects: objects.clone(),
                keep: keep.clone(),
            })
        }
        ("Action::TransformEachPermanent", [Objects(objects)]) => {
            return Some(Operation::KeywordEach {
                actor: actor.clone(),
                action: KeywordAction::Transform,
                objects: objects.clone(),
                until: None,
            })
        }
        ("Action::UnsuspectEachPermanent", [Objects(objects)]) => {
            return Some(Operation::KeywordEach {
                actor: actor.clone(),
                action: KeywordAction::Unsuspect,
                objects: objects.clone(),
                until: None,
            })
        }
        (
            "Action::UnsuspectPermanent"
            | "Action::TurnExiledCardFaceUp"
            | "Action::UnpreparePermanent",
            [Object(object)],
        ) => {
            return Some(Operation::KeywordEach {
                actor: actor.clone(),
                action: match name {
                    "Action::UnsuspectPermanent" => KeywordAction::Unsuspect,
                    "Action::TurnExiledCardFaceUp" => KeywordAction::TurnFaceUp,
                    _ => KeywordAction::Unprepare,
                },
                objects: ObjectSet::One(object.clone()),
                until: None,
            })
        }
        (
            "Action::HeistPlayersLibrary"
            | "Action::GainControlOfPlayerDuringTheirNextTurn"
            | "Action::ExchangeLifeTotalWithPlayer",
            [Operand::Player(player)],
        ) => {
            return Some(Operation::TowardPlayer {
                actor: actor.clone(),
                action: match name {
                    "Action::HeistPlayersLibrary" => PlayerPairAction::Heist,
                    "Action::ExchangeLifeTotalWithPlayer" => PlayerPairAction::ExchangeLife,
                    _ => PlayerPairAction::ControlNextTurn,
                },
                player: player.clone(),
            })
        }
        (
            "Action::IntensifyCardsOwnedByPlayer",
            [Operand::Filter(cards), Operand::Player(owner)],
        ) => {
            return Some(Operation::Intensify {
                cards: cards.clone(),
                owner: owner.clone(),
            })
        }
        ("Action::FlipCoins", [Number(count)]) => {
            return Some(Operation::FlipCoins {
                actor: actor.clone(),
                count: count.clone(),
            })
        }
        ("Action::RevealCardFromOutsideGameAndPutInHand", [Operand::Filter(filter)]) => {
            return Some(Operation::Wish {
                actor: actor.clone(),
                filter: filter.clone(),
                reveal: true,
            })
        }
        ("Action::AbandonScheme", [_]) => {
            return Some(Operation::AbandonScheme {
                actor: actor.clone(),
            })
        }
        ("Action::Cipher", [Operand::Object(spell)]) => {
            return Some(Operation::KeywordEach {
                actor: actor.clone(),
                action: KeywordAction::Cipher,
                objects: ObjectSet::One(spell.clone()),
                until: None,
            })
        }
        ("Action::PutAStickerOnAPermanent", [Operand::Objects(objects)]) => {
            return Some(Operation::PutSticker {
                actor: actor.clone(),
                objects: objects.clone(),
                name_only: false,
            })
        }
        ("Action::PutANameStickerOnPermanent", [Operand::Object(object)]) => {
            return Some(Operation::PutSticker {
                actor: actor.clone(),
                objects: ObjectSet::One(object.clone()),
                name_only: true,
            })
        }
        ("Action::BeginGameWithCardOnBattlefield", [Operand::Vocabulary { .. }, flags]) => {
            return Some(Operation::BeginOnBattlefield {
                actor: actor.clone(),
                flags: Box::new(flags.clone()),
            })
        }
        _ => {}
    }
    use Operand::{Players, Text};
    let draft = |spellbook: &String, times: Expr| Operation::DraftFromSpellbook {
        actor: actor.clone(),
        spellbook: spellbook.clone(),
        times,
    };
    let each =
        |action, objects: &ObjectSet, until: Option<&crate::ir::Duration>| Operation::KeywordEach {
            actor: actor.clone(),
            action,
            objects: objects.clone(),
            until: until.cloned(),
        };
    let pay = |resource| Operation::PayAnyAmount {
        actor: actor.clone(),
        resource,
    };
    let reveal = |cards, count, at_random| Operation::RevealFromHand {
        actor: actor.clone(),
        cards,
        count,
        at_random,
    };
    // CR 701.10d: +X/+0 where X is its power as this resolves, each on its own.
    let double = |objects, power, toughness| {
        let own = |doubled: bool, property| {
            if doubled {
                Expr::ObjectProperty {
                    object: ObjectRef::Affected,
                    property,
                    information: Information::CurrentOrLastKnown,
                }
            } else {
                Expr::Constant(0)
            }
        };
        Operation::CreateContinuousEffect {
            source: ObjectRef::Source,
            affected: EffectSubjects::Objects(objects),
            changes: vec![Change::Pump {
                power: own(power, Property::Power),
                toughness: own(toughness, Property::Toughness),
            }],
            duration: crate::ir::Duration::EndOfTurn,
            evaluation: EffectEvaluation::Snapshot,
        }
    };
    Some(match (name.strip_prefix("Action::")?, operands) {
        ("GoadEachCreature", [Objects(objects)]) => each(KeywordAction::Goad, objects, None),
        ("GoadEachCreatureUntil", [Objects(objects), Duration(until)]) => {
            each(KeywordAction::Goad, objects, Some(until))
        }
        ("GoadCreatureUntil", [Object(object), Duration(until)]) => each(
            KeywordAction::Goad,
            &ObjectSet::One(object.clone()),
            Some(until),
        ),
        ("PhaseOutEachPermanent", [Objects(objects)]) => {
            each(KeywordAction::PhaseOut, objects, None)
        }
        ("PhaseOutEachPermanentUntil", [Objects(objects), Duration(until)]) => {
            each(KeywordAction::PhaseOut, objects, Some(until))
        }
        ("PhaseOutPermanentUntil", [Object(object), Duration(until)]) => each(
            KeywordAction::PhaseOut,
            &ObjectSet::One(object.clone()),
            Some(until),
        ),
        ("SuspectEachPermanent", [Objects(objects)]) => each(KeywordAction::Suspect, objects, None),
        ("DetainEachPermanent", [Objects(objects)]) => each(KeywordAction::Detain, objects, None),
        ("AirbendPermanents", [Objects(objects)]) => each(KeywordAction::Airbend, objects, None),
        ("DoubleCreaturesPowerUntilEndOfTurn", [Object(o)]) => {
            double(ObjectSet::One(o.clone()), true, false)
        }
        ("DoubleCreaturesPowerAndToughnessUntilEndOfTurn", [Object(o)]) => {
            double(ObjectSet::One(o.clone()), true, true)
        }
        ("DoubleEachCreaturesPowerUntilEndOfTurn", [Objects(o)]) => double(o.clone(), true, false),
        ("DoubleEachCreaturesToughnessUntilEndOfTurn", [Objects(o)]) => {
            double(o.clone(), false, true)
        }
        ("DoubleEachCreaturesPowerAndToughnessUntilEndOfTurn", [Objects(o)]) => {
            double(o.clone(), true, true)
        }
        ("PutTopCardOfLibraryOfTypeOnBattlefield", [Operand::Filter(cards), flags @ ..]) => {
            Operation::MoveTopCard {
                actor: actor.clone(),
                cards: cards.clone(),
                reveal: false,
                destination: Destination::OwnersBattlefield,
                entry: flags.to_vec(),
            }
        }
        ("RevealTopCardOfLibraryAndPutIntoHand", [Operand::Filter(cards)]) => {
            Operation::MoveTopCard {
                actor: actor.clone(),
                cards: cards.clone(),
                reveal: true,
                destination: Destination::OwnersHand,
                entry: vec![],
            }
        }
        ("PayAnyAmountOfMana", []) => pay(PaidResource::Mana),
        ("PayAnyAmountOfEnergy", []) => pay(PaidResource::Energy),
        ("PayAnyAmountOfLife", []) => pay(PaidResource::Life),
        ("RevealACardFromHand", []) => reveal(ObjectFilter::Any, Some(Expr::Constant(1)), false),
        ("RevealACardFromHandAtRandom", []) => {
            reveal(ObjectFilter::Any, Some(Expr::Constant(1)), true)
        }
        ("RevealNumberCardsFromHand", [Number(n)]) => {
            reveal(ObjectFilter::Any, Some(n.clone()), false)
        }
        ("RevealNumberCardsFromHandAtRandom", [Number(n)]) => {
            reveal(ObjectFilter::Any, Some(n.clone()), true)
        }
        ("RevealAllCardsOfTypeFromHand", [Operand::Filter(cards)]) => {
            reveal(cards.clone(), None, false)
        }
        ("RevealACardOfTypeFromHandAtRandom", [Operand::Filter(cards)]) => {
            reveal(cards.clone(), Some(Expr::Constant(1)), true)
        }
        // CR 701.34a: a +1/+1 counter on each of up to N other target creatures.
        ("Support", [Objects(objects)]) => Operation::PutCounters {
            placements: vec![CounterPlacement {
                recipients: Recipients::Objects(objects.clone()),
                kind: crate::counter::CounterKind::PowerToughness(1, 1),
                amount: Expr::Constant(1),
                selection: None,
            }],
        },
        ("Clash", [Players(opponents), List(won), List(lost)]) => Operation::Clash {
            actor: actor.clone(),
            opponents: opponents.clone(),
            won: program_list(won)?,
            lost: program_list(lost)?,
        },
        ("DraftACardFromSpellBook", [Text(spellbook)]) => draft(spellbook, Expr::Constant(1)),
        ("DraftACardFromSpellBookNumberTimes", [Text(spellbook), Number(times)]) => {
            draft(spellbook, times.clone())
        }
        _ => return None,
    })
}

/// Conjuring, from its several dozen rows: what, how many, and where to.
fn reduce_conjure(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    use ConjureSpot as Spot;
    use ConjuredCard as C;
    use EventZone as Z;
    use Operand::{Number, Object, Objects, Player, Text};
    use SpellbookPick as Pick;
    let form = name.strip_prefix("Action::Conjure")?;
    let one = Expr::Constant(1);
    let named = |card: &String| C::Named(card.clone());
    let dup = |object: &ObjectRef| C::Duplicate(ObjectSet::One(object.clone()));
    let dups = |objects: &ObjectSet| C::Duplicate(objects.clone());
    let book = |book: &String, pick| C::Spellbook {
        book: book.clone(),
        pick,
    };
    let (card, count, zone, owner, spot, entry): (_, _, _, Option<&PlayerRef>, _, &[Operand]) =
        match (form, operands) {
            ("CardIntoHand", [Text(c)]) => (named(c), one, Z::Hand, None, None, &[]),
            ("CardOntoBattlefield", [Text(c), flags @ ..]) => {
                (named(c), one, Z::Battlefield, None, None, flags)
            }
            ("ACardIntoGraveyard", [Text(c)]) => (named(c), one, Z::Graveyard, None, None, &[]),
            ("CardsIntoGraveyard", [Number(n), Text(c)]) => {
                (named(c), n.clone(), Z::Graveyard, None, None, &[])
            }
            ("NumberCardsIntoHand", [Number(n), Text(c)]) => {
                (named(c), n.clone(), Z::Hand, None, None, &[])
            }
            ("NumberCardsOntoBattlefield", [Number(n), Text(c), flags @ ..]) => {
                (named(c), n.clone(), Z::Battlefield, None, None, flags)
            }
            ("NumberCardsIntoLibrary", [Number(n), Text(c)]) => {
                (named(c), n.clone(), Z::Library, None, None, &[])
            }
            ("NumberCardsIntoLibraryAndShuffle", [Number(n), Text(c)]) => (
                named(c),
                n.clone(),
                Z::Library,
                None,
                Some(Spot::Shuffled),
                &[],
            ),
            ("NumberCardsOnTopOfLibrary", [Number(n), Text(c)]) => {
                (named(c), n.clone(), Z::Library, None, Some(Spot::Top), &[])
            }
            ("NumberCardsIntoPlayersGraveyard", [Number(n), Text(c), Player(p)]) => {
                (named(c), n.clone(), Z::Graveyard, Some(p), None, &[])
            }
            ("CardIntoTheTopNumberCardsOfLibraryAtRandom", [Text(c), Number(n)]) => (
                named(c),
                one,
                Z::Library,
                None,
                Some(Spot::AmongTop(n.clone())),
                &[],
            ),
            ("CardIntoLibraryNthFromTop", [Text(c), Number(n)]) => (
                named(c),
                one,
                Z::Library,
                None,
                Some(Spot::FromTop(n.clone())),
                &[],
            ),
            (
                "DuplicateOfPermanentIntoHand"
                | "DuplicateOfSpellIntoHand"
                | "DuplicateOfGraveyardCardIntoHand"
                | "DuplicateOfCardInHandIntoHand"
                | "DuplicateOfExiledCardIntoHand",
                [Object(o)],
            ) => (dup(o), one, Z::Hand, None, None, &[]),
            (
                "DuplicateOfEachPermanentIntoHand"
                | "DuplicateOfEachExiledCardIntoHand"
                | "DuplicateOfEachDestroyedPermanentIntoHand",
                [Objects(o)],
            ) => (dups(o), one, Z::Hand, None, None, &[]),
            (
                "ADuplicateOfPermanentOntoTheBattlefield"
                | "ADuplicateOfSpellCardOntoTheBattlefield",
                [Object(o), flags @ ..],
            ) => (dup(o), one, Z::Battlefield, None, None, flags),
            ("ADuplicateOfEachPermanentOntoTheBattlefield", [Objects(o), flags @ ..]) => {
                (dups(o), one, Z::Battlefield, None, None, flags)
            }
            (
                "DuplicateOfPermanentIntoPlayersGraveyard"
                | "DuplicateOfGraveyardCardIntoPlayersGraveyard"
                | "DuplicateOfExiledCardIntoPlayersGraveyard",
                [Object(o), Player(p)],
            ) => (dup(o), one, Z::Graveyard, Some(p), None, &[]),
            ("ADuplicateOfEachPermanentIntoGraveyard", [Objects(o)]) => {
                (dups(o), one, Z::Graveyard, None, None, &[])
            }
            ("NumberDuplicateCardsIntoGraveyard", [Number(n), Object(o)]) => {
                (dup(o), n.clone(), Z::Graveyard, None, None, &[])
            }
            ("DuplicateOfPermanentIntoExile", [Object(o)]) => {
                (dup(o), one, Z::Exile, None, None, &[])
            }
            ("DuplicateOfGraveyardCardOnTopOfPlayersLibrary", [Object(o), Player(p)]) => {
                (dup(o), one, Z::Library, Some(p), Some(Spot::Top), &[])
            }
            (
                "DuplicateOfExiledCardIntoTheTopNumberCardsOfLibraryAtRandom",
                [Object(o), Number(n)],
            ) => (
                dup(o),
                one,
                Z::Library,
                None,
                Some(Spot::AmongTop(n.clone())),
                &[],
            ),
            ("ACardOfChoiceFromSpellBookIntoHand", [Text(b)]) => {
                (book(b, Pick::Choice), one, Z::Hand, None, None, &[])
            }
            ("NumberCardsOfChoiceFromSpellBookIntoHand", [Number(n), Text(b)]) => {
                (book(b, Pick::Choice), n.clone(), Z::Hand, None, None, &[])
            }
            ("ACardOfChoiceFromSpellBookOntoBattlefield", [Text(b), flags @ ..]) => (
                book(b, Pick::Choice),
                one,
                Z::Battlefield,
                None,
                None,
                flags,
            ),
            ("ARandomCardFromSpellBookIntoHand", [Text(b)]) => {
                (book(b, Pick::Random), one, Z::Hand, None, None, &[])
            }
            ("NumberRandomCardsFromSpellBookIntoHand", [Number(n), Text(b)]) => {
                (book(b, Pick::Random), n.clone(), Z::Hand, None, None, &[])
            }
            ("ARandomCardFromSpellBookOntoBattlefield", [Text(b), flags @ ..]) => (
                book(b, Pick::Random),
                one,
                Z::Battlefield,
                None,
                None,
                flags,
            ),
            ("ARandomCardFromSpellBookIntoExile", [Text(b)]) => {
                (book(b, Pick::Random), one, Z::Exile, None, None, &[])
            }
            ("ARandomCardFromSpellbookIntoTheTopNumberCardsOfLibrary", [Text(b), Number(n)]) => (
                book(b, Pick::Random),
                one,
                Z::Library,
                None,
                Some(Spot::AmongTop(n.clone())),
                &[],
            ),
            ("EachCardFromSpellBookIntoLibrary", [Text(b)]) => {
                (book(b, Pick::Each), one, Z::Library, None, None, &[])
            }
            ("EachCardFromSpellBookOntoTheBattlefield", [Text(b), flags @ ..]) => {
                (book(b, Pick::Each), one, Z::Battlefield, None, None, flags)
            }
            _ => return None,
        };
    Some(Operation::Conjure {
        actor: actor.clone(),
        card,
        count,
        zone,
        owner: owner.cloned(),
        spot,
        entry: entry.to_vec(),
    })
}

/// Copying and retargeting a spell or ability, as Forge has one of each.
fn reduce_stack_copy(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    use NewTargets as T;
    use Operand::{List, Number, Object, Objects, Players};
    let one = || Expr::Constant(1);
    let copy =
        |object: &ObjectRef, times: Expr, targets, effects: &[Operand]| Operation::CopyOnStack {
            actor: actor.clone(),
            object: object.clone(),
            times,
            targets,
            effects: effects.to_vec(),
        };
    let retarget = |object: &ObjectRef, targets| Operation::Retarget {
        actor: actor.clone(),
        object: object.clone(),
        targets,
    };
    Some(match (name.strip_prefix("Action::")?, operands) {
        ("CopySpell", [Object(o)]) => copy(o, one(), T::Same, &[]),
        ("CopySpellNumberTimes" | "CopySpellForEach", [Object(o), Number(n)]) => {
            copy(o, n.clone(), T::Same, &[])
        }
        (
            "CopySpellNumberTimesAndMayChooseNewTargets"
            | "CopySpellForEachAndMayChooseNewTargets"
            | "CopyAbilityNumberTimesAndMayChooseNewTargets",
            [Object(o), Number(n)],
        ) => copy(o, n.clone(), T::May, &[]),
        (
            "CopySpellAndMayChooseNewTargets"
            | "CopyAbilityAndMayChooseNewTargets"
            | "CopySpellOrAbilityAndMayChooseNewTargets",
            [Object(o)],
        ) => copy(o, one(), T::May, &[]),
        ("CopySpellWithModifiers", [Object(o), List(effects)]) => copy(o, one(), T::Same, effects),
        ("CopySpellAndMayChooseNewTargetsWithEffects", [Object(o), List(effects)]) => {
            copy(o, one(), T::May, effects)
        }
        ("CopySpellAndMustChooseNewTarget", [Object(o), Object(to)]) => {
            copy(o, one(), T::To(ObjectSet::One(to.clone())), &[])
        }
        (
            "CopySpellForEachPermanentAndMustChooseItAsNewTarget"
            | "CopyAbilityForEachPermanentItCouldTarget",
            [Object(o), Objects(each)],
        ) => copy(o, one(), T::Each(each.clone()), &[]),
        ("ChooseNewTargetsForSpell" | "ChooseNewTargetsForSpellOrAbility", [Object(o)]) => {
            retarget(o, T::May)
        }
        (
            "ChangeTargetsOfSpell"
            | "ChangeTargetsOfAbility"
            | "ChangeTargetsOfSpellOrAbility"
            | "ChangeTheTargetOfSpell"
            | "ChangeTheTargetOfAbility"
            | "ChangeTheTargetOfSpellOrAbility",
            [Object(o)],
        ) => retarget(o, T::Must),
        ("ChangeTheTargetOfSpellToPermanent", [Object(o), Object(to)]) => {
            retarget(o, T::To(ObjectSet::One(to.clone())))
        }
        ("ChangeTheTargetOfSpellToAPermanent", [Object(o), Objects(to)]) => {
            retarget(o, T::To(to.clone()))
        }
        ("ChangeTheTargetsOfSpellToAPlayer", [Object(o), Players(to)]) => {
            retarget(o, T::ToPlayers(to.clone()))
        }
        _ => return None,
    })
}

/// A reviewed row that makes a card a later reference can read.
pub fn makes_card(operation: &Operation) -> bool {
    matches!(
        operation,
        Operation::Conjure { .. } | Operation::DraftFromSpellbook { .. }
    ) || matches!(operation, Operation::Primitive { name, .. }
            if name.starts_with("Action::Conjure") || name.starts_with("Action::Draft"))
}

/// The kind of value a variable form reads, when it is the one chosen.
fn chosen_kind(operand: &Operand) -> Option<ValueKind> {
    let Operand::Term { form, .. } = operand else {
        return None;
    };
    match form.as_str() {
        "TheChosenCreatureType" | "TheChosenCreatureTypes" => Some(ValueKind::CreatureType),
        "TheChosenLandType" => Some(ValueKind::LandType),
        "TheChosenCardtype" => Some(ValueKind::CardType),
        _ => None,
    }
}

/// A row's filter in generic form, or the row itself when it has none.
/// Keywords a filter asks about as a family, whatever their cost or number.
const KEYWORD_FAMILIES: &[(&str, KeywordFamily)] = &[
    ("AnyFlashback", KeywordFamily::Flashback),
    ("AnySuspend", KeywordFamily::Suspend),
    ("AnyMorph", KeywordFamily::Morph),
    ("AnyForetell", KeywordFamily::Foretell),
    ("Convoke", KeywordFamily::Convoke),
    ("AnyMutate", KeywordFamily::Mutate),
    ("AnyDisturb", KeywordFamily::Disturb),
    ("Cascade", KeywordFamily::Cascade),
    ("AnyFreerunning", KeywordFamily::Freerunning),
    ("AnyKicker", KeywordFamily::Kicker),
    ("AnyUnearth", KeywordFamily::Unearth),
    ("Decayed", KeywordFamily::Decayed),
    ("AnyModular", KeywordFamily::Modular),
    ("AnyCumulativeUpkeep", KeywordFamily::CumulativeUpkeep),
    ("AnyWarp", KeywordFamily::Warp),
    ("Soulbond", KeywordFamily::Soulbond),
    ("AnyAwaken", KeywordFamily::Awaken),
    ("AnyFading", KeywordFamily::Fading),
    ("AnyRampage", KeywordFamily::Rampage),
    ("AnyBlitz", KeywordFamily::Blitz),
    ("LevelUp", KeywordFamily::LevelUp),
    ("AnyMadness", KeywordFamily::Madness),
    ("Disguise", KeywordFamily::Disguise),
];

pub fn reduce_filter(name: String, operands: Vec<Operand>) -> ObjectFilter {
    let form = name.split("::").nth(1).unwrap_or(&name);
    if name.starts_with("CheckHasable::") && operands.is_empty() {
        let family = KEYWORD_FAMILIES.iter().find(|(row, _)| *row == form);
        if let Some((_, family)) = family {
            return ObjectFilter::HasKeywordFamily(family.clone());
        }
    }
    if let ("SharesAColorWithPermanent", [Operand::Object(object)]) = (form, operands.as_slice()) {
        return ObjectFilter::SharesColorWith(object.clone());
    }
    if form == "IsntPrepared" && operands.is_empty() {
        return ObjectFilter::Not(Box::new(ObjectFilter::State(ObjectState::Prepared)));
    }
    let nth = |ordinal: &Expr, spells: Option<&ObjectFilter>, by: Option<&PlayerRef>, scope| {
        ObjectFilter::NthCast {
            ordinal: ordinal.clone(),
            spells: Box::new(spells.cloned().unwrap_or(ObjectFilter::Any)),
            by: by.map(|player| PlayerSet::One(player.clone())),
            scope,
        }
    };
    match (form, operands.as_slice()) {
        ("WasKickedWithKicker", kicker) => return ObjectFilter::KickedWith(kicker.to_vec()),
        ("ManaFromAPermanentWasSpentToCastIt", [Operand::Objects(objects)]) => {
            if let Some(sources) = event_filter(objects) {
                return ObjectFilter::ManaSpentFrom(Box::new(sources));
            }
        }
        ("ManaFromATeasureWasSpentToCastIt" | "ManaFromTeasureWasSpentToCast", []) => {
            return ObjectFilter::ManaSpentFrom(Box::new(ObjectFilter::Subtype(
                crate::subtype::artifact_type(&crate::mtg_types::ArtifactType::Treasure),
            )))
        }
        ("IsSuspended", []) => return ObjectFilter::State(ObjectState::Suspended),
        ("HasASticker", []) => return ObjectFilter::State(ObjectState::HasSticker),
        ("PowerIsLessThanToughness", []) => {
            return ObjectFilter::State(ObjectState::PowerBelowToughness)
        }
        ("IsTransformed", []) => return ObjectFilter::State(ObjectState::Transformed),
        ("HasNoAbilities", []) => return ObjectFilter::State(ObjectState::NoAbilities),
        ("ControlledSinceBeginningOfTurn", []) => {
            return ObjectFilter::State(ObjectState::ControlledSinceTurnBegan)
        }
        ("NotControlledSinceBeginningOfTurn", []) => {
            return ObjectFilter::Not(Box::new(ObjectFilter::State(
                ObjectState::ControlledSinceTurnBegan,
            )))
        }
        ("TargetsOnlyASingleTarget", []) => return ObjectFilter::OnStack(StackTrait::SingleTarget),
        ("IsCreatureTypeVariable" | "IsLandTypeVariable", [variable])
            if chosen_kind(variable).is_some() =>
        {
            return ObjectFilter::Chosen(chosen_kind(variable).unwrap())
        }
        ("ManaAmountOfTypeWasSpentToCastIt", [Operand::Comparison(comparison, value), color])
            if simple_color(color).is_some() =>
        {
            return ObjectFilter::Condition(Box::new(Condition::Compare {
                left: Expr::ManaSpent {
                    object: ObjectRef::Candidate,
                    filter: SpentManaFilter {
                        colors: Some(vec![simple_color(color).unwrap()]),
                        snow: false,
                    },
                    aggregation: SpentManaAggregation::Count,
                },
                comparison: comparison.clone(),
                right: value.clone(),
            }))
        }
        ("HasAnAdventure", []) => return ObjectFilter::State(ObjectState::HasAdventure),
        ("BlockedThisTurn", []) => return ObjectFilter::State(ObjectState::BlockedThisTurn),
        ("IsFaceUp", []) => {
            return ObjectFilter::Not(Box::new(ObjectFilter::State(ObjectState::FaceDown)))
        }
        ("HasASingleTarget", []) => return ObjectFilter::OnStack(StackTrait::SingleTarget),
        ("TargetsOnlySinglePermanent", [Operand::Object(object)]) => {
            return ObjectFilter::All(vec![
                ObjectFilter::OnStack(StackTrait::SingleTarget),
                ObjectFilter::TargetsAny(Box::new(Recipients::Objects(ObjectSet::One(
                    object.clone(),
                )))),
            ])
        }
        ("TargetsOnlyASinglePermanent", [Operand::Objects(objects)]) => {
            return ObjectFilter::All(vec![
                ObjectFilter::OnStack(StackTrait::SingleTarget),
                ObjectFilter::TargetsAny(Box::new(Recipients::Objects(objects.clone()))),
            ])
        }
        ("AttachedToPlayer", [Operand::Player(player)]) => {
            return ObjectFilter::AttachedToPlayer(player.clone())
        }
        ("APermanentWithTheHighestManaValue", [Operand::Objects(objects)]) => {
            return ObjectFilter::Extreme {
                among: Box::new(objects.clone()),
                property: Property::ManaValue,
                highest: true,
            }
        }
        ("SharesANameWithAnExiled", [Operand::Objects(objects)]) => {
            return ObjectFilter::SharesNameWithAny(Box::new(objects.clone()))
        }
        ("IsEnchantedByAPermanent", [Operand::Objects(objects)])
            if event_filter(objects).is_some() =>
        {
            return ObjectFilter::HasAttached(Box::new(event_filter(objects).unwrap()))
        }
        ("TheNthSpellCastThisTurn", [Operand::Number(ordinal)]) => {
            return nth(ordinal, None, None, CastScope::Turn)
        }
        (
            "TheNthSpellCastByPlayerThisTurn",
            [Operand::Number(ordinal), Operand::Filter(spells), Operand::Player(player)],
        ) => return nth(ordinal, Some(spells), Some(player), CastScope::Turn),
        (
            "TheNthSpellCastByPlayerThisGame",
            [Operand::Number(ordinal), Operand::Filter(spells), Operand::Player(player)],
        ) => return nth(ordinal, Some(spells), Some(player), CastScope::Game),
        _ => {}
    }
    match (name.as_str(), operands.as_slice()) {
        (
            "Target::TargetAbility"
            | "Target::UptoOneTargetAbility"
            | "Target::TargetSpellOrAbility"
            | "Target::UptoNumberTargetSpellsOrAbilities",
            [.., stack],
        ) => {
            if let Some(filter) = stack_filter(stack) {
                return filter;
            }
        }
        ("Color::TheChosenColor", []) => return ObjectFilter::Chosen(ValueKind::Color),
        (
            "NameFilter::TheChosenName"
            | "NameFilter::TheChosenCardName"
            | "NameFilter::OneOfTheChosenNames",
            [],
        ) => return ObjectFilter::Chosen(ValueKind::CardName),
        (_, [variable]) if form.ends_with("TypeVariable") || form == "IsCardtypeVariable" => {
            if let Some(kind) = chosen_kind(variable) {
                return ObjectFilter::Chosen(kind);
            }
        }
        _ => {}
    }
    let state = match form {
        "IsEnchanted" => Some(ObjectState::Enchanted),
        "IsEquipped" => Some(ObjectState::Equipped),
        "IsSaddled" => Some(ObjectState::Saddled),
        "IsACommander" => Some(ObjectState::Commander),
        "WasKicked" => Some(ObjectState::Kicked),
        "IsFaceDown" => Some(ObjectState::FaceDown),
        "IsMonstrous" => Some(ObjectState::Monstrous),
        "IsRenowned" => Some(ObjectState::Renowned),
        "IsUnblocked" => Some(ObjectState::Unblocked),
        "IsBlocked" => Some(ObjectState::Blocked),
        "IsAttackingAlone" => Some(ObjectState::AttackingAlone),
        "WasBargained" => Some(ObjectState::Bargained),
        "Escaped" => Some(ObjectState::Escaped),
        "IsDoubleFaced" => Some(ObjectState::DoubleFaced),
        "PowerIsGreaterThanBasePower" => Some(ObjectState::PowerAboveBase),
        "WasCastForItsWarpCost" => Some(ObjectState::Warped),
        "HasBeenGoaded" => Some(ObjectState::Goaded),
        "IsPaired" => Some(ObjectState::Paired),
        "IsSuspected" => Some(ObjectState::Suspected),
        "IsDamaged" => Some(ObjectState::Damaged),
        "TributeWasntPaid" => Some(ObjectState::TributeNotPaid),
        "WasCastByPlayerDuringTheirMainPhase" => Some(ObjectState::CastDuringOwnMainPhase),
        "HasXInManaCost" | "HasXInCost" => Some(ObjectState::HasX),
        _ => None,
    };
    match (state, form, operands.as_slice()) {
        (Some(state), _, []) => ObjectFilter::State(state),
        (_, "IsYourCommander", []) => ObjectFilter::All(vec![
            ObjectFilter::State(ObjectState::Commander),
            ObjectFilter::OwnedBy(PlayerSet::One(PlayerRef::AbilityController)),
        ]),
        (_, "IsntEnchanted", []) => {
            ObjectFilter::Not(Box::new(ObjectFilter::State(ObjectState::Enchanted)))
        }
        // An outlaw is any of these five creature types.
        (_, "IsAnOutlaw", []) => {
            use crate::mtg_types::CreatureType as C;
            ObjectFilter::Either(
                [C::Assassin, C::Mercenary, C::Pirate, C::Rogue, C::Warlock]
                    .iter()
                    .map(|ty| ObjectFilter::Subtype(crate::subtype::creature_type(ty)))
                    .collect(),
            )
        }
        (_, "AttachedToPermanent", [Operand::Object(object)]) => {
            ObjectFilter::AttachedTo(Box::new(ObjectFilter::SameObject(object.clone())))
        }
        (_, "AttachedToAPermanent", [Operand::Objects(objects)]) => match event_filter(objects) {
            Some(filter) => ObjectFilter::AttachedTo(Box::new(filter)),
            None => ObjectFilter::Primitive { name, operands },
        },
        (_, "IsBlockingAttacker", [Operand::Object(object)]) => {
            ObjectFilter::BlockingObject(object.clone())
        }
        (_, "IsBlockedByDefender", [Operand::Object(object)]) => {
            ObjectFilter::BlockedByObject(object.clone())
        }
        (_, "IsAttackingPlayer", [Operand::Player(player)]) => ObjectFilter::AttackingDefender(
            Box::new(AttackDefenders::Players(PlayerSet::One(player.clone()))),
        ),
        (_, "IsAttackingAPlayer", [Operand::Players(players)]) => {
            ObjectFilter::AttackingDefender(Box::new(AttackDefenders::Players(players.clone())))
        }
        _ => reduce_object_filter(form, &operands)
            .unwrap_or(ObjectFilter::Primitive { name, operands }),
    }
}

include!("reduce_objects.rs");
include!("reduce_numbers.rs");
include!("reduce_players.rs");
include!("reduce_rules.rs");
include!("reduce_actions.rs");
include!("reduce_events.rs");

/// A color word's mana color.
fn simple_color(operand: &Operand) -> Option<ManaColor> {
    let Operand::Vocabulary { form, .. } = operand else {
        return None;
    };
    match form.as_str() {
        "White" => Some(ManaColor::White),
        "Blue" => Some(ManaColor::Blue),
        "Black" => Some(ManaColor::Black),
        "Red" => Some(ManaColor::Red),
        "Green" => Some(ManaColor::Green),
        _ => None,
    }
}

/// A row that compares a number the AST can already state.
fn reduce_comparison(name: &str, operands: &[Operand]) -> Option<Condition> {
    use Operand::{Comparison as Than, Objects, Player};
    let controlled = |player: &PlayerRef, objects: &ObjectSet| {
        Some(ObjectSet::Battlefield(ObjectFilter::All(vec![
            event_filter(objects)?,
            ObjectFilter::ControlledBy(PlayerSet::One(player.clone())),
        ])))
    };
    let count = |objects: ObjectSet| Expr::CountObjects(Box::new(objects));
    let compare = |left, comparison: &Comparison, right: &Expr| {
        Some(Condition::Compare {
            left,
            comparison: comparison.clone(),
            right: right.clone(),
        })
    };
    match (name, operands) {
        (
            "Players::HasHighestNumberOfCardsInHandAmongPlayers",
            [Player(p), Operand::Players(players)],
        ) => compare(
            Expr::PlayerNumber {
                player: p.clone(),
                number: PlayerNumber::CardsInHand,
            },
            &Comparison::Greater,
            &among(players, PlayerNumber::CardsInHand, Aggregation::Maximum),
        ),
        (
            "Players::ControlsMorePermanentsThanEachPlayer",
            [Player(p), Operand::Players(players), Objects(s)],
        ) => compare(
            count(controlled(p, s)?),
            &Comparison::Greater,
            &among(
                players,
                PlayerNumber::Permanents(Box::new(event_filter(s)?)),
                Aggregation::Maximum,
            ),
        ),
        ("Players::ControlsLessPermanentThanPlayer", [Player(p), Player(other), Objects(s)]) => {
            compare(
                count(controlled(p, s)?),
                &Comparison::Less,
                &count(controlled(other, s)?),
            )
        }
        ("Players::SacrificedAPermanentThisTurn", [Player(p), Objects(s)]) => compare(
            Expr::EventHistory {
                event: Box::new(EventSpec::Player {
                    event: PlayerEvent::Sacrifices,
                    players: PlayerSet::One(p.clone()),
                    objects: Some(event_filter(s)?),
                }),
                window: HistoryWindow::ThisTurn,
                measure: HistoryMeasure::Occurrences,
            },
            &Comparison::GreaterOrEqual,
            &Expr::Constant(1),
        ),
        ("Players::OwnsAndControlsA" | "Players::OwnsAPermanent", [Player(p), Objects(s)]) => {
            let mut filter = vec![
                event_filter(s)?,
                ObjectFilter::OwnedBy(PlayerSet::One(p.clone())),
            ];
            if name == "Players::OwnsAndControlsA" {
                filter.push(ObjectFilter::ControlledBy(PlayerSet::One(p.clone())));
            }
            compare(
                count(ObjectSet::Battlefield(ObjectFilter::All(filter))),
                &Comparison::GreaterOrEqual,
                &Expr::Constant(1),
            )
        }
        ("Players::OwnsACardInExile", [Player(p), Objects(s)]) => compare(
            count(ObjectSet::Matching {
                objects: Box::new(s.clone()),
                filter: ObjectFilter::OwnedBy(PlayerSet::One(p.clone())),
            }),
            &Comparison::GreaterOrEqual,
            &Expr::Constant(1),
        ),
        ("Condition::NumCardsInExileIs", [Than(c, v), Objects(s)]) => {
            compare(count(s.clone()), c, v)
        }
        ("Condition::TotalToughnessOfPermanentsIs", [Than(c, v), Objects(s)]) => {
            compare(aggregate(s, Property::Toughness, Aggregation::Sum), c, v)
        }
        ("Condition::ThereAreNumberCardTypesInPlayersGraveyard", [Than(c, v), Player(p)]) => {
            compare(
                distinct(
                    &graveyard_of(p, ObjectFilter::Any),
                    DistinctProperty::CardTypes,
                ),
                c,
                v,
            )
        }
        (
            "Condition::NumDifferentManaValueAmongCardsInPlayersGraveyardIs",
            [Than(c, v), Operand::Filter(cards), Player(p)],
        ) => compare(
            distinct(
                &graveyard_of(p, cards.clone()),
                DistinctProperty::ManaValues,
            ),
            c,
            v,
        ),
        ("Condition::NumDifferentManaValuesAmongCardsInExileIs", [Than(c, v), Objects(s)]) => {
            compare(distinct(s, DistinctProperty::ManaValues), c, v)
        }
        ("Condition::ThereAreNumberCardTypesAmongExiled", [Than(c, v), Objects(s)]) => {
            compare(distinct(s, DistinctProperty::CardTypes), c, v)
        }
        ("Players::ControlsMorePermanentThanPlayer", [Player(p), Player(other), Objects(s)]) => {
            compare(
                count(controlled(p, s)?),
                &Comparison::Greater,
                &count(controlled(other, s)?),
            )
        }
        ("Players::DevotionToColorsIs", [Player(p), Operand::List(colors), Than(c, v)]) => compare(
            Expr::CountManaSymbols {
                objects: Box::new(ObjectSet::Battlefield(ObjectFilter::ControlledBy(
                    PlayerSet::One(p.clone()),
                ))),
                colors: colors.iter().map(simple_color).collect::<Option<_>>()?,
            },
            c,
            v,
        ),
        ("Players::ControlsNumWithDifferentPowers", [Player(p), Than(c, v), Objects(s)]) => {
            compare(
                Expr::CountDistinct {
                    objects: Box::new(controlled(p, s)?),
                    property: DistinctProperty::Powers,
                },
                c,
                v,
            )
        }
        ("Players::ControlsNumWithDifferentNames", [Player(p), Than(c, v), Objects(s)]) => compare(
            Expr::CountDistinct {
                objects: Box::new(controlled(p, s)?),
                property: DistinctProperty::Names,
            },
            c,
            v,
        ),
        ("Players::NumCardsInLibraryIs", [Player(p), Than(c, v)]) => compare(
            count(ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Library,
                    players: PlayerSet::One(p.clone()),
                },
                kind: ObjectKind::Card,
                filter: ObjectFilter::Any,
            }),
            c,
            v,
        ),
        ("Players::NumOpponentsIs", [Player(p), Than(c, v)]) => {
            compare(Expr::CountPlayers(PlayerSet::OpponentsOf(p.clone())), c, v)
        }
        ("Condition::TotalPowerOfPermanentsIs", [Than(c, v), Objects(s)]) => compare(
            Expr::Aggregate {
                objects: Box::new(s.clone()),
                property: Property::Power,
                aggregation: Aggregation::Sum,
            },
            c,
            v,
        ),
        ("Condition::TimesThisAbilityHasResolvedThisTurnIs", [Than(c, v)]) => {
            compare(Expr::TimesResolvedThisTurn, c, v)
        }
        _ => None,
    }
}

/// A row's condition in generic form, or the row itself when it has none.
pub fn reduce_condition(name: String, operands: Vec<Operand>) -> Condition {
    if let Some(condition) = name
        .strip_prefix("Condition::")
        .and_then(|form| reduce_event_condition(form, &operands))
    {
        return condition;
    }
    match name.as_str() {
        "Condition::TheGiftWasPromised" => return Condition::GiftPromised(true),
        "Condition::TheGiftWasntPromised" => return Condition::GiftPromised(false),
        "Condition::ItsNeitherDayOrNight" => return Condition::ItsNeitherDayNorNight,
        "Condition::ItsNight" => return Condition::ItsNight,
        _ => {}
    }
    match (name.as_str(), operands.as_slice()) {
        ("Condition::TheChosenNumbersMatch", []) => {
            return Condition::Compare {
                left: among(
                    &PlayerSet::All,
                    PlayerNumber::ChosenNumber,
                    Aggregation::Maximum,
                ),
                comparison: Comparison::Equal,
                right: among(
                    &PlayerSet::All,
                    PlayerNumber::ChosenNumber,
                    Aggregation::Minimum,
                ),
            }
        }
        ("Condition::ASpellWasWarpedThisTurn", []) => {
            return Condition::Compare {
                left: Expr::EventHistory {
                    event: Box::new(EventSpec::Spell {
                        event: SpellEvent::Cast,
                        players: PlayerSet::All,
                        spells: ObjectFilter::State(ObjectState::Warped),
                    }),
                    window: HistoryWindow::ThisTurn,
                    measure: HistoryMeasure::Occurrences,
                },
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(1),
            }
        }
        ("Condition::TheChosenWordWas", [Operand::Text(word)]) => {
            return Condition::ChosenWord(word.clone())
        }
        ("Condition::TriggerDiceResultIs", [Operand::Comparison(comparison, value)]) => {
            return Condition::Compare {
                left: Expr::EventAmount,
                comparison: comparison.clone(),
                right: value.clone(),
            }
        }
        (
            "Condition::TheCardInHandPassesFilter",
            [Operand::Object(object), Operand::Filter(filter)],
        ) => {
            return Condition::ObjectMatches {
                object: object.clone(),
                filter: filter.clone(),
            }
        }
        (
            "Players::AttackedPlayerThisTurn",
            [Operand::Player(attacker), Operand::Player(defender)],
        ) => {
            return Condition::Compare {
                left: Expr::EventHistory {
                    event: Box::new(EventSpec::Attack {
                        attackers: ObjectFilter::Any,
                        by: Some(PlayerSet::One(attacker.clone())),
                        defenders: AttackDefenders::Players(PlayerSet::One(defender.clone())),
                    }),
                    window: HistoryWindow::ThisTurn,
                    measure: HistoryMeasure::Occurrences,
                },
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(1),
            }
        }
        ("Players::IsAttackingPlayer", [Operand::Player(attacker), Operand::Player(defender)]) => {
            return Condition::PlayerAttacks {
                attacker: attacker.clone(),
                defender: defender.clone(),
            }
        }
        ("Condition::ActivatedAbilityPassesFilter", [Operand::Object(object), ability]) => {
            if let Some(filter) = stack_filter(ability) {
                return Condition::ObjectMatches {
                    object: object.clone(),
                    filter,
                };
            }
        }
        (
            "Condition::ACardLeftPlayersGraveyardThisTurn",
            [Operand::Filter(cards), Operand::Player(player)],
        ) => {
            return Condition::Compare {
                left: Expr::EventHistory {
                    event: Box::new(EventSpec::Object {
                        event: ObjectEvent::LeavesGraveyard,
                        objects: ObjectFilter::All(vec![
                            cards.clone(),
                            ObjectFilter::OwnedBy(PlayerSet::One(player.clone())),
                        ]),
                    }),
                    window: HistoryWindow::ThisTurn,
                    measure: HistoryMeasure::Occurrences,
                },
                comparison: Comparison::GreaterOrEqual,
                right: Expr::Constant(1),
            }
        }
        _ => {}
    }
    if let ("Players::OwnsAndControls", [Operand::Player(player), Operand::Object(object)]) =
        (name.as_str(), operands.as_slice())
    {
        let them = || PlayerSet::One(player.clone());
        return Condition::ObjectMatches {
            object: object.clone(),
            filter: ObjectFilter::All(vec![
                ObjectFilter::OwnedBy(them()),
                ObjectFilter::ControlledBy(them()),
            ]),
        };
    }
    if let ("Condition::CardIsExiled", [Operand::Object(object)]) =
        (name.as_str(), operands.as_slice())
    {
        return Condition::ObjectMatches {
            object: object.clone(),
            filter: ObjectFilter::InZone(ZoneScope::Shared(EventZone::Exile)),
        };
    }
    match (name.as_str(), operands.as_slice()) {
        ("Condition::MostVotesForWordIs", [Operand::Text(word)]) => {
            return Condition::MostVotes {
                option: VoteOption::Word(word.clone()),
                tied: false,
            }
        }
        ("Condition::MostVotesOrTiedForMostVotesForWordIs", [Operand::Text(word)]) => {
            return Condition::MostVotes {
                option: VoteOption::Word(word.clone()),
                tied: true,
            }
        }
        ("Condition::WordWasVotedFor", [Operand::Text(word)]) => {
            return Condition::Voted(VoteOption::Word(word.clone()))
        }
        ("Players::YourTeam", [Operand::Player(player)]) => {
            return Condition::SameTeam {
                player: player.clone(),
                other: PlayerRef::AbilityController,
            }
        }
        (
            "Players::VotedForWord" | "Players::ChoseWord",
            [Operand::Player(player), Operand::Text(word)],
        ) => {
            return Condition::VotedFor {
                player: player.clone(),
                option: VoteOption::Word(word.clone()),
            }
        }
        _ => {}
    }
    if let Some(condition) = reduce_comparison(&name, &operands) {
        return condition;
    }
    let window = match name.as_str() {
        "Condition::IsBeforeAttackersDeclared" => Some(TurnWindow::BeforeAttackersDeclared),
        "Condition::IsDuringCombat" => Some(TurnWindow::Combat),
        "Condition::IsDuringUpkeep" => Some(TurnWindow::Upkeep),
        "Condition::IsDuringEndStep" => Some(TurnWindow::EndStep),
        "Condition::IsBeforeCombatDamageStep" => Some(TurnWindow::BeforeCombatDamage),
        "Condition::IsDuringDeclareAttackersStep" => Some(TurnWindow::DeclareAttackers),
        "Condition::IsDuringDeclareBlockersStep" => Some(TurnWindow::DeclareBlockers),
        "Condition::IsBeforeBlockersDeclared" => Some(TurnWindow::BeforeBlockersDeclared),
        "Condition::IsFirstCombatPhaseOfTurn" => Some(TurnWindow::FirstCombat),
        "Condition::IsAfterBlockersAreDeclared" => Some(TurnWindow::AfterBlockersDeclared),
        _ => None,
    };
    let state = match name.as_str() {
        "Players::IsTheMonarch" => Some((PlayerState::Monarch, false)),
        "Players::HasTheCitysBlessing" => Some((PlayerState::CitysBlessing, false)),
        "Players::HasAFullParty" => Some((PlayerState::FullParty, false)),
        "Players::IsNotTheStartingPlayer" => Some((PlayerState::StartingPlayer, true)),
        "Players::IsTheirUpkeep" => Some((PlayerState::InTheirUpkeep, false)),
        "Players::IsTheirMainPhase" => Some((PlayerState::InTheirMainPhase, false)),
        "Players::CompletedADungeon" => Some((PlayerState::CompletedDungeon, false)),
        "Players::HasTheInitiative" => Some((PlayerState::Initiative, false)),
        "Players::Descended" => Some((PlayerState::Descended, false)),
        "Players::Poisoned" => Some((PlayerState::Poisoned, false)),
        "Players::IsTheStartingPlayer" => Some((PlayerState::StartingPlayer, false)),
        "Players::CommitedACrimeThisTurn" => Some((PlayerState::CommittedCrime, false)),
        "Players::IsAttacked" => Some((PlayerState::BeingAttacked, false)),
        "Players::HasAnEnduringStory" => Some((PlayerState::EnduringStory, false)),
        _ => None,
    };
    match (window, state, operands.as_slice()) {
        (Some(window), _, []) => Condition::Window(window),
        (_, Some((state, negated)), [Operand::Player(player)]) => {
            let held = Condition::PlayerState {
                player: player.clone(),
                state,
            };
            if negated {
                Condition::Not(Box::new(held))
            } else {
                held
            }
        }
        _ => reduce_player_condition(&name, &operands)
            .unwrap_or(Condition::Primitive { name, operands }),
    }
}

/// A row's change in generic form, or the row itself when it has none.
pub fn reduce_change(name: String, operands: Vec<Operand>) -> Change {
    match (name.as_str(), operands.as_slice()) {
        ("LayerEffect::SwitchPT", []) => Change::SwitchPowerToughness,
        (
            "LayerEffect::AddCreatureTypeVariable" | "LayerEffect::AddLandTypeVariable",
            [variable],
        ) if chosen_kind(variable).is_some() => Change::ChosenType {
            kind: chosen_kind(variable).unwrap(),
            mode: ChangeMode::Add,
        },
        (
            "LayerEffect::SetCreatureTypeVariable" | "LayerEffect::SetLandTypeVariable",
            [variable],
        ) if chosen_kind(variable).is_some() => Change::ChosenType {
            kind: chosen_kind(variable).unwrap(),
            mode: ChangeMode::Set,
        },
        ("LayerEffect::LosesAbility", [ability]) => Change::LosesAbility(Box::new(ability.clone())),
        ("LayerEffect::SetLandType", [land]) => Change::SetLandType(Box::new(land.clone())),
        ("LayerEffect::SetName", [Operand::Text(name)]) => Change::SetName(name.clone()),
        ("LayerEffect::AddAbilityAndLoseAllOtherAbilities", [Operand::Ability(ability)]) => {
            Change::OnlyAbility(ability.clone())
        }
        ("LayerEffect::IsACopyOfExiledCard", [Operand::Object(object), effects]) => {
            Change::CopyOf {
                object: object.clone(),
                effects: Box::new(effects.clone()),
            }
        }
        ("SpellEffect::Convoke", []) => Change::GrantAbility(Box::new(Ability::Mechanic {
            mechanic: Mechanic::Convoke,
            cost: None,
            amount: None,
            filter: None,
        })),
        ("SpellEffect::CantBeCast", []) => Change::Rule(RuleChange::Prohibit(ObjectAction::Cast)),
        (
            "LayerEffect::IsACopyOfLibraryCard",
            [Operand::Objects(ObjectSet::Selected(result)), effects],
        ) => Change::CopyOf {
            object: ObjectRef::Result {
                result: result.clone(),
                phase: ObjectPhase::After,
                index: 0,
            },
            effects: Box::new(effects.clone()),
        },
        ("LayerEffect::IsACopyOfGraveyardCard", [Operand::Object(object), effects]) => {
            Change::CopyOf {
                object: object.clone(),
                effects: Box::new(effects.clone()),
            }
        }
        (
            "StaticLayerEffect::AddAbilityFromCardsInGraveyardsHasable",
            [Operand::Filter(cards), kinds],
        ) => Change::GainAbilitiesOf {
            objects: ObjectSet::Zone {
                scope: ZoneScope::Players {
                    zone: EventZone::Graveyard,
                    players: PlayerSet::All,
                },
                kind: ObjectKind::Card,
                filter: cards.clone(),
            },
            kinds: Box::new(kinds.clone()),
        },
        (
            "LayerEffect::AddAbilityFromPermanentHasable"
            | "LayerEffect::AddAbilityFromGraveyardCardHasable"
            | "StaticLayerEffect::AddAbilityFromExiledHasable",
            [Operand::Object(object), kinds],
        ) => Change::GainAbilitiesOf {
            objects: ObjectSet::One(object.clone()),
            kinds: Box::new(kinds.clone()),
        },
        (
            "LayerEffect::AddAbilityFromEachPermanentHasable"
            | "StaticLayerEffect::AddAbilityFromAnExiledHasable",
            [Operand::Objects(objects), kinds],
        ) => Change::GainAbilitiesOf {
            objects: objects.clone(),
            kinds: Box::new(kinds.clone()),
        },
        (
            "LayerEffect::ReplaceColorWordVariableWithNewColorWordVariable"
            | "LayerEffect::ReplaceLandTypeVariableWithNewLandTypeVariable",
            [from, to],
        ) => Change::ReplaceWord {
            from: Box::new(from.clone()),
            to: Box::new(to.clone()),
        },
        ("LayerEffect::AddEnchantmentType", [kind]) => {
            Change::AddEnchantmentType(Box::new(kind.clone()))
        }
        ("LayerEffect::IsACopyOfPermanent", [Operand::Object(object), effects]) => Change::CopyOf {
            object: object.clone(),
            effects: Box::new(effects.clone()),
        },
        _ => Change::Primitive { name, operands },
    }
}

/// A row's cost in generic form, or the row itself.
pub fn reduce_cost(name: String, operands: Vec<Operand>) -> CostPart {
    let keyword = |action, amount: Option<&Expr>| CostPart::Keyword {
        action,
        amount: amount.cloned(),
    };
    let bare = name.strip_prefix("Cost::").unwrap_or(&name);
    match (bare, operands.as_slice()) {
        ("PutAnExiledCardIntoOwnersGraveyard", [Operand::Objects(cards)]) => {
            CostPart::ExiledToGraveyard(cards.clone())
        }
        ("CollectEvidence", [Operand::Number(amount)]) => {
            keyword(CostKeyword::CollectEvidence, Some(amount))
        }
        ("Blight", [Operand::Number(amount)]) => keyword(CostKeyword::Blight, Some(amount)),
        ("BlightX", []) => keyword(CostKeyword::Blight, Some(&Expr::AnnouncedX)),
        ("Forage", []) => keyword(CostKeyword::Forage, None),
        ("DiscardHand", []) => CostPart::DiscardHand,
        ("RevealCardFromHand", [Operand::Vocabulary { form, .. }]) if form == "ThisCardInHand" => {
            CostPart::Reveal {
                count: Count::Exactly(Expr::Constant(1)),
                filter: ObjectFilter::SameObject(ObjectRef::Source),
                random: false,
            }
        }
        ("ExertPermanent", [Operand::Object(object)]) => CostPart::Exert(object.clone()),
        ("UnattachPermanent", [Operand::Object(object)]) => CostPart::Unattach(object.clone()),
        ("UnspecializeGraveyardCard", [Operand::Object(object)]) => {
            CostPart::Unspecialize(object.clone())
        }
        ("BeholdAndExile", [what]) => CostPart::BeholdAndExile(Box::new(what.clone())),
        ("BeholdA", [what]) => CostPart::Behold {
            count: Expr::Constant(1),
            what: Box::new(what.clone()),
        },
        ("BeholdNumber", [Operand::Number(count), what]) => CostPart::Behold {
            count: count.clone(),
            what: Box::new(what.clone()),
        },
        ("RevealTopCardOfLibraryOfType", [Operand::Filter(filter)]) => {
            CostPart::RevealTop(filter.clone())
        }
        ("PayManaCostOfPermanent" | "PayManaCostOfSpell", [Operand::Object(object)]) => {
            CostPart::ManaCostOf(object.clone())
        }
        _ => match reduce_cost_part(bare, &operands) {
            Some(part) => part,
            None => CostPart::Primitive { name, operands },
        },
    }
}

/// A row's rule about an object in generic form, or the row itself.
pub fn reduce_rule(name: String, operands: Vec<Operand>) -> RuleChange {
    let not_spell = || ObjectFilter::Not(Box::new(ObjectFilter::OnStack(StackTrait::Spell)));
    let bare = name.strip_prefix("PermanentRule::").unwrap_or(&name);
    match (bare, operands.as_slice()) {
        ("IsGoadedByPlayer", [Operand::Player(player)]) => RuleChange::GoadedBy(player.clone()),
        ("CantAttackIfDefendingPlayer", [Operand::Condition(condition)]) => {
            RuleChange::CantAttackDefenderIf(condition.clone())
        }
        ("TheLegendsRuleDoesntApply", []) => RuleChange::LegendRuleExempt,
        ("CantBeTheTargetOfSpells", [Operand::Filter(spells)]) => {
            RuleChange::Prohibit(ObjectAction::BeTargeted(ObjectFilter::All(vec![
                ObjectFilter::OnStack(StackTrait::Spell),
                spells.clone(),
            ])))
        }
        ("CantBeTheTargetOfAbilities", [abilities]) if stack_filter(abilities).is_some() => {
            RuleChange::Prohibit(ObjectAction::BeTargeted(ObjectFilter::All(vec![
                not_spell(),
                stack_filter(abilities).unwrap(),
            ])))
        }
        ("CantBeTheTargetOfSpellsOrAbilities", [sources]) if stack_filter(sources).is_some() => {
            RuleChange::Prohibit(ObjectAction::BeTargeted(stack_filter(sources).unwrap()))
        }
        ("CantHaveCountersPutOnIt", []) => {
            RuleChange::Prohibit(ObjectAction::ReceiveCounters(None))
        }
        ("CantHaveCountersOfTypePutOnIt", [kind]) if counter_kind(kind).is_some() => {
            RuleChange::Prohibit(ObjectAction::ReceiveCounters(counter_kind(kind)))
        }
        ("CantTransform", []) => RuleChange::Prohibit(ObjectAction::Transform),
        ("CantPhaseOut", []) => RuleChange::Prohibit(ObjectAction::PhaseOut),
        ("CantCrew", []) => RuleChange::Prohibit(ObjectAction::Crew),
        ("CrewsVehiclesAsThoughPowerWereGreater", [Operand::Number(bonus)]) => {
            RuleChange::TapPowerBonus {
                cost: TapPowerCost::Crew,
                bonus: bonus.clone(),
            }
        }
        ("SaddlesMountsAsThoughPowerWereGreater", [Operand::Number(bonus)]) => {
            RuleChange::TapPowerBonus {
                cost: TapPowerCost::Saddle,
                bonus: bonus.clone(),
            }
        }
        ("MayBeExertedAsItAttacks", []) => RuleChange::MayExert(None),
        ("MayBeExertedAsItAttacksWithTrigger", [then]) => {
            RuleChange::MayExert(Some(Box::new(then.clone())))
        }
        ("CanBeBlockedAsThoughItDidntHave", [ability]) => {
            RuleChange::BlockedAsThoughLacking(Box::new(ability.clone()))
        }
        ("CantGainAbility", [ability]) => RuleChange::CantGainAbility(Box::new(ability.clone())),
        _ => match reduce_object_static(bare, &operands) {
            Some(rule) => rule,
            None => RuleChange::Primitive { name, operands },
        },
    }
}

/// A row's rule about a player in generic form, or the row itself.
pub fn reduce_player_rule(name: String, operands: Vec<Operand>) -> PlayerRule {
    let permission =
        |cards: ObjectSet, from, filter, cost, into_exile| PlayerRule::PlayPermissionWith {
            permission: Box::new(PlayerRule::PlayPermission {
                cards,
                from,
                action: CardPlayAction::CastSpell,
                filter,
            }),
            terms: PlayTerms {
                limit: None,
                during: None,
                cost,
                spend_as: None,
                into_exile,
                condition: None,
                effects: vec![],
                flash: false,
                may_look: false,
                then: None,
            },
        };
    let abilities = |spells: &ObjectFilter, from, nth_each_turn, abilities: &Operand| {
        PlayerRule::SpellsHaveAbilities {
            spells: spells.clone(),
            from,
            nth_each_turn,
            abilities: Box::new(abilities.clone()),
        }
    };
    let spend = |on| PlayerRule::SpendManaAs {
        spend_as: SpendAs::AnyColor,
        on,
    };
    let cost = |abilities, change, floor| PlayerRule::ActivationCost {
        abilities,
        change,
        floor,
    };
    let reduce = |amount: &Operand| ActivationCostChange::Reduce(Box::new(amount.clone()));
    let equip = || ObjectFilter::OnStack(StackTrait::Keyword(AbilityKeyword::Equip));
    let alternate = |spells: &ObjectFilter, paid: PlayCost, limit| PlayerRule::SpellCost {
        spells: spells.clone(),
        modifier: SpellCostModifier::Alternate { cost: paid, limit },
    };
    let bare = name.strip_prefix("PlayerEffect::").unwrap_or(&name);
    match (bare, operands.as_slice()) {
        (
            "MayLookAtFaceDownExiledCards" | "MayLookAtFaceDownPermanents",
            [Operand::Objects(objects)],
        ) => PlayerRule::Visibility {
            objects: objects.clone(),
            audience: VisibilityAudience::AffectedPlayer,
        },
        ("MayLookAtFaceDownExiledCard", [Operand::Object(object)]) => PlayerRule::Visibility {
            objects: ObjectSet::One(object.clone()),
            audience: VisibilityAudience::AffectedPlayer,
        },
        (
            "SpellsCastHaveAbility" | "SpellsControlledHaveAbility",
            [Operand::Filter(spells), list],
        ) => abilities(spells, None, None, list),
        ("SpellsCastFromHandHaveAbility", [Operand::Filter(spells), list]) => {
            abilities(spells, Some(EventZone::Hand), None, list)
        }
        (
            "TheNthSpellCastEachTurnHasAbility",
            [Operand::Number(nth), Operand::Filter(spells), list],
        ) => abilities(spells, None, Some(nth.clone()), list),
        ("MaySpendManaAsThoughItWasAnyColor", []) => spend(ManaPayment::Anything),
        ("MaySpendManaAsThoughItWasAnyColorToCastSpells", [Operand::Filter(spells)]) => {
            spend(ManaPayment::Spells(spells.clone()))
        }
        ("MaySpendManaAsThoughItWasAnyColorToPayForAbilities", [ability])
            if stack_filter(ability).is_some() =>
        {
            spend(ManaPayment::Abilities(stack_filter(ability).unwrap()))
        }
        ("CantAttackWithCreatures", [Operand::Objects(objects)])
            if event_filter(objects).is_some() =>
        {
            PlayerRule::Prohibit(PlayerAction::AttackWith(event_filter(objects).unwrap()))
        }
        ("CantBlockWithCreatures", [Operand::Objects(objects)])
            if event_filter(objects).is_some() =>
        {
            PlayerRule::Prohibit(PlayerAction::BlockWith(event_filter(objects).unwrap()))
        }
        ("CantAttackPlayer", [Operand::Player(player)]) => PlayerRule::Prohibit(
            PlayerAction::AttackInto(Recipients::Players(PlayerSet::One(player.clone()))),
        ),
        ("CantAttackAPermanent", [Operand::Objects(objects)]) => PlayerRule::Prohibit(
            PlayerAction::AttackInto(Recipients::Objects(objects.clone())),
        ),
        ("CantPayLifeToCastSpells", [Operand::Filter(spells)]) => {
            PlayerRule::Prohibit(PlayerAction::PayLife(ManaPayment::Spells(spells.clone())))
        }
        ("CantPayLifeToActivateAbilities", [ability]) if stack_filter(ability).is_some() => {
            PlayerRule::Prohibit(PlayerAction::PayLife(ManaPayment::Abilities(
                stack_filter(ability).unwrap(),
            )))
        }
        ("CantSacrificePermanents", [Operand::Objects(objects)])
            if event_filter(objects).is_some() =>
        {
            PlayerRule::Prohibit(PlayerAction::Sacrifice {
                objects: event_filter(objects).unwrap(),
                paying: None,
            })
        }
        (
            "CantSacrificePermanentsToCastSpells",
            [Operand::Objects(objects), Operand::Filter(spells)],
        ) if event_filter(objects).is_some() => PlayerRule::Prohibit(PlayerAction::Sacrifice {
            objects: event_filter(objects).unwrap(),
            paying: Some(ManaPayment::Spells(spells.clone())),
        }),
        ("CantSacrificePermanentsToActivateAbilities", [Operand::Objects(objects), ability])
            if event_filter(objects).is_some() && stack_filter(ability).is_some() =>
        {
            PlayerRule::Prohibit(PlayerAction::Sacrifice {
                objects: event_filter(objects).unwrap(),
                paying: Some(ManaPayment::Abilities(stack_filter(ability).unwrap())),
            })
        }
        ("DecreaseEquipAbilityCost", [amount]) => cost(equip(), reduce(amount), None),
        ("DecreaseEquipAbilityCostWhenTargetingPermanent", [amount, Operand::Object(object)]) => {
            let targets = ObjectFilter::TargetsAny(Box::new(Recipients::Objects(ObjectSet::One(
                object.clone(),
            ))));
            cost(
                ObjectFilter::All(vec![equip(), targets]),
                reduce(amount),
                None,
            )
        }
        (
            "DamageThatWouldReduceLifeTotalToLessThanNumberReducesItToThatNumberInstead",
            [Operand::Number(floor)],
        ) => PlayerRule::DamageLifeFloor(floor.clone()),
        ("ReduceActivatedCost", [ability, amount]) if stack_filter(ability).is_some() => {
            cost(stack_filter(ability).unwrap(), reduce(amount), None)
        }
        ("ReduceManaCostOfActivatedAbilities", [ability, amount])
            if stack_filter(ability).is_some() =>
        {
            cost(stack_filter(ability).unwrap(), reduce(amount), None)
        }
        (
            "ReduceManaCostOfActivatedAbilitiesNotLessThanOneX",
            [ability, amount, Operand::Number(times)],
        ) if stack_filter(ability).is_some() => cost(
            stack_filter(ability).unwrap(),
            ActivationCostChange::ReduceX {
                amount: Box::new(amount.clone()),
                times: times.clone(),
            },
            Some(1),
        ),
        ("ReduceManaCostOfActivatedAbilitiesNotLessThanOne", [ability, amount])
            if stack_filter(ability).is_some() =>
        {
            cost(stack_filter(ability).unwrap(), reduce(amount), Some(1))
        }
        ("IncreaseAbilityCost", [ability, amount]) if stack_filter(ability).is_some() => cost(
            stack_filter(ability).unwrap(),
            ActivationCostChange::Increase(Box::new(amount.clone())),
            None,
        ),
        ("MayPayAlternateCostForSpells", [paid, Operand::Filter(spells)]) => {
            alternate(spells, PlayCost::Alternate(paid.clone()), None)
        }
        ("MayPayAlternateCostForASpellOnceEachTurn", [paid, Operand::Filter(spells)]) => alternate(
            spells,
            PlayCost::Alternate(paid.clone()),
            Some(PlayLimit::EachTurn(1)),
        ),
        ("MayCastSpellsWithoutPaying", [Operand::Filter(spells)]) => {
            alternate(spells, PlayCost::Free, None)
        }
        ("MayCastCardFromGraveyardByPayingAddedCost", [Operand::Object(card), paid]) => permission(
            ObjectSet::One(card.clone()),
            EventZone::Graveyard,
            ObjectFilter::Any,
            PlayCost::Additional(paid.clone()),
            false,
        ),
        ("MayCastCardInHandWithoutPaying", [Operand::Object(card)]) => permission(
            ObjectSet::One(card.clone()),
            EventZone::Hand,
            ObjectFilter::Any,
            PlayCost::Free,
            false,
        ),
        ("MayCastExiledSpell", [Operand::Object(card), Operand::Filter(spells)]) => permission(
            ObjectSet::One(card.clone()),
            EventZone::Exile,
            spells.clone(),
            PlayCost::Normal,
            false,
        ),
        ("MayCastExiledSpellWithoutPaying", [Operand::Object(card), Operand::Filter(spells)]) => {
            permission(
                ObjectSet::One(card.clone()),
                EventZone::Exile,
                spells.clone(),
                PlayCost::Free,
                false,
            )
        }
        (
            "MayCastGraveyardCardIntoExileIfSpell",
            [Operand::Object(card), Operand::Filter(spells)],
        ) => permission(
            ObjectSet::One(card.clone()),
            EventZone::Graveyard,
            spells.clone(),
            PlayCost::Normal,
            true,
        ),
        _ => match reduce_player_static(bare, &operands) {
            Some(rule) => rule,
            None => PlayerRule::Primitive { name, operands },
        },
    }
}

fn ability_list(operands: &[Operand]) -> Option<Vec<Ability>> {
    operands
        .iter()
        .map(|operand| match operand {
            Operand::Ability(ability) => Some((**ability).clone()),
            _ => None,
        })
        .collect()
}

fn game_range(operand: &Operand) -> Option<(i64, Option<i64>)> {
    let Operand::Term { form, operands, .. } = operand else {
        return None;
    };
    match (form.as_str(), operands.as_slice()) {
        ("BetweenValues", [Operand::Integer(low), Operand::Integer(high)]) => {
            Some((*low, Some(*high)))
        }
        ("ValueOrBigger", [Operand::Integer(low)]) => Some((*low, None)),
        ("ExactValue", [Operand::Integer(value)]) => Some((*value, Some(*value))),
        _ => None,
    }
}

fn printed_pt(operand: &Operand) -> Option<(i32, i32)> {
    let Operand::Term { form, operands, .. } = operand else {
        return None;
    };
    match (form.as_str(), operands.as_slice()) {
        ("PT", [Operand::Integer(power), Operand::Integer(toughness)]) => {
            Some(((*power).try_into().ok()?, (*toughness).try_into().ok()?))
        }
        _ => None,
    }
}

/// Level up and station: both are abilities gated by a counter count.
fn staged_ability(name: &str, operands: &[Operand]) -> Option<Ability> {
    use crate::counter::{CounterKind, CounterName};
    let (level_up, stages, counter) = match (name, operands) {
        ("Rule::LevelUp", [Operand::Cost(cost), Operand::List(levels)]) => {
            let stages = levels
                .iter()
                .map(|level| {
                    let Operand::Term { form, operands, .. } = level else {
                        return None;
                    };
                    let (range, pt, abilities) = match (form.as_str(), operands.as_slice()) {
                        ("Level", [range, pt, Operand::List(abilities)]) => {
                            (range, pt, ability_list(abilities)?)
                        }
                        ("LevelNoRules", [range, pt]) => (range, pt, vec![]),
                        _ => return None,
                    };
                    let (minimum, maximum) = game_range(range)?;
                    Some(CounterStage {
                        minimum,
                        maximum,
                        power_toughness: Some(printed_pt(pt)?),
                        animates: false,
                        abilities,
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            (Some(cost.clone()), stages, CounterName::LevelCounter)
        }
        ("Rule::StationChargedAnimate", [range, Operand::List(abilities), pt]) => {
            let (minimum, maximum) = game_range(range)?;
            let stage = CounterStage {
                minimum,
                maximum,
                power_toughness: Some(printed_pt(pt)?),
                animates: true,
                abilities: ability_list(abilities)?,
            };
            (None, vec![stage], CounterName::ChargeCounter)
        }
        ("Rule::StationCharged", [range, Operand::List(abilities)]) => {
            let (minimum, maximum) = game_range(range)?;
            let stage = CounterStage {
                minimum,
                maximum,
                power_toughness: None,
                animates: false,
                abilities: ability_list(abilities)?,
            };
            (None, vec![stage], CounterName::ChargeCounter)
        }
        _ => return None,
    };
    Some(Ability::CounterStages {
        counter: CounterKind::Named(counter),
        level_up,
        stages,
    })
}

/// Forms that cast or play on resolution, with no effect or restriction of
/// their own. Each says what it offers in its operands.
const PLAY_FORMS: &[&str] = &[
    "CastACopiedCardWithoutPaying",
    "CastASpellFromExile",
    "CastASpellFromExileWithoutPaying",
    "CastASpellFromHandWithoutPaying",
    "CastAnyNumberOfCopiedCards",
    "CastAnyNumberOfCopiedCardsWithoutPaying",
    "CastAnyNumberOfExiledCardsWithoutPaying",
    "CastAnyNumberOfGraveyardCardsWithoutPayingIntoExile",
    "CastAnyNumberOfSpellsFromExileWithoutPaying",
    "CastAnyNumberOfSpellsFromHandWithoutPaying",
    "CastCardInHandWithoutPaying",
    "CastCopiedCard",
    "CastCopiedCardWithoutPaying",
    "CastExiledCardWithoutPaying",
    "CastExiledCardWithoutPayingIntoExile",
    "CastGraveyardCard",
    "CastGraveyardCardIntoExile",
    "CastGraveyardCardWithoutPaying",
    "CastGraveyardCardWithoutPayingIntoExile",
    "CastSpellFromExile",
    "CastSpellFromExileWithoutPaying",
    "CastUptoNumberCopiedCardsWithoutPaying",
    "CastUptoNumberExiledCardsWithoutPaying",
    "CastUptoNumberSpellsFromExileWithoutPaying",
    "CastUptoNumberSpellsFromHandWithoutPaying",
    "PlayACardFromExileWithoutPaying",
    "PlayAnyNumberOfExiledCards",
    "PlayExiledCard",
    "PlayExiledCardWithoutPaying",
    "PlayGraveyardCardWithoutPaying",
];

fn reduce_play(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    let form = name.strip_prefix("Action::")?;
    if !PLAY_FORMS.contains(&form) {
        return None;
    }
    let (count, rest) = match operands {
        _ if form.contains("AnyNumber") => (Count::AnyNumber, operands),
        [Operand::Number(n), rest @ ..] if form.contains("Upto") => (Count::UpTo(n.clone()), rest),
        _ => (Count::Exactly(Expr::Constant(1)), operands),
    };
    let cards = match rest {
        [Operand::Object(card)] => ObjectSet::One(card.clone()),
        [Operand::Objects(cards)] => cards.clone(),
        [Operand::Filter(filter)] if form.contains("FromHand") => ObjectSet::Zone {
            scope: ZoneScope::Players {
                zone: EventZone::Hand,
                players: actor.clone(),
            },
            kind: ObjectKind::Card,
            filter: filter.clone(),
        },
        [Operand::Filter(filter)] if form.contains("Graveyard") => ObjectSet::Zone {
            scope: ZoneScope::Players {
                zone: EventZone::Graveyard,
                players: PlayerSet::All,
            },
            kind: ObjectKind::Card,
            filter: filter.clone(),
        },
        [Operand::Filter(filter), Operand::Object(card)] => ObjectSet::Matching {
            objects: Box::new(ObjectSet::One(card.clone())),
            filter: filter.clone(),
        },
        [Operand::Filter(filter), Operand::Objects(cards)] => ObjectSet::Matching {
            objects: Box::new(cards.clone()),
            filter: filter.clone(),
        },
        _ => return None,
    };
    Some(Operation::PlayCards {
        actor: actor.clone(),
        action: if form.starts_with("Cast") {
            CardPlayAction::CastSpell
        } else {
            CardPlayAction::Either
        },
        cards,
        count,
        cost: if form.contains("WithoutPaying") {
            PlayCost::Free
        } else {
            PlayCost::Normal
        },
        into_exile: form.ends_with("IntoExile"),
    })
}

fn reduce_copy(actor: &PlayerSet, name: &str, operands: &[Operand]) -> Option<Operation> {
    let objects = match operands.first()? {
        Operand::Object(card) => ObjectSet::One(card.clone()),
        Operand::Objects(cards) => cards.clone(),
        _ => return None,
    };
    let times = match (name, &operands[1..]) {
        (
            "Action::CopyExiledCard"
            | "Action::CopyExiledCards"
            | "Action::CopyAnExiledCard"
            | "Action::CopyCardInHand",
            [],
        ) => Expr::Constant(1),
        (
            "Action::CopyExiledCardNumberTimes" | "Action::CopyAnExiledCardNumberTimes",
            [Operand::Number(times)],
        ) => times.clone(),
        _ => return None,
    };
    Some(Operation::CopyCards {
        actor: actor.clone(),
        objects,
        times,
    })
}

fn deck_rule(name: &str, operands: &[Operand]) -> Option<DeckRule> {
    let form = name.strip_prefix("DeckConstruction::")?;
    Some(match (form, operands) {
        ("CanBeYourCommander", []) => DeckRule::CanBeCommander,
        ("Partner", []) => DeckRule::Partner(PartnerKind::Any),
        ("PartnerCharacterSelect", []) => DeckRule::Partner(PartnerKind::CharacterSelect),
        ("PartnerFatherAndSon", []) => DeckRule::Partner(PartnerKind::FatherAndSon),
        ("PartnerFriendsForever", []) => DeckRule::Partner(PartnerKind::FriendsForever),
        ("PartnerSurvivors", []) => DeckRule::Partner(PartnerKind::Survivors),
        ("PartnerWith", [Operand::Text(card)]) => {
            DeckRule::Partner(PartnerKind::With(card.clone()))
        }
        ("DoctorsCompanion", []) => DeckRule::DoctorsCompanion,
        ("ChooseABackground", []) => DeckRule::ChooseBackground,
        ("CanHaveAnyNumberOfThisCard", []) => DeckRule::AnyNumberOfCopies,
        ("CanHaveUptoNumberOfThisCard", [Operand::Number(count)]) => {
            DeckRule::UpToCopies(count.clone())
        }
        ("HasNoMaximumDeckSize", []) => DeckRule::NoMaximumDeckSize,
        ("CanHaveAnyBasicLandCards", []) => DeckRule::AnyBasicLands,
        ("CanHaveAnyCards", [Operand::Filter(cards)]) => DeckRule::AnyCards(cards.clone()),
        ("CanHaveAnyCardsWithOneSingleAdditionalColor", [Operand::Filter(cards)]) => {
            DeckRule::AnyCardsWithOneAdditionalColor(cards.clone())
        }
        ("ThisCardIsBanned", []) => DeckRule::Banned,
        ("RemoveFromDeckIfNotPlayingForAnte", []) => DeckRule::RemoveUnlessPlayingForAnte,
        _ => return None,
    })
}

/// An outcome that may or may not announce targets of its own.
fn outcome(operand: &Operand) -> Option<Box<Ability>> {
    match operand {
        Operand::Ability(ability) => Some(ability.clone()),
        Operand::Program(body) => Some(Box::new(Ability::Spell {
            announcement: Announcement {
                targets: vec![],
                modes: None,
            },
            body: body.clone(),
        })),
        _ => None,
    }
}

/// How a spell or an ability is put together, when it is more than one body.
fn casting_mode(name: &str, operands: &[Operand]) -> Option<Ability> {
    use Operand::{Integer, List, Objects, Text, Vocabulary};
    let again = |cause: Option<EventSpec>, abilities: Option<&Operand>, rule| {
        Some(Ability::TriggerRule {
            cause,
            abilities: match abilities {
                Some(abilities) => Some(stack_filter(abilities)?),
                None => None,
            },
            rule,
        })
    };
    let moved = |objects: &ObjectSet, from, to| {
        Some(EventSpec::ZoneChange {
            objects: event_filter(objects)?,
            from,
            to,
        })
    };
    let entered = |objects| moved(objects, None, EventZone::Battlefield);
    let died = |objects| moved(objects, Some(EventZone::Battlefield), EventZone::Graveyard);
    let object = |event, objects: &ObjectSet| {
        Some(EventSpec::Object {
            event,
            objects: event_filter(objects)?,
        })
    };
    let player = |event, players: &PlayerSet| {
        Some(EventSpec::Player {
            event,
            players: players.clone(),
            objects: None,
        })
    };
    let legend = |objects: ObjectSet| Ability::Static {
        kind: StaticKind::Ordinary,
        affected: EffectSubjects::Objects(objects),
        changes: vec![Change::Rule(RuleChange::LegendRuleExempt)],
    };
    match (name.strip_prefix("Rule::")?, operands) {
        ("SpellActions_Gift", [gift, promised, unpromised]) => Some(Ability::Gift {
            gift: vec![match gift {
                Operand::Program(program) => program.clone(),
                _ => return None,
            }],
            promised: Some(outcome(promised)?),
            unpromised: Some(outcome(unpromised)?),
        }),
        ("Permanent_Gift", gift) => Some(Ability::Gift {
            gift: program_list(gift)?,
            promised: None,
            unpromised: None,
        }),
        ("SpellActions_Spree", modes) => Some(Ability::Spree {
            modes: modes
                .iter()
                .map(|mode| match mode {
                    Operand::Term { form, operands, .. } if form == "SpreeAction" => {
                        match operands.as_slice() {
                            [Operand::Cost(cost), ability] => Some(SpreeMode {
                                cost: cost.clone(),
                                ability: outcome(ability)?,
                            }),
                            _ => None,
                        }
                    }
                    _ => None,
                })
                .collect::<Option<_>>()?,
        }),
        ("SpellActions_Teamwork", [Integer(power), used, unused]) => Some(Ability::CostBranch {
            keyword: "Teamwork".into(),
            cost: Cost {
                parts: vec![CostPart::Teamwork((*power).try_into().ok()?)],
            },
            paid: outcome(used)?,
            unpaid: outcome(unused)?,
        }),
        ("Visit", [body]) => Some(Ability::Visit {
            body: outcome(body)?,
            prize: None,
        }),
        ("VisitAndPrize", [body, prize]) => Some(Ability::Visit {
            body: outcome(body)?,
            prize: Some(outcome(prize)?),
        }),
        ("DungeonLevel", [Text(room), body, List(leads_to)]) => Some(Ability::DungeonRoom {
            name: room.clone(),
            body: outcome(body)?,
            leads_to: leads_to
                .iter()
                .map(|room| match room {
                    Text(room) => Some(room.clone()),
                    _ => None,
                })
                .collect::<Option<_>>()?,
        }),
        ("AnchorWord", [Text(word), Operand::Ability(ability)])
        | ("AnchorWord", [Vocabulary { form: word, .. }, Operand::Ability(ability)]) => {
            Some(Ability::Conditional {
                condition: Condition::ChosenWord(word.clone()),
                then: vec![(**ability).clone()],
                otherwise: vec![],
            })
        }
        ("AbilitiesTriggerAnAdditionalTime", [abilities]) => {
            again(None, Some(abilities), TriggerRule::TriggersAgain)
        }
        (
            "APermanentEnteringTheBattlefieldCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(cause), abilities],
        ) => again(
            Some(entered(cause)?),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        (
            "APermanentDyingCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(cause), abilities],
        ) => again(
            Some(died(cause)?),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        (
            "APermanentLeavingTheBattlefieldCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(cause), abilities],
        ) => again(
            Some(object(ObjectEvent::LeavesBattlefield, cause)?),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        (
            "APermanentTurningFaceUpCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(cause), abilities],
        ) => again(
            Some(object(ObjectEvent::TurnsFaceUp, cause)?),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        (
            "APermanentAttackingCausesAbilitiesToTriggerAnAdditionalTime",
            [Objects(cause), abilities],
        ) => again(
            Some(EventSpec::Attack {
                attackers: event_filter(cause)?,
                by: None,
                defenders: AttackDefenders::Any,
            }),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        (
            "APlayerGainingLifeCausesAbilitiesToTriggerAnAdditionalTime",
            [Operand::Players(cause), abilities],
        ) => again(
            player(PlayerEvent::GainsLife, cause),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        (
            "APlayerDrawingACardCausesAbilitiesToTriggerAnAdditionalTime",
            [Operand::Players(cause), abilities],
        ) => again(
            player(PlayerEvent::Draws, cause),
            Some(abilities),
            TriggerRule::TriggersAgain,
        ),
        ("PermanentsEnteringTheBattlefieldDontCauseAbilitiesToTrigger", [Objects(cause)]) => {
            again(Some(entered(cause)?), None, TriggerRule::DoesntTrigger)
        }
        ("PermanentsDyingDontCauseAbilitiesToTrigger", [Objects(cause)]) => {
            again(Some(died(cause)?), None, TriggerRule::DoesntTrigger)
        }
        ("TheLegendsRuleDoesntApplyToPermanents", [Objects(objects)]) => {
            Some(legend(objects.clone()))
        }
        ("TheLegendsRuleDoesntApply", []) => Some(legend(ObjectSet::One(ObjectRef::Source))),
        _ => None,
    }
}

/// What "each card in a graveyard has" gives those cards.
fn zone_static(scope: ZoneScope, cards: &ObjectFilter, changes: Vec<Change>) -> Ability {
    Ability::Static {
        kind: StaticKind::Ordinary,
        affected: EffectSubjects::Objects(ObjectSet::Zone {
            scope,
            kind: ObjectKind::Card,
            filter: cards.clone(),
        }),
        changes,
    }
}

fn graveyard_changes(effects: &[Operand]) -> Option<Vec<Change>> {
    let mut changes = vec![];
    for effect in effects {
        let Operand::Term { form, operands, .. } = effect else {
            return None;
        };
        match (form.as_str(), operands.as_slice()) {
            ("AddAbility", abilities) => changes.extend(
                ability_list(abilities)?
                    .into_iter()
                    .map(|ability| Change::GrantAbility(Box::new(ability))),
            ),
            ("LosesAllAbilities", []) => changes.push(Change::RemoveAllAbilities),
            (form, operands) => changes.push(card_change(form, operands)?),
        }
    }
    Some(changes)
}

/// A row's ability in generic form, or the row itself when it has none.
/// Rules that keep damage marked, and costs of activating abilities in general.
fn reduce_rule_statics(name: &str, operands: &[Operand]) -> Option<Ability> {
    let ordinary = |affected, change| Ability::Static {
        kind: StaticKind::Ordinary,
        affected,
        changes: vec![change],
    };
    let stays = Change::Rule(RuleChange::DamageStaysMarked);
    Some(match (name, operands) {
        ("Rule::DamageIsntRemovedFromCreaturesDuringCleanup", [Operand::Objects(objects)]) => {
            ordinary(EffectSubjects::Objects(objects.clone()), stays)
        }
        ("Rule::DamageIsntRemovedFromCreatureDuringCleanup", [Operand::Object(object)]) => {
            ordinary(
                EffectSubjects::Objects(ObjectSet::One(object.clone())),
                stays,
            )
        }
        ("Rule::ActivatedAbilityEffect", [abilities, Operand::Term { form, operands, .. }]) => {
            let (change, floor) = match (form.as_str(), operands.as_slice()) {
                ("IncreaseManaCost", [amount]) => (
                    ActivationCostChange::Increase(Box::new(amount.clone())),
                    None,
                ),
                ("ReduceManaCostNotLessThanOne", [amount]) => (
                    ActivationCostChange::Reduce(Box::new(amount.clone())),
                    Some(1),
                ),
                _ => return None,
            };
            ordinary(
                EffectSubjects::Players(PlayerSet::All),
                Change::PlayerRule(PlayerRule::ActivationCost {
                    abilities: stack_filter(abilities)?,
                    change,
                    floor,
                }),
            )
        }
        _ => return None,
    })
}

/// What mana may be spent on, past spells and abilities in general.
pub fn reduce_mana_use(name: &str, operands: &[Operand]) -> Option<ManaUse> {
    use ManaPurpose as P;
    let purpose = match name.strip_prefix("ManaUseModifier::")? {
        "CanOnlySpendOnXCost" => P::XCost,
        "CanOnlySpendOnCumulativeUpkeepCosts" => P::CumulativeUpkeep,
        "CanOnlySpendToUnlockDoors" => P::UnlockDoor,
        "CanOnlySpendToTurnAPermanentFaceUp" => P::TurnFaceUp(ObjectFilter::Any),
        "CanOnlySpendToTurnACreatureFaceUp" => {
            P::TurnFaceUp(ObjectFilter::CardType(crate::mtg_types::CardType::Creature))
        }
        "CanOnlySpendOnMorphCosts" => P::MorphCost,
        "CanOnlySpendToPayDisturbCosts" => P::DisturbCost,
        "CanOnlySpendToForetellCards" => P::Foretell,
        "CanOnlySpendToGainAClassLevel" => P::ClassLevel,
        "CanOnlySpendToActivateEquipAbilities" => P::AbilityKeyword(AbilityKeyword::Equip),
        "CanOnlySpendToActivatePowerUpAbilities" => P::AbilityKeyword(AbilityKeyword::PowerUp),
        "CantSpendOnGenericCosts" => {
            return Some(ManaUse::Not(Box::new(ManaUse::Purpose(P::GenericCost))))
        }
        "CanOnlySpendToCastTheirCommander" => {
            return Some(ManaUse::CastSpell(ObjectFilter::State(
                ObjectState::Commander,
            )))
        }
        _ => return None,
    };
    operands.is_empty().then_some(ManaUse::Purpose(purpose))
}

/// A duration that ends on an event the trigger table already names.
pub fn reduce_duration(name: &str, operands: &[Operand]) -> Option<Duration> {
    let [Operand::Players(players)] = operands else {
        return None;
    };
    let event = match name {
        "Expiration::UntilAPlayerPlaneswalks" => PlayerEvent::PlaneswalksAway,
        "Expiration::UntilAPlayerBecomesTheMonarch" => PlayerEvent::BecomesMonarch,
        _ => return None,
    };
    Some(Duration::UntilEvent(Box::new(EventSpec::Player {
        event,
        players: players.clone(),
        objects: None,
    })))
}

pub fn reduce_ability(name: String, operands: Vec<Operand>) -> Ability {
    if let Some(rule) = deck_rule(&name, &operands) {
        return Ability::DeckRule(rule);
    }
    if let Some(ability) = casting_mode(&name, &operands) {
        return ability;
    }
    if let Some(ability) = staged_ability(&name, &operands) {
        return ability;
    }
    if let Some(ability) = reduce_rule_statics(&name, &operands) {
        return ability;
    }
    if let Some(ability) = reduce_rule_ability(&name, &operands) {
        return ability;
    }
    match (name.as_str(), operands.as_slice()) {
        (
            "Rule::AsSelfDraft"
            | "Rule::FaceUpDraftEffect"
            | "Rule::BeforeShufflingDeckToStartTheGame",
            effects,
        ) => return Ability::DraftRule(effects.to_vec()),
        ("Rule::Companion", [condition]) => return Ability::Companion(Box::new(condition.clone())),
        ("Rule::DeckConstructionIfCommander" | "Rule::DeckBuildingIfCommander", [rule]) => {
            return Ability::CommanderDeckRule(Box::new(rule.clone()))
        }
        ("Rule::Offering", [Operand::Filter(filter)]) => return Ability::Offering(filter.clone()),
        ("Rule::FlashForCasters", [Operand::Condition(condition)]) => {
            return Ability::FlashIf(condition.clone())
        }
        ("Rule::CraftWithACraftable", [materials, cost]) => {
            return Ability::Craft {
                materials: Box::new(materials.clone()),
                count: None,
                cost: Box::new(cost.clone()),
            }
        }
        (
            "Rule::CraftWithANumberOfCraftables",
            [Operand::Comparison(comparison, value), materials, cost],
        ) => {
            return Ability::Craft {
                materials: Box::new(materials.clone()),
                count: Some((comparison.clone(), value.clone())),
                cost: Box::new(cost.clone()),
            }
        }
        ("Rule::Mystery", [Operand::Condition(condition), Operand::List(abilities)]) => {
            if let Some(abilities) = ability_list(abilities) {
                return Ability::Mystery {
                    condition: condition.clone(),
                    abilities,
                };
            }
        }
        ("Rule::ProtectionAndDoesntRemovePermanents", [from, Operand::Objects(keeps)]) => {
            return Ability::ProtectionExcept {
                from: Box::new(from.clone()),
                keeps: keeps.clone(),
            }
        }
        ("Rule::MaxSpeed", [Operand::Ability(ability)]) => {
            return Ability::Conditional {
                condition: Condition::PlayerState {
                    player: PlayerRef::AbilityController,
                    state: PlayerState::MaxSpeed,
                },
                then: vec![(**ability).clone()],
                otherwise: vec![],
            }
        }
        (
            "Rule::EachCardInGraveyardEffect",
            [Operand::Filter(cards), Operand::Player(owner), Operand::List(effects)],
        ) => {
            if let Some(changes) = graveyard_changes(effects) {
                return Ability::Static {
                    kind: StaticKind::Ordinary,
                    affected: EffectSubjects::Objects(ObjectSet::Zone {
                        scope: ZoneScope::Players {
                            zone: EventZone::Graveyard,
                            players: PlayerSet::One(owner.clone()),
                        },
                        kind: ObjectKind::Card,
                        filter: cards.clone(),
                    }),
                    changes,
                };
            }
        }
        (
            "Rule::EachCardInPlayersHandEffect",
            [Operand::Filter(cards), Operand::Player(owner), Operand::List(effects)],
        ) => {
            if let Some(changes) = graveyard_changes(effects) {
                return zone_static(
                    ZoneScope::Players {
                        zone: EventZone::Hand,
                        players: PlayerSet::One(owner.clone()),
                    },
                    cards,
                    changes,
                );
            }
        }
        (
            "Rule::EachCardInAGraveyardEffect",
            [Operand::Filter(cards), Operand::Players(owners), Operand::List(effects)],
        ) => {
            if let Some(changes) = graveyard_changes(effects) {
                return zone_static(
                    ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: owners.clone(),
                    },
                    cards,
                    changes,
                );
            }
        }
        ("Rule::Backup", [Operand::Integer(amount), Operand::List(abilities)]) => {
            if let Some(abilities) = ability_list(abilities) {
                return Ability::Backup {
                    amount: *amount,
                    abilities,
                };
            }
        }
        _ => {}
    }
    let Some((_, mechanic)) = MECHANICS.iter().find(|(row, _)| *row == name) else {
        return Ability::Primitive { name, operands };
    };
    let (mut cost, mut amount, mut filter) = (None, None, None);
    for operand in &operands {
        match operand {
            Operand::Cost(value) => cost = Some(value.clone()),
            Operand::Number(value) => amount = Some(value.clone()),
            Operand::Integer(value) => amount = Some(Expr::Constant(*value)),
            Operand::Filter(value) => filter = Some(value.clone()),
            Operand::Objects(objects) => match event_filter(objects) {
                Some(value) => filter = Some(value),
                None => return Ability::Primitive { name, operands },
            },
            _ => return Ability::Primitive { name, operands },
        }
    }
    Ability::Mechanic {
        mechanic: *mechanic,
        cost,
        amount,
        filter,
    }
}

/// What a reviewed row says its body can name.
pub fn row_bindings(name: &str) -> EventBindings {
    let binds = crate::primitive_spec::lookup(name).map_or(&[][..], |r| &r.binds);
    let has = |field: &str| binds.iter().any(|b| b == field);
    EventBindings {
        object: has("object"),
        permanent: has("permanent"),
        spell: has("spell"),
        player: has("player"),
        amount: has("amount"),
        block_pair: has("block_pair"),
        group: has("group"),
        stack_object: has("stack_object"),
        other: has("other"),
        other_player: has("other_player"),
        moved: has("moved"),
        ..EventBindings::default()
    }
}

/// `outer` with what a reviewed row adds for its own operands.
/// Trigger rows that are one player event. Each keeps its reviewed `binds`.
const PLAYER_EVENTS: &[(&str, PlayerEvent)] = &[
    ("Trigger::WhenAPlayerGainsLife", PlayerEvent::GainsLife),
    (
        "Trigger::WhenAPermanentEntersTheBattlefieldUnderAPlayersControl",
        PlayerEvent::EntersUnderControl,
    ),
    (
        "Trigger::WhenAPermanentEntersTheBattlefieldUnderAPlayersControlWithoutBeingPlayed",
        PlayerEvent::EntersUnplayed,
    ),
    ("Trigger::WhenAPlayerLosesLife", PlayerEvent::LosesLife),
    ("Trigger::WhenAPlayerDrawsACard", PlayerEvent::Draws),
    ("Trigger::WhenAPlayerAttacks", PlayerEvent::Attacks),
    ("Trigger::WhenAPlayerScrys", PlayerEvent::Scries),
    ("Trigger::WhenAPlayerSurveils", PlayerEvent::Surveils),
    (
        "Trigger::WhenAPlayerCommitsACrime",
        PlayerEvent::CommitsCrime,
    ),
    (
        "Trigger::WhenAPlayerSearchesTheirLibrary",
        PlayerEvent::SearchesLibrary,
    ),
    ("Trigger::WhenAPlayerLosesTheGame", PlayerEvent::LosesGame),
    (
        "Trigger::WhenAPlayerRollsAnyNumberOfDice",
        PlayerEvent::RollsDice,
    ),
    (
        "Trigger::WhenAPlayerWinsACoinFlip",
        PlayerEvent::WinsCoinFlip,
    ),
    (
        "Trigger::WhenAPlayerSacrificesAPermanent",
        PlayerEvent::Sacrifices,
    ),
    ("Trigger::WhenAPlayerPlaysALand", PlayerEvent::PlaysLand),
    ("Trigger::WhenAPlayerUnlocksADoor", PlayerEvent::UnlocksDoor),
    ("Trigger::WhenChaosEnsues", PlayerEvent::ChaosEnsues),
    (
        "Trigger::WhenAPlayerSetsASchemeInMotion",
        PlayerEvent::SetsSchemeInMotion,
    ),
    (
        "Trigger::WhenAPlayerPlaneswalksToAPlane",
        PlayerEvent::Planeswalks,
    ),
    (
        "Trigger::WhenAPlayerEncountersAPhenomenon",
        PlayerEvent::EncountersPhenomenon,
    ),
    (
        "Trigger::WhenAPlayerFullyUnlocksARoom",
        PlayerEvent::FullyUnlocksRoom,
    ),
    (
        "Trigger::WhenAPlayerTapsAPermanentForMana",
        PlayerEvent::TapsForMana,
    ),
    (
        "Trigger::WhenDayBecomesNightOrNightBecomesDay",
        PlayerEvent::DayNightChanges,
    ),
    (
        "Trigger::WhenPlayersFinishVoting",
        PlayerEvent::FinishVoting,
    ),
    ("Trigger::AtTheBeginningOfTheGame", PlayerEvent::GameBegins),
    (
        "Trigger::WhenAPlayerLosesControlOfAPermanent",
        PlayerEvent::LosesControl,
    ),
    ("Trigger::WhenTheRingTemptsAPlayer", PlayerEvent::RingTempts),
    (
        "Trigger::WhenAPlayerCreatesAToken",
        PlayerEvent::CreatesToken,
    ),
    ("Trigger::WhenAPlayerExertsACreature", PlayerEvent::Exerts),
    (
        "Trigger::WhenAPlayerProliferates",
        PlayerEvent::Proliferates,
    ),
    (
        "Trigger::WhenAPlayerRevealsFirstCardDrawn",
        PlayerEvent::RevealsFirstCardDrawn,
    ),
    (
        "Trigger::WhenAPlayerPlaysACardFromExile",
        PlayerEvent::PlaysCardFromExile,
    ),
    (
        "Trigger::WhenAPlayerPlaysALandFromExile",
        PlayerEvent::PlaysLandFromExile,
    ),
    (
        "Trigger::WhenAPlayerPutsAPermanentOnTheBattlefield",
        PlayerEvent::PutsOntoBattlefield,
    ),
    ("Trigger::WhenAPlayerGetsEnergy", PlayerEvent::GetsEnergy),
    (
        "Trigger::WhenAPlayerBecomesTheMonarch",
        PlayerEvent::BecomesMonarch,
    ),
    (
        "Trigger::WhenAPlayerCompletesADungeon",
        PlayerEvent::CompletesDungeon,
    ),
    (
        "Trigger::WhenAPlayerTapsAPermanent",
        PlayerEvent::TapsPermanent,
    ),
    ("Trigger::WhenAPlayerPlaysACard", PlayerEvent::PlaysCard),
    (
        "Trigger::WhenAPlayerDrawsARevealedCard",
        PlayerEvent::DrawsRevealedCard,
    ),
    (
        "Trigger::WhenAPlayerDrawsASpecificCard",
        PlayerEvent::DrawsCard,
    ),
    (
        "Trigger::WhenAPlayerSeeksAnyNumberOfCards",
        PlayerEvent::Seeks,
    ),
    (
        "Trigger::WhenAPlayerVisitsAnAttraction",
        PlayerEvent::VisitsAttraction,
    ),
    ("Trigger::WhenAPlayerRollsADie", PlayerEvent::RollsADie),
    (
        "Trigger::WhenAPlayerRollsThePlanarDie",
        PlayerEvent::RollsPlanarDie,
    ),
    (
        "Trigger::WhenAPlayerShufflesTheirLibrary",
        PlayerEvent::Shuffles,
    ),
    (
        "Trigger::WhenAPlayerConjuresAnyNumberOfCards",
        PlayerEvent::Conjures,
    ),
    (
        "Trigger::WhenAPlayerConjuresAnyNumberOfOtherCards",
        PlayerEvent::ConjuresOther,
    ),
    ("Trigger::WhenAPlayerDiscovers", PlayerEvent::Discovers),
    ("Trigger::WhenAPlayerClashes", PlayerEvent::Clashes),
    ("Trigger::WhenAPlayerClashesAndWins", PlayerEvent::WinsClash),
    (
        "Trigger::WhenAPlayerInvestigates",
        PlayerEvent::Investigates,
    ),
    (
        "Trigger::WhenAPlayerCollectsEvidence",
        PlayerEvent::CollectsEvidence,
    ),
    (
        "Trigger::WhenAPlayerPutsAStickerOnAPermanent",
        PlayerEvent::PutsSticker,
    ),
    (
        "Trigger::WhenAPlayerPutsANameStickerOnAPermanent",
        PlayerEvent::PutsNameSticker,
    ),
    (
        "Trigger::WhenAPlayerPutsAnAbilityStickerOnAPermanent",
        PlayerEvent::PutsAbilitySticker,
    ),
    (
        "Trigger::WhenAPlayerPutsAnArtStickerOnAPermanent",
        PlayerEvent::PutsArtSticker,
    ),
    (
        "Trigger::WhenAPlayerPlacesASticker",
        PlayerEvent::PlacesSticker,
    ),
    (
        "Trigger::WhenAPlayerRollsToVisitTheirAttractions",
        PlayerEvent::RollsToVisit,
    ),
    (
        "Trigger::WhenAPlayerWaterEarthFireOrAirBends",
        PlayerEvent::Bends,
    ),
    (
        "Trigger::WhenAPlayerChoosesARingBearer",
        PlayerEvent::ChoosesRingBearer,
    ),
    ("Trigger::WhenAPlayerSolvesACase", PlayerEvent::SolvesCase),
    (
        "Trigger::WhenAPlayerSelectsAnyNumberOfTargetsAtRandom",
        PlayerEvent::SelectsTargetsAtRandom,
    ),
    (
        "Trigger::WhenAPlayerSelectsAModeAtRandom",
        PlayerEvent::SelectsModeAtRandom,
    ),
    (
        "Trigger::WhenAPlayerFlipsAnyNumberOfCoins",
        PlayerEvent::FlipsCoins,
    ),
    ("Trigger::WhenAPlayerForages", PlayerEvent::Forages),
    (
        "Trigger::WhenAPlayerRollsANatural20",
        PlayerEvent::RollsNatural20,
    ),
    (
        "Trigger::WhenAPlayerRollsADiesHighestNaturalResult",
        PlayerEvent::RollsHighestNatural,
    ),
    (
        "Trigger::WhenAPlayerRollsABlankOnThePlanarDie",
        PlayerEvent::RollsPlanarBlank,
    ),
    ("Trigger::WhenAPlayerForetellsACard", PlayerEvent::Foretells),
    ("Trigger::WhenAPlayerPaysLife", PlayerEvent::PaysLife),
    ("Trigger::WhenAPlayerGivesAGift", PlayerEvent::GivesGift),
    (
        "Trigger::WhenAPlayerLosesACoinFlip",
        PlayerEvent::LosesCoinFlip,
    ),
    (
        "Trigger::WhenAPlayerTurnsAPermanentFaceUp",
        PlayerEvent::TurnsFaceUp,
    ),
    (
        "Trigger::WhenAPlayerSacrificesAPermanentForEmerge",
        PlayerEvent::SacrificesForEmerge,
    ),
    (
        "Trigger::WhenAPlayerPaysAPermanentsCumulativeUpkeepCost",
        PlayerEvent::PaysCumulativeUpkeep,
    ),
    (
        "Trigger::WhenAPlayerDoesntPayAPermanentsCumulativeUpkeepCost",
        PlayerEvent::DeclinesCumulativeUpkeep,
    ),
    (
        "Trigger::WhenAPlayerMillsASpecificCard",
        PlayerEvent::MillsCard,
    ),
    (
        "Trigger::WhenAPlayerMillsAnyNumberOfCards",
        PlayerEvent::Mills,
    ),
    (
        "Trigger::WhenAPlayerManifestsDread",
        PlayerEvent::ManifestsDread,
    ),
    (
        "Trigger::WhenAPlayerFacesADilemma",
        PlayerEvent::FacesDilemma,
    ),
    (
        "Trigger::WhenAPlayerOpensAnAttraction",
        PlayerEvent::OpensAttraction,
    ),
    (
        "Trigger::WhenAPlayerClaimsThePrizeOfAnAttraction",
        PlayerEvent::ClaimsPrize,
    ),
    (
        "Trigger::WhenAPlayerDiscardsACardAtRandom",
        PlayerEvent::DiscardsAtRandom,
    ),
    (
        "Trigger::WhenAPlayerPlaysALandFromAnywhereOtherThanTheirHand",
        PlayerEvent::PlaysLandNotFromHand,
    ),
    (
        "Trigger::WhenAPlayerChoosesToPutAnyCardsOnTheBottomOfTheirLibraryWhileScrying",
        PlayerEvent::BottomsWhileScrying,
    ),
    (
        "Trigger::WhenAPlayerPlaneswalksAwayFromAPlane",
        PlayerEvent::PlaneswalksAway,
    ),
    (
        "Trigger::WhenAPlayerBecomesTheTargetOfASpellOrAbility",
        PlayerEvent::BecomesTarget,
    ),
    (
        "Trigger::WhenAPlayerBecomesTheTargetOfASpell",
        PlayerEvent::BecomesTarget,
    ),
];

const OBJECT_EVENTS: &[(&str, ObjectEvent)] = &[
    ("Trigger::WhenAPermanentTransforms", ObjectEvent::Transforms),
    (
        "Trigger::WhenAPermanentBecomesMonstrous",
        ObjectEvent::BecomesMonstrous,
    ),
    ("Trigger::WhenACreatureMutates", ObjectEvent::Mutates),
    (
        "Trigger::WhenACreatureSpecializes",
        ObjectEvent::Specializes,
    ),
    (
        "Trigger::WhenAPermanentIsTappedForMana",
        ObjectEvent::TappedForMana,
    ),
    ("Trigger::WhenAVehicleBecoemsCrewed", ObjectEvent::Crewed),
    ("Trigger::WhenAPermanentExplores", ObjectEvent::Explores),
    (
        "Trigger::WhenAPermanentLeavesTheBattlefieldWithoutDying",
        ObjectEvent::LeavesWithoutDying,
    ),
    ("Trigger::WhenAPermanentPhasesIn", ObjectEvent::PhasesIn),
    ("Trigger::WhenAPermanentPhasesOut", ObjectEvent::PhasesOut),
    ("Trigger::WhenAPermanentConnives", ObjectEvent::Connives),
    ("Trigger::WhenAPermanentFights", ObjectEvent::Fights),
    (
        "Trigger::WhenACreatureBecomesRenowned",
        ObjectEvent::BecomesRenowned,
    ),
    ("Trigger::WhenAPermanentEvolves", ObjectEvent::Evolves),
    ("Trigger::WhenACreatureIsDevoured", ObjectEvent::Devoured),
    ("Trigger::WhenAPermanentIsDestroyed", ObjectEvent::Destroyed),
    (
        "Trigger::WhenAPermanentIsSacrificed",
        ObjectEvent::Sacrificed,
    ),
    ("Trigger::WhenAPermanentTrains", ObjectEvent::Trains),
    (
        "Trigger::WhenAPermanentsCumulativeUpkeepCostIsPaid",
        ObjectEvent::CumulativeUpkeepPaid,
    ),
    (
        "Trigger::WhenAnEchoCostOfAPermanentIsPaid",
        ObjectEvent::EchoPaid,
    ),
    (
        "Trigger::WhenAPermanentBecomesTappedToPayATeamworkCost",
        ObjectEvent::TappedForTeamwork,
    ),
    (
        "Trigger::WhenTheFinalChapterOfASagaResolves",
        ObjectEvent::FinalChapterResolves,
    ),
    (
        "Trigger::WhenTheFinalChapterOfASagaTriggers",
        ObjectEvent::FinalChapterTriggers,
    ),
];

const PAIR_EVENTS: &[(&str, PairEvent)] = &[
    ("Trigger::WhenACreatureBlocksACreature", PairEvent::Blocks),
    (
        "Trigger::WhenACreatureBecomesBlockedByACreature",
        PairEvent::BecomesBlockedBy,
    ),
    ("Trigger::WhenACreatureCrewsAVehicle", PairEvent::Crews),
    ("Trigger::WhenACreatureSaddlesAMount", PairEvent::Saddles),
    (
        "Trigger::WhenAPermanentBecomesAttachedToAPermanent",
        PairEvent::BecomesAttachedTo,
    ),
    (
        "Trigger::WhenAPermanentExploitsAPermanent",
        PairEvent::Exploits,
    ),
    ("Trigger::WhenACreatureEnlistsACreature", PairEvent::Enlists),
    ("Trigger::WhenACreatureMentorsACreature", PairEvent::Mentors),
    (
        "Trigger::WhenAPermanentBecomesUnattachedFromAPermanent",
        PairEvent::BecomesUnattachedFrom,
    ),
    (
        "Trigger::WhenAPermanentIsChampionedWithAPermanent",
        PairEvent::ChampionedWith,
    ),
    (
        "Trigger::WhenAPermanentStationsAPermanent",
        PairEvent::Stations,
    ),
    (
        "Trigger::WhenAPermanentEntersTheBattlefieldAttachedToAPermanent",
        PairEvent::EntersAttachedTo,
    ),
    (
        "Trigger::WhenAPermanentTransformsFromIntoAPermanent",
        PairEvent::TransformsInto,
    ),
];

const TARGETING_EVENTS: &[(&str, TargetingKind)] = &[
    (
        "Trigger::WhenAPermanentBecomesTheTargetOfASpellOrAbility",
        TargetingKind::SpellOrAbility,
    ),
    (
        "Trigger::WhenAPermanentBecomesTheTargetOfASpell",
        TargetingKind::Spell,
    ),
    (
        "Trigger::WhenAPermanentBecomesTheTargetOfAnAbility",
        TargetingKind::Ability,
    ),
];

/// What an event says about objects, when a plain filter can say it.
/// A filter on the stack, from whichever of the three families wrote it.
fn stack_filter(operand: &Operand) -> Option<ObjectFilter> {
    use StackTrait as T;
    let (family, form, operands): (&str, &str, &[Operand]) = match operand {
        Operand::Term {
            family,
            form,
            operands,
        } => (family, form, operands),
        Operand::Vocabulary { family, form } => (family, form, &[]),
        _ => return None,
    };
    if !matches!(
        family,
        "Abilities" | "ActivatedAbilities" | "SpellsAndAbilities"
    ) {
        return None;
    }
    let is = |value| ObjectFilter::OnStack(value);
    let not = |value| ObjectFilter::Not(Box::new(ObjectFilter::OnStack(value)));
    let keyword = |value| ObjectFilter::OnStack(T::Keyword(value));
    let list = || -> Option<Vec<ObjectFilter>> {
        let mut out = vec![];
        for operand in operands {
            match operand {
                Operand::List(items) => {
                    for item in items {
                        out.push(stack_filter(item)?);
                    }
                }
                _ => out.push(stack_filter(operand)?),
            }
        }
        Some(out)
    };
    let objects = |operand: &Operand| match operand {
        Operand::Objects(objects) => Some(objects_filter(objects)),
        Operand::Filter(filter) => Some(filter.clone()),
        Operand::Object(object) => Some(ObjectFilter::SameObject(object.clone())),
        _ => None,
    };
    let target = |recipients| ObjectFilter::TargetsAny(Box::new(recipients));
    Some(match (form, operands) {
        ("And", _) => ObjectFilter::All(list()?),
        ("Or", _) => ObjectFilter::Either(list()?),
        ("AnyAbility" | "AnySpellOrAbility", []) => ObjectFilter::Any,
        ("TriggeredAbility", []) => is(T::Triggered),
        ("ActivatedAbility", []) => is(T::Activated),
        ("ManaAbility", []) => is(T::Mana),
        ("NonManaAbility", []) => not(T::Mana),
        ("LoyaltyAbility", []) => is(T::Loyalty),
        ("NonLoyaltyAbility", []) => not(T::Loyalty),
        ("HasXInCost", []) => is(T::XInCost),
        ("HasTapSelfInCost", []) => is(T::TapSymbolInCost),
        ("DoesntHaveTapSelfInCost", []) => not(T::TapSymbolInCost),
        ("HasASingleTarget", []) => is(T::SingleTarget),
        ("BackupAbility", []) => keyword(AbilityKeyword::Backup),
        ("BoastAbility", []) => keyword(AbilityKeyword::Boast),
        ("EmbalmAbility", []) => keyword(AbilityKeyword::Embalm),
        ("EquipAbility", []) => keyword(AbilityKeyword::Equip),
        ("EternalizeAbility", []) => keyword(AbilityKeyword::Eternalize),
        ("ExhaustAbility", []) => keyword(AbilityKeyword::Exhaust),
        ("ModularAbility", []) => keyword(AbilityKeyword::Modular),
        ("NinjutsuAbility", []) => keyword(AbilityKeyword::Ninjutsu),
        ("OutlastAbility", []) => keyword(AbilityKeyword::Outlast),
        ("PowerUpAbility", []) => keyword(AbilityKeyword::PowerUp),
        ("AbilityOfPermanent" | "AbilityOfAPermanent" | "AbilityOfASource", [source]) => {
            ObjectFilter::AbilityOf(Box::new(objects(source)?))
        }
        ("NotAnAbilityOfAPermanent", [source]) => ObjectFilter::Not(Box::new(
            ObjectFilter::AbilityOf(Box::new(objects(source)?)),
        )),
        ("ControlledByAPlayer", [Operand::Players(players)]) => {
            ObjectFilter::ControlledBy(players.clone())
        }
        ("IsSpell", [Operand::Filter(spells)]) => {
            ObjectFilter::All(vec![is(T::Spell), spells.clone()])
        }
        ("TargetsAPermanent", [Operand::Objects(set)]) => target(Recipients::Objects(set.clone())),
        ("TargetsPermanent", [Operand::Object(object)]) => {
            target(Recipients::Objects(ObjectSet::One(object.clone())))
        }
        ("TargetsAPlayer", [Operand::Players(players)]) => {
            target(Recipients::Players(players.clone()))
        }
        ("TargetsPlayer", [Operand::Player(player)]) => {
            target(Recipients::Players(PlayerSet::One(player.clone())))
        }
        _ => return None,
    })
}

fn event_filter(objects: &ObjectSet) -> Option<ObjectFilter> {
    match objects {
        ObjectSet::Battlefield(filter) => Some(filter.clone()),
        ObjectSet::One(object) => Some(ObjectFilter::SameObject(object.clone())),
        _ => None,
    }
}

/// An attack in generic form. A batch names how the attackers group.
fn reduce_attack(name: &str, operands: &[Operand]) -> Option<EventSpec> {
    use AttackDefenders as D;
    use Operand::{Comparison as Compare, Objects, Players};
    let attack = |attackers: Option<&ObjectSet>, by: Option<&PlayerSet>, defenders| {
        Some(EventSpec::Attack {
            attackers: attackers.map_or(Some(ObjectFilter::Any), event_filter)?,
            by: by.cloned(),
            defenders,
        })
    };
    let batch = |event: Option<EventSpec>, per, count: Option<(&Comparison, &Expr)>| {
        Some(EventSpec::Batch {
            event: Box::new(event?),
            per,
            count: count.map(|(comparison, value)| (comparison.clone(), value.clone())),
            including: None,
        })
    };
    let to = |players: &PlayerSet| D::Players(players.clone());
    let or_walkers = |players: &PlayerSet| D::PlayersAndPlaneswalkers(players.clone());
    match (name.strip_prefix("Trigger::")?, operands) {
        ("WhenACreatureAttacksAPlayer", [Objects(a), Players(d)]) => attack(Some(a), None, to(d)),
        ("WhenACreatureAttacksAPlayerOrPlaneswalkerTheyControl", [Objects(a), Players(d)]) => {
            attack(Some(a), None, or_walkers(d))
        }
        (
            "WhenACreatureAttacksAPlaneswalker" | "WhenACreatureAttacksABattle",
            [Objects(a), Objects(d)],
        ) => attack(Some(a), None, D::Objects(event_filter(d)?)),
        ("WhenAPlayerAttacksWithACreature", [Players(p), Objects(a)]) => {
            attack(Some(a), Some(p), D::Any)
        }
        ("WhenAnyNumberOfCreaturesAttack", [Objects(a)]) => {
            batch(attack(Some(a), None, D::Any), BatchPer::Whole, None)
        }
        ("WhenAnyNumberOfCreaturesAttackAPlayer", [Objects(a), Players(d)]) => {
            batch(attack(Some(a), None, to(d)), BatchPer::Defender, None)
        }
        (
            "WhenAnyNumberOfCreaturesAttackAPlayerOrPlaneswalkerTheyControl",
            [Objects(a), Players(d)],
        ) => batch(
            attack(Some(a), None, or_walkers(d)),
            BatchPer::Defender,
            None,
        ),
        ("WhenAPlayerAttacksWithAnyNumberOfCreatures", [Players(p), Objects(a)]) => {
            batch(attack(Some(a), Some(p), D::Any), BatchPer::Whole, None)
        }
        ("WhenAPlayerAttacksWithANumberOfCreatures", [Players(p), Compare(c, v), Objects(a)]) => {
            batch(
                attack(Some(a), Some(p), D::Any),
                BatchPer::Whole,
                Some((c, v)),
            )
        }
        ("WhenANumberOfCreaturesAttack", [Compare(c, v), Objects(a)]) => {
            batch(attack(Some(a), None, D::Any), BatchPer::Whole, Some((c, v)))
        }
        ("WhenANumberOfCreaturesAttackAPlayer", [Compare(c, v), Objects(a), Players(d)]) => batch(
            attack(Some(a), None, to(d)),
            BatchPer::Defender,
            Some((c, v)),
        ),
        ("WhenAPlayerAttacksAPlayer", [Players(p), Players(d)]) => {
            batch(attack(None, Some(p), to(d)), BatchPer::Defender, None)
        }
        ("WhenAPlayerAttacksAPlayerAndOrPlaneswalkerTheyControl", [Players(p), Players(d)]) => {
            batch(
                attack(None, Some(p), or_walkers(d)),
                BatchPer::Defender,
                None,
            )
        }
        (
            "WhenAPlayerAttacksAPlayerWithAnyNumberOfCreatures",
            [Players(p), Players(d), Objects(a)],
        ) => batch(attack(Some(a), Some(p), to(d)), BatchPer::Defender, None),
        (
            "WhenAPlayerAttacksAPlayerWithANumberOfCreatures",
            [Players(p), Players(d), Compare(c, v), Objects(a)],
        ) => batch(
            attack(Some(a), Some(p), to(d)),
            BatchPer::Defender,
            Some((c, v)),
        ),
        (
            "WhenASingleCreatureAndANumberOfOtherCreaturesAttack",
            [Operand::Object(one), Compare(c, v), Objects(a)],
        ) => {
            let Some(EventSpec::Batch {
                event, per, count, ..
            }) = batch(attack(Some(a), None, D::Any), BatchPer::Whole, Some((c, v)))
            else {
                return None;
            };
            Some(EventSpec::Batch {
                event,
                per,
                count,
                including: Some(one.clone()),
            })
        }
        _ => None,
    }
}

/// A counter's kind, read from its operand.
fn counter_kind(operand: &Operand) -> Option<crate::counter::CounterKind> {
    use crate::counter::CounterKind;
    match operand {
        Operand::Term {
            family,
            form,
            operands,
        } if family == "CounterType" && form == "PTCounter" => match operands.as_slice() {
            [Operand::Integer(power), Operand::Integer(toughness)] => {
                Some(CounterKind::PowerToughness(
                    (*power).try_into().ok()?,
                    (*toughness).try_into().ok()?,
                ))
            }
            _ => None,
        },
        Operand::Vocabulary { family, form } if family == "CounterType" => {
            let value = serde_json::from_value(serde_json::Value::String(form.clone())).ok()?;
            Some(crate::counter::lower(&value))
        }
        _ => None,
    }
}

/// A counter event in generic form. "Any number of" is a batch for each object.
fn reduce_counters(name: &str, operands: &[Operand]) -> Option<EventSpec> {
    use CounterChange::{LastRemoved, Put, Removed};
    let (change, batched, whole) = match name.strip_prefix("Trigger::")? {
        "WhenACounterOfTypeIsPutOnAPermanent" | "WhenAPlayerPutsACounterOfTypeOnAPermanent" => {
            (Put, false, false)
        }
        "WhenAnyNumberOfCountersOfTypeArePutOnAPermanent"
        | "WhenAnyNumberOfCountersArePutOnAPermanent"
        | "WhenAPlayerPutsAnyNumberOfCountersOfTypeOnAPermanent"
        | "WhenAPlayerPutsAnyNumberOfCountersOnAPermanent" => (Put, true, false),
        "WhenAnyNumberOfCountersOfTypeArePutOnAnyNumberOfPermanents"
        | "WhenAPlayerPutsAnyNumberOfCountersOfTypeOnAnyNumberOfPermanents" => (Put, true, true),
        "WhenACounterOfTypeIsRemovedFromAPermanent"
        | "WhenACounterOfTypeIsRemovedFromAnExiledCard" => (Removed, false, false),
        "WhenAnyNumberOfCountersOfTypeAreRemovedFromAPermanent"
        | "WhenAnyNumberOfCountersAreRemovedFromAPermanent" => (Removed, true, false),
        "WhenTheLastCounterOfTypeIsRemovedFromAPermanent"
        | "WhenAPlayerRemovesTheLastCounterOfTypeFromAPermanent"
        | "WhenTheLastCounterOfTypeIsRemovedFromAnExiledCard" => (LastRemoved, false, false),
        _ => return None,
    };
    let (by, kind, objects) = match operands {
        [Operand::Objects(objects)] => (None, None, objects),
        [kind, Operand::Objects(objects)] if counter_kind(kind).is_some() => {
            (None, counter_kind(kind), objects)
        }
        [Operand::Players(by), Operand::Objects(objects)] => (Some(by.clone()), None, objects),
        [Operand::Players(by), kind, Operand::Objects(objects)] => {
            (Some(by.clone()), Some(counter_kind(kind)?), objects)
        }
        _ => return None,
    };
    let event = EventSpec::Counters {
        change,
        kind,
        objects: event_filter(objects)?,
        by,
    };
    Some(if batched {
        EventSpec::Batch {
            event: Box::new(event),
            per: if whole {
                BatchPer::Whole
            } else {
                BatchPer::Subject
            },
            count: None,
            including: None,
        }
    } else {
        event
    })
}

/// An event where any number of objects share one occurrence.
fn reduce_batch(name: &str, operands: &[Operand]) -> Option<EventSpec> {
    let form = name.strip_prefix("Trigger::")?;
    let whole = |event| {
        Some(EventSpec::Batch {
            event: Box::new(event),
            per: BatchPer::Whole,
            count: None,
            including: None,
        })
    };
    let moved = |objects: ObjectFilter, from, to| EventSpec::ZoneChange { objects, from, to };
    let owned = |cards: &ObjectFilter, owners: &PlayerSet| {
        ObjectFilter::All(vec![cards.clone(), ObjectFilter::OwnedBy(owners.clone())])
    };
    match (form, operands) {
        ("WhenAnyNumberOfGraveyardCardsLeave", [Operand::Filter(cards)]) => {
            return whole(EventSpec::Object {
                event: ObjectEvent::LeavesGraveyard,
                objects: cards.clone(),
            })
        }
        (
            "WhenACreatureBlocksAnyNumberOfCreatures",
            [Operand::Objects(blockers), Operand::Objects(blocked)],
        )
        | (
            "WhenACreatureBecomesBlockedByAnyNumberOfCreatures",
            [Operand::Objects(blockers), Operand::Objects(blocked)],
        ) => {
            return Some(EventSpec::Batch {
                event: Box::new(EventSpec::Pair {
                    event: if form == "WhenACreatureBlocksAnyNumberOfCreatures" {
                        PairEvent::Blocks
                    } else {
                        PairEvent::BecomesBlockedBy
                    },
                    objects: event_filter(blockers)?,
                    others: event_filter(blocked)?,
                }),
                per: BatchPer::Subject,
                count: None,
                including: None,
            })
        }
        (
            "WhenACreatureBlocksANumberOfCreatures",
            [Operand::Objects(blockers), Operand::Comparison(comparison, value), Operand::Objects(blocked)],
        )
        | (
            "WhenACreatureBecomesBlockedByANumberOfCreatures",
            [Operand::Objects(blockers), Operand::Comparison(comparison, value), Operand::Objects(blocked)],
        ) => {
            return Some(EventSpec::Batch {
                event: Box::new(EventSpec::Pair {
                    event: if form == "WhenACreatureBlocksANumberOfCreatures" {
                        PairEvent::Blocks
                    } else {
                        PairEvent::BecomesBlockedBy
                    },
                    objects: event_filter(blockers)?,
                    others: event_filter(blocked)?,
                }),
                per: BatchPer::Subject,
                count: Some((comparison.clone(), value.clone())),
                including: None,
            })
        }
        (
            "WhenAPlayerCreatesAnyNumberOfTokens",
            [Operand::Players(players), Operand::Objects(tokens)],
        ) => {
            return whole(EventSpec::Player {
                event: PlayerEvent::CreatesToken,
                players: players.clone(),
                objects: Some(event_filter(tokens)?),
            })
        }
        ("WhenAnyNumberOfPlayersLoseLife", [Operand::Players(players)]) => {
            return whole(EventSpec::Player {
                event: PlayerEvent::LosesLife,
                players: players.clone(),
                objects: None,
            })
        }
        (
            "WhenAnyNumberOfCardsArePutIntoAPlayersLibraryFromAnywhere",
            [Operand::Players(owners)],
        ) => {
            return whole(moved(
                ObjectFilter::OwnedBy(owners.clone()),
                None,
                EventZone::Library,
            ))
        }
        (
            "WhenAnyNumberOfGenericCardsArePutIntoExileFromAPlayersHand",
            [Operand::Players(owners)],
        ) => {
            return whole(moved(
                ObjectFilter::OwnedBy(owners.clone()),
                Some(EventZone::Hand),
                EventZone::Exile,
            ))
        }
        ("WhenAnyNumberOfSpecificCardsAreMilled", [Operand::Filter(cards)]) => {
            return whole(EventSpec::Player {
                event: PlayerEvent::MillsCard,
                players: PlayerSet::All,
                objects: Some(cards.clone()),
            })
        }
        (
            "WhenAPlayerMillsAnyNumberOfSpecificCards",
            [Operand::Players(players), Operand::Filter(cards)],
        ) => {
            return whole(EventSpec::Player {
                event: PlayerEvent::MillsCard,
                players: players.clone(),
                objects: Some(cards.clone()),
            })
        }
        (
            "WhenAnyNumberOfCardsArePutIntoExileFromAPlayersGraveyard",
            [Operand::Filter(cards), Operand::Players(owners)],
        ) => {
            return whole(moved(
                owned(cards, owners),
                Some(EventZone::Graveyard),
                EventZone::Exile,
            ))
        }
        ("WhenAnyNumberOfCardsArePutIntoExile", [Operand::Filter(cards)]) => {
            return whole(moved(cards.clone(), None, EventZone::Exile))
        }
        (
            "WhenAnyNumberOfCardsArePutIntoAPlayersGraveyardFromTheirLibrary",
            [Operand::Filter(cards), Operand::Players(owners)],
        ) => {
            return whole(moved(
                owned(cards, owners),
                Some(EventZone::Library),
                EventZone::Graveyard,
            ))
        }
        (
            "WhenAnyNumberOfCardsArePutIntoAPlayersGraveyardFromAnywhere",
            [Operand::Filter(cards), Operand::Players(owners)],
        ) => {
            let cards = ObjectFilter::All(vec![
                owned(cards, owners),
                ObjectFilter::Not(Box::new(ObjectFilter::Token)),
            ]);
            return whole(moved(cards, None, EventZone::Graveyard));
        }
        _ => {}
    }
    let [Operand::Objects(objects)] = operands else {
        return None;
    };
    let objects = event_filter(objects)?;
    let object = |event, objects| EventSpec::Object { event, objects };
    whole(match form {
        "WhenAnyNumberOfPermanentsEnterTheBattlefield" => {
            moved(objects, None, EventZone::Battlefield)
        }
        "WhenAnyNumberOfCreaturesOrPlaneswalkersDie" => {
            moved(objects, Some(EventZone::Battlefield), EventZone::Graveyard)
        }
        "WhenAnyNumberOfPermanentsLeaveTheBattlefield" => {
            object(ObjectEvent::LeavesBattlefield, objects)
        }
        "WhenAnyNumberOfPermanentsBecomeTapped" => object(ObjectEvent::BecomesTapped, objects),
        "WhenAnyNumberOfCreaturesBecomeBlocked" => object(ObjectEvent::BecomesBlocked, objects),
        "WhenAnyNumberOfCreaturesBlock" => object(ObjectEvent::Blocks, objects),
        "WhenAnyNumberOfPermanentsLeaveTheBattlefieldWithoutDying" => {
            object(ObjectEvent::LeavesWithoutDying, objects)
        }
        "WhenAnyNumberOfPermanentsPhaseOut" => object(ObjectEvent::PhasesOut, objects),
        "WhenAnyNumberOfPermanentsFight" => object(ObjectEvent::Fights, objects),
        "WhenAnyNumberOfPermanentsAreReturnedToHand" => {
            moved(objects, Some(EventZone::Battlefield), EventZone::Hand)
        }
        _ => return None,
    })
}

/// `cards` owned by one of `owners`, for an event that names whose zone it is.
fn owned_by(cards: ObjectFilter, owners: &PlayerSet) -> ObjectFilter {
    ObjectFilter::All(vec![cards, ObjectFilter::OwnedBy(owners.clone())])
}

/// A plane, scheme or phenomenon event's card, when it is this card.
fn this_card(operand: &Operand) -> bool {
    matches!(operand, Operand::Term { family, form, .. }
    if matches!(
        (family.as_str(), form.as_str()),
        ("Schemes", "SingleScheme") | ("Planes", "SinglePlane") | ("Phenomena", "SinglePhenomenon")
    ))
}

fn reduce_object_event(name: &str, operands: &[Operand]) -> Option<EventSpec> {
    match operands {
        [Operand::Objects(objects)] => {
            let filter = event_filter(objects)?;
            let enters = |extra| {
                Some(EventSpec::ZoneChange {
                    objects: ObjectFilter::All(vec![filter.clone(), extra]),
                    from: None,
                    to: EventZone::Battlefield,
                })
            };
            match name {
                "Trigger::WhenAPermanentIsExiled" => {
                    return Some(EventSpec::ZoneChange {
                        objects: filter,
                        from: Some(EventZone::Battlefield),
                        to: EventZone::Exile,
                    })
                }
                "Trigger::WhenAPermanentIsPutIntoTheCommandZone" => {
                    return Some(EventSpec::ZoneChange {
                        objects: filter,
                        from: Some(EventZone::Battlefield),
                        to: EventZone::Command,
                    })
                }
                "Trigger::WhenAPermanentEntersTheBattlefieldTapped" => {
                    return enters(ObjectFilter::Tapped)
                }
                "Trigger::WhenAPermanentEntersTheBattlefieldUntapped" => {
                    return enters(ObjectFilter::Not(Box::new(ObjectFilter::Tapped)))
                }
                "Trigger::WhenAPermanentEntersTheBattlefieldAttacking" => {
                    return enters(ObjectFilter::Attacking)
                }
                _ => {}
            }
            let event = OBJECT_EVENTS.iter().find(|(row, _)| *row == name)?.1;
            Some(EventSpec::Object {
                event,
                objects: filter,
            })
        }
        [Operand::Filter(cards)] => match name {
            "Trigger::WhenAGraveyardCardLeaves" => Some(EventSpec::Object {
                event: ObjectEvent::LeavesGraveyard,
                objects: cards.clone(),
            }),
            "Trigger::WhenAGraveyardCardSpecializes" => Some(EventSpec::Object {
                event: ObjectEvent::Specializes,
                objects: cards.clone(),
            }),
            "Trigger::WhenAGraveyardCardIsPutIntoHand" => Some(EventSpec::ZoneChange {
                objects: cards.clone(),
                from: Some(EventZone::Graveyard),
                to: EventZone::Hand,
            }),
            _ => None,
        },
        [Operand::Filter(cards), Operand::Players(owners)]
            if name == "Trigger::WhenACardIsPutIntoAPlayersGraveyardFromTheirHand" =>
        {
            Some(EventSpec::ZoneChange {
                objects: owned_by(cards.clone(), owners),
                from: Some(EventZone::Hand),
                to: EventZone::Graveyard,
            })
        }
        [Operand::Objects(objects), Operand::Players(owners)]
            if name == "Trigger::WhenAPermanentEntersTheBattlefieldFromAPlayersGraveyard"
                || name == "Trigger::WhenAPermanentEntersTheBattlefieldFromAPlayersHand"
                || name == "Trigger::WhenAPermanentIsReturnedToAPlayersHand" =>
        {
            let (from, to) = match name {
                "Trigger::WhenAPermanentEntersTheBattlefieldFromAPlayersGraveyard" => {
                    (EventZone::Graveyard, EventZone::Battlefield)
                }
                "Trigger::WhenAPermanentEntersTheBattlefieldFromAPlayersHand" => {
                    (EventZone::Hand, EventZone::Battlefield)
                }
                _ => (EventZone::Battlefield, EventZone::Hand),
            };
            Some(EventSpec::ZoneChange {
                objects: owned_by(event_filter(objects)?, owners),
                from: Some(from),
                to,
            })
        }
        [Operand::Objects(objects), Operand::Objects(others)] => Some(EventSpec::Pair {
            event: PAIR_EVENTS.iter().find(|(row, _)| *row == name)?.1,
            objects: event_filter(objects)?,
            others: event_filter(others)?,
        }),
        [Operand::Objects(objects), by] => Some(EventSpec::BecomesTarget {
            objects: event_filter(objects)?,
            kind: TARGETING_EVENTS.iter().find(|(row, _)| *row == name)?.1,
            by: Box::new(by.clone()),
        }),
        _ => None,
    }
}

fn graveyard_of(player: &PlayerRef, cards: ObjectFilter) -> ObjectSet {
    ObjectSet::Zone {
        scope: ZoneScope::Players {
            zone: EventZone::Graveyard,
            players: PlayerSet::One(player.clone()),
        },
        kind: ObjectKind::Card,
        filter: cards,
    }
}

fn rounded_up(number: &Expr, denominator: i64) -> Expr {
    Expr::Divide {
        numerator: Box::new(number.clone()),
        denominator: Box::new(Expr::Constant(denominator)),
        rounding: Rounding::Up,
    }
}

fn property_of(object: &ObjectRef, property: Property) -> Expr {
    Expr::ObjectProperty {
        object: object.clone(),
        property,
        information: Information::CurrentOrLastKnown,
    }
}

fn among(players: &PlayerSet, number: PlayerNumber, aggregation: Aggregation) -> Expr {
    Expr::AmongPlayers {
        players: players.clone(),
        number,
        aggregation,
    }
}

fn aggregate(objects: &ObjectSet, property: Property, aggregation: Aggregation) -> Expr {
    Expr::Aggregate {
        objects: Box::new(objects.clone()),
        property,
        aggregation,
    }
}

fn distinct(objects: &ObjectSet, property: DistinctProperty) -> Expr {
    Expr::CountDistinct {
        objects: Box::new(objects.clone()),
        property,
    }
}

/// A row's number in generic form, or the row itself when it has none.
pub fn reduce_number(name: String, operands: Vec<Operand>) -> Expr {
    if let Some(number) = name
        .strip_prefix("GameNumber::")
        .and_then(|form| reduce_event_number(form, &operands))
    {
        return number;
    }
    match (name.as_str(), operands.as_slice()) {
        ("GameNumber::TheChosenNumber", []) => Expr::ChosenNumber,
        ("GameNumber::TheHighestNumberChosen", []) => among(
            &PlayerSet::All,
            PlayerNumber::ChosenNumber,
            Aggregation::Maximum,
        ),
        ("GameNumber::NumCreaturesInPlayersParty", [Operand::Player(player)]) => {
            Expr::PartySize(player.clone())
        }
        ("GameNumber::StartingLifeTotalOfPlayer", [Operand::Player(player)]) => {
            Expr::StartingLife(player.clone())
        }
        (
            "GameNumber::NumTimesSpellWasKicked" | "GameNumber::TheNumberOfTimesPermanentWasKicked",
            [Operand::Object(object)],
        ) => Expr::TimesKicked(object.clone()),
        ("GameNumber::ThirdRoundedUp", [Operand::Number(number)]) => rounded_up(number, 3),
        ("GameNumber::TenthRoundedUp", [Operand::Number(number)]) => rounded_up(number, 10),
        ("GameNumber::TheNumberOfColorsSpellIs", [Operand::Object(object)]) => {
            property_of(object, Property::ColorCount)
        }
        (
            "GameNumber::IntensityOfPermanent" | "GameNumber::IntensityOfSpell",
            [Operand::Object(object)],
        ) => property_of(object, Property::Intensity),
        ("GameNumber::SpeedOfPlayer", [Operand::Player(player)]) => Expr::PlayerNumber {
            player: player.clone(),
            number: PlayerNumber::Speed,
        },
        ("GameNumber::TheAmountOfEnergyPlayerHas", [Operand::Player(player)]) => {
            Expr::PlayerNumber {
                player: player.clone(),
                number: PlayerNumber::Energy,
            }
        }
        (
            "GameNumber::HighestLifeTotalAmongPlayers"
            | "GameNumber::TheHighestLifeTotalAmongPlayers",
            [Operand::Players(players)],
        ) => among(players, PlayerNumber::Life, Aggregation::Maximum),
        ("GameNumber::LowestLifeTotalAmongPlayers", [Operand::Players(players)]) => {
            among(players, PlayerNumber::Life, Aggregation::Minimum)
        }
        ("GameNumber::TheHighestNumberOfCardsInHandAmongPlayers", [Operand::Players(players)]) => {
            among(players, PlayerNumber::CardsInHand, Aggregation::Maximum)
        }
        ("GameNumber::TheLowestNumberOfCardsInHandAmongPlayers", [Operand::Players(players)]) => {
            among(players, PlayerNumber::CardsInHand, Aggregation::Minimum)
        }
        (
            "GameNumber::TheGreatestNumberOfPermanentsControlledAmongPlayers",
            [Operand::Objects(objects), Operand::Players(players)],
        ) if event_filter(objects).is_some() => among(
            players,
            PlayerNumber::Permanents(Box::new(event_filter(objects).unwrap())),
            Aggregation::Maximum,
        ),
        (
            "GameNumber::TheLowestNumberOfPermanentsControlledAmongPlayers",
            [Operand::Objects(objects), Operand::Players(players)],
        ) if event_filter(objects).is_some() => among(
            players,
            PlayerNumber::Permanents(Box::new(event_filter(objects).unwrap())),
            Aggregation::Minimum,
        ),
        ("GameNumber::TheLeastPowerAmongPermanents", [Operand::Objects(objects)]) => {
            aggregate(objects, Property::Power, Aggregation::Minimum)
        }
        ("GameNumber::TheLeastToughnessAmongPermanents", [Operand::Objects(objects)]) => {
            aggregate(objects, Property::Toughness, Aggregation::Minimum)
        }
        ("GameNumber::TheGreatestPowerOrToughnessAmongPermanents", [Operand::Objects(objects)]) => {
            Expr::Maximum(
                Box::new(aggregate(objects, Property::Power, Aggregation::Maximum)),
                Box::new(aggregate(
                    objects,
                    Property::Toughness,
                    Aggregation::Maximum,
                )),
            )
        }
        (
            "GameNumber::NumDifferentlyPoweredCreaturesAmongPermanents",
            [Operand::Objects(objects)],
        ) => distinct(objects, DistinctProperty::Powers),
        (
            "GameNumber::NumDifferentManaValuesAmongPermanents"
            | "GameNumber::NumDifferentManaValuesAmongCardsInExile",
            [Operand::Objects(objects)],
        ) => distinct(objects, DistinctProperty::ManaValues),
        ("GameNumber::TheNumberOfCardTypesAmongPermanents", [Operand::Objects(objects)]) => {
            distinct(objects, DistinctProperty::CardTypes)
        }
        (
            "GameNumber::TheNumberOfPlaneswalkerTypesAmongPermanents",
            [Operand::Objects(objects)],
        ) => distinct(objects, DistinctProperty::PlaneswalkerTypes),
        ("GameNumber::TheNumberOfCounterTypesAmongPermanents", [Operand::Objects(objects)]) => {
            distinct(objects, DistinctProperty::CounterTypes)
        }
        ("GameNumber::TheNumberOfCardTypesPermanentHas", [Operand::Object(object)]) => {
            distinct(&ObjectSet::One(object.clone()), DistinctProperty::CardTypes)
        }
        ("GameNumber::NumEnteredTheBattlefieldThisTurn", [Operand::Objects(objects)])
            if event_filter(objects).is_some() =>
        {
            Expr::EventHistory {
                event: Box::new(EventSpec::ZoneChange {
                    objects: event_filter(objects).unwrap(),
                    from: None,
                    to: EventZone::Battlefield,
                }),
                window: HistoryWindow::ThisTurn,
                measure: HistoryMeasure::Occurrences,
            }
        }
        ("GameNumber::TheNumberOfVotesForWord", [Operand::Text(word)]) => {
            Expr::Votes(VoteOption::Word(word.clone()))
        }
        ("GameNumber::TheNumberOfVotesReceivedByPlayer", [Operand::Player(player)]) => {
            Expr::Votes(VoteOption::Player(player.clone()))
        }
        ("GameNumber::TheNumberOfVotesReceivedByPermanent", [Operand::Object(object)]) => {
            Expr::Votes(VoteOption::Object(object.clone()))
        }
        ("GameNumber::TheNumberOfTimesCreatureAttackedThisGame", [Operand::Object(object)]) => {
            Expr::EventHistory {
                event: Box::new(EventSpec::Object {
                    event: ObjectEvent::Attacks,
                    objects: ObjectFilter::SameObject(object.clone()),
                }),
                window: HistoryWindow::ThisGame,
                measure: HistoryMeasure::Occurrences,
            }
        }
        (
            "GameNumber::NumTimesPlayerHasCastTheirCommanderFromCommandZone"
            | "GameNumber::NumTimesPlayerHasCastACommanderFromCommandZone"
            | "GameNumber::NumTimesPlayerHasACastACommanderFromCommandZone",
            [Operand::Player(player)],
        ) => Expr::EventHistory {
            event: Box::new(EventSpec::Filtered {
                event: Box::new(EventSpec::Spell {
                    event: SpellEvent::Cast,
                    players: PlayerSet::One(player.clone()),
                    spells: ObjectFilter::State(ObjectState::Commander),
                }),
                condition: Condition::WasCastFrom {
                    object: ObjectRef::EventSubject,
                    zone: EventZone::Command,
                },
            }),
            window: HistoryWindow::ThisGame,
            measure: HistoryMeasure::Occurrences,
        },
        (
            "GameNumber::MinPermanentsControlledByAPlayer",
            [Operand::Objects(objects), Operand::Players(players)],
        ) if event_filter(objects).is_some() => among(
            players,
            PlayerNumber::Permanents(Box::new(event_filter(objects).unwrap())),
            Aggregation::Minimum,
        ),
        ("GameNumber::WouldCreateTokens_NumberTokens", []) => Expr::EventAmount,
        ("GameNumber::ManaValueOfTheCardDiscardedThisWay", [Operand::Result(result)]) => {
            property_of(
                &ObjectRef::Result {
                    result: result.clone(),
                    phase: ObjectPhase::After,
                    index: 0,
                },
                Property::ManaValue,
            )
        }
        ("GameNumber::AmountOfUnspentManaPlayerHas", [Operand::Player(player)]) => {
            Expr::PlayerNumber {
                player: player.clone(),
                number: PlayerNumber::UnspentMana,
            }
        }
        ("GameNumber::NumTimesCreatureHasMutated", [Operand::Object(object)]) => {
            Expr::TimesMutated(object.clone())
        }
        ("GameNumber::TheAmountOfEnergyPaidThisWay", [Operand::Result(result)]) => {
            Expr::Performed(result.clone())
        }
        (name_, _) => {
            let form = name_.split("::").nth(1).unwrap_or(name_);
            reduce_game_number(form, &operands).unwrap_or(Expr::Primitive { name, operands })
        }
    }
}

/// Say who did `event`, for history that asks about one player.
pub fn set_event_player(event: &mut EventSpec, player: PlayerSet) -> bool {
    match event {
        EventSpec::Primitive { operands, .. } => operands[0] = Operand::Players(player),
        EventSpec::Player { players, .. } | EventSpec::Activated { players, .. } => {
            *players = player
        }
        EventSpec::Attack { by, .. } | EventSpec::Counters { by, .. } => *by = Some(player),
        EventSpec::Batch { event, .. } => return set_event_player(event, player),
        _ => return false,
    }
    true
}

const REPLACED_PLAYER_EVENTS: &[(&str, ReplacedPlayerEvent)] = &[
    (
        "WouldDraw::APlayerWouldDrawACard",
        ReplacedPlayerEvent::Draw,
    ),
    (
        "WouldDraw::APlayerWouldDrawOneOrMoreCards",
        ReplacedPlayerEvent::DrawCards,
    ),
    (
        "WouldDraw::PlayerWouldDrawExceptFirstDrawStepDraw",
        ReplacedPlayerEvent::DrawExceptFirstInDrawStep,
    ),
    (
        "WouldDraw::APlayerWouldDrawExceptFirstDrawStepDraw",
        ReplacedPlayerEvent::DrawExceptFirstInDrawStep,
    ),
    (
        "WouldDraw::PlayerWouldDrawDuringTheirDrawStep",
        ReplacedPlayerEvent::DrawInDrawStep,
    ),
    (
        "WouldGainLife::PlayerWouldGainLife",
        ReplacedPlayerEvent::GainLife,
    ),
    (
        "WouldGainLife::APlayerWouldGainLife",
        ReplacedPlayerEvent::GainLife,
    ),
    (
        "WouldLoseLife::APlayerWouldLoseLife",
        ReplacedPlayerEvent::LoseLife,
    ),
    (
        "WouldLoseTheGame::PlayerWouldLoseTheGame",
        ReplacedPlayerEvent::LoseGame,
    ),
    (
        "WouldGetEnergy::APlayerWouldGetEnergy",
        ReplacedPlayerEvent::GetEnergy,
    ),
    (
        "WouldMill::APlayerWouldMillAnyNumberOfCards",
        ReplacedPlayerEvent::Mill,
    ),
    ("WouldScry::PlayerWouldScry", ReplacedPlayerEvent::Scry),
    ("WouldLearn::APlayerWouldLearn", ReplacedPlayerEvent::Learn),
    (
        "WouldProliferate::APlayerWouldProliferate",
        ReplacedPlayerEvent::Proliferate,
    ),
    (
        "WouldBeginATurn::APlayerWouldBeginAnExtraTurn",
        ReplacedPlayerEvent::BeginExtraTurn,
    ),
];

const REPLACED_OBJECT_EVENTS: &[(&str, ReplacedObjectEvent)] = &[
    (
        "WouldDestroy::PermanentWouldBeDestroyed",
        ReplacedObjectEvent::Destroyed,
    ),
    (
        "WouldLeaveTheBattlefield::PermanentWouldLeaveTheBattlefield",
        ReplacedObjectEvent::LeavesBattlefield,
    ),
    (
        "WouldUntap::APermanentWouldUntapDuringsItsControllersUntapStep",
        ReplacedObjectEvent::UntapsInUntapStep,
    ),
    (
        "WouldConnive::APermanentWouldConnive",
        ReplacedObjectEvent::Connives,
    ),
];

/// A row's would-be event in generic form, when the row is a plain event.
fn reduced_replacement(name: &str, operands: &[Operand]) -> Option<ReplacementEvent> {
    use Operand::{Object, Objects, Player, Players};
    let form = name.strip_prefix("ReplacableEvent")?;
    let two_or_more = |event| ReplacementEvent::Measured {
        event: Box::new(event),
        comparison: Comparison::GreaterOrEqual,
        amount: Expr::Constant(2),
    };
    if let Some((_, event)) = REPLACED_PLAYER_EVENTS.iter().find(|(row, _)| *row == form) {
        let players = match operands {
            [Players(players)] => players.clone(),
            [Player(player)] => PlayerSet::One(player.clone()),
            _ => return None,
        };
        let event = *event;
        return Some(ReplacementEvent::Player { event, players });
    }
    if let Some((_, event)) = REPLACED_OBJECT_EVENTS.iter().find(|(row, _)| *row == form) {
        let objects = match operands {
            [Objects(objects)] => event_filter(objects)?,
            [Object(object)] => ObjectFilter::SameObject(object.clone()),
            _ => return None,
        };
        let event = *event;
        return Some(ReplacementEvent::Object { event, objects });
    }
    let tokens = |tokens: &ObjectSet,
                  controllers: Option<&PlayerSet>,
                  creators: Option<&PlayerSet>,
                  effect_only,
                  each| {
        Some(ReplacementEvent::Tokens {
            tokens: event_filter(tokens)?,
            controllers: controllers.cloned(),
            creators: creators.cloned(),
            effect_only,
            each,
        })
    };
    let counters = |kind: Option<&Operand>,
                    objects: &ObjectSet,
                    players: Option<&PlayerSet>,
                    by: Option<&PlayerSet>,
                    effect_only| {
        Some(ReplacementEvent::Counters {
            kind: match kind {
                Some(kind) => Some(counter_kind(kind)?),
                None => None,
            },
            objects: event_filter(objects)?,
            players: players.cloned(),
            by: by.cloned(),
            effect_only,
        })
    };
    let mana = |sources: &ObjectSet, by: Option<&PlayerRef>| {
        Some(ReplacementEvent::ProduceMana {
            sources: event_filter(sources)?,
            by: by.cloned(),
        })
    };
    let (_, row) = form.split_once("::")?;
    match (row, operands) {
        ("APlayerWouldDrawTwoOrMoreCards", [Players(players)]) => {
            Some(two_or_more(ReplacementEvent::Player {
                event: ReplacedPlayerEvent::DrawCards,
                players: players.clone(),
            }))
        }
        ("AnyNumberOfTokensWouldBeCreatedUnderAPlayersControl", [Objects(t), Players(c)]) => {
            tokens(t, Some(c), None, false, false)
        }
        ("AnyNumberOfTokensWouldBeCreated", [Objects(t)]) => tokens(t, None, None, false, false),
        ("AnEffectWouldCreateAnyNumberOfTokensUnderAPlayersControl", [Objects(t), Players(c)]) => {
            tokens(t, Some(c), None, true, false)
        }
        ("AnEffectWouldCreateAnyNumberOfTokens", [Objects(t)]) => {
            tokens(t, None, None, true, false)
        }
        ("APlayerWouldCreateAToken", [Players(p), Objects(t)]) => {
            tokens(t, None, Some(p), false, true)
        }
        ("APlayerWouldCreateTokens", [Players(p), Objects(t)]) => {
            tokens(t, None, Some(p), false, false)
        }
        ("APlayerWouldCreateTokensForTheFirstTimeEachTurn", [Players(p), Objects(t)]) => {
            tokens(t, None, Some(p), false, false)
                .map(|event| ReplacementEvent::FirstEachTurn(Box::new(event)))
        }
        ("CountersOfTypeWouldBePointOnAPermanent", [kind, Objects(o)]) => {
            counters(Some(kind), o, None, None, false)
        }
        ("CountersWouldBePutOnAPermanent", [Objects(o)]) => counters(None, o, None, None, false),
        ("AnEffectWouldPutCountersOnAPermanent", [Objects(o)]) => {
            counters(None, o, None, None, true)
        }
        ("APlayerWouldPutCountersOnAPermanent", [Players(by), Objects(o)]) => {
            counters(None, o, None, Some(by), false)
        }
        ("APlayerWouldPutCountersOnAPermanentOrAPlayer", [Players(by), Objects(o), Players(p)]) => {
            counters(None, o, Some(p), Some(by), false)
        }
        (
            "APlayerWouldPutAnyNumberOfCountersOfTypeOnAPermanent",
            [Players(by), kind, Objects(o)],
        ) => counters(Some(kind), o, None, Some(by), false),
        ("TappingAPermanentWouldProduceMana", [Objects(s)]) => mana(s, None),
        ("PlayerTappingAPermanentWouldProduceMana", [Player(p), Objects(s)]) => mana(s, Some(p)),
        ("TappingAPermanentWouldProduceTwoOrMoreMana", [Objects(s)]) => {
            mana(s, None).map(two_or_more)
        }
        _ => None,
    }
}

/// A row's would-be event in generic form, or the row itself when it has none.
pub fn reduce_replacement(name: String, operands: Vec<Operand>) -> ReplacementEvent {
    reduced_replacement(&name, &operands).unwrap_or(ReplacementEvent::Primitive { name, operands })
}

/// Whether a would-be event is one a body stands in for unless it says otherwise.
pub fn body_replaces(event: &ReplacementEvent) -> bool {
    match event {
        ReplacementEvent::Primitive { .. }
        | ReplacementEvent::Player { .. }
        | ReplacementEvent::Object { .. }
        | ReplacementEvent::Tokens { .. }
        | ReplacementEvent::Counters { .. }
        | ReplacementEvent::ProduceMana { .. } => true,
        ReplacementEvent::Measured { event, .. }
        | ReplacementEvent::EachPoint(event)
        | ReplacementEvent::FirstEachTurn(event) => body_replaces(event),
        _ => false,
    }
}

/// A row's event in generic form, or the row itself when it has none.
pub fn reduce_event(name: String, operands: Vec<Operand>) -> EventSpec {
    if let Some(event) = name
        .strip_prefix("Trigger::")
        .and_then(|form| reduce_event_form(form, &operands))
    {
        return event;
    }
    match (name.as_str(), operands.as_slice()) {
        ("Trigger::WhenAPlayerActivatesAnAbility", [Operand::Players(players), abilities]) => {
            if let Some(abilities) = stack_filter(abilities) {
                return EventSpec::Activated {
                    players: players.clone(),
                    abilities,
                };
            }
        }
        ("Trigger::WhenCardSpecializes", [Operand::Vocabulary { form, .. }])
            if form == "ThisCard" =>
        {
            return EventSpec::Object {
                event: ObjectEvent::Specializes,
                objects: ObjectFilter::SameObject(ObjectRef::Source),
            }
        }
        (
            "Trigger::WhenAPlayerBecomesTheTargetOfASpellOrAbility"
            | "Trigger::WhenAPlayerBecomesTheTargetOfAnAbility",
            [Operand::Players(players), by],
        ) => {
            if let Some(by) = stack_filter(by) {
                return EventSpec::Player {
                    event: PlayerEvent::BecomesTarget,
                    players: players.clone(),
                    objects: Some(by),
                };
            }
        }
        (
            "Trigger::WhenAPlayerLosesLifeDuringTheirTurn"
            | "Trigger::WhenAPlayerGainsLifeDuringTheirTurn"
            | "Trigger::WhenAPlayerDrawsACardDuringTheirTurn",
            [Operand::Players(players)],
        ) => {
            let event = match name.as_str() {
                "Trigger::WhenAPlayerLosesLifeDuringTheirTurn" => PlayerEvent::LosesLife,
                "Trigger::WhenAPlayerGainsLifeDuringTheirTurn" => PlayerEvent::GainsLife,
                _ => PlayerEvent::Draws,
            };
            return EventSpec::Filtered {
                event: Box::new(EventSpec::Player {
                    event,
                    players: players.clone(),
                    objects: None,
                }),
                condition: Condition::PlayerIsActive(PlayerRef::EventSubject),
            };
        }
        (
            "Trigger::WhenAPlayerRollsAValueOnADie",
            [Operand::Players(players), Operand::Comparison(comparison, value)],
        ) => {
            return EventSpec::Filtered {
                event: Box::new(EventSpec::Player {
                    event: PlayerEvent::RollsADie,
                    players: players.clone(),
                    objects: None,
                }),
                condition: Condition::Compare {
                    left: Expr::EventAmount,
                    comparison: comparison.clone(),
                    right: value.clone(),
                },
            };
        }
        (
            "Trigger::WhenAPlayerIsAttacked" | "Trigger::WhenAnyNumberOfPlayersAreAttacked",
            [Operand::Players(players)],
        ) => {
            return EventSpec::Batch {
                event: Box::new(EventSpec::Attack {
                    attackers: ObjectFilter::Any,
                    by: None,
                    defenders: AttackDefenders::Players(players.clone()),
                }),
                per: if name == "Trigger::WhenAPlayerIsAttacked" {
                    BatchPer::Defender
                } else {
                    BatchPer::Whole
                },
                count: None,
                including: None,
            };
        }
        ("Trigger::WhenNoPermanentsAreOnTheBattlefield", [Operand::Objects(objects)]) => {
            if let Some(filter) = event_filter(objects) {
                return EventSpec::State {
                    condition: Condition::Compare {
                        left: Expr::CountObjects(Box::new(ObjectSet::Battlefield(filter))),
                        comparison: Comparison::Equal,
                        right: Expr::Constant(0),
                    },
                };
            }
        }
        (
            "Trigger::WhenAClassBecomesLevel",
            [Operand::Objects(objects), Operand::Number(level)],
        ) => {
            if let Some(objects) = event_filter(objects) {
                return EventSpec::ClassLevel {
                    objects,
                    level: level.clone(),
                };
            }
        }
        (
            "Trigger::WhenASpellOrAbilityCausesAPlayerToDiscardACard",
            [cause, Operand::Players(players), cards],
        ) => {
            let this_card = |operands: &[Operand]| matches!(operands, [Operand::Vocabulary { form, .. }] if form == "ThisCardInHand");
            let cards = match cards {
                Operand::Filter(cards) => Some((cards.clone(), false)),
                Operand::Vocabulary { form, .. } if form == "AnyCard" => {
                    Some((ObjectFilter::Any, false))
                }
                Operand::Term { form, operands, .. }
                    if form == "SingleCardInHand" && this_card(operands) =>
                {
                    Some((ObjectFilter::Any, true))
                }
                _ => None,
            };
            if let (Some(cause), Some((cards, source_only))) = (stack_filter(cause), cards) {
                return EventSpec::Discard {
                    players: players.clone(),
                    cards,
                    source_only,
                    kind: DiscardKind::CausedBy(cause),
                    grouping: DiscardGrouping::Card,
                };
            }
        }
        (
            "Trigger::WhenAPermanentEntersTheBattlefieldOrTheCreatureItHauntsDies",
            [Operand::Objects(objects)],
        ) => {
            if let Some(objects) = event_filter(objects) {
                return EventSpec::Either(vec![
                    EventSpec::ZoneChange {
                        objects,
                        from: None,
                        to: EventZone::Battlefield,
                    },
                    EventSpec::ZoneChange {
                        objects: ObjectFilter::State(ObjectState::HauntedBySource),
                        from: Some(EventZone::Battlefield),
                        to: EventZone::Graveyard,
                    },
                ]);
            }
        }
        (
            "Trigger::WhenAPlayerHasAnAmountOfLife",
            [Operand::Players(PlayerSet::One(player)), Operand::Comparison(comparison, value)],
        ) => {
            return EventSpec::State {
                condition: Condition::Compare {
                    left: Expr::PlayerLife(player.clone()),
                    comparison: comparison.clone(),
                    right: value.clone(),
                },
            }
        }
        ("Trigger::WhenAnAbilityIsActivated", [abilities]) => {
            if let Some(abilities) = stack_filter(abilities) {
                return EventSpec::Activated {
                    players: PlayerSet::All,
                    abilities,
                };
            }
        }
        _ => {}
    }
    let reduced = reduce_attack(&name, &operands)
        .or_else(|| reduce_counters(&name, &operands))
        .or_else(|| reduce_batch(&name, &operands))
        .or_else(|| reduce_object_event(&name, &operands));
    if let Some(event) = reduced {
        return event;
    }
    let event = PLAYER_EVENTS
        .iter()
        .find(|(row, _)| *row == name)
        .map(|(_, event)| *event);
    match (event, operands.as_slice()) {
        (Some(event), []) => {
            return EventSpec::Player {
                event,
                players: PlayerSet::All,
                objects: None,
            }
        }
        (Some(event), [Operand::Players(players), card]) if this_card(card) => {
            return EventSpec::Player {
                event,
                players: players.clone(),
                objects: Some(ObjectFilter::SameObject(ObjectRef::Source)),
            }
        }
        (Some(event), [Operand::Players(players)]) => {
            return EventSpec::Player {
                event,
                players: players.clone(),
                objects: None,
            }
        }
        (Some(event), [Operand::Players(players), Operand::Filter(cards)]) => {
            return EventSpec::Player {
                event,
                players: players.clone(),
                objects: Some(cards.clone()),
            }
        }
        (Some(event), [Operand::Objects(objects), Operand::Players(players)])
        | (Some(event), [Operand::Players(players), Operand::Objects(objects)]) => {
            let filter = event_filter(objects);
            if filter.is_some() {
                return EventSpec::Player {
                    event,
                    players: players.clone(),
                    objects: filter,
                };
            }
        }
        _ => {}
    }
    match (name.as_str(), operands.as_slice()) {
        (
            "Trigger::WhenAPlayerControlsNoPermanents",
            [Operand::Players(players), Operand::Objects(objects)],
        ) => match event_filter(objects) {
            Some(filter) => EventSpec::State {
                condition: Condition::Compare {
                    left: Expr::CountObjects(Box::new(ObjectSet::Battlefield(ObjectFilter::All(
                        vec![filter, ObjectFilter::ControlledBy(players.clone())],
                    )))),
                    comparison: Comparison::Equal,
                    right: Expr::Constant(0),
                },
            },
            None => EventSpec::Primitive { name, operands },
        },
        (
            "Trigger::WhenAPlayerDrawsTheirNthCardEachTurn",
            [Operand::Players(players), Operand::Comparison(comparison, value)],
        ) => EventSpec::NthEachTurn {
            event: Box::new(EventSpec::Player {
                event: PlayerEvent::Draws,
                players: players.clone(),
                objects: None,
            }),
            comparison: comparison.clone(),
            value: value.clone(),
        },
        (
            "Trigger::WhenAPlayerExpendsAnAmount",
            [Operand::Players(players), Operand::Comparison(comparison, value)],
        ) => EventSpec::NthEachTurn {
            event: Box::new(EventSpec::Player {
                event: PlayerEvent::Expends,
                players: players.clone(),
                objects: None,
            }),
            comparison: comparison.clone(),
            value: value.clone(),
        },
        (
            "Trigger::WhenAPlayerCastsTheirNthSpellInATurn",
            [Operand::Players(players), Operand::Comparison(comparison, value), Operand::Filter(spells)],
        ) => EventSpec::NthEachTurn {
            event: Box::new(EventSpec::Spell {
                event: SpellEvent::Cast,
                players: players.clone(),
                spells: spells.clone(),
            }),
            comparison: comparison.clone(),
            value: value.clone(),
        },
        _ => EventSpec::Primitive { name, operands },
    }
}

pub fn with_row(outer: &EventBindings, name: &str) -> EventBindings {
    with_bindings(outer, &row_bindings(name))
}

/// The enclosing scope plus what `event` binds, for its own filters.
pub fn with_event(outer: &EventBindings, event: &EventSpec) -> EventBindings {
    with_bindings(outer, &event_bindings(event))
}

fn with_bindings(outer: &EventBindings, row: &EventBindings) -> EventBindings {
    EventBindings {
        object: outer.object || row.object,
        permanent: outer.permanent || row.permanent,
        spell: outer.spell || row.spell,
        player: outer.player || row.player,
        amount: outer.amount || row.amount,
        block_pair: outer.block_pair || row.block_pair,
        group: outer.group || row.group,
        stack_object: outer.stack_object || row.stack_object,
        other: outer.other || row.other,
        other_player: outer.other_player || row.other_player,
        moved: outer.moved || row.moved,
        ..*outer
    }
}

pub fn event_bindings(event: &EventSpec) -> EventBindings {
    match event {
        EventSpec::SagaChapter(_) => EventBindings::default(),
        EventSpec::FirstEachTurn(event)
        | EventSpec::FirstForObjectEachTurn(event)
        | EventSpec::FirstEachGame(event) => event_bindings(event),
        EventSpec::NthEachTurn { event, .. } => event_bindings(event),
        EventSpec::Caused { event, .. } => EventBindings {
            stack_object: true,
            ..event_bindings(event)
        },
        EventSpec::ZoneChangeFromElsewhere { to, .. } => EventBindings {
            object: true,
            permanent: true,
            moved: true,
            moved_owner: matches!(to, EventZone::Graveyard | EventZone::Hand),
            ..EventBindings::default()
        },
        EventSpec::NthCounter { .. } => {
            row_bindings("Trigger::WhenTheNthCounterOfTypeIsPutOnAPermanent")
        }
        EventSpec::Primitive { name, .. } => row_bindings(name),
        EventSpec::Player { event, .. } => PLAYER_EVENTS
            .iter()
            .find(|(_, row)| row == event)
            .map_or_else(EventBindings::default, |(name, _)| row_bindings(name)),
        EventSpec::Discard { grouping, kind, .. } => EventBindings {
            group: matches!(grouping, DiscardGrouping::Player),
            object: matches!(grouping, DiscardGrouping::Card),
            player: true,
            amount: matches!(grouping, DiscardGrouping::Player),
            discard: true,
            discarded: matches!(grouping, DiscardGrouping::Card),
            activation_x: matches!(kind, DiscardKind::Cycling),
            stack_object: matches!(kind, DiscardKind::CausedBy(_)),
            moved: matches!(kind, DiscardKind::CausedBy(_)),
            ..Default::default()
        },
        EventSpec::ZoneChange { to, .. } => EventBindings {
            object: true,
            permanent: true,
            moved: true,
            moved_owner: matches!(to, EventZone::Graveyard | EventZone::Hand),
            ..EventBindings::default()
        },
        EventSpec::Object { event, .. } => {
            OBJECT_EVENTS.iter().find(|(_, row)| row == event).map_or(
                EventBindings {
                    object: true,
                    permanent: true,
                    ..EventBindings::default()
                },
                |(name, _)| row_bindings(name),
            )
        }
        EventSpec::Pair { event, .. } => PAIR_EVENTS
            .iter()
            .find(|(_, row)| row == event)
            .map_or_else(EventBindings::default, |(name, _)| row_bindings(name)),
        EventSpec::BecomesTarget { .. } => EventBindings {
            object: true,
            permanent: true,
            stack_object: true,
            ..EventBindings::default()
        },
        EventSpec::Attack { by, defenders, .. } => {
            let attacked = !matches!(defenders, AttackDefenders::Any);
            EventBindings {
                object: true,
                permanent: true,
                player: by.is_some() || attacked,
                other_player: by.is_some() && attacked,
                ..EventBindings::default()
            }
        }
        EventSpec::Counters { by, .. } => EventBindings {
            object: true,
            permanent: true,
            player: by.is_some(),
            amount: true,
            ..EventBindings::default()
        },
        EventSpec::Batch {
            event,
            per,
            count,
            including,
        } => {
            let mut inner = event_bindings(event);
            // The subject of a batched pair is one object even though its
            // other half is not.
            if matches!(**event, EventSpec::Pair { .. }) {
                inner.object = true;
                inner.permanent = true;
                inner.block_pair = false;
            }
            let one = matches!(per, BatchPer::Subject)
                || count.is_none() && including.is_none() && !matches!(per, BatchPer::Defender);
            EventBindings {
                object: inner.object && one,
                permanent: inner.permanent && one,
                amount: true,
                // "One or more creatures" names the other half of a pair.
                group: !matches!(per, BatchPer::Subject)
                    || matches!(**event, EventSpec::Pair { .. }),
                ..inner
            }
        }
        EventSpec::ClassLevel { .. } => EventBindings {
            object: true,
            permanent: true,
            ..EventBindings::default()
        },
        EventSpec::Damage(event) => {
            let source = matches!(
                event.grouping,
                DamageGrouping::Source | DamageGrouping::SourceRecipient
            );
            let recipient = matches!(
                event.grouping,
                DamageGrouping::Recipient | DamageGrouping::SourceRecipient
            );
            let objects = matches!(event.recipients, DamageRecipients::Objects(_));
            let players = matches!(event.recipients, DamageRecipients::Players(_));
            let object = match event.subject {
                DamageSubject::Source => source,
                DamageSubject::Recipient => recipient && objects,
            };
            EventBindings {
                object,
                permanent: object
                    && match event.subject {
                        DamageSubject::Source => matches!(event.source_kind, ObjectKind::Permanent),
                        DamageSubject::Recipient => true,
                    },
                spell: object
                    && matches!(event.subject, DamageSubject::Source)
                    && matches!(event.source_kind, ObjectKind::Spell),
                player: recipient && players,
                amount: true,
                moved: false,
                damage: true,
                damage_source: source,
                damage_source_permanent: source
                    && matches!(event.source_kind, ObjectKind::Permanent),
                damage_recipient: recipient && objects,
                block_pair: false,
                group: false,
                stack_object: false,
                other: false,
                other_player: false,
                moved_owner: false,
                damage_objects: !players,
                damage_players: !objects,
                damage_subject: Some(event.subject),
                discard: false,
                discarded: false,
                activation_x: false,
            }
        }
        EventSpec::Spell { .. } => EventBindings {
            object: true,
            spell: true,
            player: true,
            ..EventBindings::default()
        },
        EventSpec::Activated { .. } => EventBindings {
            object: true,
            permanent: true,
            player: true,
            stack_object: true,
            ..EventBindings::default()
        },
        EventSpec::Turn { .. } => EventBindings {
            player: true,
            ..EventBindings::default()
        },
        EventSpec::State { .. } => EventBindings::default(),
        EventSpec::Filtered { event, .. } => event_bindings(event),
        EventSpec::Either(events) => events
            .iter()
            .map(event_bindings)
            .reduce(EventBindings::shared)
            .unwrap_or_default(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ActivationKind {
    Mana,
    Stack,
}
#[derive(Default)]
struct Effects {
    mana: bool,
    library: bool,
    objects: std::collections::BTreeMap<u32, (bool, bool)>,
    targets: std::collections::BTreeMap<u16, bool>,
}
impl Effects {
    fn program(&mut self, program: &Program) {
        for instruction in &program.instructions {
            match instruction {
                Instruction::Effect { result, operation } => match operation {
                    Operation::FlipCoin { .. }
                    | Operation::ChoosePowerToughness { .. }
                    | Operation::DuplicateCounters { .. }
                    | Operation::ChooseAtRandom { .. }
                    | Operation::ChooseValue { .. } => {}
                    Operation::JoinResults { results } => {
                        let joined = results
                            .iter()
                            .filter_map(|r| self.objects.get(&r.0))
                            .fold((false, false), |a, b| (a.0 | b.0, a.1 | b.1));
                        self.objects.insert(result.0, joined);
                    }
                    Operation::AddMana { .. } => self.mana = true,
                    Operation::Draw { .. } | Operation::Mill { .. } => self.library = true,
                    Operation::Move {
                        objects,
                        destination,
                        ..
                    }
                    | Operation::ChangeZone {
                        cards: objects,
                        destination,
                        ..
                    }
                    | Operation::Counter {
                        objects,
                        destination,
                    } => {
                        let before = self.objects_library(objects);
                        let after = Self::destination_library(destination);
                        self.library |= before || after;
                        self.objects.insert(result.0, (before, after));
                    }
                    Operation::ChooseObjects { objects, .. }
                    | Operation::SeparatePiles { objects, .. }
                    | Operation::NoteObjects { objects } => {
                        let library = self.objects_library(objects);
                        self.objects.insert(result.0, (library, library));
                    }
                    Operation::Search { .. } => {
                        self.objects.insert(result.0, (true, true));
                    }
                    Operation::LookAt { objects, .. }
                    | Operation::ReorderLibrary { objects, .. } => {
                        let library = self.objects_library(objects);
                        self.objects.insert(result.0, (library, library));
                    }
                    Operation::Surveil { amount, .. } => {
                        self.library |= !matches!(amount, Expr::Constant(0))
                    }
                    Operation::Reveal { .. }
                    | Operation::RevealUntil { .. }
                    | Operation::RevealUntilGroup { .. }
                    | Operation::Shuffle { .. }
                    | Operation::Scry { .. } => {}
                    Operation::PayCost { cost, .. } | Operation::PayRepeatedly { cost, .. } => {
                        self.cost(cost);
                    }
                    // Its body executes in a later triggered ability, not
                    // during this activation's resolution (CR 605.1a).
                    Operation::CreateDelayedTrigger { .. }
                    | Operation::CreateTrigger { .. }
                    | Operation::BindValue { .. }
                    | Operation::RepeatProcess
                    | Operation::DiscardObjects { .. }
                    | Operation::ReflexiveTrigger { .. }
                    | Operation::ReturnAfter { .. }
                    | Operation::CreateReplacement { .. }
                    | Operation::ModifyEvent { .. }
                    | Operation::CreateRegenerationShields { .. }
                    | Operation::Discard { .. }
                    | Operation::RemoveCounters { .. }
                    | Operation::CreateTokens { .. }
                    | Operation::Attach { .. }
                    | Operation::GainLife { .. }
                    | Operation::SetLifeTotal { .. }
                    | Operation::LoseLife { .. }
                    | Operation::DealDamage { .. }
                    | Operation::DealDamageParts { .. }
                    | Operation::DealDamageFromEach { .. }
                    | Operation::DealDamageWith { .. }
                    | Operation::Destroy { .. }
                    | Operation::Tap { .. }
                    | Operation::Sacrifice { .. }
                    | Operation::PutCounters { .. }
                    | Operation::CreateContinuousEffect { .. } => {}
                    // Its effects are unknown, so it must not read as a mana ability.
                    Operation::Named { .. }
                    | Operation::Primitive { .. }
                    | Operation::Keyword { .. }
                    | Operation::CreatePerpetualEffect { .. }
                    | Operation::CreateEmblem { .. }
                    | Operation::MoveCounters { .. }
                    | Operation::DoubleCounters { .. }
                    | Operation::PlayCards { .. }
                    | Operation::PlayWith { .. }
                    | Operation::Together(_)
                    | Operation::Repeat { .. }
                    | Operation::TurnFaceDownAs { .. }
                    | Operation::Exchange { .. }
                    | Operation::ChooseValueWith { .. }
                    | Operation::RevealChoices { .. }
                    | Operation::CoinFlips { .. }
                    | Operation::AttachEach { .. }
                    | Operation::CopyEach { .. }
                    | Operation::RetargetEach { .. }
                    | Operation::TakeSpell { .. }
                    | Operation::SetX { .. }
                    | Operation::ChangeCombat { .. }
                    | Operation::PlayersDo { .. }
                    | Operation::CreateBoon { .. }
                    | Operation::AddPhases { .. }
                    | Operation::ChooseAndPay { .. }
                    | Operation::CopyCards { .. }
                    | Operation::ChangeSpell { .. }
                    | Operation::KeywordEach { .. }
                    | Operation::MoveTopCard { .. }
                    | Operation::PayAnyAmount { .. }
                    | Operation::RevealFromHand { .. }
                    | Operation::CopyOnStack { .. }
                    | Operation::Retarget { .. }
                    | Operation::Clash { .. }
                    | Operation::Vote { .. }
                    | Operation::ChooseAction { .. }
                    | Operation::EffectOn { .. }
                    | Operation::GameEffect { .. }
                    | Operation::AttachChosen { .. }
                    | Operation::DrawUpTo { .. }
                    | Operation::SacrificeAllBut { .. }
                    | Operation::DiscardAllBut { .. }
                    | Operation::TowardPlayer { .. }
                    | Operation::Intensify { .. }
                    | Operation::FlipCoins { .. }
                    | Operation::Wish { .. }
                    | Operation::AbandonScheme { .. }
                    | Operation::PutSticker { .. }
                    | Operation::BeginOnBattlefield { .. }
                    | Operation::Manifest { .. }
                    | Operation::DraftFromSpellbook { .. }
                    | Operation::Conjure { .. } => self.library = true,
                },
                Instruction::Group(p) | Instruction::Optional { body: p, .. } => self.program(p),
                Instruction::If {
                    then, otherwise, ..
                } => {
                    self.program(then);
                    self.program(otherwise);
                }
                Instruction::Times { body, .. }
                | Instruction::ForEachObject { body, .. }
                | Instruction::ForEachPlayer { body, .. }
                | Instruction::Repeatable(body) => self.program(body),
                Instruction::ChooseOne { options, .. } => {
                    options.iter().for_each(|option| self.program(option));
                }
            }
        }
    }
    fn destination_library(destination: &Destination) -> bool {
        match destination {
            Destination::OwnersLibraryTop
            | Destination::OwnersLibraryBottom
            | Destination::OwnersLibraryFromTop(_)
            | Destination::OwnersLibraryBottomRandom
            | Destination::OwnersLibraryTopOrBottom
            | Destination::OwnersLibraryShuffled => true,
            Destination::OwnersHand
            | Destination::OwnersGraveyard
            | Destination::Exile
            | Destination::Battlefield(_)
            | Destination::OwnersBattlefield
            | Destination::Hand(_)
            | Destination::CommandZone
            | Destination::Ante => false,
        }
    }
    fn object_library(&self, object: &ObjectRef) -> bool {
        match object {
            ObjectRef::Target { slot, .. } => self.targets.get(&slot.0).copied().unwrap_or(false),
            ObjectRef::Query { objects, .. } => self.objects_library(objects),
            ObjectRef::Result { result, phase, .. } => self.result_library(result, phase),
            ObjectRef::ChosenBy { result, .. } => self.result_library(result, &ObjectPhase::After),
            ObjectRef::Source
            | ObjectRef::KeywordHost
            | ObjectRef::MadeHere
            | ObjectRef::Granter
            | ObjectRef::Discarded
            | ObjectRef::Damage(_)
            | ObjectRef::Candidate
            | ObjectRef::AttachedTo(_)
            | ObjectRef::PairedWith(_)
            | ObjectRef::SoleTargetOf(_)
            | ObjectRef::Affected
            | ObjectRef::EventSubject
            | ObjectRef::EventStackObject
            | ObjectRef::EventOther
            | ObjectRef::RecipientObject { .. }
            | ObjectRef::BlockPair(_)
            | ObjectRef::EventDestination
            | ObjectRef::Chosen
            | ObjectRef::Primitive { .. }
            | ObjectRef::Bound(_) => false,
        }
    }
    fn result_library(&self, result: &ResultId, phase: &ObjectPhase) -> bool {
        self.objects
            .get(&result.0)
            .is_some_and(|(before, after)| match phase {
                ObjectPhase::Before => *before,
                ObjectPhase::After => *after,
            })
    }
    fn objects_library(&self, objects: &ObjectSet) -> bool {
        match objects {
            ObjectSet::LibraryTops(_) => true,
            ObjectSet::Primitive { .. }
            | ObjectSet::Named(_)
            | ObjectSet::AnyZone(_)
            | ObjectSet::OutsideTheGame(_) => false,
            ObjectSet::Observed(result) => self.result_library(result, &ObjectPhase::Before),
            ObjectSet::Difference { objects, .. } | ObjectSet::Matching { objects, .. } => {
                self.objects_library(objects)
            }
            ObjectSet::Zone { scope, .. } => matches!(
                scope,
                ZoneScope::Players {
                    zone: EventZone::Library,
                    ..
                }
            ),
            ObjectSet::InZone { zone, .. } => matches!(zone, EventZone::Library),
            ObjectSet::One(o) => self.object_library(o),
            ObjectSet::Union(sets) => sets.iter().any(|s| self.objects_library(s)),
            ObjectSet::Take { objects, .. }
            | ObjectSet::Random { objects, .. }
            | ObjectSet::Chosen { objects, .. } => self.objects_library(objects),
            ObjectSet::Targets(slot) => self.targets.get(&slot.0).copied().unwrap_or(false),
            ObjectSet::Selected(result) | ObjectSet::Unselected(result) => {
                self.result_library(result, &ObjectPhase::After)
            }
            ObjectSet::Transition { result, phase } | ObjectSet::MovedBy { result, phase, .. } => {
                self.result_library(result, phase)
            }
            ObjectSet::Battlefield(_)
            | ObjectSet::Created(_)
            | ObjectSet::Copies(_)
            | ObjectSet::Unpaid(_)
            | ObjectSet::DamageSources(_) => false,
            ObjectSet::EventGroup
            | ObjectSet::Damage(_)
            | ObjectSet::CostObjects { .. }
            | ObjectSet::Affected(_) => false,
        }
    }
    fn cost(&mut self, cost: &Cost) -> bool {
        let mut loyalty = false;
        for part in &cost.parts {
            match part {
                CostPart::Primitive { .. }
                | CostPart::ManaWithX { .. }
                | CostPart::Waterbend(_)
                | CostPart::Teamwork(_)
                | CostPart::Keyword { .. }
                | CostPart::DiscardHand
                | CostPart::Exert(_)
                | CostPart::Unattach(_)
                | CostPart::Unspecialize(_)
                | CostPart::Behold { .. }
                | CostPart::BeholdAndExile(_)
                | CostPart::LifeEqualToOwnManaValue
                | CostPart::ManaAnyNumberOfTimes(_)
                | CostPart::ManaCostReduced { .. }
                | CostPart::RevealChoice(_)
                | CostPart::ManaXBounded { .. }
                | CostPart::UnattachFrom { .. }
                | CostPart::EnergyOneOrMore
                | CostPart::EachPlayerAction { .. }
                | CostPart::ManaReducedForEach { .. }
                | CostPart::ObjectAction { .. }
                | CostPart::ManaFromTreasure(_)
                | CostPart::SinglePlayersCards(_)
                | CostPart::PutCounters(_) => {}
                CostPart::ShuffleFromHand | CostPart::PlayFromTop(_) => self.library = true,
                CostPart::ExiledToGraveyard(_) => {}
                CostPart::RevealTop(_) => self.library = true,
                CostPart::Move {
                    selections,
                    destination,
                    ..
                } => {
                    self.library |= Self::destination_library(destination)
                        || selections.iter().any(|s| self.objects_library(&s.objects));
                }
                CostPart::Choice(options) => {
                    for option in options {
                        loyalty |= self.cost(option);
                    }
                }
                CostPart::Constrained { part, .. } => {
                    loyalty |= self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    })
                }
                CostPart::Loyalty(_) => loyalty = true,
                CostPart::Mill { .. } | CostPart::Draw { .. } => self.library = true,
                CostPart::Repeated { part, .. } => {
                    loyalty |= self.cost(&Cost {
                        parts: vec![part.as_ref().clone()],
                    })
                }
                CostPart::Reveal { .. } | CostPart::PlayerAction { .. } => {}
                CostPart::RemoveCounters { .. }
                | CostPart::ManaCostOf(_)
                | CostPart::Mana(_)
                | CostPart::ChangeTapState { .. }
                | CostPart::TapSymbol
                | CostPart::Life(_)
                | CostPart::Energy(_)
                | CostPart::Sacrifice { .. }
                | CostPart::Discard { .. } => {}
            }
        }
        loyalty
    }
    fn announcement(&mut self, announcement: &Announcement) -> bool {
        for target in &announcement.targets {
            if let TargetKind::Object(domain) = &target.kind {
                self.targets
                    .insert(target.slot.0, domain.zones.contains(&EventZone::Library));
            }
        }
        let mut targets = !announcement.targets.is_empty();
        if let Some(modes) = &announcement.modes {
            for mode in &modes.modes {
                targets |= self.announcement(&mode.announcement);
                self.program(&mode.body);
            }
        }
        targets
    }
}
/// CR 605.1a and 115.6, effective September 25, 2026. Even optional targets
/// require targets. Costs/effects moving cards to or from libraries disqualify
/// an activated ability; granted token abilities do not produce mana for it.
pub fn activation_kind(cost: &Cost, announcement: &Announcement, body: &Program) -> ActivationKind {
    let mut effects = Effects::default();
    let targets = effects.announcement(announcement);
    effects.program(body);
    let loyalty = effects.cost(cost);
    if effects.mana && !targets && !loyalty && !effects.library {
        ActivationKind::Mana
    } else {
        ActivationKind::Stack
    }
}
