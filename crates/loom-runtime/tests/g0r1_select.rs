//! G0R1(개정 #12) C4·C7 — key·값 채널 연결 뒤의 구조 단언.
//!
//! - `RowKey`/`OrderedKey` 없는 row가 `ListRows`/`OrderKeyMembers`에 있으면 debug에서 터진다(조용히 빠지면 stale DOM).
//! - 퇴화 `Hash`(상수) 아이템 타입으로 D·D-LIS·E·E-LIS를 연쇄 틱으로 돌려도 DOM 결과가 정확하다 —
//!   **충돌 계수 > 0을 먼저 단언**한 뒤(충돌 경로가 실제로 실행됐다는 존재 단언) DOM을 문자열 오라클과 비교한다.

mod support;
use support::MockDom;

use std::hash::{Hash, Hasher};

use loom_runtime::{
    list_reconcile_d_lis_system, list_reconcile_d_system, list_reconcile_e_lis_system,
    list_reconcile_e_system, DomTarget, ListRows, LoomApp, OrderKeyMembers, Signal,
};

/// `a == b ⇒ hash(a) == hash(b)`를 지키는 가장 나쁜 Hash: 모든 값이 같은 해시.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Deg(String);
impl Hash for Deg {
    fn hash<H: Hasher>(&self, h: &mut H) {
        h.write_u8(0);
    }
}
impl std::fmt::Display for Deg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn items(xs: &[&str]) -> Vec<Deg> {
    xs.iter().map(|s| Deg(s.to_string())).collect()
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "key·값 채널이 없는 row")]
fn row_without_key_in_list_rows_panics_in_debug() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(list_reconcile_d_system::<String>);
    let stray = app.world.spawn_empty().id(); // RowKey 없음
    app.world.spawn((
        Signal(vec!["a".to_string()]),
        ListRows(vec![stray]),
        DomTarget(root),
    ));
    app.tick(&mut dom);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "key·값 채널이 없는 row")]
fn row_without_key_in_members_panics_in_debug() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(list_reconcile_e_system::<String>);
    let stray = app.world.spawn_empty().id();
    app.world.spawn((
        Signal(vec!["a".to_string()]),
        OrderKeyMembers(vec![stray]),
        DomTarget(root),
    ));
    app.tick(&mut dom);
}

macro_rules! collision_chain {
    ($name:ident, $sys:ident, $rows:ident) => {
        #[test]
        fn $name() {
            let seq: [&[&str]; 6] = [
                &["a", "b", "c", "d"],
                &["d", "c", "b", "a"],
                &["b", "x", "a", "a", "d"],
                &["a", "a", "a"],
                &[],
                &["q", "a", "b", "q"],
            ];
            let (mut dom, root) = MockDom::new_root();
            let mut app = LoomApp::new();
            app.add_systems($sys::<Deg>);
            let e = app
                .world
                .spawn((Signal(Vec::<Deg>::new()), $rows::default(), DomTarget(root)))
                .id();
            let mut prev_len = 0usize;
            for step in seq {
                #[cfg(debug_assertions)]
                let before = loom_runtime::list_key::KEY_COLLISIONS.load(std::sync::atomic::Ordering::Relaxed);
                app.world.get_mut::<Signal<Vec<Deg>>>(e).unwrap().0 = items(step);
                app.tick(&mut dom);
                // 존재 단언을 먼저: 옛·새가 모두 비어 있지 않은 단계에서는 상수 해시 때문에 한 run에 서로 다른
                // Item이 섞이므로 충돌 경로가 반드시 돌아야 한다. (이 단계의 DOM 정확성은 그 뒤에 본다.)
                #[cfg(debug_assertions)]
                if prev_len > 0 && !step.is_empty() {
                    let now = loom_runtime::list_key::KEY_COLLISIONS.load(std::sync::atomic::Ordering::Relaxed);
                    assert!(now > before, "충돌 경로가 실행되지 않았다: {step:?}");
                }
                prev_len = step.len();
                assert_eq!(
                    dom.child_texts(root),
                    step.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                    "충돌 key 아래에서 DOM이 틀렸다: {step:?}"
                );
            }
        }
    };
}
collision_chain!(d_collision_chain, list_reconcile_d_system, ListRows);
collision_chain!(d_lis_collision_chain, list_reconcile_d_lis_system, ListRows);
collision_chain!(e_collision_chain, list_reconcile_e_system, OrderKeyMembers);
collision_chain!(e_lis_collision_chain, list_reconcile_e_lis_system, OrderKeyMembers);
