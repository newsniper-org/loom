//! Runs the real loomc pipeline (SWC parse -> IR -> codegen) at build time,
//! against the same TSX demo source loom-demo's Phase 1 live path uses — the
//! whole point being that one compiled component serves both without
//! per-target codegen.

use std::path::PathBuf;

fn main() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../demo/counter-list.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("loom-resume-demo/build.rs: failed to read {source_path:?}: {e}"));
    let generated = loom_compiler::compile_tsx_to_rust(&source);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = PathBuf::from(out_dir).join("generated.rs");
    std::fs::write(&dest, generated)
        .unwrap_or_else(|e| panic!("loom-resume-demo/build.rs: failed to write {dest:?}: {e}"));
}
