// §2.5 동결 규율에 따른 트랙 L 재수집.
//
// `place_in_order`가 쓰는 `Chain`이 손코딩본에서 `loom-flush-core`의 Verus
// 검증본으로 교체됐다. §2.5는 "공유 스택 + 하네스 + 분석 코드의 커밋 해시를
// 캠페인 단위로 동결. 캠페인 중 변경 시 영향받는 전 셀을 전 후보에 대해
// 재실행. **상이한 기반의 측정치를 같은 판정에 혼용 금지**"라고 규정하므로,
// 행동이 동일하더라도 이전 데이터(v6)와 섞어 쓸 수 없다.
//
// 사용법 (bench/serve-isolated.py가 떠 있어야 함):
//   node bench/e2e/rerun-track-l.mjs chromium <out.json> [timeout_ms]
//   node bench/e2e/rerun-track-l.mjs firefox  <out.json> [timeout_ms]

import { chromium, firefox } from "@playwright/test";
import { writeFileSync } from "node:fs";

const which = process.argv[2] ?? "chromium";
const outPath = process.argv[3];
if (!outPath) throw new Error("출력 경로를 인자로 주세요");

const timeoutMs = Number(process.argv[4] ?? 5400000); // 기본 90분

const browser = await (which === "firefox" ? firefox : chromium).launch();
const page = await browser.newPage();

// **즉시 실패**: wasm이 패닉/트랩하면 페이지는 영원히 안 끝난다. 2026-08-03에
// 실제로 `[pageerror] unreachable` 직후 6시간 타임아웃까지 매달렸고, 그동안
// 아무도 실패를 몰랐다. 이제 첫 pageerror에서 바로 죽는다.
let pageErr = null;
page.on("pageerror", (e) => {
  pageErr = e;
  console.error("[pageerror]", e.message);
});
page.on("console", (m) => {
  if (m.type() === "error") console.error("[console.error]", m.text());
});

await page.goto("http://localhost:8931/demo/list_bench.html");

// 트랙 L(#results)과 트랙 T(#results-cond)가 순차 실행되므로, 뒤엣것이
// 채워지면 앞엣것도 끝난 것이다. 진행 상황을 주기적으로 찍어 "살아 있는지"를
// 밖에서 알 수 있게 한다 — 이전에는 완전 무음이라 죽었는지 도는지 구별이
// 안 됐다.
const started = Date.now();
let lastLog = 0;
for (;;) {
  if (pageErr) {
    await browser.close();
    throw new Error(`페이지가 wasm 패닉/트랩으로 죽었다: ${pageErr.message}`);
  }
  const done = await page
    .evaluate(() => {
      const el = document.getElementById("results-cond");
      return !!(el && el.textContent && el.textContent !== "running...");
    })
    .catch(() => false);
  if (done) break;
  const elapsed = Date.now() - started;
  if (elapsed > timeoutMs) {
    await browser.close();
    throw new Error(`타임아웃 ${(timeoutMs / 60000).toFixed(0)}분 초과`);
  }
  if (elapsed - lastLog >= 60000) {
    lastLog = elapsed;
    const partial = await page
      .evaluate(() => (document.getElementById("results")?.textContent ?? "").length)
      .catch(() => 0);
    console.log(
      `[track-l] ${(elapsed / 60000).toFixed(1)}분 경과 — #results ${partial}바이트`
    );
  }
  await new Promise((r) => setTimeout(r, 5000));
}

const listJson = await page.locator("#results").textContent();
const parsed = JSON.parse(listJson);
writeFileSync(outPath, JSON.stringify(parsed, null, 1) + "\n");
console.log(`[track-l] ${which}: wrote ${parsed.length} records -> ${outPath}`);

await browser.close();
