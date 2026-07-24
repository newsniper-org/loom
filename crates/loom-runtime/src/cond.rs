//! RQ1 Track T (conditional rendering) candidates B and F — real,
//! tested implementations of the two conditional-toggle strategies
//! RQ1-dynamic-control-flow-scheduling.md contrasts.
//!
//! Phase 1 scope (documented, not silently narrowed): only the runtime
//! mechanisms are implemented and measured here (loom-runtime +
//! loom-bench), the same way Track L's C/D/E started. loom-compiler has no
//! conditional-JSX authoring surface yet (`{cond && <X/>}` lowering is not
//! implemented) — extending it is separate follow-up work, not attempted
//! in this pass. F here is specifically the **CSS-hide preserve variant**
//! (`display` toggle) — RQ1's fuller F design also describes a
//! DocumentFragment-detach variant and several sleeping-System-gating
//! sub-variants; this Phase 1 cut implements only the simplest one, which
//! also happens to double as RQ1-benchmark-protocol.md's required
//! "CSS 은닉(v-show형)" control arm.
//!
//! `CondBlock.size` (added 2026-07-24, RQ1 Track T subtree-size axis): the
//! conditional's child is generalized from a single node to `size` sibling
//! nodes sharing one wrapper — the block's own `DomTarget` anchor. This is
//! what makes F's O(1)-regardless-of-subtree-size claim testable at all:
//! F toggles `set_visible` on the shared anchor once, not once per child,
//! so its DOM-call count never grows with `size`. B still despawns/removes
//! each child individually (its defining "no shared wrapper trick" cost),
//! so B's DOM-call count grows with `size` — that asymmetry is the whole
//! point of this axis. See RQ1's "Phase 1 트랙 T" section for the readout.

use crate::{DomTarget, DomWrite, PendingDomWrites, Signal};
use bevy_ecs::prelude::*;

/// The conditional block's own bookkeeping: how many sibling children this
/// subtree has when shown (`size`), and which entities are currently
/// mounted (`children`, empty when hidden-and-never-shown or, for B,
/// hidden). POD (`usize` + `Vec<Entity>`), not a closure — CLAUDE.md 금지 4.
#[derive(Component, Debug, Clone)]
pub struct CondBlock {
    pub size: usize,
    pub children: Vec<Entity>,
}

impl Default for CondBlock {
    fn default() -> Self {
        CondBlock { size: 1, children: Vec::new() }
    }
}

impl CondBlock {
    /// Build a block whose subtree has `size` sibling children when shown —
    /// RQ1 Track T's subtree-size axis (1/10/50 in the benchmark).
    pub fn with_size(size: usize) -> Self {
        CondBlock { size, children: Vec::new() }
    }
}

/// RQ1 candidate **B** ("조건부는 Show/Hide 아닌 실제 despawn"): the
/// `size` children — and any state they own, here a `Signal<i32>` per
/// child standing in for "whatever local state a conditional subtree
/// accumulates while visible" — are destroyed on every `false` transition
/// and rebuilt from scratch (value reset to 0 each) on every `true`
/// transition. This is the "toggle loses state" cost RQ1 documents as B's
/// known tradeoff, made concrete and testable rather than just asserted.
/// Each child is despawned and removed individually — no shared wrapper
/// shortcut — so this System's DOM-call count scales with `size`.
pub fn cond_reconcile_b_system(
    mut q: Query<(&Signal<bool>, &mut CondBlock, &DomTarget), Changed<Signal<bool>>>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (cond, mut block, anchor) in &mut q {
        if cond.0 && block.children.is_empty() {
            for _ in 0..block.size {
                let child = commands.spawn(Signal(0_i32)).id();
                pending.0.push(DomWrite::SpawnRow {
                    entity: child,
                    parent: anchor.0,
                    tag: "span",
                    text: "0".to_string(),
                    before: None,
                });
                block.children.push(child);
            }
        } else if !cond.0 && !block.children.is_empty() {
            // Read each handle before despawning — same reason as
            // list.rs's RemoveNode handling: Commands are flushed before
            // `LoomApp::tick`'s write-application loop runs.
            for child in block.children.drain(..) {
                if let Ok(target) = dom_targets.get(child) {
                    pending.0.push(DomWrite::RemoveNode { node: target.0 });
                }
                commands.entity(child).despawn();
            }
        }
        // (true, non-empty) already mounted; (false, empty) never was —
        // both are no-ops.
    }
}

/// RQ1 candidate **F**, CSS-hide preserve variant: all `size` children are
/// spawned **once**, on the first `true` (RQ1's "lazy-spawn-then-hide" —
/// the initial wake-up payload stays identical to B's), and every
/// subsequent toggle only flips `DomOps::set_visible` **on the block's
/// shared anchor** — never per-child. Hiding a parent hides every DOM
/// descendant for free, so this System's DOM-call count per toggle is a
/// constant 1 regardless of `size` — the entities, their `Signal<i32>`s,
/// and their DOM nodes are never touched again after the first mount.
/// This is F's whole argument: state survives a toggle that would have
/// reset it under B, and that toggle stays O(1) as the subtree grows.
pub fn cond_reconcile_f_system(
    mut q: Query<(&Signal<bool>, &mut CondBlock, &DomTarget), Changed<Signal<bool>>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (cond, mut block, anchor) in &mut q {
        if block.children.is_empty() {
            if cond.0 {
                for _ in 0..block.size {
                    let child = commands.spawn(Signal(0_i32)).id();
                    pending.0.push(DomWrite::SpawnRow {
                        entity: child,
                        parent: anchor.0,
                        tag: "span",
                        text: "0".to_string(),
                        before: None,
                    });
                    block.children.push(child);
                }
            }
            // (false, empty) — never shown yet, nothing to hide.
        } else {
            pending.0.push(DomWrite::SetVisible {
                node: anchor.0,
                visible: cond.0,
            });
        }
    }
}
