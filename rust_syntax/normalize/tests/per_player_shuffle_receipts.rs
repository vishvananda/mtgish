//! Each player draws their own shuffled count; a distinct target does not redefine a global receipt.
use mtgish_ast::{corpus, mtg_types as source, unified, validate};

fn card(name: &str) -> serde_json::Value {
    let source: source::OracleCard = include_str!("fixtures/per_player_shuffle_receipts.ronl")
        .lines()
        .map(|line| ron::from_str(line).unwrap())
        .find(|card| {
            corpus::Identity::of(card)
                .unwrap()
                .names
                .iter()
                .any(|n| n == name)
        })
        .unwrap();
    let lowered = unified::lower(&source);
    assert!(
        lowered.diagnostics.is_empty(),
        "{name}: {:?}",
        lowered.diagnostics
    );
    validate::unified(&lowered.card).unwrap();
    serde_json::to_value(lowered.card).unwrap()
}
fn collect<'a>(value: &'a serde_json::Value, kind: &str, out: &mut Vec<&'a serde_json::Value>) {
    if value.get("kind").and_then(|v| v.as_str()) == Some(kind) {
        out.push(&value["args"]);
    }
    match value {
        serde_json::Value::Object(o) => {
            for v in o.values() {
                collect(v, kind, out);
            }
        }
        serde_json::Value::Array(a) => {
            for v in a {
                collect(v, kind, out);
            }
        }
        _ => (),
    }
}
#[test]
fn a_later_player_loop_reads_that_players_shuffle_receipt() {
    for name in ["Winds of Change", "Molten Psyche", "Whirlpool Warrior"] {
        let card = card(name);
        let mut draws = vec![];
        collect(&card, "Draw", &mut draws);
        let draw = draws.last().unwrap();
        let amount = &draw["amount"];
        assert_eq!(amount["kind"], "CountObjects", "{name}: {draw}");
        assert_eq!(amount["args"]["kind"], "MovedBy", "{name}: {draw}");
        assert_eq!(
            amount["args"]["args"]["phase"], "Before",
            "count actual shuffled original cards: {draw}"
        );
        assert_eq!(
            amount["args"]["args"]["player"], draw["players"]["args"],
            "bind each draw to the same player's shuffle: {draw}"
        );
    }
}
#[test]
fn ordinary_shuffle_then_draw_keeps_the_whole_single_player_receipt() {
    for name in ["Credit Voucher", "Whirlpool Rider"] {
        let card = card(name);
        let mut draws = vec![];
        collect(&card, "Draw", &mut draws);
        assert_eq!(
            draws[0]["amount"]["args"]["kind"], "Transition",
            "{name}: {}",
            draws[0]
        );
    }
}
#[test]
fn a_different_mill_recipient_does_not_reassign_the_casters_shuffle() {
    let card = card("Psychic Spiral");
    let text = serde_json::to_string(&card).unwrap();
    assert!(
        !text.contains("MovedBy"),
        "the target mills the caster's count, not its own: {text}"
    );
    assert!(text.contains("Transition"));
}
