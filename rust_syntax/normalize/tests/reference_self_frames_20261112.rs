use mtgish_ast::{corpus, ir::*, mtg_types::OracleCard, unified, validate};
use serde_json::{json, Value};

const FIXTURE: &str = include_str!("fixtures/reference_self_frames_20261112.ronl");
fn source(name: &str) -> &'static str {
    FIXTURE
        .lines()
        .find(|line| line.contains(&format!("Name: {name:?},")))
        .unwrap()
}
fn lower(text: &str) -> Card {
    let source: OracleCard = ron::from_str(text).unwrap();
    let result = unified::lower(&source);
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    validate::unified(&result.card).unwrap();
    unified::round_trip(&result.card).unwrap();
    result.card.faces.into_iter().next().unwrap()
}
fn nodes<'a>(value: &'a Value, kind: &str, found: &mut Vec<&'a Value>) {
    if value["kind"] == kind {
        found.push(&value["args"]);
    }
    match value {
        Value::Object(fields) => fields.values().for_each(|v| nodes(v, kind, found)),
        Value::Array(items) => items.iter().for_each(|v| nodes(v, kind, found)),
        _ => {}
    }
}
fn history_excludes(value: &impl serde::Serialize, referent: &str) {
    let value = serde_json::to_value(value).unwrap();
    let mut history = vec![];
    nodes(&value, "EventHistory", &mut history);
    assert_eq!(history.len(), 1, "{value}");
    let mut excluded = vec![];
    nodes(history[0], "SameObject", &mut excluded);
    assert_eq!(excluded, vec![&json!({"kind":referent})]);
}

#[test]
fn self_attack_history_and_self_entry_spell_counts_exclude_source() {
    corpus::on_large_stack(|| {
        let warden = lower(source("Waterspout Warden"));
        let Ability::Triggered {
            intervening_if: Some(condition),
            ..
        } = &warden.abilities[0]
        else {
            panic!()
        };
        history_excludes(condition, "Source");
        history_excludes(&lower(source("Storm Entity")).abilities, "Source");
    });
}

#[test]
fn other_event_subjects_are_preserved_in_history_and_entry_replacements() {
    corpus::on_large_stack(|| {
        let attack = source("Waterspout Warden")
            .replace(
                "WhenACreatureAttacks(SinglePermanent(ThisPermanent))",
                "WhenACreatureAttacks(IsCardtype(Creature))",
            )
            .replace("Other(ThisPermanent)", "Other(Trigger_ThatCreature)");
        let card = lower(&attack);
        let Ability::Triggered {
            intervening_if: Some(condition),
            ..
        } = &card.abilities[0]
        else {
            panic!()
        };
        history_excludes(condition, "EventSubject");

        let entry = source("Storm Entity")
            .replace(
                "AsPermanentEnters(ThisPermanent,",
                "ReplaceWouldEnter(APermanentWouldEnterTheBattlefield(Other(ThisPermanent)),",
            )
            .replace(
                "NumSpellsCastThisTurn(Other(ThisSpell))",
                "NumSpellsCastThisTurn(Other(ThatEnteringPermanent))",
            );
        history_excludes(&lower(&entry).abilities, "EventSubject");
    });
}

#[path = "support/source_json.rs"]
mod source_json;
#[test]
fn current_source_formats_agree_on_every_reviewed_record() {
    corpus::on_large_stack(|| source_json::assert_fixture_matches(FIXTURE));
}
