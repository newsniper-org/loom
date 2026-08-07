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
