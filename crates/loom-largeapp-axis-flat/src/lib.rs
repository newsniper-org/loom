//! RQ6 §4.4 "원본 앱 규모" 축 — H-Qwik(Flatten) 팔, N=50 filler + 1
//! TargetWidget (build.rs 참고). wasm 엔트리는 `crates/loom-resume-demo`의
//! `ResumedComponent`(비-Island 루트 컴포넌트의 기존 resumability 엔트리
//! 포인트) 패턴을 그대로 재사용 — 이 그래프엔 `sharedSignal`이 없어
//! `crates/loom-widget-embed`의 `ResumedFlatApp`보다 더 단순한 (has_shared
//! =false) 시그니처를 쓴다.

pub mod generated_flat {
    include!(concat!(env!("OUT_DIR"), "/generated_flat.rs"));
}

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct ResumedLargeFlatApp {
    app: loom_runtime::LoomApp,
    dom: loom_runtime::web::BrowserDom,
    signals: generated_flat::Signals,
}

#[wasm_bindgen]
impl ResumedLargeFlatApp {
    #[wasm_bindgen(constructor)]
    pub fn new(captures_str: &str) -> ResumedLargeFlatApp {
        console_error_panic_hook::set_once();
        let document = web_sys::window().expect("no `window`").document().expect("no `document`");

        let captures: Vec<(String, String)> = captures_str
            .split(',')
            .filter(|s| !s.is_empty())
            .map(|kv| {
                let mut parts = kv.splitn(2, ':');
                let key = parts.next().unwrap_or_default().to_string();
                let value = parts.next().unwrap_or_default().to_string();
                (key, value)
            })
            .collect();
        let capture_refs: Vec<(&str, &str)> = captures.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();

        let mut app = loom_runtime::LoomApp::new();
        let mut dom = loom_runtime::web::BrowserDom::new_for_resume(document.clone());

        let tagged = document
            .query_selector_all("[data-loom-id]")
            .expect("query_selector_all failed");
        let mut by_name: std::collections::HashMap<String, loom_runtime::NodeHandle> = std::collections::HashMap::new();
        for i in 0..tagged.length() {
            let node = tagged.get(i).expect("NodeList index out of bounds");
            let el: web_sys::Element = wasm_bindgen::JsCast::unchecked_into(node.clone());
            let name = el.get_attribute("data-loom-id").expect("just matched the selector");
            let handle = dom.adopt(node);
            by_name.insert(name, handle);
        }
        let find_node = move |name: &str| -> loom_runtime::NodeHandle {
            *by_name
                .get(name)
                .unwrap_or_else(|| panic!("loom resume: no element tagged data-loom-id=\"{name}\""))
        };

        let signals = generated_flat::resume(&mut app, &mut dom, find_node, &capture_refs);
        ResumedLargeFlatApp { app, dom, signals }
    }

    pub fn dispatch(&mut self, symbol: &str, value: Option<String>) {
        let action = generated_flat::resume_action_by_symbol(symbol, value.as_deref().unwrap_or(""));
        generated_flat::apply_action(&mut self.app, &self.signals, &action);
        self.app.tick(&mut self.dom);
    }
}
