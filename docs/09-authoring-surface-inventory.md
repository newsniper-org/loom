# Loom 저작 표면 격차 인벤토리

- **작성일:** 2026-10-17
- **상태:** 읽기 전용 감사 결과입니다. 이 문서는 코드를 바꾸지 않습니다.
- **위치:** 이 문서(`docs/09-authoring-surface-inventory.md`). 완성 단계(Phase 7) 계획은 병익님 결정 대기라 여기 넣지 않는다.

## 0. 범위

이 문서가 답하는 질문은 하나입니다. **TSX 저작자가 쓸 법한 구문 가운데 지금 loomc가 지원하는 것은 무엇이고, 거부하는 것은 무엇이며, 조용히 버리거나 잘못 컴파일하는 것은 무엇인가.**

**대상**
- `crates/loom-compiler`: `lib.rs`, `lower.rs`(TSX→IR), `ir.rs`, `codegen.rs`(IR→Rust)
- `crates/loom-runtime`: `lib.rs`, `web.rs`, `island.rs`
- JS 진입점: `demo/resume/bootstrap.js`, `demo/widget-embed/flat/bootstrap-flat.js`, `registry.js`, `shared-store.js`
- `loom-fic`(StringDom, FIC ABI, sidecar)

**대상이 아닌 것**
- 성능
- RQ1·RQ6 판정의 옳고 그름
- 공식 검증 산출물

**상태 범례**

| 상태 | 뜻 |
|---|---|
| 지원 | 그 형태 그대로 정상 동작합니다. |
| 부분 | 일부 형태만 지원합니다. 나머지는 거부되거나 조용히 처리됩니다. |
| 거부(lower) / 거부(codegen) | loomc가 `panic!`/`assert!`/`unreachable!`로 멈춥니다. |
| 거부(rustc) | loomc는 통과합니다. 산출물을 쓰는 하위 크레이트를 빌드할 때 실패합니다. |
| 거부(실행) | 빌드는 통과합니다. 생성된 코드가 실행 중에 패닉합니다. |
| 부재 | 처리 경로 자체가 없습니다. 조용히 무시되는지는 칸에 따로 적었습니다. |
| **조용** | 패닉 없이 통과하면서 의미가 바뀌거나 무언가가 사라집니다. §1 위험 표의 항목입니다. |

**영향 열의 약어**

| 약어 | 뜻 |
|---|---|
| **L§2.5** | 트랙 L 공유 기반 동결이 깨집니다(RQ1-benchmark-protocol.md:78-81). `loom-bench`는 `loom-runtime`에만 의존합니다(crates/loom-bench/Cargo.toml). 그래서 **런타임을 고칠 때만** 해당합니다. |
| **N-G2** | 트랙 N G2를 재인증해야 합니다. 리스트·조건부·row 템플릿 codegen을 고쳐 `loom-track-n`/`loom-nested-fixture` 산출물이 바뀔 때 해당합니다. 직전 재인증은 8204f61입니다. |
| **RQ6** | RQ6 위젯-임베드 / N-스케일링 / 대규모 앱 축 픽스처의 산출물(HTML, wasm, JS)이 바뀌어 측정을 다시 재현해야 합니다. |
| **조건부B** | 조건부를 "첫 spawn 지연 + CSS 은닉" 경로로 둔 것과 그 B 전환 결정에 관한 사안입니다(PHASES.md "아직 결정 기록이 없는 것"). |
| **PT** | PatchTable 결정(2026-10-17)입니다. 제네릭 System과 닫힌 열거형이 정본입니다(docs/02:145-149). 재개 조건은 docs/02:156-158입니다. |
| **G0** | docs/03 "No-VDOM 판별 기준" 조문입니다. 2026-10-17 첫 실행에서 D·E가 P1·P3로 기각됐습니다. |
| **CAP** | LRef 최소 캡처의 안전 전제입니다(docs/02:189-205). 조건 (1) "액션 하나는 시그널 하나만 참조"가 :197-198, 조건 (2) "캡처에서 빠진 시그널은 리터럴 초기값으로 폴백해도 안전"이 :199-205입니다. |
| — | 연구 산출물에 영향이 없습니다. |

**컴파일러 산출물을 쓰는 크레이트**
- `build.rs`로 직접 쓰는 크레이트
  - `loom-track-n`
  - `loom-nested-fixture`(트랙 N G2)
  - `loom-widget-embed`
  - `loom-widget-embed-island-{checkout-form, mini-dashboard, product-card, quantity-picker}`
  - `loom-largeapp-axis-{flat, island-target}`
  - `loom-fic`
  - `loom-demo`
  - `loom-resume-demo`
  - `loom-island-compiled`
- 간접 소비: `loom-rq6-fixture-gen`. `build.rs`는 없지만 `loom-widget-embed`에 의존합니다(crates/loom-rq6-fixture-gen/Cargo.toml:14).
- 직접 입력 TSX 7개
  - `demo/counter-list.tsx`
  - `demo/islands/product-card.tsx`
  - `demo/nested-list-derived.tsx`
  - `demo/widget-embed.tsx`
  - `nested-conditional.tsx`
  - `nested-list-derived.tsx`
  - `nested-list.tsx`

## 0.1 방법

1. **인벤토리 네 갈래를 병합했습니다.** 갈래는 lower, codegen, runtime, target(JS·FIC)입니다. lower·codegen 갈래는 프로브 약 110건으로 loomc 산출물을 직접 확인했습니다.
2. **적대적 검증을 했습니다.** 측정 시점의 load average가 27.66이라 빌드는 하지 않았습니다. 대신 코드를 정독하고 lower.rs 로직에 TSX 조각을 손으로 대입했습니다. 약 50행을 샘플링해 정정 16건을 반영했습니다.
3. **완전성 비평을 했습니다.** single·islands·flatten 세 모드로 프로브 42건을 돌려 누락 행을 추가했습니다(R40–R46 등).

각 행의 **확인 수준**은 이렇게 구분합니다.
- **실측**: loomc 산출물이나 테스트로 확인했습니다.
- **코드**: 정독으로 경로를 끝까지 따라가 확정했습니다.
- **추론**: 브라우저·rustc로 실행하지 않은 그럴듯한 판단입니다.

## 0.2 한계

- **브라우저에서 재현한 행이 없습니다.** R13, R14, R29, R38, R43, R44(Island 쪽)의 브라우저 동작은 코드에서 추론했습니다.
- **추론에 머문 행이 있습니다.**
  - R10(조건부 안 단순 리스트 갱신): 1174와 3191의 상호작용을 끝까지 추적하지 못했습니다.
  - R11(값 오염)
  - R37(StringDom)
  - 단일 Island stub의 E0428
  - "같은 이름 signal 중복은 rustc에서 실패": R23과 얽혀 SWC 복구 에러 여부가 미확인입니다.
  - §11의 loom-fic 줄 번호: 459-463만 확인했습니다.
  - RQ1:159 / :1996 인용은 대조하지 않았습니다.
- **줄 번호는 2026-10-17 HEAD(aac2dc2) 기준입니다.** 코드가 바뀌면 줄 번호도 어긋납니다.
- **"필요도"는 판단입니다.** 기준은 "위젯 하나를 실제로 임베드할 때 피해 갈 수 있는가"입니다. 측정값이 아닙니다.
- **XSS 경로는 현재 표면에서 찾지 못했습니다(§2.3 참고).** 이는 props 같은 런타임 값이 정적 HTML 경로로 들어가지 않기 때문입니다. R1을 고친 뒤 다시 확인해야 합니다.

## 0.3 거부가 드러나는 방식(공통)

- **진단 체계가 없습니다.**
  - lower.rs에서 `panic!`/`assert!`/`assert_eq!`/`unreachable!`이 있는 줄은 **121개**입니다(grep -cE). lower.rs에는 테스트가 없으므로 전부 실제 거부 지점입니다.
  - codegen에도 별도의 패닉이 있습니다.
  - span·줄·열 정보가 없고, 메시지에 SWC AST의 `{:?}` 덤프가 섞입니다.
- **사용자 오류가 내부 오류처럼 보입니다.**
  - `unreachable!`: lower.rs:812-814
  - "never spawned": codegen.rs:1271
  - `assert_eq!`: lower.rs:489-492
- **실패가 뒤로 미뤄집니다.** 일부는 rustc에서, 일부는 wasm 실행 중에 드러납니다.
- **복구 가능한 파스 에러를 버립니다.** `take_errors()`를 호출하지 않습니다(lib.rs:20-24, R23).

---

## 1. 위험 표: 조용한 오컴파일·누락 (패닉 없이 통과)

**등급 기준**
- **S1**: 의미가 바뀌거나 사용자 데이터가 사라지고, 흔한 저작 패턴에서 발생합니다.
- **S2**: 의미가 바뀌지만 덜 흔한 패턴에서 발생합니다.
- **S3**: 경계 사례이거나 진단 품질의 문제입니다.

| # | 등급 | 구문 / 상황 | 결과 | 근거 | 필요도 | 막히는 레이어 | 고치면 영향 | 확인 |
|---|---|---|---|---|---|---|---|---|
| R1 | S1 | 컴포넌트 props 전달 `<W client:load initial={5}/>`, 매개변수 `function W(props)` | 값이 증발하고 선언 초기값이 쓰입니다. "패닉하지 않음"을 고정하는 테스트가 있습니다 | lower.rs:154-196(params를 읽지 않음), :403-409; codegen.rs:544, :3400(`..`); lib.rs:989-1005. `specialize_component_refs`는 ir.rs:325 주석에만 있습니다 | 필수 | lower(params)·IR·codegen | **CAP 조건 (2)가 깨집니다**(docs/02:199-205, codegen.rs:1306-1323). RQ로 올려야 합니다. RQ6(픽스처는 props 없이 구성) | 실측 |
| R2 | S1 | ComponentRef children `<W client:load><p>…</p></W>` | 버려집니다. SSR HTML에서도 사라집니다 | lower.rs:378-422(`el.children`을 읽지 않음) | 필수 | lower·IR(슬롯 없음) | RQ6 | 코드 |
| R3 | S1 | `c.increment(5)` | 인자를 무시하고 +1입니다. `increment()`와 산출물이 바이트 단위로 같습니다 | lower.rs:660 | 중요 | lower·IR(`ClickAction`에 인자 없음) | PT(열거형 확장) | 실측 |
| R4 | S1 | `o.toggle(true)` | 인자를 무시하고 반전합니다 | lower.rs:690-696 | 중요 | lower | PT | 코드 |
| R5 | S2 | `onClick` 두 번 | 앞 핸들러가 사라지고 single-writer 검사도 우회됩니다 | lower.rs:353 | 중요 | lower | — | 코드 |
| R6 | S3 | `class`/`className` 중복, input 속성 중복 | 뒤의 것이 이깁니다 | lower.rs:350, :477-478 | 선택 | lower | — | 코드 |
| R7 | S1 | 같은 signal을 binding 사이트 두 곳에 사용(`{n()}` 두 번, `{open()&&…}` 두 번) | 첫 사이트가 고아가 되어 갱신되지 않습니다. 두 번째 `let n_entity`가 첫 번째를 섀도잉하고, `Signals` 리터럴은 Rust 스코프상 마지막 엔티티를 가리킵니다. `data-loom-id`가 중복되어 last-wins입니다 | codegen.rs:2800-2807, 2969, 3128, 3153, 3303; Signals 1264-1273; resume 1565-1579; 400-401, 726 | 필수 | codegen(사이트별 entity) | N-G2(row source 공유 경로 2983-3009), RQ6 | 코드 |
| R8 | S1 | 리스트 row 루트의 `onClick` `<li onClick={…}>{x}</li>`. 조건부 안 리스트의 row 루트 포함 | 액션이 사라져 `enum Action {}`이 됩니다. 그런데 single-writer 집계(`action_signal_names`가 `ListBinding`까지 내려감)에는 포함됩니다 | codegen.rs:1768-1794(`collect_actions`가 `_ => {}`로 List/Cond를 내려가지 않음, 1789), 단순 경로 2810, 복잡 경로 `collect_row_mirrors` 2260-2296, 조건부 안 리스트 2008(`Element { children, .. }`); ir.rs:262 | 필수(행 삭제 패턴) | codegen | N-G2, PT | 코드 |
| R9 | S2 | row 태그·class `<span class="c">{x}</span>`, `<li class="c">{x}</li>` | 단순 경로에서는 `li`로 고정되고 class가 버려집니다. 판정 기준이 `has_rich_row_template`(2232-2244)라서 class만 단 `<li>`도 단순 경로로 갑니다. 복잡 경로도 class를 버립니다 | codegen.rs:2810(`row_tag`를 방출에 쓰지 않음), 2828-2846, 2868, 2107, 2536, 2946; runtime lib.rs:735-736(`tag: "li"`), 800, 1009 | 중요 | codegen·runtime(`SpawnRow`에 class 없음) | **L§2.5**(`SpawnRow`/`RowSpawner`), N-G2 | 코드 |
| R10 | S2 | 복잡 row가 있는 컴포넌트 + 조건부 안의 단순 리스트 | 첫 오픈 뒤 append/rotate가 DOM에 반영되지 않는 것으로 보입니다 | codegen.rs:1174, 1433, 3135-3146, 3191 | 중요 | codegen | N-G2, 조건부B | 추론 |
| R11 | S2 | 복잡 row 리스트의 resume | `RowSite` 마커가 빠져 다시 재조정되지 않습니다. `adopt_children`이 정적·중첩 텍스트를 아이템 값에 섞습니다 | codegen.rs:1581-1647; runtime lib.rs:1054; web.rs:461-472 | 중요 | codegen·web.rs | N-G2, L§2.5(web.rs) | 추론 |
| R12 | S1 | 조건부의 then 자식이 **binding 하나뿐**인 경우. `{c() && <p>{n()}</p>}`와 **`{c() && <ul>{xs().map(…)}</ul>}`**가 해당합니다. SSR 시점에 열려 있는 상태 | 직계 단일 binding 경로는 안쪽 signal에 `data-loom-id`를 달지 않습니다. 다는 곳은 wrapper의 `cond` id(3079)뿐입니다. 그래서 resume의 `find_node`가 **실행 중 패닉**합니다(I32는 1508, 리스트는 1604). 반면 `emit_lazy_subtree` 경로는 `loom_id: Some(..)`를 답니다(2190, 2198). loomc는 통과하므로 이 표에 둡니다 | codegen.rs:3079, 3173-3195, 1504-1508, 1601-1604, 410, 732 | 중요 | codegen | 조건부B, N-G2 | 코드 |
| R13 | S1 | `bootstrap.js`에서 lref 엘리먼트마다 따로 생기는 인스턴스 | lref 엘리먼트마다 World가 생겨 상태가 갈라집니다(append/rotate 시나리오). 에러는 없습니다. `bootstrap-flat.js:53-56`은 같은 문제를 고쳤습니다 | bootstrap.js:88-90(`_loomInstance`), 110; codegen.rs:331-433; demo/resume/index.html:15 | 필수 | bootstrap.js | — (resume 데모 한정) | 추론 |
| R14 | S1 | `bootstrap.js`에서 `<input>`을 클릭하면 값이 지워짐 | click 위임이 input 자신도 잡아 `SetString("")`이 됩니다 | bootstrap.js:117-123; codegen.rs:444, 1126-1131; lref에 이벤트 종류 없음(bootstrap.js:82) | 필수 | bootstrap.js·codegen | — | 추론 |
| R15 | S1 | 단일 Island 클라이언트 안의 `<input>` | 거부되지 않습니다. click 리스너가 붙고 값이 `""`로 고정됩니다(775). 그래서 클릭하면 지워집니다. 다중 Islands 검사(630-638)가 단일 경로(840-871)에는 없습니다 | codegen.rs:630-638, 775, 840-871 | 중요 | codegen | RQ6(Island arm) | 코드 |
| R16 | S2 | Islands에서 같은 컴포넌트를 두 번, 디렉티브가 다름 | **트리 순회에서 처음 만난** 디렉티브가 둘 다에 적용됩니다(`find_map`). 첫 번째가 visible이면 둘 다 visible입니다 | ir.rs:322-337; codegen.rs:218-223, 552 | 중요 | IR | RQ6(Island arm) | 코드 |
| R17 | S3 | Flatten의 `client:load`/`client:visible` | 효과 없이 수용됩니다. H-Qwik 설계상 의도지만 컴파일 시점 신호가 없습니다 | codegen.rs:544; docs/06:49-51 | 선택(경고) | 진단 | RQ6(기본 모델 정의) | 코드 |
| R18 | S3 | `client:load={false}`, `client:load client:visible` 동시 사용 | 값을 무시하거나 뒤의 것이 이깁니다 | lower.rs:395 | 선택 | lower | — | 코드 |
| R19 | S2 | 숫자 리터럴 `signal(1.7)`, `signal(3e10)`, sharedSignal/sharedValue/prop | 절단(1)되거나 i32로 포화(2147483647)됩니다 | lower.rs:275, :239, :755, :435 | 중요 | lower | — | 코드 |
| R20 | S1 | JSX 텍스트 공백 `hello <b>x</b> world` | `trim()`으로 단어 사이 공백이 사라집니다. 실증: `cart: {…}`가 `<span>cart:<span data-loom-shared-subscriber=…>`로 나옵니다 | lower.rs:709; widget-embed.tsx:85 → demo/widget-embed/flat/index.html:20 | 필수 | lower | **RQ6**(픽스처 HTML 바이트), N-G2(텍스트를 쓰는 픽스처) | 실측 |
| R21 | S2 | 여러 줄 JSX 텍스트 | JSX 줄 접기 규칙을 따르지 않아 textContent가 달라집니다 | lower.rs:709 | 중요 | lower | R20과 같음 | 코드 |
| R22 | S3 | `return`이 여러 개 | 도달 불가능한 마지막 return이 루트가 됩니다 | lower.rs:184 | 선택 | lower | — | 코드 |
| R23 | S2 | SWC 복구 가능 파스 에러(`signal(010)`, `with`, 생성자 중복) | 버려집니다. `010`은 8로 수용됩니다 | lib.rs:20-24(`take_errors()`를 호출하지 않음) | 중요 | lib.rs | — | 코드 |
| R24 | S2 | 소문자 이름 헬퍼 `function helper()` + `<helper/>` | HTML 태그로 취급되고 함수 본문은 무시됩니다 | lower.rs:323 | 중요 | lower | — | 코드 |
| R25 | S2 | 최상위 import·type·const·class. 다른 export가 있을 때의 `export const`/`export default` | 조용히 무시됩니다. import 없이 `sharedValue`를 써도 통과합니다. 실례: widget-embed.tsx:30의 import에 `sharedValue`가 없는데 :85에서 씁니다 | lower.rs:147 | 중요 | lower | — | 코드 |
| R26 | S3 | `<input>` children, 초과 인자(`.set(e.target.value, 1)`, `signal(4, 9)`, `append("a","b")`, `.map(fn, thisArg)`) | 드롭되거나 무시됩니다 | lower.rs:466-494, :569, :271, :679, :914 | 선택 | lower | — | 코드 |
| R27 | S3 | 함수 매개변수, `async` 함수, generator, 반환 타입, **signal 초기화식인** `let`/`var` | 검사하지 않습니다. signal 초기화식인 `let`/`var`는 `const`처럼 취급합니다. 그 밖의 초기화식은 거부합니다(§2.2) | lower.rs:154-196, :167 | 선택 | lower | — | 코드 |
| R28 | S2 | `item.length` 단위 | `chars().count()`를 써서 JS의 UTF-16 길이와 비-BMP 문자에서 어긋납니다 | codegen.rs:2408, 2420, 2905, 2917 | 중요 | codegen | **N-G2**(`LenGt`/`PrefixByItemLen`) | 코드 |
| R29 | S2 | void 태그 SSR `<br/>`, `<img/>` | `<br></br>`로 나가 브라우저에서 줄바꿈이 2개가 될 것으로 봅니다 | codegen.rs:1930-1932; loom-fic lib.rs:186-188 | 중요 | codegen·StringDom 4벌 | RQ6(정적 HTML) | 추론 |
| R30 | S2 | `<script>…</script>`, `<form>` 안의 `<button onClick>` | 그대로 출력됩니다. 기본 제출로 페이지가 이동합니다(preventDefault 경로 없음) | lower.rs:318-321; runtime·bootstrap에서 `preventDefault` grep 0건 | 중요 | lower·bootstrap | — | 코드 |
| R31 | S2 | resume 시 I32 캡처 실패, Island 진입점의 캡처 | `unwrap_or(initial)`로 선언 초기값이 들어갑니다. Island는 캡처를 항상 `&[]`로 넘깁니다 | codegen.rs:1494, 740, 749 | 중요 | codegen | RQ6(Island arm), CAP | 코드 |
| R32 | S2 | row source / row 안 조건부 / 숨겨진 채 SSR된 리스트의 resume | 선언 초기값으로 되돌아갑니다(문서화된 한계) | codegen.rs:1463-1488, 189-190, 1592-1598, 1638-1644 | 중요 | codegen | N-G2, 조건부B | 코드 |
| R33 | S2 | Flatten 진입점의 전역 `data-loom-id` 조회 | 중복 id가 last-wins입니다(R7과 결합) | codegen.rs:376-402 | 중요 | codegen | RQ6 | 코드 |
| R34 | S2 | `tick`의 부모·엔티티 해석 실패 | `dropped_writes`만 늘어납니다. `SpawnRow`/`Reposition`의 `before` 앵커 실패는 None이면 append라 집계조차 안 되고 순서가 어긋납니다 | runtime lib.rs:1422-1425, 1446-1448, 1456-1461, 1470-1484 | 중요 | runtime | **L§2.5** | 코드 |
| R35 | S2 | `adopt_children`이 공백·주석 노드까지 입양 | 호스트가 HTML을 재포맷하면 빈 문자열 행이 생깁니다 | web.rs:461-472 | 중요 | web.rs | L§2.5, N-G2(resume) | 코드 |
| R36 | S3 | `set_visible`이 인라인 `display`만 바꿈 | 스타일시트로 숨긴 요소는 다시 보이게 할 수 없습니다 | web.rs:398-403 | 선택 | web.rs | L§2.5, 조건부B/CSS 은닉 arm | 코드 |
| R37 | S3 | StringDom의 `set_attribute` 중복, `set_visible` 미반영 | 잠복 중입니다 | loom-fic lib.rs:234-236, 156-189 | 선택 | StringDom | RQ6(SSR) | 추론 |
| R38 | **S1** | **Flatten(기본 모델) 경로의 `<input>`** | **결함 둘을 코드로 확정했습니다(브라우저 미재현).** (a) bootstrap-flat에 `input` 리스너가 없어 타이핑이 signal에 반영되지 않습니다. (b) codegen이 input 자신에게 `data-loom-lref`를 달고, click 위임이 `dispatchLref(el)`를 value 없이 부르며, `dispatch`가 `unwrap_or("")`로 `SetString("")`을 만듭니다. 그래서 **클릭하면 값이 지워집니다**. R14와 같은 결함입니다 | codegen.rs:3322-3323, 439-460, 444; bootstrap-flat.js:63-66 | 필수 | bootstrap-flat.js·codegen | **RQ6**(기본 모델 bootstrap 바이트) | 코드 |
| R39 | S3 | `shared-store.js`가 DomOps 밖에서 `textContent`를 직접 씀, key 미이스케이프 | 두 번째 DOM 작성자가 생깁니다. key에 `"`가 있으면 셀렉터가 깨집니다 | demo/resume/shared-store.js:25-26; web.rs:32-33 | 선택(RQ4 폐기 경로) | JS | — | 코드 |
| R40 | S1 | 텍스트 양끝의 `&nbsp;`와 유니코드 공백(U+00A0, U+3000, em space 리터럴 포함) | Rust `str::trim()`은 유니코드 White_Space 전체를 자릅니다. JSX는 nbsp를 보존합니다. `<p>&nbsp;x&nbsp;</p><span>&nbsp;</span>`가 `<p>x</p><span></span>`가 되고, nbsp만 있는 노드는 통째로 사라집니다. R20과는 별개 결함이라 회귀 케이스가 따로 필요합니다 | lower.rs:709-711 | 필수 | lower | RQ6, N-G2 | 실측 |
| R41 | S1(R20과 결합) | 공백을 강제하는 관용구 `a{" "}b` | 거부됩니다(`unsupported JSX expression: Lit(Str…)`). 그래서 R20에 **저작자 쪽 우회로가 없습니다** | lower.rs:944 | 필수 | lower | R20과 묶음 | 실측 |
| R42 | S3 | raw-text 요소 `<style>`/`<script>`의 정적 내용 | HTML 이스케이프 때문에 내용이 바뀝니다. `<style>a > b</style>`가 `a &gt; b`로 나가 CSS 선택자가 깨집니다. `{…}`는 거부되므로 우회할 수 없습니다 | codegen.rs:1898-1900, 1915; loom-fic lib.rs:130-132, 160-162 | 선택 | codegen·StringDom | — | 실측 |
| R43 | S2 | 동적 서브트리 안의 SVG `<svg><g><text>{c()}</text></g></svg>` | 동적 경로는 `document.create_element(tag)`를 써서 HTML 네임스페이스로 만듭니다. 그래서 SVG로 렌더되지 않을 것으로 봅니다. 정적 서브트리는 `insert_adjacent_html`을 거쳐 정상입니다. **같은 마크업이 binding 유무에 따라 다르게 렌더됩니다** | web.rs:288-292, 317; codegen.rs:1913-1937 | 중요 | runtime(`create_element_ns` 없음)·codegen | **L§2.5** | create_element 생성은 실측, 렌더는 추론 |
| R44 | S1(공정성) | 중첩 `onClick` `<div onClick={d.inc}><button onClick={c.inc}>` | 진입점마다 의미가 다릅니다. bootstrap.js와 bootstrap-flat.js는 `closest("[data-loom-lref]")`로 **가장 안쪽 하나만** 실행합니다(사실상 stopPropagation). Island 진입점은 lref마다 리스너를 붙여 **버블링으로 둘 다** 실행합니다. lower와 codegen은 경고 없이 수용합니다 | bootstrap.js:117-118; bootstrap-flat.js:63-64; codegen.rs:759, 788 | 중요 | bootstrap·codegen | RQ6(Island arm과 Flatten arm의 의미 불일치) | 수용은 실측, 실행은 추론 |
| R45 | S3 | I32 `increment` 오버플로 `signal(2147483647)` | `.0 += 1`을 생성합니다. 워크스페이스 release 프로파일에 `overflow-checks`가 없어 wasm release에서는 `-2147483648`로 감깁니다. dev에서는 실행 중 패닉입니다. JS 의미(2147483648)와 다릅니다 | codegen.rs:1079; Cargo.toml:38-44 | 선택 | codegen | PT | 코드 |
| R46 | S2 | signal 이름이 Rust 키워드이거나 `$`를 포함(`const type = signal(0)`, `const $c`) | loomc는 통과합니다. `pub type: Entity`, `signals.$c`가 나와 **하위 크레이트 rustc에서 실패**합니다. 해당 키워드: `type ref move mod use impl where loop struct trait unsafe pub crate dyn match fn` 등. 한글 식별자는 통과합니다(2026-10-18 F1 주: 이 측정은 signal 이름 기준이다. 7a C7에서 "비ASCII 전부 거부"로 바뀌었고, 다시 받는 것은 7b — 문서 끝 "R46 정정" 절) | codegen.rs의 `Signals` 필드와 `{name}_entity` 생성 | 중요 | codegen(맹글링 없음) | — | 실측 |
| R47 | S3 | Flatten에서 루트로부터 도달할 수 없는 헬퍼 | 검사 없이 수용되고 컴파일되지 않습니다. Islands는 626에서 거부합니다 | codegen.rs:499-568, 626 | 선택 | codegen | — | 코드 |

---

## 2. 통합 격차 매트릭스

### 2.1 컴포넌트·props

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `export function App` 1개 + 비export 헬퍼 N개 | 지원 | lower.rs:130-146 | 필수 | — | — |
| export 2개 이상, TS 오버로드 | 거부(lower). 오버로드에서는 메시지가 오해를 부릅니다 | lower.rs:136-141 | 선택 | lower | — |
| `export default` / `export const App = () =>` / `export { App }` | 부재. "no exported function" 패닉이 납니다. 다른 export가 있으면 **조용**합니다(R25) | lower.rs:147, :150 | 중요 | lower | — |
| JSX가 아닌 일반 헬퍼 함수 | 거부(lower). 모든 비export 함수를 컴포넌트로 취급합니다 | lower.rs:64-66, :181-183 | 중요 | lower | — |
| props 선언·사용(`{props.x}`, `signal(props.x)`) | 사용은 거부(lower), 전달은 **조용**(R1) | lower.rs:944, :295 | 필수 | lower·IR·codegen | CAP(2), RQ6 |
| 미참조 헬퍼 | Islands는 거부(codegen 626), Flatten은 **조용**(R47) | codegen.rs:626, 499-568 | 선택 | codegen | — |
| 순환 참조 | **Flatten**: 순환에 signal을 가진 컴포넌트가 있으면 두 번째 방문에서 552의 "signal 이름 전역 유일" 패닉이 납니다. 원인을 가리키지 않는 메시지입니다. 루트 signal은 501에서 미리 들어가므로 루트가 순환에 끼면 반드시 이 패닉입니다. signal이 없는 순환만 스택 오버플로 abort(메시지 없음)입니다. **Islands**: 루트가 순환에 끼면 거부(rustc)입니다. 루트는 `*_generated` 모듈 없이 방출되는데 3422가 `{root}_generated::setup`을 호출하기 때문입니다. 루트가 아닌 헬퍼끼리의 B↔C 순환만 네이티브 SSR 무한 재귀(실행)입니다 | codegen.rs:501, 550-556, 620-622, 3407-3422 | 중요(진단) | lower 검사 부재 | — |
| 자기 참조, 헬퍼가 루트를 참조 | 거부(rustc) | lower.rs:56-60; codegen.rs:3407-3422 | 선택 | lower | — |
| `compile_tsx_to_rust`(single)에서 ComponentRef | 거부(rustc). 존재하지 않는 `*_generated::setup`을 호출합니다 | lib.rs:36-40 | 선택 | codegen | — |
| 대문자 HTML 태그 `<DIV>` | 거부(lower). "컴포넌트 미선언" 메시지이고 원인은 짐작할 수 있습니다 | lower.rs:323-329 | 선택 | 진단 | — |
| 동적 컴포넌트 선택 | 부재. 도입하면 PT가 다시 열립니다(docs/02:156-158) | — | 선택 | 전 레이어 | **PT** |

### 2.2 시그널·파생값

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `signal(int\|bool\|"s"\|["a"])` | 지원 | lower.rs:252-303; ir.rs:8-21 | 필수 | — | — |
| `signal(-1)`(Unary) | 거부(lower) | lower.rs:295 | 중요 | lower | — |
| `signal(1.7)`, `signal(3e10)` | **조용**(R19) | lower.rs:275 | 중요 | lower | — |
| `0 as number`, `x!` 등 TS 단언 | 거부(lower). `unwrap_paren`은 Paren만 벗깁니다 | lower.rs:245-250 | 선택 | lower | — |
| 객체·숫자 배열·레코드 시그널 | 거부(lower) | lower.rs:287-297 | 중요 | lower·IR·runtime | PT, G0(객체 row key) |
| `computed`/`derived`/파생 `const` | 거부(lower). row 스코프의 `LenGt`/`PrefixByItemLen`만 있습니다 | lower.rs:175; ir.rs:147-167 | 필수 | lower·IR·codegen(정적 System) | PT |
| 로컬 별칭 `const d = c;`, 시그널이 아닌 `const`/`let` | 거부(lower) | lower.rs:175 계열 | 중요 | lower | — |
| 같은 이름 signal 중복 | 거부(rustc, 추론). setup과 resume의 초기값도 어긋납니다 | lower.rs:170-173 | 중요 | lower | — |
| Rust 키워드나 `$`가 들어간 signal 이름 | 거부(rustc), loomc는 통과(R46) | codegen `Signals` 생성 | 중요 | codegen | — |
| 선언만 하고 binding이 없는 signal | 거부(codegen). 사용자 오류인데 "internal error"로 표시됩니다 | codegen.rs:1271 | 중요 | 진단 | — |
| I32 시그널에 writer 2개 | 거부(lower). single-writer 규칙이고 설계상 의도입니다 | lower.rs:92-118 | 설계 | — | CAP |
| `sharedSignal`/`sharedValue` | 부분. I32만 지원하고, RQ4 폐기 뒤에도 남은 잔존물입니다 | lower.rs:206-243, :739-757; runtime lib.rs:144-147 | 선택 | — | — |
| 최상위 `const c = signal(0)` | 무시된 뒤 사용 시점에 거부(lower) | lower.rs:147, :851-855 | 선택 | lower | — |

### 2.3 엘리먼트·속성

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| 소문자 태그(이름 제한 없음), 하이픈 커스텀 엘리먼트 `<my-el>`, 자기 닫힘 `<div/>`, 정적 `<textarea>` | 지원(실측). 모듈 문서 :16의 "5종"은 낡은 서술입니다 | lower.rs:318-321 | 필수 | — | — |
| void 태그 SSR | **조용**(R29) | codegen.rs:1930-1932 | 중요 | codegen | RQ6 |
| 동적 경로의 SVG | **조용**(R43) | web.rs:288-292 | 중요 | runtime | L§2.5 |
| `<Foo.Bar>`, `<svg:rect>` | 거부(lower) | lower.rs:318-320 | 선택 | lower | — |
| 정적 `class`/`className` | 지원(중복은 R6) | lower.rs:346-351 | 필수 | — | — |
| `class={"a"}`, 값 없는 `class` | 거부(lower) | lower.rs:347-349 | 선택 | lower | — |
| 정적 `href`/`id`/`src`/`alt`/`for`/`type`/`aria-*`/`data-*`/`style` | 거부(lower). IR Element 슬롯이 tag·class·action·children뿐입니다 | lower.rs:355; ir.rs:55-145 | **필수**(링크·이미지·label) | lower·IR·codegen(정적 prerender 1913-1937) | RQ6(픽스처 HTML은 지금 이 속성을 안 쓰므로, 추가만 하면 산출물이 불변) |
| 불리언 속성 `disabled` | 거부(lower) | lower.rs:355 | 필수 | lower·IR | — |
| `ref`, `dangerouslySetInnerHTML` | 거부(lower) "unsupported attribute". DOM 핸들 접근(focus·측정) 표면도, raw HTML 표면도 없습니다 | lower.rs:355 | 중요 / 선택 | 전 레이어 | L§2.5(ref가 핸들을 노출하면) |
| 동적 attr/class/style 바인딩 | 부재. `SetAttribute`나 class 토글 `DomWrite`가 없고 class는 생성 시점에 고정됩니다 | runtime lib.rs:278-284, 330-431; web.rs:293 | 필수 | 전 레이어 + **runtime DomWrite** | **L§2.5**, PT(제네릭 System + variant 추가가 정본, docs/02:145-149) |
| spread, 네임스페이스 속성 | 거부(lower) | lower.rs:339-344 | 선택 | lower | — |
| 정적 HTML 이스케이프 | **지원**(정정). 모든 속성을 큰따옴표로 감싸므로 `& < > "` 4문자면 텍스트와 속성 문맥 모두 충분합니다. 브라우저 쪽은 `text_content`·`set_attribute`·`.value`만 씁니다. raw 경로 두 곳(`append_static_html`, `RAW_HTML_TAG`)은 codegen이 이스케이프한 컴파일 시점 리터럴만 받습니다. 남은 결함은 raw-text 요소의 과잉 이스케이프(R42)뿐입니다. **R1을 고쳐 런타임 값이 정적 HTML 경로에 들어오면 다시 감사해야 합니다** | codegen.rs:1898-1924; loom-fic lib.rs:158-175; web.rs:317, 321-369, 443-459 | 중요 | — | — |

### 2.4 텍스트·표현식

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| JSX 텍스트, 엔티티 | 부분. 엔티티는 정상입니다. SWC가 디코드하고 codegen이 다시 이스케이프하므로 이중 이스케이프는 없습니다(실측). **공백은 깨집니다**(R20·R21·R40) | lower.rs:708-715 | 필수 | lower | RQ6, N-G2 |
| `{" "}`, `{"lit"}`, `{5}` | 거부(lower) 944(R41) | lower.rs:944 | 필수(`{" "}`) | lower | R20과 묶음 |
| `{n()}`(I32) | 지원 | codegen.rs:2790-2807 | 필수 | — | — |
| `{s()}`·`{b()}`(Str/Bool) | 거부(codegen) | codegen.rs:2003, 2798 | 필수 | codegen. `patch_text_system<T: ToString>`는 런타임에 이미 있습니다(lib.rs:965-977) | N-G2(텍스트 경로), PT |
| 정적 텍스트 + `TextBinding`/`ListBinding` 혼합 `Count: {n()}` | 거부(codegen). 조건부 안에서는 원인을 가리키지 않는 "미지원 노드" 메시지입니다 | codegen.rs:3331-3339, 2222 | **필수** | codegen·runtime(텍스트 노드 단위 핸들 없음. `set_text_content`는 첫 텍스트 노드만 바꿈, web.rs:321-369) | **L§2.5**(web.rs), N-G2 |
| 정적 텍스트 + `SharedValueBinding`/`CondBinding` 혼합 | 지원. `has_dynamic`이 이 두 종류는 보지 않습니다. 예: widget-embed.tsx:85 | codegen.rs:3332-3339 | — | — | — |
| binding + 형제 요소 혼합(`<ul><li>head</li>{xs().map(…)}</ul>`, 한 부모 아래 리스트 2개, `<span>{c()}<b>x</b></span>`) | 거부(codegen) "mixes a dynamic binding with other children". 조건부는 형제와 섞어도 통과합니다 | codegen.rs:3331-3339 | 필수 | codegen | N-G2 |
| `{c()+1}`, 템플릿 리터럴, 메서드 호출 | 거부(lower) "unsupported JSX expression" | lower.rs:870-872, :944 | 중요 | lower·IR·codegen | PT |
| JSX 주석 `{/* */}` | 거부(lower) | lower.rs:721 | 중요(목업이 사용) | lower | — |
| Fragment `<>`(루트·자식) | 거부(lower) | lower.rs:182, :721 | 중요 | lower·IR | — |
| row 밖의 `{item}`, `{item.name}` | 거부(lower) | lower.rs:944 | 객체 row와 함께 | lower | — |

### 2.5 이벤트·핸들러

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `onClick={() => x.increment()/append("lit")/rotate()/toggle()}` | 지원. 인자는 **조용**히 무시됩니다(R3·R4) | lower.rs:619-699; ir.rs:30-52 | 필수 | — | — |
| 타입이 맞지 않는 액션(`toggle`을 I32에, `increment`를 Bool·StringList에, `append`를 I32에) | 거부(lower). 메시지가 명확하고 경계가 정상입니다(실측) | lower.rs:619-699 | — | — | — |
| `onClick` 중복 | **조용**(R5) | lower.rs:353 | 중요 | lower | — |
| 중첩 `onClick` | **조용**(R44) | — | 중요 | bootstrap·codegen | RQ6 |
| `.set(...)`, `decrement`, 대입 | 거부(lower). Decrement가 없어 product-card가 우회합니다 | lower.rs:697; product-card.tsx:9-12 | **필수** | lower·IR(`ClickAction`) | PT(닫힌 열거형 확장), CAP(1)(.set이 다른 signal을 읽을 때) |
| 블록 본문, 여러 문장, 여러 시그널, 조건 분기 | 거부(lower) | lower.rs:633-635, :642 | 중요 | lower·IR | **CAP 조건 (1)**(docs/02:197-198) |
| `(e)=>…`, `onClick={c.increment}`, `function(){}` | 거부(lower) | lower.rs:627-632 | 선택 | lower | — |
| `async () => …`(onClick·onInput) | **조용**. `is_async`를 검사하지 않고 동기로 처리합니다 | lower.rs:538, 627 | 선택 | lower | — |
| `onChange`/`onSubmit`/`onKeyDown`/`onFocus`/`onBlur` | 거부(lower) | lower.rs:355 | **필수** | lower·IR·codegen·bootstrap(위임 이벤트는 click/input뿐, lref에 이벤트 종류 없음) | RQ6(bootstrap-flat) |
| `preventDefault`/`stopPropagation` | 부재(grep 0건). form 안의 버튼은 **조용**(R30) | — | 필수(form) | bootstrap·IR | RQ6 |
| 이벤트 객체 데이터(key 등) | 부재. input `.value`만 있습니다 | codegen.rs:770-791; bootstrap.js:125 | 중요 | 전 레이어 | CAP |
| 조건부 안 엘리먼트의 onClick | 거부(codegen) | codegen.rs:2023, 2172, 3040 | 필수 | codegen | 조건부B, N-G2 |
| row 안 onClick | row 루트는 **조용**(R8). 루트의 **직계** 자식 `<button onClick>`은 2290의 일반 메시지("지원하지 않는 노드")로 거부됩니다. `try_render_fully_static`이 `action: None`만 받기 때문입니다(1919). row 안 조건부 내부는 2319에서 거부됩니다 | codegen.rs:1919, 2289-2292, 2317-2320 | 필수 | codegen | N-G2 |
| Island 진입점 이벤트 | 부분. lref마다 click만 받고 값은 `""`로 고정됩니다(R15) | codegen.rs:770-791 | 중요 | codegen | RQ6(Island arm) |

### 2.6 조건부

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `{b() && <el>…</el>}`(Bool) | **부분**(정정). then 안의 `{n()}`·`.map`·중첩 요소는 지원합니다. 다만 then의 자식이 **binding 하나뿐**이고 SSR 시점에 열려 있으면 resume에서 실행 중 패닉합니다(R12). 네이티브 setup과 tick 경로는 정상입니다 | lower.rs:784-838; codegen.rs:3173-3195 | 필수 | codegen | 조건부B, N-G2 |
| `!b()`, `n()>3` | 거부(lower) 790 | lower.rs:790 | **필수** | lower·IR(파생 술어) | 조건부B, PT |
| Bool이 아닌 signal 술어 | 거부(lower) 803-806 | lower.rs:803-806 | 필수 | lower | — |
| `\|\|` | 거부(lower). `LogicalAnd` arm에 걸리지 않아 944의 일반 메시지로 나갑니다 | lower.rs:761-763, 944 | 중요 | lower | — |
| 삼항 `c ? <A/> : <B/>` | 거부(lower) 944, 일반 메시지. 설계는 `&&` 두 개로 desugar하는 것입니다(docs/02:73-74) | lower.rs:944 | **필수** | lower | 조건부B, G0 |
| `b() && "text"` / `<>…</>` | 거부(lower) | lower.rs:808-810 | 중요 | lower | — |
| then이 Component 또는 `<input>` | 거부(lower). `unreachable!` 내부 에러 메시지입니다 | lower.rs:812-814; codegen.rs:2031 | 중요 | lower·codegen | 조건부B |
| 조건부 안 조건부(직계 / 깊은 중첩) | 거부(lower :827-833 / codegen 2031) | — | 필수 | lower·codegen | 조건부B, **N-G2** |
| 조건부 안 sharedValue | 거부(codegen) | codegen.rs:2031, 2222 | 선택 | codegen | — |
| row 안 `o() && item.length > n` | 지원(`LenGt`). 단위는 R28 | lower.rs:770-781, :951-972 | 연구용 | — | N-G2 |
| row 조건부(정적만, 중첩 리스트 없음) | 거부(rustc). `list_signals`가 선언되지 않습니다 | codegen.rs:2675, 2695 | 중요 | codegen | N-G2 |
| 내용이 빈 조건부 | `SetVisible`이 없음(해 없음) | codegen.rs:3216-3221 | — | — | — |

### 2.7 리스트

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `xs().map(x => <li>{x}</li>)`(StringList) | 지원(K만 등록). 태그와 class는 **조용**히 버려집니다(R9) | lower.rs:914-942; codegen `list_reconcile_k_system` | 필수 | — | — |
| row v1 확장(`{item}` + 정적 + 조건부 1개 + 중첩 리스트 1개) | 지원 | codegen.rs:2258-2330 | 연구용 | — | N-G2 |
| 그 밖의 row 모양(`<li><b>{x}</b></li>`, row 안 `{c()}`, 조건부 2개) | 거부(codegen) | codegen.rs:2263, 2273, 2281, 2290, 2310, 2313, 2325 | 필수 | codegen | **N-G2** |
| `(x, i)` 인덱스, 구조분해, 블록 본문, Fragment row | 거부(lower) | lower.rs:917-934 | 중요 | lower | — |
| 중첩 리스트에서 바깥 row 변수 참조 | 거부(lower). `{outer}`는 Ident arm(733)에 걸리지 않아 944에서 거부됩니다. `.slice(0, outer.length)`는 992-995의 assert에서 거부됩니다. 935는 row 스코프를 교체하는 지점입니다 | lower.rs:733, 935, 944, 992-995 | 중요 | lower·IR(스코프) | N-G2 |
| key 저작 표면 / unkeyed opt-out | 부재(`key` 속성은 거부). 목표는 P1 고정 폭 POD key `key_op`입니다 | lower.rs:355; docs/03:19, 231 | **필수(RQ1)** | lower·IR·codegen·runtime | **G0**(D·E 기각 원인이 `String` key), L§2.5, N-G2 |
| `.filter(f).map(…)` | 거부(lower) 901-902 "must be called on a bare `signalName()`" | lower.rs:890-903 | 중요 | lower·IR(파생) | G0 |
| `.map`으로 끝나지 않는 체인(`.sort()`, `.reverse()` 단독 등) | 거부(lower) 870-872 | lower.rs:870-872 | 중요 | lower | — |
| 인자가 있는 호출 `xs(1).map` | 거부(lower) 898-900 | lower.rs:898-900 | 선택 | lower | — |
| 그 밖의 `.slice` | 거부(lower) | lower.rs:882-884 | 중요 | lower | — |
| remove/insert/set 연산 | 부재(append/rotate만) | lower.rs:697 | 필수 | IR·codegen | RQ1 시나리오 |
| 객체 row | 거부(§2.2) | — | 중요 | 전 레이어 | G0, PT |
| K가 아닌 후보 + 리스트의 resume | 거부(실행) `unimplemented!` | codegen.rs:1420-1432 | 연구용 | codegen | **G0**(D/E arm) |
| 조건부 안 리스트의 복잡 row | 거부(codegen) | codegen.rs:2009, 2012 | 중요 | codegen | N-G2 |

### 2.8 입력·폼

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `<input value={s()} onInput={(e)=>s.set(e.target.value)}/>` | 부분. 이 형태만 지원합니다. 진입점별 동작은 아래 행을 보십시오 | lower.rs:466-494 | 필수 | — | — |
| 단독 `<input/>`, `onInput` 없는 `value`(읽기 전용·비제어) | 거부(lower) "requires value" / "requires onInput" | lower.rs:466-494 | 중요 | lower·IR | — |
| `value={n()}`(I32) | 거부(lower) "requires a Str signal" | lower.rs:466-494 | 중요 | lower·IR | — |
| input의 `type`/`name`/`placeholder`/`class` | 거부(lower) | lower.rs:479-481 | **필수** | lower·IR | — |
| value와 onInput signal 불일치 | 거부. 메시지가 `assert_eq!`입니다 | lower.rs:489-492 | 선택 | 진단 | — |
| `(e.target as HTMLInputElement).value`, `currentTarget` | 거부(lower) | lower.rs:572-599 | 중요 | lower | — |
| checkbox `checked`, `<select>`, `<textarea>` 바인딩 | 부재(속성 거부). 런타임에 `checked`/`selected` 프로퍼티 쓰기가 없고 `HtmlInputElement` unchecked 캐스트만 있습니다 | web.rs:448-459 | **필수** | 전 레이어 + **runtime DomWrite** | **L§2.5**, PT |
| 조건부 안 input | 거부(lower `unreachable!`) | lower.rs:812 | 필수 | lower·codegen | 조건부B |
| 다중 Islands 안 input | 거부(codegen) | codegen.rs:630-637 | 중요 | codegen | RQ6 |
| 단일 Island 안 input | **조용**(R15) | codegen.rs:840-871 | 중요 | codegen | RQ6 |
| resume 데모(bootstrap.js) input | **조용**(R14). 클릭하면 값이 지워집니다 | bootstrap.js:117-123 | 필수 | bootstrap.js | — |
| **Flatten(기본 모델) input** | **조용**(R38). 타이핑이 반영되지 않고, 클릭하면 지워집니다. 코드로 확정했습니다 | bootstrap-flat.js:63-66; codegen.rs:3322-3323 | **필수** | bootstrap-flat.js·codegen | **RQ6** |
| form submit / 검증(파생 술어, 문자열 표시, 동적 disabled) | 부재. 구성 요소가 모두 거부되거나 부재입니다 | §2.2·2.3·2.4·2.5 | 필수(RQ6 §9-6) | 전 레이어 | RQ6. §9-6 위젯이 Bool 토글 3개로 대체된 원인입니다(RQ6-verification-plan.md:676, 1137-1139; widget-embed.tsx:13-15, 53-67) |

### 2.9 컴포지션·children

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| `<W client:load/>`, `<W client:visible/>` | 지원 | lower.rs:386-401 | 필수 | — | — |
| 디렉티브 없는 `<W/>` | 거부(lower). Flatten에서도 디렉티브가 필수라 H-Qwik 기본 모델과 표면이 어긋납니다 | lower.rs:411-417 | **필수** | lower | RQ6(표면 정합) |
| Flatten에서 디렉티브 | **조용**(R17) | codegen.rs:544 | 선택 | 진단 | RQ6 |
| 디렉티브 값·중복 | **조용**(R18) | lower.rs:395 | 선택 | lower | — |
| `client:idle`/`only`/`server:*` | 거부(lower). registry.js에도 load/visible만 있습니다 | lower.rs:389-400; registry.js:183-189 | 선택 | lower·registry.js | — |
| children 전달 | **조용**(R2) | lower.rs:378-422 | 필수 | lower·IR | RQ6 |
| 같은 컴포넌트 두 번 | Flatten은 거부(codegen 552, signal 이름 충돌). Islands는 **조용**(R16) | codegen.rs:550-556; docs/05:45-47 | **필수** | codegen(Flatten 재명명) | **RQ6**(픽스처가 이름을 손으로 고유화, widget-embed.tsx:16-20) |
| 단일 Island stub 중복 ref | 거부(rustc E0428, 추론) | codegen.rs:876-893 | 선택 | codegen | — |
| row 안 / 조건부 안 ComponentRef | 거부(codegen) | codegen.rs:2031, 2290 | 중요 | codegen | — |

### 2.10 효과·비동기·생명주기

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| 비동기 로드 / suspense(`Loading` variant) | 부재(저작 문법 미정의). 설계는 actor + Command입니다 | docs/06:39, 63-68 | 필수(RQ6 §9-6) | 전 레이어 + 런타임 스케줄러(`tick`은 동기 호출로만 돎) | L§2.5, PT, RQ6 |
| `await` 문장 | 거부(lower) | lower.rs:175, :186 | — | — | — |
| `async` 핸들러 | **조용**(§2.5) | lower.rs:538, 627 | 선택 | lower | — |
| effect / cleanup | 부재(API 미정의). Component가 POD여야 하므로 정적 System이나 열거형이어야 합니다 | docs/06:37 (57-61은 detach 프로토콜 서술입니다) | 중요 | 전 레이어 | PT |
| 타이머, fetch | 부재(grep 0건) | runtime | 중요 | runtime | L§2.5 |
| unmount / detach 정리 | 스텁. `begin_detach`가 no-op이고 리스너는 `forget()`으로 누수됩니다 | codegen.rs:791, 801; island.rs:109-116; registry.js:125-135 | 중요 | codegen·runtime | RQ6(Island arm) |
| focus / scroll / DOM 측정 | 부재(`ref`도 거부, §2.3) | runtime | 선택 | runtime | L§2.5 |

### 2.11 SSR·resume·FIC

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| I32/StringList/Bool resume(단순 형태) | 지원 | codegen.rs:1403-1723 | 필수 | — | — |
| 캡처 = 액션당 I32 1개(리터럴 초기값 전제) | 부분. props가 들어오면 CAP (2)가 깨집니다 | codegen.rs:1306-1353; docs/02:199-205 | 필수 | codegen | **CAP** |
| 복잡 row / 조건부 wrapper / row 상태의 resume | **조용**하거나 실행 중 패닉(R11·R12·R32) | — | 중요 | codegen·web.rs | N-G2, 조건부B |
| resume 직후 settle tick에서 row 전량 재생성 | 비용 문제(문서화됨) | codegen.rs:48-65, 1689-1703 | 중요 | codegen | N-G2, RQ6 측정 |
| FIC `render(component_ref, props, request_context)` | 부재. 인자 없는 함수 하나뿐이고 컴포넌트가 고정입니다 | loom-fic lib.rs:28-38, 420, 440; build.rs:9; docs/05:63-82 | 필수(docs/05) | loom-fic·codegen | CAP |
| anchor별 조각 / assets | 부재. assets는 항상 `Vec::new()`입니다 | loom-fic lib.rs:335; docs/05:178-186 | 중요 | loom-fic | — |
| sidecar | 부분. `starts_with("GET /render")`만 보고 쿼리를 무시하며, `_full`은 노출하지 않습니다 | sidecar.rs:8-25 | 선택 | loom-fic | — |
| `loom_fic_free` ABI | 안전한 `extern "C"` 시그니처 안에서 `unsafe { Vec::from_raw_parts }`를 씁니다(UB 가능) | loom-fic lib.rs:459-463 | 중요 | loom-fic | — |
| StringDom 복사본 4벌 | 분리 조건("세 번째 복사본", lib.rs:25-26)을 이미 넘었습니다 | loom-fic 외 3곳 | 선택 | — | RQ6 |

### 2.12 Flatten·Islands

| 구문 | 상태 | 근거 | 필요도 | 레이어 | 영향 |
|---|---|---|---|---|---|
| Flatten 기본 모델 | 지원. 전역 signal 이름이 유일해야 합니다 | codegen.rs:263-273, 499-568 | 필수 | — | RQ6 |
| Flatten signal 자동 재명명 | 부재(552에서 거부) | README.md:298-299 | 필수 | codegen | RQ6(충돌이 날 때만 재명명하면 기존 픽스처 산출물은 바뀌지 않음) |
| Flatten `shared_bindings` key 중복 | 검사 없음 | codegen.rs:559 | 선택 | codegen | — |
| Flatten 전역 id 조회 | **조용**(R33) | codegen.rs:376-402 | 중요 | codegen | RQ6 |
| 완전 정적 Island | 거부(codegen) | codegen.rs:1196 | 선택 | codegen | — |
| Island 레지스트리 상태 기계 | Rust와 JS에 손으로 이중 구현 | island.rs:50-79; registry.js:12-155 | 선택 | — | — |
| `loomc` CLI | 부재(빈 `main`) | main.rs:8 | 중요 | 툴링 | — |

### 2.13 진단·툴링·문서

| 항목 | 상태 | 근거 | 필요도 | 레이어 |
|---|---|---|---|---|
| span·줄·열이 있는 진단 / `Result` API | 부재. lower 거부 지점 121줄 + codegen 패닉 | lower.rs:104-990; codegen 전반 | **필수** | lib 전체 |
| 복구 가능 파스 에러 | **조용**(R23) | lib.rs:20-24 | 중요 | lib.rs |
| 사용자 오류를 internal로 표시 | `unreachable!`(lower.rs:812-814), "never spawned"(codegen.rs:1271), `assert_eq!`(lower.rs:489-492) | — | 중요 | 진단 |
| 생성 코드의 실행 시점 패닉 지점 | `expect`(codegen.rs:335, 376, 387, 397, 704, 788), `find_node`(410, 732), unknown symbol(1389), `unimplemented!`(1428) | codegen.rs | 중요 | codegen |
| 문서와 코드 불일치 | 아래 목록 참조 | — | 선택 | 문서 |

**문서와 코드가 어긋난 곳**
- lower.rs 모듈 문서 :14, :16("5종"), :32
- runtime lib.rs:205-208, :413-421
- `compile_tsx_to_rust` 문서 주석(lib.rs:32-33)이 호출처로 `loom-bench`를 적고 있습니다. 실제로 `loom-bench`는 `loom-runtime`에만 의존합니다.
- ir.rs:324-325의 `specialize_component_refs` 주석이 존재하지 않는 함수를 가리킵니다. docs/02:262-263은 이미 정정돼 있습니다.
- `examples/counter-island.sample.tsx`를 현 컴파일러가 거부합니다. CLAUDE.md "이 패키지의 한계"에 적힌 대로 목업이지만, 그 사실을 표시해야 합니다.

---

## 3. 요약: 목표까지의 거리

1. **연구용 표면은 지원하고, 실사용 표면은 대부분 비어 있습니다.**
   - 지원: 스칼라·문자열 리스트 signal, I32 텍스트, Bool `&&` 조건부(R12 예외 있음), 문자열 `.map`과 v1 중첩 row, 닫힌 4종 click 액션.
   - text input은 정확히 한 형태만 받습니다. 그 형태도 기본 모델(Flatten)에서는 동작하지 않습니다(R38).
   - 위젯 하나에 흔히 필요한 것은 전부 거부되거나 부재입니다: 정적 속성, 혼합 텍스트, Str 표시, 삼항, `.set`/decrement, onSubmit/onChange, 검증 폼, key, 비동기, effect.
2. **가장 큰 위험은 거부가 아니라 조용한 오컴파일입니다(47건).** S1만 꼽으면 다음과 같습니다.
   - 의미 소실: R1, R2(props·children), R3, R4(인자), R7(signal 공유 사이트), R8(row onClick), R12(resume 패닉)
   - 텍스트 바이트 변경: R20, R40, R41(공백, 우회로 없음)
   - 브라우저 상태 파괴: R13, R14, R38(input 소거·무반응)
   - 진입점 간 의미 불일치: R44
3. **진단 체계가 없습니다.** 저작 표면을 넓히기 전에 `Result` 기반 span 진단이 먼저입니다. SWC의 `swc_common` span/`Handler`를 재사용해야 합니다. 직접 진단 프레임워크를 만드는 것은 CLAUDE.md 2-1 "바퀴 재발명 금지" 위반입니다.
4. **연구 동결과 부딪히는 지점은 레이어로 정해집니다.**
   - 컴파일러만 고치면 L§2.5와 무관합니다.
   - 리스트·조건부·row codegen 산출물이 바뀌면 N-G2 재인증이 필요합니다.
   - Flatten 출력, 텍스트·HTML, bootstrap-flat.js가 바뀌면 RQ6 재측정이 필요합니다. R20을 고치기만 해도 픽스처 HTML 바이트가 바뀝니다.
   - 런타임 `DomWrite`·`tick`·`web.rs`를 고치면 L§2.5 동결이 깨집니다.
5. **결정 재개 조건**
   - key 표면은 G0 P1(고정 폭 POD key)을 먼저 만족해야 합니다.
   - 다중 시그널 핸들러는 CAP 조건 (1)을, props 초기값은 CAP 조건 (2)를 깨므로 RQ로 올려야 합니다.
   - 동적 컴포넌트 선택은 PT를 다시 엽니다.
   - 조건부 안 조건부, 삼항, then이 Component인 경우는 조건부 경로(첫 spawn 지연 + CSS 은닉)의 확장입니다. 아직 결정 기록이 없는 B 전환 문제와 얽힙니다.
6. **측정 유효성에 대한 열린 질문.** 새로운 명시적 거부(R1–R8, R44 등)를 넣을 때는 현재 소비자 TSX 7개와 생성기 산출물에 먼저 적용해 봐야 합니다.
   - 걸리는 픽스처가 **하나라도 있으면** 그 픽스처는 지금까지 잘못 컴파일된 코드로 측정된 것입니다. 그 경우 연구 산출물 재검토가 수리보다 먼저입니다.
   - 정독으로는 `demo/widget-embed.tsx`에 해당 패턴이 없습니다. 나머지 파일은 확인하지 않았습니다.

---

## 이 문서의 지위

- 2026-10-17 감사 시점(HEAD aac2dc2)의 **사실 기록**이다. 컴파일러를 고치면 해당 행에 날짜 붙은 갱신을 덧붙인다(이력 보존).
- 위험 표(§1)의 항목은 **아직 고치지 않았다.** 수리 순서와 범위(Phase 7 후보: 위험 수리 → 위젯 실사용 최소 표면 →
  런타임 표면 → 결정 의존 표면)는 병익님 결정 대기다.
- §3.6의 측정 유효성 질문(기존 소비자 TSX 7개가 위험 패턴에 걸리는가)은 아직 전수 확인하지 않았다. 확인된 것은 R20
  하나다 — RQ6 위젯-임베드 픽스처의 `cart: {…}`가 공백을 잃은 채 SSR된다(widget-embed.tsx:85 → flat/index.html:20).
  같은 소스를 두 팔(Islands/Flatten)이 공유하므로 팔 간 비교에는 대칭이지만, 픽스처 HTML 바이트가 저작 의도와 다르다.

## 2026-10-17 갱신 — §3.6 전수 판정 결과 (연구 산출물 재검토)

- **히트(실제 산출물에서 확인)**: R20(widget-embed, 두 팔), R13·R14(counter-list → resume 데모), R11·R32(nested-list →
  loom-nested-fixture resume 테스트). R29는 바이트만 해당(`</input>`, 의미 영향 없음).
- **잠복(패턴은 있으나 산출물이 정답과 같음)**: R31(R13과 결합하면 확정), R17(설계상 의도), R25, R39, R28(트랙 N 오라클이 같은
  의미를 복제해 검출력 없음).
- **재검토 결론**: 무효가 되는 판정은 없다. RQ6 §3.9 유지 + 주석(RQ6-verification-plan §3.10), RQ1 트랙 N G2·트랙 L/T 영향 없음
  (프로토콜 말미 재검토 기록). 공허한 확인 4건을 표시했다(RQ6 M8 정적 텍스트 미단언, Phase 2 resume 순서가 R13을 못 잡음,
  Phase 2 input 클릭 미검증, nested_list resume 테스트).
- **행 갱신**: R13 영향 "—" → "RQ6 §3.1·§3.6 M1 Layer 0, Phase 2 검증, CAP (2)". R14 영향 "—" → "Phase 2 input 검증".
  R11 확인 수준 "추론" → "산출물 실측", N-G2 영향 → "없음(G2는 resume 미사용)". R20의 N-G2 영향 → "트랙 N 픽스처에는 히트 없음".
- **§0 정정**: "직접 입력 TSX 7개"는 TSX 6개 + build.rs 합성 소스 1종(largeapp 두 크레이트)이다(nested-list-derived가 두 번
  세어졌다). §3 항목 6의 "정독으로는 widget-embed.tsx에 해당 패턴이 없습니다"는 R20 히트와 모순된다 — 정정한다.
- 부분 확인: `demo/islands/product-card.tsx`와 합성 소스는 R20만 확인했다. 확인 수준 한계: 고부하로 빌드·브라우저 실행 없이
  커밋 산출물과 OUT_DIR 생성 소스를 읽어 판정했다.

## 2026-10-17 갱신(7a) — Phase 7a 위험 수리 결과 (행별)

> 위 표(§1)와 §0.3·§2.13의 원문은 감사 시점(aac2dc2) 기록이라 고치지 않는다. 아래가 행별 갱신이다.
> 출처: 브랜치 phase7a 커밋 20ce418(C0)~46e0da7(C9), 설계 design-7a(개정판). 모든 커밋이 바이트 동일 게이트를
> 통과했다(RQ1 프로토콜 "개정 #11 적용 기록 — 7a"). **소비자 22회 컴파일 호출 중 새 거부에 걸린 것은 0건**이다
> (`crates/loom-compiler/tests/consumer_conformance.rs`가 22회 전부 `Ok`와 호출별 기대 경고를 단언). 진단 체계는 docs/02
> "진단 (2026-10-17, Phase 7a)" 절에 있다.

**처리 방식**: "거부"는 지금까지 조용히 다른 뜻으로 컴파일되던 입력을 `Err(Diagnostics)`로 막은 것이다. 받아 주는 입력의 IR과
codegen은 바뀌지 않았다(codegen.rs는 C9의 R12 텍스트 수정 한 곳만 바뀜). 따라서 "거부"는 **기능 지원이 아니다** — 해당 표면은
여전히 없고, 이제 없다는 사실이 진단으로 드러난다. 각 행의 "보낼 곳"이 지원을 맡는다.

| 행 | 2026-10-17 갱신(7a) | 커밋 | 지원은 |
|---|---|---|---|
| R1 | **거부 LOOM-R01.** 리터럴 prop(`<W client:load initial={5}/>`)과 컴포넌트 함수 매개변수. 순서: 기존 `lower_prop_literal`을 먼저 돌리고 리터럴에만 R01 — 비리터럴 prop은 7a 이전 문구 그대로 L00이라 RQ6 M10 증거 테스트(`component_ref_prop_cannot_carry_a_live_signal_value`)의 문구·관측 메커니즘이 유지된다. 이 행이 지목한 "패닉하지 않음을 고정하는 테스트"(옛 lib.rs:989)는 R01 부정 테스트로 뒤집었다(이름은 이력 보존). CAP 조건 (2)는 그대로 열린 문제다 | C6 714c1f6 | 7d(RQ 먼저) |
| R2 | **거부 LOOM-R02.** 컴포넌트 참조의 children — 공백뿐인 텍스트 외 전부, `{/* c */}` 포함(일반 요소의 기존 규칙과 같은 경계) | C6 | 7d(슬롯) |
| R3 | **거부 LOOM-R03.** `.increment(…)`·`.rotate(…)`에 인자. 문구는 `{s}.increment(…)`(lower에 SourceMap이 없어 인자 원문 대신 span 강조로 보여 줌) | C5 fa37391 | 7b(액션 인자) |
| R4 | **거부 LOOM-R04.** `.toggle(…)`에 인자 | C5 | 7b |
| R5 | **거부 LOOM-R05.** 한 요소에 `onClick` 두 번(둘째 attr이 주 span, 첫째에 label) | C6 | — |
| R6 | **거부 LOOM-R06.** 같은 attr 두 번 — `class`/`className` 합산, `<input>`의 `value`/`onInput` 중복 포함 | C6 | — |
| R7 | **거부 LOOM-R07**(사이트 2곳 이상, 사이트마다 label) + **LOOM-R07U**(표시되는 곳이 없는 signal — codegen.rs "never spawned" 내부 오류를 사용자 오류로 승격). 집계 대상: Text·Cond 조건·List·Input binding과 row 원본(row 안 cond/list). **이후 트랙 N 픽스처 설계의 제약**: row 원본 signal을 바깥 binding에서 함께 쓸 수 없다(실제로 두 번 spawn되는 결함 경로라 거부). R07U는 모드 무관이라, 7a 이전 단일 Island 경로에서 대상 밖 컴포넌트의 미표시 signal이 통과하던 것도 이제 거부한다 | C7 caa5bbb | 사이트별 entity(미정) |
| R8 | **거부 LOOM-R08.** `.map` row 루트의 `onClick`(조건부 안 리스트 포함) | C7 | 7d(key 표면, G0 P1 이후) |
| R12 | **텍스트 직계는 구현, 리스트 직계는 거부.** (1) `{c() && <p>{n()}</p>}`: resume의 I32 블록이 SSR DOM에 없는 `find_node("n")` 대신 wrapper `find_node("c")`를 쓰도록 codegen 수정(`CondSite.direct_text`). 방출 차이는 그 한 줄. 정확한 방출은 블록 전체 문자열 일치로 단언하고, 미커밋 스크래치 크레이트(별도 `[workspace]`)에서 SSR→resume을 실측했다 — 수정 컴파일러 2/2 통과, 부모 컴파일러는 "SSR DOM에 없는 data-loom-id=\"count\""로 패닉(결함 존재 확인). 영구 회귀 크레이트는 Cargo.lock을 바꿔 게이트 D에 걸리므로 7b로 미룬다. **대역 불일치 발견**: `loom-nested-fixture`의 `TreeDom::adopt_children`은 텍스트 조각을 빼고 돌려주는데 `BrowserDom::adopt_children`(web.rs)은 텍스트 노드도 adopt한다. 그래서 TreeDom 위에서는 열린 직계 텍스트를 닫힘으로 오판해 부모 컴파일러도 패닉 없이 통과한다 — 7b의 영구 회귀 테스트는 **텍스트 노드를 adopt하는 대역** 위에서 돌려야 한다(스크래치 실측은 그런 래퍼로 했다). (2) `{c() && <ul>{xs().map(…)}</ul>}`: **거부 LOOM-R12**(주 span은 wrapper 태그, "`<div>`로 한 번 감싸십시오"). 구현하면 같은 DOM 노드를 Bool 패스와 리스트 패스가 두 번 adopt하고(CondBlock 크기 1과도 어긋남), SSR 시점 열림 + 빈 리스트를 닫힘으로 복원한다 | C9 46e0da7, C8 681a30e | 리스트 직계 지원 7b |
| R13·R14 | **7a에서 처리하지 않음 — 병익님 결정 대기.** `demo/resume/bootstrap.js` 수정(설계 C11)은 개정 #11 조건 1이 "bootstrap JS"를 산출물로 열거한다는 문언 해석이 걸려 있어 만들지 않았다. 사실관계: RQ6 M1 Layer 0이 잰 것은 16c7e10판이고 49b8660에서 이미 바뀌었다(HEAD판은 측정판이 아님) | — | 결정 후 C11 또는 7b |
| R15 | **거부 LOOM-R15.** 단일 Island 대상 컴포넌트의 `<input>` 양방향 binding. 찾기는 List row·Cond 안까지 모든 노드를 내려간다. 다중 Islands의 같은 거부는 7a 이전부터 있던 것이라 LOOM-L00·기존 문구를 유지하고 순회 깊이만 맞췄다 | C8 | 7b(Flatten input 수리와 함께) |
| R16 | **거부 LOOM-R16**(Islands·단일 Island). 같은 컴포넌트를 가리키는 참조가 그래프 전체에 2개 이상(디렉티브가 같아도 island id 충돌). **Flatten에는 검사를 더하지 않았다**: signal 있는 중복은 기존 Flatten 이름 유일성 검사(L00)가 이미 거부하고, signal 없는 중복 참조는 같은 정적 마크업을 두 번 펼칠 뿐이라 무해하다(테스트로 고정) | C8 | — |
| R17 | 7a 범위 밖(경고로 해도 Flatten 소비자 전부에서 뜸, "디렉티브 없는 `<W/>`" 표면 결정과 묶임) | — | 7b |
| R18 | **거부 LOOM-R18.** 디렉티브의 값(`client:load={false}`), 디렉티브 둘 | C5 | — |
| R19 | **거부 LOOM-R19.** `lit_i32`(소수부 0이고 i32 범위): `signal()`, `sharedSignal`·`sharedValue` 초기값, prop 리터럴. 길이 비교 `it.length > 1.5`(`LenGt`)도 거부하지만 **오컴파일이 아니라 표기 정규화**로 분류한다 — 길이는 정수라 `> 1.5`와 `> 1`이 같은 뜻이고 포화도 같은 결과다. 그래서 그 문구에는 "잘림"을 쓰지 않는다 | C5 | — |
| R22 | **거부 LOOM-R22.** `return` 뒤의 문장 | C6 | — |
| R23 | **거부 LOOM-R23.** `parse_module()` 뒤 `take_errors()` 수거(SWC 메시지 재사용). 착수 전 프로브: 소비자 22회 호출 소스의 수거 0건, `with`/`010` 양성 대조는 실제로 수거됨. SWC는 `with`·`010`에 같은 span으로 2건(TS 진단 + strict 진단)을 내고, 중복 제거 없이 둘 다 보고한다 | C4 0b94bb7 | — |
| R24 | **거부 LOOM-R24.** 소문자 태그가 선언된 컴포넌트 이름과 같음 | C6 | — |
| R25 | **두 갈래.** (1) 거부 LOOM-R25: 최상위는 `import {…} from "loom"`(이름 지정자, 별칭 없음, `signal`·`sharedSignal`·`sharedValue`의 부분집합), `function`, `export function`만 받는다. 그 밖은 전부 오류 — 설계가 열거하지 않은 빈 문장 `;`, enum, 재export, 별칭·default·namespace import도 함께 오류다(`;`는 무해하지만 함께 거부, 완화 여부는 병익님 판단 대상). (2) **경고 LOOM-W25**: 쓴 callee를 "loom"에서 import하지 않음(callee마다 1건, 첫 사이트). 소비자 8회 호출에서 뜬다(widget-embed 6회 `sharedValue` — 이 행이 적은 widget-embed.tsx:30/:85 실례, largeapp 합성 소스 2회 `signal`). 그 8개 크레이트의 build.rs가 `cargo:warning`을 출력하지만 산출물 바이트는 같다. 원인 소스를 고치는 것은 RQ6 픽스처 소스를 건드리는 일이라 7b로 미룬다 | C6 | 7b(소스 정리 후 오류 승격 검토) |
| R26 | **거부 LOOM-R26.** 인자 초과: `signal()`, `.set()`, `.append()`, `.map()`. `<input>` 자식(R02와 같은 경계). `sharedSignal`·`sharedValue`는 이미 정확히 2개를 L00으로 요구하므로 더하지 않았다 | C5 | — |
| R27 | **거부 LOOM-R27(확장).** 컴포넌트 함수의 async/generator/타입 매개변수, `let`/`var`, **그리고 onClick·onInput·`.map` 화살표의 async/generator/타입 매개변수**. 반환 타입 주석은 의미가 없으므로 받는다. 매개변수는 R01이 맡는다 | C6 | — |
| R44 | **거부 LOOM-R44.** onClick 요소 안의 onClick·`<input onInput>`·컴포넌트 참조(진입점 간 의미 불일치 차단). 안쪽이 주 span, 바깥 onClick에 label | C7 | 이벤트 전파 의미 결정 후 |
| R46 | **거부 LOOM-R46(확장).** (a) signal·컴포넌트 이름이 Rust 2024 strict·reserved 키워드이거나 `_`, (b) ASCII 영숫자·`_` 밖 문자(`$`, **비ASCII 전부** — 이 행의 "한글 식별자는 통과합니다"는 7a부터 거부로 바뀜; U+200C/U+200D·비-NFC도 여기서 걸림), (c) 루트가 아닌 두 컴포넌트가 codegen의 모듈 이름 변환 뒤 충돌(`ProductCard`/`Product_card`, Islands·단일 Island만). (c)는 codegen의 private 함수를 부르지 않고 같은 규칙을 check.rs에 다시 구현했고, codegen이 실제 방출한 `pub mod …_generated` 이름과 대조하는 표류 방지 테스트를 뒀다 **2026-10-18 정정(F1)**: (b)의 "비ASCII 전부"와 (a)의 컴포넌트 이름 키워드 검사는 F1에서 바뀌었다 — 문서 끝 "R46 정정" 절 | C7(정정 F1) | 맹글링(미정) |
| R47 | **거부 LOOM-R47(모드 무관).** 루트에서 도달할 수 없는 헬퍼. Islands의 기존 거부(codegen.rs:620-627)를 흡수 | C7 | — |
| (추가) G01 | **거부 LOOM-G01.** 단일 컴포넌트 API(`compile_tsx_to_rust`, `_with_candidate`)에 컴포넌트 참조 — 지금까지는 rustc 단계에서야 실패했다. 같은 소스는 그래프 API에서 Ok | C7 | — |

**7a에서 손대지 않은 행**: R9–R11, R20, R21, R28–R43(R44 제외), R45. 사유와 보낼 곳은 설계 §1.4와 같다 — 산출물 바이트가
바뀌는 것(R20·R21·R40·R41·R29·R38 → 7b, R10·R11·R28·R32 → G0 새 기반, R45 → 7b/PT), 런타임 수리가 필요한 것(R9·R34–R37·R43 → 7c),
CAP(R31 → 7d). 설계 §1.2의 선택 항목(R30·R42 태그 거부, R33 테스트)은 넣지 않았다.

**§0.3·§2.13 갱신**
- "진단 체계가 없습니다" → 7a에서 생겼다. 공개 API 4종과 `parse_tsx`가 `Result<_, Diagnostics>`를 돌려주고, 진단은 swc_common의
  `Handler`/`EmitterWriter`로 `파일:줄:열` + 소스 발췌 + 강조를 렌더한다(새 의존성 없음). lower.rs의 거부 122곳 중 120곳이 진단이 됐고,
  매치 가드가 보장한 모양을 다시 분해하는 내부 `unreachable!` 2곳만 남았다. 7a 이전부터 있던 거부는 코드 LOOM-L00(single-writer는
  LOOM-L01)이고 기존 문구를 유지했다.
- 사용자 오류를 internal로 표시하던 셋: `unreachable!`(옛 lower.rs:812) → L00, "never spawned"(codegen.rs:1271) → R07U,
  `assert_eq!`(옛 lower.rs:489-492) → L00.
- codegen의 사용자 도달 패닉은 codegen을 고치지 않고 `check.rs`가 같은 순회 순서로 **사전 검사로 복제**한다. codegen의 패닉은 내부
  백스톱으로 남고, 행마다 "검사 없이 codegen을 부르면 그 패닉이 실제로 난다 + 공개 API는 진단을 준다" 테스트가 있다.
- 복구 가능 파스 에러(R23) → 위 표.
- 생성 코드의 실행 시점 패닉 지점(`expect`·`find_node` 등)은 7a 범위 밖이다. 단, R12 텍스트 직계의 `find_node` 패닉 경로는 C9가 없앴다.
- 문서·코드 불일치 목록 중 `compile_tsx_to_rust` 주석의 `loom-bench` 오기는 C1에서 정정됐다. 나머지(lower.rs 모듈 문서, runtime lib.rs,
  ir.rs의 `specialize_component_refs`, `examples/counter-island.sample.tsx`)는 그대로다.

**"이 문서의 지위" 갱신**: "위험 표(§1)의 항목은 아직 고치지 않았다"는 7a 이후 위 행별 갱신으로 대체한다. 7a 수리는 컴파일러만이고
런타임·데모 JS(R13·R14 결정 대기)·소비 픽스처 소스는 건드리지 않았다.

### R46 정정 (2026-10-18, Phase 7a 후속 F1 — 소유자 위임 결정)

위 7a 표의 R46 행(b)는 "ASCII 영숫자·`_` 밖 문자는 전부 거부"였고, 진단은 그 이유를 "생성 코드가 rustc에서 실패"라고 적었다. 이것은
signal 이름에 대해 **사실이 아니었다** — 7a 이전 컴파일러 + rustc 1.98.1로 한글·`café`(NFC) signal이 islands·flatten·single 세 경로
모두 통과했다(위 R46 위험 행의 "한글 식별자는 통과합니다"와 같은 결과). 병익님은 경쟁 프레임워크를 살펴보고 알아서 정하라고 위임했다.
이전 문구(위 표)는 7a C7 시점의 기록으로 남긴다.

**경쟁 프레임워크 조사 요약**(원본 링크·줄 번호는 세션 스크래치 r46-research.md): 사용자 식별자를 거부하는 곳은 없었다. 모두 원래 문자열을
그대로 두고, 컴파일러가 사용자 이름에서 **파생해 만드는** 출력 식별자만 ASCII로 바꾸고 중복을 해소한다 — Svelte `Scope.generate`, Qwik optimizer
`escape_sym`(+카운터·해시), Vue `toValidAssetId`, Solid는 JS 식별자가 아닌 키를 문자열/computed 키로 낸다. Rust 쪽 선례 Dioxus는 키워드를 `r#`로
받고 HTML에는 원래 문자열을 쓴다(Rust 식별자와 출력 문자열 분리).

**결정(최종 형태)**:
- signal 이름: Rust 식별자 규칙(UAX#31 XID, ZWJ/ZWNJ 금지) + **NFC 강제**로 판정해 한글·`café`·그리스 문자·CJK 등을 받는다. NFC 강제는 Rust가
  식별자를 NFC로 정규화해 JS에서 다른 두 철자가 한 이름으로 합쳐지는 문제(같은 컴포넌트 → E0124/E0062, Flatten으로 합친 두 컴포넌트 → E0124;
  Flatten 이름 유일성 검사는 바이트 비교라 놓침)를 원천 차단한다 — 그러면 "바이트가 같다"와 "rustc가 같은 이름으로 본다"가 일치한다.
  세션 합성안의 "한글 음절만 허용 목록"은 채택하지 않았다(특정 언어만 고르는 근거가 없음).
- 키워드는 `r#`(raw 불가한 `self super crate Self _`는 계속 거부), `$` 등은 Rust 쪽 식별자만 결정적으로 맹글링하고 원래 문자열은
  `data-loom-id`·lref에 유지, 맹글링 결과 사이 충돌 검사.
- 컴포넌트 이름: ASCII만(Islands·단일 Island의 `{C}Island` wasm-bindgen 내보내기가 wasm32에서 rustc E0754). 키워드 검사는 제거 — 걸리는
  키워드가 `Self`뿐인데 방출형이 `self_generated`·`SelfIsland`라 막는 결함이 없었다.

**7a에서 하지 못한 것과 그 이유(실측)**: XID·NFC 판정에는 `unicode-ident`·`unicode-normalization`이 필요하다. 이것을 loom-compiler에 직접
의존성으로 넣자(F1 첫 시도) 게이트가 **FAIL**했다 — 생성 소스(A 22/22)·SSR(B 5/5)은 바이트 동일이었지만, 컴파일러를 build-dependency로 쓰는
소비자 크레이트의 빌드 메타데이터 해시가 바뀌어 **host OUT_DIR 경로가 달라졌다**(`research-questions/data/phase7a-followup-byte-gate-2026-10-18-F1a.json`,
`failures: ["outdirs:host"]`). OUT_DIR 절대경로는 wasm에 박히므로(7a C0 검토 실측) 측정 산출물(RQ6 소비자 wasm) 바이트가 바뀐다. 컴파일러
**소스** 변경은 메타데이터 해시에 들어가지 않아 7a 내내 문제가 없었지만, **의존성** 변경은 다르다. 바이트 동일이 조건인 7a에서는 넣을 수 없으므로
일반 판정은 소비자 바이트가 어차피 바뀌는 7b(JSX 공백 규칙 수리 + RQ6 재측정)로 옮겼다.

**7a 잠정 규칙(F1 커밋)**: 의존성 없이 할 수 있는 것만 했다.

| 이름 | 처리(7a 후속 F1) | 진단 문구 |
|---|---|---|
| signal: Rust 키워드, `_` | 거부(변경 없음) | Rust 키워드라 생성 코드(`Signals` 필드)가 rustc에서 실패, 7b: `r#` 지원 예정 |
| signal: `$` 등 ASCII 비식별자 문자 | 거부 | Rust 식별자에 쓸 수 없는 문자 U+XXXX — rustc 실패, 7b: 맹글링 예정 |
| signal: 비ASCII 문자(한글, `café`, ZWJ, 결합 문자 등) | 거부(**문구 정정**) | "아직 지원하지 않습니다 — 7b에서 XID·NFC 판정과 함께 지원 예정(rustc가 받는 이름도 있지만 지금은 정확히 가릴 수 없어 거부)". "rustc 실패"라고 쓰지 않는다(테스트로 단언) |
| 컴포넌트: ASCII, 키워드 포함(`Self`) | **받음(키워드 검사 제거)** | — |
| 컴포넌트: 비ASCII | 거부(모든 모드) | `{C}Island` wasm-bindgen 내보내기 → wasm32 rustc E0754 |
| 컴포넌트: R46(c) 모듈 이름 충돌 | 변경 없음 | 기존 문구 |

**알고 택한 과잉 거부**: 비ASCII 컴포넌트 이름은 `{C}Island`를 내보내지 않는 Flatten·루트 단일 경로에서는 wasm32 `cargo check`를 통과한다
(F1 측정). 경로마다 규칙이 갈리지 않도록 모드 무관하게 거부하고, 문구에 그 사실을 적었다. JSX에서 비대문자 비ASCII 태그(`<카드/>`)는 R46과
별개로 lower의 `is_uppercase()` 규칙 때문에 HTML 태그로 해석된다(R24) — 별도 결정 사항.

**스크래치 교차 확인**(저장소 밖, rustc 1.98.1, wasm-bindgen 0.2.126, `cargo check --target wasm32-unknown-unknown`):
- (첫 시도의 XID·NFC 판정 컴파일러로 측정 — 7b 설계 근거) 그 컴파일러 산출물: signal `개수`·`café`·`μετρητής`·`计数`·ASCII 대조 × root·islands·flatten·single 20건, 컴포넌트 `Self`·`Arger` 8건 — **28/28 통과**.
- 컴포넌트 검사·NFC 검사를 끈 컴파일러: `Ärger` Island는 islands·single에서 **E0754 2건 + "items in `extern` blocks cannot use non-ascii
  identifiers" 1건으로 실패**(결함 존재), flatten·root는 통과, native 대상에서는 네 경로 모두 통과(wasm-bindgen 내보내기가 wasm32에서만
  생성되므로 — r46-research의 두 보고서가 엇갈린 이유). NFC·NFD `개수`가 함께 있는 루트는 native·wasm32 모두 E0124 1건 + E0062 2건.

**확인하지 못한 것**: 비ASCII `data-loom-id`·lref가 실제 브라우저의 `bootstrap.js` 파싱과 노드 찾기에서 동작하는지(구분자 `| , :`는 식별자에
들어갈 수 없어 문제없을 것으로 보지만 측정하지 않았다). SWC가 식별자 안의 `\uXXXX` 이스케이프를 풀어서 넘기는지.

### R25 완화 (2026-10-18, Phase 7a 후속 F2 — 병익님 결정)

7a C6은 최상위 빈 문장 `;`도 R25로 거부했고, 컴포넌트 본문의 `;`은 7a 이전부터 "unsupported statement"였다. 빈 문장은 의미가 없어 막는
결함이 없으므로 둘 다 받는다(R22 "return 뒤의 문장"에서도 빈 문장은 제외). 산출물이 `;` 없는 같은 소스와 4개 API 모두에서 바이트 동일함을
테스트로 단언했다(`f2_empty_statements`). 그 밖의 R25 거부(최상위 `const`/`class`/`export default` 등)는 그대로다.

