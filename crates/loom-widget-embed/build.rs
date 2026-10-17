//! Task #26's RQ6 §4.2 widget-embed fixture build.rs — compiles
//! demo/widget-embed.tsx through the real loomc pipeline TWICE, once per
//! `GraphMode`, exactly like crates/loom-island-compiled does for Task
//! #25's smaller acceptance fixture. This is what makes RQ6 §4.1's
//! "정체성 통제" principle literal for the *actual* RQ6 comparison fixture:
//! both arms derived from one JSX source, differing only in which
//! directive-respecting `GraphMode` compiles it.

use std::path::PathBuf;

fn main() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/widget-embed.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("loom-widget-embed/build.rs: failed to read {source_path:?}: {e}"));

    let islands = loom_compiler::build_support::expect_for_build_rs(loom_compiler::compile_tsx_to_rust_graph(
        &source_path.display().to_string(),
        &source,
        loom_compiler::GraphMode::Islands,
    ));
    let flat = loom_compiler::build_support::expect_for_build_rs(loom_compiler::compile_tsx_to_rust_graph(
        &source_path.display().to_string(),
        &source,
        loom_compiler::GraphMode::Flatten,
    ));

    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated.rs"), islands)
        .unwrap_or_else(|e| panic!("loom-widget-embed/build.rs: failed to write generated.rs: {e}"));
    std::fs::write(PathBuf::from(&out_dir).join("generated_flat.rs"), flat)
        .unwrap_or_else(|e| panic!("loom-widget-embed/build.rs: failed to write generated_flat.rs: {e}"));
}
