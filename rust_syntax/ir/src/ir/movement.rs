//! Movement in the normalized semantic model.
use super::*;

/// What a face-down permanent is, when not a plain 2/2.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct FaceDownAs {
    pub power: Expr,
    pub toughness: Expr,
    pub creature_type: Option<CreatureType>,
    pub artifact: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum CardOrder {
    Chosen,
    Preserve,
    Random,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum EntryController {
    Owner,
    Player(PlayerRef),
}

/// Placement terms are independent of selecting the objects that will move.
/// The default retains the destination's own ordering and chooser rules.
#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub struct MovementTerms {
    pub order: MovementOrder,
    pub placement: MovementPlacement,
    /// Qualifications over the entire moved set when no selection is stated.
    /// These do not implicitly ask a player to choose a subset.
    pub qualifiers: Vec<SelectionConstraint>,
}

impl MovementTerms {
    pub fn is_default(&self) -> bool {
        matches!(self.order, MovementOrder::DestinationDefault)
            && matches!(self.placement, MovementPlacement::DestinationDefault)
            && self.qualifiers.is_empty()
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MovementOrder {
    #[default]
    DestinationDefault,
    Random,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum MovementPlacement {
    #[default]
    DestinationDefault,
    /// A stated player chooses a library placement, rather than each owner.
    By(PlayerSet),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum Destination {
    OwnersHand,
    OwnersGraveyard,
    OwnersLibraryTop,
    OwnersLibraryBottom,
    Exile,
    Battlefield(PlayerRef),
    OwnersBattlefield,
    /// "Shuffle it into its owner's library": the shuffle is one action.
    OwnersLibraryShuffled,
    /// The position counted from the top: third from the top is 3.
    OwnersLibraryFromTop(Expr),
    /// The bottom, in a random order.
    OwnersLibraryBottomRandom,
    /// Its owner's choice of the top or the bottom.
    OwnersLibraryTopOrBottom,
    Hand(PlayerRef),
    CommandZone,
    Ante,
    /// This end of its owner's planar deck (901.3).
    OwnersPlanarDeck(SequenceEnd),
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
pub enum SequenceEnd {
    Top,
    Bottom,
}

/// A movement address and the state meaningful at that destination. Library
/// ordering and choice describe placement; selection qualifiers stay on the
/// action so they cannot introduce an unstated choice of which objects move.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum MovementDestination {
    Battlefield {
        controller: EntryController,
        arrival: BattlefieldArrival,
    },
    Exile {
        visibility: ExileVisibility,
        arrival: ExileArrival,
    },
    Library {
        position: LibraryPosition,
        order: MovementOrder,
        arrival: CardArrival,
    },
    OwnersHand(CardArrival),
    Hand {
        player: PlayerRef,
        arrival: CardArrival,
    },
    OwnersGraveyard(CardArrival),
    CommandZone(CardArrival),
    Ante(CardArrival),
    OwnersPlanarDeck {
        end: SequenceEnd,
        arrival: CardArrival,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum LibraryPosition {
    Top,
    Bottom,
    Shuffled,
    FromTop(Expr),
    BottomRandom,
    TopOrBottom { chooser: LibraryChooser },
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum LibraryChooser {
    #[default]
    Owners,
    Players(PlayerSet),
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum ExileVisibility {
    #[default]
    FaceUp,
    FaceDown,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub struct BattlefieldArrival {
    pub controller: Option<EntryController>,
    pub tapped: bool,
    pub counters: Vec<(crate::counter::CounterKind, Expr)>,
    pub states: Vec<BattlefieldState>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub struct ExileArrival {
    pub counters: Vec<(crate::counter::CounterKind, Expr)>,
    pub states: Vec<ExileState>,
}

/// Changes to a card outside the battlefield or exile. Permanent entry state,
/// controller, tapped status and exile duration have no fields in this type.
#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub struct CardArrival {
    pub counters: Vec<(crate::counter::CounterKind, Expr)>,
    pub states: Vec<CardArrivalState>,
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum BattlefieldState {
    Transformed,

    Prepared,

    CopyOf {
        objects: ObjectSet,
        exceptions: Vec<CopyException>,
        until: Option<Duration>,
    },

    Attacking {
        player: Option<PlayerRef>,
        or_planeswalker: bool,
    },
    Blocking(ObjectRef),
    AttachedTo(ObjectRef),
    AttachedToChosen(ObjectFilter),
    AttachedToPlayer(PlayerRef),
    Changes(Vec<Change>),
    ChangesUntil {
        changes: Vec<Change>,
        duration: Duration,
    },

    When {
        filter: ObjectFilter,
        entry: Box<BattlefieldArrival>,
    },
    FaceDown,
    Flipped,

    ChooseChanges(Vec<Vec<Change>>),

    CountersOfChoice {
        kinds: Vec<crate::counter::CounterKind>,
        amount: Expr,
        distinct: bool,
    },

    CountersOf {
        object: ObjectRef,
        one_of_each: bool,
    },
    FaceDownAs(FaceDownAs),
    FaceDownWith {
        face_down: FaceDownAs,
        abilities: Vec<Ability>,

        noted: Option<ObjectRef>,
    },
    FaceDownLand(crate::mtg_types::LandType),
    NotAnAura,

    NotedCounters,
    Perpetual(Vec<Change>),

    Optional {
        decider: PlayerRef,
        entry: Box<BattlefieldArrival>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum ExileState {
    /// A conditional arrival may hide the cards it applies to. Ordinary move
    /// visibility is carried by MovementDestination::Exile.
    FaceDown,
    Changes(Vec<Change>),
    ChangesUntil {
        changes: Vec<Change>,
        duration: Duration,
    },

    When {
        filter: ObjectFilter,
        entry: Box<ExileArrival>,
    },

    ExilePile {
        shuffled: bool,
    },
    Perpetual(Vec<Change>),

    Optional {
        decider: PlayerRef,
        entry: Box<ExileArrival>,
    },

    ExiledFor(Duration),
    ExiledForWith {
        until: Duration,

        objects: ObjectSet,

        body: Program,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum CardArrivalState {
    Changes(Vec<Change>),
    ChangesUntil {
        changes: Vec<Change>,
        duration: Duration,
    },

    When {
        filter: ObjectFilter,
        entry: Box<CardArrival>,
    },
    Perpetual(Vec<Change>),

    Optional {
        decider: PlayerRef,
        entry: Box<CardArrival>,
    },
}

/// Conjuring creates a card owned by the action's owner (or acting player).
/// That ownership remains separate from a battlefield controller override.
/// Library placement belongs only to the library variant.
#[derive(Clone, Debug, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum ConjureDestination {
    Battlefield(BattlefieldArrival),
    Exile {
        visibility: ExileVisibility,
        arrival: ExileArrival,
    },
    Library {
        position: ConjurePosition,
        arrival: CardArrival,
    },
    Hand(CardArrival),
    Graveyard(CardArrival),
    Stack(CardArrival),
    Command(CardArrival),
    Ante(CardArrival),
    Outside(CardArrival),
    PlanarDeck(CardArrival),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, bincode::Encode, bincode::Decode)]
#[serde(deny_unknown_fields)]
pub enum ConjurePosition {
    #[default]
    Random,
    Shuffled,
    Top,
    AmongTop(Expr),
    FromTop(Expr),
}

impl MovementDestination {
    /// The addressed zone/player/position, without projecting any arrival state.
    pub fn address(&self) -> Destination {
        match self {
            Self::Battlefield { controller, .. } => match controller {
                EntryController::Owner => Destination::OwnersBattlefield,
                EntryController::Player(player) => Destination::Battlefield(player.clone()),
            },
            Self::Exile { .. } => Destination::Exile,
            Self::Library { position, .. } => match position {
                LibraryPosition::Top => Destination::OwnersLibraryTop,
                LibraryPosition::Bottom => Destination::OwnersLibraryBottom,
                LibraryPosition::Shuffled => Destination::OwnersLibraryShuffled,
                LibraryPosition::FromTop(n) => Destination::OwnersLibraryFromTop(n.clone()),
                LibraryPosition::BottomRandom => Destination::OwnersLibraryBottomRandom,
                LibraryPosition::TopOrBottom { .. } => Destination::OwnersLibraryTopOrBottom,
            },
            Self::OwnersHand(_) => Destination::OwnersHand,
            Self::Hand { player, .. } => Destination::Hand(player.clone()),
            Self::OwnersGraveyard(_) => Destination::OwnersGraveyard,
            Self::CommandZone(_) => Destination::CommandZone,
            Self::Ante(_) => Destination::Ante,
            Self::OwnersPlanarDeck { end, .. } => Destination::OwnersPlanarDeck(end.clone()),
        }
    }
    pub fn hidden(&self) -> bool {
        matches!(
            self,
            Self::Exile {
                visibility: ExileVisibility::FaceDown,
                ..
            }
        )
    }
    /// Whether the address alone expresses this movement's entire arrival.
    pub fn is_plain(&self) -> bool {
        fn card(entry: &CardArrival) -> bool {
            entry.counters.is_empty() && entry.states.is_empty()
        }
        match self {
            Self::Battlefield { arrival, .. } => {
                arrival.controller.is_none()
                    && !arrival.tapped
                    && arrival.counters.is_empty()
                    && arrival.states.is_empty()
            }
            Self::Exile {
                visibility,
                arrival,
            } => {
                matches!(visibility, ExileVisibility::FaceUp)
                    && arrival.counters.is_empty()
                    && arrival.states.is_empty()
            }
            Self::Library {
                position,
                order,
                arrival,
            } => {
                card(arrival)
                    && matches!(order, MovementOrder::DestinationDefault)
                    && !matches!(
                        position,
                        LibraryPosition::TopOrBottom {
                            chooser: LibraryChooser::Players(_)
                        }
                    )
            }
            Self::Hand { arrival, .. }
            | Self::OwnersPlanarDeck { arrival, .. }
            | Self::OwnersHand(arrival)
            | Self::OwnersGraveyard(arrival)
            | Self::CommandZone(arrival)
            | Self::Ante(arrival) => card(arrival),
        }
    }
}

impl BattlefieldArrival {
    pub fn simple_tapped(&self) -> Option<bool> {
        (self.controller.is_none() && self.counters.is_empty() && self.states.is_empty())
            .then_some(self.tapped)
    }
}

impl From<Destination> for MovementDestination {
    fn from(address: Destination) -> Self {
        let card = CardArrival::default();
        match address {
            Destination::Battlefield(player) => Self::Battlefield {
                controller: EntryController::Player(player),
                arrival: BattlefieldArrival::default(),
            },
            Destination::OwnersBattlefield => Self::Battlefield {
                controller: EntryController::Owner,
                arrival: BattlefieldArrival::default(),
            },
            Destination::Exile => Self::Exile {
                visibility: ExileVisibility::FaceUp,
                arrival: ExileArrival::default(),
            },
            Destination::OwnersHand => Self::OwnersHand(card),
            Destination::OwnersGraveyard => Self::OwnersGraveyard(card),
            Destination::Hand(player) => Self::Hand {
                player,
                arrival: card,
            },
            Destination::CommandZone => Self::CommandZone(card),
            Destination::Ante => Self::Ante(card),
            Destination::OwnersPlanarDeck(end) => Self::OwnersPlanarDeck { end, arrival: card },
            address => {
                let position = match address {
                    Destination::OwnersLibraryTop => LibraryPosition::Top,
                    Destination::OwnersLibraryBottom => LibraryPosition::Bottom,
                    Destination::OwnersLibraryShuffled => LibraryPosition::Shuffled,
                    Destination::OwnersLibraryFromTop(n) => LibraryPosition::FromTop(n),
                    Destination::OwnersLibraryBottomRandom => LibraryPosition::BottomRandom,
                    Destination::OwnersLibraryTopOrBottom => LibraryPosition::TopOrBottom {
                        chooser: LibraryChooser::Owners,
                    },
                    _ => unreachable!(),
                };
                Self::Library {
                    position,
                    order: MovementOrder::DestinationDefault,
                    arrival: card,
                }
            }
        }
    }
}
