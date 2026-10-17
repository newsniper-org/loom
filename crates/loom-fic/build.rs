//! Same pattern as loom-demo/loom-resume-demo's build.rs — compiles the
//! same `demo/counter-list.tsx` every other Phase 1-3 demo uses, so Phase
//! 4 proves backend-agnostic embedding of the *existing* compiled output,
//! not a specially-built component.

use std::path::PathBuf;

fn main() {
    let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/counter-list.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());

    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("loom-fic/build.rs: failed to read {source_path:?}: {e}"));
    let generated = loom_compiler::build_support::expect_for_build_rs(loom_compiler::compile_tsx_to_rust(
        &source_path.display().to_string(),
        &source,
    ));

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = PathBuf::from(out_dir).join("generated.rs");
    std::fs::write(&dest, generated).unwrap_or_else(|e| panic!("loom-fic/build.rs: failed to write {dest:?}: {e}"));
}
