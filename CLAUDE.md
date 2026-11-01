# CLAUDE.md — 이 프로젝트에서 작업할 때의 지침

## 작업 패러다임

병익님의 기존 워크플로우를 그대로 따름: **descript → verify → implement**.

- **descript**: `docs/`에 이미 있는 설계를 정확히 이해. 여기 적힌 결정은 "왜"가 함께 있으니
  그 근거를 읽지 않고 다르게 구현하지 말 것.
- **verify**: `formal-verification/`에 정의된 대상만, 정의된 범위만 검증. 프로토타입 이전에
  전체를 증명하려 하지 말 것 — 설계가 아직 움직이고 있음 (특히 RQ1 미해결. RQ6는
  2026-07-28 Phase 6에서 확정됨 — 아래 3번 정정 참고).
  **2026-10-17 주기**: RQ1은 여전히 미해결이다 — 트랙 L은 v22 §9 판정까지 나왔지만 채택 판정이
  아니고(G0 심사 기록·G1 판정 기록·Firefox GC 통제 재수집 등이 남음), 현황은 `roadmap/PHASES.md` 맨 위
  "현재 상태 요약". 공식 검증은 Lean 4 모델 증명 2건과 Verus 코드 검증(`crates/loom-flush-core`)이
  실제로 있다(아래 "이 패키지의 한계" 정정 참고).
- **implement**: `roadmap/PHASES.md` 순서를 따름. Phase를 건너뛰지 말 것. 특히 Phase 0
  (research-questions 수렴) 없이 Phase 1로 넘어가지 말 것.
  (**2026-10-17 주기**: Phase 0~6은 전부 실행됐다. 남은 일은 Phase를 새로 여는 것이 아니라 각 Phase가
  남긴 열린 항목 — 주로 RQ1 트랙 L/T/R, RQ2 — 이다. Phase 7 이상은 정의돼 있지 않다.)

## 절대 하지 말 것

1. **Virtual DOM을 재도입하지 말 것.** "일단 VDOM으로 만들고 나중에 최적화" 같은 타협 금지.
   No-VDOM은 요구조건이지 최적화 대상이 아님. 동적 시퀀스 사이트의 "좁은 전용 알고리즘"
   (keyed reconcile류)이 VDOM 재도입인지는 감으로 판단하지 말고 docs/03의 "No-VDOM 판별
   기준" 조문(2026-07-22 신설)으로만 판정할 것.
2. **Qwik/Solid/Astro/Bevy의 API를 그대로 베끼지 말 것.** 이 패키지는 그것들의 메커니즘에서
   원리를 추출해 2-layer 아키텍처 위에 재설계한 것. 표면 API가 비슷해 보여도 내부 근거
   (`docs/08-prior-art-comparison.md`)를 따라 다시 유도할 것.
2-1. 단, **바퀴는 재발명하지 말 것.** 이미 잘 풀린 문제(예: Bevy의 read/write 접근 선언 기반
   스케줄링, Qwik의 event delegation)는 메커니즘을 그대로 재사용하고, 우리가 진짜 새로 풀어야
   하는 지점(Component가 POD로 남아야 한다는 제약 하에서의 patch dispatch, island 간 통신)에만
   설계 노력을 집중할 것.
   **2026-10-17 정정**: 위 괄호의 "island 간 통신"은 2026-07-28 RQ4 전면 폐기로 기본 모델에서
   사라진 문제다 — 기본 모델은 페이지 전체가 하나의 World라 위젯 간 공유는 같은 World의 Signal로
   풀린다(`research-questions/RQ4-cross-island-shared-state.md`). 남는 것은 crash 통지 채널
   (`window.__loomShared`, "경계 간 통신"이 아니라 "상태 관찰")뿐이고, 이것은 세분화
   (`GraphMode::Islands`)를 선택했을 때 유용하게 남는다.
3. **RQ6("islands가 정말 필요한가")를 무시하고 넘어가지 말 것.** 이건 장식적 질문이 아니라
   이 프로젝트 존재 이유 중 하나에 대한 질문. Phase 1-2 프로토타입이 나오면 반드시 벤치마크로
   재검토.
   **2026-07-28 정정**: 이 지시는 실제로 이행됐음 — RQ6-verification-plan.md §7 게이트를
   위젯-임베드 정체성 통제 픽스처의 실측 데이터에 적용해 **H-Qwik 확정(강한 형태)**으로
   귀결됨(상세: `research-questions/RQ6-are-islands-necessary.md`의 "상태" 절). Island
   경계는 기본 모델에서 필수가 아니게 됐고, docs/04·05·06·07과 RQ3(부분)·RQ4(전면)가
   그 결과에 맞춰 개정됨 — 이 사실을 무시하고 "Island이 항상 필요하다"는 전제로 새 코드를
   작성하지 말 것(docs/04의 "선택적 최적화" 재해석 참고).
4. **Component에 클로저/함수 포인터를 직접 저장하지 말 것.** Resumability를 위해 Component는
   항상 직렬화 가능한 데이터여야 함. "행동"이 필요하면 `docs/03-reactivity-no-vdom.md`의
   PatchTable 패턴(정수 opcode + args → 컴파일타임 생성된 정적 디스패치 테이블)을 따를 것.
   **2026-10-17 주기**: 금지 자체는 그대로다. 다만 PatchTable(opcode 테이블)은 **아직 구현되지
   않았다** — Phase 1 이래 컴파일러는 같은 성질("정적 디스패치, Component엔 클로저 없음")을 제네릭
   Rust System(`patch_text_system::<T>` 등)과 닫힌 열거형으로 지킨다(docs/07 용어표, PHASES Phase 1
   축소 1번). ~~opcode 전환 여부는 결정 기록이 없는 열린 항목이다.~~
   **2026-10-17 병익님 결정**: "행동"은 **컴파일러가 생성한 정적 코드**에 둔다 — 타입별로 monomorphize된
   System(`patch_text_system::<T>` 등)과 닫힌 열거형의 정적 `match`(생성된 `Action`/`apply_action`/
   `resume_action_by_symbol`, `DomWrite`)가 정본이다. Component와 직렬화 페이로드에는 데이터와 심볼만 둔다.
   정수 opcode PatchTable은 **재개 조건부 대안**으로 강등한다: "binding 종류가 런타임 데이터로 정해지는
   기능이 명세되면 다시 연다 — 1차 후보는 닫힌 `BindingKind` 열거형 Component + 그것을 `match`하는 System,
   정수 opcode 테이블은 2차 후보". 근거는 docs/02 "PatchTable 결정(2026-10-17)" 절. 위 금지(클로저/함수
   포인터 저장 금지)는 그대로이고, 새 코드에서 `DomBinding`/opcode 테이블을 새로 만들지 말 것.

## 언어/툴체인 기본값 (변경 가능, 근거 있으면)

- 컴파일러 + 런타임 코어: Rust (병익님 기존 스택과 일치, Qwik 자체도 Optimizer를 Rust로 작성한 선례 있음)
- 클라이언트 배포 타겟: WASM
- JSX 파싱: SWC 기반 (Rust 생태계 내 기존 검증된 파서 재사용, 직접 파서 작성 금지 — 리서치 가치 없음)
- 백엔드 임베딩: Rust core → C ABI shim → 언어별 바인딩 (leo4의 "C shim이 lean.h 관심사를 흡수"
  패턴과 동일 구조. `docs/05-backend-agnostic-integration.md` 참고)
- 공식 검증 (2026-10-17 추가 — 실제로 쓰고 있는 것을 적음): 상태 기계·축소 모델처럼 **손으로 다시 쓴
  모델**은 Lean 4, `Vec<DomWrite>`처럼 **배포되는 Rust 코드 자체**에 계약을 걸 수 있는 대상은 Verus
  (RQ5 후보 #4, 2026-07-28 병익님 제안). 도구는 대상의 성질로 고른다. `cargo verus verify`는 vstd
  불일치로 동작하지 않으므로 `verus --crate-type=lib crates/loom-flush-core/src/lib.rs`와
  `verus formal-verification/verus/flush_order.rs`를 쓴다(`formal-verification/verus/README.md`). **2026-10-26**: Verus 0.2026.10.18 이후 기본 rlimit로는 `chain_unlink_denotes_remove`가 자원 한도를 넘는다 — 두 명령 모두 `--rlimit 60`을 붙이면 90/28 verified로 통과한다(README 맨 끝). **도구 버전은 고정이다**(`tools/toolchain-pins.env`, `rust-toolchain.toml` — loom stable 릴리즈 전까지; bump는 Verus breaking release/rustc major bump 때 개별 검토). 빌드·검증 전 `tools/check-toolchain.sh`. **2026-10-30**: 활성 툴체인에 `rust-src`가 설치돼 있으면 wasm 바이트가 바뀐다(std 경로가 로컬 sysroot 경로로 박힘) — 고정 툴체인 1.99.1에는 설치하지 말 것(검사 스크립트가 대조). 기존 `target/` 캐시는 고정 이전 산출물이 섞여 있을 수 있으므로 **측정·동결 산출물은 `tools/build-frozen-bench.sh`(빈 target 빌드 + 경로 오염 단언)로만** 만든다(RQ1 프로토콜 개정 #12 후속 6). **2026-10-30 정정(같은 날)**: "1.99.1에 rust-src를 설치하지 말 것"은 폐기 — 병익님이 설치했고, `build-frozen-bench.sh`가 `--remap-path-prefix=<sysroot>/lib/rustlib/src/rust=/rustc/<해시>`로 경로를 되돌려 rust-src가 있어도 동결본과 같은 바이트를 낸다(검사 스크립트는 rust-src 유무를 정보로만 출력; 개정 #12 후속 7). remap 없는 일반 `wasm-pack build` 산출물은 여전히 바이트가 다르다 — 측정·동결엔 쓰지 말 것. **2026-11-01 빌드 캐시 위치**(병익님 지시): 이 프로젝트의 모든 빌드 캐시는 `/run/media/ybi/T9/.build-cache/loom-research/`(worktree는 `loom-research-wt-<이름>/`)에 둔다 — `target`·`tools/surface-probe/target`은 그곳을 가리키는 링크이고 `tools/build-cache.sh`(`--link`, `just cache`)가 맞춘다. 동결 빌드의 빈 target, 컴파일러 게이트의 `$G`, WebKit Docker의 npm 캐시, `just`의 파이썬 바이트코드 캐시도 그 아래다. 저장소 안이나 `/tmp`에 새 target을 만들지 말 것.

## 진행 중 발견한 것은 문서에 반영

설계가 이 패키지와 다르게 가야 한다는 증거를 찾으면(벤치마크, 반례, 막다른 길) 조용히
방향을 바꾸지 말고 해당 `docs/*.md` 또는 `research-questions/*.md`를 업데이트하고 왜
바뀌었는지 남길 것. 이 패키지 자체가 살아있는 스펙임.

## 이 패키지의 한계

이 문서들은 한 번의 설계 대화에서 나온 결정이지 검증된 진실이 아님. 특히:
- RQ1, RQ2는 실제로 열려 있고 실험이 필요함. RQ3는 부분 폐기, RQ4는 전면 폐기,
  RQ6는 확정됨(2026-07-28 정정, 위 "절대 하지 말 것" 3번 참고) —
  (RQ3는 `roadmap/PHASES.md` Phase 0의 수렴 대상인데 초판 목록에서 누락되어 있었음 — 2026-07-22 정정)
  **2026-10-17 주기**: 위 목록에 RQ5가 빠져 있었다 — RQ5(공식 검증 대상)는 후보 #1~#4가 전부
  이행됐다(#4는 2026-08-11 LIS 건전성까지). RQ6 확정은 §7 (a)를 반사실 조항으로 읽는 해석(병익님
  확정)과 위젯 5개 규모라는 한계 위에 서 있다 — "Island은 필요 없다"로 일반화해 전달하지 말 것.
- formal-verification/의 Lean4/TLA+ 파일은 스켈레톤이지 증명이 아님
  **2026-10-17 정정**: 이 줄은 2026-07-24 이후 사실이 아니다. Lean 4 두 파일
  (`SupervisionTree.lean`, `ResumabilitySoundness.lean`)은 `sorry` 0으로 증명이 끝났다 — 단 **모델**
  증명이라 모델이 코드와 일치한다는 가정 위에 선다. TLA+(`SchedulerSafety.stub.tla`)만 Bevy에 위임한
  **의도적 스텁**이다. 이 줄이 언급조차 안 한 Verus 트랙이 있다: 모델 `verus/flush_order.rs` 28 verified,
  배포 코드 `crates/loom-flush-core` 90 verified, 0 errors(2026-08-11 기준선). 증명되지 않은 것도 남아
  있다 — LIS 최적성, 원시 입력의 구조적 사전조건 넷(`debug_assert!`로만 지켜짐), `ownedEntities`의
  despawn 시점. 상세: `formal-verification/README.md`, `roadmap/PHASES.md` Phase 5 절.
- examples/의 JSX 예시는 문법 목업이지 실제로 컴파일되지 않음
  (**2026-10-17 주기**: 이 목업의 `island()` 래퍼는 "Island이 항상 필요하다"는 RQ6 확정 이전 전제로
  쓰였다. 실제 컴파일러가 받는 TSX 부분집합은 `crates/loom-compiler/src/lower.rs` 모듈 문서와
  `demo/*.tsx`가 기준이다.)

이 사실을 숨기거나 마치 다 정해진 것처럼 다음 사람에게 전달하지 말 것.
