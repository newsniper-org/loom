# loom (provisional codename) — 리서치 핸드오프 패키지

> **상태**: 2026-07-24부로 Phase 1(컴파일러 + No-VDOM 반응성 프로토타입, RQ1 트랙
> L/T 후보 전부 구현+실측 + LIS 최소 이동 변형 + paired bootstrap 통계)과 Phase 2
> (Resumability)가 상당히 진전됨 — `crates/`에 실제 동작하는 코드가 있음(SWC 기반
> lowering + Bevy ECS 기반 런타임 + 브라우저에서 실측 검증된 카운터/동적 리스트
> 데모 + **컴파일러가 직접 생성하는** SSR·resume·lazy chunk·전역 delegation, 리스트
> 재개(DOM에서 row 값 복구) + 조건부 재개(런타임 레벨) + LRef 최소 캡처 계산 +
> stale-chunk 폴백 포함). PHASES.md "여기서 RQ6 재검토"의 §3.1(Phase 2 종료 —
> H-Qwik 팔 단독 기준선) 실행 완료 — 번들 크기/콜드-웜 지연시간 실측, Island
> 비교는 Phase 3 이후로 명시적으로 남김. Island/backend-agnostic은 아직 범위 밖 —
> 정확한 스코프는 `roadmap/PHASES.md`의 Phase 1/2 상태 참고. docs/*의 설계 문서
> 자체는 여전히 리서치 산출물이며 검증된 진실이 아님(CLAUDE.md "이 패키지의 한계"
> 참고).
> **이름**: "loom"은 가칭. `docs/07-naming-and-terminology.md` 참고 후 전역 치환 권장.

## 이 패키지는 무엇인가

2026-07-22 대화에서 도출된 UI 프레임워크 아키텍처의 리서치 핸드오프 패키지.
출발점은 RIB/MVU/ECS 비교 분석 → RIB의 Rx 스케줄러를 대체하며 ECS와 융합하는 "2-layer 분리" 설계였고,
여기에 다음 4개 요구조건이 추가로 얹힘:

1. **JSX/TSX** 문법으로 저작
2. **Islands architecture**와 **Resumability**를 **동시에** 지원
3. 어떤 백엔드 프레임워크에도 결합 가능한 유연성 (backend-agnostic)
4. **No Virtual DOM**

이 네 요구조건은 서로 자유롭게 조합되지 않는다. 특히 2번은 내부적으로 충돌 가능성이 있음이
리서치 중 발견됨 (Qwik 공식 문서가 명시적으로 "resumability가 있으면 islands의 trade-off는 불필요"라고
주장) — 이 긴장을 어떻게 푸는지가 `docs/04-islands-and-resumability.md`의 핵심 내용.

## 왜 이 형태로 패키지를 구성했는가

기존 프레임워크(Qwik, Solid, Astro, Bevy)의 프리미티브를 그대로 이식하는 게 아니라, 이전 대화에서
합의된 "2-layer(Actor 계층 = lifecycle/구조, ECS World = 상태/벌크 계산)" 설계를 골격으로 두고
JSX 컴파일 전략·Islands·Resumability·backend adapter를 그 골격 위에 **어떻게 얹을지** 새로 설계함.
즉 이건 기존 아트의 짜깁기가 아니라 원 아키텍처들의 job 분해에서 유도한 합성 설계이고,
`docs/08-prior-art-comparison.md`에서 무엇을 그대로 재사용했고 무엇이 실제로 새로운지 명시적으로 구분해둠.

## 디렉토리 맵

```
loom-research/
├── README.md                     ← 지금 읽는 파일
├── CLAUDE.md                     ← Claude Code 작업 지침 (반드시 먼저 읽을 것)
├── docs/                         ← 아키텍처 설계 문서 (확정된 결정 + 근거)
│   ├── 01-architecture-overview.md
│   ├── 02-compiler-design.md
│   ├── 03-reactivity-no-vdom.md
│   ├── 04-islands-and-resumability.md   ← 핵심 긴장 관계와 해소안
│   ├── 05-backend-agnostic-integration.md
│   ├── 06-lifecycle-actor-layer.md
│   ├── 07-naming-and-terminology.md
│   └── 08-prior-art-comparison.md
├── research-questions/            ← 아직 안 풀린 것들, RQ1~RQ6
│   └── data/                       실측 원시 데이터 (예: RQ1 트랙 L core-grid 결과 JSON)
├── formal-verification/           ← Lean4/TLA+ 스코프 정의 + 스켈레톤 (증명 아님)
├── roadmap/PHASES.md              ← 리서치→프로토타입→검증 단계 계획
├── examples/counter-island.sample.tsx  ← 최종 API 표면 목업 (동작 안 함 — Phase 1 실제
│                                          구현은 이 문법의 훨씬 좁은 부분집합만 지원)
├── demo/                          ← counter-list.tsx(Phase 1 컴파일 대상) + index.html +
│                                     list_bench.html(RQ1 트랙 L/T 벤치마크 페이지) +
│                                     resume/(Phase 2 SSR+lazy chunk 데모)
└── crates/                        ← Phase 1/2 실제 구현 (Rust workspace)
    ├── loom-runtime/                Layer 2 반응성 코어: Signal<T>/DomTarget Component,
    │                                 Changed<T> 게이팅 patch System, RQ1 트랙 L 후보
    │                                 K/C/D/E/D-LIS/E-LIS(src/list.rs)·트랙 T 후보 B/F
    │                                 (src/cond.rs, 서브트리 크기 축 지원) 전부 구현,
    │                                 web feature로 브라우저 DomOps 공유(src/web.rs,
    │                                 adopt_children/set_attribute 등 resume 지원 포함)
    ├── loom-compiler/                SWC 기반 TSX → IR → Rust codegen ("loomc") —
    │                                 setup/apply_action(Phase 1 라이브)뿐 아니라
    │                                 resume/resume_action_by_symbol(Phase 2 재개)도
    │                                 생성, LRef 최소 캡처 계산 포함(src/codegen.rs)
    ├── loom-demo/                    wasm-bindgen 진입점 — build.rs가 loomc를 호출해
    │                                 demo/counter-list.tsx를 컴파일, web_sys DOM에 마운트
    ├── loom-bench/                   RQ1 트랙 L 6후보(K/C/D/E/D-LIS/E-LIS) + 트랙 T
    │                                 (B/F, 서브트리 크기 축) 실측 하네스
    │                                 (demo/list_bench.html, Chromium+Firefox)
    └── loom-resume-demo/             Phase 2 — SSR(src/bin/ssr.rs, 네이티브) + lazy wasm
                                       청크(컴파일러 생성 카운터+리스트 + 손코딩 조건부
                                       토글, RQ6 §4.1 전체-앱 픽스처) + 전역 delegation
                                       리스너(demo/resume/bootstrap.js, stale-chunk
                                       리로드 폴백 포함), 실제 네트워크 요청 감시로
                                       "상호작용 전 client JS 0 실행" 검증
```

## 읽는 순서 (Claude Code 기준)

1. `CLAUDE.md` — 작업 프로토콜
2. `docs/01-architecture-overview.md` — 전체 그림
3. `docs/04-islands-and-resumability.md` — 가장 중요한 설계 결정
4. 나머지 `docs/*`
5. `research-questions/*` — 여기서부터가 실제 리서치 작업 대상
6. `roadmap/PHASES.md` — Phase 0부터 시작

## 확정된 것 vs 열린 것

**확정 (이 대화에서 합의):**
- Layer 1 = actor/supervision tree (lifecycle, attach/detach, island root, suspense 경계)
- Layer 2 = ECS World (상태 저장, `Changed<T>` 게이팅 patch system, 정적 스케줄링)
- JSX는 컴파일타임에 static/dynamic binding site로 분해되어 직접 DOM 호출로 컴파일 (VDOM 없음)
- Island = 하나의 supervision root + 하나의 독립 ECS World partition
- Resumability는 QRL과 유사한 패턴(전역 위임 리스너 + lazy 코드 로딩)을 쓰되, 캡처 대상이
  Component(= plain data)라서 Qwik의 임의 클로저 직렬화보다 구조적으로 더 단순해질 여지가 있음

**미정 (research-questions/ 참고):**
- 동적 리스트/조건부 렌더링의 정적 스케줄링 방법 (RQ1)
- Resumability 페이로드의 스키마 버저닝 (RQ2)
- Island 중첩 시 supervision 전파 규칙 (RQ3)
- Island 간 통신 (RQ4)
- 공식 검증 대상 우선순위 (RQ5)
- **Islands가 정말 필요한가 — resumability만으로 충분하다는 Qwik의 주장과의 관계 (RQ6, 가장 중요)**
