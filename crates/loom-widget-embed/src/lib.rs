//! Task #26 — RQ6 §4.2 widget-embed fixture, compiled through the real
//! loomc pipeline TWICE (`build.rs`, once per `GraphMode`) from the SAME
//! `demo/widget-embed.tsx` source — RQ6 §4.1's "정체성 통제" principle made
//! literal.
//!
//! `generated` (`GraphMode::Islands`): `App`'s own `setup()`/`resume()` is
//! the page's SSR entry point (`render_html()`); `ProductCardIsland`/
//! `QuantityPickerIsland`/`CheckoutFormIsland`/`MiniDashboardIsland`
//! (generated inside their own namespaced modules) are this crate's
//! compiled Island entry points, wired by `demo/resume/registry.js` and
//! `demo/resume/shared-store.js` exactly like Task #25's smaller fixture.
//!
//! `generated_flat` (`GraphMode::Flatten`): the SAME source as RQ6 §8 item
//! 4's H-Qwik counterfactual — ONE merged `Signals`/`Action`/`World`, no
//! Island boundary at all (`render_html_flat()`).

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub mod generated_flat {
    include!(concat!(env!("OUT_DIR"), "/generated_flat.rs"));
}

/// RQ6 §6 M4(상주 메모리) — RQ1 §7 M5와 같은 "1차 근사" 방식(실제 힙 할당자
/// 통계가 아니라 wasm 선형 메모리의 총 페이지 수) — 여러 Island 인스턴스가
/// 이 wasm 모듈을 공유하는 N-스케일링 셀에서, 그 공유 선형 메모리 전체
/// 크기를 관찰하기 위함(§6 M6의 "타입당 vs 인스턴스당 격리" 질문과 직접
/// 연결됨 — 같은 타입의 인스턴스가 늘어날수록 이 값이 늘어난다면 그 자체가
/// "공유 인스턴스" 증거).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn __loom_debug_memory_bytes() -> u32 {
    (core::arch::wasm32::memory_size(0) * 65536) as u32
}

/// RQ6 §6 M6 N-스케일링 결합 셀 전용 fault-injection — `QuantityPickerIsland`
/// 자체는 codegen이 생성하므로 손으로 `panic_now()` 메서드를 얹을 수 없어,
/// 같은 wasm 모듈/같은 선형 메모리를 공유하는 별도의 자유 함수로 노출한다.
/// 이 함수가 패닉해도 *다른* `QuantityPickerIsland` 인스턴스가 계속 응답하는지
/// 확인하는 것이 "조건 B가 타입당 격리인가 인스턴스당 격리인가"라는, §27
/// 계획이 미리 등록해 둔 질문에 대한 실측이다.
///
/// **실측 결과는 사전 예상과 반대였다**: 같은 청크를 가리키는 N개 인스턴스가
/// (오늘의 빌드 구성상) 실제로 하나의 `WebAssembly.Instance`/선형 메모리를
/// 공유함(M4가 N에 비례해 메모리가 느는 것으로 이미 확인)에도 불구하고, 이
/// 함수의 패닉 이후 *다른* 인스턴스들은 계속 정상 응답했다(아래
/// research-questions/data/rq6-m6-nscaling.json 참고). 이유: wasm의 트랩은
/// "그 트랩을 일으킨 현재 호출 스택"만 되감을 뿐, 인스턴스의 선형 메모리
/// 자체를 파괴하지 않는다 — 이 함수는 어떤 공유 상태도 건드리지 않고 즉시
/// panic하므로, 트랩 시점에 이미 손상된 상태가 하나도 없었다. **주의**: 이건
/// "타입당 격리가 인스턴스당 격리만큼 안전하다"는 뜻이 아니다 — 이 프로브가
/// 검증한 건 "이 인스턴스와 무관한 임의 위치에서 패닉이 나도 다른 인스턴스가
/// 안전한가"이지, "한 인스턴스가 *자기 자신의* 상태를 변경하던 도중(예:
/// `RefCell::borrow_mut()` 도중) panic해도 다른 인스턴스가 안전한가"가 아님
/// — 후자가 진짜 위험한 시나리오이고, 이 픽스처는 그 경로를 아직 실측하지
/// 않았다(미해결 항목으로 명시 기록, 아래 JSON의 "limitations" 참고).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn __loom_debug_panic_shared_instance() {
    panic!("loom-widget-embed: deliberate fault injection for RQ6 M6 N-scaling combined cell");
}

/// RQ6 §6 M1(번들 크기)의 H-Qwik 팔 실제 wasm 엔트리 — Task #27. `crates/
/// loom-resume-demo`의 `ResumedComponent`(비-Island 루트 컴포넌트의 기존
/// resumability 엔트리 포인트, Phase 2에서 이미 확립됨)와 **정확히 같은
/// 메커니즘을 재사용**한다(CLAUDE.md 2-1 — 이미 잘 풀린 문제를 다시 발명하지
/// 않음): 페이지 전역 `document.query_selector_all("[data-loom-id]")`로
/// 모든 태그된 노드를 한 번에 흡수하고, `generated_flat::resume()`으로 병합된
/// 하나의 `World`를 되살린 뒤, 이후의 모든 클릭을 `dispatch()`로 그 인스턴스에
/// 재사용한다 — Island 엔트리(`generate_island_entry_body`)와 달리 호스트
/// 스코프 탐색이 아니라 페이지 전역 탐색인 이유: H-Qwik 팔은 애초에 "경계가
/// 없다"는 것 자체가 RQ6 §8의 핵심 전제이므로, 이건 우회해야 할 버그가 아니라
/// 이 팔의 정의적 특성이다. `ResumedComponent`와의 유일한 실질적 차이는
/// `has_shared=true`라서 `resume`/`apply_action`이 `JsSharedStore` 인자를
/// 하나 더 받는다는 것뿐(§4.2의 CartBadge가 `sharedValue("cartCount")`를
/// 읽으므로 flatten된 그래프에 공유 바인딩이 실제로 존재함).
///
/// **왜 손코딩인가(숨기지 않음)**: `GraphMode::Flatten`은 codegen이 이미
/// 실제로 만들어내지만(Task #26), "이 merged component를 브라우저에서
/// 어떻게 resumable하게 로드하는가"는 아직 컴파일러가 자동 생성하지 않는다
/// (`generate_islands`의 Island 엔트리 생성기처럼, 루트/Flatten 전용
/// wasm-bindgen 엔트리 생성기는 이번 라운드 범위 밖) — `loom-resume-demo`가
/// 이미 확립한 "코드젠 동형 손코딩" 관례를 그대로 적용해, 컴파일러가 모르는
/// 런타임 지식 없이 기존 생성 함수(`resume`/`apply_action`/
/// `resume_action_by_symbol`)만 호출한다.
#[cfg(target_arch = "wasm32")]
mod resumed_flat_app {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    pub struct ResumedFlatApp {
        app: ::loom_runtime::LoomApp,
        dom: ::loom_runtime::web::BrowserDom,
        signals: super::generated_flat::Signals,
        shared: ::loom_runtime::web::JsSharedStore,
    }

    #[wasm_bindgen]
    impl ResumedFlatApp {
        /// `captures_str`는 `ResumedComponent::new`와 동일한 포맷(`"count:0"`
        /// 류) — 최초 클릭의 LRef가 실어 나른 그 액션 자신의 캡처. 병합된
        /// 그래프의 다른 시그널들은 DOM adoption으로 복구되므로(List/Bool과
        /// 동일한 논리), 어느 위젯의 첫 클릭에서 생성되든 전체 World가
        /// 올바르게 복구된다 — 이게 바로 "경계가 없다"의 실질적 의미.
        #[wasm_bindgen(constructor)]
        pub fn new(captures_str: &str) -> ResumedFlatApp {
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

            let mut app = ::loom_runtime::LoomApp::new();
            let mut dom = ::loom_runtime::web::BrowserDom::new_for_resume(document.clone());

            let tagged = document
                .query_selector_all("[data-loom-id]")
                .expect("query_selector_all failed");
            let mut by_name: std::collections::HashMap<String, ::loom_runtime::NodeHandle> =
                std::collections::HashMap::new();
            for i in 0..tagged.length() {
                let node = tagged.get(i).expect("NodeList index out of bounds");
                let el: web_sys::Element = wasm_bindgen::JsCast::unchecked_into(node.clone());
                let name = el.get_attribute("data-loom-id").expect("just matched the selector");
                let handle = dom.adopt(node);
                by_name.insert(name, handle);
            }
            let find_node = move |name: &str| -> ::loom_runtime::NodeHandle {
                *by_name
                    .get(name)
                    .unwrap_or_else(|| panic!("loom resume: no element tagged data-loom-id=\"{name}\""))
            };

            let mut shared = ::loom_runtime::web::JsSharedStore;
            let signals = super::generated_flat::resume(&mut app, &mut dom, find_node, &capture_refs, &mut shared);
            ResumedFlatApp { app, dom, signals, shared }
        }

        pub fn dispatch(&mut self, symbol: &str, value: Option<String>) {
            let action = super::generated_flat::resume_action_by_symbol(symbol, value.as_deref().unwrap_or(""));
            super::generated_flat::apply_action(&mut self.app, &self.signals, &action, &mut self.shared);
            self.app.tick(&mut self.dom);
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use resumed_flat_app::ResumedFlatApp;

// A minimal native-only DomOps impl for producing this page's initial SSR
// HTML — see crates/loom-island-compiled's identical copy for why this
// isn't shared as a dependency (established one-crate-one-StringDom
// convention in this repo).
#[cfg(not(target_arch = "wasm32"))]
mod string_dom {
    use loom_runtime::{DomOps, NodeHandle};

    #[derive(Default)]
    pub struct StringDom {
        tags: Vec<String>,
        attrs: Vec<Vec<(String, String)>>,
        text: Vec<Option<String>>,
        children: Vec<Vec<usize>>,
        visible: std::collections::HashMap<u32, bool>,
    }

    const TEXT_NODE_TAG: &str = "#text-wrapper";
    const RAW_HTML_TAG: &str = "#raw-html";

    fn html_escape(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
    }

    impl StringDom {
        pub fn new_root(root_tag: &str) -> (Self, NodeHandle) {
            let mut dom = StringDom::default();
            let root = dom.alloc(root_tag);
            (dom, root)
        }

        fn alloc(&mut self, tag: &str) -> NodeHandle {
            let idx = self.tags.len();
            self.tags.push(tag.to_string());
            self.attrs.push(Vec::new());
            self.text.push(None);
            self.children.push(Vec::new());
            NodeHandle(idx as u32)
        }

        pub fn render_html(&self, root: NodeHandle) -> String {
            let mut out = String::new();
            self.render_node(root.0 as usize, &mut out);
            out
        }

        fn render_node(&self, idx: usize, out: &mut String) {
            let tag = &self.tags[idx];
            if tag == TEXT_NODE_TAG {
                if let Some(text) = &self.text[idx] {
                    out.push_str(&html_escape(text));
                }
                return;
            }
            if tag == RAW_HTML_TAG {
                if let Some(html) = &self.text[idx] {
                    out.push_str(html);
                }
                return;
            }
            out.push('<');
            out.push_str(tag);
            for (name, value) in &self.attrs[idx] {
                out.push(' ');
                out.push_str(name);
                out.push_str("=\"");
                out.push_str(&html_escape(value));
                out.push('"');
            }
            out.push('>');
            if let Some(text) = &self.text[idx] {
                out.push_str(&html_escape(text));
            }
            for &child in &self.children[idx] {
                self.render_node(child, out);
            }
            out.push_str("</");
            out.push_str(tag);
            out.push('>');
        }
    }

    impl DomOps for StringDom {
        fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
            let handle = self.alloc(tag);
            if let Some(class) = class {
                self.set_attribute(handle, "class", class);
            }
            self.children[parent.0 as usize].push(handle.0 as usize);
            handle
        }

        fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
            let handle = self.alloc(TEXT_NODE_TAG);
            self.text[handle.0 as usize] = Some(text.to_string());
            self.children[parent.0 as usize].push(handle.0 as usize);
        }

        fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
            let handle = self.alloc(RAW_HTML_TAG);
            self.text[handle.0 as usize] = Some(html.to_string());
            self.children[parent.0 as usize].push(handle.0 as usize);
        }

        fn set_text_content(&mut self, target: NodeHandle, text: &str) {
            self.text[target.0 as usize] = Some(text.to_string());
        }

        fn clear_children(&mut self, parent: NodeHandle) {
            self.children[parent.0 as usize].clear();
        }

        fn remove_node(&mut self, _node: NodeHandle) {
            unimplemented!("StringDom is SSR-only — nothing is removed during a one-shot initial render")
        }

        fn reposition(&mut self, _parent: NodeHandle, _node: NodeHandle, _before: Option<NodeHandle>) {
            unimplemented!("StringDom is SSR-only — nothing is reordered during a one-shot initial render")
        }

        fn set_visible(&mut self, node: NodeHandle, visible: bool) {
            self.visible.insert(node.0, visible);
        }

        fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
            self.attrs[node.0 as usize].push((name.to_string(), value.to_string()));
        }

        fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
            self.children[parent.0 as usize]
                .iter()
                .map(|&idx| (NodeHandle(idx as u32), self.text[idx].clone().unwrap_or_default()))
                .collect()
        }

        fn set_input_value(&mut self, node: NodeHandle, value: &str) {
            let idx = node.0 as usize;
            if let Some(entry) = self.attrs[idx].iter_mut().find(|(k, _)| k == "value") {
                entry.1 = value.to_string();
            } else {
                self.attrs[idx].push(("value".to_string(), value.to_string()));
            }
        }

        fn get_input_value(&mut self, node: NodeHandle) -> String {
            let idx = node.0 as usize;
            self.attrs[idx]
                .iter()
                .find(|(k, _)| k == "value")
                .map(|(_, v)| v.clone())
                .unwrap_or_default()
        }
    }
}

/// `App`'s initial SSR HTML (`GraphMode::Islands`) — App itself authors no
/// `sharedSignal(...)` (only `QuantityPicker`, nested, does), so App's own
/// `setup()` keeps its plain 3-arg signature — no shared-store stub needed
/// at this top level (see `codegen.rs`'s `ComponentRef` arm for where a
/// throwaway one gets threaded in instead, one level down).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_html() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    dom.render_html(root)
}

/// The H-Qwik counterfactual's SSR output (`GraphMode::Flatten`) — since
/// flattening merges `QuantityPicker`'s `sharedSignal` binding straight
/// into the one merged component, the merged `setup()` DOES need a
/// shared-store argument here (unlike `render_html()` above) — a plain
/// native stub is enough (SSR never reads back what it publishes).
#[cfg(not(target_arch = "wasm32"))]
pub fn render_html_flat() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let mut shared = loom_runtime::InMemorySharedStore::default();
    let (_signals, _actions, _input_actions) = generated_flat::setup(&mut app, &mut dom, root, &mut shared);
    app.tick(&mut dom);
    dom.render_html(root)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn render_html_tags_all_four_islands_correctly() {
        let html = render_html();
        assert!(html.contains(r#"data-loom-island-id="product-card""#), "{html}");
        assert!(html.contains(r#"data-loom-activation="visible""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="quantity-picker""#), "{html}");
        assert!(html.contains(r#"data-loom-activation="load""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="checkout-form""#), "{html}");
        assert!(html.contains(r#"data-loom-island-id="mini-dashboard""#), "{html}");
        // CartBadge: no chunk of its own, just a plain subscriber marker.
        assert!(html.contains(r#"data-loom-shared-subscriber="cartCount""#), "{html}");
        assert!(!html.contains(r#"data-loom-chunk="app""#), "{html}"); // App itself is never an Island
    }

    #[test]
    fn render_html_matches_render_html_flat_on_shared_reference_content() {
        // M8-style consistency check: both arms show the same reference
        // content (same static text, same data-loom-id markers, same
        // initial values) — only the Island boundary differs.
        let islands = render_html();
        let flat = render_html_flat();
        for needle in [
            r#"data-loom-shared-subscriber="cartCount""#,
            "show details",
            "toggle express shipping",
            "toggle gift wrap",
            "submit order",
            "add tag",
            "news",
            "sale",
            "+1 visit",
        ] {
            assert!(islands.contains(needle), "islands missing {needle:?}: {islands}");
            assert!(flat.contains(needle), "flat missing {needle:?}: {flat}");
        }
        // The Island boundary itself is the one thing that must differ.
        assert!(islands.contains("data-loom-island"));
        assert!(!flat.contains("data-loom-island"));
    }
}
