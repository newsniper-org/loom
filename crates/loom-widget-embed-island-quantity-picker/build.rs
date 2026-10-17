//! RQ6 §6 M1 real per-Island code splitting (Task #27) — see
//! crates/loom-widget-embed-island-product-card/build.rs's doc comment for
//! the full rationale (identical here, just a different target Island).

use std::path::PathBuf;

fn main() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/widget-embed.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("build.rs: failed to read {source_path:?}: {e}"));

    let generated = loom_compiler::build_support::expect_for_build_rs(loom_compiler::compile_tsx_to_single_island_rust(
        &source_path.display().to_string(),
        &source,
        "QuantityPicker",
    ));

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated.rs"), generated)
        .unwrap_or_else(|e| panic!("build.rs: failed to write generated.rs: {e}"));
}
