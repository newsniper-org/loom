//! Prints `render_html()`'s output to stdout — used once to produce
//! `demo/islands/compiled/index.html`'s `#app` div content (pasted in by
//! hand, same convention `demo/resume/index.html` already established for
//! Phase 2's resumability demo). Re-run and re-paste whenever
//! `demo/islands/product-card.tsx` changes.
// SSR 렌더러는 네이티브 전용(`#[cfg(not(wasm))]`)이므로 이 bin도 같이
// 게이트한다. 이게 없으면 `cargo build --workspace --target wasm32-...`가
// 여기서 깨져, wasm 빌드 확인이 조용히 `--lib` 범위로 좁아진다.
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn main() {
    println!("{}", loom_island_compiled::render_html());
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn main() {}
