# RQ4. Island 간 통신

> **2026-07-28 (Phase 6, Task #29) — 전면 폐기.** RQ6 §7 게이트가 H-Qwik을
> 확정(강한 형태)함에 따라, 이 RQ가 다루던 문제 자체가 소거된다 — 기본 모델은
> 페이지 전체가 하나의 `bevy_ecs::World`이므로(docs/04 재해석), "서로 다른
> Island 경계를 넘는 통신"이라는 문제 정의 자체가 성립하지 않는다. 여러 위젯이
> 같은 값을 공유해야 하는 요구(예: 장바구니 배지)는 이제 "경계를 넘는 통신"이
> 아니라 **그냥 같은 World 안의 평범한 Signal 하나를 여러 DOM anchor가
> 구독하는 것** — widget-embed 픽스처의 CartBadge가 이미 이렇게 동작한다
> (`{sharedValue("cartCount", 0)}`가 Flatten 모드에서는 특별한 메커니즘 없이
> 그냥 같은 World 내 Signal 읽기로 자연 실현됨 — Task #26의 `graph_mode_
> flatten_merges_nested_components_into_one_signals_and_action` 테스트가 이미
> 확인한 사실).
>
> **완전히 사라지지 않는 것**: 아래 "RQ3에서 이관된 요구"의 **crash 통지
> 채널**은 Task #28-b가 이미 구현했고(`window.__loomShared`의
> `__island_state:<id>` 키), 이건 "경계 간 통신"이 아니라 "상태 관찰"이라
> 세분화를 선택했을 때(RQ3의 부분 폐기 절 참고) 여전히 유용한 메커니즘으로
> 남는다 — RQ4가 폐기된다고 그 구현까지 되돌리지 않는다.
>
> 아래 원문은 수정하지 않는다(이력 보존 — 이 RQ가 실제로 풀렸던 최소 슬라이스
> 구현과 그 과정에서 나온 통찰은 여전히 유효한 기록이다).

## 문제

Qwik/Astro 둘 다 "island끼리 통신 불가"를 명시된 한계로 인정함 (04 참고). 실제 앱에서는
흔한 요구사항임 (예: 헤더의 장바구니 개수 위젯이 상품 상세 페이지의 "담기" 위젯 상태 변화를
반영해야 함) — 이 두 위젯이 서로 다른 Island일 가능성이 높음.

## 04에서 제안한 후보안

**공유 lazy signal store**: 여러 Island이 구독 가능한 전역 Signal 집합. 구독도 LRef 방식으로
지연 등록(구독 등록 자체가 실제 이벤트 발생 전까지 코드를 안 당김).

## 검토해야 할 문제

1. 이 공유 store가 "각 Island은 완전히 독립적으로 resume/격리된다"는 04의 핵심 속성을
   얼마나 깨뜨리는가 — 공유 store를 통한 의존성은 결국 두 Island의 supervision을 은연중에
   결합시킬 위험 (RQ3와 상호작용)
2. 공유 store 자체는 어느 Layer에 속하는가 — 특정 Island의 IslandWorld에 속할 수 없으므로
   앱 전역 World(모든 IslandWorld의 부모격)가 필요한가? 이건 "Island = 완전히 독립된
   World partition"이라는 전제를 수정해야 할 수도 있음
3. 대안: 공유 store 없이 항상 백엔드를 경유(서버가 진실 공급원, 각 Island은 각자 polling/SSE로
   갱신) — island 격리는 지키지만 실시간성이 떨어지고 백엔드 의존이 늘어남 (05의 backend-agnostic
   목표와 미묘하게 긴장 — 특정 상태 동기화 메커니즘을 프레임워크가 강제하게 됨)

## RQ3에서 이관된 요구 (2026-07-23)

- **crash 통지 채널**: RQ3 검토 중 "부모가 자식 island의 crash를 관찰하는 monitor 채널"
  안이 제안되었으나, 등록 시점 순서 의존으로 통지가 조용히 유실되는 결함 + 사실상 최초의
  cross-island 메시지 채널이라 RQ4를 선점한다는 이유로 Phase 0에서 제외되고 이 RQ로
  이관됨. cross-island 통신을 설계할 때 crash/상태 변화 통지를 같은 메커니즘의 특수
  케이스로 다룰 수 있는지 검토할 것.
- **IslandRegistry 스키마 폐쇄 경계**: RQ3가 도입한 Registry는 토폴로지 추적 + detach
  발행 전용으로 스키마가 폐쇄됨. RQ4 해결안이 Registry를 공유 상태 부착점으로 요구하면
  그 경계의 재협상이 필요함 (RQ3 재검토 트리거 8번).
- **SSR prop 스냅샷 동결의 한계**: RQ3 Q2에 따라 부모→자식 데이터는 SSR 시점에 동결되고
  동적 전달은 전적으로 이 RQ의 관할. 동결이 실무 불가로 판정되면 RQ3 Q2의 재협상 트리거.

## 상태

미착수. Phase 3 진행 중 RQ3와 함께 다뤄야 함 — 둘이 독립적으로 풀리지 않을 가능성 높음.
(RQ3 쪽은 2026-07-23 잠정 규칙 확정 — 위 이관 항목 참고.)

**2026-07-24 (Phase 3) — 최소 슬라이스 구현됨**: 검토 문제 1번(공유 store가 격리를 얼마나
깨는가)과 2번(Layer 소속)에 대한 실제 답. **결정**: 공유 store는 항상 로드되는 작은 JS
(`demo/resume/shared-store.js`, `window.__loomShared.set/get`) + DOM 마커 기반 구독
(`data-loom-shared-subscriber`) — 04 원안의 "구독도 LRef로 지연 등록"과 **다르게 감**(순수
표시용 구독에 청크 로드를 요구하는 게 이득 없이 비용만 큼 — docs/04에 이유 상세 기록).
문제 2번(어느 Layer 소속인가)의 답: 특정 Island의 World에 속할 수 없으므로 **"Layer 0 —
Host Glue"**로 명명한 상시 상주 계층에 둠(docs/04 참고) — "Island = 완전 독립 World
partition"이라는 원 전제의 부분 수정이지만, 공개 표면이 폐쇄된 get/set/notify뿐이라 "하나의
큰 공유 World로 붕괴"하지는 않음.

**실제 검증(Playwright, crates/loom-island-demo + demo/islands/index.html)**: QuantityPicker
(client:load)의 수량 변경 → `loom_shared_set("cartCount", n)` 호출 → CartBadge(자기 wasm
청크가 아예 없음)가 순수 JS DOM patch로 반영. **CartBadge 자신의 청크는 전혀 로드되지
않음**을 실측 확인(RQ6 §4.2 시나리오와 동일 패턴) — dormant island가 필요조차 없는, "Island
자체가 없어도 되는" 더 강한 경우임을 보여줌.

**문제 3번(백엔드 경유 대안)은 여전히 미착수** — 이번 라운드는 프레임워크 내부 공유
메커니즘만 구현. **crash 통지 채널(RQ3에서 이관된 요구)도 미착수** — 이번 최소 슬라이스는
값 공유만 다루고 crash/상태 변화 통지는 다루지 않음. **저작 표면(`sharedSignal()` 같은
컴파일러 확장)도 스트레치로 미룸** — 이번엔 손코딩 레벨(`loom_shared_set` extern import
직접 호출)까지만.

**2026-10-17 — 현행 확인**: 상태는 그대로 **전면 폐기**(2026-07-28)다. 그 뒤 이 문서를 고친 커밋은 없고, 폐기를 되돌릴
사실도 찾지 못했다. 위 "상태" 절에서 "미착수"로 적힌 항목 가운데 둘은 최상단 폐기 공지가 이미 이행으로 적고 있다.
crash 통지 채널은 Task #28-b가 `window.__loomShared`의 `__island_state:<id>` 키로 이행했다. 저작 표면은 Task #26에서
`sharedValue(...)`로 들어갔다. 공지에 인용된 `graph_mode_flatten_merges_nested_components_into_one_signals_and_action`
테스트가 Flatten 쪽을 확인한다. 미착수로 남은 것은 문제 3번(백엔드 경유 대안)뿐이다. 기본 모델에서는 이 문제 자체가
성립하지 않는다.
