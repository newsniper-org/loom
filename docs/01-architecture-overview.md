# 01. 아키텍처 개요

> **2026-10-17 갱신 — 이 문서를 읽기 전에**: 아래 본문과 그림은 2026-07-22 설계 대화의 원본이고,
> 그 뒤 실측으로 바뀐 부분이 있다. 원본 문장은 지우지 않고 해당 자리에 날짜 붙은 정정을 달았다.
> 요약하면 다음과 같다.
> - **Island 서술은 낡았다.** 2026-07-28 RQ6가 **H-Qwik 확정(강한 형태)**으로 끝나, 기본 모델은
>   "페이지 전체가 하나의 World, 하나의 앱 청크(`App_chunk`), 전역 위임 리스너 하나"다. Island는
>   선택적 최적화로 지위가 낮아졌다(docs/04). 그림의 "Island root = supervision root"와
>   "Island별 partition", 본문의 "Island = 독립 IslandWorld"는 이제 기본 모델이 아니다.
>   이 판정은 RQ6 §7 (a)를 반사실 조항으로 읽는 해석(병익님 확정)에 기대고, 위젯 5개 규모에서
>   나왔다(`research-questions/RQ6-are-islands-necessary.md` "상태" 절의 한계 4개).
> - **Layer 0 — Host Glue**(JS 쪽 `registry.js`/`shared-store.js`, 그 네이티브 거울인 Rust `island.rs`의
>   `NativeIslandRegistry` — docs/04 원문이 셋을 실물로 든다)가 Phase 3에서 추가됐다. 아래에서 같은
>   6상태 기계를 "Layer 1 구현"으로 세는 것은 담당 관심사(supervision) 기준이다(docs/07 용어표). 기본
>   모델에서는 `shared-store.js`와 전역 위임 `bootstrap*.js`만 쓰이고, `registry.js`의 6상태
>   추적은 세분화를 택했을 때만 쓰인다(docs/04 "Layer 0" 절).
> - **Layer 1은 구현 범위가 좁다.** 실제 코드에 있는 Layer 1은 Island 경로의 6상태 기계
>   (`crates/loom-runtime/src/island.rs`, JS `registry.js`)와 그 Lean 4 모델 증명
>   (`formal-verification/lean4/SupervisionTree.lean`)이다. 컴포넌트별 범용 actor 런타임,
>   `Future`를 await하는 비동기 actor, suspense 경계는 crates/ 어디에도 구현돼 있지 않다
>   (2026-10-17 grep 확인 — `async`/`.await`는 `loom-bench`의 측정 보조 함수에만 있다).
> - **Layer 2 실행기는 Bevy `SingleThreaded`다**(`bevy_ecs` 0.19, `multi_threaded` feature 미사용 —
>   `formal-verification/tla/SchedulerSafety.stub.tla` 서두). 컴파일러가 System을 생성하므로 접근
>   선언도 컴파일러가 만들지만, 병렬 실행은 하지 않는다.
> - **PatchTable(opcode 디스패치)은 구현되지 않았다.** Phase 1이 제네릭 Rust System
>   (`patch_text_system::<T>` 등)으로 대신했고 아직 전환하지 않았다(docs/07, `crates/loom-runtime/src/lib.rs` 헤더).
> - RQ1(동적 리스트/조건부의 정적 스케줄링)은 아직 열려 있다. 트랙 L v22 판정(2026-09-30~10-01)은
>   났다 — C 채택 불가, D·E 기각 미발동(D는 W5 부속 발동). 그러나 채택 판정은 아니다: G0(D·E No-VDOM
>   구현 재심사)·G1 판정 기록·D의 W5 unkeyed opt-out 명세가 남아 있고 C는 D·E 자격에 종속된다.
>   Firefox WebDriver BiDi GC 통제 재수집은 2026-10-17 현재 대기 중(미시작)이다
>   (`research-questions/RQ1-dynamic-control-flow-scheduling.md` "v22" 절 "채택까지 남은 것").

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

**2026-10-17 정정 (위 그림)**: 그림은 2026-07-22 원본이다. RQ6 H-Qwik 확정(2026-07-28) 뒤 현행은 다음과 같다.
- Layer 1 상자의 "Island root = supervision root"는 **"지연 activation 서브트리의 root"**로 읽는다
  (docs/06 "지연 activation 서브트리와의 관계" 절, docs/07 용어표의 IslandRoot 폐지).
- Layer 2 상자의 "ECS World (Island별 partition)"는 **페이지 전체가 하나의 World**로 읽는다.
  Island별 partition은 `GraphMode::Islands`를 선택했을 때만 생긴다(docs/04).
- "suspense = async await 경계"는 설계 의도로만 남아 있고 구현되지 않았다(위 상단 갱신 블록).
- "Resumability 페이로드" 아래에 그림에 없는 JS 쪽 Layer 0 — Host Glue가 있다.
- 실제 SSR은 Rust 네이티브 렌더러(`loom-resume-demo`의 ssr bin, `loom-fic`의 `render_html_full`)이고,
  클라이언트는 wasm 청크 하나를 첫 상호작용 때 materialize한다(Phase 2, `roadmap/PHASES.md`).

## 각 layer가 왜 그 요구조건의 자연스러운 자리인지

- **Islands**는 구조적 개념(어디서 client JS가 필요한가) → **Layer 1**의 관할.
  Island = 독립된 supervision subtree root = 독립된 IslandWorld(Layer 2 partition) 하나.
  **(2026-10-17 정정: 이 등식은 2026-07-28 RQ6 판정으로 폐기됐다. 실측(위젯-임베드 정체성 통제
  픽스처, 위젯 5개)에서 Island 경계 없이도 임베딩 요구가 충족됐고, M1 번들 크기는 H-Qwik 팔이
  완전 관여 시 약 3.1배, 무상호작용 시 약 182배 가벼웠다. 현행: Island는 Layer 1의 필수 단위가
  아니라 "지연 activation 서브트리를 별도 wasm 산출물로 쪼갤지"의 선택적 최적화다. 위젯 여러 개의
  임베드는 "하나의 World, 복수 anchor root"로 처리한다 — docs/04, docs/05.)**
- **Resumability**는 효율성 메커니즘(client가 뭘 언제 실행하는가) → **Layer 2**가 만든
  Component-as-data 특성 덕분에 크게 단순해질 것으로 기대함 — 단, 이는 아직 실측되지 않은
  핵심 가설임(08의 "명시적으로 검증되지 않은 가설" 참고). 자세한 상충 관계는 04 참고.
  **(2026-10-17 보강: 메커니즘 자체는 Phase 2(2026-07-23~24)에 구현·검증됐다 — SSR 후 상호작용 전
  지연 청크 로드·실행 0, 첫 상호작용 때 lazy chunk, 카운터·리스트·조건부·폼 입력 재개. 축소 모델의
  건전성은 `formal-verification/lean4/ResumabilitySoundness.lean`에서 증명됐다(`sorry` 0. 정리 둘 —
  i32 카운터 스냅샷형 `resumability_sound`는 단일 writer 전제, DOM 복구형 재개(Bool/String/리스트 행)
  `dom_recovered_resumability_sound`는 전제 없음). 그러나 "Qwik보다 단순해진다"는 비교 주장은 여전히 실측되지 않았다 — RQ6 M3는
  우리 두 팔(Islands/Flatten)끼리 비교했지 실제 Qwik과 비교하지 않았다. 페이로드 포맷은 RQ2에서
  잠정 텍스트 포맷 `chunk|symbol|key:value`만 정해졌다.)**
- **No VDOM**은 03에서: JSX 구조가 컴파일타임에 알려져 있으므로 diff 알고리즘이 필요 없고,
  compiler가 생성한 narrow patch system이 `Changed<T>` 쿼리로 직접 DOM을 건드림.
  **(2026-10-17 보강: 정적 부분은 이렇게 구현됐다. 정적 서브트리는 컴파일타임에 HTML 문자열로 미리
  렌더한다(docs/02). 동적 시퀀스 사이트(`.map()`/조건부)에 어떤 좁은 전용 알고리즘이 허용되는지는
  docs/03 "No-VDOM 판별 기준"(P1–P3, N1–N5, 판정례 1–9)으로 판정한다. 리스트·조건부 전략의 최종
  채택은 RQ1에 열려 있다.)**
- **Backend-agnostic**은 05에서: SSR 렌더러를 순수 함수 `(component tree, request data) →
  (HTML, resumability payload)`로 만들고 이걸 WASM/sidecar/native binding 세 가지 경로로
  아무 언어에서나 호출 가능하게 함.
  **(2026-10-17 보강: Phase 4(2026-07-24)에서 `crates/loom-fic`의 C ABI shim 위에 세 경로를 실제로
  구현했다 — Python ctypes(네이티브), Node.js sidecar HTTP, Python wasmtime(WASM-in-process). 단, 현재
  구현은 컴포넌트 하나(`demo/counter-list.tsx`)에 고정돼 있고 `props`/`request_context`는 비어 있으며
  `assets`는 항상 빈 배열이다 — docs/05.)**

## 이 문서에서 의도적으로 안 다루는 것

- 구체적 API 표면(함수 시그니처, 데코레이터 문법 등) — 이건 Phase 1 프로토타입에서 실험적으로 결정
  **(2026-10-17 현황: Phase 1–3에서 컴파일러가 받는 TSX의 좁은 부분집합이 정해졌다 — signal 선언,
  `.map()` 리스트, `{cond() && ...}` 조건부(중첩 포함), `<input>` 양방향 바인딩, `client:load`/
  `client:visible` 컴포넌트 참조. 삼항 조건부, props 실제 resolve 등은 미지원이다(docs/02). 저작 API는
  확정되지 않았다. `examples/counter-island.sample.tsx`는 RQ6 확정 이전의 `island()` 래퍼를 쓰는
  목업이라 기본 모델이 아니다.)**
- 빌드 툴체인 세부사항(번들러 선택 등) — roadmap에서 다룸
  **(2026-10-17 현황: 컴파일러를 쓰는 데모·픽스처 크레이트(20개 중 13개)의 `build.rs`가 `loom-compiler`를
  라이브러리로 호출한다(나머지 7개 — runtime·compiler·flush-core·bench·island-demo·island-productcard·
  island-quantitypicker — 에는 `build.rs`가 없다). 브라우저 산출물은 wasm-pack으로, FIC의 WASM-in-process
  산출물은 plain `cargo build --target wasm32-unknown-unknown`으로 만든다. `loomc` bin은 빈 스텁이다. 저장소에 번들러 설정은 없다(2026-10-17 grep 확인).)**
