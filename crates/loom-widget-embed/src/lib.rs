//! Task #26 — RQ6 §4.2 widget-embed fixture, compiled through the real
//! loomc pipeline TWICE (`build.rs`, once per `GraphMode`) from the SAME
//! `demo/widget-embed.tsx` source — RQ6 §4.1's "정체성 통제" principle made
//! literal.
//!
//! `generated` (`GraphMode::Islands`): `App`'s own `setup()`/`resume()` is
//! the page's SSR entry point (`render_html()`); `ProductCardIsland`/
//! `QuantityPickerIsland`/`CheckoutFormIsland`/`MiniDashboardIsland`
//! (generated inside their own namespaced modules) are this crate's
//! compiled Island entry points, wired by `demo/resume/registry.js` and
//! `demo/resume/shared-store.js` exactly like Task #25's smaller fixture.
//!
//! `generated_flat` (`GraphMode::Flatten`): the SAME source as RQ6 §8 item
//! 4's H-Qwik counterfactual — ONE merged `Signals`/`Action`/`World`, no
//! Island boundary at all (`render_html_flat()`).

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub mod generated_flat {
    include!(concat!(env!("OUT_DIR"), "/generated_flat.rs"));
}

// A minimal native-only DomOps impl for producing this page's initial SSR
// HTML — see crates/loom-island-compiled's identical copy for why this
// isn't shared as a dependency (established one-crate-one-StringDom
// convention in this repo).
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

/// `App`'s initial SSR HTML (`GraphMode::Islands`) — App itself authors no
/// `sharedSignal(...)` (only `QuantityPicker`, nested, does), so App's own
/// `setup()` keeps its plain 3-arg signature — no shared-store stub needed
/// at this top level (see `codegen.rs`'s `ComponentRef` arm for where a
/// throwaway one gets threaded in instead, one level down).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_html() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    dom.render_html(root)
}

/// The H-Qwik counterfactual's SSR output (`GraphMode::Flatten`) — since
/// flattening merges `QuantityPicker`'s `sharedSignal` binding straight
/// into the one merged component, the merged `setup()` DOES need a
/// shared-store argument here (unlike `render_html()` above) — a plain
/// native stub is enough (SSR never reads back what it publishes).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_html_flat() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let mut shared = loom_runtime::InMemorySharedStore::default();
    let (_signals, _actions, _input_actions) = generated_flat::setup(&mut app, &mut dom, root, &mut shared);
    app.tick(&mut dom);
    dom.render_html(root)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn render_html_tags_all_four_islands_correctly() {
        let html = render_html();
        assert!(html.contains(r#"data-loom-island-id="product-card""#), "{html}");
        assert!(html.contains(r#"data-loom-activation="visible""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="quantity-picker""#), "{html}");
        assert!(html.contains(r#"data-loom-activation="load""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="checkout-form""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="mini-dashboard""#), "{html}");
        // CartBadge: no chunk of its own, just a plain subscriber marker.
        assert!(html.contains(r#"data-loom-shared-subscriber="cartCount""#), "{html}");
        assert!(!html.contains(r#"data-loom-chunk="app""#), "{html}"); // App itself is never an Island
    }

    #[test]
    fn render_html_matches_render_html_flat_on_shared_reference_content() {
        // M8-style consistency check: both arms show the same reference
        // content (same static text, same data-loom-id markers, same
        // initial values) — only the Island boundary differs.
        let islands = render_html();
        let flat = render_html_flat();
        for needle in [
            r#"data-loom-shared-subscriber="cartCount""#,
            "show details",
            "toggle express shipping",
            "toggle gift wrap",
            "submit order",
            "add tag",
            "news",
            "sale",
            "+1 visit",
        ] {
            assert!(islands.contains(needle), "islands missing {needle:?}: {islands}");
            assert!(flat.contains(needle), "flat missing {needle:?}: {flat}");
        }
        // The Island boundary itself is the one thing that must differ.
        assert!(islands.contains("data-loom-island"));
        assert!(!flat.contains("data-loom-island"));
    }
}
