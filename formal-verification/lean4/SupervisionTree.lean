-- SupervisionTree.lean
-- Promoted from SupervisionTree.stub.lean (2026-07-24, Phase 5) — RQ5 후보 #1's
-- first real theorems. Scope, stated plainly: this formalizes exactly the
-- state machine `crates/loom-runtime/src/island.rs`'s `NativeIslandRegistry`
-- actually implements (same transition names, same guards, verified by
-- reading that file side-by-side while writing this one) — NOT yet the full
-- "actor state machine × Entity/DOM ownership" composition RQ3's own modeling
-- note below says is the real target. That composition needs `SupervisionNode`
-- to carry real tree/Entity structure, which still doesn't exist here (see
-- the TODO at the bottom) — so what's proved below is strictly narrower than
-- RQ3's full teardown-safety claim: it proves the *actor lifecycle labels*
-- can never re-enable access after a detach begins, not that the ECS/DOM
-- layer actually stops touching a specific dead Entity. That second half
-- requires modeling `PendingDomWrites`/`Commands`/System scheduling, which
-- is future work, honestly left open here rather than silently assumed.
--
-- 2026-07-23: RQ3 잠정 규칙 확정에 따라 ActorState를 4개 → 6개로 확장 (dormant, failed 추가).
-- 이 상태 집합은 RQ3 잠정 규칙의 형식화 대상 스펙이며, 규칙이 잠정인 만큼 이 스펙도 잠정임
-- (RQ3의 "Phase 3 재검토 트리거" 발동 시 함께 개정).
--
-- RQ3의 2단계 detach 프로토콜에 맞춰 두 정리로 분리됨:
--   (a) 동기 안전성 → `beginDetach_forbids_access` +
--       `no_transition_restores_access_from_detaching_or_detached`
--   (b) 진행성 → `detach_reaches_detached_in_at_most_two_steps`

namespace Loom.Supervision

/-- Island/컴포넌트 actor의 lifecycle 상태. RQ3 잠정 규칙(2026-07-23)의 6-상태 기계.
    dormant = DOM 존재 + LRef 무장 + actor 부재 (resumable 대기 — 06의 지연 spawn 상태.
              구 스텁 주석이 예견한 "Detached와 Attached 사이 추가 상태"가 이것)
    failed  = fault 흡수 상태 (no-restart 정책. DOM은 동결된 채 존재, Registry 항목이
              남아 상위 cascade 시 정리 대상 — detached와 구분 필수)
    전이 (역전이 없음 — attached→dormant 부재 = "승격은 비가역"의 상태기계 표현):
      dormant   → attaching                    (activation: LRef 발화 또는 client: 조건)
      dormant   → detached                     (환경발 제거 — actor 비경유, Registry 직접 수행.
                                                형식화의 핵심 신규 지점)
      attaching → attached | detaching | failed (detaching: 로드 중 detach 도달, 결과 폐기 /
                                                 failed: 로드·역직렬화 실패)
      attached  → detaching | failed            (failed: crash, no-restart)
      detaching → detached
      failed    → detached                      (상위 cascade 시 정리)
    미결(RQ1 접속점): 클라이언트 태생 island(B-보강 SpawnTable 경유)의 진입 경로는
    detached → attaching으로 잠정 처리. -/
inductive ActorState where
  | detached
  | dormant
  | attaching
  | attached
  | detaching
  | failed
  deriving Repr, DecidableEq

/-- `crates/loom-runtime/src/island.rs`'s `IslandRegistryBackend::begin_attach`
    (`Dormant -> Attaching`). Every other source state is `none` — Rust's
    version `assert!`s and panics there, which we model as "this transition
    is simply not defined for that state," matching the assert's role as an
    internal-invariant check on correct callers, not a recoverable case. -/
def beginAttach : ActorState → Option ActorState
  | .dormant => some .attaching
  | _ => none

/-- Mirrors `mark_attached` (`Attaching -> Attached`). -/
def markAttached : ActorState → Option ActorState
  | .attaching => some .attached
  | _ => none

/-- Mirrors `mark_failed` (`Attaching -> Failed` load/deserialize failure,
    or `Attached -> Failed` crash, no-restart). -/
def markFailed : ActorState → Option ActorState
  | .attaching => some .failed
  | .attached => some .failed
  | _ => none

/-- Mirrors `begin_detach`'s exact case split: `Dormant`/`Failed` skip
    straight to `Detached` ("actor 비경유 환경 전이" — no live actor to
    notify), `Attaching`/`Attached` land on `Detaching` (2-phase protocol's
    synchronous half), and `Detaching`/`Detached` are `none` (Rust panics:
    "began detach twice"). -/
def beginDetach : ActorState → Option ActorState
  | .dormant => some .detached
  | .failed => some .detached
  | .attaching => some .detaching
  | .attached => some .detaching
  | .detaching => none
  | .detached => none

/-- Mirrors `finish_detach` (`Detaching -> Detached`, the 2-phase protocol's
    async half completing). -/
def finishDetach : ActorState → Option ActorState
  | .detaching => some .detached
  | _ => none

/-- Whether a live (or in-flight) actor could possibly be executing/
    dispatching against this Island's World/DOM right now — the property
    RQ3's "동기 안전성" goal is actually about ("어떤 System도 ... 접근을 새로
    시작할 수 없다"). Only `attached` and `attaching` (mid-load, e.g. an
    `IslandInstance` already constructed but not yet marked `Attached`) can
    have this; every other state structurally cannot, since no actor object
    exists yet or anymore. -/
def actorMayAccess : ActorState → Bool
  | .attached => true
  | .attaching => true
  | _ => false

/-- **RQ3 §Q3 동기 안전성, part 1**: any state `beginDetach` successfully
    transitions out of immediately lands somewhere `actorMayAccess` is
    false. This is the base case the two theorems below build on. -/
theorem beginDetach_forbids_access (s s' : ActorState) (h : beginDetach s = some s') :
    actorMayAccess s' = false := by
  cases s <;> cases s' <;> simp_all [beginDetach, actorMayAccess]

/-- **RQ3 §Q3 동기 안전성, part 2 (the actual "no System ever begins new
    access again" claim)**: once a state is `detaching` or `detached`, EVERY
    registry transition defined for it (there are only two:
    `beginDetach`/`finishDetach` — `beginAttach`/`markAttached`/`markFailed`
    are all `none` from these states by construction, so vacuously satisfy
    the implication) can only ever land on another `actorMayAccess = false`
    state. Combined with `beginDetach_forbids_access`, this shows the
    "no access" property is not just true at the instant of detach but
    stays true for every subsequent step forever — there is no sequence of
    calls that restores `actorMayAccess`. -/
theorem no_transition_restores_access_from_detaching_or_detached
    (s : ActorState) (h : s = .detaching ∨ s = .detached) (s' : ActorState)
    (hs' : beginAttach s = some s' ∨ markAttached s = some s' ∨
           markFailed s = some s' ∨ beginDetach s = some s' ∨
           finishDetach s = some s') :
    actorMayAccess s' = false := by
  cases s' <;> rcases h with h | h <;> subst h <;>
    rcases hs' with h1 | h1 | h1 | h1 | h1 <;>
    simp_all [beginAttach, markAttached, markFailed, beginDetach, finishDetach, actorMayAccess]

/-- **RQ3 §Q3 진행성**: from any state where `beginDetach` is defined at all,
    it reaches `detached` in at most two registry calls — either directly
    (`Dormant`/`Failed` case) or via one `finishDetach` (`Attaching`/
    `Attached` case). No unbounded waiting, no loop — matching the actual
    implementation, which has no genuine async cleanup step yet to wait on
    (RQ3's "알려진 약점" #5: real timeout semantics for a future async
    cleanup phase are still undecided; this theorem is only as strong as
    today's synchronous-everything implementation). -/
theorem detach_reaches_detached_in_at_most_two_steps (s : ActorState)
    (h : beginDetach s ≠ none) :
    ∃ s', beginDetach s = some s' ∧ (s' = .detached ∨ finishDetach s' = some .detached) := by
  cases s <;> simp_all [beginDetach, finishDetach] <;> decide

/-- Sanity check the theorems aren't vacuous over an empty transition
    relation: every state *other than* `detached` has *some* defined
    transition somewhere among the five functions (otherwise
    `no_transition_restores_access...` would hold trivially for
    uninteresting reasons). `detached` is correctly the one true absorbing
    state — no transition function is defined for it at all, matching the
    real implementation (once detached, a fresh `register` is the only way
    back in, and that's a different Island id, not a transition of this
    one). Not a safety property, just a guard against the proofs above
    being accidentally vacuous. -/
example : ∀ s : ActorState, s ≠ .detached →
    beginAttach s ≠ none ∨ markAttached s ≠ none ∨ markFailed s ≠ none ∨
    beginDetach s ≠ none ∨ finishDetach s ≠ none := by
  intro s hs
  cases s <;> simp_all [beginAttach, markAttached, markFailed, beginDetach, finishDetach]

/-- `detached` really is a dead end: every transition function returns
    `none` for it. Stated separately so this fact is visible as a named
    claim, not just an exclusion in the example above. -/
example :
    beginAttach .detached = none ∧ markAttached .detached = none ∧
    markFailed .detached = none ∧ beginDetach .detached = none ∧
    finishDetach .detached = none := by
  simp [beginAttach, markAttached, markFailed, beginDetach, finishDetach]

/-- 아직 정의되지 않음: 트리 구조, 부모-자식 관계, Entity 참조 매핑.
    RQ3 잠정 규칙에 따른 모델링 방침(2026-07-23):
    - children은 island **내부** tree만 다룬다 — island 경계를 넘는 supervision edge는
      0개(supervision forest)이므로, 중첩 island 관계는 이 구조가 아니라 별도의
      IslandRegistry 상태기계(containment 관계)로 모델링한다.
    - 따라서 증명 대상은 "단일 tree 불변식"이 아니라 actor 상태기계 × Registry 상태기계의
      합성이다 (형식화 부담이 줄어든 것이 아니라 이동한 것).
    **2026-07-24 갱신**: 위 세 정리는 딱 "actor 상태기계" 절반만 다룬다 — `SupervisionNode`가
    실제 Entity/DOM 소유권을 담기 전까지는 "detach 이후 어떤 System도 죽은 Entity를
    참조하지 않는다"는 RQ5 #1의 원래 문장 전체를 증명한 게 아니라, 그 문장이 성립하기
    위한 필요조건(actor 라벨이 절대 되살아나지 않음)만 증명한 것 — 이 구분을 숨기지
    않는다. Entity 소유권 모델링은 Layer 2 스펙(docs/03)이 Entity 그래프 소유권을
    더 안정적으로 확정한 뒤 진행. -/
structure SupervisionNode where
  state : ActorState
  -- children : List SupervisionNode  -- island 내부 tree 한정 (위 방침 참고), 구현 안정화까지 보류
  -- ownedEntities : List EntityId    -- ECS World와의 연결점, Layer 2 스펙 확정 후 추가

-- TODO: Entity/DOM 소유권을 SupervisionNode에 채운 뒤, 위 actor-label 정리들을
-- "detach 이후 실제로 어떤 System도 그 Entity에 접근 안 함"으로 승격.

end Loom.Supervision
