//! Generate typed traversal once with the independently pinned type package.
#[allow(dead_code)]
#[path = "src/declarations.rs"]
mod declarations;
#[path = "src/visit_spec.rs"]
mod visit_spec;
fn main() {
    println!("cargo:rerun-if-changed=src/declarations.rs");
    for source in declarations::SOURCES
        .iter()
        .filter(|source| source.role == declarations::Role::Semantic)
    {
        println!("cargo:rerun-if-changed={}", source.path);
    }
    println!("cargo:rerun-if-changed=src/visit_spec.rs");
    let source = declarations::traversal_text();
    let generated = format!(
        "{}\n{}",
        visit_spec::generate(&source),
        visit_spec::generate_mut(&source)
    );
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output"));
    std::fs::write(out.join("ir_visit.rs"), generated).expect("typed traversal");
}
