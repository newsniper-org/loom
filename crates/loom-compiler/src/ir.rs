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
    /// `<QuantityPicker client:load initial={5} />` — a JSX element
    /// referencing *another authored component*, as opposed to `Element`
    /// (an HTML tag). Its own variant, not a repurposed `Element{tag}`, for
    /// the same reason `InputBinding`/`CondBinding` each got their own
    /// variant: it needs slots `Element` has no room for (which *component*
    /// this names — resolved against `ComponentGraph::components` by name,
    /// not treated as an HTML string; an `IslandActivation`; a `props`
    /// list), and it participates in codegen completely differently
    /// (contributes zero entries to its *referrer's* `Signals`/`Action` —
    /// see codegen.rs's `collect_actions`/`collect_binding_shapes`).
    ///
    /// Phase 3 증분 4 scope (stated, not hidden): `activation` is always
    /// present — a bare `<Component/>` reference with no `client:` directive
    /// (ordinary nested composition sharing the parent's own World, no
    /// Island boundary) is rejected by lower.rs with a clear "not yet
    /// supported" panic; only `GraphMode::Flatten` codegen (Task #26's
    /// scope) is designed to eventually accept that shape.
    ComponentRef {
        component: String,
        activation: IslandActivation,
        props: Vec<(String, PropLiteral)>,
    },
}

/// `client:load` / `client:visible` — Astro's client-directive *concept*
/// reused (docs/08 "그대로 재사용한 것"), reinterpreted per docs/06 as "지연
/// actor spawn 트리거 시점" rather than hydration timing. Phase 3 증분 4's
/// only two supported directives — matching `demo/resume/registry.js`'s own
/// `bootstrapIslands()`, which already only understands these two values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandActivation {
    Load,
    Visible,
}

impl IslandActivation {
    /// The exact string `data-loom-activation` must carry — registry.js's
    /// `bootstrapIslands()` matches on these two literal values.
    pub fn as_attr_value(self) -> &'static str {
        match self {
            IslandActivation::Load => "load",
            IslandActivation::Visible => "visible",
        }
    }
}

/// A literal prop value passed at a `ComponentRef` call site — deliberately
/// restricted to the same literal shapes `SignalInit` already allows (Phase
/// 1's "no non-literal initial values anywhere" restriction, *extended*
/// rather than broken — see codegen.rs's `format_i32_capture_for_action` doc
/// comment for why this restriction is load-bearing for LRef capture
/// safety).
#[derive(Debug, Clone, PartialEq)]
pub enum PropLiteral {
    I32(i32),
    StringList(Vec<String>),
    Bool(bool),
    Str(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub signals: Vec<SignalDecl>,
    pub root: JsxNode,
}

impl Component {
    /// One entry per action-bearing binding site's target signal name — one
    /// per `onClick`-tagged `Element` plus one per `InputBinding` (which
    /// synthesizes a `SetString` action the same way `codegen.rs`'s
    /// `collect_actions` already counts it). Used by lower.rs's "signal당
    /// 액션 1개" static check (Task #28-f) — kept here in ir.rs, the data
    /// layer both lower.rs and codegen.rs already depend on.
    pub fn action_signal_names(&self) -> Vec<String> {
        fn walk(node: &JsxNode, out: &mut Vec<String>) {
            match node {
                JsxNode::Element { action, children, .. } => {
                    if let Some(a) = action {
                        out.push(a.signal_name().to_string());
                    }
                    for c in children {
                        walk(c, out);
                    }
                }
                JsxNode::InputBinding { signal } => out.push(signal.clone()),
                JsxNode::ListBinding { row, .. } => walk(row, out),
                JsxNode::CondBinding { then_branch, .. } => walk(then_branch, out),
                JsxNode::ComponentRef { .. } => {} // a different component's own action sites, not this one's.
                JsxNode::StaticText(_) | JsxNode::TextBinding { .. } | JsxNode::RowItemText => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }
}

impl ClickAction {
    /// The one signal every variant names (see this type's own doc comment).
    pub fn signal_name(&self) -> &str {
        match self {
            ClickAction::Increment { signal }
            | ClickAction::Rotate { signal }
            | ClickAction::Append { signal, .. }
            | ClickAction::Toggle { signal }
            | ClickAction::SetString { signal } => signal,
        }
    }
}

/// The output of lowering an entire module — replaces the Phase 1 bug where
/// only the first exported function was kept and every other declaration
/// was silently discarded with no diagnostic (see lower.rs's
/// `collect_component_fn_decls`). `components` holds one entry per authored
/// component *definition* found in the module.
#[derive(Debug, Clone, PartialEq)]
pub struct ComponentGraph {
    pub components: Vec<Component>,
    /// Name of the module's single designated entry point — the one
    /// `export`ed `function` (Phase 1's original, unrelaxed restriction;
    /// see lower.rs's module doc comment for why 증분 4 keeps this rule
    /// rather than allowing multiple independent exported roots per
    /// module).
    pub root: String,
}

impl ComponentGraph {
    pub fn root_component(&self) -> &Component {
        self.components
            .iter()
            .find(|c| c.name == self.root)
            .unwrap_or_else(|| {
                panic!(
                    "loomc internal error: ComponentGraph::root {:?} names no component in `components`",
                    self.root
                )
            })
    }

    /// Finds the `IslandActivation` the `ComponentRef` naming `name` carries,
    /// by scanning every component's tree. `None` if no `ComponentRef`
    /// anywhere names this component (e.g. it's the graph's own root, or an
    /// unreferenced helper function — see `lower_module`'s doc comment for
    /// why unreferenced helpers are accepted rather than lint-rejected).
    /// Phase 3 증분 4 scope: a component definition is only ever referenced
    /// from at most one `ComponentRef` site, so the first match found is the
    /// only one that matters — this is not re-validated here (a stricter
    /// check belongs to `specialize_component_refs`, not this lookup).
    pub fn activation_for(&self, name: &str) -> Option<IslandActivation> {
        fn walk(node: &JsxNode, name: &str) -> Option<IslandActivation> {
            match node {
                JsxNode::ComponentRef { component, activation, .. } if component == name => Some(*activation),
                JsxNode::Element { children, .. } => children.iter().find_map(|c| walk(c, name)),
                JsxNode::ListBinding { row, .. } => walk(row, name),
                JsxNode::CondBinding { then_branch, .. } => walk(then_branch, name),
                _ => None,
            }
        }
        self.components.iter().find_map(|c| walk(&c.root, name))
    }
}
