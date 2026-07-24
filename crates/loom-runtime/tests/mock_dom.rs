//! Native tests against a mock DOM: prove the Changed<T>-gated patch
//! pipeline and the list control-group reconciliation work correctly across
//! *multiple ticks*, with no browser involved, before wiring web_sys in
//! loom-demo. Multiple ticks matter here specifically because the bug this
//! architecture had to avoid (rebuilding QueryState every call, which
//! resets Bevy's per-System change-detection cursor) only shows up across
//! ticks, never on a single call.

mod support;
use support::MockDom;

use loom_runtime::{
    list_reconcile_k_system, patch_input_system, patch_text_system, DomOps, DomTarget, InputValueTarget, ListRows,
    LoomApp, Signal,
};

#[test]
fn counter_patches_only_on_change_no_vdom_diff() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(patch_text_system::<i32>);

    let value_node = dom.create_element(root, "span", Some("value"));
    let count_entity = app
        .world
        .spawn((Signal(0_i32), DomTarget(value_node)))
        .id();

    // First tick: Added counts as Changed in Bevy, so the initial mount
    // must patch to "0".
    app.tick(&mut dom);
    assert_eq!(dom.text[&value_node.0], "0");

    // No mutation happened across this tick -> the patch System must be a
    // true no-op. This is exactly the case a fresh-QueryState-per-call bug
    // gets wrong (it would re-run and "fix" the sentinel back to "0").
    dom.text.insert(value_node.0, "STALE-SENTINEL".into());
    app.tick(&mut dom);
    assert_eq!(
        dom.text[&value_node.0], "STALE-SENTINEL",
        "patch system ran even though nothing changed since last tick — Changed<T> gating is broken"
    );

    // Mutate (equivalent to the `+1` button handler) and verify exactly the
    // bound DOM node gets patched, with no tree comparison anywhere.
    app.world.get_mut::<Signal<i32>>(count_entity).unwrap().0 += 1;
    app.tick(&mut dom);
    assert_eq!(dom.text[&value_node.0], "1");

    // A second tick with no further mutation must again be a no-op.
    dom.text.insert(value_node.0, "STALE-2".into());
    app.tick(&mut dom);
    assert_eq!(dom.text[&value_node.0], "STALE-2");
}

#[test]
fn dynamic_list_updates_across_multiple_ticks_via_candidate_k() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(list_reconcile_k_system::<String>);

    let items_entity = app
        .world
        .spawn((
            Signal(vec!["a".to_string(), "b".to_string()]),
            ListRows::default(),
            DomTarget(root),
        ))
        .id();

    app.tick(&mut dom);
    assert_eq!(dom.child_texts(root), vec!["a", "b"]);
    let rows_after_mount = app.world.get::<ListRows>(items_entity).unwrap().0.clone();
    assert_eq!(rows_after_mount.len(), 2);

    // No change across this tick -> list must not respawn (proves the same
    // persistent-Schedule fix that the counter test proves).
    app.tick(&mut dom);
    let rows_after_noop = app.world.get::<ListRows>(items_entity).unwrap().0.clone();
    assert_eq!(
        rows_after_noop, rows_after_mount,
        "list respawned on a tick with no change to the backing Signal<Vec<T>>"
    );
    assert_eq!(dom.child_texts(root), vec!["a", "b"]);

    // Append + reorder in one write: Candidate K's "despawn all, respawn
    // all" reconciliation (research-questions/RQ1-benchmark-protocol.md
    // control arm) must reflect the new order exactly.
    app.world
        .get_mut::<Signal<Vec<String>>>(items_entity)
        .unwrap()
        .0 = vec!["b".to_string(), "c".to_string(), "a".to_string()];
    app.tick(&mut dom);
    assert_eq!(dom.child_texts(root), vec!["b", "c", "a"]);
    let rows_after_update = app.world.get::<ListRows>(items_entity).unwrap().0.clone();
    assert_eq!(rows_after_update.len(), 3);
    assert!(
        rows_after_update
            .iter()
            .all(|e| !rows_after_mount.contains(e)),
        "K must fully respawn rows, not reuse old row entities"
    );

    // A further no-op tick must not touch anything again.
    app.tick(&mut dom);
    assert_eq!(dom.child_texts(root), vec!["b", "c", "a"]);
    assert_eq!(
        app.world.get::<ListRows>(items_entity).unwrap().0,
        rows_after_update
    );
}

#[test]
fn counter_and_list_coexist_in_the_same_schedule() {
    // Guards against the two Systems' Changed<T> gates cross-contaminating
    // each other (e.g. one System's query accidentally also matching the
    // other's entities).
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems((patch_text_system::<i32>, list_reconcile_k_system::<String>));

    let counter_node = dom.create_element(root, "span", None);
    let counter = app.world.spawn((Signal(0_i32), DomTarget(counter_node))).id();

    let list_anchor = dom.create_element(root, "ul", None);
    app.world.spawn((
        Signal(vec!["x".to_string()]),
        ListRows::default(),
        DomTarget(list_anchor),
    ));

    app.tick(&mut dom);
    assert_eq!(dom.text[&counter_node.0], "0");
    assert_eq!(dom.child_texts(list_anchor), vec!["x"]);

    app.world.get_mut::<Signal<i32>>(counter).unwrap().0 = 42;
    app.tick(&mut dom);
    assert_eq!(dom.text[&counter_node.0], "42");
    // List must not have been touched by the counter's mutation.
    assert_eq!(dom.child_texts(list_anchor), vec!["x"]);
}

#[test]
fn input_value_patches_only_on_change_and_ignores_plain_string_signals() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(patch_input_system);

    let input_node = dom.create_element(root, "input", None);
    dom.set_attribute(input_node, "value", "hello");
    let name_entity = app
        .world
        .spawn((Signal("hello".to_string()), InputValueTarget, DomTarget(input_node)))
        .id();

    // A `Signal<String>` with no `InputValueTarget` marker must be ignored
    // by `patch_input_system` entirely — this is the exact cross-match
    // `InputValueTarget`'s doc comment says the marker exists to prevent
    // (relevant once/if a plain-text `Signal<String>` binding ever exists).
    let plain_node = dom.create_element(root, "span", None);
    app.world.spawn((Signal("not an input".to_string()), DomTarget(plain_node)));

    app.tick(&mut dom); // Added counts as Changed -> first tick writes "hello".
    assert_eq!(dom.input_values[&input_node.0], "hello");
    assert_eq!(dom.input_value_writes, 1, "the plain Signal<String> must not have been patched as an input");

    // No mutation -> true no-op, same Changed<T> gating every other patch
    // System here is tested for.
    app.tick(&mut dom);
    assert_eq!(dom.input_value_writes, 1);

    // Simulates what `apply_action`'s generated `SetString` arm does.
    app.world.get_mut::<Signal<String>>(name_entity).unwrap().0 = "world".to_string();
    app.tick(&mut dom);
    assert_eq!(dom.input_values[&input_node.0], "world");
    assert_eq!(dom.input_value_writes, 2);
}

#[test]
fn static_text_is_never_touched_by_a_patch_tick() {
    let (mut dom, root) = MockDom::new_root();
    let mut app = LoomApp::new();
    dom.append_static_text(root, "label:");
    app.tick(&mut dom); // no systems registered — must not panic or alter anything
    assert_eq!(dom.child_texts(root), vec!["label:"]);
}
