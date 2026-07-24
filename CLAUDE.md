# CLAUDE.md — 이 프로젝트에서 작업할 때의 지침

## 작업 패러다임

병익님의 기존 워크플로우를 그대로 따름: **descript → verify → implement**.

- **descript**: `docs/`에 이미 있는 설계를 정확히 이해. 여기 적힌 결정은 "왜"가 함께 있으니
  그 근거를 읽지 않고 다르게 구현하지 말 것.
- **verify**: `formal-verification/`에 정의된 대상만, 정의된 범위만 검증. 프로토타입 이전에
  전체를 증명하려 하지 말 것 — 설계가 아직 움직이고 있음 (특히 RQ1, RQ6 미해결).
- **implement**: `roadmap/PHASES.md` 순서를 따름. Phase를 건너뛰지 말 것. 특히 Phase 0
  (research-questions 수렴) 없이 Phase 1로 넘어가지 말 것.

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
3. **RQ6("islands가 정말 필요한가")를 무시하고 넘어가지 말 것.** 이건 장식적 질문이 아니라
   이 프로젝트 존재 이유 중 하나에 대한 질문. Phase 1-2 프로토타입이 나오면 반드시 벤치마크로
   재검토.
4. **Component에 클로저/함수 포인터를 직접 저장하지 말 것.** Resumability를 위해 Component는
   항상 직렬화 가능한 데이터여야 함. "행동"이 필요하면 `docs/03-reactivity-no-vdom.md`의
   PatchTable 패턴(정수 opcode + args → 컴파일타임 생성된 정적 디스패치 테이블)을 따를 것.

## 언어/툴체인 기본값 (변경 가능, 근거 있으면)

- 컴파일러 + 런타임 코어: Rust (병익님 기존 스택과 일치, Qwik 자체도 Optimizer를 Rust로 작성한 선례 있음)
- 클라이언트 배포 타겟: WASM
- JSX 파싱: SWC 기반 (Rust 생태계 내 기존 검증된 파서 재사용, 직접 파서 작성 금지 — 리서치 가치 없음)
- 백엔드 임베딩: Rust core → C ABI shim → 언어별 바인딩 (leo4의 "C shim이 lean.h 관심사를 흡수"
  패턴과 동일 구조. `docs/05-backend-agnostic-integration.md` 참고)

## 진행 중 발견한 것은 문서에 반영

설계가 이 패키지와 다르게 가야 한다는 증거를 찾으면(벤치마크, 반례, 막다른 길) 조용히
방향을 바꾸지 말고 해당 `docs/*.md` 또는 `research-questions/*.md`를 업데이트하고 왜
바뀌었는지 남길 것. 이 패키지 자체가 살아있는 스펙임.

## 이 패키지의 한계

이 문서들은 한 번의 설계 대화에서 나온 결정이지 검증된 진실이 아님. 특히:
- RQ1, RQ2, RQ3, RQ4, RQ6는 실제로 열려 있고 실험이 필요함
  (RQ3는 `roadmap/PHASES.md` Phase 0의 수렴 대상인데 초판 목록에서 누락되어 있었음 — 2026-07-22 정정)
- formal-verification/의 Lean4/TLA+ 파일은 스켈레톤이지 증명이 아님
- examples/의 JSX 예시는 문법 목업이지 실제로 컴파일되지 않음

이 사실을 숨기거나 마치 다 정해진 것처럼 다음 사람에게 전달하지 말 것.
