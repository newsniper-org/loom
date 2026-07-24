//! RQ1 Track L (dynamic list) candidates C, D, E — real, tested
//! implementations, alongside candidate K (`list_reconcile_k_system` in
//! `lib.rs`). research-questions/RQ1-benchmark-protocol.md still governs
//! which candidate loom eventually adopts; Phase 1 implements all four so a
//! real "핵심 격자" (core grid) measurement is possible against actual
//! mechanisms rather than descriptions of them — see
//! `crates/loom-demo/src/bin/list_bench.rs` for the measurement harness and
//! `roadmap/PHASES.md`'s Phase 1 status for what's *not* covered (the
//! protocol's full statistical treatment, N-scaling, multi-browser matrix).

use crate::{DomTarget, DomWrite, NodeHandle, PendingDomWrites, Signal};
use bevy_ecs::prelude::*;

// ============================================================================
// Candidate C — Slot-Fixed Value-Flow (entity identity = position)
// ============================================================================

/// Per-slot last-written text, so C can implement the real tradeoff RQ1
/// flags: compare-before-write (paid here as an `O(n)` `PartialEq` pass)
/// instead of an unconditional write every reconcile. `ListRows` (from
/// `lib.rs`) is reused as C's slot-entity list, in position order — for C,
/// "row identity" *is* "position", matching RQ1's description exactly.
#[derive(Component, Debug, Clone, Default)]
pub struct SlotValue(pub String);

/// RQ1 candidate **C** ("고정 슬롯 값-통과"): entity identity = list
/// position, not item identity. Growing/shrinking the list only
/// spawns/despawns the *tail* slots (count-diff, like K); every slot in the
/// common `min(old_len, new_len)` range gets its *value* rewritten in
/// place, so reordering becomes "rewrite values at fixed slots" — no
/// entity/DOM-node ever moves. This is the reason C's opt-in scope (RQ1)
/// exists: row-local state (e.g. focus) is bound to the *slot*, not the
/// item, which is surprising if the caller expects keyed semantics.
pub fn list_reconcile_c_system<T>(
    mut lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), Changed<Signal<Vec<T>>>>,
    mut slots: Query<(&mut SlotValue, &DomTarget)>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (items, mut rows, anchor) in &mut lists {
        let new_len = items.0.len();
        let old_len = rows.0.len();
        let common = new_len.min(old_len);

        // Value-through for slots that survive: compare, write only if the
        // text actually differs (the O(n×|Item|) `PartialEq` cost RQ1
        // flags as C's real, non-free tradeoff).
        for i in 0..common {
            let entity = rows.0[i];
            let new_text = items.0[i].to_string();
            if let Ok((mut slot_value, target)) = slots.get_mut(entity) {
                if slot_value.0 != new_text {
                    slot_value.0 = new_text.clone();
                    pending.0.push(DomWrite::SetText {
                        target: target.0,
                        text: new_text,
                    });
                }
            }
        }

        if new_len > old_len {
            for item in &items.0[old_len..new_len] {
                let text = item.to_string();
                let entity = commands.spawn(SlotValue(text.clone())).id();
                rows.0.push(entity);
                pending.0.push(DomWrite::SpawnRow {
                    entity,
                    parent: anchor.0,
                    tag: "li",
                    text,
                    before: None, // growing the tail always appends.
                });
            }
        } else if new_len < old_len {
            for stale in rows.0.split_off(new_len) {
                // Must read the node handle *before* despawning: Commands
                // are flushed when `Schedule::run` returns, before
                // `LoomApp::tick`'s write-application loop even starts, so
                // resolving it later (by `Entity`) would find nothing.
                if let Ok((_, target)) = slots.get(stale) {
                    pending.0.push(DomWrite::RemoveNode { node: target.0 });
                }
                commands.entity(stale).despawn();
            }
        }
    }
}

// ============================================================================
// Shared matching/reposition logic for candidates D and E
// ============================================================================

/// One target position: either an entity retained from the previous order,
/// or one that needs to be freshly spawned and mounted there.
enum Slot {
    Retained(Entity),
    Fresh { entity: Entity, text: String },
}

struct Matched {
    /// `(new_key_index, entity)` for every entity retained from the old
    /// order.
    retained: Vec<(usize, Entity)>,
    /// Old entities whose key no longer appears anywhere in the new keys.
    removed: Vec<Entity>,
}

/// Key-matches `old_order` against `new_keys` (first-match-wins; Phase 1's
/// demo doesn't exercise duplicate keys — key == display text, see D/E doc
/// comments below).
fn match_keys(old_order: &[(String, Entity)], new_keys: &[String]) -> Matched {
    let mut pool: Vec<(String, Entity)> = old_order.to_vec();
    let mut retained = Vec::with_capacity(new_keys.len());
    for (i, key) in new_keys.iter().enumerate() {
        if let Some(pos) = pool.iter().position(|(k, _)| k == key) {
            let (_, entity) = pool.remove(pos);
            retained.push((i, entity));
        }
    }
    let removed = pool.into_iter().map(|(_, e)| e).collect();
    Matched { retained, removed }
}

/// Emits `SpawnRow`/`Reposition` writes to bring `current` (the DOM-ordered
/// entities *before* this reconcile) into the exact order `target`
/// describes. Retained entities already correctly placed emit **zero**
/// writes — this is D/E's entire advantage over K.
///
/// Processes **right to left** so every `before` anchor names an entity
/// whose *final* position has already been fixed earlier in this same
/// pass (or is `None`, meaning "end of the list"). This matters: anchoring
/// a placement to an entity that itself still needs to move later would be
/// wrong (that neighbor would leave the just-placed entity behind when it
/// moves) — going right to left guarantees every anchor is already settled.
///
/// This is a "matching only" reconciliation (RQ1's noted variant axis: no
/// LIS-based move-count minimization) — every out-of-place retained entity
/// gets its own `Reposition`, always *correct* but not necessarily
/// move-count-*minimal*. Documented, not silently assumed.
///
/// Tracks the working DOM order as a doubly linked list (`Chain`), not a
/// `Vec` — an earlier draft used `Vec::insert`/`position()` here, which
/// looks fine for small lists but makes the whole reconcile **O(n²)**: an
/// "already adjacent?" check needs an entity's current position, and a
/// linear scan for that on every one of n target slots (even the ones that
/// don't move) is exactly what caused RQ1's first n=10,000 core-grid pass
/// to show D/E losing to K by 10-15x (research-questions/data/
/// rq1-phase1-track-l-core-grid-v2-*.json, superseded by the corrected
/// re-run this fix produced). `Chain`'s `next_of`/`unlink`/`insert_before`
/// are all O(1), so adjacency checks and moves no longer cost more than the
/// handful of rows that actually move.
fn place_in_order(parent: NodeHandle, current: &[Entity], target: &[Slot], pending: &mut PendingDomWrites) {
    let mut chain = Chain::from_slice(current);
    let mut next_anchor: Option<Entity> = None;

    for slot in target.iter().rev() {
        let entity = match slot {
            Slot::Fresh { entity, text } => {
                pending.0.push(DomWrite::SpawnRow {
                    entity: *entity,
                    parent,
                    tag: "li",
                    text: text.clone(),
                    before: next_anchor,
                });
                chain.insert_before(*entity, next_anchor);
                *entity
            }
            Slot::Retained(entity) => {
                let already_adjacent = match next_anchor {
                    Some(a) => chain.next_of(*entity) == Some(a),
                    None => chain.is_tail(*entity),
                };
                if !already_adjacent {
                    chain.unlink(*entity);
                    chain.insert_before(*entity, next_anchor);
                    pending.0.push(DomWrite::Reposition {
                        parent,
                        entity: *entity,
                        before: next_anchor,
                    });
                }
                *entity
            }
        };
        next_anchor = Some(entity);
    }
}

/// A doubly linked list over `Entity`, used only to make `place_in_order`'s
/// "what's currently adjacent to X" and "splice X in here" operations O(1)
/// instead of the O(n) `Vec::position`/`insert` they'd otherwise need.
struct Chain {
    head: Option<Entity>,
    tail: Option<Entity>,
    next: std::collections::HashMap<Entity, Entity>,
    prev: std::collections::HashMap<Entity, Entity>,
}

impl Chain {
    fn from_slice(items: &[Entity]) -> Self {
        let mut next = std::collections::HashMap::with_capacity(items.len());
        let mut prev = std::collections::HashMap::with_capacity(items.len());
        for w in items.windows(2) {
            next.insert(w[0], w[1]);
            prev.insert(w[1], w[0]);
        }
        Chain {
            head: items.first().copied(),
            tail: items.last().copied(),
            next,
            prev,
        }
    }

    fn next_of(&self, e: Entity) -> Option<Entity> {
        self.next.get(&e).copied()
    }

    fn is_tail(&self, e: Entity) -> bool {
        self.tail == Some(e)
    }

    /// Unlinks `e` from wherever it currently sits (no-op if absent).
    fn unlink(&mut self, e: Entity) {
        let p = self.prev.remove(&e);
        let n = self.next.remove(&e);
        match (p, n) {
            (Some(p), Some(n)) => {
                self.next.insert(p, n);
                self.prev.insert(n, p);
            }
            (Some(p), None) => {
                self.next.remove(&p);
                self.tail = Some(p);
            }
            (None, Some(n)) => {
                self.prev.remove(&n);
                self.head = Some(n);
            }
            (None, None) => {
                self.head = None;
                self.tail = None;
            }
        }
    }

    /// Inserts `e` immediately before `anchor` (`None` = at the tail).
    fn insert_before(&mut self, e: Entity, anchor: Option<Entity>) {
        match anchor {
            None => {
                if let Some(t) = self.tail {
                    self.next.insert(t, e);
                    self.prev.insert(e, t);
                } else {
                    self.head = Some(e);
                }
                self.tail = Some(e);
            }
            Some(a) => {
                match self.prev.get(&a).copied() {
                    Some(p) => {
                        self.next.insert(p, e);
                        self.prev.insert(e, p);
                    }
                    None => self.head = Some(e),
                }
                self.next.insert(e, a);
                self.prev.insert(a, e);
            }
        }
    }
}

/// Returns the set of indices (into `sequence`) forming *a* longest
/// strictly-increasing subsequence (there can be several of the same
/// length; this returns one valid choice, not necessarily unique).
/// Standard O(n log n) patience-sorting algorithm with predecessor
/// reconstruction — RQ1's noted "LIS 기반 최소 이동" variant needs exactly
/// this, and it's an already-solved problem (CLAUDE.md 2-1), not something
/// to design from scratch.
fn lis_indices(sequence: &[usize]) -> std::collections::HashSet<usize> {
    if sequence.is_empty() {
        return std::collections::HashSet::new();
    }
    // `tails[k]` = index into `sequence` of the smallest possible tail
    // value for an increasing subsequence of length k+1 found so far.
    let mut tails: Vec<usize> = Vec::new();
    let mut predecessor: Vec<Option<usize>> = vec![None; sequence.len()];

    for i in 0..sequence.len() {
        let val = sequence[i];
        let pos = tails.partition_point(|&ti| sequence[ti] < val);
        if pos > 0 {
            predecessor[i] = Some(tails[pos - 1]);
        }
        if pos == tails.len() {
            tails.push(i);
        } else {
            tails[pos] = i;
        }
    }

    let mut result = std::collections::HashSet::new();
    let mut cursor = tails.last().copied();
    while let Some(idx) = cursor {
        result.insert(idx);
        cursor = predecessor[idx];
    }
    result
}

/// The **LIS-optimized** variant of `place_in_order` — RQ1's other noted
/// variant-axis choice ("이동 최소화: 순열 매칭만 vs LIS"). Retained entities
/// whose *old* positions form a longest increasing subsequence (relative to
/// their new-order positions) are provably already in the correct relative
/// order and need **zero** `Reposition` calls; only entities outside that
/// subsequence actually move. For a pure "swap the two ends" workload this
/// finds the true minimum (2 moves) where `place_in_order`'s matching-only
/// approach was shown by measurement to move nearly every retained entity
/// (research-questions/RQ1-dynamic-control-flow-scheduling.md's Phase 1
/// Track L section) — see this function's sibling for that non-LIS variant
/// and why both are kept as separate, comparable candidates rather than one
/// replacing the other.
fn place_in_order_lis(
    parent: NodeHandle,
    current: &[Entity],
    target: &[Slot],
    pending: &mut PendingDomWrites,
) {
    let old_index_of: std::collections::HashMap<Entity, usize> =
        current.iter().enumerate().map(|(i, &e)| (e, i)).collect();

    // Old-index sequence for retained entities only, in target order —
    // fresh entities have no old position to participate with.
    let retained_old_indices: Vec<usize> = target
        .iter()
        .filter_map(|slot| match slot {
            Slot::Retained(e) => old_index_of.get(e).copied(),
            Slot::Fresh { .. } => None,
        })
        .collect();
    let lis = lis_indices(&retained_old_indices);

    // `keep_in_place[i]` is only meaningful when `target[i]` is `Retained`;
    // built by walking retained slots in order and checking LIS membership
    // by their position *within the retained-only subsequence*.
    let mut keep_in_place = vec![false; target.len()];
    let mut retained_seen = 0;
    for (i, slot) in target.iter().enumerate() {
        if matches!(slot, Slot::Retained(_)) {
            if lis.contains(&retained_seen) {
                keep_in_place[i] = true;
            }
            retained_seen += 1;
        }
    }

    let mut chain = Chain::from_slice(current);
    let mut next_anchor: Option<Entity> = None;

    for (i, slot) in target.iter().enumerate().rev() {
        let entity = match slot {
            Slot::Fresh { entity, text } => {
                pending.0.push(DomWrite::SpawnRow {
                    entity: *entity,
                    parent,
                    tag: "li",
                    text: text.clone(),
                    before: next_anchor,
                });
                chain.insert_before(*entity, next_anchor);
                *entity
            }
            Slot::Retained(entity) => {
                if keep_in_place[i] {
                    // Trust the LIS proof: this entity's relative order is
                    // already correct, no adjacency check or move needed.
                } else {
                    // Still worth a cheap adjacency check: LIS picks *a*
                    // maximal subsequence, not necessarily the only one, so
                    // an entity outside it can occasionally already be
                    // correctly placed too — skip a redundant DOM write.
                    let already_adjacent = match next_anchor {
                        Some(a) => chain.next_of(*entity) == Some(a),
                        None => chain.is_tail(*entity),
                    };
                    if !already_adjacent {
                        chain.unlink(*entity);
                        chain.insert_before(*entity, next_anchor);
                        pending.0.push(DomWrite::Reposition {
                            parent,
                            entity: *entity,
                            before: next_anchor,
                        });
                    }
                }
                *entity
            }
        };
        next_anchor = Some(entity);
    }
}

// ============================================================================
// Candidate D — Keyed sequence sync (controller-side ordered Vec)
// ============================================================================

/// D's per-row key component (Phase 1 scope: key == an entity's own display
/// text — no separate key/value channel; see
/// research-questions/RQ1-dynamic-control-flow-scheduling.md's candidate D
/// write-up for the fuller design this simplifies).
#[derive(Component, Debug, Clone)]
pub struct RowKey(pub String);

/// RQ1 candidate **D** ("Keyed 시퀀스 동기화", controller-Vec storage
/// variant): `rows.0` (reused from `lib.rs`'s `ListRows`) holds the DOM
/// order directly. Retained rows that are already correctly placed cost
/// zero DOM operations; only rows whose position actually changed get a
/// `Reposition`, and only new/removed keys get `SpawnRow`/`RemoveNode` —
/// this is the entire "row identity survives reordering" property K can't
/// offer.
pub fn list_reconcile_d_system<T>(
    mut lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), Changed<Signal<Vec<T>>>>,
    keys: Query<&RowKey>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (items, mut rows, anchor) in &mut lists {
        let new_keys: Vec<String> = items.0.iter().map(ToString::to_string).collect();
        let old_order: Vec<(String, Entity)> = rows
            .0
            .iter()
            .filter_map(|&e| keys.get(e).ok().map(|k| (k.0.clone(), e)))
            .collect();

        let matched = match_keys(&old_order, &new_keys);
        for stale in &matched.removed {
            // Read the handle before despawning — see `DomWrite::RemoveNode`'s
            // doc comment for why this can't be deferred to apply time.
            if let Ok(target) = dom_targets.get(*stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(*stale).despawn();
        }
        // `current` must reflect what the DOM will look like *after* the
        // `RemoveNode` writes above take effect (they're emitted first, so
        // the driver applies them first) — removed entities are excluded,
        // or all subsequent position/adjacency math would be computed
        // against stale slots that are about to disappear.
        let current: Vec<Entity> = old_order
            .iter()
            .map(|(_, e)| *e)
            .filter(|e| !matched.removed.contains(e))
            .collect();

        let mut retained_at: std::collections::HashMap<usize, Entity> =
            matched.retained.into_iter().collect();
        let target: Vec<Slot> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| match retained_at.remove(&i) {
                Some(e) => Slot::Retained(e),
                None => Slot::Fresh {
                    entity: commands.spawn(RowKey(key.clone())).id(),
                    text: key.clone(),
                },
            })
            .collect();

        place_in_order(anchor.0, &current, &target, &mut pending);

        rows.0 = target
            .iter()
            .map(|s| match s {
                Slot::Retained(e) => *e,
                Slot::Fresh { entity, .. } => *entity,
            })
            .collect();
    }
}

/// **D-LIS**: identical to `list_reconcile_d_system` in every respect
/// (matching, spawn, removal, controller-Vec storage) except the final
/// placement pass uses `place_in_order_lis` instead of `place_in_order` —
/// RQ1's "이동 최소화: LIS" variant, kept as a separate candidate from
/// plain D (not a replacement) so the two can be measured against each
/// other directly (research-questions/RQ1-dynamic-control-flow-scheduling.md).
pub fn list_reconcile_d_lis_system<T>(
    mut lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), Changed<Signal<Vec<T>>>>,
    keys: Query<&RowKey>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (items, mut rows, anchor) in &mut lists {
        let new_keys: Vec<String> = items.0.iter().map(ToString::to_string).collect();
        let old_order: Vec<(String, Entity)> = rows
            .0
            .iter()
            .filter_map(|&e| keys.get(e).ok().map(|k| (k.0.clone(), e)))
            .collect();

        let matched = match_keys(&old_order, &new_keys);
        for stale in &matched.removed {
            if let Ok(target) = dom_targets.get(*stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(*stale).despawn();
        }
        let current: Vec<Entity> = old_order
            .iter()
            .map(|(_, e)| *e)
            .filter(|e| !matched.removed.contains(e))
            .collect();

        let mut retained_at: std::collections::HashMap<usize, Entity> =
            matched.retained.into_iter().collect();
        let target: Vec<Slot> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| match retained_at.remove(&i) {
                Some(e) => Slot::Retained(e),
                None => Slot::Fresh {
                    entity: commands.spawn(RowKey(key.clone())).id(),
                    text: key.clone(),
                },
            })
            .collect();

        place_in_order_lis(anchor.0, &current, &target, &mut pending);

        rows.0 = target
            .iter()
            .map(|s| match s {
                Slot::Retained(e) => *e,
                Slot::Fresh { entity, .. } => *entity,
            })
            .collect();
    }
}

// ============================================================================
// Candidate E — OrderKey dense per-entity tag (no controller-side Vec)
// ============================================================================

/// Per-entity order tag. Unlike D's `RowKey` + controller `ListRows` Vec,
/// **order itself is per-entity data** here — the DOM order is *derived* by
/// sorting entities on this value, never stored as a separate ordered list.
/// This is the architectural distinction RQ1 draws between D and E.
///
/// Phase 1 scope: `OrderKey` is densely re-tagged to exactly `0..n-1` on
/// every reconcile (only entities whose *target* index actually changed get
/// a write) rather than RQ1's full fractional-indexing design (neighbor-
/// midpoint tags + local relabeling on gap exhaustion, native `moveBefore`
/// repositioning). That fuller design is deferred — see
/// research-questions/RQ1-dynamic-control-flow-scheduling.md's candidate E
/// and `roadmap/PHASES.md`'s Phase 1 status.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderKey(pub u64);

/// E's per-entity key (Phase 1 scope: key == display text, same
/// simplification as D's `RowKey`).
#[derive(Component, Debug, Clone)]
pub struct OrderedKey(pub String);

/// The set of row entities currently belonging to this list (an unordered
/// membership set — DOM order is *derived* from `OrderKey`, never stored
/// here).
#[derive(Component, Default, Debug, Clone)]
pub struct OrderKeyMembers(pub Vec<Entity>);

/// RQ1 candidate **E** ("OrderKey 밀집 순서 태그"): same reconciliation
/// shape as D, but order lives on each row entity (`OrderKey`) instead of a
/// controller-side `Vec` — architecturally closer to "archetype table
/// partial memcpy" (RQ1's description) since there's no separate ordered
/// index structure to keep in sync.
pub fn list_reconcile_e_system<T>(
    mut lists: Query<(&Signal<Vec<T>>, &mut OrderKeyMembers, &DomTarget), Changed<Signal<Vec<T>>>>,
    // Single query for both reading (`OrderedKey`, current `OrderKey`) and
    // later writing `OrderKey` — two separate Queries (one read, one write)
    // over the same component conflict under Bevy's system-param checker
    // (B0001) even though they're used at disjoint times within this one
    // system; merging avoids that without a `ParamSet`.
    mut keyed: Query<(&OrderedKey, &mut OrderKey)>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (items, mut members, anchor) in &mut lists {
        let new_keys: Vec<String> = items.0.iter().map(ToString::to_string).collect();

        let mut old_sorted: Vec<(String, Entity, OrderKey)> = members
            .0
            .iter()
            .filter_map(|&e| keyed.get(e).ok().map(|(k, ord)| (k.0.clone(), e, *ord)))
            .collect();
        old_sorted.sort_by_key(|(_, _, ord)| *ord);
        let old_order: Vec<(String, Entity)> =
            old_sorted.iter().map(|(k, e, _)| (k.clone(), *e)).collect();

        let matched = match_keys(&old_order, &new_keys);
        for stale in &matched.removed {
            // Read the handle before despawning — see `DomWrite::RemoveNode`'s
            // doc comment for why this can't be deferred to apply time.
            if let Ok(target) = dom_targets.get(*stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(*stale).despawn();
        }
        // See D's identical comment: `current` must exclude entities whose
        // `RemoveNode` write was just emitted above.
        let current: Vec<Entity> = old_order
            .iter()
            .map(|(_, e)| *e)
            .filter(|e| !matched.removed.contains(e))
            .collect();

        let mut retained_at: std::collections::HashMap<usize, Entity> =
            matched.retained.into_iter().collect();
        let target: Vec<Slot> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| match retained_at.remove(&i) {
                Some(e) => Slot::Retained(e),
                None => Slot::Fresh {
                    entity: commands.spawn((OrderedKey(key.clone()), OrderKey(i as u64))).id(),
                    text: key.clone(),
                },
            })
            .collect();

        // Dense re-tag: only entities whose target index actually changed
        // get an `OrderKey` write. Freshly spawned entities already got
        // their final `OrderKey` at spawn time above.
        for (i, slot) in target.iter().enumerate() {
            if let Slot::Retained(entity) = slot {
                if let Ok((_, mut key)) = keyed.get_mut(*entity) {
                    if key.0 != i as u64 {
                        key.0 = i as u64;
                    }
                }
            }
        }

        place_in_order(anchor.0, &current, &target, &mut pending);

        members.0 = target
            .iter()
            .map(|s| match s {
                Slot::Retained(e) => *e,
                Slot::Fresh { entity, .. } => *entity,
            })
            .collect();
    }
}

/// **E-LIS**: identical to `list_reconcile_e_system` (per-entity `OrderKey`,
/// no controller Vec) except the final placement pass uses
/// `place_in_order_lis` — see `list_reconcile_d_lis_system`'s doc comment
/// for why this is a separate, directly-comparable candidate rather than a
/// replacement.
pub fn list_reconcile_e_lis_system<T>(
    mut lists: Query<(&Signal<Vec<T>>, &mut OrderKeyMembers, &DomTarget), Changed<Signal<Vec<T>>>>,
    mut keyed: Query<(&OrderedKey, &mut OrderKey)>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    for (items, mut members, anchor) in &mut lists {
        let new_keys: Vec<String> = items.0.iter().map(ToString::to_string).collect();

        let mut old_sorted: Vec<(String, Entity, OrderKey)> = members
            .0
            .iter()
            .filter_map(|&e| keyed.get(e).ok().map(|(k, ord)| (k.0.clone(), e, *ord)))
            .collect();
        old_sorted.sort_by_key(|(_, _, ord)| *ord);
        let old_order: Vec<(String, Entity)> =
            old_sorted.iter().map(|(k, e, _)| (k.clone(), *e)).collect();

        let matched = match_keys(&old_order, &new_keys);
        for stale in &matched.removed {
            if let Ok(target) = dom_targets.get(*stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(*stale).despawn();
        }
        let current: Vec<Entity> = old_order
            .iter()
            .map(|(_, e)| *e)
            .filter(|e| !matched.removed.contains(e))
            .collect();

        let mut retained_at: std::collections::HashMap<usize, Entity> =
            matched.retained.into_iter().collect();
        let target: Vec<Slot> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| match retained_at.remove(&i) {
                Some(e) => Slot::Retained(e),
                None => Slot::Fresh {
                    entity: commands.spawn((OrderedKey(key.clone()), OrderKey(i as u64))).id(),
                    text: key.clone(),
                },
            })
            .collect();

        for (i, slot) in target.iter().enumerate() {
            if let Slot::Retained(entity) = slot {
                if let Ok((_, mut key)) = keyed.get_mut(*entity) {
                    if key.0 != i as u64 {
                        key.0 = i as u64;
                    }
                }
            }
        }

        place_in_order_lis(anchor.0, &current, &target, &mut pending);

        members.0 = target
            .iter()
            .map(|s| match s {
                Slot::Retained(e) => *e,
                Slot::Fresh { entity, .. } => *entity,
            })
            .collect();
    }
}

#[cfg(test)]
mod lis_tests {
    use super::lis_indices;

    #[test]
    fn empty_sequence() {
        assert!(lis_indices(&[]).is_empty());
    }

    #[test]
    fn already_increasing_keeps_everything() {
        let lis = lis_indices(&[0, 1, 2, 3, 4]);
        assert_eq!(lis.len(), 5);
    }

    #[test]
    fn swap_ends_finds_the_middle_as_lis() {
        // [A,B,C,D,E] -> [E,B,C,D,A]: old-index sequence in new order is
        // [4,1,2,3,0]. The LIS is {1,2,3} (B,C,D) — exactly the 3 middle
        // elements that shouldn't need to move for a pure end-swap.
        let lis = lis_indices(&[4, 1, 2, 3, 0]);
        let mut positions: Vec<usize> = lis.into_iter().collect();
        positions.sort();
        assert_eq!(positions, vec![1, 2, 3]);
    }

    #[test]
    fn full_reversal_lis_is_a_single_element() {
        // A full reversal has no increasing run longer than 1.
        let lis = lis_indices(&[4, 3, 2, 1, 0]);
        assert_eq!(lis.len(), 1);
    }

    #[test]
    fn head_insert_pattern_keeps_the_whole_tail() {
        // Prepending conceptually looks like old-indices [0,1,2,3] (no
        // "new" entry participates in the old-index sequence at all, since
        // fresh entities are filtered out before this function ever sees
        // them) — everything already-existing stays in the LIS.
        let lis = lis_indices(&[0, 1, 2, 3]);
        assert_eq!(lis.len(), 4);
    }
}
