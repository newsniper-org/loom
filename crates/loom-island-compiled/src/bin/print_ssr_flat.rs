//! Prints `render_html_flat()`'s output to stdout — the H-Qwik
//! counterfactual (`GraphMode::Flatten`) SSR of the SAME
//! `demo/islands/product-card.tsx` source `print_ssr.rs` renders in
//! Island mode. Useful for eyeballing the §4.1 "정체성 통제"-derived
//! diff between arms (same content, no `data-loom-island*` attributes,
//! LRef chunk prefix collapsed to one `App_chunk` namespace).
fn main() {
    println!("{}", loom_island_compiled::render_html_flat());
}
