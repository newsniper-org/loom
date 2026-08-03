//! Correctness tests for RQ1 Track L candidates C, D, E
//! (research-questions/RQ1-benchmark-protocol.md). Each candidate must get
//! the final DOM *content and order* right across a battery of operations
//! (append, prepend, remove-middle, swap, shuffle, no-op) — and for D/E
//! specifically, must *preserve row identity* (the same `NodeHandle`) for
//! every item that survives a reorder, which is the entire architectural
//! point of implementing them as distinct from candidate K.

mod support;
use support::MockDom;

use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, DomTarget, ListRows, LoomApp,
    OrderKeyMembers, Signal,
};

fn set_items(app: &mut LoomApp, entity: bevy_ecs::entity::Entity, items: &[&str]) {
    app.world.get_mut::<Signal<Vec<String>>>(entity).unwrap().0 =
        items.iter().map(|s| s.to_string()).collect();
}

// ============================================================================
// Candidate C — value-through at fixed slots
// ============================================================================

mod candidate_c {
    use super::*;

    fn setup(initial: &[&str]) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.add_systems(list_reconcile_c_system::<String>);
        let entity = app
            .world
            .spawn((
                Signal(initial.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                ListRows::default(),
                DomTarget(root),
            ))
            .id();
        app.tick(&mut dom);
        (dom, app, entity, root)
    }

    #[test]
    fn mounts_initial_items() {
        let (dom, _app, _e, root) = setup(&["a", "b", "c"]);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
    }

    #[test]
    fn noop_tick_does_not_touch_dom() {
        let (mut dom, mut app, _e, root) = setup(&["a", "b", "c"]);
        let created_before = dom.created.len();
        app.tick(&mut dom); // no change to the Signal since mount
        assert_eq!(dom.created.len(), created_before, "C must not touch DOM when nothing changed");
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
    }

    #[test]
    fn swap_rewrites_values_without_moving_or_recreating_slots() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        let slot_handles_before = dom.child_handles(root);
        set_items(&mut app, e, &["c", "b", "a"]); // swap ends
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["c", "b", "a"]);
        // C's whole point: slots (DOM nodes) never move, only their text
        // does — the same 3 NodeHandles, in the same order, must remain.
        assert_eq!(dom.child_handles(root), slot_handles_before);
        assert_eq!(dom.repositions, 0, "C must never reposition — value flows through fixed slots");
    }

    #[test]
    fn shrink_removes_only_the_tail_slots() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d"]);
        let handles_before = dom.child_handles(root);
        set_items(&mut app, e, &["a", "x"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "x"]);
        // First two slots retained (2nd got a value rewrite b->x), tail two removed.
        assert_eq!(dom.child_handles(root), handles_before[..2]);
        assert_eq!(dom.removed.len(), 2);
    }

    #[test]
    fn grow_appends_new_slots_without_touching_existing_ones() {
        let (mut dom, mut app, e, root) = setup(&["a", "b"]);
        let handles_before = dom.child_handles(root);
        set_items(&mut app, e, &["a", "b", "c", "d"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c", "d"]);
        assert_eq!(&dom.child_handles(root)[..2], handles_before.as_slice());
    }
}

// ============================================================================
// Candidate D — keyed reconcile, controller-Vec order
// ============================================================================

mod candidate_d {
    use super::*;

    fn setup(initial: &[&str]) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.add_systems(list_reconcile_d_system::<String>);
        let entity = app
            .world
            .spawn((
                Signal(initial.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                ListRows::default(),
                DomTarget(root),
            ))
            .id();
        app.tick(&mut dom);
        (dom, app, entity, root)
    }

    #[test]
    fn mounts_initial_items() {
        let (dom, _app, _e, root) = setup(&["a", "b", "c"]);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
    }

    #[test]
    fn noop_tick_is_a_true_noop() {
        let (mut dom, mut app, _e, root) = setup(&["a", "b", "c"]);
        let handles_before = dom.child_handles(root);
        let repositions_before = dom.repositions;
        app.tick(&mut dom);
        assert_eq!(dom.repositions, repositions_before, "no change since last tick must mean zero DOM ops");
        assert_eq!(dom.child_handles(root), handles_before);
    }

    #[test]
    fn swap_preserves_row_identity_and_moves_only_what_changed() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        let handles_before = dom.child_handles(root); // [a, b, c] handles
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["c", "b", "a"]); // swap a<->c, b stays put
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["c", "b", "a"]);
        // Same 3 DOM nodes, no new ones created for a swap of existing keys.
        let handles_after = dom.child_handles(root);
        let mut sorted_before = handles_before.clone();
        let mut sorted_after = handles_after.clone();
        sorted_before.sort_by_key(|h| h.0);
        sorted_after.sort_by_key(|h| h.0);
        assert_eq!(sorted_before, sorted_after, "swap must reuse the exact same 3 DOM nodes");
        let delta = dom.repositions - repositions_before;
        assert!(delta >= 1 && delta <= 2, "swapping the two ends needs 1-2 moves, not a full respawn (got {delta})");
    }

    #[test]
    fn reverse_order_preserves_identity() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        set_items(&mut app, e, &["e", "d", "c", "b", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "d", "c", "b", "a"]);
        assert!(dom.created.len() == 5, "no new nodes should be created by a pure reorder");
    }

    #[test]
    fn append_only_creates_exactly_the_new_row() {
        let (mut dom, mut app, e, root) = setup(&["a", "b"]);
        let created_before = dom.created.len();
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["a", "b", "c"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
        assert_eq!(dom.created.len() - created_before, 1, "append must create exactly 1 new row");
        assert_eq!(dom.repositions, repositions_before, "a pure append needs no repositioning");
    }

    #[test]
    fn head_insert_creates_one_row_and_reuses_the_rest() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        let old_handles = dom.child_handles(root);
        let created_before = dom.created.len();
        set_items(&mut app, e, &["z", "a", "b", "c"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["z", "a", "b", "c"]);
        assert_eq!(dom.created.len() - created_before, 1);
        // a, b, c must still be the same nodes, now shifted right by one.
        assert_eq!(&dom.child_handles(root)[1..], old_handles.as_slice());
    }

    #[test]
    fn remove_middle_element() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d"]);
        set_items(&mut app, e, &["a", "c", "d"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "c", "d"]);
        assert_eq!(dom.removed.len(), 1);
    }

    #[test]
    fn shuffle_10_percent_style_mixed_operation() {
        // append + remove + reorder in the same tick — the kind of
        // combined change RQ1's protocol calls "shuffle 10%".
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        set_items(&mut app, e, &["e", "c", "new", "a"]); // b, d removed; "new" inserted; rest reordered
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "c", "new", "a"]);
        assert_eq!(dom.removed.len(), 2);
    }

    #[test]
    fn duplicate_edit_cycle_stays_consistent_across_many_ticks() {
        // Repeated churn must never desync the controller Vec from the DOM.
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        let sequences: [&[&str]; 5] = [
            &["c", "b", "a"],
            &["a", "b", "c", "d"],
            &["d", "c", "b", "a"],
            &["a", "d"],
            &["x", "a", "y", "d", "z"],
        ];
        for seq in sequences {
            set_items(&mut app, e, seq);
            app.tick(&mut dom);
            assert_eq!(dom.child_texts(root), seq.to_vec());
            assert_eq!(
                app.world.get::<ListRows>(e).unwrap().0.len(),
                seq.len(),
                "controller Vec length must always track the DOM"
            );
        }
    }

    /// Guards the `Chain`-based O(1) adjacency/splice fix in
    /// `place_in_order` (an earlier `Vec`-position-scan version was
    /// algorithmically correct but accidentally O(n²) — see list.rs's doc
    /// comment) at a scale where an off-by-one in the linked-list
    /// bookkeeping would actually show up (full reversal touches every
    /// entity's neighbors; a partial shuffle exercises head/tail-adjacent
    /// splices specifically).
    #[test]
    fn large_scale_full_reversal_and_partial_shuffle_stay_correct() {
        const N: usize = 500;
        let initial: Vec<String> = (0..N).map(|i| format!("item-{i}")).collect();
        let (mut dom, mut app, e, root) = setup(&initial.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(dom.child_texts(root), initial);

        let reversed: Vec<String> = initial.iter().rev().cloned().collect();
        set_items(&mut app, e, &reversed.iter().map(String::as_str).collect::<Vec<_>>());
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), reversed);
        assert_eq!(app.world.get::<ListRows>(e).unwrap().0.len(), N);

        // Partial shuffle from the reversed state: rotate the front 50 and
        // back 50 items into each other's territory, leaving the middle
        // alone — exercises head- and tail-adjacent splices together.
        let mut shuffled = reversed.clone();
        shuffled.rotate_left(50);
        set_items(&mut app, e, &shuffled.iter().map(String::as_str).collect::<Vec<_>>());
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), shuffled);
    }
}

// ============================================================================
// Candidate E — OrderKey per-entity tag, no controller Vec
// ============================================================================

mod candidate_e {
    use super::*;

    fn setup(initial: &[&str]) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.add_systems(list_reconcile_e_system::<String>);
        let entity = app
            .world
            .spawn((
                Signal(initial.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                OrderKeyMembers::default(),
                DomTarget(root),
            ))
            .id();
        app.tick(&mut dom);
        (dom, app, entity, root)
    }

    #[test]
    fn mounts_initial_items_in_order() {
        let (dom, _app, _e, root) = setup(&["a", "b", "c"]);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
    }

    #[test]
    fn noop_tick_writes_no_order_keys_and_touches_no_dom() {
        let (mut dom, mut app, _e, root) = setup(&["a", "b", "c"]);
        let handles_before = dom.child_handles(root);
        let repositions_before = dom.repositions;
        app.tick(&mut dom);
        assert_eq!(dom.repositions, repositions_before, "no change since last tick must mean zero DOM ops");
        assert_eq!(dom.child_handles(root), handles_before);
    }

    #[test]
    fn swap_preserves_identity_like_d_does() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        let handles_before = dom.child_handles(root);
        set_items(&mut app, e, &["c", "b", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["c", "b", "a"]);
        let mut sorted_before = handles_before.clone();
        let mut sorted_after = dom.child_handles(root);
        sorted_before.sort_by_key(|h| h.0);
        sorted_after.sort_by_key(|h| h.0);
        assert_eq!(sorted_before, sorted_after);
    }

    #[test]
    fn membership_set_tracks_length_with_no_ordered_vec() {
        let (mut dom, mut app, e, _root) = setup(&["a", "b"]);
        set_items(&mut app, e, &["a", "b", "c", "d"]);
        app.tick(&mut dom);
        assert_eq!(app.world.get::<OrderKeyMembers>(e).unwrap().0.len(), 4);
    }

    #[test]
    fn append_creates_exactly_one_row() {
        let (mut dom, mut app, e, root) = setup(&["a", "b"]);
        let created_before = dom.created.len();
        set_items(&mut app, e, &["a", "b", "c"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
        assert_eq!(dom.created.len() - created_before, 1);
    }

    #[test]
    fn shuffle_mixed_operation_matches_target_order() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        set_items(&mut app, e, &["e", "c", "new", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "c", "new", "a"]);
    }

    #[test]
    fn dense_retag_only_writes_order_key_for_moved_entities() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        // Move only "a" to the end; b and c keep their relative order/index.
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["b", "c", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["b", "c", "a"]);
        let delta = dom.repositions - repositions_before;
        // b and c are still adjacent in that relative order -> reposition
        // count should reflect moving roughly 1 entity's worth of ops, not
        // a full-list rewrite (loose bound: fewer ops than 2x list length).
        assert!(delta <= 3, "got {delta} repositions for a single-item move");
    }
}

// ============================================================================
// Candidate D-LIS / E-LIS — same correctness contract as D/E, but the whole
// point is a *smaller* move count for redistributive workloads (swap,
// shuffle) where research-questions/RQ1-dynamic-control-flow-scheduling.md's
// Phase 1 measurement showed plain D/E moving nearly every retained row.
// ============================================================================

mod candidate_d_lis {
    use super::*;

    fn setup(initial: &[&str]) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.add_systems(list_reconcile_d_lis_system::<String>);
        let entity = app
            .world
            .spawn((
                Signal(initial.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                ListRows::default(),
                DomTarget(root),
            ))
            .id();
        app.tick(&mut dom);
        (dom, app, entity, root)
    }

    #[test]
    fn mounts_initial_items() {
        let (dom, _app, _e, root) = setup(&["a", "b", "c"]);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
    }

    #[test]
    fn noop_tick_is_a_true_noop() {
        let (mut dom, mut app, _e, root) = setup(&["a", "b", "c"]);
        let handles_before = dom.child_handles(root);
        let repositions_before = dom.repositions;
        app.tick(&mut dom);
        assert_eq!(dom.repositions, repositions_before);
        assert_eq!(dom.child_handles(root), handles_before);
    }

    #[test]
    fn swap_ends_costs_only_two_repositions_not_nearly_the_whole_list() {
        // The whole point of the LIS variant: research-questions/
        // RQ1-dynamic-control-flow-scheduling.md's hand-traced n=5 example
        // showed plain D moving 4 of 5 retained rows for an end-swap; LIS
        // should move only the 2 that actually swapped.
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        let handles_before = dom.child_handles(root);
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["e", "b", "c", "d", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "b", "c", "d", "a"]);
        let delta = dom.repositions - repositions_before;
        assert_eq!(delta, 2, "LIS should move only the 2 swapped entities, got {delta}");
        let mut sorted_before = handles_before.clone();
        let mut sorted_after = dom.child_handles(root);
        sorted_before.sort_by_key(|h| h.0);
        sorted_after.sort_by_key(|h| h.0);
        assert_eq!(sorted_before, sorted_after, "same 5 DOM nodes, none recreated");
    }

    #[test]
    fn full_reversal_still_ends_up_correct() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        set_items(&mut app, e, &["e", "d", "c", "b", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "d", "c", "b", "a"]);
        assert_eq!(dom.created.len(), 5, "no new nodes for a pure reorder");
    }

    #[test]
    fn append_creates_exactly_one_row_and_moves_nothing() {
        let (mut dom, mut app, e, root) = setup(&["a", "b"]);
        let created_before = dom.created.len();
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["a", "b", "c"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
        assert_eq!(dom.created.len() - created_before, 1);
        assert_eq!(dom.repositions, repositions_before);
    }

    #[test]
    fn remove_middle_element() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d"]);
        set_items(&mut app, e, &["a", "c", "d"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["a", "c", "d"]);
        assert_eq!(dom.removed.len(), 1);
    }

    #[test]
    fn shuffle_mixed_operation_matches_target_order() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        set_items(&mut app, e, &["e", "c", "new", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "c", "new", "a"]);
    }

    #[test]
    fn duplicate_edit_cycle_stays_consistent_across_many_ticks() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c"]);
        let sequences: [&[&str]; 5] = [
            &["c", "b", "a"],
            &["a", "b", "c", "d"],
            &["d", "c", "b", "a"],
            &["a", "d"],
            &["x", "a", "y", "d", "z"],
        ];
        for seq in sequences {
            set_items(&mut app, e, seq);
            app.tick(&mut dom);
            assert_eq!(dom.child_texts(root), seq.to_vec());
            assert_eq!(app.world.get::<ListRows>(e).unwrap().0.len(), seq.len());
        }
    }

    #[test]
    fn large_scale_full_reversal_and_partial_shuffle_stay_correct() {
        const N: usize = 500;
        let initial: Vec<String> = (0..N).map(|i| format!("item-{i}")).collect();
        let (mut dom, mut app, e, root) = setup(&initial.iter().map(String::as_str).collect::<Vec<_>>());
        assert_eq!(dom.child_texts(root), initial);

        let reversed: Vec<String> = initial.iter().rev().cloned().collect();
        set_items(&mut app, e, &reversed.iter().map(String::as_str).collect::<Vec<_>>());
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), reversed);

        let mut shuffled = reversed.clone();
        shuffled.rotate_left(50);
        set_items(&mut app, e, &shuffled.iter().map(String::as_str).collect::<Vec<_>>());
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), shuffled);
    }
}

mod candidate_e_lis {
    use super::*;

    fn setup(initial: &[&str]) -> (MockDom, LoomApp, bevy_ecs::entity::Entity, loom_runtime::NodeHandle) {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.add_systems(list_reconcile_e_lis_system::<String>);
        let entity = app
            .world
            .spawn((
                Signal(initial.iter().map(|s| s.to_string()).collect::<Vec<_>>()),
                OrderKeyMembers::default(),
                DomTarget(root),
            ))
            .id();
        app.tick(&mut dom);
        (dom, app, entity, root)
    }

    #[test]
    fn mounts_initial_items_in_order() {
        let (dom, _app, _e, root) = setup(&["a", "b", "c"]);
        assert_eq!(dom.child_texts(root), vec!["a", "b", "c"]);
    }

    #[test]
    fn swap_ends_costs_only_two_repositions() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["e", "b", "c", "d", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "b", "c", "d", "a"]);
        let delta = dom.repositions - repositions_before;
        assert_eq!(delta, 2, "LIS should move only the 2 swapped entities, got {delta}");
    }

    #[test]
    fn membership_set_tracks_length_with_no_ordered_vec() {
        let (mut dom, mut app, e, _root) = setup(&["a", "b"]);
        set_items(&mut app, e, &["a", "b", "c", "d"]);
        app.tick(&mut dom);
        assert_eq!(app.world.get::<OrderKeyMembers>(e).unwrap().0.len(), 4);
    }

    #[test]
    fn shuffle_mixed_operation_matches_target_order() {
        let (mut dom, mut app, e, root) = setup(&["a", "b", "c", "d", "e"]);
        set_items(&mut app, e, &["e", "c", "new", "a"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_texts(root), vec!["e", "c", "new", "a"]);
    }
}

// ============================================================================
// RQ1 row-depth axis (R1/R2) — RowBindings applies uniformly across all six
// candidates (`LoomApp::set_row_bindings`), stressed here against K (fully
// recreates every row's content, so its cost should genuinely scale with
// depth) and D (never touches a retained row's content, so depth shouldn't
// change its behavior at all beyond newly-created rows).
// ============================================================================

mod row_bindings_axis {
    use super::*;
    use loom_runtime::list_reconcile_k_system;

    #[test]
    fn default_row_bindings_is_r1_no_extra_children() {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new(); // no set_row_bindings call -> default R1 (1)
        app.add_systems(list_reconcile_k_system::<String>);
        app.world
            .spawn((Signal(vec!["a".to_string()]), ListRows::default(), DomTarget(root)));
        app.tick(&mut dom);

        let rows = dom.child_handles(root);
        assert_eq!(rows.len(), 1);
        assert_eq!(
            dom.children[&rows[0].0].len(),
            0,
            "R1 (default) must add no extra static children per row"
        );
    }

    #[test]
    fn r2_gives_every_k_row_the_configured_extra_static_children() {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_bindings(8); // RQ1-benchmark-protocol.md §3.2's R2: "binding 8개"
        app.add_systems(list_reconcile_k_system::<String>);
        app.world.spawn((
            Signal(vec!["a".to_string(), "b".to_string()]),
            ListRows::default(),
            DomTarget(root),
        ));
        app.tick(&mut dom);

        let rows = dom.child_handles(root);
        assert_eq!(rows.len(), 2);
        for row in rows {
            assert_eq!(
                dom.children[&row.0].len(),
                7,
                "each row must carry 7 extra static children (8 total bindings, 1 primary)"
            );
        }
    }

    #[test]
    fn r2_extra_children_are_recreated_on_every_k_full_respawn() {
        // K always despawns+respawns everything, so its row-depth cost is
        // paid again on every tick — this is exactly what makes R2 stress
        // K harder than D/E (see DomWrite::AppendStaticChildren's doc
        // comment on what this axis does and doesn't model).
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_bindings(4);
        app.add_systems(list_reconcile_k_system::<String>);
        let e = app
            .world
            .spawn((Signal(vec!["a".to_string()]), ListRows::default(), DomTarget(root)))
            .id();
        app.tick(&mut dom);

        // A second, unrelated change (growing the list) still fully clears
        // and respawns *every* row, including the untouched one — so its
        // extra children get recreated too, not just the new row's.
        set_items(&mut app, e, &["a", "b"]);
        app.tick(&mut dom);
        assert_eq!(dom.child_handles(root).len(), 2);
        for row in dom.child_handles(root) {
            assert_eq!(dom.children[&row.0].len(), 3, "every respawned row must carry all 3 extra children again");
        }
    }

    #[test]
    fn r2_does_not_touch_retained_rows_content_for_candidate_d() {
        // D never revisits a retained row's content — depth only affects
        // genuinely *new* rows, never a pure reorder of existing ones. This
        // is the asymmetry RQ1 §9's D-vs-K rejection gate at R2 needs.
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_bindings(5);
        app.add_systems(list_reconcile_d_system::<String>);
        let e = app
            .world
            .spawn((Signal(vec!["a".to_string(), "b".to_string()]), ListRows::default(), DomTarget(root)))
            .id();
        app.tick(&mut dom);
        for row in dom.child_handles(root) {
            assert_eq!(dom.children[&row.0].len(), 4, "initial rows still get their 4 extra children once");
        }

        // Pure swap of the two existing rows (no new/removed keys) must not
        // re-touch either row's already-created extra children.
        let repositions_before = dom.repositions;
        set_items(&mut app, e, &["b", "a"]);
        app.tick(&mut dom);
        assert!(dom.repositions > repositions_before, "the swap itself must still reposition something");
        for row in dom.child_handles(root) {
            assert_eq!(
                dom.children[&row.0].len(),
                4,
                "a pure reorder must not add/remove any retained row's extra children"
            );
        }
    }
}

/// **2026-07-28 R2 축 수리의 인수 조건**. §9 C 기각 조항(W2×R2)이 겨냥하는
/// 비용은 "슬롯의 아이템이 바뀌면 그 아이템에서 파생된 binding을 **전부**
/// 다시 써야 한다"(O(n×binding))인데, 수리 전 R2 축은 추가 binding을 spawn 시
/// 1회 만드는 **정적 텍스트**로 두어 그 비용이 계측에 아예 없었다 —
/// 그래서 C가 D보다 3~10배 빠르게(임계값과 정반대로) 측정됐다.
///
/// 여기서 못박는 것은 그 **비대칭**이다: 재정렬 시 C는 파생 binding을 다시
/// 쓰고, keyed 후보 D는 retained row를 아예 건드리지 않는다.
mod r2_value_derived_bindings {
    use super::*;
    use bevy_ecs::system::ScheduleSystem;

    fn shuffle_and_count_text_writes<M>(
        system: impl bevy_ecs::prelude::IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> usize {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_bindings(8); // R2
        app.add_systems(system);
        let initial: Vec<String> = (0..4).map(|i| format!("item-{i}")).collect();
        let e = app
            .world
            .spawn((Signal(initial.clone()), ListRows::default(), DomTarget(root)))
            .id();
        app.tick(&mut dom);

        // 재정렬(역순) — 슬롯 기준으로 보면 **모든 슬롯의 아이템이 바뀐다**.
        let reversed: Vec<String> = initial.iter().rev().cloned().collect();
        app.world.get_mut::<Signal<Vec<String>>>(e).unwrap().0 = reversed;
        let before = dom.set_text_calls;
        app.tick(&mut dom);
        dom.set_text_calls - before
    }

    #[test]
    fn c_repatches_derived_bindings_on_reorder_but_d_does_not() {
        let c_writes = shuffle_and_count_text_writes(
            loom_runtime::list_reconcile_c_system::<String>,
        );
        let d_writes = shuffle_and_count_text_writes(
            loom_runtime::list_reconcile_d_system::<String>,
        );

        // C: 슬롯 4개 × (primary 1 + 파생 7) = 32회 근처. 정확한 수보다
        // 중요한 것은 "파생 binding까지 다시 쓴다"는 사실이다.
        assert!(
            c_writes >= 4 * 8,
            "C는 재정렬 시 슬롯마다 파생 binding까지 다시 써야 한다 (관측 {c_writes}회)"
        );
        // D: retained row를 건드리지 않으므로 텍스트 재기록이 없다.
        assert_eq!(
            d_writes, 0,
            "D는 retained row의 아이템이 그대로이므로 재기록이 없어야 한다 (관측 {d_writes}회)"
        );
        assert!(
            c_writes > d_writes,
            "이 비대칭이 §9 C 기각 조항이 겨냥하는 바로 그것이다"
        );
    }
}

/// **R3(§3.2 "이질 브랜치 row") 인수 조건**. §9 C 기각 (ii)는 "W2 × R3
/// 프로파일에서 archetype move 구간이 스크립트 시간의 50% 이상"을 재는데,
/// 그 전제는 **C에서 실제로 archetype move가 일어난다**는 것이다.
/// §10이 전환형을 "슬롯 entity 컴포넌트 교체형"으로 사전 고정했으므로,
/// 여기서 확인하는 것은 그 교체가 C에서는 일어나고 keyed 후보(D)에서는
/// 일어나지 않는다는 비대칭이다(RQ1의 "재정렬마다 archetype move 비용" 예측).
mod r3_heterogeneous_branch {
    use super::*;
    use loom_runtime::{RowVariantA, RowVariantB};

    /// 재정렬 후 브랜치 marker가 몇 개나 바뀌었는지 센다.
    fn branch_flips_after_reorder<M>(
        system: impl bevy_ecs::prelude::IntoScheduleConfigs<bevy_ecs::system::ScheduleSystem, M>,
    ) -> usize {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        app.set_row_variant_mode(true); // R3
        app.add_systems(system);
        // 마지막 바이트의 홀짝이 브랜치를 정하므로 A/B가 섞이도록 고른다.
        let initial: Vec<String> = (0..8).map(|i| format!("item-{i}")).collect();
        let e = app
            .world
            .spawn((Signal(initial.clone()), ListRows::default(), DomTarget(root)))
            .id();
        app.tick(&mut dom);

        let snapshot = |app: &LoomApp| -> Vec<bool> {
            app.world
                .get::<ListRows>(e)
                .unwrap()
                .0
                .iter()
                .map(|&r| app.world.get::<RowVariantA>(r).is_some())
                .collect()
        };
        let before = snapshot(&app);

        let reversed: Vec<String> = initial.iter().rev().cloned().collect();
        app.world.get_mut::<Signal<Vec<String>>>(e).unwrap().0 = reversed;
        app.tick(&mut dom);
        let after = snapshot(&app);

        before.iter().zip(after.iter()).filter(|(a, b)| a != b).count()
    }

    #[test]
    fn c_switches_row_branches_on_reorder_but_d_does_not() {
        let c_flips = branch_flips_after_reorder(loom_runtime::list_reconcile_c_system::<String>);
        let d_flips = branch_flips_after_reorder(loom_runtime::list_reconcile_d_system::<String>);

        assert!(
            c_flips > 0,
            "C는 슬롯의 아이템이 바뀌므로 브랜치 전환(= archetype move)이 일어나야 한다 \
             (관측 {c_flips}건). 이게 0이면 §9 C 기각 (ii)를 잴 대상 자체가 없다."
        );
        assert_eq!(
            d_flips, 0,
            "D는 retained row의 아이템이 그대로이므로 브랜치가 바뀌지 않아야 한다 \
             (관측 {d_flips}건)"
        );
    }

    /// R3를 켜지 않으면 기존 동작이 그대로여야 한다(기본값 회귀 방지).
    #[test]
    fn r3_is_off_by_default_and_adds_no_markers() {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new(); // set_row_variant_mode 호출 없음
        app.add_systems(loom_runtime::list_reconcile_c_system::<String>);
        let e = app
            .world
            .spawn((Signal(vec!["a".to_string()]), ListRows::default(), DomTarget(root)))
            .id();
        app.tick(&mut dom);
        let row = app.world.get::<ListRows>(e).unwrap().0[0];
        assert!(app.world.get::<RowVariantA>(row).is_none());
        assert!(app.world.get::<RowVariantB>(row).is_none());
    }
}

// ============================================================================
// 노드 슬롯 수명 — 2026-08-03 회귀 테스트
// ============================================================================
//
// **왜 이 테스트가 있는가**: `web::BrowserDom`이 `create_element`로 만든
// `web_sys::Node`를 `nodes` Vec에 넣기만 하고 **어떤 경로로도 놓아 주지
// 않았다**. `clear_children`/`remove_node`는 문서 트리에서만 뗐고 슬롯이
// JsValue를 붙들고 있어 GC 대상이 되지 못했다. K는 매 tick 전량 재스폰하므로
// n=10,000 조합에서 3,528 tick × 10,000 = **3,500만 개**의 분리된 `<li>`가
// 상주했다(실측 노드당 ~300B → 약 10 GB).
//
// 이 결함은 **규모에서만** 드러난다 — 축소 스모크(약 10 tick)에서는 70만
// 노드라 아무 일도 없었고, 워밍업 50 + 측정 200 도입이 tick 수를 48배로
// 올린 뒤 전면 수집 첫 조합에서 죽었다. 그래서 "많이 돌려도 상주가 안 늘어난다"를
// 계약으로 못박고 여기서 검사한다.
//
// `MockDom`은 실제 백엔드와 **같은 계약**을 구현한다(support/mod.rs 참고).
#[test]
fn repeated_reconcile_does_not_accumulate_node_slots() {
    use loom_runtime::{list_reconcile_k_system, DomOps};

    const N: usize = 50;
    const TICKS: usize = 200;

    let arms: [(&str, fn(&mut LoomApp)); 4] = [
        ("K", |app| { app.add_systems(list_reconcile_k_system::<String>); }),
        ("C", |app| { app.add_systems(list_reconcile_c_system::<String>); }),
        ("D", |app| { app.add_systems(list_reconcile_d_system::<String>); }),
        ("E", |app| { app.add_systems(list_reconcile_e_system::<String>); }),
    ];
    for (label, add) in arms {
        let (mut dom, root) = MockDom::new_root();
        let mut app = LoomApp::new();
        add(&mut app);
        let initial: Vec<String> = (0..N).map(|i| format!("item-{i}")).collect();
        let entity = app
            .world
            .spawn((
                Signal(initial.clone()),
                ListRows::default(),
                OrderKeyMembers::default(),
                DomTarget(root),
            ))
            .id();
        app.tick(&mut dom);
        let after_first = dom.live_node_count().unwrap();

        // 매 tick 순서를 바꾼다(회전) — K는 전량 재스폰, C/D/E는 retain 경로.
        for t in 0..TICKS {
            let mut v = initial.clone();
            v.rotate_left(t % N);
            app.world.get_mut::<Signal<Vec<String>>>(entity).unwrap().0 = v;
            app.tick(&mut dom);
        }
        let after_many = dom.live_node_count().unwrap();

        // 상주 슬롯이 **첫 tick 대비 자라지 않아야** 한다. 여유를 조금 두되
        // (row 구조상 정확히 같지 않을 수 있다) 배수로 자라면 실패다.
        assert!(
            after_many <= after_first * 2,
            "{label}: {TICKS}회 재조정 후 상주 슬롯이 {after_first} → {after_many}로 자랐다 \
             — 노드 슬롯이 해제되지 않고 있다(2026-08-03 BrowserDom 누수와 같은 결함)"
        );
    }
}
