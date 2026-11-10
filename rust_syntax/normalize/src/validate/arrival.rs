//! Check zone-specific arrival state after its ordinary lexical references.
//! Counters and granted rules can belong to exiled cards (Delay). Views of
//! typed destination payloads retain the same reference and zone checks.
use super::*;

pub(super) fn destination(entry: &EntrySpec, destination: &Destination) -> Result {
    at_destination(entry, destination, true)
}

fn at_destination(
    entry: &EntrySpec,
    destination: &Destination,
    visibility_in_entry: bool,
) -> Result {
    let zone = match destination {
        Destination::Battlefield(_) | Destination::OwnersBattlefield => EventZone::Battlefield,
        Destination::Exile => EventZone::Exile,
        Destination::OwnersLibraryTop
        | Destination::OwnersLibraryBottom
        | Destination::OwnersLibraryShuffled
        | Destination::OwnersLibraryFromTop(_)
        | Destination::OwnersLibraryBottomRandom
        | Destination::OwnersLibraryTopOrBottom => EventZone::Library,
        Destination::OwnersHand | Destination::Hand(_) => EventZone::Hand,
        Destination::OwnersGraveyard => EventZone::Graveyard,
        Destination::CommandZone => EventZone::Command,
        Destination::Ante => EventZone::Ante,
        Destination::OwnersPlanarDeck(_) => EventZone::PlanarDeck,
    };
    check(entry, &zone, visibility_in_entry)
}

pub(super) fn battlefield(entry: &dyn crate::movement::ArrivalView) -> Result {
    let entry = entry.entry_view();
    check(&entry, &EventZone::Battlefield, false)
}

pub(super) fn conjure(entry: &EntrySpec, zone: &EventZone, spot: &Option<ConjureSpot>) -> Result {
    if spot.is_some() && !matches!(zone, EventZone::Library) {
        return Err("conjure library placement requires a library destination".into());
    }
    // Conjure and the other specialized actions carry hidden exile in their
    // entry state; ordinary Move instead has a distinct visibility field.
    check(entry, zone, true)
}

fn check(entry: &EntrySpec, zone: &EventZone, visibility_in_entry: bool) -> Result {
    let battlefield = matches!(zone, EventZone::Battlefield);
    let exile = matches!(zone, EventZone::Exile);
    if entry.tapped && !battlefield {
        return Err("tapped arrival requires a battlefield destination".into());
    }
    if entry.controller.is_some() && !battlefield {
        return Err("entering controller requires a battlefield destination".into());
    }
    for state in &entry.states {
        match state {
            EntryState::Transformed
            | EntryState::Prepared
            | EntryState::CopyOf { .. }
            | EntryState::Attacking { .. }
            | EntryState::Blocking(_)
            | EntryState::AttachedTo(_)
            | EntryState::AttachedToChosen(_)
            | EntryState::AttachedToPlayer(_)
            | EntryState::Flipped
            | EntryState::FaceDownAs(_)
            | EntryState::FaceDownWith { .. }
            | EntryState::FaceDownLand(_)
            | EntryState::NotAnAura
            | EntryState::NotedCounters
                if !battlefield =>
            {
                return Err("permanent arrival state requires a battlefield destination".into());
            }
            EntryState::FaceDown if !battlefield && !(visibility_in_entry && exile) => {
                return Err("face-down permanent arrival requires a battlefield destination; exile uses movement visibility".into());
            }
            EntryState::ExilePile { .. }
            | EntryState::ExiledFor(_)
            | EntryState::ExiledForWith { .. }
                if !exile =>
            {
                return Err("exile arrival state requires an exile destination".into());
            }
            EntryState::When { entry, .. } | EntryState::Optional { entry, .. } => {
                check(entry, zone, visibility_in_entry)?
            }
            _ => {}
        }
    }
    Ok(())
}
