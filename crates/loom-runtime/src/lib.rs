//! loom-runtime — Layer 2 reactive core (docs/02, docs/03).
//!
//! Phase 1 scope (roadmap/PHASES.md): Island/Resumability/backend-agnostic
//! excluded. Single World, client=server same process.
//!
//! ## Scheduling
//! Reuses Bevy ECS's System/Schedule model as-is (CLAUDE.md 2-1 — "이미 잘
//! 풀린 문제는 재발명하지 않음"). Change-detection ticks are tracked *per
//! System*, persisted across calls by Bevy's `Schedule` — building a fresh
//! `QueryState` every tick (an earlier draft of this file did that) silently
//! resets that cursor and makes `Changed<T>` always true. The fix is to
//! register real Systems once and call `schedule.run(&mut world)` every
//! tick, which is exactly the Bevy-idiomatic way, not a workaround.
//!
//! ## PatchTable, deferred to Phase 2
//! docs/02 specifies `DomBinding { patch_op: u16, args }` + a compiler
//! generated integer-opcode dispatch table so that "which behavior" can be
//! represented as *data* on a Component rather than a closure. Its stated
//! reason is resumability (docs/04's LRef): a serialized SSR payload can
//! only reference a chunk+symbol pair, never a captured Rust closure.
//! Phase 1 has no serialization boundary (client = server, same process),
//! so this file uses Rust generics as the static dispatch mechanism instead
//! — `patch_text_system::<T>` is monomorphized per `T` at compile time,
//! which is a zero-cost, closure-free dispatch table in exactly the sense
//! CLAUDE.md 금지 4 cares about (no closure/fn-pointer ever sits in a
//! Component's storage). The integer-opcode `DomBinding` form becomes
//! necessary again once Phase 2 introduces LRef; this is a scope-appropriate
//! simplification, not a silent deviation from docs/02, and is recorded as
//! such in roadmap/PHASES.md's Phase 1 status.

use bevy_ecs::prelude::*;
use bevy_ecs::schedule::Schedule;

pub mod list;
pub use list::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, OrderKey, OrderKeyMembers, OrderedKey,
    RowKey, SlotValue,
};

pub mod cond;
pub use cond::{cond_reconcile_b_system, cond_reconcile_f_system, CondBlock};

pub mod island;
pub use island::{
    IslandId, IslandInstance, IslandInstanceHandle, IslandRegistryBackend, IslandRegistryEntry, IslandState,
    NativeIslandRegistry,
};

#[cfg(feature = "browser")]
pub mod web;

/// docs/03: reactive atom is an ECS Component, not a closure. Bevy's own
/// change detection (tracked on `&mut` access, and on insertion) *is* the
/// "dirty flag" docs/03 describes.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Signal<T: Send + Sync + 'static>(pub T);

/// A stable integer handle into a DOM side table. Components never hold a
/// raw `web_sys::Node`/`JsValue` directly — same POD discipline RQ3 applies
/// to island host handles, and required unconditionally by CLAUDE.md 금지 4.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeHandle(pub u32);

/// The DOM node a binding site's patches target. For a scalar binding this
/// is the element whose *entire* text content is the dynamic value (see
/// `DomOps::set_text_content` doc); Phase 1 doesn't support mixing static
/// and dynamic children within one element (loom-compiler enforces this at
/// lowering time and rejects the mixed shape with a clear error).
#[derive(Component, Debug, Clone, Copy)]
pub struct DomTarget(pub NodeHandle);

/// RQ1 control-group (candidate K) bookkeeping: the row entities a list
/// binding site currently owns. POD (`Vec<Entity>` is plain data, not a
/// closure), so this stays within CLAUDE.md 금지 4's rule.
#[derive(Component, Default, Debug, Clone)]
pub struct ListRows(pub Vec<Entity>);

/// Marks a `Signal<String>` entity as an `<input>`'s two-way value binding,
/// gating `patch_input_system` — not every `Signal<String>` should have its
/// `DomTarget` written to via `set_input_value` (a plain text binding, if
/// `TextBinding` ever grows `String` support, would need `set_text_content`
/// instead). Unit struct, not data — the marker itself carries no state,
/// same POD discipline as every other Component here.
#[derive(Component, Debug, Clone, Copy)]
pub struct InputValueTarget;

/// The narrow, closed DOM operation surface a patch System is allowed to
/// describe. No general tree-diff method exists here by construction.
pub trait DomOps {
    /// Creates a new, empty element as the last child of `parent`. Used both
    /// for static structural markup (created once, never revisited) and as
    /// the target element for a dynamic text binding (whose content is set
    /// afterward by a patch, never by `append_static_text`).
    fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle;
    /// Creates a literal, never-patched text node as the last child of
    /// `parent`. Only for purely static text content.
    fn append_static_text(&mut self, parent: NodeHandle, text: &str);
    /// Sets an element's entire text content in one call (`Node.textContent`
    /// semantics: replaces whatever children it had). This is the one
    /// operation every dynamic patch in Phase 1 performs — no VDOM diff.
    fn set_text_content(&mut self, target: NodeHandle, text: &str);
    /// RQ1 A-보강 fast path: remove every child of `parent` in one call
    /// rather than N individual despawns.
    fn clear_children(&mut self, parent: NodeHandle);
    /// Removes exactly one node (candidates C/D/E's targeted despawn — the
    /// counterpart to K's "clear everything" fast path).
    fn remove_node(&mut self, node: NodeHandle);
    /// Moves an *existing* node to be immediately before `before` (or at the
    /// end of `parent`'s children if `None`), without recreating it — the
    /// one operation candidates D and E use instead of despawn+respawn.
    /// Backends should prefer a state-preserving primitive where available
    /// (e.g. WHATWG `moveBefore`), falling back to `insertBefore` semantics
    /// otherwise (RQ1 candidate E's documented Safari fallback) — that
    /// choice is a DOM-backend detail, not part of D/E's own algorithm.
    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>);
    /// RQ1 Track T candidate F (CSS-hide preserve variant): toggles an
    /// element's visibility without touching its existence, children, or
    /// any internal state — the DOM/backend counterpart of "don't despawn,
    /// just hide" (`style.display`), as opposed to candidate B's despawn.
    fn set_visible(&mut self, node: NodeHandle, visible: bool);
    /// Sets a single attribute on an element — outside the reactive patch
    /// pipeline (no `DomWrite` variant emits this; codegen calls it directly
    /// during `setup()`, once, same as static markup). Phase 2 resumability
    /// codegen uses this to tag SSR output with `data-loom-id` (a binding
    /// site's resume-time lookup key) and `data-loom-lref` (the chunk|symbol|
    /// captures payload) — see loom-compiler/src/codegen.rs. Not used by any
    /// Phase 1 candidate.
    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str);
    /// Resume-only: adopts every existing child of `parent` (SSR'd list rows,
    /// already showing the right content) as tracked nodes, returning each
    /// one's fresh `NodeHandle` (for a `DomTarget`) and current text content
    /// (the row's captured value) in DOM order. List resume recovers row
    /// *values* this way — reading them back out of the DOM that's already
    /// sitting there correctly — rather than serializing every row into the
    /// LRef payload, keeping captures O(1) regardless of list length (see
    /// loom-compiler/src/codegen.rs's `resume()` generation). Not used by any
    /// Phase 1 candidate or patch System.
    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)>;
    /// `<input>` two-way binding's reactive write. **Must** set the DOM
    /// property (`element.value = ...`), not the `value` *attribute* —
    /// unlike every other write in this trait, an `<input>`'s attribute only
    /// seeds its value at parse time; once a user has typed into it, only
    /// the live property reflects what's actually displayed. Backends
    /// should use `set_attribute(node, "value", ...)` instead for an
    /// input's *initial* SSR/creation-time value (see codegen.rs's `setup()`
    /// generation) — this method is for the ongoing `patch_input_system`.
    fn set_input_value(&mut self, node: NodeHandle, value: &str);
    /// Resume-only: reads an `<input>`'s *current* value directly from the
    /// DOM, the same reasoning as `adopt_children` for list rows — a
    /// captured LRef value would reflect the state at SSR time, not
    /// whatever the user has typed since then (impossible to know at SSR
    /// time in the first place), so resume recovers it from the live DOM
    /// instead of ever attempting to serialize it into a capture.
    fn get_input_value(&mut self, node: NodeHandle) -> String;
}

/// A DOM write emitted by a patch System this tick. Systems never touch a
/// `DomOps` implementor directly (which would drag !Send JS handles into
/// the ECS scheduler) — they only push a description of the write into
/// `PendingDomWrites`; `LoomApp::tick` drains it at a single sync point,
/// mirroring docs/06's "Command 큐를 통해 정해진 sync point에서 Component에
/// 반영" but for the World→DOM direction.
///
/// `SpawnRow`/`Reposition`/`RemoveNode` name their DOM anchors by `Entity`,
/// not `NodeHandle` — resolved to a real handle by the driver at the moment
/// each write is applied (`LoomApp::tick`), not when the System emits it.
/// This is required, not incidental: a row spawned earlier in the *same*
/// flush batch has no `NodeHandle` yet when a later write in that batch
/// wants to reference it as a `before` anchor. Processing writes strictly
/// in emission order and resolving anchors just-in-time makes that safe —
/// see candidates D/E in `list.rs`.
pub enum DomWrite {
    SetText {
        target: NodeHandle,
        text: String,
    },
    ClearChildren {
        parent: NodeHandle,
    },
    /// Emitted for each fresh row a list reconcile System spawns via
    /// `Commands`. The entity already exists (Commands pre-allocate the id)
    /// but has no `DomTarget` yet — the driver creates the real element,
    /// sets its text once, inserts it at the right position, and attaches
    /// `DomTarget` to it.
    SpawnRow {
        entity: Entity,
        parent: NodeHandle,
        tag: &'static str,
        text: String,
        /// `None` = append at the end of `parent`'s current children.
        before: Option<Entity>,
    },
    /// Moves an already-mounted row (see `DomOps::reposition`).
    Reposition {
        parent: NodeHandle,
        entity: Entity,
        before: Option<Entity>,
    },
    /// Removes one row's DOM node (candidate C/D/E despawn of a single row,
    /// as opposed to K's whole-parent `ClearChildren`). Carries a
    /// `NodeHandle` directly rather than an `Entity` — unlike `SpawnRow`/
    /// `Reposition`'s anchors, by the time this write is *applied* the
    /// entity has typically already been despawned (via `Commands`, flushed
    /// when the enclosing `Schedule::run` returns, before `LoomApp::tick`'s
    /// post-schedule write-application loop even starts), so it can no
    /// longer be resolved to a handle then — the emitting System must read
    /// its `DomTarget` up front, while the entity still exists.
    RemoveNode { node: NodeHandle },
    /// RQ1 Track T candidate F: show/hide without despawn (see
    /// `DomOps::set_visible`).
    SetVisible { node: NodeHandle, visible: bool },
    /// RQ1 row-depth axis (R1/R2, `RowBindings`): appends `texts.len()` extra
    /// static text children under a row — standing in for "N bindings per
    /// row" (RQ1-benchmark-protocol.md §3.2's R2, "binding 8개"). Named by
    /// `Entity` like `SpawnRow`'s `before`: the row may have been spawned
    /// earlier in the *same* flush batch, so its `NodeHandle` isn't resolved
    /// until this write is actually applied (by then the row's `SpawnRow`
    /// write — always emitted first — has already attached its `DomTarget`).
    /// Models row-*creation* cost scaling with depth (correctly stresses K's
    /// full-recreate-every-tick cost against D/E's move-only cost, which is
    /// what RQ1 §9's D-vs-K rejection gate at R2 needs) — not candidate C's
    /// separate "O(n×binding) retained-slot value-patch" cost, which would
    /// need each binding independently re-compared/rewritten on every tick,
    /// not just created once. Stated plainly: this axis doesn't test that
    /// half of R2's prediction, and RQ1 §9's C adoption conditions don't
    /// need R2 data at all (R1-only), so it isn't required to.
    AppendStaticChildren { row: Entity, texts: Vec<String> },
    /// Compiler-generated conditionals (`{cond() && <X/>}`, loom-compiler's
    /// codegen.rs): appends one static text child under an *already
    /// resolved* parent node. Unlike `AppendStaticChildren`, no
    /// `Entity`-to-`NodeHandle` resolution is needed here — the parent (the
    /// conditional's wrapper element) was created directly via
    /// `DomOps::create_element` before any reconcile System ever ran, so it
    /// already has a real `NodeHandle` by the time this write is emitted.
    AppendStaticText { parent: NodeHandle, text: String },
    /// Compiler-generated `<input>` two-way binding's reactive write (see
    /// `patch_input_system`). Carries an already-resolved `NodeHandle`, same
    /// reason as `AppendStaticText` — an input element is created directly
    /// via `DomOps::create_element` in `setup()`/`resume()`, never as part
    /// of a list/conditional spawn batch that would need `Entity`
    /// resolution.
    SetInputValue { node: NodeHandle, value: String },
}

#[derive(Resource, Default)]
pub struct PendingDomWrites(pub Vec<DomWrite>);

/// RQ1 row-depth axis: how many static text children each list row carries
/// beyond its own primary content (default 1 = R1, "binding 1개"). Read by
/// every Track L candidate's spawn path (`list_reconcile_k_system` here;
/// `list_reconcile_c_system`/`place_in_order`/`place_in_order_lis` in
/// `list.rs`) — see `DomWrite::AppendStaticChildren`'s doc comment for what
/// this axis does and doesn't model.
#[derive(Resource, Clone, Copy)]
pub struct RowBindings(pub usize);

impl Default for RowBindings {
    fn default() -> Self {
        RowBindings(1)
    }
}

pub(crate) fn extra_binding_texts(row_bindings: usize, primary: &str) -> Vec<String> {
    (1..row_bindings).map(|i| format!("{primary}-bind{i}")).collect()
}

/// docs/03 §"반응성 파이프라인": one narrow System per binding-site *shape*
/// (monomorphized per `T`), gated by `Changed<Signal<T>>`, performing
/// exactly one DOM operation. No diff — the changed value is written
/// straight through.
pub fn patch_text_system<T>(
    q: Query<(&Signal<T>, &DomTarget), Changed<Signal<T>>>,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (signal, target) in &q {
        pending.0.push(DomWrite::SetText {
            target: target.0,
            text: signal.0.to_string(),
        });
    }
}

/// The `<input>` two-way binding's reactive write — deliberately **not**
/// generic over `T` like `patch_text_system`, unlike every other patch
/// System here: an input's *value* has exactly one type (`String`) by
/// construction (`DomOps::set_input_value` only makes sense for text), so
/// there's no `T` to monomorphize over. Gated by `InputValueTarget` (not
/// just `Signal<String>`) so a future plain-text `Signal<String>` binding
/// (if `TextBinding` ever grows string support) can't be cross-matched by
/// this System and have its DOM node's `value` property written instead of
/// its `textContent` — see `InputValueTarget`'s doc comment.
pub fn patch_input_system(
    q: Query<(&Signal<String>, &DomTarget), (Changed<Signal<String>>, With<InputValueTarget>)>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (signal, target) in &q {
        pending.0.push(DomWrite::SetInputValue {
            node: target.0,
            value: signal.0.clone(),
        });
    }
}

/// RQ1 candidate **K** ("재정렬=despawn+respawn") — the fair-benchmark
/// control group, deliberately chosen as Phase 1's reference list
/// implementation because it prejudges nothing about RQ1's still-open C/D/E
/// comparison (research-questions/RQ1-benchmark-protocol.md). On any change
/// to the backing `Signal<Vec<T>>`, every row entity is despawned and the
/// current items are respawned fresh, in order — gated by the exact same
/// `Changed<T>` mechanism as scalar signals, so lists follow the same
/// No-VDOM pipeline rather than a separate diff algorithm.
///
/// Phase 1 scope: the row tag is hardcoded to `"li"` (the only row shape
/// loom-compiler's demo lowering produces) rather than parameterized per
/// binding site — see roadmap/PHASES.md Phase 1 status. Rows carry no
/// ongoing `Signal<T>`/patch System of their own: K always fully recreates
/// them with correct content baked in at spawn time, so there is nothing
/// left to patch after that.
pub fn list_reconcile_k_system<T>(
    mut q: Query<(&Signal<Vec<T>>, &mut ListRows, &DomTarget), Changed<Signal<Vec<T>>>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
    row_bindings: Res<RowBindings>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (items, mut rows, anchor) in &mut q {
        for old_row in rows.0.drain(..) {
            commands.entity(old_row).despawn();
        }
        pending.0.push(DomWrite::ClearChildren { parent: anchor.0 });
        for item in items.0.iter() {
            let new_entity = commands.spawn_empty().id();
            rows.0.push(new_entity);
            let text = item.to_string();
            pending.0.push(DomWrite::SpawnRow {
                entity: new_entity,
                parent: anchor.0,
                tag: "li",
                text: text.clone(),
                before: None, // ClearChildren already emptied the parent; append in order.
            });
            let extra = extra_binding_texts(row_bindings.0, &text);
            if !extra.is_empty() {
                pending.0.push(DomWrite::AppendStaticChildren { row: new_entity, texts: extra });
            }
        }
    }
}

/// Owns the persistent `World` + `Schedule` pair. Bevy's change-detection
/// ticks are only correct if the same `Schedule` (and thus the same
/// per-System `QueryState`s) runs every tick — this type exists so callers
/// can't accidentally rebuild the schedule and silently break that.
pub struct LoomApp {
    pub world: World,
    schedule: Schedule,
}

impl Default for LoomApp {
    fn default() -> Self {
        Self::new()
    }
}

impl LoomApp {
    pub fn new() -> Self {
        let mut world = World::new();
        world.init_resource::<PendingDomWrites>();
        world.init_resource::<RowBindings>();
        Self {
            world,
            schedule: Schedule::default(),
        }
    }

    /// RQ1 row-depth axis: set how many static text children each list row
    /// carries beyond its primary content (default 1). Must be called
    /// before any list reconcile System runs for it to take effect on that
    /// tick — matches the "configure once, then run" shape every other
    /// LoomApp setup step already follows (`add_systems`).
    pub fn set_row_bindings(&mut self, count: usize) -> &mut Self {
        self.world.insert_resource(RowBindings(count));
        self
    }

    pub fn add_systems<M>(&mut self, systems: impl IntoSystemConfigs<M>) -> &mut Self {
        self.schedule.add_systems(systems);
        self
    }

    /// Runs one reactive tick: execute all registered patch Systems, then
    /// apply the DOM writes they produced at the single sync point. This is
    /// the whole "No-VDOM" pipeline — no tree is diffed anywhere in it.
    ///
    /// Writes are applied strictly in emission order, and `Entity`-named
    /// anchors (`before` on `SpawnRow`/`Reposition`) are resolved to a real
    /// `NodeHandle` *at the moment each write is applied* — by then, any
    /// earlier write in this same batch that mounted that entity has
    /// already attached its `DomTarget`. See `DomWrite`'s doc comment.
    pub fn tick<D: DomOps>(&mut self, dom: &mut D) {
        self.schedule.run(&mut self.world);
        let writes = std::mem::take(&mut self.world.resource_mut::<PendingDomWrites>().0);
        for write in writes {
            match write {
                DomWrite::SetText { target, text } => dom.set_text_content(target, &text),
                DomWrite::ClearChildren { parent } => dom.clear_children(parent),
                DomWrite::SpawnRow {
                    entity,
                    parent,
                    tag,
                    text,
                    before,
                } => {
                    let handle = dom.create_element(parent, tag, None);
                    dom.set_text_content(handle, &text);
                    if let Some(before_handle) = before.and_then(|e| self.dom_target(e)) {
                        dom.reposition(parent, handle, Some(before_handle));
                    }
                    self.world.entity_mut(entity).insert(DomTarget(handle));
                }
                DomWrite::Reposition {
                    parent,
                    entity,
                    before,
                } => {
                    let Some(handle) = self.dom_target(entity) else {
                        continue;
                    };
                    let before_handle = before.and_then(|e| self.dom_target(e));
                    dom.reposition(parent, handle, before_handle);
                }
                DomWrite::RemoveNode { node } => dom.remove_node(node),
                DomWrite::SetVisible { node, visible } => dom.set_visible(node, visible),
                DomWrite::AppendStaticChildren { row, texts } => {
                    let Some(handle) = self.dom_target(row) else {
                        continue;
                    };
                    for text in &texts {
                        dom.append_static_text(handle, text);
                    }
                }
                DomWrite::AppendStaticText { parent, text } => dom.append_static_text(parent, &text),
                DomWrite::SetInputValue { node, value } => dom.set_input_value(node, &value),
            }
        }
    }

    fn dom_target(&self, entity: Entity) -> Option<NodeHandle> {
        self.world.get::<DomTarget>(entity).map(|t| t.0)
    }
}
