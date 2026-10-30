# 로드맵

각 Phase는 이전 Phase의 "종료 조건"을 만족해야 시작. 건너뛰지 말 것 (CLAUDE.md).

## 현재 상태 요약 (2026-10-17 갱신)

> 이 절은 현재 상태 요약이라 갱신할 때마다 다시 쓴다(2026-10-17 작성). 아래 Phase별 절은
> **시간순 실행 기록**이라 지우지 않는다. 과거 서술이 지금 틀린 곳에는 날짜 붙은 정정을 달았다.
> 이 파일은 2026-08-13 이후 2026-10-17까지 갱신되지 않았다. 그 사이의 RQ1 진행(개정 #9 정정·후속,
> 개정 #10, v21·v22 캠페인)은 `research-questions/RQ1-benchmark-protocol.md` 끝부분과 판정문
> `research-questions/data/verdicts/`가 1차 출처이고, 여기에는 요약만 옮긴다.

| Phase | 상태 (2026-10-17) |
|---|---|
| 0 리서치 수렴 | 완료 2026-07-23 |
| 1 컴파일러 + No-VDOM | 종료 조건 충족 2026-07-23(스코프 축소 명시). **RQ1 트랙 L 판정은 아직 채택 아님**(아래) |
| 2 Resumability | 완료 2026-07-23~24(카운터·리스트·조건부·폼 입력). RQ2 본체는 열림 |
| 3 Islands | 실행 2026-07-24~25, 잔여 2026-07-28(Task #28). 2026-07-28에 "지연 activation 경계 세분화(필요시)"로 재해석 |
| 4 백엔드 어댑터 | 완료 2026-07-24. FIC `resumability_payload`는 2026-07-28 추가, `assets`는 여전히 빈 배열 |
| 5 공식 검증 | RQ5 후보 #1~#4 전부 이행(#4는 2026-08-11 LIS 건전성까지, `loom-flush-core` 90 verified). 미증명 잔여는 Phase 5 절 끝 |
| 6 RQ6 최종 검증 | **H-Qwik 확정(강한 형태)** 2026-07-28. §7 (a)를 반사실 조항으로 읽는 해석(병익님 확정)에 기댄다 |

**RQ 상태**: RQ1 열림 · RQ2 열림(잠정 포맷만) · RQ3 부분 폐기 · RQ4 전면 폐기 · RQ5 후보 #1~#4 이행 ·
RQ6 확정. (RQ5 문서 후보 표 #4 행에도 2026-10-17 갱신이 붙어 2026-08-11 완료로 정리됐다.)

**RQ1 트랙 L 경과 (2026-08-12 → 2026-10-17)**:
1. 개정 #9(2026-08-12): 조항 대 하네스 전수 대조에서 R2 축 불일치·write 변형 이중 측정 부재 등이 나와
   v1~v18·v19-r3·v20 전 수집분 무효. 이어 2026-08-13~09-25에 정정 2·3과 후속 등재가 붙었다(R2 중첩
   조건부가 작동하지 않던 것, K의 R2 조건부 미물질화, 판정기의 "데이터 부재를 결론으로 세기" 등).
2. v21(2026-09-26~27): 개정 #9 이후 첫 완전 판정. 결론 C 채택 불가·D 기각 (iii) 불성립+양성 충족+W5
   부속·E 기각 미발동. 적대적 검증은 판정 변경 0건이었지만 미이행 3건과 셀 간 짝 표류를 드러냈다.
   트랙 N G2는 v21 기반에서 R1·R2 재인증 통과(8204f61).
3. 개정 #10(2026-09-27): 두 write 변형을 같은 셀에서 측정, GC 통제(M-15), W7, rep별 단계 시간 →
   **v22로 전면 재수집**. 후속 2: 렌더러 재사용이 셀 간 오염을 일으켜(K에 불리한 방향) **셀마다 새
   브라우저**로 수집하도록 바꾸고 첫 v22 수집(23/58)은 폐기. **v21 판정은 "v22와 대조해 재검토할
   대상"**으로 등재(자동 무효는 아님).
4. v22 결과(판정문 `rq1-track-l-v22-*.txt`):
   - Chromium 비스로틀 58/58셀(eec14bf), 무결성 문제 0건, GC 통제 58/58. **판정(f6c1bdb)은 v21과
     같다** — C 채택 불가(기각 미발동), D 기각 (iii) 불성립·양성 충족·W5 부속 발동, E 기각 미발동.
     C 기각 앞 분기 하한 2.882(v21 2.630)로 임계 3에 가까워졌다.
   - Chromium 4× 스로틀 12/12그룹(70cc5a9, d8325ba): **C 기각 앞 분기가 스로틀에서만 발동**(W2×R2
     C/D 하한 3.162 ≥ 3 — d8325ba 판정문 값. Firefox 절을 함께 돌린 694d274 재실행은 부트스트랩
     난수열 위치가 달라 3.163). 게이트 결론이 갈린 요소 1개 — §8에 따라 판정은 비스로틀 그대로이고
     §11-1(b) 검토 대상 등재 사유다.
   - Firefox 교차 확인 16그룹(판정 관여 10 + 스케일링 6, 1d85c4f, 판정문 694d274)은 **GC 통제 없이**
     수집됐다. 게이트 결론은 Chromium과 같지만 우세 방향이 정반대인 쌍이 4개(W2×R1 K/D-LIS·D-LIS/K·
     E-LIS/K, W3×R1 C/D-LIS) — §11-1(b) 등재 사유. 그 밖에 배치 모드가 브라우저 간에 달라 방향 대조가
     같은 조건의 비교가 아닌 판정 짝 15건(W1×R2·W4×R1·W4×R2)은 "비교 불가"로 빠졌다.
   - **2026-10-17 보류(병익님 결정)**: G0 결과(D·E 조문상 기각) 때문에 아래 재수집을 당분간 멈춘다 — 수리 후 재심사로 가면 새 기반 캠페인에 포함, 확정이면 현 기반으로 재개(프로토콜 말미 "Firefox BiDi 재수집 보류").
   - **WebDriver BiDi로 GC 통제를 붙인 Firefox 재수집은 대기 중**(개정 #10 후속 3, 2026-10-17 등재,
     구현과 수집 전 검토 2회 완료). 다른 작업과의 CPU 경합 때문에 병익님 결정으로 미뤘고, 첫 단계는
     무부하 그룹 11 단독 스모크다. 정본 이름은 `v22-firefox-bidi-g<g>`이고 1d85c4f 수집분은 보존한다.
5. **채택까지 남은 것**(판정문·프로토콜 기준, 2026-10-17 확인):
   - D·E의 G0(No-VDOM 구현 재심사, docs/03 판별 기준) 심사 기록과 G1(트랙별 정합성 게이트)의 v22 기반
     판정 기록이 없다. 판정기는 G0~G2 상태를 입력으로 받지 않는다(G2만 v21 기반 재인증 8204f61, "G0 심사
     기록·G1은 범위 밖").
     **2026-10-17 G0 실행**: D·D-LIS·E·E-LIS 모두 조문상 **기각**(P1: `String` key 비교, P3: `match_keys` Θ(n·m)) — "VDOM이다"가 아니라 HEAD 구현 변형이 형식 요건을 못 채운 것. 확정 vs 수리 후 재심사는 병익님 확인 대기. 상세: RQ1 문서 끝 "G0 — D·E No-VDOM 구현 재심사 기록".
   - D 기각 (iii) 뒷항(M4/M7/M3) 미계측, D 채택 시 W5 부속 조건(unkeyed opt-out 명세).
   - C 판정은 C-4 기준선 규칙 때문에 D·E의 자격에 종속된다(D·E가 모두 탈락하면 C 채택 (a)가 충족으로 뒤집힘).
   - D per-row 변형·트랙 R S1은 "v22 이후 캠페인", §7 M5 트랙 L 메모리 셀·WebKit 열·Chromium 폴백 강제는
     미이행 등재만 돼 있다.
   - 스로틀 C 앞 분기와 Firefox 방향 뒤집힘의 §11-1(b) 등재(프로토콜 말미, 2026-10-17)와 RQ1 본문 v22 절
     ("v22 — 개정 #10 이후 전면 재수집 판정", "채택까지 남은 것")은 작성됐다. 두 검토 대상의 검토와 셀 단위
     v21↔v22 대조는 아직 하지 않았다(Firefox 4쌍 검토는 BiDi 재수집 결과와 함께 보기로 함).
   - §5 FV 계류: 2026-08-11에 소진(Verus 90 verified). RQ1 본문과 프로토콜에 남아 있던 "남은 조건" 표류는
     2026-10-17 날짜 붙은 정정으로 정리됐다. 판정기 고정 출력 "※ 범위 밖: §5 FV 계류"
     (`analysis/rq1-verdicts.py` 809·932행)만 다음 판정기 개정 때 고칠 항목으로 남았다(판정 결론 영향 없음).

**트랙 T**: v2 캠페인(2026-08-07) 판정 B 유지·F 계열 기각, 그 뒤 변동 없음. 재수집 때 할 항목으로
GC 통제(M-15), B arm 중복 `set_text` 수리, J-a on/off 재정의(v3 사전 등록 후보)가 등재돼 있다.
**트랙 N**: 사이클 1(2026-08-08) G2 통과 + v21 기반 재인증(2026-09-27). 불변식 4는 여전히 약한 형태만.
**트랙 R**: 프로토콜 §6에 정의만 있고 실행 기록 없음.

**아직 결정 기록이 없는 것**: 컴파일러가 내는 조건부는 "첫 spawn 지연 + CSS 은닉(`SetVisible`, 숨긴 동안
patch 게이팅 없음)"으로, 정상 상태 토글은 트랙 T의 CSS 은닉 대조 arm과 같은 알고리즘이다(SSR-false 사이트의 첫 spawn 지연은 lazy arm의 앞 절반, F 계열 아님). 트랙 T 판정은 B 유지이고
CSS 은닉은 대조군이라(원칙 5) 판정이 채택하지 않았다 — §9 K-3은 "CSS가 F를 지배하더라도 자동 채택은 아니다"라는 보강 조항이고, v2에서는 그 지배도 깨졌다(K-3 (1) 미발동) — 의미론 반박 답변 기록도, B 전환 결정도 없다. 별개로 트랙 N 불변식 4의
진짜 형태는 F 계열(fragment 분리 + `Frozen`) 출력을 요구한다. (2026-10-17 재정정: 같은 날 첫 판은 이것을 "F 계열/F 기반"이라 적었다. 코드젠에는 F 계열의 장치 — fragment 분리, `CondEffective`, `Frozen` 게이팅, catch-up — 가 하나도 없고, `cond.rs`가 "F"라고 부르는 `cond_reconcile_f_system`은 스스로 "CSS-hide preserve variant"이자 프로토콜의 CSS 은닉 대조 arm이라고 밝힌다. Phase 1(07-24) 시절의 이름표를 그대로 옮긴 오류였다.) PatchTable(opcode) 전환도 Phase 1 이래 이행되지 않았다(docs/07) — **2026-10-17 병익님 결정으로 종결**: 제네릭
System + 닫힌 열거형이 정본이고 opcode는 재개 조건부 대안(docs/02 "PatchTable 결정 (2026-10-17)").

## Phase 7 — 저작 표면 완성 (2026-10-17 신설, 병익님 결정: 옵션 3)

컴파일러는 연구용 좁은 부분집합이고 조용한 오컴파일 47건이 있다(`docs/09-authoring-surface-inventory.md`). 슬라이스:
- **7a 위험 수리**: 조용한 항목을 명시적 거부로, `Result` + SWC span 진단 도입. 컴파일러·데모 JS만. 산출물 바이트 동일이면 재인증
  불필요(RQ1 프로토콜 개정 #11). 첫 단계였던 소비 픽스처 전수 대조는 2026-10-17 완료 — 히트 확인, 연구 산출물 재검토 수행(무효 없음).
  - **7a 상태 (2026-10-17, 브랜치 phase7a C0~C10 — main 병합 전)**: 컴파일러 쪽 완료, **데모 JS(R13·R14)는 병익님 결정 대기**.
    - 한 일: 바이트 동일 게이트 하네스(`tools/compiler-gate/`), `Result` + swc_common span 진단(docs/02 "진단" 절), 새 거부
      R01–R08·R07U·R12(리스트)·R15·R16·R18·R19·R22–R27·R44·R46·R47·G01 + 경고 W25, R12 텍스트 직계 resume 수정(codegen 유일 변경).
      행별 내용: docs/09 "2026-10-17 갱신(7a)".
    - 게이트: C0~C9 커밋마다 BASE=부모로 A·B·D(C0·C1은 C 포함) 전부 PASS, 기록 JSON을 각 커밋에 동봉, 커밋 후 트리 대조 10/10 OK.
      7a 끝 최종 실행(BASE=da4a9bc, A·B·C·D, base2 재현성 포함) 결과는 RQ1 프로토콜 "개정 #11 적용 기록 — 7a". 소비자 22회 호출 중
      새 거부에 걸린 것 0건 — 정지 규칙(연구 산출물 재검토 먼저) 미발동.
    - **범위 변경(날짜 기록)**: (1) **R13·R14(`demo/resume/bootstrap.js`) — 미처리, 병익님 결정 대기.** 7a 정의("컴파일러·데모 JS")에
      들어가지만, 개정 #11 조건 1이 "bootstrap JS"를 산출물로 열거한다는 문언 해석이 걸려 있어 설계의 조건부 커밋 C11을 만들지 않았다.
      선택지는 (i) 7a에 C11로 포함(D 허용 목록 예외 1건, 전/후 sha 기록 — 측정판은 16c7e10이고 49b8660에서 이미 바뀌어 현 측정 산출물이
      아님) 또는 (ii) 7b로 이동. 설계 권고는 (i). (2) **R12 직계 리스트의 지원 → 7b**(7a는 거부 LOOM-R12; 이중 adopt와 빈 리스트
      열림 판정을 함께 설계해야 함). (3) R12 텍스트의 영구 회귀 크레이트 → 7b(Cargo.lock 변경이 게이트 D에 걸림; 7a는 미커밋 스크래치
      실측). 회귀 테스트는 텍스트 노드를 adopt하는 DOM 대역 위에서 돌려야 한다(`TreeDom`은 텍스트를 adopt하지 않아 이 결함을 못 잡음).
      (4) W25 원인 소스(widget-embed.tsx import, largeapp 합성 소스) 정리 → 7b(RQ6 픽스처 소스 변경).
    - **7a 후속 F2·F3 (2026-10-18)**: R25 완화(빈 문장 `;` 허용 — 최상위·본문·return 뒤, 산출물 바이트 동일 단언). C11(R13·R14,
      병익님 승인으로 7a에 포함): `demo/resume/bootstrap.js` 청크당 인스턴스 하나 + input 클릭 제외, Playwright 재검증(음성 대조 포함) 통과.
      7a의 C11 "결정 대기"는 해소됐다.
    - **7a 후속 F1 (2026-10-18)**: R46 소유자 위임 결정. 최종 형태는 signal 이름 = Rust 식별자 규칙(XID) + NFC 강제(한글·`café` 등 받음),
      키워드 `r#`·`$` 맹글링(Rust 쪽만), 컴포넌트 이름 = ASCII(wasm32 E0754). 판정용 의존성 추가가 소비자 OUT_DIR 경로(→ wasm 바이트)를
      바꿔 7a 게이트가 FAIL했으므로 일반 판정은 **7b**로 옮기고, 7a에서는 진단 문구를 원인별로 사실대로 고치고(비ASCII signal = "7b 지원 예정")
      컴포넌트 키워드 검사를 걷었다. 상세: docs/09 "R46 정정", RQ1 프로토콜 "개정 #11 적용 기록 — 7a 후속".
- **7b 위젯 실사용 최소 표면**(컴파일러 전용): 정적 속성, 삼항, 액션 인자, Str/Bool 텍스트, JSX 공백 규칙, Flatten input 수리. 기존 산출물을
  바꾸는 묶음(공백 규칙·bootstrap-flat)은 RQ6 재측정 1회에 모은다.
- **7c 런타임 표면**: `DomWrite` 추가(속성·class·checked·SVG 네임스페이스·`SpawnRow` tag/class 등). §2.5 동결을 깨므로 **G0 수리 캠페인의 새
  동결 기반에 합류**(옵션 3). 새 variant 추가만 하고 커밋을 분리한다.
  - **2026-10-26 구현됨(런타임 표면만)**: 브랜치 `g0-repair` 커밋 2779566 — `DomWrite::Surface`(마지막 variant 하나), `surface.rs`, `DomOps` 신규
    메서드 8개(기본 `unimplemented!`), `SurfaceOpCounts`(`DomOpCounts`는 불변), `BrowserDom` 구현, 사이트 마커 patch System. 소비 codegen은 7b.
    `size_of::<DomWrite>()` 88(x86_64) 불변, 기존 판별값 불변. 설계 원문 `roadmap/phase7c-runtime-surface-design.md`, 행별 결과 docs/09
    "2026-10-26 갱신(7c)". 함수 단위 B0/B1 diff 기록(7c-7)과 브라우저 프로브(Chromium·Firefox, 2026-10-30)는 완료 — docs/09. 혼합 텍스트 SSR 마커 규약 v0(B2, 주석 `<!--lm-->`)은 2026-10-30 확정(docs/09). G0 재심사(C12)는 2026-10-30 실행·병익님 잠정 판정으로 D·D-LIS·E·E-LIS 통과(자격만), C 최선형 수리 둘, D prefix/suffix 절단은 명세에서 제외(개정 #12 후속 4). 판정기 사전 점검은 2026-10-30 완료 — 판정 기울어짐 없음(개정 #12 후속 5). 동결(C13)은 2026-10-30 기록 — 기반 `2fa7779`, P1 `56495615…`·P2=최종 `b509135f…`, 빈 target 빌드로 재현 확인(개정 #12 후속 6; 그 과정에서 기존 `target/`의 툴체인 경로 오염과 `rust-src` 원인을 발견). 같은 날 `rust-src`를 1.99.1에 설치한 뒤 `target/`를 지우고 재점검 — 동결 빌드에 경로 remap을 넣어 `rust-src` 유무와 무관하게 P1·P2 해시 재현(개정 #12 후속 7). **남은 것**: WebKit 프로브, G1, 트랙 N G2 재인증, 수리 소요 시간 기록, A/A 드라이버 — 그다음 v23 수집(병익님 지시 후).
- **7d 결정 의존 표면**: props/children(CAP를 RQ로 먼저), key(G0 P1 이후), 폼 이벤트, effect·비동기, 동적 컴포넌트(PatchTable 재개 조건).
- 순서: 7a → (G0 수리 결정 시 7c를 그 기반에 합류) → 7b → 7d. 제약: No-VDOM, Component에 클로저 금지(핸들러는 생성된 `Action` 정적 `match`),
  다른 프레임워크 API 복사 금지.

## Phase 0 — 리서치 수렴 (완료 2026-07-23)

> **2026-10-17 정정**: 이 제목은 초판 이래 "(지금 여기)"로 남아 있었다. 아래 세 항목이 2026-07-23에
> 전부 이행돼 종료 조건이 충족됐고, Phase 1~6이 그 뒤 실행됐으므로 현재 위치 표식을 "완료"로 바꾼다.

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
- JSX → Entity graph + PatchTable 컴파일 (02) *(2026-10-17: 실제는 제네릭 System + 닫힌 열거형 — 그것이 정본으로 결정됨, docs/02)*
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
     (**2026-10-17 주기**: 이 전환은 Phase 2~6을 거치는 동안 이행되지 않았다. docs/07 용어표는
     PatchTable/DomBinding을 "실제로는 미구현"으로 적고, 현재 코드젠도 제네릭 System을 직접
     등록한다. 전환 여부를 정한 결정 기록은 없다 — 열린 항목.)
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
     **2026-08-07 갱신 — 트랙 T 최종 판정**: §4.1 완전 준수 v2 캠페인(6 arm ×
     통상 구간 × 변경률 축 × 4× 스로틀 × catch-up × chain3, 44시간, 13파일)에
     §9 B/F 전 조항을 기계 적용한 결과 **"또는" 분기 발동 → B 유지, F 계열
     (F-i/ii/iii) 기각**. 1차 분기(승리 지도)는 세 변형 모두 생존했으나(각각
     9셀 중 4~5셀에서 B를 J-a·J-b 모두 우세로 이김), B의 spawn 비용이 양쪽
     스로틀 조건 모두 1ms/100 entity 아래(0.26~0.81ms)라 사전 등록 문언대로
     B 유지다. 2026-07-28의 미발동 판정은 노드 슬롯 누수 하네스에서 나온
     것으로 §11-1(b) 무효 표기(프로토콜 개정 #7). 승리 지도와 임계 분기가
     반대 방향을 가리킨 사실은 §11-3 탐색적 관찰로 병기 — 새 기준은 새 사전
     등록(v2)으로만. B의 상태 소실 의미론은 원칙 6대로 별도(M8). 상세는 RQ1
     "트랙 T v2 캠페인" 절.

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

> **⚠️ 2026-08-12 — 위 문단들의 트랙 L 판정은 §11-2 개정 #9로 전부 무효다.**
> 조항 대 하네스 전수 대조에서 미이행 열두 건이 나왔고, 그중 **R2 축이 조항
> 원문("binding 8개 + 중첩 조건부 1개")이 아니라 반응하지 않는 정적 텍스트
> 자식 8개**였다 — §9의 D 기각 (iii)·D 채택 양성 조건·E 기각 (i)이 **전부 R2
> 셀에 걸려 있다.** 게다가 write 변형 이중 측정이 전면 부재해 §9 서두의 "셀별
> 우세 write 변형" 규정이 작동할 수 없었고 C가 자기 최선형으로 측정되지 않았다.
> 트랙 L 전 수집분(v1~v18 + v19-r3 + v20)을 무효 표기하고 재수집 중이다.
> 위 서술을 지우지 않는 것은 §11-2가 개정 이력 누적을 요구하기 때문이다.
> 상세와 공평성 사전 등재: `research-questions/RQ1-benchmark-protocol.md`의
> "개정 #9"와 그 정정·후속 절.
>
> **트랙 T 판정(B 유지, F 계열 기각)은 영향 없다** — 별도 격자·별도 드라이버다.
> 트랙 N 사이클 1과 §5 FV 계류 소진도 영향 없다.
>
> **2026-10-17 갱신 — 위 "재수집 중"의 경과.** 재수집은 v21(2026-09-26~27)로 한 번 완주했고, 그 뒤
> 개정 #10(2026-09-27)이 측정 방법(같은 셀 write 변형 짝·GC 통제 M-15·W7·rep별 단계 시간)을 고쳐
> **v22로 전면 재수집**했다. 개정 #10 후속 2가 셀 간 오염(렌더러 재사용)을 찾아 셀마다 새 브라우저로
> 바꿨고, v21 판정은 v22와 대조해 재검토할 대상으로 등재됐다. v22 Chromium 비스로틀 58/58셀 판정은
> v21과 같다 — **C 채택 불가(기각 미발동), D 기각 (iii) 불성립·양성 충족·W5 부속 발동, E 기각
> 미발동**. 즉 위 2026-07-24 서술의 "D/E 조건부 채택"은 지금도 그대로 읽으면 안 된다 — 성능 게이트
> 결론은 D·E에 유리하게 나왔지만 G0 심사 기록·D 기각 (iii) 뒷항 계측·W5 부속 명세 등이 남아 있어
> **채택 판정이 아니다**. 4× 스로틀에서는 C 기각 앞 분기가 발동했고(판정은 비스로틀 그대로),
> GC 통제 없는 Firefox 교차 확인에서 우세 방향이 정반대인 쌍 4개가 나왔으며, GC 통제를 붙인
> Firefox(BiDi) 재수집은 대기 중이다. 요약과 채택까지 남은 것은 이 파일 맨 위 "현재 상태 요약",
> 원문은 `RQ1-benchmark-protocol.md`의 개정 #10과 후속 1~3, 판정문 `research-questions/data/verdicts/
> rq1-track-l-v22-*.txt`.
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
(**2026-10-17 주기**: 실제로 계측된 것은 "페이지 로드 시 지연 청크(.js/.wasm) 요청 0건"이다. 즉시
로드분 `bootstrap.js`는 로드 직후 실행돼 전역 위임 리스너를 등록하므로 "client JS 0 실행"을 글자
그대로 읽으면 안 된다 — 아래 §3.1 실행 기록도 "상호작용 전 실행 거의 0"이라고 쓴다.)

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
  (**2026-10-17 주기**: 이 절과 아래 두 곳의 "RQ6 재검토 안 함"은 시간순 기록이다. 재검토는
  2026-07-24 이 Phase 마지막 라운드("여기서 RQ6 재검토" 실행 절, §3.1)에서 이행됐고, RQ6
  자체는 2026-07-28 Phase 6에서 확정됐다.)

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
  *(2026-10-17 부기) 이 순서는 매 단계가 그 요소의 첫 클릭이라 bootstrap.js의 요소별 인스턴스 결함(docs/09 R13)을 원리적으로 잡지 못한다 — 공허한 확인. 코드 추적상 다른 요소를 처음 누르면 count가 0으로 되돌아가고 닫힌 조건부가 다시 열린다(브라우저 미재현). R13 수리 뒤 "상태 변경 → 다른 요소 첫 클릭" 순서로 재검증한다.*
  *(2026-10-18 재검증 완료 — 7a 후속 F3/C11에서 bootstrap.js를 청크당 인스턴스 하나로 고친 뒤 Playwright로 "상태 변경 → 다른 요소 첫 클릭"과 "input 입력 후 클릭"을 확인했다: 새 판 통과, 수리 전 판은 count 0 리셋·materialize 4회·입력값 소거로 결함 재현. `research-questions/data/phase7a-followup-F3-resume-reverify-2026-10-18.json`)*

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
  (2026-10-17 주: 이 절 제목의 "RQ1 후보 확정"은 트랙 L(D/E 조건부 채택, 개정 #9로 무효)을 가리킨다 —
  조건부(트랙 T) 후보가 확정된 적은 없다.)
  (2026-10-17 주: 여기 "F"는 Phase 1 이름표다 — `cond_reconcile_f_system`은 CSS 은닉 보존 변형이고,
  F 계열 F-i/ii/iii는 2026-07-31에 따로 생겼다. 컴파일러 산출물은 프로토콜의 CSS 은닉 arm에 해당한다.)
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
- **의도적 스코프 제한(숨기지 않음)** — **2026-08-07 부분 해소, 아래 "조건부
  내용의 중첩 지원" 절 참고**: `then_branch`는 정적 텍스트 자식만 허용
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
  *(2026-10-17 부기) input 클릭은 검증하지 않았다 — 클릭하면 값이 지워진다(docs/09 R14). "무관한 +1 버튼의 독립적 재개"는 R13 동작 그 자체다.*
  *(2026-10-18: R14도 같은 커밋에서 고쳤다 — click 위임이 input을 건너뛴다. 재검증 결과는 위 부기와 같은 파일.)*

## Phase 3 — Islands (2026-07-28 재해석: "지연 activation 경계 세분화(필요시)")

> **2026-07-28 (Phase 6, Task #29) 재해석**: RQ6 §7 게이트가 H-Qwik을 확정
> (강한 형태)함에 따라, 이 Phase가 "필수 구현 대상"으로 프레이밍했던 Island가
> 기본 모델에서는 필수가 아니게 됐다(docs/04 참고) — "Islands 프레이밍"을
> "지연 activation 경계를 (필요하면) 세분화하는 선택적 최적화"로 재해석한다.
> 아래 실행 기록은 수정하지 않는다(전부 실제로 구현·검증된 유효한 산출물 —
> `crates/loom-widget-embed-island-*` 등 실제 코드가 지금도 이 산출물에
> 의존함) — 다만 "이게 없으면 임베딩이 안 된다"는 원래의 전제는 재해석됨을
> 여기 명시한다.

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
2. ~~컴파일러 통합(증분 4)~~ → **해소됨(2026-07-24, Task #25)** — 아래 "Task #25
   실행 상태" 절 참고.
3. `sharedSignal()` 저작 표면(증분 5, 스트레치) — 미착수.
4. crash 통지 채널(RQ4로 이관된 요구) — 값 공유만 구현, 상태 변화 통지는 미착수.
5. 비동기 정리(구조적 동시성 취소)의 실제 타임아웃 의미론 — 이번 `begin_detach`/
   `finish_detach`는 동기 즉시 호출(RQ3의 "알려진 약점" #5, 미결로 남김).
6. 리스트 row 안의 Island N개 인스턴스, 크로스 임베드 DOM 중첩 — 계획대로 범위 밖.

> **2026-10-17 정정 — 위 목록의 현재 상태** (이 목록은 2026-07-24 시점이고 그 뒤 Task #25~#28이
> 아래 별도 절에 기록돼 있다):
> - 1번(Island당 별도 `.wasm`): **부분 해소**. Task #27(2026-07-25)이 `generate_single_island_client`와
>   `crates/loom-widget-embed-island-*` 4개 크레이트로 위젯-임베드 픽스처에서 실제 per-Island 분리
>   빌드를 했다. `loom-island-demo`(이 절의 손코딩 데모)와 `loom-fic`은 여전히 단일 산출물이고,
>   청크 간 공유 vendor 모듈(런타임 중복 제거)은 미구현이다.
> - 3번(`sharedSignal()`): **해소** — Task #26(2026-07-24)에서 앞당겨 구현.
> - 4번(crash 통지 채널): **해소** — Task #28-b(2026-07-28), `window.__loomShared`의
>   `__island_state:<id>` 게시. RQ4는 2026-07-28 전면 폐기됐지만 이 채널은 유지된다.
> - 5번(비동기 정리 타임아웃 의미론): **여전히 열림**(docs/06 "미해결", RQ3 알려진 약점 #5).
> - 2026-07-28 RQ6 확정 이후 이 목록 전체는 "세분화를 선택했을 때"의 과제로 지위가 낮아졌다(위 재해석 공지).

## Task #25 실행 상태 (2026-07-24) — 컴파일러 Island 통합 (증분 4)

`ir.rs`에 `ComponentGraph`/`IslandActivation`/`PropLiteral`/`JsxNode::ComponentRef`
추가. `lower.rs`가 이제 모듈 안의 모든 top-level 함수를 스캔(기존엔 첫 export만 쓰고
나머지를 진단 없이 버리는 실제 버그가 있었음 — 이번에 같이 고침)하고
`client:load`/`client:visible`(SWC가 이미 `JSXNamespacedName`으로 올바르게 파싱하던
것을 `lower.rs`가 그동안 무조건 거부하고 있었을 뿐 — 파서 문제가 아니었음)을 파싱.
`codegen.rs`의 `generate_islands`가 Island로 지정된 각 컴포넌트를
`pub mod <name>_generated { ... }`로 네임스페이싱(컴파일러가 내는 모든 식별자가
원래 전역 스코프였다는, 문서에 없던 진짜 블로커였음 — 루트 컴포넌트는 하위호환 위해
계속 무네임스페이스), 호스트 엘리먼트에 `data-loom-island`/`-island-id`/`-activation`
/`-chunk`/`-ctor` 5개 속성 방출. Island 진입점(`#[wasm_bindgen(constructor)]`)은
기존 `Action`/`apply_action`/`resume_action_by_symbol`/`resume` 범용 디스패치
기계를 그대로 재사용(손코딩 Island처럼 별도 클로저 배선 방식을 새로 만들지 않음)하되
DOM 탐색은 (새로 `loom-runtime::web`으로 승격한) `own_tagged_descendants`/`find_own`
/`index_own_tagged_descendants`/`own_tagged_lref_descendants`로 opaque
`[data-loom-island]` 경계를 존중하도록 스코프를 좁힘 — `ResumedComponent::new`의
페이지 전역 쿼리 버그를 재생산하지 않음.

**실제 컴파일 중 발견한 진짜 버그 세 개(가정 아님)**:
1. 2단계 이상 중첩된 Island 참조(예: App → ProductCard → Widget)는 부모의 참조
   호출에 `super::` 경로 접두사가 필요함 — 모든 Island 모듈이 JSX 중첩 깊이와
   무관하게 flat sibling으로 생성되기 때문(`generate_islands`가 `ComponentGraph::
   components`를 평평하게 순회). 첫 시도(단일 레벨 중첩)에서는 안 드러나고, 2단계
   중첩 회귀 테스트를 직접 추가해서야 발견됨 — `CodegenCtx::wrapped_in_own_module`
   플래그로 해결.
2. 액션이 0개인 컴포넌트(`ComponentRef`만 있는 루트처럼)의 `enum Action {}`에 대해
   `match action {}`이 Rust에서 예외적으로 exhaustive로 인정 안 됨(rustc 자체 진단:
   "references are always considered inhabited") — 이전까지 모든 컴파일 대상
   컴포넌트가 액션을 최소 1개씩 가져서 한 번도 안 걸렸던, Island가 처음으로 드러낸
   기존 `apply_action` 생성 로직의 진짜 갭. `_ => {}` 폴백 arm 추가로 해결.
3. Island 진입점의 `find_node` 클로저가 `dom.adopt(...)`를 지연 호출하도록 짰더니
   `resume(&mut app, &mut dom, &mut find_node, ...)` 호출에서 `dom`이 이중 가변
   차용되는 진짜 borrow 오류 — `ResumedComponent::new`처럼 태그된 엘리먼트를
   `NodeHandle`로 미리(eagerly) 전부 adopt해두고 클로저는 그 결과만 불변 참조하도록
   고쳐 해결.

**실제 end-to-end 검증**: `demo/islands/product-card.tsx`(ProductCard `client:visible`
가 Widget `client:load`를 중첩) + 신규 크레이트 `crates/loom-island-compiled`(build.rs가
`loom_compiler::compile_tsx_to_rust_graph(..., GraphMode::Islands)` 호출) +
`demo/islands/compiled/index.html`(기존 `demo/islands/index.html`과 나란히, 대체
아님) — 기존 손코딩 데모의 시나리오 체크리스트(로드 시 활성화, 화면 밖에서 dormant
유지, 스크롤 시 활성화, opaque 스코핑, "show details" 실제 정적 콘텐츠, detach 후
DOM 제거)를 Playwright로 전부 재현 확인(8/8 통과), 3개 회귀 스크립트(원본 Island
데모, condition B 스파이크) 전부 무회귀 재확인, `cargo test --workspace` 전체 통과.

**의도적으로 남긴 범위(숨기지 않음)**:
- `GraphMode::Flatten`(H-Qwik 반사실 모양, RQ6 §8 item4) — 시그니처만 있고 미구현,
  실제 구현은 Task #26 몫.
- Island당 별도 `.wasm` 산출물 빌드 툴링 — 여전히 미해결. 이번 acceptance 크레이트도
  ProductCard/Widget 둘 다 같은 크레이트에서 나오므로, `demo/islands/compiled/
  index.html`에서 두 `data-loom-chunk` 값(`product_card`/`widget`)은 **같은 크레이트를
  두 번 `wasm-pack build --out-name`한 결과**(내용은 완전히 같은 두 개의 진짜로 동작하는
  wasm 모듈, 각각 안 쓰는 쪽 Island 구조체도 딸려 들어감) — 진짜 "1 Island = 1 빌드
  산출물"은 여전히 아님.
- `PropLiteral` 실제 resolve(`specialize_component_refs`, 인스턴스별 특수화)는 파싱/
  검증(활성화 지시어 필수, 알 수 없는 `client:` 값 거부 등)만 되고 실제 override
  구현은 스트레치로 남김 — 인수 조건 픽스처가 props 없이도 충족되므로.
- `sharedSignal()`/RQ4 cross-widget(`cartCount`) — Task #28-a 몫, 이번 acceptance
  픽스처는 이 부분을 의도적으로 뺐음(`demo/islands/compiled/index.html`의 스코프
  노트 참고).

(**2026-10-17 주기**: `GraphMode::Flatten`과 `sharedSignal()`은 바로 다음 Task #26(같은 날)에서
구현됐다. Island당 별도 산출물은 Task #27에서 위젯-임베드 픽스처 한정으로 해소됐다. `PropLiteral`
resolve는 2026-10-17 현재도 스트레치로 남아 있다(docs/02 "미해결").)

`docs/02-compiler-design.md`(미해결 절), `docs/07-naming-and-terminology.md`
("ComponentRef" 용어 충돌 정리)도 같이 갱신함.

## Task #26 실행 상태 (2026-07-24) — H-Qwik 위젯-임베드 반사실 픽스처

`codegen.rs`에 `GraphMode::Flatten`을 실제로 구현(`flatten_graph` — JSX 트리 레벨
에서 모든 `ComponentRef`를 재귀적으로 인라인해 그래프 전체를 하나의 `Component`로
합침, 임의 중첩 깊이 자동 처리). §4.2가 사전등록한 **5-컴포넌트 전체 위젯-임베드
픽스처**(`demo/widget-embed.tsx`: CartBadge/QuantityPicker/ProductCard/
CheckoutForm/MiniDashboard)를 `crates/loom-widget-embed`가 `GraphMode::Islands`/
`GraphMode::Flatten` 양쪽으로 컴파일 — RQ6 §4.1 "정체성 통제" 원칙을 문자 그대로
충족(별도 손코딩 아님). CartBadge를 동작시키기 위해 `sharedSignal()`/
`sharedValue()`(원래 Task #28-a 몫)를 이번에 앞당겨 구현.

**최초엔 5-컴포넌트 중 2개(ProductCard/Widget)만 재사용하고 나머지를 Task #27로
미루려 했으나, 이는 승인 없이 임의로 축소한 것이었음 — 병익님 지적 후 정정,
5-컴포넌트 전체를 이번에 완성함.** 상세는
`research-questions/RQ6-verification-plan.md`의 "§3.4 실행" 절(정정 내용 포함).

**실제 컴파일 중 발견한 진짜 버그 두 개**(CheckoutForm의 3-conditional 구성이
처음으로 노출시킴): (1) 한 컴포넌트에 조건부가 2개 이상이면 `cond_reconcile_
site_N`들이 구분자 없는 동일 쿼리 모양을 공유해 서로 크로스와이어됨(하나만
토글해도 모든 사이트가 반응) — 사이트별 `CondSiteN` 마커 Component +
`With<CondSiteN>` 쿼리 필터로 해결. (2) 그 수정의 1차 시도 자체가 마커를
`setup()` 로컬 스코프에 선언해 `resume()`/최상위 리컨실 함수에서 안 보이는
버그였음 — 최상위 스코프로 옮기고 `generate_resume`의 Bool arm도 같은 마커를
붙이도록 수정. 둘 다 유닛 테스트로 회귀 고정.

Playwright 실브라우저 검증(16개 시나리오 전부 통과, RQ4 cross-widget 포함),
Task #25 기존 8개 시나리오 무회귀, `cargo test --workspace` 전체 통과.

**여전히 남은 것**: 시그널 이름 전역 유일성 요구(Island는 이름 충돌을 허용하지만
Flatten은 구조적으로 불가 — 별도 유닛 테스트로 확인된 실제 발견), anchor marker는
`data-loom-id`로 loosely 충족, M1/M6/M9의 실제 정량 재측정(Task #27 몫), 청크
디렉토리 공유로 인한 Task #25/#26 데모 페이지 간 빌드 순서 의존성.

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

## Task #27 실행 상태 (1부, 2026-07-25) — bench/e2e 인프라 + M7 + 교차브라우저 + N-스케일링

상세는 `research-questions/RQ6-verification-plan.md`의 "§3.5 실행" 절 +
`research-questions/data/rq6-task27-part1-e2e-infra-and-nscaling.json`(원시
데이터) — 여기선 요지만.

**`bench/e2e/` 신설**: 이 저장소 최초의 커밋된 Node/Playwright 하네스(Chromium/
Firefox/WebKit 3 프로젝트). WebKit은 호스트(Arch/CachyOS)에서 Playwright 내장
바이너리가 직접 실행 안 됨(Ubuntu 24.04 전용 prebuilt가 요구하는 구버전
SONAME이 롤링 릴리스 배포판에 부재 — 패키지 추가 설치로 해소 안 되는 진짜
불일치) — Docker/Podman 컨테이너(`--network=host`)로 우회(`bench/e2e/
run-webkit-in-docker.sh`).

**M7(호스트 간섭) 최초 실측**: 3개 브라우저 전부 3패턴 10/10 통과. 단, 간섭-2
테스트 중 `registry.js`의 MutationObserver 기반 감지가 순수 DOM 재배치를
detach로 오탐하는 실제 결함을 발견(10/10 재현, 3개 브라우저 동일) — 병익님
지시로 같은 라운드 내에 수정: MutationObserver 콜백(마이크로태스크라 재배치가
이미 끝난 뒤 실행됨)에서 candidate host element의 `isConnected`를 재확인,
아직 문서에 붙어 있으면(재배치일 뿐) detach를 건너뛰도록 함. 수정 후 3개
브라우저 전부 0/10 오탐(테스트에 회귀 게이트 고정).

**M6 조건 B/M8/M9 Firefox·WebKit 교차 확인**: Chromium과 완전히 동일한 결과.

**M4 + M6 N-스케일링 결합 셀(N∈{1,10,50}) 신규 실측 — 실제 버그 발견 및 수정**:
N=50 픽스처가 "같은 wasm 청크를 가리키는 client:load Island 여럿이 동시
activate()"되는, 이 저장소가 이제껏 만든 적 없는 조건을 처음 재현 →
wasm-bindgen의 `__wbg_init` 초기화 가드가 check-then-act이라 락이 없어 동시
호출마다 별도 `WebAssembly.Instance`가 생기고 가장 늦게 끝난 것이 공유 바인딩을
덮어써 먼저 만들어진 인스턴스들이 조용히 오작동(클릭 무반응)하는 실제 레이스를
발견 — `registry.js`의 `activate()`에 `chunkInitPromises` 메모이제이션을
추가해 청크당 import+init을 정확히 한 번만 공유하도록 수정. 수정 후 기존
M7/M8/M9/M6-조건B 스펙 전체를 3개 브라우저에서 재실행해 무회귀 확인(14/14).
**M4 결과**: N=1/10/50 → 1280.0/1344.0/1728.0 KiB(3개 브라우저 완전히 동일값,
N에 비례 증가 — 공유 인스턴스 가설의 직접 증거). **M6 결합 셀 결과(사전 가설과
반대)**: 무작위 위치에 실제 panic 주입 후에도 다른 인스턴스 전부 생존 — wasm
트랩이 선형 메모리 자체를 파괴하지 않고, 주입한 패닉이 어떤 공유 상태도 건드리지
않았기 때문. 단 이 프로브는 "한 인스턴스가 자기 자신의 상태 변경 도중 패닉"하는
더 위험한 시나리오는 검증하지 않음(미실측으로 명시 이관).

**여전히 남은 것(숨기지 않음)**: M6의
"자기 상태 변경 중 패닉" 시나리오 미실측, `crates/loom-rq6-fixture-gen`(계획
명시) 대신 Node 스크립트로 N-스케일링 픽스처 생성(병익님 미확인 판단), M2/M3/
원본 앱 규모 축/M10 전부 이번 라운드 미착수(Task #27 나머지 몫).

**2026-10-17 병익님 결정 — 생성기 대체 건 종결**: 두 생성기를 **공존**시키고 어느 쪽을 쓸지는 설정으로
고른다. Rust 판 `crates/loom-rq6-fixture-gen`(bin `gen_nscaling`)을 새로 만들었고, Node 판
(`bench/e2e/fixtures/gen-nscaling-page.mjs`)은 그대로 둔다. 선택은 `bench/e2e/fixtures/gen-nscaling.sh`의
`LOOM_NSCALING_GEN=node|rust`(기본 node — Task #27이 실제로 쓴 쪽)다. **마커 어긋남 가드**
(`cargo test -p loom-rq6-fixture-gen`, 테스트 3개)가 다음을 지킨다: (1) Rust 출력 = 커밋된 n{1,10,50}.html
(바이트 단위), (2) Node 출력 = Rust 출력(바이트 단위 — Node가 없으면 실패하고 건너뛰지 않는다), (3) 블록의
Island 마커(activation·chunk·ctor·lref·data-loom-id)가 컴파일러 산출물(`loom-widget-embed`의
GraphMode::Islands SSR)의 QuantityPicker 마커와 일치(존재 단언 먼저). 음성 대조: 생성기의 lref를
`Action1`로 바꾸면 세 테스트가 모두 실패했다. 두 생성기로 각각 재생성해도 커밋된 픽스처에 diff가 없었다.
이 결정은 이 픽스처로 얻은 M4·M6 결합 셀 데이터(위)를 바꾸지 않는다 — 산출물이 바이트 단위로 같기 때문이다.

## Task #27 실행 상태 (2부, 2026-07-25) — M1(번들 크기) 실측

상세는 `research-questions/RQ6-verification-plan.md`의 "§3.6 실행" 절 +
`research-questions/data/rq6-task27-m1-bundle-size.json`(원시 데이터) — 여기선
요지만.

**M1을 측정하려다 발견한 치명적 갭**: `crates/loom-widget-embed`의 4개
"청크"(product_card/quantity_picker/checkout_form/mini_dashboard)와
`GraphMode::Flatten` 번들이 `--out-name`만 다를 뿐 사실상 동일한 바이너리
(~438KB, 4개 Island 생성자 + `ResumedFlatApp` 심볼 전부 포함)였다 —
`generate_islands`가 그래프 전체를 항상 하나의 크레이트에 방출하고
`#[wasm_bindgen]` export는 DCE 대상이 아니기 때문. Island의 핵심 이점("위젯
하나만 로드하면 그 코드만 온다")이 오늘까지 실제로 구현된 적 없었다는, RQ6
H-04의 비용 프로파일에 직접 걸리는 발견 — 병익님께 확인 후("컴파일러에 실제
per-Island 코드분할 기능 추가") 진행.

**수정**: `loom-compiler`에 `codegen::generate_single_island_client(graph,
name)` 신설(대상 Island 하나의 코드만 방출, `ComponentRef`로 중첩된 형제
Island는 client-side에서 죽은 코드인 `setup()`용 `unreachable!()` 스텁으로
대체) + `lib.rs`의 `compile_tsx_to_single_island_rust()` 공개 + 신규 유닛
테스트. `crates/loom-widget-embed-island-{product-card,quantity-picker,
checkout-form,mini-dashboard}` 4개 신규 크레이트로 같은 `demo/widget-embed.tsx`
에서 실제 분리 빌드(§4.1 정체성 통제 유지). `cargo test --workspace` 전체
통과, 4개 크레이트 wasm32 `--lib` 체크 통과.

**실측 결과**(raw/gzip9): Layer 0 — H-Qwik `bootstrap.js` 6085/2754, H-Island
`registry.js`+`shared-store.js` 12781/5388(이번 세션 버그 수정 2건으로 이전보다
커짐). 위젯별 분리 빌드 4개는 327~358KB로 서로 30KB 이내 — 위젯 로직은 몇 KB뿐,
~300KB+가 모든 청크에 중복되는 공유 런타임(bevy_ecs+wasm-bindgen). **무상호작용
페이지 로드**: Island는 client:load 위젯 3개가 무조건 즉시 다운로드됨
(1,107,583/396,088바이트) vs H-Qwik은 첫 상호작용 전까지 0바이트(6,085/2,754) —
약 182배/144배 차이(단 이 픽스처는 4/5가 client:load라 Island에 불리한 구성,
대표성 미검증). **완전 관여**(4개 위젯 전부 상호작용): Island
1,471,392/526,636 vs H-Qwik 473,110/158,847 — Island이 약 3.1배/3.3배 더
무겁다(공유 런타임을 4번 중복 지불하기 때문).

**여전히 남은 것**: 청크 간 공유 vendor 모듈(런타임 중복 제거) 미구현 — 효과
미실측. 이 픽스처의 activation 구성 대표성 미검증. M2/M3/M10 미착수.

## Task #27 실행 상태 (3부, 2026-07-25) — M2/M3(TTI/wake-up payload) 실측

상세는 `research-questions/RQ6-verification-plan.md`의 "§3.7 실행" 절 +
`research-questions/data/rq6-task27-m2-m3-tti-wakeup.json`(원시 데이터) — 요지만.

**신규 픽스처**: `demo/widget-embed/flat/index.html` — H-Qwik 반사실 팔의
브라우저 서빙 페이지(Task #26이 예고만 하고 안 만들었던 것). 같은
`demo/widget-embed.tsx`를 `GraphMode::Flatten`으로 파생, `bootstrap-flat.js`는
`demo/resume/bootstrap.js`의 메커니즘을 `ResumedFlatApp` 대상으로 재사용.

**M2(위젯-로컬 TTI, 15회 반복) 결과 — 사실상 비판별적**: Island의 client:load
위젯("페이지 로드 → attached")과 H-Qwik("첫 클릭 → materialize+dispatch 완료")
모두 Chromium 기준 22~42ms 범위에 함께 몰림 — §6 표가 사전에 인정한 "비판별적
지표일 수 있음"이 실제로 확인됨. Firefox/WebKit 교차 확인도 같은 패턴(절대값만
다름).

**M3(wake-up payload, SSR HTML 직접 파싱 — 완전 결정론적) 결과**: 매칭 액션의
`captures` 세그먼트(진짜 페이로드)가 두 팔에서 바이트 단위로 완전히 동일
(`cart_count:0`=12바이트, `visits:0`=8바이트) — "island wake-up ≈ 단일 유닛
wake-up?"(RQ1 트랙 R S2(a))에 대한 답은 그렇다. **실질적 발견**: M1(번들 크기)은
앱 규모에 비례해 커지지만 M3(wake-up 시 역직렬화 바이트)는 앱 규모와 무관 —
두 비용 축이 독립적.

**남은 것**: 원본 앱 규모 축, M10(기능 매트릭스) 미착수.

## Task #27 실행 상태 (4부·마지막, 2026-07-25) — 원본 앱 규모 축 + M10

상세는 `research-questions/RQ6-verification-plan.md`의 "§3.8 실행" 절 +
`research-questions/data/rq6-task27-largeapp-axis-and-m10.json` — 요지만.

**원본 앱 규모 축**: 합성 그래프(TargetWidget 1개 + filler N개, N=0/50)를
`crates/loom-largeapp-axis-flat`(Flatten, 전체)와 `crates/
loom-largeapp-axis-island-target`(같은 N=50 그래프에서 TargetWidget만 추출)
로 실제 빌드해 비교. **결과: 가설 확인** — H-Qwik 번들은 원본 앱 규모에
비례해 실제로 샌다(N=0→50에서 +24,699바이트, 컴포넌트당 +494바이트).
Island의 단일 추출은 filler 개수와 무관하게 일정(327,160 vs 324,706바이트,
잡음 수준). 이번 라운드에서 H-04 쪽에 유리한 유일한 정량 결과 — 단 극단순
합성 필러 기반이라 과잉 일반화 금지(명시).

**M10(기능 매트릭스)**: RQ3 "알려진 약점" 8건 재검토. 신규 실측: (3) 동적
prop 전달 부재 — 컴파일러 유닛 테스트로 구조적 거부 확인. (6) 호스트발 직접
제거의 좀비 위험 — 새 스펙으로 `el.remove()` 직접 호출 시나리오를 재현,
MutationObserver가 정상 감지해 `detached`로 전이함을 3개 브라우저 확인.
기존 데이터 재인용: (1) panic 봉쇄, (4) 상주 승격 단조 증가. 정성 기록: (2)
crash 시 상태 전손(정책 미결), (5) 비동기 정리(해당 없음), (7)(8) 미착수로
정직하게 기록.

**Task #27 종합**: bench/e2e 인프라, M7, Firefox/WebKit 교차 확인, N-스케일링
M4/M6, M1, M2/M3, 원본 앱 규모 축, M10 — 계획이 등록한 항목 전부 최소 1회
데이터 등록 완료. §7 정식 판정은 Task #29 몫으로 남김.

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

> **2026-10-17 정정 — 위 미룬 항목의 현재 상태**: `resumability_payload`는 Task #28-c(2026-07-28)에서
> `render_html_full()`/`loom_fic_render_full()`로 추가됐다(와이어 포맷 JSON). `assets`는 여전히 **항상 빈
> 배열**인 명시적 플레이스홀더다(`loom-fic`에 청크 빌드 툴링 없음). `request_context` 파라미터화와 Sidecar
> 콜드 스타트/커넥션 풀링은 여전히 열려 있다(docs/05 "미해결"). "실제 폴리글랏 백엔드 재확인"이 Phase 6에서
> 이행됐다는 기록은 찾지 못했다 — Phase 6의 M12(Edge/V8 경유 wasm ABI 확인)는 이것과 별개 축이다.
> 2026-07-28 RQ6 확정 뒤 위젯 임베드 단위는 "하나의 World, 복수 anchor root"로 재해석됐고 FIC 계약과 세
> 경로 자체는 무변경이다(docs/05).

## Phase 5 — 공식 검증

- RQ5 우선순위(teardown 안전성 → resumability 건전성 → 스케줄러 무충돌성)대로
- 이 시점엔 설계가 실제 코드로 안정화되어 있어야 함 — 추상적 스펙을 먼저 증명하고
  나중에 코드를 맞추지 말 것

**실행 상태 (2026-07-24) — RQ5 세 후보 전부 실제 결론**

**#2(스케줄러 무충돌성)를 먼저 조사함**(RQ5가 명시한 순서 그대로 — 타입시스템으로
대체 가능한지부터 검토): `bevy_ecs` 0.14.2 실제 소스를 읽어 확인 — 이 프로젝트는
`multi_threaded` feature를 어떤 크레이트에서도 안 켜서(default feature 아님)
실제 실행기가 `SingleThreaded` 하나뿐이고, 동시 실행 자체가 없어 데이터 레이스가
날 대상 자체가 없음. 혹시 몰라 `MultiThreadedExecutor`도 확인했더니
`can_run`(`schedule/executor/multi_threaded.rs:459`)이 실행 직전에
`archetype_component_access().is_compatible(...)`을 무조건 검사(`ambiguity_
detection` 로그 설정과 무관)해 겹치는 System을 절대 동시 실행 안 시킴 —
**Bevy 자체가 이미 이 안전성을 런타임에 보장**함을 확인. **결론: TLA+ 증명
안 함** — 이미 잘 풀린 문제를 재검증하는 것이라 스코프 밖(CLAUDE.md 2-1).
`formal-verification/tla/SchedulerSafety.stub.tla`에 파일:라인 단위 근거 기록,
스텁 상태를 의도적으로 유지.

**#1(teardown 안전성)**: `formal-verification/lean4/SupervisionTree.lean`(구
`.stub.lean`에서 승격) — `crates/loom-runtime/src/island.rs`의
`NativeIslandRegistry` 6-상태 기계를 그대로 모델링해 RQ3의 동기 안전성
(`beginDetach_forbids_access`, `no_transition_restores_access_from_detaching_
or_detached`) + 진행성(`detach_reaches_detached_in_at_most_two_steps`) 정리를
**실제로 증명**(`sorry` 없음, `lean` 명령으로 직접 확인). 스코프 한계 명시:
Entity/DOM 소유권까지 포함한 전체 주장이 아니라 actor 라벨 절반만 증명.

**#3(resumability 건전성, 가장 가치 있으나 정의부터 없던 것)**: `formal-verification/
lean4/ResumabilitySoundness.lean` 신설 — "resume 결과 = hydration했다면 나왔을
결과와 동일하다"를 i32 스칼라 카운터 축소 모델로 **처음 formal 정의**(`Resumability
Sound`)하고, 현재 IR의 실제 제약("시그널당 액션 최대 1개", `SingleWriter`) 하에서
증명. 그 제약이 없으면 실제로 깨지는 반례(같은 시그널을 두 독립 엘리먼트가 각자
재개하면 서로 다른, 둘 다 틀린 값에 도달하는 구체적 계산)도 함께 남김 — IR이 지금
이 제약을 두는 실제 이유. Qwik이 이 정리를 공식 증명한 선례는 재조사에서도 못 찾음.

**검증**: `lean SupervisionTree.lean`/`lean ResumabilitySoundness.lean` 둘 다
에러·경고 없이 통과(Lean 4.29.1). `cargo test --workspace`는 이번 라운드에서
변경 없음(순수 formal-verification/ 작업, Rust 코드 미변경).

**2026-07-28 정정 — "Phase 5 완료"는 RQ5 후보 표에 한해서만 참이었다.**
위 세 후보는 실제로 끝났지만, **Phase 1의 D/E 판정을 실제로 막고 있는 FV 항목은
저 표에 애초에 없었다**. 그 항목은 `RQ1-benchmark-protocol.md` §5의 "추가 주기
(v1.1 — D-1)"가 별도 도입한 **flush 규약 v0의 순서 보장 검증**이고, 조문 자체가
"FV 항목은 특정 후보가 아니라 **공통 인프라(flush 규약 v0)의 검증 항목**"이라고
못박고 있다. 확인 결과 `formal-verification/` 전체에 flush 관련 산출물이 **0건**
이고, 이 문서에서도 위 97행("§5 FV 계류")이 **유일한 언급**이다.
(**2026-08-08 주기**: 이 "0건"과 "유일한 언급"은 2026-07-31 시점의 확인 결과다.
지금은 `formal-verification/verus/flush_order.rs`(28 verified)와
`crates/loom-flush-core`(47 verified)가 있다.) 즉 "Phase 5
완료"와 "D/E를 막는 FV 미착수"가 문서 사이에 모순으로 끼어 있었다 — 후보 표에
없으니 우선순위 논의에도 Phase 5 실행에도 한 번도 올라오지 않았던 것.

→ `RQ5-formal-verification-targets.md`에 **후보 #4**로 정식 등재했고, 증명 의무를
두 갈래(anchor 선행성 / 순서 수렴)로 구체화해 두었다. **Phase 5는 후보 #4가
끝나기 전까지 "완료"가 아니다** — 이 절의 상태를 그렇게 읽을 것.

> **2026-08-08 정정 — 후보 #4의 두 증명 의무는 이행됐다.** anchor 선행성
> (2026-07-31), 순서 수렴(2026-08-01, `plan_writes_converging`의 `ensures`로
> **실제 exec 함수**에 대해). 실측(2026-08-08, `/usr/bin/verus` + Z3 4.12.5):
> `verus formal-verification/verus/flush_order.rs` → 28 verified, 0 errors /
> `verus --crate-type=lib crates/loom-flush-core/src/lib.rs` → 47 verified,
> 0 errors. 남은 것은 **LIS 변형(`plan_writes`)의 수렴**이고, 그 사전조건 P는
> 2026-08-08에 `requires`로 올라갔다.
>
> **2026-10-17 갱신 — 위 "남은 것"은 2026-08-09·08-11에 닫혔다** (이 파일은 그 뒤 갱신되지 않아
> 08-08 시점에 멈춰 있었다. 1차 출처: `RQ5-formal-verification-targets.md` "상태" 절,
> `formal-verification/verus/README.md`의 "LIS 경로의 수렴"·"LIS 건전성 — 증명 완료" 절과 "기준선"):
> - **2026-08-09 — LIS 변형 수렴 증명**: `plan_writes`가 `plan_writes_converging`과 같은 모양의 수렴
>   사후조건을 갖는다. `crates/loom-flush-core` **70 verified, 0 errors**. 단 사전조건
>   **Q(`current ⊆ target`)를 추가**해야 했다 — 목표 명제가 P만으로는 거짓이었다(전수 탐색으로 반례 확인).
> - **2026-08-11 — LIS 건전성 증명**: `lis_indices`/`keep` 조립이 `loom_flush_core::lis_keep`으로 옮겨 가
>   P가 사후조건이 됐고, `plan_lis_from_selection_auto`가 그것을 다음 함수의 사전조건에 물린다.
>   **90 verified, 0 errors**(모델 `flush_order.rs`는 28 verified 그대로). 방출 write 열은 불변
>   (`write_identity` + 옛/새 `keep` 전수 대조 30,153건 불일치 0)이라 §2.5 재측정 사유가 아니다.
> - 따라서 **RQ5 후보 #1~#4가 전부 이행됐고**, 위 2026-07-28 정정의 "후보 #4가 끝나기 전까지 완료가
>   아니다"라는 조건은 충족됐다. 다만 같은 정정의 교훈대로 이 "완료"는 **RQ5 후보 표에 오른 대상 한정**이다.
> - **증명되지 않은 것 (2026-10-17 기준, 숨기지 않음)**:
>   - LIS **최적성** — 의도적으로 증명하지 않았다. "이동을 최소화하는가"는 측정으로만 뒷받침된다.
>   - 원시 입력의 구조적 사실 넷(`selection_preconditions_hold`)은 `debug_assert!`로만 지켜진다.
>   - Verus가 덮는 것은 D/E 계열의 쓰기 계획 계산이고, 적용 루프 자체는 spec으로 모델링돼 있다.
>   - Lean 두 파일은 **모델** 증명이다(모델-코드 일치는 증명 밖). `ownedEntities`가 언제 실제로 비워지는지
>     (despawn 시점)는 `SupervisionTree.lean`의 TODO로 남아 있다.
>   - TLA+ 스텁(#2)은 여전히 **의도적 스텁**(Bevy 위임)이다. 위임 근거는 `bevy_ecs` 0.14.2 소스 인용이었고
>     2026-07-28 의존성 업데이트로 0.19.0이 된 뒤 스텁의 "메이저 업그레이드 시 재확인" 조건이 약 석 달
>     이행되지 않았다. 2026-10-17 스텁 파일에 0.19.0 소스 재확인 절이 추가됐다(결론 유지, 충돌 검사
>     메커니즘은 바뀜, 재검토 조건을 "`set_executor`로 실행기를 바꾸지 않는다"까지 넓힘). 소스 읽기이지
>     실행 증거는 아니다 — 상세와 한계는 `formal-verification/tla/SchedulerSafety.stub.tla`.
>   - RQ1 쪽 "§5 FV 계류" 표류는 2026-10-17 RQ1 본문·프로토콜 정정으로 정리됐다(판정기 고정 문자열만
>     남음 — 맨 위 "현재 상태 요약" 참고).

**도구 선택(2026-07-28, 병익님 제안)**: 후보 #4는 **Verus**(시스템에 업스트림
설치됨, `cargo-verus` + `vstd` + Z3 확인)로 진행한다.
(**2026-08-08 주기 — 실행 명령 정정**: `cargo verus verify`는 업스트림 Verus와
crates.io `vstd`의 불일치로 지금은 실행되지 않는다. 실제 검증 명령은
`verus formal-verification/verus/flush_order.rs`와
`verus --crate-type=lib crates/loom-flush-core/src/lib.rs` 두 줄이다.) 이유는 #1/#3과 대상의
성질이 다르기 때문 — 저 둘은 Lean4 안에 상태 기계·축소 모델을 **손으로 다시 써서**
증명했으므로 "모델이 실제 코드와 같은가"가 증명 밖에 남지만(두 파일 모두 자인),
#4의 대상은 `LoomApp::tick`의 적용 루프와 `Vec<DomWrite>`라는 **구체 자료구조**라
Bevy도 DOM도 개입하지 않아 실제 함수에 `requires`/`ensures`를 걸 수 있다 —
이 저장소에서 **모델-구현 간극이 처음으로 사라지는 항목**. 반대로 `bevy_ecs`/
`web-sys`/SWC가 개입하는 구간과 #3의 남은 과제(의미론적 동치)는 Verus 대상이
아니며 Lean4가 계속 맞다.

**의도적으로 미룬 것(숨기지 않음)**: `SupervisionNode`의 실제 Entity/DOM 소유권
모델링(Layer 2 스펙 확정 후), `ResumabilitySoundness`를 StringList/Bool/Str(DOM-
복구형 재개)로 확장, IR의 "시그널당 액션 1개" 제약을 정적 검사로 실제 강제(현재는
저작자 규율 의존 — `ResumabilitySoundness.lean`의 반례가 이 갭의 실제 근거).
(**2026-10-17 주기**: 이 세 항목은 바로 아래 Task #28의 (d)·(e)·(f)로 2026-07-28에 이행됐다 —
Entity 소유권은 `ownedEntities`까지(DOM 소유권과 despawn 시점은 여전히 밖), 정적 강제는 i32 시그널 한정.)

## Task #28 실행 상태 (2026-07-28) — Phase 3/4/5 잔여 항목 완료

권장 실행 순서(e→d→f→b→c, sharedSignal은 이미 Task #26에서 완료돼 제외)대로
5항목 전부 완료. 상세는 `formal-verification/README.md`의 "2026-07-28" 절 +
아래.

**(e) `ResumabilitySoundness.lean`을 Bool/StringList/Str로 확장** — 이 세
kind는 전부 DOM-복구형(캡처된 리터럴이 아니라 지금 DOM을 직접 읽음)이라
`SingleWriter` 전제가 무조건 필요 없음을 formalize(`dom_recovered_
resumability_sound`, 전제 없는 등식). zero `sorry`.

**(d) `SupervisionTree.lean`에 `ownedEntities` 추가** — 기존 actor-라벨
정리("detach 이후 접근 불가")를 "그 노드가 실제로 소유한 Entity" 수준으로 승격
(`detach_forbids_owned_entity_access` 등). `children`은 방침대로 보류 유지.
zero `sorry`.

**(f) "시그널당 액션 1개" 정적 검사 실장** — `lower.rs`의
`check_one_action_per_signal`. **실측 중 발견한 정정**: 첫 시도는 모든 시그널
종류에 적용했다가 `demo/counter-list.tsx`의 정당한 `items.append()`+
`items.rotate()`(같은 StringList 시그널)를 즉시 깨뜨림(`cargo test --workspace`
로 실제 발견) — (e)가 증명한 대로 StringList/Bool/Str은 SingleWriter가 불필요
하므로 i32 시그널에만 적용하도록 수정. 회귀 테스트 2개 추가.

**(b) crash/상태변화 통지 채널** — `demo/resume/registry.js`에 `setState()`
헬퍼 신설, 상태 전이마다 `window.__loomShared`(RQ4 cross-widget용 기존 保持형
스토어, 무변경 재사용)에 `__island_state:<id>` 키로 게시. 이벤트가 아니라
상태값이라 늦게 붙은 관찰자도 절대 놓치지 않음 — `bench/e2e/specs/
m10-crash-notification-channel.spec.ts`로 실제 crash(존재하지 않는 청크로
`activate()` 실패시켜 진짜 `failed` 경로를 탐)까지 포함해 검증, 3개 브라우저
통과.

**(c) FIC `resumability_payload`/`assets`** — `crates/loom-fic`에
`render_html_full()`/`loom_fic_render_full()`을 기존 `render_html()`/
`loom_fic_render()`와 나란히 추가(대체 아님). 와이어 포맷은 JSON(bincode
아님 — FIC 자체가 언어 무관 소비자를 요구조건에 넣고 있어 Rust 전용 스키마는
부적합, Cargo.toml에 근거 주석). `resumability_payload`는 `html`에 이미
인라인된 `data-loom-lref`를 `StringDom`의 구조화된 attr 목록에서 직접 추출한
것(문자열 재파싱 아님) — 정직한 실질 콘텐츠. `assets`는 여전히 항상 빈 배열
(이 크레이트엔 Island 청크 빌드 툴링이 없음 — "구현했지만 우연히 비었다"가
아니라 명시적 플레이스홀더). serde/serde_json 추가에도 JS-glue-free
wasm32-unknown-unknown 빌드(WASM-in-process 경로)는 무회귀 확인 —
`wasm-tools print`로 `__wbindgen_*` import가 여전히 기존 4개(bevy_ecs의
wasm-bindgen-futures 의존, docs/05가 이미 문서화)뿐이고 새로 추가된 게
없음을 직접 확인.

**검증**: `cargo test --workspace` 전체 통과, `lean` 양쪽 파일 zero `sorry`,
bench/e2e 전체 스위트 3개 브라우저 무회귀.

## Phase 6 — RQ6 최종 검증

- Phase 2 종료 시점의 벤치마크와 Phase 3 완료 후 벤치마크를 비교
- Island 도입이 실제로 이득(백엔드 임베딩 유연성)에 비해 복잡도(RQ3/RQ4) 비용을
  정당화하는지 최종 판단. 이 시점에 04의 재해석이 맞았는지 여부가 결정됨

**실행 상태 (2026-07-24) — 종료 조건 충족(조건부/비결정 출력)**

RQ6-verification-plan.md §7 게이트를 기계적으로 적용 시도 — **게이트 진입 자체가
불가능함을 확인**. 이유는 데이터가 애매해서가 아니라 **H-Qwik 위젯-임베드 팔 자체가
한 번도 만들어진 적이 없기 때문**(§4.1 "정체성 통제" 원칙 때문에 컴파일러 통합 없이는
손코딩으로 만들면 원칙을 어기게 되어 §3.2에서 의도적으로 스킵 — 그 결정이 그대로
Phase 6까지 이어짐). §7의 모든 확정 조건(H-Qwik 강/약, H-04 강/약)이 "H-Qwik 팔의
실측값"을 요구하는데 그게 없어 어느 쪽도 진입 요건 자체를 충족 못함.

**결론: 조건부/비결정 (§11 경로 발동)** — 이건 "실패"가 아니라 RQ6 문서 자신이 미리
"가장 정직한 가능성 높은 결과"로 인정해 둔 범주. 실제로 확보된 것: Island의 장애
격리(조건 B)가 **완전 봉쇄**로 실측 확인되고 그 비용(공유 청크 대비 +91% 바이트)도
정확히 잡힘 — "Island을 만들어봤더니 격리조차 안 됨"이라는 최악의 시나리오는 배제됨.
남은 미지수 둘: (1) H-Qwik으로 동일 시나리오를 만들면 얼마나 나쁜가(순수 기술적
잔여 작업, 비교 기준선 부재), (2) 위젯-임베드 유스케이스가 실제로 중요한가(시장
질문, §12가 이미 이 프로젝트 관할 밖으로 선언).

**Phase 6 종료 조건 충족 방식**: §11이 명시한 대로 "확정 판정"이 아니라 **"조건부
명제 등록 + 이관 방법 확정 대기"**로 충족됨 — 이관 방법 3가지((i) 별도 시장 리서치
(ii) 병익님의 제품 판단 (iii) 조건부 결론 등재) 중 어느 것을 택할지는 병익님 판단이
필요한 지점이라 이 문서가 대신 정하지 않음(RQ6 문서 §3.3 실행 기록 참고).

> **2026-10-17 정정**: 위 2026-07-24 "조건부/비결정" 판정과 "이관 방법 결정 대기"는 **아래 2026-07-28
> (Task #29) 최종 판정으로 대체됐다** — 막고 있던 이유(H-Qwik 위젯-임베드 팔 부재)가 Task #25~#28로
> 해소돼 §7 게이트에 진입했고, 결과는 H-Qwik 확정(강한 형태)이다. 그러므로 이관 방법 (i)/(ii)/(iii) 선택은
> §11 조건부 경로가 발동하지 않아 불필요해졌다. 단 "위젯 임베드가 실제로 흔한 유스케이스인가"라는 제품
> 질문 자체는 아래 07-28 판정의 "정직한 한계"대로 여전히 병익님의 제품 판단 영역으로 열려 있다.
> 이 절은 시간순 기록이라 지우지 않는다.

**실행 상태 (2026-07-28, Task #29) — 최종 판정 확정: H-Qwik 확정 (강한 형태)**

위 2026-07-24 판정을 가로막았던 이유("H-Qwik 위젯-임베드 팔 자체가 없음")가
Task #25~28을 거치며 완전히 해소됐다 — 컴파일러가 실제로 `GraphMode::Islands`/
`GraphMode::Flatten` 양쪽을 같은 소스에서 파생하고(§4.1 정체성 통제 충족),
Task #27이 M1-M5/M9를, Task #29가 마지막 공백(H-Qwik 팔의 M6/M7/M8)을 채웠다.

`RQ6-verification-plan.md` §7 게이트를 실제 데이터에 기계적으로 적용한 결과:
- M8(정합성) 양쪽 통과.
- M6(격리, 조건 A — 오늘 실제 채택 상태)에서 등가 — §5.4 사전 예상과 일치.
- M7(간섭)에서 등가(간섭-1 실측 등가, 간섭-2는 H-Qwik에 대응 실패 모드 자체가
  없어 사전 지정대로 참고용).
- M1(번들 크기)에서 H-Qwik 명백히 우세(완전 관여 시 약 3.1배 가벼움). M2/M3/M9
  등가. M4는 Island 단독의 구조적 약점(hibernation 없음).

§7 "H-Qwik 확정 — 강한 형태" (a)(b)(c) 전부 충족 — 단, (a)의 "조건 B/C 채택
시에도 등가 등급을 달성한다"는 문구가 반사실적 조항(오늘 미채택인 조건 B/C는
지금 당장의 요구조건이 아님)인지 실제 요구조건인지 해석이 갈리는 지점이 있어,
**병익님이 전자로 확정**해 이 판정에 이르렀다(후자였다면 조건부/비결정).

**판정의 실제 의미**: "Island이 필요 없다"가 아니라, 이 프로젝트가 실측한
규모(5개 위젯 픽스처)에서는 Island 경계를 컴파일타임에 강제하는 별도 격리
단위 없이도 resumability 하나만으로 요구를 동등하거나 더 나은 비용으로
충족한다는 것 — Qwik의 원래 주장이 이 프로젝트 자신의 실측으로 확인됐다.

**§11 폐기 경로 실행 완료**: docs/04(전면 재작성 — "Island=격리 단위" 재해석을
폐기 사유와 함께 이력 보존, "지연 activation 서브트리" 신모델 등재), docs/05
(재해석 — "하나의 World, 복수 anchor root" 모델), docs/06(IslandRoot 개념
제거), docs/07(Island 계열 용어를 "세분화 선택 시" 의미로 재정의), RQ3(부분
폐기 — 2단계 detach/dormant는 존치), RQ4(전면 폐기), RQ2/formal-verification
(경미 재검토, 무효화 아님), 이 Phase 3 절(위 재해석 공지). 상세는
`RQ6-verification-plan.md`의 "§3.9 실행" 절과 `RQ6-are-islands-necessary.md`의
"상태" 절.

**정직한 한계(숨기지 않음)**: 이 판정은 오늘 실측한 규모(5개 위젯)에서 나온
것 — 원본 앱 규모 축 실측이 컴포넌트 수가 커지면 H-Qwik의 번들 크기 우위가
줄어듦(마진 비용이 0이 아님)을 이미 보였으므로, 훨씬 큰 규모에서는 판정이
달라질 수 있다. "위젯 임베드가 실제로 흔한 유스케이스인가"는 이 판정과
무관하게 여전히 병익님의 제품 판단 영역이다.

**상세**: `research-questions/RQ6-verification-plan.md`의 "§3.3 'Phase 6 — 최종
판정' 실행" 절 — §6 프로파일 전체 표, §7 각 조건별 명시적 미평가 사유.

**추가 실행 (2026-07-24) — ISR/Edge 측정 축 (§4.5, M11/M12)**

> **2026-10-17 주기 — 이 절의 위치**: 날짜가 2026-07-24인 이 절은 위 2026-07-28 최종 판정 문단보다
> **뒤에** 놓여 있다(07-28 판정을 적을 때 07-24 블록들 사이에 끼워 넣은 결과). 내용은 07-28 판정 **이전**의
> 것이므로, 절 끝의 "이관 방법은 여전히 병익님의 결정 대기"는 07-28 판정으로 해소된 상태로 읽을 것.

위 세 가지 이관 옵션 중 하나를 바로 고르는 대신, 병익님이 "SSG/CSR/SSR/ESR/ISR을
전부 지원하려면 어떻게 해야 하나"라는 역질문으로 새 측정 축을 먼저 요구함 — ISR과
위젯-임베드 유스케이스가 정확히 겹친다는 관찰(Next.js/Astro가 대중화시킨 조합)에
따라 §4.5로 설계·실측을 추가하고 §7 게이트 진입 전에 등록.

- **M11 (ISR 부분 재생성 비용)**: H-Qwik류 모놀리식 컴포넌트는 어떤 변경에도 페이지
  전체를 다시 렌더해야 하는 반면(하나의 컴파일 단위), Island는 변경된 위젯만
  재생성. `crates/loom-fic`에 세 변형(모놀리식-naive/모놀리식-optimized/
  위젯-only)을 구현해 `isr_bench.rs`로 측정 — `research-questions/data/
  rq6-m11-isr-partial-regen.json` 참고.
- **M12 (Edge/ESR wasm ABI 확인)**: Phase 4의 plain wasm32 FIC 빌드(wasm-bindgen
  없음)가 wasmtime류(이미 Phase 4에서 증명)뿐 아니라 V8-isolate류 엣지 런타임에서도
  같은 4개 스텁 import만으로 로드되는지 Node.js(V8 기반) 경유로 확인 —
  `crates/loom-fic/examples/edge_v8_proxy.js`.

**이 축을 실측하는 도중 발견한 별개 갭 — 정적 마크업 컴파일타임 처리**: 병익님이
"VDOM을 아예 배제하거나 컴파일타임에 전부 처리해 런타임엔 VDOM이 없는 것과
마찬가지가 되도록"이라고 제안 — docs/02가 원래부터 명시했던 설계("순수 정적
마크업 → 컴파일타임 HTML 템플릿 문자열")를 실제 코드젠이 지키지 않고 있었다는
실제 갭이었음. 별도 커밋으로 분리해 먼저 고침(직전 커밋) — M11 데이터는 그
수정 전/후 둘 다 측정해 효과를 정직하게 기록: 정적 구간당 런타임 비용 7~14배
절감, 그러나 여전히 O(n)(Island의 "그 바이트를 아예 안 건드림"과는 질적으로 다름).

이관 방법(i/ii/iii) 자체는 여전히 병익님의 결정 대기 — 이 축은 그 결정에 필요한
데이터를 하나 더 채운 것이지, §11의 결정 자체를 대신하지 않음.
(**2026-10-17 정정**: 이 "결정 대기"는 2026-07-28 Task #29의 §7 게이트 판정(H-Qwik 확정, 강한 형태)으로
해소됐다 — 위 "실행 상태 (2026-07-28, Task #29)" 참고.)

## 조건부 내용의 중첩 지원 (2026-08-07) — 트랙 N을 막고 있던 컴파일러 벽 제거

**왜 지금**: RQ1 §5 트랙 N은 "리스트 > 조건부 > 리스트" 3중 중첩 위에서
property test를 돌리라고 요구한다. 그런데 `lower.rs`가 조건부 내용을 **정적
텍스트만**으로 제한하고 있었고(`"Phase 1 conditional content must be
static-only"`), §2.3-1이 "사이트별 반복 부분은 기계적 복제 가능 형태 —
**컴파일러가 생성할 수 없는 형태의 코드 금지**"라 손코딩 픽스처로 우회하는
길도 규약상 막혀 있었다. 즉 **컴파일러가 먼저 중첩을 배워야만** 트랙 N을
시작할 수 있는 상태였다.

두 라운드에 걸친 트랙 N 설계(8개 안)가 전부 반려됐던 이유도 여기 있었다 —
런타임 쪽에서 우회하려다 다른 조항과 충돌한 것이 아니라, **애초에 컴파일러
쪽이 정답**이었다. 병익님의 방향 지시("컴파일 시간이 좀 오래 걸리더라도
런타임이 가볍고 빠르면 감수할 만하다")와도 정확히 같은 방향이다.

**된 것**: 조건부 안에 정적 마크업 / `{signal()}` / `{signal().map(...)}` /
그것들을 감싸는 엘리먼트를 넣을 수 있다.

- **설계의 핵심은 entity와 노드의 수명 분리**다. 중첩 binding의 `Entity`는
  `setup()`이 항상 즉시 만들되 **`DomTarget`은 붙이지 않는다**. patch System의
  쿼리가 `&DomTarget`을 요구하므로, DOM에 없는 동안은 **archetype 수준에서**
  (런타임 분기 없이) 자동 제외된다 — Bevy의 접근 선언 기반 스케줄링 재사용
  (CLAUDE.md 2-1). 조건부가 처음 `true`가 될 때 새 `DomWrite::SpawnElement`가
  노드를 만들고 `DomTarget`을 붙이면 그때부터 평범한 binding이 된다.
- entity를 항상 즉시 만드는 이유는 `Signals` 구조체·action·`resume()`이
  "조건부 안이냐 밖이냐"를 런타임에 따지지 않게 하기 위함이다.
- 사이트 함수가 안쪽 entity를 찾는 방법은 새 `CondNested`(entity 벡터) +
  **컴파일타임에 고정된 리터럴 인덱스**뿐이다 — 이름 조회도 트리 순회도
  런타임에 남지 않는다. 완전 정적 부분트리는 컴파일타임에 HTML 한 문자열로
  접혀 `AppendStaticHtml` 호출 한 번이 된다(`setup()` 쪽 접기와 동형).
- 런타임 추가는 최소한이다: `WriteParent`(핸들 | entity), `DomWrite::
  SpawnElement`, `DomWrite::AppendStaticHtml`, `CondNested`. `AppendStaticText`의
  `parent`가 `NodeHandle` → `WriteParent`로 넓어졌다(기존 방출부는 전부
  `WriteParent::Node(...)`, 의미 무변경).

**설계 도중 실제로 걸렸고 피해 간 함정(음성 대조로 확인)**: 지연 생성되는
binding의 초기값을 제네릭 System에 맡기면 **영영 그려지지 않는다**. 그 entity의
`DomTarget`은 write가 적용될 때 붙으므로, `Signal`이 변경 표시된 틱에는 쿼리가
매치하지 않았고 `DomTarget`이 생긴 다음 틱에는 이미 `Changed`가 지나가 있다.
그래서 텍스트는 `SpawnElement.text`에, 리스트의 초기 row는 사이트 함수가 직접
방출하고 `ListRows`를 시드한다(`SpawnRow`가 텍스트를 생성 시점에 굽는 것과 같은
근거). 이후 변경은 평범하게 제네릭 K가 이어받는다.

**resume**: 조건부 안의 signal은 **SSR 시점에 그 조건부가 보였는지**에 따라
갈린다. 숨겨진 채 SSR됐다면 안쪽 노드가 아예 없으므로 `find_node`를 부르면 안
된다 — `DomTarget` 없이 spawn하고 첫 열림에 지연 경로가 이어받는다. 보였다면
`data-loom-id`(지연 생성 시점에 `SpawnElement`가 단다)로 평범하게 찾는다.
그래서 `resume()`은 조건부(Bool)를 먼저 전부 처리해 `<cond>_shown`을 세운 뒤
나머지 signal을 처리하고, `CondNested`는 루프가 끝난 뒤 붙인다.
**한계(숨기지 않음)**: 숨겨진 채 SSR된 리스트는 **선언된 초기값**으로 seed된다 —
그 사이의 편집 내역은 SSR HTML에 흔적이 없어 복원할 수 없다(RQ3 Q2의 "SSR 스냅샷
동결" 한계와 같은 성질).

**아직 못 받는 것(명시)**: 조건부 안의 또 다른 조건부, `<input>` 양방향 binding,
지연 노드의 자체 `onClick`, `<Component/>` 참조, `sharedValue(...)`. 그리고
**리스트 row 템플릿 안의 조건부** — 트랙 N이 요구하는 *바깥쪽* 중첩이며,
row 템플릿을 사이트별로 monomorphize하는 별도 작업이다(다음 단계). 즉 이번
라운드로 트랙 N이 열린 것은 아니고, **막고 있던 두 벽 중 안쪽 하나가
치워졌다**.

**검증**: 새 크레이트 `crates/loom-nested-fixture` — `demo/nested-conditional.tsx`를
실제 loomc로 컴파일해 네이티브 `TreeDom` 위에서 돌린다(문자열 단언만 하는
loom-compiler 단위 테스트로는 "rustc를 통과하는가"도 "돌렸을 때 옳은가"도 알 수
없기 때문). 8개 통합 테스트 + loom-compiler 신규 단위 테스트 3개.
**음성 대조 3건 전부 확인**: (1) 초기 row 방출 제거 → 조건부를 열어도 리스트가
비어 있음, (2) 텍스트 초기값 굽기 제거 → 열었을 때 값이 안 보임, (3) resume의
shown 게이트 제거 → 숨겨진 조건부 resume이 없는 노드를 찾아 패닉.
`cargo test --workspace` 157 통과, wasm32 빌드 통과.

**실브라우저 검증 범위(정확히)**: 기존 `demo/index.html`을 wasm-pack으로 다시
빌드해 Playwright Chromium으로 전 기능(카운터 +1×2, append, rotate, 조건부
토글 3회) 무회귀 확인 — `AppendStaticText`의 `parent` 타입 변경과 조건부 코드젠
재작성이 배포 경로를 깨지 않았음을 확인한 것이다. **중첩 조건부 자체는 아직
실브라우저에서 돌려 보지 않았다** — 네이티브 `TreeDom` 테스트 8개 + 음성 대조
3건으로만 확인됐다. 위험이 낮다고 판단한 근거는 `SpawnElement`가 쓰는 DOM
연산(`create_element`/`set_attribute`/`set_text_content`/`append_static_html`)이
전부 `setup()`과 `SpawnRow`가 이미 브라우저에서 쓰고 있는 것들이고 새 DOM
의미가 하나도 없다는 것이다. 다음 단계(row 템플릿 monomorphize)에서 어차피
중첩 픽스처의 wasm 데모가 필요하므로 그때 함께 붙인다.

## row 템플릿의 사이트별 monomorphize (2026-08-07, 같은 날 2단계) — 트랙 N의 바깥쪽 벽 제거

앞 절이 조건부 **안쪽**을 열었다면, 이번은 **바깥쪽** — 리스트 row 템플릿이
`<tag>{item}</tag>` 한 형태로 못박혀 있던 제약이다. 이제 row 안에 정적 마크업과
**조건부 하나**(그 안에 중첩 리스트 하나)를 넣을 수 있고, 그래서 §5가 요구하는
**"리스트 > 조건부 > 리스트" 3중 중첩을 컴파일러가 낼 수 있다**.

**왜 알고리즘을 코드젠으로 복제하지 않았나**: D/E의 배치 계획은 Verus로 검증된
`loom_flush_core::plan_writes*`가 낸다. 사이트마다 알고리즘을 복제하면 검증된
코드가 아니라 **복제본**이 배포된다. 그래서 알고리즘은 제네릭으로 두고 **모양만**
컴파일타임 파라미터로 뺐다(`loom_runtime::RowSpawner`). RQ1 §2.4 [E]가 최선형
항목으로 이미 요구하던 "site별 monomorphize"가 정확히 이 형태이기도 하다.

- **런타임**: `RowSpawner{Ctx, Marker, emit}` + `RowSpawnCtx` + `DefaultRow` +
  후보별 `*_system_with`. 후보 본문은 여전히 하나씩이고, 접미사 없는 시스템은
  그 본문을 `DefaultRow`로 부르는 껍데기다 — 시그니처가 그대로라 트랙 L 하네스는
  한 줄도 안 바뀌었다.
- **방출 동일성 증명**: `tests/write_identity.rs`가 리팩터 직전 커밋(f0eaf6f)을
  별도 워크트리에 체크아웃해 같은 레코더로 뽑은 골든과 비교한다(손으로 적은
  기대값이 아니다). 후보 5종 × R1/R2 × 8단계 시퀀스, entity id는 최초 등장
  순서로 정규화. **완전 일치** → v11~v18 수집분은 §2.5 재측정 대상이 아니다.
- **컴파일러**: 사이트마다 `RowSite{i}Spawner`(ZST) + `RowSite{i}`(리스트 entity
  마커) + `RowSite{i}Ctx`(Resource) + 세 시스템(`_ctx` → `_sync` →
  `cond_reconcile_row_site_{i}`)을 생성. 한 컴포넌트에 복잡한 row가 하나라도
  있으면 그 컴포넌트의 **모든** 리스트 사이트를 이 경로로 낸다(마커 없는 제네릭
  K와 섞이면 한 리스트를 두 시스템이 재조정한다).
- **row별 상태**: row 안의 조건부/중첩 리스트는 row 인스턴스마다 자기 entity를
  갖고, 값은 컴포넌트 스코프 signal → 사이트 Resource → mirror entity로 흐른다.
  `_ctx`가 **리스트 System보다 먼저** 돌아야 새로 생기는 row가 현재 값으로
  만들어진다.

**한계(숨기지 않음)**:

1. **row별 독립성이 없다.** 조건부 술어와 안쪽 리스트 내용이 모든 row에서 같다 —
   row 스코프 파생 signal이 IR에 없기 때문이다. 3중 중첩 구조, row별 구간, 부모
   entity별 파티션은 그대로 성립하지만 **구간 크기가 row마다 균일**하다. 따라서
   §5 불변식 2("move가 다른 row/블록 노드를 찢지 않음")의 결함 중 *균일 구간에서
   상쇄되는 부류*는 이 픽스처가 못 잡는다. 트랙 N 실행 전에 §10-1에 이 축소를
   등재하거나, row 스코프 파생 signal을 IR에 추가해야 한다 — **결정 전에
   트랙 N을 돌리지 말 것**.
2. row 안 조건부는 `data-loom-id`를 달 수 없어(row가 여럿이라 id가 유일하지
   않다) `resume()`이 선언된 초기값으로 되살린다. SSR 이후 편집 이력은 복원되지
   않는다(RQ3 Q2의 "SSR 스냅샷 동결"과 같은 성질). 네이티브 property test인
   트랙 N에는 영향 없다.
3. row 템플릿의 첫 자식은 `{item}`이어야 하고, 조건부는 하나, 그 안의 리스트도
   하나다. row 안 조건부의 wrapper를 binding 노드로 바로 쓸 수 없다(엘리먼트로
   한 겹 감싸야 한다 — row는 런타임에 생겨 wrapper의 핸들이 방출 시점에 없다).
4. 현재 이 경로는 **K로만** 돌아간다(컴파일러가 K를 등록). 트랙 N이 D/E의
   불변식을 재려면 후보를 갈아 끼우는 배선이 더 필요하다 — `*_system_with`는
   D/D-LIS/E/E-LIS 전부 이미 있으므로 등록 선택만 남았다.

**검증**: `demo/nested-list.tsx`(3중 중첩) → `crates/loom-nested-fixture`가 실제
loomc로 컴파일해 네이티브 `TreeDom`에서 구동. 통합 7개(닫힘/열림/열린 채 추가된
row/안쪽 편집/바깥 회전/닫기/resume). **음성 대조 2건**: (1) 열린 채 생긴 row의
내용을 제네릭 경로에 맡기면 "열린 채 추가" + "회전" 두 테스트만 실패, (2) sync가
안쪽 리스트를 전파하지 않으면 "안쪽 편집" 하나만 실패. 그리고
**`demo/counter-list.tsx` 산출물이 리팩터 전후 바이트 동일**(복잡한 row가 없으면
2026-08-07 이전과 완전히 같은 코드를 낸다). `cargo test --workspace` 173 통과,
wasm32 빌드 통과.
*(2026-10-17 부기) 7개 중 resume 테스트(nested_list.rs의 resume 케이스)는 공허하다 — SSR 전 TOGGLE로 남은 잔존물을 세고, 열림·닫힘을 구별하지 못한다. resume 산출물에는 RowSite 마커와 row 엔티티가 없다(docs/09 R11·R32). 트랙 N G2는 resume을 쓰지 않아 영향이 없다.*

## row 스코프 파생 signal (2026-08-07, 3단계) — 트랙 N 픽스처의 판별력 확보

앞 절이 남긴 **1번 한계**("row별 독립성이 없어 구간 크기가 균일하다")를 해소한다.
그 한계는 §5 불변식 2의 결함 중 균일 구간에서 상쇄되는 부류를 원리적으로 못 잡게
만들고 있었다 — 트랙 N을 돌리기 전에 결정이 필요하다고 기록해 둔 항목이다.
**선택: IR 확장**(§10-1에 축소를 등재하는 쪽이 아니라).

**저작 표면 두 형태**(둘 다 닫힌 형태, 표현식 언어를 여는 것이 아니다):

- `{open() && item.length > n && <X/>}` — 조건부 실효값 = `공유 signal && row 술어`
- `{xs().slice(0, item.length).map(...)}` — 안쪽 리스트 길이가 아이템에 묶인다

**클로저가 아니다.** `ir::RowPredicate`/`ir::RowSlice`는 닫힌 열거형이고 계산은
사이트별 생성 코드에 인라인된다. Component에 들어가는 것은 결과뿐
(`RowDerivedFlag(bool)`/`RowDerivedTake(usize)`) — CLAUDE.md 금지 4 그대로이고,
docs/03의 PatchTable 패턴(정수 opcode + 컴파일타임 정적 디스패치)과 같은 결이다.
결과를 저장하는 이유: 공유 signal이 바뀌면 row별로 다시 계산해야 하는데 그 시점에
"이 row의 아이템"을 알 방법이 없다(후보마다 `RowKey`/`OrderedKey`/없음).

**§5 요구와의 정합**: 내용은 여전히 공유 signal에서 오므로 "외부 리스트 ops +
조건부 토글 + 내부 리스트 ops의 무작위 인터리브"가 그대로 성립한다 — 파생이
조작을 잡아먹지 않는다. 파생되는 것은 **길이/개폐**뿐이다.

**검증**: `demo/nested-list-derived.tsx`(outer=["a","bb","ccc"],
inner=["p","q","r","s"]) → row별 구간이 각각 0/2/3. 통합 6개(초기/열기/공유 편집/
긴 row 추가/회전/닫기+재열기), row HTML을 통째로 비교(구간 경계를 세는 보조
코드가 틀리면 테스트가 조용히 무력해지므로).

**핵심 확인 — 강화가 실제로 판별력을 더했는가**: 파생 술어를 무시하도록 코드젠을
망가뜨리면(= 모든 row가 열리는 균일화 결함) **균일 픽스처는 7개 전부 통과하고
파생 픽스처만 5개 실패**한다. 파생 slice를 무시했을 때도 파생 픽스처에서만 2개
실패. 균일 판본은 그대로 두었다 — 두 코드젠 경로(파생 없음/있음)를 모두 덮는다.

`cargo test --workspace` 179 통과.

## R3 축이 C에만 적용돼 있던 문제 수리 (2026-08-07)

**발견**: `RowVariantMode`(§3.2 R3 축의 on/off)를 시스템 파라미터로 **선언**하는
후보는 다섯인데 **읽는** 후보는 C 하나뿐이었다. D/D-LIS/E/E-LIS는 선언만 하고
본문에서 안 썼고(rustc 미사용 경고 4건이 떠 있었다), K는 선언조차 없었다.
`place_in_order`/`emit_plan`(D·E 공용 spawn 경로) 어디에도 브랜치 marker 삽입이
없었다.

**귀결**: 수집분 v11~v18 전부에 `row_variants: true` 셀이 6후보 모두 기록돼
있었지만 C 외 다섯은 **R1 셀에 R3 라벨이 붙은 것**이었다. §3.3의
"R3 × {C, D}" 사전등록에서 D 열이 한 번도 측정된 적이 없다.
→ `RQ1-benchmark-protocol.md` §11-2 **개정 #8**로 등재, 해당 셀 무효 처리.

**이미 내린 판정에는 영향 없음**: `rq1-verdicts.py`가 `variants=True`를 읽는 곳은
§9 C 기각 뒷 분기 하나뿐이고 그것은 C 자신의 절대 임계값이다. C의 R3 구현은
처음부터 있었다.

**더 중요한 것 — 공허성**: "keyed 후보는 재정렬마다의 archetype move 비용이
없다"는 RQ1의 예측이, marker 자체가 없으면 **keyed 성질이 아니라 구현 누락 때문에**
참이 된다. 이 세션에서 같은 공허성이 **세 번** 나타났다:

1. 수집 데이터의 D열 R3 셀(위).
2. 새로 쓴 회귀 테스트 `r3_axis.rs`의 "전환 없음" 단언 — 결함을 재주입해 보니
   marker 없이도 통과했다. marker 존재를 먼저 단언하도록 고쳤다.
3. **기존 테스트** `list_candidates.rs`의
   `c_switches_row_branches_on_reorder_but_d_does_not` — `d_flips == 0`이 통과하고
   있었지만 D에 marker가 없어서였다. 게다가 이 함수는 브랜치를 **위치 기준**으로
   비교하고 있어서, marker가 생기자마자 재정렬만으로 오탐이 났다(entity 기준으로
   고침). 즉 이 테스트는 두 가지 이유로 무력했다.

**수리**: row를 만드는 단일 지점(`RowSpawner::emit`)에서 축이 켜져 있으면 값에 따라
`RowVariantA`/`RowVariantB`를 배정한다. §10이 사전 고정한 전환형은 안 바꿨다 —
바뀐 것은 **초기 배정이 존재한다**는 것뿐이다.

**검증**: 새 회귀 테스트 `crates/loom-runtime/tests/r3_axis.rs` — 6후보 전부에 대해
(a) 축이 꺼지면 marker 0개, (b) 켜지면 전 row에 배정되고 A/B가 실제로 갈린다,
(c) keyed 후보는 재정렬로 전환되지 않는다(공허성 방지 단언 포함). 음성 대조:
marker 배정을 없애면 7개 중 6개 실패. retained row의 브랜치를 뒤집는 결함을 넣으면
고친 기존 테스트가 잡는다. `write_identity` 골든은 그대로 통과한다 — marker는
`Commands`이지 `DomWrite`가 아니라 방출 write 열에 나타나지 않고, 축이 꺼진
조합에서는 삽입 자체가 없다.

**재수집 필요**: R3 조합(`variants=true`)에 한해 전 후보 재실행. 다른 조합은 불변.
(**2026-10-17 주기**: 이 재수집은 개정 #9(2026-08-12)의 트랙 L 전면 무효화에 흡수됐다. 그 뒤
2026-09-23(0711224)에 "R3 축이 D·E에 전혀 안 걸리고 있었다 — 하네스가 마운트 뒤에 켰다"가 추가로
발견·수리됐고, R3 셀은 v21·v22 전면 재수집에 포함돼 측정됐다(v22 판정문의 C 기각 뒷 분기 W2×R3 —
rep별 sync_point 비중 median 21.0% < 50%, 미발동).)

**함께 기록한 별건(수리하지 않음)**: §3.2의 R3 문언은 "값에 따라 `<A/>`/`<B/>`
**구조 분기**"인데 C를 포함해 **DOM 구조는 A/B가 동일하다** — 분기는 ECS archetype
수준에만 있다. §9 C 기각 (ii)가 재는 것은 sync point 구간이므로 그 조항의 판정은
이 갭 없이도 성립한다. 구조 차이까지 모델링할지는 재수집 전에 별도 결정(개정 #8에
기록).

## 전체 점검 (2026-08-07) — 이 세션 변경분의 사후 검사

세 단계(중첩 조건부 → row 템플릿 monomorphize → row 스코프 파생 → R3 수리)를
마친 뒤 전체를 훑었다. **새로 나온 것 두 건**:

**(1) docs/03 N3 위반 — 실제로 있었고 고쳤다.** CLAUDE.md 금지 1은 "동적 시퀀스
사이트의 좁은 전용 알고리즘이 VDOM 재도입인지는 감으로 판단하지 말고 docs/03의
판별 기준 조문으로만 판정할 것"을 요구한다. 그 판정을 실제로 돌려 보니, row별
mirror sync System(`row_site_N_sync`)이 매 틱 그 사이트의 row를 전부 훑고 있었다 —
무관한 갱신 하나에도 사이트 크기에 비례하는 비용이 붙으므로 N3 위반이다.
원본이 실제로 바뀐 틱에만 돌도록 게이팅했다(`Res::is_changed`).

이 결함은 **동작 테스트로는 원리적으로 관측되지 않는다**(값이 같으면 어차피 쓰지
않으므로 결과 DOM에 차이가 없다 — 게이팅을 걷어내고 14개 동작 테스트를 돌려도
전부 통과한다). 그래서 생성 코드의 구조 단언으로 고정했다. 판정 근거 전문은
docs/03 판정례 9로 등재(N5-(i) 요구).

**(2) `Workload::is_deterministic`이 만들어만 놓고 안 쓰이고 있었다.** §7 M2
("시드 고정 하에 결정적 수치")를 **런 안에서** 검사할 자리인데 비어 있었다.
배선했다 — 결정적 워크로드(swap/remove_middle)에서 첫 rep과 마지막 rep의 op 수를
대조해 `m2_deterministic`으로 레코드에 싣는다(추가 tick 없음). v15의 "배치 묶음이
리스트를 소진해 rep마다 다른 길이에서 remove_middle이 돌던" 결함이 정확히 이
검사로 잡히는 유형이다. 분석 쪽에도 대조를 넣되, **필드가 없는 옛 수집분은
"미계측"으로 보고**한다(0건을 "통과"로 읽히게 두지 않는다).

**나머지 점검 결과**: `cargo test --workspace` 190 통과, wasm32 전 크레이트 빌드
통과, 실브라우저(Playwright Chromium) 전 기능 무회귀(카운터/append/rotate/조건부
3회 토글/폼 입력, 페이지 에러 0). 런타임·하네스의 미사용 파라미터 경고 0건
(R3 수리로 4건이 사라졌다). `lower.rs` 모듈 문서의 지원 범위를 구현과 맞췄다
(그 전까지 "exactly one text-child row shape"로 남아 있었다 — 문서↔구현 어긋남).

## 트랙 N 하네스 구축 + D/E 후보 등록 가능화 (2026-08-08)

### 후보 선택 (`ListCandidate`)

컴파일러가 K만 등록하던 것을 K/D/D-LIS/E/E-LIS 중 선택 가능하게 했다.
**필요한 이유는 공허성 방지다**: §5 불변식 2는 D의, 불변식 3은 E의 성질을 재는
것이라, 그 후보로 컴파일된 픽스처가 없으면 애초에 잴 대상이 없다.

후보를 바꾸면 달라지는 것은 셋뿐이다 — 등록하는 System, 리스트 entity의 저장
Component(`ListRows` vs `OrderKeyMembers`), 그리고 미리 방출하는 초기 row의
정체성 Component. row 모양(`RowSpawner`)·조건부·파생은 후보와 무관하다.
`compile_tsx_to_rust`(기존 build.rs 전부)는 계속 K를 내고 **산출물이 바이트
동일**하다.

**비-K 산출물은 네이티브 전용**이다: `resume()`은 후보별 저장 Component와 row
키/순서 태그를 SSR DOM에서 되살려야 하는데 그 복구가 후보마다 다르고 트랙 N은
resume을 쓰지 않는다. 조용히 K 모양으로 되살리면 **다른 후보인 척하는 앱**이
나오므로, 부른 쪽에서 즉시 터지게 해 뒀다.

### 하네스 (`crates/loom-track-n`)

구조는 §5 그대로: 외부 리스트(n≤50) → row 안 조건부 → 그 안 내부 리스트.
픽스처는 `demo/nested-list-derived.tsx` **하나**이고 다섯 arm은 그 한 소스를
`ListCandidate`만 바꿔 컴파일한 것이다(픽스처가 arm마다 다르면 반례가 후보의
결함인지 픽스처 차이인지 구별할 수 없다).

- **참조 모델**(`model.rs`): 후보 구현을 참조하지 않고 픽스처 템플릿에서 직접
  계산한다. 후보 의미론을 인자로 받는다(§5 문언 "후보 의미론별") — row 정체성이
  유지되는가(D/E) 재생성되는가(K). 그 차이가 실제로 DOM에 나타난다("한 번
  열렸다가 숨겨진" 상태가 재생성되면 사라진다).
- **RecordingDom**(`dom.rs`): 불변식 2를 **연산 시점에** 검사한다 — 앵커가
  목적지 부모의 직계 자식인가, 이동이 서브트리를 온전히 옮기는가, 무관한 노드의
  부모가 바뀌지 않는가. 최종 상태만 봐서는 못 잡는다(잘못된 앵커로 옮겼다가
  우연히 같은 자리에 오는 경우).
- **생성기**(`ops.rs`): PCG64 시드 고정, 외부 ops(삽입/삭제/이동/교환/역순) +
  토글 + 내부 ops를 무작위 인터리브, 길이 ≤200. 아이템 길이를 1~4로 섞는다 —
  술어와 slice가 아이템에서 파생되므로 길이가 균일하면 구간이 균일해져 불변식
  2의 판별력이 사라진다.
- **shrinking**: 델타 디버깅. **같은 계열**이 계속 터질 때만 채택한다 — 다른
  계열로 미끄러지면 최소형이 원래 문제를 설명하지 않는다.
- **시드 원장**: `--out`으로 JSON. 통과해도 무엇을 돌렸는지 남는다.

### 불변식 커버리지 (숨기지 않고 적음)

| §5 | 이 하네스 |
|----|-----------|
| 1 참조 모델 | 검사함 |
| 2 D DOM 구간 | 검사함 (연산 시점) |
| 3 E 순서 수렴 | 검사함 (매 tick `OrderKey` 전순서 ↔ DOM 순서 + 파티션 침범) |
| 4 F catch-up | **약한 형태만** |
| 5 flush 결정성 | 검사함 (dropped write 0 + 같은 시드 재현) |

**불변식 4에 대한 정직한 진술**: 컴파일러가 내는 조건부는 숨김에 `SetVisible`을
쓰고 patch System을 **동결하지 않는다**. "동결 중"이라는 상태가 이 arm에 없으므로
§5가 쓴 그대로의 catch-up은 잴 대상이 없다. 여기서 검사하는 것은 더 약한
"숨겨진 동안의 변경이 다시 보일 때 반영돼 있다"이다. **이걸 통과했다고 불변식 4가
검증됐다고 읽으면 안 된다** — 그건 공허한 확인이다. 진짜 형태를 재려면 `cond.rs`의
F 계열(fragment 분리 + `Frozen` 게이팅)을 컴파일러가 낼 수 있어야 한다(미구현).
`dropped_writes` 카운터도 이때 함께 신설했다(그 전까지 앵커 미해석은 `continue`로
조용히 삼켜지고 있었다 — 불변식 5의 "앵커 dangling 없음"이 잴 것이 없었다).

### 하네스가 즉시 잡은 결함 (다섯 번째 케이스)

미리 방출하는 **초기 row가 후보의 정체성 Component를 안 달고** 있었다
(`RowKey` / `OrderedKey`+`OrderKey`). 그래서 keyed 후보의 다음 재조정이 그
row들을 못 알아보고 전부 새로 spawn했고, 옛 노드는 DOM에 남아 내용이 중복됐다.
**K는 매번 전면 재생성이라 증상이 없어** 그때까지 드러나지 않았다 — 후보를 등록
가능하게 만들지 않았으면 영영 안 보였을 결함이다. 수리 후 통과.

### 음성 대조 (하네스가 실제로 잡는지)

| 주입한 결함 | 결과 |
|---|---|
| D의 row 정체성 제거 | D/D-LIS만 반례, K/E 무영향 (arm 특정성 확인) |
| 재배치 앵커 오염 | D/D-LIS에서 **대표 계열 DomSpan**, 터진 계열 [ReferenceModel, DomSpan] |
| `OrderKey` 재태그 일부 생략 | **E/E-LIS에서만 OrderConvergence** |

계열이 여럿 터질 때 **대표 계열**은 후보 특정 계열(2·3)을 고른다 — §5의 귀결이
계열별로 갈리는데 먼저 걸린 쪽(보통 1)만 적으면 원인 계열이 가려진다.

### 실행 상태 — **사이클은 아직 시작하지 않았다**

(**2026-10-17 주기**: 시간순 기록이다. 사이클 1은 바로 아래 절에서 같은 날(2026-08-08) 실행돼 G2를
통과했고, 2026-09-27에 v21 기반 재인증 R1·R2도 6 arm 전부 통과했다 — `RQ1-benchmark-protocol.md`
"§5 재인증 결과".)

§5는 사이클당 10^4 시퀀스, 최대 3사이클, 결과 확인 후 변경 불가로 사전 등록돼
있다. 지금까지 돌린 것은 **하네스 검증**이다: 1,000케이스 × 5 arm 전 arm 통과
(결정성 재현 40건/arm, 약 25초). 사전등록 10^4는 약 4분으로 실행 가능하다.
사이클 1을 시작하면 3사이클 중 하나를 소비하므로, 시작 여부는 별도 결정으로
남긴다. 실행기는 `--cases`가 10000이 아니면 출력에 "사이클이 아니다"를 스스로
찍는다.

## 트랙 N 사이클 1 실행 — G2 게이트 최초 통과 (2026-08-08)

§9 게이트 순서에서 미실행으로 남아 있던 **G2(트랙 N property)**를 처음 돌렸다.
사전등록 10^4 시퀀스 × 5 arm, **전부 통과**. 상세·경위·한계는
`research-questions/RQ1-dynamic-control-flow-scheduling.md`의 "트랙 N 사이클 1" 절,
원장은 `research-questions/data/rq1-track-n-cycle1.json`(동결 기반 `87d91de`).

**두 번 중단하고 고쳤다 — 둘 다 결과를 판정에 쓰기 전이다.**

1. 생성기가 §5 상한 n=50에 한 번도 닿지 않았다(랜덤워크로 4~5 근처). 중단
   시점에 arm 결과가 한 줄도 안 나왔고 원장도 없었다 — 아무것도 관측하지 않은
   채 고쳤다. 평균 길이 4.1 → 19.0.
2. 불변식 4의 검사가 **원리적으로 발동 불가**였다(불변식 1 통과 시에만 실행되는
   부분문자열 재비교). 이 계열은 불변식 1과 독립적으로 발동할 수 없으므로,
   검사식 대신 **시나리오 발생 횟수**로 바꿨다 — 0이면 미검사 경고를 찍는다.

이 세션에서 같은 공허성 패턴이 **다섯 번째**다(R3 데이터/새 테스트/기존 테스트/
불변식 4/`dropped_writes`). 부정형 단언은 메커니즘이 없을 때 자동으로 참이 되고,
그것은 데이터가 예측과 **어긋날 때**가 아니라 **부합할 때** 생긴다.

**남은 미이행 → 같은 날 해소**: 후보 C의 arm을 추가해 §9의 "전 후보 공통"을
충족시켰다. **6개 arm 전부 통과**(원장 재생성, 동결 기반 `528e991`). C를 넣는
과정에서 하네스가 결함 둘을 잡았다 — 파생값 미갱신, 그리고 `revalue`의 Commands가
같은 스케줄에서 적용되지 않아 생긴 **한 틱 지연**(사이트 시스템을 `.chain()`으로
묶어 해결). 둘 다 keyed 후보에서는 구조적으로 발생하지 않는 경로라, C arm이
없었으면 영영 검사되지 않았을 것이다.
