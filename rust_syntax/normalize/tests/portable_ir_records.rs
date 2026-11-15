use mtgish_ast::{artifacts, corpus, inspect};
use serde_json::Value;
#[test]
fn export_keeps_explicit_primary_faces_and_source_provenance() {
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/layouts.ronl");
    for source in corpus::read_cards(&path).unwrap() {
        let record=artifacts::record(&source).unwrap();
        let raw=inspect::value(&source).unwrap();
        let args=&raw["args"];
        let primary=match record["card"]["layout"].as_str().unwrap() {
            "Transforming"|"ModalDFC"=>&args["FrontFace"]["Name"],
            "Flip"=>&args["Unflipped"]["Name"],
            "Adventurer"|"Ominous"|"Preparer"=>&args["Name"],
            "Room"=>&args["LeftDoor"]["Name"],
            "Split"=>&args["Cards"][0]["Name"],
            _=>&record["card"]["faces"][0]["name"],
        };
        assert_eq!(&record["context"]["primary_face"],primary);
        assert_eq!(record["identity"],serde_json::to_value(corpus::Identity::of(&source).unwrap()).unwrap());
        assert_eq!(record["format_version"],1);
        assert!(record["card"]["faces"].as_array().unwrap().iter().any(|face| &face["name"]==primary));
        let owned:mtgish_ast::ir::UnifiedCard=serde_json::from_value(record["card"].clone()).unwrap();
        mtgish_ast::validate::unified(&owned).unwrap();
        assert_ne!(record["identity"]["source_sha256"],Value::Null);
    }
}
