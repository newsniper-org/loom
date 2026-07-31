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

use bevy_ecs::{prelude::*, system::ScheduleSystem};
use bevy_ecs::schedule::Schedule;

pub mod list;
pub use list::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, OrderKey, OrderKeyMembers, OrderedKey,
    RowKey, SlotValue,
};

pub mod stage;
pub use stage::{now_ms, StageTimings};

pub mod counting;
pub use counting::{CountingDom, DomOpCounts};

pub mod cond;
pub use cond::{
    bump_version_system, cond_reconcile_b_system, cond_reconcile_f_i_system,
    cond_reconcile_f_ii_system, cond_reconcile_f_iii_system, cond_reconcile_f_system,
    patch_text_f_i_system, patch_text_f_ii_system, patch_text_f_iii_system, BindingVersion,
    CondBlock, CondEffective, Frozen,
};

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

/// Host-agnostic access to RQ4's cross-island shared store (Task #28-a,
/// pulled forward for Task #26's CartBadge scenario) — the same
/// "generated code stays host-agnostic, a host adapter implements the
/// trait" shape `DomOps`/`BrowserDom` already established. Kept minimal
/// (Phase 1 scope: i32 values only, matching `__loomShared`'s current
/// JS-side signature) rather than pre-guessing a richer future shape.
pub trait SharedStore {
    fn set(&mut self, key: &str, value: i32);
    fn get(&mut self, key: &str) -> Option<i32>;
}

/// A `SharedStore` any native (non-wasm) caller can use — SSR (no real
/// `window.__loomShared` exists there) and native tests. The `set()` call
/// has no effect on SSR'd HTML either way (it's a side channel to an
/// external store, not part of the DOM tree being built), so a plain
/// in-memory map is sufficient; it exists only to satisfy the type
/// signature, not to model real cross-widget behavior (that only happens
/// in a real browser via `web::JsSharedStore`).
#[derive(Debug, Default)]
pub struct InMemorySharedStore(std::collections::HashMap<String, i32>);

impl SharedStore for InMemorySharedStore {
    fn set(&mut self, key: &str, value: i32) {
        self.0.insert(key.to_string(), value);
    }

    fn get(&mut self, key: &str) -> Option<i32> {
        self.0.get(key).copied()
    }
}

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
    /// Inserts a **whole pre-rendered static subtree** as the last child(ren)
    /// of `parent` in one call, given its final HTML verbatim — `html` is
    /// not escaped again here, it must already be well-formed markup (the
    /// caller, `codegen.rs`, builds it at compile time from a subtree that
    /// has no dynamic binding anywhere in it, escaping each static text run
    /// exactly once at that point).
    ///
    /// **Why this exists (docs/02's original design, actually implemented
    /// now)**: docs/02 always said purely static markup should be "SSR용
    /// HTML 템플릿 문자열로 직접 방출" — emitted directly as a template
    /// string, not walked node-by-node at runtime. Until 2026-07-24, Phase
    /// 1's actual codegen didn't do this: every static `<tag>`/text run
    /// went through its own `create_element`/`append_static_text` call, the
    /// same one-node-at-a-time path dynamic bindings use. That's a real,
    /// previously-undocumented gap from the stated design, not a No-VDOM
    /// violation (nothing was ever diffed — see docs/03's No-VDOM
    /// criteria), but it meant a page with N static nodes paid N runtime
    /// calls to render, once per SSR, exactly like every other framework's
    /// naive path. `append_static_html` is the fix: `codegen.rs` now
    /// collapses any maximal fully-static subtree into one HTML string
    /// *at compile time* and emits a single call here — turning "N calls,
    /// O(n) runtime work" into "1 call" regardless of how large that
    /// static subtree is (see `research-questions/data/rq6-m11-isr-partial-regen.json`
    /// for the measured effect this has on ISR-style regeneration cost).
    fn append_static_html(&mut self, parent: NodeHandle, html: &str);
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
    /// RQ1 Track T **F 계열 공통 코어**: `node`의 **자식들**(= 조건부 서브트리)을
    /// 떼어 DocumentFragment로 옮겨 보관한다. `node` 자신은 앵커로 문서에
    /// 남는다 — 통째로 옮기면 재부착 시 자기 자신을 자기 안에 넣는 계층 오류가
    /// 난다. CSS 은닉(`set_visible`)과 결정적으로 다른 점 — 은닉은
    /// 노드가 문서에 **남아 있어** 형제 셀렉터(`:nth-child`)·레이아웃 계산·SSR
    /// 바이트에 계속 참여하지만, 분리는 문서에서 완전히 빠진다. RQ1이 F의
    /// 정의적 특성으로 규정한 "DOM은 파괴하지 않고 fragment로 분리 보관"이
    /// 이것이다(entity와 내부 Signal 상태는 그대로 살아 있다).
    ///
    /// fragment 핸들은 **비직렬화 사이드테이블**에 두고 Component에는 정수만
    /// 남긴다(RQ1 "F 공통 코어", CLAUDE.md 금지 4와도 정합).
    ///
    /// **기본 구현이 있는 이유**: 이 저장소의 `DomOps` 구현 5개 중 4개는 SSR
    /// 전용 `StringDom`이고, SSR은 조건부를 **토글하지 않는다**(초기 상태를
    /// 한 번 그릴 뿐). 그쪽에 의미 없는 코드를 넣는 대신 호출되면 즉시
    /// 실패하게 두어, 만약 SSR 경로에서 불린다면 조용히 잘못 동작하지 않고
    /// 바로 드러나게 한다.
    fn detach_to_fragment(&mut self, _node: NodeHandle) -> FragmentHandle {
        unimplemented!("detach_to_fragment: 이 DomOps 구현은 F 계열 토글을 지원하지 않는다(SSR 전용)")
    }
    /// 분리해 둔 노드를 `parent`의 마지막 자식으로 되돌린다. 기본 구현의
    /// 이유는 `detach_to_fragment`와 같다.
    fn attach_from_fragment(&mut self, _parent: NodeHandle, _fragment: FragmentHandle) {
        unimplemented!("attach_from_fragment: 이 DomOps 구현은 F 계열 토글을 지원하지 않는다(SSR 전용)")
    }
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
    /// R2 축: 이미 만들어진 파생 binding들의 텍스트를 **다시 쓴다**. 슬롯의
    /// 아이템이 바뀌면 그 아이템에서 파생된 binding이 전부 바뀌므로, 후보 C의
    /// 재정렬(= 슬롯 값 교체)은 이 write를 row마다 한 번씩 내게 된다 —
    /// 그것이 §9 C 기각 조항이 겨냥하는 비용이다. keyed 후보(D/E)는 retained
    /// row의 아이템이 바뀌지 않으므로 이 write를 내지 않는다(그 비대칭이 곧
    /// RQ1의 예측).
    SetRowBindingTexts { row: Entity, texts: Vec<String> },
    /// RQ1 Track T F 계열: 노드를 fragment로 분리한다. 결과 핸들은
    /// `LoomApp::tick`이 `FragmentTable`에 적재해 `CondBlock`이 정수로만
    /// 참조하게 한다.
    DetachToFragment { block: Entity, node: NodeHandle },
    /// 분리해 둔 것을 되돌린다.
    AttachFromFragment { block: Entity, parent: NodeHandle },
    /// Compiler-generated `<input>` two-way binding's reactive write (see
    /// `patch_input_system`). Carries an already-resolved `NodeHandle`, same
    /// reason as `AppendStaticText` — an input element is created directly
    /// via `DomOps::create_element` in `setup()`/`resume()`, never as part
    /// of a list/conditional spawn batch that would need `Entity`
    /// resolution.
    SetInputValue { node: NodeHandle, value: String },
}

/// `detach_to_fragment`가 돌려주는 보관함 식별자. `NodeHandle`과 같은 정수
/// 스킴이지만 **다른 테이블**을 가리키므로 타입을 분리한다 — 섞으면 분리된
/// 노드를 살아 있는 노드처럼 조작하게 된다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FragmentHandle(pub u32);

/// 분리 보관 중인 조건부 블록이 자기 fragment를 가리키는 **정수 하나**.
/// RQ1 "F 공통 코어"가 요구한 그대로 — 실제 `DocumentFragment`는 `BrowserDom`의
/// 사이드테이블에만 있고 Component에는 인덱스만 들어간다(CLAUDE.md 금지 4).
/// 이 Component의 존재 자체가 "지금 분리돼 있다"는 뜻이다.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetachedFragment(pub FragmentHandle);

/// RQ1 row 깊이 축(R2)의 추가 binding들이 붙은 DOM 노드 핸들. **2026-07-28
/// 신설** — 그 전까지 추가 binding은 핸들 없는 정적 텍스트 노드라 한 번
/// 만들어지면 다시 쓸 수 없었고, 그 때문에 §9 C 기각 조항이 겨냥하는
/// "O(n×binding) retained-slot value-patch" 비용이 계측에 존재하지 않았다
/// (상세는 RQ1의 "C 기각 조건 재검토" 절). 핸들을 들고 있어야 슬롯 값이
/// 바뀔 때 파생 binding을 재기록할 수 있다.
#[derive(Component, Debug, Clone, Default)]
pub struct RowBindingTargets(pub Vec<NodeHandle>);

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

/// RQ1 §3.2의 **R3 "이질 브랜치 row"** 축: 켜면 각 row가 자기 **값에 따라**
/// 두 구조 중 하나(`<A/>`/`<B/>`)를 갖는다. 꺼져 있으면(기본) R1/R2와 동일.
///
/// §10이 전환형을 **"슬롯 entity 컴포넌트 교체(archetype move)형"으로 사전
/// 고정**했으므로, 브랜치가 바뀌면 marker Component를 교체한다 — 그 교체가
/// Commands를 통해 sync point에서 적용되며 archetype 이동을 일으킨다.
/// §9 C 기각 (ii)가 재는 "archetype move 구간"이 정확히 그 구간이다(§7.1).
#[derive(Resource, Clone, Copy, Default)]
pub struct RowVariantMode(pub bool);

/// R3의 브랜치 A/B marker. 값에서 파생되므로 슬롯의 아이템이 바뀌면 바뀔 수
/// 있고, 그때 교체가 일어난다(keyed 후보는 retained row의 아이템이 안 바뀌므로
/// 교체가 없다 — 그 비대칭이 RQ1의 "재정렬마다 archetype move 비용" 예측).
#[derive(Component, Debug, Clone, Copy)]
pub struct RowVariantA;

#[derive(Component, Debug, Clone, Copy)]
pub struct RowVariantB;

/// 값 → 브랜치. 결정적이어야 시드 고정 하에 M2가 결정적 수치가 된다(§7 M2).
pub(crate) fn row_variant_is_a(text: &str) -> bool {
    text.as_bytes().last().map(|b| b % 2 == 0).unwrap_or(true)
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
        world.init_resource::<RowVariantMode>();
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

    /// RQ1 §3.2의 R3 축을 켠다(기본 꺼짐 = R1/R2 동작 무변화).
    pub fn set_row_variant_mode(&mut self, on: bool) -> &mut Self {
        self.world.insert_resource(RowVariantMode(on));
        self
    }

    pub fn add_systems<M>(&mut self, systems: impl IntoScheduleConfigs<ScheduleSystem, M>) -> &mut Self {
        self.schedule.add_systems(systems);
        self
    }

    /// `add_systems`와 같되, RQ1-benchmark-protocol.md **§7.1의 단계 귀속
    /// 계측**을 함께 건다 — `systems` 앞뒤에 마커를 두고 그 뒤에
    /// `ApplyDeferred`를 명시적으로 끼워, "후보 System 실행 구간"과
    /// "Commands apply(sync point) 구간"을 분리 측정할 수 있게 만든다
    /// (`stage.rs` 참고). 결과는 `StageTimings` Resource에 매 tick 덮어써진다.
    ///
    /// 후보 코드는 **한 줄도 건드리지 않는다** — §7.1이 명시한 "하네스가
    /// System/스테이지 경계에서 타임스탬프를 찍는 방식(후보 코드 무침습)"
    /// 그대로다. 계측이 필요 없는 앱(데모/컴파일러 산출물 전부)은 계속
    /// `add_systems`를 쓰면 되고, 그쪽에는 `StageTimings` Resource 자체가
    /// 존재하지 않는다.
    pub fn add_instrumented_systems<M>(
        &mut self,
        systems: impl IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> &mut Self {
        self.world.init_resource::<StageTimings>();
        self.schedule.add_systems(
            (
                stage::stage_mark_systems_start,
                systems,
                stage::stage_mark_systems_end,
                ApplyDeferred,
                stage::stage_mark_sync_end,
            )
                // **`chain()`이 아니라 `chain_ignore_deferred()`인 이유(실제로
                // 틀렸다가 프로브로 잡아냄)**: Bevy는 `chain()`으로 순서를 걸면
                // deferred 버퍼를 가진 System과 그 의존 System 사이에 sync point를
                // **자동 삽입**한다. 그러면 Commands가 우리 `ApplyDeferred`보다
                // **먼저** 적용되어, archetype move 비용이 `sync_point_ms`가 아니라
                // `systems_ms`에 잡힌다 — §7.1이 정의한 구간과 어긋난다.
                // `chain_ignore_deferred()`는 순서만 걸고 자동 삽입은 하지 않으므로
                // 아래 명시적 `ApplyDeferred`가 유일한 sync point가 된다.
                .chain_ignore_deferred(),
        );
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
        // §7.1의 "DOM 배치 적용 구간"은 스케줄 **바깥**이라 마커 System으로는
        // 잴 수 없다 — 여기서 직접 스탬프한다. `StageTimings`가 없는 앱
        // (= `add_instrumented_systems`를 안 쓴 전부)에서는 아래 저장이
        // 통째로 생략되므로, 남는 비용은 시계 호출 3회(~100ns)뿐이다.
        let t0 = stage::now_ms();
        self.schedule.run(&mut self.world);
        let t_schedule_end = stage::now_ms();
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
                    // 2026-07-28: `append_static_text`(핸들 없는 텍스트 노드)
                    // 대신 추적되는 엘리먼트를 만든다 — 그래야 슬롯 값이 바뀔 때
                    // 재기록할 수 있고, R2가 "binding 8개"를 실제로 모델링하게
                    // 된다(binding site = 동적 텍스트를 갖는 엘리먼트).
                    let mut targets = Vec::with_capacity(texts.len());
                    for text in &texts {
                        let child = dom.create_element(handle, "span", None);
                        dom.set_text_content(child, text);
                        targets.push(child);
                    }
                    self.world.entity_mut(row).insert(RowBindingTargets(targets));
                }
                DomWrite::SetRowBindingTexts { row, texts } => {
                    let Some(targets) = self.world.get::<RowBindingTargets>(row).cloned() else {
                        continue;
                    };
                    for (handle, text) in targets.0.iter().zip(texts.iter()) {
                        dom.set_text_content(*handle, text);
                    }
                }
                DomWrite::DetachToFragment { block, node } => {
                    let handle = dom.detach_to_fragment(node);
                    self.world.entity_mut(block).insert(DetachedFragment(handle));
                }
                DomWrite::AttachFromFragment { block, parent } => {
                    let Some(frag) = self.world.get::<DetachedFragment>(block).copied() else {
                        continue;
                    };
                    dom.attach_from_fragment(parent, frag.0);
                    self.world.entity_mut(block).remove::<DetachedFragment>();
                }
                DomWrite::AppendStaticText { parent, text } => dom.append_static_text(parent, &text),
                DomWrite::SetInputValue { node, value } => dom.set_input_value(node, &value),
            }
        }
        let t_end = stage::now_ms();
        if let Some(mut timings) = self.world.get_resource_mut::<StageTimings>() {
            timings.dom_apply_ms = t_end - t_schedule_end;
            timings.total_ms = t_end - t0;
        }
    }

    fn dom_target(&self, entity: Entity) -> Option<NodeHandle> {
        self.world.get::<DomTarget>(entity).map(|t| t.0)
    }
}
