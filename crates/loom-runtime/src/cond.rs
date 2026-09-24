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

use crate::{DetachedFragment, DomTarget, DomWrite, PendingDomWrites, Signal};
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
/// 조건부 블록 자식 하나의 컴포넌트 번들 — **B arm과 R2 row의 spawn 시점 물질화가
/// 같은 것을 만들게** 한 곳에 둔다(2026-09-24). 두 벌로 두면 한쪽만 바뀌는 사고가 난다.
///
/// `ChildOf(블록)`: 블록이 despawn되면 자식도 함께 사라진다(2026-08-13 수리 —
/// R2로 조건부가 row 안에 들어가면서 "true인 채로 row와 함께 despawn"이 생겼다).
pub fn block_child_bundle(block: Entity) -> impl bevy_ecs::bundle::Bundle {
    (
        Signal(0_i32),
        Derived1::default(),
        Derived2::default(),
        Derived3::default(),
        ::bevy_ecs::hierarchy::ChildOf(block),
    )
}

pub fn cond_reconcile_b_system(
    mut q: Query<(Entity, &Signal<bool>, &mut CondBlock, &DomTarget), Changed<Signal<bool>>>,
    dom_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (anchor_entity, cond, mut block, anchor) in &mut q {
        if cond.0 && block.children.is_empty() {
            for _ in 0..block.size {
                let child = commands.spawn(block_child_bundle(anchor_entity)).id();
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
                    let child = commands
                    .spawn((
                        Signal(0_i32),
                        Derived1::default(),
                        Derived2::default(),
                        Derived3::default(),
                    ))
                    .id();
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

// ============================================================================
// RQ1 트랙 T — F 계열 3변형 (§4.1이 요구하는 6 arm 중 그동안 없던 것들)
// ============================================================================
//
// **왜 이제야**: 2026-07-28 트랙 T f축 캠페인이 §9 "또는" 분기를 실제로 적용해
// 미발동으로 확정하면서, 트랙 T 판정을 막는 장벽이 "하네스가 없다"에서
// **"arm이 없다"**로 좁혀졌다. §9의 F 기각 1차 분기는 F-i/ii/iii **각각**에
// 대해 판정하라고 요구하는데, `cond.rs`에는 B와 CSS 은닉 겸용 F 하나뿐이었다.
//
// **공통 코어(RQ1 문언)**: 토글-off 시 DOM을 **파괴하지 않고 fragment로 분리
// 보관**한다(`DomOps::detach_to_fragment`). entity와 내부 `Signal` 상태가
// 토글을 넘어 살아남는 것 — 이것이 B가 원리적으로 줄 수 없는 능력이고 F의
// 진짜 논거다. CSS 은닉과 다른 점: 은닉은 노드가 문서에 남아 형제 셀렉터·
// 레이아웃에 계속 참여하지만, 분리는 문서에서 완전히 빠진다.
//
// **세 변형이 갈리는 지점**은 "분리돼 있는 동안 patch System을 어떻게 재우는가"
// 하나뿐이다(§10 "F: 3변형 — 셋 다 구현·측정, marker-vs-value 비용 구조가
// 전제부터 실측 대상"):
//
// | 변형 | 게이팅 | catch-up (§10 "변형별 자기 명세") |
// |---|---|---|
// | F-i   | `CondEffective(bool)` 값 필터 — 상시 런타임 분기 | 재부착 시 전파 System 1회 |
// | F-ii  | 없음 — detached DOM에 그대로 계속 패치 | 불필요(멈춘 적이 없음) |
// | F-iii | `Without<Frozen>` marker 필터 — 토글당 archetype 이동 2회 | 버전 카운터 쌍, 불일치만 재패치 |
//
// **공통 난점(RQ1 확정)**: 꺼진 동안 스킵된 patch는 `Changed` 틱이 소모되어
// 재부착 시 자동 재실행되지 않는다 → catch-up이 **필수**이고 "reattach =
// opcode 1회" 주장은 그대로 성립하지 않는다. 아래 각 변형이 자기 방식으로
// 이걸 처리한다.


/// F-i의 값 필터. 서브트리의 **모든** entity에 존재해야 하므로 토글 1회 =
/// O(서브트리) 쓰기 — RQ1이 F-i의 비용으로 미리 지목한 바로 그 지점이다.
#[derive(Component, Debug, Clone, Copy)]
pub struct CondEffective(pub bool);

/// F-iii의 marker. 붙였다 뗄 때마다 archetype 이동이 일어난다(토글당 2회).
#[derive(Component, Debug, Clone, Copy)]
pub struct Frozen;

/// F-iii의 catch-up용 버전 쌍. `applied`가 `current`보다 뒤처져 있으면
/// 해동 시 그 binding만 재패치한다(Adapton cleaning의 정수 재유도).
///
/// **site별 분리인 이유**(RQ1 확정): 버전을 공유 `DomBinding` 타입에 넣으면
/// 모든 patch System이 그 타입의 writer가 되어 patch 병렬성이 매 프레임
/// 붕괴한다. 그래서 binding site마다 별도 Component를 둔다.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct BindingVersion {
    /// 이 binding이 마지막으로 DOM에 반영한 값의 버전.
    pub applied: u32,
    /// 상류 Signal의 현재 버전.
    pub current: u32,
}


/// F 계열이 공유하는 spawn/detach/attach 골격. 세 변형의 차이는 **게이팅
/// 방식**뿐이므로, 첫 true의 lazy spawn과 이후의 분리/재부착은 여기 한 곳에
/// 모은다(RQ1 "despawn은 폐기가 아니라 승격 계층으로 강등: SSR 시점 false인
/// 블록은 entity 미탄생으로 두고 첫 true에 lazy spawn" — 덕분에 초기 wake-up
/// 페이로드가 B와 사실상 같다).
///
/// 반환값: `Some(true)` = 이번 tick에 재부착됨(= catch-up이 필요한 시점),
/// `Some(false)` = 분리됨, `None` = 변화 없음(또는 첫 spawn).
fn f_common_toggle(
    cond: bool,
    block_entity: Entity,
    block: &mut CondBlock,
    anchor: &DomTarget,
    detached: bool,
    commands: &mut Commands,
    pending: &mut PendingDomWrites,
) -> Option<bool> {
    if block.children.is_empty() {
        if cond {
            // 첫 true — lazy spawn. 이 시점의 페이로드는 B와 동일하다.
            for _ in 0..block.size {
                let child = commands
                    .spawn((
                        Signal(0_i32),
                        CondEffective(true),
                        // F-iii의 catch-up(버전 쌍)은 이 컴포넌트가 자식에
                        // 붙어 있어야 동작한다. 붙이지 않으면
                        // `bump_version_system`의 쿼리가 아무것도 매치하지
                        // 못해 F-iii의 catch-up 경로가 **조용히 죽는다** —
                        // 실제로 그동안 그 상태였고, 하네스의 known_gaps에도
                        // 그렇게 적혀 있었다(2026-08-02 수리).
                        BindingVersion::default(),
                        // §4.1 서브트리 구성 축용. 체인 System을 등록하지
                        // 않은 셀에서는 그냥 놀고 있는 POD 3개다 — 전 arm에
                        // 동일하게 붙으므로 축 간 비교에 편향을 주지 않는다.
                        Derived1::default(),
                        Derived2::default(),
                        Derived3::default(),
                    ))
                    .id();
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
        return None;
    }
    if cond && detached {
        pending.0.push(DomWrite::AttachFromFragment { block: block_entity, parent: anchor.0 });
        Some(true)
    } else if !cond && !detached {
        pending.0.push(DomWrite::DetachToFragment { block: block_entity, node: anchor.0 });
        Some(false)
    } else {
        None
    }
}

/// **F-i — 값 필터(`CondEffective`)**. 분리/재부착 시 서브트리의 **모든**
/// entity에 `CondEffective`를 다시 쓴다(토글 1회 = O(서브트리) 쓰기).
/// catch-up은 §10 명세대로 "재부착 시 전파 System 1회" — 재부착 시
/// `Signal`을 건드려 `Changed`를 다시 세워, 꺼진 동안 스킵된 patch가 다음
/// tick에 재실행되게 한다.
pub fn cond_reconcile_f_i_system(
    mut q: Query<(Entity, &Signal<bool>, &mut CondBlock, &DomTarget, Option<&DetachedFragment>), Changed<Signal<bool>>>,
    mut effective: Query<&mut CondEffective>,
    mut signals: Query<&mut Signal<i32>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (e, cond, mut block, anchor, frag) in &mut q {
        let transition = f_common_toggle(
            cond.0, e, &mut block, anchor, frag.is_some(), &mut commands, &mut pending,
        );
        let Some(reattached) = transition else { continue };
        // 값 필터 갱신 — 이게 F-i의 정의적 비용이다.
        for &child in &block.children {
            if let Ok(mut ce) = effective.get_mut(child) {
                ce.0 = cond.0;
            }
        }
        if reattached {
            // catch-up: 스킵된 patch의 Changed 틱이 이미 소모됐으므로,
            // 재부착 시 전파 System 1회로 다시 세운다.
            for &child in &block.children {
                if let Ok(mut sig) = signals.get_mut(child) {
                    sig.set_changed();
                }
            }
        }
    }
}

/// **F-ii — 게이팅 없음**. 분리된 DOM에 그대로 계속 패치한다("연결 안 된 DOM
/// 쓰기는 저렴하다"는 가설의 실측 대상 — 죽은 작업이 누적되는 위험도 함께).
/// catch-by 필요 없음: 멈춘 적이 없으므로 재부착 시 이미 최신이다.
pub fn cond_reconcile_f_ii_system(
    mut q: Query<(Entity, &Signal<bool>, &mut CondBlock, &DomTarget, Option<&DetachedFragment>), Changed<Signal<bool>>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (e, cond, mut block, anchor, frag) in &mut q {
        f_common_toggle(
            cond.0, e, &mut block, anchor, frag.is_some(), &mut commands, &mut pending,
        );
    }
}

/// **F-iii — `Without<Frozen>` marker**. 토글마다 서브트리 전 entity에
/// marker를 붙였다/뗀다 → **archetype 이동 2회/토글**(RQ1이 지목한 F-iii의
/// 비용). catch-up은 버전 카운터 쌍: 동결 중 상류가 바뀌면 `current`만
/// 올라가고, 해동 시 `applied != current`인 binding만 재패치한다.
pub fn cond_reconcile_f_iii_system(
    mut q: Query<(Entity, &Signal<bool>, &mut CondBlock, &DomTarget, Option<&DetachedFragment>), Changed<Signal<bool>>>,
    mut versions: Query<&mut BindingVersion>,
    mut signals: Query<&mut Signal<i32>>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (e, cond, mut block, anchor, frag) in &mut q {
        let transition = f_common_toggle(
            cond.0, e, &mut block, anchor, frag.is_some(), &mut commands, &mut pending,
        );
        let Some(reattached) = transition else { continue };
        for &child in &block.children {
            if cond.0 {
                commands.entity(child).remove::<Frozen>();
            } else {
                commands.entity(child).insert(Frozen);
            }
        }
        if reattached {
            // catch-up: 버전 불일치가 있는 binding만 다시 세운다 — F-i가
            // 전량을 다시 세우는 것과 대비되는 지점(그게 이 변형의 논거).
            for &child in &block.children {
                let stale = versions
                    .get(child)
                    .map(|v| v.applied != v.current)
                    .unwrap_or(false);
                if stale {
                    if let Ok(mut sig) = signals.get_mut(child) {
                        sig.set_changed();
                    }
                    if let Ok(mut v) = versions.get_mut(child) {
                        v.applied = v.current;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 게이팅된 patch System — 세 변형의 비용 차이가 **실제로 드러나는** 지점
// ---------------------------------------------------------------------------
//
// 위 reconcile System들은 토글 순간만 다룬다. 세 변형이 갈리는 진짜 지점은
// **분리돼 있는 동안 상류 Signal이 계속 바뀔 때** 각자가 무엇을 하느냐다
// (§4.1의 "동결 중 상류 Signal 변경률" 축). 그 축이 없으면 세 변형의 J-b가
// 사실상 같아진다 — 2026-07-28 f축 스모크런에서 실제로 그렇게 관측됐다.

/// F-i용: `CondEffective(true)`인 entity만 DOM에 쓴다. 필터가 **상시 런타임
/// 분기**이므로 분리 중에도 쿼리는 매 프레임 돌고 조건만 거른다 — 이것이
/// RQ1이 F-i의 "게이팅 상주 비용"으로 지목한 것이다.
pub fn patch_text_f_i_system(
    q: Query<(&Signal<i32>, &DomTarget, &CondEffective), Changed<Signal<i32>>>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (signal, target, eff) in &q {
        if eff.0 {
            pending.0.push(DomWrite::SetText { target: target.0, text: signal.0.to_string() });
        }
    }
}

/// F-ii용: 게이팅 없음 — 분리된 DOM에도 그대로 쓴다. "연결 안 된 DOM 쓰기는
/// 저렴하다"는 가설이 여기서 실측된다(죽은 작업 누적이 대가).
pub fn patch_text_f_ii_system(
    q: Query<(&Signal<i32>, &DomTarget), Changed<Signal<i32>>>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (signal, target) in &q {
        pending.0.push(DomWrite::SetText { target: target.0, text: signal.0.to_string() });
    }
}

/// F-iii용: `Without<Frozen>` — 동결된 entity는 **쿼리 매칭 단계에서** 빠지므로
/// 상시 분기가 없다(그 대신 토글마다 archetype 이동을 낸다). 동결 중 상류가
/// 바뀌면 아래 `bump_version_system`이 `current`만 올려 두고, 해동 시
/// `cond_reconcile_f_iii_system`이 불일치분만 재패치한다.
pub fn patch_text_f_iii_system(
    q: Query<(&Signal<i32>, &DomTarget), (Changed<Signal<i32>>, Without<Frozen>)>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (signal, target) in &q {
        pending.0.push(DomWrite::SetText { target: target.0, text: signal.0.to_string() });
    }
}

/// F-iii의 동결 중 dirty 추적: 동결된 entity의 Signal이 바뀌면 `current`만
/// 올린다(DOM은 건드리지 않음). 해동 시 `applied != current`인 것만 재패치되어
/// **전량 재패치(F-i)보다 적게 일하는 것**이 이 변형의 논거다.
pub fn bump_version_system(
    mut q: Query<&mut BindingVersion, (Changed<Signal<i32>>, With<Frozen>)>,
) {
    for mut v in &mut q {
        v.current = v.current.wrapping_add(1);
    }
}

// ============================================================================
// §4.1 서브트리 구성 축 — 파생/계산 체인 깊이 3
// ============================================================================
//
// §4.1: "**서브트리 구성 축**: flat binding형 vs 파생/계산 체인 깊이 3
// 포함형 — s=100 행에서만 양쪽." 2026-08-02까지 하네스에는 flat 쪽만 있었다.
//
// **왜 단계를 갈라 두는가**: 한 System 안에서 세 번 계산하면 "체인 깊이 3"의
// 스케줄링 비용이 사라진다. 이 축이 재려는 것이 바로 그 비용이다 — F-i는
// 값 필터를 **단계마다** 확인해야 하고(런타임 분기 ×3), F-iii는 archetype
// 필터라 단계 수와 무관하게 공짜인 반면 marker가 세 단계의 쿼리에 전부
// 걸려야 한다. 즉 깊이가 F 변형들의 게이팅 비용 구조를 갈라 보이게 하는
// 축이고, 그러려면 단계가 실제로 별개 System이어야 한다.
//
// 컴포넌트는 전부 POD(i32)다 — CLAUDE.md 금지 4.

/// 파생 체인 1단: `Signal<i32>` → 이것.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Derived1(pub i32);
/// 파생 체인 2단.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Derived2(pub i32);
/// 파생 체인 3단 — DOM 텍스트는 이 값에서 나온다.
#[derive(Component, Debug, Clone, Copy, Default)]
pub struct Derived3(pub i32);

/// 각 단의 계산. 내용 자체는 중요하지 않고(상수 시간), 중요한 것은 단계가
/// **실제로 별개 System**이라 `Changed` 전파가 세 번 일어난다는 점이다.
#[inline]
fn derive_step(x: i32) -> i32 {
    x.wrapping_mul(3).wrapping_add(1)
}

// --- F-i: 값 필터(`CondEffective`)를 단계마다 확인 ---------------------------

pub fn derive1_f_i_system(
    mut q: Query<(&Signal<i32>, &mut Derived1, &CondEffective), Changed<Signal<i32>>>,
) {
    for (s, mut d, eff) in &mut q {
        if eff.0 {
            d.0 = derive_step(s.0);
        }
    }
}

pub fn derive2_f_i_system(
    mut q: Query<(&Derived1, &mut Derived2, &CondEffective), Changed<Derived1>>,
) {
    for (a, mut b, eff) in &mut q {
        if eff.0 {
            b.0 = derive_step(a.0);
        }
    }
}

pub fn derive3_f_i_system(
    mut q: Query<(&Derived2, &mut Derived3, &CondEffective), Changed<Derived2>>,
) {
    for (a, mut b, eff) in &mut q {
        if eff.0 {
            b.0 = derive_step(a.0);
        }
    }
}

/// 체인형에서의 F-i 패치 — 텍스트는 `Derived3`에서 나온다.
pub fn patch_chain_f_i_system(
    q: Query<(&Derived3, &DomTarget, &CondEffective), Changed<Derived3>>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (d, target, eff) in &q {
        if eff.0 {
            pending.0.push(DomWrite::SetText { target: target.0, text: d.0.to_string() });
        }
    }
}

// --- F-ii: 게이팅 없음 -------------------------------------------------------

pub fn derive1_f_ii_system(mut q: Query<(&Signal<i32>, &mut Derived1), Changed<Signal<i32>>>) {
    for (s, mut d) in &mut q {
        d.0 = derive_step(s.0);
    }
}

pub fn derive2_f_ii_system(mut q: Query<(&Derived1, &mut Derived2), Changed<Derived1>>) {
    for (a, mut b) in &mut q {
        b.0 = derive_step(a.0);
    }
}

pub fn derive3_f_ii_system(mut q: Query<(&Derived2, &mut Derived3), Changed<Derived2>>) {
    for (a, mut b) in &mut q {
        b.0 = derive_step(a.0);
    }
}

pub fn patch_chain_f_ii_system(
    q: Query<(&Derived3, &DomTarget), Changed<Derived3>>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (d, target) in &q {
        pending.0.push(DomWrite::SetText { target: target.0, text: d.0.to_string() });
    }
}

// --- F-iii: `Without<Frozen>` archetype 필터 ---------------------------------

pub fn derive1_f_iii_system(
    mut q: Query<(&Signal<i32>, &mut Derived1), (Changed<Signal<i32>>, Without<Frozen>)>,
) {
    for (s, mut d) in &mut q {
        d.0 = derive_step(s.0);
    }
}

pub fn derive2_f_iii_system(
    mut q: Query<(&Derived1, &mut Derived2), (Changed<Derived1>, Without<Frozen>)>,
) {
    for (a, mut b) in &mut q {
        b.0 = derive_step(a.0);
    }
}

pub fn derive3_f_iii_system(
    mut q: Query<(&Derived2, &mut Derived3), (Changed<Derived2>, Without<Frozen>)>,
) {
    for (a, mut b) in &mut q {
        b.0 = derive_step(a.0);
    }
}

pub fn patch_chain_f_iii_system(
    q: Query<(&Derived3, &DomTarget), (Changed<Derived3>, Without<Frozen>)>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (d, target) in &q {
        pending.0.push(DomWrite::SetText { target: target.0, text: d.0.to_string() });
    }
}

// --- B / CSS 은닉: 게이팅 자체가 없는 arm들 ---------------------------------

/// B와 CSS 은닉은 게이팅 기계장치가 없으므로 F-ii와 같은 무게이팅 체인을
/// 쓴다. (B는 숨을 때 entity 자체가 사라지므로 체인도 함께 사라진다 —
/// 그것이 B의 "기계장치 0"이 성립하는 이유이고, 이 축에서도 마찬가지다.)
pub fn patch_chain_plain_system(
    q: Query<(&Derived3, &DomTarget), Changed<Derived3>>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (d, target) in &q {
        pending.0.push(DomWrite::SetText { target: target.0, text: d.0.to_string() });
    }
}
