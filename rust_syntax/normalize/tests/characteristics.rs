use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};

fn sources() -> impl Iterator<Item = source::OracleCard> {
    include_str!("fixtures/characteristics.ronl")
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
    let mut c = source("Maro");
    let source::OracleCard::Card { rules, .. } = &mut c else {
        panic!()
    };
    *rules = Some(vec![ron::from_str(rule).unwrap()]);
    lower::card(&c)
}
fn definitions(c: &Card) -> Vec<&Ability> {
    c.abilities
        .iter()
        .filter(|a| {
            matches!(
                a,
                Ability::Static {
                    kind: StaticKind::CharacteristicDefining { .. },
                    ..
                }
            )
        })
        .collect()
}
fn changes(a: &Ability) -> &[Change] {
    let Ability::Static { changes, .. } = a else {
        panic!()
    };
    changes
}

#[test]
fn characteristic_families_roundtrip_and_other_unimplemented_abilities_are_not_dropped() {
    let incomplete: [&str; 0] = [];
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
    assert_eq!(complete, 54);
}

#[test]
fn joint_definitions_and_genuinely_separate_abilities_retain_their_printed_boundaries() {
    let c = card("Maro");
    assert_eq!(c.abilities.len(), 1);
    assert!(
        matches!(&c.abilities[0],Ability::Static {kind:StaticKind::CharacteristicDefining {..},affected:EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),changes} if matches!(changes.as_slice(),[Change::SetBasePowerToughness {power:Expr::CountObjects(_),toughness:Expr::CountObjects(_)}]))
    );
    let c = card("Yavimaya Kavu");
    assert_eq!(c.abilities.len(), 2);
    assert!(matches!(changes(&c.abilities[0]), [Change::SetPower(_)]));
    assert!(matches!(
        changes(&c.abilities[1]),
        [Change::SetToughness(_)]
    ));
    let c = custom("CDA_PowerToughness(Integer(2),Plus(Integer(2),Integer(1)))").unwrap();
    validate::card(&c).unwrap();
    assert_eq!(c.abilities.len(), 1);
}

#[test]
fn defining_functions_remain_live_and_distinct_from_ordinary_layers() {
    let c = card("Maro");
    assert_eq!(c.characteristics.power_toughness, Some((0, 0)));
    let [Change::SetBasePowerToughness {
        power: Expr::CountObjects(objects),
        ..
    }] = changes(definitions(&c)[0])
    else {
        panic!()
    };
    assert!(matches!(
        objects.as_ref(),
        ObjectSet::Zone {
            scope: ZoneScope::Players {
                zone: EventZone::Hand,
                players: PlayerSet::One(PlayerRef::AbilityController)
            },
            ..
        }
    ));
    let c = card("Death's Shadow");
    assert!(definitions(&c).is_empty());
    assert!(
        matches!(&c.abilities[0],Ability::Static {kind:StaticKind::Ordinary,changes,..} if matches!(changes.as_slice(),[Change::Pump {..}]))
    );
    let c = card("Multani, Yavimaya's Avatar");
    assert!(definitions(&c).is_empty());
    let mut c = card("Maro");
    c.abilities[0] = Ability::Scoped {
        zones: vec![EventZone::Battlefield],
        condition: None,
        ability: Box::new(c.abilities[0].clone()),
    };
    assert!(validate::card(&c).unwrap_err().contains("everywhere"));
}

#[test]
fn distinct_values_aggregation_and_mana_symbol_queries_remain_different_queries() {
    let c = card("Tarmogoyf");
    let [Change::SetBasePowerToughness { power, toughness }] = changes(definitions(&c)[0]) else {
        panic!()
    };
    assert!(
        matches!(power,Expr::CountDistinct {property:DistinctProperty::CardTypes,objects} if matches!(objects.as_ref(),ObjectSet::Zone {scope:ZoneScope::Players {zone:EventZone::Graveyard,players:PlayerSet::All},..}))
    );
    assert!(matches!(toughness,Expr::Add(_,one) if matches!(one.as_ref(),Expr::Constant(1))));
    let c = card("Aven Trailblazer");
    assert!(matches!(
        changes(definitions(&c)[0]),
        [Change::SetToughness(Expr::CountDistinct {
            property: DistinctProperty::BasicLandTypes,
            ..
        })]
    ));
    let c = card("Ancient Ooze");
    assert!(matches!(
        changes(definitions(&c)[0]),
        [Change::SetBasePowerToughness {
            power: Expr::Aggregate {
                property: Property::ManaValue,
                aggregation: Aggregation::Sum,
                ..
            },
            ..
        }]
    ));
    let c = card("Daxos, Blessed by the Sun");
    assert_eq!(c.characteristics.power_toughness, Some((2, 0)));
    assert!(
        matches!(changes(definitions(&c)[0]),[Change::SetToughness(Expr::CountManaSymbols {colors,objects})] if matches!(colors.as_slice(),[ManaColor::White]) && matches!(objects.as_ref(),ObjectSet::Battlefield(ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)))))
    );
    let c = custom("CDA_Power(TheHighestManaValueAmongPermanents(AnyPermanent))").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        changes(definitions(&c)[0]),
        [Change::SetPower(Expr::Aggregate {
            aggregation: Aggregation::Maximum,
            ..
        })]
    ));
}

#[test]
fn color_definitions_preserve_keyword_and_color_identity_contribution_without_changing_costs() {
    let c = card("Complete Disregard");
    assert!(
        matches!(definitions(&c)[0],Ability::Static {kind:StaticKind::CharacteristicDefining {keyword:Some(Keyword::Devoid),..},changes,..} if matches!(changes.as_slice(),[Change::Colors {mode:ChangeMode::Set,colors}] if colors.is_empty()))
    );
    let c = card("Ghostfire");
    assert!(
        matches!(definitions(&c)[0],Ability::Static {kind:StaticKind::CharacteristicDefining {keyword:None,..},changes,..} if matches!(changes.as_slice(),[Change::Colors {colors,..}] if colors.is_empty()))
    );
    assert!(c
        .characteristics
        .mana_cost
        .as_ref()
        .unwrap()
        .iter()
        .any(|s| matches!(s, ManaSymbol::Color(ManaColor::Red))));
    let c = card("Transguild Courier");
    assert!(
        matches!(definitions(&c)[0],Ability::Static {kind:StaticKind::CharacteristicDefining {contributes_to_color_identity:true,..},changes,..} if matches!(changes.as_slice(),[Change::Colors {colors,..}] if colors.len()==5))
    );
    let c = custom("CDA_ColorButNotColorIdentity(AllColors)").unwrap();
    validate::card(&c).unwrap();
    assert!(matches!(
        &c.abilities[0],
        Ability::Static {
            kind: StaticKind::CharacteristicDefining {
                contributes_to_color_identity: false,
                ..
            },
            ..
        }
    ));
    let c = card("Darkest Hour");
    assert!(
        matches!(&c.abilities[0],Ability::Static {kind:StaticKind::Ordinary,changes,..} if matches!(changes.as_slice(),[Change::Colors {mode:ChangeMode::Set,colors}] if matches!(colors.as_slice(),[ManaColor::Black])))
    );
}

#[test]
fn every_creature_type_preserves_other_categories_and_does_not_make_an_instant_a_creature() {
    let c = card("Universal Automaton");
    assert!(
        matches!(definitions(&c)[0],Ability::Static {kind:StaticKind::CharacteristicDefining {keyword:Some(Keyword::Changeling),..},changes,..} if matches!(changes.as_slice(),[Change::CreatureTypes {mode:ChangeMode::Set,types:CreatureTypeSet::All}]))
    );
    let c = card("Mistform Ultimus");
    assert!(
        matches!(definitions(&c)[0],Ability::Static {kind:StaticKind::CharacteristicDefining {keyword:None,..},changes,..} if matches!(changes.as_slice(),[Change::CreatureTypes {types:CreatureTypeSet::All,..}]))
    );
    let c = card("Crib Swap");
    assert!(!c
        .characteristics
        .card_types
        .iter()
        .any(|t| matches!(t, source::CardType::Creature)));
    assert!(!definitions(&c).is_empty());
    let c = card("Amoeboid Changeling");
    let mut found_all = false;
    let mut found_empty = false;
    for a in &c.abilities {
        if let Ability::Activated { body, .. } = a {
            for i in &body.instructions {
                if let Instruction::Effect {
                    operation: Operation::CreateContinuousEffect { changes, .. },
                    ..
                } = i
                {
                    found_all |= matches!(
                        changes.as_slice(),
                        [Change::CreatureTypes {
                            mode: ChangeMode::Set,
                            types: CreatureTypeSet::All
                        }]
                    );
                    found_empty |= matches!(changes.as_slice(),[Change::CreatureTypes {mode:ChangeMode::Set,types:CreatureTypeSet::Listed(t)}] if t.is_empty());
                }
            }
        }
    }
    assert!(found_all && found_empty);
}

#[test]
fn token_creation_retains_its_defining_ability_recipe_instead_of_snapshotting_the_formula() {
    let c = card("Hallowed Haunting");
    let Ability::Triggered { body, .. } = &c.abilities[1] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateTokens { tokens, .. },
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    let [token] = tokens.as_slice() else { panic!() };
    assert!(matches!(
        token.definition().unwrap().power_toughness,
        Some((Expr::Constant(0), Expr::Constant(0)))
    ));
    assert!(
        matches!(token.definition().unwrap().abilities.as_slice(),[Ability::Static {kind:StaticKind::CharacteristicDefining {..},affected:EffectSubjects::Objects(ObjectSet::One(ObjectRef::Source)),changes}] if matches!(changes.as_slice(),[Change::SetBasePowerToughness {power:Expr::CountObjects(_),..}]))
    );
}

#[test]
fn invalid_definitions_and_unbound_numeric_components_are_rejected() {
    let mut c = card("Maro");
    let Ability::Static { affected, .. } = &mut c.abilities[0] else {
        panic!()
    };
    *affected = EffectSubjects::Objects(ObjectSet::Battlefield(ObjectFilter::Any));
    assert!(validate::card(&c).unwrap_err().contains("only its source"));
    let mut c = card("Maro");
    let Ability::Static { changes, .. } = &mut c.abilities[0] else {
        panic!()
    };
    changes.push(Change::Pump {
        power: Expr::Constant(1),
        toughness: Expr::Constant(1),
    });
    assert!(validate::card(&c).unwrap_err().contains("may define only"));
    let mut c = card("Maro");
    let Ability::Static { changes, .. } = &mut c.abilities[0] else {
        panic!()
    };
    *changes = vec![Change::SetPower(Expr::AnnouncedX)];
    assert!(validate::card(&c).unwrap_err().contains("announced X"));
    let mut c = card("Maro");
    c.abilities[0] = Ability::Conditional {
        condition: Condition::PlayerIsActive(PlayerRef::AbilityController),
        then: vec![c.abilities[0].clone()],
        otherwise: vec![],
    };
    assert!(validate::card(&c).unwrap_err().contains("conditional"));
    assert!(
        custom("PermanentLayerEffect(ThisPermanent,[AddAbility([CDA_Power(Integer(3))])])")
            .is_err()
    );
    let mut c = card("Complete Disregard");
    let Ability::Static { changes, .. } = &mut c.abilities[0] else {
        panic!()
    };
    *changes = vec![Change::Colors {
        mode: ChangeMode::Set,
        colors: vec![ManaColor::Red],
    }];
    assert!(validate::card(&c).unwrap_err().contains("defining keyword"));
}

#[test]
fn legacy_joint_source_form_roundtrips_bincode_on_a_cli_sized_stack() {
    // The large legacy derive uses more stack than Rust's default test thread;
    // keep that requirement separate from the small normalized representation.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            let mut total = 0;
            for source in sources() {
                let bytes = bincode::encode_to_vec(&source, bincode::config::standard()).unwrap();
                let (decoded, used): (source::OracleCard, usize) =
                    bincode::decode_from_slice(&bytes, bincode::config::standard()).unwrap();
                assert_eq!(used, bytes.len());
                assert_eq!(
                    serde_json::to_value(decoded).unwrap(),
                    serde_json::to_value(source).unwrap()
                );
                total += 1;
            }
            assert_eq!(total, 54);
        })
        .unwrap()
        .join()
        .unwrap();
}
