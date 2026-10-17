// RQ1 §5 트랙 N이 요구하는 **3중 중첩**(리스트 > 조건부 > 리스트) 픽스처.
// §2.3-1이 "컴파일러가 생성할 수 없는 형태의 코드 금지"라, 이 구조는 반드시
// 컴파일러 산출물이어야 한다.
//
// 알려진 한계(숨기지 않음): 조건부의 술어와 안쪽 리스트의 내용이 **row마다
// 독립적이지 않다** — 컴포넌트 스코프 signal 하나가 모든 row로 흘러간다.
// row 스코프 파생 signal이 IR에 없기 때문이다. 구조(3중 중첩, row별 구간,
// 부모 entity별 파티션)는 그대로 성립하지만, 구간 크기가 row마다 같아서
// "구간 크기를 엉뚱한 데서 계산하는" 결함 중 균일 구간에서 상쇄되는 부류는
// 이 픽스처가 못 잡는다. roadmap 참고.
//
// **2026-10-17 정정**: "row 스코프 파생 signal이 IR에 없다"는 같은 날
// (2026-08-07, commit 5c37d1b) 해소됐다 — 닫힌 형태 두 개
// (`{open() && item.length > n && ...}`, `{xs().slice(0, item.length).map(...)}`,
// ir.rs의 RowPredicate/RowSlice)가 들어갔고, 그것을 쓰는 판본이
// demo/nested-list-derived.tsx다. 트랙 N(crates/loom-track-n)의 픽스처는 그
// 판본이다. 이 파일 자체의 한계(구간 균일)는 이 파일에 대해서는 여전히 사실이다.
import { signal } from "loom";

export function App() {
  const outer = signal(["a", "b", "c"]);
  const open = signal(false);
  const inner = signal(["x", "y"]);

  return (
    <div>
      <button onClick={() => outer.append("n")}>outer append</button>
      <button onClick={() => outer.rotate()}>outer rotate</button>
      <button onClick={() => open.toggle()}>toggle</button>
      <button onClick={() => inner.append("m")}>inner append</button>
      <ul class="outer">
        {outer().map((it) => (
          <li>
            {it}
            {open() && (
              <div class="panel">
                <ul class="inner">
                  {inner().map((x) => (
                    <li>{x}</li>
                  ))}
                </ul>
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
