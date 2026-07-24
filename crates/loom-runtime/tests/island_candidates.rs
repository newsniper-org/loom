//! Native, DOM-free proof of RQ3's lifecycle rules
//! (research-questions/RQ3-island-supervision-interaction.md) as an actual
//! state machine — Q1 (independent activation), Q2 (no adoption —
//! supervision edge count is zero), Q3 (2-phase detach), Q4
//! (promote-once-and-stay + no-restart) — before any DOM/wasm/compiler
//! integration exists, the same way RQ1 Track L/T started as native
//! `list.rs`/`cond.rs` tests.

use loom_runtime::{
    IslandInstance, IslandInstanceHandle, IslandRegistryBackend, IslandState, NativeIslandRegistry, NodeHandle,
};

#[test]
fn two_islands_register_independently_at_dormant() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.register(2, NodeHandle(20));

    assert_eq!(reg.state(1), Some(IslandState::Dormant));
    assert_eq!(reg.state(2), Some(IslandState::Dormant));
    assert_eq!(reg.entry(1).unwrap().host, NodeHandle(10));
    assert_eq!(reg.entry(2).unwrap().host, NodeHandle(20));
}

#[test]
#[should_panic(expected = "registered twice")]
fn registering_the_same_id_twice_panics() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.register(1, NodeHandle(99));
}

#[test]
fn full_lifecycle_dormant_attaching_attached_detaching_detached() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));

    reg.begin_attach(1);
    assert_eq!(reg.state(1), Some(IslandState::Attaching));

    reg.mark_attached(1, IslandInstanceHandle::JsHandle(42));
    assert_eq!(reg.state(1), Some(IslandState::Attached));

    reg.begin_detach(1);
    assert_eq!(
        reg.state(1),
        Some(IslandState::Detaching),
        "an attached island's detach must go through Detaching first (2-phase protocol), not straight to Detached"
    );

    reg.finish_detach(1);
    assert_eq!(reg.state(1), Some(IslandState::Detached));
}

#[test]
fn dormant_island_detaches_directly_with_no_actor_involved() {
    // RQ3: "dormant island이 환경발 제거 시 actor 비경유로 detached 직행" —
    // never clicked, never attached, and its detach must not require (or
    // produce) any Detaching intermediate step.
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));

    reg.begin_detach(1);
    assert_eq!(
        reg.state(1),
        Some(IslandState::Detached),
        "a never-attached (Dormant) island must detach directly, skipping Detaching"
    );
}

#[test]
fn failed_island_detaches_directly_on_cascade_cleanup() {
    // RQ3 Q4: crash absorbs into Failed (no restart); a later cascade
    // cleanup (e.g. the whole page area being torn down) must still be able
    // to reap it, going straight to Detached — same "actor 비경유" reasoning
    // as Dormant, since Failed already tore down its own actor.
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.begin_attach(1);
    reg.mark_attached(1, IslandInstanceHandle::JsHandle(1));
    reg.mark_failed(1);
    assert_eq!(reg.state(1), Some(IslandState::Failed));

    reg.begin_detach(1);
    assert_eq!(reg.state(1), Some(IslandState::Detached));
}

#[test]
fn in_flight_activation_that_gets_detached_discards_its_result() {
    // Lean stub: `attaching -> detaching` ("로드 중 detach 도달, 결과 폐기").
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.begin_attach(1);
    assert_eq!(reg.state(1), Some(IslandState::Attaching));

    reg.begin_detach(1);
    assert_eq!(reg.state(1), Some(IslandState::Detaching));
    reg.finish_detach(1);
    assert_eq!(reg.state(1), Some(IslandState::Detached));
}

#[test]
fn crash_from_attached_and_from_attaching_both_absorb_into_failed() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.begin_attach(1);
    reg.mark_failed(1); // load/deserialize failure
    assert_eq!(reg.state(1), Some(IslandState::Failed));

    reg.register(2, NodeHandle(20));
    reg.begin_attach(2);
    reg.mark_attached(2, IslandInstanceHandle::JsHandle(2));
    reg.mark_failed(2); // crash after a successful attach
    assert_eq!(reg.state(2), Some(IslandState::Failed));
}

#[test]
#[should_panic(expected = "expected Dormant")]
fn attaching_a_non_dormant_island_panics() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.begin_attach(1);
    reg.begin_attach(1); // already Attaching, not Dormant
}

#[test]
#[should_panic(expected = "already Detached")]
fn detaching_an_already_detached_island_panics() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.begin_detach(1); // Dormant -> Detached
    reg.begin_detach(1); // already Detached
}

/// RQ3 Q1/Q2 as executable behavior: "island 경계를 넘는 supervision edge는
/// 0개" — detaching one island must not observe or alter another island's
/// state at all, and there is no API on `IslandRegistryBackend` that could
/// even express a parent/child relationship. This is the "2단계 detach
/// cascade" scenario (two islands whose hosts both happen to be inside a
/// removed DOM subtree) modeled at the Registry level: the *caller*
/// (host glue / DOM-containment check, not tested here) decides which ids
/// are in the cascade and calls `begin_detach` on each independently.
#[test]
fn detaching_one_island_does_not_touch_a_sibling_island_zero_supervision_edges() {
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.register(2, NodeHandle(20));
    reg.begin_attach(1);
    reg.mark_attached(1, IslandInstanceHandle::JsHandle(1));
    reg.begin_attach(2);
    reg.mark_attached(2, IslandInstanceHandle::JsHandle(2));

    reg.begin_detach(1);
    assert_eq!(reg.state(1), Some(IslandState::Detaching));
    assert_eq!(
        reg.state(2),
        Some(IslandState::Attached),
        "island 2 must be completely unaffected by island 1's detach — no supervision edge exists between them"
    );

    // The "cascade" itself is just the caller calling begin_detach on every
    // id it independently determined is in the removed range — both reach
    // Detached, but never because one caused the other.
    reg.begin_detach(2);
    reg.finish_detach(1);
    reg.finish_detach(2);
    assert_eq!(reg.state(1), Some(IslandState::Detached));
    assert_eq!(reg.state(2), Some(IslandState::Detached));
}

/// `IslandInstance::begin_detach` (the synchronous detach notification a
/// live actor receives) must actually be invoked for a `Native` actor
/// handle when the Registry transitions it to `Detaching` — proves the
/// wiring, not just the state label.
#[test]
fn native_actor_receives_begin_detach_when_its_island_detaches() {
    struct RecordingActor {
        detach_called: std::rc::Rc<std::cell::Cell<bool>>,
    }
    impl IslandInstance for RecordingActor {
        fn begin_detach(&mut self) {
            self.detach_called.set(true);
        }
    }

    let flag = std::rc::Rc::new(std::cell::Cell::new(false));
    let mut reg = NativeIslandRegistry::new();
    reg.register(1, NodeHandle(10));
    reg.begin_attach(1);
    reg.mark_attached(
        1,
        IslandInstanceHandle::Native(Box::new(RecordingActor { detach_called: flag.clone() })),
    );

    assert!(!flag.get(), "begin_detach must not fire before detach starts");
    reg.begin_detach(1);
    assert!(flag.get(), "Registry must call the live Native actor's begin_detach on synchronous detach");
}
