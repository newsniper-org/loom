# RQ3. Island 중첩 시 Supervision 전파 규칙

> **2026-07-28 (Phase 6, Task #29) — 부분 폐기 공지.** RQ6 §7 게이트가 H-Qwik을
> 확정(강한 형태)함에 따라, 이 문서가 전제했던 "Island 경계가 여러 개 있고 그
> 사이의 관계를 규율해야 한다"는 상황은 **기본 모델에서는 더 이상 발생하지
> 않는다**(docs/04 재해석 — 기본은 하나의 World, 하나의 supervision tree).
> 그래서 이 문서의 내용 중 **"경계가 여럿이라서 필요했던 부분"**(Q1의 "독립
> resume", Q2의 "입양 없음/forest", IslandRegistry의 containment 추적, Q3의
> "중첩 island 간" 조율)은 **세분화를 선택했을 때만 적용되는 규칙**으로
> 지위가 낮아진다 — 삭제하지 않는다, 세분화 자체가 폐지된 게 아니라 필수에서
> 선택으로 바뀌었을 뿐이기 때문(docs/04 "선택적 최적화" 절).
>
> 반면 **"지연 활성화되는 서브트리를 안전하게 해제한다"는 더 일반적인 문제의
> 답으로 나온 부분은 폐기되지 않고 그대로 유지된다** — 이걸 통째로 버리면 이미
> 검증된 설계(2단계 detach가 해소한 "동기 대기 vs 좀비 창 딜레마")를 다시
> 발명하게 된다(CLAUDE.md 2-1 위반). 구체적으로:
> - **유지**: Q3의 2단계 detach 프로토콜(동기 detaching 표시 + 비동기 정리),
>   Q4의 dormant/attached/failed 상태 개념과 promote-once-and-stay/no-restart
>   정책 — 이건 세분화 여부와 무관하게 "지연 activation된 서브트리 하나"에
>   그대로 적용됨(docs/06 갱신 참고).
> - **세분화 선택 시로 좁혀짐**: Q1(독립 resume — 기본 모델엔 애초에 "다른
>   Island"가 없음), Q2(입양 없음/supervision forest — 경계가 하나뿐이면
>   forest 자체가 성립 안 함), IslandRegistry의 containment 추적(경계가
>   여럿일 때만 의미 있음), panic 봉쇄 조건 B(인스턴스 분리 — 경계가 있어야
>   가능).
> - **RQ4와의 연동이 소거됨**: RQ4가 전면 폐기됨에 따라(RQ4 문서 참고),
>   "cross-island 통신"으로 이관됐던 항목(crash 통지 채널 등)은 이제 "경계가
>   있을 때만 의미 있는 부가 기능" — Task #28-b가 이미 구현한 crash 통지
>   채널(`window.__loomShared`)은 세분화 여부와 무관하게 유용하므로 유지.
>
> 아래 원문은 수정하지 않는다(이력 보존) — 각 절을 읽을 때 위 재구분을 적용할 것.

## 문제

06에서 짚은 대로, Island은 supervision subtree의 root지만 실제 앱에서는 다음과 같은
중첩이 자연스럽게 발생함:

```
<StaticHeader>              ← 정적, actor 없음
  <ProductCard client:visible>   ← Island A (root actor A)
    <StaticBadge />               ← 정적, actor 없음
    <QuantityPicker client:load>  ← Island B (root actor B, A의 자식?)
  </ProductCard>
</StaticHeader>
```

여기서 Island B가 Island A 내부에 있을 때:

1. B의 activation이 A의 activation을 강제로 트리거해야 하는가, 아니면 완전히 독립적으로
   resume될 수 있는가? (04의 "각 Island은 독립적으로 resume" 원칙과, 06의 "supervision은
   부모-자식 생사를 관장"이라는 원칙이 여기서 충돌 가능)
2. A가 나중에(다른 트리거로) activate되면, 이미 떠 있는 B의 actor를 A의 supervision tree
   안으로 "입양"해야 하는가, 아니면 B는 영구히 별도 tree로 남는가?
3. 부모 Island이 detach되면 자식 Island도 강제로 detach되어야 하는가 — 이건 "격리
   단위"라는 04의 정의와 배치될 수 있음(자식이 진짜 독립적이라면 부모 소멸에 영향받지 않아야
   할 수도)

추가로 06이 이 RQ에 연결해 둔 질문: (4) 지연 spawn된 actor의 승격 정책 — 최초 상호작용
후 상주하는가, 매 상호작용마다 재지연되는가.

## 왜 중요한가

이 규칙을 잘못 정하면 04에서 설계한 "격리 단위" 속성 자체가 무너짐 — Island이 서로 독립이
아니라 은근히 얽혀있게 됨.

## 잠정 규칙 (2026-07-23 확정, Phase 0)

> **상태: 잠정(provisional).** 4개 렌즈(격리 우선 / supervision 우선 / 물리 제약 유도 /
> 선례 우선)의 독립 규칙안을 7개 시나리오 공격 + 문서 정합성 공격에 통과시킨 뒤 생존
> 요소를 조합한 것이다. 실측 검증 전의 설계 가설이며, Phase 1-2 프로토타입에서 중첩
> island detach·crash 시나리오를 실제로 돌리기 전까지는 아래 '재검토 트리거'와 함께
> 읽어야 한다.

**한 문장 요약: island 간 결합은 DOM이라는 물리 기반의 공유에서만 오고, supervision
tree에서는 결코 오지 않는다. island 경계를 넘는 supervision edge는 0개다.**

### Q1. Activation — 완전 독립 resume

- B의 activation은 A의 activation·코드 청크 로드·IslandWorld 역직렬화 중 어느 것도 강제하지
  않는다. B의 root actor는 spawn 시 부모 island lookup 자체를 하지 않는다.
- "activation"의 정의: **LRef 발화 또는 `client:` 지시어 조건 충족** (상호작용 경로만이 아님).
- 이벤트 버블링 의미론: B 내부의 물리 이벤트가 A의 조상 노드 LRef까지 버블링해 A를 깨우는
  것은 허용되며, 이는 "B가 A를 트리거"한 것이 아니라 **같은 물리 이벤트가 두 island를 각각
  독자 트리거**한 것으로 정의한다(버블링 차단 없음).
- 근거: 06의 "actor 부재가 기본값" — 부모가 static이면 actor가 영원히 없을 수 있으므로
  "부모 actor 없음 + 자식 actor 있음"은 이미 정상 경로다. 부모가 우연히 Island라는 이유로
  규칙이 달라지면 정적 부모와 dormant 부모가 다른 의미론을 갖게 된다. 또한 dispatch·직렬화·
  spawn 어느 경로에도 부모 actor를 요구하는 물리적 제약이 없다.
- 기각한 대안: **hollow 스파인**(조상 tree 자리를 선실체화해 단일 tree 유지) — "위상은
  컴파일타임 확정·런타임 불변"이라는 전제가 리스트 row 내 island(동일 정의의 인스턴스 N개)와
  조건부 재생성에서 거짓이 되고, 정적 경로 마커(`data-loom-path`)는 인스턴스를 식별하지
  못한다(RQ1이 기록한 site_id 불충분 문제와 동형). 공격에서 치명 판정.

### Q2. 입양 — 없음, 영구 별도 tree (supervision forest)

- A가 나중에 activate되어도 B의 tree와 병합되지 않으며, A는 B에 대한 kill/restart 권한을
  영원히 갖지 않는다. 각 island root는 자기 tree의 최종 trap 지점이다.
- A는 IslandRegistry(아래)에서 B의 존재와 상태 enum을 **조회**할 수 있을 뿐, **관장**할 수 없다.
- A→B 데이터는 SSR 시점 prop 스냅샷에 동결된다. 동적 전달은 RQ4 관할이며, A가 늦게 깨어난
  시점에 A의 스냅샷과 B의 실제 상태가 불일치할 수 있고 **프레임워크는 이를 감지하지 못한다**
  (알려진 제한, 숨기지 말 것).
- 근거: 입양을 허용하면 B의 장애 도메인이 "A가 언제 activate되었는가"라는 런타임 순서에 따라
  변해 04의 격리 경계가 비결정적이 된다. OTP에 실행 중 re-parenting 프리미티브가 없다는
  선례도 이를 뒷받침한다. forest는 activation 순서와 무관하게 최종 topology가 동일하다.
- 기각한 대안: **monitor 채널**(A가 B를 단방향 관찰, IslandDown 통지) — 등록 시점이 감시자
  spawn 시점 하나뿐이라 부모-먼저 순서에서 통지가 조용히 유실되는 결함이 있고, 사실상 최초의
  cross-island 메시지 채널이라 RQ4를 선점한다. crash 통지 요구가 실증되면 RQ4 논의로 이관.

### Q3. Detach cascade — 강제 detach되나, supervision이 아닌 환경 채널로

**채널 분리가 핵심이다: detach는 lifecycle 채널(하향 전파), crash는 fault 채널(경계 차단).**
04의 격리는 fault 격리이지 소멸 면제가 아니므로 이 규칙은 04와 배치되지 않는다.
A가 B를 죽이는 것이 아니라, B가 "내 host 환경이 소멸한다"는 물리적 사실에 반응하는 것이다.

- **발화 조건 (4안 공격의 공통 최대 구멍 S5의 수리): detach의 발화 사실은 'island detach
  이벤트'가 아니라 '실제 DOM 범위 제거'다.** 발화 경로 3개:
  1. island 자신의 detach (root actor의 detaching 진입)
  2. **Layer 2 patch의 subtree 제거 op** (리스트 row despawn, 조건부 despawn) — 제거 범위 내
     island host 포함 여부를 Registry에 조회한다. 단 island 경계는 컴파일타임 확정이므로,
     island host를 포함할 수 없는 제거 op는 **컴파일타임 플래그로 조회를 생략**해 no-VDOM
     patch 경로의 O(interaction) 속성 침식을 억제한다.
  3. 호스트 환경발 제거 (05 임베딩 시나리오) — **명시적 unmount API가 1차 계약**,
     MutationObserver 감시는 보강 후보 (감지 경로 설계는 Phase 3 종료 조건에 포함).
- **2단계 detach 프로토콜** (동기 대기 vs 좀비 창 딜레마의 해소):
  - [동기] Registry 조회 → 제거 범위 내 모든 island를 즉시 **detaching**으로 표시. 이 시점부터
    해당 tree의 DOM 접근·이벤트 라우팅·patch 발행이 차단된다. dormant/failed island는
    **detached로 직행**(actor 비경유 환경 전이). 그 후 DOM을 **즉시 제거**한다(완료 대기 없음).
  - [비동기] 각 detaching tree는 06의 구조적 동시성 취소로 in-flight 작업을 정리하고 detached로
    전이. 타임아웃 초과 시 강제 drop(의미론 미정 — Phase 3).
  - 결과: "죽은 DOM을 만질 수 있는 창"은 동기 차단으로 0이 되고, 정리 완료만 eventually로
    완화된다. 라우트 전환·리스트 삭제가 자식 cleanup에 인질로 잡히지 않는다.
- **DOM 소유권**: island host 엘리먼트는 부모 문맥 소유다. island의 자체 정리는 host **내부**만
  건드리고 host는 남긴다(부모 patch anchor의 dangling 방지). 부모 patch는 자식 island host
  내부를 절대 patch하지 않는다(opaque 경계).
- **crash와의 분리**: A의 crash는 DOM을 제거하지 않으므로 B에 아무 영향이 없다. Phase 0의
  no-restart 정책(Q4) 덕에 "A restart가 DOM 재렌더로 B를 파괴"하는 경로 자체가 존재하지 않는다.
- **RQ1-F 조건화 (잠정)**: 이 cascade는 '실제 DOM 파괴'에만 걸린다. 따라서 **island host를
  포함한 조건부 분기는 토글-off 시 보존형(F, OP_DETACH_TO_FRAGMENT)이 아닌 despawn 의미론을
  강제**한다. RQ1 B/F 실측에서 F가 이기면 frozen류 제3 상태와 함께 본 조항을 재개정한다.

### Q4. 승격 — promote-once-and-stay + no-restart

- 한 번 attached가 된 island root actor는 그 island가 detach될 때까지 상주한다.
  attached→dormant 역전이는 상태기계에 없다.
- **일반 원칙: 승격은 resumability의 비가역적 소비다.** serialize는 SSR 시점에만 일어나며
  클라이언트측 재직렬화(bfcache류 복원, 세션 저장)는 Phase 0 범위 밖이다.
- **crash 정책 (Phase 0): restart 없음.** island root까지 escalate된 crash는 **failed** 전이 +
  DOM 동결(inert, 리스너 마커 제거)로 흡수한다. 복구는 페이지 재로드/재-SSR에 맡긴다.
  상호작용 후 상태는 소실된다 — 이를 숨기지 않고 명시한다.
- 근거: 재지연은 미해결 RQ2 전체(클라이언트 재직렬화기)를 매 이벤트마다 다시 푸는 것.
  "마지막 스냅샷에서 재시작"은 공격에서 이중 붕괴했다 — 스냅샷 갱신 규칙이 없어 SSR본으로의
  무통보 롤백이고, 복원된 World와 crash 직전까지 패치된 DOM이 불일치하는데 Changed 게이팅은
  이를 감지 못하며 No-VDOM이라 재렌더 탈출구도 없다. no-restart가 이 문제군 전체를 회피하는
  유일한 단순 규칙이다.

## 메커니즘: IslandRegistry

어떤 IslandWorld/supervision tree에도 속하지 않는 런타임 인스턴스당 1개의 중립 구조.

- **등록 시점: SSR 마커 스캔/로드 시점** (resume 시점이 아님 — dormant island가 containment
  조회에서 누락되는 결함의 수리).
- **스키마 폐쇄**: `{island id, host element ref, 상태 enum, actor handle?}`. 임의 페이로드
  필드 금지 — RQ4의 공유 상태 부착점이 되지 않도록 토폴로지 추적 + detach 발행 전용으로
  경계를 명문화한다. crash 관찰 이벤트 구독 기능은 Phase 0에서 제외(RQ4 이관).
- Registry 스코프는 런타임 인스턴스당이다. 한 페이지에 복수 임베드가 꽂히고 임베드 간 DOM
  중첩이 발생하는 경우는 **미정의**(알려진 한계, 05 접속점).

## 4안 수렴/분기 기록

수렴(만장일치): Q1 독립 resume, Q2 입양 없음, Q3 동반 detach 불가피 + lifecycle/fault 채널
분리, Q4 promote-once, dormant 상태 필요. 공격 결과도 수렴 — S5(patch에 의한 island 파괴)가
4안 공통 최대 구멍이었고, wasm panic 전제 누락도 공통 결함이었다.

분기와 판정: hollow 단일 tree(기각 — 동적 리스트에서 공리 붕괴), monitor 채널(기각 — 순서
의존 유실 + RQ4 선점), 자식 정리 완료 동기 대기(기각 — 최빈 경로 인질화, 2단계 프로토콜로
대체), 스냅샷 재시작(기각 — World-DOM 정합성 붕괴, no-restart로 대체), failed 상태(채택).

## Lean4 ActorState 영향

상태 4개 → **6개**: `dormant`(DOM 존재 + LRef 무장 + actor 부재), `failed`(fault 흡수,
DOM 동결 존재, 부모 cascade 시 정리 대상) 추가. 주요 전이:

```
dormant   → attaching   (activation)
dormant   → detached    (환경발 제거 — actor 비경유, Registry가 직접 수행)
attaching → attached | detaching(로드 결과 폐기) | failed(로드 실패)
attached  → detaching | failed(crash, no-restart)
detaching → detached
failed    → detached    (상위 cascade 시 정리)
```

역전이 없음(attached→dormant 부재 = 승격 비가역의 상태기계 표현). SupervisionNode.children은
island 내부 tree만 다루고 island 간 관계는 Registry containment로 별도 모델링 — 증명 대상은
"단일 tree 불변식"이 아니라 **actor 상태기계 × Registry 상태기계의 합성**이다(형식화 부담이
줄어든 게 아니라 이동했음을 인정). `no_dangling_entity_after_detach`는 2단계 detach에 맞춰
두 정리로 분리: (a) 동기 안전성 — detaching 표시 이후 어떤 System도 해당 tree의 DOM/Entity
접근을 새로 시작할 수 없다, (b) 진행성 — detaching 진입 후 유한 단계(타임아웃 상계) 내
detached 도달.

## 알려진 약점 (숨기지 말 것)

1. **wasm panic 봉쇄는 이 규칙의 전제이지 결론이 아니다.** wasm32 기본 panic=abort + 단일
   공유 인스턴스면 B의 panic이 supervision 형태와 무관하게 앱 전체를 죽인다. Phase 1-2에서
   catch_unwind(wasm exception-handling 의존) / panic 회피 규약 / island별 인스턴스 분리 중
   하나를 실측으로 골라야 하며, 이 선택 없이는 fault 격리 주장이 공허하다.
   **2026-07-24 (Phase 3) 갱신**: RQ6-verification-plan.md §9-4가 이미 등록해 둔 폴백대로
   **조건 A(미해결)를 Phase 3 기본값으로 실제 채택**(후보를 실측으로 다 시도해 고른 것이
   아니라, 시한 내 미결이면 A를 쓴다는 사전 등록된 규칙을 그대로 발동시킨 것 — 재검토
   트리거 5번은 아직 발동 안 함). 대신 안정 채널·리스크 0인 완화책 2개를 실제로 실행함:
   (a) "Island 1개 = wasm 청크 1개 = wasm 인스턴스 1개" 불변식은 여전히 목표 상태일 뿐—
   `crates/loom-island-demo`의 두 Island(ProductCard/QuantityPicker)가 **같은** 청크
   파일(`loom_island_demo`)을 공유하는 현재 빌드 산출물이 그 증거(빌드 툴링 작업은 여전히
   범위 밖). (b) 실제로 발견해 고친 진짜 버그: `bootstrap.js`의 `dispatch(...)` 호출 자체가
   try/catch 밖에 있었음(청크 import/wasm-init/생성자만 감쌌었음) — 고쳐서 dispatch가
   던지면 해당 엘리먼트의 `_loomInstance`를 버리고 이후 그 엘리먼트로의 dispatch를 전부
   거부하도록 함(RQ3 Q4의 attached→failed 전이를 host glue 레벨에서 실제로 구현한 것,
   영향 범위를 그 엘리먼트 하나로 한정).
   **2026-07-24 (RQ6 §3.2 실측 라운드) — 조건 C를 실제로 시도해 이 문서의 기존 주장을
   정정함(중요, 숨기지 않음)**: 이 항목이 인용한 "표준화된 TryTable 인코딩이
   wasm-bindgen의 CLI 인터프리터를 깨는 현재 열려 있는 호환성 버그(wasm-bindgen#4861)"를
   실제로 재현해보려고 시도했는데, **재현되지 않았다**. 실제로 한 것: nightly 툴체인에서
   `wasm32-unknown-unknown` 대상의 사전 빌드 std를 제거하고(`rustup target remove
   wasm32-unknown-unknown --toolchain nightly`, prebuilt std와 `-Zbuild-std`가 충돌하는
   `E0152 duplicate lang item` 회피), `RUSTFLAGS="-Cpanic=unwind" cargo +nightly build
   --target wasm32-unknown-unknown -Zbuild-std=std,panic_unwind
   -Zbuild-std-features=panic-unwind`로 실제 `panic=unwind` 바이너리를 빌드하고,
   `catch_unwind`로 감싼 함수 하나(패닉하는 경로/안 하는 경로 각각 호출)를
   `wasm-bindgen-cli 0.2.126`(이 저장소가 쓰는 버전과 동일)으로 직접 처리(wasm-pack이
   아니라 CLI를 직접 호출 — wasm-pack은 기본적으로 stable 툴체인만 사용해 이 실험을
   못 태움). `wasm-tools print`로 산출물을 실제 확인해 `try_table`/`throw_ref` 명령어
   (표준화된 wasm exception-handling 제안의 실제 인코딩, 레거시 `try`/`delegate`가
   아님)가 실제로 들어있음을 확인 — 즉 "쉬운 케이스라 TryTable을 안 쓴 것"이 아니라
   진짜 TryTable 인코딩을 담은 바이너리로 테스트한 것. **결과**: `wasm-bindgen` CLI가
   에러 없이 처리했고(종료 코드 0), Chromium에서 실제로 로드해 호출한 결과 패닉하는
   호출은 `catch_unwind`가 정확히 잡아 정수 반환값(-1)으로 돌아왔으며, **같은 wasm
   인스턴스가 그 후에도 정상 호출(42 반환)에 계속 응답**함 — 인스턴스가 오염되거나
   죽지 않았다. **단, 과대 해석 금지**: (a) 이건 stable 채널이 아니라 nightly +
   `-Zbuild-std` 조합이라 "안정 채널에서 no-op"이라는 이 항목의 원래 문장은 여전히
   유효 — 안정 채널 자체가 뚫린 게 아니라 "nightly로 실제 빌드는 가능하고 작동한다"는
   사실이 새로 확인된 것. (b) 이번 실험은 Chromium 1개 브라우저, `loom-island-demo`가
   아니라 별도 최소 재현 크레이트(`/tmp` 스크래치패드, 저장소에 커밋 안 됨)에서만
   확인함 — 실제 `crates/loom-island-demo`/`bootstrap.js`에 조건 C를 통합하려면
   wasm-pack이 기본으로 지원 안 하는 nightly 빌드 스크립트가 별도로 필요함(다음 단계로
   미룸, 이번엔 "가능한지" 자체만 검증). (c) `wasm-bindgen#4861`이 인용하는 정확히
   그 실패 패턴을 내가 재현 못 한 것뿐일 수도 있음(다른 코드 패턴에서는 여전히 깨질
   가능성 배제 못 함) — "그 버그가 존재하지 않는다"가 아니라 "이 저장소가 실제 쓰는
   `catch_unwind` 사용 패턴 + wasm-bindgen 0.2.126 조합에서는 재현 안 됨"으로 한정해
   기록한다. **RQ6-verification-plan.md §5.4 조건 C 재평가는 그 문서에 기록.**

   **2026-07-28 재검토 — stable 채널 승격 가능성 재확인, 여전히 뚫리지 않음**:
   병익님이 "catch_unwind가 stable에 이미 있는 것 같다"는 관찰을 계기로,
   Rust 1.96(edition 2024)로 실제 상향한 뒤 다시 시도. 확인한 것과 그 순서:
   (1) `std::panic::catch_unwind` 자체는 원래도 stable API였음(착각이 아니라
   사실) — 막혔던 지점은 그게 아니라 `wasm32-unknown-unknown`의 **프리빌드
   std가 `panic=abort`로 고정 배포**되어 있다는 것. (2) RUSTFLAGS로 명시적
   `-C panic=unwind`를 주면(build-std 없이) `error: the crate 'panic_unwind'
   does not have the panic strategy 'unwind'`로 하드 실패 — 1.96에서도 동일.
   (3) `Cargo.toml`의 `[profile.release]`/`[profile.dev]`에 `panic = "unwind"`
   를 직접 설정하면(RUSTFLAGS 없이) **에러 없이 빌드는 되지만**, 산출된 wasm에
   실제 unwind 인코딩(`try_table`/`throw_ref`)이 **0개**이고 `unreachable`
   트랩만 나옴(`crates/loom-widget-embed`를 wasm32 릴리스로 실제 빌드해
   `wasm-tools print`로 직접 확인, RUSTFLAGS 유무와 무관하게 동일 바이너리) —
   즉 이 경로는 하드 에러 대신 **조용히 abort로 폴백**한다(`cdylib` crate-type
   조합에서 Cargo/rustc가 그렇게 처리하는 것으로 보이나, 정확한 내부 이유는
   이번 라운드에서 확인 안 함). **결론**: bevy_ecs 0.19/getrandom 0.4/edition
   2024/rust 1.96까지 전부 최신으로 올려도 **stable 채널에서 build-std 없이
   wasm32 panic=unwind를 얻는 방법은 여전히 없다** — 1.96에서도 안 풀림.

   **같은 라운드 — wasm64-unknown-unknown도 시도, wasm32보다 더 나쁨**: "혹시
   64비트 타겟이면 다를까"라는 질문으로 병행 확인. `wasm64-unknown-unknown`은
   `rustc --print target-list`엔 있지만 **Tier 3라 stable/nightly 어느 채널도
   프리빌드 std를 배포하지 않음**(`rustup target add`가 두 채널 다 즉시 실패,
   "no prebuilt artifacts available"). nightly + `-Zbuild-std=std,panic_unwind,
   panic_abort -Zbuild-std-features=panic-unwind`로 std 자체를 처음부터
   빌드하면 컴파일은 성공(unwind 전략 지정 포함, 에러 없음)하지만, 산출된 wasm
   (`--release`/`--dev` 둘 다 확인)에 `try_table`/`throw_ref`가 여전히
   **0개** — 코드생성 백엔드 자체가 이 타겟에 대해 wasm exception-handling을
   아직 구현하지 않은 것으로 보임(target-spec-json의 `eh-frame-header: false`
   와 정합). **wasm64는 wasm32보다 지원 수준이 더 낮다**: wasm32는 최소한
   nightly+build-std 조합에서 실제로 작동한다는 게 이미 확인돼 있는데(위 2026-
   07-24 절), wasm64는 nightly+build-std로도 작동하지 않는다. CLAUDE.md가 명시한
   클라이언트 배포 타겟(WASM, 사실상 wasm32)에는 영향 없는 참고 정보지만, "다른
   타겟으로 우회할 수 있는가"라는 질문에 대한 답도 실측으로 막힘을 확인했다는
   점에서 조건 C의 막다른 골목이 이번 라운드로 한 번 더 좁혀졌다(nightly +
   wasm32 + build-std 조합 하나만 유효 경로로 남음).

   **같은 라운드 — 조건 B(island별 wasm 인스턴스 분리)도 실제로 구현·측정함**: 새 크레이트
   `crates/loom-island-productcard`/`crates/loom-island-quantitypicker`(각자 독립된
   `.wasm` 산출물 — `loom-island-demo`가 하나의 청크를 공유하는 것과 달리 이번엔 진짜로
   분리)를 만들고, `demo/islands/condition-b/index.html`에서 둘을 같은 페이지에 함께
   로드해 QuantityPicker 쪽에 의도적 panic을 주입(`panic_now()`)한 뒤 Playwright로 실제
   확인: (1) panic이 잡을 수 있는 JS 예외로만 나타나고 페이지 전체를 안 죽임, (2)
   **별도 wasm 인스턴스인 ProductCard는 완전히 무사**(패닉 전과 동일하게 계속
   상호작용 가능) — 조건 B의 핵심 주장이 실제로 성립함을 직접 관찰. 부수 관찰:
   panic이 난 QuantityPicker 인스턴스 자신은 이후 호출에서 wasm-bindgen 자체의
   `WasmRefCell` borrow-poison 메커니즘("recursive use of an object detected which
   would lead to unsafe aliasing") 때문에 영구적으로 응답 불능이 됨 — 이는 버그가
   아니라 **정확히 RQ3 Q4의 no-restart/failed 상태가 요구하는 동작**(오염된 객체는
   그대로 두고 복구하지 않음)과 우연히 정합적으로 들어맞음. M6(장애 격리)/M5(b)(인스턴스
   분리 비용) 실측 데이터로 RQ6-verification-plan.md에 등록.
2. crash 시 상호작용 후 상태 전손(no-restart의 의도된 비용). failed island의 사용자 노출
   정책(동결 vs 폴백 마크업)은 미결.
3. 중첩 island 간 동적 데이터 전달 부재(SSR prop 동결) + 불일치 감지 불가 — RQ4 선결 의존.
4. 상주 승격으로 활성 actor 단조 증가 — hibernation 없는 Phase 0의 의도된 비용, RQ6에서 실측.
5. 비동기 정리 타임아웃 시 강제 drop 의미론 미정 — 그 경로에서 구조적 동시성의 정상 취소
   보장이 약해진다.
6. 호스트발 제거의 명시 unmount API 계약이 지켜지지 않는 호스트(레거시 스크립트의 임의 DOM
   조작)에서는 좀비가 생길 수 있다 — MutationObserver 보강은 미설계.
   **2026-10-17 정정**: "미설계"는 낡았다. 2026-07-24 RQ6 §3.2 실측 라운드에서 `demo/resume/registry.js`에
   `watchForHostInitiatedRemoval()`(MutationObserver, `document.body` subtree 관찰)이 구현됐다. registry API를
   거치지 않은 `.remove()`도 detach 시퀀스를 발동시킨다는 것을 Playwright로 확인했다(RQ6-verification-plan.md
   §3.2 4번). 2026-07-25에는 M7 오탐 결함을 수리했고(§3.5), 3개 브라우저 재검증도 했다(§3.8 M10 (6)). 남은 한계는
   검증이 Island 팔 픽스처에 한정된다는 것이다. 실제 레거시 호스트에서 확인한 것은 아니다.
7. 크로스 임베드 DOM 중첩(05) 미정의.
8. Layer 2 제거 op의 Registry 조회가 컴파일타임 플래그로 얼마나 생략되는지 미실측 —
   worst case에는 patch 경로에 동기 조회 비용이 남는다.

## Phase 3 재검토 트리거 (이 규칙을 깨는 증거)

1. **RQ1 B/F 판정에서 F 승리** → frozen 상태 추가 + Q3 despawn 강제 조항 재개정.
2. **RQ6 벤치마크에서 상주 메모리 한계 초과** → hibernation 설계 강제(RQ2 클라이언트
   재직렬화 선결).
3. **failed island 빈도/UX 실측 미달** → restart 도입 = DOM 전량 파기+재스폰 설계 + 스냅샷
   갱신 규칙 필요.
4. **비동기 정리 타임아웃 초과가 실측에서 유의미** → 강제 drop 의미론 설계.
5. **panic 봉쇄 후보 전부 실패** → island별 인스턴스 분리 재검토(규칙 전면 재검토 수준).
   *상호 등록(2026-07-23, RQ6-verification-plan.md §9-4)*: RQ6 벤치마크가 이 결정을
   "Phase 3 착수 전"까지 필요로 함 — 이 시한까지 결정되지 않으면 RQ6은 조건 A(미해결)를
   기본값으로 채택해 Phase 3 측정을 진행한다(RQ6 조건 C 스파이크는 이 결정과 독립 병행 가능).
6. **호스트 주도 제거가 unmount API로 커버 안 되는 실사례** → MutationObserver 보강 설계.
7. **RQ4 진행 중 SSR prop 동결이 실무 불가 판정** → Q2의 통신 분리 재협상 (RQ4 문서의
   "둘이 독립적으로 풀리지 않는다" 경고의 현실화).
8. **Registry를 공유 상태 부착점으로 요구하는 RQ4 해결안** → 스키마 폐쇄 원칙 재협상.

**트리거별 현재 상태 (2026-10-17 확인 — 아래 "상태" 절의 2026-07-28 공지대로 전부 "세분화 선택 시에만
발동 가능")**:
1. **미발동.** RQ1 트랙 T v2 판정(2026-08-07)은 "B 유지, F-i/ii/iii 기각"이다. 다만 컴파일러가 실제 조건부에
   내는 코드는 lazy-spawn-then-hide(SetVisible 숨김)다. 이것은 F-i의 정상 상태를 상속한다(RQ1 트랙 T 절의
   2026-10-17 부기). Q3 "RQ1-F 조건화"가 요구한 "island host를 포함한 조건부는 despawn 의미론 강제"를
   컴파일러가 지키는지는 대조하지 않았다.
2. **발동 기록 없음.** RQ6 M4 실측(2026-07-25, RQ6-verification-plan.md §3.5)은 N=1/10/50에서 1280/1344/1728 KiB였다.
   §9-2 임계("N=50에서 순증 +50% 또는 절대 +5MB")와 N=1 대비로 비교하면 +35%·+448 KiB로 둘 다 미달이다.
   **이 산술 대조는 2026-10-17에 한 것이다.** RQ6 문서는 M4를 "Island 단독의 구조적 약점"으로만 적었고,
   임계 대조를 명시하지 않았다. "순증"의 기준점도 다시 확인하지 않았다.
3. **미측정.** failed island의 사용자 노출 정책은 여전히 미결이다(RQ6 §3.8 M10 (2)).
4. **해당 없음(현 구현).** `begin_detach`/`finish_detach`가 동기 즉시 호출이라 비동기 정리 자체가 없다(RQ6 §3.8 M10 (5)).
   알려진 약점 5(강제 drop 의미론)는 미결로 남아 있다.
5. **미발동.** 조건 A가 사전 등록 폴백으로 채택됐다. 조건 B는 실측으로 완전 봉쇄를 보였다. 조건 C는 nightly +
   build-std에서만 작동하고, stable에서는 2026-07-28 재확인 때도 뚫리지 않았다. "후보 전부 실패"가 아니다.
6. **처방 이행됨.** MutationObserver 보강이 구현됐다(알려진 약점 6의 2026-10-17 정정).
7·8. **기본 모델에서 성립하지 않음.** RQ4 전면 폐기(2026-07-28).

## 상태

잠정 규칙 확정(2026-07-23, Phase 0 산출물). 완전한 답이 아니라 "이렇게 가정하고
진행한다"는 명시적 결정이며, Phase 3까지 위 재검토 트리거에 따라 반복 개선한다.
연쇄 갱신: docs/06(승격 정책·2단계 detach 반영), Lean4 stub(ActorState 6상태 스펙 주석),
RQ1(F 조건화 접속점), RQ2(dormant 저장 위치 요구), RQ4(crash 통지 채널 이관).

**2026-07-24 (Phase 3) — 잠정 규칙이 실제 코드로 검증됨**: `crates/loom-runtime/src/
island.rs`(6상태 기계 + `IslandRegistryBackend` trait, native 테스트 11개 — Q1/Q2/Q3
2단계 detach/Q4 전부 실제 상태 전이로 확인) + `crates/loom-island-demo`(ProductCard
`client:visible` 안에 QuantityPicker `client:load` 중첩, 실제 `BrowserDom`) +
`demo/resume/registry.js`(JS 쪽 authoritative Registry — 이유는 아래) + Playwright
실브라우저 검증(Q1: quantity-picker가 즉시 attach되어도 product-card는 화면에 스크롤
되기 전까지 dormant로 남음 / Q3: "remove product card" 클릭이 중첩된 quantity-picker까지
cascade로 detached 전이시키고 DOM subtree가 실제로 사라짐). 재검토 트리거 8개 중 발동한
것 없음 — 잠정 규칙은 그대로 유지.

**핵심 발견(코드 문서에 위임)**: 권위 있는 Registry가 Rust 구조체일 수 없는 이유(서로 다른
청크 = 서로 다른 wasm 인스턴스 = 서로 다른 선형 메모리)는 `crates/loom-runtime/src/
island.rs`와 `demo/resume/registry.js` 양쪽의 모듈 문서에 상세 기술 — 이 발견 하나가
"Island당 별도 World"와 "Registry는 JS 쪽"을 동시에 결정함. docs/04에 "Layer 0 — Host
Glue" 개념으로 반영.

**2026-07-28 (Phase 6, Task #29) — 부분 폐기**: RQ6 §7 게이트가 H-Qwik을 확정(강한
형태)함에 따라 이 문서 최상단에 부분 폐기 공지를 추가했다. 재검토 트리거 8개는
이제 "잠정 규칙을 깨는 증거"가 아니라 "세분화를 선택했을 때만 발동 가능한
트리거"로 지위가 바뀐다(기본 모델에서는 발동 조건 자체가 성립하지 않음) — 단,
트리거 자체를 삭제하지 않는다(세분화가 선택되면 여전히 유효한 재검토 절차이므로).
연쇄 갱신: docs/04(전면 재작성), docs/06(IslandRoot 개념 제거, 일반화된 지연
activation 서브트리로 재기술), docs/07(Island 계열 용어 재정의), RQ4(전면
폐기 — cross-island 통신 문제 자체가 소거).

**2026-10-17 — 현행 확인(내용 변경 없음)**: 2026-07-28 이후 이 문서를 고친 커밋은 없다(`git log`). 그 뒤 이 RQ의 지위를
바꾼 사실도 찾지 못했다. 상태는 그대로 **부분 폐기**이고, 기본 모델에도 적용되는 부분(2단계 detach,
dormant/attached/failed, promote-once-and-stay/no-restart)은 여전히 **잠정(provisional)** 규칙이다. 이 규칙들은 Phase 3
픽스처에서 코드로 확인됐지만, 그 확인이 규칙을 "정식"으로 승격하지는 않았다. 정식 승격 경로(RQ6-verification-plan.md
§11의 "H-04 확정 시")는 H-Qwik 확정으로 발동하지 않았다. 이번에 확인하면서 덧붙인 것은 둘이다. 하나는 위 "알려진 약점"
6번의 정정(MutationObserver는 2026-07-24에 구현됨)이다. 다른 하나는 "재검토 트리거" 아래의 트리거별 현재 상태다. 아직
열린 것은 강제 drop 의미론(약점 5), failed island의 사용자 노출 정책(약점 2), 크로스 임베드 DOM 중첩(약점 7), Registry
조회의 컴파일타임 생략 실측(약점 8)이다. `formal-verification/lean4/SupervisionTree.lean`의 `ownedEntities`가 언제
실제로 비워지는지(despawn 시점)도 미모델링으로 남아 있다(그 파일의 TODO).
