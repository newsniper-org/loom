//! Task #25/#26 acceptance crate's build.rs — runs the real loomc pipeline
//! (SWC parse -> ComponentGraph -> codegen) against
//! demo/islands/product-card.tsx, the same "no hand-authored stand-in"
//! convention every other demo crate's build.rs already follows
//! (loom-demo/loom-resume-demo/loom-fic). Compiles the SAME source TWICE,
//! once per `GraphMode` — this literally satisfies RQ6 §4.1's "정체성 통제"
//! principle (both arms derived from one JSX source, differing only in
//! which directive-respecting `GraphMode` compiles it), rather than two
//! independently hand-written fixtures.

use std::path::PathBuf;

fn main() {
    let source_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/islands/product-card.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("loom-island-compiled/build.rs: failed to read {source_path:?}: {e}"));

    let islands = loom_compiler::compile_tsx_to_rust_graph(&source, loom_compiler::GraphMode::Islands);
    let flat = loom_compiler::compile_tsx_to_rust_graph(&source, loom_compiler::GraphMode::Flatten);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated.rs"), islands)
        .unwrap_or_else(|e| panic!("loom-island-compiled/build.rs: failed to write generated.rs: {e}"));
    std::fs::write(PathBuf::from(&out_dir).join("generated_flat.rs"), flat)
        .unwrap_or_else(|e| panic!("loom-island-compiled/build.rs: failed to write generated_flat.rs: {e}"));
}
