//! 컴파일러가 낸 **중첩 조건부** 코드를 브라우저 없이 실행해 보는 픽스처.
//!
//! `loom-compiler`의 단위 테스트는 생성된 *문자열*만 본다 — 그것만으로는
//! "이 코드가 rustc를 통과하는가"도, "돌렸을 때 옳은 DOM이 나오는가"도 알 수
//! 없다. 이 크레이트는 build.rs로 실제 산출물을 만들어 컴파일하고
//! (`tests/nested.rs`가) `TreeDom` 위에서 tick을 돌려 결과 트리를 직접
//! 비교한다.
//!
//! `TreeDom`은 `loom-runtime`의 테스트용 `MockDom`과 목적이 다르다: 후자는
//! 트랙 L 후보들의 불변식 위반을 잡는 도구고, 이쪽은 **최종 트리 모양을 사람이
//! 읽을 수 있는 HTML 한 줄로** 뽑아 비교하는 것이 전부다. 트랙 N 하네스의
//! RecordingDom이 생기면 그쪽으로 흡수될 수 있다.

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

/// `demo/nested-list.tsx` — RQ1 §5 트랙 N의 3중 중첩(리스트 > 조건부 > 리스트).
pub mod generated_list {
    include!(concat!(env!("OUT_DIR"), "/generated_list.rs"));
}

use loom_runtime::{DomOps, FragmentHandle, NodeHandle};

#[derive(Debug, Clone)]
enum Node {
    Element {
        tag: String,
        class: Option<String>,
        attrs: Vec<(String, String)>,
        children: Vec<Child>,
        visible: bool,
    },
    /// `create_element` 없이 붙은 텍스트/HTML 조각 — 핸들이 없다.
    Raw(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Child {
    Node(u32),
    /// `Node::Raw`를 가리키는 인덱스.
    Raw(u32),
}

/// 네이티브 DOM 대역. 실제 브라우저의 의미 중 이 픽스처가 의존하는 것만 —
/// 트리 구조, 텍스트, class/속성, 표시 여부 — 을 갖는다.
#[derive(Default)]
pub struct TreeDom {
    nodes: Vec<Node>,
    /// `create_element`가 만든 노드의 인덱스만 담는다(핸들 → 인덱스).
    handles: Vec<u32>,
    parent_of: std::collections::HashMap<u32, u32>,
}

impl TreeDom {
    /// 루트 하나를 가진 빈 DOM과 그 핸들.
    pub fn new() -> (Self, NodeHandle) {
        let mut dom = TreeDom::default();
        let root = dom.push_element("root".to_string(), None);
        (dom, NodeHandle(root))
    }

    fn push_element(&mut self, tag: String, class: Option<String>) -> u32 {
        let idx = self.nodes.len() as u32;
        self.nodes.push(Node::Element {
            tag,
            class,
            attrs: Vec::new(),
            children: Vec::new(),
            visible: true,
        });
        let handle = self.handles.len() as u32;
        self.handles.push(idx);
        handle
    }

    fn idx(&self, handle: NodeHandle) -> u32 {
        self.handles[handle.0 as usize]
    }

    fn children_mut(&mut self, idx: u32) -> &mut Vec<Child> {
        match &mut self.nodes[idx as usize] {
            Node::Element { children, .. } => children,
            Node::Raw(_) => panic!("TreeDom: 텍스트 조각에는 자식을 붙일 수 없다"),
        }
    }

    fn attach(&mut self, parent: u32, child: Child) {
        self.children_mut(parent).push(child);
        if let Child::Node(c) = child {
            self.parent_of.insert(c, parent);
        }
    }

    fn detach(&mut self, node: u32) {
        if let Some(parent) = self.parent_of.remove(&node) {
            self.children_mut(parent).retain(|c| *c != Child::Node(node));
        }
    }

    /// 지금 DOM에 실제로 존재하는 `data-loom-id` → 핸들 표.
    /// 브라우저 글루의 `document.querySelector("[data-loom-id=...]")`에
    /// 해당하며, `resume()`이 넘겨받는 `find_node`가 이 표를 읽는다.
    /// **없는 id는 표에도 없다** — 숨겨진 조건부 안의 노드를 찾으려 드는
    /// 결함이 조용히 통과하지 못하게 하는 것이 요점이다.
    pub fn loom_id_table(&self) -> std::collections::HashMap<String, NodeHandle> {
        let mut table = std::collections::HashMap::new();
        for (handle, idx) in self.handles.iter().enumerate() {
            if let Node::Element { attrs, .. } = &self.nodes[*idx as usize] {
                for (k, v) in attrs {
                    if k == "data-loom-id" {
                        table.insert(v.clone(), NodeHandle(handle as u32));
                    }
                }
            }
        }
        table
    }

    /// 노드 하위의 텍스트 조각만 이어 붙인다(엘리먼트 마크업 제외) —
    /// 브라우저 `textContent`의 의미. `adopt_children`이 row 값을 이걸로
    /// 되읽는다.
    fn text_content(&self, idx: u32) -> String {
        match &self.nodes[idx as usize] {
            Node::Raw(text) => text.clone(),
            Node::Element { children, .. } => children
                .iter()
                .map(|c| match c {
                    Child::Node(n) | Child::Raw(n) => self.text_content(*n),
                })
                .collect(),
        }
    }

    /// 트리를 HTML 한 줄로. 테스트가 읽는 유일한 출력이다.
    pub fn html(&self, handle: NodeHandle) -> String {
        let mut out = String::new();
        self.render(self.idx(handle), &mut out);
        out
    }

    fn render(&self, idx: u32, out: &mut String) {
        match &self.nodes[idx as usize] {
            Node::Raw(text) => out.push_str(text),
            Node::Element {
                tag,
                class,
                attrs,
                children,
                visible,
            } => {
                out.push('<');
                out.push_str(tag);
                if let Some(c) = class {
                    out.push_str(&format!(" class=\"{c}\""));
                }
                for (k, v) in attrs {
                    out.push_str(&format!(" {k}=\"{v}\""));
                }
                if !visible {
                    out.push_str(" hidden");
                }
                out.push('>');
                for child in children {
                    match child {
                        Child::Node(c) | Child::Raw(c) => self.render(*c, out),
                    }
                }
                out.push_str(&format!("</{tag}>"));
            }
        }
    }
}

impl DomOps for TreeDom {
    fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
        let parent_idx = self.idx(parent);
        let handle = self.push_element(tag.to_string(), class.map(str::to_string));
        let child_idx = self.handles[handle as usize];
        self.attach(parent_idx, Child::Node(child_idx));
        NodeHandle(handle)
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        let parent_idx = self.idx(parent);
        let raw = self.nodes.len() as u32;
        self.nodes.push(Node::Raw(text.to_string()));
        self.attach(parent_idx, Child::Raw(raw));
    }

    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        // 이 대역은 HTML을 파싱하지 않는다 — 문자열 그대로 트리에 남겨 둔다.
        // 컴파일타임에 이미 완성된 조각이라 테스트가 비교할 대상은 그 문자열
        // 자체다(브라우저에서는 `insertAdjacentHTML`이 파싱한다).
        self.append_static_text(parent, html);
    }

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        let idx = self.idx(target);
        // 브라우저의 `textContent =`처럼 자식을 전부 갈아치우는 대신, 첫
        // 텍스트 조각만 갈아 끼운다 — `BrowserDom::set_text_content`가
        // 2026-08-07에 정확히 이 의미로 고쳐졌다(R2 row의 binding span들을
        // 파괴하던 버그).
        let children = match &self.nodes[idx as usize] {
            Node::Element { children, .. } => children.clone(),
            Node::Raw(_) => panic!("TreeDom: 텍스트 조각에는 set_text_content를 쓸 수 없다"),
        };
        if let Some(Child::Raw(raw)) = children.iter().find(|c| matches!(c, Child::Raw(_))) {
            self.nodes[*raw as usize] = Node::Raw(text.to_string());
            return;
        }
        let raw = self.nodes.len() as u32;
        self.nodes.push(Node::Raw(text.to_string()));
        self.children_mut(idx).insert(0, Child::Raw(raw));
    }

    fn clear_children(&mut self, parent: NodeHandle) {
        let idx = self.idx(parent);
        let children = std::mem::take(self.children_mut(idx));
        for child in children {
            if let Child::Node(c) = child {
                self.parent_of.remove(&c);
            }
        }
    }

    fn remove_node(&mut self, node: NodeHandle) {
        let idx = self.idx(node);
        self.detach(idx);
    }

    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        let parent_idx = self.idx(parent);
        let node_idx = self.idx(node);
        self.detach(node_idx);
        let anchor = before.map(|b| self.idx(b));
        let pos = anchor
            .and_then(|a| self.children_mut(parent_idx).iter().position(|c| *c == Child::Node(a)))
            .unwrap_or_else(|| self.children_mut(parent_idx).len());
        self.children_mut(parent_idx).insert(pos, Child::Node(node_idx));
        self.parent_of.insert(node_idx, parent_idx);
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        let idx = self.idx(node);
        if let Node::Element { visible: v, .. } = &mut self.nodes[idx as usize] {
            *v = visible;
        }
    }

    fn detach_to_fragment(&mut self, _node: NodeHandle) -> FragmentHandle {
        unimplemented!("이 픽스처는 F 계열 토글을 쓰지 않는다")
    }

    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        let idx = self.idx(node);
        if let Node::Element { attrs, .. } = &mut self.nodes[idx as usize] {
            if let Some(slot) = attrs.iter_mut().find(|(k, _)| k == name) {
                slot.1 = value.to_string();
            } else {
                attrs.push((name.to_string(), value.to_string()));
            }
        }
    }

    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        let idx = self.idx(parent);
        let children = match &self.nodes[idx as usize] {
            Node::Element { children, .. } => children.clone(),
            Node::Raw(_) => Vec::new(),
        };
        children
            .iter()
            .filter_map(|c| match c {
                Child::Node(n) => {
                    let handle = self
                        .handles
                        .iter()
                        .position(|h| h == n)
                        .expect("TreeDom: adopt 대상이 create_element로 만들어지지 않았다");
                    Some((NodeHandle(handle as u32), self.text_content(*n)))
                }
                Child::Raw(_) => None,
            })
            .collect()
    }

    fn set_input_value(&mut self, node: NodeHandle, value: &str) {
        self.set_attribute(node, "value", value);
    }

    fn get_input_value(&mut self, node: NodeHandle) -> String {
        let idx = self.idx(node);
        match &self.nodes[idx as usize] {
            Node::Element { attrs, .. } => attrs
                .iter()
                .find(|(k, _)| k == "value")
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
            Node::Raw(_) => String::new(),
        }
    }
}
