//! Shared mock DOM for native tests (no browser). Tracks enough state to
//! verify not just *content* (text) but *identity* (whether a DOM node was
//! reused/moved vs freshly created) — the whole point of testing candidates
//! C/D/E against K is proving they preserve node identity across a reorder
//! where K always respawns.

use loom_runtime::{DomOps, NodeHandle};

#[derive(Default)]
pub struct MockDom {
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
        NodeHandle(id)
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

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        self.text.insert(target.0, text.to_string());
    }

    fn clear_children(&mut self, parent: NodeHandle) {
        self.children.get_mut(&parent.0).unwrap().clear();
    }

    fn remove_node(&mut self, node: NodeHandle) {
        for siblings in self.children.values_mut() {
            siblings.retain(|&id| id != node.0);
        }
        self.removed.push(node);
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
