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

/// RQ1-benchmark-protocol.md §8의 프로토콜 버전 태그. 시드 유도에 들어가므로
/// 이 문자열이 바뀌면 **모든 난수열이 바뀐다** — 즉 §2.5상 전면 재수집 사유다.
/// 함부로 건드리지 말 것.
const PROTOCOL_TAG: &str = "RQ1-benchmark-protocol/v1.1";

/// §8: "PRNG = PCG64". PCG-XSL-RR 128/64 — 표준 변종을 그대로 옮긴 것이다
/// (새 PRNG를 설계하지 말 것; 이건 이미 잘 풀린 문제다).
///
/// 이전에는 xorshift64였고, 더 나빴던 것은 **시드에 셀 ID가 없었다는 점**이다
/// (`0x9E37... ^ (rep + 1)`). 서로 다른 셀이 같은 난수열을 썼고, §8이 요구하는
/// "SHA-256(태그 ‖ 셀 ID ‖ 반복 번호)"와 무관했다. 2026-08-02 교체.
struct Pcg64 {
    state: u128,
    inc: u128,
}

impl Pcg64 {
    const MULT: u128 = 0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645;

    fn from_seed(seed: u64) -> Self {
        // 표준 초기화: inc는 홀수여야 한다(스트림 선택자).
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
        // XSL RR: 상위/하위 64비트를 XOR한 뒤 상위 6비트로 회전.
        let x = self.state;
        let rot = (x >> 122) as u32;
        let xsl = ((x >> 64) as u64) ^ (x as u64);
        xsl.rotate_right(rot)
    }

    /// `[0, n)` 균등. 모듈로 편향을 제거한다 — n이 2의 거듭제곱이 아닐 때
    /// `% n`은 작은 값 쪽으로 치우치고, Fisher-Yates가 그 편향을 순열 분포에
    /// 그대로 옮긴다.
    fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
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

/// §8: "시드 = SHA-256(프로토콜 버전 태그 ‖ 셀 ID ‖ 반복 번호) **하위 64비트**".
///
/// "하위 64비트"는 다이제스트를 빅엔디언 정수로 볼 때의 하위 64비트, 즉
/// 마지막 8바이트를 빅엔디언으로 읽은 값으로 확정한다(문언에 자릿수 규약이
/// 없어 여기서 못박는다 — 제3자 재현을 위해).
///
/// `band`는 §8의 "워밍업은 별도 접두 태그의 분리 시드 대역"을 구현한다:
/// `"measure"` / `"warmup"`.
fn derive_seed(band: &str, cell_id: &str, rep: u32) -> u64 {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(PROTOCOL_TAG.as_bytes());
    h.update(b"\x1f");
    h.update(band.as_bytes());
    h.update(b"\x1f");
    h.update(cell_id.as_bytes());
    h.update(b"\x1f");
    h.update(rep.to_string().as_bytes());
    let d = h.finalize();
    u64::from_be_bytes(d[24..32].try_into().unwrap())
}

/// 셀 ID — §8이 "시드 열 전 후보 공유"를 요구하므로 **후보 이름은 들어가지
/// 않는다**. 그래야 같은 반복에서 6후보가 동일한 target 순열을 본다(paired
/// bootstrap의 전제).
fn cell_id(type_name: &str, n: usize, row_bindings: usize, row_variants: bool, workload: &str) -> String {
    format!("{type_name}|n={n}|R{row_bindings}|variants={row_variants}|{workload}")
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
    /// **W7** (§3.1) — "값 갱신: 순서 불변, 10% 아이템 값만 변경(retain 경로
    /// 표적)". 2026-08-02 신설. 이전까지 이 하네스에는 W7 셀이 **하나도**
    /// 없었다 — §3.4가 "C, D, E (retain 경로)"로 기대하는 셀 전체가 미계측이었다.
    ///
    /// **Phase 1 구현과 §3.4 기대의 어긋남(숨기지 않음)**: D의 키는 Phase 1
    /// 범위에서 아이템의 표시 텍스트 그 자체다(`RowKey(item.to_string())`,
    /// 별도 key/value 채널 없음 — `list.rs`의 D 항목 주석). 따라서 **값 변경 =
    /// 키 변경**이고, D는 W7에서 retain하지 못하고 remove+insert로 간다.
    /// §3.4의 "D (retain 경로)" 기대는 별도 key 채널을 전제한 것이다. 여기서
    /// 키 스킴을 바꾸면 §10에 없는 새 변형 축을 만드는 셈이므로 바꾸지 않고,
    /// 실측된 대로 보고한다.
    ValueUpdate,
}

impl Workload {
    const ALL: [Workload; 7] = [
        Workload::Swap,
        Workload::Shuffle,
        Workload::Shuffle10,
        Workload::HeadInsert,
        Workload::Append,
        Workload::RemoveMiddle,
        Workload::ValueUpdate,
    ];

    fn name(&self) -> &'static str {
        match self {
            Workload::Swap => "swap",
            Workload::Shuffle => "shuffle",
            Workload::Shuffle10 => "shuffle10",
            Workload::HeadInsert => "head_insert",
            Workload::Append => "append",
            Workload::RemoveMiddle => "remove_middle",
            Workload::ValueUpdate => "value_update",
        }
    }

    /// 이 워크로드가 `rng`를 전혀 쓰지 않는가 — 같은 baseline이면 시드와
    /// 무관하게 **같은 target**을 낸다.
    fn is_deterministic(&self) -> bool {
        matches!(self, Workload::Swap | Workload::RemoveMiddle)
    }

    /// Fisher-Yates, using the shared per-rep `Rng` so every candidate in a
    /// rep sees the identical target permutation (same pairing discipline
    /// as `Shuffle10`).
    fn fisher_yates<T>(v: &mut [T], rng: &mut Pcg64) {
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
    fn apply<T: Clone>(&self, baseline: &[T], rng: &mut Pcg64, fresh: impl Fn(u64) -> T) -> Vec<T> {
        // 방어 가드: 아래 분기 여럿이 `v`가 비어 있지 않다고 가정한다
        // (`v.len() - 1`, `v.remove(mid)`, `rng.below(v.len())`). 빈 입력이
        // 오면 wasm에서는 조용한 트랩(`unreachable`)이 되어 원인 추적이
        // 어려우므로 여기서 명시적으로 막는다 — 2026-08-03에 배치 측정이
        // 리스트를 비워 실제로 그 트랩이 났다.
        assert!(
            !baseline.is_empty(),
            "Workload::apply에 빈 baseline이 들어왔다 — 호출부가 리스트를 소진시켰다"
        );
        let mut v = baseline.to_vec();
        match self {
            // §3.1 W1: "원거리 2행 교환(i=1, j=n−2; prefix/suffix 절단이
            // 소거 못 하는 위치 **고정**)". 2026-08-03까지 (0, n−1)이었다 —
            // 조항이 굳이 안쪽 한 칸을 지정한 이유가 정확히 prefix/suffix
            // 절단에 먹히지 않게 하려는 것이므로 문언대로 고친다.
            Workload::Swap => {
                if v.len() >= 4 {
                    let last = v.len() - 2;
                    v.swap(1, last);
                } else {
                    let last = v.len() - 1;
                    v.swap(0, last);
                }
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
            // W7 (§3.1) — "순서 불변, 10% 아이템 값만 변경(retain 경로 표적)".
            // 인덱스는 **중복 없이** 뽑는다(부분 Fisher-Yates): 복원추출로
            // 뽑으면 실제 변경 비율이 10%에 못 미쳐 조항 문언과 어긋난다.
            Workload::ValueUpdate => {
                let k = (v.len() / 10).max(1);
                let mut idx: Vec<usize> = (0..v.len()).collect();
                for i in 0..k {
                    let j = i + rng.below(v.len() - i);
                    idx.swap(i, j);
                }
                for &i in idx.iter().take(k) {
                    v[i] = fresh(rng.next());
                }
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
    /// **2026-07-28 §7 M2 배선**: `BrowserDom`을 `CountingDom`으로 감싼다.
    /// M2(DOM 연산 수)는 §9 E 기각 (i)이 "결정적 보조 지표로 **병기**"하라고
    /// 명시적으로 요구하는데 지금까지 전면 미계측이었다(2026-07-28 계측
    /// 커버리지 감사에서 발견). `DomOps`가 트레잇이라 후보 코드는 무변경.
    dom: loom_runtime::CountingDom<BrowserDom>,
    entity: bevy_ecs::entity::Entity,
    /// 직전 반복의 M2 카운트 — 반복마다 갱신되고, 셀 종료 시 보고된다.
    last_counts: loom_runtime::DomOpCounts,
    /// 직전 반복의 §7.1 단계 귀속 시간. §9 C 기각 (ii)가 재는 "archetype move
    /// 구간"(= Commands apply sync point)이 여기 들어 있다.
    last_stages: loom_runtime::StageTimings,
    _marker: std::marker::PhantomData<T>,
}

fn measure_now() -> f64 {
    web_sys::window().unwrap().performance().unwrap().now()
}

/// §8 반복 프로토콜: "트랙 L: 셀당 워밍업 50회 후 측정 200회".
/// 2026-08-02까지 이 하네스는 **워밍업이 아예 없었고** 측정도 n에 따라
/// 30/15/6이었다 — 조항 미준수였고, 특히 n=10,000의 6회는 median/IQR/p95를
/// 말하기에 표본이 너무 적었다.
const WARMUP: u32 = 50;
const MEASURE_REPS: u32 = 200;
/// §8 배치 규칙의 묶음 크기 k.
const BATCH_K: u32 = 10;
/// 배치 전환 여부를 **셀 단위로** 정하기 위한 짧은 파일럿 반복 수.
/// 본 워밍업을 전 후보에 대해 먼저 돌리고 나서 측정하면 워밍 상태가 날아가므로
/// 판정만 따로 뽑는다.
const PILOT: u32 = 5;
/// §8 셀 상한: "셀당 벽시계 상한 30분(전 트랙)".
const CELL_CAP_MS: f64 = 30.0 * 60.0 * 1000.0;

/// §8 "(후보 × 셀) 시드 셔플 무작위화". 시드 유도가 결정적이므로 순서도
/// 재현 가능하다.
fn shuffled_indices(len: usize, scope: &str, band: &str) -> Vec<usize> {
    let mut v: Vec<usize> = (0..len).collect();
    let mut rng = Pcg64::from_seed(derive_seed(band, scope, 0));
    for i in (1..v.len()).rev() {
        v.swap(i, rng.below(i + 1));
    }
    v
}

/// §8: "셀별 유효 해상도를 실행 시점 기록". `performance.now()`가 실제로
/// 구별해 내는 최소 간격 — 값을 계속 읽어 **0이 아닌 최소 증분**을 찾는다.
/// COOP/COEP 격리 하에서는 5µs, 아니면 100µs 근처가 나온다.
fn measure_timer_resolution() -> f64 {
    let mut min_delta = f64::INFINITY;
    for _ in 0..32 {
        let a = measure_now();
        let mut b = measure_now();
        // 같은 눈금이면 바뀔 때까지 돈다.
        let mut spins = 0u32;
        while b == a && spins < 1_000_000 {
            b = measure_now();
            spins += 1;
        }
        let d = b - a;
        if d > 0.0 && d < min_delta {
            min_delta = d;
        }
    }
    if min_delta.is_finite() { min_delta } else { 0.0 }
}

/// `requestAnimationFrame` 두 번을 기다린다.
///
/// 왜 **두 번**인가: 한 번은 "다음 프레임의 렌더 직전"까지만 보장한다. 두
/// 번째 콜백이 불렸다는 것은 첫 프레임이 실제로 **완결**(style·layout·paint)
/// 됐다는 뜻이다 — §7 M1(b)가 재려는 것이 그 시점이고, §8이 복원↔측정
/// 경계로 요구하는 것도 "복원이 남긴 지연 작업까지 끝난 뒤"다.
async fn double_raf() {
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let window = web_sys::window().unwrap();
        let w2 = window.clone();
        let inner = Closure::once_into_js(move || {
            let cb = Closure::once_into_js(move || {
                resolve.call0(&JsValue::NULL).ok();
            });
            w2.request_animation_frame(cb.unchecked_ref()).ok();
        });
        window.request_animation_frame(inner.unchecked_ref()).ok();
    });
    let _ = wasm_bindgen_futures::JsFuture::from(promise).await;
}

/// §7 M2 교차 검증. `body`가 내는 DOM 변형을 MutationObserver로 관측해
/// 채널별로 센다.
///
/// 설정은 §7 M2 (v1.1 정정 M-11)가 명시한 그대로 — childList +
/// characterData + attributes + attributeOldValue + subtree. 대응표도 조항의
/// 것이다: insert/remove/move 폴백/detach/attach → childList, setText →
/// characterData, setAttr → attributes.
///
/// **네이티브 `moveBefore`는 관측상 remove+insert 쌍으로 보일 수 있다**(§7 M2
/// 주석 요구). 따라서 childList의 added/removed 합이 우리 카운트보다 클 수
/// 있고, 그건 불일치가 아니라 관측 채널의 성질이다 — 판정에 쓰지 말고
/// 병기만 할 것.
fn observe_mutations(target: &web_sys::Node, body: impl FnOnce()) -> String {
    let noop = Closure::<dyn FnMut(js_sys::Array, web_sys::MutationObserver)>::new(
        |_: js_sys::Array, _: web_sys::MutationObserver| {},
    );
    let Ok(observer) = web_sys::MutationObserver::new(noop.as_ref().unchecked_ref()) else {
        body();
        return String::from("null");
    };
    let init = web_sys::MutationObserverInit::new();
    init.set_child_list(true);
    init.set_character_data(true);
    init.set_attributes(true);
    init.set_attribute_old_value(true);
    init.set_subtree(true);
    if observer.observe_with_options(target, &init).is_err() {
        body();
        return String::from("null");
    }

    body();

    let records = observer.take_records();
    observer.disconnect();

    let (mut child_list, mut character_data, mut attributes) = (0u32, 0u32, 0u32);
    let (mut added, mut removed) = (0u32, 0u32);
    for r in records.iter() {
        let rec: web_sys::MutationRecord = r.unchecked_into();
        match rec.type_().as_str() {
            "childList" => {
                child_list += 1;
                added += rec.added_nodes().length();
                removed += rec.removed_nodes().length();
            }
            "characterData" => character_data += 1,
            "attributes" => attributes += 1,
            _ => {}
        }
    }
    format!(
        "{{\"child_list\": {child_list}, \"added_nodes\": {added}, \"removed_nodes\": {removed}, \
         \"character_data\": {character_data}, \"attributes\": {attributes}}}"
    )
}

fn median_of(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let m = v.len() / 2;
    if v.len() % 2 == 1 { v[m] } else { (v[m - 1] + v[m]) / 2.0 }
}

impl<T: Send + Sync + Clone + ToString + 'static> Candidate<T> {
    fn set_and_tick(&mut self, items: Vec<T>) -> f64 {
        // §8 "각 반복 전 초기 상태 복원"과 같은 지점에서 카운터를 0으로 —
        // 셋업 단계의 op가 측정 반복에 새지 않게 한다.
        self.dom.reset_counts();
        // §7 M1(a): "**조작 트리거 직전** performance.now() = t0".
        //
        // 2026-08-04 이전에는 Signal 대입을 끝낸 **뒤에** t0를 찍었다. 대입은
        // 새 Vec를 move-in하고 **직전 리스트를 그 자리에서 drop**한다 — heap
        // 아이템이면 n개의 String 해제라 O(n)이다. 네이티브 실측으로 n=1,000
        // 0.0062ms / n=10,000 0.0672ms이고 wasm의 dlmalloc은 통상 2~4배이므로
        // 그만큼이 측정 괄호 밖에 있었다. 같은 하네스가 M1(b)에서는 t0를 대입
        // **앞**에 찍고 있어서 같은 조항의 (a)와 (b)가 서로 다른 t0를 썼다.
        let start = measure_now();
        self.app.world.get_mut::<Signal<Vec<T>>>(self.entity).unwrap().0 = items;
        self.app.tick(&mut self.dom);
        let elapsed = measure_now() - start;
        self.last_counts = self.dom.counts();
        if let Some(t) = self.app.world.get_resource::<loom_runtime::StageTimings>() {
            self.last_stages = *t;
        }
        elapsed
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
    let (dom, handle) = BrowserDom::new(document.clone(), root);
    let mut dom = loom_runtime::CountingDom::new(dom);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_instrumented_systems(list_reconcile_k_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "K", app, dom, entity, last_counts: Default::default(), last_stages: Default::default(), _marker: std::marker::PhantomData }
}

fn make_c<T: Send + Sync + Clone + ToString + PartialEq + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "C");
    let (dom, handle) = BrowserDom::new(document.clone(), root);
    let mut dom = loom_runtime::CountingDom::new(dom);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_instrumented_systems(list_reconcile_c_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "C", app, dom, entity, last_counts: Default::default(), last_stages: Default::default(), _marker: std::marker::PhantomData }
}

fn make_d<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "D");
    let (dom, handle) = BrowserDom::new(document.clone(), root);
    let mut dom = loom_runtime::CountingDom::new(dom);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_instrumented_systems(list_reconcile_d_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "D", app, dom, entity, last_counts: Default::default(), last_stages: Default::default(), _marker: std::marker::PhantomData }
}

fn make_e<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "E");
    let (dom, handle) = BrowserDom::new(document.clone(), root);
    let mut dom = loom_runtime::CountingDom::new(dom);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_instrumented_systems(list_reconcile_e_system::<T>);
    let entity =
        app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "E", app, dom, entity, last_counts: Default::default(), last_stages: Default::default(), _marker: std::marker::PhantomData }
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
    let (dom, handle) = BrowserDom::new(document.clone(), root);
    let mut dom = loom_runtime::CountingDom::new(dom);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_instrumented_systems(list_reconcile_d_lis_system::<T>);
    let entity = app.world.spawn((Signal(initial), ListRows::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "D-LIS", app, dom, entity, last_counts: Default::default(), last_stages: Default::default(), _marker: std::marker::PhantomData }
}

/// RQ1's LIS-optimized variant of E.
fn make_e_lis<T: Send + Sync + Clone + ToString + 'static>(
    document: &web_sys::Document,
    parent: &web_sys::Node,
    initial: Vec<T>,
    row_bindings: usize,
) -> Candidate<T> {
    let root = make_container(document, parent, "E-LIS");
    let (dom, handle) = BrowserDom::new(document.clone(), root);
    let mut dom = loom_runtime::CountingDom::new(dom);
    let mut app = LoomApp::new();
    app.set_row_bindings(row_bindings);
    app.add_instrumented_systems(list_reconcile_e_lis_system::<T>);
    let entity =
        app.world.spawn((Signal(initial), OrderKeyMembers::default(), DomTarget(handle))).id();
    app.tick(&mut dom);
    Candidate { name: "E-LIS", app, dom, entity, last_counts: Default::default(), last_stages: Default::default(), _marker: std::marker::PhantomData }
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
///
/// **왜 `async`인가 (2026-08-02)**: §8이 "복원과 측정 사이 double-rAF 1회
/// 경계(복원 비용 누출 차단)"를 요구한다. 이전 하네스는 복원 직후 곧바로
/// 측정해 이 조항이 미이행이었다 — 복원이 남긴 style/layout/GC가 다음 측정
/// 괄호 안으로 샐 수 있었다. rAF를 기다리려면 async여야 하고, 같은 구조에
/// M1(b)(double-rAF 프레임 완결 시간)도 얹힌다.
#[allow(clippy::too_many_arguments)]
async fn run_combination<T, F>(
    document: &web_sys::Document,
    scratch_parent: &web_sys::Node,
    type_name: &str,
    n: usize,
    reps: u32,
    initial: Vec<T>,
    fresh: F,
    row_bindings: usize,
    // RQ1 §3.2의 R3 축(이질 브랜치 row). §3.3이 "R3: W2 × R3 × n=1,000 ×
    // {C, D}(E·K는 참고 측정)"으로 셀을 고정했다.
    row_variants: bool,
    // §8 "셀마다 fresh page load"를 위한 워크로드 선택. `Some(i)`이면
    // 셔플된 워크로드 순서의 i번째 하나만 돌린다(그 페이지의 몫).
    only_workload: Option<usize>,
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
        .map(|maker| {
            let mut c = maker(document, &combo_node, initial.clone(), row_bindings);
            // R3 축(§3.2) 적용. 초기 spawn은 이미 끝난 뒤이므로 첫 재정렬부터
            // 브랜치 전환이 일어난다 — §9 C 기각 (ii)가 재는 것이 바로 그
            // "재정렬마다의 archetype move"이므로 문제되지 않는다.
            c.app.set_row_variant_mode(row_variants);
            c
        })
        .collect();

    // §8: "셀별 유효 해상도를 실행 시점 기록". 배치 규칙의 임계값이 여기서
    // 나오므로 조합마다 새로 잰다(스로틀·격리 상태가 바뀌면 값이 달라진다).
    let resolution_ms = measure_timer_resolution();
    let batch_threshold_ms = resolution_ms * 50.0;

    // §8: "셀 실행 순서는 (후보 × 셀) 시드 셔플 무작위화 — thermal drift 교락
    // 방지". 2026-08-03까지 미이행이었다(워크로드도 후보도 고정 순서였고,
    // 후보 간 시간 간격이 수 분이라 드리프트가 후보에 계통적으로 실렸다).
    // 시드는 조합 단위로 유도해 재현 가능하게 둔다.
    let combo_id = format!("{type_name}|n={n}|R{row_bindings}|variants={row_variants}");
    let workload_order = shuffled_indices(Workload::ALL.len(), &combo_id, "workload-order");

    for (wpos, &wi) in workload_order.iter().enumerate() {
        if only_workload.is_some_and(|w| w != wpos) {
            continue;
        }
        let workload = &Workload::ALL[wi];
        let cid = cell_id(type_name, n, row_bindings, row_variants, workload.name());
        let cand_order = shuffled_indices(candidates.len(), &cid, "candidate-order");

        // -------------------------------------------------------------------
        // §8 배치 규칙의 판정을 **셀 단위로 한 번** 내린다.
        //
        // 2026-08-03까지는 후보마다 따로 정했는데, 그러면 같은 셀에서 어떤
        // 후보는 `derive_seed(.., rep*K+j)`를, 어떤 후보는 `derive_seed(.., rep)`를
        // 쓰게 되어 §8이 paired 분석의 전제로 못박은 "시드 열 전 후보 공유"가
        // 깨진다. 실측 median으로 실제로 갈렸다(n=1,000 R1 swap: C만 배치,
        // 나머지 5후보 비배치 — 그것도 §9 C 채택 조건이 걸린 판정 셀이다).
        //
        // 판정 기준은 **최소** median: 한 후보라도 해상도 아래면 그 후보의
        // 표본이 못 쓰게 되므로 셀 전체를 배치로 올린다. 느린 후보는 10배 일을
        // 하고 10으로 나누는 것뿐이라 잃는 게 없다.
        //
        // 추정치는 짧은 파일럿에서 얻는다 — 본 워밍업을 전 후보에 대해 먼저
        // 돌리고 나서 측정하면 워밍업과 측정 사이에 다른 후보들이 끼어들어
        // 워밍 상태가 날아간다.
        // -------------------------------------------------------------------
        let mut pilot_min = f64::INFINITY;
        for &ci in &cand_order {
            let mut ps = Vec::with_capacity(PILOT as usize);
            for w in 0..PILOT {
                candidates[ci].set_and_tick(initial.clone());
                double_raf().await;
                let mut rng = Pcg64::from_seed(derive_seed("pilot", &cid, w));
                let target = workload.apply(&initial, &mut rng, &fresh);
                ps.push(candidates[ci].set_and_tick(target));
            }
            pilot_min = pilot_min.min(median_of(&ps));
        }
        let batched = pilot_min < batch_threshold_ms;
        // -------------------------------------------------------------------
        // §8 배치 규칙의 묶음 구성 — **전부 baseline↔target 교대**.
        //
        // 2026-08-04 이전에는 rng를 쓰는 워크로드를 "매 target을 baseline에서
        // 새로 유도해 이어 붙이는"(chain) 방식으로 묶었다. 그 방식은 두 번
        // 틀렸다:
        //
        //   (1) rng를 안 쓰는 워크로드(W1 swap, remove_middle)에서는 10회 중
        //       9회가 **무변화**가 된다(같은 시드든 아니든 같은 target).
        //       retain 계열은 DOM write를 0개 내고 K는 값과 무관하게 전량
        //       재스폰하므로 후보별 비대칭 감쇠가 생겼다. → 2026-08-03 수리.
        //   (2) **길이가 변하는 워크로드에서는 남는 9회가 워크로드가 아니다.**
        //       head_insert의 t_{j-1}→t_j는 "머리에 삽입"이 아니라 "머리 항목
        //       교체"다. 위치 귀속 후보 C는 삽입에서 전 슬롯을 다시 쓰지만
        //       (setText 1001회) 교체에서는 슬롯 하나만 쓴다(1회) — 실측에서
        //       chain 셀의 C가 정확히 `setText=1`로 나왔다. K는 어차피 전량
        //       재스폰이라 1001을 그대로 내므로 **C만 10배 싸진다**. 이 때문에
        //       Firefox 열에서 §9 C 채택 (a)가 충족으로 뒤집혔다(Chromium은
        //       같은 셀이 비배치라 영향 없음).
        //
        // 교대는 두 문제를 함께 없앤다: 10회 전부 실제 상태 전이이고, 길이가
        // baseline과 target 사이에서만 오간다. 되돌림은 정방향의 정확한 역이라
        // (삽입↔삭제, 재정렬↔복원) 크기가 같은 연산이며, M2/단계 귀속은
        // **정방향 전이**의 값을 따로 붙잡아 보고한다.
        // -------------------------------------------------------------------
        let batch_mode = if batched { "alternate" } else { "none" };

        // -------------------------------------------------------------------
        // 측정 — **rep 단위 라운드로빈**으로 후보를 교대한다.
        //
        // 2026-08-04 이전에는 후보를 통째 블록으로 순차 측정했다
        // (`for 후보 { 워밍업 50 → 측정 200 }`). 그러면 (셀, rep i)의 K 표본과
        // D 표본이 수 초~수십 초 떨어진 서로 다른 시간창에서 나온 값이 되는데,
        // §9의 모든 판정은 `paired_ratio_ci`가 **같은 rep 인덱스로 두 후보를
        // 리샘플**해서 낸다. 시드 공유는 *같은 target*을 보장할 뿐 *같은 측정
        // 환경*을 보장하지 않으므로, 블록 수준 오프셋(thermal/JIT 드리프트)이
        // 부트스트랩이 리샘플하지 않는 성분이 되어 CI에서 통째로 빠진다.
        // 게다가 판정 셀의 절반(W1 swap, remove_middle)은 rng를 안 써서 200
        // rep이 전부 같은 target이라, 그 셀에서 인덱스 짝짓기는 문자 그대로
        // 아무것도 짝짓지 않았다.
        //
        // 라운드로빈이면 한 rep 안의 6후보가 같은 시간창(약 0.2초)을 공유하므로
        // 짝짓기가 의미를 갖는다. tick 총수는 그대로다.
        // -------------------------------------------------------------------
        let cell_started_ms = measure_now();
        let mut nat0 = vec![0u64; candidates.len()];
        let mut fb0 = vec![0u64; candidates.len()];
        for &ci in &cand_order {
            let (n0, f0) = loom_runtime::DomOps::reposition_split(&candidates[ci].dom);
            nat0[ci] = n0;
            fb0[ci] = f0;
        }

        // §8 워밍업: "셀당 워밍업 50회". 시드는 **분리 대역**("warmup")에서
        // 뽑는다 — 측정 반복이 워밍업이 이미 데운 바로 그 난수열을 다시 보면
        // 캐시 워밍 편향이 생긴다. 워밍업도 라운드로빈으로 돌려 측정 직전의
        // 상태가 후보 간에 같아지게 한다.
        for w in 0..WARMUP {
            for &ci in &cand_order {
                candidates[ci].set_and_tick(initial.clone());
                double_raf().await; // §8 복원↔측정 경계
                let mut rng = Pcg64::from_seed(derive_seed("warmup", &cid, w));
                let target = workload.apply(&initial, &mut rng, &fresh);
                candidates[ci].set_and_tick(target);
            }
        }

        let mut samples: Vec<Vec<f64>> = vec![Vec::with_capacity(reps as usize); candidates.len()];
        let mut fwd_counts = vec![loom_runtime::DomOpCounts::default(); candidates.len()];
        let mut fwd_stages = vec![loom_runtime::StageTimings::default(); candidates.len()];
        let mut over_cap = false;
        'measure: for rep in 0..reps {
            for &ci in &cand_order {
                let candidate = &mut candidates[ci];
                // Reset to the identical baseline (untimed) before each
                // timed rep, so every rep starts from the same condition.
                candidate.set_and_tick(initial.clone());
                double_raf().await; // §8 복원↔측정 경계
                let mut rng = Pcg64::from_seed(derive_seed("measure", &cid, rep));
                let target = workload.apply(&initial, &mut rng, &fresh);
                if batch_mode == "alternate" {
                    // §8 배치 규칙: "k=10회 묶음 총 시간/k = 1측정치".
                    //
                    // **묶음 안의 매 회에도 프레임 경계를 준다**(2026-08-04).
                    // 그 전에는 double-rAF가 묶음 앞에 하나뿐이라 j=1..9가 한
                    // JS 태스크에서 연속 실행됐고, 그 사이 style/layout/paint가
                    // 한 번도 돌지 않았다 — §8이 "복원 비용 누출 차단"을 위해
                    // 요구한 바로 그 경계가 묶음 내부에 없었다. 그 결과 배치
                    // 셀은 같은 조작을 **0.28~0.98배로 계통 감쇠**해서 쟀고,
                    // 감쇠 계수가 후보마다 달라(K 0.525 / C 0.653 / D 0.716)
                    // 셀 안에서 상쇄되지 않았다. §9가 읽는 median 비율이
                    // 12~110% 왜곡됐다(같은 셀 force_batch A/B로 실측 확인).
                    //
                    // 경계를 주면 배치는 **순수한 양자화 저감 장치**가 된다 —
                    // 같은 물리량을 10회 재서 평균할 뿐이다. 대가는 벽시계고,
                    // 그건 조항이 요구한 정밀도의 값이다.
                    let mut total = 0.0;
                    for j in 0..BATCH_K {
                        if j > 0 {
                            double_raf().await;
                        }
                        let items = if j % 2 == 0 { target.clone() } else { initial.clone() };
                        total += candidate.set_and_tick(items);
                        // M2/단계 귀속은 **정방향 전이**의 값이어야 한다 —
                        // 교대 묶음의 마지막(j=K-1, 홀수)은 되돌림이다.
                        if j % 2 == 0 {
                            fwd_counts[ci] = candidate.last_counts;
                            fwd_stages[ci] = candidate.last_stages;
                        }
                    }
                    samples[ci].push(total / BATCH_K as f64);
                } else {
                    samples[ci].push(candidate.set_and_tick(target));
                    fwd_counts[ci] = candidate.last_counts;
                    fwd_stages[ci] = candidate.last_stages;
                }
            }
            // §8 셀 상한: "셀당 벽시계 상한 30분(전 트랙). 초과 시 임기응변
            // 중단이 아니라 '측정 불가(상한 초과)' 프로파일 등재."
            if measure_now() - cell_started_ms > CELL_CAP_MS {
                over_cap = true;
                break 'measure;
            }
        }

        for &ci in &cand_order {
            let candidate = &mut candidates[ci];
            let samples = &samples[ci];
            // §9 C 기각 (ii)/E 기각 (ii)가 읽는 `m2_dom_ops`·`stage_ms`는
            // **마지막 측정 반복**의 값이어야 한다. 아래 M1(b)/MutationObserver
            // tick이 이것들을 덮어쓰므로 여기서 붙잡아 둔다 — 2026-08-03까지는
            // 덮어쓰인 뒤라 관측자가 붙어 있는 tick의 값이 나갔고, 관측자
            // 오버헤드는 op 수에 비례해 **후보별로 비대칭**이라 그 두 조항이
            // 오염된 값을 읽고 있었다.
            // 주의: `reposition_*_cell_total`은 이 셀 전체(파일럿+워밍업+측정+
            // M1(b)+MO)의 누계이고 `m2_dom_ops`는 **마지막 측정 tick 1회**의
            // 값이다. 스케일이 다르므로 이름에 그 사실을 넣었다 — 감사가
            // "두 필드의 시간 스케일이 다르다는 점을 레코드만 봐서는 알 수
            // 없다"고 지적한 지점.
            let (measured_counts, measured_stages) = (fwd_counts[ci], fwd_stages[ci]);
            let (nat1, fb1) = loom_runtime::DomOps::reposition_split(&candidate.dom);
            let (native, fallback) = (nat1 - nat0[ci], fb1 - fb0[ci]);

            // ---------------------------------------------------------------
            // §7 M1(b): "프레임 완결 시간 = double-rAF 콜백 − t0". 판정에는
            // 쓰지 않는다(§7 M1 — vsync 양자화로 판정 부적합, 프로파일 전용).
            // 그래서 **셀당 1회**만 잰다.
            // ---------------------------------------------------------------
            candidate.set_and_tick(initial.clone());
            double_raf().await;
            let m1b_ms = {
                let mut rng = Pcg64::from_seed(derive_seed("m1b", &cid, 0));
                let target = workload.apply(&initial, &mut rng, &fresh);
                let t0 = measure_now();
                candidate.set_and_tick(target);
                double_raf().await;
                measure_now() - t0
            };

            // ---------------------------------------------------------------
            // §7 M2 교차 검증: "셀당 1회는 MutationObserver(childList +
            // characterData + attributes(+attributeOldValue), subtree)로".
            // `takeRecords()`가 동기이므로 tick 직후 그대로 꺼낼 수 있다.
            // ---------------------------------------------------------------
            candidate.set_and_tick(initial.clone());
            double_raf().await;
            let mutation_json = {
                // §7 M2 교차 검증은 `m2_dom_ops`와 **같은 전이**를 봐야 의미가
                // 있다 — 2026-08-04까지 별도 난수 대역("mo")을 써서 다른 표적을
                // 관측하고 있었고, 그러면 두 수치를 대조할 근거가 없다.
                // 마지막 측정 rep과 같은 시드를 쓴다.
                let mut rng = Pcg64::from_seed(derive_seed("measure", &cid, reps - 1));
                let target = workload.apply(&initial, &mut rng, &fresh);
                observe_mutations(&combo_node, || {
                    candidate.set_and_tick(target);
                })
            };
            // 셀 벽시계는 라운드로빈 구조상 (조합 × 워크로드) 전체의 값이다.
            let cell_ms = measure_now() - cell_started_ms;
            let live_nodes = loom_runtime::DomOps::live_node_count(&candidate.dom).unwrap_or(0);

            // **셀 단위 진행 보고 (2026-08-04)**. 조합 단위로는 "느린 것"과
            // "멈춘 것"을 구별할 수 없다 — stock Firefox 수집이 n=10,000
            // 조합에서 228분간 진행 없이 상한에 걸렸는데, 42개 셀 중 어디서
            // 그러는지 알 방법이 없었다. 셀이 끝날 때마다 한 줄 보낸다
            // (측정 구간 밖).
            report_progress(&format!(
                "{{\"kind\": \"cell\", \"combination\": \"{type_name} n={n} R{row_bindings} variants={row_variants}\", \
                  \"workload\": \"{}\", \"candidate\": \"{}\", \"cell_ms\": {:.0}, \
                  \"over_cap\": {}, \"elapsed_ms\": {:.0}}}",
                workload.name(),
                candidate.name,
                cell_ms,
                over_cap,
                measure_now(),
            ));

            if !*first_entry {
                out.push_str(",\n");
            }
            *first_entry = false;
            let samples_json = samples
                .iter()
                .map(|s| format!("{s:.4}"))
                .collect::<Vec<_>>()
                .join(", ");
            // §7 M2: 마지막 반복의 op 종류별 카운트를 병기한다. §9 E 기각 (i)이
            // "결정적 보조 지표로 병기"를 명시 요구하는 값 — 시드 고정 하에
            // 결정적이므로(§7 M2) 반복마다 같아야 하고, 따라서 마지막 1회로 충분하다.
            out.push_str(&format!(
                "  {{\"candidate\": \"{}\", \"workload\": \"{}\", \"item_type\": \"{}\", \"n\": {}, \"row_bindings\": {}, \"reps\": {}, \"warmup\": {}, \"samples_ms\": [{}], \"m2_dom_ops\": {}, \"row_variants\": {}, \
                 \"timer_resolution_ms\": {:.6}, \"batched\": {}, \"batch_mode\": \"{}\", \"batch_k\": {}, \
                 \"reposition_native_cell_total\": {}, \"reposition_fallback_cell_total\": {}, \
                 \"m1b_frame_ms\": {:.4}, \"m1b_reps\": 1, \"mutation_records\": {}, \
                 \"cell_wall_ms\": {:.1}, \"over_cell_cap\": {}, \"live_node_slots\": {}, \
                 \"user_agent\": \"{}\", \"cross_origin_isolated\": {}, \
                 \"stage_ms\": {{\"systems\": {:.4}, \"sync_point\": {:.4}, \
                 \"dom_apply\": {:.4}, \"total\": {:.4}}}}}",
                candidate.name,
                workload.name(),
                type_name,
                n,
                row_bindings,
                reps,
                WARMUP,
                samples_json,
                measured_counts.to_json(),
                row_variants,
                resolution_ms,
                batched,
                batch_mode,
                if batched { BATCH_K } else { 1 },
                native,
                fallback,
                m1b_ms,
                mutation_json,
                cell_ms,
                over_cap,
                live_nodes,
                user_agent(),
                cross_origin_isolated(),
                measured_stages.systems_ms,
                measured_stages.sync_point_ms,
                measured_stages.dom_apply_ms,
                measured_stages.total_ms,
            ));
        }
    }

    // Tear down this combination's DOM before the next one mounts.
    scratch_parent.remove_child(&combo_container).ok();

    // 진행 상황을 밖에서 볼 수 있게 남긴다. `#results`는 전 조합이 끝난 뒤에야
    // 한 번에 기록되므로(그게 원자적 결과 규약이다), 드라이버가 "살아 있는지"를
    // 판단할 근거가 없었다 — 2026-08-03의 6시간 무음 매달림 때 정확히 이게
    // 없어서 죽었는지 도는지 구별할 수 없었다.
    let progress = format!(
        "{{\"combination\": \"{type_name} n={n} R{row_bindings} variants={row_variants}\", \
          \"records\": {}, \"elapsed_ms\": {:.0}}}",
        out.matches("\"candidate\"").count(),
        measure_now(),
    );
    if let Some(win) = web_sys::window() {
        let _ = js_sys::Reflect::set(
            &win,
            &JsValue::from_str("__loomProgress"),
            &JsValue::from_str(&progress),
        );
    }
    report_progress(&progress);
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    // 하네스가 async가 됐으므로(§8 복원↔측정 double-rAF 경계) 시작 함수는
    // 즉시 반환하고 본체는 마이크로태스크로 넘긴다. `#results`가 채워지는
    // 시점이 늦어질 뿐, 그걸 폴링하는 e2e 드라이버 쪽은 무변경으로 동작한다.
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = run_all().await {
            web_sys::console::error_1(&e);
        }
    });
    Ok(())
}

async fn run_all() -> Result<(), JsValue> {
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
    // per row — RQ1-benchmark-protocol.md §3.2). 워크로드는 W1~W5 +
    // remove_middle + W7(값 갱신).
    //
    // **반복 수(2026-08-02)**: n에 따라 30/15/6으로 줄이던 것을 §8 문언대로
    // 전 셀 워밍업 50 + 측정 200으로 통일했다. 이전 규칙은 "큰 n은 신호가
    // 노이즈를 압도하니 적게"라는 하네스 자체 판단이었는데, §8은 그런 재량을
    // 주지 않고 n=10,000의 6회는 median/IQR/p95를 말하기에 표본이 모자랐다.
    // -----------------------------------------------------------------------
    // §8 격리 조항: "새 프로필, 확장 없음, **셀마다 fresh page load**".
    //
    // 하네스는 2026-08-04까지 252개 셀 전부를 **한 페이지 로드**에서 돌렸고
    // (모듈 doc이 "same page load so results share hardware/JIT-warmup
    // conditions"라고 이점으로 서술했지만 조항과 반대 방향이라는 사실은
    // 어디에도 없었다), 그 대가가 실측으로 드러났다 — stock Firefox에서
    // n=10,000 조합이 **한 페이지에 누적된 상태에서는 228분 넘게 안 끝나는데**,
    // 같은 조합을 새 페이지에서 단독으로 돌리면 셀당 소요가 3.7배(Chromium
    // 대비)로 정상 범위였다. 즉 오래 산 페이지에서 측정치가 열화한다.
    //
    // `?group=<i>`는 (조합 × 워크로드) 하나만 돌리고, 끝나면 `i+1`로 **스스로
    // 이동**한다. 브라우저 프로세스는 하나지만 페이지는 매번 새로 뜨므로
    // 조항에 맞고 누적 열화도 끊긴다.
    //
    // 왜 (조합 × 워크로드)이지 (조합 × 워크로드 × 후보)가 아닌가: §8의 배치
    // 규칙 판정과 시드 열 공유가 **한 셀의 6후보를 함께** 보도록 요구하기
    // 때문이다. 후보별로 페이지를 쪼개면 그 전제가 깨진다.
    // -----------------------------------------------------------------------
    let group: Option<usize> = query_param("group").and_then(|v| v.parse().ok());
    // 조합 개수: String n∈{100,1000,10000} + POD n=1000 + R2 + R3 = 6.
    const COMBINATIONS: usize = 6;
    let combos = COMBINATIONS;
    let (only_n, only_workload, next_group) = match group {
        Some(g) if g < combos * Workload::ALL.len() => {
            (Some(g / Workload::ALL.len()), Some(g % Workload::ALL.len()), Some(g + 1))
        }
        Some(_) => (Some(usize::MAX), Some(usize::MAX), None), // 범위 밖 = 끝
        None => (None, None, None),
    };

    // `?only=<n>` — **진단 전용** 조합 선택(레거시). 판정 데이터는 항상 전
    // 조합을 돌린 것이어야 한다.
    let only_n: Option<usize> = match only_n {
        Some(_) => None, // group 모드에서는 아래 조합 인덱스로 거른다
        None => query_param("only").and_then(|v| v.parse().ok()),
    };
    let combo_index: Option<usize> = group.map(|g| g / Workload::ALL.len());
    let mut ci_counter: usize = 0;
    // **§3.2 "Item 타입 축: ... 기본 POD, W7과 트랙 R에서 양쪽."**
    //
    // 2026-08-04 이전에는 정반대로 String(heap)을 전 축의 기본으로 돌리고 POD는
    // n=1,000 × R1 한 조합만 돌렸다. 그래서 §9 판정 셀 전부(§3.3 핵심 격자)가
    // 조항의 기본값이 아닌 아이템 타입에서 나왔고, R2(= D 기각 (iii)·D 양성·
    // E 기각 (i)의 셀)에는 POD 레코드가 **하나도 없어** 조항 기본값으로는
    // 판정 자체가 불가능했다.
    //
    // 이제 POD가 기본 축이고(n 스케일링 + R2 + R3), String은 한 조합만 남긴다 —
    // §3.2가 양쪽을 요구하는 것은 W7과 트랙 R뿐이고, W7은 그 String 조합에
    // 들어 있다.
    for &n in &[100usize, 1000, 10000] {
        let this = ci_counter;
        ci_counter += 1;
        if only_n.is_some_and(|o| o != n) || combo_index.is_some_and(|c| c != this) {
            continue;
        }
        let reps = MEASURE_REPS;
        let initial: Vec<ItemId> = (0..n as u32).map(ItemId).collect();
        run_combination(
            &document,
            &container_node,
            "ItemId(POD u32)",
            n,
            reps,
            initial,
            |seed| ItemId(1_000_000 + (seed as u32 % 1_000_000)),
            1, // R1
            false, // R3 축 꺼짐
            only_workload,
            &mut report,
            &mut first_entry,
        ).await;
    }

    // `?only=`이 지정되면 아래 세 조합(POD·R2·R3)은 건너뛴다 — 진단 전용.
    let skip_extra = only_n.is_some();
    macro_rules! combo_gate {
        () => {{
            let this = ci_counter;
            ci_counter += 1;
            skip_extra || combo_index.is_some_and(|c| c != this)
        }};
    }

    // POD (u32 newtype) axis: one representative size (n=1000), not the
    // full N-scaling sweep — a partial, not full, cross-check of RQ1's
    // heap-vs-POD item axis (stated plainly, not silently narrowed).
    if !combo_gate!() {
        let n = 1000;
        // §3.2가 "양쪽"을 요구하는 것은 W7과 트랙 R이다 — 이 조합이 W7의
        // heap 쪽을 담당하고, 나머지 워크로드는 참고 프로파일이 된다.
        let initial: Vec<String> = (0..n).map(|i| format!("item-{i}")).collect();
        run_combination(
            &document,
            &container_node,
            "String(heap)",
            n,
            MEASURE_REPS,
            initial,
            |seed| format!("new-{seed}"),
            1, // R1
            false, // R3 축 꺼짐
            only_workload,
            &mut report,
            &mut first_entry,
        ).await;
    }

    // Row-depth axis: R2 ("binding 8개", RQ1-benchmark-protocol.md §3.2) at
    // n=1,000 String(heap) — needed specifically for RQ1 §9's D-rejection
    // gate (iii), which requires `{W1,W2,W3,W4}×{R1,R2}`. Not a full
    // N-scaling sweep at R2 (stated plainly, matching the POD axis's own
    // partial-coverage precedent) — the D-vs-K decision this axis exists to
    // inform doesn't need one (see research-questions/
    // RQ1-dynamic-control-flow-scheduling.md's row-depth write-up).
    if !combo_gate!() {
        let n = 1000;
        let initial: Vec<ItemId> = (0..n as u32).map(ItemId).collect();
        run_combination(
            &document,
            &container_node,
            "ItemId(POD u32)",
            n,
            MEASURE_REPS,
            initial,
            |seed| ItemId(1_000_000 + (seed as u32 % 1_000_000)),
            8, // R2
            false, // R3 축 꺼짐
            only_workload,
            &mut report,
            &mut first_entry,
        ).await;
    }

    // R3 축(§3.3): W2 × R3 × n=1,000. 판정 대상은 {C, D}이고 E·K는 참고
    // 측정이지만, 하네스는 6후보를 한 조합에서 함께 돌리므로 전부 수집된다
    // (§2.4 동일 조건 유지에도 유리).
    if !combo_gate!() {
        let n = 1000;
        let initial: Vec<ItemId> = (0..n as u32).map(ItemId).collect();
        run_combination(
            &document,
            &container_node,
            "ItemId(POD u32)",
            n,
            MEASURE_REPS,
            initial,
            |seed| ItemId(1_000_000 + (seed as u32 % 1_000_000)),
            1, // R3는 row 깊이와 독립 축이므로 binding은 R1로 둔다
            true, // R3 축 켬
            only_workload,
            &mut report,
            &mut first_entry,
        ).await;
    }

    report.push_str("\n]");

    let results_el = document
        .get_element_by_id("results")
        .expect("no #results element — see demo/list_bench.html");
    results_el.set_text_content(Some(&report));

    // 그룹 모드에서는 페이지가 42번 뜨므로 트랙 T를 매번 돌리면 같은 셀을
    // 42번 재는 셈이다(실측 확인: 16 → 672 레코드). 트랙 T는 자기 드라이버
    // (`bench/e2e/run-track-t-frequency.mjs`)가 §4.1 격자로 따로 수집하므로
    // 여기서는 그룹 모드가 아닐 때만 돌린다.
    if group.is_none() {
        run_track_t(&document, &container_node);
    } else if let Some(el) = document.get_element_by_id("results-cond") {
        // Playwright 경로의 완료 신호를 그룹 모드에서도 유지한다.
        el.set_text_content(Some("[]"));
    }

    // -----------------------------------------------------------------------
    // **stock 브라우저 수집 경로 (2026-08-03 신설)**
    //
    // 지금까지 결과 추출은 Playwright가 `#results`를 읽는 방식뿐이었고, 그래서
    // **Playwright가 번들한 브라우저 빌드**로만 측정할 수 있었다. 조사 결과
    // Playwright의 Firefox 빌드가 stock Firefox와 크게 다르다는 것이 확인됐다 —
    // 같은 wasm 바이너리로 stock Firefox 150.0.1은 Chromium과 동률(0.91×)인데
    // Playwright 빌드는 8.1~8.4배 느리다(그 바이너리를 Playwright 없이 직접
    // 띄워도 동일하므로 드라이버가 아니라 빌드의 성질). 즉 그동안의 Firefox
    // 교차 확인은 비대표 빌드에서 나온 것이었다.
    //
    // `?collect=<이름>` 쿼리가 있으면 결과를 서버로 POST한다. **모든 측정이
    // 끝난 뒤 한 번** 호출되므로 측정 구간에는 영향이 없다 — Playwright 경로의
    // 동작도 그대로다(`#results`는 여전히 채워진다).
    // -----------------------------------------------------------------------
    if let Some(name) = query_param("collect") {
        let cond = document
            .get_element_by_id("results-cond")
            .and_then(|e| e.text_content())
            .unwrap_or_default();
        match next_group {
            // 그룹 모드: 이 페이지 몫의 레코드만 조각으로 보내고 다음 그룹으로
            // **스스로 이동**한다. 페이지가 매번 새로 뜨므로 §8의 "셀마다
            // fresh page load"에 맞고 누적 열화도 끊긴다.
            Some(g) => {
                let payload =
                    format!("{{\"group\": {}, \"track_l\": {report}, \"track_t\": {cond}}}", g - 1);
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(&JsValue::from_str(&payload));
                let req = window.fetch_with_str_and_init(&format!("/__collect?name={name}.part"), &opts);
                // POST가 서버에 닿은 뒤에 이동해야 조각이 유실되지 않는다.
                let href = format!("?collect={name}&group={g}");
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = wasm_bindgen_futures::JsFuture::from(req).await;
                    if let Some(w) = web_sys::window() {
                        let _ = w.location().set_search(&href);
                    }
                });
            }
            None => {
                // 그룹 모드의 **마지막 페이지**(범위 밖 인덱스)는 아무 조합도
                // 돌지 않았으므로 빈 결과다 — 출력 파일을 덮어쓰지 않도록
                // 별도 이름으로 보낸다. 그룹 모드가 아닐 때(전 조합 1페이지)는
                // 종전대로 출력 파일에 보낸다.
                let suffix = if group.is_some() { ".done" } else { "" };
                let payload = format!("{{\"track_l\": {report}, \"track_t\": {cond}}}");
                let opts = web_sys::RequestInit::new();
                opts.set_method("POST");
                opts.set_body(&JsValue::from_str(&payload));
                let _ =
                    window.fetch_with_str_and_init(&format!("/__collect?name={name}{suffix}"), &opts);
            }
        }
    }

    Ok(())
}

/// 브라우저 빌드 식별자.
///
/// §11 개정 #4가 Firefox 열 전체를 무효화한 원인은 **Playwright가 번들한
/// 비-stable 빌드**였는데, 레코드에 빌드 식별자가 없어 데이터만으로는 그
/// 사실을 다시 판별할 수 없었다. 이제 매 레코드가 자기 출처를 들고 다닌다.
fn user_agent() -> String {
    web_sys::window()
        .and_then(|w| w.navigator().user_agent().ok())
        .unwrap_or_default()
        .replace('"', "'")
}

/// 진행 상황 한 줄을 서버로 보낸다(`?collect=<이름>`일 때만).
///
/// stock 브라우저 경로에는 드라이버가 없어 밖에서 `window`를 들여다볼 수
/// 없다 — 이게 없으면 수집이 느린 것인지 멈춘 것인지 구별이 안 된다.
/// 측정 구간 **밖**(셀/조합 경계)에서만 호출한다.
fn report_progress(line: &str) {
    let Some(name) = query_param("collect") else { return };
    let Some(win) = web_sys::window() else { return };
    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(line));
    let _ = win.fetch_with_str_and_init(&format!("/__collect?name={name}.progress"), &opts);
}

/// 현재 페이지 URL의 쿼리 파라미터 하나를 읽는다.
fn query_param(key: &str) -> Option<String> {
    let search = web_sys::window()?.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(key)
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
    dom: loom_runtime::CountingDom<BrowserDom>,
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
    /// §4.1(v1.1 — F-1) "**동결/은닉 중** 누적 CPU 시간은 6 arm 전부 보고".
    /// 상주 시간을 조건 상태별로 갈라야 이 조항이 성립한다 — 뭉쳐 놓으면
    /// "B=0(부재 자체가 데이터)"라는 기준선도, "F-ii·CSS=계속되는 패치"라는
    /// 표적도 수치로 확인할 수 없다(2026-08-02 발견: 그동안 뭉쳐 있었다).
    residency_hidden_ms: f64,
    residency_hidden_ticks: u32,
    residency_shown_ms: f64,
    residency_shown_ticks: u32,
    /// J-b 그 자체: 창 안의 모든 tick 시간 합(토글 + 상주).
    total_script_ms: f64,
    frames: u32,

    /// §4.1 "동결/attached 구간 상류 Signal 변경률" 축(0~100%).
    change_rate_pct: u32,
    /// §4.1 서브트리 구성 축: "flat" | "chain3".
    composition: String,

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

/// 트랙 T의 **arm 구성 단일 지점**. f-격자 셀(`TrackTCell`)과 catch-up 곡선
/// 셀(`CatchUpCell`)이 같은 것을 쓴다 — arm 명세가 축마다 갈리면 두 축의
/// 결과를 겹쳐 읽을 수 없기 때문이다(§2.4 "전 후보 동일 조건").
fn build_cell(
    arm: &str,
    size: usize,
    composition: &str,
) -> (LoomApp, loom_runtime::CountingDom<BrowserDom>, bevy_ecs::entity::Entity, web_sys::Element) {
        use loom_runtime::{
            bump_version_system, cond_reconcile_b_system, cond_reconcile_f_i_system,
            cond_reconcile_f_ii_system, cond_reconcile_f_iii_system, cond_reconcile_f_system,
            derive1_f_i_system, derive1_f_ii_system, derive1_f_iii_system, derive2_f_i_system,
            derive2_f_ii_system, derive2_f_iii_system, derive3_f_i_system, derive3_f_ii_system,
            derive3_f_iii_system, patch_chain_f_i_system, patch_chain_f_ii_system,
            patch_chain_f_iii_system, patch_chain_plain_system, patch_text_f_i_system,
            patch_text_f_ii_system, patch_text_f_iii_system, CondBlock,
        };
        use bevy_ecs::prelude::IntoScheduleConfigs;
        console_error_panic_hook::set_once();

        let document = web_sys::window().unwrap().document().unwrap();
        let body: web_sys::Node = document.body().unwrap().into();

        let container = document.create_element("div").unwrap();
        container.set_attribute("data-track-t-cell", arm).unwrap();
        body.append_child(&container).unwrap();
        let anchor = document.create_element("div").unwrap();
        container.append_child(&anchor).unwrap();
        let anchor_node: web_sys::Node = anchor.into();

        let (raw_dom, handle) = BrowserDom::new(document.clone(), anchor_node);
        // §4.1 "수집 명시(v1.1)": "트랙 T 전 셀에서 M1, **M2**, M4, M5, M6 수집·보고".
        // 2026-08-03까지 트랙 T는 `BrowserDom`을 날것으로 써서 M2가 전면 미계측이었다
        // (트랙 L은 이미 `CountingDom`을 쓰고 있었으므로 도구가 없어서가 아니다).
        let mut dom = loom_runtime::CountingDom::new(raw_dom);
        let chain3 = match composition {
            "flat" => false,
            "chain3" => true,
            other => panic!("TrackTCell: unknown composition {other:?} (expected flat | chain3)"),
        };
        let mut app = LoomApp::new();
        match arm {
            // B와 CSS 은닉(F)은 **게이팅 기계장치가 없다**. 그건 patch
            // 파이프라인이 없다는 뜻이 **아니다** — 2026-08-03까지 flat 구성에서
            // 두 arm에 patch System이 하나도 등록되지 않아, §4.1 상류 변경률
            // 축이 건드린 Signal을 아무도 소비하지 않았고 두 arm의 상주/은닉
            // CPU가 구조적으로 0이었다. §10의 CSS 은닉 행은 "은닉 중 패치
            // 파이프라인은 게이팅 없이 계속 동작 — catch-up 기계장치 없음이 이
            // arm의 정의적 특성(게이팅을 붙이면 허수아비화)"이라고 명시한다.
            // §4.1의 "B=0(부재 자체가 데이터)"는 *게이팅*이 0이라는 뜻이지
            // *패치*가 0이라는 뜻이 아니다(B는 숨을 때 entity 자체가 사라져서
            // 은닉 중 0이 되는 것이고, 그게 기준선의 의미다).
            "B" => {
                app.add_systems(cond_reconcile_b_system);
                if chain3 {
                    app.add_systems(
                        (
                            derive1_f_ii_system,
                            derive2_f_ii_system,
                            derive3_f_ii_system,
                            patch_chain_plain_system,
                        )
                            .chain(),
                    );
                } else {
                    app.add_systems(patch_text_f_ii_system);
                }
            }
            "F" => {
                app.add_systems(cond_reconcile_f_system);
                if chain3 {
                    app.add_systems(
                        (
                            derive1_f_ii_system,
                            derive2_f_ii_system,
                            derive3_f_ii_system,
                            patch_chain_plain_system,
                        )
                            .chain(),
                    );
                } else {
                    app.add_systems(patch_text_f_ii_system);
                }
            }
            "F-i" => {
                if chain3 {
                    app.add_systems(
                        (
                            cond_reconcile_f_i_system,
                            derive1_f_i_system,
                            derive2_f_i_system,
                            derive3_f_i_system,
                            patch_chain_f_i_system,
                        )
                            .chain(),
                    );
                } else {
                    app.add_systems((cond_reconcile_f_i_system, patch_text_f_i_system).chain());
                }
            }
            "F-ii" => {
                if chain3 {
                    app.add_systems(
                        (
                            cond_reconcile_f_ii_system,
                            derive1_f_ii_system,
                            derive2_f_ii_system,
                            derive3_f_ii_system,
                            patch_chain_f_ii_system,
                        )
                            .chain(),
                    );
                } else {
                    app.add_systems((cond_reconcile_f_ii_system, patch_text_f_ii_system).chain());
                }
            }
            "F-iii" => {
                if chain3 {
                    app.add_systems(
                        (
                            cond_reconcile_f_iii_system,
                            bump_version_system,
                            derive1_f_iii_system,
                            derive2_f_iii_system,
                            derive3_f_iii_system,
                            patch_chain_f_iii_system,
                        )
                            .chain(),
                    );
                } else {
                    app.add_systems(
                        (cond_reconcile_f_iii_system, bump_version_system, patch_text_f_iii_system)
                            .chain(),
                    );
                }
            }
            // §4 "구현 arm 6개"의 여섯 번째. §10 행: "SSR-false 시 entity
            // 미탄생(경계 마커만) → 첫 true에 SpawnTable spawn(B-보강 명세,
            // 스폰 번들 초기값 포함) → 이후 토글은 F-i 명세 **전액 상속**".
            // 그래서 시스템은 F-i와 같은 것을 쓰고, 갈리는 지점은 아래
            // `prematerialize`(= 창을 열기 전에 실체화하는가) 하나뿐이다.
            "lazy" => {
                // §10: "이후 토글은 F-i 명세 전액 상속" — 그래서 시스템 구성도
                // F-i와 같다.
                if chain3 {
                    app.add_systems(
                        (
                            cond_reconcile_f_i_system,
                            derive1_f_i_system,
                            derive2_f_i_system,
                            derive3_f_i_system,
                            patch_chain_f_i_system,
                        )
                            .chain(),
                    );
                } else {
                    app.add_systems((cond_reconcile_f_i_system, patch_text_f_i_system).chain());
                }
            }
            other => panic!(
                "TrackTCell: unknown arm {other:?} (expected B | F | F-i | F-ii | F-iii | lazy)"
            ),
        }
        let entity = app
            .world
            .spawn((Signal(false), CondBlock::with_size(size), DomTarget(handle)))
            .id();
        // 초기 tick은 창 밖 — 셀은 "숨겨진 상태"에서 출발한다.
        app.tick(&mut dom);

        // -------------------------------------------------------------------
        // **실체화 (2026-08-02 신설)** — §4의 6 arm을 실제로 구별시키는 지점.
        //
        // 그동안 F 계열 전부가 `f_common_toggle`의 "첫 true — lazy spawn"
        // 경로로 출발했다. 즉 F-i/F-ii/F-iii·CSS 은닉이 **전부 사실상
        // lazy-spawn-then-hide**였고, §10이 별도 arm으로 사전 선언한 그것은
        // 따로 존재하지 않았다(2026-08-02 발견).
        //
        // 자기 명세대로면 F 계열과 CSS 은닉은 서브트리가 **이미 실체화된**
        // 상태에서 출발하고(SSR이 그렸거나 즉시 실체화), 첫 토글은 spawn이
        // 아니라 detach다. 그래서 창을 열기 전에 true→false를 한 번 돌려
        // 실체화만 시켜 둔다. `lazy`와 `B`는 그러지 않는다 — 아무것도 없는
        // 상태에서 출발하는 것이 각자의 명세다.
        //
        // 정상 상태 수치에는 영향이 없다(첫 true 이후 lazy ≡ F-i, §10 문언).
        // 바뀌는 것은 **첫 토글**의 의미와 §6 S2(a) 페이로드 비교뿐이며,
        // 그 둘이 정확히 이 arm을 사전 선언한 이유다.
        // -------------------------------------------------------------------
        let prematerialize = matches!(arm, "F" | "F-i" | "F-ii" | "F-iii");
        if prematerialize {
            app.world.get_mut::<Signal<bool>>(entity).unwrap().0 = true;
            app.tick(&mut dom);
            app.world.get_mut::<Signal<bool>>(entity).unwrap().0 = false;
            app.tick(&mut dom);
        }
    (app, dom, entity, container)
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
    /// `arm`: "B" | "F"(CSS 은닉 겸용) | "F-i" | "F-ii" | "F-iii".
    /// `change_rate_pct`: §4.1의 "상류 Signal 변경률" 축 — 매 프레임 이 비율만큼의
    /// 자식 Signal을 건드린다. **이 축이 없으면 F 변형 3개의 J-b가 사실상
    /// 같아진다**(2026-07-28 f축 스모크런에서 실제로 그렇게 관측됨) — 게이팅
    /// 비용은 "게이팅할 일이 있을 때"만 드러나기 때문이다.
    #[wasm_bindgen(constructor)]
    /// `composition`: §4.1의 **서브트리 구성 축** — `"flat"`(binding 1단) 또는
    /// `"chain3"`(파생/계산 체인 깊이 3). 조항이 "s=100 행에서만 양쪽"으로
    /// 고정했다. 2026-08-02까지 하네스에는 flat 쪽만 있었다.
    pub fn new(
        arm: &str,
        f_millihz: u32,
        size: usize,
        window_ms: f64,
        change_rate_pct: u32,
        composition: &str,
    ) -> TrackTCell {
        let (app, dom, entity, container) = build_cell(arm, size, composition);


        // 창을 열기 *전에* 잰다 — 바쁜 대기가 창 안의 프레임 예산을 먹지
        // 않도록.
        let timer_resolution_ms = probe_timer_resolution_ms();

        let now = measure_now();
        let toggle_interval_ms = 1000.0 / (f_millihz as f64 / 1000.0);
        TrackTCell {
            change_rate_pct,
            composition: composition.to_string(),
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
            residency_hidden_ms: 0.0,
            residency_hidden_ticks: 0,
            residency_shown_ms: 0.0,
            residency_shown_ticks: 0,
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

        // §4.1 상류 변경률 축: 매 프레임 자식 Signal의 `change_rate_pct`%를
        // 건드린다. **이것이 F 변형들을 구별되게 만드는 유일한 장치다** —
        // 게이팅 비용은 게이팅할 일이 있어야 드러나기 때문(F-i는 필터로 막고,
        // F-ii는 detached DOM에 계속 쓰고, F-iii는 버전만 올린다).
        if self.change_rate_pct > 0 {
            let children: Vec<bevy_ecs::entity::Entity> = self
                .app
                .world
                .get::<loom_runtime::CondBlock>(self.entity)
                .map(|b| b.children.clone())
                .unwrap_or_default();
            let touch = (children.len() * self.change_rate_pct as usize).div_ceil(100);
            for &child in children.iter().take(touch) {
                if let Some(mut sig) = self.app.world.get_mut::<Signal<i32>>(child) {
                    sig.0 = sig.0.wrapping_add(1);
                }
            }
        }

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
            // §4.1 F-1: 은닉/동결 구간과 표시 구간을 갈라 센다.
            if self.cond_state {
                self.residency_shown_ms += elapsed;
                self.residency_shown_ticks += 1;
            } else {
                self.residency_hidden_ms += elapsed;
                self.residency_hidden_ticks += 1;
            }
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
             \"residency_hidden_ms\": {:.4}, \"residency_hidden_ticks\": {}, \
             \"residency_shown_ms\": {:.4}, \"residency_shown_ticks\": {}, \
             \"j_b_total_script_ms\": {:.4}, \
             \"timer_resolution_ms\": {:.6}, \"cross_origin_isolated\": {}, \
             \"change_rate_pct\": {}, \"composition\": \"{}\", \
             \"m2_dom_ops\": {}, \"live_node_slots\": {}}}",
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
            self.residency_hidden_ms,
            self.residency_hidden_ticks,
            self.residency_shown_ms,
            self.residency_shown_ticks,
            self.total_script_ms,
            self.timer_resolution_ms,
            cross_origin_isolated(),
            self.change_rate_pct,
            self.composition,
            self.dom.counts().to_json(),
            loom_runtime::DomOps::live_node_count(&self.dom).unwrap_or(0),
        )
    }
}

// ============================================================================
// §4.1 catch-up 곡선 축 (M6)
// ============================================================================
//
// §4.1: "**catch-up 곡선 축**(M6): 동결 기간 {1s, 10s} × 동결 중 상류 Signal
// 변경률 {0, 10, 100%} → **재부착 비용 곡선과 B 재스폰 비용의 교차점**."
//
// 이건 f-격자 셀과 **측정 모양이 다르다**. f-격자는 "60초 창에서 주기적으로
// 토글하며 J-a/J-b를 모은다"이고, 여기는 "한 번 동결 → 정해진 기간 동안
// 정해진 비율로 상류를 흔든다 → 재부착 1회의 비용을 잰다"이다. 재부착 비용은
// **동결 기간 × 변경률**의 함수이고, 그 곡선이 B의 재스폰 비용선과 어디서
// 만나는지가 이 축의 산출물이다.
//
// 그래서 `TrackTCell`을 재사용하지 않고 별도 타입으로 둔다 — f-격자에 억지로
// 끼워 넣으면 "토글 간 상주"와 "동결 중 변경"이 섞여 곡선이 나오지 않는다.
#[wasm_bindgen]
pub struct CatchUpCell {
    app: LoomApp,
    dom: loom_runtime::CountingDom<BrowserDom>,
    entity: bevy_ecs::entity::Entity,
    container: web_sys::Element,
    arm: String,
    size: usize,
    /// 동결 기간(ms) — 사전 등록값 {1000, 10000}.
    freeze_ms: f64,
    /// 동결 **중** 상류 Signal 변경률(%) — 사전 등록값 {0, 10, 100}.
    change_rate_pct: u32,
    composition: String,

    started_at_ms: f64,
    /// 동결 진입 tick 비용(detach 쪽). 재부착과 비대칭이라 따로 보고한다
    /// (§4.1 "detach/attach 비대칭 분리").
    detach_ms: f64,
    /// 동결 중 누적 CPU — §4.1 F-1이 6 arm 전부 요구하는 값. F-ii·CSS는
    /// 여기가 크고(계속 패치), B는 0에 가깝다(entity 자체가 없음).
    frozen_cpu_ms: f64,
    frozen_ticks: u32,
    /// **이 축의 산출물**: 재부착 1회의 tick 비용.
    reattach_ms: f64,
    /// 재부착 직후 catch-up이 실제로 끝날 때까지 든 추가 tick들. F-i는
    /// 전파 System 1회, F-iii는 버전 불일치분만 — 여기가 갈린다.
    catchup_ms: f64,
    catchup_ticks: u32,
    timer_resolution_ms: f64,
    done: bool,
}

#[wasm_bindgen]
impl CatchUpCell {
    /// `arm`: B | F | F-i | F-ii | F-iii | lazy.
    #[wasm_bindgen(constructor)]
    pub fn new(
        arm: &str,
        size: usize,
        freeze_ms: f64,
        change_rate_pct: u32,
        composition: &str,
    ) -> CatchUpCell {
        // 셀 구성은 f-격자 셀과 **완전히 동일한 경로**(`build_cell`)를 쓴다 —
        // arm 명세가 두 축에서 갈리면 곡선과 격자를 겹쳐 읽을 수 없다.
        let (app, dom, entity, container) = build_cell(arm, size, composition);
        let timer_resolution_ms = probe_timer_resolution_ms();

        CatchUpCell {
            app,
            dom,
            entity,
            container,
            arm: arm.to_string(),
            size,
            freeze_ms,
            change_rate_pct,
            composition: composition.to_string(),
            started_at_ms: measure_now(),
            detach_ms: f64::NAN,
            frozen_cpu_ms: 0.0,
            frozen_ticks: 0,
            reattach_ms: f64::NAN,
            catchup_ms: 0.0,
            catchup_ticks: 0,
            timer_resolution_ms,
            done: false,
        }
    }

    /// 창을 연다: 표시 → (측정) 동결 → 동결 기간 동안 상류 흔들기 → (측정)
    /// 재부착 → catch-up이 잦아들 때까지 몇 tick 더.
    ///
    /// rAF 콜백마다 1회 호출. `false`를 반환하면 끝.
    pub fn step(&mut self, now_ms: f64) -> bool {
        if self.done {
            return false;
        }
        let t = now_ms - self.started_at_ms;

        // 0단계: 표시 상태로 올린다(측정 전 준비 — 창 밖으로 친다).
        if self.detach_ms.is_nan() && t < 0.0 {
            return true;
        }
        if self.detach_ms.is_nan() {
            self.app.world.get_mut::<Signal<bool>>(self.entity).unwrap().0 = true;
            self.app.tick(&mut self.dom);
            // 이제 동결로 들어가며 detach 비용을 잰다.
            self.app.world.get_mut::<Signal<bool>>(self.entity).unwrap().0 = false;
            let s = measure_now();
            self.app.tick(&mut self.dom);
            self.detach_ms = measure_now() - s;
            self.started_at_ms = measure_now();
            return true;
        }

        // 1단계: 동결 기간 — 매 프레임 상류를 change_rate_pct%만큼 흔든다.
        if t < self.freeze_ms {
            self.touch_upstream();
            let s = measure_now();
            self.app.tick(&mut self.dom);
            self.frozen_cpu_ms += measure_now() - s;
            self.frozen_ticks += 1;
            return true;
        }

        // 2단계: 재부착 1회 — 이 축의 산출물.
        if self.reattach_ms.is_nan() {
            self.app.world.get_mut::<Signal<bool>>(self.entity).unwrap().0 = true;
            let s = measure_now();
            self.app.tick(&mut self.dom);
            self.reattach_ms = measure_now() - s;
            return true;
        }

        // 3단계: 재부착 **이후** 잔여.
        //
        // **주의(2026-08-03 실측 확인)**: catch-up은 재부착과 **같은 tick 안에서
        // 끝난다** — `cond_reconcile_f_*_system`의 `set_changed()`와
        // `patch_text_f_*_system`이 같은 Schedule에 `.chain()`으로 이어져 있고
        // 뒤 System의 `last_run`이 직전 tick이라 같은 run에서 `Changed`가 보인다.
        // 프로브로 확인: F-i는 재부착 tick에 set_text 2→4(전량 재패치), F-iii는
        // 2→3(불일치분 1개만), 이후 tick은 둘 다 변화 없음.
        //
        // 따라서 §4.1 M6의 "재부착 완료 시간"은 `reattach_ms`가 맞고, 아래
        // `catchup_ms`는 **구조적으로 0에 가깝다**. 분석 쪽이 "catch-up 비용 =
        // catchup_ms"로 읽으면 곡선이 평평한 직선으로 결론나므로, 필드 이름이
        // 아니라 이 사실을 레코드에 명시한다(`catchup_is_intra_tick`).
        // tick 수는 잔여가 정말 없는지 확인할 만큼만(2회) 둔다.
        if self.catchup_ticks < 2 {
            let s = measure_now();
            self.app.tick(&mut self.dom);
            self.catchup_ms += measure_now() - s;
            self.catchup_ticks += 1;
            return true;
        }

        self.done = true;
        false
    }

    fn touch_upstream(&mut self) {
        if self.change_rate_pct == 0 {
            return;
        }
        let children: Vec<bevy_ecs::entity::Entity> = self
            .app
            .world
            .get::<loom_runtime::CondBlock>(self.entity)
            .map(|b| b.children.clone())
            .unwrap_or_default();
        let touch = (children.len() * self.change_rate_pct as usize).div_ceil(100);
        for &child in children.iter().take(touch) {
            if let Some(mut sig) = self.app.world.get_mut::<Signal<i32>>(child) {
                sig.0 = sig.0.wrapping_add(1);
            }
        }
    }

    pub fn finish(&mut self) -> String {
        self.container.remove();
        format!(
            "{{\"track\": \"T\", \"axis\": \"catchup\", \"arm\": \"{}\", \"size\": {}, \
             \"freeze_ms\": {:.1}, \"change_rate_pct\": {}, \"composition\": \"{}\", \
             \"detach_ms\": {:.4}, \"frozen_cpu_ms\": {:.4}, \"frozen_ticks\": {}, \
             \"reattach_ms\": {:.4}, \"catchup_ms\": {:.4}, \"catchup_ticks\": {}, \
             \"catchup_is_intra_tick\": true, \
             \"timer_resolution_ms\": {:.6}, \"cross_origin_isolated\": {}, \
             \"m2_dom_ops\": {}, \"live_node_slots\": {}}}",
            self.arm,
            self.size,
            self.freeze_ms,
            self.change_rate_pct,
            self.composition,
            if self.detach_ms.is_nan() { 0.0 } else { self.detach_ms },
            self.frozen_cpu_ms,
            self.frozen_ticks,
            if self.reattach_ms.is_nan() { 0.0 } else { self.reattach_ms },
            self.catchup_ms,
            self.catchup_ticks,
            self.timer_resolution_ms,
            cross_origin_isolated(),
            self.dom.counts().to_json(),
            loom_runtime::DomOps::live_node_count(&self.dom).unwrap_or(0),
        )
    }
}
