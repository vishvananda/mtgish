use mtgish_ast::{corpus, ir::*, mtg_types as source, pipeline, validate};
fn custom(rules: &[&str]) -> source::OracleCard {
    let mut card: source::OracleCard = ron::from_str(
        include_str!("fixtures/pipeline.ronl")
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let source::OracleCard::Card { rules: target, .. } = &mut card else {
        panic!()
    };
    *target = Some(rules.iter().map(|r| ron::from_str(r).unwrap()).collect());
    card
}
fn normalized(rule: &str) -> Card {
    pipeline::finalize(&custom(&[rule])).unwrap()
}
fn json(card: &Card) -> serde_json::Value {
    serde_json::to_value(card).unwrap()
}
fn roundtrip(card: &Card) {
    let bytes = bincode::encode_to_vec(card, bincode::config::standard()).unwrap();
    let (binary, used): (Card, usize) =
        bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
    assert_eq!(used, bytes.len());
    for loaded in [
        binary,
        ron::from_str(&ron::ser::to_string(card).unwrap()).unwrap(),
        serde_json::from_value(json(card)).unwrap(),
    ] {
        validate::card(&loaded).unwrap();
        assert_eq!(json(&loaded), json(card));
    }
}
fn histories(value: &serde_json::Value) -> Vec<&serde_json::Value> {
    let mut out = vec![];
    fn walk<'a>(v: &'a serde_json::Value, out: &mut Vec<&'a serde_json::Value>) {
        if v["kind"] == "CostObjects" {
            out.push(&v["args"]);
        }
        match v {
            serde_json::Value::Object(v) => {
                for x in v.values() {
                    walk(x, out)
                }
            }
            serde_json::Value::Array(v) => {
                for x in v {
                    walk(x, out)
                }
            }
            _ => {}
        }
    }
    walk(value, &mut out);
    out
}
#[test]
fn activation_properties_keep_the_exact_component_and_original_identity_view() {
    let source=custom(&["Activated(And([PayMana([ManaCostGeneric(1)]),TapPermanent(ThisPermanent),SacrificeAPermanent(IsCardtype(Creature))]),ActionList([GainLife(PowerOfTheSacrificedCreature)]))"]);
    let card = pipeline::finalize(&source).unwrap();
    let value = json(&card);
    let hs = histories(&value);
    assert_eq!(hs.len(), 1);
    assert_eq!(hs[0]["payment"], "Activation");
    assert_eq!(hs[0]["component"], serde_json::json!([2]));
    assert_eq!(hs[0]["phase"], "Before");
    assert_eq!(hs[0]["view"], "Inputs");
    let stage = pipeline::Intermediate::new(&source).unwrap();
    assert_eq!(json(&stage.final_card.unwrap()), value);
    roundtrip(&card);
}
#[test]
fn casting_history_names_the_specific_declaration_and_is_available_only_to_the_spell() {
    let source = custom(&[
        "Flash",
        "CastEffect(AdditionalCastingCost(SacrificeAPermanent(IsCardtype(Creature))))",
        "SpellActions(ActionList([GainLife(ToughnessOfTheSacrificedCreature)]))",
    ]);
    let card = pipeline::finalize(&source).unwrap();
    let value = json(&card);
    let hs = histories(&value);
    assert_eq!(
        hs[0]["payment"],
        serde_json::json!({"Casting":{"ability":1}})
    );
    roundtrip(&card);
    for other in ["Activated(PayMana([ManaCostGeneric(1)]),ActionList([GainLife(PowerOfTheSacrificedCreature)]))","TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)),ActionList([GainLife(PowerOfTheSacrificedCreature)]))"] {
        assert!(pipeline::finalize(&custom(&["CastEffect(AdditionalCastingCost(SacrificeAPermanent(IsCardtype(Creature))))",other])).is_err());
    }
}
#[test]
fn optional_casting_cost_counts_performed_objects_separately_from_designated_inputs() {
    let card=pipeline::finalize(&custom(&["CastEffect(OptionalAdditionalCastingCost(SacrificeAnyNumberOfPermanents(IsCardtype(Creature))))","SpellActions(ActionList([GainLife(TheNumberOfPermanentsSacrificedThisWay)]))"])).unwrap();
    let value = json(&card);
    let hs = histories(&value);
    assert_eq!(hs[0]["view"], "Performed");
    assert_eq!(hs[0]["component"], serde_json::json!([0]));
    roundtrip(&card);
    // A singular reference cannot silently index an any-number payment.
    assert!(pipeline::finalize(&custom(&["Activated(SacrificeAnyNumberOfPermanents(IsCardtype(Creature)),ActionList([GainLife(PowerOfTheSacrificedCreature)]))"])).is_err());
}
#[test]
fn resolution_payment_history_uses_the_payment_result_and_preserves_the_payer() {
    let card=normalized("SpellActions(Targeted([TargetPlayer(AnyPlayer)],ActionList([PlayerMayCost(Ref_TargetPlayer,SacrificeAPermanent(IsCardtype(Creature))),If(CostWasPaid,[GainLife(PowerOfTheSacrificedCreature)])])))");
    let value = json(&card);
    let hs = histories(&value);
    assert_eq!(hs[0]["payment"], serde_json::json!({"Resolution":0}));
    let Ability::Spell { body, .. } = &card.abilities[0] else {
        panic!()
    };
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::PayCost {
                player: PlayerRef::Target { slot: Slot(0), .. },
                optional: true,
                ..
            },
            ..
        }
    ));
    roundtrip(&card);
}
#[test]
fn branch_local_payments_do_not_escape_or_rebind_a_sibling() {
    for rule in [
        "SpellActions(ActionList([MayActions([MayCost(SacrificeAPermanent(IsCardtype(Creature)))]),GainLife(PowerOfTheSacrificedCreature)]))",
        "SpellActions(ActionList([IfElse(IsPlayersTurn(You),[MayCost(SacrificeAPermanent(IsCardtype(Creature)))],[GainLife(PowerOfTheSacrificedCreature)])]))",
        "Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([MayActions([MayCost(SacrificeAPermanent(IsCardtype(Creature)))]),GainLife(PowerOfTheSacrificedCreature)]))",
    ] {assert!(pipeline::finalize(&custom(&[rule])).is_err(),"{rule}");}
    let card=normalized("SpellActions(ActionList([MayActions([MayCost(SacrificeAPermanent(IsCardtype(Creature))),If(CostWasPaid,[GainLife(PowerOfTheSacrificedCreature)])])]))");
    roundtrip(&card);
}
#[test]
fn modal_resolution_inherits_the_activation_payment_but_granted_abilities_do_not() {
    let card=normalized("Activated(SacrificeAPermanent(IsCardtype(Creature)),Modal_ChooseOne([ActionList([GainLife(PowerOfTheSacrificedCreature)]),ActionList([DrawNumberCards(ToughnessOfTheSacrificedCreature)])]))");
    roundtrip(&card);
    assert_eq!(histories(&json(&card)).len(), 2);
    let unbound="Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([CreatePermanentLayerEffectUntil(ThisPermanent,[AddAbility([Activated(PayMana([ManaCostGeneric(1)]),ActionList([GainLife(PowerOfTheSacrificedCreature)]))])],UntilEndOfTurn)]))";
    assert!(pipeline::finalize(&custom(&[unbound])).is_err());
    assert!(pipeline::finalize(&custom(&["Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([DrawACard]))","Activated(PayMana([ManaCostGeneric(1)]),ActionList([GainLife(PowerOfTheSacrificedCreature)]))"])).is_err());
}
#[test]
fn ambiguous_components_and_cost_declarations_are_diagnosed_without_an_arbitrary_choice() {
    for cost in ["And([SacrificeAPermanent(IsCardtype(Creature)),SacrificeAPermanent(IsCardtype(Artifact))])","Or([SacrificeAPermanent(IsCardtype(Creature)),SacrificeAPermanent(IsCardtype(Artifact))])"] {
        let source=custom(&[&format!("Activated({cost},ActionList([GainLife(PowerOfTheSacrificedCreature)]))")]);
        assert!(pipeline::finalize(&source).unwrap_err().iter().any(|e|e.message.contains("unique component")));
    }
    let source = custom(&[
        "CastEffect(AdditionalCastingCost(SacrificeAPermanent(IsCardtype(Creature))))",
        "CastEffect(AdditionalCastingCost(SacrificeAPermanent(IsCardtype(Artifact))))",
        "SpellActions(ActionList([GainLife(PowerOfTheSacrificedCreature)]))",
    ]);
    assert!(pipeline::finalize(&source).is_err());
}
#[test]
fn target_constraints_can_name_planned_cost_inputs_under_rule_601_5() {
    let rule="Activated(SacrificeAPermanent(IsCardtype(Creature)),Targeted([TargetPermanent(And([IsCardtype(Creature),PowerIs(LessThan(PowerOfTheSacrificedCreature))]))],ActionList([TapPermanent(Ref_TargetPermanent)])))";
    let card = pipeline::finalize(&custom(&[rule])).unwrap();
    roundtrip(&card);
    let value = json(&card);
    assert!(histories(&value).iter().all(|h| h["view"] == "Inputs"));
}
#[test]
fn cost_paths_cross_alternatives_and_transparent_group_constraints_without_losing_identity() {
    let card=normalized("Activated(Or([SacrificeAPermanent(IsCardtype(Creature)),PayMana([ManaCostGeneric(3)])]),ActionList([GainLife(PowerOfTheSacrificedCreature)]))");
    let value = json(&card);
    assert_eq!(
        histories(&value)[0]["component"],
        serde_json::json!([0, 0, 0])
    );
    roundtrip(&card);
    let card=normalized("Activated(SacrificeNumberGroupPermanents(Integer(1),IsCardtype(Creature),DifferentNames),ActionList([GainLife(PowerOfTheSacrificedCreature)]))");
    let value = json(&card);
    assert_eq!(histories(&value)[0]["component"], serde_json::json!([0]));
    roundtrip(&card);
}
#[test]
fn native_history_validation_rejects_forged_namespaces_paths_views_and_plural_indexing() {
    let base=normalized("Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([GainLife(PowerOfTheSacrificedCreature)]))");
    for case in 0..8 {
        let mut card = base.clone();
        let Ability::Activated { cost, body, .. } = &mut card.abilities[0] else {
            panic!()
        };
        let Instruction::Effect {
            operation:
                Operation::GainLife {
                    amount:
                        Expr::ObjectProperty {
                            object: ObjectRef::Query { objects, index },
                            ..
                        },
                    ..
                },
            ..
        } = &mut body.instructions[0]
        else {
            panic!()
        };
        let ObjectSet::CostObjects {
            payment,
            component,
            view,
            phase,
        } = objects.as_mut()
        else {
            panic!()
        };
        match case {
            0 => *payment = PaymentRef::Resolution(ResultId(0)),
            1 => *payment = PaymentRef::Casting { ability: 0 },
            2 => component.push(1),
            3 => component.clear(),
            4 => *phase = ObjectPhase::After,
            5 => *index = 1,
            7 => *view = CostObjectView::Performed,
            _ => {
                let CostPart::Sacrifice { choose, .. } = &mut cost.parts[0] else {
                    panic!()
                };
                *choose = Some(Count::Exactly(Expr::Constant(2)));
            }
        }
        assert!(case == 7 || matches!(view, CostObjectView::Inputs));
        assert!(validate::card(&card).is_err(), "case {case}");
    }
}
#[test]
fn numeric_history_recipes_preserve_field_accounting_and_saved_artifact_checks() {
    let original = serde_json::to_value(pipeline::numbers::registry()).unwrap();
    for source in ["PowerOfTheSacrificedCreature", "PowerOfPermanent"] {
        let mut value = original.clone();
        let row = value["rules"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["source"] == source)
            .unwrap();
        if source == "PowerOfPermanent" {
            row["expression"]["object"]
                .as_object_mut()
                .unwrap()
                .remove("argument");
        } else {
            row["expression"]["object"]["argument"] = 0.into();
        }
        assert!(mtgish_ast::number_spec::Registry::parse(&value.to_string()).is_err());
    }
    let source=custom(&["Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([GainLife(PowerOfTheSacrificedCreature)]))"]);
    let stage = pipeline::Intermediate::new(&source).unwrap();
    let mut changed = stage.structural.clone();
    let rewrite = changed
        .numbers
        .iter_mut()
        .find(|n| n.rule == "number.PowerOfTheSacrificedCreature")
        .unwrap();
    rewrite.expression = pipeline::Quantity::Property {
        object: pipeline::NumberObject::Payment(mtgish_ast::number_spec::PaymentAction::Discard),
        property: Property::Power,
    };
    assert!(pipeline::bind(&changed).is_err());
}
#[test]
fn real_payment_history_cards_roundtrip_and_keep_independent_gaps() {
    let mut complete = 0;
    let incomplete = [
        "Devouring Greed",
        "Disciple of Bolas",
        "Endemic Plague",
        "Rite of Consumption",
        "Ruthless Technomancer",
        "The Ever-Changing 'Dane",
    ];
    for line in include_str!("fixtures/paid.ronl").lines() {
        let source: source::OracleCard = ron::from_str(line).unwrap();
        let name = corpus::Identity::of(&source).unwrap().names.remove(0);
        match pipeline::finalize(&source) {
            Ok(card) => {
                assert!(!incomplete.contains(&name.as_str()), "{name}");
                roundtrip(&card);
                complete += 1;
                println!("complete: {name}");
            }
            Err(errors) => {
                assert!(incomplete.contains(&name.as_str()), "{name}: {errors:?}");
            }
        }
    }
    assert_eq!(complete, 17);
}

#[test]
fn later_ordinary_actions_shadow_payment_shorthand() {
    for rule in [
        "Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([SacrificeAPermanent(IsCardtype(Creature)),GainLife(PowerOfTheSacrificedCreature)]))",
        "Activated(DiscardACard,ActionList([DiscardACard,GainLife(ManaValueOfTheDiscardedCard)]))",
    ] {assert!(pipeline::finalize(&custom(&[rule])).is_err(),"{rule}");}
}

#[test]
fn delayed_closures_capture_payment_namespaces_without_creating_a_new_payment() {
    for rule in [
        "Activated(SacrificeAPermanent(IsCardtype(Creature)),ActionList([CreateFutureTrigger(AtTheBeginningOfTheNextEndStep,ActionList([GainLife(PowerOfTheSacrificedCreature)]))]))",
        "SpellActions(ActionList([MayCost(SacrificeAPermanent(IsCardtype(Creature))),If(CostWasPaid,[CreateFutureTrigger(AtTheBeginningOfTheNextEndStep,ActionList([GainLife(PowerOfTheSacrificedCreature)]))])]))",
    ] {
        let card=normalized(rule);assert_eq!(histories(&json(&card)).len(),1);roundtrip(&card);
    }
}

#[test]
fn fixed_cost_subjects_can_be_named_in_target_filters_without_reading_a_future_history() {
    let card=normalized("Activated(TapPermanent(HostPermanent),Targeted([TargetPermanent(Other(ThePermanentTappedThisWay))],ActionList([GainLife(PowerOfPermanent(ThePermanentTappedThisWay))])))");
    let Ability::Activated {
        announcement, body, ..
    } = &card.abilities[0]
    else {
        panic!()
    };
    // The same cost reference resolves the known selector before payment and
    // the recorded identity afterward, including at target revalidation.
    assert_eq!(
        histories(&serde_json::to_value(announcement).unwrap()).len(),
        1
    );
    let value = serde_json::to_value(body).unwrap();
    assert_eq!(histories(&value).len(), 1);
    roundtrip(&card);
}

#[test]
fn cost_history_appends_its_binary_tag_without_reordering_existing_object_sets() {
    let config = bincode::config::standard();
    assert_eq!(
        bincode::encode_to_vec(ObjectSet::Observed(ResultId(7)), config).unwrap(),
        vec![0, 7]
    );
    assert_eq!(
        bincode::encode_to_vec(ObjectSet::Created(ResultId(7)), config).unwrap(),
        vec![13, 7]
    );
    let history = ObjectSet::CostObjects {
        payment: PaymentRef::Activation,
        component: vec![0],
        view: CostObjectView::Inputs,
        phase: ObjectPhase::Before,
    };
    assert_eq!(bincode::encode_to_vec(history, config).unwrap()[0], 14);
}
