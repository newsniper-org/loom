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

현재 결과: **15 verified, 0 errors** (Verus 업스트림 설치본, `/usr/bin/verus`).

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

대조 A가 특히 중요하다 — 정리 (1)이 배제하는 바로 그 폴백을 강제로 켜면 (2)가
깨진다는 것이, 두 정리가 실제로 맞물려 있음을 보여 준다.

## 남은 것 (숨기지 않음)

1. **`already_adjacent` 스킵 최적화의 안전성**: 모델은 스킵을 생략하고 Retained도
   항상 `Move`를 방출하는 **상위집합**을 증명한다. 스킵은 write를 줄이기만 하고
   스킵된 row는 Retained라 `current.contains`로 계속 해석 가능하므로 (1)을
   깨뜨릴 수 없다 — 그러나 **이 논증 자체를 정리로 만들지는 않았다**. (2)에
   대해서는 더 강한 주장이 필요하다: 스킵된 row가 "이미 올바른 위치에 있다"는
   것을 증명해야 접미사 불변식이 유지된다.
2. **모델-구현 결선 미완**: 이 파일은 `place_in_order`의 구조를 옮겨 적은
   것이지, `list.rs`의 그 함수를 직접 검증한 것이 아니다. 직접 검증하려면
   `Chain`(std `HashMap` 2개)을 Verus가 다룰 수 있는 형태로 재작성해야 하고,
   그건 §2.5 동결 규율상 트랙 L 재수집을 동반한다(RQ5 문서의 비용 분석 참고).
   따라서 현재 상태는 "Lean4보다는 코드에 가깝지만 아직 코드 그 자체는 아님"
   이다 — 이 한계를 과장하지 말 것.
3. **`emit_writes`와 `apply_k`의 결선**: `apply_k`는 `emit_writes`의 사후조건을
   `t`(목표 id열)만으로 다시 쓴 것이다. 둘이 같은 시퀀스를 가리킨다는 것을
   별도 정리로 잇지 않았다 — 사후조건을 눈으로 대조한 상태다.
