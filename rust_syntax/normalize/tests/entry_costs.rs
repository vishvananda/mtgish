use mtgish_ast::{
    corpus,
    ir::*,
    lower, mtg_types as source,
    semantics::{activation_kind, ActivationKind},
    validate,
};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/entry_costs.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
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
fn program(rule: &str) -> Result<Card, Vec<lower::Diagnostic>> {
    let mut c = source("Urborg Volcano");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}

#[test]
fn complete_entry_and_cost_cards_roundtrip_and_unknown_nested_rules_stay_incomplete() {
    let incomplete = ["Aeve, Progenitor Ooze", "Hooded Hydra", "Sage of Hours"];
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
    assert_eq!(complete, 41);
}

#[test]
fn entry_modifications_are_pending_changes_and_global_replacements_do_not_apply_to_their_own_entry()
{
    let c = card("Tranquil Cove");
    assert!(
        matches!(&c.abilities[0],Ability::Replacement {active:ReplacementActive::SourceEntering,event:ReplacementEvent::Enter {objects:ObjectFilter::SameObject(ObjectRef::Source)},body}
        if matches!(body.instructions.as_slice(),[Instruction::Effect {operation:Operation::ModifyEvent {change:EventChange::SetTapped(true)},..}]))
    );
    assert!(matches!(
        &c.abilities[1],
        Ability::Triggered {
            event: EventSpec::ZoneChange {
                to: EventZone::Battlefield,
                ..
            },
            ..
        }
    ));
    let c = card("Orb of Dreams");
    assert!(matches!(
        &c.abilities[0],
        Ability::Replacement {
            active: ReplacementActive::Battlefield,
            event: ReplacementEvent::Enter {
                objects: ObjectFilter::Any
            },
            ..
        }
    ));
    let c = card("Vivid Creek");
    assert!(
        matches!(&c.abilities[0],Ability::Replacement {body,..} if matches!(body.instructions.as_slice(),[
            Instruction::Effect {operation:Operation::ModifyEvent {change:EventChange::SetTapped(true)},..},
            Instruction::Effect {operation:Operation::ModifyEvent {change:EventChange::AddCounters {amount:Expr::Constant(2),..}},..}
        ]))
    );
}

#[test]
fn entry_x_comes_from_the_cast_and_counter_queries_keep_last_known_information() {
    let counted=program("Activated(SacrificePermanent(ThisPermanent), ActionList([DrawNumberCards(TheNumberOfPermanentsOnTheBattlefield(SinglePermanent(ThisPermanent)))]))").unwrap();
    validate::card(&counted).unwrap();
    let Ability::Activated { body, .. } = &counted.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::Draw {amount:Expr::CountObjects(objects),..},..}
        if matches!(objects.as_ref(),ObjectSet::InZone {zone:EventZone::Battlefield,objects}
            if matches!(objects.as_ref(),ObjectSet::One(ObjectRef::Source))))
    );
    let c = card("Hangarback Walker");
    assert!(
        matches!(&c.abilities[0],Ability::Replacement {body,..} if matches!(body.instructions.as_slice(),[
            Instruction::Effect {operation:Operation::ModifyEvent {change:EventChange::AddCounters {amount:Expr::CastX(ObjectRef::EventSubject),..}},..}
        ]))
    );
    let Ability::Triggered { body, .. } = &c.abilities[1] else {
        panic!()
    };
    assert!(
        matches!(body.instructions.as_slice(),[Instruction::Effect {operation:Operation::CreateTokens {tokens,..},..}]
        if matches!(&tokens[0].amount,Expr::CountCounters {recipients,information:Information::CurrentOrLastKnown,..}
            if matches!(recipients.as_ref(),Recipients::Objects(ObjectSet::One(ObjectRef::EventSubject)))))
    );
}

#[test]
fn shock_land_payment_uses_the_prospective_controller_and_a_payment_decision() {
    let c = card("Hallowed Fountain");
    let [Ability::Replacement { body, .. }] = c.abilities.as_slice() else {
        panic!()
    };
    let [Instruction::Effect {
        result: ResultId(payment),
        operation:
            Operation::PayCost {
                player,
                cost,
                optional: true,
            },
    }, Instruction::If {
        condition: Condition::Not(condition),
        then,
        otherwise,
    }] = body.instructions.as_slice()
    else {
        panic!()
    };
    assert!(matches!(player,PlayerRef::ControllerOf(p) if matches!(**p,ObjectRef::EventSubject)));
    assert!(matches!(
        cost.parts.as_slice(),
        [CostPart::Life(Expr::Constant(2))]
    ));
    assert!(matches!(condition.as_ref(),Condition::PaymentChosen(ResultId(id)) if id==payment));
    assert!(otherwise.instructions.is_empty());
    assert!(matches!(
        then.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::ModifyEvent {
                change: EventChange::SetTapped(true)
            },
            ..
        }]
    ));
}

#[test]
fn resolving_draw_and_discard_costs_remain_costs_with_a_separate_followup() {
    let c = card("Academy Wall");
    assert!(
        matches!(&c.abilities[1],Ability::Triggered {limits:TriggerLimits {per_turn:Some(1),..},body,..}
        if matches!(body.instructions.as_slice(),[
            Instruction::Effect {operation:Operation::PayCost {cost:Cost {parts},optional:true,..},..},
            Instruction::If {condition:Condition::PaymentChosen(_),then,..}
        ] if matches!(parts.as_slice(),[CostPart::Draw {count:Expr::Constant(1)}]) && matches!(then.instructions.as_slice(),[
            Instruction::Effect {operation:Operation::Discard {amount:Expr::Constant(1),random:false,..},..}
        ])))
    );
    let c = card("Akki Ronin");
    let Ability::Triggered { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::PayCost {cost:Cost {parts},optional:true,..},..}
        if matches!(parts.as_slice(),[CostPart::Discard {count:Expr::Constant(1), ..}]))
    );
}

#[test]
fn counter_costs_and_state_triggers_keep_their_source_without_inventing_an_event() {
    let c = card("Gemstone Mine");
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &c.abilities[1]
    else {
        panic!()
    };
    assert!(
        matches!(&cost.parts[1],CostPart::RemoveCounters {removals} if matches!(removals.as_slice(),[
            CounterRemoval {recipients:Recipients::Objects(ObjectSet::One(ObjectRef::Source)),amount:CounterAmount::Count(Count::Exactly(Expr::Constant(1))),..}
        ]))
    );
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Mana
    );
    let c = card("Dark Depths");
    let Ability::Triggered { event, body, .. } = &c.abilities[2] else {
        panic!()
    };
    assert!(matches!(
        event,
        EventSpec::State {
            condition: Condition::ObjectMatches {
                object: ObjectRef::Source,
                filter: ObjectFilter::Counters {
                    comparison: Comparison::Equal,
                    value: Expr::Constant(0),
                    ..
                }
            }
        }
    ));
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::PayCost {cost:Cost {parts},optional:false,..},..}
        if matches!(parts.as_slice(),[CostPart::Sacrifice {objects:ObjectSet::One(ObjectRef::Source),..}]))
    );
    assert!(
        matches!(&body.instructions[1],Instruction::If {condition:Condition::PaymentChosen(_),then,..}
        if matches!(then.instructions.as_slice(),[Instruction::Effect {operation:Operation::CreateTokens {tokens,..},..}]
            if matches!(&tokens[0].definition().unwrap().name,TokenName::Explicit(n) if n=="Marit Lage")))
    );
}

#[test]
fn payment_results_do_not_escape_optional_or_conditional_scopes() {
    for rule in [
        "Activated(PayMana([]), ActionList([MayActions([MayCost(PayLife(Integer(1)))]), If(CostWasPaid, [DrawACard])]))",
        "Activated(PayMana([]), ActionList([MayCost(PayLife(Integer(1))), If(PermanentPassesFilter(ThisPermanent, IsTapped), [MayCost(PayLife(Integer(2)))]), If(CostWasPaid, [DrawACard])]))",
    ] {
        let errors=program(rule).unwrap_err();
        assert!(errors.iter().any(|e|e.message.contains("payment reference")),"{errors:?}");
    }
    // The unchanged branch still sees the parent decision, even if the other
    // branch makes a new payment. The outer program cannot join those results.
    let c=program("Activated(PayMana([]), ActionList([MayCost(PayLife(Integer(1))), IfElse(PermanentPassesFilter(ThisPermanent, IsTapped), [MayCost(PayLife(Integer(2)))], [If(CostWasPaid, [DrawACard])])]))").unwrap();
    validate::card(&c).unwrap();
    let Ability::Activated { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[1],Instruction::If {otherwise,..} if matches!(otherwise.instructions.as_slice(),[
            Instruction::If {condition:Condition::PaymentChosen(ResultId(0)),..}
        ]))
    );
}

#[test]
fn payment_decisions_require_payment_results_and_counter_allocation_is_not_approximated() {
    let mut c = card("Academy Wall");
    let Ability::Triggered { body, .. } = &mut c.abilities[1] else {
        panic!()
    };
    let Instruction::Effect { operation, .. } = &mut body.instructions[0] else {
        panic!()
    };
    *operation = Operation::Draw {
        players: PlayerSet::One(PlayerRef::AbilityController),
        amount: Expr::Constant(1),
    };
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("not an available payment decision"));
    let errors=program("Activated(RemoveCounters([NumberCountersOfTypeFromAmongPermanents(Integer(2), ChargeCounter, AnyPermanent)]), ActionList([DrawACard]))").unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.form == "NumberCountersOfTypeFromAmongPermanents"));
    let c = program(
        "ReplaceWouldEnter(PermanentWouldEnterTheBattlefield(ThisPermanent), [EntersTapped])",
    )
    .unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        &c.abilities[0],
        Ability::Replacement {
            active: ReplacementActive::SourceEntering,
            ..
        }
    ));
}

#[test]
fn player_predicate_bindings_and_replacement_changes_are_scoped() {
    let mut c = card("Abandoned Campground");
    let Ability::Replacement { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::If {
        condition: Condition::Not(condition),
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(condition.as_ref(),Condition::ExistsPlayer {players:PlayerSet::All,binding:Binding(0),condition}
        if matches!(condition.as_ref(),Condition::Compare {left:Expr::PlayerLife(PlayerRef::Bound(Binding(0))),comparison:Comparison::LessOrEqual,right:Expr::Constant(13)}))
    );
    body.instructions.push(Instruction::Effect {
        result: ResultId(900),
        operation: Operation::PayCost {
            player: PlayerRef::Bound(Binding(0)),
            cost: Cost { parts: vec![] },
            optional: true,
        },
    });
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("binding has no declaration"));

    let mut c = card("Vivid Creek");
    let Ability::Replacement { body, .. } = &c.abilities[0] else {
        panic!()
    };
    c.abilities = vec![Ability::Spell {
        announcement: Announcement::empty(),
        body: body.clone(),
    }];
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("outside a replacement"));
}

#[test]
fn counter_removal_choice_belongs_to_the_payer_and_drawing_costs_disqualify_mana_abilities() {
    let c=program("Activated(PayMana([]), ActionList([PlayerAction(TheActivePlayer, RemoveCounters([ACounterFromPermanent(ThisPermanent)]))]))").unwrap();
    validate::card(&c).unwrap();
    let Ability::Activated { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::RemoveCounters {removals},..}
        if matches!(removals.as_slice(),[CounterRemoval {player:PlayerRef::Active,kind:None,selection:None,..}]))
    );
    let c=program("Activated(PayMana([]), ActionList([PlayerMayCost(ControllerOfPermanent(ThisPermanent), RemoveCounters([ACounterOfTypeFromAPermanent(ChargeCounter, AnyPermanent)]))]))").unwrap();
    validate::card(&c).unwrap();
    let Ability::Activated { body, .. } = &c.abilities[0] else {
        panic!()
    };
    assert!(
        matches!(&body.instructions[0],Instruction::Effect {operation:Operation::PayCost {player:PlayerRef::ControllerOf(_),cost:Cost {parts},..},..}
        if matches!(parts.as_slice(),[CostPart::RemoveCounters {removals}] if matches!(&removals[0].selection,Some(SelectionChoice {player:PlayerRef::ControllerOf(_),..}))))
    );
    let c=program("Activated(TapPermanent(ThisPermanent), ActionList([MayCost(DrawACard), AddMana(ManaProduceG)]))").unwrap();
    validate::card(&c).unwrap();
    let Ability::Activated {
        cost,
        announcement,
        body,
        ..
    } = &c.abilities[0]
    else {
        panic!()
    };
    assert_eq!(
        activation_kind(cost, announcement, body),
        ActivationKind::Stack
    );
}
