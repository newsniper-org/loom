# Verus 검증 (RQ5 후보 #4 — flush 규약 v0 순서 보장)

`../README.md`(Lean4/TLA+ 스코프)와 함께 볼 것.

## 왜 여기만 Verus인가

`lean4/`의 두 증명(#1 teardown, #3 resumability)은 Lean4 **안에** 상태 기계와
축소 모델을 손으로 다시 쓰고 그걸 증명한다 — 두 파일 모두 "모델이 실제 코드와
같은가는 증명 밖에 남는다"고 스스로 명시하고 있다.

후보 #4는 대상의 성질이 다르다. `LoomApp::tick`의 적용 루프와 `Vec<DomWrite>`
라는 **구체 자료구조**가 대상이고, `bevy_ecs`도 `web_sys`도 개입하지 않는 순수
Rust 구간이라 **실제 함수에 `requires`/`ensures`를 걸 수 있다**. 이 저장소에서
모델-구현 간극이 처음으로 좁혀지는 항목이다.

반대로 Verus를 쓰지 **않는** 곳도 분명히 해 둔다: `bevy_ecs`가 개입하는 전부
(System 스케줄링·archetype move·change detection), `web-sys`/DOM 실호출, SWC
기반 lowering. 그리고 #3의 남은 과제(StringList/Bool/Str 확장)는 *의미론적
동치*에 관한 것이라 Lean4가 계속 맞다.

## 실행

```
verus formal-verification/verus/flush_order.rs
```

현재 결과: **25 verified, 0 errors**(모델 파일) + **22 verified, 0 errors**(`crates/loom-flush-core`, `cargo verus verify`) (Verus 업스트림 설치본, `/usr/bin/verus`).

## 무엇이 증명됐나 (2026-07-28)

**정리 (1) anchor 선행성** — `anchor_precedence`. `place_in_order`가 방출한 write
시퀀스에서, `before: Some(a)`를 실은 모든 write에 대해 그 write가 적용되는
시점에 `a`는 이미 DOM에 존재한다. 따라서 `LoomApp::tick`의 **두 폴백 경로가
한 번도 실행되지 않는다**:

```rust
DomWrite::SpawnRow { …, before } =>
    if let Some(h) = before.and_then(|e| self.dom_target(e)) { … }   // ← 해석 실패 시 노드가 끝에 남음
DomWrite::Reposition { …, before } =>
    let before_handle = before.and_then(|e| self.dom_target(e));      // ← 해석 실패 시 끝으로 이동
```

이 두 폴백은 **에러도 로그도 없이** 노드를 잘못된 위치에 두고, 이후 아무도
고쳐 주지 않는다. 즉 순서 정합성 전체가 이 정리 하나에 걸려 있었다.

이는 RQ1-benchmark-protocol.md §5 트랙 N **불변식 5**("flush 규약 v0 하
spawn/move 인터리빙 — 이동 기준 anchor가 미스폰 row — 가 결정적 순서로
수렴")와 같은 명제다. 트랙 N은 property-based 하네스가 아직 없는데, 이
항목만은 퍼징이 아니라 **증명으로** 확보된 셈이다(§5의 사전 등록 캠페인을
대체하지는 않는다 — 더 강한 증거를 먼저 얻은 것).

**핵심 근거**: `place_in_order`가 `target`을 **역순으로** 순회한다는 것.
`next_anchor`는 항상 target에서 바로 오른쪽 원소이고, 역순이므로 그 원소의
write는 이미 push된 뒤다. **`.rev()` 한 글자가 load-bearing이다.**

## 명세가 공허하지 않음을 확인한 음성 대조

"통과했다"만으로는 명세가 실제로 무언가를 말하는지 알 수 없으므로, 일부러
깨뜨려 보고 **거부되는지** 확인했다(둘 다 `verification results:: 3 verified,
1 errors`로 실패):

| 대조 | 조작 | 결과 |
|---|---|---|
| 1 | anchor를 오른쪽 이웃 → **왼쪽 이웃**으로(정순 순회 상당) | `assertion failed` — 정리 (1) 자체가 깨짐 |
| 2 | 호출부 전제("Retained는 이미 DOM에 있다") 제거 | `assert(current.contains(a))` 실패 |

## 정리 (2) 순서 수렴 — 증명 완료 (2026-07-28)

**`order_converges`**: `emit_writes`가 낸 write를 전부 적용하고 나면 DOM의
**뒤쪽 |target|개가 목표 순서와 정확히 일치**한다.
**`order_converges_exactly`**(따름정리): 잔여 원소가 없으면(길이가 같으면)
최종 DOM이 목표 순서 **그 자체**다 — 순수 재정렬 워크로드(W1~W4, §9 판정 셀)가
정확히 이 경우.

접미사로 서술한 이유(과장 방지): `place_in_order`의 호출부는 target에 없는 기존
row를 **별도 경로로** 제거하므로, 이 함수만 놓고 보면 제거되지 않은 잔여 원소가
앞쪽에 남을 수 있다. 그 잔여를 모델링하지 않고 말할 수 있는 가장 강한 참인
명제가 "뒤쪽 n개 = target"이다.

**증명 구조**: `place_before`(적용 루프 한 걸음 = "e를 떼어내 anchor 바로 앞에
놓기")를 정의하고, 접미사 불변식을 `k`에 대한 귀납으로 증명한다 —
`suffix_invariant`: *k*개를 적용하면 DOM의 뒤쪽 *k*개가 target의 뒤쪽 *k*개와
같다. 한 걸음은 `lemma_place_extends_suffix`가 담당하고, 그 아래로
`remove_value`/`index_of_first`/`insert`가 조각 이어붙이기와 어떻게 상호작용하는지를
L1~L6 여섯 개 보조정리로 깔았다(`no_duplicates` 보존 포함).

정리 (1)이 여기서 쓰이는 방식: (1)이 anchor 해석 실패를 배제하므로
`place_before`의 `None` 폴백 가지가 **도달 불가**가 되고, 그래야 각 걸음이
"anchor 바로 앞에 삽입"으로 확정된다. 즉 (2)는 (1) 위에 서 있다.

## 명세가 공허하지 않음을 확인한 음성 대조

"통과했다"만으로는 명세가 실제로 무언가를 말하는지 알 수 없으므로, 일부러
깨뜨려 보고 **거부되는지** 확인했다(전부 `1 errors`로 실패):

| # | 대상 | 조작 | 결과 |
|---|---|---|---|
| 1 | 정리 (1) | anchor를 오른쪽 → **왼쪽 이웃**으로(정순 순회 상당) | `assertion failed` |
| 2 | 정리 (1) | 호출부 전제("Retained는 이미 DOM에 있다") 제거 | `assert(current.contains(a))` 실패 |
| A | 정리 (2) | `place_before`가 **항상 끝에 붙이도록**(= tick의 폴백이 매번 발동) | `assertion failed` |
| B | 정리 (2) | `apply_k`의 anchor를 왼쪽 이웃으로 | `postcondition not satisfied` |
| C | 정리 (2) | `t.no_duplicates()` 전제 제거 | `assertion failed` |
| D | 스킵 안전성 | 인접 조건을 무시하고 Retained를 **무조건 스킵** | `postcondition not satisfied` |
| E | 스킵 안전성 | 인접 판정을 "한 칸 건너"(`d[i+2]==a`)로 잘못 둠 | `assertion failed` |
| F | Chain 대응 | `unlink`의 중간 노드 갈래에서 **재결선 누락** | `assertion failed` |
| G | Chain 대응 | `insert_before`에서 `prev[a]=e` 갱신 누락 | `assertion failed` |
| H | Chain 대응 | `chain_denotes`에서 `next` 정의역 조항 제거 | `assertion failed` ×4 |

대조 A가 특히 중요하다 — 정리 (1)이 배제하는 바로 그 폴백을 강제로 켜면 (2)가
깨진다는 것이, 두 정리가 실제로 맞물려 있음을 보여 준다.

## `already_adjacent` 스킵 최적화 안전성 — 증명 완료 (2026-07-28)

`place_in_order`는 Retained row가 **이미 올바른 위치에 있으면** write를 방출하지
않는다(`chain.next_of(entity) == Some(a)`, anchor가 없으면 `chain.is_tail`).
앞선 라운드에서는 이 최적화를 모델에서 **생략**하고 "스킵은 write를 줄이기만
하므로 (1)을 깨뜨릴 수 없다"는 비형식 논증만 남겨 두었다. 이번에 정리로 만들었다.

**핵심 관찰**: 스킵된 write는 애초에 **항등(no-op)**이었다.

- `lemma_skip_is_noop`: `e`가 `d`에서 `a` 바로 앞에 있으면
  `place_before(d, e, Some(a)) == d`.
- `lemma_skip_is_noop_tail`: `e`가 `d`의 꼬리면 `place_before(d, e, None) == d`.

**본정리 `skip_optimization_is_safe`**: 스킵하는 실제 구현(`apply_k_opt`)과
스킵하지 않는 상위집합(`apply_k`)이 **같은 DOM을 낸다** — 접미사 성질이 양쪽에서
성립한다는 약한 진술이 아니라 **시퀀스 자체가 동일**하다는 강한 진술이다.
따라서 정리 (2)를 상위집합에 대해 증명한 것으로 충분하고, 따름정리
`order_converges_with_skip`이 실제 구현의 수렴을 직접 준다.

**모델링 노트**: 원본은 인접성 질의를 `Chain`(`next_of`)으로 O(1)에 하지만,
`Chain`은 "지금까지 방출한 write를 적용한 뒤의 순서"를 그대로 추적하는
자료구조이므로 의미는 `adjacent(d, e, a)` 술어와 같다. 이 대응 자체는 눈으로
확인한 것이고 정리로 잇지 않았다(아래 남은 것 참고).

## Chain ↔ adjacent 대응 — 증명 완료 (2026-07-28)

앞선 라운드에서 "원본은 인접성 질의를 `Chain`(`next_of`)으로 O(1)에 하고 모델은
`adjacent` 술어로 하는데, 그 대응은 **코드를 읽어 확인했을 뿐 정리로 잇지
않았다**"고 남겨 둔 간극을 닫았다.

`Chain`은 head/tail + next/prev 두 맵으로 순서를 들고 있다. 그것이 **어떤
시퀀스를 표기하는가**를 `chain_denotes`로 규정하고(도달가능성·무순환을 따로
논증하지 않는 이유는 표기 시퀀스를 고스트로 함께 들고 다니기 때문 — 표현이
아니라 **표현과 의미의 대응**을 불변식으로 유지한다), 네 정리를 증명했다:

| 방향 | 정리 | 내용 |
|---|---|---|
| 질의 | `chain_next_of_iff_adjacent` | `next_of(e) == Some(a)` ⟺ 표기 시퀀스에서 `e`가 `a` 바로 앞 |
| 질의 | `chain_is_tail_iff_last` | `is_tail(e)` ⟺ 표기 시퀀스의 마지막이 `e` |
| 변이 | `chain_unlink_denotes_remove` | `unlink(e)` 후 표기 = `s.remove_value(e)` |
| 변이 | `chain_insert_before_denotes_insert` | `insert_before(e, a)` 후 표기 = `s.insert(j, e)` |

**합성 본정리 `chain_ops_match_place_before`**: `place_in_order`가 비인접
Retained에 대해 하는 `unlink(e)` → `insert_before(e, Some(a))` 시퀀스가 표기
수준에서 정확히 `place_before(s, e, Some(a))`다. 이로써 정리 (2)/스킵 안전성이
`place_before` 위에서 증명한 모든 것이 **원본의 `Chain` 조작에 그대로 적용**된다.

변이 정리들의 맵 조작(`unlink_next`/`unlink_prev`/`ib_next`/`ib_prev` 등)은
`list.rs`의 `match (p, n)` 네 갈래와 `insert_before`의 두 갈래를 1:1로 옮긴
것이다 — 즉 "그 코드가 하는 맵 수술이 실제로 리스트를 올바르게 재결선하는가"가
증명 대상이었고, 대조 F/G가 그 수술의 어느 한 줄만 빠져도 증명이 깨짐을 보인다.

## 모델-구현 결선 — 완료 (2026-07-28)

**`crates/loom-flush-core`가 신설됐다. 검증된 코드가 곧 배포되는 코드다.**

`Chain`이 이제 이 크레이트 안에 있고, 표기 시퀀스를 고스트 필드로 들고 다니며
각 연산의 표기 효과를 **사후조건으로** 못박는다. `crates/loom-runtime/src/
list.rs`의 `place_in_order`/`place_in_order_lis`가 이것을 호출한다 — 손코딩
`Chain`(std `HashMap` 두 개)은 삭제됐다.

| 검증된 실행 메서드 | 사후조건이 말하는 것 |
|---|---|
| `Chain::from_slice` | 표기 = 입력 슬라이스 |
| `Chain::push_back` | 표기 = `s.push(e)` |
| `Chain::unlink` | 표기 = `s.remove_value(e)` |
| `Chain::insert_before` | 표기 = `s.insert(j, e)` (j = a의 위치) |
| `Chain::next_is` | 반환값 ⟺ `adjacent(표기, e, a)` |
| `Chain::is_tail` | 반환값 ⟺ 표기의 마지막이 `e` |

`cargo verus verify` → **12 verified, 0 errors**. 같은 소스가 일반
`cargo build`로도 컴파일된다(ghost 소거) — **wasm32 포함**.

**`vstd` 의존이 wasm에 짐을 남기지 않는가**: 남기지 않는다. ghost가 컴파일
시점에 지워지므로 `wasm32-unknown-unknown` 빌드가 정상 통과하고, 데모 청크
크기도 실측상 변화가 없다.

**id 표현**: `Entity`는 Verus가 다룰 수 없는 불투명 타입이라 호출부가
`Entity::to_bits()`로 `u64`를 만들어 넘긴다(단사이므로 동일성·구별성 보존).

### §2.5 재수집 결과 — 판정은 하나도 바뀌지 않았다

`place_in_order`는 D/E/D-LIS/E-LIS **4개 후보의 공유 경로**이므로 §2.5가
"상이한 기반의 측정치를 같은 판정에 혼용 금지"를 걸어 트랙 L 전면 재수집이
필요했다(v6 → **v7**, Chromium/Firefox 각 180 레코드).

| 셀 | v6 (손코딩) | v7 (Verus 검증) |
|---|---|---|
| Chromium R2×swap, D vs K | `[0.220,0.250]` 우세 | `[0.222,0.239]` 우세 |
| Chromium R2×shuffle10, E vs K | `[0.220,0.273]` 우세 | `[0.259,0.267]` 우세 |
| **Firefox R2×shuffle, D vs K** | `[0.833,1.100]` **비결정** | `[0.865,0.901]` **우세** |
| C head_insert (채택 조건 a) | `[1.333,2.500]` 열세 | `[1.286,1.491]` 열세 |

**§9 판정이 뒤집힌 셀은 없다.** Firefox의 비결정 셀 2개가 우세로 확정된 것은
Chain 교체 때문이 아니라 **COOP/COEP 격리로 타이머 클램프(Firefox 1ms)가
풀린 덕**이다 — v6와 v7 사이에는 Chain 교체와 타이머 해상도 개선이 **동시에**
들어갔으므로 절대 median의 차이는 두 원인으로 분해되지 않는다. 다만 §9 판정이
쓰는 것은 후보 간 **비율**이고, 그 비율의 방향은 전부 보존됐다 — 이 교란을
숨기지 않고 기록한다.

### 방출 루프 이관 완료 — `plan_writes` (2026-07-28, 2단계)

이전 라운드의 남은 항목("`place_in_order`가 Chain의 사전조건을 지키는지는
기계적으로 확인되지 않는다")을 해소했다. **순서 결정 루프 전체가
`loom_flush_core::plan_writes`로 옮겨졌다.**

`list.rs`의 `place_in_order`/`place_in_order_lis`는 이제 `Chain`을 **직접
만지지 않는다** — `plan_writes`가 돌려준 `PlanOp` 열을 `DomWrite`로 옮겨 적는
`emit_plan`만 남았다. 순서 판단이 그 파일에서 완전히 사라졌으므로
`unlink`/`insert_before`/`push_back`의 사전조건은 **전부 검증 크레이트 안에서
증명으로 해소**된다.

루프 불변식 네 개로 사전조건을 유도한다: (a) `chain.well_formed()`,
(b) 처음부터 있던 원소는 사라지지 않는다, (c) 없던 원소가 생기지 않는다
(= chain ⊆ current ∪ 이미 처리한 target), (d) 이미 처리한 target 원소는 chain
안에 있다(= anchor가 항상 해석 가능). 세 분기(Fresh / Retained-스킵 /
Retained-이동)가 공통으로 만족하는 모양을 `same_plus`(표기 집합 = 이전 집합 ∪
{e})로 뽑아 불변식을 균일하게 유도했다.

**LIS 변형도 같은 함수를 쓴다**: `place_in_order_lis`의 "이 원소는 상대 순서가
이미 맞으니 건드리지 말라"는 표시를 `keep` 파라미터로 받는다. 스킵은 write를
내지 않고 chain도 그대로 두므로 chain은 계속 DOM과 일치한다. **단, LIS 판단
자체의 정당성은 검증 범위 밖**이다(`lis_indices`는 비검증 코드) — 검증된 것은
"그 표시를 받아들여도 Chain 조작의 사전조건이 지켜진다"까지다.

§2.5 재수집(v7 → **v8**): §9 판정 전부 동일(CI 폭만 미세 변동).

## 남은 것 (숨기지 않음)

1. **`plan_writes`의 사후조건은 "수렴"이 아니다 — 시도했고, 도구 한계로 접었다
   (2026-07-28)**. 현재 사후조건은 방출된 op의 인덱스 유효성과 spawn 플래그
   일치까지다. "적용하면 target 순서가 된다"(정리 (2))는 `flush_order.rs`에서
   **모델에 대해** 증명돼 있다.

   **시도한 것**: `place_before`/`apply_ops`를 이 크레이트에 정의하고,
   "chain의 표기 = 방출한 op를 적용한 결과"라는 **결선 불변식**을 세웠다
   (여기까지는 성립 — 26 verified). 그 위에 접미사 불변식을 얹어 수렴을
   사후조건으로 끌어오려 했으나 실패했다.

   **실패 원인은 증명이 아니라 Verus/Z3의 한계**다. `plan_writes`는 이미 불변식
   9개를 지고 있고(LIS 변형 때문에 셋은 조건부), 접미사 추론이 더해지자:

   | rlimit | 결과 |
   |---|---|
   | 4000 | budget 초과 |
   | 6000 | Verus 크래시 (`expected rlimit-count in smt statistics`) |
   | 20000 | 동일 크래시 |

   작동하는 구간이 없었다. 추론을 독립 `proof fn` 4개로 분리하는 구조 변경도
   넘지 못했다. 부분 패치된 파일을 남기는 대신 known-good 상태로 되돌렸다.

   **2026-07-28 2차 시도 — 네 가지 구조 개선을 모두 적용했으나 동일한 벽**.
   다음 착수자가 같은 길을 다시 걷지 않도록 확인된 것을 남긴다:

   | 시도한 구조 개선 | 효과 |
   |---|---|
   | `keep` 없는 전용 함수로 분리(조건부 불변식 3개 제거) | **효과 있음** — rlimit 벽이 일시적으로 사라지고 구체적 단언 실패로 진행됐다 |
   | `i` 감소를 루프 **끝**으로 이동 | **필수** — 앞에서 감소하면 불변식의 `i`(옛 값)와 본문의 `i`(새 값)를 프루버가 잇지 못한다 |
   | 분기 합침(`if fresh \|\| !adj`) | 검증 경로 4 → 3 |
   | 모든 `forall` 추론을 `proof fn`으로 추출 | 루프 본문엔 호출문만 남겼으나 **여전히 크래시** |

   최종 결과: rlimit 4000/6000/20000 전부에서 Verus가
   `expected rlimit-count in smt statistics`로 **크래시**(증명 실패가 아니라
   도구가 Z3 통계를 파싱하지 못함). 작동하는 구간이 없다.

   **함께 확인된 Verus 규칙(문서에 없어 두 번 걸렸다)**: `while` 루프는
   **불변식만으로 모듈러 검증**된다 — 함수 `requires`에 있는 사실이라도, 그것이
   불변 입력에 대한 것이라도, 루프 안에서 자동으로 살아 있지 않다. 전부 불변식에
   다시 적어야 한다. 그래서 불변식 수를 줄이려는 시도 자체가 한계가 있다
   (필수 불변식만 세어도 14개).

   **다음에 시도할 가치가 있는 것**: (a) 루프를 둘로 분할해 `None` 분기(마지막
   원소)를 루프 밖으로 빼기 — 검증 경로가 하나 더 줄어든다, (b) `chain`을
   `Chain` 구조체가 아니라 `Seq<u64>` 고스트로만 다루는 별도 순수 함수로
   수렴을 증명한 뒤 실행 코드와 연결(즉 `flush_order.rs`의 `suffix_invariant`를
   그대로 쓰고 실행 루프는 그것과의 대응만 유지), (c) Verus 버전 업그레이드 —
   크래시가 도구 버그일 가능성이 있다.

   **도중에 확정한 설계 판단 하나**: 수렴은 `keep`이 전부 false일 때만 주장할
   수 있다. LIS 변형이 `keep`으로 "이건 이미 제자리"라고 표시하면 그 원소는
   인접하지 않아도 스킵되는데, 그 판단의 정당성은 `lis_indices`(비검증)에
   있다. 따라서 `place_in_order_lis`의 수렴은 §5 트랙 N property test로
   확보하는 것이 맞고, Verus로 닫으려면 LIS 최적성부터 증명해야 한다.
2. **`lis_indices`는 비검증**: LIS 계산 자체(그리고 그 결과를 `keep`으로 쓰는
   것의 정당성)는 검증 대상이 아니다.
3. **`emit_writes`와 `apply_k`의 결선**: `apply_k`는 `emit_writes`의 사후조건을
   목표 id열만으로 다시 쓴 것이다. 둘이 같은 시퀀스를 가리킨다는 것을 별도
   정리로 잇지 않았다.

### (해소됨) `std::HashMap` ↔ `Map` 결선

이전 라운드의 남은 항목이었다. `vstd::hash_map::HashMapWithView`가 std
`HashMap`을 감싸 `Map` 뷰와 완전한 계약을 제공하므로, 그 위에서 `Chain`을
구현함으로써 해소됐다. (참고용 이전 서술: : 위 증명은 `Chain`의
   두 맵을 Verus의 `Map`으로 모델링한다. 실제 코드는 `std::collections::HashMap`
   에 `bevy_ecs::entity::Entity` 키를 쓴다. 이 간극은 두 조각이다 —
   (a) *`std::HashMap`이 부분함수로서 `Map`처럼 행동하는가*: `vstd`가
   `HashMapWithView`로 이미 명세를 제공하며, 이 명제 자체는 진지한 의심의
   대상이 아니다(표준 라이브러리 계약). (b) *`Entity` 키가 `usize`처럼
   구별되는가*: `Entity`는 불투명 id이고 `Eq`/`Hash`가 동일성과 일치한다.
   — 이 서술은 위 결선으로 해소됐다.)

**과장하지 말 것**: 현재 상태는 "순서 결정 루프와 그것이 쓰는 `Chain`이 검증된
실행 코드이고, 그 안에서 모든 사전조건이 증명으로 해소된다"이지, "그 루프가
target 순서로 수렴함이 실제 코드에 대해 증명됐다"가 아니다 — 수렴은 모델
수준(위 남은 것 1번).
