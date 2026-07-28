# 05. 백엔드 애그노스틱 통합

**2026-07-28 (Phase 6, Task #29) — RQ6 §7 게이트가 H-Qwik을 확정(강한 형태)함에
따라 재해석**: 이 문서가 원래 전제했던 "레거시 백엔드에 위젯 하나만 꽂는다"는
시나리오의 실현 단위가 "Island"(별도 격리 World)에서 "전체 앱을 하나의 큰
embeddable 조각"으로 바뀐다 — 아래 "복수 마운트 지점" 절 참고. FIC 계약 자체
(`{html, resumability_payload, assets}`)와 세 임베딩 경로(WASM-in-process/
Sidecar/네이티브 바인딩)는 이 재해석과 무관하게 그대로 유효하다 — Island
경계 유무는 FIC가 감싸는 **내부** 구현 디테일이지 계약의 모양 자체를 바꾸지
않는다.

## 복수 마운트 지점 — "여러 Island" 대신 "하나의 World, 복수 anchor root"

원래 이 문서와 RQ6-are-islands-necessary.md가 예견했던 대로: **Island이
필요없다고 판명나면, 05의 backend-agnostic 요구는 "전체 앱을 하나의 큰
embeddable 조각으로" 방식으로 재해석해야 한다** — 그 예견이 실제로 §4-§5
실측 결과(§3.9)로 확인됐다.

구체적으로 이렇게 충족된다: 호스트 페이지에 위젯이 여러 군데(예: 헤더의
장바구니 배지, 본문의 상품 카드, 푸터의 뉴스레터 폼) 흩어져 있어도, 이들은
`crates/loom-widget-embed`의 `demo/widget-embed.tsx`가 이미 증명한 것처럼
**하나의 merged `bevy_ecs::World`**(하나의 `App_chunk`)에 속한 서로 다른
DOM anchor일 뿐이다 — SSR이 그 World 전체를 한 번에 렌더링해 여러 위치에
마크업을 흩뿌리고(`data-loom-id`/`data-loom-lref` 마커는 그대로 각 anchor에
붙음), client는 **어느 anchor에서 첫 상호작용이 일어나든** 하나의 `bootstrap.js`
전역 리스너가 그 하나의 청크를 materialize해 전체 World를 깨운다(M9 cross-widget
실측이 이미 확인: 한 anchor의 클릭이 다른 anchor의 DOM도 실시간 갱신).

이건 "Island처럼 여러 개의 독립 마운트 지점"이 아니라 **"하나의 마운트 지점(하나의
World)이 여러 개의 DOM 좌표에 투사된다"**는 모델이다 — §8이 이미 이 모델을
"H-Qwik 팔의 복수 anchor 메커니즘"으로 잠정 고정해 뒀던 것이, 이제 잠정이 아니라
확정된 기본 모델이 됐다.

## 요구조건 해석

"어떤 백엔드 프레임워크에든 잘 결합"은 "Node.js 런타임 위의 다른 JS 프레임워크와 잘 어울림"
정도가 아니라, Rails/Django/Spring/Actix 등 **언어가 다른 백엔드에서도** 이 프레임워크의
렌더링 결과물(HTML 조각 + resumability payload)을 받아 자기 템플릿에 꽂을 수 있어야 한다는
뜻으로 해석함.

## Framework Integration Contract (FIC)

SSR 렌더러를 **순수 함수**로 정의:

```
render(component_ref, props, request_context) -> {
  html: string,                // 초기 페인트용 정적/동적 HTML 조각
  resumability_payload: ...,   // LRef 목록(chunk/symbol/captures) — "Island별"이 아니라
                                // 이 렌더의 전체 World가 발행한 LRef 전부 (04 재해석 참고)
  assets: [url],                // client가 필요로 할 청크 목록 (전부 lazy, prefetch 힌트용)
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

## Phase 4 실제 구현 (2026-07-24)

`crates/loom-fic` — C ABI shim(`loom_fic_render`/`loom_fic_free`, `[len: u32 LE][utf8
html]` 와이어 포맷 — WASM-in-process 호스트는 콜리의 리니어 메모리만 읽을 수 있어
길이를 별도 out-포인터가 아니라 버퍼 자체에 실어야 함) 위에 세 경로 전부 실제 구현·검증:

1. **네이티브 바인딩 — 계획보다 더 얇게 감**: 원안은 "Node: napi-rs, Python: PyO3 등
   언어별 바인딩 생성"이었으나, 실제로는 **그조차 필요 없음**을 확인 — Python
   `ctypes`가 C ABI `.so`를 직접 로드해서 호출(`examples/native_binding.py`). "하나의
   C shim이 FIC 전체를 흡수한다"는 leo4 원칙을 문자 그대로 밀어붙이면 PyO3/napi-rs
   같은 언어별 래퍼조차 선택 사항이 됨 — 이 사실 자체가 이번 라운드의 실측 발견.
2. **Sidecar HTTP**: `std::net::TcpListener`만 쓴 최소 서버(`loom-fic-sidecar`
   바이너리, 프레임워크 없음, 원안 그대로) — Node.js가 `fetch()`로 호출.
3. **WASM-in-process**: `wasm32-unknown-unknown`(wasm-bindgen 없는 plain 빌드) +
   Python `wasmtime`. **실제로 발견한 마찰(숨기지 않음)**: `bevy_ecs`가
   `bevy_tasks`의 wasm32 비동기 실행기를 통해 `wasm-bindgen-futures`에 **무조건**
   의존함 — 이 렌더 경로는 그 코드를 전혀 안 쓰는데도 인스턴스화 시점에 4개의
   `__wbindgen_*` import(`__wbindgen_describe`, `__wbg___wbindgen_throw_*`,
   `__wbindgen_externref_table_grow`/`_set_null`)를 요구함. `wasm-tools print`로
   정확히 어떤 4개인지 확인한 뒤 `wasmtime`의 `Linker.define_func`로 no-op
   스텁을 제공해 해결(이 경로는 실제로 그 함수들을 호출 안 하므로 no-op이어도
   안전함을 확인). `getrandom`도 wasm32 기본값(`wasm_js` 백엔드, wasm-bindgen
   의존)을 요구해 "custom backend" 메커니즘(`--cfg getrandom_backend="custom"` +
   직접 구현한 `__getrandom_v03_custom`)으로 대체 — 이 렌더 경로가 결정론적
   단발성 SSR이라 진짜 난수가 필요 없어서 안전(주석에 왜 안전한지 명시, 프로덕션
   WASM 호스트라면 실제 난수 소스를 import해야 함).

**종료 조건 충족**: Python(ctypes 네이티브 바인딩 + `wasmtime` WASM-in-process 둘 다)
+ Node.js(Sidecar HTTP) — 서로 다른 두 언어에서 동일 컴포넌트(`demo/counter-list.tsx`,
다른 모든 데모와 같은 산출물)를 렌더링해 각자 독립적으로 만든 "레거시 호스트 페이지"
(loom이 생성 안 한 nav/footer 포함)에 실제로 임베드 성공, Playwright 스크린샷으로
검증 완료. 상세는 roadmap/PHASES.md Phase 4 절.

## 미해결

- ~~FIC의 request_context 스키마를 얼마나 최소화할 수 있는가~~ → **이번 스코프에서
  자명하게 해소됨**: Phase 1 IR이 시그널 초기값을 요청별로 다르게 받는 방법 자체가
  없으므로(리터럴 선언값 고정), `request_context`는 최소화할 대상이 아니라 **비어
  있음** — 최소화 판단이 아니라 아직 파라미터화할 게 없다는 사실의 반영. IR이 요청별
  동적 초기값을 지원하게 되면 이 항목을 다시 열 것(`crates/loom-fic`의 모듈 문서 참고).
- Sidecar 경로에서 콜드 스타트/커넥션 풀링 전략 — 여전히 미해결(이번 스코프는
  "구조적으로 가능한가"만 증명, 프로덕션 배포 전략은 범위 밖).
- ~~`resumability_payload`/`assets`는 이번 라운드에 구현 안 함~~ → **2026-07-28
  (Task #28-c) 구현됨**: `render_html_full()`/`loom_fic_render_full()`을 기존
  함수와 나란히 추가, JSON 와이어 포맷. `resumability_payload`는 `html`에 이미
  인라인된 `data-loom-lref`를 구조화해 추출한 것(정직한 실질 콘텐츠 — 문자열
  재파싱 불필요라는 실무적 가치). `assets`는 여전히 항상 빈 배열 — 이 크레이트
  (`crates/loom-fic`)엔 청크 빌드 툴링이 없고, RQ6이 Island 세분화를 "필수가
  아닌 선택적 최적화"로 재해석했으므로(04 참고) 이제 "다중 asset 매니페스트가
  필요해지는 시점"은 Phase 3 완료가 아니라 **누군가 실제로 세분화를 선택했을
  때**로 조건이 바뀌었다.
- 이 문서는 아직 어느 것도 RQ로 등록 안 됨 — 이번 라운드에서 발견한 마찰(wasm-bindgen-futures
  무조건 의존, getrandom 백엔드 선택)은 전부 우회 가능한 빌드/링킹 디테일이었지 새
  RQ가 필요한 설계 수준 질문은 아니었음.
