#[path = "support/reference_followup.rs"]
mod fixtures;

use mtgish_ast::pipeline::selectors::{self, SelectorSource};
use serde_json::{json, Value};

fn predicate(source: SelectorSource) -> Value {
    serde_json::to_value(selectors::rewrite(source.as_ref()).unwrap()).unwrap()
}

#[test]
fn permanent_domain_includes_typeless_battlefield_objects() {
    let source = ron::from_str("Card(Name:\"Domain control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[SpellActions(Targeted([TargetPermanent(IsPermanent)],ActionList([TapPermanent(Ref_TargetPermanent)])))])").unwrap();
    let card = serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap();
    assert_eq!(
        card["abilities"][0]["args"]["announcement"]["targets"][0]["filter"],
        json!({"kind":"Any"})
    );
}

#[test]
fn permanent_cards_and_spells_keep_their_card_type_requirement() {
    let sources = [
        SelectorSource::Cards(ron::from_str("IsPermanent").unwrap()),
        SelectorSource::Spells(ron::from_str("IsPermanent").unwrap()),
        SelectorSource::CardsInGraveyards(ron::from_str("IsPermanent").unwrap()),
        SelectorSource::CardsInHand(ron::from_str("IsPermanent").unwrap()),
        SelectorSource::CardsInExile(ron::from_str("IsPermanent").unwrap()),
    ];
    for source in sources {
        let actual = predicate(source);
        let parts = actual["Either"].as_array().unwrap();
        assert_eq!(parts.len(), 6);
        for kind in [
            "Artifact",
            "Battle",
            "Creature",
            "Enchantment",
            "Land",
            "Planeswalker",
        ] {
            assert!(parts.contains(&json!({"Vocabulary":{"CardType":kind}})));
        }
    }
}

#[test]
fn explicit_six_type_battlefield_restriction_is_preserved() {
    let actual = predicate(SelectorSource::Permanents(ron::from_str(
        "Or([IsCardtype(Artifact),IsCardtype(Battle),IsCardtype(Creature),IsCardtype(Enchantment),IsCardtype(Land),IsCardtype(Planeswalker)])"
    ).unwrap()));
    assert_eq!(actual["Either"].as_array().unwrap().len(), 6);
}

#[test]
fn mirrorform_keeps_only_the_non_aura_target_restriction() {
    let card = serde_json::to_value(fixtures::card("Mirrorform")).unwrap();
    let target = &card["abilities"][0]["args"]["announcement"]["targets"][0];
    assert_eq!(
        target["kind"],
        json!({"Object":{"kind":"Permanent","zones":["Battlefield"]}})
    );
    assert_eq!(
        target["filter"],
        json!({"kind":"All","args":[
            {"kind":"Not","args":{"kind":"Subtype","args":"Aura"}},
            {"kind":"Any"}
        ]})
    );
}

#[test]
fn time_counter_modes_keep_the_counter_requirement_and_suspended_card_domain() {
    for (name, ability) in [("Shivan Sand-Mage", 0), ("Timebender", 1)] {
        let card = serde_json::to_value(fixtures::card(name)).unwrap();
        let modes = &card["abilities"][ability]["args"]["announcement"]["modes"]["modes"];
        let permanent =
            |mode| &modes[mode]["announcement"]["modes"]["modes"][0]["announcement"]["targets"][0];
        assert_eq!(permanent(0)["filter"], json!({"kind":"Any"}));
        assert_eq!(
            permanent(1)["filter"],
            json!({"kind":"All","args":[
                {"kind":"Any"},
                {"kind":"Counters","args":{"kind":{"Named":"TimeCounter"},
                    "comparison":"Greater","value":{"kind":"Constant","args":0}}}
            ]})
        );
        for mode in 0..2 {
            let suspended =
                &modes[mode]["announcement"]["modes"]["modes"][1]["announcement"]["targets"][0];
            assert_eq!(
                suspended["kind"],
                json!({"Object":{"kind":"Card","zones":["Exile"]}})
            );
            assert_eq!(
                suspended["filter"],
                json!({"kind":"State","args":"Suspended"})
            );
        }
    }
}

#[cfg(feature = "cantrip")]
#[test]
fn trigger_rule_context_still_requires_a_permanent_source() {
    // AbilityOf in a trigger rule has an implicit permanent-source domain,
    // including the look-back source for a leaves-the-battlefield trigger.
    // Removing the redundant card-type predicate must keep that domain check.
    let code = mtgish_ast::cantrip::module(&fixtures::card("Fractured Realm")).unwrap();
    assert!(code.contains("triggering.permanent"), "{code}");
}

#[test]
fn counter_replacement_retains_the_previous_guard_when_its_domain_is_erased() {
    let card = serde_json::to_value(fixtures::card("Solid Ground")).unwrap();
    let filter = &card["abilities"][1]["args"]["event"]["Counters"]["objects"]["args"][0];
    assert_eq!(
        filter,
        &json!({"kind":"Either","args":[
            {"kind":"CardType","args":"Artifact"},
            {"kind":"CardType","args":"Battle"},
            {"kind":"CardType","args":"Creature"},
            {"kind":"CardType","args":"Enchantment"},
            {"kind":"CardType","args":"Land"},
            {"kind":"CardType","args":"Planeswalker"}
        ]})
    );
}

#[test]
fn raw_counter_and_targeting_events_keep_their_previous_type_domain() {
    for (event, kind) in [
        (
            "WhenACounterOfTypeIsRemovedFromAPermanent(OilCounter,IsPermanent)",
            "Counters",
        ),
        (
            "WhenAPermanentBecomesTheTargetOfASpellOrAbility(IsPermanent,AnySpellOrAbility)",
            "BecomesTarget",
        ),
    ] {
        let text = format!("Card(Name:\"Domain control\",Typeline:(Supertypes:[],Cardtypes:[Instant],Subtypes:[]),Rules:[TriggerA({event},ActionList([GainLife(Integer(1))]))])");
        let source = ron::from_str(&text).unwrap();
        let card = serde_json::to_value(mtgish_ast::lower::card(&source).unwrap()).unwrap();
        let filter = &card["abilities"][0]["args"]["event"][kind]["objects"];
        assert_eq!(filter["kind"], json!("Either"));
        let types: Vec<_> = filter["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v["args"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            [
                "Artifact",
                "Battle",
                "Creature",
                "Enchantment",
                "Land",
                "Planeswalker"
            ]
        );
        assert!(!types.contains(&"Instant") && !types.contains(&"Sorcery"));
    }
}
