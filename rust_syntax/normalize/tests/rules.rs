use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/rules.ronl")
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
fn program(rule: &str) -> Result<Card, Vec<lower::Diagnostic>> {
    let mut c = source("Pacifism");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn changes(c: &Card, index: usize) -> &[Change] {
    let Ability::Static { changes, .. } = &c.abilities[index] else {
        panic!()
    };
    changes
}

#[test]
fn real_rule_families_roundtrip_in_rust_and_missing_nested_forms_remain_diagnostics() {
    let mut complete = 0;
    for source in sources() {
        let name = corpus::Identity::of(&source).unwrap().names.remove(0);
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
    assert_eq!(complete, 57);
}

#[test]
fn restrictions_and_combat_exceptions_do_not_grant_or_remove_keyword_abilities() {
    let c = card("Pacifism");
    assert!(matches!(
        changes(&c, 1),
        [
            Change::Rule(RuleChange::Prohibit(ObjectAction::Attack(
                AttackDefenders::Any
            ))),
            Change::Rule(RuleChange::Prohibit(ObjectAction::Block(ObjectFilter::Any)))
        ]
    ));
    let c = card("Guard Duty");
    assert!(matches!(
        changes(&c, 1),
        [Change::GrantKeyword(Keyword::Defender)]
    ));
    let c = card("Animate Wall");
    assert!(matches!(
        changes(&c, 1),
        [Change::Rule(RuleChange::AttackException {
            ignore: AttackCheck::Defender,
            ..
        })]
    ));
    let c = card("Arrest");
    assert!(matches!(
        changes(&c, 1),
        [
            _,
            _,
            Change::Rule(RuleChange::Prohibit(ObjectAction::Activate(
                ActivationFilter::Any
            )))
        ]
    ));
    let c = card("Bonds of Faith");
    assert!(
        matches!(&c.abilities[1],Ability::Conditional{condition:Condition::ObjectMatches{object:ObjectRef::AttachedTo(_),..},then,otherwise}
        if matches!(then.as_slice(),[Ability::Static{changes,..}]if matches!(changes.as_slice(),[Change::Pump{..}]))
        && matches!(otherwise.as_slice(),[Ability::Static{changes,..}]if matches!(changes.as_slice(),[Change::Rule(_),Change::Rule(_)])))
    );
}

#[test]
fn resolving_rule_effects_retain_subject_queries_while_characteristic_changes_snapshot() {
    let mut falter = card("Falter");
    let Ability::Spell { body, .. } = &mut falter.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                affected,
                evaluation,
                changes,
                ..
            },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        affected,
        EffectSubjects::Objects(ObjectSet::Battlefield(_))
    ));
    assert!(matches!(evaluation, EffectEvaluation::LiveObjects));
    assert!(matches!(
        changes.as_slice(),
        [Change::Rule(RuleChange::Prohibit(ObjectAction::Block(_)))]
    ));
    *evaluation = EffectEvaluation::Snapshot;
    assert!(validate::card(&falter)
        .unwrap_err()
        .contains("live subject queries"));
    let Ability::Spell { body, .. } = &mut falter.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                evaluation,
                changes,
                ..
            },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *evaluation = EffectEvaluation::LiveObjects;
    changes.push(Change::Pump {
        power: Expr::Constant(1),
        toughness: Expr::Constant(0),
    });
    assert!(validate::card(&falter)
        .unwrap_err()
        .contains("independently evaluated parts"));
    let c = card("Trumpet Blast");
    assert!(
        matches!(&c.abilities[0],Ability::Spell{body,..}if matches!(body.instructions.as_slice(),[
            Instruction::Effect{operation:Operation::CreateContinuousEffect{evaluation:EffectEvaluation::Snapshot,..},..}
        ]))
    );
}

#[test]
fn declaration_costs_belong_to_the_declaring_player_and_remain_separate_from_requirements() {
    let c = card("Ghostly Prison");
    assert!(matches!(changes(&c,0),[Change::Rule(RuleChange::CombatCost{
        action:ObjectAction::Attack(AttackDefenders::Players(PlayerSet::One(PlayerRef::AbilityController))),payer:PlayerRef::DeclaringPlayer,cost:Cost{parts}
    })]if matches!(parts.as_slice(),[CostPart::Mana(_)])));
    let c = card("Awesome Presence");
    assert!(matches!(
        changes(&c, 1),
        [Change::Rule(RuleChange::CombatCost {
            action: ObjectAction::BeBlocked(_),
            payer: PlayerRef::DeclaringPlayer,
            ..
        })]
    ));
    let c = card("Courtly Provocateur");
    assert!(
        matches!(&c.abilities[0],Ability::Activated{body,..}if matches!(body.instructions.as_slice(),[
        Instruction::Effect{operation:Operation::CreateContinuousEffect{changes,..},..}
    ]if matches!(changes.as_slice(),[Change::Rule(RuleChange::RequireCombat{action:ObjectAction::Attack(_),amount:RequirementAmount::AtLeast(Expr::Constant(1))})])))
    );
    let c = card("Lure");
    assert!(matches!(
        changes(&c, 1),
        [Change::Rule(RuleChange::RequireCombat {
            action: ObjectAction::BeBlocked(_),
            amount: RequirementAmount::Each
        })]
    ));
    let c = card("Blazing Archon");
    assert!(matches!(
        changes(&c, 1),
        [Change::Rule(RuleChange::Prohibit(ObjectAction::Attack(
            AttackDefenders::Players(_)
        )))]
    ));
    let c = card("Vow of Duty");
    assert!(c.abilities.iter().any(|a|matches!(a,Ability::Static{changes,..}if matches!(changes.as_slice(),[
        Change::Rule(RuleChange::Prohibit(ObjectAction::Attack(AttackDefenders::PlayersAndPlaneswalkers(_))))
    ]))));
}

#[test]
fn combat_groups_use_proposed_participation_without_forcing_an_attack_or_block() {
    let c = card("Dueling Grounds");
    assert!(matches!(
        changes(&c, 0),
        [Change::Rule(RuleChange::CombatGroup {
            role: CombatRole::Attacker,
            comparison: Comparison::LessOrEqual,
            amount: Expr::Constant(1),
            ..
        })]
    ));
    assert!(matches!(
        changes(&c, 1),
        [Change::Rule(RuleChange::CombatGroup {
            role: CombatRole::Blocker,
            comparison: Comparison::LessOrEqual,
            amount: Expr::Constant(1),
            ..
        })]
    ));
    let c = card("Jackal Familiar");
    assert!(matches!(
        changes(&c, 0),
        [
            Change::Rule(RuleChange::CombatGroup {
                role: CombatRole::Attacker,
                comparison: Comparison::Greater,
                amount: Expr::Constant(1),
                ..
            }),
            Change::Rule(RuleChange::CombatGroup {
                role: CombatRole::Blocker,
                comparison: Comparison::Greater,
                amount: Expr::Constant(1),
                ..
            })
        ]
    ));
    let c=program("PermanentRuleEffect(ThisPermanent,[CantBeBlockedExceptByMultipleDefenders(GreaterThanOrEqualTo(Integer(2)),IsCardtype(Creature))])").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        changes(&c, 0),
        [
            Change::Rule(RuleChange::Prohibit(ObjectAction::BeBlocked(_))),
            Change::Rule(RuleChange::CombatGroup {
                role: CombatRole::BlockedAttacker,
                comparison: Comparison::GreaterOrEqual,
                amount: Expr::Constant(2),
                ..
            })
        ]
    ));
    let c = program(
        "PermanentRuleEffect(ThisPermanent,[MustBeBlockedByAtLeastNumberDefenders(Integer(2))])",
    )
    .unwrap();
    assert!(matches!(
        changes(&c, 0),
        [Change::Rule(RuleChange::RequireCombat {
            amount: RequirementAmount::AtLeast(Expr::Constant(2)),
            ..
        })]
    ));
}

#[test]
fn untap_step_rules_keep_optional_and_additional_untapping_distinct_from_prohibitions() {
    let c = card("Basalt Monolith");
    assert!(
        matches!(changes(&c,0),[Change::Rule(RuleChange::UntapStep{players:PlayerSet::One(PlayerRef::ControllerOf(p)),behavior:UntapBehavior::Skip})]if matches!(**p,ObjectRef::Affected))
    );
    let c = card("Drumbellower");
    assert!(matches!(
        changes(&c, 1),
        [Change::Rule(RuleChange::UntapStep {
            players: PlayerSet::OthersThan(PlayerRef::AbilityController),
            behavior: UntapBehavior::Additional
        })]
    ));
    let c = program(
        "PermanentRuleEffect(ThisPermanent,[MayChooseNotToUntapDuringUntap,CantBecomeUntapped])",
    )
    .unwrap();
    assert!(matches!(
        changes(&c, 0),
        [
            Change::Rule(RuleChange::UntapStep {
                behavior: UntapBehavior::Optional,
                ..
            }),
            Change::Rule(RuleChange::Prohibit(ObjectAction::Untap))
        ]
    ));
}

#[test]
fn activation_guards_are_checked_before_costs_and_combined_without_turning_into_body_conditions() {
    let c = card("Bloodshot Trainee");
    assert!(
        matches!(&c.abilities[0],Ability::Activated{limits:ActivationLimits{condition:Some(Condition::ObjectMatches{
        object:ObjectRef::Source,filter:ObjectFilter::Property{property:Property::Power,comparison:Comparison::GreaterOrEqual,value:Expr::Constant(4)}
    }),..},body,..}if matches!(body.instructions.as_slice(),[Instruction::Effect{operation:Operation::DealDamage{..},..}]))
    );
    let c=program("ActivatedWithModifiers(TapPermanent(ThisPermanent),ActionList([DrawACard]),And([ActivateOnlyIf(PermanentPassesFilter(ThisPermanent,IsCardtype(Creature))),ActivateOnlyDuringTheirTurn,CantActivateIf(PermanentPassesFilter(ThisPermanent,IsTapped)),ActivateOnlyOnceEachTurn]))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        &c.abilities[0],
        Ability::Activated {
            limits: ActivationLimits {
                once_each_turn: true,
                condition: Some(Condition::All(_)),
                ..
            },
            ..
        }
    ));
    let errors=program("ActivatedWithModifiers(PayMana([]),ActionList([MayCost(PayLife(Integer(1)))]),ActivateOnlyIf(CostWasPaid))").unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.message.contains("payment reference")));
    #[cfg(feature = "cantrip")]
    assert!(mtgish_ast::cantrip::module(&card("Bloodshot Trainee"))
        .unwrap()
        .contains("power"));
}

#[test]
fn activation_filters_keep_mana_exceptions_and_battlefield_scope() {
    let c = card("Damping Matrix");
    assert!(
        matches!(changes(&c,0),[Change::Rule(RuleChange::Prohibit(ObjectAction::Activate(ActivationFilter::Not(filter))))]if matches!(**filter,ActivationFilter::Mana))
    );
    let c = card("Stony Silence");
    assert!(
        matches!(&c.abilities[0],Ability::Static{affected:EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::CardType(source::CardType::Artifact))),changes,..}
        if matches!(changes.as_slice(),[Change::Rule(RuleChange::Prohibit(ObjectAction::Activate(ActivationFilter::Any)))]))
    );
    // Sources in any zone are a rule on players, not on battlefield permanents.
    let anywhere = program("ActivatedAbilityEffect(NonManaAbility,CantBeActivated)").unwrap();
    assert!(
        matches!(&anywhere.abilities[0],Ability::Static{affected:EffectSubjects::Players(PlayerSet::All),changes,..}
        if matches!(changes.as_slice(),[Change::PlayerRule(PlayerRule::Prohibit(PlayerAction::Activate(ActivationFilter::Not(_))))]))
    );
}

#[test]
fn filter_candidates_and_declaring_players_do_not_escape_their_scopes() {
    let c = card("Ancient Lumberknot");
    let Ability::Static {
        affected: EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::All(filters))),
        ..
    } = &c.abilities[0]
    else {
        panic!()
    };
    assert!(filters.iter().any(|f| matches!(
        f,
        ObjectFilter::Property {
            value: Expr::ObjectProperty {
                object: ObjectRef::Candidate,
                property: Property::Power,
                ..
            },
            ..
        }
    )));
    let mut c = card("Bloodshot Trainee");
    let Ability::Activated { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::DeclaringPlayer),
            amount: Expr::Constant(1),
        },
    }];
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("outside a combat declaration"));
    let Ability::Activated { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    body.instructions = vec![Instruction::Effect {
        result: ResultId(0),
        operation: Operation::Draw {
            players: PlayerSet::One(PlayerRef::AbilityController),
            amount: Expr::ObjectProperty {
                object: ObjectRef::Candidate,
                property: Property::Power,
                information: Information::CurrentOrLastKnown,
            },
        },
    }];
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("outside an object predicate"));
}
