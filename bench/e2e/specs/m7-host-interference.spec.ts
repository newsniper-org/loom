import { test, expect } from "@playwright/test";

// RQ6 §6 M7 — 호스트 간섭 3패턴, 각 10회 독립 시행. Island 팔(demo/widget-embed)
// 만으로 측정 가능 — H-Qwik 팔 불필요(RQ6-verification-plan.md §5.3/§6 참고).
// §7 판정 규칙: 10회 중 pass 횟수 차이 0-1회는 동등, 2회 이상은 열세/우세
// (여기서는 절대 pass count만 기록 — 판정 자체는 Task #29 몫).

const BASE = "/demo/widget-embed/index.html";
const TRIALS = 10;

test.describe("M7 간섭-1: 호스트 속성 변조", () => {
  test("QuantityPicker는 형제 요소 속성이 계속 바뀌는 동안에도 정상 동작한다", async ({ page }) => {
    let passCount = 0;
    for (let i = 0; i < TRIALS; i++) {
      await page.goto(BASE);
      await page.waitForTimeout(200);
      // 호스트가 QuantityPicker와 무관한 형제 요소의 class/style을 계속 변조.
      await page.evaluate(() => {
        const target = document.body;
        (window as any).__interference1 = setInterval(() => {
          target.setAttribute("data-noise", String(Math.random()));
          target.className = target.className === "a" ? "b" : "a";
        }, 10);
      });
      await page.waitForTimeout(150);
      const before = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.locator("[data-loom-lref^='QuantityPicker_chunk']").click();
      await page.waitForTimeout(100);
      const after = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.evaluate(() => clearInterval((window as any).__interference1));
      if (Number(after) === Number(before) + 1) passCount++;
    }
    console.log(`M7 간섭-1 (Chromium/Firefox 별도 기록): ${passCount}/${TRIALS} pass`);
    expect(passCount).toBeGreaterThanOrEqual(TRIALS - 1); // 0-1회 실패까지는 §7 동등 판정 기준과 일치
  });
});

test.describe("M7 간섭-2: 호스트 DOM 재배치", () => {
  test("QuantityPicker 마운트 지점의 조상이 재배치되어도 정상 동작한다", async ({ page }) => {
    let passCount = 0;
    const observerFlags: boolean[] = [];
    for (let i = 0; i < TRIALS; i++) {
      await page.goto(BASE);
      await page.waitForTimeout(200);
      // registry.js의 watchForHostInitiatedRemoval()이 순수 재배치(제거 후
      // 같은 tick에 재삽입)를 오탐 detach로 처리하는지 함께 관찰 — RQ6 §27
      // 계획에서 미리 지목한 위험.
      const wrongfullyDetached = await page.evaluate(() => {
        return new Promise<boolean>((resolve) => {
          const host = document.querySelector("#app")!;
          const parent = host.parentElement!;
          const before = (window as any).__loomRegistry.getState("product-card");
          parent.insertBefore(host, parent.firstChild); // 재배치 — 제거 후 재삽입
          setTimeout(() => {
            const after = (window as any).__loomRegistry.getState("product-card");
            resolve(before === "dormant" && after === "detached");
          }, 150);
        });
      });
      observerFlags.push(wrongfullyDetached);
      await page.waitForTimeout(100);
      const before = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.locator("[data-loom-lref^='QuantityPicker_chunk']").click();
      await page.waitForTimeout(100);
      const after = await page.locator("[data-loom-id='cart_count']").innerText();
      if (Number(after) === Number(before) + 1) passCount++;
    }
    const falsePositives = observerFlags.filter(Boolean).length;
    console.log(`M7 간섭-2 (Chromium/Firefox 별도 기록): ${passCount}/${TRIALS} pass, MutationObserver 오탐 detach: ${falsePositives}/${TRIALS}`);
    expect(passCount).toBeGreaterThanOrEqual(TRIALS - 1);
  });
});

test.describe("M7 간섭-3: 전역 클릭 위임 (참고용, §7 게이트 미포함)", () => {
  test("호스트의 document 레벨 클릭 리스너가 있어도 위젯 자신의 디스패치가 손상되지 않는다", async ({ page }) => {
    let passCount = 0;
    for (let i = 0; i < TRIALS; i++) {
      await page.goto(BASE);
      await page.waitForTimeout(200);
      await page.evaluate(() => {
        document.addEventListener("click", () => {
          (window as any).__hostClickCount = ((window as any).__hostClickCount || 0) + 1;
        });
      });
      const before = await page.locator("[data-loom-id='cart_count']").innerText();
      await page.locator("[data-loom-lref^='QuantityPicker_chunk']").click();
      await page.waitForTimeout(100);
      const after = await page.locator("[data-loom-id='cart_count']").innerText();
      const hostClickCount = await page.evaluate(() => (window as any).__hostClickCount || 0);
      if (Number(after) === Number(before) + 1 && hostClickCount >= 1) passCount++;
    }
    console.log(`M7 간섭-3 (Chromium/Firefox 별도 기록, §7 게이트 미포함): ${passCount}/${TRIALS} pass`);
    expect(passCount).toBeGreaterThanOrEqual(TRIALS - 1);
  });
});
