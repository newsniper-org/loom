//! Prints `render_html_flat()`'s output (GraphMode::Flatten, the H-Qwik
//! counterfactual) to stdout — the same source `print_ssr.rs` renders in
//! Island mode.
// SSR 렌더러는 네이티브 전용(`#[cfg(not(wasm))]`)이므로 이 bin도 같이
// 게이트한다. 이게 없으면 `cargo build --workspace --target wasm32-...`가
// 여기서 깨져, wasm 빌드 확인이 조용히 `--lib` 범위로 좁아진다.
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
fn main() {
    println!("{}", loom_widget_embed::render_html_flat());
}

#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
fn main() {}
