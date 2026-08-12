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
fn c_body<T, R: crate::RowSpawner, F: bevy_ecs::query::QueryFilter>(
    site: &R::Ctx,
    mut lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), F>,
    mut slots: Query<(&mut SlotValue, &DomTarget)>,
    owned: Query<&crate::RowOwned>,
    variant_nodes: Query<&crate::RowVariantNode>,
    variant_targets: Query<&DomTarget>,
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
                    // **개정 #9-11(2026-08-12)로 여기서 하던 일이 사라졌다.**
                    // 예전에는 이 자리에서 `SetRowBindingTexts`로 R2 바인딩 핸들에
                    // 직접 썼다("O(n×binding) retained-slot value-patch"). 이제
                    // 바인딩은 자기 entity + 자기 `Signal`을 갖고, 아래
                    // `RowSpawner::revalue`가 그 Signal을 갈아 끼우면
                    // `patch_row_binding_system`이 `Changed` 게이팅을 거쳐 패치한다.
                    // **같은 비용을 조항이 정한 기계로 내는 것**이지 비용을 없앤 게
                    // 아니다 — 여기서 write를 계속 내면 이중 기록이 되고, 게다가
                    // `RowBindingTargets`가 더 이상 존재하지 않아 전부
                    // `dropped_writes`로 떨어진다.
                    // **R3(§3.2) — 이질 브랜치 row**: 값이 바뀌면 그 값에서
                    // 파생된 브랜치도 바뀔 수 있다. §10이 전환형을 "슬롯 entity
                    // 컴포넌트 교체(archetype move)형"으로 사전 고정했으므로
                    // marker를 교체한다 — 이 Commands가 sync point에서 적용되며
                    // archetype 이동을 일으키고, §9 C 기각 (ii)가 그 구간을 잰다.
                    // 사이트별 row 생성기의 파생값 재계산 (2026-08-08).
                    // C는 아이템이 제자리에서 바뀌는 유일한 후보이므로, row에서
                    // 파생된 값과 그 mirror entity가 여기서 갱신돼야 한다.
                    // `DefaultRow`에서는 no-op이라 트랙 L 측정에 영향이 없다.
                    {
                        let owned_slice: &[Entity] =
                            owned.get(entity).map(|o| o.0.as_slice()).unwrap_or(&[]);
                        let mut out = crate::RowSpawnCtx {
                            commands: &mut commands,
                            pending: &mut pending.0,
                            row_bindings: row_bindings.0,
                            row_variants: variant_mode.0,
                        };
                        R::revalue(site, &mut out, entity, owned_slice, &new_text);
                    }
                    if variant_mode.0 {
                        let was_a = crate::row_variant_is_a(&old_text);
                        let now_a = crate::row_variant_is_a(&new_text);
                        if was_a != now_a {
                            if now_a {
                                commands.entity(entity)
                                    .remove::<crate::RowVariantB>()
                                    .insert(crate::RowVariantA);
                                // **구조 분기 전환 B→A**(개정 #9-12): `<em>` 래퍼를
                                // 실제로 걷어낸다. 핸들은 despawn 전에 읽어야 한다 —
                                // Commands는 sync point에서 적용되고 write 적용 루프는
                                // 그 뒤에 돌기 때문(`DomWrite::RemoveNode` doc 참고).
                                if let Some(node) = variant_nodes.get(entity).ok().copied() {
                                    if let Ok(target) = variant_targets.get(node.0) {
                                        pending.0.push(DomWrite::RemoveNode { node: target.0 });
                                    }
                                    commands.entity(node.0).despawn();
                                    commands.entity(entity).remove::<crate::RowVariantNode>();
                                }
                            } else {
                                commands.entity(entity)
                                    .remove::<crate::RowVariantA>()
                                    .insert(crate::RowVariantB);
                                // 전환 A→B: 래퍼를 새로 만든다.
                                let em = commands
                                    .spawn(::bevy_ecs::hierarchy::ChildOf(entity))
                                    .id();
                                pending.0.push(DomWrite::SpawnElement {
                                    entity: em,
                                    parent: crate::WriteParent::Entity(entity),
                                    tag: "em",
                                    class: None,
                                    loom_id: None,
                                    text: Some(new_text.clone()),
                                });
                                commands.entity(entity).insert(crate::RowVariantNode(em));
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
                rows.0.push(entity);
                // R3 브랜치 marker와 R2 추가 binding은 `DefaultRow::emit`이
                // 함께 낸다 — 여기 인라인으로 두면 사이트별 생성기를 쓸 때
                // 두 번 붙는다. 방출되는 write 열은 동일하다.
                {
                    let mut out = crate::RowSpawnCtx {
                        commands: &mut commands,
                        pending: &mut pending.0,
                        row_bindings: row_bindings.0,
                        row_variants: variant_mode.0,
                    };
                    // 꼬리 성장은 항상 append이므로 앵커 없음.
                    R::emit(site, &mut out, entity, anchor.0, &text, None);
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

/// Key-matches `old_order` against `new_keys` and reports the result as an
/// **index selection**: `take_old[i]` is the old index new slot `i` takes
/// over, or `>= old_order.len()` when slot `i` is fresh.
///
/// **2026-08-11 — 조립이 검증 크레이트로 넘어갔다.** 예전에는 이 함수가
/// `(retained, removed)`를 내고 호출부 네 곳이 거기서 `current`와 `target`을
/// *따로* 조립했다. 그래서 `plan_writes`의 사전조건 Q("`current`에 target 밖
/// 원소가 없다")가 릴리스 빌드에서 사라지는 `debug_assert!` 하나에만 걸려
/// 있었다. 지금은 `loom_flush_core::plan_inputs`가 이 선택 하나로부터
/// `current`와 `target_ids`를 **함께** 만들므로 Q가 그 함수의 **사후조건으로
/// 증명**된다 — 매칭이 옳든 그르든 상관없이 구조적으로 참이다. 여기 남은 것은
/// 키 비교(String)뿐이다.
///
/// first-match-wins + 소비(`pool.remove`) 규율은 그대로다. **중복 키는 실제로
/// 발생한다**: `crates/loom-bench`의 POD 축 × W7(`value_update`) 셀에서
/// fresh 민터의 공역(100만)이 n=10,000에서 부족해지며 rep의 34%가 중복 키를
/// D/E에 먹인다. 그 경우 같은 키의 두 번째 등장은 pool에 짝이 남아 있으면
/// 그것과 매치되고 없으면 fresh가 되며, 필요한 성질은 전부 성립한다(전수 확인:
/// `formal-verification/verus/match-keys-exhaustive.py`). 이 자리에 있던
/// "Phase 1's demo doesn't exercise duplicate keys"는 사실이 아니어서 함께
/// 정정했다.
fn match_keys(old_order: &[(String, Entity)], new_keys: &[String]) -> Vec<usize> {
    let mut pool: Vec<(String, usize)> = old_order
        .iter()
        .enumerate()
        .map(|(j, (key, _))| (key.clone(), j))
        .collect();
    let mut take_old = Vec::with_capacity(new_keys.len());
    for key in new_keys {
        if let Some(pos) = pool.iter().position(|(k, _)| k == key) {
            let (_, j) = pool.remove(pos);
            take_old.push(j);
        } else {
            take_old.push(old_order.len());
        }
    }
    take_old
}

/// `loom_flush_core::plan_inputs`(및 두 래퍼)의 **구조적 사전조건 넷**을
/// 디버그 빌드에서 실제로 검사한다.
///
/// 이번 이관으로 비검증 코드가 지는 짐은 여섯(현재/타깃 무중복, 길이,
/// fresh↔포함, P, Q)에서 **넷**으로 줄었고, 남은 넷은 전부 *원시 입력에 대한
/// 구조적 사실*이지 `current`/`target`의 상호작용에 대한 명제가 아니다:
///
/// 1. **옛 Entity가 서로 다르다.** 귀납적으로 닫혀 있다 —
///    `ListRows`/`OrderKeyMembers`는 `Default`(빈 Vec)로만 생기고 코드 전체에서
///    오직 `plan_*`의 `target_ids`로만 대입되는데, 그 유일성은 이제
///    `plan_inputs`의 **사후조건**이다. 즉 검사가 보는 것은 귀납의 기저다.
/// 2. **`take_old`가 단사다.** `match_keys`의 소비(`pool.remove`) 규율이 준다.
/// 3. **fresh Entity가 옛 것과 겹치지 않는다.** `commands.spawn()`은 live가
///    아닌 새 Entity를 주고, 같은 시스템의 `despawn()`은 Commands 큐에 들어가
///    나중에 적용되므로 이 reconcile 중에 인덱스 재활용이 일어날 수 없다.
/// 4. **fresh Entity끼리도 다르다.** 같은 이유.
///
/// (2)~(4)는 지금까지 **아무도 검사하지 않던 것**이라 debug 검사가 순증이다.
fn selection_preconditions_hold(old_ids: &[u64], take_old: &[usize], fresh_ids: &[u64]) -> bool {
    use std::collections::HashSet;
    let m = old_ids.len();
    let mut olds: HashSet<u64> = HashSet::with_capacity(m);
    if !old_ids.iter().all(|bits| olds.insert(*bits)) {
        return false;
    }
    let mut taken: HashSet<usize> = HashSet::with_capacity(take_old.len());
    let mut fresh: HashSet<u64> = HashSet::new();
    for (i, &t) in take_old.iter().enumerate() {
        if t < m {
            if !taken.insert(t) {
                return false;
            }
        } else if olds.contains(&fresh_ids[i]) || !fresh.insert(fresh_ids[i]) {
            return false;
        }
    }
    true
}

/// 위 넷을 어긴 채로 진행하면 무슨 일이 나는지 한 곳에 적어 둔다 —
/// `debug_assert!` 메시지를 네 호출부에 복사하지 않기 위한 상수.
const SELECTION_PRECONDITION_MSG: &str =
    "plan_inputs의 구조적 사전조건 위반 — 옛 Entity 중복 / take_old 비단사 / \
     fresh Entity 재사용 중 하나다. 이 상태로 진행하면 current·target_ids의 \
     무중복과 fresh↔포함 대응이 깨져 DOM 순서 보증이 성립하지 않는다.";

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
/// **2026-08-11**: 이 설명이 가리키던 `place_in_order`는 사라졌다 — 하던 일
/// (`current`/`target` 조립 + `plan_writes_converging` 호출)이 통째로
/// `loom_flush_core::plan_from_selection` 안으로 들어갔고, 그래서 그 함수의
/// 사후조건이 `plan_writes_converging`의 사전조건을 실제로 충족하는지가
/// **한 검증 단위 안에서 기계검사**된다(예전에는 그 연결이 이 파일의 주석과
/// `debug_assert!`였다). 여기 남은 것은 아래 `emit_plan` — 계획을 `DomWrite`로
/// 옮겨 적는 것뿐이다.
///
/// 검증된 계획(`PlanOp` 열, 방출 순서 그대로)을 실제 `DomWrite`로 옮긴다.
/// **순서 판단은 여기 없다** — `before` anchor도 `plan_writes`가 정한 것을
/// 그대로 쓴다. 이 함수가 하는 일은 Entity/텍스트를 붙이는 것뿐이다.
///
/// `texts[i]`는 슬롯 `i`의 표시 텍스트(= 키, Phase 1 범위)다. 예전에 여기
/// 있던 `unreachable!("plan_writes marks spawn only for Fresh slots")`는
/// 없앴다 — `op.spawn == target_fresh[op.idx]`가 `plan_*`의 **사후조건**이라
/// 런타임 갈래 자체가 없어졌다.
fn emit_plan<R: crate::RowSpawner>(
    site: &R::Ctx,
    parent: NodeHandle,
    target_ids: &[u64],
    texts: &[String],
    ops: &[loom_flush_core::PlanOp],
    out: &mut crate::RowSpawnCtx,
) {
    for op in ops {
        // 검증 코어의 id는 u64(`Entity::to_bits`)이므로 되돌린다.
        let entity = Entity::from_bits(target_ids[op.idx]);
        let before = op.before.map(Entity::from_bits);
        if op.spawn {
            R::emit(site, out, entity, parent, &texts[op.idx], before);
        } else {
            out.pending.push(DomWrite::Reposition { parent, entity, before });
        }
    }
}

// **2026-07-28 — 손코딩 `Chain`은 `loom-flush-core`의 Verus 검증본으로
// 대체됐다**(RQ5 후보 #4). 이전에는 여기 `std::collections::HashMap` 두 개로
// 된 이중연결리스트가 손코딩돼 있었고, 그 맵 수술(`unlink`의 네 갈래,
// `insert_before`의 두 갈래)이 실제로 순서를 올바르게 재결선하는지는
// 검증되지 않은 채였다.
//
// **2026-07-28 (같은 날, 2단계)**: 방출 루프 자체도 검증 크레이트로 옮겼다.
// 이제 `list.rs`는 `Chain`을 **직접 만지지 않고** `plan_writes`가 돌려준
// 계획을 `DomWrite`로 옮겨 적기만 한다 — 순서 판단이 이 파일에서 완전히
// 사라졌으므로, Chain 연산의 사전조건 준수까지 기계적으로 검증된다.
// `formal-verification/verus/README.md` 참고.
//
// **2026-08-11 (3단계)**: `current`/`target` **조립**도 옮겼다. 이제 이 파일이
// 검증 코어에 넘기는 것은 키 매칭의 결과인 인덱스 선택(`take_old`)뿐이고,
// `plan_writes`의 사전조건 여섯 중 다섯이 코어의 사후조건으로 선다. 남은
// 미검증 계약은 `lis_indices`가 P를 세운다는 것 하나였다.
//
// **2026-08-11 (4단계) — 그 하나도 없어졌다.** `lis_indices`와 `keep` 조립이
// 통째로 `loom-flush-core`로 옮겨 갔고(`loom_flush_core::lis_keep`), P는 그
// 함수의 **사후조건으로 증명**된다. 그래서 이 파일에는 P에 대한 계약이 아예
// 나타나지 않는다 — 아래 두 후보가 부르는 것은
// `plan_lis_from_selection_auto` 하나뿐이고, `keep`은 인자가 아니라 검증
// 크레이트 **안에서** 만들어져 같은 검증 단위 안에서
// `plan_lis_from_selection`의 `requires`에 물린다. `keep`을 여기서 받아
// 되넘겼다면 두 계약 사이를 아무도 검사하지 않아 `debug_assert!`와 같은 틈이
// 이름만 바꿔 남았을 것이다.
//
// 알고리즘은 그대로다(patience sorting + predecessor 재구성, O(n log n)) —
// 방출되는 write 열은 한 글자도 바뀌지 않는다. `partition_point`만 vstd에
// 스펙이 없어 의미가 같은 명시적 이분 탐색으로 대체됐다(술어가 `<`이므로
// 둘 다 lower bound). 전수 대조 30,153건에서 `keep` 벡터 불일치 0건.
//
// `Entity`는 Verus가 다룰 수 없는 불투명 타입이라 `to_bits()`로 `u64`를
// 만들어 넘긴다(단사이므로 동일성·구별성 보존).
//
// LIS 변형이 사 오는 것(RQ1의 "이동 최소화: 순열 매칭만 vs LIS" 축):
// retained entity 중 옛 자리가 증가 부분수열을 이루는 것들은 이미 올바른 상대
// 순서에 있으므로 `Reposition`이 **0회**고, 그 밖의 것만 실제로 움직인다.
// 양 끝 교환 워크로드에서 매칭 전용 방식이 거의 모든 retained entity를
// 옮기던 것과 달리 진짜 최소(2회)를 찾는다
// (research-questions/RQ1-dynamic-control-flow-scheduling.md의 Phase 1 트랙 L
// 절). 두 변형은 서로를 대체하지 않고 직접 비교 가능한 별개 후보로 남는다.
// **사전조건 Q는 여기서 사라졌다** (2026-08-11). 예전에 이 자리에는
// "`current`에 target 밖 원소(extras)가 없다"를 보는 `debug_assert!`가 있었고,
// 그것이 릴리스 빌드에서 사라지는 바람에 `plan_writes`의 수렴 사후조건이
// 비검증 코드의 선의에 걸려 있었다(Q 없이는 그 사후조건이 **거짓**이다 —
// 최소 반례 `current=[0,1] target=[0] keep=[true]`). 지금은
// `loom_flush_core::plan_inputs`가 `current`와 `target_ids`를 같은 선택에서
// **함께** 만들고, Q는 그 함수의 **사후조건으로 증명**된다. 검사할 것이
// 남아 있지 않다.
//
// **비-LIS 경로는 원래 Q가 필요 없다** — 거기서는 비인접이면 무조건 옮기므로
// 외래 원소가 저절로 앞으로 밀려난다. 두 경로의 진짜 비대칭이고, `keep`
// 단락이 사 오는 비용이다. 이 사실은 이관 뒤에도 그대로다.

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

        let take_old = match_keys(&old_order, &new_keys);
        let old_ids: Vec<u64> = old_order.iter().map(|(_, e)| e.to_bits()).collect();
        // fresh row는 **슬롯 순서대로** spawn한다 — 옛 코드가 target을 조립하며
        // 하던 것과 같은 순서라 Entity id 배정이 그대로다. despawn보다 앞서지만
        // `Commands::despawn`은 sync point까지 미뤄지므로 id 재활용이 끼어들 수
        // 없고, `pending`에 들어가는 write 순서(RemoveNode 먼저)도 그대로다.
        let fresh_ids: Vec<u64> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                if take_old[i] >= old_ids.len() {
                    commands.spawn(RowKey(key.clone())).id().to_bits()
                } else {
                    0
                }
            })
            .collect();
        debug_assert!(
            selection_preconditions_hold(&old_ids, &take_old, &fresh_ids),
            "{SELECTION_PRECONDITION_MSG}"
        );

        // `current`(= DOM에서 살아남는 옛 row들)와 `target_ids`/`target_fresh`를
        // **함께** 만드는 것이 이 호출이다 — 그래서 사전조건 Q가 구조적으로
        // 참이고, 그 사실이 `plan_writes_converging`의 `requires`를 실제로
        // 충족한다는 것까지 한 검증 단위 안에서 확인된다. `plan_writes`(LIS
        // 변형용)와 달리 `keep`이 없어 불변식이 전부 무조건이고, "방출한 op를
        // 적용하면 DOM의 뒤쪽 |target|개가 목표 순서와 일치한다"가 사후조건으로
        // 증명돼 있다(RQ5 #4 정리 (2) — `formal-verification/verus/README.md`).
        let plan = loom_flush_core::plan_from_selection(&old_ids, &take_old, &fresh_ids);

        // `plan.removed`는 `plan.current`와 서로소임이 증명돼 있다(사후조건) —
        // 즉 여기서 despawn하는 row가 뒤이어 배치되는 일이 있을 수 없다.
        for bits in &plan.removed {
            // Read the handle before despawning — see `DomWrite::RemoveNode`'s
            // doc comment for why this can't be deferred to apply time.
            let stale = Entity::from_bits(*bits);
            if let Ok(target) = dom_targets.get(stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(stale).despawn();
        }

        {
            let mut out = crate::RowSpawnCtx {
                commands: &mut commands,
                pending: &mut pending.0,
                row_bindings: row_bindings.0,
                row_variants: variant_mode.0,
            };
            emit_plan::<R>(site, anchor.0, &plan.target_ids, &new_keys, &plan.ops, &mut out);
        }

        rows.0 = plan.target_ids.iter().map(|b| Entity::from_bits(*b)).collect();
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

        let take_old = match_keys(&old_order, &new_keys);
        let old_ids: Vec<u64> = old_order.iter().map(|(_, e)| e.to_bits()).collect();
        // D와 같은 순서·같은 이유 — `list_reconcile_d_system` 참고.
        let fresh_ids: Vec<u64> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                if take_old[i] >= old_ids.len() {
                    commands.spawn(RowKey(key.clone())).id().to_bits()
                } else {
                    0
                }
            })
            .collect();
        debug_assert!(
            selection_preconditions_hold(&old_ids, &take_old, &fresh_ids),
            "{SELECTION_PRECONDITION_MSG}"
        );

        // `keep`은 **검증 크레이트 안에서** 만들어진다 — 여기서 받아 되넘기지
        // 않는 것이 핵심이다(사슬이 닫히는 자리는 이 파일이 아니라
        // `plan_lis_from_selection_auto` 안이다).
        let plan =
            loom_flush_core::plan_lis_from_selection_auto(&old_ids, &take_old, &fresh_ids);

        for bits in &plan.removed {
            let stale = Entity::from_bits(*bits);
            if let Ok(target) = dom_targets.get(stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(stale).despawn();
        }

        {
            let mut out = crate::RowSpawnCtx {
                commands: &mut commands,
                pending: &mut pending.0,
                row_bindings: row_bindings.0,
                row_variants: variant_mode.0,
            };
            emit_plan::<R>(site, anchor.0, &plan.target_ids, &new_keys, &plan.ops, &mut out);
        }

        rows.0 = plan.target_ids.iter().map(|b| Entity::from_bits(*b)).collect();
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

        let take_old = match_keys(&old_order, &new_keys);
        let old_ids: Vec<u64> = old_order.iter().map(|(_, e)| e.to_bits()).collect();
        // D와 같은 순서·같은 이유(`list_reconcile_d_system` 참고). **E의 차이는
        // 유지된다** — fresh row가 `OrderedKey` + 슬롯 인덱스 `OrderKey`로
        // 태어난다.
        let fresh_ids: Vec<u64> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                if take_old[i] >= old_ids.len() {
                    commands
                        .spawn((OrderedKey(key.clone()), OrderKey(i as u64)))
                        .id()
                        .to_bits()
                } else {
                    0
                }
            })
            .collect();
        debug_assert!(
            selection_preconditions_hold(&old_ids, &take_old, &fresh_ids),
            "{SELECTION_PRECONDITION_MSG}"
        );

        let plan = loom_flush_core::plan_from_selection(&old_ids, &take_old, &fresh_ids);

        for bits in &plan.removed {
            // Read the handle before despawning — see `DomWrite::RemoveNode`'s
            // doc comment for why this can't be deferred to apply time.
            let stale = Entity::from_bits(*bits);
            if let Ok(target) = dom_targets.get(stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(stale).despawn();
        }

        // Dense re-tag: only entities whose target index actually changed
        // get an `OrderKey` write. Freshly spawned entities already got
        // their final `OrderKey` at spawn time above.
        for (i, &t) in take_old.iter().enumerate() {
            if t < old_ids.len() {
                let entity = Entity::from_bits(plan.target_ids[i]);
                if let Ok((_, mut key)) = keyed.get_mut(entity) {
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
                row_variants: variant_mode.0,
            };
            emit_plan::<R>(site, anchor.0, &plan.target_ids, &new_keys, &plan.ops, &mut out);
        }

        members.0 = plan.target_ids.iter().map(|b| Entity::from_bits(*b)).collect();
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

        let take_old = match_keys(&old_order, &new_keys);
        let old_ids: Vec<u64> = old_order.iter().map(|(_, e)| e.to_bits()).collect();
        let fresh_ids: Vec<u64> = new_keys
            .iter()
            .enumerate()
            .map(|(i, key)| {
                if take_old[i] >= old_ids.len() {
                    commands
                        .spawn((OrderedKey(key.clone()), OrderKey(i as u64)))
                        .id()
                        .to_bits()
                } else {
                    0
                }
            })
            .collect();
        debug_assert!(
            selection_preconditions_hold(&old_ids, &take_old, &fresh_ids),
            "{SELECTION_PRECONDITION_MSG}"
        );

        // `keep`은 **검증 크레이트 안에서** 만들어진다 — 여기서 받아 되넘기지
        // 않는 것이 핵심이다(사슬이 닫히는 자리는 이 파일이 아니라
        // `plan_lis_from_selection_auto` 안이다).
        let plan =
            loom_flush_core::plan_lis_from_selection_auto(&old_ids, &take_old, &fresh_ids);

        for bits in &plan.removed {
            let stale = Entity::from_bits(*bits);
            if let Ok(target) = dom_targets.get(stale) {
                pending.0.push(DomWrite::RemoveNode { node: target.0 });
            }
            commands.entity(stale).despawn();
        }

        for (i, &t) in take_old.iter().enumerate() {
            if t < old_ids.len() {
                let entity = Entity::from_bits(plan.target_ids[i]);
                if let Ok((_, mut key)) = keyed.get_mut(entity) {
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
                row_variants: variant_mode.0,
            };
            emit_plan::<R>(site, anchor.0, &plan.target_ids, &new_keys, &plan.ops, &mut out);
        }

        members.0 = plan.target_ids.iter().map(|b| Entity::from_bits(*b)).collect();
    }
}

#[cfg(test)]
mod lis_tests {
    /// **2026-08-11 (4단계)** — 대상이 `super::lis_indices`(삭제됨)에서
    /// `loom_flush_core::lis_keep`으로 바뀌었다. 옛 함수는 retained 열에 대한
    /// LIS **인덱스 집합**을 냈고 새 함수는 `take_old` 전체에 대한 `keep`
    /// **마스크**를 낸다 — 표시되는 슬롯은 같으므로 아래 기대값은 그대로이고,
    /// 마스크를 인덱스 목록으로 되돌려 비교한다.
    ///
    /// 여기 있는 테스트는 전부 **retained 전용** 입력(`old_len == len`)이라
    /// 옛 판본의 `sequence` 인자와 글자 그대로 같은 것을 넣는다.
    use loom_flush_core::lis_keep;

    /// 표시된 슬롯의 인덱스를 오름차순으로.
    fn marked(keep: &[bool]) -> Vec<usize> {
        keep.iter()
            .enumerate()
            .filter_map(|(i, &k)| if k { Some(i) } else { None })
            .collect()
    }

    #[test]
    fn empty_sequence() {
        assert!(marked(&lis_keep(&Vec::new(), 0)).is_empty());
    }

    #[test]
    fn already_increasing_keeps_everything() {
        let lis = marked(&lis_keep(&vec![0, 1, 2, 3, 4], 5));
        assert_eq!(lis.len(), 5);
    }

    #[test]
    fn swap_ends_finds_the_middle_as_lis() {
        // [A,B,C,D,E] -> [E,B,C,D,A]: old-index sequence in new order is
        // [4,1,2,3,0]. The LIS is {1,2,3} (B,C,D) — exactly the 3 middle
        // elements that shouldn't need to move for a pure end-swap.
        assert_eq!(marked(&lis_keep(&vec![4, 1, 2, 3, 0], 5)), vec![1, 2, 3]);
    }

    #[test]
    fn full_reversal_lis_is_a_single_element() {
        // A full reversal has no increasing run longer than 1.
        let lis = marked(&lis_keep(&vec![4, 3, 2, 1, 0], 5));
        assert_eq!(lis.len(), 1);
    }

    /// **사전조건 P의 건전성 무작위 검사** — `lis_keep`이 표시한 슬롯들이 정말
    /// *증가* 부분수열인가.
    ///
    /// **이제 이것은 회귀 검사이지 유일한 근거가 아니다.** P는
    /// `lis_keep`의 Verus 사후조건(`take_order_preserved`)으로 **증명**돼 있고,
    /// 그 사후조건이 `plan_lis_from_selection`의 사전조건에 물리는 것까지
    /// `plan_lis_from_selection_auto` 안에서 기계검사된다. 이 테스트가 남아
    /// 있는 이유는 ghost가 지워진 **빌드 산출물**이 증명된 것과 같은 코드임을
    /// 값으로 한 번 더 확인하기 위해서다.
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
                let idx = marked(&lis_keep(&seq, n));
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
        // Prepending conceptually looks like old-indices [0,1,2,3] (fresh
        // slots carry `take_old >= old_len` and are skipped inside
        // `lis_keep`, so they never participate) — everything
        // already-existing stays in the LIS.
        let lis = marked(&lis_keep(&vec![0, 1, 2, 3], 4));
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

/// 후보 C — 접미사 없는 판본과 사이트별 monomorphize 판본.
/// 위 keyed 후보들의 매크로와 같은 구조이지만 C는 쿼리 구성이 달라 따로 쓴다
/// (슬롯 값 채널 `SlotValue`, 그리고 파생값 재계산에 필요한 `RowOwned`).
pub fn list_reconcile_c_system<T>(
    lists: Query<(&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget), Changed<Signal<Vec<T>>>>,
    slots: Query<(&mut SlotValue, &DomTarget)>,
    owned: Query<&crate::RowOwned>,
    variant_nodes: Query<&crate::RowVariantNode>,
    variant_targets: Query<&DomTarget>,
    commands: Commands,
    pending: ResMut<PendingDomWrites>,
    row_bindings: Res<crate::RowBindings>,
    variant_mode: Res<crate::RowVariantMode>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    c_body::<T, crate::DefaultRow, _>(
        &crate::NoRowCtx,
        lists,
        slots,
        owned,
        variant_nodes,
        variant_targets,
        commands,
        pending,
        row_bindings,
        variant_mode,
    );
}

pub fn list_reconcile_c_system_with<T, R: crate::RowSpawner>(
    lists: Query<
        (&Signal<Vec<T>>, &mut crate::ListRows, &DomTarget),
        (Changed<Signal<Vec<T>>>, With<R::Marker>),
    >,
    slots: Query<(&mut SlotValue, &DomTarget)>,
    owned: Query<&crate::RowOwned>,
    variant_nodes: Query<&crate::RowVariantNode>,
    variant_targets: Query<&DomTarget>,
    commands: Commands,
    pending: ResMut<PendingDomWrites>,
    row_bindings: Res<crate::RowBindings>,
    variant_mode: Res<crate::RowVariantMode>,
    site: Res<R::Ctx>,
) where
    T: Send + Sync + Clone + ToString + 'static,
{
    c_body::<T, R, _>(
        &site,
        lists,
        slots,
        owned,
        variant_nodes,
        variant_targets,
        commands,
        pending,
        row_bindings,
        variant_mode,
    );
}
