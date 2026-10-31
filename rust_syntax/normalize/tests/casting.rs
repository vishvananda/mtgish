use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, semantics, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/casting.ronl")
        .lines()
        .map(|s| ron::from_str(s).unwrap())
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
    let mut c = source("Cancel");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn casting(c: &Card) -> &CastingRule {
    c.abilities
        .iter()
        .find_map(|ability| match ability {
            Ability::Casting(rule) => Some(rule),
            _ => None,
        })
        .unwrap()
}

#[test]
fn real_casting_families_roundtrip_and_nested_gaps_remain_incomplete() {
    let incomplete = ["Ertai's Meddling", "Rites of Initiation", "Soul Burn"];
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
    assert_eq!(complete, 74);
}

#[test]
fn additional_alternative_and_choice_costs_do_not_become_resolution_effects() {
    let c = card("Altar's Reap");
    assert!(
        matches!(casting(&c), CastingRule::AdditionalCost { optional: false, repetition: CostRepetition::Once, cost: Cost { parts } } if matches!(parts.as_slice(), [CostPart::Sacrifice { choose: Some(Count::Exactly(Expr::Constant(1))), .. }]))
    );
    assert!(
        matches!(&c.abilities[1], Ability::Spell { body, .. } if matches!(body.instructions.as_slice(), [Instruction::Effect { operation: Operation::Draw { .. }, .. }]))
    );
    let c = card("Abolish");
    assert!(
        matches!(casting(&c), CastingRule::AlternativeCost(Cost { parts }) if matches!(parts.as_slice(), [CostPart::Discard { count: Expr::Constant(1), filter: ObjectFilter::Subtype(source::SubType::Plains), random: false }]))
    );
    let c = card("Foil");
    assert!(
        matches!(casting(&c), CastingRule::AlternativeCost(Cost { parts }) if matches!(parts.as_slice(), [CostPart::Discard { filter: ObjectFilter::Subtype(source::SubType::Island), .. }, CostPart::Discard { filter: ObjectFilter::Any, .. }]))
    );
    let c = card("Bitter Triumph");
    assert!(
        matches!(casting(&c), CastingRule::AdditionalCost { cost: Cost { parts }, .. } if matches!(parts.as_slice(), [CostPart::Choice(options)] if matches!(options.as_slice(), [Cost { parts: a }, Cost { parts: b }] if matches!(a.as_slice(), [CostPart::Discard { .. }]) && matches!(b.as_slice(), [CostPart::Life(Expr::Constant(3))]))))
    );
    let c = card("Acceptable Losses");
    assert!(
        matches!(casting(&c), CastingRule::AdditionalCost { cost: Cost { parts }, .. } if matches!(parts.as_slice(), [CostPart::Discard { random: true, .. }]))
    );
}

#[test]
fn exile_and_return_costs_keep_strict_counts_private_zones_and_destinations() {
    let c = card("Force of Will");
    assert!(
        matches!(casting(&c), CastingRule::AlternativeCost(Cost { parts }) if matches!(parts.as_slice(), [CostPart::Life(Expr::Constant(1)), CostPart::Move { destination: Destination::Exile, selections, .. }] if matches!(selections.as_slice(), [CostObjectSelection { objects: ObjectSet::Zone { scope: ZoneScope::Players { zone: EventZone::Hand, players: PlayerSet::One(PlayerRef::AbilityController) }, filter: ObjectFilter::Color(ManaColor::Blue), kind: ObjectKind::Card }, choose: Some(Count::Exactly(Expr::Constant(1))) }])))
    );
    let c = card("Gush");
    assert!(
        matches!(casting(&c), CastingRule::AlternativeCost(Cost { parts }) if matches!(parts.as_slice(), [CostPart::Move { destination: Destination::OwnersHand, selections, .. }] if matches!(selections.as_slice(), [CostObjectSelection { choose: Some(Count::Exactly(Expr::Constant(2))), .. }])))
    );
    // A library effect may take fewer cards. This cost must have all three top
    // cards, and cannot pick arbitrary cards from elsewhere in the library.
    let mut c = custom(
        "CastEffect(AdditionalCastingCost(Exile([TheTopNumberCardsOfLibrary(Integer(3))], [])))",
    )
    .unwrap();
    validate::card(&c).unwrap();
    assert!(
        matches!(casting(&c), CastingRule::AdditionalCost { cost: Cost { parts }, .. } if matches!(parts.as_slice(), [CostPart::Move { selections, .. }] if matches!(selections.as_slice(), [CostObjectSelection { objects: ObjectSet::Take { from: SequenceEnd::Top, count: Expr::Constant(3), .. }, choose: Some(Count::Exactly(Expr::Constant(3))) }])))
    );
    let Ability::Casting(CastingRule::AdditionalCost { cost, .. }) = &mut c.abilities[0] else {
        panic!()
    };
    let CostPart::Move { selections, .. } = &mut cost.parts[0] else {
        panic!()
    };
    selections[0].choose = Some(Count::AsManyAsCan(Expr::Constant(3)));
    assert!(validate::card(&c).unwrap_err().contains("payable in full"));
}

#[test]
fn reductions_share_expressions_and_keep_typed_target_queries() {
    let c = card("Blasphemous Act");
    assert!(
        matches!(casting(&c), CastingRule::ReduceCost(reductions) if matches!(reductions.as_slice(), [ManaReduction { color: None, amount: Expr::CountObjects(_) }]))
    );
    let c = card("Academy Journeymage");
    assert!(
        matches!(casting(&c), CastingRule::If { condition: Condition::Controls { player: PlayerRef::AbilityController, .. }, rule } if matches!(rule.as_ref(), CastingRule::ReduceCost(_)))
    );
    let c = card("No One Left Behind");
    assert!(
        matches!(casting(&c), CastingRule::If { condition: Condition::TargetsMatch { object: ObjectRef::Source, kind: ObjectKind::Card, .. }, rule } if matches!(rule.as_ref(), CastingRule::ReduceCost(_)))
    );
    let c = card("Brush Off");
    assert!(
        matches!(casting(&c), CastingRule::If { condition: Condition::TargetsMatch { kind: ObjectKind::Spell, .. }, rule } if matches!(rule.as_ref(), CastingRule::ReduceCost(reductions) if reductions.len() == 2))
    );
    let c = custom("CastEffect(ReduceCastingCost([CostReduceU]))").unwrap();
    assert!(
        matches!(casting(&c), CastingRule::ReduceCost(r) if matches!(r.as_slice(), [ManaReduction { color: Some(ManaColor::Blue), amount: Expr::Constant(1) }]))
    );
    assert!(custom("CastEffect(ReduceCastingCost([CostReduceGeneric(-1)]))").is_err());
}

#[test]
fn target_multipliers_timing_and_mana_provenance_stay_casting_metadata() {
    let c = card("Aerial Formation");
    assert!(matches!(
        casting(&c),
        CastingRule::AdditionalCost {
            repetition: CostRepetition::TargetsBeyond(1),
            ..
        }
    ));
    let c = card("Phyrexian Purge");
    assert!(
        matches!(casting(&c), CastingRule::AdditionalCost { repetition: CostRepetition::TargetsBeyond(0), cost: Cost { parts }, .. } if matches!(parts.as_slice(), [CostPart::Life(Expr::Constant(3))]))
    );
    let c = card("Silver Scrutiny");
    assert!(
        matches!(casting(&c), CastingRule::If { condition: Condition::Compare { left: Expr::AnnouncedX, comparison: Comparison::LessOrEqual, right: Expr::Constant(3) }, rule } if matches!(rule.as_ref(), CastingRule::Flash { additional_cost: None }))
    );
    let c = card("Imperiosaur");
    assert!(matches!(
        casting(&c),
        CastingRule::RestrictMana {
            portion: ManaCostPortion::All,
            filter: ManaPaymentFilter::ProducedBy(ObjectFilter::All(_))
        }
    ));
    let c = card("Consume Spirit");
    assert!(
        matches!(casting(&c), CastingRule::RestrictMana { portion: ManaCostPortion::X, filter: ManaPaymentFilter::Colors(colors) } if matches!(colors.as_slice(), [ManaColor::Black]))
    );
    let c = card("Dream Leash");
    assert!(
        matches!(casting(&c), CastingRule::ProhibitTargets { domain, filter: ObjectFilter::Not(_) } if *domain == ObjectDomain::permanent())
    );
}

#[test]
fn casting_requirements_use_the_proposal_and_cannot_read_resolution_payments() {
    let c = custom("CastEffect(CantBeCastFromAnywhereOtherThanGraveyard)").unwrap();
    assert!(matches!(
        casting(&c),
        CastingRule::Require(Condition::WasCastFrom {
            object: ObjectRef::Source,
            zone: EventZone::Graveyard
        })
    ));
    let c = custom("CastEffect(CantBeCastUnless(IsPlayersTurn(You)))").unwrap();
    assert!(matches!(
        casting(&c),
        CastingRule::Require(Condition::PlayerIsActive(PlayerRef::AbilityController))
    ));
    let c = custom("CastEffect(CantBeCastIf(IsPlayersTurn(You)))").unwrap();
    assert!(matches!(
        casting(&c),
        CastingRule::Require(Condition::Not(_))
    ));
    let mut c =
        custom("CastEffect(OptionalAdditionalCastingCost(PayMana([ManaCostGeneric(2)])))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        casting(&c),
        CastingRule::AdditionalCost { optional: true, .. }
    ));
    c.abilities[0] = Ability::Casting(CastingRule::Require(Condition::PaymentChosen(ResultId(0))));
    assert!(validate::card(&c).unwrap_err().contains("payment decision"));
    c.abilities[0] = Ability::Casting(CastingRule::AlternativeCost(Cost {
        parts: vec![CostPart::Choice(vec![])],
    }));
    assert!(validate::card(&c).unwrap_err().contains("no alternatives"));
    assert!(custom("CastEffect(MayCastAsThoughItHadFlashWithSpecialAction([DrawACard]))").is_ok());
}

#[test]
fn mana_classification_follows_cost_choices_and_library_movement() {
    use semantics::{activation_kind, ActivationKind};
    let c = card("Llanowar Elves");
    let Ability::Activated {
        body, announcement, ..
    } = &c.abilities[0]
    else {
        panic!()
    };
    let c = custom(
        "CastEffect(AdditionalCastingCost(Exile([TheTopNumberCardsOfLibrary(Integer(3))], [])))",
    )
    .unwrap();
    let CastingRule::AdditionalCost { cost, .. } = casting(&c) else {
        panic!()
    };
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Stack
    );
    let cost = Cost {
        parts: vec![CostPart::Choice(vec![
            Cost {
                parts: vec![CostPart::Mana(vec![])],
            },
            cost.clone(),
        ])],
    };
    assert_eq!(
        activation_kind(&cost, announcement, body),
        ActivationKind::Stack
    );
    let c = card("Force of Will");
    let CastingRule::AlternativeCost(cost) = casting(&c) else {
        panic!()
    };
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Mana
    );
}
