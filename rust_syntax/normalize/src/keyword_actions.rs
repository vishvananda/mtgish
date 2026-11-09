//! Reviewed legacy/source keyword signatures and borrowed compiler views.
//! Complete IR carries typed action variants, never this boundary's field bag.
use crate::ir::*;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// A tag used only at the historical/source boundary. Complete IR stores
/// `KeywordAction` or `ObjectKeywordAction` with their required parameters.
pub enum KeywordKind {
    Transform,
    Fight,
    Investigate,
    Proliferate,
    Amass,
    Explore,
    RingTempts,
    Connive,
    Goad,
    Earthbend,
    Venture,
    ManifestDread,
    Monstrosity,
    Discover,
    GainEnergy,
    BecomeMonarch,
    LoseGame,
    WinGame,
    TakeExtraTurn,
    PhaseOut,
    Prepare,
    TapOrUntap,
    ExchangeControl,
    RollDie(u32),
    EmpowerJace,
    Incubate,
    Adapt,
    Populate,
    Bolster,
    Learn,
    TakeInitiative,
    OpenAttraction,
    GetTickets,
    RemoveFromCombat,
    Flip,
    Awaken,
    Planeswalk,
    Convert,
    Suspect,
    Endure,
    Blight,
    TurnFaceUp,
    TurnFaceDown,
    Detain,
    EndTheTurn,
    TimeTravel,
    Intensify,
    Forage,
    Ascend,
    BecomeDay,
    BecomeNight,
    ChaosEnsues,
    Airbend,
    Saddle,
    Harness,
    Unattach,
    Fateseal,
    Skip(SkippedPart),
    AdditionalPhase(AddedPhases),
    Cipher,
    Recruit,
    Unsuspect,
    Unprepare,
    Epic,
    Paradigm,
    CollectEvidence,
    PhaseIn,
    Ungoad,
    UnlockDoor,
    LockOrUnlockDoor,
    BecomeBlocked,
    Assimilate,
    RemoveFromCombatUnblocking,
    ReverseTurnOrder,
    LoseUnspentMana,
    DoubleUnspentMana,
    DrawGame,
    ShuffleGraveyard,
    RollPlanarDie,
    VisitAttractions,
    EndCombat,
    RollKeepHighest(u32),
    RollTwoInOrder(u32),
    TakeExtraTurnAfterNext,
    TakeExtraTurnSkippingUntap,
    PlaySubgame,
    ChooseDamageNumber,
    ChooseSector,
    ClaimPrize,
    ChooseProtection,
    SetStake,
    DoubleStake,
    ChooseCopy,
    TopOfPlanarDeckToBottom,
    RevealTopOfPlanarDeck,
    PlanarDeckChaos,
    PlanarDeckPlaneswalkWithoutLeaving,
    PlanarDeckPickPlane,
    PlanarDeckPlaneswalkToEach,
    PlanarDeckChaosEach,
    ShuffleIfSearched,
    RerollStoredDice,
    RollAndStoreD6,
    SetRepeatablePlayers,
    SetSchemeInMotion,
    ReorderGraveyard,
    DiscardAnyNumberAtRandom,
    RemainingToBottomAtRandom,
    ActivateManaAbility,
    TurnPileFaceUp,
    RemoveAllDamage,
}

pub(crate) struct Parts<'a> {
    pub kind: KeywordKind,
    pub object: Option<&'a ObjectRef>,
    pub other: Option<&'a ObjectRef>,
    pub amount: Option<&'a Expr>,
    pub subtype: Option<Cow<'a, str>>,
}
impl Parts<'_> {
    fn plain(kind: KeywordKind) -> Self {
        Self {
            kind,
            object: None,
            other: None,
            amount: None,
            subtype: None,
        }
    }
}
pub(crate) fn parts(action: &KeywordAction) -> Parts<'_> {
    match action {
        KeywordAction::Transform { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Transform)
        },
        KeywordAction::Fight { first, second } => Parts {
            object: Some(first),
            other: Some(second),
            ..Parts::plain(KeywordKind::Fight)
        },
        KeywordAction::Investigate { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::Investigate)
        },
        KeywordAction::Proliferate { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::Proliferate)
        },
        KeywordAction::Amass {
            amount,
            creature_type,
        } => Parts {
            amount: Some(amount),
            subtype: Some(Cow::Owned(format!("{creature_type:?}"))),
            ..Parts::plain(KeywordKind::Amass)
        },
        KeywordAction::Explore { object, times } => Parts {
            object: Some(object),
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::Explore)
        },
        KeywordAction::RingTempts => Parts {
            ..Parts::plain(KeywordKind::RingTempts)
        },
        KeywordAction::Connive { object, amount } => Parts {
            object: Some(object),
            amount: amount.as_ref(),
            ..Parts::plain(KeywordKind::Connive)
        },
        KeywordAction::Goad { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Goad)
        },
        KeywordAction::Earthbend { object, amount } => Parts {
            object: Some(object),
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Earthbend)
        },
        KeywordAction::Venture => Parts {
            ..Parts::plain(KeywordKind::Venture)
        },
        KeywordAction::ManifestDread { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::ManifestDread)
        },
        KeywordAction::Monstrosity { subject, amount } => Parts {
            object: subject.as_ref(),
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Monstrosity)
        },
        KeywordAction::Discover { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Discover)
        },
        KeywordAction::GainEnergy { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::GainEnergy)
        },
        KeywordAction::BecomeMonarch => Parts {
            ..Parts::plain(KeywordKind::BecomeMonarch)
        },
        KeywordAction::LoseGame => Parts {
            ..Parts::plain(KeywordKind::LoseGame)
        },
        KeywordAction::WinGame => Parts {
            ..Parts::plain(KeywordKind::WinGame)
        },
        KeywordAction::TakeExtraTurn { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::TakeExtraTurn)
        },
        KeywordAction::PhaseOut { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::PhaseOut)
        },
        KeywordAction::Prepare { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Prepare)
        },
        KeywordAction::TapOrUntap { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::TapOrUntap)
        },
        KeywordAction::ExchangeControl { first, second } => Parts {
            object: Some(first),
            other: Some(second),
            ..Parts::plain(KeywordKind::ExchangeControl)
        },
        KeywordAction::RollDie { sides, times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::RollDie(*sides))
        },
        KeywordAction::EmpowerJace { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::EmpowerJace)
        },
        KeywordAction::Incubate { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Incubate)
        },
        KeywordAction::Adapt { subject, amount } => Parts {
            object: subject.as_ref(),
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Adapt)
        },
        KeywordAction::Populate { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::Populate)
        },
        KeywordAction::Bolster { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Bolster)
        },
        KeywordAction::Learn => Parts {
            ..Parts::plain(KeywordKind::Learn)
        },
        KeywordAction::TakeInitiative => Parts {
            ..Parts::plain(KeywordKind::TakeInitiative)
        },
        KeywordAction::OpenAttraction { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::OpenAttraction)
        },
        KeywordAction::GetTickets { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::GetTickets)
        },
        KeywordAction::RemoveFromCombat { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::RemoveFromCombat)
        },
        KeywordAction::Flip { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Flip)
        },
        KeywordAction::Awaken { object, amount } => Parts {
            object: Some(object),
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Awaken)
        },
        KeywordAction::Planeswalk => Parts {
            ..Parts::plain(KeywordKind::Planeswalk)
        },
        KeywordAction::Convert { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Convert)
        },
        KeywordAction::Suspect { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Suspect)
        },
        KeywordAction::Endure { object, amount } => Parts {
            object: Some(object),
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Endure)
        },
        KeywordAction::Blight { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Blight)
        },
        KeywordAction::TurnFaceUp { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::TurnFaceUp)
        },
        KeywordAction::TurnFaceDown { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::TurnFaceDown)
        },
        KeywordAction::Detain { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Detain)
        },
        KeywordAction::EndTheTurn => Parts {
            ..Parts::plain(KeywordKind::EndTheTurn)
        },
        KeywordAction::TimeTravel { times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::TimeTravel)
        },
        KeywordAction::Intensify { object, times } => Parts {
            object: Some(object),
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::Intensify)
        },
        KeywordAction::Forage => Parts {
            ..Parts::plain(KeywordKind::Forage)
        },
        KeywordAction::Ascend => Parts {
            ..Parts::plain(KeywordKind::Ascend)
        },
        KeywordAction::BecomeDay => Parts {
            ..Parts::plain(KeywordKind::BecomeDay)
        },
        KeywordAction::BecomeNight => Parts {
            ..Parts::plain(KeywordKind::BecomeNight)
        },
        KeywordAction::ChaosEnsues => Parts {
            ..Parts::plain(KeywordKind::ChaosEnsues)
        },
        KeywordAction::Airbend { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Airbend)
        },
        KeywordAction::Saddle { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Saddle)
        },
        KeywordAction::Harness { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Harness)
        },
        KeywordAction::Unattach { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Unattach)
        },
        KeywordAction::Fateseal { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::Fateseal)
        },
        KeywordAction::Skip { part, times } => Parts {
            amount: times.as_ref(),
            ..Parts::plain(KeywordKind::Skip(*part))
        },
        KeywordAction::AdditionalPhase { phases } => Parts {
            ..Parts::plain(KeywordKind::AdditionalPhase(*phases))
        },
        KeywordAction::Cipher { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Cipher)
        },
        KeywordAction::Recruit => Parts {
            ..Parts::plain(KeywordKind::Recruit)
        },
        KeywordAction::Unsuspect { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Unsuspect)
        },
        KeywordAction::Unprepare { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Unprepare)
        },
        KeywordAction::Epic => Parts {
            ..Parts::plain(KeywordKind::Epic)
        },
        KeywordAction::Paradigm { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Paradigm)
        },
        KeywordAction::CollectEvidence { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::CollectEvidence)
        },
        KeywordAction::PhaseIn { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::PhaseIn)
        },
        KeywordAction::Ungoad { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Ungoad)
        },
        KeywordAction::UnlockDoor { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::UnlockDoor)
        },
        KeywordAction::LockOrUnlockDoor { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::LockOrUnlockDoor)
        },
        KeywordAction::BecomeBlocked { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::BecomeBlocked)
        },
        KeywordAction::Assimilate { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::Assimilate)
        },
        KeywordAction::RemoveFromCombatUnblocking { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::RemoveFromCombatUnblocking)
        },
        KeywordAction::ReverseTurnOrder => Parts {
            ..Parts::plain(KeywordKind::ReverseTurnOrder)
        },
        KeywordAction::LoseUnspentMana => Parts {
            ..Parts::plain(KeywordKind::LoseUnspentMana)
        },
        KeywordAction::DoubleUnspentMana => Parts {
            ..Parts::plain(KeywordKind::DoubleUnspentMana)
        },
        KeywordAction::DrawGame => Parts {
            ..Parts::plain(KeywordKind::DrawGame)
        },
        KeywordAction::ShuffleGraveyard => Parts {
            ..Parts::plain(KeywordKind::ShuffleGraveyard)
        },
        KeywordAction::RollPlanarDie => Parts {
            ..Parts::plain(KeywordKind::RollPlanarDie)
        },
        KeywordAction::VisitAttractions => Parts {
            ..Parts::plain(KeywordKind::VisitAttractions)
        },
        KeywordAction::EndCombat => Parts {
            ..Parts::plain(KeywordKind::EndCombat)
        },
        KeywordAction::RollKeepHighest { sides, amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::RollKeepHighest(*sides))
        },
        KeywordAction::RollTwoInOrder { sides } => Parts {
            ..Parts::plain(KeywordKind::RollTwoInOrder(*sides))
        },
        KeywordAction::TakeExtraTurnAfterNext => Parts {
            ..Parts::plain(KeywordKind::TakeExtraTurnAfterNext)
        },
        KeywordAction::TakeExtraTurnSkippingUntap => Parts {
            ..Parts::plain(KeywordKind::TakeExtraTurnSkippingUntap)
        },
        KeywordAction::PlaySubgame => Parts {
            ..Parts::plain(KeywordKind::PlaySubgame)
        },
        KeywordAction::ChooseDamageNumber => Parts {
            ..Parts::plain(KeywordKind::ChooseDamageNumber)
        },
        KeywordAction::ChooseSector => Parts {
            ..Parts::plain(KeywordKind::ChooseSector)
        },
        KeywordAction::ClaimPrize => Parts {
            ..Parts::plain(KeywordKind::ClaimPrize)
        },
        KeywordAction::ChooseProtection => Parts {
            ..Parts::plain(KeywordKind::ChooseProtection)
        },
        KeywordAction::SetStake { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::SetStake)
        },
        KeywordAction::DoubleStake => Parts {
            ..Parts::plain(KeywordKind::DoubleStake)
        },
        KeywordAction::ChooseCopy => Parts {
            ..Parts::plain(KeywordKind::ChooseCopy)
        },
        KeywordAction::TopOfPlanarDeckToBottom => Parts {
            ..Parts::plain(KeywordKind::TopOfPlanarDeckToBottom)
        },
        KeywordAction::RevealTopOfPlanarDeck => Parts {
            ..Parts::plain(KeywordKind::RevealTopOfPlanarDeck)
        },
        KeywordAction::PlanarDeckChaos => Parts {
            ..Parts::plain(KeywordKind::PlanarDeckChaos)
        },
        KeywordAction::PlanarDeckPlaneswalkWithoutLeaving => Parts {
            ..Parts::plain(KeywordKind::PlanarDeckPlaneswalkWithoutLeaving)
        },
        KeywordAction::PlanarDeckPickPlane { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::PlanarDeckPickPlane)
        },
        KeywordAction::PlanarDeckPlaneswalkToEach { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::PlanarDeckPlaneswalkToEach)
        },
        KeywordAction::PlanarDeckChaosEach { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::PlanarDeckChaosEach)
        },
        KeywordAction::ShuffleIfSearched => Parts {
            ..Parts::plain(KeywordKind::ShuffleIfSearched)
        },
        KeywordAction::RerollStoredDice => Parts {
            ..Parts::plain(KeywordKind::RerollStoredDice)
        },
        KeywordAction::RollAndStoreD6 { amount } => Parts {
            amount: Some(amount),
            ..Parts::plain(KeywordKind::RollAndStoreD6)
        },
        KeywordAction::SetRepeatablePlayers => Parts {
            ..Parts::plain(KeywordKind::SetRepeatablePlayers)
        },
        KeywordAction::SetSchemeInMotion { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::SetSchemeInMotion)
        },
        KeywordAction::ReorderGraveyard => Parts {
            ..Parts::plain(KeywordKind::ReorderGraveyard)
        },
        KeywordAction::DiscardAnyNumberAtRandom => Parts {
            ..Parts::plain(KeywordKind::DiscardAnyNumberAtRandom)
        },
        KeywordAction::RemainingToBottomAtRandom => Parts {
            ..Parts::plain(KeywordKind::RemainingToBottomAtRandom)
        },
        KeywordAction::ActivateManaAbility { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::ActivateManaAbility)
        },
        KeywordAction::TurnPileFaceUp => Parts {
            ..Parts::plain(KeywordKind::TurnPileFaceUp)
        },
        KeywordAction::RemoveAllDamage { object } => Parts {
            object: Some(object),
            ..Parts::plain(KeywordKind::RemoveAllDamage)
        },
    }
}

/// Strictly decode one historical/source action; irrelevant or missing fields
/// are errors. Validate the resulting action in its containing lexical scope.
pub fn from_parts(
    kind: KeywordKind,
    object: Option<ObjectRef>,
    other: Option<ObjectRef>,
    amount: Option<Expr>,
    subtype: Option<String>,
) -> Result<KeywordAction, String> {
    match (kind, object, other, amount, subtype) {
        (KeywordKind::Transform, Some(object), None, None, None) => {
            Ok(KeywordAction::Transform { object })
        }
        (KeywordKind::Fight, Some(first), Some(second), None, None) => {
            Ok(KeywordAction::Fight { first, second })
        }
        (KeywordKind::Investigate, None, None, times, None) => {
            Ok(KeywordAction::Investigate { times })
        }
        (KeywordKind::Proliferate, None, None, times, None) => {
            Ok(KeywordAction::Proliferate { times })
        }
        (KeywordKind::Amass, None, None, Some(amount), Some(creature_type)) => {
            Ok(KeywordAction::Amass {
                amount,
                creature_type: serde_json::from_value(serde_json::Value::String(creature_type))
                    .map_err(|e| format!("amass requires a creature type: {e}"))?,
            })
        }
        (KeywordKind::Explore, Some(object), None, times, None) => {
            Ok(KeywordAction::Explore { object, times })
        }
        (KeywordKind::RingTempts, None, None, None, None) => Ok(KeywordAction::RingTempts),
        (KeywordKind::Connive, Some(object), None, amount, None) => {
            Ok(KeywordAction::Connive { object, amount })
        }
        (KeywordKind::Goad, Some(object), None, None, None) => Ok(KeywordAction::Goad { object }),
        (KeywordKind::Earthbend, Some(object), None, Some(amount), None) => {
            Ok(KeywordAction::Earthbend { object, amount })
        }
        (KeywordKind::Venture, None, None, None, None) => Ok(KeywordAction::Venture),
        (KeywordKind::ManifestDread, None, None, times, None) => {
            Ok(KeywordAction::ManifestDread { times })
        }
        (KeywordKind::Monstrosity, subject, None, Some(amount), None) => {
            Ok(KeywordAction::Monstrosity { subject, amount })
        }
        (KeywordKind::Discover, None, None, Some(amount), None) => {
            Ok(KeywordAction::Discover { amount })
        }
        (KeywordKind::GainEnergy, None, None, Some(amount), None) => {
            Ok(KeywordAction::GainEnergy { amount })
        }
        (KeywordKind::BecomeMonarch, None, None, None, None) => Ok(KeywordAction::BecomeMonarch),
        (KeywordKind::LoseGame, None, None, None, None) => Ok(KeywordAction::LoseGame),
        (KeywordKind::WinGame, None, None, None, None) => Ok(KeywordAction::WinGame),
        (KeywordKind::TakeExtraTurn, None, None, times, None) => {
            Ok(KeywordAction::TakeExtraTurn { times })
        }
        (KeywordKind::PhaseOut, Some(object), None, None, None) => {
            Ok(KeywordAction::PhaseOut { object })
        }
        (KeywordKind::Prepare, Some(object), None, None, None) => {
            Ok(KeywordAction::Prepare { object })
        }
        (KeywordKind::TapOrUntap, Some(object), None, None, None) => {
            Ok(KeywordAction::TapOrUntap { object })
        }
        (KeywordKind::ExchangeControl, Some(first), Some(second), None, None) => {
            Ok(KeywordAction::ExchangeControl { first, second })
        }
        (KeywordKind::RollDie(sides), None, None, times, None) => {
            Ok(KeywordAction::RollDie { sides, times })
        }
        (KeywordKind::EmpowerJace, None, None, Some(amount), None) => {
            Ok(KeywordAction::EmpowerJace { amount })
        }
        (KeywordKind::Incubate, None, None, Some(amount), None) => {
            Ok(KeywordAction::Incubate { amount })
        }
        (KeywordKind::Adapt, subject, None, Some(amount), None) => {
            Ok(KeywordAction::Adapt { subject, amount })
        }
        (KeywordKind::Populate, None, None, times, None) => Ok(KeywordAction::Populate { times }),
        (KeywordKind::Bolster, None, None, Some(amount), None) => {
            Ok(KeywordAction::Bolster { amount })
        }
        (KeywordKind::Learn, None, None, None, None) => Ok(KeywordAction::Learn),
        (KeywordKind::TakeInitiative, None, None, None, None) => Ok(KeywordAction::TakeInitiative),
        (KeywordKind::OpenAttraction, None, None, times, None) => {
            Ok(KeywordAction::OpenAttraction { times })
        }
        (KeywordKind::GetTickets, None, None, Some(amount), None) => {
            Ok(KeywordAction::GetTickets { amount })
        }
        (KeywordKind::RemoveFromCombat, Some(object), None, None, None) => {
            Ok(KeywordAction::RemoveFromCombat { object })
        }
        (KeywordKind::Flip, Some(object), None, None, None) => Ok(KeywordAction::Flip { object }),
        (KeywordKind::Awaken, Some(object), None, Some(amount), None) => {
            Ok(KeywordAction::Awaken { object, amount })
        }
        (KeywordKind::Planeswalk, None, None, None, None) => Ok(KeywordAction::Planeswalk),
        (KeywordKind::Convert, Some(object), None, None, None) => {
            Ok(KeywordAction::Convert { object })
        }
        (KeywordKind::Suspect, Some(object), None, None, None) => {
            Ok(KeywordAction::Suspect { object })
        }
        (KeywordKind::Endure, Some(object), None, Some(amount), None) => {
            Ok(KeywordAction::Endure { object, amount })
        }
        (KeywordKind::Blight, None, None, Some(amount), None) => {
            Ok(KeywordAction::Blight { amount })
        }
        (KeywordKind::TurnFaceUp, Some(object), None, None, None) => {
            Ok(KeywordAction::TurnFaceUp { object })
        }
        (KeywordKind::TurnFaceDown, Some(object), None, None, None) => {
            Ok(KeywordAction::TurnFaceDown { object })
        }
        (KeywordKind::Detain, Some(object), None, None, None) => {
            Ok(KeywordAction::Detain { object })
        }
        (KeywordKind::EndTheTurn, None, None, None, None) => Ok(KeywordAction::EndTheTurn),
        (KeywordKind::TimeTravel, None, None, times, None) => {
            Ok(KeywordAction::TimeTravel { times })
        }
        (KeywordKind::Intensify, Some(object), None, times, None) => {
            Ok(KeywordAction::Intensify { object, times })
        }
        (KeywordKind::Forage, None, None, None, None) => Ok(KeywordAction::Forage),
        (KeywordKind::Ascend, None, None, None, None) => Ok(KeywordAction::Ascend),
        (KeywordKind::BecomeDay, None, None, None, None) => Ok(KeywordAction::BecomeDay),
        (KeywordKind::BecomeNight, None, None, None, None) => Ok(KeywordAction::BecomeNight),
        (KeywordKind::ChaosEnsues, None, None, None, None) => Ok(KeywordAction::ChaosEnsues),
        (KeywordKind::Airbend, Some(object), None, None, None) => {
            Ok(KeywordAction::Airbend { object })
        }
        (KeywordKind::Saddle, Some(object), None, None, None) => {
            Ok(KeywordAction::Saddle { object })
        }
        (KeywordKind::Harness, Some(object), None, None, None) => {
            Ok(KeywordAction::Harness { object })
        }
        (KeywordKind::Unattach, Some(object), None, None, None) => {
            Ok(KeywordAction::Unattach { object })
        }
        (KeywordKind::Fateseal, None, None, Some(amount), None) => {
            Ok(KeywordAction::Fateseal { amount })
        }
        (KeywordKind::Skip(part), None, None, times, None) => {
            Ok(KeywordAction::Skip { part, times })
        }
        (KeywordKind::AdditionalPhase(phases), None, None, None, None) => {
            Ok(KeywordAction::AdditionalPhase { phases })
        }
        (KeywordKind::Cipher, Some(object), None, None, None) => {
            Ok(KeywordAction::Cipher { object })
        }
        (KeywordKind::Recruit, None, None, None, None) => Ok(KeywordAction::Recruit),
        (KeywordKind::Unsuspect, Some(object), None, None, None) => {
            Ok(KeywordAction::Unsuspect { object })
        }
        (KeywordKind::Unprepare, Some(object), None, None, None) => {
            Ok(KeywordAction::Unprepare { object })
        }
        (KeywordKind::Epic, None, None, None, None) => Ok(KeywordAction::Epic),
        (KeywordKind::Paradigm, Some(object), None, None, None) => {
            Ok(KeywordAction::Paradigm { object })
        }
        (KeywordKind::CollectEvidence, None, None, Some(amount), None) => {
            Ok(KeywordAction::CollectEvidence { amount })
        }
        (KeywordKind::PhaseIn, Some(object), None, None, None) => {
            Ok(KeywordAction::PhaseIn { object })
        }
        (KeywordKind::Ungoad, Some(object), None, None, None) => {
            Ok(KeywordAction::Ungoad { object })
        }
        (KeywordKind::UnlockDoor, Some(object), None, None, None) => {
            Ok(KeywordAction::UnlockDoor { object })
        }
        (KeywordKind::LockOrUnlockDoor, Some(object), None, None, None) => {
            Ok(KeywordAction::LockOrUnlockDoor { object })
        }
        (KeywordKind::BecomeBlocked, Some(object), None, None, None) => {
            Ok(KeywordAction::BecomeBlocked { object })
        }
        (KeywordKind::Assimilate, Some(object), None, None, None) => {
            Ok(KeywordAction::Assimilate { object })
        }
        (KeywordKind::RemoveFromCombatUnblocking, Some(object), None, None, None) => {
            Ok(KeywordAction::RemoveFromCombatUnblocking { object })
        }
        (KeywordKind::ReverseTurnOrder, None, None, None, None) => {
            Ok(KeywordAction::ReverseTurnOrder)
        }
        (KeywordKind::LoseUnspentMana, None, None, None, None) => {
            Ok(KeywordAction::LoseUnspentMana)
        }
        (KeywordKind::DoubleUnspentMana, None, None, None, None) => {
            Ok(KeywordAction::DoubleUnspentMana)
        }
        (KeywordKind::DrawGame, None, None, None, None) => Ok(KeywordAction::DrawGame),
        (KeywordKind::ShuffleGraveyard, None, None, None, None) => {
            Ok(KeywordAction::ShuffleGraveyard)
        }
        (KeywordKind::RollPlanarDie, None, None, None, None) => Ok(KeywordAction::RollPlanarDie),
        (KeywordKind::VisitAttractions, None, None, None, None) => {
            Ok(KeywordAction::VisitAttractions)
        }
        (KeywordKind::EndCombat, None, None, None, None) => Ok(KeywordAction::EndCombat),
        (KeywordKind::RollKeepHighest(sides), None, None, Some(amount), None) => {
            Ok(KeywordAction::RollKeepHighest { sides, amount })
        }
        (KeywordKind::RollTwoInOrder(sides), None, None, None, None) => {
            Ok(KeywordAction::RollTwoInOrder { sides })
        }
        (KeywordKind::TakeExtraTurnAfterNext, None, None, None, None) => {
            Ok(KeywordAction::TakeExtraTurnAfterNext)
        }
        (KeywordKind::TakeExtraTurnSkippingUntap, None, None, None, None) => {
            Ok(KeywordAction::TakeExtraTurnSkippingUntap)
        }
        (KeywordKind::PlaySubgame, None, None, None, None) => Ok(KeywordAction::PlaySubgame),
        (KeywordKind::ChooseDamageNumber, None, None, None, None) => {
            Ok(KeywordAction::ChooseDamageNumber)
        }
        (KeywordKind::ChooseSector, None, None, None, None) => Ok(KeywordAction::ChooseSector),
        (KeywordKind::ClaimPrize, None, None, None, None) => Ok(KeywordAction::ClaimPrize),
        (KeywordKind::ChooseProtection, None, None, None, None) => {
            Ok(KeywordAction::ChooseProtection)
        }
        (KeywordKind::SetStake, None, None, Some(amount), None) => {
            Ok(KeywordAction::SetStake { amount })
        }
        (KeywordKind::DoubleStake, None, None, None, None) => Ok(KeywordAction::DoubleStake),
        (KeywordKind::ChooseCopy, None, None, None, None) => Ok(KeywordAction::ChooseCopy),
        (KeywordKind::TopOfPlanarDeckToBottom, None, None, None, None) => {
            Ok(KeywordAction::TopOfPlanarDeckToBottom)
        }
        (KeywordKind::RevealTopOfPlanarDeck, None, None, None, None) => {
            Ok(KeywordAction::RevealTopOfPlanarDeck)
        }
        (KeywordKind::PlanarDeckChaos, None, None, None, None) => {
            Ok(KeywordAction::PlanarDeckChaos)
        }
        (KeywordKind::PlanarDeckPlaneswalkWithoutLeaving, None, None, None, None) => {
            Ok(KeywordAction::PlanarDeckPlaneswalkWithoutLeaving)
        }
        (KeywordKind::PlanarDeckPickPlane, None, None, Some(amount), None) => {
            Ok(KeywordAction::PlanarDeckPickPlane { amount })
        }
        (KeywordKind::PlanarDeckPlaneswalkToEach, None, None, Some(amount), None) => {
            Ok(KeywordAction::PlanarDeckPlaneswalkToEach { amount })
        }
        (KeywordKind::PlanarDeckChaosEach, None, None, Some(amount), None) => {
            Ok(KeywordAction::PlanarDeckChaosEach { amount })
        }
        (KeywordKind::ShuffleIfSearched, None, None, None, None) => {
            Ok(KeywordAction::ShuffleIfSearched)
        }
        (KeywordKind::RerollStoredDice, None, None, None, None) => {
            Ok(KeywordAction::RerollStoredDice)
        }
        (KeywordKind::RollAndStoreD6, None, None, Some(amount), None) => {
            Ok(KeywordAction::RollAndStoreD6 { amount })
        }
        (KeywordKind::SetRepeatablePlayers, None, None, None, None) => {
            Ok(KeywordAction::SetRepeatablePlayers)
        }
        (KeywordKind::SetSchemeInMotion, Some(object), None, None, None) => {
            Ok(KeywordAction::SetSchemeInMotion { object })
        }
        (KeywordKind::ReorderGraveyard, None, None, None, None) => {
            Ok(KeywordAction::ReorderGraveyard)
        }
        (KeywordKind::DiscardAnyNumberAtRandom, None, None, None, None) => {
            Ok(KeywordAction::DiscardAnyNumberAtRandom)
        }
        (KeywordKind::RemainingToBottomAtRandom, None, None, None, None) => {
            Ok(KeywordAction::RemainingToBottomAtRandom)
        }
        (KeywordKind::ActivateManaAbility, Some(object), None, None, None) => {
            Ok(KeywordAction::ActivateManaAbility { object })
        }
        (KeywordKind::TurnPileFaceUp, None, None, None, None) => Ok(KeywordAction::TurnPileFaceUp),
        (KeywordKind::RemoveAllDamage, Some(object), None, None, None) => {
            Ok(KeywordAction::RemoveAllDamage { object })
        }
        (kind, ..) => Err(format!("invalid parameters for keyword action {kind:?}")),
    }
}
/// Static source rules use this boundary too. An invalid signature becomes
/// explicitly unreviewed source material, so validation refuses it.
pub(crate) fn operation(
    actor: &PlayerSet,
    kind: KeywordKind,
    object: Option<ObjectRef>,
    other: Option<ObjectRef>,
    amount: Option<Expr>,
    subtype: Option<String>,
) -> Operation {
    match from_parts(
        kind,
        object.clone(),
        other.clone(),
        amount.clone(),
        subtype.clone(),
    ) {
        Ok(action) => Operation::Keyword {
            actor: actor.clone(),
            action,
        },
        Err(_) => Operation::Primitive {
            actor: actor.clone(),
            name: format!("InvalidKeyword::{kind:?}"),
            operands: object
                .into_iter()
                .chain(other)
                .map(Operand::Object)
                .chain(amount.map(Operand::Number))
                .chain(subtype.map(Operand::Text))
                .collect(),
        },
    }
}
/// Decode a historical action on a set without changing its batch boundary.
pub fn each_from_parts(
    kind: KeywordKind,
    until: Option<Duration>,
) -> Result<ObjectKeywordAction, String> {
    match (kind, until) {
        (KeywordKind::Transform, None) => Ok(ObjectKeywordAction::Transform),
        (KeywordKind::Explore, None) => Ok(ObjectKeywordAction::Explore),
        (KeywordKind::Connive, None) => Ok(ObjectKeywordAction::Connive),
        (KeywordKind::Goad, until) => Ok(ObjectKeywordAction::Goad { until }),
        (KeywordKind::PhaseOut, until) => Ok(ObjectKeywordAction::PhaseOut { until }),
        (KeywordKind::Prepare, None) => Ok(ObjectKeywordAction::Prepare),
        (KeywordKind::TapOrUntap, None) => Ok(ObjectKeywordAction::TapOrUntap),
        (KeywordKind::RemoveFromCombat, None) => Ok(ObjectKeywordAction::RemoveFromCombat),
        (KeywordKind::Flip, None) => Ok(ObjectKeywordAction::Flip),
        (KeywordKind::Convert, None) => Ok(ObjectKeywordAction::Convert),
        (KeywordKind::Suspect, None) => Ok(ObjectKeywordAction::Suspect),
        (KeywordKind::TurnFaceUp, None) => Ok(ObjectKeywordAction::TurnFaceUp),
        (KeywordKind::TurnFaceDown, None) => Ok(ObjectKeywordAction::TurnFaceDown),
        (KeywordKind::Detain, None) => Ok(ObjectKeywordAction::Detain),
        (KeywordKind::Intensify, None) => Ok(ObjectKeywordAction::Intensify),
        (KeywordKind::Airbend, None) => Ok(ObjectKeywordAction::Airbend),
        (KeywordKind::Saddle, None) => Ok(ObjectKeywordAction::Saddle),
        (KeywordKind::Harness, None) => Ok(ObjectKeywordAction::Harness),
        (KeywordKind::Unattach, None) => Ok(ObjectKeywordAction::Unattach),
        (KeywordKind::Cipher, None) => Ok(ObjectKeywordAction::Cipher),
        (KeywordKind::Unsuspect, None) => Ok(ObjectKeywordAction::Unsuspect),
        (KeywordKind::Unprepare, None) => Ok(ObjectKeywordAction::Unprepare),
        (KeywordKind::Paradigm, None) => Ok(ObjectKeywordAction::Paradigm),
        (KeywordKind::PhaseIn, None) => Ok(ObjectKeywordAction::PhaseIn),
        (KeywordKind::Ungoad, None) => Ok(ObjectKeywordAction::Ungoad),
        (KeywordKind::UnlockDoor, None) => Ok(ObjectKeywordAction::UnlockDoor),
        (KeywordKind::LockOrUnlockDoor, None) => Ok(ObjectKeywordAction::LockOrUnlockDoor),
        (KeywordKind::BecomeBlocked, None) => Ok(ObjectKeywordAction::BecomeBlocked),
        (KeywordKind::Assimilate, None) => Ok(ObjectKeywordAction::Assimilate),
        (KeywordKind::RemoveFromCombatUnblocking, None) => {
            Ok(ObjectKeywordAction::RemoveFromCombatUnblocking)
        }
        (KeywordKind::SetSchemeInMotion, None) => Ok(ObjectKeywordAction::SetSchemeInMotion),
        (KeywordKind::ActivateManaAbility, None) => Ok(ObjectKeywordAction::ActivateManaAbility),
        (KeywordKind::RemoveAllDamage, None) => Ok(ObjectKeywordAction::RemoveAllDamage),
        (kind, _) => Err(format!("invalid set keyword action {kind:?}")),
    }
}
pub(crate) fn each_parts(action: &ObjectKeywordAction) -> (KeywordKind, Option<&Duration>) {
    match action {
        ObjectKeywordAction::Transform => (KeywordKind::Transform, None),
        ObjectKeywordAction::Explore => (KeywordKind::Explore, None),
        ObjectKeywordAction::Connive => (KeywordKind::Connive, None),
        ObjectKeywordAction::Goad { until } => (KeywordKind::Goad, until.as_ref()),
        ObjectKeywordAction::PhaseOut { until } => (KeywordKind::PhaseOut, until.as_ref()),
        ObjectKeywordAction::Prepare => (KeywordKind::Prepare, None),
        ObjectKeywordAction::TapOrUntap => (KeywordKind::TapOrUntap, None),
        ObjectKeywordAction::RemoveFromCombat => (KeywordKind::RemoveFromCombat, None),
        ObjectKeywordAction::Flip => (KeywordKind::Flip, None),
        ObjectKeywordAction::Convert => (KeywordKind::Convert, None),
        ObjectKeywordAction::Suspect => (KeywordKind::Suspect, None),
        ObjectKeywordAction::TurnFaceUp => (KeywordKind::TurnFaceUp, None),
        ObjectKeywordAction::TurnFaceDown => (KeywordKind::TurnFaceDown, None),
        ObjectKeywordAction::Detain => (KeywordKind::Detain, None),
        ObjectKeywordAction::Intensify => (KeywordKind::Intensify, None),
        ObjectKeywordAction::Airbend => (KeywordKind::Airbend, None),
        ObjectKeywordAction::Saddle => (KeywordKind::Saddle, None),
        ObjectKeywordAction::Harness => (KeywordKind::Harness, None),
        ObjectKeywordAction::Unattach => (KeywordKind::Unattach, None),
        ObjectKeywordAction::Cipher => (KeywordKind::Cipher, None),
        ObjectKeywordAction::Unsuspect => (KeywordKind::Unsuspect, None),
        ObjectKeywordAction::Unprepare => (KeywordKind::Unprepare, None),
        ObjectKeywordAction::Paradigm => (KeywordKind::Paradigm, None),
        ObjectKeywordAction::PhaseIn => (KeywordKind::PhaseIn, None),
        ObjectKeywordAction::Ungoad => (KeywordKind::Ungoad, None),
        ObjectKeywordAction::UnlockDoor => (KeywordKind::UnlockDoor, None),
        ObjectKeywordAction::LockOrUnlockDoor => (KeywordKind::LockOrUnlockDoor, None),
        ObjectKeywordAction::BecomeBlocked => (KeywordKind::BecomeBlocked, None),
        ObjectKeywordAction::Assimilate => (KeywordKind::Assimilate, None),
        ObjectKeywordAction::RemoveFromCombatUnblocking => {
            (KeywordKind::RemoveFromCombatUnblocking, None)
        }
        ObjectKeywordAction::SetSchemeInMotion => (KeywordKind::SetSchemeInMotion, None),
        ObjectKeywordAction::ActivateManaAbility => (KeywordKind::ActivateManaAbility, None),
        ObjectKeywordAction::RemoveAllDamage => (KeywordKind::RemoveAllDamage, None),
    }
}

pub(crate) fn each_operation(
    actor: &PlayerSet,
    kind: KeywordKind,
    objects: ObjectSet,
    until: Option<Duration>,
) -> Operation {
    match each_from_parts(kind, until.clone()) {
        Ok(action) => Operation::KeywordEach {
            actor: actor.clone(),
            action,
            objects,
        },
        Err(_) => Operation::Primitive {
            actor: actor.clone(),
            name: format!("InvalidSetKeyword::{kind:?}"),
            operands: std::iter::once(Operand::Objects(objects))
                .chain(until.map(Operand::Duration))
                .collect(),
        },
    }
}

/// Exact legacy field origins, independent of compiler label values.
pub(crate) fn legacy_fields(action: &KeywordAction) -> &'static [(&'static str, &'static str)] {
    match action {
        KeywordAction::Adapt { .. } | KeywordAction::Monstrosity { .. } => {
            &[("object", "subject"), ("amount", "amount")]
        }
        KeywordAction::Transform { .. }
        | KeywordAction::Goad { .. }
        | KeywordAction::PhaseOut { .. }
        | KeywordAction::Prepare { .. }
        | KeywordAction::TapOrUntap { .. }
        | KeywordAction::RemoveFromCombat { .. }
        | KeywordAction::Flip { .. }
        | KeywordAction::Convert { .. }
        | KeywordAction::Suspect { .. }
        | KeywordAction::TurnFaceUp { .. }
        | KeywordAction::TurnFaceDown { .. }
        | KeywordAction::Detain { .. }
        | KeywordAction::Airbend { .. }
        | KeywordAction::Saddle { .. }
        | KeywordAction::Harness { .. }
        | KeywordAction::Unattach { .. }
        | KeywordAction::Cipher { .. }
        | KeywordAction::Unsuspect { .. }
        | KeywordAction::Unprepare { .. }
        | KeywordAction::Paradigm { .. }
        | KeywordAction::PhaseIn { .. }
        | KeywordAction::Ungoad { .. }
        | KeywordAction::UnlockDoor { .. }
        | KeywordAction::LockOrUnlockDoor { .. }
        | KeywordAction::BecomeBlocked { .. }
        | KeywordAction::Assimilate { .. }
        | KeywordAction::RemoveFromCombatUnblocking { .. }
        | KeywordAction::SetSchemeInMotion { .. }
        | KeywordAction::ActivateManaAbility { .. }
        | KeywordAction::RemoveAllDamage { .. } => &[("object", "object")],
        KeywordAction::Fight { .. } | KeywordAction::ExchangeControl { .. } => {
            &[("object", "first"), ("other", "second")]
        }
        KeywordAction::Investigate { .. }
        | KeywordAction::Proliferate { .. }
        | KeywordAction::ManifestDread { .. }
        | KeywordAction::TakeExtraTurn { .. }
        | KeywordAction::RollDie { .. }
        | KeywordAction::Populate { .. }
        | KeywordAction::OpenAttraction { .. }
        | KeywordAction::TimeTravel { .. }
        | KeywordAction::Skip { .. } => &[("amount", "times")],
        KeywordAction::Amass { .. } => &[("amount", "amount"), ("subtype", "creature_type")],
        KeywordAction::Explore { .. } | KeywordAction::Intensify { .. } => {
            &[("object", "object"), ("amount", "times")]
        }
        KeywordAction::RingTempts
        | KeywordAction::Venture
        | KeywordAction::BecomeMonarch
        | KeywordAction::LoseGame
        | KeywordAction::WinGame
        | KeywordAction::Learn
        | KeywordAction::TakeInitiative
        | KeywordAction::Planeswalk
        | KeywordAction::EndTheTurn
        | KeywordAction::Forage
        | KeywordAction::Ascend
        | KeywordAction::BecomeDay
        | KeywordAction::BecomeNight
        | KeywordAction::ChaosEnsues
        | KeywordAction::AdditionalPhase { .. }
        | KeywordAction::Recruit
        | KeywordAction::Epic
        | KeywordAction::ReverseTurnOrder
        | KeywordAction::LoseUnspentMana
        | KeywordAction::DoubleUnspentMana
        | KeywordAction::DrawGame
        | KeywordAction::ShuffleGraveyard
        | KeywordAction::RollPlanarDie
        | KeywordAction::VisitAttractions
        | KeywordAction::EndCombat
        | KeywordAction::RollTwoInOrder { .. }
        | KeywordAction::TakeExtraTurnAfterNext
        | KeywordAction::TakeExtraTurnSkippingUntap
        | KeywordAction::PlaySubgame
        | KeywordAction::ChooseDamageNumber
        | KeywordAction::ChooseSector
        | KeywordAction::ClaimPrize
        | KeywordAction::ChooseProtection
        | KeywordAction::DoubleStake
        | KeywordAction::ChooseCopy
        | KeywordAction::TopOfPlanarDeckToBottom
        | KeywordAction::RevealTopOfPlanarDeck
        | KeywordAction::PlanarDeckChaos
        | KeywordAction::PlanarDeckPlaneswalkWithoutLeaving
        | KeywordAction::ShuffleIfSearched
        | KeywordAction::RerollStoredDice
        | KeywordAction::SetRepeatablePlayers
        | KeywordAction::ReorderGraveyard
        | KeywordAction::DiscardAnyNumberAtRandom
        | KeywordAction::RemainingToBottomAtRandom
        | KeywordAction::TurnPileFaceUp => &[],
        KeywordAction::Connive { .. }
        | KeywordAction::Earthbend { .. }
        | KeywordAction::Awaken { .. }
        | KeywordAction::Endure { .. } => &[("object", "object"), ("amount", "amount")],
        KeywordAction::Discover { .. }
        | KeywordAction::GainEnergy { .. }
        | KeywordAction::EmpowerJace { .. }
        | KeywordAction::Incubate { .. }
        | KeywordAction::Bolster { .. }
        | KeywordAction::GetTickets { .. }
        | KeywordAction::Blight { .. }
        | KeywordAction::Fateseal { .. }
        | KeywordAction::CollectEvidence { .. }
        | KeywordAction::RollKeepHighest { .. }
        | KeywordAction::SetStake { .. }
        | KeywordAction::PlanarDeckPickPlane { .. }
        | KeywordAction::PlanarDeckPlaneswalkToEach { .. }
        | KeywordAction::PlanarDeckChaosEach { .. }
        | KeywordAction::RollAndStoreD6 { .. } => &[("amount", "amount")],
    }
}
