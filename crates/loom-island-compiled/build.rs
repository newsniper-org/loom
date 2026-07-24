//! Task #25 acceptance crate's build.rs — runs the real loomc pipeline
//! (SWC parse -> ComponentGraph -> GraphMode::Islands codegen) against
//! demo/islands/product-card.tsx, the same "no hand-authored stand-in"
//! convention every other demo crate's build.rs already follows
//! (loom-demo/loom-resume-demo/loom-fic).

use std::path::PathBuf;

fn main() {
    let source_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/islands/product-card.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("loom-island-compiled/build.rs: failed to read {source_path:?}: {e}"));
    let generated = loom_compiler::compile_tsx_to_rust_graph(&source, loom_compiler::GraphMode::Islands);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = PathBuf::from(out_dir).join("generated.rs");
    std::fs::write(&dest, generated)
        .unwrap_or_else(|e| panic!("loom-island-compiled/build.rs: failed to write {dest:?}: {e}"));
}
