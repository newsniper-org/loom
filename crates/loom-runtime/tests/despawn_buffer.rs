//! **despawn이 쌓는 Bevy 메시지 버퍼를 아무도 비우지 않던 결함** (2026-08-13).
//!
//! v21 트랙 L 수집이 `W5 append × R2` 셀에서 90초 만에 wasm `unreachable` 트랩으로
//! 죽었다. 프로파일링 빌드로 스택을 풀어 보니 정확히 이렇다:
//!
//! ```text
//! EntityWorldMut::despawn_with_caller
//!   → EntityWorldMut::despawn_no_free_with_caller
//!     → Messages<M>::write_with_caller
//!       → RawVec::grow_one
//!         → handle_alloc_error → rust_oom → unreachable
//! ```
//!
//! Bevy는 despawn마다 `RemovedComponents` 메시지를 버퍼에 넣는다. 그 버퍼는
//! **이중 버퍼**라 매 프레임 `World::clear_trackers()`가 스왑·비움을 해야 하는데,
//! 그것을 부르는 것은 Bevy의 `App`이다. `LoomApp`은 `App`을 안 쓰고 맨 `World` +
//! `Schedule`을 직접 돌리므로 **그 호출이 없었다** — despawn한 만큼 버퍼가 영원히
//! 자라고, 결국 할당이 실패한다.
//!
//! 왜 지금까지 안 터졌나: 대부분의 셀은 despawn 수가 작다. 개정 #9-1이 W5를
//! 조항대로("100건 연속 append 시퀀스 = 1반복") 고치면서 한 셀의 despawn이
//! 수십만 건으로 뛰었고, 개정 #9-11의 R2가 row당 entity를 9배로 만들어 그 수를
//! 다시 곱했다. **두 개정이 원래 있던 결함의 임계를 넘긴 것**이지 개정이 만든
//! 결함이 아니다.
//!
//! 이 테스트가 고정하는 것: **tick을 반복해도 removed 버퍼가 누적되지 않는다.**

use bevy_ecs::prelude::Component;
use loom_runtime::{list_reconcile_k_system, DomTarget, ListRows, LoomApp, Signal};

mod support;
use support::MockDom;

#[derive(Component)]
struct Probe;

/// 순수 ECS 수준의 최소 재현 — 매 tick entity를 만들고 지운다.
///
/// **관측 지점이 tick "직전"인 이유**: 수리가 tick 끝에서 버퍼를 비우므로 tick
/// 뒤에 읽으면 언제나 0이고, 그러면 "안 자란다"가 despawn이 없어도 참이 된다.
/// tick 직전에 읽으면 (a) 이번 tick의 despawn이 실제로 버퍼에 들어갔는지(공허성
/// 방지)와 (b) 지난 tick 분이 남아 있는지(누적 여부)를 **동시에** 본다.
#[test]
fn removed_component_buffer_does_not_grow_across_ticks() {
    let (mut dom, _root) = MockDom::new_root();
    let mut app = LoomApp::new();

    for tick in 0..12 {
        let ids: Vec<_> = (0..10).map(|_| app.world.spawn(Probe).id()).collect();
        for id in ids {
            app.world.entity_mut(id).despawn();
        }
        let pending = app.world.removed::<Probe>().count();
        assert!(
            pending > 0,
            "tick {tick}: despawn했는데 removed 버퍼가 비었다 — 아래 상한 단언이 공허해진다"
        );
        assert!(
            pending <= 20,
            "tick {tick}에서 removed 버퍼가 {pending}건 — 이중 버퍼가 안 비워지면 \
             despawn한 만큼 영원히 쌓이고, wasm에서는 할당 실패(트랩)로 끝난다"
        );
        app.tick(&mut dom);
    }
}

/// 실제 후보 경로(K는 매 재정렬마다 전량 despawn한다)에서도 같은지.
///
/// 여기서는 despawn이 **시스템 안에서** 일어나 tick 직전에 못 읽는다. 그래서
/// 공허성은 "row entity가 실제로 교체됐는가"로 따로 세우고, 누적 여부는 tick 뒤
/// 버퍼가 라운드 수에 비례해 자라지 않는지로 본다.
#[test]
fn candidate_k_reorders_do_not_accumulate_removed_messages() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_instrumented_systems(list_reconcile_k_system::<String>);
    let items: Vec<String> = (0..40).map(|i| format!("item-{i}")).collect();
    let list = app
        .world
        .spawn((Signal(items.clone()), ListRows::default(), DomTarget(root)))
        .id();
    app.tick(&mut dom);

    let mut respawns = 0usize;
    let mut peak = 0usize;
    for round in 0..15 {
        let before: Vec<_> = app.world.get::<ListRows>(list).unwrap().0.clone();
        let mut next = items.clone();
        let shift = round % items.len() + 1;
        next.rotate_left(shift);
        app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 = next;
        app.tick(&mut dom);
        let after: Vec<_> = app.world.get::<ListRows>(list).unwrap().0.clone();
        if after.iter().all(|e| !before.contains(e)) {
            respawns += 1;
        }
        peak = peak.max(app.world.removed::<DomTarget>().count());
    }
    assert_eq!(
        respawns, 15,
        "K가 매 재정렬마다 전량 재스폰하지 않았다 — 아래 누적 단언이 공허해진다"
    );
    assert!(
        peak <= 40 * 2,
        "K 재정렬 15라운드 뒤 removed 버퍼 최대 {peak}건 — 라운드 수에 비례해 자라면 \
         누적되는 것이다(40개 row × 15라운드 = 600건 규모가 된다)"
    );
}
