//! G0 재심사(C12, 2026-10-30)에서 적대 교차 리뷰가 찾은 C 최선형 미달 둘의 수리를 고정한다.
//!
//! 1. §2.4 [C] A-보강 2 — 전량 삭제는 `ClearChildren` 한 번(행마다 `RemoveNode` 아님). 부분 축소는 그대로 행별 `RemoveNode`.
//! 2. write마다 `to_string`이 **한 번**(새 텍스트). 이전에는 R3 전용인 옛 텍스트까지 만들어 두 번이었다.
//!
//! 공허한 확인 방지: 수리 전 코드에서 둘 다 실패해야 한다(변이 점검 기록은 커밋 메시지).

mod support;
use support::MockDom;

use std::cell::Cell;

use loom_runtime::{list_reconcile_c_system, DomTarget, DomWrite, ListRows, LoomApp, PendingDomWrites, Signal};

thread_local! { static TOSTR: Cell<usize> = const { Cell::new(0) }; }

#[derive(Clone, Debug, PartialEq)]
struct Disp(u32);
impl std::fmt::Display for Disp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        TOSTR.set(TOSTR.get() + 1);
        write!(f, "d{}", self.0)
    }
}

fn drain(app: &mut LoomApp) -> Vec<DomWrite> {
    app.run_schedule_for_test();
    std::mem::take(&mut app.world.resource_mut::<PendingDomWrites>().0)
}

fn mounted(n: u32) -> (LoomApp, MockDom, loom_runtime::NodeHandle, bevy_ecs::entity::Entity) {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(list_reconcile_c_system::<Disp>);
    let list = app
        .world
        .spawn((Signal((0..n).map(Disp).collect::<Vec<_>>()), ListRows::default(), DomTarget(root)))
        .id();
    app.tick(&mut dom);
    (app, dom, root, list)
}

#[test]
fn full_clear_is_one_clear_children_and_partial_shrink_stays_per_row() {
    let (mut app, mut dom, root, list) = mounted(5);
    assert_eq!(dom.child_texts(root).len(), 5);
    // 존재 대조(부분 축소는 행별 RemoveNode): 5 → 2
    app.world.get_mut::<Signal<Vec<Disp>>>(list).unwrap().0 = (0..2).map(Disp).collect();
    let w = drain(&mut app);
    assert_eq!(w.iter().filter(|x| matches!(x, DomWrite::RemoveNode { .. })).count(), 3, "{w:?}");
    assert_eq!(w.iter().filter(|x| matches!(x, DomWrite::ClearChildren { .. })).count(), 0);
    // 전량 삭제: 2 → 0
    app.world.get_mut::<Signal<Vec<Disp>>>(list).unwrap().0 = vec![];
    let w = drain(&mut app);
    assert_eq!(w.iter().filter(|x| matches!(x, DomWrite::ClearChildren { .. })).count(), 1, "{w:?}");
    assert_eq!(w.iter().filter(|x| matches!(x, DomWrite::RemoveNode { .. })).count(), 0, "{w:?}");
    assert!(app.world.get::<ListRows>(list).unwrap().0.is_empty());
    // 적용 결과: DOM에서 비었고, 행 entity도 없다(despawn).
    let mut app2 = mounted(4);
    app2.0.world.get_mut::<Signal<Vec<Disp>>>(app2.3).unwrap().0 = vec![];
    app2.0.tick(&mut app2.1);
    assert_eq!(app2.1.child_texts(app2.2).len(), 0);
    assert_eq!(loom_runtime::DomOps::live_node_count(&app2.1).unwrap(), 1, "루트만 남아야 한다");
    // 비운 뒤 다시 채워도 된다.
    app2.0.world.get_mut::<Signal<Vec<Disp>>>(app2.3).unwrap().0 = (0..3).map(Disp).collect();
    app2.0.tick(&mut app2.1);
    assert_eq!(app2.1.child_texts(app2.2), vec!["d0", "d1", "d2"]);
}

#[test]
fn c_conditional_calls_to_string_once_per_written_slot_and_never_for_unchanged() {
    let n = 100u32;
    let (mut app, _dom, _root, list) = mounted(n);
    // 같은 값으로 다시 쓰면(=Changed지만 슬롯 값 동일) write가 없고 to_string도 없다.
    app.world.get_mut::<Signal<Vec<Disp>>>(list).unwrap().0 = (0..n).map(Disp).collect();
    TOSTR.set(0);
    let w = drain(&mut app);
    assert_eq!(w.len(), 0);
    assert_eq!(TOSTR.get(), 0, "바뀐 슬롯이 없으면 to_string도 없다");
    // 역순: 짝수 길이라 모든 슬롯이 바뀐다 → write n, to_string 정확히 n.
    app.world.get_mut::<Signal<Vec<Disp>>>(list).unwrap().0 = (0..n).rev().map(Disp).collect();
    TOSTR.set(0);
    let w = drain(&mut app);
    let writes = w.iter().filter(|x| matches!(x, DomWrite::SetText { .. })).count();
    assert!(writes > 0, "존재 단언: write가 나가야 한다");
    assert_eq!(writes, n as usize);
    assert_eq!(TOSTR.get(), writes, "write당 to_string 한 번(옛 텍스트는 R3 전용)");
}
