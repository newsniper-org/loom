mod support;
use support::MockDom;
use loom_runtime::{DomTarget, ListRows, LoomApp, RowVariantA, Signal, StageTimings};

#[test]
fn c_r3_archetype_move_lands_in_sync_point() {
    for variants in [false, true] {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_variant_mode(variants);
        app.add_instrumented_systems(loom_runtime::list_reconcile_c_system::<String>);
        let initial: Vec<String> = (0..1000).map(|i| format!("item-{i}")).collect();
        let e = app.world.spawn((Signal(initial.clone()), ListRows::default(), DomTarget(root))).id();
        app.tick(&mut dom);
        let rev: Vec<String> = initial.iter().rev().cloned().collect();
        app.world.get_mut::<Signal<Vec<String>>>(e).unwrap().0 = rev;
        app.tick(&mut dom);
        let t = *app.world.get_resource::<StageTimings>().unwrap();
        let marked = app.world.get::<ListRows>(e).unwrap().0.iter()
            .filter(|&&r| app.world.get::<RowVariantA>(r).is_some()).count();
        println!("R3={variants}: systems={:.4} sync={:.4} dom={:.4} total={:.4} sync/total={:.1}% | RowVariantA 보유={marked}",
                 t.systems_ms, t.sync_point_ms, t.dom_apply_ms, t.total_ms,
                 if t.total_ms>0.0 { t.sync_point_ms/t.total_ms*100.0 } else {0.0});
    }
}
