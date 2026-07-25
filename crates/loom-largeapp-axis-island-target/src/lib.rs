//! RQ6 §4.4 "원본 앱 규모" 축 — Island 팔, 같은 합성 그래프(TargetWidget +
//! filler 50개)에서 TargetWidget 하나만 추출(build.rs 참고). 이 모듈은
//! `generate_single_island_client`가 이미 완결된 wasm-bindgen Island
//! 엔트리(struct/constructor 포함)까지 전부 생성하므로 손으로 얹을 코드가
//! 없다 — `crates/loom-widget-embed-island-*`와 동일한 구조.

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
