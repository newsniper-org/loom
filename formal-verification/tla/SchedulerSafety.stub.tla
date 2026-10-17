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
\*
\* ## 2026-10-17 — 재검토 조건 이행: bevy_ecs 0.19.0 소스로 다시 읽음
\*
\* **이행 경위(정정 포함)**: 위 본문은 2026-07-24에 bevy_ecs 0.14.2 소스를 읽고
\* 쓴 것이다. 2026-07-28 커밋 7e30659("의존성 업데이트")에서 Cargo.lock의
\* bevy_ecs가 0.14.2 → 0.19.0으로 **메이저 업그레이드**됐고, 같은 커밋이 이 파일
\* 10행의 버전 문자열만 `"0.14"` → `"0.19"`로 바꿨다. 위 재검토 조건이 요구한
\* "해당 버전의 실행기 소스를 다시 읽기"는 그때 이행되지 않았다 — 그래서 약 석 달
\* 동안 10행은 0.19를, 나머지 근거는 0.14.2의 파일:라인을 가리키는 혼합 상태였다.
\* 아래가 그 재확인이다. 위 0.14.2 인용(schedule.rs:206, multi_threaded.rs:459/
\* 503-504 등)은 **0.14.2 기준의 역사적 기록**으로 남겨 둔다 — 0.19에는
\* `ExecutorKind`도 `archetype_component_access`도 없다.
\*
\* 확인 대상: Cargo.lock의 `bevy_ecs 0.19.0`(워크스페이스 `bevy_ecs = "0.19"`,
\* Cargo.toml:41, default-features 그대로). 아래 경로는
\* `~/.cargo/registry/src/index.crates.io-*/bevy_ecs-0.19.0/` 기준이다. 레지스트리에
\* 있는 0.19.1의 `src/schedule/` 디렉터리는 0.19.0과 바이트 단위로 동일함을
\* `diff -r`로 확인했다(그러므로 0.19.1로 올라가도 아래 결론은 그대로다).
\*
\* (a) `multi_threaded`는 0.19.0에서도 default feature가 아니다 — **성립 유지.**
\*   - `Cargo.toml:63-68`: `default = ["std", "bevy_reflect", "async_executor",
\*     "backtrace"]`. `multi_threaded = ["bevy_tasks/multi_threaded"]`(71행)는 별도
\*     opt-in이다.
\*   - 실제로 해석된 feature: `cargo tree --offline -e features -i bevy_ecs`
\*     (호스트와 `--target wasm32-unknown-unknown` 둘 다)가 bevy_ecs에
\*     `default, std, bevy_reflect, async_executor, backtrace`만, bevy_tasks에
\*     `async_executor, futures-lite`만 보고한다. 다른 의존이 feature 통합으로
\*     `multi_threaded`를 켜는 경로도 없다.
\*
\* (b) 이 프로젝트가 실제로 쓰는 실행기는 `SingleThreadedExecutor`다 — **성립 유지.**
\*   - 0.19에서는 `ExecutorKind` 열거형이 사라지고 `Schedule::new`가
\*     `executor: default_executor()`로 실행기를 고른다(`schedule/schedule.rs:405-416`,
\*     특히 410행). `default_executor()`(`schedule/executor/mod.rs:45-65`)는
\*     `cfg(all(not(target_arch = "wasm32"), feature = "std", feature =
\*     "multi_threaded"))`일 때만 `MultiThreadedExecutor`를, 그 밖에는
\*     `SingleThreadedExecutor`를 돌려준다. **wasm32 타깃에서는 feature와 무관하게
\*     항상 SingleThreaded**다 — 이 프로젝트의 클라이언트 배포 타깃이 WASM이므로,
\*     브라우저 쪽은 설령 `multi_threaded`를 켜도 바뀌지 않는다.
\*   - 이 저장소의 스케줄 생성은 `crates/loom-runtime/src/lib.rs`의 `LoomApp::new`
\*     안 `Schedule::default()` 하나뿐이고(`Schedule::default` → `Schedule::new`,
\*     schedule.rs:393-401), `set_executor` 호출은 저장소 어디에도 없다
\*     (`git grep` 확인). 실행은 `fn tick`과 테스트용 `fn run_schedule_for_test`의
\*     `self.schedule.run`뿐이다. (줄 번호는 주석 추가로 쉽게 밀리므로 식별자로
\*     가리킨다. 2026-10-17 작업 트리 기준 lib.rs:1272 / 1399·1405 / 1395-1396.)
\*   - SingleThreaded 실행기는 `executor/single_threaded.rs:71`의
\*     `for system_index in 0..schedule.systems.len()` 루프로 System을 한 스레드에서
\*     순서대로 돈다. 따라서 위 "동시 실행 자체가 없다"는 논증은 0.19에서도 그대로다.
\*   - 주의할 차이 하나: 0.19에서는 `MultiThreadedExecutor`가 `multi_threaded`가
\*     아니라 `std` feature만으로 컴파일·공개된다(`executor/mod.rs:1-2, 11-12`).
\*     즉 누군가 `schedule.set_executor(MultiThreadedExecutor::new())`를 쓰면 feature
\*     없이도 다중 스레드 실행기로 갈 수 있다. 재검토 조건 (a)를 "feature가 꺼져
\*     있다"에서 **"feature가 꺼져 있고 `set_executor`로 실행기를 바꾸지 않는다"**로
\*     넓혀 읽어야 한다.
\*
\* (c) multi_threaded 실행기의 무충돌 보장(위 본문 (b)) — **성립 유지, 메커니즘은
\*     바뀜.** 0.14.2의 "실행 직전에 `archetype_component_access().is_compatible
\*     (&self.active_access)`를 검사"는 0.19에 없다. 대신:
\*   - `executor/multi_threaded.rs:181-227`: 실행기 `init`에서 System마다
\*     `system2.access.is_compatible(&system1.access)`(188행)로 **충돌 System 비트셋
\*     (`conflicting_systems`)**을 미리 계산한다. run condition·set condition과의
\*     충돌도 같은 방식으로 계산한다(196-227행). `access`는 `System::initialize`가
\*     돌려준 `FilteredAccessSet`이다(`schedule/node.rs:36-42`) — archetype 단위가
\*     아니라 컴포넌트·필터 단위 정적 접근 선언이다.
\*   - `can_run`(multi_threaded.rs:543-577)은 System을 띄우기 직전에 그 비트셋이
\*     현재 실행 중인 System 집합(`running_systems`)과 **disjoint하지 않으면 false**를
\*     돌려준다(569-575행). exclusive System은 실행 중인 System이 하나라도 있으면
\*     못 뜬다(545-547행). 호출부 `spawn_system_tasks`(440-540행)는 `can_run`이
\*     false면 그 System을 건너뛰고(488-494행) 다음 기회에 다시 본다.
\*   - 예외 한 줄: `can_run`의 충돌 검사는 `skipped_systems`에 든 System을 면제한다
\*     (569행 `!self.skipped_systems.contains(..)`). 그런 System은 바로 뒤
\*     `should_run`이 false가 되어 실행되지 않고 skip 처리되므로(501-512행, should_run 592행) 동시
\*     접근이 생기지 않는다.
\*   - `ambiguity_detection`은 0.19에서도 기본값 `LogLevel::Ignore`
\*     (schedule.rs:1627)이고, 스케줄 빌드 때 순서가 정해지지 않은 충돌 쌍을
\*     경고(Warn)하거나 빌드 오류(Error)로 바꾸는 진단일 뿐이다(schedule.rs:1253-1261).
\*     실행기의 충돌 비트셋 계산·검사와는 독립이다 — 위 본문의 "로그 설정과 무관하게
\*     항상 작동"은 0.19에서도 참이다.
\*
\* **결론(2026-10-17)**: 위 2026-07-24 결론 — TLA+ 모델체킹 없이 Bevy에 위임 — 은
\* bevy_ecs 0.19.0에서도 유지된다. 근거 파일:라인은 이 절의 것으로 대체한다.
\* **한계(숨기지 않음)**: (1) 이번 확인은 소스 읽기이지 실행 증거가 아니다 —
\* 실행기 종류를 런타임에 단언하는 테스트는 없다(실행기 타입이 공개 API로 노출되지
\* 않음). (2) Bevy의 `FilteredAccessSet::is_compatible`(query/access.rs:1153)
\* 자체가 옳다는 것은 Bevy를 믿는 것이다 — 원래 결론과 같은 위임이다. (3) 다음
\* 메이저 업그레이드(0.20 이상) 때 이 절을 다시 이행해야 한다. 이번처럼 버전 문자열만
\* 바꾸고 넘어가지 말 것.

====
