# 04. Islands + Resumability — 긴장 관계와 해소안

**이 문서가 이 패키지에서 가장 중요함.** 나머지 문서는 대부분 기존 기법의 재구성이지만,
여기는 "왜 이 둘을 동시에 요구하는 게 자명하지 않은가"부터 시작해야 함.

## 발견한 긴장 관계

리서치 중 확인한 사실: Qwik 공식 문서는 명시적으로 다음을 주장함 —

> islands는 개발자가 수동으로 경계를 정의해야 하고, island끼리 통신할 수 없다는 한계가 있다.
> Qwik은 반응성 그래프에서 필요한 상태만 직렬화하는 방식으로 hydration 자체를 없앴기 때문에,
> **resumability는 partial hydration(=islands)의 trade-off 없이도 확장된다**고 본다.

즉 Qwik 입장에서 resumability는 islands의 **대체재**임 — 앱 전체가 이미 "O(1) JS 비용"이라면
굳이 수동으로 섬을 나눌 이유가 없다는 논리. Astro도 마찬가지로 island끼리 통신 불가를 명시된
한계로 인정함.

병익님 요구조건은 "동시에 지원"이므로, 이걸 단순 합집합으로 처리하면 안 되고 **둘이 서로 다른
문제를 푸는 것으로 역할을 재정의**해야 앞뒤가 맞음.

## 해소안: 두 개념의 관할을 분리

| | Island | Resumability |
|---|---|---|
| **관할 layer** | Layer 1 (구조/lifecycle) | Layer 2 (상태/직렬화) |
| **푸는 문제** | 임베딩·격리·장애 도메인 경계 — "client JS가 필요한 영역이 어디까지인가", "이 영역이 백엔드의 어느 템플릿 조각에 꽂히는가" | 효율성 — "그 영역 안에서 hydration 재실행 비용을 어떻게 O(interaction)으로 낮추는가" |
| **결정 시점** | 컴파일타임 (개발자가 명시적으로 경계 표시) | 런타임 (SSR 시 상태 직렬화, client는 상호작용 시점에만 실행) |
| **단위** | 1 Island = 1 supervision root = 1 IslandWorld(Layer 2 partition) | 1 LRef = 1 (모듈 URL, export 심볼, 캡처된 ComponentRef 집합) |

즉 **Island은 resumability의 대체재가 아니라 resumability가 작동하는 "격리 단위"**로 재정의함.
Qwik이 이 재정의를 안 한 이유는 Qwik에 애초에 "백엔드에 임베드되는 독립 단위"라는 요구조건이
없었기 때문 (Qwik은 자기 자신이 풀스택 프레임워크). 여기서는 05의 backend-agnostic 요구조건
때문에 Island이 여전히 필요함 — 레거시 Rails/Django/Spring 페이지 안에 "이 부분만 우리
프레임워크로 만든 위젯"을 꽂아야 하는 시나리오에서, 그 위젯의 경계가 곧 Island임.

**따라서 결론**: Island = 배포/임베딩/장애격리의 단위, Resumability = 그 단위 내부(및 잠재적으로
단위 간, RQ4 참고)의 실행 비용을 낮추는 메커니즘. 서로 직교하는 축이고 충돌하지 않음 — 단,
이건 이 대화에서 유도한 재해석이지 실측 검증된 결론이 아님. RQ6에서 다시 다룸.

## Resumability 메커니즘 설계 (Qwik의 QRL 원리를 재사용, 캡처 대상만 다름)

확인된 Qwik 메커니즘 (그대로 재사용):
- SSR 시점에 리스너 위치 정보를 수집해 HTML에 직렬화 (attribute에 청크 URL + 심볼 이름)
- 클라이언트는 **전역 리스너 하나**만 등록(delegation) — 엘리먼트마다 리스너를 안 닮
- 실제 상호작용이 일어날 때만 해당 청크를 lazy import, 심볼을 꺼내 실행
- 핸들러가 참조하던 클로저 캡처값은 별도 직렬화된 참조 배열에서 복원

**여기서 다르게 가져가는 부분**: Qwik은 임의의 JS 클로저를 캡처해야 해서 직렬화 메커니즘이
꽤 정교함(클래스 인스턴스, Date/URL/Map/Set 등 제한적 특수 케이스 지원, 그 외엔 client-only
탈출구 필요). 우리는 02의 설계 결정(Component는 항상 POD) 덕분에 **캡처 대상이 애초에
ECS Component뿐**임 — 즉 "이 핸들러가 필요로 하는 최소 Entity/Component 부분집합"을
스냅샷하면 되고, 이건 임의 JS 힙 객체 직렬화보다 훨씬 제약된(따라서 더 단순하고 예측 가능한)
문제임. 이게 정확히 왜 성립하는지, 얼마나 단순해지는지는 아직 실측 안 됨.

이름: Qwik의 QRL과 구분하기 위해 **LRef**(Lazy Reference)로 부름 (잠정, `07` 참고).
LRef = `{ chunk: string, symbol: string, captures: EntityComponentRef[] }`

## IslandWorld 직렬화

각 Island는 독립된 ECS World partition을 가지므로, resumability 페이로드는 Island 단위로
분리되어 직렬화됨 — Astro의 "각 island가 독립적으로 병렬 hydrate"라는 속성이 여기서도
그대로 유지됨(오히려 우리는 hydrate 자체가 없으므로 "독립적으로 병렬 resume"에 가까움).
Component가 POD이므로 이 스냅샷은 사실상 archetype 테이블의 부분 memcpy에 가까운 형태로
설계 가능 — 정확한 바이너리/JSON 포맷은 RQ2.

## 남는 문제: Island 간 통신 (Qwik/Astro가 공통으로 포기한 부분)

두 프레임워크 모두 "island끼리 통신 불가"를 명시된 한계로 인정함. 우리는 이걸 그대로 받아들이지
않고 완화 방안을 하나 제안: **공유 lazy signal store** — 여러 Island가 구독할 수 있는 최소한의
전역 Signal 집합을 두고, 구독도 LRef 방식으로 지연 등록. 단, 이건 "각 Island은 완전히 독립적으로
resume된다"는 속성을 부분적으로 깨뜨리므로 신중하게 범위를 제한해야 함. RQ4에서 계속.

## "Island = 완전 독립 World partition" 전제에 대한 수정 — Layer 0(Host Glue) (2026-07-24, Phase 3)

Phase 3에서 위 두 메커니즘(IslandRegistry, 공유 signal store)을 실제로 구현하며 확인된 사실:
Island World 어디에도 속하지 않는 **상시 상주 계층이 실제로 필요**함 — `IslandRegistry`도,
공유 store도 특정 Island의 `bevy_ecs::World`에 담을 수 없다(어느 Island가 소유하든 다른
Island의 wasm 인스턴스에서 도달 불가능해지므로). 이 계층을 **"Layer 0 — Host Glue"**로
명명함(`crates/loom-runtime/src/island.rs`, `demo/resume/registry.js`, `demo/resume/
shared-store.js`가 실물). "Island = 완전히 독립된 World partition"이라는 원래 전제는 부분
수정 필요 — Island World들은 서로 독립이되, 그 위에 이 얇은 중립 계층 하나가 상시 얹혀 있음.

**단, 이게 "하나의 큰 공유 World로 도로 붕괴"하는 건 아님**: Layer 0의 공개 표면은 IslandRegistry의
폐쇄된 스키마(topology 추적 + detach 발행 전용, RQ3 참고)와 공유 store의 작은 get/set/notify
API뿐 — `&mut World`를 통째로 넘기거나 임의 Component를 공유하는 것과 질적으로 다름.

**위 "구독도 LRef 방식으로 지연 등록"은 실제 구현에서 채택하지 않음(정정)**: 공유 store 자체는
항상 로드되는 작은 JS(`shared-store.js`)로 구현하고, 구독자는 `data-loom-shared-subscriber`
DOM 마커만으로 표시함 — 순수 표시용 구독에 청크 로드를 요구하는 건 이득 없이 비용만 추가하기
때문(코드젠이 SSR 시점에 정적으로 구독자를 마킹하는 게 LRef 지연보다 **더** 게으름, 코드 자체가
전혀 안 실림). 예: CartBadge는 QuantityPicker의 수량이 바뀌어도 자기 wasm 청크를 전혀 로드하지
않음 — `demo/islands/index.html` + Playwright 실측(roadmap/PHASES.md Phase 3 절 참고)으로 확인.
