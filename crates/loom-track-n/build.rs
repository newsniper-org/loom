//! `demo/nested-list-derived.tsx` **하나**를 후보 다섯으로 각각 컴파일한다.
//! 같은 소스에서 파생시키는 것이 요점 — 픽스처가 후보마다 다르면 반례가
//! 후보의 결함인지 픽스처 차이인지 구별할 수 없다.

use loom_compiler::ListCandidate;
use std::path::PathBuf;

fn main() {
    let source_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/nested-list-derived.tsx");
    println!("cargo:rerun-if-changed={}", source_path.display());
    let source = std::fs::read_to_string(&source_path)
        .unwrap_or_else(|e| panic!("loom-track-n/build.rs: {source_path:?}: {e}"));

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    for (name, candidate) in [
        ("k", ListCandidate::K),
        ("c", ListCandidate::C),
        ("d", ListCandidate::D),
        ("d_lis", ListCandidate::DLis),
        ("e", ListCandidate::E),
        ("e_lis", ListCandidate::ELis),
    ] {
        let generated = loom_compiler::compile_tsx_to_rust_with_candidate(&source, candidate);
        std::fs::write(out_dir.join(format!("arm_{name}.rs")), generated)
            .unwrap_or_else(|e| panic!("loom-track-n/build.rs: write arm_{name}: {e}"));
    }
}
