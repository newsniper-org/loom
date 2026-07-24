# 05. 백엔드 애그노스틱 통합

## 요구조건 해석

"어떤 백엔드 프레임워크에든 잘 결합"은 "Node.js 런타임 위의 다른 JS 프레임워크와 잘 어울림"
정도가 아니라, Rails/Django/Spring/Actix 등 **언어가 다른 백엔드에서도** 이 프레임워크의
렌더링 결과물(HTML 조각 + resumability payload)을 받아 자기 템플릿에 꽂을 수 있어야 한다는
뜻으로 해석함.

## Framework Integration Contract (FIC)

SSR 렌더러를 **순수 함수**로 정의:

```
render(component_ref, props, request_context) -> {
  html: string,              // 초기 페인트용 정적/동적 HTML 조각
  resumability_payload: ...,  // Island별 직렬화된 IslandWorld + LRef 목록 (04 참고)
  assets: [url],              // client가 필요로 할 청크 목록 (전부 lazy, prefetch 힌트용)
}
```

이 계약만 만족하면 백엔드가 무엇이든 상관없음 — 백엔드는 이 결과물을 자기 템플릿 엔진의
문자열 삽입 지점에 넣기만 하면 됨. 이건 사실 Astro의 SSR adapter 개념을 JS 런타임 경계
바깥까지 확장한 것.

## 세 가지 임베딩 경로

1. **WASM-in-process**: Rust 코어를 WASM으로 컴파일, WASI 지원 런타임(wasmtime 등)이 있는
   어떤 언어에서든 인프로세스로 호출. 네트워크 홉 없음, 가장 저지연. leo4에서 이미 다루고 있는
   FFI/ABI 설계 경험과 직접 겹치는 지점.
2. **Sidecar HTTP 마이크로서비스**: 렌더러를 독립 프로세스로 띄우고 backend는 그냥 HTTP POST.
   가장 단순하고 정말로 언어 무관하지만 네트워크 홉 비용 발생. 통합 난이도 최저, 성능 최저.
3. **네이티브 바인딩**: 같은 Rust 코어에서 언어별 바인딩 생성 (Node: napi-rs, Python: PyO3,
   Ruby: magnus 등). 성능은 WASM보다 나을 수 있으나 언어별로 별도 빌드/배포 필요.

## 설계 원칙: leo4 패턴 재사용

leo4에서 이미 검증한 구조 — "C shim이 `lean.h`의 모든 관심사를 흡수하고, 그 위에 각 언어
바인딩을 얇게 얹는다" — 를 그대로 가져옴. 여기서는 **하나의 C ABI shim이 FIC 전체를 흡수**하고,
WASM/sidecar/네이티브 바인딩 셋 다 이 shim을 통해서만 코어에 접근. N개 언어에 대해 N개의
독립적 통합을 따로 설계하지 않음.

## 미해결

- FIC의 request_context 스키마를 얼마나 최소화할 수 있는가 (백엔드마다 요청 컨텍스트 모양이
  다름 — 라우팅 파라미터, 세션 정보 등)
- Sidecar 경로에서 콜드 스타트/커넥션 풀링 전략
- 이 문서는 아직 어느 것도 RQ로 등록 안 됨 — Phase 4에서 구체화되면서 RQ 추가될 가능성 높음
