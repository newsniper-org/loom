import { test, expect } from "@playwright/test";

// RQ6 §7 게이트 적용(Task #29) — M6을 §5.4가 요구하는 조건 A로, "정체성 통제"
// 픽스처(demo/widget-embed) 자체에서 직접 측정한다. 지금까지의 M6 데이터는
// (a) demo/islands/condition-b — 다른(더 작은, 비-정체성-통제) 2-컴포넌트
// 픽스처의 조건 B, (b) demo/islands/nscaling — 같은 위젯의 N-스케일링(같은
// *타입*의 다중 인스턴스)뿐이었다. 여기선 widget-embed의 **서로 다른 위젯
// 타입끼리**(QuantityPicker vs CheckoutForm vs MiniDashboard vs ProductCard)
// 조건 A(오늘 실제 배포된 옴니버스 크레이트 — 4개 위젯 타입 전부가 사실상
// 하나의 WebAssembly.Instance를 공유함, Task #27의 M1 조사로 이미 확인됨)에서
// panic이 실제로 전파되는지를 직접 잰다.

const BASE = "/demo/widget-embed/index.html";

test("조건 A: QuantityPicker에 실제 panic 주입 후 CheckoutForm/MiniDashboard(다른 타입)가 생존하는가", async ({ page }) => {
  await page.goto(BASE);
  await page.waitForTimeout(300);

  // 베이스라인 — 3개 위젯 타입 전부 정상 동작 확인.
  await page.locator("[data-loom-lref^='QuantityPicker_chunk']").click();
  await expect(page.locator("[data-loom-id='cart_count']")).toHaveText("1");
  await page.locator("[data-loom-lref^='CheckoutForm_chunk|Action0']").click();
  await expect(page.locator("[data-loom-id='shipping']")).toHaveText("Express shipping selected.");
  await page.locator("[data-loom-lref^='MiniDashboard_chunk|Action0']").click();
  await expect(page.locator("[data-loom-id='visits']")).toHaveText("1");

  // QuantityPicker가 로드한 것과 같은 wasm 모듈을 가리키는 자유 함수로 실제
  // panic 주입 — M4/M6 N-스케일링과 동일한 훅, 이번엔 "다른 타입" 간 전파를
  // 관찰하는 데 쓴다.
  let panicThrew = false;
  try {
    await page.evaluate(async () => {
      const mod = await import("../islands/chunks/quantity_picker.js");
      (mod as any).__loom_debug_panic_shared_instance();
    });
  } catch {
    panicThrew = true;
  }

  // panic 이후: CheckoutForm/MiniDashboard(QuantityPicker와 "다른 타입")가
  // 계속 응답하는지 확인 — 이게 "조건 A에서도 Island 구조가 서로 다른
  // 위젯 타입 간에는 그래도 뭔가 격리를 제공하는가"라는, §5.4가 명시
  // "예상과 다르면 반드시 규명" 대상으로 지목한 바로 그 질문이다.
  let checkoutSurvived = false;
  let miniDashSurvived = false;
  try {
    await page.locator("[data-loom-lref^='CheckoutForm_chunk|Action1']").click();
    await expect(page.locator("[data-loom-id='gift']")).toHaveText("Gift wrap selected.");
    checkoutSurvived = true;
  } catch {}
  try {
    await page.locator("[data-loom-lref^='MiniDashboard_chunk|Action0']").click();
    await expect(page.locator("[data-loom-id='visits']")).toHaveText("2");
    miniDashSurvived = true;
  } catch {}

  console.log(
    `M6 조건 A widget-embed(다른 타입 간): panic threw=${panicThrew}, CheckoutForm survived=${checkoutSurvived}, MiniDashboard survived=${miniDashSurvived}`
  );
  expect(panicThrew).toBe(true);
  // 실측 결과를 그대로 기록 — 사전 예단하지 않음(§5.4).
});
