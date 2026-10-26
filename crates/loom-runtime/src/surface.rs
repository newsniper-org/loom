//! **Phase 7c — 컴파일러가 7b에서 쓸 최소 런타임 표면** (2026-10-26, `DomWrite::Surface`).
//!
//! 이 모듈은 기존 `DomWrite` 12종·`tick`의 기존 arm·`DomOpCounts`(M2 정의)를 **한 바이트도 바꾸지 않고**, 새
//! 변형을 하나(`DomWrite::Surface`, 반드시 **마지막** variant)만 붙여 아래를 싣는다: 엘리먼트 네임스페이스(SVG),
//! 정적 속성 묶음, 동적 속성·class 토글·불리언 프로퍼티·style, 혼합 텍스트(형제 텍스트 노드).
//!
//! # 이 경로의 지위 (7b 전용)
//! 소비자는 7b에서 바뀔 **컴파일러 생성 코드**뿐이다. RQ1·RQ6 측정 arm(K/C/D/E/T, 트랙 N)은 v23에서 이 경로를 쓰지
//! 않는다 — 그래서 측정 기반 안에서 이 모듈은 "들어 있지만 실행되지 않는 코드"다. 그 사실은 write-stream에서
//! `Surface` 0건을 세는 tripwire 테스트(`tests/surface_writes.rs`)가 고정한다.
//!
//! # No-VDOM (docs/03 "No-VDOM 판별 기준", N1–N4)
//! - `ElementSpec`은 컴파일러가 `static`으로 굽는 **정적 생성 지시**다. 갱신마다 만들어지지 않고(비용 N3), 어떤
//!   비교·조정 알고리즘의 입력도 되지 않는다(N1) — 무저촉의 근거는 `PartialEq` 부재가 아니라 이 두 사실이며,
//!   `tests/surface_writes.rs`의 구조 단언이 `list.rs`/`cond.rs`/`loom-flush-core`가 이 모듈을 참조하지 않음을 고정한다.
//! - patch System은 `Changed<Signal<_>>`로 게이팅되고, 변경된 값을 **그대로** 쓴다. 이전 값을 읽어 같으면 건너뛰는
//!   코드는 어디에도 없다(N4). 백엔드 쓰기 메서드도 DOM의 현재 값을 읽지 않는다.
//! - 유일한 읽기 연산은 resume 전용 `DomOps::adopt_element_children` 하나이고, 이것을 소비하는 codegen은 N1 판정례
//!   후보가 판정될 때까지 켜지 않는다(설계 §2.4).
//!
//! # 설계 정본과 다른 점 (기록)
//! 설계안(`design-7c`)은 `SpawnSpec`/`SpawnRowSpec`이 `DomTarget` 대신 `ElementHandle`을 붙이게 했다. 구현에서
//! **`DomTarget`을 붙인다**: (1) 기존 `SpawnElement`/`SpawnRow`와 같은 계약이라 `WriteParent::Entity` 해석,
//! `Reposition`/`RemoveNode` 앵커, `patch_text_system`(엘리먼트 자체가 텍스트 binding인 경우)이 그대로 작동한다 —
//! `ElementHandle`로는 7b가 리스트 row를 `SpawnRowSpec`으로 옮길 수 없다. (2) 설계가 `ElementHandle`로 막으려던 교차
//! 매칭(`patch_text_system::<String>`이 속성 binding에 `SetText`를 내는 것)은 **속성/텍스트노드 binding을 엘리먼트
//! entity와 분리된 별도 entity에 두고 `AttrTarget`/`TextNodeTarget`만 붙이는 규약**으로 같게 막힌다 — 그 entity에는
//! `DomTarget`이 없으므로 `patch_text_system`의 쿼리가 원천적으로 맞지 않는다(테스트 5).

use bevy_ecs::prelude::*;

use crate::{DomOps, DomTarget, DomWrite, LoomApp, NodeHandle, PendingDomWrites, Signal, WriteParent};

/// 엘리먼트 네임스페이스. `DomOps::create_element_ns`가 받는다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Namespace {
    Html = 0,
    Svg = 1,
    MathMl = 2,
}

impl Namespace {
    pub const fn uri(self) -> &'static str {
        match self {
            Namespace::Html => "http://www.w3.org/1999/xhtml",
            Namespace::Svg => "http://www.w3.org/2000/svg",
            Namespace::MathMl => "http://www.w3.org/1998/Math/MathML",
        }
    }
}

/// **반사(reflect)되지 않는** dirty-state 프로퍼티만. `disabled`/`hidden`/`required` 등 속성이 프로퍼티로 반사되는
/// 것은 `SetAttr`/`RemoveAttr`로 충분하다. 이름은 닫힌 열거형에서만 나오므로 임의 문자열 프로퍼티 쓰기가 아니다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BoolProp {
    Checked = 0,
    Selected = 1,
    Indeterminate = 2,
}

impl BoolProp {
    pub const fn js_name(self) -> &'static str {
        match self {
            BoolProp::Checked => "checked",
            BoolProp::Selected => "selected",
            BoolProp::Indeterminate => "indeterminate",
        }
    }
}

/// 컴파일러가 `static`으로 생성하는 정적 생성 지시. 위 모듈 문서의 No-VDOM 항 참고.
#[derive(Debug)]
pub struct ElementSpec {
    pub ns: Namespace,
    pub tag: &'static str,
    pub class: Option<&'static str>,
    pub loom_id: Option<&'static str>,
    pub attrs: &'static [(&'static str, &'static str)],
}

/// `DomWrite::Surface`의 내용. 전부 POD(클로저·함수 포인터 없음, CLAUDE.md 금지 4).
#[derive(Debug)]
pub enum SurfaceWrite {
    /// 엘리먼트 하나를 `spec`대로 만들어 `parent` 아래 **끝**에 붙이고 `DomTarget`을 단다(`SpawnElement`의 상위 호환).
    /// 초기 텍스트는 `append_static_text`로 넣는다(`set_text_content`는 자식을 읽고 분기하므로 쓰지 않는다).
    SpawnSpec { entity: Entity, parent: WriteParent, spec: &'static ElementSpec, text: Option<String> },
    /// `SpawnRow`의 상위 호환 — row 엘리먼트를 `spec`대로 만든다. `before`가 `Some`인데 해석되지 않으면
    /// **끝에 붙이되 `SurfaceStats.anchor_misses`를 올린다**(기존 `SpawnRow`는 조용히 append — R34).
    SpawnRowSpec {
        entity: Entity,
        parent: NodeHandle,
        spec: &'static ElementSpec,
        text: Option<String>,
        before: Option<Entity>,
    },
    /// `binding` entity에 `AttrTarget(element의 핸들)`을 단다. 지연 생성된 엘리먼트의 핸들은 같은 배치 안에서만
    /// 확정되므로(`SpawnElement`의 초기값 논거와 같다) 생성과 같은 배치에서 방출한다.
    BindAttrTarget { binding: Entity, element: WriteParent },
    SetAttr { node: WriteParent, name: &'static str, value: String },
    RemoveAttr { node: WriteParent, name: &'static str },
    ToggleClass { node: WriteParent, class: &'static str, on: bool },
    SetBoolProp { node: WriteParent, prop: BoolProp, value: bool },
    /// `parent`의 **자식 텍스트 노드**를 하나 만들고 `entity`에 `TextNodeTarget`을 단다(혼합 텍스트 `Count: {n()}`).
    SpawnText { entity: Entity, parent: WriteParent, text: String },
    SetTextData { node: NodeHandle, text: String },
    /// `style.setProperty(prop, value)` / `value == None`이면 `removeProperty(prop)`.
    SetStyle { node: WriteParent, prop: &'static str, value: Option<String> },
}

/// 새 경로에서만 세는 진단 카운터. 지연 생성(`apply_surface` 안) — `LoomApp` 필드는 바꾸지 않는다.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SurfaceStats {
    /// `SpawnRowSpec`의 `before`가 `Some`이었는데 해석되지 않아 끝에 붙인 횟수.
    pub anchor_misses: u32,
}

/// 속성 binding entity가 가리키는 **속성 노드(엘리먼트)**. `DomTarget`과 의도적으로 분리 — 모듈 문서 참고.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttrTarget(pub NodeHandle);

/// 혼합 텍스트의 **형제 텍스트 노드**. `DomTarget`과 의도적으로 분리 — `SetText`가 Text 노드에 가면
/// `insert_before`가 실패한다.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextNodeTarget(pub NodeHandle);

// ---------------------------------------------------------------------------
// 사이트 마커 — 같은 `T`를 쓰는 사이트가 여럿이어도 서로의 binding을 건드리지 않게 한다.
// 마커는 binding entity에 붙는 ZST Component이고 쿼리는 `With<A>`로 거른다. 이름은 연관 상수라 Component에
// 값으로 저장되지 않는다(금지 4).
// ---------------------------------------------------------------------------

pub trait AttrSite: Component {
    const NAME: &'static str;
}
pub trait ClassSite: Component {
    const CLASS: &'static str;
}
pub trait BoolPropSite: Component {
    const PROP: BoolProp;
}
pub trait StyleSite: Component {
    const PROP: &'static str;
}

/// 속성 하나의 값 binding. 값은 `Changed`일 때만, **그대로** 쓴다.
pub fn patch_attr_system<T, A>(
    q: Query<(&Signal<T>, &AttrTarget), (With<A>, Changed<Signal<T>>)>,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + ToString + 'static,
    A: AttrSite,
{
    for (signal, target) in &q {
        pending.0.push(DomWrite::Surface(SurfaceWrite::SetAttr {
            node: WriteParent::Node(target.0),
            name: A::NAME,
            value: signal.0.to_string(),
        }));
    }
}

/// `Some`이면 `SetAttr`, `None`이면 `RemoveAttr`.
pub fn patch_opt_attr_system<T, A>(
    q: Query<(&Signal<Option<T>>, &AttrTarget), (With<A>, Changed<Signal<Option<T>>>)>,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + ToString + 'static,
    A: AttrSite,
{
    for (signal, target) in &q {
        let node = WriteParent::Node(target.0);
        pending.0.push(DomWrite::Surface(match &signal.0 {
            Some(v) => SurfaceWrite::SetAttr { node, name: A::NAME, value: v.to_string() },
            None => SurfaceWrite::RemoveAttr { node, name: A::NAME },
        }));
    }
}

pub fn patch_class_toggle_system<C>(
    q: Query<(&Signal<bool>, &AttrTarget), (With<C>, Changed<Signal<bool>>)>,
    mut pending: ResMut<PendingDomWrites>,
) where
    C: ClassSite,
{
    for (signal, target) in &q {
        pending.0.push(DomWrite::Surface(SurfaceWrite::ToggleClass {
            node: WriteParent::Node(target.0),
            class: C::CLASS,
            on: signal.0,
        }));
    }
}

pub fn patch_bool_prop_system<P>(
    q: Query<(&Signal<bool>, &AttrTarget), (With<P>, Changed<Signal<bool>>)>,
    mut pending: ResMut<PendingDomWrites>,
) where
    P: BoolPropSite,
{
    for (signal, target) in &q {
        pending.0.push(DomWrite::Surface(SurfaceWrite::SetBoolProp {
            node: WriteParent::Node(target.0),
            prop: P::PROP,
            value: signal.0,
        }));
    }
}

pub fn patch_style_system<S>(
    q: Query<(&Signal<Option<String>>, &AttrTarget), (With<S>, Changed<Signal<Option<String>>>)>,
    mut pending: ResMut<PendingDomWrites>,
) where
    S: StyleSite,
{
    for (signal, target) in &q {
        pending.0.push(DomWrite::Surface(SurfaceWrite::SetStyle {
            node: WriteParent::Node(target.0),
            prop: S::PROP,
            value: signal.0.clone(),
        }));
    }
}

/// 혼합 텍스트의 동적 조각 — 형제 텍스트 노드의 `data`만 쓴다(정적 조각과 엘리먼트 자식은 건드리지 않는다).
pub fn patch_text_node_system<T>(
    q: Query<(&Signal<T>, &TextNodeTarget), Changed<Signal<T>>>,
    mut pending: ResMut<PendingDomWrites>,
) where
    T: Send + Sync + ToString + 'static,
{
    for (signal, target) in &q {
        pending.0.push(DomWrite::Surface(SurfaceWrite::SetTextData {
            node: target.0,
            text: signal.0.to_string(),
        }));
    }
}

/// `LoomApp::tick`의 `Surface` arm 본문. 단형화 폭주를 막고 기존 `tick` 본문을 키우지 않으려고 분리한다.
///
/// **호출 금지**(구조 단언 대상): `get_input_value`, `adopt_children`, `adopt_element_children` — 이 경로는 DOM을
/// 읽지 않는다.
#[inline(never)]
pub(crate) fn apply_surface<D: DomOps>(app: &mut LoomApp, dom: &mut D, write: SurfaceWrite) {
    match write {
        SurfaceWrite::SpawnSpec { entity, parent, spec, text } => {
            let Some(parent_handle) = app.resolve_parent(parent) else {
                app.dropped_writes += 1;
                return;
            };
            let handle = create_from_spec(dom, parent_handle, spec);
            if let Some(text) = text {
                dom.append_static_text(handle, &text);
            }
            app.world.entity_mut(entity).insert(DomTarget(handle));
        }
        SurfaceWrite::SpawnRowSpec { entity, parent, spec, text, before } => {
            let handle = create_from_spec(dom, parent, spec);
            if let Some(text) = text {
                dom.append_static_text(handle, &text);
            }
            if let Some(anchor) = before {
                match app.dom_target(anchor) {
                    Some(before_handle) => dom.reposition(parent, handle, Some(before_handle)),
                    None => {
                        // R34를 새 경로에서 바로잡는다: 앵커를 못 찾으면 조용히 append하되 센다.
                        app.world.get_resource_or_insert_with(SurfaceStats::default).anchor_misses += 1;
                    }
                }
            }
            app.world.entity_mut(entity).insert(DomTarget(handle));
        }
        SurfaceWrite::BindAttrTarget { binding, element } => {
            let Some(handle) = app.resolve_parent(element) else {
                app.dropped_writes += 1;
                return;
            };
            match app.world.get_entity_mut(binding) {
                Ok(mut e) => {
                    e.insert(AttrTarget(handle));
                }
                Err(_) => app.dropped_writes += 1,
            }
        }
        SurfaceWrite::SetAttr { node, name, value } => match app.resolve_parent(node) {
            Some(h) => dom.set_attribute(h, name, &value),
            None => app.dropped_writes += 1,
        },
        SurfaceWrite::RemoveAttr { node, name } => match app.resolve_parent(node) {
            Some(h) => dom.remove_attribute(h, name),
            None => app.dropped_writes += 1,
        },
        SurfaceWrite::ToggleClass { node, class, on } => match app.resolve_parent(node) {
            Some(h) => dom.toggle_class(h, class, on),
            None => app.dropped_writes += 1,
        },
        SurfaceWrite::SetBoolProp { node, prop, value } => match app.resolve_parent(node) {
            Some(h) => dom.set_bool_prop(h, prop, value),
            None => app.dropped_writes += 1,
        },
        SurfaceWrite::SetStyle { node, prop, value } => match app.resolve_parent(node) {
            Some(h) => dom.set_style(h, prop, value.as_deref()),
            None => app.dropped_writes += 1,
        },
        SurfaceWrite::SpawnText { entity, parent, text } => {
            let Some(parent_handle) = app.resolve_parent(parent) else {
                app.dropped_writes += 1;
                return;
            };
            let handle = dom.create_text_node(parent_handle, &text);
            app.world.entity_mut(entity).insert(TextNodeTarget(handle));
        }
        SurfaceWrite::SetTextData { node, text } => dom.set_text_data(node, &text),
    }
}

/// `spec`대로 엘리먼트를 만든다: 네임스페이스·태그·class, 그 뒤 `data-loom-id`와 정적 속성들. 모두 쓰기뿐이다.
fn create_from_spec<D: DomOps>(dom: &mut D, parent: NodeHandle, spec: &'static ElementSpec) -> NodeHandle {
    let handle = dom.create_element_ns(parent, spec.ns, spec.tag, spec.class);
    if let Some(id) = spec.loom_id {
        dom.set_attribute(handle, "data-loom-id", id);
    }
    for (name, value) in spec.attrs {
        dom.set_attribute(handle, name, value);
    }
    handle
}
