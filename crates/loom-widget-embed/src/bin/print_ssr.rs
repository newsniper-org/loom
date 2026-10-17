//! Prints `render_html()`'s output (GraphMode::Islands) to stdout — used
//! once to produce `demo/widget-embed/compiled/index.html`'s `#app` div
//! content, matching `demo/resume/index.html`'s established convention.
//!
//! **2026-10-17 정정**: 위 경로 `demo/widget-embed/compiled/index.html`은
//! 존재한 적이 없다(git 이력에도 없음 — `demo/islands/compiled/`의 관례를
//! 옮겨 적다 생긴 오기로 보인다). 이 bin의 출력이 들어간 실제 파일은
//! `demo/widget-embed/index.html`(Island 팔)이고, 그 파일의 머리 주석이
//! 이 bin을 출처로 적는다. Flatten 팔은 `print_ssr_flat` →
//! `demo/widget-embed/flat/index.html`.
// SSR 렌더러는 네이티브 전용(`#[cfg(not(wasm))]`)이므로 이 bin도 같이
// 게이트한다. 이게 없으면 `cargo build --workspace --target wasm32-...`가
// 여기서 깨져, wasm 빌드 확인이 조용히 `--lib` 범위로 좁아진다.
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn main() {
    println!("{}", loom_widget_embed::render_html());
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn main() {}
