//! RQ1 §3.2의 **R3 축(이질 브랜치 row)이 후보마다 실제로 적용되는지**.
//!
//! 2026-08-07까지 이 축을 적용하는 후보는 C 하나뿐이었다 — D/D-LIS/E/E-LIS는
//! `RowVariantMode`를 시스템 파라미터로 **선언만 하고 읽지 않았고**, K는 선언조차
//! 없었다. 그래서 수집분(v11~v18) 전체에 `row_variants: true` 셀이 6후보 모두에
//! 기록돼 있었지만 C 외 다섯은 **R1 셀에 R3 라벨이 붙은 것**이었다.
//! §3.3이 "R3: W2 × R3 × n=1,000 × {C, D}"를 사전등록해 두었는데 D 열이 측정된
//! 적이 없었다는 뜻이다.
//!
//! 이 파일은 그 어긋남이 다시 생기지 않게 고정한다. 계측이 "존재한다"와
//! "조항이 요구하는 걸 잰다"는 다르고, 그 차이는 **판정을 실제로 돌려봐야만**
//! 드러난다 — 여기서는 축을 켜고 실제로 marker가 붙는지 본다.

use bevy_ecs::entity::Entity;
use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, list_reconcile_k_system, DomTarget,
    ListRows, LoomApp, NodeHandle, OrderKeyMembers, PendingDomWrites, RowVariantA, RowVariantB,
    Signal, SlotValue,
};

/// `row_variant_is_a`는 마지막 바이트가 짝수면 A. 아래 아이템들은 A/B가 섞이게
/// 골랐다(그래야 "전부 A" 같은 무의미한 통과가 안 나온다).
const ITEMS: &[&str] = &["a0", "a1", "a2", "a3"];

enum Storage {
    Rows,
    OrderKeys,
    Slots,
}

fn run(register: impl FnOnce(&mut LoomApp), storage: Storage, variants: bool) -> (usize, usize) {
    let mut app = LoomApp::new();
    app.set_row_variant_mode(variants);
    register(&mut app);
    let initial: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    let anchor = NodeHandle(0);
    let list = match storage {
        Storage::OrderKeys => app
            .world
            .spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(anchor)))
            .id(),
        _ => app.world.spawn((Signal(initial), ListRows::default(), DomTarget(anchor))).id(),
    };
    app.run_schedule_for_test();
    app.world.resource_mut::<PendingDomWrites>().0.clear();

    let rows: Vec<Entity> = if let Some(r) = app.world.get::<ListRows>(list) {
        r.0.clone()
    } else {
        app.world.get::<OrderKeyMembers>(list).map(|m| m.0.clone()).unwrap_or_default()
    };
    assert_eq!(rows.len(), ITEMS.len(), "row가 안 만들어졌다");
    // C는 슬롯 entity에 `SlotValue`를 두므로 존재만 확인해 둔다(후보 구분용).
    if matches!(storage, Storage::Slots) {
        assert!(app.world.get::<SlotValue>(rows[0]).is_some());
    }
    let a = rows.iter().filter(|e| app.world.get::<RowVariantA>(**e).is_some()).count();
    let b = rows.iter().filter(|e| app.world.get::<RowVariantB>(**e).is_some()).count();
    (a, b)
}

macro_rules! axis_applies {
    ($name:ident, $sys:ident, $storage:expr) => {
        #[test]
        fn $name() {
            // 축을 끄면 marker가 하나도 없어야 한다(= 기존 R1/R2 셀 무변경).
            let (a, b) = run(|app| { app.add_systems($sys::<String>); }, $storage, false);
            assert_eq!((a, b), (0, 0), "축이 꺼져 있는데 브랜치 marker가 붙었다");

            // 켜면 모든 row가 값에 따라 A/B 중 하나를 갖고, 둘 다 나와야 한다.
            let (a, b) = run(|app| { app.add_systems($sys::<String>); }, $storage, true);
            assert_eq!(a + b, ITEMS.len(), "축이 켜졌는데 브랜치가 배정되지 않은 row가 있다");
            assert!(a > 0 && b > 0, "A/B가 갈리지 않았다 (a={a}, b={b}) — 축이 값에 반응하지 않는다");
        }
    };
}

axis_applies!(candidate_c, list_reconcile_c_system, Storage::Slots);
axis_applies!(candidate_d, list_reconcile_d_system, Storage::Rows);
axis_applies!(candidate_d_lis, list_reconcile_d_lis_system, Storage::Rows);
axis_applies!(candidate_e, list_reconcile_e_system, Storage::OrderKeys);
axis_applies!(candidate_e_lis, list_reconcile_e_lis_system, Storage::OrderKeys);
axis_applies!(candidate_k, list_reconcile_k_system, Storage::Rows);

#[test]
fn keyed_candidates_do_not_transition_branches_on_reorder() {
    // RQ1이 예측한 비대칭: keyed 후보는 retained row의 아이템이 바뀌지 않으므로
    // 브랜치 전환(= archetype move)이 일어나지 않는다. **이제 이 명제는 marker가
    // 실제로 존재하는 상태에서 성립한다** — 2026-08-07 이전에는 marker 자체가
    // 없어서 "전환이 없다"가 keyed 성질이 아니라 구현 누락으로 참이었다(공허성).
    let mut app = LoomApp::new();
    app.set_row_variant_mode(true);
    app.add_systems(list_reconcile_d_system::<String>);
    let initial: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    let list = app
        .world
        .spawn((Signal(initial), ListRows::default(), DomTarget(NodeHandle(0))))
        .id();
    app.run_schedule_for_test();

    let before: Vec<(Entity, bool)> = app
        .world
        .get::<ListRows>(list)
        .unwrap()
        .0
        .iter()
        .map(|e| (*e, app.world.get::<RowVariantA>(*e).is_some()))
        .collect();
    // **공허성 방지**: marker가 아예 없으면 "전환이 없다"는 자동으로 참이 된다 —
    // 이 결함을 재주입해 보면 이 단언 없이는 테스트가 그냥 통과한다(확인함).
    let with_marker = before
        .iter()
        .filter(|(e, _)| {
            app.world.get::<RowVariantA>(*e).is_some() || app.world.get::<RowVariantB>(*e).is_some()
        })
        .count();
    assert_eq!(with_marker, ITEMS.len(), "브랜치 marker가 없다 — 아래 단언이 공허해진다");

    // 역순 재정렬 — 전부 retained.
    app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 =
        ITEMS.iter().rev().map(|s| s.to_string()).collect();
    app.run_schedule_for_test();

    for (entity, was_a) in before {
        let now_a = app.world.get::<RowVariantA>(entity).is_some();
        assert_eq!(was_a, now_a, "keyed 후보에서 브랜치가 전환됐다 — 예측과 다르다");
    }
}
