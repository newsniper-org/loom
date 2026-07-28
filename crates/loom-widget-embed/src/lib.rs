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
#[cfg(any(target_arch = "wasm32"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn __loom_debug_memory_bytes() -> u32 {
    (core::arch::wasm32::memory_size(0) * 65536) as u32
}

#[cfg(any(target_arch = "wasm64"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn __loom_debug_memory_bytes() -> u32 {
    (core::arch::wasm64::memory_size(0) * 65536) as u32
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
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn __loom_debug_panic_shared_instance() {
    panic!("loom-widget-embed: deliberate fault injection for RQ6 M6 N-scaling combined cell");
}

/// RQ6 §6 M1(번들 크기)의 H-Qwik 팔 실제 wasm 엔트리 — Task #27에서는
/// 손코딩(`ResumedFlatApp`, `loom-resume-demo`의 `ResumedComponent` 패턴 재사용)
/// 했었으나, **2026-07-28(RQ6 §7 판정 이후, "실제로 쓸 수 있는 프레임워크"로
/// 가는 첫 단계) 컴파일러가 직접 생성하도록 승격**됐다 — `codegen.rs`의
/// `generate_flatten_entry_body`가 `GraphMode::Flatten`을 컴파일할 때마다 자동으로
/// `generated_flat::ResumedApp`을 만든다(Island 엔트리 생성기와 동형 패턴, 상세는
/// 그 함수의 doc comment). 이 크레이트에서 할 일은 그 이름을 재노출하는 것뿐 —
/// 손코딩된 구조체 정의는 더 이상 없다.
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub use generated_flat::ResumedApp;

// A minimal native-only DomOps impl for producing this page's initial SSR
// HTML — see crates/loom-island-compiled's identical copy for why this
// isn't shared as a dependency (established one-crate-one-StringDom
// convention in this repo).
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
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
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
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
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
pub fn render_html_flat() -> String {
    let (mut dom, root) = string_dom::StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let mut shared = loom_runtime::InMemorySharedStore::default();
    let (_signals, _actions, _input_actions) = generated_flat::setup(&mut app, &mut dom, root, &mut shared);
    app.tick(&mut dom);
    dom.render_html(root)
}

#[cfg(all(test, not(any(target_arch = "wasm32", target_arch = "wasm64"))))]
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
