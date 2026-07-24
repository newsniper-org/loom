# 02. 컴파일러 설계 — JSX/TSX → ECS Entity Graph

## 핵심 원칙

React의 `createElement` 방식(런타임에 tree 객체를 만들고 diff)은 채택하지 않음. 대신
Solid의 컴파일 전략처럼, JSX 트리를 컴파일타임에 **static 부분**과 **dynamic binding site**로
분해해서 직접 호출로 컴파일함. 차이는: Solid는 Signal(클로저 기반)을 반응성 원자로 쓰지만,
여기서는 **ECS Component**를 반응성 원자로 씀 — 이유는 03에서 다루는 No-VDOM 스케줄링과
04에서 다루는 resumability 직렬화 둘 다에 필요하기 때문.

## 컴파일 단계

1. **정적 분석**: JSX 트리를 순회하며 각 노드/표현식을 분류
   - 순수 정적 마크업 → SSR용 HTML 템플릿 문자열로 직접 방출 (client에 아무 코드도 안 감).
     **2026-07-24 정정**: 이 문장은 처음부터 있었지만 Phase 1 실제 코드젠은 최근까지
     이걸 안 지켰다 — 정적 엘리먼트조차 동적 바인딩과 똑같이 `DomOps::create_element`/
     `append_static_text` 런타임 호출을 노드 하나당 하나씩 냈다(문서 따로, 구현 따로
     — 조용히 방치하지 않고 이번에 실제로 맞춤). 지금은 `codegen.rs`의
     `try_render_fully_static`이 액션도 없고 동적 바인딩도 없는 최대 정적 서브트리를
     찾아 **컴파일타임에 하나의 HTML 문자열로 미리 렌더**하고, `DomOps::
     append_static_html` 호출 하나로 방출한다 — 이 문장이 원래 말한 그대로.
     RQ6 §4.5.1(M11, ISR 부분 재생성 측정)에서 실측한 효과: 정적 구간 하나당
     런타임 비용이 대략 7~14배 감소(하지만 0으로 가진 않음 — 여전히 O(n), Island의
     "그 바이트를 아예 안 건드림"과는 질적으로 다름, `research-questions/data/
     rq6-m11-isr-partial-regen.json` 참고).
   - 동적 바인딩 지점(interpolation, 속성, 조건부, 리스트) → binding site로 등록
   - 이벤트 핸들러 → LRef(지연 참조, 04 참고)로 변환
2. **Entity 그래프 생성**: 각 컴포넌트 인스턴스 = World 안의 Entity spawn 호출 하나.
   컴포넌트의 로컬 상태 선언(예: `signal()`) = 그 Entity에 붙는 `Signal<T>` Component.
3. **Patch System 생성**: 각 dynamic binding site마다 컴파일러가 좁은 System 하나를 생성.
   `Query<Changed<Signal<T>>>` 형태로 게이팅되고, 참이면 저장된 DOM 노드 핸들에 직접
   `node.textContent = ...` 류의 최소 연산만 수행. **범용 diff 알고리즘이 존재하지 않음** —
   이게 No-VDOM의 실제 구현 지점.
4. **제어 흐름 특수형**: `{items.map(...)}`, `{cond() && <X/>}` 등은 범용 System으로
   표현 불가능(엔티티 개수/존재 자체가 동적). **`.map()`은 RQ1 트랙 L 후보
   K/C/D/E(+LIS 변형)로, `{cond() && <X/>}`는 RQ1 트랙 T 후보 F로 각각 해소됨
   (2026-07-24)** — 단 후자는 범용 System을 파라미터화하는 대신, 사이트마다
   F의 제어 흐름을 복제해 실제 정적 콘텐츠를 인라인한 전용 함수를 코드젠이
   생성하는 방식(`crates/loom-compiler/src/codegen.rs`의 `CondBinding` 처리
   참고). **삼항 조건부(`cond ? <A/> : <B/>`)는 여전히 미착수** — 두 개의
   `{cond&&}` 바인딩으로 desugar 가능하다는 설계만 있고 구현은 후속 과제.

## 핵심 설계 결정: Component는 반드시 POD(plain data)여야 함

문제: Patch System은 "이 DOM 노드를 어떻게 갱신할지"라는 **행동**을 알아야 하는데,
행동(클로저/함수 포인터)을 Component에 그대로 저장하면 04의 resumability 직렬화가 깨짐
(클로저는 직렬화 불가능하거나, 가능하더라도 Qwik처럼 별도의 무거운 캡처 메커니즘이 필요해짐).

**해법 — PatchTable 패턴**:
- Component는 순수 데이터만 가짐: `DomBinding { patch_op: u16, args: SmallVec<ComponentRef> }`
- `patch_op`는 정수 opcode일 뿐, 실제 patch 함수는 **컴파일러가 생성한 정적 디스패치 테이블**
  (하나의 큰 match/switch, 모듈 로드 시점에 고정)에 들어있음
- 즉 "행동"은 코드(정적, 모듈에 컴파일되어 있음)에 남고, "이 행동을 어느 데이터에 적용할지"만
  Component(데이터)에 남음 — QRL이 "청크 URL + 심볼 이름"으로 이벤트 핸들러 단위를 참조하는 것과
  같은 원리를, 이벤트 핸들러보다 훨씬 세분화된 "patch 연산" 단위에도 적용한 것

이 결정이 이 컴파일러 설계의 핵심 아이디어이자, 04의 resumability 설계가 성립하기 위한 전제조건임.

## 왜 직접 파서를 안 짜는가

JSX/TSX 파싱 자체는 이미 여러 번 잘 풀린 문제(SWC, Babel). 여기서 리서치 가치가 있는 부분은
파싱이 아니라 "파싱된 AST를 ECS entity graph + patch system으로 lowering하는 규칙"임.
SWC(Rust 기반, 병익님 기존 툴체인과 자연스럽게 맞물림)를 프론트엔드로 채택하고 커스텀 lowering
pass만 새로 작성하는 게 맞음.

## 미해결

- ~~제어 흐름 특수형의 정확한 컴파일 타겟 (RQ1)~~ → **`.map()`/`{cond&&}` 둘 다
  해소됨(2026-07-24, 위 "컴파일 단계" 4번 참고)**. 남은 것: 삼항 조건부(미착수).
- ~~폼 입력 양방향 바인딩~~ → **해소됨(2026-07-24)**: `<input value={s()}
  onInput={(e) => s.set(e.target.value)}/>`가 `JsxNode::InputBinding`으로
  실제로 컴파일됨. 재개(resume)는 캡처가 아니라 `DomOps::get_input_value`로
  DOM에서 직접 현재 값을 읽어옴(리스트/조건부 재개와 같은 원리) — 이유는
  캡처 방식으로는 원리적으로 불가능: SSR 시점엔 사용자가 아직 아무것도 안
  쳤으므로 "미래에 타이핑할 값"을 캡처에 실을 수 없음. `SetString` 액션은
  기존 `Action`/`apply_action`/`resume_action_by_symbol` 기계를 그대로
  재사용하되, 유일하게 그 심볼만으로 결정되지 않는 액션이라
  `resume_action_by_symbol`이 `string_arg` 런타임 인자를 새로 받음(다른 모든
  액션은 무시) — `crates/loom-compiler/src/codegen.rs` 모듈 문서 참고.
- ~~LRef 캡처 시 "이 핸들러가 참조하는 최소 Component 집합"을 정적으로 어떻게
  계산할지~~ → **현재 IR 범위에서 해소됨(2026-07-24)**: `ClickAction` 각 variant가
  정확히 시그널 하나만 참조하고 `apply_action`의 해당 match arm도 그 시그널만
  건드리므로, 최소 캡처 집합 = "그 액션이 참조하는 시그널 하나"(i32면 캡처,
  StringList면 캡처 없음 — 리스트는 DOM에서 값을 복구하므로)로 정확히 계산
  가능함을 확인 — `crates/loom-compiler/src/codegen.rs`의
  `format_i32_capture_for_action`. **단, 이 결론은 두 가지에 의존한다**: (1)
  현재 IR이 액션 하나당 시그널 하나만 참조하는 형태로 제한돼 있다는 것(제어
  흐름/여러 시그널을 동시에 건드리는 핸들러가 추가되면 재검토 필요), (2)
  캡처에서 빠진 시그널은 `resume()`에서 리터럴 선언 초기값으로 폴백하는데,
  이게 안전한 이유는 Phase 1 IR이 시그널의 실제 첫 렌더 값이 선언 리터럴과
  달라지는 경우를 아예 지원하지 않기 때문 — 만약 IR이 요청별로 다른 초기값
  (예: 로그인 사용자별 카운터 시작값)을 지원하게 되면, 이 폴백 안전성이
  깨지고 최소 캡처 계산을 다시 봐야 함(`codegen.rs`의 해당 함수 문서에 상세
  기술). 이 두 조건 중 하나라도 바뀌면 RQ로 승격해 재검토할 것.
- ~~컴파일러 Island 통합 (Phase 3 증분 4)~~ → **해소됨(2026-07-24, Task #25)**:
  `ir.rs`에 `ComponentGraph`/`IslandActivation`/`PropLiteral`/
  `JsxNode::ComponentRef` 추가, `lower.rs`가 이제 모듈 안의 모든 top-level
  함수를 스캔(기존엔 첫 export만 쓰고 나머지를 진단 없이 버리는 실제 버그가
  있었음 — 이번에 같이 고침)하고 `client:load`/`client:visible`(SWC가 이미
  `JSXNamespacedName`으로 올바르게 파싱하던 것을 lower.rs가 그동안 무조건
  거부하고 있었을 뿐)을 파싱. `codegen.rs`의 `generate_islands`가 Island로
  지정된 각 컴포넌트를 `pub mod <name>_generated { ... }`로 네임스페이싱하고
  (컴파일러가 내는 모든 식별자가 원래 전역 스코프였다는, 문서에 없던 진짜
  블로커였음 — 루트 컴포넌트는 하위호환 위해 계속 무네임스페이스), 호스트
  엘리먼트에 `data-loom-island`/`-island-id`/`-activation`/`-chunk`/`-ctor`
  5개 속성을 방출하며, Island 진입점(`#[wasm_bindgen(constructor)]`)은
  기존 `Action`/`apply_action`/`resume_action_by_symbol`/`resume` 범용
  디스패치 기계를 그대로 재사용(손코딩 Island처럼 별도 클로저 배선 방식을
  새로 만들지 않음)하되 DOM 탐색은 `loom_runtime::web::own_tagged_descendants`
  (opaque `[data-loom-island]` 경계 존중, `ResumedComponent::new`의 페이지
  전역 쿼리 버그를 재생산하지 않음)로 스코프를 좁힘. 실제 컴파일 중 발견한
  두 개의 진짜 버그: (1) 2단계 이상 중첩된 Island 참조는 `super::` 경로
  접두사가 필요함(모든 Island 모듈이 JSX 중첩 깊이와 무관하게 flat sibling
  이라서) — `CodegenCtx::wrapped_in_own_module`로 해결. (2) 액션이 0개인
  컴포넌트(`ComponentRef`만 있는 루트처럼)의 `enum Action {}`에 대해
  `match action {}`이 Rust에서 예외적으로 exhaustive로 인정 안 됨("references
  are always considered inhabited") — `_ => {}` 폴백 arm 추가로 해결.
  `demo/islands/product-card.tsx` + `crates/loom-island-compiled` +
  `demo/islands/compiled/index.html`로 실제 컴파일러 산출물이 기존
  손코딩 데모(`demo/islands/index.html`)와 동일한 시나리오(중첩 Island
  활성화, opaque 스코핑, 클릭 디스패치, detach)를 Playwright로 재현함을
  확인. **의도적으로 남긴 범위**: `GraphMode::Flatten`(H-Qwik 반사실
  모양, Task #26 몫)은 시그니처만 있고 미구현, Island당 별도 `.wasm` 산출물
  빌드 툴링(여전히 build.rs 수준 문제, 이번 라운드는 크레이트 하나에 여러
  Island를 한꺼번에 컴파일), props 실제 resolve(`specialize_component_refs`)
  는 파싱/검증만 되고 구현은 스트레치로 남김(인수 조건 픽스처가 props 없이도
  충족됨). 상세: `research-questions/RQ6-verification-plan.md`/
  `roadmap/PHASES.md` Phase 3 절.
