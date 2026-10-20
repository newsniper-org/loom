//! G0R1 C7 §7-4 — **측정 경로**에서 비용 성질을 구조로 단언한다.
//!
//! `match_by_key` 단위 테스트(`list_key.rs`)는 매처 함수만 본다. 여기서는 하네스와 같은 경로
//! (`add_instrumented_systems` → 한 틱)로 D·D-LIS·E·E-LIS **System**을 돌려, 새 매처가 실제로 호출되는지
//! 판별한다 — 옛 매처는 `String`을 비교하므로 `Item` 비교 수가 0이라 이 단언에서 실패한다.
//!
//! `CntItem`의 `PartialEq`/`Ord`는 호출마다 1씩 센다. 역순 n=2000(무충돌, run 크기 1/1)에서 `Item` 비교는
//! **정확히 n**이다(run마다 새 원소 1회). 하한을 먼저 단언해 "세지 못해서 0"인 공허한 통과를 막는다.

mod support;
use support::MockDom;

use std::cell::Cell;
use std::hash::{Hash, Hasher};

use loom_runtime::{
    list_reconcile_d_lis_system, list_reconcile_d_system, list_reconcile_e_lis_system,
    list_reconcile_e_system, DomTarget, ListRows, LoomApp, OrderKeyMembers, Signal,
};

thread_local! { static CMP: Cell<usize> = const { Cell::new(0) }; }

#[derive(Clone, Debug)]
struct CntItem(u32);
impl PartialEq for CntItem {
    fn eq(&self, o: &Self) -> bool {
        CMP.set(CMP.get() + 1);
        self.0 == o.0
    }
}
impl Eq for CntItem {}
impl PartialOrd for CntItem {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for CntItem {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        CMP.set(CMP.get() + 1);
        self.0.cmp(&o.0)
    }
}
impl Hash for CntItem {
    fn hash<H: Hasher>(&self, h: &mut H) {
        h.write_u32(self.0);
    }
}
impl std::fmt::Display for CntItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id-{}", self.0)
    }
}

const N: u32 = 2000;

macro_rules! cost_path {
    ($name:ident, $sys:ident, $rows:ident) => {
        #[test]
        fn $name() {
            let (mut dom, root) = MockDom::new_root();
            let mut app = LoomApp::new();
            app.add_instrumented_systems($sys::<CntItem>);
            let list = app
                .world
                .spawn((Signal((0..N).map(CntItem).collect::<Vec<_>>()), $rows::default(), DomTarget(root)))
                .id();
            app.tick(&mut dom); // 마운트: 옛 행 0개 → 비교 없음
            app.world.get_mut::<Signal<Vec<CntItem>>>(list).unwrap().0 = (0..N).rev().map(CntItem).collect();
            CMP.set(0);
            app.run_schedule_for_test();
            let cmp = CMP.get();
            // 하한(존재) 먼저: 세지 못해서 0이면 안 된다.
            assert!(cmp > 0, "Item 비교가 한 번도 세어지지 않았다 — 새 매처를 안 부르거나 계수가 배선되지 않았다");
            assert_eq!(cmp, N as usize, "측정 경로의 Item 비교 수는 정확히 n이어야 한다(역순, 무충돌)");
            app.tick(&mut dom);
            let want: Vec<String> = (0..N).rev().map(|i| format!("id-{i}")).collect();
            assert_eq!(dom.child_texts(root), want);
        }
    };
}
cost_path!(d_cost_path, list_reconcile_d_system, ListRows);
cost_path!(d_lis_cost_path, list_reconcile_d_lis_system, ListRows);
cost_path!(e_cost_path, list_reconcile_e_system, OrderKeyMembers);
cost_path!(e_lis_cost_path, list_reconcile_e_lis_system, OrderKeyMembers);
