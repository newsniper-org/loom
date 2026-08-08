//! 트랙 N 실행기.
//!
//! `cargo run -p loom-track-n --release -- --cases 10000 --cycle 1 --seed-base 20260808`
//!
//! **축소 실행은 사이클이 아니다.** §5는 사이클당 10^4 시퀀스를 사전 등록했고
//! 그 수는 결과 확인 후 변경 불가다. `--cases`가 10000이 아닌 실행은 하네스
//! 검증이므로 보고할 때 반드시 그렇게 적을 것 — 이 프로그램도 그렇게 출력한다.

use loom_track_n::{dom::Family, ops, shrink, Arm, INITIAL_INNER, INITIAL_OUTER};

const REGISTERED_CASES: usize = 10_000;
const SEQ_LEN: usize = 200;

fn arg(name: &str, default: usize) -> usize {
    let args: Vec<String> = std::env::args().collect();
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn out_path() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    args.iter().position(|a| a == "--out").and_then(|i| args.get(i + 1)).cloned()
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

fn main() {
    let cases = arg("--cases", REGISTERED_CASES);
    let cycle = arg("--cycle", 1);
    let seed_base = arg("--seed-base", 20_260_808) as u64;
    let determinism_every = arg("--determinism-every", 50);

    let registered = cases == REGISTERED_CASES;
    println!("트랙 N — 중첩 정합성 property test (RQ1 §5)");
    println!("  사이클 {cycle} / 케이스 {cases} / 시퀀스 길이 ≤{SEQ_LEN} / 시드 기저 {seed_base}");
    if !registered {
        println!("  ⚠️  **하네스 검증 실행이지 사이클이 아니다** — 사전 등록값은 {REGISTERED_CASES}이고");
        println!("      그 수는 §5상 결과 확인 후 변경 불가다. 이 출력은 판정 데이터가 아니다.");
    }
    println!();

    // §5 "시드 기록" — 반례가 나오든 안 나오든 **무엇을 돌렸는지**가 남아야
    // 재현·재심사가 가능하다. 통과했다는 문장만 남기면 그 실행은 검증 불가다.
    let mut ledger: Vec<String> = Vec::new();
    let mut failures = 0usize;
    for arm in Arm::ALL {
        let mut first_fail: Option<(u64, Vec<ops::Op>, Family, String, Vec<Family>)> = None;
        let mut det_checked = 0usize;
        for case in 0..cases {
            // 시드 = 기저 ⊕ (사이클, arm, 케이스) — 사이클마다 **새 독립 시드**를
            // 쓰라는 §5 수리 사이클 규정 그대로.
            let seed = seed_base
                .wrapping_mul(1_000_003)
                .wrapping_add(cycle as u64 * 7_919)
                .wrapping_add(arm as u64 * 104_729)
                .wrapping_add(case as u64);
            let ops = ops::sequence(seed, SEQ_LEN, INITIAL_OUTER.len(), INITIAL_INNER.len());
            let outcome = loom_track_n::run(arm, &ops);
            if !outcome.ok() {
                let family = outcome.primary_family().unwrap();
                let detail = outcome
                    .violations
                    .iter()
                    .find(|v| v.family == family)
                    .map(|v| v.detail.clone())
                    .unwrap_or_default();
                first_fail = Some((seed, ops, family, detail, outcome.families()));
                break;
            }
            // 불변식 5의 결정성 쪽 — 같은 시드로 다시 돌려 op 열이 동일한가.
            if determinism_every > 0 && case % determinism_every == 0 {
                det_checked += 1;
                let again = loom_track_n::run(arm, &ops);
                if again.op_log_hash != outcome.op_log_hash {
                    first_fail = Some((
                        seed,
                        ops,
                        Family::FlushOrdering,
                        "같은 시드로 두 번 돌렸는데 DOM 연산 열이 다르다".to_string(),
                        vec![Family::FlushOrdering],
                    ));
                    break;
                }
            }
        }

        match first_fail {
            None => {
                println!("  {:6} 통과 — {cases}케이스, 결정성 재현 {det_checked}건", arm.name());
                ledger.push(format!(
                    "    {{\"arm\": \"{}\", \"result\": \"pass\", \"cases\": {cases}, \"determinism_checks\": {det_checked}}}",
                    arm.name()
                ));
            }
            Some((seed, ops, family, detail, all)) => {
                failures += 1;
                let minimal = shrink(arm, &ops, family);
                println!(
                    "  {:6} **반례** — 대표 계열 {family:?} / 터진 계열 전부 {all:?}",
                    arm.name()
                );
                println!("      시드: {seed}");
                println!("      최소형 {}개 op (원본 {}개):", minimal.len(), ops.len());
                for op in &minimal {
                    println!("        {op:?}");
                }
                println!("      {detail}");
                ledger.push(format!(
                    "    {{\"arm\": \"{}\", \"result\": \"counterexample\", \"seed\": {seed}, \
                     \"primary_family\": \"{family:?}\", \"families\": {:?}, \
                     \"original_ops\": {}, \"minimal_ops\": {}, \
                     \"minimal\": \"{}\", \"detail\": \"{}\"}}",
                    arm.name(),
                    all.iter().map(|f| format!("{f:?}")).collect::<Vec<_>>(),
                    ops.len(),
                    minimal.len(),
                    json_escape(&format!("{minimal:?}")),
                    json_escape(&detail),
                ));
            }
        }
    }

    if let Some(path) = out_path() {
        let body = format!(
            "{{\n  \"track\": \"N\",\n  \"registered_cycle\": {},\n  \"cycle\": {cycle},\n  \"cases\": {cases},\n  \"seq_len\": {SEQ_LEN},\n  \"seed_base\": {seed_base},\n  \"fixture\": \"demo/nested-list-derived.tsx\",\n  \"arms\": [\n{}\n  ]\n}}\n",
            registered,
            ledger.join(",\n")
        );
        std::fs::write(&path, body).unwrap_or_else(|e| panic!("시드 원장 쓰기 실패 {path}: {e}"));
        println!("\n시드 원장: {path}");
    }

    println!();
    if failures == 0 {
        println!("전 arm 통과.");
    } else {
        println!("{failures}개 arm에서 반례 — §5 수리 사이클 규정 적용(수리 → 새 독립 시드로 전체 재실행 = 1사이클, 최대 3).");
        println!("귀속 선행: 반례 최소형이 특정 arm 없이 공유 인프라만으로 재현되는지 먼저 확인할 것.");
        std::process::exit(1);
    }
}
