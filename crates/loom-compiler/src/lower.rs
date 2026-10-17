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
//!       with a clear "not yet supported" panic (see `lower_component_ref`).
//! Anything outside this is a compile error (`panic!` with a clear message)
//! — this is a real, narrow compiler front end, not a fuzzy best-effort one.

use crate::ir::{
    ClickAction, Component, ComponentGraph, IslandActivation, JsxNode, PropLiteral, RowPredicate,
    RowSlice, SharedBinding, SignalDecl, SignalInit,
};
use std::collections::HashSet;
use swc_ecma_ast::*;

pub fn lower_module(module: &Module) -> ComponentGraph {
    let (root_decl, helper_decls) = collect_component_fn_decls(module);
    let root_name = root_decl.ident.sym.to_string();

    let known_components: HashSet<String> = helper_decls
        .iter()
        .map(|f| f.ident.sym.to_string())
        .chain(std::iter::once(root_name.clone()))
        .collect();

    let mut components = Vec::new();
    components.push(lower_component_fn(&root_decl, &known_components));
    for decl in &helper_decls {
        components.push(lower_component_fn(decl, &known_components));
    }

    check_one_action_per_signal(&components);

    ComponentGraph {
        components,
        root: root_name,
    }
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
fn check_one_action_per_signal(components: &[Component]) {
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
                panic!(
                    "loomc: component `{}` has more than one action-bearing binding site targeting i32 signal `{signal_name}` — \
                     signal당 액션 1개(single-writer) 제약 위반. formal-verification/lean4/ResumabilitySoundness.lean의 \
                     SingleWriter 전제가 바로 이 불변식에 의존하므로 정적으로 거부한다 — 허용하면 resume 시점에 두 \
                     독립 인스턴스가 서로 다른 값으로 갈라질 수 있다(그 파일의 반례 스케치 참고). Bool/StringList/Str \
                     시그널은 DOM-복구형이라 이 제약이 필요 없다(dom_recovered_resumability_sound 참고) — 그래서 \
                     이 검사는 i32 시그널에만 적용된다.",
                    component.name
                );
            }
        }
    }
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
fn collect_component_fn_decls(module: &Module) -> (FnDecl, Vec<FnDecl>) {
    let mut root = None;
    let mut helpers = Vec::new();
    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl: Decl::Fn(f), .. })) => {
                if root.is_some() {
                    panic!(
                        "loomc: more than one exported `function` found — Phase 1/증분 4 both require exactly one exported root component, got a second: `{}`",
                        f.ident.sym
                    );
                }
                root = Some(f.clone());
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) => {
                helpers.push(f.clone());
            }
            _ => {}
        }
    }
    let root = root.unwrap_or_else(|| panic!("loomc: no exported `function NAME() {{ ... }}` found"));
    (root, helpers)
}

fn lower_component_fn(fn_decl: &FnDecl, known_components: &HashSet<String>) -> Component {
    let body = fn_decl
        .function
        .body
        .as_ref()
        .unwrap_or_else(|| panic!("loomc: component function has no body"));

    let mut signals = Vec::new();
    let mut shared_bindings = Vec::new();
    let mut root = None;

    for stmt in &body.stmts {
        match stmt {
            Stmt::Decl(Decl::Var(var_decl)) => {
                for decl in &var_decl.decls {
                    if let Some((signal, shared)) = try_lower_shared_signal_decl(decl) {
                        signals.push(signal);
                        shared_bindings.push(shared);
                    } else if let Some(signal) = try_lower_signal_decl(decl) {
                        signals.push(signal);
                    } else {
                        panic!("loomc: unsupported `const` declaration in component body (only `const x = signal(...)`/`const x = sharedSignal(\"key\", ...)` is supported): {decl:?}");
                    }
                }
            }
            Stmt::Return(ReturnStmt { arg: Some(arg), .. }) => {
                let jsx = unwrap_paren(arg);
                let Expr::JSXElement(el) = jsx else {
                    panic!("loomc: component must `return` a single JSX element, got: {jsx:?}");
                };
                root = Some(lower_element(el, &signals, None, known_components));
            }
            other => panic!("loomc: unsupported statement in component body: {other:?}"),
        }
    }

    Component {
        name: fn_decl.ident.sym.to_string(),
        signals,
        shared_bindings,
        root: root.unwrap_or_else(|| panic!("loomc: component has no `return` statement")),
    }
}

/// `const cartCount = sharedSignal("cartCount", 0)` — tried *before*
/// `try_lower_signal_decl` (matches the callee name `sharedSignal` instead
/// of `signal`), producing both a normal `SignalDecl` (the writer role
/// reuses every existing local-signal codegen path unchanged) and a
/// `SharedBinding` recording that this signal should also be published to
/// `__loomShared`. `None` (not a panic) if the callee isn't `sharedSignal`
/// at all, so `lower_component_fn` can fall through to the ordinary
/// `signal(...)` case.
fn try_lower_shared_signal_decl(decl: &VarDeclarator) -> Option<(SignalDecl, SharedBinding)> {
    let Pat::Ident(BindingIdent { id, .. }) = &decl.name else {
        return None;
    };
    let init = decl.init.as_deref()?;
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = init
    else {
        return None;
    };
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        return None;
    };
    if sym != "sharedSignal" {
        return None;
    }
    let [key_arg, initial_arg] = args.as_slice() else {
        panic!("loomc: sharedSignal() requires exactly two arguments, e.g. sharedSignal(\"cartCount\", 0)");
    };
    let Expr::Lit(Lit::Str(key)) = unwrap_paren(&key_arg.expr) else {
        panic!("loomc: sharedSignal() requires a string key and an integer initial value, e.g. sharedSignal(\"cartCount\", 0)");
    };
    let Expr::Lit(Lit::Num(Number { value, .. })) = unwrap_paren(&initial_arg.expr) else {
        panic!("loomc: sharedSignal() requires a string key and an integer initial value, e.g. sharedSignal(\"cartCount\", 0)");
    };
    let name = id.sym.to_string();
    let key = key.value.to_string_lossy().into_owned();
    Some((
        SignalDecl {
            name: name.clone(),
            init: SignalInit::I32(*value as i32),
        },
        SharedBinding { signal: name, key },
    ))
}

fn unwrap_paren(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(ParenExpr { expr, .. }) => unwrap_paren(expr),
        other => other,
    }
}

fn try_lower_signal_decl(decl: &VarDeclarator) -> Option<SignalDecl> {
    let Pat::Ident(BindingIdent { id, .. }) = &decl.name else {
        return None;
    };
    let init = decl.init.as_deref()?;
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = init
    else {
        return None;
    };
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        return None;
    };
    if sym != "signal" {
        return None;
    }
    let arg = args
        .first()
        .unwrap_or_else(|| panic!("loomc: signal() requires one argument"));
    let init = match unwrap_paren(&arg.expr) {
        Expr::Lit(Lit::Num(Number { value, .. })) => SignalInit::I32(*value as i32),
        Expr::Lit(Lit::Bool(b)) => SignalInit::Bool(b.value),
        Expr::Lit(Lit::Str(s)) => SignalInit::Str(s.value.to_string_lossy().into_owned()),
        Expr::Array(ArrayLit { elems, .. }) => {
            let items = elems
                .iter()
                .map(|e| {
                    let e = e
                        .as_ref()
                        .unwrap_or_else(|| panic!("loomc: sparse array literals unsupported"));
                    match unwrap_paren(&e.expr) {
                        Expr::Lit(Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                        other => panic!(
                            "loomc: signal([...]) array elements must be string literals, got: {other:?}"
                        ),
                    }
                })
                .collect();
            SignalInit::StringList(items)
        }
        other => panic!(
            "loomc: signal(...) initializer must be a number, boolean, string, or string-array literal, got: {other:?}"
        ),
    };
    Some(SignalDecl {
        name: id.sym.to_string(),
        init,
    })
}

/// `row_var`: `Some(name)` while lowering a list row template, so a bare
/// `{name}` reference resolves to `JsxNode::RowItemText` instead of being
/// looked up as a top-level signal. `known_components`: every component
/// name declared anywhere in this module (root + helpers) — used to decide
/// whether an uppercase-leading tag is a real component reference (and to
/// give a clear error if it names nothing declared) rather than treating
/// every tag as a bare HTML string.
fn lower_element(
    el: &JSXElement,
    signals: &[SignalDecl],
    row_var: Option<&str>,
    known_components: &HashSet<String>,
) -> JsxNode {
    let JSXElementName::Ident(Ident { sym: tag, .. }) = &el.opening.name else {
        panic!("loomc: only plain tag names are supported, got: {:?}", el.opening.name);
    };
    let tag = tag.to_string();

    if tag.chars().next().is_some_and(|c| c.is_uppercase()) {
        if !known_components.contains(&tag) {
            panic!(
                "loomc: `<{tag}/>` looks like a component reference (uppercase-leading name) but no `function {tag}()` is declared in this module"
            );
        }
        return lower_component_ref(el, &tag);
    }

    if tag == "input" {
        return lower_input_binding(el, signals);
    }

    let mut class = None;
    let mut action = None;
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            panic!("loomc: spread attributes are not supported");
        };
        let JSXAttrName::Ident(IdentName { sym: name, .. }) = &attr.name else {
            panic!("loomc: namespaced attributes are not supported");
        };
        match name.as_ref() {
            "class" | "className" => {
                let Some(JSXAttrValue::Str(s)) = &attr.value else {
                    panic!("loomc: `class` must be a static string literal");
                };
                class = Some(s.value.to_string_lossy().into_owned());
            }
            "onClick" => {
                action = Some(lower_click_action(attr, signals));
            }
            other => panic!("loomc: unsupported attribute `{other}` on <{tag}>"),
        }
    }

    let children = el
        .children
        .iter()
        .filter_map(|child| lower_child(child, signals, row_var, known_components))
        .collect();

    JsxNode::Element {
        tag,
        class,
        action,
        children,
    }
}

/// Parses `<ComponentName client:load|client:visible prop={literal} .../>`
/// into `JsxNode::ComponentRef`. This is the **only** place `client:`-
/// namespaced attributes are accepted — plain `Element`/`InputBinding`
/// still reject every namespaced attribute unchanged (their own panics,
/// untouched by this function).
fn lower_component_ref(el: &JSXElement, name: &str) -> JsxNode {
    let mut activation = None;
    let mut props = Vec::new();
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            panic!("loomc: spread attributes are not supported");
        };
        match &attr.name {
            JSXAttrName::JSXNamespacedName(JSXNamespacedName {
                ns, name: attr_name, ..
            }) => {
                if ns.sym.as_ref() != "client" {
                    panic!(
                        "loomc: unsupported attribute namespace `{}:` on <{name}> (only `client:` is supported)",
                        ns.sym
                    );
                }
                activation = Some(match attr_name.sym.as_ref() {
                    "load" => IslandActivation::Load,
                    "visible" => IslandActivation::Visible,
                    other => panic!(
                        "loomc: unsupported `client:{other}` directive on <{name}> (supported: client:load, client:visible)"
                    ),
                });
            }
            JSXAttrName::Ident(IdentName { sym: prop_name, .. }) => {
                let value = attr
                    .value
                    .as_ref()
                    .unwrap_or_else(|| panic!("loomc: prop `{prop_name}` on <{name}> requires a value"));
                props.push((prop_name.to_string(), lower_prop_literal(value)));
            }
        }
    }
    let activation = activation.unwrap_or_else(|| {
        panic!(
            "loomc: <{name}/> has no `client:load`/`client:visible` directive — 증분 4 scope: ordinary (non-Island) component references are not yet supported, every ComponentRef must currently carry an activation"
        )
    });
    JsxNode::ComponentRef {
        component: name.to_string(),
        activation,
        props,
    }
}

/// A prop literal must be exactly one of the same shapes `signal(...)`'s own
/// initializer already allows (see `try_lower_signal_decl`) — Phase 1's "no
/// non-literal initial values anywhere" restriction, extended rather than
/// broken (see `ir::PropLiteral`'s doc comment).
fn lower_prop_literal(value: &JSXAttrValue) -> PropLiteral {
    match value {
        JSXAttrValue::Str(s) => PropLiteral::Str(s.value.to_string_lossy().into_owned()),
        JSXAttrValue::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => match unwrap_paren(expr) {
            Expr::Lit(Lit::Num(Number { value, .. })) => PropLiteral::I32(*value as i32),
            Expr::Lit(Lit::Bool(b)) => PropLiteral::Bool(b.value),
            Expr::Lit(Lit::Str(s)) => PropLiteral::Str(s.value.to_string_lossy().into_owned()),
            Expr::Array(ArrayLit { elems, .. }) => {
                let items = elems
                    .iter()
                    .map(|e| {
                        let e = e
                            .as_ref()
                            .unwrap_or_else(|| panic!("loomc: sparse array literals unsupported"));
                        match unwrap_paren(&e.expr) {
                            Expr::Lit(Lit::Str(s)) => s.value.to_string_lossy().into_owned(),
                            other => panic!("loomc: prop array elements must be string literals, got: {other:?}"),
                        }
                    })
                    .collect();
                PropLiteral::StringList(items)
            }
            other => panic!(
                "loomc: component prop value must be a number, boolean, string, or string-array literal, got: {other:?}"
            ),
        },
        other => panic!("loomc: unsupported component prop value shape: {other:?}"),
    }
}

/// `<input value={s()} onInput={(e) => s.set(e.target.value)}/>` — Phase 1's
/// two-way text binding. Handled as its own `JsxNode` (not a generic
/// `Element`, which has no slot for "this element's value and its update
/// handler are the same binding site") — the same reason `{cond() && <X/>}`
/// got its own `JsxNode::CondBinding` rather than riding on `Element`.
fn lower_input_binding(el: &JSXElement, signals: &[SignalDecl]) -> JsxNode {
    let mut value_signal: Option<String> = None;
    let mut oninput_signal: Option<String> = None;
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            panic!("loomc: spread attributes are not supported");
        };
        let JSXAttrName::Ident(IdentName { sym: name, .. }) = &attr.name else {
            panic!("loomc: namespaced attributes are not supported");
        };
        match name.as_ref() {
            "value" => value_signal = Some(lower_input_value_attr(attr, signals)),
            "onInput" => oninput_signal = Some(lower_input_oninput_attr(attr, signals)),
            other => panic!(
                "loomc: <input> only supports `value`/`onInput` (Phase 1 scope: two-way text binding only), got `{other}`"
            ),
        }
    }
    let value_signal =
        value_signal.unwrap_or_else(|| panic!("loomc: <input> requires a `value={{signalName()}}` attribute"));
    let oninput_signal = oninput_signal.unwrap_or_else(|| {
        panic!("loomc: <input> requires an `onInput={{(e) => signalName.set(e.target.value)}}` attribute")
    });
    assert_eq!(
        value_signal, oninput_signal,
        "loomc: <input>'s `value` and `onInput` must reference the same signal, got `{value_signal}` and `{oninput_signal}`"
    );
    JsxNode::InputBinding { signal: value_signal }
}

fn lower_input_value_attr(attr: &JSXAttr, signals: &[SignalDecl]) -> String {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        panic!("loomc: <input>'s `value` must be `{{signalName()}}`");
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = unwrap_paren(expr)
    else {
        panic!("loomc: <input>'s `value` must be a bare `signalName()` call");
    };
    if !args.is_empty() {
        panic!("loomc: `signalName()` in <input>'s `value` must take no arguments");
    }
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        panic!("loomc: <input>'s `value` must call a bare signal identifier");
    };
    let name = sym.to_string();
    let signal = signals
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("loomc: <input>'s `value` references unknown signal `{name}`"));
    assert!(
        matches!(signal.init, SignalInit::Str(_)),
        "loomc: <input>'s `value` requires a Str signal, `{name}` is not one"
    );
    name
}

fn lower_input_oninput_attr(attr: &JSXAttr, signals: &[SignalDecl]) -> String {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        panic!("loomc: <input>'s `onInput` must be `{{(e) => ...}}`");
    };
    let Expr::Arrow(ArrowExpr { body, params, .. }) = unwrap_paren(expr) else {
        panic!("loomc: <input>'s `onInput` must be an arrow function");
    };
    let [Pat::Ident(BindingIdent { id: event_param, .. })] = params.as_slice() else {
        panic!("loomc: <input>'s `onInput` arrow must take exactly one parameter, e.g. `(e) => ...`");
    };
    let event_name = event_param.sym.to_string();
    let BlockStmtOrExpr::Expr(body_expr) = body.as_ref() else {
        panic!("loomc: <input>'s `onInput` arrow body must be a single expression `(e) => signal.set(e.target.value)` (not a block `{{ ... }}`)");
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = unwrap_paren(body_expr)
    else {
        panic!("loomc: <input>'s `onInput` body must be a single `signal.set(...)` call");
    };
    let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
        panic!("loomc: <input>'s `onInput` body must call a method on a signal, e.g. `signal.set(...)`");
    };
    let Expr::Ident(Ident { sym: signal_name, .. }) = obj.as_ref() else {
        panic!("loomc: <input>'s `onInput` target must be a bare signal identifier");
    };
    let signal_name = signal_name.to_string();
    let MemberProp::Ident(IdentName { sym: method, .. }) = prop else {
        panic!("loomc: unsupported method reference in <input>'s `onInput`");
    };
    if method != "set" {
        panic!("loomc: <input>'s `onInput` must call `.set(...)`, got `.{method}(...)`");
    }
    let arg = args
        .first()
        .unwrap_or_else(|| panic!("loomc: `.set(...)` requires one argument, `e.target.value`"));
    let Expr::Member(MemberExpr {
        obj: target_obj,
        prop: value_prop,
        ..
    }) = unwrap_paren(&arg.expr)
    else {
        panic!("loomc: <input>'s `.set(...)` argument must be `{event_name}.target.value`");
    };
    let MemberProp::Ident(IdentName { sym: value_sym, .. }) = value_prop else {
        panic!("loomc: <input>'s `.set(...)` argument must be `{event_name}.target.value`");
    };
    if value_sym != "value" {
        panic!("loomc: <input>'s `.set(...)` argument must end in `.value`, got `.{value_sym}`");
    }
    let Expr::Member(MemberExpr {
        obj: event_obj,
        prop: target_prop,
        ..
    }) = target_obj.as_ref()
    else {
        panic!("loomc: <input>'s `.set(...)` argument must be `{event_name}.target.value`");
    };
    let MemberProp::Ident(IdentName { sym: target_sym, .. }) = target_prop else {
        panic!("loomc: <input>'s `.set(...)` argument must be `{event_name}.target.value`");
    };
    if target_sym != "target" {
        panic!("loomc: <input>'s `.set(...)` argument must be `{event_name}.target.value`, got `.{target_sym}` instead of `.target`");
    }
    let Expr::Ident(Ident { sym: event_ref, .. }) = event_obj.as_ref() else {
        panic!("loomc: <input>'s `.set(...)` argument must reference the arrow's own event parameter `{event_name}`");
    };
    if event_ref.as_ref() != event_name {
        panic!(
            "loomc: <input>'s `.set(...)` argument must reference the arrow's own event parameter `{event_name}`, got `{event_ref}`"
        );
    }
    let signal = signals
        .iter()
        .find(|s| s.name == signal_name)
        .unwrap_or_else(|| panic!("loomc: <input>'s `onInput` references unknown signal `{signal_name}`"));
    assert!(
        matches!(signal.init, SignalInit::Str(_)),
        "loomc: <input>'s `onInput` requires a Str signal, `{signal_name}` is not one"
    );
    signal_name
}

fn lower_click_action(attr: &JSXAttr, signals: &[SignalDecl]) -> ClickAction {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        panic!("loomc: `onClick` must be `{{() => ...}}`");
    };
    let Expr::Arrow(ArrowExpr { body, params, .. }) = unwrap_paren(expr) else {
        panic!("loomc: `onClick` must be an arrow function");
    };
    if !params.is_empty() {
        panic!("loomc: `onClick` arrow function must take no parameters");
    }
    let BlockStmtOrExpr::Expr(body_expr) = body.as_ref() else {
        panic!("loomc: `onClick` arrow body must be a single expression, e.g. `() => x.increment()` (not a block `{{ ... }}`)");
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = unwrap_paren(body_expr)
    else {
        panic!("loomc: `onClick` body must be a single method call like `signal.increment()`");
    };
    let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
        panic!("loomc: `onClick` body must call a method on a signal, e.g. `signal.increment()`");
    };
    let Expr::Ident(Ident { sym: signal_name, .. }) = obj.as_ref() else {
        panic!("loomc: `onClick` target must be a bare signal identifier");
    };
    let signal_name = signal_name.to_string();
    let signal = signals
        .iter()
        .find(|s| s.name == signal_name)
        .unwrap_or_else(|| panic!("loomc: `onClick` references unknown signal `{signal_name}`"));
    let MemberProp::Ident(IdentName { sym: method, .. }) = prop else {
        panic!("loomc: unsupported method reference in `onClick`");
    };

    match method.as_ref() {
        "increment" => {
            assert!(
                matches!(signal.init, SignalInit::I32(_)),
                "loomc: `.increment()` requires an I32 signal, `{signal_name}` is not one"
            );
            ClickAction::Increment { signal: signal_name }
        }
        "rotate" => {
            assert!(
                matches!(signal.init, SignalInit::StringList(_)),
                "loomc: `.rotate()` requires a string-list signal, `{signal_name}` is not one"
            );
            ClickAction::Rotate { signal: signal_name }
        }
        "append" => {
            assert!(
                matches!(signal.init, SignalInit::StringList(_)),
                "loomc: `.append(...)` requires a string-list signal, `{signal_name}` is not one"
            );
            let arg = args
                .first()
                .unwrap_or_else(|| panic!("loomc: `.append(...)` requires one string-literal argument"));
            let Expr::Lit(Lit::Str(s)) = unwrap_paren(&arg.expr) else {
                panic!("loomc: `.append(...)` argument must be a string literal (Phase 1 scope: no dynamic expressions)");
            };
            ClickAction::Append {
                signal: signal_name,
                literal: s.value.to_string_lossy().into_owned(),
            }
        }
        "toggle" => {
            assert!(
                matches!(signal.init, SignalInit::Bool(_)),
                "loomc: `.toggle()` requires a Bool signal, `{signal_name}` is not one"
            );
            ClickAction::Toggle { signal: signal_name }
        }
        other => panic!("loomc: unsupported signal method `.{other}()` (supported: increment, append, rotate, toggle)"),
    }
}

fn lower_child(
    child: &JSXElementChild,
    signals: &[SignalDecl],
    row_var: Option<&str>,
    known_components: &HashSet<String>,
) -> Option<JsxNode> {
    match child {
        JSXElementChild::JSXText(text) => {
            let trimmed = text.value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(JsxNode::StaticText(trimmed.to_string()))
            }
        }
        JSXElementChild::JSXElement(el) => Some(lower_element(el, signals, row_var, known_components)),
        JSXElementChild::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => Some(lower_expr_container(expr, signals, row_var, known_components)),
        other => panic!("loomc: unsupported JSX child: {other:?}"),
    }
}

fn lower_expr_container(
    expr: &Expr,
    signals: &[SignalDecl],
    row_var: Option<&str>,
    known_components: &HashSet<String>,
) -> JsxNode {
    match unwrap_paren(expr) {
        // `{item}` inside a list row template.
        Expr::Ident(Ident { sym, .. }) if row_var == Some(sym.as_ref()) => JsxNode::RowItemText,

        // `{sharedValue("cartCount", 0)}` — read-only cross-widget display,
        // no local signal lookup at all (unlike every other binding here) —
        // that absence is itself the point (see `JsxNode::SharedValueBinding`'s
        // doc comment).
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            ..
        }) if matches!(callee.as_ref(), Expr::Ident(Ident { sym, .. }) if sym == "sharedValue") => {
            let [key_arg, initial_arg] = args.as_slice() else {
                panic!("loomc: sharedValue() requires exactly two arguments, e.g. sharedValue(\"cartCount\", 0)");
            };
            let Expr::Lit(Lit::Str(key)) = unwrap_paren(&key_arg.expr) else {
                panic!("loomc: sharedValue() requires a string key and an integer initial value, e.g. sharedValue(\"cartCount\", 0)");
            };
            let Expr::Lit(Lit::Num(Number { value, .. })) = unwrap_paren(&initial_arg.expr) else {
                panic!("loomc: sharedValue() requires a string key and an integer initial value, e.g. sharedValue(\"cartCount\", 0)");
            };
            JsxNode::SharedValueBinding {
                key: key.value.to_string_lossy().into_owned(),
                initial: *value as i32,
            }
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
                    let row_var = row_var.unwrap_or_else(|| {
                        panic!("loomc: `a() && b && <X/>` 형태의 가운데 항은 row 템플릿 안에서만 쓸 수 있다 — 바깥에서는 조건 signal 하나만 받는다")
                    });
                    (inner_left.as_ref(), Some(parse_row_predicate(inner_right, row_var)))
                }
                other => (other, None),
            };
            let Expr::Call(CallExpr {
                callee: Callee::Expr(callee),
                args,
                ..
            }) = unwrap_paren(left)
            else {
                panic!("loomc: conditional's left side must be a bare `signalName()` boolean read");
            };
            if !args.is_empty() {
                panic!("loomc: `signalName()` in a conditional must take no arguments");
            }
            let Expr::Ident(Ident { sym: cond_name, .. }) = callee.as_ref() else {
                panic!("loomc: conditional's left side must be a bare signal identifier call");
            };
            let cond_name = cond_name.to_string();
            let signal = signals
                .iter()
                .find(|s| s.name == cond_name)
                .unwrap_or_else(|| panic!("loomc: `{{{cond_name}() && ...}}` references unknown signal `{cond_name}`"));
            assert!(
                matches!(signal.init, SignalInit::Bool(_)),
                "loomc: `&&`-conditional requires a Bool signal, `{cond_name}` is not one"
            );

            let Expr::JSXElement(then_el) = unwrap_paren(right) else {
                panic!("loomc: conditional's right side must be a single JSX element");
            };
            let then_branch = lower_element(then_el, signals, row_var, known_components);
            let JsxNode::Element { children, .. } = &then_branch else {
                unreachable!()
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
            for child in children {
                if matches!(child, JsxNode::CondBinding { .. }) {
                    panic!(
                        "loomc: `{{{cond_name}() && ...}}` 안에 또 조건부를 넣는 것은 아직 지원하지 않는다 — 안쪽 사이트도 자기 marker/전용 함수/`CondNested`가 필요한데, 그 중첩 생성 경로가 아직 없다(codegen.rs의 `CondBinding` arm 참고)"
                    );
                }
            }
            JsxNode::CondBinding {
                cond: cond_name,
                row_predicate,
                then_branch: Box::new(then_branch),
            }
        }

        // `{signalName()}` — a bare zero-arg call to a known signal.
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            ..
        }) if args.is_empty() && matches!(callee.as_ref(), Expr::Ident(_)) => {
            let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
                unreachable!()
            };
            let name = sym.to_string();
            if signals.iter().any(|s| s.name == name) {
                JsxNode::TextBinding { signal: name }
            } else {
                panic!("loomc: `{{{name}()}}` does not reference a declared signal");
            }
        }

        // `{signalName().map((row) => <el>{row}</el>)}`
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            ..
        }) => {
            let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
                panic!("loomc: unsupported dynamic expression in JSX (only `signal()`, or `signal().map(...)`, are supported)");
            };
            let MemberProp::Ident(IdentName { sym: method, .. }) = prop else {
                panic!("loomc: unsupported member call in JSX");
            };
            if method != "map" {
                panic!("loomc: only `.map(...)` is supported on a list signal read, got `.{method}(...)`");
            }
            // `xs().slice(0, item.length).map(...)` — row 변수로 좁힌 리스트
            // (2026-08-07). `.slice`가 끼어 있으면 한 겹 벗기고 그 사실을 기록한다.
            let (obj, row_slice) = match obj.as_ref() {
                Expr::Call(CallExpr { callee: Callee::Expr(c), args: slice_args, .. })
                    if matches!(c.as_ref(), Expr::Member(MemberExpr { prop: MemberProp::Ident(IdentName { sym, .. }), .. }) if sym == "slice") =>
                {
                    let Expr::Member(MemberExpr { obj: slice_obj, .. }) = c.as_ref() else {
                        unreachable!()
                    };
                    let row_var = row_var.unwrap_or_else(|| {
                        panic!("loomc: `.slice(...)`는 row 템플릿 안에서만 쓸 수 있다 — row 변수로 좁히는 것이 유일한 용도다")
                    });
                    parse_row_slice(slice_args, row_var);
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
                panic!("loomc: `.map(...)` must be called directly on `signalName()`");
            };
            if !inner_args.is_empty() {
                panic!("loomc: `signalName()` must be called with no arguments before `.map(...)`");
            }
            let Expr::Ident(Ident { sym: signal_name, .. }) = inner_callee.as_ref() else {
                panic!("loomc: `.map(...)` must be called on a bare `signalName()`");
            };
            let signal_name = signal_name.to_string();
            let signal = signals
                .iter()
                .find(|s| s.name == signal_name)
                .unwrap_or_else(|| panic!("loomc: `.map(...)` on unknown signal `{signal_name}`"));
            assert!(
                matches!(signal.init, SignalInit::StringList(_)),
                "loomc: `.map(...)` requires a string-list signal, `{signal_name}` is not one"
            );

            let map_arg = args
                .first()
                .unwrap_or_else(|| panic!("loomc: `.map(...)` requires one arrow-function argument"));
            let Expr::Arrow(ArrowExpr {
                params,
                body: row_body,
                ..
            }) = unwrap_paren(&map_arg.expr)
            else {
                panic!("loomc: `.map(...)` argument must be an arrow function");
            };
            let [Pat::Ident(BindingIdent { id: row_ident, .. })] = params.as_slice() else {
                panic!("loomc: `.map((row) => ...)` must take exactly one bare identifier parameter");
            };
            let row_var_name = row_ident.sym.to_string();
            let BlockStmtOrExpr::Expr(row_expr) = row_body.as_ref() else {
                panic!("loomc: `.map(...)` row template must be a single JSX expression, not a block");
            };
            let Expr::JSXElement(row_el) = unwrap_paren(row_expr) else {
                panic!("loomc: `.map(...)` row template must be a JSX element");
            };
            let row = lower_element(row_el, signals, Some(&row_var_name), known_components);

            JsxNode::ListBinding {
                signal: signal_name,
                row_var: row_var_name,
                row: Box::new(row),
                row_slice,
            }
        }
        other => panic!("loomc: unsupported JSX expression: {other:?}"),
    }
}

/// `item.length > n` 한 형태만 받는다. 표현식 언어를 여는 것이 목적이 아니라,
/// **row마다 구조가 달라지는 픽스처**를 컴파일러가 낼 수 있게 하는 것이 목적이다
/// (RQ1 §5 트랙 N — 구간 크기가 row마다 같으면 못 잡는 결함이 있다).
fn parse_row_predicate(expr: &Expr, row_var: &str) -> RowPredicate {
    let Expr::Bin(BinExpr { op: BinaryOp::Gt, left, right, .. }) = unwrap_paren(expr) else {
        panic!("loomc: row 술어는 `{row_var}.length > <정수>` 형태여야 한다 — 받은 것: {expr:?}");
    };
    let Expr::Member(MemberExpr { obj, prop: MemberProp::Ident(IdentName { sym, .. }), .. }) =
        unwrap_paren(left)
    else {
        panic!("loomc: row 술어의 왼쪽은 `{row_var}.length` 여야 한다");
    };
    assert!(sym == "length", "loomc: row 술어가 지원하는 속성은 `.length` 뿐이다 (`.{sym}` 받음)");
    let Expr::Ident(Ident { sym: obj_name, .. }) = obj.as_ref() else {
        panic!("loomc: row 술어의 왼쪽은 row 변수여야 한다");
    };
    assert!(
        obj_name == row_var,
        "loomc: row 술어는 이 row의 변수(`{row_var}`)를 써야 한다 — `{obj_name}` 받음"
    );
    let Expr::Lit(Lit::Num(Number { value, .. })) = unwrap_paren(right) else {
        panic!("loomc: row 술어의 오른쪽은 정수 리터럴이어야 한다");
    };
    RowPredicate::LenGt(*value as usize)
}

/// `.slice(0, item.length)` 한 형태만. 인자 모양을 실제로 검사한다 —
/// 통과시켜 놓고 다른 의미로 코드를 내면 저작 표면과 산출물이 조용히 어긋난다.
fn parse_row_slice(args: &[ExprOrSpread], row_var: &str) {
    let [start, end] = args else {
        panic!("loomc: `.slice(...)`는 `slice(0, {row_var}.length)` 형태여야 한다 (인자 2개)");
    };
    let Expr::Lit(Lit::Num(Number { value: 0.0, .. })) = unwrap_paren(&start.expr) else {
        panic!("loomc: `.slice(...)`의 첫 인자는 `0` 이어야 한다");
    };
    let Expr::Member(MemberExpr { obj, prop: MemberProp::Ident(IdentName { sym, .. }), .. }) =
        unwrap_paren(&end.expr)
    else {
        panic!("loomc: `.slice(...)`의 둘째 인자는 `{row_var}.length` 여야 한다");
    };
    assert!(sym == "length", "loomc: `.slice(...)`의 둘째 인자는 `.length` 여야 한다");
    let Expr::Ident(Ident { sym: obj_name, .. }) = obj.as_ref() else {
        panic!("loomc: `.slice(...)`의 둘째 인자는 row 변수여야 한다");
    };
    assert!(
        obj_name == row_var,
        "loomc: `.slice(...)`는 이 row의 변수(`{row_var}`)를 써야 한다 — `{obj_name}` 받음"
    );
}
