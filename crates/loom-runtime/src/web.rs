//! A real-browser `DomOps` implementation, shared by `loom-demo` and any
//! benchmark harness so neither hand-rolls its own copy. Gated behind the
//! `browser` cargo feature (wasm-bindgen/web-sys aren't needed for the
//! native `Changed<T>`/reconciliation tests in `tests/`).

use crate::{DomOps, NodeHandle};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// `Element.moveBefore` isn't in web-sys yet (it's a very recent WHATWG
/// addition — Chrome/Edge 133+, RQ1-benchmark-protocol.md's candidate E
/// cites it), and it can't be added as a static extern-method binding on an
/// *already-defined* foreign type (`web_sys::Element`) — wasm-bindgen's
/// `method` attribute expands to an inherent `impl` on its `this` type,
/// which Rust's orphan rules forbid for a type this crate doesn't own. Call
/// it dynamically via `Reflect`/`Function` instead. Returns `Err` if the
/// method doesn't exist (older engines) or throws — callers must fall back
/// to `insertBefore` in that case (`BrowserDom::reposition`, the only caller).
fn move_before(
    parent: &web_sys::Element,
    node: &web_sys::Node,
    before: Option<&web_sys::Node>,
) -> Result<(), JsValue> {
    let method = js_sys::Reflect::get(parent, &JsValue::from_str("moveBefore"))?;
    let method: js_sys::Function = method.dyn_into()?;
    let before_arg: JsValue = before.cloned().map(JsValue::from).unwrap_or(JsValue::NULL);
    method.call2(parent, node, &before_arg)?;
    Ok(())
}

/// Nodes are addressed by the same integer `NodeHandle` scheme
/// loom-runtime's Components use — the actual `web_sys::Node` lives only in
/// this side table, never inside an ECS Component (CLAUDE.md 금지 4).
pub struct BrowserDom {
    document: web_sys::Document,
    nodes: Vec<web_sys::Node>,
}

impl BrowserDom {
    pub fn new(document: web_sys::Document, mount: web_sys::Node) -> (Self, NodeHandle) {
        let dom = BrowserDom {
            document,
            nodes: vec![mount],
        };
        (dom, NodeHandle(0))
    }

    /// Phase 2 resume-side constructor: no fresh mount is created (resuming
    /// never spawns new top-level structure — every node it needs already
    /// exists in the SSR'd document and is found via `adopt`).
    pub fn new_for_resume(document: web_sys::Document) -> Self {
        BrowserDom { document, nodes: Vec::new() }
    }

    /// Registers an *already-existing* DOM node (found via `data-loom-id`,
    /// not created here) under a fresh `NodeHandle` — the resume-time
    /// counterpart to `create_element`'s "allocate + track" half, without the
    /// "create" half. See loom-compiler's generated `resume()`.
    pub fn adopt(&mut self, node: web_sys::Node) -> NodeHandle {
        self.alloc(node)
    }

    fn node(&self, handle: NodeHandle) -> &web_sys::Node {
        &self.nodes[handle.0 as usize]
    }

    fn alloc(&mut self, node: web_sys::Node) -> NodeHandle {
        let handle = NodeHandle(self.nodes.len() as u32);
        self.nodes.push(node);
        handle
    }

    /// The real `web_sys::Element` for a handle produced by `create_element`
    /// — used only by event-wiring code, which lives outside the `DomOps`
    /// trait entirely (loom-demo's `start()`/benchmark harness own that).
    pub fn element(&self, handle: NodeHandle) -> web_sys::Element {
        self.node(handle).clone().unchecked_into()
    }
}

impl DomOps for BrowserDom {
    fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
        let el = self
            .document
            .create_element(tag)
            .unwrap_or_else(|e| panic!("create_element({tag}) failed: {e:?}"));
        if let Some(class) = class {
            el.set_class_name(class);
        }
        self.node(parent)
            .append_child(&el)
            .expect("append_child failed");
        self.alloc(el.unchecked_into())
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        let text_node = self.document.create_text_node(text);
        self.node(parent)
            .append_child(&text_node)
            .expect("append_child failed");
        // Never patched again, so no handle is kept for it.
    }

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        self.node(target).set_text_content(Some(text));
    }

    fn clear_children(&mut self, parent: NodeHandle) {
        // `textContent = null` removes every child in one native call — the
        // browser-DOM realization of RQ1's "clear_children fast path".
        self.node(parent).set_text_content(None);
    }

    fn remove_node(&mut self, node: NodeHandle) {
        let n = self.node(node).clone();
        if let Some(parent) = n.parent_node() {
            parent.remove_child(&n).expect("remove_child failed");
        }
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        let el: web_sys::HtmlElement = self.node(node).clone().unchecked_into();
        el.style()
            .set_property("display", if visible { "" } else { "none" })
            .expect("set display style failed");
    }

    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        let el: web_sys::Element = self.node(node).clone().unchecked_into();
        el.set_attribute(name, value).expect("set_attribute failed");
    }

    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        let parent_node = self.node(parent).clone();
        let children = parent_node.child_nodes();
        let mut out = Vec::with_capacity(children.length() as usize);
        for i in 0..children.length() {
            let Some(child) = children.item(i) else { continue };
            let text = child.text_content().unwrap_or_default();
            let handle = self.alloc(child);
            out.push((handle, text));
        }
        out
    }

    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        let parent_node = self.node(parent).clone();
        let node_ref = self.node(node).clone();
        let before_ref = before.map(|h| self.node(h).clone());

        let parent_el: web_sys::Element = parent_node.clone().unchecked_into();
        if move_before(&parent_el, &node_ref, before_ref.as_ref()).is_ok() {
            return; // state-preserving native move succeeded.
        }
        // Fell through: engine has no `moveBefore`, or the call threw.
        // Fallback (RQ1 candidate E's documented Safari path): plain
        // `insertBefore` still relocates an *existing* node without
        // recreating it — it just doesn't carry `moveBefore`'s extra state
        // guarantees (e.g. avoiding an iframe/video reload).
        parent_node
            .insert_before(&node_ref, before_ref.as_ref())
            .expect("insert_before fallback failed");
    }
}
