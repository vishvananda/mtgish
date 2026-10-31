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
        ReplacementEvent::Measured { event, .. } => replacement_bindings(event),
        ReplacementEvent::Damage(damage) => event_bindings(&EventSpec::Damage(damage.clone())),
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
    let name = match operation {
        Operation::Primitive { name, .. } => name.as_str(),
        Operation::Keyword { action, .. } => {
            KEYWORD_ACTIONS.iter().find(|(_, row)| row == action)?.0
        }
        _ => return None,
    };
    crate::primitive_spec::lookup(name)?.yields.as_deref()
}

/// Action rows that are one keyword action. Each keeps its reviewed `yields`.
const KEYWORD_ACTIONS: &[(&str, KeywordAction)] = &[
    ("Action::TransformPermanent", KeywordAction::Transform),
    ("Action::Fight", KeywordAction::Fight),
    (
        "Action::CopySpellAndMayChooseNewTargets",
        KeywordAction::CopySpell,
    ),
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

/// A row's action in generic form, or the row itself when it has none.
pub fn reduce_action(actor: PlayerSet, name: String, operands: Vec<Operand>) -> Operation {
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

/// A reviewed row that makes a card a later reference can read.
pub fn makes_card(operation: &Operation) -> bool {
    matches!(operation, Operation::Primitive { name, .. }
        if name.starts_with("Action::Conjure") || name.starts_with("Action::Draft"))
}

/// A row's filter in generic form, or the row itself when it has none.
pub fn reduce_filter(name: String, operands: Vec<Operand>) -> ObjectFilter {
    let form = name.split("::").nth(1).unwrap_or(&name);
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
        "HasBeenGoaded" => Some(ObjectState::Goaded),
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
        _ => ObjectFilter::Primitive { name, operands },
    }
}

/// A row's condition in generic form, or the row itself when it has none.
pub fn reduce_condition(name: String, operands: Vec<Operand>) -> Condition {
    let window = match name.as_str() {
        "Condition::IsBeforeAttackersDeclared" => Some(TurnWindow::BeforeAttackersDeclared),
        "Condition::IsDuringCombat" => Some(TurnWindow::Combat),
        "Condition::IsDuringUpkeep" => Some(TurnWindow::Upkeep),
        "Condition::IsDuringEndStep" => Some(TurnWindow::EndStep),
        "Condition::IsBeforeCombatDamageStep" => Some(TurnWindow::BeforeCombatDamage),
        "Condition::IsDuringDeclareAttackersStep" => Some(TurnWindow::DeclareAttackers),
        _ => None,
    };
    let state = match name.as_str() {
        "Players::IsTheMonarch" => Some((PlayerState::Monarch, false)),
        "Players::HasTheCitysBlessing" => Some((PlayerState::CitysBlessing, false)),
        "Players::HasAFullParty" => Some((PlayerState::FullParty, false)),
        "Players::IsNotTheStartingPlayer" => Some((PlayerState::StartingPlayer, true)),
        "Players::IsTheirUpkeep" => Some((PlayerState::InTheirUpkeep, false)),
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
        _ => Condition::Primitive { name, operands },
    }
}

/// A row's change in generic form, or the row itself when it has none.
pub fn reduce_change(name: String, operands: Vec<Operand>) -> Change {
    match (name.as_str(), operands.as_slice()) {
        ("LayerEffect::SwitchPT", []) => Change::SwitchPowerToughness,
        ("LayerEffect::LosesAbility", [ability]) => Change::LosesAbility(Box::new(ability.clone())),
        ("LayerEffect::SetLandType", [land]) => Change::SetLandType(Box::new(land.clone())),
        ("LayerEffect::IsACopyOfPermanent", [Operand::Object(object), effects]) => Change::CopyOf {
            object: object.clone(),
            effects: Box::new(effects.clone()),
        },
        _ => Change::Primitive { name, operands },
    }
}

/// A row's ability in generic form, or the row itself when it has none.
pub fn reduce_ability(name: String, operands: Vec<Operand>) -> Ability {
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
        "WhenACounterOfTypeIsRemovedFromAPermanent" => (Removed, false, false),
        "WhenAnyNumberOfCountersOfTypeAreRemovedFromAPermanent"
        | "WhenAnyNumberOfCountersAreRemovedFromAPermanent" => (Removed, true, false),
        "WhenTheLastCounterOfTypeIsRemovedFromAPermanent"
        | "WhenAPlayerRemovesTheLastCounterOfTypeFromAPermanent" => (LastRemoved, false, false),
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
        _ => return None,
    })
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
            if name == "Trigger::WhenAPermanentIsExiled" {
                return Some(EventSpec::ZoneChange {
                    objects: filter,
                    from: Some(EventZone::Battlefield),
                    to: EventZone::Exile,
                });
            }
            let event = OBJECT_EVENTS.iter().find(|(row, _)| *row == name)?.1;
            Some(EventSpec::Object {
                event,
                objects: filter,
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

/// Say who did `event`, for history that asks about one player.
pub fn set_event_player(event: &mut EventSpec, player: PlayerSet) -> bool {
    match event {
        EventSpec::Primitive { operands, .. } => operands[0] = Operand::Players(player),
        EventSpec::Player { players, .. } => *players = player,
        EventSpec::Attack { by, .. } | EventSpec::Counters { by, .. } => *by = Some(player),
        EventSpec::Batch { event, .. } => return set_event_player(event, player),
        _ => return false,
    }
    true
}

/// A row's event in generic form, or the row itself when it has none.
pub fn reduce_event(name: String, operands: Vec<Operand>) -> EventSpec {
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
        (Some(event), [Operand::Players(players), Operand::Objects(objects)]) => {
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
        EventSpec::FirstEachTurn(event) => event_bindings(event),
        EventSpec::NthEachTurn { event, .. } => event_bindings(event),
        EventSpec::Primitive { name, .. } => row_bindings(name),
        EventSpec::Player { event, .. } => PLAYER_EVENTS
            .iter()
            .find(|(_, row)| row == event)
            .map_or_else(EventBindings::default, |(name, _)| row_bindings(name)),
        EventSpec::Discard { grouping, kind, .. } => EventBindings {
            object: matches!(grouping, DiscardGrouping::Card),
            player: true,
            amount: matches!(grouping, DiscardGrouping::Player),
            discard: true,
            discarded: matches!(grouping, DiscardGrouping::Card),
            activation_x: matches!(kind, DiscardKind::Cycling),
            ..Default::default()
        },
        EventSpec::ZoneChange { to, .. } => EventBindings {
            object: true,
            permanent: true,
            moved: true,
            moved_owner: matches!(to, EventZone::Graveyard),
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
            ..EventBindings::default()
        },
        EventSpec::Batch {
            event,
            per,
            count,
            including,
        } => {
            let inner = event_bindings(event);
            let one = matches!(per, BatchPer::Subject)
                || count.is_none() && including.is_none() && !matches!(per, BatchPer::Defender);
            EventBindings {
                object: inner.object && one,
                permanent: inner.permanent && one,
                amount: true,
                group: !matches!(per, BatchPer::Subject),
                ..inner
            }
        }
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
                    Operation::FlipCoin { .. } | Operation::ChoosePowerToughness { .. } => {}
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
                    Operation::PayCost { cost, .. } => {
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
                    | Operation::Keyword { .. } => self.library = true,
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
            | Destination::OwnersBattlefield => false,
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
            ObjectSet::Take { objects, .. } => self.objects_library(objects),
            ObjectSet::Targets(slot) => self.targets.get(&slot.0).copied().unwrap_or(false),
            ObjectSet::Selected(result) | ObjectSet::Unselected(result) => {
                self.result_library(result, &ObjectPhase::After)
            }
            ObjectSet::Transition { result, phase } | ObjectSet::MovedBy { result, phase, .. } => {
                self.result_library(result, phase)
            }
            ObjectSet::Battlefield(_) | ObjectSet::Created(_) | ObjectSet::DamageSources(_) => {
                false
            }
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
                CostPart::Primitive { .. } => {}
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
