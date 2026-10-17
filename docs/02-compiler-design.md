# 02. 컴파일러 설계 — JSX/TSX → ECS Entity Graph

> **현재 상태 (2026-10-17 갱신)** — 이 문서의 원래 서술과 실제 구현이 갈리는 지점의 요약.
> 상세는 각 절의 "2026-10-17 정정"에 있고, 원래 서술은 지우지 않았다.
> - **PatchTable/`DomBinding`(정수 opcode 디스패치)은 구현되지 않았다.** 실제 구현은 값 타입별로
>   monomorphize된 제네릭 Rust System(`patch_text_system::<T>` 등)이 `DomWrite` 열거형 값을
>   `PendingDomWrites` 큐에 쌓는 방식이다(docs/07, `crates/loom-runtime/src/lib.rs` 헤더).
>   "행동은 정적 코드에, Component에는 데이터만"이라는 원리(CLAUDE.md 금지 4)는 지켜지고 있다.
> - **`GraphMode::Flatten`은 구현됐다**(2026-07-24, 560b645). RQ6 위젯-임베드 픽스처에서 Islands 팔과
>   실측 비교됐고 H-Qwik 확정(2026-07-28)의 근거가 됐다. 지금은 Flatten이 기본 모델이고 Island 분할은
>   선택적 최적화다(docs/04).
> - **`.map()`·`{cond&&}`의 "해소"는 "컴파일 타겟 기계가 있다"는 뜻일 뿐, RQ1 후보 채택이 아니다.**
>   리스트는 기본 산출물이 K(전면 재생성 대조군)이고 C/D/E(+LIS)는 트랙 N용 선택판으로만 낸다. RQ1은
>   열려 있다(트랙 L v22 판정도 "아직 채택 판정 아님"). 조건부 코드젠은 "첫 spawn 지연 + CSS 은닉"
>   (`SetVisible`, 정상 상태 토글은 트랙 T의 CSS 은닉 대조 arm과 같은 알고리즘, SSR-false 사이트의 첫 spawn 지연은 lazy arm의 앞 절반 — F 계열 아님)을 내는데, 트랙 T 판정은 "B 유지"
>   (2026-08-07)이고 CSS 은닉은 대조군이라(원칙 5) 채택되지 않았다 — **결정되지 않은 설계 긴장**으로 남아 있다
>   (아래 "컴파일 단계" 4번 정정).
> - 남은 미구현: 삼항 조건부, 조건부 안의 조건부, props 실제 resolve, Flatten의 시그널 이름 자동
>   재명명, `loomc` CLI(`main.rs`가 빈 `fn main() {}` — 컴파일러는 build.rs에서 라이브러리로만 호출됨).

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
   **2026-10-17 정정**: 실제 코드젠의 Entity 단위는 컴포넌트 인스턴스가 아니라 **시그널 선언
   하나**다 — 시그널마다 `Signal<T>`를 가진 Entity 하나를 spawn하고, 그 값을 표시하는 노드가 있으면
   `DomTarget`도 붙인다(표시 노드가 없는 시그널은 `Signal`만, 또는 marker와 함께 spawn된다 —
   `codegen.rs`의 `setup()`/`resume()` 생성부). 컴포넌트 인스턴스에 대응하는 단일
   Entity는 없다. 리스트 row와 조건부 내용은 그 아래 별도 Entity로 생긴다. Flatten(현 기본
   모델)에서는 페이지 전체가 World 하나다.
3. **Patch System 생성**: 각 dynamic binding site마다 컴파일러가 좁은 System 하나를 생성.
   `Query<Changed<Signal<T>>>` 형태로 게이팅되고, 참이면 저장된 DOM 노드 핸들에 직접
   `node.textContent = ...` 류의 최소 연산만 수행. **범용 diff 알고리즘이 존재하지 않음** —
   이게 No-VDOM의 실제 구현 지점.
   **2026-10-17 정정**: "사이트마다 System 하나를 생성"은 실제와 다르다. 텍스트·입력 binding은
   사이트별 System이 아니라 **binding 모양별 System 하나**(값 타입별로 monomorphize되는 제네릭
   `patch_text_system::<T>`, `Signal<String>` 고정인 `patch_input_system`)를 등록하고, 그 System이
   해당 타입의 모든 사이트 Entity를 `Changed<Signal<T>>`로 걸러 처리한다. 사이트별로 생성되는 것은 리스트·조건부·row mirror
   sync 같은 제어 흐름 System이다. DOM 연산은 바로 실행하지 않고 `DomWrite` 값으로
   `PendingDomWrites`에 쌓였다가 `LoomApp::tick`이 일괄 적용한다. "범용 diff 없음"은 그대로
   성립한다(갱신된 값이 그대로 `SetText`로 내려감).
4. **제어 흐름 특수형**: `{items.map(...)}`, `{cond() && <X/>}` 등은 범용 System으로
   표현 불가능(엔티티 개수/존재 자체가 동적). **`.map()`은 RQ1 트랙 L 후보
   K/C/D/E(+LIS 변형)로, `{cond() && <X/>}`는 RQ1 트랙 T 후보 F로 각각 해소됨
   (2026-07-24)** *(2026-10-17 주: 여기 "후보 F"는 Phase 1 이름표다 — 실제로는 CSS 은닉 보존 변형
   `cond_reconcile_f_system`의 제어 흐름이고, 프로토콜상 CSS 은닉 대조 arm에 해당한다. F 계열 F-i/ii/iii는
   2026-07-31에 따로 생겼다.)* — 단 후자는 범용 System을 파라미터화하는 대신, 사이트마다
   F의 제어 흐름을 복제해 실제 정적 콘텐츠를 인라인한 전용 함수를 코드젠이
   생성하는 방식(`crates/loom-compiler/src/codegen.rs`의 `CondBinding` 처리
   참고). **삼항 조건부(`cond ? <A/> : <B/>`)는 여전히 미착수** — 두 개의
   `{cond&&}` 바인딩으로 desugar 가능하다는 설계만 있고 구현은 후속 과제.

   **2026-10-17 정정 — "해소됨"의 범위.** 위의 "해소됨"은 컴파일러가 그 형태를 실제로
   lowering할 수 있게 됐다는 뜻으로만 유효하다. RQ1의 후보 **채택**이 정해졌다는 뜻이 아니고,
   RQ1은 2026-10-17 현재 열려 있다.
   - **리스트**: 기본 경로(`compile_tsx_to_rust`·`compile_tsx_to_rust_graph`·
     `compile_tsx_to_single_island_rust` — TSX를 컴파일하는 build.rs 중 `loom-track-n`을 뺀 전부가
     이 셋 중 하나를 부른다)는
     **K**(RQ1의 대조군)를 낸다(`codegen.rs`의 `ListCandidate` 문서). K는 2026-09-22(e6bb644)부터 §10
     디스패치 네 분기 — 완전 동등 no-op·append·truncate를 먼저 보고 그 외만 전량 재생성 — 를 가진다
     (`loom-runtime/src/lib.rs`의 `KDispatch`). C/D/D-LIS/E/E-LIS는
     2026-08-08에 생긴 `compile_tsx_to_rust_with_candidate`로만 낼 수 있고, 트랙 N(정합성
     property test)용 **네이티브 전용**이다 — 비-K 산출물의 `resume()`은 후보별 상태 복구가 없어
     일부러 즉시 실패한다. 어느 후보를 기본으로 채택할지는 RQ1 트랙 L 판정에 걸려 있다. 그
     판정은 개정 #9(2026-08-12)로 이전 수집분이 전부 무효화된 뒤 v22로 재수집 중이다. Chromium
     v22 판정(f6c1bdb)도 "아직 채택 판정 아님"이다. 남은 것은 G0 구현 재심사·G1 판정 기록·D 기각
     (iii) 뒷항 미계측·D W5 부속의 opt-out 경로 명세·C의 D·E 자격 종속(C-4)·§11-1(b)/v21 재검토·
     Firefox BiDi 재수집 등이다(`research-questions/RQ1-dynamic-control-flow-scheduling.md` v22 절
     "채택까지 남은 것", `research-questions/data/verdicts/rq1-track-l-v22-*.txt`). 판정문의 "범위 밖:
     §5 FV 계류" 줄은 판정기 고정 문자열이 남긴 표류다 — §5 FV 계류는 2026-08-11에 소진됐다(RQ1
     2026-10-17 정리). (이 항목의 첫 판은 같은 날 "§5 FV 계류 등 남음, RQ1 v21 절"이라고 적었으나 RQ1이
     v22 절을 판정 근거로 정리하면서 고쳤다.)
   - **조건부 — 결정되지 않은 설계 긴장**: 컴파일러가 내는 조건부는 **첫 spawn 지연 + CSS 은닉**이다.
     첫 `true`에 내용을 spawn하고, 이후 토글은 앵커에 `DomWrite::SetVisible` 하나만 내며, 숨긴 동안에도
     patch System이 게이팅 없이 돈다(`codegen.rs` "Conditionals" 절, `cond.rs`의 `cond_reconcile_f_system`
     — CSS 은닉 보존 변형). 프로토콜 §10이 CSS 은닉 arm의 정의적 특성으로 꼽는 바로 그 동작이고, F 계열
     (F-i/ii/iii)의 장치는 없다. RQ1 트랙 T의 최종 판정(2026-08-07, v2 캠페인 44시간)은 **"B 유지,
     F-i/ii/iii 기각"**이고(RQ1 "트랙 T v2 캠페인" 절), CSS 은닉은 대조군이라(원칙 5) 판정이 채택하지 않았다 — §9 K-3은 "CSS가 F를 지배하더라도 자동 채택은 아니다"라는 보강 조항이고, v2에서는 그 지배도 깨졌다(K-3 (1) 미발동)
     — 채택하려면 형제 셀렉터 참여·SSR HTML 바이트 반박에 대한 답을 RQ1에 기록해야 한다. 그 기록도, 코드젠을
     B로 바꾸자는 결정도 2026-10-17 현재 없다. (2026-10-17 재정정: 같은 날 첫 판은 이것을 "F 계열/F 기반"이라 적었다. 코드젠에는 F 계열의 장치 — fragment 분리, `CondEffective`, `Frozen` 게이팅, catch-up — 가 하나도 없고, `cond.rs`가 "F"라고 부르는 `cond_reconcile_f_system`은 스스로 "CSS-hide preserve variant"이자 프로토콜의 CSS 은닉 대조 arm이라고 밝힌다. Phase 1(07-24) 시절의 이름표를 그대로 옮긴 오류였다.) 반대 방향의 압력도
     있다: 트랙 N 불변식 4의 진짜 형태(동결 중 catch-up)를 재려면 컴파일러가 F 계열(fragment
     분리 + `Frozen` 게이팅)을 낼 수 있어야 한다(RQ1 트랙 N 사이클 1 "이 통과가 말하지
     않는 것" 2번). 벤치 하네스의 R2 중첩 조건부는 판정대로 B arm을 쓴다(`loom-runtime/src/lib.rs`).
     **이 문서는 이 긴장을 판정하지 않는다** — RQ1에서 결정할 것.
   - **조건부 안의 조건부**는 아직 거부된다(`lower.rs`의 `CondBinding` 중첩 검사). 조건부 안의
     정적 마크업·`{signal()}`·`{signal().map(...)}`은 2026-08-07부터 된다(`codegen.rs` "중첩" 절).

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

**2026-10-17 정정 — PatchTable/`DomBinding`은 구현되지 않았고, resumability는 그것 없이 성립했다.**
- Phase 1이 정수 opcode 대신 **Rust 제네릭을 정적 디스패치 수단**으로 썼다. `patch_text_system::<T>`가
  `T`별로 컴파일타임에 monomorphize되고, Component에는 클로저도 함수 포인터도 없다(`crates/loom-runtime/
  src/lib.rs` 헤더 "PatchTable, deferred to Phase 2", `roadmap/PHASES.md` Phase 1 축소 항목, docs/07
  용어표). `DomBinding`/`patch_op`라는 타입은 코드 어디에도 없다.
- 그 헤더는 "Phase 2가 LRef를 도입하면 opcode형 `DomBinding`이 다시 필요해진다"고 예상했다. 그런데
  Phase 2 resumability(LRef, `resume()`)는 이 형태 없이 구현됐다. 직렬화 경계를 넘는 것은 시그널
  **값**(캡처)과 `chunk|symbol`뿐이고, 행동은 컴파일러가 생성한 `Action` 열거형 + `apply_action`/
  `resume_action_by_symbol`의 정적 `match`, 그리고 `resume()`이 다시 등록하는 제네릭 System에 남는다.
  즉 "행동은 정적 코드에, '어느 데이터에 적용할지'만 데이터에"라는 **원리**는 지켜졌다. 다만 그
  데이터를 Component 안의 opcode로 표현할 필요가 지금까지 생기지 않았다. 따라서 위 문단의
  "전제조건"은 원리 수준에서만 맞고, PatchTable이라는 특정 형태에 대해서는 실측으로 반박됐다.
- 남은 질문(미해결, 결정 기록 없음): 제네릭 System 방식으로 충분한지, 아니면 PatchTable 형태가
  필요해지는 경우(예: 런타임에 binding 종류가 데이터로 결정돼야 하는 경우)가 있는지. PatchTable을
  현행처럼 부르는 서술은 다른 문서에도 있었고, 모두 같은 날 정정이 달렸다 — docs/03(판정례 7·8),
  docs/08 "새로 설계한 것" #2, RQ1 프로토콜 §2.1 "공유 런타임 조각"(append-only라 본문은 그대로 두고
  말미 "문서 표류 정정 둘 (2026-10-17)"에 등재).

**PatchTable 결정 (2026-10-17, 병익님 결정 — 위 "남은 질문"의 종결)**
- **정본**: 행동은 컴파일러가 생성한 정적 코드에 둔다. 수단은 (a) 값 타입별로 monomorphize된 제네릭 System
  (`patch_text_system::<T>`, `patch_input_system` 등)과 (b) 닫힌 열거형의 정적 `match`다 — 이벤트 쪽은 생성된
  `Action` 열거형 + `apply_action`/`resume_action_by_symbol`, DOM 쪽은 `DomWrite` 열거형의 단일 디스패치 지점,
  row 파생값은 닫힌 IR 열거형(`RowPredicate`/`RowSlice`). Component와 직렬화 페이로드에는 데이터와 심볼만 둔다.
  위 "해법 — PatchTable 패턴" 문단의 **원리**("행동은 정적 코드에, 어느 데이터에 적용할지만 데이터에")는
  유지되고, 그 원리의 **형태**가 정수 opcode 테이블에서 이것으로 바뀐다.
- **근거**: (1) Phase 2 resumability가 opcode 없이 구현됐다(직렬화 경계는 위 열거형으로 이미 닫힘). (2) RQ1
  트랙 L(v21·v22)·T·N과 RQ6 H-Qwik 확정의 모든 측정이 이 경로 위에서 나왔다 — 정본을 실측 기반과 일치시키는
  것이라 재수집이 필요 없다. (3) opcode 형태를 지지하는 측정 데이터는 없다. 반대로 전환하면 RQ1 측정 기반
  전체(§2.5 공유 기반, 대기 중인 Firefox BiDi 재수집 포함)를 다시 세워야 한다.
- **재개 조건**: binding 종류가 **런타임 데이터로** 정해지는 기능(예: 동적 컴포넌트 선택, row마다 모양이 다른
  이종 리스트)이 명세되면 이 결정을 다시 연다. 1차 후보는 닫힌 `BindingKind` 열거형 Component + 그것을 `match`하는
  System 하나(CLAUDE.md 금지 4를 지키며 Bevy 타입 쿼리·접근 선언 스케줄링을 유지), 정수 opcode 테이블은 2차 후보다.
  사이트 수에 따른 monomorphize 비용(RQ1 M3, 미계측)이 유의하게 크다고 실측되면 그때도 다시 연다.
- **하지 않은 것(숨기지 않음)**: opcode 경로와 제네릭 경로를 같은 셀에서 비교하는 격리 스파이크는 선택 사항이라
  하지 않았다. "PatchTable로 바꾸면 후보 간 비율이 바뀌는가"(RQ1 프로토콜 말미 정정 2)는 여전히 미측정이다.
- 이 결정은 "새로 설계한 것" 목록(docs/08 #2)의 PatchTable을 고유 설계에서 **설계 스케치(미채택)**로 확정한다.

**2026-10-17 — 저작 표면 전수 감사**: 지원·거부·부재 목록과 조용한 오컴파일 47건은 `docs/09-authoring-surface-inventory.md`에
있다. 이 문서의 "해소됨" 항목들은 "그 구문의 lowering 경로가 생겼다"는 범위에서만 맞고, 그 경로 안에도 조용한 결함이 있다
(예: JSX 텍스트 `trim()`으로 단어 사이 공백 소실 — RQ6 위젯-임베드 픽스처에서 실제로 발생).

## 왜 직접 파서를 안 짜는가

JSX/TSX 파싱 자체는 이미 여러 번 잘 풀린 문제(SWC, Babel). 여기서 리서치 가치가 있는 부분은
파싱이 아니라 "파싱된 AST를 ECS entity graph + patch system으로 lowering하는 규칙"임.
SWC(Rust 기반, 병익님 기존 툴체인과 자연스럽게 맞물림)를 프론트엔드로 채택하고 커스텀 lowering
pass만 새로 작성하는 게 맞음.

## 미해결

- ~~제어 흐름 특수형의 정확한 컴파일 타겟 (RQ1)~~ → **`.map()`/`{cond&&}` 둘 다
  해소됨(2026-07-24, 위 "컴파일 단계" 4번 참고)**. 남은 것: 삼항 조건부(미착수).
  **2026-10-17 정정**: 취소선은 "lowering 경로가 생겼다"는 범위에서만 맞다. **어느 후보를
  컴파일 타겟으로 채택할지는 RQ1에서 여전히 미결이다** — 리스트 기본 산출물은 대조군 K이고,
  조건부 코드젠(첫 spawn 지연 + CSS 은닉 — F 계열 아님)은 트랙 T 판정(B 유지)과 어긋난 채 결정이 없다.
  상세는 "컴파일 단계" 4번의 2026-10-17 정정. 남은 것에 **조건부 안의 조건부**(lower.rs가
  거부)를 더한다. 삼항 조건부는 2026-10-17 현재도 미착수다(lower.rs에 처리 경로 없음).
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
  **2026-10-17 확인**: 두 조건 모두 그대로다. `ClickAction` variant 다섯 개
  (`Increment`/`Append`/`Rotate`/`Toggle`/`SetString`)가 전부 시그널 하나만 참조하고,
  props 실제 resolve가 아직 없어 요청별 초기값이 들어올 경로도 없다. 덧붙여 2026-08-08
  (Task #28)에 관련 형식화가 진행됐다. Lean4 `ResumabilitySoundness`가 "시그널 하나당
  writer 액션 하나"(`SingleWriter`)를 가정으로 건전성을 증명했고 DOM 복구형
  (StringList/Bool/Str)으로 확장됐다(#28-e). `lower.rs`의 `check_one_action_per_signal`이
  그 가정을 **i32 시그널에 한해** 정적으로 강제한다(#28-f, RQ5). 주의할 점: 이것은 위 조건
  (1)("액션 하나당 시그널 하나")과 방향이 다른 별개 제약이다. 조건 (1) 자체는 정적 검사가
  아니라 IR의 표현력 한계로 보장된다.
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
  **2026-10-17 정정 — "의도적으로 남긴 범위" 세 항목의 현재 상태**:
  - `GraphMode::Flatten`: **구현됐다**(2026-07-24, 560b645 — 같은 날 이 문단 뒤에 들어온
    커밋이라 이 문단에는 반영되지 않았다). `flatten_graph`가 그래프의 모든 컴포넌트를
    `LoomApp`/`World`/`Signals`/`Action` 하나로 인라인한다. RQ6 위젯-임베드 픽스처(5개
    컴포넌트)를 같은 소스로 Islands/Flatten 양쪽 컴파일해 실측했고, 그 결과로 **H-Qwik 확정
    (2026-07-28)**이 났다(docs/04 "실측이 확인한 것"). 이후 `generate_flatten_entry_body`가 브라우저
    진입점(`ResumedApp`)까지 생성해, 크레이트 세 곳의 손코딩 진입점을 흡수했다(`codegen.rs`
    문서). 남은 스코프 컷: Flatten은 **그래프 전체에서 시그널 이름이 전역 유일**해야 하고
    자동 재명명은 없다(위반 시 컴파일 패닉).
  - Island당 별도 `.wasm`: **부분 이행**(2026-07-25, 8f34a6a, Task #27). `compile_tsx_to_single_
    island_rust`가 Island 하나만 컴파일한다. 다만 Island마다 크레이트를 손으로 하나씩 둔다
    (`crates/loom-widget-embed-island-*` 4개, 각자 build.rs). 범용 빌드 툴링은 여전히 없다.
    RQ6 확정 이후 Island 분할은 "선택적 최적화"이므로(docs/04·07) 이 툴링의 우선순위도 그에
    맞춰 낮아졌다.
  - props 실제 resolve: **여전히 미구현**. `PropLiteral`은 파싱되지만 codegen이 쓰지 않는다.
    `specialize_component_refs`는 `ir.rs` 주석에만 이름이 나오고 함수로 존재하지 않는다.
  - 이 항목 전체의 전제("Island = 컴파일러가 반드시 지원할 단위")는 RQ6 확정으로 바뀌었다.
    `GraphMode::Islands`는 폐기되지 않고 세분화를 선택할 때 쓰는 경로로 남는다(docs/04).

*(2026-10-17 부기) 이 안전성 논증에는 세 번째 전제가 숨어 있었다: (3) 페이지당 resume이 SSR 직후 한 번뿐이라는 것. `demo/resume/bootstrap.js`는 lref 요소마다 resume하므로(docs/09 R13) 실행 중 바뀐 값을 리터럴로 덮어쓴다. `bootstrap-flat.js`는 인스턴스를 공유해 이 전제를 지킨다.*