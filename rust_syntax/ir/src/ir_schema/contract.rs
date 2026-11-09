//! Freeze the typed shape that grammar declarations target, including wire order.
use super::Schema;

fn signature() -> String {
    let schema = Schema::of_ir().unwrap();
    let mut names: Vec<_> = schema.types.keys().collect();
    names.sort();
    let mut out = format!("schema_version={}\n", crate::ir::SCHEMA_VERSION);
    for name in names {
        out.push_str(&format!("{name}: {:?}\n", schema.types[name]));
    }
    out
}

#[test]
fn schema_shape_is_frozen() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("schema/{}.txt", crate::ir::SCHEMA_VERSION));
    assert_eq!(
        signature(),
        std::fs::read_to_string(fixture).unwrap(),
        "IR shape changed: bump the schema, review grammar patterns, and freeze a new contract"
    );
}

#[test]
#[ignore = "explicit contract generation; never updates the checked fixture"]
fn dump_schema_contract() {
    println!("BEGIN_SCHEMA_CONTRACT\n{}END_SCHEMA_CONTRACT", signature());
}

/// Migration origins must be interpreted using the schema at the saved epoch,
/// not a reconstruction from whichever types happen to compile today.
#[test]
#[ignore = "explicit frozen descriptor generation"]
fn dump_schema_descriptors() {
    println!("BEGIN_SCHEMA_DESCRIPTORS\n{}\nEND_SCHEMA_DESCRIPTORS", serde_json::to_string_pretty(&serde_json::json!({"schema_version":crate::ir::SCHEMA_VERSION,"schema":Schema::of_ir().unwrap()})).unwrap());
}

#[test]
fn frozen_descriptors_match_the_compiled_contract() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(format!("schema/{}.json", crate::ir::SCHEMA_VERSION));
    let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(fixture).unwrap()).unwrap();
    assert_eq!(value, serde_json::json!({"schema_version":crate::ir::SCHEMA_VERSION,"schema":Schema::of_ir().unwrap()}));
    let saved: Schema = serde_json::from_value(value["schema"].clone()).unwrap();
    assert_eq!(serde_json::to_value(saved).unwrap(), value["schema"]);
}
