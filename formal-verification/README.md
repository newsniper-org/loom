# 공식 검증 스코프

RQ5 참고 후 읽을 것.

## 현재 상태 요약 (2026-10-17 갱신)

이 절은 2026-10-17에 새로 쓴 요약이다. 아래 날짜별 절은 당시 기록이라 수치와
"남은 것"이 그 시점 기준이다(낡은 곳에는 정정을 달았다). RQ5 후보는 **넷**이고
(#4는 2026-07-28에 추가 등재), 전부 결론이 났다.

| # | 대상 | 도구 | 상태(2026-10-17 직접 재실행으로 확인) |
|---|---|---|---|
| 1 | teardown 안전성 — `lean4/SupervisionTree.lean` | Lean 4 (4.29.1) | 증명 완료, `sorry` 0, `lean` 무출력 통과. **모델** 증명. despawn 시점은 미모델링(파일 끝 TODO) |
| 2 | 스케줄러 무충돌성 — `tla/SchedulerSafety.stub.tla` | TLA+ | **의도적 스텁**(증명 없음, Bevy에 위임). 2026-10-17에 bevy_ecs 0.19.0 소스로 재검토 조건을 이행했고 결론 유지 — 스텁 끝 절 |
| 3 | resumability 건전성 — `lean4/ResumabilitySoundness.lean` | Lean 4 | 증명 완료, `sorry` 0. i32는 `SingleWriter` 전제(컴파일러 `check_one_action_per_signal`이 강제), StringList/Bool/Str은 무전제. **축소 모델** 증명 |
| 4 | flush 규약 v0 순서 보장 — 모델 `verus/flush_order.rs` | Verus | **28 verified, 0 errors**. Verus로 쓴 **모델** |
| 4 | 같은 성질 + LIS 경로 — 배포 코드 `crates/loom-flush-core` | Verus | **90 verified, 0 errors**(2026-08-11 LIS 건전성 이후 코드 변경 없음). 쓰기 계획 계산 코드 자체를 검증하지만 적용 루프는 spec으로 모델링, D/E 계열 경로만(C·K 제외) |

남은 것(숨기지 않음):
- `loom-flush-core`의 원시 입력 구조적 사전조건 넷(`selection_preconditions_hold`)은
  `debug_assert!`로만 지켜진다(릴리스 빌드에서는 검사 없음).
- LIS **최적성**은 증명하지 않았다(의도적 — 정합성이 아니라 성능 성질, 측정으로만 뒷받침).
- `ownedEntities`가 detach 이후 언제 비워지는지(despawn 시점)는 미모델링.
- Lean 두 파일은 모델 증명이라 "모델 = 코드"는 증명 밖이다(코드 리뷰·테스트로 유지).
- `cargo verus verify`는 vstd 불일치로 실행되지 않는다. 아래 `verus …` 명령을 쓸 것.
- `roadmap/PHASES.md`의 Phase 5 절은 2026-08-08(47 verified) 시점에 쓰였고, 2026-10-17에
  08-09(수렴, 70 verified)·08-11(LIS 건전성, 90 verified) 경위와 미증명 목록이 보충됐다.
  증명 단계별 상세(실패 시도·rlimit 기준선 포함)는 `verus/README.md`에 있다.

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
   **2026-10-17 정정**: 위 근거는 0.14.2 기준이다. 2026-07-28 커밋 7e30659에서
   bevy_ecs가 0.19.0으로 올라갔는데 스텁 자신의 재검토 조건("메이저 업그레이드하면
   실행기 소스를 다시 읽을 것")이 이행되지 않은 채 버전 문자열만 바뀌어 있었다.
   2026-10-17에 0.19.0 소스로 이행했다. (a) `multi_threaded`는 여전히 default가
   아니고 `cargo tree`로도 켜져 있지 않으며, `default_executor()`가 SingleThreaded를
   고른다(wasm32에서는 feature와 무관하게 항상). (b) `can_run`의 무충돌 검사는
   남아 있지만 메커니즘이 바뀌었다 — 실행 직전의 `archetype_component_access`
   검사가 아니라, 실행기 init 때 `FilteredAccessSet`로 미리 계산한 충돌 비트셋을
   실행 중 System 집합과 대조한다. 결론(위임)은 유지. 새 주의점: 0.19에서는
   `set_executor(MultiThreadedExecutor::new())`로 feature 없이도 다중 스레드 실행기를
   쓸 수 있다(저장소에 그런 호출은 없음). 파일:라인은 스텁 끝 절.
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

## 2026-07-28 (Task #28 e·d·f) — 위 세 항목 전부 실제로 착수·완료

이전 라운드가 "남은 과제"로 남겨둔 세 항목을 이번에 전부 실제로 채웠다.

1. **`ResumabilitySoundness.lean`을 StringList/Bool/Str로 확장** — `codegen.rs`의
   `generate_resume` 실제 arm을 컴포넌트별로 대조한 결과, 이 세 kind는 전부
   `adopt_children`/`get_input_value`로 **지금 DOM에 보이는 값을 직접 읽는다**(I32처럼
   `captures` 배열의 캡처된 리터럴을 쓰지 않음) — 그래서 `SingleWriter` 전제 자체가
   필요 없다. `dom_recovered_resumability_sound` 정리로 이 사실을 formalize(전제 없는
   무조건 등식, `rfl`). zero `sorry`.
2. **`SupervisionNode`에 Entity/DOM 소유권 모델링** — `ownedEntities : List EntityId`를
   실제로 추가(`children`은 기존 방침대로 여전히 보류), `entityAccessible` 정의 +
   `detach_forbids_owned_entity_access`/`no_transition_restores_owned_entity_access`로
   기존 actor-라벨 정리("detach 이후 접근 불가")를 "그 노드가 실제로 소유한 Entity"
   수준으로 승격. Bevy의 read/write 스케줄링 자체는 재구현하지 않고 이미
   `tla/SchedulerSafety.stub.tla`가 위임해 둔 것 그대로 재사용(CLAUDE.md 2-1) — 이 파일이
   보장하는 건 "actor 상태가 허용하는 동안에만 그 Entity가 접근 가능하다고 취급한다"는
   더 상위 불변식. zero `sorry`. **여전히 남음**: `ownedEntities`가 detach 이후 언제
   실제로 비워지는지(despawn 시점)는 미모델링 — 파일 안 TODO로 명시.
3. **"시그널당 액션 최대 1개" 정적 검사** — `lower.rs`의 `check_one_action_per_signal`이
   실제로 존재하게 됨(이전부터 있었지만 안 쓰이던 `Component::action_signal_names()`를
   연결). **실측 중 발견한 중요한 정정**: 첫 시도는 모든 시그널 종류에 이 검사를
   적용했는데, `demo/counter-list.tsx`의 정당한 `items.append("new")`+`items.rotate()`
   (같은 `StringList` 시그널을 같이 건드림)를 즉시 깨뜨렸다(`cargo test --workspace`로
   실제 발견, 추측이 아님) — 항목 1이 증명한 대로 `StringList`/`Bool`/`Str`은
   `SingleWriter`가 필요 없으므로, 최종적으로는 **`i32` 시그널에만** 적용하도록
   수정했다. `ResumabilitySoundness.lean`의 반례 스케치가 지적했던 갭이 이제 진짜로
   막혔다 — `SingleWriter`는 더 이상 "저작자의 규율"이 아니라 컴파일러가 강제하는
   불변식.

## 2026-07-28 (Task #29) — RQ6 §7 게이트 확정에 따른 스코프 재검토

RQ6 §7 게이트가 H-Qwik을 확정(강한 형태)함에 따라, `SupervisionTree.lean`의
`ActorState` 6종·forest 모델의 범위를 재검토했다(§11이 "경미" 항목으로 요구한
그대로) — **결론: 증명 자체는 무효화되지 않는다, 프레이밍만 재해석된다.**

`ActorState`(dormant/attaching/attached/detaching/detached/failed)와 그 위의
정리들(`beginDetach_forbids_access` 등)이 모델링하는 건 "Island의 lifecycle"이
아니라 **"지연 activation된 서브트리 하나의 lifecycle"**이다 — RQ3의 부분
폐기 절이 이미 확인한 대로, 이 상태기계 자체는 세분화 여부와 무관하게 그대로
필요하다(기본 모델에서는 페이지 전체가 그 "서브트리 하나", 세분화를 선택하면
Island마다 하나씩). 그래서 `IslandState`/`IslandRegistryBackend`라는 이름은
docs/07의 재해석대로 "세분화를 선택했을 때"의 특수 사례로 좁혀 읽어야 하지만,
Lean 파일 자체의 타입/정리는 코드 변경 없이 유효하다 — 이름이 여전히
"Island"인 이유는 실제 Rust 코드(`crates/loom-runtime/src/island.rs`)의
식별자와 맞춰야 하기 때문이지, 그 코드가 세분화 전용이라서가 아니다.

`SupervisionNode.ownedEntities`(Task #28-d)도 마찬가지 — "이 Island이 소유한
Entity"가 아니라 "이 지연 activation 서브트리가 소유한 Entity"로 일반화해
읽으면 그대로 유효하다.

## 아직 스켈레톤조차 없는 것 / 남은 과제

- `SupervisionNode.ownedEntities`가 detach 이후 언제 실제로 비워지는지(despawn 시점,
  메모리 회수) — RQ6 M4/M6과 연결되는 지점이나 이 파일은 아직 다루지 않음.

(2026-10-17 보충: 이 목록은 Lean 쪽만 적혀 있었다. Verus 트랙의 남은 것 — 구조적
사전조건 넷의 `debug_assert!` 의존, LIS 최적성 미증명 — 은 맨 위 "현재 상태 요약"과
`verus/README.md` 맨 끝 "남는 것"에 있다. 2026-10-17 현재 위 항목은 여전히 남아 있다.)

## 상세 실행 기록

`roadmap/PHASES.md`의 "Phase 5 — 공식 검증" 절 + `research-questions/
RQ5-formal-verification-targets.md` 참고. (2026-10-17 주: PHASES.md Phase 5 절에는
2026-10-17에 08-09·08-11 경위가 보충됐다. 증명 단계별 상세는 `verus/README.md`를 볼 것.)


## 2026-08-08 — 세 번째 트랙: Verus (`verus/`)

이 파일이 오랫동안 Lean4와 TLA+만 다뤘는데, RQ5 후보 #4(flush 규약 v0 순서
보장)가 `formal-verification/verus/`에서 진행돼 **세 번째 트랙**이 생겼다.
왜 여기만 Verus인지(대상이 모델이 아니라 **실제 실행 함수**라서)는
`verus/README.md`의 첫 절에 있다.

실행:

```
verus formal-verification/verus/flush_order.rs              # 모델 — 28 verified, 0 errors
verus --crate-type=lib crates/loom-flush-core/src/lib.rs    # 실행 코드 — 90 verified, 0 errors
```

**2026-10-17 정정**: 위 블록의 실행 코드 수치는 `70 verified`였다. 이 절이
쓰인 2026-08-08의 실제 값은 47이었고, 70은 2026-08-09(LIS 경로 수렴)의 값이 나중에
여기 반영된 것이다. 그 뒤 2026-08-11 LIS 건전성 증명으로 90이 됐다. 2026-10-17에
두 명령을 다시 돌려 28 / 90 verified, 0 errors를 확인했다(기준선은
`verus/README.md`의 "기준선" 절).

`cargo verus verify`는 업스트림 Verus와 crates.io `vstd`의 불일치로 **실행되지
않는다**(`unexpected verus diagnostic item …opens_invariants_none`).

남은 것과 실패 경위(rlimit/메모리 벽, 두 번의 잘못된 가설, 함수 단위 프로파일의
필요성)는 전부 `verus/README.md`에 있다.
