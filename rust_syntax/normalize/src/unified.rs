//! Total lowering of every record into one schema. Strict lowering alone decides
//! whether a face is reduced; this only adds a representation for the rest, so it
//! cannot change what counts as complete.
use crate::{
    ir::{Ability, Card, Field, Operand, UnifiedCard, SCHEMA_VERSION},
    lower::{self, Diagnostic},
    mtg_types::OracleCard,
    pipeline::source_tree::decode,
};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// `Card` fields. A wrapper field outside this list is a nested face or layout data.
const FACE_FIELDS: [&str; 8] = [
    "Name",
    "Typeline",
    "ManaCost",
    "ColorIndicator",
    "Rules",
    "CardPT",
    "Loyalty",
    "Defense",
];
/// Shared by the faces beneath a wrapper that has no name of its own, such as
/// a Room's typeline and a Flip card's mana cost.
const INHERITED: [&str; 2] = ["Typeline", "ManaCost"];

pub struct Lowered {
    pub card: UnifiedCard,
    /// Why strict lowering rejected each unreduced face.
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Default)]
struct Walk {
    faces: Vec<Card>,
    data: Vec<Field>,
    diagnostics: Vec<Diagnostic>,
}

/// Named operands recurse through every lowerer, and those frames are large in
/// debug builds, so a deep card must not depend on the caller's stack.
const STACK_BYTES: usize = 256 << 20;

pub fn lower(value: &OracleCard) -> Lowered {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(STACK_BYTES)
            .spawn_scoped(scope, || lower_record(value))
            .expect("lowering thread")
            .join()
            .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

fn lower_record(value: &OracleCard) -> Lowered {
    let inspected = crate::inspect::value(value).expect("source AST serialization");
    let mut walk = Walk::default();
    walk.fields(
        inspected["args"].as_object().expect("OracleCard fields"),
        &Map::new(),
        "",
    );
    let card = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: inspected["variant"].as_str().expect("layout").into(),
        faces: walk.faces,
        data: walk.data,
    };
    Lowered {
        card,
        diagnostics: walk.diagnostics,
    }
}

impl Walk {
    fn fields(&mut self, fields: &Map<String, Value>, inherited: &Map<String, Value>, path: &str) {
        let mut scope = inherited.clone();
        for key in INHERITED {
            if let Some(value) = fields.get(key) {
                scope.insert(key.into(), value.clone());
            }
        }
        let is_face = fields.contains_key("Name") && scope.contains_key("Typeline");
        if is_face {
            let mut face: Map<String, Value> = scope.clone();
            for key in FACE_FIELDS {
                if let Some(value) = fields.get(key) {
                    face.insert(key.into(), value.clone());
                }
            }
            self.face(&json!({"enum": "OracleCard", "variant": "Card", "args": face}), path);
        }
        for (key, value) in fields {
            let consumed = if is_face {
                FACE_FIELDS.contains(&key.as_str())
            } else {
                INHERITED.contains(&key.as_str())
            };
            if !consumed {
                self.extra(key, value, &scope, path);
            }
        }
    }

    fn extra(&mut self, key: &str, value: &Value, scope: &Map<String, Value>, path: &str) {
        fn plain(v: &Value) -> Option<&Map<String, Value>> {
            v.as_object().filter(|fields| !fields.contains_key("enum"))
        }
        let path = format!("{path}{key}/");
        match value {
            Value::Object(_) if plain(value).is_some() => {
                self.fields(plain(value).unwrap(), scope, &path);
            }
            Value::Array(items) if !items.is_empty() && items.iter().all(|v| plain(v).is_some()) => {
                for (i, item) in items.iter().enumerate() {
                    self.fields(plain(item).unwrap(), scope, &format!("{path}{i}/"));
                }
            }
            _ => self.data.push(Field {
                name: key.into(),
                value: lower::operand(value),
            }),
        }
    }

    fn face(&mut self, node: &Value, path: &str) {
        let source: OracleCard = decode(node).expect("face fields are Card fields");
        match crate::pipeline::finalize(&source) {
            Ok(card) => self.faces.push(card),
            Err(errors) => {
                self.diagnostics.extend(errors.into_iter().map(|mut d| {
                    d.path = format!("{path}{}", d.path);
                    d
                }));
                self.faces.push(repaired(&source));
            }
        }
    }
}

/// Lenient lowering can bind something the validator rejects. Name that whole rule
/// instead. Each pass removes one reduced ability, and a named rule always validates.
fn repaired(source: &OracleCard) -> Card {
    let mut card = lower::unified_face(source);
    while let Some((index, _)) = crate::validate::first_invalid(&card) {
        card.abilities[index] = Ability::Named(lower::rule_at(source, index));
    }
    card
}

/// Every `Named` node, nested ones included, as `Type::Form` with its count.
pub fn named_forms(value: &impl Serialize) -> BTreeMap<String, usize> {
    fn walk(value: &Value, forms: &mut BTreeMap<String, usize>) {
        match value {
            Value::Object(fields) => {
                if let (Some(ty), Some(form), true) = (
                    fields.get("source_type").and_then(Value::as_str),
                    fields.get("form").and_then(Value::as_str),
                    fields.contains_key("operands"),
                ) {
                    *forms.entry(format!("{ty}::{form}")).or_default() += 1;
                }
                fields.values().for_each(|child| walk(child, forms));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, forms)),
            _ => {}
        }
    }
    let mut forms = BTreeMap::new();
    walk(&serde_json::to_value(value).expect("serializable AST"), &mut forms);
    forms
}

/// Fails loudly when the schema stops surviving serialization.
pub fn round_trip(card: &UnifiedCard) -> Result<(), String> {
    let expected = serde_json::to_value(card).map_err(|e| e.to_string())?;
    let json: UnifiedCard = serde_json::from_value(expected.clone()).map_err(|e| e.to_string())?;
    let config = bincode::config::standard();
    let bytes = bincode::encode_to_vec(card, config).map_err(|e| e.to_string())?;
    let (binary, _): (UnifiedCard, usize) =
        bincode::decode_from_slice(&bytes, config).map_err(|e| e.to_string())?;
    let text = ron::ser::to_string(card).map_err(|e| e.to_string())?;
    let ron: UnifiedCard = ron::from_str(&text).map_err(|e| e.to_string())?;
    for (format, copy) in [("JSON", json), ("bincode", binary), ("RON", ron)] {
        if serde_json::to_value(&copy).map_err(|e| e.to_string())? != expected {
            return Err(format!("{format} round trip changed the card"));
        }
    }
    Ok(())
}

/// Operands that need no review, so a record of them alone can be complete.
pub fn reduced_operand(value: &Operand) -> bool {
    named_forms(value).is_empty()
}
