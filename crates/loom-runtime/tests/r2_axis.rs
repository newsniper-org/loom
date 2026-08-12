//! RQ1 §3.2의 **R2 축(깊은 row = "binding 8개 + 중첩 조건부 1개")이 조항 원문대로
//! 구현됐는지**. 개정 #9-11(2026-08-12)의 "수집 전 이행 확인 의무"가 요구하는 검사다.
//!
//! 조항 원문을 그대로 재느냐가 왜 중요한가: §9의 **D 기각 (iii)·D 채택 양성 조건·
//! E 기각 (i)이 전부 R2 셀에 걸려 있다**. 2026-08-12 이전의 R2는
//! `RowBindingTargets(Vec<NodeHandle>)` — reconcile System이 명령형으로 핸들에 직접
//! 쓰는 구조였고, 그래서 binding당 변경 감지도, entity spawn/despawn 비용도,
//! 중첩 조건부도 재지 않았다. 대조군 K가 재정렬마다 실제로 무엇을 다시 만드는지가
//! 측정에서 빠져 있었다는 뜻이다.
//!
//! **이 파일이 막는 공허성**: "바인딩이 8개 있다"는 옛 구조도 만족한다. 그래서
//! 여기서 단언하는 것은 **"값이 바뀌면 8개가 실제로 패치된다"** 와 **"안 바뀌면
//! 하나도 안 패치된다"**(= `Changed` 게이팅이 진짜로 게이팅한다) 양쪽이다.
//! 후자가 없으면 "패치된다"는 무조건 write로도 참이 된다.

mod support;
use support::MockDom;

use bevy_ecs::entity::Entity;
use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_system, list_reconcile_k_system, CondBlock, DomTarget,
    DomWrite, ListRows, LoomApp, NodeHandle, PendingDomWrites, RowBinding, RowCond, RowOwned,
    Signal,
};

const R2: usize = 8;

/// `row_cond_is_shown`은 마지막 바이트의 **하위 두 번째** 비트를 본다. 아래 아이템은
/// 조건부가 켜진 것과 꺼진 것이 섞이도록 골랐다 — 전부 한쪽이면 토글 검사가 공허해진다.
const ITEMS: &[&str] = &["a0", "a1", "a2", "a3"];

/// **`tick`을 쓴다** — `run_schedule_for_test`는 write 큐를 남길 뿐 적용하지
/// 않으므로 `DomTarget`이 안 붙는다. R2 바인딩은 DOM에 붙어 있어야 patch
/// System이 표적을 가지므로 마운트는 실제 적용 경로를 타야 한다.
fn app_with(register: impl FnOnce(&mut LoomApp), bindings: usize) -> (LoomApp, MockDom, Entity) {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.set_row_bindings(bindings);
    register(&mut app);
    if bindings > 1 {
        app.add_row_depth_systems();
    }
    let initial: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    let list = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id();
    app.tick(&mut dom);
    (app, dom, list)
}

fn rows_of(app: &LoomApp, list: Entity) -> Vec<Entity> {
    app.world.get::<ListRows>(list).map(|r| r.0.clone()).unwrap_or_default()
}

fn binding_entities(app: &LoomApp) -> Vec<Entity> {
    app.world.iter_entities().filter(|e| e.contains::<RowBinding>()).map(|e| e.id()).collect()
}

/// 리스트를 `next`로 바꾸고 한 스텝 돌린 뒤, **그 스텝에 방출된 write 중 R2
/// 바인딩을 표적으로 한 `SetText`의 수**를 돌려준다.
///
/// 스케줄을 먼저 돌려 큐를 들여다보고(`run_schedule_for_test`) 그 다음 `tick`으로
/// 적용한다. `tick`이 스케줄을 한 번 더 돌리지만 `Changed` 표시가 이미 소비됐으므로
/// 새 write는 안 나오고, 남아 있던 큐가 그대로 적용된다.
fn step(app: &mut LoomApp, dom: &mut MockDom, list: Entity, next: Vec<String>) -> usize {
    app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 = next;
    app.run_schedule_for_test();
    let targets: Vec<NodeHandle> = app
        .world
        .iter_entities()
        .filter(|e| e.contains::<RowBinding>())
        .filter_map(|e| e.get::<DomTarget>().map(|t| t.0))
        .collect();
    let n = app
        .world
        .resource::<PendingDomWrites>()
        .0
        .iter()
        .filter(|w| matches!(w, DomWrite::SetText { target, .. } if targets.contains(target)))
        .count();
    app.tick(dom);
    n
}

#[test]
fn r1_has_no_binding_entities_at_all() {
    // 축이 꺼져 있으면 R2 기계장치가 통째로 없어야 한다 — R1 셀의 실행 경로가
    // 한 System도, 한 entity도 달라지지 않았음을 고정한다.
    let (app, _dom, list) = app_with(|a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); }, 1);
    assert_eq!(rows_of(&app, list).len(), ITEMS.len());
    assert_eq!(app.world.iter_entities().filter(|e| e.contains::<RowBinding>()).count(), 0);
    assert_eq!(app.world.iter_entities().filter(|e| e.contains::<RowCond>()).count(), 0);
}

#[test]
fn r2_row_carries_seven_bindings_and_one_nested_conditional() {
    let (app, _dom, list) = app_with(|a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); }, R2);
    let rows = rows_of(&app, list);
    assert_eq!(rows.len(), ITEMS.len());

    for &row in &rows {
        let owned = app.world.get::<RowOwned>(row).expect("RowOwned가 없다").0.clone();
        // binding 8개 = 기본 텍스트 1개(`<li>` 자신) + 자식 7개.
        assert_eq!(owned.len(), R2, "row가 소유한 entity 수가 조항의 binding 수와 안 맞는다");
        let (cond, bindings) = owned.split_last().unwrap();
        assert_eq!(bindings.len(), R2 - 1);
        for &b in bindings {
            assert!(app.world.get::<RowBinding>(b).is_some(), "바인딩이 entity가 아니다");
            assert!(app.world.get::<Signal<String>>(b).is_some(), "바인딩에 Signal이 없다");
            assert!(app.world.get::<DomTarget>(b).is_some(), "바인딩이 DOM에 안 붙었다");
        }
        assert!(app.world.get::<RowCond>(*cond).is_some(), "중첩 조건부가 없다");
        assert!(app.world.get::<CondBlock>(*cond).is_some(), "조건부에 CondBlock이 없다");
    }

    // **공허성 방지**: 조건부 실효값이 전부 같으면 아래 토글 검사가 의미를 잃는다.
    let shown = rows
        .iter()
        .filter_map(|r| app.world.get::<RowOwned>(*r))
        .filter_map(|o| o.0.last().copied())
        .filter(|c| app.world.get::<Signal<bool>>(*c).is_some_and(|s| s.0))
        .count();
    assert!(shown > 0 && shown < rows.len(), "조건부가 값에 반응하지 않는다 (shown={shown})");
}

#[test]
fn c_value_through_actually_patches_every_binding_and_the_conditional() {
    // 후보 C만 슬롯의 아이템이 **제자리에서** 바뀐다 — R2의 값 패치 비용
    // (RQ1이 예측한 C의 O(n×binding))이 실제로 발생하는 유일한 경로다.
    let (mut app, mut dom, list) = app_with(|a| { a.add_instrumented_systems(list_reconcile_c_system::<String>); }, R2);
    let rows = rows_of(&app, list);
    let cond0 = *app.world.get::<RowOwned>(rows[0]).unwrap().0.last().unwrap();
    let before = app.world.get::<Signal<bool>>(cond0).unwrap().0;

    // 아이템 하나만 바꾼다. `row_cond_is_shown`이 실제로 뒤집히는 값을 골라야
    // 조건부 검사가 공허해지지 않는다("a0" 0x30 → "a2" 0x32에서 뒤집힌다).
    let mut flipped: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    flipped[0] = "a2".to_string();
    let patched = step(&mut app, &mut dom, list, flipped);

    assert_eq!(
        patched,
        R2 - 1,
        "슬롯 값이 바뀌었는데 바인딩 7개가 전부 패치되지 않았다 — \
         이것이 §9 C 기각 조항이 겨냥하는 O(n×binding) 비용이다"
    );
    let after = app.world.get::<Signal<bool>>(cond0).unwrap().0;
    assert_ne!(before, after, "중첩 조건부가 값 변경에 반응하지 않았다");
}

#[test]
fn unchanged_rows_patch_nothing_so_the_gating_is_real() {
    // **위 검사가 공허하지 않음을 세우는 짝**: `Changed` 게이팅이 진짜로 게이팅하는가.
    // 무조건 write라면 위 검사는 통과하면서 이 검사가 깨진다.
    let (mut app, mut dom, list) = app_with(|a| { a.add_instrumented_systems(list_reconcile_c_system::<String>); }, R2);
    let same: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    let patched = step(&mut app, &mut dom, list, same);
    assert_eq!(patched, 0, "값이 안 바뀌었는데 바인딩이 패치됐다 — 게이팅이 없다");
}

#[test]
fn keyed_reorder_does_not_touch_bindings_but_k_rebuilds_them_all() {
    // 개정 #9 정정이 사전등록한 공평성 방향의 **기계적 확인**: R2 충실화의 지배
    // 비용은 row당 entity의 생성·소멸이고, 재정렬 셀에서 그 비용을 내는 것은
    // §10 디스패치 규칙 (4)에 따라 전량 재스폰하는 **K**다. D는 retain한다.
    //
    // 결과를 보기 전에 방향을 적어 뒀으므로, 여기서는 그 방향이 코드에서 실제로
    // 성립하는지만 본다(성능 수치가 아니라 구조적 사실).
    let reversed: Vec<String> = ITEMS.iter().rev().map(|s| s.to_string()).collect();

    let (mut d_app, mut d_dom, d_list) =
        app_with(|a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); }, R2);
    let d_before = binding_entities(&d_app);
    step(&mut d_app, &mut d_dom, d_list, reversed.clone());
    let d_after = binding_entities(&d_app);
    assert_eq!(d_before.len(), ITEMS.len() * (R2 - 1));
    assert_eq!(d_after, d_before, "D가 재정렬에서 바인딩 entity를 다시 만들었다 — retain이 아니다");

    let (mut k_app, mut k_dom, k_list) =
        app_with(|a| { a.add_instrumented_systems(list_reconcile_k_system::<String>); }, R2);
    let k_before = binding_entities(&k_app);
    step(&mut k_app, &mut k_dom, k_list, reversed);
    let k_after = binding_entities(&k_app);
    assert_eq!(k_before.len(), ITEMS.len() * (R2 - 1));
    assert_eq!(
        k_after.len(),
        k_before.len(),
        "K의 바인딩 수가 재스폰 후 달라졌다 — 옛 row의 바인딩이 안 지워지면 매 틱 누적된다"
    );
    assert!(
        k_after.iter().all(|e| !k_before.contains(e)),
        "K가 바인딩 entity를 재사용했다 — 전량 재스폰이라면 전부 새 entity여야 한다"
    );
}

// ---------------------------------------------------------------------------
// **W8(§3.1 "중첩 증폭")** — 개정 #9-3. "외부 100 × 내부 100 중첩에서 외부
// head-insert(C의 O(n_out×n_in) 표적)".
//
// §3.4가 등록한 예측: 유리 = D·E, 불리 = C(증폭). 그 예측이 성립하려면 내부
// 리스트 값이 **row의 아이템에서 파생**돼야 한다 — 그래야 슬롯 아이템이 제자리에서
// 바뀌는 C만 n_in개를 다시 쓴다. 여기서 그 구조를 고정한다.
// ---------------------------------------------------------------------------

const INNER: usize = 5;

fn app_with_inner(register: impl FnOnce(&mut LoomApp), inner: usize) -> (LoomApp, MockDom, Entity) {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.set_row_inner(inner);
    register(&mut app);
    app.add_row_depth_systems();
    let initial: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    let list = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id();
    app.tick(&mut dom);
    (app, dom, list)
}

#[test]
fn w8_row_carries_a_real_nested_list() {
    let (app, dom, _list) = app_with_inner(
        |a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); },
        INNER,
    );
    let root = NodeHandle(0);
    for row in dom.child_handles(root) {
        let kids = dom.child_handles(row);
        assert_eq!(kids.len(), 1, "row는 `<ul>` 하나만 직접 자식으로 가져야 한다");
        assert_eq!(
            dom.child_handles(kids[0]).len(),
            INNER,
            "내부 `<ul>` 아래에 항목 {INNER}개가 있어야 한다 — 이게 '중첩'이다"
        );
    }
    assert_eq!(
        app.world.iter_entities().filter(|e| e.contains::<RowBinding>()).count(),
        ITEMS.len() * INNER
    );
}

#[test]
fn w8_c_rewrites_the_whole_inner_list_but_d_does_not() {
    // 이것이 §3.4가 W8에 등록한 비대칭 그 자체다.
    let (mut c_app, mut c_dom, c_list) = app_with_inner(
        |a| { a.add_instrumented_systems(list_reconcile_c_system::<String>); },
        INNER,
    );
    let mut shifted: Vec<String> = ITEMS.iter().map(|s| s.to_string()).collect();
    shifted.insert(0, "zz".to_string()); // 외부 head-insert
    let c_patched = step(&mut c_app, &mut c_dom, c_list, shifted.clone());
    assert_eq!(
        c_patched,
        ITEMS.len() * INNER,
        "C는 head-insert로 전 슬롯의 아이템이 밀리므로 슬롯마다 내부 {INNER}개를 \
         전부 다시 써야 한다 — 조항이 말한 O(n_out×n_in) 증폭"
    );

    let (mut d_app, mut d_dom, d_list) = app_with_inner(
        |a| { a.add_instrumented_systems(list_reconcile_d_system::<String>); },
        INNER,
    );
    let d_patched = step(&mut d_app, &mut d_dom, d_list, shifted);
    assert_eq!(d_patched, 0, "D는 retained row를 안 건드리므로 내부 재기록이 없어야 한다");
}
