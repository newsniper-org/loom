// 조건부 안에 중첩 binding이 들어가는 최소 픽스처 (2026-08-07 신설).
//
// 왜 필요한가: RQ1 §5 트랙 N은 "리스트 > 조건부 > 리스트" 3중 중첩 위에서
// property test를 돌리라고 요구하는데, §2.3-1이 "컴파일러가 생성할 수 없는
// 형태의 코드 금지"라 손코딩 픽스처는 규약 위반이다. 그래서 컴파일러가 먼저
// 중첩을 낼 수 있어야 한다. 이 파일은 그 첫 단계인 **조건부 > {정적, 텍스트
// binding, 리스트}** 를 실제 코드젠으로 통과시키는 픽스처다.
//
// 아직 없는 것(숨기지 않고 적어 둠): 리스트 row 템플릿 안의 조건부
// (= 트랙 N이 요구하는 바깥쪽 중첩). 그것은 row 템플릿을 사이트별로
// monomorphize하는 별도 작업이다 — roadmap 참고.
//
// **2026-10-17 정정**: 위 "아직 없는 것"은 같은 날(2026-08-07) 해소됐다 — row
// 템플릿의 사이트별 monomorphize(commit 1071c47)로 row 안의 조건부(그 안에
// 중첩 리스트 하나)를 컴파일러가 낸다. 그 픽스처가 demo/nested-list.tsx와
// demo/nested-list-derived.tsx이고, 트랙 N(crates/loom-track-n)은 뒤의 것을
// 쓴다. 이 파일은 "조건부 > {정적, 텍스트 binding, 리스트}" 단계의 픽스처로
// 그대로 남는다(crates/loom-nested-fixture의 tests/nested.rs). 조건부 안의 또
// 다른 조건부, <input> 양방향 binding, 지연 노드의 자체 onClick, <Component/>
// 참조, sharedValue(...)는 2026-10-17 현재도 컴파일러가 거부한다(lower.rs의
// 조건부 arm, codegen.rs의 collect_lazy_slots_node/emit_lazy_subtree 패닉).
import { signal } from "loom";

export function App() {
  const open = signal(false);
  const count = signal(0);
  const items = signal(["a", "b", "c"]);

  return (
    <div>
      <button onClick={() => open.toggle()}>toggle</button>
      <button onClick={() => count.increment()}>+1</button>
      <button onClick={() => items.append("new")}>append</button>
      {open() && <section class="panel">
        <h3>details</h3>
        <p class="count">{count()}</p>
        <ul class="inner">{items().map((it) => <li>{it}</li>)}</ul>
      </section>}
    </div>
  );
}
