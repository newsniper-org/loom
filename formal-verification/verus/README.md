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
# 모델(정리들)
verus formal-verification/verus/flush_order.rs

# 실행 코드(검증된 코드 = 배포되는 코드)
verus --crate-type=lib crates/loom-flush-core/src/lib.rs
```

**`cargo verus verify`를 쓰지 않는 이유(2026-08-01)**: 업스트림 최신 Verus는
crates.io의 어떤 vstd 릴리스보다 앞서 있어(버전 문자열이 같아도 내용이 다름)
바이너리가 `unexpected verus diagnostic item`으로 거부한다. 설치본을 path로
가리키는 것도 막힌다 — 설치 디렉터리에 워크스페이스 루트가 없어
`builtin_macros` 등의 `{ workspace = true }` 상속이 풀리지 않고 `/opt/verus`는
쓰기 불가다. `verus`를 직접 쓰면 **번들된 vstd**를 자동으로 쓰므로 바이너리와
짝이 반드시 맞는다(이 크레이트는 의존이 vstd 하나뿐이라 가능하다).

빌드 경로(`cargo build`)는 crates.io vstd를 쓴다 — ghost 소거 후 실체로 남는
것은 `HashMapWithView`(std HashMap 얇은 래퍼)뿐이고, API가 어긋나면 빌드가
시끄럽게 실패하므로 조용한 불일치는 생기지 않는다.

현재 결과(2026-10-17 재실측, 같은 명령): **28 verified, 0 errors**(모델 파일
`flush_order.rs`) + **90 verified, 0 errors**(`crates/loom-flush-core`,
`verus --crate-type=lib`, 경고 0 — 출력은 자동 트리거 선택 `note`뿐). Verus
업스트림 설치본 `/usr/bin/verus`. 실행 시간은 이번엔 기록하지 않는다(재실측 당시
머신 load average가 ~48이라 2026-08-11의 smt-run 6.4초 / total-time 8.2초와 비교할 수 없다).
`crates/loom-flush-core`(마지막 커밋 9d2ca26, 2026-08-11)와 `flush_order.rs`(5959aa4,
2026-08-02)는 그 뒤 바뀌지 않았다(`git log` 확인). 80→90의 10개는 LIS 건전성 단계에서 들어온 것
(`lis_keep`, 래퍼 `plan_lis_from_selection_auto`, `lower_bound`, `tails_*` 불변식과
보존 보조정리 등) — 맨 아래 "LIS 건전성 — 증명 완료, 사슬이 닫혔다" 절과 그
"기준선". 그 절의 "남는 것"(원시 입력 구조적 사전조건 넷은 `debug_assert!`로만,
LIS 최적성 미증명)도 이 수치와 함께 읽을 것.

> **2026-10-17 정정**: 이 줄은 "현재 결과(2026-08-11 실측): … **80** verified,
> 13.5초 … 마지막 9개는 경계 함수 `plan_inputs`와 두 래퍼, 그리고 번역된 P와 그
> 대조 2건"이었다. 80은 같은 날(2026-08-11) 앞 단계("경계를 옮겼다")의 수치이고,
> 같은 날 4단계(LIS 건전성)가 90으로 올린 뒤 이 줄만 갱신되지 않았다 — 아래
> 경고문이 말하는 바로 그 표류가 또 일어났다. 같은 파일 "기준선" 절은 처음부터
> 90이었다.

**이 줄은 계속 밀린다.** 2026-08-08에 `45`로 적힌 뒤 그날 안에 `71`이 됐는데도
여기만 남아 있었다(바로 아래 "주의"가 말하는 것과 같은 종류의 표류다). 고칠
때는 아래 명령을 실제로 돌린 원문을 옮길 것.

**주의**: 이 줄에 한동안 `25 + 22`가 적혀 있었고 `cargo verus verify` 기준이라고
쓰여 있었다 — 둘 다 틀렸다. 그 명령은 위 31행이 설명한 vstd 불일치로 **실행조차
되지 않는다**(업스트림 마이그레이션 때 앞 절만 갱신되고 이 줄이 남았다).

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

`cargo verus verify` → **12 verified, 0 errors**(2026-07-31 당시, 업스트림
마이그레이션 이전 기준). 같은 소스가 일반 `cargo build`로도 컴파일된다
(ghost 소거) — **wasm32 포함**.

> **2026-08-08 정정**: 이 명령은 2026-08-02 마이그레이션 이후 vstd 불일치로
> **실행되지 않는다**(위 "실행" 절 참고). 같은 크레이트의 현재 수치는
> `verus --crate-type=lib crates/loom-flush-core/src/lib.rs` 기준이다.

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
자체의 정당성은 검증 범위 밖**이었다(`lis_indices`는 비검증 코드) — 검증된 것은
"그 표시를 받아들여도 Chain 조작의 사전조건이 지켜진다"까지다.

§2.5 재수집(v7 → **v8**): §9 판정 전부 동일(CI 폭만 미세 변동).

## 수렴 사후조건 — 세 번째 시도에서 통과 (2026-08-01)

`crates/loom-flush-core::plan_writes_converging`가 **수렴을 사후조건으로** 갖는다:

```rust
ensures ({
    let d = apply_ops(current@, ops@, ops@.len() as int);
    &&& d.len() >= target_ids@.len()
    &&& d.subrange(d.len() - target_ids@.len(), d.len() as int) =~= target_ids@
})
```

"방출한 op를 순서대로 적용하면 DOM의 뒤쪽 |target|개가 목표 순서와 정확히
일치한다"가 **실제 실행 코드에 대해** 증명됐다(RQ5 #4 정리 (2)). `list.rs`의
`place_in_order`가 이 함수를 호출한다. **44 verified, 0 errors**
(`verus --crate-type=lib crates/loom-flush-core/src/lib.rs`).

### 두 번의 실패가 왜 실패했는지 — `--profile`이 알려준 것

1·2차 시도(2026-07-28)는 "불변식이 많아서"라고 **추정**하고 구조를 바꾸는 데
매달렸다: 조건부 불변식 제거(`keep` 없는 전용 함수 분리), 분기 합침,
모든 `forall` 추론을 `proof fn`으로 추출, 루프 구조 교정. 이 조치들은 전부
**필요했지만 원인이 아니었다**. `--profile`을 돌리자 한 줄로 드러났다:

```
88% of all instantiations (2,286,314 / 2,584,749)
 --> vstd/seq_lib.rs:520  →  Seq::no_duplicates()
```

`no_duplicates`는 `forall|i, j| ... ==> s[i] != s[j]` — 인덱스 **쌍** 전체에
대한 2변수 양화자다. 루프 문맥에 이 사실이 셋(`current`/`target_ids`/
`chain.denotes()`) 떠 있으니 Z3가 O(n²) 패턴을 폭발시켰다.

### 해결: 불투명 래퍼

```rust
#[verifier::opaque]
pub open spec fn seq_nodup(s: Seq<u64>) -> bool { s.no_duplicates() }
```

루프는 "중복이 없다"는 **명제만** 들고 다니고 양화자는 보지 않는다. 펼치는
것(`reveal`)은 그것이 필요한 보조정리 **안쪽**뿐이며, 거기 검증 조건은 작아서
감당된다.

**한 번 헛짚은 지점**: 처음엔 `requires`만 래퍼로 바꿨는데 프로파일 수치가
줄기는커녕 **89%로 늘었다**. 원인은 루프가 호출하는 보조정리의 **`ensures`**가
raw `no_duplicates`를 호출할 때마다 문맥에 되돌려 주입한 것이었다. 불투명화는
**계약의 양방향을 모두** 봐야 한다.

### 함께 확인된 Verus 규칙 둘 (문서에 없어 각각 두 번씩 걸렸다)

- `while` 루프는 **불변식만으로 모듈러 검증**된다 — 함수 `requires`의 사실이라도,
  불변 입력에 대한 것이라도, 루프 안에서 자동으로 살아 있지 않다.
- 인덱스 감소를 루프 **앞**에서 하면 불변식의 `i`(옛 값)와 본문의 `i`(새 값)를
  프루버가 잇지 못한다. 읽기 전용 인덱스를 두고 감소는 **끝**에서.

### 진단 순서에 대한 교훈

**자원 벽에 부딪히면 추정 전에 프로파일.** 증상("루프가 무겁다")에서
원인("불변식이 많다")을 추정해 두 라운드를 썼는데, 프로파일 한 번이 진짜 원인을
지목했다. 참고로 크래시(`expected rlimit-count in smt statistics`)는 도구
버그가 아니었다 — 낮은 rlimit에서는 깨끗한 "초과"가 나오므로 Z3가 오래·크게
돌다 비정상 출력을 낸 증상이었고, 부담을 줄이자 사라졌다.

## `emit_writes` ↔ `apply_k` 결선 — 완료 (2026-08-02)

정리 (2)까지의 서술에는 사람이 눈으로 이은 대응이 하나 남아 있었다. `apply_k`는
`emit_writes`의 사후조건을 **손으로 목표 id열만으로 다시 쓴 것**이고, 그 재작성이
옳다는 정리가 없었다. 만약 재작성이 틀렸다면 정리 (2)는 실제 방출 시퀀스와
무관한 명제가 된다.

`flush_order.rs`에 셋을 추가해 닫았다(**28 verified, 0 errors**, 첫 시도 통과):

- `apply_writes(dom, writes, k)` — 각 write가 **실제로 들고 있는**
  `id()`/`before()`를 그대로 적용한다(`LoomApp::tick`의 적용 루프에 대응).
- `lemma_emit_matches_apply_k` — `emit_writes`의 사후조건을 만족하는 시퀀스에
  대해 `apply_writes(dom, writes, k) =~= apply_k(dom, ids_of(target), k)`.
- `emit_writes_verified` — **exec 함수**. `emit_writes`를 호출해 얻은 바로 그
  `Vec<Write>`에 대해 정리 (1)과 (2)를 한 문장으로 낸다:

```rust
ensures
    writes@.len() == target@.len(),
    anchors_always_resolvable(dom, writes@),          // 정리 (1)
    ({ let r = apply_writes(dom, writes@, writes@.len() as int);
       &&& r.len() >= target@.len()
       &&& r.subrange(r.len() - target@.len(), r.len() as int) =~= ids_of(target@) }),
```

이제 "사후조건을 만족하는 **어떤** 시퀀스"가 아니라 **그 함수가 실제로 낸
시퀀스**에 대해 서술된다.

### 음성 대조 3건 (전부 거부 확인)

| # | 고의 결함 | 결과 |
|---|---|---|
| NC-1 | `apply_writes`가 write를 **반대 순서**로 적용 | `postcondition not satisfied` |
| NC-2 | `ids_of`를 target **역순**으로 정의 | `assertion failed` ×2 |
| NC-3 | 결선 정리에서 `before()` 사후조건 전제를 제거 | `postcondition not satisfied` |

### 남은 비대칭 — 정직하게

`emit_writes`는 `already_adjacent` 스킵을 하지 **않는** 상위집합이다(모델 파일이
의도적으로 그렇게 뒀고, `skip_optimization_is_safe`가 `apply_k_opt =~= apply_k`로
그 차이를 메운다). 스킵을 실제로 하는 exec 함수는 모델 파일에 없다 — **그건
`crates/loom-flush-core::plan_writes_converging`이고, 거기서는 스킵 분기를 포함한
채로 수렴이 직접 증명돼 있다**(모델이 아니라 배포되는 코드). 즉 이 비대칭은
간극이 아니라 역할 분담이다.

한 가지는 여전히 모델에만 있다: **정리 (1)("두 폴백 경로가 한 번도 실행되지
않는다")**. `loom-flush-core`의 `place_before`는 폴백 가지를 그대로 모델링하고도
수렴을 증명하므로 "폴백이 순서를 깨뜨리지 않는다"는 결론은 코드 쪽에도 있지만,
"폴백이 아예 도달 불가"라는 더 강한 명제는 `flush_order.rs`의 것이다.

## LIS 변형의 `keep` — 사전조건 P를 확정 (2026-08-02)

`place_in_order_lis`는 `plan_writes`에 `keep`을 넘기고, `plan_writes`는
`keep[i] || <인접성 검사>`로 **단락**한다. 즉 `keep`이 틀리면 인접성 검사가
아예 돌지 않고 그 원소는 제자리에 남는다. 지금까지 이 지점은 "LIS 판단 자체의
정당성은 검증 범위 밖(호출부 책임)"이라고만 적혀 있었다 — 호출부가 **무엇을**
지켜야 하는지는 적혀 있지 않았다.

### `keep`은 지워도 되는 것이 아니다 (먼저 확인한 것)

`chain.next_is(e, a)` 인접성 검사는 이미 O(1)이므로, `keep` 단락이 실제로
방출량을 줄이는지부터 확인했다. 줄인다 — 크게:

| 워크로드 (n=1000) | `keep` 있음 | 인접성 검사만 |
|---|---|---|
| swap ends | **2** | 999 |
| shuffle | 944 | 992 |
| 10% 섞기 | 82 | 97 |
| reverse / head→tail | 999 / 1 | 999 / 1 |

n≤7 전 순열 중 3,320건에서 이동 수가 달랐다. LIS 축은 인접성 검사로 붕괴되지
않는 **진짜 변형 축**이고, 따라서 `keep` 경로는 없앨 수 없다.

### 사전조건 P — 필요충분

> **P**: `keep`으로 표시된 원소들의 `current` 내 상대 순서가 target 순서와 같다.

전수 탐색으로 P가 정확히 필요충분임을 확인했다. 스크립트를 저장소에 두었다 —
`formal-verification/verus/lis-keep-exhaustive.py`(모의 실행이 `apply_ops`/
`place_before`와 같은 의미가 되도록 폴백 가지까지 옮겼다):

- P를 만족하는 keep 조합 **28,215건**(n≤6 전 순열 × 모든 keep 부분집합
  15,125건 + fresh 혼합 n≤4 13,090건) → 순서 위반 **0건**
- P를 **위반**하는 조합 **2,484건** → **100%** 순서가 깨짐
- `lis_indices`가 만든 keep이 P를 위반한 경우(n≤6 전 순열): **0건**

이보다 약한 조건으로는 안 되고, 이보다 강할 필요도 없다.

**그런데 이걸 `loom-flush-core`의 명세로 넣지 못했다** — 시도했고, 아래
"검증이 rlimit 경계에 붙어 있다"에 적은 이유로 되돌렸다. 지금 P는
`place_in_order_lis`의 주석과 이 문서, 그리고 디버그 빌드 검사에만 있다.

### `lis_indices`에 필요한 것은 건전성뿐 — 최적성이 아니다

P는 "표시된 것들이 증가 부분수열을 이룬다"와 같다. 즉 `lis_indices`가
**증가** 부분수열을 돌려주기만 하면 되고, 그것이 **가장 긴지**는 무관하다.
최적이 아니면 이동이 더 나올 뿐 순서는 깨지지 않는다 — 성능 성질이지 정합성
성질이 아니다. 이 구분이 남은 증명 부담을 크게 줄인다.

그 사이 보호로 둘을 넣었다:
- `place_in_order_lis`에 **디버그 빌드 P 검사**(`debug_assert!`) — 정확히 그
  사전조건을 O(n)에 검사한다.
- `lis_result_is_always_an_increasing_subsequence` — n=1..40 × 50 순열 무작위
  건전성 속성 검사(최적성은 검사하지 않음, 그 이유를 테스트에 적어 뒀다).

### 검증이 rlimit 경계에 붙어 있다 — 이번에 드러난 더 큰 문제

P를 `kept_order_preserved`라는 `spec fn`으로 쓰고 `plan_writes`의 `requires`에
넣었더니 **내가 건드리지도 않은 `plan_writes_converging`의 루프**가 깨졌다
(`expected rlimit-count in smt statistics` — Z3가 비정상 통계를 내며 크래시).
원인을 추정하지 않고 이분했다:

| 실행 | 결과 |
|---|---|
| HEAD, 단독 | **44 verified, 0 errors** |
| HEAD + P(불투명) + `requires`, 단독 | 크래시 |
| 위에서 `requires` 한 줄만 제거, 단독 | 크래시 |
| 위치 계산 `spec fn`을 불투명 술어 안으로 인라인, 단독 | 크래시 |
| `#[verifier::spinoff_prover]` 추가, 단독 | 크래시 |
| **HEAD, 다른 verus와 동시 실행** | **크래시** |

마지막 줄이 결정적이다. **HEAD 자체가 부하만 걸려도 깨진다.** 즉 이건 "내 추가가
증명을 깼다"가 아니라 **`plan_writes_converging`의 검증이 rlimit 경계에 바짝
붙어 있다**는 것이고, 모듈에 `spec fn` 하나를 더하든 머신이 바빠지든 어느 쪽으로도
넘어간다. `--profile`은 통계를 읽기 전에 죽으므로 이 크래시에는 쓸 수 없다.

**따라서 "44 verified, 0 errors"는 재현 가능한 상태가 아니다.** 한가한 머신에서
단독으로 돌리면 나오고, 아니면 크래시한다. 이 사실을 숨기지 않는다 — B1 성공을
기록할 때 이 조건을 몰랐고, 이번에 P를 넣으려다 드러났다.

> **2026-08-08 정정**: 이 조건은 사라졌다. 위 표의 마지막 줄("다른 verus와
> 동시 실행 → 크래시")과 이 문단은 **그 시점의 관측**으로 읽을 것. 지금은 같은
> 명령 2개를 동시에 돌려도 두 인스턴스 모두 통과한다(아래 "rlimit 여유 확보"
> 절, 그리고 `Chain::well_formed` 불투명화 이후 모듈 전체가 3.4초).

**남은 최우선 항목**은 P의 증명이 아니라 **이 여유(headroom)를 만드는 것**이다.
그게 되기 전에는 어떤 추가 명세도 같은 벽에 부딪힌다.

> **2026-08-08 정정**: 이 항목은 수행됐고, 그 다음 단계였던 **P를 `requires`로
> 올리는 것까지 끝났다**(아래 두 절). 이 문장을 현재 지침으로 읽지 말 것. (B1에서 `no_duplicates`를
불투명화해 88%를 걷어냈듯, 다음으로 큰 인스턴스화 원인을 찾아야 하는데 —
크래시 상태에서는 프로파일이 안 나오므로 부담을 먼저 조금 덜어 "깨끗한 rlimit
초과"를 만든 뒤 프로파일하는 순서가 필요하다.)

### 왜 "P ⟹ 수렴"을 지금 증명하지 않는가 (범위 결정)

> **2026-08-09 해소**: 이 절은 그때의 범위 결정 기록으로 남긴다. 수렴은
> 증명됐다 — 맨 아래 "LIS 경로의 수렴 — 증명 완료" 절. 다만 **결론이
> 그대로는 아니다**: 아래가 "필요한 불변식"으로 적은 J1/J2 중 J2는 필요
> 없었고(사전조건 Q가 대신한다), 정작 **목표 명제 자체가 P만으로는
> 거짓**이었다. 아래 J1/J2 서술은 그 사실을 모르고 쓴 것이다.

`plan_writes_converging`의 증명은 **접미사 불변식**("k개 처리 후 DOM의 뒤쪽
k개 = target의 뒤쪽 k개")에 얹혀 있다. LIS 경로에서는 이 불변식이 **중간에
깨진다** — 확인한 예: `current=[A,B,C]`, `target=[A,C,B]`, LIS={A,B}. i=2에서
B를 스킵하는 순간 뒤쪽 1개는 `[C]`지 `[B]`가 아니다. 그런데도 최종 결과는
맞다: 사이에 낀 원소는 전부 `target[0..i-1]`에 속하고, 나중 단계에서 **왼쪽으로**
치워지기 때문이다.

따라서 필요한 불변식은 접미사가 아니라 **부분수열** 형태다:

> J(i): (J1) DOM에서 `target[i..]`만 뽑으면 `target[i..]` 그대로다.
>        (J2) `target[i]` 뒤에 있는 모든 원소는 `target[i..]` 또는
>             `target[0..i-1]`에 속한다.
>
> i=0에서 J1+J2와 무중복이 합쳐지면 "뒤쪽 n개 = target"이 나온다.

이건 기존 `subrange` 기계로는 안 되고 `Seq::filter`(또는 증가 인덱스 사상)
계열을 새로 들여와야 한다. 게다가 P 자체가 2변수 양화자라 — B1에서 인스턴스화의
88%를 먹었던 바로 그 형태다 — SMT 부담이 처음부터 높다. 그리고 위에서 확인했듯
**지금은 P를 문맥에 올려놓는 것조차 안 된다**. 최소 B1급(3라운드) 작업이고
그 전에 여유부터 만들어야 하므로, **지금은 하지 않는다**.

대신 남긴 것: 경계가 "LIS 판단은 검증 밖"이라는 모호한 말에서 **P라는 정확한
명제**로 바뀌었고, P의 필요충분성은 전수 탐색으로 뒷받침되며, 필요한 불변식의
모양(J1/J2)까지 적혀 있어 재개 가능하다. **증명됐다고 말하지 않는다.**

## 남은 것 (숨기지 않음)

1. ~~`plan_writes`의 사후조건이 "수렴"이 아니다~~ → **2026-08-01 해결**
   (위 "수렴 사후조건 — 세 번째 시도에서 통과" 절). 1·2차 시도의 실패 경위와
   거기서 확인된 Verus 규칙들은 그 절에 옮겨 두었다.

2. **검증이 rlimit 경계에 붙어 있다 (최우선)** — HEAD도 부하가 걸리면
   크래시한다. "44 verified"는 한가한 머신 단독 실행에서만 재현된다.
   상세와 이분 결과는 위 "검증이 rlimit 경계에 붙어 있다" 절.
   > **2026-10-17 정정**: 부하 시 크래시(원인 하나가 OOM)와 2026-08-08 당시의
   > rlimit 부족은 2026-08-08에 해소됐다(아래 "rlimit 여유 확보"와 "P를
   > `requires`로 올렸다" 절 — 동시 2개 실행에서도 둘 다 통과). 위 1·3·4번처럼
   > 해결 표시 없이 남아 있었다. **다만 rlimit 여유 문제 자체가 닫힌 것은 아니다**:
   > 2026-08-11 Q 승격에서 임계가 3 → 7로 올라 여유가 1.4배로 줄었고("경계를
   > 옮겼다" 절), 같은 날 LIS 건전성 단계 뒤 7 → 5(2.0배)로 회복됐지만 "이 개선은
   > 재현 보장이 없다"는 경고가 "rlimit 여유가 늘었다" 절에 있다. 회귀 판정
   > 기준선은 맨 아래 "기준선" 절의 모듈 총 rlimit. 당시 기록으로 남긴다.
3. ~~**`plan_writes`(LIS 변형)의 수렴은 미증명**~~ → **2026-08-09 해결**
   (맨 아래 "LIS 경로의 수렴 — 증명 완료" 절). 사전조건 **Q를 추가**해야
   했다 — P만으로는 목표 명제가 거짓이었다. 남은 미증명은 `lis_indices`가
   P를 세운다는 것(**LIS 건전성**)뿐이고, 필요한 것이 **건전성뿐이고
   최적성이 아님**은 확인돼 있다.
   > **2026-10-17 정정**: LIS 건전성도 2026-08-11에 증명됐다(맨 아래 "LIS
   > 건전성 — 증명 완료, 사슬이 닫혔다" 절, 90 verified). 지금 남은 것은 원시
   > 입력 구조적 사전조건 넷의 `debug_assert!` 의존과 LIS 최적성(의도적
   > 미증명)이다 — 같은 절 "남는 것" 참고.
4. ~~`emit_writes`와 `apply_k`의 결선~~ → **2026-08-02 해결**
   (위 "`emit_writes` ↔ `apply_k` 결선" 절).

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

> **2026-08-08 정정**: 이 서술은 2026-08-01 이전 상태다. **비-LIS 경로**는
> `plan_writes_converging`이 수렴을 **실행 코드의 사후조건으로** 갖는다(위
> "수렴 사후조건" 절). 지금 미증명으로 남은 것은 **LIS 변형(`plan_writes`)의
> 수렴**이고, 참조는 "남은 것" 1번이 아니라 그 항목이다. 과장 방지 취지 자체는
> 유효하다 — 다만 대상이 좁아졌다.
>
> **2026-10-17 정정**: 위 2026-08-08 정정도 이제 낡았다. LIS 변형의 수렴은
> 2026-08-09에("LIS 경로의 수렴 — 증명 완료" 절, 70 verified), LIS 건전성은
> 2026-08-11에("LIS 건전성 — 증명 완료, 사슬이 닫혔다" 절, 90 verified)
> 증명됐다. 과장 방지 취지는 여전히 유효하고 대상은 더 좁아졌다 — 남은 것은
> 구조적 사전조건 넷의 `debug_assert!` 의존과 LIS 최적성(의도적 미증명)이다.

---

## rlimit 여유 확보 — `plan_writes`의 부담을 114배 줄였다 (2026-08-08)

"검증이 rlimit 경계에 붙어 있다"가 남긴 **최우선 항목**("이 여유를 만드는 것")을
해소했다. 결론부터: `plan_writes`(LIS 변형)의 루프 불변식이 raw
`no_duplicates`를 들고 있었고, 그것을 `plan_writes_converging`이 2026-08-01에
받은 것과 **같은 처방**(불투명 래퍼 + 필요한 `by` 블록 안에서만 `reveal`)으로
바꿨다.

### 측정 (같은 방법으로 전후 비교)

| `plan_writes` 단독 | 수리 전 | 수리 후 | 배 |
|--------------------|---------|---------|-----|
| SMT 시간 | 111,835 ms | **983 ms** | 114× |
| rlimit | 334,368,439 | **9,490,029** | 35× |
| 인스턴스화 | 8,535,321 | **91,350** | 93× |

그리고 README가 기록한 크래시 조건이 사라졌다:

> **HEAD, 다른 verus와 동시 실행 → 크래시**

같은 조건에서 지금은 **두 인스턴스 모두 44 verified, 0 errors**다. "44 verified는
재현 가능한 상태가 아니다"라고 적어 둔 것이 이제 성립하지 않는다.

### 두 번 헛짚었다 — 방법의 교훈

**`--profile-all --verify-root`는 모듈 전체 통계이고, 그것으로는 귀속이 안 된다.**
프로파일이 `vstd/seq_lib.rs:520`(= `no_duplicates`)을 82%로 지목했을 때, 나는
**어느 함수가 그것을 인스턴스화하는지**를 모른 채 위치를 추정했다.

1. **가설 ①(맞았으나 측정이 틀림)**: `plan_writes` 루프 불변식. 고친 뒤
   **모듈 전체** 인스턴스화가 3.76M → 3.87M로 *늘어서* "효과 없음"으로 읽었다.
   나중에 `--verify-function plan_writes`로 그 함수만 재니 8.54M → 91K였다.
   **모듈 총계로 함수 단위 개선을 판정하면 안 된다.**
2. **가설 ②(반증)**: `chain_denotes`의 `no_duplicates`가 `well_formed()`를 통해
   두 루프 불변식에 들어오는 것이 원인. 불투명화하니 chain 보조정리 6곳이
   `Resource limit (rlimit) exceeded`(**깨끗한 초과** — 크래시가 아니다)를 냈고,
   그 6곳에 `reveal`을 넣자 이번엔 `plan_writes_converging`이 크래시했다.
   **단독 검증에서도** 크래시하므로 모듈 압력이 아니라 그 함수가 더 어려워진
   것이다. 되돌렸다.
3. **부수 확인**: 인라인 증명을 `pub proof fn`으로 빼는 안은 그 `ensures`가
   모듈 문맥에 새 트리거를 더해 **건드리지도 않은** 함수를 크래시시켰다.
   공리를 늘리지 않는 `assert ... by { reveal(...) }` 쪽이 안전하다.

교훈을 한 줄로: **"추정 전에 프로파일"의 다음 줄은 "함수 단위로 프로파일"이다.**
모듈 총계는 어느 양화자인지는 알려주지만 어디서 터지는지는 알려주지 않는다.

### 남은 것

`plan_writes_converging`이 이제 가장 무겁다(단독 264초 / rlimit 3.21B). 그 루프
불변식은 이미 `seq_nodup`을 쓰지만 `chain.well_formed()`가 raw
`no_duplicates`를 다시 들여온다 — 가설 ②가 겨냥한 지점이고, 단순 불투명화로는
안 됐다. 다음 시도는 **그 함수만 프로파일해** 무엇이 몇 백만 번씩 터지는지 먼저
확정하는 것이다(이번에 `plan_writes`에서 그렇게 해서 풀렸다).

> **2026-08-08 정정 둘**: (1) 여기 처음에 "812만 번"이라고 적었는데 **출처가
> 없는 수치**였다 — 측정한 것은 `plan_writes`의 8,535,321건(수리 **전**)이고
> 이 함수의 값이 아니다. 근거 없는 수치를 남기지 않기 위해 지웠다. (2) 그리고
> 이 "다음 시도"는 실제로 수행됐다 — 아래 두 절 참고. 원인은 `chain_denotes`가
> 아니라 그것을 감싼 **`Chain::well_formed`가 불투명하지 않았던 것**이었다.

그리고 이 여유의 목적이었던 **P(`keep`의 사전조건)를 `plan_writes`의 `requires`로
올리는 시도**는 아직 하지 않았다. 그 함수의 부담이 114배 줄었으므로 이제
가능성이 생겼다는 것까지가 이번 결과다 — 실제로 되는지는 해 봐야 안다.

> **2026-10-17 정정**: 같은 날(2026-08-08) 바로 다음 절 "P를 `requires`로
> 올렸다"에서 실제로 됐다(45 verified). 이 문장은 당시 기록으로 읽을 것.

## P를 `requires`로 올렸다 — 그리고 "크래시"의 원인 하나가 OOM이었다 (2026-08-08, 같은 날 후속)

바로 위 절이 "해 봐야 안다"로 남긴 것을 했다. 결과: **45 verified, 0 errors,
4.7초**. `plan_writes`의 사전조건에 P가 들어 있다.

```rust
#[verifier::opaque]
pub open spec fn kept_order_preserved(
    current: Seq<u64>, target_ids: Seq<u64>, target_fresh: Seq<bool>, keep: Seq<bool>,
) -> bool {
    forall|a: int, b: int| #![trigger keep[a], keep[b]]
        0 <= a < b < target_ids.len()
            && keep[a] && !target_fresh[a] && keep[b] && !target_fresh[b]
            ==> pos(current, target_ids[a]) < pos(current, target_ids[b])
}
```

`pos(s, x) == s.index_of_first(x).unwrap()` — `Option` 언랩을 한 곳에 가둔
정수 위치 함수다. `pos`는 `open`이지만 P가 불투명해서 `reveal` 전에는 `pos(..)`
항 자체가 문맥에 뜨지 않는다(정의 공리가 매칭될 대상이 없다).

### 이 "크래시"는 Verus 버그도 rlimit도 아니었다 — **커널 OOM 킬러**였다

이게 이번의 진짜 발견이다. P를 넣고 전체 모듈을 돌리자 README에 적혀 있던 그
증상이 **글자 그대로 재현**됐다:

```
crates/loom-flush-core/src/lib.rs:1257:5: (#43) expected rlimit-count in smt statistics
thread '<unnamed>' panicked at rust_verify/src/verifier.rs:2178:29: worker thread panicked
```

추정하지 않고 커널 로그를 봤다:

```
Out of memory: Killed process 2604232 (rust_verify)
  total-vm:48259240kB, anon-rss:16937576kB, oom_score_adj:200
```

**`rust_verify`가 16.9GB를 쓰다 OOM 킬러에게 죽은 것이다.** Verus는 죽은
프로세스에서 SMT 통계를 읽으려다 "expected rlimit-count"를 뱉는다. 즉 그
메시지는 원인이 아니라 **증상**이고, 지금까지 "rlimit 경계"라고 부른 것은
정확히는 **메모리 경계**였다. 이것으로 README가 남긴 다른 관찰들이 한 번에
설명된다:

- "HEAD도 **다른 verus와 동시 실행**하면 크래시" — 두 인스턴스가 같은 RAM을 두고
  경쟁하니 당연하다. CPU 경쟁이 아니었다.
- "낮은 rlimit에서는 깨끗한 초과가 나온다" — rlimit에 먼저 걸리면 메모리를 더
  쓰기 전에 멈추니 당연하다.
- 크래시할 때 `--profile`이 안 나온다 — 프로세스가 죽었으니 당연하다.

**교훈**: 도구가 이상한 메시지를 내면 도구 탓하기 전에 `journalctl -k`를 볼 것.
이 프로젝트가 이미 배운 "추정 전에 프로파일"의 형제 조항이다.

> **과일반화하지 말 것 (같은 날, 적대적 검토에서 나온 반례)**: 위 진단은
> **그날 실측된 그 크래시**에 대한 것이고, `expected rlimit-count in smt
> statistics`가 **항상** OOM이라는 뜻은 아니다. 반례가 있다 — 유휴 머신(가용
> 약 20GB 유지, `rust_verify` RSS 371MB, `z3` RSS 최대 약 2.2GB, 커널 OOM
> 메시지 **없음**)에서 다른 정식화 시도가 **같은 메시지로** 두 번 패닉했다.
> 정확한 서술은 "이 메시지는 솔버가 죽었거나 통계를 잘려서 돌려줬다는 뜻이고,
> OOM은 그 원인 중 하나"다. `journalctl -k`를 보라는 조언은 그대로 유효하다 —
> 다만 로그가 비어 있다고 OOM이 아니었다고 결론 내릴 수는 있어도, 그것으로
> 원인이 밝혀진 것은 아니다.

### 진짜 수리 — `Chain::well_formed`를 불투명으로

P를 넣을 자리를 만들려면 `plan_writes_converging`의 메모리를 줄여야 했고, 위
"남은 것"이 지목한 지점이 맞았다. 다만 **가설 ②가 겨눈 곳이 한 칸 얕았다**:

- 그때 시도: `chain_denotes` **안쪽**의 `no_duplicates`만 불투명화 → 나머지 7개
  논리곱(그중 3개가 양화자)은 그대로 루프에 남는다. 실패.
- 이번: **`well_formed()` 자체**를 `#[verifier::opaque]`로. 8개 논리곱 전부가
  루프에서 사라진다.

핵심은 **`closed`가 같은 모듈 안에서는 아무것도 가리지 않는다**는 것이다.
`pub closed spec fn well_formed`은 *모듈 밖*에서만 본문을 숨기는데 이 크레이트는
전부 한 모듈이라, 두 루프의 불변식에 있는 `chain.well_formed()`가 매 반복
`chain_denotes(...)`로 펼쳐지고 있었다. B1에서 걷어낸 그 2변수 양화자가
**`well_formed()`라는 뒷문으로 되돌아와 있었던 것**이다.

펼치는 것은 그것이 실제로 필요한 `Chain`의 메서드 7곳뿐이고
(`reveal(Chain::well_formed)` — 연관 함수에도 쓸 수 있다), 그쪽은 루프가 아니라
직선 코드라 감당된다. `from_slice`는 `new`/`push_back`의 계약만으로 충분해
`reveal`이 필요 없었다 — 캡슐화가 실제로 작동한다는 증거다.

| `verus --crate-type=lib` 전체 모듈 | 수리 전 | 수리 후 |
|---|---|---|
| 벽시계 | 157.9초 | **4.7초** (33×) |
| 결과 | 44 verified (P 없이) | **45 verified** (P 포함) |
| 동시 2개 실행 | 크래시 | **둘 다 45 verified** |

`reveal`을 `proof { }` 블록 **안**이 아니라 함수 본문 최상위에 두어야 한다는
점에 주의(`reveal`은 블록 스코프라, 블록 안에 두면 함수 종료 시점의 사후조건
검사에는 미치지 않는다).

부작용으로 `plan_writes_converging`에 두 줄이 필요해졌다: `place_before`의 첫
단계 `remove_value(e)`가 항등이라는 사실(`!d_mid.contains(e)`)을 전에는
프루버가 펼쳐진 `chain_denotes`에서 곁다리로 주웠는데, 이제는 명시해야 한다.
**수렴 사후조건은 그대로다** — 약화된 정리는 없다.

### 명세가 공허하지 않음을 확인한 음성 대조

`requires`는 **약할수록 통과하기 쉽다**. 항상 참인 술어를 사전조건에 얹는 것은
아무것도 하지 않는 것과 같으므로, P가 실제로 무언가를 *거절*하는지 못박았다 —
`lemma_kept_order_rejects_inversion`: `current = [7, 9]`, `target = [9, 7]`,
둘 다 Retained·`keep` 표시면 P는 **거짓**이다(45번째 항목).

### 그래도 아직 증명되지 **않은** 것 — 숨기지 않음

> **2026-08-09 해소**: 아래 첫 단락은 그날의 상태다. P로부터 수렴이 유도됐고
> (사전조건 Q를 하나 더 얹어서) `plan_writes`의 사후조건은 이제 인덱스
> 유효성뿐이 아니다 — 맨 아래 "LIS 경로의 수렴 — 증명 완료" 절. 둘째 단락
> (`lis_indices`의 건전성)은 **여전히 유효하다**.

**P는 지금 `plan_writes`의 사전조건으로 "적혀" 있을 뿐, 그것으로부터 수렴이
유도되지는 않았다.** `plan_writes`의 사후조건은 여전히 인덱스 유효성뿐이다.
"P ⟹ 수렴"에는 위 "왜 'P ⟹ 수렴'을 지금 증명하지 않는가" 절이 적어 둔
J1/J2(접미사가 아니라 **부분수열** 형태의 불변식)가 그대로 필요하다. 이번에
바뀐 것은 **자원 조건**이다 — 그 절이 "그 전에 여유부터 만들어야 한다"고 적은
전제가 해소됐고(4.7초, 33배 여유), P가 문맥에 올라가 있으므로 J1/J2 시도는 이제
자원이 아니라 증명 설계의 문제다.

`lis_indices`가 P를 세운다는 것(LIS 건전성)도 여전히 미증명이다 — 비검증
`place_in_order_lis`의 디버그 빌드 검사와 무작위 속성 검사가 지키고 있다.

> **2026-10-17 정정**: 2026-08-11에 증명됐다(맨 아래 "LIS 건전성 — 증명 완료,
> 사슬이 닫혔다" 절). P의 `debug_assert!`는 그때 삭제됐다. 당시 기록으로 남긴다.

### 공허성 대조를 양방향으로 (2026-08-08, 채택 직후 보강)

채택 시점에는 **음성 대조만** 있었다(`lemma_kept_order_rejects_inversion` —
역전된 입력에서 P가 거짓). 그것만으로는 **항상 거짓인 술어**와 구별되지 않고,
항상 거짓인 `requires`는 `plan_writes`를 검증 코드에서 호출 불가능하게 만드는
또 다른 종류의 공허다. 적대적 검토가 지적한 그대로라 둘을 더 넣었다:

- `lemma_kept_order_accepts_increasing` — **같은 `current`/`target_ids`** 위에서
  `keep`만 바꾸면 P가 참이 된다. 값이 실제로 갈린다는 것이 요점이다.
- `lemma_kept_order_ignores_fresh` — `!fresh` 조항이 실제로 쌍을 제외하는지.
  이것이 없으면 P의 정의에서 `&& !target_fresh[a] && !target_fresh[b]`를 통째로
  지워도 다른 모든 항목이 통과한다 — 즉 그 조항이 검사되지 않는 채 남는다.

두 대조 모두 **음성 대조로 판별력을 확인**했다:

| 주입한 결함 | 결과 |
|---|---|
| P에서 `!fresh` 조항 제거 | `postcondition not satisfied` (46 verified, 1 error) |
| P의 `<`를 항상 참으로 약화 | `postcondition not satisfied` (46 verified, 1 error) |

즉 P는 항상 참도 항상 거짓도 아니고, 각 조항이 실제로 무언가를 배제한다.

## LIS 경로의 수렴 — 증명 완료 (2026-08-09)

`crates/loom-flush-core::plan_writes`(LIS 변형, `keep` 있음)가 이제
`plan_writes_converging`과 **글자 그대로 같은 모양의 수렴 사후조건**을 갖는다:

```rust
({
    let d = apply_ops(current@, ops@, ops@.len() as int);
    &&& d.len() >= target_ids@.len()
    &&& d.subrange(d.len() - target_ids@.len(), d.len() as int) =~= target_ids@
})
```

전 모듈 **70 verified, 0 errors**, 6.1초(smt-run 4.56초, rlimit 32.7M).
직전 HEAD는 47 verified / 5.0초 / rlimit 26.6M이었으므로 1.2배.

위 "왜 'P ⟹ 수렴'을 지금 증명하지 않는가" 절과 "남은 것" 3번은 **이 절로
대체된다**. 다만 그 절이 예고한 J1/J2 형태 그대로 된 것은 아니다 — 아래.

### 목표 명제는 P만으로는 **거짓**이었다 — 사전조건 Q를 추가했다

J1/J2를 세우기 전에 전수 탐색으로 목표 명제부터 확인했더니 **반례가 있었다**:

```
current = [0, 1]   target_ids = [0]   target_fresh = [false]   keep = [true]
```

P는 성립한다(표시된 원소가 하나뿐이라 비교할 쌍 자체가 없어 공허하게 참).
기존 `requires`도 전부 성립한다. 그런데 `keep[0]`이 인접성 검사를 **단락**하므로
`0`은 그대로 남고 op가 하나도 나오지 않아 `apply_ops`의 결과는 `[0, 1]`이다.
뒤쪽 1개는 `[1]`이지 `[0]`이 아니다. P를 만족하는 530,362개 조합 중
**219,501건에서 수렴이 깨진다**.

원인은 `current`에만 있고 `target`에는 없는 원소(**extras**)다. 기존 `requires`는
"retained target ⊆ current"만 말하고 그 역을 말하지 않았다. 그래서 조항을 하나
**추가**했다(기존 조항의 약화가 아니다):

```rust
// 사전조건 Q
forall|x: u64| #![trigger current@.contains(x)]
    current@.contains(x) ==> target_ids@.contains(x),
```

P ∧ Q인 133,034건 전부 수렴, 위반 0건. Q는 충분하지만 최소는 아니다 — 더 약한
P2("extras는 전부 kept 원소들보다 `current`에서 앞에 있다")로도 반례가 나오지
않았으나(177,827건), P2는 다시 2변수 양화자이고 Q는 1변수라 Q를 택했다.

**왜 이전 전수 탐색이 이걸 못 봤나**: `lis-keep-exhaustive.py`(2026-08-02)는
`cur = list(range(n))`, `tgt = cur의 순열`로 훑었다 — `current`가 **항상**
target의 retained 집합과 정확히 같았다. extras 축은 한 번도 시험되지 않았고,
반례는 정확히 그 축에 산다. "P는 필요충분"이라는 그때 결론은 **그 공간 안에서는
여전히 옳다**. 새 스크립트는 `lis-convergence-exhaustive.py`.

**두 경로의 진짜 비대칭이 여기 있다.** `plan_writes_converging`(비-LIS)은 Q가
필요 없다 — 비인접이면 무조건 옮기므로 외래 원소가 저절로 앞으로 밀려난다.
Q는 **`keep` 단락이 사 오는 비용**이다. 호출부
(`crates/loom-runtime/src/list.rs`의 `d_lis_body`)는 구조적으로 이미 항상
만족하지만(`current = old_order − removed`이므로 정확히 retained 집합) 계약에
적혀 있지 않았을 뿐이고, P와 같은 방식으로 `place_in_order_lis`의
`debug_assert!`에 대응 검사를 넣어 경계를 코드에 박았다.

### 쓴 불변식 — J1/J2가 아니라 **분할점을 함께 든 부분수열형**

예고했던 J1/J2 중 **J2는 필요 없었다**. Q가 있으면 최종 DOM은 target 원소만으로
이루어지므로 "DOM에 target 밖 원소가 있다"는 경우가 아예 사라지고, J2는 그
따름정리가 된다. 남는 것은 J1 하나인데 **J1은 단독으로 귀납적이지 않다**:
keep 스킵 분기에서 "`target[i-1]`이 DOM에서 `target[i..]`보다 앞"을 J1만으로는
못 얻는다. 그걸 주는 것이 P인데, **P는 `current`의 위치를 말하지 DOM의 위치를
말하지 않는다.** 그 간극이 이 증명의 전부다.

메운 방식은 J1을 **분할점 `sp`와 함께** 들고 다니는 것이다(`split_inv`).
`sp`는 DOM 안에서 `target[i]`가 있는 자리이고,

- `sp` **앞**을 아직 처리 안 한 kept 원소로 거르면 `keptseq(i)`,
- `sp` **뒤**를 `target[i..]`로 거르면 `target[i..]` 그대로.

분할점을 명시적으로 옮기는 것이 요점이다. 분할점 없이
`filter(dom, kept ∪ target[i..]) == keptseq ++ target[i..]`만 들고 있으면 매
스텝 `A + B`를 쪼개는 자리를 다시 찾아야 하고, 그러려면 filter 결과의 무중복성이
필요해진다 — 이 파일이 두 번 자원 벽에 부딪힌 바로 그 형태다. `sp`를 함께
옮기면 세 분기 모두 자리가 **주어진다**.

### 설계상 가장 큰 이득 — `keptseq`를 `target`이 아니라 `current` 위에 정의

`keptseq(i)`("아직 처리하지 않은 kept 원소들을 늘어놓은 것")를 **`current`
순서로** 정의했다:

```rust
pub open spec fn keptseq(c, t, fr, kp, i) -> Seq<u64> {
    c.filter(|x: u64| kept_lt(t, fr, kp, i, x))
}
```

target 순서로 정의하면 루프 진입 시 `current.filter(kept) == target.filter(kept)`
를 세워야 하는데, 그건 "순서 보존 선택 ⟹ filter 동일"이라는 별도의 무거운
정리다. `current` 쪽으로 정의하면 **진입 조건이 항진명제**가 되고, P는
keep 스킵 한 걸음(`lemma_keptseq_push`)에서만 소비된다. 이 파일에서 P가
쓰이는 곳은 그 정리 **한 군데뿐**이다.

세 분기는 전부 filter의 한 걸음이다:

| 분기 | DOM | 새 분할점 | 쓰는 것 |
|---|---|---|---|
| keep 스킵 | 불변 | `keptseq(i)`의 마지막 원소가 있는 자리 | P(`lemma_keptseq_push`) + `lemma_filter_split_last` |
| 인접 스킵 | 불변 | `sp - 1` | 인접성이 곧바로 준다 |
| 이동/신규 | `remove_value` 후 `insert` | 삽입한 자리 | `e`가 술어 집합에 없다는 것 |

끝맺음은 `lemma_endgame` 하나다: `i == 0`에서 filter가 항등이 되어(Q가 주는 것)
**DOM == target**이 그냥 나온다.

### `Seq::filter` 기계 — vstd에 있는 것과 없는 것

`filter_distributes_over_add`는 vstd **기본 broadcast**(`group_seq_lib_default`)라
`broadcast use` 없이 뜬다. 그래서 `remove(i)`/`insert(j,e)`를 `A + seq![x] + B`로
쪼개면 filter가 그대로 따라 쪼개진다. 싱글턴 filter는
`reveal_with_fuel(Seq::filter, 2)`로 풀리므로 **`lemma_filter_push`
(= `group_seq_extra`, 기본 broadcast 아님)를 한 번도 들여오지 않았다** — 모듈에
새 broadcast 트리거를 하나도 붙이지 않았다는 뜻이고, 이 파일의 "보조정리의
`ensures`가 건드리지도 않은 함수를 깨뜨린다"는 이력상 그게 중요하다.

직접 쓴 것(전부 vstd에 없다): `lemma_filter_none`, `lemma_filter_agree`,
`lemma_filter_all`, `lemma_filter_singleton`, `lemma_filter_remove_nonmatching`,
`lemma_filter_split_last`. 각 5~25줄이다. 이 중
**`lemma_filter_split_last`가 keep 스킵의 심장**이고, filter가 `drop_last`
재귀라 오른쪽에서 왼쪽으로 가는 그 귀납과 정확히 맞물린다.

`split_inv`는 `#[verifier::opaque]`다 — 본문에 filter 항이 둘, 그 술어 안에
존재 양화가 둘 들어간다. `seq_nodup`/`Chain::well_formed`와 같은 처방으로,
루프는 명제만 들고 다니고 펼치는 것은 단계 보조정리 안뿐이다.

### 먼저 해야 했던 것 — `chain_insert_before_denotes_insert`의 rlimit

`chain_insert_before_denotes_insert`는 **기본 rlimit 경계에 붙어 있어서**,
모듈에 새 `pub proof fn` 하나만 추가해도 재현성 있게 넘어갔다. 작업 시작 전에
`#[verifier::rlimit(60)]`을 붙이고 baseline(47 verified)을 다시 잡았다. 자기
증명은 그대로이고 예산만 올린 것이며 이 파일은 이미
`rlimit(400)/(2000)/(4000)`을 쓰고 있다.

### 음성 대조 — 사후조건이 실제로 검사되는지

첫 시도에 통과했으므로 대조가 특히 필요했다. 조항을 하나씩 빼고 확인했다
(사전조건과 루프 불변식 양쪽에서 빼야 실패 지점이 사후조건 쪽으로 내려온다):

| 뺀 것 | 결과 |
|---|---|
| Q | `assertion failed` — 끝맺음의 "DOM의 모든 원소가 target 안" (69 verified, 1 error) |
| P | `precondition not satisfied` — `lemma_step_keep_skip` 호출 (69 verified, 1 error) |

즉 실패 지점이 전수 탐색의 반례가 사는 곳(Q)과 P를 소비하는 유일한 지점(P)에
정확히 대응한다.

**양성 대조도 넣었다** — `lemma_plan_writes_requires_satisfiable`. 사전조건이
늘어날 때 가장 위험한 실패 양식은 "조항들이 서로 모순이라 함수가 공허하게
검증되는 것"이다. P·Q·fresh/retained 조항을 **동시에** 만족하는 구체적 증인
(`current=[7,9] target=[7,9] keep=[true,true]`)을 기계검사한다. `keep`이 전부
참이므로 P를 실제로 소비하는 분기가 살아 있는 입력이다.

호출부 쪽에서는 `cargo test -p loom-runtime` 109건 전원 통과 — 새로 넣은 Q의
`debug_assert!`가 켜진 채로다(`d_lis_r1`/`e_lis_r1`이 이 경로를 탄다).

### 남은 것

`lis_indices`가 P를 세운다는 것(**LIS 건전성**)은 여전히 미증명이다. 이번에
증명된 것은 "P ∧ Q ⟹ 수렴"이고, "LIS가 P를 만든다"가 아니다. 그 경계는
`place_in_order_lis`의 두 `debug_assert!`(P, Q)와 무작위 속성 검사가 지킨다.

> **2026-10-17 정정**: 둘 다 그 뒤 검증 안으로 들어왔다 — Q는 2026-08-11 "경계를
> 옮겼다" 절에서 `plan_inputs`의 사후조건이 됐고, P는 같은 날 "LIS 건전성" 절에서
> `lis_keep`의 사후조건이 됐다. 남은 비검증 계약은 원시 입력의 구조적 사실
> 넷(`selection_preconditions_hold`, `debug_assert!`)이다. 당시 기록으로 남긴다.


---

## 채택 후 보강 (2026-08-09, 병렬 시도 4안 중 채택안에 적용)

네 시도가 **전부** 목표 사후조건과 바이트 동일한 `ensures`로 통과했고, 넷 다
독립적으로 **같은 누락 사전조건 Q**를 찾았다. 판정자가 각 워크트리를 직접
재검증(2회 반복, 크래시 없음)하고 돌연변이 2종으로 대조한 뒤 부담이 가장 가벼운
안을 채택했다. 채택 후 판정이 지적한 것을 반영했다:

1. **경고 5건 제거** — `reveal(Seq::<u64>::filter)`의 타입 인자가 무시된다는
   경고. `Seq::<_>::filter`로. 이 모듈의 경고는 계속 0건이어야 한다.
2. **호출부 Q 검사를 O(n²) → O(n)** — `list.rs`의 `debug_assert!`가 이중 루프
   였다. 바로 위 P 검사가 맵으로 O(n)인 것과 같이 `HashSet`으로.
3. **루프 불변식의 Q 사본 제거** — Q는 `current@`/`target_ids@`(둘 다 루프에서
   불변)만 말하므로 함수 `requires`가 루프 안에서도 살아 있다. "Verus의 while은
   불변식만으로 모듈러 검증된다"는 규칙을 **변하지 않는 상태에까지** 과잉 적용한
   것이었다. 지우고 통과 확인(71 verified).
4. **양성 대조의 증인 교체 + 음성 대조 추가** — 채택안의 양성 증인은
   `current == target`이라 실행이 퇴화했다(방출 op 0개 = 아무것도 시험하지
   않는다). 다른 시도의 증인(`c=[7,9]`, `t=[5,7,9]`, `fr=[T,F,F]`, `kp=[F,T,T]`
   — P 비공허 + Fresh 포함 + `current != target`)으로 바꾸고,
   `lemma_q_rejects_extras`(Q 없이는 목표 사후조건이 **거짓**임을 기계로 세운다)를
   추가했다.

### 최종 상태와 음성 대조

```
verus --crate-type=lib crates/loom-flush-core/src/lib.rs   → 71 verified, 0 errors, 경고 0
verus formal-verification/verus/flush_order.rs             → 28 verified, 0 errors
```

| 주입한 결함 | 결과 |
|---|---|
| 수렴 사후조건의 `target_ids@`를 `.reverse()`로 | `postcondition not satisfied` (70 verified, 1 error) |
| `requires`에서 Q 제거 | `assertion failed` (70 verified, 1 error) |

즉 사후조건이 load-bearing이고 Q가 장식이 아니다.

### 남는 것 (숨기지 않음)

- **계약이 좁아졌고 그 대가를 비검증 코드가 진다.** 이제 `place_in_order_lis`가
  지킬 조항이 P와 Q **둘**이고, 둘 다 `debug_assert!`(릴리스에서는 사라진다)와
  무작위 속성 검사로만 지켜진다.
  > **2026-10-17 정정**: 2026-08-11에 Q(`plan_inputs` 사후조건)와 P(`lis_keep`
  > 사후조건) 둘 다 기계검사로 옮겨졌다. 지금 `debug_assert!`로만 지켜지는 것은
  > 원시 입력의 구조적 사실 넷이다(맨 아래 "남는 것"). 당시 기록으로 남긴다.
- **`lis_indices`가 P를 세운다는 LIS 건전성은 여전히 미증명이다.** 증명된 것은
  "P ∧ Q ⟹ 수렴"이지 "LIS가 P를 만든다"가 아니다.
  > **2026-08-11 정정 (4단계)**: 이제 증명됐다. 아래 "LIS 건전성 — 증명 완료,
  > 사슬이 닫혔다" 절 참고. 이 항목은 당시 기록으로 남긴다.
- Q는 **최소가 아니다**. 더 약한 P2("extras는 전부 표시된 원소보다 `current`에서
  앞")로도 전수 탐색 반례 0건(177,827건)이었다. Q를 택한 이유는 P2가 다시 2변수
  양화자라는 **성능 근거**지 수학적 필연이 아니다.
- 다음 수: `match_keys`를 `loom-flush-core`로 옮겨 Q를 검증 안으로 끌어들이는
  것. Q 쪽 구조적 논증이 P보다 훨씬 단순하다.

## 경계를 옮겼다 — Q가 사후조건이 됐다 (2026-08-11)

바로 위 절의 "다음 수"를 했다. 다만 `match_keys` **전체**를 옮기지는 않았다 —
아래 "왜 이 모양인가" 참고.

### 무엇이 어디로 갔나

**매칭과 구성을 분리**했다. 키 비교(String)는 `list.rs`(비검증)에 남기고, 그
결과를 **인덱스 선택** `take_old: Vec<usize>`(새 슬롯 `i`가 가져갈 옛 인덱스,
`>= old_ids.len()`이면 fresh)로 표현해 검증 코어에 넘긴다. 코어는 그 선택
하나로부터 `current`와 `target_ids`/`target_fresh`를 **함께** 만든다.

그래서 **Q가 구조적으로 참**이다 — 매칭이 옳든 그르든 상관없다. `current`의
원소는 정의상 "어떤 슬롯이 가져간 옛 원소"이고, 그 슬롯의 `target_ids`가 바로
그 원소이기 때문이다. Q는 이제 `plan_inputs`의 **사후조건**이고, `list.rs`의
Q `debug_assert!`는 **삭제됐다**(검사할 것이 남아 있지 않다).

새로 생긴 것(전부 `crates/loom-flush-core/src/lib.rs`):

| 이름 | 무엇 |
|---|---|
| `plan_inputs` | 경계 함수. `(old_ids, take_old, fresh_ids)` → `PlanInputs { current, removed, target_ids, target_fresh }` |
| `plan_from_selection` | 비-LIS 경로(D·E). 위를 부르고 이어서 `plan_writes_converging`까지 |
| `plan_lis_from_selection` | LIS 경로(D-LIS·E-LIS). 위를 부르고 이어서 `plan_writes` |
| `selection_order_preserved` | `current`가 옛 순서를 보존한다(불투명). P 번역의 재료 |
| `take_order_preserved` | P를 `take_old` 항으로 옮긴 형태(불투명) + 음성/양성 대조 2건 |

### `plan_inputs`가 실제로 세우는 사후조건

```
r.target_ids@.len() == take_old@.len()
r.target_fresh@.len() == r.target_ids@.len()
forall|i| r.target_fresh@[i] == (take_old@[i] >= old_ids@.len())
forall|i| r.target_ids@[i] == if take_old@[i] < old_ids@.len()
                                  { old_ids@[take_old@[i]] } else { fresh_ids@[i] }
seq_nodup(r.current@)                      // == r.current@.no_duplicates()
seq_nodup(r.target_ids@)                   // == r.target_ids@.no_duplicates()
forall|x| r.current@.contains(x) ==> r.target_ids@.contains(x)          // ← Q
forall|i| (r.target_fresh@[i] ==> !r.current@.contains(r.target_ids@[i]))
       && (!r.target_fresh@[i] ==> r.current@.contains(r.target_ids@[i]))
forall|x| r.removed@.contains(x) ==> !r.current@.contains(x)
selection_order_preserved(r.current@, old_ids@, take_old@, r.target_ids@)
```

즉 `plan_writes`의 **여섯 사전조건 중 다섯**(Q + 무중복 2 + 길이 + fresh↔포함)이
여기서 선다. 남는 것은 **P 하나**뿐이고, 그마저도 `plan_lis_from_selection`이
`take_order_preserved`(= P를 `take_old` 항으로 옮긴 것)로 받아 `current` 항의
P(`kept_order_preserved`)로 **번역하는 것까지 기계검사**한다. 번역 다리가
`selection_order_preserved`다.

덤으로 `removed ∩ current = ∅`이 증명된다 — despawn하는 row가 곧이어 배치되는
일이 있을 수 없다는 뜻이고, 예전 코드는 이걸 아무 데서도 확인하지 않았다.

무중복 둘을 불투명 `seq_nodup`으로 낸 이유는 이 모듈의 기존 처방과 같다(raw
`no_duplicates`는 2변수 양화자). 소비자(두 래퍼)가 자기 안에서만 편다.

### 왜 `match_keys` 전체가 아니라 "인덱스 선택"인가

정찰은 Verus의 String 지원으로 `match_keys` 전체 이관이 **가능**함을 확인했다
(`&String == &String`이 아니라 `*a == *b` 형태를 써야 한다는 조건 하에).
그런데도 인덱스 선택으로 쪼갠 이유:

- **Q에 키가 전혀 필요 없다.** Q는 Entity 수준 명제이고 어떤 키 조건도 요구하지
  않는다(전수 확인: 적대적 스윕에서도 위반 0건). 키를 검증 코어에 들여도 Q
  증명이 한 줄도 쉬워지지 않는다.
- **`Vec<String>` 클론이 검증 경계를 넘나들면 트랙 L 측정이 흔들린다.** 지금
  구조는 `list.rs`가 이미 만들어 둔 것만 `u64`/`usize`로 넘긴다.
- **중복 키가 실제로 발생한다**(아래) — 키를 계약에 넣으면 그 계약을 벤치가
  위반한다.

### 남는 것 (숨기지 않음)

- **`lis_indices`가 P를 세운다는 LIS 건전성은 여전히 미증명이다.** 이번에도
  아니다. 다만 남은 미검증 계약이 **정확히 그것 하나**가 됐다.
  > **2026-08-11 정정 (같은 날 4단계)**: 그 하나도 닫혔다. 아래 "LIS 건전성 —
  > 증명 완료, 사슬이 닫혔다" 절 참고. 이 항목은 당시 기록으로 남긴다.
- **비검증 코드가 지는 짐: 여섯 → 넷**, 그리고 성격이 바뀌었다. 예전 넷은
  `current`/`target`의 *상호작용*에 대한 명제였고, 지금 넷은 원시 입력에 대한
  **구조적 사실**이다: 옛 Entity 유일성 / `take_old` 단사성 / fresh Entity가 옛
  것과 겹치지 않음 / fresh끼리 다름. `list.rs`의
  `selection_preconditions_hold`가 디버그 빌드에서 넷 **전부**를 검사한다
  (예전에는 뒤 셋을 아무도 검사하지 않았다). 첫 번째는 귀납적으로 닫혀 있다 —
  `ListRows`/`OrderKeyMembers`는 `Default`로만 생기고 오직 `target_ids`로만
  대입되는데 그 유일성이 이제 사후조건이다.
- **rlimit 여유가 줄었다.** 모듈 전체가 34.4M → 48.6M rlimit(smt-run 7.0s →
  12.3s). 늘어난 몫의 대부분은 새 함수 자신이 아니라(`plan_inputs` 단독 0.54M)
  **기존 최대 비용 증명 `chain_unlink_denotes_remove`가 10.6M → 13.2M로
  오른 것**이다 — 이 모듈이 이미 문서화한 "새 심볼이 모듈 문맥을 흔든다"
  현상이고, `--profile-all`은 그 함수 인스턴스화의 87%를 `Seq::no_duplicates`
  하나(vstd `seq_lib.rs:520`)로 지목한다. 기본값(`--rlimit 10`)에서는 통과하지만
  임계가 **3 → 7**로 올라갔다. 즉 여유가 3.3배에서 1.4배가 됐다. 새 코드를
  별도 서브모듈로 가두는 안도 시도했으나 오히려 기본값에서 실패했다(더 나쁨).
  다음에 이 모듈을 건드리는 사람은 `chain_unlink_denotes_remove`부터 손봐야
  한다.

### 방출 동일성

`cargo test -p loom-runtime --test write_identity` 통과 — 후보 5종 × R1/R2 ×
8단계 골든이 한 글자도 안 바뀌었다. `cargo test --workspace` 195 통과,
`loom-track-n --cases 40` 6 arm 전부 통과.

두 군데가 "같은 결과가 나오는 이유"를 특별히 적어 둔다:

1. **fresh row spawn이 despawn 앞으로 옮겨졌다.** `Commands::spawn`은 호출
   시점에 Entity를 예약하고 `despawn`은 sync point까지 미뤄지므로 id 배정이
   같고, `pending`에 들어가는 write 순서(RemoveNode 먼저)도 그대로다.
2. **LIS 입력이 `current` 안의 자리에서 옛 인덱스로 바뀌었다.** `current`는 옛
   순서를 보존하는 부분열이라 두 수열은 순서 동형이고, `lis_indices`는 값의
   대소만 보므로(`sequence[ti] < val` 하나뿐) 고르는 집합이 글자 그대로 같다.
   덤으로 reconcile마다 짓던 `HashMap<Entity, usize>` 하나가 없어졌다.

### 곁다리 — 사실이 아니던 주석 하나

`list.rs`의 `match_keys`에 있던 "Phase 1's demo doesn't exercise duplicate
keys"는 **틀렸다**. `crates/loom-bench`의 POD 축 × W7(`value_update`) 셀에서
fresh 민터의 공역(100만)이 n=10,000에서 부족해져 rep의 34%가 중복 키를 D/E에
먹인다. 성질은 전부 성립하므로 수집분이 무효가 되지는 않지만(전수 확인:
`match-keys-exhaustive.py`), 주석은 이번에 고쳤다. 별건으로 남길 것: 그 공역
부족은 W7의 "10% 값 변경" 조항을 계통적으로 미달시킨다(RQ1 §3.1 계측 정확도).

## LIS 건전성 — 증명 완료, 사슬이 닫혔다 (2026-08-11, 같은 날 4단계)

바로 위 절이 "남은 미검증 계약이 **정확히 그것 하나**"라고 적은 그것 —
`lis_indices`가 사전조건 P를 세운다는 것 — 을 증명했다. **이제 P는 가정이
아니라 실행 코드의 사후조건이다.**

### 무엇이 어디로 갔나

`list.rs`의 `lis_indices`(patience sorting)와 `lis_keep`(마스크 조립)이 통째로
`crates/loom-flush-core/src/lib.rs`로 옮겨 갔다. 새로 생긴 것:

| 이름 | 무엇 |
|---|---|
| `lis_keep` | `(take_old, old_len)` → `keep: Vec<bool>`. **`take_order_preserved`가 사후조건** |
| `plan_lis_from_selection_auto` | **사슬이 닫히는 자리**. `lis_keep`을 부르고 그 결과로 `plan_lis_from_selection`을 부른다 |
| `lower_bound` | `partition_point`의 의미를 그대로 적은 명시적 이분 탐색 |
| `tails_sorted` | I3(`tails`가 가리키는 값이 엄격 증가). **불투명** — 2변수 + 이중 인덱싱 |
| `tails_bounded` / `pred_local` | I2 / I1. 둘 다 1변수라 불투명화 불필요 |
| `lemma_sorted_after_set` / `lemma_sorted_after_push` | I3 보존(교체 가지 / 연장 가지) |

`list.rs`의 P `debug_assert!`는 **삭제됐다**. 그 파일에 P에 대한 계약이 이제
한 줄도 없다 — `keep`이 인자로 오가지 않기 때문이다.

### 왜 래퍼 한 겹(`plan_lis_from_selection_auto`)이 필요했나

`list.rs`에서 `lis_keep(...)` → `plan_lis_from_selection(..., &keep)`으로
이어 놓는 것으로는 **사슬이 닫히지 않는다**. `loom-runtime`은 검증 대상이
아니므로 그 두 계약 사이를 아무도 검사하지 않고, 예전 `debug_assert!`와 같은
종류의 틈이 이름만 바꿔 남는다. 두 호출이 **한 검증 단위 안에** 있어야 한다.

기계로 확인했다 — `lis_keep`의 `ensures`에서 `take_order_preserved` 한 줄을
지우면:

```
error: precondition not satisfied
    --> lib.rs:3672:5
     |
3560 |         take_order_preserved(old_ids@.len(), take_old@, keep@),
     |         ------------------------------------------------------ failed precondition
...
3672 |     plan_lis_from_selection(old_ids, take_old, fresh_ids, &keep)
     |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
verification results:: 89 verified, 1 errors
```

즉 사후조건 → 사전조건이 **실제로** 이어져 있다.

### 건전성만 증명했다 — 최적성은 아니다

의도적이다. P는 "표시된 것들이 증가 부분수열을 이룬다"와 같고, 그것이면
충분하다. LIS가 최적이 아니면 이동이 더 나올 뿐 순서는 깨지지 않는다(성능
성질이지 정합성 성질이 아니다). 그래서 patience sorting이 *가장 긴* 부분수열을
찾는다는 것은 증명하지 않았고, 필요한 것은 predecessor 사슬의 **국소
불변식**뿐이었다:

```
I1  pred[i] == Some(p) ==> p < i && seq[p] < seq[i]        (pred_local)
I2  tails[k] < i                                           (tails_bounded)
I3  k1 < k2 ==> seq[tails[k1]] < seq[tails[k2]]            (tails_sorted, 불투명)
```

사슬 되짚기의 갱신 부담은 **1변수** 불변식이 진다("표시된 모든 m이 cursor보다
뒤에 있고 값도 크다"). 그러면 `keep[cursor] = true` 시점에 새 원소 대 기존
원소가 전부 덮여 2변수 쌍별 성질이 자동 보존된다. 종료 척도는 I1의 `p < i`가
그대로 준다.

### `partition_point`는 못 쓴다

vstd에 스펙이 없다(`/opt/verus/vstd` 전수 grep 0건, 프로브로 실증:
`core::slice::impl&%0::partition_point` is not supported). Verus가 제시하는
유일한 해결책이 `assume_specification`인데 그것은 우회 금지 조항에 걸린다.
그래서 의미가 같은 명시적 이분 탐색(`lower_bound`)으로 대체했다 — 술어가 `<=`가
아니라 **`<`**이므로 둘 다 lower bound이고, `mid = lo + (hi - lo) / 2`라
오버플로가 없으며 **O(n log n)이 유지**된다.

### 중복 값에 `no_duplicates`가 필요 없다

`lis_keep`에는 **사전조건이 하나도 없다**. 이분 탐색 술어가 `<`라
`pred[i] = tails[pos-1]`이 `seq[tails[pos-1]] < val`을 **엄격하게** 만족하고,
즉 이 알고리즘은 strictly increasing 사슬만 만든다 — 같은 값 둘이 한 사슬에
들어갈 수 없다. I3도 엄격한 채로 유지된다. 그래서 이 모듈이 전에 128초를
태웠던 `Seq::no_duplicates` 2변수 양화자를 `requires`에 걸 필요가 **없었다**.

### 방출 동일성 — 알고리즘은 한 글자도 안 바뀌었다

옛 코드는 `take_old`에서 retained 슬롯만 뽑은 `retained_old_indices`에 LIS를
돌린 뒤 순위(rank)로 되짚어 `keep`을 만들었다. 새 코드는 `take_old`를 직접
훑되 비-retained 슬롯을 **건너뛴다**. 처리되는 값의 열과 순서가 같고 비교가
`sequence[ti] < val` 하나뿐이므로 `tails`/`pred`가 담는 인덱스가 순서 동형으로
대응하고, 고르는 집합이 글자 그대로 같다 — 대신 순위 되짚기라는 간접 참조 한
겹이 없어져 증명이 짧아진다.

기계로 확인했다. `lis-output-identity-exhaustive.py`가 옛 구현과 새 구현의
`keep` 벡터를 전수 대조한다:

| 입력 공간 | 건수 |
|---|---|
| 전 순열 n≤7 (전부 retained) | 5,914 |
| fresh 혼합 전수 (중복 포함) | 5,704 |
| 중복 값 열 ({0,1}≤9, {0,1,2}≤7, {0,1,2,3}≤6) | 9,764 |
| 단사 retained + fresh (실제 `match_keys` 출력 모양) | 8,771 |
| **합계** | **30,153** |

`keep` 벡터 불일치 **0건**, P 위반 **0건**.

### 명세가 공허하지 않음을 확인한 음성 대조

| 주입한 결함 | 결과 |
|---|---|
| `lis_keep`의 `take_order_preserved` 사후조건 제거 | `precondition not satisfied` (89 verified, 1 error) |
| 사슬 밖 원소도 표시(`keep.set(idx - 1, true)`) | 루프 불변식 2건 `not satisfied` (89 verified, 1 error) |

기존 `lemma_take_order_rejects_inversion`(음성) /
`lemma_take_order_accepts_increasing`(양성)도 그대로 통과한다.

### rlimit 여유가 **늘었다** (예상 밖)

위 절이 "임계가 3 → 7로 올라갔다, 여유가 3.3배에서 1.4배가 됐다"고 경고했는데,
이번 변경 뒤 임계가 **7 → 5로 내려갔다**(여유 1.4배 → 2.0배). 같은 기계에서
재현 확인:

| 모듈 | `--rlimit 4` | `--rlimit 5` | `--rlimit 6` | `--rlimit 7` |
|---|---|---|---|---|
| 이번 변경 전(HEAD) | — | 79 verified, 1 error | 79 verified, 1 error | **80 verified, 0 errors** |
| 이번 변경 후 | 89 verified, 1 error | **90 verified, 0 errors** | — | — |

모듈 전체 비용도 같은 방향이다(`--time`, 같은 기계):

| | 총 rlimit | smt-run | total-time |
|---|---|---|---|
| 이번 변경 전(HEAD, 80 verified) | 52,235,573 | 6,732 ms | 8,338 ms |
| 이번 변경 후(90 verified) | **39,585,099** | **6,437 ms** | **8,221 ms** |

검증 함수가 10개 늘었는데 총 rlimit이 **24.2% 줄었다**.

> **기준선 수치 정정(2026-08-11, 채택 직후 실측)**: 이 표에 처음 적힌 기준선은
> 48,585,897 / 7,971ms / 9,616ms였고 감소폭을 18.5%로 적었다 — **셋 다 틀렸다**.
> 유휴 머신에서 2회 재측정한 결정적 값이 52,235,573 / 6,732ms / 8,338ms이고
> 감소폭은 24.2%다(판정자 재검증과도 일치). rlimit은 결정적 수치라 잡음이
> 아니었다. 이 표는 다음 사람이 회귀 판정에 쓰는 기준선이므로 실측값으로 고쳤다.
>
> **이 개선은 재현 보장이 없다.** 병목 `chain_unlink_denotes_remove`는 한 줄도
> 안 바뀌었고 여전히 단독 12.7~13.2M rlimit이며 그 인스턴스화 대부분이 vstd의
> `Seq::no_duplicates`다. **무관한 심볼을 하나 더 얹는 순간 여유가 다시 사라질
> 수 있다.** 그래서 모듈 총 rlimit을 여기 기준선으로 못박아 둔다 — 이번 라운드에
> 네 후보 중 셋이 이 수치를 틀리게 보고한 이유가 정확히 "아무도 이 값을 고정
> 기준선으로 갖고 있지 않아서"였다.

병목은 그대로 `chain_unlink_denotes_remove`이고 `--profile`이 그 함수
인스턴스화의 **87%**(298,680 / 340,414)를 vstd `seq_lib.rs:520`의
`Seq::no_duplicates` 하나로 지목한다 — 새 코드는 병목이 아니다. 임계가 내려간
것은 "새 심볼이 모듈 문맥을 흔든다"는 이 모듈의 알려진 현상이 이번엔 좋은
쪽으로 작용한 것으로 보인다. **인과를 단정하지는 않는다** — 관측값만 적어
둔다. 다음에 이 모듈을 건드리는 사람은 여전히
`chain_unlink_denotes_remove`부터 손봐야 한다.

### 기준선

```
verus --crate-type=lib crates/loom-flush-core/src/lib.rs   → 90 verified, 0 errors, 경고 0
verus formal-verification/verus/flush_order.rs             → 28 verified, 0 errors
cargo test --workspace                                     → 195 passed
cargo test -p loom-runtime --test write_identity           → 1 passed
cargo run --release -p loom-track-n -- --cases 40          → 6 arm 전부 통과
```

### 남는 것 (숨기지 않음)

- **비검증 코드가 지는 짐: 넷 → 넷.** P가 빠졌지만 원시 입력에 대한 구조적
  사실 넷(`selection_preconditions_hold`)은 그대로다. 그 넷은 여전히
  `debug_assert!`로만 지켜진다 — 성격이 다른 계약이고 이번 범위 밖이었다.
- **최적성은 증명되지 않았다.** 위에 적은 대로 의도적이지만, "LIS 변형이 정말
  이동을 최소화하는가"는 RQ1의 **성능** 주장이고 그것은 여전히 측정으로만
  뒷받침된다.
- **`lis_keep`은 `Vec<usize>`를 받는다**(`&[usize]`가 아니라). vstd의 슬라이스
  스펙이 얇아서 그렇게 뒀다. 호출부가 이미 `Vec`을 들고 있어 실제 비용은 없다.

---

## 2026-10-20 — G0R1 C10: 호출부 key 표현이 바뀐 뒤의 재실행 (로직 변경 0줄)

프로토콜 개정 #12(G0R1)로 `loom-runtime`의 D·D-LIS·E·E-LIS 호출부가 key를 `to_string()`으로 만든 `String`에서 아이템 전체의
고정 해시 `u64` + `Item` 동등 검증으로 바꿨다. 이 크레이트(`loom-flush-core`)의 **로직은 한 줄도 바뀌지 않았다** — 입력 `take_old`의 계약이
같고(동치 증명: `crates/loom-runtime/src/list_key.rs` 머리말, 무작위 오라클 테스트 `match_equivalence_against_legacy_oracle`),
이 크레이트가 매칭을 안 본다는 점도 그대로다. 바뀐 것은 `plan_inputs` 앞 doc 주석 한 줄뿐이다("키 비교(String)는 호출부에 남고" →
"키 매칭은 호출부에 남고" + 날짜 정정 블록). 그래도 주석이 바뀌면 소스 바이트가 바뀌므로(panic 위치 줄 번호 → wasm 바이트, memory
`comment-edits-change-wasm`) "검증된 소스 = 배포되는 소스" 대응을 새 sha256으로 다시 세운다.

| 항목 | 값 |
|---|---|
| `crates/loom-flush-core/src/lib.rs` sha256 (전) | `61575b9e791df0e7ca0728d7be10487a453be79bb75422d920faec28d36623d5` (0ff9bd5) |
| 같은 파일 sha256 (후) | `393f3523ba0d2d9128fc48ca2667260b0f300391266795ff35d62afdf0389db9` (이 커밋) |
| `verus --crate-type=lib crates/loom-flush-core/src/lib.rs` | **90 verified, 0 errors** (전 기록과 같다) |
| `verus formal-verification/verus/flush_order.rs` | **28 verified, 0 errors** |
| 툴체인 | `/usr/bin/verus` → `/opt/verus/verus`, 패키지 `verus-singular 0.2026.07.27.31579f0-2`, `--version`은 "Version: Unknown / Profile: release" |

**증명 범위(숨기지 않음)**: 이 재실행은 `plan_inputs`·`plan_*`·`lis_keep`·`Chain`의 사후조건이 이 소스에서 여전히 성립함을 보인다. 호출부의
`match_by_key`(radix 병합)는 **검증 대상이 아니다** — 그것은 Rust 테스트(무작위 동치 3×4 조합 + 변이 점검)와
`match-keys-exhaustive.py`의 모형 전수 조사(sweep 6)로만 뒷받침된다. 모형과 Rust 소스 사이의 대응은 "한 줄씩" 읽어서 맞춘 것이지
기계가 검사한 것이 아니다.
