// RQ6 §4.2 위젯-임베드 픽스처 — Task #26. §4.1의 "정체성 통제" 원칙대로, 이 한
// 소스가 GraphMode::Islands(Island 팔)와 GraphMode::Flatten(H-Qwik 반사실 팔)
// 양쪽으로 컴파일된다 — 별도로 손코딩된 두 구현이 아니다(crates/loom-widget-embed
// 참고).
//
// 5개 컴포넌트 — §4.2/§9-6/§6 M8의 사전등록된 요구를 그대로 충족:
//   - CartBadge: 별도 함수가 아니라 App 자신의 정적 마크업에 인라인된
//     {sharedValue(...)} 읽기 전용 구독 — 원본 손코딩 데모(demo/islands/index.html)
//     와 동일하게 "자기 청크가 없는" 구독자임을 그대로 유지.
//   - QuantityPicker (client:load): sharedSignal("cartCount", ...)의 유일한
//     writer — RQ4 cross-widget 시나리오의 실제 주체.
//   - ProductCard (client:visible): QuantityPicker를 중첩.
//   - CheckoutForm (client:load): §9-6 "고위험 위젯"(멀티필드 상태) 요구 —
//     <input> 없이(컴파일된 Island 진입점은 이번 라운드에 click만 지원, 위
//     generate_islands의 has_input_binding 가드 참고) Bool 시그널 3개로 구성.
//   - MiniDashboard (client:load): 카운터+리스트 조합 재사용(demo/counter-list.tsx
//     와 동형 패턴) — 시그널 이름은 GraphMode::Flatten의 "전역 유일성" 제약(이번
//     라운드의 명시된 스코프 컷) 때문에 고유하게 유지; "이름 충돌" 시나리오 자체는
//     크레이트가 아니라 loom-compiler의 별도 유닛 테스트
//     (graph_mode_flatten_rejects_colliding_signal_names_across_components)로 검증.
import { signal, sharedSignal } from "loom";

function QuantityPicker() {
  const cart_count = sharedSignal("cartCount", 0);
  return (
    <div>
      <button onClick={() => cart_count.increment()}>+</button>
      <span>{cart_count()}</span>
    </div>
  );
}

function ProductCard() {
  const details = signal(false);
  return (
    <div>
      <button onClick={() => details.toggle()}>show details</button>
      {details() && <p>Free shipping on orders over $50.</p>}
      <QuantityPicker client:load />
    </div>
  );
}

function CheckoutForm() {
  const shipping = signal(false);
  const gift = signal(false);
  const confirmed = signal(false);
  return (
    <div>
      <button onClick={() => shipping.toggle()}>toggle express shipping</button>
      {shipping() && <p>Express shipping selected.</p>}
      <button onClick={() => gift.toggle()}>toggle gift wrap</button>
      {gift() && <p>Gift wrap selected.</p>}
      <button onClick={() => confirmed.toggle()}>submit order</button>
      {confirmed() && <p>Order confirmed.</p>}
    </div>
  );
}

function MiniDashboard() {
  const visits = signal(0);
  const tags = signal(["news", "sale"]);
  return (
    <div>
      <button onClick={() => visits.increment()}>+1 visit</button>
      <span>{visits()}</span>
      <ul>{tags().map((tag) => <li>{tag}</li>)}</ul>
      <button onClick={() => tags.append("promo")}>add tag</button>
    </div>
  );
}

export function App() {
  return (
    <div>
      <span>cart: {sharedValue("cartCount", 0)}</span>
      <ProductCard client:visible />
      <CheckoutForm client:load />
      <MiniDashboard client:load />
    </div>
  );
}
