//! wasm-bindgen entry point: wires the compiler-generated app (docs/02, see
//! `generated` module below, produced by build.rs from ../../demo/counter-list.tsx)
//! to a real browser DOM. This is where the Phase 1 end condition
//! (roadmap/PHASES.md: "VDOM 없이 카운터 + 동적 리스트가 실제로 브라우저에서
//! 갱신됨") gets exercised for real, not just against the native mock DOM in
//! loom-runtime's tests.
//!
//! Event wiring is hand-written, not generated: Phase 1 excludes
//! Resumability/LRef (roadmap/PHASES.md), so there's no serialization
//! boundary requiring behavior to be addressed by an integer opcode — a
//! direct Rust closure per action is the honest, scope-appropriate choice
//! (see loom-compiler/src/codegen.rs's module doc comment).

mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use loom_runtime::web::BrowserDom;
use loom_runtime::LoomApp;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

struct AppState {
    app: LoomApp,
    dom: BrowserDom,
    signals: generated::Signals,
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().expect("no global `window`");
    let document = window.document().expect("no `document` on window");
    let mount_el = document
        .get_element_by_id("app")
        .expect("no element with id=\"app\" — see demo/index.html");
    let mount_node: web_sys::Node = mount_el.unchecked_into();

    let (mut dom, mount_handle) = BrowserDom::new(document, mount_node);
    let mut app = LoomApp::new();
    let (signals, actions, input_actions) = generated::setup(&mut app, &mut dom, mount_handle);

    // Render the initial mount (Signal::Added counts as Changed — see
    // loom-runtime's doc comment) before wiring any events.
    app.tick(&mut dom);

    let state = Rc::new(RefCell::new(AppState { app, dom, signals }));

    for (handle, action) in actions {
        let state = Rc::clone(&state);
        let element = state.borrow().dom.element(handle);
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::MouseEvent| {
            let mut state = state.borrow_mut();
            let AppState { app, dom, signals } = &mut *state;
            generated::apply_action(app, signals, &action);
            app.tick(dom);
        });
        element
            .add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())
            .expect("add_event_listener_with_callback failed");
        // Intentionally leaked: these listeners must live for the page's
        // whole lifetime, and Phase 1's demo never tears the app down.
        closure.forget();
    }

    // `<input>` two-way bindings: `make_action` is a bare tuple-variant
    // constructor (`Action::ActionN`, itself a real `fn(String) -> Action`,
    // no closure — see codegen.rs's `setup()` doc comment), applied to the
    // just-typed value read straight off the real `input` DOM event.
    for (handle, make_action) in input_actions {
        let state = Rc::clone(&state);
        let element = state.borrow().dom.element(handle);
        let closure = Closure::<dyn FnMut(_)>::new(move |event: web_sys::Event| {
            let input: web_sys::HtmlInputElement = event.target().expect("input event has no target").unchecked_into();
            let action = make_action(input.value());
            let mut state = state.borrow_mut();
            let AppState { app, dom, signals } = &mut *state;
            generated::apply_action(app, signals, &action);
            app.tick(dom);
        });
        element
            .add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())
            .expect("add_event_listener_with_callback failed");
        closure.forget();
    }

    Ok(())
}
