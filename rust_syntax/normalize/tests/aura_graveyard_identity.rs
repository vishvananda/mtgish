//! CR 400.7f names the Aura's own first graveyard incarnation, independently of its host.
use mtgish_ast::ir::*;
use serde_json::{json, Value};

fn nodes<'a>(value: &'a Value, kind: &str, found: &mut Vec<&'a Value>) {
    match value {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some(kind) {
                found.push(value);
            }
            for value in fields.values() {
                nodes(value, kind, found);
            }
        }
        Value::Array(values) => {
            for value in values {
                nodes(value, kind, found);
            }
        }
        _ => {}
    }
}

#[test]
fn every_aura_return_names_the_aura_after_its_original_host_left() {
    for source in include_str!("fixtures/aura_graveyard_identity.ronl").lines() {
        let returns = source.matches("ReturnEnchantingGraveyardCard").count()
            + source.matches("ReturnDeadGuestGraveyardCard").count();
        let source = ron::from_str(source).unwrap();
        let lowered = mtgish_ast::unified::lower(&source);
        let value = serde_json::to_value(&lowered.card).unwrap();
        let name = &lowered.card.faces[0].name;
        let mut moves = Vec::new();
        nodes(&value, "Move", &mut moves);
        let aura_returns = moves
            .iter()
            .filter(|m| {
                m["args"]["objects"]["kind"] == "One"
                    && m["args"]["objects"]["args"]["kind"] == "AuraAfterEnchantedLeft"
            })
            .count();
        assert_eq!(aura_returns, returns, "{name}: {:?}", lowered.blockers);
        let mut references = Vec::new();
        nodes(&value, "AuraAfterEnchantedLeft", &mut references);
        for reference in references {
            assert_eq!(
                reference["args"]["aura"],
                json!({"kind":"Source"}),
                "{name}"
            );
            assert_eq!(
                reference["args"]["host"],
                json!({"kind":"EventSubject"}),
                "{name}"
            );
        }
        mtgish_ast::unified::round_trip(&lowered.card).unwrap();
    }
}

#[test]
fn explicit_your_graveyard_is_a_separate_controller_restriction() {
    let mut restricted = Vec::new();
    for source in include_str!("fixtures/aura_graveyard_identity.ronl").lines() {
        let source = ron::from_str(source).unwrap();
        let lowered = mtgish_ast::unified::lower(&source);
        let name = &lowered.card.faces[0].name;
        let value = serde_json::to_value(&lowered.card).unwrap();
        let mut conditions = Vec::new();
        nodes(&value, "ObjectMatches", &mut conditions);
        let checks: Vec<_> = conditions
            .into_iter()
            .filter(|condition| condition["args"]["object"]["kind"] == "AuraAfterEnchantedLeft")
            .collect();
        let needs_your_graveyard = name.starts_with("Genju of the ")
            || matches!(
                name.as_str(),
                "Reins of the Vinesteed" | "Screams from Within"
            );
        assert_eq!(
            checks.len(),
            usize::from(needs_your_graveyard),
            "{name}: {:?}",
            lowered.blockers
        );
        for check in checks {
            assert_eq!(
                check["args"]["filter"],
                json!({"kind":"InZone","args":{"Players":{
                    "zone":"Graveyard","players":{"kind":"One","args":{"kind":"AbilityController"}}
                }}}),
                "{name}"
            );
            restricted.push(name.clone());
        }
    }
    assert_eq!(restricted.len(), 8);
}

#[test]
fn an_unrelated_zone_change_cannot_supply_the_aura_exception() {
    let source = include_str!("fixtures/aura_graveyard_identity.ronl")
        .lines()
        .find(|line| line.contains("Name: \"Puppet Master\""))
        .unwrap();
    let source = ron::from_str(source).unwrap();
    let mut card = mtgish_ast::lower::card(&source).unwrap();
    mtgish_ast::validate::card(&card).unwrap();
    let event = card
        .abilities
        .iter_mut()
        .find_map(|ability| match ability {
            Ability::Triggered { event, .. } => Some(event),
            _ => None,
        })
        .unwrap();
    *event = EventSpec::ZoneChange {
        objects: ObjectFilter::SameObject(ObjectRef::Source),
        from: Some(EventZone::Battlefield),
        to: EventZone::Graveyard,
    };
    let error = mtgish_ast::validate::card(&card).unwrap_err();
    assert!(
        error.contains("Aura graveyard reference has no enchanted-permanent departure"),
        "{error}"
    );
}

#[test]
fn a_replacement_proposal_is_not_an_actual_enchanted_permanent_departure() {
    let event = ReplacementEvent::ZoneChange {
        objects: ObjectFilter::SameObject(ObjectRef::AttachedTo(Box::new(ObjectRef::Source))),
        from: Some(EventZone::Battlefield),
        to: EventZone::Graveyard,
    };
    assert!(!mtgish_ast::semantics::replacement_bindings(&event).enchanted_left);
}

#[cfg(feature = "cantrip")]
#[test]
fn native_aura_query_is_required_before_the_card_can_be_generated() {
    let source = include_str!("fixtures/aura_graveyard_identity.ronl")
        .lines()
        .find(|line| line.contains("Name: \"Puppet Master\""))
        .unwrap();
    let source = ron::from_str(source).unwrap();
    let card = mtgish_ast::lower::card(&source).unwrap();
    let missing =
        mtgish_ast::cantrip::planned::with_features(&[], || mtgish_ast::cantrip::module(&card))
            .unwrap_err();
    assert!(
        mtgish_ast::cantrip::planned::requirement(&missing.feature)
            .starts_with("aura-graveyard-identity"),
        "{missing:?}"
    );
    let code = mtgish_ast::cantrip::planned::with_features(
        &["aura-graveyard-identity", "snow-mana"],
        || mtgish_ast::cantrip::module(&card),
    )
    .unwrap();
    assert!(
        code.contains("q.found_aura_after_enchanted_left("),
        "{code}"
    );
    assert!(!code.contains("q.found_after_move("), "{code}");
    std::fs::write(
        std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("aura-puppet-generated.rs"),
        code,
    )
    .unwrap();
}
