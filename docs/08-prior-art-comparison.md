# 08. 기존 아트 비교 — 무엇을 재사용했고 무엇이 새로운가

정직성 문서. 이 프로젝트를 "완전히 새로운 발명"으로 과장하지 않기 위해, 각 요소가 기존
어디서 왔고 어디가 실제로 새로 설계된 것인지 명시.

## 축별 비교

| 축 | React | Solid | Qwik | Astro | Bevy(ECS) | RIB | **loom(가칭)** |
|---|---|---|---|---|---|---|---|
| Diff 방식 | VDOM | 컴파일타임 fine-grained | 컴파일타임 fine-grained | (컴포넌트별 프레임워크에 위임) | 해당없음(게임 루프) | 해당없음(UI 레이어 아님) | 컴파일타임 fine-grained, ECS 저장 |
| 반응성 원자 | state+VDOM | Signal(클로저) | Signal | (프레임워크별) | Component | Rx Observable | **Component**(POD) |
| Hydration | 전체 replay | 전체 replay(단, 컴파일러 최적화 있음) | **없음**(resumable) | 부분(Island별) | 해당없음 | 해당없음 | **없음** + Island 격리 (04) |
| Lifecycle 모델 | 콜백 훅 | 콜백 훅 | 콜백 훅 | 프레임워크별 | 없음(World만) | Supervision tree | **Supervision tree** (06) |
| 스케줄링 | 없음(React 자체 reconciler) | 없음(즉시 실행) | 없음 | 없음 | **선언된 접근집합 기반 자동 병렬** | Rx 스케줄러(런타임 동적) | Bevy 방식 + **컴파일타임 자동 생성** |
| 백엔드 결합 | Node 중심 | Node 중심 | Node/Deno/Bun 중심 | 여러 JS 런타임 어댑터 | 해당없음 | 모바일 앱 내부 구조 | **언어 무관**(FIC, 05) |
| 공식 검증 | 없음 | 없음 | 없음 | 없음 | 없음 | 없음(업계 관행) | Lean4/TLA+ 스텁 (formal-verification/) |

## 그대로 재사용한 것 (재발명 안 함)

- Qwik의 QRL 메커니즘 원리(전역 delegation 리스너 + lazy chunk 로딩) — LRef로 이름만 바꿔 재사용
- Bevy의 read/write 선언 기반 System 병렬 스케줄링, Command 지연 적용 패턴
- Astro의 client 지시자 개념(언제 활성화할지 우선순위 지정) — 다만 우리는 hydrate가 아니라
  "지연 actor spawn 트리거 시점"으로 재해석 (06)
- RIB의 supervision/attach-detach 트리 구조, Interactor의 비동기 처리 패턴
- JSX 파싱 자체(SWC) — 파서를 새로 만들 이유 없음

## 실제로 새로 설계한 것

1. **Component-as-reactivity-atom**: Signal을 클로저가 아니라 ECS Component로 둔 것.
   Solid/Qwik 둘 다 안 하는 선택. 이게 resumability 직렬화를 크게 단순화시킨다는 게 이
   프로젝트의 핵심 가설 (아직 실측 안 됨).
2. **PatchTable 패턴** (02): Component를 POD로 유지하면서도 "패치 행동"을 표현하는 방법.
   정수 opcode + 컴파일러 생성 정적 디스패치 테이블. 알려진 선례를 찾지 못함.
3. **Island = 격리 단위, Resumability = 효율성 메커니즘으로 관할을 분리한 재해석** (04):
   Qwik/Astro 어느 쪽도 이 둘을 이렇게 명시적으로 직교시키지 않음. Qwik은 오히려 이 둘을
   경쟁 관계로 봄. 이 재해석이 맞는지는 RQ6로 열어둠.
4. **WaveScalar류 dataflow와의 구조적 유비**(안정 구간=정적 스케줄, 재구성=리스케줄) —
   설명적 유비이지 구현에 직접 쓰이는 알고리즘은 아님. 과장하지 말 것.

## 명시적으로 검증되지 않은 가설

- "Component가 POD라서 resumability 페이로드가 Qwik보다 단순해진다" — 논리적으로는
  그럴듯하나 실제 페이로드 크기/복잡도를 비교 측정한 적 없음
- "컴파일러가 자동 생성한 스케줄이 대부분 disjoint해서 병렬성이 높다" — 일반적인 UI
  패턴에서는 그럴 것으로 예상되나, 반례(같은 부모 Component를 여러 binding site가
  공유하는 경우 등)가 있을 수 있음
