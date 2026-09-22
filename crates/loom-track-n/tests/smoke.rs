//! 트랙 N 하네스 자체의 회귀 테스트. **캠페인이 아니다** — 워크스페이스 테스트가
//! 도는 동안 하네스가 컴파일되고, 다섯 arm이 전부 부팅되며, 초기 상태가
//! 참조 모델과 일치하는지만 확인한다(§5 사전등록 수치와 무관).
//!
//! 이게 있어야 하는 이유: 하네스가 조용히 깨지면 "통과"가 계속 나온다.
//! 초기 상태 대조는 픽스처 상수(`INITIAL_OUTER`/`INITIAL_INNER`)와 실제 TSX가
//! 어긋나는 것을 첫 tick에서 잡는다.

use loom_track_n::{ops, Arm};

#[test]
fn every_arm_boots_and_matches_the_reference_model_at_step_zero() {
    for arm in Arm::ALL {
        let outcome = loom_track_n::run(arm, &[]);
        assert!(
            outcome.ok(),
            "{} arm이 초기 상태부터 어긋난다: {:#?}",
            arm.name(),
            outcome.violations
        );
    }
}

#[test]
fn a_short_sequence_passes_on_every_arm() {
    for arm in Arm::ALL {
        for case in 0..8u64 {
            let ops = ops::sequence(41_000 + case, 40, 3, 4);
            let outcome = loom_track_n::run(arm, &ops);
            assert!(
                outcome.ok(),
                "{} arm, case {case}: {:#?}",
                arm.name(),
                outcome.violations
            );
        }
    }
}

#[test]
fn the_same_seed_produces_the_same_dom_op_sequence() {
    // §5 불변식 5의 결정성 쪽. 이게 깨지면 반례의 시드 재현이 성립하지 않는다.
    for arm in Arm::ALL {
        let ops = ops::sequence(777, 60, 3, 4);
        let a = loom_track_n::run(arm, &ops);
        let b = loom_track_n::run(arm, &ops);
        assert_eq!(a.op_log_hash, b.op_log_hash, "{} arm이 비결정적이다", arm.name());
        assert_eq!(a.dropped_writes, 0, "{} arm에서 write가 버려졌다", arm.name());
    }
}

#[test]
fn keyed_arms_are_the_ones_that_retain_row_identity() {
    // 참조 모델이 arm별 의미론을 실제로 구분하는지 — 구분이 없으면 불변식 1이
    // 어느 한쪽에서 공허해진다(모두 같은 모델로 통과해 버린다).
    //
    // **2026-09-22 갱신**: 전이를 재정렬로 바꿨다. 이전에는 `[a,bb] → [a,bb,cc]`
    // (순수 append)를 썼는데, §10 "K: 디스패치 규칙"의 (3)번 분기가 구현되면서
    // 그 전이에서 K는 **retain한다**. 즉 옛 전이는 더 이상 "재생성"을 유발하지
    // 않으므로 arm 구분을 못 세운다. 분기 (4)를 확실히 타는 재정렬을 쓴다.
    use loom_track_n::model::{Model, RowIdentity};
    let outer: Vec<String> = ["a", "bb"].iter().map(|s| s.to_string()).collect();
    let inner: Vec<String> = ["p", "q"].iter().map(|s| s.to_string()).collect();
    let mut retained = Model::new(RowIdentity::Retained, outer.clone(), false, inner.clone());
    let mut recreated = Model::new(RowIdentity::Recreated, outer.clone(), false, inner.clone());
    for m in [&mut retained, &mut recreated] {
        m.set_open(true); // 내용 생성
        m.set_open(false); // 숨김
        // 재정렬 = §10 (4) "그 외" → K는 전량 재생성한다.
        m.set_outer(vec!["bb".to_string(), "a".to_string()]);
    }
    assert!(retained.render_rows().contains("hidden"), "retained는 숨김 상태를 유지해야 한다");
    assert!(
        !recreated.render_rows().contains("hidden"),
        "recreated는 재생성으로 숨김 상태가 사라져야 한다"
    );
}

#[test]
fn recreated_arm_follows_the_clause_10_dispatch_not_blanket_recreation() {
    // 위 검사의 짝 — K가 **언제나** 재생성하는 게 아니라 §10의 분기를 탄다는 것.
    // 이게 없으면 "재생성으로 상태가 사라진다"가 모든 전이에 대해 참인 모델도
    // 통과하고, 그러면 W5(append)·W9(clear)에서 참조 모델이 구현과 어긋난다.
    use loom_track_n::model::{Model, RowIdentity};
    let outer: Vec<String> = ["a", "bb"].iter().map(|s| s.to_string()).collect();
    let inner: Vec<String> = ["p", "q"].iter().map(|s| s.to_string()).collect();

    // (3) 순수 append → 기존 row는 숨김 상태를 유지한다.
    let mut m = Model::new(RowIdentity::Recreated, outer.clone(), false, inner.clone());
    m.set_open(true);
    m.set_open(false);
    m.set_outer(vec!["a".to_string(), "bb".to_string(), "cc".to_string()]);
    assert!(
        m.render_rows().contains("hidden"),
        "§10 (3) 순수 append에서 K가 기존 row를 재생성했다 — 조항은 count-diff다"
    );

    // (2) 완전 동등 → 아무 일도 없다.
    let mut m = Model::new(RowIdentity::Recreated, outer.clone(), false, inner.clone());
    m.set_open(true);
    m.set_open(false);
    let before = m.render_rows();
    m.set_outer(outer.clone());
    assert_eq!(m.render_rows(), before, "§10 (2) 완전 동등인데 모델이 바뀌었다");
}
