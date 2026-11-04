#![cfg(feature = "cantrip")]
use mtgish_ast::{cantrip, corpus, lower, mtg_types::OracleCard};

fn labels(name: &str) -> Vec<String> {
    let card = include_str!("fixtures/keyword_choice_words.ronl").lines().find_map(|line| {
        let source: OracleCard = ron::from_str(line).unwrap();
        corpus::Identity::of(&source).unwrap().names.iter().any(|n| n == name)
            .then(|| lower::card(&source).unwrap())
    }).unwrap();
    let module = cantrip::module(&card).unwrap();
    let file = syn::parse_file(&module).unwrap();
    let item = file.items.iter().find_map(|item| match item {
        syn::Item::Static(item) if item.ident == "CARD" => Some(item), _ => None,
    }).unwrap();
    let syn::Expr::Struct(definition) = &*item.expr else { panic!("card definition") };
    let field = definition.fields.iter().find(|field| matches!(&field.member, syn::Member::Named(name) if name == "labels")).unwrap();
    let syn::Expr::Reference(reference) = &field.expr else { panic!("labels reference") };
    let syn::Expr::Array(array) = &*reference.expr else { panic!("label array") };
    array.elems.iter().map(|label| match label {
        syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }) => text.value(),
        _ => panic!("printed label"),
    }).collect()
}

#[test]
fn urborg_offers_the_named_swampwalk_rather_than_the_landwalk_family() {
    assert_eq!(labels("Urborg"), ["first strike", "swampwalk"]);
}

#[test]
fn the_entry_form_keeps_its_printed_type_word_for_native_capture() {
    assert_eq!(labels("Primal Clay"), ["3/3 artifact creature", "2/2 artifact creature with flying",
        "1/6 Wall artifact creature with defender"]);
}
