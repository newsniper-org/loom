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
