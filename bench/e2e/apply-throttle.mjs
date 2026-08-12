// §8 "저사양 프록시: Chromium CDP 4× CPU throttle을 (i) **트랙 L 핵심 격자**,
// (ii) 트랙 T의 s∈{100,1000} × f∈{0.1,1,10} 셀에 추가" — 그중 (i)의 이행
// (개정 #9-7, 2026-08-12). 그동안 `--throttle`은 트랙 T 드라이버에만 있었다.
//
// **왜 별도 프로세스인가**: 판정 데이터는 stock 브라우저로 재야 한다(개정 #4 —
// Playwright 번들 Firefox가 대표 빌드가 아님이 실측으로 드러났고, 그래서
// `collect-stock.sh`가 공식 배포판을 직접 띄운다). 그런데 CPU 스로틀은 CDP
// 명령이라 드라이버 연결이 필요하다. 그래서 stock 브라우저를
// `--remote-debugging-port`로 띄우고 여기서 **붙기만** 한다 — 브라우저 빌드는
// stock 그대로이고 CDP는 스로틀 설정에만 쓰인다.
//
// 사용:
//   node bench/e2e/apply-throttle.mjs <포트> <배수>
// 예:
//   node bench/e2e/apply-throttle.mjs 9222 4
//
// 이 프로세스는 브라우저가 살아 있는 동안 계속 떠 있으면서, 페이지가 스스로
// 다음 그룹으로 이동할 때마다(§8 "셀마다 fresh page load") 스로틀을 **다시**
// 건다. 한 번만 걸고 마는 구현은 두 번째 셀부터 조용히 비스로틀이 된다 —
// 계측이 "존재한다"와 "조항이 요구하는 걸 잰다"의 차이가 정확히 그 지점이다.

import { chromium } from "playwright";

const port = Number(process.argv[2] ?? 9222);
const rate = Number(process.argv[3] ?? 4);
if (!Number.isFinite(rate) || rate < 1) {
  throw new Error(`스로틀 배수가 이상하다: ${process.argv[3]}`);
}

// 브라우저가 뜰 때까지 기다린다(collect-stock.sh가 백그라운드로 띄운 직후에
// 이 스크립트를 부르므로).
let browser = null;
for (let i = 0; i < 60 && !browser; i++) {
  try {
    browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  } catch {
    await new Promise((r) => setTimeout(r, 500));
  }
}
if (!browser) throw new Error(`CDP 포트 ${port}에 붙지 못했다`);

let applied = 0;

async function apply(page) {
  try {
    const cdp = await page.context().newCDPSession(page);
    await cdp.send("Emulation.setCPUThrottlingRate", { rate });
    applied++;
    console.log(`[throttle] rate=${rate}x 적용 (누적 ${applied}회) — ${page.url()}`);
  } catch (e) {
    // 페이지가 이미 이동/종료했을 수 있다. 다음 navigation에서 다시 건다.
    console.error(`[throttle] 적용 실패: ${e.message}`);
  }
}

async function watch(page) {
  await apply(page);
  // 자기 이동(`location.href = ...?group=i+1`)마다 다시 건다.
  page.on("framenavigated", async (frame) => {
    if (frame === page.mainFrame()) await apply(page);
  });
}

for (const context of browser.contexts()) {
  for (const page of context.pages()) await watch(page);
  context.on("page", watch);
}

console.log(`[throttle] 감시 시작 — 포트 ${port}, 배수 ${rate}x`);

// 브라우저가 죽으면 같이 끝난다.
browser.on("disconnected", () => {
  console.log(`[throttle] 브라우저 연결 종료 — 총 ${applied}회 적용`);
  process.exit(0);
});
