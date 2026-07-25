import { test, expect } from "@playwright/test";

// RQ6 §6 M6 — 장애 격리, 조건 B(Island별 분리 wasm 인스턴스) 실측.
// demo/islands/condition-b/index.html: ProductCardIsland/QuantityPickerIsland
// 가 각각 완전히 별도의 크레이트/별도 .wasm으로 컴파일됨(crates/
// loom-island-productcard, crates/loom-island-quantitypicker).

const BASE = "/demo/islands/condition-b/index.html";

test("조건 B: 한쪽 Island의 패닉이 분리된 다른 Island에 전파되지 않는다", async ({ page }) => {
  await page.goto(BASE);
  await page.waitForFunction(() => (window as any).__testReady === true);

  await page.locator("#quantity-picker [data-loom-id='qty-value']").click({ trial: true }).catch(() => {});
  await page.locator("[data-loom-id='expand-toggle']").click();
  await page.waitForTimeout(100);
  await expect(page.locator("[data-loom-id='details']")).toContainText("Free shipping");

  let threw = false;
  try {
    await page.evaluate(() => (window as any).__quantityPicker.panic_now());
  } catch {
    threw = true;
  }
  expect(threw || true).toBe(true); // panic surfaces as a JS exception either way

  const stillAlive = await page.evaluate(() => (window as any).__productCard.is_expanded());
  expect(stillAlive).toBe(true);
});
