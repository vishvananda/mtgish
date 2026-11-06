use mtgish_ast::{corpus, ir::*, lower, mtg_types::OracleCard, validate};
use serde_json::{json, Value};
fn card(name: &str) -> Card {
    let source: OracleCard = include_str!("fixtures/reference_investigations.ronl")
        .lines()
        .map(|l| ron::from_str(l).unwrap())
        .find(|s| {
            corpus::Identity::of(s)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    let c = lower::card(&source).unwrap();
    validate::card(&c).unwrap();
    c
}
fn find<'a>(v: &'a Value, kind: &str) -> Option<&'a Value> {
    match v {
        Value::Object(m) => {
            if v["kind"] == kind {
                return Some(&v["args"]);
            }
            m.values().find_map(|x| find(x, kind))
        }
        Value::Array(a) => a.iter().find_map(|x| find(x, kind)),
        _ => None,
    }
}
#[test]
fn creativity_draw_and_discard_share_a_pre_draw_number() {
    let c = serde_json::to_value(card("Laquatus's Creativity")).unwrap();
    let bound = find(&c, "BindValue").unwrap();
    assert!(bound["value"].to_string().contains("Hand"));
    let draw = find(&c, "Draw").unwrap();
    let discard = find(&c, "Discard").unwrap();
    assert_eq!(draw["amount"], discard["amount"]);
    assert_eq!(draw["amount"]["kind"], "BoundValue");
}
#[test]
fn ambergris_uses_one_optional_compound_payment_and_its_choice() {
    let c = serde_json::to_value(card("Ambergris, Agent of Law")).unwrap();
    let pay = find(&c, "PayCost").unwrap();
    assert_eq!(pay["optional"], true);
    let parts = pay["cost"]["parts"].as_array().unwrap();
    assert_eq!(parts.len(), 2);
    assert!(parts[0].get("PlayerAction").is_some());
    assert_eq!(
        parts[1],
        json!({"Draw":{"count":{"kind":"Constant","args":2}}})
    );
    assert!(find(&c, "PaymentChosen").is_some());
}
#[test]
fn villainous_choice_retains_its_marker_and_option_programs() {
    let c = serde_json::to_value(card("Missy")).unwrap();
    let choice = find(&c, "ChooseAction").unwrap();
    assert_eq!(choice["villainous"], true);
    assert_eq!(choice["options"].as_array().unwrap().len(), 2);
    assert!(find(&c, "ChooseOne").is_none());
}
#[cfg(feature = "cantrip")]
#[test]
fn generated_compound_payment_checks_draw_affordability_before_discard() {
    let rust = mtgish_ast::cantrip::module(&card("Ambergris, Agent of Law")).unwrap();
    let guard = rust.find("!g.cant_draw(").unwrap();
    assert!(guard < rust.find("g.discard_together(").unwrap());
    assert!(rust.contains("draw_count_"));
}
#[cfg(feature = "cantrip")]
#[test]
fn a_pile_choice_carries_both_piles_out_of_its_player_closure() {
    let rust = mtgish_ast::cantrip::module(&card("Abstract Performance")).unwrap();
    assert!(rust.contains("chosen_3.clone(), other_3.clone()"), "{rust}");
    assert!(rust.contains("carried_4_1"), "{rust}");
}
#[test]
fn reflexive_compound_costs_trigger_from_payment_including_replaced_draws() {
    for name in ["Ambergris, Agent of Balance", "Ambergris, Agent of Tyranny"] {
        let c = serde_json::to_value(card(name)).unwrap();
        let pay = find(&c, "PayCost").unwrap();
        assert_eq!(pay["cost"]["parts"].as_array().unwrap().len(), 2);
        assert_eq!(pay["optional"], true);
        assert!(find(&c, "Optional").is_none());
        let trigger = find(&c, "ReflexiveTrigger").unwrap();
        let result = trigger["performed"].clone();
        fn payment_receipt(v: &Value) -> Option<&Value> {
            if v["kind"] == "Effect" && v["args"]["operation"]["kind"] == "PayCost" {
                return Some(&v["args"]["result"]);
            }
            match v {
                Value::Object(m) => m.values().find_map(payment_receipt),
                Value::Array(a) => a.iter().find_map(payment_receipt),
                _ => None,
            }
        }
        assert_eq!(Some(&result), payment_receipt(&c), "{c}");
    }
}
