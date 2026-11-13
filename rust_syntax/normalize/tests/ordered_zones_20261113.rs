use mtgish_ast::{corpus, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};
use std::collections::BTreeMap;
const FIXTURE: &str = include_str!("fixtures/ordered_zones_20261113.ronl");
fn cards() -> BTreeMap<String, Value> {
    FIXTURE
        .lines()
        .flat_map(|line| {
            let source: OracleCard = ron::from_str(line).unwrap();
            let result = unified::lower(&source);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            validate::unified(&result.card).unwrap();
            unified::round_trip(&result.card).unwrap();
            result.card.faces
        })
        .map(|card| {
            (
                card.name.clone(),
                serde_json::to_value(card.abilities).unwrap(),
            )
        })
        .collect()
}
fn nodes<'a>(v: &'a Value, kind: &str) -> Vec<&'a Value> {
    let mut out = vec![];
    if v["kind"] == kind {
        out.push(&v["args"]);
    }
    match v {
        Value::Object(m) => {
            for x in m.values() {
                out.extend(nodes(x, kind));
            }
        }
        Value::Array(a) => {
            for x in a {
                out.extend(nodes(x, kind));
            }
        }
        _ => {}
    }
    out
}
#[test]
fn complete_positional_action_packet_validates() {
    corpus::on_large_stack(|| {
        assert!(cards().len() >= 40);
    });
}
#[test]
fn top_matching_creature_and_test_actual_top_card_have_different_filter_positions() {
    corpus::on_large_stack(|| {
        let c = cards();
        let corpse = nodes(&c["Corpse Dance"], "Move")[0];
        assert_eq!(corpse["objects"]["kind"], "Take");
        assert_eq!(
            corpse["objects"]["args"]["objects"]["args"]["filter"],
            json!({"kind":"CardType","args":"Creature"})
        );
        let test = nodes(&c["Guiding Spirit"], "ObjectMatches")[0];
        assert_eq!(
            test["object"]["args"]["objects"]["args"]["filter"],
            json!({"kind":"Any"})
        );
        assert_eq!(test["filter"], json!({"kind":"CardType","args":"Creature"}));
        let furnace = nodes(&c["Phyrexian Furnace"], "Move")[0];
        assert_eq!(furnace["objects"]["kind"], "Take");
        assert_eq!(furnace["objects"]["args"]["from"], "Bottom");
    });
}
#[test]
fn looked_revealed_exiled_and_manifested_cards_keep_their_action_contracts() {
    corpus::on_large_stack(|| {
        let c = cards();
        let wand = &c["Wand of Denial"];
        assert_eq!(nodes(wand, "LookAt").len(), 1);
        assert!(nodes(wand, "Reveal").is_empty());
        assert_eq!(
            nodes(wand, "ObjectMatches")[0]["object"],
            json!({"kind":"Query","args":{"objects":{"kind":"Observed","args":0},"index":0}})
        );
        assert_eq!(
            nodes(wand, "Move")[0]["objects"],
            json!({"kind":"Observed","args":0})
        );
        let etali = &c["Etali, Primal Storm"];
        assert_eq!(
            nodes(etali, "Move")[0]["objects"],
            json!({"kind":"LibraryTops","args":{"kind":"All"}})
        );
        assert!(!nodes(etali, "Transition").is_empty());
        assert!(nodes(etali, "Observed").is_empty());
        let manifest = nodes(&c["Soul Summons"], "Manifest")[0];
        assert_eq!(manifest["cloak"], false);
        assert!(manifest["source"].get("TopOfLibrary").is_some());
        assert_eq!(nodes(&c["Cryptic Coat"], "Manifest")[0]["cloak"], true);
        let dread = &c["Manifest Dread"];
        assert!(dread.to_string().contains("ManifestDread"));
        assert!(nodes(dread, "Move").is_empty());
        let owner =
            &nodes(&c["Thieving Amalgam"], "Manifest")[0]["source"]["TopOfLibrary"]["player"];
        assert_eq!(owner, &json!({"kind":"EventSubject"}));
    });
}
#[test]
fn equivalent_live_selectors_use_the_same_single_library_form() {
    corpus::on_large_stack(|| {
        let c = cards();
        let agent = &c["Agent of Raffine"];
        let copy = agent[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]["card"]
            ["Duplicate"]
            .clone();
        assert_eq!(copy, nodes(agent, "Move")[0]["objects"]);
        for name in [
            "Conspicuous Snoop",
            "Skill Borrower",
            "Agent of Raffine",
            "The Fourth Doctor",
        ] {
            assert!(nodes(&c[name], "LibraryTops").is_empty(), "{name}");
        }
    });
}

#[test]
fn entire_collected_cached_family_has_one_positional_form_and_valid_ir() {
    corpus::on_large_stack(|| {
        use mtgish_ast::{
            ir::*,
            visit::{Visitor, Walk},
        };
        use std::collections::BTreeSet;
        struct Canonical;
        impl Visitor for Canonical {
            fn object_set(&mut self, objects: &ObjectSet) -> bool {
                assert!(!matches!(
                    objects,
                    ObjectSet::LibraryTops(PlayerSet::One(_))
                ));
                if let ObjectSet::One(ObjectRef::Query { objects, index: 0 }) = objects {
                    assert!(!matches!(
                        objects.as_ref(),
                        ObjectSet::Zone {
                            scope: ZoneScope::Players {
                                zone: EventZone::Library | EventZone::Graveyard,
                                players: PlayerSet::One(_)
                            },
                            kind: ObjectKind::Card,
                            ..
                        }
                    ));
                }
                true
            }
        }
        let names: BTreeSet<String> =
            serde_json::from_str(include_str!("fixtures/ordered_zone_names_20261113.json"))
                .unwrap();
        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../data/mtgish.lines.ron"
        ))
        .unwrap();
        let mut found = BTreeSet::new();
        for line in source.lines() {
            let raw: OracleCard = ron::from_str(line).unwrap();
            let identity = corpus::Identity::of(&raw).unwrap();
            if !identity.names.iter().any(|n| names.contains(n)) {
                continue;
            }
            let result = unified::lower(&raw);
            assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
            validate::unified(&result.card).unwrap();
            unified::round_trip(&result.card).unwrap();
            result.card.walk(&mut Canonical);
            for face in result.card.faces {
                if names.contains(&face.name) {
                    found.insert(face.name);
                }
            }
        }
        assert_eq!(found, names, "every cataloged cached face must be checked");
    });
}
