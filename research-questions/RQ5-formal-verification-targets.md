# RQ5. 공식 검증 대상 후보 및 우선순위

formal-verification/README.md와 함께 볼 것. 여기서는 "무엇을 검증할 가치가 있는가"와
"얼마나 어려운가"를 분리해서 정리.

## 후보 정리

| # | 정리(theorem) 후보 | 대상 | 난이도(추정) | 가치 |
|---|---|---|---|---|
| 1 | Supervision teardown 안전성: detach가 항상 전체 subtree를 정리하고, detach 이후
     어떤 System도 죽은 Entity를 참조하지 않는다 | Lean4, 상태 기계 모델 | 중 | 높음 — 메모리
     안전성/dangling reference 버그를 구조적으로 배제 |
| 2 | 스케줄러 무충돌성: 컴파일러가 생성한 두 System의 read/write 접근 집합이 disjoint하다고
     선언되면, 실제로 어떤 인터리빙에서도 데이터 레이스가 없다 | TLA+, 모델체킹(작은 인스턴스) | 중~하 |
     중간 — Rust의 borrow checker가 실질적으로 이미 이걸 런타임에 보장해줄 가능성이 높아서,
     정리 자체의 한계효용이 낮을 수 있음. RQ로 먼저 "이거 굳이 TLA+로 다시 증명할 가치가
     있는가, 아니면 타입 시스템으로 충분한가"부터 따져야 함 |
| 3 | **Resumability 건전성**: resume된 상태에서 핸들러를 실행한 결과가, 만약 hydration을
     했다면(전체 재실행) 나왔을 결과와 동일하다 | Lean4/수기 증명, 정의 자체가 아직 안 됨 | 높음 |
     **가장 가치 있음** — 이게 이 프레임워크의 핵심 주장(resumability가 정확하다)을 뒷받침하는
     정리인데, 아직 "동일하다"의 formal한 정의조차 없음. Qwik 자체도 이걸 공식적으로 증명한
     적 없는 것으로 보임(리서치 필요) — 진짜 새로운 기여가 될 수 있는 지점 |

## 우선순위 제안

1번(teardown 안전성) → 3번(resumability 건전성) → 2번(스케줄러 무충돌성, 타입시스템으로
대체 가능한지 먼저 검토)

## 상태

**2026-07-24 (Phase 5) — 세 후보 전부 실행함**:

- **1번(teardown 안전성)**: `formal-verification/lean4/SupervisionTree.lean`에 실제
  증명 완료(`sorry` 없음, `lean` 직접 확인). `crates/loom-runtime/src/island.rs`의
  `NativeIslandRegistry` 6-상태 기계를 그대로 모델링해 RQ3의 동기 안전성 +
  진행성 두 정리 증명. **스코프 한계(숨기지 않음)**: Entity/DOM 소유권까지
  포함한 전체 주장이 아니라 "actor 라벨이 절대 되살아나지 않는다"는 필요조건만
  증명 — Entity 소유권 모델링은 남은 과제.
- **2번(스케줄러 무충돌성)**: **TLA+ 증명 안 함, 실측 기반 결론**. `bevy_ecs`
  0.14.2 소스를 실제로 읽어 확인: 이 프로젝트가 `multi_threaded` feature를 어디서도
  안 켜서 동시 실행 자체가 없고(SingleThreaded 실행기뿐), 설령 켜더라도 Bevy의
  `MultiThreadedExecutor::can_run`이 실행 직전에 access-set 호환성을 무조건 검사해
  겹치는 System은 절대 동시 실행 안 됨(`ambiguity_detection` 로그 설정과 무관). 이건
  이 프로젝트 코드가 아니라 Bevy 자체가 이미 구현·검증한 메커니즘이라 재검증은
  스코프 밖(CLAUDE.md 2-1) — 원래 우선순위 제안이 예견한 "타입시스템(정확히는
  Bevy 런타임)으로 대체 가능"이 실제로 확인된 사례. 근거는
  `formal-verification/tla/SchedulerSafety.stub.tla`에 파일:라인 단위로 기록.
- **3번(resumability 건전성)**: `formal-verification/lean4/ResumabilitySoundness.lean`에
  **처음으로 formal한 정의를 작성**하고, i32 스칼라(단순 카운터) 축소 모델 +
  "시그널당 액션 최대 1개"라는 현재 IR의 실제 제약(`SingleWriter`) 하에서 증명
  완료. 그 제약이 없으면 실제로 깨지는 구체적 반례(같은 시그널을 두 독립 엘리먼트가
  각자 재개하면 서로 다른, 둘 다 틀린 값에 도달)도 계산해 남김 — IR이 지금
  "시그널당 액션 1개"를 강제하는 실제 이유가 이 반례임. Qwik이 이걸 공식 증명한
  선례는 재조사에서도 확인 안 됨 — 새로운 기여일 가능성 유지.

**남은 과제**: `SupervisionNode`의 Entity/DOM 소유권 모델링, `ResumabilitySoundness`를
StringList/Bool/Str(DOM-복구형 재개)로 확장, IR의 "시그널당 액션 1개" 제약을 실제
정적 검사로 강제(현재는 저작자 규율 의존). 상세는 `formal-verification/README.md` +
`roadmap/PHASES.md` Phase 5 절.

---

## 2026-07-28 — 후보 표에서 누락돼 있던 네 번째 대상: flush 규약 v0

**발견 경위**: 시스템에 설치된 업스트림 Verus를 FV에 활용하자는 제안(병익님)을
검토하며 "그럼 지금 무엇이 검증 대기 중인가"를 다시 세어 보다가 드러났다.

위 후보 표(#1 teardown, #2 스케줄러, #3 resumability)는 **RQ1의 판정을 실제로
막고 있는 FV 항목을 담고 있지 않다**. 그 항목은 `RQ1-benchmark-protocol.md`
§5의 "추가 주기(v1.1 — D-1)"가 별도로 도입한 것이다:

> flush 규약 v0의 순서 보장에 정합성이 의존하는 **이동 발행 후보 D·E 공통**으로,
> 채택은 본 트랙 통과에 더해 **flush 규약의 formal-verification 통과를 선결
> 조건**으로 가진다. … FV는 Phase 5 소관이므로 Phase 1 시점의 D·E 판정은 최대
> **"조건부 채택(FV 계류)"**까지. **FV 항목 자체는 특정 후보가 아니라 공통
> 인프라(flush 규약 v0)의 검증 항목이다.**

현재 상태(확인함):

- `formal-verification/` 전체에 flush 관련 산출물 **0건**(`grep -rl flush` 무결과).
- `roadmap/PHASES.md`에서 flush FV 언급은 **단 1회**, 그것도 "D의 계류 사유"로서만.
- 그런데 Phase 5는 위 세 후보를 근거로 **완료 선언**됨.

즉 **Phase 5가 "완료"인 동시에, Phase 1의 D/E 판정을 막는 FV는 손도 안 댄 상태**
라는 모순이 문서 사이에 끼어 있었다. 후보 표에 없으니 우선순위 논의에도, Phase 5
실행에도 한 번도 올라오지 않았던 것이다. 이 표에 **후보 #4로 정식 등재**한다.

| # | 정리 후보 | 대상 | 난이도(추정) | 가치 |
|---|---|---|---|---|
| 4 | **flush 규약 v0의 순서 보장**: 구조 변경 Commands와 DOM op를 단일 순서 큐로 병합 flush할 때, (a) 적용 순서가 방출 순서와 일치하고, (b) 이동 기준 anchor가 같은 배치에서 아직 spawn되지 않은 row인 인터리빙에서도 최종 DOM 순서가 목표 순서로 **결정적으로** 수렴한다 | 순수 Rust(`Vec<DomWrite>` 적용 루프) — **Verus 적합** | 중 | **가장 시급** — D·E 채택의 유일한 남은 선결 조건이고, 트랙 N 불변식 5와 동일한 성질 |

**왜 Verus가 이 항목에 특히 맞는가**(#1/#3이 Lean4인 것과 대비):

- #1/#3은 *모델*을 증명했다 — Lean4 안에 6-상태 기계와 i32 축소 모델을 **손으로
  다시 쓰고** 그걸 증명했으므로, "모델이 실제 코드와 같은가"는 증명 밖에 남는다
  (두 파일 모두 이 한계를 스스로 명시하고 있다).
- #4는 대상이 `LoomApp::tick`의 **적용 루프와 `Vec<DomWrite>`라는 구체 자료구조**
  다. Bevy도 DOM도 개입하지 않는 순수 Rust 구간이라, Verus로 **실제 함수에
  `requires`/`ensures`를 걸어** 검증할 수 있다 — 모델-구현 간극이 처음으로
  사라지는 항목.

**같은 이유로 Verus가 맞지 않는 것들**(범위를 넘기지 않기 위해 명시):

- `bevy_ecs`가 개입하는 전부(System 스케줄링, archetype move, change detection) —
  Verus가 추론할 수 없는 코드다. #2가 이미 "Bevy가 이미 구현·검증한 메커니즘,
  재검증은 스코프 밖"(CLAUDE.md 2-1)으로 종결된 것과 같은 이유.
- `web-sys`/DOM 실호출, SWC 기반 lowering — 동일.
- #3의 남은 과제(StringList/Bool/Str 확장)는 *의미론적 동치*에 관한 것이라
  Lean4가 계속 맞다. Verus로 옮길 이유 없음.

**전제 조건(정직하게)**: 오늘의 `list.rs`/`lib.rs`는 Bevy 쿼리·`Commands`와
얽혀 있어 Verus를 그대로 겨눌 수 없다. 실무 경로는 **순수 알고리즘 코어를
분리 추출**(이동 집합 계산, LIS, flush 적용 순서 결정)한 뒤 그 함수에 spec을
붙이고, Bevy System은 그 함수를 호출만 하게 만드는 것이다 — 검증 가능성과
별개로 테스트 용이성도 올라가지만, **리팩터링이라는 비용이 선행**한다는 점을
숨기지 않는다.
