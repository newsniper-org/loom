# RQ4. Island 간 통신

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
