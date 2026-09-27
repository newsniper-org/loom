// **트랙 L Chromium 수집의 CDP 에이전트** — §8의 두 조항을 한 연결로 이행한다
// (개정 #10, 2026-09-27; v21까지는 스로틀만 `apply-throttle.mjs`가 했다).
//
//   §8 하드웨어·스로틀: "저사양 프록시: Chromium CDP 4× CPU throttle을 (i) 트랙 L 핵심 격자 … 에 추가"
//   §8 격리(M-15):      "성능·메모리 셀 시작 시 CDP 강제 GC 1회, 측정 중 GC 이벤트 로그
//                        (이상치 제거 없이 GC 주석 병기)"
//
// v21 적대적 검증(2026-09-27)이 M-15가 하네스·드라이버·레코드 어디에도 없다는 것을 확인했다
// — 비스로틀 런에는 CDP 연결 자체가 없었다. 그래서 이제 **모든 Chromium 런**이 이 에이전트를
// 띄운다(배수 1이면 스로틀만 안 건다).
//
// ── 왜 페이지와 핸드셰이크하는가 ──────────────────────────────────────────────
// v21의 스로틀 감시는 `framenavigated`에 반응해 **페이지와 경주하며** 걸었다. 강제 GC와
// 트레이스 경계는 그렇게 걸 수 없다 — "셀 시작 시"와 "측정 중"은 페이지만 아는 시점이다.
// 그래서 페이지(하네스)가 CDP 바인딩 `__loomCdp`를 부르고 **응답이 올 때까지 멈춰 기다린다**.
// 에이전트의 CPU 작업(트레이스 전송·파싱)은 전부 그 대기 중에, 즉 측정 괄호 밖에서 일어난다.
//
//   cell-start  → (배수>1이면) Emulation.setCPUThrottlingRate, Tracing.start      → 응답
//   gc          → HeapProfiler.collectGarbage                                     → 응답
//   trace-flush → Tracing.end → GC 구간 추출 → Tracing.start (W6의 500삽입 경계용) → 응답
//   measure-end → Tracing.end → GC 구간 추출                                      → 응답
//
// ── 왜 Playwright가 아니라 원시 CDP인가 ────────────────────────────────────────
// `connectOverCDP`는 붙는 순간 페이지마다 Page/Runtime/Network 등 여러 도메인을 켜고 유틸리티
// world를 주입한다. 측정 대상 렌더러에 붙는 것은 **최소한**이어야 한다(v21에서 측정 중인
// 렌더러에 진단 프로브를 넣었다가 감시 프로세스가 죽은 사고가 있었다). 여기서 켜는 것은
// Runtime(바인딩 알림용)과, 명령 시점의 Emulation/HeapProfiler/Tracing뿐이다.
//
// ── GC 이벤트의 출처 ───────────────────────────────────────────────────────────
// 카테고리 `v8`(MinorGC·MajorGC·V8.GC* — V8 힙)과 `cppgc`(CppGC.* — Blink DOM 노드가 사는
// Oilpan 힙)의 **렌더러 메인 스레드** 'X' 이벤트다. `disabled-by-default-*`만 넣으면 기본
// 카테고리가 통째로 켜지므로(2026-09-27 프로브로 확인: 이벤트 23,807개 중 GC는 소수)
// 존재하지 않는 카테고리 `__loom_none`을 함께 넣어 기본 카테고리를 끈다. `blink.user_timing`은
// 하네스의 `performance.mark("loom:…")`를 받는다 — 그 이벤트의 `args.data.startTime`이 페이지
// 시계(performance.now)라서 트레이스 시계(ts)와 **직접** 대응된다.
//
// 사용:
//   node bench/e2e/cdp-agent.mjs <CDP 포트> <스로틀 배수(1=없음)> <GC 로그 jsonl 경로> <수집 이름>
//
// **수집 이름이 URL에 없는 페이지에는 붙지 않는다** (2026-09-27). 앞 수집의 브라우저가 살아남아
// 같은 포트를 쥐고 있으면 에이전트가 그 옛 페이지에 붙고, 새 페이지는 GC 통제·스로틀 없이
// 측정된다 — 개정 #10 스모크에서 실제로 그랬다.

import { appendFileSync } from "node:fs";

const port = Number(process.argv[2] ?? 9222);
const rate = Number(process.argv[3] ?? 1);
const gcLog = process.argv[4];
const collectName = process.argv[5];
if (!Number.isFinite(rate) || rate < 1) throw new Error(`스로틀 배수가 이상하다: ${process.argv[3]}`);
if (!gcLog) throw new Error("GC 로그 경로가 필요하다 (세 번째 인자)");
if (!collectName) throw new Error("수집 이름이 필요하다 (네 번째 인자)");

const CATEGORIES = ["__loom_none", "v8", "cppgc", "blink.user_timing"];
// 트레이스 버퍼(KB). 넘치면 Chrome이 조용히 버리므로 `dataLossOccurred`를 반드시 싣는다.
const BUFFER_KB = 200 * 1024;
const BINDING = "__loomCdp";

const log = (...a) => console.log(`[cdp-agent ${new Date().toISOString().slice(11, 19)}]`, ...a);

// ---------------------------------------------------------------------------
// 원시 CDP 클라이언트 — 브라우저 엔드포인트 하나에 평탄화(flatten) 세션으로 붙는다.
// ---------------------------------------------------------------------------
let version = null;
for (let i = 0; i < 120 && !version; i++) {
  try {
    version = await (await fetch(`http://127.0.0.1:${port}/json/version`)).json();
  } catch {
    await new Promise((r) => setTimeout(r, 500));
  }
}
if (!version) throw new Error(`CDP 포트 ${port}에 붙지 못했다`);

const ws = new WebSocket(version.webSocketDebuggerUrl);
await new Promise((res, rej) => {
  ws.onopen = res;
  ws.onerror = rej;
});
let nextId = 1;
const pending = new Map();
const listeners = new Map(); // method → [fn(params, sessionId)]
function send(method, params = {}, sessionId) {
  const id = nextId++;
  ws.send(JSON.stringify(sessionId ? { id, method, params, sessionId } : { id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject, method }));
}
function on(method, fn) {
  if (!listeners.has(method)) listeners.set(method, []);
  listeners.get(method).push(fn);
}
ws.onmessage = (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id !== undefined) {
    const p = pending.get(msg.id);
    if (!p) return;
    pending.delete(msg.id);
    if (msg.error) p.reject(new Error(`${p.method}: ${msg.error.message}`));
    else p.resolve(msg.result);
    return;
  }
  for (const fn of listeners.get(msg.method) ?? []) fn(msg.params, msg.sessionId);
};
ws.onclose = () => {
  log(`브라우저 연결 종료 — 스로틀 적용 ${stats.throttle}회, 강제 GC ${stats.gc}회, 트레이스 구간 ${stats.traces}개`);
  process.exit(0);
};

const stats = { throttle: 0, gc: 0, traces: 0 };

// ---------------------------------------------------------------------------
// 트레이스 — 세션마다 하나. 이벤트는 Tracing.end 뒤에 한꺼번에 온다(2026-09-27 프로브:
// end 전 도착 0회).
// ---------------------------------------------------------------------------
const traceBuf = new Map(); // sessionId → { events, done }
// **압축 보관** (개정 #10 후속): 셀 하나가 수십만 이벤트(스모크 W1×R1: 40만)이고 30분 셀은 그 몇 배다.
// 전체 객체를 쌓으면 에이전트 힙이 GB 단위가 된다 — 필요한 필드만 남긴다.
on("Tracing.dataCollected", (p, sid) => {
  const t = traceBuf.get(sid);
  if (!t) return;
  for (const e of p.value) {
    if (e.cat === "blink.user_timing" && e.name?.startsWith("loom:")) {
      t.marks.push({ name: e.name, ts: e.ts, pid: e.pid, tid: e.tid, startTime: e.args?.data?.startTime });
    } else if (e.ph === "X" && GC_NAME.test(e.name) && (e.cat.includes("v8") || e.cat.includes("cppgc"))) {
      t.gc.push([e.ts, e.dur ?? 0, e.name, e.pid, e.tid]);
    }
  }
});
on("Tracing.tracingComplete", (p, sid) => traceBuf.get(sid)?.done(p));

async function traceStart(sid) {
  let done;
  const complete = new Promise((r) => (done = r));
  traceBuf.set(sid, { marks: [], gc: [], done, complete });
  await send(
    "Tracing.start",
    {
      traceConfig: { includedCategories: CATEGORIES, recordMode: "recordUntilFull", traceBufferSizeInKb: BUFFER_KB },
      transferMode: "ReportEvents",
    },
    sid,
  );
}

async function traceEnd(sid) {
  const t = traceBuf.get(sid);
  if (!t) return null;
  await send("Tracing.end", {}, sid);
  const fin = await t.complete;
  traceBuf.delete(sid);
  stats.traces++;
  return { marks: t.marks, gc: t.gc, dataLoss: !!fin.dataLossOccurred };
}

const GC_NAME = /^(MinorGC|MajorGC|V8\.GC|CppGC\.)/;
const median = (xs) => {
  const v = xs.slice().sort((a, b) => a - b);
  return v.length ? v[v.length >> 1] : NaN;
};

/**
 * 트레이스 → 페이지 시계(performance.now, ms)의 GC 구간 합집합.
 *
 * - 메인 스레드는 하네스의 `loom:` 표식이 찍힌 (pid, tid)로 정한다 — 이름("CrRendererMain")으로
 *   고르면 다른 렌더러(예: 확장·서비스 워커)의 메인 스레드와 섞일 수 있다.
 * - 시계 대응 = 표식마다 `ts/1000 − startTime`의 중앙값. 흩어짐(최대−최소)을 함께 싣는다 —
 *   그것이 이 주석의 시간 정밀도다.
 */
function extract(trace) {
  const marks = trace.marks;
  if (!marks.length) {
    return { ok: false, why: "트레이스에 loom: 표식이 없다 — 시계 대응 불가", data_loss: trace.dataLoss };
  }
  // 시계 대응이 **숫자여야 한다** (개정 #10 후속). 표식 형식이 바뀌어 startTime이 없으면 NaN이 되고,
  // 그러면 모든 구간 끝점이 JSON에서 null이 되어 "GC 0"이라는 공허한 결과가 된다 — 실패로 돌려준다.
  if (marks.some((m) => typeof m.startTime !== "number" || !Number.isFinite(m.startTime))) {
    return { ok: false, why: "loom: 표식에 args.data.startTime이 없다 — 시계 대응 불가", data_loss: trace.dataLoss };
  }
  const { pid, tid } = marks[0];
  const offs = marks.map((m) => m.ts / 1000 - m.startTime);
  const off = median(offs);
  const spread = Math.max(...offs) - Math.min(...offs);
  const markTimes = Object.fromEntries(marks.map((m) => [m.name, m.startTime]));
  const counts = {};
  const raw = [];
  for (const [ts, dur, name, epid, etid] of trace.gc) {
    if (epid !== pid || etid !== tid) continue;
    counts[name] = (counts[name] || 0) + 1;
    const s = ts / 1000 - off;
    raw.push([s, s + dur / 1000, name]);
  }
  raw.sort((a, b) => a[0] - b[0]);
  // 중첩(예: MajorGC ⊃ V8.GCFinalizeMC)을 합집합으로 접는다.
  const union = [];
  for (const [s, e] of raw) {
    const last = union[union.length - 1];
    if (last && s <= last[1]) last[1] = Math.max(last[1], e);
    else union.push([s, e]);
  }
  // 강제 GC가 **실제로 일어났는가**(존재 단언). 요청~완료 표식 사이에 MajorGC가 있어야 한다.
  const req = markTimes["loom:gc:req"], fin = markTimes["loom:gc:done"];
  const forced =
    req === undefined || fin === undefined
      ? null
      : raw.filter(([s, e, n]) => n === "MajorGC" && e >= req && s <= fin).length;
  return { ok: true, data_loss: trace.dataLoss, sync_spread_ms: spread, n_events: raw.length, counts, union, markTimes, forced };
}

const r4 = (x) => Math.round(x * 1e4) / 1e4;

// ---------------------------------------------------------------------------
// 페이지 명령 처리
// ---------------------------------------------------------------------------
async function handle(sid, req) {
  const base = { group: req.group, cid: req.cid, cmd: req.cmd };
  switch (req.cmd) {
    case "cell-start": {
      // 앞 페이지가 측정 도중 죽으면(렌더러 OOM — v21에서 실제로 있었다) 트레이스가 열린 채
      // 남는다. 그대로 두면 이 `Tracing.start`가 "이미 시작됨"으로 실패하고 **이후 전 셀**이 GC
      // 통제 실패가 된다. 닫고 버리되 로그에 남긴다(버린 구간은 어느 레코드에도 안 쓰인다).
      if (traceBuf.has(sid)) {
        log(`group=${req.group}: 앞 페이지의 트레이스가 열린 채였다 — 닫고 버린다`);
        await traceEnd(sid).catch((e) => log(`  닫기 실패: ${e.message}`));
        traceBuf.delete(sid);
      }
      let applied = 1;
      if (rate > 1) {
        // v21과 같은 CDP 명령. 이제 **셀 시작마다 페이지가 기다리는 동안** 건다 —
        // 첫 측정 전에 걸렸음이 구조적으로 보장되고, 응답이 레코드에 실린다.
        await send("Emulation.setCPUThrottlingRate", { rate }, sid);
        applied = rate;
        stats.throttle++;
      }
      await traceStart(sid);
      return { ok: true, throttle_rate: applied, tracing: CATEGORIES.join(",") };
    }
    case "gc": {
      await send("HeapProfiler.collectGarbage", {}, sid);
      stats.gc++;
      return { ok: true, method: "HeapProfiler.collectGarbage" };
    }
    case "trace-flush":
    case "measure-end": {
      const t = await traceEnd(sid);
      if (!t) return { ok: false, why: "진행 중인 트레이스가 없다" };
      const x = extract(t);
      if (req.cmd === "trace-flush") await traceStart(sid);
      if (!x.ok) {
        appendFileSync(gcLog, JSON.stringify({ ...base, ok: false, why: x.why, data_loss: x.data_loss }) + "\n");
        return { ok: false, why: x.why, data_loss: x.data_loss };
      }
      // 원시 기록(판정 아님) — 메인 스레드 GC 구간 합집합 전부와 이름별 개수.
      appendFileSync(
        gcLog,
        JSON.stringify({
          ...base,
          ok: true,
          data_loss: x.data_loss,
          sync_spread_ms: r4(x.sync_spread_ms),
          n_events: x.n_events,
          counts: x.counts,
          forced_majorgc_events: x.forced,
          marks: x.markTimes,
          union_ms: x.union.map(([s, e]) => [r4(s), r4(e)]),
        }) + "\n",
      );
      // 페이지에는 합집합만 돌려준다 — 겹침 계산은 창(timed window)을 가진 하네스가 한다.
      return {
        ok: true,
        data_loss: x.data_loss,
        sync_spread_ms: r4(x.sync_spread_ms),
        n_events: x.n_events,
        forced_majorgc_events: x.forced,
        union: x.union.flatMap(([s, e]) => [r4(s), r4(e)]),
      };
    }
    default:
      return { ok: false, why: `모르는 명령 ${req.cmd}` };
  }
}

// 바인딩 호출 → 처리 → `window.__loomCdpAck(응답)`. 페이지는 그동안 멈춰 있다.
//
// **세션마다 직렬로 처리한다** (개정 #10 후속). 페이지가 응답을 120초 기다리다 포기하고 다음 명령을
// 보내면, 앞 명령(큰 트레이스의 종료·파싱)과 겹쳐 같은 세션의 트레이스 상태를 두 핸들러가 만졌다.
// 이제 앞 명령이 끝나야 다음 명령이 시작되고, 응답에는 요청 id가 실려 페이지가 늦은 응답을 버린다.
const chains = new Map(); // sessionId → Promise
on("Runtime.bindingCalled", (p, sid) => {
  if (p.name !== BINDING) return;
  const prev = chains.get(sid) ?? Promise.resolve();
  const next = prev.then(() => onBinding(p, sid)).catch((e) => log(`핸들러 오류: ${e.message}`));
  chains.set(sid, next);
});
async function onBinding(p, sid) {
  let req;
  try {
    req = JSON.parse(p.payload);
  } catch {
    req = { cmd: "?" };
  }
  let reply;
  try {
    reply = await handle(sid, req);
  } catch (e) {
    reply = { ok: false, why: String(e.message ?? e) };
  }
  reply.req_id = req.req_id ?? null;
  if (!reply.ok) log(`명령 실패 group=${req.group} ${req.cmd}: ${reply.why}`);
  else if (req.cmd === "measure-end" || req.cmd === "cell-start")
    log(`group=${req.group} ${req.cmd}${reply.n_events !== undefined ? ` GC 이벤트 ${reply.n_events}, 유실=${reply.data_loss}, 강제GC MajorGC=${reply.forced_majorgc_events}` : ` 배수=${reply.throttle_rate}`}`);
  try {
    await send(
      "Runtime.evaluate",
      { expression: `window.__loomCdpAck && window.__loomCdpAck(${JSON.stringify(reply)})`, contextId: p.executionContextId },
      sid,
    );
  } catch (e) {
    log(`응답 전달 실패(페이지가 이미 이동했을 수 있다): ${e.message}`);
  }
}

// ---------------------------------------------------------------------------
// 페이지 타깃에 붙는다. 바인딩은 "이후 생기는 컨텍스트에도" 설치되므로 페이지가 스스로 다음
// 그룹으로 이동해도(같은 타깃) 다시 걸 필요가 없다.
// ---------------------------------------------------------------------------
const attached = new Set();
const refused = new Set();
async function attach(info) {
  if (info.type !== "page" || attached.has(info.targetId)) return;
  if (!info.url.includes(`collect=${collectName}&`) && !info.url.endsWith(`collect=${collectName}`)) {
    if (!refused.has(info.targetId + info.url)) {
      refused.add(info.targetId + info.url);
      log(`다른 수집의 페이지 — 붙지 않는다: ${info.url}`);
    }
    return;
  }
  attached.add(info.targetId);
  const { sessionId } = await send("Target.attachToTarget", { targetId: info.targetId, flatten: true });
  await send("Runtime.enable", {}, sessionId);
  await send("Runtime.addBinding", { name: BINDING }, sessionId);
  log(`페이지 타깃에 붙음 ${info.targetId.slice(0, 8)} — ${info.url}`);
}
on("Target.targetCreated", (p) => attach(p.targetInfo).catch((e) => log(`붙기 실패: ${e.message}`)));
// 발견 시점에 URL이 아직 수집 URL이 아니었던 탭(about:blank 등)을 놓치지 않게, URL이 바뀌면 다시 본다.
on("Target.targetInfoChanged", (p) => attach(p.targetInfo).catch((e) => log(`붙기 실패: ${e.message}`)));
await send("Target.setDiscoverTargets", { discover: true });
for (const t of (await send("Target.getTargets")).targetInfos) await attach(t).catch((e) => log(`붙기 실패: ${e.message}`));

log(`감시 시작 — 포트 ${port}, 스로틀 배수 ${rate}x, GC 로그 ${gcLog}`);
