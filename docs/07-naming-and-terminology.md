# 07. 네이밍/용어집

## 프레임워크 이름

**"Loom"으로 확정(2026-07-28, 병익님 결정).** 2-layer(actor + ECS)를 "베 짜기"처럼
엮는다는 은유에서 붙인 이름이 그대로 정식 이름이 됐다 — 더 이상 가칭이 아니다.
표기 규칙: 산문/문서에서 프레임워크 자체를 가리킬 때는 **Loom**(대문자 시작), 코드
식별자(크레이트명 `loom-*`, 함수/모듈명, `LOOM_*` 매크로·환경변수 접두어 등)는
기존 관례대로 소문자 `loom` 그대로 유지 — 이 저장소의 기존 크레이트/식별자를
전역 치환할 필요는 없다(가칭이 아닌 게 확정됐을 뿐, 표기 자체는 이미 이 관례를
따르고 있었으므로).

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

**2026-10-17 정정·보강 (위 표)**: 표를 코드와 대조한 결과 세 행이 현행과 다르다. 원래 정의는 그대로 두고
여기 적는다.
- **Signal\<T\>**: 실제 정의는 `pub struct Signal<T>(pub T)` — **값만** 가진 Component이고 dirty flag
  필드는 없다(`crates/loom-runtime/src/lib.rs`). "바뀌었는가"는 Bevy의 change-detection tick이 System별로
  추적하고, patch System은 `Changed<Signal<T>>` 쿼리로 그 변경만 받는다. 즉 dirty flag 역할은 Bevy가 맡는다.
- **FIC**: 표의 시그니처는 목표 계약이다. 실제 `crates/loom-fic`의 `render_html()`/`render_html_full()`은
  인자가 없다 — 컴포넌트 하나(`demo/counter-list.tsx`)에 고정, `props` 없음, `request_context` 비어 있음,
  `assets`는 항상 빈 배열(docs/05 2026-10-17 현황).
- **지연 activation 서브트리**: 정본 용어로서의 정의는 유효하지만, 기본 모델(Flatten) 구현에는
  `client:load`/`client:visible` 트리거가 결선돼 있지 않다. 지금 기본 모델에서 실제로 지연되는 단위는
  페이지 전체 하나이고, 첫 클릭의 전역 위임이 유일한 트리거다(docs/04 2026-10-17 현황 확인 3).
- **PatchTable/DomBinding**: 2026-10-17에도 미구현 그대로다. Phase 1의 "Phase 2에서 opcode로 전환" 계획은
  이행되지 않았고, Phase 2 resumability는 LRef + 생성된 정적 `dispatch(symbol, value)`로 구현됐다.
  **같은 날 병익님 결정으로 정본이 바뀌었다**: 정본 용어는 "정적 디스패치 = monomorphize된 제네릭 System + 닫힌
  열거형 `match`"이고, PatchTable/DomBinding은 **재개 조건부 대안**(런타임 데이터로 binding 종류가 정해지는 기능이
  명세될 때)이다. 새 코드에서 이 두 이름의 타입을 만들지 말 것(docs/02 "PatchTable 결정 (2026-10-17)").

### 추가 용어 (2026-10-17 등재)

이 문서 초판 이후 다른 문서와 코드에서 쓰이기 시작했는데 이 표에 없던 용어들이다.

| 이 프로젝트 용어 | 정의 | 출처 |
|---|---|---|
| **Layer 0 — Host Glue** | 어느 Island World에도 속하지 않는 상주 계층. 권위 있는 실물은 JS 쪽 `demo/resume/registry.js`(Island 6상태 추적, load/visible 트리거, 호스트발 제거 감지)와 `shared-store.js`(공유 값 저장소)다. Rust `crates/loom-runtime/src/island.rs`의 `NativeIslandRegistry`는 같은 상태기계의 네이티브 거울(테스트·증명 대응용, wasm 쪽 권위 레지스트리는 아님 — 모듈 헤더)이며 docs/04 원문은 이것도 Layer 0 실물로 든다. docs/01·06이 같은 6상태 기계를 "Layer 1 구현"으로 세는 것은 담당 관심사(supervision 생명주기) 기준이고, 여기의 Layer 0은 상주 위치 기준이다. 기본 모델에서는 `shared-store.js`와 전역 위임 `bootstrap*.js`만 쓰인다 | docs/04 "Layer 0" 절 (Phase 3 도입, 2026-07-28 지위 축소) |
| **`GraphMode::Flatten`** | 여러 컴포넌트를 하나의 merged World(`ResumedApp`, 청크 `App_chunk` 하나)로 컴파일하는 모드. **기본 모델** | `crates/loom-compiler` (Task #26), RQ6 H-Qwik 팔 |
| **`GraphMode::Islands`** | 컴포넌트마다 별도 Island 엔트리(컴포넌트별 생성자, 자기 호스트 스코프 안의 LRef에 직접 리스너)로 컴파일하는 모드. **선택적 최적화**("세분화를 선택했을 때") | `crates/loom-compiler` (Task #25), RQ6 Island 팔 |
| **복수 anchor root** | 하나의 World가 호스트 페이지의 여러 DOM 좌표에 투사되는 기본 모델의 임베드 방식. 지금은 `data-loom-id`/`data-loom-lref` 마커로 느슨하게 충족하고, 전용 `anchor_id`나 anchor별 FIC 조각 계약은 없다 | docs/05 "복수 마운트 지점" |
| **RQ1 후보 기호 (트랙 L)** | **K** = 대조군. §10 "K: 디스패치 규칙"대로 신·구 시퀀스를 O(n) 위치 대응 동등 스캔 → 완전 동등이면 no-op / 순수 append·truncate면 count-diff / 그 밖의 경우에만 전량 despawn+respawn(2026-09-22 e6bb644부터. 그 전 구현에는 마지막 분기만 있어 "허수아비 대조군" 지적을 받고 고쳐졌다 — `research-questions/RQ1-benchmark-protocol.md` §10) / **C** = 위치가 정체성, 값 비교 후 변경분만 write / **D** = `RowKey` + 컨트롤러 Vec / **E** = `OrderKey` + 순서 없는 멤버십 집합. **-LIS** 접미는 LIS 기반 최소 이동 변형이며 현행 판정 대상 D·E는 D-LIS·E-LIS를 가리킨다. **2026-10-30 주기**: G0R1(2026-10-20, 프로토콜 개정 #12) 이후 D·E의 행 key는 아이템 전체의 고정 해시 `u64`(`list_key::row_key`)이고, 행마다 값 채널 `RowValue<T>`를 들며(E도 포함), 매칭은 radix 병합(`list_key::match_by_key`, 옛 `match_keys`와 같은 반환 계약)이다. v22까지의 D·E 수치는 그 이전 변형(String key, Θ(n·m) 매칭)의 것이다 | `research-questions/RQ1-dynamic-control-flow-scheduling.md`, `crates/loom-runtime/src/list.rs` |
| **RQ1 후보 기호 (트랙 T)** | **B** = 조건이 꺼지면 despawn, 켜지면 respawn / **F** = 조건이 꺼져도 엔티티를 despawn하지 않는 계열 — F-i 값 필터(`CondEffective`) / F-ii 필터 없이 detached DOM에 계속 패치 / F-iii `Without<Frozen>` 마커. CSS 은닉(v-show형)과 lazy-spawn-then-hide는 F 변형이 아니라 별도 arm이다(RQ1-benchmark-protocol.md "구현 arm 6개"). 판정은 B 유지, F-i/ii/iii 기각(2026-08-07). 단 컴파일러가 실제로 내는 조건부 코드젠은 "첫 spawn 지연 + CSS 은닉"(`SetVisible`, 숨긴 동안 patch 게이팅 없음)으로, 정상 상태 토글은 CSS 은닉 arm과 같은 알고리즘이다(SSR-false 사이트의 첫 spawn 지연만 lazy arm의 앞 절반) — 프로토콜의 lazy-spawn-then-hide arm은 첫 spawn 뒤 F-i를 상속하므로 그것도 아니다. 코드 주석의 "candidate F"는 Phase 1(07-24) 이름표다 | RQ1 트랙 T 절 |
| **Chain** | `crates/loom-flush-core`의 flush 순서 재연결 자료구조. flush-core의 계획 함수(`plan_from_selection`·`plan_lis_from_selection_auto` → `plan_writes*`)가 내부에서 쓴다. `loom-runtime`은 2026-07-28부터 Chain을 직접 호출하지 않고 반환된 계획(PlanOp)을 `DomWrite`로 옮겨 적기만 한다(옛 `place_in_order`는 2026-08-11에 flush-core 안으로 흡수돼 사라졌다 — `crates/loom-runtime/src/list.rs` 314–320·395–401행 주석). Verus로 검증된 배포 코드다(LIS 쓰기 계획 포함 90 verified) | `crates/loom-flush-core/src/lib.rs`, RQ5 후보 #4 |

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
