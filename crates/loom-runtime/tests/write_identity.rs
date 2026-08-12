//! 트랙 L 후보들이 내는 **write 열**을 골든으로 고정한다.
//!
//! 골든이 두 개인 이유(2026-08-12, 개정 #9-11):
//!
//! - `golden/list-candidate-writes.txt` — `RowSpawner` 도입 **직전 커밋**(f0eaf6f)을
//!   별도 워크트리에 체크아웃해 같은 레코더(`recorder_common.rs`)로 뽑은 것. 손으로
//!   적은 기대값이 아니다. **이 파일의 R1 구간은 계속 유효하고, 이 테스트가 그것을
//!   그대로 대조한다** — R2 축을 조항 원문대로 바꾼 변경이 R1로 새지 않았다는 증거다
//!   (실제로 안 샜음을 확인하고 이 구조를 잡았다).
//! - `golden/list-candidate-writes-r2.txt` — R2 구간의 **새 기준선**. 개정 #9-11로
//!   R2가 "정적 텍스트 자식 n−1개"에서 "`RowBinding` entity n−1개 + 중첩 조건부 1개"로
//!   바뀌었으므로 옛 R2 골든은 더 이상 기대값이 아니다. 이 파일은 그 변경 **이후**에
//!   뽑았고, 따라서 "리팩터로 안 바뀌었다"의 증거가 아니라 **"여기서부터 다시 안
//!   바뀐다"의 기준선**이다. 그 차이를 흐리지 않기 위해 파일을 나눴다.
//!
//! 후보 동작을 **의도적으로** 바꿀 때는 이 파일을 지우지 말고 해당 골든을 다시
//! 뽑되, 그때는 §2.5 재측정 절차를 함께 밟을 것.

mod recorder_common;

/// 로그를 `======== <arm> ========` 단위로 잘라 R1/R2 구간을 분리한다.
fn sections(log: &str, want_r2: bool) -> String {
    log.split("======== ")
        .filter(|sec| !sec.is_empty())
        .filter(|sec| sec.lines().next().is_some_and(|h| h.contains(" R2 ") == want_r2))
        .map(|sec| format!("======== {sec}"))
        .collect()
}

#[test]
fn r1_writes_are_unchanged_since_before_the_row_spawner_refactor() {
    let golden = include_str!("golden/list-candidate-writes.txt");
    assert_eq!(
        sections(&recorder_common::full_log(), false),
        sections(golden, false),
        "R1 방출이 리팩터 이전과 달라졌다 — R2 축 변경이 R1로 샜거나 다른 회귀다"
    );
}

#[test]
fn r2_writes_match_the_post_amendment_baseline() {
    let golden = include_str!("golden/list-candidate-writes-r2.txt");
    assert_eq!(
        sections(&recorder_common::full_log(), true),
        sections(golden, true),
        "R2 방출이 개정 #9-11 기준선과 달라졌다"
    );
}
