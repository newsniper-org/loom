//! G0R1 C5 — D·D-LIS 무조건 변형의 방출 변화를 **다중집합 동일 + 결정적**으로 고정한다(개정 #12).
//!
//! 예전 D 무조건은 retained 행마다 `SetText`를 **직접** 발행했다(P2-(iii)로 기각). 새 정의는 값 채널 write →
//! `Changed<RowValue<T>>` → patch System의 `SetText`다. 설계가 허용한 차이는 **방출 위치·순서뿐**이다:
//! 틱마다 `SetText` 다중집합과 나머지 write(SpawnRow/Reposition/RemoveNode)의 열은 같아야 한다.
//! 옛 기준선은 `golden/list-candidate-writes-axes.pre-g0r1.txt`(G0 기각 변형의 기록)로 보존한다.

mod recorder_axes;

use std::collections::BTreeMap;

/// 절(`======== <후보> <축> ========`)별로, 틱마다 (SetText 다중집합, 그 밖의 write 열)로 접는다.
fn fold(log: &str) -> BTreeMap<String, Vec<(Vec<String>, Vec<String>)>> {
    let mut out: BTreeMap<String, Vec<(Vec<String>, Vec<String>)>> = BTreeMap::new();
    let mut cur = String::new();
    for line in log.lines() {
        if let Some(h) = line.strip_prefix("======== ") {
            cur = h.trim_end_matches(" ========").to_string();
            out.entry(cur.clone()).or_default();
        } else if line.starts_with("--- tick") {
            out.get_mut(&cur).unwrap().push((Vec::new(), Vec::new()));
        } else if !line.is_empty() {
            let ticks = out.get_mut(&cur).unwrap();
            let t = ticks.last_mut().unwrap();
            if line.starts_with("SetText") {
                t.0.push(line.to_string());
            } else {
                t.1.push(line.to_string());
            }
        }
    }
    for ticks in out.values_mut() {
        for t in ticks.iter_mut() {
            t.0.sort();
        }
    }
    out
}

#[test]
fn d_unconditional_emission_changes_only_position_and_order() {
    let old = fold(include_str!("golden/list-candidate-writes-axes.pre-g0r1.txt"));
    let new_log = recorder_axes::full_log();
    if let Ok(path) = std::env::var("LOOM_DUMP_AXES_GOLDEN") {
        std::fs::write(path, &new_log).unwrap();
    }
    let new = fold(&new_log);
    assert_eq!(old.keys().collect::<Vec<_>>(), new.keys().collect::<Vec<_>>());

    // 존재 단언을 먼저: 무조건 절에 SetText가 실제로 나오고(공허한 동일 방지), 옛 열과 새 열이 모두 비어 있지 않다.
    for sec in ["D UNCOND", "D-LIS UNCOND"] {
        let n: usize = new[sec].iter().map(|t| t.0.len()).sum();
        assert!(n > 0, "{sec}: 새 방출에 SetText가 하나도 없다");
        let m: usize = old[sec].iter().map(|t| t.0.len()).sum();
        assert_eq!(n, m, "{sec}: SetText 총수가 달라졌다");
    }
    // 그 다음: 무조건 D 두 절은 틱별 다중집합 + 나머지 열이 같고, 다른 모든 절은 글자 그대로 같다.
    for (sec, ticks) in &old {
        assert_eq!(&new[sec], ticks, "절 {sec}의 방출이 달라졌다");
    }
}

#[test]
fn unconditional_emission_is_deterministic() {
    assert_eq!(recorder_axes::full_log(), recorder_axes::full_log());
}
