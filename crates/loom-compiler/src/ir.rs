//! docs/02 "정적 분석" 단계의 산출물 — 저작 표면(TSX)과 코드 생성(codegen.rs)
//! 사이의 중간 표현. Phase 1이 실제로 지원하는 저작 표면은 아래 지원 범위로
//! 의도적으로 좁혀져 있다(roadmap/PHASES.md Phase 1 상태 참고) — 일반 JS 표현식
//! 트랜스파일러가 아니라, 컴파일러가 "정적 마크업 vs 동적 binding site"를 실제로
//! 분류하는 것 자체를 증명하는 프로토타입.

#[derive(Debug, Clone, PartialEq)]
pub enum SignalInit {
    I32(i32),
    StringList(Vec<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalDecl {
    pub name: String,
    pub init: SignalInit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClickAction {
    /// `onClick={() => NAME.increment()}` — NAME must be an I32 signal.
    Increment { signal: String },
    /// `onClick={() => NAME.append("literal")}` — NAME must be a StringList
    /// signal.
    Append { signal: String, literal: String },
    /// `onClick={() => NAME.rotate()}` — moves the front item to the back.
    /// Deliberately not named "shuffle": it's a real, honestly-described
    /// reorder, not a fake random one (see loom-demo's source comment).
    Rotate { signal: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum JsxNode {
    StaticText(String),
    /// `{signal()}` — a top-level, component-scope signal read.
    TextBinding { signal: String },
    /// `{item}` inside a list row template — refers to the row's own value,
    /// never a named top-level signal (distinct IR node so codegen can't
    /// confuse the two scopes).
    RowItemText,
    /// `{signal().map((row_var) => <row/>)}`
    ListBinding {
        signal: String,
        row_var: String,
        row: Box<JsxNode>,
    },
    Element {
        tag: String,
        class: Option<String>,
        action: Option<ClickAction>,
        children: Vec<JsxNode>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub signals: Vec<SignalDecl>,
    pub root: JsxNode,
}
