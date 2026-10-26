//! Phase 7c(`DomWrite::Surface`) 테스트 — 설계 §4의 1~7번 + 크기·판별값·구조 단언·tripwire·위임.
//!
//! 이 파일의 `SurfDom`은 `tests/support::MockDom`을 확장하지 않고 **따로** 둔다 — 공유 목(Mock)은 수십 개 기존 테스트가
//! 쓰고 있어, 7c 전용 상태(네임스페이스·classList·프로퍼티·텍스트 노드·슬롯 재사용)를 넣으면 그 테스트들의 기준선이
//! 흔들린다. 슬롯 재사용은 `BrowserDom`의 free list(`begin_batch`에서 열림)를 같은 규칙으로 모형화한다.
//!
//! 공허한 확인 방지(MEMORY "공허한 확인"): 부정형 단언(쓰기가 0건이다) 앞에는 반드시 **존재 단언**(같은 조건에서
//! 쓰기가 실제로 나간다)을 둔다.

use bevy_ecs::prelude::*;
use loom_runtime::{
    patch_attr_system, patch_bool_prop_system, patch_class_toggle_system, patch_opt_attr_system, patch_style_system,
    patch_text_node_system, patch_text_system, AttrSite, AttrTarget, BoolProp, BoolPropSite, ClassSite, CountingDom,
    DomOps, DomTarget, DomWrite, ElementSpec, LoomApp, Namespace, NodeHandle, PendingDomWrites, Signal, StyleSite,
    SurfaceStats, SurfaceWrite, TextNodeTarget, WriteParent,
};
use std::collections::{BTreeMap, BTreeSet};

// ---------------------------------------------------------------------------
// 7c 전용 모의 DOM
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Kid {
    Node(u32),
    /// 핸들 없는 정적 텍스트(`append_static_text`).
    Anon(String),
}

#[derive(Debug, Clone, Default)]
struct N {
    ns: Option<Namespace>,
    tag: String,
    is_text: bool,
    data: String,
    attrs: BTreeMap<String, String>,
    classes: BTreeSet<String>,
    bools: BTreeMap<&'static str, bool>,
    style: BTreeMap<String, String>,
    kids: Vec<Kid>,
}

#[derive(Default)]
struct SurfDom {
    nodes: Vec<Option<N>>,
    free: Vec<u32>,
    pending_free: Vec<u32>,
    /// 새 표면 메서드가 불린 순서(`"이름:노드"`). 존재 단언과 개수 단언에 쓴다.
    log: Vec<String>,
}

impl SurfDom {
    fn new_root() -> (Self, NodeHandle) {
        let mut d = SurfDom::default();
        let r = d.alloc(N { ns: Some(Namespace::Html), tag: "root".into(), ..N::default() });
        (d, r)
    }

    fn alloc(&mut self, n: N) -> NodeHandle {
        let i = match self.free.pop() {
            Some(i) => {
                self.nodes[i as usize] = Some(n);
                i
            }
            None => {
                self.nodes.push(Some(n));
                (self.nodes.len() - 1) as u32
            }
        };
        NodeHandle(i)
    }

    fn n(&self, h: NodeHandle) -> &N {
        self.nodes[h.0 as usize].as_ref().unwrap_or_else(|| panic!("해제된 NodeHandle({})을 다시 썼다", h.0))
    }

    fn n_mut(&mut self, h: NodeHandle) -> &mut N {
        self.nodes[h.0 as usize].as_mut().unwrap_or_else(|| panic!("해제된 NodeHandle({})을 다시 썼다", h.0))
    }

    fn release(&mut self, h: u32) {
        let mut st = vec![h];
        while let Some(c) = st.pop() {
            if let Some(n) = self.nodes[c as usize].take() {
                for k in n.kids {
                    if let Kid::Node(x) = k {
                        st.push(x);
                    }
                }
                self.pending_free.push(c);
            }
        }
    }

    fn unlink(&mut self, h: u32) {
        for slot in self.nodes.iter_mut().flatten() {
            slot.kids.retain(|k| !matches!(k, Kid::Node(x) if *x == h));
        }
    }

    /// 직렬화 — `<li class="c" k="v">자식…</li>`, 텍스트 노드는 data.
    fn html(&self, h: NodeHandle) -> String {
        let n = self.n(h);
        if n.is_text {
            return n.data.clone();
        }
        let mut s = format!("<{}", n.tag);
        if n.ns != Some(Namespace::Html) {
            s.push_str(&format!(" ns={:?}", n.ns.unwrap()));
        }
        if !n.classes.is_empty() {
            s.push_str(&format!(" class=\"{}\"", n.classes.iter().cloned().collect::<Vec<_>>().join(" ")));
        }
        for (k, v) in &n.attrs {
            s.push_str(&format!(" {k}=\"{v}\""));
        }
        for (k, v) in &n.bools {
            s.push_str(&format!(" .{k}={v}"));
        }
        for (k, v) in &n.style {
            s.push_str(&format!(" style:{k}={v}"));
        }
        s.push('>');
        for k in &n.kids {
            match k {
                Kid::Node(x) => s.push_str(&self.html(NodeHandle(*x))),
                Kid::Anon(t) => s.push_str(t),
            }
        }
        s.push_str(&format!("</{}>", n.tag));
        s
    }

    fn kids_of(&self, h: NodeHandle) -> Vec<NodeHandle> {
        self.n(h).kids.iter().filter_map(|k| if let Kid::Node(x) = k { Some(NodeHandle(*x)) } else { None }).collect()
    }

    fn count(&self, prefix: &str) -> usize {
        self.log.iter().filter(|l| l.starts_with(prefix)).count()
    }
}

impl DomOps for SurfDom {
    fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
        self.create_element_ns(parent, Namespace::Html, tag, class)
    }
    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        self.n_mut(parent).kids.push(Kid::Anon(text.to_string()));
    }
    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        self.n_mut(parent).kids.push(Kid::Anon(html.to_string()));
    }
    /// `BrowserDom`과 같은 계약: 첫 텍스트 노드만 제자리 변경, 없으면 맨 앞에 하나 만든다. 엘리먼트 자식은 보존.
    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        let first_text = self.n(target).kids.iter().position(|k| match k {
            Kid::Anon(_) => true,
            Kid::Node(x) => self.nodes[*x as usize].as_ref().is_some_and(|n| n.is_text),
        });
        match first_text {
            Some(i) => match self.n(target).kids[i].clone() {
                Kid::Anon(_) => self.n_mut(target).kids[i] = Kid::Anon(text.to_string()),
                Kid::Node(x) => self.n_mut(NodeHandle(x)).data = text.to_string(),
            },
            None => self.n_mut(target).kids.insert(0, Kid::Anon(text.to_string())),
        }
    }
    fn clear_children(&mut self, parent: NodeHandle) {
        let kids = std::mem::take(&mut self.n_mut(parent).kids);
        for k in kids {
            if let Kid::Node(x) = k {
                self.release(x);
            }
        }
    }
    fn remove_node(&mut self, node: NodeHandle) {
        self.unlink(node.0);
        self.release(node.0);
    }
    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        self.unlink(node.0);
        let kids = &mut self.n_mut(parent).kids;
        let at = before
            .and_then(|b| kids.iter().position(|k| matches!(k, Kid::Node(x) if *x == b.0)))
            .unwrap_or(kids.len());
        kids.insert(at, Kid::Node(node.0));
    }
    fn begin_batch(&mut self) {
        self.free.append(&mut self.pending_free);
    }
    fn live_node_count(&self) -> Option<usize> {
        Some(self.nodes.iter().filter(|n| n.is_some()).count())
    }
    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        self.n_mut(node).style.insert("display".into(), if visible { "".into() } else { "none".into() });
    }
    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        self.log.push(format!("set_attribute:{}:{name}", node.0));
        self.n_mut(node).attrs.insert(name.into(), value.into());
    }
    fn adopt_children(&mut self, _p: NodeHandle) -> Vec<(NodeHandle, String)> {
        unreachable!("7c 테스트에서 불리면 안 된다")
    }
    fn set_input_value(&mut self, _n: NodeHandle, _v: &str) {}
    fn get_input_value(&mut self, _n: NodeHandle) -> String {
        unreachable!("7c 테스트에서 불리면 안 된다")
    }

    fn create_element_ns(&mut self, parent: NodeHandle, ns: Namespace, tag: &str, class: Option<&str>) -> NodeHandle {
        let mut n = N { ns: Some(ns), tag: tag.into(), ..N::default() };
        if let Some(c) = class {
            n.classes.insert(c.into());
        }
        let h = self.alloc(n);
        self.n_mut(parent).kids.push(Kid::Node(h.0));
        self.log.push(format!("create_element_ns:{}", h.0));
        h
    }
    fn remove_attribute(&mut self, node: NodeHandle, name: &str) {
        self.log.push(format!("remove_attribute:{}:{name}", node.0));
        self.n_mut(node).attrs.remove(name);
    }
    fn toggle_class(&mut self, node: NodeHandle, class: &str, on: bool) {
        self.log.push(format!("toggle_class:{}:{class}:{on}", node.0));
        let c = &mut self.n_mut(node).classes;
        if on {
            c.insert(class.into());
        } else {
            c.remove(class);
        }
    }
    fn set_bool_prop(&mut self, node: NodeHandle, prop: BoolProp, value: bool) {
        self.log.push(format!("set_bool_prop:{}:{}:{value}", node.0, prop.js_name()));
        self.n_mut(node).bools.insert(prop.js_name(), value);
    }
    fn create_text_node(&mut self, parent: NodeHandle, text: &str) -> NodeHandle {
        let h = self.alloc(N { is_text: true, data: text.into(), ..N::default() });
        self.n_mut(parent).kids.push(Kid::Node(h.0));
        self.log.push(format!("create_text_node:{}", h.0));
        h
    }
    fn set_text_data(&mut self, node: NodeHandle, text: &str) {
        self.log.push(format!("set_text_data:{}", node.0));
        let n = self.n_mut(node);
        assert!(n.is_text, "set_text_data가 텍스트 노드가 아닌 곳({})에 갔다", node.0);
        n.data = text.into();
    }
    fn set_style(&mut self, node: NodeHandle, prop: &str, value: Option<&str>) {
        self.log.push(format!("set_style:{}:{prop}", node.0));
        let s = &mut self.n_mut(node).style;
        match value {
            Some(v) => {
                s.insert(prop.into(), v.into());
            }
            None => {
                s.remove(prop);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 정적 스펙과 사이트 마커
// ---------------------------------------------------------------------------

static SPEC_LI: ElementSpec =
    ElementSpec { ns: Namespace::Html, tag: "li", class: Some("c"), loom_id: Some("row"), attrs: &[("data-k", "v")] };
static SPEC_SVG: ElementSpec =
    ElementSpec { ns: Namespace::Svg, tag: "g", class: Some("s"), loom_id: None, attrs: &[("fill", "red")] };
static SPEC_PLAIN: ElementSpec = ElementSpec { ns: Namespace::Html, tag: "div", class: None, loom_id: None, attrs: &[] };

macro_rules! attr_site {
    ($t:ident, $name:expr) => {
        #[derive(Component)]
        struct $t;
        impl AttrSite for $t {
            const NAME: &'static str = $name;
        }
    };
}
attr_site!(TitleSite, "title");
attr_site!(AltSite, "alt");

#[derive(Component)]
struct OnClass;
impl ClassSite for OnClass {
    const CLASS: &'static str = "on";
}
#[derive(Component)]
struct CheckedSite;
impl BoolPropSite for CheckedSite {
    const PROP: BoolProp = BoolProp::Checked;
}
#[derive(Component)]
struct ColorSite;
impl StyleSite for ColorSite {
    const PROP: &'static str = "color";
}

fn push(app: &mut LoomApp, w: SurfaceWrite) {
    app.world.resource_mut::<PendingDomWrites>().0.push(DomWrite::Surface(w));
}

// ---------------------------------------------------------------------------
// 1. variant별 적용 결과
// ---------------------------------------------------------------------------

#[test]
fn spawn_spec_and_row_spec_apply_ns_class_loom_id_attrs_and_text() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    let (a, b, c) = (app.world.spawn_empty().id(), app.world.spawn_empty().id(), app.world.spawn_empty().id());
    push(&mut app, SurfaceWrite::SpawnSpec { entity: a, parent: WriteParent::Node(root), spec: &SPEC_LI, text: Some("hi".into()) });
    push(&mut app, SurfaceWrite::SpawnSpec { entity: b, parent: WriteParent::Node(root), spec: &SPEC_SVG, text: None });
    push(
        &mut app,
        SurfaceWrite::SpawnRowSpec { entity: c, parent: root, spec: &SPEC_PLAIN, text: Some("r".into()), before: Some(a) },
    );
    app.tick(&mut dom);

    let ha = app.world.get::<DomTarget>(a).expect("SpawnSpec은 DomTarget을 단다").0;
    let hb = app.world.get::<DomTarget>(b).unwrap().0;
    let hc = app.world.get::<DomTarget>(c).expect("SpawnRowSpec도 DomTarget을 단다(리스트 기계와 호환)").0;
    assert_eq!(dom.html(ha), "<li class=\"c\" data-k=\"v\" data-loom-id=\"row\">hi</li>");
    assert_eq!(dom.html(hb), "<g ns=Svg class=\"s\" fill=\"red\"></g>", "SVG 네임스페이스가 실제로 실려야 한다(R43)");
    assert_eq!(dom.html(hc), "<div>r</div>");
    // before=a → c가 a 앞에: 순서 [c, a, b]
    assert_eq!(dom.kids_of(root), vec![hc, ha, hb]);
    assert_eq!(app.dropped_writes, 0);
}

#[test]
fn attr_class_bool_style_text_variants_apply() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    let el = app.world.spawn_empty().id();
    let tn = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnSpec { entity: el, parent: WriteParent::Node(root), spec: &SPEC_PLAIN, text: None });
    push(&mut app, SurfaceWrite::SetAttr { node: WriteParent::Entity(el), name: "title", value: "t".into() });
    push(&mut app, SurfaceWrite::ToggleClass { node: WriteParent::Entity(el), class: "on", on: true });
    push(&mut app, SurfaceWrite::SetBoolProp { node: WriteParent::Entity(el), prop: BoolProp::Checked, value: true });
    push(&mut app, SurfaceWrite::SetStyle { node: WriteParent::Entity(el), prop: "color", value: Some("red".into()) });
    push(&mut app, SurfaceWrite::SpawnText { entity: tn, parent: WriteParent::Entity(el), text: "5".into() });
    app.tick(&mut dom);
    let h = app.world.get::<DomTarget>(el).unwrap().0;
    assert_eq!(dom.html(h), "<div class=\"on\" title=\"t\" .checked=true style:color=red>5</div>");

    let t = app.world.get::<TextNodeTarget>(tn).expect("SpawnText는 TextNodeTarget을 단다").0;
    push(&mut app, SurfaceWrite::SetTextData { node: t, text: "6".into() });
    push(&mut app, SurfaceWrite::RemoveAttr { node: WriteParent::Entity(el), name: "title" });
    push(&mut app, SurfaceWrite::ToggleClass { node: WriteParent::Entity(el), class: "on", on: false });
    push(&mut app, SurfaceWrite::SetStyle { node: WriteParent::Entity(el), prop: "color", value: None });
    app.tick(&mut dom);
    assert_eq!(dom.html(h), "<div .checked=true>6</div>");
}

// ---------------------------------------------------------------------------
// 2. 같은 배치의 Entity 부모 해석, BindAttrTarget이 초기값을 굽는지
// ---------------------------------------------------------------------------

#[test]
fn bind_attr_target_bakes_initial_value_in_same_batch() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    let el = app.world.spawn_empty().id();
    let bind = app.world.spawn((Signal("v0".to_string()), TitleSite)).id();
    push(&mut app, SurfaceWrite::SpawnSpec { entity: el, parent: WriteParent::Node(root), spec: &SPEC_PLAIN, text: None });
    push(&mut app, SurfaceWrite::BindAttrTarget { binding: bind, element: WriteParent::Entity(el) });
    push(&mut app, SurfaceWrite::SetAttr { node: WriteParent::Entity(el), name: "title", value: "v0".into() });
    app.tick(&mut dom);
    let h = app.world.get::<DomTarget>(el).unwrap().0;
    assert_eq!(dom.html(h), "<div title=\"v0\"></div>");
    assert_eq!(app.world.get::<AttrTarget>(bind).expect("바인딩 entity에 AttrTarget이 붙어야 한다").0, h);
    assert_eq!(app.dropped_writes, 0);
    // 부모가 해석되지 않으면 조용히 넘기지 않고 센다(방출 순서 위반 = 방출 측 버그).
    let ghost = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SetAttr { node: WriteParent::Entity(ghost), name: "x", value: "y".into() });
    app.tick(&mut dom);
    assert_eq!(app.dropped_writes, 1);
}

// ---------------------------------------------------------------------------
// 3. 앵커 miss — 존재 단언이 먼저
// ---------------------------------------------------------------------------

#[test]
fn anchor_miss_is_counted_on_new_path_only_when_it_happens() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    // 정상 경로: 앵커가 해석된다 → 0.
    let a = app.world.spawn_empty().id();
    let b = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: a, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: b, parent: root, spec: &SPEC_PLAIN, text: None, before: Some(a) });
    app.tick(&mut dom);
    let misses = app.world.get_resource::<SurfaceStats>().map(|s| s.anchor_misses).unwrap_or(0);
    assert_eq!(misses, 0, "정상 경로에서는 miss가 없다");

    // 존재 단언: 의도적으로 틀린 앵커(아직 DomTarget이 없는 entity)를 주면 센다 — 이게 안 되면 위 0은 공허하다.
    let never_mounted = app.world.spawn_empty().id();
    let c = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: c, parent: root, spec: &SPEC_PLAIN, text: None, before: Some(never_mounted) });
    app.tick(&mut dom);
    assert_eq!(app.world.resource::<SurfaceStats>().anchor_misses, 1);
    // miss여도 row는 끝에 붙고 DomTarget이 달린다(동작은 SpawnRow와 같고 집계만 새로 생긴다).
    let hc = app.world.get::<DomTarget>(c).unwrap().0;
    assert_eq!(dom.kids_of(root).last().copied(), Some(hc));
}

// ---------------------------------------------------------------------------
// 4. 혼합 텍스트 — 형제 정적 텍스트 보존
// ---------------------------------------------------------------------------

#[test]
fn mixed_text_updates_only_the_dynamic_sibling_text_node() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(patch_text_node_system::<i32>);
    let p = app.world.spawn_empty().id();
    let n = app.world.spawn((Signal(5_i32), ChildOf(p))).id();
    push(&mut app, SurfaceWrite::SpawnSpec { entity: p, parent: WriteParent::Node(root), spec: &SPEC_PLAIN, text: Some("Count: ".into()) });
    push(&mut app, SurfaceWrite::SpawnText { entity: n, parent: WriteParent::Entity(p), text: "5".into() });
    app.tick(&mut dom);
    let hp = app.world.get::<DomTarget>(p).unwrap().0;
    assert_eq!(dom.html(hp), "<div>Count: 5</div>");
    // 첫 틱은 Added=Changed지만 이 binding에는 TextNodeTarget이 틱 끝에 붙었으므로 그 틱엔 쓰기가 없었다. 값을 바꾼다.
    app.world.get_mut::<Signal<i32>>(n).unwrap().0 = 6;
    app.tick(&mut dom);
    assert_eq!(dom.html(hp), "<div>Count: 6</div>", "정적 형제 'Count: '가 보존되고 동적 조각만 바뀐다");
    assert_eq!(dom.count("set_text_data"), 1);
}

// ---------------------------------------------------------------------------
// 5. 교차 매칭 — 자기 사이트로는 먼저 나가는지(존재), 상대 사이트로는 0건인지
// ---------------------------------------------------------------------------

fn drained(app: &mut LoomApp) -> Vec<DomWrite> {
    app.run_schedule_for_test();
    std::mem::take(&mut app.world.resource_mut::<PendingDomWrites>().0)
}

#[test]
fn cross_match_text_system_vs_attr_system() {
    let mut app = LoomApp::new();
    app.add_systems((patch_text_system::<String>, patch_attr_system::<String, TitleSite>));
    // (가) 엘리먼트 entity: Signal<String> + DomTarget → SetText만 나가야 한다.
    app.world.spawn((Signal("elem".to_string()), DomTarget(NodeHandle(1))));
    // (나) 속성 binding entity: Signal<String> + AttrTarget(+마커) → SetAttr만. DomTarget이 없으므로 SetText는 원천 불가.
    app.world.spawn((Signal("attr".to_string()), AttrTarget(NodeHandle(2)), TitleSite));
    let w = drained(&mut app);
    let set_text: Vec<_> = w.iter().filter(|x| matches!(x, DomWrite::SetText { .. })).collect();
    let set_attr: Vec<_> =
        w.iter().filter(|x| matches!(x, DomWrite::Surface(SurfaceWrite::SetAttr { .. }))).collect();
    // 존재 단언: 각자의 사이트로는 정확히 1건씩 나간다.
    assert_eq!(set_text.len(), 1, "{w:?}");
    assert_eq!(set_attr.len(), 1, "{w:?}");
    // 상대 사이트로의 교차는 0건: SetText의 목적지는 노드 1(엘리먼트)뿐, SetAttr의 목적지는 노드 2뿐.
    assert!(matches!(set_text[0], DomWrite::SetText { target, .. } if target.0 == 1));
    assert!(matches!(set_attr[0], DomWrite::Surface(SurfaceWrite::SetAttr { node: WriteParent::Node(n), .. }) if n.0 == 2));
    assert_eq!(w.len(), 2);
}

#[test]
fn cross_match_two_attr_sites_with_same_value_type() {
    let mut app = LoomApp::new();
    app.add_systems((patch_attr_system::<String, TitleSite>, patch_attr_system::<String, AltSite>));
    app.world.spawn((Signal("t".to_string()), AttrTarget(NodeHandle(1)), TitleSite));
    app.world.spawn((Signal("a".to_string()), AttrTarget(NodeHandle(2)), AltSite));
    let w = drained(&mut app);
    // 존재: 두 사이트 각각 1건.
    let mut got: Vec<(u32, &str)> = w
        .iter()
        .filter_map(|x| match x {
            DomWrite::Surface(SurfaceWrite::SetAttr { node: WriteParent::Node(n), name, .. }) => Some((n.0, *name)),
            _ => None,
        })
        .collect();
    got.sort();
    // 교차 0건: 노드1은 title로만, 노드2는 alt로만(마커 없이 `<String>` 하나였다면 각 노드에 두 이름이 다 나간다).
    assert_eq!(got, vec![(1, "title"), (2, "alt")]);
}

#[test]
fn opt_attr_class_bool_style_systems_emit_the_right_variant() {
    let mut app = LoomApp::new();
    app.add_systems((
        patch_opt_attr_system::<String, TitleSite>,
        patch_class_toggle_system::<OnClass>,
        patch_bool_prop_system::<CheckedSite>,
        patch_style_system::<ColorSite>,
    ));
    let o = app.world.spawn((Signal(Some("x".to_string())), AttrTarget(NodeHandle(1)), TitleSite)).id();
    app.world.spawn((Signal(true), AttrTarget(NodeHandle(2)), OnClass));
    app.world.spawn((Signal(false), AttrTarget(NodeHandle(3)), CheckedSite));
    app.world.spawn((Signal(Some("red".to_string())), AttrTarget(NodeHandle(4)), ColorSite));
    let w = drained(&mut app);
    assert_eq!(w.len(), 4, "{w:?}");
    let has = |f: &dyn Fn(&SurfaceWrite) -> bool| w.iter().any(|x| matches!(x, DomWrite::Surface(s) if f(s)));
    assert!(has(&|s| matches!(s, SurfaceWrite::SetAttr { name: "title", value, .. } if value == "x")));
    assert!(has(&|s| matches!(s, SurfaceWrite::ToggleClass { class: "on", on: true, .. })));
    assert!(has(&|s| matches!(s, SurfaceWrite::SetBoolProp { prop: BoolProp::Checked, value: false, .. })));
    assert!(has(&|s| matches!(s, SurfaceWrite::SetStyle { prop: "color", value: Some(v), .. } if v == "red")));
    // None → RemoveAttr
    app.world.get_mut::<Signal<Option<String>>>(o).unwrap().0 = None;
    let w = drained(&mut app);
    assert_eq!(w.len(), 1);
    assert!(matches!(&w[0], DomWrite::Surface(SurfaceWrite::RemoveAttr { name: "title", .. })));
}

// ---------------------------------------------------------------------------
// 6. 게이팅과 §2.2 재패치 위험 — 설계 §1.1
// ---------------------------------------------------------------------------

/// row 스코프 binding을 `Commands`로 같은 tick에 만드는 방출 System. 한 번만 돈다.
fn spawner(mut commands: Commands, mut done: Local<bool>, mut pending: ResMut<PendingDomWrites>, root: Res<RootHandle>) {
    if *done {
        return;
    }
    *done = true;
    let row = commands.spawn_empty().id();
    let bind = commands.spawn((Signal("init".to_string()), TitleSite, ChildOf(row))).id();
    let parent = root.0;
    pending.0.push(DomWrite::Surface(SurfaceWrite::SpawnRowSpec { entity: row, parent, spec: &SPEC_PLAIN, text: None, before: None }));
    pending.0.push(DomWrite::Surface(SurfaceWrite::BindAttrTarget { binding: bind, element: WriteParent::Entity(row) }));
    // 초기값은 스포너가 직접 굽는다(설계 §1.1).
    pending.0.push(DomWrite::Surface(SurfaceWrite::SetAttr { node: WriteParent::Entity(row), name: "title", value: "init".into() }));
}

fn title_sets(dom: &SurfDom) -> usize {
    dom.log.iter().filter(|l| l.ends_with(":title") && l.starts_with("set_attribute")).count()
}

#[test]
fn setup_spawned_binding_patches_once_then_never() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(patch_attr_system::<String, TitleSite>);
    let el = app.world.spawn_empty().id();
    let bind = app.world.spawn((Signal("v".to_string()), TitleSite)).id();
    push(&mut app, SurfaceWrite::SpawnSpec { entity: el, parent: WriteParent::Node(root), spec: &SPEC_PLAIN, text: None });
    push(&mut app, SurfaceWrite::BindAttrTarget { binding: bind, element: WriteParent::Entity(el) });
    app.tick(&mut dom); // 바인딩은 틱 끝에 AttrTarget을 받는다 — 이 틱의 patch는 아직 못 본다.
    let after_mount = title_sets(&dom);
    app.tick(&mut dom);
    // setup 경로에서 만든 Signal의 Added 틱은 첫 틱의 patch 실행 이전이다 → 다음 틱에 Changed가 거짓.
    // 존재 단언은 아래 (b)의 hazard 테스트와 값 변경 후 1건 나가는 것으로 건다.
    assert_eq!(title_sets(&dom), after_mount, "값이 안 바뀌었는데 재패치가 났다");
    app.world.get_mut::<Signal<String>>(bind).unwrap().0 = "w".into();
    app.tick(&mut dom);
    assert_eq!(title_sets(&dom), after_mount + 1, "값 변경은 정확히 1건");
}

#[test]
fn commands_spawned_row_binding_is_not_repatched_when_patch_is_ordered_after_sync() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    // 계약: patch System은 spawn의 sync point(Commands 적용) **뒤에** 둔다 — 기존 RowBinding 패치와 같은 규칙.
    app.add_systems((spawner, patch_attr_system::<String, TitleSite>).chain());
    // 방출 System이 부모 핸들을 알아야 한다.
    app.world.insert_resource(RootHandle(root));
    app.tick(&mut dom);
    assert_eq!(title_sets(&dom), 1, "스폰한 틱의 초기값은 스포너가 굽는 1건뿐");
    app.tick(&mut dom);
    assert_eq!(title_sets(&dom), 1, "다음 틱에 Changed가 참이 되어 한 번 더 나가면 §2.2의 '초기값 후속 재패치'다");
}

#[test]
fn hazard_exists_when_patch_is_ordered_before_the_spawner() {
    // 존재 단언: 순서를 뒤집으면 재패치가 **실제로** 난다 — 위 테스트의 0이 메커니즘 부재가 아니라 성질 때문임을 보인다.
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems((patch_attr_system::<String, TitleSite>, spawner).chain());
    app.world.insert_resource(RootHandle(root));
    app.tick(&mut dom);
    assert_eq!(title_sets(&dom), 1);
    app.tick(&mut dom);
    assert_eq!(title_sets(&dom), 2, "patch가 spawn 앞에 있으면 다음 틱에 초기값이 재패치된다(설계 §1.1의 위험)");
}

#[derive(Resource)]
struct RootHandle(NodeHandle);

// ---------------------------------------------------------------------------
// 7. 수명 — 핸들 슬롯 재사용 뒤 옛 binding이 다른 노드에 쓰지 않는다
// ---------------------------------------------------------------------------

#[test]
fn child_of_binding_dies_with_its_row_and_never_writes_to_a_reused_slot() {
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(patch_text_node_system::<String>);
    let row = app.world.spawn_empty().id();
    let tn = app.world.spawn((Signal("a".to_string()), ChildOf(row))).id();
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: row, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    push(&mut app, SurfaceWrite::SpawnText { entity: tn, parent: WriteParent::Entity(row), text: "a".into() });
    app.tick(&mut dom);
    let old_text_handle = app.world.get::<TextNodeTarget>(tn).unwrap().0;
    let row_handle = app.world.get::<DomTarget>(row).unwrap().0;

    // row 삭제: DOM에서 RemoveNode, ECS에서 despawn(소유자를 지우면 ChildOf binding도 함께 사라진다).
    app.world.entity_mut(row).despawn();
    assert!(app.world.get_entity(tn).is_err(), "ChildOf로 매단 binding은 소유자와 함께 사라져야 한다(linked_spawn)");
    app.world.resource_mut::<PendingDomWrites>().0.push(DomWrite::RemoveNode { node: row_handle });
    app.tick(&mut dom);

    // 슬롯 재사용: 새 노드가 옛 텍스트 노드의 슬롯을 받는다 — 존재 단언(같은 인덱스).
    let fresh = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: fresh, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    let fresh2 = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: fresh2, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    app.tick(&mut dom);
    let reused: Vec<u32> = [fresh, fresh2].iter().map(|e| app.world.get::<DomTarget>(*e).unwrap().0 .0).collect();
    assert!(reused.contains(&old_text_handle.0), "슬롯이 실제로 재사용되지 않았다 — 이 테스트는 공허하다: {reused:?} vs {old_text_handle:?}");

    // 이제 옛 binding은 없다. (살아 있었다면 Signal 변경이 재사용된 슬롯에 SetTextData를 보냈을 것이다.)
    let before = dom.count("set_text_data");
    app.tick(&mut dom);
    assert_eq!(dom.count("set_text_data"), before);
}

#[test]
fn orphan_binding_without_child_of_would_write_to_the_reused_slot() {
    // 음성 대조군: ChildOf를 안 달면 옛 binding이 남아 재사용된 슬롯(다른 노드)에 쓴다 — 규약이 필요한 이유의 시연.
    let (mut dom, root) = SurfDom::new_root();
    let mut app = LoomApp::new();
    app.add_systems(patch_text_node_system::<String>);
    let row = app.world.spawn_empty().id();
    let orphan = app.world.spawn(Signal("a".to_string())).id(); // ChildOf 없음
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: row, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    push(&mut app, SurfaceWrite::SpawnText { entity: orphan, parent: WriteParent::Entity(row), text: "a".into() });
    app.tick(&mut dom);
    let row_handle = app.world.get::<DomTarget>(row).unwrap().0;
    let stale = app.world.get::<TextNodeTarget>(orphan).unwrap().0;
    app.world.entity_mut(row).despawn();
    app.world.resource_mut::<PendingDomWrites>().0.push(DomWrite::RemoveNode { node: row_handle });
    app.tick(&mut dom);
    // 재사용될 때까지 새 row를 만든다. 재사용 슬롯이 **텍스트 노드가 아니라** 엘리먼트면 set_text_data가 단언에 걸려 패닉한다.
    let a = app.world.spawn_empty().id();
    let b = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: a, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    push(&mut app, SurfaceWrite::SpawnRowSpec { entity: b, parent: root, spec: &SPEC_PLAIN, text: None, before: None });
    app.tick(&mut dom);
    assert!([a, b].iter().any(|e| app.world.get::<DomTarget>(*e).unwrap().0 == stale), "슬롯이 재사용돼야 한다");
    app.world.get_mut::<Signal<String>>(orphan).unwrap().0 = "b".into();
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| app.tick(&mut dom)));
    assert!(r.is_err(), "고아 binding이 재사용된 슬롯에 썼어야 한다(ChildOf 규약이 없으면 이렇게 깨진다)");
}

// ---------------------------------------------------------------------------
// 크기·판별값
// ---------------------------------------------------------------------------

#[test]
fn dom_write_size_is_unchanged_by_surface() {
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<DomWrite>(), 88, "7c 이전 실측과 같아야 한다(x86_64)");
    #[cfg(target_pointer_width = "32")]
    assert_eq!(std::mem::size_of::<DomWrite>(), 56, "7c 이전 실측과 같아야 한다(wasm32)");
}

/// 기존 variant들의 판별값이 `Surface` 추가 뒤에도 같다 — `Surface`가 마지막이어야 한다는 doc의 근거.
/// 값은 7c 이전 HEAD에서 같은 방식(첫 8바이트)으로 읽은 niche 인코딩이다(x86_64, 설계 §0 실측).
#[test]
#[cfg(target_arch = "x86_64")]
fn existing_variant_discriminants_are_pinned() {
    fn tag(w: &DomWrite) -> u64 {
        let p = w as *const DomWrite as *const u64;
        unsafe { p.read() }
    }
    let e = Entity::from_raw_u32(1).unwrap();
    let h = NodeHandle(1);
    assert_eq!(tag(&DomWrite::SetText { target: h, text: String::new() }), 0x8000_0000_0000_0000);
    assert_eq!(tag(&DomWrite::ClearChildren { parent: h }), 0x8000_0000_0000_0001);
    assert_eq!(tag(&DomWrite::RemoveNode { node: h }), 0x8000_0000_0000_0004);
    assert_eq!(tag(&DomWrite::SetVisible { node: h, visible: true }), 0x8000_0000_0000_0005);
    assert_eq!(tag(&DomWrite::SetInputValue { node: h, value: String::new() }), 0x8000_0000_0000_000a);
    let _ = e;
}

// ---------------------------------------------------------------------------
// 위임 테스트 — CountingDom
// ---------------------------------------------------------------------------

#[test]
fn counting_dom_delegates_and_counts_every_new_method_without_touching_dom_op_counts() {
    let (inner, root) = SurfDom::new_root();
    let mut dom = CountingDom::new(inner);
    let before_json = dom.counts().to_json();
    let el = dom.create_element_ns(root, Namespace::Svg, "g", Some("c"));
    dom.remove_attribute(el, "x");
    dom.toggle_class(el, "on", true);
    dom.set_bool_prop(el, BoolProp::Selected, true);
    let t = dom.create_text_node(el, "1");
    dom.set_text_data(t, "2");
    dom.set_style(el, "color", Some("red"));
    let c = dom.surface_counts();
    assert_eq!(
        (c.create_element_ns, c.remove_attribute, c.toggle_class, c.set_bool_prop, c.create_text_node, c.set_text_data, c.set_style),
        (1, 1, 1, 1, 1, 1, 1)
    );
    assert_eq!(c.total_mutations(), 7);
    // inner에 효과가 갔다.
    let h = dom.inner_mut().html(el);
    assert_eq!(h, "<g ns=Svg class=\"c on\" .selected=true style:color=red>2</g>");
    // 기존 DomOpCounts는 새 메서드로 한 칸도 움직이지 않았고 to_json 출력도 같다(측정 정의 불변).
    assert_eq!(dom.counts().to_json(), before_json);
    assert_eq!(dom.counts().total_mutations(), 0);
    // reset은 둘 다 0으로.
    dom.reset_counts();
    assert_eq!(dom.surface_counts().total_mutations(), 0);
}

#[test]
fn surface_set_attribute_is_counted_in_the_existing_counter() {
    let (inner, root) = SurfDom::new_root();
    let mut dom = CountingDom::new(inner);
    let mut app = LoomApp::new();
    let el = app.world.spawn_empty().id();
    push(&mut app, SurfaceWrite::SpawnSpec { entity: el, parent: WriteParent::Node(root), spec: &SPEC_LI, text: None });
    app.tick(&mut dom);
    // SPEC_LI: loom_id 1 + attrs 1 = set_attribute 2회(기존 카운터), create_element_ns 1회(신규 카운터).
    assert_eq!(dom.counts().set_attribute, 2);
    assert_eq!(dom.surface_counts().create_element_ns, 1);
    assert_eq!(dom.counts().create_element, 0, "기존 create_element 카운터는 움직이지 않는다");
}

// ---------------------------------------------------------------------------
// 구조 단언 (판정례 9(다)의 선례: 소스 grep) — 존재 단언(본문을 실제로 찾았는가)이 먼저
// ---------------------------------------------------------------------------

fn src(rel: &str) -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// `fn name`의 본문(중괄호 균형)을 돌려준다. 못 찾으면 패닉 — grep 단언이 빈 문자열을 통과하는 공허함을 막는다.
fn fn_body(source: &str, sig: &str) -> String {
    let at = source.find(sig).unwrap_or_else(|| panic!("함수 `{sig}`를 찾지 못했다"));
    let open = source[at..].find('{').unwrap() + at;
    let mut depth = 0usize;
    for (i, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return source[open..open + i + 1].to_string();
                }
            }
            _ => {}
        }
    }
    panic!("`{sig}` 본문의 끝을 찾지 못했다");
}

#[test]
fn structural_apply_surface_never_reads_the_dom() {
    let s = src("src/surface.rs");
    let body = fn_body(&s, "pub(crate) fn apply_surface");
    assert!(body.lines().count() > 20, "본문을 실제로 찾았는지(줄 수 > 0) 먼저 단언한다");
    for banned in ["get_input_value", "adopt_children", "adopt_element_children", "get_attribute", "text_content"] {
        assert!(!body.contains(banned), "apply_surface가 DOM을 읽는다: {banned}");
    }
    let helper = fn_body(&s, "fn create_from_spec");
    assert!(helper.lines().count() > 3);
    for banned in ["get_input_value", "adopt_", "get_attribute"] {
        assert!(!helper.contains(banned));
    }
}

#[test]
fn structural_only_adopt_element_children_reads_in_web_rs() {
    let w = src("src/web.rs");
    for name in [
        "fn create_element_ns",
        "fn remove_attribute",
        "fn toggle_class",
        "fn set_bool_prop",
        "fn create_text_node",
        "fn set_text_data",
        "fn set_style",
    ] {
        let body = fn_body(&w, name);
        assert!(body.lines().count() > 1, "{name} 본문을 찾지 못했다");
        for read in ["get_attribute", "contains(", "text_content", ".value()", "get_property_value", ".data()", "children()"] {
            assert!(!body.contains(read), "쓰기 메서드 {name}가 DOM을 읽는다: {read}");
        }
    }
    // 존재 단언: 읽기 API 목록이 실제로 걸러낼 수 있는 대상이 있다 — 허용된 유일한 읽기 메서드.
    let adopt = fn_body(&w, "fn adopt_element_children");
    assert!(adopt.contains("children()") && adopt.contains("text_content"));
}

#[test]
fn structural_measured_paths_do_not_reference_surface() {
    // list.rs / cond.rs / loom-flush-core는 Surface를 방출하거나 참조하지 않는다(측정 arm이 7c 경로를 안 탄다).
    let list = src("src/list.rs");
    let cond = src("src/cond.rs");
    let core = src("../loom-flush-core/src/lib.rs");
    for (name, text) in [("list.rs", &list), ("cond.rs", &cond), ("loom-flush-core", &core)] {
        assert!(text.len() > 1000, "{name}를 실제로 읽었는지");
        for banned in ["SurfaceWrite", "ElementSpec", "DomWrite::Surface", "surface::"] {
            assert!(!text.contains(banned), "{name}이 {banned}를 참조한다");
        }
    }
}

#[test]
fn structural_no_site_marker_value_is_stored_in_a_component() {
    // 금지 4: 이름/클래스 문자열은 연관 상수로만 — Component 필드에 &'static str/fn 포인터가 없다.
    let s = src("src/surface.rs");
    for comp in ["pub struct AttrTarget(", "pub struct TextNodeTarget("] {
        let at = s.find(comp).unwrap_or_else(|| panic!("{comp}"));
        let field = &s[at + comp.len()..s[at..].find(");").unwrap() + at];
        assert_eq!(field, "pub NodeHandle", "Component 필드는 정수 핸들뿐이어야 한다: {field}");
    }
}

// ---------------------------------------------------------------------------
// tripwire — 측정 경로가 Surface를 방출하지 않는다 (검출기 자체를 먼저 시험)
// ---------------------------------------------------------------------------

fn surface_count(w: &[DomWrite]) -> usize {
    w.iter().filter(|x| matches!(x, DomWrite::Surface(_))).count()
}

#[test]
fn tripwire_detector_counts_a_planted_surface_write() {
    let planted = vec![DomWrite::SetText { target: NodeHandle(1), text: "x".into() }, DomWrite::Surface(SurfaceWrite::SetTextData { node: NodeHandle(1), text: "y".into() })];
    assert_eq!(surface_count(&planted), 1, "검출기가 심어 둔 1건을 못 세면 아래 0 단언은 전부 공허하다");
}

#[test]
fn tripwire_existing_list_and_cond_paths_emit_zero_surface_writes() {
    use loom_runtime::{list_reconcile_c_system, list_reconcile_d_lis_system, list_reconcile_d_system, list_reconcile_k_system, ListRows};
    let mut total = 0usize;
    let mut writes_seen = 0usize;
    for which in 0..4 {
        let mut app = LoomApp::new();
        match which {
            0 => app.add_systems(list_reconcile_k_system::<String>),
            1 => app.add_systems(list_reconcile_c_system::<String>),
            2 => app.add_systems(list_reconcile_d_system::<String>),
            _ => app.add_systems(list_reconcile_d_lis_system::<String>),
        };
        app.world.spawn((
            Signal(vec!["a".to_string(), "b".to_string(), "c".to_string()]),
            ListRows::default(),
            DomTarget(NodeHandle(0)),
        ));
        let w = drained(&mut app);
        assert!(w.len() >= 3, "존재 단언: 후보 {which}가 실제로 write를 냈다({})", w.len());
        writes_seen += w.len();
        total += surface_count(&w);
    }
    assert!(writes_seen >= 12);
    assert_eq!(total, 0);
}

#[test]
fn trait_defaults_panic_for_backends_without_the_surface() {
    struct Bare;
    impl DomOps for Bare {
        fn create_element(&mut self, _: NodeHandle, _: &str, _: Option<&str>) -> NodeHandle { NodeHandle(0) }
        fn append_static_text(&mut self, _: NodeHandle, _: &str) {}
        fn append_static_html(&mut self, _: NodeHandle, _: &str) {}
        fn set_text_content(&mut self, _: NodeHandle, _: &str) {}
        fn clear_children(&mut self, _: NodeHandle) {}
        fn remove_node(&mut self, _: NodeHandle) {}
        fn reposition(&mut self, _: NodeHandle, _: NodeHandle, _: Option<NodeHandle>) {}
        fn set_visible(&mut self, _: NodeHandle, _: bool) {}
        fn set_attribute(&mut self, _: NodeHandle, _: &str, _: &str) {}
        fn adopt_children(&mut self, _: NodeHandle) -> Vec<(NodeHandle, String)> { vec![] }
        fn set_input_value(&mut self, _: NodeHandle, _: &str) {}
        fn get_input_value(&mut self, _: NodeHandle) -> String { String::new() }
    }
    let r = std::panic::catch_unwind(|| {
        let mut b = Bare;
        b.create_element_ns(NodeHandle(0), Namespace::Svg, "g", None);
    });
    assert!(r.is_err(), "create_element_ns가 조용히 create_element로 폴백하면 R43 같은 결함을 되만든다");
}
