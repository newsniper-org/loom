//! RQ6 §4.4 "원본 앱 규모" 축 — H-Qwik(Flatten) 팔, N=50 filler + 1
//! TargetWidget (build.rs 참고). wasm 엔트리(`ResumedApp`)는 2026-07-28부터
//! `generated_flat` 모듈 안에 컴파일러가 직접 생성한다(`codegen.rs`의
//! `generate_flatten_entry_body`) — 이전엔 `ResumedLargeFlatApp`이라는 이름으로
//! 손코딩됐었으나, `crates/loom-widget-embed`/`crates/loom-resume-demo`의 거의
//! 동일한 손코딩 세 벌을 컴파일러 기능 하나로 흡수하며 이름도 `ResumedApp`으로
//! 통일됨.

pub mod generated_flat {
    include!(concat!(env!("OUT_DIR"), "/generated_flat.rs"));
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub use generated_flat::ResumedApp;
