# 08. 기존 아트 비교 — 무엇을 재사용했고 무엇이 새로운가

정직성 문서. 이 프로젝트를 "완전히 새로운 발명"으로 과장하지 않기 위해, 각 요소가 기존
어디서 왔고 어디가 실제로 새로 설계된 것인지 명시.

> **2026-10-17 갱신**: 이 문서의 표와 "새로 설계한 것" 목록 중 세 군데가 현행과 달라 날짜 붙은
> 정정을 달았다. 원문은 지우지 않았다.
> (1) Hydration 행과 "새로 설계한 것" #3의 Island 격리 재해석은 2026-07-28 RQ6 **H-Qwik 확정(강한
> 형태)**으로 폐기됐다. (2) "새로 설계한 것" #2 PatchTable은 **구현되지 않았다**. (3) 공식 검증 칸의
> Verus 수치는 70이 아니라 **90 verified**(2026-08-11 LIS 건전성 증명 이후)다.
> 그 밖에 스케줄링 행과 "검증되지 않은 가설" 절에 현황을 보탰다(같은 날 검토 지적으로 RQ1 v22 판정
> 상태·트랙 N 재인증·"그대로 재사용한 것"의 병렬 스케줄링·Verus 수치 경로를 다시 고쳤다).

## 축별 비교

| 축 | React | Solid | Qwik | Astro | Bevy(ECS) | RIB | **Loom** |
|---|---|---|---|---|---|---|---|
| Diff 방식 | VDOM | 컴파일타임 fine-grained | 컴파일타임 fine-grained | (컴포넌트별 프레임워크에 위임) | 해당없음(게임 루프) | 해당없음(UI 레이어 아님) | 컴파일타임 fine-grained, ECS 저장 |
| 반응성 원자 | state+VDOM | Signal(클로저) | Signal | (프레임워크별) | Component | Rx Observable | **Component**(POD) |
| Hydration | 전체 replay | 전체 replay(단, 컴파일러 최적화 있음) | **없음**(resumable) | 부분(Island별) | 해당없음 | 해당없음 | **없음** + Island 격리 (04) |
| Lifecycle 모델 | 콜백 훅 | 콜백 훅 | 콜백 훅 | 프레임워크별 | 없음(World만) | Supervision tree | **Supervision tree** (06) |
| 스케줄링 | 없음(React 자체 reconciler) | 없음(즉시 실행) | 없음 | 없음 | **선언된 접근집합 기반 자동 병렬** | Rx 스케줄러(런타임 동적) | Bevy 방식 + **컴파일타임 자동 생성** |
| 백엔드 결합 | Node 중심 | Node 중심 | Node/Deno/Bun 중심 | 여러 JS 런타임 어댑터 | 해당없음 | 모바일 앱 내부 구조 | **언어 무관**(FIC, 05) |
| 공식 검증 | 없음 | 없음 | 없음 | 없음 | 없음 | 없음(업계 관행) | Lean4 증명 2건(`sorry` 0) + TLA+ 스텁 1건 + **Verus로 배포 코드 검증**(`crates/loom-flush-core` 70 verified, 모델 `flush_order.rs` 28 verified — 2026-08-09 실측) *(초판 2026-07-24 서술: "Lean4/TLA+ 스텁")* |

**2026-10-17 정정 (위 표)**:
- **Hydration 행**: "**없음** + Island 격리 (04)"의 "Island 격리" 부분은 폐기됐다. 2026-07-28 RQ6 §7
  게이트(위젯-임베드 정체성 통제 픽스처, 위젯 5개)에서 H-Qwik이 확정되어 기본 모델은 "페이지 전체가
  하나의 World, 하나의 `App_chunk`, 전역 위임 리스너 하나"다. 이 행의 Loom 칸은 이제 Qwik 칸과 같은
  **없음(resumable)**이고, Island 세분화(`GraphMode::Islands`)는 선택적 최적화로만 남는다(docs/04).
  판정은 §7 (a)를 반사실 조항으로 읽는 해석(병익님 확정)에 기대고, 다른 해석이면 조건부/비결정이었다.
- **Lifecycle 모델 행**: supervision tree는 설계 골격이다. 코드로 있는 것은 Island 경로의 6상태 기계
  (`crates/loom-runtime/src/island.rs`, JS `registry.js`)와 그 Lean 4 모델뿐이다. 기본 모델(Flatten)의
  `bootstrap-flat.js`에는 이 상태기계가 결선돼 있지 않다(docs/06 2026-10-17 보강).
- **스케줄링 행**: "Bevy 방식 + 컴파일타임 자동 생성" 중 "자동 생성"은 맞다 — 컴파일러가 Bevy System을
  생성하고, 접근 집합은 그 System 시그니처에서 나온다. 그러나 Bevy 칸의 "자동 병렬"은 Loom에서 쓰이지
  않는다. 모든 크레이트가 `bevy_ecs` 0.19를 `multi_threaded` feature 없이 쓰므로 실행기는
  `SingleThreaded`다(`formal-verification/tla/SchedulerSafety.stub.tla` 서두. 그 근거 인용은 bevy_ecs
  0.14.2 소스 기준이고, 0.19에서의 재확인은 이행되지 않았다).
- **공식 검증 행**: `crates/loom-flush-core`는 **90 verified, 0 errors**다. 2026-08-09 수렴 증명 시점 70
  (같은 날 채택 후 보강으로 71)에서, 2026-08-11 하루에 두 단계를 거쳐 늘었다 — 구조적 사전조건 Q를
  `plan_inputs`/`plan_from_selection`의 사후조건으로 옮긴 단계 뒤 80(LIS 건전성 절의 "변경 전 HEAD" 수치),
  이어 LIS 건전성 증명으로 90(`formal-verification/verus/README.md` "경계를 옮겼다 — Q가 사후조건이 됐다"·
  "LIS 건전성 — 증명 완료" 절, RQ5 2026-08-11 절).
  모델 `flush_order.rs`는 28 verified 그대로다. Lean 4 두 파일(`SupervisionTree.lean`,
  `ResumabilitySoundness.lean`)은 `sorry` 0, TLA+ `SchedulerSafety.stub.tla`는 의도적 스텁(Bevy에 위임)이다.
  Verus가 덮는 배포 코드는 D/E 계열 리스트 경로의 flush 순서 Chain과 LIS 쓰기 계획뿐이다(C·K 제외).
  원시 입력의 구조적 사전조건 4개는 `debug_assert!`로만 지켜지고, LIS 최적성은 증명하지 않았다.
  Lean 두 건은 **모델** 증명이라 모델과 코드의 일치는 가정이다.

## 그대로 재사용한 것 (재발명 안 함)

- Qwik의 QRL 메커니즘 원리(전역 delegation 리스너 + lazy chunk 로딩) — LRef로 이름만 바꿔 재사용
- Bevy의 read/write 선언 기반 System 병렬 스케줄링, Command 지연 적용 패턴
  **(2026-10-17 정정: 실제로 재사용한 것은 접근 선언 기반 스케줄 구성과 Command 지연 적용이다. 실행기가
  `SingleThreaded`라 병렬 실행은 재사용되지 않았다 — 위 표의 스케줄링 행 정정.)**
- Astro의 client 지시자 개념(언제 활성화할지 우선순위 지정) — 다만 우리는 hydrate가 아니라
  "지연 actor spawn 트리거 시점"으로 재해석 (06)
  **(2026-10-17 보강: 이 재해석은 Island 경로의 `registry.js`에만 구현돼 있다(load 즉시,
  visible은 `IntersectionObserver`). 기본 모델(Flatten)에서는 지시어가 트리거로 결선되지 않고 첫 클릭의
  전역 위임만 있다 — docs/04 2026-10-17 현황 확인 3.)**
- RIB의 supervision/attach-detach 트리 구조, Interactor의 비동기 처리 패턴
- JSX 파싱 자체(SWC) — 파서를 새로 만들 이유 없음

## 실제로 새로 설계한 것

1. **Component-as-reactivity-atom**: Signal을 클로저가 아니라 ECS Component로 둔 것.
   Solid/Qwik 둘 다 안 하는 선택. 이게 resumability 직렬화를 크게 단순화시킨다는 게 이
   프로젝트의 핵심 가설 (아직 실측 안 됨).
2. **PatchTable 패턴** (02): Component를 POD로 유지하면서도 "패치 행동"을 표현하는 방법.
   정수 opcode + 컴파일러 생성 정적 디스패치 테이블. 알려진 선례를 찾지 못함.
   **(2026-10-17 정정: 이 패턴은 구현되지 않았다.** Phase 1이 opcode 디스패치 대신 제네릭 Rust System
   (`patch_text_system::<T>` 등, 타입별 monomorphize)으로 갔고, "Phase 2에서 opcode로 전환"하겠다는
   기록(roadmap/PHASES.md Phase 1 절, `crates/loom-runtime/src/lib.rs` 헤더)은 이행되지 않았다. Phase 2
   resumability는 opcode 없이 LRef(`chunk|symbol|captures`)와 생성된 정적 `dispatch(symbol, value)`로
   구현됐다. CLAUDE.md 금지 4(Component에 클로저/함수 포인터 저장 금지)는 제네릭 System 방식으로도
   지켜진다. 따라서 이 항목은 "새로 설계해 실현한 것"이 아니라 **설계 스케치**다. opcode 형태가 실제로
   필요한지 자체가 열려 있다 — docs/07 용어표.)**
3. **Island = 격리 단위, Resumability = 효율성 메커니즘으로 관할을 분리한 재해석** (04):
   Qwik/Astro 어느 쪽도 이 둘을 이렇게 명시적으로 직교시키지 않음. Qwik은 오히려 이 둘을
   경쟁 관계로 봄. 이 재해석이 맞는지는 RQ6로 열어둠.
   **(2026-10-17 정정: 이 항목은 폐기됐다.** RQ6를 2026-07-28에 실측으로 판정한 결과 H-Qwik(강한
   형태)이 확정됐다. Island 경계 없이도 임베딩 요구가 충족됐고(M8 양쪽 통과, M6 조건 A·M7·M2/M3/M9
   등가), M1 번들 크기는 H-Qwik 팔이 완전 관여 시 약 3.1배, 무상호작용 시 약 182배 가벼웠다. 즉 이
   항목에서 Qwik 쪽 주장("resumability는 islands의 trade-off 없이 확장된다")이 우리 실측으로 확인됐다.
   따라서 이 항목은 "새로 설계한 것"이 아니라 **검증에서 살아남지 못한 가설**로 남긴다(docs/04 "폐기된
   이전 설계", `research-questions/RQ6-are-islands-necessary.md` "상태" 절과 한계 4개).)**
4. **WaveScalar류 dataflow와의 구조적 유비**(안정 구간=정적 스케줄, 재구성=리스케줄) —
   설명적 유비이지 구현에 직접 쓰이는 알고리즘은 아님. 과장하지 말 것.

## 명시적으로 검증되지 않은 가설

- "Component가 POD라서 resumability 페이로드가 Qwik보다 단순해진다" — 논리적으로는
  그럴듯하나 실제 페이로드 크기/복잡도를 비교 측정한 적 없음
- "컴파일러가 자동 생성한 스케줄이 대부분 disjoint해서 병렬성이 높다" — 일반적인 UI
  패턴에서는 그럴 것으로 예상되나, 반례(같은 부모 Component를 여러 binding site가
  공유하는 경우 등)가 있을 수 있음

**2026-10-17 현황**:
- 첫째 가설은 **여전히 실측되지 않았다.** RQ6 M3(wake-up payload)는 우리 Islands 팔과 Flatten 팔을
  비교했을 뿐(두 팔의 캡처 세그먼트가 동일) 실제 Qwik과 비교하지 않았다. 다만 resumability 자체는 Phase 2
  에서 구현·검증됐고, 축소 모델의 건전성은 `formal-verification/lean4/ResumabilitySoundness.lean`에서
  증명됐다. 정리는 둘이다 — i32 카운터 스냅샷형 `resumability_sound`(컴파일러가 정적으로 강제하는
  단일 writer 전제 아래)와 DOM 복구형 재개(Bool/String/리스트 행) `dom_recovered_resumability_sound`
  (단일 writer 전제 없음 — resume이 DOM을 직접 되읽으므로). 페이로드 포맷은 RQ2에서
  잠정 텍스트 포맷 `chunk|symbol|key:value`만 정해졌고, 리스트 상태는 페이로드에 싣지 않고 DOM에서
  되읽는다(RQ2).
- 둘째 가설은 **현재 배포 형태에서는 검증할 대상이 없다.** 실행기가 `SingleThreaded`라 병렬 실행
  자체가 없다(위 표 정정). 병렬성을 재거나 `multi_threaded`를 켠 적이 없으므로 이 가설은 측정되지 않은
  채 남는다.
- 위 두 가설과 별개로, 아래는 이 문서 초판 이후에 생긴 "검증 상태" 사실이다.
  - **RQ1(동적 리스트/조건부의 정적 스케줄링)은 열려 있다.** 트랙 L은 개정 #9(2026-08-12)로 그 전
    판정이 모두 무효가 됐고, 개정 #10 이후 전면 재수집한 v22가 2026-09-30~10-01에 판정됐다(Chromium
    비스로틀 f6c1bdb, 4× 스로틀 d8325ba, Firefox 694d274): C 채택 불가(기각 미발동), D(=D-LIS) 기각 (iii)
    불성립·W5 부속 발동, E(=E-LIS) 기각 미발동. **그래도 채택 판정은 아니다** — G0(D·E No-VDOM 구현
    재심사)와 G1 판정 기록이 없고, D의 W5 unkeyed opt-out 명세가 없으며, C는 D·E 자격에 종속된다(§9 C-4).
    Firefox는 GC 통제 없는 수집분이라 WebDriver BiDi GC 통제 재수집이 예정돼 있지만 2026-10-17 현재
    시작하지 않았다(RQ1 "v22" 절 "채택까지 남은 것"). 트랙 T는 B 유지, F-i/ii/iii 기각(2026-08-07).
    트랙 N G2는 2026-08-08에 처음 통과했고, 그 뒤 리스트 구현이 재작성돼 2026-09-27에 재인증받았다
    (R1·R2 둘 다 6 arm 통과, `research-questions/RQ1-benchmark-protocol.md` "§5 재인증 결과"). 현행 v22
    런타임은 이 재인증 기반(98f2d3d)과 같다. G2만 다룬 인증이고 G0·G1은 범위 밖이다.
  - Verus 증명(위 표)은 RQ1 후보 D/E 계열이 쓰는 쓰기 계획 코드의 **정합성**을 보장할 뿐, 그 후보가
    **채택될지**는 RQ1 실측이 정한다.
