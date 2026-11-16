//! Complete amended faces, not name-specific substitutions in production.
use mtgish_ast::{canonical, corpus, ir::UnifiedCard, mtg_types::OracleCard, unified, validate};
use std::collections::BTreeMap;
fn key(card: &UnifiedCard) -> Vec<String> {
    let mut names = card
        .faces
        .iter()
        .map(|f| f.name.clone())
        .collect::<Vec<_>>();
    names.sort();
    names
}
fn meaning(card: &UnifiedCard) -> serde_json::Value {
    // Face arrays are not a primary-face ordering contract. Compare each
    // complete ability list under its immutable face name.
    let mut faces = card.faces.iter().collect::<Vec<_>>();
    faces.sort_by(|a, b| a.name.cmp(&b.name));
    let mut abilities =
        serde_json::to_value(faces.iter().map(|f| &f.abilities).collect::<Vec<_>>()).unwrap();
    for face in abilities.as_array_mut().unwrap() {
        canonical::semantic_abilities(face).unwrap();
    }
    abilities
}
#[test]
fn amendments_align_with_only_explicitly_reported_remaining_differences() {
    corpus::on_large_stack(|| {
        let expected = include_str!("fixtures/grammar_adoption_expected.jsonl")
            .lines()
            .map(|l| {
                let c: UnifiedCard = serde_json::from_str(l).unwrap();
                (key(&c), c)
            })
            .collect::<BTreeMap<_, _>>();
        let remaining: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/grammar_adoption_remaining.json")).unwrap();
        let mut errors = vec![];
        let mut checked = 0;
        for line in include_str!("fixtures/grammar_adoption.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let actual = unified::lower(&source);
            assert!(
                actual.diagnostics.is_empty(),
                "{:?}: {:?}",
                key(&actual.card),
                actual.diagnostics
            );
            validate::unified(&actual.card).unwrap();
            unified::round_trip(&actual.card).unwrap();
            let names = key(&actual.card);
            let Some(gold) = expected.get(&names) else {
                errors.push(format!("Missing reference for {names:?}"));
                continue;
            };
            let (got, mut want) = (meaning(&actual.card), meaning(gold));
            checked += 1;
            // Keep unresolved reference/normalization differences explicit and
            // input checked. Neither production comparison nor gold is repaired.
            for entry in remaining.as_array().unwrap() {
                if entry["names"] != serde_json::to_value(&names).unwrap() {
                    continue;
                }
                for change in entry["changes"].as_array().unwrap() {
                    let path = change["path"].as_str().unwrap();
                    assert_eq!(
                        want.pointer(path),
                        Some(&change["reference"]),
                        "{names:?}: original reference at {path}"
                    );
                    assert_eq!(
                        got.pointer(path),
                        Some(&change["source"]),
                        "{names:?}: reviewed Source at {path}"
                    );
                    *want.pointer_mut(path).unwrap() = change["source"].clone();
                }
            }
            if got != want {
                errors.push(names.join(" // "));
                if let Ok(root) = std::env::var("GRAMMAR_ADOPTION_EVIDENCE") {
                    std::fs::create_dir_all(&root).unwrap();
                    std::fs::write(
                        format!("{root}/{checked}.json"),
                        serde_json::to_vec_pretty(
                            &serde_json::json!({"names":names,"actual":got,"expected":want}),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                }
            }
        }
        assert_eq!(checked, expected.len());
        assert!(
            errors.is_empty(),
            "{} disagreeing records: {}",
            errors.len(),
            errors.join(", ")
        );
    });
}

#[test]
fn overabundance_gives_the_tapper_mana_and_keeps_the_enchantment_damage_source() {
    corpus::on_large_stack(|| {
        let source: OracleCard =
            ron::from_str(include_str!("fixtures/overabundance_023.ronl").trim()).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty());
        validate::unified(&lowered.card).unwrap();
        let body = serde_json::to_value(&lowered.card.faces[0].abilities[0]).unwrap();
        let body = &body["args"]["body"]["instructions"];
        assert_eq!(body[0]["args"]["operation"]["kind"], "AddMana");
        assert_eq!(
            body[0]["args"]["operation"]["args"]["players"],
            serde_json::json!({"kind":"One","args":{"kind":"EventSubject"}})
        );
        assert_eq!(body[1]["args"]["operation"]["kind"], "DealDamage");
        assert_eq!(
            body[1]["args"]["operation"]["args"]["source"],
            serde_json::json!({"kind":"Source"})
        );
    });
}

#[test]
fn player_iteration_keeps_your_library_and_linked_card_power_keeps_vehicles() {
    corpus::on_large_stack(|| {
        for line in include_str!("fixtures/grammar_023_controls.ronl").lines() {
            let source: OracleCard = ron::from_str(line).unwrap();
            let lowered = unified::lower(&source);
            assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
            validate::unified(&lowered.card).unwrap();
            let face = &lowered.card.faces[0];
            let abilities = serde_json::to_value(&face.abilities).unwrap();
            match face.name.as_str() {
                "Protection Racket" => {
                    let policy = &abilities[0]["args"]["body"]["instructions"][0]["args"]
                        ["operation"]["args"];
                    let body = &policy["body"][0]["args"]["instructions"];
                    assert_eq!(body[0]["args"]["operation"]["kind"], "Reveal");
                    assert_eq!(
                        body[0]["args"]["operation"]["args"]["objects"]["args"]["objects"]["args"]
                            ["scope"]["Players"]["players"],
                        serde_json::json!({"kind":"One","args":{"kind":"AbilityController"}})
                    );
                    assert_eq!(
                        body[1]["args"]["operation"]["args"]["player"],
                        serde_json::json!({"kind":"Acting"})
                    );
                }
                "Drach'Nyen" => {
                    let power = &abilities[1]["args"]["changes"][1]["args"]["power"];
                    assert_eq!(power["kind"], "Aggregate");
                    assert_eq!(power["args"]["aggregation"], "Sum");
                    assert_eq!(
                        power["args"]["objects"]["args"]["filter"],
                        serde_json::json!({"kind":"ExiledWith","args":{"kind":"Source"}})
                    );
                }
                "Ensnared by the Mara" => {
                    // The opponent makes the villainous choice; YOU may cast.
                    // A player loop must not rebind every implicit action.
                    let optional = abilities.pointer("/0/args/body/instructions/0/args/body/instructions/0/args/operation/args/options/0/args/instructions/1").unwrap();
                    assert_eq!(optional["kind"], "Optional");
                    assert_eq!(
                        optional["args"]["decider"],
                        serde_json::json!({"kind":"AbilityController"})
                    );
                    assert_eq!(
                        optional
                            .pointer("/args/body/instructions/0/args/operation/args/actor")
                            .unwrap(),
                        &serde_json::json!({"kind":"One","args":{"kind":"AbilityController"}})
                    );
                }
                other => panic!("unexpected control {other}"),
            }
        }
    });
}
