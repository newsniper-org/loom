import { test, expect } from "@playwright/test";

// RQ6 §7 게이트 적용(Task #29) 전용 — M8/M9를 §7이 요구하는 정밀도로 두 팔
// 모두에서 실측한다. 기존 m8-m9-widget-embed-island.spec.ts는 Island 팔의
// 단발 정확성만 확인했고(§7 자격 게이트로는 충분), M9는 원래 RQ1 §8 정확도/
// 지연시간 반복시행 관행(200회, 병렬 대신 순차, in-page performance.now())을
// 따라야 §9-11의 등가 대역(±10%)을 적용할 수 있다 — 이 스펙이 그 실측이다.

const ISLAND_BASE = "/demo/widget-embed/index.html";
const FLAT_BASE = "/demo/widget-embed/flat/index.html";
const TRIALS = 200;

function median(xs: number[]): number {
  const sorted = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}
function p95(xs: number[]): number {
  const sorted = [...xs].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(0.95 * sorted.length) - 1)];
}

test.describe("M8: H-Qwik(Flatten) 팔 정합성 — 5개 위젯 전부 하나의 공유 인스턴스로 정상 동작", () => {
  test("ProductCard 토글 / QuantityPicker / CheckoutForm 3필드 / MiniDashboard 전부 동작", async ({ page }) => {
    await page.goto(FLAT_BASE);
    await page.waitForTimeout(200);

    // ProductCard의 "show details" 토글 (Action0, 무캡처)
    await page.locator("[data-loom-lref^='App_chunk|Action0']").click();
    await expect(page.locator("[data-loom-id='details']")).toHaveText("Free shipping on orders over $50.");

    // QuantityPicker (Action1, cart_count 캡처) — 이후 클릭은 전부 같은 공유 인스턴스 재사용.
    await page.locator("[data-loom-lref^='App_chunk|Action1']").click();
    await expect(page.locator("[data-loom-id='cart_count']")).toHaveText("1");
    await expect(page.locator("[data-loom-shared-subscriber='cartCount']")).toHaveText("1");

    // CheckoutForm 3개 독립 필드 (Action2/3/4) — 서로 크로스와이어되지 않아야 함
    // (Island 팔의 회귀 테스트와 동형 검증, 여기선 Flatten 팔 대상).
    await page.locator("[data-loom-lref^='App_chunk|Action2']").click();
    await expect(page.locator("[data-loom-id='shipping']")).toHaveText("Express shipping selected.");
    await expect(page.locator("[data-loom-id='gift']")).toHaveText("");
    await expect(page.locator("[data-loom-id='confirmed']")).toHaveText("");

    await page.locator("[data-loom-lref^='App_chunk|Action3']").click();
    await expect(page.locator("[data-loom-id='gift']")).toHaveText("Gift wrap selected.");
    await expect(page.locator("[data-loom-id='shipping']")).toHaveText("Express shipping selected."); // 아직 살아있어야 함

    await page.locator("[data-loom-lref^='App_chunk|Action4']").click();
    await expect(page.locator("[data-loom-id='confirmed']")).toHaveText("Order confirmed.");

    // MiniDashboard (Action5 visits, Action6 add tag)
    await page.locator("[data-loom-lref^='App_chunk|Action5']").click();
    await expect(page.locator("[data-loom-id='visits']")).toHaveText("1");
    await page.locator("[data-loom-lref^='App_chunk|Action6']").click();
    await expect(page.locator("[data-loom-id='tags']")).toContainText("promo");
  });
});

test.describe("M9: cross-widget reactivity — 정확도/지연시간 200회 반복, 양쪽 팔", () => {
  test(`Island 팔 (N=${TRIALS})`, async ({ page }) => {
    await page.goto(ISLAND_BASE);
    await page.waitForTimeout(300);
    const latencies: number[] = [];
    let correct = 0;
    for (let i = 0; i < TRIALS; i++) {
      const result = await page.evaluate(() => {
        const btn = document.querySelector("[data-loom-lref^='QuantityPicker_chunk']") as HTMLElement;
        const badge = document.querySelector("[data-loom-shared-subscriber='cartCount']") as HTMLElement;
        const start = performance.now();
        btn.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        const end = performance.now();
        return { badgeText: badge.textContent, latency: end - start };
      });
      latencies.push(result.latency);
      if (result.badgeText === String(i + 1)) correct++;
    }
    console.log(
      `M9 Island(widget-embed) N=${TRIALS}: accuracy=${correct}/${TRIALS} median=${median(latencies).toFixed(3)}ms p95=${p95(latencies).toFixed(3)}ms max=${Math.max(...latencies).toFixed(3)}ms`
    );
    expect(correct).toBe(TRIALS);
  });

  test(`H-Qwik(Flatten) 팔 (N=${TRIALS})`, async ({ page }) => {
    await page.goto(FLAT_BASE);
    await page.waitForTimeout(200);
    // 첫 클릭은 materialize 비용(청크 로드+wasm init)이 섞여 있어 이후
    // 반복시행과 별개로 분리 — M9(정상 상태 지연시간)는 웜 인스턴스 기준.
    await page.locator("[data-loom-lref^='App_chunk|Action1']").click();
    await expect(page.locator("[data-loom-id='cart_count']")).toHaveText("1");

    const latencies: number[] = [];
    let correct = 0;
    for (let i = 0; i < TRIALS; i++) {
      const result = await page.evaluate(() => {
        const btn = document.querySelector("[data-loom-lref^='App_chunk|Action1']") as HTMLElement;
        const badge = document.querySelector("[data-loom-shared-subscriber='cartCount']") as HTMLElement;
        const start = performance.now();
        btn.dispatchEvent(new MouseEvent("click", { bubbles: true }));
        const end = performance.now();
        return { badgeText: badge.textContent, latency: end - start };
      });
      latencies.push(result.latency);
      if (result.badgeText === String(i + 2)) correct++; // +2: 위 warm-up 클릭이 이미 1을 만들어둠
    }
    console.log(
      `M9 H-Qwik(Flatten, widget-embed) N=${TRIALS}: accuracy=${correct}/${TRIALS} median=${median(latencies).toFixed(3)}ms p95=${p95(latencies).toFixed(3)}ms max=${Math.max(...latencies).toFixed(3)}ms`
    );
    expect(correct).toBe(TRIALS);
  });
});
