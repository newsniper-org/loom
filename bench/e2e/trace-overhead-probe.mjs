// **개정 #10 공평성 진술 4(c)의 근거 프로브** (2026-09-27, v22 수집 전) — 측정 데이터가 아니다.
//
// 질문: GC 트레이싱(v8+cppgc+user_timing)의 기록 비용은 GC 이벤트 수에 비례하므로, 쓰레기를 많이
// 만드는 후보(K)의 timed window에 더 얹혀 X/K 비율을 X에 유리하게 기울이는가?
//
// 방법: 같은 페이지에서 트레이싱 끔/켬을 블록 교대(40블록 = 인접 쌍 20, 블록당 60회)로 바꾸며
// K형(1,000행 전량 재생성)과 C형(1,000행 텍스트만 갱신) 조작 시간을 잰다. 인접 쌍 안의
// (C_on/K_on)/(C_off/K_off)는 두 부하에 공통인 표류를 약분한다.
//
// 결과(2026-09-27, Chrome stable, COOP/COEP 격리): C/K 비율 변화 median 0.993, 99% CI
// [0.892, 1.042] — 검출된 비중립 없음. **분해능은 ±5~10%**(블록 간 흔들림)라 수 % 기울기는 배제 못 함.
//
// 실행: bench/serve-isolated.py 8931 을 띄운 뒤  node bench/e2e/trace-overhead-probe.mjs
import { chromium } from "playwright";
const browser = await chromium.launch({ executablePath: "/opt/google/chrome/chrome", headless: true });
const page = await browser.newPage();
await page.goto("http://localhost:8931/research-questions/"); // COOP/COEP 헤더가 붙는 정적 목록 페이지(하네스 미실행)
console.log("crossOriginIsolated:", await page.evaluate(() => crossOriginIsolated));
await page.evaluate(() => { document.body.innerHTML = ""; });
const cdp = await page.context().newCDPSession(page);
let collected = 0;
cdp.on("Tracing.dataCollected", (e) => (collected += e.value.length));
const CATS = ["__loom_none", "v8", "cppgc", "blink.user_timing"];
const block = (kind) => page.evaluate(async (kind) => {
  const ul = document.createElement("ul"); document.body.appendChild(ul);
  for (let i = 0; i < 1000; i++) { const li = document.createElement("li"); li.textContent = "x" + i; ul.appendChild(li); }
  const ts = [];
  for (let r = 0; r < 60; r++) {
    const t0 = performance.now();
    if (kind === "K") {
      ul.textContent = "";
      for (let i = 0; i < 1000; i++) { const li = document.createElement("li"); li.textContent = "k" + r + ":" + i; ul.appendChild(li); }
    } else {
      const lis = ul.children;
      for (let i = 0; i < lis.length; i++) lis[i].firstChild.data = "c" + r + ":" + i;
    }
    ts.push(performance.now() - t0);
    await new Promise((res) => requestAnimationFrame(() => requestAnimationFrame(res)));
  }
  ul.remove();
  ts.sort((a, b) => a - b);
  return { med: ts[ts.length >> 1], mean: ts.reduce((a, b) => a + b, 0) / ts.length };
}, kind);
const res = { K: { on: [], off: [] }, C: { on: [], off: [] } };
const pairs = [];
for (let b = 0; b < 40; b++) {
  const tracing = b % 2 === 1;
  let done;
  if (tracing) {
    done = new Promise((r) => cdp.once("Tracing.tracingComplete", r));
    await cdp.send("Tracing.start", { traceConfig: { includedCategories: CATS }, transferMode: "ReportEvents" });
  }
  const cur = {};
  for (const kind of b % 4 < 2 ? ["K", "C"] : ["C", "K"]) {
    const m = await block(kind);
    res[kind][tracing ? "on" : "off"].push(m.med);
    cur[kind] = m.med;
  }
  if (!tracing) pairs.push({ off: cur });
  else pairs[pairs.length - 1].on = cur;
  if (tracing) { await cdp.send("Tracing.end"); await done; }
}
await browser.close();
const med = (xs) => { const v = xs.slice().sort((a, b) => a - b); return v[v.length >> 1]; };
for (const k of ["K", "C"]) {
  const on = med(res[k].on), off = med(res[k].off);
  console.log(`${k}형: 트레이싱 끔 median ${off.toFixed(4)}ms / 켬 ${on.toFixed(4)}ms → 켬/끔 ${(on / off).toFixed(4)}  (블록 ${res[k].on.length}+${res[k].off.length})`);
  console.log(`   끔 블록들: ${res[k].off.map((x) => x.toFixed(3)).join(" ")}`);
  console.log(`   켬 블록들: ${res[k].on.map((x) => x.toFixed(3)).join(" ")}`);
}
const kr = med(res.K.on) / med(res.K.off), cr = med(res.C.on) / med(res.C.off);
console.log(`C/K 비율이 트레이싱으로 움직이는 정도(블록 median) ≈ ${(cr / kr).toFixed(4)} (1이면 중립) · 수집 이벤트 ${collected}`);
// 인접 끔/켬 블록 쌍 안에서: shift_i = (C_on/K_on)/(C_off/K_off) — 두 부하에 공통인 표류가 약분된다.
const shifts = pairs.filter((p) => p.on).map((p) => (p.on.C / p.on.K) / (p.off.C / p.off.K));
const kOnly = pairs.filter((p) => p.on).map((p) => p.on.K / p.off.K);
const cOnly = pairs.filter((p) => p.on).map((p) => p.on.C / p.off.C);
let seed = 12345; const rnd = () => ((seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648);
const boot = (xs) => { const ms = []; for (let i = 0; i < 10000; i++) { const s = []; for (let j = 0; j < xs.length; j++) s.push(xs[Math.floor(rnd() * xs.length)]); ms.push(med(s)); } ms.sort((a, b) => a - b); return [ms[49], ms[9950]]; };
const [lo, hi] = boot(shifts), [klo, khi] = boot(kOnly), [clo, chi] = boot(cOnly);
console.log(`쌍 ${shifts.length}개 — C/K 비율 변화 median ${med(shifts).toFixed(4)}  99% CI [${lo.toFixed(4)}, ${hi.toFixed(4)}]`);
console.log(`   K 켬/끔 median ${med(kOnly).toFixed(4)} [${klo.toFixed(4)}, ${khi.toFixed(4)}] · C 켬/끔 median ${med(cOnly).toFixed(4)} [${clo.toFixed(4)}, ${chi.toFixed(4)}]`);
