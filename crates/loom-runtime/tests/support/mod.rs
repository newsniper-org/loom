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

    // ---------------------------------------------------------------------
    // §5 트랙 N 관측 축 (2026-08-07 신설)
    // ---------------------------------------------------------------------
    /// 노드 → 부모 역인덱스. 이게 없으면 **트리임이 보장되지 않는다** —
    /// `reposition`이 목표 부모의 자식 목록에서만 노드를 빼던 탓에, 부모를
    /// 건너뛴 이동이면 노드가 **양쪽 부모에 동시에** 남았다(감사 실측 확인).
    /// §5 불변식 2("move가 다른 row/블록 노드를 찢지 않음")는 그 상태를
    /// 표현할 수 없으면 검사할 수도 없다.
    pub parent_of: std::collections::HashMap<u32, u32>,
    /// **위반을 패닉 대신 기록한다.** property test는 위반을 모아야 shrinking이
    /// 되고 불변식 계열별로 분류할 수 있다 — 첫 패닉에서 멈추면 한 시퀀스가
    /// 위반 1건만 산출한다. `strict`가 켜져 있을 때만 채워진다.
    pub violations: Vec<DomViolation>,
    /// 켜면 위반 시 패닉하지 않고 `violations`에 적재 후 안전한 no-op으로 진행.
    /// 기본값은 꺼짐 — 기존 테스트 ~50개가 패닉을 기대하고 있으므로.
    pub strict: bool,
}

/// §5 불변식 계열. 위반을 어느 조항에 귀속시킬지 분류한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvariantFamily {
    /// 1 — 최종 DOM 순서·내용 = 참조 모델
    ReferenceModel,
    /// 2 — D의 DOM 구간(찢김, anchor 소유)
    DomSpan,
    /// 3 — E의 순서 수렴
    OrderConvergence,
    /// 4 — F의 catch-up(앵커 dangling 포함)
    CatchUp,
    /// 5 — 공통 flush(spawn/move 인터리빙)
    FlushOrdering,
    /// 트리 구조 자체가 깨짐(부모 둘, 사이클 등) — 어느 계열이든 선행 위반
    TreeStructure,
}

#[derive(Debug, Clone)]
pub struct DomViolation {
    pub family: InvariantFamily,
    pub op: String,
    pub detail: String,
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
            self.parent_of.remove(&cur);
        }
    }

    // ---------------------------------------------------------------------
    // §5 관측 축 — 트리 유지와 위반 기록
    // ---------------------------------------------------------------------

    /// 노드를 **실제 부모**에서 뗀다(양쪽 부모 잔존 방지의 핵심).
    fn detach_from_current_parent(&mut self, node: u32) {
        if let Some(p) = self.parent_of.remove(&node) {
            if let Some(sib) = self.children.get_mut(&p) {
                sib.retain(|&id| id != node);
            }
        }
    }

    fn link(&mut self, parent: u32, node: u32, index: usize) {
        self.detach_from_current_parent(node);
        let sib = self.children.entry(parent).or_default();
        let i = index.min(sib.len());
        sib.insert(i, node);
        self.parent_of.insert(node, parent);
    }

    fn violate(&mut self, family: InvariantFamily, op: &str, detail: String) -> bool {
        if self.strict {
            self.violations.push(DomViolation {
                family,
                op: op.to_string(),
                detail,
            });
            true // 호출부는 안전한 no-op으로 진행
        } else {
            false // 기존 동작(패닉) 유지
        }
    }

    /// **트리 구조 불변식**. property test가 매 op 뒤에 부르면 위반이 즉시
    /// 계열별로 잡힌다.
    pub fn assert_tree_wellformed(&mut self) {
        let mut bad: Vec<String> = Vec::new();
        for (p, kids) in &self.children {
            for c in kids {
                match self.parent_of.get(c) {
                    Some(pp) if pp == p => {}
                    Some(pp) => bad.push(format!(
                        "노드 {c}가 {p}의 자식 목록에 있는데 parent_of는 {pp}"
                    )),
                    None => bad.push(format!(
                        "노드 {c}가 {p}의 자식 목록에 있는데 parent_of 없음"
                    )),
                }
            }
        }
        for d in bad {
            self.violations.push(DomViolation {
                family: InvariantFamily::TreeStructure,
                op: "assert_tree_wellformed".into(),
                detail: d,
            });
        }
    }

    /// 이 노드가 root에서 도달 가능한가(= 문서 안인가). "앵커 dangling 없음"
    /// (§5 불변식 4)을 물을 술어.
    pub fn is_in_document(&self, root: NodeHandle, node: NodeHandle) -> bool {
        let mut cur = node.0;
        for _ in 0..10_000 {
            if cur == root.0 {
                return true;
            }
            match self.parent_of.get(&cur) {
                Some(&p) => cur = p,
                None => return false,
            }
        }
        false // 사이클 — 별도로 TreeStructure 위반이 잡힌다
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
        // `entry().or_default()` — detach된 앵커(자식 엔트리가 없는 상태)에
        // 자식을 만드는 것이 **패닉이 아니어야** §5 불변식 4 시나리오(동결 중
        // 내부 리스트 편집)를 끝까지 돌릴 수 있다. 이전 `unwrap()`은 그 시퀀스를
        // 전부 하네스 크래시로 끝냈다.
        let idx = self.children.entry(parent.0).or_default().len();
        self.link(parent.0, handle.0, idx);
        self.created.push(handle);
        handle
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        let handle = self.alloc();
        let idx = self.children.entry(parent.0).or_default().len();
        self.link(parent.0, handle.0, idx);
        self.text.insert(handle.0, text.to_string());
    }

    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        let handle = self.alloc();
        self.children.get_mut(&parent.0).unwrap().push(handle.0);
        self.raw_html.insert(handle.0, html.to_string());
        self.static_html_writes += 1;
    }

    /// `SetText`는 "이 엘리먼트의 **텍스트 바인딩** 갱신"이지 "자식을 전부
    /// 갈아치움"이 아니다 — 그래서 자식을 건드리지 않는다. 이 계약은
    /// `web::BrowserDom::set_text_content`와 동형이어야 한다(2026-08-07에
    /// 실브라우저 쪽이 파괴적 폴백을 갖고 있던 것을 수리하면서 맞췄다).
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
        self.detach_from_current_parent(node.0);
        self.removed.push(node);
        self.release_subtree(node.0);
    }

    fn live_node_count(&self) -> Option<usize> {
        Some(self.live.len())
    }

    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        self.repositions += 1;
        // **§5 불변식 2의 관측점.** 이전 판은 목표 부모의 자식 목록에서만
        // node를 뺐다 — 실제로 붙어 있던 부모에서는 빼지 않아 노드가 양쪽에
        // 동시에 남았고(감사 실측), 그래서 "찢는 이동"이 조용히 표현 가능했다.
        // 이제 `link`가 실제 부모에서 떼고 옮긴다.
        // **먼저 실제 부모에서 뗀다.** 떼기 전에 앵커 위치를 계산하면, 같은
        // 부모 안 이동에서 node가 앵커보다 앞에 있을 때 인덱스가 하나 밀린다.
        self.detach_from_current_parent(node.0);
        let idx = match before {
            Some(b) => {
                let sib = self.children.entry(parent.0).or_default();
                match sib.iter().position(|&id| id == b.0) {
                    Some(pos) => pos,
                    None => {
                        // anchor가 이 부모의 자식이 아니다 — §5 불변식 2의
                        // "anchor가 자기 소유 노드만 포위" 위반.
                        if self.violate(
                            InvariantFamily::DomSpan,
                            "reposition",
                            format!("anchor {} 가 부모 {} 의 자식이 아님 (node {})", b.0, parent.0, node.0),
                        ) {
                            return;
                        }
                        panic!("reposition: `before` anchor not found among parent's children");
                    }
                }
            }
            None => self.children.entry(parent.0).or_default().len(),
        };
        self.link(parent.0, node.0, idx);
    }

    fn detach_to_fragment(&mut self, node: NodeHandle) -> loom_runtime::FragmentHandle {
        let h = loom_runtime::FragmentHandle(self.detach_calls as u32);
        // 실제 구현과 같은 의미론: anchor 자신이 아니라 그 자식들이 나간다.
        // 자식 **엔트리 자체는 남긴다**(빈 Vec) — 지우면 detach 후 그 앵커에
        // 대한 모든 조작이 패닉해 §5 불변식 4 시나리오가 실행 불가였다.
        let kids: Vec<u32> = self.children.entry(node.0).or_default().drain(..).collect();
        for k in &kids {
            self.parent_of.remove(k); // 문서 밖 — 부모 없음
        }
        self.detached.insert(h.0, node);
        self.detached_children.insert(h.0, kids);
        self.detach_calls += 1;
        h
    }

    fn attach_from_fragment(&mut self, parent: NodeHandle, fragment: loom_runtime::FragmentHandle) {
        self.detached.remove(&fragment.0);
        if let Some(kids) = self.detached_children.remove(&fragment.0) {
            // **앞쪽에 되돌린다.** 끝에 붙이면 동결 중 앵커에 추가된 노드와
            // 순서가 역전된다([a,b] detach → c 추가 → 재부착이 [c,a,b])
            // — 실브라우저도 같은 결함이고 §5 불변식 4가 겨냥하는 지점이다.
            for (i, k) in kids.into_iter().enumerate() {
                self.link(parent.0, k, i);
            }
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
