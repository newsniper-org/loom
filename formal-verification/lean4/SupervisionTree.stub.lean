-- SupervisionTree.stub.lean
-- 스켈레톤. 정리(theorem)와 증명 없음. RQ5 후보 #1, Phase 1 프로토타입 안정화 후 본격 착수.
--
-- 2026-07-23: RQ3 잠정 규칙 확정에 따라 ActorState를 4개 → 6개로 확장 (dormant, failed 추가).
-- 이 상태 집합은 RQ3 잠정 규칙의 형식화 대상 스펙이며, 규칙이 잠정인 만큼 이 스펙도 잠정임
-- (RQ3의 "Phase 3 재검토 트리거" 발동 시 함께 개정).
--
-- 목표(아직 형식화 안 됨) — RQ3의 2단계 detach 프로토콜에 맞춰 두 정리로 분리됨:
--   (a) 동기 안전성: detaching 표시 이후 어떤 System도 해당 tree의 DOM/Entity 접근을
--       새로 시작할 수 없다
--   (b) 진행성: detaching 진입 후 유한 단계(타임아웃 상계) 내 detached 도달

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

/-- 아직 정의되지 않음: 트리 구조, 부모-자식 관계, Entity 참조 매핑.
    RQ3 잠정 규칙에 따른 모델링 방침(2026-07-23):
    - children은 island **내부** tree만 다룬다 — island 경계를 넘는 supervision edge는
      0개(supervision forest)이므로, 중첩 island 관계는 이 구조가 아니라 별도의
      IslandRegistry 상태기계(containment 관계)로 모델링한다.
    - 따라서 증명 대상은 "단일 tree 불변식"이 아니라 actor 상태기계 × Registry 상태기계의
      합성이다 (형식화 부담이 줄어든 것이 아니라 이동한 것).
    Phase 1 프로토타입에서 구조가 안정화되면 채울 것. -/
structure SupervisionNode where
  state : ActorState
  -- children : List SupervisionNode  -- island 내부 tree 한정 (위 방침 참고), 구현 안정화까지 보류
  -- ownedEntities : List EntityId    -- ECS World와의 연결점, Layer 2 스펙 확정 후 추가

-- TODO(Phase 1 안정화 후): sync_safety_after_detaching : ∀ node, ... (RQ3 목표 (a), 정리 없음)
-- TODO(Phase 1 안정화 후): detaching_terminates : ∀ node, ... (RQ3 목표 (b), 정리 없음)
-- TODO: detach_cascades_fully / no_dangling_entity_after_detach 는 위 두 정리로 분해·대체 예정

end Loom.Supervision
