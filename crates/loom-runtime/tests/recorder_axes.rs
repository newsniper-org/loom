// 개정 #9로 신설된 **축들**의 방출 write 열 레코더 — R3(변형)·W8(row 내부 리스트)·
// 무조건 write 변형, 그리고 후보 **C**.
//
// **왜 `recorder_common.rs`와 따로 있나**: 그쪽 세 골든은 "리팩터 이전과 한 바이트도
// 안 바뀌었다"를 주장하는 **동결된 기준선**이다. 거기에 축을 얹으려면 레코더의 앱
// 구성 자체를 바꿔야 하고(`add_systems` → `add_instrumented_systems` +
// `add_row_depth_systems`), 그러면 기준선이 흔들려 그 주장을 못 한다. 그래서 새
// 축들은 새 레코더 + 새 골든으로 따로 고정한다.
//
// **하네스 경로를 그대로 미러한다**: `loom-bench`의 `RowShape::apply`와 같은 순서 —
// 후보 System을 `add_instrumented_systems`로 먼저 등록하고, 그 다음 축 리소스를
// 세우고, 깊이 축이 켜져 있으면 `add_row_depth_systems`. 순서가 다르면 골든이
// "측정되는 경로"가 아니라 다른 경로를 고정하게 된다(v21을 죽인 결함이 정확히
// 그 형태였다 — 검증된 경로 ≠ 측정되는 경로).

use bevy_ecs::entity::Entity;
use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, list_reconcile_k_system, DomTarget,
    DomWrite, ListRows, LoomApp, NodeHandle, OrderKeyMembers, PendingDomWrites, Signal,
    WriteVariant,
};

struct Ids(std::collections::HashMap<String, usize>);

impl Ids {
    fn new() -> Self {
        Ids(std::collections::HashMap::new())
    }
    fn of(&mut self, e: &Entity) -> String {
        let raw = format!("{e:?}");
        let n = self.0.len();
        let idx = *self.0.entry(raw).or_insert(n);
        format!("e{idx}")
    }
    fn opt(&mut self, e: &Option<Entity>) -> String {
        match e {
            Some(e) => self.of(e),
            None => "None".to_string(),
        }
    }
}

fn render(ids: &mut Ids, w: &DomWrite) -> String {
    match w {
        DomWrite::SpawnRow { entity, parent, tag, text, before } => {
            let (e, b) = (ids.of(entity), ids.opt(before));
            format!("SpawnRow({e}, {parent:?}, {tag}, {text:?}, before={b})")
        }
        DomWrite::Reposition { parent, entity, before } => {
            let (e, b) = (ids.of(entity), ids.opt(before));
            format!("Reposition({parent:?}, {e}, before={b})")
        }
        DomWrite::SpawnElement { entity, parent, tag, class, loom_id, text } => {
            let e = ids.of(entity);
            let p = match parent {
                loom_runtime::WriteParent::Entity(pe) => ids.of(pe),
                loom_runtime::WriteParent::Node(n) => format!("{n:?}"),
            };
            format!("SpawnElement({e}, parent={p}, {tag}, class={class:?}, loom_id={loom_id:?}, text={text:?})")
        }
        DomWrite::RemoveNode { node } => format!("RemoveNode({node:?})"),
        DomWrite::ClearChildren { parent } => format!("ClearChildren({parent:?})"),
        other => format!("{other:?}"),
    }
}

/// **§10 K 디스패치 네 분기를 전부 지나가는 전이 열.** `recorder_common`의
/// `SEQUENCES`는 Append·Full·NoOp만 지나가서 Truncate·Clear가 골든에 안 들어갔다.
const SEQUENCES: &[&[&str]] = &[
    &["a", "b", "c", "d", "e"], // Append
    &["a", "b"],                // Truncate
    &["b", "a"],                // Full (재정렬)
    &[],                        // Clear
    &["x", "y"],                // Full (빈 것에서 재구축)
    &["x", "y"],                // NoOp
];

const INITIAL: &[&str] = &["a", "b", "c", "d"];

#[derive(Clone, Copy)]
enum Keyed {
    Rows,
    OrderKeys,
}

/// `loom-bench`의 `RowShape`와 같은 네 축.
#[derive(Clone, Copy)]
pub struct Shape {
    pub bindings: usize,
    pub inner: usize,
    pub variants: bool,
    pub write_variant: WriteVariant,
}

impl Shape {
    const fn r1() -> Self {
        Shape { bindings: 1, inner: 0, variants: false, write_variant: WriteVariant::Conditional }
    }
}

fn attach_targets(app: &mut LoomApp, list: Entity) {
    let rows: Vec<Entity> = if let Some(r) = app.world.get::<ListRows>(list) {
        r.0.clone()
    } else if let Some(m) = app.world.get::<OrderKeyMembers>(list) {
        m.0.clone()
    } else {
        Vec::new()
    };
    for (i, row) in rows.into_iter().enumerate() {
        if app.world.get::<DomTarget>(row).is_none() {
            app.world.entity_mut(row).insert(DomTarget(NodeHandle(100 + i as u32)));
        }
    }
}

fn record<F: FnOnce(&mut LoomApp)>(shape: Shape, register: F, keyed: Keyed) -> Vec<String> {
    let mut app = LoomApp::new();
    // 하네스와 같은 순서: System 등록 → 축 리소스 → 깊이 축 System.
    register(&mut app);
    app.set_row_bindings(shape.bindings);
    app.set_row_inner(shape.inner);
    app.set_row_variant_mode(shape.variants);
    app.set_write_variant(shape.write_variant);
    if shape.bindings > 1 || shape.inner > 0 {
        app.add_row_depth_systems();
    }
    let initial: Vec<String> = INITIAL.iter().map(|s| s.to_string()).collect();
    let anchor = NodeHandle(0);
    let entity = match keyed {
        Keyed::Rows => app.world.spawn((Signal(initial), ListRows::default(), DomTarget(anchor))).id(),
        Keyed::OrderKeys => {
            app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(anchor))).id()
        }
    };
    let mut log = Vec::new();
    let mut ids = Ids::new();
    for step in 0..=SEQUENCES.len() {
        if step > 0 {
            app.world.get_mut::<Signal<Vec<String>>>(entity).unwrap().0 =
                SEQUENCES[step - 1].iter().map(|s| s.to_string()).collect();
        }
        app.run_schedule_for_test();
        let writes = std::mem::take(&mut app.world.resource_mut::<PendingDomWrites>().0);
        log.push(format!("--- tick {step} ({} writes) ---", writes.len()));
        for w in &writes {
            log.push(render(&mut ids, w));
        }
        attach_targets(&mut app, entity);
    }
    log
}

/// 축 조합 — 골든 절 머리글에 그대로 쓰인다.
fn shapes() -> Vec<(&'static str, Shape)> {
    vec![
        ("R1", Shape::r1()),
        ("R2", Shape { bindings: 8, ..Shape::r1() }),
        ("R3", Shape { variants: true, ..Shape::r1() }),
        ("W8-inner3", Shape { inner: 3, ..Shape::r1() }),
        ("UNCOND", Shape { write_variant: WriteVariant::Unconditional, ..Shape::r1() }),
    ]
}

pub fn full_log() -> String {
    let mut out = Vec::new();
    for (axis, shape) in shapes() {
        for (name, keyed) in [
            ("C", Keyed::Rows),
            ("K", Keyed::Rows),
            ("D", Keyed::Rows),
            ("D-LIS", Keyed::Rows),
            ("E", Keyed::OrderKeys),
            ("E-LIS", Keyed::OrderKeys),
        ] {
            out.push(format!("======== {name} {axis} ========"));
            let log = match name {
                "C" => record(shape, |a| { a.add_instrumented_systems(list_reconcile_c_system::<String>); }, keyed),
                "K" => record(shape, |a| { a.add_instrumented_systems(list_reconcile_k_system::<String>); }, keyed),
                "D" => record(shape, |a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); }, keyed),
                "D-LIS" => record(shape, |a| { a.add_instrumented_systems(list_reconcile_d_lis_system::<String>); }, keyed),
                "E" => record(shape, |a| { a.add_instrumented_systems(list_reconcile_e_system::<String>); }, keyed),
                _ => record(shape, |a| { a.add_instrumented_systems(list_reconcile_e_lis_system::<String>); }, keyed),
            };
            out.extend(log);
        }
    }
    out.join("\n")
}
