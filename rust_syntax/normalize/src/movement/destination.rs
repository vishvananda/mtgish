//! One conversion from historical/source movement fields to zone-specific
//! destinations. Source lowering and saved-document migration use the same
//! conversion; generators consume the resulting typed destination.
use super::*;
use serde_json::{json, Value};

pub(crate) struct DestinationTransfer {
    pub destination: MovementDestination,
    pub qualifiers: Vec<SelectionConstraint>,
    /// Prefix moves from the supplied parts to the destination and qualifiers.
    /// Entries are recorded only for migration, never inferred from ID values.
    pub moves: Vec<(String, String)>,
}
#[derive(Clone, Copy)]
enum Zone {
    Battlefield,
    Exile,
    Other,
}

pub(crate) fn destination_from_parts(
    destination: &Destination,
    entry: &EntrySpec,
    face_down: bool,
    terms: &MovementTerms,
    paths: Option<(&str, &str, &str, &str, &str)>,
) -> Result<DestinationTransfer, String> {
    let (old_destination, old_entry, old_terms, new_destination, new_qualifiers) =
        paths.unwrap_or(("", "", "", "", ""));
    let record = paths.is_some();
    let mut moves = vec![];
    let library = matches!(
        destination,
        Destination::OwnersLibraryTop
            | Destination::OwnersLibraryBottom
            | Destination::OwnersLibraryShuffled
            | Destination::OwnersLibraryFromTop(_)
            | Destination::OwnersLibraryBottomRandom
            | Destination::OwnersLibraryTopOrBottom
    );
    if !library
        && (!matches!(terms.order, MovementOrder::DestinationDefault)
            || !matches!(terms.placement, MovementPlacement::DestinationDefault))
    {
        return Err("movement ordering and placement require a library destination".into());
    }
    if face_down && !matches!(destination, Destination::Exile) {
        return Err("hidden movement requires an exile destination".into());
    }
    if !matches!(destination, Destination::OwnersLibraryTopOrBottom)
        && !matches!(terms.placement, MovementPlacement::DestinationDefault)
    {
        return Err("a placement chooser requires a top-or-bottom destination".into());
    }
    let zone = match destination {
        Destination::Battlefield(_) | Destination::OwnersBattlefield => Zone::Battlefield,
        Destination::Exile => Zone::Exile,
        _ => Zone::Other,
    };
    let (variant, arrival_path) = match destination {
        Destination::Battlefield(_) | Destination::OwnersBattlefield => (
            "Battlefield",
            format!("{new_destination}/Battlefield/arrival"),
        ),
        Destination::Exile => ("Exile", format!("{new_destination}/Exile/arrival")),
        _ if library => ("Library", format!("{new_destination}/Library/arrival")),
        Destination::OwnersHand => ("OwnersHand", format!("{new_destination}/OwnersHand")),
        Destination::Hand(_) => ("Hand", format!("{new_destination}/Hand/arrival")),
        Destination::OwnersGraveyard => (
            "OwnersGraveyard",
            format!("{new_destination}/OwnersGraveyard"),
        ),
        Destination::CommandZone => ("CommandZone", format!("{new_destination}/CommandZone")),
        Destination::Ante => ("Ante", format!("{new_destination}/Ante")),
        Destination::OwnersPlanarDeck(_) => (
            "OwnersPlanarDeck",
            format!("{new_destination}/OwnersPlanarDeck/arrival"),
        ),
        _ => unreachable!("all library destinations handled"),
    };
    let raw = serde_json::to_value(entry).map_err(|e| e.to_string())?;
    let (arrival, hidden) = arrival(
        &raw,
        zone,
        old_entry,
        &arrival_path,
        record,
        true,
        &mut moves,
    )?;
    let value = match destination {
        Destination::Battlefield(player) => {
            if record {
                moves.push((
                    format!("{old_destination}/Battlefield"),
                    format!("{new_destination}/Battlefield/controller/Player"),
                ));
            }
            json!({"Battlefield":{"controller":{"Player":player},"arrival":arrival}})
        }
        Destination::OwnersBattlefield => {
            json!({"Battlefield":{"controller":"Owner","arrival":arrival}})
        }
        Destination::Exile => {
            json!({"Exile":{"visibility":if face_down||hidden{"FaceDown"}else{"FaceUp"},"arrival":arrival}})
        }
        _ if library => {
            let position = match destination {
                Destination::OwnersLibraryTop => json!("Top"),
                Destination::OwnersLibraryBottom => json!("Bottom"),
                Destination::OwnersLibraryShuffled => json!("Shuffled"),
                Destination::OwnersLibraryBottomRandom => json!("BottomRandom"),
                Destination::OwnersLibraryFromTop(n) => {
                    if record {
                        moves.push((
                            format!("{old_destination}/OwnersLibraryFromTop"),
                            format!("{new_destination}/Library/position/FromTop"),
                        ));
                    }
                    json!({"FromTop":n})
                }
                Destination::OwnersLibraryTopOrBottom => {
                    let chooser = match &terms.placement {
                        MovementPlacement::DestinationDefault => json!("Owners"),
                        MovementPlacement::By(players) => {
                            if record {
                                moves.push((format!("{old_terms}/placement/By"),format!("{new_destination}/Library/position/TopOrBottom/chooser/Players")));
                            }
                            json!({"Players":players})
                        }
                    };
                    json!({"TopOrBottom":{"chooser":chooser}})
                }
                _ => unreachable!(),
            };
            json!({"Library":{"position":position,"order":terms.order,"arrival":arrival}})
        }
        Destination::Hand(player) => {
            if record {
                moves.push((
                    format!("{old_destination}/Hand"),
                    format!("{new_destination}/Hand/player"),
                ));
            }
            json!({"Hand":{"player":player,"arrival":arrival}})
        }
        Destination::OwnersPlanarDeck(end) => {
            json!({"OwnersPlanarDeck":{"end":end,"arrival":arrival}})
        }
        _ => {
            let mut value = serde_json::Map::new();
            value.insert(variant.into(), arrival);
            Value::Object(value)
        }
    };
    if record {
        moves.push((format!("{old_terms}/qualifiers"), new_qualifiers.into()));
    }
    Ok(DestinationTransfer {
        destination: serde_json::from_value(value).map_err(|e| e.to_string())?,
        qualifiers: terms.qualifiers.clone(),
        moves,
    })
}

fn arrival(
    raw: &Value,
    zone: Zone,
    from: &str,
    to: &str,
    record: bool,
    top: bool,
    moves: &mut Vec<(String, String)>,
) -> Result<(Value, bool), String> {
    if !matches!(zone, Zone::Battlefield)
        && (raw["tapped"] != false || !raw["controller"].is_null())
    {
        return Err(
            "tapped arrival and entering controller require a battlefield destination".into(),
        );
    }
    let mut output = serde_json::Map::new();
    output.insert("counters".into(), raw["counters"].clone());
    if record {
        moves.push((format!("{from}/counters"), format!("{to}/counters")));
    }
    if matches!(zone, Zone::Battlefield) {
        output.insert("tapped".into(), raw["tapped"].clone());
        output.insert("controller".into(), raw["controller"].clone());
        if record {
            moves.push((format!("{from}/controller"), format!("{to}/controller")));
        }
    }
    let mut states = vec![];
    let mut hidden = false;
    for (i, state) in raw["states"]
        .as_array()
        .ok_or("arrival requires a state list")?
        .iter()
        .enumerate()
    {
        let name = state
            .as_str()
            .or_else(|| state.as_object()?.keys().next().map(String::as_str))
            .ok_or("arrival requires a state constructor")?;
        let common = matches!(
            name,
            "Changes" | "ChangesUntil" | "Perpetual" | "When" | "Optional"
        );
        let legal = match zone {
            Zone::Battlefield => {
                !matches!(name, "ExilePile" | "ExiledFor" | "ExiledForWith" | "Draft")
            }
            Zone::Exile => {
                common
                    || matches!(
                        name,
                        "FaceDown" | "ExilePile" | "ExiledFor" | "ExiledForWith"
                    )
            }
            Zone::Other => common,
        };
        if !legal {
            return Err(
                if matches!(name, "ExilePile" | "ExiledFor" | "ExiledForWith") {
                    format!("{name} requires an exile destination")
                } else if name == "Draft" {
                    "unresolved draft is not valid arrival state".into()
                } else {
                    format!("{name} requires a battlefield destination")
                },
            );
        }
        if matches!(zone, Zone::Exile) && top && name == "FaceDown" {
            hidden = true;
            continue;
        }
        let source = format!("{from}/states/{i}");
        let target = format!("{to}/states/{}", states.len());
        let mut state = state.clone();
        if matches!(name, "When" | "Optional") {
            let (nested, _) = arrival(
                &state[name]["entry"],
                zone,
                &format!("{source}/{name}/entry"),
                &format!("{target}/{name}/entry"),
                record,
                false,
                moves,
            )?;
            state[name]["entry"] = nested;
            let field = if name == "When" { "filter" } else { "decider" };
            if record {
                moves.push((
                    format!("{source}/{name}/{field}"),
                    format!("{target}/{name}/{field}"),
                ));
            }
        } else if record {
            moves.push((source, target));
        }
        states.push(state);
    }
    output.insert("states".into(), Value::Array(states));
    Ok((Value::Object(output), hidden))
}

/// A backend/source boundary view. This is not a serialized semantic payload.
/// It lets callers retain their action-specific ABI while taking all state
/// from the already typed destination.
pub(crate) struct DestinationView {
    pub destination: Destination,
    pub entry: EntrySpec,
    pub face_down: bool,
    pub terms: MovementTerms,
}
pub(crate) fn destination_view(
    destination: &MovementDestination,
    qualifiers: &[SelectionConstraint],
) -> DestinationView {
    let mut terms = MovementTerms {
        qualifiers: qualifiers.to_vec(),
        ..MovementTerms::default()
    };
    let (address, entry, hidden) = match destination {
        MovementDestination::Battlefield {
            controller,
            arrival,
        } => {
            let address = match controller {
                EntryController::Owner => Destination::OwnersBattlefield,
                EntryController::Player(player) => Destination::Battlefield(player.clone()),
            };
            (
                address,
                entry_view(serde_json::to_value(arrival).expect("typed battlefield arrival")),
                false,
            )
        }
        MovementDestination::Exile {
            visibility,
            arrival,
        } => (
            Destination::Exile,
            entry_view(serde_json::to_value(arrival).expect("typed exile arrival")),
            matches!(visibility, ExileVisibility::FaceDown),
        ),
        MovementDestination::Library {
            position,
            order,
            arrival,
        } => {
            terms.order = order.clone();
            let address = match position {
                LibraryPosition::Top => Destination::OwnersLibraryTop,
                LibraryPosition::Bottom => Destination::OwnersLibraryBottom,
                LibraryPosition::Shuffled => Destination::OwnersLibraryShuffled,
                LibraryPosition::FromTop(n) => Destination::OwnersLibraryFromTop(n.clone()),
                LibraryPosition::BottomRandom => Destination::OwnersLibraryBottomRandom,
                LibraryPosition::TopOrBottom { chooser } => {
                    terms.placement = match chooser {
                        LibraryChooser::Owners => MovementPlacement::DestinationDefault,
                        LibraryChooser::Players(players) => MovementPlacement::By(players.clone()),
                    };
                    Destination::OwnersLibraryTopOrBottom
                }
            };
            (
                address,
                entry_view(serde_json::to_value(arrival).expect("typed card arrival")),
                false,
            )
        }
        MovementDestination::Hand { player, arrival } => (
            Destination::Hand(player.clone()),
            entry_view(serde_json::to_value(arrival).expect("typed card arrival")),
            false,
        ),
        MovementDestination::OwnersPlanarDeck { end, arrival } => (
            Destination::OwnersPlanarDeck(end.clone()),
            entry_view(serde_json::to_value(arrival).expect("typed card arrival")),
            false,
        ),
        MovementDestination::OwnersHand(arrival)
        | MovementDestination::OwnersGraveyard(arrival)
        | MovementDestination::CommandZone(arrival)
        | MovementDestination::Ante(arrival) => {
            let address = match destination {
                MovementDestination::OwnersHand(_) => Destination::OwnersHand,
                MovementDestination::OwnersGraveyard(_) => Destination::OwnersGraveyard,
                MovementDestination::CommandZone(_) => Destination::CommandZone,
                _ => Destination::Ante,
            };
            (
                address,
                entry_view(serde_json::to_value(arrival).expect("typed card arrival")),
                false,
            )
        }
    };
    DestinationView {
        destination: address,
        entry,
        face_down: hidden,
        terms,
    }
}
fn entry_view(mut value: Value) -> EntrySpec {
    fn fill(value: &mut Value) {
        let fields = value.as_object_mut().expect("typed arrival record");
        fields.entry("controller").or_insert(Value::Null);
        fields.entry("tapped").or_insert(json!(false));
        for state in fields
            .get_mut("states")
            .expect("typed arrival states")
            .as_array_mut()
            .expect("typed arrival list")
        {
            for variant in ["When", "Optional"] {
                if let Some(fields) = state.get_mut(variant) {
                    fill(&mut fields["entry"]);
                }
            }
        }
    }
    fill(&mut value);
    serde_json::from_value(value).expect("zone-specific state is a subset of entry vocabulary")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roundtrip(
        destination: Destination,
        entry: EntrySpec,
        face_down: bool,
        terms: MovementTerms,
    ) {
        let typed = destination_from_parts(&destination, &entry, face_down, &terms, None).unwrap();
        let view = destination_view(&typed.destination, &typed.qualifiers);
        assert_eq!(
            serde_json::to_value((&destination, &entry, face_down, &terms)).unwrap(),
            serde_json::to_value((&view.destination, &view.entry, view.face_down, &view.terms))
                .unwrap()
        );
    }
    #[test]
    fn destination_conversion_keeps_arrival_order_and_lexical_children() {
        roundtrip(
            Destination::OwnersHand,
            EntrySpec::default(),
            false,
            MovementTerms::default(),
        );
        roundtrip(
            Destination::OwnersLibraryTopOrBottom,
            EntrySpec::default(),
            false,
            MovementTerms {
                placement: MovementPlacement::By(PlayerSet::All),
                ..MovementTerms::default()
            },
        );
        roundtrip(
            Destination::OwnersLibraryTop,
            EntrySpec::default(),
            false,
            MovementTerms {
                order: MovementOrder::Random,
                ..MovementTerms::default()
            },
        );
        roundtrip(
            Destination::Battlefield(PlayerRef::AbilityController),
            EntrySpec {
                tapped: true,
                states: vec![EntryState::When {
                    filter: ObjectFilter::Any,
                    entry: Box::new(EntrySpec {
                        states: vec![EntryState::FaceDown],
                        ..EntrySpec::default()
                    }),
                }],
                ..EntrySpec::default()
            },
            false,
            MovementTerms::default(),
        );
        roundtrip(
            Destination::Exile,
            EntrySpec {
                counters: vec![(
                    crate::counter::CounterKind::Named(crate::counter::CounterName::TimeCounter),
                    Expr::Constant(3),
                )],
                states: vec![EntryState::Changes(vec![Change::GrantKeyword(
                    Keyword::Haste,
                )])],
                ..EntrySpec::default()
            },
            true,
            MovementTerms::default(),
        );
    }
    #[test]
    fn foreign_zone_state_cannot_be_converted_or_deserialized() {
        let attacking = EntrySpec {
            states: vec![EntryState::Attacking {
                player: None,
                or_planeswalker: false,
            }],
            ..EntrySpec::default()
        };
        assert!(destination_from_parts(
            &Destination::OwnersHand,
            &attacking,
            false,
            &MovementTerms::default(),
            None
        )
        .is_err());
        let tapped = EntrySpec {
            tapped: true,
            ..EntrySpec::default()
        };
        assert!(destination_from_parts(
            &Destination::Exile,
            &tapped,
            false,
            &MovementTerms::default(),
            None
        )
        .is_err());
        assert!(destination_from_parts(
            &Destination::OwnersBattlefield,
            &EntrySpec::default(),
            true,
            &MovementTerms::default(),
            None
        )
        .is_err());
        assert!(serde_json::from_value::<CardArrival>(
            json!({"counters":[],"states":["FaceDown"]})
        )
        .is_err());
        assert!(serde_json::from_value::<BattlefieldArrival>(json!({"controller":null,"tapped":false,"counters":[],"states":[{"ExilePile":{"shuffled":true}}]})).is_err());
    }
    #[test]
    fn mappings_follow_nested_fields_instead_of_numeric_labels() {
        let entry = EntrySpec {
            states: vec![EntryState::When {
                filter: ObjectFilter::Any,
                entry: Box::new(EntrySpec {
                    states: vec![EntryState::AttachedTo(ObjectRef::Target {
                        slot: TargetSlot(0),
                        index: 0,
                    })],
                    ..EntrySpec::default()
                }),
            }],
            ..EntrySpec::default()
        };
        let transfer = destination_from_parts(
            &Destination::Battlefield(PlayerRef::Target {
                slot: TargetSlot(1),
                index: 0,
            }),
            &entry,
            false,
            &MovementTerms::default(),
            Some((
                "/old/destination",
                "/old/entry",
                "/old/terms",
                "/new/destination",
                "/new/qualifiers",
            )),
        )
        .unwrap();
        assert!(transfer.moves.contains(&(
            "/old/destination/Battlefield".into(),
            "/new/destination/Battlefield/controller/Player".into()
        )));
        assert!(transfer.moves.contains(&(
            "/old/entry/states/0/When/entry/states/0".into(),
            "/new/destination/Battlefield/arrival/states/0/When/entry/states/0".into()
        )));
    }
}

pub(crate) struct ConjureTransfer {
    pub destination: ConjureDestination,
    pub moves: Vec<(String, String)>,
}
pub(crate) fn conjure_from_parts(
    zone: &EventZone,
    spot: &Option<ConjureSpot>,
    entry: &EntrySpec,
    paths: Option<(&str, &str, &str)>,
) -> Result<ConjureTransfer, String> {
    if spot.is_some() && !matches!(zone, EventZone::Library) {
        return Err("conjure library placement requires a library destination".into());
    }
    let (old_entry, old_spot, new_destination) = paths.unwrap_or(("", "", ""));
    let record = paths.is_some();
    let mut moves = vec![];
    let name = serde_json::to_value(zone)
        .map_err(|e| e.to_string())?
        .as_str()
        .ok_or("expected a zone")?
        .to_string();
    let (kind, at) = match zone {
        EventZone::Battlefield => (Zone::Battlefield, format!("{new_destination}/Battlefield")),
        EventZone::Exile => (Zone::Exile, format!("{new_destination}/Exile/arrival")),
        EventZone::Library => (Zone::Other, format!("{new_destination}/Library/arrival")),
        _ => (Zone::Other, format!("{new_destination}/{name}")),
    };
    let (arrival, hidden) = arrival(
        &serde_json::to_value(entry).map_err(|e| e.to_string())?,
        kind,
        old_entry,
        &at,
        record,
        true,
        &mut moves,
    )?;
    let value = match zone {
        EventZone::Exile => {
            json!({"Exile":{"visibility":if hidden{"FaceDown"}else{"FaceUp"},"arrival":arrival}})
        }
        EventZone::Library => {
            let position = match spot {
                None => json!("Random"),
                Some(ConjureSpot::Top) => json!("Top"),
                Some(ConjureSpot::Shuffled) => json!("Shuffled"),
                Some(ConjureSpot::AmongTop(n)) => {
                    if record {
                        moves.push((
                            format!("{old_spot}/AmongTop"),
                            format!("{new_destination}/Library/position/AmongTop"),
                        ));
                    }
                    json!({"AmongTop":n})
                }
                Some(ConjureSpot::FromTop(n)) => {
                    if record {
                        moves.push((
                            format!("{old_spot}/FromTop"),
                            format!("{new_destination}/Library/position/FromTop"),
                        ));
                    }
                    json!({"FromTop":n})
                }
            };
            json!({"Library":{"position":position,"arrival":arrival}})
        }
        _ => {
            let mut value = serde_json::Map::new();
            value.insert(name, arrival);
            Value::Object(value)
        }
    };
    Ok(ConjureTransfer {
        destination: serde_json::from_value(value).map_err(|e| e.to_string())?,
        moves,
    })
}
pub(crate) fn conjure_view(
    destination: &ConjureDestination,
) -> (EventZone, Option<ConjureSpot>, EntrySpec) {
    let (zone, spot, raw, hidden) = match destination {
        ConjureDestination::Battlefield(entry) => (
            EventZone::Battlefield,
            None,
            serde_json::to_value(entry).expect("typed arrival"),
            false,
        ),
        ConjureDestination::Exile {
            visibility,
            arrival,
        } => (
            EventZone::Exile,
            None,
            serde_json::to_value(arrival).expect("typed arrival"),
            matches!(visibility, ExileVisibility::FaceDown),
        ),
        ConjureDestination::Library { position, arrival } => {
            let spot = match position {
                ConjurePosition::Random => None,
                ConjurePosition::Top => Some(ConjureSpot::Top),
                ConjurePosition::Shuffled => Some(ConjureSpot::Shuffled),
                ConjurePosition::AmongTop(n) => Some(ConjureSpot::AmongTop(n.clone())),
                ConjurePosition::FromTop(n) => Some(ConjureSpot::FromTop(n.clone())),
            };
            (
                EventZone::Library,
                spot,
                serde_json::to_value(arrival).expect("typed arrival"),
                false,
            )
        }
        ConjureDestination::Hand(entry)
        | ConjureDestination::Graveyard(entry)
        | ConjureDestination::Stack(entry)
        | ConjureDestination::Command(entry)
        | ConjureDestination::Ante(entry)
        | ConjureDestination::Outside(entry)
        | ConjureDestination::PlanarDeck(entry) => {
            let zone = match destination {
                ConjureDestination::Hand(_) => EventZone::Hand,
                ConjureDestination::Graveyard(_) => EventZone::Graveyard,
                ConjureDestination::Stack(_) => EventZone::Stack,
                ConjureDestination::Command(_) => EventZone::Command,
                ConjureDestination::Ante(_) => EventZone::Ante,
                ConjureDestination::Outside(_) => EventZone::Outside,
                _ => EventZone::PlanarDeck,
            };
            (
                zone,
                None,
                serde_json::to_value(entry).expect("typed arrival"),
                false,
            )
        }
    };
    let mut entry = entry_view(raw);
    if hidden {
        entry.states.insert(0, EntryState::FaceDown);
    }
    (zone, spot, entry)
}

#[cfg(test)]
mod corpus_tests {
    use super::*;
    #[test]
    fn conjure_preserves_ownership_independent_arrival_and_library_position() {
        for (zone, spot, entry) in [
            (
                EventZone::Exile,
                None,
                EntrySpec {
                    states: vec![EntryState::FaceDown],
                    ..EntrySpec::default()
                },
            ),
            (
                EventZone::Library,
                Some(ConjureSpot::AmongTop(Expr::Constant(5))),
                EntrySpec::default(),
            ),
            (
                EventZone::Battlefield,
                None,
                EntrySpec {
                    controller: Some(EntryController::Player(PlayerRef::AbilityController)),
                    tapped: true,
                    ..EntrySpec::default()
                },
            ),
        ] {
            let converted = conjure_from_parts(&zone, &spot, &entry, None).unwrap();
            let projected = conjure_view(&converted.destination);
            assert_eq!(
                serde_json::to_value((&zone, &spot, &entry)).unwrap(),
                serde_json::to_value(projected).unwrap()
            );
        }
        assert!(conjure_from_parts(
            &EventZone::Hand,
            &Some(ConjureSpot::Top),
            &EntrySpec::default(),
            None
        )
        .is_err());
    }
    #[test]
    #[ignore = "whole corpus conversion audit; set MOVEMENT_CORPUS"]
    fn all_existing_destinations_have_lossless_typed_views() {
        crate::corpus::on_large_stack(|| {
            use std::io::BufRead;
            struct Audit {
                movements: usize,
                conjures: usize,
            }
            impl Visitor for Audit {
                fn operation(&mut self, operation: &Operation) -> bool {
                    match operation {
                        Operation::Move {
                            destination,
                            qualifiers,
                            ..
                        } => {
                            let view = destination_view(destination, qualifiers);
                            let converted = destination_from_parts(
                                &view.destination,
                                &view.entry,
                                view.face_down,
                                &view.terms,
                                None,
                            )
                            .unwrap();
                            assert_eq!(
                                serde_json::to_value((destination, qualifiers)).unwrap(),
                                serde_json::to_value((
                                    &converted.destination,
                                    &converted.qualifiers
                                ))
                                .unwrap()
                            );
                            self.movements += 1;
                        }
                        Operation::MoveTopCard { destination, .. }
                        | Operation::CounterWith { destination, .. } => {
                            let view = destination_view(destination, &[]);
                            let converted = destination_from_parts(
                                &view.destination,
                                &view.entry,
                                view.face_down,
                                &view.terms,
                                None,
                            )
                            .unwrap();
                            assert_eq!(
                                serde_json::to_value(destination).unwrap(),
                                serde_json::to_value(converted.destination).unwrap()
                            );
                            self.movements += 1;
                        }
                        Operation::Conjure { destination, .. } => {
                            let (zone, spot, entry) = conjure_view(destination);
                            let converted = conjure_from_parts(&zone, &spot, &entry, None).unwrap();
                            assert_eq!(
                                serde_json::to_value(destination).unwrap(),
                                serde_json::to_value(converted.destination).unwrap()
                            );
                            self.conjures += 1;
                        }
                        _ => (),
                    };
                    true
                }
            }
            let path = std::env::var("MOVEMENT_CORPUS").unwrap();
            let file = std::fs::File::open(path).unwrap();
            let mut audit = Audit {
                movements: 0,
                conjures: 0,
            };
            let mut records = 0;
            for line in std::io::BufReader::new(file).lines() {
                let line = line.unwrap();
                let card = crate::migrate::read_json(&line)
                    .unwrap_or_else(|e| panic!("record {records}: {e}"));
                card.walk(&mut audit);
                records += 1;
            }
            assert_eq!(records, 33529);
            assert!(audit.movements > 7000);
            assert!(audit.conjures > 100);
            println!(
                "{records} records; {} movement and {} conjure destinations roundtrip",
                audit.movements, audit.conjures
            );
        });
    }
}

/// Source constructors use the same checked conversion as saved-epoch input.
/// No engine decision or descriptive-term decoding occurs in generation.
pub(crate) fn move_to(
    objects: ObjectSet,
    destination: Destination,
    entry: EntrySpec,
    face_down: bool,
    terms: MovementTerms,
) -> Result<Operation, String> {
    let converted = destination_from_parts(&destination, &entry, face_down, &terms, None)?;
    Ok(Operation::Move {
        objects,
        destination: converted.destination,
        qualifiers: converted.qualifiers,
    })
}
/// An address with no extra arrival/visibility/placement is always a valid
/// destination shape. Reference and numeric-expression checks remain scoped.
pub(crate) fn move_plain(objects: ObjectSet, destination: Destination) -> Operation {
    Operation::Move {
        objects,
        destination: destination.into(),
        qualifiers: vec![],
    }
}

pub(crate) fn counter_with(
    object: ObjectRef,
    when: Option<ObjectFilter>,
    destination: Destination,
    entry: EntrySpec,
) -> Result<Operation, String> {
    let converted =
        destination_from_parts(&destination, &entry, false, &MovementTerms::default(), None)?;
    Ok(Operation::CounterWith {
        object,
        when,
        destination: converted.destination,
    })
}
pub(crate) fn move_top_card(
    actor: PlayerSet,
    cards: ObjectFilter,
    reveal: bool,
    destination: Destination,
    entry: EntrySpec,
) -> Result<Operation, String> {
    let converted =
        destination_from_parts(&destination, &entry, false, &MovementTerms::default(), None)?;
    Ok(Operation::MoveTopCard {
        actor,
        cards,
        reveal,
        destination: converted.destination,
    })
}
pub(crate) fn conjure_to(
    actor: PlayerSet,
    card: ConjuredCard,
    count: Expr,
    zone: EventZone,
    owner: Option<PlayerRef>,
    spot: Option<ConjureSpot>,
    entry: EntrySpec,
) -> Result<Operation, String> {
    let converted = conjure_from_parts(&zone, &spot, &entry, None)?;
    Ok(Operation::Conjure {
        actor,
        card,
        count,
        owner,
        destination: converted.destination,
    })
}
/// Specialized move actions carry exile visibility as an arrival term in their
/// existing ABI. The typed destination retains it separately in semantic IR.
pub(crate) fn entry_destination_view(destination: &MovementDestination) -> DestinationView {
    let mut view = destination_view(destination, &[]);
    if view.face_down {
        view.entry.states.insert(0, EntryState::FaceDown);
        view.face_down = false;
    }
    view
}

pub(crate) fn cost_move(
    selections: Vec<CostObjectSelection>,
    destination: Destination,
    entry: EntrySpec,
    face_down: bool,
) -> Result<CostPart, String> {
    let converted = destination_from_parts(
        &destination,
        &entry,
        face_down,
        &MovementTerms::default(),
        None,
    )?;
    Ok(CostPart::Move {
        selections,
        destination: converted.destination,
    })
}
pub(crate) fn cost_move_plain(
    selections: Vec<CostObjectSelection>,
    destination: Destination,
) -> CostPart {
    CostPart::Move {
        selections,
        destination: destination.into(),
    }
}
pub(crate) fn can_move_to(
    destination: Destination,
    entry: EntrySpec,
    face_down: bool,
) -> Result<SelectionConstraint, String> {
    let converted = destination_from_parts(
        &destination,
        &entry,
        face_down,
        &MovementTerms::default(),
        None,
    )?;
    Ok(SelectionConstraint::CanMoveTo {
        destination: converted.destination,
    })
}
pub(crate) fn can_move_plain(destination: Destination) -> SelectionConstraint {
    SelectionConstraint::CanMoveTo {
        destination: destination.into(),
    }
}

pub(crate) fn battlefield_from_parts(entry: &EntrySpec) -> Result<BattlefieldArrival, String> {
    match destination_from_parts(
        &Destination::OwnersBattlefield,
        entry,
        false,
        &MovementTerms::default(),
        None,
    )?
    .destination
    {
        MovementDestination::Battlefield { arrival, .. } => Ok(arrival),
        _ => unreachable!(),
    }
}
/// A typed input viewed through the existing backend arrival ABI. This view
/// has no wire representation and performs no semantic inference.
pub(crate) trait ArrivalView {
    fn entry_view(&self) -> std::borrow::Cow<'_, EntrySpec>;
}
impl ArrivalView for EntrySpec {
    fn entry_view(&self) -> std::borrow::Cow<'_, EntrySpec> {
        std::borrow::Cow::Borrowed(self)
    }
}
impl ArrivalView for BattlefieldArrival {
    fn entry_view(&self) -> std::borrow::Cow<'_, EntrySpec> {
        std::borrow::Cow::Owned(entry_view(
            serde_json::to_value(self).expect("typed battlefield arrival"),
        ))
    }
}
impl<T: ArrivalView> ArrivalView for Box<T> {
    fn entry_view(&self) -> std::borrow::Cow<'_, EntrySpec> {
        (**self).entry_view()
    }
}

pub(crate) fn battlefield_state_from_parts(state: &EntryState) -> Result<BattlefieldState, String> {
    battlefield_from_parts(&EntrySpec {
        states: vec![state.clone()],
        ..EntrySpec::default()
    })?
    .states
    .into_iter()
    .next()
    .ok_or_else(|| "missing battlefield state".into())
}
pub(crate) fn battlefield_state_view(state: &BattlefieldState) -> EntryState {
    serde_json::from_value(serde_json::to_value(state).expect("typed battlefield state"))
        .expect("battlefield state has an ABI view")
}
