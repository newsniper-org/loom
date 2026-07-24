# 공식 검증 스코프

RQ5 참고 후 읽을 것.

## 2026-07-24 (Phase 5) — 세 후보 전부 실제로 착수·완료

이전까지는 이 디렉터리 전체가 **스켈레톤**(증명 없음)이었음. Phase 5에서 RQ5의 세
후보 전부를 실제로 진행해 결론을 냈음 — 아래가 그 결과.

1. **`lean4/SupervisionTree.lean`** (RQ5 후보 #1, teardown 안전성 — **실제 증명
   완료**, `sorry` 없음, `lean`으로 직접 확인함). `crates/loom-runtime/src/island.rs`의
   `NativeIslandRegistry`가 실제로 구현하는 6-상태 기계(`IslandState`)를 그대로
   모델링하고, RQ3의 2단계 detach가 요구하는 두 정리를 증명:
   - 동기 안전성(`beginDetach_forbids_access` +
     `no_transition_restores_access_from_detaching_or_detached`): detach가
     시작된 이후로는 어떤 후속 상태 전이도 "actor가 접근 중일 수 있음" 상태를
     되살리지 못한다.
   - 진행성(`detach_reaches_detached_in_at_most_two_steps`): 최대 2단계
     (`begin_detach` + 필요시 `finish_detach`)만에 `detached`에 도달 — 무한정
     대기 없음.
   **정직한 스코프 한계**: 이건 "actor lifecycle 라벨" 절반만 증명한 것 —
   `SupervisionNode`가 아직 실제 Entity/DOM 소유권을 담지 않으므로, "detach 이후
   어떤 System도 죽은 Entity를 실제로 참조하지 않는다"는 RQ5 #1 원문 전체가 아니라
   그 필요조건만 증명함. Entity 소유권 모델링은 이후 과제(파일 안에 TODO로 명시).
2. **`tla/SchedulerSafety.stub.tla`** (RQ5 후보 #2, 스케줄러 무충돌성 — **의도적으로
   스텁 유지, TLA+ 증명 안 함**). 실제로 `bevy_ecs` 0.14.2 소스를 읽어 확인한 결과:
   (a) 이 프로젝트 어떤 크레이트도 `multi_threaded` feature를 켜지 않음(default
   feature 아님) — 실제 실행기는 `SingleThreaded` 하나뿐이라 동시 실행 자체가
   없음. (b) 설령 `multi_threaded`를 켜더라도, Bevy의 `MultiThreadedExecutor::
   can_run`이 System 실행 직전에 `archetype_component_access().is_compatible(...)`
   을 무조건 검사함(`ambiguity_detection` 로그 설정과 무관하게 항상 작동하는
   런타임 안전장치) — 겹치는 System은 절대 동시 실행되지 않음. 이 안전성은 Bevy
   자체가 이미 구현·검증한 메커니즘이라 재검증은 스코프 밖(CLAUDE.md 2-1). 근거
   전체는 스텁 파일 안 주석에 파일:라인 단위로 기록.
3. **`lean4/ResumabilitySoundness.lean`** (RQ5 후보 #3, resumability 건전성 —
   **처음으로 formal한 정의를 작성하고, 현재 IR 제약 하에서 실제로 증명**).
   "resume 결과 = hydration했다면 나왔을 결과와 동일하다"를 `i32` 스칼라
   시그널(`+1` 카운터) 축소 모델로 정의(`ResumabilitySound`)하고, "시그널 하나당
   액션 하나"라는 현재 IR의 실제 제약(`SingleWriter`) 하에서 증명(`resumability_sound`).
   **다중 writer일 때 실제로 깨지는 구체적 반례**(같은 시그널을 두 개의 독립
   엘리먼트가 각자 재개하면 서로 다른, 둘 다 틀린 값에 도달)도 계산해서 남김 —
   이게 IR이 "시그널당 액션 최대 1개"를 지금 강제하는 실제 이유. Qwik이 이 정리를
   공식 증명한 선례는 재조사에서도 확인 안 됨 — RQ5가 이미 추정한 대로 이 프로젝트의
   실질적으로 새로운 기여일 가능성이 남아 있음(반증 못 함).

## 아직 스켈레톤조차 없는 것 / 남은 과제

- `SupervisionNode`에 실제 Entity/DOM 소유권 모델링 — Layer 2 스펙(docs/03)이 Entity
  그래프 소유권을 더 안정적으로 확정한 뒤.
- `ResumabilitySoundness.lean`을 StringList/Bool/Str(DOM-복구형 재개)로 확장 — 이번엔
  가장 단순한 I32 캡처형만 다룸.
- IR이 "시그널당 액션 최대 1개"를 실제로 강제하는 정적 검사(현재는 컴파일러가 아니라
  저작자 규율에 의존) — `ResumabilitySoundness.lean`의 반례가 이 갭을 정확히 지적함.

## 상세 실행 기록

`roadmap/PHASES.md`의 "Phase 5 — 공식 검증" 절 + `research-questions/
RQ5-formal-verification-targets.md` 참고.
