# Phase 7c 설계 원문 (보존본, 2026-10-26 저장소에 옮김)

> **지위**: 2026-10-17 워크플로(설계 → 적대 검토 → 개정)의 산출 원문이다. 구현(`crates/loom-runtime/src/surface.rs`, 커밋 2779566)과 다른 점은
> 한 가지다 — `SpawnSpec`/`SpawnRowSpec`은 `ElementHandle`이 아니라 **`DomTarget`을 붙인다**(이유: `surface.rs` 모듈 문서의 "설계 정본과 다른 점").
> 이 파일을 저장소에 둔 이유: 설계 원문이 스크래치 디렉터리(`/tmp`)에만 있었고 날짜가 바뀐 뒤 사라졌다(전사본에서 복원). 원문은 고치지 않는다.
> 구현 결과와 행별 갱신은 docs/09 "2026-10-26 갱신(7c)", 사전 등재와 적용 기록은 RQ1 프로토콜 "개정 #12 후속 1"이다.

---

# Phase 7c 최소 런타임 표면 설계안 (개정판, 읽기 전용 검토, HEAD da4a9bc 기준)

이 설계안은 아직 확정되지 않았습니다. 아래 사실을 먼저 밝혀 둡니다.
- 크기 실측은 저장소 밖 scratch 크레이트에서 했습니다.
- 판별값 인코딩은 x86_64에서만 확인했습니다. 확인한 것은 검토자입니다.
- 브라우저 동작은 대부분 추론이고, 프로브로 확인해야 합니다(§4).
- `adopt_element_children`과 N1의 관계는 판정이 나지 않았고, 판정례 후보로 올립니다(§2.4).

## 요약

**구조**
- `DomWrite`에는 **맨 끝에 variant 하나**(`Surface(SurfaceWrite)`)만 붙입니다.
- 내용은 새 모듈 `crates/loom-runtime/src/surface.rs`에 모읍니다. 여기에 들어가는 것은 별도 열거형, `#[inline(never)]` 적용 함수, 사이트 마커, 카운터입니다.
- lib.rs에서 바뀌는 곳은 네 군데뿐입니다.
  - `tick`의 arm 1개
  - 파일 끝의 `mod surface; pub use surface::*;`
  - `DomOps` trait의 새 기본 메서드. 메서드는 trait 블록 안에 둘 수밖에 없어 그 뒤 줄이 밀립니다.
  - doc 주석 정정

**바꾸지 않는 것**
- 기존 variant 12종과 `tick`의 기존 arm
- `DefaultRow`, `set_text_content`, `set_visible`, `adopt_children`
- `DomOpCounts`, `total_mutations`, `mutation_observer_channel`, `to_json`. 이 넷은 **한 바이트도 바꾸지 않습니다.** 그래서 M2의 정의가 그대로입니다.

**`SpawnRow`에 class를 넣는 것은 기존 variant를 고치는 일**입니다. 그래서 새 variant(`SpawnRowSpec`)로 분리합니다.

**7c의 위치**
- 7c 경로는 **7b 생성 코드 전용**입니다. RQ1·RQ6에서 측정하는 arm은 v23에서 이 경로를 쓰지 않습니다.
- 그래서 7c는 v23 기반에 "들어가 있지만 실행되지 않는 코드"입니다. 이 점은 §5에서 구조적으로 확인합니다.

**크기 실측**
- 방법: 실제 정의를 복제한 scratch 크레이트에서 `cargo check`로 const-eval을 돌렸습니다.
- 결과:
  - 복제본 `DomWrite`: wasm32 56B / x86_64 88B. 실물과 같습니다.
  - `Surface`를 추가한 뒤에도 56/88입니다.
  - `SurfaceWrite`는 40/56입니다.
- 판별값: x86_64에서 niche 인코딩입니다. dataful variant는 `SpawnElement`이고, 태그는 offset 0에 있는 String capacity의 niche에 들어갑니다. 예: SetText=0x8000…00, ClearChildren=…01, RemoveNode=…04, SetVisible=…05, SetInputValue=…0a.
  - `Surface`를 **맨 끝**에 붙이면 이 값들이 그대로 유지됩니다.
  - wasm32에서는 확인하지 않았습니다. §5의 함수 diff로 확인합니다.

## 0. 먼저 알아야 할 발견 3건

**1. 프로토콜 재검토 기록 중 "`loom-bench`는 `set_visible`을 쓰지 않는다"(research-questions/RQ1-benchmark-protocol.md:2312)는 틀렸습니다.**

이 서술의 영향 범위는 bench보다 넓습니다.
- **bench**: 트랙 T의 `"F"` arm이 `cond_reconcile_f_system`을 등록합니다(crates/loom-bench/src/lib.rs:2641-2642, 구 B/F 벤치는 :2410). 이 System이 `SetVisible`을 내고(cond.rs:175), `tick` arm(lib.rs:1464)을 거쳐 `BrowserDom::set_visible`(web.rs:398-403)이 실행됩니다.
- **컴파일러 생성 조건부**: else 분기에서 `SetVisible`을 냅니다(crates/loom-compiler/src/codegen.rs:2714, :3265). 트랙 N과 RQ6 픽스처가 이 경로를 탑니다.
- **손코딩 Island**: crates/loom-island-demo/src/lib.rs:158, crates/loom-island-productcard/src/lib.rs:42.

그래서 R36을 web.rs에서 고치면 트랙 T, 트랙 N, RQ6의 측정 경로가 **모두** 바뀝니다. `adopt_children` 쪽 서술은 맞습니다. bench에서는 호출이 0건입니다.

은닉 수단의 계약은 이미 있습니다. 프로토콜 §10:381이 "은닉 수단 사전 선언: inline style `display:none`"이라고 정해 두었습니다(docs/09:153 R36 행에도 "조건부B/CSS 은닉 arm"으로 적혀 있음). 새 계약을 만들지 않고 이 조항을 인용합니다.

**2. R34는 기존 arm을 고치지 않으면 해결되지 않습니다.**
- `SpawnRow`(lib.rs:1446)와 `Reposition`(:1460)은 앵커 해석에 실패하면 조용히 append합니다.
- 이것을 집계하려면 기존 arm에 분기를 넣어야 합니다. 이는 "추가만" 제약에 어긋나므로 §3에서 미룹니다.

**3. 혼합 텍스트의 resume는 런타임 원시 연산만으로 풀리지 않습니다.**
- SSR된 `Count: 5`는 HTML 파서가 텍스트 노드 하나로 합칩니다.
- 그래서 SSR 쪽 마커 규약(주석 구분자 등)이 먼저 정해져야 adopt 원시 연산을 설계할 수 있습니다.
- 클라이언트에서 새로 만드는 경우는 문제가 없습니다.

## 1. variant 목록 (전부 POD, 클로저·함수 포인터 없음)

```rust
// crates/loom-runtime/src/surface.rs (신규 모듈)
#[derive(Clone, Copy, Debug, PartialEq, Eq)] #[repr(u8)]
pub enum Namespace { Html = 0, Svg = 1, MathMl = 2 }

/// 반사(reflect)되지 않는 dirty-state 프로퍼티만. disabled/hidden/required 등은 SetAttr/RemoveAttr.
#[derive(Clone, Copy, Debug, PartialEq, Eq)] #[repr(u8)]
pub enum BoolProp { Checked = 0, Selected = 1, Indeterminate = 2 }

/// 컴파일러가 `static`으로 생성하는 정적 생성 지시.
/// 무저촉 근거는 PartialEq 부재가 아니라 다음 두 가지다:
/// (1) 갱신마다 생성되지 않음 (2) 어떤 비교·조정 알고리즘에도 입력되지 않음 — §4 구조 단언으로 강제.
#[derive(Debug)]
pub struct ElementSpec {
    pub ns: Namespace,
    pub tag: &'static str,
    pub class: Option<&'static str>,
    pub loom_id: Option<&'static str>,
    pub attrs: &'static [(&'static str, &'static str)],
}

#[derive(Debug)]
pub enum SurfaceWrite {
    SpawnSpec      { entity: Entity, parent: WriteParent, spec: &'static ElementSpec, text: Option<String> },
    SpawnRowSpec   { entity: Entity, parent: NodeHandle, spec: &'static ElementSpec, text: Option<String>, before: Option<Entity> },
    BindAttrTarget { binding: Entity, element: WriteParent },
    SetAttr        { node: WriteParent, name: &'static str, value: String },
    RemoveAttr     { node: WriteParent, name: &'static str },
    ToggleClass    { node: WriteParent, class: &'static str, on: bool },
    SetBoolProp    { node: WriteParent, prop: BoolProp, value: bool },
    SpawnText      { entity: Entity, parent: WriteParent, text: String },
    SetTextData    { node: NodeHandle, text: String },
    SetStyle       { node: WriteParent, prop: &'static str, value: Option<String> }, // 선택(결정 사항 3)
}
// lib.rs: DomWrite의 **마지막** variant로 Surface(SurfaceWrite).
// doc에 "Surface는 반드시 마지막 variant. 앞에 끼우면 기존 생성 지점(lib.rs:733, cond.rs:108/:163/:278)의 판별값 코드가 바뀐다"를 명시.
```

### `SpawnSpec`/`SpawnRowSpec`의 계약

- 이 둘로 만든 엘리먼트 entity에는 **`DomTarget`을 붙이지 않습니다.** 대신 `ElementHandle(NodeHandle)`을 붙입니다(surface.rs 신규 Component).
  - 이유: `DomTarget`과 `Signal<String>`이 같은 entity에 있으면 `patch_text_system::<String>`(lib.rs:965-977)이 그 entity에 개입합니다.
- 기존 `SpawnElement`(lib.rs:347-354, tag/class/loom_id/text)와는 겹칩니다. 다만 `SpawnElement`는 계속 HTML 네임스페이스로만 엘리먼트를 만듭니다.
  - 따라서 **7b에서 생성 코드를 `SpawnSpec`으로 옮기기 전까지 R43(SVG 오컴파일)은 그대로 남습니다.** docs/09 R43 행에도 이렇게 적습니다.

### 새 Component (`DomTarget`을 재사용하지 않음)

`AttrTarget(NodeHandle)`, `TextNodeTarget(NodeHandle)`, `ElementHandle(NodeHandle)`을 새로 둡니다.

- **분리하는 이유**: `patch_text_system::<T>`(lib.rs:965-977)에는 마커 필터가 없습니다.
  - `Signal<String>`과 `DomTarget`을 가진 속성 binding이 있으면 교차 매칭으로 `SetText`가 나갑니다.
  - 그 `SetText`가 텍스트 노드 핸들에 가면 web.rs:367의 `insert_before`가 Text 노드에서 실패해 패닉합니다.
  - 기존 System을 고치지 않고 이 문제를 피하려면 타깃 Component를 분리하는 수밖에 없습니다.
- **수명**: binding entity는 반드시 `ChildOf(소유 엘리먼트 또는 row entity)`로 매답니다.
  - 근거: lib.rs:757-760의 RowBinding 선례입니다. `Children`이 `linked_spawn`이라 소유자를 despawn하면 binding도 함께 사라집니다.
  - 이것이 없으면 위험합니다. `release_subtree`가 `alloc_under`로 매단 텍스트 노드 슬롯을 회수합니다(web.rs:236 이하 `alloc_under`, free list 재사용). 그 뒤 binding이 남아 있으면 오래된 핸들로 다른 노드에 쓰거나, "해제된 NodeHandle" 패닉(web.rs:228)이 납니다.
  - 이를 확인하는 테스트는 §4의 7번입니다.

### 새 generic patch System

docs/02 PatchTable 결정의 정본인 "제네릭 System + variant 추가" 형태를 그대로 따릅니다.

- `patch_attr_system<T, A: AttrSite>`
- `patch_opt_attr_system<T, A>`: `Some`이면 SetAttr, `None`이면 RemoveAttr
- `patch_class_toggle_system<C: ClassToken>`
- `patch_bool_prop_system<P: BoolPropSite>`
- `patch_text_node_system<T>`

**사이트 마커**
- `#[derive(Component)]` ZST입니다. binding entity에 붙이고, 쿼리는 `With<A>`로 거릅니다.
  - 이렇게 해야 같은 `T`를 쓰는 속성 사이트가 둘(`patch_attr_system<String, A1>`과 `<String, A2>`)이어도 서로 교차 매칭되지 않습니다.
- 이름은 `const NAME/CLASS/PROP` 연관 상수로 둡니다. 값이 Component에 저장되지 않습니다(금지 4).

**게이팅**
- 모두 `Changed<Signal<_>>`로 게이팅합니다.
- 초기값과의 관계는 §1.1에서 다룹니다.

### `DomOps` 신규 메서드

전부 기본 구현을 `unimplemented!`로 둡니다. `detach_to_fragment`(lib.rs:270)의 관례를 따른 것입니다.

- `create_element_ns(parent, ns: Namespace, tag: &str, class: Option<&str>) -> NodeHandle`
  - `create_element`로 조용히 폴백하는 기본 구현은 두지 않습니다. 그러면 R43과 같은 종류의 조용한 결함을 다시 만듭니다.
- `remove_attribute(node, name)`
- `toggle_class(node, class, on)`
- `set_bool_prop(node, prop, value)`
- `create_text_node(parent, text) -> NodeHandle`
- `set_text_data(node, text)`
- `adopt_element_children(parent) -> Vec<(NodeHandle, String)>`: R35를 새 메서드로 처리하고, 기존 메서드는 그대로 둡니다.
- (선택) `set_style(node, prop, Option<&str>)`
- 속성 쓰기는 기존 `set_attribute`(lib.rs:285)를 재사용합니다. 그 doc의 "no DomWrite variant emits this"(lib.rs:278-284)는 정정합니다.

### `SpawnRow` 판정: 수정에 해당하므로 새 variant로 분리

근거는 넷입니다.
- (a) 필드를 추가하면 레이아웃이 바뀝니다. 그러면 K/C/D/E가 공유하는 생성 지점이 모두 다시 컴파일됩니다: `DefaultRow::emit`(lib.rs:733-739), cond.rs:108, :163, :278.
- (b) 공통 `tick` arm(lib.rs:1437-1450)이 바뀝니다.
- (c) 테스트 렌더러 세 곳이 필드를 구조분해합니다(row_spawner.rs:25, recorder_common.rs:30, recorder_axes.rs:46). 컴파일이 깨집니다.
- (d) codegen.rs:2868, :2946의 산출물이 바뀌어 트랙 N OUT_DIR의 바이트가 달라집니다.

R9의 **tag** 부분은 7b에서 런타임 변경 없이 풀립니다. `SpawnRow.tag`가 이미 있기 때문입니다(:2868이 `row_tag`를 넘김). 런타임이 새로 맡아야 하는 것은 class·ns·attrs뿐이고, `SpawnRowSpec`이 이것을 담당합니다.

### 1.1 초기값 규약과 프로토콜 §2.2 (research-questions/RQ1-benchmark-protocol.md:57, :383)

§2.2(v1.1 주석)의 내용은 다음과 같습니다.
- 초기값은 스폰 번들에 포함하는 것이 최선형입니다.
- 초기값까지 후속 재패치하는 2-pass 스폰은 체크리스트 미달입니다.

7c는 이 조항을 다음과 같이 맞춥니다.

**경로 구분**
- 정적 속성과 정적 class는 `ElementSpec`에 담깁니다. 따라서 스폰 write 하나로 들어갑니다.
- 동적 속성의 초기값은 같은 배치 안에서, 스폰 write 바로 뒤에 `BindAttrTarget` + `SetAttr { node: WriteParent::Entity(el) }`로 굽습니다.
  - 이것은 "스폰 write와 분리된 write"입니다. §2.2 기준으로 최선형과 같은지는 **판정하지 않습니다.**
  - 대신 §11 등재문에 이렇게 적습니다. "7c 경로는 v23의 RQ1 측정 arm이 쓰지 않는 7b 전용 경로이고, §2.2와 비교한 유불리는 7b 산출물을 측정 arm에 넣기 전에 다시 심사한다."

**`BindAttrTarget`이 필요한 이유**
- 지연 생성된 엘리먼트의 핸들은 같은 배치 안에서만 확정됩니다.
- 그 전에 지나간 `Changed`는 관측할 수 없습니다(lib.rs:342-346과 같은 사유).

**알려진 위험: 다음 tick의 재패치**
- row 스코프 binding처럼 Commands로 같은 tick에 spawn된 binding은 `Signal`의 Added 틱이 patch System의 `last_run`보다 뒤입니다.
- 그러면 **다음 tick에 `Changed`가 참이 되어 `SetAttr`가 한 번 더** 나갈 수 있습니다. 이것이 곧 "초기값 후속 재패치"입니다.
- §4의 6번 테스트로 확인합니다. 실패하면 7c를 동결하기 전에 설계를 바꿉니다. 대안은 둘입니다.
  - (i) 초기값은 patch System의 Added 후속 스테이지(§2.2 v0, 전 후보 공통) **한 곳**에서만 냅니다. 스포너는 초기값을 내지 않습니다.
  - (ii) spawn 쪽에서 `Signal`의 change tick을 patch System이 이미 본 것으로 표시합니다.
- 어느 쪽을 택할지는 테스트 결과를 보고 병익님이 정합니다.

### 어느 컴파일러 기능이 무엇을 쓰는가

| 변형 / 메서드 | 소비 기능(슬라이스) | docs/09 |
|---|---|---|
| SpawnSpec, `create_element_ns` | 지연 서브트리의 SVG, 지연 경로의 정적 속성(7b) | R43, §2.3 정적 속성 |
| SpawnRowSpec | row의 class/ns/attrs(7b) | R9 |
| BindAttrTarget, SetAttr, RemoveAttr | 동적 attr, Option attr, 반사 불리언 속성(disabled 등)(7b/7d) | §2.3 동적 attr, `disabled` |
| ToggleClass | `class:` 토글, 삼항 class(7b) | §2.3 |
| SetBoolProp | checkbox `checked`, `<select>`의 `selected`(7b/7d) | §2.8 |
| SpawnText, SetTextData, `create_text_node`, `set_text_data` | 혼합 텍스트 `Count: {n()}`(7b), Str/Bool 텍스트 | §2.4 |
| `adopt_element_children` | resume 경로(7b 이후 codegen이 교체) | R35(부분, §2.4 참고) |
| SetStyle(선택) | `style` 프로퍼티 바인딩(7d) | §2.3 |

## 2. 적용부와 web.rs 스케치, No-VDOM 점검

### 2.1 lib.rs와 surface.rs

**`tick`**
- arm 하나만 추가합니다: `DomWrite::Surface(w) => surface::apply_surface(self, dom, w)`.

**`apply_surface`** (`#[inline(never)]`, surface.rs)
- 부모 해석에 실패하면 기존 `dropped_writes`를 올립니다. 기존 필드를 쓰기만 하므로 기존 코드는 바뀌지 않습니다.
- `SpawnRowSpec`의 `before`가 해석되지 않으면 **append하면서 `SurfaceStats.anchor_misses`를 올립니다.** 새 경로에서는 R34를 바로잡는 것입니다.
- `SurfaceStats`는 `apply_surface` 안에서 지연 생성하는 Resource입니다. `LoomApp` 필드나 `LoomApp::new`는 바꾸지 않습니다.

**초기 텍스트 처리**
- `SpawnSpec`/`SpawnRowSpec`의 초기 텍스트는 `set_text_content`가 아니라 **`append_static_text`**로 넣습니다.
- `set_text_content`(web.rs:356-368)는 자식을 읽고 분기합니다. 판정례 9(가)의 단서("opcode 구현부가 노드 내용을 읽어 분기")와 G0 판정례 후보 (f)에 걸리는 지점입니다. 새 경로는 DOM을 읽지 않으므로 이 문제를 피합니다.

**호출 금지**
- `apply_surface`는 `get_input_value`, `adopt_children`, `adopt_element_children`을 호출하지 않습니다(§4 구조 단언).

### 2.2 web.rs (`impl DomOps for BrowserDom` 블록 끝, :501 뒤)

trait impl은 한 블록이어야 하므로 블록 안의 끝에 둡니다.

| 메서드 | 구현 |
|---|---|
| `create_element_ns` | `document.create_element_ns(Some(ns.uri()), tag)`. class는 Html이면 `set_class_name`, 아니면 `set_attribute("class", c)`. SVG의 `className`은 SVGAnimatedString이라 쓰기가 반영되지 않기 때문입니다. 그 뒤 `append_child`, `alloc_under(.., Some(parent))`. |
| `remove_attribute` | `el.remove_attribute(name)` |
| `toggle_class` | `el.class_list().toggle_with_force(class, on)`. 필요한 web-sys feature는 이미 켜져 있습니다(crates/loom-runtime/Cargo.toml:17). |
| `set_bool_prop` | `js_sys::Reflect::set(&el, prop.js_name(), &on.into())`. 이름은 닫힌 열거형에서만 나옵니다. `move_before`(web.rs:46-56)와 같은 방식입니다. |
| `create_text_node` | `document.create_text_node(text)`, `append_child`, `alloc_under(text.into(), Some(parent))`. `clear_children`/`release_subtree`가 회수합니다. 회수된 뒤의 binding 처리는 §1의 "수명"과 같습니다. |
| `set_text_data` | `unchecked_ref::<CharacterData>().set_data(text)` |
| `adopt_element_children` | `parent.children()`을 순회합니다. HTMLCollection은 Element만 담으므로 공백·주석 노드가 원리적으로 빠집니다. 텍스트는 `text_content()`로 읽습니다. 한계는 §2.4에 적습니다. |

**금지**
- 쓰기 메서드에는 **현재 값을 읽어 같으면 건너뛰는 코드**(`get_attribute`, `classList.contains`, `text_content()`)를 넣지 않습니다. 넣으면 N1(출력 재독)과 N4(변경 판별 대체)에 걸립니다.
- `adopt_element_children`만 예외적으로 DOM을 읽습니다(§4의 허용 목록).

**추론으로 남는 것**
- `<textarea>`/`<select>`의 value는 기존 `set_input_value`(web.rs:448-454)가 덕타이핑으로 처리할 것으로 봅니다. wasm-bindgen 글루가 `.value =`로 대입하기 때문입니다.
- 이것은 **추론**이고 브라우저 프로브로 확인해야 합니다.

### 2.3 counting.rs (`DomOpCounts`는 바꾸지 않음)

**바꾸지 않는 것**
- 새 카운터는 `DomOpCounts`(counting.rs:32-49)에 붙이지 않습니다. 별도 구조체 `SurfaceOpCounts`로 둡니다.
- 그래서 `total_mutations`(:55, §9 E 기각 (i)의 "셀당 총 DOM op 수"), `mutation_observer_channel`(:74), `to_json`(:91), `counts()`(:140)의 정의와 본문이 그대로입니다.
- 측정 정의도 바뀌지 않으므로 §11에서 측정 정의를 개정할 필요가 없습니다.

**바뀌는 것**
- `CountingDom`에 필드 `surface: SurfaceOpCounts`를 추가하고, 조회용 `surface_counts()`를 둡니다.
- `reset_counts()`(:147)는 이 필드도 함께 0으로 만들어야 하므로 **본문이 바뀝니다.** 호출 위치는 측정 괄호 밖입니다(bench lib.rs:923, 괄호는 :931 `start` ~ :934 `end`).
- `CountingDom`의 크기가 바뀌므로, 이를 포함하는 `Candidate` 단형화의 필드 오프셋이 달라질 수 있습니다(§5에서 허용 변경으로 처리).

**위임**
- 새 메서드는 **전부 명시적으로 위임하고 카운트**합니다. 2026-08-03 F-i 트랩(counting.rs:208-212)이 바로 이 누락에서 생겼습니다.

**채널 매핑**
- 매핑은 `SurfaceOpCounts::observer_channel`에 둡니다.

| 메서드 | 채널 |
|---|---|
| `create_element_ns`, `create_text_node` | childList |
| `remove_attribute`, `toggle_class` | attributes |
| `set_text_data` | characterData |
| `set_bool_prop` | unobservable(property) |
| `adopt_element_children` | **읽기 전용** 그룹. 기존 `adopt_children`(counting.rs:45-47)과 같고, 변이 합계에서 뺍니다. |

- 주석으로 밝혀 둘 것: `toggle_with_force`가 이미 원하는 상태일 때, 없는 속성을 `remove_attribute`할 때는 MutationRecord가 생기지 않습니다. 그래서 **카운트가 관측값보다 클 수 있습니다**(moveBefore 주석과 같은 형식). `setAttribute`는 값이 같아도 항상 기록됩니다.

### 2.4 No-VDOM (N1–N4는 전역 적용, P1–P3은 관할 밖)

| 항목 | N1 | N2 | N3 | N4 | 라벨 |
|---|---|---|---|---|---|
| ElementSpec·SpawnSpec | 요소 기술자의 형상이지만 static이고 갱신마다 생성하지 않음. 비교·조정에 입력되지 않음(구조 단언으로 강제) | 재귀 없음 | 생성 엘리먼트당 O(attrs) | — | 대상외(무저촉) |
| SpawnRowSpec | 위와 같음 | 없음 | O(1) + reposition | — | insert 구현부(판정례 9(가)), 노드 내용 미열람 |
| SetAttr/RemoveAttr/ToggleClass/SetBoolProp/SetStyle | 값 출처가 Signal | 없음 | 변경 binding당 O(1) | `Changed` 게이팅, 값 비교 없음 | 대상외 |
| SpawnText/SetTextData | 값 출처가 Signal. 이전 값을 아예 쓰지 않음(판정례 8의 "이전 값 출처") | 없음 | O(1) | 게이팅 | 대상외 |
| `adopt_element_children` | **미판정, 판정례 후보로 등재**(아래) | 직계 자식만 | O(자식) | — | 보류 |

**`adopt_element_children`과 N1: 판정하지 않고 후보로 올림**
- 기존 list resume 생성 코드가 이미 이 형태입니다.
  - `adopt_children`이 읽은 텍스트로 row의 `Signal`과 리스트의 `Signal<Vec<String>>`을 시드합니다(crates/loom-compiler/src/codegen.rs:1607-1628).
  - 그 값이 다음 reconcile의 "이전 값"이 됩니다.
- 따라서 "읽은 값을 비교 기준으로 시드하면 N1 위반"이라는 단서는 기존 resume에도 그대로 적용됩니다.
- 어느 쪽이 맞는지 감으로 정하지 않습니다(CLAUDE.md 1). 다음 질문을 **docs/03 판정례 후보**로 등재합니다(RQ1 G0 기록의 후보 (a)~(h) 옆): "resume 시점의 SSR DOM 상태 복구가 N1의 '알고리즘 입력이 렌더 산출물'에 해당하는가."
- 판정이 나기 전까지 7c는 이 메서드를 **제공만** 합니다. 소비 codegen(7b)이 이 메서드를 쓰는 것은 판정 뒤로 미룹니다.

**`text_content()`의 한계**
- 자손 텍스트를 모두 이어 붙입니다. 그래서 `<li>v<span>…</span></li>`처럼 생긴 row는 값이 틀리게 복구됩니다.
- R35는 "공백 노드" 문제만 풀리고 "복합 row 텍스트" 문제는 남습니다. docs/09 R35 행에 "부분 해결"로 적습니다.

## 3. 지금 넣지 않을 것

**R36 (`set_visible`)**
- 트랙 T, 트랙 N(생성 조건부), RQ6(Island)의 측정 경로입니다(§0-1).
- 계약은 이미 §10:381에 있으므로 수리는 미룹니다.
- 프로토콜 :2312의 서술은 원문을 지우지 않고 정정 주석을 답니다(이력 보존).

**R34의 기존 arm 수리**
- 미룹니다. `#[cfg(debug_assertions)]`로 기존 arm에서만 집계하는 선택지가 있지만, 그 전제를 다음과 같이 정정합니다.
  - 트랙 N은 **release**로 돕니다(프로토콜 :1838 `cargo build --release -p loom-track-n`). 그래서 debug 전용 카운터는 트랙 N에서 작동하지 않습니다. 작동하는 곳은 `cargo test`(debug)로 도는 loom-nested-fixture뿐입니다.
  - `cfg`로 빠지는 코드라도 기존 arm 안에 넣으면 뒤쪽 줄 번호가 밀립니다. 그러면 panic Location 데이터가 바뀌므로 **release wasm의 바이트도 바뀝니다.**
  - 런타임 소스가 바뀌므로, 개정 #11의 트랙 N 조건("의존 크레이트 소스 무변경")에 따른 바이트 면제도 받을 수 없습니다.
- 결론: 얻는 것은 debug 테스트에서의 관측뿐인데, 바이트 변경 비용은 그대로 듭니다. 병익님 결정 사항입니다.

**혼합 텍스트의 resume 원시 연산**
- SSR 마커 규약을 정한 뒤에 넣습니다(§0-3).
- 7c 동결 전에 규약이 정해지지 않으면 다음 캠페인으로 넘어갑니다. 결정 지점으로 등재합니다.

**StringDom 4벌의 신규 메서드 구현, R37 중복 속성**
- 7b에서 StringDom을 통합할 때 합니다.
- StringDom은 `loom-runtime` 밖이라 L§2.5 동결과 무관합니다.
- SSR 초기값은 setup이 기존 `set_attribute`와 `create_element` class로 굽습니다.

**TreeDom(loom-nested-fixture), RecordingDom(loom-track-n)**
- 변경하지 않습니다. 기본 `unimplemented!`가 2차 tripwire 역할을 합니다.
- 1차 tripwire는 §4의 write-stream 단언입니다.

**범위 밖**
- R11 row 값 출처(codegen)
- 이벤트·preventDefault·ref/focus·타이머(7d)
- key/해시 매칭(G0 수리 쪽)
- `BoolProp` 확장(반사 속성은 SetAttr로 충분)

**SetStyle**
- 선택 사항입니다. 빼면 style 프로퍼티 바인딩을 추가할 때 캠페인을 한 번 더 해야 합니다.
- 비용이 작으니 포함을 권합니다.

## 4. 테스트

### 기존 골든 (재생성 금지)
- crates/loom-runtime/tests/write_identity.rs의 골든 4개가 무수정으로 통과해야 합니다.
- 렌더러에 `other =>` 폴백이 있어 새 variant가 있어도 컴파일됩니다(recorder_common.rs:51, row_spawner.rs:33).

### 신규 `tests/surface_writes.rs` (MockDom 포함)

1. 각 variant를 `tick`으로 적용한 뒤의 결과 상태를 확인합니다.
2. 같은 배치 안에서 `WriteParent::Entity`가 해석되는지, `BindAttrTarget`이 초기값을 굽는지 확인합니다.
3. 앵커 miss는 **존재 단언을 먼저** 둡니다. 의도적으로 틀린 앵커를 넣었을 때 `anchor_misses`가 1이 되는지 확인한 뒤에, 정상 경로에서 0인지 확인합니다.
4. 혼합 텍스트: `Count: ` + `5`가 `6`으로 바뀔 때 형제 정적 텍스트가 보존되는지 확인합니다.
5. 교차 매칭: 아래 두 등록 각각에서 상대 사이트로 가는 write가 0건인지 확인합니다. 그 전에 **자기 사이트로는 1건씩 나가는지 먼저 단언**합니다.
   - (a) `patch_text_system::<String>`과 `patch_attr_system`을 함께 등록
   - (b) **같은 `T`의 속성 사이트 둘**(`<String, A1>`, `<String, A2>`)을 등록
6. 게이팅과 §2.2 재패치 (§1.1):
   - (a) setup에서 spawn한 binding: 첫 tick에 정확히 1건, 다음 tick에 0건
   - (b) **Commands로 같은 tick에 spawn한 row 스코프 binding**: 스폰한 tick의 초기값 1건, **다음 tick에 0건**
   - (b)가 실패하면 동결 전에 §1.1의 대안 (i) 또는 (ii)로 설계를 바꿉니다.
7. 수명: binding을 `ChildOf`로 매단 row를 삭제합니다. 그 뒤 새 노드를 만들어 슬롯이 재사용되게 하고, 옛 binding의 Signal을 바꿉니다. 이때 패닉이 없고 다른 노드에 write가 0건이어야 합니다. 그 전에 **슬롯이 실제로 재사용됐다**는 존재 단언(같은 인덱스)을 둡니다.

### 그 밖

**위임 테스트**
- CountingDom의 새 메서드마다 `SurfaceOpCounts`가 +1 되는지와 inner에 효과가 가는지 확인합니다.
- `DomOpCounts`와 `to_json()` 출력이 바뀌지 않았는지 확인합니다(새 메서드를 호출하기 전과 후 비교).

**크기와 판별값 단언**
- native: `size_of::<DomWrite>() == 88`
- `#[cfg(target_arch="wasm32")]` const 단언: 56
- native에서 기존 12 variant의 판별값 고정 테스트: 대표 값을 `transmute_copy`로 읽어 위 niche 값과 같은지 확인합니다. wasm32는 §5 diff로 확인합니다.

**구조 단언** (판정례 9(다)의 선례, 소스 grep)
- `list.rs`, `cond.rs`, `loom-flush-core`가 `ElementSpec`/`spec`/`SurfaceWrite`를 참조하지 않는지 확인합니다.
- `apply_surface`가 `get_input_value`/`adopt*`를 호출하지 않는지 확인합니다.
- web.rs의 DOM 읽기 API는 **메서드별 허용 목록**으로 확인합니다.
  - `adopt_element_children`: `children`, `text_content` 허용
  - 나머지 신규 메서드: 읽기 API 0건
  - grep이 대상 메서드 본문을 **실제로 찾았는지**(본문 줄 수 > 0)를 먼저 단언합니다.

**tripwire**
- (1차, 핵심) 트랙 N 실행과 loom-nested-fixture의 write-stream 기록에서 `DomWrite::Surface`가 0건인지 확인합니다.
  - 같은 테스트 안에서 먼저 `Surface`를 1건 넣고, 검출기가 1을 세는지 확인합니다.
  - bench 쪽은 K/C/D/E/T 전 arm의 native write-stream에 같은 단언을 둡니다.
- (2차) TreeDom에서 신규 메서드를 부르면 `should_panic`인지 확인합니다. 이것은 기본 구현이 있다는 것만 보여 줍니다.

**브라우저 프로브** (docs/09 R43의 렌더 동작은 추론이었음)
- SVG `namespaceURI`, SVG class 적용
- 사용자가 클릭한 뒤의 `checked`
- `toggle_with_force` no-op일 때 MutationRecord가 생기지 않는지
- 공백이 끼어 재포맷된 HTML과 복합 row(`<li>v<span>..</span></li>`)에서 `adopt_element_children` 결과
- `<textarea>`/`<select>`에 `set_input_value`

## 5. 바이트 변화와 K/C 경로 동일성 검사

### 전제
- 이번 변경은 `loom-runtime` 소스를 바꿉니다. 그래서 **측정 산출물의 바이트가 반드시 바뀝니다.**
- 따라서 **개정 #11의 "바이트 동일이면 재실행·재인증 불필요" 면제를 쓰지 않습니다.**
- 아래 함수 diff는 면제 근거가 아닙니다. "K/C가 자연 대조군"이라는 **공평성 근거**일 뿐입니다.

### 기반 정의
- **B0**: G0 수리의 최종 커밋
- **B1**: B0 + 7c 커밋만
- **B2**: B1 + v23 하네스 변경(Firefox BiDi 재개, 트랙 L 하네스 등). B1→B2는 **따로 diff하고 따로 기록**합니다.
- v23 측정 산출물은 B2에서 만듭니다.

### (a) B0 점검: G0 수리가 공유 코드를 건드렸는가 (v22 기반 a55ba87 대 B0)
- 아래 (b)의 도구로 함수 단위 diff를 냅니다.
- 대상: K/C·cond·`DefaultRow::emit`(lib.rs:733-739)·`RowSpawnCtx`·`list.rs` 공통 헬퍼·`loom-flush-core` `plan_writes*`·`BrowserDom` 함수 본문
- 골든 `tests/lis_output_identity.rs`, `tests/write_identity.rs`가 무수정으로 통과해야 합니다.
- G0 수리가 HashMap을 쓴다면 RandomState 시드를 바꿔 돌리는 write-stream 결정성 테스트를 B0 조건에 넣습니다.
- N5 재심사 대상에는 **insert 경로**(`SpawnRow`인지 `SpawnRowSpec`인지)를 명시합니다. G0 후보 (f)의 지적대로 판정례 9(가)는 apply 경로를 보지 않습니다. 7b가 컴파일된 리스트를 `SpawnRowSpec`으로 옮기면 심사받은 산출물과 실제 산출물이 달라지기 때문입니다.
- **사전 선언 규칙**: 공유 코드가 바뀌었으면 K/C는 자연 대조군이 아닙니다. 그 경우 (c)의 드리프트 관찰 자체를 하지 않고, v23 단독 판정만 합니다.

### (b) 함수 단위 diff: B0 대 B1 (그리고 B1 대 B2를 따로)

**빌드 두 가지**
- 진단 빌드 ①: `cargo build -p loom-bench --target wasm32-unknown-unknown --release`. name section이 있는 wasm-opt 이전 산출물입니다.
- 진단 빌드 ②: `wasm-pack build --profiling crates/loom-bench`. wasm-opt를 거친 뒤에도 이름이 유지됩니다.
- 이유: crates/loom-bench/Cargo.toml에 `wasm-opt = false` 메타데이터가 없어, 측정에 쓰는 `wasm-pack build --release`는 wasm-opt를 돌립니다. wasm-opt의 인라이닝과 중복 함수 제거는 모듈 전체를 보고 결정됩니다. 워크스페이스 프로파일도 `lto=true`, `opt-level="s"`(Cargo.toml:38 이하)라 LLVM 인라이닝 역시 전역입니다. 그래서 ①의 결과만으로는 측정 wasm을 보증할 수 없습니다.
- ②도 `--release`와 wasm-opt 플래그가 같다는 보장은 없습니다. 그래서 측정용 `--release` 산출물은 **함수 개수와 코드 섹션 크기**를 함께 적습니다.

**부수 기록**
- crates/loom-bench/Cargo.toml:24-28의 `[profile.release]`/`[profile.dev]`(`panic = "unwind"`)는 워크스페이스 멤버의 프로파일이라 Cargo가 무시합니다. 이 사실을 기록에 남깁니다.

**정규화** (함수 본문은 `wasm-tools print`로 뽑음)
- `call`은 이름으로 바꿉니다.
- `call_indirect`의 type은 시그니처 텍스트로 바꿉니다.
- 데이터 세그먼트 범위 안의 `i32.const`는 `<DATA>`로 마스킹합니다. panic Location 주소가 밀리기 때문입니다(개정 #11의 189바이트 사례).

**분류**
- **추가**: `apply_surface`, BrowserDom·CountingDom 신규 메서드, `SurfaceOpCounts` 관련 함수
  - generic patch System은 bench에 소비처가 없어 인스턴스화되지 않는 것이 정상입니다.
  - "새 함수 이름 중 bench K/C/D/E/T 경로에서 호출되는 것 0"을 함께 확인합니다.
- **허용된 변경**: 아래 각 항목에 사람이 직접 검토한 기록을 붙입니다.
  - `LoomApp::tick` 단형화들과 그것을 인라인한 함수: "br_table 항목과 새 arm 블록뿐"
  - `drop_in_place<DomWrite>`, `<DomWrite as Debug>::fmt`
  - `CountingDom::reset_counts`: 호출 위치가 측정 괄호 밖(bench lib.rs:923)
  - `Candidate::set_and_tick` 단형화들(bench lib.rs:920-940): `CountingDom` 크기가 바뀌어 필드 오프셋 상수가 달라질 수 있습니다. 바뀐 명령이 `start`(:931)~`end`(:934) **바깥에만** 있거나, 괄호 안이라도 오프셋 상수만 다르다는 것을 확인합니다.
  - `Candidate`를 담는 그 밖의 함수: 오프셋 상수만 바뀌었다는 것을 같은 방식으로 확인합니다. 해당 호출처 예시: bench lib.rs:1647, :2965, :3168의 `counts().to_json()`.
- **그 외 변경은 0이어야 합니다.** `DomOpCounts::{to_json,total_mutations,mutation_observer_channel}`과 `CountingDom::counts`는 **동일**이어야 합니다.

**개수 단언** (공허한 확인 방지)
- B0/B1 함수 총수와, 동일·허용 변경·추가 각각의 개수를 기록합니다.
- 존재 단언으로 "허용 변경 ≥ 1(tick)"을 둡니다. tick이 같게 나오면 도구가 arm을 보지 못하고 있는 것입니다.

**wasm 말고 함께 비교할 산출물**
- `loom_bench.js` 글루: 새 import(`createElementNS`, `classList.toggle`, `removeAttribute`, `Reflect.set` 등)로 바뀌는 것이 정상입니다. diff를 기록합니다.
- RQ6 픽스처마다 SSR HTML, 청크 wasm·JS, bootstrap:
  - SSR HTML과 bootstrap은 **동일**을 기대합니다.
  - wasm·JS는 변경되며, 재측정 대상입니다.
- 트랙 N OUT_DIR: 코드 생성을 건드리지 않으므로 **바이트 동일**을 단언합니다. 실행 바이너리는 런타임이 바뀌어 다르므로 G2를 새로 받습니다.
- 커밋돼 있는 `crates/loom-island-*/pkg/*.wasm`: 소스와 어긋나게 됩니다. 재빌드해 커밋할지는 RQ6 재측정 절차에 따르고, 어긋남 자체를 기록합니다.

### (c) 결정적 자연 대조 (탐색적 관찰, 판정 비관여)

**판정은 v23 데이터만으로**
- §9 판정(G0 수리 후의 D/E 변형 판정 포함)은 **v23 데이터만으로** 합니다.
- §2.5 "상이한 기반 혼용 금지"에 따라 v22 값은 판정 입력이 되지 않습니다.

**§11에 사전 등록할 탐색적 관찰** (공평성 진술: 전 arm에 같은 기반 변경이고, 7c 경로는 전 arm에서 쓰이지 않음)
- (i) M2 결정성: K·C 전 셀과 트랙 T 전 arm의 M2 JSON이 v22와 v23에서 **완전히 같아야** 합니다.
  - `DomOpCounts`를 바꾸지 않으므로 키 집합도 같아야 합니다. 비교기는 **키 집합이 같다는 것과 비교한 셀 수가 0보다 크다는 것**을 먼저 단언합니다.
  - 전제: `PROTOCOL_TAG`(bench lib.rs:55, `"RQ1-benchmark-protocol/v1.1"`)는 시드에 들어갑니다. v23에서 **이 태그를 바꾸지 않는다**는 것을 등록문에 적습니다.
  - 결과가 다르면 기반 변경이 행동으로 샌 것이므로 캠페인을 중단합니다.
- (ii) 시간 지표: K/C의 v22→v23 이동을 관찰로만 기록합니다. 드리프트 귀속은 (a)의 전제가 성립할 때만 서술합니다.
- (iii) 선택(결정 사항 4): B0 산출물과 B1(또는 B2) 산출물로 K/C 부분 셀을 같은 세션에서 교차 실행하는 A/A 대조를 둡니다.

## 6. 커밋 계획 (G0 수리 커밋들 뒤에 둠, B0이 잘 정의된 중간점이 되도록)

**1. docs, 사전 등재**
- docs/09에 "7c 설계" 절(이 안)을 추가합니다.
- RQ1 프로토콜 §11에 다음을 적습니다.
  - v23 기반 구성(B0/B1/B2)
  - 공평성 진술
  - 개정 #11 면제를 쓰지 않는다는 명시
  - §5 (a)의 사전 선언 규칙
  - §5 (c)를 "탐색적 관찰, 판정 비관여"로 등록, `PROTOCOL_TAG` 고정
  - §1.1(§2.2와의 관계, 7b 전 재심사)
- 프로토콜 :2312의 `set_visible` 서술에 정정 주석을 답니다(원문 보존, §10:381 인용, 영향 범위 bench/codegen/Island 명시).
- docs/03에 판정례 후보를 등재합니다(resume 상태 복구 대 N1).
- PHASES.md:87-88 상태를 갱신합니다.

**2. 7c-1 타입과 적용부**
- `surface.rs`: `Namespace`/`BoolProp`/`ElementSpec`/`SurfaceWrite`, `apply_surface`, `SurfaceStats`, `AttrTarget`/`TextNodeTarget`/`ElementHandle`
- lib.rs: `DomWrite::Surface`(마지막 variant와 그 doc), `DomOps` 기본 메서드
- 크기·판별값 단언

**3. 7c-2 CountingDom**
- `SurfaceOpCounts`, 위임, 채널 매핑과 "카운트 ≥ 관측" 주석, 위임 테스트

**4. 7c-3 web.rs**
- `BrowserDom` 구현

**5. 7c-4 patch System**
- 제네릭 patch System, 사이트 마커(Component ZST, `With<A>`), `ChildOf` 규약

**6. 7c-5 테스트**
- MockDom과 `surface_writes.rs`(1~7), 구조 단언, tripwire

**7. 7c-6 주석 정정**
- lib.rs:278-284의 set_attribute doc, lib.rs:413-421의 깨진 doc(docs/09:374)
- 동결 전에 합니다. 주석도 바이트를 바꾸기 때문입니다.

**8. 7c-7 diff 도구와 비교 기록**
- 도구, 그리고 a55ba87 대 B0, B0 대 B1의 기록을 research-questions/data/에 둡니다.
- 툴체인, sha256, 개수, 함께 비교한 산출물, profiling 빌드 결과를 포함합니다.

**9. 하네스 변경 (B2)과 동결**
- B1→B2 diff를 따로 기록합니다.
- `pkg-bench`를 빌드하고 sha256과 툴체인을 기록한 뒤 v23 캠페인을 시작합니다.
- docs/09의 R9/R34/R35(부분)/R36/R43 행과 §2.3/§2.4/§2.8 행에 날짜를 붙여 "런타임 표면 7c 추가, 소비 codegen은 7b"를 덧붙입니다. R43 행에는 "`SpawnElement` 경로는 7b 전까지 결함 유지"도 적습니다.

## 병익님 결정이 필요한 것

1. R34 기존 arm을 debug 전용으로 집계할지, 미룰지. 집계하면 release 바이트가 바뀌고 트랙 N에서는 작동하지 않습니다(§3).
2. 혼합 텍스트 SSR 마커 규약을 v23 동결 전에 정할지.
3. SetStyle 포함 여부.
4. A/A 대조(§5 (c)(iii))를 사전 등록할지.
5. §4의 6(b)가 실패할 경우 초기값 규약을 §1.1의 (i)과 (ii) 중 어느 쪽으로 할지.
6. `adopt_element_children`의 N1 판정례 후보를 언제 심사할지. 7b의 resume codegen 전환보다는 먼저여야 합니다.

검증 근거: 크기 프로브(scratch, 저장소 미수정) `/tmp/claude-1000/-home-ybi-loom-research/57c90ce0-a918-4fbe-b381-bd36cb7cd380/scratchpad/sizeprobe/src/lib.rs`. 판별값 확인은 검토자의 `tagprobe`(x86_64만)입니다.

---

# 검토 반영 메모

| # | 지적 | 처리 | 근거 / 반영 위치 |
|---|---|---|---|
| 1 | set_visible 영향 범위 과소 | **수용** | codegen.rs:2714, :3265와 island-demo:158, productcard:42에서 `SetVisible`을 확인했습니다. §10:381의 원문도 확인했습니다. 반영: §0-1, §3, 커밋 1 |
| 2 | set_and_tick 누락 | **부분 수용** | :1647/:2965/:3168도 실제로 `counts().to_json()`의 호출처라서 "틀렸다"기보다 목록이 불완전했던 것입니다. 측정 경로의 호출(:923 `reset_counts`, :939 `counts`, 괄호 :931~:934)은 확인했습니다. 대안(별도 구조체)을 채택해 `DomOpCounts`/`counts`/`to_json`을 동일하게 유지하고, `set_and_tick`은 오프셋 한정 허용 변경으로 넣었습니다. 반영: §2.3, §5(b) |
| 3 | 진단 빌드와 측정 산출물 불일치 | **수용** | loom-bench Cargo.toml에 wasm-opt 메타데이터가 없고 멤버 프로파일(:24-28)이 무시됨을 확인했습니다. `--profiling` 빌드를 추가했고, 그것도 `--release`와 같지 않다는 한계를 명기했습니다. 반영: §5(b) |
| 4 | 판별값 인코딩 | **수용** | x86_64 값은 검토자 실측을 인용했고, wasm32는 미확인으로 표시했습니다. "마지막 variant" doc과 판별값 고정 테스트를 추가했습니다. 반영: 요약, §1, §4 |
| 5 | R34 debug 전용의 전제 | **수용** | 프로토콜 :1838에서 release 빌드를 확인했습니다. 반영: §3, 결정 1 |
| 6 | adopt N1 단서와 기존 코드 모순 | **수용** | codegen.rs:1607-1628의 시드 형태를 확인했습니다. 단정을 철회하고 판정례 후보로 등재했으며, 소비 codegen은 판정 뒤로 미뤘습니다. `text_content` 한계는 R35 부분 해결로 적었습니다. 반영: §2.4, 결정 6 |
| 7 | §2.2 저촉, 재패치 위험 | **수용** | §2.2:57과 §10:383의 원문을 확인했습니다. 7b 전용 등재, 6(a)/(b) 분리 테스트, 실패 시 대안 (i)/(ii)를 넣었습니다. 반영: §1.1, §4-6, 결정 5 |
| 8 | §2.5 혼용 | **수용** | 판정은 v23 단독으로 하고, v22 대조는 탐색적 관찰로 등록합니다. 반영: §5(c) |
| 9 | total_mutations 정의 변경 | **회피로 해소** | `DomOpCounts`를 바꾸지 않으므로(SurfaceOpCounts 분리) 측정 정의 개정이 필요 없습니다. `adopt_element_children`은 읽기 전용 그룹에 넣었습니다. 반영: §2.3 |
| 10 | M2 대조가 공허해질 위험, PROTOCOL_TAG | **수용(형태 조정)** | 새 키가 생기지 않으므로 "새 키 존재" 단언 대신 "키 집합 동일과 비교 셀 수 > 0"을 단언합니다. bench lib.rs:55의 태그를 고정합니다. 반영: §5(c)(i) |
| 11 | 바이트 게이트의 누락 소비자 | **수용** | 글루 JS, RQ6 산출물, 트랙 N OUT_DIR 동일 단언, 커밋된 pkg wasm을 추가했습니다. 개정 #11 면제를 쓰지 않는다고 명시했습니다. 반영: §5 전제, §5(b) |
| 12 | 하네스 변경의 오염 | **수용** | B0/B1/B2를 분리했습니다. 반영: §5, 커밋 9 |
| 13 | §5(a) 전제가 깨질 때 | **수용** | 사전 선언 규칙(드리프트 관찰 포기, v23 단독)을 넣었습니다. 반영: §5(a) |
| 14 | B0 점검 보강 | **수용** | 골든, HashMap 결정성, N5 insert 경로 명시를 넣었습니다. 단 G0 의미론 자체는 이 설계의 범위 밖으로 유지합니다. 반영: §5(a) |
| 15 | 사이트 마커 교차 매칭 | **수용** | Component ZST와 `With<A>`, 테스트 5(b). 반영: §1, §4 |
| 16 | 핸들 수명 | **수용** | web.rs의 free list 재사용(alloc_under)과 :228 패닉, lib.rs:757-760의 선례를 확인했습니다. `ChildOf` 규약과 테스트 7을 넣었습니다. 반영: §1, §4 |
| 17 | SpawnSpec 계약 | **수용** | `DomTarget` 대신 `ElementHandle`을 붙이고, `SpawnElement`는 7b 전까지 R43 유지로 명시했습니다. 반영: §1 |
| 18 | ElementSpec 무저촉 근거 | **수용** | PartialEq를 가드로 삼는 서술을 철회했습니다. 두 가지 근거와 구조 단언으로 바꿨습니다. 반영: §1, §2.4, §4 |
| 19 | grep 단언 깨짐 | **수용** | 메서드별 허용 목록 방식으로 바꾸고, 본문을 실제로 찾았는지 존재 단언을 추가했습니다. 반영: §4 |
| 20 | M2 채널 과대 계상 | **수용** | 반영: §2.3 주석 |
| 21 | tripwire 약함 | **수용** | write-stream에서 `Surface` 0건 단언을 1차로 두고(검출기 자체 시험 포함), `should_panic`은 2차로 내렸습니다. 반영: §4 |
| 22 | surface.rs 모듈 분리 | **수용(한계 명기)** | lib.rs 변경을 줄여 줍니다. 다만 `DomOps` 메서드와 web.rs·counting.rs의 trait impl은 한 블록이어야 하므로 그 안의 줄 이동은 피할 수 없습니다. 반영: 요약, §2 |

기각한 지적은 없습니다. 2번은 사실관계 일부(:1647 등도 호출처)를 정정해 부분 수용했습니다. 9번과 10번은 지적한 문제를 해결하되, 제안된 방법 대신 `DomOpCounts`를 아예 바꾸지 않는 방식으로 해소했습니다.

---

# 부록: 설계안 적대 검토 원문 (개정판 말미 "검토 반영 메모"의 근거)

# Phase 7c 설계안 적대 검토 (HEAD da4a9bc, 저장소는 수정하지 않음)

결론: 문제는 22건입니다. 그중 9건은 동결이나 판정을 직접 흔들 수 있어 고친 뒤에 7c 사전 등재(커밋 계획 1)로 넘어가야 합니다. 바이트 게이트(§5)는 측정에 쓰는 산출물을 다 보지 못하고 있고, 새 binding 쪽에는 프로토콜 §2.2와 부딪치는 지점과 핸들 수명 문제가 남아 있습니다.

검증에 쓴 것:
- 크기 프로브의 복사본(scratchpad `tagprobe`)으로 x86_64 판별값을 직접 확인했습니다. wasm32는 확인하지 않았습니다.
- 그 밖의 줄 번호는 전부 소스와 대조했습니다.

## A. 사실 오류와 누락 (설계 본문 정정 필요)

1. **§0-1의 `set_visible` 영향 범위가 과소합니다.**
   - `SetVisible`은 bench의 트랙 T `"F"` arm만 내는 것이 아닙니다. 컴파일러가 생성하는 조건부의 else 분기도 냅니다(codegen.rs:2714, :3265). 손코딩 Island(loom-island-demo/src/lib.rs:158, loom-island-productcard/src/lib.rs:42)도 냅니다.
   - 따라서 R36을 web.rs에서 고치면 트랙 N과 RQ6 경로도 바뀝니다.
   - "인라인 `display`만 소유" 계약은 새로 만들 필요가 없습니다. 프로토콜 §10:381의 "은닉 수단 사전 선언: inline style `display:none`"이 이미 그 계약입니다. docs/09:153(R36 행)에도 "조건부B/CSS 은닉 arm"이 적혀 있습니다.
   - **수정**: §0-1을 "프로토콜 :2312의 재검토 기록만 틀렸다"로 좁힙니다. 계약은 §10:381을 인용합니다. :2312 정정은 원문을 지우지 않는 주석 형태로 합니다.

2. **§5(b) 허용 변경 목록에서 측정 함수가 빠졌습니다.**
   - `CountingDom::{reset_counts,counts}`의 실제 호출처는 :1647/:2965/:3168이 아닙니다. 매 반복 호출되는 `Candidate::set_and_tick`입니다(crates/loom-bench/src/lib.rs:923 `reset_counts`, :939 `counts`). 이 함수 안에 측정 괄호(:931 `start` ~ :934 `end`)가 있습니다.
   - `DomOpCounts`에 필드를 붙이면 `set_and_tick`의 단형화 본문과 `last_counts` 레이아웃이 바뀝니다. 그러면 "그 외 변경 0" 게이트가 실패하거나, 측정 괄호 안의 변화가 묻힙니다.
   - **수정**: 허용 변경에 `set_and_tick` 단형화들을 명시합니다. 사람이 직접 검토해 "바뀐 명령이 `start`/`end` 바깥에만 있다"를 기록합니다.
   - 대안: `DomOpCounts`를 늘리지 않고 새 카운터를 별도 구조체(`SurfaceOpCounts`)로 분리해 bench 경로의 바이트를 덜 흔듭니다.

3. **진단 빌드와 측정 산출물이 다릅니다.**
   - `cargo build --release`의 wasm은 wasm-opt를 거치기 전 상태입니다. 측정에 쓰는 `wasm-pack build --release`는 wasm-opt를 돌립니다. crates/loom-bench/Cargo.toml에 `wasm-opt = false` 메타데이터가 없기 때문입니다.
   - wasm-opt의 인라이닝과 중복 함수 제거는 모듈 전체에 걸쳐 결정됩니다. 그래서 opt 전 diff가 같다고 해서 측정 wasm의 K/C 함수가 같다는 보장은 없습니다. 워크스페이스 프로파일도 `opt-level="s"`, `lto=true`(Cargo.toml)라 LLVM 인라이닝 역시 전역입니다.
   - 덤으로, crates/loom-bench/Cargo.toml의 자체 `[profile.release]`는 Cargo가 무시합니다. 이 사실도 기록에 남겨야 합니다.
   - **수정**: `wasm-pack build --profiling`(opt 후에도 이름 유지)으로 B0/B1을 한 번 더 diff합니다. 측정용 `--release` 산출물은 함수 개수와 코드 섹션 크기를 병기합니다.

4. **크기 프로브는 크기만 봤고, 판별값 인코딩은 보지 않았습니다.** 크기가 같다고 해서 생성 지점의 기계어가 같다는 뜻은 아닙니다.
   - 직접 확인한 결과(x86_64): `DomWrite`는 niche 인코딩입니다. `SpawnElement`가 dataful variant이고, 태그는 offset 0의 String capacity niche에 들어갑니다(SetText=0x8000…00, ClearChildren=…01, RemoveNode=…04, SetVisible=…05, SetInputValue=…0a).
   - `Surface`를 **맨 끝**에 붙이면 이 값들이 유지됩니다. 중간에 넣으면 기존 생성 지점 전부(lib.rs:733, cond.rs:108/:163/:278 등)의 코드가 바뀝니다. wasm32는 확인하지 않았습니다.
   - **수정**: "Surface는 마지막 variant" 규칙을 doc에 명시합니다. 기존 variant의 판별값을 고정하는 테스트(native)를 추가하고, wasm32는 §5 diff로 확인합니다.

5. **R34 debug 전용 선택지의 전제가 부정확합니다.**
   - 트랙 N은 release로 돕니다(프로토콜 :1838 `cargo build --release -p loom-track-n`). 그래서 debug 전용 카운터는 트랙 N에서 작동하지 않고, 작동하는 것은 `cargo test`(debug)로 도는 loom-nested-fixture뿐입니다.
   - `cfg`로 빠지는 코드라도 기존 arm 안에 들어가면 뒤쪽 줄 번호가 밀립니다. 그래서 panic 위치 데이터가 바뀝니다.
   - 런타임 소스가 바뀌므로 개정 #11의 트랙 N 조건("의존 크레이트 소스 무변경")도 깨집니다.
   - "release wasm에 영향 없음"은 바이트 수준에서 틀린 말입니다. **수정**: 결정 사항 1의 설명을 이렇게 고칩니다.

6. **`adopt_element_children`의 N1 단서가 기존 코드와 모순됩니다.**
   - 기존 list resume 코드 생성은 `adopt_children`이 읽은 텍스트로 row의 `Signal`과 리스트의 `Signal<Vec<String>>`을 시드합니다(codegen.rs:1607-1628). 이 값이 다음 reconcile의 "이전 값"이 됩니다. 설계안이 "N1 위반이므로 금지"라고 한 바로 그 형태입니다.
   - 단서가 맞다면 기존 resume이 N1 위반입니다. 단서가 틀렸다면 문구를 고쳐야 합니다. 어느 쪽인지 감으로 단정하지 말아야 합니다(CLAUDE.md 1).
   - **수정**: "resume 시점의 SSR DOM 상태 복구 대 N1 '알고리즘 입력이 렌더 산출물'" 문제를 docs/03 판정례 후보로 등재합니다(RQ1 G0 기록의 후보 (a)~(h) 옆).
   - 덧붙여 `text_content()`는 자손 텍스트를 모두 이어 붙입니다. 그래서 `<li>v<span>…</span></li>`처럼 생긴 row는 값이 틀리게 복구됩니다. 공백 노드는 빠지지만 R35는 절반만 해결되는 셈이므로 docs/09 R35 행에 적어야 합니다.

## B. 프로토콜 저촉

7. **§2.2:57, §10:383과의 관계가 정리되지 않았습니다.** 해당 조항은 "초기값은 스폰 번들 포함이 최선형이고, 초기값까지 후속 재패치하는 2-pass 스폰은 체크리스트 미달"이라고 정합니다.
   - `BindAttrTarget` 뒤에 별도 `SetAttr`로 초기값을 넣는 방식은 같은 배치이긴 하지만 스폰 write와 분리돼 있습니다.
   - 더 큰 문제가 있습니다. row 스코프 binding처럼 **같은 tick에 Commands로 spawn된** binding은 `Signal`의 Added 틱이 patch System의 `last_run`보다 뒤입니다. 그래서 **다음 tick에 `Changed`가 참이 되어 `SetAttr`가 한 번 더** 나갈 수 있습니다. 이것이 곧 "초기값 후속 재패치"입니다.
   - **수정**: (a) 7c 경로는 RQ1 측정 arm이 쓰지 않는 7b 전용 경로라고 §11 등재문에 명시하고, §2.2와 대비해 유불리를 기록합니다. (b) 테스트 6을 setup에서 spawn한 경우와 Commands로 spawn한 row 스코프 경우 둘 다로 나누고, 두 번째 tick에 0건임을 단언합니다. 실패하면 설계를 바꿉니다.

8. **§2.5 "상이한 기반 혼용 금지"에 걸립니다.**
   - §5(c)는 "K/C의 v22→v23 시간 이동을 드리프트로 귀속하고, D/E는 K 대비 비율로 해석"한다고 합니다. 이것이 판정 입력이 되면 위반입니다.
   - **수정**: §9 판정은 v23 데이터만으로 한다고 명시합니다. v22 대조와 B0 대 B1의 A/A 교차 실행은 §11에 "탐색적 관찰, 판정 비관여"로 사전 등록하고 공평성 진술을 붙입니다.

9. **`total_mutations`의 정의가 바뀝니다.**
   - 이 값은 §9 E 기각 (i)의 "셀당 총 DOM op 수"입니다(counting.rs:51-53).
   - 필드를 합산에 넣는 것은 측정 정의의 변경이므로 §11 개정 항목과 공평성 진술(전 arm에서 0이라 중립)이 필요합니다.
   - 분석 코드도 개정 #11이 열거한 동결 대상입니다.
   - **수정**: §11 등재 항목에 포함합니다. `adopt_element_children`은 기존 `adopt_children`처럼 읽기 전용 그룹에 넣고 합산에서 뺍니다(counting.rs:45-47). 설계안의 채널 매핑에 이 항목이 빠져 있습니다.

10. **M2 결정성 대조(§5(c))가 공허해질 수 있습니다.**
    - v22 JSON에는 새 키가 없습니다. 비교기가 모르는 키를 무시하면 "같음"이 공허하게 통과합니다. **수정**: v23에 새 키가 **존재**한다는 단언을 먼저 두고, v22 쪽에 없는 키는 0으로 취급합니다.
    - 전제 조건이 하나 더 있습니다. `PROTOCOL_TAG`(crates/loom-bench/src/lib.rs:55, `"…/v1.1"`)는 시드에 들어갑니다. v23에서 이 값을 올리면 M2가 전부 달라집니다. 이 태그를 바꾸지 않는다는 것을 사전 등록에 적어야 합니다.

## C. 바이트 게이트의 누락 소비자

11. **비교 대상이 `loom_bench_bg.wasm` 하나뿐입니다.** 다음이 빠졌습니다.
    - `loom_bench.js` 글루: `createElementNS`, `classList.toggle`, `removeAttribute`, `Reflect.set` 등 새 import 때문에 바뀝니다.
    - RQ6 픽스처마다: SSR HTML, 청크 wasm·JS, bootstrap.
    - 트랙 N OUT_DIR: 코드 생성을 건드리지 않으므로 **같아야 함**을 단언합니다.
    - 커밋돼 있는 `crates/loom-island-*/pkg/*.wasm`: 소스와 어긋나게 됩니다.
    - **수정**: 이번 변경은 바이트가 반드시 바뀌므로 개정 #11 면제를 **쓰지 않는다**고 명시합니다. 함수 diff는 면제 근거가 아니라 "K/C가 자연 대조군"이라는 공평성 근거라고 구분해 둡니다.

12. **v23 캠페인의 다른 하네스 변경이 게이트를 오염시킵니다.**
    - Firefox BiDi 재개나 트랙 L 하네스 수정이 `loom-bench`를 바꾸면, B0→B1 diff의 "그 외 변경 0"을 지킬 수 없습니다.
    - **수정**: B1을 "B0 + 7c 커밋만"으로 정의합니다. 하네스 변경은 B1→B2로 따로 diff하고 따로 기록합니다.

13. **§5(a) 전제가 깨질 때의 대응이 없습니다.**
    - G0 수리가 공유 코드(`list.rs` 공통 헬퍼, `loom-flush-core` `plan_writes*`, `RowSpawnCtx`/`DefaultRow::emit` lib.rs:733-739)를 건드리면 K/C는 더 이상 자연 대조군이 아닙니다.
    - **수정**: 그 경우 드리프트 귀속을 포기하고 v23 단독 판정으로 간다는 규칙을 사전에 선언합니다.

14. **G0 의미론은 이 설계의 범위 밖이지만, B0 점검에 넣어야 할 것이 있습니다.**
    - G0 수리가 HashMap을 쓰면 반복 순서가 write 열로 새는지 봐야 합니다. 골든(`tests/lis_output_identity.rs`, `tests/write_identity.rs`) 무수정 통과와 RandomState 시드를 바꿔 돌리는 결정성 테스트를 B0 조건에 넣습니다.
    - N5 재심사 대상에는 **insert 경로**(SpawnRow인지 SpawnRowSpec인지)를 명시해야 합니다. G0 기록의 후보 (f)가 "판정례 9(가)는 apply 경로를 보지 않음"이라고 했기 때문입니다. 7b가 컴파일된 리스트를 `SpawnRowSpec`으로 옮기면 심사받은 산출물과 실제 산출물이 달라집니다.

## D. 의미와 No-VDOM

15. **사이트 마커가 Component 필터가 아니면 교차 매칭이 다시 생깁니다.**
    - `patch_attr_system<String, A1>`과 `<String, A2>`가 둘 다 모든 `AttrTarget` entity와 매칭될 수 있습니다. `AttrTarget`을 도입한 이유(lib.rs:965-977의 교차 매칭)가 그대로 재발합니다.
    - **수정**: 마커를 `#[derive(Component)]` ZST로 만들고 쿼리를 `With<A>`로 거른다고 명시합니다. 테스트 5에 "같은 T의 속성 사이트 두 개"를 추가합니다.

16. **`AttrTarget`/`TextNodeTarget` 핸들의 수명이 정해지지 않았습니다.**
    - `release_subtree`는 `alloc_under`로 부모에 매단 텍스트 노드 슬롯까지 해제합니다(web.rs:225-235 부근). binding entity가 남아 있으면 핸들이 오래된 채로 남습니다. 슬롯이 재사용되면 엉뚱한 노드에 쓰거나 "해제된 NodeHandle" panic이 납니다.
    - **수정**: binding을 `ChildOf(row 또는 엘리먼트 entity)`로 매답니다(lib.rs:757-760의 RowBinding 선례). "row 삭제 후 슬롯 재사용" 테스트를 추가합니다.

17. **`SpawnSpec`의 계약이 비어 있습니다.**
    - entity에 `DomTarget`을 붙이는지가 정해지지 않았습니다. 붙이고 `Signal<String>`도 있으면 `patch_text_system::<String>`이 개입합니다.
    - 기존 `SpawnElement`(lib.rs:347-354, tag/class/loom_id/text)와 겹칩니다. `SpawnElement` 경로는 계속 HTML 네임스페이스로 남아, 7b에서 생성 코드를 바꾸기 전까지 R43이 유지된다는 점도 적어야 합니다.
    - **수정**: 두 가지를 명시합니다.

18. **`ElementSpec`은 N1의 실질 기준("이름이 아니라 형상")으로 보면 요소 기술자입니다.**
    - `PartialEq`를 derive하지 않는 것은 가드가 되지 못합니다. `ptr::eq`나 필드 비교는 여전히 가능합니다.
    - **수정**: 무저촉 근거를 "갱신마다 생성하지 않음"과 "비교·조정에 입력되지 않음" 두 가지로 적습니다. 구조 단언도 추가합니다. `list.rs`·`cond.rs`·`loom-flush-core`가 `ElementSpec`/`spec`을 참조하지 않아야 하고, `apply_surface`가 `get_input_value`/`adopt*`를 호출하지 않아야 합니다.

19. **"web.rs 신규 메서드에 DOM 읽기 없음" grep 단언이 깨집니다.**
    - `adopt_element_children`은 설계상 DOM을 읽습니다(`children()`, `text_content()`).
    - **수정**: 메서드별 허용 목록 방식으로 바꿉니다. 파일 전체를 grep하면 실패하거나, 실패를 피하려고 파일 전체를 건너뛰게 됩니다.

20. **M2 채널 대응이 어긋납니다.**
    - `toggle_with_force`가 이미 원하는 상태일 때, 그리고 없는 속성에 `remove_attribute`할 때는 MutationRecord가 생기지 않습니다. `setAttribute`는 항상 생깁니다.
    - **수정**: counting.rs의 `mutation_observer_channel`에 "카운트가 관측보다 클 수 있음" 주석을 답니다(moveBefore 주석의 선례).

## E. 테스트와 존재 단언 보강

21. **tripwire가 약합니다.**
    - TreeDom의 `should_panic`은 기본 구현이 있다는 것만 보여 줍니다. 트랙 N이 7c 경로를 타지 않는다는 것은 보여 주지 않습니다.
    - **수정**: 트랙 N 실행에서 `PendingDomWrites`에 `Surface`가 0건이라는 단언을 추가합니다. 같은 테스트 안에서 의도적으로 `Surface`를 1건 넣어 검출기가 1을 세는지 먼저 확인합니다.

22. **배치 위치에 대한 제안입니다.**
    - 새 타입을 lib.rs 끝에 두는 대신 `surface.rs` 모듈로 분리하면 lib.rs에서 바뀌는 줄이 `tick`의 arm과 모듈 선언뿐입니다. 사람이 직접 하는 diff 검토가 쉬워집니다.

## 대조해서 맞다고 확인한 것

- 기존 variant는 12종입니다.
- 기존 렌더러 세 곳(recorder_common.rs, row_spawner.rs, recorder_axes.rs)에 `other =>` 폴백이 있습니다. `DomWrite`는 `Debug`만 derive합니다.
- 필요한 web-sys feature는 이미 다 켜져 있습니다(crates/loom-runtime/Cargo.toml:17). feature 추가가 필요 없습니다.
- cond.rs:175, bench :2410/:2641-2642, web.rs의 인용 줄, 판정례 9(가), G0 후보 (f)는 원문과 일치합니다.
