// RQ4's minimal cross-island communication mechanism (research-questions/
// RQ4-cross-island-shared-state.md) — a tiny, always-loaded (not lazy — see
// roadmap/PHASES.md Phase 3's RQ4 section for why lazy-loading a
// subscription mechanism is worse than just shipping this, small enough
// not to matter) shared value store + DOM-marker-based subscription.
//
// The key property this proves: reading a shared value never requires
// loading the *reading* Island's own wasm chunk at all — only writing one
// does (see demo/islands/index.html's CartBadge, which has no Island/chunk
// of its own, and crates/loom-island-demo's QuantityPicker, which calls
// `set` via a `wasm_bindgen` extern import after its own quantity changes).
// This is deliberately NOT LRef-gated the way `bootstrap.js`'s action
// dispatch is — a subscriber is just a `[data-loom-shared-subscriber]`
// marker element, patched directly by plain JS, no chunk load implied.
//
// Kept schema-minimal on purpose, mirroring RQ3's IslandRegistry "폐쇄"
// discipline (research-questions/RQ4-cross-island-shared-state.md's
// "IslandRegistry 스키마 폐쇄 경계" note) — this is a *separate* mechanism
// from the Registry, not a backdoor into it.
window.__loomShared = (() => {
  const values = new Map();

  function set(key, value) {
    values.set(key, value);
    document.querySelectorAll(`[data-loom-shared-subscriber="${key}"]`).forEach((el) => {
      el.textContent = String(value);
    });
  }

  function get(key) {
    return values.get(key);
  }

  return { set, get };
})();
