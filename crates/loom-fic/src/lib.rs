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
//! (needed there for the wasm-only `ResumedApp` lazy-chunk path),
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
//! - `resumability_payload`/`assets`: **2026-07-28 (Task #28-c) — 실제로
//!   구현**. `render_html_full()`/`loom_fic_render_full()`를 기존
//!   `render_html()`/`loom_fic_render()`와 나란히 추가(대체 아님 — 기존 호출자
//!   무변경). 정직한 스코프: `resumability_payload`는 `html`에 이미 인라인된
//!   `data-loom-lref` 속성들을 *구조화해* 뽑아낸 것(문자열 재파싱이 아니라
//!   `StringDom`이 이미 갖고 있는 구조화된 attr 목록에서 직접 추출) — 여전히
//!   `html`이 담은 정보를 "중복"하지만, 호스트가 문자열을 다시 파싱하지 않고
//!   구조체로 바로 쓸 수 있다는 실질적 가치가 있다. `assets`는 **여전히 항상
//!   빈 배열** — 이 크레이트는 이 컴포넌트를 Island 청크로 쪼개 빌드하는
//!   툴링이 없으므로(Phase 3의 "목표 상태", 여전히 미착수) 진짜 asset
//!   목록이 존재하지 않는다; 빈 배열로 정직하게 두는 것이지 "구현했지만
//!   우연히 비었다"가 아니다.
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
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
#[unsafe(no_mangle)]
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

impl StringDom {
    /// Task #28-c — every `data-loom-lref` attribute this render pass set,
    /// parsed into structured fields. Reads directly from `self.attrs`
    /// (already-structured `Vec<(String, String)>` per node, built by
    /// `set_attribute` during `setup()`) — not a re-parse of the rendered
    /// HTML string, so this can never drift from what `render_html()` above
    /// actually emitted for the same `DomOps` calls.
    fn collect_lref_entries(&self) -> Vec<LRefEntry> {
        self.attrs
            .iter()
            .filter_map(|attrs| attrs.iter().find(|(k, _)| k == "data-loom-lref"))
            .map(|(_, lref)| {
                let mut parts = lref.splitn(3, '|');
                LRefEntry {
                    chunk: parts.next().unwrap_or_default().to_string(),
                    symbol: parts.next().unwrap_or_default().to_string(),
                    captures: parts.next().unwrap_or_default().to_string(),
                }
            })
            .collect()
    }
}

/// docs/05's `resumability_payload`'s per-action entry, structured — the
/// same `chunk|symbol|captures` triple `data-loom-lref` already carries
/// inline in `html`, just parsed once here instead of left for every host
/// to re-parse out of the markup itself.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct LRefEntry {
    pub chunk: String,
    pub symbol: String,
    pub captures: String,
}

/// docs/05's FIC contract, `{html, resumability_payload, assets}`, made
/// concrete for this crate's one compiled component. See this module's doc
/// comment ("resumability_payload/assets" section) for exactly what's
/// honest content here and what's a stated placeholder.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
pub struct FicRenderResult {
    pub html: String,
    pub resumability_payload: Vec<LRefEntry>,
    /// Always empty this round — see module doc comment.
    pub assets: Vec<String>,
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

/// Task #28-c — docs/05's FIC contract in full: `{html,
/// resumability_payload, assets}`, not just `html`. Added alongside
/// `render_html()` (unchanged, still what every existing caller uses) —
/// see this module's doc comment for what's honest content vs. a stated
/// placeholder here.
pub fn render_html_full() -> FicRenderResult {
    let (mut dom, root) = StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    let resumability_payload = dom.collect_lref_entries();
    let html = dom.render_html(root);
    FicRenderResult {
        html,
        resumability_payload,
        assets: Vec::new(),
    }
}

/// RQ6 §4.5.1 (M11) — the H-Qwik "monolithic" half of the ISR partial-
/// regeneration comparison, **naive variant**: `n_sections` static
/// `<section>` blocks (standing in for "the rest of a large page" — RQ6
/// §4.4's "원본 앱 규모" axis) plus this crate's one dynamic widget, each
/// section created via its own `create_element`/`append_static_text` calls
/// — the pre-2026-07-24 codegen shape (see git history: this is what every
/// static element used to compile to, one runtime `DomOps` call per node,
/// before `append_static_html`/`try_render_fully_static` existed). Kept
/// alongside `render_large_page_monolithic_optimized` (below) specifically
/// so the *size* of the optimization's effect is itself measurable, not
/// just its existence.
pub fn render_large_page_monolithic_naive(n_sections: usize) -> (String, std::time::Duration) {
    let start = std::time::Instant::now();
    let (mut dom, root) = StringDom::new_root("div");
    for i in 0..n_sections {
        let section = dom.create_element(root, "section", None);
        dom.append_static_text(section, &format!("static section {i}"));
    }
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    let html = dom.render_html(root);
    (html, start.elapsed())
}

/// RQ6 §4.5.1 (M11) — the H-Qwik "monolithic" half, **optimized variant**:
/// what codegen.rs's `try_render_fully_static`/`DomOps::append_static_html`
/// optimization (2026-07-24) actually buys a monolithic component — the
/// `n_sections` static blocks are built as one plain Rust `String` (no
/// `DomOps`/ECS calls at all while building it, just `push_str`) and hand
/// to `append_static_html` in a **single** call, exactly like real codegen
/// now does for any fully-static subtree it can see at compile time (see
/// that function's doc comment). This is the fair comparison point for the
/// question 병익님 actually asked — "what if static content were fully
/// compile-time-processed so runtime has no VDOM-like per-node overhead" —
/// answered empirically here rather than assumed. **What this does NOT
/// change**: the fundamental ISR asymmetry. Even at this optimized
/// constant-per-byte cost, a monolithic component still has to *touch*
/// (build + copy) all `n_sections` worth of bytes on every regeneration,
/// because it's one inseparable render() call — `render_widget_only_for_isr`
/// below still touches zero of those bytes, ever, because the static page
/// and the widget are different compiled units. The scaling *slope*
/// should shrink a lot; whether it goes to *zero* (making the two paths
/// equivalent) is exactly what actually running this answers.
pub fn render_large_page_monolithic_optimized(n_sections: usize) -> (String, std::time::Duration) {
    let start = std::time::Instant::now();
    let mut static_html = String::new();
    for i in 0..n_sections {
        static_html.push_str("<section>static section ");
        static_html.push_str(&i.to_string());
        static_html.push_str("</section>");
    }
    let (mut dom, root) = StringDom::new_root("div");
    dom.append_static_html(root, &static_html);
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    let html = dom.render_html(root);
    (html, start.elapsed())
}

/// RQ6 §4.5.1 (M11) — the Island half: regenerating *only* the widget
/// (`render_html`, unchanged, timed here) while the surrounding
/// `n_sections`-large page is assumed already cached/unaffected (an Island
/// architecture's whole premise — the static page and the widget are
/// separate compiled units, so this call never even sees `n_sections`).
/// The `n_sections` parameter is accepted only so callers can log which
/// page-size condition this timing corresponds to; it plays no role in the
/// computation itself — that absence *is* the point being measured.
pub fn render_widget_only_for_isr(_n_sections: usize) -> (String, std::time::Duration) {
    let start = std::time::Instant::now();
    let html = render_html();
    (html, start.elapsed())
}

/// C ABI shim (leo4 pattern, docs/05). Returns a pointer to a
/// `[len: u32 LE][utf8 bytes]` buffer — see this module's doc comment for
/// why length travels inside the buffer rather than via an out-parameter.
/// Caller **must** pass the returned pointer and the buffer's *total*
/// length (4 + html byte length) to `loom_fic_free` exactly once.
#[unsafe(no_mangle)]
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

/// Task #28-c — the full FIC contract over the C ABI: same
/// `[len: u32 LE][bytes]` wire convention `loom_fic_render` uses, except
/// `bytes` is now the UTF-8 JSON encoding of a `FicRenderResult` (`html` +
/// `resumability_payload` + `assets`), not raw HTML. Freed with the same
/// `loom_fic_free` — the wire format's shape (length-prefixed byte buffer)
/// doesn't care what the bytes mean.
#[unsafe(no_mangle)]
pub extern "C" fn loom_fic_render_full() -> *mut u8 {
    let result = render_html_full();
    let json_bytes =
        serde_json::to_vec(&result).expect("FicRenderResult has no non-serializable fields — cannot fail");
    let len = json_bytes.len() as u32;
    let mut buf = Vec::with_capacity(4 + json_bytes.len());
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&json_bytes);
    let mut boxed = buf.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    ptr
}

/// Frees a buffer `loom_fic_render`/`loom_fic_render_full` returned.
/// `total_len` is the *whole* buffer's length (4-byte length prefix
/// included), not just the HTML/JSON portion — callers get this by reading
/// the length prefix and adding 4, not by measuring the decoded payload.
#[unsafe(no_mangle)]
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

    #[test]
    fn render_html_full_matches_render_html_and_extracts_every_lref() {
        // Task #28-c — html 필드는 render_html()과 byte-identical해야 하고
        // (같은 DomOps 호출로 만들어지므로), resumability_payload는 그 html에
        // 인라인된 data-loom-lref 속성들을 정확히 구조화해 담아야 한다.
        let full = render_html_full();
        assert_eq!(full.html, render_html());
        assert!(!full.resumability_payload.is_empty(), "counter-list.tsx has multiple actions");
        assert!(full.resumability_payload.iter().any(|e| e.chunk == "App_chunk"
            && e.symbol == "Action0"
            && e.captures == "count:0"));
        // Append/Rotate는 캡처 없음(minimal-capture computation) — 빈 문자열
        // captures로 구조화되어 있어야지 아예 빠지면 안 됨.
        assert!(full
            .resumability_payload
            .iter()
            .any(|e| e.chunk == "App_chunk" && e.symbol == "Action1" && e.captures.is_empty()));
        assert!(full.assets.is_empty(), "no Island chunk build tooling exists for this crate yet — see module doc comment");
    }

    #[test]
    fn c_abi_full_shim_round_trips_the_same_result_render_html_full_returns() {
        let expected = render_html_full();
        unsafe {
            let ptr = loom_fic_render_full();
            let len = u32::from_le_bytes([*ptr, *ptr.add(1), *ptr.add(2), *ptr.add(3)]);
            let bytes = std::slice::from_raw_parts(ptr.add(4), len as usize);
            let decoded: FicRenderResult = serde_json::from_slice(bytes).unwrap();
            assert_eq!(decoded, expected);
            loom_fic_free(ptr, 4 + len);
        }
    }

    #[test]
    fn large_page_monolithic_naive_includes_both_static_sections_and_the_widget() {
        let (html, _elapsed) = render_large_page_monolithic_naive(3);
        assert!(html.contains("<section>static section 0</section>"));
        assert!(html.contains("<section>static section 2</section>"));
        assert!(html.contains(r#"data-loom-id="count">0<"#), "the widget itself must still be present");
    }

    #[test]
    fn large_page_monolithic_optimized_produces_byte_identical_html_to_naive() {
        // The optimization must never change *what* gets rendered, only
        // *how many DomOps calls* it costs to render it.
        let (naive_html, _) = render_large_page_monolithic_naive(5);
        let (optimized_html, _) = render_large_page_monolithic_optimized(5);
        assert_eq!(naive_html, optimized_html);
    }

    #[test]
    fn widget_only_render_ignores_n_sections_and_matches_render_html() {
        let (html_small, _) = render_widget_only_for_isr(0);
        let (html_large, _) = render_widget_only_for_isr(50_000);
        assert_eq!(html_small, html_large, "n_sections must not affect this path's output at all");
        assert_eq!(html_small, render_html());
    }
}
