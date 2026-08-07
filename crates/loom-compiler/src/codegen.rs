//! `ir::Component` → generated Rust source that calls straight into
//! `loom_runtime` (docs/02 "Entity 그래프 생성" + "Patch System 생성").
//!
//! Output shape: a `setup(app, dom, mount) -> (Signals, Vec<(NodeHandle,
//! Action)>)` function that spawns the entity graph once (static markup is
//! created and never revisited; dynamic binding sites get a `Signal<T>` +
//! `DomTarget` and register the matching generic patch System), plus an
//! `apply_action` function the host glue calls on a real DOM click event.
//! Event wiring itself (attaching a real `web_sys` listener) is NOT
//! generated — ordinary host-side Rust code owns that (see
//! loom-demo/src/lib.rs, which drives `setup`/`apply_action` for Phase 1's
//! live, non-resumable path).
//!
//! ## Phase 2 resumability codegen (2026-07-24)
//! `setup()` additionally tags the DOM it builds: every `TextBinding`'s
//! target element gets `data-loom-id="<signal>"` (a resume-time lookup key),
//! every `ListBinding`'s own container element gets the same (so its
//! existing rows can be found and adopted, see below), and **every**
//! action-bearing element gets `data-loom-lref="<component>_chunk|
//! <action-variant>|<captures>"` — the tentative RQ2 payload shape the
//! hand-written Phase 2 kickoff proof used (`crates/loom-resume-demo`), now
//! emitted generically for any compiled component's actions, not hand-rolled
//! per component.
//!
//! ## Minimal LRef captures (2026-07-24 — docs/02's "LRef 최소 캡처 집합")
//! `captures` is **not** every `i32` signal in the component — it's exactly
//! the one signal the tagged action itself reads/writes (empty for
//! `StringList` actions, which need no capture at all — see the List resume
//! section below), computed by `format_i32_capture_for_action`. This is
//! provably complete for this IR (every `ClickAction` variant names exactly
//! one signal, and `apply_action`'s generated match arm for it touches only
//! that signal) — see that function's doc comment for the exact reasoning
//! and an important scope caveat (the narrowing's safety currently depends
//! on Phase 1 never allowing a signal's true value to diverge from its
//! literal declared initial; revisit if that ever changes).
//!
//! ## List resume (2026-07-24) — row values come from the DOM, not the LRef
//! `StringList` signals don't need a capture entry: `resume()` calls
//! `DomOps::adopt_children` on the list's own SSR'd container to recover
//! each row's *current* value directly from the DOM that's already showing
//! it correctly, spawns one entity per adopted row wired to its existing
//! node, and registers `list_reconcile_k_system::<String>` so subsequent
//! Append/Rotate actions patch it like any live Phase 1 list. This keeps
//! the LRef payload O(1) regardless of list length — no per-row capture
//! encoding needed. See `DomOps::adopt_children`'s doc comment for why this
//! reads the DOM instead of serializing row content.
//!
//! **First-tick settle cost (documented, not hidden)**: every signal
//! `resume()` just spawned looks "Changed" to a patch/reconcile System that
//! has never run before (Bevy's Added-counts-as-Changed). `resume()` calls
//! `app.tick(dom)` once, internally, before returning — this means the list
//! rows it just carefully adopted get immediately despawned and respawned
//! fresh (`list_reconcile_k_system`'s unconditional despawn+respawn shape),
//! and the scalar span gets a redundant same-value `set_text_content`. Both
//! are harmless *correctness*-wise but real, scale-dependent *cost*: without
//! this settle tick, an unrelated later action (e.g. clicking "+1") would
//! trigger this same rebuild as a surprising side effect, since the list's
//! `Signal<Vec<String>>` would still look "never-yet-observed-changed" to
//! `list_reconcile_k_system`. Paying the cost once, synchronously, as part
//! of first-interaction chunk materialization is consistent with
//! resumability's "zero JS before interaction, cost paid at interaction"
//! claim — but it does mean list rows aren't truly left untouched at
//! resume time, which is worth narrowing later (e.g. skipping the
//! `list_reconcile_k_system` run when the just-recovered rows already match
//! `items`), not attempted here.
//!
//! `resume_action_by_symbol` + `resume` are the client-chunk-side generated
//! functions: given the LRef's parsed captures, `resume` rebuilds just
//! enough `World` state for `apply_action` to run unmodified against it —
//! the same function Phase 1's live path already uses, now also correct for
//! a resumed World.
//!
//! ## Conditionals (2026-07-24) — per-site function, not a parameterized System
//! `{cond() && <tag>...}` (`JsxNode::CondBinding`) reuses RQ1 Track T
//! candidate F's *algorithm* (lazy-spawn-then-hide, `cond.rs`'s
//! `cond_reconcile_f_system`) without reusing that System itself: F's
//! benchmark variant hardcodes its spawn content (a fixed span, standing in
//! for "whatever a conditional subtree contains"), but a real compiled site
//! needs its own actual static content spawned. So `lower_node`'s
//! `CondBinding` arm generates a **fresh top-level function per site**
//! (`cond_reconcile_site_N`) replicating F's control flow with that site's
//! real `StaticText` children inlined as `DomWrite::AppendStaticText` calls
//! — registered via `app.add_systems` in both `setup()` and `resume()`.
//! Resume recovers shown/hidden from the DOM (`adopt_children` on the
//! wrapper — empty means never shown, matching `SignalInit::Bool`'s doc
//! comment on `generate_resume` for why that's a *complete* read of every
//! state this pipeline's SSR can actually produce, not just today's
//! observed cases).
//!
//! ### 중첩 (2026-08-07) — 사이트별 특수화를 한 겹 더
//! 2026-08-07까지 조건부 내용은 **정적 텍스트만** 허용됐다(lower.rs가 거부).
//! 그 제약은 RQ1 §5 트랙 N("리스트 > 조건부 > 리스트" property test)을
//! 원천 봉쇄하고 있었다 — §2.3-1이 "컴파일러가 생성할 수 없는 형태의 코드
//! 금지"라 픽스처 손코딩으로 우회할 수도 없었기 때문이다. 이제 조건부 안에
//! 정적 마크업 / `{signal()}` / `{signal().map(...)}` / 그것들을 감싸는
//! 엘리먼트를 넣을 수 있다.
//!
//! 설계의 핵심은 **entity와 노드의 수명을 분리**한 것이다:
//!
//! - 중첩 binding의 `Entity`는 `setup()`이 **항상 즉시** 만든다. 그래야
//!   `Signals` 구조체·action·`resume()`이 "조건부 안이냐 밖이냐"를 따지지
//!   않는다.
//! - 다만 `DomTarget`은 붙이지 않는다. patch System들의 쿼리가 `&DomTarget`을
//!   요구하므로, DOM에 없는 동안은 **archetype 수준에서**(런타임 분기 없이)
//!   자동으로 제외된다 — Bevy의 접근 선언 기반 스케줄링 재사용(CLAUDE.md 2-1).
//! - 조건부가 처음 `true`가 될 때 `DomWrite::SpawnElement`가 노드를 만들고
//!   `DomTarget`을 붙인다. 그 순간부터는 평범한 binding이다.
//!
//! 사이트 함수가 안쪽 entity를 찾는 방법은 `CondNested`(entity 벡터) +
//! **컴파일타임에 고정된 리터럴 인덱스**뿐이다. 이름 조회도 트리 순회도
//! 런타임에 남지 않는다.
//!
//! 초기값을 `SpawnElement`/초기 row로 **구워서** 보내는 것은 편의가 아니라
//! 필수다: 이 entity의 `DomTarget`은 그 write가 적용될 때 붙으므로, 그
//! 전에 지나간 `Changed` 틱은 어떤 patch System도 관측할 수 없다. 제네릭
//! `list_reconcile_k_system`에 초기 렌더를 맡기면 안쪽 리스트가 **영영
//! 그려지지 않는다**(`crates/loom-nested-fixture`의 음성 대조로 확인됨).
//!
//! 아직 못 받는 것(숨기지 않고 적어 둠): 조건부 안의 또 다른 조건부,
//! `<input>` 양방향 binding, 지연 노드의 자체 `onClick`, `<Component/>`
//! 참조, `sharedValue(...)`. 그리고 **리스트 row 템플릿 안의 조건부** —
//! 트랙 N이 요구하는 바깥쪽 중첩이며, row 템플릿을 사이트별로
//! monomorphize하는 별도 작업이다.
//!
//! ## `<input>` two-way binding (2026-07-24)
//! `<input value={s()} onInput={(e) => s.set(e.target.value)}/>`
//! (`JsxNode::InputBinding`) synthesizes a `ClickAction::SetString` action
//! (see `collect_actions`) so it rides the *same* `Action`/`apply_action`/
//! `resume_action_by_symbol` machinery every onClick action already uses,
//! rather than a parallel dispatch mechanism — `setup()`'s third return
//! value, `Vec<(NodeHandle, fn(String) -> Action)>`, is how the live path's
//! host glue (loom-demo) wires a real `input` DOM listener per site: each
//! entry's `fn(String) -> Action` is literally the bare tuple-variant
//! constructor (`Action::ActionN`), not a closure. Resume recovers the
//! *live* value via `DomOps::get_input_value` — captures are structurally
//! impossible here (at SSR time nothing has been typed yet), so this is the
//! one case where DOM-recovery isn't just cheaper than a capture but the
//! only way to get a correct answer at all. `SetString` is also the one
//! action whose payload isn't determined by its symbol alone: unlike every
//! other action, `resume_action_by_symbol` takes a `string_arg` — every
//! other action ignores it, `SetString` uses it (see that function's doc
//! comment).

use crate::ir::{
    ClickAction, Component, ComponentGraph, IslandActivation, JsxNode, SharedBinding, SignalDecl, SignalInit,
};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Which physical Island chunk/constructor a compiled unit corresponds to —
/// derived deterministically from the component's own name. **Known,
/// documented gap (Task #25 scope cut)**: multiple simultaneous instances of
/// the *same* authored component on one page would collide on these derived
/// values (same `island_id`/`chunk`/`ctor` for each) — not solved this
/// round.
struct IslandTag {
    island_id: String,
    chunk: String,
    ctor: String,
    activation: IslandActivation,
}

/// Two ways a `ComponentGraph` can be compiled — see each variant's doc
/// comment.
pub enum GraphMode {
    /// Every Island-activation-bearing component gets its own namespaced
    /// `pub mod <name>_generated { ... }` plus its own generated Island
    /// entry point (`generate_island_entry_body`). The graph's root is
    /// emitted unqualified, exactly as today's `generate()` does —
    /// **required** for backward compatibility with `loom-demo`/`loom-fic`'s
    /// existing `mod generated { include!(...) }` call sites, both of which
    /// still go through the older `compile_tsx_to_rust`/`generate()` path
    /// (single-component, no `ComponentGraph`), not this enum at all — the
    /// unqualified-root guarantee just keeps their output format compatible
    /// if they were ever moved onto it. (2026-07-28: `loom-resume-demo` used
    /// to be a third crate covered by this note, back when it also emitted
    /// through `Islands`; it has since moved to `GraphMode::Flatten` — see
    /// `generate_flatten_entry_body`'s doc comment — so it no longer applies
    /// here.)
    Islands,
    /// RQ6 §8's H-Qwik counterfactual target shape — every component in the
    /// graph, Island-activated or not, becomes part of ONE shared
    /// `LoomApp`/`World`/`Signals`/`Action`, no per-component module
    /// boundary, no Island host attributes, one LRef payload. Makes Task
    /// #26's "정체성 통제" fixture derivable from the *same* JSX source that
    /// produces `Islands` mode's output, differing only in which
    /// `GraphMode` compiles it — see `flatten_graph`'s doc comment for the
    /// implementation approach and its one stated scope cut.
    Flatten,
}

pub fn generate_graph(graph: &ComponentGraph, mode: GraphMode) -> String {
    match mode {
        GraphMode::Islands => generate_islands(graph),
        GraphMode::Flatten => {
            let flattened = flatten_graph(graph);
            let has_shared = !flattened.shared_bindings.is_empty();
            // No `ComponentRef` nodes survive `flatten_graph` (all inlined),
            // so `lower_node`'s `ComponentRef` arm never runs for this mode
            // — the shared-component-names set is irrelevant here, but
            // still required by `generate_component`'s signature.
            let mut out = generate_component(&flattened, None, has_shared, &HashSet::new());
            out.push_str(&generate_flatten_entry_body(has_shared));
            out
        }
    }
}

/// RQ6 §7 판정(H-Qwik 확정, 2026-07-28) 이후 실제로 필요해진 것 — `GraphMode::
/// Flatten`이 컴파일한 그래프를 브라우저에서 resumable하게 로드하는 wasm-bindgen
/// 진입점을, `generate_island_entry_body`가 Island 엔트리를 만드는 것과 똑같이
/// **컴파일러가 직접 생성**한다. 이전까지는 이 진입점이 크레이트마다(`loom-resume-
/// demo`의 `ResumedComponent`, `loom-widget-embed`의 `ResumedFlatApp`,
/// `loom-largeapp-axis-flat`의 `ResumedLargeFlatApp`) 손코딩으로 세 번 복붙됐다
/// — 셋 다 `has_shared` 유무만 다른 완전히 같은 패턴이었다(코드 리뷰로 확인). RQ6이
/// H-Qwik을 확정한 이상 "임의의 .tsx를 컴파일러에 넣으면 동작하는 앱이 나온다"는
/// 최소 성립 조건에 이 손코딩이 정면으로 걸렸으므로, 세 크레이트를 이 함수 하나로
/// 흡수한다(구조체 이름을 `ResumedApp`으로 통일 — 더 이상 크레이트마다 다른 이름을
/// 붙일 이유가 없음, 픽스처가 아니라 컴파일러 산출물이므로).
///
/// **Island 엔트리와의 결정적 차이 — 왜 훨씬 단순한가**: Island 엔트리는 자기
/// 호스트 스코프 안의 각 LRef 엘리먼트마다 직접 클릭 리스너를 건다(활성화 시점에
/// 곧바로, RQ3의 "load/visible 시 즉시 activate" 모델). Flatten 엔트리는 그렇게
/// 하지 않는다 — 경계가 없는 H-Qwik 모델의 정의적 특성대로, **전역 클릭 위임은
/// 이 wasm 엔트리가 아니라 JS 쪽(`bootstrap.js`/`bootstrap-flat.js`, Qwik 메커니즘
/// 재사용, CLAUDE.md 2-1)이 담당**하고, 이 구조체는 딱 `new(captures_str)` 생성자
/// (첫 클릭의 LRef가 실어온 캡처로 페이지 전체 World를 한 번에 materialize)와
/// `dispatch(symbol, value)`(그 이후 클릭마다 호출)만 노출하면 된다 — 그래서
/// `Rc<RefCell<...>>`도, 클로저 캡처도, 이벤트 리스너 부착 루프도 전혀 필요 없다.
fn generate_flatten_entry_body(has_shared: bool) -> String {
    let mut out = String::new();
    let field_types = if has_shared {
        "app: ::loom_runtime::LoomApp, dom: ::loom_runtime::web::BrowserDom, signals: super::Signals, shared: ::loom_runtime::web::JsSharedStore"
    } else {
        "app: ::loom_runtime::LoomApp, dom: ::loom_runtime::web::BrowserDom, signals: super::Signals"
    };
    writeln!(out, "#[cfg(any(target_arch = \"wasm32\", target_arch = \"wasm64\"))]").unwrap();
    writeln!(out, "mod resumed_app {{").unwrap();
    writeln!(out, "    use ::wasm_bindgen::prelude::*;").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    #[wasm_bindgen]").unwrap();
    writeln!(out, "    pub struct ResumedApp {{ {field_types} }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "    #[wasm_bindgen]").unwrap();
    writeln!(out, "    impl ResumedApp {{").unwrap();
    writeln!(
        out,
        "        /// `captures_str`는 첫 클릭의 LRef가 실어 나른 그 액션 자신의 캡처"
    )
    .unwrap();
    writeln!(
        out,
        "        /// (`\"count:0\"`류) — 병합된 그래프의 다른 시그널들은 DOM adoption으로"
    )
    .unwrap();
    writeln!(
        out,
        "        /// 복구되므로(List/Bool과 동일 논리), 어느 위젯의 첫 클릭에서 생성되든"
    )
    .unwrap();
    writeln!(out, "        /// 전체 World가 올바르게 복구된다.").unwrap();
    writeln!(out, "        #[wasm_bindgen(constructor)]").unwrap();
    writeln!(out, "        pub fn new(captures_str: &str) -> ResumedApp {{").unwrap();
    writeln!(out, "            ::console_error_panic_hook::set_once();").unwrap();
    writeln!(
        out,
        "            let document = ::web_sys::window().expect(\"no `window`\").document().expect(\"no `document`\");"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "            let captures: ::std::vec::Vec<(String, String)> = captures_str"
    )
    .unwrap();
    writeln!(out, "                .split(',')").unwrap();
    writeln!(out, "                .filter(|s| !s.is_empty())").unwrap();
    writeln!(out, "                .map(|kv| {{").unwrap();
    writeln!(out, "                    let mut parts = kv.splitn(2, ':');").unwrap();
    writeln!(
        out,
        "                    let key = parts.next().unwrap_or_default().to_string();"
    )
    .unwrap();
    writeln!(
        out,
        "                    let value = parts.next().unwrap_or_default().to_string();"
    )
    .unwrap();
    writeln!(out, "                    (key, value)").unwrap();
    writeln!(out, "                }})").unwrap();
    writeln!(out, "                .collect();").unwrap();
    writeln!(
        out,
        "            let capture_refs: ::std::vec::Vec<(&str, &str)> = captures.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "            let mut app = ::loom_runtime::LoomApp::new();").unwrap();
    writeln!(
        out,
        "            let mut dom = ::loom_runtime::web::BrowserDom::new_for_resume(document.clone());"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "            let tagged = document.query_selector_all(\"[data-loom-id]\").expect(\"query_selector_all failed\");"
    )
    .unwrap();
    writeln!(
        out,
        "            let mut by_name: ::std::collections::HashMap<String, ::loom_runtime::NodeHandle> = ::std::collections::HashMap::new();"
    )
    .unwrap();
    writeln!(out, "            for i in 0..tagged.length() {{").unwrap();
    writeln!(
        out,
        "                let node = tagged.get(i).expect(\"NodeList index out of bounds\");"
    )
    .unwrap();
    writeln!(
        out,
        "                let el: ::web_sys::Element = ::wasm_bindgen::JsCast::unchecked_into(node.clone());"
    )
    .unwrap();
    writeln!(
        out,
        "                let name = el.get_attribute(\"data-loom-id\").expect(\"just matched the selector\");"
    )
    .unwrap();
    writeln!(out, "                let handle = dom.adopt(node);").unwrap();
    writeln!(out, "                by_name.insert(name, handle);").unwrap();
    writeln!(out, "            }}").unwrap();
    writeln!(
        out,
        "            let find_node = move |name: &str| -> ::loom_runtime::NodeHandle {{"
    )
    .unwrap();
    writeln!(
        out,
        "                *by_name.get(name).unwrap_or_else(|| panic!(\"loomc generated resumed app entry: no element tagged data-loom-id={{:?}}\", name))"
    )
    .unwrap();
    writeln!(out, "            }};").unwrap();
    writeln!(out).unwrap();
    if has_shared {
        writeln!(
            out,
            "            let mut shared = ::loom_runtime::web::JsSharedStore;"
        )
        .unwrap();
        writeln!(
            out,
            "            let signals = super::resume(&mut app, &mut dom, find_node, &capture_refs, &mut shared);"
        )
        .unwrap();
        writeln!(out, "            ResumedApp {{ app, dom, signals, shared }}").unwrap();
    } else {
        writeln!(
            out,
            "            let signals = super::resume(&mut app, &mut dom, find_node, &capture_refs);"
        )
        .unwrap();
        writeln!(out, "            ResumedApp {{ app, dom, signals }}").unwrap();
    }
    writeln!(out, "        }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "        pub fn dispatch(&mut self, symbol: &str, value: Option<String>) {{"
    )
    .unwrap();
    writeln!(
        out,
        "            let action = super::resume_action_by_symbol(symbol, value.as_deref().unwrap_or(\"\"));"
    )
    .unwrap();
    if has_shared {
        writeln!(
            out,
            "            super::apply_action(&mut self.app, &self.signals, &action, &mut self.shared);"
        )
        .unwrap();
    } else {
        writeln!(
            out,
            "            super::apply_action(&mut self.app, &self.signals, &action);"
        )
        .unwrap();
    }
    writeln!(out, "            self.app.tick(&mut self.dom);").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out, "#[cfg(any(target_arch = \"wasm32\", target_arch = \"wasm64\"))]").unwrap();
    writeln!(out, "pub use resumed_app::ResumedApp;").unwrap();
    writeln!(out).unwrap();
    out
}

/// RQ6 §8 item 4's H-Qwik counterfactual: recursively inlines every
/// `ComponentRef` in `graph` with its target's own JSX subtree, producing
/// ONE merged `Component` — no Island boundary, no separate compiled unit,
/// one `Signals`/`Action`/`World`/LRef payload for the whole graph. Because
/// this operates at the JSX-tree level *before* codegen, every existing
/// codegen mechanism (`collect_actions`, `collect_binding_shapes`,
/// `lower_node`, `resume`, LRef generation) works correctly and completely
/// unchanged on the result — from codegen's point of view it's just one
/// (larger) ordinary component tree, exactly the "no independent World
/// partition, no independent payload" difference from `GraphMode::Islands`
/// RQ6 §8 item 4 specifies. This also handles arbitrarily deep nesting for
/// free (a `ComponentRef` inside an inlined component's own tree is itself
/// recursively flattened), unlike `GraphMode::Islands`'s module-per-
/// component approach, which needed the explicit `super::` path-prefix
/// fix (`CodegenCtx::wrapped_in_own_module`) to handle nesting at all.
///
/// **Scope cut (stated, not hidden)**: requires every signal name to be
/// globally unique across the whole graph (panics with a clear message
/// otherwise) — automatic per-component renaming/namespacing is possible
/// but not needed by Task #26's own fixture (which simply avoids colliding
/// names across its components), so it isn't implemented here. Also: RQ6
/// §8 item 4's "서로 다른 anchor marker" language is satisfied loosely via
/// each inlined component's own existing `data-loom-id`/`data-loom-lref`
/// tags (already emitted identically in both `GraphMode`s) rather than a
/// dedicated new "anchor id" IR field/attribute — sufficient to locate
/// each former-Island's own subtree for Task #26/#27's measurement
/// purposes without a wider IR change; revisit if a future measurement
/// genuinely needs a single distinguished per-component root marker this
/// doesn't provide.
fn flatten_graph(graph: &ComponentGraph) -> Component {
    let root = graph.root_component();
    let mut signals = root.signals.clone();
    let mut shared_bindings = root.shared_bindings.clone();
    let flattened_root = flatten_node(&root.root, graph, &mut signals, &mut shared_bindings);
    Component {
        name: root.name.clone(),
        signals,
        shared_bindings,
        root: flattened_root,
    }
}

fn flatten_node(
    node: &JsxNode,
    graph: &ComponentGraph,
    signals: &mut Vec<SignalDecl>,
    shared_bindings: &mut Vec<SharedBinding>,
) -> JsxNode {
    match node {
        JsxNode::Element {
            tag,
            class,
            action,
            children,
        } => JsxNode::Element {
            tag: tag.clone(),
            class: class.clone(),
            action: action.clone(),
            children: children
                .iter()
                .map(|c| flatten_node(c, graph, signals, shared_bindings))
                .collect(),
        },
        JsxNode::ListBinding { signal, row_var, row } => JsxNode::ListBinding {
            signal: signal.clone(),
            row_var: row_var.clone(),
            row: Box::new(flatten_node(row, graph, signals, shared_bindings)),
        },
        JsxNode::CondBinding { cond, then_branch } => JsxNode::CondBinding {
            cond: cond.clone(),
            then_branch: Box::new(flatten_node(then_branch, graph, signals, shared_bindings)),
        },
        JsxNode::ComponentRef { component: ref_name, .. } => {
            let target = graph
                .components
                .iter()
                .find(|c| &c.name == ref_name)
                .unwrap_or_else(|| panic!("loomc internal error: ComponentRef names unknown component `{ref_name}`"));
            for s in &target.signals {
                if signals.iter().any(|existing| existing.name == s.name) {
                    panic!(
                        "loomc: GraphMode::Flatten requires every signal name to be globally unique across the whole ComponentGraph (found a second declaration of `{}`, referenced via component `{}`) — automatic renaming is a stated scope cut, not implemented",
                        s.name, target.name
                    );
                }
            }
            signals.extend(target.signals.iter().cloned());
            shared_bindings.extend(target.shared_bindings.iter().cloned());
            flatten_node(&target.root, graph, signals, shared_bindings)
        }
        other @ (JsxNode::StaticText(_)
        | JsxNode::TextBinding { .. }
        | JsxNode::RowItemText
        | JsxNode::InputBinding { .. }
        | JsxNode::SharedValueBinding { .. }) => other.clone(),
    }
}

/// PascalCase component name -> snake_case Rust module name (`ProductCard`
/// -> `product_card`). Component names are already valid Rust-identifier-
/// shaped strings (SWC only accepts valid JS identifiers as function names,
/// which overlap with valid Rust identifiers for every case this compiler's
/// narrow subset allows).
fn sanitize_mod_name(component_name: &str) -> String {
    let mut out = String::new();
    for (i, c) in component_name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn island_tag_for(component: &Component, activation: IslandActivation) -> IslandTag {
    let base = sanitize_mod_name(&component.name);
    IslandTag {
        island_id: base.replace('_', "-"),
        chunk: base,
        ctor: format!("{}Island", component.name),
        activation,
    }
}

/// Compiles every component in `graph` — the root unqualified (backward
/// compatible with `generate()`), every other (Island-activated —
/// `ComponentRef`'s doc comment states 증분 4 requires this) component
/// namespaced into its own `pub mod <name>_generated { ... }` plus a
/// generated `#[wasm_bindgen(constructor)]` Island entry point inside that
/// same module.
fn generate_islands(graph: &ComponentGraph) -> String {
    // Precomputed once: which components author `sharedSignal(...)`
    // directly — see `CodegenCtx::components_with_shared_bindings`'s doc
    // comment for how `lower_node`'s `ComponentRef` arm uses this.
    let components_with_shared_bindings: HashSet<String> = graph
        .components
        .iter()
        .filter(|c| !c.shared_bindings.is_empty())
        .map(|c| c.name.clone())
        .collect();

    let mut out = String::new();
    for component in &graph.components {
        let has_shared = !component.shared_bindings.is_empty();
        if component.name == graph.root {
            out.push_str(&generate_component(component, None, has_shared, &components_with_shared_bindings));
            continue;
        }
        let activation = graph.activation_for(&component.name).unwrap_or_else(|| {
            panic!(
                "loomc: component `{}` is declared but never referenced via a `client:load`/`client:visible` reference — 증분 4 scope: every non-root component in `GraphMode::Islands` must be Island-activated",
                component.name
            )
        });
        let has_input_binding = collect_actions(&component.root)
            .iter()
            .any(|a| matches!(a, ClickAction::SetString { .. }));
        if has_input_binding {
            panic!(
                "loomc: component `{}` is Island-activated but contains an <input> two-way binding — 증분 4 scope: compiled Island entry points wire `click` listeners only (see generate_island_entry_body's doc comment); not yet supported",
                component.name
            );
        }
        let tag = island_tag_for(component, activation);
        let mod_name = sanitize_mod_name(&component.name);
        writeln!(out, "pub mod {mod_name}_generated {{").unwrap();
        writeln!(out, "    use ::wasm_bindgen::JsCast as _;").unwrap();
        out.push_str(&generate_component(
            component,
            Some(&tag),
            has_shared,
            &components_with_shared_bindings,
        ));
        out.push_str(&generate_island_entry_body(&tag, has_shared));
        writeln!(out, "}}").unwrap();
    }
    out
}

/// Generated *inside* an Island component's own `pub mod <name>_generated {
/// ... }` block (so it can refer to `Signals`/`setup`/`resume`/`apply_action`
/// /`resume_action_by_symbol` unqualified). The compiler-generated
/// replacement for `loom-island-demo`'s hand-written
/// `QuantityPickerIsland`/`ProductCardIsland` structs. Reuses the *same*
/// generic `Action`/`apply_action`/`resume_action_by_symbol`/`resume`
/// dispatch machinery every non-Island resumed component already uses
/// (design-fork decision, Task #25 plan §3.4) — the only real difference
/// from `ResumedComponent` is *where* DOM discovery happens (host-scoped via
/// `loom_runtime::web::index_own_tagged_descendants`/
/// `own_tagged_lref_descendants` — RQ3's opaque-boundary rule — **not**
/// `ResumedComponent::new`'s page-global `query_selector_all` bug) and *when*
/// listeners get wired (proactively at construction time, matching Islands'
/// load/visible activation model, not lazily on first click the way
/// `bootstrap.js`'s page-global delegation works).
///
/// **Scope cut (stated, not hidden)**: wires `click` listeners only —
/// `generate_islands` panics before calling this if the component contains
/// an `<input>` binding. Unlike `bootstrap.js`'s runtime event-type guessing
/// (`click` vs `input`, decided per dispatched element at dispatch time),
/// every LRef site an Island entry point wires is statically known (at
/// codegen time) to be `click`-only within this scope cut — so there is
/// nothing left to guess at runtime, a real (if narrow) correctness
/// improvement over the page-global delegation path, not just parity with
/// it.
fn generate_island_entry_body(tag: &IslandTag, has_shared: bool) -> String {
    let struct_name = &tag.ctor;
    let mut out = String::new();
    let island_state_fields = if has_shared {
        "app: ::loom_runtime::LoomApp, dom: ::loom_runtime::web::BrowserDom, signals: Signals, shared: ::loom_runtime::web::JsSharedStore"
    } else {
        "app: ::loom_runtime::LoomApp, dom: ::loom_runtime::web::BrowserDom, signals: Signals"
    };
    writeln!(out, "struct IslandState {{ {island_state_fields} }}").unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#[::wasm_bindgen::prelude::wasm_bindgen]").unwrap();
    writeln!(
        out,
        "pub struct {struct_name} {{ state: ::std::rc::Rc<::std::cell::RefCell<IslandState>> }}"
    )
    .unwrap();
    writeln!(out).unwrap();
    writeln!(out, "#[::wasm_bindgen::prelude::wasm_bindgen]").unwrap();
    writeln!(out, "impl {struct_name} {{").unwrap();
    writeln!(out, "    #[::wasm_bindgen::prelude::wasm_bindgen(constructor)]").unwrap();
    writeln!(out, "    pub fn new(host: ::web_sys::Element) -> {struct_name} {{").unwrap();
    writeln!(out, "        ::console_error_panic_hook::set_once();").unwrap();
    writeln!(
        out,
        "        let document = ::web_sys::window().expect(\"no `window`\").document().expect(\"no `document`\");"
    )
    .unwrap();
    writeln!(
        out,
        "        let mut dom = ::loom_runtime::web::BrowserDom::new_for_resume(document);"
    )
    .unwrap();
    writeln!(out, "        let mut app = ::loom_runtime::LoomApp::new();").unwrap();
    // Adopt every own-scope tagged element into a `NodeHandle` *eagerly*,
    // up front — matching `ResumedComponent::new`'s exact pattern (see
    // that struct's own doc comment). This is not just style parity: if
    // `find_node` instead called `dom.adopt(...)` *lazily* inside its own
    // closure body, the closure would capture `dom` by mutable reference,
    // and the very next line's `resume(&mut app, &mut dom, &mut find_node,
    // ...)` would then try to borrow `dom` mutably *again* while that
    // capture is still live — a real double-mutable-borrow rustc rejects
    // (found by actually compiling this fixture, not assumed). Adopting up
    // front means `find_node` only ever reads an already-built `HashMap`,
    // an immutable capture, so no such conflict exists.
    writeln!(
        out,
        "        let by_name: ::std::collections::HashMap<String, ::loom_runtime::NodeHandle> = ::loom_runtime::web::index_own_tagged_descendants(&host).into_iter().map(|(name, el)| (name, dom.adopt(el.unchecked_into()))).collect();"
    )
    .unwrap();
    writeln!(out, "        let mut find_node = |name: &str| -> ::loom_runtime::NodeHandle {{").unwrap();
    writeln!(
        out,
        "            *by_name.get(name).unwrap_or_else(|| panic!(\"loomc generated Island entry: no own-scope element tagged data-loom-id={{:?}}\", name))"
    )
    .unwrap();
    writeln!(out, "        }};").unwrap();
    if has_shared {
        writeln!(out, "        let mut shared = ::loom_runtime::web::JsSharedStore;").unwrap();
        writeln!(
            out,
            "        let signals = resume(&mut app, &mut dom, &mut find_node, &[], &mut shared);"
        )
        .unwrap();
        writeln!(
            out,
            "        let state = ::std::rc::Rc::new(::std::cell::RefCell::new(IslandState {{ app, dom, signals, shared }}));"
        )
        .unwrap();
    } else {
        writeln!(out, "        let signals = resume(&mut app, &mut dom, &mut find_node, &[]);").unwrap();
        writeln!(
            out,
            "        let state = ::std::rc::Rc::new(::std::cell::RefCell::new(IslandState {{ app, dom, signals }}));"
        )
        .unwrap();
    }
    writeln!(
        out,
        "        for lref_el in ::loom_runtime::web::own_tagged_lref_descendants(&host) {{"
    )
    .unwrap();
    writeln!(
        out,
        "            let lref = lref_el.get_attribute(\"data-loom-lref\").unwrap_or_default();"
    )
    .unwrap();
    writeln!(out, "            let mut parts = lref.splitn(3, '|');").unwrap();
    writeln!(out, "            let _chunk = parts.next();").unwrap();
    writeln!(out, "            let symbol = parts.next().unwrap_or_default().to_string();").unwrap();
    writeln!(out, "            let state_for_closure = state.clone();").unwrap();
    writeln!(
        out,
        "            let closure = ::wasm_bindgen::prelude::Closure::<dyn FnMut(::web_sys::MouseEvent)>::new(move |_e: ::web_sys::MouseEvent| {{"
    )
    .unwrap();
    writeln!(out, "                let action = resume_action_by_symbol(&symbol, \"\");").unwrap();
    writeln!(out, "                let mut s = state_for_closure.borrow_mut();").unwrap();
    if has_shared {
        writeln!(out, "                let IslandState {{ app, dom, signals, shared }} = &mut *s;").unwrap();
        writeln!(out, "                apply_action(app, signals, &action, shared);").unwrap();
    } else {
        writeln!(out, "                let IslandState {{ app, dom, signals }} = &mut *s;").unwrap();
        writeln!(out, "                apply_action(app, signals, &action);").unwrap();
    }
    writeln!(out, "                app.tick(dom);").unwrap();
    writeln!(out, "            }});").unwrap();
    writeln!(
        out,
        "            lref_el.add_event_listener_with_callback(\"click\", closure.as_ref().unchecked_ref()).expect(\"add_event_listener_with_callback failed\");"
    )
    .unwrap();
    writeln!(out, "            closure.forget();").unwrap();
    writeln!(out, "        }}").unwrap();
    writeln!(out, "        {struct_name} {{ state }}").unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out).unwrap();
    writeln!(
        out,
        "    /// RQ3's synchronous detach notification (`IslandInstance::begin_detach`'s contract)."
    )
    .unwrap();
    writeln!(out, "    pub fn begin_detach(&mut self) {{}}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();
    out
}

/// RQ6 §6 M1 real per-Island code splitting — Task #27. Empirically found
/// (not assumed): every artifact `generate_islands` produces — regardless
/// of which `--out-name` wasm-pack is told to use — contains **every**
/// component's `#[wasm_bindgen]`-exported Island struct, since they all
/// still live in the one crate/cdylib target that name merely relabels.
/// `--out-name` never controlled *what code* gets compiled, only what the
/// resulting files are called — a previously-undocumented gap, more severe
/// than the already-known "chunk directory sharing" issue, bearing
/// directly on RQ6 H-04's cost profile (a supposed benefit of Islands —
/// "loading one widget doesn't pull in every other widget's code" — was
/// not actually true of any Island fixture built so far). This function
/// generates ONE target Island's client-side code alone (`Signals`/
/// `Action`/`apply_action`/`resume_action_by_symbol`/`resume`/Island
/// entry) — a crate whose generated code is *only* this function's output
/// compiles to a wasm binary containing exactly that one Island's exported
/// symbols, nothing else.
///
/// **Why `setup()` doesn't need to be complete here**: client-side wasm
/// never calls `setup()` at all (SSR is native-only, driven by the full
/// `ComponentGraph` elsewhere, e.g. `render_html()`) — only `resume()`/
/// `apply_action()` run in the browser, and neither ever references a
/// `ComponentRef` (see `collect_binding_shapes`'s empty `ComponentRef`
/// arm) — so real per-Island isolation is unaffected by `setup()`'s
/// content. The one wrinkle: `generate_component` unconditionally emits
/// `setup()`'s text too (shared code path with the native/SSR path, not
/// worth forking just for this), and if the target's own tree contains a
/// `ComponentRef` (e.g. `ProductCard` nesting `QuantityPicker`), that
/// text calls `super::{sibling}_generated::setup(...)` — a module that
/// genuinely doesn't exist in this standalone crate. `generate_stub_for_
/// component_ref` fills exactly that gap: a same-shaped, `unreachable!()`
/// -bodied stand-in purely so rustc's type/borrow checker is satisfied;
/// since nothing ever calls the real `setup()` client-side, this stub is
/// itself dead code and never actually runs.
pub fn generate_single_island_client(graph: &ComponentGraph, island_component_name: &str) -> String {
    let components_with_shared_bindings: HashSet<String> = graph
        .components
        .iter()
        .filter(|c| !c.shared_bindings.is_empty())
        .map(|c| c.name.clone())
        .collect();
    let component = graph.components.iter().find(|c| c.name == island_component_name).unwrap_or_else(|| {
        panic!("loomc: generate_single_island_client: no component named `{island_component_name}` in this graph")
    });
    let activation = graph.activation_for(&component.name).unwrap_or_else(|| {
        panic!(
            "loomc: generate_single_island_client: component `{island_component_name}` is not Island-activated"
        )
    });
    let has_shared = !component.shared_bindings.is_empty();
    let tag = island_tag_for(component, activation);

    let mut out = String::new();
    for ref_name in collect_component_refs(&component.root) {
        out.push_str(&generate_stub_for_component_ref(
            &ref_name,
            components_with_shared_bindings.contains(&ref_name),
        ));
    }
    writeln!(out, "pub mod {}_generated {{", sanitize_mod_name(&component.name)).unwrap();
    writeln!(out, "    use ::wasm_bindgen::JsCast as _;").unwrap();
    out.push_str(&generate_component(component, Some(&tag), has_shared, &components_with_shared_bindings));
    out.push_str(&generate_island_entry_body(&tag, has_shared));
    writeln!(out, "}}").unwrap();
    out
}

/// Every `ComponentRef` target named anywhere in `node`'s subtree — used by
/// `generate_single_island_client` to know which sibling stub modules a
/// standalone Island crate needs (see that function's doc comment).
fn collect_component_refs(node: &JsxNode) -> Vec<String> {
    let mut names = Vec::new();
    fn walk(node: &JsxNode, names: &mut Vec<String>) {
        match node {
            JsxNode::ComponentRef { component, .. } => names.push(component.clone()),
            JsxNode::Element { children, .. } => {
                for c in children {
                    walk(c, names);
                }
            }
            JsxNode::ListBinding { row, .. } => walk(row, names),
            JsxNode::CondBinding { then_branch, .. } => walk(then_branch, names),
            JsxNode::StaticText(_) | JsxNode::TextBinding { .. } | JsxNode::RowItemText | JsxNode::InputBinding { .. } | JsxNode::SharedValueBinding { .. } => {}
        }
    }
    walk(node, &mut names);
    names
}

/// A same-shaped, always-`unreachable!()` stand-in for a sibling Island's
/// `_generated` module — see `generate_single_island_client`'s doc comment
/// for why this is safe (the real `setup()` referencing it is itself dead
/// code in a client-only build).
fn generate_stub_for_component_ref(ref_name: &str, has_shared: bool) -> String {
    let mod_name = sanitize_mod_name(ref_name);
    let mut out = String::new();
    writeln!(out, "pub mod {mod_name}_generated {{").unwrap();
    writeln!(out, "    pub struct Signals;").unwrap();
    writeln!(out, "    pub enum Action {{}}").unwrap();
    let sig = if has_shared {
        "    pub fn setup(_app: &mut ::loom_runtime::LoomApp, _dom: &mut impl ::loom_runtime::DomOps, _mount: ::loom_runtime::NodeHandle, _shared: &mut impl ::loom_runtime::SharedStore) -> (Signals, ::std::vec::Vec<(::loom_runtime::NodeHandle, Action)>, ::std::vec::Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {"
    } else {
        "    pub fn setup(_app: &mut ::loom_runtime::LoomApp, _dom: &mut impl ::loom_runtime::DomOps, _mount: ::loom_runtime::NodeHandle) -> (Signals, ::std::vec::Vec<(::loom_runtime::NodeHandle, Action)>, ::std::vec::Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {"
    };
    writeln!(out, "{sig}").unwrap();
    writeln!(
        out,
        "        unreachable!(\"generate_single_island_client stub: setup() is dead code in a client-only Island build\")"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    out
}

pub fn generate(component: &Component) -> String {
    let has_shared = !component.shared_bindings.is_empty();
    generate_component(component, None, has_shared, &HashSet::new())
}

fn generate_component(
    component: &Component,
    island: Option<&IslandTag>,
    has_shared: bool,
    components_with_shared_bindings: &HashSet<String>,
) -> String {
    let mut out = String::new();
    writeln!(out, "// GENERATED by loomc from {}() — do not edit by hand.", component.name).unwrap();
    writeln!(out).unwrap();
    writeln!(out, "pub struct Signals {{").unwrap();
    for s in &component.signals {
        writeln!(out, "    pub {}: ::bevy_ecs::entity::Entity,", s.name).unwrap();
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(out, "#[derive(Debug, Clone)]").unwrap();
    writeln!(out, "pub enum Action {{").unwrap();
    let actions = collect_actions(&component.root);
    let mut variant_names = Vec::new();
    for (i, action) in actions.iter().enumerate() {
        let variant = format!("Action{i}");
        match action {
            ClickAction::Increment { .. } | ClickAction::Rotate { .. } | ClickAction::Toggle { .. } => {
                writeln!(out, "    {variant},").unwrap();
            }
            ClickAction::Append { .. } | ClickAction::SetString { .. } => {
                writeln!(out, "    {variant}(String),").unwrap();
            }
        }
        variant_names.push(variant);
    }
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    let apply_action_sig = if has_shared {
        "pub fn apply_action(app: &mut ::loom_runtime::LoomApp, signals: &Signals, action: &Action, shared: &mut impl ::loom_runtime::SharedStore) {"
    } else {
        "pub fn apply_action(app: &mut ::loom_runtime::LoomApp, signals: &Signals, action: &Action) {"
    };
    writeln!(out, "{apply_action_sig}").unwrap();
    writeln!(out, "    match action {{").unwrap();
    for (variant, action) in variant_names.iter().zip(actions.iter()) {
        match action {
            ClickAction::Increment { signal } => {
                writeln!(out, "        Action::{variant} => {{").unwrap();
                writeln!(
                    out,
                    "            app.world.get_mut::<::loom_runtime::Signal<i32>>(signals.{signal}).unwrap().0 += 1;"
                )
                .unwrap();
                // Only I32 signals (`Increment`) can ever have a matching
                // `SharedBinding` — `sharedSignal(...)`'s writer role is
                // I32-only (see that type's doc comment), and every other
                // `ClickAction` variant targets a different signal kind
                // (StringList/Bool/Str), so none of them can match here.
                if let Some(shared_binding) = component.shared_bindings.iter().find(|sb| &sb.signal == signal) {
                    writeln!(
                        out,
                        "            shared.set({:?}, app.world.get::<::loom_runtime::Signal<i32>>(signals.{signal}).unwrap().0);",
                        shared_binding.key
                    )
                    .unwrap();
                }
                writeln!(out, "        }}").unwrap();
            }
            ClickAction::Rotate { signal } => {
                writeln!(out, "        Action::{variant} => {{").unwrap();
                writeln!(
                    out,
                    "            let sig = &mut app.world.get_mut::<::loom_runtime::Signal<Vec<String>>>(signals.{signal}).unwrap().0;"
                )
                .unwrap();
                writeln!(out, "            if !sig.is_empty() {{ let front = sig.remove(0); sig.push(front); }}").unwrap();
                writeln!(out, "        }}").unwrap();
            }
            ClickAction::Append { signal, .. } => {
                writeln!(out, "        Action::{variant}(literal) => {{").unwrap();
                writeln!(
                    out,
                    "            app.world.get_mut::<::loom_runtime::Signal<Vec<String>>>(signals.{signal}).unwrap().0.push(literal.clone());"
                )
                .unwrap();
                writeln!(out, "        }}").unwrap();
            }
            ClickAction::Toggle { signal } => {
                writeln!(out, "        Action::{variant} => {{").unwrap();
                writeln!(
                    out,
                    "            let sig = &mut app.world.get_mut::<::loom_runtime::Signal<bool>>(signals.{signal}).unwrap().0;"
                )
                .unwrap();
                writeln!(out, "            *sig = !*sig;").unwrap();
                writeln!(out, "        }}").unwrap();
            }
            ClickAction::SetString { signal } => {
                writeln!(out, "        Action::{variant}(value) => {{").unwrap();
                writeln!(
                    out,
                    "            app.world.get_mut::<::loom_runtime::Signal<String>>(signals.{signal}).unwrap().0 = value.clone();"
                )
                .unwrap();
                writeln!(out, "        }}").unwrap();
            }
        }
    }
    if actions.is_empty() {
        // A component with zero actions of its own is a real, new shape
        // 증분 4 introduces: a `ComponentRef`-only tree (e.g. `App`'s root
        // being just a wrapper around an Island reference) contributes
        // nothing to `collect_actions`, so `enum Action {}` has zero
        // variants. `match action {}` on a zero-variant enum is *not*
        // treated as exhaustive by rustc when matching through a reference
        // ("references are always considered inhabited", rustc's own
        // E0004 diagnostic wording) — a genuine, pre-existing gap in this
        // function surfaced for the first time by Islands (every compiled
        // component before this had at least one action).
        writeln!(out, "        _ => {{}}").unwrap();
    }
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    let setup_sig = if has_shared {
        "pub fn setup(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mount: ::loom_runtime::NodeHandle, shared: &mut impl ::loom_runtime::SharedStore) -> (Signals, Vec<(::loom_runtime::NodeHandle, Action)>, Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {"
    } else {
        "pub fn setup(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mount: ::loom_runtime::NodeHandle) -> (Signals, Vec<(::loom_runtime::NodeHandle, Action)>, Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {"
    };
    writeln!(out, "{setup_sig}").unwrap();

    // Register exactly the patch Systems this component's binding sites
    // actually need — no unconditional "register everything" default.
    let shapes = collect_binding_shapes(&component.root);
    for shape in &shapes {
        match shape {
            BindingShape::TextI32 => {
                writeln!(out, "    app.add_systems(::loom_runtime::patch_text_system::<i32>);").unwrap();
            }
            BindingShape::ListString => {
                writeln!(out, "    app.add_systems(::loom_runtime::list_reconcile_k_system::<String>);").unwrap();
            }
            BindingShape::InputString => {
                writeln!(out, "    app.add_systems(::loom_runtime::patch_input_system);").unwrap();
            }
        }
    }
    writeln!(out, "    let mut actions: Vec<(::loom_runtime::NodeHandle, Action)> = Vec::new();").unwrap();
    // `fn(String) -> Action` values (a tuple-variant constructor used bare,
    // e.g. `Action::Action3`, is itself such a function — no closure
    // allocated, so CLAUDE.md 금지 4 doesn't apply) — the live path's host
    // glue (loom-demo) wires a real `input` DOM listener per entry, reading
    // `event.target.value` and calling the constructor itself.
    writeln!(
        out,
        "    let mut input_actions: Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)> = Vec::new();"
    )
    .unwrap();

    if island.is_some() && try_render_fully_static(&component.root).is_some() {
        panic!(
            "loomc: component `{}` is Island-activated but has no dynamic content at all — a fully static subtree collapses into one `append_static_html` call with no addressable root node to tag with `data-loom-island*` attributes (see try_render_fully_static's doc comment); remove the `client:` directive if this component is meant to be pure static markup",
            component.name
        );
    }

    let mut ctx = CodegenCtx {
        node_counter: 0,
        signal_entities: Vec::new(),
        action_index: 0,
        cond_site_index: 0,
        cond_site_fns: String::new(),
        cond_site_fn_names: Vec::new(),
        wrapped_in_own_module: island.is_some(),
        components_with_shared_bindings: components_with_shared_bindings.clone(),
    };
    lower_node(&mut out, &component.root, "mount", component, &mut ctx);

    // Publish this component's own directly-authored `sharedSignal(...)`
    // initial values (see `ir::SharedBinding`'s doc comment) — the literal
    // declared initial is always correct here (this call happens the
    // moment the entity is first spawned, before anything could have
    // changed it).
    for shared_binding in &component.shared_bindings {
        let initial = component
            .signals
            .iter()
            .find(|s| s.name == shared_binding.signal)
            .and_then(|s| match &s.init {
                SignalInit::I32(v) => Some(*v),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!(
                    "loomc internal error: sharedSignal `{}` names a signal that isn't I32",
                    shared_binding.signal
                )
            });
        writeln!(out, "    shared.set({:?}, {initial});", shared_binding.key).unwrap();
    }

    if let Some(tag) = island {
        // `node_0` is guaranteed to be the component's own root element's
        // handle: `ctx.node_counter` starts at 0, and the guard above already
        // ruled out the one way the root could avoid calling
        // `ctx.fresh_node_var()` at all (a whole-subtree static collapse).
        writeln!(out, "    dom.set_attribute(node_0, \"data-loom-island\", \"\");").unwrap();
        writeln!(
            out,
            "    dom.set_attribute(node_0, \"data-loom-island-id\", {:?});",
            tag.island_id
        )
        .unwrap();
        writeln!(
            out,
            "    dom.set_attribute(node_0, \"data-loom-activation\", {:?});",
            tag.activation.as_attr_value()
        )
        .unwrap();
        writeln!(out, "    dom.set_attribute(node_0, \"data-loom-chunk\", {:?});", tag.chunk).unwrap();
        writeln!(out, "    dom.set_attribute(node_0, \"data-loom-ctor\", {:?});", tag.ctor).unwrap();
    }

    writeln!(out, "    let signals = Signals {{").unwrap();
    for s in &component.signals {
        let entity_var = ctx
            .signal_entities
            .iter()
            .find(|(name, _)| name == &s.name)
            .map(|(_, var)| var.clone())
            .unwrap_or_else(|| panic!("loomc internal error: signal `{}` was never spawned", s.name));
        writeln!(out, "        {}: {},", s.name, entity_var).unwrap();
    }
    writeln!(out, "    }};").unwrap();
    writeln!(out, "    (signals, actions, input_actions)").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    // Per-site conditional reconcile functions (top-level, can't be emitted
    // inline mid-`setup()`) — one per `CondBinding` site encountered above.
    out.push_str(&ctx.cond_site_fns);

    generate_resume_action_by_symbol(&mut out, &actions, &variant_names);
    writeln!(out).unwrap();
    generate_resume(&mut out, component, &ctx.cond_site_fn_names, has_shared);

    out
}

/// docs/02's "LRef 최소 캡처 집합" — computed exactly (not deferred) for the
/// restricted IR this compiler supports: every `ClickAction` variant names
/// exactly one signal, and `apply_action`'s generated match arm for that
/// variant only ever touches that one signal — so the minimal capture set
/// for a given action is `{that signal}` if it's `i32`, or `{}` if it's a
/// `StringList` (list resume recovers content from the DOM instead, needing
/// no capture at all — see this module's "List resume" doc section).
///
/// **Caveat, stated plainly — why this narrowing is safe today but not
/// unconditionally**: every signal a capture list omits falls back to its
/// *literal declared initial value* in `resume()`. That's only correct
/// because Phase 1's IR never lets a signal's true first-render value
/// diverge from that literal (no per-request/dynamic initial values exist
/// yet). Each action-bearing element resumes its *own independent* full
/// `Signals` on first click (`demo/resume/bootstrap.js` caches
/// `_loomInstance` per **element**, not per component) — so today, a signal
/// this action doesn't capture but a *different* button's action would
/// need is never actually read by *this* resumed instance, making the
/// omission harmless. If the IR ever gains non-literal initial values
/// (e.g. a per-request starting count), this stops being safe: a signal
/// omitted here would silently resume to a stale/wrong value instead of
/// its real current one, for any other action later dispatched against
/// this same instance. Revisit before that IR extension ships — not
/// re-derived as "whole-component capture" preemptively here, since that
/// would just be re-introducing the over-capture this function exists to
/// remove without being asked to by an actual requirement yet.
fn format_i32_capture_for_action(component: &Component, action: &ClickAction) -> String {
    let signal_name = match action {
        ClickAction::Increment { signal }
        | ClickAction::Rotate { signal }
        | ClickAction::Append { signal, .. }
        | ClickAction::Toggle { signal }
        | ClickAction::SetString { signal } => signal,
    };
    component
        .signals
        .iter()
        .find(|s| &s.name == signal_name)
        .and_then(|s| match &s.init {
            SignalInit::I32(v) => Some(format!("{signal_name}:{v}")),
            // None of these need a capture entry: StringList recovers row
            // values from the DOM (list resume, see this module's doc
            // comment); Bool recovers shown/hidden the same way (see
            // `generate_resume`'s `SignalInit::Bool` arm); Str (input
            // binding) recovers the live typed value the same way (see
            // `generate_resume`'s `SignalInit::Str` arm) — all three are
            // DOM-recovered, not capture-fallback-dependent, so omitting
            // them is safe unconditionally, not just "safe today" like the
            // I32 case. (`SetString` itself never actually reaches this
            // function in practice — `JsxNode::InputBinding` gets its own
            // LRef directly in `lower_node`, not through this Element-action
            // path — but the match must stay exhaustive.)
            SignalInit::StringList(_) | SignalInit::Bool(_) | SignalInit::Str(_) => None,
        })
        .unwrap_or_default()
}

/// Inverse of the `Action` collection in `generate()`: parses an LRef's
/// `symbol` segment (an `ActionN` variant name) back into an `Action` value.
/// Generated for every action the component declares, not just the ones
/// `setup()` currently tags with an LRef — harmless extra completeness.
///
/// `Append`'s literal argument is baked in here at codegen time, the same
/// way `setup()`'s `actions.push(...)` bakes it into the live path's
/// `Action` value — it's compile-time known (Phase 1's IR only allows
/// literal `.append("...")` arguments), so there's nothing for a caller to
/// supply at resume time. **`string_arg` (2026-07-24)** is the one exception
/// to "every action shape is fully determined by its symbol alone": a
/// `SetString` action's payload is a value the user just typed, which by
/// construction cannot be known until the moment the real `input` DOM event
/// fires — so unlike every other action, its host glue caller (see
/// `ResumedComponent::dispatch`) must thread that live value through. Every
/// other action ignores this parameter entirely.
fn generate_resume_action_by_symbol(out: &mut String, actions: &[ClickAction], variant_names: &[String]) {
    writeln!(out, "pub fn resume_action_by_symbol(symbol: &str, string_arg: &str) -> Action {{").unwrap();
    writeln!(out, "    match symbol {{").unwrap();
    for (variant, action) in variant_names.iter().zip(actions.iter()) {
        match action {
            ClickAction::Increment { .. } | ClickAction::Rotate { .. } | ClickAction::Toggle { .. } => {
                writeln!(out, "        {variant:?} => Action::{variant},").unwrap();
            }
            ClickAction::Append { literal, .. } => {
                writeln!(out, "        {variant:?} => Action::{variant}({literal:?}.to_string()),").unwrap();
            }
            ClickAction::SetString { .. } => {
                writeln!(out, "        {variant:?} => Action::{variant}(string_arg.to_string()),").unwrap();
            }
        }
    }
    writeln!(
        out,
        "        other => panic!(\"loomc generated resume_action_by_symbol: unknown action symbol {{other:?}}\"),"
    )
    .unwrap();
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

/// The client-chunk-side counterpart to `setup()`: rebuilds just enough
/// `World` state from an LRef's parsed captures for `apply_action` to run
/// against it unmodified. `find_node` resolves a signal name (matching the
/// `data-loom-id` `setup()` tagged it with) to the already-existing SSR'd
/// node's `NodeHandle` — host glue supplies this (e.g. a
/// `document.querySelector` wrapper), not generated code, same reason
/// `setup()` takes `dom: &mut impl DomOps` instead of hardcoding a backend.
fn generate_resume(
    out: &mut String,
    component: &Component,
    cond_site_fn_names: &[CondSite],
    has_shared: bool,
) {
    let resume_sig = if has_shared {
        "pub fn resume(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mut find_node: impl FnMut(&str) -> ::loom_runtime::NodeHandle, captures: &[(&str, &str)], shared: &mut impl ::loom_runtime::SharedStore) -> Signals {"
    } else {
        "pub fn resume(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mut find_node: impl FnMut(&str) -> ::loom_runtime::NodeHandle, captures: &[(&str, &str)]) -> Signals {"
    };
    writeln!(out, "{resume_sig}").unwrap();
    writeln!(out, "    app.add_systems(::loom_runtime::patch_text_system::<i32>);").unwrap();
    let has_list = component.signals.iter().any(|s| matches!(s.init, SignalInit::StringList(_)));
    if has_list {
        writeln!(out, "    app.add_systems(::loom_runtime::list_reconcile_k_system::<String>);").unwrap();
    }
    let has_input = component.signals.iter().any(|s| matches!(s.init, SignalInit::Str(_)));
    if has_input {
        writeln!(out, "    app.add_systems(::loom_runtime::patch_input_system);").unwrap();
    }
    // 조건부 안에 사는 signal은 **그 조건부가 SSR 시점에 보였는지**에 따라
    // resume 방식이 갈린다(아래 각 arm의 `gate` 처리). 그래서 조건부(Bool)를
    // 먼저 전부 처리해 `<cond>_shown`을 스코프에 올려 둔다 — 선언 순서와
    // 무관하게. `Signals` 구조체는 이 루프 뒤에 따로 조립되므로 순서를 바꿔도
    // 관측 가능한 차이는 없다.
    let nested_owner: HashMap<&str, &str> = cond_site_fn_names
        .iter()
        .flat_map(|site| site.nested.iter().map(|slot| (slot.signal(), site.signal.as_str())))
        .collect();
    let bool_first = component
        .signals
        .iter()
        .filter(|s| matches!(s.init, SignalInit::Bool(_)))
        .chain(component.signals.iter().filter(|s| !matches!(s.init, SignalInit::Bool(_))));
    for s in bool_first {
        let entity_var = format!("{}_entity", s.name);
        // `Some(cond)` = 이 signal은 `cond` 조건부의 지연 서브트리 안에 있다.
        let gate = nested_owner.get(s.name.as_str()).copied();
        match &s.init {
            SignalInit::I32(initial) => {
                writeln!(out, "    let {entity_var} = {{").unwrap();
                writeln!(
                    out,
                    "        let value: i32 = captures.iter().find(|(k, _)| *k == {:?}).and_then(|(_, v)| v.parse().ok()).unwrap_or({initial}_i32);",
                    s.name
                )
                .unwrap();
                match gate {
                    // 숨겨진 채 SSR됐다면 이 binding의 노드는 **아직 존재하지
                    // 않는다** — `find_node`를 부르면 안 된다. `DomTarget` 없이
                    // spawn하면 `setup()` 경로와 정확히 같은 상태가 되고,
                    // 조건부가 처음 열릴 때 `SpawnElement`가 노드를 만들며
                    // `DomTarget`을 붙인다.
                    Some(cond) => {
                        writeln!(out, "        if {cond}_shown {{").unwrap();
                        writeln!(
                            out,
                            "            app.world.spawn((::loom_runtime::Signal(value), ::loom_runtime::DomTarget(find_node({:?})))).id()",
                            s.name
                        )
                        .unwrap();
                        writeln!(out, "        }} else {{").unwrap();
                        writeln!(out, "            app.world.spawn((::loom_runtime::Signal(value),)).id()").unwrap();
                        writeln!(out, "        }}").unwrap();
                    }
                    None => writeln!(
                        out,
                        "        app.world.spawn((::loom_runtime::Signal(value), ::loom_runtime::DomTarget(find_node({:?})))).id()",
                        s.name
                    )
                    .unwrap(),
                }
                writeln!(out, "    }};").unwrap();
            }
            SignalInit::Bool(_) => {
                // DOM-recovered, same reasoning as list resume above — no
                // capture entry needed (`format_i32_capture_for_action`
                // already omits `Bool` signals unconditionally). Unlike the
                // I32/StringList cases, this doesn't need `captures` at all:
                // `setup()` never toggles a conditional before the page is
                // sent (nothing runs between `setup()` and its own settle
                // tick that could flip a `Bool` signal), so SSR only ever
                // produces two real states — wrapper rendered empty
                // (`initial == false`) or wrapper rendered with its static
                // content already inlined (`initial == true`, spawned by
                // this same site's reconcile function during `setup()`'s
                // settle tick). Whether the wrapper's children are empty or
                // not is therefore a complete, correct read of shown/hidden
                // for every SSR output this pipeline can actually produce —
                // this deliberately does NOT cover a hidden-*after*-having-
                // been-shown snapshot (nothing here persists a visibility
                // style into SSR'd HTML for that case to be read back; see
                // `cond.rs`'s `cond_reconcile_f_system` doc for why that
                // state can still arise from a *live*, already-hydrated
                // toggle — it just never reaches a fresh SSR pass).
                writeln!(out, "    let {}_node = find_node({:?});", s.name, s.name).unwrap();
                writeln!(
                    out,
                    "    let {}_adopted: Vec<(::loom_runtime::NodeHandle, String)> = dom.adopt_children({}_node);",
                    s.name, s.name
                )
                .unwrap();
                writeln!(out, "    let {}_shown = !{}_adopted.is_empty();", s.name, s.name).unwrap();
                writeln!(
                    out,
                    "    let {}_children: Vec<::bevy_ecs::entity::Entity> = {}_adopted.iter().map(|_| app.world.spawn(()).id()).collect();",
                    s.name, s.name
                )
                .unwrap();
                // Must attach the SAME `CondSiteN` marker `setup()`'s own
                // spawn for this site does — the reconcile System's query
                // filters on `With<CondSiteN>` (see the `CondBinding` arm's
                // doc comment), so a resumed entity missing this marker
                // would never be seen by its own site's system at all.
                let site = cond_site_fn_names
                    .iter()
                    .find(|site| site.signal == s.name)
                    .unwrap_or_else(|| panic!("loomc internal error: no reconcile function recorded for conditional `{}`", s.name));
                let (fn_name, marker_name) = (&site.fn_name, &site.marker);
                // `CondNested`는 여기서 붙일 수 없다 — 중첩 binding entity들은
                // 아래(비-Bool 패스)에서야 생긴다. 루프가 끝난 뒤 한꺼번에
                // 붙인다(바로 아래 `nested_inserts`).
                writeln!(
                    out,
                    "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({}_shown), ::loom_runtime::CondBlock {{ size: {}_children.len(), children: {}_children }}, ::loom_runtime::DomTarget({}_node), {marker_name})).id();",
                    s.name, s.name, s.name, s.name
                )
                .unwrap();
                writeln!(out, "    app.add_systems({fn_name});").unwrap();
            }
            SignalInit::StringList(_) => {
                // List resume recovers row *values* from the already-correct
                // SSR'd DOM (`adopt_children`), not from `captures` — see
                // `DomOps::adopt_children`'s doc comment for why. Both the
                // per-row entities' own `Signal<String>` *and* the list's
                // own `Signal<Vec<String>>` are seeded from the same
                // adopted values — the list signal must reflect the real
                // current content, not an empty Vec, or the settle tick
                // below (which re-derives DOM rows from *this* signal) would
                // silently drop everything adopt_children just recovered.
                //
                // 조건부 안의 리스트는 여기에 한 가지가 더 붙는다: 숨겨진 채
                // SSR됐다면 `<ul>` 자체가 없으므로 adopt할 것도 없다. 그때는
                // **선언된 초기값**으로 seed하고 `DomTarget` 없이 둔다 —
                // 조건부가 열릴 때 생성 함수가 초기 row를 방출한다. 이것은
                // "SSR 시점에 숨겨져 있던 리스트의 편집 내역은 복원되지
                // 않는다"는 뜻이고(그런 편집은 SSR HTML에 흔적이 없다), RQ3 Q2의
                // "SSR 스냅샷 동결" 한계와 같은 성질이다 — 숨기지 않고 적어 둔다.
                writeln!(out, "    let {entity_var} = {{").unwrap();
                let indent = if gate.is_some() { "            " } else { "        " };
                if let Some(cond) = gate {
                    writeln!(out, "        if {cond}_shown {{").unwrap();
                }
                writeln!(out, "{indent}let node = find_node({:?});", s.name).unwrap();
                writeln!(
                    out,
                    "{indent}let adopted: Vec<(::loom_runtime::NodeHandle, String)> = dom.adopt_children(node);"
                )
                .unwrap();
                writeln!(
                    out,
                    "{indent}let rows: Vec<::bevy_ecs::entity::Entity> = adopted.iter().map(|(handle, text)| {{"
                )
                .unwrap();
                writeln!(
                    out,
                    "{indent}    app.world.spawn((::loom_runtime::Signal(text.clone()), ::loom_runtime::DomTarget(*handle))).id()"
                )
                .unwrap();
                writeln!(out, "{indent}}}).collect();").unwrap();
                writeln!(
                    out,
                    "{indent}let values: Vec<String> = adopted.into_iter().map(|(_, text)| text).collect();"
                )
                .unwrap();
                writeln!(
                    out,
                    "{indent}app.world.spawn((::loom_runtime::Signal(values), ::loom_runtime::ListRows(rows), ::loom_runtime::DomTarget(node))).id()"
                )
                .unwrap();
                if gate.is_some() {
                    let SignalInit::StringList(items) = &s.init else { unreachable!() };
                    let items_rust = items
                        .iter()
                        .map(|s| format!("{s:?}.to_string()"))
                        .collect::<Vec<_>>()
                        .join(", ");
                    writeln!(out, "        }} else {{").unwrap();
                    writeln!(
                        out,
                        "            app.world.spawn((::loom_runtime::Signal(vec![{items_rust}]), ::loom_runtime::ListRows::default())).id()"
                    )
                    .unwrap();
                    writeln!(out, "        }}").unwrap();
                }
                writeln!(out, "    }};").unwrap();
            }
            SignalInit::Str(_) => {
                // DOM-recovered, same reasoning as list/cond resume above —
                // no capture needed. Unlike those two, there's nothing to
                // reconstruct beyond the value itself: `get_input_value`
                // reads the *live* `.value` property directly (see that
                // method's doc comment for why an attribute/captured value
                // would be wrong the instant a user has typed anything).
                writeln!(out, "    let {}_node = find_node({:?});", s.name, s.name).unwrap();
                writeln!(
                    out,
                    "    let {}_value = dom.get_input_value({}_node);",
                    s.name, s.name
                )
                .unwrap();
                writeln!(
                    out,
                    "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({}_value), ::loom_runtime::InputValueTarget, ::loom_runtime::DomTarget({}_node))).id();",
                    s.name, s.name
                )
                .unwrap();
            }
        }
    }
    // 이제서야 모든 중첩 binding entity가 존재하므로 각 조건부에 `CondNested`를
    // 붙인다 — `setup()`이 spawn 시점에 함께 넣는 것과 **같은 순서**(각 사이트의
    // `LazySlot` 순서)여야 한다. 생성된 reconcile 함수가 그 순서를 리터럴
    // 인덱스로 박아 두고 있기 때문이다.
    for site in cond_site_fn_names {
        let nested = site
            .nested
            .iter()
            .map(|slot| format!("{}_entity", slot.signal()))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            out,
            "    app.world.entity_mut({}_entity).insert(::loom_runtime::CondNested(vec![{nested}]));",
            site.signal
        )
        .unwrap();
    }
    // Every signal this `LoomApp` just spawned looks "Changed" to a patch
    // System that has never run before (Bevy's Added-counts-as-Changed —
    // see loom-runtime/src/lib.rs's module doc). Settling that here, once,
    // before returning `Signals`, means the *next* tick (from a real
    // dispatched action) only reflects that action's actual mutation — not
    // an unrelated action accidentally re-running every patch/reconcile
    // System and (for lists) despawning+recreating rows resume just spent
    // effort adopting. This trades a bounded "re-render everything once, at
    // first-interaction materialization time" cost for correctness; it does
    // not violate resumability's "zero JS before interaction" claim (this
    // only runs as part of first-interaction chunk materialization, same as
    // the rest of `resume()`), but it does mean list rows are rebuilt (not
    // reused) at that moment rather than truly left untouched — a real,
    // scale-dependent cost worth narrowing later, not hidden here.
    writeln!(out, "    app.tick(dom);").unwrap();
    // Publish the *actually resumed* value (not the literal declared
    // initial — resume may have recovered a different live value, e.g.
    // from a real capture) to the shared store, same as `setup()` does
    // for its own freshly-spawned value — see that call site's doc
    // comment for why this is the writer role's other seeding point.
    for shared_binding in &component.shared_bindings {
        writeln!(
            out,
            "    shared.set({:?}, app.world.get::<::loom_runtime::Signal<i32>>({}_entity).unwrap().0);",
            shared_binding.key, shared_binding.signal
        )
        .unwrap();
    }
    writeln!(out, "    Signals {{").unwrap();
    for s in &component.signals {
        writeln!(out, "        {}: {}_entity,", s.name, s.name).unwrap();
    }
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
}

enum BindingShape {
    TextI32,
    ListString,
    InputString,
}

fn collect_binding_shapes(node: &JsxNode) -> Vec<BindingShape> {
    let mut shapes = Vec::new();
    fn walk(node: &JsxNode, shapes: &mut Vec<BindingShape>) {
        match node {
            JsxNode::TextBinding { .. } => shapes.push(BindingShape::TextI32),
            JsxNode::ListBinding { .. } => shapes.push(BindingShape::ListString),
            JsxNode::InputBinding { .. } => shapes.push(BindingShape::InputString),
            JsxNode::Element { children, .. } => {
                for c in children {
                    walk(c, shapes);
                }
            }
            // Conditionals register their own per-site reconcile function
            // inline in `lower_node` (each site's spawned content differs,
            // so there's no single shared System to add here the way
            // `list_reconcile_k_system` is for every `ListBinding`).
            // **다만 안쪽은 훑어야 한다**(2026-08-07): 조건부 안의
            // `{signal()}`/`{signal().map(...)}`도 결국 `patch_text_system` /
            // `list_reconcile_k_system`이 돌봐야 하는 평범한 binding이고,
            // 그 등록은 여기서 모은 shape 목록으로 결정된다. 안 훑으면 조건부
            // 안에만 리스트가 있는 컴포넌트에서 System이 아예 등록되지 않는다.
            JsxNode::CondBinding { then_branch, .. } => walk(then_branch, shapes),
            // A referenced component's own patch systems are registered by
            // *its own* generated `setup()`, called via the emitted
            // `<mod>_generated::setup(...)` line — contributes nothing to
            // *this* component's own binding shapes.
            JsxNode::ComponentRef { .. } => {}
            // Read-only, no Signal/Entity/patch system at all — see this
            // variant's doc comment.
            JsxNode::SharedValueBinding { .. } => {}
            JsxNode::StaticText(_) | JsxNode::RowItemText => {}
        }
    }
    walk(node, &mut shapes);
    shapes
}

fn collect_actions(node: &JsxNode) -> Vec<ClickAction> {
    let mut actions = Vec::new();
    fn walk(node: &JsxNode, actions: &mut Vec<ClickAction>) {
        match node {
            JsxNode::Element { action, children, .. } => {
                if let Some(a) = action {
                    actions.push(a.clone());
                }
                for c in children {
                    walk(c, actions);
                }
            }
            // Synthesized, not user-written — every `InputBinding` implies
            // exactly one `SetString` action, the same way `collect_actions`
            // finds one `ClickAction` per `onClick`-bearing `Element`. Not
            // reachable via `Element`'s own `action` field since
            // `InputBinding` is its own `JsxNode` variant (see that type's
            // doc comment for why).
            JsxNode::InputBinding { signal } => {
                actions.push(ClickAction::SetString { signal: signal.clone() });
            }
            _ => {}
        }
    }
    walk(node, &mut actions);
    actions
}

struct CodegenCtx {
    node_counter: u32,
    signal_entities: Vec<(String, String)>, // (signal name, rust var holding its Entity)
    /// Counts `ActionN` variants in the exact tree order `collect_actions`
    /// (in `generate()`) assigned them — incremented by *both* the
    /// `Element` arm (onClick) and the `InputBinding` arm (synthesized
    /// `SetString`), since `collect_actions` counts both in one pass. Must
    /// stay a single shared counter, not two independent ones, or an
    /// `InputBinding` sitting between two onClick elements in the tree would
    /// desync this arm's `ActionN` naming from `collect_actions`'s.
    action_index: usize,
    cond_site_index: usize,
    /// Source of each `CondBinding` site's generated reconcile function,
    /// appended to `out` once at the end of `generate()` (top-level
    /// functions can't be emitted inline mid-`setup()`).
    cond_site_fns: String,
    /// (cond signal name, generated function name, marker component name) —
    /// `generate_resume` needs this to register the *same* per-site
    /// function `setup()` used (not the generic `cond_reconcile_f_system`,
    /// which doesn't know this site's actual static content) AND to attach
    /// the same `CondSiteN` marker its own spawned entity needs — see
    /// `generate_resume`'s `Bool` arm and the `CondBinding` arm's own doc
    /// comment for why every site needs a distinct marker.
    cond_site_fn_names: Vec<CondSite>,
    /// `true` while generating a non-root Island component (wrapped in its
    /// own `pub mod <name>_generated { ... }` by `generate_islands`). Every
    /// other Island's own generated module is a **flat sibling** at that
    /// same one level of `pub mod` nesting, regardless of how deeply nested
    /// its `<ComponentRef/>` usage is in the *JSX* tree (`generate_islands`
    /// iterates `ComponentGraph::components` — a flat list — emitting one
    /// `pub mod` per non-root component, never nesting one inside another).
    /// So a `ComponentRef` arm generated *while wrapped* must escape its own
    /// module with exactly one `super::` hop to reach a sibling; a
    /// `ComponentRef` arm generated for the *unqualified root* component
    /// needs no prefix at all, since the root's own code and every Island
    /// module already sit at the same top level. Exactly one `super::` hop
    /// is correct at any JSX nesting depth for this reason — not just for
    /// one level of nesting.
    wrapped_in_own_module: bool,
    /// Names of every component in the whole graph whose OWN
    /// `shared_bindings` is non-empty (precomputed once — see
    /// `generate_islands`) — used by `lower_node`'s `ComponentRef` arm to
    /// decide whether the generated call to that component's `setup()`
    /// needs a trailing shared-store argument. A **fresh, throwaway**
    /// `InMemorySharedStore` is passed at each such call site rather than
    /// forwarding a `shared` parameter through every caller up the chain —
    /// safe because `setup()` only ever runs once, during the SSR pass,
    /// and whatever it publishes to a store there is never read back (the
    /// SSR'd HTML's own content, e.g. `SharedValueBinding`'s `initial`
    /// literal, doesn't depend on it) — see this field's use site for the
    /// full reasoning. Empty for the legacy single-component `generate()`
    /// path (no graph, no `ComponentRef` ever meaningfully appears there).
    components_with_shared_bindings: HashSet<String>,
}

impl CodegenCtx {
    fn fresh_node_var(&mut self) -> String {
        let name = format!("node_{}", self.node_counter);
        self.node_counter += 1;
        name
    }

    fn fresh_cond_site_fn(&mut self) -> String {
        let name = format!("cond_reconcile_site_{}", self.cond_site_index);
        self.cond_site_index += 1;
        name
    }

    /// Must be called *before* `fresh_cond_site_fn` for the same site, so
    /// both share the same site index (`CondSite0`/`cond_reconcile_site_0`,
    /// etc.) — does not itself advance `cond_site_index`.
    fn fresh_cond_site_marker(&self) -> String {
        format!("CondSite{}", self.cond_site_index)
    }
}

/// Emits the statements that build `node`'s subtree as a child of the DOM
/// node named by `parent_var` (a variable already in scope holding a
/// `NodeHandle`). `row_ctx` carries the current row template variable name
/// (the runtime entity being spawned by the enclosing list system, and the
/// text expression to use for `RowItemText`) — non-`None` only while
/// generating a `.map(...)` row template, which is emitted as a *comment*
/// documenting the row shape (Phase 1's row instantiation itself is handled
/// generically at runtime by `list_reconcile_k_system`, not per binding
/// site — see loom-runtime's doc comment on that function).
/// Escapes exactly the same four characters `StringDom`'s own `html_escape`
/// does (`loom-fic`/`loom-resume-demo`) — kept in lockstep on purpose so a
/// blob rendered here at codegen time is byte-for-byte identical to what
/// the old node-by-node path would have produced through that runtime
/// escaping, for the same static content.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Returns `Some(html)` if `node` — and everything inside it — is fully
/// static: no `TextBinding`/`ListBinding`/`CondBinding`/`InputBinding`
/// anywhere, and no `Element` carrying an `onClick` action anywhere. Such a
/// subtree can be pre-rendered into one HTML string *at codegen time* and
/// handed to `DomOps::append_static_html` as a single call, instead of
/// walking it node-by-node at runtime — see that trait method's doc
/// comment for why this exists (docs/02's originally-stated design,
/// actually implemented as of 2026-07-24). `None` means "this node needs
/// its own `NodeHandle`/individual runtime calls" (it's a binding site, an
/// action target, or contains one) — `lower_node`'s normal per-kind match
/// handles those unchanged.
fn try_render_fully_static(node: &JsxNode) -> Option<String> {
    match node {
        JsxNode::StaticText(text) => Some(html_escape(text)),
        JsxNode::Element {
            tag,
            class,
            action: None,
            children,
        } => {
            let mut html = format!("<{tag}");
            if let Some(c) = class {
                html.push_str(&format!(" class=\"{}\"", html_escape(c)));
            }
            html.push('>');
            for child in children {
                html.push_str(&try_render_fully_static(child)?);
            }
            html.push_str("</");
            html.push_str(tag);
            html.push('>');
            Some(html)
        }
        _ => None,
    }
}

/// 조건부의 **지연 서브트리** 안에서 자기 entity를 가져야 하는 binding 하나.
/// `loom_runtime::CondNested`의 원소 하나에 1:1 대응하고, 순서는
/// `collect_lazy_slots`의 pre-order 순회로 컴파일타임에 고정된다 — 생성된
/// reconcile 함수는 `nested.0[<리터럴>]`로만 접근하므로 이름 조회도, 트리
/// 순회도 런타임에 남지 않는다.
#[derive(Clone, Debug)]
enum LazySlot {
    /// `{signal()}` — i32 텍스트 binding.
    Text { signal: String, initial: i32 },
    /// `{signal().map((row) => <li>{row}</li>)}` — 문자열 리스트 binding.
    List { signal: String, initial: Vec<String> },
}

impl LazySlot {
    fn signal(&self) -> &str {
        match self {
            LazySlot::Text { signal, .. } | LazySlot::List { signal, .. } => signal,
        }
    }
}

/// 조건부 사이트 하나에 대해 `generate_resume`이 알아야 하는 것 전부.
/// (2026-08-07 이전에는 `(String, String, String)` 튜플이었는데, 중첩 binding이
/// 생기면서 resume이 "이 사이트 안에 어떤 signal이 들어 있나"를 알아야 하게
/// 됐다 — 그 목록이 `nested`.)
struct CondSite {
    /// 조건 자체의 signal 이름.
    signal: String,
    fn_name: String,
    marker: String,
    /// `CondNested`의 순서와 정확히 같다.
    nested: Vec<LazySlot>,
}

/// 조건부 wrapper의 자식들을 pre-order로 훑어 `LazySlot`을 모은다.
/// `emit_lazy_subtree`가 **같은 순회를 같은 순서로** 하며 슬롯을 하나씩 소비
/// 하므로 인덱스가 어긋날 수 없다 — 그래도 어긋나면 `LazyEmit::take_slot`이
/// signal 이름 불일치로 그 자리에서 패닉한다(조용히 잘못된 entity를 쓰는 대신).
fn collect_lazy_slots(children: &[JsxNode], component: &Component, cond: &str) -> Vec<LazySlot> {
    let mut slots = Vec::new();
    for child in children {
        collect_lazy_slots_node(child, component, cond, &mut slots);
    }
    slots
}

fn collect_lazy_slots_node(node: &JsxNode, component: &Component, cond: &str, slots: &mut Vec<LazySlot>) {
    // 완전 정적 부분트리는 컴파일타임에 HTML 한 덩어리가 되므로 entity를
    // 하나도 만들지 않는다 — `emit_lazy_subtree`의 같은 위치 조기 반환과 짝.
    if matches!(node, JsxNode::Element { .. }) && try_render_fully_static(node).is_some() {
        return;
    }
    let lookup = |signal: &str| {
        &component
            .signals
            .iter()
            .find(|s| s.name == signal)
            .unwrap_or_else(|| panic!("loomc internal error: unknown signal {signal}"))
            .init
    };
    match node {
        JsxNode::StaticText(_) => {}
        JsxNode::TextBinding { signal } => {
            let SignalInit::I32(initial) = lookup(signal) else {
                panic!("loomc: `{{{signal}()}}` text binding requires an I32 signal");
            };
            slots.push(LazySlot::Text { signal: signal.clone(), initial: *initial });
        }
        JsxNode::ListBinding { signal, row, .. } => {
            let JsxNode::Element { children, .. } = row.as_ref() else {
                panic!("loomc: list row template must be a single element");
            };
            if !matches!(children.as_slice(), [JsxNode::RowItemText]) {
                panic!(
                    "loomc: Phase 1 only supports list rows shaped exactly as `<tag>{{item}}</tag>` (single text child referencing the row variable) — got {children:?}"
                );
            }
            let SignalInit::StringList(initial) = lookup(signal) else {
                panic!("loomc: list binding requires a string-list signal");
            };
            slots.push(LazySlot::List { signal: signal.clone(), initial: initial.clone() });
        }
        JsxNode::Element { action, children, .. } => {
            if action.is_some() {
                panic!(
                    "loomc: `{{{cond}() && ...}}` 안의 엘리먼트는 아직 자기 `onClick`을 가질 수 없다 — 지연 생성된 노드에 LRef 속성을 심는 경로가 아직 없다(codegen.rs의 `emit_lazy_subtree` 참고)"
                );
            }
            for c in children {
                collect_lazy_slots_node(c, component, cond, slots);
            }
        }
        other => panic!(
            "loomc: `{{{cond}() && ...}}` 안에서 아직 지원하지 않는 노드다 — 지원 범위는 정적 마크업, `{{signal()}}`, `{{signal().map(...)}}`, 그리고 그것들을 감싸는 엘리먼트다. 받은 것: {other:?}"
        ),
    }
}

/// `emit_lazy_subtree`가 들고 다니는 상태. 어떤 보조 쿼리가 필요한지를 본문
/// 생성 도중에 누적하므로, 함수 시그니처는 본문을 다 쓴 **뒤에** 만들어야 한다.
struct LazyEmit<'a> {
    slots: &'a [LazySlot],
    cursor: usize,
    tmp: usize,
    needs_i32: bool,
    needs_list: bool,
}

impl<'a> LazyEmit<'a> {
    fn new(slots: &'a [LazySlot]) -> Self {
        LazyEmit { slots, cursor: 0, tmp: 0, needs_i32: false, needs_list: false }
    }

    fn fresh_tmp(&mut self) -> String {
        let name = format!("lazy_{}", self.tmp);
        self.tmp += 1;
        name
    }

    /// 다음 슬롯을 소비하고, 그 entity를 담은 지역 변수 이름을 돌려준다.
    /// `signal`이 어긋나면 즉시 패닉 — `collect_lazy_slots`와 이 방출기의
    /// 순회가 갈라졌다는 뜻이고, 조용히 넘기면 엉뚱한 entity에 노드가 붙는다.
    fn take_slot(&mut self, out: &mut String, signal: &str, indent: &str) -> String {
        let idx = self.cursor;
        let slot = self.slots.get(idx).unwrap_or_else(|| {
            panic!("loomc internal error: lazy slot {idx} for `{signal}` is missing — collect_lazy_slots and emit_lazy_subtree disagree")
        });
        assert_eq!(
            slot.signal(),
            signal,
            "loomc internal error: lazy slot {idx} is `{}` but the emitter wants `{signal}`",
            slot.signal()
        );
        if matches!(slot, LazySlot::List { .. }) {
            self.needs_list = true;
        }
        self.cursor += 1;
        let var = format!("slot_{idx}");
        writeln!(out, "{indent}let {var} = nested.0[{idx}usize];").unwrap();
        var
    }

    /// 지연 생성된 리스트의 **초기 row들을 직접 방출**하고 `ListRows`를 시드한다.
    ///
    /// 왜 제네릭 `list_reconcile_k_system`에 맡기지 않는가: 그 System의 쿼리는
    /// `&DomTarget`을 요구하는데, 이 리스트의 `DomTarget`은 지금 이 배치가
    /// 적용될 때 비로소 붙는다. 그러니 `Signal`이 변경 표시된 틱(= `setup()`의
    /// spawn 틱)에는 쿼리가 매치하지 않았고, `DomTarget`이 생긴 다음 틱에는
    /// 이미 `Changed`가 지나가 있다 — 즉 초기 내용이 **영원히 렌더되지 않는다**.
    /// `SpawnRow`가 텍스트를 생성 시점에 굽는 것과 같은 이유로, 초기 상태는
    /// 여기서 실어 보내야 한다. 이후의 변경은 평범하게 K가 처리한다(그때는
    /// `DomTarget`이 이미 있다).
    fn emit_initial_rows(&mut self, out: &mut String, list_var: &str, parent_expr: &str, indent: &str) {
        self.needs_list = true;
        let rows = self.fresh_tmp();
        writeln!(out, "{indent}let mut {rows}: Vec<::bevy_ecs::entity::Entity> = Vec::new();").unwrap();
        writeln!(out, "{indent}if let Ok(list) = list_signals.get({list_var}) {{").unwrap();
        writeln!(out, "{indent}    for item in list.0.iter() {{").unwrap();
        writeln!(out, "{indent}        let row = commands.spawn_empty().id();").unwrap();
        writeln!(out, "{indent}        {rows}.push(row);").unwrap();
        writeln!(
            out,
            "{indent}        pending.0.push(::loom_runtime::DomWrite::SpawnElement {{ entity: row, parent: {parent_expr}, tag: \"li\", class: None, loom_id: None, text: Some(item.clone()) }});"
        )
        .unwrap();
        writeln!(out, "{indent}    }}").unwrap();
        writeln!(out, "{indent}}}").unwrap();
        writeln!(out, "{indent}commands.entity({list_var}).insert(::loom_runtime::ListRows({rows}));").unwrap();
    }

    fn finish(&self, slots: &[LazySlot]) {
        assert_eq!(
            self.cursor,
            slots.len(),
            "loomc internal error: {} lazy slots collected but {} emitted",
            slots.len(),
            self.cursor
        );
    }
}

/// 조건부가 처음 `true`가 될 때 실행될 문장들을 만든다. `lower_node`와 달리
/// 여기서 나오는 코드는 `setup()` 본문이 아니라 **생성된 reconcile System의
/// 본문**이다: `dom`이 없으므로 노드는 전부 `DomWrite`로 만들고, 아직 핸들이
/// 없는 부모는 `WriteParent::Entity`로 지목한다(같은 배치에서 부모가 자식보다
/// 먼저 방출되므로 적용 시점엔 항상 해석된다 — `SpawnRow`의 `before` 앵커와
/// 같은 규약).
fn emit_lazy_subtree(
    out: &mut String,
    node: &JsxNode,
    parent_expr: &str,
    cond: &str,
    emit: &mut LazyEmit,
    indent: &str,
) {
    // 완전 정적 부분트리는 컴파일타임에 HTML 한 문자열로 굽고 런타임 호출은
    // 한 번만 남긴다 — `lower_node`의 `try_render_fully_static` 접기와 동형.
    if matches!(node, JsxNode::Element { .. }) {
        if let Some(html) = try_render_fully_static(node) {
            writeln!(
                out,
                "{indent}pending.0.push(::loom_runtime::DomWrite::AppendStaticHtml {{ parent: {parent_expr}, html: {html:?}.to_string() }});"
            )
            .unwrap();
            return;
        }
    }
    match node {
        JsxNode::StaticText(text) => {
            writeln!(
                out,
                "{indent}pending.0.push(::loom_runtime::DomWrite::AppendStaticText {{ parent: {parent_expr}, text: {text:?}.to_string() }});"
            )
            .unwrap();
        }
        JsxNode::Element {
            tag,
            class,
            action,
            children,
        } => {
            if action.is_some() {
                panic!(
                    "loomc: `{{{cond}() && ...}}` 안의 엘리먼트는 아직 자기 `onClick`을 가질 수 없다 (codegen.rs의 `emit_lazy_subtree` 참고)"
                );
            }
            let class_arg = match class {
                Some(c) => format!("Some({c:?})"),
                None => "None".to_string(),
            };
            match children.as_slice() {
                // 이 엘리먼트 자체가 binding 노드다 — `lower_node`의 eager
                // 경로에서 `TextBinding`이 **부모 엘리먼트의 핸들**에 붙는 것과
                // 같은 의미다(엘리먼트에 동적 자식이 있으면 그 하나뿐이라는 것은
                // `Element` arm이 이미 강제한다).
                [JsxNode::TextBinding { signal }] => {
                    let var = emit.take_slot(out, signal, indent);
                    emit.needs_i32 = true;
                    writeln!(
                        out,
                        "{indent}pending.0.push(::loom_runtime::DomWrite::SpawnElement {{ entity: {var}, parent: {parent_expr}, tag: {tag:?}, class: {class_arg}, loom_id: Some({signal:?}), text: Some(i32_signals.get({var}).map(|s| s.0.to_string()).unwrap_or_default()) }});"
                    )
                    .unwrap();
                }
                [JsxNode::ListBinding { signal, .. }] => {
                    let var = emit.take_slot(out, signal, indent);
                    writeln!(
                        out,
                        "{indent}pending.0.push(::loom_runtime::DomWrite::SpawnElement {{ entity: {var}, parent: {parent_expr}, tag: {tag:?}, class: {class_arg}, loom_id: Some({signal:?}), text: None }});"
                    )
                    .unwrap();
                    let child_parent = format!("::loom_runtime::WriteParent::Entity({var})");
                    emit.emit_initial_rows(out, &var, &child_parent, indent);
                }
                // 그 밖은 순수 컨테이너 — 자기 signal이 없으므로 임시 entity를
                // 하나 만들어 자식들의 부모 지목에만 쓴다(이 분기는 사이트당
                // 한 번만 실행되므로 누적되지 않는다).
                _ => {
                    let tmp = emit.fresh_tmp();
                    writeln!(out, "{indent}let {tmp} = commands.spawn_empty().id();").unwrap();
                    writeln!(
                        out,
                        "{indent}pending.0.push(::loom_runtime::DomWrite::SpawnElement {{ entity: {tmp}, parent: {parent_expr}, tag: {tag:?}, class: {class_arg}, loom_id: None, text: None }});"
                    )
                    .unwrap();
                    let child_parent = format!("::loom_runtime::WriteParent::Entity({tmp})");
                    for child in children {
                        emit_lazy_subtree(out, child, &child_parent, cond, emit, indent);
                    }
                }
            }
        }
        other => panic!(
            "loomc: `{{{cond}() && ...}}` 안에서 아직 지원하지 않는 노드다 — 지원 범위는 정적 마크업, `{{signal()}}`, `{{signal().map(...)}}`, 그리고 그것들을 감싸는 엘리먼트다. 받은 것: {other:?}"
        ),
    }
}

fn lower_node(
    out: &mut String,
    node: &JsxNode,
    parent_var: &str,
    component: &Component,
    ctx: &mut CodegenCtx,
) {
    // Collapse a whole fully-static `Element` subtree into one
    // `append_static_html` call instead of recursing node-by-node — see
    // `try_render_fully_static`'s doc comment. Deliberately scoped to
    // `Element` only (not a bare top-level `StaticText`, which stays on
    // the existing `append_static_text` path below — that single-text-run
    // case is already O(1) per call, so there's nothing to collapse, and
    // keeping it on its original method avoids changing that arm's
    // observable call shape for no benefit).
    if matches!(node, JsxNode::Element { .. }) {
        if let Some(html) = try_render_fully_static(node) {
            writeln!(out, "    dom.append_static_html({parent_var}, {html:?});").unwrap();
            return;
        }
    }
    match node {
        JsxNode::StaticText(text) => {
            writeln!(out, "    dom.append_static_text({parent_var}, {text:?});").unwrap();
        }
        JsxNode::RowItemText => {
            // Row templates are not instantiated per-site by generated
            // code; see the ListBinding arm below.
            unreachable!("RowItemText must only appear inside a ListBinding row, handled there");
        }
        JsxNode::TextBinding { signal } => {
            let init = &component
                .signals
                .iter()
                .find(|s| &s.name == signal)
                .unwrap_or_else(|| panic!("loomc internal error: unknown signal {signal}"))
                .init;
            let SignalInit::I32(initial) = init else {
                panic!("loomc: `{{{signal}()}}` text binding requires an I32 signal");
            };
            let entity_var = format!("{signal}_entity");
            writeln!(
                out,
                "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({initial}_i32), ::loom_runtime::DomTarget({parent_var}))).id();"
            )
            .unwrap();
            writeln!(out, "    dom.set_attribute({parent_var}, \"data-loom-id\", {signal:?});").unwrap();
            ctx.signal_entities.push((signal.clone(), entity_var));
        }
        JsxNode::ListBinding { signal, row, .. } => {
            // Phase 1 scope: exactly one supported row shape — a single
            // element wrapping `RowItemText` (e.g. `<li>{item}</li>`).
            let JsxNode::Element { children, .. } = row.as_ref() else {
                panic!("loomc: list row template must be a single element");
            };
            if !matches!(children.as_slice(), [JsxNode::RowItemText]) {
                panic!(
                    "loomc: Phase 1 only supports list rows shaped exactly as `<tag>{{item}}</tag>` (single text child referencing the row variable) — got {children:?}"
                );
            }
            let init = &component
                .signals
                .iter()
                .find(|s| &s.name == signal)
                .unwrap_or_else(|| panic!("loomc internal error: unknown signal {signal}"))
                .init;
            let SignalInit::StringList(items) = init else {
                panic!("loomc: list binding requires a string-list signal");
            };
            let entity_var = format!("{signal}_entity");
            let items_rust = items
                .iter()
                .map(|s| format!("{s:?}.to_string()"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(
                out,
                "    let {entity_var} = app.world.spawn((::loom_runtime::Signal(vec![{items_rust}]), ::loom_runtime::ListRows::default(), ::loom_runtime::DomTarget({parent_var}))).id();"
            )
            .unwrap();
            writeln!(out, "    dom.set_attribute({parent_var}, \"data-loom-id\", {signal:?});").unwrap();
            ctx.signal_entities.push((signal.clone(), entity_var));
        }
        // `{cond() && <tag>...</tag>}` — RQ1 Track T candidate F
        // (`cond_reconcile_f_system`), but this compiler-generated site
        // needs its *own* reconcile function rather than reusing that
        // generic System: F's benchmark variant hardcodes its spawn content
        // (a fixed `<span>0</span>` per child, standing in for "whatever a
        // conditional subtree contains" for measurement purposes), whereas
        // a real compiled site has real, different static content per call
        // site. So the function generated below literally replicates F's
        // control flow (see cond.rs's doc comment) with this site's actual
        // `StaticText` children inlined as `DomWrite::AppendStaticText`
        // calls, instead of parameterizing `cond_reconcile_f_system` itself
        // — keeps that already-tested/benchmarked System untouched.
        JsxNode::CondBinding { cond, then_branch } => {
            let JsxNode::Element {
                tag,
                class,
                action,
                children,
            } = then_branch.as_ref()
            else {
                panic!("loomc internal error: CondBinding's then_branch must be an Element");
            };
            if action.is_some() {
                panic!(
                    "loomc: Phase 1 conditional content may not carry its own `onClick` — `{{{cond}() && <{tag} onClick=...>}}` is out of scope (the toggle button must be a separate sibling element, see codegen.rs's CondBinding arm)"
                );
            }

            // 이 사이트의 지연 서브트리 안에서 **entity가 필요한 binding**들을
            // 컴파일타임에 순서까지 확정한다 — 생성되는 reconcile 함수는 이
            // 순서를 `nested.0[<상수>]`로 그대로 인덱싱하므로 런타임 조회가
            // 하나도 남지 않는다(2026-08-07 "컴파일타임에 무겁게, 런타임은
            // 가볍게" 방향 결정).
            let slots = collect_lazy_slots(children, component, cond);

            let init = &component
                .signals
                .iter()
                .find(|s| &s.name == cond)
                .unwrap_or_else(|| panic!("loomc internal error: unknown signal {cond}"))
                .init;
            let SignalInit::Bool(initial) = init else {
                panic!("loomc: `{{{cond}() && ...}}` requires a Bool signal");
            };

            // The wrapper is created eagerly, same as any other static
            // element — it just starts with no children when `initial` is
            // `false` (an empty element is indistinguishable from "hidden"
            // without any extra bookkeeping, matching the hand-written
            // `ssr_render_full_app` proof this replaces). Its *content* is
            // spawned lazily, on the site's own reconcile function's first
            // `true` observation, exactly like `cond_reconcile_f_system`.
            let node_var = ctx.fresh_node_var();
            let class_arg = match class {
                Some(c) => format!("Some({c:?})"),
                None => "None".to_string(),
            };
            writeln!(
                out,
                "    let {node_var} = dom.create_element({parent_var}, {tag:?}, {class_arg});"
            )
            .unwrap();
            writeln!(out, "    dom.set_attribute({node_var}, \"data-loom-id\", {cond:?});").unwrap();

            // Each conditional site needs its OWN marker Component to keep
            // its reconcile System from matching every *other* site's
            // entity too. Without this, two or more `{cond() && ...}`
            // sites in the same component all spawn the same shape
            // (`Signal<bool>` + `CondBlock` + `DomTarget`, no other
            // distinguishing type) — a real bug found by actually building
            // a fixture with 3 conditionals in one component (RQ6 §4.2's
            // CheckoutForm): `Changed<Signal<bool>>` is tracked per-entity,
            // not per-System, so when *any* site's signal changes, *every*
            // site's `cond_reconcile_site_N` query matches that same
            // entity — each system would then apply *its own* hardcoded
            // static text to whichever entity the query call happens to
            // return, cross-wiring completely unrelated conditionals'
            // content. `With<CondSiteN>` scopes each system back to only
            // the one entity it actually owns. Declared at **top level**
            // (via `ctx.cond_site_fns`, spliced in after `setup()` closes —
            // same as the reconcile function itself), not inline in
            // `setup()`'s own body: a struct declared as a local item
            // inside one function has block scope, invisible to `resume()`
            // and to the top-level `cond_reconcile_site_N` function that
            // needs to name it in a `Query<..., With<CondSiteN>>` — a
            // second real bug found immediately after fixing the first
            // (this fix's own first attempt didn't compile/resume
            // correctly until caught).
            let marker_name = ctx.fresh_cond_site_marker();
            let mut marker_decl = String::new();
            writeln!(marker_decl, "#[derive(::bevy_ecs::prelude::Component)]").unwrap();
            writeln!(marker_decl, "struct {marker_name};").unwrap();
            writeln!(marker_decl).unwrap();
            ctx.cond_site_fns.push_str(&marker_decl);

            // 중첩 binding의 entity는 조건부가 보이든 안 보이든 **여기서 즉시**
            // 만든다. `DomTarget`만 빼고. 이유 두 가지:
            //
            //  (1) 조건부 안의 signal도 `Signals` 구조체·action·`resume()`에
            //      평범하게 참여해야 한다 — entity가 나중에 생기면 그 전부가
            //      "조건부 안이냐 밖이냐"를 런타임에 따져야 한다.
            //  (2) `DomTarget`이 없으면 patch System들의 쿼리(`&DomTarget` 요구)
            //      가 **archetype 수준에서** 이 entity를 제외한다 — 즉 "아직 DOM에
            //      없음"이 런타임 분기가 아니라 컴파일된 쿼리 필터로 처리된다
            //      (Bevy의 접근 선언 기반 스케줄링 재사용, CLAUDE.md 2-1).
            //
            // 조건부가 처음 `true`가 될 때 `DomWrite::SpawnElement`가 실제 노드를
            // 만들고 `DomTarget`을 붙이며, 그 순간부터 평범한 binding이 된다.
            let mut nested_vars: Vec<String> = Vec::new();
            for slot in &slots {
                let signal = slot.signal();
                let var = format!("{signal}_entity");
                match slot {
                    LazySlot::Text { initial, .. } => writeln!(
                        out,
                        "    let {var} = app.world.spawn((::loom_runtime::Signal({initial}_i32),)).id();"
                    )
                    .unwrap(),
                    LazySlot::List { initial, .. } => {
                        let items_rust = initial
                            .iter()
                            .map(|s| format!("{s:?}.to_string()"))
                            .collect::<Vec<_>>()
                            .join(", ");
                        writeln!(
                            out,
                            "    let {var} = app.world.spawn((::loom_runtime::Signal(vec![{items_rust}]), ::loom_runtime::ListRows::default())).id();"
                        )
                        .unwrap();
                    }
                }
                ctx.signal_entities.push((signal.to_string(), var.clone()));
                nested_vars.push(var);
            }

            let entity_var = format!("{cond}_entity");
            writeln!(
                out,
                "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({initial}), ::loom_runtime::CondBlock::with_size({size}), ::loom_runtime::DomTarget({node_var}), {marker_name}, ::loom_runtime::CondNested(vec![{nested}]))).id();",
                size = children.len(),
                nested = nested_vars.join(", ")
            )
            .unwrap();
            ctx.signal_entities.push((cond.clone(), entity_var));

            let fn_name = ctx.fresh_cond_site_fn();
            // 본문을 먼저 만든다 — 어떤 보조 쿼리가 필요한지(`i32_signals`/
            // `list_signals`)는 본문을 다 써 봐야 알 수 있고, 안 쓰는 쿼리를
            // 시그니처에 남기면 경고가 난다.
            let mut body = String::new();
            let mut emit = LazyEmit::new(&slots);
            match children.as_slice() {
                // wrapper 자신이 binding 노드인 경우(`{c() && <p>{n()}</p>}`) —
                // 노드는 이미 eager하게 있으므로 새로 만들지 않고 `DomTarget`만
                // 뒤늦게 붙인다. `SpawnElement`가 필요 없는 유일한 경우다.
                [JsxNode::TextBinding { signal }] => {
                    let var = emit.take_slot(&mut body, signal, "                ");
                    emit.needs_i32 = true;
                    writeln!(
                        body,
                        "                commands.entity({var}).insert(::loom_runtime::DomTarget(anchor.0));"
                    )
                    .unwrap();
                    writeln!(
                        body,
                        "                pending.0.push(::loom_runtime::DomWrite::SetText {{ target: anchor.0, text: i32_signals.get({var}).map(|s| s.0.to_string()).unwrap_or_default() }});"
                    )
                    .unwrap();
                }
                [JsxNode::ListBinding { signal, .. }] => {
                    let var = emit.take_slot(&mut body, signal, "                ");
                    writeln!(
                        body,
                        "                commands.entity({var}).insert(::loom_runtime::DomTarget(anchor.0));"
                    )
                    .unwrap();
                    emit.emit_initial_rows(&mut body, &var, "::loom_runtime::WriteParent::Node(anchor.0)", "                ");
                }
                _ => {
                    for child in children {
                        emit_lazy_subtree(
                            &mut body,
                            child,
                            "::loom_runtime::WriteParent::Node(anchor.0)",
                            cond,
                            &mut emit,
                            "                ",
                        );
                    }
                }
            }
            emit.finish(&slots);
            // `CondBlock.children`는 이 사이트의 생성 함수에서는 "이미 한 번
            // 채웠는가"(`is_empty()`)로만 쓰인다. 실제 binding entity를 여기
            // 넣지 않고 빈 entity를 자식 수만큼 넣는 것은 의도적이다 — 이
            // 벡터는 후보 B(`cond_reconcile_b_system`)에서 **despawn 대상**
            // 이기도 해서, signal entity가 섞여 들어가면 다른 경로에서
            // 살아 있어야 할 entity가 지워질 수 있다.
            writeln!(
                body,
                "                for _ in 0..{}usize {{ block.children.push(commands.spawn(()).id()); }}",
                children.len()
            )
            .unwrap();

            let mut site_fn = String::new();
            writeln!(site_fn, "fn {fn_name}(").unwrap();
            writeln!(
                site_fn,
                "    mut q: ::bevy_ecs::prelude::Query<(&::loom_runtime::Signal<bool>, &mut ::loom_runtime::CondBlock, &::loom_runtime::DomTarget, &::loom_runtime::CondNested), (::bevy_ecs::prelude::Changed<::loom_runtime::Signal<bool>>, ::bevy_ecs::prelude::With<{marker_name}>)>,"
            )
            .unwrap();
            if emit.needs_i32 {
                writeln!(
                    site_fn,
                    "    i32_signals: ::bevy_ecs::prelude::Query<&::loom_runtime::Signal<i32>>,"
                )
                .unwrap();
            }
            if emit.needs_list {
                writeln!(
                    site_fn,
                    "    list_signals: ::bevy_ecs::prelude::Query<&::loom_runtime::Signal<Vec<String>>>,"
                )
                .unwrap();
            }
            writeln!(site_fn, "    mut commands: ::bevy_ecs::prelude::Commands,").unwrap();
            writeln!(
                site_fn,
                "    mut pending: ::bevy_ecs::prelude::ResMut<::loom_runtime::PendingDomWrites>,"
            )
            .unwrap();
            writeln!(site_fn, ") {{").unwrap();
            writeln!(site_fn, "    for (cond, mut block, anchor, nested) in &mut q {{").unwrap();
            if slots.is_empty() {
                // 중첩 binding이 없는 사이트에서는 `nested`를 읽지 않는다 —
                // 쿼리에는 남기되(모든 사이트가 같은 shape이면 `resume()`도
                // 한 갈래로 끝난다) 미사용 경고만 막는다.
                writeln!(site_fn, "        let _ = nested;").unwrap();
            }
            writeln!(site_fn, "        if block.children.is_empty() {{").unwrap();
            writeln!(site_fn, "            if cond.0 {{").unwrap();
            site_fn.push_str(&body);
            writeln!(site_fn, "            }}").unwrap();
            writeln!(site_fn, "        }} else {{").unwrap();
            writeln!(
                site_fn,
                "            pending.0.push(::loom_runtime::DomWrite::SetVisible {{ node: anchor.0, visible: cond.0 }});"
            )
            .unwrap();
            writeln!(site_fn, "        }}").unwrap();
            writeln!(site_fn, "    }}").unwrap();
            writeln!(site_fn, "}}").unwrap();
            writeln!(site_fn).unwrap();
            ctx.cond_site_fns.push_str(&site_fn);
            ctx.cond_site_fn_names.push(CondSite {
                signal: cond.clone(),
                fn_name: fn_name.clone(),
                marker: marker_name.clone(),
                nested: slots,
            });

            writeln!(out, "    app.add_systems({fn_name});").unwrap();
        }
        // `<input value={s()} onInput={(e) => s.set(e.target.value)}/>` —
        // created eagerly like any static element (its initial `value` is
        // set via the same `set_attribute` path `class`/`data-loom-id` use
        // — see `DomOps::set_input_value`'s doc comment for why *ongoing*
        // writes need a different, property-setting operation instead).
        JsxNode::InputBinding { signal } => {
            let init = &component
                .signals
                .iter()
                .find(|s| &s.name == signal)
                .unwrap_or_else(|| panic!("loomc internal error: unknown signal {signal}"))
                .init;
            let SignalInit::Str(initial) = init else {
                panic!("loomc: <input> binding requires a Str signal");
            };

            let node_var = ctx.fresh_node_var();
            writeln!(out, "    let {node_var} = dom.create_element({parent_var}, \"input\", None);").unwrap();
            writeln!(out, "    dom.set_attribute({node_var}, \"value\", {initial:?});").unwrap();
            writeln!(out, "    dom.set_attribute({node_var}, \"data-loom-id\", {signal:?});").unwrap();

            let entity_var = format!("{signal}_entity");
            writeln!(
                out,
                "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({initial:?}.to_string()), ::loom_runtime::InputValueTarget, ::loom_runtime::DomTarget({node_var}))).id();"
            )
            .unwrap();
            ctx.signal_entities.push((signal.clone(), entity_var));

            // Shares `ctx.action_index` with the `Element` arm below rather
            // than keeping a separate counter — `collect_actions` (in
            // `generate()`) walks the tree in this exact same order and
            // counts *both* onClick-bearing Elements and InputBindings as it
            // goes (see that function), so incrementing one shared counter
            // at both sites is what keeps this arm's variant name in sync
            // with the `ActionN` name `collect_actions` already assigned
            // this exact `SetString` action. No capture needed (Str is
            // DOM-recovered, see `generate_resume`'s `SignalInit::Str` arm).
            let variant = format!("Action{}", ctx.action_index);
            ctx.action_index += 1;
            let lref = format!("{}_chunk|{variant}|", component.name);
            writeln!(out, "    dom.set_attribute({node_var}, \"data-loom-lref\", {lref:?});").unwrap();
            writeln!(out, "    input_actions.push(({node_var}, Action::{variant}));").unwrap();
        }
        JsxNode::Element {
            tag,
            class,
            action,
            children,
        } => {
            let has_dynamic = children
                .iter()
                .any(|c| matches!(c, JsxNode::TextBinding { .. } | JsxNode::ListBinding { .. }));
            if has_dynamic && children.len() > 1 {
                panic!(
                    "loomc: <{tag}> mixes a dynamic binding with other children — Phase 1 requires an element's content to be either all-static or exactly one dynamic binding. Split `{{...}}` into its own wrapping element."
                );
            }

            let node_var = ctx.fresh_node_var();
            let class_arg = match class {
                Some(c) => format!("Some({c:?})"),
                None => "None".to_string(),
            };
            writeln!(
                out,
                "    let {node_var} = dom.create_element({parent_var}, {tag:?}, {class_arg});"
            )
            .unwrap();

            if let Some(action) = action {
                let variant = format!("Action{}", ctx.action_index);
                ctx.action_index += 1;
                let value = match action {
                    ClickAction::Increment { .. } | ClickAction::Rotate { .. } | ClickAction::Toggle { .. } => {
                        format!("Action::{variant}")
                    }
                    ClickAction::Append { literal, .. } => {
                        format!("Action::{variant}({literal:?}.to_string())")
                    }
                    ClickAction::SetString { .. } => {
                        unreachable!(
                            "SetString is synthesized only for JsxNode::InputBinding (see collect_actions), \
                             never attached as an Element's own onClick action"
                        )
                    }
                };
                writeln!(out, "    actions.push(({node_var}, {value}));").unwrap();

                // Every action gets an LRef, and its captures are narrowed
                // to exactly the one signal *this* action reads/writes
                // (docs/02's "LRef 최소 캡처 집합" — see
                // `format_i32_capture_for_action`'s doc comment for the
                // exact reasoning and its scope caveat). `StringList`
                // actions (Append/Rotate) get an empty capture list — list
                // resume recovers row values from the DOM instead.
                let lref = format!(
                    "{}_chunk|{variant}|{}",
                    component.name,
                    format_i32_capture_for_action(component, action)
                );
                writeln!(out, "    dom.set_attribute({node_var}, \"data-loom-lref\", {lref:?});").unwrap();
            }

            for child in children {
                lower_node(out, child, &node_var, component, ctx);
            }
        }
        // Calls the referenced component's own generated `setup()` (in its
        // namespaced `pub mod <name>_generated { ... }` — see
        // `generate_islands`) to build *its* markup as a child of this
        // component's tree, using the *same* `app`/`dom` (one `LoomApp`/
        // `World` for the whole page's SSR pass — see `ir::ComponentGraph`'s
        // doc comment). Contributes zero entries to *this* component's own
        // `Signals`/`Action` (see `collect_actions`/`collect_binding_shapes`'
        // matching empty arms) — the referenced component resumes/dispatches
        // entirely independently later, via its own generated Island entry
        // point, never through this component's own `resume()`.
        JsxNode::ComponentRef { component: ref_name, .. } => {
            let sub_mod = sanitize_mod_name(ref_name);
            // See `CodegenCtx::wrapped_in_own_module`'s doc comment — every
            // Island module is a flat sibling regardless of JSX nesting
            // depth, so a reference generated *from inside* another
            // Island's own wrapped module needs exactly one `super::` hop;
            // a reference generated for the unqualified root needs none.
            let path_prefix = if ctx.wrapped_in_own_module { "super::" } else { "" };
            // If the target authors `sharedSignal(...)` itself, its own
            // `setup()` needs a trailing shared-store argument (see
            // `CodegenCtx::components_with_shared_bindings`'s doc comment
            // for why a fresh throwaway store here — not a `shared`
            // parameter threaded through *this* component's own signature
            // — is correct: `setup()` only ever runs once, at SSR time,
            // and nothing reads back what it publishes).
            let shared_arg = if ctx.components_with_shared_bindings.contains(ref_name) {
                ", &mut ::loom_runtime::InMemorySharedStore::default()"
            } else {
                ""
            };
            writeln!(
                out,
                "    let _ = {path_prefix}{sub_mod}_generated::setup(app, dom, {parent_var}{shared_arg});"
            )
            .unwrap();
        }
        // `{sharedValue("cartCount", 0)}` — a plain static-text node at SSR
        // time (the `initial` fallback literal) tagged with
        // `data-loom-shared-subscriber` so `shared-store.js`'s existing,
        // unmodified DOM-marker patch mechanism picks it up — no
        // Signal/Entity/DomTarget/patch system involved at all (see this
        // variant's doc comment).
        JsxNode::SharedValueBinding { key, initial } => {
            let node_var = ctx.fresh_node_var();
            writeln!(
                out,
                "    let {node_var} = dom.create_element({parent_var}, \"span\", None);"
            )
            .unwrap();
            writeln!(
                out,
                "    dom.set_attribute({node_var}, \"data-loom-shared-subscriber\", {key:?});"
            )
            .unwrap();
            writeln!(out, "    dom.append_static_text({node_var}, {:?});", initial.to_string()).unwrap();
        }
    }
}
