mod codegen;
mod ir;
mod lower;

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
/// → Rust codegen. See `lower.rs` for the exact (intentionally narrow)
/// Phase 1 authoring-surface subset this supports.
pub fn compile_tsx_to_rust(source: &str) -> String {
    let module = parse_tsx(source);
    let component = lower::lower_module(&module);
    codegen::generate(&component)
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
        let component = lower::lower_module(&module);
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
        assert!(rust.contains("dom.append_static_text"));
        // 3 buttons -> 3 actions collected in source order.
        assert!(rust.contains("Action0"));
        assert!(rust.contains("Action1(String)"));
        assert!(rust.contains("Action2"));
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
        let component = lower::lower_module(&module);
        codegen::generate(&component);
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
            let component = lower::lower_module(&module);
            codegen::generate(&component);
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
}
