#![cfg(feature = "cantrip")]
use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};
use syn::{visit::Visit, Expr, Lit};

fn modules() -> &'static BTreeMap<String, String> {
    static MODULES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    MODULES.get_or_init(|| {
        let fixtures = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
        let output = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("delayed-labels.jsonl");
        mtgish_ast::cantrip::planned::tracked(&fixtures.join("delayed_labels.ronl"),
            &fixtures.join("delayed_labels.oracle.json"), &output).unwrap();
        std::fs::read_to_string(output).unwrap().lines().map(|line| {
            let row: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(row["faces"][0]["gaps"].as_array().unwrap().is_empty(), "{row}");
            (row["name"].as_str().unwrap().to_string(), row["file"].as_str().unwrap().to_string())
        }).collect()
    })
}

fn constructors(source: &str, constructor: &str) -> Vec<(String, String)> {
    struct Calls<'a> { name: &'a str, found: Vec<(String, String)> }
    impl<'ast> Visit<'ast> for Calls<'_> {
        fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
            if matches!(&*call.func, Expr::Path(path) if path.path.is_ident(self.name)) {
                let callback = match call.args.first().unwrap() {
                    Expr::Path(path) => path.path.get_ident().unwrap().to_string(),
                    _ => String::new(),
                };
                let text = call.args.iter().rev().find_map(|arg| match arg {
                    Expr::Lit(syn::ExprLit { lit: Lit::Str(text), .. }) => Some(text.value()),
                    _ => None,
                }).unwrap();
                self.found.push((callback, text));
            }
            syn::visit::visit_expr_call(self, call);
        }
    }
    let mut calls = Calls { name: constructor, found: vec![] };
    calls.visit_file(&syn::parse_file(source).unwrap());
    calls.found
}

fn docs(source: &str, name: &str) -> String {
    syn::parse_file(source).unwrap().items.iter().find_map(|item| match item {
        syn::Item::Fn(f) if f.sig.ident == name => Some({ let attrs = &f.attrs; quote::quote!(#(#attrs)*).to_string() }),
        _ => None,
    }).unwrap()
}

#[test]
fn slow_cantrips_name_the_draw_and_its_timing_in_their_own_oracle_paragraph() {
    for name in ["Blessed Wine", "Lightning Blow", "Panic", "Ray of Erasure"] {
        let source = &modules()[name];
        let calls = constructors(source, "delayed_trigger");
        assert_eq!(calls.len(), 1, "{name}: {source}");
        assert_eq!(calls[0].1, "Draw a card at the beginning of the next turn's upkeep.", "{name}");
        let documentation = docs(source, &calls[0].0);
        assert!(documentation.contains("Draw a card at the beginning"), "{name}: {documentation}");
        for wrong in ["You gain 1 life", "gains first strike", "can't block", "mills a card"] {
            assert!(!documentation.contains(wrong), "{name}: {documentation}");
        }
    }
}

#[test]
fn event_trigger_created_by_a_spell_quotes_its_later_paragraph() {
    let source = &modules()["Battle Cry"];
    let calls = constructors(source, "triggered");
    assert_eq!(calls.len(), 1, "{source}");
    assert_eq!(calls[0].1, "Whenever a creature blocks this turn, it gets +0/+1 until end of turn.");
}

#[test]
fn state_trigger_predicates_cite_state_trigger_rules() {
    for name in ["Dandân", "Island Fish Jasconius", "Merchant Ship"] {
        let source = &modules()[name];
        let calls = constructors(source, "state_triggered");
        assert!(!calls.is_empty(), "{name}: {source}");
        for (condition, _) in calls {
            let documentation = docs(source, &condition);
            assert!(documentation.contains("603.8"), "{name}: {documentation}");
            assert!(!documentation.contains("608.2"), "{name}: {documentation}");
        }
    }
}

#[test]
fn casting_restriction_before_a_keyword_keeps_its_own_printed_sentence() {
    let source = &modules()["Blizzard"];
    let file = syn::parse_file(source).unwrap();
    let documentation = file.items.iter().filter_map(|item| match item {
        syn::Item::Fn(f) => Some({ let attrs = &f.attrs; quote::quote!(#(#attrs)*).to_string() }),
        _ => None,
    }).find(|attrs| attrs.contains("601.3")).unwrap();
    assert!(documentation.contains("Cast this spell only if you control a snow land."), "{documentation}");
    assert!(!documentation.contains("flying don't untap"), "{documentation}");
}
