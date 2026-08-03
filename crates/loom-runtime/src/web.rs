//! A real-browser `DomOps` implementation, shared by `loom-demo` and any
//! benchmark harness so neither hand-rolls its own copy. Gated behind the
//! `browser` cargo feature (wasm-bindgen/web-sys aren't needed for the
//! native `Changed<T>`/reconciliation tests in `tests/`).

use crate::{DomOps, FragmentHandle, NodeHandle, SharedStore};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__loomShared"])]
    fn set(key: &str, value: i32);
    #[wasm_bindgen(js_namespace = ["window", "__loomShared"])]
    fn get(key: &str) -> JsValue;
}

/// Real `window.__loomShared` (`demo/resume/shared-store.js`) binding —
/// generated code (`setup`/`apply_action`/`resume`, when a component
/// authors `sharedSignal(...)`) calls this through the `SharedStore` trait,
/// same "host-agnostic generated code, host adapter implements the trait"
/// shape `DomOps`/`BrowserDom` already established. Replaces what used to
/// be a hand-written `#[wasm_bindgen] extern "C" { ... }` block duplicated
/// per Island crate (`crates/loom-island-demo`, before Task #26).
pub struct JsSharedStore;

impl SharedStore for JsSharedStore {
    fn set(&mut self, key: &str, value: i32) {
        set(key, value);
    }

    fn get(&mut self, key: &str) -> Option<i32> {
        get(key).as_f64().map(|v| v as i32)
    }
}

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

/// Walks `host`'s descendants collecting every `[data-loom-id]` element,
/// **without recursing past a nested `[data-loom-island]` boundary** — RQ3's
/// opaque-boundary rule (docs/06, RQ3's "DOM 소유권" clause) applied at
/// DOM-query time. Without this, a parent Island's own query would also pick
/// up a nested child Island's tagged elements, since the child's host is
/// physically nested inside the parent's — the same bug class
/// `ResumedComponent::new`'s page-global query has (see codegen.rs's module
/// doc and roadmap/PHASES.md's "resume 시점 DOM 쿼리" finding).
///
/// Promoted here (2026-07-24, Task #25) from three independent, byte-for-byte
/// duplicated copies of this exact function
/// (`loom-island-demo`/`loom-island-productcard`/`loom-island-quantitypicker`,
/// each of which duplicated it deliberately at the time since each crate's
/// entire point was being a separate wasm compilation unit — see those
/// crates' own doc comments). A *shared dependency crate* (as opposed to
/// shared wasm-instance state) doesn't compromise that separation — only
/// duplicated **behavior**, not a duplicated **binary**, was the actual
/// requirement — so hoisting this into `loom-runtime` (already a shared
/// dependency of all three) removes the duplication without weakening
/// anything condition B's fault-isolation spike depends on.
pub fn own_tagged_descendants(host: &web_sys::Element) -> Vec<web_sys::Element> {
    let mut found = Vec::new();
    let children = host.children();
    for i in 0..children.length() {
        let Some(child) = children.item(i) else { continue };
        if child.has_attribute("data-loom-island") {
            continue; // opaque — a nested Island's own DOM, not ours to touch.
        }
        if child.has_attribute("data-loom-id") {
            found.push(child.clone());
        }
        found.extend(own_tagged_descendants(&child));
    }
    found
}

/// Same opaque-boundary walk as `own_tagged_descendants`, but collecting
/// `[data-loom-lref]` elements instead of `[data-loom-id]` ones — the shape
/// a compiler-generated Island entry point needs to find its own action
/// dispatch sites (see codegen.rs's `generate_island_entry`), as opposed to
/// its own signal-bearing text/input nodes.
pub fn own_tagged_lref_descendants(host: &web_sys::Element) -> Vec<web_sys::Element> {
    let mut found = Vec::new();
    let children = host.children();
    for i in 0..children.length() {
        let Some(child) = children.item(i) else { continue };
        if child.has_attribute("data-loom-island") {
            continue;
        }
        if child.has_attribute("data-loom-lref") {
            found.push(child.clone());
        }
        found.extend(own_tagged_lref_descendants(&child));
    }
    found
}

/// `own_tagged_descendants`, pre-indexed by name — the shape most callers
/// (both the hand-coded Island demos and codegen's generated `find_node`
/// closures) actually want, rather than a linear `Vec` scanned once per name.
pub fn index_own_tagged_descendants(
    host: &web_sys::Element,
) -> std::collections::HashMap<String, web_sys::Element> {
    own_tagged_descendants(host)
        .into_iter()
        .filter_map(|el| {
            let name = el.get_attribute("data-loom-id")?;
            Some((name, el))
        })
        .collect()
}

/// Looks up one `[data-loom-id]`-tagged element by name within `host`'s own
/// (opaque-boundary-respecting) scope — panics if absent, matching this
/// project's "narrow explicit subset, panic on anything unsupported"
/// convention (same message shape the three original hand-coded copies used).
pub fn find_own(host: &web_sys::Element, name: &str) -> web_sys::Element {
    own_tagged_descendants(host)
        .into_iter()
        .find(|el| el.get_attribute("data-loom-id").as_deref() == Some(name))
        .unwrap_or_else(|| {
            panic!("loom_runtime::web::find_own: no own-scope element tagged data-loom-id=\"{name}\" under this host")
        })
}

/// Nodes are addressed by the same integer `NodeHandle` scheme
/// loom-runtime's Components use — the actual `web_sys::Node` lives only in
/// this side table, never inside an ECS Component (CLAUDE.md 금지 4).
pub struct BrowserDom {
    document: web_sys::Document,
    /// `None` = **해제된 슬롯**.
    ///
    /// 2026-08-03 이전에는 `Vec<web_sys::Node>`였고 **어떤 경로로도 줄지
    /// 않았다**. `clear_children`/`remove_node`가 노드를 문서에서 떼어내도
    /// 여기 `web_sys::Node`(= wasm-bindgen heap 슬롯)가 살아 있어 GC 대상이
    /// 되지 않았다. K는 매 tick 전량 재스폰하므로 n=10,000 조합에서 셀당
    /// 504 tick × 7 워크로드 = 3,528 tick × 10,000 = **3,528만 개**의 분리된
    /// `<li>`가 상주했다(노드당 실측 ~300B → 약 10 GB). 워밍업 50 + 측정
    /// 200 도입 전에는 tick 수가 48배 적어 드러나지 않았고, 축소 스모크에서는
    /// 원리적으로 드러날 수 없다.
    nodes: Vec<Option<web_sys::Node>>,
    /// 각 핸들이 `create_element`로 만든 **자식 핸들들**. 해제를 재귀적으로
    /// 하려면 필요하다 — `clear_children(parent)`는 자식 핸들을 인자로 받지
    /// 않으므로 이 표가 없으면 무엇을 해제할지 알 수 없다.
    children: Vec<Vec<u32>>,
    /// 부모 핸들(`u32::MAX` = 없음). `remove_node` 시 부모의 자식 목록에서
    /// 빼기 위해 필요하다.
    parent_of: Vec<u32>,
    /// 재사용 가능한 슬롯. 핸들을 재사용하지 않으면 슬롯 자체가 수천만 개로
    /// 자라 부기(簿記)만으로 GB급이 된다.
    free: Vec<u32>,
    /// **한 배치 안에서는 재사용하지 않는다**. 배치 도중 해제된 핸들이 같은
    /// 배치의 뒤쪽 write에 anchor로 등장하면 엉뚱한 노드를 가리키게 되기
    /// 때문이다. `begin_batch()`에서만 `free`로 옮긴다.
    pending_free: Vec<u32>,
    /// RQ1 Track T F 계열의 **비직렬화 사이드테이블**: 분리 보관 중인
    /// DocumentFragment들. Component에는 `FragmentHandle`(정수)만 들어간다
    /// (RQ1 "F 공통 코어", CLAUDE.md 금지 4).
    fragments: Vec<web_sys::DocumentFragment>,
    /// §7 M2: `reposition`이 네이티브 `moveBefore`로 갔는지 `insertBefore`
    /// 폴백으로 갔는지 누적. 엔진별로 갈리므로(§8 "E 3중 측정") 셀마다 실제로
    /// 어느 경로였는지를 기록해야 한다.
    reposition_native: u64,
    reposition_fallback: u64,
}

impl BrowserDom {
    pub fn new(document: web_sys::Document, mount: web_sys::Node) -> (Self, NodeHandle) {
        let dom = BrowserDom {
            document,
            nodes: vec![Some(mount)],
            children: vec![Vec::new()],
            parent_of: vec![u32::MAX],
            free: Vec::new(),
            pending_free: Vec::new(),
            fragments: Vec::new(),
            reposition_native: 0,
            reposition_fallback: 0,
        };
        (dom, NodeHandle(0))
    }

    /// Phase 2 resume-side constructor: no fresh mount is created (resuming
    /// never spawns new top-level structure — every node it needs already
    /// exists in the SSR'd document and is found via `adopt`).
    pub fn new_for_resume(document: web_sys::Document) -> Self {
        BrowserDom {
            document,
            nodes: Vec::new(),
            children: Vec::new(),
            parent_of: Vec::new(),
            free: Vec::new(),
            pending_free: Vec::new(),
            fragments: Vec::new(),
            reposition_native: 0,
            reposition_fallback: 0,
        }
    }

    /// Registers an *already-existing* DOM node (found via `data-loom-id`,
    /// not created here) under a fresh `NodeHandle` — the resume-time
    /// counterpart to `create_element`'s "allocate + track" half, without the
    /// "create" half. See loom-compiler's generated `resume()`.
    pub fn adopt(&mut self, node: web_sys::Node) -> NodeHandle {
        self.alloc(node)
    }

    fn node(&self, handle: NodeHandle) -> &web_sys::Node {
        self.nodes[handle.0 as usize]
            .as_ref()
            .unwrap_or_else(|| panic!("해제된 NodeHandle({})을 다시 썼다", handle.0))
    }

    fn alloc(&mut self, node: web_sys::Node) -> NodeHandle {
        self.alloc_under(node, None)
    }

    fn alloc_under(&mut self, node: web_sys::Node, parent: Option<NodeHandle>) -> NodeHandle {
        let idx = match self.free.pop() {
            Some(i) => {
                self.nodes[i as usize] = Some(node);
                self.children[i as usize].clear();
                i
            }
            None => {
                self.nodes.push(Some(node));
                self.children.push(Vec::new());
                self.parent_of.push(u32::MAX);
                (self.nodes.len() - 1) as u32
            }
        };
        self.parent_of[idx as usize] = parent.map_or(u32::MAX, |p| p.0);
        if let Some(p) = parent {
            self.children[p.0 as usize].push(idx);
        }
        NodeHandle(idx)
    }

    /// `h`와 그 아래 전부를 슬롯에서 놓아 준다 — DOM 노드가 GC 대상이 된다.
    /// 문서 트리에서 떼는 것은 호출부가 이미 했다고 가정한다.
    fn release_subtree(&mut self, h: u32) {
        // 재귀 대신 명시 스택 — 깊은 트리에서 wasm 스택을 넘기지 않도록.
        let mut stack = vec![h];
        while let Some(cur) = stack.pop() {
            let kids = std::mem::take(&mut self.children[cur as usize]);
            stack.extend_from_slice(&kids);
            self.nodes[cur as usize] = None;
            self.parent_of[cur as usize] = u32::MAX;
            self.pending_free.push(cur);
        }
    }

    fn detach_from_parent_table(&mut self, h: u32) {
        let p = self.parent_of[h as usize];
        if p != u32::MAX {
            self.children[p as usize].retain(|&c| c != h);
        }
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
        self.alloc_under(el.unchecked_into(), Some(parent))
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        let text_node = self.document.create_text_node(text);
        self.node(parent)
            .append_child(&text_node)
            .expect("append_child failed");
        // Never patched again, so no handle is kept for it.
    }

    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        // One native call parses `html` and inserts the resulting nodes —
        // this is the actual point of `append_static_html` (see that
        // trait method's doc comment): a whole precomputed static subtree,
        // however many nodes it contains, becomes exactly one DOM API call
        // instead of one call per node.
        let el: web_sys::Element = self.node(parent).clone().unchecked_into();
        el.insert_adjacent_html("beforeend", html)
            .expect("insert_adjacent_html failed");
    }

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        let node = self.node(target).clone();
        // 자식이 텍스트 노드 **하나뿐**이면 그 자리에서 `data`만 바꾼다.
        //
        // 왜 이게 필요한가 (2026-08-02, §11-1(b) 측정 방법 결함):
        // `Node.textContent = x`는 **자식을 전부 지우고 새 텍스트 노드를
        // 만든다**. MutationObserver 교차 검증(§7 M2)에서 드러났다 — C의
        // setText 999회가 `characterData` 0건, `childList` 999건(+999/-999)
        // 으로 관측됐다. §7 M2의 대응표는 "setText → characterData"라고
        // 명시하므로, 조항이 전제한 것은 **제자리 변경**이다. 즉 이전 구현은
        // 조항이 재려던 것과 다른 일을 하고 있었다(그리고 매 패치마다 텍스트
        // 노드를 파괴·재생성하고 있었다).
        //
        // 영향이 후보별로 다르다는 점을 숨기지 않는다: 이 경로를 가장 많이
        // 쓰는 것은 retained 슬롯에 값을 다시 쓰는 C이고, K처럼 매번 새
        // 엘리먼트를 만드는 쪽은 첫 자식이 없어 여기 걸리지 않는다. 따라서
        // 이 수리는 **C에 유리한 방향**이며, §9 판정을 새 데이터로 다시
        // 유도해야 한다(§2.5 전면 재수집 사유).
        if let Some(first) = node.first_child() {
            if first.node_type() == web_sys::Node::TEXT_NODE && first.next_sibling().is_none() {
                first.unchecked_ref::<web_sys::CharacterData>().set_data(text);
                return;
            }
        }
        node.set_text_content(Some(text));
    }

    fn clear_children(&mut self, parent: NodeHandle) {
        // `textContent = null` removes every child in one native call — the
        // browser-DOM realization of RQ1's "clear_children fast path".
        self.node(parent).set_text_content(None);
        // 문서에서 뗀 것만으로는 부족하다 — 슬롯이 `web_sys::Node`를 붙들고
        // 있으면 GC가 못 가져간다(위 `nodes` 주석 참고).
        let kids = std::mem::take(&mut self.children[parent.0 as usize]);
        for k in kids {
            self.release_subtree(k);
        }
    }

    fn remove_node(&mut self, node: NodeHandle) {
        let n = self.node(node).clone();
        if let Some(parent) = n.parent_node() {
            parent.remove_child(&n).expect("remove_child failed");
        }
        self.detach_from_parent_table(node.0);
        self.release_subtree(node.0);
    }

    fn begin_batch(&mut self) {
        // 한 배치 안에서 해제된 핸들은 그 배치가 끝난 뒤에야 재사용 가능해진다
        // (같은 배치의 뒤쪽 write가 anchor로 참조할 수 있으므로).
        self.free.append(&mut self.pending_free);
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        let el: web_sys::HtmlElement = self.node(node).clone().unchecked_into();
        el.style()
            .set_property("display", if visible { "" } else { "none" })
            .expect("set display style failed");
    }

    fn detach_to_fragment(&mut self, node: NodeHandle) -> FragmentHandle {
        let frag = self.document.create_document_fragment();
        // **anchor 자신이 아니라 그 자식들**을 옮긴다. anchor를 통째로 옮기면
        // 재부착 시 "자기 자신을 자기 안에 넣는" 계층 오류가 난다(실제로 그렇게
        // 짰다가 F-i 셀이 멈춰서 발견). 조건부가 감추려는 것은 어차피 anchor
        // 아래의 서브트리이므로 이쪽이 의미상으로도 맞다.
        let parent = self.node(node).clone();
        while let Some(child) = parent.first_child() {
            frag.append_child(&child)
                .expect("detach_to_fragment: append child to fragment failed");
        }
        let handle = FragmentHandle(self.fragments.len() as u32);
        self.fragments.push(frag);
        handle
    }

    fn attach_from_fragment(&mut self, parent: NodeHandle, fragment: FragmentHandle) {
        let frag = self.fragments[fragment.0 as usize].clone();
        self.node(parent)
            .append_child(&frag)
            .expect("attach_from_fragment: append fragment failed");
        // fragment는 append 시 비워진다(내용이 부모로 옮겨감) — 테이블에는
        // 빈 껍데기가 남지만, 핸들 안정성을 위해 제거하지 않는다.
    }

    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        let el: web_sys::Element = self.node(node).clone().unchecked_into();
        el.set_attribute(name, value).expect("set_attribute failed");
    }

    fn set_input_value(&mut self, node: NodeHandle, value: &str) {
        // Property set (`.value =`), not `set_attribute` — see this trait
        // method's doc comment for why an attribute write wouldn't be
        // reflected in what the user actually sees once they've typed.
        let el: web_sys::HtmlInputElement = self.node(node).clone().unchecked_into();
        el.set_value(value);
    }

    fn get_input_value(&mut self, node: NodeHandle) -> String {
        let el: web_sys::HtmlInputElement = self.node(node).clone().unchecked_into();
        el.value()
    }

    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        let parent_node = self.node(parent).clone();
        let children = parent_node.child_nodes();
        let mut out = Vec::with_capacity(children.length() as usize);
        for i in 0..children.length() {
            let Some(child) = children.item(i) else { continue };
            let text = child.text_content().unwrap_or_default();
            let handle = self.alloc_under(child, Some(parent));
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
            self.reposition_native += 1; // §7 M2 분해
            return; // state-preserving native move succeeded.
        }
        self.reposition_fallback += 1;
        // Fell through: engine has no `moveBefore`, or the call threw.
        // Fallback (RQ1 candidate E's documented Safari path): plain
        // `insertBefore` still relocates an *existing* node without
        // recreating it — it just doesn't carry `moveBefore`'s extra state
        // guarantees (e.g. avoiding an iframe/video reload).
        parent_node
            .insert_before(&node_ref, before_ref.as_ref())
            .expect("insert_before fallback failed");
    }

    fn reposition_split(&self) -> (u64, u64) {
        (self.reposition_native, self.reposition_fallback)
    }

    fn live_node_count(&self) -> Option<usize> {
        Some(self.nodes.iter().filter(|n| n.is_some()).count())
    }
}
