//! Generate reviewed typed adapters and carry predefined-token source recipes.
//! No English parsing or backend generation happens here.
use std::{
    env, fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
};
mod mechanic_spec;
mod number_spec;
mod operand_spec;
mod program_spec;
mod rewrite_spec;
mod selector_spec;
/// Enums with only fieldless variants are vocabularies: a value is just its name.
fn write_vocabularies() {
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../src/mtg_types.rs");
    println!("cargo:rerun-if-changed={}", path.display());
    let names: Vec<String> = syn::parse_file(&fs::read_to_string(path).unwrap())
        .unwrap()
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Enum(e)
                if !e.variants.is_empty()
                    && e.variants.iter().all(|v| matches!(v.fields, syn::Fields::Unit)) =>
            {
                Some(e.ident.to_string())
            }
            _ => None,
        })
        .collect();
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("vocabularies.rs"),
        format!("const VOCABULARIES: &[&str] = &{names:?};"),
    )
    .unwrap();
}

fn main() {
    write_vocabularies();
    let mechanics_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("rewrites/mechanics.json5");
    println!("cargo:rerun-if-changed={}", mechanics_path.display());
    println!("cargo:rerun-if-changed=src/mechanic_spec.rs");
    let mechanics = mechanic_spec::Registry::parse(&fs::read_to_string(mechanics_path).unwrap())
        .expect("valid mechanic registry");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pipeline_mechanics.rs"),
        mechanics.generate(),
    )
    .unwrap();
    let operands_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("rewrites/operands.json5");
    println!("cargo:rerun-if-changed={}", operands_path.display());
    println!("cargo:rerun-if-changed=src/operand_spec.rs");
    let operands = operand_spec::Registry::parse(&fs::read_to_string(operands_path).unwrap())
        .expect("valid operand registry");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pipeline_operands.rs"),
        operands.generate(),
    )
    .unwrap();
    let programs_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("rewrites/programs.json5");
    println!("cargo:rerun-if-changed={}", programs_path.display());
    println!("cargo:rerun-if-changed=src/program_spec.rs");
    let programs = program_spec::Registry::parse(&fs::read_to_string(programs_path).unwrap())
        .expect("valid program registry");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pipeline_programs.rs"),
        programs.generate(),
    )
    .unwrap();
    let selectors_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("rewrites/selectors.json5");
    println!("cargo:rerun-if-changed={}", selectors_path.display());
    println!("cargo:rerun-if-changed=src/selector_spec.rs");
    let selectors = selector_spec::Registry::parse(&fs::read_to_string(selectors_path).unwrap())
        .expect("valid selector registry");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pipeline_selectors.rs"),
        selectors.generate(),
    )
    .unwrap();
    let numbers_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("rewrites/numbers.json5");
    println!("cargo:rerun-if-changed={}", numbers_path.display());
    println!("cargo:rerun-if-changed=src/number_spec.rs");
    let numbers = number_spec::Registry::parse(&fs::read_to_string(numbers_path).unwrap())
        .expect("valid numeric registry");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pipeline_numbers.rs"),
        numbers.generate(),
    )
    .unwrap();
    let registry_path =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("rewrites/actions.json5");
    println!("cargo:rerun-if-changed={}", registry_path.display());
    println!("cargo:rerun-if-changed=src/rewrite_spec.rs");
    let registry = rewrite_spec::Registry::parse(&fs::read_to_string(registry_path).unwrap())
        .expect("valid reviewed rewrite registry");
    let mut ids = std::collections::BTreeSet::new();
    for id in registry
        .rules
        .iter()
        .map(|r| &r.id)
        .chain(numbers.rules.iter().map(|r| &r.id))
        .chain(selectors.rules.iter().map(|r| &r.id))
        .chain(programs.rules.iter().map(|r| &r.id))
        .chain(operands.rules.iter().map(|r| &r.id))
        .chain(mechanics.rules.iter().map(|r| &r.id))
    {
        assert!(
            ids.insert(id),
            "rewrite IDs must be unique across registries: {id}"
        );
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("pipeline_rewrites.rs"),
        registry.generate(),
    )
    .unwrap();
    let corpus = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../data/mtgish.lines.ron");
    println!("cargo:rerun-if-changed={}", corpus.display());
    let mut out = fs::File::create(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("token_definitions.ronl"),
    )
    .unwrap();
    let mut count = 0;
    // The checked-in parser export is canonical, one complete record per line.
    // Runtime deserialization still checks the extracted records against Rust.
    for line in BufReader::new(fs::File::open(corpus).unwrap()).lines() {
        let line = line.unwrap();
        if line.starts_with("TokenDefinition(") {
            writeln!(out, "{line}").unwrap();
            count += 1;
        }
    }
    assert!(
        count > 0,
        "canonical source corpus has no predefined-token definitions"
    );
}
