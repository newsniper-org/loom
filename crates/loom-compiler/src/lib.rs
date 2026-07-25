mod codegen;
mod ir;
mod lower;

pub use codegen::GraphMode;

use swc_common::sync::Lrc;
use swc_common::{FileName, SourceFile, SourceMap};
use swc_ecma_ast::Module;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

pub fn parse_tsx(source: &str) -> Module {
    let cm: Lrc<SourceMap> = Default::default();
    let fm: Lrc<SourceFile> =
        cm.new_source_file(Lrc::new(FileName::Custom("input.tsx".into())), source.to_string());
    let syntax = Syntax::Typescript(TsSyntax {
        tsx: true,
        ..Default::default()
    });
    let lexer = Lexer::new(syntax, Default::default(), StringInput::from(&*fm), None);
    let mut parser = Parser::new_from(lexer);
    parser
        .parse_module()
        .unwrap_or_else(|e| panic!("loomc: parse error: {e:?}"))
}

/// docs/02's whole compile pipeline in one call: real SWC parse → IR lower
/// → Rust codegen. Single-component only (compiles just the module's one
/// exported root component, ignoring any Island-referenced helper
/// components — see `compile_tsx_to_rust_graph` for the 증분 4 multi-
/// component path) — kept exactly as-is for backward compatibility with
/// every existing `build.rs` call site (`loom-demo`, `loom-resume-demo`,
/// `loom-fic`, `loom-bench`), none of which need to change. See `lower.rs`
/// for the exact (intentionally narrow) authoring-surface subset this
/// supports.
pub fn compile_tsx_to_rust(source: &str) -> String {
    let module = parse_tsx(source);
    let graph = lower::lower_module(&module);
    codegen::generate(graph.root_component())
}

/// 증분 4 (Task #25) multi-component path — compiles the module's full
/// `ComponentGraph` (root + every Island-referenced helper component) in the
/// given `GraphMode`. See `codegen::GraphMode`'s doc comment for the two
/// modes' shapes.
pub fn compile_tsx_to_rust_graph(source: &str, mode: GraphMode) -> String {
    let module = parse_tsx(source);
    let graph = lower::lower_module(&module);
    codegen::generate_graph(&graph, mode)
}

/// RQ6 §6 M1 real per-Island code splitting (Task #27) — parses+lowers
/// `source` into its full `ComponentGraph`, then generates ONLY
/// `island_component_name`'s own client-side code, suitable for a
/// standalone wasm crate whose compiled binary contains exactly that one
/// Island's `#[wasm_bindgen]`-exported symbols — unlike
/// `compile_tsx_to_rust_graph(..., GraphMode::Islands)`, which always
/// emits every component in the graph into one artifact (see
/// `codegen::generate_single_island_client`'s doc comment for the gap this
/// closes and why it matters for RQ6).
pub fn compile_tsx_to_single_island_rust(source: &str, island_component_name: &str) -> String {
    let module = parse_tsx(source);
    let graph = lower::lower_module(&module);
    codegen::generate_single_island_client(&graph, island_component_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEMO_SOURCE: &str = r#"
        import { signal } from "loom";

        export function App() {
          const count = signal(0);
          const items = signal(["a", "b", "c"]);

          return (
            <div>
              <span class="label">count:</span>
              <span class="value">{count()}</span>
              <button onClick={() => count.increment()}>+1</button>
              <ul>
                {items().map((item) => (
                  <li>{item}</li>
                ))}
              </ul>
              <button onClick={() => items.append("new")}>append</button>
              <button onClick={() => items.rotate()}>rotate</button>
            </div>
          );
        }
    "#;

    #[test]
    fn lowers_demo_source_without_panicking() {
        let module = parse_tsx(DEMO_SOURCE);
        let graph = lower::lower_module(&module);
        let component = graph.root_component();
        assert_eq!(component.name, "App");
        assert_eq!(component.signals.len(), 2);
    }

    #[test]
    fn generates_rust_source_with_expected_shape() {
        let rust = compile_tsx_to_rust(DEMO_SOURCE);
        assert!(rust.contains("pub struct Signals"));
        assert!(rust.contains("count: ::bevy_ecs::entity::Entity"));
        assert!(rust.contains("items: ::bevy_ecs::entity::Entity"));
        assert!(rust.contains("pub fn setup("));
        assert!(rust.contains("pub fn apply_action("));
        assert!(rust.contains("list_reconcile_k_system::<String>"));
        assert!(rust.contains("patch_text_system::<i32>"));
        // The `<span class="label">count:</span>` wrapper is fully static
        // (no action, no dynamic binding inside it) -> collapsed into one
        // `append_static_html` call at codegen time (see codegen.rs's
        // `try_render_fully_static`), not the old node-by-node
        // create_element+append_static_text pair.
        assert!(rust.contains(r#"dom.append_static_html(node_0, "<span class=\"label\">count:</span>")"#));
        // A button's own text ("+1") is still a bare StaticText child of an
        // *action-bearing* element (not itself fully static, since it has
        // an onClick) -> stays on the original append_static_text path.
        assert!(rust.contains(r#"dom.append_static_text(node_2, "+1")"#));
        // 3 buttons -> 3 actions collected in source order.
        assert!(rust.contains("Action0"));
        assert!(rust.contains("Action1(String)"));
        assert!(rust.contains("Action2"));
    }

    mod static_html_collapse {
        use super::*;

        #[test]
        fn nested_fully_static_subtree_collapses_into_one_call() {
            let source = r#"
                export function App() {
                  const count = signal(0);
                  return (
                    <div>
                      <header class="top"><h1>Title</h1><p>subtitle text</p></header>
                      <span>{count()}</span>
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            assert!(rust.contains(
                r#"dom.append_static_html(node_0, "<header class=\"top\"><h1>Title</h1><p>subtitle text</p></header>")"#
            ));
            // Only one call for the whole 3-element subtree, not three.
            assert_eq!(rust.matches("dom.create_element").count(), 2, "only the outer <div> and the dynamic <span> should call create_element, got: {rust}");
        }

        #[test]
        fn element_with_an_action_is_not_collapsed_even_if_its_own_children_are_static() {
            let source = r#"
                export function App() {
                  const count = signal(0);
                  return (
                    <div>
                      <span>{count()}</span>
                      <button onClick={() => count.increment()}>+1</button>
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            assert!(
                !rust.contains("append_static_html"),
                "an action-bearing element must never be collapsed (it needs its own NodeHandle for the LRef), got: {rust}"
            );
            assert!(rust.contains(r#"dom.append_static_text(node_2, "+1")"#));
        }

        #[test]
        fn element_wrapping_a_nested_dynamic_binding_is_not_collapsed() {
            // `<p>text <span>{count()}</span></p>` is a *different* shape
            // from `rejects_mixed_static_and_dynamic_children`'s case (a
            // bare `{binding}` sitting directly alongside other children in
            // the *same* element) — here the binding is one level deeper,
            // wrapped in its own `<span>`, which Phase 1 already allows (the
            // "mixes a dynamic binding" check only looks at an element's
            // *direct* children). This confirms `try_render_fully_static`
            // still correctly refuses to collapse `<p>` here (it must not
            // silently flatten away the nested `<span>`'s own binding into
            // dead static text) — it falls through to the normal per-node
            // path instead, so both the static text run and the `<span>`'s
            // own binding remain individually addressable.
            let source = r#"
                export function App() {
                  const count = signal(0);
                  return (
                    <div>
                      <p>count is <span>{count()}</span></p>
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            assert!(
                !rust.contains(r#"append_static_html(node_1, "<p>"#),
                "the <p> wrapping a nested dynamic binding must not be collapsed, got: {rust}"
            );
            assert!(rust.contains(r#"dom.append_static_text(node_1, "count is")"#));
            assert!(rust.contains(r#"data-loom-id", "count""#));
        }
    }

    #[test]
    fn lref_captures_are_narrowed_to_the_one_signal_each_action_needs() {
        // docs/02's "LRef 최소 캡처 집합" — Action0 (Increment on `count`)
        // must capture `count`; Action1/Action2 (Append/Rotate on `items`, a
        // StringList) must capture *nothing*, not `count` too, even though
        // `count` exists elsewhere in the same component.
        let rust = compile_tsx_to_rust(DEMO_SOURCE);
        assert!(
            rust.contains(r#""App_chunk|Action0|count:0""#),
            "Increment's LRef must capture count:0"
        );
        assert!(
            rust.contains(r#""App_chunk|Action1|""#),
            "Append's LRef must capture nothing — it doesn't read/write count, \
             and list resume recovers `items` from the DOM instead of a capture"
        );
        assert!(
            rust.contains(r#""App_chunk|Action2|""#),
            "Rotate's LRef must capture nothing, for the same reason as Append"
        );
    }

    #[test]
    #[should_panic(expected = "mixes a dynamic binding")]
    fn rejects_mixed_static_and_dynamic_children() {
        let source = r#"
            export function App() {
              const count = signal(0);
              return <span>count: {count()}</span>;
            }
        "#;
        let module = parse_tsx(source);
        let graph = lower::lower_module(&module);
        codegen::generate(graph.root_component());
    }

    mod conditional {
        use super::*;

        const COND_SOURCE: &str = r#"
            import { signal } from "loom";

            export function App() {
              const details = signal(false);

              return (
                <div>
                  <button onClick={() => details.toggle()}>toggle</button>
                  {details() && <p class="details">shown!</p>}
                </div>
              );
            }
        "#;

        #[test]
        fn generates_a_wrapper_a_per_site_fn_and_registers_it_in_setup_and_resume() {
            let rust = compile_tsx_to_rust(COND_SOURCE);
            // Wrapper created eagerly with the then_branch's own tag/class,
            // tagged for resume lookup — starts empty (hidden) since
            // `details` initializes to `false`.
            assert!(rust.contains(r#"dom.create_element(node_0, "p", Some("details"))"#));
            assert!(rust.contains(r#""data-loom-id", "details")"#));
            assert!(rust.contains("::loom_runtime::Signal(false)"));
            assert!(rust.contains("::loom_runtime::CondBlock::with_size(1)"));
            // Per-site function replicates F's control flow with this
            // site's actual static content inlined, not a generic System.
            assert!(rust.contains("fn cond_reconcile_site_0("));
            assert!(rust.contains(r#"text: "shown!".to_string()"#));
            assert!(rust.contains("::loom_runtime::DomWrite::SetVisible"));
            // Registered in both the live path and the resume path — same
            // function, not two different mechanisms.
            let setup_registration_count = rust.matches("app.add_systems(cond_reconcile_site_0)").count();
            assert_eq!(
                setup_registration_count, 2,
                "cond_reconcile_site_0 must be registered once in setup() and once in resume()"
            );
            // Resume recovers shown/hidden from the DOM itself, not a capture.
            assert!(rust.contains("let details_adopted"));
            assert!(rust.contains("let details_shown = !details_adopted.is_empty();"));
        }

        #[test]
        fn multiple_conditionals_in_one_component_get_distinct_site_markers() {
            // Regression test for a real bug found while building Task
            // #26's CheckoutForm fixture (3 conditionals in one
            // component): every `cond_reconcile_site_N` used to share the
            // exact same query shape (Signal<bool>+CondBlock+DomTarget, no
            // discriminating marker) — since `Changed<Signal<bool>>` is
            // tracked per-entity (not per-System), toggling ANY one of the
            // conditionals made EVERY site's reconcile system match that
            // same entity, cross-wiring each site's own hardcoded static
            // text onto whichever entity happened to match. Each site must
            // get its own `CondSiteN` marker component + a `With<CondSiteN>`
            // filter so its system only ever touches its own entity.
            let source = r#"
                export function App() {
                  const shipping = signal(false);
                  const gift = signal(false);
                  return (
                    <div>
                      <button onClick={() => shipping.toggle()}>toggle shipping</button>
                      {shipping() && <p>Express shipping selected.</p>}
                      <button onClick={() => gift.toggle()}>toggle gift</button>
                      {gift() && <p>Gift wrap selected.</p>}
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            assert!(rust.contains("struct CondSite0;"), "{rust}");
            assert!(rust.contains("struct CondSite1;"), "{rust}");
            assert!(
                rust.contains("::bevy_ecs::prelude::With<CondSite0>") && rust.contains("::bevy_ecs::prelude::With<CondSite1>"),
                "{rust}"
            );
            // Each site's spawn (in setup() AND resume()) must carry its
            // OWN marker.
            assert_eq!(rust.matches(", CondSite0)).id();").count(), 2, "{rust}"); // setup() + resume()
            assert_eq!(rust.matches(", CondSite1)).id();").count(), 2, "{rust}");
        }

        #[test]
        fn toggle_action_gets_an_empty_lref_capture() {
            // Bool signals are DOM-recovered (see generate_resume's Bool
            // arm) — the toggle action must not carry a stale-initial-value
            // capture the way an I32 action does.
            let rust = compile_tsx_to_rust(COND_SOURCE);
            assert!(
                rust.contains(r#""App_chunk|Action0|""#),
                "toggle's LRef must capture nothing, got: {rust}"
            );
        }

        #[test]
        #[should_panic(expected = "requires a Bool signal")]
        fn rejects_toggle_on_a_non_bool_signal() {
            let source = r#"
                export function App() {
                  const count = signal(0);
                  return <button onClick={() => count.toggle()}>toggle</button>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "conditional's left side must be a bare")]
        fn rejects_a_non_bare_signal_call_on_the_left_of_and() {
            let source = r#"
                export function App() {
                  const details = signal(false);
                  return <div>{details.get() && <p>shown!</p>}</div>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "must be static-only")]
        fn rejects_nested_bindings_inside_conditional_content() {
            let source = r#"
                export function App() {
                  const details = signal(false);
                  const count = signal(0);
                  return <div>{details() && <p>{count()}</p>}</div>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "may not carry its own")]
        fn rejects_an_onclick_on_the_conditionals_own_wrapper() {
            let source = r#"
                export function App() {
                  const details = signal(false);
                  const count = signal(0);
                  return (
                    <div>
                      {details() && <button onClick={() => count.increment()}>go</button>}
                    </div>
                  );
                }
            "#;
            let module = parse_tsx(source);
            let graph = lower::lower_module(&module);
            codegen::generate(graph.root_component());
        }
    }

    mod input_binding {
        use super::*;

        const INPUT_SOURCE: &str = r#"
            import { signal } from "loom";

            export function App() {
              const name = signal("");

              return (
                <div>
                  <input value={name()} onInput={(e) => name.set(e.target.value)} />
                </div>
              );
            }
        "#;

        #[test]
        fn generates_the_input_element_setup_third_return_and_patch_system() {
            let rust = compile_tsx_to_rust(INPUT_SOURCE);
            assert!(rust.contains(r#"dom.create_element(node_0, "input", None)"#));
            assert!(rust.contains(r#"dom.set_attribute(node_1, "value", "")"#));
            assert!(rust.contains(r#""data-loom-id", "name""#));
            assert!(rust.contains("::loom_runtime::InputValueTarget"));
            assert!(rust.contains("::loom_runtime::patch_input_system"));
            // setup() must return the third `input_actions` value, and the
            // input site must be recorded in it via the bare tuple-variant
            // constructor (no closure — see codegen.rs's `setup()` doc).
            assert!(rust.contains("Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>"));
            assert!(rust.contains("input_actions.push((node_1, Action::Action0));"));
        }

        #[test]
        fn set_string_action_gets_an_empty_lref_capture_like_bool_and_stringlist() {
            let rust = compile_tsx_to_rust(INPUT_SOURCE);
            assert!(
                rust.contains(r#""App_chunk|Action0|""#),
                "input's LRef must capture nothing (Str is DOM-recovered), got: {rust}"
            );
        }

        #[test]
        fn resume_recovers_the_live_value_from_the_dom_not_a_capture() {
            let rust = compile_tsx_to_rust(INPUT_SOURCE);
            assert!(rust.contains("dom.get_input_value(name_node)"));
            assert!(rust.contains("fn resume_action_by_symbol(symbol: &str, string_arg: &str) -> Action"));
            assert!(rust.contains(r#""Action0" => Action::Action0(string_arg.to_string()),"#));
        }

        #[test]
        fn action_numbering_stays_in_sync_when_an_input_sits_between_two_click_actions() {
            // Regression test: `ctx.action_index` (the Element arm's onClick
            // counter) and `collect_actions`'s tree-order counting must
            // agree even when an `InputBinding` — which used to be looked up
            // by a separate signal-keyed map — sits between two onClick
            // buttons in source order. Before the fix, `rotate`'s button
            // below would incorrectly get "Action1" again instead of
            // "Action2".
            let source = r#"
                import { signal } from "loom";

                export function App() {
                  const items = signal(["a"]);
                  const name = signal("");

                  return (
                    <div>
                      <button onClick={() => items.append("x")}>append</button>
                      <input value={name()} onInput={(e) => name.set(e.target.value)} />
                      <button onClick={() => items.rotate()}>rotate</button>
                      <ul>
                        {items().map((item) => (
                          <li>{item}</li>
                        ))}
                      </ul>
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            assert!(rust.contains("input_actions.push((node_2, Action::Action1));"));
            assert!(rust.contains("actions.push((node_3, Action::Action2));"));
        }

        #[test]
        #[should_panic(expected = "must reference the same signal")]
        fn rejects_value_and_oninput_referencing_different_signals() {
            let source = r#"
                export function App() {
                  const a = signal("");
                  const b = signal("");
                  return <input value={a()} onInput={(e) => b.set(e.target.value)} />;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "must reference the arrow's own event parameter")]
        fn rejects_oninput_not_reading_from_its_own_event_param() {
            let source = r#"
                export function App() {
                  const name = signal("");
                  return <input value={name()} onInput={(e) => name.set(other.target.value)} />;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "requires a Bool signal")]
        fn rejects_toggle_on_a_str_signal() {
            let source = r#"
                export function App() {
                  const name = signal("");
                  return <button onClick={() => name.toggle()}>go</button>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }
    }

    /// Task #25 — compiler Island integration (증분 4): `ComponentGraph`/
    /// `IslandActivation`/`ComponentRef` multi-component support.
    mod islands {
        use super::*;

        const ISLAND_SOURCE: &str = r#"
            import { signal } from "loom";

            function Widget() {
              const count = signal(0);
              return <button onClick={() => count.increment()}>{count()}</button>;
            }

            export function App() {
              return (
                <div>
                  <Widget client:load />
                </div>
              );
            }
        "#;

        #[test]
        fn multi_component_graph_has_both_components_with_correct_root() {
            let module = parse_tsx(ISLAND_SOURCE);
            let graph = lower::lower_module(&module);
            assert_eq!(graph.root, "App");
            assert_eq!(graph.components.len(), 2);
            assert!(graph.components.iter().any(|c| c.name == "Widget"));
            assert_eq!(graph.activation_for("Widget"), Some(ir::IslandActivation::Load));
            assert_eq!(graph.activation_for("App"), None);
        }

        #[test]
        fn client_visible_parses_to_the_visible_activation() {
            let source = ISLAND_SOURCE.replace("client:load", "client:visible");
            let module = parse_tsx(&source);
            let graph = lower::lower_module(&module);
            assert_eq!(graph.activation_for("Widget"), Some(ir::IslandActivation::Visible));
        }

        #[test]
        fn single_component_graph_mode_islands_matches_legacy_compile_tsx_to_rust() {
            // Regression gate (Task #25 plan step 4): a graph with no
            // `ComponentRef` at all must compile byte-identically under
            // `GraphMode::Islands` to the pre-existing single-component
            // `compile_tsx_to_rust` path — no existing build.rs call site
            // should ever observe a behavior change from this task.
            let legacy = compile_tsx_to_rust(DEMO_SOURCE);
            let graph_mode = compile_tsx_to_rust_graph(DEMO_SOURCE, GraphMode::Islands);
            assert_eq!(legacy, graph_mode);
        }

        #[test]
        fn generate_islands_wraps_the_helper_component_and_emits_island_attributes() {
            let out = compile_tsx_to_rust_graph(ISLAND_SOURCE, GraphMode::Islands);
            assert!(out.contains("pub mod widget_generated {"), "{out}");
            assert!(out.contains("dom.set_attribute(node_0, \"data-loom-island\", \"\");"), "{out}");
            assert!(out.contains("\"data-loom-activation\", \"load\""), "{out}");
            assert!(out.contains("\"data-loom-chunk\", \"widget\""), "{out}");
            assert!(out.contains("\"data-loom-ctor\", \"WidgetIsland\""), "{out}");
            assert!(out.contains("pub struct WidgetIsland"), "{out}");
            assert!(out.contains("wasm_bindgen(constructor)"), "{out}");
            // The root component (App) is still emitted unqualified, exactly
            // like today's `generate()` — the parent's own `setup()` calls
            // into the Island's namespaced module rather than inlining it.
            assert!(out.contains("let _ = widget_generated::setup(app, dom,"), "{out}");
        }

        #[test]
        fn two_level_nested_island_reference_uses_a_super_path_not_a_bare_one() {
            // Regression test for a real bug found while building the
            // acceptance fixture: `ProductCard` (non-root, wrapped in its
            // own `pub mod`) referencing `Widget` (also non-root, a FLAT
            // sibling module, never nested inside ProductCard's own module —
            // see `generate_islands`) must escape its own wrapping module
            // with `super::`, unlike App's (the root's, unqualified) own
            // reference to a directly-owned Island, which needs no prefix.
            let source = r#"
                function Widget() {
                  const count = signal(0);
                  return <button onClick={() => count.increment()}>{count()}</button>;
                }
                function ProductCard() {
                  const details = signal(false);
                  return (
                    <div>
                      <button onClick={() => details.toggle()}>show details</button>
                      {details() && <p>Free shipping on orders over $50.</p>}
                      <Widget client:load />
                    </div>
                  );
                }
                export function App() {
                  return <div><ProductCard client:visible /></div>;
                }
            "#;
            let out = compile_tsx_to_rust_graph(source, GraphMode::Islands);
            assert!(out.contains("pub mod product_card_generated {"), "{out}");
            assert!(out.contains("pub mod widget_generated {"), "{out}");
            // App (root, unqualified) -> ProductCard: no prefix needed, both
            // sit at the same top level.
            assert!(out.contains("let _ = product_card_generated::setup(app, dom,"), "{out}");
            // ProductCard (wrapped) -> Widget: must escape via `super::`,
            // since `widget_generated` is ProductCard's sibling, not its
            // child.
            assert!(out.contains("let _ = super::widget_generated::setup(app, dom,"), "{out}");
        }

        #[test]
        fn single_island_client_output_contains_only_the_target_island() {
            // RQ6 §6 M1 real per-Island code splitting: unlike
            // `compile_tsx_to_rust_graph(..., GraphMode::Islands)` (which
            // always emits every component in the graph — the empirically-
            // found gap this function fixes), `compile_tsx_to_single_island_rust`
            // must emit ONLY the requested Island's own wasm-bindgen struct,
            // not its siblings'.
            let source = r#"
                function Widget() {
                  const count = signal(0);
                  return <button onClick={() => count.increment()}>{count()}</button>;
                }
                function ProductCard() {
                  const details = signal(false);
                  return (
                    <div>
                      <button onClick={() => details.toggle()}>show details</button>
                      {details() && <p>Free shipping on orders over $50.</p>}
                      <Widget client:load />
                    </div>
                  );
                }
                export function App() {
                  return <div><ProductCard client:visible /></div>;
                }
            "#;
            let widget_only = compile_tsx_to_single_island_rust(source, "Widget");
            assert!(widget_only.contains("pub struct WidgetIsland"), "{widget_only}");
            assert!(!widget_only.contains("ProductCardIsland"), "{widget_only}");
            // A leaf Island (no ComponentRef of its own) needs no stub
            // modules at all.
            assert!(!widget_only.contains("_generated {\n    pub struct Signals;"), "{widget_only}");

            let product_card_only = compile_tsx_to_single_island_rust(source, "ProductCard");
            assert!(product_card_only.contains("pub struct ProductCardIsland"), "{product_card_only}");
            assert!(!product_card_only.contains("WidgetIsland"), "{product_card_only}");
            // ProductCard nests a ComponentRef to Widget, so its (dead-code,
            // client-side-unused) `setup()` still textually references
            // `super::widget_generated::setup(...)` — a stub must exist so
            // this standalone crate still compiles.
            assert!(product_card_only.contains("let _ = super::widget_generated::setup(app, dom,"), "{product_card_only}");
            assert!(product_card_only.contains("pub mod widget_generated {\n    pub struct Signals;"), "{product_card_only}");
            assert!(product_card_only.contains("unreachable!("), "{product_card_only}");
        }

        #[test]
        fn graph_mode_flatten_merges_nested_components_into_one_signals_and_action() {
            // Task #26 — RQ6 §8 item 4's H-Qwik counterfactual shape: the
            // SAME source that produces two `pub mod` blocks under
            // GraphMode::Islands must instead produce ONE flat component
            // (no `pub mod`, no `data-loom-island*` attributes at all) with
            // BOTH nested components' signals/actions merged in.
            let source = r#"
                function Widget() {
                  const count = signal(0);
                  return <button onClick={() => count.increment()}>{count()}</button>;
                }
                function ProductCard() {
                  const details = signal(false);
                  return (
                    <div>
                      <button onClick={() => details.toggle()}>show details</button>
                      {details() && <p>Free shipping on orders over $50.</p>}
                      <Widget client:load />
                    </div>
                  );
                }
                export function App() {
                  return <div><ProductCard client:visible /></div>;
                }
            "#;
            let out = compile_tsx_to_rust_graph(source, GraphMode::Flatten);
            assert!(!out.contains("pub mod "), "{out}");
            assert!(!out.contains("data-loom-island"), "{out}");
            assert!(!out.contains("wasm_bindgen"), "{out}");
            // Both signals present in ONE merged Signals struct.
            assert!(out.contains("pub details: ::bevy_ecs::entity::Entity,"), "{out}");
            assert!(out.contains("pub count: ::bevy_ecs::entity::Entity,"), "{out}");
            // Both actions present in ONE merged Action enum/apply_action —
            // collect_actions walks the already-flattened tree in one pass.
            assert!(out.contains("enum Action {\n    Action0,\n    Action1,\n}"), "{out}");
        }

        #[test]
        #[should_panic(expected = "requires every signal name to be globally unique")]
        fn graph_mode_flatten_rejects_colliding_signal_names_across_components() {
            let source = r#"
                function Widget() {
                  const count = signal(0);
                  return <button onClick={() => count.increment()}>{count()}</button>;
                }
                export function App() {
                  const count = signal(0);
                  return (
                    <div>
                      <span>{count()}</span>
                      <Widget client:load />
                    </div>
                  );
                }
            "#;
            compile_tsx_to_rust_graph(source, GraphMode::Flatten);
        }

        #[test]
        #[should_panic(expected = "no `client:load`/`client:visible` directive")]
        fn bare_component_ref_without_activation_panics() {
            let source = r#"
                function Widget() {
                  const count = signal(0);
                  return <span>{count()}</span>;
                }
                export function App() {
                  return <div><Widget /></div>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "unsupported `client:idle` directive")]
        fn unsupported_client_directive_panics() {
            let source = ISLAND_SOURCE.replace("client:load", "client:idle");
            let module = parse_tsx(&source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "unsupported attribute namespace `data:`")]
        fn non_client_namespace_panics() {
            let source = r#"
                function Widget() {
                  const count = signal(0);
                  return <span>{count()}</span>;
                }
                export function App() {
                  return <div><Widget data:foo="bar" /></div>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        #[should_panic(expected = "no `function Foo()` is declared in this module")]
        fn unresolved_component_reference_panics_with_clear_message() {
            let source = r#"
                export function App() {
                  return <div><Foo client:load /></div>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        fn component_ref_with_prop_literals_lowers_without_panicking() {
            let source = r#"
                function Widget(props) {
                  const count = signal(0);
                  return <button onClick={() => count.increment()}>{count()}</button>;
                }
                export function App() {
                  return (
                    <div>
                      <Widget client:load initial={5} label="hi" active={true} />
                    </div>
                  );
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module); // must not panic
        }
    }

    /// Task #26 — `sharedSignal()`/`sharedValue()` (RQ4 cross-widget, pulled
    /// forward from Task #28-a to unblock the widget-embed fixture's
    /// CartBadge scenario).
    mod shared_signal {
        use super::*;

        const CART_SOURCE: &str = r#"
            import { signal, sharedSignal } from "loom";

            function Widget() {
              const cartCount = sharedSignal("cartCount", 0);
              return <button onClick={() => cartCount.increment()}>{cartCount()}</button>;
            }

            export function App() {
              return (
                <div>
                  <span>{sharedValue("cartCount", 0)}</span>
                  <Widget client:load />
                </div>
              );
            }
        "#;

        #[test]
        fn islands_mode_threads_shared_store_only_where_needed() {
            let out = compile_tsx_to_rust_graph(CART_SOURCE, GraphMode::Islands);
            // Widget authors sharedSignal directly -> its own setup/
            // apply_action/resume all gain the shared-store parameter.
            assert!(
                out.contains(
                    "pub fn apply_action(app: &mut ::loom_runtime::LoomApp, signals: &Signals, action: &Action, shared: &mut impl ::loom_runtime::SharedStore) {"
                ),
                "{out}"
            );
            assert!(out.contains("shared.set(\"cartCount\", app.world.get::<::loom_runtime::Signal<i32>>(signals.cartCount).unwrap().0);"), "{out}");
            assert!(out.contains("shared.set(\"cartCount\", 0);"), "{out}"); // setup()'s own seeding
            // App itself has NO shared_bindings of its own -> its OWN
            // setup() signature is untouched (still 3-arg) — but its
            // ComponentRef call into Widget passes a throwaway store.
            assert!(
                out.contains(
                    "pub fn setup(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mount: ::loom_runtime::NodeHandle) -> (Signals, Vec<(::loom_runtime::NodeHandle, Action)>, Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {"
                ),
                "{out}"
            );
            assert!(
                out.contains("let _ = widget_generated::setup(app, dom, node_0, &mut ::loom_runtime::InMemorySharedStore::default());"),
                "{out}"
            );
            // CartBadge-equivalent reader: plain static text + subscriber
            // attribute, no Signal/Entity at all for it.
            assert!(out.contains("data-loom-shared-subscriber\", \"cartCount\""), "{out}");
        }

        #[test]
        fn flatten_mode_merges_shared_bindings_and_needs_no_component_ref_forwarding() {
            let out = compile_tsx_to_rust_graph(CART_SOURCE, GraphMode::Flatten);
            assert!(!out.contains("pub mod "), "{out}");
            assert!(!out.contains("InMemorySharedStore"), "{out}"); // no ComponentRef left to forward through
            // The ONE merged component now directly owns the shared binding.
            assert!(
                out.contains(
                    "pub fn setup(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mount: ::loom_runtime::NodeHandle, shared: &mut impl ::loom_runtime::SharedStore) -> (Signals, Vec<(::loom_runtime::NodeHandle, Action)>, Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {"
                ),
                "{out}"
            );
            assert!(out.contains("data-loom-shared-subscriber\", \"cartCount\""), "{out}");
        }
    }
}
