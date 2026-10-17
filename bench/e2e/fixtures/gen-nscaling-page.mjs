#!/usr/bin/env node
// RQ6 §4.4 N-스케일링(N∈{1,10,50}) 픽스처 생성기 — Task #27.
//
// crates/loom-rq6-fixture-gen라는 별도 Rust 크레이트를 새로 만드는 대신
// 이 스크립트로 대체함(의도적 선택, 숨기지 않음): N-스케일링이 실제로
// 필요로 하는 건 이미 컴파일된 QuantityPicker 청크(demo/islands/chunks/
// quantity_picker.js/.wasm)를 가리키는 마커 HTML을 N번 반복하는 것뿐이라
// — 컴파일러(loom_compiler)를 다시 구동할 이유가 없는 순수 문자열 템플릿
// 작업. "원본 앱 규모" 축처럼 실제로 큰 합성 .tsx를 새로 컴파일해야 하는
// 경우에만 Rust 툴이 정당화됨(별도로 처리).
//
// 생성물: demo/islands/nscaling/n{N}.html — N개의 독립 QuantityPicker
// 인스턴스(서로 다른 data-loom-island-id, 같은 data-loom-chunk="quantity_picker"
// — 이게 바로 "타입당 인스턴스 격리"를 실측하는 지점: 모두 같은 wasm
// 인스턴스를 공유하는지 여부).
import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
// 2026-10-17: `LOOM_NSCALING_OUT`로 출력 디렉터리를 바꿀 수 있다 — Rust 생성기(crates/loom-rq6-fixture-gen)와의
// 바이트 동일성 가드가 임시 디렉터리에 쓰게 하려는 것. 기본값(커밋 위치)과 출력 내용은 그대로다.
// 두 생성기 중 무엇을 쓸지는 gen-nscaling.sh의 설정으로 고른다(병익님 결정).
const outDir = process.env.LOOM_NSCALING_OUT ?? join(__dirname, "..", "..", "..", "demo", "islands", "nscaling");
mkdirSync(outDir, { recursive: true });

function quantityPickerBlock(i) {
  return `<div id="qp-${i}" data-loom-island data-loom-island-id="quantity-picker-${i}" data-loom-activation="load" data-loom-chunk="quantity_picker" data-loom-ctor="QuantityPickerIsland">
    <button data-loom-lref="QuantityPicker_chunk|Action0|cart_count:0">+</button>
    <span data-loom-id="cart_count">0</span>
  </div>`;
}

function page(n) {
  const blocks = Array.from({ length: n }, (_, i) => quantityPickerBlock(i)).join("\n  ");
  return `<!doctype html>
<html lang="ko">
<head><meta charset="utf-8" /><title>RQ6 §4.4 N-스케일링 (N=${n})</title></head>
<body>
  <!--
    Task #27 — N개의 독립 QuantityPicker 인스턴스(client:load), 전부
    같은 data-loom-chunk="quantity_picker" 청크를 가리킴. crates/
    loom-widget-embed의 __loom_debug_memory_bytes()로 wasm 선형 메모리
    총량을 관찰(M4), 무작위 하나를 panic_now 유사 방식으로 실패시켜
    나머지 생존율을 관찰(M6 결합 셀). 오늘의 registry.js/wasm-bindgen
    구성상 같은 chunk를 가리키는 인스턴스는 브라우저의 ES 모듈 캐시 덕에
    같은 WebAssembly.Instance(같은 선형 메모리)를 공유한다는 게 이 축의
    핵심 가설 — 실측으로 확인한다.
  -->
  <div id="app">
  ${blocks}
  </div>
  <script src="../../resume/shared-store.js"></script>
  <script src="../../resume/registry.js"></script>
</body>
</html>
`;
}

for (const n of [1, 10, 50]) {
  const path = join(outDir, `n${n}.html`);
  writeFileSync(path, page(n));
  console.log(`wrote ${path}`);
}
