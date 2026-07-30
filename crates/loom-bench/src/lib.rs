//! RQ1 Track L "핵심 격자" (core grid) — real browser measurement across
//! candidates K/C/D/E/D-LIS/E-LIS on the *same* workload sequence, run
//! back-to-back in one page load so results share hardware/JIT-warmup
//! conditions. D-LIS/E-LIS are RQ1's "이동 최소화: LIS" variant axis,
//! measured as separate candidates alongside plain D/E (not replacing
//! them) — see research-questions/RQ1-dynamic-control-flow-scheduling.md's
//! Phase 1 Track L section for why plain D/E move far more than necessary
//! on redistributive workloads (swap, shuffle) without this.
//!
//! Scope, stated plainly (roadmap/PHASES.md Phase 1 status): this extends
//! the first pass (single size, single item type, median/min/max only) with
//! N-scaling ({100, 1000, 10000}) and a POD-vs-heap item-type axis, and
//! emits *raw samples* (not pre-reduced statistics) so the Python-side
//! report can compute the fuller distributional picture
//! (median/p25/p75/p95/min/max) RQ1-benchmark-protocol.md §8 calls for.
//! Still not the protocol's full sanctioned run — no multi-browser sweep
//! from *this* binary (done separately, Chromium + Firefox, same build),
//! no row-depth (R1/R2/R3) axis, no paired-bootstrap CI. See
//! `research-questions/RQ1-dynamic-control-flow-scheduling.md`'s "Phase 1
//! 트랙 L" section for the complete, current list of what's covered.
//!
//! Results are written as a JSON string into `#results`' `textContent`
//! (kept as raw per-rep sample arrays, not pre-aggregated).

use bevy_ecs::system::ScheduleSystem;
use loom_runtime::web::BrowserDom;
use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, list_reconcile_k_system, DomTarget,
    ListRows, LoomApp, OrderKeyMembers, Signal,
};
use wasm_bindgen::prelude::*;

/// Deterministic xorshift64 — reproducible across candidates/reps without
/// pulling in the `rand` crate for this bounded harness.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// A minimal POD item type (fixed-width `u32`) alongside `String` (heap),
/// so the item-type axis RQ1-benchmark-protocol.md §3.2/R-axis calls for
/// gets at least a partial cross-check, not just the heap case.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ItemId(u32);
impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "id-{}", self.0)
    }
}

enum Workload {
    Swap,
    /// W2 (RQ1-benchmark-protocol.md §3.1) — full Fisher-Yates permutation,
    /// as opposed to `Shuffle10`'s partial relabeling. Added alongside the
    /// row-depth axis specifically because RQ1 §9's D-rejection gate (iii)
    /// needs `{W1,W2,W3,W4}×{R1,R2}` — W2 didn't exist in this harness
    /// before (stated plainly: the core grid measured so far only ever
    /// covered W1/W3/W4/W5/remove_middle).
    Shuffle,
    Shuffle10,
    HeadInsert,
    Append,
    RemoveMiddle,
}

impl Workload {
    const ALL: [Workload; 6] = [
        Workload::Swap,
        Workload::Shuffle,
        Workload::Shuffle10,
        Workload::HeadInsert,
        Workload::Append,
        Workload::RemoveMiddle,
    ];

    fn name(&self) -> &'static str {
        match self {
            Workload::Swap => "swap",
            Workload::Shuffle => "shuffle",
            Workload::Shuffle10 => "shuffle10",
            Workload::HeadInsert => "head_insert",
            Workload::Append => "append",
            Workload::RemoveMiddle => "remove_middle",
        }
    }

    /// Fisher-Yates, using the shared per-rep `Rng` so every candidate in a
    /// rep sees the identical target permutation (same pairing discipline
    /// as `Shuffle10`).
    fn fisher_yates<T>(v: &mut [T], rng: &mut Rng) {
        for i in (1..v.len()).rev() {
            let j = rng.below(i + 1);
            v.swap(i, j);
        }
    }

    /// Applies this workload to a *fresh copy* of the baseline list, using
    /// `rng` only where randomness is called for (Shuffle/Shuffle10/inserts)
    /// so every candidate sees the identical target sequence for a given
    /// rep. `fresh` mints a never-seen-before item for insert-shaped
    /// workloads.
    fn apply<T: Clone>(&self, baseline: &[T], rng: &mut Rng, fresh: impl Fn(u64) -> T) -> Vec<T> {
        let mut v = baseline.to_vec();
        match self {
            Workload::Swap => {
                let last = v.len() - 1;
                v.swap(0, last);
            }
            Workload::Shuffle => {
                Self::fisher_yates(&mut v, rng);
            }
            Workload::Shuffle10 => {
                let moves = (v.len() / 10).max(1);
                for _ in 0..moves {
                    let from = rng.below(v.len());
                    let to = rng.below(v.len());
                    let item = v.remove(from);
                    v.insert(to.min(v.len()), item);
                }
            }
            Workload::HeadInsert => {
                v.insert(0, fresh(rng.next()));
            }
            Workload::Append => {
                v.push(fresh(rng.next()));
            }
            Workload::RemoveMiddle => {
                let mid = v.len() / 2;
                v.remove(mid);
            }
        }
        v
    }
}

/// One candidate under test: owns its own `LoomApp` + `BrowserDom` (each
/// candidate gets an isolated World/DOM subtree so none of their
/// bookkeeping — `ListRows` vs `OrderKeyMembers` — has to be reconciled
/// into one shape).
struct Candidate<T: Send + Sync + Clone + ToString + 'static> {
    name: &'static str,
    app: LoomApp,
    dom: BrowserDom,
    entity: bevy_ecs::entity::Entity,
    _marker: std::marker::PhantomData<T>,
}

fn measure_now() -> f64 {
    web_sys::window().unwrap().performance().unwrap().now()
}

impl<T: Send + Sync + Clone + ToString + 'static> Candidate<T> {
    fn set_and_tick(&mut self, items: Vec<T>) -> f64 {
        self.app.world.get_mut::<Signal<Vec<T>>>(self.entity).unwrap().0 = items;
        let start = measure_now();
        self.app.tick(&mut self.dom);
        measure_now() - start
    }
}

fn make_container(document: &web_sys::Document, parent: &web_sys::Node, label: &str) -> web_sys::Node {
    let heading = document.create_element("h4").unwrap();
    heading.set_text_content(Some(label));
    parent.append_child(&heading).unwrap();
    let ul = document.create_element("ul").unwrap();
    parent.append_child(&ul).unwrap();
    ul.into()
}

fn make_k<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "K");
    let (mut dom, handle) = BrowserDom::new(document.clone(), root);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_systems(list_reconcile_k_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "K", app, dom, entity, _marker: std::marker::PhantomData }
}

fn make_c<T: Send + Sync + Clone + ToString + PartialEq + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "C");
    let (mut dom, handle) = BrowserDom::new(document.clone(), root);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_systems(list_reconcile_c_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "C", app, dom, entity, _marker: std::marker::PhantomData }
}

fn make_d<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "D");
    let (mut dom, handle) = BrowserDom::new(document.clone(), root);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_systems(list_reconcile_d_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "D", app, dom, entity, _marker: std::marker::PhantomData }
}

fn make_e<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "E");
    let (mut dom, handle) = BrowserDom::new(document.clone(), root);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_systems(list_reconcile_e_system::<T>);
    let entity =
        app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "E", app, dom, entity, _marker: std::marker::PhantomData }
}

/// RQ1's LIS-optimized variant of D — see
/// research-questions/RQ1-dynamic-control-flow-scheduling.md's Phase 1
/// Track L section for why this is measured as a *separate* candidate
/// alongside plain D, not a replacement.
fn make_d_lis<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "D-LIS");
    let (mut dom, handle) = BrowserDom::new(document.clone(), root);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_systems(list_reconcile_d_lis_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "D-LIS", app, dom, entity, _marker: std::marker::PhantomData }
}

/// RQ1's LIS-optimized variant of E.
fn make_e_lis<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "E-LIS");
    let (mut dom, handle) = BrowserDom::new(document.clone(), root);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_systems(list_reconcile_e_lis_system::<T>);
    let entity =
        app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "E-LIS", app, dom, entity, _marker: std::marker::PhantomData }
}

type Maker<T> = fn(&web_sys::Document, &web_sys::Node, Vec<T>, usize) -> Candidate<T>;

/// All 6 Track L candidates, measured together every time so every result
/// set shares the same page load / hardware / JIT-warmup conditions.
fn all_makers<T: Send + Sync + Clone + ToString + PartialEq + 'static>() -> [Maker<T>; 6] {
    [make_k, make_c, make_d, make_e, make_d_lis, make_e_lis]
}

/// Runs the full workload battery for one (item type, size) combination
/// against all 6 candidates, appending one JSON object per (candidate,
/// workload) cell — with the *raw* per-rep sample array — to `out`.
/// Mounts a scratch container, runs everything, then removes it: peak DOM
/// size stays bounded to one combination at a time (at n=10000 that's still
/// up to ~60,000 live `<li>`s across 6 candidates simultaneously, but never
/// more than one (type, size) combination's worth).
fn run_combination<T, F>(
    document: &web_sys::Document,
    scratch_parent: &web_sys::Node,
    type_name: &str,
    n: usize,
    reps: u32,
    initial: Vec<T>,
    fresh: F,
    row_bindings: usize,
    out: &mut String,
    first_entry: &mut bool,
) where
    T: Send + Sync + Clone + ToString + PartialEq + 'static,
    F: Fn(u64) -> T,
{
    let combo_container = document.create_element("div").unwrap();
    scratch_parent.append_child(&combo_container).unwrap();
    let combo_node: web_sys::Node = combo_container.clone().into();

    let mut candidates: Vec<Candidate<T>> = all_makers()
        .iter()
        .map(|maker| maker(document, &combo_node, initial.clone(), row_bindings))
        .collect();

    for workload in &Workload::ALL {
        for candidate in &mut candidates {
            let mut samples = Vec::with_capacity(reps as usize);
            for rep in 0..reps {
                // Reset to the identical baseline (untimed) before each
                // timed rep, so every rep starts from the same condition.
                candidate.set_and_tick(initial.clone());
                let mut rng = Rng(0x9E3779B97F4A7C15 ^ (rep as u64 + 1));
                let target = workload.apply(&initial, &mut rng, &fresh);
                samples.push(candidate.set_and_tick(target));
            }
            if !*first_entry {
                out.push_str(",\n");
            }
            *first_entry = false;
            let samples_json = samples
                .iter()
                .map(|s| format!("{s:.4}"))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!(
                "  {{\"candidate\": \"{}\", \"workload\": \"{}\", \"item_type\": \"{}\", \"n\": {}, \"row_bindings\": {}, \"reps\": {}, \"samples_ms\": [{}]}}",
                candidate.name,
                workload.name(),
                type_name,
                n,
                row_bindings,
                reps,
                samples_json
            ));
        }
    }

    // Tear down this combination's DOM before the next one mounts.
    scratch_parent.remove_child(&combo_container).ok();
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    // 2026-07-28: `demo/track_t_freq_bench.html`(f축 셀 드라이버)은 같은 wasm
    // 모듈을 로드하지만 트랙 L 배터리를 돌리면 안 된다 — 그 페이지에는 `#bench`가
    // 없으므로 여기서 조용히 빠져나가고, 그쪽은 `TrackTCell`을 직접 생성해서
    // 쓴다(이전엔 `.expect()`라 그런 페이지가 존재할 수 없었음).
    let Some(container) = document.get_element_by_id("bench") else {
        return Ok(());
    };
    let container_node: web_sys::Node = container.into();

    let mut report = String::from("[\n");
    let mut first_entry = true;

    // String (heap) axis: full N-scaling, row depth R1 (default, 1 binding
    // per row — RQ1-benchmark-protocol.md §3.2). REPS shrinks as n grows so
    // total wall time stays bounded — small n needs more reps to see
    // through measurement noise, large n's signal dominates noise on fewer
    // reps. Now also covers W2 (full shuffle) alongside the original 5
    // workloads — RQ1 §9's D-rejection gate (iii) needs `{W1,W2,W3,W4}`.
    for &(n, reps) in &[(100usize, 30u32), (1000, 15), (10000, 6)] {
        let initial: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
        run_combination(
            &document,
            &container_node,
            "String(heap)",
            n,
            reps,
            initial,
            |seed| format!("new-{seed}"),
            1, // R1
            &mut report,
            &mut first_entry,
        );
    }

    // POD (u32 newtype) axis: one representative size (n=1000), not the
    // full N-scaling sweep — a partial, not full, cross-check of RQ1's
    // heap-vs-POD item axis (stated plainly, not silently narrowed).
    {
        let n = 1000;
        let initial: Vec<ItemId> = (0..n as u32).map(ItemId).collect();
        run_combination(
            &document,
            &container_node,
            "ItemId(POD u32)",
            n,
            15,
            initial,
            |seed| ItemId(1_000_000 + (seed as u32 % 1_000_000)),
            1, // R1
            &mut report,
            &mut first_entry,
        );
    }

    // Row-depth axis: R2 ("binding 8개", RQ1-benchmark-protocol.md §3.2) at
    // n=1,000 String(heap) — needed specifically for RQ1 §9's D-rejection
    // gate (iii), which requires `{W1,W2,W3,W4}×{R1,R2}`. Not a full
    // N-scaling sweep at R2 (stated plainly, matching the POD axis's own
    // partial-coverage precedent) — the D-vs-K decision this axis exists to
    // inform doesn't need one (see research-questions/
    // RQ1-dynamic-control-flow-scheduling.md's row-depth write-up).
    {
        let n = 1000;
        let initial: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
        run_combination(
            &document,
            &container_node,
            "String(heap)",
            n,
            15,
            initial,
            |seed| format!("new-{seed}"),
            8, // R2
            &mut report,
            &mut first_entry,
        );
    }

    report.push_str("\n]");

    let results_el = document
        .get_element_by_id("results")
        .expect("no #results element — see demo/list_bench.html");
    results_el.set_text_content(Some(&report));

    run_track_t(&document, &container_node);

    Ok(())
}

/// RQ1 Track T (conditional rendering, candidates B/F) — see cond.rs's
/// module doc comment for scope. Measures toggle-on and toggle-off
/// separately (B's costs differ a lot between the two; F's shouldn't).
/// RQ1 Track T subtree-size axis: 1 was the original minimal-subtree run
/// (2026-07-23, "묻혀 판별 불가"); 10/50/500 are new (2026-07-24). 10/50
/// showed only a directional signal (nonzero-sample rate rising for B,
/// flat for F) still mostly under `performance.now()`'s clamped
/// resolution — 500 is here specifically to find the point where B's
/// per-child DOM-call cost clears that floor decisively.
const SUBTREE_SIZES: [usize; 4] = [1, 10, 50, 500];

fn run_track_t(document: &web_sys::Document, scratch_parent: &web_sys::Node) {
    use loom_runtime::{cond_reconcile_b_system, cond_reconcile_f_system, CondBlock};

    const REPS: u32 = 100;

    struct CondCandidate {
        app: LoomApp,
        dom: BrowserDom,
        entity: bevy_ecs::entity::Entity,
    }
    impl CondCandidate {
        fn set_and_tick(&mut self, value: bool) -> f64 {
            self.app.world.get_mut::<Signal<bool>>(self.entity).unwrap().0 = value;
            let start = measure_now();
            self.app.tick(&mut self.dom);
            measure_now() - start
        }
    }

    fn make_cond<M>(
        document: &web_sys::Document,
        parent: &web_sys::Node,
        name: &str,
        size: usize,
        system: impl bevy_ecs::prelude::IntoScheduleConfigs<ScheduleSystem, M>,
    ) -> (CondCandidate, web_sys::Element) {
        let container = document.create_element("div").unwrap();
        parent.append_child(&container).unwrap();
        let heading = document.create_element("h4").unwrap();
        heading.set_text_content(Some(name));
        container.append_child(&heading).unwrap();
        let anchor = document.create_element("div").unwrap();
        container.append_child(&anchor).unwrap();
        let anchor_node: web_sys::Node = anchor.clone().into();

        let (mut dom, handle) = BrowserDom::new(document.clone(), anchor_node);
        let mut app = LoomApp::new();
        app.add_systems(system);
        let entity = app
            .world
            .spawn((Signal(false), CondBlock::with_size(size), DomTarget(handle)))
            .id();
        app.tick(&mut dom);
        (CondCandidate { app, dom, entity }, container)
    }

    let mut report = String::from("[\n");
    let mut first = true;

    for size in SUBTREE_SIZES {
        for name in ["B", "F"] {
            let (mut candidate, container) = if name == "B" {
                make_cond(document, scratch_parent, name, size, cond_reconcile_b_system)
            } else {
                make_cond(document, scratch_parent, name, size, cond_reconcile_f_system)
            };

            let mut on_samples = Vec::with_capacity(REPS as usize);
            let mut off_samples = Vec::with_capacity(REPS as usize);
            for _ in 0..REPS {
                on_samples.push(candidate.set_and_tick(true));
                off_samples.push(candidate.set_and_tick(false));
            }

            for (workload, samples) in [("toggle_on", on_samples), ("toggle_off", off_samples)] {
                if !first {
                    report.push_str(",\n");
                }
                first = false;
                let samples_json = samples.iter().map(|s| format!("{s:.4}")).collect::<Vec<_>>().join(", ");
                report.push_str(&format!(
                    "  {{\"candidate\": \"{name}\", \"workload\": \"{workload}\", \"size\": {size}, \"reps\": {REPS}, \"samples_ms\": [{samples_json}]}}"
                ));
            }

            scratch_parent.remove_child(&container).ok();
        }
    }
    report.push_str("\n]");

    let results_el = document
        .get_element_by_id("results-cond")
        .expect("no #results-cond element — see demo/list_bench.html");
    results_el.set_text_content(Some(&report));
}

// ============================================================================
// RQ1 트랙 T — 토글 빈도(f) 축 (2026-07-28 신설)
// ============================================================================

/// `RQ1-benchmark-protocol.md` §4.1이 사전 등록한 **토글 빈도 축**
/// (f ∈ {0.1, 1, 10, 30, 60}/s × s ∈ {1, 10, 100, 1000}, 셀당 60초 구동)의
/// 드라이버. 이 축이 없다는 사실 자체가 RQ1 문서에 "§9 B/F 조항을 문자 그대로
/// 적용할 방법이 원리적으로 없다 → **판정 불가**"로 기록돼 있었다
/// (`RQ1-dynamic-control-flow-scheduling.md`) — 이 타입이 그 갭을 닫는다.
///
/// **왜 위의 `run_track_t`(기존 서브트리 크기 축)로는 안 되는가**: 그쪽은
/// `toggle_on`/`toggle_off`를 100회 *연속으로* 재는 루프라 "시간당 빈도"라는
/// 개념 자체가 없다. §9의 셀 승패 판정은 J-a(토글당 스크립트 시간) **와**
/// J-b(셀 구동 창 총 스크립트 CPU — 토글 처리 + **토글 간 상주 비용** 포함)가
/// *둘 다* 우세해야 성립하는데, J-b의 "토글 간 상주" 항은 토글 사이에 실제
/// 시간이 흘러야만(그 동안 프레임마다 tick이 돌아야만) 존재한다. 그래서 이
/// 드라이버는 rAF 루프(실시간)에 매달려 돌아가고, 벽시계 기준으로 다음 토글
/// 시점을 스스로 계산한다 — 동기 루프로는 흉내낼 수 없는 부분.
///
/// **JS가 루프를 소유하는 이유**: rAF 콜백을 Rust 쪽에서 잡으면
/// `Closure::wrap` + `Rc<RefCell<..>>` 자기참조 패턴이 필요한데, 여기서 얻는
/// 게 없다 — 시간 판단(다음 토글이 지금인가)과 계측은 전부 Rust가 하고 JS는
/// `requestAnimationFrame`으로 `step(now)`을 호출해 주기만 하면 된다.
/// `demo/track_t_freq_bench.html` 참고.
///
/// **범위(숨기지 않고 명시)**: §4.1이 요구하는 6 arm 중 이 하네스가 가진 건
/// 여전히 2개(B, 그리고 `cond.rs`가 자기 doc comment에 "CSS 은닉(v-show형)
/// 대조군을 겸한다"고 명시한 오늘의 F)뿐이다. F-i(`CondEffective` 값 필터)/
/// F-ii(detached DOM 계속 패치)/F-iii(`Without<Frozen>` marker)는 구현 자체가
/// 없다. 따라서 이 드라이버만으로 §9의 F 기각 1차 분기("F-i/ii/iii 각각이
/// 통상 구간에서 B를 이기는 셀이 하나도 없으면 기각")를 적용할 수는 없다 —
/// 적용 가능해지는 것은 **"또는" 분기**(B의 spawn 비용이 전 그리드에서
/// < 1ms/100 entity면, 비스로틀·4× 스로틀 양쪽 성립 조건 하에 결론 = B 유지)
/// 쪽이고, 그건 B만 측정하면 되므로 이 하네스로 완결된다.
///
/// 또한 §4.1의 부속 축들(서브트리 구성 flat vs 파생 체인, catch-up 곡선,
/// attached 구간 변경률)도 아직 없다 — 이 커밋은 **기본 f × s 격자**만 연다.
#[wasm_bindgen]
pub struct TrackTCell {
    app: LoomApp,
    dom: BrowserDom,
    entity: bevy_ecs::entity::Entity,
    container: web_sys::Element,
    arm: String,
    size: usize,
    /// 사전 등록 격자의 f를 정수로 나르기 위한 milli-Hz(0.1/s = 100).
    f_millihz: u32,
    toggle_interval_ms: f64,
    window_ms: f64,

    started_at_ms: f64,
    next_toggle_at_ms: f64,
    cond_state: bool,
    toggles_done: u32,

    /// §4.1/§8: "시행 내 첫 토글은 정상 상태 통계에서 제외·별도 보고" —
    /// lazy-spawn이 걸리는 유일한 토글이라 정상 상태와 섞으면 안 된다.
    first_toggle_ms: f64,
    toggle_on_ms: Vec<f64>,
    toggle_off_ms: Vec<f64>,

    /// 토글이 없는 프레임의 tick 시간 합 — J-b의 "토글 간 상주" 항.
    residency_total_ms: f64,
    residency_ticks: u32,
    /// J-b 그 자체: 창 안의 모든 tick 시간 합(토글 + 상주).
    total_script_ms: f64,
    frames: u32,

    /// §8 타이머 조항이 "셀별 유효 해상도를 실행 시점 기록"하라고 요구하는
    /// 그 값 — `probe_timer_resolution()`이 셀 시작 시 1회 실측한다. 이게
    /// 없으면 §8의 배치 규칙("median 추정치 < 해상도 × 50이면 배치 측정으로
    /// 전환")을 적용할지 여부조차 판단할 수 없다.
    timer_resolution_ms: f64,
}

/// `performance.now()`의 **실효** 해상도를 실측한다 — 값이 실제로 바뀔 때까지
/// 바쁜 대기하며 최소 증분을 찾는다. 크로스오리진 격리(COOP/COEP)가 없으면
/// 브라우저가 타이머를 클램프하므로(오늘의 Chromium 기준 100µs) 이 값이 커지고,
/// 격리가 걸리면 5µs 수준으로 떨어진다. §8이 "COOP/COEP로 고해상도 타이머
/// 확보"를 사전 등록해 둔 이유가 이것 — 트랙 T의 토글 비용은 원래 이 클램프
/// 바로 위에서 놀기 때문에(RQ1의 "묻혀 판별 불가" 기록 참고) 해상도가 곧
/// 측정 가능성 자체다.
/// `window.crossOriginIsolated` — 위 해상도 값이 "격리가 걸린 상태의 해상도"인지
/// "클램프된 해상도"인지 구별하는 근거. `web-sys`의 `Window`에 이 게터가 없어
/// `js_sys::Reflect`로 직접 읽는다(§8이 요구하는 기록 항목이라 생략 불가).
fn cross_origin_isolated() -> bool {
    js_sys::Reflect::get(
        &web_sys::window().unwrap(),
        &JsValue::from_str("crossOriginIsolated"),
    )
    .ok()
    .and_then(|v| v.as_bool())
    .unwrap_or(false)
}

fn probe_timer_resolution_ms() -> f64 {
    let mut min_delta = f64::INFINITY;
    for _ in 0..10 {
        let t0 = measure_now();
        let mut t1 = measure_now();
        // 값이 바뀔 때까지 회전 — 바뀐 순간의 증분이 곧 이 시계의 최소 눈금.
        let mut spins = 0u32;
        while t1 == t0 && spins < 5_000_000 {
            t1 = measure_now();
            spins += 1;
        }
        let delta = t1 - t0;
        if delta > 0.0 && delta < min_delta {
            min_delta = delta;
        }
    }
    if min_delta.is_finite() {
        min_delta
    } else {
        0.0
    }
}

#[wasm_bindgen]
impl TrackTCell {
    /// `arm`: "B" | "F". `f_millihz`: 사전 등록 격자 {100, 1000, 10000,
    /// 30000, 60000}. `size`: 서브트리 entity 수 {1, 10, 100, 1000}.
    /// `window_ms`: §4.1 사전 등록값은 60_000 — 파라미터로 둔 이유는
    /// 하네스 자체를 스모크 검증할 때 짧은 창으로 돌리기 위함이고, 실제
    /// 판정 데이터는 60_000으로만 수집한다(축소 창으로 얻은 수치는 §9 판정에
    /// 쓰지 않는다 — JSON에 `window_ms`가 그대로 실려 나가므로 사후에
    /// 구별 가능).
    #[wasm_bindgen(constructor)]
    pub fn new(arm: &str, f_millihz: u32, size: usize, window_ms: f64) -> TrackTCell {
        use loom_runtime::{cond_reconcile_b_system, cond_reconcile_f_system, CondBlock};
        console_error_panic_hook::set_once();

        let document = web_sys::window().unwrap().document().unwrap();
        let body: web_sys::Node = document.body().unwrap().into();

        let container = document.create_element("div").unwrap();
        container.set_attribute("data-track-t-cell", arm).unwrap();
        body.append_child(&container).unwrap();
        let anchor = document.create_element("div").unwrap();
        container.append_child(&anchor).unwrap();
        let anchor_node: web_sys::Node = anchor.into();

        let (mut dom, handle) = BrowserDom::new(document.clone(), anchor_node);
        let mut app = LoomApp::new();
        match arm {
            "B" => {
                app.add_systems(cond_reconcile_b_system);
            }
            "F" => {
                app.add_systems(cond_reconcile_f_system);
            }
            other => panic!("TrackTCell: unknown arm {other:?} (expected \"B\" or \"F\")"),
        }
        let entity = app
            .world
            .spawn((Signal(false), CondBlock::with_size(size), DomTarget(handle)))
            .id();
        // 초기 tick은 창 밖 — 셀은 "숨겨진 상태"에서 출발한다.
        app.tick(&mut dom);

        // 창을 열기 *전에* 잰다 — 바쁜 대기가 창 안의 프레임 예산을 먹지
        // 않도록.
        let timer_resolution_ms = probe_timer_resolution_ms();

        let now = measure_now();
        let toggle_interval_ms = 1000.0 / (f_millihz as f64 / 1000.0);
        TrackTCell {
            timer_resolution_ms,
            app,
            dom,
            entity,
            container,
            arm: arm.to_string(),
            size,
            f_millihz,
            toggle_interval_ms,
            window_ms,
            started_at_ms: now,
            next_toggle_at_ms: now + toggle_interval_ms,
            cond_state: false,
            toggles_done: 0,
            first_toggle_ms: f64::NAN,
            toggle_on_ms: Vec::new(),
            toggle_off_ms: Vec::new(),
            residency_total_ms: 0.0,
            residency_ticks: 0,
            total_script_ms: 0.0,
            frames: 0,
        }
    }

    /// rAF 콜백마다 1회. 반환값 `true` = 창이 아직 안 끝났으니 계속 호출하라.
    ///
    /// 토글이 걸린 프레임이면 Signal을 뒤집고 tick, 아니면 그냥 tick한다 —
    /// **토글이 없어도 매 프레임 tick하는 것이 핵심**이다. 그게 J-b의 "토글 간
    /// 상주" 항이자, F 계열의 상주 게이팅 비용이 드러나는 유일한 지점이기
    /// 때문(B는 여기서 사실상 0이 나오는 것이 기준선 — §4.1의 "B=0(부재
    /// 자체가 데이터)").
    pub fn step(&mut self, now_ms: f64) -> bool {
        if now_ms - self.started_at_ms >= self.window_ms {
            return false;
        }
        self.frames += 1;

        let due = now_ms >= self.next_toggle_at_ms;
        if due {
            self.cond_state = !self.cond_state;
            self.app
                .world
                .get_mut::<Signal<bool>>(self.entity)
                .unwrap()
                .0 = self.cond_state;
        }

        let start = measure_now();
        self.app.tick(&mut self.dom);
        let elapsed = measure_now() - start;
        self.total_script_ms += elapsed;

        if due {
            if self.toggles_done == 0 {
                self.first_toggle_ms = elapsed;
            } else if self.cond_state {
                self.toggle_on_ms.push(elapsed);
            } else {
                self.toggle_off_ms.push(elapsed);
            }
            self.toggles_done += 1;
            // 누적 드리프트를 막으려고 "지금 + interval"이 아니라 예정 시각
            // 기준으로 다음을 잡되, 창 하나를 통째로 놓쳤으면(탭 백그라운드
            // 등) 현재 시각으로 재동기화한다.
            self.next_toggle_at_ms += self.toggle_interval_ms;
            if self.next_toggle_at_ms <= now_ms {
                self.next_toggle_at_ms = now_ms + self.toggle_interval_ms;
            }
        } else {
            self.residency_total_ms += elapsed;
            self.residency_ticks += 1;
        }
        true
    }

    /// 셀 종료 — DOM을 정리하고 JSON 한 줄을 돌려준다. 기존 하네스 관례대로
    /// **원시 표본 배열**을 그대로 싣는다(통계는 Python 분석 쪽에서 계산).
    pub fn finish(&mut self, now_ms: f64) -> String {
        self.container.remove();
        let on = self
            .toggle_on_ms
            .iter()
            .map(|s| format!("{s:.4}"))
            .collect::<Vec<_>>()
            .join(", ");
        let off = self
            .toggle_off_ms
            .iter()
            .map(|s| format!("{s:.4}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{{\"track\": \"T\", \"axis\": \"frequency\", \"arm\": \"{}\", \
             \"f_hz\": {:.4}, \"size\": {}, \"window_ms\": {:.1}, \
             \"actual_window_ms\": {:.1}, \"frames\": {}, \"toggles\": {}, \
             \"first_toggle_ms\": {:.4}, \
             \"toggle_on_ms\": [{}], \"toggle_off_ms\": [{}], \
             \"residency_total_ms\": {:.4}, \"residency_ticks\": {}, \
             \"j_b_total_script_ms\": {:.4}, \
             \"timer_resolution_ms\": {:.6}, \"cross_origin_isolated\": {}}}",
            self.arm,
            self.f_millihz as f64 / 1000.0,
            self.size,
            self.window_ms,
            now_ms - self.started_at_ms,
            self.frames,
            self.toggles_done,
            if self.first_toggle_ms.is_nan() { 0.0 } else { self.first_toggle_ms },
            on,
            off,
            self.residency_total_ms,
            self.residency_ticks,
            self.total_script_ms,
            self.timer_resolution_ms,
            cross_origin_isolated(),
        )
    }
}
