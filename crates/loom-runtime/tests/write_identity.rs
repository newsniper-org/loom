//! **리팩터 이전 코드가 실제로 낸 write 열**과 지금 코드의 write 열이 같은지.
//!
//! `tests/golden/list-candidate-writes.txt`는 `RowSpawner` 도입 **직전 커밋**
//! (f0eaf6f)을 별도 워크트리에 체크아웃해 같은 레코더(`recorder_common.rs`)로
//! 뽑은 것이다 — 손으로 적은 기대값이 아니다. 이게 통과한다는 것은 트랙 L
//! 후보 다섯의 방출이 리팩터로 인해 바뀌지 않았다는 뜻이고, 따라서 v11~v18
//! 수집분이 §2.5의 재측정 대상이 되지 않는다는 뜻이다.
//!
//! 후보 동작을 **의도적으로** 바꿀 때는 이 파일을 지우지 말고 골든을 다시
//! 뽑되, 그때는 §2.5 재측정 절차를 함께 밟을 것.

mod recorder_common;

#[test]
fn track_l_candidate_writes_are_unchanged_by_the_row_spawner_refactor() {
    let golden = include_str!("golden/list-candidate-writes.txt");
    assert_eq!(
        recorder_common::full_log(),
        golden.trim_end_matches('\n'),
        "트랙 L 후보의 방출 write 열이 리팩터 이전과 달라졌다"
    );
}
