//! TSX (real SWC AST, parsed by `swc_ecma_parser` — no hand-rolled parser,
//! per docs/02 "왜 직접 파서를 안 짜는가") → `ir::Component`.
//!
//! Supported subset (Phase 1 prototype scope, documented here rather than
//! silently assumed):
//!   - exactly one exported `function NAME() { ... }` per module
//!   - `const NAME = signal(NUMBER)` or `const NAME = signal([STRING, ...])`
//!   - a single JSX return, containing:
//!     - static tags (`div`/`span`/`ul`/`li`/`button`) with an optional
//!       static `class` attribute
//!     - static text children
//!     - `{signal()}` text bindings
//!     - `{signal().map((row) => <tag>{row}</tag>)}` list bindings, exactly
//!       one text-child row shape
//!     - `onClick={() => signal.increment()|.append("lit")|.rotate()}`
//! Anything outside this is a compile error (`panic!` with a clear message)
//! — this is a real, narrow compiler front end, not a fuzzy best-effort one.

use crate::ir::{ClickAction, Component, JsxNode, SignalDecl, SignalInit};
use swc_ecma_ast::*;

pub fn lower_module(module: &Module) -> Component {
    let fn_decl = find_exported_component_fn(module)
        .unwrap_or_else(|| panic!("loomc: no exported `function NAME() {{ ... }}` found"));

    let body = fn_decl
        .function
        .body
        .as_ref()
        .unwrap_or_else(|| panic!("loomc: component function has no body"));

    let mut signals = Vec::new();
    let mut root = None;

    for stmt in &body.stmts {
        match stmt {
            Stmt::Decl(Decl::Var(var_decl)) => {
                for decl in &var_decl.decls {
                    if let Some(signal) = try_lower_signal_decl(decl) {
                        signals.push(signal);
                    } else {
                        panic!("loomc: unsupported `const` declaration in component body (only `const x = signal(...)` is supported): {decl:?}");
                    }
                }
            }
            Stmt::Return(ReturnStmt { arg: Some(arg), .. }) => {
                let jsx = unwrap_paren(arg);
                let Expr::JSXElement(el) = jsx else {
                    panic!("loomc: component must `return` a single JSX element, got: {jsx:?}");
                };
                root = Some(lower_element(el, &signals, None));
            }
            other => panic!("loomc: unsupported statement in component body: {other:?}"),
        }
    }

    Component {
        name: fn_decl.ident.sym.to_string(),
        signals,
        root: root.unwrap_or_else(|| panic!("loomc: component has no `return` statement")),
    }
}

fn find_exported_component_fn(module: &Module) -> Option<FnDecl> {
    for item in &module.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl {
            decl: Decl::Fn(fn_decl),
            ..
        })) = item
        {
            return Some(fn_decl.clone());
        }
    }
    None
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
            "loomc: signal(...) initializer must be a number or string-array literal, got: {other:?}"
        ),
    };
    Some(SignalDecl {
        name: id.sym.to_string(),
        init,
    })
}

/// `row_var`: `Some(name)` while lowering a list row template, so a bare
/// `{name}` reference resolves to `JsxNode::RowItemText` instead of being
/// looked up as a top-level signal.
fn lower_element(el: &JSXElement, signals: &[SignalDecl], row_var: Option<&str>) -> JsxNode {
    let JSXElementName::Ident(Ident { sym: tag, .. }) = &el.opening.name else {
        panic!("loomc: only plain tag names are supported, got: {:?}", el.opening.name);
    };
    let tag = tag.to_string();

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
        .filter_map(|child| lower_child(child, signals, row_var))
        .collect();

    JsxNode::Element {
        tag,
        class,
        action,
        children,
    }
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
        other => panic!("loomc: unsupported signal method `.{other}()` (supported: increment, append, rotate)"),
    }
}

fn lower_child(
    child: &JSXElementChild,
    signals: &[SignalDecl],
    row_var: Option<&str>,
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
        JSXElementChild::JSXElement(el) => Some(lower_element(el, signals, row_var)),
        JSXElementChild::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => Some(lower_expr_container(expr, signals, row_var)),
        other => panic!("loomc: unsupported JSX child: {other:?}"),
    }
}

fn lower_expr_container(expr: &Expr, signals: &[SignalDecl], row_var: Option<&str>) -> JsxNode {
    match unwrap_paren(expr) {
        // `{item}` inside a list row template.
        Expr::Ident(Ident { sym, .. }) if row_var == Some(sym.as_ref()) => JsxNode::RowItemText,

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
            let Expr::Call(CallExpr {
                callee: Callee::Expr(inner_callee),
                args: inner_args,
                ..
            }) = obj.as_ref()
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
            let row = lower_element(row_el, signals, Some(&row_var_name));

            JsxNode::ListBinding {
                signal: signal_name,
                row_var: row_var_name,
                row: Box::new(row),
            }
        }
        other => panic!("loomc: unsupported JSX expression: {other:?}"),
    }
}
