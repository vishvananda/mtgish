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

#[cfg(feature = "cantrip")]
fn future_code(card: &Card) -> String {
    mtgish_ast::cantrip::planned::with_additional_features(&["event-object-capture"], || {
        mtgish_ast::cantrip::module(card)
    }).unwrap()
}

#[cfg(feature = "cantrip")]
#[test]
fn future_capture_preserves_the_original_event_payload_and_never_requeries_the_host() {
    corpus::on_large_stack(|| {
        let card = vampire();
        let code = future_code(&card.faces[0]);
        syn::parse_file(&code).unwrap();
        assert!(code.contains("when_asked_with_other_object("), "{code}");
        assert!(code.contains("Option<(Fired, Option<ObjectId>)>"), "{code}");
        assert!(code.contains("Some((Fired { subject, data }, captured_other))"), "{code}");
        assert!(code.contains("q.attached_to(o)"), "use the matching view, including look-back: {code}");
        assert!(code.contains("item.event_other_object()"), "{code}");
        let resolve = code.split("fn ability_2(g:").nth(1).unwrap().split("pub static CARD:").next().unwrap();
        assert!(!resolve.contains("attached_to"), "{resolve}");
        assert!(!code.contains("planned_value!"), "{code}");
        assert!(mtgish_ast::cantrip::planned::with_additional_features(&["creation-exile-links"], || {
            mtgish_ast::cantrip::module(&card.faces[0])
        }).is_err());
        if let Ok(path) = std::env::var("MTGISH_CAPTURE_MODULE") {
            std::fs::write(path, &code).unwrap();
        }
    });
}

#[cfg(feature = "cantrip")]
#[test]
fn capture_is_available_to_match_conditions_and_target_filters() {
    corpus::on_large_stack(|| {
        let mut card = vampire().faces.remove(0);
        let Ability::Triggered { intervening_if, announcement, .. } = &mut card.abilities[2] else { panic!() };
        *intervening_if = Some(Condition::ObjectMatches { object: ObjectRef::EventOther, filter: ObjectFilter::Name("Host".into()) });
        announcement.targets.push(TargetSpec { slot: TargetSlot(0), kind: TargetKind::Object(ObjectDomain::permanent()), count: Count::Exactly(Expr::Constant(1)), filter: ObjectFilter::SameObject(ObjectRef::EventOther), players: None, distribute: None, constraints: vec![], chooser: None });
        validate::card(&card).unwrap();
        let code = future_code(&card);
        syn::parse_file(&code).unwrap();
        assert!(code.contains("captured_other.is_some_and("), "{code}");
        assert!(code.contains("q.event_other_object()"), "{code}");
        assert!(code.contains("item.event_other_object()"), "{code}");
    });
}

#[cfg(feature = "cantrip")]
#[test]
#[ignore = "type-check exact future API against MTGISH_CANTRIP_DIR; does not implement engine capture"]
fn future_capture_contract_typechecks() {
    use std::{fs, path::PathBuf, process::Command};
    let checkout = PathBuf::from(std::env::var_os("MTGISH_CANTRIP_DIR").unwrap());
    let root = PathBuf::from(std::env::var_os("MTGISH_CAPTURE_PROBE").unwrap());
    fs::create_dir_all(root.join("src")).unwrap();
    let mut manifest = "[package]\nname=\"mtgish-capture-contract\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n".to_string();
    for dep in ["cantrip-api", "cantrip-mechanics"] {
        manifest.push_str(&format!("{dep}={{path={:?}}}\n", checkout.join("crates").join(dep)));
    }
    fs::write(root.join("Cargo.toml"), manifest).unwrap();
    let contract = r#"
mod contract {
    use cantrip_api::*;
    pub trait PlannedStackCapture { fn event_other_object(&self) -> Option<ObjectId>; }
    impl PlannedStackCapture for StackItem {
        fn event_other_object(&self) -> Option<ObjectId> { unimplemented!("compile-only contract") }
    }
    pub trait PlannedQueryCapture { fn event_other_object(&self) -> Option<ObjectId>; }
    impl<T: Query + ?Sized> PlannedQueryCapture for T {
        fn event_other_object(&self) -> Option<ObjectId> { unimplemented!("compile-only contract") }
    }
    pub const fn when_asked_with_other_object(_mask: EventMask,
        _matches: fn(&dyn Query, &Asker, &Event) -> Option<(Fired, Option<ObjectId>)>) -> Trigger {
        fn disabled(_: &dyn Query, _: &Asker, _: &Event) -> Option<Fired> { None }
        when_asked(EventMask(0), disabled)
    }
}
"#;
    let mut lib = format!("#![allow(dead_code, unused_imports, unused_variables, unused_mut, unused_parens)]\n{contract}\n");
    corpus::on_large_stack(|| {
        let card = vampire().faces.remove(0);
        lib.push_str(&format!("mod vampiric {{ use crate::contract::*; {} }}", future_code(&card)));
        let mut guarded = card;
        let Ability::Triggered { intervening_if, announcement, .. } = &mut guarded.abilities[2] else { panic!() };
        *intervening_if = Some(Condition::ObjectMatches { object: ObjectRef::EventOther, filter: ObjectFilter::Name("Host".into()) });
        announcement.targets.push(TargetSpec { slot: TargetSlot(0), kind: TargetKind::Object(ObjectDomain::permanent()), count: Count::Exactly(Expr::Constant(1)), filter: ObjectFilter::SameObject(ObjectRef::EventOther), players: None, distribute: None, constraints: vec![], chooser: None });
        lib.push_str(&format!("mod guarded {{ use crate::contract::*; {} }}", future_code(&guarded)));
    });
    fs::write(root.join("src/lib.rs"), lib).unwrap();
    let output = Command::new("cargo").args(["check", "--manifest-path"]).arg(root.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", std::env::var_os("MTGISH_CANTRIP_TARGET").unwrap()).output().unwrap();
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
}
