// Phase 2 — the "global delegation listener" docs/04 describes, reused from
// Qwik's mechanism (CLAUDE.md 2-1: don't reinvent an already-solved
// problem). This is the *entire* interactive-framework footprint that loads
// before any user interaction — no wasm, no generated app code, nothing
// from crates/loom-resume-demo.
//
// 2026-07-24: generalized to route by symbol instead of hard-coding
// "increment" — loom-compiler's codegen now emits `resume`/
// `resume_action_by_symbol` for any component (see codegen.rs), and finding
// each captured signal's DOM node (`data-loom-id`) happens Rust-side inside
// `ResumedComponent::new`, not here — this file no longer needs to know
// which signal a given action reads, only how to get the chunk loaded and
// forward the symbol.
//
// RQ2 (payload format) is still open — the `data-loom-lref` format parsed
// here (`chunk|symbol|key:value,...`) is a tentative pick, not RQ2's
// eventual answer (see crates/loom-resume-demo/src/lib.rs's module doc
// comment).
//
// 2026-07-24 — RQ2 sub-question 2 (stale chunk / version skew): a chunk
// reference can go bad between when this HTML was served and when the user
// finally clicks (redeploy rotated out the old chunk file, CDN cache
// weirdness, a long-open tab). There is no way to recover *this specific*
// interaction once that's happened — the same conclusion Qwik/Next reach —
// so the fallback here is a full reload, which trades away resumability's
// "no re-execution" promise for correctness in exactly this one failure
// case. A better answer is still open (RQ2).
// 2026-07-24 — `<input>` two-way bindings: a keystroke needs the exact same
// "materialize chunk once, then reuse" flow as a click, just on the `input`
// event instead and carrying the just-typed value along (see
// crates/loom-resume-demo/src/lib.rs's `ResumedComponent::dispatch` doc
// comment for why a value has to ride along at all — unlike every other
// action, `SetString`'s payload can't be known until this exact moment).
// Factored out so click/input share one materialize-and-cache path rather
// than duplicating it.
//
// 2026-07-24 (Phase 3, wasm panic containment prerequisite — roadmap/
// PHASES.md's "실제로 발견한 갭"): `instance.dispatch(...)` itself used to be
// called *outside* any try/catch, on both the cached-instance path and the
// freshly-materialized path — only the chunk import/wasm-init/constructor
// sequence was guarded. A panic inside `dispatch` (which runs arbitrary
// compiled component code via `apply_action`/`tick`) would throw uncaught
// out of this event listener. `console_error_panic_hook` turns a Rust panic
// into a JS exception rather than crashing the tab, but the wasm instance
// itself is left in an unspecified, possibly-unsound state afterward
// (stable-channel `catch_unwind` is a no-op here — see roadmap/PHASES.md
// Phase 3's wasm panic containment section, "condition A, unresolved" is
// the accepted default). `dispatchToInstance` now catches that, and
// deliberately stops dispatching to this *specific* element's instance
// again (RQ3 Q4's `attached -> failed` transition, done here at the host
// glue layer since there is no IslandRegistry in Phase 2's non-Island
// path) — this bounds the blast radius to the one element whose dispatch
// actually panicked, rather than leaving it undefined.
function dispatchToInstance(el, instance, symbol, value) {
  try {
    instance.dispatch(symbol, value);
  } catch (err) {
    console.error(
      `[bootstrap] dispatch("${symbol}") threw — this element's resumed instance is presumed corrupted; ` +
        "no further dispatches will be attempted against it (see bootstrap.js's panic-containment note):",
      err
    );
    el._loomInstance = undefined;
    el._loomFailed = true;
  }
}

async function dispatchLref(el, value) {
  if (el._loomFailed) {
    console.error(
      "[bootstrap] this element previously failed a dispatch — ignoring further interactions on it " +
        "(no recovery mechanism exists yet, see roadmap/PHASES.md Phase 3's wasm panic containment section)"
    );
    return;
  }

  const [chunk, symbol, capturesRaw] = el.dataset.loomLref.split("|");

  // RQ3 "promote-once-and-stay": once a chunk has materialized a live
  // instance for this element, every later interaction reuses it directly —
  // no re-import, no re-materialization from the (by-then-stale) captured
  // snapshot.
  if (el._loomInstance) {
    dispatchToInstance(el, el._loomInstance, symbol, value);
    return;
  }

  console.log(`[bootstrap] first interaction — loading chunk "${chunk}" (symbol "${symbol}")`);
  let instance;
  try {
    const mod = await import(`./chunks/${chunk}.js`);
    await mod.default(); // wasm-bindgen's init(): fetches + instantiates the .wasm
    // 2026-07-24: conditionals (`{cond() && <X/>}`) are compiler-generated
    // now (loom-compiler/src/codegen.rs), so their toggle is a normal
    // `Action` dispatched through the same `ResumedComponent` every other
    // action uses — no per-symbol special-casing needed here anymore.
    instance = new mod.ResumedComponent(capturesRaw);
  } catch (err) {
    console.error(`[bootstrap] failed to materialize chunk "${chunk}" — reloading:`, err);
    window.location.reload();
    return;
  }
  dispatchToInstance(el, instance, symbol, value);
  if (!el._loomFailed) {
    el._loomInstance = instance;
    console.log("[bootstrap] chunk materialized and applied; subsequent clicks reuse this instance");
  }
}

console.log("[bootstrap] loaded — 0 chunks requested yet");

document.addEventListener("click", (event) => {
  const el = event.target.closest("[data-loom-lref]");
  if (!el) return;
  // `value` omitted -> `undefined` -> wasm-bindgen's `Option<String>` sees
  // `None` -> every non-`SetString` action ignores it, same as before.
  dispatchLref(el);
});

document.addEventListener("input", (event) => {
  const el = event.target.closest("[data-loom-lref]");
  if (!el) return;
  dispatchLref(el, event.target.value);
});
