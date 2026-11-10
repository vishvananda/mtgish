//! Grammar-facing contracts over the simplified model, without legacy lowering.
use mtgish_ast::{canonical, ir::Ability, validate};
use serde_json::{json, Value};

fn spell(operations: Vec<Value>) -> Value {
    json!([{"kind":"Spell","args":{"announcement":{"targets":[],"modes":null},
        "body":{"instructions":operations.into_iter().enumerate().map(|(id, operation)|
            json!({"kind":"Effect","args":{"result":id,"operation":operation}})
        ).collect::<Vec<_>>()}}}])
}

fn pool(kind: &str) -> Value {
    json!({"kind":"Zone","args":{"scope":{"Shared":"Battlefield"},"kind":kind,
        "filter":{"kind":"CardType","args":"Creature"}}})
}

fn select(id: u32, objects: Value, action: Value) -> Value {
    json!({"kind":"WithSelections","args":{"steps":[{"binding":id,
        "chooser":{"Players":{"kind":"One","args":{"kind":"AbilityController"}}},
        "objects":objects,"count":{"Exactly":{"kind":"Constant","args":1}},"constraints":[]}],
        "action":action}})
}

fn picked(id: u32) -> Value {
    json!({"kind":"Prepared","args":id})
}

fn reveal(objects: Value) -> Value {
    json!({"kind":"Reveal","args":{"objects":objects}})
}

fn normalized(mut value: Value) -> Value {
    let before: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&before).unwrap();
    canonical::abilities(&mut value).unwrap();
    let after: Vec<Ability> = serde_json::from_value(value.clone()).unwrap();
    validate::abilities(&after).unwrap();
    let mut again = value.clone();
    canonical::abilities(&mut again).unwrap();
    assert_eq!(value, again);
    value
}

#[test]
fn a_shadowed_selection_and_an_alpha_renamed_selection_have_one_meaning() {
    let body = |outer, inner| {
        spell(vec![select(
            outer,
            pool("Permanent"),
            select(inner, picked(outer), reveal(picked(inner))),
        )])
    };
    assert_eq!(normalized(body(17, 17)), normalized(body(41, 92)));
}

#[test]
fn siblings_have_independent_preparation_scopes() {
    let body = |first, second| {
        spell(vec![
            select(first, pool("Permanent"), reveal(picked(first))),
            select(second, pool("Permanent"), reveal(picked(second))),
        ])
    };
    assert_eq!(normalized(body(17, 17)), normalized(body(41, 92)));
}

#[test]
fn selection_ids_and_instruction_result_ids_have_separate_namespaces() {
    let body = |selection| {
        let mut body = spell(vec![
            select(
                selection,
                pool("Permanent"),
                json!({"kind":"Move","args":{"objects":picked(selection),
                "destination":{"OwnersHand":{"counters":[],"states":[]}},"qualifiers":[]}}),
            ),
            reveal(json!({"kind":"Transition","args":{"result":0,"phase":"After"}})),
        ]);
        body[0]["args"]["body"]["instructions"][0]["args"]["result"] = json!(0);
        body
    };
    assert_eq!(normalized(body(0)), normalized(body(87)));
}

#[test]
fn permanent_type_reduction_keeps_other_query_domains_distinct() {
    let types = [
        "Artifact",
        "Battle",
        "Creature",
        "Enchantment",
        "Land",
        "Planeswalker",
    ]
    .map(|name| json!({"kind":"CardType","args":name}));
    let filter = json!({"kind":"Either","args":types});
    for (kind, reduced) in [("Permanent", true), ("Card", false)] {
        let mut objects = pool(kind);
        objects["args"]["filter"] = filter.clone();
        let normalized = normalized(spell(vec![reveal(objects)]));
        let actual = &normalized[0]["args"]["body"]["instructions"][0]["args"]["operation"]["args"]
            ["objects"]["args"]["filter"];
        assert_eq!(actual["kind"] == "Any", reduced);
    }
}

#[test]
fn an_omitted_default_arrival_has_the_native_default_meaning() {
    let code = |destination| {
        format!("Card{{name:\"Default arrival\",properties:{{card_types:[Instant]}},abilities:[Spell{{do:[MoveObjects{{objects:One(This),destination:{destination}}}]}}]}}")
    };
    let bare = mtgish_ast::human::read(&code("OwnersHand")).unwrap();
    let explicit = mtgish_ast::human::read(&code("OwnersHand({counters:[],states:[]})")).unwrap();
    assert_eq!(
        normalized(json!(bare.faces[0].abilities)),
        normalized(json!(explicit.faces[0].abilities))
    );
}
