//! loom-runtime — Layer 2 reactive core (docs/02, docs/03).
//!
//! Phase 1 scope (roadmap/PHASES.md): Island/Resumability/backend-agnostic
//! excluded. Single World, client=server same process.
//!
//! **2026-10-17 정정**: 위 문단은 Phase 1 착수 시점의 스코프이고 지금 이
//! 크레이트의 범위가 아니다. 그 뒤로 같은 크레이트에 들어온 것:
//! Resumability 경로(`DomOps::adopt_children`/`get_input_value` 등 resume용
//! 연산, Phase 2, 2026-07-24), `<input>` 양방향 binding(`InputValueTarget`,
//! 2026-07-24), Island supervision 상태 기계(`island.rs`, Phase 3)와
//! `SharedStore`(Task #28-a), RQ1 트랙 L 후보(`list.rs`, K/C/D/E/D-LIS/E-LIS)·
//! 트랙 T 후보(`cond.rs`, B/F/F-i/F-ii/F-iii)·§7.1 단계 계측(`stage.rs`)·
//! §7 M2 DOM 연산 계수(`counting.rs`). "Single World"는 RQ6 H-Qwik 확정
//! (2026-07-28) 뒤 오히려 **기본 모델** 그 자체가 됐다(docs/04). 단
//! "client=server same process"는 네이티브 테스트 경로에만 맞는 말이다 —
//! 브라우저 경로는 `browser` feature의 `web.rs`.
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
//!
//! **2026-10-17 정정**: 위 제목의 "deferred to Phase 2"와 "becomes necessary
//! again once Phase 2 introduces LRef"라는 예측은 실현되지 않았다. Phase 2
//! (2026-07-23~24)는 LRef를 넣었지만 opcode 형태의 `DomBinding`/PatchTable은
//! 만들지 않았다 — 직렬화 경계를 넘는 "행동"은 이벤트 action뿐이었고, 그것은
//! 컴파일러가 생성하는 `Action` 열거형 + `resume_action_by_symbol`의
//! `match symbol { ... }`(codegen.rs)로 풀렸다. patch 쪽 디스패치는 지금도
//! 제네릭 System(`patch_text_system::<T>` 등) 그대로다. 그래서 2026-10-17
//! 현재 PatchTable은 **미구현**이고 전환 결정 기록도 없다(docs/07 PatchTable
//! 행, roadmap/PHASES.md 참고). 금지 4(Component에 클로저/함수 포인터 없음)는
//! 이 방식으로도 지켜진다.

use bevy_ecs::{prelude::*, system::ScheduleSystem};
use bevy_ecs::schedule::Schedule;

pub mod list;
pub use list::{
    list_reconcile_c_system, list_reconcile_c_system_with, list_reconcile_d_lis_system, list_reconcile_d_lis_system_with,
    list_reconcile_d_lis_unconditional_system, list_reconcile_d_lis_unconditional_system_with,
    list_reconcile_d_system, list_reconcile_d_system_with, list_reconcile_d_unconditional_system,
    list_reconcile_d_unconditional_system_with, list_reconcile_e_lis_system,
    list_reconcile_e_lis_system_with, list_reconcile_e_system, list_reconcile_e_system_with,
    OrderKey, OrderKeyMembers, SlotValue,
};
pub use list_key::{row_key, OrderedKey, RowKey, RowValue};

/// `add_row_patch_window`가 심는 표식 — 창 중복 등록을 막는다.
#[derive(Resource)]
pub struct RowPatchWindow;

pub mod stage;
pub use stage::{now_ms, StageTimings};

pub mod counting;
pub use counting::{CountingDom, DomOpCounts, SurfaceOpCounts};

pub mod cond;
pub use cond::{
    bump_version_system, cond_reconcile_b_system, cond_reconcile_f_i_system,
    cond_reconcile_f_ii_system, cond_reconcile_f_iii_system, cond_reconcile_f_system,
    patch_text_f_i_system, patch_text_f_ii_system, patch_text_f_iii_system, BindingVersion,
    CondBlock, CondEffective, Frozen,
    // §4.1 서브트리 구성 축 — 파생/계산 체인 깊이 3
    derive1_f_i_system, derive1_f_ii_system, derive1_f_iii_system,
    derive2_f_i_system, derive2_f_ii_system, derive2_f_iii_system,
    derive3_f_i_system, derive3_f_ii_system, derive3_f_iii_system,
    patch_chain_f_i_system, patch_chain_f_ii_system, patch_chain_f_iii_system,
    patch_chain_plain_system, Derived1, Derived2, Derived3,
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
///
/// **2026-10-17 정정(지위)**: RQ4는 2026-07-28 전면 폐기됐다(RQ6 H-Qwik 확정).
/// RQ4 문서 머리말은 기본 모델(하나의 World)에서 위젯 간 공유가 같은 World의
/// 평범한 Signal로 풀린다고 적는다. 다만 코드는 그 서술보다 넓다: 2026-10-17
/// 현재 컴파일러는 소스가 `sharedSignal`/`sharedValue`를 쓰면
/// `GraphMode::Flatten` 진입점에도 이 trait(`web::JsSharedStore`)를 그대로
/// 통과시킨다(codegen.rs `generate_flatten_entry_body`의 `has_shared`). 즉
/// 이 trait는 지금도 두 GraphMode 모두에서 쓰이며, Flatten에서도 이것이 필요한지
/// 정리한 결정 기록은 찾지 못했다.
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
    ///
    /// **2026-10-26 정정(7c)**: 위 "replaces whatever children it had"는 2026-08-07 이후 사실이 아니다 —
    /// `BrowserDom`/`MockDom`의 계약은 **첫 텍스트 노드만 제자리에서 바꾸고, 없으면 맨 앞에 하나 만들며, 엘리먼트
    /// 자식은 건드리지 않는다**(`web.rs`의 `set_text_content` 주석: R2 row의 binding span이 전멸하던 결함의 수리).
    /// 위 서술은 이력으로 남긴다.
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
    /// §7 M2가 요구하는 `moveBefore` **네이티브** vs `insertBefore` **폴백**
    /// 분해 — `(네이티브, 폴백)` 누적 호출 수.
    ///
    /// 이 분기는 백엔드 **안에서** 일어나므로 바깥 데코레이터(`CountingDom`)
    /// 에서는 보이지 않는다. 그래서 트레잇 자체에 얹는다. 기본값 `(0, 0)`은
    /// "이 백엔드에는 그런 분기가 없다"는 뜻이다(SSR `StringDom` 4종이 그렇다).
    ///
    /// §9 E의 "채택 시 조건부 제약" 보고에 필요하다 — E의 이점이 네이티브
    /// 경로에만 있는지, 폴백에서도 남는지를 이 수치 없이는 말할 수 없다.
    fn reposition_split(&self) -> (u64, u64) {
        (0, 0)
    }
    /// `LoomApp::tick`이 DOM write 배치를 적용하기 **직전**에 부른다.
    ///
    /// 왜 필요한가: 백엔드가 노드 핸들 슬롯을 재사용하려면 "언제부터 재사용해도
    /// 안전한가"를 알아야 한다. 한 배치 안에서 해제된 핸들을 같은 배치의 뒤쪽
    /// write가 anchor로 참조할 수 있으므로, 재사용은 배치 경계에서만 열어야
    /// 한다. 기본 구현은 아무것도 하지 않는다(핸들을 재사용하지 않는 백엔드).
    fn begin_batch(&mut self) {}
    /// 이 백엔드가 현재 붙들고 있는 노드 슬롯 수 — 누수 회귀 확인용.
    /// `None` = 이 백엔드는 그런 개념이 없다.
    fn live_node_count(&self) -> Option<usize> {
        None
    }
    /// (2026-10-17 note: "candidate F" is the Phase 1 label. `cond_reconcile_f_system` is the CSS-hide preserve variant, i.e. the protocol's CSS 은닉 (v-show) control arm — not the F family F-i/ii/iii added 2026-07-31. See cond.rs header.)
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
    /// Sets a single attribute on an element. **2026-10-26 정정(7c)**: 아래 "no `DomWrite` variant emits this"는
    /// `Surface`(`SetAttr`/`SpawnSpec`의 정적 속성 등) 이전의 서술이다 — 이제 `DomWrite::Surface`가 이 메서드를
    /// 반응 경로에서도 부른다. 원문: outside the reactive patch
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

    // ---- Phase 7c(`DomWrite::Surface`)가 쓰는 메서드들 — `surface.rs` 모듈 문서 참고 -------------------------
    // 전부 기본 구현이 `unimplemented!`이다(`detach_to_fragment`의 관례): SSR `StringDom` 등 이 표면을 아직 구현하지
    // 않은 백엔드가 불리면 조용히 잘못 동작하지 않고 즉시 실패한다. 특히 `create_element_ns`를 `create_element`로 조용히
    // 폴백하는 기본 구현은 두지 않는다 — SVG가 HTML 엘리먼트로 만들어지는 R43과 같은 종류의 조용한 결함을 되만든다.
    // 쓰기 메서드는 DOM의 **현재 값을 읽어 같으면 건너뛰지 않는다**(N1/N4).

    /// `create_element`의 네임스페이스 판. SVG 엘리먼트의 `className`은 `SVGAnimatedString`이라 쓰기가 반영되지
    /// 않으므로 구현은 비-HTML에서 `class` 속성으로 쓴다.
    fn create_element_ns(&mut self, _parent: NodeHandle, _ns: Namespace, _tag: &str, _class: Option<&str>) -> NodeHandle {
        unimplemented!("create_element_ns: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    fn remove_attribute(&mut self, _node: NodeHandle, _name: &str) {
        unimplemented!("remove_attribute: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    /// `classList.toggle(class, on)` — 강제 상태 판. 이미 그 상태여도 호출하며, 비교는 하지 않는다.
    fn toggle_class(&mut self, _node: NodeHandle, _class: &str, _on: bool) {
        unimplemented!("toggle_class: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    /// 반사되지 않는 불리언 **프로퍼티** 대입(`el.checked = ...`).
    fn set_bool_prop(&mut self, _node: NodeHandle, _prop: BoolProp, _value: bool) {
        unimplemented!("set_bool_prop: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    /// `parent`의 마지막 자식으로 **텍스트 노드**를 만들고 그 핸들을 돌려준다(혼합 텍스트의 동적 조각).
    fn create_text_node(&mut self, _parent: NodeHandle, _text: &str) -> NodeHandle {
        unimplemented!("create_text_node: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    /// 텍스트 노드의 `data`를 쓴다(`CharacterData.data`).
    fn set_text_data(&mut self, _node: NodeHandle, _text: &str) {
        unimplemented!("set_text_data: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    /// `style.setProperty(prop, value)`, `value == None`이면 `removeProperty(prop)`.
    fn set_style(&mut self, _node: NodeHandle, _prop: &str, _value: Option<&str>) {
        unimplemented!("set_style: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
    /// Resume 전용 읽기: `adopt_children`의 **엘리먼트 자식만** 판. 공백·주석 노드가 원리적으로 빠진다.
    /// 텍스트는 `text_content`라 자손 텍스트를 이어 붙인다 — `<li>v<span>..</span></li>` 같은 복합 row는 값이
    /// 틀리게 복구된다(R35는 "공백 노드"만 해결, "복합 row 텍스트"는 남는다). **소비 codegen은 N1 판정례 후보
    /// (docs/03)가 판정될 때까지 켜지 않는다.**
    fn adopt_element_children(&mut self, _parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        unimplemented!("adopt_element_children: 이 DomOps 구현은 7c Surface 표면을 지원하지 않는다")
    }
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
#[derive(Debug)]
pub enum DomWrite {
    /// 컴파일러가 생성한 **지연 서브트리**의 엘리먼트 하나
    /// (loom-compiler codegen.rs의 `emit_lazy_subtree`). `SpawnRow`와 같은
    /// 계약이다 — entity는 이미 `Commands`나 `setup()`이 할당해 뒀고, 드라이버가
    /// 실제 엘리먼트를 만들어 `DomTarget`을 붙인다. 다른 점 셋:
    ///
    /// 1. 부모가 `WriteParent` — 이미 해석된 핸들일 수도, **같은 배치에서 바로
    ///    앞에 만든 entity**일 수도 있다. 중첩 서브트리는 부모가 자식보다 항상
    ///    먼저 방출되므로, 적용 시점엔 부모의 `DomTarget`이 이미 붙어 있다
    ///    (`SpawnRow`의 `before` 앵커 해석과 정확히 같은 근거).
    /// 2. `class`/`loom_id`를 싣는다 — 지연 생성된 노드도 `data-loom-id`가
    ///    있어야 `resume()`이 SSR된 DOM에서 자기 노드를 다시 찾는다.
    /// 3. `text`가 `Option` — 컨테이너 엘리먼트(`<ul>` 등)에 빈 텍스트 노드를
    ///    만들지 않기 위해서다. `Some`일 때 값을 **생성 시점에 굽는** 이유는
    ///    `SpawnRow`와 같다: 이 entity의 `DomTarget`은 지금 이 write가 적용될
    ///    때 비로소 붙으므로, 그 전에 지나간 `Changed` 틱은 patch System이
    ///    관측할 수 없다. 초기값은 반드시 여기서 실려야 한다.
    SpawnElement {
        entity: Entity,
        parent: WriteParent,
        tag: &'static str,
        class: Option<&'static str>,
        loom_id: Option<&'static str>,
        text: Option<String>,
    },
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
    /// (2026-10-17 note: "candidate F" is the Phase 1 label. `cond_reconcile_f_system` is the CSS-hide preserve variant, i.e. the protocol's CSS 은닉 (v-show) control arm — not the F family F-i/ii/iii added 2026-07-31. See cond.rs header.)
    /// RQ1 Track T candidate F: show/hide without despawn (see
    /// `DomOps::set_visible`).
    SetVisible { node: NodeHandle, visible: bool },
    /// Compiler-generated conditionals (`{cond() && <X/>}`, loom-compiler's
    /// codegen.rs): appends one static text child under an *already
    /// resolved* parent node. No `Entity`-to-`NodeHandle` resolution is
    /// needed here — the parent (the
    /// conditional's wrapper element) was created directly via
    /// `DomOps::create_element` before any reconcile System ever ran, so it
    /// already has a real `NodeHandle` by the time this write is emitted.
    ///
    /// **2026-08-07**: `parent`가 `NodeHandle`에서 `WriteParent`로 넓어졌다 —
    /// 조건부 안에 중첩된 엘리먼트가 생기면서, 정적 텍스트의 부모가 *같은
    /// 배치에서 방금 만들어진* 노드일 수 있게 됐기 때문이다(`SpawnElement`
    /// 참고). 기존 방출부는 전부 `WriteParent::Node(...)`로, 의미 무변경.
    AppendStaticText { parent: WriteParent, text: String },
    /// 지연 서브트리 안의 **완전 정적** 부분트리 하나를 통째로. `setup()` 쪽
    /// `try_render_fully_static` → `append_static_html` 접기와 같은 최적화를
    /// 지연 경로에도 적용한 것이다 — HTML 문자열은 컴파일타임에 완성되고
    /// 런타임에는 호출 한 번만 남는다.
    AppendStaticHtml { parent: WriteParent, html: String },
    /// **2026-10-26 정정(7c)**: 이 doc의 앞부분은 R2 축 변형 `SetRowBindingTexts`("R2 축: 이미 만들어진 파생
    /// binding들의 텍스트를 **다시 쓴다** … 그것이 §9 C 기각 조항이 겨냥하는 비용이다 … (그 비대칭이 곧")의
    /// 설명이었는데, 그 변형이 개정 #9-11(98c5d25)에서 제거될 때 이 변형의 doc과 문장 중간에서 이어 붙은 채
    /// 남아 있었다. 지금은 그 변형이 없으므로 잘린 앞부분을 뺀다(docs/09 §3 "문서와 코드가 어긋난 곳").
    ///
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
    /// Phase 7c 표면(`surface.rs`). **반드시 마지막 variant로 둔다.** 앞에 끼우면 niche 인코딩된 기존 variant들의
    /// 판별값이 바뀌어, 기존 생성 지점(`DefaultRow::emit`, `cond.rs`의 방출부)의 코드가 달라진다.
    /// `tests/surface_writes.rs`가 크기와 대표 판별값을 고정한다.
    Surface(surface::SurfaceWrite),
}

/// row 스코프 파생값의 **결과**들 (2026-08-07). 파생 *계산*은 컴파일러가
/// 사이트별 생성 코드에 인라인하므로 런타임에는 남지 않고, Component에는 그
/// 결과만 POD로 들어간다 — CLAUDE.md 금지 4가 요구하는 그대로다(클로저도,
/// 런타임에 해석되는 표현식 트리도 없다).
///
/// 왜 저장해야 하는가: 공유 signal이 바뀌면 row별 mirror를 다시 계산해야 하는데,
/// 그때 "이 row의 아이템이 무엇이었나"를 다시 알아낼 방법이 없다(row 아이템은
/// 후보에 따라 `RowKey`/`OrderedKey`/없음으로 제각각이다). 그래서 아이템에서
/// 파생된 **부분**을 spawn 시점에 한 번 계산해 여기 남긴다.
///
/// **2026-10-20 정정(G0R1)**: D/E의 `RowKey`/`OrderedKey`는 이제 아이템 전체의 `u64` 해시이고 아이템 값은 별도
/// 값 채널 `RowValue<T>`에 있다. 그래도 파생값을 여기 따로 두는 이유는 같다 — 컴파일러 산출물의 row 템플릿은 `T=String`이
/// 아닌 파생 mirror를 갖고, 그 재계산을 `RowValue<T>`의 타입에 묶지 않기 위해서다.
#[derive(Component, Debug, Clone, Copy)]
pub struct RowDerivedFlag(pub bool);

#[derive(Component, Debug, Clone, Copy)]
pub struct RowDerivedTake(pub usize);

/// row가 소유한 **사이트 고유 entity들**, 컴파일타임에 고정된 순서.
/// (2026-08-08 신설) 후보 C는 슬롯의 아이템이 **제자리에서 바뀌는** 유일한
/// 후보라, row에서 파생된 값(`RowDerivedFlag`/`RowDerivedTake`)과 그 값을 담은
/// mirror entity를 다시 계산해야 한다 — 그러려면 row에서 그것들을 찾아갈 수
/// 있어야 한다. keyed 후보(D/E)는 retained row의 키(=텍스트)가 그대로라 이
/// 경로를 타지 않는다. POD(`Vec<Entity>`)이므로 금지 4 유지.
#[derive(Component, Debug, Clone, Default)]
pub struct RowOwned(pub Vec<Entity>);

/// `DomWrite`가 부모를 지목하는 두 가지 방식. **이미 해석된 핸들**(`setup()`이
/// 직접 만든 노드)과 **아직 만들어지지 않았을 수 있는 entity**(같은 배치의 앞선
/// write가 만들 노드)를 한 타입으로 합친다. 후자는 `SpawnRow`의 `before` 앵커가
/// 이미 쓰던 규약과 동일하다 — write는 방출 순서대로 적용되고, 부모는 자식보다
/// 항상 먼저 방출되므로 해석은 항상 성공한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteParent {
    Node(NodeHandle),
    Entity(Entity),
}

/// 조건부 사이트가 **자기 지연 서브트리 안의 binding entity들**을 순서대로
/// 들고 있는 사이드테이블. 컴파일러가 사이트마다 고정한 순서이고, 생성된
/// reconcile 함수는 `nested.0[<컴파일타임 상수>]`로만 접근한다 — 런타임 탐색이
/// 없다(이름 조회도, 트리 순회도 아니다).
///
/// **왜 entity를 미리 만들어 두는가**: 조건부 안의 signal도 `Signals` 구조체와
/// action/`resume()` 경로에 그대로 참여해야 한다. 그래서 entity 자체는 `setup()`
/// 이 항상 즉시 spawn하되 **`DomTarget`은 붙이지 않는다** — patch System들의
/// 쿼리가 `&DomTarget`을 요구하므로, DOM에 아직 없는 동안에는 archetype 수준에서
/// (= 런타임 분기 없이) 자동으로 제외된다. 조건부가 처음 `true`가 될 때
/// `SpawnElement`가 `DomTarget`을 붙이면 그때부터 평범한 binding이 된다.
/// CLAUDE.md 금지 4 유지: 여기 들어가는 것은 `Entity`(정수 쌍)뿐이다.
#[derive(Component, Debug, Clone, Default)]
pub struct CondNested(pub Vec<Entity>);

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

#[derive(Resource, Default)]
pub struct PendingDomWrites(pub Vec<DomWrite>);

/// RQ1 row-depth axis: how many static text children each list row carries
/// beyond its own primary content (default 1 = R1, "binding 1개"). Read by
/// every Track L candidate's spawn path (`list_reconcile_k_system` here;
/// `list_reconcile_c_system`/`place_in_order`/`place_in_order_lis` in
/// `list.rs`; **2026-10-17 정정**: 뒤의 둘은 2026-08-11 제거됐고, 지금 D/E 계열의
/// spawn은 `list.rs`의 `emit_plan` → `RowSpawner::emit` 경로다). **개정 #9-11(2026-08-12)로 이 축의 모델이 바뀌었다**: 예전에는
/// row당 정적 텍스트 자식 `n-1`개였고, 이제는 `RowBinding` entity `n-1`개
/// (각자 `Signal<String>` + `DomTarget`, `Changed` 게이팅)와 중첩 조건부 1개다 —
/// §3.2 문언("binding 8개 + 중첩 조건부 1개") 그대로. `RowBinding`의 doc comment에
/// 왜 바꿨는지와 그 변경이 어느 후보에 어떤 방향인지 상세.
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

/// **§10의 "write 변형" 축**(개정 #9-6). 조건부 `PartialEq` vs 무조건 write.
///
/// §3.3이 "판정에 걸린 {W1,W2,W3,W4}×{R1,R2} 및 W10에서 C·D 각각 양 변형 측정"을
/// 사전등록했고, §9 서두가 "판정은 각 후보의 **셀별 우세 write 변형** 기준(양
/// 변형이 모두 측정되므로 선택은 기계적 — 사후 재량 없음)"이라고 못박았다.
/// 2026-08-12까지 조건부 하나만 구현돼 있었고, 그래서
///
/// - 그 §9 규정이 **작동할 수 없었고**(고를 변형이 하나뿐),
/// - §10의 근거 문언이 "재정렬 셀에서 **C 최선 = 무조건**"을 지목하므로
///   **C가 자기 최선형으로 측정되고 있지 않았다**(§2.4 최선형 규율 위반).
///
/// 무조건 변형의 의미는 후보마다 다르다. C는 슬롯 값 비교를 건너뛰고 항상 쓴다.
/// D는 키(=텍스트) 매칭이라 retained row의 텍스트가 정의상 같지만, 비교를 하지
/// **않는** 구현은 대상 행 전부에 텍스트를 다시 쓴다 — 그것이 이 축이 D에 대해
/// 재는 실제 구현 선택지다.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum WriteVariant {
    /// 값이 다를 때만 쓴다(2026-08-12까지의 유일한 구현).
    #[default]
    Conditional,
    /// 비교 없이 항상 쓴다.
    Unconditional,
}

/// **W8(§3.1 "중첩 증폭")의 row별 내부 리스트 크기.** 0 = 꺼짐(기본).
///
/// 조항: "외부 100 × 내부 100 중첩에서 외부 head-insert(C의 O(n_out×n_in) 표적)".
/// row마다 `<ul>` 하나와 그 아래 `RowInner.0`개의 `<li>`를 만든다. 내부 리스트는
/// **스스로 재조정하지 않는다** — 값이 row의 아이템에서 파생되므로, 슬롯 아이템이
/// 제자리에서 바뀌는 C에서만 n_in개가 다시 쓰이고 retained row를 갖는 D·E에서는
/// 아무 일도 안 일어난다. 그 비대칭이 §3.4가 W8에 등록한 예측 그 자체다.
#[derive(Resource, Clone, Copy)]
pub struct RowInner(pub usize);

impl Default for RowInner {
    fn default() -> Self {
        RowInner(0)
    }
}

/// **브랜치 B rows의 구조 차이를 담는 래퍼 노드**(개정 #9-12, 2026-08-12).
///
/// 개정 #8 말미가 "§3.2의 R3 문언은 '값에 따라 `<A/>`/`<B/>` **구조 분기**'인데
/// DOM 구조는 A/B가 동일하다 — 분기는 ECS archetype 수준에만 존재한다"를 별도
/// 미이행으로 등재하고 "재수집 전에 별도로 결정할 것"으로 남겼다. 개정 #9가 그
/// 결정을 내렸다: **구조를 실제로 다르게 만든다.** A는 텍스트만, B는 `<em>` 래퍼가
/// 하나 더 붙는다 — 최소한이지만 진짜 구조 차이다.
///
/// 값에서 파생되므로 슬롯 아이템이 제자리에서 바뀌는 C에서는 전환마다 DOM 조작이
/// 실제로 발생하고, retained row를 갖는 keyed 후보에서는 발생하지 않는다.
#[derive(Component, Debug, Clone, Copy)]
pub struct RowVariantNode(pub Entity);

/// 값 → 브랜치. 결정적이어야 시드 고정 하에 M2가 결정적 수치가 된다(§7 M2).
pub(crate) fn row_variant_is_a(text: &str) -> bool {
    text.as_bytes().last().map(|b| b % 2 == 0).unwrap_or(true)
}

pub(crate) fn extra_binding_texts(row_bindings: usize, primary: &str) -> Vec<String> {
    (1..row_bindings).map(|i| format!("{primary}-bind{i}")).collect()
}

/// **R2(§3.2)의 row 내부 바인딩 하나.** 개정 #9-11로 신설.
///
/// 2026-08-12 이전의 R2는 `RowBindingTargets(Vec<NodeHandle>)` — reconcile System이
/// 명령형으로 핸들에 직접 쓰는 구조였다. 값 패치 비용(C의 O(n×binding))은 그것으로도
/// 잡혔지만, docs/03이 말하는 binding은 **`Changed<Signal<T>>`로 게이팅된 좁은 System
/// 하나가 DOM op 하나를 내는 것**이다. 그래서 옛 구조는 binding당 변경 감지, 8n개
/// entity가 만드는 archetype 폭, patch System의 쿼리 순회, entity spawn/despawn 비용을
/// 하나도 재지 않았다 — §9의 D 기각(iii)·D 양성·E 기각(i)이 전부 R2 셀에 걸려 있는데도.
///
/// 이 marker가 붙은 entity는 `Signal<String>` + `DomTarget`을 갖고
/// `patch_row_binding_system`이 자기 것만 골라 패치한다. `patch_text_system::<String>`을
/// 그냥 쓰지 않는 이유는 `Signal<String>`이 `<input>` 양방향 바인딩과 같은 타입이라
/// 교차 매칭될 수 있기 때문이다(`patch_input_system`의 doc comment와 같은 사유).
#[derive(Component, Debug, Clone, Copy)]
pub struct RowBinding;

/// **R2의 "중첩 조건부 1개"**(§3.2). row마다 하나씩 달리는 조건부 블록의 앵커.
/// 실효값은 row의 아이템에서 파생되므로(`row_cond_is_shown`) 슬롯 값이 제자리에서
/// 바뀌는 후보 C에서는 토글이 실제로 일어나고, retained row를 갖는 D·E에서는
/// 일어나지 않는다 — R3 브랜치와 같은 비대칭이며 같은 이유다.
#[derive(Component, Debug, Clone, Copy)]
pub struct RowCond;

/// 값 → 중첩 조건부의 실효값. `row_variant_is_a`와 같은 이유로 결정적이어야 한다
/// (§7 M2가 "시드 고정 하에 결정적 수치"를 요구).  마지막 바이트의 하위 2비트를
/// 보므로 `row_variant_is_a`(하위 1비트)와 **독립적이지 않게** 겹치지 않도록 다른
/// 비트를 쓴다 — 두 축이 같은 조건에서 동시에 뒤집히면 R2와 R3의 효과가 교락된다.
pub(crate) fn row_cond_is_shown(text: &str) -> bool {
    text.as_bytes().last().map(|b| (b >> 1) % 2 == 0).unwrap_or(true)
}

/// row 하나를 만들 때 생성기가 쓸 수 있는 것들. `Commands`와 write 큐만 있고
/// **World를 읽을 수단은 없다** — row 생성은 "이 모양대로 write를 낸다"이지
/// 상태를 조회하는 일이 아니어야 하기 때문이다. 사이트가 외부 값을 필요로
/// 하면 `RowSpawner::Ctx`(아래)로 받는다.
pub struct RowSpawnCtx<'a, 'w, 's> {
    pub commands: &'a mut Commands<'w, 's>,
    pub pending: &'a mut Vec<DomWrite>,
    pub row_bindings: usize,
    /// W8(§3.1)의 row별 내부 리스트 크기. 0이면 내부 리스트를 안 만든다.
    pub row_inner: usize,
    /// RQ1 §3.2의 R3 축이 켜져 있는가. `RowVariantMode`를 그대로 실어 나른다 —
    /// row를 만드는 지점이 곧 브랜치 marker를 붙일 지점이기 때문이다.
    pub row_variants: bool,
}

/// **리스트 row의 모양**. 알고리즘(C/D/E/K)은 제네릭으로 그대로 두고 이 한
/// 지점만 컴파일타임에 갈아 끼운다 — 컴파일러가 사이트마다 ZST 구현을 하나
/// 생성하고 시스템을 그 타입으로 monomorphize한다. 런타임 디스패치도, row
/// 템플릿 트리를 런타임에 해석하는 인터프리터도 없다.
///
/// 왜 알고리즘을 코드젠으로 복제하지 않는가: D/E의 배치 계획은 Verus로 검증된
/// `loom_flush_core::plan_writes*`가 낸다(`list.rs` 참고). 사이트마다 그 알고리즘을
/// 복제하면 검증된 코드가 아니라 **복제본**이 배포된다. 모양만 파라미터로 빼면
/// 검증 대상은 하나로 유지된다.
///
/// **RQ1 §2.4 [E]의 "site별 monomorphize"** 최선형 항목이 요구하는 것이 정확히
/// 이 형태이기도 하다 — 새로 발명한 구조가 아니다.
pub trait RowSpawner: Send + Sync + 'static {
    /// 이 생성기가 row를 만들 때 필요로 하는 **사이트 고유 입력**. 없으면
    /// `NoRowCtx`. Resource로 두는 이유: row 생성은 리스트 System 안에서
    /// 일어나므로 값을 미리 계산해 두는 System을 앞에 세워야 하고, 그 둘
    /// 사이의 통로가 Resource다(Component에 클로저를 담지 않는다는 원칙
    /// — CLAUDE.md 금지 4 — 을 유지하면서 값을 전달하는 방법).
    type Ctx: Resource + Default;

    /// 이 사이트의 **리스트 entity**에 붙는 표시. 한 컴포넌트에 리스트 사이트가
    /// 여럿이면 각자 자기 것만 재조정해야 하는데, 리스트 entity의 컴포넌트
    /// 구성(`Signal<Vec<T>>` + `ListRows` + `DomTarget`)만으로는 구분이 안 된다
    /// — 조건부의 `CondSiteN`이 필요했던 것과 같은 이유, 같은 해법이다.
    ///
    /// **접미사 없는 시스템은 이 필터를 쓰지 않는다.** 그래서 트랙 L 하네스처럼
    /// 마커 없이 spawn하는 쪽은 아무 영향도 받지 않는다. 컴파일러는 한
    /// 컴포넌트에 복잡한 row 템플릿이 하나라도 있으면 **그 컴포넌트의 리스트
    /// 사이트 전부**를 마커 + `_with`로 내보내므로, 마커 없는 시스템과 섞여
    /// 한 리스트가 두 번 재조정되는 일이 생기지 않는다.
    type Marker: Component;

    /// row 서브트리를 만드는 write들을 방출한다. `row`는 알고리즘이 이미 만들어
    /// 둔 entity(키/OrderKey를 이미 달고 있다)이고, 드라이버가 여기에
    /// `DomTarget`을 붙인다. `text`는 이 row의 아이템 값.
    fn emit(
        site: &Self::Ctx,
        out: &mut RowSpawnCtx,
        row: Entity,
        parent: NodeHandle,
        text: &str,
        before: Option<Entity>,
    );

    /// 슬롯의 아이템이 **제자리에서 바뀌었을 때** row의 파생값을 다시 계산한다.
    /// 후보 C의 값 관통 경로에서만 불린다 — keyed 후보는 retained row의 키가
    /// 그대로여서 파생값도 그대로다(그 비대칭이 RQ1의 예측이다).
    /// `owned`는 이 row의 `RowOwned`(없으면 빈 슬라이스).
    ///
    /// 기본 구현이 no-op인 것은 `DefaultRow`가 파생값을 갖지 않기 때문이다 —
    /// 사이트별 생성기만 이것을 구현한다.
    fn revalue(
        site: &Self::Ctx,
        out: &mut RowSpawnCtx,
        row: Entity,
        owned: &[Entity],
        text: &str,
    ) {
        let _ = (site, out, row, owned, text);
    }
}

/// `RowSpawner::Ctx`가 필요 없는 사이트용 빈 입력.
#[derive(Resource, Default)]
pub struct NoRowCtx;

/// 오늘까지의 유일한 row 모양(`<li>{item}</li>` + R2 축의 추가 binding들).
/// **방출하는 write 열이 이 리팩터 이전과 한 글자도 다르지 않아야 한다** —
/// 그래야 트랙 L의 기존 측정이 §2.5의 재측정 대상이 되지 않는다.
pub struct DefaultRow;

/// `DefaultRow`용 마커. 접미사 없는 시스템은 마커를 보지 않으므로 실제로는
/// `_with::<_, DefaultRow>`를 직접 쓰는 테스트에서만 등장한다.
#[derive(Component, Default)]
pub struct AnyRow;

impl RowSpawner for DefaultRow {
    type Ctx = NoRowCtx;
    type Marker = AnyRow;

    fn emit(
        _site: &NoRowCtx,
        out: &mut RowSpawnCtx,
        row: Entity,
        parent: NodeHandle,
        text: &str,
        before: Option<Entity>,
    ) {
        out.pending.push(DomWrite::SpawnRow {
            entity: row,
            parent,
            tag: "li",
            text: text.to_string(),
            before,
        });
        // **R2(§3.2) — 깊은 row = "binding 8개 + 중첩 조건부 1개"** (개정 #9-11,
        // 2026-08-12). 축이 꺼져 있으면(`row_bindings == 1`) 아래는 통째로 건너뛰고
        // 방출 write 열이 R1에서 한 글자도 안 바뀐다 — 변경을 R2 셀에만 가둔다.
        //
        // 이전 구현은 `AppendStaticChildren`으로 핸들만 만들고 reconcile System이
        // 명령형으로 썼다. 이제 바인딩 하나가 **자기 entity + 자기 Signal**을 갖고
        // `patch_row_binding_system`이 `Changed`로 게이팅해 패치한다(`RowBinding`의
        // doc comment에 왜 바꿨는지 상세). 그 결과 K는 재정렬마다 8n개 entity를
        // 만들고 지우게 되는데, 그것이 조항이 정한 row 모양에서 대조군이 실제로
        // 내는 비용이다.
        if out.row_bindings > 1 || out.row_inner > 0 {
            let mut owned = Vec::with_capacity(out.row_bindings + out.row_inner);
            for extra in extra_binding_texts(out.row_bindings, text) {
                // `ChildOf`로 row에 매단다 — Bevy의 `Children`이
                // `#[relationship_target(.., linked_spawn)]`이라 **row를 despawn하면
                // 이 entity들도 함께 사라진다**. 손으로 정리하면 list.rs의 despawn
                // 지점 여섯 곳을 전부 고쳐야 하고 하나라도 빠뜨리면 World가 매 틱
                // 누적된다(실제로 K에서 그 누수가 났고 `tests/r2_axis.rs`가 잡았다).
                // CLAUDE.md 2-1 — 이미 잘 풀린 메커니즘은 그대로 재사용한다.
                let child = out
                    .commands
                    .spawn((Signal(extra.clone()), RowBinding, ::bevy_ecs::hierarchy::ChildOf(row)))
                    .id();
                out.pending.push(DomWrite::SpawnElement {
                    entity: child,
                    parent: WriteParent::Entity(row),
                    tag: "span",
                    class: None,
                    loom_id: None,
                    text: Some(extra),
                });
                owned.push(child);
            }
            // **W8 — row별 내부 리스트**(개정 #9-3). `<ul>` 하나 아래 `row_inner`개의
            // `<li>`. 각 항목은 R2 바인딩과 같은 기계(`RowBinding` + `Signal<String>`)를
            //쓴다 — 값이 row의 아이템에서 파생되므로 C의 값-통과가 n_in개를 다시
            // 쓰고, 그것이 조항이 말한 O(n_out×n_in) 증폭이다.
            if out.row_inner > 0 {
                let ul = out.commands.spawn(::bevy_ecs::hierarchy::ChildOf(row)).id();
                out.pending.push(DomWrite::SpawnElement {
                    entity: ul,
                    parent: WriteParent::Entity(row),
                    tag: "ul",
                    class: None,
                    loom_id: None,
                    text: None,
                });
                for i in 0..out.row_inner {
                    let item_text = format!("{text}-in{i}");
                    let item = out
                        .commands
                        .spawn((
                            Signal(item_text.clone()),
                            RowBinding,
                            ::bevy_ecs::hierarchy::ChildOf(ul),
                        ))
                        .id();
                    out.pending.push(DomWrite::SpawnElement {
                        entity: item,
                        parent: WriteParent::Entity(ul),
                        tag: "li",
                        class: None,
                        loom_id: None,
                        text: Some(item_text),
                    });
                    owned.push(item);
                }
            }

            // 중첩 조건부 1개 — **R2 축에만 딸린다**(§3.2의 "binding 8개 + 중첩
            // 조건부 1개"). W8은 조항이 조건부를 요구하지 않으므로 안 만든다.
            // 앵커 엘리먼트를 먼저 만들고(`DomTarget`이 붙어야
            // `cond_reconcile_b_system`이 자식을 그 아래에 넣는다) 실효값은 row의
            // 아이템에서 파생한다. arm은 B — 트랙 T 판정(2026-08-07)이 "B 유지,
            // F 계열 기각"이므로 기준선 arm을 쓰는 것이 맞고, 조건부 후보 판정은
            // 트랙 T의 일이지 트랙 L의 row 깊이 축이 정할 일이 아니다.
            if out.row_bindings > 1 {
                let shown = row_cond_is_shown(text);
                let cond = out.commands.spawn_empty().id();
                out.pending.push(DomWrite::SpawnElement {
                    entity: cond,
                    parent: WriteParent::Entity(row),
                    tag: "span",
                    class: None,
                    loom_id: None,
                    text: None,
                });
                // **실효값이 true면 자식을 spawn 시점에 함께 낸다** (2026-09-24, 감사
                // 발견 #1/#12). 그 전에는 `cond_reconcile_b_system`이 다음 tick에
                // 물질화하는 구조였는데, 하네스의 측정 반복에서 **K의 row는 정확히
                // 1 tick만 산다**(매 tick 재정렬로 전량 재스폰) — 그래서 K의 조건부는
                // 한 번도 물질화되지 않았고 대조군만 조건부 몫의 create/set_text/
                // spawn/despawn 비용을 0으로 냈다. §9의 D 기각(iii)·D 양성·E 기각(i)이
                // 전부 R2 셀의 K 기준선에 걸려 있으므로 그 넷이 keyed 후보에 불리한
                // 방향으로 계통 편향돼 있었다. `Added<DomTarget>` 기반 수정으로는
                // 원리적으로 못 고친다(1 tick 뒤엔 row가 없다) — 같은 tick에 내야 한다.
                //
                // 자식은 `cond_reconcile_b_system`과 **같은 번들**(`block_child_bundle`)
                // 이고, 부모는 `WriteParent::Entity(cond)`라 같은 배치 안에서 앵커의
                // `DomTarget`이 붙은 직후 해석된다(R2 바인딩과 같은 기제).
                let block = cond::CondBlock::default();
                let mut children = Vec::new();
                if shown {
                    for _ in 0..block.size {
                        let child = out.commands.spawn(cond::block_child_bundle(cond)).id();
                        out.pending.push(DomWrite::SpawnElement {
                            entity: child,
                            parent: WriteParent::Entity(cond),
                            tag: "span",
                            class: None,
                            loom_id: None,
                            text: Some("0".to_string()),
                        });
                        children.push(child);
                    }
                }
                out.commands.entity(cond).insert((
                    Signal(shown),
                    cond::CondBlock { size: block.size, children },
                    RowCond,
                    ::bevy_ecs::hierarchy::ChildOf(row),
                ));
                owned.push(cond);
            }
            // `ChildOf`가 정리를 맡고 `RowOwned`는 **순서 있는 접근**을 맡는다 —
            // `revalue`가 "R2가 켜져 있으면 마지막이 조건부, 나머지는 전부 값 파생
            // 텍스트 바인딩"이라는 고정 배치로 인덱싱하기 때문이다(`Children`의
            // 순서에 기대지 않는다). 그 조건은 `RowSpawnCtx`만 보고 결정되므로
            // `revalue`가 World를 안 읽고도 같은 배치를 재구성할 수 있다.
            out.commands.entity(row).insert(RowOwned(owned));
        }
        // **R3(§3.2) — 이질 브랜치 row** (2026-08-07 수리). 그 전까지 이 축을
        // 실제로 적용하는 후보는 C 하나뿐이었고, D/D-LIS/E/E-LIS는
        // `RowVariantMode`를 파라미터로 **선언만 하고 읽지 않았으며** K는 선언조차
        // 없었다. 그래서 수집분의 `row_variants: true` 셀이 C 외 다섯 후보에
        // 대해서는 R1 셀에 R3 라벨이 붙은 것이었다(§3.3의 "R3 × {C,D}" 사전등록이
        // D에 대해 이행된 적이 없다).
        //
        // 여기서 붙이는 것은 §10이 사전 고정한 "슬롯 entity 컴포넌트 교체
        // (archetype move)형"의 **초기 배정**이다. keyed 후보(D/E)에서 브랜치
        // *전환*이 일어나지 않는 것은 정상이다 — retained row는 키(=텍스트)가
        // 그대로라 파생 브랜치도 그대로다. 그것이 RQ1이 예측한 비대칭이고, 이제
        // 그 비대칭은 **구현 누락 때문이 아니라 keyed 성질 때문에** 성립한다.
        if out.row_variants {
            if row_variant_is_a(text) {
                out.commands.entity(row).insert(RowVariantA);
            } else {
                out.commands.entity(row).insert(RowVariantB);
                // **구조 분기**(개정 #9-12): B만 `<em>` 래퍼를 하나 더 갖는다.
                // `ChildOf`이므로 row가 despawn되면 함께 사라진다.
                let em = out
                    .commands
                    .spawn(::bevy_ecs::hierarchy::ChildOf(row))
                    .id();
                out.pending.push(DomWrite::SpawnElement {
                    entity: em,
                    parent: WriteParent::Entity(row),
                    tag: "em",
                    class: None,
                    loom_id: None,
                    text: Some(text.to_string()),
                });
                out.commands.entity(row).insert(RowVariantNode(em));
            }
        }
    }

    /// **C 전용 경로** — 슬롯의 아이템이 제자리에서 바뀌었을 때 그 값에서 파생된
    /// 것들을 갱신한다. R2 축이 꺼져 있으면 `owned`가 비어 있어 통째로 no-op이다.
    ///
    /// `Signal`을 `insert`로 덮어쓰면 변경 표시가 서므로 `patch_row_binding_system`과
    /// `cond_reconcile_b_system`이 **평범하게** 반응한다 — 여기서 DOM write를 직접
    /// 내지 않는 것이 요점이다. 옛 구현(`SetRowBindingTexts`)은 reconcile System이
    /// DOM에 직접 썼고, 그래서 binding당 변경 감지라는 것이 존재하지 않았다.
    fn revalue(
        _site: &NoRowCtx,
        out: &mut RowSpawnCtx,
        _row: Entity,
        owned: &[Entity],
        text: &str,
    ) {
        if owned.is_empty() {
            return;
        }
        // R2가 켜져 있으면 마지막 하나가 조건부다(`emit`과 같은 조건 — World를
        // 읽지 않고 `RowSpawnCtx`만으로 배치를 재구성한다).
        let (texts, cond) = if out.row_bindings > 1 {
            let (last, rest) = owned.split_last().unwrap();
            (rest, Some(*last))
        } else {
            (owned, None)
        };
        let n_bind = out.row_bindings.saturating_sub(1);
        for (i, &child) in texts.iter().enumerate() {
            let next = if i < n_bind {
                format!("{text}-bind{}", i + 1)
            } else {
                format!("{text}-in{}", i - n_bind)
            };
            out.commands.entity(child).insert(Signal(next));
        }
        if let Some(cond) = cond {
            out.commands.entity(cond).insert(Signal(row_cond_is_shown(text)));
        }
    }
}

/// R2 바인딩의 patch System — docs/03 "반응성 파이프라인"의 좁은 System 하나.
/// `Changed<Signal<String>>`로 게이팅되고 DOM op를 정확히 하나 낸다.
pub fn patch_row_binding_system(
    q: Query<(&Signal<String>, &DomTarget), (Changed<Signal<String>>, With<RowBinding>)>,
    mut pending: ResMut<PendingDomWrites>,
) {
    for (signal, target) in &q {
        pending.0.push(DomWrite::SetText {
            target: target.0,
            text: signal.0.clone(),
        });
    }
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
    q: Query<
        (
            Entity,
            &Signal<Vec<T>>,
            &mut ListRows,
            &DomTarget,
            Option<&mut KPrevItems<T>>,
        ),
        Changed<Signal<Vec<T>>>,
    >,
    row_targets: Query<&DomTarget>,
    commands: Commands,
    pending: ResMut<PendingDomWrites>,
    row_bindings: Res<RowBindings>,
    row_inner: Res<RowInner>,
    row_variants: Res<RowVariantMode>,
) where
    T: Send + Sync + Clone + ToString + PartialEq + 'static,
{
    k_body::<T, DefaultRow, _>(
        &NoRowCtx, q, row_targets, commands, pending, row_bindings, row_inner, row_variants,
    );
}

/// K를 **사이트별 row 모양**으로 monomorphize한 판본 (2026-08-07 신설).
/// 알고리즘은 `list_reconcile_k_system`과 같은 함수 하나(`k_body`)이고, 다른
/// 것은 `R`뿐이다 — 즉 이 변형이 생겼다고 해서 K의 동작이 두 갈래로 갈라지지
/// 않는다. loom-compiler가 `<tag>{item}</tag>`보다 복잡한 row 템플릿을
/// 만났을 때 이쪽을 등록한다.
pub fn list_reconcile_k_system_with<T, R: RowSpawner>(
    q: Query<
        (
            Entity,
            &Signal<Vec<T>>,
            &mut ListRows,
            &DomTarget,
            Option<&mut KPrevItems<T>>,
        ),
        (Changed<Signal<Vec<T>>>, With<R::Marker>),
    >,
    row_targets: Query<&DomTarget>,
    commands: Commands,
    pending: ResMut<PendingDomWrites>,
    row_bindings: Res<RowBindings>,
    row_inner: Res<RowInner>,
    row_variants: Res<RowVariantMode>,
    site: Res<R::Ctx>,
) where
    T: Send + Sync + Clone + ToString + PartialEq + 'static,
{
    k_body::<T, R, _>(
        &site, q, row_targets, commands, pending, row_bindings, row_inner, row_variants,
    );
}

/// **§10 "K: 디스패치 규칙"이 사전 선언한 네 분기.**
///
/// 조문(v1.1 — K-1): "(1) 신·구 시퀀스의 O(n) **위치 대응 동등 스캔**(Item PartialEq —
/// 매칭·검색 없음), (2) 완전 동등 → no-op, (3) 순수 append/truncate(한쪽이 다른 쪽의
/// prefix) → A-보강 count-diff(전량 삭제는 clear_children), (4) **그 외 → 전량
/// despawn+respawn**(스폰 번들 초기값 포함). **값-변경 retain 분기는 두지 않는다**".
///
/// **2026-09-22까지 (4)번밖에 없었다.** 그래서 대조군 K가 append 한 건에도 전 행을
/// 재생성했고, §3.4가 K의 등록된 유리 구간으로 잡아 둔 W5(append-only)와 W9(clear)에서
/// K가 **자기 최선형이 아니라 최악형으로** 측정되고 있었다 — 원칙 1("허수아비 대조군
/// 금지")의 조작화가 바로 이 조문인데 그것이 미이행이었다.
///
/// 위치 대응 동등 스캔은 **매칭이 아니다**(같은 인덱스끼리만 비교하고 탐색이 없다).
/// docs/03 판정례 4의 "대상외 지위"가 그 근거이고, 조문도 "매칭 도입 변형 금지(N5)"를
/// 별도로 못박고 있다. 값-변경 retain 분기를 두지 않는 이유도 조문에 있다: 위치 스캔만
/// 으로는 값 변경(W7형)과 순서 변경(W2형)을 **원리적으로 구별 불능**이고, retain 분기를
/// 가진 K는 C와 같은 메커니즘이 되어 대조군 독립성을 잃는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KDispatch {
    /// (2) 완전 동등 → 아무 write도 내지 않는다.
    NoOp,
    /// (3) 순수 append — `old`가 `new`의 prefix. 값은 **새로 스폰할 시작 인덱스**(= 옛 길이).
    Append(usize),
    /// (3) 순수 truncate — `new`가 `old`의 prefix이고 비어 있지 않다. 값은 **남길 길이**.
    Truncate(usize),
    /// (3) 전량 삭제 → `clear_children` 한 번.
    Clear,
    /// (4) 그 외 → 전량 despawn + respawn.
    Full,
}

/// §10의 위치 대응 동등 스캔. O(min(|old|, |new|))이고 탐색·해시·매칭이 없다.
pub fn k_dispatch<T: PartialEq>(old: &[T], new: &[T]) -> KDispatch {
    let common = old.len().min(new.len());
    let mut i = 0;
    while i < common && old[i] == new[i] {
        i += 1;
    }
    if i < common {
        // 같은 위치에서 값이 다르다 → prefix 관계가 아니다.
        return KDispatch::Full;
    }
    if old.len() == new.len() {
        KDispatch::NoOp
    } else if new.is_empty() {
        KDispatch::Clear
    } else if new.len() > old.len() {
        KDispatch::Append(old.len())
    } else {
        KDispatch::Truncate(new.len())
    }
}

/// K가 **직전 시퀀스**를 들고 있는 곳. §10의 위치 대응 동등 스캔에 필요하다.
///
/// 이것이 "매칭 도입"이 아닌 이유: 저장하는 것은 순서 그대로의 값 열뿐이고, 조회는
/// 같은 인덱스끼리의 비교뿐이다(키 → 위치 사상도, 탐색도 없다). D의 `RowKey`나 E의
/// `OrderKey`처럼 **정체성을 추적하는 구조가 아니다**.
#[derive(Component, Debug, Clone)]
pub struct KPrevItems<T: Send + Sync + 'static>(pub Vec<T>);

fn k_body<T, R: RowSpawner, F: bevy_ecs::query::QueryFilter>(
    site: &R::Ctx,
    mut q: Query<
        (
            Entity,
            &Signal<Vec<T>>,
            &mut ListRows,
            &DomTarget,
            Option<&mut KPrevItems<T>>,
        ),
        F,
    >,
    row_targets: Query<&DomTarget>,
    mut commands: Commands,
    mut pending: ResMut<PendingDomWrites>,
    row_bindings: Res<RowBindings>,
    row_inner: Res<RowInner>,
    row_variants: Res<RowVariantMode>,
) where
    T: Send + Sync + Clone + ToString + PartialEq + 'static,
{
    for (list, items, mut rows, anchor, prev) in &mut q {
        // §10 (1) 위치 대응 동등 스캔.
        //
        // **직전 시퀀스를 모르면 분기 (4)다** — 빈 열로 보고 Append(0)로 가면 안 된다.
        // 조문의 (3)은 "한쪽이 다른 쪽의 prefix"라는 **관측된 사실**에 걸린 분기인데,
        // 옛 열이 없으면 그 사실을 주장할 근거가 없다. 그래서 "그 외"가 정직한 분류다.
        //
        // 이게 실제로 중요하다: 컴파일러가 낸 중첩 조건부 사이트는 조건부가 처음
        // 보일 때 초기 row를 **직접 스폰하고 `ListRows`를 시드**한 뒤 이 System에
        // 넘긴다(`crates/loom-nested-fixture/tests/nested.rs`가 그 인수 조건). 그때
        // 빈 열로 보면 이미 DOM에 있는 row를 한 벌 더 스폰해 **중복**이 된다
        // (수리 전 이 경로에서 `a,b,c,a,b,c,new`가 나왔다).
        let dispatch = match prev.as_deref() {
            Some(p) => k_dispatch(&p.0, &items.0),
            None => KDispatch::Full,
        };

        let mut spawn_from = |from: usize,
                              rows: &mut ListRows,
                              commands: &mut Commands,
                              pending: &mut Vec<DomWrite>| {
            for item in items.0.iter().skip(from) {
                let new_entity = commands.spawn_empty().id();
                rows.0.push(new_entity);
                let text = item.to_string();
                let mut out = RowSpawnCtx {
                    commands,
                    pending,
                    row_bindings: row_bindings.0,
                    row_inner: row_inner.0,
                    row_variants: row_variants.0,
                };
                R::emit(site, &mut out, new_entity, anchor.0, &text, None);
            }
        };

        match dispatch {
            // (2) 완전 동등 → write를 하나도 내지 않는다.
            KDispatch::NoOp => {}

            // (3) 순수 append → 꼬리만 스폰한다(A-보강 count-diff).
            KDispatch::Append(from) => {
                spawn_from(from, &mut rows, &mut commands, &mut pending.0);
            }

            // (3) 순수 truncate → 꼬리만 지운다. 핸들은 despawn 전에 읽어야 한다
            // (`DomWrite::RemoveNode`의 doc comment 참고).
            KDispatch::Truncate(keep) => {
                for stale in rows.0.split_off(keep) {
                    if let Ok(target) = row_targets.get(stale) {
                        pending.0.push(DomWrite::RemoveNode { node: target.0 });
                    }
                    commands.entity(stale).despawn();
                }
            }

            // (3) 전량 삭제 → 조문 그대로 `clear_children` **한 번**.
            KDispatch::Clear => {
                for stale in rows.0.drain(..) {
                    commands.entity(stale).despawn();
                }
                pending.0.push(DomWrite::ClearChildren { parent: anchor.0 });
            }

            // (4) 그 외 → 전량 despawn + respawn(스폰 번들 초기값 포함).
            KDispatch::Full => {
                for old_row in rows.0.drain(..) {
                    commands.entity(old_row).despawn();
                }
                pending.0.push(DomWrite::ClearChildren { parent: anchor.0 });
                spawn_from(0, &mut rows, &mut commands, &mut pending.0);
            }
        }

        // 다음 tick의 스캔 기준. 값 열만 들고 있고 정체성 추적은 없다
        // (`KPrevItems`의 doc comment 참고).
        match prev {
            Some(mut p) => {
                p.0.clear();
                p.0.extend_from_slice(&items.0);
            }
            None => {
                commands.entity(list).insert(KPrevItems(items.0.clone()));
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
    /// 앵커/부모 entity가 해석되지 않아 **적용되지 못하고 버려진 write 수**.
    /// 평소에는 0이어야 한다 — write는 방출 순서대로 적용되고 부모/앵커는
    /// 항상 먼저 방출되므로(`DomWrite` 문서). 0이 아니면 flush 규약이 깨진
    /// 것이고, RQ1 §5 트랙 N 불변식 5("앵커 dangling 없음")가 재는 값이다.
    /// 그 전까지 이 실패는 `continue`로 조용히 삼켜지고 있었다.
    pub dropped_writes: usize,
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
        world.init_resource::<RowInner>();
        world.init_resource::<WriteVariant>();
        world.init_resource::<RowVariantMode>();
        // `RowSpawner::Ctx`의 기본값 — `DefaultRow`로 monomorphize된
        // `*_system_with`가 `Res<NoRowCtx>`를 요구한다.
        world.init_resource::<NoRowCtx>();
        Self {
            world,
            schedule: Schedule::default(),
            dropped_writes: 0,
        }
    }

    /// RQ1 row-depth axis: set how many bindings each list row carries
    /// (default 1 = R1). 개정 #9-11 이후 이 값이 1보다 크면 row는 반응
    /// 바인딩 `count-1`개와 중첩 조건부 1개를 갖는다. Must be called
    /// before any list reconcile System runs for it to take effect on that
    /// tick — matches the "configure once, then run" shape every other
    /// LoomApp setup step already follows (`add_systems`).
    pub fn set_row_bindings(&mut self, count: usize) -> &mut Self {
        self.world.insert_resource(RowBindings(count));
        self
    }

    /// §10의 write 변형 축을 고른다(기본 = 조건부, 2026-08-12까지의 유일한 구현).
    pub fn set_write_variant(&mut self, v: WriteVariant) -> &mut Self {
        self.world.insert_resource(v);
        self
    }

    /// **W8(§3.1 "중첩 증폭") 축** — row마다 `<ul>` + `n`개의 `<li>`를 만든다.
    /// 0(기본)이면 내부 리스트를 안 만들고 다른 셀의 방출이 안 바뀐다.
    pub fn set_row_inner(&mut self, count: usize) -> &mut Self {
        self.world.insert_resource(RowInner(count));
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

    /// **R2(§3.2) row 깊이 축의 patch System 묶음**을 등록한다(개정 #9-11).
    /// `set_row_bindings(n>1)`인 앱만 부르면 되고, 부르지 않으면 스케줄에 아무것도
    /// 안 붙으므로 R1 셀의 실행 경로가 한 System도 달라지지 않는다.
    ///
    /// **sync point 뒤에 놓는 것이 요점이다**: 후보 C의 `RowSpawner::revalue`가
    /// `Commands`로 `Signal`을 갈아 끼우므로, 그 변경 표시를 같은 tick에 보려면
    /// `ApplyDeferred` 이후여야 한다. 그리고 `systems_ms`(§7.1이 E 기각 (ii)의
    /// "매칭 비용"으로 정의한 구간) 밖이어야 그 조항이 재려는 양이 안 오염된다 —
    /// 그래서 자기 구간(`row_patch_ms`)을 따로 잰다.
    /// **전제**: 후보 System은 `add_instrumented_systems`로 등록돼 있어야 한다.
    /// 아래 순서 제약이 그쪽이 놓는 `stage_mark_sync_end`를 기준점으로 삼기
    /// 때문이고, 그것이 곧 "sync point 뒤"의 유일한 표현이다. 측정 경로가
    /// 그 등록 방식을 쓰므로 테스트도 같은 방식을 써야 한다 — 다른 경로로
    /// 검증하면 그 자체가 계측과 판정 대상의 어긋남이다.
    pub fn add_row_depth_systems(&mut self) -> &mut Self {
        assert!(
            self.world.get_resource::<StageTimings>().is_some(),
            "add_row_depth_systems는 add_instrumented_systems 뒤에 불러야 한다 \
             (순서 기준점이 stage_mark_sync_end다)"
        );
        self.schedule.add_systems(
            (
                stage::stage_mark_row_patch_start,
                (patch_row_binding_system, cond::cond_reconcile_b_system),
                stage::stage_mark_row_patch_end,
            )
                .chain_ignore_deferred()
                .after(stage::stage_mark_sync_end),
        );
        self
    }

    /// **D 무조건 변형(G0R1)의 row_patch 창만** 등록한다 — 표식 System 둘뿐이고 `stage_mark_sync_end` 뒤에 건다.
    /// `add_row_depth_systems`(R2/R3의 patch 묶음 + 창)는 바이트 그대로 두고, R1에서 D 무조건의 `SetText`
    /// 방출을 `row_patch_ms`에 귀속시키기 위한 창을 따로 둔다. 개정 #12 계측 대칭 규칙: **R1 × 무조건 셀에서는 그 셀의
    /// 전 후보에 이 창을 등록한다**(D만 표식 비용을 지는 비대칭 제거). 같은 앱에서 `add_row_depth_systems`와 함께
    /// 부르면 창이 중복된다 — `RowPatchWindow` 자원으로 이 함수끼리의 중복은 막고, depth와의 중복은 하네스 규칙이다.
    pub fn add_row_patch_window(&mut self) -> &mut Self {
        assert!(
            self.world.get_resource::<StageTimings>().is_some(),
            "add_row_patch_window는 add_instrumented_systems 뒤에 불러야 한다 (순서 기준점이 stage_mark_sync_end다)"
        );
        assert!(
            self.world.get_resource::<RowPatchWindow>().is_none(),
            "row_patch 창이 이미 등록돼 있다"
        );
        self.world.insert_resource(RowPatchWindow);
        self.schedule.add_systems(
            (stage::stage_mark_row_patch_start, stage::stage_mark_row_patch_end)
                .chain_ignore_deferred()
                .after(stage::stage_mark_sync_end),
        );
        self
    }

    /// `patch_row_value_system::<T>`를 row_patch 창 안에 끼운다. 창은 만들지 않는다 —
    /// `add_row_patch_window` 또는 `add_row_depth_systems`가 먼저 창을 만들어야 한다.
    pub fn add_row_value_patch<T: Send + Sync + ToString + 'static>(&mut self) -> &mut Self {
        assert!(
            self.world.get_resource::<StageTimings>().is_some(),
            "add_row_value_patch는 add_instrumented_systems 뒤에 불러야 한다"
        );
        self.schedule.add_systems(
            list_key::patch_row_value_system::<T>
                .after(stage::stage_mark_row_patch_start)
                .before(stage::stage_mark_row_patch_end),
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
    /// **테스트 전용**: 스케줄만 돌리고 write 큐는 그대로 둔다.
    /// `tick`은 큐를 비우며 DOM에 적용하므로, "무엇이 방출됐는가"를 그대로
    /// 보려면 이 경로가 필요하다(`tests/row_spawner.rs` — `DefaultRow`의
    /// write-동일성 검사). 프로덕션 경로에서는 쓰지 말 것.
    #[doc(hidden)]
    pub fn run_schedule_for_test(&mut self) {
        self.schedule.run(&mut self.world);
    }

    pub fn tick<D: DomOps>(&mut self, dom: &mut D) {
        // §7.1의 "DOM 배치 적용 구간"은 스케줄 **바깥**이라 마커 System으로는
        // 잴 수 없다 — 여기서 직접 스탬프한다. `StageTimings`가 없는 앱
        // (= `add_instrumented_systems`를 안 쓴 전부)에서는 아래 저장이
        // 통째로 생략되므로, 남는 비용은 시계 호출 3회(~100ns)뿐이다.
        let t0 = stage::now_ms();
        self.schedule.run(&mut self.world);
        let t_schedule_end = stage::now_ms();
        let writes = std::mem::take(&mut self.world.resource_mut::<PendingDomWrites>().0);
        dom.begin_batch();
        for write in writes {
            match write {
                DomWrite::SpawnElement {
                    entity,
                    parent,
                    tag,
                    class,
                    loom_id,
                    text,
                } => {
                    let Some(parent_handle) = self.resolve_parent(parent) else {
                        self.dropped_writes += 1;
                        continue;
                    };
                    let handle = dom.create_element(parent_handle, tag, class);
                    if let Some(id) = loom_id {
                        dom.set_attribute(handle, "data-loom-id", id);
                    }
                    if let Some(text) = text {
                        dom.set_text_content(handle, &text);
                    }
                    self.world.entity_mut(entity).insert(DomTarget(handle));
                }
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
                        self.dropped_writes += 1;
                        continue;
                    };
                    let before_handle = before.and_then(|e| self.dom_target(e));
                    dom.reposition(parent, handle, before_handle);
                }
                DomWrite::RemoveNode { node } => dom.remove_node(node),
                DomWrite::SetVisible { node, visible } => dom.set_visible(node, visible),
                DomWrite::DetachToFragment { block, node } => {
                    let handle = dom.detach_to_fragment(node);
                    self.world.entity_mut(block).insert(DetachedFragment(handle));
                }
                DomWrite::AttachFromFragment { block, parent } => {
                    let Some(frag) = self.world.get::<DetachedFragment>(block).copied() else {
                        self.dropped_writes += 1;
                        continue;
                    };
                    dom.attach_from_fragment(parent, frag.0);
                    self.world.entity_mut(block).remove::<DetachedFragment>();
                }
                DomWrite::AppendStaticText { parent, text } => match self.resolve_parent(parent) {
                    Some(handle) => dom.append_static_text(handle, &text),
                    None => self.dropped_writes += 1,
                },
                DomWrite::AppendStaticHtml { parent, html } => match self.resolve_parent(parent) {
                    Some(handle) => dom.append_static_html(handle, &html),
                    None => self.dropped_writes += 1,
                },
                DomWrite::SetInputValue { node, value } => dom.set_input_value(node, &value),
                DomWrite::Surface(w) => surface::apply_surface(self, dom, w),
            }
        }
        // **Bevy의 removed-component 이중 버퍼를 비운다** (2026-08-13 수리).
        //
        // Bevy는 despawn마다 `RemovedComponents` 메시지를 버퍼에 넣는다. 그 버퍼는
        // 이중 버퍼라 매 프레임 `World::clear_trackers()`가 스왑·비움을 해야 하는데,
        // 그 호출은 Bevy의 `App`이 한다. `LoomApp`은 `App`을 안 쓰고 맨 `World` +
        // `Schedule`을 직접 돌리므로 **그 호출이 없었고**, despawn한 만큼 버퍼가
        // 영원히 자랐다.
        //
        // 실제로 터졌다: v21 트랙 L 수집이 `W5 append × R2` 셀에서 90초 만에 wasm
        // `unreachable` 트랩으로 죽었고, 프로파일링 빌드의 스택이
        // `despawn → Messages::write_with_caller → RawVec::grow_one →
        //  handle_alloc_error → rust_oom`을 정확히 가리켰다.
        //
        // **개정이 만든 결함이 아니라 임계를 넘긴 것이다**: 개정 #9-1이 W5를 조항대로
        // 고쳐 한 셀의 despawn을 수십만 건으로 만들었고, #9-11의 R2가 row당 entity를
        // 9배로 만들어 그 수를 다시 곱했다.
        //
        // 측정에 미치는 영향: `clear_trackers`는 removal 큐 스왑과 `last_change_tick`
        // 증가뿐이고, 시스템별 `Changed`/`Added` 감지는 **시스템 자신의 `last_run`**
        // 으로 판정되므로 영향이 없다(`tests/despawn_buffer.rs`와 기존 게이팅 검사가
        // 함께 고정한다). 비용은 tick당 상수다.
        self.world.clear_trackers();

        let t_end = stage::now_ms();
        if let Some(mut timings) = self.world.get_resource_mut::<StageTimings>() {
            timings.dom_apply_ms = t_end - t_schedule_end;
            timings.total_ms = t_end - t0;
        }
    }

    fn dom_target(&self, entity: Entity) -> Option<NodeHandle> {
        self.world.get::<DomTarget>(entity).map(|t| t.0)
    }

    /// `WriteParent`를 실제 핸들로. `Entity` 쪽이 `None`이 되는 경우는 부모를
    /// 만드는 write가 같은 배치의 **앞**에 없었다는 뜻이므로 방출 측 버그다 —
    /// 여기서 조용히 넘기고(`continue`) 자식만 유실시키는 대신, 나머지 배치는
    /// 계속 적용한다(다른 arm들의 `dom_target` 실패 처리와 같은 관례).
    fn resolve_parent(&self, parent: WriteParent) -> Option<NodeHandle> {
        match parent {
            WriteParent::Node(handle) => Some(handle),
            WriteParent::Entity(entity) => self.dom_target(entity),
        }
    }
}

// G0R1(개정 #12): key 표현·값 채널·radix 병합 매칭. 새 코드를 별도 모듈에 둔다 — list.rs 줄 번호 보존.
pub mod list_key;

// Phase 7c(`DomWrite::Surface`): 최소 런타임 표면. 새 코드를 별도 모듈에 둔다 — 기존 소스 줄 번호 보존.
pub mod surface;
pub use surface::{
    patch_attr_system, patch_bool_prop_system, patch_class_toggle_system, patch_opt_attr_system, patch_style_system,
    patch_text_node_system, AttrSite, AttrTarget, BoolProp, BoolPropSite, ClassSite, ElementSpec, Namespace, StyleSite,
    SurfaceStats, SurfaceWrite, TextNodeTarget,
};
