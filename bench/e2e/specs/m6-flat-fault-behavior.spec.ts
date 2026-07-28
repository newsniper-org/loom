import { test, expect } from "@playwright/test";

// RQ6 §7 게이트 적용(Task #29) — M6 조건 A를 H-Qwik(Flatten) 팔에서도 측정.
// §5.4의 사전 예상("조건 A에서는 두 팔 사이에 격리 차이가 없을 것")을 검증하려면
// H-Qwik 팔 자체의 장애 거동도 실측해야 한다 — Island 쪽만 재고 H-Qwik은
// 안 재면 비교가 성립하지 않는다.

const FLAT_BASE = "/demo/widget-embed/flat/index.html";

test("Flatten 팔: 무관한 위치의 실제 panic 주입 후 모든 위젯이 계속 응답하는가", async ({ page }) => {
  await page.goto(FLAT_BASE);
  await page.waitForTimeout(200);

  // 베이스라인 — 최초 클릭이 전체 앱을 materialize.
  await page.locator("[data-loom-lref^='App_chunk|Action1']").click(); // QuantityPicker 동형
  await expect(page.locator("[data-loom-id='cart_count']")).toHaveText("1");

  let panicThrew = false;
  try {
    await page.evaluate(async () => {
      const mod = await import("../flat/chunks/App_chunk.js");
      (mod as any).__loom_debug_panic_shared_instance();
    });
  } catch {
    panicThrew = true;
  }

  // panic 이후: 서로 다른 "위젯"들(MiniDashboard/CheckoutForm 동형 액션)이
  // 계속 응답하는지 확인 — Island 쪽 m6-widget-embed-cross-type-isolation.spec.ts
  // 와 정확히 대응되는 실측.
  let checkoutSurvived = false;
  let miniDashSurvived = false;
  try {
    await page.locator("[data-loom-lref^='App_chunk|Action2']").click(); // CheckoutForm 동형
    await expect(page.locator("[data-loom-id='shipping']")).toHaveText("Express shipping selected.");
    checkoutSurvived = true;
  } catch {}
  try {
    await page.locator("[data-loom-lref^='App_chunk|Action5']").click(); // MiniDashboard 동형
    await expect(page.locator("[data-loom-id='visits']")).toHaveText("1");
    miniDashSurvived = true;
  } catch {}

  console.log(
    `M6 조건 A widget-embed Flatten 팔: panic threw=${panicThrew}, CheckoutForm동형 survived=${checkoutSurvived}, MiniDashboard동형 survived=${miniDashSurvived}`
  );
  expect(panicThrew).toBe(true);
});
