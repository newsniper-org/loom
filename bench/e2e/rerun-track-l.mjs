// §2.5 동결 규율에 따른 트랙 L 재수집.
//
// `place_in_order`가 쓰는 `Chain`이 손코딩본에서 `loom-flush-core`의 Verus
// 검증본으로 교체됐다. §2.5는 "공유 스택 + 하네스 + 분석 코드의 커밋 해시를
// 캠페인 단위로 동결. 캠페인 중 변경 시 영향받는 전 셀을 전 후보에 대해
// 재실행. **상이한 기반의 측정치를 같은 판정에 혼용 금지**"라고 규정하므로,
// 행동이 동일하더라도 이전 데이터(v6)와 섞어 쓸 수 없다.
//
// 사용법 (bench/serve-isolated.py가 떠 있어야 함):
//   node bench/e2e/rerun-track-l.mjs chromium <out.json>
//   node bench/e2e/rerun-track-l.mjs firefox  <out.json>

import { chromium, firefox } from "@playwright/test";
import { writeFileSync } from "node:fs";

const which = process.argv[2] ?? "chromium";
const outPath = process.argv[3];
if (!outPath) throw new Error("출력 경로를 인자로 주세요");

const browser = await (which === "firefox" ? firefox : chromium).launch();
const page = await browser.newPage();
page.on("pageerror", (e) => console.log("[pageerror]", e.message));

await page.goto("http://localhost:8931/demo/list_bench.html");

// 트랙 L(#results)과 트랙 T(#results-cond)가 순차 실행되므로, 뒤엣것이
// 채워지면 앞엣것도 끝난 것이다.
await page.waitForFunction(
  () => {
    const el = document.getElementById("results-cond");
    return el && el.textContent && el.textContent !== "running...";
  },
  undefined,
  { timeout: 900000 }
);

const listJson = await page.locator("#results").textContent();
const parsed = JSON.parse(listJson);
writeFileSync(outPath, JSON.stringify(parsed, null, 1) + "\n");
console.log(`[track-l] ${which}: wrote ${parsed.length} records -> ${outPath}`);

await browser.close();
