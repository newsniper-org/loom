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
    let container = document
        .get_element_by_id("bench")
        .expect("no #bench element — see demo/list_bench.html");
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
        system: impl bevy_ecs::prelude::IntoSystemConfigs<M>,
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
