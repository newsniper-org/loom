// **트랙 L Firefox 수집의 GC 통제 에이전트 — WebDriver BiDi** (개정 #10 후속 3, 2026-10-01)
//
// §8 격리(M-15): "성능·메모리 셀 시작 시 CDP 강제 GC 1회, 측정 중 GC 이벤트 로그(이상치 제거 없이 GC 주석 병기)".
// Firefox는 CDP를 지원하지 않아 개정 #10에서 "unavailable"(§8 스로틀 한계와 같은 성격)로 등재했다. 병익님 제안으로
// WebDriver BiDi를 확인한 결과(2026-10-01 프로브, Firefox 155.0.1) Firefox의 원격 에이전트가 `--remote-allow-system-access`
// 아래에서 **chrome(특권) 컨텍스트**를 BiDi로 열어 주고, 그 안에서 다음이 된다:
//   - 콘텐츠 프로세스 강제 GC: `child-gc-request` + `child-cc-request` 알림(about:memory의 GC 버튼이 쓰는 경로) —
//     프로브에서 콘텐츠 프로세스마다 GCMajor 1회 + CC가 실제로 관측됐다.
//   - GC 로그: Gecko 프로파일러(스택 샘플링 끔, 마커만, 메인 스레드)의 GCMajor/GCSlice/GCMinor/CC/CCSlice 마커.
//     하네스의 `performance.mark("loom:…")`가 같은 스레드에 UserTiming 마커로 찍히므로, 페이지의
//     `performance.getEntriesByType("mark")`와 짝지어 프로파일러 시계 ↔ 페이지 시계(performance.now)를 맞춘다.
// **CPU 스로틀은 여전히 불가**다 — Firefox에 해당 기능 자체가 없다(§8 한계 그대로). 배수 1만 받는다.
//
// 하네스(wasm)는 **바꾸지 않는다**: 하네스는 `window.__loomCdp(payload)`를 부르고 `window.__loomCdpAck(응답)`을
// 기다린다. 여기서는 BiDi 프리로드 스크립트의 **채널**로 `__loomCdp`를 만들어 같은 핸드셰이크를 흉내 낸다.
// 그래서 Firefox 레코드의 `gc.control = "cdp"`는 "에이전트 핸드셰이크 정상"이라는 하네스의 상태 이름일 뿐이고,
// 실제 출처는 `gc.cell_start.tracing`에 실린다(개정 #10 후속 3 등재).
//
// 응답 형식은 `cdp-agent.mjs`와 같다(ok, throttle_rate, data_loss, sync_spread_ms, n_events, forced_majorgc_events, union).
//
// 사용:
//   node bench/e2e/ff-bidi-agent.mjs <원격 포트> <스로틀 배수(1만)> <GC 로그 jsonl 경로> <수집 이름>

import { appendFileSync } from "node:fs";

const port = Number(process.argv[2] ?? 9222);
const rate = Number(process.argv[3] ?? 1);
const gcLog = process.argv[4];
const collectName = process.argv[5];
if (rate !== 1) throw new Error("Firefox에는 CPU 스로틀 수단이 없다(§8) — 배수 1만 받는다");
if (!gcLog || !collectName) throw new Error("GC 로그 경로와 수집 이름이 필요하다");

// 프로파일러 버퍼(엔트리 수). 원형 버퍼라 넘치면 **앞쪽**이 버려진다 — 구간의 페이지 표식이 프로파일에 다 있는지로
// 유실을 판정한다(존재 단언).
const PROFILER_ENTRIES = 8 * 1024 * 1024;
const GC_MARKER = /^(GCMajor|GCSlice|GCMinor|CC|CCSlice|GC Interrupt)$/;
// **합집합에 넣는 것은 메인 스레드 일시정지 수준 마커뿐이다** (개정 #10 후속 3 수집 전 검토 지적, 검증자 실재 확인).
// `GCMajor`·`CC`는 증분 사이클 **전체**(첫 슬라이스 ~ 마지막 슬라이스)를 덮는 구간이라 슬라이스 사이의 일반 실행
// 시간까지 포함한다 — 그것을 넣으면 모든 후보가 표본 시간의 ~10%를 "GC"로 받는 공허한 주석이 됐다(스모크 g7: D-LIS
// 10.4%·C 10.1%·K 9.2%). 실제 일시정지는 `GCSlice`·`GCMinor`·`CCSlice`다(Chromium 쪽도 일시정지 수준 이벤트만 쓴다).
// 사이클 구간은 개수·길이만 로그에 따로 남긴다.
const PAUSE_MARKER = /^(GCSlice|GCMinor|CCSlice)$/;
const FORCED_GC_WAIT_MS = 1500;

const log = (...a) => console.log(`[ff-agent ${new Date().toISOString().slice(11, 19)}]`, ...a);

// ---------------------------------------------------------------------------
// BiDi 연결 (Firefox 원격 에이전트의 /session 웹소켓)
// ---------------------------------------------------------------------------
let ws = null;
for (let i = 0; i < 120 && !ws; i++) {
  try {
    const w = new WebSocket(`ws://127.0.0.1:${port}/session`);
    await new Promise((res, rej) => { w.onopen = res; w.onerror = rej; });
    ws = w;
  } catch {
    await new Promise((r) => setTimeout(r, 500));
  }
}
if (!ws) throw new Error(`BiDi 포트 ${port}에 붙지 못했다`);
let nextId = 1;
const pending = new Map();
const listeners = new Map();
function send(method, params = {}) {
  const id = nextId++;
  ws.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject, method }));
}
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id !== undefined && pending.has(msg.id)) {
    const p = pending.get(msg.id);
    pending.delete(msg.id);
    if (msg.type === "error") p.reject(new Error(`${p.method}: ${msg.error} ${msg.message ?? ""}`));
    else p.resolve(msg.result);
    return;
  }
  for (const fn of listeners.get(msg.method) ?? []) fn(msg.params);
};
ws.onclose = () => {
  log(`브라우저 연결 종료 — 강제 GC ${stats.gc}회, 프로파일 구간 ${stats.traces}개`);
  process.exit(0);
};
function on(method, fn) {
  if (!listeners.has(method)) listeners.set(method, []);
  listeners.get(method).push(fn);
}
const stats = { gc: 0, traces: 0 };

const sess = await send("session.new", { capabilities: { alwaysMatch: {} } });
log(`BiDi 세션 — ${sess.capabilities?.browserName} ${sess.capabilities?.browserVersion}`);
await send("session.subscribe", { events: ["script.message", "browsingContext.load"] });

// chrome(특권) 컨텍스트 — `--remote-allow-system-access`가 있어야 열린다.
const chromeTree = await send("browsingContext.getTree", { "moz:scope": "chrome" });
const chromeCtx = chromeTree.contexts?.[0]?.context;
if (!chromeCtx) throw new Error("chrome 컨텍스트가 없다 — Firefox를 --remote-allow-system-access로 띄웠는가");

async function evalIn(ctx, expression) {
  const r = await send("script.evaluate", { expression, target: { context: ctx }, awaitPromise: true });
  if (r.type === "exception") throw new Error(`평가 예외: ${r.exceptionDetails?.text}`);
  return r.result?.value;
}

// 하네스의 `__loomCdp` 바인딩 — 이후 생기는 문서마다(프리로드) + 이미 떠 있는 문서에(callFunction).
const BIND = "(ch) => { window.__loomCdp = (p) => ch(p); }";
await send("script.addPreloadScript", { functionDeclaration: BIND, arguments: [{ type: "channel", value: { channel: "loom" } }] });
const contextUrl = new Map();
async function bindExisting() {
  const tree = await send("browsingContext.getTree", {});
  for (const c of tree.contexts) {
    contextUrl.set(c.context, c.url);
    if (!matches(c.url)) continue;
    await send("script.callFunction", {
      functionDeclaration: BIND, awaitPromise: false, target: { context: c.context },
      arguments: [{ type: "channel", value: { channel: "loom" } }],
    }).catch((e) => log(`기존 문서 바인딩 실패: ${e.message}`));
    announce(c.context, c.url);
  }
}
function matches(url) {
  return url.includes(`collect=${collectName}&`) || url.endsWith(`collect=${collectName}`);
}
// "붙음" 로그는 문맥마다 한 번 — 시작 시 URL이 아직 수집 URL이 아니었던 문맥도 load/첫 메시지에서 남긴다(재검토 지적:
// 시작 시 한 번만 찍으면 드라이버의 90초 부착 검사가 정상 수집을 죽일 수 있었다).
const announced = new Set();
function announce(ctx, url) {
  if (announced.has(ctx) || !matches(url)) return;
  announced.add(ctx);
  log(`페이지 타깃에 붙음 ${ctx.slice(0, 8)} — ${url}`);
}
on("browsingContext.load", (p) => {
  contextUrl.set(p.context, p.url);
  announce(p.context, p.url);
});
await bindExisting();

// ---------------------------------------------------------------------------
// 프로파일 구간
// ---------------------------------------------------------------------------
const seg = new Map(); // context → { pageStart }

async function profilerStart(ctx) {
  await evalIn(chromeCtx, `(async () => { await Services.profiler.StartProfiler(${PROFILER_ENTRIES}, 10, ["nostacksampling"], ["GeckoMain"]); return Services.profiler.IsActive(); })()`);
  // 구간 시작의 페이지 시각 — 유실 판정(이 시각 이후의 페이지 표식이 프로파일에 다 있어야 한다)에 쓴다.
  const pageStart = await evalIn(ctx, "performance.now()");
  seg.set(ctx, { pageStart });
}

/** 프로파일을 거두고 멈춘다. 측정 페이지의 메인 스레드에서 loom: UserTiming 마커와 GC 마커를 뽑는다. */
async function profilerCollect(ctx) {
  const t0 = Date.now();
  const raw = await evalIn(chromeCtx, `(async () => {
    const p = await Services.profiler.getProfileDataAsync();
    Services.profiler.StopProfiler();
    const out = [];
    const visit = (proc) => {
      for (const t of proc.threads || []) {
        const m = t.markers; if (!m || !m.data) continue;
        const strs = t.stringTable || []; const s = m.schema;
        const ut = [], gc = [];
        for (const r of m.data) {
          const nm = strs[r[s.name]];
          if (nm === "UserTiming") {
            const d = r[s.data];
            if (d && typeof d.name === "string" && d.name.startsWith("loom:")) ut.push([d.name, r[s.startTime]]);
          } else if (${GC_MARKER}.test(nm)) {
            const d = r[s.data] || {}; const tm = d.timings || {};
            // GCMajor의 reason — 강제 GC(child-gc-request = DOM_IPC)와 자연 GC를 가른다.
            gc.push([nm, r[s.startTime], r[s.endTime], r[s.phase], nm === "GCMajor" ? (tm.reason ?? null) : null]);
          }
        }
        if (ut.length) out.push({ pid: t.pid, tid: t.tid, ut, gc, markerRows: m.data.length });
      }
      for (const c of proc.processes || []) visit(c);
    };
    visit(p);
    return JSON.stringify(out);
  })()`);
  const threads = JSON.parse(raw);
  const collectMs = Date.now() - t0;
  const page = JSON.parse(await evalIn(ctx, `JSON.stringify(performance.getEntriesByType("mark").filter(m => m.name.startsWith("loom:")).map(m => [m.name, m.startTime]))`));
  return { threads, page, pageStart: seg.get(ctx)?.pageStart ?? 0, collectMs };
}

const median = (xs) => { const v = xs.slice().sort((a, b) => a - b); return v.length ? v[v.length >> 1] : NaN; };

function extract({ threads, page, pageStart }) {
  // 이 구간에 찍혔어야 할 페이지 표식(구간 시작 이후).
  const expected = page.filter(([, t]) => t >= pageStart);
  if (!expected.length) return { ok: false, why: "이 구간에 페이지 loom: 표식이 없다 — 시계 대응 불가", data_loss: false };
  // 측정 페이지의 메인 스레드 = 그 표식들이 찍힌 스레드. 표식 이름이 가장 많이 겹치는 스레드를 고른다.
  const names = new Set(expected.map(([n]) => n));
  let best = null, bestHits = 0;
  for (const th of threads) {
    const hits = th.ut.filter(([n]) => names.has(n)).length;
    if (hits > bestHits) { best = th; bestHits = hits; }
  }
  if (!best) return { ok: false, why: "프로파일에 이 페이지의 loom: 표식이 없다", data_loss: true };
  // 짝짓기: 같은 이름의 표식은 시간 순서대로 대응시킨다(W6 flush 표식이 반복된다).
  const byName = (arr) => { const m = new Map(); for (const [n, t] of arr) { if (!m.has(n)) m.set(n, []); m.get(n).push(t); } for (const v of m.values()) v.sort((a, b) => a - b); return m; };
  const pm = byName(expected), um = byName(best.ut);
  const offs = [];
  let missing = 0;
  for (const [n, pts] of pm) {
    const uts = um.get(n) || [];
    // 프로파일 쪽이 모자라면 앞쪽이 버려진 것(원형 버퍼) — 뒤에서부터 짝짓는다.
    const k = Math.min(pts.length, uts.length);
    missing += pts.length - k;
    for (let i = 1; i <= k; i++) offs.push(uts[uts.length - i] - pts[pts.length - i]);
  }
  if (!offs.length) return { ok: false, why: "짝지을 표식이 없다", data_loss: true };
  const off = median(offs);
  const spread = Math.max(...offs) - Math.min(...offs);
  // GC 구간: phase 1 = 구간(start, end), 2 = 시작, 3 = 끝(같은 이름끼리 짝), 0 = 순간(길이 0 — 버린다).
  // 구간을 만든 뒤 일시정지 마커(raw → 합집합)와 사이클 구간(cycles — 로그만)으로 나눈다.
  const raw = [], cycles = [], counts = {}, open = new Map();
  let unpairedEnd = 0;
  const put = (seg) => (PAUSE_MARKER.test(seg[2]) ? raw : cycles).push(seg);
  for (const [nm, s, e, ph, reason] of best.gc) {
    counts[nm] = (counts[nm] || 0) + 1;
    if (ph === 1) put([s - off, e - off, nm, reason]);
    else if (ph === 2) open.set(nm, [s, reason]);
    else if (ph === 3 && open.has(nm)) { const [s0, r0] = open.get(nm); put([s0 - off, e - off, nm, r0]); open.delete(nm); }
    else if (ph === 3) unpairedEnd++;
  }
  const unpaired = { start: open.size, end: unpairedEnd };
  raw.sort((a, b) => a[0] - b[0]);
  const union = [];
  for (const [s, e] of raw) {
    const last = union[union.length - 1];
    if (last && s <= last[1]) last[1] = Math.max(last[1], e);
    else union.push([s, e]);
  }
  const markTimes = Object.fromEntries(expected.map(([n, t]) => [n, t]));
  const req = markTimes["loom:gc:req"], fin = markTimes["loom:gc:done"];
  // 강제 GC 존재 단언: 요청~완료 표식 사이의 GCMajor 중 **reason이 DOM_IPC**(child-gc-request가 만든 것)만 센다 —
  // 자연 증분 GCMajor 구간이 그 창에 걸쳐도 세지 않는다(수집 전 검토 지적: 그러면 기제가 안 돌아도 통과한다).
  const forced = req === undefined || fin === undefined ? null
    : cycles.filter(([s, e, n, r]) => n === "GCMajor" && r === "DOM_IPC" && e >= req && s <= fin).length;
  // 강제 GC가 **일시정지로도** 보이는가(합집합 정의가 실제 마커를 잡는다는 존재 단언) · CC 요청도 실제로 돌았는가.
  const inWin = (s, e) => req !== undefined && fin !== undefined && e >= req && s <= fin;
  const forcedPause = req === undefined || fin === undefined ? null : raw.filter(([s, e, n]) => n === "GCSlice" && inWin(s, e)).length;
  const forcedCc = req === undefined || fin === undefined ? null
    : cycles.concat(raw).filter(([s, e, n]) => (n === "CC" || n === "CCSlice") && inWin(s, e)).length;
  const cycleSummary = {};
  for (const [s, e, n] of cycles) { const c = (cycleSummary[n] ||= { n: 0, total_ms: 0 }); c.n++; c.total_ms += e - s; }
  // n_events = 합집합에 들어간 구간 수(Chromium 에이전트와 같은 정의) — 하네스의 bad-union 판정이 이것을 쓴다.
  return { ok: true, data_loss: missing > 0, missing_marks: missing, sync_spread_ms: spread, n_events: raw.length,
    counts, cycles: cycleSummary, unpaired, marker_rows: best.markerRows, union, markTimes, forced, forcedPause, forcedCc };
}

const r4 = (x) => Math.round(x * 1e4) / 1e4;

async function handle(ctx, req) {
  const t0Handle = Date.now();
  const base = { group: req.group, cid: req.cid, cmd: req.cmd };
  switch (req.cmd) {
    case "cell-start": {
      if (await evalIn(chromeCtx, "Services.profiler.IsActive()")) {
        log(`group=${req.group}: 앞 구간의 프로파일러가 켜진 채였다 — 멈추고 버린다`);
        await evalIn(chromeCtx, "Services.profiler.StopProfiler()");
      }
      await profilerStart(ctx);
      return { ok: true, throttle_rate: 1, agent: "ff-bidi-agent",
        tracing: "gecko-profiler GeckoMain markers via WebDriver BiDi; union = GCSlice|GCMinor|CCSlice (pause-level); " +
          "forced = GCMajor reason DOM_IPC (+CC, +GCSlice in window); GCMajor/CC cycle spans logged only, not in union" };
    }
    case "gc": {
      // 콘텐츠 프로세스 GC + CC 요청(about:memory GC 버튼과 같은 경로). 비동기라 처리될 시간을 준 뒤 응답한다 —
      // 실제로 일어났는지는 프로파일의 GCMajor(요청~완료 표식 사이)로 사후 단언한다.
      // **GC → CC → GC** (재검토 지적): CC가 끊은 DOM↔JS 사이클의 JS 쪽은 다음 GC가 있어야 해제된다. GC·CC 한 번씩으로
      // 끝내면 그 몫이 파일럿·워밍업(길게는 측정 초반)의 자연 GC로 미뤄진다. Chromium의 HeapProfiler.collectGarbage는
      // V8+Oilpan 통합 힙을 한 번에 치우므로, 이 순서가 "셀 시작 시 쓰레기를 치운다"는 같은 의도에 가깝다.
      const ask = (topic) => evalIn(chromeCtx, `Services.obs.notifyObservers(null, "${topic}"); true`);
      await ask("child-gc-request");
      await new Promise((r) => setTimeout(r, FORCED_GC_WAIT_MS / 3));
      await ask("child-cc-request");
      await new Promise((r) => setTimeout(r, FORCED_GC_WAIT_MS / 3));
      await ask("child-gc-request");
      await new Promise((r) => setTimeout(r, FORCED_GC_WAIT_MS / 3));
      stats.gc++;
      return { ok: true, method: "child-gc-request → child-cc-request → child-gc-request" };
    }
    case "trace-flush":
    case "measure-end": {
      const col = await profilerCollect(ctx);
      stats.traces++;
      const x = extract(col);
      if (req.cmd === "trace-flush") await profilerStart(ctx);
      if (!x.ok) {
        appendFileSync(gcLog, JSON.stringify({ ...base, ok: false, why: x.why, data_loss: x.data_loss }) + "\n");
        return { ok: false, why: x.why, data_loss: x.data_loss };
      }
      appendFileSync(gcLog, JSON.stringify({
        ...base, ok: true, data_loss: x.data_loss, missing_marks: x.missing_marks, sync_spread_ms: r4(x.sync_spread_ms),
        n_events: x.n_events, counts: x.counts, cycles: x.cycles, unpaired: x.unpaired, marker_rows: x.marker_rows,
        profile_collect_ms: col.collectMs, handshake_ms: Date.now() - t0Handle,
        forced_majorgc_events: x.forced, forced_pause_events: x.forcedPause, forced_cc_events: x.forcedCc, marks: x.markTimes,
        union_ms: x.union.map(([s, e]) => [r4(s), r4(e)]),
      }) + "\n");
      return {
        ok: true, data_loss: x.data_loss, sync_spread_ms: r4(x.sync_spread_ms), n_events: x.n_events,
        forced_majorgc_events: x.forced, forced_pause_events: x.forcedPause, forced_cc_events: x.forcedCc, union: x.union.flatMap(([s, e]) => [r4(s), r4(e)]),
      };
    }
    default:
      return { ok: false, why: `모르는 명령 ${req.cmd}` };
  }
}

// 페이지 → 에이전트(채널 메시지). 문맥마다 직렬로 처리하고(cdp-agent와 같은 이유), 응답에 요청 id를 싣는다.
const chains = new Map();
on("script.message", (p) => {
  if (p.channel !== "loom") return;
  const ctx = p.source?.context;
  const prev = chains.get(ctx) ?? Promise.resolve();
  chains.set(ctx, prev.then(async () => {
    // 이동 직후면 load 이벤트보다 메시지가 먼저 올 수 있다 — URL을 다시 조회한다.
    let url = contextUrl.get(ctx) ?? "";
    if (!matches(url)) {
      const t = await send("browsingContext.getTree", { root: ctx }).catch(() => null);
      url = t?.contexts?.[0]?.url ?? url;
      contextUrl.set(ctx, url);
    }
    if (!matches(url)) { log(`다른 수집의 페이지 메시지 — 무시: ${url}`); return; }
    announce(ctx, url);
    await onMessage(ctx, p);
  }).catch((e) => log(`핸들러 오류: ${e.message}`)));
});
async function onMessage(ctx, p) {
  let req;
  try { req = JSON.parse(p.data?.value ?? "{}"); } catch { req = { cmd: "?" }; }
  let reply;
  try { reply = await handle(ctx, req); } catch (e) { reply = { ok: false, why: String(e.message ?? e) }; }
  reply.req_id = req.req_id ?? null;
  if (!reply.ok) log(`명령 실패 group=${req.group} ${req.cmd}: ${reply.why}`);
  else if (req.cmd === "measure-end" || req.cmd === "cell-start")
    log(`group=${req.group} ${req.cmd}${reply.n_events !== undefined ? ` GC 마커 ${reply.n_events}, 유실=${reply.data_loss}, 강제GC GCMajor=${reply.forced_majorgc_events}` : ""}`);
  try {
    await send("script.callFunction", {
      functionDeclaration: "(s) => { if (window.__loomCdpAck) window.__loomCdpAck(JSON.parse(s)); }",
      awaitPromise: false, target: { context: ctx }, arguments: [{ type: "string", value: JSON.stringify(reply) }],
    });
  } catch (e) {
    log(`응답 전달 실패(페이지가 이미 이동했을 수 있다): ${e.message}`);
  }
}

log(`감시 시작 — 포트 ${port}, 수집 ${collectName}, GC 로그 ${gcLog} (스로틀 없음 — Firefox 수단 부재)`);
