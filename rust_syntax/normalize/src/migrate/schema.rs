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
    static COPY: OnceLock<Schema> = OnceLock::new();
    static PAYLOAD: OnceLock<Schema> = OnceLock::new();
    static KEYWORD: OnceLock<Schema> = OnceLock::new();
    static MODES: OnceLock<Schema> = OnceLock::new();
    static ROLES: OnceLock<Schema> = OnceLock::new();
    static MOVEMENT: OnceLock<Schema> = OnceLock::new();
    static FACTS: OnceLock<Schema> = OnceLock::new();
    static CURRENT: OnceLock<Schema> = OnceLock::new();
    match version {
        180 => Ok(OLD.get_or_init(load_180)),
        181 => Ok(COPY.get_or_init(|| {
            let value: serde_json::Value =
                serde_json::from_str(include_str!("../../../ir/schema/181.json"))
                    .expect("frozen schema-181 descriptors");
            assert_eq!(value["schema_version"], 181);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-181 descriptors")
        })),
        182 => Ok(PAYLOAD.get_or_init(|| {
            let value: serde_json::Value =
                serde_json::from_str(include_str!("../../../ir/schema/182.json"))
                    .expect("frozen schema-182 descriptors");
            assert_eq!(value["schema_version"], 182);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-182 descriptors")
        })),
        183 => Ok(KEYWORD.get_or_init(|| {
            let value: serde_json::Value =
                serde_json::from_str(include_str!("../../../ir/schema/183.json"))
                    .expect("frozen schema-183 descriptors");
            assert_eq!(value["schema_version"], 183);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-183 descriptors")
        })),
        184 => Ok(MODES.get_or_init(|| {
            let value: serde_json::Value =
                serde_json::from_str(include_str!("../../../ir/schema/184.json"))
                    .expect("frozen schema-184 descriptors");
            assert_eq!(value["schema_version"], 184);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-184 descriptors")
        })),
        185 => Ok(ROLES.get_or_init(|| {
            let value: serde_json::Value =
                serde_json::from_str(include_str!("../../../ir/schema/185.json"))
                    .expect("frozen schema-185 descriptors");
            assert_eq!(value["schema_version"], 185);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-185 descriptors")
        })),
        186 => Ok(MOVEMENT.get_or_init(|| {
            let value: serde_json::Value = serde_json::from_str(include_str!("../../../ir/schema/186.json")).expect("frozen schema-186 descriptors");
            assert_eq!(value["schema_version"], 186);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-186 descriptors")
        })),
        187 => Ok(FACTS.get_or_init(|| {
            let value: serde_json::Value = serde_json::from_str(include_str!("../../../ir/schema/187.json")).expect("frozen schema-187 descriptors");
            assert_eq!(value["schema_version"], 187);
            serde_json::from_value(value["schema"].clone()).expect("typed schema-187 descriptors")
        })),
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
