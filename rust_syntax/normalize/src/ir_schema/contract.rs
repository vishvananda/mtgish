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
fn schema_167_shape_is_frozen() {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/167.txt");
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
