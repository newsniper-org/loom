---- MODULE SchedulerSafety ----
\* 스켈레톤. 안전성 속성 정의 없음. RQ5 후보 #2.
\*
\* 착수 전 먼저 판단할 것: 이 속성이 Rust 타입 시스템(borrow checker)으로 이미
\* 보장되는 것의 중복 증명인지. 만약 그렇다면 TLA+ 모델체킹의 한계효용은 낮고,
\* 대신 "컴파일러가 생성한 read/write 선언이 실제 System 본문과 일치한다"는
\* 훨씬 좁고 실질적인 속성(=코드젠 정확성)으로 대상을 바꾸는 게 나을 수 있음.

EXTENDS Naturals, Sequences

\* TODO: Phase 1 프로토타입이 실제 System을 생성하기 시작한 뒤,
\* 그 예시의 실제 접근 패턴(ReadSet/WriteSet)을 가지고 아래를 채울 것.
\* 추상적으로 먼저 만들고 나중에 끼워맞추지 말 것 (CLAUDE.md 참고).

CONSTANTS
    Systems,      \* 아직 정의 안 됨
    ComponentIds  \* 아직 정의 안 됨

VARIABLES
    scheduled     \* 아직 정의 안 됨 -- 이번 tick에 스케줄된 System 집합

\* TODO: ReadSet(sys), WriteSet(sys) 연산자 정의
\* TODO: Disjoint(sys1, sys2) == (ReadSet(sys1) \cup WriteSet(sys1)) \cap WriteSet(sys2) = {} /\ ...
\* TODO: SafetyProperty == \A s1, s2 \in scheduled : s1 # s2 => Disjoint(s1, s2) \/ ExplicitOrdering(s1, s2)

====
