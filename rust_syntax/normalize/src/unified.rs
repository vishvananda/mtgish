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
    /// The failure behind every named action and rule. Unlike `diagnostics`, this
    /// continues past the first failure, and unlike `named_forms` it leaves out
    /// children that only became named because their parent failed.
    pub blockers: Vec<Diagnostic>,
}

#[derive(Default)]
struct Walk {
    faces: Vec<Card>,
    data: Vec<Field>,
    diagnostics: Vec<Diagnostic>,
    blockers: Vec<Diagnostic>,
}

pub fn lower(value: &OracleCard) -> Lowered {
    let mut lowered = crate::corpus::on_large_stack(|| lower_record(value));
    crate::movement::normalize_unified(&mut lowered.card);
    lowered
}

/// Token definitions and sticker sheets have no card face, but hold ordinary
/// abilities. `None` leaves anything unreduced to the generic walk.
fn data_record(value: &OracleCard) -> Option<Lowered> {
    let (layout, faces, data) = match value {
        OracleCard::TokenDefinition { name, token } => (
            "TokenDefinition",
            lower::token_faces(name, token).ok()?,
            vec![],
        ),
        OracleCard::StickerSheet { stickers } => {
            let stickers = stickers
                .iter()
                .map(lower::sticker)
                .collect::<Result<Vec<_>, _>>()
                .ok()?;
            let data = vec![Field {
                name: "Stickers".into(),
                value: Operand::Stickers(stickers),
            }];
            ("StickerSheet", vec![], data)
        }
        _ => return None,
    };
    let card = UnifiedCard {
        schema_version: SCHEMA_VERSION,
        layout: layout.into(),
        faces,
        data,
    };
    crate::validate::unified(&card).ok()?;
    Some(Lowered {
        card,
        diagnostics: vec![],
        blockers: vec![],
    })
}

fn lower_record(value: &OracleCard) -> Lowered {
    if let Some(lowered) = data_record(value) {
        return lowered;
    }
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
        blockers: walk.blockers,
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
            self.face(
                &json!({"enum": "OracleCard", "variant": "Card", "args": face}),
                path,
            );
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
            Value::Array(items)
                if !items.is_empty() && items.iter().all(|v| plain(v).is_some()) =>
            {
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
                let at = |mut d: Diagnostic| {
                    d.path = format!("{path}{}", d.path);
                    d
                };
                self.diagnostics.extend(errors.into_iter().map(at));
                let (card, blockers) = repaired(&source);
                self.blockers.extend(blockers.into_iter().map(at));
                self.faces.push(card);
            }
        }
    }
}

/// Lenient lowering can bind something the validator rejects. Name that whole rule
/// instead. Each pass removes one reduced ability, and a named rule always validates.
fn repaired(source: &OracleCard) -> (Card, Vec<Diagnostic>) {
    let (mut card, mut blockers) = lower::unified_face(source);
    while let Some((index, message)) = crate::validate::first_invalid(&card) {
        card.abilities[index] = Ability::Named(lower::rule_at(source, index));
        blockers.push(Diagnostic {
            path: format!("Rules[{index}]"),
            source_type: "invariant".into(),
            form: "validator".into(),
            message,
        });
    }
    (card, blockers)
}

/// Unreduced and primitive forms in a value, nested ones included, with counts.
#[derive(Default)]
pub struct Forms {
    pub named: BTreeMap<String, usize>,
    pub primitives: BTreeMap<String, usize>,
}

pub fn forms(value: &impl Serialize) -> Forms {
    fn walk(value: &Value, forms: &mut Forms) {
        match value {
            Value::Object(fields) => {
                if let (Some(ty), Some(form), true) = (
                    fields.get("source_type").and_then(Value::as_str),
                    fields.get("form").and_then(Value::as_str),
                    fields.contains_key("operands"),
                ) {
                    *forms.named.entry(format!("{ty}::{form}")).or_default() += 1;
                }
                // Tagged enums nest it under `args`, and untagged ones under the variant.
                let primitive = if fields.get("kind").and_then(Value::as_str) == Some("Primitive") {
                    fields.get("args")
                } else {
                    fields.get("Primitive")
                };
                if let Some(name) = primitive.and_then(|p| p["name"].as_str()) {
                    *forms.primitives.entry(name.into()).or_default() += 1;
                }
                fields.values().for_each(|child| walk(child, forms));
            }
            Value::Array(items) => items.iter().for_each(|child| walk(child, forms)),
            _ => {}
        }
    }
    let mut forms = Forms::default();
    walk(
        &serde_json::to_value(value).expect("serializable AST"),
        &mut forms,
    );
    forms
}

pub fn named_forms(value: &impl Serialize) -> BTreeMap<String, usize> {
    forms(value).named
}

/// Fails loudly when the schema stops surviving serialization. Compares encoded
/// bytes, which are deterministic, because this runs on every record.
pub fn round_trip(card: &UnifiedCard) -> Result<(), String> {
    let text = |e: &dyn std::fmt::Display| e.to_string();
    let config = bincode::config::standard();
    let encode = |value: &UnifiedCard| bincode::encode_to_vec(value, config);
    let expected = encode(card).map_err(|e| text(&e))?;
    let (binary, _): (UnifiedCard, usize) =
        bincode::decode_from_slice(&expected, config).map_err(|e| text(&e))?;
    let json = serde_json::to_string(card).map_err(|e| text(&e))?;
    let json: UnifiedCard = serde_json::from_str(&json).map_err(|e| text(&e))?;
    let ron_text = ron::ser::to_string(card).map_err(|e| text(&e))?;
    let ron: UnifiedCard = ron::from_str(&ron_text).map_err(|e| text(&e))?;
    for (format, copy) in [("bincode", binary), ("JSON", json), ("RON", ron)] {
        if encode(&copy).map_err(|e| text(&e))? != expected {
            return Err(format!("{format} round trip changed the card"));
        }
    }
    Ok(())
}
