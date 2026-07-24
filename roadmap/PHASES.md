# 로드맵

각 Phase는 이전 Phase의 "종료 조건"을 만족해야 시작. 건너뛰지 말 것 (CLAUDE.md).

## Phase 0 — 리서치 수렴 (지금 여기)

**목표**: 코드를 쓰기 전에 최소한의 잠정 답을 확보.
- RQ1(동적 제어 흐름): ~~후보 A/B 중 하나 잠정 선택~~ → **판별 프로토콜 사전 등록으로
  대체·이행됨(2026-07-23)**. 초판 문구는 후보가 A/B 2안이던 시점 기준 — 후보가 A~F로
  확장되었고(RQ1, 2026-07-22), 병익님 지시로 산출물을 무가중 공평 프로토콜로 재정의.
  `research-questions/RQ1-benchmark-protocol.md` 참고
- RQ3(Island 중첩): 최소 규칙 하나 잠정 확정 (완벽하지 않아도 됨, Phase 3까지 반복 개선)
  → **이행됨(2026-07-23)**: supervision forest + 독립 resume + DOM 물리성 결속 2단계
  detach + promote-once/no-restart. RQ3 문서의 잠정 규칙과 재검토 트리거 참고
- RQ6(Islands 필요성): 최소한 "검증 계획"이라도 확정 — Phase 1/2를 Island 없이 먼저
  만들어서 판단 재료를 얻는 것 자체가 Phase 0의 산출물이 될 수 있음 → **이행됨
  (2026-07-23)**: `research-questions/RQ6-verification-plan.md` (사전 등록 검증
  계획, v1.1 — RQ1과 동형의 3인 교차 감사 완료). 판정 자체는 Phase 2/3/6 세
  시점에 걸쳐 순차 확정 — Phase 2 종료
  항목(아래), Phase 3 종료 항목, 본 파일 Phase 6 항목 참고.

**종료 조건**: 위 3개에 대한 잠정 답이 문서화됨(완전한 답이 아니어도 "이렇게 가정하고
진행한다"는 명시적 결정이면 충분).

## Phase 1 — 컴파일러 + No-VDOM 반응성 프로토타입

**범위**: Island/Resumability/backend-agnostic 전부 제외. 단일 앱, 단일 IslandWorld,
client=server 동일 프로세스(Node나 WASM 테스트 하네스) 가정.
- JSX → Entity graph + PatchTable 컴파일 (02)
- `Changed<T>` 게이팅 patch system 동작 확인 (03)
- RQ1 벤치마크 프로토콜(사전 등록 v1.1)의 공통 기반(§2) 구축 + 트랙 L/T 핵심 격자 실측
  (2026-07-23 갱신 — 초판 "RQ1 잠정안으로 동적 리스트 하나" 문구를 프로토콜 체제로 대체)

**종료 조건**: VDOM 없이 카운터 + 동적 리스트가 실제로 브라우저에서 갱신됨.

**착수 상태 (2026-07-23) — 종료 조건 충족, 단 스코프를 아래처럼 명시적으로 축소함:**

- **된 것**: `crates/loom-runtime`(Signal<T>/DomTarget/ListRows Component,
  `Changed<T>` 게이팅 patch System, RQ1 candidate K 리스트 reconcile, 영속
  Bevy `Schedule` 기반 tick 드라이버) + `crates/loom-compiler`(SWC
  `swc_ecma_parser`로 실제 TSX 파싱 → IR → Rust codegen, "loomc") +
  `crates/loom-demo`(wasm-bindgen + web_sys 브라우저 DOM 바인딩). Playwright로
  headless Chromium에서 실제 카운터 증가(0→3)와 리스트 append/rotate가
  DOM에 정확히 반영됨을 확인함(스크린샷 포함) — 종료 조건 충족.
- **의도적으로 축소한 것 (CLAUDE.md 정직성 요구 — 숨기지 않음)**:
  1. **PatchTable(opcode+dispatch table)를 구현하지 않고 Rust 제네릭 System으로
     대체함.** docs/02의 opcode 설계는 Phase 2의 LRef 직렬화 경계를 위한 것이고
     Phase 1은 그 경계가 없음(client=server 동일 프로세스) — 제네릭
     `patch_text_system::<T>`가 컴파일타임 모노모picture화라는 동일한 "정적
     디스패치, Component엔 클로저 없음" 속성을 만족함. Phase 2에서 opcode
     방식으로 전환 필요(loom-runtime/src/lib.rs 모듈 문서 참고).
  2. **RQ1 벤치마크 프로토콜의 사전 등록 본실행(row 깊이 축·원본 앱 규모 축·
     WebKit/CDP 스로틀링·트랙 N/R·§2.4 형식 최선형 체크리스트 전수 대조)은
     여전히 실행하지 않음.** 단 **트랙 L(리스트) 후보 C/D/E는 2026-07-23 전부
     구현·네이티브 검증(22개 테스트)·N-스케일링(n∈{100,1000,10000})·POD/heap
     아이템축(부분)·Chromium+Firefox 2브라우저 실측을 마쳤다** —
     `crates/loom-runtime/src/list.rs`, `crates/loom-bench`, 원시 데이터
     `research-questions/data/`, 상세는 RQ1 문서 "Phase 1 트랙 L 진행 상황"
     참고. **이 실측 도중 `place_in_order`가 우발적으로 O(n²)이던 실제 성능
     버그를 발견해 이중 연결 리스트 기반 O(1) 구현으로 수정·재검증·재측정했고**,
     수정 후에도 LIS 미적용으로 인해 swap/shuffle10류 전역 재배치 워크로드에서
     D/E가 K보다 뚜렷이 느리다는 것을 확인함 — RQ1이 Phase 0에서 이미 예측했던
     "LIS 없이는 O(이동 수) 주장 불성립"이 실측으로 검증된 사례. **이어서
     2026-07-24, LIS 기반 최소 이동 변형 D-LIS/E-LIS를 구현·네이티브 검증(18개
     테스트 신규)·재측정함** — swap(국소 재배치)은 median 점추정으로는 K와
     가까워졌으나(Chromium 0.99x-1.04x K), shuffle10(광역 재배치, 실제로는
     `v.len()/10`회 무작위 재배치)은 좁아지되 닫히지 않음(2.12x→1.56x-1.69x K)
     — 원인은 shuffle10이 swap과 달리 진짜로 이동해야 하는 개수 자체가 n에
     비례해 크기 때문. **같은 날 §8 paired bootstrap 통계(10^4 리샘플, 99% CI,
     우세/열세/등가/비결정 어휘)를 기존 데이터에 적용해 재검증한 결과, swap의
     "K와 근접"은 점추정만으로는 그렇게 보였을 뿐 통계적으로는 대부분 비결정
     (CI가 1을 걸침)이고 Firefox E-LIS는 오히려 열세로 판정됨** —
     `research-questions/analysis/rq1-paired-bootstrap.py`. 상세는 RQ1 문서
     "3차 실측"·"§8 통계 절차 적용" 절, 원시 데이터 `research-questions/data/
     rq1-phase1-track-l-core-grid-v5-*.json`. **트랙 T(조건부) 후보 B/F도
     구현·네이티브 검증(10개 테스트)·브라우저 실측을 마쳤다** —
     `crates/loom-runtime/src/cond.rs`. 상태 보존 여부(B=손실, F=보존)는 정합성
     테스트로 확실히 검증했으나, 최초 실측은 자식 1개짜리 최소 서브트리라 타이머
     해상도 아래에 묻혀 성능 축 판별 불가였음. **2026-07-24, 서브트리 크기 축을
     1/10/50/500으로 확장**(`CondBlock`을 `child: Option<Entity>`에서
     `{size, children: Vec<Entity>}`로 일반화하고, F가 개별 자식이 아니라 공유
     anchor 하나에만 `set_visible`을 호출하도록 변경 — 신규 테스트 3개 포함)한
     결과 size=500에서 마침내 결정적 수치를 확보: B의 toggle 비용은 노이즈
     바닥 위로 뚜렷이 올라온 반면(Chromium median 0.5ms, Firefox median 1.0ms,
     거의 매회 0 초과) F는 같은 크기에서도 여전히 사실상 0(0-3% 표본만 0 초과)
     — **F의 토글 비용이 서브트리 크기와 무관하게 O(1)이라는 RQ1의 핵심 주장이
     실측으로 확정됨**. 상세는 RQ1 문서 "트랙 T 재실측" 절, 원시 데이터
     `research-questions/data/rq1-phase1-track-t-toggle-v2-*.json`.

**row 깊이 축 구현 + RQ1-benchmark-protocol.md §9 실제 적용 (2026-07-24)**:
row 깊이(R1/R2) 축을 실제로 구현·측정(`RowBindings` Resource,
`crates/loom-bench`에 W2(전체 shuffle) 워크로드 신설)하고, §9의 채택/기각
조건을 **기계적으로 적용**함. 결과: **D/E가 "조건부 채택(FV 계류)" 상태로
격상됨** — R2(n=1,000)에서 D·E가 K를 paired bootstrap 99% CI로 확실하게
이기는 셀이 다수 확인돼(swap/shuffle10 양쪽 브라우저) D의 §9 채택 양성
조건이 충족되고 기각 조건 (iii)도 미발동, 다만 §5 FV 계류(flush 규약 v0의
공식 검증, Phase 5 몫)라는 선결 조건이 아직 안 끝나 "조건부"에 머무름. **C는
반대로 이전 세션의 "채택 조건 충족" 서술을 이번에 정정함** — W2를 실제로
추가해 정확한 4셀을 갖추고 재확인한 결과 head_insert에서 C가 D에 열세임이
드러나(paired bootstrap 확정) §9의 "전 셀 동등 이상" 조건이 깨져, C의 상태는
"채택"이 아니라 **판정 비결정**이 정직한 서술. 상세는 RQ1 문서의 "row 깊이
축 구현 + RQ1 §9 실제 적용" 절, 원시 데이터 `research-questions/data/
rq1-phase1-track-l-core-grid-v6-*.json`, 분석 스크립트
`research-questions/analysis/rq1-row-depth-bootstrap.py`. **B/F는 여전히
판정 불가** — §9가 요구하는 초당 토글 빈도(f) 축 자체가 하네스에 없음(이번
라운드도 스코프 밖으로 명시).

RQ1 자체의 잠정 선택은 이제 완전 미착수는 아니다 — D/E는 조건부 채택,
C/B/F는 판정 비결정(또는 판정 불가)이라는, 이전보다 훨씬 구체화된 상태다.
  3. **저작 표면(TSX)은 examples/counter-island.sample.tsx보다 훨씬 좁은
     부분집합.** `signal(리터럴)`, `{signal()}`, `{signal().map((x) => <tag>{x}</tag>)}`,
     `onClick={() => signal.increment()/.append("literal")/.rotate()}`만
     지원 — 일반 JS 표현식 트랜스파일이 아니라 "정적/동적 분류 + 좁은 patch
     생성"이라는 핵심 메커니즘을 증명하는 프로토타입(`crates/loom-compiler/src/lower.rs`
     모듈 문서에 지원 범위 명시). API 표면은 여전히 미확정(docs/02).
  4. Island/Resumability/backend-agnostic 제외는 원래 스코프대로 유지.

## Phase 2 — Resumability 레이어

- LRef 직렬화/역직렬화 (RQ2 잠정 포맷으로)
- 전역 delegation 리스너 + lazy chunk 로딩
- Component 스냅샷 → 핸들러 실행 → 최소 상태만 materialize되는지 확인

**종료 조건**: SSR 후 client JS 0 실행 상태에서, 상호작용 시점에만 코드가 로드되고 정확히
동작함이 계측으로 확인됨 (번들 실행 시점 로깅 등).

**여기서 RQ6 재검토**: Island 없이 이 정도로 충분한지 벤치마크.

**착수 상태 (2026-07-23) — 카운터 단일 바인딩 사이트 하나로 종료 조건 충족, 범위는
명시적으로 축소함:**

- **된 것**: `crates/loom-resume-demo` — `src/bin/ssr.rs`(네이티브, SSR 역할 —
  Phase 1의 `StringDom: DomOps`로 정적 HTML + `data-loom-id`/`data-loom-lref`
  태깅 생성) + `demo/resume/bootstrap.js`(전역 클릭 위임 리스너, Qwik 메커니즘
  그대로 재사용 — CLAUDE.md 2-1) + wasm "청크"(`ResumedCounter`, SSR로 이미 만든
  DOM 노드에 직접 붙어 patch만 하는 `ResumeDom`). Playwright로 **네트워크 요청
  자체를 감시**해 실제로 확인: (1) 페이지 로드 시 청크(.js/.wasm) 요청 0건,
  (2) 첫 클릭 시에만 청크 요청 발생 + 값이 정확히 0→1, (3) 두 번째 클릭은
  청크 재요청 없이(=RQ3 "promote-once-and-stay") 같은 인스턴스 재사용으로 1→2.
  이게 종료 조건이 요구하는 "번들 실행 시점 로깅"의 실측.
- **RQ2 포맷 잠정 결정**: 텍스트 기반 `chunk|symbol|key:value,...`(JSON/바이너리
  아님) — 파싱에 라이브러리가 필요 없을 만큼 단순한 것을 우선한 잠정 선택.
  RQ2 자체는 여전히 미해결(포맷 트레이드오프 문서 참고).
- **의도적으로 축소한 것 (CLAUDE.md 정직성 요구)**:
  1. **loom-compiler를 거치지 않음** — 이 프로토타입은 카운터 하나를 손으로
     작성한 것이지, "컴파일된 모든 액션이 자동으로 LRef를 얻는" 일반 파이프라인이
     아님. Track L/T가 컴파일러 통합 전에 런타임 레벨부터 시작한 것과 동일한
     순서.
  2. **캡처 대상이 스칼라 `Signal<i32>` 하나뿐** — 리스트/조건부 바인딩의 재개는
     `ResumeDom`이 아직 지원하지 않음(다른 모든 `DomOps` 메서드는 `unimplemented!`).
  3. Island 관련 내용(IslandRegistry, 2단계 detach 등, RQ3) 전부 미적용 — Phase 2는
     Island 이전 단계.
  4. LRef 캡처 최소집합의 정적 계산(over/under-capture 문제, docs/02 미해결 항목)은
     다루지 않음 — 이 데모는 캡처할 대상이 정확히 1개뿐이라 문제 자체가 발생하지 않음.
- **RQ6 재검토는 아직 하지 않음** — 이 시점 벤치마크(Island 없는 이 프로토타입의
  번들 크기/TTI)는 후속 작업.

**컴파일러 통합 (2026-07-24) — 위 "loom-compiler를 거치지 않음" 축소를 실제로 해소:**

- **된 것**: `crates/loom-compiler/src/codegen.rs`가 이제 `data-loom-id`/
  `data-loom-lref` 태깅과 `resume`/`resume_action_by_symbol` 함수 자체를
  생성함 — `crates/loom-resume-demo`는 카운터를 손코딩하는 대신
  `demo/counter-list.tsx`(loom-demo의 Phase 1 라이브 경로가 쓰는 것과 **동일한
  소스**)를 컴파일해 SSR(`ssr_render`)과 재개(`ResumedComponent`) 양쪽에 씀 —
  "손으로 고른 카운터 하나"가 아니라 "컴파일러의 평범한 출력이 별도 노력 없이
  재개 가능한가"를 실제로 검증한 것. LRef 페이로드는 컴파일타임에 알려진
  선언 초기값으로 코드젠 시점에 문자열로 굳어짐(런타임 포맷 불필요) — captures는
  컴포넌트 전체의 `i32` 시그널(안전한 과대 캡처, 최소 집합 계산은 별도 항목).
  `set_attribute`를 `DomOps` trait에 정식으로 추가해 `BrowserDom`/`StringDom`/
  테스트용 `MockDom` 전부 구현하도록 함(이전엔 `StringDom`에만 있던 inherent
  메서드). Playwright로 재검증: SSR 직후 청크/wasm 요청 0건, 첫 클릭에서만
  로드+정확히 0→1, 두 번째 클릭은 캐시된 인스턴스 재사용(1→2, 재요청 없음) —
  Phase 1 라이브 경로에서 이미 검증된 카운터가 컴파일러 통합 후에도 동일하게
  동작함.
- **실행 도중 실제 버그를 발견함**: `StringDom::append_static_text`가 실제
  텍스트 노드 대신 `<#text-wrapper>...</#text-wrapper>`라는 **잘못된 HTML
  태그**를 방출하고 있었음 — 손으로 쓴 카운터 단일 프로토타입은 이 경로를
  전혀 타지 않아서(라벨을 `set_text_content`로만 썼음) 숨어 있던 결함. 컴파일된
  코드는 정적 텍스트마다("count:", "+1", "append", "rotate") `append_static_text`를
  호출하므로 이번에 처음 실측으로 드러남 — 조용히 넘기지 않고 `TEXT_NODE_TAG`
  센티널을 `render_node`가 특별 취급하도록 고쳐 진짜 텍스트 노드처럼(래핑 태그
  없이) 렌더링하게 수정하고 재검증함.
- **의도적으로 축소한 것 (당시 기준)**:
  1. 캡처/재개 대상이 `i32` 시그널(Increment 액션)뿐이었음 — `items`
     (`StringList`, Append/Rotate)는 자리표시자만. **→ 아래 리스트 재개 항목에서
     해소함.**
  2. LRef 캡처는 여전히 최소 집합이 아니라 컴포넌트 전체 과대 캡처.
  3. Island 관련 내용은 여전히 전부 미적용.
- **RQ6 재검토는 여전히 하지 않음.**

**리스트/조건부 바인딩의 재개 지원 (2026-07-24, 컴파일러 통합 바로 다음 라운드):**

- **된 것 — 리스트(컴파일러 통합)**: `codegen.rs`의 `resume()`이 이제
  `StringList` 시그널도 진짜로 재개함. 핵심 설계: row **값**은 LRef payload에
  싣지 않고 **이미 올바른 내용을 보여주고 있는 SSR'd DOM 자체에서 읽어온다** —
  새 `DomOps::adopt_children(parent) -> Vec<(NodeHandle, String)>`가 기존 자식
  각각을 추적 대상으로 등록하며 현재 텍스트를 반환. 이러면 리스트 길이와
  무관하게 LRef payload가 O(1)로 유지됨(리스트 전체를 문자열로 직렬화할 필요
  없음). `resume()`은 이 값들로 row entity들과 리스트 자신의
  `Signal<Vec<String>>`을 함께 복원하고 `list_reconcile_k_system::<String>`을
  등록함. Append/Rotate 액션도 이제 LRef를 받음(`resume_action_by_symbol`은
  `Append`의 리터럴 인자도 코드젠 시점에 굽는다 — 런타임 인자 전달 자체가
  불필요해짐). Playwright로 재검증: append 후 a,b,c,new / rotate 후
  b,c,new,a / 이어서 +1도 정상 동작하며 리스트를 건드리지 않음.
- **실행 도중 실제 버그 2건을 더 발견함**:
  1. **Bevy의 "새로 스폰된 컴포넌트는 첫 관찰에서 항상 Changed로 보임" 특성이
     리스트 재개에서 실제 문제였음** — 등록만 해두고 안 건드리면, 무관한
     액션(예: +1) 하나가 훗날 우연히 처음 tick을 트리거하는 순간
     `list_reconcile_k_system`이 방금 어렵게 adopt한 행들을 전부 despawn+
     respawn해버림. 해결: `resume()`이 끝에서 **정착(settle) tick**을
     한 번 내부적으로 수행 — 이 비용은 "상호작용 전 JS 실행 0"이라는 핵심
     주장을 깨지 않지만(첫 상호작용 시점의 청크 materialize 비용 안에
     들어감), 리스트 행이 그 순간엔 실제로 다시 만들어진다는(재사용이 아님)
     진짜 비용이며 숨기지 않고 문서화함(`codegen.rs` 모듈 문서).
  2. **리스트 자신의 `Signal<Vec<String>>`을 빈 벡터로 복원하는 버그** —
     row entity들은 adopt해놓고 리스트의 값 자체는 `Vec::new()`로 스폰해서,
     첫 append가 `[new]`가 되어 a/b/c를 통째로 잃는 결과가 실제로 재현됨
     (Playwright로 직접 확인). `adopt_children`이 돌려준 텍스트들을 리스트
     시그널에도 반영하도록 수정.
- **된 것 — 조건부(런타임 레벨 증명만, 컴파일러 미통합)**: `lower.rs`가
  `{cond && <X/>}` 저작 표면을 아직 지원하지 않으므로(컴파일러에 연결할
  대상 자체가 없음), RQ1 Track T의 B/F가 컴파일러 통합 전에 런타임에서
  먼저 증명됐던 것과 같은 방식으로 `crates/loom-resume-demo::cond_resume`에
  **네이티브 전용** 증명을 추가함 — F(CSS-hide)로 SSR 렌더된 자식의 내부
  카운터가 완전히 새로운 `LoomApp`/`World`(SSR 쪽 World와 공유 상태 없음)로
  넘어간 뒤에도 살아남고, 이어서 off→on 토글에도 보존됨을 네이티브 테스트
  2개로 확인. **브라우저/wasm-pack 청크로는 검증하지 않음** — 컴파일러 저작
  표면이 없어 실제 배포 형태를 만들 수 없기 때문(스칼라/리스트와의 명확한
  차이, 숨기지 않음).
- **여전히 안 한 것**: 조건부의 컴파일러 통합 자체(TSX 저작 표면 신설이
  선행돼야 함 — 별도 스코프), Island 관련 전부, RQ6 재검토.

**LRef 최소 캡처 집합 정적 계산 (2026-07-24, 바로 다음 라운드) — docs/02 미해결
항목 해소:**

- **된 것**: 이전엔 모든 액션의 LRef가 컴포넌트 전체의 `i32` 시그널을 과대
  캡처했음(예: Append/Rotate도 무관한 `count:0`을 실었음). 현재 IR에서는 각
  `ClickAction` variant가 시그널 정확히 하나만 참조하고 `apply_action`도 그
  하나만 건드리므로, 최소 캡처 집합을 **정확히** 계산 가능함을 확인하고 구현함
  (`codegen.rs`의 `format_i32_capture_for_action`) — Increment는 자신의 `i32`
  시그널만, Append/Rotate는 아예 캡처 없음(리스트는 DOM에서 복구하므로).
  새 테스트로 고정: `Action1`/`Action2`의 LRef가 `count:0`을 안 싣고 완전히
  빈 캡처(`"App_chunk|Action1|"`)를 갖는지 직접 단언. Playwright로 스칼라/리스트
  재개 둘 다 여전히 정상 동작함을 재확인.
- **중요한 스코프 한계(숨기지 않음)**: 이 최소성은 두 조건에 의존한다 — (1)
  액션 하나가 시그널 하나만 참조하는 현재 IR의 제약, (2) 캡처에서 빠진
  시그널은 `resume()`에서 리터럴 선언 초기값으로 폴백하는데, 이게 안전한 건
  Phase 1 IR이 시그널의 실제 첫 렌더 값이 선언값과 달라지는 경우를 지원 안
  하기 때문. `bootstrap.js`가 **엘리먼트별로** 독립적인 재개 인스턴스를
  캐싱하므로(컴포넌트별 공유 아님) 오늘은 문제없지만, IR이 요청별 동적
  초기값을 지원하게 되면 이 폴백 안전성부터 재검토해야 함 — docs/02·
  `codegen.rs` 양쪽에 이 caveat을 명시함.

**RQ2 본체 착수 (2026-07-24, 같은 날 마지막 라운드) — 5개 하위 질문 중 진행
가능한 만큼 실측·설계:**

- **하위 질문 2(버저닝/stale chunk) — 부분 해소**: `bootstrap.js`가 청크
  materialize 단계(동적 import/wasm init/생성자) 어디서 실패해도 명확한
  진단 로그 후 전체 페이지 리로드로 폴백하도록 수정 — Qwik/Next와 동일한
  업계 표준 대응(RQ2가 이미 지목한 "재실행 없음과 정면충돌"이라는 트레이드오프
  그대로 수용). 존재하지 않는 청크를 참조하는 버튼으로 Playwright 실제
  재현·검증(임시 픽스처, 실제 데모 파일은 미변경). "조용한 실패 → 있어보이는
  실패"로 바뀐 것이지 진짜 graceful 복구는 아님 — 청크 버전 해시, 페이로드
  스키마 버전 필드 등은 미착수.
- **하위 질문 4(Entity id remap) — 실측으로 재산정**: 현재 스코프(스칼라 +
  keyless K 리스트)에서는 remap이 **전혀 불필요**함을 확인 — 스칼라는
  이름 기반 조회(`find_node`), K는 애초에 keyless라 SSR 쪽 Entity id 자체가
  페이로드에 등장하지 않음. 단 이건 조건부 결론이다 — RQ1의 keyed 후보 D
  (`RowKey`)/E(`OrderKey`)를 resume 대상으로 쓰게 되면 그 키들이 참조하는
  entity를 안정적 식별자로 바꿔 싣고 resume 쪽에서 새 Entity id로 되짚는
  진짜 remap 인프라가 필요해짐 — 설계는 RQ2 문서에 기술, 구현은 D/E가
  resume 후보로 채택된 이후로 미룸.
- **하위 질문 3(압축)**: 현재 페이로드가 너무 작아(스칼라 1쌍 또는 빈 문자열)
  압축을 고민할 대상 자체가 없음 — 판단을 의도적으로 유보(성급한 최적화 방지).
- **하위 질문 5(dormant island 생존)**: Phase 3 인프라 부재로 여전히 미착수.

**"여기서 RQ6 재검토" 실행 (2026-07-24, Phase 2 마지막 라운드) — RQ6-verification-
plan.md §3.1 "1차 재검토" 그대로:**

- **스코프, §3.1 문언대로**: 이 시점엔 Island가 없으므로(Phase 3 산출물) 두
  가설 대결이 아니라 **H-Qwik 팔 단독 기준선 확정 + 데이터 등록**만 실행 —
  Island 팔·위젯-임베드·N-스케일링·원본 앱 규모·호스트 간섭·장애 주입은
  전부 Phase 3(§3.2) 몫으로 정직하게 미룸(§7 정식 판정도 Phase 6 몫).
- **된 것**: §4.1 "전체-앱" 픽스처의 H-Qwik 팔 — 카운터+리스트(컴파일러 생성,
  RQ1 산출물 재사용) + 중첩 조건부 토글(RQ1 §2.3 "코드젠 동형 손코딩" 규약
  준수, `cond_reconcile_f_system` 재사용) — `crates/loom-resume-demo`의
  `ssr_render_full_app`/`ResumedCond`. **폼 입력은 제외**(loom-runtime에
  양방향 폼 바인딩 자체가 없음 — 이 저장소 전체의 현재 능력 한계, 이 픽스처만의
  축소가 아님). Playwright로 전체(카운터+리스트+조건부) 실제 브라우저 검증.
- **M1(번들 크기) 실측**: 즉시 로드분(`bootstrap.js`) 3,241B raw/1,662B
  gzip — §4.3 제로-프레임워크 기준선(2,021B/966B) 대비 1.6-1.7배로,
  resumability의 "상호작용 전 실행 거의 0" 주장이 크기 축에서도 대체로
  유지됨을 보여주는 조기 신호. 지연 청크(카운터+리스트+조건부 wasm+js)는
  382,504B raw/136,542B gzip — 기준선 대비 **약 190배/141배**, wasm/Bevy
  ECS 런타임 고정비가 지배적이라는 해석(단 이 픽스처 한 크기에서만 유효,
  §4.4 N-스케일링 없이는 고정비/비례분 분해 불가 — 다음 우선순위). 부수
  데이터: 기능 추가에 따른 raw wasm 증분 곡선(카운터만 304,001B →
  +리스트 345,473B → +조건부 361,057B) — 기능당 증분이 줄어드는 추세.
- **M2 대신 콜드/웜 클릭 지연시간**: 이 픽스처엔 §4.2식 "여러 위젯"이 없어
  M2(위젯-로컬 TTI)는 §6 자기 예측대로 사실상 0에 수렴 — 대신 콜드(첫 클릭,
  청크 materialize 포함) vs 웜(캐시 인스턴스) 클릭 지연시간을 실측: Chromium
  localhost 15회 반복, 콜드 median 73.9ms vs 웜 median 37.0ms(차이 ≈37ms가
  순수 materialize 비용 추정치). **한계**: 로컬호스트라 실사용 환경의
  지배적 비용(132KB gzip 청크의 실제 네트워크 fetch 시간)이 전혀 반영 안
  됨 — RQ1-benchmark-protocol.md의 CDP 스로틀링 관행을 이 축에도 적용하는
  게 다음 우선순위. 원시 데이터: `research-questions/data/
  rq6-phase2-baseline-chromium.json`.
- 상세는 RQ6-verification-plan.md "§3.1 실행" 절 참고.

**조건부(`{cond() && <X/>}`)의 컴파일러 통합 (2026-07-24, RQ1 후보 확정과 같은
세션) — 위 "여전히 안 한 것" 항목 해소:**

- **된 것**: `ir.rs`에 `SignalInit::Bool`/`ClickAction::Toggle`/`JsxNode::
  CondBinding{cond, then_branch}` 추가, `lower.rs`가 `Expr::Bin(LogicalAnd)`
  패턴(`.map()` 탐지의 자매 패턴)으로 `{cond() && <tag>...}` 을 실제로 파싱.
  **`codegen.rs`는 F(`cond_reconcile_f_system`)를 파라미터화하지 않고, 사이트별로
  F의 제어 흐름(빈 children이면 첫 `true`에 스폰, 아니면 `SetVisible`)을 그대로
  복제해 그 사이트의 실제 정적 자식을 인라인한 새 top-level 함수를 생성**
  (`cond_reconcile_site_N`, `app.add_systems`로 `setup()`/`resume()` 양쪽에
  등록) — 이미 테스트·벤치마크된 `cond_reconcile_f_system` 자체는 손대지 않음
  (RQ1 §2.3 "코드젠 동형 손코딩" 규약과 다른 선택이지만, 사이트마다 다른 정적
  콘텐츠를 제네릭 System 하나로 표현할 방법이 없어 이 방향이 필요했음 — 코드젠
  주석에 이유 기록).
- **재개(resume)**: 캡처 없이 DOM에서 직접 shown/hidden 복구 —
  `dom.adopt_children(wrapper)`가 비어있으면 아직 한 번도 보인 적 없음(hidden),
  아니면 이미 보인 적 있음(shown)으로 판정. 이게 완전한 이유: `setup()`과
  `resume()`의 정착(settle) tick 사이에 아무도 `Bool` 시그널을 건드리지 않으므로
  실제 SSR 출력이 낼 수 있는 상태는 이 두 가지뿐(→ "한 번 보였다가 다시 숨겨진"
  스냅샷은 이 파이프라인이 아예 만들 수 없는 상태 — 코드젠 주석에 명시, 조용히
  누락한 게 아님). `format_i32_capture_for_action`은 `Bool` 시그널을 무조건
  캡처 없음으로 처리(`StringList`와 동일 근거 — DOM 복구형).
- **의도적 스코프 제한(숨기지 않음)**: `then_branch`는 정적 텍스트 자식만 허용
  (중첩 `{...}` 바인딩 불가, `lower.rs`가 거부) + **자기 자신의 `onClick`도 불가**
  (`codegen.rs`가 명시적으로 panic) — 토글 버튼은 항상 별도 형제 엘리먼트여야 함.
  내부에 자기 `Signal<i32>` 상태를 갖는 조건부(F가 "토글에도 상태 보존" 주장을
  증명하는 실제 사례)는 여전히 컴파일러로 못 만듦 — `crates/loom-resume-demo::
  cond_resume`이 이 더 깊은 케이스의 손코딩 증명으로 남음(중첩 바인딩 지원은
  별도 후속 과제).
- **대체된 손코딩**: `demo/counter-list.tsx`에 `details` Bool 시그널 + 토글
  버튼 + `{details() && <p class="details">shown!</p>}` 추가 → 이제
  `generated::setup()`/`resume()`이 이 조건부를 직접 생성. 기존
  `ssr_render_full_app`/`ResumedCond`(RQ6 §4.1 전체-앱 픽스처의 손코딩 조건부
  절반)는 완전히 삭제하고 컴파일러 생성 경로로 교체. `bootstrap.js`의
  `symbol === "CondToggle"` 특수 분기(손코딩 `ResumedCond` 클래스로 라우팅하던
  기존 코드)도 제거 — 토글 액션이 이제 다른 모든 액션과 똑같이 `ResumedComponent`/
  `resume_action_by_symbol`을 그대로 타므로 특수 취급이 필요 없어짐.
- **RQ6 §3.1 베이스라인에 대한 영향(투명하게 기록)**: 위 §3.1 M1 실측(청크
  382,504B raw 등)은 옛 손코딩 조건부 기준이었다 — 이번 교체로 실제 wasm 바이너리
  구성이 달라졌으므로(기능 자체는 동일: 카운터+리스트+조건부), 그 수치는 지금의
  바이너리를 정확히 반영하지 않는다. 재측정은 하지 않음(§3.1은 이미 "1차
  재검토"로 완료 처리됐고, Phase 3 §3.2가 어차피 전체를 다시 실측함) — 다음
  §3.2 실측 라운드에서 새 베이스라인으로 자동 갱신됨.
- **검증**: `cargo test --workspace`(loom-compiler 신규 10개 조건부 테스트 포함)
  + Playwright 실브라우저 — 라이브 데모(토글 3회 연속 클릭, hide/show 상태와
  텍스트 보존 확인, 기존 카운터/리스트 기능 무손상 확인)와 재개 데모(SSR 셸이
  이미 빈 wrapper를 보여줌 → 첫 클릭에 청크 materialize + 표시 → 재사용 인스턴스로
  두 번째 클릭에 숨김 → 무관한 +1 버튼도 독립적으로 정상 재개) 둘 다 통과.

**폼 입력 양방향 바인딩의 컴파일러 통합 (2026-07-24, 같은 세션 — 1단계 마지막
항목) — 위 §4.1 "폼 입력은 제외" 항목 해소:**

- **된 것**: `ir.rs`에 `SignalInit::Str`/`JsxNode::InputBinding{signal}` 추가.
  `lower.rs`가 `<input value={s()} onInput={(e) => s.set(e.target.value)}/>`을
  실제로 파싱(`value`/`onInput`가 같은 시그널을 참조하는지 교차 검증,
  `onInput`이 정확히 `{event_param}.target.value`를 읽는지까지 구조 검증).
  `loom-runtime`에 실제로 새로 필요했던 능력: `DomOps::set_input_value`(속성이
  아니라 `.value` **프로퍼티**를 씀 — 사용자가 한 번이라도 타이핑하면 속성
  갱신은 화면에 반영 안 됨), `DomOps::get_input_value`(재개용, DOM에서 현재
  값을 직접 읽음), `InputValueTarget` 마커 + `patch_input_system`(비제네릭,
  `Signal<String>` 전용, 마커로 게이팅 — 미래에 `TextBinding`이 `String`을
  지원하게 될 때 크로스매치 방지). `BrowserDom`/`StringDom`/`MockDom` 셋 다
  구현·테스트.
- **`SetString` 액션 — 새 메커니즘을 만들지 않고 기존 것에 편입**: 새
  `ClickAction::SetString{signal}`을 만들되, `Action`/`apply_action`/
  `resume_action_by_symbol`을 파라미터화하지 않고 `collect_actions`가
  `InputBinding`을 만나면 이 액션을 합성해 **기존 `ActionN` 넘버링에 끼워
  넣는** 방식 채택 — `setup()`의 새 세 번째 반환값
  `Vec<(NodeHandle, fn(String) -> Action)>`이 라이브 경로(loom-demo)의 실제
  `input` DOM 리스너 배선에 쓰임(`Action::ActionN` 튜플 variant 생성자 자체가
  이미 `fn(String) -> Action`이라 클로저 불필요).
- **실제로 발견해 고친 버그(구현 도중, 가정 아님)**: `lower_node`의
  `Element`(onClick) 분기가 자체 `ctx.action_index` 카운터로 `ActionN` 이름을
  매겼는데, `collect_actions`(같은 이름을 `generate()`에서 미리 계산하는 쪽)는
  트리 순서상 `InputBinding`도 함께 세면서 두 카운터가 어긋날 수 있는 구조였음
  — "onClick 버튼 사이에 `<input>`이 끼는" 실제 케이스로 재현(테스트로 고정:
  `action_numbering_stays_in_sync_when_an_input_sits_between_two_click_actions`)
  하여 발견, `ctx.action_index` 하나를 두 분기가 공유하도록 고쳐 해소. 처음엔
  시그널 이름으로 조회하는 별도 매핑(`input_action_variants`)을 만들었었는데
  이 버그를 고치며 불필요해져 삭제함(더 단순한 설계로 수렴).
- **재개(resume)**: 캡처가 원리적으로 불가능한 유일한 사례 — SSR 시점엔
  사용자가 아직 아무것도 안 쳤으므로 "미래에 타이핑할 값"을 캡처에 실을 수
  없음. `dom.get_input_value(node)`로 DOM에서 직접 현재 값을 읽어와
  `Signal<String>` + `InputValueTarget` + `DomTarget`으로 스폰. `SetString`은
  심볼만으로 결정되지 않는 유일한 액션이라 `resume_action_by_symbol`에
  `string_arg: &str` 매개변수가 새로 추가됨(다른 모든 액션은 무시) —
  `ResumedComponent::dispatch`도 `value: Option<String>`을 받도록 확장
  (`None` = click처럼 값이 필요 없는 액션, `Some` = `input` 이벤트).
  `bootstrap.js`에 `input` 이벤트 리스너 신설(click과 청크
  materialize-and-cache 로직 공유, `dispatchLref` 헬퍼로 통합).
- **`demo/counter-list.tsx`**: `name = signal("")` + `<input value={name()}
  onInput={(e) => name.set(e.target.value)} />` 추가 — RQ6 §4.1 픽스처가
  요구하는 4요소(카운터+리스트+조건부+폼 입력) 중 마지막 하나가 이제 실제로
  컴파일러 산출물로 존재함(다만 아직 `loom-resume-demo`의 별도 전용 픽스처로
  승격은 안 함 — `demo/full-app.tsx` 신설은 이후 라운드로 미룸, 지금은
  `counter-list.tsx` 하나로 충분히 커버됨).
- **검증**: `cargo test --workspace`(loom-compiler 신규 7개 폼 입력 테스트 +
  loom-runtime 신규 `patch_input_system` 네이티브 테스트 포함) + Playwright
  실브라우저 — 라이브 데모(타이핑 반영, 무관한 +1 클릭이 입력값을 안 건드림
  확인 — `Changed<Signal<String>>` 게이팅 검증)와 재개 데모(SSR 셸이 빈
  `<input>`을 미리 보여줌 → 첫 keystroke에 청크 materialize + 값 반영 → 이후
  keystroke는 같은 인스턴스 재사용·청크 재요청 없음(네트워크 요청 수 직접
  카운트) → 무관한 +1 버튼의 독립적 재개도 입력값을 안 건드림) 둘 다 통과.

## Phase 3 — Islands

- 경계 지정 문법 확정 (06의 client directive 유비)
- IslandWorld partition, IslandRoot supervision 분리
- RQ3 규칙 실제 구현, RQ4(Island 간 통신) 최소 버전 시도

**종료 조건**: 중첩 Island 예제 하나가 RQ3 규칙대로 동작. Island 간 통신 예제(장바구니
카운터류) 하나 동작.

**Phase 3 실행 상태 (2026-07-24)**

**핵심 발견 — 모든 결정을 좌우한 제약**: wasm 모듈/인스턴스 경계가 `bevy_ecs::World`
경계보다 더 근본적이다. `bootstrap.js`가 이미 각 LRef의 chunk를 독립된 동적 `import()`로
지연 로드하므로, 서로 다른 청크로 컴파일된 두 Island는 구조적으로 별개의 wasm 인스턴스
(별개의 선형 메모리)다 — Rust 참조/`Entity`/`Box<dyn Trait>`는 그 경계를 못 건너고,
wasm-bindgen이 마셜링한 값만 건넌다. 이 사실 하나가 아래 전부를 동시에 결정했다: Island당
별도 `bevy_ecs::World`(공유 World+마커 방식 기각), IslandRegistry가 JS 쪽 권위 구조여야
함(Rust `HashMap`이 아니라), RQ4 공유 store가 "Layer 0 — Host Glue"라는 새 계층에 있어야
함(docs/04 갱신).

**증분 0 — panic 봉쇄 선결 조건**: `bootstrap.js`에서 실제로 발견한 버그를 고침 —
`instance.dispatch(...)` 호출 자체가 try/catch 밖에 있었음(청크 import/wasm-init/생성자만
감쌌었음). 고쳐서 dispatch가 던지면 그 엘리먼트의 `_loomInstance`를 버리고 이후 그
엘리먼트로의 상호작용을 전부 거부(RQ3 Q4의 attached→failed를 host glue 레벨에서 구현,
영향 범위를 그 엘리먼트 하나로 한정). wasm panic 봉쇄 자체는 RQ6-verification-plan.md
§9-4가 이미 등록해 둔 폴백대로 **조건 A(미해결)를 그대로 채택**(RQ3 재검토 트리거 5번
미발동) — 후보를 실측으로 다 시도해 고른 게 아니라 사전 등록된 시한 규칙을 발동시킨 것,
숨기지 않고 기록.

**증분 1 — `island.rs` + 순수 상태기계 검증**: `crates/loom-runtime/src/island.rs` 신설 —
`IslandState`(RQ3/Lean4 stub과 동일한 6상태), `IslandRegistryEntry`(폐쇄 스키마),
`IslandInstanceHandle`(JsHandle/Native), `IslandInstance`/`IslandRegistryBackend` trait,
`NativeIslandRegistry`(Phase 4 네이티브 임베딩 + 이번 native 테스트 양쪽에 쓰이는 유일한
Rust 구현체 — 이유는 위 핵심 발견 참고). `tests/island_candidates.rs` 11개: 2-island 등록,
전체 lifecycle(dormant→attaching→attached→detaching→detached), dormant/failed의
"actor 비경유" 직행 detach, in-flight attaching의 detach 폐기, 잘못된 전이 panic, **"detach
1개가 다른 island를 안 건드림"(supervision edge 0개)을 실제로 단언**, `IslandInstance::
begin_detach`가 실제로 호출되는지 확인.

**증분 2+3 — 중첩 Island 손코딩 + Island 간 통신 (동시 완료)**: 새 크레이트
`crates/loom-island-demo` — `ProductCardIsland`(`client:visible`)가
`QuantityPickerIsland`(`client:load`)를 물리적으로 감싸는 실제 중첩 구조, 각자 독립된
`LoomApp`/`World`, 같은 `BrowserDom`에 대고 호출(새 trait 불필요, 계획대로). **실제로
구현하며 발견한 것**: 두 Island가 물리적으로 중첩돼 있으면 부모의 DOM 쿼리가 자식 Island의
태그된 엘리먼트까지 주워버리는 문제(opaque 경계 위반) — `own_tagged_descendants`가
`[data-loom-island]` 경계를 만나면 재귀를 멈추는 방식으로 해소(코드젠의 `ResumedComponent::
new`가 미래에 중첩 Island를 지원하려면 똑같이 고쳐야 함 — RQ3에 이미 기록된 "resume 시점
DOM 쿼리" 지점). ProductCard의 "상세 보기" 토글은 `cond_reconcile_f_system`을 직접 쓰지
않고(그건 벤치마크용 하드코딩 span을 스폰함) 그 제어 흐름만 복제한 전용 함수를 손으로
작성 — Task #10에서 코드젠이 한 것과 정확히 같은 이유.

`demo/resume/registry.js`(JS 쪽 authoritative IslandRegistry — `data-loom-island` 스캔,
`client:load`는 즉시 활성화, `client:visible`은 `IntersectionObserver` 1개) +
`demo/resume/shared-store.js`(RQ4 최소 공유 store) + `demo/islands/index.html`(CartBadge는
자기 Island/청크가 아예 없음 — RQ4의 핵심 주장).

**Playwright 실브라우저 검증**(모두 통과, 콘솔/페이지 에러 0): quantity-picker(client:load)가
스크롤 없이 즉시 attached / product-card(client:visible)는 중첩된 quantity-picker가 이미
attach된 후에도 화면 밖에 있는 동안 계속 dormant(Q1 독립 활성화 실증) / quantity 변경 시
CartBadge가 즉시 반영되고 product-card는 여전히 dormant(강제 결합 없음) / 스크롤 시
product-card가 attached로 전이 / "상세 보기" 클릭 시 실제 상품 설명 텍스트 노출 /
quantity-picker는 product-card attach 이후에도 독립적으로 계속 동작(Q2 입양 없음 실증) /
"remove product card" 클릭이 중첩된 quantity-picker까지 cascade로 detached 전이시키고 DOM
subtree가 실제로 사라짐(2단계 detach 실증) / CartBadge(자기 Island 없음)는 제거와 무관하게
값 유지.

**의도적으로 축소한 것 / 여전히 안 한 것 (숨기지 않음)**:
1. Island당 별도 `.wasm` 파일 빌드 툴링 — `loom-island-demo`의 두 Island가 지금도 **같은**
   청크 파일을 공유함(빌드 산출물로 직접 확인됨). "Island=청크=인스턴스 1:1" 불변식은
   여전히 목표 상태이지 현실이 아님 — 이번 Playwright 검증도 이 사실 때문에 "청크 요청
   안 됨"이 아니라 "registry 상태가 dormant로 남음"으로 Q1을 확인(더 정확한 방법으로 대체,
   숨기지 않고 이유 기록).
2. 컴파일러 통합(증분 4) — `ir.rs`/`lower.rs`/`codegen.rs`에 `ComponentGraph`/
   `IslandActivation`/`ComponentRef` 추가는 미착수. 계획 문서가 이미 "증분 0~3만으로 Phase 3
   종료 조건 충족, 컴파일러 통합은 별도 날짜로 추적"이라 명시했으므로 종료 조건 자체는
   충족됨.
3. `sharedSignal()` 저작 표면(증분 5, 스트레치) — 미착수.
4. crash 통지 채널(RQ4로 이관된 요구) — 값 공유만 구현, 상태 변화 통지는 미착수.
5. 비동기 정리(구조적 동시성 취소)의 실제 타임아웃 의미론 — 이번 `begin_detach`/
   `finish_detach`는 동기 즉시 호출(RQ3의 "알려진 약점" #5, 미결로 남김).
6. 리스트 row 안의 Island N개 인스턴스, 크로스 임베드 DOM 중첩 — 계획대로 범위 밖.

## RQ6 §3.2 — "Phase 3 완료, 첫 정면 대결" 실측 (2026-07-24)

상세는 `research-questions/RQ6-verification-plan.md`의 "§3.2 실행" 절 +
`research-questions/data/rq6-phase3-widget-embed-chromium.json`(원시 데이터) —
여기선 요지만.

**§3.2 자체 규칙("축소 불가 4항목")을 전부 실제로 수행함**: (1) RQ3 6-상태
기계/2단계 detach는 Phase 3에서 이미 완료. (2) **panic 봉쇄 조건 B(island별
wasm 인스턴스 분리)를 실제 구현** — 새 크레이트 `crates/loom-island-productcard`/
`crates/loom-island-quantitypicker`(진짜 별도 `.wasm`)로 Playwright 장애 주입
실측: 한쪽 panic이 다른 쪽 인스턴스에 전혀 영향 없음(완전 봉쇄 실증). (3)
**panic 봉쇄 조건 C를 실제로 시도해 이 문서/RQ3의 기존 서술을 정정** — nightly +
`-Zbuild-std` 조합으로 진짜 `panic=unwind` wasm 바이너리를 빌드해 표준
exception-handling 인코딩(`try_table`/`throw_ref`)이 실제로 담긴 산출물을
`wasm-bindgen-cli`로 처리·실행한 결과, "wasm-bindgen이 이 인코딩에서 깨진다"던
알려진 버그가 **재현되지 않음**(단 nightly 한정, stable 통합은 별도 과제 —
상세는 RQ3 문서). (4) **MutationObserver 기반 호스트발 제거 감지를 실제 구현**
— `registry.js`에 추가, "레거시 스크립트가 explicit unmount API 없이 직접 DOM을
지우는" 시나리오를 실제로 재현해 정상 감지·정리됨을 확인.

**§4.1 전체-앱 Island 팔은 이번에 안 만듦(숨기지 않은 이유)**: §4.1의 "정체성
통제" 원칙(같은 소스에서 지시어만 다르게 파생)은 컴파일러 통합(Phase 3 증분 4,
스트레치로 미룸) 없이는 손코딩으로 만족시킬 수 없음 — 어기고 만드느니 갭으로
남김. 대신 §4.2 위젯-임베드 픽스처(Phase 3의 `loom-island-demo` 그대로 재사용,
정체성 통제 문제 없음)를 실측.

**실측 결과 요지**: M1 — Island의 상시 로드분(Layer 0: registry.js+shared-store.js)
이 H-Qwik의 bootstrap.js보다 raw 62%/gzip 52% 더 큼(오케스트레이션 고정비).
청크를 공유하면(조건 A) H-Island 쪽이 작아 보이지만 위젯 구성이 다른 픽스처라
직접 비교 불가. **인스턴스 분리(조건 B)의 실제 가격표: 공유 대비 +91% 바이트**
— "진짜 격리"는 공짜가 아님을 수치로 확인(M5(b)에 계상). M8 — 이름/ID 충돌
시나리오에서 opaque 경계 스코프 쿼리 덕에 전역 충돌 무영향 확인(`ResumedComponent::
new`의 페이지 전역 쿼리 취약점과 대조). M9 — cross-widget 정확성 200/200(100%),
지연시간 median 0.0ms/max 0.6ms(공유 store가 순수 동기 JS 경로라 사실상 O(0)).

**미실측(§10 절차대로 명시)**: Firefox/WebKit, N-스케일링({1,10,50})·원본 앱
규모 축, 호스트 간섭 3패턴(§5.3), M2-M4·M7·M10, §4.1 Island 팔. 전부 판정이
아니라 데이터 등록 — §7 정식 판정은 Phase 6 전유.

## Phase 4 — 백엔드 어댑터

- FIC 확정 (05)
- WASM-in-process 하네스 먼저 (가장 저마찰, Rust 우선순위와 일치)
- Sidecar HTTP 다음
- 네이티브 바인딩 1~2개(Node, Python 등) — "진짜 언어 무관"임을 증명하는 용도

**종료 조건**: 최소 2개 이상 서로 다른 언어의 "백엔드"에서 동일 컴포넌트를 렌더링해 임베드 성공.

**실행 상태 (2026-07-24) — 종료 조건 충족**

`crates/loom-fic` 신설. FIC를 `render_html() -> String`(같은 `demo/counter-list.tsx`
산출물 재사용, 새 컴포넌트 안 만듦 — CLAUDE.md 2-1) + C ABI shim(`loom_fic_render`/
`loom_fic_free`, `[len: u32 LE][utf8 html]` 와이어 포맷)으로 구현. `StringDom`은
`loom-resume-demo`에서 그대로 재사용하지 않고 복사함 — 그 크레이트는 wasm-bindgen/
web-sys를 무조건 의존해서(ResumedComponent 경로 때문에), 재사용하면 순수 wasmtime
호스트를 위한 "JS 없는" wasm32 빌드가 깨짐(모듈 문서에 이유 상세).

**세 경로 전부 실제 구현·검증**:
1. **네이티브 바인딩** — 계획(PyO3/napi-rs)보다 더 얇게 감: Python `ctypes`가 C ABI
   `.so`를 언어별 래퍼 크레이트 없이 직접 로드(`examples/native_binding.py`) —
   "하나의 C shim이 전체를 흡수"를 문자 그대로 밀어붙이면 PyO3조차 선택 사항임을 실측.
2. **Sidecar HTTP** — `std::net::TcpListener`만 쓴 최소 서버(`loom-fic-sidecar`
   바이너리, 프레임워크 없음) — Node.js `fetch()`로 호출(`examples/sidecar_client.js`,
   `embed_host_page.js`).
3. **WASM-in-process** — `wasm32-unknown-unknown`(wasm-bindgen 없는 plain 빌드) +
   Python `wasmtime`(`examples/wasm_in_process.py`). **실제로 발견한 마찰(숨기지
   않음)**: `bevy_ecs`가 `bevy_tasks`의 wasm32 비동기 실행기를 통해
   `wasm-bindgen-futures`에 **무조건** 의존 — 이 렌더 경로가 그 코드를 전혀 안 쓰는데도
   인스턴스화 시점에 `__wbindgen_*` import 4개를 요구함. `wasm-tools print`로 정확히
   어떤 4개인지 확인(`__wbindgen_describe`, `__wbg___wbindgen_throw_*`,
   `__wbindgen_externref_table_grow`/`_set_null`) 후 `wasmtime`의
   `Linker.define_func`로 no-op 스텁 제공해 해결(이 경로가 실제로 그 함수들을
   호출 안 함을 확인하고 안전하다고 판단). `getrandom`도 wasm32 기본 백엔드(`wasm_js`,
   wasm-bindgen 의존)를 요구해 "custom backend"(`--cfg getrandom_backend="custom"`
   + 직접 구현한 `__getrandom_v03_custom`)로 대체 — 결정론적 단발 SSR이라 진짜
   난수가 불필요해서 안전(주석에 왜 안전한지 명시, 실제 서비스라면 진짜 난수 소스
   import 필요).

**종료 조건 실측**: Python(ctypes 네이티브 바인딩 + wasmtime WASM-in-process 둘 다) +
Node.js(Sidecar HTTP) — 서로 다른 두 언어에서 동일 컴포넌트를 렌더링해 각자 독립적으로
만든 "레거시 호스트 페이지"(loom이 생성 안 한 nav/footer 포함)에 실제로 임베드 성공,
Playwright로 스크린샷 검증(`#loom-mount`에 "count:0" 등 실제 컨텐츠 렌더링 확인).

**검증**: `cargo test -p loom-fic`(2개, `render_html`/C ABI 왕복 각각) +
`cargo test --workspace`(96개 전체 통과) + 위 크로스-언어 임베드 실측.

**의도적으로 미룬 것(숨기지 않음)**: `resumability_payload`/`assets` 필드(반환 HTML이
이미 모든 정보를 인라인으로 담고 있어 지금은 중복), `request_context`의 실제 파라미터화
(Phase 1 IR이 요청별 동적 초기값을 지원 안 함 — 스키마 자체가 자명하게 빔), Sidecar의
콜드 스타트/커넥션 풀링 전략, Phase 3 §3.3 언급된 "선택 사항"인 실제 폴리글랏 백엔드
재확인(Phase 6 몫).

## Phase 5 — 공식 검증

- RQ5 우선순위(teardown 안전성 → resumability 건전성 → 스케줄러 무충돌성)대로
- 이 시점엔 설계가 실제 코드로 안정화되어 있어야 함 — 추상적 스펙을 먼저 증명하고
  나중에 코드를 맞추지 말 것

## Phase 6 — RQ6 최종 검증

- Phase 2 종료 시점의 벤치마크와 Phase 3 완료 후 벤치마크를 비교
- Island 도입이 실제로 이득(백엔드 임베딩 유연성)에 비해 복잡도(RQ3/RQ4) 비용을
  정당화하는지 최종 판단. 이 시점에 04의 재해석이 맞았는지 여부가 결정됨
