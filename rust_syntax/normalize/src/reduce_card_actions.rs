/// One-card actions read as an ordinary operation on the terms the row spells out.
fn reduce_card_actions(actor: &PlayerSet, form: &str, operands: &[Operand]) -> Option<Operation> {
    use Operand::{Comparison as Compare, Filter, List, Number, Object, Objects, Term};
    let you = actor_of(actor);
    Some(match (form, operands) {
        ("CastCardInHandWithoutPayingAsAFacedownCreatureSpell", [Object(card), pt]) => cast_with(
            actor,
            CardPlayAction::CastSpell,
            ObjectSet::One(card.clone()),
            Count::UpTo(Expr::Constant(1)),
            PlayTerms {
                cost: PlayCost::Free,
                face_down: Some(crate::source_values::face_down(pt, None, false).ok()?),
                ..play_terms()
            },
            None,
        ),
        (
            "CastSpellFromExileWithoutPayingAndFlagSpellsCastWithEffect",
            [Filter(spells), Object(card), Filter(flagged), List(effects)],
        ) => cast_with(
            actor,
            CardPlayAction::CastSpell,
            ObjectSet::Matching {
                objects: Box::new(ObjectSet::One(card.clone())),
                filter: spells.clone(),
            },
            Count::UpTo(Expr::Constant(1)),
            PlayTerms {
                cost: PlayCost::Free,
                effects: std::iter::once(Filter(flagged.clone()))
                    .chain(effects.iter().cloned())
                    .collect(),
                ..play_terms()
            },
            None,
        ),
        ("CopyEachAbilityAndMayChooseNewTargets", [abilities]) => Operation::CopyEach {
            actor: actor.clone(),
            objects: ability_filter(abilities)?,
            targets: NewTargets::May,
        },
        (
            "ForEachValueInRangeConjureDuplicateOfARandomCardOfTypeOntoBattlefield",
            [Number(from), Number(to), cards @ Filter(_), List(entry)],
        ) => Operation::EachNumber {
            from: from.clone(),
            to: to.clone(),
            action: Box::new(Operation::Conjure {
                actor: actor.clone(),
                card: ConjuredCard::Random(Box::new(cards.clone())),
                count: Expr::Constant(1),
                zone: EventZone::Battlefield,
                owner: None,
                spot: None,
                entry: crate::movement::entry_from_source(entry),
            }),
        },
        (
            "PlayALandFromTopOfLibraryOrCastASpellFromTopOfLibraryWithTrigger",
            [Objects(lands), Filter(spells), body],
        ) => Operation::WithReflexive {
            action: Box::new(cast_with(
                actor,
                CardPlayAction::Either,
                ObjectSet::Matching {
                    objects: Box::new(ObjectSet::LibraryTops(actor.clone())),
                    filter: ObjectFilter::Either(vec![
                        ObjectFilter::All(vec![
                            ObjectFilter::CardType(crate::mtg_types::CardType::Land),
                            objects_filter(lands),
                        ]),
                        ObjectFilter::All(vec![
                            ObjectFilter::Not(Box::new(ObjectFilter::CardType(
                                crate::mtg_types::CardType::Land,
                            ))),
                            spells.clone(),
                        ]),
                    ]),
                },
                Count::Exactly(Expr::Constant(1)),
                play_terms(),
                None,
            )),
            when: ReflexiveWhen::YouDo,
            body: Box::new(body.clone()),
        },
        (
            "PutAnAbilityStickerWithTicketCostOnPermanentWithoutPaying",
            [Compare(comparison, value), Object(object)],
        ) => Operation::PutStickers {
            actor: actor.clone(),
            kind: Some(StickerKind::Ability),
            objects: ObjectSet::One(object.clone()),
            amount: None,
            any_number: false,
            ticket_cost: Some((comparison.clone(), value.clone())),
        },
        ("PutExiledCardOnStackAsCopyOfSpell", [Object(card), Object(spell), effects]) => {
            // The original spell's controller, whoever controls this effect.
            Operation::CopyOnStack {
                actor: PlayerSet::One(PlayerRef::ControllerOf(Box::new(spell.clone()))),
                object: spell.clone(),
                times: Expr::Constant(1),
                targets: NewTargets::Same,
                exceptions: crate::copying::from_source(std::slice::from_ref(effects)).ok()?,
                card: Some(card.clone()),
            }
        }
        ("RemoveCounters", [Term { form, operands, .. }]) => {
            let [Objects(objects)] = operands.as_slice() else {
                return None;
            };
            let selection = match form.as_str() {
                "AllCountersFromAnyNumberOfPermanents" => Some(SelectionChoice {
                    player: you.clone(),
                    count: Count::AnyNumber,
                }),
                "AllCountersFromEachPermanent" => None,
                _ => return None,
            };
            Operation::RemoveCounters {
                removals: vec![CounterRemoval {
                    player: you,
                    recipients: Recipients::Objects(objects.clone()),
                    kind: None,
                    amount: CounterAmount::All,
                    selection,
                }],
            }
        }
        (
            "RestartTheGameLeavingEachExiledCardInExileThenPutThoseCardsOntoTheBattlefield",
            [Objects(cards), List(entry)],
        ) => Operation::RestartGame {
            keep: cards.clone(),
            then: Box::new(Operation::DraftChangeZone {
                actor: actor.clone(),
                cards: cards.clone(),
                count: None,
                random: false,
                group: None,
                destination: Destination::OwnersBattlefield,
                entry: entry.clone(),
                face_down: None,
            }),
        },
        _ => return None,
    })
}

/// Abilities on the stack a row names, with "other than this one".
fn ability_filter(term: &Operand) -> Option<ObjectFilter> {
    let Operand::Term { form, operands, .. } = term else {
        return stack_filter(term);
    };
    match (form.as_str(), operands.as_slice()) {
        ("And", items) => Some(ObjectFilter::All(
            items.iter().map(ability_filter).collect::<Option<_>>()?,
        )),
        ("Other", [Operand::Vocabulary { form, .. }]) if form == "ThisAbility" => Some(
            ObjectFilter::Not(Box::new(ObjectFilter::OnStack(StackTrait::ThisAbility))),
        ),
        _ => stack_filter(term),
    }
}

/// Costs a row spells as one form, with the parts a payment reads one by one.
pub fn reduce_card_cost(
    name: &str,
    operands: &[Operand],
    payer: &PlayerRef,
) -> Option<Vec<CostPart>> {
    use Operand::{Filter, List, Players};
    let moved = |objects, choose, destination| CostPart::Move {
        selections: vec![CostObjectSelection { objects, choose }],
        destination,
        entry: EntrySpec::default(),
        face_down: false,
    };
    Some(match (name.strip_prefix("Cost::")?, operands) {
        ("ExileACardInGraveyardsAndPayItsManaCost", [Filter(cards)]) => vec![
            moved(
                ObjectSet::Zone {
                    scope: ZoneScope::Players {
                        zone: EventZone::Graveyard,
                        players: PlayerSet::All,
                    },
                    kind: ObjectKind::Card,
                    filter: cards.clone(),
                },
                Some(Count::Exactly(Expr::Constant(1))),
                Destination::Exile,
            ),
            CostPart::ManaCostOf(earlier_object(EarlierObjects::Exiled)),
        ],
        ("PutTopCardOfEachPlayersLibraryInGraveyard", [Players(players)]) => vec![moved(
            ObjectSet::LibraryTops(players.clone()),
            None,
            Destination::OwnersGraveyard,
        )],
        ("RevealHandAndPutEachCardOnBattlefield", [Filter(cards), List(flags)]) => {
            let plain = flags
                .iter()
                .all(|flag| matches!(flag, Operand::Term { form, .. } if form == "EntersNormally"));
            if !plain {
                return None;
            }
            let hand = |filter| zone_set(EventZone::Hand, PlayerSet::One(payer.clone()), filter);
            vec![
                CostPart::Reveal {
                    count: Count::Exactly(Expr::CountObjects(Box::new(hand(ObjectFilter::Any)))),
                    filter: ObjectFilter::Any,
                    random: false,
                },
                moved(hand(cards.clone()), None, Destination::OwnersBattlefield),
            ]
        }
        _ => return None,
    })
}

/// A power or toughness change a `ModX` term spells, given X.
fn mod_x(term: &Operand, x: &Expr) -> Option<Expr> {
    match term {
        Operand::Term { form, operands, .. } => match (form.as_str(), operands.as_slice()) {
            ("PlusX", []) => Some(x.clone()),
            ("MinusX", []) => Some(Expr::multiply(Expr::Constant(-1), x.clone())),
            ("Zero", []) => Some(Expr::Constant(0)),
            ("Integer", [Operand::Integer(n)]) => Some(Expr::Constant(*n)),
            _ => None,
        },
        _ => None,
    }
}

/// A base power and toughness a `PT` term spells.
fn set_pt(term: &Operand) -> Option<Change> {
    use Operand::{Integer, Number, Term};
    let Term { form, operands, .. } = term else {
        return None;
    };
    let (power, toughness) = match (form.as_str(), operands.as_slice()) {
        ("PT", [Integer(power), Integer(toughness)]) => {
            (Expr::Constant(*power), Expr::Constant(*toughness))
        }
        ("PTX", [power, toughness, Number(x)]) => {
            let component = |term: &Operand| match term {
                Term { form, operands, .. } => match (form.as_str(), operands.as_slice()) {
                    ("X", []) => Some(x.clone()),
                    ("Integer", [Integer(n)]) => Some(Expr::Constant(*n)),
                    _ => None,
                },
                _ => None,
            };
            (component(power)?, component(toughness)?)
        }
        ("ManualPT", [Number(power), Number(toughness)]) => (power.clone(), toughness.clone()),
        _ => return None,
    };
    Some(Change::SetBasePowerToughness { power, toughness })
}

/// The cards a defined token takes abilities from, and which of their abilities.
pub fn reduce_token_abilities(operands: &[Operand]) -> Option<(ObjectSet, Box<Operand>)> {
    let [Operand::Term { form, operands, .. }] = operands else {
        return None;
    };
    match (form.as_str(), operands.as_slice()) {
        ("AddAbilityFromEachExiledHasable", [Operand::Objects(objects), kinds]) => {
            Some((objects.clone(), Box::new(kinds.clone())))
        }
        _ => None,
    }
}
