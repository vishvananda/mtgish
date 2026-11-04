//! Generate typed traversal once with the independently pinned type package.
#[path = "src/visit_spec.rs"]
mod visit_spec;
fn main() {
    println!("cargo:rerun-if-changed=src/ir.rs");
    println!("cargo:rerun-if-changed=src/visit_spec.rs");
    let source = std::fs::read_to_string("src/ir.rs").expect("IR source");
    let generated = format!(
        "{}\n{}",
        visit_spec::generate(&source),
        visit_spec::generate_mut(&source)
    );
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo output"));
    std::fs::write(out.join("ir_visit.rs"), generated).expect("typed traversal");
}
