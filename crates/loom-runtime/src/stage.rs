//! RQ1-benchmark-protocol.md **§7.1 "단계 귀속 계측 지도"** 의 구현.
//!
//! §7.1은 여러 판정 조항이 참조하는 "구간"들을 이렇게 정의해 두었다:
//!
//! | 판정 조항이 참조하는 양 | 정의된 계측 구간 |
//! |---|---|
//! | C 기각 (ii) "archetype move 단계" | 해당 프레임의 Commands apply(sync point) 구간 |
//! | E 기각 (ii) "매칭 비용" | reconcile System(해시 매칭 + LIS + successor 검색) 시간 + 이동 집합 정렬 |
//! | E 기각 (ii)·(i) 보조 "DOM 연산 시간" | 단일 디스패치 지점의 배치 적용 구간 |
//! | §9 B/F의 J-b "총 CPU 시간" | 셀 구동 창 내 전 System + flush + DOM 적용 합산 |
//!
//! 그리고 계측 **방식**까지 못박아 두었다 — "전부 하네스가 Bevy System/스테이지
//! 경계에서 타임스탬프를 찍는 방식(**후보 코드 무침습** — §2.4 정합)". 즉
//! `list_reconcile_c_system` 같은 후보 구현 안에 계측 코드를 심으면 안 되고,
//! 스케줄에 마커 System을 끼워 넣어 바깥에서 재야 한다. 이 모듈이 그 마커다.
//!
//! **왜 이게 없으면 판정이 막히나**: 2026-07-28 기준 RQ1에서 미해결로 남은
//! 항목 중 두 개가 정확히 이 계측에 걸려 있다 — E/E-LIS의 기각 (ii)("매칭
//! 비용이 절감분을 상쇄하는가")는 reconcile System 시간과 DOM 적용 시간을
//! *따로* 재야 판정 가능하고, C의 기각 뒷 분기("archetype move 구간이 스크립트
//! 시간의 50% 이상")는 sync point 구간을 따로 재야 한다. 지금까지 하네스는
//! `LoomApp::tick` 전체 시간(M1(a))만 쟀다.
//!
//! **스케줄 배치**: `LoomApp::add_instrumented_systems`가
//! `(시작 마커, 후보 System, 종료 마커, ApplyDeferred, sync 마커).chain()`
//! 으로 엮는다. `ApplyDeferred`(`bevy_ecs::prelude`)는 Commands 버퍼를 그
//! 지점에서 적용시키는 배타 System이라, 그 앞뒤 마커의 차이가 곧 §7.1이 말하는
//! "Commands apply(sync point) 구간"이 된다 — C의 R3 브랜치 전환형(§10 사전
//! 고정)이 archetype move를 일으키는 바로 그 구간.
//!
//! **주의 — 계측된 tick과 비계측 tick의 시간은 직접 비교하지 말 것**: 위
//! `.chain()`은 그룹 전체를 강제로 직렬 실행시키고 스케줄 중간에
//! `ApplyDeferred` sync point를 하나 더 만든다. 비계측 경로(`add_systems`)에서는
//! Bevy가 병렬 실행 여지를 갖고 Commands도 스케줄 끝에서 한 번만 적용되므로,
//! 같은 워크로드라도 `StageTimings::total_ms`가 기존 M1(a)보다 클 수 있다.
//! 그래서 §9의 **시간 판정은 계속 비계측 M1(a)로** 하고, 이 모듈의 값은
//! §7.1이 요구하는 **구간 비율**(예: "archetype move 구간이 스크립트 시간의
//! 50% 이상", "매칭 비용 ≥ DOM 절감분") 판정에만 쓴다 — 두 조항 모두 절대
//! 시간이 아니라 같은 tick 안의 구간 간 비교라서 이 오버헤드에 불변이다.
//! 후보 간 비교에 쓸 때도 전 후보가 동일하게 계측된 상태여야 공평하다(§2.4).

use bevy_ecs::prelude::*;

/// 단조 증가하는 밀리초 시계. 브라우저에서는 `performance.now()`(§8이 요구하는
/// 고해상도 타이머 — COOP/COEP 격리 시 5µs), 그 외에는 프로세스 시작 기준
/// `Instant`.
///
/// **cfg 조건에 `target_arch`가 반드시 들어가야 하는 이유(실제로 깨뜨려 본 뒤
/// 고침)**: `browser` feature는 "브라우저 지원 코드가 **컴파일에 포함됨**"을
/// 뜻하지 "**지금 브라우저에서 실행 중**"을 뜻하지 않는다. `cargo test
/// --workspace`처럼 한 번에 빌드하면 Cargo가 feature를 통합하므로,
/// `loom-runtime`은 `browser`가 켜진 채 **단 한 벌**로 컴파일되어 네이티브
/// 크레이트(`loom-fic` 등)에도 그대로 링크된다. 처음엔 이 함수를
/// `#[cfg(feature = "browser")]`로만 갈랐는데, 그 결과 `LoomApp::tick`이
/// 네이티브 테스트에서 `web_sys::window()`를 호출해 js-sys가
/// "cannot access imported statics on non-wasm targets"로 패닉했다
/// (`loom-fic` 테스트 7개 동시 실패). feature가 아니라 **실행 타겟**으로
/// 갈라야 맞다.
#[cfg(all(feature = "browser", any(target_arch = "wasm32", target_arch = "wasm64")))]
pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or(0.0)
}

#[cfg(not(all(feature = "browser", any(target_arch = "wasm32", target_arch = "wasm64"))))]
pub fn now_ms() -> f64 {
    use std::time::Instant;
    thread_local! {
        static ORIGIN: Instant = Instant::now();
    }
    ORIGIN.with(|o| o.elapsed().as_secs_f64() * 1000.0)
}

/// 한 tick의 단계별 소요 시간. `LoomApp::add_instrumented_systems`를 쓴
/// 앱에만 존재하며(일반 앱에는 이 Resource 자체가 없다), 매 tick 덮어써진다 —
/// 누적은 하네스 쪽 몫이다(§8의 표본 규정이 셀마다 다른 집계를 요구하므로
/// 런타임이 미리 줄여 놓으면 안 됨).
#[derive(Resource, Default, Clone, Copy, Debug)]
pub struct StageTimings {
    /// 후보 reconcile System 자체의 실행 시간 — §7.1의 E 기각 (ii) "매칭 비용".
    pub systems_ms: f64,
    /// `ApplyDeferred` 구간 = Commands apply(sync point) — §7.1의 C 기각 (ii)
    /// "archetype move 단계".
    pub sync_point_ms: f64,
    /// 단일 디스패치 지점의 DOM 배치 적용 구간 — §7.1의 E 보조 "DOM 연산 시간".
    /// `LoomApp::tick`이 직접 채운다(스케줄 바깥이라 마커 System으로는 못 잼).
    pub dom_apply_ms: f64,
    /// **R2(§3.2) row 깊이 축의 patch 구간** (개정 #9-11, 2026-08-12).
    /// row 내부 바인딩 patch System + 중첩 조건부 reconcile System의 시간.
    ///
    /// `systems_ms`와 **분리**하는 이유: §7.1이 E 기각 (ii)의 "매칭 비용"을
    /// "reconcile System(해시 매칭 + LIS + successor 검색) 시간 + 이동 집합 정렬
    /// 구간"으로 정의했다. row 바인딩 패치는 그 어느 것도 아니므로 같은 구간에
    /// 합산하면 그 조항이 재려는 양이 오염된다. 축이 꺼져 있으면 0이다.
    pub row_patch_ms: f64,
    /// tick 전체(= M1(a)). 위 구간들의 합보다 크며, 그 차이가 스케줄 오버헤드다.
    pub total_ms: f64,

    // --- 내부 스탬프(마커 System 간 전달용) ---
    t_systems_start: f64,
    t_systems_end: f64,
    t_row_patch_start: f64,
}

/// 후보 System **앞**에 놓이는 마커.
pub fn stage_mark_systems_start(mut t: ResMut<StageTimings>) {
    t.t_systems_start = now_ms();
}

/// 후보 System **뒤**, `ApplyDeferred` **앞**에 놓이는 마커.
pub fn stage_mark_systems_end(mut t: ResMut<StageTimings>) {
    t.t_systems_end = now_ms();
}

/// `ApplyDeferred` **뒤**에 놓이는 마커 — 여기서 두 구간이 확정된다.
pub fn stage_mark_sync_end(mut t: ResMut<StageTimings>) {
    let now = now_ms();
    t.systems_ms = t.t_systems_end - t.t_systems_start;
    t.sync_point_ms = now - t.t_systems_end;
    // 축이 꺼져 있으면 아래 두 마커가 아예 등록되지 않으므로 여기서 0으로
    // 되돌려 둔다 — 지난 tick 값이 남아 프로파일에 유령이 서지 않게.
    t.row_patch_ms = 0.0;
}

/// R2 patch System 묶음 **앞**(= `stage_mark_sync_end` 뒤).
pub fn stage_mark_row_patch_start(mut t: ResMut<StageTimings>) {
    t.t_row_patch_start = now_ms();
}

/// R2 patch System 묶음 **뒤**.
pub fn stage_mark_row_patch_end(mut t: ResMut<StageTimings>) {
    t.row_patch_ms = now_ms() - t.t_row_patch_start;
}
