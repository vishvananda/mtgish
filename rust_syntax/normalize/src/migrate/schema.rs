//! Immutable input descriptors, independent of whichever IR compiles today.
use crate::ir_schema::{Fields, Schema, Ty, TypeDef};
use std::sync::OnceLock;

fn load_180() -> Schema {
    let value: serde_json::Value =
        serde_json::from_str(include_str!("../../../ir/schema/180.json"))
            .expect("frozen schema-180 descriptors");
    assert_eq!(value["schema_version"], 180);
    serde_json::from_value(value["schema"].clone()).expect("typed schema-180 descriptors")
}
pub(super) fn input(version: u32) -> Result<&'static Schema, String> {
    static OLD: OnceLock<Schema> = OnceLock::new();
    static ENTRY: OnceLock<Schema> = OnceLock::new();
    static CURRENT: OnceLock<Schema> = OnceLock::new();
    match version {
        180 => Ok(OLD.get_or_init(load_180)),
        179 => Ok(ENTRY.get_or_init(|| {
            // This is the exact reviewed 179->180 delta applied to an immutable
            // snapshot, not a reconstruction from the current compiled model.
            let mut schema = load_180();
            let TypeDef::Enum { variants, .. } = schema.types.get_mut("Operation").unwrap() else {
                unreachable!()
            };
            for (name, fields) in variants {
                if super::ENTRY_OPERATIONS.contains(&name.as_str()) {
                    let Fields::Named(fields) = fields else {
                        unreachable!()
                    };
                    fields
                        .iter_mut()
                        .find(|(name, _)| name == "entry")
                        .unwrap()
                        .1 = Ty::Vec(Box::new(Ty::Named("Operand".into())));
                }
            }
            let TypeDef::Struct(Fields::Named(fields)) = schema.types.get_mut("EntrySpec").unwrap()
            else {
                unreachable!()
            };
            fields.retain(|(name, _)| name != "controller");
            schema.types.remove("EntryController");
            schema
        })),
        version if version == crate::ir::SCHEMA_VERSION => {
            Ok(CURRENT.get_or_init(|| Schema::of_ir().expect("current IR descriptors")))
        }
        _ => Err("unsupported schema epoch for typed migration origins".into()),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn historical_entry_descriptors_match_the_frozen_179_contract() {
        let schema = super::input(179).unwrap();
        let mut names: Vec<_> = schema.types.keys().collect();
        names.sort();
        let mut signature = String::from("schema_version=179\n");
        for name in names {
            signature.push_str(&format!("{name}: {:?}\n", schema.types[name]));
        }
        assert_eq!(signature, include_str!("../../../ir/schema/179.txt"));
    }
}
