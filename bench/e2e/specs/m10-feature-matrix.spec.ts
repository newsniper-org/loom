import { test, expect } from "@playwright/test";

// RQ6 §6 M10(기능 매트릭스) — RQ3 "알려진 약점" 8건 중, 이 픽스처+하네스로
// 직접 재현 가능한 항목만 실제 코드로 검증한다(나머지는
// research-questions/data/rq6-task27-m10-feature-matrix.json에 정성 판단으로
// 기록, 여기서 억지로 테스트를 만들지 않음 — CLAUDE.md 정직성 요구).

const BASE = "/demo/widget-embed/index.html";

test.describe("M10 항목 6: 호스트가 detachAndRemove() 없이 el.remove()로 직접 제거해도 좀비가 안 남는다", () => {
  test("MutationObserver가 잡아 registry 상태를 detached로 전이시킨다", async ({ page }) => {
    await page.goto(BASE);
    await page.waitForTimeout(200);

    const before = await page.evaluate(() => (window as any).__loomRegistry.getState("mini-dashboard"));
    expect(before).toBe("attached"); // client:load라 이미 활성화돼 있어야 함

    // RQ3 "알려진 약점" 6번이 상정하는 정확한 시나리오: registry의
    // detachAndRemove()를 거치지 않고, 레거시 스크립트가 직접 DOM을 지움.
    const after = await page.evaluate(() => {
      return new Promise<string>((resolve) => {
        document.querySelector('[data-loom-island-id="mini-dashboard"]')!.remove();
        const poll = () => {
          const state = (window as any).__loomRegistry.getState("mini-dashboard");
          if (state === "detached") {
            resolve(state);
          } else {
            requestAnimationFrame(poll);
          }
        };
        poll();
      });
    });
    expect(after).toBe("detached"); // "좀비"(attached로 영원히 남는 상태)가 아님을 확인
  });
});
