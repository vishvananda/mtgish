//! A trigger-time capture retains an identity, not a live attachment query.
use mtgish_ast::{corpus, ir::*, migrate, mtg_types::OracleCard, semantics, unified, validate};
use serde_json::{json, Value};

fn vampire() -> UnifiedCard {
    let source: OracleCard =
        ron::from_str(include_str!("fixtures/vampiric_embrace_capture_20261111.ronl").trim())
            .unwrap();
    let lowered = unified::lower(&source);
    assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
    validate::unified(&lowered.card).unwrap();
    lowered.card
}

#[test]
fn capture_preserves_the_host_victim_and_aura_as_three_distinct_objects() {
    corpus::on_large_stack(|| {
        let card = vampire();
        let Ability::Triggered {
            event,
            body,
            announcement,
            ..
        } = &card.faces[0].abilities[2]
        else {
            panic!()
        };
        let EventSpec::WithOtherObject {
            event: inner,
            object,
        } = event
        else {
            panic!("{event:?}")
        };
        assert!(
            matches!(object, ObjectRef::AttachedTo(aura) if matches!(**aura, ObjectRef::Source))
        );
        assert!(matches!(
            inner.as_ref(),
            EventSpec::ZoneChange {
                from: Some(EventZone::Battlefield),
                to: EventZone::Graveyard,
                ..
            }
        ));
        let bindings = semantics::event_bindings(event);
        assert!(bindings.object && bindings.permanent && bindings.other);
        assert!(
            announcement.targets.is_empty(),
            "the captured host is not a target"
        );
        let value = serde_json::to_value(body).unwrap();
        assert_eq!(
            value["instructions"][0]["args"]["operation"]["args"]["placements"][0]["recipients"],
            json!({"kind":"Objects","args":{"kind":"One","args":{"kind":"EventOther"}}})
        );
        let text = serde_json::to_string(body).unwrap();
        assert!(
            !text.contains("AttachedTo"),
            "resolution must not re-query the Aura's current host"
        );
        assert!(
            !text.contains("EventSubject"),
            "the dying victim does not receive the counter"
        );
        assert_eq!(
            card.faces[0].abilities.len(),
            3,
            "the Aura retains its own trigger and controller"
        );
    });
}

#[test]
fn capture_expression_uses_inner_bindings_but_cannot_read_or_overwrite_itself() {
    corpus::on_large_stack(|| {
        let mut card = vampire();
        let Ability::Triggered {
            event: EventSpec::WithOtherObject { object, .. },
            ..
        } = &mut card.faces[0].abilities[2]
        else {
            panic!()
        };
        *object = ObjectRef::EventSubject;
        validate::unified(&card).unwrap();
        let Ability::Triggered {
            event: EventSpec::WithOtherObject { object, .. },
            ..
        } = &mut card.faces[0].abilities[2]
        else {
            panic!()
        };
        *object = ObjectRef::EventOther;
        assert!(validate::unified(&card)
            .unwrap_err()
            .contains("second object"));
        let mut card = vampire();
        let Ability::Triggered {
            event: EventSpec::WithOtherObject { event, .. },
            ..
        } = &mut card.faces[0].abilities[2]
        else {
            panic!()
        };
        *event = Box::new(EventSpec::Pair {
            event: PairEvent::Crews,
            objects: ObjectFilter::Any,
            others: ObjectFilter::Any,
        });
        assert!(validate::unified(&card).unwrap_err().contains("overwrite"));
    });
}

#[test]
fn historical_189_programs_keep_their_meaning_and_cannot_claim_new_capture_syntax() {
    corpus::on_large_stack(|| {
        let old = include_str!("fixtures/vampiric_embrace_schema189.json");
        let mut expected: Value = serde_json::from_str(old).unwrap();
        expected["schema_version"] = json!(SCHEMA_VERSION);
        expected["faces"][0]["schema_version"] = json!(SCHEMA_VERSION);
        let migrated = migrate::mapped_read_json(old).unwrap();
        assert_eq!(migrated.source_version, 189);
        assert!(migrated.map.introduced.is_empty());
        assert!(migrated
            .map
            .sites
            .iter()
            .all(|(from, to)| to == &[from.clone()]));
        assert_eq!(serde_json::to_value(migrated.card).unwrap(), expected);
        let mut wrong = serde_json::to_value(vampire()).unwrap();
        wrong["schema_version"] = json!(189);
        wrong["faces"][0]["schema_version"] = json!(189);
        assert!(migrate::read_json(&wrong.to_string())
            .unwrap_err()
            .contains("WithOtherObject"));
    });
}

#[test]
fn event_alternatives_only_export_the_capture_if_every_branch_supplies_it() {
    corpus::on_large_stack(|| {
        let mut card = vampire();
        let Ability::Triggered { event, .. } = &mut card.faces[0].abilities[2] else {
            panic!()
        };
        let EventSpec::WithOtherObject {
            event: uncaptured, ..
        } = event.clone()
        else {
            panic!()
        };
        *event = EventSpec::AnyOf(vec![event.clone(), *uncaptured]);
        assert!(validate::unified(&card)
            .unwrap_err()
            .contains("second object"));
    });
}

#[cfg(feature = "cantrip")]
#[test]
fn unsupported_runtime_capture_is_explicit_instead_of_using_a_live_host() {
    corpus::on_large_stack(|| {
        let card = vampire();
        let err = mtgish_ast::cantrip::module(&card.faces[0]).unwrap_err();
        assert!(
            format!("{err:?}").contains("trigger-time secondary object capture"),
            "{err:?}"
        );
    });
}

#[test]
fn explicit_secondary_capture_does_not_turn_into_a_damage_recipient_alias() {
    corpus::on_large_stack(|| {
        let source: OracleCard = ron::from_str(r#"Card(Name: "Capture role control", Typeline: (Supertypes: [], Cardtypes: [Enchantment], Subtypes: [Aura]), Rules: [TriggerA(WithOtherPermanent(WhenAPermanentDealsDamageToAPermanent(SinglePermanent(HostPermanent), IsCardtype(Creature)), ThisPermanent), ActionList([TapPermanent(Trigger_ThatOtherPermanent)]))])"#).unwrap();
        let lowered = unified::lower(&source);
        assert!(lowered.diagnostics.is_empty(), "{:?}", lowered.diagnostics);
        validate::unified(&lowered.card).unwrap();
        let value = serde_json::to_value(lowered.card).unwrap();
        let trigger = &value["faces"][0]["abilities"][0]["args"];
        assert_eq!(
            trigger["event"]["WithOtherObject"]["object"],
            json!({"kind":"Source"})
        );
        let body = trigger["body"].to_string();
        assert!(body.contains("EventOther"), "{body}");
        assert!(
            !body.contains("Damage"),
            "the retained secondary object is distinct from the victim: {body}"
        );
    });
}
