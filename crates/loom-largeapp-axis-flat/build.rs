//! RQ6 §4.4 "원본 앱 규모" 축(Task #27) — H-Qwik(Flatten) 팔의 절반: 하나의
//! TargetWidget과 N=50개의 무관한 filler 컴포넌트(RQ6 §4.4 N-스케일링의 N=50
//! 관례 재사용, CLAUDE.md 2-1 — 새 숫자를 임의로 고르지 않음)를 전부 같은
//! `client:load` 그래프로 합성해 `GraphMode::Flatten`으로 컴파일한다. 이
//! 크레이트의 wasm 산출물 크기를 `crates/loom-largeapp-axis-island-target`
//! (같은 그래프에서 TargetWidget 하나만 `generate_single_island_client`로
//! 추출)와 비교하는 것이 이 축의 핵심 — "H-Qwik은 필요없는 49개 filler까지
//! 전부 번들에 끌고 들어가는가, Island은 대상 하나만 뽑아내는가".
//!
//! 소스를 파일이 아니라 이 build.rs가 직접 문자열로 합성하는 이유: 이 축이
//! 필요로 하는 건 "컴포넌트 개수가 많다"는 사실 자체지 특정 저작 내용이
//! 아니므로, 매번 같은 패턴을 N번 반복하는 합성 소스로 충분하다(실제
//! 프로덕션 트리셰이킹 동작을 재는 게 아니라 이 컴파일러의 `Flatten`/단일-
//! Island 추출 경로 자체가 컴포넌트 개수에 얼마나 민감한지를 재는 것).

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // 합성 소스는 2026-10-17(Phase 7a C1)부터 loom-compiler의 build_support에
    // 단일 정의로 있다(이전엔 두 largeapp build.rs에 문자열 그대로 복제). 출력
    // 문자열은 바이트 단위로 같다.
    let source = loom_compiler::build_support::largeapp_synth_source(50);
    let generated = loom_compiler::build_support::expect_for_build_rs(loom_compiler::compile_tsx_to_rust_graph(
        loom_compiler::build_support::LARGEAPP_SYNTH_FILE_NAME,
        &source,
        loom_compiler::GraphMode::Flatten,
    ));
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated_flat.rs"), generated)
        .unwrap_or_else(|e| panic!("build.rs: failed to write generated_flat.rs: {e}"));
}
