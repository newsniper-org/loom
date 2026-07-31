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
    row_bindings: Res<crate::RowBindings>,
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
                        text: new_text.clone(),
                    });
                    // **2026-07-28 R2 축 수리**: 이 슬롯의 아이템이 바뀌었으므로
                    // 그 아이템에서 파생된 binding도 전부 다시 써야 한다.
                    // 이것이 RQ1이 C의 비용으로 예측한 "O(n×binding) retained-slot
                    // value-patch"이고, 그 전까지 계측에 존재하지 않던 부분이다
                    // (§9 C 기각 조항이 겨냥하는 바로 그 비용).
                    let extra = crate::extra_binding_texts(row_bindings.0, &new_text);
                    if !extra.is_empty() {
                        pending.0.push(DomWrite::SetRowBindingTexts { row: entity, texts: extra });
                    }
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
                    text: text.clone(),
                    before: None, // growing the tail always appends.
                });
                let extra = crate::extra_binding_texts(row_bindings.0, &text);
                if !extra.is_empty() {
                    pending.0.push(DomWrite::AppendStaticChildren { row: entity, texts: extra });
                }
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
fn place_in_order(
    parent: NodeHandle,
    current: &[Entity],
    target: &[Slot],
    pending: &mut PendingDomWrites,
    row_bindings: usize,
) {
    // 순서 결정은 전부 검증된 `loom_flush_core::plan_writes`가 한다 — 이
    // 함수에 남은 일은 그 계획을 `DomWrite`로 옮겨 적는 것뿐이다.
    let ops = loom_flush_core::plan_writes(
        &current.iter().map(|e| e.to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| slot_entity(s).to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| matches!(s, Slot::Fresh { .. })).collect::<Vec<bool>>(),
        &vec![false; target.len()], // LIS 변형이 아니므로 강제 유지 없음
    );
    emit_plan(parent, target, &ops, pending, row_bindings);
}

/// `Slot`에서 Entity 하나를 꺼낸다.
fn slot_entity(slot: &Slot) -> Entity {
    match slot {
        Slot::Fresh { entity, .. } => *entity,
        Slot::Retained(entity) => *entity,
    }
}

/// 검증된 계획(`PlanOp` 열, 방출 순서 그대로)을 실제 `DomWrite`로 옮긴다.
/// **순서 판단은 여기 없다** — `before` anchor도 `plan_writes`가 정한 것을
/// 그대로 쓴다. 이 함수가 하는 일은 Entity/텍스트를 붙이는 것뿐이다.
fn emit_plan(
    parent: NodeHandle,
    target: &[Slot],
    ops: &[loom_flush_core::PlanOp],
    pending: &mut PendingDomWrites,
    row_bindings: usize,
) {
    for op in ops {
        let slot = &target[op.idx];
        let entity = slot_entity(slot);
        // `plan_writes`의 anchor는 u64(Entity::to_bits)이므로 되돌린다.
        let before = op.before.map(Entity::from_bits);
        if op.spawn {
            let text = match slot {
                Slot::Fresh { text, .. } => text.clone(),
                Slot::Retained(_) => unreachable!("plan_writes marks spawn only for Fresh slots"),
            };
            pending.0.push(DomWrite::SpawnRow {
                entity,
                parent,
                tag: "li",
                text: text.clone(),
                before,
            });
            let extra = crate::extra_binding_texts(row_bindings, &text);
            if !extra.is_empty() {
                pending.0.push(DomWrite::AppendStaticChildren { row: entity, texts: extra });
            }
        } else {
            pending.0.push(DomWrite::Reposition { parent, entity, before });
        }
    }
}

/// **2026-07-28 — 손코딩 `Chain`은 `loom-flush-core`의 Verus 검증본으로
/// 대체됐다**(RQ5 후보 #4). 이전에는 여기 `std::collections::HashMap` 두 개로
/// 된 이중연결리스트가 손코딩돼 있었고, 그 맵 수술(`unlink`의 네 갈래,
/// `insert_before`의 두 갈래)이 실제로 순서를 올바르게 재결선하는지는
/// 검증되지 않은 채였다.
///
/// **2026-07-28 (같은 날, 2단계)**: 방출 루프 자체도 검증 크레이트로 옮겼다.
/// 이제 `list.rs`는 `Chain`을 **직접 만지지 않고** `plan_writes`가 돌려준
/// 계획을 `DomWrite`로 옮겨 적기만 한다 — 순서 판단이 이 파일에서 완전히
/// 사라졌으므로, Chain 연산의 사전조건 준수까지 기계적으로 검증된다.
/// `formal-verification/verus/README.md` 참고.
///
/// `Entity`는 Verus가 다룰 수 없는 불투명 타입이라 `to_bits()`로 `u64`를
/// 만들어 넘긴다(단사이므로 동일성·구별성 보존).
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
    row_bindings: usize,
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

    // 순서 결정은 검증된 `plan_writes`가 한다 — LIS가 고른 "유지" 표시는
    // `keep`으로 넘긴다. **LIS 판단 자체의 정당성은 검증 범위 밖**이고
    // (`lis_indices`는 비검증 코드), `plan_writes`가 보장하는 것은 그 표시를
    // 받아들였을 때도 Chain 조작의 사전조건이 지켜진다는 것이다.
    let ops = loom_flush_core::plan_writes(
        &current.iter().map(|e| e.to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| slot_entity(s).to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| matches!(s, Slot::Fresh { .. })).collect::<Vec<bool>>(),
        &keep_in_place,
    );
    emit_plan(parent, target, &ops, pending, row_bindings);
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
    row_bindings: Res<crate::RowBindings>,
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

        place_in_order(anchor.0, &current, &target, &mut pending, row_bindings.0);

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
    row_bindings: Res<crate::RowBindings>,
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

        place_in_order_lis(anchor.0, &current, &target, &mut pending, row_bindings.0);

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
    row_bindings: Res<crate::RowBindings>,
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

        place_in_order(anchor.0, &current, &target, &mut pending, row_bindings.0);

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
    row_bindings: Res<crate::RowBindings>,
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

        place_in_order_lis(anchor.0, &current, &target, &mut pending, row_bindings.0);

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
