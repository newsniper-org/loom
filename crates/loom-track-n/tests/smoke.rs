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
    use loom_track_n::model::{Model, RowIdentity};
    let outer: Vec<String> = ["a", "bb"].iter().map(|s| s.to_string()).collect();
    let inner: Vec<String> = ["p", "q"].iter().map(|s| s.to_string()).collect();
    let mut retained = Model::new(RowIdentity::Retained, outer.clone(), false, inner.clone());
    let mut recreated = Model::new(RowIdentity::Recreated, outer.clone(), false, inner.clone());
    for m in [&mut retained, &mut recreated] {
        m.set_open(true); // 내용 생성
        m.set_open(false); // 숨김
        m.set_outer(vec!["a".to_string(), "bb".to_string(), "cc".to_string()]);
    }
    assert!(retained.render_rows().contains("hidden"), "retained는 숨김 상태를 유지해야 한다");
    assert!(
        !recreated.render_rows().contains("hidden"),
        "recreated는 재생성으로 숨김 상태가 사라져야 한다"
    );
}
