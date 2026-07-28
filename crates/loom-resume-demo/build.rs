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
    // 2026-07-28 (RQ6 §7 판정 이후) — `compile_tsx_to_rust`(단일 컴포넌트 레거시
    // 경로, 엔트리 자동생성 없음) 대신 `GraphMode::Flatten`을 씀 — 이 소스는
    // ComponentRef가 없는 단일 컴포넌트라 `flatten_graph`가 그대로 통과시키므로
    // 산출되는 `setup`/`resume`/... 함수들은 바이트 단위로 동일하고, 추가로
    // `generate_flatten_entry_body`가 `ResumedApp`(옛 손코딩 `ResumedComponent`를
    // 대체)까지 자동 생성해준다는 차이만 있다.
    let generated = loom_compiler::compile_tsx_to_rust_graph(&source, loom_compiler::GraphMode::Flatten);

    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest = PathBuf::from(out_dir).join("generated.rs");
    std::fs::write(&dest, generated)
        .unwrap_or_else(|e| panic!("loom-resume-demo/build.rs: failed to write {dest:?}: {e}"));
}
