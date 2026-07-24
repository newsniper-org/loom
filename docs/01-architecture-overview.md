# 01. 아키텍처 개요

## 출발점: 2-layer 분리

이전 대화(RIB → ECS 융합 논의)에서 나온 관찰: Rx 스케줄러는 사실 두 가지 job을 뭉뚱그리고 있었음.

1. **불규칙한 lifecycle/비동기 스케줄링** — 토폴로지가 런타임에 바뀜 (mount/unmount, 비동기 완료)
2. **규칙적인 벌크 상태 전파** — 토폴로지가 안정된 구간에서 "이 값이 바뀌면 저 값도 갱신"

ECS 스케줄러는 (2)를 잘 풀지만 (1)엔 원래 약함 (flat entity set은 동적 계층 재구성을 표현하기 어려움).
그래서 단일 스케줄러 교체 대신 **역할을 분리**함:

- **Layer 1 (구조/lifecycle)**: Actor/supervision tree. 부모가 자식의 생사를 관장, 실패/해제가
  하위로 전파. attach = spawn_child, detach = terminate_child.
- **Layer 2 (상태/벌크 계산)**: ECS World + 정적 스케줄러. 각 actor는 자기 상태를 private field가
  아니라 World 안의 자기 Entity에 붙은 Component로 소유. 부수효과 결과는 Command 큐를 통해
  정해진 sync point에서 Component에 반영.

## 이 골격 위에 4개 요구조건을 얹는 지점

```
                         ┌─────────────────────────────────┐
                         │   JSX/TSX 저작 (compile-time)     │  ← 02
                         └───────────────┬───────────────────┘
                                         │ 컴파일
                         ┌───────────────▼───────────────────┐
                         │  Layer 1: Actor/Supervision Tree   │  ← 06
                         │  - Island root = supervision root  │  ← 04
                         │  - attach/detach = mount/unmount   │
                         │  - suspense = async await 경계     │
                         └───────────────┬───────────────────┘
                                         │ 소유
                         ┌───────────────▼───────────────────┐
                         │  Layer 2: ECS World (Island별 partition) │
                         │  - Signal<T> Component              │  ← 03
                         │  - Changed<T> 게이팅 patch system    │  (No VDOM)
                         │  - 정적 스케줄 (컴파일타임 dependency) │
                         └───────────────┬───────────────────┘
                                         │ 직렬화
                         ┌───────────────▼───────────────────┐
                         │  Resumability 페이로드 (SSR 출력)    │  ← 04
                         │  - Component = POD → 직렬화 자연스러움│
                         │  - 이벤트는 LRef(지연 참조)로          │
                         └───────────────┬───────────────────┘
                                         │ HTML 조각 + payload
                         ┌───────────────▼───────────────────┐
                         │  Framework Integration Contract     │  ← 05
                         │  (어떤 백엔드든 이 계약만 만족하면 됨) │
                         └─────────────────────────────────────┘
```

## 각 layer가 왜 그 요구조건의 자연스러운 자리인지

- **Islands**는 구조적 개념(어디서 client JS가 필요한가) → **Layer 1**의 관할.
  Island = 독립된 supervision subtree root = 독립된 IslandWorld(Layer 2 partition) 하나.
- **Resumability**는 효율성 메커니즘(client가 뭘 언제 실행하는가) → **Layer 2**가 만든
  Component-as-data 특성 덕분에 크게 단순해질 것으로 기대함 — 단, 이는 아직 실측되지 않은
  핵심 가설임(08의 "명시적으로 검증되지 않은 가설" 참고). 자세한 상충 관계는 04 참고.
- **No VDOM**은 03에서: JSX 구조가 컴파일타임에 알려져 있으므로 diff 알고리즘이 필요 없고,
  compiler가 생성한 narrow patch system이 `Changed<T>` 쿼리로 직접 DOM을 건드림.
- **Backend-agnostic**은 05에서: SSR 렌더러를 순수 함수 `(component tree, request data) →
  (HTML, resumability payload)`로 만들고 이걸 WASM/sidecar/native binding 세 가지 경로로
  아무 언어에서나 호출 가능하게 함.

## 이 문서에서 의도적으로 안 다루는 것

- 구체적 API 표면(함수 시그니처, 데코레이터 문법 등) — 이건 Phase 1 프로토타입에서 실험적으로 결정
- 빌드 툴체인 세부사항(번들러 선택 등) — roadmap에서 다룸
