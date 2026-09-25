//! §10 "K: 디스패치 규칙"의 네 분기가 조문대로인지.
//!
//! 2026-09-22까지 `k_body`에는 **(4)번밖에 없었다** — 무조건 전량 despawn+respawn.
//! 그래서 §3.4가 K의 등록된 유리 구간으로 잡아 둔 W5(append-only)와 W9(clear)에서
//! 대조군이 자기 최선형이 아니라 **최악형**으로 측정되고 있었다(원칙 1 위반).

use loom_runtime::{k_dispatch, KDispatch};

#[test]
fn clause_branch_2_identical_is_noop() {
    assert_eq!(k_dispatch(&[1, 2, 3], &[1, 2, 3]), KDispatch::NoOp);
    assert_eq!(k_dispatch::<i32>(&[], &[]), KDispatch::NoOp);
}

#[test]
fn clause_branch_3_pure_append_and_truncate() {
    // W5(append-only)가 타야 하는 분기.
    assert_eq!(k_dispatch(&[1, 2], &[1, 2, 3, 4]), KDispatch::Append(2));
    assert_eq!(k_dispatch::<i32>(&[], &[1]), KDispatch::Append(0));
    // truncate.
    assert_eq!(k_dispatch(&[1, 2, 3, 4], &[1, 2]), KDispatch::Truncate(2));
    // W9(clear)가 타야 하는 분기 — 조문: "전량 삭제는 clear_children".
    assert_eq!(k_dispatch(&[1, 2, 3], &[] as &[i32]), KDispatch::Clear);
}

#[test]
fn clause_branch_4_anything_else_is_full_respawn() {
    // 재정렬(W1/W2/W3) — 위치가 어긋나므로 전량.
    assert_eq!(k_dispatch(&[1, 2, 3], &[3, 2, 1]), KDispatch::Full);
    // head-insert(W4) — 첫 위치부터 어긋난다. append처럼 보이지만 prefix가 아니다.
    assert_eq!(k_dispatch(&[1, 2], &[9, 1, 2]), KDispatch::Full);
    // 값 변경(W7) — 조문이 "값-변경 retain 분기는 두지 않는다"고 못박았다.
    assert_eq!(k_dispatch(&[1, 2, 3], &[1, 9, 3]), KDispatch::Full);
    // 아이템 교체(W10).
    assert_eq!(k_dispatch(&[1, 2], &[7, 8]), KDispatch::Full);
    // 꼬리가 다른 truncate — prefix가 아니므로 전량.
    assert_eq!(k_dispatch(&[1, 2, 3], &[1, 9]), KDispatch::Full);
}

#[test]
fn scan_is_positional_not_matching() {
    // **매칭이면** [2,3]이 [1,2,3]의 부분열이므로 "앞에 1 삽입"으로 보일 것이다.
    // 위치 대응 스캔은 그런 판단을 하지 않는다 — 인덱스 0에서 2 != 1이므로 Full.
    // 이 구별이 §10의 "매칭·검색 없음"과 docs/03 판정례 4의 대상외 지위를 세운다.
    assert_eq!(k_dispatch(&[2, 3], &[1, 2, 3]), KDispatch::Full);
    // 같은 값이 여러 번 나와도 위치로만 본다.
    assert_eq!(k_dispatch(&[1, 1, 2], &[1, 1, 2, 1]), KDispatch::Append(3));
}

// ---------------------------------------------------------------------------
// 분기가 **DOM에 실제로 무엇을 하는가**
//
// 위의 네 테스트는 순수 함수 `k_dispatch`만 본다 — 어느 분기를 고르는지는 고정하지만
// **그 분기가 고른 뒤에 내는 write**는 하나도 단언하지 않는다. 2026-09-24에 그것이
// 실제 구멍이었음을 반사실 주입으로 확인했다: `k_body`의 Truncate arm에서
// `DomWrite::RemoveNode` 방출을 지워도 **전 테스트(220건)가 통과했다**.
//
// 왜 골든이 못 잡았나: `recorder_common.rs`의 `SEQUENCES`는 Append·Full·NoOp만
// 지나간다(초기 5개 → 6개 = Append, 이후 전부 위치 어긋남 = Full, 마지막 두 개가
// 같아서 NoOp). **Truncate와 Clear를 지나가는 전이가 없다.** 한편 Truncate arm을
// `panic!`으로 막아 보니 `batchprobe.rs`의 교대 묶음 테스트 2건이 그 arm을
// **실행은 한다** — 실행되지만 결과를 아무도 보지 않는 전형적인 형태다.
//
// 그래서 아래는 write 열이 아니라 **MockDom의 최종 상태**를 단언한다. 그러면
// "RemoveNode를 냈다"가 아니라 "노드가 문서에서 실제로 빠졌다"가 고정된다.

mod support;
use support::MockDom;

use bevy_ecs::entity::Entity;
use loom_runtime::{list_reconcile_k_system, DomTarget, ListRows, LoomApp, Signal};

fn mount(items: &[&str]) -> (LoomApp, MockDom, loom_runtime::NodeHandle, Entity) {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_instrumented_systems(list_reconcile_k_system::<String>);
    let initial: Vec<String> = items.iter().map(|s| s.to_string()).collect();
    let list = app
        .world
        .spawn((Signal(initial), ListRows::default(), DomTarget(root)))
        .id();
    app.tick(&mut dom);
    (app, dom, root, list)
}

fn step(app: &mut LoomApp, dom: &mut MockDom, list: Entity, next: &[&str]) {
    app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 =
        next.iter().map(|s| s.to_string()).collect();
    app.tick(dom);
}

fn rows(app: &LoomApp, list: Entity) -> Vec<Entity> {
    app.world.get::<ListRows>(list).map(|r| r.0.clone()).unwrap_or_default()
}

/// **§10 (3) 순수 truncate** — 꼬리 노드가 문서에서 빠지고, 남은 앞부분은 건드리지
/// 않는다.
///
/// 공허성 방지: "남은 row entity가 그대로다"를 먼저 단언한다. 그게 없으면 Full
/// 분기(전량 despawn+respawn)로도 "DOM에 2개만 남았다"가 참이 되어, 이 테스트가
/// truncate 분기를 검사하는 것이 아니게 된다.
#[test]
fn clause_branch_3_truncate_removes_the_dropped_nodes_from_the_document() {
    let (mut app, mut dom, root, list) = mount(&["a", "b", "c", "d", "e", "f"]);
    let before = rows(&app, list);
    let dropped: Vec<_> = before[2..]
        .iter()
        .map(|&e| app.world.get::<DomTarget>(e).expect("마운트 뒤 row에 DomTarget").0)
        .collect();
    assert_eq!(dom.child_texts(root).len(), 6, "마운트가 6행을 안 만들었다");

    step(&mut app, &mut dom, list, &["a", "b"]);

    let after = rows(&app, list);
    assert_eq!(
        after,
        before[..2].to_vec(),
        "남은 row entity가 교체됐다 — truncate가 아니라 Full 분기를 탔고, \
         그러면 아래 DOM 단언이 truncate를 검사하지 않는다"
    );
    assert_eq!(dom.child_texts(root), vec!["a", "b"], "부모에 남은 자식이 조항과 다르다");
    for h in dropped {
        assert!(
            !dom.is_in_document(root, h),
            "잘려 나간 row의 노드 {h:?}가 문서에 남아 있다 — entity만 despawn하고 \
             DOM 노드를 안 지우면 노드가 영원히 새고, §7 M2 op 수도 실제보다 작게 잡힌다"
        );
    }
    dom.assert_tree_wellformed();
}

/// **§10 (3) 전량 삭제 → `clear_children`** — 부모가 비고 노드가 남지 않는다.
#[test]
fn clause_branch_3_clear_empties_the_parent() {
    let (mut app, mut dom, root, list) = mount(&["a", "b", "c"]);
    let handles: Vec<_> = rows(&app, list)
        .iter()
        .map(|&e| app.world.get::<DomTarget>(e).expect("마운트 뒤 row에 DomTarget").0)
        .collect();
    assert_eq!(handles.len(), 3, "마운트가 3행을 안 만들었다 — 아래 단언이 공허해진다");

    step(&mut app, &mut dom, list, &[]);

    assert!(rows(&app, list).is_empty(), "clear 뒤에도 ListRows가 남았다");
    assert!(dom.child_texts(root).is_empty(), "clear 뒤에도 부모에 자식이 남았다");
    for h in handles {
        assert!(!dom.is_in_document(root, h), "clear 뒤에도 노드 {h:?}가 문서에 있다");
    }
    dom.assert_tree_wellformed();
}

/// **§10 (3) 순수 append** — 앞부분 노드를 재생성하지 않고 꼬리만 붙인다.
///
/// 이것이 §3.4가 K의 등록된 유리 구간으로 잡아 둔 W5의 실체다. 수리 전(전량
/// despawn+respawn)에는 `before`의 entity가 전부 교체됐다.
#[test]
fn clause_branch_3_append_keeps_the_existing_rows() {
    let (mut app, mut dom, root, list) = mount(&["a", "b"]);
    let before = rows(&app, list);
    let kept: Vec<_> = before
        .iter()
        .map(|&e| app.world.get::<DomTarget>(e).expect("마운트 뒤 row에 DomTarget").0)
        .collect();

    step(&mut app, &mut dom, list, &["a", "b", "c", "d"]);

    let after = rows(&app, list);
    assert_eq!(after[..2].to_vec(), before, "append인데 앞부분 row entity가 교체됐다");
    for h in &kept {
        assert!(dom.is_in_document(root, *h), "append인데 기존 노드 {h:?}가 문서에서 빠졌다");
    }
    assert_eq!(dom.child_texts(root), vec!["a", "b", "c", "d"], "꼬리가 순서대로 안 붙었다");
    dom.assert_tree_wellformed();
}
