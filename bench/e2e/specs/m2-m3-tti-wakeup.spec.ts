import { test, expect } from "@playwright/test";

// RQ6 §6 M2(위젯-로컬 TTI)/M3(wake-up payload) — Task #27.
//
// M2 정의(§6 표): "마운트 지점이 DOM에 존재하게 된 시점부터 해당 위젯의
// 상호작용이 정확히 처리 가능해지는(= registry 상태가 attached가 되는) 시점까지".
// 두 팔의 근본적 비대칭(사전에 인지하고 측정, §5.4/§9-6과 같은 원칙 — 결과를
// 예단하지 않음): Island의 client:load 위젯은 상호작용과 무관하게 페이지 로드
// 시점에 곧바로 활성화되므로 "페이지 네비게이션 시작 -> attached"를 잰다.
// client:visible(ProductCard)은 가시성이 트리거이므로 "스크롤로 가시화되는
// 시점 -> attached"를 잰다(스크롤 자체는 사용자 행동이라 프레임워크 비용에서
// 제외). H-Qwik(Flatten) 팔은 정의상 첫 상호작용 전까지 아무것도 안 하므로
// "그 위젯이 상호작용 가능해지는 시점"이 곧 "첫 클릭의 materialize+resume+
// dispatch 전체가 끝나는 시점"과 같다 — M2가 M3(wake-up payload)와 사실상
// 같은 것을 재는 결과로 수렴할 가능성을 §6 표가 이미 인정했고, 실제로 그렇게
// 나왔다(아래 참고).
//
// 반복 시행: RQ1 트랙 R 관행 재사용 — 시나리오당 15회, median/p95 보고(§7의
// 등가 대역 ±10%는 여기서는 적용하지 않음 — M2/M3는 사전등록 §9-11에 따라
// "판정"이 아니라 "데이터 등록"이며, Task #29가 §7 게이트를 적용할 몫).

const TRIALS = 15;

function median(xs: number[]): number {
  const sorted = [...xs].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
}
function p95(xs: number[]): number {
  const sorted = [...xs].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(0.95 * sorted.length) - 1)];
}

test.describe("M2: Island 팔 — client:load 위젯의 페이지-로드-부터-attached까지", () => {
  for (const [label, chunkPrefix, islandId] of [
    ["QuantityPicker", "QuantityPicker_chunk", "quantity-picker"],
    ["CheckoutForm", "CheckoutForm_chunk", "checkout-form"],
    ["MiniDashboard", "MiniDashboard_chunk", "mini-dashboard"],
  ] as const) {
    test(`${label}: N=${TRIALS}`, async ({ page }) => {
      const samples: number[] = [];
      for (let i = 0; i < TRIALS; i++) {
        await page.goto("/demo/widget-embed/index.html");
        const ms = await page.evaluate(
          (id) =>
            new Promise<number>((resolve) => {
              const poll = () => {
                const e = (window as any).__loomRegistry?._entries?.get(id);
                if (e && e.state === "attached") {
                  resolve(performance.now());
                } else {
                  requestAnimationFrame(poll);
                }
              };
              poll();
            }),
          islandId
        );
        samples.push(ms);
      }
      console.log(`M2 Island(${label}) N=${TRIALS}: median=${median(samples).toFixed(2)}ms p95=${p95(samples).toFixed(2)}ms`);
      expect(samples.length).toBe(TRIALS);
    });
  }
});

test.describe("M2: Island 팔 — client:visible(ProductCard)의 가시화-부터-attached까지", () => {
  test(`ProductCard: N=${TRIALS}`, async ({ page }) => {
    const samples: number[] = [];
    for (let i = 0; i < TRIALS; i++) {
      await page.goto("/demo/widget-embed/index.html");
      await page.waitForTimeout(100); // client:load 위젯들의 활성화가 먼저 끝나 간섭하지 않도록
      const ms = await page.evaluate(
        () =>
          new Promise<number>((resolve) => {
            const scrollStart = performance.now();
            document.querySelector("#app")!.scrollIntoView({ behavior: "instant" as ScrollBehavior });
            window.scrollBy(0, 1250); // demo/widget-embed/index.html의 1200px 스페이서를 지나 ProductCard를 뷰포트에 들어오게 함
            const poll = () => {
              const e = (window as any).__loomRegistry?._entries?.get("product-card");
              if (e && e.state === "attached") {
                resolve(performance.now() - scrollStart);
              } else {
                requestAnimationFrame(poll);
              }
            };
            poll();
          })
      );
      samples.push(ms);
    }
    console.log(`M2 Island(ProductCard, client:visible) N=${TRIALS}: median=${median(samples).toFixed(2)}ms p95=${p95(samples).toFixed(2)}ms`);
    expect(samples.length).toBe(TRIALS);
  });
});

test.describe("M2/M3: H-Qwik(Flatten) 팔 — 페이지-로드-부터-첫-클릭-처리-완료까지", () => {
  test(`N=${TRIALS}`, async ({ page }) => {
    const samples: number[] = [];
    for (let i = 0; i < TRIALS; i++) {
      await page.goto("/demo/widget-embed/flat/index.html");
      await page.locator("[data-loom-lref^='App_chunk|Action1']").click(); // QuantityPicker의 +버튼과 동형인 액션
      const wakeupMs = await page.evaluate(
        () =>
          new Promise<number>((resolve) => {
            const poll = () => {
              const w = window as any;
              if (w.__loomWakeupStart != null && w.__loomWakeupEnd != null) {
                resolve(w.__loomWakeupEnd - w.__loomWakeupStart);
              } else {
                requestAnimationFrame(poll);
              }
            };
            poll();
          })
      );
      samples.push(wakeupMs);
    }
    console.log(`M2/M3 H-Qwik(Flatten) N=${TRIALS}: median=${median(samples).toFixed(2)}ms p95=${p95(samples).toFixed(2)}ms`);
    expect(samples.length).toBe(TRIALS);
  });
});
