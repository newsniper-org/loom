//! RQ6 §6 M1 real per-Island code splitting (Task #27) — compiles
//! demo/widget-embed.tsx's ProductCard Island ALONE via
//! `loom_compiler::compile_tsx_to_single_island_rust`, unlike
//! `crates/loom-widget-embed`'s build.rs (which emits the whole
//! `ComponentGraph` into one artifact — an empirically-found gap, see
//! `loom-compiler`'s `codegen.rs::generate_single_island_client` doc
//! comment). Same source as `crates/loom-widget-embed` — RQ6 §4.1's
//! "정체성 통제" principle still holds; only the *build target* (whole
//! graph vs one Island) differs.

use std::path::PathBuf;

fn main() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/widget-embed.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("build.rs: failed to read {source_path:?}: {e}"));

    let generated = loom_compiler::compile_tsx_to_single_island_rust(&source, "ProductCard");

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated.rs"), generated)
        .unwrap_or_else(|e| panic!("build.rs: failed to write generated.rs: {e}"));
}
