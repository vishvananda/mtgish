#![cfg(feature = "cantrip")]
//! Rules a resolving spell or ability gives an open set of objects (CR 611.2c),
//! and "doesn't untap during its controller's next untap step": Cantrip's
//! `lasting-rules` (cantrip-rs/cantrip-rs#897). The code is the same whether the
//! pinned Cantrip has the feature or not; without it, it is held in holes. Each
//! test writes one form with `planned::with_features`, which stands in for the
//! snapshot's `FEATURES`. It reads the corpus and the printed words for these
//! cards alone, and has nothing to say without them.
use mtgish_ast::cantrip::{self, planned};
use std::{collections::BTreeMap, path::PathBuf};

const LASTING_RULES: &[&str] = &["lasting-rules"];

/// The card, what its module has to say with the feature, and whether it is whole then.
const CARDS: &[(&str, &str, bool)] = &[
    // "Creatures without flying can't block this turn": a creature that
    // arrives later is in the set too.
    ("Falter", "rule: LastingRuleKind::CantBlock", true),
    ("Falter", "g.add_lasting_rule(item, 0, about_0, until_0);", true),
    ("Goblin Diplomats", "rule: LastingRuleKind::MustAttack", true),
    ("Goblin Diplomats", "text: \"Each creature attacks this turn if able.\"", true),
    // The target opponent is what the rule reads of the resolution, and
    // "their next untap step" is that player's.
    ("Exhaustion", "rule: LastingRuleKind::DoesntUntap", true),
    ("Exhaustion", "about.player() == Some(q.controller(id))", true),
    ("Exhaustion", "Duration::DuringNextUntapStepOf(", true),
    // A rule's words are its sentence of the card: the kicked spell's, the
    // loyalty ability's past its cost, the mode's.
    (
        "Orim's Chant",
        "text: \"If this spell was kicked, creatures can't attack this turn.\"",
        true,
    ),
    (
        "Jace, Arcane Strategist",
        "text: \"Creatures you control can't be blocked this turn.\"",
        true,
    ),
    (
        "Reverse the Polarity",
        "text: \"Creatures can't be blocked this turn.\"",
        true,
    ),
    // "Other creatures": the target creature is the one left out.
    ("Intimidation Bolt", "!(about.object() == Some(id))", true),
    // Attacking creatures are the ones attacking as it resolves (Tangle's
    // ruling), each waiting on its own controller's untap step.
    ("Tangle", "EffectKind::DoesntUntap", true),
    ("Tangle", "Duration::DuringItsControllersNextUntapStep", true),
    // Under a change of control the permanent waits on its new controller's
    // untap step, which exerting wouldn't (Frost Titan's ruling).
    ("Frost Titan", "EffectKind::DoesntUntap", true),
    ("Frost Titan", "Duration::DuringItsControllersNextUntapStep", true),
    // "For as long as you control Icefall Regent": the same rule, lasting.
    ("Icefall Regent", "EffectKind::DoesntUntap", true),
    // A creature the target opponent chose, and that player: two things the
    // ability chose, which a rule's `about` can't both hold.
    ("Mark for Death", "a-lasting-rule-reading-two-things-its-ability-chose", false),
    ("Goblin War Cry", "a-lasting-rule-reading-what-its-ability-chose", false),
];

/// The cards whose whole file is one Cantrip's PR commits, as it commits it.
const COMMITTED: &[(&str, &str)] = &[
    ("Falter", include_str!("fixtures/open_rules/falter.rs")),
    ("Goblin Diplomats", include_str!("fixtures/open_rules/goblin_diplomats.rs")),
    ("Exhaustion", include_str!("fixtures/open_rules/exhaustion.rs")),
    ("Tangle", include_str!("fixtures/open_rules/tangle.rs")),
];

fn data() -> Option<(PathBuf, PathBuf)> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let (corpus, oracle) = (data.join("mtgish.lines.ron"), data.join("oracle.json"));
    (corpus.exists() && oracle.exists()).then_some((corpus, oracle))
}

/// Each card's module, written for a Cantrip with `features`.
fn modules(features: &[&str]) -> Option<BTreeMap<String, String>> {
    let (corpus, oracle) = data()?;
    let mut names: Vec<String> = CARDS.iter().map(|(name, ..)| name.to_string()).collect();
    names.dedup();
    let modules = planned::with_features(features, || cantrip::modules_printed(&corpus, &oracle, &names));
    Some(modules.unwrap().into_iter().collect())
}

/// What a module says once its holes are filled: each hole's code where the
/// hole stood. A hole is `planned!("id" => code);`, `planned_item!("id" =>
/// item);` or `planned_value!("feature", placeholder => code)`.
fn filled(module: &str) -> String {
    let mut out = String::new();
    let mut rest = module;
    loop {
        let at = ["planned!(\"", "planned_item!(\"", "planned_value!(\""]
            .iter()
            .filter_map(|mark| rest.find(mark).map(|at| (at, *mark)))
            .min();
        let Some((at, mark)) = at else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..at]);
        let body = &rest[at + mark.len() - 1..];
        let end = closing(body);
        let inner = &body[..end];
        let code = &inner[inner.find(" => ").expect("a hole with code") + 4..];
        out.push_str(&filled(code));
        rest = &body[end + 1..];
        // A statement's hole ends `);`, as its code already does.
        if mark != "planned_value!(\"" {
            rest = rest.strip_prefix(';').unwrap_or(rest);
        }
    }
}

/// Where the parenthesis open before `text` closes, strings stepped over.
fn closing(text: &str) -> usize {
    let (mut depth, mut quoted, mut escaped) = (1, false, false);
    for (at, c) in text.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quoted => escaped = true,
            '"' => quoted = !quoted,
            _ if quoted => {}
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth -= 1;
                if depth == 0 {
                    return at;
                }
            }
            _ => {}
        }
    }
    panic!("unclosed hole in {text}")
}

#[test]
fn with_lasting_rules_modules_say_what_their_cards_say() {
    let Some(modules) = modules(LASTING_RULES) else { return };
    for (name, says, whole) in CARDS {
        let module = modules.get(*name).unwrap_or_else(|| panic!("no module for {name}"));
        assert!(module.contains(says), "{name} should have {says}:\n{module}");
        if *whole {
            assert!(!module.contains("planned"), "{name} should be whole:\n{module}");
        }
    }
}

/// Without the feature the code is the same, in holes: never the rejected
/// `add_open_effect`, and never an exert where a change of control matters.
#[test]
fn without_lasting_rules_the_same_code_is_in_holes() {
    let Some(holes) = modules(&[]) else { return };
    let Some(whole) = modules(LASTING_RULES) else { return };
    for (name, _, is_whole) in CARDS {
        let module = &holes[*name];
        assert!(!module.contains("add_open_effect"), "{name}:\n{module}");
        assert!(!module.contains("g.exert("), "{name}:\n{module}");
        if !*is_whole {
            continue;
        }
        assert!(module.contains("planned"), "{name} should wait on the feature:\n{module}");
        // A CardDef field can't be a hole: the table is written beside it.
        let whole = whole[*name].replace("lasting_rules: LASTING_RULES, ", "");
        assert_eq!(filled(module), whole, "{name}'s holes should hold its code");
    }
    for name in ["Falter", "Goblin Diplomats", "Exhaustion", "Orim's Chant"] {
        let module = &holes[name];
        assert!(
            module.contains("planned!(\"engine-no-resolving-rule-on-an-open-set-") && module.contains("=> g.add_lasting_rule(item, 0, about_0, until_0);)"),
            "{name}:\n{module}"
        );
        assert!(module.contains("planned_item!(\"engine-no-resolving-rule-on-an-open-set-") && module.contains("=> static LASTING_RULES: &[LastingRule] = &["), "{name}:\n{module}");
        assert!(!module.contains("lasting_rules: LASTING_RULES"), "{name}:\n{module}");
    }
    for name in ["Tangle", "Frost Titan", "Icefall Regent"] {
        let module = &holes[name];
        assert!(module.contains("planned_value!(\"continuous rule UntapStep\", EffectKind::CantBlock => EffectKind::DoesntUntap)"), "{name}:\n{module}");
    }
}

/// With the feature, a whole card's file is the one Cantrip's PR commits.
#[test]
fn with_lasting_rules_a_card_file_is_the_one_committed() {
    let Some((corpus, oracle)) = data() else { return };
    let tmp = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let (subset, out) = (tmp.join("open_rules.ron"), tmp.join("open_rules.jsonl"));
    let lines: Vec<String> = std::fs::read_to_string(&corpus)
        .unwrap()
        .lines()
        .filter(|line| COMMITTED.iter().any(|(name, _)| line.contains(&format!("Name: \"{name}\""))))
        .map(str::to_string)
        .collect();
    std::fs::write(&subset, lines.join("\n") + "\n").unwrap();
    planned::with_features(LASTING_RULES, || planned::tracked(&subset, &oracle, &out)).unwrap();
    let files: BTreeMap<String, String> = std::fs::read_to_string(&out)
        .unwrap()
        .lines()
        .map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            (row["name"].as_str().unwrap().to_string(), row["file"].as_str().unwrap_or("").to_string())
        })
        .collect();
    for (name, committed) in COMMITTED {
        let file = files.get(*name).unwrap_or_else(|| panic!("no row for {name}"));
        assert_eq!(file.trim_end(), committed.trim_end(), "{name}");
    }
}
