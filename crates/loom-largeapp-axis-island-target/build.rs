//! RQ6 §4.4 "원본 앱 규모" 축(Task #27) — Island 팔의 절반: `crates/
//! loom-largeapp-axis-flat/build.rs`와 **동일한 합성 그래프**(TargetWidget +
//! filler 50개, 문자열 그대로 복제 — 두 크레이트가 정확히 같은 그래프를
//! 봐야 비교가 성립하므로)에서 `generate_single_island_client`로
//! TargetWidget **하나만** 추출한다. 이 크레이트의 wasm 산출물 크기가
//! filler 개수(N=50)와 무관하게 `demo/widget-embed`의 단독 QuantityPicker
//! 추출본과 비슷한 크기로 남아 있는지가 이 축의 핵심 질문.
//!
//! 2026-10-17(Phase 7a C1) 갱신: "문자열 그대로 복제"는 해소됐다 — 두 크레이트가
//! 이제 `loom_compiler::build_support::largeapp_synth_source`라는 단일 정의를
//! 호출한다(출력 문자열은 이관 전과 바이트 동일, 게이트 A로 확인).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // 합성 소스는 2026-10-17(Phase 7a C1)부터 loom-compiler의 build_support에
    // 단일 정의로 있다(이전엔 두 largeapp build.rs에 문자열 그대로 복제). 출력
    // 문자열은 바이트 단위로 같다.
    let source = loom_compiler::build_support::largeapp_synth_source(50);
    let generated = loom_compiler::build_support::expect_for_build_rs(loom_compiler::compile_tsx_to_single_island_rust(
        loom_compiler::build_support::LARGEAPP_SYNTH_FILE_NAME,
        &source,
        "TargetWidget",
    ));
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated.rs"), generated)
        .unwrap_or_else(|e| panic!("build.rs: failed to write generated.rs: {e}"));
}
