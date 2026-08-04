//! 감사 프로브 (일회성): §8 배치 규칙의 "교대"에서 정방향 전이와 역방향
//! 전이가 같은 크기의 연산인가를 후보 × 워크로드별로 센다.
//! 하네스(`loom-bench/src/lib.rs`)의 `Workload::apply`를 그대로 옮겨 왔다.

mod support;
use loom_runtime::{
    list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system,
    list_reconcile_e_lis_system, list_reconcile_e_system, list_reconcile_k_system, CountingDom,
    DomOpCounts, DomTarget, ListRows, LoomApp, OrderKeyMembers, Signal, StageTimings,
};
use support::MockDom;

struct Pcg64 {
    state: u128,
    inc: u128,
}
impl Pcg64 {
    const MULT: u128 = 0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645;
    fn from_seed(seed: u64) -> Self {
        let mut rng = Pcg64 { state: 0, inc: (0xDA3E_39CB_94B9_5BDB_u64 as u128) << 1 | 1 };
        rng.step();
        rng.state = rng.state.wrapping_add(seed as u128);
        rng.step();
        rng
    }
    fn step(&mut self) {
        self.state = self.state.wrapping_mul(Self::MULT).wrapping_add(self.inc);
    }
    fn next(&mut self) -> u64 {
        self.step();
        let x = self.state;
        let rot = (x >> 122) as u32;
        let xsl = ((x >> 64) as u64) ^ (x as u64);
        xsl.rotate_right(rot)
    }
    fn below(&mut self, n: usize) -> usize {
        let n = n as u64;
        let zone = u64::MAX - (u64::MAX % n) - 1;
        loop {
            let v = self.next();
            if v <= zone {
                return (v % n) as usize;
            }
        }
    }
}

fn apply(w: &str, baseline: &[String], rng: &mut Pcg64) -> Vec<String> {
    let mut v = baseline.to_vec();
    match w {
        "swap" => {
            let last = v.len() - 2;
            v.swap(1, last);
        }
        "shuffle" => {
            for i in (1..v.len()).rev() {
                let j = rng.below(i + 1);
                v.swap(i, j);
            }
        }
        "shuffle10" => {
            let moves = (v.len() / 10).max(1);
            for _ in 0..moves {
                let from = rng.below(v.len());
                let to = rng.below(v.len());
                let item = v.remove(from);
                v.insert(to.min(v.len()), item);
            }
        }
        "head_insert" => v.insert(0, format!("new-{}", rng.next())),
        "append" => v.push(format!("new-{}", rng.next())),
        "remove_middle" => {
            let mid = v.len() / 2;
            v.remove(mid);
        }
        "value_update" => {
            let k = (v.len() / 10).max(1);
            let mut idx: Vec<usize> = (0..v.len()).collect();
            for i in 0..k {
                let j = i + rng.below(v.len() - i);
                idx.swap(i, j);
            }
            for &i in idx.iter().take(k) {
                v[i] = format!("new-{}", rng.next());
            }
        }
        other => panic!("{other}"),
    }
    v
}

struct Cand {
    app: LoomApp,
    dom: CountingDom<MockDom>,
    entity: bevy_ecs::entity::Entity,
}

impl Cand {
    fn new(name: &str, initial: Vec<String>, row_bindings: usize) -> Cand {
        let (dom, root) = MockDom::new_root();
        let mut dom = CountingDom::new(dom);
        let mut app = LoomApp::new();
        app.set_row_bindings(row_bindings);
        let entity = match name {
            "K" => {
                app.add_instrumented_systems(list_reconcile_k_system::<String>);
                app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id()
            }
            "C" => {
                app.add_instrumented_systems(list_reconcile_c_system::<String>);
                app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id()
            }
            "D" => {
                app.add_instrumented_systems(list_reconcile_d_system::<String>);
                app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id()
            }
            "D-LIS" => {
                app.add_instrumented_systems(list_reconcile_d_lis_system::<String>);
                app.world.spawn((Signal(initial), ListRows::default(), DomTarget(root))).id()
            }
            "E" => {
                app.add_instrumented_systems(list_reconcile_e_system::<String>);
                app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(root))).id()
            }
            "E-LIS" => {
                app.add_instrumented_systems(list_reconcile_e_lis_system::<String>);
                app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(root))).id()
            }
            other => panic!("{other}"),
        };
        app.tick(&mut dom);
        Cand { app, dom, entity }
    }

    fn set_and_tick(&mut self, items: Vec<String>) -> (f64, DomOpCounts, StageTimings) {
        self.app.world.get_mut::<Signal<Vec<String>>>(self.entity).unwrap().0 = items;
        self.dom.reset_counts();
        let t0 = loom_runtime::now_ms();
        self.app.tick(&mut self.dom);
        let el = loom_runtime::now_ms() - t0;
        let st = *self.app.world.get_resource::<StageTimings>().unwrap();
        (el, self.dom.counts(), st)
    }
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = v.len() / 2;
    if v.len() % 2 == 1 { v[m] } else { (v[m - 1] + v[m]) / 2.0 }
}

/// 하네스는 배치 여부를 **워밍업 이전**의 5회 파일럿 median으로 정한다
/// (`run_combination`: 파일럿 루프가 후보별 WARMUP 루프보다 앞에 있다).
/// 그 5회가 워밍 후 200회 median을 얼마나 잘 대변하는가.
#[test]
fn pilot5_cold_vs_measure200_warm() {
    let n = 1000usize;
    println!("{:<14} {:<6} {:>12} {:>12} {:>10}", "workload", "cand", "pilot5(cold)", "warm200", "cold/warm");
    for w in ["swap", "head_insert", "value_update"] {
        for c in ["K", "C", "D", "E"] {
            let initial: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
            let mut cand = Cand::new(c, initial.clone(), 1);
            let mut pilot = Vec::new();
            for p in 0..5u32 {
                cand.set_and_tick(initial.clone());
                let mut rng = Pcg64::from_seed(0x0d00 + p as u64);
                let target = apply(w, &initial, &mut rng);
                pilot.push(cand.set_and_tick(target).0);
            }
            for wu in 0..50u32 {
                cand.set_and_tick(initial.clone());
                let mut rng = Pcg64::from_seed(0xa0 + wu as u64);
                let t = apply(w, &initial, &mut rng);
                cand.set_and_tick(t);
            }
            let mut meas = Vec::new();
            for rep in 0..200u32 {
                cand.set_and_tick(initial.clone());
                let mut rng = Pcg64::from_seed(0xb0 + rep as u64);
                let t = apply(w, &initial, &mut rng);
                meas.push(cand.set_and_tick(t).0);
            }
            let (p5, m200) = (median(pilot), median(meas));
            println!("{:<14} {:<6} {:>12.4} {:>12.4} {:>10.2}", w, c, p5, m200, p5 / m200);
        }
    }
}

#[test]
fn batch_alternate_forward_vs_reverse_r2() {
    // R2(binding 8개) — C의 SetRowBindingTexts가 켜지는 축.
    let n = 1000usize;
    let workloads = ["swap", "head_insert", "append", "remove_middle", "value_update"];
    let cands = ["K", "C", "D", "E"];
    println!("--- R2 (row_bindings=8), n=1000 ---");
    println!(
        "{:<14} {:<6} | {:>8} {:>8} {:>6} | {:>9} {:>9} {:>6}",
        "workload", "cand", "fwdOps", "revOps", "r_ops", "fwd_ms", "rev_ms", "r_ms"
    );
    for w in workloads {
        for c in cands {
            let initial: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
            let mut cand = Cand::new(c, initial.clone(), 8);
            let (mut fwd_ms, mut rev_ms) = (Vec::new(), Vec::new());
            let (mut fo, mut ro) = (0u32, 0u32);
            for rep in 0..8u32 {
                cand.set_and_tick(initial.clone());
                let mut rng = Pcg64::from_seed(0x5eed_1000 + rep as u64);
                let target = apply(w, &initial, &mut rng);
                for j in 0..10u32 {
                    let items = if j % 2 == 0 { target.clone() } else { initial.clone() };
                    let (ms, counts, _) = cand.set_and_tick(items);
                    if rep >= 2 {
                        if j % 2 == 0 {
                            fwd_ms.push(ms);
                            fo = counts.total_mutations();
                        } else {
                            rev_ms.push(ms);
                            ro = counts.total_mutations();
                        }
                    }
                }
            }
            let (f, r) = (median(fwd_ms), median(rev_ms));
            println!(
                "{:<14} {:<6} | {:>8} {:>8} {:>6.2} | {:>9.4} {:>9.4} {:>6.2}",
                w,
                c,
                fo,
                ro,
                fo as f64 / ro.max(1) as f64,
                f,
                r,
                f / r
            );
        }
    }
}

#[test]
fn batch_alternate_forward_vs_reverse() {
    let n = 1000usize;
    let workloads =
        ["swap", "shuffle", "shuffle10", "head_insert", "append", "remove_middle", "value_update"];
    let cands = ["K", "C", "D", "E", "D-LIS", "E-LIS"];
    println!(
        "{:<14} {:<6} | {:>8} {:>8} {:>6} | {:>9} {:>9} {:>6}",
        "workload", "cand", "fwdOps", "revOps", "r_ops", "fwd_ms", "rev_ms", "r_ms"
    );
    for w in workloads {
        for c in cands {
            let initial: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
            let mut cand = Cand::new(c, initial.clone(), 1);
            // 워밍업 + 표본: 하네스의 교대 묶음과 동일한 상태 전이 열.
            let mut fwd_ms = Vec::new();
            let mut rev_ms = Vec::new();
            let mut fwd_ops = 0u32;
            let mut rev_ops = 0u32;
            for rep in 0..12u32 {
                cand.set_and_tick(initial.clone());
                let mut rng = Pcg64::from_seed(0x5eed_0000 + rep as u64);
                let target = apply(w, &initial, &mut rng);
                for j in 0..10u32 {
                    let items = if j % 2 == 0 { target.clone() } else { initial.clone() };
                    let (ms, counts, _st) = cand.set_and_tick(items);
                    if rep >= 2 {
                        if j % 2 == 0 {
                            fwd_ms.push(ms);
                            fwd_ops = counts.total_mutations();
                        } else {
                            rev_ms.push(ms);
                            rev_ops = counts.total_mutations();
                        }
                    }
                }
            }
            let f = median(fwd_ms);
            let r = median(rev_ms);
            println!(
                "{:<14} {:<6} | {:>8} {:>8} {:>6.2} | {:>9.4} {:>9.4} {:>6.2}",
                w,
                c,
                fwd_ops,
                rev_ops,
                if rev_ops > 0 { fwd_ops as f64 / rev_ops as f64 } else { f64::INFINITY },
                f,
                r,
                if r > 0.0 { f / r } else { f64::INFINITY }
            );
        }
    }
}
