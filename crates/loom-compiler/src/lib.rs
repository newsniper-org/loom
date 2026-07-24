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
}
