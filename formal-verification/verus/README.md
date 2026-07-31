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

현재 결과: **4 verified, 0 errors** (Verus 업스트림 설치본, `/usr/bin/verus`).

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

## 남은 것 (숨기지 않음)

1. **정리 (2) 순서 수렴 미증명**: (1)을 전제로 "적용 루프의 최종 DOM 순서 =
   target 순서"를 아직 증명하지 않았다. (1)이 폴백 부재를 보장하므로 남은 것은
   "각 노드를 자기 오른쪽 이웃 앞에 놓는 연산을 오른쪽부터 수행하면 목표
   순서가 된다"인데, 이는 별도의 귀납 증명이 필요하다.
2. **`already_adjacent` 스킵 최적화의 안전성**: 모델은 스킵을 생략하고 Retained도
   항상 `Move`를 방출하는 **상위집합**을 증명한다. 스킵은 write를 줄이기만 하고
   스킵된 row는 Retained라 `current.contains`로 계속 해석 가능하므로 (1)을
   깨뜨릴 수 없다 — 그러나 **이 논증 자체를 정리로 만들지는 않았다**.
3. **모델-구현 결선 미완**: 이 파일은 `place_in_order`의 구조를 옮겨 적은
   것이지, `list.rs`의 그 함수를 직접 검증한 것이 아니다. 직접 검증하려면
   `Chain`(std `HashMap` 2개)을 Verus가 다룰 수 있는 형태로 재작성해야 하고,
   그건 §2.5 동결 규율상 트랙 L 재수집을 동반한다(RQ5 문서의 비용 분석 참고).
   따라서 현재 상태는 "Lean4보다는 코드에 가깝지만 아직 코드 그 자체는 아님"
   이다 — 이 한계를 과장하지 말 것.
