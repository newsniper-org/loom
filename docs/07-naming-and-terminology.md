# 07. 네이밍/용어집

## 프레임워크 이름

**"loom"은 가칭.** 2-layer(actor + ECS)를 "베 짜기"처럼 엮는다는 은유에서 임시로 붙임.
다른 이름으로 확정 시 이 저장소 전체에서 대소문자 구분 없이 전역 치환 필요
(`loom`, `Loom`, `LOOM_*` 매크로/환경변수 접두어 등).

## 용어 대응표

**2026-07-28 (Phase 6, Task #29) 갱신**: RQ6 §7 게이트가 H-Qwik을 확정(강한
형태)함에 따라 "Island"가 기본 모델에서 필수 개념이 아니게 됐다(docs/04 참고).
아래는 그 재해석을 반영해 갱신한 표 — "Island"/"IslandWorld"/"IslandRoot"는
삭제하지 않고 **"세분화를 선택했을 때만 쓰는 용어"**로 재정의한다(조용히
지우면 `crates/loom-widget-embed-island-*` 등 실제 코드의 식별자와 어긋나므로).

| 이 프로젝트 용어 | 정의 | 유사 개념 (참고용, 동일하지 않음) |
|---|---|---|
| **지연 activation 서브트리** (신설, 04 기본 모델의 정본 용어) | `client:load`/`client:visible` 지시어가 표시하는, 별도 실행 단위가 아니라 페이지 전체의 하나의 World 안에서 지연 트리거가 걸린 서브트리 | Qwik의 lazy-loaded component 경계 (Qwik이 채택한 것과 사실상 같은 개념으로 수렴) |
| **Island** (세분화 선택 시에만 사용) | 개발자가 명시적으로 별도 컴파일 단위(별도 wasm 산출물)로 분리하기로 선택한 지연 activation 서브트리 | Astro의 island — 단, 이제 "필수 임베딩 단위"가 아니라 "선택적 최적화" |
| **IslandWorld** (세분화 선택 시에만 사용) | 세분화된 Island 하나에 대응하는 독립 ECS World partition (Layer 2) | Bevy의 `World` (스코프만 좁힌 것) |
| **IslandRoot** → **폐지, "지연 activation 서브트리의 root actor"로 대체** | (구) Island에 대응하는 supervision subtree의 root actor | RIB의 최상위 Interactor / Erlang supervisor |
| **LRef** (Lazy Reference) | `{chunk, symbol, captures}` — 지연 로딩되는 이벤트 핸들러/구독 참조. 기본 모델에서는 페이지 전체가 보통 `chunk` 하나(`App_chunk`)를 공유 | Qwik의 QRL, 단 캡처 대상이 ECS Component로 제한됨 |
| **PatchTable** | 컴파일러가 생성하는 정적 디스패치 테이블(opcode → 실제 DOM 패치 함수) | 없음 (이 프로젝트 고유 설계) — **실제로는 미구현**: Phase 1이 이 대신 제네릭 Rust System(예: `patch_text_system<T>`)을 직접 등록하는 방식으로 갔음, 아래 DomBinding도 마찬가지 |
| **DomBinding** | Component. `{patch_op: u16, args: [ComponentRef]}` — **미구현, 이 문서의 원래 설계 스케치일 뿐** | 없음 |
| **Signal\<T\>** | 값 + dirty flag를 가진 Component. 반응성의 최소 단위 | Solid/Vue의 Signal, 단 저장소가 클로저가 아니라 ECS storage |
| **FIC** (Framework Integration Contract) | `(component, props, request_context) -> (html, resumability_payload, assets)` 순수 함수 계약 | Astro SSR adapter의 언어 무관 확장판 |

## 명명 시 유의

- Qwik의 "QRL"과 혼동되지 않도록 이벤트/구독 지연 참조는 항상 **LRef**로 통일 표기
- ~~"Island"라는 용어를 쓰되, RQ6가 풀리기 전까지는 이 개념 자체의 존속 여부가
  열려있다는 점을 코드 주석에도 명시~~ → **RQ6가 풀렸다(2026-07-28, H-Qwik 확정)**:
  "Island"는 이제 폐지되지 않되 위 표대로 "세분화를 선택했을 때"로 좁혀진 의미로만
  쓴다 — 새 코드에서 "이 서브트리는 지연 activation된다"는 뜻으로 무심코 "Island"를
  쓰지 말 것(기본 모델엔 별도 World 경계가 없으므로 부정확함).
- **"ComponentRef" 용어 충돌 (2026-07-24 발견, Task #25)**: 이 문서와 docs/02가 원래
  "ComponentRef"를 미구현 `DomBinding`의 인자 타입(위 표)이라는 의미로 썼는데, Task #25
  (컴파일러 Island 통합, 증분 4)가 `roadmap/PHASES.md`/`RQ6-verification-plan.md`에
  이미 사전등록된 이름 그대로 **"JSX에서 다른 authored 컴포넌트를 참조하는 노드"**라는
  전혀 다른, 실제 구현된 의미로 같은 이름을 다시 씀
  (`crates/loom-compiler/src/ir.rs`의 `JsxNode::ComponentRef`). 조용히 덮어쓰지 않고
  여기 명시: **오늘부로 "ComponentRef"는 후자(ir.rs의 실제 타입)를 가리킨다** — 전자
  (DomBinding의 인자)는 애초에 구현된 적 없는 설계 스케치이므로 이 이름을 계속 쓸
  근거가 약하고, 실제로 구현되어 사용 중인 쪽이 이 이름의 정본이 되는 게 자연스러움.
  이 용법은 "세분화 선택 시" 의미로도(각 Island가 자기 컴포넌트를 참조) 그대로
  쓰이므로 위 RQ6 재해석과 무관하게 유지된다.
