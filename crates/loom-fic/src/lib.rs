//! Phase 4 — Framework Integration Contract (docs/05). A single, safe Rust
//! function (`render_html`) implements docs/05's `render(component_ref,
//! props, request_context) -> {html, resumability_payload, assets}`
//! reduced to what's actually decidable at this point in the project (see
//! below for the honest cut), plus a thin C ABI shim around it — the "C
//! shim이 모든 관심사를 흡수한다" (leo4) pattern docs/05 specifies. Every
//! embedding path (WASM-in-process, sidecar HTTP, native binding) goes
//! through this **one** shim; no per-language redesign.
//!
//! ## Reuse, not reinvention (CLAUDE.md 2-1) — with one accepted exception
//! `generated` (below) compiles the exact same `demo/counter-list.tsx`
//! every other Phase 1-3 demo uses (same `build.rs` pattern as loom-demo/
//! loom-resume-demo) — Phase 4 is not "build a new component to prove
//! backend-agnosticism," it's "prove the *existing* compiled output can
//! leave the wasm-bindgen world through a plain C ABI." `StringDom` below
//! is a **direct copy** of `loom-resume-demo::StringDom` (same file, same
//! behavior) rather than a shared dependency — reusing that crate directly
//! would drag in its unconditional `wasm-bindgen`/`web-sys` dependency
//! (needed there for the wasm-only `ResumedComponent` lazy-chunk path),
//! which breaks a **plain** `wasm32-unknown-unknown` build with no JS host
//! (the WASM-in-process path needs exactly that — see `examples/
//! wasm_in_process.py`: wasmtime has no `__wbindgen_placeholder__` imports
//! to satisfy). This ~90-line duplication is the accepted cost of keeping
//! this crate's own dependency graph free of any JS-only assumption; if
//! `StringDom` ever needs a third copy, factor it into a small shared
//! `loom-runtime`-adjacent crate then, not before.
//!
//! ## FIC shape, decided (docs/05 "미해결" resolved for this scope)
//! - `component_ref`: fixed at this crate's one compiled component — no
//!   registry/lookup mechanism exists yet (would need Phase 3's
//!   `ComponentGraph` generalization, not done — see roadmap/PHASES.md).
//! - `props`: none — Phase 1's IR has no way to parameterize a signal's
//!   initial value per request (documented repeatedly since Phase 2's LRef
//!   capture work: "Phase 1 IR이 시그널의 실제 첫 렌더 값이 선언 리터럴과
//!   달라지는 경우를 아예 지원 안 함"). `request_context` is therefore
//!   **trivially empty** for this round — not a considered minimization,
//!   an honest reflection of "there is nothing to parameterize yet."
//!   Revisit once the IR supports non-literal initial values.
//! - `resumability_payload`/`assets`: **not implemented this round** — the
//!   returned `html` already carries every `data-loom-id`/`data-loom-lref`
//!   attribute inline (same as every other SSR path in this repo), so a
//!   separate payload struct would just duplicate what a host can already
//!   read out of the HTML string — see codegen.rs. A real multi-asset
//!   manifest becomes necessary once Islands compile to genuinely separate
//!   chunks (Phase 3's "목표 상태" build tooling, still not built); until
//!   then this would be a struct with no real content, so it's left out
//!   rather than stubbed.
//!
//! ## Wire format (used by every embedding path identically)
//! `loom_fic_render()` returns a raw pointer to a buffer laid out as
//! `[u32 length (little-endian)][UTF-8 HTML bytes]`. No separate
//! out-pointer parameter — a WASM-in-process host (wasmtime, no shared
//! address space with the caller) can only read the callee's own linear
//! memory, so the length has to travel *inside* the one buffer the callee
//! already returns a pointer to, not through an extra pointer argument the
//! host would have no address for. The same convention is reused unchanged
//! for the native (same-address-space) call — one contract, not two.

mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

/// `bevy_ecs`/`hashbrown` need *some* randomness source on
/// `wasm32-unknown-unknown` (`HashMap`'s DOS-resistance seed) — normally
/// satisfied by `getrandom`'s `wasm_js` backend, but that backend itself
/// depends on `wasm-bindgen`/`js-sys` to call `crypto.getRandomValues`,
/// which defeats the entire point of this crate's WASM-in-process path
/// (a plain wasmtime host, no JS, no `__wbindgen_*` imports to satisfy —
/// see this module's doc comment and `examples/wasm_in_process.py`).
/// `getrandom`'s "custom backend" mechanism lets a crate provide its own
/// randomness function instead, with **no** JS dependency. This
/// implementation is a fixed, non-cryptographic fill — accepted *only*
/// because `render_html()` never uses randomness for anything
/// security-relevant (a deterministic, single-shot SSR of a fixed compiled
/// component); a real production WASM-in-process host would instead
/// import an actual RNG function from its own environment. Active only
/// when built with `RUSTFLAGS='--cfg getrandom_backend="custom"'` (see
/// `examples/wasm_in_process.py`'s build instructions) — the native/sidecar
/// paths use the normal OS entropy source and never touch this.
#[cfg(all(target_arch = "wasm32", getrandom_backend = "custom"))]
#[no_mangle]
unsafe extern "Rust" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), getrandom::Error> {
    for i in 0..len {
        unsafe {
            *dest.add(i) = (i as u8).wrapping_mul(197).wrapping_add(41);
        }
    }
    Ok(())
}

use loom_runtime::{DomOps, NodeHandle};

/// Direct copy of `loom_resume_demo::StringDom` — see this module's doc
/// comment for why it's duplicated rather than shared.
#[derive(Default)]
struct StringDom {
    tags: Vec<String>,
    attrs: Vec<Vec<(String, String)>>,
    text: Vec<Option<String>>,
    children: Vec<Vec<usize>>,
    visible: std::collections::HashMap<u32, bool>,
}

const TEXT_NODE_TAG: &str = "#text-wrapper";
/// Sentinel "tag" marking a node as a pre-rendered raw HTML blob (see
/// `DomOps::append_static_html`) — `render_node` emits its stored text
/// verbatim, no escaping, no wrapping tag.
const RAW_HTML_TAG: &str = "#raw-html";

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl StringDom {
    fn new_root(root_tag: &str) -> (Self, NodeHandle) {
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

    fn render_html(&self, root: NodeHandle) -> String {
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
        unimplemented!("StringDom is SSR-only — list rows are never removed during a one-shot initial render")
    }

    fn reposition(&mut self, _parent: NodeHandle, _node: NodeHandle, _before: Option<NodeHandle>) {
        unimplemented!("StringDom is SSR-only — nothing gets reordered during a one-shot initial render")
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

/// The actual render step — safe Rust, no `unsafe`, no FFI concerns. Every
/// embedding path in this crate calls this and nothing else.
pub fn render_html() -> String {
    let (mut dom, root) = StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    dom.render_html(root)
}

/// C ABI shim (leo4 pattern, docs/05). Returns a pointer to a
/// `[len: u32 LE][utf8 bytes]` buffer — see this module's doc comment for
/// why length travels inside the buffer rather than via an out-parameter.
/// Caller **must** pass the returned pointer and the buffer's *total*
/// length (4 + html byte length) to `loom_fic_free` exactly once.
#[no_mangle]
pub extern "C" fn loom_fic_render() -> *mut u8 {
    let html = render_html();
    let html_bytes = html.into_bytes();
    let len = html_bytes.len() as u32;
    let mut buf = Vec::with_capacity(4 + html_bytes.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&html_bytes);
    let mut boxed = buf.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    ptr
}

/// Frees a buffer `loom_fic_render` returned. `total_len` is the *whole*
/// buffer's length (4-byte length prefix included), not just the HTML
/// portion — callers get this by reading the length prefix and adding 4,
/// not by measuring the HTML string they decoded.
#[no_mangle]
pub extern "C" fn loom_fic_free(ptr: *mut u8, total_len: u32) {
    unsafe {
        drop(Vec::from_raw_parts(ptr, total_len as usize, total_len as usize));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_html_produces_the_same_ssr_shape_every_other_demo_does() {
        let html = render_html();
        assert!(html.contains(r#"data-loom-id="count">0<"#));
        assert!(html.contains("<li>a</li><li>b</li><li>c</li>"));
        assert!(html.contains(r#"data-loom-lref="App_chunk|Action0|count:0""#));
    }

    #[test]
    fn c_abi_shim_round_trips_the_same_html_render_html_returns() {
        let expected = render_html();
        unsafe {
            let ptr = loom_fic_render();
            let len = u32::from_le_bytes([*ptr, *ptr.add(1), *ptr.add(2), *ptr.add(3)]);
            let bytes = std::slice::from_raw_parts(ptr.add(4), len as usize);
            let html = std::str::from_utf8(bytes).unwrap().to_string();
            assert_eq!(html, expected);
            loom_fic_free(ptr, 4 + len);
        }
    }
}
