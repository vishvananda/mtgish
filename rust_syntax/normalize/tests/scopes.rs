use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/scopes.ronl")
        .lines()
        .map(|l| ron::from_str(l).unwrap())
}
fn source(name: &str) -> source::OracleCard {
    sources()
        .find(|c| {
            corpus::Identity::of(c)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}
fn card(name: &str) -> Card {
    let c = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&c).unwrap();
    c
}
fn custom(rule: &str) -> Result<Card, Vec<lower::Diagnostic>> {
    let mut c = source("Unearth");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn inner(a: &Ability) -> &Ability {
    match a {
        Ability::Scoped { ability, .. } => inner(ability),
        a => a,
    }
}
fn cycling(c: &Card) -> &Ability {
    c.abilities
        .iter()
        .find(|a| {
            matches!(
                inner(a),
                Ability::Activated {
                    origin: ActivationOrigin::Cycling { .. },
                    ..
                }
            )
        })
        .unwrap()
}
fn cycle_trigger(c: &Card) -> &Ability {
    c.abilities
        .iter()
        .find(|a| {
            matches!(
                inner(a),
                Ability::Triggered {
                    event: EventSpec::Discard {
                        kind: DiscardKind::Cycling,
                        ..
                    },
                    ..
                }
            )
        })
        .unwrap()
}

#[test]
fn real_scoped_families_roundtrip_and_nested_mechanics_stay_incomplete() {
    let incomplete: [&str; 0] = [];
    let mut complete = 0;
    for source in sources() {
        let name = corpus::Identity::of(&source).unwrap().names.remove(0);
        if incomplete.contains(&name.as_str()) {
            assert!(lower::card(&source).is_err(), "{name}");
            continue;
        }
        let c = lower::card(&source).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        let expected = serde_json::to_value(&c).unwrap();
        let bytes = bincode::encode_to_vec(&c, bincode::config::standard()).unwrap();
        let (binary, used): (Card, usize) =
            bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
        assert_eq!(used, bytes.len());
        for loaded in [
            binary,
            ron::from_str(&ron::ser::to_string(&c).unwrap()).unwrap(),
            serde_json::from_value(expected.clone()).unwrap(),
        ] {
            validate::card(&loaded).unwrap();
            assert_eq!(serde_json::to_value(loaded).unwrap(), expected, "{name}");
        }
        complete += 1;
    }
    assert_eq!(complete, 92);
}

#[test]
fn cycling_retains_its_identity_exact_source_cost_and_hand_activation_scope() {
    let mut c = card("Lonely Sandbar");
    let a = cycling(&c);
    assert!(
        matches!(a, Ability::Scoped { zones, condition: None, .. } if zones == &[EventZone::Hand])
    );
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = inner(a)
    else {
        panic!()
    };
    assert!(matches!(
        cost.parts.as_slice(),
        [
            CostPart::Mana(_),
            CostPart::Discard {
                count: Expr::Constant(1),
                filter: ObjectFilter::SameObject(ObjectRef::Source),
                random: false
            }
        ]
    ));
    assert_eq!(
        semantics::activation_kind(cost, announcement, body),
        semantics::ActivationKind::Stack
    );
    let c2 = custom("Cycling(PayLife(Integer(2)))").unwrap();
    assert!(
        matches!(inner(cycling(&c2)), Ability::Activated { cost: Cost { parts }, .. } if matches!(parts.as_slice(), [CostPart::Life(Expr::Constant(2)), CostPart::Discard { .. }]))
    );
    let index = c
        .abilities
        .iter()
        .position(|a| {
            matches!(
                inner(a),
                Ability::Activated {
                    origin: ActivationOrigin::Cycling { .. },
                    ..
                }
            )
        })
        .unwrap();
    c.abilities[index] = inner(&c.abilities[index]).clone();
    assert!(validate::card(&c).unwrap_err().contains("cycling scope"));
    let c =
        custom("EachPermanentLayerEffect(HasAbility(AnyCycling), [AddAbility([Flying])])").unwrap();
    assert!(matches!(
        &c.abilities[0],
        Ability::Static {
            affected: EffectSubjects::Objects(ObjectSet::Battlefield(
                ObjectFilter::HasActivatedAbility(ActivationFilter::Cycling)
            )),
            ..
        }
    ));
}

#[test]
fn graveyard_and_command_scopes_do_not_replace_intervening_conditions() {
    let c = card("Reassembling Skeleton");
    assert!(
        matches!(&c.abilities[0], Ability::Scoped { zones, ability, .. } if zones == &[EventZone::Graveyard] && matches!(ability.as_ref(), Ability::Activated { body, .. } if matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::Move { destination: Destination::Battlefield(PlayerRef::AbilityController), entry: EntrySpec { tapped: true, .. }, .. }, .. }])))
    );
    let c = card("Anger");
    assert!(
        matches!(&c.abilities[1], Ability::Scoped { zones, condition: Some(Condition::Controls { .. }), ability } if zones == &[EventZone::Graveyard] && matches!(ability.as_ref(), Ability::Static { changes, .. } if matches!(changes.as_slice(), [Change::GrantKeyword(Keyword::Haste)])))
    );
    let c = card("Genesis");
    assert!(
        matches!(&c.abilities[0], Ability::Scoped { condition: None, ability, .. } if matches!(ability.as_ref(), Ability::Triggered { intervening_if: Some(Condition::ObjectMatches { .. }), .. }))
    );
    let c = card("Arahbo, Roar of the World");
    assert!(
        matches!(&c.abilities[0], Ability::Scoped { zones, ability, .. } if zones == &[EventZone::Command, EventZone::Battlefield] && matches!(ability.as_ref(), Ability::Triggered { intervening_if: Some(_), .. }))
    );
    let c = custom("FromAnyZone(TriggerA(WhenAPlayerCyclesACard(AnyPlayer, AnyCard), ActionList([DrawACard])))").unwrap();
    assert!(
        matches!(&c.abilities[0], Ability::Scoped { zones, .. } if zones.contains(&EventZone::Command) && zones.contains(&EventZone::Ante) && zones.len() == 8)
    );
}

#[test]
fn cycling_events_follow_the_discarded_card_and_or_discard_triggers_do_not_duplicate() {
    let c = card("Resounding Thunder");
    let Ability::Triggered { event, body, .. } = inner(cycle_trigger(&c)) else {
        panic!()
    };
    assert!(matches!(
        event,
        EventSpec::Discard {
            kind: DiscardKind::Cycling,
            source_only: true,
            grouping: DiscardGrouping::Card,
            ..
        }
    ));
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::DealDamage {
                source: ObjectRef::Discarded,
                amount: Expr::Constant(6),
                ..
            },
            ..
        }]
    ));
    let c = card("Archfiend of Ifnir");
    let Ability::Triggered { event, .. } = &c.abilities[1] else {
        panic!()
    };
    assert!(matches!(
        event,
        EventSpec::Discard {
            kind: DiscardKind::Any,
            grouping: DiscardGrouping::Card,
            ..
        }
    ));
    let c = custom("TriggerA(WhenAPlayerDiscardsAnyNumberOfCards(AnyPlayer, AnyCard), ActionList([PlayerAction(Trigger_ThatPlayer, DrawACard)]))").unwrap();
    let Ability::Triggered { event, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        event,
        EventSpec::Discard {
            grouping: DiscardGrouping::Player,
            ..
        }
    ));
    assert!(!semantics::event_bindings(event).discarded);
}

#[test]
fn cycling_x_cast_entry_x_and_stack_spell_x_keep_their_own_provenance() {
    let c = card("Shark Typhoon");
    let Ability::Triggered { body, .. } = inner(cycle_trigger(&c)) else {
        panic!()
    };
    assert!(
        matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::CreateTokens { tokens, .. }, .. }] if matches!(tokens[0].definition().unwrap().power_toughness, Some((Expr::EventActivationX, Expr::EventActivationX))))
    );
    let c = card("Rampaging War Mammoth");
    let Ability::Triggered { announcement, .. } = inner(cycle_trigger(&c)) else {
        panic!()
    };
    assert!(matches!(
        announcement.targets[0].count,
        Count::UpTo(Expr::EventActivationX)
    ));
    let c = card("Gadwick, the Wizened");
    assert!(
        matches!(&c.abilities[0], Ability::Triggered { body, .. } if matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::Draw { amount: Expr::CastX(ObjectRef::EventSubject), .. }, .. }]))
    );
    let c = card("Hydroid Krasis");
    let Ability::Triggered { body, .. } = inner(&c.abilities[0]) else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0], Instruction::Effect { operation: Operation::GainLife { amount: Expr::Divide { numerator, .. }, .. }, .. } if matches!(numerator.as_ref(), Expr::ObjectProperty { object: ObjectRef::EventSubject, property: Property::SpellX, .. }))
    );
    let c = card("Banefire");
    assert!(
        matches!(&c.abilities[0], Ability::Scoped { condition: Some(Condition::Compare { left: Expr::ObjectProperty { object: ObjectRef::Source, property: Property::SpellX, .. }, .. }), ability, .. } if matches!(ability.as_ref(), Ability::Static { changes, .. } if matches!(changes.as_slice(), [Change::Rule(RuleChange::Prohibit(ObjectAction::Counter))])))
    );
}

#[test]
fn source_scopes_validate_intersections_and_start_fresh_reference_scopes() {
    let c = custom("FromStack(ThisSpellEffect([CantBeCountered]))").unwrap();
    assert!(
        matches!(&c.abilities[0], Ability::Scoped { zones, ability, .. } if zones == &[EventZone::Stack] && matches!(ability.as_ref(), Ability::Static { .. }))
    );
    assert!(custom("FromGraveyard(FromStack(ThisSpellEffect([CantBeCountered])))").is_err());
    let mut c = custom("Cycling(PayMana([ManaCostGeneric(2)]))").unwrap();
    let Ability::Scoped { condition, .. } = &mut c.abilities[0] else {
        panic!()
    };
    *condition = Some(Condition::Compare {
        left: Expr::EventActivationX,
        comparison: Comparison::Equal,
        right: Expr::Constant(0),
    });
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("event activation X"));
    let mut c =
        custom("FromGraveyard(Activated(PayMana([ManaCostGeneric(1)]), ActionList([DrawACard])))")
            .unwrap();
    let Ability::Scoped { zones, .. } = &mut c.abilities[0] else {
        panic!()
    };
    zones.push(EventZone::Graveyard);
    assert!(validate::card(&c).unwrap_err().contains("repeated zones"));
    let c =
        custom("TriggerA(WhenAPlayerDiscardsACard(AnyPlayer, AnyCard), ActionList([DrawACard]))")
            .unwrap();
    let Ability::Triggered { body, .. } = c.abilities[0].clone() else {
        panic!()
    };
    let independent = Ability::Spell {
        announcement: Announcement::empty(),
        body: Program {
            instructions: vec![Instruction::Effect {
                result: ResultId(0),
                operation: Operation::DealDamage {
                    source: ObjectRef::Discarded,
                    recipients: Recipients::Players(PlayerSet::All),
                    amount: Expr::Constant(1),
                    preventable: true,
                },
            }],
        },
    };
    assert!(validate::validate_ability(&independent)
        .unwrap_err()
        .contains("discarded card"));
    assert!(!body.instructions.is_empty());
}

#[test]
fn legacy_damage_roles_resolve_from_shared_roles_and_verified_template_shapes() {
    let c = custom("TriggerA(WhenAPermanentDealsDamageToAPermanent(IsCreatureType(Goblin), IsCardtype(Creature)), ActionList([PermanentDealsDamage(Trigger_ThatCreature, Trigger_ThatMuch, Player(ControllerOfPermanent(Trigger_ThatPermanent)))]))").unwrap();
    validate::card(&c).unwrap();
    assert!(
        matches!(&c.abilities[0], Ability::Triggered { body, .. } if matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::DealDamage { source: ObjectRef::Damage(DamageSubject::Source), recipients: Recipients::Players(PlayerSet::One(PlayerRef::ControllerOf(object))), .. }, .. }] if matches!(object.as_ref(), ObjectRef::Damage(DamageSubject::Recipient))))
    );
    assert!(custom("TriggerA(WhenAPermanentDealsDamageToAPermanent(AnyPermanent, AnyPermanent), ActionList([DestroyPermanent(Trigger_ThatPermanent)]))").is_err());
}

fn lowered(rule: &str) -> String {
    format!("{:?}", custom(rule).unwrap_or_else(|e| panic!("{e:?}")))
}

#[test]
fn a_reference_cannot_pass_as_a_vocabulary_word() {
    assert!(custom("SpellActions(ActionList([CounterAbility(Ref_TargetAbility)]))").is_err());
    let bound = lowered(
        "SpellActions(Targeted([TargetAbility(AnyAbility)], ActionList([CounterAbility(Ref_TargetAbility)])))",
    );
    assert!(bound.contains("Object(Target { slot: Slot(0)"), "{bound}");
    assert!(!bound.contains("\"Ref_TargetAbility\""), "{bound}");
}

#[test]
fn references_read_the_slot_of_their_arity() {
    let body = lowered(
        "SpellActions(Targeted([UptoNumberTargetPermanents(Integer(2), AnyPermanent), TargetPermanent(AnyPermanent)], ActionList([DestroyPermanent(Ref_TargetPermanent), DestroyEachPermanent(Ref_TargetPermanents)])))",
    );
    assert!(
        body.contains("objects: One(Target { slot: Slot(1)"),
        "{body}"
    );
    assert!(body.contains("objects: Targets(Slot(0))"), "{body}");
    let numbered = lowered(
        "SpellActions(Targeted([AnyNumberOfTargetPermanents(AnyPermanent), UptoOneTargetPermanent(AnyPermanent), TargetPermanent(AnyPermanent)], ActionList([DestroyPermanent(Ref_TargetPermanent1), DestroyPermanent(Ref_TargetPermanent2)])))",
    );
    assert!(
        numbered.contains("objects: One(Target { slot: Slot(1)"),
        "{numbered}"
    );
    assert!(
        numbered.contains("objects: One(Target { slot: Slot(2)"),
        "{numbered}"
    );
}

#[test]
fn a_count_inside_a_static_compares_with_the_affected_object() {
    let body = lowered(
        "EachPermanentLayerEffect(IsCardtype(Creature), [AdjustPTForEach(+1, +1, TheNumberOfPermanentsOnTheBattlefield(And([Other(EachablePermanent), IsCardtype(Creature)])))])",
    );
    assert!(body.contains("Not(SameObject(Affected))"), "{body}");
    assert!(!body.contains("SameObject(Candidate)"), "{body}");
}

#[test]
fn it_after_a_tap_is_the_target_even_if_already_tapped() {
    let body = lowered(
        "SpellActions(Targeted([TargetPermanent(AnyPermanent)], ActionList([TapPermanent(Ref_TargetPermanent), PermanentDoesntUntapDuringControllersNextUntap(ThePermanentTappedThisWay)])))",
    );
    assert!(body.contains("affected: Objects(One(Target"), "{body}");
    assert!(!body.contains("Transition"), "{body}");
}

#[test]
fn a_cost_any_player_may_pay_is_paid_by_that_player() {
    let body = lowered(
        "SpellActions(ActionList([AnyPlayerMayCost(AnyPlayer, HavePermanentDealDamage(ThisPermanent, Integer(4), Player(ActionPlayer)))]))",
    );
    assert!(
        body.contains("PlayerAction { player: FirstWilling"),
        "{body}"
    );
    assert!(
        body.contains("recipients: Players(One(FirstWilling"),
        "{body}"
    );
}

#[test]
fn a_this_way_row_binds_nothing_when_no_instruction_has_its_verb() {
    let body = lowered(
        "SpellActions(ActionList([Surveil(Integer(1)), DrawACard, If(APermanentWasCopiedThisWay(AnyPermanent), [Scry(Integer(2))])]))",
    );
    assert!(!body.contains("Result(ResultId("), "{body}");
}

#[test]
fn a_this_way_row_names_the_instruction_it_reads() {
    let body = lowered(
        "SpellActions(ActionList([Surveil(Integer(1)), DrawACard, If(ACardWasSurveiledIntoGraveyardThisWay(IsCardtype(Creature)), [Scry(Integer(1))])]))",
    );
    assert!(body.contains("Result(ResultId(0))"), "{body}");
}

#[test]
fn a_player_event_row_is_one_generic_event() {
    let body =
        lowered("TriggerA(WhenAPlayerGainsLife(SinglePlayer(You)), ActionList([DrawACard]))");
    assert!(body.contains("Player { event: GainsLife"), "{body}");
    assert!(!body.contains("WhenAPlayerGainsLife"), "{body}");
}

#[test]
fn the_nth_event_each_turn_wraps_the_plain_event() {
    let body = lowered(
        "TriggerA(WhenAPlayerDrawsTheirNthCardEachTurn(SinglePlayer(You), EqualTo(Integer(2))), ActionList([DrawACard]))",
    );
    assert!(
        body.contains("NthEachTurn { event: Player { event: Draws"),
        "{body}"
    );
}

#[test]
fn object_event_rows_reduce_and_keep_what_they_bind() {
    let body = lowered(
        "TriggerA(WhenACreatureBlocksACreature(SinglePermanent(ThisPermanent), IsCardtype(Creature)), ActionList([DrawACard]))",
    );
    assert!(body.contains("Pair { event: Blocks"), "{body}");
    let body = lowered(
        "TriggerA(WhenAPermanentBecomesAttachedToAPermanent(IsCardtype(Enchantment), ManaValueIs(LessThanOrEqualTo(ManaValueOfPermanent(Trigger_ThatPermanent)))), ActionList([DrawACard]))",
    );
    assert!(body.contains("Pair { event: BecomesAttachedTo"), "{body}");
}

#[test]
fn a_keyword_action_row_is_one_generic_operation_and_keeps_its_result() {
    let body = lowered("SpellActions(ActionList([Investigate]))");
    assert!(body.contains("Keyword { actor"), "{body}");
    assert!(body.contains("action: Investigate"), "{body}");
    let body = lowered(
        "SpellActions(ActionList([Amass(Integer(2), Zombie), PutCounters([ACounterOfTypeOnPermanent(PTCounter(+1, +1), TheArmyAmassedThisWay)])]))",
    );
    assert!(body.contains("action: Amass"), "{body}");
    assert!(body.contains("subtype: Some(\"Zombie\")"), "{body}");
}
