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
// `ResumedApp::new`, not here — this file no longer needs to know
// which signal a given action reads, only how to get the chunk loaded and
// forward the symbol.
//
// 2026-07-28: `ResumedApp`은 손코딩 `ResumedComponent`를 대체하는 이름 —
// loom-compiler의 `generate_flatten_entry_body`가 GraphMode::Flatten에서
// 자동 생성한다. 로직은 동일, 이름과 생성 주체만 바뀜
// (crates/loom-resume-demo/src/lib.rs 참고).
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
// crates/loom-resume-demo/src/lib.rs's `ResumedApp::dispatch` doc
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
//
// 2026-10-18 (Phase 7a 후속 F3, docs/09 R13·R14 수리 — 병익님 결정으로 7a에 포함):
// - R13: 예전에는 materialize한 인스턴스를 **lref 요소마다** `el._loomInstance`에 따로
//   캐시했다. 그래서 다른 요소를 처음 누르면 같은 청크를 다시 resume해 **새 World**가 생기고,
//   그 사이 바뀐 상태(카운트, 리스트, 열린 조건부)가 SSR 시점 캡처값으로 되돌아갔다(공허했던
//   Phase 2 검증 — 매 클릭이 그 요소의 첫 클릭이었다). 이제 **청크당 인스턴스 하나**를
//   Promise로 캐시해, 페이지당 resume은 청크마다 정확히 한 번이다(동시 첫 클릭도 한 번).
//   bootstrap-flat.js(widget-embed Flatten 팔)가 이미 쓰던 공유 방식과 같은 원리다.
//   panic 봉쇄의 단위도 그에 맞춰 요소 → 청크로 바뀐다: 공유 인스턴스가 throw하면 그 청크를
//   쓰는 모든 요소가 같은 (손상됐을 수 있는) World를 보므로 청크 전체를 failed로 둔다.
// - R14: `<input>`은 lref를 `input` 이벤트용으로만 단다(컴파일러가 input에 허용하는 것은
//   value/onInput 바인딩뿐). 그런데 click 위임이 input 클릭에도 값 없이 dispatch해서
//   `SetString("")`이 되어 입력값이 지워졌다. click 위임에서 텍스트 입력 요소를 건너뛴다.
//   (JS만 고친다 — 컴파일러 산출물은 바이트 동일.)
// 측정 관계: RQ6 M1 Layer 0이 잰 것은 이 파일의 16c7e10판뿐이고(49b8660에서 이미 바뀜),
// 이 파일을 로드하는 측정 스펙은 없다(bench/e2e/specs grep) — 측정 산출물이 아니다.
const chunkInstances = new Map(); // chunk -> Promise<instance | null>
const failedChunks = new Set();

function dispatchToInstance(chunk, instance, symbol, value) {
  try {
    instance.dispatch(symbol, value);
  } catch (err) {
    console.error(
      `[bootstrap] dispatch("${symbol}") threw — chunk "${chunk}"'s shared resumed instance is presumed corrupted; ` +
        "no further dispatches will be attempted against it (see bootstrap.js's panic-containment note):",
      err
    );
    failedChunks.add(chunk);
    chunkInstances.delete(chunk);
  }
}

function materialize(chunk, capturesRaw) {
  let p = chunkInstances.get(chunk);
  if (p) return p;
  console.log(`[bootstrap] first interaction — loading chunk "${chunk}"`);
  p = (async () => {
    try {
      const mod = await import(`./chunks/${chunk}.js`);
      await mod.default(); // wasm-bindgen's init(): fetches + instantiates the .wasm
      return new mod.ResumedApp(capturesRaw);
    } catch (err) {
      console.error(`[bootstrap] failed to materialize chunk "${chunk}" — reloading:`, err);
      window.location.reload();
      return null;
    }
  })();
  chunkInstances.set(chunk, p);
  return p;
}

async function dispatchLref(el, value) {
  const [chunk, symbol, capturesRaw] = el.dataset.loomLref.split("|");
  if (failedChunks.has(chunk)) {
    console.error(
      `[bootstrap] chunk "${chunk}" previously failed a dispatch — ignoring further interactions on it ` +
        "(no recovery mechanism exists yet, see roadmap/PHASES.md Phase 3's wasm panic containment section)"
    );
    return;
  }
  // RQ3 "promote-once-and-stay": 청크가 한 번 materialize되면 이후 모든 상호작용(어느 요소든)이
  // 그 인스턴스를 그대로 쓴다 — 재import·재materialize 없음. 첫 요소의 캡처만 resume에 쓰인다
  // (그 뒤 요소들의 캡처는 이미 낡은 SSR 스냅샷이므로 쓰면 안 된다 — R13의 원인이 바로 그것).
  const instance = await materialize(chunk, capturesRaw);
  if (!instance || failedChunks.has(chunk)) return;
  dispatchToInstance(chunk, instance, symbol, value);
}

console.log("[bootstrap] loaded — 0 chunks requested yet");

document.addEventListener("click", (event) => {
  const el = event.target.closest("[data-loom-lref]");
  if (!el) return;
  // R14: 텍스트 입력 요소의 lref는 `input` 이벤트 전용이다 — click으로 dispatch하면 값 없이
  // `SetString`이 실행돼 입력값이 지워진다.
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") return;
  // `value` omitted -> `undefined` -> wasm-bindgen's `Option<String>` sees
  // `None` -> every non-`SetString` action ignores it, same as before.
  dispatchLref(el);
});

document.addEventListener("input", (event) => {
  const el = event.target.closest("[data-loom-lref]");
  if (!el) return;
  dispatchLref(el, event.target.value);
});
