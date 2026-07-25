// RQ6 §4.2 H-Qwik 반사실 팔의 bootstrap — demo/resume/bootstrap.js와 동일한
// "전역 클릭 위임 후 첫 상호작용에서 청크 1회 materialize" 메커니즘(Qwik에서
// 재사용, CLAUDE.md 2-1)을 그대로 쓴다. 유일한 차이: 여기서 만드는 인스턴스는
// `mod.ResumedComponent`가 아니라 `mod.ResumedFlatApp`(crates/loom-widget-embed의
// GraphMode::Flatten 전용 wasm 엔트리, `ResumedComponent`와 동일 패턴을 재사용해
// 손코딩됨 — 그 struct의 doc comment 참고) — 5개 컴포넌트가 전부 하나의 merged
// World로 합쳐진 이 픽스처에서는 어느 위젯을 먼저 클릭하든 전체 앱이 한 번에
// materialize된다는 점 자체가 RQ6 M2/M3 측정의 핵심 관찰 대상.
function dispatchToInstance(el, instance, symbol, value) {
  try {
    instance.dispatch(symbol, value);
  } catch (err) {
    console.error(`[bootstrap-flat] dispatch("${symbol}") threw:`, err);
    el._loomInstance = undefined;
    el._loomFailed = true;
  }
}

async function dispatchLref(el, value) {
  if (el._loomFailed) {
    console.error("[bootstrap-flat] this element previously failed a dispatch — ignoring further interactions on it");
    return;
  }

  const [chunk, symbol, capturesRaw] = el.dataset.loomLref.split("|");

  if (el._loomInstance) {
    dispatchToInstance(el, el._loomInstance, symbol, value);
    return;
  }

  // M2/M3 계측 훅 — RQ6 §6 M2(위젯-로컬 TTI)/M3(wake-up payload) 실측용,
  // bench/e2e/specs/m2-m3-tti-wakeup.spec.ts가 이 마커를 읽는다.
  window.__loomWakeupStart = performance.now();
  console.log(`[bootstrap-flat] first interaction anywhere on the page — loading chunk "${chunk}" (symbol "${symbol}")`);
  let instance;
  try {
    const mod = await import(`./chunks/${chunk}.js`);
    await mod.default();
    instance = new mod.ResumedFlatApp(capturesRaw);
  } catch (err) {
    console.error(`[bootstrap-flat] failed to materialize chunk "${chunk}" — reloading:`, err);
    window.location.reload();
    return;
  }
  dispatchToInstance(el, instance, symbol, value);
  window.__loomWakeupEnd = performance.now();
  if (!el._loomFailed) {
    // H-Qwik 반사실의 핵심: 5개 컴포넌트 전부 같은 merged World이므로, 이
    // "첫 클릭" 하나가 전체 앱을 한 번에 깨운다 — 이후 어느 위젯을 클릭해도
    // 전부 이 하나의 instance를 공유(Island 팔의 "위젯별 독립 인스턴스"와
    // 대비되는 지점).
    window.__loomSharedInstance = instance;
    document.querySelectorAll("[data-loom-lref]").forEach((otherEl) => {
      if (!otherEl._loomInstance) otherEl._loomInstance = instance;
    });
    console.log("[bootstrap-flat] whole app materialized; every widget's subsequent clicks reuse this one instance");
  }
}

console.log("[bootstrap-flat] loaded — 0 bytes of app-specific wasm/js requested yet");

document.addEventListener("click", (event) => {
  const el = event.target.closest("[data-loom-lref]");
  if (!el) return;
  dispatchLref(el);
});
