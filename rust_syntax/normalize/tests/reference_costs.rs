#[path = "support/query_aliases.rs"]
mod query_aliases;
use mtgish_ast::{mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};

fn lowered(line: &str) -> Value {
    let source: OracleCard = ron::from_str(line).unwrap();
    let result = unified::lower(&source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    validate::unified(&result.card).unwrap();
    unified::round_trip(&result.card).unwrap();
    serde_json::to_value(result.card).unwrap()
}

// Only a conjunct alongside a subtype in a tap-cost selection may change.
fn remove_creature_conjunct(value: &mut Value) -> usize {
    let mut count = 0;
    if value["kind"] == "All" {
        let items = value["args"].as_array_mut().unwrap();
        if items.iter().any(|v| v["kind"] == "Subtype") {
            let before = items.len();
            items.retain(|v| *v != json!({"kind":"CardType","args":"Creature"}));
            count += before - items.len();
        }
    }
    match value {
        Value::Array(items) => {
            count += items
                .iter_mut()
                .map(remove_creature_conjunct)
                .sum::<usize>()
        }
        Value::Object(items) => {
            count += items
                .values_mut()
                .map(remove_creature_conjunct)
                .sum::<usize>()
        }
        _ => {}
    }
    count
}

fn repair_tap_costs(value: &mut Value) -> usize {
    let mut count = 0;
    if let Some(tap) = value.get_mut("ChangeTapState") {
        for selection in tap["selections"].as_array_mut().unwrap() {
            count += remove_creature_conjunct(&mut selection["objects"]);
        }
    }
    match value {
        Value::Array(items) => count += items.iter_mut().map(repair_tap_costs).sum::<usize>(),
        Value::Object(items) => count += items.values_mut().map(repair_tap_costs).sum::<usize>(),
        _ => {}
    }
    count
}

fn contains_value(value: &Value, needle: &Value) -> bool {
    value == needle
        || match value {
            Value::Array(items) => items.iter().any(|v| contains_value(v, needle)),
            Value::Object(items) => items.values().any(|v| contains_value(v, needle)),
            _ => false,
        }
}

#[test]
fn subtype_only_tap_costs_preserve_every_other_operand() {
    let mut cards_changed = 0;
    let mut selections_changed = 0;
    for (before, after) in include_str!("fixtures/reference_costs_before.ronl")
        .lines()
        .zip(include_str!("fixtures/reference_costs.ronl").lines())
    {
        if before == after || before.contains("Name: \"Highway Reaver\"") {
            continue;
        }
        cards_changed += 1;
        let mut expected = lowered(before);
        selections_changed += repair_tap_costs(&mut expected);
        assert_eq!(lowered(after), expected, "{after}");
    }
    assert_eq!(cards_changed, 35); // Reported 22 plus 13 found by the full-corpus audit.
    assert_eq!(selections_changed, 38); // Two double costs and two repeated Saga chapters.
}

#[test]
fn plural_and_explicit_creature_costs_are_unchanged() {
    let names = [
        "Birchlore Rangers",
        "Heritage Druid",
        "Catapult Squad",
        "Catapult Master",
        "Lullmage Mentor",
        "Mothdust Changeling",
        "Tradewind Rider",
    ];
    for name in names {
        let line = include_str!("fixtures/reference_costs.ronl")
            .lines()
            .find(|line| line.contains(&format!("Name: {name:?}")))
            .unwrap();
        let card = lowered(line);
        let text = serde_json::to_string(&card).unwrap();
        assert!(text.contains("ChangeTapState"));
        if ["Mothdust Changeling", "Tradewind Rider"].contains(&name) {
            assert!(contains_value(
                &card,
                &json!({"kind":"CardType","args":"Creature"})
            ));
        } else {
            // A subtype remains sufficient for the plural costs too.
            for ability in card["faces"][0]["abilities"].as_array().unwrap() {
                if let Some(parts) = ability.pointer("/args/cost/parts") {
                    assert!(!contains_value(
                        parts,
                        &json!({"kind":"CardType","args":"Creature"})
                    ));
                }
            }
        }
    }
}

#[test]
fn max_speed_wraps_only_highway_reavers_alternate_cost() {
    let find = |text: &'static str| {
        text.lines()
            .find(|line| line.contains("Name: \"Highway Reaver\""))
            .unwrap()
    };
    let before = lowered(find(include_str!("fixtures/reference_costs_before.ronl")));
    let mut expected = before.clone();
    let old = expected["faces"][0]["abilities"][3].take();
    expected["faces"][0]["abilities"][3] = json!({"kind":"Conditional","args":{
        "condition":{"kind":"PlayerState","args":{"player":{"kind":"AbilityController"},"state":"MaxSpeed"}},
        "then":[old],"otherwise":[]
    }});
    assert_eq!(
        lowered(find(include_str!("fixtures/reference_costs.ronl"))),
        expected
    );
    assert_eq!(
        before["faces"][0]["abilities"][3]["args"]["changes"][0]["args"]["KeywordCost"]
            ["first_each_turn"],
        true
    );
}

fn replace_announced_x(value: &mut Value, result: &Value) -> usize {
    if value == &json!({"kind":"AnnouncedX"}) {
        *value = json!({"kind":"BoundValue","args":result});
        return 1;
    }
    match value {
        Value::Array(items) => items
            .iter_mut()
            .map(|v| replace_announced_x(v, result))
            .sum(),
        Value::Object(items) => items
            .values_mut()
            .map(|v| replace_announced_x(v, result))
            .sum(),
        _ => 0,
    }
}

#[test]
fn all_three_reflexive_x_cards_only_change_the_amount_binding() {
    for before in include_str!("fixtures/reflexive_x_before.jsonl").lines() {
        let mut expected: Value = serde_json::from_str(before).unwrap();
        // This is the pre-fix producer snapshot. A later schema migration does
        // not change the exact amount-only semantic comparison.
        query_aliases::battlefield(&mut expected);
        query_aliases::value_choices(&mut expected);
        expected["schema_version"] = json!(mtgish_ast::ir::SCHEMA_VERSION);
        for face in expected["faces"].as_array_mut().unwrap() {
            face["schema_version"] = json!(mtgish_ast::ir::SCHEMA_VERSION);
        }
        let name = expected["faces"][0]["name"].as_str().unwrap().to_owned();
        let line = include_str!("fixtures/reference_costs.ronl")
            .lines()
            .find(|line| line.contains(&format!("Name: {name:?}")))
            .unwrap();
        query_aliases::movement_defaults(&mut expected);
        assert_eq!(replace_announced_x(&mut expected, &json!(0)), 1);
        assert_eq!(lowered(line), expected, "{name}");
    }
}

fn spell(actions: &str, cost: &str) -> Value {
    lowered(&format!(
        r#"Card(Name: "Test", Typeline: (Supertypes: [], Cardtypes: [Instant], Subtypes: []), ManaCost: {cost}, Rules: [SpellActions(ActionList([{actions}]))])"#
    ))
}

fn operations<'a>(value: &'a Value, kind: &str, out: &mut Vec<&'a Value>) {
    if value
        .pointer("/args/operation/kind")
        .is_some_and(|v| v == kind)
    {
        out.push(value);
    }
    match value {
        Value::Array(items) => {
            for v in items {
                operations(v, kind, out);
            }
        }
        Value::Object(items) => {
            for v in items.values() {
                operations(v, kind, out);
            }
        }
        _ => {}
    }
}

#[test]
fn reflexive_x_captures_the_actual_payment_result_after_an_earlier_action() {
    let value = spell("DrawACard, MayAction(Reflexive_PayManaAnyX_WhenYouDo([ManaCostX], ActionList([GainLife(ValueX)])))", "[]");
    let mut payments = Vec::new();
    operations(&value, "PayCost", &mut payments);
    let mut triggers = Vec::new();
    operations(&value, "ReflexiveTrigger", &mut triggers);
    let mut gains = Vec::new();
    operations(&value, "GainLife", &mut gains);
    assert_eq!((payments.len(), triggers.len(), gains.len()), (1, 1, 1));
    let result = &payments[0]["args"]["result"];
    assert_ne!(*result, json!(0));
    assert_eq!(
        triggers[0]["args"]["operation"]["args"]["performed"],
        *result
    );
    assert_eq!(
        gains[0]["args"]["operation"]["args"]["amount"],
        json!({"kind":"BoundValue","args":result})
    );
    // Declining the Optional skips both the payment and creation of the trigger.
    assert_eq!(
        value["faces"][0]["abilities"][0]["args"]["body"]["instructions"][1]["kind"],
        "Optional"
    );
}

#[test]
fn announced_x_and_fixed_mana_payments_keep_their_meaning() {
    let announced = spell("GainLife(ValueX)", "[ManaCostX]");
    assert!(announced.to_string().contains("AnnouncedX"));
    let fixed = spell("MayAction(Reflexive_PayMana_WhenYouDo([ManaCostGeneric(2)], ActionList([GainLife(ValueX)])))", "[ManaCostX]");
    assert!(fixed.to_string().contains("AnnouncedX"));
    assert!(!fixed.to_string().contains("BoundValue"));
    let fixed_x = lowered(
        r#"Card(Name: "Fixed X", Typeline: (Supertypes: [], Cardtypes: [Creature], Subtypes: []), Rules: [Activated(PayManaX([ManaCostX], TheNumberOfPermanentsOnTheBattlefield(IsCardtype(Land))), ActionList([DrawACard]))])"#,
    );
    let x = &fixed_x["faces"][0]["abilities"][0]["args"]["cost"]["parts"][0]["ManaWithX"]["x"];
    assert_eq!(x["kind"], "CountObjects");
    assert!(!fixed_x.to_string().contains("BoundValue"));
}
