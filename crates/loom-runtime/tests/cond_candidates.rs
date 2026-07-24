//! Correctness tests for RQ1 Track T candidates B and F
//! (research-questions/RQ1-dynamic-control-flow-scheduling.md). The single
//! claim under test: B's despawn-based toggle **loses** the conditional
//! subtree's internal state on every off→on cycle; F's CSS-hide toggle
//! **preserves** it. Everything else (DOM presence/visibility, entity
//! lifecycle) is verified in service of that claim.

mod support;
use support::MockDom;

use loom_runtime::{
    cond_reconcile_b_system, cond_reconcile_f_system, CondBlock, DomTarget, LoomApp, Signal,
};

fn setup<M>(system: impl bevy_ecs::prelude::IntoSystemConfigs<M>) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
    setup_with_size(system, 1)
}

fn setup_with_size<M>(
    system: impl bevy_ecs::prelude::IntoSystemConfigs<M>,
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
