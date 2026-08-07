//! `_with::<_, DefaultRow>`가 접미사 없는 판본과 **같은 write를 내는지**.
//!
//! 이 파일이 검사하는 것은 정확히 그 하나다 — 두 등록 경로가 갈라지지 않았다는
//! 것. `DefaultRow` 자체가 리팩터 이전과 같은지는 **여기서 못 잡는다**(양쪽 다
//! `DefaultRow`를 쓰므로 같이 틀린다). 그쪽은 `tests/write_identity.rs`가
//! 리팩터 직전 커밋에서 뽑은 골든으로 검사한다. 음성 대조로 확인된 분업이다:
//! `DefaultRow`를 망가뜨리면 `write_identity`만 실패하고 이 파일은 통과한다.
//!
//! 방법: 같은 시나리오를 두 번 돌리고 매 tick의 `PendingDomWrites`를 가로채
//! 비교한다(DOM 최종 상태가 아니라 **방출된 write 열** 자체를).

use bevy_ecs::entity::Entity;
use loom_runtime::{
    list_reconcile_d_lis_system, list_reconcile_d_lis_system_with, list_reconcile_d_system,
    list_reconcile_d_system_with, list_reconcile_e_lis_system, list_reconcile_e_lis_system_with,
    list_reconcile_e_system, list_reconcile_e_system_with, list_reconcile_k_system,
    list_reconcile_k_system_with, DefaultRow, DomTarget, DomWrite, ListRows, LoomApp, NodeHandle,
    AnyRow, OrderKeyMembers, PendingDomWrites, RowBindings, Signal,
};

/// write 하나를 비교 가능한 문자열로. `Entity`는 두 실행에서 같은 순서로
/// 만들어지므로 그대로 비교해도 되지만, 실패 메시지를 읽을 수 있게 만든다.
fn render(w: &DomWrite) -> String {
    match w {
        DomWrite::SpawnRow { entity, parent, tag, text, before } => {
            format!("SpawnRow({entity:?}, {parent:?}, {tag}, {text:?}, before={before:?})")
        }
        DomWrite::Reposition { parent, entity, before } => {
            format!("Reposition({parent:?}, {entity:?}, before={before:?})")
        }
        DomWrite::RemoveNode { node } => format!("RemoveNode({node:?})"),
        DomWrite::ClearChildren { parent } => format!("ClearChildren({parent:?})"),
        DomWrite::AppendStaticChildren { row, texts } => {
            format!("AppendStaticChildren({row:?}, {texts:?})")
        }
        DomWrite::SetText { target, text } => format!("SetText({target:?}, {text:?})"),
        DomWrite::SetRowBindingTexts { row, texts } => {
            format!("SetRowBindingTexts({row:?}, {texts:?})")
        }
        other => format!("{other:?}"),
    }
}

/// 리스트 하나에 시퀀스를 먹이고, **매 tick 방출된 write 열**을 모아 돌려준다.
/// `app.tick()`을 부르지 않고 스케줄만 돌린 뒤 큐를 직접 비운다 — DOM 백엔드를
/// 끼우면 적용 과정에서 정보가 사라지기 때문(순서·앵커까지 그대로 봐야 한다).
fn record<F>(row_bindings: usize, register: F, keyed: Keyed, marker: bool) -> Vec<String>
where
    F: FnOnce(&mut LoomApp),
{
    let mut app = LoomApp::new();
    app.world.insert_resource(RowBindings(row_bindings));
    register(&mut app);

    let initial: Vec<String> = ["a", "b", "c", "d", "e"].iter().map(|s| s.to_string()).collect();
    let anchor = NodeHandle(0);
    let entity = match keyed {
        Keyed::Rows => app.world.spawn((Signal(initial), ListRows::default(), DomTarget(anchor))).id(),
        Keyed::OrderKeys => app
            .world
            .spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(anchor)))
            .id(),
    };
    // `_with` 쪽은 `With<R::Marker>`로 걸러지므로 마커를 붙여 준다 — 접미사
    // 없는 쪽은 필터가 없으므로 붙이지 않는다(하네스가 spawn하는 모양 그대로).
    if marker {
        app.world.entity_mut(entity).insert(AnyRow);
    }

    let mut log = Vec::new();
    let mut drain = |app: &mut LoomApp, log: &mut Vec<String>| {
        app.run_schedule_for_test();
        let writes = std::mem::take(&mut app.world.resource_mut::<PendingDomWrites>().0);
        log.push(format!("--- tick ({} writes) ---", writes.len()));
        for w in &writes {
            log.push(render(w));
        }
        // 다음 tick의 앵커 해석이 이전 tick의 spawn에 의존하지 않도록,
        // 이 테스트는 DOM을 적용하지 않는다 — 비교 대상은 방출된 계획 자체다.
        attach_targets(app, entity);
    };
    drain(&mut app, &mut log); // 초기 마운트
    for seq in SEQUENCES {
        set_items(&mut app, entity, seq);
        drain(&mut app, &mut log);
    }
    log
}

/// 방출된 row entity들에 가짜 `DomTarget`을 붙인다 — D/E는 `RemoveNode`를 낼 때
/// 사라질 row의 핸들을 미리 읽으므로, 이게 없으면 두 실행 모두에서 제거 write가
/// 통째로 빠져 비교가 무의미해진다.
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

fn set_items(app: &mut LoomApp, entity: Entity, items: &[&str]) {
    app.world.get_mut::<Signal<Vec<String>>>(entity).unwrap().0 =
        items.iter().map(|s| s.to_string()).collect();
}

enum Keyed {
    Rows,
    OrderKeys,
}

/// 앞뒤 삽입, 중간 삭제, swap, 역순, 전면 교체 — 배치 계획이 갈릴 만한 것들.
const SEQUENCES: &[&[&str]] = &[
    &["a", "b", "c", "d", "e", "f"],
    &["z", "a", "b", "c", "d", "e", "f"],
    &["z", "a", "c", "d", "e", "f"],
    &["z", "f", "c", "d", "e", "a"],
    &["f", "e", "d", "c", "a", "z"],
    &["p", "q", "r"],
    &["p", "q", "r"],
];

macro_rules! equivalence {
    ($name:ident, $plain:ident, $with:ident, $keyed:expr, $bindings:expr) => {
        #[test]
        fn $name() {
            let plain = record($bindings, |app| { app.add_systems($plain::<String>); }, $keyed, false);
            let with = record($bindings, |app| { app.add_systems($with::<String, DefaultRow>); }, $keyed, true);
            assert_eq!(
                plain.join("\n"),
                with.join("\n"),
                "`_with::<DefaultRow>`가 접미사 없는 판본과 다른 write를 냈다 — \
                 트랙 L 측정의 유효성 전제가 깨진 것이다"
            );
            assert!(plain.iter().any(|l| l.starts_with("SpawnRow")), "시나리오가 아무것도 안 냈다");
        }
    };
}

equivalence!(k_r1, list_reconcile_k_system, list_reconcile_k_system_with, Keyed::Rows, 1);
equivalence!(k_r2, list_reconcile_k_system, list_reconcile_k_system_with, Keyed::Rows, 8);
equivalence!(d_r1, list_reconcile_d_system, list_reconcile_d_system_with, Keyed::Rows, 1);
equivalence!(d_r2, list_reconcile_d_system, list_reconcile_d_system_with, Keyed::Rows, 8);
equivalence!(d_lis_r1, list_reconcile_d_lis_system, list_reconcile_d_lis_system_with, Keyed::Rows, 1);
equivalence!(e_r1, list_reconcile_e_system, list_reconcile_e_system_with, Keyed::OrderKeys, 1);
equivalence!(e_r2, list_reconcile_e_system, list_reconcile_e_system_with, Keyed::OrderKeys, 8);
equivalence!(
    e_lis_r1,
    list_reconcile_e_lis_system,
    list_reconcile_e_lis_system_with,
    Keyed::OrderKeys,
    1
);
