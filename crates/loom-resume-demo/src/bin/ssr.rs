//! Stands in for a real backend's SSR step (Phase 4's actual concern — FIC,
//! WASM-in-process, etc.) with a native binary that writes a static HTML
//! file. Run with: `cargo run --bin ssr -- <output-path>`.

fn main() {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "demo/resume/index.html".to_string());

    let body_html = loom_resume_demo::ssr_render();

    let page = format!(
        r#"<!doctype html>
<html lang="ko">
<head>
  <meta charset="utf-8" />
  <title>loom Phase 2 resumability demo — SSR + lazy chunk</title>
</head>
<body>
  <!--
    Everything inside #app was produced by ssr_render() (native Rust, no
    browser, no wasm) — this file has zero <script> tags that load any
    interactive framework code. Only bootstrap.js below runs on load, and
    it is deliberately tiny (see its own file for what it does and does
    not do before the first click).
  -->
  <div id="app">{body_html}</div>
  <script src="./bootstrap.js"></script>
</body>
</html>
"#
    );

    std::fs::write(&out_path, page).unwrap_or_else(|e| panic!("failed to write {out_path}: {e}"));
    eprintln!("wrote {out_path}");
}
