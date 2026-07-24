// Phase 1 demo source (roadmap/PHASES.md end condition: "VDOM 없이 카운터 +
// 동적 리스트가 실제로 브라우저에서 갱신됨"). Authoring surface here is a
// deliberately narrow Phase 1 subset — see
// crates/loom-compiler/src/lower.rs's module doc comment for exactly what's
// supported. The final API surface is still explicitly undecided
// (docs/02 "미해결"), same as examples/counter-island.sample.tsx notes.
import { signal } from "loom";

export function App() {
  const count = signal(0);
  const items = signal(["a", "b", "c"]);
  const details = signal(false);
  const name = signal("");

  return (
    <div>
      <span class="label">count:</span>
      <span class="value">{count()}</span>
      <button onClick={() => count.increment()}>+1</button>
      <ul>
        {items().map((item) => (
          <li>{item}</li>
        ))}
      </ul>
      <button onClick={() => items.append("new")}>append</button>
      <button onClick={() => items.rotate()}>rotate</button>
      <button onClick={() => details.toggle()}>toggle details</button>
      {details() && <p class="details">shown!</p>}
      <input value={name()} onInput={(e) => name.set(e.target.value)} />
    </div>
  );
}
