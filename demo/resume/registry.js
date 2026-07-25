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

  // 실제로 발견한 버그(RQ6 §4.4 N-스케일링 실측 중, N=50 동시 client:load
  // 재현): wasm-bindgen이 생성하는 초기화 함수(`quantity_picker.js`의
  // `__wbg_init` 등)는 `if (wasm !== undefined) return wasm;`라는
  // check-then-act 패턴만 쓰고, "이미 초기화 진행 중"인 경우의 락/공유
  // Promise가 없다 — 같은 청크를 가리키는 N개의 `client:load` Island가
  // 전부 같은 tick에 activate()를 fire-and-forget으로 호출하면(아래
  // bootstrapIslands 참고), 첫 번째 초기화가 끝나기 *전에* 나머지 N-1개가
  // 전부 자기만의 fetch+instantiate를 또 시작해버려 서로 다른
  // WebAssembly.Instance가 여러 개 생기고, 그 중 가장 늦게 끝난 것이
  // 모듈 레벨 `wasm` 바인딩을 마지막으로 덮어씀 — 그 전에 이미 그 바인딩을
  // 참조해 생성된 인스턴스(닫힌 클로저가 아니라 재할당 가능한 바인딩을
  // 참조하므로)들은 이후 아무 에러 없이 조용히 오작동한다(클릭이 그냥
  // 아무 효과도 없음 — 실측: N=50에서 재현, 우연이 아니라 100% 재현됨).
  // 이 메모이제이션이 그 레이스를 원천 차단 — 같은 청크를 가리키는 모든
  // activate() 호출이 "import+init" 시퀀스를 정확히 한 번만 공유하게 함
  // (서로 다른 청크끼리는 여전히 병렬로 로드됨, 불필요한 직렬화 없음).
  const chunkInitPromises = new Map(); // chunk name -> Promise<module>

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
      // 위 chunkInitPromises 주석 참고 — 같은 청크를 가리키는 동시
      // activate() 호출들이 import+init을 정확히 한 번만 공유하도록.
      if (!chunkInitPromises.has(e.chunk)) {
        chunkInitPromises.set(
          e.chunk,
          (async () => {
            const m = await import(`../islands/chunks/${e.chunk}.js`);
            await m.default(); // wasm-bindgen's init(): fetches + instantiates the .wasm
            return m;
          })()
        );
      }
      mod = await chunkInitPromises.get(e.chunk);
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
  // 실제로 발견한 버그(RQ6 §6 M7 간섭-2 실측 중): 호스트가 `insertBefore`로
  // 어떤 조상을 "제거 후 같은 tick에 재삽입"하는 순수 재배치를 하면, 그
  // 조상은 DOM에서 한 번 떨어졌다가 다시 붙는 것이므로 MutationObserver의
  // `removedNodes`에 나타난다 — 그런데 MutationObserver 콜백은 마이크로태스크
  // 큐잉이라 그 시점엔 재삽입이 이미 끝나 있다. 즉 `removedNodes`에 있다는
  // 사실만으로 "진짜 제거됐다"고 판단하면, 순수 재배치를 오탐 detach로
  // 처리하게 된다(실측: 10/10 재현, 3개 브라우저 전부). 수정: 콜백 시점에
  // `hostEl.isConnected`를 다시 확인 — 아직 문서에 붙어 있으면(다른 위치로
  // 옮겨졌을 뿐) 진짜 제거가 아니므로 건너뛴다.
  const observer = new MutationObserver((mutations) => {
    for (const mutation of mutations) {
      for (const removedNode of mutation.removedNodes) {
        if (!(removedNode instanceof Element)) continue;
        const hosts = [];
        if (removedNode.matches?.("[data-loom-island]")) hosts.push(removedNode);
        if (removedNode.querySelectorAll) hosts.push(...removedNode.querySelectorAll("[data-loom-island]"));

        for (const hostEl of hosts) {
          if (hostEl.isConnected) continue; // 위 주석 참고 — 순수 재배치, 진짜 제거 아님.
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
