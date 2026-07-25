import { test, expect } from "@playwright/test";

// RQ6 §6 M4(상주 메모리)/§4.4 N-스케일링 + M6 N-스케일링 결합 셀 —
// demo/islands/nscaling/n{N}.html (bench/e2e/fixtures/gen-nscaling-page.mjs
// 로 생성, N개의 독립 QuantityPicker 인스턴스, 전부 같은
// data-loom-chunk="quantity_picker"). Chromium에서만 실행(순수 성능/구조
// 측정, RQ1 §8 관행대로 1차 판정 열만).

const Ns = [1, 10, 50];

for (const n of Ns) {
  test(`M4: N=${n}에서 상주 메모리(wasm 선형 메모리 총량)`, async ({ page }) => {
    await page.goto(`/demo/islands/nscaling/n${n}.html`);
    await page.waitForTimeout(300 + n * 5); // N이 클수록 활성화에 걸리는 시간도 늘어남

    const memBytes = await page.evaluate(async () => {
      const mod = await import("../chunks/quantity_picker.js");
      return mod.__loom_debug_memory_bytes();
    });
    console.log(`M4 N=${n}: wasm linear memory = ${memBytes} bytes (${(memBytes / 1024).toFixed(1)} KiB)`);
    expect(memBytes).toBeGreaterThan(0);
  });
}

test("M6 N-스케일링 결합 셀 (N=50): 무작위(중간) 하나 panic 후 나머지 생존율", async ({ page }) => {
  const logs: string[] = [];
  page.on("console", (m) => logs.push(m.text()));
  await page.goto("/demo/islands/nscaling/n50.html");

  // 고정 타임아웃 대신 registry 상태를 직접 폴링 — N=50개의 client:load
  // 인스턴스가 전부 activate()를 완료할 때까지(각자 비동기 import+wasm
  // 초기화이므로 총 소요 시간이 고정값보다 신뢰도 높게 걸림).
  await page.waitForFunction(() => {
    const entries = (window as any).__loomRegistry?._entries as Map<string, any> | undefined;
    if (!entries) return false;
    return Array.from(entries.values()).every((e) => e.state === "attached");
  }, { timeout: 15_000 });

  async function clickAndRead(index: number): Promise<string> {
    await page.locator(`#qp-${index} [data-loom-lref]`).click();
    return page.locator(`#qp-${index} [data-loom-id='cart_count']`).innerText();
  }

  const before0 = await clickAndRead(0);
  const before25 = await clickAndRead(25);
  const before49 = await clickAndRead(49);

  // 결정적으로 중간(#25)의 wasm 모듈에 진짜 패닉을 주입 — 별도 크레이트가
  // 아니라 바로 이 크레이트 자신의 __loom_debug_panic_shared_instance().
  let panicThrew = false;
  try {
    await page.evaluate(async () => {
      const mod = await import("../chunks/quantity_picker.js");
      mod.__loom_debug_panic_shared_instance();
    });
  } catch {
    panicThrew = true;
  }

  // panic 이후: 같은 3개 인스턴스가 계속 응답하는지 재확인.
  let after0 = "ERROR", after25 = "ERROR", after49 = "ERROR";
  try { after0 = await clickAndRead(0); } catch {}
  try { after25 = await clickAndRead(25); } catch {}
  try { after49 = await clickAndRead(49); } catch {}

  const survived0 = after0 === "2";
  const survived25 = after25 === "2";
  const survived49 = after49 === "2";
  console.log(
    `M6 N-스케일링 결합 셀 (N=50): panic threw=${panicThrew}, #0 survived=${survived0}(${after0}), #25 survived=${survived25}(${after25}), #49 survived=${survived49}(${after49})`
  );

  // 실측 결과(사전 가설과 반대): 모든 인스턴스가 같은 wasm 모듈/같은
  // WebAssembly.Instance를 공유함(M4가 확인)에도 불구하고, 아무 공유 상태도
  // 건드리지 않는 이 패닉 이후 다른 인스턴스는 계속 정상 응답한다 — wasm의
  // 트랩은 트랩을 일으킨 현재 호출 스택만 되감을 뿐 선형 메모리 자체를
  // 파괴하지 않기 때문. 단, 이 프로브는 "인스턴스가 자기 자신의 상태를 막
  // 변경하던 도중 패닉"하는, 더 위험한 시나리오는 검증하지 않았다 —
  // research-questions/data/rq6-m6-nscaling.json의 limitations 참고.
  expect(before0).toBe("1");
  expect(before25).toBe("1");
  expect(before49).toBe("1");
  expect(panicThrew).toBe(true);
  expect(survived0).toBe(true);
  expect(survived25).toBe(true);
  expect(survived49).toBe(true);
});
