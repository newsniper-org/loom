# 06. Lifecycle Actor 계층 (Layer 1) — UI 용어로 구체화

이전 대화의 "RIB × ECS 융합" 논의를 UI 프레임워크 용어로 재정리.

## 대응 관계

| RIB/Actor 개념 | 이 프레임워크에서 |
|---|---|
| Interactor | 컴포넌트 인스턴스의 actor (mount 시 spawn) |
| attach | mount = actor spawn + IslandWorld에 Entity spawn |
| detach | unmount = actor terminate(구조적 동시성으로 in-flight 작업 자동 취소) + Entity despawn(cascade) |
| Router 트리 | supervision tree — 반드시 뷰 계층과 일치할 필요 없음(RIB의 viewless 개념 계승) |
| Interactor가 스트림 구독 | actor가 async 작업(fetch 등)을 await, 완료 시 Command로 Component에 반영 |

## Island root와의 관계

Island = 이 supervision tree에서 **하나의 독립된 subtree root**. Island root actor가
terminate되면 그 아래 전체 subtree가 cascade로 정리됨 — RIB의 detach 전파와 동일 원리.
04에서 다룬 "Island = 격리 단위"라는 정의가 여기서 구체적으로 구현됨: 격리는 이 supervision
subtree 경계로 실현됨.

*RQ3 잠정 규칙 반영(2026-07-23)*: "부모가 자식의 생사를 관장"은 **island 내부에서만**
성립함 — island 경계를 넘는 supervision edge는 0개(supervision forest), 중첩 island 간
관계는 중립 IslandRegistry의 containment로만 추적. 중첩 island의 동반 detach는
supervision cascade가 아니라 "실제 DOM 범위 제거"에 결속된 2단계 프로토콜(동기: detaching
표시 + DOM 접근 차단 + DOM 즉시 제거 / 비동기: 구조적 동시성 취소 후 detached 전이)로
처리. 상세와 재검토 트리거는 RQ3.

## Suspense/비동기 데이터 경계

RIB의 Interactor가 스트림을 직접 트리거하던 자리를, 여기서는 actor가 `Future`를 await하고
resolve되면 Command 큐를 통해 Component를 갱신하는 패턴으로 대체 (이미 이전 대화에서 합의됨).
"로딩 중" UI는 이 await 구간 동안의 Component 상태(예: `Loading` variant)로 표현되고,
patch system이 그 상태를 감지해 폴백 마크업을 렌더링.

## Resumability와의 상호작용 — 지연 spawn

일반 mount는 즉시 actor를 spawn하지만, resumability가 걸린 Island의 경우 **spawn 자체가
LRef 호출 시점까지 지연됨**. 즉 "mount"와 "actor 존재"가 분리됨:
- SSR: mount는 일어났지만(HTML은 나옴) client-side actor는 존재하지 않음
- Client, 상호작용 전: 여전히 actor 없음, DOM만 존재
- 상호작용 발생: LRef가 청크를 로드하고, 그 시점에 최소한의 actor+Entity가 materialize됨

이게 04에서 "resume ≠ hydrate"라고 한 것의 Layer 1 쪽 구현. 승격 정책은 RQ3 잠정
규칙(2026-07-23)으로 결정됨: **promote-once-and-stay** — 한 번 attached된 island root
actor는 detach까지 상주하고, attached→dormant 역전이는 없음("승격은 resumability의
비가역적 소비"). 매 상호작용 재지연안은 클라이언트 재직렬화기(미해결 RQ2 전체)를 매
이벤트마다 요구해 기각. crash는 restart 없이 failed 전이 + DOM 동결(Phase 0). 근거와
재검토 트리거는 RQ3 참고.

## 미해결

- ~~중첩 Island에서 supervision 전파 규칙~~ → RQ3 잠정 규칙으로 결정됨(2026-07-23,
  Phase 3까지 반복 개선). 잔여 미해결은 RQ3의 "알려진 약점" 참고 — 특히 wasm panic
  봉쇄 방식(fault 격리의 전제), 비동기 정리 타임아웃 시 강제 drop 의미론, 호스트발
  DOM 제거 감지 경로.
