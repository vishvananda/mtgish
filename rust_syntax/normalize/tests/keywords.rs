use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/keywords.ronl")
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
    let mut c = source("White Knight");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn program(c: &Card) -> &Program {
    c.abilities
        .iter()
        .find_map(|a| match a {
            Ability::Activated { body, .. } | Ability::Spell { body, .. } => Some(body),
            _ => None,
        })
        .unwrap()
}
fn protection(c: &Card) -> &Protection {
    c.abilities
        .iter()
        .find_map(|a| {
            if let Ability::Keyword(Keyword::Protection(q)) = a {
                Some(q.as_ref())
            } else {
                None
            }
        })
        .unwrap()
}
fn ward(c: &Card) -> &Cost {
    c.abilities
        .iter()
        .find_map(|a| {
            if let Ability::Keyword(Keyword::Ward(cost)) = a {
                Some(cost.as_ref())
            } else {
                None
            }
        })
        .unwrap()
}
fn instance_count(q: &Protection) -> usize {
    match q {
        Protection::Each(qs) => qs.iter().map(instance_count).sum(),
        _ => 1,
    }
}

#[test]
fn native_keyword_and_regeneration_families_roundtrip_without_hiding_nested_gaps() {
    let incomplete = [
        "Phyrexian Fleshgorger",
        "Mother of Runes",
        "Earnest Fellowship",
        "True-Name Nemesis",
        "Volatile Stormdrake",
        "Minthara, Merciless Soul",
        "Albino Troll",
        "Aboleth Spawn",
        "Spectra Ward",
        "Samite Elder",
        "The Serpent Society",
    ];
    let mut complete = 0;
    for s in sources() {
        let name = corpus::Identity::of(&s).unwrap().names.remove(0);
        if incomplete.contains(&name.as_str()) {
            assert!(lower::card(&s).is_err(), "{name}");
            continue;
        }
        let c = lower::card(&s).unwrap_or_else(|e| panic!("{name}: {e:?}"));
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
    assert_eq!(complete, 36);
}

#[test]
fn separate_protection_qualities_remain_separate_keyword_instances() {
    let c = card("Iridescent Angel");
    assert_eq!(instance_count(protection(&c)), 5);
    let c = card("Pristine Angel");
    let Ability::Conditional { then, .. } = &c.abilities[1] else {
        panic!()
    };
    let Ability::Static { changes, .. } = &then[0] else {
        panic!()
    };
    let [Change::GrantKeyword(Keyword::Protection(qualities))] = changes.as_slice() else {
        panic!()
    };
    assert_eq!(instance_count(qualities), 6); // artifacts plus each of five colors
    let c = custom("Protection(FromColor(Colored))").unwrap();
    validate::card(&c).unwrap();
    assert_eq!(instance_count(protection(&c)), 1);
    assert!(
        matches!(protection(&c), Protection::Objects { kind: ObjectKind::Any, filter: ObjectFilter::Not(f) } if matches!(**f,ObjectFilter::Colorless))
    );
    let c = custom("Protection(FromColor(Colors([Black,Black])))").unwrap();
    assert_eq!(instance_count(protection(&c)), 2);
}

#[test]
fn protection_source_domains_are_not_silently_narrowed_to_battlefield_permanents() {
    let c = card("Beloved Chaplain");
    assert!(matches!(
        protection(&c),
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::CardType(source::CardType::Creature)
        }
    ));
    let c = card("Mistmeadow Skulk");
    assert!(matches!(
        protection(&c),
        Protection::Objects {
            kind: ObjectKind::Any,
            filter: ObjectFilter::Property {
                property: Property::ManaValue,
                comparison: Comparison::GreaterOrEqual,
                value: Expr::Constant(3)
            }
        }
    ));
    let c = card("Devoted Caretaker");
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { changes, .. },
        ..
    } = &program(&c).instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(changes.as_slice(), [Change::GrantKeyword(Keyword::Protection(q))] if matches!(q.as_ref(), Protection::Each(qs) if qs.len()==2 && qs.iter().all(|q| matches!(q, Protection::Objects { kind: ObjectKind::Spell, .. }))))
    );
    let c = custom("Protection(FromPermanents(AnyPermanent))").unwrap();
    assert!(matches!(
        protection(&c),
        Protection::Objects {
            kind: ObjectKind::Permanent,
            ..
        }
    ));
    let c = custom("PlayerEffect(You,[Protection(FromEverything)])").unwrap();
    validate::card(&c).unwrap();
    assert!(
        matches!(&c.abilities[0], Ability::Static { affected: EffectSubjects::Players(_), changes, .. } if matches!(changes.as_slice(), [Change::PlayerRule(PlayerRule::GrantKeyword(PlayerKeyword::Protection(q)))] if matches!(q.as_ref(),Protection::Objects { kind:ObjectKind::Any, filter:ObjectFilter::Any })))
    );
}

#[test]
fn protection_from_players_preserves_the_player_quality_and_captures_granting_bindings() {
    let c = card("Cliffside Rescuer");
    let Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                changes,
                evaluation: EffectEvaluation::Snapshot,
                ..
            },
        ..
    } = &program(&c).instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(changes.as_slice(), [Change::GrantKeyword(Keyword::Protection(q))] if matches!(q.as_ref(), Protection::Players(PlayerSet::OpponentsOf(PlayerRef::AbilityController))))
    );
    let c = custom("SpellActions(Targeted([TargetPlayer(AnyPlayer),TargetPermanent(AnyPermanent)],ActionList([CreatePermanentLayerEffectUntil(Ref_TargetPermanent,[AddAbility([Protection(FromPlayers(SinglePlayer(Ref_TargetPlayer)))])],UntilEndOfTurn)])))").unwrap();
    validate::card(&c).unwrap();
    let Instruction::Effect {
        operation:
            Operation::CreateContinuousEffect {
                affected:
                    EffectSubjects::Objects(ObjectSet::One(ObjectRef::Target { slot: Slot(1), .. })),
                changes,
                ..
            },
        ..
    } = &program(&c).instructions[0]
    else {
        panic!()
    };
    assert!(
        matches!(changes.as_slice(), [Change::GrantKeyword(Keyword::Protection(q))] if matches!(q.as_ref(), Protection::Players(PlayerSet::One(PlayerRef::Target { slot: Slot(0), .. }))))
    );
}

#[test]
fn landwalk_and_qualified_hexproof_keep_keyword_identity_and_typed_predicates() {
    let c = card("Anaconda");
    assert!(
        matches!(&c.abilities[0], Ability::Keyword(Keyword::Landwalk(f)) if matches!(f.as_ref(),ObjectFilter::Subtype(source::SubType::Swamp)))
    );
    let c = card("Dryad Sophisticate");
    assert!(
        matches!(&c.abilities[0], Ability::Keyword(Keyword::Landwalk(f)) if matches!(f.as_ref(),ObjectFilter::Not(f) if matches!(f.as_ref(),ObjectFilter::Supertype(source::SuperType::Basic))))
    );
    let c = card("Knight of Grace");
    assert!(c.abilities.iter().any(|a| matches!(a,Ability::Keyword(Keyword::HexproofFrom(q)) if matches!(q.as_ref(),Protection::Objects { kind:ObjectKind::Any,filter:ObjectFilter::Color(ManaColor::Black) }))));
    let c = custom("EachPermanentLayerEffect(HasAbility(AnyWard),[AdjustPT(1,1)])").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        &c.abilities[0],
        Ability::Static {
            affected: EffectSubjects::Objects(ObjectSet::Battlefield(
                ObjectFilter::HasKeywordFamily(KeywordFamily::Ward)
            )),
            ..
        }
    ));
}

#[test]
fn ward_retains_a_later_triggered_payment_and_cost_choice_is_not_a_joint_cost() {
    let c = card("Dwarven Forge-Chanter");
    assert!(matches!(
        ward(&c).parts.as_slice(),
        [CostPart::Life(Expr::Constant(2))]
    ));
    let c = card("Ovika, Enigma Goliath");
    assert!(matches!(
        ward(&c).parts.as_slice(),
        [CostPart::Mana(_), CostPart::Life(Expr::Constant(3))]
    ));
    let c = card("Vein Ripper");
    assert!(matches!(
        ward(&c).parts.as_slice(),
        [CostPart::Sacrifice {
            choose: Some(Count::Exactly(Expr::Constant(1))),
            ..
        }]
    ));
    let c = card("Titania, Rugged Rumbler");
    assert!(
        matches!(ward(&c).parts.as_slice(),[CostPart::Choice(costs)] if matches!(costs.as_slice(),[Cost { parts: a },Cost { parts:b }] if matches!(a.as_slice(),[CostPart::Discard { .. }]) && matches!(b.as_slice(),[CostPart::Mana(_)])))
    );
    let c = card("Raubahn, Bull of Ala Mhigo");
    assert!(matches!(
        ward(&c).parts.as_slice(),
        [CostPart::Life(Expr::ObjectProperty {
            object: ObjectRef::KeywordHost,
            property: Property::Power,
            information: Information::CurrentOrLastKnown
        })]
    ));
    let c = custom("EachPermanentLayerEffect(IsCardtype(Creature),[AddAbility([Ward(PayLife(PowerOfPermanent(ThisPermanent)))])])").unwrap();
    validate::card(&c).unwrap();
    assert!(
        matches!(&c.abilities[0],Ability::Static { changes, .. } if matches!(changes.as_slice(),[Change::GrantKeyword(Keyword::Ward(cost))] if matches!(cost.parts.as_slice(),[CostPart::Life(Expr::ObjectProperty { object:ObjectRef::KeywordHost,.. })])))
    );
}

#[test]
fn regeneration_creates_shields_without_immediate_tapping_damage_removal_or_combat_changes() {
    let c = card("Drudge Skeletons");
    assert!(matches!(
        program(&c).instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::CreateRegenerationShields {
                objects: ObjectSet::One(ObjectRef::Source)
            },
            ..
        }]
    ));
    let c = card("Wrap in Vigor");
    assert!(matches!(
        program(&c).instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::CreateRegenerationShields {
                objects: ObjectSet::Battlefield(_)
            },
            ..
        }]
    ));
    let c = custom("SpellActions(ActionList([RegeneratePermanent(ThisPermanent),RegeneratePermanent(ThisPermanent)]))").unwrap();
    assert_eq!(program(&c).instructions.len(), 2); // do not deduplicate shields
    assert!(custom("SpellActions(ActionList([RegeneratePermanent_WhenItRengeratesThisWay(ThisPermanent,ActionList([DrawACard]))]))").is_err());
    let mut c = card("Wrap in Vigor");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateRegenerationShields { objects },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *objects = ObjectSet::Zone {
        scope: ZoneScope::Players {
            zone: EventZone::Graveyard,
            players: PlayerSet::All,
        },
        kind: ObjectKind::Card,
        filter: ObjectFilter::Any,
    };
    assert!(validate::card(&c).unwrap_err().contains("Battlefield"));
}

#[test]
fn keyword_cost_bindings_and_nested_quality_references_cannot_escape_their_scope() {
    for amount in [
        Expr::PlayerLife(PlayerRef::WardPayer),
        Expr::ObjectProperty {
            object: ObjectRef::KeywordHost,
            property: Property::Power,
            information: Information::CurrentOrLastKnown,
        },
    ] {
        let mut c = card("Death Ward");
        let Ability::Spell { body, .. } = &mut c.abilities[0] else {
            panic!()
        };
        body.instructions.push(Instruction::Effect {
            result: ResultId(99),
            operation: Operation::Draw {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount,
            },
        });
        assert!(validate::card(&c).is_err());
    }
    let mut c = card("White Knight");
    let Ability::Keyword(Keyword::Protection(q)) = &mut c.abilities[1] else {
        panic!()
    };
    *q = Box::new(Protection::Each(vec![]));
    assert!(validate::card(&c).unwrap_err().contains("at least two"));
    let mut c = card("White Knight");
    let Ability::Keyword(Keyword::Protection(q)) = &mut c.abilities[1] else {
        panic!()
    };
    *q = Box::new(Protection::Players(PlayerSet::One(PlayerRef::WardPayer)));
    assert!(validate::card(&c).unwrap_err().contains("ward payer"));
}
