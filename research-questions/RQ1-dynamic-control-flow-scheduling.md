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

### row 깊이 축 구현 + RQ1 §9 실제 적용 (2026-07-24, 별도 라운드)

**구현**: 청사진 그대로 — `RowBindings(usize)` Resource(기본값 1) +
`DomWrite::AppendStaticChildren{row: Entity, texts: Vec<String>}` +
`LoomApp::set_row_bindings()`, K(`list_reconcile_k_system`)·C(`list_reconcile_
c_system`)·D/E가 공유하는 `place_in_order`·D-LIS/E-LIS가 공유하는
`place_in_order_lis`에 각각 반영 — 4개 스폰 경로가 아니라 **2개의 공유 함수**
(`place_in_order`/`place_in_order_lis`가 D·E, D-LIS·E-LIS 각각 2개씩을
커버)였음이 실제 코드를 다시 읽어보니 확인됨(당초 "4개 스폰 경로 수정"이라던
청사진 서술보다 실제로는 더 적은 지점 수정으로 충분했음). 새 네이티브 테스트
4개(`list_candidates.rs`의 `row_bindings_axis` 모듈) — 기본값 무변경 확인,
K가 매 tick마다 전체 재생성으로 추가 자식을 다시 만든다는 것, D가 순수 재정렬
에서는 기존 row의 추가 자식을 전혀 건드리지 않는다는 것을 직접 단언. 기존
39개(35+4) 테스트 전부 통과, `cargo test --workspace` 회귀 없음.

**Chromium+Firefox 실측(n=1,000, String, R1 vs R2=8)**: 원시 데이터
`research-questions/data/rq1-phase1-track-l-core-grid-v6-{chromium,firefox}.json`.
핵심 발견 — **R1→R2로 갈 때 K의 비용은 워크로드 무관하게 대략 4~4.5배
증가**(Chromium swap: 1.1→4.9ms; Firefox: 3.0→12.0ms)하는 반면 **D/E/D-LIS/
E-LIS의 로컬 변경(head_insert/append/remove_middle) 비용은 R1과 R2에서
완전히 동일**(예: Chromium D append 0.2ms → 0.2ms, 무변화) — retained row의
내용을 재정렬 시 전혀 건드리지 않는다는 D/E의 설계 자체가 실측으로 재확인됨.
K는 매 tick 전체를 지우고 다시 만들므로 깊이에 선형 비례해 비싸지고, D/E는
깊이와 무관 — 이게 바로 RQ1 §9의 D-vs-K 판정 게이트가 필요로 하는 정확한
비대칭.

**paired bootstrap(10^4 리샘플, 99% CI)으로 재확인** —
`research-questions/analysis/rq1-paired-bootstrap.py`의 방법론을 재사용:

| 브라우저 | 셀 | D vs K | E vs K |
|---|---|---|---|
| Chromium | R2×swap | CI=[0.220,0.250] **우세** | CI=[0.216,0.250] **우세** |
| Chromium | R2×shuffle10 | CI=[0.204,0.250] **우세** | CI=[0.220,0.273] **우세** |
| Firefox | R2×swap | CI=[0.231,0.333] **우세** | CI=[0.231,0.364] **우세** |
| Firefox | R2×shuffle10 | CI=[0.250,0.364] **우세** | CI=[0.308,0.364] **우세** |

R2에서 D·E 둘 다 K를 4~5배 차이로 확실하게 이긴다 — median만 봐도 명확했지만
paired bootstrap으로 재확인해 우연이 아님을 확정.

### RQ1-benchmark-protocol.md §9를 실제로, 기계적으로 적용

**D — 조건부 채택(FV 계류)**:
- **기각 (iii) 미발동**: "{W1,W2,W3,W4}×{R1,R2} 전 셀에서 K가 D 대비 동등
  이상"을 요구하는데, 방금 확인한 R2×swap/shuffle10에서 **D가 K를 압도적으로
  이김**(K 동등 이상이 아님) — "전 셀" 조건이 이 지점에서 이미 깨짐. 기각 (iii)
  발동 안 함.
- **채택 양성 조건 충족**: "{W1,W2,W3}×R2 중 최소 1셀에서 K 대비 우세"를
  요구하는데, 위 표대로 **두 브라우저 모두 2개 이상의 셀**에서 우세 확인 —
  넉넉히 충족.
- **W5(append) 열세 확인 안 됨** → unkeyed opt-out 부속 선결 조건 미발동(n=1000
  R1에서 D append=0.2ms, K=1.0ms — D가 오히려 우세).
- **단, 채택의 추가 선결 조건("§5 FV 계류")이 아직 미충족** — flush 규약 v0의
  formal verification은 이 저장소 전체에서 아직 스켈레톤 수준(Phase 5 몫,
  이번 계획의 4단계).

> **2026-08-08 갱신 — 이 문단의 "스켈레톤 수준" 진단은 더는 성립하지 않는다.**
> flush 규약 v0의 FV는 2026-07-31~08-08에 실제로 수행됐다:
> `verus formal-verification/verus/flush_order.rs` → 28 verified, 0 errors,
> `verus --crate-type=lib crates/loom-flush-core/src/lib.rs` → 47 verified,
> 0 errors. 다만 **§5 FV 계류가 완전히 해소된 것은 아니다** — 비-LIS 경로
> (`plan_writes_converging`)는 수렴이 실행 코드의 사후조건으로 증명됐지만,
> D·E가 실제로 쓰는 **LIS 변형(`plan_writes`)의 수렴은 미증명**이다. 그 사전조건
> P만 2026-08-08에 `requires`로 올라갔다. 따라서 **최대 "조건부 채택(FV 계류)"** — 완전한
  채택은 아직 아님, §9 자체가 명시한 정확히 그 중간 상태.

> **2026-08-09 갱신 — LIS 변형의 수렴도 증명됐다.**
> `verus --crate-type=lib crates/loom-flush-core/src/lib.rs` → **70 verified,
> 0 errors**. `plan_writes`가 이제 `plan_writes_converging`과 **같은 모양의
> 수렴 사후조건**을 갖는다(상세: `formal-verification/verus/README.md`의
> "LIS 경로의 수렴 — 증명 완료" 절). 즉 위 문단이 §5 FV 계류의 잔여분으로
>지목한 항목은 해소됐다.
>
> **그러나 계류가 완전히 걷힌 것은 아니고, 두 가지를 정확히 알아야 한다.**
>
> 1. **사전조건이 하나 늘었다(Q)**. 전수 탐색 결과 목표 수렴 명제는 P만으로는
>    **거짓**이었다 — `current`에 target 밖 원소(extras)가 있으면 `keep` 단락
>    때문에 영영 치워지지 않는다(최소 반례 `current=[0,1] target=[0]
>    keep=[true]`). 그래서 "`current ⊆ target`"을 `requires`에 **추가**했다.
>    D·E의 실제 호출부(`d_lis_body`)는 `current = old_order − removed`를
>    만들므로 구조적으로 항상 만족하고, 디버그 빌드 검사도 넣었다. 다만 이는
>    **비-LIS 경로에는 없는 조건**이며 `keep` 단락이 사 오는 비용이다.
> 2. **`lis_indices`가 P를 세운다는 것(LIS 건전성)은 여전히 미증명**이다.
>    증명된 것은 "P ∧ Q ⟹ 수렴"이지 "LIS가 P를 만든다"가 아니다. 그 경계는
>    `place_in_order_lis`의 두 `debug_assert!`와 무작위 속성 검사가 지킨다.
>
> 따라서 채택 상태는 "조건부 채택(FV 계류)"에서 **"조건부 채택(LIS 건전성
> 계류)"**로 좁혀진 것이다 — 계류 항목이 배치 알고리즘 전체에서 `lis_indices`
> 하나로 줄었다. 완전한 채택으로 올리려면 그 하나가 남았다.

**2026-08-11 정정 — 그 하나가 닫혔다.** 위 인용의 2번(`lis_indices`가 P를
세운다는 LIS 건전성)은 **증명됐다**. `lis_indices`와 `keep` 조립이
`loom_flush_core::lis_keep`으로 옮겨 갔고 P(`take_order_preserved`)가 그 함수의
**사후조건**이다. 그리고 그 사후조건이 `plan_lis_from_selection`의 사전조건으로
이어지는 것까지 `plan_lis_from_selection_auto` 안에서 기계검사된다(사후조건
한 줄을 지우면 `precondition not satisfied`가 난다 — 값만 넘긴 것이 아니다).
`list.rs`의 P `debug_assert!`는 삭제됐다.

방출되는 write 열은 바뀌지 않았다(`write_identity` 통과 + 옛/새 구현 `keep`
벡터 전수 대조 30,153건 불일치 0건)이므로 **§2.5 재측정 사유가 아니다** —
트랙 L 수집분은 그대로 유효하다.

기준선: `crates/loom-flush-core` **90 verified, 0 errors, 경고 0**.
상세: `formal-verification/verus/README.md`의 "LIS 건전성 — 증명 완료, 사슬이
닫혔다" 절.

**주의 — 이것이 곧 "완전한 채택"은 아니다.** 위 문장이 말한 계류 항목은
해소됐지만, 최종 채택 판정은 §5 게이트와 측정 데이터에 걸린 별개 판단이다.
여기서는 **FV 계류 항목이 소진됐다는 사실만** 기록한다.

**E — D와 같은 방향이나 근거가 더 얕음**:
- **기각 (i) 미발동**: 판정 셀 그대로("W1·W3×R2, n=1,000") 확인 — 두 브라우저
  모두 두 셀 다 **우세**(위 표). "어느 셀에서도 우세 없음"이 기각 조건인데
  정확히 반대로 나왔으므로 미발동.
- **기각 (ii)/(iii) 미평가**: (ii)는 §7.1의 단계별 계측(reconcile System
  시간 vs DOM 배치 적용 시간 절감분)을 요구하는데 현재 하네스는 이 세부
  구간을 계측하지 않음(전체 tick 시간만 잼) — 미평가로 정직하게 남김. (iii)
  G0(No-VDOM 재심사)은 docs/03의 "판정례" 절차로 이미 설계 단계에서 다뤄졌으나
  이번 라운드에 재확인하지 않음.
- 결론: D와 같은 "조건부 채택 방향"이나, (ii)/(iii) 미평가라는 갭이 있어 D보다
  근거가 얕다 — 굳이 D와 동일한 확정성으로 서술하지 않음.

**C — 이전 예상과 다른, 더 신중한 결론(정정)**: 이번 세션 초반 계획 문서에서
"C 채택 조건 (a)(b) 충족"이라고 썼던 것은 **W2(전체 shuffle)가 아직 존재하지
않던 시점의 부분 데이터에 기반한 성급한 판단이었다 — 이번에 W2를 실제로
추가하고 정확한 4셀({W1,W2,W3,W4}×R1)을 갖춰 재확인한 결과 정정한다**:
- **조건 (a) 위반**: "{W1,W2,W3,W4}×R1 전 셀에서 D 대비 동등 이상"을
  요구하는데, **head_insert에서 C가 D에 열세**(Chromium: C vs D CI=
  [1.333,2.500], 열세로 확정) — head_insert는 C의 전 슬롯 값 재작성이
  걸리는 정확히 그 시나리오(RQ1 유불리 지도가 이미 예견한 "C 비용 (1)").
  나머지 3셀(swap/shuffle/shuffle10)은 전부 D 대비 우세지만, "전 셀"
  조건이 head_insert 하나에서 깨지므로 (a)는 **불충족**.
- **조건 (b)는 충족**: "{W1,W2,W3,W4}×R1 중 최소 1셀에서 K 대비 우세" — 4셀
  전부 K 대비 우세(head_insert 포함, C vs K CI=[0.364,0.500]) — 넉넉히 충족.
- **결론**: 세 조건 모두 충족을 요구하는 §9 문언상 C는 **아직 완전한 채택
  판정을 받지 못함** — 기각 조건(W2×R2 CI 하한≥3 또는 W2×R3 archetype move
  지배)도 이번 라운드에 R3 데이터가 없어 미검증. 그러므로 C의 §9 상태는
  "채택"도 "기각"도 아닌 **판정 비결정**(RQ1-benchmark-protocol.md §8의
  그 범주 그대로)이 정직한 서술 — 이전 세션의 "충족" 서술은 이 문서에서
  명시적으로 정정한다.

**B/F — 여전히 판정 불가, 원인 재확인**: §9 B/F 조항은 F가 "통상 구간
f∈[0.1,10]/s × s∈[1,100]" 안에서 B를 이기는 셀이 있는지를 요구하는데,
`crates/loom-bench`의 트랙 T 하네스에는 **초당 토글 빈도(f) 축 자체가
없음**(`toggle_on`/`toggle_off`를 100회 반복할 뿐, 시간당 빈도 개념이 없음)
— 이 축 없이는 §9의 B/F 조항을 문자 그대로 적용할 방법이 원리적으로 없다.
이번 라운드도 이 축을 새로 만들지 않았으므로(스코프 밖으로 명시적으로 남김),
**"판정 불가" 자체가 이번 라운드의 정직한 결론**.

> **2026-07-28 갱신 — 이 "원리적으로 불가" 상태는 해소됨(하네스 한정).**
> 아래 "트랙 T 토글 빈도(f) 축 신설" 절 참고. 위 문단의 진단(축이 없어서
> 판정 자체가 불가능)은 그 시점에는 정확했고, 그 축을 이제 실제로 만들었다.
> 다만 **판정이 곧바로 나온 것은 아니다** — 아직 캠페인 미실행이고, F arm
> 4개가 여전히 미구현이라 §9 F 기각의 1차 분기는 그대로 적용 불가다.

**D-LIS/E-LIS**: §9엔 이 둘을 위한 별도 조항이 없다(D/E의 변형 취급) — 이미
확인된 대로 D-LIS/E-LIS는 D/E보다 항상 같거나 더 빠르므로(R2에서도 D-LIS=
0.06-0.08x K, D=0.22-0.25x K — D-LIS가 더 우세), D/E의 판정에 흡수해 함께
보고한다.

**종합**: RQ1의 최종 후보 선택은 여전히 완전히 확정되지 않았으나(§5 FV 계류가
D·E 공통의 선결 조건으로 남아 있고, C는 판정 비결정), 이번 라운드로 실제로
바뀐 것은 — **D/E가 "조건부 채택(FV 계류)" 상태로 격상됐다**(이전엔 R2 데이터
자체가 없어 §9를 적용할 수조차 없었음). RQ1의 잠정 결론이 "K가 사실상 기본"
쪽으로 굳어질 것이라는 이번 세션 초반의 예상은 **정정한다** — row 깊이 축을
실제로 넣어보니 D/E가 넓은 워크로드·row 깊이 조합에서 K를 확실히 이기는
영역이 있다는 게 드러났고, 유일하게 남은 진짜 장벽은 성능이 아니라 §5 FV
계류(공유 인프라 검증)다.

---

### 트랙 T 토글 빈도(f) 축 신설 — "판정 원리적 불가" 해소 (2026-07-28)

**왜 이걸 먼저 했나**: RQ6 §7 판정이 끝나고 "실제로 쓸 수 있는 프레임워크"로
가는 선결 과제를 정리하면서 RQ1의 남은 갭을 전수 재확인한 결과, 트랙 T의
f축 부재가 **가장 작고 가장 명확한 다음 스텝**으로 판정됐다 — 다른 갭들(트랙
N/R 하네스 신규 작성, E/E-LIS §7.1 단계 귀속 계측)과 달리 이건 "무엇을 만들지"가
프로토콜에 이미 완전히 명세돼 있어 설계 논쟁이 없었다.

**만든 것**

| 산출물 | 무엇 | 어느 조항을 충족시키나 |
|---|---|---|
| `crates/loom-bench`의 `TrackTCell` | f × s 격자 셀 1개를 실시간(rAF) 창으로 구동, J-a(토글당)·J-b(창 전체 스크립트 CPU) 동시 수집 | §4.1 격자, §9 B/F 판정 지표 |
| `demo/track_t_freq_bench.html` | 셀 파라미터를 쿼리스트링으로 받는 1페이지=1시행 드라이버 페이지 | §8 "시행마다 fresh page load" |
| `bench/e2e/run-track-t-frequency.mjs` | 격자·시행 순회, fresh context/page load, CDP 4× CPU 스로틀, JSON 산출 | §8 표본 규정·격리·스로틀 |
| `bench/serve-isolated.py` | COOP/COEP 헤더를 붙이는 정적 서버 | §8 타이머 조항 |

**설계 결정 — 왜 rAF 실시간 루프인가**: §9의 셀 승패는 J-a와 J-b가 *둘 다*
우세해야 성립하는데, J-b의 정의("토글 처리 + **토글 간 상주** + 동결/은닉 중
패치·게이팅 비용")에서 "토글 간 상주" 항은 **토글 사이에 실제 시간이 흐르고 그
동안 프레임마다 tick이 돌아야만** 존재한다. 기존 하네스처럼 동기 루프로 토글을
연달아 때리면 이 항이 구조적으로 0이 되어 J-b가 J-a의 상수배로 퇴화한다 — 즉
f축은 "for 루프에 파라미터 하나 더"가 아니라 구동 모델 자체를 바꿔야 하는
작업이었다.

**측정 중 발견한 진짜 문제 — §8 타이머 조항이 여태 한 번도 지켜지지 않았음**:
하네스를 붙이고 첫 스모크런을 돌리자 모든 표본이 정확히 0.1ms의 배수로 떨어졌다
— `performance.now()`가 100µs로 클램프된 것. §8은 "COOP/COEP 크로스오리진
격리로 고해상도 타이머 확보, **셀별 유효 해상도를 실행 시점 기록**"을 사전
등록해 두었는데, 이 저장소의 벤치는 여태 `python3 -m http.server`로 서빙됐고
그건 COOP/COEP를 못 붙인다. 즉 **지금까지의 트랙 T 데이터(서브트리 크기 축
포함)는 전부 클램프된 시계로 수집된 것**이다. `bench/serve-isolated.py`로
바꾸자 실측 해상도가 **0.1ms → 0.005ms(20×)**로 떨어졌고(`crossOriginIsolated
=== true` 확인), 이제 셀마다 `timer_resolution_ms`/`cross_origin_isolated`가
결과 JSON에 함께 실린다. 이건 RQ1이 이전 라운드에 기록한 "묻혀 판별 불가"
현상의 원인 일부였을 가능성이 크다 — 다만 그 데이터를 소급 무효화하지는
않는다(size=500에서 B가 노이즈 바닥을 확실히 벗어났다는 결론 자체는 클램프된
시계로도 성립).

**스모크 관측(판정 아님 — 창 3~4초, 시행 1~2회, 사전 등록값 60초×10시행 미달)**:
Chromium 비스로틀에서 B의 spawn 경로가 s=100·f=10 셀에서 median ≈0.62–0.70ms,
4× 스로틀에서 ≈0.87ms. 둘 다 §9 "또는" 분기의 임계값 **1ms/100 entity 아래**다.
전 그리드로 확장했을 때도 유지되면 그 분기가 발동해 "결론 = B 유지"가 되지만,
**지금 데이터로 그렇게 말하지 않는다** — 창·시행 수가 사전 등록값에 미달하고,
s=1/s=1000 셀이 없다.

**여전히 남은 갭(숨기지 않음)**

1. **arm 4개 부재**: §4.1은 6 arm(B, F-i `CondEffective` 값 필터, F-ii detached
   DOM 계속 패치, F-iii `Without<Frozen>` marker, lazy-spawn-then-hide, CSS
   은닉)을 요구하나 `cond.rs`에는 2개뿐이고, 그중 F는 자기 doc comment가 명시하듯
   **CSS 은닉(v-show형) 대조군을 겸하는 변형**이다. 따라서 §9 F 기각의 1차
   분기("F-i/ii/iii 각각이 통상 구간에서 B를 이기는 셀이 하나도 없으면 기각")는
   **여전히 적용 불가** — 오늘 적용 가능해진 것은 "또는" 분기뿐이다. 이 구별을
   뭉개고 "오늘의 F가 B를 이겼으니 F 채택"이라고 쓰면 카테고리 오류가 된다.
2. **부속 축 부재**: 서브트리 구성(flat vs 파생 체인 깊이 3), catch-up 곡선
   (동결 기간 × 상류 변경률), attached 구간 변경률 — 전부 미구현. 특히 attached
   구간 변경률 축은 F-i의 상주 게이팅 비용이 드러나는 **유일한** 지점이라,
   이게 없으면 J-b의 B-vs-F 차이가 사실상 J-a의 함수로 퇴화한다(스모크에서
   실제로 B/F의 상주 tick 비용이 구별되지 않았다 — 둘 다 `Changed<Signal<bool>>`
   쿼리라 토글 없는 프레임에서 하는 일이 없기 때문).
3. **캠페인 미실행**: 사전 등록값(셀당 60초 × 독립 10시행 × fresh page load)으로
   격자를 돌리면 통상 구간(9셀) × 2 arm 기준 순수 측정만 약 3시간, 전체 격자
   (20셀)면 약 6.7시간이다. §8의 격리 조항상 그 동안 같은 머신에서 다른 빌드/
   테스트를 돌리면 데이터가 오염되므로 병행 불가 — 별도 실행 결정이 필요하다.
4. **`< 1ms/100 entity` 임계값의 해석 문제(신규 발견, 미해결로 등재)**: 이 임계값을
   s=1 셀에 적용하면 "entity 1개 spawn 비용 × 100"이 되어 고정 오버헤드가
   100배로 증폭된다 — s가 작을수록 임계값을 넘기기 쉬워지는 역설. RQ1 원문
   ("B의 spawn 비용이 전 영역에서 무시 가능(<1ms/100 entity)")도 §9도 이 정규화를
   어느 s에서 재는지 명시하지 않는다. **결과를 보고 나서 정하면 원칙 3 위반**
   이므로, 캠페인 실행 **전에** §11 개정 절차로 해석을 고정해야 한다. 이번
   라운드는 발견 사실만 등재하고 해석을 정하지 않는다.

### C 기각 조건 재검토 — "R3가 없어서 미검증"이 아니라 **앞 분기는 측정 자체가 무효** (2026-07-28)

**출발점**: 위 2026-07-24 라운드는 C의 §9 상태를 "판정 비결정"으로 정리하면서
기각 조건에 대해 이렇게 적었다 — "기각 조건(W2×R2 CI 하한≥3 또는 W2×R3
archetype move 지배)도 이번 라운드에 **R3 데이터가 없어** 미검증". 이번에
그 문장을 다시 읽고 확인해 보니 **두 분기가 서로 다른 이유로 막혀 있는데 하나의
이유로 뭉뚱그려져 있었다**.

§9 C 기각 조항은 '또는'으로 연결된 두 분기다:

- **앞 분기**: W2 × R2에서 D 대비 M1(a) 비율 CI 하한 ≥ 3
- **뒷 분기**: W2 × R3 프로파일에서 archetype move 구간(§7.1)이 스크립트 시간의 50% 이상

앞 분기는 **R3를 전혀 요구하지 않는다**. 그리고 `crates/loom-bench`의
`all_makers`는 6후보 전부를 매 조합마다 돌리므로 R2(row_bindings=8) × shuffle
셀에 C 표본이 **이미 들어 있다**(`rq1-phase1-track-l-core-grid-v6-*.json`).
즉 앞 분기는 새 측정 없이 계산 가능했고, 지금까지 아무도 계산하지 않았을 뿐이다
(`analysis/rq1-row-depth-bootstrap.py`는 C에 대해 **채택** 조건만 R1에서 계산하고
끝난다).

**계산해 보니 나온 것 — 그리고 그게 드러낸 더 큰 문제**: 해당 셀의 median은
Chromium에서 C=0.50ms vs D=1.50ms(비율 0.333), Firefox에서 C=1.00ms vs
D=10.00ms(비율 0.100)로, **C가 D보다 3~10배 빠르다**. 기각 임계값(비율 하한
≥ 3, 즉 C가 D보다 3배 이상 *느릴* 것)과 정반대 방향이다. 그런데 이건 RQ1이
예측한 방향("C 기각 조건 '깊은 row shuffle 3배'" — §3.4 유불리 지도가 W2×R2를
C의 기각 표적 셀로 지목)과도 정반대다. 예측이 틀린 게 아니라 **측정이 그
비용을 모델링하지 않는다**는 것이 원인이었다:

- `DomWrite::AppendStaticChildren`(= R2 축의 구현)은 자기 doc comment에 이미
  이렇게 적어 두었다 — "Models row-*creation* cost scaling with depth ... **not
  candidate C's separate 'O(n×binding) retained-slot value-patch' cost**, which
  would need each binding independently re-compared/rewritten on every tick,
  not just created once. Stated plainly: **this axis doesn't test that half of
  R2's prediction**, and RQ1 §9's C **adoption** conditions don't need R2 data
  at all (R1-only), so it isn't required to."
- `list_reconcile_c_system`의 실제 코드도 그대로다: 유지되는 슬롯에 대해서는
  **primary text 1개만** 비교·재기록하고, 추가 binding들(`extra_binding_texts`)은
  **spawn 경로에서만** 만든다. 따라서 길이 불변 shuffle에서 C의 R2 비용은
  사실상 R1 비용과 같다.

**결론(정정)**: 이 doc comment의 면책("C의 **채택** 조건은 R1만 쓰므로 R2를
정확히 모델링할 의무가 없다")은 채택 조항에 대해서는 맞지만, **기각 조항은
바로 그 W2×R2를 쓴다**. 그 연결이 지금까지 누구에게도 이어지지 않았다. 그러므로
앞 분기의 정확한 상태는 —

> "R3 데이터가 없어 미검증"이 **아니라**, R2 축이 binding을 값 파생이 아니라
> **정적 자식**으로 모델링하고 있어 기각 조항이 겨냥하는 비용 자체가 계측에
> 존재하지 않는다 → **현 하네스로는 앞 분기 측정 무효**(§11-1(b) "측정 방법
> 결함" — 계측이 측정하려는 것을 측정하지 않음).

**사전 등록 부트스트랩으로도 재확인(2026-07-28 실행)**: 방법론을 §8 그대로
(시드-쌍대 부트스트랩 10^4 리샘플, 판정용 99% CI) 적용한 결과 —
Chromium C/D 비율 CI=[0.312, 0.400], Firefox CI=[0.091, 0.200]. 기각 임계값
(CI 하한 ≥ 3)에서 **한 자릿수 배 이상 떨어져 있고 방향도 반대**다. 즉 이 셀은
"아슬아슬해서 판정이 갈리는" 경우가 아니라 **계측이 조항의 대상 비용을 아예
담고 있지 않은** 경우다(`analysis/rq1-c-rejection-gate.py`).

위의 0.333/0.100이라는 수치는 그래서 **판정에 쓰지 않는다**. 쓰면 "C에 유리한
쪽으로 계측 결함이 작동하는" 데이터로 기각을 면제해 주는 셈이 된다.

**기존 D/E 판정에는 영향 없음(확인)**: 같은 R2 데이터로 내린 D 기각 (iii)
미발동과 E 기각 (i) 미발동은 그대로 유효하다 — 그 판정들이 쓰는 비대칭은
"K는 매 tick 전체를 재생성하므로 추가 자식 8개를 매번 다시 만들고, D/E는
retained row를 건드리지 않는다"인데, 이건 정적 자식 모델로도 정확히 재현된다.
값 파생 재패치를 넣어도 K가 더 비싸지면 비싸졌지 D/E가 K를 이긴다는 결론은
바뀌지 않는다. 영향받는 것은 **C의 기각 앞 분기 하나뿐**이다.

**필요한 수리(다음 라운드 작업 항목)**: R2 축의 추가 binding을 "spawn 시 1회
생성되는 정적 텍스트"에서 "**슬롯 값에서 파생되어, 슬롯 값이 바뀌면 전부
재기록되는** 값 파생 binding"으로 바꾼다. 이게 정확히 RQ1이 예측한 비대칭을
살린다 — keyed 후보(D/E)는 retained row의 아이템이 바뀌지 않으므로 파생
binding도 재기록할 필요가 없고, C는 슬롯의 아이템 자체가 바뀌므로 8개 전부
재기록해야 한다. §11-1(b)에 따라 이 수리는 결과 확인 여부와 무관하게 가능하되,
수리 후 해당 셀을 **재실행**해야 한다(임계값을 결과에 맞추는 수리가 아니라
계측을 조항에 맞추는 수리이므로 허용).

**뒷 분기는 별개로 여전히 막혀 있음**: W2 × R3는 (a) R3 row 유형(값에 따라
`<A/>`/`<B/>` 구조 분기, 전환형은 §10이 archetype move형으로 사전 고정)과
(b) §7.1의 "Commands apply(sync point) 구간" 계측을 **둘 다** 요구한다. (b)는
E/E-LIS의 기각 (ii)("매칭 비용")가 요구하는 계측과 같은 인프라다 — 즉 앞선
우선순위 정리에서 "C의 R3 셀"과 "E/E-LIS §7.1 계측"을 서로 독립이라고 적었던
것은 **부정확했고**, 둘은 §7.1 단계 귀속 계측을 공통 선결로 공유한다(정정).

### 계측 커버리지 감사 — §7 측정 항목 대 하네스 실태 (2026-07-28)

위 C 건("조항이 요구하는 비용이 계측에 아예 없었는데 아무도 몰랐다")이 단발
사고인지 확인하려고 §7의 측정 항목 정의를 하네스 코드와 전수 대조했다. **두
건이 더 나왔다** — 둘 다 지금까지 어느 문서에도 미기록.

**(1) M1(b) 프레임 완결 시간 미계측**. §7 M1은 "(a) 스크립트 시간 … (b) 프레임
완결 시간 = double-rAF 콜백 − t0. **둘 다 보고**"라고 무조건 요구한다.
`crates/loom-bench`는 `measure_now()`로 tick 전후만 감싸므로 M1(a)만 잰다.
§9의 시간 판정은 M1(a)만 쓰므로 **판정에는 영향이 없지만**, "둘 다 보고"라는
보고 의무는 여태 미이행이다. (M1(b)는 프로파일 전용이라 판정 무효 사유는 아님.)

**(2) M2 DOM 연산 수 전면 미계측**. §7 M2는 op 종류별 카운트 + 셀당 1회
MutationObserver 교차 검증을 정의하고, §9 E 기각 (i)은 "DOM 연산 수(M2)는
결정적 보조 지표로 강등: E의 셀당 총 DOM op 수 ≥ K의 50%이면 'DOM 연산 절감
없음' 결정적 판정(통계 비적용, **병기**)"으로 그 산출을 명시적으로 요구한다.
하네스에는 카운터도 MutationObserver도 없다(`grep` 확인). E 기각 (i) 자체는
M1(a)로 이미 미발동 판정됐으므로 **그 판정은 유효하되**, 함께 실려야 할
결정적 보조 지표가 통째로 빠진 채 보고된 상태다.
→ 수리 난이도 낮음: `DomOps`가 트레잇이므로 호출을 세어 위임하는 래퍼
   (`CountingDom<D>`) 하나면 op 종류별 카운트가 나온다. MutationObserver
   교차 검증은 셀당 1회만 요구되므로 하네스 JS 쪽에 붙이면 된다.

**(3) 이번에 새로 만든 트랙 T f축 하네스도 같은 기준으로 미달임을 자진 기록**:
§4.1의 "수집 명시(v1.1)"는 **트랙 T 전 셀에서 M1, M2, M4, M5, M6(해당 축)
수집·보고**를 요구하는데, `TrackTCell`이 내는 것은 M1(a) 계열(J-a/J-b)뿐이다.
M2(op 수)·M4(직렬화 페이로드)·M5(메모리)·M6(catch-up 곡선·상주 CPU) 전부
미수집. 위 (2)의 `CountingDom` 래퍼가 생기면 M2는 트랙 L·T 양쪽에서 동시에
해결된다.

**감사 결론**: C 건은 단발 사고가 아니었다 — 하네스는 §9의 **시간 판정
경로(M1(a))는 충실히 구현**했지만, 조항들이 함께 요구하는 **보조·결정적
지표(M2)와 보고 의무(M1(b))는 계통적으로 빠져 있다**. 지금까지 내려진 판정
(D 조건부 채택, E 조건부 채택, C 판정 비결정)은 전부 M1(a) 기반이라 **소급
무효가 되지는 않으나**, "§9를 기계적으로 적용했다"는 서술은 **시간 판정
조항에 한해서만** 참이다. 이 한정을 앞으로 빼놓지 말 것.

### 트랙 T f축 캠페인 실행 + §9 B/F 조항 적용 (2026-07-28)

원시 데이터: `data/rq1-track-t-frequency-chromium-b-{unthrottled,throttled4x}.json`
(커밋 2ac8dd3, 5f3e85f — §11-5 배치별 즉시 커밋). 분석:
`analysis/rq1-track-t-bf-gate.py`.

**수집 조건 — 사전 등록값 그대로**: 비스로틀 f∈{0.1,1,10,30,60}/s ×
s∈{1,10,100,1000} 20셀, 4× 스로틀은 §8이 지정한 s∈{100,1000} × f∈{0.1,1,10}
6셀. 양쪽 다 셀당 독립 10시행(시행마다 fresh page load) × 창 60초. **전 셀에서
`crossOriginIsolated=true`, 실측 타이머 해상도 0.005ms** — 이 저장소에서 §8
타이머 조항을 충족한 첫 측정이다.

**B의 spawn 비용(= toggle_on median), s=100 판정 행**:

| f (/s) | 비스로틀 ms/100ent | 4× 스로틀 ms/100ent |
|---|---|---|
| 0.1 | 0.8950 | 3.5975 |
| 1 | 0.3775 | 2.1975 |
| 10 | 0.2850 | 1.7825 |

**§9 "또는" 분기 — 미발동(측정으로 확정)**. 조문은 "B의 spawn 비용이 전
그리드에서 < 1ms/100 entity — 단 **비스로틀과 4× 스로틀 양쪽 모두에서 성립할
때만 발동**"이다. 비스로틀은 최댓값 0.8950으로 **간신히** 성립하지만, 4×
스로틀에서 최댓값 3.5975로 **임계값의 3.6배** — 불성립. 따라서 분기는 발동하지
않는다.

이것은 §8 감사 지적 **F-2**("<1ms 임계값 고성능 머신 단독 판정" → 스로틀 확장
+ 양쪽 성립 시만 발동)가 예견한 바로 그 상황이 **실측으로 현실화된** 사례다.
그 개정이 없었다면 비스로틀 수치(0.8950 < 1.0)만 보고 "B 유지"로 결론냈을
것이고, 그 결론은 저사양 기기에서 성립하지 않는다.

**§11 개정 #1의 해석 선택이 결과를 좌우하지 않았음(확인)**: 개정 시점에 이미
축소 스모크런 수치를 봤다는 정황을 기록해 두었으므로, 그 선택이 결론을 만들지
않았는지 직접 확인했다 — 4× 스로틀에서 가장 관대한 해석인 **s=1000에서도
최댓값 1.6802로 불성립**이다(s=100은 3.5975). 즉 세 해석 중 어느 것을 택했어도
분기는 미발동이다. 공평성 진술이 실측으로 뒷받침됐다.

**§8 배치 규칙 검증 — 판정 셀 선택이 옳았음**: 해상도 0.005ms 기준 배치 규칙
발동선은 0.25ms인데, s=1(median 0.10~0.18ms)과 s=10(0.145~0.2075ms) 행은 전부
그 아래로 **해상도 바닥에 눌려 있다**. s=100/1000만 바닥 위다. §11 개정 #1이
판정 셀을 s=100으로 고정한 것이 이 점에서도 맞았다. 또 s=1 행의 ms/100entity가
10~18로 튀는 것은 개정 사유로 적었던 "고정 오버헤드가 100배 증폭" 현상이
실측으로 그대로 재현된 것이다.

**트랙 T 종합 — 판정 불가는 유지되나, 사유가 바뀌었다**:

| | 이전(2026-07-24) | 지금 |
|---|---|---|
| "또는" 분기 | f축 부재로 **적용 자체 불가** | **적용 완료 → 미발동**(측정 확정) |
| 1차 분기 | 적용 불가 | **여전히 적용 불가** — F-i/ii/iii 미구현 |

즉 두 분기 중 하나는 이번에 닫혔고, 남은 장벽이 "하네스가 없다"에서 **"arm이
없다"**로 정확히 좁혀졌다. 트랙 T를 실제로 판정하려면 이제 `cond.rs`에
F-i(`CondEffective` 값 필터)/F-ii(detached DOM 계속 패치)/F-iii(`Without<Frozen>`
marker)를 구현하는 것 **하나만** 남는다(그리고 §4.1의 부속 축 — attached 구간
변경률 축이 F-i의 상주 게이팅 비용이 드러나는 유일한 지점).

### 트랙 T F 계열 3변형 구현 — "arm이 없다"는 장벽 해소 (2026-07-28)

앞선 f축 캠페인이 §9 "또는" 분기를 미발동으로 확정하면서 트랙 T의 남은 장벽이
**"F-i/ii/iii가 구현 자체가 없다"** 하나로 좁혀졌다. 그것을 구현했다.

**공통 코어(RQ1 문언 그대로)**: 토글-off 시 DOM을 파괴하지 않고 **fragment로
분리 보관**한다(`DomOps::detach_to_fragment`/`attach_from_fragment` 신설,
`FragmentHandle`은 비직렬화 사이드테이블 인덱스 — Component엔 정수만, CLAUDE.md
금지 4 정합). entity와 내부 `Signal`이 토글을 넘어 살아남는 것이 B가 원리적으로
줄 수 없는 능력이자 F의 진짜 논거다. CSS 은닉과 다른 점: 은닉은 노드가 문서에
남아 형제 셀렉터·레이아웃에 계속 참여하지만 분리는 문서에서 완전히 빠진다.

**세 변형과 각자의 catch-up**(§10 "변형별 자기 명세" 그대로):

| 변형 | 게이팅 | catch-up |
|---|---|---|
| F-i | `CondEffective(bool)` 값 필터(상시 런타임 분기) | 재부착 시 전파 System 1회 |
| F-ii | 없음(detached DOM에 계속 패치) | 불필요 — 멈춘 적이 없음 |
| F-iii | `Without<Frozen>` marker(토글당 archetype 이동 2회) | 버전 카운터 쌍, 불일치분만 |

F-iii의 버전 쌍은 **site별 Component**로 분리했다 — RQ1이 지적한 대로 공유
`DomBinding` 타입에 넣으면 모든 patch System이 그 타입의 writer가 되어 patch
병렬성이 매 프레임 붕괴하기 때문이다.

**§4.1 "상류 Signal 변경률" 축도 함께 구현**했다(`--rate`). 이게 없으면 **세
변형이 구별되지 않는다** — 게이팅 비용은 게이팅할 일이 있어야 드러나기
때문이고, 2026-07-28 f축 스모크런에서 실제로 그렇게 관측됐다.

**파일럿 관측(판정 아님 — 창 5초·1시행, 사전 등록값 60초×10시행 미달)**,
Chromium, rate=100%, 창 안 누적 상주 비용(residency, ms):

| arm | s=10,f=1 | s=10,f=10 | s=100,f=1 | s=100,f=10 |
|---|---|---|---|---|
| B | 4.50 | 3.97 | 4.24 | **3.07** |
| F(CSS 은닉) | 4.86 | 3.67 | 4.55 | 3.26 |
| F-i | 12.38 | 12.04 | 32.69 | 33.90 |
| F-ii | 17.05 | 17.39 | 46.60 | **46.74** |
| F-iii | 12.93 | 12.57 | 33.30 | 33.26 |

RQ1의 사전 예측과 방향이 맞는다: B의 상주 비용이 사실상 0(§4.1의 "B=0, 부재
자체가 데이터"), F-ii가 가장 비쌈(detached DOM에 계속 쓰는 죽은 작업),
F-i·F-iii는 근접 — RQ1이 "marker-vs-value 비용 구조는 전제부터 실측 대상"
이라 한 지점이 이제 **측정 가능**해졌다.

**정합성은 네이티브 테스트로 먼저 못박았다**(`cond_candidates.rs`의 `f_variants`
6개): 분리가 despawn이 아님(entity·상태 보존), F-i의 값 필터가 서브트리 전체에
쓰이는 것, F-iii의 archetype 왕복, **세 변형이 분리 중에 실제로 다른 일을 함**,
그리고 catch-up이 stale DOM을 실제로 복구함. 음성 대조 2건으로 이 테스트들이
공허하지 않음도 확인(catch-up 제거 → 실패, 값 필터 무력화 → 실패).

**구현 중 발견한 실제 버그**: 처음엔 anchor **자신**을 fragment로 옮겼는데,
재부착 시 "자기 자신을 자기 안에 넣는" DOM 계층 오류가 나 F-i 셀이 무한
대기했다. anchor는 문서에 남기고 **그 자식들**(= 조건부 서브트리)만 옮기도록
수정 — 의미상으로도 이쪽이 맞다.

**남은 갭**: (1) §4.1의 6번째 arm("lazy-spawn-then-hide"를 CSS 은닉과 분리한
것)은 여전히 미구현 — 현재 F가 둘을 겸한다. (2) 부속 축 중 서브트리 구성
(flat vs 파생 체인 깊이 3)과 catch-up 곡선(동결 기간 × 변경률)은 미구현.
(3) F-iii의 `BindingVersion`을 하네스가 자식에 삽입하지 않아 그 catch-up
경로는 브라우저 측정에서 미가동(네이티브 테스트로만 검증됨).

### R2 축 수리 — C 기각 앞 분기가 마침내 **정당하게** 측정됨 (2026-07-28, 2단계)

앞서 "§11-1(b) 측정 방법 결함으로 측정 무효"로 진단했던 것을 수리했다.

**무엇을 고쳤나**: R2 축의 추가 binding을 "spawn 시 1회 만들어지는 **정적 텍스트
노드**"에서 "**슬롯 값에서 파생되어, 값이 바뀌면 전부 재기록되는** binding"으로
바꿨다. 구체적으로 —

- `DomWrite::AppendStaticChildren` 처리부가 `append_static_text`(핸들 없음)
  대신 **추적되는 엘리먼트**를 만들고 그 핸들을 `RowBindingTargets`에 저장한다.
  핸들이 있어야 다시 쓸 수 있고, 그래야 R2가 프로토콜 원문의 "binding 8개"를
  실제로 모델링한다(binding site = 동적 텍스트를 갖는 엘리먼트).
- 새 `DomWrite::SetRowBindingTexts`가 그 핸들들을 재기록한다.
- `list_reconcile_c_system`의 유지 슬롯 경로가 슬롯 값이 바뀔 때 이걸 낸다 —
  이것이 RQ1이 예측한 O(n×binding) 비용이다.
- keyed 후보(D/E)는 retained row의 **아이템이 바뀌지 않으므로** 이 write를
  내지 않는다. 그 비대칭이 곧 RQ1의 예측이고, 수리 전에는 보이지 않던 것이다.

인수 조건은 네이티브 테스트로 먼저 못박았다
(`list_candidates.rs`의 `r2_value_derived_bindings`): 역순 재정렬에서 C는
슬롯당 파생 binding까지 재기록(≥32회)하고 D는 **0회**다.

**결과 — 방향이 뒤집혔다**(§2.5 재수집 v9, n=1000 String, W2×R2):

| | Chromium C/D 비율 99% CI | Firefox |
|---|---|---|
| v6 (정적 binding) | `[0.312, 0.400]` — C가 3배 **빠름** | `[0.091, 0.200]` |
| **v9 (값 파생)** | **`[1.736, 2.290]`** — C가 ~2배 **느림** | `[0.819, 0.951]` |

**§9 C 기각 앞 분기: 미발동(정당한 판정)**. 임계값은 "CI 하한 ≥ 3"인데 하한이
1.736이다. 중요한 것은 **미발동의 사유가 바뀌었다**는 점이다 — 이전에는 계측이
조항의 대상 비용을 담지 않아 판정 자체가 무효였고, 지금은 **실제로 3배까지
느리지는 않아서** 미발동이다. 방향은 RQ1 예측대로 뒤집혔다.

**Firefox가 방향을 뒤집는다**(C가 근소하게 빠름) — §8 커버리지 표대로 판정은
Chromium 1차 판정 열에서만 내리되, 이 불일치는 **§11-1(b) 검토 대상으로
등재**한다(자동 판정 변경 아님).

**C의 남은 판정 경로**: 기각은 이제 뒷 분기(W2 × R3에서 archetype move 구간이
스크립트 시간의 50% 이상)에만 달려 있다. §7.1 단계 귀속 계측은 이미 구현됐고
(`stage.rs`), 남은 것은 **R3 row 유형**(값에 따라 `<A/>`/`<B/>` 구조 분기,
전환형은 §10이 archetype move형으로 사전 고정) 하나다.

### §7 M2(DOM 연산 수) 배선 — 계통적 미계측 해소 (2026-07-28, 3단계)

계측 커버리지 감사에서 "전면 미계측"으로 드러났던 M2를 하네스에 연결했다.
`CountingDom<BrowserDom>` 데코레이터를 끼우기만 하면 되므로 후보 코드는 무변경
(§2.4 "후보 코드 무침습"). 반복 경계에서 카운터를 리셋해 셋업 op가 측정에
새지 않게 했다(§8 "각 반복 전 초기 상태 복원"과 같은 지점).

**§9 E 기각 (i)의 결정적 보조 판정** — 조항이 "E의 셀당 총 DOM op 수 ≥ K의
50%이면 'DOM 연산 절감 없음' 결정적 판정(통계 비적용, **병기**)"이라 요구한 것을
이제 실제로 낼 수 있다(Chromium, R2, n=1000):

| 워크로드 | E ops | K ops | E/K | 판정 |
|---|---|---|---|---|
| swap | 999 | 16001 | 0.062 | 절감 있음 |
| shuffle | 992 | 16001 | 0.062 | 절감 있음 |
| shuffle10 | 953 | 16001 | 0.060 | 절감 있음 |
| head_insert | 17 | 16017 | 0.001 | 절감 있음 |

전부 0.5에 한참 못 미치므로 "DOM 연산 절감 없음" 판정은 **미발동** — E 기각 (i)이
M1(a)로 이미 미발동인 것과 같은 방향이고, 이제 조항이 요구한 병기가 실제로
이루어진다.

**op 종류별 분해가 RQ1의 예측을 그대로 보여준다**(R2 × shuffle, n=1000):

| 후보 | create | setText | reposition | total |
|---|---|---|---|---|
| K | 8000 | 8000 | 0 | 16001 |
| C | 0 | **8000** | 0 | 8000 |
| D / E | 0 | 0 | 992 | 992 |

- K는 매 tick 전량 재생성(1000 row × 8 binding).
- C는 create 0 · setText 8000 — 슬롯을 재사용하되 **파생 binding까지 전부
  재기록**한다. 이것이 바로 위 R2 축 수리가 살려낸 O(n×binding) 비용이며,
  시간(M1(a))이 아니라 **연산 수**로도 확인된 셈이다.
- D/E는 reposition 992회뿐, 텍스트 쓰기 0 — retained row를 전혀 건드리지 않는다.

**남은 갭**: (1) §7 M2가 요구하는 **MutationObserver 교차 검증**(셀당 1회)은
미구현 — `DomOpCounts::mutation_observer_channel`에 대응표를 코드로 고정해
두었으나 실제 대조는 하지 않았다. (2) `moveBefore` 네이티브 vs `insertBefore`
폴백 분해도 미구현(`BrowserDom::reposition` 내부 분기라 데코레이터에서 안 보임).
(3) M1(b)(double-rAF 프레임 완결 시간)는 여전히 미계측.

### R3 축 구현 + §9 C 기각 (ii) 적용 — C의 **두 기각 분기가 모두 실측됨** (2026-07-28, 4단계)

C 판정을 막고 있던 마지막 조각인 R3 row 유형을 구현했다.

**구현**(§10 사전 고정 그대로): R3를 켜면 각 row가 자기 **값에서 파생된** 브랜치
marker(`RowVariantA`/`RowVariantB`)를 갖는다. 슬롯의 아이템이 바뀌어 브랜치가
달라지면 **컴포넌트를 교체**한다 — §10이 전환형을 "슬롯 entity 컴포넌트 교체
(archetype move)형"으로 사전 고정했으므로 그대로 따랐다. keyed 후보는 retained
row의 아이템이 바뀌지 않아 교체가 없다(테스트로 못박음: C는 flips>0, D는 0).

**§7.1 계측을 실제로 쓰다가 발견한 결함 — `chain()`의 자동 sync point 삽입**:
처음 측정에서 archetype move만 하는 System조차 `sync_point_ms`가 N과 무관하게
1.2µs로 평평했다(반면 `systems_ms`는 N에 정확히 선형: N=1,000 → 1.37ms,
N=10,000 → 14.04ms). 원인은 Bevy가 `chain()`으로 순서를 걸면 deferred 버퍼를
가진 System과 그 의존 System 사이에 **sync point를 자동 삽입**한다는 것 —
Commands가 우리의 명시적 `ApplyDeferred`보다 **먼저** 적용되어 비용이
`systems_ms`로 잘못 귀속됐다. `chain_ignore_deferred()`로 바꾸자 같은 프로브가
sync 75~93%로 뒤집혔다. **즉 §7.1 계측은 도입 시점부터 이 결함을 갖고 있었고,
R3를 실제로 재려 하지 않았다면 드러나지 않았을 것이다.**

(같은 라운드에 하네스 배선 실수도 하나 있었다 — `set_row_variant_mode` 호출이
실제로 삽입되지 않아 R3가 JSON에 보고만 되고 적용은 안 된 상태로 한 번
측정됐다. C의 sync_point가 0으로 나온 것을 이상하게 여겨 네이티브와 대조하다
발견했다.)

**§9 C 기각 (ii) 적용**(Chromium 1차 판정, W2 × R3, n=1,000):

| 후보 | systems | sync_point | dom_apply | total | sync/total | 판정 |
|---|---|---|---|---|---|---|
| **C** | 0.120 | **0.140** | 0.405 | 0.665 | **21.1%** | 미발동 |
| **D** | 0.815 | 0.000 | 0.975 | 1.790 | 0.0% | 미발동 |
| K | 0.055 | 0.080 | 0.895 | 1.030 | 7.8% | 미발동 |
| E / D-LIS / E-LIS | ~0.9 | 0.000 | ~0.6 | ~1.5 | 0.0% | 미발동 |

C에서 archetype move가 **실제로 일어나고**(0.140ms, D/E는 정확히 0 — 예측된
비대칭), 그러나 스크립트 시간의 **21.1%로 지배적이지 않다**. 임계값 50% 미달로
**기각 (ii) 미발동**.

**C의 §9 종합 — 두 기각 분기 모두 실측 완료, 여전히 판정 비결정**:

| 조항 | 상태 | 근거 |
|---|---|---|
| 채택 (a) {W1..W4}×R1 전 셀 D 대비 동등 이상 | **불충족** | head_insert에서 C vs D CI=[2.526,3.857] 열세 |
| 채택 (b) 최소 1셀 K 대비 우세 | 충족 | swap/shuffle/shuffle10 전부 우세 |
| 기각 앞 분기 W2×R2 CI 하한 ≥ 3 | **미발동** | CI=[2.035, 3.084] — 하한 2.035 < 3 |
| 기각 뒷 분기 W2×R3 archetype move ≥ 50% | **미발동** | 21.1% |

세 조건 모두 충족을 요구하는 채택은 (a) 위반으로 불성립, 기각은 두 분기 모두
미발동 — 따라서 C는 **판정 비결정**(§8 판정 어휘의 그 범주)이 확정된다.
**중요한 변화**: 이전에는 두 기각 분기가 *측정 불가*였고(앞 분기는 계측 결함,
뒷 분기는 R3 부재) 그래서 "미검증"이었다. 지금은 **둘 다 실제로 측정한 뒤의
미발동**이다 — 같은 "비결정"이지만 근거의 성격이 완전히 다르다.

기각 앞 분기가 CI 하한 2.035로 임계값 3에 **꽤 근접**했다는 점은 프로파일로
기록해 둔다(§9는 임계값 미달을 미발동으로 처리할 뿐 근접도를 반영하지 않지만,
n·row 깊이를 더 키우면 발동할 여지가 있다는 뜻이다 — 다만 그건 새 사전 등록
(v2)이 필요하며 같은 데이터에 새 기준을 소급 적용할 수는 없다, §11-3).

### §9 E 기각 (ii) 적용 — §7.1 수리의 부수 효과 (2026-07-28, 4단계 직후)

R3를 재려다 §7.1 계측을 고친 결과, **E 기각 (ii)도 즉시 계산 가능해졌다**.
그 조항은 "reconcile System 시간(매칭 + LIS + successor 검색 + 이동 집합 정렬)
≥ (K의 DOM 배치 적용 시간 − E의 DOM 배치 적용 시간)이면 '매칭 비용이 절감분
상쇄'로 기각"인데, 세 항이 전부 §7.1 구간이라 계측 없이는 손댈 수 없었다.

Chromium 1차 판정, R2, n=1,000, 판정 셀(W1·W3 = swap·shuffle10):

| 워크로드 | E systems | K dom_apply | E dom_apply | 절감분 | 판정 |
|---|---|---|---|---|---|
| swap | 0.4500 | 6.5950 | 0.8600 | 5.7350 | 미발동 |
| shuffle10 | 0.5250 | 7.0300 | 0.8250 | 6.2050 | 미발동 |

E의 매칭 비용이 DOM 절감분의 **8% 남짓**이라 상쇄와는 거리가 멀다 →
**기각 (ii) 미발동**. E의 남은 기각 조항은 (iii) G0(별도 심사)뿐이고,
채택은 여전히 §5 FV 계류에 걸려 있다.

---

## §8 완전 준수 첫 수집(v15)에 §9 전 조항 기계 적용 (2026-08-03)

이전 라운드들과 다른 점은 **조항이 요구하는 조건에서 처음 쟀다**는 것이다.
그동안 트랙 L 수집은 워밍업이 없었고(§8은 50회 요구), 반복이 n에 따라
30/15/6이었으며(§8은 200회), 복원과 측정 사이 double-rAF 경계가 없었고,
시드에 **셀 ID가 아예 들어 있지 않아** 서로 다른 셀이 같은 난수열을 썼다.
그리고 §9 조항 다섯 개는 **어느 스크립트도 계산하지 않고 있었다**.

v15는 그 전부를 이행한 상태에서 수집했고(`rq1-verdicts.py`가 §9 트랙 L
조항을 하나도 빠뜨리지 않고 순서대로 적용한다), 판정 앞에 무결성 점검을
세웠다 — Chromium·Firefox 둘 다 **문제 0건**.

수집: 252 레코드 × 2 브라우저, 워밍업 50 + 측정 200, COOP/COEP 격리 확인
(Chromium 0.005ms / Firefox 0.020ms — 엔진별 격리 하한),
Chromium 56분 / Firefox 134분.

### 판정 (Chromium 1차 판정 — §8 커버리지 표)

| 후보 | 결론 | 근거 |
|---|---|---|
| **C** | **채택 불가**, 기각도 **미발동** | 채택 (a) 불충족 — W4(head_insert) × R1에서 D 대비 열세 CI=[2.206, 2.396]. 채택 (b)는 {W1..W4} × R1 **전 셀**에서 K 대비 우세로 충족. 기각 앞 분기 미발동(W2×R2 하한 1.953 < 3), 뒷 분기 미발동(R3 archetype move 16.7% < 50%) |
| **D** | 기각 (iii) **불성립**, 채택 양성 조건 **충족** | 앞항이 명확히 불성립 — K가 D 대비 동등 이상이어야 하는데 8개 판정 셀 중 7개에서 K가 열세(R2에서 4~22배), 나머지 1개는 비결정. 뒷항(M4/M7/M3)은 미계측이나 §9가 연접을 명시하므로 결론 불변 |
| **E** | 기각 (i)·(ii) 모두 **미발동** | 네이티브 경로 확인(reposition 네이티브 496,258 / 폴백 0 — §9 E-1 요건). W1·W3 × R2 양쪽에서 K 대비 우세. 병기 지표: E의 총 DOM op가 K의 **6.2%**(임계 50%). 기각 (ii): E systems 1.215ms < 절감분 9.665ms |

D·E는 §5 FV 계류가 남아 있으므로 최대 **"조건부 채택(FV 계류)"**이고, §9
게이트 순서상 앞선 **G2(트랙 N property)는 아직 실행되지 않았다** — 지금
판정은 그 게이트를 건너뛴 상태에서 나온 것이다.

> **2026-08-08 정정**: G2는 이후 실행됐고 **K/D/D-LIS/E/E-LIS 5개 arm 전부
> 통과**했다(사이클 1, 이 문서 말미 "트랙 N 사이클 1" 절). 따라서 위 문단의
> "건너뛴 상태"는 D·E에 대해 해소됐다. 다만 **후보 C는 arm이 만들어지지 않아
> 여전히 G2를 안 거쳤고**, §5 FV 계류는 그대로 남아 있다.

### C가 걸린 지점의 메커니즘

C 채택 (a)를 깨뜨린 것은 **W4 한 셀**이고, 원인은 위치 귀속 의미론의 구조적
비용이다. n=1,000 head_insert에서:

| 후보 | median | setText | create | reposition | 총 op |
|---|---|---|---|---|---|
| C | 1.1900ms | **1001** | 1 | 0 | 1002 |
| D | 0.5150ms | 1 | 1 | 1 | **3** |
| E | 0.5600ms | 1 | 1 | 1 | **3** |
| K | 2.7075ms | 1001 | 1001 | 0 | 2003 |

머리에 하나를 끼우면 C는 전 슬롯 값을 다시 쓴다. §9는 "전 셀에서 동등 이상"을
요구하므로 한 셀 열세로 (a) 불충족이다.

### 그런데 이 관계는 n에 따라 뒤집힌다 (원칙 6 — 병기)

W4 × R1, Chromium median:

| n | C | D | |
|---|---|---|---|
| 100 | 0.0265 | 0.0455 | C 우세 |
| **1,000** | **1.1900** | **0.5150** | **C 열세 (판정 셀)** |
| 10,000 | 9.2775 | 12.1050 | C 우세 |

n=10,000에서 C가 setText 10,001회로 D의 3 op를 **이긴다**. D의 매칭 비용
(10,000개 String 키 해싱 + HashMap 재구축)이 그 규모에서 DOM 절감분을 넘어선다는
뜻이다. §9 판정 n은 §3.3이 1,000으로 고정했으므로 판정은 위 그대로이되, 이
비단조성은 "어느 후보가 이기는 영역 지도"에 그대로 들어간다.

### Firefox 교차 확인 — §11-1(b) 검토 대상 등재 사유 발생

**게이트 결론 셋은 전부 일치한다**(C 채택 불가/기각 미발동, D 기각 불성립/양성
충족, E 기각 미발동). 그러나 **셀 단위 우세 방향은 비교쌍 40건 중 11건에서
갈리고, 그중 9건이 정반대(우세↔열세)다.**

| 셀 | 비교 | Chromium | Firefox | 걸린 조항 |
|---|---|---|---|---|
| W1×R1 | E/K | 우세 | **열세** | E 기각(i) |
| W2×R1 | K/D · D/K · E/K | 열세·우세·우세 | **우세·열세·열세** | D 기각(iii)앞항 · D 양성 · E 기각(i) |
| W3×R1 | K/D · D/K · E/K | 열세·우세·우세 | **우세·열세·열세** | 〃 |
| W2×R2 | C/D | 열세 | **우세** | C 채택(a) |
| W3×R2 | C/D | 열세 | **우세** | 〃 |

**뒤집힘이 R1에 몰려 있고 판정 행인 R2에서는 대체로 일치한다**는 것이 구조적으로
읽힌다 — R1(row당 binding 1개)에서는 K의 재스폰 비용이 낮아 Firefox의 훨씬
느린 reposition 경로가 K를 이기게 만들고, R2(binding 8개)에서는 K의 재스폰
비용이 8배라 두 브라우저 모두 D/E가 이긴다. (예외: C/D는 R2에서 뒤집히는데,
그건 Firefox에서 D의 매칭 비용이 커져 C가 상대적으로 유리해지기 때문이다.)

§8이 브라우저 간 풀링·평균을 금지하므로 판정은 Chromium 단독으로 유지하되,
이 목록 자체를 §11-1(b) 검토 대상으로 등재한다.

### Firefox가 2.4배 느린 이유 — 폴백이 아니다

먼저 배제된 것: **양쪽 다 네이티브 `moveBefore` 경로다**(폴백 0). v15에서
`reposition_native_cell_total`/`reposition_fallback_cell_total`을 처음 수집해
확인했다(§7 M2가 요구하던 분해인데 그동안 미계측이었다). Firefox 151, 양쪽 다
`crossOriginIsolated=true` 실측 확인.

느려짐은 **균일하지 않다.** n=10,000 shuffle, §7.1 단계 귀속(Chromium→Firefox):

| 후보 | systems | dom_apply |
|---|---|---|
| K | 0.580 → 2.980 (5.1×) | 21.555 → 50.580 (2.3×) |
| C | 1.235 → 4.500 (3.6×) | 8.885 → 12.680 (1.4×) |
| D | 48.905 → **670.920 (13.7×)** | 12.460 → 274.700 (22.0×) |
| E | 49.150 → **640.120 (13.0×)** | 12.200 → 268.620 (22.0×) |

`dom_apply`의 22배는 Firefox의 네이티브 `moveBefore`가 Chromium보다 느리다는
것으로 읽히지만, **`systems`의 13.7배는 순수 wasm 실행 시간**이라 설명이 필요하다
(엔진 간 wasm 성능 차는 보통 2배 이내). 별도 조사 중이며, 규명 전까지는 원인
미상으로 둔다 — 추정으로 채우지 않는다.

### W7(값 갱신) 프로파일

Chromium n=1,000 R1: C 100 op(정확히 10%) / D·E 400 op / K 2001 op.
§3.4의 "D (retain 경로)" 기대는 별도 key 채널을 전제한 것이고 Phase 1의 D는
키가 표시 텍스트 자체라 값 변경 = 키 변경이 되어 retain하지 못한다
(§11 개정 #3에 기록).

---

## v18 — 감사 지적 전부 반영 후 첫 완전 판정 (2026-08-04)

v15~v17은 각각 측정 방법 결함이 발견되어 §11-1(b)로 무효 표기됐다(개정 #4:
Playwright 번들 Firefox 비대표 빌드, 개정 #5: 배치 묶음 구성, 개정 #6: 페이지
격리 미이행). v18은 두 차례의 적대적 감사(총 지적 82건, 확증 49건)가 잡은
결함을 전부 수리한 뒤의 수집이다:

- stock 브라우저(Chrome 150 / Firefox 150.0.1), 레코드마다 `user_agent` 기록
- 페이지 격리(그룹 모드 — 42회 fresh page load)
- 배치 묶음: 전부 baseline↔target 교대 + **묶음 내 매 회 double-rAF 경계**
- 후보 **rep 단위 라운드로빈**(블록 순차 아님 — paired bootstrap의 전제 복구)
- t0를 조작 트리거 **직전**으로(§7 M1(a) 문언)
- **판정 셀 전부 §3.2 기본값(POD)** — R2에 POD 셀이 처음으로 존재
- §9의 D = **D-LIS**(§10 D-4 "LIS 고정"), 등가 대역 [1/1.1, 1.1] 적용,
  E 기각 (ii)를 판정 셀(W1·W3 × R2)에서 계산

무결성: Chromium 0건. Firefox 1건 — n=10,000 swap 셀이 §8 셀 상한(30분)에서
119표본으로 중단, 조항이 규정한 "측정 불가 프로파일 등재"의 정상 작동이며
라운드로빈이라 6후보가 같은 rep에서 잘려 짝짓기 대칭이 유지된다.

### 판정 (Chromium 1차, POD, n=1,000)

| 후보 | 결론 | 근거 |
|---|---|---|
| **C** | **채택 불가**, 기각 미발동 | (a) 불충족 — W3 열세 [1.055, 1.114], W4 열세 [1.564, 1.615]. (b) 충족(전 셀 K 대비 우세). 기각 앞 분기 1.597 < 3, 뒷 분기 14.9% < 50% |
| **D**(-LIS) | 기각 불성립, 양성 충족, **W5 부속 발동** | K가 8개 판정 셀 전체에서 D 대비 동등 이상이 아님(전 셀 열세). 양성: R2 전 셀 우세. **W5에서 C 대비 4.9배 열세 → §9 문언대로 "unkeyed 정적 opt-out 경로 명세"가 채택 조건에 부속됨** |
| **E** | 기각 (i)·(ii) 미발동 | 판정 셀 양쪽에서 K 대비 우세(0.230~0.239 / 0.196~0.209), DOM op 6.1~6.2%(임계 50%), systems 0.65~0.86ms ≪ 절감분 11.2~13.1ms. 네이티브 경로 확인(폴백 0) |

D·E의 §5 FV 계류와 G2(트랙 N) 미실행은 그대로 남아 있다 — 이 판정은 여전히
그 게이트들을 통과하지 못한 상태의 것이다.

**v15 판정과의 차이**: C 채택 (a)의 열세 셀이 W4 하나에서 **W3·W4 둘**로
늘었다. 두 원인이 겹쳤다 — (i) 기준선이 §10대로 D-LIS로 바뀌었고(무LIS D보다
W3에서 빠르다), (ii) 판정 아이템이 POD로 바뀌면서 C의 setText 비용 구조가
달라졌다. 이전 판정이 틀린 데이터에서 나온 것이었으므로 "판정이 바뀌었다"가
아니라 **"처음으로 유효한 판정이 나왔다"**로 읽는 것이 맞다.

### 브라우저 간 방향 뒤집힘 — 이제 실체다 (§11-1(b) 등재)

빌드 아티팩트(개정 #4)도, 배치 결함(개정 #5)도, 블록 순차도 없는 데이터에서
**9건이 여전히 정반대**다: W1·W2·W3 × R1의 {K/D, D/K, E/K}와 W2·W3 × R2의
C/D. 원인은 이미 계측으로 분해돼 있다 — **Firefox의 네이티브 `moveBefore`가
Chromium보다 ~18배 느리다**(stock 빌드에서 실측, dom_apply 단계 귀속).
reposition을 내는 D/E가 R1(재스폰이 싼 구간)에서 K에게 지는 것은 그 귀결이다.

§8대로 판정은 Chromium 단독을 유지하지만, 이 관측의 함의는 판정보다 넓다:
**E의 "채택 시 조건부 제약"(§9)에 Firefox가 사실상 포함되어야 한다** — 폴백
환경(WebKit)만이 아니라 네이티브인데 느린 환경도 있다는 것이 실측됐다.
W4 × {R1,R2}는 배치 모드가 브라우저 간에 달라(해상도 차이 → 임계값 차이)
비교 불가로 제외했다 — §8 "해상도 × 50" 문언의 구조적 귀결이며, 그 문언을
바꾸는 것은 §11-1(d)상 불가.

---

## 트랙 T v2 캠페인 — §4.1 완전 준수 첫 수집과 §9 B/F 전 조항 적용 (2026-08-07)

§4.1이 요구하는 전부를 처음으로 갖춘 수집이다: 6 arm(B, CSS 은닉, F-i, F-ii,
F-iii, lazy-spawn-then-hide) × 통상 구간(f∈{0.1,1,10} × s∈{1,10,100}) ×
변경률 {0,10,100%} + 4× 스로틀(s∈{100,1000}) + catch-up 곡선 축(M6) + 구성
축(chain3, s=100행). 셀당 10시행 × 60초, 시행마다 fresh page load, Chromium
1차. 총 벽시계 약 44시간, 13파일 전부 §11-5대로 즉시 커밋, 무결성 문제 0건.
판정 절차(`rq1-track-t-verdicts.py`)는 §8 "분석 절차 동결"대로 **실행 전에
커밋**했고, 판정 셀 해석(15개 정본 조건)과 방법론 이탈(비쌍대 부트스트랩 —
트랙 T에는 쌍대의 전제가 없음)을 절차 안에 사전 명시했다.

### 결과: "또는" 분기 발동 — 결론 = B 유지, F 계열 기각

**1차 분기(승리 지도)는 세 변형 모두 생존이다.** B를 J-a·J-b 모두에서(99%
CI 우세) 이기는 셀이 F-i 4/9, F-ii 5/9, F-iii 4/9. 승리는 고빈도·대형
구간(f=10, s≥10)과 s=100 × rate>0 구간에 몰려 있다 — §4.2 유불리 지도의
예측 그대로다.

**그러나 "또는" 분기가 발동했다.** B의 s=100 토글-on median이 전 f에서
비스로틀 0.260~0.438ms, 4× 스로틀 0.593~0.812ms — 양쪽 모두 사전 등록 임계
1ms 아래다. §9 문언: "두 경우 모두 결론 = B 유지." 따라서 캠페인 판정은
**B 유지, F-i/ii/iii 기각**이다.

부속 판정: CSS 처리 규칙 (1)의 지배는 세 변형 모두에서 깨졌다(F 기계장치
도입 근거가 CSS로 상실되지는 않음 — M5 미수집이어도 확정). 거울상 의무 (2)는
세 변형 모두 미발동(B-승리 셀에서 CSS가 동등 이상인 경우가 과반 미달).

### 이전 판정과의 뒤집힘 — 경위를 숨기지 않는다

2026-07-28의 같은 분기 적용은 **미발동**이었다(4× 스로틀 3.598ms ≫ 1.0).
이번에 0.593~0.812ms로 5~6배 내려간 원인은 측정 대상이 아니라 **하네스**다:
구 측정은 `BrowserDom` 노드 슬롯 누수(2026-08-03 감사 확증 — `clear_children`/
`remove_node`가 슬롯을 놓아 주지 않아 분리 노드가 무한 상주) 위에서 수집됐고,
B는 매 토글 despawn이라 그 캠페인 동안 누적된 수십만 슬롯의 할당·GC 압력이
spawn 경로를 정확히 부풀리는 방향이었다. §11-1(b)대로 구 측정은 무효 표기,
이번 측정이 유효본이다.

### §11-3 기록 — 기준의 판별력에 대한 탐색적 관찰 (판정 아님)

기계 적용의 결과는 위와 같고 그대로 선다. 다만 §11-3("기준이 판별력 없다고
판명되면 — 그것도 결과다 — 판정과 별도로 기록")에 따라 적는다: 이번 데이터는
**승리 지도와 "또는" 분기가 반대 방향을 가리키는** 사례다. F 변형들은 사전
등록된 셀-승패 지표로 통상 구간 9셀 중 4~5셀에서 B를 이기는데, "또는" 분기는
B의 절대 spawn 비용 하나로 그 지도를 통째로 기각한다. "spawn이 1ms보다
싸면 F 기계장치가 정당화되지 않는다"는 RQ1 문언의 논리가 "그래도 고빈도
구간에서는 F가 총 CPU를 25~30% 줄인다"는 실측과 충돌하는 지점이다. 새 기준은
새 사전 등록(v2)으로만 가능하며, 이 관찰은 채택/기각 근거로 쓸 수 없다.

의미론 축은 판정과 별도로 남는다(원칙 6): B는 토글마다 서브트리 로컬 상태를
전량 소실한다 — 상태 보존 요건 사이트에서 B가 답이 아니라는 사실은 이 성능
판정으로 지워지지 않으며, M8 기능 매트릭스로 보고한다. catch-up 축은 F-iii의
재부착 비용이 동결 중 변경률 100%에서 B 재스폰을 역전함(0.117→0.552ms vs
B 0.41~0.48ms)을 곡선으로 남겼다 — §4.1 M6이 요구한 교차점이다.

---

## 트랙 N 사이클 1 — G2 게이트 최초 실행 (2026-08-08)

§9 게이트 순서(G0 → G1 → **G2 트랙 N** → 성능 측정)에서 그동안 미실행으로 남아
있던 G2를 처음 돌렸다. 그 전까지의 D·E 판정은 이 게이트를 **건너뛴 상태**에서
나온 것이라고 이 문서가 스스로 적어 두었던 항목이다.

**결과: K/C/D/D-LIS/E/E-LIS 6개 arm 전부 통과**(사이클 1, 사전등록 10^4 시퀀스).
원장: `research-questions/data/rq1-track-n-cycle1.json`
(동결 기반 커밋 `528e991`, 시드 기저 20260808, 시퀀스 길이 ≤200).

| arm | 결과 | 결정성 재현 | 불변식 4 시나리오 |
|-----|------|-------------|-------------------|
| K | 통과 | 200 | 182,869회 (63% 케이스) |
| C | 통과 | 200 | 1,532,299회 (100% 케이스) |
| D | 통과 | 200 | 1,429,377회 (99.99% 케이스) |
| D-LIS | 통과 | 200 | 1,431,850회 (100% 케이스) |
| E | 통과 | 200 | 1,438,507회 (100% 케이스) |
| E-LIS | 통과 | 200 | 1,432,283회 (100% 케이스) |

수리 사이클은 발동하지 않았다(§5: 탈락 시에만 수리 → 재실행 = 1사이클).

### 이 통과가 **말하지 않는** 것

1. ~~후보 C는 이 게이트를 거치지 않았다.~~ **해소(2026-08-08, 같은 날)**:
   C arm을 추가해 §9 문언의 "전 후보 공통"을 충족시켰다. 단순 등록으로는 안
   됐다 — C는 **슬롯의 아이템이 제자리에서 바뀌는 유일한 후보**라 row에서
   파생된 값(술어·slice 길이)이 낡는다. `RowOwned`(row → 소유 mirror entity)와
   `RowSpawner::revalue`(기본 no-op이라 `DefaultRow` 경로 무영향)를 신설해
   닫았고, 그 과정에서 하네스가 결함 둘을 잡았다(아래 "C arm 추가에서 나온 것").
2. **불변식 4는 약한 형태만 덮는다.** 컴파일러가 내는 조건부는 숨김에
   `SetVisible`을 쓰고 patch System을 동결하지 않으므로 "동결 중"이라는 상태가
   없다. 위 표의 "시나리오"는 §5 문언 그대로의 catch-up이 아니라 "숨은 동안 바뀐
   내부 리스트가 다시 보였다"는 더 약한 전이다. 진짜 형태를 재려면 `cond.rs`의
   F 계열(fragment 분리 + `Frozen` 게이팅)을 컴파일러가 낼 수 있어야 한다.
3. **픽스처는 하나다.** `demo/nested-list-derived.tsx` — 외부 리스트 > row별
   조건부 하나 > 내부 리스트 하나. §5의 구조 요건은 충족하지만, 조건부가 둘
   이상이거나 중첩 깊이가 4 이상인 형태는 이 사이클에 없다.
4. **§5 FV 계류는 그대로다.** D·E 채택의 추가 선결 조건(flush 규약 v0의 formal
   verification)은 이 게이트와 독립이고, `plan_writes`(LIS 변형)의 수렴은 아직
   미증명이다.

### 실행 경위 — 두 번 중단하고 고쳤다 (숨기지 않음)

사이클 1은 **세 번 시작해서 세 번째만 완주**했다. 두 번의 중단은 모두
**결과를 판정에 쓰기 전**이었고, 둘 다 §11-1(b) 측정 방법 결함이다.

- **1차 중단**: 생성기의 삽입/삭제 확률이 같아 리스트가 4~5 근처를
  랜덤워크했고 §5 상한 n=50에 **한 번도 닿지 않았다**(400시퀀스 시뮬레이션에서
  상한 도달 0%, 최대 도달 평균 8.4). 불변식 2·3이 겨냥하는 영역이 통째로
  미검사되는 상태였다. 중단 시점에 **arm 결과가 한 줄도 출력되지 않았고 원장
  파일도 생성되지 않았다** — 어떤 결과도 관측하지 않은 채 고쳤다. 수리 후
  커버리지: 외부 최대도달 중앙 28/최대 50/≥40 비율 24%, 평균 길이 4.1 → 19.0
  (회귀 테스트 `tests/coverage.rs`로 고정).
- **2차 중단**: 통과 결과가 나왔으나 **보고 전에** 불변식 4의 검사가 원리적으로
  발동 불가임을 발견했다 — 불변식 1 위반 시 조기 반환하므로 그 검사는 불변식 1이
  통과했을 때만 실행되는데, 그때는 이미 일치한 문자열의 부분을 다시 보는 것이라
  반드시 통과한다. 이 계열은 애초에 불변식 1과 독립적으로 발동할 수 없다(숨은
  동안의 변경이 반영 안 됐다는 것이 곧 DOM ≠ 참조 모델이다). 계열을 구분 짓는
  것은 검사식이 아니라 **어떤 시나리오가 일어났는가**이므로, (a) 불변식 1이 깨진
  tick이 그 전이였으면 계열을 함께 귀속하고 (b) 전이 횟수를 세어 원장에 싣도록
  바꿨다. 0이면 미검사이고 실행기가 경고를 찍는다.

결과를 본 뒤에 생성기나 검사를 손보는 것은 §5가 막으려는 자유도이므로, 이 순서
(보기 전 / 판정 전)가 성립 조건이다. 사이클 수(3) 자체는 소비되지 않았다 —
§5의 사이클은 **탈락 후 수리**로 정의되고, 여기서는 탈락이 없었다.

### C arm 추가에서 나온 것 (2026-08-08)

C를 넣자 하네스가 **둘**을 잡았고, 둘 다 C에서만 터지고 K/D/E는 무영향이었다
(arm 특정성 확인).

1. **파생값 미갱신** — 값 관통으로 슬롯의 아이템이 바뀌었는데 그 row의 안쪽
   리스트 길이가 옛 아이템 길이로 남았다. `revalue` 훅으로 닫았다.
2. **한 틱 지연** — `revalue`가 `Commands`로 mirror의 `Signal`을 갈아 끼우는데,
   같은 스케줄 안에 sync point가 없어 그 삽입이 스케줄이 끝난 뒤에야 적용됐다.
   그러면 그 row의 조건부·중첩 리스트가 다음 틱에야 반응한다. 사이트 시스템들을
   `.chain()`으로 묶어 닫았다(`chain()`은 deferred 버퍼를 가진 시스템 사이에
   sync point를 자동 삽입한다 — `chain_ignore_deferred`는 그러지 않는다).
   음성 대조: `chain_ignore_deferred`로 되돌리면 C만 다시 반례가 난다.

두 결함 모두 **keyed 후보에서는 구조적으로 발생하지 않는다**(retained row의
키=텍스트가 그대로여서 파생값도 그대로다). 즉 C arm이 없었으면 이 경로는 영영
검사되지 않았을 것이고, §9가 G2를 "전 후보 공통"으로 규정한 이유가 이것이다.

### 하네스가 잡은 실제 결함 (사이클 시작 전)

미리 방출하는 초기 row가 후보의 정체성 Component(`RowKey` / `OrderedKey`+
`OrderKey`)를 달지 않아, keyed 후보의 다음 재조정이 그 row들을 못 알아보고 전부
새로 spawn했다. 옛 노드는 DOM에 남아 내용이 중복됐다. **K는 매번 전면 재생성이라
증상이 없어** 후보를 등록 가능하게 만들기 전까지는 드러나지 않던 결함이다.
다섯 번째 케이스에서 잡혔고 수리했다.

### 검사가 공허하지 않음을 확인한 음성 대조

| 주입한 결함 | 잡히는 계열 |
|---|---|
| D의 row 정체성 제거 | D/D-LIS만 (K/E 무영향 — arm 특정성) |
| 재배치 앵커를 인접 핸들로 오염 | **DomSpan**(대표) + ReferenceModel |
| `OrderKey` 재태그 일부 생략 | **E/E-LIS만 OrderConvergence** |
| 앵커 해석 강제 실패 | **FlushOrdering** + ReferenceModel |

불변식 4는 위 (a)/(b) 구조로 처리했다(독립 발동이 원리적으로 불가하므로 검사식
대신 시나리오 발생 횟수로 공허성을 막는다).
