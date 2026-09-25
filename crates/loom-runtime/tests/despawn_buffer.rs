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
use loom_runtime::{list_reconcile_k_system, DomTarget, ListRows, LoomApp, RowBinding, Signal};

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
/// **2026-09-25 재작성 — 이전 판의 상한이 공허했다.** 전에는 `tick` 뒤에
/// `removed::<DomTarget>().count()`를 읽고 `peak <= 40 * 2`를 단언했는데, 수리가
/// tick 끝에서 버퍼를 비우므로 그 값은 **항상 0**이다. 즉 `40 * 2`는 관측값과
/// 아무 관계가 없는 숫자였고, 상한을 400으로 적어도 4로 적어도 똑같이 통과했다.
/// 통과하는 이유가 "안 자란다"가 아니라 "읽는 시점에 이미 비었다"였던 것이다.
///
/// 지금은 **누적이 실제로 어떤 규모인지를 먼저 측정한다**: `run_schedule_for_test`는
/// 스케줄만 돌리고 `clear_trackers`를 부르지 않으므로(= 수리 이전 `LoomApp`과 같은
/// 상태) 같은 라운드 수에서 버퍼가 얼마까지 자라는지 관측할 수 있다. 그 값을
/// 기준으로 `tick` 경로의 상한을 세우면, 상한이 관측에 묶인다.
///
/// **왜 R2 축을 켜고 `RowBinding` 제거를 세는가**: R1의 row entity는 컴포넌트가
/// 없다(`spawn_empty` + `SpawnRow` write뿐이고 `DomTarget`은 write가 **적용**될 때
/// 붙는다). 그래서 DOM 적용을 안 하는 `run_schedule_for_test` 경로에서는 R1 row를
/// 지워도 removed 버퍼에 아무것도 안 들어가고, 누수 규모를 관측할 수가 없다.
/// R2 바인딩 자식은 `Signal` + `RowBinding`을 실제로 들고 있으므로 두 경로에서
/// 똑같이 세어진다 — 그리고 v21을 죽인 셀이 바로 R2다.
#[test]
fn candidate_k_reorders_do_not_accumulate_removed_messages() {
    /// 라운드마다 회전을 대입하고 `step`으로 한 스텝 돌린 뒤, 그 시점의
    /// removed 버퍼 최댓값과 **전량 재스폰이 일어난 라운드 수**를 돌려준다.
    fn rounds(
        n: usize,
        step: impl Fn(&mut LoomApp, &mut MockDom),
    ) -> (usize, usize, usize) {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_bindings(8);
        app.add_instrumented_systems(list_reconcile_k_system::<String>);
        app.add_row_depth_systems();
        let items: Vec<String> = (0..40).map(|i| format!("item-{i}")).collect();
        let list = app
            .world
            .spawn((Signal(items.clone()), ListRows::default(), DomTarget(root)))
            .id();
        app.tick(&mut dom);

        let (mut respawns, mut peak) = (0usize, 0usize);
        for round in 0..n {
            let before: Vec<_> = app.world.get::<ListRows>(list).unwrap().0.clone();
            let mut next = items.clone();
            next.rotate_left(round % items.len() + 1);
            app.world.get_mut::<Signal<Vec<String>>>(list).unwrap().0 = next;
            step(&mut app, &mut dom);
            let after: Vec<_> = app.world.get::<ListRows>(list).unwrap().0.clone();
            if !after.is_empty() && after.iter().all(|e| !before.contains(e)) {
                respawns += 1;
            }
            peak = peak.max(app.world.removed::<RowBinding>().count());
        }
        (respawns, peak, n)
    }

    const N: usize = 15;
    /// row 40개 × row당 R2 바인딩 자식 7개(= `row_bindings - 1`).
    const PER_ROUND: usize = 40 * 7;

    // (1) **누적이 실제로 일어난다** — `clear_trackers` 없이 돌리면 버퍼가 라운드
    //     수에 비례해 자란다. 이 관측이 아래 상한을 공허하지 않게 만든다.
    let (respawns_leak, peak_leak, _) = rounds(N, |app, _dom| app.run_schedule_for_test());
    assert_eq!(
        respawns_leak, N,
        "K가 매 재정렬마다 전량 재스폰하지 않았다 — 아래 단언 전부가 공허해진다"
    );
    assert!(
        peak_leak >= PER_ROUND * N,
        "clear_trackers 없이 {N}라운드를 돌렸는데 removed 버퍼가 {peak_leak}건뿐이다 \
         (라운드당 {PER_ROUND}건 × {N}라운드 이상이어야 한다) — 누수 경로가 사라졌다면 \
         이 테스트는 더 이상 수리를 검사하지 않는다"
    );

    // (2) `tick` 경로는 (1)의 규모로 자라지 않는다. 상한을 한 라운드 분량의
    //     두 배로 잡는다 — (1)이 {leak}건인데 여기가 그 근처면 수리가 없는 것이다.
    let (respawns, peak, _) = rounds(N, |app, dom| app.tick(dom));
    assert_eq!(respawns, N, "tick 경로에서 K가 전량 재스폰하지 않았다");
    assert!(
        peak <= PER_ROUND * 2,
        "K 재정렬 {N}라운드 뒤 removed 버퍼 최대 {peak}건 — 라운드당 {PER_ROUND}건이므로 \
         상한은 두 라운드 분량이다. 같은 조건에서 clear_trackers 없이는 {peak_leak}건까지 \
         자랐다. 라운드 수에 비례해 자라면 wasm에서 할당 실패(트랩)로 끝난다"
    );
}
