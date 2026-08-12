// 공통 레코더 — 리팩터 전/후 워크트리에서 **같은 소스**로 돌려 write 열을 뽑는다.
use bevy_ecs::entity::Entity;
use loom_runtime::{
    list_reconcile_d_lis_system, list_reconcile_d_system, list_reconcile_e_lis_system,
    list_reconcile_e_system, list_reconcile_k_system, DomTarget, DomWrite, ListRows, LoomApp,
    NodeHandle, OrderKeyMembers, PendingDomWrites, RowBindings, Signal,
};

/// Entity id를 **최초 등장 순서**로 정규화한다. 원시 id는 World가 내부적으로
/// 몇 개를 먼저 할당했는지에 따라 통째로 밀리므로(예: Resource 하나 추가),
/// 그대로 비교하면 구조가 같아도 다르게 보인다. 여기서 보고 싶은 것은
/// **어떤 write가 어떤 순서로, 어느 슬롯을 가리키며 나왔는가**다.
struct Ids(std::collections::HashMap<String, usize>);

impl Ids {
    fn new() -> Self { Ids(std::collections::HashMap::new()) }
    fn of(&mut self, e: &Entity) -> String {
        let raw = format!("{e:?}");
        let n = self.0.len();
        let idx = *self.0.entry(raw).or_insert(n);
        format!("e{idx}")
    }
    fn opt(&mut self, e: &Option<Entity>) -> String {
        match e { Some(e) => self.of(e), None => "None".to_string() }
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
        // 개정 #9-11로 R2가 내는 write. `SpawnRow`와 같은 이유로 entity와
        // parent entity를 둘 다 정규화해야 골든이 id 할당 변화에 안 흔들린다.
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

const SEQUENCES: &[&[&str]] = &[
    &["a", "b", "c", "d", "e", "f"],
    &["z", "a", "b", "c", "d", "e", "f"],
    &["z", "a", "c", "d", "e", "f"],
    &["z", "f", "c", "d", "e", "a"],
    &["f", "e", "d", "c", "a", "z"],
    &["p", "q", "r"],
    &["p", "q", "r"],
];

#[derive(Clone, Copy)]
enum Keyed { Rows, OrderKeys }

fn attach_targets(app: &mut LoomApp, list: Entity) {
    let rows: Vec<Entity> = if let Some(r) = app.world.get::<ListRows>(list) {
        r.0.clone()
    } else if let Some(m) = app.world.get::<OrderKeyMembers>(list) {
        m.0.clone()
    } else { Vec::new() };
    for (i, row) in rows.into_iter().enumerate() {
        if app.world.get::<DomTarget>(row).is_none() {
            app.world.entity_mut(row).insert(DomTarget(NodeHandle(100 + i as u32)));
        }
    }
}

fn record<F: FnOnce(&mut LoomApp)>(row_bindings: usize, register: F, keyed: Keyed) -> Vec<String> {
    let mut app = LoomApp::new();
    app.world.insert_resource(RowBindings(row_bindings));
    register(&mut app);
    let initial: Vec<String> = ["a", "b", "c", "d", "e"].iter().map(|s| s.to_string()).collect();
    let anchor = NodeHandle(0);
    let entity = match keyed {
        Keyed::Rows => app.world.spawn((Signal(initial), ListRows::default(), DomTarget(anchor))).id(),
        Keyed::OrderKeys => app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(anchor))).id(),
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
        for w in &writes { log.push(render(&mut ids, w)); }
        attach_targets(&mut app, entity);
    }
    log
}

pub fn full_log() -> String {
    let mut out = Vec::new();
    for (label, bindings) in [("R1", 1usize), ("R2", 8usize)] {
        out.push(format!("======== K {label} ========"));
        out.extend(record(bindings, |a| { a.add_systems(list_reconcile_k_system::<String>); }, Keyed::Rows));
        out.push(format!("======== D {label} ========"));
        out.extend(record(bindings, |a| { a.add_systems(list_reconcile_d_system::<String>); }, Keyed::Rows));
        out.push(format!("======== D-LIS {label} ========"));
        out.extend(record(bindings, |a| { a.add_systems(list_reconcile_d_lis_system::<String>); }, Keyed::Rows));
        out.push(format!("======== E {label} ========"));
        out.extend(record(bindings, |a| { a.add_systems(list_reconcile_e_system::<String>); }, Keyed::OrderKeys));
        out.push(format!("======== E-LIS {label} ========"));
        out.extend(record(bindings, |a| { a.add_systems(list_reconcile_e_lis_system::<String>); }, Keyed::OrderKeys));
    }
    out.join("\n")
}
