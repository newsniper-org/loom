# Loom — 리서치 패키지

> **상태 (2026-10-17 기준)**: `roadmap/PHASES.md`의 Phase 0–4·6은 완료됐다. Phase 5(공식 검증)는 RQ5 후보
> #1–#4가 이행됐다. 단, 스케줄러는 Bevy에 위임하고 TLA+는 의도적 스텁으로 두었고, LIS 최적성과 원시 입력의
> 구조적 사전조건 4건은 증명하지 않았다.
> RQ6는 **H-Qwik 확정(강한 형태)**(2026-07-28)이다. Island 경계는 기본 모델에서 필수가 아니고 선택적
> 최적화로 지위가 낮아졌다. 이 판정은 §7 (a)를 반사실 조항으로 읽는 해석(병익님 확정)에 기대고, 다른
> 해석이면 조건부/비결정이었다. 그 결과로 RQ3는 부분 폐기, RQ4는 전면 폐기됐다.
> **RQ1(동적 리스트/조건부의 정적 스케줄링)은 아직 열려 있다.**
> - 트랙 L은 v22 재수집 캠페인이 진행 중이다. Chromium 비스로틀·4× 스로틀은 끝났고, Firefox는 GC 통제
>   재수집이 보류 중이다(G0 결정 뒤 재개 여부를 정함). 채택 판정은 아직 없다.
> - 트랙 T는 B 유지, F 계열 기각으로 판정됐다.
> - 트랙 N은 G2 게이트를 통과했다.
>
> RQ2(재개 페이로드 포맷)는 잠정 포맷만 있고 열려 있다. docs/*와 research-questions/*는 여전히 리서치
> 산출물이며 검증된 진실이 아니다(CLAUDE.md "이 패키지의 한계" 참고).
>
> 이 배너는 2026-10-17에 전면 갱신했다. 그 전 배너는 2026-07-24(Phase 1/2) 시점에 머물러 있었다.

## 이 패키지는 무엇인가

2026-07-22 대화에서 도출된 UI 프레임워크 아키텍처의 리서치 패키지다. 이름은 **Loom**이다(2026-07-28 확정).
산문에서는 Loom이라 쓰고, 코드 식별자는 소문자 `loom-*`를 그대로 둔다(`docs/07-naming-and-terminology.md`).

출발점은 RIB/MVU/ECS 비교 분석이다. 거기서 RIB의 Rx 스케줄러를 대체하며 ECS와 융합하는 **"2-layer 분리"**
설계가 나왔다.

- **Layer 1 — Actor/supervision 계층**: lifecycle, attach/detach, 구조 (`docs/06-lifecycle-actor-layer.md`)
- **Layer 2 — ECS World**: 상태 저장과 벌크 계산, `Changed<T>` 게이팅 patch system, Bevy 스케줄러 재사용
  (`docs/03-reactivity-no-vdom.md`)
- Phase 3에서 JS 쪽 **Layer 0 — Host Glue**(`registry.js`/`shared-store.js`)가 추가됐다. RQ6 확정 뒤에도
  축소된 형태로 남아 있다(`docs/04`).

여기에 다음 4개 요구조건이 얹혔다.

1. **JSX/TSX** 문법으로 저작한다(SWC 기반 컴파일러).
2. **Resumability**와 **Islands**. 원래 요구는 둘을 "동시에 지원"하는 것이었다. Qwik은 "resumability가 있으면
   islands의 trade-off는 불필요"라고 주장하는데, 이 긴장을 RQ6로 실측했다. 그 결과 **resumability가 기본
   모델이고 Island는 선택적 최적화**로 귀결됐다(`docs/04-islands-and-resumability.md`).
3. 어떤 백엔드 프레임워크에도 결합할 수 있다(backend-agnostic). 구조는 Rust 코어 → C ABI shim → 언어별
   바인딩이다.
4. **No Virtual DOM**. 최적화 대상이 아니라 요구조건이다. 경계 사례는 `docs/03`의 "No-VDOM 판별 기준"
   조문으로만 판정한다.

Resumability를 위해 **Component는 항상 직렬화 가능한 plain data(POD)로 남는다**. 클로저나 함수 포인터를
Component에 저장하지 않는다(CLAUDE.md "절대 하지 말 것" 4).

## 왜 이 형태로 패키지를 구성했는가

기존 프레임워크(Qwik, Solid, Astro, Bevy)의 프리미티브를 그대로 이식하지 않았다. 대신 이전 대화에서 합의된
2-layer 설계를 골격으로 두고, JSX 컴파일 전략·Resumability·Island·backend adapter를 그 골격 위에 **어떻게
얹을지** 새로 설계했다. 즉 기존 아트를 짜깁기한 것이 아니라, 원 아키텍처들의 job 분해에서 유도한 합성
설계다. `docs/08-prior-art-comparison.md`는 그대로 재사용한 것(QRL 원리, Bevy의 read/write 선언 기반
스케줄링, Astro client 지시어, SWC)과 실제로 새로운 것을 구분해 둔다.

단, docs/08의 "새로 설계한 것" 중 두 항목은 현행과 다르다. #3(Island 격리 재해석)은 RQ6로 폐기됐고,
#2(PatchTable)는 구현되지 않았다. docs/08에 2026-10-17 정정이 붙어 있다.

## 현재 상태

### Phase (`roadmap/PHASES.md`)

| Phase | 내용 | 상태 |
|---|---|---|
| 0 | 리서치 수렴: RQ1 판별 프로토콜 사전 등록, RQ3 잠정 규칙, RQ6 검증 계획 | 완료 2026-07-23 |
| 1 | 컴파일러 + No-VDOM 반응성 프로토타입 | 완료 2026-07-23. 스코프 축소를 명시했다(아래 "열린 것") |
| 2 | Resumability: SSR 후 상호작용 전 지연 청크 로드·실행 0(즉시 실행은 `bootstrap.js` 전역 리스너뿐), 첫 상호작용 시 lazy chunk | 완료 2026-07-23~24. 카운터·리스트·조건부·폼 입력 |
| 3 | Islands. 2026-07-28에 "지연 activation 경계 세분화(필요시)"로 재해석됐다 | 실행 2026-07-24~25, 잔여 항목 2026-07-28(Task #28) |
| 4 | 백엔드 어댑터: 두 언어 이상에서 같은 컴포넌트를 임베드 | 완료 2026-07-24. Python ctypes, Node sidecar, Python wasmtime. 잔여 FIC payload 2026-07-28 |
| 5 | 공식 검증 | RQ5 후보 #1–#4 이행. 아래 "공식 검증" |
| 6 | RQ6 최종 검증 | **H-Qwik 확정(강한 형태)** 2026-07-28 |

PHASES.md는 2026-08-13 이후 갱신되지 않았다. 그래서 두 가지가 반영돼 있지 않다.
- RQ1의 이후 진행(개정 #10, v22). 이 내용은 `research-questions/RQ1-benchmark-protocol.md`에만 있다.
- Phase 5 절은 2026-08-08 시점(47 verified, LIS 수렴 미증명)에 멈춰 있다. 08-09 수렴 증명과 08-11 LIS
  건전성 증명(90 verified)은 RQ5 문서와 `formal-verification/verus/README.md`에만 있다.

### Research questions (`research-questions/`)

| RQ | 질문 | 상태 |
|---|---|---|
| RQ1 | 동적 리스트/조건부 렌더링의 정적 스케줄링 | **열림**. 아래 |
| RQ2 | Resumability 페이로드의 직렬화 포맷·버저닝 | **열림**. 잠정 텍스트 포맷만 채택했고, 하위 질문 일부만 부분 해소 |
| RQ3 | Island 중첩 시 supervision 규칙 | **부분 폐기**(2026-07-28). 2단계 detach, promote-once-and-stay 등은 유지(지위는 여전히 "잠정 규칙"). 나머지는 세분화를 택할 때만 적용 |
| RQ4 | Island 간 통신 | **전면 폐기**(2026-07-28). 기본 모델은 하나의 World라서 같은 World의 Signal로 해결된다. crash 통지 채널만 유지 |
| RQ5 | 공식 검증 대상 우선순위 | 후보 #1–#4 이행. 아래 "공식 검증" |
| RQ6 | Islands가 정말 필요한가 | **확정: H-Qwik(강한 형태)**. 아래 |

**RQ6 판정 근거**: 위젯-임베드 정체성 통제 픽스처(위젯 5개)로 비교했다.
- M1 번들 크기: H-Qwik이 완전 관여 시 약 3.1배, 무상호작용 시 약 182배 가볍다.
- M2·M3·M9, M6 조건 A, M7 간섭-1은 등가다.
- M4(상주 메모리)는 Island 팔의 구조적 약점이다(N에 비례해 증가). H-Qwik 쪽에는 대응하는 축이 없다.
- 판정은 §7 (a)를 반사실 조항으로 읽는 해석(병익님 확정)에 기댄다.
- 한계는 넷이다. 규모가 커지면 우위가 줄어들 수 있고, 자기 상태를 바꾸는 도중의 패닉은 미검증이며, 조건 C가
  stable 채널에 오면 전제가 바뀌고, 위젯 임베드가 얼마나 흔한지는 답하지 않는다.
  상세는 `RQ6-are-islands-necessary.md`의 "상태" 절.

**RQ1 세부**. 사전 등록 프로토콜은 `RQ1-benchmark-protocol.md` v1.1에 개정 #1–#10이 붙은 것이다. 판정기는
`research-questions/analysis/rq1-verdicts.py`, 판정문은 `research-questions/data/verdicts/`에 있다.

- **트랙 L(리스트, 후보 K/C/D/E와 LIS 변형)**: 개정 #9(2026-08-12)로 그 전 판정이 전부 무효화됐다. 개정 #10
  하네스로 전면 재수집한 **v22**가 현행이다. v21 판정은 셀 간 오염(렌더러 재사용, K에 불리한 방향) 때문에
  재검토 대상으로 등재돼 있어서 독립 확증이 아니다.
  - Chromium 비스로틀 58/58그룹(셀마다 새 브라우저) 판정:
    - C: 채택 불가, 기각 미발동.
    - D: 기각 (iii) 불성립, 양성 조건 충족, W5 부속 발동.
    - E: 기각 미발동.
  - Chromium 4× 스로틀(강건성 확인): C 기각 앞 분기가 스로틀에서만 발동했다(W2×R2 C/D 하한 ≈3.16 ≥ 3).
    판정은 §8에 따라 비스로틀 열 그대로다.
  - Firefox(교차 확인) 16그룹을 GC 통제 없이 수집한 분: 게이트 결론은 Chromium과 같다. 셀 단위 우세 방향이
    정반대인 것은 4쌍이다(서로 다른 비교는 3건, 셀은 W2×R1·W3×R1 두 개). 비교쌍 25건 중 15건은 배치 모드가
    브라우저 간에 달라 비교할 수 없었다.
  - **WebDriver BiDi로 GC 통제를 붙인 Firefox 재수집은 보류 중이다**(2026-10-17 병익님 결정 — G0 결정 뒤 재개 여부를 정함, 프로토콜 말미). 원래는 개정 #10 후속 3에서 대기 중이었다. 재개하면 무부하 그룹 11 단독 스모크부터 시작한다.
  - §5 FV 계류(flush 규약 v0의 형식 검증)는 2026-08-11에 소진됐다(`loom-flush-core` 90 verified). 원시 입력
    구조적 사전조건 넷과 LIS 최적성은 그 계약 밖이고 미증명이다.
  - **채택까지 남은 것**:
    - G0(No-VDOM 구현 재심사, docs/03 판별 기준)는 2026-10-17에 처음 실행했다. HEAD 구현의 D·D-LIS·E·E-LIS가 모두
      조문상 **기각**이다(P1: `String` key 비교, P3: `match_keys` Θ(n·m)). "VDOM이다"가 아니라 형식 요건 미충족이고,
      확정할지 수리 후 재심사할지는 병익님 확인 대기다(RQ1 문서 끝 G0 기록).
    - D 기각 (iii) 뒷항(M4/M7/M3)이 미계측이다.
    - D를 채택하면 W5 부속 조건(unkeyed opt-out 명세)이 붙는다.
    - D per-row 변형과 트랙 R S1은 "v22 이후 캠페인"으로 미뤄져 있다.
    - C 판정은 C-4 기준선 규칙 때문에 D·E의 자격에 종속된다.
  - 남은 등재와 반영:
    - 스로틀 C 앞 분기와 Firefox 방향 뒤집힘의 §11-1(b) 등재
    - v21↔v22 셀 단위 대조
    - v22 결과의 RQ1 본문 반영(현재 RQ1 본문은 v21 절에서 끝남)
- **트랙 T(조건부, B/F)**: v2 캠페인(2026-08-07) 판정은 B 유지, F-i/ii/iii 기각이다. 재수집 때 할 항목은
  다음과 같다.
  - GC 통제(M-15) 적용
  - B arm의 중복 `set_text` 수리
  - J-a on/off 재정의(v3 사전 등록)
  - 2026-08-13 이후 기반 반영
- **트랙 N(중첩 정합성)**: 사이클 1(2026-08-08)에서 6 arm이 전부 통과해 G2 게이트를 통과했다. v21 기반
  재인증도 2회 통과했다. 불변식 4(F catch-up)는 약한 형태만 검사한다. 진짜 형태를 재려면 컴파일러가 F 계열을
  낼 수 있어야 한다.
- **트랙 R(Resumability)**: 프로토콜 §6에 정의만 있고 실행하지 않았다.

### 공식 검증 (`formal-verification/`)

| 대상 | 도구 | 상태 |
|---|---|---|
| Supervision 6상태 기계 + Entity 소유권 (`lean4/SupervisionTree.lean`) | Lean 4 | 증명 완료, `sorry` 0. 단, `ownedEntities`가 언제 실제로 비워지는지(despawn 시점)는 모델링하지 않았다(파일 안 TODO) |
| Resumability 건전성, 축소 모델 (`lean4/ResumabilitySoundness.lean`) | Lean 4 | 증명 완료, `sorry` 0. i32 카운터는 단일 writer 전제가 필요하고, 컴파일러가 이를 정적으로 강제한다. StringList/Bool/Str은 DOM 복구형이라 전제 없이 성립한다 |
| 스케줄러 안전성 (`tla/SchedulerSafety.stub.tla`) | TLA+ | **의도적 스텁**. Bevy SingleThreaded 실행기에 위임했다. 2026-10-17에 bevy_ecs 0.19.0 소스로 재검토를 이행해 위임 근거가 성립함을 확인했다(wasm32에서는 항상 SingleThreaded). 0.19부터는 조건에 "`set_executor`로 실행기를 바꾸지 않는다"가 더해졌다 |
| flush 순서 모델 (`verus/flush_order.rs`) | Verus | 28 verified, 0 errors. Verus로 쓴 **모델**이다 |
| flush 순서 + LIS **배포되는 코드** (`crates/loom-flush-core`) | Verus | 90 verified, 0 errors(LIS 건전성 증명 2026-08-11). 덮는 범위는 D/E 계열 경로뿐이고 C·K는 제외다. 원시 입력의 구조적 사전조건 4개는 `debug_assert!`로만 지켜진다(미검증). LIS 최적성은 증명하지 않았다 |

두 Lean 파일은 **모델** 증명이다. 모델이 코드와 일치한다는 가정 위에 선다. 반면 `loom-flush-core`의 Verus
검증은 배포되는 쓰기 계획 계산 코드 자체를 덮는다. 다만 그 안에서도 적용 루프는 spec으로 모델링돼 있고,
입력 사전조건 넷은 검증 밖에 있다.

`*-exhaustive.py` 5개는 명제와 구현 동일성을 기계로 확인하는 전수 탐색 검사 스크립트이고, 증명은 아니다.
`cargo verus verify`는 vstd 불일치로 동작하지 않는다. 실제 명령은 아래 "재현·검증"에 있다.

## 재현·검증

`formal-verification/verus/README.md` "기준선" 절에 적힌 **2026-08-11 기준선**이다. 테스트 수는 그 뒤로
늘었을 수 있다.

```
verus --crate-type=lib crates/loom-flush-core/src/lib.rs   → 90 verified, 0 errors
verus formal-verification/verus/flush_order.rs             → 28 verified, 0 errors
cargo test --workspace                                     → 195 passed (2026-08-11)
cargo run --release -p loom-track-n -- --cases 40          → 6 arm 전부 통과
python3 research-questions/analysis/fixtures/selfcheck.py  → 판정기 합성 픽스처 18모드 통과 (2026-10-17)
```

**측정 운영 규칙**(트랙 L 수집):
- 셀(그룹)마다 새 브라우저를 띄운다(`bench/collect-per-cell.sh`, 개정 #10 후속 2). 한 브라우저로 여러 셀을 재면
  렌더러 프로세스 상태가 셀을 건너 오염시킨다.
- CPU 경합이 있으면 셀이 상한(30분)에 걸려 표본이 잘린다. `LOAD_MAX` 가드를 쓴다.
- 수집 시작·재시작은 병익님 결정 사항이다. 현재 Firefox BiDi 재수집은 **보류**다(2026-10-17, G0 결정 뒤 재개 여부를 정함).
  재개하면 첫 단계는 무부하 그룹 11 단독 스모크다. 그 결과를 무부하 기준(18분·200표본)과 대조한 뒤 본수집으로 넘어가거나
  재등재한다.

## 디렉토리 맵

```
loom-research/
├── README.md                     ← 지금 읽는 파일
├── CLAUDE.md                     ← Claude Code 작업 지침 (반드시 먼저 읽을 것)
├── Cargo.toml                    ← workspace 정의 (크레이트 21개, edition 2024, bevy_ecs 0.19)
├── docs/                         ← 아키텍처 설계 문서 (결정 + 근거, 날짜 붙은 정정은 append-only)
│   ├── 01-architecture-overview.md
│   ├── 02-compiler-design.md
│   ├── 03-reactivity-no-vdom.md          ← No-VDOM 판별 기준 조문
│   ├── 04-islands-and-resumability.md    ← RQ6 확정 뒤 전면 재작성(2026-07-28)
│   ├── 05-backend-agnostic-integration.md
│   ├── 06-lifecycle-actor-layer.md
│   ├── 07-naming-and-terminology.md
│   ├── 08-prior-art-comparison.md
│   └── 09-authoring-surface-inventory.md ← 컴파일러 저작 표면 격차·조용한 오컴파일 47건(2026-10-17 감사)
├── research-questions/            ← RQ1–RQ6, RQ1 사전 등록 프로토콜, RQ6 검증 계획
│   ├── analysis/                   판정 스크립트(rq1-verdicts.py 등) + fixtures/(합성 픽스처·selfcheck)
│   └── data/                       실측 원시 데이터(append-only), verdicts/(판정문), raw/(GC 로그·환경).
│                                   트랙 L v1~v19-r3·v20-chromium-headed와 v15~v18 판정문은 개정 #9로
│                                   무효(보존만 함), v21은 재검토 대상 — 트랙 L 판정 근거는 v22만
├── formal-verification/           ← Lean 4 증명 2건, TLA+ 스텁 1건, Verus(모델 flush_order.rs +
│                                     전수 탐색 검사 스크립트)
├── roadmap/PHASES.md              ← Phase 0–6 계획과 실행 기록
├── examples/counter-island.sample.tsx  ← 초기 API 표면 목업 (컴파일 안 됨. `island()` 래퍼는
│                                          RQ6 확정 이전 전제라 기본 모델이 아님)
├── demo/                          ← 컴파일 대상 TSX(counter-list, nested-*, widget-embed,
│                                     islands/product-card) + 페이지(index.html, list_bench.html,
│                                     track_t_freq_bench.html, resume/, islands/, widget-embed/,
│                                     rq6-zero-framework-baseline.html)
├── bench/                         ← 실측 수집 파이프라인
│   ├── serve-isolated.py            COOP/COEP 정적 서버(타이머 해상도 확보, 기본 포트 8931)
│   ├── collect-stock.sh             stock 브라우저 수집 드라이버(한 수집 = 한 브라우저)
│   ├── collect-per-cell.sh          트랙 L 기본 경로: 그룹마다 새 브라우저, 부하 가드
│   ├── collect-firefox-subset.sh    §8 Firefox 교차 확인 16그룹
│   ├── merge-parts.py               그룹 조각 병합(누락·중복 검사)
│   ├── collected/                   (gitignore됨) 로컬 수집 작업 디렉토리. 판정용 정본은
│   │                                 research-questions/data/에 커밋
│   └── e2e/                         Playwright(RQ6 M-시리즈 specs) + cdp-agent.mjs(Chromium GC 통제·
│                                     스로틀) + ff-bidi-agent.mjs(Firefox GC 통제, WebDriver BiDi) + 프로브
└── crates/                        ← Rust workspace
    │  ─ 코어
    ├── loom-runtime/                Layer 2 반응성 코어: Signal/DomTarget, Changed<T> 게이팅 patch
    │                                 System, RQ1 트랙 L 후보(list.rs)·트랙 T 후보(cond.rs), 단계별
    │                                 계측(stage.rs)·M2 DOM 연산 계수(counting.rs), supervision 상태
    │                                 기계(island.rs), `browser` feature로 실제 DOM
    ├── loom-compiler/                SWC 기반 TSX → IR → Rust codegen. Phase 1 라이브 코드, Phase 2
    │                                 resume 코드·LRef 최소 캡처, 트랙 L 후보 선택, 다중 컴포넌트
    │                                 ComponentGraph + GraphMode::{Islands, Flatten}. 각 크레이트의
    │                                 build.rs가 라이브러리로 호출(`loomc` bin은 빈 스텁)
    ├── loom-flush-core/              Verus 검증된 flush 순서 Chain + LIS(D/E 계열 경로). 검증된 코드가
    │                                 곧 배포 코드(입력 사전조건 4개는 debug_assert로만)
    │  ─ Phase 1 / RQ1
    ├── loom-demo/                    Phase 1 종료 조건 데모(wasm-bindgen, demo/counter-list.tsx)
    ├── loom-bench/                   RQ1 트랙 L/T 실측 하네스(demo/list_bench.html)
    ├── loom-track-n/                 RQ1 트랙 N 중첩 정합성 속성 테스트(네이티브)
    ├── loom-nested-fixture/          컴파일러의 중첩 조건부/리스트 출력을 네이티브 TreeDom에서 대조
    │  ─ Phase 2
    ├── loom-resume-demo/             SSR(네이티브) + lazy wasm 청크 + 전역 delegation(demo/resume/).
    │                                 "상호작용 전 지연 청크 0"을 네트워크 감시로 검증
    │  ─ Phase 3 / RQ3 / RQ6
    ├── loom-island-demo/             RQ3 규칙의 손코딩 실증(중첩 Island + JS registry)
    ├── loom-island-productcard/      RQ6 §5.4 panic 봉쇄 조건 B 스파이크(별도 .wasm 인스턴스)
    ├── loom-island-quantitypicker/   〃
    ├── loom-island-compiled/         같은 소스를 Islands/Flatten 두 팔로 컴파일(정체성 통제)
    ├── loom-widget-embed/            RQ6 판정 픽스처: 위젯 5개, 두 GraphMode
    ├── loom-widget-embed-island-*/   (×4) 위젯별 실제 per-Island 코드 분할(RQ6 M1)
    ├── loom-largeapp-axis-flat/      RQ6 "원본 앱 규모" 축, H-Qwik 팔
    ├── loom-largeapp-axis-island-target/  같은 축, Island 팔
    ├── loom-rq6-fixture-gen/         RQ6 N-스케일링 픽스처 Rust 생성기 — Node 판과 바이트 동일, 설정으로 선택
    │                                 (bench/e2e/fixtures/gen-nscaling.sh), 마커 어긋남 가드 테스트
    │  ─ Phase 4
    └── loom-fic/                     Framework Integration Contract(render_html) + C ABI shim,
                                      sidecar·isr-bench bin, examples/(Python ctypes·wasmtime, Node)
```

## 읽는 순서 (Claude Code 기준)

1. `CLAUDE.md`: 작업 프로토콜
2. `docs/01-architecture-overview.md`: 전체 그림. 원래 그림은 RQ6 확정 전 것이고, 맨 앞의 2026-10-17 갱신 블록과 그림 아래 정정을 함께 읽을 것
3. `docs/04-islands-and-resumability.md`: 가장 중요한 설계 결정(RQ6 확정 결과)
4. 나머지 `docs/*`
5. `research-questions/*`: 현재 진행 중인 리서치 작업 대상은 RQ1
6. `roadmap/PHASES.md`: 실행 기록

## 확정된 것 vs 열린 것

**확정:**
- Layer 1 = actor/supervision tree. lifecycle, attach/detach, **지연 activation 서브트리** 경계, suspense
  경계를 맡는다. 2단계 detach와 promote-once-and-stay는 기본 모델에도 적용된다. 다만 출처가 RQ3 "잠정 규칙"이라
  실측 검증 전 설계 가설의 지위이고, 강제 drop 의미론은 미정이다.
- Layer 2 = ECS World. 상태를 저장하고, `Changed<T>` 게이팅 patch system을 돌리고, Bevy의 read/write 선언
  기반 정적 스케줄링을 쓴다. 선언은 컴파일러가 생성하고, 실행기는 SingleThreaded다.
- JSX는 컴파일타임에 static/dynamic binding site로 분해되어 직접 DOM 호출로 컴파일된다(VDOM 없음). 정적
  서브트리는 컴파일타임에 HTML로 미리 렌더한다. 경계 사례는 docs/03 판별 기준(P1–P3, N1–N5, 판정례 1–9)으로
  판정한다.
- **기본 모델 = 페이지 전체가 하나의 World, 하나의 앱 청크, 전역 위임 리스너 하나**(RQ6 H-Qwik 확정).
  - 위젯 임베드는 "하나의 World, 복수 anchor root"다.
  - `GraphMode::Islands`는 폐기하지 않고 **선택적 최적화**로 남긴다.
- Resumability는 QRL과 유사한 패턴(전역 위임 리스너 + lazy 코드 로딩)을 쓴다. 캡처 대상은 Component(= plain
  data)이고, LRef 최소 캡처는 컴파일러가 계산한다.
- 백엔드 임베딩 = FIC 계약 + C ABI shim + 세 경로(네이티브 바인딩, sidecar HTTP, WASM-in-process).
- 이름 = Loom.

**열린 것:**
- 동적 리스트/조건부 렌더링 전략의 최종 채택(RQ1. 트랙 L v22 진행 중, 트랙 R 미실행)
- 컴파일러 조건부 코드젠을 어느 후보로 맞출지. 지금 컴파일러가 내는 조건부는 **첫 spawn 지연 + CSS 은닉**이다.
  첫 `true`에 내용을 spawn하고, 이후 토글은 래퍼에 `SetVisible` 하나만 내며, 숨긴 동안에도 patch가 게이팅 없이
  돈다. 이것은 트랙 T의 **CSS 은닉 대조 arm**(벤치의 `cond_reconcile_f_system`)과 같은 알고리즘이고(정상 상태 토글 기준. SSR 초기값 false 사이트의 첫 spawn 지연은 lazy arm의 앞 절반이라, 6 arm 중 어느 것과도 정확히 같지는 않은 혼합형이다), F 계열
  (F-i/ii/iii)이 아니다. 트랙 T 판정은 B 유지이고, CSS 은닉은 대조군이라(원칙 5) 판정이 채택하지 않았다 — §9 K-3은 "CSS가 F를 지배하더라도 자동 채택은 아니다"라는 보강 조항이고, v2에서는 그 지배도 깨졌다(K-3 (1) 미발동). 채택하려면 형제
  셀렉터 참여·SSR 바이트 같은 의미론 반박에 대한 답을 RQ1에 기록해야 하는데, 그 기록도 B로 바꾸자는 결정도 없다.
  별개로, 트랙 N 불변식 4의 진짜 형태를 재려면 컴파일러가 F 계열을 낼 수 있어야 한다.
- Resumability 페이로드의 스키마·버저닝(RQ2. 잠정 포맷 `chunk|symbol|key:value`)
- Island 세분화가 실제로 이득인 경우의 프로파일링(docs/04)
- 컴파일러 표면: **연구용 좁은 부분집합**이다. 전수 감사(docs/09, 2026-10-17)는 지원·거부·부재와 함께 **패닉 없이 의미가 바뀌는 조용한 오컴파일 47건**(S1: props/children 증발, 같은 signal 두 사이트, row onClick 소실, JSX 공백 소실, Flatten `<input>` 소거 등)을 찾았다. 진단 체계 없음(전부 panic). 수리 순서(Phase 7 후보)는 병익님 결정 대기. 그 밖에 삼항 조건부, 조건부 안의 조건부, props 실제 resolve. (PatchTable(opcode) 전환은 2026-10-17 병익님
  결정으로 종결 — 제네릭 Rust System + 닫힌 열거형이 정본, opcode는 런타임 binding 종류가 필요해질 때의 재개
  조건부 대안. docs/02 "PatchTable 결정 (2026-10-17)")
- FIC `assets` 매니페스트(현재 항상 빈 배열), `request_context`, sidecar 콜드 스타트
- wasm panic 봉쇄(조건 C는 stable 채널에서 미검증), 비동기 정리 타임아웃 시 강제 drop
- LIS 최적성 증명, 입력 사전조건 4개의 검증
- 기본 모델(Flatten)의 Layer 1 결선: 6상태 기계, 2단계 detach, 호스트발 DOM 제거 감지, load/visible 트리거가
  Island 경로(`registry.js`)에만 있고 `bootstrap-flat.js`에는 없다(docs/04·06)
- 복수 위치 임베드 계약: 실측 픽스처의 위젯 5개는 한 `#app` 안에 연속으로 있었고, anchor별 조각 계약이 없으며,
  시그널을 가진 같은 컴포넌트를 두 번 참조하면 Flatten이 거부한다(docs/05)

## 2026-10-17 일괄 정정

이 README를 갱신하면서 다른 문서와 주석의 낡은 서술도 함께 현재 상태로 고쳤다. 대상은 docs/01–08, CLAUDE.md,
roadmap/PHASES.md, formal-verification 문서와 TLA+ 스텁, RQ1–RQ6 문서, 크레이트·데모 doc 주석이다. 원문은 지우지
않고 "2026-10-17 정정"을 덧붙이는 방식으로 고쳤다. RQ1 프로토콜은 append-only라 끝에 등재만 했다. RQ1 본문에는
v22 절과 §11-1(b) 등재를 넣었다. 코드는 주석만 바뀌었고, 동결된 하네스의 빌드 산출물은 그대로다.

일부러 남긴 것:
- workspace 버전 문자열 `0.0.1-phase1`: 바꾸면 Cargo.lock과 wasm-pack 산출물 메타데이터가 바뀌어 동결된 RQ1
  하네스(§2.5)에 닿을 수 있다. Firefox GC 통제 재수집이 끝난 뒤 정리한다(Cargo.toml 주석).
- `research-questions/data/verdicts/*.txt`의 "※ 범위 밖: §5 FV 계류" 줄: 커밋된 판정 기록이라 고치지 않았다.
  판정기의 고정 문구는 고쳤으므로 다음 판정문부터 반영된다.
- docs/01의 ASCII 그림과 docs/04의 "폐기된 이전 설계" 절: 이력 보존용 원문이다. 정정은 그 아래에 있다.
