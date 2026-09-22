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
#[test]
fn identical_sequence_emits_no_writes_at_all() {
    let k = sections(&recorder_common::full_log(), is_k);
    assert!(
        k.contains("--- tick 7 (0 writes) ---"),
        "동일 시퀀스 전이가 write를 냈다 — §10 (2) no-op 분기가 없다:\n{k}"
    );
}
