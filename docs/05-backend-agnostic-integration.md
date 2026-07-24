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
- `resumability_payload`/`assets`는 이번 라운드에 구현 안 함 — 반환된 `html`이 이미
  모든 `data-loom-id`/`data-loom-lref`를 인라인으로 담고 있어 별도 payload 구조체가
  지금은 그걸 중복할 뿐임. Island가 진짜 별도 청크로 컴파일되는 빌드 툴링(Phase 3의
  "목표 상태", 아직 미착수)이 생기면 진짜 다중 asset 매니페스트가 필요해짐 — 그때
  다시 열 것.
- 이 문서는 아직 어느 것도 RQ로 등록 안 됨 — 이번 라운드에서 발견한 마찰(wasm-bindgen-futures
  무조건 의존, getrandom 백엔드 선택)은 전부 우회 가능한 빌드/링킹 디테일이었지 새
  RQ가 필요한 설계 수준 질문은 아니었음.
