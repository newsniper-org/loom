# 02. 컴파일러 설계 — JSX/TSX → ECS Entity Graph

## 핵심 원칙

React의 `createElement` 방식(런타임에 tree 객체를 만들고 diff)은 채택하지 않음. 대신
Solid의 컴파일 전략처럼, JSX 트리를 컴파일타임에 **static 부분**과 **dynamic binding site**로
분해해서 직접 호출로 컴파일함. 차이는: Solid는 Signal(클로저 기반)을 반응성 원자로 쓰지만,
여기서는 **ECS Component**를 반응성 원자로 씀 — 이유는 03에서 다루는 No-VDOM 스케줄링과
04에서 다루는 resumability 직렬화 둘 다에 필요하기 때문.

## 컴파일 단계

1. **정적 분석**: JSX 트리를 순회하며 각 노드/표현식을 분류
   - 순수 정적 마크업 → SSR용 HTML 템플릿 문자열로 직접 방출 (client에 아무 코드도 안 감)
   - 동적 바인딩 지점(interpolation, 속성, 조건부, 리스트) → binding site로 등록
   - 이벤트 핸들러 → LRef(지연 참조, 04 참고)로 변환
2. **Entity 그래프 생성**: 각 컴포넌트 인스턴스 = World 안의 Entity spawn 호출 하나.
   컴포넌트의 로컬 상태 선언(예: `signal()`) = 그 Entity에 붙는 `Signal<T>` Component.
3. **Patch System 생성**: 각 dynamic binding site마다 컴파일러가 좁은 System 하나를 생성.
   `Query<Changed<Signal<T>>>` 형태로 게이팅되고, 참이면 저장된 DOM 노드 핸들에 직접
   `node.textContent = ...` 류의 최소 연산만 수행. **범용 diff 알고리즘이 존재하지 않음** —
   이게 No-VDOM의 실제 구현 지점.
4. **제어 흐름 특수형**: `{items.map(...)}`, 삼항 조건부 등은 범용 System으로 표현 불가능
   (엔티티 개수 자체가 동적). RQ1 참고 — archetype 템플릿 방식으로 별도 처리 예정,
   아직 미확정.

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

- 제어 흐름 특수형의 정확한 컴파일 타겟 (RQ1)
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
