use mtgish_ast::{corpus, ir::*, lower, mtg_types as source, validate};

fn source(name: &str) -> source::OracleCard {
    include_str!("fixtures/attachments.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap()
}

fn card(name: &str) -> Card {
    let card = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&card).unwrap();
    card
}

#[test]
fn aura_and_equipment_families_roundtrip_as_typed_rust() {
    for name in [
        "Holy Strength",
        "Unholy Strength",
        "Flight",
        "Angelic Gift",
        "Frog Tongue",
        "Lifelink",
        "Robe of Mirrors",
        "Bladed Pinions",
        "Bonesplitter",
        "Short Sword",
        "Leonin Scimitar",
        "Cobbled Wings",
        "Giant Spectacle",
        "Lignify",
        "Pacifism",
        "Lightning Greaves",
    ] {
        let c = card(name);
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
    }
}

#[test]
fn enchant_keeps_host_legality_and_static_changes_follow_the_current_host() {
    let c = card("Holy Strength");
    assert!(matches!(
        &c.abilities[0],
        Ability::Enchant {
            hosts: EnchantHosts::Permanents(ObjectFilter::CardType(source::CardType::Creature))
        }
    ));
    let Ability::Static {
        affected, changes, ..
    } = &c.abilities[1]
    else {
        panic!()
    };
    assert!(
        matches!(affected, EffectSubjects::Objects(ObjectSet::One(ObjectRef::AttachedTo(attachment))) if matches!(**attachment, ObjectRef::Source))
    );
    assert!(matches!(
        changes.as_slice(),
        [Change::Pump {
            power: Expr::Constant(1),
            toughness: Expr::Constant(2)
        }]
    ));

    let lignify = card("Lignify");
    let Ability::Static { changes, .. } = &lignify.abilities[1] else {
        panic!()
    };
    assert!(matches!(
        changes.as_slice(),
        [
            Change::CreatureTypes {
                mode: ChangeMode::Set,
                ..
            },
            Change::SetBasePowerToughness {
                power: Expr::Constant(0),
                toughness: Expr::Constant(4)
            },
            Change::RemoveAllAbilities
        ]
    ));
}

#[test]
fn equip_uses_an_ordinary_targeted_attach_program_but_retains_keyword_identity() {
    let mut c = card("Bonesplitter");
    let Ability::Activated {
        origin,
        limits,
        announcement,
        body,
        ..
    } = &c.abilities[1]
    else {
        panic!()
    };
    assert!(matches!(origin, ActivationOrigin::Equip));
    assert!(limits.sorcery);
    assert!(matches!(announcement.targets.as_slice(), [TargetSpec {
        kind: TargetKind::Object(_), count: Count::Exactly(Expr::Constant(1)),
        filter: ObjectFilter::All(filters), ..
    }] if matches!(filters.as_slice(), [ObjectFilter::CardType(source::CardType::Creature), ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController))])));
    assert!(matches!(
        body.instructions.as_slice(),
        [Instruction::Effect {
            operation: Operation::Attach {
                attachments: ObjectSet::One(ObjectRef::Source),
                host: ObjectRef::Target {
                    slot: Slot(0),
                    index: 0
                }
            },
            ..
        }]
    ));

    let Ability::Activated { limits, .. } = &mut c.abilities[1] else {
        panic!()
    };
    limits.sorcery = false;
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("invalid equip activation"));
    let Ability::Activated { limits, body, .. } = &mut c.abilities[1] else {
        panic!()
    };
    limits.sorcery = true;
    let Instruction::Effect {
        operation: Operation::Attach { host, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *host = ObjectRef::Target {
        slot: Slot(1),
        index: 0,
    };
    assert!(validate::card(&c)
        .unwrap_err()
        .contains("different target slot"));
}

#[test]
fn attachment_restrictions_are_preserved_and_special_equip_still_requires_lowering() {
    let pacifism = card("Pacifism");
    assert!(
        matches!(&pacifism.abilities[1],Ability::Static {changes,..} if matches!(changes.as_slice(),[
            Change::Rule(RuleChange::Prohibit(ObjectAction::Attack(AttackDefenders::Any))),
            Change::Rule(RuleChange::Prohibit(ObjectAction::Block(ObjectFilter::Any)))
        ]))
    );
    // Equip planeswalker stays its own form, not an ordinary qualifier.
    let luxior = format!(
        "{:?}",
        lower::card(&source("Luxior, Giada's Gift")).unwrap()
    );
    assert!(
        luxior.contains("mechanic: Equip") && luxior.contains("CardType(Planeswalker)"),
        "special equip legality cannot silently become an ordinary qualifier"
    );
}
