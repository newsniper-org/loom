// **개정 #10 후속 3 공평성 진술의 근거 프로브(Firefox)** (2026-10-01) — 측정 데이터가 아니다.
//
// 질문: Gecko 프로파일러(스택 샘플링 끔, 마커만, GeckoMain)를 켜면 GC 마커 기록 비용이 쓰레기를 많이 만드는
// 후보(K형)에 더 얹혀 C/K 비율이 기우는가? Chromium의 `trace-overhead-probe.mjs`와 같은 설계 — 같은 페이지에서
// 프로파일러 끔/켬을 블록 교대(인접 쌍 20, 블록당 60회)로 바꾸며 K형(1,000행 전량 재생성)과 C형(텍스트만 갱신)을 잰다.
//
// 실행: bench/serve-isolated.py 8931 을 띄우고,
//   firefox --headless --no-remote --profile <빈 디렉터리> --remote-debugging-port 9444 --remote-allow-system-access about:blank
//   node bench/e2e/ff-profiler-overhead-probe.mjs
const port = 9444;
let ws;
for (let i = 0; i < 60 && !ws; i++) {
  try { const w = new WebSocket(`ws://127.0.0.1:${port}/session`); await new Promise((r, j) => { w.onopen = r; w.onerror = j; }); ws = w; }
  catch { await new Promise((r) => setTimeout(r, 500)); }
}
let id = 0; const pend = new Map();
ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id !== undefined && pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); } };
const send = (method, params = {}) => new Promise((r) => { const i = ++id; pend.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
await send("session.new", { capabilities: { alwaysMatch: {} } });
const content = (await send("browsingContext.getTree", {})).result.contexts[0].context;
const chrome = (await send("browsingContext.getTree", { "moz:scope": "chrome" })).result.contexts[0].context;
const ev = (ctx, expression) => send("script.evaluate", { expression, target: { context: ctx }, awaitPromise: true }).then((r) => r.result?.result?.value);
await send("browsingContext.navigate", { context: content, url: "http://localhost:8931/research-questions/", wait: "complete" });
console.log("crossOriginIsolated:", await ev(content, "crossOriginIsolated"));
await ev(content, `document.body.innerHTML = ""`);
const block = (kind) => ev(content, `(async () => {
  const ul = document.createElement("ul"); document.body.appendChild(ul);
  for (let i = 0; i < 1000; i++) { const li = document.createElement("li"); li.textContent = "x" + i; ul.appendChild(li); }
  const ts = [];
  for (let r = 0; r < 60; r++) {
    const t0 = performance.now();
    if ("${kind}" === "K") { ul.textContent = ""; for (let i = 0; i < 1000; i++) { const li = document.createElement("li"); li.textContent = "k" + r + ":" + i; ul.appendChild(li); } }
    else { const lis = ul.children; for (let i = 0; i < lis.length; i++) lis[i].firstChild.data = "c" + r + ":" + i; }
    ts.push(performance.now() - t0);
    await new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(res)));
  }
  ul.remove(); ts.sort((a, b) => a - b); return ts[ts.length >> 1];
})()`);
const pairs = [];
for (let b = 0; b < 40; b++) {
  const on = b % 2 === 1;
  if (on) await ev(chrome, `(async () => { await Services.profiler.StartProfiler(${8 * 1024 * 1024}, 10, ["nostacksampling"], ["GeckoMain"]); return true; })()`);
  const cur = {};
  for (const kind of b % 4 < 2 ? ["K", "C"] : ["C", "K"]) cur[kind] = await block(kind);
  if (on) { await ev(chrome, `(async () => { await Services.profiler.getProfileDataAsync(); Services.profiler.StopProfiler(); return true; })()`); pairs[pairs.length - 1].on = cur; }
  else pairs.push({ off: cur });
}
const med = (xs) => { const v = xs.slice().sort((a, b) => a - b); return v[v.length >> 1]; };
const shifts = pairs.filter((p) => p.on).map((p) => (p.on.C / p.on.K) / (p.off.C / p.off.K));
const kOnly = pairs.filter((p) => p.on).map((p) => p.on.K / p.off.K);
const cOnly = pairs.filter((p) => p.on).map((p) => p.on.C / p.off.C);
let seed = 12345; const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const boot = (xs) => { const ms = []; for (let i = 0; i < 10000; i++) { const s = []; for (let j = 0; j < xs.length; j++) s.push(xs[Math.floor(rnd() * xs.length)]); ms.push(med(s)); } ms.sort((a, b) => a - b); return [ms[49], ms[9950]]; };
const [lo, hi] = boot(shifts), [klo, khi] = boot(kOnly), [clo, chi] = boot(cOnly);
console.log(`쌍 ${shifts.length}개 — C/K 비율 변화 median ${med(shifts).toFixed(4)}  99% CI [${lo.toFixed(4)}, ${hi.toFixed(4)}]`);
console.log(`   K 켬/끔 median ${med(kOnly).toFixed(4)} [${klo.toFixed(4)}, ${khi.toFixed(4)}] · C 켬/끔 median ${med(cOnly).toFixed(4)} [${clo.toFixed(4)}, ${chi.toFixed(4)}]`);
await send("session.end", {});
process.exit(0);
