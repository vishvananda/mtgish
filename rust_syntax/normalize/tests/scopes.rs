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
    assert!(bound.contains("One(Target { slot: Slot(0)"), "{bound}");
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
fn attacks_are_one_event_and_a_batch_wraps_it() {
    let body = lowered(
        "TriggerA(WhenACreatureAttacksAPlayer(IsCardtype(Creature), Opponent), ActionList([DrawACard]))",
    );
    assert!(
        body.contains("Attack { attackers: CardType(Creature)"),
        "{body}"
    );
    assert!(body.contains("defenders: Players(OpponentsOf"), "{body}");
    let body = lowered(
        "TriggerA(WhenAPlayerAttacksWithAnyNumberOfCreatures(SinglePlayer(You), IsCardtype(Creature)), ActionList([DrawACard]))",
    );
    assert!(body.contains("Batch { event: Attack"), "{body}");
    assert!(body.contains("per: Whole"), "{body}");
    assert!(!body.contains("WhenAPlayerAttacks"), "{body}");
    let body = lowered(
        "TriggerA(WhenAPlayerAttacksAPlayer(SinglePlayer(You), Opponent), ActionList([DrawACard]))",
    );
    assert!(body.contains("per: Defender"), "{body}");
}

#[test]
fn battalion_names_the_creature_that_must_be_among_the_attackers() {
    let body = lowered(
        "TriggerA(WhenASingleCreatureAndANumberOfOtherCreaturesAttack(ThisPermanent, GreaterThanOrEqualTo(Integer(2)), IsCardtype(Creature)), ActionList([DrawACard]))",
    );
    assert!(body.contains("including: Some(Source)"), "{body}");
    assert!(body.contains("count: Some((GreaterOrEqual"), "{body}");
}

#[test]
fn a_batch_of_an_exact_group_cannot_read_one_of_its_objects() {
    let event = EventSpec::Batch {
        event: Box::new(EventSpec::ZoneChange {
            objects: ObjectFilter::Any,
            from: None,
            to: EventZone::Battlefield,
        }),
        per: BatchPer::Whole,
        count: Some((Comparison::GreaterOrEqual, Expr::Constant(2))),
        including: None,
    };
    let bindings = semantics::event_bindings(&event);
    assert!(!bindings.object && bindings.group && bindings.amount);
}

#[test]
fn counter_events_keep_their_kind_and_group_by_object() {
    let body = lowered(
        "TriggerA(WhenAnyNumberOfCountersOfTypeArePutOnAPermanent(PTCounter(1, 1), IsCardtype(Creature)), ActionList([DrawACard]))",
    );
    assert!(
        body.contains("Batch { event: Counters { change: Put"),
        "{body}"
    );
    assert!(body.contains("kind: Some(PowerToughness(1, 1))"), "{body}");
    assert!(body.contains("per: Subject"), "{body}");
}

#[test]
fn a_plane_or_scheme_event_reads_this_card() {
    let body = lowered(
        "TriggerA(WhenAPlayerSetsASchemeInMotion(SinglePlayer(You), SingleScheme(ThisScheme)), ActionList([DrawACard]))",
    );
    assert!(body.contains("event: SetsSchemeInMotion"), "{body}");
    assert!(body.contains("objects: Some(SameObject(Source))"), "{body}");
}

#[test]
fn another_of_the_same_name_never_counts_the_object_itself() {
    let body = lowered(
        "SpellActions(ActionList([DestroyEachPermanent(SharesANameWithAnotherPermanent(AnyPermanent))]))",
    );
    assert!(body.contains("SharesNameWithAnother("), "{body}");
    assert!(!body.contains("SharesNameWithAny"), "{body}");
    let body = lowered(
        "SpellActions(ActionList([DestroyEachPermanent(DoesntShareANameWithAnotherPermanent(AnyPermanent))]))",
    );
    assert!(body.contains("Not(SharesNameWithAnother("), "{body}");
}

#[test]
fn a_deck_rule_is_typed_and_names_its_partner() {
    let body = lowered("DeckConstruction(PartnerWith(\"Alisaie Leveilleur\"))");
    assert!(body.contains("DeckRule(Partner(With("), "{body}");
    let body = lowered("DeckConstruction(ChooseABackground)");
    assert!(body.contains("DeckRule(ChooseBackground)"), "{body}");
}

#[test]
fn choosing_a_player_or_an_ability_is_a_chosen_value() {
    let body = lowered("SpellActions(ActionList([ChooseAPlayer(Opponent)]))");
    assert!(body.contains("kind: Player, count: 1"), "{body}");
    let body = lowered("SpellActions(ActionList([ChooseAnAbility([Flying, Haste])]))");
    assert!(body.contains("kind: Ability, count: 1"), "{body}");
}

#[test]
fn a_loyalty_cost_of_x_has_loyalty_timing() {
    let body = lowered("Activated(LoyaltyMinusX, ActionList([DrawACard]))");
    assert!(
        body.contains("Loyalty(Multiply(Constant(-1), AnnouncedX))"),
        "{body}"
    );
    assert!(body.contains("sorcery: true"), "{body}");
}

#[test]
fn costs_with_x_and_counters_are_typed() {
    let body =
        lowered("Activated(PayManaX([ManaCostX, ManaCostU], ValueX), ActionList([DrawACard]))");
    assert!(body.contains("Mana([X, Color(Blue)])"), "{body}");
    let body = lowered(
        "Activated(PayManaX([ManaCostX], TheNumberOfPermanentsOnTheBattlefield(IsCardtype(Land))), ActionList([DrawACard]))",
    );
    assert!(body.contains("ManaWithX { mana: [X], x: Some("), "{body}");
    let body = lowered(
        "Activated(PutCounters([ACounterOfTypeOnPermanent(PTCounter(-1, -1), ThisPermanent)]), ActionList([DrawACard]))",
    );
    assert!(body.contains("PutCounters([CounterPlacement"), "{body}");
    let body = lowered("Activated(Waterbend([ManaCostGeneric(3)]), ActionList([DrawACard]))");
    assert!(body.contains("Waterbend([Generic(3)])"), "{body}");
}

#[test]
fn an_activation_rule_is_data_on_the_ability() {
    let body = lowered(
        "ActivatedWithModifiers(TapPermanent(ThisPermanent), ActionList([DrawACard]), ReduceCostForEach([CostReduceGeneric(1)], TheNumberOfPermanentsOnTheBattlefield(IsCardtype(Land))))",
    );
    assert!(
        body.contains("ActivationRule(Cost { modifier: Reduce"),
        "{body}"
    );
    let body = lowered(
        "ActivatedWithModifiers(TapPermanent(ThisPermanent), ActionList([DrawACard]), Boast)",
    );
    assert!(body.contains("keyword: Some(Boast)"), "{body}");
    assert!(body.contains("once_each_turn: true"), "{body}");
}

#[test]
fn an_activation_is_an_event_with_a_stack_filter() {
    let body = lowered(
        "TriggerA(WhenAPlayerActivatesAnAbility(SinglePlayer(You), LoyaltyAbility), ActionList([DrawACard]))",
    );
    assert!(body.contains("abilities: OnStack(Loyalty)"), "{body}");
    let body = lowered(
        "TriggerA(WhenAPlayerActivatesAnAbility(Opponent, And([ExhaustAbility, AbilityOfPermanent(HostPermanent)])), ActionList([DrawACard]))",
    );
    assert!(body.contains("OnStack(Keyword(Exhaust))"), "{body}");
    assert!(body.contains("AbilityOf(SameObject("), "{body}");
}

#[test]
fn level_up_and_station_are_stages_of_a_counter_count() {
    let body = lowered(
        "LevelUp(PayMana([ManaCostGeneric(2)]), [LevelNoRules(BetweenValues(1, 3), PT(4, 4)), Level(ValueOrBigger(4), PT(6, 6), [Trample])])",
    );
    assert!(body.contains("counter: Named(LevelCounter)"), "{body}");
    assert!(
        body.contains("minimum: 1, maximum: Some(3), power_toughness: Some((4, 4))"),
        "{body}"
    );
    assert!(body.contains("minimum: 4, maximum: None"), "{body}");
    let body = lowered("StationChargedAnimate(ValueOrBigger(8), [Flying], PT(5, 4))");
    assert!(body.contains("counter: Named(ChargeCounter)"), "{body}");
    assert!(body.contains("animates: true"), "{body}");
}

#[test]
fn max_speed_is_an_ordinary_condition_and_backup_keeps_its_abilities() {
    let body = lowered("MaxSpeed(Flying)");
    assert!(body.contains("Conditional {"), "{body}");
    assert!(body.contains("state: MaxSpeed"), "{body}");
    let body = lowered("Backup(2, [Flying])");
    assert!(body.contains("Backup { amount: 2"), "{body}");
}

#[test]
fn casting_on_resolution_is_one_operation() {
    let body =
        lowered("SpellActions(ActionList([CastASpellFromHandWithoutPaying(IsCardtype(Sorcery))]))");
    assert!(body.contains("PlayCards {"), "{body}");
    assert!(body.contains("action: CastSpell"), "{body}");
    assert!(body.contains("cost: Free"), "{body}");
    assert!(body.contains("zone: Hand"), "{body}");
    assert!(body.contains("into_exile: false"), "{body}");
    let body = lowered(
        "SpellActions(Targeted([TargetGraveyardCard(IsNonCardtype(Land))], ActionList([Exile([CardInGraveyards(Ref_TargetGraveyardCard)], []), CopyExiledCard(TheCardExiledThisWay), MayAction(CastCopiedCardWithoutPaying)])))",
    );
    assert!(body.contains("CopyCards {"), "{body}");
    assert!(body.contains("cards: Copies("), "{body}");
}

#[test]
fn a_change_to_a_spell_names_the_spell_and_ordinary_changes() {
    let body = lowered(
        "SpellActions(ActionList([CreateFutureSpellEffect(TheNextSpellPlayerCastsThisTurn(AnySpell, You), [Cascade])]))",
    );
    assert!(body.contains("ChangeSpell { subject: Next"), "{body}");
    assert!(body.contains("this_turn: true"), "{body}");
    assert!(body.contains("GrantAbility("), "{body}");
    let body = lowered(
        "TriggerA(WhenAPlayerCastsASpell(SinglePlayer(You), AnySpell), ActionList([CreateSpellEffect(Trigger_ThatSpell, [EntersWithACounter(PTCounter(1, 1))])]))",
    );
    assert!(body.contains("subject: One("), "{body}");
    assert!(body.contains("EntersWithCounters {"), "{body}");
}

#[test]
fn a_chosen_value_is_one_choice_read_by_kind() {
    let body = lowered("SpellActions(ActionList([ChooseACreatureType]))");
    assert!(body.contains("ChooseValue"), "{body}");
    assert!(body.contains("kind: CreatureType, count: 1"), "{body}");
    let body = lowered(
        "PermanentLayerEffect(HostPermanent, [AddCreatureTypeVariable(TheChosenCreatureType)])",
    );
    assert!(
        body.contains("ChosenType { kind: CreatureType, mode: Add }"),
        "{body}"
    );
}

#[test]
fn a_noted_type_is_not_a_chosen_one() {
    let body = lowered(
        "SpellActions(ActionList([DestroyEachPermanent(IsCreatureTypeVariable(TheNotedCreatureType))]))",
    );
    assert!(
        body.contains("Permanents::IsCreatureTypeVariable"),
        "{body}"
    );
    assert!(!body.contains("Chosen(CreatureType)"), "{body}");
}

#[test]
fn a_player_comparison_is_an_ordinary_count() {
    let body = lowered(
        "TriggerI(WhenACreatureAttacks(SinglePermanent(ThisPermanent)), PlayerPassesFilter(Trigger_DefendingPlayer, ControlsMorePermanentThanPlayer(You, IsCardtype(Land))), ActionList([DrawACard]))",
    );
    assert!(body.contains("Compare"), "{body}");
    assert!(body.contains("comparison: Greater"), "{body}");
    assert!(!body.contains("ControlsMorePermanentThanPlayer"), "{body}");
}

#[test]
fn party_size_and_starting_life_are_numbers() {
    let body =
        lowered("SpellActions(ActionList([DrawNumberCards(NumCreaturesInPlayersParty(You))]))");
    assert!(body.contains("PartySize(AbilityController)"), "{body}");
}

#[test]
fn a_perpetual_effect_is_an_ordinary_change_on_any_subject() {
    let body = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([CreatePerpetualPermanentEffect(ThisPermanent, [AdjustPT(1, 1)])]))",
    );
    assert!(body.contains("CreatePerpetualEffect"), "{body}");
    assert!(body.contains("Pump"), "{body}");
    assert!(!body.contains("Action::CreatePerpetual"), "{body}");
}

#[test]
fn an_emblem_holds_ordinary_abilities() {
    let body = lowered("SpellActions(ActionList([GetAnEmblem([Flying])]))");
    assert!(body.contains("CreateEmblem"), "{body}");
    assert!(!body.contains("Action::GetAnEmblem"), "{body}");
}

#[test]
fn a_clone_enters_as_a_copy_with_its_exceptions() {
    let body = lowered(
        "AsPermanentEnters(ThisPermanent, [MayActions([EnterAsACopyOfAPermanent(IsCardtype(Creature), NoCopyEffects)])])",
    );
    assert!(
        body.contains("Entry(CopyOf { objects: Battlefield(CardType(Creature))"),
        "{body}"
    );
}

#[test]
fn counter_moves_say_how_many_of_what_from_where() {
    let body = lowered(
        "Activated(TapPermanent(ThisPermanent), ActionList([MoveCounters([AllCountersFromPermanentOntoNewPermanent(ThisPermanent, ThisPermanent)])]))",
    );
    assert!(body.contains("MoveCounters"), "{body}");
    assert!(body.contains("amount: All"), "{body}");
}

#[test]
fn a_keyword_row_keeps_its_result_when_conjuring() {
    let body = lowered(r#"SpellActions(ActionList([ConjureCardIntoHand("Nettlecyst")]))"#);
    assert!(body.contains("Conjure"), "{body}");
    assert!(body.contains("zone: Hand"), "{body}");
}

#[test]
fn rolling_a_die_says_how_many_sides() {
    let body = lowered("SpellActions(ActionList([RollAD6]))");
    assert!(body.contains("RollDie(6)"), "{body}");
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

#[test]
fn a_keyword_ability_row_carries_typed_parameters() {
    let body = lowered("Morph(PayMana([ManaCostGeneric(3)]))");
    assert!(
        body.contains("Mechanic { mechanic: Morph, cost: Some("),
        "{body}"
    );
    let body = lowered("Toxic(2)");
    assert!(body.contains("mechanic: Toxic"), "{body}");
    assert!(body.contains("amount: Some(Constant(2))"), "{body}");
    let body = lowered("Affinity(IsCardtype(Artifact))");
    assert!(body.contains("mechanic: Affinity"), "{body}");
    assert!(!body.contains("filter: None"), "{body}");
}

#[test]
fn object_state_rows_are_filters_that_compose() {
    let body = lowered(
        "SpellActions(ActionList([DestroyEachPermanent(And([IsCardtype(Creature), IsEnchanted]))]))",
    );
    assert!(body.contains("State(Enchanted)"), "{body}");
    assert!(!body.contains("Permanents::IsEnchanted"), "{body}");
    let body = lowered(
        "SpellActions(ActionList([DestroyEachPermanent(And([IsCardtype(Creature), IsYourCommander]))]))",
    );
    assert!(body.contains("State(Commander)"), "{body}");
    assert!(body.contains("OwnedBy(One(AbilityController))"), "{body}");
}

#[test]
fn an_outlaw_is_its_five_creature_types() {
    let body = lowered("SpellActions(ActionList([DestroyEachPermanent(IsAnOutlaw)]))");
    assert!(body.contains("Either(["), "{body}");
    assert_eq!(body.matches("Subtype(").count(), 5, "{body}");
}

#[test]
fn turn_windows_and_player_states_are_generic_conditions() {
    let body = lowered("If(IsDuringCombat, [PlayerEffect(You, [CantGainLife])])");
    assert!(body.contains("Window(Combat)"), "{body}");
    let body =
        lowered("If(PlayerPassesFilter(You, IsTheMonarch), [PlayerEffect(You, [CantGainLife])])");
    assert!(body.contains("state: Monarch"), "{body}");
}

#[test]
fn a_clash_keeps_both_outcomes_and_its_opponent_only_exists_inside_them() {
    let frozen = "CreateEachPermanentRuleEffectUntil(And([IsCardtype(Creature), ControlledByPlayer(ClashOpponent)]), [DoesntUntapDuringControllersUntap], DuringPlayersNextUntapStep(ClashOpponent))";
    let body = lowered(&format!(
        "SpellActions(ActionList([Clash(Opponent, [{frozen}], [DoNothing])]))"
    ));
    assert!(body.contains("Clash {"), "{body}");
    assert!(body.contains("won: ["), "{body}");
    assert!(body.contains("lost: ["), "{body}");
    assert!(body.contains("ClashOpponent"), "{body}");
    assert!(!body.contains("Action::Clash"), "{body}");
    let inside = format!("SpellActions(ActionList([Clash(Opponent, [{frozen}], [DoNothing])]))");
    validate::card(&custom(&inside).unwrap()).unwrap();
    let outside = custom(&format!("SpellActions(ActionList([{frozen}]))")).unwrap();
    assert!(validate::card(&outside).is_err());
}

#[test]
fn support_is_counters_and_goading_a_set_is_one_operation() {
    let body = lowered(
        "SpellActions(Targeted([UptoNumberTargetPermanents(Integer(2), IsCardtype(Creature))], ActionList([Support(Ref_TargetPermanents)])))",
    );
    assert!(body.contains("PutCounters {"), "{body}");
    assert!(body.contains("PowerToughness(1, 1)"), "{body}");
    let body = lowered("SpellActions(ActionList([GoadEachCreature(IsCardtype(Creature))]))");
    assert!(body.contains("action: Goad"), "{body}");
    assert!(body.contains("until: None"), "{body}");
    let body = lowered("PermanentRuleEffect(HostPermanent, [IsGoadedByPlayer(You)])");
    assert!(body.contains("GoadedBy("), "{body}");
}

#[test]
fn spellbooks_pairs_and_recipients_are_typed_references() {
    let body = lowered("SpellActions(ActionList([DraftACardFromSpellBook(\"Planets\")]))");
    assert!(
        body.contains("DraftFromSpellbook") && body.contains("spellbook: \"Planets\""),
        "{body}"
    );
    let body = lowered(
        "If(PermanentPassesFilter(ThisPermanent, IsPaired), [EachPermanentLayerEffect(SinglePermanent(TheCreaturePairedWithPermanent(ThisPermanent)), [AdjustPT(+2, +2)])])",
    );
    assert!(body.contains("PairedWith(Source)"), "{body}");
    let body = lowered(
        "SpellActions(Targeted([TargetPlayerOrPermanent(Opponent, IsCardtype(Planeswalker))], ActionList([PlayerAction(PlayerOrControllerOfPermanent(Ref_TargetPlayerOrPermanent), DrawACard)])))",
    );
    assert!(body.contains("PlayerOrControllerOf("), "{body}");
}

#[test]
fn the_nth_spell_counts_among_its_own_filter() {
    let body = lowered(
        "StackSpellsEffect(TheNthSpellCastByPlayerThisTurn(Integer(2), IsCardtype(Instant), You), [Cascade])",
    );
    assert!(body.contains("NthCast { ordinal: Constant(2)"), "{body}");
    assert!(body.contains("spells: CardType(Instant)"), "{body}");
    assert!(body.contains("scope: Turn"), "{body}");
}

#[test]
fn entering_under_control_binds_the_controller_and_leaving_a_graveyard_is_a_batch() {
    let body = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefieldUnderAPlayersControl(IsCardtype(Land), Opponent), ActionList([DrawACard]))",
    );
    assert!(body.contains("event: EntersUnderControl"), "{body}");
    assert!(body.contains("objects: Some(CardType(Land))"), "{body}");
    let body = lowered(
        "TriggerA(WhenAnyNumberOfGraveyardCardsLeave(InAPlayersGraveyard(SinglePlayer(You))), ActionList([DrawACard]))",
    );
    assert!(body.contains("Batch {"), "{body}");
    assert!(body.contains("event: LeavesGraveyard"), "{body}");
    assert!(body.contains("per: Whole"), "{body}");
}

#[test]
fn a_graveyard_static_reaches_cards_in_that_graveyard() {
    let body = lowered(
        "EachCardInGraveyardEffect(IsCardtype(Land), You, [AddAbility([Dredge(Integer(2))])])",
    );
    assert!(body.contains("Static {"), "{body}");
    assert!(body.contains("zone: Graveyard"), "{body}");
    assert!(body.contains("GrantAbility"), "{body}");
    assert!(!body.contains("EachCardInGraveyardEffect"), "{body}");
}

#[test]
fn would_be_events_are_generic_and_keep_who_they_replace() {
    let body = lowered(
        "If(IsPlayersTurn(You), [ReplaceWouldLoseLife(APlayerWouldLoseLife(Opponent), [LoseLife(Twice(WouldLoseLife_ThatMuch))])])",
    );
    assert!(body.contains("event: LoseLife"), "{body}");
    assert!(body.contains("EventSubject"), "{body}");
    let body = lowered(
        "ReplaceWouldDraw(APlayerWouldDrawTwoOrMoreCards(Opponent), [DrawACard, PlayerAction(WouldDrawACard_ThatPlayer, DrawACard)])",
    );
    assert!(body.contains("Measured {"), "{body}");
    assert!(body.contains("event: DrawCards"), "{body}");
    assert!(body.contains("Cancel"), "{body}");
    let body = lowered(
        "ReplaceAnyNumberOfTokensWouldBeCreated(AnyNumberOfTokensWouldBeCreatedUnderAPlayersControl(AnyPermanent, SinglePlayer(You)), [CreateTokensInstead([ThoseTokens, FoodToken])])",
    );
    assert!(body.contains("Tokens {"), "{body}");
    assert!(body.contains("effect_only: false"), "{body}");
    let body = lowered(
        "ReplaceWouldPutCounters(CountersOfTypeWouldBePointOnAPermanent(PTCounter(+1, +1), IsCardtype(Creature)), [PutNewAmount(Twice(ThatManyCounters))])",
    );
    assert!(
        body.contains("Counters { kind: Some(PowerToughness(1, 1))"),
        "{body}"
    );
    let body = lowered(
        "ReplaceWouldProduceMana(TappingAPermanentWouldProduceMana(IsCardtype(Land)), [WouldProduceMana_AddMana(ManaProduceB)])",
    );
    assert!(body.contains("ProduceMana {"), "{body}");
    let body = lowered(
        "ReplaceWouldDestroy(PermanentWouldBeDestroyed(ThisPermanent), [RegeneratePermanent(WouldBeDestroyed_ThatPermanent)])",
    );
    assert!(body.contains("event: Destroyed"), "{body}");
}

#[test]
fn casting_modes_keep_each_outcome_as_an_ability() {
    let body = lowered(
        "SpellActions_Gift(PlayerAction(TheGiftedPlayer, DrawACard), GiftWasPromised(ActionList([DrawACard])), GiftWasntPromised(ActionList([DiscardACard])))",
    );
    assert!(body.contains("Gift {"), "{body}");
    assert!(body.contains("GiftedPlayer"), "{body}");
    assert!(body.contains("unpromised: Some("), "{body}");
    let body = lowered("Permanent_Gift([PlayerAction(TheGiftedPlayer, DrawACard)])");
    assert!(body.contains("promised: None"), "{body}");
    let body = lowered(
        "SpellActions_Spree([SpreeAction(PayMana([ManaCostGeneric(1)]), ActionList([DrawACard])), SpreeAction(PayMana([ManaCostGeneric(2)]), ActionList([DiscardACard]))])",
    );
    assert!(body.contains("Spree { modes: [SpreeMode"), "{body}");
    assert!(body.contains("Generic(2)"), "{body}");
    let body = lowered(
        "SpellActions_Teamwork(2, CastUsingTeamwork(ActionList([DrawACard])), NotCastUsingTeamwork(ActionList([DiscardACard])))",
    );
    assert!(body.contains("CostBranch {"), "{body}");
    assert!(body.contains("Teamwork(2)"), "{body}");
}

#[test]
fn printed_costs_visits_rooms_and_anchor_words_are_typed() {
    let body = lowered("Prowl([ManaCostGeneric(1), ManaCostR])");
    assert!(body.contains("mechanic: Prowl"), "{body}");
    assert!(body.contains("Mana([Generic(1), Color(Red)])"), "{body}");
    let body = lowered(
        "Prototype(ManaCost: [ManaCostGeneric(1), ManaCostR], CardPT: (Power: 2, Toughness: 1))",
    );
    assert!(
        body.contains("Prototype { mana: [Generic(1), Color(Red)], power: 2, toughness: 1"),
        "{body}"
    );
    let body = lowered("Visit(ActionList([DrawACard]))");
    assert!(body.contains("Visit {"), "{body}");
    assert!(body.contains("prize: None"), "{body}");
    let body =
        lowered("DungeonLevel(\"Temple of Dumathoin\", ActionList([DrawACard]), [\"Next\"])");
    assert!(
        body.contains("DungeonRoom { name: \"Temple of Dumathoin\""),
        "{body}"
    );
    assert!(body.contains("leads_to: [\"Next\"]"), "{body}");
    let body = lowered("AnchorWord(\"Khans\", Flying)");
    assert!(body.contains("ChosenWord(\"Khans\")"), "{body}");
}

#[test]
fn doubling_triggers_names_the_event_that_causes_them() {
    let body = lowered(
        "APermanentDyingCausesAbilitiesToTriggerAnAdditionalTime(IsCardtype(Creature), AbilityOfAPermanent(ControlledByAPlayer(SinglePlayer(You))))",
    );
    assert!(body.contains("TriggerRule {"), "{body}");
    assert!(body.contains("cause: Some(ZoneChange"), "{body}");
    assert!(body.contains("to: Graveyard"), "{body}");
    assert!(body.contains("rule: TriggersAgain"), "{body}");
    let body = lowered(
        "PermanentsEnteringTheBattlefieldDontCauseAbilitiesToTrigger(IsCardtype(Creature))",
    );
    assert!(body.contains("rule: DoesntTrigger"), "{body}");
    assert!(body.contains("abilities: None"), "{body}");
    let body = lowered("TheLegendsRuleDoesntApplyToPermanents(IsCardtype(Creature))");
    assert!(body.contains("LegendRuleExempt"), "{body}");
}

#[test]
fn copying_and_retargeting_keep_what_happens_to_the_targets() {
    let body = lowered(
        "FromStack(TriggerA(WhenAPlayerCastsASpell(SinglePlayer(You), SingleSpell(ThisSpell)), ActionList([CopySpellNumberTimesAndMayChooseNewTargets(ThisSpell, ValueX)])))",
    );
    assert!(body.contains("CopyOnStack {"), "{body}");
    assert!(body.contains("targets: May"), "{body}");
    let body = lowered(
        "SpellActions(Targeted([TargetSpell(HasASingleTarget)], ActionList([ChangeTargetsOfSpell(Ref_TargetSpell)])))",
    );
    assert!(body.contains("Retarget {"), "{body}");
    assert!(body.contains("targets: Must"), "{body}");
}

#[test]
fn conjuring_names_the_card_the_count_and_the_place() {
    let body = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([ConjureNumberCardsOnTopOfLibrary(Integer(7), \"Seven Dwarves\")]))",
    );
    assert!(body.contains("card: Named(\"Seven Dwarves\")"), "{body}");
    assert!(body.contains("count: Constant(7)"), "{body}");
    assert!(body.contains("spot: Some(Top)"), "{body}");
    let body = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([ConjureACardOfChoiceFromSpellBookIntoHand(\"Paradigm\")]))",
    );
    assert!(
        body.contains("Spellbook { book: \"Paradigm\", pick: Choice }"),
        "{body}"
    );
    let body = lowered(
        "TriggerA(WhenACreatureAttacks(SinglePermanent(ThisPermanent)), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([ConjureDuplicateOfPermanentIntoHand(Ref_TargetPermanent)])))",
    );
    assert!(body.contains("card: Duplicate("), "{body}");
    assert!(body.contains("zone: Hand"), "{body}");
}

#[test]
fn a_keyword_action_on_a_set_names_the_action() {
    let body = lowered(
        "SpellActions(Targeted([UptoNumberTargetPermanents(ValueX, IsCardtype(Creature))], ActionList([PhaseOutEachPermanent(Ref_TargetPermanents)])))",
    );
    assert!(body.contains("KeywordEach {"), "{body}");
    assert!(body.contains("action: PhaseOut"), "{body}");
}

#[test]
fn doubling_power_is_a_pump_by_its_own_power() {
    let body = lowered(
        "SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([DoubleCreaturesPowerUntilEndOfTurn(Ref_TargetPermanent)])))",
    );
    assert!(
        body.contains("Pump { power: ObjectProperty { object: Affected, property: Power"),
        "{body}"
    );
    assert!(body.contains("toughness: Constant(0)"), "{body}");
    assert!(body.contains("evaluation: Snapshot"), "{body}");
}

#[test]
fn the_top_card_moves_only_if_it_matches() {
    let body = lowered(
        "Activated(Loyalty(+1), ActionList([LookAtTheTopCardOfPlayersLibrary(You), MayAction(RevealTopCardOfLibraryAndPutIntoHand(IsCardtype(Creature)))]))",
    );
    assert!(body.contains("MoveTopCard {"), "{body}");
    assert!(body.contains("cards: CardType(Creature)"), "{body}");
    assert!(body.contains("reveal: true"), "{body}");
    assert!(body.contains("destination: OwnersHand"), "{body}");
}

#[test]
fn a_chosen_colors_read_any_number() {
    let body = lowered(
        "SpellActions(Targeted([TargetPermanent(AnyPermanent)], ActionList([ChooseColors, CreatePermanentLayerEffect(Ref_TargetPermanent, [SetColor(TheChosenColors)])])))",
    );
    assert!(body.contains("kind: Color, count: 0"), "{body}");
}

#[test]
fn zone_and_state_triggers_are_generic() {
    let untapped = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefieldUntapped(SinglePermanent(ThisPermanent)), ActionList([CreateTokens([FoodToken], [])]))",
    );
    assert!(
        untapped.contains("ZoneChange") && untapped.contains("Not(Tapped)"),
        "{untapped}"
    );
    let none = lowered(
        "TriggerA(WhenNoPermanentsAreOnTheBattlefield(IsCardtype(Land)), ActionList([SacrificePermanent(ThisPermanent)]))",
    );
    assert!(
        none.contains("State") && !none.contains("Primitive"),
        "{none}"
    );
}

#[test]
fn a_block_of_several_keeps_one_subject() {
    let body = lowered(
        "TriggerA(WhenACreatureBlocksANumberOfCreatures(SinglePermanent(ThisPermanent), GreaterThanOrEqualTo(Integer(2)), IsCardtype(Creature)), ActionList([CreatePermanentLayerEffectUntil(Trigger_ThatPermanent, [AddAbility([FirstStrike])], UntilEndOfTurn)]))",
    );
    assert!(
        body.contains("per: Subject") && body.contains("Blocks"),
        "{body}"
    );
}

#[test]
fn a_player_targeted_or_on_their_turn_is_a_player_event() {
    let targeted = lowered(
        "TriggerA(WhenAPlayerBecomesTheTargetOfASpellOrAbility(SinglePlayer(You), ControlledByAPlayer(Opponent)), ActionList([MayAction(DrawACard)]))",
    );
    assert!(targeted.contains("BecomesTarget"), "{targeted}");
    let turn = lowered(
        "TriggerA(WhenAPlayerLosesLifeDuringTheirTurn(SinglePlayer(You)), ActionList([DrawACard]))",
    );
    assert!(turn.contains("PlayerIsActive(EventSubject)"), "{turn}");
}

#[test]
fn numbers_among_players_and_objects_are_generic() {
    let body = lowered(
        "SpellActions(ActionList([GainLife(HighestLifeTotalAmongPlayers(AnyPlayer)), GainLife(TheLeastPowerAmongPermanents(IsCardtype(Creature))), GainLife(ThirdRoundedUp(Integer(9)))]))",
    );
    assert!(
        body.contains("AmongPlayers") && body.contains("Minimum"),
        "{body}"
    );
    assert!(
        body.contains("rounding: Up") && !body.contains("Primitive"),
        "{body}"
    );
}

#[test]
fn casting_a_commander_counts_from_history() {
    let body = lowered(
        "PlayerEffect(You, [DecreaseBlitzCostsForEach([ManaCostGeneric(1)], NumTimesPlayerHasCastTheirCommanderFromCommandZone(You))])",
    );
    assert!(
        body.contains("WasCastFrom") && body.contains("ThisGame"),
        "{body}"
    );
}

#[test]
fn rules_about_targeting_and_tap_costs_are_generic() {
    let spells = lowered("PermanentRuleEffect(ThisPermanent, [CantBeTheTargetOfSpells(AnySpell)])");
    assert!(
        spells.contains("BeTargeted") && spells.contains("OnStack(Spell)"),
        "{spells}"
    );
    let crew = lowered(
        "PermanentRuleEffect(ThisPermanent, [CrewsVehiclesAsThoughPowerWereGreater(Integer(2))])",
    );
    assert!(
        crew.contains("TapPowerBonus") && crew.contains("Crew"),
        "{crew}"
    );
    let exert = lowered(
        "PermanentRuleEffect(ThisPermanent, [MayBeExertedAsItAttacksWithTrigger(ActionList([DrawACard]))])",
    );
    assert!(exert.contains("MayExert(Some"), "{exert}");
}

#[test]
fn rules_about_a_players_spells_and_abilities_are_generic() {
    let delve = lowered("PlayerEffect(You, [SpellsCastHaveAbility(AnySpell, [Delve])])");
    assert!(delve.contains("SpellsHaveAbilities"), "{delve}");
    let equip = lowered("PlayerEffect(You, [DecreaseEquipAbilityCost([CostReduceGeneric(1)])])");
    assert!(
        equip.contains("ActivationCost") && equip.contains("Equip"),
        "{equip}"
    );
    let look = lowered(
        "PlayerEffect(You, [MayLookAtFaceDownExiledCards(WasExiledWithPermanent(ThisPermanent))])",
    );
    assert!(look.contains("Visibility"), "{look}");
}

#[test]
fn votes_are_counted_by_option() {
    let body = lowered(
        r#"SpellActions(ActionList([EachPlayerStartingWithAction(AnyPlayer, You, [VoteForAWord(["Death", "Taxes"])]), EachPlayerAction(Opponent, SacrificeNumberPermanents(TheNumberOfVotesForWord("Death"), IsCardtype(Creature)))]))"#,
    );
    assert!(body.contains("Vote {") && body.contains("Words"), "{body}");
    assert!(body.contains("Votes(Word(\"Death\"))"), "{body}");
}

#[test]
fn keyword_costs_are_generic() {
    let body = lowered("Ward(Blight(Integer(2)))");
    assert!(body.contains("Keyword { action: Blight"), "{body}");
    let exert = lowered(
        "Activated(And([TapPermanent(ThisPermanent), ExertPermanent(ThisPermanent)]), ActionList([AddManaRepeated(AnyManaColor, Integer(2))]))",
    );
    assert!(exert.contains("Exert(Source)"), "{exert}");
}

#[test]
fn manifesting_keeps_what_entered() {
    let body = lowered("SpellActions(ActionList([ManifestTheTopCardOfPlayersLibrary(You)]))");
    assert!(
        body.contains("Manifest {") && body.contains("TopOfLibrary"),
        "{body}"
    );
    let cloak = lowered("SpellActions(ActionList([CloakTheTopCardOfPlayersLibrary(You)]))");
    assert!(cloak.contains("cloak: true"), "{cloak}");
}

#[test]
fn mana_that_stays_and_mana_like_what_was_produced() {
    let body = lowered(
        "TriggerA(AtTheBeginningOfAPlayersUpkeep(SinglePlayer(You)), ActionList([AddManaWithModifiers(ManaProduceG, DontLoseAsStepsAndPhasesEnd(UntilEndOfTurn))]))",
    );
    assert!(body.contains("Persists"), "{body}");
    let same = lowered(
        "TriggerA(WhenAPlayerTapsAPermanentForMana(SinglePlayer(You), IsCardtype(Land)), ActionList([AddMana(Trigger_AnyManaTypeProduced)]))",
    );
    assert!(same.contains("SameTypeAsProduced"), "{same}");
}

#[test]
fn layer_rows_and_hand_statics_are_generic() {
    let hand = lowered(
        "EachCardInPlayersHandEffect(IsCardtype(Land), You, [AddAbility([Cycling(PayMana([ManaCostR]))])])",
    );
    assert!(
        hand.contains("Hand") && hand.contains("GrantAbility"),
        "{hand}"
    );
    let gained = lowered(
        "PermanentLayerEffect(ThisPermanent, [AddAbilityFromEachPermanentHasable(IsCardtype(Land), [ActivatedAbility])])",
    );
    assert!(gained.contains("GainAbilitiesOf"), "{gained}");
}

#[test]
fn effects_on_spells_and_game_rules_are_operations() {
    let exiled = lowered(
        "TriggerA(WhenAPermanentDealsDamage(SinglePermanent(ThisPermanent)), ActionList([Exile([Permanent(ThisPermanent)], [ExilesFaceDown]), CreateExiledCardEffect(TheCardExiledThisWay, [IsForetold])]))",
    );
    assert!(exiled.contains("EffectOn"), "{exiled}");
    let game = lowered(
        "SpellActions(ActionList([CreateGameEffect(UntilEndOfTurn, DamageCantBePrevented)]))",
    );
    assert!(game.contains("GameEffect"), "{game}");
}

#[test]
fn keeping_some_and_sacrificing_the_rest() {
    let body = lowered(
        "TriggerA(AtTheBeginningOfAPlayersEndStep(Opponent), ActionList([PlayerAction(Trigger_ThatPlayer, SacrificeAllPermanentsExceptForNum(IsCardtype(Creature), Integer(2)))]))",
    );
    assert!(body.contains("SacrificeAllBut"), "{body}");
}

#[test]
fn deck_and_draft_rules_are_abilities() {
    let craft = lowered("CraftWithACraftable(IsLandType(Cave), [ManaCostGeneric(5), ManaCostG])");
    assert!(craft.contains("Craft {"), "{craft}");
    let offering = lowered("Offering(IsCreatureType(Fox))");
    assert!(offering.contains("Offering(Subtype"), "{offering}");
    let draft = lowered("AsSelfDraft([DraftFaceUp])");
    assert!(draft.contains("DraftRule"), "{draft}");
}

#[test]
fn counters_copied_and_players_compared() {
    let copied = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([PutCounters([DuplicateCountersOfPermanentOnPermanent(ThisPermanent, ThisPermanent)])]))",
    );
    assert!(copied.contains("DuplicateCounters"), "{copied}");
    assert!(!copied.contains("Primitive"), "{copied}");
    let most = lowered(
        "If(PlayerPassesFilter(You, HasHighestNumberOfCardsInHandAmongPlayers(Opponent)), [PermanentLayerEffect(ThisPermanent, [AdjustPT(+1, +1)])])",
    );
    assert!(most.contains("Greater"), "{most}");
    assert!(!most.contains("GreaterOrEqual"), "{most}");
    assert!(most.contains("AmongPlayers"), "{most}");
    let class = lowered(
        "TriggerA(WhenAClassBecomesLevel(SinglePermanent(ThisPermanent), Integer(2)), ActionList([DrawNumberCards(Integer(2))]))",
    );
    assert!(class.contains("ClassLevel"), "{class}");
}

#[test]
fn escaping_mana_purposes_and_random_choices() {
    let escape =
        lowered("AsPermanentEscapes(ThisPermanent, [EntersWithACounter(PTCounter(+1, +1))])");
    assert!(escape.contains("Escaped"), "{escape}");
    assert!(validate::card(
        &custom("AsPermanentEscapes(ThisPermanent, [EntersWithACounter(PTCounter(+1, +1))])")
            .unwrap()
    )
    .is_ok());
    let random = lowered(
        "SpellActions(ActionList([ChooseAPlayerAtRandom(AnyPlayer), PlayerAction(TheChosenPlayer, DrawNumberCards(Integer(1)))]))",
    );
    assert!(random.contains("ChooseAtRandom"), "{random}");
    let villain = lowered(
        "SpellActions(ActionList([PlayerAction(You, FaceAVillianousChoice([[DrawNumberCards(Integer(1))], [DrawNumberCards(Integer(2))]]))]))",
    );
    assert!(villain.contains("villainous: true"), "{villain}");
    let held = lowered("DamageIsntRemovedFromCreaturesDuringCleanup(IsCardtype(Creature))");
    assert!(held.contains("DamageStaysMarked"), "{held}");
}

#[test]
fn life_floors_mana_sources_and_acting_players() {
    let floor = lowered(
        "PlayerEffect(You, [DamageThatWouldReduceLifeTotalToLessThanNumberReducesItToThatNumberInstead(Integer(1))])",
    );
    assert!(floor.contains("DamageLifeFloor"), "{floor}");
    let half = lowered(
        "TriggerA(WhenACreatureAttacks(SinglePermanent(ThisPermanent)), ActionList([LoseLife(HalfRoundedUp(LifeTotalOfPlayer(LoseLife_ThatPlayer)))]))",
    );
    assert!(half.contains("Acting"), "{half}");
    assert!(!half.contains("Primitive"), "{half}");
    let spent = lowered(
        "StackSpellsEffect(And([CastByPlayer(You), ManaFromAPermanentWasSpentToCastIt(IsCardtype(Artifact))]), [SplitSecond])",
    );
    assert!(spent.contains("ManaSpentFrom"), "{spent}");
}

#[test]
fn keyword_families_and_paid_cards() {
    let flashback = lowered(
        "SpellActions(Targeted([TargetExiledCard(And([HasAbility(AnyFlashback), OwnedByAPlayer(SinglePlayer(You))]))], ActionList([PutExiledCardIntoOwnersHand(Ref_TargetExiledCard)])))",
    );
    assert!(
        flashback.contains("HasKeywordFamily(Flashback)"),
        "{flashback}"
    );
    let reduced = lowered(
        "CastEffect(ReduceCastingCostForEachAlternateCost([CostReduceGeneric(2)], SacrificeAPermanent(IsCardtype(Creature))))",
    );
    assert!(reduced.contains("ReduceForEachPaid"), "{reduced}");
    let moved = lowered(
        "CastEffect(AdditionalCastingCost(PutAnExiledCardIntoOwnersGraveyard(OwnedByAPlayer(Opponent))))",
    );
    assert!(moved.contains("ExiledToGraveyard"), "{moved}");
    let seek = lowered(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), ActionList([MayCost(DiscardACard), If(CostWasPaid, [SeekACard(ManaValueIs(EqualTo(ManaValueOfTheCardDiscardedThisWay)))])]))",
    );
    assert!(seek.contains("ObjectProperty"), "{seek}");
    assert!(
        !seek.contains("ManaValueOfTheCardDiscardedThisWay"),
        "{seek}"
    );
}

#[test]
fn players_are_barred_from_attacks_and_payments() {
    let attack = lowered(
        "EachPlayerEffect(And([Opponent, CastASpellThisTurn(AnySpell)]), [CantAttackWithCreatures(AnyPermanent)])",
    );
    assert!(attack.contains("AttackWith"), "{attack}");
    let life = lowered(
        "EachPlayerEffect(AnyPlayer, [CantPayLifeToCastSpells(AnySpell), CantPayLifeToActivateAbilities(NonManaAbility)])",
    );
    assert!(life.contains("PayLife(Spells"), "{life}");
    assert!(life.contains("PayLife(Abilities"), "{life}");
}

#[test]
fn balance_and_abilities_of_sources() {
    let balance = lowered(
        "SpellActions(ActionList([EachPlayerAction(AnyPlayer, DiscardAllButNumberCards(TheLowestNumberOfCardsInHandAmongPlayers(AnyPlayer)))]))",
    );
    assert!(balance.contains("DiscardAllBut"), "{balance}");
    let abolisher = lowered(
        "If(IsPlayersTurn(You), [EachPlayerEffect(Opponent, [CantCastSpells(AnySpell), CantActivateAbilities(AbilityOfAPermanent(Or([IsCardtype(Artifact), IsCardtype(Creature), IsCardtype(Enchantment)])))])])",
    );
    assert!(abolisher.contains("Activate(Of("), "{abolisher}");
}

#[test]
fn dice_history_and_ability_traits() {
    let dice = lowered(
        "TriggerA(WhenAPlayerRollsAValueOnADie(SinglePlayer(You), GreaterThanOrEqualTo(Integer(4))), ActionList([If(TriggerDiceResultIs(GreaterThanOrEqualTo(Integer(6))), [DrawACard])]))",
    );
    assert!(dice.contains("EventAmount"), "{dice}");
    assert!(!dice.contains("TriggerDiceResultIs"), "{dice}");
    let least = lowered(
        "SpellActions(ActionList([DrawNumberCards(MinPermanentsControlledByAPlayer(IsCardtype(Creature), AnyPlayer))]))",
    );
    assert!(least.contains("Minimum"), "{least}");
}

#[test]
fn the_rest_of_a_search_and_chosen_token_colors() {
    let gifts = lowered(
        "SpellActions(Targeted([TargetPlayer(Opponent)], ActionList([SearchLibrary([FindNumberGroupCards(Integer(4), AnyCardInLibrary, DifferentNames), RevealFoundCards, APlayerChoosesNumberFoundCards(SinglePlayer(Ref_TargetPlayer), Integer(2)), PutChosenFoundCardsIntoGraveyard, PutFoundCardsIntoHand, Shuffle])])))",
    );
    assert!(gifts.contains("ChooseObjects"), "{gifts}");
    assert!(gifts.contains("Unselected"), "{gifts}");
    assert!(gifts.contains("OwnersGraveyard"), "{gifts}");
}

#[test]
fn chosen_damage_recipients() {
    let curse = lowered(
        "TriggerA(AtTheBeginningOfAPlayersUpkeep(SinglePlayer(HostPlayer)), ActionList([ChooseADamageRecipient(APlayerOrAPermanent(SinglePlayer(Trigger_ThatPlayer), And([IsCardtype(Planeswalker), ControlledByAPlayer(SinglePlayer(Trigger_ThatPlayer))]))), PermanentDealsDamage(ThisPermanent, Integer(1), TheChosenDamageRecipient)]))",
    );
    assert!(curse.contains("kind: Recipient"), "{curse}");
    assert!(curse.contains("Union"), "{curse}");
    assert!(curse.contains("Chosen"), "{curse}");
}

#[test]
fn mana_riders_and_optional_inspection_steps() {
    let rider = lowered(
        "Activated(TapPermanent(ThisPermanent), ActionList([AddManaWithModifiers(ManaProduceR, CreatePermanentSpellLayerEffect(UntilItLeavesTheBattlefield, IsCardtype(Creature), [AddAbility([Haste])]))]))",
    );
    assert!(rider.contains("SpentOn"), "{rider}");
    assert!(rider.contains("Indefinite"), "{rider}");

    let ritual = lowered(
        "TriggerA(AtTheBeginningOfAPlayersEndStep(SinglePlayer(You)), ActionList([LookAtTheTopNumberCardsOfLibrary(Integer(7), [MayCost(SacrificeAPermanent(IsCardtype(Creature))), If(CostWasPaid, [PutAGenericCardIntoHand]), PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder])]))",
    );
    assert!(ritual.contains("PayCost"), "{ritual}");
    assert!(ritual.contains("PaymentChosen"), "{ritual}");

    let genesis = lowered(
        "SpellActions(ActionList([LookAtTheTopNumberCardsOfLibrary(Integer(4), [MayCost(PutACardOfTypeOntoTheBattlefield(IsCardtype(Land), [EntersTapped])), Unless(CostWasPaid, [PutAGenericCardIntoHand]), PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder])]))",
    );
    assert!(!genesis.contains("PayCost"), "{genesis}");
    assert!(genesis.contains("CountObjects"), "{genesis}");

    let keeper = lowered(
        "TriggerA(WhenAPermanentDies(SinglePermanent(ThisPermanent)), ActionList([RevealCardsFromTheTopOfLibraryUntilACardOfTypeIsRevealed(IsNonCardtype(Land), [MayCastTheCardFoundThisWayWithoutPaying, PutTheRemainingCardsOnTheBottomOfLibraryInARandomOrder])]))",
    );
    assert!(keeper.contains("PlayCards"), "{keeper}");
    assert!(keeper.contains("cost: Free"), "{keeper}");
}

#[test]
fn conditional_delayed_triggers_and_copiable_choices() {
    let throne = lowered(
        "SpellActions(Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([CreateTriggerUntilI(WhenAPermanentDies(SinglePermanent(Ref_TargetPermanent)), PlayerPassesFilter(You, ControlsA(IsYourCommander)), ActionList([BecomeTheMonarch]), UntilEndOfTurn)])))",
    );
    assert!(throne.contains("intervening_if: Some"), "{throne}");

    let elves = lowered(
        "TriggerA(WhenAPlayerCastsASpell(SinglePlayer(You), IsCreatureType(Elf)), ActionList([CreatePermanentSpellLayerEffect(UntilEndOfTurn, Trigger_ThatSpell, [AddAbility([Haste])])]))",
    );
    assert!(elves.contains("Continuing"), "{elves}");
    assert!(elves.contains("until: EndOfTurn"), "{elves}");

    let clay = lowered(
        "AsPermanentEnters(ThisPermanent, [EntersWithLayerEffectOfChoice([[SetCopiablePT(PT(3, 3)), AddCopiableCardtype(Creature)], [SetCopiablePT(PT(2, 2)), AddCopiableAbility([Flying])]])])",
    );
    assert!(clay.contains("ChooseChanges"), "{clay}");
    assert!(clay.contains("Copiable"), "{clay}");
}

#[test]
fn first_events_for_an_object_dice_and_group_exile() {
    let successor = lowered(
        "TriggerA(WhenAnyNumberOfCountersArePutOnASpecificPermanentForTheFirstTimeEachTurn(And([IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))])), ActionList([DrawACard]))",
    );
    assert!(successor.contains("FirstForObjectEachTurn"), "{successor}");

    let dice = lowered(
        "ReplaceWouldRollDice(APlayerWouldRollANumberOfDice(SinglePlayer(You)), [RollThatManyPlusOneAndIgnoreLowestInstead])",
    );
    assert!(dice.contains("RollExtraDie(Lowest)"), "{dice}");
    assert!(!dice.contains("Cancel"), "{dice}");

    let triad = lowered(
        "Activated(Exile([AnyNumberOfGroupCardsInGraveyards(And([IsHistoric, InAPlayersGraveyard(SinglePlayer(You))]), TotalManaValueIs(GreaterThanOrEqualTo(Integer(30))))], []), ActionList([DrawACard]))",
    );
    assert!(triad.contains("Constrained"), "{triad}");
    assert!(triad.contains("TotalProperty"), "{triad}");

    let winter = lowered(
        "Activated(Exile([AnyNumberOfGroupCardsInGraveyards(InAPlayersGraveyard(SinglePlayer(You)), ANumberOfDifferentCardTypes(GreaterThanOrEqualTo(Integer(4))))], []), ActionList([DrawACard]))",
    );
    assert!(winter.contains("DistinctCount"), "{winter}");

    let crown = lowered(
        "ReplaceAPlayerWouldCreateTokens(APlayerWouldCreateTokensForTheFirstTimeEachTurn(SinglePlayer(You), AnyPermanent), [ChooseAnAction([[CreateTokensInstead([NumberTokens(WouldCreateTokens_NumberTokens, TokenCopyOfPermanent(HostPermanent, NoTokenCopyEffects))])], [CreateTokensInstead([TreasureToken])]])])",
    );
    assert!(crown.contains("FirstEachTurn"), "{crown}");
    assert!(crown.contains("ChooseAction"), "{crown}");
}

#[test]
fn own_colors_chosen_token_types_and_explored_replacements() {
    let fellowship = lowered(
        "EachPermanentLayerEffect(IsCardtype(Creature), [AddAbility([Protection(FromColor(ItsOwnColors))])])",
    );
    assert!(fellowship.contains("SharesColorWith"), "{fellowship}");

    let lab = lowered(
        "Activated(TapPermanent(ThisPermanent), ActionList([CreateTokens([TokenWithPT(PT(2, 2), TheChosenColor, [], [Creature], TheChosenCreatureType, [])], [])]))",
    );
    assert!(lab.contains("chosen_creature_type: true"), "{lab}");

    let sponge = lowered(
        "Activated(TapPermanent(ThisPermanent), Targeted([TargetPermanent(IsCardtype(Creature))], ActionList([ChooseACheckableAbility([Flying, FirstStrike, Trample]), CreatePermanentLayerEffectUntil(Ref_TargetPermanent, [LosesAbility(TheChosenAbility)], UntilEndOfTurn)])))",
    );
    assert!(sponge.contains("kind: Ability"), "{sponge}");

    let tracker = lowered(
        "ReplaceWouldExplore(APermanentWouldExplore(And([IsCardtype(Creature), ControlledByAPlayer(SinglePlayer(You))])), [ItExplores, ItExplores])",
    );
    assert_eq!(tracker.matches("Explore").count() >= 2, true, "{tracker}");
    assert!(tracker.contains("Cancel"), "{tracker}");
}

#[test]
fn replacement_steps_with_rows_keep_their_operands() {
    let censor = lowered(
        "ReplaceWouldSearchLibrary(APlayerWouldSearchTheirLibrary(Opponent), [SearchTopNumberCardsOfLibraryInstead(Integer(4))])",
    );
    assert!(
        censor
            .contains("ReplacementActionWouldSearchLibrary::SearchTopNumberCardsOfLibraryInstead"),
        "{censor}"
    );
    assert!(censor.contains("Constant(4)"), "{censor}");
}

#[test]
fn repeated_payments_and_payments_per_object() {
    let oath = lowered(
        "SpellActions(ActionList([RepeatedMayCost(Integer(2), PayMana([ManaCostGeneric(1)])), LoseLifeForEach(Integer(1), TheNumberOfRepeatedCostsNotPaid)]))",
    );
    assert!(oath.contains("PayRepeatedly"), "{oath}");
    assert!(oath.contains("Subtract(Constant(2), Performed("), "{oath}");
    let cleansing = lowered(
        "SpellActions(ActionList([ForEachPermanentAnyPlayerMayCost(IsCardtype(Land), AnyPlayer, PayLife(Integer(1))), DestroyEachPermanent(CostWasntPaidForIt)]))",
    );
    assert!(cleansing.contains("ForEachObject"), "{cleansing}");
    assert!(cleansing.contains("Unpaid(ResultId(0))"), "{cleansing}");
    validate::card(&custom(
        "SpellActions(ActionList([ForEachPermanentAnyPlayerMayCost(IsCardtype(Land), AnyPlayer, PayLife(Integer(1))), DestroyEachPermanent(CostWasntPaidForIt)]))",
    ).unwrap())
    .unwrap();
}

#[test]
fn offers_chosen_filters_and_first_upkeeps() {
    let tempt = lowered(
        "SpellActions(ActionList([EachPlayerMayCreateTokens(Opponent, [FoodToken], []), CreateTokens([NumberTokens(NumPlayers(CreatedATokenThisWay), FoodToken)], [])]))",
    );
    assert!(tempt.contains("PlayerAction"), "{tempt}");
    assert!(tempt.contains("PlayerTook"), "{tempt}");
    let filter = lowered(
        "SpellActions(ActionList([ChoosePermanentFilter([IsCardtype(Land), IsCardtype(Creature)]), DestroyEachPermanent(IsTheChosenPermanentFilter)]))",
    );
    assert!(filter.contains("kind: Filter"), "{filter}");
    assert!(filter.contains("ChosenFilter"), "{filter}");
    let dispatch =
        lowered("TriggerA(AtTheBeginningOfTheFirstUpkeepOfTheGame, ActionList([DrawACard]))");
    assert!(dispatch.contains("FirstEachGame"), "{dispatch}");
}

#[test]
fn listed_values_negative_power_and_copied_stack_objects() {
    let listed = lowered(
        "TriggerA(WhenAPlayerCastsASpell(SinglePlayer(You), ManaValueIs(OneOf([4, 5, 6]))), ActionList([DrawACard]))",
    );
    assert!(listed.contains("AnyOf([Constant(4)"), "{listed}");
    let negative = lowered("If(PermanentPassesFilter(ThisPermanent, PowerIs(Negative)), [Flying])");
    assert!(negative.contains("Less"), "{negative}");
    let copied = lowered(
        "FromStack(TriggerA(WhenAPlayerCastsASpell(SinglePlayer(You), SingleSpell(ThisSpell)), ActionList([CopySpellAndMayChooseNewTargets(Trigger_ThatSpell)])))",
    );
    assert!(copied.contains("CopyOnStack {"), "{copied}");
    assert!(copied.contains("targets: May"), "{copied}");
}

#[test]
fn reveal_branches_counters_among_objects_and_stack_loops() {
    let branched = lowered(
        "SpellActions(ActionList([RevealCardsFromTheTopOfLibraryUntilACardOfTypeIsRevealed(IsCardtype(Creature), [IfElse(ACardWasFoundThisWay, [PutTheCardFoundThisWayOnTopOfLibrary, PutTheRemainingCardsIntoGraveyard], [PutTheRemainingCardsBackIntoLibraryAndShuffle])])]))",
    );
    assert!(branched.contains("If {"), "{branched}");
    assert!(branched.contains("ReorderLibrary"), "{branched}");
    let bounded = lowered(
        "SpellActions(ActionList([RevealCardsFromTheTopOfLibraryUntilACardOfTypeIsRevealedOrUntilNumberCardsAreRevealed(IsCardtype(Artifact), Integer(3), [PutTheRemainingCardsIntoGraveyard])]))",
    );
    assert!(bounded.contains("at_most: Some(Constant(3))"), "{bounded}");
    let among = lowered(
        "SpellActions(ActionList([RemoveCounters([UptoNumberCountersOfTypeFromAmongPermanents(Integer(3), StunCounter, AnyPermanent)])]))",
    );
    assert!(among.contains("AmongThem(UpTo(Constant(3)))"), "{among}");
    let stack = lowered(
        "SpellActions(ActionList([ActionForEachSpellAndAbility(ControlledByAPlayer(Opponent), [CounterSpellOrAbility(EachableSpellOrAbility)])]))",
    );
    assert!(stack.contains("ForEachObject"), "{stack}");
    assert!(stack.contains("Shared(Stack)"), "{stack}");
}

#[test]
fn first_draws_skipped_turns_and_group_constraints() {
    let first = lowered(
        "ReplaceWouldDraw(PlayerWouldDrawACardForTheFirstTimeEachPlayersTurnExceptFirstDrawStepDraw(You), [DrawNumberCards(Integer(4))])",
    );
    assert!(first.contains("FirstEachTurn(Player"), "{first}");
    assert!(first.contains("DrawExceptFirstInDrawStep"), "{first}");
    let skipped = lowered(
        "ReplaceWouldBeginDrawStep(PlayerWouldBeginTheirDrawStep(You), [MayCost(SkipThisDrawStep), If(CostWasPaid, [GainLife(Integer(2))])])",
    );
    assert!(skipped.contains("optional: true"), "{skipped}");
    assert_eq!(skipped.matches("Cancel").count(), 1, "{skipped}");
    let duel = lowered(
        "SpellActions(Targeted([NumberTargetGroupPermanents(Integer(2), IsCardtype(Creature), SharesNoCreatureTypes)], ActionList([Fight(Ref_TargetPermanents_0, Ref_TargetPermanents_1)])))",
    );
    assert!(duel.contains("NoCommonProperty(CreatureTypes)"), "{duel}");
    let exchange = lowered(
        "SpellActions(Targeted([NumberTargetPlayers(Integer(2), AnyPlayer)], ActionList([PlayerAction(Ref_TargetPlayers_0, ExchangeLifeTotalWithPlayer(Ref_TargetPlayers_1))])))",
    );
    assert!(exchange.contains("index: 1"), "{exchange}");
}

#[test]
fn chosen_pairs_damage_source_operands_and_hexproof_from_abilities() {
    let pair = lowered(
        "SpellActions(ActionList([ChooseNumberPermanents(Integer(2), IsCardtype(Creature)), CreateValueX(DifferenceBetween(PowerOfPermanent(TheFirstChosenPermanent), PowerOfPermanent(TheSecondChosenPermanent))), DrawNumberCards(ValueX)]))",
    );
    assert!(pair.contains("index: 1"), "{pair}");
    let hexproof = lowered("HexproofFrom(FromActivatedAndTriggeredAbilities)");
    assert!(hexproof.contains("OnStack(Activated)"), "{hexproof}");
    assert!(hexproof.contains("OnStack(Triggered)"), "{hexproof}");
}
