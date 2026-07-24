//! docs/02 "정적 분석" 단계의 산출물 — 저작 표면(TSX)과 코드 생성(codegen.rs)
//! 사이의 중간 표현. Phase 1이 실제로 지원하는 저작 표면은 아래 지원 범위로
//! 의도적으로 좁혀져 있다(roadmap/PHASES.md Phase 1 상태 참고) — 일반 JS 표현식
//! 트랜스파일러가 아니라, 컴파일러가 "정적 마크업 vs 동적 binding site"를 실제로
//! 분류하는 것 자체를 증명하는 프로토타입.

#[derive(Debug, Clone, PartialEq)]
pub enum SignalInit {
    I32(i32),
    StringList(Vec<String>),
    /// `signal(true)`/`signal(false)` — drives a `{cond() && <X/>}` binding
    /// (compiles to RQ1 Track T candidate F, `cond_reconcile_f_system` —
    /// see codegen.rs's module doc comment).
    Bool(bool),
    /// `signal("...")` — a plain string, driving a two-way
    /// `<input value={s()} onInput={(e) => s.set(e.target.value)}/>`
    /// binding (`JsxNode::InputBinding`). Distinct from `StringList`, which
    /// backs `.map()` list bindings, not a single scalar value.
    Str(String),
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
    /// `onClick={() => NAME.toggle()}` — flips a `Bool` signal. The only
    /// action a `{cond() && <X/>}` binding's driving signal supports.
    Toggle { signal: String },
    /// Not triggered by `onClick` at all — synthesized by codegen (see
    /// `collect_actions`) for every `JsxNode::InputBinding`, to reuse the
    /// same `Action`/`apply_action`/`resume_action_by_symbol` machinery
    /// every other action already goes through, rather than a parallel
    /// dispatch mechanism. Unlike `Append`'s `literal` (baked at compile
    /// time), this variant's `String` is only known at the moment a real
    /// `input` DOM event fires — see codegen.rs's `setup()`/`resume()` doc
    /// comments for how that runtime value actually reaches `apply_action`.
    SetString { signal: String },
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
    /// `{cond() && <tag>...</tag>}` — compiles to RQ1 Track T candidate F
    /// (`cond_reconcile_f_system`, already implemented/tested/benchmarked —
    /// see codegen.rs's module doc comment for why this reuses it rather
    /// than deriving a new mechanism). `cond` must name a declared `Bool`
    /// signal. Phase 1 scope (stated, not hidden): `then_branch` must be a
    /// single root `Element` whose children are all `StaticText` — no
    /// nested `{...}` bindings inside a conditional yet (see lower.rs's
    /// parsing rule for this restriction's enforcement).
    CondBinding {
        cond: String,
        then_branch: Box<JsxNode>,
    },
    /// `<input value={s()} onInput={(e) => s.set(e.target.value)}/>` — a
    /// two-way text binding. `s` must be a declared `Str` signal, and
    /// `value`/`onInput` must reference the *same* signal (lower.rs cross-
    /// checks this). Phase 1 scope: exactly this shape, no other `<input>`
    /// attributes (no `class`, no static `<input>` without the pair).
    InputBinding { signal: String },
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
