---- MODULE SchedulerSafety ----
\* 스켈레톤으로 유지함(2026-07-24, Phase 5 — 의도적 결정, 방치 아님). RQ5 후보 #2.
\*
\* 결론: TLA+ 모델체킹을 진행하지 않는다. 아래가 그 근거의 전부다 — "귀찮아서"가
\* 아니라 실제로 코드를 읽고 확인한 결과다(CLAUDE.md의 반-과잉엔지니어링 원칙과
\* 정합: 이미 해결된 문제를 다시 풀지 않는다).
\*
\* ## 조사한 것
\*
\* 이 프로젝트는 `bevy_ecs = "0.19"`를 어떤 크레이트에서도 `multi_threaded` feature
\* 없이 사용한다(전 크레이트 Cargo.toml 확인, roadmap/PHASES.md Phase 5 절 참고) —
\* `multi_threaded`는 bevy_ecs 0.14.2의 default feature가 아니다
\* (`~/.cargo/registry/.../bevy_ecs-0.14.2/Cargo.toml`의 `[features]`:
\* `default = ["bevy_reflect"]`, `multi_threaded`는 별도 opt-in). 즉 이 프로젝트가
\* 실제로 쓰는 실행기는 `ExecutorKind::SingleThreaded`뿐이다
\* (`bevy_ecs-0.14.2/src/schedule/schedule.rs:206`) — 한 `Schedule::run` 호출 안의
\* 모든 System은 한 스레드에서 순차 실행된다. **동시 실행 자체가 없으므로, "두
\* System의 read/write 집합이 겹치면 데이터 레이스가 난다"는 이 정리가 막으려는
\* 사건 자체가 이 배포 형태에서는 애초에 발생할 수 없다** — 정리가 거짓이 아니라
\* 대상(동시성)이 없어 공허하게 참이다.
\*
\* 이것만으로는 부족해서(이 프로젝트가 나중에 `multi_threaded`를 켤 수도 있으므로)
\* multi_threaded 실행기 자체의 안전성도 확인함:
\* `bevy_ecs-0.14.2/src/schedule/executor/multi_threaded.rs`의 `can_run`
\* (459행)이 System을 실제로 실행하기 **직전에**
\* `system.archetype_component_access().is_compatible(&self.active_access)`
\* (503-504행)을 검사한다 — 이건 `ambiguity_detection`(기본값 `LogLevel::Ignore`,
\* schedule.rs:2023) 설정과 **무관하게** 항상 실행되는 런타임 안전장치다.
\* `ambiguity_detection`은 "선언되지 않은 순서로 겹치는 System 쌍이 있다"는 걸
\* **경고만** 하는 개발자 진단 기능이지(schedule.rs:1832-1850 부근), 실제 동시
\* 실행을 막는 코드가 아니다 — 그 역할은 `can_run`이 무조건 수행한다. 즉
\* multi_threaded를 켜더라도, 겹치는 System 두 개는 (경고를 안 받더라도) 절대
\* 동시에 실행되지 않고 하나가 끝날 때까지 다른 하나가 대기한다.
\*
\* ## 결론
\*
\* "컴파일러가 생성한 두 System의 read/write 접근 집합이 disjoint하다고 선언되면,
\* 실제로 어떤 인터리빙에서도 데이터 레이스가 없다"는 이 정리가 필요로 하는 성질은
\* 이 프로젝트 자체의 코드가 아니라 **Bevy ECS 자체가 이미 구현·검증한 런타임
\* 메커니즘**(위 `can_run`)이 무조건 제공한다 — 이걸 TLA+로 다시 증명하는 건 Bevy
\* 자체를 재검증하는 것이고, 그건 이 프로젝트의 스코프 밖(CLAUDE.md 2-1: 이미
\* 잘 풀린 문제를 재발명/재검증하지 않음, 우리가 진짜 새로 풀어야 하는 지점에만
\* 집중). 이 프로젝트가 실제로 책임져야 할 유일한 몫은 "우리가 생성하는 System의
\* `Query`/`Res`/`ResMut` 타입 선언이 그 System 본문이 실제로 건드리는 데이터를
\* 정확히 반영하는가"(=코드젠 정확성)인데, 이건 애초에 이 정리가 다루는 "스케줄러
\* 무충돌성"과는 다른 질문이고, `loom-compiler`의 기존 native 테스트(생성된
\* Rust가 컴파일되고 원하는 shape을 갖는지 확인)로 이미 실질적으로 커버된다 —
\* 별도의 TLA+ 모델이 필요한 새 정리가 아니다.
\*
\* **재검토 조건(향후)**: 이 결론은 (a) `multi_threaded` feature가 계속 꺼져 있고,
\* (b) Bevy 0.14의 위 `can_run` 메커니즘이 유지되는 한 성립한다. Bevy를 메이저
\* 업그레이드하거나 `multi_threaded`를 켜기로 결정하면, 최소한 (b)가 여전히
\* 성립하는지(해당 버전의 실행기 소스를 다시 읽어) 재확인할 것 — 이 결론을
\* 한 번 내렸다고 영구히 참으로 가정하지 않는다.
\*
\* 상세 근거는 roadmap/PHASES.md Phase 5 절 + research-questions/
\* RQ5-formal-verification-targets.md에도 기록.

====
