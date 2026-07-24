//! Prints `render_html()`'s output (GraphMode::Islands) to stdout — used
//! once to produce `demo/widget-embed/compiled/index.html`'s `#app` div
//! content, matching `demo/resume/index.html`'s established convention.
fn main() {
    println!("{}", loom_widget_embed::render_html());
}
