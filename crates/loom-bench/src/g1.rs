//! **G1 — 트랙별 정합성 게이트** (RQ1 프로토콜 §3.5·§4.3, 개정 #12 후속 8; 설계 `roadmap/rq1-g1-gate-design.md`).
//!
//! `--features g1` 빌드에만 들어간다. 측정 빌드(기능 끔)는 동결 산출물과 바이트가 같아야 하므로(후속 8 결정 2)
//! 이 모듈은 lib.rs **맨 끝**에서만 선언된다 — 앞쪽 줄 번호(= panic 위치 문자열)를 밀지 않는다.
//!
//! 하위 모듈이라 측정과 **같은** `make_*`·`RowShape`·`Workload::apply`·`derive_seed`·`cell_id`·`build_cell`을 그대로 쓴다.
//! 시간은 재지 않는다(측정 데이터가 아니다 — 후속 8 결정 1).
//!
//! 검사(조작마다):
//! 1. 내용: 조작 뒤 `<ul>`의 `outerHTML` = 같은 후보(같은 maker·같은 `RowShape`)를 목표 리스트로 **새로 마운트**한 `<ul>`의 `outerHTML`.
//!    독립 닻으로 각 행의 텍스트가 `item.to_string()`을 순서대로 포함하는지도 본다.
//! 2. 정체성: 조작 전에 행 엘리먼트마다 JS 표식(`__g1`)을 붙이고 조작 뒤 대조한다 — 후보 **자신의** 의미론으로.
//!    keyed(D·D-LIS·E·E-LIS) = 새 슬롯마다 아직 안 쓴 옛 행 중 같은 아이템의 **첫** 것(first-match-wins + 소비, list.rs 중복 키 주석)과
//!    같은 노드, 없으면 새 노드 / C = 인덱스 `< min(old,new)`는 같은 노드 / K = `k_dispatch(old,new)` 분기대로.
//! 3. 행 수 = 리스트 길이(고아 없음).
//! 4. E·E-LIS: `OrderKeyMembers`를 `OrderKey`로 정렬한 엔티티의 DOM 노드 열 = `<ul>`의 자식 열.
//! 5. K: 분기 발동 집계(NoOp·Append·Truncate·Clear·Full).

use super::*;
use js_sys::Reflect;
use std::collections::{HashMap, VecDeque};
use std::fmt::Write as _;

// ---------------------------------------------------------------------------
// 셀 목록
// ---------------------------------------------------------------------------

/// 행 모양 축. 측정 셀 목록의 R1/R2/R3/W8에 대응한다.
#[derive(Clone, Copy, PartialEq)]
enum RowAxis {
    /// R1, patch_window 끔(측정의 비이중 셀).
    R1,
    /// R1, patch_window 켬(측정의 이중 측정 셀 — 조건부 인스턴스도 창 표식을 단다).
    R1Pw,
    /// R2(bindings 8).
    R2,
    /// R3(이질 브랜치 row).
    R3,
    /// W8(외부 n × 내부 100 중첩).
    W8,
}

impl RowAxis {
    fn name(self) -> &'static str {
        match self {
            RowAxis::R1 => "R1",
            RowAxis::R1Pw => "R1+pw",
            RowAxis::R2 => "R2",
            RowAxis::R3 => "R3",
            RowAxis::W8 => "W8",
        }
    }
    fn bindings(self) -> usize {
        if self == RowAxis::R2 { 8 } else { 1 }
    }
    fn inner(self) -> usize {
        if self == RowAxis::W8 { 100 } else { 0 }
    }
    fn variants(self) -> bool {
        self == RowAxis::R3
    }
    fn shape(self, write_variant: loom_runtime::WriteVariant) -> RowShape {
        let unc = write_variant == loom_runtime::WriteVariant::Unconditional;
        RowShape {
            bindings: self.bindings(),
            inner: self.inner(),
            variants: self.variants(),
            write_variant,
            // 측정에서 무조건 인스턴스는 이중 측정 셀에만 있고 그 셀은 전 인스턴스가 창 표식을 단다.
            patch_window: unc || self == RowAxis::R1Pw,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    /// 측정과 같은 형태: rep마다 `initial`로 복원 → 워크로드 1회(W5는 100스텝). 복원도 전이로 검사한다.
    Op(Workload, &'static str),
    /// 복원 없이 이어 붙인 연쇄: 라운드마다 워크로드를 시드 순서로 적용한다.
    Chain(u32),
    /// K 분기 전용 시퀀스(같은 리스트 재대입, 꼬리 제거 등).
    KSeq,
    /// E 반례 `[A,B,C]→[B,C,A]` 등 작은 명시 케이스.
    Small,
    /// W6: 빈 리스트에서 head-insert 연속. 내용 신탁(새 마운트)은 `every` 스텝마다만.
    W6(u32),
    /// 음성 대조: 일부러 틀린 의미론으로 검사해 불일치가 실제로 잡히는지 본다.
    Neg,
}

#[derive(Clone, Copy)]
struct G1Cell {
    pod: bool,
    n: usize,
    row: RowAxis,
    kind: Kind,
}

const OP_REPS: u32 = 20;
/// 연속 시퀀스·연쇄에서 내용 신탁(새 마운트)을 거는 간격. 정체성·행 수·텍스트 닻·OrderKey 검사는 매 전이에서 한다.
/// 2026-11-01 실행 조정(개정 #12 후속 8 이행 기록): 신탁 비용 대부분이 전이마다 LoomApp을 새로 만드는 고정 비용이라
/// 매 전이 신탁이면 한 브라우저 release 실행이 8–10시간이었다. 구조가 깨진 행은 교체될 때까지 남으므로 표본 신탁도
/// 지속 결함은 잡는다 — 놓치는 것은 10전이 안에 저절로 고쳐지는 일시 결함뿐이다.
const SEQ_ORACLE_EVERY: u32 = 10;

fn g1_cell_list() -> Vec<G1Cell> {
    let mut v = Vec::new();
    let works = [
        Workload::Swap,
        Workload::Shuffle,
        Workload::Shuffle10,
        Workload::HeadInsert,
        Workload::Append,
        Workload::RemoveMiddle,
        Workload::ValueUpdate,
        Workload::Clear,
        Workload::Replace50,
        Workload::Replace100,
    ];
    for pod in [true, false] {
        for n in [100usize, 1000] {
            for row in [RowAxis::R1, RowAxis::R1Pw, RowAxis::R2, RowAxis::R3] {
                for w in works {
                    for band in ["measure", "g1"] {
                        v.push(G1Cell { pod, n, row, kind: Kind::Op(w, band) });
                    }
                }
                v.push(G1Cell { pod, n, row, kind: Kind::Chain(OP_REPS) });
            }
        }
        // W8: 측정 셀은 외부 100 × 내부 100 head-insert. 다른 워크로드도 같은 모양에서 검사한다.
        for w in works {
            for band in ["measure", "g1"] {
                v.push(G1Cell { pod, n: 100, row: RowAxis::W8, kind: Kind::Op(w, band) });
            }
        }
        // n = 10,000: 연쇄 1개(라운드 1).
        v.push(G1Cell { pod, n: 10_000, row: RowAxis::R1, kind: Kind::Chain(1) });
    }
    for row in [RowAxis::R1, RowAxis::R2, RowAxis::R3] {
        v.push(G1Cell { pod: true, n: 100, row, kind: Kind::KSeq });
    }
    v.push(G1Cell { pod: true, n: 3, row: RowAxis::R1, kind: Kind::Small });
    v.push(G1Cell { pod: true, n: 0, row: RowAxis::R1, kind: Kind::W6(100) });
    v.push(G1Cell { pod: true, n: 100, row: RowAxis::R1, kind: Kind::Neg });
    v
}

fn kind_name(k: Kind) -> String {
    match k {
        Kind::Op(w, band) => format!("op:{}:{}", w.name(), band),
        Kind::Chain(r) => format!("chain:{r}"),
        Kind::KSeq => "kseq".into(),
        Kind::Small => "small".into(),
        Kind::W6(e) => format!("w6:every{e}"),
        Kind::Neg => "neg".into(),
    }
}

fn cell_label(c: &G1Cell) -> String {
    format!("{}|n={}|{}|{}", if c.pod { "pod" } else { "heap" }, c.n, c.row.name(), kind_name(c.kind))
}

/// 트랙 L 셀 목록(JSON 문자열 배열)과 트랙 T 셀 목록.
#[wasm_bindgen]
pub fn g1_cells() -> String {
    let l: Vec<String> = g1_cell_list().iter().map(|c| format!("\"{}\"", cell_label(c))).collect();
    let t: Vec<String> = t_cell_list().iter().map(|(a, c, s)| format!("\"{a}|{c}|{s}\"")).collect();
    format!("{{\"track_l\": [{}], \"track_t\": [{}]}}", l.join(", "), t.join(", "))
}

/// 빌드 정보 — dev 빌드인지(`debug_assert`가 살아 있는지)와 키 충돌 계수기.
#[wasm_bindgen]
pub fn g1_meta() -> String {
    #[cfg(debug_assertions)]
    let (dbg, coll) = (true, loom_runtime::list_key::KEY_COLLISIONS.load(std::sync::atomic::Ordering::Relaxed).to_string());
    #[cfg(not(debug_assertions))]
    let (dbg, coll) = (false, String::from("null"));
    format!("{{\"debug_assertions\": {dbg}, \"key_collisions\": {coll}, \"protocol_tag\": \"{PROTOCOL_TAG}\"}}")
}

// ---------------------------------------------------------------------------
// 트랙 L 검사기
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Sem {
    Keyed,
    Slots,
    K,
}

fn sem_of(name: &str) -> Sem {
    match name {
        "K" => Sem::K,
        "C" => Sem::Slots,
        _ => Sem::Keyed,
    }
}

#[derive(Default)]
struct Stats {
    transitions: u64,
    content_checked: u64,
    content_cached: u64,
    content_mismatch: u64,
    anchor_mismatch: u64,
    count_mismatch: u64,
    ident_same: u64,
    ident_new: u64,
    ident_mismatch: u64,
    orderkey_checked: u64,
    orderkey_mismatch: u64,
    moved_ops: u64,
    k_noop: u64,
    k_append: u64,
    k_truncate: u64,
    k_clear: u64,
    k_full: u64,
    unchecked_steps: u64,
    failures: Vec<String>,
}

impl Stats {
    fn fail(&mut self, s: String) {
        if self.failures.len() < 3 {
            self.failures.push(s);
        }
    }
    fn json(&self) -> String {
        let f: Vec<String> = self.failures.iter().map(|s| format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))).collect();
        format!(
            "{{\"transitions\": {}, \"content_checked\": {}, \"content_cached\": {}, \"content_mismatch\": {}, \"anchor_mismatch\": {}, \"count_mismatch\": {}, \
             \"ident_same\": {}, \"ident_new\": {}, \"ident_mismatch\": {}, \"orderkey_checked\": {}, \"orderkey_mismatch\": {}, \"moved_ops\": {}, \
             \"k\": {{\"noop\": {}, \"append\": {}, \"truncate\": {}, \"clear\": {}, \"full\": {}}}, \"unchecked_steps\": {}, \"failures\": [{}]}}",
            self.transitions, self.content_checked, self.content_cached, self.content_mismatch, self.anchor_mismatch, self.count_mismatch,
            self.ident_same, self.ident_new, self.ident_mismatch, self.orderkey_checked, self.orderkey_mismatch, self.moved_ops,
            self.k_noop, self.k_append, self.k_truncate, self.k_clear, self.k_full, self.unchecked_steps, f.join(", ")
        )
    }
}

struct Inst<T: Send + Sync + Clone + ToString + 'static> {
    cand: Candidate<T>,
    maker: Maker<T>,
    shape: RowShape,
    holder: web_sys::Element,
    ul: web_sys::Element,
    /// 검사에 쓰는 의미론. 음성 대조에서만 후보의 실제 의미론과 다르다.
    sem: Sem,
    label: String,
    stats: Stats,
    /// 초기 리스트와 그 마운트 직후의 `outerHTML`. 마운트는 결정적이므로 목표가 초기 리스트인 전이(측정의 "복원")는
    /// 새 마운트 대신 이것과 비교한다(2026-11-01 실행 조정 — 신탁 비용의 대부분이 LoomApp 생성 고정 비용이다).
    initial: Vec<T>,
    initial_html: String,
}

fn mount<T>(document: &web_sys::Document, parent: &web_sys::Node, maker: Maker<T>, initial: Vec<T>, shape: RowShape) -> (Candidate<T>, web_sys::Element, web_sys::Element)
where
    T: Send + Sync + Clone + ToString + 'static,
{
    let holder = document.create_element("div").unwrap();
    parent.append_child(&holder).unwrap();
    let holder_node: web_sys::Node = holder.clone().into();
    let cand = maker(document, &holder_node, initial, shape);
    let ul: web_sys::Element = holder.last_element_child().expect("maker가 <ul>을 만들지 않았다");
    (cand, holder, ul)
}

fn rows_of(ul: &web_sys::Element) -> Vec<web_sys::Element> {
    let c = ul.children();
    (0..c.length()).map(|i| c.item(i).unwrap()).collect()
}

fn tag_of(el: &web_sys::Element) -> Option<u32> {
    Reflect::get(el, &JsValue::from_str("__g1")).ok().and_then(|v| v.as_f64()).map(|f| f as u32)
}

fn set_tag(el: &web_sys::Element, id: u32) {
    Reflect::set(el, &JsValue::from_str("__g1"), &JsValue::from(id)).unwrap();
}

/// 새 슬롯 `i`가 넘겨받아야 할 옛 인덱스(`None` = 새 노드).
fn expected_identity<T: PartialEq + Eq + std::hash::Hash>(sem: Sem, old: &[T], new: &[T], st: &mut Stats) -> Vec<Option<usize>> {
    match sem {
        Sem::Keyed => {
            let mut pool: HashMap<&T, VecDeque<usize>> = HashMap::new();
            for (j, x) in old.iter().enumerate() {
                pool.entry(x).or_default().push_back(j);
            }
            new.iter().map(|x| pool.get_mut(x).and_then(|q| q.pop_front())).collect()
        }
        Sem::Slots => (0..new.len()).map(|i| if i < old.len() { Some(i) } else { None }).collect(),
        Sem::K => match loom_runtime::k_dispatch(old, new) {
            loom_runtime::KDispatch::NoOp => {
                st.k_noop += 1;
                (0..new.len()).map(Some).collect()
            }
            loom_runtime::KDispatch::Append(from) => {
                st.k_append += 1;
                (0..new.len()).map(|i| if i < from { Some(i) } else { None }).collect()
            }
            loom_runtime::KDispatch::Truncate(_) => {
                st.k_truncate += 1;
                (0..new.len()).map(Some).collect()
            }
            loom_runtime::KDispatch::Clear => {
                st.k_clear += 1;
                Vec::new()
            }
            loom_runtime::KDispatch::Full => {
                st.k_full += 1;
                vec![None; new.len()]
            }
        },
    }
}

struct Ctx {
    document: web_sys::Document,
    scratch: web_sys::Node,
    next_tag: u32,
}

impl<T> Inst<T>
where
    T: Send + Sync + Clone + ToString + PartialEq + Eq + std::hash::Hash + 'static,
{
    /// `old`(지금 DOM에 있어야 하는 리스트) → `new` 전이 하나를 실행하고 검사한다.
    fn step(&mut self, cx: &mut Ctx, old: &[T], new: &[T], oracle: bool) {
        let st = &mut self.stats;
        st.transitions += 1;
        let before = rows_of(&self.ul);
        if before.len() != old.len() {
            st.count_mismatch += 1;
            st.fail(format!("{}: 전이 전 행 수 {} ≠ 리스트 길이 {}", self.label, before.len(), old.len()));
        }
        let mut old_tags = Vec::with_capacity(before.len());
        for el in &before {
            cx.next_tag += 1;
            set_tag(el, cx.next_tag);
            old_tags.push(cx.next_tag);
        }
        let first_old_tag = old_tags.first().copied().unwrap_or(u32::MAX);
        let expect = expected_identity(self.sem, old, new, st);

        self.cand.set_and_tick(new.to_vec());

        let after = rows_of(&self.ul);
        if after.len() != new.len() {
            st.count_mismatch += 1;
            st.fail(format!("{}: 행 수 {} ≠ 리스트 길이 {}", self.label, after.len(), new.len()));
        }
        // 2. 정체성
        let mut moved = false;
        for (i, el) in after.iter().enumerate().take(new.len()) {
            let got = tag_of(el).filter(|t| *t >= first_old_tag);
            match expect.get(i).copied().flatten() {
                Some(j) => {
                    if got == old_tags.get(j).copied() {
                        st.ident_same += 1;
                        if j != i {
                            moved = true;
                        }
                    } else {
                        st.ident_mismatch += 1;
                        st.fail(format!("{}: 슬롯 {i}는 옛 행 {j}의 노드여야 하는데 {:?}", self.label, got.map(|g| g.wrapping_sub(first_old_tag))));
                    }
                }
                None => {
                    if got.is_none() {
                        st.ident_new += 1;
                    } else {
                        st.ident_mismatch += 1;
                        st.fail(format!("{}: 슬롯 {i}는 새 노드여야 하는데 옛 행 {:?}의 노드", self.label, got.map(|g| g - first_old_tag)));
                    }
                }
            }
        }
        if moved {
            st.moved_ops += 1;
        }
        // 1-닻. 행 텍스트가 아이템 표시 문자열을 포함한다.
        for (i, el) in after.iter().enumerate().take(new.len()) {
            let text = el.text_content().unwrap_or_default();
            if !text.contains(&new[i].to_string()) {
                st.anchor_mismatch += 1;
                st.fail(format!("{}: 슬롯 {i} 텍스트에 {:?}가 없다", self.label, new[i].to_string()));
                break;
            }
        }
        // 4. E: OrderKey 순서 = DOM 순서
        if self.cand.name.starts_with('E') {
            st.orderkey_checked += 1;
            let members = self.cand.app.world.get::<loom_runtime::list::OrderKeyMembers>(self.cand.entity).map(|m| m.0.clone()).unwrap_or_default();
            let mut keyed: Vec<(u64, bevy_ecs::entity::Entity)> = members
                .iter()
                .map(|&e| (self.cand.app.world.get::<loom_runtime::list::OrderKey>(e).map(|k| k.0).unwrap_or(u64::MAX), e))
                .collect();
            keyed.sort_by_key(|x| x.0);
            let ok = keyed.len() == after.len()
                && keyed.iter().zip(&after).all(|((_, e), el)| {
                    self.cand
                        .app
                        .world
                        .get::<loom_runtime::DomTarget>(*e)
                        .map(|t| self.cand.dom.inner_mut().element(t.0).is_same_node(Some(el)))
                        .unwrap_or(false)
                });
            if !ok {
                st.orderkey_mismatch += 1;
                st.fail(format!("{}: OrderKey 순서 ≠ DOM 순서 (members {}, rows {})", self.label, keyed.len(), after.len()));
            }
        }
        // 1. 내용 신탁: 같은 후보를 목표 리스트로 새로 마운트
        if oracle {
            st.content_checked += 1;
            let want = if new == self.initial.as_slice() {
                st.content_cached += 1;
                self.initial_html.clone()
            } else {
                let (fresh, holder, ul) = mount(&cx.document, &cx.scratch, self.maker, new.to_vec(), self.shape);
                let w = ul.outer_html();
                drop(fresh);
                holder.remove();
                w
            };
            let got = self.ul.outer_html();
            if want != got {
                st.content_mismatch += 1;
                let at = want.bytes().zip(got.bytes()).position(|(a, b)| a != b).unwrap_or(want.len().min(got.len()));
                let lo = at.saturating_sub(40);
                st.fail(format!(
                    "{}: 내용 ≠ 새 마운트 (첫 차이 {at}바이트, 기대 …{}… / 실제 …{}…)",
                    self.label,
                    want.get(lo..(at + 60).min(want.len())).unwrap_or(""),
                    got.get(lo..(at + 60).min(got.len())).unwrap_or("")
                ));
            }
        }
    }
}

fn instances<T>(cx: &Ctx, row: RowAxis, initial: &[T], neg: bool) -> Vec<Inst<T>>
where
    T: Send + Sync + Clone + ToString + PartialEq + Ord + Eq + std::hash::Hash + 'static,
{
    let mut out = Vec::new();
    let cond = row.shape(loom_runtime::WriteVariant::Conditional);
    let unc = row.shape(loom_runtime::WriteVariant::Unconditional);
    for (maker, write_axis) in all_makers::<T>() {
        let mut shapes = vec![(cond, "cond")];
        if write_axis {
            shapes.push((unc, "uncond"));
        }
        for (shape, tag) in shapes {
            let (cand, holder, ul) = mount(&cx.document, &cx.scratch, maker, initial.to_vec(), shape);
            let real = sem_of(cand.name);
            let sem = if neg {
                // 음성 대조: C를 keyed로, K를 "전부 retain"(=위치 귀속)으로, keyed를 위치 귀속으로 검사한다.
                match real {
                    Sem::Slots => Sem::Keyed,
                    Sem::K => Sem::Slots,
                    Sem::Keyed => Sem::Slots,
                }
            } else {
                real
            };
            let label = format!("{}[{tag}]", cand.name);
            let initial_html = ul.outer_html();
            out.push(Inst { cand, maker, shape, holder, ul, sem, label, stats: Stats::default(), initial: initial.to_vec(), initial_html });
        }
    }
    out
}

fn run_list_cell<T, F>(cx: &mut Ctx, c: &G1Cell, type_name: &str, initial: Vec<T>, fresh: F) -> String
where
    T: Send + Sync + Clone + ToString + PartialEq + Ord + Eq + std::hash::Hash + 'static,
    F: Fn(u64) -> T,
{
    let mut insts = instances::<T>(cx, c.row, &initial, c.kind == Kind::Neg);
    // 초기 마운트 자체도 신탁과 대조한다(전이 0개로 세지 않는다).
    let drive = |insts: &mut Vec<Inst<T>>, cx: &mut Ctx, old: &[T], new: &[T], oracle: bool| {
        for inst in insts.iter_mut() {
            inst.step(cx, old, new, oracle);
        }
    };
    let cid_of = |w: &Workload| cell_id(type_name, c.n, c.row.bindings(), c.row.inner(), c.row.variants(), w.name());
    match c.kind {
        Kind::Op(w, band) => {
            let cid = cid_of(&w);
            let mut cur = initial.clone();
            for rep in 0..OP_REPS {
                // 측정의 "각 반복 전 초기 상태 복원"
                drive(&mut insts, cx, &cur, &initial, true);
                cur = initial.clone();
                let mut rng = Pcg64::from_seed(derive_seed(band, &cid, rep));
                match w.shape() {
                    Shape::Single => {
                        let t = w.apply(&cur, &mut rng, &fresh);
                        drive(&mut insts, cx, &cur, &t, true);
                        cur = t;
                    }
                    Shape::Sequence(k) => {
                        // 연속 시퀀스: 내용 신탁은 10번째 스텝마다 + 마지막 스텝(SEQ_ORACLE_EVERY). 나머지 검사는 매 스텝.
                        for i in 0..k {
                            let t = w.apply(&cur, &mut rng, &fresh);
                            drive(&mut insts, cx, &cur, &t, i % SEQ_ORACLE_EVERY == SEQ_ORACLE_EVERY - 1 || i == k - 1);
                            cur = t;
                        }
                    }
                }
            }
        }
        Kind::Chain(rounds) => {
            let works = [
                Workload::Swap,
                Workload::Shuffle,
                Workload::Shuffle10,
                Workload::HeadInsert,
                Workload::Append,
                Workload::RemoveMiddle,
                Workload::ValueUpdate,
                Workload::Clear,
                Workload::Replace50,
                Workload::Replace100,
            ];
            let mut cur = initial.clone();
            let mut tick_no = 0u32;
            let total_rounds = rounds;
            for r in 0..rounds {
                let order = shuffled_indices(works.len(), &format!("{}|chain|{r}", type_name), "g1");
                let mut rng = Pcg64::from_seed(derive_seed("g1", &format!("{}|chain", cell_label(c)), r));
                for &wi in &order {
                    let w = works[wi];
                    if cur.is_empty() {
                        drive(&mut insts, cx, &cur, &initial, true);
                        cur = initial.clone();
                    }
                    let steps = match w.shape() {
                        Shape::Single => 1,
                        Shape::Sequence(k) => k,
                    };
                    for _ in 0..steps {
                        let t = w.apply(&cur, &mut rng, &fresh);
                        tick_no += 1;
                        // 연쇄의 내용 신탁도 10번째 전이마다(연쇄는 하나의 긴 시퀀스다). 마지막 라운드의 마지막 워크로드 끝은 아래에서 한 번 더.
                        drive(&mut insts, cx, &cur, &t, tick_no % SEQ_ORACLE_EVERY == 0);
                        cur = t;
                    }
                }
                let _ = total_rounds;
            }
            // 연쇄 끝: 같은 리스트 재대입(전 후보에서 write 0 경로)으로 마지막 상태를 신탁과 한 번 더 대조한다.
            let last = cur.clone();
            drive(&mut insts, cx, &cur, &last, true);
        }
        Kind::KSeq => {
            // K의 다섯 분기를 전부 밟는 시퀀스(측정 워크로드만으로는 NoOp·Truncate가 안 나온다).
            let mut cur = initial.clone();
            let mut rng = Pcg64::from_seed(derive_seed("g1", &cell_label(c), 0));
            let mut seq: Vec<Vec<T>> = Vec::new();
            seq.push(cur.clone()); // NoOp
            seq.push(cur[..cur.len() - 10].to_vec()); // Truncate
            seq.push(seq[1].clone()); // NoOp
            let mut ap = seq[2].clone();
            for _ in 0..5 {
                ap.push(fresh(rng.next()));
            }
            seq.push(ap.clone()); // Append
            seq.push(ap[..1].to_vec()); // Truncate → 1
            seq.push(Vec::new()); // Clear
            seq.push(Vec::new()); // NoOp(빈 → 빈)
            seq.push(initial.clone()); // Append(0)
            let mut sh = initial.clone();
            Workload::fisher_yates(&mut sh, &mut rng);
            seq.push(sh); // Full
            seq.push(initial.clone()); // Full
            for t in seq {
                drive(&mut insts, cx, &cur, &t, true);
                cur = t;
            }
        }
        Kind::Small => {
            // n = 3: [A,B,C] → [B,C,A](E 반례) → [C,A,B] → [C,B,A] → [A,B,C] → [A,C] → [C,A,B]
            let a = initial.clone();
            let p = |ix: &[usize]| ix.iter().map(|&i| a[i].clone()).collect::<Vec<T>>();
            let seq = [p(&[1, 2, 0]), p(&[2, 0, 1]), p(&[2, 1, 0]), p(&[0, 1, 2]), p(&[0, 2]), p(&[2, 0, 1])];
            let mut cur = initial.clone();
            for t in seq {
                drive(&mut insts, cx, &cur, &t, true);
                cur = t;
            }
        }
        Kind::W6(every) => {
            // 측정 W6과 같은 민팅: 빈 리스트에서 head-insert 10^4회.
            let mut rng = Pcg64::from_seed(derive_seed("g1", "w6", 0));
            let mut cur: Vec<T> = initial.clone();
            for i in 0..10_000u32 {
                let mut t = cur.clone();
                t.insert(0, fresh(rng.next()));
                // 매 스텝 전수 검사(표식 O(n))는 10^4 스텝에서 O(n²)라 표본 스텝만 검사한다 — 나머지 스텝은
                // 측정과 똑같이 대입·tick만 하고 `unchecked_steps`로 센다(원장에 그대로 실린다).
                if i % every == every - 1 {
                    drive(&mut insts, cx, &cur, &t, true);
                } else {
                    for inst in insts.iter_mut() {
                        inst.cand.set_and_tick(t.clone());
                        inst.stats.unchecked_steps += 1;
                    }
                }
                cur = t;
            }
        }
        Kind::Neg => {
            let w = Workload::Shuffle;
            let cid = cid_of(&w);
            let mut cur = initial.clone();
            for rep in 0..3 {
                drive(&mut insts, cx, &cur, &initial, false);
                cur = initial.clone();
                let mut rng = Pcg64::from_seed(derive_seed("g1", &cid, rep));
                let t = w.apply(&cur, &mut rng, &fresh);
                drive(&mut insts, cx, &cur, &t, false);
                cur = t;
            }
        }
    }
    let mut out = format!("{{\"cell\": \"{}\", \"instances\": {{", cell_label(c));
    for (i, inst) in insts.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "\"{}\": {{\"sem\": \"{:?}\", \"stats\": {}}}", inst.label, inst.sem, inst.stats.json());
    }
    out.push_str("}}");
    for inst in insts {
        inst.holder.remove();
    }
    out
}

/// 트랙 L 셀 하나를 돌리고 결과 JSON을 낸다. 인덱스는 `g1_cells().track_l`의 순서.
#[wasm_bindgen]
pub fn g1_run_l(index: usize) -> String {
    console_error_panic_hook::set_once();
    let cells = g1_cell_list();
    let c = cells[index];
    let document = web_sys::window().unwrap().document().unwrap();
    let scratch = document.create_element("div").unwrap();
    document.body().unwrap().append_child(&scratch).unwrap();
    let mut cx = Ctx { document: document.clone(), scratch: scratch.clone().into(), next_tag: 0 };
    let out = if c.pod {
        let initial: Vec<ItemId> = (0..c.n as u32).map(ItemId).collect();
        let init = if matches!(c.kind, Kind::W6(_)) { Vec::new() } else { initial };
        run_list_cell(&mut cx, &c, "ItemId(POD u32)", init, |seed| ItemId(1_000_000 + (seed as u32 % 1_000_000)))
    } else {
        let initial: Vec<String> = (0..c.n).map(heap_item).collect();
        run_list_cell(&mut cx, &c, "String(heap 128B)", initial, |seed| heap_item(1_000_000 + (seed as usize % 1_000_000)))
    };
    scratch.remove();
    out
}

// ---------------------------------------------------------------------------
// 트랙 T
// ---------------------------------------------------------------------------

fn t_cell_list() -> Vec<(&'static str, &'static str, usize)> {
    let mut v = Vec::new();
    for arm in ["B", "F", "F-i", "F-ii", "F-iii", "lazy"] {
        for comp in ["flat", "chain3"] {
            for size in [1usize, 100, 1000] {
                v.push((arm, comp, size));
            }
        }
    }
    v
}

fn derive3(x: i32) -> i32 {
    let s = |x: i32| x.wrapping_mul(3).wrapping_add(1);
    s(s(s(x)))
}

#[derive(Default)]
struct TStats {
    steps: u64,
    on: u64,
    off: u64,
    true_true: u64,
    false_false: u64,
    change_while_off: u64,
    catchup_checked: u64,
    mismatch: u64,
    redundant_writes: u64,
    settle_ticks_with_writes: u64,
    settle_writes: u64,
    failures: Vec<String>,
}

/// 트랙 T 셀 하나. 시드 시퀀스로 토글·재대입·값 변경을 섞어 스텝마다 검사한다.
#[wasm_bindgen]
pub fn g1_run_t(index: usize) -> String {
    console_error_panic_hook::set_once();
    let (arm, comp, size) = t_cell_list()[index];
    let chain3 = comp == "chain3";
    let (mut app, mut dom, entity, container) = build_cell(arm, size, comp);
    let anchor: web_sys::Element = container.first_element_child().unwrap();
    let mut st = TStats::default();
    let fail = |st: &mut TStats, s: String| {
        st.mismatch += 1;
        if st.failures.len() < 3 {
            st.failures.push(s);
        }
    };
    let shown_text = |v: i32| if chain3 { derive3(v).to_string() } else { v.to_string() };
    let prematerialized = matches!(arm, "F" | "F-i" | "F-ii" | "F-iii");
    // 모형 상태
    let mut cond = false;
    let mut values: Vec<i32> = if prematerialized { vec![0; size] } else { Vec::new() };
    let mut spans: Vec<web_sys::Element> = Vec::new(); // 마지막으로 붙어 있을 때 본 자식 노드
    let mut changed_while_off = false;
    // stale LRef 재현(B만, 기록용)
    let mut stale_probe: Option<Vec<bevy_ecs::entity::Entity>> = None;
    let mut stale_resolved = 0u32;
    let mut stale_index_reused = 0u32;
    let mut stale_checked = 0u32;

    let mut rng = Pcg64::from_seed(derive_seed("g1", &format!("T|{arm}|{comp}|{size}"), 0));
    for step in 0..240u32 {
        st.steps += 1;
        // 앞 몇 스텝은 커버리지를 고정한다: 켬 → 값 변경 → 끔 → 값 변경 → 켬(catch-up) → 참→참 → 끔 → 거짓→거짓
        let action = match step {
            0 => 0,
            1 => 2,
            2 => 0,
            3 => 2,
            4 => 0,
            5 => 1,
            6 => 0,
            7 => 1,
            _ => [0, 1, 2, 2][rng.below(4)],
        };
        let mut value_changed = false;
        match action {
            0 => {
                cond = !cond;
                if cond {
                    st.on += 1;
                } else {
                    st.off += 1;
                }
                app.world.get_mut::<Signal<bool>>(entity).unwrap().0 = cond;
            }
            1 => {
                // 재대입 검사를 앞 전이의 지연 쓰기와 가르기 위해, 변화 없는 정착 tick을 먼저 돌린다.
                // 거기서 나온 쓰기는 정합성 불일치가 아니라 관찰(`settle_writes`)로 따로 센다 — 예: B·F는 patch System이
                // reconcile과 체인으로 묶여 있지 않아 스폰 다음 tick에 초기값 SetText가 한 번 더 나온다.
                dom.reset_counts();
                app.tick(&mut dom);
                let w = dom.counts().total_mutations();
                if w > 0 {
                    st.settle_ticks_with_writes += 1;
                    st.settle_writes += w as u64;
                }
                if cond {
                    st.true_true += 1;
                } else {
                    st.false_false += 1;
                }
                app.world.get_mut::<Signal<bool>>(entity).unwrap().0 = cond;
            }
            _ => {
                let children = app.world.get::<loom_runtime::CondBlock>(entity).map(|b| b.children.clone()).unwrap_or_default();
                if !children.is_empty() {
                    let k = 1 + rng.below(children.len().div_ceil(10));
                    for _ in 0..k {
                        let i = rng.below(children.len());
                        if let Some(mut s) = app.world.get_mut::<Signal<i32>>(children[i]) {
                            s.0 = s.0.wrapping_add(1);
                            values[i] = values[i].wrapping_add(1);
                            value_changed = true;
                        }
                    }
                    if value_changed && !cond {
                        st.change_while_off += 1;
                        changed_while_off = true;
                    }
                }
            }
        }
        // B: 거짓→참 전이에서 자식이 새로 만들어지므로 값이 0으로 돌아간다(B의 명세 — 상태 소실).
        let reattach = action == 0 && cond;
        if reattach && (arm == "B" || values.is_empty()) {
            values = vec![0; size];
        }
        if arm == "B" && action == 0 && !cond {
            stale_probe = app.world.get::<loom_runtime::CondBlock>(entity).map(|b| b.children.clone());
        }
        dom.reset_counts();
        app.tick(&mut dom);
        let writes = dom.counts().total_mutations();
        // 재대입(참→참·거짓→거짓)은 값 변경이 없으면 DOM write 0이어야 한다.
        if action == 1 && writes != 0 {
            st.redundant_writes += 1;
            fail(&mut st, format!("step {step}: {} 재대입에서 DOM write {writes}건", if cond { "참→참" } else { "거짓→거짓" }));
        }
        // 검사
        let kids = rows_of(&anchor);
        let display = anchor.unchecked_ref::<web_sys::HtmlElement>().style().get_property_value("display").unwrap_or_default();
        if cond {
            if kids.len() != size {
                fail(&mut st, format!("step {step}: 참인데 자식 {} ≠ {size}", kids.len()));
            }
            if display == "none" {
                fail(&mut st, format!("step {step}: 참인데 display:none"));
            }
            for (i, el) in kids.iter().enumerate().take(values.len()) {
                let t = el.text_content().unwrap_or_default();
                if t != shown_text(values[i]) {
                    fail(&mut st, format!("step {step}: 참, 자식 {i} 텍스트 {t:?} ≠ 기대 {:?}", shown_text(values[i])));
                    break;
                }
            }
            if reattach && changed_while_off {
                st.catchup_checked += 1;
            }
            if reattach {
                changed_while_off = false;
                // 정체성: B는 새 노드, 나머지는 이전 노드 유지
                if !spans.is_empty() && kids.len() == spans.len() {
                    let same = kids.iter().zip(&spans).all(|(a, b)| a.is_same_node(Some(b)));
                    if arm == "B" && same {
                        fail(&mut st, format!("step {step}: B 재표시인데 옛 노드가 남았다"));
                    }
                    if arm != "B" && !same {
                        fail(&mut st, format!("step {step}: {arm} 재부착인데 노드가 바뀌었다"));
                    }
                }
                if let Some(old) = stale_probe.take() {
                    let new = app.world.get::<loom_runtime::CondBlock>(entity).map(|b| b.children.clone()).unwrap_or_default();
                    for e in &old {
                        stale_checked += 1;
                        if app.world.get_entity(*e).is_ok() {
                            stale_resolved += 1;
                        }
                        if new.iter().any(|n| n.index() == e.index()) {
                            stale_index_reused += 1;
                        }
                    }
                }
            }
            spans = kids;
        } else {
            match arm {
                "B" => {
                    if !kids.is_empty() {
                        fail(&mut st, format!("step {step}: B 거짓인데 자식 {}", kids.len()));
                    }
                }
                "F" => {
                    if !values.is_empty() {
                        if display != "none" {
                            fail(&mut st, format!("step {step}: CSS 은닉 거짓인데 display={display:?}"));
                        }
                        if kids.len() != size {
                            fail(&mut st, format!("step {step}: CSS 은닉 거짓인데 자식 {} ≠ {size}", kids.len()));
                        }
                        // 자기 명세: 은닉 중에도 패치가 계속된다.
                        for (i, el) in kids.iter().enumerate().take(values.len()) {
                            let t = el.text_content().unwrap_or_default();
                            if t != shown_text(values[i]) {
                                fail(&mut st, format!("step {step}: CSS 은닉 중 자식 {i} {t:?} ≠ {:?}", shown_text(values[i])));
                                break;
                            }
                        }
                    }
                }
                _ => {
                    if !kids.is_empty() {
                        fail(&mut st, format!("step {step}: {arm} 거짓인데 앵커에 자식 {}(분리 안 됨)", kids.len()));
                    }
                    // F-ii: 분리된 노드에도 계속 패치된다(자기 명세).
                    if arm == "F-ii" {
                        for (i, el) in spans.iter().enumerate().take(values.len()) {
                            let t = el.text_content().unwrap_or_default();
                            if t != shown_text(values[i]) {
                                fail(&mut st, format!("step {step}: F-ii 분리 중 자식 {i} {t:?} ≠ {:?}", shown_text(values[i])));
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    container.remove();
    let f: Vec<String> = st.failures.iter().map(|s| format!("\"{}\"", s.replace('"', "\\\""))).collect();
    format!(
        "{{\"cell\": \"{arm}|{comp}|{size}\", \"steps\": {}, \"on\": {}, \"off\": {}, \"true_true\": {}, \"false_false\": {}, \"change_while_off\": {}, \
         \"catchup_checked\": {}, \"mismatch\": {}, \"redundant_writes\": {}, \"settle_ticks_with_writes\": {}, \"settle_writes\": {}, \"stale_lref\": {}, \"failures\": [{}]}}",
        st.steps, st.on, st.off, st.true_true, st.false_false, st.change_while_off, st.catchup_checked, st.mismatch, st.redundant_writes,
        st.settle_ticks_with_writes, st.settle_writes,
        if arm == "B" { format!("{{\"checked\": {stale_checked}, \"resolved\": {stale_resolved}, \"index_reused\": {stale_index_reused}}}") } else { "null".into() },
        f.join(", ")
    )
}
