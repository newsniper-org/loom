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

use crate::ir::{ClickAction, Component, JsxNode, SignalInit};
use std::fmt::Write as _;

pub fn generate(component: &Component) -> String {
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

    writeln!(
        out,
        "pub fn apply_action(app: &mut ::loom_runtime::LoomApp, signals: &Signals, action: &Action) {{"
    )
    .unwrap();
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
    writeln!(out, "    }}").unwrap();
    writeln!(out, "}}").unwrap();
    writeln!(out).unwrap();

    writeln!(
        out,
        "pub fn setup(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mount: ::loom_runtime::NodeHandle) -> (Signals, Vec<(::loom_runtime::NodeHandle, Action)>, Vec<(::loom_runtime::NodeHandle, fn(String) -> Action)>) {{"
    )
    .unwrap();

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

    let mut ctx = CodegenCtx {
        node_counter: 0,
        signal_entities: Vec::new(),
        action_index: 0,
        cond_site_index: 0,
        cond_site_fns: String::new(),
        cond_site_fn_names: Vec::new(),
    };
    lower_node(&mut out, &component.root, "mount", component, &mut ctx);

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
    generate_resume(&mut out, component, &ctx.cond_site_fn_names);

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
fn generate_resume(out: &mut String, component: &Component, cond_site_fn_names: &[(String, String)]) {
    writeln!(
        out,
        "pub fn resume(app: &mut ::loom_runtime::LoomApp, dom: &mut impl ::loom_runtime::DomOps, mut find_node: impl FnMut(&str) -> ::loom_runtime::NodeHandle, captures: &[(&str, &str)]) -> Signals {{"
    )
    .unwrap();
    writeln!(out, "    app.add_systems(::loom_runtime::patch_text_system::<i32>);").unwrap();
    let has_list = component.signals.iter().any(|s| matches!(s.init, SignalInit::StringList(_)));
    if has_list {
        writeln!(out, "    app.add_systems(::loom_runtime::list_reconcile_k_system::<String>);").unwrap();
    }
    let has_input = component.signals.iter().any(|s| matches!(s.init, SignalInit::Str(_)));
    if has_input {
        writeln!(out, "    app.add_systems(::loom_runtime::patch_input_system);").unwrap();
    }
    for s in &component.signals {
        let entity_var = format!("{}_entity", s.name);
        match &s.init {
            SignalInit::I32(initial) => {
                writeln!(out, "    let {entity_var} = {{").unwrap();
                writeln!(
                    out,
                    "        let value: i32 = captures.iter().find(|(k, _)| *k == {:?}).and_then(|(_, v)| v.parse().ok()).unwrap_or({initial}_i32);",
                    s.name
                )
                .unwrap();
                writeln!(
                    out,
                    "        app.world.spawn((::loom_runtime::Signal(value), ::loom_runtime::DomTarget(find_node({:?})))).id()",
                    s.name
                )
                .unwrap();
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
                writeln!(
                    out,
                    "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({}_shown), ::loom_runtime::CondBlock {{ size: {}_children.len(), children: {}_children }}, ::loom_runtime::DomTarget({}_node))).id();",
                    s.name, s.name, s.name, s.name
                )
                .unwrap();
                let fn_name = cond_site_fn_names
                    .iter()
                    .find(|(signal, _)| signal == &s.name)
                    .map(|(_, f)| f.clone())
                    .unwrap_or_else(|| panic!("loomc internal error: no reconcile function recorded for conditional `{}`", s.name));
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
                writeln!(out, "    let {}_node = find_node({:?});", s.name, s.name).unwrap();
                writeln!(
                    out,
                    "    let {}_adopted: Vec<(::loom_runtime::NodeHandle, String)> = dom.adopt_children({}_node);",
                    s.name, s.name
                )
                .unwrap();
                writeln!(
                    out,
                    "    let {}_rows: Vec<::bevy_ecs::entity::Entity> = {}_adopted.iter().map(|(handle, text)| {{",
                    s.name, s.name
                )
                .unwrap();
                writeln!(
                    out,
                    "        app.world.spawn((::loom_runtime::Signal(text.clone()), ::loom_runtime::DomTarget(*handle))).id()"
                )
                .unwrap();
                writeln!(out, "    }}).collect();").unwrap();
                writeln!(
                    out,
                    "    let {}_values: Vec<String> = {}_adopted.into_iter().map(|(_, text)| text).collect();",
                    s.name, s.name
                )
                .unwrap();
                writeln!(
                    out,
                    "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({}_values), ::loom_runtime::ListRows({}_rows), ::loom_runtime::DomTarget({}_node))).id();",
                    s.name, s.name, s.name
                )
                .unwrap();
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
            JsxNode::CondBinding { .. } => {}
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
    /// (cond signal name, generated function name) — `generate_resume`
    /// needs this to register the *same* per-site function `setup()` used,
    /// not the generic `cond_reconcile_f_system` (which doesn't know this
    /// site's actual static content) — see `generate_resume`'s `Bool` arm.
    cond_site_fn_names: Vec<(String, String)>,
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
fn lower_node(
    out: &mut String,
    node: &JsxNode,
    parent_var: &str,
    component: &Component,
    ctx: &mut CodegenCtx,
) {
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
            let texts: Vec<&str> = children
                .iter()
                .map(|c| match c {
                    JsxNode::StaticText(t) => t.as_str(),
                    other => panic!(
                        "loomc internal error: conditional content must be static-only (lower.rs should already reject this) — got {other:?}"
                    ),
                })
                .collect();

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

            let entity_var = format!("{cond}_entity");
            writeln!(
                out,
                "    let {entity_var} = app.world.spawn((::loom_runtime::Signal({initial}), ::loom_runtime::CondBlock::with_size({size}), ::loom_runtime::DomTarget({node_var}))).id();",
                size = texts.len()
            )
            .unwrap();
            ctx.signal_entities.push((cond.clone(), entity_var));

            let fn_name = ctx.fresh_cond_site_fn();
            let mut site_fn = String::new();
            writeln!(site_fn, "fn {fn_name}(").unwrap();
            writeln!(
                site_fn,
                "    mut q: ::bevy_ecs::prelude::Query<(&::loom_runtime::Signal<bool>, &mut ::loom_runtime::CondBlock, &::loom_runtime::DomTarget), ::bevy_ecs::prelude::Changed<::loom_runtime::Signal<bool>>>,"
            )
            .unwrap();
            writeln!(site_fn, "    mut commands: ::bevy_ecs::prelude::Commands,").unwrap();
            writeln!(
                site_fn,
                "    mut pending: ::bevy_ecs::prelude::ResMut<::loom_runtime::PendingDomWrites>,"
            )
            .unwrap();
            writeln!(site_fn, ") {{").unwrap();
            writeln!(site_fn, "    for (cond, mut block, anchor) in &mut q {{").unwrap();
            writeln!(site_fn, "        if block.children.is_empty() {{").unwrap();
            writeln!(site_fn, "            if cond.0 {{").unwrap();
            for text in &texts {
                writeln!(
                    site_fn,
                    "                pending.0.push(::loom_runtime::DomWrite::AppendStaticText {{ parent: anchor.0, text: {text:?}.to_string() }});"
                )
                .unwrap();
                writeln!(site_fn, "                block.children.push(commands.spawn(()).id());").unwrap();
            }
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
            ctx.cond_site_fn_names.push((cond.clone(), fn_name.clone()));

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
    }
}
