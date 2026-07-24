//! Task #25/#26 acceptance crate — compiles `demo/islands/product-card.tsx`
//! through the real loomc pipeline TWICE (`build.rs`, once per
//! `GraphMode`), instead of hand-authoring the Island structs the way
//! `crates/loom-island-demo` did.
//!
//! `generated` (`GraphMode::Islands`): `App`'s own `setup()`/`resume()`
//! (unqualified, root — see `codegen.rs`'s `generate_islands`) is the
//! page's SSR entry point (see `render_html()` below, used to produce
//! `demo/islands/compiled/index.html`'s initial markup); `ProductCardIsland`
//! /`WidgetIsland` (generated inside `product_card_generated::`/
//! `widget_generated::`) are this crate's compiled Island entry points,
//! wired by `demo/resume/registry.js` exactly like the hand-coded crate's
//! structs were — same JS glue, same `data-loom-island*` marker
//! convention, no changes needed there.
//!
//! `generated_flat` (`GraphMode::Flatten`, Task #26): the SAME source,
//! compiled as RQ6 §8 item 4's H-Qwik counterfactual — ONE merged
//! `Signals`/`Action`/`World`, no Island boundary, no `data-loom-island*`
//! attributes at all (see `render_html_flat()`). This is what makes the
//! §4.1 "정체성 통제" (identity control) principle literal for this
//! fixture: both arms are derived from `demo/islands/product-card.tsx`,
//! differing only in which `GraphMode` compiled them — not two
//! independently hand-written implementations.

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub mod generated_flat {
    include!(concat!(env!("OUT_DIR"), "/generated_flat.rs"));
}

// A minimal native-only DomOps impl for producing this page's initial SSR
// HTML (no wasm, no browser) — a separate small copy rather than a shared
// dependency, matching this project's established one-crate-one-StringDom
// convention (see loom-fic's module doc comment for the precedent and
// reasoning — that crate's copy stays JS-free for an unrelated reason this
// crate doesn't share, since this crate already depends on wasm-bindgen
// unconditionally for its Island entry points; the copies are kept
// separate anyway rather than introducing a new shared dependency for it).
#[cfg(not(target_arch = "wasm32"))]
mod string_dom {
    use loom_runtime::{DomOps, NodeHandle};

    #[derive(Default)]
    pub struct StringDom {
        tags: Vec<String>,
        attrs: Vec<Vec<(String, String)>>,
        text: Vec<Option<String>>,
        children: Vec<Vec<usize>>,
        visible: std::collections::HashMap<u32, bool>,
    }

    const TEXT_NODE_TAG: &str = "#text-wrapper";
    const RAW_HTML_TAG: &str = "#raw-html";

    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    impl StringDom {
        pub fn new_root(root_tag: &str) -> (Self, NodeHandle) {
            let mut dom = StringDom::default();
            let root = dom.alloc(root_tag);
            (dom, root)
        }

        fn alloc(&mut self, tag: &str) -> NodeHandle {
            let idx = self.tags.len();
            self.tags.push(tag.to_string());
            self.attrs.push(Vec::new());
            self.text.push(None);
            self.children.push(Vec::new());
            NodeHandle(idx as u32)
        }

        pub fn render_html(&self, root: NodeHandle) -> String {
            let mut out = String::new();
            self.render_node(root.0 as usize, &mut out);
            out
        }

        fn render_node(&self, idx: usize, out: &mut String) {
            let tag = &self.tags[idx];
            if tag == TEXT_NODE_TAG {
                if let Some(text) = &self.text[idx] {
                    out.push_str(&html_escape(text));
                }
                return;
            }
            if tag == RAW_HTML_TAG {
                if let Some(html) = &self.text[idx] {
                    out.push_str(html);
                }
                return;
            }
            out.push('<');
            out.push_str(tag);
            for (name, value) in &self.attrs[idx] {
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                out.push_str(&html_escape(value));
                out.push('"');
            }
            out.push('>');
            if let Some(text) = &self.text[idx] {
                out.push_str(&html_escape(text));
            }
            for &child in &self.children[idx] {
                self.render_node(child, out);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
    }

    impl DomOps for StringDom {
        fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
            let handle = self.alloc(tag);
            if let Some(class) = class {
                self.set_attribute(handle, "class", class);
            }
            self.children[parent.0 as usize].push(handle.0 as usize);
            handle
        }

        fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
            let handle = self.alloc(TEXT_NODE_TAG);
            self.text[handle.0 as usize] = Some(text.to_string());
            self.children[parent.0 as usize].push(handle.0 as usize);
        }

        fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
            let handle = self.alloc(RAW_HTML_TAG);
            self.text[handle.0 as usize] = Some(html.to_string());
            self.children[parent.0 as usize].push(handle.0 as usize);
        }

        fn set_text_content(&mut self, target: NodeHandle, text: &str) {
            self.text[target.0 as usize] = Some(text.to_string());
        }

        fn clear_children(&mut self, parent: NodeHandle) {
            self.children[parent.0 as usize].clear();
        }

        fn remove_node(&mut self, _node: NodeHandle) {
            unimplemented!("StringDom is SSR-only — nothing is removed during a one-shot initial render")
        }

        fn reposition(&mut self, _parent: NodeHandle, _node: NodeHandle, _before: Option<NodeHandle>) {
            unimplemented!("StringDom is SSR-only — nothing is reordered during a one-shot initial render")
        }

        fn set_visible(&mut self, node: NodeHandle, visible: bool) {
            self.visible.insert(node.0, visible);
        }

        fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
            self.attrs[node.0 as usize].push((name.to_string(), value.to_string()));
        }

        fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
            self.children[parent.0 as usize]
                .iter()
                .map(|&idx| (NodeHandle(idx as u32), self.text[idx].clone().unwrap_or_default()))
                .collect()
        }

        fn set_input_value(&mut self, node: NodeHandle, value: &str) {
            let idx = node.0 as usize;
            if let Some(entry) = self.attrs[idx].iter_mut().find(|(k, _)| k == "value") {
                entry.1 = value.to_string();
            } else {
                self.attrs[idx].push(("value".to_string(), value.to_string()));
            }
        }

        fn get_input_value(&mut self, node: NodeHandle) -> String {
            let idx = node.0 as usize;
            self.attrs[idx]
                .iter()
                .find(|(k, _)| k == "value")
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        }
    }
}

/// Renders `App`'s initial SSR HTML — the same content
/// `demo/islands/compiled/index.html`'s `#app` div is pasted from (see
/// `demo/resume/index.html`'s identical convention for Phase 2's
/// resumability demo). Native, no wasm, no browser — every embedding path
/// in this crate that needs *initial* markup goes through this.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_html() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    dom.render_html(root)
}

/// Task #26 — the H-Qwik counterfactual's SSR output, from the SAME source
/// `render_html()` compiles, just `GraphMode::Flatten`'d instead. RQ6 §8
/// item 4's cost/isolation measurements (M1/M6 etc.) compare this against
/// `render_html()`'s output.
#[cfg(not(target_arch = "wasm32"))]
pub fn render_html_flat() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated_flat::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    dom.render_html(root)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn render_html_produces_islands_with_the_five_expected_attributes_each() {
        let html = render_html();
        // ProductCard (client:visible) and Widget (client:load), each
        // tagged by their own generated setup() — see codegen.rs's
        // `generate_component`'s island-tagging block.
        assert!(html.contains(r#"data-loom-island-id="product-card""#), "{html}");
        assert!(html.contains(r#"data-loom-activation="visible""#), "{html}");
        assert!(html.contains(r#"data-loom-chunk="product_card""#), "{html}");
        assert!(html.contains(r#"data-loom-ctor="ProductCardIsland""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="widget""#), "{html}");
        assert!(html.contains(r#"data-loom-activation="load""#), "{html}");
        assert!(html.contains(r#"data-loom-chunk="widget""#), "{html}");
        assert!(html.contains(r#"data-loom-ctor="WidgetIsland""#), "{html}");
        // The conditional starts hidden (empty wrapper) — same SSR contract
        // every other conditional fixture in this repo already relies on.
        assert!(html.contains(r#"<p data-loom-id="details"></p>"#), "{html}");
        // Widget's own counter starts at its declared literal.
        assert!(html.contains(">0<"), "{html}");
    }

    #[test]
    fn render_html_flat_has_no_island_boundary_at_all() {
        // Task #26 — RQ6 §8 item 4's H-Qwik counterfactual: same signals/
        // static content as the Island arm, but zero Island markers, since
        // there's no separate compiled unit for ProductCard/Widget in this
        // mode at all — everything lives in App's own single Signals/
        // World/setup().
        let html = render_html_flat();
        assert!(!html.contains("data-loom-island"), "{html}");
        assert!(!html.contains("data-loom-chunk"), "{html}");
        assert!(!html.contains("data-loom-ctor"), "{html}");
        // M8-style consistency check: the same reference content is still
        // present — same static text, same data-loom-id markers, same
        // initial signal values — only the Island boundary is gone.
        assert!(html.contains(r#"<p data-loom-id="details"></p>"#), "{html}");
        assert!(html.contains(r#"data-loom-id="count""#), "{html}");
        assert!(html.contains(">0<"), "{html}");
        assert!(html.contains("show details"), "{html}");
    }
}
