#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, ir::*, lower, mtg_types::OracleCard};
use std::{fs, path::PathBuf, process::Command};
fn card(name: &str) -> Card {
    let source: OracleCard = include_str!("fixtures/core.ronl")
        .lines()
        .chain(include_str!("fixtures/choices.ronl").lines())
        .chain(include_str!("fixtures/tokens.ronl").lines())
        .chain(include_str!("fixtures/mana.ronl").lines())
        .chain(include_str!("fixtures/backend.ronl").lines())
        .map(|s| ron::from_str(s).unwrap())
        .find(|s| {
            corpus::Identity::of(s)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    lower::card(&source).unwrap_or_else(|e| panic!("{name}: {e:?}"))
}
/// Where nested builds of generated crates go: under this build's target, so a
/// separate worktree does not share or contend for one Cantrip build.
fn generated_target() -> PathBuf {
    std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("mtgish-cantrip-build")
}
fn verify_cantrip_snapshot(checkout: &std::path::Path) {
    let result = Command::new("python3")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools/update_cantrip_vocabulary.py"))
        .arg(checkout)
        .arg("--check")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn backend_capabilities_do_not_change_ast_coverage_or_publish_partial_code() {
    let mut kessig = card("Kessig Wolf Run");
    let Ability::Activated { limits, .. } = &mut kessig.abilities[1] else {
        panic!()
    };
    limits.activators = Some(PlayerSet::All);
    let error = cantrip::module(&kessig).unwrap_err();
    assert!(error.path.contains("abilities[1]"), "{error:?}");
    assert_eq!(error.feature, "activation by other players");
    let root = std::env::temp_dir().join(format!("mtgish-backend-atomic-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let (input, output) = (root.join("cards.ronl"), root.join("cards.rs"));
    fs::write(&output, "previous artifact").unwrap();
    fs::write(
        &input,
        format!(
            "{}\n{}\n",
            ron::ser::to_string(&card("Divination")).unwrap(),
            ron::ser::to_string(&kessig).unwrap()
        ),
    )
    .unwrap();
    assert!(cantrip::export(&input, &output).is_err());
    assert_eq!(fs::read_to_string(&output).unwrap(), "previous artifact");
    fs::remove_dir_all(root).unwrap();
}
#[test]
fn numeric_bounds_are_checked_before_rust_is_published() {
    let mut c = card("Divination");
    let Ability::Spell { body, .. } = &mut c.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::Draw { amount, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *amount = Expr::Constant(i64::MAX);
    assert!(cantrip::module(&c)
        .unwrap_err()
        .feature
        .contains("numeric range"));
}
#[test]
fn unsupported_limits_are_not_omitted_and_one_group_is_one_sentence() {
    let mut drake = card("Peregrine Drake");
    let Ability::Triggered { limits, .. } = &mut drake.abilities[1] else {
        panic!()
    };
    limits.total = Some(1);
    assert_eq!(
        cantrip::module(&drake).unwrap_err().feature,
        "trigger total limit"
    );

    let mut shivan = card("Shivan Dragon");
    let Ability::Activated { body, .. } = &mut shivan.abilities[1] else {
        panic!()
    };
    let first = body.instructions[0].clone();
    let mut second = first.clone();
    let Instruction::Effect { result, .. } = &mut second else {
        panic!()
    };
    result.0 += 1;
    body.instructions = vec![Instruction::Group(Program {
        instructions: vec![first, second],
    })];
    let rust = cantrip::module(&shivan).unwrap();
    assert_eq!(rust.matches("g.end_sentence();").count(), 1);
}
/// Actual Cantrip core execution is an explicit integration check, so ordinary
/// lowerer edits do not repeatedly compile the sibling workspace.
#[test]
#[ignore = "run with --ignored to compile and execute generated callbacks against ../cantrip"]
fn generated_modules_execute_in_cantrip() {
    let cantrip_root = std::env::var_os("MTGISH_CANTRIP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../cantrip"));
    let cantrip_root = cantrip_root
        .canonicalize()
        .expect("Cantrip checkout (or MTGISH_CANTRIP_DIR)");
    verify_cantrip_snapshot(&cantrip_root);
    let root =
        std::env::temp_dir().join(format!("mtgish-cantrip-scenarios-{}", std::process::id()));
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest="[package]\nname=\"mtgish-generated-cantrip-check\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in [
        "cantrip-api",
        "cantrip-mechanics",
        "cantrip-rules",
        "cantrip-state",
    ] {
        manifest.push_str(&format!(
            "{dep}={{path={:?}}}\n",
            cantrip_root.join("crates").join(dep).to_string_lossy()
        ));
    }
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let names = [
        ("divination", "Divination"),
        ("shivan", "Shivan Dragon"),
        ("offering", "Divine Offering"),
        ("unsummon", "Unsummon"),
        ("murder", "Murder"),
        ("drake", "Peregrine Drake"),
        ("whale", "Great Whale"),
        ("palinchron", "Palinchron"),
        ("alarm", "Raise the Alarm"),
        ("menace", "Bestial Menace"),
        ("shadow", "Shadow Summoning"),
        ("bake", "Bake into a Pie"),
        ("servo", "Servo Exhibition"),
        ("posse", "Form a Posse"),
        ("pair", "Prickly Pair"),
        ("elves", "Llanowar Elves"),
        ("birds", "Birds of Paradise"),
        ("ring", "Sol Ring"),
        ("lotus", "Gilded Lotus"),
        ("ritual", "Dark Ritual"),
        ("morph", "Manamorphose"),
        ("sphere", "Chromatic Sphere"),
        ("cradle", "Gaea's Cradle"),
        ("opportunist", "Argothian Opportunist"),
        ("workshop", "Mishra's Workshop"),
        ("hawker", "Omen Hawker"),
        ("signet", "Selesnya Signet"),
        ("pirates", "Prosperous Pirates"),
        ("visionary", "Elvish Visionary"),
        ("traveler", "Doomed Traveler"),
        ("anthem", "Glorious Anthem"),
        ("strength", "Holy Strength"),
        ("command", "Kolaghan's Command"),
        ("raise", "Raise Dead"),
        ("tutor", "Diabolic Tutor"),
        ("gate", "Azorius Guildgate"),
        ("seer", "Viscera Seer"),
        ("splitter", "Bonesplitter"),
        ("cave", "Forgotten Cave"),
        ("bolt", "Lightning Bolt"),
        ("blade", "Doom Blade"),
        ("bears", "Grizzly Bears"),
        ("melee", "Arcane Melee"),
        ("crucible", "Crucible of Worlds"),
        ("mending", "Mending Hands"),
        ("blink", "Blink of an Eye"),
        ("looting", "Faithless Looting"),
        ("reap", "Altar's Reap"),
        ("wastes", "Adarkar Wastes"),
        ("cliffs", "Blackcleave Cliffs"),
        ("impulse", "Act on Impulse"),
        ("cindermaw", "Giant Cindermaw"),
        ("ghostway", "Ghostway"),
        ("wine", "Blessed Wine"),
        ("index", "Index"),
        ("seal", "Imperial Seal"),
        ("impulse_look", "Impulse"),
        ("stirrings", "Ancient Stirrings"),
        ("litho", "Lithobraking"),
        ("agent", "Agent of the Fates"),
        ("priest", "Banisher Priest"),
        ("spitemare", "Spitemare"),
        ("faith", "Renewed Faith"),
        ("golem", "Flint Golem"),
        ("good", "Greater Good"),
        ("arrest", "Arrest"),
        ("pulse", "Maelstrom Pulse"),
        ("zealot", "Ash Zealot"),
        ("sunderflock", "Sunderflock"),
        ("rhino", "Charging Rhino"),
        ("acolyte", "Cathedral Acolyte"),
        ("overlord", "Abhorrent Overlord"),
        ("decay", "Abrupt Decay"),
        ("woodland", "Woodland Changeling"),
        ("lignify", "Lignify"),
        ("law", "Absolute Law"),
        ("burrowing", "Burrowing"),
        ("depths", "Dark Depths"),
        ("endless", "Endless One"),
        ("detonate", "Detonate"),
        ("journey", "Journey to Nowhere"),
        ("falconer", "Falconer Adept"),
        ("rage", "Monstrous Rage"),
        ("skystreak", "Skystreak Engineer"),
        ("caress", "Caress of Phyrexia"),
        ("arc", "Arc Lightning"),
        ("glimpse", "Glimpse of Nature"),
        ("mark", "Make Your Mark"),
        ("wish", "Make a Wish"),
        ("cultivator", "Nantuko Cultivator"),
        ("fling", "Fling"),
        ("truths", "Painful Truths"),
        ("strength_two", "Consume Strength"),
        ("salvo", "Blazing Salvo"),
    ];
    let mut lib = String::new();
    for (module, name) in names {
        lib.push_str(&format!(
            "mod {module} {{\n{}\n}}\n",
            cantrip::module(&card(name)).unwrap()
        ));
    }
    // Exercise the complete Rust export and registration function as well as
    // individual modules. Same-named tokens with different characteristics must
    // retain distinct definitions in Cantrip's table.
    let mut exported_cards = [
        "Raise the Alarm",
        "Bestial Menace",
        "Shadow Summoning",
        "Bake into a Pie",
        "Servo Exhibition",
    ]
    .map(card)
    .to_vec();
    let mut larger = card("Raise the Alarm");
    larger.name = "Larger Soldier probe".into();
    let Ability::Spell { body, .. } = &mut larger.abilities[0] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateTokens { tokens, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    tokens[0].definition_mut().unwrap().power_toughness =
        Some((Expr::Constant(2), Expr::Constant(2)));
    exported_cards.push(larger);
    let input = root.join("exported.ronl");
    let output = root.join("exported.rs");
    fs::write(
        &input,
        exported_cards
            .iter()
            .map(|c| ron::ser::to_string(c).unwrap())
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    assert_eq!(cantrip::export(&input, &output).unwrap(), 6);
    lib.push_str(&format!(
        "mod exported {{\n{}\n}}\n",
        fs::read_to_string(output).unwrap()
    ));
    // A generic program probe exercises replay with mutations before two
    // prompts. It is deliberately not presented as a printed card.
    let mut probe = card("Peregrine Drake");
    probe.name = "Replay probe".into();
    let Ability::Triggered { body, .. } = &mut probe.abilities[1] else {
        panic!()
    };
    let group = body.instructions[0].clone();
    let mut second = group.clone();
    let Instruction::Group(inner) = &mut second else {
        panic!()
    };
    for instruction in &mut inner.instructions {
        let Instruction::Effect { operation, result } = instruction else {
            panic!()
        };
        result.0 += 100;
        if let Operation::Tap {
            objects: ObjectSet::Selected(selected),
            ..
        } = operation
        {
            selected.0 += 100;
        }
    }
    body.instructions = vec![
        Instruction::Effect {
            result: ResultId(999),
            operation: Operation::Draw {
                players: PlayerSet::One(PlayerRef::AbilityController),
                amount: Expr::Constant(1),
            },
        },
        group,
        second,
    ];
    lib.push_str(&format!(
        "mod replay_probe {{\n{}\n}}\n",
        cantrip::module(&probe).unwrap()
    ));
    let mut pump = card("Shivan Dragon");
    pump.name = "Collection pump probe".into();
    let Ability::Activated { body, .. } = &mut pump.abilities[1] else {
        panic!()
    };
    let Instruction::Effect {
        operation: Operation::CreateContinuousEffect { affected, .. },
        ..
    } = &mut body.instructions[0]
    else {
        panic!()
    };
    *affected = ObjectSet::Battlefield(ObjectFilter::All(vec![
        ObjectFilter::CardType(mtgish_ast::mtg_types::CardType::Creature),
        ObjectFilter::ControlledBy(PlayerSet::One(PlayerRef::AbilityController)),
    ]))
    .into();
    lib.push_str(&format!(
        "mod collection_pump {{\n{}\n}}\n",
        cantrip::module(&pump).unwrap()
    ));
    lib.push_str(include_str!("fixtures/cantrip_scenarios.rs"));
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    let result = Command::new("cargo")
        .args(["test", "--offline", "--manifest-path"])
        .env("CANTRIP_VERIFY", "1")
        .arg(root.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(generated_target())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "generated crate at {}\n{}\n{}",
        root.display(),
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(root).unwrap();
}

/// Each card on its own in a table, so one that a table refuses names itself.
const LOADS: &str = r#"
#[test]
fn every_definition_loads_into_a_card_table() {
    std::panic::set_hook(Box::new(|_| {}));
    let mut refused = Vec::new();
    for &(card, extra) in ALL {
        let definitions: Vec<&'static cantrip_api::CardDef> =
            std::iter::once(card).chain(extra.iter().copied()).collect();
        if let Err(why) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cantrip_api::CardTable::new(definitions))) {
            let why = why
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| why.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            refused.push(format!("{}: {why}", card.name));
        }
    }
    let _ = std::panic::take_hook();
    assert!(refused.is_empty(), "{} refused\n{}", refused.len(), refused.join("\n"));
}
"#;

/// Checks every advertised backend capability against the actual API, using the
/// cached report to avoid decoding the unsupported legacy records again.
#[test]
#[ignore = "set MTGISH_AST_COVERAGE to a fresh ast check report and run with --ignored"]
fn every_advertised_module_compiles_against_cantrip() {
    let report_path =
        std::env::var_os("MTGISH_AST_COVERAGE").expect("MTGISH_AST_COVERAGE report path");
    let report: mtgish_ast::report::Report =
        serde_json::from_slice(&fs::read(report_path).unwrap()).unwrap();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let source = fs::read(root.join("data/mtgish.lines.ron")).unwrap();
    assert_eq!(report.schema_version, SCHEMA_VERSION);
    assert!(
        report.backend_checked,
        "backend compilation needs an explicit check-cantrip report"
    );
    assert_eq!(report.corpus_sha256, corpus::digest(&source));
    assert_eq!(
        report.inputs,
        mtgish_ast::pipeline::input_fingerprints(&root).unwrap(),
        "coverage report must match current lowering and generator inputs"
    );
    let lines = std::str::from_utf8(&source)
        .unwrap()
        .lines()
        .collect::<Vec<_>>();
    let scratch = std::env::temp_dir().join(format!(
        "mtgish-cantrip-whole-capability-{}",
        std::process::id()
    ));
    fs::create_dir_all(scratch.join("src")).unwrap();
    let cantrip_root = std::env::var_os("MTGISH_CANTRIP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("../cantrip"))
        .canonicalize()
        .unwrap();
    verify_cantrip_snapshot(&cantrip_root);
    let manifest = format!(
        "[package]\nname=\"mtgish-all-capable-modules\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\ncantrip-api={{path={:?}}}\ncantrip-mechanics={{path={:?}}}\n",
        cantrip_root.join("crates/cantrip-api").to_string_lossy(),
        cantrip_root
            .join("crates/cantrip-mechanics")
            .to_string_lossy()
    );
    fs::write(scratch.join("Cargo.toml"), manifest).unwrap();
    let mut rust = String::from("#![allow(unused_variables,dead_code)]\n");
    let mut generated = 0;
    let mut loaded = String::new();
    // Compiling every module takes minutes. While a family is being added, an
    // older report narrows this to what is newly capable since then.
    let since = std::env::var_os("MTGISH_CANTRIP_SINCE").map(|path| {
        let older: mtgish_ast::report::Report =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        older
            .records
            .iter()
            .filter(|r| r.backend_capable)
            .map(|r| r.line)
            .collect::<std::collections::BTreeSet<_>>()
    });
    let skipped = since.as_ref().map_or(0, |older| {
        report
            .records
            .iter()
            .filter(|r| r.backend_capable && older.contains(&r.line))
            .count()
    });
    let wanted = |line: usize| since.as_ref().map_or(true, |older| !older.contains(&line));
    for record in report
        .records
        .iter()
        .filter(|r| r.backend_capable && wanted(r.line))
    {
        let parsed: OracleCard = ron::from_str(lines[record.line - 1]).unwrap();
        let card = lower::card(&parsed).unwrap();
        rust.push_str(&format!(
            "pub mod card_{} {{\n{}\n}}\n",
            record.line,
            cantrip::module(&card).unwrap()
        ));
        loaded.push_str(&format!(
            "(&card_{0}::CARD, card_{0}::EXTRA),\n",
            record.line
        ));
        generated += 1;
    }
    assert_eq!(generated + skipped, report.backend_capable);
    assert!(generated > 0);
    // A card table asserts what no type does. Building and running that takes
    // far longer than checking, so it is asked for.
    let load = std::env::var_os("MTGISH_CANTRIP_LOAD").is_some();
    if load {
        rust.push_str(&format!(
            "static ALL: &[(&cantrip_api::CardDef, &[&cantrip_api::CardDef])] = &[\n{loaded}];\n{LOADS}"
        ));
    }
    fs::write(scratch.join("src/lib.rs"), rust).unwrap();
    let verb = if load { "test" } else { "check" };
    let result = Command::new("cargo")
        .args([verb, "--offline", "--manifest-path"])
        .arg(scratch.join("Cargo.toml"))
        .arg("--target-dir")
        .arg(generated_target())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "generated crate at {}\n{}\n{}",
        scratch.display(),
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn mana_cost_parts_are_combined_before_checking_api_bounds() {
    let mut card = card("Shivan Dragon");
    let Ability::Activated { cost, .. } = &mut card.abilities[1] else {
        panic!()
    };
    cost.parts = vec![
        CostPart::Mana(vec![ManaSymbol::Generic(130)]),
        CostPart::Mana(vec![ManaSymbol::Generic(130)]),
    ];
    assert!(cantrip::module(&card)
        .unwrap_err()
        .feature
        .contains("mana cost exceeds"));
}
