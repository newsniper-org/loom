//! Phase 3 increment 2/3 — hand-coded proof that RQ3's rules
//! (research-questions/RQ3-island-supervision-interaction.md) hold with a
//! REAL nested DOM + REAL JS registry (demo/resume/registry.js), not just
//! the pure state-machine level `loom_runtime::island` already covers
//! natively. Not compiler-generated — no Island authoring surface exists in
//! loom-compiler yet (see roadmap/PHASES.md Phase 3 status); this is the
//! same "runtime mechanism proven by hand first" step RQ1 Track L/T and
//! Phase 2's resumability kickoff both took before any compiler
//! integration existed for either.
//!
//! Two independent Islands, ProductCard (`client:visible`) nesting
//! QuantityPicker (`client:load`) — RQ3's own example scenario
//! (`<ProductCard client:visible><QuantityPicker client:load/></ProductCard>`,
//! see that doc's problem statement) — each gets its own `LoomApp`/`World`
//! (see `loom_runtime::island`'s module doc for why that's not a design
//! choice but a consequence of the wasm-instance boundary), sharing only
//! the same physical `BrowserDom`/page — see `demo/islands/index.html`.
//!
//! QuantityPicker's quantity feeds RQ4's minimal shared-signal-store
//! mechanism (`demo/resume/shared-store.js`) — CartBadge (in that HTML
//! file's header) reflects it purely via a JS-side DOM patch, with **no
//! Island/wasm chunk of its own at all** — there is deliberately no
//! `CartBadgeIsland` type in this file; see that HTML file's markup and
//! roadmap/PHASES.md Phase 3's RQ4 section for why that absence is the
//! actual point being proven, not an oversight.
//!
//! **2026-10-17 정정**: 이 크레이트는 2026-07-24 시점의 손코딩 실증으로 보존된
//! 것이고, 위 서술 중 둘은 그 뒤 사실과 다르다.
//! - "no Island authoring surface exists in loom-compiler yet": 컴파일러 Island
//!   통합(증분 4, `ComponentGraph`/`client:*` 지시자, 2026-07-24 b4dc6e7)과
//!   Task #25/#26의 `GraphMode::Islands`가 생기면서 같은 구조를 컴파일러가 낸다 —
//!   `crates/loom-island-compiled`(같은 ProductCard 시나리오), `crates/
//!   loom-widget-embed`, per-Island 코드 분할 `crates/loom-widget-embed-island-*`
//!   (Task #27). 아래 `ProductCardIsland` 문서의 같은 문장도 마찬가지다.
//! - RQ4의 shared-signal-store: RQ4는 2026-07-28 **전면 폐기**됐다. RQ6가
//!   H-Qwik(강한 형태)으로 확정되어 기본 모델은 페이지 전체가 하나의 World이고,
//!   위젯 간 공유는 같은 World의 Signal 문제가 됐다. Island 경계 자체가 이제
//!   "지연 activation 경계 세분화"를 선택했을 때의 선택적 최적화다(docs/04). 이
//!   크레이트가 보인 것(RQ3 규칙이 실제 DOM·registry에서 성립, 청크 없는
//!   구독자)은 기록으로 유효하지만 기본 모델의 설명이 아니다.

use loom_runtime::web::{find_own, BrowserDom};
use loom_runtime::{LoomApp, Signal};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__loomShared"])]
    fn set(key: &str, value: i32);
}

struct QuantityState {
    app: LoomApp,
    dom: BrowserDom,
    entity: bevy_ecs::entity::Entity,
}

/// `client:load` — activates immediately on page load (see registry.js).
/// Owns its own `LoomApp`/`World`, wired to elements already present in
/// `host`'s SSR'd (here, hand-authored) markup via `adopt`, not recreated —
/// same "resume ≠ hydrate" shape as Phase 2's `ResumedComponent`.
#[wasm_bindgen]
pub struct QuantityPickerIsland {
    #[allow(dead_code)] // kept alive for the closures capturing a clone of it
    state: Rc<RefCell<QuantityState>>,
}

#[wasm_bindgen]
impl QuantityPickerIsland {
    #[wasm_bindgen(constructor)]
    pub fn new(host: web_sys::Element) -> QuantityPickerIsland {
        console_error_panic_hook::set_once();
        let document = web_sys::window().expect("no `window`").document().expect("no `document`");
        let mut dom = BrowserDom::new_for_resume(document);

        let value_el = find_own(&host, "qty-value");
        let initial: i32 = value_el
            .text_content()
            .unwrap_or_default()
            .trim()
            .parse()
            .unwrap_or(1);
        let value_handle = dom.adopt(value_el.unchecked_into());

        let mut app = LoomApp::new();
        app.add_systems(loom_runtime::patch_text_system::<i32>);
        let entity = app
            .world
            .spawn((Signal(initial), loom_runtime::DomTarget(value_handle)))
            .id();
        app.tick(&mut dom); // settle, same reasoning as generated::resume()'s.
        set("cartCount", initial); // seed the shared store with the initial quantity.

        let state = Rc::new(RefCell::new(QuantityState { app, dom, entity }));

        let wire = |el: web_sys::Element, delta: i32, state: Rc<RefCell<QuantityState>>| {
            let closure = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
                let mut s = state.borrow_mut();
                let QuantityState { app, dom, entity } = &mut *s;
                let value = &mut app.world.get_mut::<Signal<i32>>(*entity).unwrap().0;
                *value = (*value + delta).max(0);
                let new_value = *value;
                app.tick(dom);
                set("cartCount", new_value);
            });
            el.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                .expect("add_event_listener_with_callback failed");
            closure.forget();
        };
        wire(find_own(&host, "qty-increment"), 1, state.clone());
        wire(find_own(&host, "qty-decrement"), -1, state.clone());

        QuantityPickerIsland { state }
    }

    /// RQ3's synchronous detach notification (`IslandInstance::begin_detach`'s
    /// contract, invoked by registry.js). This demo has no in-flight async
    /// work to cancel, so there is no separate "finish" step here — the
    /// click closures were `forget()`'d (an accepted leak for the page's
    /// lifetime, same as every other demo in this repo) and simply stop
    /// mattering once their host element is gone.
    pub fn begin_detach(&mut self) {}
}

/// (2026-10-17 note: "candidate F" is the Phase 1 label. `cond_reconcile_f_system` is the CSS-hide preserve variant, i.e. the protocol's CSS 은닉 (v-show) control arm — not the F family F-i/ii/iii added 2026-07-31. See cond.rs header.)
/// Replicates RQ1 Track T candidate F's control flow
/// (`cond_reconcile_f_system`) with this specific site's *real* content
/// inlined, instead of registering that generic System directly — the same
/// relationship codegen.rs's compiler-generated `cond_reconcile_site_N`
/// functions have to it (see that module's doc comment for why: F's own
/// System hardcodes a benchmark placeholder child, which is right for
/// measuring the mechanism but wrong for an actual product description).
fn product_details_reconcile_system(
    mut q: bevy_ecs::prelude::Query<
        (
            &Signal<bool>,
            &mut loom_runtime::CondBlock,
            &loom_runtime::DomTarget,
        ),
        bevy_ecs::prelude::Changed<Signal<bool>>,
    >,
    mut commands: bevy_ecs::prelude::Commands,
    mut pending: bevy_ecs::prelude::ResMut<loom_runtime::PendingDomWrites>,
) {
    for (cond, mut block, anchor) in &mut q {
        if block.children.is_empty() {
            if cond.0 {
                pending.0.push(loom_runtime::DomWrite::AppendStaticText {
                    parent: loom_runtime::WriteParent::Node(anchor.0),
                    text: "Free shipping on orders over $50.".to_string(),
                });
                block.children.push(commands.spawn(()).id());
            }
        } else {
            pending.0.push(loom_runtime::DomWrite::SetVisible {
                node: anchor.0,
                visible: cond.0,
            });
        }
    }
}

struct ProductCardState {
    app: LoomApp,
    dom: BrowserDom,
    expanded_entity: bevy_ecs::entity::Entity,
}

/// `client:visible` — activates only once its host scrolls into view (see
/// registry.js's `IntersectionObserver`) for its own "show details" toggle.
/// Not compiler-generated, since no Island authoring surface exists yet.
/// (2026-10-17: 작성 당시 기준. 컴파일된 판본은 `crates/loom-island-compiled`
/// — 모듈 문서의 2026-10-17 정정 참고.)
#[wasm_bindgen]
pub struct ProductCardIsland {
    #[allow(dead_code)]
    state: Rc<RefCell<ProductCardState>>,
}

#[wasm_bindgen]
impl ProductCardIsland {
    #[wasm_bindgen(constructor)]
    pub fn new(host: web_sys::Element) -> ProductCardIsland {
        console_error_panic_hook::set_once();
        let document = web_sys::window().expect("no `window`").document().expect("no `document`");
        let mut dom = BrowserDom::new_for_resume(document);

        let wrapper_el = find_own(&host, "details");
        let wrapper = dom.adopt(wrapper_el.unchecked_into());

        let mut app = LoomApp::new();
        app.add_systems(product_details_reconcile_system);
        let expanded_entity = app
            .world
            .spawn((
                Signal(false),
                loom_runtime::CondBlock::with_size(1),
                loom_runtime::DomTarget(wrapper),
            ))
            .id();
        app.tick(&mut dom);

        let state = Rc::new(RefCell::new(ProductCardState { app, dom, expanded_entity }));

        let toggle_el = find_own(&host, "expand-toggle");
        let toggle_state = state.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
            let mut s = toggle_state.borrow_mut();
            let ProductCardState { app, dom, expanded_entity, .. } = &mut *s;
            let current = app.world.get::<Signal<bool>>(*expanded_entity).unwrap().0;
            app.world.get_mut::<Signal<bool>>(*expanded_entity).unwrap().0 = !current;
            app.tick(dom);
        });
        toggle_el
            .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
            .expect("add_event_listener_with_callback failed");
        closure.forget();

        ProductCardIsland { state }
    }

    /// See `QuantityPickerIsland::begin_detach`'s doc comment — same
    /// reasoning, no in-flight async work exists in this minimal slice.
    pub fn begin_detach(&mut self) {}
}
