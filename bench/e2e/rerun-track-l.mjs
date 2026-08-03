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
let fatal = null;
page.on("pageerror", (e) => {
  fatal = `wasm 패닉/트랩: ${e.message}`;
  console.error("[pageerror]", e.message);
});
// **렌더러 크래시(OOM 등)는 pageerror를 내지 않는다** — 탭 프로세스가 죽고
// 이후 모든 evaluate가 `Target crashed`로 거부될 뿐이다. 2026-08-03의 감사가
// 지적한 바로 그 경로이고, 노드 슬롯 누수를 고치기 전에는 가장 일어나기 쉬운
// 실패였다. 여기서 명시적으로 잡는다.
page.on("crash", () => {
  fatal = "렌더러 프로세스 크래시(OOM 가능성)";
  console.error("[crash] 렌더러 프로세스가 죽었다");
});
page.on("close", () => {
  if (!fatal) fatal = "페이지가 예기치 않게 닫혔다";
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
  if (fatal) {
    await browser.close();
    throw new Error(`수집이 죽었다: ${fatal}`);
  }
  // evaluate 거부를 **삼키지 않는다**. `Target crashed`/`Execution context
  // was destroyed`가 "아직 도는 중"으로 흡수되면 크래시가 타임아웃까지
  // 무음으로 매달린다.
  let done;
  try {
    done = await page.evaluate(() => {
      const el = document.getElementById("results-cond");
      return !!(el && el.textContent && el.textContent !== "running...");
    });
  } catch (e) {
    await browser.close();
    throw new Error(`페이지 평가가 거부됐다(크래시 가능성): ${e.message}`);
  }
  if (done) break;
  const elapsed = Date.now() - started;
  if (elapsed > timeoutMs) {
    await browser.close();
    throw new Error(`타임아웃 ${(timeoutMs / 60000).toFixed(0)}분 초과`);
  }
  if (elapsed - lastLog >= 60000) {
    lastLog = elapsed;
    // `#results`는 끝에 한 번에 채워지므로 진행 지표가 못 된다(감사 지적).
    // 하네스가 조합마다 갱신하는 `window.__loomProgress`를 읽는다.
    let prog = "(아직 없음)";
    try {
      prog = await page.evaluate(() => window.__loomProgress ?? "(아직 없음)");
    } catch (e) {
      await browser.close();
      throw new Error(`진행 조회가 거부됐다(크래시 가능성): ${e.message}`);
    }
    console.log(`[track-l] ${(elapsed / 60000).toFixed(1)}분 경과 — ${prog}`);
  }
  await new Promise((r) => setTimeout(r, 5000));
}

const listJson = await page.locator("#results").textContent();
const parsed = JSON.parse(listJson);
writeFileSync(outPath, JSON.stringify(parsed, null, 1) + "\n");
console.log(`[track-l] ${which}: wrote ${parsed.length} records -> ${outPath}`);

await browser.close();
