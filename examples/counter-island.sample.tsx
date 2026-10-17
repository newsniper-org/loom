// counter-island.sample.tsx
//
// !! 문법 목업입니다. 컴파일되지 않습니다. !!
// API 표면(함수 이름, import 경로, 데코레이터 형태)은 전부 확정 아님 — Phase 1에서
// 실제 파서/컴파일러를 만들며 실험적으로 결정. 여기서는 docs/에서 설명한 개념들이
// 저작자 입장에서 어떻게 보일지 감을 잡기 위한 목적만 있음.
//
// **2026-10-17 정정**: 이 목업은 RQ6 확정 이전(초기 설계 대화) 것이라 "Island이
// 항상 있다"는 전제 위에 서 있다. 2026-07-28 RQ6가 H-Qwik(강한 형태)으로
// 확정되면서 기본 모델은 페이지 전체가 하나의 World + 하나의 앱 청크 + 전역 위임
// 리스너 하나이고, Island 경계는 "지연 activation 경계 세분화"를 선택할 때만 쓰는
// 선택적 최적화가 됐다(docs/04, research-questions/RQ6-are-islands-necessary.md).
// 따라서 아래 `island()` 래퍼는 기본 저작 형태가 아니다. 실제로 컴파일되는 저작
// 표면은 이 파일과 다르다: 래퍼 함수가 아니라 컴포넌트 참조에 붙는
// `client:load`/`client:visible` 지시자이고(demo/widget-embed.tsx,
// demo/islands/product-card.tsx), 다중 컴포넌트 그래프를 기본 모델로 컴파일하는
// `GraphMode::Flatten`은 그 경계를 지우고 하나의 World로 합친다(경계를 살리는 쪽이
// `GraphMode::Islands`). 지원 범위의 정본은 crates/loom-compiler/src/lower.rs 모듈 문서다.
// 최종 API 표면이 미정이라는 위 서술은 2026-10-17 현재도 그대로다.

import { signal, island } from "loom"; // 가칭 API

// island()로 감싼 부분만 client 상호작용 대상 (Island 경계, docs/04)
// 두 번째 인자는 06에서 다룬 "언제 지연 activation을 트리거할지" 지시자
export const CounterIsland = island(
  function Counter(props: { start: number }) {
    // signal() 호출 -> 이 컴포넌트의 Entity에 Signal<number> Component 하나로 컴파일됨
    // (docs/02, docs/03). 클로저가 아니라 ECS storage에 값이 실제로 저장됨.
    const count = signal(props.start);

    return (
      <div>
        {/* 순수 정적 마크업 -- client에 어떤 코드도 내려가지 않음 */}
        <span class="label">현재 카운트:</span>

        {/* 동적 binding site. count가 Changed<Signal<number>>일 때만
            컴파일러가 생성한 patch system이 이 노드의 textContent만 직접 갱신.
            VDOM diff 없음 (docs/03). */}
        <span class="value">{count()}</span>

        {/*
          onClick은 클로저로 남지 않음. 컴파일러가 아래와 유사한 LRef로 변환:
            { chunk: "counter-island.chunk3.js",
              symbol: "increment",
              captures: [ComponentRef(count)] }
          SSR 시점에 이 정보는 button의 data-attribute에 직렬화되고,
          실제 클릭 전까지 이 chunk는 client에 로드되지 않음 (docs/04, LRef).
        */}
        <button onClick={() => count.set(count() + 1)}>+1</button>
      </div>
    );
  },
  { activate: "visible" } // Astro의 client:visible에 대응하는 지연 activation 지시자 (docs/06)
);

// --- 백엔드 임베딩 쪽 사용 예시 (의사코드, docs/05 FIC 참고) ---
//
// Rails 컨트롤러(의사코드):
//   result = LoomRenderer.render("CounterIsland", { start: 0 }, request_context)
//   render html: layout_html.sub("<!--WIDGET-->", result.html + result.resumability_script_tag)
//
// 여기서 LoomRenderer는 WASM-in-process 또는 sidecar HTTP 어느 경로든 될 수 있음 —
// Rails 쪽 코드는 어느 경로인지 몰라도 됨 (FIC가 그 차이를 흡수).
