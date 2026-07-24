# RQ1. 동적 리스트/조건부 렌더링의 정적 스케줄링

## 문제

03에서 다룬 patch system 모델은 "엔티티 개수가 고정"이라는 전제 위에서 성립. 하지만
`{items.map(...)}`, 조건부 렌더링(`{cond && <X/>}`), `<Show>`류는 엔티티 개수/존재 여부
자체가 런타임에 바뀜. 순수 정적 컴파일타임 스케줄로는 표현 불가능.

## 후보안

> **2026-07-22 확장**: 초판의 A/B 2개에 더해, 6개 계보(fine-grained 컴파일러 / opcode
> 블록 렌더링 / Rust 선언형 UI / ECS 네이티브 패턴 / order-maintenance / incremental
> computation)에서 유도한 12개 안을 두 렌즈(4대 제약 위반 탐색, 비용·실효성 검증)로
> 적대 검증한 뒤 병합·기각을 거쳐 C~F를 추가함. **모든 항목은 미확정 가설.** 순서는
> 유망도 순이 아니라 메커니즘 계열 순 — 유망도 판정은 Phase 0/1의 몫.

### 확장 과정에서 나온 관찰

**다섯 계보의 독립 수렴.** keyed 리스트에 대해 서로 무관한 다섯 계보(Solid mapArray /
dom-expressions, Glimmer VM / Million.js, Leptos / Dioxus / Xilem, Bevy Children /
SceneSpawner, Adapton / Incr_map / differential dataflow)가 사실상 같은 형태 —
"POD key→entity 매핑 + row가 소유한 연속 DOM 구간 + 이동 전용 opcode + row 내부는
절대 diff하지 않음" — 로 수렴했다(후보 D). 수렴 자체가 실행 가능성의 방증이지만,
loom 위에서의 비용 검증은 아니다.

**A의 숨은 전제가 재정렬 문제의 원인이었다.** A는 암묵적으로 entity=아이템(keyed
의미론)을 가정했기에 재정렬이 미해결로 남았다. 이 가정을 명시하면 갈림길이 드러난다:
entity=위치로 뒤집으면 재정렬 문제 자체가 소거되고(C), entity=아이템을 유지하면
정체성 추적이 필요하다(D/E). 또한 A의 "개수만 비교"는 중간 삽입/삭제에서 아이템↔
엔티티 대응이 어긋나므로, **A는 독립 후보가 아니라 C의 골격 또는 D의 unkeyed
퇴화형으로만 정합적이다** — A의 지위를 아래처럼 재기술한다.

### 기존 후보 — 유지 + 보강

**A. Archetype 템플릿 + Command 기반 증분 spawn/despawn**
컴파일러가 "리스트 아이템 하나를 spawn하는 방법"을 템플릿으로 미리 생성. 리스트 길이 변화를
감지하는 전용 System이 diff 없이 **개수 차이만** spawn/despawn Command로 발행 (React의
key 기반 diff보다 훨씬 좁은 문제 — "몇 개 늘었다/줄었다"만 알면 됨).
*지위 재기술(2026-07-22)*: 위 관찰 참고 — 단독 후보가 아니라 C의 골격 또는 D의
unkeyed 퇴화형으로 존속.

A-보강 (계보 유도에서 나온 개선, 전부 미확정):
- spawn bundle에 초기 item 값 포함 → 스폰 후 재패치하는 2-pass 제거.
- 리스트가 부모의 유일한 자식이면 전량 삭제 = `clear_children` u16 opcode fast path
  (Svelte EACH_IS_CONTROLLED의 재유도).
- 미결 스케줄 결정: 신규 spawn 서브트리의 초기 patch를 같은 프레임에 완료하려면
  sync point 이후 Added 게이팅 후속 스테이지가 필요 — 스테이지 추가냐 1프레임
  지연이냐를 Phase 1 전에 확정할 것.

**B. 조건부는 Show/Hide 아닌 실제 despawn**
조건부 렌더링을 "숨김 처리"가 아니라 실제 entity despawn/spawn으로 처리. Resumability
관점에서 더 일관적(존재하지 않는 것은 직렬화할 필요 없음)이지만 spawn 비용이 매 토글마다
발생. *2026-07-22 추가*: 토글 시 서브트리 로컬 상태(입력값, 펼침 상태) 전량 소실도
B의 명시 비용으로 등재 — F가 이 지점의 대립 가설.

B-보강 — "B를 채택할 경우의 구현 명세". 두 계보(Glimmer TryOpcode/BlockOpcode,
SwiftUI \_ConditionalContent + Xilem OneOfN)에서 독립 유도된 두 안(CondBlock,
BranchSlot)을 병합:
- **경계 엔티티 + SpawnTable**: 토글과 무관하게 항상 존재하는 경계 entity
  (`CondBlock { anchor, branch_spawn_ops: [u16], live_branch }` 류, 완전 POD)가
  재스폰 위치와 resume 앵커를 겸함. 분기 스폰은 SpawnTable(u16 spawn_op →
  컴파일타임 생성 정적 스폰 스크립트, PatchTable 동형)로 표현 — **아래 "검증 필요"
  구항목 2번("archetype 템플릿이 POD 원칙 유지하며 구현 가능한가")에 대한 최초의
  구체적 답**. 경계 안은 부분 diff 없이 통째 폐기 후 재스폰(Glimmer 약 10년
  실운영이 방증, 단 loom의 ECS spawn 비용은 미검증).
- **닫힌 variant 집합**: `{cond && <X/>}`/삼항/else-if 체인을 컴파일타임에 닫힌
  u8 variant 집합으로 번호 매김 — 분기 판별 = 1바이트 비교, diff라 부를 것이 없음.
- **truthiness-에지 게이트**: `CondGate { last: bool }`로 조건 값이 아니라 truthiness
  에지(`!old != !new`)에서만 토글 발행 — 참→참 변화 무시(Solid Show equals 재유도).
  검증에서 발견된 공백: 브랜치 내부가 조건식 값 자체에 의존하는 경우
  (`{user && <p>{user.name}</p>}`) 값 채널 갱신 경로가 별도로 필요.
- **generation 스탬프 (조건부 채택)**: despawn된 분기를 향하던 stale LRef를 세대
  불일치로 폐기하는 안. 검증 판정 — Bevy generational entity id + 직렬화 시점
  captures 해소 검사로 대체 가능한 **중복 방어일 공산이 큼**. stale 재현 테스트를
  먼저 수행해 필요성부터 확정하고, 채택 시에도 중첩 재화신의 세대 리셋 충돌과
  리스트 내 조건부의 인스턴스 식별(site_id 불충분) 문제를 선수리.
- **hide 강등 특례 철회 권고**: "상태·LRef 없는 순수 정적 분기는 display 토글로 강등"
  안은 display:none 노드가 :nth-child/형제 셀렉터에 계속 참여하고 SSR HTML을
  키우므로 의미론적으로 투명하지 않다는 반박이 유효 — "안전한 최적화" 주장 철회.
- 설계 공백 등록 4건: (1) 중첩 조건부 재스폰 시 `Changed` 게이팅만으로는 새 경계
  entity가 영영 미초기화 — `Added<T>` OR 게이팅 필요. (2) `run_spawn(op, anchor)`에
  부모 스코프 값 전달용 `captures: [ComponentRef]` 파라미터 부재. (3) dead 분기
  SpawnTable+템플릿의 eager 번들 적재가 04의 lazy import 원칙과 충돌 — lazy chunk화
  시 비동기 토글 설계가 전무. (4) 토글 시 서브트리 로컬 상태 전량 소실(위에 등재).

### 신규 후보 (전부 미확정 가설)

**C. 고정 슬롯 값-통과(Slot-Fixed Value-Flow) — entity 정체성 = 리스트 위치 (unkeyed, opt-in 한정)**

- 원리 출처: fine-grained 컴파일러 계보 — Solid indexArray("인덱스 슬롯이 정체성,
  값이 그 자리를 통과"), Vue Vapor unkeyed 경로("commonLength까지 블록 유지, 값만
  교체"). 정체성 컨테이너를 아이템이 아니라 위치(슬롯 entity)에 붙인 변형.
- 메커니즘: 컨트롤러에 `ListLen_S { len: u32 }`, 슬롯 entity에 값 채널
  `SlotItem_S { value: Item }`(site별 newtype — 접근 집합이 타입 수준에서 disjoint,
  Bevy식 병렬 유지)과 불변 `SlotIdx_S`. 개수 변화 = A의 count-diff 그대로, 내용
  변화 = min(old,new)까지 달라진 슬롯만 값 write → 기존 `Changed` 게이팅 patch
  파이프라인 위임. **리스트 전용 diff 알고리즘이 아예 존재하지 않음** — No-VDOM을
  가장 강한 형태로 만족. 재정렬은 로직이 없는 방식으로 해결: 순서 변경 = 고정 슬롯에
  대한 값 재기록 n건으로 환원되어 문제 자체가 소거됨.
- 비용: head-insert/셔플이 O(n × row 내 binding 수) 값 패치(keyed의 O(이동 수) 노드
  이동 대비 깊은 row에서 불리). row-local 상태(포커스, 입력 중 텍스트)가 아이템이
  아니라 자리에 귀속 — keyed 의미론 기대와 충돌. 검증에서 추가 확인: (1) "달라진
  슬롯만 write"는 `Item: PartialEq` 강제 + O(n×|Item|) 비교이고, 비교를 생략하면
  coarse Changed가 전 슬롯 write로 증폭 — 두 비용 중 하나는 반드시 지불. (2)
  `Signal<Vec<Item>>` 원본 + 슬롯 사본의 이중 상주/이중 직렬화 — **"resumability가
  가장 깨끗하다"는 원안 자평은 페이로드 측면에서 과장**이며 canonical 소스 결정 필요.
  (3) heap 소유 Item(String/Vec)은 부분 memcpy 불성립. (4) 중첩 리스트에서 외부
  head-insert가 내부 sync를 전부 재트리거 — O(n_outer × n_inner) 증폭. (5) site별
  newtype monomorphization의 WASM 크기.
- 리스크: 이걸 default 리스트 의미론으로 삼는 것은 위험 — **양 렌즈 모두 opt-in
  (`<Index>` 상당) 경로 한정으로 검증 진행을 권고**. keyed 경로가 여전히 별도로
  필요하므로 이 후보 단독으로 RQ1은 닫히지 않음. 아이템 값에 따라 row 구조가 갈리면
  (`{items.map(i => i.x ? <A/> : <B/>)}`) 재정렬마다 archetype move 비용.
- 판별 실험: swap 2행 / 전체 shuffle / head-insert × row 깊이 2축(binding 1개 얕은
  row vs binding 8개+중첩 조건부 깊은 row)에서 D와 직접 비교. 측정 축에 직렬화
  페이로드와 WASM 바이너리 크기 추가(검증 요구). 채택: 얕은 row에서 keyed 동등
  이상 + 위치 귀속 의미론이 opt-in 문법으로 수용 가능. 기각: 깊은 row shuffle에서
  keyed 대비 3배 이상 열세 또는 이질 브랜치 archetype move가 지배 비용.

**D. Keyed 시퀀스 동기화 — key→entity 매핑 + row 소유 DOM 구간 + 이동 전용 opcode (5계보 병합)**

- 원리 출처(수렴): Solid mapArray/dom-expressions reconcileArrays("정체성이 상위
  레이어에서 보장되면 남는 것은 동일 집합 두 순열 간 1차원 최소 이동, 참조 동일성
  비교만"), Glimmer ListBlockOpcode.sync("리스트 갱신 = key 시퀀스 동기화, 출력은
  retain/insert/move/delete 4연산으로 닫힘") + Million.js 4방향 양끝 diff, Svelte 5
  each("재사용 단위 = 스코프 + 소유 연속 노드 구간, 재정렬은 site별 코드젠이 아닌
  단일 공유 런타임 함수"), Leptos `<For>`/Dioxus(prefix-suffix 절단 + LIS 최소 이동)/
  Xilem ElementSplice, Bevy Children("순서는 부모측 파생 리스트 한 곳") +
  SceneSpawner entity remap, Nominal Adapton("재사용 단위는 안정된 이름, 재정렬은
  이름→슬롯 재배선") + Incr_map(크기/값 채널 분리).
- 메커니즘(공통 코어): (1) 컨트롤러/앵커 entity에 POD key→entity 매핑, row entity에
  값 채널(site별 newtype Signal)과 DOM 구간(`NodeSpan`/`RowBounds` — SSR anchor
  마커 기반, entity가 소유한 연속 노드 범위). (2) site별 System은
  `Changed<Signal<Vec<Item>>>` 게이팅 후 key 시퀀스 diff — 출력은 정확히 4연산:
  retain = 값이 실제로 달라진 row만 Signal write(이후는 기존 Changed 파이프라인,
  reconcile은 row 내부를 절대 열지 않음) / insert = SpawnTable Commands / delete =
  despawn_subtree Commands / move = DOM 구간 통이동 opcode(entity·archetype·상태
  불변). (3) 재정렬 로직은 site별 코드젠이 아니라 단일 공유 런타임 함수 + 얇은
  wrapper(Svelte 재유도 — WASM 크기 완화). (4) No-VDOM 허용 논거는 docs/03
  "No-VDOM 판별 기준" 조문(P1~P3·N1~N5, 2026-07-22 신설)으로 명문화되어 **반입 조건
  이행됨** — 초기 "3논거"(동등성 비교만 / 구조 재귀 없음 / 1차원 순열) 표현은 조문
  P1(비교 피연산자 제한)로 대체. D는 산문 명세 단계 심사 통과(잠정; "LIS 포함 keyed
  reconcile"로 명명할 것), 구현 제출 시 재판정하며 심사 기록에 변형 축(순서 저장 위치,
  retain write 방식) 명시 필요(N5).
- 변형 축(병합 시 남긴 미결정, Phase 1에서 판정):
  - 순서 저장 위치: 컨트롤러 Vec(keys+rows; 4개 계보) vs per-row order 필드에서
    매번 재구성(KeyedRegion; 부수 상태 제로 → 직렬화 공짜이나 append 1건에도
    O(n log n) 정렬). Vec 쪽은 부분 memcpy 이탈 + O(n) 상주가 비용.
  - 이동 최소화: 순열 매칭만(reconcileArrays 계보, LIS 없음) vs LIS(Dioxus/Leptos
    계보) — **"O(이동 수)"를 주장하려면 LIS가 필요함을 검증이 확인**, 논거를 "LIS
    포함 keyed reconcile"로 정직하게 기술할 것.
  - move의 분류: 비구조 값 변경으로 취급해 즉시성 확보(ListSeq permute 변형) vs
    Commands와 같은 큐에서 순서 보존 — 검증 결과 신규 spawn row와 move의 인터리빙
    (move의 기준 anchor가 미스폰 row일 수 있음) 때문에 **사실상 단일 순서 큐로의
    병합 flush 규약이 필요** — 채택 전 설계 결정 사항이며 formal-verification 후보.
- 비용(검증에서 확정된 것 포함): 아이템 데이터 이중 저장(`Signal<Vec<Item>>` + row별
  사본 — 메모리·직렬화 페이로드 약 2배, 원안들의 비용 분석에서 공통 누락). retain
  판정의 `Item: PartialEq` + O(n) 비교 vs 무조건 write의 Changed 폭풍 양자택일.
  key 계산은 POD 제약상 u16 key_op(문법 제약된 표현식)로만 — 임의 함수 key 불가,
  u64 해시 충돌 정책(완전 동등성 경로) 미설계. row당 anchor comment의 SSR 바이트.
  site별 monomorphization의 WASM 크기. resume 시 anchor/경로 재획득이 리스트에서는
  O(n) 마커 walk(사실상 mini-hydration)로 퇴화 + Entity id remap 인프라 필요. retain
  write의 같은 프레임 가시성을 위한 .after() 간선 체인. move(insertBefore/구간
  재삽입)는 입력값은 보존하나 **포커스·CSS 애니메이션·iframe 상태는 리셋** — 판별
  실험의 "포커스 보존" 기준은 수정 필요. append-only 사이트가 keyed 전액 비용을
  내는 문제 — unkeyed(A 퇴화형/C)로의 정적 opt-out 경로를 명세할 것.
- 리스크: 중첩 동적 블록(리스트>조건부>리스트)에서 DOM 구간 불변식이 깨지면 move가
  노드를 찢는 정합성 버그 — 구조적으로 재발하기 쉬운 유형이며 이 프로젝트 단일 최대
  복잡도 지점. row 내부 중첩 블록 성장 시 구간 경계 갱신 규칙 미설계(Leptos/Solid가
  정수 인덱스 대신 실제 anchor 노드를 쓰는 이유가 이 문제). "좁은 전용 알고리즘"
  논거가 설계 리뷰에서 기각될 가능성(No-VDOM 제약 심사 대상).
- 판별 실험: (1) shuffle/swap/head-insert × row 깊이 축에서 C 및 "재정렬=despawn+
  respawn" 대조군과 3자 비교 — keyed가 이기기 시작하는 row 깊이 k 확정. (2)
  serialize → resume → 즉시 shuffle로 구간 lazy 복원 정합성과 비용 스파이크. (3)
  3중 중첩 구조에서 move 정합성 property-based test. 측정 항목에 직렬화 페이로드와
  WASM 바이너리 크기 포함(검증 요구). 기각: 구간 유지 총비용이 despawn+respawn보다
  비싸거나 중첩 정합성 버그가 구조적으로 재발 → 재정렬=despawn+respawn + C(opt-in)
  조합으로 후퇴.

**E. OrderKey 밀집 순서 태그 — 순서를 per-entity 데이터로, 이동은 moveBefore 단일 opcode**

- 원리 출처: order-maintenance 계보 — fractional indexing(Figma)의 "순서는 이웃과의
  상대 위치가 아니라 각 항목이 소유한 정렬 키 데이터" + Dietz–Sleator/Bender의
  고정 폭 정수 태그 + 정밀도 고갈 시 국소 재라벨링(가변 길이 fractional 문자열은
  부분 memcpy와 상성이 나빠 기각). CRDT 시퀀스(Logoot/LSEQ)에서는 반면교사만:
  정렬 키에 정체성을 섞으면 이동 시 정체성이 깨짐 → Entity id(정체성)와
  OrderKey(순서)를 별도 Component로 분리. DOM 반영은 WHATWG `moveBefore()`(상태
  보존 원자 이동; Chrome/Edge 133+, Firefox 144+, Safari 미구현 — 2026-07 확인).
- 메커니즘: `OrderKey(u64)` 밀집 태그 + `ItemIdentity { list_id, key_hash }` (전부
  고정 폭 POD). Reconcile System이 해시 매칭으로 신규→spawn Command(이웃 태그
  중간값 발급), 소멸→despawn Command, **잔존 이동→OrderKey 값 write만**(구조 변경
  없음). Move patch System이 `Changed<OrderKey>` 게이팅으로 successor 검색 후
  `OP_MOVE_BEFORE` 1회(Safari는 insertBefore 폴백 — World에 상태가 있으므로
  프레임워크 상태는 무손실, 브라우저 네이티브 상태만 점진적 향상). gap 고갈 시
  sync point 전용 Relabel System이 국소 재라벨(상대 순서 불변이므로
  bypass-change-detection 쓰기 → DOM 이동 유발 안 함). 조건부도 정적 형제에
  컴파일타임 상수 OrderKey를 배정하면 anchor comment 없이 동일한 위치 해석으로 수렴.
- D와의 차이: 컨트롤러측 Vec이 없어 archetype 부분 memcpy와 가장 정합. 순서가
  diff의 출력이 아니라 per-entity 데이터라는 점이 계열 구분의 본질.
- 비용·수정 필수 3건(검증에서 확정): (1) "O(이동 수)" 주장은 LIS 계산(O(n log n))
  없이는 불성립 — 명세에 LIS를 명시하고 No-VDOM 논증도 "LIS 포함"으로 재제출.
  (2026-07-22 갱신: docs/03 조문 신설로 심사 경로 확정 — reconcile System + Move patch
  System을 묶은 심사 단위로 산문 명세 단계 심사 통과(잠정). Relabel System의 심사 단위
  포함 여부는 판정 기록에 명시할 것이며, bypass-change-detection은 조문의 "만능 예외
  창구 아님" 부기에 따라 별도 심사 대상으로 남음.)
  (2) **정확성 결함**: `Changed<OrderKey>` 무순서 순회로 moveBefore를 적용하면
  틀린 DOM 순서로 수렴하는 반례 존재([A,B,C]→[B,C,A]) — 이동 집합을 OrderKey
  내림차순 정렬 후 적용 필수(O(m log m)). (3) successor 검색은 실질적으로 파티션별
  정렬 인덱스 = World 밖 파생 상태를 요구 — resume 시 O(n log n) 재구축이 사실상
  확정 비용("복원 작업 0" 주장은 DOM 순서에만 성립).
- 추가 비용/리스크: bypass-change-detection이 "Component 쓰기는 Changed를 트립한다"
  는 03 코어 불변식에 예외 도입 — OrderKey 사용자 구독 금지 규칙 등 정합성 장치
  필요. 단일 OrderKey 타입이면 모든 리스트의 System이 write 충돌로 직렬 실행 —
  "리스트 간 병렬 자동 성립" 주장은 거짓, site별 monomorphize 필요. key_hash u64
  충돌 시 무감지 상태 오염(원본 키 보관은 고정폭 제약과 충돌 — 128비트 해시 등
  검토). 중첩 리스트에서 정적 list_id만으로 파티션 불가 — 부모 entity 포함 필요.
  실 상주 비용은 정렬 패딩으로 24B/entity(원안 20B 주장 부정확). Relabel은
  amortized O(log n)일 뿐 단발 최악은 파티션 전체 — 프레임 예산 기준을 worst-case로.
- 판별 실험: swap rows / shuffle 10%를 (i) A 단독(재정렬=despawn+respawn) vs (ii)
  A+OrderKey로 비교 — DOM 연산 수, 프레임 시간, resume 페이로드. 맨 앞 반복 삽입
  10^6회 스트레스에서 재라벨 빈도와 최대 sync point 정지시간. 셔플 정합성(적용 순서
  반례) 테스트 추가(검증 요구). 기각: 유의차 없음, keyed 매칭 비용이 절감분 상쇄,
  또는 No-VDOM 논증 기각.

**F. 보존형 조건부 토글 — 토글 = 배치/수요 변경, despawn은 승격 계층 (B의 대립 가설, 3변형 병합)**

- 원리 출처(수렴): Bevy Visibility 3분할("자주 토글되는 것은 값 변경, 정말 사라지는
  것만 구조 변경" — ECS 진영조차 표시 토글을 despawn으로 하지 않음) + bevy_ui의
  Display::None/Visibility::Hidden 2단 강도, order-maintenance의 "정체성·상태와
  배치(placement)의 분리"(moveBefore가 플랫폼 수준에서 승인한 원리의 일반화),
  Adapton의 수요 주도 2단계 전파(dirtying은 싸게 계속, cleaning은 수요 복귀 시점에
  변경분만). Vue KeepAlive / React Activity(Offscreen)가 프로덕션 선례.
- 공통 코어: 조건부 루트에 배치/동결 상태 POD 하나(`Placement(u8)` /
  `CondIntent`+`CondEffective(bool)` / `Frozen` 태그). 토글 = 값 쓰기(또는 Frozen
  add/remove Commands) + `OP_DETACH_TO_FRAGMENT`/`OP_ATTACH` opcode — DOM은
  파괴하지 않고 fragment로 분리 보관(핸들은 비직렬화 사이드테이블, Component엔
  정수 인덱스만). **entity와 내부 Signal 상태가 토글을 넘어 보존됨**(탭 입력값,
  펼침 상태) — 이것이 B가 원리적으로 제공할 수 없는 능력이며 이 후보의 진짜 논거.
  검증 판정: 원안들의 "sync point 회피가 질적 차이" 논거는 과장(Commands flush는
  매 프레임 있음; 실차이는 spawn 작업량) — 논거를 상태 보존으로 재배치할 것.
- despawn(B)은 폐기가 아니라 승격 계층으로 강등: (a) SSR 시점 false인 블록은 entity
  미탄생 상태로 두고 첫 true에 lazy spawn(이후 값 게이팅 운영) — 덕분에 **초기
  server→client wake-up 페이로드는 B와 사실상 동일**하며, 페이로드 증가는
  클라이언트측 pause/resume 시나리오에서만 노출됨(원안의 자체 비용 분석이 이 지점을
  잘못 짚었음을 검증이 교정). (b) "무거운 서브트리 + 드문 토글"은 개발자 힌트
  (@rare)로 매 토글 despawn.
- 잠자는 System 게이팅의 3변형(병합 시 남긴 미결정): (i) 값 필터 주입
  (`CondEffective==true` — 상시 런타임 분기, 파생값은 직렬화 제외 후 resume 시
  전파 System 1회로 재구성), (ii) 필터 없이 detached DOM에 그대로 패치(연결 안 된
  DOM 쓰기는 저렴하다는 가설 — 죽은 작업 누적 위험), (iii) `Without<Frozen>` marker
  필터(archetype 이동 2회/토글). marker-vs-value 비용 구조 자체는 1차 문서 미확인
  (미검증 기억 기반)이므로 전제부터 실측 대상.
- **공통 난점 — 재부착 stale DOM**: 세 변형이 독립적으로 같은 문제에 부딪힘. 꺼진
  동안 스킵된 patch는 Changed 틱이 소모되어 재부착 시 자동 재실행되지 않음 →
  dirty 추적 또는 catch-up이 필수이고, "reattach = opcode 1회" 주장은 그대로는
  성립하지 않음. Frozen 변형의 버전 카운터 쌍(binding별 applied_version vs Signal별
  version, 해동 시 불일치만 재패치 — Adapton cleaning의 정수 재유도)이 유일한 명시적
  해법이나, 버전을 공유 DomBinding 타입에 넣으면 모든 patch System이 그 타입의
  writer가 되어 **patch 병렬성이 매 프레임 붕괴** — site별 버전 Component 분리
  코드젠이 전제. 파생/계산 체인이 있는 scope의 catch-up은 위상 순서 재실행이 필요해
  컴파일 산출물이 하나 더 늘어남.
- 기타 비용/리스크(검증 확정분): 분리된 DocumentFragment는 직렬화 불가능한
  클라이언트 메모리 상태 — pause/resume 시 "DOM 보존" 이점이 직렬화 경계를 넘지
  못함. detach/attach 경로는 브라우저 네이티브 상태(iframe, focus, 재생 중 media)를
  보존하지 않음 — 보존되는 것은 ECS 상태뿐이라는 한계를 과장하지 말 것. 동결/분리
  상주 메모리에 상한·강등 정책 필요(라우트 전환·중첩 아코디언 누적 시 무한 상주).
  값 필터 변형은 CondEffective가 서브트리 전 entity에 존재해야 해 토글 1회 =
  O(서브트리) 쓰기 + 컬럼 스킵 직렬화 인프라 필요. tier/전략 선택 기준("빈번 vs
  드문"을 컴파일타임에 모름)이 새 복잡도 표면. 외부 LRef가 블록 내부 ComponentRef를
  캡처한 채 tier-2 despawn되면 dangling — 컴파일러 escape 분석 또는 despawn 강등
  금지 규칙 필요(원안의 검사 방향이 뒤집혀 있었음을 검증이 교정).
- RQ3 접속점(2026-07-23): RQ3 잠정 규칙은 **island host를 포함한 조건부 분기에 한해**
  토글-off 시 F(보존형)가 아닌 despawn 의미론을 강제함(cascade가 "실제 DOM 파괴"에
  결속되기 때문). B/F 실측에서 F가 이기면 RQ3 쪽 조항이 재개정됨 — 판별 실험 결과
  보고 시 이 접속점을 함께 판정할 것.
- 판별 실험: 토글 빈도(초당 0.1~60회) × 서브트리 크기(entity 1~1000) 2차원 그리드에서
  4구현 비교 — [B: 매번 despawn/spawn] vs [값 게이팅+DOM 분리] vs
  [lazy-spawn-then-hide] vs [**CSS 은닉(v-show형) — 검증이 추가를 요구한 제3안**:
  핸들 테이블·실체화 경로가 모두 불필요해 더 쌀 수 있음]. 측정: 토글당 시간,
  재부착 replay 비용, 토글당 전파 쓰기 폭, 동결 중 상류 Signal 변경률 0/10/100%별
  catch-up 비용 곡선(B를 역전하는 교차점), 메모리 상주, 직렬화 페이로드, resume
  시간. 기각: 값 게이팅이 이기는 빈도 구간이 실사용 분포 밖이거나 B의 spawn 비용이
  전 영역에서 무시 가능(<1ms/100 entity) — 그 경우 B 유지가 결론.

## 고려했으나 기각

**비수요 분기 동결의 ColdScope collapse 직렬화 규칙** (incremental 계보 원안의 일부).
12개 유도안 중 두 렌즈 모두에서 전면 기각된 것은 없었고, 24개 판정 전체에서 발견된
유일한 경성 제약 위반이 이것: 동결 entity를 직렬화에서 제외하되 Signal 상태를
ColdScope blob으로 추출한다는 규칙은 (1) "페이로드는 B와 동일" 주장과 자기모순
(blob을 넣으면 B보다 크고, 빼면 상태 보존이 resume 경계에서 사라져 B로 퇴화)이고,
(2) collapse된 entity를 가리키는 LRef captures가 dangling이 되는데 resume 후 재spawn은
새 entity id를 만들어 재매핑 없이는 핸들러 복원 불가 — Resumability 제약 위반.
축소 수리안(동결을 런타임 전용 캐시로 한정, 직렬화 경계에서는 B와 동일하게 퇴화)만
후보 F의 freeze 변형으로 존속시켰다.

## 검증 필요 (2026-07-22 갱신)

초판 2개 항목은 다음으로 대체/구체화됨:

- ~~리스트 재정렬을 despawn/respawn 없이 처리할 방법이 있는가~~ → 후보 C(문제 소거)/
  D(keyed 이동)/E(순서 태그)가 세 갈래 답을 제시. Phase 1 판별 실험으로 교차 비교.
- ~~archetype 템플릿 방식이 Component-POD 원칙(02)을 유지하면서 구현 가능한가~~ →
  B-보강의 SpawnTable이 구체적 답 제시. 스폰 스크립트의 captures 파라미터화와
  lazy chunk 충돌은 미해결로 남음.

신규 등록 (여러 후보에 공통인 인프라 결정 — 후보 채택과 무관하게 필요):

- ~~No-VDOM 판별 기준의 명문화 (docs/03에 반영 필요)~~ → **이행됨(2026-07-22)**:
  docs/03 "No-VDOM 판별 기준" 조문 신설(양성 기준 P1~P3, 전역 금지 N1~N4, 절차 N5,
  판정례 8건). 초기 3논거는 P1으로 대체·정밀화. 8사례 판별력 시험과 9사례 회귀 재시험
  (N4 표적 포함)을 통과한 확정본. D·E는 심사 자격 획득(산문 단계 잠정 통과) — 채택은
  여전히 Phase 1 판별 실험의 몫이며 각 후보의 기각 조건은 그대로 유효.
- **이중 지연 큐 병합 flush 규약**: 구조 변경 Commands와 DOM op 버퍼가 같은 anchor를
  대상으로 섞일 때의 결정적 교차 순서(신규 spawn row의 DOM은 flush 후에야 존재) —
  formal-verification 대상 후보.
- **스냅샷 채취 시점 규칙**: 직렬화는 모든 Commands/patch flush 이후의 프레임
  경계에서만 — 큐에 걸린 토글이 있는 채 직렬화하면 World와 SSR HTML이 불일치.
- **Entity id remap 인프라**: keys/rows Vec, children_root, LRef captures 등 스냅샷
  내 모든 Entity 참조에 파급되는 전역 remap 패스 — "부분 memcpy 지향"이 memcpy+fixup
  으로 격하되는 정도를 RQ2에서 재산정.
- **POD 정의 확정**: 엄격 memcpy-safe인가, "직렬화 가능한 순수 데이터"인가 —
  Vec/SmallVec 내장 Component(keys, rows)의 허용 여부가 D 계열 후보의 직렬화 경로를
  가름.
- **resume 후 DOM 노드 재획득 프로토콜**: 동적 리스트에서 정수 경로는 무효화되므로
  마커 기반 O(n) walk(mini-hydration)가 현실 — 비용 상한과 SSR 마커 방출 스펙.
- **신규 spawn 서브트리의 same-frame 초기 patch**: Added 게이팅 후속 스테이지 vs
  1프레임 지연.

## 상태

후보 확장 완료(2026-07-22): 6개 계보에서 12개 안 유도 → 2렌즈 적대 검증(24개 판정)
→ 병합·기각 후 C~F 등재, A/B 보강. 리스트 문제(C/D/E 계열)와 조건부 문제(B/F 계열)는
부분적으로 독립이므로 트랙을 분리해 판정.

**Phase 0 산출 확정(2026-07-23)**: 병익님 지시("어느 선택지에도 가중치를 두지 않고
공평하게")에 따라 Phase 0 산출물을 "후보 하나의 잠정 선택"이 아니라 **공평 사전 등록
판별 프로토콜**로 확정 — `RQ1-benchmark-protocol.md` (v1.1; 후보별 옹호자 6 + 방법론
감사 1의 교차 감사와 그 처리 기록 포함). "이렇게 가정하고 진행한다"의 내용: 공통
기반(프로토콜 §2)을 Phase 1에서 구축하고, C/D/E/K(리스트 트랙)·B/F/대조군(조건부
트랙)을 사전 등록된 셀·정합성 게이트·임계값대로 교차 실측한다. **선택은 실측이 내린다.**
Phase 1 프로토타입에서 가장 먼저 부딪힐 문제라는 판단은 유지.

### Phase 1 트랙 L 진행 상황 (2026-07-23) — C/D/E 구현·검증·1차 실측 완료

**구현**: `crates/loom-runtime/src/list.rs`에 K(기존)에 더해 **C, D, E를 모두 실제
Bevy System으로 구현**함 — 세 후보 모두 서술이 아니라 컴파일·테스트 가능한 코드다.

- **C**: `list_reconcile_c_system` — entity 정체성 = 위치. `SlotValue`로 값 비교 후
  변경 시에만 write(RQ1이 지적한 O(n×|Item|) 비용을 실제로 지불하는 구현).
- **D**: `list_reconcile_d_system` — `RowKey`(key=표시 텍스트, Phase 1 단순화) +
  컨트롤러 `ListRows` Vec. **순열 매칭만(LIS 미적용)** 변형 — RQ1이 명시한 변형 축 중
  하나를 선택 구현했고, 다른 변형(LIS 기반 최소 이동)은 미구현.
- **E**: `list_reconcile_e_system` — `OrderKey`(u64, per-entity) + `OrderKeyMembers`
  (순서 없는 멤버십 집합, 컨트롤러 Vec 없음 — D와의 아키텍처 차이를 실제로 구현).
  **밀집 재태깅(0..n-1) 방식**만 구현 — RQ1의 fractional gap 할당(이웃 중간값 발급,
  국소 재라벨링)은 미구현.
- 공유 재배치 로직(`place_in_order`)은 **오른쪽에서 왼쪽으로** 처리해 앵커가 항상
  이미 확정된 위치를 가리키도록 보장 — 처음 구현에서 이 불변식을 놓쳐 "새로 스폰된
  행이 연속으로 붙거나, 나중에 이동할 유지 행을 앵커로 삼는" 실제 버그가 났고
  (remove_middle 등에서 오배치 확인), right-to-left 앵커링으로 재작성해 해결함.
- `DomWrite::RemoveNode`가 `Entity`가 아니라 `NodeHandle`을 직접 담도록 정정함 —
  despawn은 Commands로 지연되어 `Schedule::run` 종료 시점에 이미 적용되므로, 그
  *이후* 실행되는 `LoomApp::tick`의 write 적용 루프에서 `Entity`로 재조회하면 이미
  없는 엔티티를 찾는 꼴이었음(실제로 재현되어 수정한 버그).
- Bevy 쿼리 충돌(E의 읽기 쿼리와 쓰기 쿼리가 같은 `OrderKey` 컴포넌트를 다른
  System 파라미터로 동시에 요청해 B0001 패닉)도 실제로 발생해, 단일 `&mut` 쿼리로
  병합해 해결.

**네이티브 검증**: `crates/loom-runtime/tests/list_candidates.rs` — 21개 테스트
(mount/no-op/swap/reverse/append/head-insert/remove-middle/shuffle-mixed/다중 틱
연속 편집), 특히 **D/E가 유지되는 행의 `NodeHandle`(entity 정체성)을 실제로
보존**하고 K는 항상 재생성한다는 것을 핸들 비교로 직접 검증함 — 이게 K 대비
C/D/E의 존재 이유이므로.

**1차 실측(2026-07-23, n=200 단일 크기)**: `crates/loom-bench` 초판으로 K/C/D/E ×
5워크로드 × 20회, Chromium만, median/min/max만 보고. 방향성(K가 최하위)은 이론과
일치했으나 스코프가 좁아(단일 크기·단일 브라우저·거친 통계) 아래 확장으로 대체됨.
원본 데이터는 provenance로 보존: `research-questions/data/rq1-phase1-track-l-core-grid.json`.

### 2차 실측 — N-스케일링·POD축·전체 분포 통계·2브라우저 확장, 그리고 실제로 발견한 성능 버그

**확장한 것**: `crates/loom-bench`를 (1) N-스케일링 n∈{100,1,000,10,000}(String,
전 크기), (2) POD 대 heap 아이템 축(`ItemId(u32)` POD 타입을 n=1,000에서 부분
교차검증 — 전 크기 아님, 명시), (3) 원시 샘플 전량 방출로 바꿔 median/p25/p75/p95/
min/max 전체 분포를 계산 가능하게, (4) Chromium **및 Firefox** 양쪽에서 실행하도록
확장. 여전히 아닌 것: row 깊이(R1/R2/R3) 축, 원본 앱 규모 축, paired bootstrap,
§2.4 형식적 최선형 체크리스트 대조.

**실행 중 실제로 성능 버그를 발견함 — 조용히 넘기지 않고 수정 후 재측정함.**
1차 확장 실행(수정 전) 결과, n=10,000에서 D/E가 K보다 **최대 25배 느림**(Firefox
swap: K 24ms vs D 330ms/E 331ms)이 나왔다 — "덜 건드리는 D/E가 항상 이긴다"는
순진한 기대와 정반대. 원인을 추적한 결과 `place_in_order`의 "이미 인접한가?"
판정이 `Vec::position()` 선형 탐색이었다 — n개 슬롯 각각에 대해 O(n) 탐색을 하므로
**실제로는 O(n²)**(움직이지 않는 행까지 포함해서!)였다. RQ1의 설계 의도(O(n) 매칭)와
전혀 다른 우발적 비효율이라 문서화만 하고 넘어갈 결과가 아니라 실제로 고쳐야 하는
버그로 판정 — `place_in_order`를 이중 연결 리스트(`Chain`, `next`/`prev` 해시맵)
기반으로 재작성해 인접성 확인과 삽입/제거를 O(1)로 만들고, 22개 네이티브 테스트
(대규모 정합성 테스트 1개 추가 포함, n=500 전체 반전+부분 셔플)로 정합성을 재확인한
뒤 재측정함. 수정 전/후 원시 데이터 둘 다 보존:
`research-questions/data/rq1-phase1-track-l-core-grid-v2-*.json`(버그 있던 실행),
`-v3-*.json`(수정 후, 현재 유효).

**수정 후에도 남은, 더 미묘하고 더 중요한 발견 — LIS 없이는 "이동 최소화" 주장이
성립하지 않는다는 RQ1 Phase 0 예측이 실측으로 확인됨.** O(n²)→O(1) 수정 후에도
**append/head_insert/remove_middle**(국소적 변경 — 앞/뒤/중간 한 지점만 바뀜)에서는
D/E가 K와 동등하거나 우세한 반면, **swap(양끝 교환)/shuffle10**(전역적으로 재배치)
에서는 D/E가 여전히 K보다 뚜렷이 느렸다(Chromium n=10,000: swap K=13.05ms vs
D=23.65ms/E=21.65ms; shuffle10 K=15.05ms vs D=27.45ms/E=27.55ms — Firefox
shuffle10은 K=24ms vs D=123ms/E=125.5ms로 격차가 더 큼). 원인을 손으로 추적한 결과
(n=4~5 예시로 알고리즘을 직접 시뮬레이션): 이 조문서 자신이 이미 등록해 둔 변형 축
"순열 매칭만(LIS 없음) vs LIS 기반 최소 이동" 중 **전자만 구현**했기 때문이다 —
"양끝 교환"은 겉보기엔 원소 2개만 옮기면 되지만, 내 알고리즘은 오른쪽에서 왼쪽으로
훑으며 "각 유지 행의 현재 다음-이웃이 목표와 같은가"만 보므로, 교환된 두 원소
사이의 **중간 원소 전부**가 "다음 이웃이 바뀌었다"는 이유로 불필요하게 재배치
대상이 된다(직접 시뮬레이션: n=5 양끝 교환에서 5개 중 4개가 이동 판정을 받음).
반대로 국소 변경(append 등)은 나머지 항목들의 상대적 인접 관계가 전혀 안 깨지므로
이동이 0~1건에 그친다. **이것은 RQ1이 이미 Phase 0에서 적대적 검증으로 예측한
바로 그 결함이다** — "'O(이동 수)'를 주장하려면 LIS가 필요함을 검증이 확인"이라고
후보 D/E 항목에 이미 적어 두었는데, 이번이 그 예측을 실측으로 검증한 것.
**LIS 기반 최소 이동 변형은 아직 구현하지 않음** — 이게 정말 swap/shuffle10에서
격차를 좁히는지는 후속 작업.

**흥미로운 부수 확인**: `Element.moveBefore`(WHATWG 신규 API, RQ1 후보 E가
언급)가 테스트에 쓴 Chromium 147과 Firefox 148 양쪽에서 실제로
`typeof Element.prototype.moveBefore === "function"`으로 확인되어, E의 `reposition`
호출이 (Safari 폴백 없이) 실제 네이티브 경로를 탔다.

**대표 수치 표 (Chromium, n=10,000, String, post-fix, median ms)**:

| 워크로드 | K | C | D | E |
|---|---|---|---|---|
| append | 15.45 | 0.30 | **12.15** | 12.80 |
| head_insert | 11.55 | **5.35** | 12.25 | 12.80 |
| remove_middle | 14.10 | **3.25** | 12.05 | 12.35 |
| shuffle10 | **15.05** | 5.85 | 27.45 | 27.55 |
| swap | **13.05** | 0.30 | 23.65 | 21.65 |

(C는 재정렬 자체가 없으므로 모든 워크로드에서 압도적으로 낮음 — 단 opt-in 한정
스코프라는 조건은 그대로 유효. 굵게 표시한 것이 각 행의 승자.)

**이 실측이 아닌 것 (숨기지 않음)**: `RQ1-benchmark-protocol.md`가 사전 등록한 전체
실행이 여전히 아니다 — (1) row 깊이 축(R1/R2/R3), 원본 앱 규모 축, 고위험 위젯 등
프로토콜의 전체 워크로드 그리드 미실행, (2) POD 축은 n=1,000 한 크기만(전 크기
교차는 아님), (3) paired bootstrap/등가 대역 판정 없음(percentile까지만), (4) 대조군
최선형 체크리스트(§2.4) 형식 대조는 하지 않음, (5) 단일 머신(하드웨어 스로틀링/반복
캠페인 없음). **결론**: RQ1 잠정 선택은 여전히 미착수. 이번 확장 실측이 준 것은
방향성 신호를 넘어 **후보 간 트레이드오프의 실제 구조**다 — D/E는 국소 변경에
강하고 전역 재배치에 약하며(LIS 없이는), 그 약점이 두 브라우저·두 아이템 타입·
N-스케일링 전반에서 일관되게 재현된다. LIS 구현 여부가 이 트레이드오프를 얼마나
바꾸는지가 RQ1의 다음 실측 우선순위로 등록됨.

### 3차 실측 — LIS 기반 최소 이동 변형(D-LIS/E-LIS) 구현 및 검증 (2026-07-24)

**구현**: `crates/loom-runtime/src/list.rs`에 `lis_indices`(patience sorting +
predecessor 역추적, O(n log n))와 `place_in_order_lis`(LIS로 "제자리 유지" 대상을
먼저 증명한 뒤, 그 집합만 이동에서 제외하고 나머지만 `Chain` 기반 O(1) 재배치를
수행)를 추가. D/E를 대체하지 않고 **별도 후보 D-LIS/E-LIS**로 나란히 추가해
`list_reconcile_d_lis_system`/`list_reconcile_e_lis_system`으로 노출 — 순열
매칭만(D/E) vs LIS 기반 최소 이동(D-LIS/E-LIS)이 같은 벤치마크 한 페이지에서
동시에 비교되도록 함. 네이티브 테스트 18개 신규 추가(단위 테스트
`list::lis_tests::*` 5개 — 예: `[4,1,2,3,0]` 같은 손 계산 가능한 순열에서 LIS
위치 집합이 정확한지, 빈 입력·완전 역순·헤드 삽입 패턴 등 경계 케이스 — 및 통합
테스트 `candidate_d_lis::*` 9개 + `candidate_e_lis::*` 4개, 그 중
`swap_ends_costs_only_two_repositions*`는 "양끝 교환에서 실제로 재배치 연산이
정확히 2건만 발생하는가"를 직접 단언). `cargo test --workspace` 전체 재실행 —
회귀 없이 전부 통과(`list_candidates.rs` 35개 포함).

**측정**: `crates/loom-bench`를 K/C/D/E/D-LIS/E-LIS 6후보 동시 비교로 확장(
`all_makers()` 배열화로 후보 추가/제거가 한 곳에서 관리되도록 리팩터링). 동일
워크로드 그리드(N∈{100,1,000,10,000}×5워크로드)로 Chromium·Firefox 재실행. 원시
데이터: `research-questions/data/rq1-phase1-track-l-core-grid-v5-{chromium,
firefox}.json`.

**결과 — swap은 격차가 사실상 닫히고, shuffle10은 좁혀지되 닫히지 않음**
(n=10,000, String, median ms):

| 워크로드 | K | C | D | E | D-LIS | E-LIS |
|---|---|---|---|---|---|---|
| append | 12.85 | **0.30** | 12.45 | 12.05 | 13.25 | 13.55 |
| head_insert | 10.90 | **5.25** | 12.65 | 12.85 | 12.45 | 13.45 |
| remove_middle | 13.50 | **3.00** | 12.20 | 11.75 | 12.75 | 13.85 |
| shuffle10 | 12.40 | **5.20** | 26.30 | 26.30 | 19.35 | 20.95 |
| swap | 12.65 | **0.30** | 22.55 | 22.40 | 12.55 | 13.10 |

(Chromium. Firefox는 절대값은 다르나 같은 패턴 — swap: K=24.0 vs D=42.0/E=43.5 vs
**D-LIS=24.0/E-LIS=25.5**; shuffle10: K=26.0 vs D=130.0/E=134.5 vs
D-LIS=117.5/E-LIS=113.5. 원시 수치는 위 v5 데이터 파일 참고.)

**swap(양끝 교환)**: D-LIS/E-LIS가 K와 사실상 동률(Chromium 0.99x/1.04x, Firefox
1.00x/1.06x)로 격차를 닫았다 — 2차 실측에서 D/E가 K 대비 1.75-1.81배 느렸던
바로 그 워크로드다. 이는 정확히 2차 실측이 손으로 시뮬레이션해 예측한 그대로:
LIS가 "교환된 두 원소만 진짜로 옮겨야 한다"는 걸 증명하고, 나머지는 이동 판정에서
제외하기 때문 — `swap_ends_costs_only_two_repositions*` 테스트가 이 사실을
알고리즘 수준에서 이미 확인했고, 이번 실측이 그게 실제 성능으로도 나타남을
확인한 것.

**shuffle10(격차 좁아지되 안 닫힘) — 왜인지 코드를 다시 읽어 확인함**: LIS
변형이 순매칭 대비 확실히 개선되긴 한다(Chromium 2.12x→1.56x/1.69x, Firefox
5.00x/5.17x→4.52x/4.37x)만, K에는 여전히 뚜렷이 못 미친다. 원인을 추측에 맡기지
않고 `Workload::Shuffle10`의 실제 정의(`crates/loom-bench/src/lib.rs`)를
확인함: 이름과 달리 "항목 10개 셔플"이 아니라 **`v.len()/10`회의 무작위
제거+삽입**이다 — n=10,000이면 실제로 **1,000회**의 개별 재배치가 일어난다(이는
RQ1 후보안 절 219행의 원래 설계 의도 "shuffle 10%"와 일치하는 이름이며, 오해의
소지가 있어 이번에 확인한 김에 명시해 둠). 즉 진짜로 이동해야 하는 최소 개수
자체가 swap(2개 고정)과 달리 n에 비례해 크다 — LIS는 불필요한 이동을 없애줄 뿐
필요한 이동 자체를 줄여주진 않으므로, 이 워크로드에서는 K와의 격차가 구조적으로
남는다. 거기에 더해 LIS 계산 자체가 실제 이동 건수와 무관하게 매 reconcile마다
치르는 **고정 O(n log n) 비용**이라는 게 append/head_insert/remove_middle에서
보인다 — 이동이 0~1건뿐인 이 워크로드들에서도 D-LIS/E-LIS가 D/E보다 소폭
느리다(Chromium +3~8%, Firefox +10~20%). 두 요인(구조적으로 큰 실제 이동 수 +
LIS 계산 자체의 고정 비용)이 합쳐진 결과로 해석함.

**이 실측이 답한 것과 안 답한 것**: RQ1 후보안 절이 "O(이동 수) 주장을 하려면
LIS가 필요하다"는 걸 예측했고 2차 실측이 그 예측(순매칭만으로는 실패)을
확인했다면, 이번 3차 실측은 "LIS를 실제로 넣으면 얼마나 회복되는가"에 답한다 —
답은 **워크로드에 따라 다르다**: 국소 재배치(swap류)는 완전히 회복, 광역
재배치(shuffle10류)는 부분 회복. 여전히 안 답한 것: (1) 2차 실측과 동일한
미실행 축 전부(row 깊이, 원본 앱 규모, paired bootstrap, §2.4 체크리스트,
단일 머신) 이번 라운드도 그대로 남음, (2) LIS 계산 자체의 O(n log n) 고정
비용이 얼마나 더 큰 n(예: 100,000)에서 문제가 되는지 미측정, (3) shuffle10의
"재배치 비율"을 변수로 만들어(예: 1%/10%/50%) LIS 이득이 재배치 비율의 함수로
어떻게 변하는지는 미측정. **RQ1 잠정 선택은 여전히 미착수** — 다만 이번 결과로
트레이드오프가 한 축 더 명확해졌다: C는 opt-in 스코프 안에서 항상 최저비용,
D-LIS/E-LIS는 순매칭보다 항상 낫고 국소 재배치에서는 K와 동급이지만 광역
재배치에서는 여전히 K에 못 미치며, 어느 쪽이든 LIS 계산 비용을 국소 변경
워크로드에서 일부 지불한다.

### Phase 1 트랙 T 진행 상황 (2026-07-23) — B/F 구현·검증 완료, 실측은 결정적이지 않음

**구현**: `crates/loom-runtime/src/cond.rs` — 후보 **B**(`cond_reconcile_b_system`,
despawn/respawn)와 **F**(`cond_reconcile_f_system`, CSS 은닉/`display` 토글 변형 —
RQ1-benchmark-protocol.md가 요구하는 "CSS 은닉(v-show형)" 대조군을 겸함). 조건부
블록은 `Signal<bool>` + `CondBlock{child: Option<Entity>}`로 표현하고, 내부에
`Signal<i32>`를 가진 자식 entity 하나(상태 보존 여부를 실제로 관찰 가능하게 하는
최소 장치)를 토글 대상으로 삼음. 새 `DomOps::set_visible`/`DomWrite::SetVisible`을
추가함(list.rs의 `RemoveNode`와 동일한 이유로 despawn 전에 핸들을 미리 읽어둠).
**Phase 1 스코프**: F는 3변형 중 CSS-hide 하나만, DocumentFragment 분리 변형과
"sleeping System 게이팅" 3변형은 미구현. loom-compiler는 조건부 JSX 저작 표면을
아직 지원하지 않음 — 이번 라운드도 트랙 L처럼 런타임/네이티브 테스트/브라우저
벤치 레벨에서만 구현·검증했고 TSX 컴파일 경로에는 아직 연결하지 않음.

**네이티브 검증**: `crates/loom-runtime/tests/cond_candidates.rs` — 10개 테스트.
핵심 주장을 직접 관찰: **B는 토글 off→on 시 내부 상태(counter=3)를 0으로
리셋**하고 새 entity를 발급하는 반면(`toggle_off_then_on_loses_internal_state`),
**F는 같은 entity를 재사용하며 counter=3을 그대로 보존**한다
(`toggle_off_then_on_preserves_internal_state`) — RQ1이 F의 존재 이유로 지목한
바로 그 속성을 정합성 테스트로 고정함. 반복 5회 토글에서도 B는 매번 완전히
새 노드/entity를, F는 항상 같은 노드/entity 하나만 쓴다는 것도 확인.

**브라우저 실측(2026-07-23, Chromium+Firefox)**: `crates/loom-bench`의
`run_track_t` — B/F 각각 toggle_on/toggle_off 100회, 자식 1개(내부 `Signal<i32>`
하나)짜리 최소 서브트리. **결과가 결정적이지 않음** — 양쪽 브라우저 모두
median 0.0000ms, 최대값도 0.0~1.0ms 수준으로 `performance.now()`의 실측 해상도
아래에 있었다(RQ1-benchmark-protocol.md가 이미 M-7로 등록해 둔 타이머 해상도
문제가 정확히 이 지점에서 재현됨). **원인은 서브트리 크기**: 자식 1개짜리
토글은 B/F 어느 쪽이든 연산 자체가 너무 가벼워 차이가 노이즈에 묻힌다 —
RQ1-benchmark-protocol.md가 요구하는 "서브트리 크기" 축(자식 다수·중첩 구조)을
아직 구현하지 않았기 때문이며, 그 축을 넣기 전까지는 B/F의 **성능** 차이를
실측으로 판별할 수 없다는 뜻으로 정직하게 읽어야 한다. 원시 데이터:
`research-questions/data/rq1-phase1-track-t-toggle-{chromium,firefox}.json`.

**결론**: 트랙 T는 B/F 둘 다 실제로 동작하고, 이 둘을 가르는 핵심 주장(상태
보존 여부)은 정합성 테스트로 확실히 검증됐다. 그러나 **성능** 축은 이번 실측
스코프(자식 1개)에서 판별 불가 — 서브트리 크기를 늘리는 것이 다음 실측
우선순위. RQ1의 B/F 잠정 선택은 여전히 미착수.

### Phase 1 트랙 T 재실측 — 서브트리 크기 축 확장, 마침내 결정적인 결과 (2026-07-24)

**구현**: `CondBlock`을 `child: Option<Entity>` 단일 자식에서 `{size: usize,
children: Vec<Entity>}`로 일반화(`crates/loom-runtime/src/cond.rs`) — `size`개의
형제 자식이 **블록 자신의 `DomTarget` anchor를 공유 wrapper로 삼아** 그 아래
스폰된다. 이 일반화 자체가 핵심 설계 결정 하나를 강제했다: **F는 이제 개별
자식이 아니라 공유 anchor 하나에 대해서만 `set_visible`을 호출**한다 —
부모를 숨기면 그 안의 자식 전부가 공짜로 같이 숨겨지는 실제 CSS 동작을 그대로
활용한 것이며, 이게 바로 "F의 토글 비용이 서브트리 크기와 무관하게 O(1)"이라는
주장을 실제로 성립시키는 메커니즘이다(자식마다 개별 `set_visible`을 불렀다면
F도 B처럼 O(size)가 되어 이 축을 확장하는 의미 자체가 없어짐). 반면 **B는
여전히 자식 하나하나를 개별적으로 destroy+RemoveNode**한다 — "공유 wrapper
요령이 없다"는 것 자체가 B의 정의적 특징이므로 그대로 유지. 이 변경으로 F의
가시성 대상이 "자식 자신의 노드"에서 "공유 anchor 노드"로 바뀌어 기존
`cond_candidates.rs` 테스트 중 시각성 관련 3개를 anchor 기준으로 갱신했고,
`subtree_size` 모듈에 신규 테스트 3개를 추가(`b_mounts_exactly_size_children_
and_despawns_all_of_them_on_hide` — B의 제거 비용이 size에 비례함을 직접
카운트로 단언; `f_mounts_exactly_size_children_and_toggles_them_with_one_
set_visible_call` — F의 토글이 자식 수와 무관하게 정확히 `set_visible` 호출
1건임을 단언; `f_toggle_cost_does_not_grow_between_size_1_and_size_50` — size=1과
size=50에서 F의 호출 수가 동일함을 직접 비교). `cargo test --workspace`
회귀 없이 통과(`cond_candidates.rs` 13개, 전체 신규 3개 포함).

**측정 1차(size=1/10/50) — 여전히 타이머 해상도 아래**: `crates/loom-bench`의
`run_track_t`에 `SUBTREE_SIZES` 축을 추가해 B/F × toggle_on/toggle_off ×
{1,10,50} 조합을 100회씩 실측. Chromium에서 size=50 toggle_on의 0-초과 표본
비율이 B는 4%(size=1) → 6%(size=10) → **47%**(size=50)로 뚜렷이 증가하는 반면
F는 0-3%에서 정체 — 방향은 예측대로였지만 **median은 세 크기 전부 0.0000ms**로
여전히 확정적 수치를 못 냄. 여기서 멈추지 않고 크기를 한 단계 더 키움.

**측정 2차(size=500 추가) — 마침내 노이즈 바닥을 벗어남**: `SUBTREE_SIZES`에
500을 추가해 재실측(Chromium+Firefox). 결과, size=500에서 **B는 0-초과 표본
비율이 사실상 100%로, F는 여전히 0-3%에 머물렀다**:

| 브라우저 | size | 후보 | workload | median(ms) | 0-초과 비율 | max(ms) |
|---|---|---|---|---|---|---|
| Chromium | 500 | B | toggle_on | **0.5000** | 100/100 | 0.7 |
| Chromium | 500 | B | toggle_off | **0.2000** | 100/100 | 1.5 |
| Chromium | 500 | F | toggle_on | 0.0000 | 1/100 | 0.6 |
| Chromium | 500 | F | toggle_off | 0.0000 | 0/100 | 0.0 |
| Firefox | 500 | B | toggle_on | **1.0000** | 97/100 | 4.0 |
| Firefox | 500 | B | toggle_off | 0.0000 | 38/100 | 2.0 |
| Firefox | 500 | F | toggle_on | 0.0000 | 1/100 | 2.0 |
| Firefox | 500 | F | toggle_off | 0.0000 | 1/100 | 1.0 |

원시 데이터: `research-questions/data/rq1-phase1-track-t-toggle-v2-{chromium,
firefox}.json`(1/10/50/500 전체 포함).

**결론 — RQ1 트랙 T의 핵심 주장이 실측으로 확정됨**: 서브트리 크기를 충분히
키우자(500) B의 토글 비용은 두 브라우저 모두에서 노이즈 바닥 위로 뚜렷이
올라왔고(Chromium median 0.5ms, Firefox median 1.0ms, 거의 매 회 0 초과), F는
같은 크기에서도 여전히 사실상 0(양쪽 다 0-3% 표본만 0 초과)이었다 — **F의
토글 비용이 서브트리 크기와 무관하게 O(1)로 유지된다는 RQ1의 핵심 주장이
실측으로 확정**됐고, `f_toggle_cost_does_not_grow_between_size_1_and_size_50`
테스트가 이미 알고리즘 수준에서 증명한 바로 그 성질이 실제 브라우저 성능으로도
나타남을 확인한 것. 다만 **B의 toggle_off가 toggle_on보다 뚜렷이 싼 것**(예:
Firefox size=500: on median 1.0ms vs off median 0.0ms이지만 0-초과 비율은 38%)도
관찰됨 — B의 "show" 경로(500개 `create_element`+`SpawnRow`)가 "hide" 경로(500개
`RemoveNode`)보다 원천적으로 더 비싼 DOM 연산이기 때문으로 해석되며, 이는 트랙
L의 "생성은 피할 수 없는 O(n) 비용, 제거/은닉은 상대적으로 쌈" 패턴과도
일치한다(별도로 정밀 분해 측정은 하지 않음 — 다음 우선순위로 남김).

**이 실측이 답한 것과 안 답한 것**: 답한 것 — B/F 성능 차이가 (a) 실재하고
(b) 서브트리 크기의 함수로 나타나며 (c) F가 주장하는 정확한 형태(O(1))로
나타남. 안 답한 것: (1) size=500과 실제 애플리케이션의 "무거운 위젯"(원본 앱
규모 축) 사이의 간극 — 프로토콜이 요구하는 원본 앱 규모 축은 여전히 미실행,
(2) B의 on/off 비대칭에 대한 근본 원인 분해(생성 vs 제거 비용의 정밀 분리),
(3) paired bootstrap 등 §2.4/§8 통계 절차 여전히 미적용(percentile/비율까지만),
(4) F의 3변형 중 CSS-hide 하나만 측정 — DocumentFragment 분리 변형은 여전히
미구현. **RQ1의 B/F 잠정 선택은 여전히 미착수**이지만, "성능 차이가 실재하는가"
라는 이번 라운드 이전까지 완전히 열려 있던 질문 하나는 닫혔다.

### RQ1-benchmark-protocol.md §8 통계 절차를 기존 실측 데이터에 적용 (2026-07-24)

**한 것**: `RQ1-benchmark-protocol.md` §8이 사전 등록한 정확한 절차 — 후보/기준
median 비율의 **시드-쌍대(paired) 부트스트랩**(10^4 리샘플), **99% 판정용 CI +
95% 지도용 CI 병기**, **우세/열세/등가([1/1.1, 1.1])/"3배 이상"(CI 하한≥3)/비결정**
어휘 — 를 이미 수집된 트랙 L n=10,000 데이터(v5, K/C/D/E/D-LIS/E-LIS)에 그대로
적용함. 새 실측이 아니라 **기존 원시 데이터의 재분석**이며, 스크립트는
`research-questions/analysis/rq1-paired-bootstrap.py`에 커밋해 §8 "분석 절차
동결" 요구에 맞춤. 쌍대 부트스트랩이 통계적으로 유효한 이유: `run_combination`이
반복 `rep`마다 `Rng(SEED ^ (rep+1))`으로 시드하고 이 seed는 **그 rep에서 측정되는
모든 후보에 대해 동일**하므로(`crates/loom-bench/src/lib.rs`), 후보별
`samples_ms[rep]`는 이미 같은 목표 시퀀스를 겪은 쌍이다 — 리샘플 시 후보 양쪽에
동일한 rep-인덱스 집합을 사용하는 것이 정당함.

**중요한 정정 — 점추정만으로는 "동률"이라 읽었던 것이 통계적으로는 확정 아님**:
3차 실측(LIS 변형) 절이 "D-LIS/E-LIS가 swap에서 K와 사실상 동률(0.99x-1.04x)"이라고
쓴 것은 **median 점추정**만 본 것이었다. 이번 부트스트랩으로 재검증한 결과:

| 워크로드 | 후보 | Chromium 99% CI | 판정 | Firefox 99% CI | 판정 |
|---|---|---|---|---|---|
| swap | D-LIS vs K | [0.806, 1.050] | **비결정** | [0.885, 1.087] | **비결정** |
| swap | E-LIS vs K | [0.812, 1.187] | **비결정** | [1.042, 1.304] | **열세** |
| shuffle10 | D-LIS vs K | [1.245, 1.702] | 열세 | [4.148, 5.167] | 열세 [3배 이상] |
| shuffle10 | E-LIS vs K | [1.354, 1.965] | 열세 | [4.111, 5.000] | 열세 [3배 이상] |

즉 swap에서 D-LIS/E-LIS가 K와 "동등"하다는 건 아직 **증명되지 않았다** — CI가
1을 걸쳐 있어 우세도 열세도 등가도 아닌 **비결정**이 정직한 판정이며(reps=6이라
CI가 넓음 — 아래 갭 참고), Firefox의 E-LIS는 오히려 **열세**로 판정된다. 반대로
shuffle10은 Firefox에서 D-LIS/E-LIS 둘 다 **"3배 이상"** 깃발까지 뜬다(K 대비
4.1-5.2배) — LIS 적용 후에도 여전히 이 정도로 나쁘다는 것이 3차 실측 절의
"좁아지되 닫히지 않음" 서술보다 한층 더 강한 확정이다. **이 정정 자체가
CLAUDE.md의 정직성 요구 사례** — 이전 절의 표현을 조용히 유지하지 않고 여기서
명시적으로 수정함.

**C의 채택 조건 (a) 관련 확인**: `{W1,W3,W4}×R1`(swap/shuffle10/head_insert,
R1 등가)에서 C vs D 비율은 두 브라우저 모두 CI 전체가 1보다 훨씬 작아 **전 셀
우세** — C가 D 대비 동등 이상이라는 조건 (a)의 이 서브셋에 대해서는 강하게
충족됨(단, 프로토콜이 요구하는 정확한 W1-W4×R1 6조합 전체·W2는 미실행이므로
공식 §9 판정은 아직 아님 — 아래 갭 참고). **D-LIS/E-LIS vs D 자체도 흥미로운
부작용**: append/head_insert(국소 변경, LIS가 아무것도 절약 못 하는 구간)에서
D-LIS/E-LIS가 D 대비 **열세**로 나오는 셀이 있다(예: Firefox append, D-LIS
[1.045,1.333] 열세) — LIS 계산 자체의 고정 비용이 통계적으로도 확인된 것.

**남은 갭(숨기지 않음)**:
1. **반복 수 부족**: §8 사전 등록은 셀당 워밍업 50 + 측정 200회를 요구하나 현재
   n=10,000 셀은 **6회**뿐(N-스케일링 때문에 reps를 줄인 설계 — 위 "2차 실측"
   절 참고) — 6개 표본에 대한 부트스트랩은 리샘플 다양성 자체가 낮아 CI가
   넓다. n=10,000에서 최소 30회 이상으로 늘려 재측정하는 것이 다음 우선순위.
2. **PRNG 불일치**: §8은 PCG64 + SHA-256(프로토콜판‖셀ID‖반복번호) 시드를
   요구하나, `crates/loom-bench`는 xorshift64 변형(`Rng`)을 씀 — 쌍대성(같은
   rep→같은 시퀀스)은 유효하지만 프로토콜이 못박은 PRNG 자체는 아님.
3. **W2(전체 shuffle)·W7(값만 갱신)** 워크로드 미구현 — 현재 벤치의 `shuffle10`은
   프로토콜의 W3(10%)에 해당, W2(Fisher-Yates 전체 순열)와 W7(retain 경로 표적)은
   후속 작업.
4. **row 깊이 축(R1/R2/R3) 전면 미실행** — 설계는 검토했으나 구현하지 않음,
   이유와 청사진을 아래에 명시.
5. **원본 앱 규모 축, WebKit, CDP 스로틀링, 트랙 N(중첩 property-based)/R
   (resumability), §2.4 형식적 최선형 체크리스트 전수 대조, §5 FV 게이트** 모두
   미실행.

**row 깊이 축(R1/R2) 청사진 — 설계는 끝냈으나 이번 라운드에 구현하지 않음**:
검토 결과 이 축은 6개 후보의 스폰 경로 4곳(K, C, D/D-LIS의 `place_in_order[_lis]`,
E/E-LIS)을 전부 건드려야 하는 실측 회귀 위험이 있는 변경이라, LIS/Track T처럼
한 세션에서 안전하게 끝내기엔 이번 차례의 남은 여력을 넘어선다고 판단해 구현을
미루고 설계만 고정해 다음 착수자가 바로 시작할 수 있게 남긴다. 계획: (a)
`RowBindings(usize)` Bevy Resource(기본값 1=R1) 추가, (b) 각 스폰 경로가 자기
row의 `SpawnRow` write를 낸 직후 `RowBindings.0 - 1`개의 추가 정적 텍스트 자식을
내는 새 `DomWrite::AppendStaticChildren{parent_entity: Entity, tag, texts}`
variant 도입(`before`처럼 parent를 Entity로 받아 tick() 적용 시점에 NodeHandle로
해석 — 같은 배치에서 막 스폰된 row 아래에 자식을 붙여야 하므로), (c) 이 패딩은
**반드시 같은 tick 안에서 동기적으로** 내야 함 — `Added<DomTarget>` 기반 후속
System으로 하면 다음 tick으로 밀려 측정 대상 연산의 타이밍 밖으로 빠짐. (d) 이
설계는 8개의 독립적으로 반응하는 바인딩이 아니라 **정적 텍스트 자식 8개**로
"row 깊이"를 근사하는 것이므로 프로토콜의 R2 원문("binding 8개 + 중첩 조건부
1개")보다 좁은 프록시다 — 실행 시 이 축소를 그대로 문서화할 것. (e) 기존
`list_candidates.rs` 35개 테스트는 `RowBindings` 기본값 1(R1)에서 전혀 변경 없이
그대로 통과해야 하며, 축은 `crates/loom-bench`에서만 opt-in으로 켠다.
