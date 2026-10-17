//! TSX (real SWC AST, parsed by `swc_ecma_parser` — no hand-rolled parser,
//! per docs/02 "왜 직접 파서를 안 짜는가") → `ir::ComponentGraph`.
//!
//! Supported subset (Phase 1/증분 4 prototype scope, documented here rather
//! than silently assumed):
//!   - exactly one *exported* `function NAME() { ... }` per module (the
//!     graph's root) — Phase 1's original restriction, kept (see
//!     `collect_component_fn_decls`'s doc comment for why 증분 4 doesn't
//!     relax this rule itself, only what surrounds it)
//!   - any number of additional, non-exported top-level `function NAME() {
//!     ... }` declarations — helper components, only meaningful if some
//!     `<Name client:load|client:visible .../>` reference (transitively
//!     reachable from the root) names them (증분 4, Task #25)
//!   - `const NAME = signal(NUMBER)` or `const NAME = signal([STRING, ...])`
//!   - a single JSX return, containing:
//!     - static tags (`div`/`span`/`ul`/`li`/`button`) with an optional
//!       static `class` attribute
//!     - static text children
//!     - `{signal()}` text bindings
//!     - `{signal().map((row) => <tag>{row}</tag>)}` list bindings. **2026-08-07
//!       이후** row 템플릿은 `{item}`으로 시작하기만 하면 그 뒤로 정적 마크업과
//!       **조건부 하나**(그 안에 정적 마크업 + 중첩 리스트 하나)를 가질 수 있다
//!       — RQ1 §5 트랙 N의 3중 중첩이 이것으로 표현된다(codegen.rs 모듈 문서의
//!       "row 템플릿의 사이트별 monomorphize" 절)
//!     - `{cond() && <tag>...</tag>}` 조건부. **2026-08-07 이후** 그 안에 정적
//!       마크업 / `{signal()}` / `{signal().map(...)}` / 그것들을 감싸는
//!       엘리먼트를 넣을 수 있다(그 전에는 정적 텍스트만)
//!     - **row 스코프 파생** (row 템플릿 안에서만):
//!       `{open() && item.length > n && <X/>}` 와
//!       `{xs().slice(0, item.length).map(...)}` — 둘 다 닫힌 형태이고,
//!       계산은 생성 코드에 인라인된다(`ir::RowPredicate`/`ir::RowSlice`)
//!     - `onClick={() => signal.increment()|.append("lit")|.rotate()}`
//!     - `<ComponentName client:load|client:visible prop={literal} .../>` —
//!       an uppercase-leading tag name is *always* classified as a
//!       component reference, never an HTML tag (JSX's own universal
//!       lexical convention — the same rule every framework in this
//!       ecosystem already uses, not something specific to one of them;
//!       reused rather than reinvented per CLAUDE.md 2-1). 증분 4 scope:
//!       the directive is *required* — a bare `<Name/>` with no `client:`
//!       directive (ordinary non-Island nested composition) is rejected
//!       with a clear "not yet supported" error (see `lower_component_ref`).
//! Anything outside this is a compile error — this is a real, narrow
//! compiler front end, not a fuzzy best-effort one.
//!
//! **Phase 7a C2 (2026-10-17)**: 거부는 더 이상 `panic!`이 아니라
//! `Err(Diagnostic)`이다(문제 구문의 AST span을 단다). 7a 이전부터 있던 거부는
//! 모두 코드 `LOOM-L00`(지원 범위 밖 저작 표면), single-writer 검사는
//! `LOOM-L01`이다 — 7a가 새로 더하는 거부(`LOOM-Rnn`, design-7a §1.1)와
//! 구별하려는 것이다. 문구는 기존 테스트의 부분 문자열이 그대로 맞도록
//! 유지했다(앞의 `loomc: ` 접두와 끝의 AST `Debug` 덤프만 뺐다 — 진단 헤더가
//! 코드를, span 강조가 해당 소스를 이미 보여 준다). 진짜 내부 불변식인
//! `unreachable!` 2곳(매치 가드가 이미 보장한 모양을 다시 벗기는 곳)만 남겼다.
//! 성공 경로의 IR은 C1과 같다 — 산출물 바이트는 게이트 A가 확인한다.
//!
//! 함께 `SpanTable`을 돌려준다. IR(`ir.rs`)에 span을 넣지 않는 이유는 IR의
//! `derive(PartialEq)`와 codegen의 구조체 생성을 건드리지 않기 위해서다
//! (design-7a §3.3). 그래프 검사(single-writer, 이후 커밋의 R07/R44 등)는 이
//! 표에서 사이트 위치를 찾는다.

use crate::diag::Diagnostic;
use crate::ir::{
    ClickAction, Component, ComponentGraph, IslandActivation, JsxNode, PropLiteral, RowPredicate,
    RowSlice, SharedBinding, SignalDecl, SignalInit,
};
use std::collections::HashSet;
use swc_common::{Span, Spanned};
use swc_ecma_ast::*;

/// 7a 이전부터 있던 "지원 범위 밖" 거부의 공용 코드.
pub(crate) const L00: &str = "LOOM-L00";
/// signal당 액션 1개(single-writer, Task #28-f) 위반.
pub(crate) const L01: &str = "LOOM-L01";

type LResult<T> = Result<T, Diagnostic>;

fn unsupported(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(L00, span, message)
}

/// lower가 IR 밖에 따로 돌려주는 소스 위치 표(design-7a §3.3). 모든 목록은
/// **소스 순서의 `Vec`**이다(§3.4 결정성 — 해시 순회 없음).
#[derive(Debug, Default, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct SpanTable {
    /// 컴포넌트 이름 → 함수 ident span.
    pub components: Vec<(String, Span)>,
    /// (컴포넌트, signal) → 선언 ident span.
    pub signals: Vec<(String, String, Span)>,
    /// `<C client:…/>` 참조 사이트: (참조하는 컴포넌트, 참조되는 컴포넌트, 태그 이름 span).
    pub component_refs: Vec<(String, String, Span)>,
    /// signal을 표시하는 binding 사이트.
    pub bindings: Vec<BindingSite>,
    /// 이벤트 attr(`onClick`/`onInput`) 사이트.
    pub events: Vec<EventSite>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum BindingKind {
    /// `{s()}`
    Text,
    /// `{s() && <X/>}`의 조건 signal
    Cond,
    /// `{s().map(...)}`
    List,
    /// `<input value={s()} onInput=…/>`
    Input,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct BindingSite {
    pub component: String,
    pub signal: String,
    pub kind: BindingKind,
    /// 리스트 row 템플릿 안의 사이트(row 원본)인지.
    pub in_row: bool,
    /// 사이트를 이루는 식(`s()`, `s() && …`의 `s()`, `s().map(...)` 전체, `<input>` 태그 이름).
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct EventSite {
    pub component: String,
    /// 핸들러가 쓰는 signal.
    pub signal: String,
    /// `"onClick"` 또는 `"onInput"`.
    pub attr: &'static str,
    /// attr 전체(`onClick={…}`) span.
    pub span: Span,
}

/// 한 컴포넌트 본문을 lower하는 동안 들고 다니는 문맥.
struct Cx<'a> {
    component: &'a str,
    signals: &'a [SignalDecl],
    known_components: &'a HashSet<String>,
    spans: &'a mut SpanTable,
}

impl Cx<'_> {
    fn find_signal(&self, name: &str) -> Option<&SignalDecl> {
        self.signals.iter().find(|s| s.name == name)
    }

    fn binding(&mut self, signal: &str, kind: BindingKind, row_var: Option<&str>, span: Span) {
        self.spans.bindings.push(BindingSite {
            component: self.component.to_string(),
            signal: signal.to_string(),
            kind,
            in_row: row_var.is_some(),
            span,
        });
    }

    fn event(&mut self, signal: &str, attr: &'static str, span: Span) {
        self.spans.events.push(EventSite {
            component: self.component.to_string(),
            signal: signal.to_string(),
            attr,
            span,
        });
    }
}

pub fn lower_module(module: &Module) -> LResult<(ComponentGraph, SpanTable)> {
    let (root_decl, helper_decls) = collect_component_fn_decls(module)?;
    let root_name = root_decl.ident.sym.to_string();

    let known_components: HashSet<String> = helper_decls
        .iter()
        .map(|f| f.ident.sym.to_string())
        .chain(std::iter::once(root_name.clone()))
        .collect();

    let mut spans = SpanTable::default();
    let mut components = Vec::new();
    components.push(lower_component_fn(root_decl, &known_components, &mut spans)?);
    for decl in &helper_decls {
        components.push(lower_component_fn(decl, &known_components, &mut spans)?);
    }

    check_one_action_per_signal(&components, &spans)?;

    Ok((
        ComponentGraph {
            components,
            root: root_name,
        },
        spans,
    ))
}

/// Task #28-f — statically enforces the "signal당 액션 1개" (single-writer)
/// invariant `formal-verification/lean4/ResumabilitySoundness.lean`'s
/// `SingleWriter` precondition depends on. **Scoped to `I32` signals only**
/// (found while wiring this up — an earlier draft checked every signal kind
/// and immediately broke `demo/counter-list.tsx`'s legitimate
/// `items.append(...)`/`items.rotate()` pair, both targeting the same
/// `StringList` signal): the Lean file's own new `dom_recovered_
/// resumability_sound` theorem (Task #28-e) proves `Bool`/`StringList`/`Str`
/// resume is safe under *any* number of writers, since it reads the DOM's
/// current state directly rather than a captured literal snapshot — only
/// `I32`'s resume path (`format_i32_capture_for_action`'s captured `v0`)
/// actually needs `SingleWriter`. Enforcing single-writer beyond `I32` would
/// reject legitimate, already-safe programs for no soundness reason. Runs
/// once per component, over `Component::action_signal_names()` (already
/// existed, unused until this call site — ir.rs's doc comment on it) cross-
/// referenced against `component.signals` for each name's `SignalInit`.
///
/// (7a C2: 판정은 그대로 IR에서 하고, 위치만 `SpanTable`의 이벤트 사이트에서
/// 찾는다 — 주 span은 두 번째 핸들러, 첫 번째에 label.)
fn check_one_action_per_signal(components: &[Component], spans: &SpanTable) -> LResult<()> {
    for component in components {
        let mut seen = HashSet::new();
        for signal_name in component.action_signal_names() {
            let is_i32 = component
                .signals
                .iter()
                .any(|s| s.name == signal_name && matches!(s.init, crate::ir::SignalInit::I32(_)));
            if !is_i32 {
                continue;
            }
            if !seen.insert(signal_name.clone()) {
                let sites: Vec<Span> = spans
                    .events
                    .iter()
                    .filter(|e| e.component == component.name && e.signal == signal_name)
                    .map(|e| e.span)
                    .collect();
                let fallback = spans
                    .components
                    .iter()
                    .find(|(n, _)| *n == component.name)
                    .map(|(_, s)| *s)
                    .unwrap_or_default();
                let primary = sites.get(1).copied().unwrap_or(fallback);
                let mut d = Diagnostic::error(
                    L01,
                    primary,
                    format!(
                        "component `{}` has more than one action-bearing binding site targeting i32 signal `{signal_name}` — \
                         signal당 액션 1개(single-writer) 제약 위반. formal-verification/lean4/ResumabilitySoundness.lean의 \
                         SingleWriter 전제가 바로 이 불변식에 의존하므로 정적으로 거부한다 — 허용하면 resume 시점에 두 \
                         독립 인스턴스가 서로 다른 값으로 갈라질 수 있다(그 파일의 반례 스케치 참고). Bool/StringList/Str \
                         시그널은 DOM-복구형이라 이 제약이 필요 없다(dom_recovered_resumability_sound 참고) — 그래서 \
                         이 검사는 i32 시그널에만 적용된다.",
                        component.name
                    ),
                );
                if let Some(first) = sites.first() {
                    d = d.with_label(*first, format!("`{signal_name}`의 첫 번째 액션 사이트"));
                }
                return Err(d);
            }
        }
    }
    Ok(())
}

/// Replaces the Phase 1 bug where only the first exported function was kept
/// and every other top-level declaration was silently discarded with no
/// diagnostic. Scans every top-level `function` declaration in the module:
/// exactly one *exported* function is the graph's root (Phase 1's original
/// restriction — kept here rather than relaxed, since allowing multiple
/// independent exported roots per module is a different, unrelated
/// generalization no current fixture/task needs); every other, non-exported
/// `function` is a candidate helper component, only actually meaningful if
/// some `ComponentRef` (transitively reachable from the root) names it —
/// unreferenced helpers are accepted, not lint-rejected (this narrow
/// compiler doesn't dead-code-eliminate; a stated scope cut, not an
/// oversight).
fn collect_component_fn_decls(module: &Module) -> LResult<(&FnDecl, Vec<&FnDecl>)> {
    let mut root: Option<&FnDecl> = None;
    let mut helpers = Vec::new();
    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl: Decl::Fn(f), .. })) => {
                if let Some(first) = root {
                    return Err(unsupported(
                        f.ident.span,
                        format!(
                            "more than one exported `function` found — Phase 1/증분 4 both require exactly one exported root component, got a second: `{}`",
                            f.ident.sym
                        ),
                    )
                    .with_label(first.ident.span, "첫 번째 export 함수"));
                }
                root = Some(f);
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) => {
                helpers.push(f);
            }
            _ => {}
        }
    }
    let root = root.ok_or_else(|| {
        unsupported(module.span.shrink_to_lo(), "no exported `function NAME() { ... }` found")
    })?;
    Ok((root, helpers))
}

fn lower_component_fn(
    fn_decl: &FnDecl,
    known_components: &HashSet<String>,
    spans: &mut SpanTable,
) -> LResult<Component> {
    let component_name = fn_decl.ident.sym.to_string();
    spans.components.push((component_name.clone(), fn_decl.ident.span));
    let body = fn_decl
        .function
        .body
        .as_ref()
        .ok_or_else(|| unsupported(fn_decl.ident.span, "component function has no body"))?;

    let mut signals = Vec::new();
    let mut shared_bindings = Vec::new();
    let mut root = None;

    for stmt in &body.stmts {
        match stmt {
            Stmt::Decl(Decl::Var(var_decl)) => {
                for decl in &var_decl.decls {
                    if let Some((signal, shared)) = try_lower_shared_signal_decl(decl)? {
                        spans.signals.push((component_name.clone(), signal.name.clone(), decl.name.span()));
                        signals.push(signal);
                        shared_bindings.push(shared);
                    } else if let Some(signal) = try_lower_signal_decl(decl)? {
                        spans.signals.push((component_name.clone(), signal.name.clone(), decl.name.span()));
                        signals.push(signal);
                    } else {
                        return Err(unsupported(
                            decl.span,
                            "unsupported `const` declaration in component body (only `const x = signal(...)`/`const x = sharedSignal(\"key\", ...)` is supported)",
                        ));
                    }
                }
            }
            Stmt::Return(ReturnStmt { arg: Some(arg), .. }) => {
                let jsx = unwrap_paren(arg);
                let Expr::JSXElement(el) = jsx else {
                    return Err(unsupported(jsx.span(), "component must `return` a single JSX element"));
                };
                let mut cx = Cx {
                    component: &component_name,
                    signals: &signals,
                    known_components,
                    spans,
                };
                root = Some(lower_element(el, &mut cx, None)?);
            }
            other => return Err(unsupported(other.span(), "unsupported statement in component body")),
        }
    }

    Ok(Component {
        name: component_name,
        signals,
        shared_bindings,
        root: root.ok_or_else(|| unsupported(fn_decl.ident.span, "component has no `return` statement"))?,
    })
}

/// `const cartCount = sharedSignal("cartCount", 0)` — tried *before*
/// `try_lower_signal_decl` (matches the callee name `sharedSignal` instead
/// of `signal`), producing both a normal `SignalDecl` (the writer role
/// reuses every existing local-signal codegen path unchanged) and a
/// `SharedBinding` recording that this signal should also be published to
/// `__loomShared`. `Ok(None)` (not an error) if the callee isn't
/// `sharedSignal` at all, so `lower_component_fn` can fall through to the
/// ordinary `signal(...)` case.
fn try_lower_shared_signal_decl(decl: &VarDeclarator) -> LResult<Option<(SignalDecl, SharedBinding)>> {
    let Pat::Ident(BindingIdent { id, .. }) = &decl.name else {
        return Ok(None);
    };
    let Some(init) = decl.init.as_deref() else {
        return Ok(None);
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = init
    else {
        return Ok(None);
    };
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        return Ok(None);
    };
    if sym != "sharedSignal" {
        return Ok(None);
    }
    let [key_arg, initial_arg] = args.as_slice() else {
        return Err(unsupported(
            *call_span,
            "sharedSignal() requires exactly two arguments, e.g. sharedSignal(\"cartCount\", 0)",
        ));
    };
    let Expr::Lit(Lit::Str(key)) = unwrap_paren(&key_arg.expr) else {
        return Err(unsupported(
            key_arg.expr.span(),
            "sharedSignal() requires a string key and an integer initial value, e.g. sharedSignal(\"cartCount\", 0)",
        ));
    };
    let Expr::Lit(Lit::Num(Number { value, .. })) = unwrap_paren(&initial_arg.expr) else {
        return Err(unsupported(
            initial_arg.expr.span(),
            "sharedSignal() requires a string key and an integer initial value, e.g. sharedSignal(\"cartCount\", 0)",
        ));
    };
    let name = id.sym.to_string();
    let key = key.value.to_string_lossy().into_owned();
    Ok(Some((
        SignalDecl {
            name: name.clone(),
            init: SignalInit::I32(*value as i32),
        },
        SharedBinding { signal: name, key },
    )))
}

fn unwrap_paren(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(ParenExpr { expr, .. }) => unwrap_paren(expr),
        other => other,
    }
}

/// 배열 리터럴의 원소를 문자열 리터럴로만 받는다(`signal([...])`와 prop 공용).
fn lower_string_array(elems: &[Option<ExprOrSpread>], array_span: Span, what: &str) -> LResult<Vec<String>> {
    elems
        .iter()
        .map(|e| {
            let e = e
                .as_ref()
                .ok_or_else(|| unsupported(array_span, "sparse array literals unsupported"))?;
            match unwrap_paren(&e.expr) {
                Expr::Lit(Lit::Str(s)) => Ok(s.value.to_string_lossy().into_owned()),
                other => Err(unsupported(other.span(), format!("{what} array elements must be string literals"))),
            }
        })
        .collect()
}

fn try_lower_signal_decl(decl: &VarDeclarator) -> LResult<Option<SignalDecl>> {
    let Pat::Ident(BindingIdent { id, .. }) = &decl.name else {
        return Ok(None);
    };
    let Some(init) = decl.init.as_deref() else {
        return Ok(None);
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = init
    else {
        return Ok(None);
    };
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        return Ok(None);
    };
    if sym != "signal" {
        return Ok(None);
    }
    let arg = args
        .first()
        .ok_or_else(|| unsupported(*call_span, "signal() requires one argument"))?;
    let init = match unwrap_paren(&arg.expr) {
        Expr::Lit(Lit::Num(Number { value, .. })) => SignalInit::I32(*value as i32),
        Expr::Lit(Lit::Bool(b)) => SignalInit::Bool(b.value),
        Expr::Lit(Lit::Str(s)) => SignalInit::Str(s.value.to_string_lossy().into_owned()),
        Expr::Array(ArrayLit { elems, span }) => {
            SignalInit::StringList(lower_string_array(elems, *span, "signal([...])")?)
        }
        other => {
            return Err(unsupported(
                other.span(),
                "signal(...) initializer must be a number, boolean, string, or string-array literal",
            ))
        }
    };
    Ok(Some(SignalDecl {
        name: id.sym.to_string(),
        init,
    }))
}

/// `row_var`: `Some(name)` while lowering a list row template, so a bare
/// `{name}` reference resolves to `JsxNode::RowItemText` instead of being
/// looked up as a top-level signal. `cx.known_components`: every component
/// name declared anywhere in this module (root + helpers) — used to decide
/// whether an uppercase-leading tag is a real component reference (and to
/// give a clear error if it names nothing declared) rather than treating
/// every tag as a bare HTML string.
fn lower_element(el: &JSXElement, cx: &mut Cx<'_>, row_var: Option<&str>) -> LResult<JsxNode> {
    let JSXElementName::Ident(Ident { sym: tag, span: tag_span, .. }) = &el.opening.name else {
        return Err(unsupported(el.opening.name.span(), "only plain tag names are supported"));
    };
    let tag = tag.to_string();

    if tag.chars().next().is_some_and(|c| c.is_uppercase()) {
        if !cx.known_components.contains(&tag) {
            return Err(unsupported(
                *tag_span,
                format!(
                    "`<{tag}/>` looks like a component reference (uppercase-leading name) but no `function {tag}()` is declared in this module"
                ),
            ));
        }
        let node = lower_component_ref(el, &tag)?;
        cx.spans
            .component_refs
            .push((cx.component.to_string(), tag.clone(), *tag_span));
        return Ok(node);
    }

    if tag == "input" {
        let node = lower_input_binding(el, cx)?;
        if let JsxNode::InputBinding { signal } = &node {
            cx.binding(signal, BindingKind::Input, row_var, *tag_span);
        }
        return Ok(node);
    }

    let mut class = None;
    let mut action = None;
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            return Err(unsupported(attr.span(), "spread attributes are not supported"));
        };
        let JSXAttrName::Ident(IdentName { sym: name, .. }) = &attr.name else {
            return Err(unsupported(attr.name.span(), "namespaced attributes are not supported"));
        };
        match name.as_ref() {
            "class" | "className" => {
                let Some(JSXAttrValue::Str(s)) = &attr.value else {
                    return Err(unsupported(attr.span, "`class` must be a static string literal"));
                };
                class = Some(s.value.to_string_lossy().into_owned());
            }
            "onClick" => {
                let a = lower_click_action(attr, cx)?;
                cx.event(a.signal_name(), "onClick", attr.span);
                action = Some(a);
            }
            other => {
                return Err(unsupported(
                    attr.name.span(),
                    format!("unsupported attribute `{other}` on <{tag}>"),
                ))
            }
        }
    }

    let mut children = Vec::new();
    for child in &el.children {
        if let Some(node) = lower_child(child, cx, row_var)? {
            children.push(node);
        }
    }

    Ok(JsxNode::Element {
        tag,
        class,
        action,
        children,
    })
}

/// Parses `<ComponentName client:load|client:visible prop={literal} .../>`
/// into `JsxNode::ComponentRef`. This is the **only** place `client:`-
/// namespaced attributes are accepted — plain `Element`/`InputBinding`
/// still reject every namespaced attribute unchanged (their own errors,
/// untouched by this function).
fn lower_component_ref(el: &JSXElement, name: &str) -> LResult<JsxNode> {
    let mut activation = None;
    let mut props = Vec::new();
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            return Err(unsupported(attr.span(), "spread attributes are not supported"));
        };
        match &attr.name {
            JSXAttrName::JSXNamespacedName(JSXNamespacedName {
                ns, name: attr_name, ..
            }) => {
                if ns.sym.as_ref() != "client" {
                    return Err(unsupported(
                        ns.span,
                        format!(
                            "unsupported attribute namespace `{}:` on <{name}> (only `client:` is supported)",
                            ns.sym
                        ),
                    ));
                }
                activation = Some(match attr_name.sym.as_ref() {
                    "load" => IslandActivation::Load,
                    "visible" => IslandActivation::Visible,
                    other => {
                        return Err(unsupported(
                            attr.name.span(),
                            format!(
                                "unsupported `client:{other}` directive on <{name}> (supported: client:load, client:visible)"
                            ),
                        ))
                    }
                });
            }
            JSXAttrName::Ident(IdentName { sym: prop_name, .. }) => {
                let value = attr.value.as_ref().ok_or_else(|| {
                    unsupported(attr.span, format!("prop `{prop_name}` on <{name}> requires a value"))
                })?;
                props.push((prop_name.to_string(), lower_prop_literal(value)?));
            }
        }
    }
    let activation = activation.ok_or_else(|| {
        unsupported(
            el.opening.name.span(),
            format!(
                "<{name}/> has no `client:load`/`client:visible` directive — 증분 4 scope: ordinary (non-Island) component references are not yet supported, every ComponentRef must currently carry an activation"
            ),
        )
    })?;
    Ok(JsxNode::ComponentRef {
        component: name.to_string(),
        activation,
        props,
    })
}

/// A prop literal must be exactly one of the same shapes `signal(...)`'s own
/// initializer already allows (see `try_lower_signal_decl`) — Phase 1's "no
/// non-literal initial values anywhere" restriction, extended rather than
/// broken (see `ir::PropLiteral`'s doc comment).
fn lower_prop_literal(value: &JSXAttrValue) -> LResult<PropLiteral> {
    match value {
        JSXAttrValue::Str(s) => Ok(PropLiteral::Str(s.value.to_string_lossy().into_owned())),
        JSXAttrValue::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => match unwrap_paren(expr) {
            Expr::Lit(Lit::Num(Number { value, .. })) => Ok(PropLiteral::I32(*value as i32)),
            Expr::Lit(Lit::Bool(b)) => Ok(PropLiteral::Bool(b.value)),
            Expr::Lit(Lit::Str(s)) => Ok(PropLiteral::Str(s.value.to_string_lossy().into_owned())),
            Expr::Array(ArrayLit { elems, span }) => {
                Ok(PropLiteral::StringList(lower_string_array(elems, *span, "prop")?))
            }
            other => Err(unsupported(
                other.span(),
                "component prop value must be a number, boolean, string, or string-array literal",
            )),
        },
        other => Err(unsupported(other.span(), "unsupported component prop value shape")),
    }
}

/// `<input value={s()} onInput={(e) => s.set(e.target.value)}/>` — Phase 1's
/// two-way text binding. Handled as its own `JsxNode` (not a generic
/// `Element`, which has no slot for "this element's value and its update
/// handler are the same binding site") — the same reason `{cond() && <X/>}`
/// got its own `JsxNode::CondBinding` rather than riding on `Element`.
fn lower_input_binding(el: &JSXElement, cx: &mut Cx<'_>) -> LResult<JsxNode> {
    let mut value_signal: Option<(String, Span)> = None;
    let mut oninput_signal: Option<(String, Span)> = None;
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            return Err(unsupported(attr.span(), "spread attributes are not supported"));
        };
        let JSXAttrName::Ident(IdentName { sym: name, .. }) = &attr.name else {
            return Err(unsupported(attr.name.span(), "namespaced attributes are not supported"));
        };
        match name.as_ref() {
            "value" => value_signal = Some((lower_input_value_attr(attr, cx)?, attr.span)),
            "onInput" => oninput_signal = Some((lower_input_oninput_attr(attr, cx)?, attr.span)),
            other => {
                return Err(unsupported(
                    attr.name.span(),
                    format!(
                        "<input> only supports `value`/`onInput` (Phase 1 scope: two-way text binding only), got `{other}`"
                    ),
                ))
            }
        }
    }
    let input_span = el.opening.name.span();
    let (value_signal, value_span) = value_signal
        .ok_or_else(|| unsupported(input_span, "<input> requires a `value={signalName()}` attribute"))?;
    let (oninput_signal, oninput_span) = oninput_signal.ok_or_else(|| {
        unsupported(input_span, "<input> requires an `onInput={(e) => signalName.set(e.target.value)}` attribute")
    })?;
    if value_signal != oninput_signal {
        return Err(unsupported(
            oninput_span,
            format!(
                "<input>'s `value` and `onInput` must reference the same signal, got `{value_signal}` and `{oninput_signal}`"
            ),
        )
        .with_label(value_span, format!("`value`는 `{value_signal}`")));
    }
    cx.event(&oninput_signal, "onInput", oninput_span);
    Ok(JsxNode::InputBinding { signal: value_signal })
}

fn lower_input_value_attr(attr: &JSXAttr, cx: &Cx<'_>) -> LResult<String> {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        return Err(unsupported(attr.span, "<input>'s `value` must be `{signalName()}`"));
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = unwrap_paren(expr)
    else {
        return Err(unsupported(expr.span(), "<input>'s `value` must be a bare `signalName()` call"));
    };
    if let Some(first) = args.first() {
        return Err(unsupported(
            first.expr.span(),
            "`signalName()` in <input>'s `value` must take no arguments",
        ));
    }
    let Expr::Ident(Ident { sym, span: name_span, .. }) = callee.as_ref() else {
        return Err(unsupported(callee.span(), "<input>'s `value` must call a bare signal identifier"));
    };
    let name = sym.to_string();
    let signal = cx
        .find_signal(&name)
        .ok_or_else(|| unsupported(*name_span, format!("<input>'s `value` references unknown signal `{name}`")))?;
    if !matches!(signal.init, SignalInit::Str(_)) {
        return Err(unsupported(
            *name_span,
            format!("<input>'s `value` requires a Str signal, `{name}` is not one"),
        ));
    }
    Ok(name)
}

fn lower_input_oninput_attr(attr: &JSXAttr, cx: &Cx<'_>) -> LResult<String> {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        return Err(unsupported(attr.span, "<input>'s `onInput` must be `{(e) => ...}`"));
    };
    let Expr::Arrow(ArrowExpr { body, params, span: arrow_span, .. }) = unwrap_paren(expr) else {
        return Err(unsupported(expr.span(), "<input>'s `onInput` must be an arrow function"));
    };
    let [Pat::Ident(BindingIdent { id: event_param, .. })] = params.as_slice() else {
        return Err(unsupported(
            *arrow_span,
            "<input>'s `onInput` arrow must take exactly one parameter, e.g. `(e) => ...`",
        ));
    };
    let event_name = event_param.sym.to_string();
    let BlockStmtOrExpr::Expr(body_expr) = body.as_ref() else {
        return Err(unsupported(
            body.span(),
            "<input>'s `onInput` arrow body must be a single expression `(e) => signal.set(e.target.value)` (not a block `{ ... }`)",
        ));
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = unwrap_paren(body_expr)
    else {
        return Err(unsupported(body_expr.span(), "<input>'s `onInput` body must be a single `signal.set(...)` call"));
    };
    let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
        return Err(unsupported(
            callee.span(),
            "<input>'s `onInput` body must call a method on a signal, e.g. `signal.set(...)`",
        ));
    };
    let Expr::Ident(Ident { sym: signal_name, span: signal_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "<input>'s `onInput` target must be a bare signal identifier"));
    };
    let signal_name = signal_name.to_string();
    let MemberProp::Ident(IdentName { sym: method, span: method_span }) = prop else {
        return Err(unsupported(prop.span(), "unsupported method reference in <input>'s `onInput`"));
    };
    if method != "set" {
        return Err(unsupported(
            *method_span,
            format!("<input>'s `onInput` must call `.set(...)`, got `.{method}(...)`"),
        ));
    }
    let arg = args
        .first()
        .ok_or_else(|| unsupported(*call_span, "`.set(...)` requires one argument, `e.target.value`"))?;
    let wrong_arg = || format!("<input>'s `.set(...)` argument must be `{event_name}.target.value`");
    let Expr::Member(MemberExpr {
        obj: target_obj,
        prop: value_prop,
        ..
    }) = unwrap_paren(&arg.expr)
    else {
        return Err(unsupported(arg.expr.span(), wrong_arg()));
    };
    let MemberProp::Ident(IdentName { sym: value_sym, span: value_span }) = value_prop else {
        return Err(unsupported(value_prop.span(), wrong_arg()));
    };
    if value_sym != "value" {
        return Err(unsupported(
            *value_span,
            format!("<input>'s `.set(...)` argument must end in `.value`, got `.{value_sym}`"),
        ));
    }
    let Expr::Member(MemberExpr {
        obj: event_obj,
        prop: target_prop,
        ..
    }) = target_obj.as_ref()
    else {
        return Err(unsupported(target_obj.span(), wrong_arg()));
    };
    let MemberProp::Ident(IdentName { sym: target_sym, span: target_span }) = target_prop else {
        return Err(unsupported(target_prop.span(), wrong_arg()));
    };
    if target_sym != "target" {
        return Err(unsupported(
            *target_span,
            format!(
                "<input>'s `.set(...)` argument must be `{event_name}.target.value`, got `.{target_sym}` instead of `.target`"
            ),
        ));
    }
    let Expr::Ident(Ident { sym: event_ref, span: event_ref_span, .. }) = event_obj.as_ref() else {
        return Err(unsupported(
            event_obj.span(),
            format!("<input>'s `.set(...)` argument must reference the arrow's own event parameter `{event_name}`"),
        ));
    };
    if event_ref.as_ref() != event_name {
        return Err(unsupported(
            *event_ref_span,
            format!(
                "<input>'s `.set(...)` argument must reference the arrow's own event parameter `{event_name}`, got `{event_ref}`"
            ),
        ));
    }
    let signal = cx.find_signal(&signal_name).ok_or_else(|| {
        unsupported(*signal_span, format!("<input>'s `onInput` references unknown signal `{signal_name}`"))
    })?;
    if !matches!(signal.init, SignalInit::Str(_)) {
        return Err(unsupported(
            *signal_span,
            format!("<input>'s `onInput` requires a Str signal, `{signal_name}` is not one"),
        ));
    }
    Ok(signal_name)
}

fn lower_click_action(attr: &JSXAttr, cx: &Cx<'_>) -> LResult<ClickAction> {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        return Err(unsupported(attr.span, "`onClick` must be `{() => ...}`"));
    };
    let Expr::Arrow(ArrowExpr { body, params, .. }) = unwrap_paren(expr) else {
        return Err(unsupported(expr.span(), "`onClick` must be an arrow function"));
    };
    if let Some(first) = params.first() {
        return Err(unsupported(first.span(), "`onClick` arrow function must take no parameters"));
    }
    let BlockStmtOrExpr::Expr(body_expr) = body.as_ref() else {
        return Err(unsupported(
            body.span(),
            "`onClick` arrow body must be a single expression, e.g. `() => x.increment()` (not a block `{ ... }`)",
        ));
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = unwrap_paren(body_expr)
    else {
        return Err(unsupported(
            body_expr.span(),
            "`onClick` body must be a single method call like `signal.increment()`",
        ));
    };
    let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
        return Err(unsupported(
            callee.span(),
            "`onClick` body must call a method on a signal, e.g. `signal.increment()`",
        ));
    };
    let Expr::Ident(Ident { sym: signal_name, span: signal_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "`onClick` target must be a bare signal identifier"));
    };
    let signal_name = signal_name.to_string();
    let signal = cx
        .find_signal(&signal_name)
        .ok_or_else(|| unsupported(*signal_span, format!("`onClick` references unknown signal `{signal_name}`")))?;
    let MemberProp::Ident(IdentName { sym: method, span: method_span }) = prop else {
        return Err(unsupported(prop.span(), "unsupported method reference in `onClick`"));
    };
    let require = |ok: bool, msg: String| -> LResult<()> {
        if ok {
            Ok(())
        } else {
            Err(unsupported(*signal_span, msg))
        }
    };

    match method.as_ref() {
        "increment" => {
            require(
                matches!(signal.init, SignalInit::I32(_)),
                format!("`.increment()` requires an I32 signal, `{signal_name}` is not one"),
            )?;
            Ok(ClickAction::Increment { signal: signal_name })
        }
        "rotate" => {
            require(
                matches!(signal.init, SignalInit::StringList(_)),
                format!("`.rotate()` requires a string-list signal, `{signal_name}` is not one"),
            )?;
            Ok(ClickAction::Rotate { signal: signal_name })
        }
        "append" => {
            require(
                matches!(signal.init, SignalInit::StringList(_)),
                format!("`.append(...)` requires a string-list signal, `{signal_name}` is not one"),
            )?;
            let arg = args
                .first()
                .ok_or_else(|| unsupported(*call_span, "`.append(...)` requires one string-literal argument"))?;
            let Expr::Lit(Lit::Str(s)) = unwrap_paren(&arg.expr) else {
                return Err(unsupported(
                    arg.expr.span(),
                    "`.append(...)` argument must be a string literal (Phase 1 scope: no dynamic expressions)",
                ));
            };
            Ok(ClickAction::Append {
                signal: signal_name,
                literal: s.value.to_string_lossy().into_owned(),
            })
        }
        "toggle" => {
            require(
                matches!(signal.init, SignalInit::Bool(_)),
                format!("`.toggle()` requires a Bool signal, `{signal_name}` is not one"),
            )?;
            Ok(ClickAction::Toggle { signal: signal_name })
        }
        other => Err(unsupported(
            *method_span,
            format!("unsupported signal method `.{other}()` (supported: increment, append, rotate, toggle)"),
        )),
    }
}

fn lower_child(child: &JSXElementChild, cx: &mut Cx<'_>, row_var: Option<&str>) -> LResult<Option<JsxNode>> {
    match child {
        JSXElementChild::JSXText(text) => {
            let trimmed = text.value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                Ok(Some(JsxNode::StaticText(trimmed.to_string())))
            }
        }
        JSXElementChild::JSXElement(el) => Ok(Some(lower_element(el, cx, row_var)?)),
        JSXElementChild::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => Ok(Some(lower_expr_container(expr, cx, row_var)?)),
        other => Err(unsupported(other.span(), "unsupported JSX child")),
    }
}

fn lower_expr_container(expr: &Expr, cx: &mut Cx<'_>, row_var: Option<&str>) -> LResult<JsxNode> {
    match unwrap_paren(expr) {
        // `{item}` inside a list row template.
        Expr::Ident(Ident { sym, .. }) if row_var == Some(sym.as_ref()) => Ok(JsxNode::RowItemText),

        // `{sharedValue("cartCount", 0)}` — read-only cross-widget display,
        // no local signal lookup at all (unlike every other binding here) —
        // that absence is itself the point (see `JsxNode::SharedValueBinding`'s
        // doc comment).
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            span: call_span,
            ..
        }) if matches!(callee.as_ref(), Expr::Ident(Ident { sym, .. }) if sym == "sharedValue") => {
            let [key_arg, initial_arg] = args.as_slice() else {
                return Err(unsupported(
                    *call_span,
                    "sharedValue() requires exactly two arguments, e.g. sharedValue(\"cartCount\", 0)",
                ));
            };
            let Expr::Lit(Lit::Str(key)) = unwrap_paren(&key_arg.expr) else {
                return Err(unsupported(
                    key_arg.expr.span(),
                    "sharedValue() requires a string key and an integer initial value, e.g. sharedValue(\"cartCount\", 0)",
                ));
            };
            let Expr::Lit(Lit::Num(Number { value, .. })) = unwrap_paren(&initial_arg.expr) else {
                return Err(unsupported(
                    initial_arg.expr.span(),
                    "sharedValue() requires a string key and an integer initial value, e.g. sharedValue(\"cartCount\", 0)",
                ));
            };
            Ok(JsxNode::SharedValueBinding {
                key: key.value.to_string_lossy().into_owned(),
                initial: *value as i32,
            })
        }

        // `{cond() && <tag>...</tag>}` — compiles to RQ1 Track T candidate F.
        // (2026-10-17 note: "candidate F" is the Phase 1 label. `cond_reconcile_f_system` is the CSS-hide preserve variant, i.e. the protocol's CSS 은닉 (v-show) control arm — not the F family F-i/ii/iii added 2026-07-31. See cond.rs header.)
        Expr::Bin(BinExpr {
            op: BinaryOp::LogicalAnd,
            left,
            right,
            ..
        }) => {
            // `{open() && item.length > 2 && <X/>}` — SWC는 `&&`를 좌결합으로
            // 파싱하므로 `left`가 다시 `&&`이면 그 오른쪽이 **row 술어**다.
            // row 템플릿 밖에서는 애초에 row 변수가 없으므로 자연히 거부된다.
            let (left, row_predicate) = match unwrap_paren(left) {
                Expr::Bin(BinExpr {
                    op: BinaryOp::LogicalAnd,
                    left: inner_left,
                    right: inner_right,
                    ..
                }) => {
                    let row_var = row_var.ok_or_else(|| {
                        unsupported(
                            inner_right.span(),
                            "`a() && b && <X/>` 형태의 가운데 항은 row 템플릿 안에서만 쓸 수 있다 — 바깥에서는 조건 signal 하나만 받는다",
                        )
                    })?;
                    (inner_left.as_ref(), Some(parse_row_predicate(inner_right, row_var)?))
                }
                other => (other, None),
            };
            let Expr::Call(CallExpr {
                callee: Callee::Expr(callee),
                args,
                span: cond_call_span,
                ..
            }) = unwrap_paren(left)
            else {
                return Err(unsupported(
                    left.span(),
                    "conditional's left side must be a bare `signalName()` boolean read",
                ));
            };
            if let Some(first) = args.first() {
                return Err(unsupported(
                    first.expr.span(),
                    "`signalName()` in a conditional must take no arguments",
                ));
            }
            let Expr::Ident(Ident { sym: cond_name, span: cond_name_span, .. }) = callee.as_ref() else {
                return Err(unsupported(
                    callee.span(),
                    "conditional's left side must be a bare signal identifier call",
                ));
            };
            let cond_name = cond_name.to_string();
            let signal = cx.find_signal(&cond_name).ok_or_else(|| {
                unsupported(
                    *cond_name_span,
                    format!("`{{{cond_name}() && ...}}` references unknown signal `{cond_name}`"),
                )
            })?;
            if !matches!(signal.init, SignalInit::Bool(_)) {
                return Err(unsupported(
                    *cond_name_span,
                    format!("`&&`-conditional requires a Bool signal, `{cond_name}` is not one"),
                ));
            }
            let cond_call_span = *cond_call_span;

            let Expr::JSXElement(then_el) = unwrap_paren(right) else {
                return Err(unsupported(right.span(), "conditional's right side must be a single JSX element"));
            };
            let then_branch = lower_element(then_el, cx, row_var)?;
            // 7a C2: 옛 `unreachable!()`. `<Widget client:load/>`나 `<input>`을
            // 오른쪽에 쓰면 사용자가 실제로 도달하므로 정식 진단으로 바꿨다.
            let JsxNode::Element { children, .. } = &then_branch else {
                return Err(unsupported(
                    then_el.opening.name.span(),
                    format!(
                        "conditional's right side must be a plain HTML element (`{{{cond_name}() && <div>…</div>}}`), not a component reference or <input>"
                    ),
                ));
            };
            // **2026-08-07**: 여기 있던 "정적 자식만 허용" 거부를 걷었다.
            // 조건부 안의 `{signal()}`/`{signal().map(...)}`/중첩 엘리먼트는
            // 이제 codegen이 사이트별 전용 함수로 방출한다(codegen.rs의
            // `emit_lazy_subtree`). 거부가 남아 있으면 RQ1 §5 트랙 N이 요구하는
            // 3중 중첩 픽스처를 **컴파일러가 생성할 수 없고**, §2.3-1이
            // "컴파일러가 생성할 수 없는 형태의 코드 금지"라 손코딩 우회도
            // 막혀 있었다.
            //
            // 아직 못 받는 것들은 codegen 쪽에서 각각 이유를 붙여 거부한다
            // (`collect_lazy_slots_node`/`emit_lazy_subtree`의 패닉 문구):
            // 조건부 안의 중첩 조건부, `<input>` 양방향 binding, 자체 `onClick`,
            // `<Component/>` 참조, `sharedValue(...)`.
            for (child, child_src) in children.iter().zip(dynamic_or_element_children(then_el)) {
                if matches!(child, JsxNode::CondBinding { .. }) {
                    return Err(unsupported(
                        child_src,
                        format!(
                            "`{{{cond_name}() && ...}}` 안에 또 조건부를 넣는 것은 아직 지원하지 않는다 — 안쪽 사이트도 자기 marker/전용 함수/`CondNested`가 필요한데, 그 중첩 생성 경로가 아직 없다(codegen.rs의 `CondBinding` arm 참고)"
                        ),
                    ));
                }
            }
            // 사이트 기록은 then 가지를 다 내려간 **뒤**다 — 그래야 이 조건부의
            // 사이트가 안쪽 사이트보다 뒤에 쌓인다. 순서가 아니라 span으로
            // 찾으므로 무관하지만, 기록 위치를 한 곳으로 고정해 둔다.
            cx.binding(&cond_name, BindingKind::Cond, row_var, cond_call_span);
            Ok(JsxNode::CondBinding {
                cond: cond_name,
                row_predicate,
                then_branch: Box::new(then_branch),
            })
        }

        // `{signalName()}` — a bare zero-arg call to a known signal.
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            span: call_span,
            ..
        }) if args.is_empty() && matches!(callee.as_ref(), Expr::Ident(_)) => {
            let Expr::Ident(Ident { sym, span: name_span, .. }) = callee.as_ref() else {
                unreachable!()
            };
            let name = sym.to_string();
            if cx.find_signal(&name).is_some() {
                cx.binding(&name, BindingKind::Text, row_var, *call_span);
                Ok(JsxNode::TextBinding { signal: name })
            } else {
                Err(unsupported(*name_span, format!("`{{{name}()}}` does not reference a declared signal")))
            }
        }

        // `{signalName().map((row) => <el>{row}</el>)}`
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            span: map_call_span,
            ..
        }) => {
            let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
                return Err(unsupported(
                    callee.span(),
                    "unsupported dynamic expression in JSX (only `signal()`, or `signal().map(...)`, are supported)",
                ));
            };
            let MemberProp::Ident(IdentName { sym: method, span: method_span }) = prop else {
                return Err(unsupported(prop.span(), "unsupported member call in JSX"));
            };
            if method != "map" {
                return Err(unsupported(
                    *method_span,
                    format!("only `.map(...)` is supported on a list signal read, got `.{method}(...)`"),
                ));
            }
            // `xs().slice(0, item.length).map(...)` — row 변수로 좁힌 리스트
            // (2026-08-07). `.slice`가 끼어 있으면 한 겹 벗기고 그 사실을 기록한다.
            let (obj, row_slice) = match obj.as_ref() {
                Expr::Call(CallExpr { callee: Callee::Expr(c), args: slice_args, span: slice_span, .. })
                    if matches!(c.as_ref(), Expr::Member(MemberExpr { prop: MemberProp::Ident(IdentName { sym, .. }), .. }) if sym == "slice") =>
                {
                    let Expr::Member(MemberExpr { obj: slice_obj, .. }) = c.as_ref() else {
                        unreachable!()
                    };
                    let row_var = row_var.ok_or_else(|| {
                        unsupported(
                            *slice_span,
                            "`.slice(...)`는 row 템플릿 안에서만 쓸 수 있다 — row 변수로 좁히는 것이 유일한 용도다",
                        )
                    })?;
                    parse_row_slice(slice_args, row_var, *slice_span)?;
                    (slice_obj.as_ref(), Some(RowSlice::PrefixByItemLen))
                }
                other => (other, None),
            };
            let Expr::Call(CallExpr {
                callee: Callee::Expr(inner_callee),
                args: inner_args,
                ..
            }) = obj
            else {
                return Err(unsupported(obj.span(), "`.map(...)` must be called directly on `signalName()`"));
            };
            if let Some(first) = inner_args.first() {
                return Err(unsupported(
                    first.expr.span(),
                    "`signalName()` must be called with no arguments before `.map(...)`",
                ));
            }
            let Expr::Ident(Ident { sym: signal_name, span: signal_span, .. }) = inner_callee.as_ref() else {
                return Err(unsupported(inner_callee.span(), "`.map(...)` must be called on a bare `signalName()`"));
            };
            let signal_name = signal_name.to_string();
            let signal = cx
                .find_signal(&signal_name)
                .ok_or_else(|| unsupported(*signal_span, format!("`.map(...)` on unknown signal `{signal_name}`")))?;
            if !matches!(signal.init, SignalInit::StringList(_)) {
                return Err(unsupported(
                    *signal_span,
                    format!("`.map(...)` requires a string-list signal, `{signal_name}` is not one"),
                ));
            }

            let map_arg = args
                .first()
                .ok_or_else(|| unsupported(*map_call_span, "`.map(...)` requires one arrow-function argument"))?;
            let Expr::Arrow(ArrowExpr {
                params,
                body: row_body,
                span: arrow_span,
                ..
            }) = unwrap_paren(&map_arg.expr)
            else {
                return Err(unsupported(map_arg.expr.span(), "`.map(...)` argument must be an arrow function"));
            };
            let [Pat::Ident(BindingIdent { id: row_ident, .. })] = params.as_slice() else {
                return Err(unsupported(
                    *arrow_span,
                    "`.map((row) => ...)` must take exactly one bare identifier parameter",
                ));
            };
            let row_var_name = row_ident.sym.to_string();
            let BlockStmtOrExpr::Expr(row_expr) = row_body.as_ref() else {
                return Err(unsupported(
                    row_body.span(),
                    "`.map(...)` row template must be a single JSX expression, not a block",
                ));
            };
            let Expr::JSXElement(row_el) = unwrap_paren(row_expr) else {
                return Err(unsupported(row_expr.span(), "`.map(...)` row template must be a JSX element"));
            };
            let row = lower_element(row_el, cx, Some(&row_var_name))?;
            cx.binding(&signal_name, BindingKind::List, row_var, *map_call_span);

            Ok(JsxNode::ListBinding {
                signal: signal_name,
                row_var: row_var_name,
                row: Box::new(row),
                row_slice,
            })
        }
        other => Err(unsupported(other.span(), "unsupported JSX expression")),
    }
}

/// `lower_element`가 IR 자식을 만드는 것과 **같은 규칙**(공백뿐인 JSXText는
/// 건너뜀)으로 원본 JSX 자식의 span을 나열한다 — IR 자식과 1:1로 짝지어
/// 진단 위치를 찾는 데만 쓴다. (`lower_child`가 성공한 뒤에만 불리므로
/// 나머지 자식 종류는 이미 걸러져 있다.)
fn dynamic_or_element_children(el: &JSXElement) -> impl Iterator<Item = Span> + '_ {
    el.children.iter().filter_map(|child| match child {
        JSXElementChild::JSXText(text) if text.value.trim().is_empty() => None,
        other => Some(other.span()),
    })
}

/// `item.length > n` 한 형태만 받는다. 표현식 언어를 여는 것이 목적이 아니라,
/// **row마다 구조가 달라지는 픽스처**를 컴파일러가 낼 수 있게 하는 것이 목적이다
/// (RQ1 §5 트랙 N — 구간 크기가 row마다 같으면 못 잡는 결함이 있다).
fn parse_row_predicate(expr: &Expr, row_var: &str) -> LResult<RowPredicate> {
    let Expr::Bin(BinExpr { op: BinaryOp::Gt, left, right, .. }) = unwrap_paren(expr) else {
        return Err(unsupported(
            expr.span(),
            format!("row 술어는 `{row_var}.length > <정수>` 형태여야 한다"),
        ));
    };
    let Expr::Member(MemberExpr { obj, prop: MemberProp::Ident(IdentName { sym, span: prop_span }), .. }) =
        unwrap_paren(left)
    else {
        return Err(unsupported(left.span(), format!("row 술어의 왼쪽은 `{row_var}.length` 여야 한다")));
    };
    if sym != "length" {
        return Err(unsupported(
            *prop_span,
            format!("row 술어가 지원하는 속성은 `.length` 뿐이다 (`.{sym}` 받음)"),
        ));
    }
    let Expr::Ident(Ident { sym: obj_name, span: obj_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "row 술어의 왼쪽은 row 변수여야 한다"));
    };
    if obj_name != row_var {
        return Err(unsupported(
            *obj_span,
            format!("row 술어는 이 row의 변수(`{row_var}`)를 써야 한다 — `{obj_name}` 받음"),
        ));
    }
    let Expr::Lit(Lit::Num(Number { value, .. })) = unwrap_paren(right) else {
        return Err(unsupported(right.span(), "row 술어의 오른쪽은 정수 리터럴이어야 한다"));
    };
    Ok(RowPredicate::LenGt(*value as usize))
}

/// `.slice(0, item.length)` 한 형태만. 인자 모양을 실제로 검사한다 —
/// 통과시켜 놓고 다른 의미로 코드를 내면 저작 표면과 산출물이 조용히 어긋난다.
fn parse_row_slice(args: &[ExprOrSpread], row_var: &str, slice_span: Span) -> LResult<()> {
    let [start, end] = args else {
        return Err(unsupported(
            slice_span,
            format!("`.slice(...)`는 `slice(0, {row_var}.length)` 형태여야 한다 (인자 2개)"),
        ));
    };
    let Expr::Lit(Lit::Num(Number { value: 0.0, .. })) = unwrap_paren(&start.expr) else {
        return Err(unsupported(start.expr.span(), "`.slice(...)`의 첫 인자는 `0` 이어야 한다"));
    };
    let Expr::Member(MemberExpr { obj, prop: MemberProp::Ident(IdentName { sym, span: prop_span }), .. }) =
        unwrap_paren(&end.expr)
    else {
        return Err(unsupported(
            end.expr.span(),
            format!("`.slice(...)`의 둘째 인자는 `{row_var}.length` 여야 한다"),
        ));
    };
    if sym != "length" {
        return Err(unsupported(*prop_span, "`.slice(...)`의 둘째 인자는 `.length` 여야 한다"));
    }
    let Expr::Ident(Ident { sym: obj_name, span: obj_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "`.slice(...)`의 둘째 인자는 row 변수여야 한다"));
    };
    if obj_name != row_var {
        return Err(unsupported(
            *obj_span,
            format!("`.slice(...)`는 이 row의 변수(`{row_var}`)를 써야 한다 — `{obj_name}` 받음"),
        ));
    }
    Ok(())
}
