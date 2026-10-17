# 06. Lifecycle Actor 계층 (Layer 1) — UI 용어로 구체화

이전 대화의 "RIB × ECS 융합" 논의를 UI 프레임워크 용어로 재정리.

**2026-07-28 (Phase 6, Task #29) 갱신**: RQ6 §7 게이트가 H-Qwik을 확정(강한
형태)함에 따라, 아래 "Island root와의 관계" 절이 전제했던 "IslandRoot =
supervision root"라는 개념을 제거한다 — Island 경계가 기본 모델에서
필수가 아니게 됐으므로(docs/04 재해석 참고), supervision subtree의 root는
"Island"라는 임베딩 단위가 아니라 **"지연 activation이 걸린 서브트리"**
일반 개념으로 재정의된다. 아래는 그 재정의를 반영한 버전이다.

> **2026-10-17 현황 — 이 문서의 설계 중 실제로 구현된 범위**: 2026-07-28 이후 RQ3·RQ6에 판정을 바꾸는
> 새 실측이나 개정은 없고(2026-10-17에 현행 확인 부기와 RQ3 "알려진 약점" 6의 문구 정정만 추가됐다), 아래 설계 서술은 그대로다. 다만 구현 범위가 설계보다 좁다는 점이 이 문서에 적혀 있지
> 않았다(2026-10-17 crates/·demo/ 코드 확인).
> - **구현된 것**: RQ3 6상태 기계(dormant/attaching/attached/detaching/detached/failed)와 2단계 detach가
>   **Island 경로**에 있다 — Rust `crates/loom-runtime/src/island.rs`(`NativeIslandRegistry`, 테스트 11개)와
>   JS `demo/resume/registry.js`(load/visible 트리거, MutationObserver 기반 호스트발 제거 감지). 그 모델은
>   `formal-verification/lean4/SupervisionTree.lean`에서 증명됐다(`sorry` 0, 6상태 정리 3개 + Entity 소유권
>   `ownedEntities` 정리 2개. `ownedEntities`가 실제로 비워지는 despawn 시점은 모델링하지 않았다).
> - **기본 모델(Flatten) 구현에는 이 상태기계가 결선돼 있지 않다.** `demo/widget-embed/flat/bootstrap-flat.js`
>   와 컴파일러가 생성하는 `ResumedApp` 엔트리는 "첫 클릭에 페이지 전체 World를 한 번 materialize하고
>   이후 그 인스턴스를 계속 쓴다"만 한다. 그 결과 promote-once-and-stay는 사실상 성립하지만, dormant/
>   attached 상태 추적, unmount/detach 경로, load/visible 트리거, 호스트발 제거 관찰자는 없다
>   (`bench/e2e/specs/m7-flat-host-interference.spec.ts` 머리 주석도 "H-Qwik에는 그런 생명주기 관찰자
>   자체가 없다"고 적는다). crash 처리는 dispatch가 던진 엘리먼트 하나에 `_loomFailed`를 세워 그 엘리먼트의
>   이후 상호작용을 거부하는 것뿐이다.
> - **구현되지 않은 것**: 컴포넌트 인스턴스마다의 범용 actor, actor가 `Future`를 await하고 Command로
>   Component를 갱신하는 비동기 경로, suspense/로딩 경계. crates/에서 `async`/`.await`는 `loom-bench`의
>   측정 보조 함수에만 나온다. 아래 "대응 관계" 표와 "Suspense/비동기 데이터 경계" 절은 설계다.

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
**(2026-10-17 보강: 이 단락은 설계다. 기본 모델 구현에서 `client:load`/`client:visible`은 서브트리
트리거로 결선돼 있지 않고, 페이지 전체가 첫 클릭 하나로 깨어난다 — 위 현황 블록, docs/04 2026-10-17
현황 확인 3.)**

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

**(2026-10-17 현황: 이 절은 구현되지 않았다.** 비동기 actor, `Loading` variant, 폴백 마크업 렌더 경로
모두 코드에 없다. Phase 1–4의 데모는 전부 동기 상호작용만 다룬다. 이 절을 구현하면 RQ3 "알려진 약점" 5
(비동기 정리 타임아웃 시 강제 drop 의미론 미정)가 실제 문제가 된다.)**

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
**(2026-10-17 보강: 이 정책의 지위는 여전히 RQ3 "잠정 규칙"이다 — 실측 검증 전의 설계 가설이고,
RQ3 상태 절 이후 이 규칙을 바꾸는 갱신이 없다(2026-10-17 부기·정정은 규칙 자체를 건드리지 않는다). 코드상으로는 Island 경로의 `IslandState`가 `Attached -> Dormant` 전이를
갖지 않고, 기본 모델의 `bootstrap-flat.js`는 materialize한 인스턴스를 버리지 않는다. 상주 승격의 비용
(활성 actor 단조 증가)은 RQ6 M4 N-스케일링에서 Island 팔의 구조적 약점으로 실측됐다.)**

## 미해결

- ~~중첩 Island에서 supervision 전파 규칙~~ → RQ3 잠정 규칙으로 결정됨(2026-07-23,
  Phase 3까지 반복 개선), **2026-07-28 RQ6 §7 게이트로 "세분화를 선택했을 때만
  적용되는 규칙"으로 지위 하락**(docs/04/RQ3 참고 — 기본 모델에서는 애초에 중첩
  경계가 없어 이 문제 자체가 발생하지 않음). 잔여 미해결은 RQ3의 "알려진 약점"
  참고 — 특히 wasm panic 봉쇄 방식(fault 격리의 전제, 세분화 선택 시에만 의미
  있음), 비동기 정리 타임아웃 시 강제 drop 의미론, 호스트발 DOM 제거 감지 경로.

**2026-10-17 정정 (위 잔여 미해결 목록)**:
- ~~호스트발 DOM 제거 감지 경로~~ → **Island 경로에서는 구현됐다**(2026-07-24, RQ6 §3.2): `registry.js`에
  MutationObserver 기반 감지를 넣었고, 순수 DOM 재배치를 detach로 오탐하던 결함은 `isConnected` 재확인으로
  고쳤으며(2026-07-25, 3개 브라우저 0/10 오탐), M10에서 `el.remove()` 직접 호출 시 `detached` 전이를 3개
  브라우저에서 확인했다(roadmap/PHASES.md Phase 3·Task #27 절). RQ3 "알려진 약점" 6의 "MutationObserver
  보강은 미설계" 문구는 이 사실보다 오래됐고, RQ3에도 같은 날(2026-10-17) 그 취지의 정정이 달렸다. **기본 모델(Flatten)에는 이런 관찰자가 없어** 호스트가
  anchor를 지워도 감지하지 않는다 — 기본 모델에서는 열린 문제다.
- wasm panic 봉쇄: 여전히 조건 A(미해결)다. 조건 C는 nightly + `-Zbuild-std`에서만 동작했고 stable 채널
  (Rust 1.96)에서는 막혀 있다(RQ3 "알려진 약점" 1의 2026-07-28 재검토). 조건 B(인스턴스 분리)는 Island
  경로에서만 실증됐다.
- 비동기 정리 타임아웃 시 강제 drop: 미정. 비동기 actor 자체가 구현되지 않아 아직 실제로 마주친 적이 없다.
- 새로 보탬 — **기본 모델의 Layer 1**: 6상태 기계와 2단계 detach를 Flatten 경로에도 결선할지, 결선한다면
  anchor 단위인지 페이지 단위인지 정해지지 않았다(위 현황 블록).
