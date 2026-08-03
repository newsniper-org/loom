//! Correctness tests for RQ1 Track T candidates B and F
//! (research-questions/RQ1-dynamic-control-flow-scheduling.md). The single
//! claim under test: B's despawn-based toggle **loses** the conditional
//! subtree's internal state on every off→on cycle; F's CSS-hide toggle
//! **preserves** it. Everything else (DOM presence/visibility, entity
//! lifecycle) is verified in service of that claim.

mod support;
use bevy_ecs::system::ScheduleSystem;
use support::MockDom;

use loom_runtime::{
    cond_reconcile_b_system, cond_reconcile_f_system, CondBlock, DomTarget, LoomApp, Signal,
};

fn setup<M>(system: impl bevy_ecs::prelude::IntoScheduleConfigs<ScheduleSystem, M>) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
    setup_with_size(system, 1)
}

fn setup_with_size<M>(
    system: impl bevy_ecs::prelude::IntoScheduleConfigs<ScheduleSystem, M>,
    size: usize,
) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
    let (dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(system);
    let entity = app
        .world
        .spawn((Signal(false), CondBlock::with_size(size), DomTarget(root)))
        .id();
    (dom, app, entity, root)
}

fn set_cond(app: &mut LoomApp, e: bevy_ecs::entity::Entity, value: bool) {
    app.world.get_mut::<Signal<bool>>(e).unwrap().0 = value;
}

fn children_of(app: &LoomApp, e: bevy_ecs::entity::Entity) -> Vec<bevy_ecs::entity::Entity> {
    app.world.get::<CondBlock>(e).unwrap().children.clone()
}

/// Convenience for the size=1 tests, where "the child" is unambiguous.
fn child_of(app: &LoomApp, e: bevy_ecs::entity::Entity) -> Option<bevy_ecs::entity::Entity> {
    children_of(app, e).first().copied()
}

mod candidate_b {
    use super::*;

    #[test]
    fn false_by_default_mounts_nothing() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_b_system);
        app.tick(&mut dom); // Signal(false) added -> Changed, but cond is false -> no-op branch
        assert!(dom.child_texts(root).is_empty());
        assert_eq!(child_of(&app, e), None);
    }

    #[test]
    fn toggling_true_spawns_a_fresh_child_with_state() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_b_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["0"]);
        assert!(child_of(&app, e).is_some());
    }

    #[test]
    fn toggling_off_despawns_and_removes_the_dom_node() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_b_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let child = child_of(&app, e).unwrap();

        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        assert!(dom.child_texts(root).is_empty(), "B must remove the DOM node entirely");
        assert!(dom.removed.iter().any(|_| true), "a RemoveNode write must have fired");
        assert_eq!(child_of(&app, e), None);
        // The despawned entity's Signal<i32> must genuinely be gone.
        assert!(app.world.get::<Signal<i32>>(child).is_none());
    }

    #[test]
    fn toggle_off_then_on_loses_internal_state() {
        // The whole point of B vs F: mutate the child's internal counter
        // while visible, hide it, show it again — B must NOT remember 3.
        let (mut dom, mut app, e, _root) = setup(cond_reconcile_b_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let child = child_of(&app, e).unwrap();
        app.world.get_mut::<Signal<i32>>(child).unwrap().0 = 3;
        app.tick(&mut dom); // patch_text_system isn't registered here, but the mutation itself is what matters

        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);

        let new_child = child_of(&app, e).unwrap();
        assert_ne!(child, new_child, "B must mint a brand-new entity on re-show");
        assert_eq!(
            app.world.get::<Signal<i32>>(new_child).unwrap().0,
            0,
            "B must reset state to its initial value — this is the documented cost of despawn+respawn"
        );
    }

    #[test]
    fn repeated_toggling_never_leaves_orphan_dom_nodes() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_b_system);
        app.tick(&mut dom);
        for _ in 0..5 {
            set_cond(&mut app, e, true);
            app.tick(&mut dom);
            assert_eq!(dom.child_texts(root).len(), 1);
            set_cond(&mut app, e, false);
            app.tick(&mut dom);
            assert_eq!(dom.child_texts(root).len(), 0);
        }
    }
}

mod candidate_f {
    use super::*;

    #[test]
    fn false_by_default_mounts_nothing() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_f_system);
        app.tick(&mut dom);
        assert!(dom.child_texts(root).is_empty());
        assert_eq!(child_of(&app, e), None);
    }

    #[test]
    fn first_true_lazily_spawns_the_child() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_f_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["0"]);
        assert!(child_of(&app, e).is_some());
    }

    #[test]
    fn toggling_off_hides_without_despawning() {
        // F toggles the shared anchor's visibility, not each child's —
        // that's what keeps its DOM-call count independent of subtree size.
        let (mut dom, mut app, e, root) = setup(cond_reconcile_f_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let child = child_of(&app, e).unwrap();
        let handle = dom.child_handles(root)[0];
        assert!(dom.is_visible(root));

        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        // Still present in the DOM (not removed), just hidden via the anchor.
        assert_eq!(dom.child_handles(root), vec![handle]);
        assert!(!dom.is_visible(root), "F must hide the shared anchor, not remove the child");
        assert_eq!(child_of(&app, e), Some(child), "F must not despawn on hide");
        assert!(
            app.world.get::<Signal<i32>>(child).is_some(),
            "F must keep the child entity (and its state) alive while hidden"
        );
    }

    #[test]
    fn toggle_off_then_on_preserves_internal_state() {
        // The mirror image of B's state-loss test: F must remember 3.
        let (mut dom, mut app, e, root) = setup(cond_reconcile_f_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let child = child_of(&app, e).unwrap();
        app.world.get_mut::<Signal<i32>>(child).unwrap().0 = 3;

        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);

        assert_eq!(child_of(&app, e), Some(child), "F must reuse the same entity across a hide/show cycle");
        assert_eq!(
            app.world.get::<Signal<i32>>(child).unwrap().0,
            3,
            "F must preserve internal state across a toggle — this is its entire reason to exist over B"
        );
        assert!(dom.is_visible(root), "re-showing must restore the anchor's visibility");
    }

    #[test]
    fn repeated_toggling_reuses_the_same_node_every_time() {
        let (mut dom, mut app, e, root) = setup(cond_reconcile_f_system);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let handle = dom.child_handles(root)[0];
        let child = child_of(&app, e).unwrap();

        for _ in 0..5 {
            set_cond(&mut app, e, false);
            app.tick(&mut dom);
            set_cond(&mut app, e, true);
            app.tick(&mut dom);
        }
        assert_eq!(dom.child_handles(root), vec![handle], "F must never create a second node");
        assert_eq!(child_of(&app, e), Some(child));
        assert_eq!(dom.created.len(), 1, "exactly one create_element call across all 5 toggle cycles");
    }
}

/// RQ1 Track T's subtree-size axis (added 2026-07-24): does B's toggle cost
/// scale with the number of children in the conditional subtree, while F's
/// stays flat? These tests use `CondBlock::with_size(10)` and assert on
/// DOM-call *counts*, not just correctness — the whole point of the axis.
mod subtree_size {
    use super::*;

    #[test]
    fn b_mounts_exactly_size_children_and_despawns_all_of_them_on_hide() {
        let (mut dom, mut app, e, root) = setup_with_size(cond_reconcile_b_system, 10);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root).len(), 10);
        assert_eq!(children_of(&app, e).len(), 10);

        let removed_before = dom.removed.len();
        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root).len(), 0, "B must remove every child");
        assert_eq!(
            dom.removed.len() - removed_before,
            10,
            "B's teardown cost scales with subtree size — 10 children means 10 RemoveNode writes"
        );
        assert!(children_of(&app, e).is_empty());
    }

    #[test]
    fn f_mounts_exactly_size_children_and_toggles_them_with_one_set_visible_call() {
        let (mut dom, mut app, e, root) = setup_with_size(cond_reconcile_f_system, 10);
        app.tick(&mut dom);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root).len(), 10);
        assert_eq!(children_of(&app, e).len(), 10);

        let writes_before = dom.visibility_writes;
        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        assert_eq!(
            dom.visibility_writes - writes_before,
            1,
            "F's toggle cost must stay O(1) regardless of subtree size — one set_visible \
             call on the shared anchor hides all 10 children for free"
        );
        assert_eq!(dom.child_texts(root).len(), 10, "F must not remove any child on hide");
        assert!(!dom.is_visible(root));

        let writes_before = dom.visibility_writes;
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        assert_eq!(dom.visibility_writes - writes_before, 1, "showing again is also O(1)");
        assert!(dom.is_visible(root));
        assert_eq!(children_of(&app, e).len(), 10, "F must never have despawned any child");
    }

    #[test]
    fn f_toggle_cost_does_not_grow_between_size_1_and_size_50() {
        // The direct comparison the axis exists to make: F's per-toggle
        // DOM-call count is identical whether the subtree has 1 child or 50.
        for size in [1usize, 50] {
            let (mut dom, mut app, e, _root) = setup_with_size(cond_reconcile_f_system, size);
            app.tick(&mut dom);
            set_cond(&mut app, e, true);
            app.tick(&mut dom);

            let writes_before = dom.visibility_writes;
            set_cond(&mut app, e, false);
            app.tick(&mut dom);
            assert_eq!(
                dom.visibility_writes - writes_before,
                1,
                "size={size}: F's hide toggle must cost exactly 1 set_visible call"
            );
        }
    }
}

/// RQ1 §10 "F: 3변형 — 셋 다 구현·측정, marker-vs-value 비용 구조가 전제부터
/// 실측 대상". 세 변형이 **실제로 서로 다르게 동작하는지**를 먼저 못박는다 —
/// 이게 성립하지 않으면 §9의 F 기각 조항을 변형별로 적용하는 것 자체가
/// 무의미해진다(허수아비 arm 방지, §2.4).
mod f_variants {
    use super::*;
    use bevy_ecs::prelude::IntoScheduleConfigs;
    use loom_runtime::{
        bump_version_system, cond_reconcile_f_i_system, cond_reconcile_f_ii_system,
        cond_reconcile_f_iii_system, patch_text_f_i_system, patch_text_f_ii_system,
        patch_text_f_iii_system, BindingVersion, CondEffective, Frozen,
    };

    /// F 계열 공통: 토글-off는 노드를 **파괴하지 않고 분리**한다. B와 갈리는
    /// 지점이자, CSS 은닉과도 갈리는 지점(은닉은 문서에 남는다).
    #[test]
    fn f_i_detaches_instead_of_destroying_and_keeps_child_state() {
        let (mut dom, mut app, e, _root) = setup_with_size(
            (cond_reconcile_f_i_system, patch_text_f_i_system).chain(),
            3,
        );
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let kids = children_of(&app, e);
        assert_eq!(kids.len(), 3);
        // 자식에 상태를 남긴다.
        app.world.get_mut::<Signal<i32>>(kids[0]).unwrap().0 = 42;
        app.tick(&mut dom);

        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        assert_eq!(dom.detach_calls, 1, "off 전이는 분리 1회여야 한다");
        // entity와 내부 상태가 살아 있다 — B가 원리적으로 줄 수 없는 것.
        assert_eq!(app.world.get::<Signal<i32>>(kids[0]).unwrap().0, 42);
        assert_eq!(children_of(&app, e).len(), 3, "분리는 despawn이 아니다");

        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        assert_eq!(dom.attach_calls, 1, "on 전이는 재부착 1회여야 한다");
        assert_eq!(app.world.get::<Signal<i32>>(kids[0]).unwrap().0, 42);
    }

    /// F-i의 정의적 비용: 토글 1회 = 서브트리 전 entity에 값 필터 쓰기
    /// (O(서브트리)). RQ1이 미리 지목한 지점이 실제로 그렇게 동작하는지 확인.
    #[test]
    fn f_i_writes_the_value_filter_to_every_child_on_toggle() {
        let (mut dom, mut app, e, _root) =
            setup_with_size((cond_reconcile_f_i_system, patch_text_f_i_system).chain(), 4);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let kids = children_of(&app, e);
        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        for &k in &kids {
            assert!(!app.world.get::<CondEffective>(k).unwrap().0,
                    "off 후 모든 자식의 CondEffective가 false여야 한다");
        }
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        for &k in &kids {
            assert!(app.world.get::<CondEffective>(k).unwrap().0);
        }
    }

    /// F-iii의 정의적 비용: 토글마다 marker를 붙였다 뗀다(archetype 이동 2회).
    #[test]
    fn f_iii_moves_children_in_and_out_of_the_frozen_archetype() {
        let (mut dom, mut app, e, _root) = setup_with_size(
            (cond_reconcile_f_iii_system, bump_version_system, patch_text_f_iii_system).chain(),
            3,
        );
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let kids = children_of(&app, e);
        for &k in &kids {
            assert!(app.world.get::<Frozen>(k).is_none(), "표시 중엔 Frozen이 없다");
        }
        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        for &k in &kids {
            assert!(app.world.get::<Frozen>(k).is_some(), "분리 중엔 Frozen이 붙는다");
        }
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        for &k in &kids {
            assert!(app.world.get::<Frozen>(k).is_none());
        }
    }

    /// **세 변형이 정말 다르다**: 동결 중 상류가 바뀔 때 각자가 하는 일이
    /// 다르다. F-ii는 계속 쓰고(죽은 작업), F-i/F-iii는 안 쓴다.
    #[test]
    fn variants_differ_in_what_they_do_while_detached() {
        // F-ii: 분리 중에도 DOM 쓰기가 계속 나간다.
        let (mut dom2, mut app2, e2, _r2) =
            setup_with_size((cond_reconcile_f_ii_system, patch_text_f_ii_system).chain(), 2);
        set_cond(&mut app2, e2, true);
        app2.tick(&mut dom2);
        let kids2 = children_of(&app2, e2);
        set_cond(&mut app2, e2, false);
        app2.tick(&mut dom2);
        let before = dom2.set_text_calls;
        app2.world.get_mut::<Signal<i32>>(kids2[0]).unwrap().0 = 7;
        app2.tick(&mut dom2);
        assert!(dom2.set_text_calls > before,
                "F-ii는 분리 중에도 계속 패치한다(그게 이 변형의 정의)");

        // F-i: 값 필터가 막아 DOM 쓰기가 나가지 않는다.
        let (mut dom1, mut app1, e1, _r1) =
            setup_with_size((cond_reconcile_f_i_system, patch_text_f_i_system).chain(), 2);
        set_cond(&mut app1, e1, true);
        app1.tick(&mut dom1);
        let kids1 = children_of(&app1, e1);
        set_cond(&mut app1, e1, false);
        app1.tick(&mut dom1);
        let before1 = dom1.set_text_calls;
        app1.world.get_mut::<Signal<i32>>(kids1[0]).unwrap().0 = 7;
        app1.tick(&mut dom1);
        assert_eq!(dom1.set_text_calls, before1,
                   "F-i는 값 필터로 분리 중 쓰기를 막는다");
    }

    /// **공통 난점의 실증**: 꺼진 동안 스킵된 patch는 `Changed` 틱이 소모되어
    /// 재부착 시 자동 재실행되지 않는다 → catch-up이 없으면 stale DOM이 남는다.
    /// F-i는 재부착 시 전파 1회로, F-iii는 버전 불일치분만으로 이를 처리한다.
    #[test]
    fn f_i_catches_up_after_reattach() {
        let (mut dom, mut app, e, _root) =
            setup_with_size((cond_reconcile_f_i_system, patch_text_f_i_system).chain(), 2);
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let kids = children_of(&app, e);
        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        // 동결 중 상류 변경 — 이 patch는 스킵되고 Changed 틱이 소모된다.
        app.world.get_mut::<Signal<i32>>(kids[0]).unwrap().0 = 99;
        app.tick(&mut dom);
        let before = dom.set_text_calls;
        // 재부착 — catch-up이 없으면 여기서 아무 쓰기도 안 나가야 하지만,
        // F-i는 전파 System 1회로 다시 세우므로 다음 tick에 반영된다.
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        app.tick(&mut dom);
        assert!(dom.set_text_calls > before,
                "재부착 후 스킵됐던 값이 catch-up으로 반영돼야 한다");
    }

    /// F-iii의 버전 카운터가 동결 중 dirty를 실제로 추적하는지.
    #[test]
    fn f_iii_version_counter_tracks_changes_while_frozen() {
        let (mut dom, mut app, e, _root) = setup_with_size(
            (cond_reconcile_f_iii_system, bump_version_system, patch_text_f_iii_system).chain(),
            2,
        );
        set_cond(&mut app, e, true);
        app.tick(&mut dom);
        let kids = children_of(&app, e);
        for &k in &kids {
            app.world.entity_mut(k).insert(BindingVersion::default());
        }
        set_cond(&mut app, e, false);
        app.tick(&mut dom);
        app.world.get_mut::<Signal<i32>>(kids[0]).unwrap().0 = 5;
        app.tick(&mut dom);
        let v = app.world.get::<BindingVersion>(kids[0]).unwrap();
        assert!(v.current > v.applied,
                "동결 중 변경은 current만 올려 불일치를 남긴다 (applied={}, current={})",
                v.applied, v.current);
    }
}

// ============================================================================
// CountingDom 위임 완전성 — 2026-08-03 회귀 테스트
// ============================================================================
//
// `DomOps`의 `detach_to_fragment`/`attach_from_fragment`는 SSR `StringDom`용
// "지원 안 함" 패닉을 **기본 구현**으로 갖는다. 데코레이터가 그 둘을 위임하지
// 않으면 감싸는 순간 F 계열 토글이 전부 트랩하는데, 컴파일은 통과한다 —
// 기본 구현이 있으니 빠뜨려도 타입 오류가 안 난다.
//
// 2026-08-03에 트랙 T를 M2 수집(§4.1)을 위해 `CountingDom`으로 감싸자마자
// F-i 첫 셀에서 그렇게 죽었다. 데코레이터를 통과하는 토글을 여기서 한 번
// 돌려 그 종류의 누락을 잡는다.
#[test]
fn counting_dom_forwards_fragment_ops() {
    use loom_runtime::{cond_reconcile_f_i_system, patch_text_f_i_system, CountingDom, DomOps};
    use bevy_ecs::prelude::IntoScheduleConfigs;

    let (raw, root) = MockDom::new_root();
    let mut dom = CountingDom::new(raw);
    let mut app = LoomApp::new();
    app.add_systems((cond_reconcile_f_i_system, patch_text_f_i_system).chain());
    let entity = app
        .world
        .spawn((Signal(false), CondBlock::with_size(4), DomTarget(root)))
        .id();
    app.tick(&mut dom);

    // 실체화 → 분리 → 재부착. 위임이 빠져 있으면 여기서 패닉한다.
    for state in [true, false, true] {
        app.world.get_mut::<Signal<bool>>(entity).unwrap().0 = state;
        app.tick(&mut dom);
    }

    let c = dom.counts();
    assert!(c.detach_to_fragment >= 1, "detach가 세어지지 않았다 — 위임 누락");
    assert!(c.attach_from_fragment >= 1, "attach가 세어지지 않았다 — 위임 누락");
    assert!(
        DomOps::live_node_count(&dom).is_some(),
        "live_node_count 위임 누락 — 누수 회귀 검사가 무력화된다"
    );
}
