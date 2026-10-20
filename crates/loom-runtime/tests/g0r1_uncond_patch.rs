//! G0R1 C5·C7 §7-6 — D 무조건 변형의 patch 경로 구조 단언(R1·R2·R3, D·D-LIS).
//!
//! (a) retained > 0인 갱신에서 `SetText`가 조건부 대비 **정확히 retained 수만큼** 더 나온다(존재 단언 포함).
//! (b) 틱 t에 spawn된 fresh 행은 틱 t+1(append)에서 **retain으로 한 번만** 쓰인다 — 위 (a)의 정확한 개수 단언이
//!     곧 "초기값 후속 재패치(§2.2) 부재"의 단언이다(재패치가 있으면 개수가 retained보다 커진다). 변이 점검:
//!     patch를 창 밖(순서 제약 없음)에 등록하면 이 테스트 6건이 모두 실패함을 확인했다(2026-10-20).
//!     (이 결과가 B5 "초기값을 Added 후속 스테이지 한 곳에서만 낸다" 대안의 필요 여부를 정한다 → 불필요.)
//! (c) 같은 입력을 두 번 돌리면 방출 순서가 같다(결정성).

mod support;
use support::MockDom;

use loom_runtime::{
    list_reconcile_d_lis_system, list_reconcile_d_lis_unconditional_system, list_reconcile_d_system,
    list_reconcile_d_unconditional_system, DomTarget, DomWrite, ListRows, LoomApp, PendingDomWrites,
    Signal, WriteVariant,
};

#[derive(Clone, Copy, Debug)]
enum Axis {
    R1,
    R2,
    R3,
}
#[derive(Clone, Copy, Debug)]
enum Cand {
    D,
    DLis,
}

fn build(cand: Cand, axis: Axis, uncond: bool) -> (MockDom, LoomApp, bevy_ecs::entity::Entity) {
    let (dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    match (cand, uncond) {
        (Cand::D, false) => app.add_instrumented_systems(list_reconcile_d_system::<String>),
        (Cand::D, true) => app.add_instrumented_systems(list_reconcile_d_unconditional_system::<String>),
        (Cand::DLis, false) => app.add_instrumented_systems(list_reconcile_d_lis_system::<String>),
        (Cand::DLis, true) => app.add_instrumented_systems(list_reconcile_d_lis_unconditional_system::<String>),
    };
    app.set_write_variant(if uncond { WriteVariant::Unconditional } else { WriteVariant::Conditional });
    match axis {
        Axis::R1 => {}
        Axis::R2 => {
            app.set_row_bindings(8);
        }
        Axis::R3 => {
            app.set_row_variant_mode(true);
        }
    }
    if matches!(axis, Axis::R2) {
        app.add_row_depth_systems(); // 창은 depth가 만든다
    } else {
        app.add_row_patch_window(); // R1·R3: 전 인스턴스에 창(개정 #12 계측 대칭)
    }
    if uncond {
        app.add_row_value_patch::<String>();
    }
    let list = app
        .world
        .spawn((Signal(Vec::<String>::new()), ListRows::default(), DomTarget(root)))
        .id();
    (dom, app, list)
}

/// 한 스텝: 리스트를 바꾸고 방출된 `SetText`(텍스트 열)를 돌려준다. 이어서 틱을 적용해 DomTarget을 붙인다.
fn step(dom: &mut MockDom, app: &mut LoomApp, list: bevy_ecs::entity::Entity, next: &[&str]) -> Vec<String> {
    app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 = next.iter().map(|s| s.to_string()).collect();
    app.run_schedule_for_test();
    let texts: Vec<String> = app
        .world
        .resource::<PendingDomWrites>()
        .0
        .iter()
        .filter_map(|w| match w {
            DomWrite::SetText { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    app.tick(dom);
    texts
}

const SEQ: &[(&[&str], usize)] = &[
    (&["a", "b"], 0),            // 전부 fresh
    (&["a", "b", "c"], 2),       // append: 옛 2행 retained, c는 fresh
    (&["a", "b", "c", "d"], 3),  // append: 직전 틱의 fresh c도 retained — 재패치가 아니라 이번 갱신의 retain
    (&["d", "c", "a", "b"], 4),  // 재정렬
    (&["d", "x", "a"], 2),       // 일부 제거·추가
];

fn run_all(cand: Cand, axis: Axis) {
    let (mut dc, mut ac, lc) = build(cand, axis, false);
    let (mut du, mut au, lu) = build(cand, axis, true);
    let mut total_retained = 0;
    for (i, (next, retained)) in SEQ.iter().enumerate() {
        let c = step(&mut dc, &mut ac, lc, next);
        let u = step(&mut du, &mut au, lu, next);
        // 존재 단언을 먼저: retained > 0이면 무조건 쪽이 조건부보다 SetText를 더 낸다.
        if *retained > 0 {
            assert!(u.len() > c.len(), "{cand:?}/{axis:?} step {i}: 무조건이 더 쓰지 않는다 (cond {}, uncond {})", c.len(), u.len());
        }
        assert_eq!(
            u.len() - c.len(),
            *retained,
            "{cand:?}/{axis:?} step {i}: 무조건의 추가 SetText 수가 retained 수와 다르다 \
             (거짓 SetText 또는 누락) — cond {c:?} / uncond {u:?}"
        );
        total_retained += retained;
        // DOM 결과는 두 변형이 같다.
        assert_eq!(dc.child_texts(root_of(&ac, lc)), dc.child_texts(root_of(&ac, lc)));
        assert_eq!(
            du.child_texts(root_of(&au, lu)),
            next.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
    }
    assert!(total_retained > 0);
}

fn root_of(app: &LoomApp, list: bevy_ecs::entity::Entity) -> loom_runtime::NodeHandle {
    app.world.get::<DomTarget>(list).unwrap().0
}

#[test]
fn d_r1() { run_all(Cand::D, Axis::R1); }
#[test]
fn d_r2() { run_all(Cand::D, Axis::R2); }
#[test]
fn d_r3() { run_all(Cand::D, Axis::R3); }
#[test]
fn d_lis_r1() { run_all(Cand::DLis, Axis::R1); }
#[test]
fn d_lis_r2() { run_all(Cand::DLis, Axis::R2); }
#[test]
fn d_lis_r3() { run_all(Cand::DLis, Axis::R3); }

/// (c) 결정성: 같은 입력 → 같은 방출 열.
#[test]
fn emission_order_is_deterministic() {
    for axis in [Axis::R1, Axis::R2, Axis::R3] {
        let mut runs = Vec::new();
        for _ in 0..2 {
            let (mut d, mut app, list) = build(Cand::D, axis, true);
            let mut all = Vec::new();
            for (next, _) in SEQ {
                all.push(step(&mut d, &mut app, list, next));
            }
            runs.push(all);
        }
        assert_eq!(runs[0], runs[1], "{axis:?}");
    }
}
