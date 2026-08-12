//! §10의 **write 변형 축**(조건부 `PartialEq` vs 무조건 write)이 C·D에 실제로
//! 적용되는지. 개정 #9-6의 "수집 전 이행 확인 의무".
//!
//! 왜 이 축이 판정에 걸려 있나: §9 서두가 "이중 측정 셀의 판정은 각 후보의
//! **셀별 우세 write 변형** 기준(양 변형이 모두 측정되므로 선택은 기계적 —
//! 사후 재량 없음)"이라고 사전등록했다. 2026-08-12까지 조건부 하나만 구현돼
//! 있었으므로 그 규정은 **작동할 수 없었고**, 게다가 §10의 근거 문언이
//! "재정렬 셀에서 **C 최선 = 무조건**"을 지목하므로 C가 자기 최선형으로 측정되고
//! 있지 않았다(§2.4 최선형 규율 위반).
//!
//! **여기서 막는 공허성**: "변형 플래그가 있다"는 플래그를 아무도 안 읽어도
//! 참이다(개정 #8이 R3 축에서 정확히 그 상태였다). 그래서 단언하는 것은
//! **두 변형이 서로 다른 write 열을 낸다**는 사실이다.

mod support;
use support::MockDom;

use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_system, DomTarget, DomWrite, ListRows, LoomApp,
    PendingDomWrites, Signal, WriteVariant,
};

const ITEMS: &[&str] = &["a", "b", "c", "d"];

/// 리스트를 `next`로 바꾸고 그 스텝에 방출된 `SetText`의 수를 돌려준다.
fn set_texts_for(
    register: impl FnOnce(&mut LoomApp),
    variant: WriteVariant,
    next: Vec<String>,
) -> usize {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.set_write_variant(variant);
    register(&mut app);
    let initial: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    let list = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id();
    app.tick(&mut dom);

    app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 = next;
    app.run_schedule_for_test();
    let n = app
        .world
        .resource::<PendingDomWrites>()
        .0
        .iter()
        .filter(|w| matches!(w, DomWrite::SetText { .. }))
        .count();
    app.tick(&mut dom);
    n
}

/// 순서만 뒤집는다 — **아이템 집합은 그대로**다. 그래야 "비교했으면 안 써도 되는"
/// 상황이 만들어져 두 변형이 갈린다.
fn reversed() -> Vec<String> {
    ITEMS.iter().rev().map(|s| s.to_string()).collect()
}

#[test]
fn c_unconditional_writes_every_slot_even_when_the_value_is_identical() {
    // 역순 재정렬이면 C의 슬롯 값은 **거의 전부** 바뀐다(위치 귀속이므로).
    // 그러나 짝수 길이 역순에서도 값이 그대로인 슬롯이 생길 수 있으므로,
    // 확실히 갈리는 경우로 **아무것도 안 바뀐 목표**를 쓴다.
    let same: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();

    let cond = set_texts_for(
        |a| { a.add_instrumented_systems(list_reconcile_c_system::<String>); },
        WriteVariant::Conditional,
        same.clone(),
    );
    let uncond = set_texts_for(
        |a| { a.add_instrumented_systems(list_reconcile_c_system::<String>); },
        WriteVariant::Unconditional,
        same,
    );

    assert_eq!(cond, 0, "조건부 변형은 값이 같으면 아무것도 안 써야 한다");
    assert_eq!(
        uncond,
        ITEMS.len(),
        "무조건 변형은 비교 없이 전 슬롯을 써야 한다 — 두 변형이 같은 write를 내면 \
         §9 서두의 '셀별 우세 변형' 규정이 고를 것이 없다"
    );
}

#[test]
fn d_unconditional_rewrites_retained_rows_but_conditional_does_not() {
    // D는 키(= 아이템 텍스트)로 매칭하므로 retained row의 텍스트는 **정의상 같다**.
    // 조건부 구현은 그래서 아무것도 안 쓴다. 비교하지 않는 구현은 전 행을 다시 쓴다.
    let cond = set_texts_for(
        |a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); },
        WriteVariant::Conditional,
        reversed(),
    );
    let uncond = set_texts_for(
        |a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); },
        WriteVariant::Unconditional,
        reversed(),
    );

    assert_eq!(cond, 0, "D의 조건부 변형은 retained row를 다시 쓰지 않는다");
    assert_eq!(
        uncond,
        ITEMS.len(),
        "D의 무조건 변형은 대상 행 전부에 텍스트를 다시 써야 한다"
    );
}

#[test]
fn unconditional_does_not_rewrite_freshly_spawned_rows() {
    // 새로 spawn된 행은 `RowSpawner::emit`이 이미 텍스트를 실었다. 거기에 또 쓰면
    // "비교 생략"이 아니라 같은 행을 두 번 쓰는 것이고, 무조건 변형의 비용을
    // 실제보다 크게 잡아 D에 부당하게 불리해진다.
    let mut grown: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    grown.push("zz".to_string());
    let uncond = set_texts_for(
        |a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); },
        WriteVariant::Unconditional,
        grown,
    );
    assert_eq!(
        uncond,
        ITEMS.len(),
        "새 행은 제외하고 retained 행 {}개만 다시 써야 한다",
        ITEMS.len()
    );
}
