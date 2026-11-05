#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, lower, mtg_types::OracleCard};
const FEATURE: &str = "multiple-blocks";
fn source(name: &str) -> OracleCard {
    ron::from_str(include_str!("fixtures/multiple_block_permissions.ronl").lines().find(|l| l.starts_with(&format!("Card(Name: \"{name}\""))).unwrap()).unwrap()
}
fn generate(name: &str, enabled: bool) -> Result<String, String> {
    let card = lower::card(&source(name)).unwrap();
    if enabled { cantrip::planned::with_additional_features(&[FEATURE], || cantrip::module(&card).map_err(|e| e.feature)) }
    else { cantrip::module(&card).map_err(|e| e.feature) }
}
#[test]
fn printed_abilities_and_rules_use_distinct_native_representations() {
    for (name, part) in [("Palace Guard", "block_allowance(u32::MAX"), ("Foriysian Brigade", "block_allowance(1"), ("Brave the Sands", "additional_blocks:"), ("Valor Made Real", "LastingRuleKind::AdditionalBlocks(u32::MAX"), ("Yare", "LastingRuleKind::AdditionalBlocks(2"), ("Kemba's Legion", "block_allowance_by("), ("Hundred-Handed One", "block_allowance_by(")] {
        assert!(generate(name, false).is_err(), "{name} must not claim old native support");
        let text = generate(name, true).unwrap_or_else(|e| panic!("{name}: {e}"));
        syn::parse_file(&text).unwrap();
        if let Ok(dir) = std::env::var("CANTRIP_MULTI_BLOCK_OUTPUT") {
            let filename = name.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "_");
            std::fs::write(std::path::Path::new(&dir).join(format!("{filename}.rs")), &text).unwrap();
        }
        assert!(text.contains(part), "{name}: {text}");
        if name == "Valor Made Real" || name == "Yare" {
            assert!(text.contains(".filter(|t| t_0.contains(t))"), "target legality must be checked: {text}");
        }
        if name == "Kemba's Legion" { assert!(text.contains("attached_to"), "{text}"); }
        if name == "Hundred-Handed One" { assert!(text.contains("marks.monstrous"), "{text}"); }
    }
}
#[test]
fn full_generator_describes_each_printed_permission() {
    for name in ["Foriysian Brigade", "Palace Guard", "Brave the Sands", "Valor Made Real", "Kemba's Legion", "Hundred-Handed One", "Yare"] {
        let source = source(name);
        let unified = mtgish_ast::unified::lower(&source).card;
        let output = cantrip::planned::with_additional_features(&[FEATURE], || cantrip::whole_module(&source, &unified)).unwrap_or_else(|e| panic!("{name}: {}", e.feature));
        syn::parse_file(&output).unwrap();
        if let Ok(dir) = std::env::var("CANTRIP_MULTI_BLOCK_WHOLE_OUTPUT") {
            let filename = name.to_lowercase().replace(|c: char| !c.is_ascii_alphanumeric(), "_");
            std::fs::write(std::path::Path::new(&dir).join(format!("{filename}.rs")), &output).unwrap();
        }
        if name == "Foriysian Brigade" { assert!(output.contains("block an additional creature"), "{output}"); }
        if name == "Palace Guard" { assert!(output.contains("block any number of creatures"), "{output}"); }
    }
}
