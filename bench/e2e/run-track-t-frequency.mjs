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
const arms = opt("arms", "B,F").split(",");
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
for (const s of grid.s) for (const f of grid.f) for (const arm of arms) cells.push({ arm, f, s });

const totalSeconds = (cells.length * trials * windowMs) / 1000;
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

    let cdp = null;
    if (throttleRate !== 1) {
      cdp = await context.newCDPSession(page);
      await cdp.send("Emulation.setCPUThrottlingRate", { rate: throttleRate });
    }

    const url =
      `${baseUrl}/demo/track_t_freq_bench.html` +
      `?arm=${cell.arm}&f=${cell.f}&size=${cell.s}&window=${windowMs}`;
    await page.goto(url);
    // 두 번째 위치 인자는 페이지 함수에 넘길 `arg`이고 옵션은 **세 번째**다 —
    // `undefined`를 빼먹으면 `{timeout}`이 arg로 먹히고 기본 30초 타임아웃이
    // 적용된다(창 60초 캠페인이 첫 셀에서 즉사하는 원인이었음).
    await page.waitForFunction(() => window.__loomTrackTDone === true, undefined, {
      timeout: windowMs + 60_000,
    });
    const json = await page.locator("#cell-result").textContent();
    const parsed = JSON.parse(json);
    parsed.trial = trial;
    parsed.browser = browserName;
    parsed.cpu_throttle_rate = throttleRate;
    records.push(parsed);

    await context.close();
    done++;
    if (trial === trials - 1) {
      console.log(
        `[track-t] ${done}/${cells.length * trials} — arm=${cell.arm} f=${cell.f}/s s=${cell.s} 완료 ` +
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
  is_preregistered_run: windowMs === 60000 && trials === 10 && gridName !== "pilot",
  unisolated_record_count: unisolated,
  known_gaps: [
    "§4.1이 요구하는 6 arm 중 B와 F(=cond.rs가 'CSS 은닉(v-show형) 대조군을 겸한다'고 명시한 변형) 2개만 존재 — F-i(CondEffective 값 필터)/F-ii(detached DOM 계속 패치)/F-iii(Without<Frozen> marker)는 구현 자체가 없음",
    "§4.1의 부속 축(서브트리 구성 flat vs 파생 체인 깊이3, catch-up 곡선, attached 구간 변경률) 미구현 — 기본 f×s 격자만",
    "그 결과 §9 F 기각의 1차 분기('F-i/ii/iii 각각이 통상 구간에서 B를 이기는 셀 부재')는 적용 불가 — 적용 가능한 것은 '또는' 분기(B의 spawn 비용 < 1ms/100 entity, 비스로틀·4× 스로틀 양쪽 성립)뿐",
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
