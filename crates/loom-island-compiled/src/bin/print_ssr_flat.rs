//! Prints `render_html_flat()`'s output to stdout — the H-Qwik
//! counterfactual (`GraphMode::Flatten`) SSR of the SAME
//! `demo/islands/product-card.tsx` source `print_ssr.rs` renders in
//! Island mode. Useful for eyeballing the §4.1 "정체성 통제"-derived
//! diff between arms (same content, no `data-loom-island*` attributes,
//! LRef chunk prefix collapsed to one `App_chunk` namespace).
// SSR 렌더러는 네이티브 전용(`#[cfg(not(wasm))]`)이므로 이 bin도 같이
// 게이트한다. 이게 없으면 `cargo build --workspace --target wasm32-...`가
// 여기서 깨져, wasm 빌드 확인이 조용히 `--lib` 범위로 좁아진다.
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn main() {
    println!("{}", loom_island_compiled::render_html_flat());
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn main() {}
