# 06. Lifecycle Actor 계층 (Layer 1) — UI 용어로 구체화

이전 대화의 "RIB × ECS 융합" 논의를 UI 프레임워크 용어로 재정리.

**2026-07-28 (Phase 6, Task #29) 갱신**: RQ6 §7 게이트가 H-Qwik을 확정(강한
형태)함에 따라, 아래 "Island root와의 관계" 절이 전제했던 "IslandRoot =
supervision root"라는 개념을 제거한다 — Island 경계가 기본 모델에서
필수가 아니게 됐으므로(docs/04 재해석 참고), supervision subtree의 root는
"Island"라는 임베딩 단위가 아니라 **"지연 activation이 걸린 서브트리"**
일반 개념으로 재정의된다. 아래는 그 재정의를 반영한 버전이다.

## 대응 관계

| RIB/Actor 개념 | 이 프레임워크에서 |
|---|---|
| Interactor | 컴포넌트 인스턴스의 actor (mount 시 spawn) |
| attach | mount = actor spawn + World에 Entity spawn |
| detach | unmount = actor terminate(구조적 동시성으로 in-flight 작업 자동 취소) + Entity despawn(cascade) |
| Router 트리 | supervision tree — 반드시 뷰 계층과 일치할 필요 없음(RIB의 viewless 개념 계승) |
| Interactor가 스트림 구독 | actor가 async 작업(fetch 등)을 await, 완료 시 Command로 Component에 반영 |

## 지연 activation 서브트리와의 관계 (구 "Island root와의 관계")

기본 모델(docs/04)에서는 World가 하나뿐이므로 "Island root actor"라는 별도
개념이 없다 — 페이지 전체가 하나의 supervision tree이고, `client:load`/
`client:visible`이 표시하는 것은 별도 root가 아니라 **그 tree 안의 한
서브트리에 걸린 지연 activation 트리거**다. 그 서브트리의 actor가
terminate되면 그 아래 자손이 cascade로 정리된다는 점은 원래 설계와 동일 —
바뀐 건 "이게 반드시 별도 격리 World의 경계와 일치해야 하는가"뿐이다(아니다).

**세분화를 선택했을 때(docs/04의 "선택적 최적화" 절)**: 그때는 여전히
아래 *RQ3 잠정 규칙*이 그대로 적용된다 — "부모가 자식의 생사를 관장"은
**세분화된 경계 내부에서만** 성립하고, 경계를 넘는 supervision edge는
0개(supervision forest), 중첩된 경계 간 관계는 중립 Registry의 containment로만
추적한다. 동반 해제는 supervision cascade가 아니라 "실제 DOM 범위 제거"에
결속된 2단계 프로토콜(동기: detaching 표시 + DOM 접근 차단 + DOM 즉시 제거 /
비동기: 구조적 동시성 취소 후 detached 전이)로 처리한다 — 이 프로토콜 자체는
RQ3가 이미 확인한 대로 "지연 활성화되는 서브트리의 안전한 해제"라는 더 일반적인
문제의 답이라 세분화 여부와 무관하게 유효하다(RQ3의 부분 폐기 절 참고).

## Suspense/비동기 데이터 경계

RIB의 Interactor가 스트림을 직접 트리거하던 자리를, 여기서는 actor가 `Future`를 await하고
resolve되면 Command 큐를 통해 Component를 갱신하는 패턴으로 대체 (이미 이전 대화에서 합의됨).
"로딩 중" UI는 이 await 구간 동안의 Component 상태(예: `Loading` variant)로 표현되고,
patch system이 그 상태를 감지해 폴백 마크업을 렌더링.

## Resumability와의 상호작용 — 지연 spawn

일반 mount는 즉시 actor를 spawn하지만, resumability가 걸린 서브트리(기본 모델에서는
페이지 전체 하나)의 경우 **spawn 자체가 LRef 호출 시점까지 지연됨**. 즉 "mount"와
"actor 존재"가 분리됨:
- SSR: mount는 일어났지만(HTML은 나옴) client-side actor는 존재하지 않음
- Client, 상호작용 전: 여전히 actor 없음, DOM만 존재
- 상호작용 발생: LRef가 청크를 로드하고, 그 시점에 최소한의 actor+Entity가 materialize됨
  (기본 모델에서는 이 하나의 materialize가 페이지 전체의 actor를 한 번에 깨움 — M2/M3
  실측이 확인한 대로, 어느 anchor에서 첫 클릭이 일어나든 동일)

이게 04에서 "resume ≠ hydrate"라고 한 것의 Layer 1 쪽 구현. 승격 정책은 RQ3 잠정
규칙(2026-07-23)으로 결정됨: **promote-once-and-stay** — 한 번 attached된 actor는
detach까지 상주하고, attached→dormant 역전이는 없음("승격은 resumability의
비가역적 소비"). 매 상호작용 재지연안은 클라이언트 재직렬화기(미해결 RQ2 전체)를 매
이벤트마다 요구해 기각. crash는 restart 없이 failed 전이 + DOM 동결(Phase 0). 이
정책 자체는 세분화 여부와 무관 — 기본 모델(페이지 전체가 하나)에서도, 세분화를
선택한 모델(RQ3의 여러 독립 서브트리)에서도 동일하게 적용된다. 근거와 재검토 트리거는
RQ3 참고.

## 미해결

- ~~중첩 Island에서 supervision 전파 규칙~~ → RQ3 잠정 규칙으로 결정됨(2026-07-23,
  Phase 3까지 반복 개선), **2026-07-28 RQ6 §7 게이트로 "세분화를 선택했을 때만
  적용되는 규칙"으로 지위 하락**(docs/04/RQ3 참고 — 기본 모델에서는 애초에 중첩
  경계가 없어 이 문제 자체가 발생하지 않음). 잔여 미해결은 RQ3의 "알려진 약점"
  참고 — 특히 wasm panic 봉쇄 방식(fault 격리의 전제, 세분화 선택 시에만 의미
  있음), 비동기 정리 타임아웃 시 강제 drop 의미론, 호스트발 DOM 제거 감지 경로.
