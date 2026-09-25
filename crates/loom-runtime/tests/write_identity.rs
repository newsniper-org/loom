//! 트랙 L 후보들이 내는 **write 열**을 골든으로 고정한다.
//!
//! 골든이 셋이고 **각각 다른 것을 주장한다** — 한 파일에 섞으면 "리팩터로 안 바뀌었다"와
//! "여기서부터 다시 안 바뀐다"가 구별되지 않는다.
//!
//! | 골든 | 기준 시점 | 덮는 arm |
//! |---|---|---|
//! | `list-candidate-writes.txt` | `RowSpawner` 도입 **직전 커밋**(f0eaf6f)에서 같은 레코더로 뽑음 | D/D-LIS/E/E-LIS **R1** |
//! | `list-candidate-writes-r2.txt` | 개정 #9-11(R2를 조항 원문대로) 이후 | D/D-LIS/E/E-LIS **R2** |
//! | `list-candidate-writes-k.txt` | **§10 K 디스패치 준수**(2026-09-22) 이후 | K R1 + R2 |
//! | `list-candidate-writes-axes.txt` | 개정 #9 축 전부(2026-09-25) | **C** · R3 · W8 · 무조건 변형 × 6후보 |
//!
//! **왜 K만 따로 떼었나**: 2026-09-22에 `k_body`가 §10 "K: 디스패치 규칙"의 네 분기를
//! 전부 갖게 됐다. 그 전에는 (4)번(전량 despawn+respawn)밖에 없어서 append 한 건에도
//! 전 행을 재생성했고, §3.4가 K의 등록된 유리 구간으로 잡아 둔 W5·W9에서 대조군이
//! **자기 최선형이 아니라 최악형으로** 측정되고 있었다(원칙 1 위반). 즉 K의 방출 변화는
//! **의도된 조항 준수**다.
//!
//! 그리고 그 변화가 K에만 갇혔다는 것이 중요하다 — 이 파일의 첫 두 골든이 그것을
//! 증명한다. D/D-LIS/E/E-LIS는 R1·R2 모두 한 바이트도 안 바뀌었다.
//!
//! 후보 동작을 **의도적으로** 바꿀 때는 이 파일을 지우지 말고 해당 골든만 다시 뽑되,
//! 그때 §2.5 재측정 절차를 함께 밟을 것.

mod recorder_axes;
mod recorder_common;

/// 로그를 `======== <arm> ========` 단위로 잘라 `pick`이 고르는 절만 이어 붙인다.
fn sections(log: &str, pick: impl Fn(&str) -> bool) -> String {
    log.split("======== ")
        .filter(|sec| !sec.is_empty())
        .filter(|sec| sec.lines().next().is_some_and(&pick))
        .map(|sec| format!("======== {sec}"))
        .collect()
}

fn is_k(head: &str) -> bool {
    head.starts_with("K ")
}

#[test]
fn keyed_r1_writes_are_unchanged_since_before_the_row_spawner_refactor() {
    let golden = include_str!("golden/list-candidate-writes.txt");
    let pick = |h: &str| !is_k(h) && h.contains(" R1 ");
    assert_eq!(
        sections(&recorder_common::full_log(), pick),
        sections(golden, pick),
        "D/E 계열의 R1 방출이 리팩터 이전과 달라졌다"
    );
}

#[test]
fn keyed_r2_writes_match_the_amendment_9_baseline() {
    let golden = include_str!("golden/list-candidate-writes-r2.txt");
    let pick = |h: &str| !is_k(h) && h.contains(" R2 ");
    assert_eq!(
        sections(&recorder_common::full_log(), pick),
        sections(golden, pick),
        "D/E 계열의 R2 방출이 개정 #9-11 기준선과 달라졌다"
    );
}

#[test]
fn control_group_k_matches_the_clause_10_dispatch_baseline() {
    let golden = include_str!("golden/list-candidate-writes-k.txt");
    assert_eq!(
        sections(&recorder_common::full_log(), is_k),
        sections(golden, is_k),
        "K의 방출이 §10 디스패치 준수 기준선과 달라졌다"
    );
}

/// **§10 (2) 완전 동등 → no-op**이 골든에 실제로 나타나는지.
///
/// 위 골든 대조만으로는 "K가 조항의 네 분기를 갖췄다"가 아니라 "지금과 같다"만 말한다.
/// 레코더의 마지막 두 시퀀스가 같은 값(`["p","q","r"]` 두 번)이므로, 그 전이는 write를
/// **하나도** 내지 않아야 한다. 수리 전에는 `ClearChildren` + `SpawnRow` 3회였다.
///
/// **R1·R2를 따로 본다** (2026-09-25). 전에는 두 절을 이어 붙인 문자열에
/// `contains`를 걸었는데, 그러면 **한쪽만 no-op이어도 통과한다** — R2에서만 write가
/// 새는 회귀(조건부·바인딩 자식은 R2에만 있으므로 정확히 그 축에서 생길 결함)를
/// 못 잡는다. §9의 D 기각(iii)·D 채택 양성·E 기각(i)이 전부 R2 셀에 걸려 있다.
#[test]
fn identical_sequence_emits_no_writes_at_all() {
    let log = recorder_common::full_log();
    for arm in ["R1", "R2"] {
        let k = sections(&log, |h| is_k(h) && h.contains(&format!(" {arm} ")));
        assert!(
            !k.is_empty(),
            "골든에서 K {arm} 절을 못 찾았다 — 아래 단언이 공허해진다"
        );
        assert!(
            k.contains("--- tick 7 (0 writes) ---"),
            "K {arm}: 동일 시퀀스 전이가 write를 냈다 — §10 (2) no-op 분기가 없다:\n{k}"
        );
    }
}

// ---------------------------------------------------------------------------
// 개정 #9로 신설된 **축**들의 방출 — 앞 세 골든이 안 덮던 범위 (2026-09-25)
//
// 앞 세 골든이 덮는 것은 D/D-LIS/E/E-LIS의 R1·R2와 K의 R1·R2뿐이다. 즉
// **후보 C 전체**, **R3(변형 축)**, **W8(row 내부 리스트)**, **무조건 write 변형**은
// 방출이 고정돼 있지 않았다. 그 넷은 전부 개정 #9가 신설한 것이고 §9가 판정에 쓰는
// 셀을 포함한다(C 채택 (a)(b)·C 기각 두 분기가 전부 C의 방출에 걸려 있다).
//
// §2.5가 "구현·기반 동결 — 하네스 변경은 캠페인 무효"를 규정하므로, **수집 전에**
// 이 축들의 방출을 고정해 두는 것이 그 조항을 실제로 집행할 수 있게 하는 유일한
// 수단이다. 고정이 없으면 수집 도중·이후의 변경을 사후에 알아낼 방법이 없다.
//
// 이 레코더는 `recorder_common`과 달리 **하네스 경로를 그대로 미러한다**
// (`add_instrumented_systems` → 축 리소스 → `add_row_depth_systems`, 즉
// `loom-bench`의 `RowShape::apply`와 같은 순서). 전이 열도 §10 K 디스패치 네 분기를
// 전부 지나간다(Append → Truncate → Full → Clear → Append → NoOp).
// ---------------------------------------------------------------------------

/// 골든에 **30개 절(6후보 × 5축)이 다 있고 축마다 방출이 실제로 다른가.**
///
/// 이 검사 없이 골든 대조만 하면, 축 리소스가 어느 순간 무시되기 시작해 다섯 축이
/// 전부 R1과 같아져도 "골든과 같다"로 통과한다 — 골든을 그때 다시 뽑았을 테니까.
/// 그래서 축이 방출을 **가른다는 것 자체**를 따로 단언한다.
#[test]
fn axis_golden_covers_every_arm_and_each_axis_changes_the_emission() {
    let log = recorder_axes::full_log();
    let arms = ["C", "K", "D", "D-LIS", "E", "E-LIS"];
    let axes = ["R1", "R2", "R3", "W8-inner3", "UNCOND"];
    let mut by_axis = std::collections::HashMap::new();
    for axis in axes {
        for arm in arms {
            let head = format!("======== {arm} {axis} ========");
            assert!(log.contains(&head), "골든에 절이 없다: {head}");
        }
        // 절 머리글은 `"<후보> <축> ========"` 꼴이다 — 두 번째 토큰이 축이다
        // (후보 "D-LIS"도 한 토큰). `ends_with(axis)`로는 못 고른다.
        let body = sections(&log, |h| h.split_whitespace().nth(1) == Some(axis));
        assert!(!body.is_empty(), "축 {axis}의 절을 못 골라냈다");
        // **머리글을 뺀 write 열만 남긴다.** 머리글을 포함한 채로 비교하면 축
        // 이름의 글자 수 차이("UNCOND" 6자 vs "R1" 2자)만으로 서로 달라 보여,
        // 축이 통째로 무시돼도 이 단언이 통과한다(실제로 그랬다 — 음성 대조
        // "write 변형 무시"가 골든만 깨고 이 검사는 통과했다).
        let writes: String = body
            .lines()
            .filter(|l| !l.starts_with("========"))
            .collect::<Vec<_>>()
            .join("\n");
        by_axis.insert(axis, writes);
    }
    // R1과 다른 넷이 전부 달라야 한다 — 같으면 그 축이 방출에 반영되지 않는다.
    for axis in ["R2", "R3", "W8-inner3", "UNCOND"] {
        assert_ne!(
            by_axis[axis], by_axis["R1"],
            "축 {axis}의 방출 write 열이 R1과 **글자 하나까지 같다** — 축 리소스가 \
             무시되고 있다. 이 상태에서 골든을 다시 뽑으면 골든 대조는 통과한다."
        );
    }
}

#[test]
fn amendment_9_axis_writes_match_the_pre_collection_baseline() {
    let golden = include_str!("golden/list-candidate-writes-axes.txt");
    assert_eq!(
        recorder_axes::full_log().trim_end(),
        golden.trim_end(),
        "개정 #9 축(C·R3·W8·무조건 변형)의 방출이 동결 기준선과 달라졌다 — \
         의도된 변경이면 §2.5 재측정 절차와 함께 골든을 다시 뽑을 것"
    );
}
