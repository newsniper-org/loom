# 07. 네이밍/용어집

## 프레임워크 이름

**"loom"은 가칭.** 2-layer(actor + ECS)를 "베 짜기"처럼 엮는다는 은유에서 임시로 붙임.
다른 이름으로 확정 시 이 저장소 전체에서 대소문자 구분 없이 전역 치환 필요
(`loom`, `Loom`, `LOOM_*` 매크로/환경변수 접두어 등).

## 용어 대응표

| 이 프로젝트 용어 | 정의 | 유사 개념 (참고용, 동일하지 않음) |
|---|---|---|
| **Island** | 컴파일타임에 표시된, client 상호작용이 필요한 JSX 서브트리 경계 | Astro의 island, Qwik이 명시적으로 채택 안 한 개념 |
| **IslandWorld** | Island 하나에 대응하는 독립 ECS World partition (Layer 2) | Bevy의 `World` (스코프만 좁힌 것) |
| **IslandRoot** | Island에 대응하는 supervision subtree의 root actor (Layer 1) | RIB의 최상위 Interactor / Erlang supervisor |
| **LRef** (Lazy Reference) | `{chunk, symbol, captures}` — 지연 로딩되는 이벤트 핸들러/구독 참조 | Qwik의 QRL, 단 캡처 대상이 ECS Component로 제한됨 |
| **PatchTable** | 컴파일러가 생성하는 정적 디스패치 테이블(opcode → 실제 DOM 패치 함수) | 없음 (이 프로젝트 고유 설계) — **실제로는 미구현**: Phase 1이 이 대신 제네릭 Rust System(예: `patch_text_system<T>`)을 직접 등록하는 방식으로 갔음, 아래 DomBinding도 마찬가지 |
| **DomBinding** | Component. `{patch_op: u16, args: [ComponentRef]}` — **미구현, 이 문서의 원래 설계 스케치일 뿐** | 없음 |
| **Signal\<T\>** | 값 + dirty flag를 가진 Component. 반응성의 최소 단위 | Solid/Vue의 Signal, 단 저장소가 클로저가 아니라 ECS storage |
| **FIC** (Framework Integration Contract) | `(component, props, request_context) -> (html, resumability_payload, assets)` 순수 함수 계약 | Astro SSR adapter의 언어 무관 확장판 |

## 명명 시 유의

- Qwik의 "QRL"과 혼동되지 않도록 이벤트/구독 지연 참조는 항상 **LRef**로 통일 표기
- "Island"라는 용어를 쓰되, RQ6가 풀리기 전까지는 이 개념 자체의 존속 여부가 열려있다는 점을
  코드 주석에도 명시 (예: 나중에 이름을 통째로 바꿔야 할 수도 있음)
- **"ComponentRef" 용어 충돌 (2026-07-24 발견, Task #25)**: 이 문서와 docs/02가 원래
  "ComponentRef"를 미구현 `DomBinding`의 인자 타입(위 표)이라는 의미로 썼는데, Task #25
  (컴파일러 Island 통합, 증분 4)가 `roadmap/PHASES.md`/`RQ6-verification-plan.md`에
  이미 사전등록된 이름 그대로 **"JSX에서 다른 authored 컴포넌트를 참조하는 노드"**라는
  전혀 다른, 실제 구현된 의미로 같은 이름을 다시 씀
  (`crates/loom-compiler/src/ir.rs`의 `JsxNode::ComponentRef`). 조용히 덮어쓰지 않고
  여기 명시: **오늘부로 "ComponentRef"는 후자(ir.rs의 실제 타입)를 가리킨다** — 전자
  (DomBinding의 인자)는 애초에 구현된 적 없는 설계 스케치이므로 이 이름을 계속 쓸
  근거가 약하고, 실제로 구현되어 사용 중인 쪽이 이 이름의 정본이 되는 게 자연스러움.
