//! RQ6 §5.4 condition B spike — see `crates/loom-island-quantitypicker`'s
//! module doc comment for the full rationale. This crate is the other half
//! of the pair: compiled to its own, separate `.wasm`, so that a panic
//! inside `loom-island-quantitypicker`'s instance is structurally unable to
//! reach this one.

use loom_runtime::web::{find_own, BrowserDom};
use loom_runtime::Signal;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// `own_tagged_descendants`/`find_own` used to be duplicated by hand in this
// crate (its entire point being a separate wasm compilation unit — see
// `loom-island-quantitypicker`'s doc comment) — promoted to
// `loom_runtime::web` (2026-07-24, Task #25) since sharing a *dependency
// crate* doesn't compromise that separation, only sharing a *binary* would.

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
    app: loom_runtime::LoomApp,
    dom: BrowserDom,
    expanded_entity: bevy_ecs::entity::Entity,
}

#[wasm_bindgen]
pub struct ProductCardIsland {
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

        let mut app = loom_runtime::LoomApp::new();
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

    /// Lets the test confirm this instance is still fully functional after
    /// a *different* wasm instance (QuantityPicker) has panicked — the
    /// actual condition-B claim being checked.
    pub fn is_expanded(&self) -> bool {
        let s = self.state.borrow();
        s.app.world.get::<Signal<bool>>(s.expanded_entity).unwrap().0
    }

    pub fn begin_detach(&mut self) {}
}
