//! RQ6 §5.4 condition B spike — "island별 wasm 인스턴스 분리"의 실제 구현
//! 시도. `crates/loom-island-demo`의 `QuantityPickerIsland`와 논리는 동일하지만,
//! **자기 자신만의 별도 크레이트/별도 `.wasm` 산출물**로 컴파일된다는 게 유일하고
//! 결정적인 차이 — `crates/loom-island-productcard`가 그 짝. 두 크레이트를 각각
//! `wasm-pack`으로 빌드하면 진짜로 서로 다른 `WebAssembly.Instance`(별개 선형
//! 메모리)가 되므로, 한쪽의 panic이 다른 쪽에 구조적으로 영향을 줄 수 없음을
//! **직접 관찰**할 수 있다(RQ3 "알려진 약점" 1번의 세 후보 중 하나를 실측하는 것 —
//! roadmap/PHASES.md RQ6 §3.2 절 참고). `panic_now`는 이 관찰을 위한 fault-injection
//! 전용 메서드 — 실제 QuantityPicker 로직에는 없는, 이 스파이크만을 위한 진입점.

use loom_runtime::web::{find_own, BrowserDom};
use loom_runtime::{LoomApp, Signal};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// `own_tagged_descendants`/`find_own` (opaque-boundary DOM walk) used to be
// duplicated by hand in this crate — sharing a *dependency* crate
// (`loom-runtime`, already a dependency of both this crate and
// `loom-island-productcard`) doesn't cross the wasm-instance boundary that
// this crate's whole point is to keep separate, only sharing a *binary*
// would — so the duplication was removed (2026-07-24, Task #25) in favor of
// `loom_runtime::web::find_own`.

struct QuantityState {
    app: LoomApp,
    dom: BrowserDom,
    entity: bevy_ecs::entity::Entity,
}

#[wasm_bindgen]
pub struct QuantityPickerIsland {
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
        app.tick(&mut dom);

        let state = Rc::new(RefCell::new(QuantityState { app, dom, entity }));

        let wire = |el: web_sys::Element, delta: i32, state: Rc<RefCell<QuantityState>>| {
            let closure = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
                let mut s = state.borrow_mut();
                let QuantityState { app, dom, entity } = &mut *s;
                let value = &mut app.world.get_mut::<Signal<i32>>(*entity).unwrap().0;
                *value = (*value + delta).max(0);
                app.tick(dom);
            });
            el.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
                .expect("add_event_listener_with_callback failed");
            closure.forget();
        };
        wire(find_own(&host, "qty-increment"), 1, state.clone());
        wire(find_own(&host, "qty-decrement"), -1, state.clone());

        QuantityPickerIsland { state }
    }

    /// Fault-injection only (condition B spike, not real QuantityPicker
    /// behavior) — deliberately panics *this* instance's Rust code, to
    /// observe from JS whether a sibling Island in a *separate* wasm
    /// instance (`loom-island-productcard`) is affected at all.
    pub fn panic_now(&mut self) {
        panic!("loom-island-quantitypicker: deliberate fault injection (condition B spike)");
    }

    /// Lets the test confirm this specific instance is still alive/usable
    /// after some other instance (or, in a condition-A-style shared-instance
    /// setup, this same instance) has panicked elsewhere.
    pub fn quantity(&self) -> i32 {
        let s = self.state.borrow();
        s.app.world.get::<Signal<i32>>(s.entity).unwrap().0
    }

    pub fn begin_detach(&mut self) {}
}
