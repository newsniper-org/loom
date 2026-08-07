// RQ1 §5 트랙 N의 3중 중첩 — **row마다 구조가 달라지는** 판본 (2026-08-07).
//
// `demo/nested-list.tsx`(균일 판본)와 나란히 둔다. 균일 판본은 모든 row의
// 구간 크기가 같아서, 불변식 2("move가 다른 row/블록 노드를 찢지 않음")의
// 결함 중 **균일 구간에서 상쇄되는 부류**를 원리적으로 못 잡는다. 이 판본은
// 그 구멍을 메운다:
//
//   - `open() && it.length > 1` — 아이템 길이가 1인 row는 열리지 않는다.
//     즉 어떤 row는 구간이 1개 노드, 어떤 row는 여러 개.
//   - `inner().slice(0, it.length)` — 열린 row의 안쪽 리스트 길이가 자기
//     아이템 길이만큼이다. 그래서 열린 row들끼리도 구간 크기가 다르다.
//
// 그리고 `inner`는 여전히 독립적으로 편집 가능하므로, §5가 요구하는
// "외부 리스트 ops + 조건부 토글 + 내부 리스트 ops의 무작위 인터리브"가
// 그대로 성립한다(파생이 조작을 잡아먹지 않는다).
import { signal } from "loom";

export function App() {
  const outer = signal(["a", "bb", "ccc"]);
  const open = signal(false);
  const inner = signal(["p", "q", "r", "s"]);

  return (
    <div>
      <button onClick={() => outer.append("dddd")}>outer append</button>
      <button onClick={() => outer.rotate()}>outer rotate</button>
      <button onClick={() => open.toggle()}>toggle</button>
      <button onClick={() => inner.append("t")}>inner append</button>
      <ul class="outer">
        {outer().map((it) => (
          <li>
            {it}
            {open() && it.length > 1 && (
              <div class="panel">
                <ul class="inner">
                  {inner()
                    .slice(0, it.length)
                    .map((x) => (
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
