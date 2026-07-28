import { test, expect } from "@playwright/test";

// Task #28-b — RQ3 "알려진 약점" 2번(crash 시 상태변화를 관찰할 방법이 없음)의
// 완화책 검증: registry.js가 이제 상태 전이마다 window.__loomShared에
// '__island_state:<id>' 키로 값을 게시한다(demo/resume/registry.js의 setState
// 참고). 핵심 주장은 "이벤트가 아니라 保持형 상태"라서 늦게 붙은 관찰자도 절대
// 놓치지 않는다는 것 — 이 스펙이 그 주장 자체를 검증한다.

const BASE = "/demo/widget-embed/index.html";

test.describe("Task #28-b: 상태변화 통지 채널", () => {
  test("정상 attach 전이가 __loomShared에 게시된다", async ({ page }) => {
    await page.goto(BASE);
    await page.waitForTimeout(300);
    const published = await page.evaluate(() => (window as any).__loomShared.get("__island_state:quantity-picker"));
    expect(published).toBe("attached");
  });

  test("crash(failed 전이)도 게시되고, 늦게 붙은 관찰자도 놓치지 않는다", async ({ page }) => {
    await page.goto(BASE);
    await page.waitForTimeout(200);

    // 실제 crash 시나리오를 인위적으로 재현: 존재하지 않는 청크를 가리키는
    // Island를 새로 등록하고 activate() — registry.js의 try/catch가 실제로
    // "failed" 경로를 타게 만든다(가짜 성공이 아니라 진짜 실패 경로).
    const result = await page.evaluate(() => {
      return new Promise<{ finalState: string; lateObserverSawIt: boolean }>((resolve) => {
        const fakeHost = document.createElement("div");
        document.body.appendChild(fakeHost);
        const id = "crash-test-island";
        (window as any).__loomRegistry.register(id, fakeHost, "this-chunk-does-not-exist", "NoSuchCtor");
        (window as any).__loomRegistry.activate(id).then(() => {
          // "늦게 붙은 관찰자" — 전이가 이미 다 끝난 뒤에야 처음으로 조회한다.
          // 保持형 스토어라면 유실 없이 현재값을 그대로 돌려줘야 한다.
          const lateRead = (window as any).__loomShared.get(`__island_state:${id}`);
          resolve({
            finalState: (window as any).__loomRegistry.getState(id),
            lateObserverSawIt: lateRead === "failed",
          });
        });
      });
    });

    expect(result.finalState).toBe("failed");
    expect(result.lateObserverSawIt).toBe(true);
  });

  test("detach 시퀀스(attached→detaching→detached)도 각 단계가 게시된다", async ({ page }) => {
    await page.goto(BASE);
    await page.waitForTimeout(300);

    const states = await page.evaluate(() => {
      return new Promise<string[]>((resolve) => {
        const seen: string[] = [];
        (window as any).__loomRegistry.beginDetach("mini-dashboard");
        seen.push((window as any).__loomShared.get("__island_state:mini-dashboard")); // "detaching"
        (window as any).__loomRegistry.finishDetach("mini-dashboard");
        seen.push((window as any).__loomShared.get("__island_state:mini-dashboard")); // "detached"
        resolve(seen);
      });
    });

    expect(states).toEqual(["detaching", "detached"]);
  });
});
