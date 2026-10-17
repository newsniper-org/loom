//! Phase 2 — a real proof of docs/04's resumability claim ("resume ≠
//! hydrate": SSR emits HTML + a resumability payload, zero interactive
//! JS/WASM loads or runs before the first interaction, and only the minimal
//! captured state materializes then) for `demo/counter-list.tsx`, run through
//! **loomc's actual codegen** rather than hand-rolled entity spawns.
//!
//! **History**: the original Phase 2 kickoff (2026-07-22) hand-wrote
//! `ssr_render`/`ResumedCounter` against a single hard-coded counter, with
//! resumability codegen explicitly deferred ("wiring resumability into the
//! compiler pipeline... is deferred"). 2026-07-24: `loom-compiler/src/
//! codegen.rs` now generates the `data-loom-id`/`data-loom-lref` tagging and
//! the `resume`/`resume_action_by_symbol` functions itself (see that module's
//! doc comment), so this crate's job shrank to *driving* generated code
//! rather than reimplementing what it does. `build.rs` compiles the same
//! `demo/counter-list.tsx` `loom-demo` uses for the Phase 1 *live* path — the
//! same compiled component now serves both, which is the actual claim being
//! tested here: not just "resumability works for a hand-picked toy," but
//! "the compiler's regular output is resumable without per-component effort."
//!
//! **2026-07-24, same day**: list binding resume also implemented — `items`
//! now genuinely resumes (row values recovered from the SSR'd DOM itself via
//! `DomOps::adopt_children`, not from the LRef payload; see codegen.rs's
//! doc comment for the mechanism and its "settle tick" cost). Both `count`
//! and `items` are fully resumable now. **Still not attempted**: conditional
//! binding resume through loomc — there is no compiler surface for
//! conditionals at all yet (`lower.rs` doesn't lower `{cond && <X/>}`), so
//! `cond_resume` (below) proves the mechanism at the runtime level only,
//! the same way RQ1 Track T's B/F started before any compiler integration —
//! native-tested, not driven through a real browser/wasm-pack chunk like
//! the counter-list proof above. **RQ2 (payload format) is still open** —
//! `data-loom-lref="chunk|symbol|key:value,..."` is a *tentative* pick, not
//! RQ2's eventual answer.
//!
//! **2026-10-17 정정**: 바로 위 "Still not attempted ... there is no compiler
//! surface for conditionals at all yet" 문장은 같은 날(2026-07-24) 안에
//! 낡았다 — 이 파일 아래 `ssr_render`의 문서(History 2026-07-24)가 이미 적듯,
//! 컴파일러가 `{cond() && <X/>}`를 lowering하게 됐고(commit fbb7d65)
//! `demo/counter-list.tsx`의 `details` 조건부가 컴파일된 경로로 SSR·resume된다.
//! 조건부 안의 중첩 binding은 2026-08-07(f0eaf6f)부터 가능하고, "이미 보이는
//! 조건부를 resume해 안쪽 binding을 다시 붙이는" 경우도 컴파일된 코드로
//! `crates/loom-nested-fixture/tests/nested.rs`
//! (`resuming_a_shown_conditional_reattaches_its_nested_bindings`)가 네이티브
//! TreeDom에서 검사한다(브라우저/wasm 청크 경로는 아님). 2026-10-17 현재 RQ2는
//! 여전히 열려 있고 위 payload 포맷도 여전히 잠정이다.
//!
//! ## Two build targets sharing one crate
//! - `src/bin/ssr.rs` (native): renders the initial HTML via generated
//!   `setup()` against `StringDom` — standing in for a real backend's SSR
//!   step (Phase 4's actual concern; here just a static file, no HTTP
//!   server).
//! - This `lib.rs` (wasm32, the lazy "chunk"): exports `ResumedApp`
//!   (2026-07-28부터 `generated::resumed_app`가 자동 생성 — 이전엔
//!   `ResumedComponent`로 손코딩됐었음), loaded and instantiated only on
//!   first click (see `demo/resume/bootstrap.js`), driving generated
//!   `resume`/`resume_action_by_symbol`/`apply_action`.

pub mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

use loom_runtime::{DomOps, NodeHandle};

/// A minimal string-building `DomOps` for SSR. Unlike `BrowserDom`, there is
/// no live tree to mutate — this just needs to produce the *initial* HTML.
/// Internally tracks a real tree (so nested `create_element` calls compose
/// correctly) and renders to a string only once, on demand.
#[derive(Default)]
pub struct StringDom {
    tags: Vec<String>,
    attrs: Vec<Vec<(String, String)>>,
    text: Vec<Option<String>>,
    children: Vec<Vec<usize>>,
    /// Visibility per node, set by `set_visible` — not reflected back into
    /// `render_html` (nothing in this crate re-renders after a toggle;
    /// `cond_resume` only exercises `set_visible`'s bookkeeping, not its
    /// visual effect). Absent = visible, matching `DomOps::set_visible`'s
    /// "visible by default" contract.
    visible: std::collections::HashMap<u32, bool>,
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
            // A real text node, not an element — no wrapping tag at all
            // (this bug — literally emitting `<#text-wrapper>...` as HTML —
            // was masked as long as nothing called `append_static_text`;
            // loomc's generated `setup()` does, for every static text run,
            // so it surfaced the first time real generated code rendered
            // through `StringDom`).
            if let Some(text) = &self.text[idx] {
                out.push_str(&html_escape(text));
            }
            return;
        }
        if tag == RAW_HTML_TAG {
            // Already-rendered HTML (see `append_static_html`'s doc
            // comment on `DomOps`) — emitted verbatim, NOT re-escaped
            // (unlike `TEXT_NODE_TAG` above, which holds literal text that
            // still needs escaping).
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

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Sentinel "tag" marking a node as a real text node rather than an element
/// — `render_node` special-cases it to emit bare escaped text, no wrapping
/// tag. Never a real HTML tag name, so it can't collide with one.
const TEXT_NODE_TAG: &str = "#text-wrapper";

/// Sentinel "tag" marking a node as a pre-rendered raw HTML blob (see
/// `DomOps::append_static_html`) — `render_node` emits its stored text
/// verbatim, no escaping, no wrapping tag.
const RAW_HTML_TAG: &str = "#raw-html";

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
        unimplemented!("StringDom is SSR-only — list rows are never removed during a one-shot initial render")
    }

    fn reposition(&mut self, _parent: NodeHandle, _node: NodeHandle, _before: Option<NodeHandle>) {
        unimplemented!("StringDom is SSR-only — nothing gets reordered during a one-shot initial render")
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        self.visible.insert(node.0, visible);
    }

    /// Marks an element so client-side resumability code can find it again
    /// after the fact (`data-loom-id`) or describes its lazy handler
    /// (`data-loom-lref`) — the SSR half of loom-compiler's generated
    /// resumability tagging (see codegen.rs).
    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        self.attrs[node.0 as usize].push((name.to_string(), value.to_string()));
    }

    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        self.children[parent.0 as usize]
            .iter()
            .map(|&idx| (NodeHandle(idx as u32), self.text[idx].clone().unwrap_or_default()))
            .collect()
    }

    /// An `<input>` has no text child to set — its rendered value is the
    /// `value` *attribute* (see `set_attribute`'s doc comment: `create_element`
    /// + `set_attribute("value", ...)` is exactly how codegen's `setup()`
    /// sets an input's initial value). Overwrites the existing entry rather
    /// than pushing a duplicate — `setup()`'s own settle tick (see
    /// codegen.rs's module doc) calls this once, redundantly, right after
    /// `set_attribute` already set the same initial value, so pushing
    /// unconditionally would render `<input value="0" value="0">`.
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

/// Runs the compiler-generated `setup()` against `StringDom` and ticks once
/// — this is what makes the output correct SSR (the `count` span already
/// reads "0") rather than an empty shell waiting for client JS to fill it
/// in. Returns the rendered HTML.
pub fn ssr_render() -> String {
    let (mut dom, root) = StringDom::new_root("div");
    let mut app = loom_runtime::LoomApp::new();
    let (_signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, root);
    app.tick(&mut dom);
    dom.render_html(root)
}

/// **History (2026-07-24)**: this crate used to carry a hand-wired
/// `ssr_render_full_app`/`ResumedCond` pair reproducing RQ6 §4.1's "전체-앱"
/// fixture's nested conditional directly against `cond_reconcile_f_system`
/// (loomc had no conditional authoring surface yet). `loom-compiler/src/
/// codegen.rs` now lowers `{cond() && <X/>}` itself (a `details` Bool signal
/// + toggle button were added to `demo/counter-list.tsx`), so `ssr_render`/
/// `ResumedApp` below already cover this fixture's simple case — the
/// hand-wired pair was deleted rather than kept alongside a now-redundant
/// duplicate. **Form input is still not included** — `loom-runtime` has no
/// two-way form-value binding yet (not a corner cut for this fixture
/// specifically; the capability doesn't exist anywhere in this codebase) —
/// stated plainly in `research-questions/RQ6-verification-plan.md`'s status
/// section, not silently substituted with something else.
///
/// **2026-10-17 정정**: 위 "Form input is still not included — `loom-runtime`
/// has no two-way form-value binding yet"는 낡았다. `<input value={s()}
/// onInput={...}/>` 양방향 binding은 2026-07-24(commit fbb7d65 "컴파일러 표면
/// 확장(조건부/폼 입력)")에 들어갔다 — loom-runtime의 `InputValueTarget`/
/// `patch_input_system`/`DomOps::set_input_value`·`get_input_value`, IR의 input
/// binding, 그리고 `demo/counter-list.tsx`의 `name` 입력이 그것이고, 위
/// `ssr_render`가 받아 버리는 세 번째 반환값(`_input_actions`)이 그 action 표다.
/// (이 문장과 그 구현은 같은 커밋 fbb7d65에 함께 들어갔다 — 커밋 안에서 문서가
/// 구현을 따라가지 못한 것으로 보인다. 아래 "static-only" 문장도 같은 커밋.)
///
/// Conditional (RQ1 Track T candidate F) resume with **existing internal
/// state** — a **runtime-only** proof, hand-written like the original
/// counter kickoff was, because compiled conditionals are static-only
/// (`JsxNode::CondBinding`'s `then_branch` may not itself declare a nested
/// signal — see `ir.rs`'s doc comment), so there is still no compiler
/// surface for *this* harder case. Proves the same property `codegen.rs`'s
/// scalar/list resume does — a fresh `LoomApp` can pick up exactly where SSR
/// left off, using only a captured value and the DOM SSR already rendered —
/// for F specifically: does the child's internal state (which F's whole
/// reason to exist is to *not* reset on toggle) survive being captured,
/// handed to a brand-new `World`, and then actually toggled off and back on
/// there. Unlike `demo/counter-list.tsx`'s compiled conditional (which
/// starts hidden and empty, so its resume path needs no captured child
/// state at all), this proof specifically exercises the harder
/// "already-shown-with-existing-state" resume case.
///
/// **2026-10-17 정정**: "compiled conditionals are static-only" 이유는
/// 2026-08-07(f0eaf6f)에 해소됐다 — `then_branch` 안에 `{signal()}`/
/// `{signal().map(...)}`를 넣을 수 있다(ir.rs `CondBinding` 문서). 다만 그
/// signal들은 컴포넌트 최상위에 선언되고 entity는 `setup()`이 항상 즉시
/// 만든다(codegen.rs "중첩 (2026-08-07)" 절) — "조건부 자식이 자기 내부 상태를
/// 소유한다"는 이 모듈의 모양(F의 `CondBlock` 자식마다 `Signal<i32>`)과 같지는
/// 않다. 컴파일된 경로의 "보이는 조건부 resume"은 loom-nested-fixture가 네이티브로
/// 검사하고, 이 모듈은 F 런타임 System 자체의 상태 보존 증명으로 그대로 남는다.
pub mod cond_resume {
    use super::StringDom;
    use loom_runtime::{
        cond_reconcile_f_system, patch_text_system, CondBlock, DomOps, DomTarget, LoomApp, NodeHandle, Signal,
    };

    /// SSR half: renders a `CondBlock` (F variant, one child) already shown,
    /// with its internal counter at `child_counter` — standing in for "this
    /// session already interacted before this snapshot was taken" (docs/04's
    /// dormant/pause-resume case, not modeled further here — out of scope,
    /// same as the scalar demo treating its captured value as already-known
    /// rather than deriving *when* it became that value).
    pub fn ssr_render_cond(child_counter: i32) -> (StringDom, NodeHandle, String) {
        let (mut dom, root) = StringDom::new_root("div");
        let mut app = LoomApp::new();
        app.add_systems(cond_reconcile_f_system);
        app.add_systems(patch_text_system::<i32>);
        let block_entity = app.world.spawn((Signal(true), CondBlock::with_size(1), DomTarget(root))).id();
        app.tick(&mut dom); // mounts the child, initial counter 0
        let child = app.world.get::<CondBlock>(block_entity).unwrap().children[0];
        app.world.get_mut::<Signal<i32>>(child).unwrap().0 = child_counter;
        app.tick(&mut dom); // patches the rendered text to child_counter
        dom.set_attribute(root, "data-loom-id", "cond_wrapper");
        let html = dom.render_html(root);
        (dom, root, html)
    }

    /// Resume half: takes the **same DOM** SSR built (standing in for a
    /// browser that already parsed the SSR'd HTML — the point being proven
    /// is specifically "don't recreate what's already there") plus the
    /// captured counter value, and wires a **brand-new** `LoomApp`/`World`
    /// (standing in for a fresh process — no shared Rust state with
    /// `ssr_render_cond`'s `app`) to it via `adopt_children`, the same
    /// mechanism `codegen.rs`'s generated list resume uses.
    pub fn resume_cond(dom: &mut StringDom, wrapper: NodeHandle, captured_counter: i32) -> (LoomApp, bevy_ecs::entity::Entity) {
        let mut app = LoomApp::new();
        app.add_systems(cond_reconcile_f_system);
        app.add_systems(patch_text_system::<i32>);

        let adopted = dom.adopt_children(wrapper);
        assert_eq!(adopted.len(), 1, "this proof only covers CondBlock::with_size(1)");
        let (child_handle, _current_text) = adopted[0].clone();
        let child = app.world.spawn((Signal(captured_counter), DomTarget(child_handle))).id();

        let mut block = CondBlock::with_size(1);
        block.children.push(child);
        let block_entity = app.world.spawn((Signal(true), block, DomTarget(wrapper))).id();

        // Same "settle tick" reasoning as codegen.rs's generated `resume()`:
        // `cond_reconcile_f_system` would otherwise fire on the *next*
        // unrelated tick (Added-counts-as-Changed) and re-run its (harmless
        // here, since F never despawns) toggle logic against a stale
        // `cond.0` read. Settling here keeps that inside first-interaction
        // materialization, not a surprise on some later action.
        app.tick(dom);
        (app, block_entity)
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use loom_runtime::Signal as Sig;

        #[test]
        fn ssr_renders_the_captured_counter_value() {
            let (_dom, _root, html) = ssr_render_cond(7);
            assert!(html.contains(r#"data-loom-id="cond_wrapper""#));
            assert!(html.contains(">7<"), "SSR output must already show the captured value, not 0");
        }

        #[test]
        fn resume_preserves_state_across_a_toggle_off_then_on() {
            let (mut dom, root, _html) = ssr_render_cond(7);

            // Cross the "resume boundary": same DOM, brand-new World/App.
            let (mut app, block_entity) = resume_cond(&mut dom, root, 7);
            let child = app.world.get::<CondBlock>(block_entity).unwrap().children[0];
            assert_eq!(app.world.get::<Sig<i32>>(child).unwrap().0, 7, "resume must restore the captured value");

            // Toggle off, then on -- F's entire reason to exist over B.
            app.world.get_mut::<Sig<bool>>(block_entity).unwrap().0 = false;
            app.tick(&mut dom);
            assert_eq!(
                app.world.get::<CondBlock>(block_entity).unwrap().children[0],
                child,
                "F must not despawn on hide, even for a resumed block"
            );
            app.world.get_mut::<Sig<bool>>(block_entity).unwrap().0 = true;
            app.tick(&mut dom);
            assert_eq!(
                app.world.get::<Sig<i32>>(child).unwrap().0,
                7,
                "the resumed child's state must survive a hide/show cycle, same as the live-path guarantee"
            );
        }
    }
}

// ============================================================================
// The lazy chunk (wasm32 target only, loaded on first click — see
// demo/resume/bootstrap.js)
// ============================================================================

/// 2026-07-28(RQ6 §7 판정 이후)까지는 여기 `ResumedComponent`가 손코딩되어
/// 있었다("actor+Entity materialize" 로직 — SSR 노드에 직접 결선, "resume ≠
/// hydrate"의 실증). 이제 `generated`(build.rs가 `GraphMode::Flatten`으로
/// 전환됨) 모듈이 `ResumedApp`이라는 이름으로 정확히 같은 로직을 자동 생성한다
/// (`codegen.rs`의 `generate_flatten_entry_body`) — 손코딩을 지우고 재노출만
/// 남긴다. `demo/resume/bootstrap.js`도 이 이름 변경에 맞춰 갱신됨.
#[cfg(any(target_arch = "wasm32", target_arch = "wasm64"))]
pub use generated::ResumedApp;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssr_render_produces_correct_static_shell_and_lref_payload() {
        let html = ssr_render();
        assert!(html.contains("count:"), "static label text must be present");
        assert!(
            html.contains(r#"data-loom-id="count""#),
            "value span must be tagged for client-side resume targeting"
        );
        assert!(
            html.contains(r#"data-loom-lref="App_chunk|Action0|count:0""#),
            "the count-incrementing button must carry the compiler-generated LRef payload \
             with the captured initial value — App/Action0 because that's what \
             loomc named this component/action, not a hand-picked symbol"
        );
        // The value span's *rendered* text must already be "0" — this is
        // real SSR output (a completed tick), not an empty shell waiting
        // for client JS to fill in the first value.
        assert!(html.contains(r#"data-loom-id="count">0<"#));
        // The list's rows must also already be server-rendered (setup()
        // spawns+ticks the list binding same as the scalar one).
        assert!(html.contains("<li>a</li><li>b</li><li>c</li>"));
        // The list's own container is tagged too — resume finds it via
        // data-loom-id and adopts its rows directly (see codegen.rs's
        // `resume()` doc comment), not via an LRef capture.
        assert!(
            html.contains(r#"data-loom-id="items""#),
            "the list's own <ul> must be tagged so resume can adopt its existing rows"
        );
        // Append/Rotate get LRefs too (list resume no longer skips them) —
        // but with *empty* captures, not count:0 — docs/02's minimal-capture
        // computation narrows each action's LRef to only the signal it
        // itself reads/writes (see codegen.rs's `format_i32_capture_for_action`),
        // and neither Append nor Rotate touches `count`.
        assert!(
            html.contains(r#"data-loom-lref="App_chunk|Action1|""#),
            "Append must carry an LRef with no captures — it doesn't touch count, \
             and list resume recovers `items` from the DOM instead"
        );
        assert!(
            html.contains(r#"data-loom-lref="App_chunk|Action2|""#),
            "Rotate must carry an LRef with no captures, for the same reason as Append"
        );
    }

    #[test]
    fn compiled_conditional_starts_collapsed_and_the_toggle_gets_a_normal_lref() {
        let html = ssr_render();
        // Counter/list still present — the conditional is one more binding
        // site in the same `generated::setup()`, not a separate fixture.
        assert!(html.contains(r#"data-loom-id="count">0<"#));
        assert!(html.contains("<li>a</li><li>b</li><li>c</li>"));
        // The conditional starts hidden — no child markup should exist
        // inside the wrapper yet (F only lazy-spawns on first `true`).
        assert!(
            html.contains(r#"<p class="details" data-loom-id="details"></p>"#),
            "the conditional wrapper must be tagged and empty while hidden, got: {html}"
        );
        // The toggle button is `Action3`, routed through the same generic
        // `ResumedApp`/`resume_action_by_symbol` path as every other
        // action — no hand-wired symbol or resumed class needed anymore.
        assert!(
            html.contains(r#"data-loom-lref="App_chunk|Action3|""#),
            "the toggle button must carry a compiler-generated LRef with no captures \
             (Bool signals are DOM-recovered, not capture-fallback-dependent)"
        );
    }
}
