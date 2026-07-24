//! Prints `render_html_flat()`'s output (GraphMode::Flatten, the H-Qwik
//! counterfactual) to stdout — the same source `print_ssr.rs` renders in
//! Island mode.
fn main() {
    println!("{}", loom_widget_embed::render_html_flat());
}
