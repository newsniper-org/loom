// RQ3's IslandRegistry (research-questions/RQ3-island-supervision-
// interaction.md) — the authoritative, wasm-target implementation.
//
// Why this is JS and not a Rust struct (see crates/loom-runtime/src/
// island.rs's module doc comment for the full finding): separate Islands
// are compiled to separate wasm chunks, each loaded via its own dynamic
// `import()` (see bootstrap.js's precedent) — that makes them separate wasm
// *instances* with disjoint linear memory. A Rust `HashMap` living inside
// one Island's instance is invisible to every other instance. The one
// thing every instance CAN reach is plain JS state on `window`, the same
// way `bootstrap.js` already tracks `el._loomInstance` there. So this file
// mirrors `loom_runtime::island::IslandRegistryBackend`'s state machine
// 1:1 — same six states, same transitions — but as an informal JS
// duplication, not a type-checked one. Keeping two representations in sync
// by hand is a real, accepted cost of the wasm-instance boundary (see
// roadmap/PHASES.md's Phase 3 status), not an oversight.
window.__loomRegistry = (() => {
  const entries = new Map(); // id -> { host, state, chunk, ctorName, instance }

  function register(id, host, chunk, ctorName) {
    if (entries.has(id)) {
      throw new Error(`[registry] island "${id}" registered twice`);
    }
    // RQ3: "등록 시점: SSR 마커 스캔/로드 시점" — registered as `dormant`
    // immediately, at page-scan time, not deferred until activation (the
    // bug this fixes: a never-yet-activated island used to be missing from
    // containment/detach lookups entirely).
    entries.set(id, { host, state: "dormant", chunk, ctorName, instance: null });
  }

  function getState(id) {
    const e = entries.get(id);
    return e ? e.state : undefined;
  }

  async function activate(id) {
    const e = entries.get(id);
    if (!e) throw new Error(`[registry] island "${id}" was never registered`);
    if (e.state !== "dormant") return; // e.g. an IntersectionObserver re-firing — no-op, not an error.
    e.state = "attaching";
    console.log(`[registry] activating island "${id}" (chunk "${e.chunk}")`);
    let mod, instance;
    try {
      mod = await import(`../islands/chunks/${e.chunk}.js`);
      await mod.default(); // wasm-bindgen's init(): fetches + instantiates the .wasm
      const ctor = mod[e.ctorName];
      instance = new ctor(e.host);
    } catch (err) {
      console.error(`[registry] island "${id}" failed to activate:`, err);
      e.state = "failed";
      return;
    }
    e.instance = instance;
    e.state = "attached";
    console.log(`[registry] island "${id}" attached`);
  }

  // RQ3 Q3's 2-phase detach, synchronous half.
  function beginDetach(id) {
    const e = entries.get(id);
    if (!e) return;
    if (e.state === "dormant" || e.state === "failed") {
      // "actor 비경유 환경 전이" — no live actor exists to notify.
      e.state = "detached";
      return;
    }
    if (e.state === "detaching" || e.state === "detached") {
      throw new Error(`[registry] island "${id}" began detach twice (already ${e.state})`);
    }
    if (e.instance && typeof e.instance.begin_detach === "function") {
      e.instance.begin_detach();
    }
    e.state = "detaching";
  }

  // The async half completing. This minimal Phase 3 slice has no in-flight
  // async cleanup work to actually wait for (RQ3's "알려진 약점" #5 — timeout
  // semantics for that case are still undecided), so callers here invoke it
  // synchronously right after `beginDetach` — a stated simplification, not
  // a claim that async cleanup is solved.
  function finishDetach(id) {
    const e = entries.get(id);
    if (!e) return;
    e.state = "detached";
    e.instance = null;
  }

  // RQ3 Q3's detach cascade: removing `id`'s host also removes every
  // Island nested inside it, so each nested id gets its own independent
  // `beginDetach`/`finishDetach` call (never a single "cascade" API on the
  // Registry itself — see `IslandRegistryBackend`'s doc comment on why
  // there is deliberately no parent/child bookkeeping there; the caller,
  // here, is the one walking the DOM to find what's in range).
  function detachAndRemove(id) {
    const e = entries.get(id);
    if (!e) return;
    const nestedIds = Array.from(e.host.querySelectorAll("[data-loom-island]")).map((el) => el.dataset.loomIslandId);
    for (const nestedId of nestedIds) beginDetach(nestedId);
    beginDetach(id);
    e.host.remove(); // synchronous — "죽은 DOM을 만질 수 있는 창"을 0으로.
    for (const nestedId of nestedIds) finishDetach(nestedId);
    finishDetach(id);
  }

  return { register, getState, activate, beginDetach, finishDetach, detachAndRemove, _entries: entries };
})();

// Scans the page for `[data-loom-island]` elements, registers each, and
// wires its activation trigger — `data-loom-activation="load"` fires
// immediately; `"visible"` fires on the host's first IntersectionObserver
// hit (Astro's `client:load`/`client:visible` equivalents, see roadmap/
// PHASES.md Phase 3 status for why only these two are implemented).
function bootstrapIslands() {
  const hosts = document.querySelectorAll("[data-loom-island]");
  const visibleObserver = new IntersectionObserver(
    (observed) => {
      for (const observedEntry of observed) {
        if (!observedEntry.isIntersecting) continue;
        const id = observedEntry.target.dataset.loomIslandId;
        window.__loomRegistry.activate(id);
        visibleObserver.unobserve(observedEntry.target);
      }
    },
    { threshold: 0.1 }
  );

  for (const host of hosts) {
    const id = host.dataset.loomIslandId;
    const chunk = host.dataset.loomChunk;
    const ctorName = host.dataset.loomCtor;
    const activation = host.dataset.loomActivation;
    window.__loomRegistry.register(id, host, chunk, ctorName);

    if (activation === "load") {
      window.__loomRegistry.activate(id);
    } else if (activation === "visible") {
      visibleObserver.observe(host);
    } else {
      throw new Error(`[registry] island "${id}" has unknown data-loom-activation "${activation}"`);
    }
  }

  watchForHostInitiatedRemoval();
}

// RQ3's "알려진 약점" 6번의 실제 완화 시도: `detachAndRemove` is the
// "명시적 unmount API" primary contract (RQ3 Q3), but a host page that
// removes an island's DOM through some *other* means (a legacy jQuery-style
// script calling `.remove()` directly, a framework's own reconciler, a
// route change that swaps a whole page section) would never call it,
// silently leaving the Registry's entry stuck at `attached`/`dormant`
// forever — a zombie. This `MutationObserver` catches that case too, from
// outside any single removal path.
function watchForHostInitiatedRemoval() {
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      for (const removedNode of mutation.removedNodes) {
        if (!(removedNode instanceof Element)) continue;
        const hosts = [];
        if (removedNode.matches?.("[data-loom-island]")) hosts.push(removedNode);
        if (removedNode.querySelectorAll) hosts.push(...removedNode.querySelectorAll("[data-loom-island]"));

        for (const hostEl of hosts) {
          const id = hostEl.dataset.loomIslandId;
          const state = window.__loomRegistry.getState(id);
          // Already handled by an explicit `detachAndRemove` call in the
          // same synchronous task (MutationObserver callbacks are always
          // microtasks, so that call — if any — already ran by now), or an
          // id this registry never saw — either way, nothing to do.
          if (state === undefined || state === "detaching" || state === "detached") continue;
          console.warn(
            `[registry] island "${id}" was removed from the DOM without going through detachAndRemove() ` +
              "-- detected via MutationObserver instead (RQ3 알려진 약점 6번 완화)"
          );
          window.__loomRegistry.beginDetach(id);
          window.__loomRegistry.finishDetach(id);
        }
      }
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
}

document.addEventListener("DOMContentLoaded", bootstrapIslands);
