//! 트랙 N용 DOM 대역 — 트리를 유지하면서 **불변식 위반을 그 자리에서** 기록한다.
//!
//! `loom-nested-fixture`의 `TreeDom`과 목적이 다르다: 저쪽은 최종 트리를 HTML로
//! 뽑아 사람이 읽을 수 있게 하는 것이 전부이고, 이쪽은 §5 불변식 2("move가 다른
//! row/블록 노드를 찢지 않음, anchor가 자기 소유 노드만 포위")를 **연산 시점에**
//! 검사한다. 최종 상태만 봐서는 잡을 수 없는 종류다 — 잘못된 앵커로 옮겼다가
//! 우연히 같은 자리에 오는 경우가 있기 때문이다.

use loom_runtime::{DomOps, FragmentHandle, NodeHandle};

#[derive(Debug, Clone)]
pub enum Node {
    Element {
        tag: String,
        class: Option<String>,
        attrs: Vec<(String, String)>,
        children: Vec<Child>,
        visible: bool,
    },
    Raw(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Child {
    Node(u32),
    Raw(u32),
}

/// §5의 불변식 계열. 위반은 계열별로 귀속돼야 한다 — "어느 불변식이 3사이클
/// 안에 재발했는가"가 후보의 귀결을 정하기 때문(§5 구조적 재발 규칙).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// 1. 최종 DOM 순서·내용 = 참조 모델
    ReferenceModel,
    /// 2. DOM 구간 불변식 (D)
    DomSpan,
    /// 3. 순서 수렴 (E)
    OrderConvergence,
    /// 4. 숨겨진 동안의 변경 반영 (F의 catch-up 자리 — 아래 주석 참고)
    CatchUp,
    /// 5. flush 규약 하 spawn/move 인터리빙의 결정적 수렴
    FlushOrdering,
}

#[derive(Debug, Clone)]
pub struct Violation {
    pub family: Family,
    pub detail: String,
}

#[derive(Default)]
pub struct RecordingDom {
    nodes: Vec<Node>,
    handles: Vec<u32>,
    parent_of: std::collections::HashMap<u32, u32>,
    pub violations: Vec<Violation>,
    /// 이 실행에서 방출된 DOM 연산의 요약. 같은 시드로 두 번 돌렸을 때 동일해야
    /// 한다(§5 불변식 5의 결정성 쪽).
    pub op_log: Vec<String>,
}

impl RecordingDom {
    pub fn new() -> (Self, NodeHandle) {
        let mut dom = RecordingDom::default();
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

    fn idx(&self, h: NodeHandle) -> u32 {
        self.handles[h.0 as usize]
    }

    fn children_mut(&mut self, idx: u32) -> &mut Vec<Child> {
        match &mut self.nodes[idx as usize] {
            Node::Element { children, .. } => children,
            Node::Raw(_) => panic!("RecordingDom: 텍스트 조각에는 자식이 없다"),
        }
    }

    fn children(&self, idx: u32) -> &[Child] {
        match &self.nodes[idx as usize] {
            Node::Element { children, .. } => children,
            Node::Raw(_) => &[],
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

    fn violate(&mut self, family: Family, detail: String) {
        self.violations.push(Violation { family, detail });
    }

    /// 서브트리의 노드 인덱스 전부(자기 포함). 구간 보존 검사에 쓴다.
    fn subtree(&self, idx: u32, out: &mut Vec<u32>) {
        out.push(idx);
        for c in self.children(idx).to_vec() {
            if let Child::Node(n) = c {
                self.subtree(n, out);
            }
        }
    }

    /// `node`가 `parent`의 몇 번째 **엘리먼트** 자식인가. 불변식 3이 DOM
    /// 순서를 읽는 유일한 통로다.
    pub fn child_index(&self, parent: NodeHandle, node: NodeHandle) -> Option<usize> {
        let (p, n) = (self.idx(parent), self.idx(node));
        self.children(p)
            .iter()
            .filter_map(|c| match c {
                Child::Node(x) => Some(*x),
                Child::Raw(_) => None,
            })
            .position(|x| x == n)
    }

    pub fn html(&self, h: NodeHandle) -> String {
        let mut out = String::new();
        self.render(self.idx(h), &mut out);
        out
    }

    fn render(&self, idx: u32, out: &mut String) {
        match &self.nodes[idx as usize] {
            Node::Raw(t) => out.push_str(t),
            Node::Element { tag, class, attrs, children, visible } => {
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
                for c in children {
                    match c {
                        Child::Node(n) | Child::Raw(n) => self.render(*n, out),
                    }
                }
                out.push_str(&format!("</{tag}>"));
            }
        }
    }

    fn text_content(&self, idx: u32) -> String {
        match &self.nodes[idx as usize] {
            Node::Raw(t) => t.clone(),
            Node::Element { children, .. } => children
                .iter()
                .map(|c| match c {
                    Child::Node(n) | Child::Raw(n) => self.text_content(*n),
                })
                .collect(),
        }
    }
}

impl DomOps for RecordingDom {
    fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
        let p = self.idx(parent);
        let h = self.push_element(tag.to_string(), class.map(str::to_string));
        let c = self.handles[h as usize];
        self.attach(p, Child::Node(c));
        self.op_log.push(format!("create({tag})"));
        NodeHandle(h)
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        let p = self.idx(parent);
        let raw = self.nodes.len() as u32;
        self.nodes.push(Node::Raw(text.to_string()));
        self.attach(p, Child::Raw(raw));
        self.op_log.push(format!("text({text})"));
    }

    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        self.append_static_text(parent, html);
    }

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        let idx = self.idx(target);
        self.op_log.push(format!("setText({text})"));
        let children = self.children(idx).to_vec();
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
        self.op_log.push("clear".to_string());
        let children = std::mem::take(self.children_mut(idx));
        for c in children {
            if let Child::Node(n) = c {
                self.parent_of.remove(&n);
            }
        }
    }

    fn remove_node(&mut self, node: NodeHandle) {
        let idx = self.idx(node);
        self.op_log.push("remove".to_string());
        self.detach(idx);
    }

    /// **§5 불변식 2의 검사 지점.** 재배치는 (a) 앵커가 목적지 부모의 **직계
    /// 자식**이어야 하고(자기 소유 밖의 노드를 앵커로 잡으면 다른 row의 구간
    /// 안으로 파고든다), (b) 옮기는 노드의 **서브트리 전체**가 따라가야 하며,
    /// (c) 그 밖의 어떤 노드도 부모가 바뀌면 안 된다(다른 row를 찢지 않음).
    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        let p = self.idx(parent);
        let n = self.idx(node);
        self.op_log.push("move".to_string());

        // (a) 앵커는 목적지 부모의 직계 자식인가.
        if let Some(b) = before {
            let b = self.idx(b);
            if !self.children(p).contains(&Child::Node(b)) {
                let owner = self.parent_of.get(&b).copied();
                self.violate(
                    Family::DomSpan,
                    format!(
                        "앵커 node#{b}가 목적지 부모 node#{p}의 직계 자식이 아니다 \
                         (실제 부모 node#{owner:?}) — 다른 구간 안을 가리키는 앵커"
                    ),
                );
            }
        }

        // (b)/(c) 이동 전후의 소유 관계 스냅샷.
        let mut moved = Vec::new();
        self.subtree(n, &mut moved);
        let before_owner: std::collections::HashMap<u32, u32> = self.parent_of.clone();

        self.detach(n);
        let anchor = before.map(|b| self.idx(b));
        let pos = anchor
            .and_then(|a| self.children(p).iter().position(|c| *c == Child::Node(a)))
            .unwrap_or_else(|| self.children(p).len());
        self.children_mut(p).insert(pos, Child::Node(n));
        self.parent_of.insert(n, p);

        let mut after_moved = Vec::new();
        self.subtree(n, &mut after_moved);
        if moved != after_moved {
            self.violate(
                Family::DomSpan,
                format!("이동이 서브트리를 온전히 옮기지 않았다: {moved:?} → {after_moved:?}"),
            );
        }
        for (child, old_parent) in &before_owner {
            if *child == n {
                continue;
            }
            match self.parent_of.get(child) {
                Some(now) if now == old_parent => {}
                other => self.violate(
                    Family::DomSpan,
                    format!("이동이 무관한 node#{child}의 부모를 바꿨다: {old_parent} → {other:?}"),
                ),
            }
        }
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        let idx = self.idx(node);
        self.op_log.push(format!("visible({visible})"));
        if let Node::Element { visible: v, .. } = &mut self.nodes[idx as usize] {
            *v = visible;
        }
    }

    fn detach_to_fragment(&mut self, _node: NodeHandle) -> FragmentHandle {
        unimplemented!("컴파일러 산출물은 fragment 분리를 쓰지 않는다 — 아래 CatchUp 주석 참고")
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
        self.children(idx)
            .to_vec()
            .iter()
            .filter_map(|c| match c {
                Child::Node(n) => {
                    let h = self.handles.iter().position(|x| x == n)?;
                    Some((NodeHandle(h as u32), self.text_content(*n)))
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
            Node::Element { attrs, .. } => {
                attrs.iter().find(|(k, _)| k == "value").map(|(_, v)| v.clone()).unwrap_or_default()
            }
            Node::Raw(_) => String::new(),
        }
    }
}
