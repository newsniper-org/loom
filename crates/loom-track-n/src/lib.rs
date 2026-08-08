//! RQ1 §5 **트랙 N — 중첩 정합성 property test**.
//!
//! 구조는 §5가 요구한 그대로다: 외부 리스트(n≤50) → 각 row 안 조건부 → 그 분기
//! 안 내부 리스트(n≤50)의 3중 중첩. 픽스처는 `demo/nested-list-derived.tsx`
//! **하나**이고, 후보 다섯(K/D/D-LIS/E/E-LIS)은 그 한 소스를
//! `ListCandidate`만 바꿔 컴파일한 것이다 — 픽스처가 후보마다 다르면 반례가
//! 후보의 결함인지 픽스처 차이인지 구별할 수 없다(§4.1 정체성 통제와 같은 논리).
//!
//! ## 불변식 계열과 이 하네스의 커버리지 (숨기지 않고 적음)
//!
//! | §5 | 내용 | 이 하네스 |
//! |----|------|-----------|
//! | 1 | 최종 DOM 순서·내용 = 참조 모델(후보 의미론별) | **검사함** — `model.rs`가 후보 구현과 독립적으로 계산 |
//! | 2 | D: DOM 구간 불변식 | **검사함** — `RecordingDom::reposition`이 연산 시점에 |
//! | 3 | E: 순서 수렴 | **검사함** — 매 tick `OrderKey` 전순서 ↔ DOM 순서 |
//! | 4 | F: 동결 중 변경의 catch-up | **약한 형태만** — 아래 참고 |
//! | 5 | flush 규약 하 결정적 수렴 | **검사함** — dropped write 0 + 같은 시드 재현 |
//!
//! **불변식 4에 대한 정직한 진술**: 컴파일러가 내는 조건부는 숨김에
//! `SetVisible`을 쓰고 **patch System을 동결하지 않는다**. 즉 "동결 중"이라는
//! 상태가 이 arm에는 존재하지 않으므로, §5가 쓴 그대로의 catch-up(동결 해제 후
//! 밀린 변경을 따라잡음)은 **잴 대상이 없다**. 여기서 검사하는 것은 그보다 약한
//! "숨겨진 동안의 내부 리스트 변경이 다시 보일 때 반영돼 있다"이다. 이것을
//! 통과했다고 §5 불변식 4가 검증됐다고 읽으면 안 된다 — 그건 공허한 확인이다.
//! 진짜 형태를 재려면 `cond.rs`의 F 계열(fragment 분리 + `Frozen` 게이팅)을
//! 컴파일러가 낼 수 있어야 한다(미구현).
//!
//! ## 사이클 규정
//!
//! §5: 사이클당 10^4 시퀀스, 최대 3사이클, **결과 확인 후 변경 불가**.
//! 이 크레이트는 개수를 인자로 받되(`src/main.rs`), 축소 실행은 **하네스 검증**
//! 이지 사이클이 아니다 — 보고할 때 반드시 그렇게 구분할 것.

use bevy_ecs::entity::Entity;
use loom_runtime::{DomTarget, LoomApp, NodeHandle, OrderKey, OrderKeyMembers};

pub mod dom;
pub mod ops;
pub mod model;

use dom::{Family, RecordingDom, Violation};
use ops::Op;
use model::{Model, RowIdentity};

/// 후보 다섯 arm. 같은 TSX 한 소스에서 `ListCandidate`만 바꿔 파생됐다.
pub mod arms {
    pub mod k {
        include!(concat!(env!("OUT_DIR"), "/arm_k.rs"));
    }
    pub mod d {
        include!(concat!(env!("OUT_DIR"), "/arm_d.rs"));
    }
    pub mod d_lis {
        include!(concat!(env!("OUT_DIR"), "/arm_d_lis.rs"));
    }
    pub mod e {
        include!(concat!(env!("OUT_DIR"), "/arm_e.rs"));
    }
    pub mod e_lis {
        include!(concat!(env!("OUT_DIR"), "/arm_e_lis.rs"));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arm {
    K,
    D,
    DLis,
    E,
    ELis,
}

impl Arm {
    pub const ALL: [Arm; 5] = [Arm::K, Arm::D, Arm::DLis, Arm::E, Arm::ELis];

    pub fn name(&self) -> &'static str {
        match self {
            Arm::K => "K",
            Arm::D => "D",
            Arm::DLis => "D-LIS",
            Arm::E => "E",
            Arm::ELis => "E-LIS",
        }
    }

    /// row 정체성이 유지되는가 — 참조 모델이 이걸로 갈린다.
    fn identity(&self) -> RowIdentity {
        match self {
            Arm::K => RowIdentity::Recreated,
            _ => RowIdentity::Retained,
        }
    }
}

/// 픽스처의 초기 상태. `demo/nested-list-derived.tsx`와 **손으로 맞춘** 값이다.
/// 어긋나면 첫 tick부터 불변식 1이 터지므로 조용히 틀릴 수 없다.
pub const INITIAL_OUTER: [&str; 3] = ["a", "bb", "ccc"];
pub const INITIAL_INNER: [&str; 4] = ["p", "q", "r", "s"];

#[derive(Debug, Clone)]
pub struct Outcome {
    pub violations: Vec<Violation>,
    pub op_log_hash: u64,
    pub dropped_writes: usize,
    /// §5 불변식 4의 **시나리오가 몇 번 실제로 일어났는가**("숨은 동안 바뀐
    /// 내부 리스트가 다시 보였다"). 0이면 그 불변식은 검사된 적이 없고,
    /// 그 실행을 "불변식 4 통과"로 읽으면 공허한 확인이다.
    pub catch_up_events: usize,
}

impl Outcome {
    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }

    /// 이 실행에서 터진 **계열 전부**. §5의 귀결(구조적 재발 판정)은 계열별로
    /// 내려지므로 하나만 보고하면 안 된다 — 불변식 2 위반이 불변식 1 위반을
    /// 동반하는 것이 보통이고(구간이 깨지면 최종 DOM도 틀리다), 먼저 걸린
    /// 쪽만 적으면 원인 계열이 가려진다.
    pub fn families(&self) -> Vec<Family> {
        let mut fams: Vec<Family> = Vec::new();
        for v in &self.violations {
            if !fams.contains(&v.family) {
                fams.push(v.family);
            }
        }
        fams
    }

    /// shrinking과 귀속에 쓸 **대표 계열**. 후보 특정 계열(2·3)이 있으면 그쪽을
    /// 고른다 — 원인을 더 좁게 가리키기 때문. 없으면 첫 위반의 계열.
    pub fn primary_family(&self) -> Option<Family> {
        let fams = self.families();
        fams.iter()
            .find(|f| matches!(f, Family::DomSpan | Family::OrderConvergence))
            .or_else(|| fams.first())
            .copied()
    }
}

fn fnv(items: &[String]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for s in items {
        for b in s.as_bytes() {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        h ^= 0xff;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h
}

/// `<ul class="outer">`의 자식들만 잘라 낸다. 바깥 `</ul>`은 **마지막** 것이다
/// (안쪽 `<ul class="inner">`이 먼저 닫히므로 앞에서 찾으면 첫 row에서 잘린다).
fn outer_rows(html: &str) -> String {
    let Some(start) = html.find("<ul class=\"outer\"") else {
        return String::new();
    };
    let open = html[start..].find('>').map(|i| start + i + 1).unwrap_or(start);
    let end = html.rfind("</ul>").unwrap_or(open);
    html[open..end.max(open)].to_string()
}

/// 한 arm을 한 시퀀스로 돌린다. `setup`은 arm 모듈의 `setup`을 감싸 세 signal
/// entity를 돌려주는 클로저다 — 그것 말고는 arm 사이에 다른 점이 없으므로
/// 나머지 로직은 여기 한 벌만 있으면 된다.
pub fn run_with<F>(arm: Arm, ops: &[Op], setup: F) -> Outcome
where
    F: FnOnce(&mut LoomApp, &mut RecordingDom, NodeHandle) -> (Entity, Entity, Entity),
{
    let (mut dom, mount) = RecordingDom::new();
    let mut app = LoomApp::new();
    let (outer_e, open_e, inner_e) = setup(&mut app, &mut dom, mount);
    app.tick(&mut dom);

    let mut outer: Vec<String> = INITIAL_OUTER.iter().map(|s| s.to_string()).collect();
    let mut inner: Vec<String> = INITIAL_INNER.iter().map(|s| s.to_string()).collect();
    let mut open = false;
    let mut m = Model::new(arm.identity(), outer.clone(), open, inner.clone());

    let mut violations = Vec::new();
    check_after_tick(&mut app, &dom, mount, &m, arm, 0, &mut violations);

    for (step, op) in ops.iter().enumerate() {
        let (before_outer, before_open, before_inner) =
            (outer.clone(), open, inner.clone());
        ops::apply(op, &mut outer, &mut open, &mut inner);

        // 앱과 모델에 **같은 해석**을 먹인다.
        m.clear_step_flag();
        if outer != before_outer {
            app.world.get_mut::<loom_runtime::Signal<Vec<String>>>(outer_e).unwrap().0 =
                outer.clone();
            m.set_outer(outer.clone());
        }
        if open != before_open {
            app.world.get_mut::<loom_runtime::Signal<bool>>(open_e).unwrap().0 = open;
            m.set_open(open);
        }
        if inner != before_inner {
            app.world.get_mut::<loom_runtime::Signal<Vec<String>>>(inner_e).unwrap().0 =
                inner.clone();
            m.set_inner(inner.clone());
        }
        app.tick(&mut dom);
        check_after_tick(&mut app, &dom, mount, &m, arm, step + 1, &mut violations);
        if !violations.is_empty() {
            break;
        }
    }

    violations.extend(dom.violations.iter().cloned());
    if app.dropped_writes > 0 {
        violations.push(Violation {
            family: Family::FlushOrdering,
            detail: format!(
                "적용되지 못하고 버려진 write {}건 — 앵커/부모 entity가 해석되지 않았다",
                app.dropped_writes
            ),
        });
    }
    Outcome {
        violations,
        op_log_hash: fnv(&dom.op_log),
        dropped_writes: app.dropped_writes,
        catch_up_events: m.catch_up_events,
    }
}

fn check_after_tick(
    app: &mut LoomApp,
    dom: &RecordingDom,
    mount: NodeHandle,
    m: &Model,
    arm: Arm,
    step: usize,
    out: &mut Vec<Violation>,
) {
    // ── 불변식 1: 참조 모델 ────────────────────────────────────────────────
    let actual = outer_rows(&dom.html(mount));
    let expected = m.render_rows();
    if actual != expected {
        out.push(Violation {
            family: Family::ReferenceModel,
            detail: format!("step {step}: DOM ≠ 참조 모델\n  기대: {expected}\n  실제: {actual}"),
        });
        if m.catch_up_this_step {
            out.push(Violation {
                family: Family::CatchUp,
                detail: format!(
                    "step {step}: 이 tick은 **숨은 동안 바뀐 내용이 다시 보인** 전이였다 \
                     — 위 불일치를 불변식 4에도 귀속한다"
                ),
            });
        }
        return;
    }

    // ── 불변식 4(약한 형태) — 계열 귀속만 한다 ───────────────────────────
    // **중요**: 이 계열은 불변식 1과 **독립적으로 발동할 수 없다**. 숨은 동안의
    // 변경이 반영되지 않았다는 것은 곧 DOM ≠ 참조 모델이기 때문이다. 그래서
    // 별도 비교를 두는 대신, 불변식 1이 깨진 tick이 **catch-up 전이가 일어난
    // tick이면** 그 계열을 함께 귀속한다(위 분기에서 처리).
    //
    // 그리고 통과했을 때 의미가 있으려면 그 전이가 실제로 몇 번 일어났는지가
    // 남아야 한다 — `Outcome::catch_up_events`. 0인 실행을 "불변식 4 통과"로
    // 읽으면 공허하다.

    // ── 불변식 3: E의 순서 수렴 ────────────────────────────────────────────
    if matches!(arm, Arm::E | Arm::ELis) {
        check_order_convergence(app, dom, step, out);
    }
}

/// `OrderKey` 전순서가 DOM 순서와 일치하는가, 그리고 사이트 사이의 파티션이
/// 서로를 침범하지 않는가(§5 불변식 3).
fn check_order_convergence(
    app: &mut LoomApp,
    dom: &RecordingDom,
    step: usize,
    out: &mut Vec<Violation>,
) {
    let mut sites: Vec<(NodeHandle, Vec<Entity>)> = Vec::new();
    let mut q = app.world.query::<(&OrderKeyMembers, &DomTarget)>();
    for (members, target) in q.iter(&app.world) {
        sites.push((target.0, members.0.clone()));
    }

    // 파티션 침범: 한 entity가 두 사이트의 멤버일 수 없다.
    let mut seen: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
    for (i, (_, members)) in sites.iter().enumerate() {
        for e in members {
            if let Some(j) = seen.insert(*e, i) {
                out.push(Violation {
                    family: Family::OrderConvergence,
                    detail: format!("step {step}: entity {e:?}가 사이트 {j}와 {i} 양쪽의 멤버다 — 중첩 파티션 침범"),
                });
            }
        }
    }

    for (parent, members) in sites {
        let mut rows: Vec<(u64, usize)> = Vec::new();
        for e in &members {
            let Some(key) = app.world.get::<OrderKey>(*e).map(|k| k.0) else {
                continue;
            };
            let Some(target) = app.world.get::<DomTarget>(*e).map(|t| t.0) else {
                // 아직 DOM에 없는 멤버 — 숨겨진 조건부 안의 리스트가 그렇다.
                continue;
            };
            let Some(pos) = dom.child_index(parent, target) else {
                out.push(Violation {
                    family: Family::OrderConvergence,
                    detail: format!("step {step}: 멤버 {e:?}가 자기 사이트의 자식이 아니다"),
                });
                continue;
            };
            rows.push((key, pos));
        }
        let mut by_key = rows.clone();
        by_key.sort_by_key(|(k, _)| *k);
        let positions: Vec<usize> = by_key.iter().map(|(_, p)| *p).collect();
        if positions.windows(2).any(|w| w[0] >= w[1]) {
            out.push(Violation {
                family: Family::OrderConvergence,
                detail: format!(
                    "step {step}: OrderKey 전순서와 DOM 순서가 다르다 — key순 DOM 위치 {positions:?}"
                ),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// arm 디스패치 — 모듈 타입이 서로 달라 매크로로 감싼다(런타임 분기 아님).
// ---------------------------------------------------------------------------

macro_rules! run_arm {
    ($arm:expr, $ops:expr, $m:path) => {{
        use $m as a;
        run_with($arm, $ops, |app, dom, mount| {
            let (s, _actions, _inputs) = a::setup(app, dom, mount);
            (s.outer, s.open, s.inner)
        })
    }};
}

pub fn run(arm: Arm, ops: &[Op]) -> Outcome {
    match arm {
        Arm::K => run_arm!(arm, ops, crate::arms::k),
        Arm::D => run_arm!(arm, ops, crate::arms::d),
        Arm::DLis => run_arm!(arm, ops, crate::arms::d_lis),
        Arm::E => run_arm!(arm, ops, crate::arms::e),
        Arm::ELis => run_arm!(arm, ops, crate::arms::e_lis),
    }
}

/// **shrinking** — 반례를 최소형으로 줄인다(§5 "shrinking으로 최소 반례를 얻어
/// RQ1에 등재"). 델타 디버깅: 큰 덩어리부터 지워 보고, 여전히 **같은 계열**이
/// 터지면 채택한다. 계열을 맞추는 것이 중요하다 — 다른 계열의 반례로 미끄러지면
/// 최소형이 원래 문제를 설명하지 않는다.
pub fn shrink(arm: Arm, ops: &[Op], family: Family) -> Vec<Op> {
    let mut best = ops.to_vec();
    let mut chunk = (best.len() / 2).max(1);
    while chunk >= 1 {
        let mut i = 0;
        while i < best.len() {
            let mut candidate = best.clone();
            let end = (i + chunk).min(candidate.len());
            candidate.drain(i..end);
            if candidate.len() < best.len()
                && run(arm, &candidate).violations.iter().any(|v| v.family == family)
            {
                best = candidate;
            } else {
                i += chunk;
            }
        }
        if chunk == 1 {
            break;
        }
        chunk /= 2;
    }
    best
}
