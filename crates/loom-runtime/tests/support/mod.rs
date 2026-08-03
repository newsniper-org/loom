//! Shared mock DOM for native tests (no browser). Tracks enough state to
//! verify not just *content* (text) but *identity* (whether a DOM node was
//! reused/moved vs freshly created) — the whole point of testing candidates
//! C/D/E against K is proving they preserve node identity across a reorder
//! where K always respawns.

use loom_runtime::{DomOps, NodeHandle};

#[derive(Default)]
pub struct MockDom {
    /// 아직 살아 있는 핸들 — 실제 백엔드의 "슬롯이 노드를 붙들고 있는가"에
    /// 대응한다. `create_element`가 넣고 `remove_node`/`clear_children`이 뺀다.
    pub live: std::collections::HashSet<u32>,
    /// RQ1 트랙 T F 계열: 분리 보관 중인 노드들. 실제 DocumentFragment 대신
    /// "어느 노드가 문서 밖으로 나갔는가"만 기록한다 — 테스트가 확인해야 할
    /// 것은 그 사실뿐이다.
    pub detached: std::collections::HashMap<u32, NodeHandle>,
    /// 분리된 서브트리의 자식들 — 재부착 시 되돌린다.
    pub detached_children: std::collections::HashMap<u32, Vec<u32>>,
    /// detach/attach 호출 횟수 — 세 F 변형의 토글 비용 구조 비교용.
    pub detach_calls: usize,
    pub attach_calls: usize,
    /// `set_text_content` 호출 횟수 — F 변형이 분리 중에 실제로 DOM 쓰기를
    /// 내는지(F-ii) 막는지(F-i/iii) 구별하는 데 쓴다.
    pub set_text_calls: usize,
    next_id: u32,
    pub text: std::collections::HashMap<u32, String>,
    pub children: std::collections::HashMap<u32, Vec<u32>>,
    /// Every `create_element` call ever made, in order — lets tests assert
    /// exactly how many DOM nodes were created (K creates a fresh one per
    /// row per reconcile; C/D/E should not, for retained rows).
    pub created: Vec<NodeHandle>,
    pub removed: Vec<NodeHandle>,
    pub repositions: u32,
    pub visibility: std::collections::HashMap<u32, bool>,
    /// Count of `set_visible` calls, independent of which node — lets tests
    /// assert a toggle touched exactly one node regardless of how many
    /// descendants that node has (RQ1 Track T's subtree-size claim for F).
    pub visibility_writes: u32,
    pub attributes: std::collections::HashMap<u32, Vec<(String, String)>>,
    /// `<input>` two-way binding's live value, written by
    /// `patch_input_system` via `DomOps::set_input_value` — separate from
    /// `text` (which models text-node/`textContent` content, not an input's
    /// `value` property).
    pub input_values: std::collections::HashMap<u32, String>,
    /// Count of `set_input_value` calls, independent of which node — lets
    /// tests assert exactly how many reactive writes a patch tick produced
    /// (same pattern as `visibility_writes`).
    pub input_value_writes: u32,
    /// Raw HTML blobs passed to `append_static_html`, keyed by the
    /// synthetic child node id MockDom allocated for each one (not parsed
    /// into individual child nodes — this mock doesn't have an HTML parser,
    /// it only needs to prove *that one call carried the whole blob*, which
    /// is exactly what `append_static_html`'s contract is about). Separate
    /// from `text` so a test can tell "one `append_static_html` call" apart
    /// from "N `append_static_text` calls that happen to concatenate to the
    /// same string."
    pub raw_html: std::collections::HashMap<u32, String>,
    /// Count of `append_static_html` calls — lets tests assert a static
    /// subtree collapsed into exactly one call regardless of how many
    /// nodes it logically contains (same pattern as `visibility_writes`).
    pub static_html_writes: u32,
}

impl MockDom {
    pub fn new_root() -> (Self, NodeHandle) {
        let mut dom = MockDom::default();
        let root = dom.alloc();
        (dom, root)
    }

    fn alloc(&mut self) -> NodeHandle {
        let id = self.next_id;
        self.next_id += 1;
        self.children.insert(id, Vec::new());
        self.live.insert(id);
        NodeHandle(id)
    }

    /// `h`와 그 아래를 전부 놓아 준다.
    fn release_subtree(&mut self, h: u32) {
        let mut stack = vec![h];
        while let Some(cur) = stack.pop() {
            if let Some(kids) = self.children.get_mut(&cur) {
                let kids: Vec<u32> = kids.drain(..).collect();
                stack.extend_from_slice(&kids);
            }
            self.live.remove(&cur);
            self.children.remove(&cur);
            self.text.remove(&cur);
        }
    }

    pub fn child_texts(&self, parent: NodeHandle) -> Vec<String> {
        self.children[&parent.0]
            .iter()
            .map(|c| self.text.get(c).cloned().unwrap_or_default())
            .collect()
    }

    pub fn child_handles(&self, parent: NodeHandle) -> Vec<NodeHandle> {
        self.children[&parent.0].iter().map(|&c| NodeHandle(c)).collect()
    }

    /// `true` unless explicitly hidden — matches `display: ""` (visible) as
    /// the default for a freshly created element.
    pub fn is_visible(&self, node: NodeHandle) -> bool {
        *self.visibility.get(&node.0).unwrap_or(&true)
    }
}

impl DomOps for MockDom {
    fn create_element(&mut self, parent: NodeHandle, _tag: &str, _class: Option<&str>) -> NodeHandle {
        let handle = self.alloc();
        self.children.get_mut(&parent.0).unwrap().push(handle.0);
        self.created.push(handle);
        handle
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        let handle = self.alloc();
        self.children.get_mut(&parent.0).unwrap().push(handle.0);
        self.text.insert(handle.0, text.to_string());
    }

    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        let handle = self.alloc();
        self.children.get_mut(&parent.0).unwrap().push(handle.0);
        self.raw_html.insert(handle.0, html.to_string());
        self.static_html_writes += 1;
    }

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        self.text.insert(target.0, text.to_string());
        self.set_text_calls += 1;
    }

    fn clear_children(&mut self, parent: NodeHandle) {
        // 실제 백엔드(`web::BrowserDom`)와 **같은 계약**을 모형화한다: 문서에서
        // 떼는 것만으로는 부족하고 슬롯도 놓아 줘야 한다. 2026-08-03에
        // `BrowserDom`이 정확히 이 절반만 하고 있어서 n=10,000 조합에서 3,500만
        // 노드를 살려 뒀다 — 그 계약을 여기서도 검사할 수 있게 만든다.
        let kids: Vec<u32> = self.children.get_mut(&parent.0).unwrap().drain(..).collect();
        for k in kids {
            self.release_subtree(k);
        }
    }

    fn remove_node(&mut self, node: NodeHandle) {
        for siblings in self.children.values_mut() {
            siblings.retain(|&id| id != node.0);
        }
        self.removed.push(node);
        self.release_subtree(node.0);
    }

    fn live_node_count(&self) -> Option<usize> {
        Some(self.live.len())
    }

    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        self.repositions += 1;
        let siblings = self.children.get_mut(&parent.0).unwrap();
        siblings.retain(|&id| id != node.0);
        match before {
            Some(b) => {
                let pos = siblings
                    .iter()
                    .position(|&id| id == b.0)
                    .expect("reposition: `before` anchor not found among parent's children");
                siblings.insert(pos, node.0);
            }
            None => siblings.push(node.0),
        }
    }

    fn detach_to_fragment(&mut self, node: NodeHandle) -> loom_runtime::FragmentHandle {
        let h = loom_runtime::FragmentHandle(self.detach_calls as u32);
        // 실제 구현과 같은 의미론: anchor 자신이 아니라 그 자식들이 나간다.
        let kids = self.children.remove(&node.0).unwrap_or_default();
        self.detached.insert(h.0, node);
        self.detached_children.insert(h.0, kids);
        self.detach_calls += 1;
        h
    }

    fn attach_from_fragment(&mut self, parent: NodeHandle, fragment: loom_runtime::FragmentHandle) {
        self.detached.remove(&fragment.0);
        if let Some(kids) = self.detached_children.remove(&fragment.0) {
            self.children.entry(parent.0).or_default().extend(kids);
        }
        self.attach_calls += 1;
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        self.visibility.insert(node.0, visible);
        self.visibility_writes += 1;
    }

    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        self.attributes
            .entry(node.0)
            .or_default()
            .push((name.to_string(), value.to_string()));
    }

    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        self.children[&parent.0]
            .iter()
            .map(|&id| (NodeHandle(id), self.text.get(&id).cloned().unwrap_or_default()))
            .collect()
    }

    fn set_input_value(&mut self, node: NodeHandle, value: &str) {
        self.input_values.insert(node.0, value.to_string());
        self.input_value_writes += 1;
    }

    fn get_input_value(&mut self, node: NodeHandle) -> String {
        // Prefers a live value (set via `set_input_value`) over the
        // creation-time `value` *attribute* — mirrors the real distinction
        // `DomOps::set_input_value`'s doc comment describes (property vs
        // attribute), so a test seeding only the initial attribute (via
        // `set_attribute`, the same path codegen's `setup()` uses) still
        // gets a sensible read before any reactive write has happened.
        self.input_values.get(&node.0).cloned().unwrap_or_else(|| {
            self.attributes
                .get(&node.0)
                .and_then(|attrs| attrs.iter().rev().find(|(k, _)| k == "value").map(|(_, v)| v.clone()))
                .unwrap_or_default()
        })
    }
}
