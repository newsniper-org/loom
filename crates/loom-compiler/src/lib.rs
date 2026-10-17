pub mod build_support;
mod codegen;
pub mod diag;
mod ir;
mod lower;

pub use codegen::{GraphMode, ListCandidate};
pub use diag::{Compiled, Diagnostic, Diagnostics, Severity};

use swc_common::sync::Lrc;
use swc_common::{FileName, SourceFile, SourceMap, Spanned};
use swc_ecma_ast::Module;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};

/// SWC로 TSX를 파싱한다. Phase 7a(2026-10-17)부터 `SourceMap`을 함께 돌려준다
/// — 진단을 `파일:줄:열`로 렌더링하려면 span만으로는 부족하다.
/// `file_name`은 진단에 찍힐 이름일 뿐이며 codegen은 읽지 않는다(산출물 무관,
/// 게이트 A가 확인).
///
/// 구문 오류는 `LOOM-R23`으로 돌려준다. 메시지는 SWC 자체 메시지
/// (`SyntaxError::msg`)를 재사용한다. (복구 가능한 오류 — `take_errors()` —
/// 의 수거는 C4에서 별도로 다룬다.)
pub fn parse_tsx(file_name: &str, source: &str) -> Result<(Lrc<SourceMap>, Module), Diagnostics> {
    let cm: Lrc<SourceMap> = Default::default();
    let fm: Lrc<SourceFile> =
        cm.new_source_file(Lrc::new(FileName::Custom(file_name.into())), source.to_string());
    let syntax = Syntax::Typescript(TsSyntax {
        tsx: true,
        ..Default::default()
    });
    let lexer = Lexer::new(syntax, Default::default(), StringInput::from(&*fm), None);
    let mut parser = Parser::new_from(lexer);
    match parser.parse_module() {
        Ok(module) => Ok((cm, module)),
        Err(e) => {
            let mut d = Diagnostics::new(cm);
            d.push(Diagnostic::error(
                "LOOM-R23",
                e.span(),
                format!("구문 오류: {}", e.kind().msg()),
            ));
            Err(d)
        }
    }
}

/// docs/02's whole compile pipeline in one call: real SWC parse → IR lower
/// → Rust codegen. Single-component only (compiles just the module's one
/// exported root component, ignoring any Island-referenced helper
/// components — see `compile_tsx_to_rust_graph` for the 증분 4 multi-
/// component path) — kept as-is for backward compatibility with the
/// legacy `build.rs` call sites (`loom-demo`, `loom-fic`,
/// `loom-nested-fixture`; `loom-track-n` uses the `_with_candidate` variant).
/// (2026-10-17 정정: 이 주석의 옛 판은 `loom-resume-demo`와 `loom-bench`를
/// 호출처로 적었으나, resume-demo는 2026-07-28부터 `_graph(Flatten)`를 쓰고
/// loom-bench에는 build.rs가 없다.) See `lower.rs` for the exact
/// (intentionally narrow) authoring-surface subset this supports.
///
/// Phase 7a(2026-10-17)부터 `Result<Compiled, Diagnostics>`를 돌려준다.
/// `Compiled::rust`는 이전 `String` 반환값과 바이트 단위로 같다.
pub fn compile_tsx_to_rust(file_name: &str, source: &str) -> Result<Compiled, Diagnostics> {
    let (cm, module) = parse_tsx(file_name, source)?;
    let graph = lower::lower_module(&module);
    let rust = codegen::generate(graph.root_component());
    Ok(Compiled { rust, warnings: Diagnostics::new(cm) })
}

/// `compile_tsx_to_rust`의 **트랙 L 후보 선택** 판본 (2026-08-08 신설).
///
/// RQ1 §5 트랙 N의 불변식 2는 D의, 불변식 3은 E의 성질을 재는 것이라, 그
/// 후보로 컴파일된 픽스처가 없으면 애초에 잴 대상이 없다(공허해진다).
/// `ListCandidate::K`를 넘기면 `compile_tsx_to_rust`와 **바이트 동일한** 코드가
/// 나온다 — 기존 build.rs는 전부 그쪽을 계속 쓴다.
///
/// **비-K 산출물은 네이티브 전용이다**: `resume()`은 후보별 저장 Component와
/// row 키/순서 태그를 SSR DOM에서 되살려야 하는데 그 복구가 후보마다 다르고
/// 트랙 N은 resume을 쓰지 않는다. 조용히 K 모양으로 되살리는 대신 부른 쪽에서
/// 즉시 터지게 해 뒀다(codegen의 `generate_resume` 참고).
pub fn compile_tsx_to_rust_with_candidate(
    file_name: &str,
    source: &str,
    candidate: ListCandidate,
) -> Result<Compiled, Diagnostics> {
    let (cm, module) = parse_tsx(file_name, source)?;
    let graph = lower::lower_module(&module);
    let rust = codegen::generate_with_candidate(graph.root_component(), candidate);
    Ok(Compiled { rust, warnings: Diagnostics::new(cm) })
}

/// 증분 4 (Task #25) multi-component path — compiles the module's full
/// `ComponentGraph` (root + every Island-referenced helper component) in the
/// given `GraphMode`. See `codegen::GraphMode`'s doc comment for the two
/// modes' shapes.
pub fn compile_tsx_to_rust_graph(
    file_name: &str,
    source: &str,
    mode: GraphMode,
) -> Result<Compiled, Diagnostics> {
    let (cm, module) = parse_tsx(file_name, source)?;
    let graph = lower::lower_module(&module);
    let rust = codegen::generate_graph(&graph, mode);
    Ok(Compiled { rust, warnings: Diagnostics::new(cm) })
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
pub fn compile_tsx_to_single_island_rust(
    file_name: &str,
    source: &str,
    island_component_name: &str,
) -> Result<Compiled, Diagnostics> {
    let (cm, module) = parse_tsx(file_name, source)?;
    let graph = lower::lower_module(&module);
    let rust = codegen::generate_single_island_client(&graph, island_component_name);
    Ok(Compiled { rust, warnings: Diagnostics::new(cm) })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 7a C1(2026-10-17): 공개 API가 `(file_name, source, …) ->
    // Result<Compiled, Diagnostics>`로 바뀌었다. 아래 기존 테스트들은 옛
    // 시그니처로 쓰여 있으므로, 같은 이름의 얇은 래퍼로 가린다(glob import보다
    // 지역 정의가 우선). 성공 경로만 감싸며, 실패는 렌더된 진단으로 unwrap
    // 패닉한다. lower의 거부는 C2 전까지 여전히 `panic!`이라 `should_panic`
    // 테스트의 관측 메커니즘은 그대로다.
    const TEST_FILE: &str = "test.tsx";
    fn parse_tsx(source: &str) -> Module {
        super::parse_tsx(TEST_FILE, source).unwrap().1
    }
    fn compile_tsx_to_rust(source: &str) -> String {
        super::compile_tsx_to_rust(TEST_FILE, source).unwrap().rust
    }
    fn compile_tsx_to_rust_graph(source: &str, mode: GraphMode) -> String {
        super::compile_tsx_to_rust_graph(TEST_FILE, source, mode).unwrap().rust
    }
    fn compile_tsx_to_single_island_rust(source: &str, island: &str) -> String {
        super::compile_tsx_to_single_island_rust(TEST_FILE, source, island).unwrap().rust
    }

    mod diagnostics_api {
        use super::super::*;

        #[test]
        fn fatal_parse_error_is_a_diagnostic_rendered_at_the_offending_position() {
            // 2번째 줄 13열의 `;`(식이 와야 할 자리)가 SWC 파서의 치명 오류 지점.
            let src = "export function App() {\n  const x = ;\n  return <div />;\n}\n";
            let err = super::super::compile_tsx_to_rust("bad.tsx", src).unwrap_err();
            assert!(err.has_errors());
            let codes: Vec<&str> = err.items().iter().map(|d| d.code).collect();
            assert_eq!(codes, ["LOOM-R23"]);
            assert!(err.items()[0].message.starts_with("구문 오류: "), "{err}");
            let r = err.render();
            assert!(r.contains("error[LOOM-R23]: 구문 오류: "), "{r}");
            assert!(r.contains("--> bad.tsx:2:13"), "{r}");
            // 결정성: 같은 입력이면 렌더 결과가 같다.
            let again = super::super::compile_tsx_to_rust("bad.tsx", src).unwrap_err().render();
            assert_eq!(r, again);
        }

        #[test]
        fn successful_compile_carries_an_empty_warning_set() {
            let src = "export function App() {\n  const n = signal(0);\n  return <span>{n()}</span>;\n}\n";
            let ok = super::super::compile_tsx_to_rust("ok.tsx", src).unwrap();
            assert!(ok.rust.contains("pub fn setup("));
            assert!(ok.warnings.is_empty());
            assert!(!ok.warnings.has_errors());
            assert_eq!(ok.warnings.render(), "");
        }

        #[test]
        fn diagnostics_are_kept_sorted_by_position_then_code_and_render_labels_and_notes() {
            let (cm, _m) = super::super::parse_tsx("x.tsx", "const a = 1;\nconst b = 2;\n").unwrap();
            let f = cm.files()[0].clone();
            let at = |off: u32| swc_common::Span::new(f.start_pos + swc_common::BytePos(off), f.start_pos + swc_common::BytePos(off + 1));
            let mut d = Diagnostics::new(cm);
            d.push(Diagnostic::warning("LOOM-W99", at(19), "second line"));
            d.push(Diagnostic::error("LOOM-R99", at(6), "first line").with_label(at(19), "related").with_note("a note"));
            let codes: Vec<&str> = d.items().iter().map(|x| x.code).collect();
            assert_eq!(codes, ["LOOM-R99", "LOOM-W99"]);
            let r = d.render();
            let e = r.find("error[LOOM-R99]: first line").expect(&r);
            let w = r.find("warning[LOOM-W99]: second line").expect(&r);
            assert!(e < w, "{r}");
            assert!(r.contains("--> x.tsx:1:7"), "{r}");
            assert!(r.contains("--> x.tsx:2:7"), "{r}");
            assert!(r.contains("related"), "{r}");
            assert!(r.contains("= note: a note"), "{r}");
        }
    }

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

    /// row 템플릿의 사이트별 monomorphize (2026-08-07). 동작 확인은
    /// `crates/loom-nested-fixture`가 실제로 돌려서 하고, 여기서는 **생성된
    /// 코드의 구조 성질** — 동작 테스트로는 원리적으로 관측할 수 없는 것 — 만
    /// 고정한다.
    mod row_template {
        use super::*;

        const NESTED: &str = r#"
            export function App() {
              const outer = signal(["a", "bb"]);
              const open = signal(false);
              const inner = signal(["p", "q"]);
              return (
                <div>
                  <button onClick={() => open.toggle()}>t</button>
                  <ul>
                    {outer().map((it) => (
                      <li>
                        {it}
                        {open() && it.length > 1 && (
                          <div class="panel">
                            <ul>{inner().slice(0, it.length).map((x) => <li>{x}</li>)}</ul>
                          </div>
                        )}
                      </li>
                    ))}
                  </ul>
                </div>
              );
            }
        "#;

        #[test]
        fn the_row_mirror_sync_system_is_gated_on_the_source_actually_changing() {
            // **docs/03 N3(변경 무관 크기 비례 갱신 금지)**. 이 System은 row를
            // 전부 훑으므로, 게이팅이 없으면 *무관한* 갱신 하나에도 이 사이트
            // 크기에 비례하는 비용이 붙는다.
            //
            // 이 단언이 문자열 대조인 이유: 게이팅의 유무는 **비용** 성질이고,
            // 결과 DOM에는 아무 차이도 남기지 않는다(값이 같으면 어차피 쓰지
            // 않으므로). 실제로 게이팅을 걷어내고 loom-nested-fixture의 동작
            // 테스트 14개를 돌려 봐도 전부 통과한다 — 즉 동작 테스트로는
            // 원리적으로 못 잡는다. 그래서 구조로 고정한다.
            let rust = compile_tsx_to_rust(NESTED);
            assert!(rust.contains("fn row_site_0_sync("), "{rust}");
            let body_start = rust.find("fn row_site_0_sync(").unwrap();
            let body = &rust[body_start..];
            assert!(
                body.contains("DetectChanges::is_changed(&site) { return; }"),
                "row mirror sync System에 N3 게이팅이 없다: {body}"
            );
        }

        #[test]
        fn every_list_site_gets_a_marker_when_any_row_template_is_rich() {
            // 마커 없는 제네릭 K와 섞이면 한 리스트를 두 시스템이 각각
            // 재조정한다 — `RowSpawner::Marker` 문서의 근거.
            let rust = compile_tsx_to_rust(NESTED);
            assert!(!rust.contains("app.add_systems(::loom_runtime::list_reconcile_k_system::<String>);"), "{rust}");
            assert!(rust.contains("list_reconcile_k_system_with::<String, RowSite0Spawner>"), "{rust}");
            assert!(rust.contains("list_reconcile_k_system_with::<String, RowSite0InnerSpawner>"), "{rust}");
        }

        #[test]
        fn row_derived_values_are_inlined_not_stored_as_behavior() {
            // CLAUDE.md 금지 4: Component에는 파생 **결과**만 POD로 들어가고,
            // 계산은 생성 코드에 인라인된다.
            let rust = compile_tsx_to_rust(NESTED);
            assert!(rust.contains("let derived_flag = text.chars().count() > 1usize;"), "{rust}");
            assert!(rust.contains("::loom_runtime::RowDerivedFlag(derived_flag)"), "{rust}");
            assert!(rust.contains("::loom_runtime::RowDerivedTake(derived_take)"), "{rust}");
        }

        #[test]
        #[should_panic(expected = "row 템플릿 안에서만")]
        fn rejects_a_row_predicate_outside_a_row_template() {
            let source = r#"
                export function App() {
                  const open = signal(false);
                  const n = signal(0);
                  return <div>{open() && n() > 1 && <p>x</p>}</div>;
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }
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
            // OWN marker. `setup()`'s spawn additionally carries this site's
            // `CondNested` (empty here — no nested bindings in this fixture);
            // `resume()` attaches it after the loop instead, so its spawn
            // ends at the marker.
            assert!(rust.contains("CondSite0, ::loom_runtime::CondNested(vec![]))).id();"), "{rust}"); // setup()
            assert_eq!(rust.matches(", CondSite0)).id();").count(), 1, "{rust}"); // resume()
            assert!(rust.contains("CondSite1, ::loom_runtime::CondNested(vec![]))).id();"), "{rust}");
            assert_eq!(rust.matches(", CondSite1)).id();").count(), 1, "{rust}");
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
        fn conditional_wrapper_can_itself_be_a_text_binding() {
            // 2026-08-07까지 이 형태는 lower.rs가 "must be static-only"로
            // 거부했다. 이제는 컴파일된다 — wrapper(`<p>`)가 곧 binding 노드인
            // 특수한 경우라 새 노드를 만들지 않고, 조건부가 처음 열릴 때
            // `DomTarget`만 뒤늦게 붙이고 현재 값을 한 번 써넣는다.
            let source = r#"
                export function App() {
                  const details = signal(false);
                  const count = signal(7);
                  return (
                    <div>
                      <button onClick={() => details.toggle()}>toggle</button>
                      {details() && <p>{count()}</p>}
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            // 중첩 signal의 entity는 `setup()`에서 **즉시** 만들어지되
            // `DomTarget` 없이 — 그래야 patch System의 쿼리가 archetype
            // 수준에서 알아서 제외한다.
            assert!(rust.contains("let count_entity = app.world.spawn((::loom_runtime::Signal(7_i32),)).id();"), "{rust}");
            // 그리고 조건부 entity가 그것을 고정 순서로 들고 있다.
            assert!(rust.contains("CondSite0, ::loom_runtime::CondNested(vec![count_entity]))).id();"), "{rust}");
            // 사이트 함수는 이름 조회 없이 리터럴 인덱스로만 접근한다.
            assert!(rust.contains("let slot_0 = nested.0[0usize];"), "{rust}");
            assert!(rust.contains("commands.entity(slot_0).insert(::loom_runtime::DomTarget(anchor.0));"), "{rust}");
            assert!(rust.contains("::loom_runtime::DomWrite::SetText { target: anchor.0"), "{rust}");
            // 숨겨진 채 SSR된 경우 `find_node`를 부르면 안 된다 — 그 노드는
            // 아직 존재하지 않는다.
            assert!(rust.contains("if details_shown {"), "{rust}");
            assert!(rust.contains("app.world.spawn((::loom_runtime::Signal(value),)).id()"), "{rust}");
        }

        #[test]
        fn conditional_can_contain_a_nested_element_tree_with_a_list() {
            // RQ1 §5 트랙 N이 요구하는 "조건부 안의 리스트"가 실제로
            // 컴파일되는지 — 손코딩 픽스처는 §2.3-1 위반이므로 컴파일러가
            // 낼 수 있어야만 한다.
            let source = r#"
                export function App() {
                  const open = signal(false);
                  const items = signal(["a", "b"]);
                  return (
                    <div>
                      <button onClick={() => open.toggle()}>toggle</button>
                      {open() && <section class="panel">
                        <h3>inner</h3>
                        <ul>{items().map((it) => <li>{it}</li>)}</ul>
                      </section>}
                    </div>
                  );
                }
            "#;
            let rust = compile_tsx_to_rust(source);
            // 완전 정적인 `<h3>inner</h3>`는 컴파일타임에 HTML 한 덩어리로
            // 접힌다 — 지연 경로에도 setup() 쪽과 같은 접기가 적용된다.
            assert!(rust.contains(r#"AppendStaticHtml { parent: ::loom_runtime::WriteParent::Node(anchor.0), html: "<h3>inner</h3>".to_string() }"#), "{rust}");
            // 리스트 노드는 지연 생성되고 `data-loom-id`를 그때 단다.
            assert!(rust.contains(r#"tag: "ul", class: None, loom_id: Some("items"), text: None"#), "{rust}");
            // 초기 row는 사이트 함수가 직접 방출한다 — 제네릭 K에 맡기면
            // `DomTarget`이 붙는 시점과 `Changed`가 엇갈려 영영 안 그려진다.
            assert!(rust.contains("commands.entity(slot_0).insert(::loom_runtime::ListRows(lazy_0));"), "{rust}");
            assert!(rust.contains("parent: ::loom_runtime::WriteParent::Entity(slot_0)"), "{rust}");
            // 그리고 이후 변경은 평범하게 제네릭 K가 처리해야 하므로 System이
            // 등록돼 있어야 한다(조건부 안만 훑지 않으면 빠지던 부분).
            assert!(rust.contains("app.add_systems(::loom_runtime::list_reconcile_k_system::<String>);"), "{rust}");
        }

        #[test]
        #[should_panic(expected = "안에 또 조건부를 넣는 것은 아직 지원하지 않는다")]
        fn rejects_a_conditional_nested_directly_inside_another_conditional() {
            let source = r#"
                export function App() {
                  const outer = signal(false);
                  const inner = signal(false);
                  return <div>{outer() && <p>{inner() && <span>x</span>}</p>}</div>;
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
            //
            // 2026-07-28 (RQ6 §7 판정 이후) 갱신: 예전엔 이 모드가 순수 데이터
            // 병합 산출물이라 `wasm_bindgen`이 전혀 없어야 한다고 검증했지만,
            // H-Qwik 확정 이후 `generate_flatten_entry_body`가 실제 브라우저
            // resumable 진입점(`ResumedApp`)을 자동 생성하므로 그 전제가
            // 뒤집혔다 — 이제는 `wasm_bindgen`이 있어야 "정상"이다. 병합 검증
            // (Signals/Action 하나로 합쳐짐) 자체는 이 갱신과 무관하게 그대로
            // 유효해 유지한다.
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
            assert!(!out.contains("pub mod "), "{out}"); // resumed_app은 pub mod가 아니라 mod
            assert!(!out.contains("data-loom-island"), "{out}");
            // Both signals present in ONE merged Signals struct.
            assert!(out.contains("pub details: ::bevy_ecs::entity::Entity,"), "{out}");
            assert!(out.contains("pub count: ::bevy_ecs::entity::Entity,"), "{out}");
            // Both actions present in ONE merged Action enum/apply_action —
            // collect_actions walks the already-flattened tree in one pass.
            assert!(out.contains("enum Action {\n    Action0,\n    Action1,\n}"), "{out}");
            // 자동 생성된 resumable wasm 진입점 — has_shared=false 형태
            // (이 픽스처는 sharedSignal을 안 씀).
            assert!(out.contains("mod resumed_app {"), "{out}");
            assert!(out.contains("pub struct ResumedApp { app: ::loom_runtime::LoomApp, dom: ::loom_runtime::web::BrowserDom, signals: super::Signals }"), "{out}");
            assert!(out.contains("pub fn new(captures_str: &str) -> ResumedApp {"), "{out}");
            assert!(out.contains("pub fn dispatch(&mut self, symbol: &str, value: Option<String>) {"), "{out}");
            assert!(!out.contains("JsSharedStore"), "{out}"); // has_shared=false라 shared 필드 없음
            assert!(out.contains("pub use resumed_app::ResumedApp;"), "{out}");
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

        #[test]
        #[should_panic(expected = "component prop value must be a number, boolean, string, or string-array literal")]
        fn component_ref_prop_cannot_carry_a_live_signal_value() {
            // RQ6 §6 M10 / RQ3 "알려진 약점" 3번 실측(Task #27) — "중첩 island
            // 간 동적 데이터 전달 부재(SSR prop 동결)"가 실제로 재현되는지
            // 직접 확인: 부모의 살아있는 시그널 값을 자식 Island의 prop으로
            // 넘기려 하면(literal이 아니라 `parentCount()` 호출식) 컴파일
            // 시점에 거부된다 — `lower_prop_literal`이 리터럴 형태만 받고
            // 그 외 표현식은 전부 panic시키기 때문. 이 문서가 말하는 "SSR
            // prop 동결"은 완화책이 아니라 이 컴파일러의 구조적 한계임을
            // 실측으로 확인.
            let source = r#"
                function Widget(props) {
                  const count = signal(0);
                  return <button onClick={() => count.increment()}>{count()}</button>;
                }
                export function App() {
                  const parentCount = signal(1);
                  return (
                    <div>
                      <Widget client:load initial={parentCount()} />
                    </div>
                  );
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
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
            // 2026-07-28 — 자동 생성된 resumable wasm 진입점, has_shared=true
            // 형태(JsSharedStore 필드 + apply_action의 4-인자 shared 호출).
            assert!(out.contains("pub struct ResumedApp { app: ::loom_runtime::LoomApp, dom: ::loom_runtime::web::BrowserDom, signals: super::Signals, shared: ::loom_runtime::web::JsSharedStore }"), "{out}");
            assert!(
                out.contains("super::apply_action(&mut self.app, &self.signals, &action, &mut self.shared);"),
                "{out}"
            );
            assert!(
                out.contains("let signals = super::resume(&mut app, &mut dom, find_node, &capture_refs, &mut shared);"),
                "{out}"
            );
        }
    }

    /// Task #28-f — "signal당 액션 1개" 정적 검사(`lower.rs`의
    /// `check_one_action_per_signal`).
    mod single_writer_check {
        use super::*;

        #[test]
        #[should_panic(expected = "more than one action-bearing binding site targeting i32 signal `count`")]
        fn two_actions_on_the_same_i32_signal_is_rejected() {
            let source = r#"
                export function App() {
                  const count = signal(0);
                  return (
                    <div>
                      <button onClick={() => count.increment()}>+1</button>
                      <button onClick={() => count.increment()}>+1 again</button>
                    </div>
                  );
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module);
        }

        #[test]
        fn two_actions_on_the_same_stringlist_signal_is_allowed() {
            // demo/counter-list.tsx의 실제 패턴 그대로(append + rotate가 같은
            // `items` StringList 시그널을 같이 건드림) — 이건 위반이 아니다:
            // StringList는 DOM-복구형이라 SingleWriter가 필요 없다
            // (formal-verification/lean4/ResumabilitySoundness.lean의
            // dom_recovered_resumability_sound). 이전에 이 검사를 모든
            // 시그널 종류에 적용하려던 첫 시도가 이 정확한 픽스처를 깨뜨렸음
            // (cargo test --workspace로 실제 발견) — 그 회귀를 여기 고정한다.
            let source = r#"
                export function App() {
                  const items = signal(["a", "b"]);
                  return (
                    <div>
                      <ul>{items().map((item) => <li>{item}</li>)}</ul>
                      <button onClick={() => items.append("new")}>append</button>
                      <button onClick={() => items.rotate()}>rotate</button>
                    </div>
                  );
                }
            "#;
            let module = parse_tsx(source);
            lower::lower_module(&module); // must not panic
        }
    }
}
