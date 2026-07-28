import { test, expect } from "@playwright/test";

// RQ6 §7 게이트 적용(Task #29) — M7을 H-Qwik(Flatten) 팔에서도 실측한다.
// §5.3은 "M7은 Island 전용 픽스처만으로 가능 — H-Qwik 팔 불필요"라고 사전
// 스코프를 정했었는데, §7 강한 형태 조건 (b)("M7 간섭-1/2에서 등가 등급")를
// 실제로 판정하려니 H-Qwik 팔 자체의 데이터가 없어 "등가 등급을 달성했다"는
// 명제를 검증할 방법이 없다는 게 드러났다 — 그 스코프 컷이 §7 강한 형태
// 판정 가능성 자체를 막는 효과를 냈다는 사실을 여기서 정직하게 메운다.
//
// 간섭-2(호스트 DOM 재배치)는 여기서 뺐다 — 그 패턴이 실제로 찌르는 대상은
// registry.js의 MutationObserver 기반 "호스트발 제거 오탐" 메커니즘인데,
// H-Qwik(bootstrap.js)에는 그런 생명주기 관찰자 자체가 없다(dormant/attached
// 같은 상태기계도 없음) — 있지도 않은 메커니즘의 오탐을 테스트할 수 없으므로
// "구조적으로 해당사항 없음"으로 기록하고 실측을 만들어내지 않는다
// (CLAUDE.md — 억지로 테스트를 만들지 않음).

const FLAT_BASE = "/demo/widget-embed/flat/index.html";
const TRIALS = 10;

test.describe("M7 간섭-1 (Flatten 팔): 호스트 속성 변조", () => {
  test("QuantityPicker 동형 액션은 형제 요소 속성이 계속 바뀌는 동안에도 정상 동작한다", async ({ page }) => {
    let passCount = 0;
    for (let i = 0; i < TRIALS; i++) {
      await page.goto(FLAT_BASE);
      await page.waitForTimeout(100);
      await page.evaluate(() => {
        const target = document.body;
        (window as any).__interference1 = setInterval(() => {
          target.setAttribute("data-noise", String(Math.random()));
          target.className = target.className === "a" ? "b" : "a";
        }, 10);
      });
      await page.waitForTimeout(100);
      const before = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.locator("[data-loom-lref^='App_chunk|Action1']").click();
      await page.waitForTimeout(100);
      const after = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.evaluate(() => clearInterval((window as any).__interference1));
      if (Number(after) === Number(before) + 1) passCount++;
    }
    console.log(`M7 간섭-1 Flatten팔 (Chromium/Firefox 별도 기록): ${passCount}/${TRIALS} pass`);
    expect(passCount).toBeGreaterThanOrEqual(TRIALS - 1);
  });
});

test.describe("M7 간섭-3 (Flatten 팔, 참고용, §7 게이트 미포함): 전역 클릭 위임 공존", () => {
  test("호스트가 별도 document 클릭 리스너를 추가해도 bootstrap.js 자신의 위임이 손상되지 않는다", async ({ page }) => {
    let passCount = 0;
    for (let i = 0; i < TRIALS; i++) {
      await page.goto(FLAT_BASE);
      await page.waitForTimeout(100);
      await page.evaluate(() => {
        document.addEventListener("click", () => {
          (window as any).__hostClickCount = ((window as any).__hostClickCount || 0) + 1;
        });
      });
      const before = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.locator("[data-loom-lref^='App_chunk|Action1']").click();
      await page.waitForTimeout(100);
      const after = await page.locator("[data-loom-id='cart_count']").innerText();
      const hostClickCount = await page.evaluate(() => (window as any).__hostClickCount || 0);
      if (Number(after) === Number(before) + 1 && hostClickCount >= 1) passCount++;
    }
    console.log(`M7 간섭-3 Flatten팔 (참고용, §7 게이트 미포함): ${passCount}/${TRIALS} pass`);
    expect(passCount).toBeGreaterThanOrEqual(TRIALS - 1);
  });
});
