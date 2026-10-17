//! RQ6 §4.4 N-스케일링(N∈{1,10,50}) 픽스처 생성기 — Rust 판.
//!
//! **경위(2026-10-17)**: Task #27(2026-07-25, 091c733)은 계획의 Rust 크레이트 대신 Node 스크립트
//! `bench/e2e/fixtures/gen-nscaling-page.mjs`로 픽스처를 만들었고, 그 대체는 "병익님께 아직 명시적으로
//! 확인받지 않은 판단"으로 등재돼 있었다(RQ6-verification-plan.md). 병익님 결정: Node와 Rust 생성기가
//! 바이트 단위로 같은 결과를 낸다면 **둘 다 공존**시키고, 어느 쪽을 쓸지는 **설정으로** 고르며,
//! **마커 어긋남 가드**를 반드시 포함한다. 이 크레이트가 그 Rust 쪽이고, 가드는 `tests/`에 있다:
//! - `committed_fixtures_are_reproduced_byte_for_byte` — 커밋된 n{N}.html과 바이트 동일
//! - `node_and_rust_generators_are_byte_identical` — Node 생성기 출력과 바이트 동일
//! - `markers_match_compiler_output` — 블록의 Island 마커가 컴파일러(`loom-widget-embed`의
//!   `GraphMode::Islands` SSR)가 실제로 내는 QuantityPicker 마커와 일치
//!
//! 템플릿은 Node 판과 **한 글자도 다르면 안 된다** — 고칠 때는 두 생성기를 같이 고치고 위 테스트로 확인할 것.

/// 사전 등록된 N 값(RQ6 §4.4).
pub const NS: [usize; 3] = [1, 10, 50];

/// 인스턴스 하나의 마커 블록. 인스턴스마다 다른 것은 `id`/`data-loom-island-id`의 번호뿐이고,
/// 나머지 마커(`data-loom-chunk`·`data-loom-ctor`·`data-loom-activation`·`data-loom-lref`·`data-loom-id`)는
/// 컴파일러 산출물과 같아야 한다(가드 테스트).
pub fn quantity_picker_block(i: usize) -> String {
    format!(
        "<div id=\"qp-{i}\" data-loom-island data-loom-island-id=\"quantity-picker-{i}\" data-loom-activation=\"load\" data-loom-chunk=\"quantity_picker\" data-loom-ctor=\"QuantityPickerIsland\">
    <button data-loom-lref=\"QuantityPicker_chunk|Action0|cart_count:0\">+</button>
    <span data-loom-id=\"cart_count\">0</span>
  </div>"
    )
}

/// N개 인스턴스 페이지 전체.
pub fn page(n: usize) -> String {
    let blocks = (0..n).map(quantity_picker_block).collect::<Vec<_>>().join("\n  ");
    format!(
        "<!doctype html>
<html lang=\"ko\">
<head><meta charset=\"utf-8\" /><title>RQ6 §4.4 N-스케일링 (N={n})</title></head>
<body>
  <!--
    Task #27 — N개의 독립 QuantityPicker 인스턴스(client:load), 전부
    같은 data-loom-chunk=\"quantity_picker\" 청크를 가리킴. crates/
    loom-widget-embed의 __loom_debug_memory_bytes()로 wasm 선형 메모리
    총량을 관찰(M4), 무작위 하나를 panic_now 유사 방식으로 실패시켜
    나머지 생존율을 관찰(M6 결합 셀). 오늘의 registry.js/wasm-bindgen
    구성상 같은 chunk를 가리키는 인스턴스는 브라우저의 ES 모듈 캐시 덕에
    같은 WebAssembly.Instance(같은 선형 메모리)를 공유한다는 게 이 축의
    핵심 가설 — 실측으로 확인한다.
  -->
  <div id=\"app\">
  {blocks}
  </div>
  <script src=\"../../resume/shared-store.js\"></script>
  <script src=\"../../resume/registry.js\"></script>
</body>
</html>
"
    )
}
