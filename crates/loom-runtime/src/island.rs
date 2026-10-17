//! Phase 3 — Island supervision (RQ3) + the shared registry both RQ3 and
//! RQ4 build on. Scope, stated plainly: this module is the *native*, pure
//! state-machine half of RQ3's design — it proves the lifecycle rules
//! (research-questions/RQ3-island-supervision-interaction.md) hold as
//! actual, tested code, the same way RQ1 Track L/T started as native
//! `list.rs`/`cond.rs` mechanisms before any compiler integration existed.
//! The *authoritative*, wasm-target Registry is NOT this Rust type — see
//! `IslandRegistryBackend`'s doc comment for why, and `demo/resume/
//! registry.js` for the real one.
//!
//! ## Why a separate `bevy_ecs::World` per Island, not a shared one
//! (roadmap/PHASES.md Phase 3 status has the full finding; summarized here
//! since it's why this module has no `bevy_ecs` dependency at all.) Two
//! Islands compiled to separate chunks are, by construction, separate wasm
//! *instances* with disjoint linear memory — `bootstrap.js` already lazy-
//! loads each LRef's chunk via a fresh dynamic `import()`. No Rust
//! reference, `Entity`, or `Box<dyn Trait>` can cross that boundary; only
//! wasm-bindgen-marshalled values (numbers, strings, `JsValue`) can. So
//! "one `bevy_ecs::World` per Island" isn't a design choice made *here* —
//! it's the only shape that's even possible once chunking exists, and this
//! module's types (all POD, all `NodeHandle`/`IslandId`-based, never an
//! `Entity`) reflect that: nothing in here reaches into any Island's own
//! `LoomApp`/`World` at all.
//!
//! **2026-10-17 정정(지위)**: 위 두 문단은 "Island이 항상 있다"는 Phase 3
//! 전제 위에서 쓰였다. RQ6가 2026-07-28 H-Qwik(강한 형태)으로 확정되면서
//! 기본 모델은 **페이지 전체가 하나의 World**이고(docs/04), Island 경계는
//! "지연 activation 경계 세분화"를 선택했을 때만 생기는 선택적 최적화가 됐다.
//! "Island마다 World 하나"라는 논증 자체(청크가 갈리면 wasm 인스턴스가 갈린다)는
//! 여전히 맞지만, 그 전제인 "청크가 Island마다 갈린다"가 이제 기본값이 아니다.
//! 첫 줄의 "RQ4 build on"도 낡았다 — RQ4는 같은 날 전면 폐기됐고, 이
//! 레지스트리에서 남은 쓰임은 RQ3 쪽 supervision과 crash 통지 채널
//! (`__island_state:<id>`, Task #28-b)이다. RQ3는 부분 폐기 상태라 여기 규칙들은
//! 세분화를 택할 때 적용되고, 2단계 detach·promote-once-and-stay는 기본 모델에도
//! 적용되지만 지위는 여전히 "잠정 규칙"이다.

use crate::NodeHandle;
use std::collections::HashMap;

pub type IslandId = u32;

/// RQ3's 6-state actor lifecycle (that doc's "Lean4 ActorState 영향" section;
/// `formal-verification/lean4/SupervisionTree.stub.lean`'s `ActorState` is
/// the formal spec of exactly this enum — variant names kept identical on
/// purpose so any future drift between the Lean stub and this real
/// implementation is visible as a naming mismatch, not silent). Transitions
/// are one-way (see `NativeIslandRegistry`'s methods) — there is no
/// `Attached -> Dormant` because promotion is irreversible (RQ3 Q4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandState {
    /// DOM gone, Registry entry either removed or kept only for cascade
    /// bookkeeping — the terminal state every other state eventually reaches.
    Detached,
    /// DOM exists, LRef is armed, no actor spawned yet — the resumable
    /// "waiting" state RQ3 added specifically so a never-yet-clicked Island
    /// is still findable by a detach cascade (the bug this fixes: dormant
    /// islands used to be missing from containment lookups entirely).
    Dormant,
    /// Activation in progress (chunk import + wasm init + constructor
    /// running) — not yet observable as a *result*, just as "not done yet".
    Attaching,
    /// A live actor exists and stays until this Island detaches
    /// (promote-once-and-stay, RQ3 Q4).
    Attached,
    /// Synchronous detach has been recorded; DOM access/patch dispatch for
    /// this Island's tree is cut off from this point on, but async cleanup
    /// (if any) hasn't necessarily finished — RQ3's 2-phase detach protocol.
    Detaching,
    /// A crash escalated all the way to the Island root. No-restart policy
    /// (RQ3 Q4): this state absorbs the fault, DOM stays frozen/inert, and
    /// recovery is left to a full page reload — not handled here.
    Failed,
}

/// RQ3's IslandRegistry schema, kept deliberately **closed** (that doc's
/// "스키마 폐쇄" clause) — no arbitrary payload fields, specifically so RQ4
/// can't turn this into a shared-state attachment point (see this module's
/// doc comment and RQ4's "IslandRegistry 스키마 폐쇄 경계" note). POD, no
/// closures (CLAUDE.md 금지 4).
#[derive(Debug)]
pub struct IslandRegistryEntry {
    pub id: IslandId,
    pub host: NodeHandle,
    pub state: IslandState,
    pub actor: Option<IslandInstanceHandle>,
}

/// Opaque reference to a live actor. `JsHandle` is what the wasm-target
/// Registry (`demo/resume/registry.js`) actually holds — a JS-side instance
/// id, since the real actor object lives in a *different wasm instance*
/// than whatever holds the Registry (see this module's doc comment).
/// `Native` is Phase 4's native-embedding counterpart: with no wasm
/// instance boundary to cross there, an actual Rust trait object can be
/// held directly.
pub enum IslandInstanceHandle {
    JsHandle(u32),
    Native(Box<dyn IslandInstance>),
}

impl std::fmt::Debug for IslandInstanceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IslandInstanceHandle::JsHandle(id) => f.debug_tuple("JsHandle").field(id).finish(),
            IslandInstanceHandle::Native(_) => f.debug_tuple("Native").field(&"<opaque>").finish(),
        }
    }
}

/// The synchronous half of RQ3's 2-phase detach protocol for a live actor:
/// mark it detaching and cut off further DOM/World access *before* the DOM
/// is actually torn down. The async cleanup half (structural-concurrency
/// cancellation of in-flight work) is explicitly out of Phase 3's minimal
/// slice — RQ3's "알려진 약점" #5 (timeout semantics still undecided).
pub trait IslandInstance {
    fn begin_detach(&mut self);
}

/// The Registry itself — RQ3's "어떤 IslandWorld/supervision tree에도 속하지
/// 않는 런타임 인스턴스당 1개의 중립 구조". **On wasm, the authoritative
/// implementation of this is not a Rust type at all** — it's a plain JS
/// object (`demo/resume/registry.js`), the same way `bootstrap.js` already
/// tracks `el._loomInstance` on the DOM element itself: every Island's wasm
/// instance calls into it via `wasm_bindgen` `extern` imports, because a
/// Rust `HashMap` living inside *one* Island's wasm instance is invisible to
/// every other Island's instance (see this module's doc comment). This
/// trait exists so `NativeIslandRegistry` (this file) can implement it for
/// two purposes that both happen to have no cross-instance boundary to
/// cross: this module's own native tests (exercising RQ3 Q1-Q4 as a pure
/// state machine, no DOM/JS involved), and Phase 4's native embedding path.
///
/// **Deliberately flat — no parent/child bookkeeping at all** (RQ3 Q1/Q2:
/// "island 경계를 넘는 supervision edge는 0개", "입양 없음, 영구 별도
/// tree"): every method takes exactly one `IslandId` and touches exactly one
/// entry. Detaching one Island never cascades to another *inside this
/// trait* — a real detach cascade (removing a DOM subtree containing
/// several Island hosts) is the *caller's* job, by calling `begin_detach`
/// once per Island it determines (via DOM containment, not via this
/// Registry) is inside the removed range. This is what "supervision edge
/// count = 0" means as executable behavior, not just as a design sentence.
pub trait IslandRegistryBackend {
    /// RQ3: "등록 시점: SSR 마커 스캔/로드 시점" — not at resume/activation
    /// time. Starts the Island at `Dormant`.
    fn register(&mut self, id: IslandId, host: NodeHandle);
    fn entry(&self, id: IslandId) -> Option<&IslandRegistryEntry>;
    fn state(&self, id: IslandId) -> Option<IslandState> {
        self.entry(id).map(|e| e.state)
    }
    fn contains(&self, id: IslandId) -> bool {
        self.entry(id).is_some()
    }
    /// `Dormant -> Attaching` — activation started (LRef fired or a
    /// `client:` condition became true), actor not live yet.
    fn begin_attach(&mut self, id: IslandId);
    /// `Attaching -> Attached` — activation finished successfully.
    fn mark_attached(&mut self, id: IslandId, actor: IslandInstanceHandle);
    /// `Attaching -> Failed` (load/deserialize failure) or `Attached ->
    /// Failed` (crash, no-restart policy — RQ3 Q4).
    fn mark_failed(&mut self, id: IslandId);
    /// RQ3 Q3's 2-phase detach, synchronous half. `Dormant -> Detached` and
    /// `Failed -> Detached` go **directly** ("actor 비경유 환경 전이" — no
    /// live actor exists to notify). `Attached -> Detaching` and `Attaching
    /// -> Detaching` (in-flight activation's result gets discarded) instead
    /// — those need `finish_detach` to actually reach `Detached`.
    fn begin_detach(&mut self, id: IslandId);
    /// The async half completing: `Detaching -> Detached`.
    fn finish_detach(&mut self, id: IslandId);
}

/// The only Rust implementor of `IslandRegistryBackend` — see that trait's
/// doc comment for the two cases where a same-instance Rust struct is
/// actually valid (native tests, Phase 4 native embedding).
#[derive(Debug, Default)]
pub struct NativeIslandRegistry {
    entries: HashMap<IslandId, IslandRegistryEntry>,
}

impl NativeIslandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn entry_mut(&mut self, id: IslandId) -> &mut IslandRegistryEntry {
        self.entries
            .get_mut(&id)
            .unwrap_or_else(|| panic!("loom-runtime: island {id} was never registered"))
    }
}

impl IslandRegistryBackend for NativeIslandRegistry {
    fn register(&mut self, id: IslandId, host: NodeHandle) {
        assert!(
            !self.entries.contains_key(&id),
            "loom-runtime: island {id} registered twice"
        );
        self.entries.insert(
            id,
            IslandRegistryEntry {
                id,
                host,
                state: IslandState::Dormant,
                actor: None,
            },
        );
    }

    fn entry(&self, id: IslandId) -> Option<&IslandRegistryEntry> {
        self.entries.get(&id)
    }

    fn begin_attach(&mut self, id: IslandId) {
        let entry = self.entry_mut(id);
        assert!(
            entry.state == IslandState::Dormant,
            "loom-runtime: island {id} began attaching from {:?}, expected Dormant",
            entry.state
        );
        entry.state = IslandState::Attaching;
    }

    fn mark_attached(&mut self, id: IslandId, actor: IslandInstanceHandle) {
        let entry = self.entry_mut(id);
        assert!(
            entry.state == IslandState::Attaching,
            "loom-runtime: island {id} marked attached from {:?}, expected Attaching",
            entry.state
        );
        entry.state = IslandState::Attached;
        entry.actor = Some(actor);
    }

    fn mark_failed(&mut self, id: IslandId) {
        let entry = self.entry_mut(id);
        assert!(
            matches!(entry.state, IslandState::Attaching | IslandState::Attached),
            "loom-runtime: island {id} marked failed from {:?}, expected Attaching or Attached",
            entry.state
        );
        entry.state = IslandState::Failed;
        entry.actor = None;
    }

    fn begin_detach(&mut self, id: IslandId) {
        let entry = self.entry_mut(id);
        entry.state = match entry.state {
            // No live actor to notify — the environment (host glue) handles
            // this transition directly (RQ3: "actor 비경유 환경 전이").
            IslandState::Dormant | IslandState::Failed => IslandState::Detached,
            // A live (or in-flight) actor gets a synchronous detaching mark;
            // `finish_detach` completes it once cleanup is done.
            IslandState::Attaching | IslandState::Attached => {
                if let Some(IslandInstanceHandle::Native(actor)) = entry.actor.as_mut() {
                    actor.begin_detach();
                }
                IslandState::Detaching
            }
            IslandState::Detaching | IslandState::Detached => {
                panic!("loom-runtime: island {id} began detach twice (already {:?})", entry.state)
            }
        };
    }

    fn finish_detach(&mut self, id: IslandId) {
        let entry = self.entry_mut(id);
        assert!(
            entry.state == IslandState::Detaching,
            "loom-runtime: island {id} finished detach from {:?}, expected Detaching",
            entry.state
        );
        entry.state = IslandState::Detached;
        entry.actor = None;
    }
}
