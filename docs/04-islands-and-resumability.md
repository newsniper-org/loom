# 04. Resumability와 지연 Activation — 폐기된 "Island=격리 단위" 재해석의 후신

**2026-07-28 (Phase 6, Task #29) — 이 문서는 전면 재작성됨.** RQ6 §7 게이트를
widget-embed 정체성 통제 픽스처의 실측 데이터에 적용한 결과, 이 문서 원본이
제안했던 "Island = resumability가 작동하는 격리 단위"라는 재해석은 검증에서
살아남지 못했다 — **H-Qwik 확정(강한 형태)**. 상세 게이트 적용과 정직한 한계는
`research-questions/RQ6-verification-plan.md`의 "§3.9 실행" 절과
`research-questions/RQ6-are-islands-necessary.md`의 "상태" 절 참고.

이 문서는 그 결론이 나온 **이유와 이력을 지우지 않는다**(CLAUDE.md 정직성 요구) —
아래 "폐기된 이전 설계"에 원본을 그대로 남기고, 그 위에 실측이 실제로 확인한
새 모델을 적는다.

## 실측이 확인한 것: Island 경계 없이도 요구가 충족된다

원본이 세웠던 긴장 관계 — "Qwik은 resumability 하나로 충분하다고 주장하지만,
05(backend-agnostic)의 '레거시 페이지에 위젯 하나만 임베드' 요구 때문에 Island
경계가 필요하다" — 는 실측으로 직접 검증 가능한 명제였다. widget-embed
픽스처(CartBadge/QuantityPicker/ProductCard/CheckoutForm/MiniDashboard 5개
컴포넌트, 같은 소스를 `GraphMode::Islands`/`GraphMode::Flatten` 양쪽으로 컴파일)
로 두 팔을 실제로 대결시킨 결과:

- **정합성(M8)**: 두 팔 모두 통과 — Island 경계 없이도(Flatten 모드) 같은 참조
  콘텐츠·같은 상호작용 결과를 낸다.
- **격리(M6, 오늘 실제 채택된 조건 A)**: 두 팔 사이에 격리 차이가 없다 — 이건
  Island 유무의 문제가 아니라 RQ3가 아직 못 정한 panic 봉쇄 전략(조건 A/B/C)의
  선택에 달려 있다는 §5.4의 사전 분석이 그대로 확인됨. 조건 B(인스턴스 완전
  분리)는 Island 경계가 있어야만 가능하지만, 조건 C(catch_unwind, 경계 비의존)가
  아직 stable 채널로 완전히 검증되지 않아 "Island이 있어야만 격리가 된다"는
  주장을 뒷받침할 근거가 갖춰지지 않았다.
- **호스트 간섭(M7)**: 두 팔 모두 등가.
- **비용(M1)**: **H-Qwik이 명백히 더 가볍다** — 완전 관여 시 약 3.1배, 무상호작용
  시(client:load 위젯들이 즉시 로드되는 Island의 특성상) 약 182배. Island의
  "위젯별 분리"가 치르는 진짜 비용은 원래 예상(격리 이득의 대가)이 아니라
  **공유 런타임(bevy_ecs/wasm-bindgen 글루)의 중복** — 위젯 자신의 로직은 몇
  KB에 불과하고, 나머지 수백 KB는 청크마다 중복되는 고정비다.

## 재해석: "Island"가 아니라 "지연 Activation의 세분화 여부"

원본이 Island을 "resumability가 작동하는 격리 단위"로 정의했던 것은 틀린
전제 위에 있었다 — resumability 자체가 이미 "언제 client 코드가 깨어나는가"를
결정하는 매커니즘이고, 실측 결과 그 위에 **별도의 컴파일타임 격리 단위를
얹지 않아도** 05의 임베딩 요구가 충족된다. 새 모델:

- **기본 모델(Qwik과 동형)**: 페이지 전체가 하나의 resumable 단위(하나의
  `bevy_ecs::World`, 하나의 LRef 주소 공간 — `App_chunk`). SSR은 이 하나의
  단위를 렌더링하고, client는 **전역 리스너 하나**(`bootstrap.js`)만 등록한다.
  첫 상호작용이 어디서 일어나든 그 하나의 청크가 materialize되고, 이후 모든
  위젯이 그 하나의 인스턴스를 공유한다 — CLAUDE.md 2-1(이미 잘 풀린 문제
  재사용)이 요구하는 그대로, Qwik의 원래 메커니즘을 그대로 가져온 것이다.
- **"세분화가 필요한가"는 사용자 판단으로 남는 선택지, 필수 아님**: 컴파일러가
  이미 갖춘 `GraphMode::Islands`(컴포넌트별 별도 wasm 산출물)는 폐기되지
  않는다 — 다만 "이게 없으면 임베딩이 안 된다"는 주장이 아니라, "M4(상주
  메모리) N-스케일링처럼 특정 워크로드에서 세분화가 실제로 이득이 되는
  경우의 **선택적 최적화**"로 지위가 낮아진다. 언제 그 선택이 실제로 이득인지는
  아직 프로파일링되지 않았다(정직한 한계 — 이번 판정이 답한 건 "기본으로
  필요한가"이지 "절대 쓸 데가 없는가"가 아니다).
- **"부분 트리 단위 지연 activation"**: RQ6-are-islands-necessary.md가 이미
  예견했던 이름 그대로. `client:load`/`client:visible` 지시어 자체는 유지된다
  (§9-6/§6 M8의 요구를 충족하려면 여전히 "이 서브트리는 언제 깨어나는가"를
  표시할 수단이 필요하므로) — 다만 그 지시어가 "별도 격리된 World로 분리한다"는
  뜻이 아니라 "이 서브트리에 로드/가시성 트리거를 건다"는 뜻으로 재정의된다.

## LRef 메커니즘 — 무변경

02의 설계 결정(Component는 항상 POD)에 기반한 LRef 캡처 메커니즘(`{chunk,
symbol, captures}`, RQ2 참고)은 이 재해석과 무관하게 그대로 유지된다 — Qwik의
QRL 원리를 재사용한다는 원본의 설계는 검증 대상이 아니었고, 실측(M3, wake-up
payload)도 두 팔에서 완전히 동일한 캡처 세그먼트를 확인했다.

## Layer 0 — Host Glue의 지위 변화

원본이 Phase 3에서 도입한 "Layer 0 — Host Glue"(IslandRegistry, 공유 signal
store)는 **완전히 폐기되지 않는다** — `shared-store.js`(공유 값 저장소, RQ4가
다뤘던 문제) 자체는 이제 "여러 개의 격리된 World 사이의 통신"이 아니라 그냥
"하나의 World 안에서 값이 하나 더 있다"는 정도로 단순해지지만, `registry.js`의
2단계 detach/dormant 상태 추적은 **"세분화를 선택했을 때"** 여전히 필요한
메커니즘으로 남는다 — RQ3의 부분 폐기 절 참고.

---

## 폐기된 이전 설계 (2026-07-24 작성, 2026-07-28 폐기 — 이력 보존)

> 아래는 이 문서의 원본이다. RQ6 §7 게이트가 실측으로 반박했으므로 폐기하지만,
> "왜 처음에 이렇게 생각했는가"를 지우지 않는다(CLAUDE.md 정직성 요구).

### 발견한 긴장 관계 (원본)

리서치 중 확인한 사실: Qwik 공식 문서는 명시적으로 다음을 주장함 —

> islands는 개발자가 수동으로 경계를 정의해야 하고, island끼리 통신할 수 없다는 한계가 있다.
> Qwik은 반응성 그래프에서 필요한 상태만 직렬화하는 방식으로 hydration 자체를 없앴기 때문에,
> **resumability는 partial hydration(=islands)의 trade-off 없이도 확장된다**고 본다.

즉 Qwik 입장에서 resumability는 islands의 **대체재**임 — 앱 전체가 이미 "O(1) JS 비용"이라면
굳이 수동으로 섬을 나눌 이유가 없다는 논리. Astro도 마찬가지로 island끼리 통신 불가를 명시된
한계로 인정함.

병익님 요구조건은 "동시에 지원"이므로, 이걸 단순 합집합으로 처리하면 안 되고 **둘이 서로 다른
문제를 푸는 것으로 역할을 재정의**해야 앞뒤가 맞음 — **이 결론(재정의가 필요하다는 전제)이
정확히 실측으로 반박된 지점이다. 동시 지원 요구 자체는 유효했지만, "격리 단위로 역할을
나눠야 한다"가 그 요구를 만족하는 유일한 방법은 아니었다.**

### 해소안: 두 개념의 관할을 분리 (원본, 폐기됨)

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
**(2026-07-28: RQ6에서 실제로 다뤘고, 반박됐다.)**

### Resumability 메커니즘 설계 (원본 — 이 부분은 폐기되지 않고 위 "무변경" 절로 승격됨)

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
문제임.

이름: Qwik의 QRL과 구분하기 위해 **LRef**(Lazy Reference)로 부름. LRef = `{ chunk: string,
symbol: string, captures: EntityComponentRef[] }`

### IslandWorld 직렬화 (원본, 폐기됨 — "IslandWorld" 개념 자체가 없어짐)

각 Island는 독립된 ECS World partition을 가지므로, resumability 페이로드는 Island 단위로
분리되어 직렬화됨 — Astro의 "각 island가 독립적으로 병렬 hydrate"라는 속성이 여기서도
그대로 유지됨. **(2026-07-28: 기본 모델에서는 World가 하나뿐이므로 이 분리 자체가 없다 —
"세분화 선택 시"에만 의미를 갖는 부차적 메커니즘으로 지위 하락.)**

### 남는 문제: Island 간 통신 (원본, 폐기됨 — RQ4가 전면 폐기됨에 따라 이 문제 자체가 소거)

두 프레임워크 모두 "island끼리 통신 불가"를 명시된 한계로 인정함. 우리는 이걸 그대로 받아들이지
않고 완화 방안을 하나 제안: **공유 lazy signal store**...(이하 원본 생략, RQ4-cross-island-
shared-state.md의 폐기 기록 참고 — 경계가 없으면 "경계 간 통신"이라는 문제 자체가 없어진다).

### "Island = 완전 독립 World partition" 전제에 대한 수정 — Layer 0(Host Glue) (원본, 2026-07-24)

Phase 3에서 위 두 메커니즘(IslandRegistry, 공유 signal store)을 실제로 구현하며 확인된 사실:
Island World 어디에도 속하지 않는 **상시 상주 계층이 실제로 필요**함 — `IslandRegistry`도,
공유 store도 특정 Island의 `bevy_ecs::World`에 담을 수 없다(어느 Island가 소유하든 다른
Island의 wasm 인스턴스에서 도달 불가능해지므로). 이 계층을 **"Layer 0 — Host Glue"**로
명명함(`crates/loom-runtime/src/island.rs`, `demo/resume/registry.js`, `demo/resume/
shared-store.js`가 실물). **(2026-07-28: 이 관찰 자체는 유효하다 — "세분화를 선택했을 때"
여전히 이 계층이 필요하다는 사실은 안 바뀐다. 위 "Layer 0의 지위 변화" 절 참고.)**
