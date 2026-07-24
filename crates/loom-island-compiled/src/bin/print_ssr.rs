//! Prints `render_html()`'s output to stdout — used once to produce
//! `demo/islands/compiled/index.html`'s `#app` div content (pasted in by
//! hand, same convention `demo/resume/index.html` already established for
//! Phase 2's resumability demo). Re-run and re-paste whenever
//! `demo/islands/product-card.tsx` changes.
fn main() {
    println!("{}", loom_island_compiled::render_html());
}
