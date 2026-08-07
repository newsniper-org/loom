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
    variant_mode: Res<crate::RowVariantMode>,
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
                    let old_text = slot_value.0.clone();
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
                    // **R3(§3.2) — 이질 브랜치 row**: 값이 바뀌면 그 값에서
                    // 파생된 브랜치도 바뀔 수 있다. §10이 전환형을 "슬롯 entity
                    // 컴포넌트 교체(archetype move)형"으로 사전 고정했으므로
                    // marker를 교체한다 — 이 Commands가 sync point에서 적용되며
                    // archetype 이동을 일으키고, §9 C 기각 (ii)가 그 구간을 잰다.
                    if variant_mode.0 {
                        let was_a = crate::row_variant_is_a(&old_text);
                        let now_a = crate::row_variant_is_a(&new_text);
                        if was_a != now_a {
                            if now_a {
                                commands.entity(entity)
                                    .remove::<crate::RowVariantB>()
                                    .insert(crate::RowVariantA);
                            } else {
                                commands.entity(entity)
                                    .remove::<crate::RowVariantA>()
                                    .insert(crate::RowVariantB);
                            }
                        }
                    }
                }
            }
        }

        if new_len > old_len {
            for item in &items.0[old_len..new_len] {
                let text = item.to_string();
                let entity = commands.spawn(SlotValue(text.clone())).id();
                if variant_mode.0 {
                    if crate::row_variant_is_a(&text) {
                        commands.entity(entity).insert(crate::RowVariantA);
                    } else {
                        commands.entity(entity).insert(crate::RowVariantB);
                    }
                }
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
fn place_in_order<R: crate::RowSpawner>(
    site: &R::Ctx,
    parent: NodeHandle,
    current: &[Entity],
    target: &[Slot],
    out: &mut crate::RowSpawnCtx,
) {
    // 순서 결정은 전부 검증된 `loom_flush_core::plan_writes`가 한다 — 이
    // 함수에 남은 일은 그 계획을 `DomWrite`로 옮겨 적는 것뿐이다.
    // **수렴까지 증명된 판본**을 쓴다(2026-08-01). `plan_writes`(LIS 변형용)와
    // 달리 `keep`이 없어 불변식이 전부 무조건이고, 그래서 "방출한 op를 적용하면
    // DOM의 뒤쪽 |target|개가 목표 순서와 일치한다"가 **사후조건으로** 증명된다
    // (RQ5 #4 정리 (2) — `formal-verification/verus/README.md` 참고).
    let ops = loom_flush_core::plan_writes_converging(
        &current.iter().map(|e| e.to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| slot_entity(s).to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| matches!(s, Slot::Fresh { .. })).collect::<Vec<bool>>(),
    );
    emit_plan::<R>(site, parent, target, &ops, out);
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
fn emit_plan<R: crate::RowSpawner>(
    site: &R::Ctx,
    parent: NodeHandle,
    target: &[Slot],
    ops: &[loom_flush_core::PlanOp],
    out: &mut crate::RowSpawnCtx,
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
            R::emit(site, out, entity, parent, &text, before);
        } else {
            out.pending.push(DomWrite::Reposition { parent, entity, before });
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
fn place_in_order_lis<R: crate::RowSpawner>(
    site: &R::Ctx,
    parent: NodeHandle,
    current: &[Entity],
    target: &[Slot],
    out: &mut crate::RowSpawnCtx,
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
    // `keep`으로 넘긴다.
    //
    // **`keep`이 지켜야 하는 조건 P**:
    // 표시된 원소들의 `current` 내 상대 순서가 target 순서와 같아야 한다.
    // 이것이 정확히 필요충분조건임은 전수 탐색으로 확인됐다(2026-08-02) —
    // P를 위반하면 **100%** 순서가 깨진다. `lis_indices`가 P를 세운다는 것
    // (= LIS 건전성)은 아직 증명되지 않았으므로, 디버그 빌드에서 실제로
    // 검사한다(P를 Verus 명세로 올리는 것은 실패했다 —
    // `formal-verification/verus/README.md`의 "검증이 rlimit 경계에 붙어
    // 있다" 절). 여기서 필요한 건 건전성뿐이고 **최적성은 아니다** — LIS가
    // 최적이 아니면 이동이 더 나올 뿐 순서는 깨지지 않는다(성능 성질).
    debug_assert!(
        {
            let mut last: Option<usize> = None;
            let mut ok = true;
            for (i, slot) in target.iter().enumerate() {
                if keep_in_place[i] {
                    if let Slot::Retained(e) = slot {
                        let pos = old_index_of[e];
                        if last.is_some_and(|l| pos <= l) {
                            ok = false;
                        }
                        last = Some(pos);
                    }
                }
            }
            ok
        },
        "lis_indices가 사전조건 P를 위반했다 — 표시된 원소들의 current 내 \
         상대 순서가 target 순서와 다르다. 이 상태로 진행하면 DOM 순서가 깨진다."
    );
    let ops = loom_flush_core::plan_writes(
        &current.iter().map(|e| e.to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| slot_entity(s).to_bits()).collect::<Vec<u64>>(),
        &target.iter().map(|s| matches!(s, Slot::Fresh { .. })).collect::<Vec<bool>>(),
        &keep_in_place,
    );
    emit_plan::<R>(site, parent, target, &ops, out);
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
fn d_body<T, R: crate::RowSpawner, F: bevy_ecs::query::QueryFilter>(
    site: &R::Ctx,
    mut lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), F>,
    keys: Query<&RowKey>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
    row_bindings: Res<crate::RowBindings>,
    variant_mode: Res<crate::RowVariantMode>,
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

        {
            let mut out = crate::RowSpawnCtx {
                commands: &mut commands,
                pending: &mut pending.0,
                row_bindings: row_bindings.0,
            };
            place_in_order::<R>(site, anchor.0, &current, &target, &mut out);
        }

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
fn d_lis_body<T, R: crate::RowSpawner, F: bevy_ecs::query::QueryFilter>(
    site: &R::Ctx,
    mut lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), F>,
    keys: Query<&RowKey>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
    row_bindings: Res<crate::RowBindings>,
    variant_mode: Res<crate::RowVariantMode>,
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

        {
            let mut out = crate::RowSpawnCtx {
                commands: &mut commands,
                pending: &mut pending.0,
                row_bindings: row_bindings.0,
            };
            place_in_order_lis::<R>(site, anchor.0, &current, &target, &mut out);
        }

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
fn e_body<T, R: crate::RowSpawner, F: bevy_ecs::query::QueryFilter>(
    site: &R::Ctx,
    mut lists: Query<(&Signal<Vec<T>>, &mut OrderKeyMembers, &DomTarget), F>,
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
    variant_mode: Res<crate::RowVariantMode>,
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

        {
            let mut out = crate::RowSpawnCtx {
                commands: &mut commands,
                pending: &mut pending.0,
                row_bindings: row_bindings.0,
            };
            place_in_order::<R>(site, anchor.0, &current, &target, &mut out);
        }

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
fn e_lis_body<T, R: crate::RowSpawner, F: bevy_ecs::query::QueryFilter>(
    site: &R::Ctx,
    mut lists: Query<(&Signal<Vec<T>>, &mut OrderKeyMembers, &DomTarget), F>,
    mut keyed: Query<(&OrderedKey, &mut OrderKey)>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
    row_bindings: Res<crate::RowBindings>,
    variant_mode: Res<crate::RowVariantMode>,
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

        {
            let mut out = crate::RowSpawnCtx {
                commands: &mut commands,
                pending: &mut pending.0,
                row_bindings: row_bindings.0,
            };
            place_in_order_lis::<R>(site, anchor.0, &current, &target, &mut out);
        }

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

    /// **사전조건 P의 건전성 무작위 검사** — `lis_indices`가 돌려준 인덱스
    /// 집합이 정말 *증가* 부분수열인가. 이것이 `plan_writes`의 `keep`이
    /// 지켜야 하는 조건(`loom_flush_core::kept_order_preserved`)의 전부다.
    ///
    /// 최적성(정말 *가장 긴가*)은 **검사하지 않는다** — 최적이 아니면 이동이
    /// 더 나올 뿐 순서는 깨지지 않으므로 성능 성질이지 정합성 성질이 아니다.
    /// (전수 탐색 결과는 `formal-verification/verus/README.md` 참고.)
    #[test]
    fn lis_result_is_always_an_increasing_subsequence() {
        // PCG64 대신 결정적 xorshift로 충분 — 이건 판정용 표본이 아니라
        // 속성 검사이고, 시드가 고정이면 재현된다.
        let mut state: u64 = 0x2026_0802_0000_0001;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for n in 1..=40usize {
            for _ in 0..50 {
                // 임의의 순열 (Fisher-Yates)
                let mut seq: Vec<usize> = (0..n).collect();
                for i in (1..n).rev() {
                    let j = (next() as usize) % (i + 1);
                    seq.swap(i, j);
                }
                let lis = lis_indices(&seq);
                let mut idx: Vec<usize> = lis.into_iter().collect();
                idx.sort_unstable();
                assert!(!idx.is_empty(), "비어 있지 않은 입력에 빈 LIS: {seq:?}");
                for w in idx.windows(2) {
                    assert!(
                        seq[w[0]] < seq[w[1]],
                        "P 위반: seq={seq:?} idx={idx:?} — {} !< {}",
                        seq[w[0]],
                        seq[w[1]]
                    );
                }
            }
        }
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

// ============================================================================
// row 모양의 사이트별 monomorphize (2026-08-07)
// ============================================================================
//
// 위 네 후보의 **알고리즘 본문은 하나씩**(`d_body`/`d_lis_body`/`e_body`/
// `e_lis_body`)이고, 아래 여덟 함수는 그 본문을 `R` 하나만 바꿔 부르는 얇은
// 껍데기다. 즉 "제네릭 판본"이 생겼다고 해서 후보 동작이 두 갈래로 갈라지지
// 않는다 — `list_reconcile_d_system`은 `DefaultRow`로 부른 `d_body`이고,
// `DefaultRow`는 리팩터 이전 `emit_plan`이 내던 write 열을 그대로 낸다.
//
// 트랙 L 하네스는 계속 접미사 없는 쪽(`list_reconcile_d_system` 등)을 등록한다 —
// 시그니처가 그대로이므로 하네스는 한 줄도 바뀌지 않았고, 따라서 §2.5의
// 재측정 사유가 되지 않는다. 접미사 `_with` 쪽은 loom-compiler가 복잡한 row
// 템플릿을 만났을 때만 쓴다.

macro_rules! keyed_candidate_systems {
    ($body:ident, $plain:ident, $with:ident, $rows:ty, $keys:ty) => {
        pub fn $plain<T>(
            lists: Query<(&Signal<Vec<T>>, &mut $rows, &DomTarget), Changed<Signal<Vec<T>>>>,
            keys: $keys,
            dom_targets: Query<&DomTarget>,
            commands: Commands,
            pending: ResMut<PendingDomWrites>,
            row_bindings: Res<crate::RowBindings>,
            variant_mode: Res<crate::RowVariantMode>,
        ) where
            T: Send + Sync + Clone + ToString + 'static,
        {
            $body::<T, crate::DefaultRow, _>(
                &crate::NoRowCtx,
                lists,
                keys,
                dom_targets,
                commands,
                pending,
                row_bindings,
                variant_mode,
            );
        }

        pub fn $with<T, R: crate::RowSpawner>(
            lists: Query<
                (&Signal<Vec<T>>, &mut $rows, &DomTarget),
                (Changed<Signal<Vec<T>>>, With<R::Marker>),
            >,
            keys: $keys,
            dom_targets: Query<&DomTarget>,
            commands: Commands,
            pending: ResMut<PendingDomWrites>,
            row_bindings: Res<crate::RowBindings>,
            variant_mode: Res<crate::RowVariantMode>,
            site: Res<R::Ctx>,
        ) where
            T: Send + Sync + Clone + ToString + 'static,
        {
            $body::<T, R, _>(
                &site,
                lists,
                keys,
                dom_targets,
                commands,
                pending,
                row_bindings,
                variant_mode,
            );
        }
    };
}

keyed_candidate_systems!(
    d_body,
    list_reconcile_d_system,
    list_reconcile_d_system_with,
    crate::ListRows,
    Query<&RowKey>
);
keyed_candidate_systems!(
    d_lis_body,
    list_reconcile_d_lis_system,
    list_reconcile_d_lis_system_with,
    crate::ListRows,
    Query<&RowKey>
);
keyed_candidate_systems!(
    e_body,
    list_reconcile_e_system,
    list_reconcile_e_system_with,
    OrderKeyMembers,
    Query<(&OrderedKey, &mut OrderKey)>
);
keyed_candidate_systems!(
    e_lis_body,
    list_reconcile_e_lis_system,
    list_reconcile_e_lis_system_with,
    OrderKeyMembers,
    Query<(&OrderedKey, &mut OrderKey)>
);
