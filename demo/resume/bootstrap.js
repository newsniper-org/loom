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
console.log("[bootstrap] loaded — 0 chunks requested yet");

document.addEventListener("click", async (event) => {
  const el = event.target.closest("[data-loom-lref]");
  if (!el) return;

  const [chunk, symbol, capturesRaw] = el.dataset.loomLref.split("|");

  // RQ3 "promote-once-and-stay": once a chunk has materialized a live
  // instance for this element, every later click reuses it directly —
  // no re-import, no re-materialization from the (by-then-stale) captured
  // snapshot.
  if (el._loomInstance) {
    el._loomInstance.dispatch(symbol);
    return;
  }

  console.log(`[bootstrap] first interaction — loading chunk "${chunk}" (symbol "${symbol}")`);
  let instance;
  try {
    const mod = await import(`./chunks/${chunk}.js`);
    await mod.default(); // wasm-bindgen's init(): fetches + instantiates the .wasm
    // RQ6 §4.1 fixture note: the conditional isn't compiler-generated (no
    // conditional codegen exists yet), so it's a second, hand-wired
    // resumable class sharing this same chunk file rather than routing
    // through `ResumedComponent` — this `symbol`-prefix check is fixture
    // glue, not a general multi-component dispatch mechanism.
    const ctor = symbol === "CondToggle" ? mod.ResumedCond : mod.ResumedComponent;
    instance = new ctor(capturesRaw);
  } catch (err) {
    console.error(`[bootstrap] failed to materialize chunk "${chunk}" — reloading:`, err);
    window.location.reload();
    return;
  }
  instance.dispatch(symbol);
  el._loomInstance = instance;
  console.log("[bootstrap] chunk materialized and applied; subsequent clicks reuse this instance");
});
