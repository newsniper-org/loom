// RQ1 트랙 T — 토글 빈도(f) 축 캠페인 드라이버.
//
// `RQ1-benchmark-protocol.md` §4.1(격자) + §8(표본 규정·격리·타이머)을 그대로
// 집행한다. 특히 §8의 "셀당 독립 10시행, **시행마다 fresh page load**"가
// 이 파일이 존재하는 이유 — 한 페이지에서 셀을 연달아 돌리면 그 조항 위반이다.
//
// 사용법 (반드시 `python3 bench/serve-isolated.py`가 떠 있어야 함 — COOP/COEP
// 없이는 타이머가 100µs로 클램프돼 §8 타이머 조항을 못 지킨다):
//
//   node bench/e2e/run-track-t-frequency.mjs --out ../../research-questions/data/rq1-track-t-frequency-chromium.json
//
// 옵션:
//   --browser chromium|firefox   (기본 chromium — §8 커버리지 표의 1차 판정)
//   --window-ms 60000            (§4.1 사전 등록값; 축소하면 판정 데이터 아님)
//   --trials 10                  (§8 사전 등록값; 축소하면 판정 데이터 아님)
//   --grid full|normal-band|pilot
//   --throttle 1|4               (§8: 4× CPU 스로틀, Chromium CDP 전용)
//   --arms B,F
//
// **격자 정의**
//   full        : §4.1 그대로 f∈{0.1,1,10,30,60} × s∈{1,10,100,1000} (20셀)
//   normal-band : §9 F 기각 조항의 "통상 구간" f∈[0.1,10]×s∈[1,100] (9셀)
//   pilot       : 하네스 검증용 축소 격자 — **판정 데이터로 쓰지 말 것**

import { chromium, firefox } from "@playwright/test";
import { writeFileSync } from "node:fs";

const args = process.argv.slice(2);
function opt(name, fallback) {
  const i = args.indexOf(`--${name}`);
  return i >= 0 && args[i + 1] !== undefined ? args[i + 1] : fallback;
}

const browserName = opt("browser", "chromium");
const windowMs = Number(opt("window-ms", "60000"));
const trials = Number(opt("trials", "10"));
const gridName = opt("grid", "normal-band");
const throttleRate = Number(opt("throttle", "1"));
// §4 "구현 arm 6개". 기본값은 6개 전부 — 이전 기본값("B,F")은 §4가 요구하는
// 것의 3분의 1이었다.
const arms = opt("arms", "B,F,F-i,F-ii,F-iii,lazy").split(",");
const changeRate = Number(opt("rate", "0")); // §4.1 상류 변경률 축(%)
// §4.1 서브트리 구성 축 — "s=100 행에서만 양쪽"이므로 드라이버 인자로 둔다.
const composition = opt("comp", "flat");
// §4.1 catch-up 곡선 축(M6). --axis catchup 이면 f 대신 freeze 격자를 돈다.
const axis = opt("axis", "frequency");
const freezeList = opt("freeze", "1000,10000").split(",").map(Number);
const outPath = opt("out", null);
const baseUrl = opt("base-url", "http://localhost:8931");

const GRIDS = {
  full: { f: [0.1, 1, 10, 30, 60], s: [1, 10, 100, 1000] },
  "normal-band": { f: [0.1, 1, 10], s: [1, 10, 100] },
  // §8 "하드웨어·스로틀"이 사전 등록한 트랙 T의 4× CPU 스로틀 셀 집합:
  // "트랙 T의 s∈{100,1000} × f∈{0.1,1,10} 셀(v1.1 확장 — F-2)". §9 "또는"
  // 분기가 "비스로틀과 4× 스로틀 **양쪽 모두**에서 성립할 때만 발동"이라고
  // 못박고 있으므로, 이 격자를 안 돌리면 그 분기는 애초에 발동할 수 없다.
  "throttle-cells": { f: [0.1, 1, 10], s: [100, 1000] },
  pilot: { f: [1, 10], s: [10, 100] },
};
const grid = GRIDS[gridName];
if (!grid) throw new Error(`unknown --grid ${gridName} (expected ${Object.keys(GRIDS).join("|")})`);

const cells = [];
if (axis === "catchup") {
  // §4.1: 동결 기간 {1s, 10s} × 동결 중 변경률 {0, 10, 100%}. 변경률은
  // --rate로 한 번에 하나씩 돈다(격자 바깥에서 순회) — f-격자 쪽 관례와 동일.
  for (const s of grid.s) for (const fz of freezeList) for (const arm of arms) cells.push({ arm, s, freeze: fz });
} else {
  for (const s of grid.s) for (const f of grid.f) for (const arm of arms) cells.push({ arm, f, s });
}

// catch-up 셀은 창이 없고 동결 기간 + 소수 tick으로 끝난다.
const perCellSeconds = axis === "catchup" ? Math.max(...freezeList) / 1000 + 2 : windowMs / 1000;
const totalSeconds = cells.length * trials * perCellSeconds;
console.log(
  `[track-t] browser=${browserName} grid=${gridName} cells=${cells.length} trials=${trials} ` +
    `window=${windowMs}ms throttle=${throttleRate}x arms=${arms.join("/")}`
);
console.log(`[track-t] 예상 순수 측정 시간 ≈ ${(totalSeconds / 60).toFixed(1)}분 (페이지 로드 오버헤드 별도)`);

const launcher = browserName === "firefox" ? firefox : chromium;
const browser = await launcher.launch();

if (throttleRate !== 1 && browserName !== "chromium") {
  throw new Error("--throttle는 Chromium CDP 전용 — §8이 Firefox/WebKit은 비스로틀만으로 명시");
}

const records = [];
let done = 0;
for (const cell of cells) {
  for (let trial = 0; trial < trials; trial++) {
    // §8 격리: 시행마다 fresh context + fresh page load.
    const context = await browser.newContext();
    const page = await context.newPage();
    // 즉시 실패 — wasm 패닉이면 셀이 영원히 안 끝난다(2026-08-03 트랙 L에서
    // 실제로 6시간 매달렸다).
    let pageErr = null;
    page.on("pageerror", (e) => {
      pageErr = e;
      console.error("[pageerror]", e.message);
    });

    let cdp = null;
    if (throttleRate !== 1) {
      cdp = await context.newCDPSession(page);
      await cdp.send("Emulation.setCPUThrottlingRate", { rate: throttleRate });
    }

    const url =
      axis === "catchup"
        ? `${baseUrl}/demo/track_t_freq_bench.html` +
          `?axis=catchup&arm=${cell.arm}&size=${cell.s}&freeze=${cell.freeze}` +
          `&rate=${changeRate}&comp=${composition}`
        : `${baseUrl}/demo/track_t_freq_bench.html` +
          `?arm=${cell.arm}&f=${cell.f}&size=${cell.s}&window=${windowMs}` +
          `&rate=${changeRate}&comp=${composition}`;
    await page.goto(url);
    // 두 번째 위치 인자는 페이지 함수에 넘길 `arg`이고 옵션은 **세 번째**다 —
    // `undefined`를 빼먹으면 `{timeout}`이 arg로 먹히고 기본 30초 타임아웃이
    // 적용된다(창 60초 캠페인이 첫 셀에서 즉사하는 원인이었음).
    await page.waitForFunction(() => window.__loomTrackTDone === true, undefined, {
      timeout: (axis === "catchup" ? cell.freeze : windowMs) + 60_000,
    });
    if (pageErr) {
      await browser.close();
      throw new Error(`셀이 wasm 패닉/트랩으로 죽었다: ${pageErr.message}`);
    }
    const json = await page.locator("#cell-result").textContent();
    const parsed = JSON.parse(json);
    parsed.trial = trial;
    parsed.browser = browserName;
    parsed.cpu_throttle_rate = throttleRate;
    parsed.change_rate_pct = changeRate;
    records.push(parsed);

    await context.close();
    done++;
    if (trial === trials - 1) {
      const what =
        axis === "catchup"
          ? `freeze=${cell.freeze}ms s=${cell.s}`
          : `f=${cell.f}/s s=${cell.s}`;
      console.log(
        `[track-t] ${done}/${cells.length * trials} — arm=${cell.arm} ${what} comp=${composition} 완료 ` +
          `(res=${parsed.timer_resolution_ms}ms isolated=${parsed.cross_origin_isolated})`
      );
    }
  }
}

await browser.close();

// §8 타이머 조항 자체 점검 — 격리가 안 걸린 채 수집된 셀이 있으면 그 자체가
// 프로토콜 위반이므로 조용히 넘기지 않고 결과에 박아 둔다.
const unisolated = records.filter((r) => !r.cross_origin_isolated).length;
if (unisolated > 0) {
  console.warn(
    `[track-t] 경고: ${unisolated}/${records.length} 레코드가 crossOriginIsolated=false — ` +
      `bench/serve-isolated.py로 서빙하지 않았을 가능성. §8 타이머 조항 미충족 데이터임.`
  );
}

const payload = {
  track: "T",
  axis: "toggle_frequency",
  protocol: "RQ1-benchmark-protocol.md §4.1 (격자) + §8 (표본·격리·타이머)",
  collected_at_note: "타임스탬프는 커밋 시각으로 갈음 — 하네스가 시계를 직접 찍지 않음",
  browser: browserName,
  grid: gridName,
  grid_definition: grid,
  arms,
  trials_per_cell: trials,
  window_ms: windowMs,
  cpu_throttle_rate: throttleRate,
  change_rate_pct: changeRate,
  is_preregistered_run: windowMs === 60000 && trials === 10 && gridName !== "pilot",
  unisolated_record_count: unisolated,
  composition: composition,
  measurement_axis: axis,
  freeze_ms_list: axis === "catchup" ? freezeList : null,
  known_gaps: [
    "2026-08-02: §4.1의 6 arm이 전부 구현됐다 — B, F(CSS 은닉), F-i, F-ii, F-iii, lazy(lazy-spawn-then-hide). 그 전까지 F 계열은 전부 f_common_toggle의 '첫 true — lazy spawn' 경로로 출발해 **사실상 lazy와 구별되지 않았다**; 이제 F 계열과 CSS는 창을 열기 전에 실체화하고(첫 토글 = detach), lazy와 B만 빈 상태에서 출발한다.",
    "2026-08-02: §4.1 부속 축 전부 배선됨 — 상류 변경률(--rate), 서브트리 구성(--comp flat|chain3, 조항상 s=100 행에서만 양쪽), catch-up 곡선(--axis catchup --freeze 1000,10000). 상주 CPU는 은닉 구간/표시 구간으로 갈라 보고한다(§4.1 F-1).",
    "2026-08-02: F-iii의 BindingVersion을 spawn 번들에 넣어 catch-up 경로를 되살렸다 — 그 전까지 bump_version_system의 쿼리가 아무것도 매치하지 않아 조용히 죽어 있었다.",
    "M4(메모리)·M5(상주 메모리)는 이 드라이버가 수집하지 않는다 — §4.1 '수집 명시'가 요구하므로 미수집 상태임을 명기한다.",
  ],
  records,
};

const out = JSON.stringify(payload, null, 2);
if (outPath) {
  writeFileSync(outPath, out + "\n");
  console.log(`[track-t] wrote ${records.length} records → ${outPath}`);
} else {
  console.log(out);
}
