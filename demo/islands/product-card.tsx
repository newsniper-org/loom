// Task #25 acceptance fixture — a real JSX source compiled through the real
// loomc pipeline (SWC parse -> ComponentGraph -> GraphMode::Islands codegen),
// replacing the hand-authored `crates/loom-island-demo` proof with a
// compiler-generated equivalent. Mirrors that crate's nested-Island shape
// (ProductCard `client:visible` nesting Widget `client:load`) but is
// deliberately NOT byte-for-byte identical to it — two things this fixture
// does NOT attempt, stated honestly rather than silently dropped:
//
//   - Widget here is a plain increment-only counter, not QuantityPicker's
//     clamped increment/decrement — `ClickAction` has no `Decrement`
//     variant (an existing, Island-unrelated IR gap), so a faithful
//     decrement isn't expressible yet regardless of Island support.
//   - There is no CartBadge/shared-store (`cartCount`) integration —
//     RQ4's cross-widget signal is only reachable via a hand-written
//     `#[wasm_bindgen] extern` FFI call today (Task #28-a's `sharedSignal()`
//     authoring surface is what would let a real `.tsx` source express
//     this; not yet implemented). This fixture proves exactly what Task
//     #25 is responsible for — nested Island composition, `client:load`/
//     `client:visible` activation, opaque-boundary DOM scoping, compiled
//     click dispatch — not RQ4's separate concern.
//
// **2026-10-17 정정**: 두 번째 항목의 "sharedSignal() authoring surface ...
// not yet implemented"는 낡았다 — `sharedSignal()`/`sharedValue()` 저작 표면은
// Task #26(2026-07-24, commit ece9da4)에 구현됐고 demo/widget-embed.tsx가 쓴다(ir.rs
// `SharedBinding`). 이 픽스처가 그것을 쓰지 않는 것은 여전히 사실이다. 그리고
// RQ4 자체가 2026-07-28 전면 폐기됐다(RQ6 H-Qwik 확정 — Island 경계는 이제
// 선택적 최적화, docs/04). 첫 항목(`ClickAction`에 `Decrement` 없음)은
// 2026-10-17 현재도 사실이다.
import { signal } from "loom";

function Widget() {
  const count = signal(0);
  return (
    <div>
      <button onClick={() => count.increment()}>+</button>
      <span>{count()}</span>
    </div>
  );
}

function ProductCard() {
  const details = signal(false);
  return (
    <div>
      <button onClick={() => details.toggle()}>show details</button>
      {details() && <p>Free shipping on orders over $50.</p>}
      <Widget client:load />
    </div>
  );
}

export function App() {
  return (
    <div>
      <ProductCard client:visible />
    </div>
  );
}
