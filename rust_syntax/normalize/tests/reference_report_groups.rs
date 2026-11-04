use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard};

fn source_line(name: &str) -> &'static str {
    include_str!("fixtures/reference_report_groups.ronl").lines().find(|line| {
        let source: OracleCard = ron::from_str(line).unwrap();
        corpus::Identity::of(&source).unwrap().names.iter().any(|n| n == name)
    }).unwrap()
}

fn lower_line(line: &str) -> Card {
    lower::card(&ron::from_str(line).unwrap()).unwrap()
}

#[test]
fn orcus_modes_and_their_target_constraints_keep_the_entry_cast_x() {
    let card = lower_line(source_line("Orcus, Prince of Undeath"));
    let json = serde_json::to_string(&card).unwrap();
    assert_eq!(json.matches("CastX").count(), 5, "{json}");
    assert!(!json.contains("AnnouncedX"), "{json}");
    let Ability::Triggered { announcement, .. } = &card.abilities[2] else { panic!() };
    let modes = &announcement.modes.as_ref().unwrap().modes;
    assert!(matches!(modes[1].announcement.targets[0].count, Count::UpTo(Expr::CastX(ObjectRef::EventSubject))));
}

#[test]
fn an_activated_modal_ability_keeps_its_own_announced_x() {
    let line = source_line("Orcus, Prince of Undeath").replace(
        "TriggerA(WhenAPermanentEntersTheBattlefield(SinglePermanent(ThisPermanent)), Modal_ChooseOne(",
        "Activated(PayManaX([], ValueX), Modal_ChooseOne(",
    );
    assert_ne!(line, source_line("Orcus, Prince of Undeath"));
    let json = serde_json::to_string(&lower_line(&line)).unwrap();
    assert_eq!(json.matches("AnnouncedX").count(), 5, "{json}");
    assert!(!json.contains("CastX"), "{json}");
}

#[test]
fn owned_card_effects_exclude_battlefield_objects_without_restricting_the_other_populations() {
    for name in ["Arcane Adaptation", "Biotransference", "Celestial Dawn", "Conspiracy", "Encroaching Mycosynth",
        "Leyline of Transformation", "Maskwood Nexus", "Roshan, Hidden Magister", "Rukarumel, Biologist"] {
        let card = lower_line(source_line(name));
        let mut cards = 0;
        for ability in &card.abilities {
            if let Ability::Static { affected: EffectSubjects::Objects(ObjectSet::AnyZone(filter)), .. } = ability {
                let json = serde_json::to_string(filter).unwrap();
                assert!(json.contains("OwnedBy"), "{name}: {json}");
                let ObjectFilter::All(filters) = filter else { panic!("{name}: {filter:?}") };
                assert!(filters.iter().any(|f| matches!(f, ObjectFilter::Not(inner)
                    if matches!(inner.as_ref(), ObjectFilter::InZone(ZoneScope::Shared(EventZone::Battlefield))))), "{name}: {json}");
                cards += 1;
            }
        }
        assert_eq!(cards, 1, "{name}");
    }
    let card = lower_line(source_line("Arcane Adaptation"));
    let mut controlled_populations = 0;
    for ability in &card.abilities {
        if let Ability::Static { affected: EffectSubjects::Objects(ObjectSet::Zone {
            scope: ZoneScope::Shared(EventZone::Battlefield | EventZone::Stack), filter, ..
        }), .. } = ability {
            let json = serde_json::to_string(filter).unwrap();
            assert!(json.contains("ControlledBy"), "{json}");
            assert!(!json.contains("InZone"), "{json}");
            controlled_populations += 1;
        }
    }
    assert_eq!(controlled_populations, 2);
}

#[test]
fn reparsed_gyruda_preserves_both_milled_card_restrictions() {
    let card = lower_line(source_line("Gyruda, Doom of Depths"));
    let Ability::Triggered { body, .. } = &card.abilities[1] else { panic!() };
    let Instruction::Group(group) = &body.instructions[1] else { panic!() };
    let Instruction::Effect { operation: Operation::ChooseObjects { objects: ObjectSet::Matching { filter, .. }, .. }, .. }
        = &group.instructions[0] else { panic!() };
    let json = serde_json::to_string(filter).unwrap();
    assert!(json.contains("Creature") && json.contains("ManaValue") && json.contains("SameParity"), "{json}");
}

#[test]
fn mandatory_actions_before_reflexive_triggers_are_not_optional() {
    for name in ["Diregraf Horde", "Glissa's Retriever"] {
        let json = serde_json::to_string(&lower_line(source_line(name))).unwrap();
        assert!(!json.contains("Optional"), "{name}: {json}");
        assert_eq!(json.matches("ReflexiveTrigger").count(), 1, "{name}: {json}");
        assert!(json.contains(if name == "Diregraf Horde" { "CreateTokens" } else { "Exile" }), "{json}");
    }
}

#[test]
fn any_player_cast_triggers_keep_their_population_including_the_additional_vision_caller() {
    for name in ["Adrenaline Jockey", "Glademuse", "Roiling Vortex", "Scytheclaw Raptor", "Vision, Synthezoid Avenger"] {
        let card = lower_line(source_line(name));
        let mut casts = 0;
        for ability in &card.abilities {
            if let Ability::Triggered { event: EventSpec::Spell { players, .. }, .. } = ability {
                assert!(matches!(players, PlayerSet::All), "{name}: {players:?}");
                casts += 1;
            }
        }
        assert_eq!(casts, 1, "{name}");
    }
}
