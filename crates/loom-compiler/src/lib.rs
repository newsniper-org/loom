pub mod build_support;
mod check;
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
/// (`SyntaxError::msg`)를 재사용한다.
///
/// 7a C4(R23, 2026-10-17): SWC 파서는 **복구 가능한** 오류(예: `with` 문 —
/// `WithInStrict`/TS2410, 레거시 8진 리터럴 `010` — `LegacyOctal`/TS1085)를
/// `parse_module()`의 `Ok` 안에 숨기고 `take_errors()`로만 내준다. 7a 이전에는
/// 이것을 수거하지 않아 그런 입력이 조용히 다음 단계로 넘어갔다. 이제 `Ok`이든
/// `Err`이든 `take_errors()`를 전부 수거해 하나라도 있으면 `Err`로 돌려준다
/// (치명 오류가 있으면 그것도 같은 `Diagnostics`에 함께). 소비자 22회 호출의
/// 소스는 C4 착수 전 프로브에서 수거 오류 0건이었다.
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
    let parsed = parser.parse_module();
    let recovered = parser.take_errors();
    let syntax_error = |e: &swc_ecma_parser::error::Error| {
        Diagnostic::error("LOOM-R23", e.span(), format!("구문 오류: {}", e.kind().msg()))
    };
    match parsed {
        Ok(module) if recovered.is_empty() => Ok((cm, module)),
        parsed => {
            let mut d = Diagnostics::new(cm);
            for e in &recovered {
                d.push(syntax_error(e));
            }
            if let Err(e) = &parsed {
                d.push(syntax_error(e));
            }
            Err(d)
        }
    }
}

fn single(cm: &Lrc<SourceMap>, d: Diagnostic) -> Diagnostics {
    let mut ds = Diagnostics::new(cm.clone());
    ds.push(d);
    ds
}

/// lower 단계(7a C2부터 `Result`)를 공개 API의 `Diagnostics`로 감싼다.
/// `SpanTable`은 7a C3부터 `check`가 진단 위치를 찾는 데 쓴다. 셋째 값은
/// lower의 **경고**(7a C6부터 — W25)를 담은 `Diagnostics`로, 성공한 컴파일의
/// `Compiled::warnings`가 된다(산출물 바이트와 무관).
fn lower_graph(
    cm: &Lrc<SourceMap>,
    module: &Module,
) -> Result<(ir::ComponentGraph, lower::SpanTable, Diagnostics), Diagnostics> {
    let (graph, spans, warnings) = lower::lower_module(module).map_err(|d| single(cm, d))?;
    let mut ws = Diagnostics::new(cm.clone());
    for w in warnings {
        ws.push(w);
    }
    Ok((graph, spans, ws))
}

/// 7a C3: codegen이 패닉으로 거부하던 사용자 도달 입력을 codegen **앞에서**
/// 진단으로 돌려준다(`check.rs`). codegen 자체는 바꾸지 않았으므로, 이 검사를
/// 통과한 입력의 산출물 바이트는 7a 이전과 같다.
fn check_graph(
    cm: &Lrc<SourceMap>,
    graph: &ir::ComponentGraph,
    spans: &lower::SpanTable,
    target: check::Target<'_>,
) -> Result<(), Diagnostics> {
    check::check(graph, spans, target).map_err(|d| single(cm, d))
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
    let (graph, spans, warnings) = lower_graph(&cm, &module)?;
    check_graph(&cm, &graph, &spans, check::Target::RootOnly)?;
    let rust = codegen::generate(graph.root_component());
    Ok(Compiled { rust, warnings })
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
    let (graph, spans, warnings) = lower_graph(&cm, &module)?;
    check_graph(&cm, &graph, &spans, check::Target::RootOnly)?;
    let rust = codegen::generate_with_candidate(graph.root_component(), candidate);
    Ok(Compiled { rust, warnings })
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
    let (graph, spans, warnings) = lower_graph(&cm, &module)?;
    check_graph(&cm, &graph, &spans, check::Target::Graph(&mode))?;
    let rust = codegen::generate_graph(&graph, mode);
    Ok(Compiled { rust, warnings })
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
    let (graph, spans, warnings) = lower_graph(&cm, &module)?;
    check_graph(&cm, &graph, &spans, check::Target::SingleIsland(island_component_name))?;
    let rust = codegen::generate_single_island_client(&graph, island_component_name);
    Ok(Compiled { rust, warnings })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Phase 7a C1(2026-10-17): 공개 API가 `(file_name, source, …) ->
    // Result<Compiled, Diagnostics>`로 바뀌었다. 아래 기존 테스트들은 옛
    // 시그니처로 쓰여 있으므로, 같은 이름의 얇은 래퍼로 가린다(glob import보다
    // 지역 정의가 우선). 성공 경로만 감싸며, 실패는 렌더된 진단으로 unwrap
    // 패닉한다.
    //
    // 7a C2(2026-10-17): lower의 거부가 `Err(Diagnostic)`이 되어, lower 거부를
    // 보던 `should_panic` 테스트 13개는 `assert_lower_diag`로 바꿨다. 기대
    // 부분 문자열은 그대로 두고, 진단 코드와 **강조되는 소스 토큰**(진단이
    // 문제 구문을 정확히 가리키는지)을 함께 단언한다. codegen이 거부하는
    // `should_panic` 3개(mixes a dynamic binding / may not carry its own /
    // globally unique)는 C3(check.rs 사전 검사)에서 다룬다.
    //
    // 7a C3(2026-10-17): 그 3개도 `assert_check_diag`로 바꿨다 — 공개 API가
    // codegen 패닉 대신 check.rs의 진단을 돌려준다. codegen 패닉이 실제로
    // 백스톱으로 남아 있는지는 `check_backstop` 모듈이 따로 단언한다.
    const TEST_FILE: &str = "test.tsx";

    /// parse + lower만 돌려 lower 진단을 받는다(성공하면 테스트 실패).
    fn lower_diag(source: &str) -> Diagnostics {
        let (cm, module) = super::parse_tsx(TEST_FILE, source).unwrap();
        match super::lower_graph(&cm, &module) {
            Ok(_) => panic!("expected a lower diagnostic, but lowering succeeded"),
            Err(d) => d,
        }
    }

    /// 진단이 정확히 1건이고, 코드·문구(부분 문자열)·강조 토큰이 기대와 같으며,
    /// 렌더 결과의 `파일:줄:열`이 그 span의 위치인지 단언한다.
    fn assert_lower_diag(source: &str, code: &str, needle: &str, token: &str) -> Diagnostics {
        let d = lower_diag(source);
        assert_eq!(d.items().len(), 1, "{d}");
        let item = &d.items()[0];
        assert_eq!(item.code, code, "{d}");
        assert_eq!(item.severity, Severity::Error, "{d}");
        assert!(item.message.contains(needle), "message {:?} lacks {needle:?}", item.message);
        assert_eq!(d.source_text(item.span).as_deref(), Some(token), "{d}");
        let (line, col) = d.line_col(item.span);
        let r = d.render();
        assert!(r.contains(&format!("error[{code}]: ")), "{r}");
        assert!(r.contains(&format!("--> {TEST_FILE}:{line}:{col}")), "{r}");
        d
    }
    /// `assert_lower_diag`의 일반판: 이미 받은 진단 묶음에 같은 단언을 한다.
    /// `token`이 `None`이면 소스 밖 진단(span 없음)이다.
    fn assert_diag(d: Diagnostics, code: &str, needle: &str, token: Option<&str>) -> Diagnostics {
        assert_eq!(d.items().len(), 1, "{d}");
        let item = &d.items()[0];
        assert_eq!(item.code, code, "{d}");
        assert_eq!(item.severity, Severity::Error, "{d}");
        assert!(item.message.contains(needle), "message {:?} lacks {needle:?}", item.message);
        let r = d.render();
        assert!(r.contains(&format!("error[{code}]: ")), "{r}");
        match token {
            Some(token) => {
                assert_eq!(d.source_text(item.span).as_deref(), Some(token), "{d}");
                let (line, col) = d.line_col(item.span);
                assert!(r.contains(&format!("--> {TEST_FILE}:{line}:{col}")), "{r}");
            }
            None => assert!(item.span.is_dummy(), "{d}"),
        }
        d
    }

    /// 공개 API 결과가 check.rs 진단 1건인지 단언한다(패닉이면 테스트 실패).
    fn assert_check_diag(result: Result<Compiled, Diagnostics>, needle: &str, token: Option<&str>) -> Diagnostics {
        match result {
            Ok(_) => panic!("expected a check diagnostic, but compilation succeeded"),
            Err(d) => assert_diag(d, lower::L00, needle, token),
        }
    }

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

        /// 7a C4 (R23): SWC가 `parse_module()`의 `Ok` 안에 숨기는 복구 가능 오류.
        /// (소스, 기대 강조 토큰, 기대 `줄:열`, 기대 SWC 메시지 부분 문자열들, 최소 차이 양성 짝)
        const R23_CASES: &[(&str, &str, &str, &[&str], &str)] = &[
            (
                "export function App() {\n  with (o) {}\n  const n = signal(0);\n  return <span>{n()}</span>;\n}\n",
                "with",
                "r23.tsx:2:3",
                &["The 'with' statement is not supported", "With statement are not allowed in strict mode"],
                "export function App() {\n  const n = signal(0);\n  return <span>{n()}</span>;\n}\n",
            ),
            (
                "export function App() {\n  const n = signal(010);\n  return <span>{n()}</span>;\n}\n",
                "010",
                "r23.tsx:2:20",
                &["Legacy octal literals are not available", "Legacy octal escape is not permitted in strict mode"],
                "export function App() {\n  const n = signal(8);\n  return <span>{n()}</span>;\n}\n",
            ),
        ];

        /// SWC를 직접 불러 (치명 오류 결과, 수거 오류 수)를 본다 — `parse_tsx`를 거치지 않음.
        fn raw_swc_parse(src: &str) -> (Result<Module, ()>, usize) {
            let cm: Lrc<SourceMap> = Default::default();
            let fm = cm.new_source_file(Lrc::new(FileName::Custom("raw.tsx".into())), src.to_string());
            let syntax = Syntax::Typescript(TsSyntax { tsx: true, ..Default::default() });
            let mut p = Parser::new_from(Lexer::new(syntax, Default::default(), StringInput::from(&*fm), None));
            let m = p.parse_module().map_err(|_| ());
            (m, p.take_errors().len())
        }

        #[test]
        fn r23_recoverable_parse_errors_exist_and_were_hidden_inside_ok() {
            // 존재 단언(공허한 확인 방지): 각 부정 스니펫에서 SWC의 parse_module()은
            // 실제로 Ok를 돌려주면서 take_errors()에 오류를 남긴다 — 즉 C4 이전
            // parse_tsx(Ok만 보던)는 이것을 놓쳤다.
            for (src, ..) in R23_CASES {
                let (m, n) = raw_swc_parse(src);
                assert!(m.is_ok(), "치명 오류가 아니어야 함: {src}");
                assert!(n > 0, "take_errors()가 비어 있음 — 존재 단언 실패: {src}");
            }
            // 그리고 레거시 8진 스니펫은 수거하지 않으면 lower까지 조용히 통과한다
            // (C4가 막는 '조용한 수용'의 실재 확인).
            let (m, _) = raw_swc_parse(R23_CASES[1].0);
            assert!(lower::lower_module(&m.unwrap()).is_ok());
        }

        #[test]
        fn r23_recoverable_parse_errors_are_rejected_at_the_offending_token_with_swc_messages() {
            for (src, token, at, msgs, ok) in R23_CASES {
                let results = [
                    super::super::compile_tsx_to_rust("r23.tsx", src).map(|c| c.rust),
                    super::super::compile_tsx_to_rust_with_candidate("r23.tsx", src, ListCandidate::K).map(|c| c.rust),
                    super::super::compile_tsx_to_rust_graph("r23.tsx", src, GraphMode::Flatten).map(|c| c.rust),
                    super::super::compile_tsx_to_rust_graph("r23.tsx", src, GraphMode::Islands).map(|c| c.rust),
                ];
                for r in results {
                    let e = r.unwrap_err();
                    assert!(!e.is_empty());
                    for d in e.items() {
                        assert_eq!(d.code, "LOOM-R23", "{e}");
                        assert!(d.message.starts_with("구문 오류: "), "{e}");
                        assert_eq!(e.source_text(d.span).as_deref(), Some(*token), "{e}");
                    }
                    let rendered = e.render();
                    for m in *msgs {
                        assert!(e.items().iter().any(|d| d.message.contains(m)), "{m} 없음:\n{rendered}");
                    }
                    assert!(rendered.contains(&format!("--> {at}")), "{rendered}");
                    assert_eq!(rendered, super::super::compile_tsx_to_rust("r23.tsx", src).unwrap_err().render());
                }
                // 최소 차이 양성 짝은 끝까지 컴파일된다.
                let c = super::super::compile_tsx_to_rust("r23.tsx", ok).unwrap();
                assert!(c.rust.contains("pub fn setup("));
            }
        }

        #[test]
        fn r23_recoverable_and_fatal_errors_are_reported_together_in_position_order() {
            let src = "export function App() {\n  with (o) {}\n  const x = ;\n  return <div />;\n}\n";
            let e = super::super::compile_tsx_to_rust("r23.tsx", src).unwrap_err();
            let at: Vec<(usize, usize)> = e.items().iter().map(|d| e.line_col(d.span)).collect();
            assert_eq!(at, [(2, 3), (2, 3), (3, 13)], "{e}");
            assert!(e.items().iter().all(|d| d.code == "LOOM-R23"));
            assert!(e.items()[2].message.contains("Expression expected"), "{e}");
        }

        #[test]
        fn successful_compile_carries_an_empty_warning_set() {
            // 7a C6: W25(import 누락 경고)가 생겼으므로 `signal`을 import한다.
            let src = "import { signal } from \"loom\";\nexport function App() {\n  const n = signal(0);\n  return <span>{n()}</span>;\n}\n";
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
        let graph = lower::lower_module(&module).unwrap().0;
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
    fn rejects_mixed_static_and_dynamic_children() {
        let source = r#"
            export function App() {
              const count = signal(0);
              return <span>count: {count()}</span>;
            }
        "#;
        let d = assert_check_diag(super::compile_tsx_to_rust(TEST_FILE, source), "mixes a dynamic binding", Some("span"));
        // 보조 label이 동적 자식을 가리킨다.
        assert_eq!(d.items()[0].labels.len(), 1);
        assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("count()"));
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
                fn rejects_a_row_predicate_outside_a_row_template() {
            let source = r#"
                export function App() {
                  const open = signal(false);
                  const n = signal(0);
                  return <div>{open() && n() > 1 && <p>x</p>}</div>;
                }
            "#;
            assert_lower_diag(source, lower::L00, "row 템플릿 안에서만", "n() > 1");
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
                fn rejects_toggle_on_a_non_bool_signal() {
            let source = r#"
                export function App() {
                  const count = signal(0);
                  return <button onClick={() => count.toggle()}>toggle</button>;
                }
            "#;
            assert_lower_diag(source, lower::L00, "requires a Bool signal", "count");
        }

        #[test]
                fn rejects_a_non_bare_signal_call_on_the_left_of_and() {
            let source = r#"
                export function App() {
                  const details = signal(false);
                  return <div>{details.get() && <p>shown!</p>}</div>;
                }
            "#;
            assert_lower_diag(source, lower::L00, "conditional's left side must be a bare", "details.get");
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
                fn rejects_a_conditional_nested_directly_inside_another_conditional() {
            let source = r#"
                export function App() {
                  const outer = signal(false);
                  const inner = signal(false);
                  return <div>{outer() && <p>{inner() && <span>x</span>}</p>}</div>;
                }
            "#;
            assert_lower_diag(source, lower::L00, "안에 또 조건부를 넣는 것은 아직 지원하지 않는다", "{inner() && <span>x</span>}");
        }

        #[test]
        fn rejects_an_onclick_on_the_conditionals_own_wrapper() {
            let source = r#"
                export function App() {
                  const details = signal(false);
                  const count = signal(0);
                  return (
                    <div>
                      {details() && <button onClick={() => count.increment()}>go</button>}
                      <p>{count()}</p>
                    </div>
                  );
                }
            "#;
            assert_check_diag(
                super::super::compile_tsx_to_rust(TEST_FILE, source),
                "may not carry its own",
                Some("onClick={() => count.increment()}"),
            );
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
                fn rejects_value_and_oninput_referencing_different_signals() {
            let source = r#"
                export function App() {
                  const a = signal("");
                  const b = signal("");
                  return <input value={a()} onInput={(e) => b.set(e.target.value)} />;
                }
            "#;
            assert_lower_diag(source, lower::L00, "must reference the same signal", "onInput={(e) => b.set(e.target.value)}");
        }

        #[test]
                fn rejects_oninput_not_reading_from_its_own_event_param() {
            let source = r#"
                export function App() {
                  const name = signal("");
                  return <input value={name()} onInput={(e) => name.set(other.target.value)} />;
                }
            "#;
            assert_lower_diag(source, lower::L00, "must reference the arrow's own event parameter", "other");
        }

        #[test]
                fn rejects_toggle_on_a_str_signal() {
            let source = r#"
                export function App() {
                  const name = signal("");
                  return <button onClick={() => name.toggle()}>go</button>;
                }
            "#;
            assert_lower_diag(source, lower::L00, "requires a Bool signal", "name");
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
            let graph = lower::lower_module(&module).unwrap().0;
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
            let graph = lower::lower_module(&module).unwrap().0;
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
            let d = assert_check_diag(
                super::super::compile_tsx_to_rust_graph(TEST_FILE, source, GraphMode::Flatten),
                "requires every signal name to be globally unique",
                Some("Widget"),
            );
            // label이 충돌한 둘째 선언(Widget 안의 `count`)을 가리킨다.
            let (label_span, _) = &d.items()[0].labels[0];
            assert_eq!(d.source_text(*label_span).as_deref(), Some("count"));
            assert_eq!(d.line_col(*label_span).0, 3);
        }

        #[test]
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
            assert_lower_diag(source, lower::L00, "no `client:load`/`client:visible` directive", "Widget");
        }

        #[test]
                fn unsupported_client_directive_panics() {
            let source = ISLAND_SOURCE.replace("client:load", "client:idle");
            assert_lower_diag(&source, lower::L00, "unsupported `client:idle` directive", "client:idle");
        }

        #[test]
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
            assert_lower_diag(source, lower::L00, "unsupported attribute namespace `data:`", "data");
        }

        #[test]
                fn unresolved_component_reference_panics_with_clear_message() {
            let source = r#"
                export function App() {
                  return <div><Foo client:load /></div>;
                }
            "#;
            assert_lower_diag(source, lower::L00, "no `function Foo()` is declared in this module", "Foo");
        }

        // 7a C6(design-7a §4): docs/09 R1이 지목한 "고정 테스트". 예전에는 prop
        // 리터럴이 lower를 통과하는 것(값은 조용히 버려짐)을 단언했다 — 이제
        // 첫 리터럴 prop에서 R01로 거부되는 것을 단언한다(이름은 이력 보존을 위해 유지).
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
            assert_lower_diag(source, "LOOM-R01", "컴포넌트 prop `initial`는 아직 전달되지 않습니다", "initial={5}");
        }

        #[test]
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
            assert_lower_diag(source, lower::L00, "component prop value must be a number, boolean, string, or string-array literal", "parentCount()");
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
            let d = assert_lower_diag(source, lower::L01, "more than one action-bearing binding site targeting i32 signal `count`", "onClick={() => count.increment()}");
            // 주 위치는 **두 번째** 핸들러(7번째 줄), 첫 번째(6번째 줄)에 label.
            assert_eq!(d.line_col(d.items()[0].span).0, 7, "{d}");
            assert_eq!(d.items()[0].labels.len(), 1, "{d}");
            assert_eq!(d.line_col(d.items()[0].labels[0].0).0, 6, "{d}");
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
            lower::lower_module(&module).unwrap(); // must not error (7a C2: Result)
        }
    }

    /// Phase 7a C2(2026-10-17) — lower 거부의 Result화와 `SpanTable`.
    mod lower_result {
        use super::*;
        use crate::lower::BindingKind;

        #[test]
        fn public_api_returns_a_diagnostic_instead_of_panicking() {
            // 공개 API 4개 모두 lower 거부를 `Err`로 돌려준다(이전에는 panic).
            let src = "export function App() {\n  const n = signal(0);\n  return <span>{m()}</span>;\n}\n";
            let errs = [
                super::super::compile_tsx_to_rust("a.tsx", src).unwrap_err(),
                super::super::compile_tsx_to_rust_with_candidate("a.tsx", src, ListCandidate::K).unwrap_err(),
                super::super::compile_tsx_to_rust_graph("a.tsx", src, GraphMode::Islands).unwrap_err(),
                super::super::compile_tsx_to_single_island_rust("a.tsx", src, "App").unwrap_err(),
            ];
            for d in &errs {
                assert_eq!(d.items().len(), 1, "{d}");
                assert_eq!(d.items()[0].code, lower::L00, "{d}");
                assert_eq!(d.source_text(d.items()[0].span).as_deref(), Some("m"), "{d}");
                assert!(d.render().contains("--> a.tsx:3:17"), "{d}");
            }
            // 결정성: 같은 입력이면 렌더 결과가 같다.
            assert_eq!(errs[0].render(), errs[2].render());
        }

        #[test]
        fn conditional_whose_right_side_is_a_component_ref_is_a_diagnostic_not_unreachable() {
            // 옛 lower.rs:812의 `unreachable!()` — 사용자가 도달할 수 있었다.
            let src = r#"
                function W() {
                  const c = signal(0);
                  return <span>{c()}</span>;
                }
                export function App() {
                  const o = signal(false);
                  return <div>{o() && <W client:load />}</div>;
                }
            "#;
            assert_lower_diag(src, lower::L00, "must be a plain HTML element", "W");
            // 최소 차이 양성 짝: 감싸면 통과한다.
            let ok = src.replace("<W client:load />", "<p><W client:load /></p>");
            let module = parse_tsx(&ok);
            lower::lower_module(&module).unwrap();
        }

        #[test]
        fn conditional_whose_right_side_is_an_input_is_a_diagnostic_not_unreachable() {
            let src = r#"
                export function App() {
                  const o = signal(false);
                  const t = signal("");
                  return <div>{o() && <input value={t()} onInput={(e) => t.set(e.target.value)} />}</div>;
                }
            "#;
            assert_lower_diag(src, lower::L00, "must be a plain HTML element", "input");
        }

        #[test]
        fn rejections_point_at_the_offending_syntax() {
            // 기존 거부 몇 개를 골라 강조 위치를 고정한다(문구는 7a 이전과 같다).
            assert_lower_diag(
                "export function App() {\n  return <div {...p}>x</div>;\n}\n",
                lower::L00,
                "spread attributes are not supported",
                "...p",
            );
            assert_lower_diag(
                "export function App() {\n  const n = signal(0);\n  return <button onClick={() => n.decrement()}>-</button>;\n}\n",
                lower::L00,
                "unsupported signal method `.decrement()`",
                "decrement",
            );
            // (7a C6: `let`은 이제 R27이 먼저 잡으므로 이 L00 행은 `const`로 바꿨다.)
            assert_lower_diag(
                "export function App() {\n  const x = 1;\n  return <div />;\n}\n",
                lower::L00,
                "unsupported `const` declaration",
                "x = 1",
            );
            assert_lower_diag(
                "export function App() {\n  const xs = signal([\"a\", 1]);\n  return <div />;\n}\n",
                lower::L00,
                "array elements must be string literals",
                "1",
            );
            assert_lower_diag(
                "function A() { return <div />; }\n",
                lower::L00,
                "no exported `function NAME() { ... }` found",
                "",
            );
            let d = assert_lower_diag(
                "export function A() { return <div />; }\nexport function B() { return <div />; }\n",
                lower::L00,
                "more than one exported `function`",
                "B",
            );
            assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("A"));
            assert_lower_diag(
                "export function App() {\n  const xs = signal([\"a\"]);\n  return <ul>{xs().map((x) => <li>{x}</li>).length}</ul>;\n}\n",
                lower::L00,
                "unsupported JSX expression",
                "xs().map((x) => <li>{x}</li>).length",
            );
        }

        #[test]
        fn span_table_records_components_signals_refs_bindings_and_events_in_source_order() {
            let src = r#"
                function W() {
                  const c = signal(0);
                  return <button onClick={() => c.increment()}>{c()}</button>;
                }
                export function App() {
                  const open = signal(false);
                  const items = signal(["a"]);
                  const t = signal("");
                  return (
                    <div>
                      <button onClick={() => open.toggle()}>t</button>
                      {open() && <ul>{items().map((it) => <li>{it}</li>)}</ul>}
                      <input value={t()} onInput={(e) => t.set(e.target.value)} />
                      <W client:load />
                    </div>
                  );
                }
            "#;
            let (cm, module) = super::super::parse_tsx(TEST_FILE, src).unwrap();
            let (graph, spans, _) = lower::lower_module(&module).unwrap();
            assert_eq!(graph.root, "App");
            let d = Diagnostics::new(cm);
            let text = |sp| d.source_text(sp).unwrap();

            // 루트가 먼저 lower된다(lower_module의 순서 그대로).
            let comps: Vec<(&str, String)> = spans.components.iter().map(|(n, s)| (n.as_str(), text(*s))).collect();
            assert_eq!(comps, [("App", "App".to_string()), ("W", "W".to_string())]);

            let sigs: Vec<(&str, &str, String)> =
                spans.signals.iter().map(|(c, n, s)| (c.as_str(), n.as_str(), text(*s))).collect();
            assert_eq!(
                sigs,
                [
                    ("App", "open", "open".to_string()),
                    ("App", "items", "items".to_string()),
                    ("App", "t", "t".to_string()),
                    ("W", "c", "c".to_string()),
                ]
            );

            assert_eq!(spans.component_refs.len(), 1);
            assert_eq!((spans.component_refs[0].0.as_str(), spans.component_refs[0].1.as_str()), ("App", "W"));
            assert_eq!(text(spans.component_refs[0].2), "W");

            let binds: Vec<(&str, &str, BindingKind, bool, String)> = spans
                .bindings
                .iter()
                .map(|b| (b.component.as_str(), b.signal.as_str(), b.kind, b.in_row, text(b.span)))
                .collect();
            assert_eq!(
                binds,
                [
                    ("App", "items", BindingKind::List, false, "items().map((it) => <li>{it}</li>)".to_string()),
                    ("App", "open", BindingKind::Cond, false, "open()".to_string()),
                    ("App", "t", BindingKind::Input, false, "input".to_string()),
                    ("W", "c", BindingKind::Text, false, "c()".to_string()),
                ]
            );

            let events: Vec<(&str, &str, &str, String)> = spans
                .events
                .iter()
                .map(|e| (e.component.as_str(), e.signal.as_str(), e.attr, text(e.span)))
                .collect();
            assert_eq!(
                events,
                [
                    ("App", "open", "onClick", "onClick={() => open.toggle()}".to_string()),
                    ("App", "t", "onInput", "onInput={(e) => t.set(e.target.value)}".to_string()),
                    ("W", "c", "onClick", "onClick={() => c.increment()}".to_string()),
                ]
            );
        }

        #[test]
        fn span_table_marks_row_template_sites_as_in_row() {
            // row 원본(리스트 row 템플릿 안의 사이트) 구분 — 이후 R07 집계가 쓴다.
            let src = r#"
                export function App() {
                  const open = signal(false);
                  const outer = signal(["a"]);
                  const inner = signal(["x"]);
                  return (
                    <ul>{outer().map((item) => <li>{item}{open() && <ol>{inner().map((y) => <li>{y}</li>)}</ol>}</li>)}</ul>
                  );
                }
            "#;
            let (_cm, module) = super::super::parse_tsx(TEST_FILE, src).unwrap();
            let (_graph, spans, _) = lower::lower_module(&module).unwrap();
            let binds: Vec<(&str, BindingKind, bool)> =
                spans.bindings.iter().map(|b| (b.signal.as_str(), b.kind, b.in_row)).collect();
            assert_eq!(
                binds,
                [
                    ("inner", BindingKind::List, true),
                    ("open", BindingKind::Cond, true),
                    ("outer", BindingKind::List, false),
                ]
            );
        }
    }

    /// Phase 7a C3(2026-10-17) — codegen 백스톱 테스트(design-7a §3.3).
    ///
    /// codegen의 **사용자 도달** 패닉마다 한 행이다. 각 행은 세 가지를 단언한다.
    /// 1. **존재 단언**: check를 건너뛰고 codegen을 직접 부르면 그 패닉이 실제로
    ///    난다(입력이 정말 그 거부에 도달한다 — 아니면 아래 단언은 공허하다).
    /// 2. 공개 API는 **패닉 없이** check.rs의 진단 1건을 돌려주고, 그 진단이 문제
    ///    구문을 가리킨다(강조 토큰 + 렌더된 `파일:줄:열`).
    /// 3. 최소 차이 양성 짝은 공개 API로 끝까지 컴파일된다(check가 codegen보다
    ///    넓게 거부하지 않는다).
    ///
    /// codegen에 사용자 도달 거부가 새로 생기면 check.rs와 여기에 행을 더한다.
    /// 의도적으로 뺀 것: 1271(R07U, C7), lower가 이미 막는 것(2017·2820·3000·
    /// 3059·3295), 앞선 검사에 가려지는 것(2172 ← 2023, 2481 ← 2290).
    mod check_backstop {
        use super::*;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        #[derive(Clone, Copy, Debug)]
        enum Api {
            Root,
            Flatten,
            Islands,
            Single(&'static str),
        }

        fn run_api(api: Api, src: &str) -> Result<Compiled, Diagnostics> {
            match api {
                Api::Root => super::super::compile_tsx_to_rust(TEST_FILE, src),
                Api::Flatten => super::super::compile_tsx_to_rust_graph(TEST_FILE, src, GraphMode::Flatten),
                Api::Islands => super::super::compile_tsx_to_rust_graph(TEST_FILE, src, GraphMode::Islands),
                Api::Single(name) => super::super::compile_tsx_to_single_island_rust(TEST_FILE, src, name),
            }
        }

        /// check를 건너뛰고 lower → codegen을 직접 부른다. 패닉 메시지를 돌려준다.
        fn codegen_panic(api: Api, src: &str) -> Option<String> {
            let (_cm, module) = super::super::parse_tsx(TEST_FILE, src).unwrap();
            let (graph, _spans, _) = lower::lower_module(&module).expect("backstop inputs must pass lower");
            let r = catch_unwind(AssertUnwindSafe(|| match api {
                Api::Root => drop(codegen::generate(graph.root_component())),
                Api::Flatten => drop(codegen::generate_graph(&graph, GraphMode::Flatten)),
                Api::Islands => drop(codegen::generate_graph(&graph, GraphMode::Islands)),
                Api::Single(name) => drop(codegen::generate_single_island_client(&graph, name)),
            }));
            r.err().map(|e| {
                e.downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default()
            })
        }

        fn backstop(api: Api, src: &str, codegen_needle: &str, needle: &str, token: Option<&str>) -> Diagnostics {
            let msg = codegen_panic(api, src)
                .unwrap_or_else(|| panic!("존재 단언 실패: codegen이 이 입력을 받아들였다({api:?}) — 행이 공허하다"));
            assert!(msg.contains(codegen_needle), "codegen panicked elsewhere: {msg}");
            let result = catch_unwind(AssertUnwindSafe(|| run_api(api, src)))
                .unwrap_or_else(|_| panic!("공개 API가 패닉했다 — check.rs가 이 codegen 거부를 복제하지 않았다({codegen_needle})"));
            assert_check_diag(result, needle, token)
        }

        fn accepted(api: Api, src: &str) {
            if let Err(d) = run_api(api, src) {
                panic!("양성 짝이 거부됐다({api:?}):\n{d}");
            }
        }

        fn sub(template: &str, key: &str, value: &str) -> String {
            assert_eq!(template.matches(key).count(), 1, "{key}");
            template.replace(key, value)
        }

        // ---- 모드 거부 ----

        const FLAT: &str = r#"
            function Widget() {
              const @W@ = signal(0);
              return <button onClick={() => @W@.increment()}>{@W@()}</button>;
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

        #[test]
        fn flatten_signal_name_collision_552() {
            let neg = FLAT.replace("@W@", "count");
            backstop(Api::Flatten, &neg, "globally unique", "globally unique", Some("Widget"));
            accepted(Api::Flatten, &FLAT.replace("@W@", "wcount"));
            // 같은 입력이 Islands에서는 정상이다(모드 거부).
            accepted(Api::Islands, &neg);
        }

        #[test]
        fn flatten_double_reference_to_a_signal_bearing_component_552() {
            // 같은 컴포넌트를 두 번 펼치면 자기 signal과 충돌한다(design-7a R16:
            // signal 없는 컴포넌트는 Flatten에서 두 번 참조해도 무해).
            let neg = sub(&FLAT.replace("@W@", "w"), "<Widget client:load />", "<Widget client:load /><Widget client:visible />");
            let d = backstop(Api::Flatten, &neg, "globally unique", "second declaration of `w`", Some("Widget"));
            // 주 span은 **둘째** 참조다.
            assert!(d.render().contains(":11:"), "{d}");
        }

        #[test]
        fn islands_unreferenced_helper_625() {
            let src = r#"
                function Unused() {
                  const u = signal(0);
                  return <button onClick={() => u.increment()}>{u()}</button>;
                }
                export function App() {
                  const c = signal(0);
                  return <div><p>{c()}</p>@REF@</div>;
                }
            "#;
            // 7a C7: 이 거부는 lower의 R47(모드 무관 도달성)이 흡수했다 — check.rs의
            // 625 복제는 걷었다. lower가 먼저 막으므로 `backstop`(lower 통과 전제)을
            // 쓸 수 없고, codegen 패닉의 존재는 IR을 직접 만들어 단언한다.
            let neg = sub(src, "@REF@", "");
            for api in [Api::Islands, Api::Flatten, Api::Root] {
                let e = run_api(api, &neg).err().unwrap_or_else(|| panic!("{api:?}가 받아 버림"));
                assert_diag(e, lower::R47, "헬퍼 `Unused`가 루트에서 도달할 수 없습니다", Some("Unused"));
            }
            accepted(Api::Islands, &sub(src, "@REF@", "<Unused client:load />"));
            // 존재 단언: 참조를 뺀 그래프를 codegen(Islands)에 직접 넣으면 625가 난다.
            let (_cm, module) = super::super::parse_tsx(TEST_FILE, &sub(src, "@REF@", "<Unused client:load />")).unwrap();
            let (mut graph, _, _) = lower::lower_module(&module).unwrap();
            let app = graph.components.iter_mut().find(|c| c.name == "App").unwrap();
            let ir::JsxNode::Element { children, .. } = &mut app.root else { panic!() };
            children.retain(|c| !matches!(c, ir::JsxNode::ComponentRef { .. }));
            let r = catch_unwind(AssertUnwindSafe(|| codegen::generate_graph(&graph, GraphMode::Islands)));
            let msg = r.err().and_then(|e| e.downcast_ref::<String>().cloned()).expect("625 패닉이 나야 한다");
            assert!(msg.contains("never referenced"), "{msg}");
        }

        #[test]
        fn islands_input_binding_634() {
            let src = r#"
                function Form() {
                  const t = signal("");
                  return <div><input value={t()} onInput={(e) => t.set(e.target.value)} /></div>;
                }
                export function App() {
                  return <div><Form client:load /></div>;
                }
            "#;
            let d = backstop(Api::Islands, src, "<input> two-way binding", "<input> two-way binding", Some("input"));
            assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("Form"));
            accepted(Api::Flatten, src);
        }

        const SINGLE: &str = r#"
            function W() {
              const n = signal(0);
              return <button onClick={() => n.increment()}>{n()}</button>;
            }
            export function App() {
              return <div><W client:load /></div>;
            }
        "#;

        #[test]
        fn single_island_unknown_target_848() {
            backstop(Api::Single("Nope"), SINGLE, "no component named `Nope`", "no component named `Nope`", None);
            accepted(Api::Single("W"), SINGLE);
        }

        #[test]
        fn single_island_target_not_activated_851() {
            backstop(Api::Single("App"), SINGLE, "is not Island-activated", "is not Island-activated", Some("App"));
        }

        #[test]
        fn fully_static_island_1196() {
            let src = r#"
                function Badge() {
                  return <span>static</span>;
                }
                export function App() {
                  const c = signal(0);
                  return <div><button onClick={() => c.increment()}>{c()}</button><Badge client:load /></div>;
                }
            "#;
            backstop(Api::Islands, src, "has no dynamic content at all", "has no dynamic content at all", Some("Badge"));
            backstop(Api::Single("Badge"), src, "has no dynamic content at all", "has no dynamic content at all", Some("Badge"));
            accepted(Api::Flatten, src);
        }

        // ---- 노드 모양 거부: eager 경로와 조건부 ----

        // 7a C7: 템플릿에서 헬퍼 `W`를 뺐다 — 참조되지 않는 헬퍼는 이제 R47이고,
        // 단일 컴포넌트 경로(Root)의 참조는 G01이다. `W`가 필요한 행은 `with_w`로
        // 붙이고 Islands 경로로 판정한다(같은 codegen 패닉에 닿는다).
        const NODE: &str = r#"
            export function App() {
              const o = signal(false);
              const q = signal(false);
              const n = signal(0);
              const xs = signal(["a"]);
              return (
                <div>
                  <button onClick={() => o.toggle()}>t</button>
                  @BASE@
                  <section>@C@</section>
                </div>
              );
            }
        "#;

        const W_DEF: &str = "function W() {\n  const w = signal(0);\n  return <b>{w()}</b>;\n}\n";
        fn with_w(src: &str) -> String {
            format!("{W_DEF}{src}")
        }

        /// 7a C7(R07/R07U): signal마다 binding 사이트가 정확히 1곳이어야 한다. 그래서
        /// 템플릿의 기본 표시(`@BASE@`)는 **스니펫이 읽지 않는** signal만 표시한다.
        fn base_for(snippet: &str, displays: &[(&str, &str)]) -> String {
            displays
                .iter()
                .filter(|(name, _)| !reads(snippet, name))
                .map(|(_, markup)| *markup)
                .collect::<Vec<_>>()
                .join("")
        }
        /// `snippet`이 `{name}()`을 읽는가(앞 글자가 식별자 문자가 아닐 때만).
        fn reads(snippet: &str, name: &str) -> bool {
            let pat = format!("{name}()");
            snippet.match_indices(&pat).any(|(i, _)| {
                !snippet[..i].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_')
            })
        }
        const NODE_DISPLAYS: &[(&str, &str)] = &[
            ("n", "<p>{n()}</p>"),
            ("xs", "<ul>{xs().map((x) => <li>{x}</li>)}</ul>"),
            ("o", "<aside>{o() && <i>o</i>}</aside>"),
            ("q", "<aside>{q() && <i>q</i>}</aside>"),
        ];
        fn node_src(snippet: &str) -> String {
            sub(&sub(NODE, "@BASE@", &base_for(snippet, NODE_DISPLAYS)), "@C@", snippet)
        }

        fn node_case(api: Api, neg: &str, pos: &str, codegen_needle: &str, needle: &str, token: &str) -> Diagnostics {
            let d = backstop(api, &node_src(neg), codegen_needle, needle, Some(token));
            accepted(api, &node_src(pos));
            d
        }

        #[test]
        fn eager_text_binding_requires_i32_2798() {
            node_case(Api::Root, "<p>{xs()}</p>", "<p>{n()}</p>", "requires an I32 signal", "requires an I32 signal", "xs()");
        }

        #[test]
        fn eager_list_row_must_be_an_element_2811() {
            // 7a C7: Root 경로에서는 이 입력이 G01(컴포넌트 참조)로 먼저 막힌다 —
            // 같은 codegen 거부에 닿는 Islands 경로로 판정한다. 양성 짝은 `W`를
            // row 밖에서 한 번 참조한다(R47 회피, 참조 1회 유지).
            let neg = with_w(&node_src("<ol>{xs().map((x) => <W client:load />)}</ol>"));
            backstop(Api::Islands, &neg, "list row template must be a single element", "list row template must be a single element", Some("W"));
            accepted(Api::Islands, &with_w(&node_src("<ol>{xs().map((x) => <li>{x}</li>)}</ol><W client:load />")));
            assert_eq!(run_api(Api::Root, &neg).err().expect("G01").items()[0].code, check::G01);
        }

        #[test]
        fn eager_mixed_children_3336() {
            node_case(Api::Root, "<p>count: {n()}</p>", "<p>{n()}</p>", "mixes a dynamic binding", "mixes a dynamic binding", "p");
        }

        #[test]
        fn conditional_wrapper_onclick_3040() {
            let d = node_case(
                Api::Root,
                "{o() && <button onClick={() => n.increment()}>go</button>}",
                "{o() && <button>go</button>}",
                "may not carry its own",
                "may not carry its own",
                "onClick={() => n.increment()}",
            );
            assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("o()"));
        }

        #[test]
        fn conditional_text_binding_requires_i32_2003() {
            node_case(
                Api::Root,
                "{o() && <div><p>{xs()}</p></div>}",
                "{o() && <div><p>{n()}</p></div>}",
                "requires an I32 signal",
                "requires an I32 signal",
                "xs()",
            );
        }

        #[test]
        fn conditional_list_row_must_be_an_element_2009() {
            // 7a C7: 2811과 같은 이유로 Islands 경로.
            let neg = with_w(&node_src("{o() && <div><ul>{xs().map((x) => <W client:load />)}</ul></div>}"));
            backstop(Api::Islands, &neg, "list row template must be a single element", "list row template must be a single element", Some("W"));
            accepted(
                Api::Islands,
                &with_w(&node_src("<div>{o() && <div><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>}</div><W client:load />")),
            );
        }

        #[test]
        fn conditional_list_row_must_be_item_only_2012() {
            node_case(
                Api::Root,
                "{o() && <div><ul>{xs().map((x) => <li>{x} !</li>)}</ul></div>}",
                "{o() && <div><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>}",
                "only supports list rows shaped exactly",
                "only supports list rows shaped exactly",
                "li",
            );
        }

        #[test]
        fn conditional_inner_element_onclick_2023() {
            node_case(
                Api::Root,
                "{o() && <div><button onClick={() => n.increment()}>+</button></div>}",
                "{o() && <div><button>+</button></div>}",
                "자기 `onClick`을 가질 수 없다",
                "자기 `onClick`을 가질 수 없다",
                "onClick={() => n.increment()}",
            );
        }

        #[test]
        fn conditional_unsupported_node_2031() {
            node_case(
                Api::Root,
                r#"{o() && <div>{sharedValue("k", 0)}</div>}"#,
                "{o() && <div>k</div>}",
                "안에서 아직 지원하지 않는 노드다",
                "안에서 아직 지원하지 않는 노드다",
                r#"sharedValue("k", 0)"#,
            );
            // lower는 then 가지의 **직계** 조건부만 막는다 — 한 겹 아래는 여기서.
            node_case(
                Api::Root,
                // (7a C7: 안쪽 조건부는 `q`다 — 같은 `o`를 두 번 읽으면 이제 R07이 먼저다.)
                "{o() && <div><p>{q() && <b>y</b>}</p></div>}",
                "{o() && <div><p><b>y</b></p></div>}",
                "안에서 아직 지원하지 않는 노드다",
                "안에서 아직 지원하지 않는 노드다",
                "q()",
            );
        }

        #[test]
        fn conditional_component_ref_is_mode_dependent_2031() {
            // Islands에서는 조건부 안의 참조가 미지원 노드지만, Flatten에서는 참조가
            // 펼쳐져(`<b>{w()}</b>`) 평범한 지연 내용이 된다 — check도 펼친 뒤 판정한다.
            let neg = with_w(&node_src("{o() && <div><W client:load /></div>}"));
            backstop(Api::Islands, &neg, "안에서 아직 지원하지 않는 노드다", "안에서 아직 지원하지 않는 노드다", Some("W"));
            accepted(Api::Flatten, &neg);
        }

        #[test]
        fn conditional_binding_with_siblings_2222() {
            node_case(
                Api::Root,
                "{o() && <div><p>{n()} x</p></div>}",
                "{o() && <div><p>{n()}</p></div>}",
                "안에서 아직 지원하지 않는 노드다",
                "유일한 자식이어야 한다",
                "n()",
            );
            node_case(
                Api::Root,
                "{o() && <div>{n()} x</div>}",
                "{o() && <div>{n()}</div>}",
                "안에서 아직 지원하지 않는 노드다",
                "유일한 자식이어야 한다",
                "n()",
            );
        }

        // ---- row 템플릿 ----

        const ROW: &str = r#"
            export function App() {
              const outer = signal(["a", "bb"]);
              const open = signal(false);
              const shut = signal(false);
              const inner = signal(["p", "q"]);
              const inner2 = signal(["r"]);
              const n = signal(0);
              return (
                <div>
                  <button onClick={() => open.toggle()}>t</button>
                  @BASE@
                  <ul>{outer().map((it) => <li>@HEAD@@ROW@</li>)}</ul>
                </div>
              );
            }
        "#;

        const ROW_OK: &str = r#"{open() && <div class="panel"><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}"#;

        fn row_src(head: &str, row: &str) -> String {
            const ROW_DISPLAYS: &[(&str, &str)] = &[
                ("n", "<p>{n()}</p>"),
                ("shut", "<section>{shut() && <i>s</i>}</section>"),
                ("inner2", "<ol>{inner2().map((y) => <li>{y}</li>)}</ol>"),
                ("open", "<aside>{open() && <i>o</i>}</aside>"),
                ("inner", "<ol>{inner().map((z) => <li>{z}</li>)}</ol>"),
            ];
            sub(&sub(&sub(ROW, "@BASE@", &base_for(row, ROW_DISPLAYS)), "@HEAD@", head), "@ROW@", row)
        }

        fn row_case(neg_row: &str, pos_row: &str, codegen_needle: &str, needle: &str, token: &str) -> Diagnostics {
            let d = backstop(Api::Root, &row_src("{it}", neg_row), codegen_needle, needle, Some(token));
            accepted(Api::Root, &row_src("{it}", pos_row));
            d
        }

        #[test]
        fn row_template_baseline_compiles() {
            accepted(Api::Root, &row_src("{it}", ROW_OK));
        }

        #[test]
        fn row_must_start_with_item_2263() {
            backstop(Api::Root, &row_src("x", ROW_OK), "`{item}`으로 시작해야 한다", "`{item}`으로 시작해야 한다", Some("li"));
        }

        #[test]
        fn row_second_conditional_2273() {
            let d = row_case(
                &format!("{ROW_OK}{{shut() && <div><b>x</b></div>}}"),
                ROW_OK,
                "조건부는 아직 하나만 지원한다",
                "조건부는 아직 하나만 지원한다",
                "shut()",
            );
            assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("open()"));
        }

        #[test]
        fn row_conditional_wrapper_is_a_binding_2281() {
            row_case(
                "{open() && <ul>{inner().map((x) => <li>{x}</li>)}</ul>}",
                "{open() && <div><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}",
                "binding 노드로 바로 쓸 수는 없다",
                "binding 노드로 바로 쓸 수는 없다",
                "ul",
            );
        }

        #[test]
        fn row_unsupported_child_2290() {
            row_case(
                &format!("{ROW_OK}<b>{{it}}</b>"),
                &format!("{ROW_OK}<b>x</b>"),
                "row 템플릿에서 아직 지원하지 않는 노드다",
                "row 템플릿에서 아직 지원하지 않는 노드다",
                "b",
            );
        }

        #[test]
        fn row_nested_list_row_must_be_an_element_2307() {
            // 7a C7: 2811과 같은 이유로 Islands 경로.
            let neg = with_w(&row_src("{it}", "{open() && <div><ul>{inner().map((x) => <W client:load />)}</ul></div>}"));
            backstop(Api::Islands, &neg, "list row template must be a single element", "list row template must be a single element", Some("W"));
            accepted(Api::Root, &row_src("{it}", ROW_OK));
        }

        #[test]
        fn row_nested_list_row_must_be_item_only_2310() {
            row_case(
                "{open() && <div><ul>{inner().map((x) => <li>{x} !</li>)}</ul></div>}",
                ROW_OK,
                "`<tag>{item}</tag>` 형태여야 한다",
                "`<tag>{item}</tag>` 형태여야 한다",
                "li",
            );
        }

        #[test]
        fn row_second_nested_list_2313() {
            let d = row_case(
                "{open() && <div><ul>{inner().map((x) => <li>{x}</li>)}</ul><ol>{inner2().map((y) => <li>{y}</li>)}</ol></div>}",
                ROW_OK,
                "리스트는 아직 하나만 지원한다",
                "리스트는 아직 하나만 지원한다",
                "inner2()",
            );
            assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("inner()"));
        }

        #[test]
        fn row_conditional_element_onclick_2319() {
            row_case(
                "{open() && <div><button onClick={() => open.toggle()}>x</button><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}",
                "{open() && <div><button>x</button><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}",
                "자기 `onClick`을 가질 수 없다",
                "자기 `onClick`을 가질 수 없다",
                "onClick={() => open.toggle()}",
            );
        }

        #[test]
        fn row_conditional_unsupported_node_2325() {
            row_case(
                "{open() && <div><p>{it}</p><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}",
                "{open() && <div><p>x</p><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}",
                "row 안 조건부에서 아직 지원하지 않는 노드다",
                "row 안 조건부에서 아직 지원하지 않는 노드다",
                "it",
            );
        }

        #[test]
        fn row_conditional_list_with_siblings_2560() {
            row_case(
                "{open() && <div><ul>{inner().map((x) => <li>{x}</li>)} tail</ul></div>}",
                ROW_OK,
                "unexpected conditional child",
                "유일한 자식이어야 한다",
                "inner()",
            );
        }

        #[test]
        fn node_span_table_is_aligned_with_the_ir_for_every_backstop_source() {
            // check.rs의 노드 → span 대응은 lower의 pre-order 기록과 IR 순회가
            // 같은 순서라는 데 기대고, 어긋나면 내부 오류로 멈춘다. 위 원본들이
            // 전부 그 단언을 통과한다는 것 자체가 이 테스트다.
            //
            // (7a C6: 여기 있던 "return 둘" 원본은 이제 R22로 거부된다 — 루트는
            // 항상 첫 `return`이므로 노드 표를 다시 비우는 경로 자체가 없어졌다.)
            let two_returns = r#"
                export function App() {
                  const n = signal(0);
                  return <p>{n()}</p>;
                  return <div><span>{n()}</span></div>;
                }
            "#;
            let e = run_api(Api::Root, two_returns).err().expect("R22");
            assert_diag(e, "LOOM-R22", "`return` 뒤에 문장이 있습니다", Some("return <div><span>{n()}</span></div>;"));
        }
    }

    /// 7a C5(2026-10-17, design-7a §1.1·§4): 인자 묶음 R03·R04·R18·R19·R26.
    /// 거부 하나당 부정 스니펫 하나 이상. 행마다 다음을 단언한다.
    /// - lower 진단이 정확히 1건이고 코드·문구·**강조 토큰**·렌더 `파일:줄:열`이 맞다.
    /// - 공개 API(Flatten 그래프)도 패닉 없이 같은 코드의 진단 1건을 돌려준다.
    /// - 최소 차이 양성 짝(문제 인자만 다른 스니펫)이 끝까지 컴파일된다.
    mod c5_argument_rejections {
        use super::*;

        /// App 본문(선언 + return JSX)만 바꿔 끼우는 틀.
        // 7a C6: W25(import 누락 경고)가 생겨 양성 짝의 "경고 0" 단언이 깨지지
        // 않도록 두 틀 모두 저작 표면 callee 셋을 import한다.
        const IMPORT: &str = "import { signal, sharedSignal, sharedValue } from \"loom\";\n";
        fn app(body: &str) -> String {
            format!("{IMPORT}export function App() {{\n{body}\n}}\n")
        }
        /// 위젯 참조 줄만 바꿔 끼우는 틀(디렉티브·prop 검사용).
        fn with_widget(r#ref: &str) -> String {
            format!(
                "{IMPORT}function Widget() {{\n  const n = signal(0);\n  return <button onClick={{() => n.increment()}}>{{n()}}</button>;\n}}\n\
                 export function App() {{\n  return <div>{ref}</div>;\n}}\n",
                r#ref = r#ref
            )
        }

        const COUNTER: &str = "  const c = signal(0);\n  return <button onClick={() => c.increment(@)}>{c()}</button>;";
        const ROTATE: &str = "  const xs = signal([\"a\", \"b\"]);\n  return <div><button onClick={() => xs.rotate(@)}>r</button><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>;";
        const TOGGLE: &str = "  const o = signal(false);\n  return <div><button onClick={() => o.toggle(@)}>t</button>{o() && <p>x</p>}</div>;";
        const SIGNAL: &str = "  const c = signal(@);\n  return <button onClick={() => c.increment()}>{c()}</button>;";
        const SHARED_SIGNAL: &str = "  const c = sharedSignal(\"k\", @);\n  return <button onClick={() => c.increment()}>{c()}</button>;";
        const SHARED_VALUE: &str = "  return <p>{sharedValue(\"k\", @)}</p>;";
        const LEN_GT: &str = "  const xs = signal([\"a\", \"bb\"]);\n  const o = signal(false);\n  return <div><button onClick={() => o.toggle()}>t</button><ul>{xs().map((it) => <li>{it}{o() && it.length > @ && <b>w</b>}</li>)}</ul></div>;";
        const APPEND: &str = "  const xs = signal([\"a\"]);\n  return <div><button onClick={() => xs.append(@)}>+</button><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>;";
        const INPUT_SET: &str = "  const t = signal(\"\");\n  return <div><input value={t()} onInput={(e) => t.set(@)} /></div>;";
        const MAP: &str = "  const xs = signal([\"a\"]);\n  return <ul>{xs().map(@)}</ul>;";
        const INPUT_CHILD: &str = "  const t = signal(\"\");\n  return <div>@</div>;";

        /// (코드, 부정 소스, 문구 부분 문자열, 강조 토큰, 양성 짝 소스)
        fn cases() -> Vec<(&'static str, String, &'static str, &'static str, Option<String>)> {
            let a = |tpl: &str, arg: &str| app(&tpl.replace('@', arg));
            vec![
                // R03
                ("LOOM-R03", a(COUNTER, "5"), "`c.increment(…)`는 지금 +1로 컴파일됩니다", "5", Some(a(COUNTER, ""))),
                ("LOOM-R03", a(ROTATE, "1"), "`.rotate()`는 인자를 받지 않습니다", "1", Some(a(ROTATE, ""))),
                // R04
                ("LOOM-R04", a(TOGGLE, "true"), "`o.toggle(…)`는 지금 반전으로 컴파일됩니다", "true", Some(a(TOGGLE, ""))),
                // R18
                (
                    "LOOM-R18",
                    with_widget("<Widget client:load={false} />"),
                    "`client:load`는 값을 받지 않습니다",
                    "{false}",
                    Some(with_widget("<Widget client:load />")),
                ),
                (
                    "LOOM-R18",
                    with_widget("<Widget client:load client:visible />"),
                    "디렉티브가 둘입니다(`client:load` + `client:visible`)",
                    "client:visible",
                    Some(with_widget("<Widget client:visible />")),
                ),
                // R19 — 잘림/포화
                ("LOOM-R19", a(SIGNAL, "1.7"), "`1.7`는 i32 정수가 아닙니다(지금은 1로", "1.7", Some(a(SIGNAL, "2"))),
                ("LOOM-R19", a(SIGNAL, "3e10"), "`3e10`는 i32 정수가 아닙니다(지금은 2147483647로", "3e10", Some(a(SIGNAL, "3e5"))),
                ("LOOM-R19", a(SHARED_SIGNAL, "1.5"), "`1.5`는 i32 정수가 아닙니다", "1.5", Some(a(SHARED_SIGNAL, "1"))),
                ("LOOM-R19", a(SHARED_VALUE, "1.5"), "`1.5`는 i32 정수가 아닙니다", "1.5", Some(a(SHARED_VALUE, "1"))),
                // prop 리터럴도 같은 `lit_i32`를 지난다. 양성 짝(`n={2}`)은 R01(C6, props
                // 거부) 대상이 되므로 두지 않는다 — 같은 헬퍼의 양성은 위 signal 행이 덮는다.
                ("LOOM-R19", with_widget("<Widget client:load n={2.5} />"), "`2.5`는 i32 정수가 아닙니다", "2.5", None),
                // R19 — 길이 비교: 표기 정규화(오컴파일 아님) 문구
                ("LOOM-R19", a(LEN_GT, "1.5"), "길이 비교에는 0 이상의 정수 리터럴을 쓰십시오(`1.5`)", "1.5", Some(a(LEN_GT, "1"))),
                // R26 — 인자 개수
                ("LOOM-R26", a(SIGNAL, "4, 9"), "`signal()`는 인자 1개를 받습니다(받은 것 2개", "9", Some(a(SIGNAL, "4"))),
                ("LOOM-R26", a(APPEND, "\"a\", \"b\""), "`.append()`는 인자 1개를 받습니다(받은 것 2개", "\"b\"", Some(a(APPEND, "\"a\""))),
                (
                    "LOOM-R26",
                    a(INPUT_SET, "e.target.value, 1"),
                    "`.set()`는 인자 1개를 받습니다(받은 것 2개",
                    "1",
                    Some(a(INPUT_SET, "e.target.value")),
                ),
                (
                    "LOOM-R26",
                    a(MAP, "(x) => <li>{x}</li>, this"),
                    "`.map()`는 인자 1개를 받습니다(받은 것 2개",
                    "this",
                    Some(a(MAP, "(x) => <li>{x}</li>")),
                ),
                // R26 — <input> 자식(공백 아닌 텍스트, 주석 식 모두; R02와 같은 경계)
                (
                    "LOOM-R26",
                    a(INPUT_CHILD, "<input value={t()} onInput={(e) => t.set(e.target.value)}>x</input>"),
                    "<input>은 자식을 가질 수 없습니다",
                    "x",
                    Some(a(INPUT_CHILD, "<input value={t()} onInput={(e) => t.set(e.target.value)} />")),
                ),
                (
                    "LOOM-R26",
                    a(INPUT_CHILD, "<input value={t()} onInput={(e) => t.set(e.target.value)}>{/* c */}</input>"),
                    "<input>은 자식을 가질 수 없습니다",
                    "{/* c */}",
                    Some(a(INPUT_CHILD, "<input value={t()} onInput={(e) => t.set(e.target.value)}> </input>")),
                ),
            ]
        }

        #[test]
        fn each_argument_rejection_points_at_the_offending_argument_and_has_a_compiling_positive_pair() {
            let cases = cases();
            // 개수 단언(행이 조용히 빠지지 않게): R03 2, R04 1, R18 2, R19 6, R26 6.
            assert_eq!(cases.len(), 17);
            for (code, neg, needle, token, pos) in &cases {
                assert_lower_diag(neg, code, needle, token);
                let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, neg, GraphMode::Flatten)
                    .err()
                    .unwrap_or_else(|| panic!("공개 API가 받아 버림:\n{neg}"));
                assert_diag(e, code, needle, Some(token));
                if let Some(pos) = pos {
                    let ok = super::super::compile_tsx_to_rust_graph(TEST_FILE, pos, GraphMode::Flatten)
                        .unwrap_or_else(|d| panic!("양성 짝이 실패:\n{pos}\n{d}"));
                    assert!(ok.warnings.is_empty());
                }
            }
        }

        #[test]
        fn second_directive_carries_a_label_on_the_first() {
            let d = assert_lower_diag(
                &with_widget("<Widget client:visible client:load />"),
                "LOOM-R18",
                "디렉티브가 둘입니다(`client:visible` + `client:load`)",
                "client:load",
            );
            let labels = &d.items()[0].labels;
            assert_eq!(labels.len(), 1, "{d}");
            assert_eq!(d.source_text(labels[0].0).as_deref(), Some("client:visible"), "{d}");
        }

        #[test]
        fn length_comparison_wording_is_normalisation_not_truncation() {
            // design-7a §1.1 R19: LenGt는 오컴파일이 아니므로 "잘림" 문구를 쓰지 않는다.
            let d = lower_diag(&app(&LEN_GT.replace('@', "1.5")));
            assert!(!d.items()[0].message.contains("잘리"), "{d}");
        }

        #[test]
        fn accepted_integer_literals_lower_to_the_same_values_as_before() {
            // 받는 입력의 IR은 C5 이전(`*value as i32`/`as usize`)과 같다: 정수 표기·지수 표기.
            let m = parse_tsx(&app(&SIGNAL.replace('@', "3e5")));
            let g = lower::lower_module(&m).unwrap().0;
            assert!(matches!(g.components[0].signals[0].init, ir::SignalInit::I32(300000)));
            let max = app(&SIGNAL.replace('@', "2147483647"));
            let g = lower::lower_module(&parse_tsx(&max)).unwrap().0;
            assert!(matches!(g.components[0].signals[0].init, ir::SignalInit::I32(i32::MAX)));
            let over = app(&SIGNAL.replace('@', "2147483648"));
            assert_lower_diag(&over, "LOOM-R19", "(지금은 2147483647로", "2147483648");
        }
    }

    /// 7a C6(2026-10-17, design-7a §1.1·§4): 구조 묶음 R01·R02·R05·R06·R22·R24·
    /// R25·R27과 경고 W25. 행마다 c5와 같은 세 가지를 단언한다 — lower 진단
    /// 1건(코드·문구·**강조 토큰**·렌더 `파일:줄:열`), 공개 API(Flatten)의 같은
    /// 진단, 최소 차이 양성 짝의 성공(경고 0).
    mod c6_structure_rejections {
        use super::*;

        const IMPORT: &str = "import { signal, sharedSignal, sharedValue } from \"loom\";\n";
        fn app(body: &str) -> String {
            format!("{IMPORT}export function App() {{\n{body}\n}}\n")
        }
        /// `Widget` 헬퍼(매개변수 `params`) + App이 `reference`로 그것을 참조.
        fn widget(params: &str, reference: &str) -> String {
            format!(
                "{IMPORT}function Widget({params}) {{\n  const n = signal(0);\n  return <button onClick={{() => n.increment()}}>{{n()}}</button>;\n}}\n\
                 export function App() {{\n  return <div>{reference}</div>;\n}}\n"
            )
        }
        /// 최상위 항목 `item`을 App 앞에 끼운다(R25). import 행은 `top_bare`로
        /// 넣어 `IMPORT`와 겹치지 않게 한다.
        fn top(item: &str) -> String {
            top_bare(&format!("{IMPORT}{item}"))
        }
        fn top_bare(item: &str) -> String {
            format!("{item}\nexport function App() {{\n  const c = signal(0);\n  return <button onClick={{() => c.increment()}}>{{c()}}</button>;\n}}\n")
        }

        const LIST: &str = "  const xs = signal([\"a\"]);\n  return <div><button @>b</button><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>;";
        const COUNTER: &str = "  const c = signal(0);\n  return <button onClick={@() => c.increment()}>{c()}</button>;";
        const STATIC: &str = "  const c = signal(0);\n  return <div><button onClick={() => c.increment()}>{c()}</button><p @>x</p></div>;";
        const INPUT: &str = "  const t = signal(\"\");\n  return <div><input @ /></div>;";
        const MAP: &str = "  const xs = signal([\"a\"]);\n  return <ul>{xs().map(@(x) => <li>{x}</li>)}</ul>;";
        const VALUE: &str = "value={t()}";
        const ONINPUT: &str = "onInput={(e) => t.set(e.target.value)}";
        const HELPER: &str = "function @() {\n  const h = signal(0);\n  return <button onClick={() => h.increment()}>{h()}</button>;\n}\n";

        /// (코드, 부정 소스, 문구 부분 문자열, 강조 토큰, 양성 짝 소스)
        fn cases() -> Vec<(&'static str, String, &'static str, &'static str, Option<String>)> {
            let a = |tpl: &str, arg: &str| app(&tpl.replace('@', arg));
            let input = |attrs: &str| a(INPUT, attrs);
            let base = top("");
            let helper_app = |name: &str, reference: &str| {
                format!(
                    "{IMPORT}{}export function App() {{\n  const c = signal(0);\n  return <div><button onClick={{() => c.increment()}}>{{c()}}</button>{reference}</div>;\n}}\n",
                    HELPER.replace('@', name)
                )
            };
            vec![
                // R01 — prop(리터럴 검사 뒤), 매개변수
                ("LOOM-R01", widget("", "<Widget client:load initial={5} />"), "컴포넌트 prop `initial`는 아직 전달되지 않습니다", "initial={5}", Some(widget("", "<Widget client:load />"))),
                ("LOOM-R01", widget("", "<Widget client:load label=\"hi\" />"), "컴포넌트 prop `label`는", "label=\"hi\"", Some(widget("", "<Widget client:load />"))),
                ("LOOM-R01", widget("p", "<Widget client:load />"), "컴포넌트 함수 `Widget`는 매개변수를 받을 수 없습니다", "p", Some(widget("", "<Widget client:load />"))),
                // R02 — children(공백 아닌 자식, 주석 식)
                ("LOOM-R02", widget("", "<Widget client:load><p>x</p></Widget>"), "children은 전달되지 않고 SSR에서도 사라집니다", "<p>x</p>", Some(widget("", "<Widget client:load> </Widget>"))),
                ("LOOM-R02", widget("", "<Widget client:load>{/* c */}</Widget>"), "children은 전달되지 않고", "{/* c */}", Some(widget("", "<Widget client:load />"))),
                // R05 — onClick 두 번
                (
                    "LOOM-R05",
                    a(LIST, "onClick={() => xs.append(\"z\")} onClick={() => xs.rotate()}"),
                    "<button>에 `onClick`이 두 번 있습니다",
                    "onClick={() => xs.rotate()}",
                    Some(a(LIST, "onClick={() => xs.rotate()}")),
                ),
                // R06 — class/className 합산, 같은 이름, <input>의 value/onInput
                ("LOOM-R06", a(STATIC, "class=\"a\" className=\"b\""), "<p>에 `className`가 두 번 있습니다", "className=\"b\"", Some(a(STATIC, "class=\"a\""))),
                ("LOOM-R06", a(STATIC, "class=\"a\" class=\"b\""), "<p>에 `class`가 두 번 있습니다", "class=\"b\"", Some(a(STATIC, "className=\"b\""))),
                (
                    "LOOM-R06",
                    input(&format!("{VALUE} {VALUE} {ONINPUT}")),
                    "<input>에 `value`가 두 번 있습니다",
                    "value={t()}",
                    Some(input(&format!("{VALUE} {ONINPUT}"))),
                ),
                (
                    "LOOM-R06",
                    input(&format!("{VALUE} {ONINPUT} {ONINPUT}")),
                    "<input>에 `onInput`가 두 번 있습니다",
                    ONINPUT,
                    Some(input(&format!("{ONINPUT} {VALUE}"))),
                ),
                // R22 — return 뒤의 문장(선언, 둘째 return)
                (
                    "LOOM-R22",
                    app("  const c = signal(0);\n  return <p>{c()}</p>;\n  const d = signal(1);"),
                    "`return` 뒤에 문장이 있습니다",
                    "const d = signal(1);",
                    Some(app("  const c = signal(0);\n  const d = signal(1);\n  return <div><p>{c()}</p><p>{d()}</p></div>;")),
                ),
                (
                    "LOOM-R22",
                    app("  const c = signal(0);\n  return <p>{c()}</p>;\n  return <div>{c()}</div>;"),
                    "첫 `return`만 루트가 될 수 있습니다",
                    "return <div>{c()}</div>;",
                    Some(app("  const c = signal(0);\n  return <p>{c()}</p>;")),
                ),
                // R24 — 소문자 태그 = 선언된 컴포넌트 이름
                (
                    "LOOM-R24",
                    helper_app("helper", "<helper />"),
                    "`<helper/>`는 HTML 태그로 해석되는데 `function helper()`가 선언돼 있습니다",
                    "helper",
                    Some(helper_app("Helper", "<Helper client:load />")),
                ),
                // R25 — 지원하지 않는 최상위 항목
                ("LOOM-R25", top("export default 1;"), "최상위 `export default`는 지원하지 않습니다", "export default 1;", Some(base.clone())),
                ("LOOM-R25", top("const x = 1;"), "최상위 `const` 선언는 지원하지 않습니다", "const x = 1;", Some(base.clone())),
                ("LOOM-R25", top("let x = 1;"), "최상위 `let` 선언는", "let x = 1;", Some(base.clone())),
                ("LOOM-R25", top("type T = number;"), "최상위 `type` 별칭는", "type T = number;", Some(base.clone())),
                ("LOOM-R25", top("interface I { a: number }"), "최상위 `interface`는", "interface I { a: number }", Some(base.clone())),
                ("LOOM-R25", top("class K {}"), "최상위 `class`는", "class K {}", Some(base.clone())),
                ("LOOM-R25", top("export const y = 1;"), "최상위 `export const`는", "export const y = 1;", Some(base.clone())),
                ("LOOM-R25", top(";"), "최상위 빈 문장(`;`)는", ";", Some(base.clone())),
                ("LOOM-R25", top_bare("import x from \"y\";"), "최상위 `import … from \"y\"`는", "\"y\"", Some(base.clone())),
                ("LOOM-R25", top_bare("import type { signal } from \"loom\";"), "최상위 `import type`는", "import type { signal } from \"loom\";", Some(base.clone())),
                ("LOOM-R25", top_bare("import { signal as s } from \"loom\";"), "import 별칭(`… as …`)는", "signal as s", Some(top_bare("import { signal } from \"loom\";"))),
                ("LOOM-R25", top_bare("import { foo } from \"loom\";"), "\"loom\"의 `foo` import는", "foo", Some(top_bare("import { signal } from \"loom\";"))),
                ("LOOM-R25", top_bare("import * as L from \"loom\";"), "namespace import(`* as …`)는", "* as L", Some(base.clone())),
                ("LOOM-R25", top_bare("import L from \"loom\";"), "default import는", "L", Some(base.clone())),
                // R27 — 함수: async/generator/타입 매개변수, 선언: let/var
                (
                    "LOOM-R27",
                    format!("{IMPORT}export async function App() {{\n  const c = signal(0);\n  return <p>{{c()}}</p>;\n}}\n"),
                    "async 컴포넌트는 지원하지 않습니다",
                    "async",
                    Some(app("  const c = signal(0);\n  return <p>{c()}</p>;")),
                ),
                (
                    "LOOM-R27",
                    format!("{IMPORT}export function* App() {{\n  const c = signal(0);\n  return <p>{{c()}}</p>;\n}}\n"),
                    "generator(`function*`) 컴포넌트는 지원하지 않습니다",
                    "App",
                    Some(app("  const c = signal(0);\n  return <p>{c()}</p>;")),
                ),
                (
                    "LOOM-R27",
                    format!("{IMPORT}export function App<T>() {{\n  const c = signal(0);\n  return <p>{{c()}}</p>;\n}}\n"),
                    "타입 매개변수는 지원하지 않습니다",
                    "<T>",
                    Some(app("  const c = signal(0);\n  return <p>{c()}</p>;")),
                ),
                ("LOOM-R27", app("  let c = signal(0);\n  return <p>{c()}</p>;"), "`let` 대신 `const`를 쓰십시오", "let", Some(app("  const c = signal(0);\n  return <p>{c()}</p>;"))),
                ("LOOM-R27", app("  var c = signal(0);\n  return <p>{c()}</p>;"), "`var` 대신 `const`를 쓰십시오", "var", Some(app("  const c = signal(0);\n  return <p>{c()}</p>;"))),
                // R27 — 화살표(onClick·onInput·.map): async, 타입 매개변수
                ("LOOM-R27", a(COUNTER, "async "), "async `onClick` 핸들러는 지원하지 않습니다", "async", Some(a(COUNTER, ""))),
                ("LOOM-R27", a(COUNTER, "<T,>"), "타입 매개변수는 지원하지 않습니다", "<T,>", Some(a(COUNTER, ""))),
                (
                    "LOOM-R27",
                    input(&format!("{VALUE} onInput={{async (e) => t.set(e.target.value)}}")),
                    "async `onInput` 핸들러는",
                    "async",
                    Some(input(&format!("{VALUE} {ONINPUT}"))),
                ),
                ("LOOM-R27", a(MAP, "async "), "async `.map(...)` row 함수는", "async", Some(a(MAP, ""))),
                ("LOOM-R27", a(MAP, "<T,>"), "타입 매개변수는 지원하지 않습니다", "<T,>", Some(a(MAP, ""))),
            ]
        }

        #[test]
        fn each_structure_rejection_points_at_the_offending_syntax_and_has_a_compiling_positive_pair() {
            let cases = cases();
            // 개수 단언: R01 3, R02 2, R05 1, R06 4, R22 2, R24 1, R25 14, R27 10.
            assert_eq!(cases.len(), 37);
            for (code, neg, needle, token, pos) in &cases {
                assert_lower_diag(neg, code, needle, token);
                let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, neg, GraphMode::Flatten)
                    .err()
                    .unwrap_or_else(|| panic!("공개 API가 받아 버림:\n{neg}"));
                assert_diag(e, code, needle, Some(token));
                if let Some(pos) = pos {
                    let ok = super::super::compile_tsx_to_rust_graph(TEST_FILE, pos, GraphMode::Flatten)
                        .unwrap_or_else(|d| panic!("양성 짝이 실패:\n{pos}\n{d}"));
                    assert!(ok.warnings.is_empty(), "{pos}\n{}", ok.warnings);
                }
            }
        }

        #[test]
        fn duplicate_attr_diagnostics_label_the_first_occurrence() {
            let a = |tpl: &str, arg: &str| app(&tpl.replace('@', arg));
            for (src, code, first) in [
                (a(LIST, "onClick={() => xs.append(\"z\")} onClick={() => xs.rotate()}"), "LOOM-R05", "onClick={() => xs.append(\"z\")}"),
                (a(STATIC, "className=\"a\" class=\"b\""), "LOOM-R06", "className=\"a\""),
            ] {
                let d = lower_diag(&src);
                assert_eq!(d.items()[0].code, code, "{d}");
                let labels = &d.items()[0].labels;
                assert_eq!(labels.len(), 1, "{d}");
                assert_eq!(d.source_text(labels[0].0).as_deref(), Some(first), "{d}");
            }
        }

        #[test]
        fn non_literal_prop_keeps_its_pre_7a_rejection_before_r01() {
            // design-7a §1.1 R1a: 리터럴 검사가 먼저다 — 비리터럴 prop은 R01이 아니라
            // 옛 L00 문구로 거부된다(RQ6 M10 증거 테스트의 관측 메커니즘 유지).
            // 리터럴 prop이 앞에 있어도 그 prop에서 R01이 먼저 나는 것은 소스 순서다.
            assert_lower_diag(
                &widget("", "<Widget client:load initial={xs()} />"),
                lower::L00,
                "component prop value must be a number, boolean, string, or string-array literal",
                "xs()",
            );
            assert_lower_diag(&widget("", "<Widget client:load a={1} b={xs()} />"), "LOOM-R01", "prop `a`", "a={1}");
        }

        #[test]
        fn arrow_return_type_annotation_is_accepted() {
            // R27은 반환 타입 주석을 거부하지 않는다(의미가 없으므로).
            let src = app("  const c = signal(0);\n  return <button onClick={(): void => c.increment()}>{c()}</button>;");
            let ok = super::super::compile_tsx_to_rust_graph(TEST_FILE, &src, GraphMode::Flatten)
                .unwrap_or_else(|d| panic!("{src}\n{d}"));
            assert!(ok.warnings.is_empty());
        }

        // ---- W25(경고) ----

        const NO_IMPORT: &str = "export function App() {\n  const a = signal(0);\n  const b = signal(0);\n  const c = signal(0);\n  return (\n    <div>\n      <button onClick={() => a.increment()}>{a()}</button>\n      <button onClick={() => b.increment()}>{b()}</button>\n      <button onClick={() => c.increment()}>{c()}</button>\n      <p>{sharedValue(\"k\", 0)}</p>\n      <p>{sharedValue(\"k\", 0)}</p>\n      <p>{sharedValue(\"k\", 0)}</p>\n    </div>\n  );\n}\n";

        fn warnings(src: &str) -> Diagnostics {
            super::super::compile_tsx_to_rust_graph(TEST_FILE, src, GraphMode::Flatten)
                .unwrap_or_else(|d| panic!("W25는 경고일 뿐 거부가 아니다:\n{d}"))
                .warnings
        }

        #[test]
        fn w25_warns_once_per_missing_callee_at_its_first_use() {
            // 존재 단언 + 압축: callee마다 3번 쓰였지만 경고는 callee당 1건, 첫 사이트.
            let w = warnings(NO_IMPORT);
            assert_eq!(w.items().len(), 2, "{w}");
            assert!(!w.has_errors());
            for (item, callee, line) in [(&w.items()[0], "signal", 2), (&w.items()[1], "sharedValue", 10)] {
                assert_eq!(item.code, "LOOM-W25");
                assert_eq!(item.severity, Severity::Warning);
                assert!(item.message.contains(&format!("`{callee}`를 \"loom\"에서 import하지 않았습니다")), "{w}");
                assert_eq!(w.source_text(item.span).as_deref(), Some(callee), "{w}");
                assert_eq!(w.line_col(item.span).0, line, "{w}");
            }
            let r = w.render();
            assert_eq!(r.matches("warning[LOOM-W25]: ").count(), 2, "{r}");
            assert!(r.contains(&format!("--> {TEST_FILE}:2:13")), "{r}");
            assert!(r.contains(&format!("--> {TEST_FILE}:10:11")), "{r}");
            // 결정성(§3.4): 같은 입력 두 번 → 같은 렌더.
            assert_eq!(r, warnings(NO_IMPORT).render());
        }

        #[test]
        fn w25_is_silent_for_imported_callees_and_counts_only_the_missing_ones() {
            assert!(warnings(&format!("{IMPORT}{NO_IMPORT}")).is_empty());
            let partial = warnings(&format!("import {{ signal }} from \"loom\";\n{NO_IMPORT}"));
            assert_eq!(partial.items().len(), 1, "{partial}");
            assert!(partial.items()[0].message.contains("`sharedValue`"), "{partial}");
            // 쓰지 않은 callee는 import하지 않아도 경고가 없다.
            let unused = warnings("import { signal } from \"loom\";\nexport function App() {\n  const c = signal(0);\n  return <p>{c()}</p>;\n}\n");
            assert!(unused.is_empty(), "{unused}");
        }

        #[test]
        fn w25_reaches_every_public_api_and_does_not_change_the_output() {
            // 경고는 산출물과 무관하다: import 유무만 다른 두 소스의 Rust 산출물이 같다.
            let bare = "export function App() {\n  const c = signal(0);\n  return <button onClick={() => c.increment()}>{c()}</button>;\n}\n";
            let imported = format!("import {{ signal }} from \"loom\";\n{bare}");
            let r1 = super::super::compile_tsx_to_rust(TEST_FILE, bare).unwrap();
            let r2 = super::super::compile_tsx_to_rust(TEST_FILE, &imported).unwrap();
            assert_eq!(r1.rust, r2.rust);
            assert_eq!(r1.warnings.items().len(), 1);
            assert!(r2.warnings.is_empty());
            let c = super::super::compile_tsx_to_rust_with_candidate(TEST_FILE, bare, ListCandidate::K).unwrap();
            assert_eq!(c.warnings.items().len(), 1);
            let isl = "function W() {\n  const c = signal(0);\n  return <button onClick={() => c.increment()}>{c()}</button>;\n}\nexport function App() {\n  return <div><W client:load /></div>;\n}\n";
            let s = super::super::compile_tsx_to_single_island_rust(TEST_FILE, isl, "W").unwrap();
            assert_eq!(s.warnings.items().len(), 1);
            let g = super::super::compile_tsx_to_rust_graph(TEST_FILE, isl, GraphMode::Islands).unwrap();
            assert_eq!(g.warnings.items().len(), 1);
        }
    }

    /// 7a C7(2026-10-17, design-7a §1.1·§3.4·§4): 그래프 묶음 R07·R07U·R08·R44·R46·R47·G01과
    /// 결정성. 행마다 다음을 단언한다.
    /// - 진단이 정확히 1건이고 코드·문구·**강조 토큰**·렌더 `파일:줄:열`이 맞다.
    /// - (lower 거부) 공개 API(Flatten)도 같은 코드의 진단을 돌려준다.
    /// - 최소 차이 양성 짝이 경고 없이 끝까지 컴파일된다.
    /// 그리고 거부하는 결함이 **실제로 있었다**는 존재 단언을 따로 둔다(검사를
    /// 건너뛴 IR을 codegen에 직접 넣어 본다) — 부정 단언만으로는 공허할 수 있다.
    mod c7_graph_rejections {
        use super::*;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const IMPORT: &str = "import { signal, sharedSignal } from \"loom\";\n";
        fn app(body: &str) -> String {
            format!("{IMPORT}export function App() {{\n{body}\n}}\n")
        }
        /// 헬퍼들(`helpers`) + App 본문.
        fn graph(helpers: &str, body: &str) -> String {
            format!("{IMPORT}{helpers}export function App() {{\n{body}\n}}\n")
        }
        const W: &str = "function W() {\n  const w = signal(0);\n  return <button onClick={() => w.increment()}>{w()}</button>;\n}\n";

        const ONINPUT: &str = "onInput={(e) => t.set(e.target.value)}";
        const ROW_COND: &str = "{open() && <div><ul>{inner().map((x) => <li>{x}</li>)}</ul></div>}";

        /// (코드, 부정 소스, 문구 부분 문자열, 강조 토큰, 양성 짝 소스) — lower 거부.
        fn lower_cases() -> Vec<(&'static str, String, &'static str, &'static str, String)> {
            let nested = |outer: &str| {
                app(&format!(
                    "  const outer = signal([\"a\"]);\n  const open = signal(false);\n  const inner = signal([\"p\"]);\n  return <div><button onClick={{() => open.toggle()}}>t</button><ul>{{outer().map((it) => <li>{{it}}{ROW_COND}</li>)}}</ul>{outer}</div>;"
                ))
            };
            let r44 = |outer: &str, inner: &str| {
                app(&format!(
                    "  const a = signal(0);\n  const b = signal(0);\n  const t = signal(\"\");\n  return <div><p>{{a()}}</p><div {outer}>{inner}</div></div>;"
                ))
            };
            let r44_inner_button = "<button onClick={() => b.increment()}>{b()}</button><input value={t()} onInput={(e) => t.set(e.target.value)} />";
            let r44_w = |outer: &str| {
                graph(W, &format!("  const a = signal(0);\n  return <div><p>{{a()}}</p><div {outer}><W client:load /></div></div>;"))
            };
            let named = |name: &str| {
                app(&format!("  const {name} = signal(0);\n  return <button onClick={{() => {name}.increment()}}>{{{name}()}}</button>;"))
            };
            let comp = |name: &str| graph(&W.replace("function W", &format!("function {name}")), &format!("  return <div><{name} client:load /></div>;"));
            vec![
                // R07 — {n()} 두 번(주 span은 소스 순 첫 사이트), {o()&&…} 두 번, row 원본 + 바깥 binding
                (
                    "LOOM-R07",
                    app("  const n = signal(0);\n  return <div><p>{n()}</p><p>{n()}</p></div>;"),
                    "signal `n`가 binding 사이트 2곳에 쓰였습니다",
                    "n()",
                    app("  const n = signal(0);\n  return <div><p>{n()}</p><p>n</p></div>;"),
                ),
                (
                    "LOOM-R07",
                    app("  const o = signal(false);\n  return <div><button onClick={() => o.toggle()}>t</button><div>{o() && <i>a</i>}</div><div>{o() && <i>b</i>}</div></div>;"),
                    "signal `o`가 binding 사이트 2곳에",
                    "o()",
                    app("  const o = signal(false);\n  return <div><button onClick={() => o.toggle()}>t</button><div>{o() && <i>a</i>}</div><div><i>b</i></div></div>;"),
                ),
                ("LOOM-R07", nested("<aside>{open() && <i>o</i>}</aside>"), "signal `open`가 binding 사이트 2곳에", "open()", nested("")),
                // R07U — 표시 없는 signal
                (
                    "LOOM-R07U",
                    app("  const n = signal(0);\n  const m = signal(0);\n  return <button onClick={() => m.increment()}>{n()}</button>;"),
                    "signal `m`가 선언됐지만 표시되는 곳이 없습니다",
                    "m",
                    app("  const n = signal(0);\n  const m = signal(0);\n  return <button onClick={() => m.increment()}><b>{n()}</b><i>{m()}</i></button>;"),
                ),
                // R08 — row 루트 onClick(직접, 조건부 안 리스트)
                (
                    "LOOM-R08",
                    app("  const xs = signal([\"a\"]);\n  return <ul>{xs().map((x) => <li onClick={() => xs.rotate()}>{x}</li>)}</ul>;"),
                    "리스트 row 루트 `<li>`의 `onClick`은 지금 조용히 사라집니다",
                    "onClick={() => xs.rotate()}",
                    app("  const xs = signal([\"a\"]);\n  return <div><button onClick={() => xs.rotate()}>r</button><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>;"),
                ),
                (
                    "LOOM-R08",
                    app("  const o = signal(true);\n  const xs = signal([\"a\"]);\n  return <div><button onClick={() => o.toggle()}>t</button>{o() && <div><ul>{xs().map((x) => <li onClick={() => xs.rotate()}>{x}</li>)}</ul></div>}</div>;"),
                    "리스트 row 루트 `<li>`의",
                    "onClick={() => xs.rotate()}",
                    app("  const o = signal(true);\n  const xs = signal([\"a\"]);\n  return <div><button onClick={() => o.toggle()}>t</button>{o() && <div><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>}</div>;"),
                ),
                // R44 — onClick 안의 onClick, onClick 안의 <input>, onClick 안의 컴포넌트 참조
                (
                    "LOOM-R44",
                    r44("onClick={() => a.increment()}", r44_inner_button),
                    "이벤트를 받는 요소 안에 또 이벤트를 받는 요소가 있습니다(<button>의 `onClick`)",
                    "onClick={() => b.increment()}",
                    r44("", r44_inner_button),
                ),
                (
                    "LOOM-R44",
                    r44("onClick={() => a.increment()}", &format!("<b>{{b()}}</b><input value={{t()}} {ONINPUT} />")),
                    "(<input>의 `onInput`)",
                    ONINPUT,
                    r44("", &format!("<b>{{b()}}</b><input value={{t()}} {ONINPUT} />")),
                ),
                ("LOOM-R44", r44_w("onClick={() => a.increment()}"), "(컴포넌트 참조 `<W>`)", "W", r44_w("")),
                // R46 — Rust 키워드, `$`, ZWJ(U+200D), 비ASCII, 컴포넌트 이름 키워드
                ("LOOM-R46", named("type"), "signal 이름 `type`은 Rust 키워드이거나", "type", named("kind")),
                ("LOOM-R46", named("$c"), "(ASCII 영숫자·`_` 이외 문자 U+0024)", "$c", named("c")),
                ("LOOM-R46", named("c\u{200D}"), "U+200D", "c\u{200D}", named("c")),
                ("LOOM-R46", named("cafe\u{301}"), "U+0301", "cafe\u{301}", named("cafe")),
                (
                    "LOOM-R46",
                    app("  const type = sharedSignal(\"k\", 0);\n  return <button onClick={() => type.increment()}>{type()}</button>;"),
                    "signal 이름 `type`은",
                    "type",
                    app("  const kind = sharedSignal(\"k\", 0);\n  return <button onClick={() => kind.increment()}>{kind()}</button>;"),
                ),
                ("LOOM-R46", comp("Self"), "컴포넌트 이름 `Self`은 Rust 키워드", "Self", comp("Selfie")),
                // R47 — 도달할 수 없는 헬퍼(단독, 서로만 참조하는 둘)
                (
                    "LOOM-R47",
                    graph(W, "  const c = signal(0);\n  return <p>{c()}</p>;"),
                    "헬퍼 `W`가 루트에서 도달할 수 없습니다",
                    "W",
                    graph(W, "  const c = signal(0);\n  return <div><p>{c()}</p><W client:load /></div>;"),
                ),
                (
                    "LOOM-R47",
                    graph(
                        "function A() {\n  return <div><B client:load /></div>;\n}\nfunction B() {\n  const b = signal(0);\n  return <div><p>{b()}</p><A client:load /></div>;\n}\n",
                        "  const c = signal(0);\n  return <p>{c()}</p>;",
                    ),
                    "헬퍼 `A`가 루트에서 도달할 수 없습니다",
                    "A",
                    graph(
                        "function A() {\n  return <div><B client:load /></div>;\n}\nfunction B() {\n  const b = signal(0);\n  return <div><p>{b()}</p></div>;\n}\n",
                        "  const c = signal(0);\n  return <div><p>{c()}</p><A client:load /></div>;",
                    ),
                ),
            ]
        }

        #[test]
        fn each_graph_rejection_points_at_the_offending_syntax_and_has_a_compiling_positive_pair() {
            let cases = lower_cases();
            // 개수 단언: R07 3, R07U 1, R08 2, R44 3, R46 6, R47 2.
            assert_eq!(cases.len(), 17);
            for (code, neg, needle, token, pos) in &cases {
                assert_lower_diag(neg, code, needle, token);
                let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, neg, GraphMode::Flatten)
                    .err()
                    .unwrap_or_else(|| panic!("공개 API가 받아 버림:\n{neg}"));
                assert_diag(e, code, needle, Some(token));
                let ok = super::super::compile_tsx_to_rust_graph(TEST_FILE, pos, GraphMode::Flatten)
                    .unwrap_or_else(|d| panic!("양성 짝이 실패:\n{pos}\n{d}"));
                assert!(ok.warnings.is_empty(), "{pos}\n{}", ok.warnings);
            }
        }

        #[test]
        fn r07_labels_every_binding_site_and_the_declaration() {
            let d = lower_diag(&app(
                "  const n = signal(0);\n  return (\n    <div>\n      <p>{n()}</p>\n      <b>{n()}</b>\n      <i>{n()}</i>\n    </div>\n  );",
            ));
            let item = &d.items()[0];
            assert_eq!(item.code, "LOOM-R07");
            assert!(item.message.contains("3곳"), "{d}");
            // 사이트마다 label(소스 순) + 선언 label.
            let sites: Vec<(usize, usize)> = item.labels.iter().map(|(s, _)| d.line_col(*s)).collect();
            assert_eq!(sites, vec![(6, 11), (7, 11), (8, 11), (3, 9)], "{d}");
            for (s, _) in &item.labels[..3] {
                assert_eq!(d.source_text(*s).as_deref(), Some("n()"));
            }
            assert_eq!(item.labels[2].1, "binding 사이트 3/3");
            let r = d.render();
            assert!(r.contains(&format!("--> {TEST_FILE}:6:11")), "{r}");
            for line in ["6 |", "7 |", "8 |"] {
                assert!(r.contains(line), "{r}");
            }
        }

        #[test]
        fn r44_labels_the_outer_handler() {
            let src = app("  const a = signal(0);\n  const b = signal(0);\n  return <div onClick={() => a.increment()}><p>{a()}</p><button onClick={() => b.increment()}>{b()}</button></div>;");
            let d = assert_lower_diag(&src, "LOOM-R44", "또 이벤트를 받는 요소", "onClick={() => b.increment()}");
            let labels = &d.items()[0].labels;
            assert_eq!(labels.len(), 1);
            assert_eq!(d.source_text(labels[0].0).as_deref(), Some("onClick={() => a.increment()}"));
        }

        // ---- check.rs의 모드 거부: G01, R46(c) ----

        #[test]
        fn g01_single_component_path_rejects_component_references() {
            let neg = graph(W, "  const c = signal(0);\n  return <div><p>{c()}</p><W client:load /></div>;");
            for result in [
                super::super::compile_tsx_to_rust(TEST_FILE, &neg),
                super::super::compile_tsx_to_rust_with_candidate(TEST_FILE, &neg, ListCandidate::K),
            ] {
                let e = result.err().expect("G01");
                assert_diag(e, check::G01, "단일 컴포넌트 경로는 컴포넌트 참조를 컴파일하지 않습니다(`<W>`)", Some("W"));
            }
            // 같은 소스가 그래프 경로에서는 정상이다(모드 거부).
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Flatten).unwrap();
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Islands).unwrap();
            // 양성 짝: 참조 없는 단일 컴포넌트.
            super::super::compile_tsx_to_rust(TEST_FILE, &app("  const c = signal(0);\n  return <div><p>{c()}</p></div>;")).unwrap();
        }

        fn two_cards(second: &str) -> String {
            let card = |name: &str, s: &str| {
                format!("function {name}() {{\n  const {s} = signal(0);\n  return <button onClick={{() => {s}.increment()}}>{{{s}()}}</button>;\n}}\n")
            };
            graph(
                &format!("{}{}", card("ProductCard", "a"), card(second, "b")),
                &format!("  return <div><ProductCard client:load /><{second} client:visible /></div>;"),
            )
        }

        #[test]
        fn r46c_module_name_collision_is_rejected_where_modules_are_generated() {
            let neg = two_cards("Product_card");
            for result in [
                super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Islands),
                super::super::compile_tsx_to_single_island_rust(TEST_FILE, &neg, "ProductCard"),
            ] {
                let d = assert_diag(
                    result.err().expect("R46(c)"),
                    "LOOM-R46",
                    "`ProductCard`와 `Product_card`가 같은 모듈 이름 `product_card`으로 변환됩니다",
                    Some("Product_card"),
                );
                assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("ProductCard"));
            }
            // Flatten은 모듈을 만들지 않는다(모드 거부).
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Flatten).unwrap();
            // 양성 짝.
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &two_cards("ProductCard2"), GraphMode::Islands).unwrap();
        }

        #[test]
        fn r46c_mod_name_matches_what_codegen_actually_emits() {
            // check.rs의 `mod_name`은 codegen의 private `sanitize_mod_name`을 다시
            // 구현한 것이다 — 표류하면 이 대조가 깨진다(codegen이 방출한 이름과 비교).
            for name in ["Product_card", "ProductCardX", "ABC", "X1y", "Card", "Qq_R"] {
                let src = two_cards(name).replace("function ProductCard()", "function Zz()").replace("<ProductCard ", "<Zz ");
                let out = super::super::compile_tsx_to_rust_graph(TEST_FILE, &src, GraphMode::Islands)
                    .unwrap_or_else(|d| panic!("{name}: {d}"))
                    .rust;
                let m = check::mod_name(name);
                assert!(out.contains(&format!("pub mod {m}_generated {{")), "{name} → {m}");
                assert!(out.contains(&format!("pub mod {}_generated {{", check::mod_name("Zz"))));
            }
        }

        // ---- 존재 단언: 막는 결함이 검사 없이는 실제로 생긴다 ----

        fn lowered(src: &str) -> ir::ComponentGraph {
            let (_cm, module) = super::super::parse_tsx(TEST_FILE, src).unwrap();
            lower::lower_module(&module).unwrap_or_else(|d| panic!("{d:?}")).0
        }
        fn children(c: &mut ir::Component) -> &mut Vec<ir::JsxNode> {
            let ir::JsxNode::Element { children, .. } = &mut c.root else { panic!("root is not an element") };
            children
        }
        fn panic_msg(f: impl FnOnce() -> String) -> Option<String> {
            catch_unwind(AssertUnwindSafe(f)).err().map(|e| {
                e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default()
            })
        }

        #[test]
        fn the_rejected_defects_exist_without_the_checks() {
            // R07: 사이트 둘 → `let n_entity`가 두 번 방출되어 앞 것이 가려진다.
            let mut g = lowered(&app("  const n = signal(0);\n  return <div><p>{n()}</p><p>n</p></div>;"));
            let ir::JsxNode::Element { children: p, .. } = &mut children(&mut g.components[0])[1] else { panic!() };
            p[0] = ir::JsxNode::TextBinding { signal: "n".into() };
            let out = codegen::generate(&g.components[0]);
            assert_eq!(out.matches("let n_entity = app.world.spawn(").count(), 2, "{out}");

            // R07U: 표시 없는 signal → codegen.rs:1271 내부 오류 패닉.
            let mut g = lowered(&app("  const n = signal(0);\n  return <div><p>{n()}</p></div>;"));
            let ir::JsxNode::Element { children: p, .. } = &mut children(&mut g.components[0])[0] else { panic!() };
            p[0] = ir::JsxNode::StaticText("x".into());
            let msg = panic_msg(|| codegen::generate(&g.components[0])).expect("never spawned 패닉");
            assert!(msg.contains("was never spawned"), "{msg}");

            // R08: row 루트에 액션을 붙여도 산출물이 **한 바이트도** 다르지 않다(조용히 사라짐).
            let src = app("  const xs = signal([\"a\"]);\n  return <div><button onClick={() => xs.rotate()}>r</button><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>;");
            let base = codegen::generate(&lowered(&src).components[0]);
            let mut g = lowered(&src);
            let ir::JsxNode::Element { children: ul, .. } = &mut children(&mut g.components[0])[1] else { panic!() };
            let ir::JsxNode::ListBinding { row, .. } = &mut ul[0] else { panic!() };
            let ir::JsxNode::Element { action, .. } = row.as_mut() else { panic!() };
            *action = Some(ir::ClickAction::Rotate { signal: "xs".into() });
            assert_eq!(codegen::generate(&g.components[0]), base);

            // R44: 바깥 요소에 액션을 붙인 중첩도 codegen은 진단 없이 받는다.
            let mut g = lowered(&app("  const a = signal(0);\n  const b = signal(0);\n  return <div><p>{a()}</p><button onClick={() => b.increment()}>{b()}</button></div>;"));
            let ir::JsxNode::Element { action, .. } = &mut g.components[0].root else { panic!() };
            *action = Some(ir::ClickAction::Increment { signal: "a".into() });
            assert!(panic_msg(|| codegen::generate(&g.components[0])).is_none());

            // R46: 키워드 이름이 그대로 Rust 필드 이름으로 방출된다(rustc에서 실패).
            let mut g = lowered(&app("  const kind = signal(0);\n  return <p>{kind()}</p>;"));
            g.components[0].signals[0].name = "type".into();
            let ir::JsxNode::Element { children: p, .. } = &mut g.components[0].root else { panic!() };
            p[0] = ir::JsxNode::TextBinding { signal: "type".into() };
            assert!(codegen::generate(&g.components[0]).contains("    pub type: ::bevy_ecs::entity::Entity,"));

            // R47: Flatten은 도달할 수 없는 헬퍼를 진단 없이 버린다.
            let mut g = lowered(&graph(W, "  const c = signal(0);\n  return <div><p>{c()}</p><W client:load /></div>;"));
            children(g.components.iter_mut().find(|c| c.name == "App").unwrap()).pop();
            let out = codegen::generate_graph(&g, GraphMode::Flatten);
            assert!(!out.contains("w_entity"), "{out}");

            // G01: 단일 컴포넌트 경로는 생성되지 않는 모듈을 부르는 코드를 낸다.
            let g = lowered(&graph(W, "  const c = signal(0);\n  return <div><p>{c()}</p><W client:load /></div>;"));
            let out = codegen::generate(g.root_component());
            assert!(out.contains("w_generated::setup("), "{out}");
            assert!(!out.contains("pub mod w_generated"), "{out}");
        }

        // ---- 결정성(§3.4) ----

        /// 이 묶음의 모든 부정 입력(lower + 모드 거부)의 렌더와, 양성 짝의 산출물.
        fn transcript() -> String {
            let mut t = String::new();
            for (code, neg, _, _, pos) in lower_cases() {
                let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Flatten).err().unwrap();
                t.push_str(&format!("== {code}\n{}", e.render()));
                t.push_str(&super::super::compile_tsx_to_rust_graph(TEST_FILE, &pos, GraphMode::Flatten).unwrap().rust);
            }
            let cards = two_cards("Product_card");
            let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, &cards, GraphMode::Islands).err().unwrap();
            t.push_str(&e.render());
            let g01 = graph(W, "  const c = signal(0);\n  return <div><p>{c()}</p><W client:load /></div>;");
            t.push_str(&super::super::compile_tsx_to_rust(TEST_FILE, &g01).err().unwrap().render());
            t
        }

        #[test]
        fn diagnostics_and_outputs_are_deterministic() {
            let a = transcript();
            let b = transcript();
            assert_eq!(a, b);
            // 별도 프로세스 대조(design-7a §3.4): `LOOM_DET_OUT`이 있으면 기록한다 —
            // `cargo test`를 두 번 돌려 두 파일을 비교한다(C7 커밋 메시지에 결과 기록).
            if let Ok(path) = std::env::var("LOOM_DET_OUT") {
                std::fs::write(path, &a).unwrap();
            }
        }
    }

    // 7a C8(2026-10-17): check.rs의 모드 거부 R15·R16과 조건부 직계 리스트 R12.
    mod c8_mode_rejections {
        use super::*;
        use std::panic::{catch_unwind, AssertUnwindSafe};

        const IMPORT: &str = "import { signal } from \"loom\";\n";
        const INPUT: &str = "<input value={t()} onInput={(e) => t.set(e.target.value)} />";

        fn app(body: &str) -> String {
            format!("{IMPORT}export function App() {{\n{body}\n}}\n")
        }

        // ---- R15 ----

        /// Island `F`. `inner`가 `@IN@` 자리에 들어간다 — 깊이별 배치(직계 Element,
        /// 조건부 안, 리스트 row 안). `with_t`가 거짓이면 `t` 선언을 뺀다(양성 짝).
        fn island_f(shape: &str, inner: &str, with_t: bool) -> String {
            let t = if with_t { "  const t = signal(\"\");\n" } else { "" };
            let xs = if shape == "row" { "  const xs = signal([\"a\"]);\n" } else { "" };
            let body = match shape {
                "direct" => format!("<div><button onClick={{() => o.toggle()}}>t</button><p>{{o() && <i>x</i>}}</p>{inner}</div>"),
                "cond" => format!("<div><button onClick={{() => o.toggle()}}>t</button>{{o() && <div><b>x</b>{inner}</div>}}</div>"),
                "row" => format!("<div><button onClick={{() => o.toggle()}}>t</button><p>{{o() && <i>x</i>}}</p><ul>{{xs().map((x) => <li>{{x}}{inner}</li>)}}</ul></div>"),
                _ => unreachable!(),
            };
            format!(
                "{IMPORT}function F() {{\n  const o = signal(true);\n{xs}{t}  return {body};\n}}\nexport function App() {{\n  return <div><F client:load /></div>;\n}}\n"
            )
        }

        #[test]
        fn r15_single_island_rejects_input_at_every_depth() {
            let shapes = ["direct", "cond", "row"];
            assert_eq!(shapes.len(), 3);
            for shape in shapes {
                let neg = island_f(shape, INPUT, true);
                let e = super::super::compile_tsx_to_single_island_rust(TEST_FILE, &neg, "F")
                    .err()
                    .unwrap_or_else(|| panic!("{shape}: 단일 Island가 input을 받아 버림"));
                let d = assert_diag(
                    e,
                    check::R15,
                    "Island `F`를 단일 Island 클라이언트로 컴파일할 때 <input> 양방향 binding은 지원하지 않습니다",
                    Some("input"),
                );
                assert_eq!(d.source_text(d.items()[0].labels[0].0).as_deref(), Some("F"), "{shape}");
                // 다중 Islands의 기존 거부(L00, 문구 유지)도 같은 깊이까지 본다.
                let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Islands).err().expect("Islands");
                assert_diag(e, lower::L00, "contains an <input> two-way binding", Some("input"));
                // 최소 차이 양성 짝: input(과 그것만 쓰던 `t`)을 뺀다.
                let pos = island_f(shape, "", false);
                let ok = super::super::compile_tsx_to_single_island_rust(TEST_FILE, &pos, "F")
                    .unwrap_or_else(|d| panic!("{shape}: 양성 짝이 실패:\n{pos}\n{d}"));
                assert!(ok.warnings.is_empty(), "{}", ok.warnings);
            }
            // 모드 거부: 직계 input은 Flatten에서는 정상이다(Flatten 진입점은 input을 연결한다).
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &island_f("direct", INPUT, true), GraphMode::Flatten).unwrap();
        }

        // ---- R16 ----

        const W: &str = "function W() {\n  const w = signal(0);\n  return <button onClick={() => w.increment()}>{w()}</button>;\n}\n";
        fn refs(second: &str, extra: &str) -> String {
            format!("{IMPORT}{W}{extra}export function App() {{\n  return <div><W client:load />\n    <{second} client:visible /></div>;\n}}\n")
        }

        #[test]
        fn r16_duplicate_reference_is_rejected_where_islands_are_generated() {
            let neg = refs("W", "");
            for result in [
                super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Islands),
                super::super::compile_tsx_to_single_island_rust(TEST_FILE, &neg, "W"),
            ] {
                let d = assert_diag(
                    result.err().expect("R16"),
                    check::R16,
                    "컴포넌트 `W`가 두 번 참조됐습니다. Islands에서는 둘째 참조가 첫 참조의 디렉티브·island id를 공유합니다",
                    Some("W"),
                );
                // 주 span은 **둘째** 참조(8행 6열), label은 두 사이트 모두.
                assert_eq!(d.line_col(d.items()[0].span), (8, 6), "{d}");
                let labels: Vec<((usize, usize), &str)> =
                    d.items()[0].labels.iter().map(|(s, l)| (d.line_col(*s), l.as_str())).collect();
                assert_eq!(labels, vec![((7, 16), "첫 참조(client:load)"), ((8, 6), "둘째 참조(client:visible)")], "{d}");
                assert!(d.render().contains(&format!("--> {TEST_FILE}:8:6")), "{d}");
            }
            // 양성 짝: 둘째 참조가 다른 컴포넌트.
            let pos = refs("V", &W.replace("function W", "function V").replace("const w", "const v").replace("w.increment", "v.increment").replace("{w()}", "{v()}"));
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &pos, GraphMode::Islands).unwrap();
            super::super::compile_tsx_to_single_island_rust(TEST_FILE, &pos, "W").unwrap();
        }

        #[test]
        fn r16_flatten_is_unchanged() {
            // design-7a R16: Flatten에는 검사를 더하지 않는다. signal 없는 컴포넌트의
            // 중복 참조는 무해해 컴파일되고(아래), signal이 있으면 기존 552가 거부한다
            // (check_backstop::flatten_double_reference_to_a_signal_bearing_component_552).
            let s = "function S() {\n  return <p>static</p>;\n}\n";
            let src = format!("{IMPORT}{s}export function App() {{\n  const c = signal(0);\n  return <div><p>{{c()}}</p><S client:load /><S client:visible /></div>;\n}}\n");
            super::super::compile_tsx_to_rust_graph(TEST_FILE, &src, GraphMode::Flatten).unwrap();
            let e = super::super::compile_tsx_to_rust_graph(TEST_FILE, &refs("W", ""), GraphMode::Flatten).err().expect("552");
            assert_diag(e, lower::L00, "second declaration of `w`", Some("W"));
        }

        // ---- R12(리스트) ----

        fn cond_list(then: &str) -> String {
            app(&format!(
                "  const o = signal(true);\n  const xs = signal([\"a\"]);\n  return <div><button onClick={{() => o.toggle()}}>t</button>{{o() && {then}}}</div>;"
            ))
        }
        const DIRECT: &str = "<ul>{xs().map((x) => <li>{x}</li>)}</ul>";
        const WRAPPED: &str = "<div><ul>{xs().map((x) => <li>{x}</li>)}</ul></div>";

        #[test]
        fn r12_direct_list_child_of_a_conditional_is_rejected_in_every_mode() {
            let neg = cond_list(DIRECT);
            let results = [
                super::super::compile_tsx_to_rust(TEST_FILE, &neg),
                super::super::compile_tsx_to_rust_with_candidate(TEST_FILE, &neg, ListCandidate::K),
                super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Flatten),
                super::super::compile_tsx_to_rust_graph(TEST_FILE, &neg, GraphMode::Islands),
            ];
            assert_eq!(results.len(), 4);
            for result in results {
                let d = assert_diag(
                    result.err().expect("R12"),
                    check::R12,
                    "조건부 `{o() && …}`의 자식이 리스트 하나뿐입니다. 지금은 resume 대상이 어긋납니다. `<div>`로 한 번 감싸십시오",
                    Some("ul"),
                );
                let labels: Vec<Option<String>> = d.items()[0].labels.iter().map(|(s, _)| d.source_text(*s)).collect();
                assert_eq!(labels, vec![Some("xs()".to_string()), Some("o()".to_string())], "{d}");
            }
            // 양성 짝: `<div>`로 한 번 감싼다.
            for result in [
                super::super::compile_tsx_to_rust(TEST_FILE, &cond_list(WRAPPED)),
                super::super::compile_tsx_to_rust_graph(TEST_FILE, &cond_list(WRAPPED), GraphMode::Flatten),
            ] {
                let ok = result.unwrap_or_else(|d| panic!("{d}"));
                assert!(ok.warnings.is_empty(), "{}", ok.warnings);
            }
            // 직계 **텍스트**는 R12 거부가 아니다(C9가 구현으로 고치는 형태).
            let text = app("  const o = signal(true);\n  const n = signal(0);\n  return <div><button onClick={() => o.toggle()}>t</button>{o() && <p>{n()}</p>}</div>;");
            super::super::compile_tsx_to_rust(TEST_FILE, &text).unwrap();
        }

        // ---- 존재 단언: 막는 결함이 검사 없이는 실제로 생긴다 ----

        fn lowered(src: &str) -> ir::ComponentGraph {
            let (_cm, module) = super::super::parse_tsx(TEST_FILE, src).unwrap();
            lower::lower_module(&module).unwrap_or_else(|d| panic!("{d:?}")).0
        }
        fn no_panic(f: impl FnOnce() -> String) -> String {
            catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| panic!("codegen이 패닉했다 — 존재 단언 전제가 틀렸다"))
        }

        #[test]
        fn the_rejected_defects_exist_without_the_checks() {
            // R15: 단일 Island codegen은 input을 진단 없이 받고, click 리스너만 연결한다.
            let g = lowered(&island_f("direct", INPUT, true));
            let out = no_panic(|| codegen::generate_single_island_client(&g, "F"));
            // input 액션이 실제로 생성된다(공허한 단언 방지) — 그런데 진입점은 그 액션을
            // 값 `""`로 부르고(`resume_action_by_symbol(&symbol, "")`), click에만 연결한다.
            assert!(out.contains("let node_4 = dom.create_element(node_0, \"input\", None);"), "{out}");
            assert!(out.contains("\"Action1\" => Action::Action1(string_arg.to_string()),"), "{out}");
            assert!(out.contains("let action = resume_action_by_symbol(&symbol, \"\");"), "{out}");
            assert!(out.contains("add_event_listener_with_callback(\"click\""), "{out}");
            assert!(!out.contains("add_event_listener_with_callback(\"input\""), "{out}");

            // R16: Islands codegen은 중복 참조를 받고, 둘째 참조의 `client:visible`이
            // 산출물 어디에도 없다(첫 참조의 `load`가 쓰인다).
            let g = lowered(&refs("W", ""));
            let out = no_panic(|| codegen::generate_graph(&g, GraphMode::Islands));
            assert_eq!(out.matches("pub mod w_generated {").count(), 1, "{out}");
            assert!(out.contains("\"data-loom-activation\", \"load\""), "{out}");
            assert!(!out.contains("\"visible\""), "{out}");

            // R12: codegen은 직계 리스트를 받아 wrapper(anchor)를 곧 리스트 부모로 쓴다.
            let g = lowered(&cond_list(DIRECT));
            let out = no_panic(|| codegen::generate(g.root_component()));
            // 리스트 entity의 DomTarget이 조건부 wrapper(anchor) 자체다 — Bool 패스가 adopt하는
            // 노드와 리스트 패스가 adopt하는 노드가 같아지는 원인.
            const LIST_ON_ANCHOR: &str = "commands.entity(slot_0).insert(::loom_runtime::DomTarget(anchor.0));";
            assert_eq!(out.matches(LIST_ON_ANCHOR).count(), 1, "{out}");
            assert!(out.contains("::loom_runtime::CondBlock::with_size(1)"), "{out}");
            // 감싼 양성 짝은 그 경로를 타지 않는다.
            let g = lowered(&cond_list(WRAPPED));
            assert!(!codegen::generate(g.root_component()).contains(LIST_ON_ANCHOR));
        }

        #[test]
        fn diagnostics_are_deterministic() {
            let render = || {
                let mut t = String::new();
                for shape in ["direct", "cond", "row"] {
                    t.push_str(&super::super::compile_tsx_to_single_island_rust(TEST_FILE, &island_f(shape, INPUT, true), "F").err().unwrap().render());
                }
                t.push_str(&super::super::compile_tsx_to_rust_graph(TEST_FILE, &refs("W", ""), GraphMode::Islands).err().unwrap().render());
                t.push_str(&super::super::compile_tsx_to_rust(TEST_FILE, &cond_list(DIRECT)).err().unwrap().render());
                t
            };
            assert_eq!(render(), render());
        }
    }
}
