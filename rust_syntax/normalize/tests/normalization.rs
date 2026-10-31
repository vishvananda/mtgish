use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard, report, validate};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

/// Hint of Insanity's reviewed row names that card; under any other name its
/// nested form has no review, so these guards still see an unknown behavior.
fn unreviewed() -> String {
    include_str!("fixtures/core.ronl")
        .lines()
        .find(|l| l.contains("Name: \"Hint of Insanity\""))
        .unwrap()
        .replace("Hint of Insanity", "Unreviewed Insanity")
}

fn source(name: &str) -> OracleCard {
    include_str!("fixtures/core.ronl")
        .lines()
        .find_map(|line| {
            let value: OracleCard = ron::from_str(line).unwrap();
            let identity = corpus::Identity::of(&value).unwrap();
            identity.names.iter().any(|n| n == name).then_some(value)
        })
        .unwrap_or_else(|| panic!("missing fixture {name}"))
}

fn normalized(name: &str) -> Card {
    let card = lower::card(&source(name)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    validate::card(&card).unwrap();
    card
}

#[test]
fn normalization_real_cards_use_shared_operations_and_valid_scopes() {
    for name in [
        "Air Elemental",
        "Divination",
        "Lightning Bolt",
        "Doom Blade",
        "Unsummon",
        "Murder",
        "Divine Offering",
        "Shivan Dragon",
        "Might of the Meek",
        "Kessig Wolf Run",
        "Subversive Acolyte",
    ] {
        let card = normalized(name);
        let json = serde_json::to_vec(&card).unwrap();
        let loaded: Card = serde_json::from_slice(&json).unwrap();
        validate::card(&loaded).unwrap();
        let ron = ron::ser::to_string(&card).unwrap();
        let from_ron: Card = ron::from_str(&ron).unwrap();
        validate::card(&from_ron).unwrap();
        assert_eq!(
            serde_json::to_value(&card).unwrap(),
            serde_json::to_value(&loaded).unwrap()
        );
    }
}

#[test]
fn normalization_preserves_divine_offering_information_without_success_gate() {
    let card = normalized("Divine Offering");
    let Ability::Spell { announcement, body } = &card.abilities[0] else {
        panic!()
    };
    assert_eq!(announcement.targets.len(), 1);
    assert!(matches!(
        &body.instructions[0],
        Instruction::Effect {
            operation: Operation::Destroy {
                objects: ObjectSet::One(ObjectRef::Target { slot: Slot(0), .. }),
                ..
            },
            ..
        }
    ));
    // Gain life is unconditional on successful destruction and reads the same
    // object's current/last-known mana value after the destroy instruction.
    assert!(matches!(
        &body.instructions[1],
        Instruction::Effect {
            operation: Operation::GainLife {
                amount: Expr::ObjectProperty {
                    object: ObjectRef::Target { slot: Slot(0), .. },
                    property: Property::ManaValue,
                    information: Information::CurrentOrLastKnown
                },
                ..
            },
            ..
        }
    ));
}

#[test]
fn normalization_subversive_acolyte_separates_type_change_from_pump() {
    let card = normalized("Subversive Acolyte");
    let Ability::Activated {
        announcement,
        limits,
        ..
    } = &card.abilities[0]
    else {
        panic!()
    };
    assert!(limits.once);
    let modes = announcement.modes.as_ref().unwrap();
    let body = &modes.modes[0].body;
    assert_eq!(body.instructions.len(), 2);
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { changes, .. },
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        changes.as_slice(),
        [Change::CreatureTypes {
            mode: ChangeMode::Set,
            ..
        }]
    ));
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { changes, .. },
        ..
    } = &body.instructions[1]
    else {
        panic!()
    };
    assert!(matches!(
        changes.as_slice(),
        [Change::Pump { .. }, Change::GrantKeyword(Keyword::Lifelink)]
    ));
}

#[test]
fn normalization_kessig_preserves_x_and_trample_together() {
    let card = normalized("Kessig Wolf Run");
    let Ability::Activated { body, cost, .. } = &card.abilities[1] else {
        panic!()
    };
    assert!(
        matches!(&cost.parts[0], CostPart::Mana(symbols) if matches!(symbols[0], ManaSymbol::X))
    );
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { changes, .. },
        ..
    } = &body.instructions[0]
    else {
        panic!()
    };
    assert!(matches!(
        changes.as_slice(),
        [
            Change::Pump {
                power: Expr::AnnouncedX,
                toughness: Expr::Constant(0)
            },
            Change::GrantKeyword(Keyword::Trample)
        ]
    ));
}

#[test]
fn normalization_rejects_unbound_targets_and_duplicate_results() {
    let mut card = normalized("Divine Offering");
    let Ability::Spell { body, .. } = &mut card.abilities[0] else {
        panic!()
    };
    let first = body.instructions[0].clone();
    body.instructions.push(first);
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("duplicate result"));
    let Ability::Spell { announcement, body } = &mut card.abilities[0] else {
        panic!()
    };
    body.instructions.pop();
    announcement.targets.clear();
    assert!(validate::card(&card)
        .unwrap_err()
        .contains("unbound target"));
}

#[test]
fn normalization_does_not_mark_unknown_nested_behavior_complete() {
    let card: OracleCard = ron::from_str(&unreviewed()).unwrap();
    let errors = lower::card(&card).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| e.form == "DiscardEachCard" && e.path.contains("actions[1]")));
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "mtgish-ast-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn normalization_late_input_failure_preserves_published_artifact() {
    let dir = Scratch::new();
    let input = dir.0.join("source.ronl");
    let output = dir.0.join("output.ronl");
    let valid = include_str!("fixtures/core.ronl")
        .lines()
        .find(|line| line.contains("Name: \"Divination\""))
        .unwrap();
    fs::write(&input, format!("{valid}\ninvalid second record\n")).unwrap();
    fs::write(&output, b"previous artifact").unwrap();
    let error = report::export_ron(&input, &output).unwrap_err().to_string();
    assert!(error.contains(":2:"), "{error}");
    assert_eq!(fs::read(&output).unwrap(), b"previous artifact");
    assert_eq!(
        fs::read_dir(&dir.0).unwrap().count(),
        2,
        "temporary artifact was not cleaned up"
    );
}

#[test]
fn normalization_late_unsupported_card_cannot_publish_partial_corpus() {
    let dir = Scratch::new();
    let input = dir.0.join("source.ronl");
    let output = dir.0.join("output.ronl");
    let lines = include_str!("fixtures/core.ronl");
    let valid = lines
        .lines()
        .find(|l| l.contains("Name: \"Divination\""))
        .unwrap();
    let unsupported = unreviewed();
    fs::write(&input, format!("{valid}\n{unsupported}\n")).unwrap();
    assert!(report::export_ron(&input, &output).is_err());
    assert!(!output.exists());
}

#[test]
fn normalization_baseline_detects_replacement_even_when_counts_match() {
    let first = corpus::Identity::of(&source("Divination")).unwrap();
    let second = corpus::Identity::of(&source("Murder")).unwrap();
    let make = |records| corpus::Manifest {
        manifest_version: 1,
        corpus_sha256: String::new(),
        inputs: Default::default(),
        records,
        forms: Default::default(),
    };
    let baseline = make(vec![first.clone(), second.clone()]);
    assert!(make(vec![second, first.clone()])
        .check_identities(&baseline)
        .is_ok());
    assert!(make(vec![first.clone(), first])
        .check_identities(&baseline)
        .is_err());
}
