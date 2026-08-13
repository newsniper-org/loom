import { chromium } from "playwright";
const log = (...a) => { console.log(new Date().toISOString().slice(11,19), ...a); };
process.on("unhandledRejection", e => log("[UNHANDLED]", String(e).slice(0,300)));
process.on("uncaughtException", e => log("[UNCAUGHT]", String(e).slice(0,300)));
process.on("exit", c => log("[EXIT]", c));
for (const sig of ["SIGTERM","SIGINT","SIGHUP"]) process.on(sig, () => { log("[SIGNAL]", sig); process.exit(9); });

const b = await chromium.launch();
const p = await b.newPage();
p.on("pageerror", e => log("[PAGEERROR]", e.message.slice(0, 400)));
p.on("crash", () => log("[CRASH] 렌더러 사망"));
p.on("close", () => log("[PAGE CLOSE]"));
b.on("disconnected", () => log("[BROWSER DISCONNECTED]"));
p.on("console", m => { if (m.type() === "error") log("[console.error]", m.text().slice(0, 300)); });

await p.goto("http://localhost:8931/demo/list_bench.html?group=6", { waitUntil: "domcontentloaded" });
log("페이지 로드됨 (collect 파라미터 없음 → 자기 이동 안 함)");
const t0 = Date.now();
const iv = setInterval(async () => {
  const el = Math.round((Date.now() - t0) / 1000);
  try {
    const s = await p.evaluate(() => ({
      mem: performance.memory ? Math.round(performance.memory.usedJSHeapSize/1048576) : -1,
      res: (document.getElementById("results")?.textContent || "").slice(0, 24),
    }));
    log(`  ${el}초  힙=${s.mem}MB  results="${s.res}"`);
  } catch (e) { log(`  ${el}초  evaluate 실패:`, e.message.slice(0,150)); }
}, 15000);
try {
  await p.waitForFunction(() => {
    const el = document.getElementById("results");
    return el && el.textContent.trim().startsWith("[");
  }, null, { timeout: 2700000 });
  const recs = JSON.parse(await p.locator("#results").textContent());
  log("완료 — 레코드", recs.length, recs[0] ? `표본=${recs[0].samples_ms.length} over_cap=${recs[0].over_cell_cap} wall=${Math.round(recs[0].cell_wall_ms/1000)}s` : "");
} catch (e) {
  log("실패:", e.message.slice(0, 250));
}
clearInterval(iv);
await b.close();
log("정상 종료");
