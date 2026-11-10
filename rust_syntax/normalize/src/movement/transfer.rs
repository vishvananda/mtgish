//! Elaboration of draft zone-change terms into selections and one typed move.
use super::*;

pub(super) fn canonical_move(operation: &Operation) -> Operation {
    let Operation::DraftChangeZone {
        actor,
        cards,
        count,
        random,
        group,
        destination,
        entry,
        face_down,
    } = operation
    else {
        unreachable!("zone-change elaboration")
    };
    let mut destination = destination.clone();
    let mut arrival = EntrySpec::default();
    let mut hidden = false;
    for flag in entry {
        entry_flag(flag, &mut destination, &mut arrival, &mut hidden);
    }
    // A canonical Move's destination already carries its controller. Other
    // creating actions retain an explicit override in their typed entry.
    arrival.controller = None;
    if let Some(shape) = face_down {
        arrival.states.push(EntryState::FaceDownAs(shape.clone()));
    }
    let mut terms = MovementTerms::default();
    if matches!(destination, Destination::OwnersLibraryTopOrBottom) {
        terms.placement = MovementPlacement::By(actor.clone());
    }
    let mut extraction = Extract {
        root: false,
        steps: vec![],
    };
    let mut objects = cards.clone();
    objects.walk_mut(&mut extraction);
    let constraints = group
        .iter()
        .map(|g| group_constraint(g))
        .collect::<Vec<_>>();
    if let Some(count) = count {
        let mut step = SelectionStep {
            binding: SelectionId(0),
            chooser: if *random {
                SelectionChooser::Chance
            } else {
                SelectionChooser::Players(actor.clone())
            },
            objects,
            count: count.clone(),
            constraints,
        };
        // Expressions and constraints can themselves depend on an embedded
        // choice. Prepare those inputs before introducing the outer binding.
        step.count.walk_mut(&mut extraction);
        step.constraints
            .iter_mut()
            .for_each(|c| c.walk_mut(&mut extraction));
        step.binding =
            SelectionId(u32::try_from(extraction.steps.len()).expect("selection id overflow"));
        objects = ObjectSet::Prepared(step.binding.clone());
        extraction.steps.push(step);
    } else {
        terms.qualifiers = constraints;
        if *random {
            terms.order = MovementOrder::Random;
        }
    }
    let Ok(action) = move_to(objects, destination, arrival, hidden, terms) else {
        // Preserve an unreduced Source form on an illegal destination payload.
        // Validation rejects it, rather than manufacturing complete semantics.
        return operation.clone();
    };
    if extraction.steps.is_empty() {
        action
    } else {
        Operation::WithSelections {
            steps: extraction.steps,
            action: Box::new(action),
        }
    }
}

pub(super) fn entry_from_source(flags: &[Operand]) -> EntrySpec {
    let mut entry = EntrySpec::default();
    let mut destination = Destination::OwnersBattlefield;
    let mut hidden = false;
    // Countered into exile with counters and a granted ability, as on Delay.
    if let [Operand::Number(amount), kind, Operand::List(effects)] = flags {
        if let Some(kind) = crate::semantics::counter_kind(kind) {
            entry.counters.push((kind, amount.clone()));
            for effect in effects {
                entry_flag(effect, &mut destination, &mut entry, &mut hidden);
            }
            return entry;
        }
    }
    for flag in flags {
        entry_flag(flag, &mut destination, &mut entry, &mut hidden);
    }
    if hidden
        && !entry
            .states
            .iter()
            .any(|state| matches!(state, EntryState::FaceDown))
    {
        entry.states.push(EntryState::FaceDown);
    }
    entry
}

fn descriptor(operand: &Operand) -> Option<(&str, &str, &[Operand])> {
    match operand {
        Operand::Vocabulary { family, form } => Some((family, form, &[])),
        Operand::Term {
            family,
            form,
            operands,
        } => Some((family, form, operands)),
        Operand::Form(named) => Some((&named.source_type, &named.form, &named.operands)),
        _ => None,
    }
}

fn entry_flag(
    flag: &Operand,
    destination: &mut Destination,
    entry: &mut EntrySpec,
    hidden: &mut bool,
) {
    if let Operand::List(flags) = flag {
        for flag in flags {
            entry_flag(flag, destination, entry, hidden);
        }
        return;
    }
    let Some((family, form, args)) = descriptor(flag) else {
        entry.states.push(EntryState::Draft {
            name: "MovementEntry::Unclassified".into(),
            operands: vec![flag.clone()],
        });
        return;
    };
    if family == "CounterType" {
        if let Some(kind) = crate::semantics::counter_kind(flag) {
            entry.counters.push((kind, Expr::Constant(1)));
            return;
        }
    }
    match (family, form, args) {
        ("EnterFlag", "EntersNormally", []) => {}
        ("EnterFlag", "EntersTapped", []) => entry.tapped = true,
        ("EnterFlag", "EntersUnderPlayersControl", [Operand::Player(player)]) => {
            *destination = Destination::Battlefield(player.clone());
            entry.controller = Some(EntryController::Player(player.clone()));
        }
        ("EnterFlag", "EntersUnderAPlayersControl", [Operand::Players(players)]) => {
            *destination = Destination::Battlefield(PlayerRef::OneOf(Box::new(players.clone())));
            entry.controller = Some(EntryController::Player(PlayerRef::OneOf(Box::new(
                players.clone(),
            ))));
        }
        ("EnterFlag", "EntersUnderOwnersControl", []) => {
            *destination = Destination::OwnersBattlefield;
            entry.controller = Some(EntryController::Owner);
        }
        ("EnterFlag", "EntersAttachedToPermanent", [Operand::Object(host)]) => {
            entry.states.push(EntryState::AttachedTo(host.clone()))
        }
        ("EnterFlag", "EntersAttachedToAPermanent", [Operand::Objects(objects)]) => entry
            .states
            .push(EntryState::AttachedToChosen(ObjectFilter::InSet(Box::new(
                objects.clone(),
            )))),
        ("EnterFlag", "EntersAttachedToPlayer", [Operand::Player(player)]) => entry
            .states
            .push(EntryState::AttachedToPlayer(player.clone())),
        ("EnterFlag", "EntersWithACounter", [kind]) => match crate::semantics::counter_kind(kind) {
            Some(kind) => entry.counters.push((kind, Expr::Constant(1))),
            None => entry.states.push(crate::semantics::reduce_entry_flag(
                format!("{family}::{form}"),
                args.to_vec(),
            )),
        },
        ("EnterFlag", "EntersWithNumberCounters", [Operand::Number(amount), kind]) => {
            match crate::semantics::counter_kind(kind) {
                Some(kind) => entry.counters.push((kind, amount.clone())),
                None => entry.states.push(crate::semantics::reduce_entry_flag(
                    format!("{family}::{form}"),
                    args.to_vec(),
                )),
            }
        }
        ("EnterFlag", "EntersTransformed" | "EntersConverted", []) => {
            entry.states.push(EntryState::Transformed)
        }
        ("EnterFlag", "EntersFaceDown", []) => entry.states.push(EntryState::FaceDown),
        ("EntryState", "FaceDown", []) => entry.states.push(EntryState::FaceDown),
        ("ExiledCardEffect", "AddAbility", [Operand::Ability(ability)]) => {
            entry
                .states
                .push(EntryState::Changes(vec![Change::GrantAbility(
                    ability.clone(),
                )]));
        }
        ("EnterFlag", "EntersFlipped", []) => entry.states.push(EntryState::Flipped),
        ("EnterFlag", "EntersAttacking", []) => entry.states.push(EntryState::Attacking {
            player: None,
            or_planeswalker: false,
        }),
        ("EnterFlag", "EntersAttackingPlayer", [Operand::Player(player)]) => {
            entry.states.push(EntryState::Attacking {
                player: Some(player.clone()),
                or_planeswalker: false,
            })
        }
        (
            "EnterFlag",
            "EntersAttackingPlayerOrPlaneswalkerControlledBy",
            [Operand::Player(player)],
        ) => entry.states.push(EntryState::Attacking {
            player: Some(player.clone()),
            or_planeswalker: true,
        }),
        ("EnterFlag", "EntersBlockingAttacker", [Operand::Object(host)]) => {
            entry.states.push(EntryState::Blocking(host.clone()))
        }
        ("ExileFlag", "ExilesIntoShuffledFaceDownPile", []) => {
            *hidden = true;
            entry.states.push(EntryState::ExilePile { shuffled: true });
        }
        ("ExileFlag", "FaceDownPile" | "ExilesIntoFaceDownPile", []) => {
            *hidden = true;
            entry.states.push(EntryState::ExilePile { shuffled: false });
        }
        ("ExileFlag", "FaceUpPile" | "ExilesIntoFaceUpPile", []) => {
            entry.states.push(EntryState::ExilePile { shuffled: false })
        }
        ("ExileFlag", "ExilesFaceDown", []) => *hidden = true,
        ("ExileFlag", "ExilesFaceUp", []) => *hidden = false,
        _ => entry.states.push(crate::semantics::reduce_entry_flag(
            format!("{family}::{form}"),
            args.to_vec(),
        )),
    }
}

fn group_constraint(group: &Operand) -> SelectionConstraint {
    let Some((family, form, args)) = descriptor(group) else {
        return SelectionConstraint::Primitive {
            name: "MovementGroup::Unclassified".into(),
            operands: vec![group.clone()],
        };
    };
    if family == "GroupFilter" {
        use DistinctProperty as D;
        use SelectionConstraint as C;
        let known = match (form, args) {
            ("DifferentNames", []) => Some(C::DifferentNames),
            ("DifferentManaValues", []) => Some(C::DifferentProperty(Property::ManaValue)),
            ("DifferentPowers", []) => Some(C::DifferentProperty(Property::Power)),
            ("DifferentControllers" | "ControlledByDifferentPlayers", []) => {
                Some(C::DifferentControllers)
            }
            ("TotalPowerIs" | "TotalManaValueIs", [Operand::Comparison(comparison, amount)]) => {
                Some(C::TotalProperty {
                    property: if form == "TotalPowerIs" {
                        Property::Power
                    } else {
                        Property::ManaValue
                    },
                    comparison: comparison.clone(),
                    amount: amount.clone(),
                })
            }
            ("ShareACardType", []) => Some(C::CommonProperty(D::CardTypes)),
            ("ShareAllCardTypes", []) => Some(C::AllSameProperty(D::CardTypes)),
            ("ShareAColor", []) => Some(C::CommonProperty(D::Colors)),
            ("HasAColorNotInCommon", []) => Some(C::NoCommonProperty(D::Colors)),
            ("ShareACreatureType" | "ShareACreatureTypeOfChoice", []) => {
                Some(C::CommonProperty(D::CreatureTypes))
            }
            ("SharesNoCreatureTypes", []) => Some(C::NoCommonProperty(D::CreatureTypes)),
            ("ShareALandType", []) => Some(C::CommonProperty(D::LandTypes)),
            ("ShareAGraveyard", []) => Some(C::CommonProperty(D::Owners)),
            ("SameNames", []) => Some(C::CommonProperty(D::Names)),
            ("SameToughness", []) => Some(C::CommonProperty(D::Toughness)),
            ("ControlledByTheSamePlayer", []) => Some(C::CommonProperty(D::Controllers)),
            ("SharesANameWithEachPermanent", [Operand::Objects(objects)]) => {
                Some(C::NamedAfterOneEach(objects.clone()))
            }
            ("ANumberOfDifferentCardTypes", [Operand::Comparison(comparison, amount)]) => {
                Some(C::DistinctCount {
                    property: D::CardTypes,
                    comparison: comparison.clone(),
                    amount: amount.clone(),
                })
            }
            ("OnePerCardType", []) => Some(C::OneForEachCardType(CardTypeDomain::All)),
            ("OnePerPermanentType", []) => Some(C::OneForEachCardType(CardTypeDomain::Permanent)),
            ("OneAndAllSameName", []) => Some(C::AllWithChosenName),
            ("OnePerCardTypeAmong", [Operand::Objects(objects)]) => Some(C::OneForEachCardType(
                CardTypeDomain::AmongObjects(objects.clone()),
            )),
            ("OnePerCardTypeAmong", [Operand::Filter(ObjectFilter::All(parts))]) => {
                let casts = parts.iter().find_map(|p| match p {
                    ObjectFilter::Cast {
                        by: Some(by),
                        from: None,
                    } => Some(by.clone()),
                    _ => None,
                });
                let filters = parts
                    .iter()
                    .filter(|p| !matches!(p, ObjectFilter::Cast { .. }))
                    .cloned()
                    .collect();
                casts.map(|by| {
                    C::OneForEachCardType(CardTypeDomain::AmongCasts {
                        by,
                        filter: ObjectFilter::All(filters),
                        window: HistoryWindow::ThisTurn,
                    })
                })
            }
            _ => None,
        };
        if let Some(known) = known {
            return known;
        }
    }
    SelectionConstraint::Primitive {
        name: format!("{family}::{form}"),
        operands: args.to_vec(),
    }
}
