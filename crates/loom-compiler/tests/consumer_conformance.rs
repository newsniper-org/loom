//! 소비자 적합성 테스트 (Phase 7a C1, 2026-10-17 신설; 설계 design-7a §4).
//!
//! 13개 소비자 `build.rs`가 하는 컴파일 호출 **22회**를 같은 (소스, 경로,
//! 모드/후보/대상) 인자로 그대로 열거해, 전부 `Ok`이고 호출별 경고가 기대 목록과
//! 정확히 같은지 단언한다. Phase 7a의 새 거부가 기존 소비 픽스처 하나라도
//! 막으면(병익님 규칙: 수리보다 연구 산출물 재검토가 먼저) 여기서 먼저 깨진다.
//!
//! - 합성 소스는 `build_support::largeapp_synth_source`를 그대로 불러 복제하지
//!   않는다(복제하면 이 테스트가 build.rs와 다른 입력을 볼 수 있다).
//! - 행이 하나라도 빠지거나 늘면 개수 단언(22)이 실패한다. 새 소비자 build.rs를
//!   추가하면 이 표와 `tools/compiler-gate/manifest`를 함께 갱신해야 한다.
//! - 산출물 **바이트**의 동일성은 이 테스트가 아니라 게이트(`tools/compiler-gate`)
//!   의 A 계층이 본다. 이 테스트는 "거부되지 않는다"와 "경고 집합"만 본다.

use loom_compiler::build_support::{largeapp_synth_source, LARGEAPP_SYNTH_FILE_NAME};
use loom_compiler::{Compiled, Diagnostics, GraphMode, ListCandidate};
use std::path::PathBuf;

enum Src {
    Demo(&'static str),
    LargeappSynth,
}

#[derive(Clone, Copy)]
enum Arm {
    Islands,
    Flatten,
}

enum Call {
    Legacy,
    Candidate(ListCandidate),
    /// `GraphMode`는 Copy가 아니라 const 표에 직접 담지 않고 팔 이름으로 둔다.
    Graph(Arm),
    SingleIsland(&'static str),
}

struct Row {
    /// 이 호출을 하는 소비자 build.rs(사람용 표지).
    consumer: &'static str,
    src: Src,
    call: Call,
    /// 이 호출에서 기대하는 경고 코드(정렬된 렌더 순서). C1 시점에는 경고를
    /// 내는 검사가 없으므로 전부 비어 있다(W25는 C6에서 추가).
    expected_warnings: &'static [&'static str],
}

const ROWS: &[Row] = &[
    Row { consumer: "loom-demo", src: Src::Demo("counter-list.tsx"), call: Call::Legacy, expected_warnings: &[] },
    Row { consumer: "loom-fic", src: Src::Demo("counter-list.tsx"), call: Call::Legacy, expected_warnings: &[] },
    Row { consumer: "loom-island-compiled", src: Src::Demo("islands/product-card.tsx"), call: Call::Graph(Arm::Islands), expected_warnings: &[] },
    Row { consumer: "loom-island-compiled", src: Src::Demo("islands/product-card.tsx"), call: Call::Graph(Arm::Flatten), expected_warnings: &[] },
    Row { consumer: "loom-largeapp-axis-flat", src: Src::LargeappSynth, call: Call::Graph(Arm::Flatten), expected_warnings: &[] },
    Row { consumer: "loom-largeapp-axis-island-target", src: Src::LargeappSynth, call: Call::SingleIsland("TargetWidget"), expected_warnings: &[] },
    Row { consumer: "loom-nested-fixture", src: Src::Demo("nested-conditional.tsx"), call: Call::Legacy, expected_warnings: &[] },
    Row { consumer: "loom-nested-fixture", src: Src::Demo("nested-list.tsx"), call: Call::Legacy, expected_warnings: &[] },
    Row { consumer: "loom-nested-fixture", src: Src::Demo("nested-list-derived.tsx"), call: Call::Legacy, expected_warnings: &[] },
    Row { consumer: "loom-resume-demo", src: Src::Demo("counter-list.tsx"), call: Call::Graph(Arm::Flatten), expected_warnings: &[] },
    Row { consumer: "loom-track-n", src: Src::Demo("nested-list-derived.tsx"), call: Call::Candidate(ListCandidate::K), expected_warnings: &[] },
    Row { consumer: "loom-track-n", src: Src::Demo("nested-list-derived.tsx"), call: Call::Candidate(ListCandidate::C), expected_warnings: &[] },
    Row { consumer: "loom-track-n", src: Src::Demo("nested-list-derived.tsx"), call: Call::Candidate(ListCandidate::D), expected_warnings: &[] },
    Row { consumer: "loom-track-n", src: Src::Demo("nested-list-derived.tsx"), call: Call::Candidate(ListCandidate::DLis), expected_warnings: &[] },
    Row { consumer: "loom-track-n", src: Src::Demo("nested-list-derived.tsx"), call: Call::Candidate(ListCandidate::E), expected_warnings: &[] },
    Row { consumer: "loom-track-n", src: Src::Demo("nested-list-derived.tsx"), call: Call::Candidate(ListCandidate::ELis), expected_warnings: &[] },
    Row { consumer: "loom-widget-embed", src: Src::Demo("widget-embed.tsx"), call: Call::Graph(Arm::Islands), expected_warnings: &[] },
    Row { consumer: "loom-widget-embed", src: Src::Demo("widget-embed.tsx"), call: Call::Graph(Arm::Flatten), expected_warnings: &[] },
    Row { consumer: "loom-widget-embed-island-checkout-form", src: Src::Demo("widget-embed.tsx"), call: Call::SingleIsland("CheckoutForm"), expected_warnings: &[] },
    Row { consumer: "loom-widget-embed-island-mini-dashboard", src: Src::Demo("widget-embed.tsx"), call: Call::SingleIsland("MiniDashboard"), expected_warnings: &[] },
    Row { consumer: "loom-widget-embed-island-product-card", src: Src::Demo("widget-embed.tsx"), call: Call::SingleIsland("ProductCard"), expected_warnings: &[] },
    Row { consumer: "loom-widget-embed-island-quantity-picker", src: Src::Demo("widget-embed.tsx"), call: Call::SingleIsland("QuantityPicker"), expected_warnings: &[] },
];

fn run(row: &Row) -> Result<Compiled, Diagnostics> {
    let (file_name, source) = match row.src {
        Src::Demo(rel) => {
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo").join(rel);
            let s = std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{p:?}: {e}"));
            (p.display().to_string(), s)
        }
        Src::LargeappSynth => (LARGEAPP_SYNTH_FILE_NAME.to_string(), largeapp_synth_source(50)),
    };
    match row.call {
        Call::Legacy => loom_compiler::compile_tsx_to_rust(&file_name, &source),
        Call::Candidate(c) => loom_compiler::compile_tsx_to_rust_with_candidate(&file_name, &source, c),
        Call::Graph(arm) => {
            let mode = match arm {
                Arm::Islands => GraphMode::Islands,
                Arm::Flatten => GraphMode::Flatten,
            };
            loom_compiler::compile_tsx_to_rust_graph(&file_name, &source, mode)
        }
        Call::SingleIsland(t) => loom_compiler::compile_tsx_to_single_island_rust(&file_name, &source, t),
    }
}

#[test]
fn all_22_consumer_compile_calls_succeed_with_exactly_the_expected_warnings() {
    // 개수 단언: 13개 build.rs의 호출 수(design-7a 전제, manifest A 22행과 같음).
    assert_eq!(ROWS.len(), 22, "소비자 컴파일 호출 개수가 22가 아님");
    let consumers: std::collections::BTreeSet<&str> = ROWS.iter().map(|r| r.consumer).collect();
    assert_eq!(consumers.len(), 13, "소비자 build.rs 개수가 13이 아님: {consumers:?}");

    let mut failures = Vec::new();
    for (i, row) in ROWS.iter().enumerate() {
        match run(row) {
            Ok(c) => {
                // 존재 단언: 비어 있지 않은 Rust 소스가 나와야 "Ok"가 의미를 갖는다.
                assert!(c.rust.contains("pub fn setup("), "row {i} ({}): 산출물에 setup 없음", row.consumer);
                let got: Vec<&str> = c.warnings.items().iter().map(|d| d.code).collect();
                if got != row.expected_warnings {
                    failures.push(format!(
                        "row {i} ({}): 경고 {got:?} != 기대 {:?}\n{}",
                        row.consumer,
                        row.expected_warnings,
                        c.warnings.render()
                    ));
                }
            }
            Err(d) => failures.push(format!("row {i} ({}): 거부됨\n{}", row.consumer, d.render())),
        }
    }
    assert!(failures.is_empty(), "소비자 적합성 실패 {}건:\n{}", failures.len(), failures.join("\n"));
}
