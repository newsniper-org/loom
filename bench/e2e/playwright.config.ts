import { defineConfig, devices } from "@playwright/test";

// RQ6 §8이 그대로 준용하는 RQ1-benchmark-protocol.md §8 환경 명세: Chromium이
// 1차 판정(primary), Firefox/WebKit은 교차 확인(cross-check)만 — 절대
// 풀링/평균 금지(§7/§8 규칙, 결과는 항상 프로젝트별로 분리 보고).
//
// WebKit 실행 환경 노트: 이 호스트(Arch/CachyOS, 롤링 릴리즈)에서 직접
// `npx playwright test --project=webkit`을 돌리면 실패한다 — Playwright의
// WebKit 프리빌드 바이너리가 우분투 24.04 전용(구 SONAME, 예: libxml2.so.2)
// 이라 Arch의 최신 라이브러리와 ABI가 안 맞음(패키지 설치로 해결 안 되는
// 근본적인 배포판 불일치, sudo로도 못 고침 — 실제로 icu74를 AUR로 설치해봐도
// xml2/flite1 SONAME 불일치는 그대로 남음). 해결책(병익님 제안): 우분투
// 24.04 기반 공식 Playwright 컨테이너(`mcr.microsoft.com/playwright:v1.61.0-noble`)
// 안에서 WebKit만 실행 — 호스트에는 `--project=webkit`이 없고, 대신
// `bench/e2e/run-webkit-in-docker.sh`가 컨테이너를 띄워 이 설정의 "webkit"
// 프로젝트를 실행한다(같은 devices["Desktop Safari"], `--network=host`로
// 호스트의 데모 서버(localhost:8931)에 접근).
export default defineConfig({
  testDir: "./specs",
  timeout: 30_000,
  fullyParallel: false,
  workers: 1, // 반복시행 통계의 타이밍 간섭을 피하려 직렬 실행
  reporter: [["list"], ["json", { outputFile: "results/last-run.json" }]],
  use: {
    baseURL: "http://localhost:8931",
  },
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } }, // 호스트에서는 launch 실패 — run-webkit-in-docker.sh로 실행
  ],
});
