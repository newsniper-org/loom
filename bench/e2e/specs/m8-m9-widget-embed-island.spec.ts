import { test, expect } from "@playwright/test";

// RQ6 §6 M8(정합성)/M9(cross-widget reactivity) — Island 팔
// (demo/widget-embed/index.html), Chromium+Firefox 양쪽. 애드혹 Playwright
// 스크립트로 이미 확인했던 16개 시나리오를 이 저장소에 committed된 하네스로
// 옮긴 것 — 재실행 가능하게 만드는 게 이번 Task #27의 핵심.

const BASE = "/demo/widget-embed/index.html";

test("M8: 4개 Island이 각자 올바른 시점에 활성화된다", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(String(e)));
  await page.goto(BASE);
  await page.waitForTimeout(300);

  await expect(page.locator("[data-loom-id='cart_count']")).toHaveText("0");
  await expect(page.locator("[data-loom-id='visits']")).toHaveText("0");
  await expect(page.locator("[data-loom-id='shipping']")).toHaveText("");
  expect(await page.evaluate(() => (window as any).__loomRegistry.getState("product-card"))).toBe("dormant");
  expect(errors).toHaveLength(0);
});

test("M9: QuantityPicker 증가가 CartBadge에 실시간 반영된다 (RQ4 cross-widget)", async ({ page }) => {
  await page.goto(BASE);
  await page.waitForTimeout(300);
  const cartBadge = page.locator("[data-loom-shared-subscriber='cartCount']");
  const cartQty = page.locator("[data-loom-id='cart_count']");

  // ProductCard(QuantityPicker의 부모)가 아직 활성화되지 않은 상태에서도 동작.
  // 실측 중 발견: Playwright의 `locator.click()`은 대상이 화면 밖이면 자동
  // 스크롤하는데, QuantityPicker는 ProductCard 안에 물리적으로 중첩돼 있어
  // (1200px 스페이서 아래) 그 자동 스크롤이 ProductCard의
  // IntersectionObserver까지 같이 발동시켜버림(WebKit에서 재현 확인 —
  // Chromium/Firefox는 이 짧은 타임아웃 안에서 관찰되지 않았지만 근본적으로
  // 같은 경쟁 상태). 이건 프레임워크 버그가 아니라 테스트 자체의 아티팩트라
  // JS 레벨 `.dispatchEvent`로 우회해 진짜 "스크롤 없이" 시나리오를
  // 브라우저 간 일관되게 검증한다.
  await page.evaluate(() => {
    const btn = document.querySelector("[data-loom-lref^='QuantityPicker_chunk']") as HTMLElement;
    btn.dispatchEvent(new MouseEvent("click", { bubbles: true }));
  });
  await expect(cartQty).toHaveText("1");
  await expect(cartBadge).toHaveText("1");
  expect(await page.evaluate(() => (window as any).__loomRegistry.getState("product-card"))).toBe("dormant");
});

test("M8: opaque 스코핑 — ProductCard 활성화 후에도 QuantityPicker가 계속 동작", async ({ page }) => {
  await page.goto(BASE);
  await page.waitForTimeout(300);
  await page.locator("[data-loom-island-id='product-card']").scrollIntoViewIfNeeded();
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => (window as any).__loomRegistry.getState("product-card"))).toBe("attached");

  await page.locator("[data-loom-lref^='QuantityPicker_chunk']").click();
  await expect(page.locator("[data-loom-id='cart_count']")).toHaveText("1");
  await expect(page.locator("[data-loom-shared-subscriber='cartCount']")).toHaveText("1");
});

test("M8: CheckoutForm 3개 독립 필드가 서로 크로스와이어되지 않는다 (회귀)", async ({ page }) => {
  // codegen.rs의 CondSiteN 마커 수정을 실브라우저에서도 재확인 — 회귀 방지.
  await page.goto(BASE);
  await page.waitForTimeout(300);
  await page.locator("[data-loom-lref^='CheckoutForm_chunk|Action0']").click();
  await expect(page.locator("[data-loom-id='shipping']")).toContainText("Express shipping");
  await expect(page.locator("[data-loom-id='gift']")).toHaveText("");
  await expect(page.locator("[data-loom-id='confirmed']")).toHaveText("");

  await page.locator("[data-loom-lref^='CheckoutForm_chunk|Action1']").click();
  await expect(page.locator("[data-loom-id='gift']")).toContainText("Gift wrap");
  await expect(page.locator("[data-loom-id='shipping']")).toContainText("Express shipping"); // 유지되어야 함

  await page.locator("[data-loom-lref^='CheckoutForm_chunk|Action2']").click();
  await expect(page.locator("[data-loom-id='confirmed']")).toContainText("confirmed");
});

test("M8: MiniDashboard 카운터+리스트가 함께 동작한다", async ({ page }) => {
  await page.goto(BASE);
  await page.waitForTimeout(300);
  await page.locator("[data-loom-lref^='MiniDashboard_chunk|Action0']").click();
  await expect(page.locator("[data-loom-id='visits']")).toHaveText("1");
  await page.locator("[data-loom-lref^='MiniDashboard_chunk|Action1']").click();
  const tagsText = await page.locator("[data-loom-id='tags']").innerText();
  expect(tagsText).toContain("news");
  expect(tagsText).toContain("sale");
  expect(tagsText).toContain("promo");
});

test("M8: 'remove product card' 클릭이 detach cascade를 일으킨다", async ({ page }) => {
  await page.goto(BASE);
  await page.waitForTimeout(300);
  await page.locator("[data-loom-island-id='product-card']").scrollIntoViewIfNeeded();
  await page.waitForTimeout(300);
  await page.locator("#remove-card").click();
  await page.waitForTimeout(200);
  await expect(page.locator("[data-loom-island-id='product-card']")).toHaveCount(0);
});
