#![cfg(feature = "cantrip")]
use mtgish_ast::{ir::*, lower, semantics, validate};

#[test]
fn a_turn_named_by_an_objects_controller_binds_that_object_and_player() {
    let event = EventSpec::Turn {
        point: TurnPoint::EndStepBegins,
        players: PlayerSet::One(PlayerRef::ControllerOf(Box::new(ObjectRef::AttachedTo(
            Box::new(ObjectRef::Source),
        )))),
    };
    let bindings = semantics::event_bindings(&event);
    assert!(bindings.object && bindings.permanent && bindings.player);
    let ordinary = semantics::event_bindings(&EventSpec::Turn {
        point: TurnPoint::EndStepBegins,
        players: PlayerSet::All,
    });
    assert!(ordinary.player && !ordinary.object && !ordinary.permanent);
}

#[test]
fn end_step_instructions_use_original_objects_and_players() {
    for line in include_str!("fixtures/aggression_original_host.ronl").lines() {
        let source = ron::from_str(line).unwrap();
        let card = lower::card(&source).unwrap();
        validate::card(&card).unwrap();
        let trigger = card
            .abilities
            .iter()
            .find_map(|a| {
                if let Ability::Triggered { body, .. } = a {
                    Some(body)
                } else {
                    None
                }
            })
            .unwrap();
        let body = serde_json::to_string(trigger).unwrap();
        assert!(body.contains("EventSubject"), "{}: {body}", card.name);
        assert!(!body.contains("AttachedTo"), "{}: {body}", card.name);
        let module = mtgish_ast::cantrip::module(&card).unwrap();
        syn::parse_file(&module).unwrap();
        assert!(module.contains("item.subject.object()"), "{module}");
        if card.name != "Aggression" {
            assert!(
                module.contains("item.data[0]"),
                "the triggering player is captured too: {module}"
            );
        }
        if card.name == "Aggression" {
            let whole = serde_json::to_string(&card).unwrap();
            assert!(
                whole.contains("AttachedTo"),
                "the continuous ability still follows the Aura"
            );
        }
    }
}
