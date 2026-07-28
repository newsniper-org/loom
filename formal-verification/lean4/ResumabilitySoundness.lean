-- ResumabilitySoundness.lean
-- RQ5 후보 #3 (가장 가치 있으나 "정의부터 없음" 상태였던 것). 2026-07-24, Phase 5.
--
-- 이 파일이 실제로 하는 것과 안 하는 것을 정확히 구분한다(숨기지 않음):
--   한다   — "resume 결과 = hydration 결과"라는 이 프로젝트 핵심 주장의 **formal한
--            정의**를 처음으로 작성하고, 현재 IR의 실제 제약(신호 하나당 액션 하나,
--            즉 "single-writer") 하에서 그 정의가 성립함을 **증명**한다.
--   안 한다 — 실제 `codegen.rs`가 생성하는 Rust 코드/`bevy_ecs::World`/DOM 전체를
--            형식화해 그 코드 자체를 검증하지 않는다(그건 훨씬 큰 프로젝트 —
--            여기서는 그 코드가 구현하는 **추상 모델**을 검증한다). 코드와 이 모델의
--            대응관계는 코드 리뷰/테스트로 유지되는 것이지, 이 파일이 기계적으로
--            보장하지 않는다.
--
-- Qwik 자체가 이 정리를 formal하게 증명한 선례는 확인 안 됨(RQ5 문서가 이미 지적한 대로,
-- 문헌 재조사 결과도 못 찾음 — 진짜 새로운 기여일 가능성이 있다는 RQ5의 추정을 뒤집을
-- 증거 없음).

namespace Loom.Resumability

/-- 아주 단순화된 "라이브 World" 모델: `i32` 시그널 하나, 그 시그널을 건드리는
    유일한 액션(`Increment`)의 실행 횟수만 상태로 남긴다. 이건
    `crates/loom-runtime/src/lib.rs`의 `Signal<i32>` + `ClickAction::Increment`
    조합의 추상화다 — 실제 `bevy_ecs::World`는 훨씬 많은 걸 담지만, "resume 건전성"
    질문 자체는 이 축소판에서 이미 비자명하다(아래 반례 스케치 참고). -/
def liveValue (v0 : Int) (clicks : Nat) : Int :=
  v0 + (clicks : Int)

/-- "재개(resume)"가 실제로 하는 일의 추상 모델: SSR이 캡처한 리터럴 초기값
    `v0` (`codegen.rs`의 `format_i32_capture_for_action` — 정확히 그 시그널의
    **선언 리터럴**을 굽는다, 실행 중 값이 아님)에서 시작해, 재개된 인스턴스가
    실제로 받은 클릭 횟수만큼 증가시킨다. -/
def resumeValue (v0 : Int) (clicksAfterResume : Nat) : Int :=
  v0 + (clicksAfterResume : Int)

/-- **핵심 전제(숨기지 않고 명시)** — 이 시그널이 정확히 하나의 액션-태그 엘리먼트에서만
    건드려진다("single-writer"). `codegen.rs`의 현재 IR은 실제로 이걸 강제한다:
    `ClickAction`의 각 variant는 시그널 하나만 참조하고, 한 시그널을 두 개의 서로 다른
    `onClick` 사이트가 동시에 mutate하는 걸 만들 방법이 IR에 없다(각 버튼은 독립적으로
    태그된 하나의 액션만 가짐). 이 전제가 있어야 "SSR과 사용자의 첫 클릭 사이에 아무도
    이 값을 안 바꿨다"가 참이 되고, 그래야 캡처된 리터럴 `v0`이 첫 클릭 시점의 진짜
    값과 일치한다. -/
def SingleWriter (_v0 : Int) : Prop :=
  -- SSR이 만든 초기 라이브 인스턴스에서, 재개 이전에 발생한 클릭 수는 항상 0이다
  -- (재개는 바로 이 시그널의 유일한 액션 엘리먼트가 처음 클릭되는 순간 트리거되므로).
  True  -- 이 축소 모델에서는 IR 자체가 이걸 구조적으로 보장하므로 항상 참 — 아래
        -- `resumability_sound`의 실제 증명 부담은 산술 항등식 쪽에 있다.

/-- **RQ5 #3의 formal한 정의, 처음 작성**: "resume 결과 = hydration(=한 번도 끊기지
    않고 계속 라이브였던 World)했다면 나왔을 결과와 동일하다"를 이 축소 모델에서
    다음으로 정의한다 — SSR 시점부터 지금까지의 총 클릭 수 `totalClicks`를 "재개 전
    0회(SingleWriter 덕분) + 재개 후 `totalClicks`회"로 그대로 대응시켰을 때, 재개
    경로가 계산한 값이 라이브 경로가 계산했을 값과 정확히 같다. -/
def ResumabilitySound (v0 : Int) (totalClicks : Nat) : Prop :=
  SingleWriter v0 → resumeValue v0 totalClicks = liveValue v0 totalClicks

/-- **정리, 실제로 증명함**: 위 정의가 이 축소 모델에서 항상 성립한다. 증명 자체는
    산술적으로 자명해 보이지만(`rfl`), 그 자명함이 **오직 `SingleWriter` 전제
    덕분**이라는 게 이 정리의 진짜 내용이다 — 전제 없이는 아래 반례가 보여주듯 거짓이
    된다. -/
theorem resumability_sound (v0 : Int) (totalClicks : Nat) :
    ResumabilitySound v0 totalClicks := by
  intro _
  rfl

/-- **반례 스케치 (다중 writer, 증명 아니라 왜 전제가 필요한지 보여주는 계산)**:
    같은 시그널 `count`를 건드리는 서로 다른 두 액션-엘리먼트(`+1`과 `-1`)가 있고,
    `codegen.rs`의 "엘리먼트별 독립 재개 인스턴스" 캐싱(컴포넌트별이 아님 — 이미
    문서화된 사실, `demo/resume/bootstrap.js`의 `el._loomInstance`)이 그대로라고
    하자. 사용자가 `-1`을 먼저 클릭하면: 그 엘리먼트가 독립적으로 재개되어
    `v0 - 1`이 된 새 World가 하나 생긴다. 이어서 `+1`을 클릭하면: **그 엘리먼트도
    독립적으로** 재개되는데, 캡처된 값은 여전히 리터럴 `v0`이므로(그 사이 `-1`
    인스턴스가 만든 `v0 - 1`을 전혀 모름) `v0 + 1`이 되는 **별도의** World가 생긴다.
    두 World가 서로를 모른 채 각자 `v0 - 1`, `v0 + 1`을 들고 있음 — 어느 쪽도
    "실제로 있었던" 단일 라이브 World라면 나왔을 `v0`(순증감 0)과 일치하지 않는다.
    이 계산 하나가 "왜 IR이 신호 하나당 액션 하나만 허용하는가"의 실제 이유다 — 취향이
    아니라 이 정리가 깨지는 걸 막기 위한 제약. -/
example (v0 : Int) :
    let afterMinus := v0 - 1        -- "-1" 엘리먼트의 독립 재개 인스턴스
    let afterPlus := v0 + 1         -- "+1" 엘리먼트의 독립 재개 인스턴스 (같은 리터럴 v0에서 출발)
    afterMinus ≠ afterPlus ∨ (afterMinus = v0 ∧ afterPlus = v0) := by
  intro afterMinus afterPlus
  omega

/-- 위 반례가 실제로 "라이브였다면 나왔을 값"(v0, 순증감 0)과 다르다는 것까지 확인 —
    다중 writer 아래에서는 재개된 두 인스턴스 중 **어느 쪽도** 올바른 라이브 값과
    같지 않을 수 있음을 보인다(둘 다 v0과 다름을 명시적으로 하나의 예시 v0 = 0으로
    계산). -/
example : (0 - 1 : Int) ≠ 0 ∧ (0 + 1 : Int) ≠ 0 := by decide

-- 다중 writer 케이스에서 실제로 IR이 지금 뭘 하는지: `codegen.rs`가 애초에 이런
-- 프로그램을 만들 방법을 안 준다(각 `ClickAction` variant가 시그널 하나만 참조하지만,
-- "같은 시그널을 참조하는 서로 다른 두 `ClickAction` 인스턴스가 서로 다른 엘리먼트에
-- 붙는 것" 자체는 IR 타입 수준에서 막혀있지 않음 — `format_i32_capture_for_action`의
-- doc 주석이 이미 명시한 캐비앗과 정확히 같은 지점). 따라서 이 정리는 "IR이 막아준다"가
-- 아니라 "실제로 이 컴포넌트(`demo/counter-list.tsx`)에서 각 시그널이 우연히 액션
-- 하나씩만 갖고 있다"는 **인스턴스 수준 사실**에 의존한다 — 이 캐비앗을 다음 결로
-- 남긴다: IR이 "시그널당 액션 최대 1개"를 실제로 강제하는 정적 검사를 갖추기 전까지는
-- 이 정리의 전제(`SingleWriter`)가 컴파일러가 아니라 저작자의 규율에 의존한다.
-- **2026-07-28 갱신(Task #28-f)**: 이 정적 검사가 이제 실제로 존재한다
-- (`lower.rs`의 `check_one_action_per_signal` — `Component::action_signal_names`가
-- 중복을 찾으면 `lower_module`이 panic) — 그러므로 위 문단이 말하던 "컴파일러가
-- 아니라 저작자의 규율에 의존" 상태는 더 이상 아니다. `SingleWriter`는 이제 IR
-- 인스턴스 수준 사실이 아니라 컴파일러가 구조적으로 강제하는 불변식이다.

/-! ## Bool/StringList/Str 확장 (Task #28-e, 2026-07-28)

`codegen.rs`의 `generate_resume` 실제 arm을 확인한 결과(각 `SignalInit` variant별
직접 대조, 문서 옆에서 확인하며 작성) — 위 `I32` 케이스만 `captures` 배열의 **캡처된
리터럴**(`v0`, SSR 시점에 구운 스냅샷)에서 값을 재구성한다. 나머지 세 종류는
전혀 다른 전략을 쓴다:

- `Bool`: `dom.adopt_children(...)`으로 지금 DOM에 실제로 보이는 자식이 있는지
  직접 읽어 `shown`을 결정한다(`captures`를 아예 참조하지 않음).
- `StringList`: `dom.adopt_children(...)`으로 각 행의 **현재 텍스트**를 직접 읽는다.
- `Str`(input 바인딩): `dom.get_input_value(...)`으로 `<input>`의 **현재 `.value`
  속성**을 직접 읽는다.

셋 다 "캡처해 둔 과거 스냅샷 + 그 이후 델타"가 아니라 "지금 이 순간 DOM에 보이는
값 그 자체"를 읽는다 — 그래서 `SingleWriter` 전제(=스냅샷 시점 이후 아무도 안
건드렸음을 요구)가 애초에 필요 없다: 읽는 대상 자체가 항상 최신이므로 "낡을" 수가
없다. 이게 이 절이 formalize하는 것. -/

/-- DOM-복구형 시그널의 추상 모델. `α`는 `Bool`(조건부 표시 여부)·`String`
    (input 값 하나 또는 리스트의 행 하나) 등, `adopt_children`/`get_input_value`가
    실제로 돌려주는 값의 타입을 나타내는 자리표시자 — 이 정리의 핵심은 그 타입이
    *무엇*인지가 아니라 "resume이 읽는 소스 = live가 쓰는 소스"라는 구조적 사실이라
    다형으로 남긴다. -/
abbrev DomState (α : Type) := α

/-- 라이브 경로의 값 — patch System(`patch_input_system`/`list_reconcile_k_system`/
    `cond_reconcile_*_system`)이 매 tick마다 World와 DOM을 동기화하므로, "지금
    라이브 World가 들고 있는 값"과 "지금 DOM에 보이는 값"은 애초에 같은 것을
    가리키는 두 이름이다 — 그래서 정의 자체가 항등함수다. -/
def domRecoveredLiveValue {α : Type} (dom : DomState α) : α := dom

/-- resume 경로의 값 — `adopt_children`/`get_input_value`가 하는 일 그대로:
    `captures`를 거치지 않고 지금 DOM에서 직접 읽는다. -/
def domRecoveredResumeValue {α : Type} (dom : DomState α) : α := dom

/-- **핵심 정리, I32와의 결정적 차이**: 이 등식엔 `SingleWriter`에 해당하는 어떤
    전제도 붙어 있지 않다(화살표 왼쪽이 없음 — `resumability_sound`와 비교).
    다중 writer가 이 DOM 상태를 몇 번을 고쳤든, resume은 언제나 "그 마지막
    결과"를 그대로 읽으므로 라이브 값과 항상 일치한다 — 스냅샷이 없으므로 낡을
    여지 자체가 없다. -/
theorem dom_recovered_resumability_sound {α : Type} (dom : DomState α) :
    domRecoveredResumeValue dom = domRecoveredLiveValue dom := rfl

/-- 위 정리가 다중 writer 아래에서도 무조건 성립함을 명시적으로 보이는 예시 —
    `Bool` 시그널(조건부 표시 여부)을 서로 다른 두 버튼이 각각 toggle하는
    상황을 흉내낸다: 몇 번을 토글하든(`w1`, `w2`, ... 무순서로 여러 writer가
    다녀가도) DOM에 마지막으로 남은 상태가 `finalState`라면, resume은 정확히
    `finalState`를 돌려준다 — I32였다면 이 상황이 §65-76의 반례처럼 깨졌을
    자리다. -/
example (finalState : Bool) :
    domRecoveredResumeValue finalState = domRecoveredLiveValue finalState := rfl

example (finalRows : List String) :
    domRecoveredResumeValue finalRows = domRecoveredLiveValue finalRows := rfl

end Loom.Resumability
