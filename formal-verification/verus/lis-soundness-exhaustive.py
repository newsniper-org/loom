"""`lis_indices`의 **건전성 불변식**을 전수 탐색으로 확정한다 (2026-08-11).

`lis-keep-exhaustive.py`가 남긴 마지막 미검증 계약 하나 —
"`lis_indices`가 실제로 사전조건 P를 세우는가" — 를 Verus로 옮기기 전의
정찰이다. 그 스크립트는 P가 **충분조건이자 필요조건**임을 보였고
(2026-08-02), 4번 검사에서 `lis_indices`가 P를 세운다는 것도 봤지만
"전 순열"만 봤다. 여기서는 **증명에 쓸 루프 불변식 자체**를 매 스텝
검사하고, **중복 값이 있는 열**까지 훑는다.

## 왜 불변식을 따로 보는가 — 건전성만 필요하고 최적성은 필요 없다

P는 "표시된 것들이 **증가** 부분수열을 이룬다"와 같다. 그것이 **가장 긴지**는
정합성과 무관하다(최적이 아니면 이동이 더 나올 뿐 순서는 안 깨진다 — 성능
성질). 따라서 patience sorting의 최적성을 증명할 필요가 없고, 아래 **국소**
불변식 넷만 서면 된다:

  I1  pred[i] == Some(p)  ==>  p < i  and  seq[p] < seq[i]
  I2  모든 k에 대해 tails[k] < len(seq)
  I3  k1 < k2  ==>  seq[tails[k1]] < seq[tails[k2]]
  I4  len(tails)는 감소하지 않는다
  I5  (I1의 따름) tails[-1]에서 pred를 따라간 사슬은 인덱스도 값도 **엄격히
      감소**한다 — 뒤집으면 증가 부분수열, 즉 P

I1~I4는 루프 불변식으로 그대로 Verus에 옮길 수 있는 모양이고, I5는 두 번째
루프(사슬 되짚기)의 불변식이다.

## 이 스크립트가 답하는 것 다섯

  1. I1~I4가 **매 스텝** 성립하는가 (전 순열 n≤7 + 중복 있는 열)
  2. I5(사슬 엄격 감소)가 성립하는가
  3. `partition_point(|&ti| seq[ti] < val)`을 **명시적 이분 탐색**으로 바꿔도
     결과가 글자 그대로 같은가 — vstd에 `partition_point` 스펙이 **없어서**
     (2026-08-11 `/opt/verus/vstd` 전수 grep: 0건) 검증 크레이트에서는 직접
     써야 하고, 다르면 방출이 달라진다(실격 조건 1번)
  4. `HashSet` 반환을 `Vec<bool>` 마스크로 바꿔도 `keep`이 같은가 — 이관 시
     API를 마스크로 바꾸는 것이 안전한지
  5. `take_old` → retained → lis → keep → `take_order_preserved` **사슬 전체**가
     성립하는가 (중복 있는 `take_old`까지)

## 중복 값에 관하여

`retained_old_indices`는 옛 인덱스라 실제로는 중복이 없다. 그래도 훑는 이유는
검증 크레이트의 `requires`를 **정하기 위해서**다 — 중복에도 성질이 서면
`no_duplicates`를 사전조건으로 걸 필요가 없고, 그러면 2변수 양화자 하나를
루프 문맥에 들이지 않아도 된다(이 모듈이 실제로 그것 때문에 128초를 태운 적이
있다 — README의 `seq_nodup` 절).

핵심 관찰: `partition_point`의 술어가 `seq[ti] < val`(`<=`가 아니라 `<`)이라
`pos`는 **lower bound**다. 따라서 `pred[i] = tails[pos-1]`은 항상
`seq[tails[pos-1]] < val`을 만족한다 — **엄격**하다. 즉 이 알고리즘은
`>=`가 아니라 `>`로 잇는 **strictly increasing** LIS이고, 중복이 있어도
사슬 안에 같은 값이 둘 들어가지 않는다. 이 스크립트가 그것을 실측으로 확인한다.

이 스크립트는 증명이 아니라 **증명 대상이 참임을 뒷받침하는 실측**이다.

실행: `python3 formal-verification/verus/lis-soundness-exhaustive.py`
"""

from itertools import permutations, product

# ---------------------------------------------------------------------------
# 대상 코드의 모형 — `crates/loom-runtime/src/list.rs`의 `lis_indices`/`lis_keep`
# ---------------------------------------------------------------------------


def partition_point_ref(tails, seq, val):
    """Rust `tails.partition_point(|&ti| seq[ti] < val)`의 **의미** 그대로.

    Rust의 `partition_point`는 "술어가 참인 원소의 개수"(분할되어 있다는 가정
    하에)다. 여기서는 그 정의를 그대로 세어서 계산한다 — 이분 탐색 구현과
    대조할 **기준선**이므로 이분 탐색을 쓰지 않는다."""
    return sum(1 for ti in tails if seq[ti] < val)


def partition_point_binsearch(tails, seq, val):
    """검증 크레이트에서 쓸 **명시적 이분 탐색**. vstd에 `partition_point`
    스펙이 없으므로 이것을 직접 쓰고 불변식을 건다. O(log n) 유지."""
    lo, hi = 0, len(tails)
    while lo < hi:
        mid = lo + (hi - lo) // 2
        if seq[tails[mid]] < val:
            lo = mid + 1
        else:
            hi = mid
    return lo


class InvariantViolation(Exception):
    pass


def check_step_invariants(seq, tails, pred, i, where):
    """I1~I3을 현재 상태에 대해 전부 확인한다. I4는 호출부에서 길이로 본다."""
    # I1 — 지금까지 확정된 pred 전부
    for k in range(len(pred)):
        p = pred[k]
        if p is None:
            continue
        if not (p < k):
            raise InvariantViolation(f"I1(index) seq={seq} {where} i={i} pred[{k}]={p}")
        if not (seq[p] < seq[k]):
            raise InvariantViolation(f"I1(value) seq={seq} {where} i={i} pred[{k}]={p}")
    # I2
    for k, ti in enumerate(tails):
        if not (0 <= ti < len(seq)):
            raise InvariantViolation(f"I2 seq={seq} {where} i={i} tails[{k}]={ti}")
    # I3
    for k in range(len(tails) - 1):
        if not (seq[tails[k]] < seq[tails[k + 1]]):
            raise InvariantViolation(
                f"I3 seq={seq} {where} i={i} tails={tails} vals="
                f"{[seq[t] for t in tails]}"
            )
    # 부가: tails의 원소는 전부 이미 처리된 인덱스여야 한다(< i 또는 == i)
    for k, ti in enumerate(tails):
        if ti > i:
            raise InvariantViolation(f"I2' seq={seq} {where} i={i} tails[{k}]={ti}")


def lis_indices_traced(seq, pp=partition_point_ref, check=True):
    """`list.rs`의 `lis_indices`를 한 스텝씩 돌리며 불변식을 검사한다.

    반환: (선택된 인덱스 집합, 사슬(되짚은 순서 그대로), pred, tails)"""
    if not seq:
        return set(), [], [], []
    tails, pred = [], [None] * len(seq)
    prev_tail_len = 0
    for i in range(len(seq)):
        val = seq[i]
        pos = pp(tails, seq, val)
        if pos > 0:
            pred[i] = tails[pos - 1]
        if pos == len(tails):
            tails.append(i)
        else:
            tails[pos] = i
        if check:
            check_step_invariants(seq, tails, pred, i, "after")
            # I4 — tails 길이는 감소하지 않는다
            if len(tails) < prev_tail_len:
                raise InvariantViolation(f"I4 seq={seq} i={i} {prev_tail_len}->{len(tails)}")
            prev_tail_len = len(tails)

    # 사슬 되짚기 — I5
    chain, cur = [], tails[-1]
    while cur is not None:
        chain.append(cur)
        cur = pred[cur]
    if check:
        for a in range(len(chain) - 1):
            if not (chain[a + 1] < chain[a]):
                raise InvariantViolation(f"I5(index) seq={seq} chain={chain}")
            if not (seq[chain[a + 1]] < seq[chain[a]]):
                raise InvariantViolation(f"I5(value) seq={seq} chain={chain}")
    return set(chain), chain, pred, tails


def lis_mask(seq, pp=partition_point_ref):
    """`HashSet` 대신 `Vec<bool>` 마스크를 돌려주는 형태 — 이관 후의 API 모양."""
    mask = [False] * len(seq)
    if not seq:
        return mask
    sel, _, _, _ = lis_indices_traced(seq, pp=pp, check=False)
    for k in sel:
        mask[k] = True
    return mask


def lis_keep_from_take_old(take_old, old_len, pp=partition_point_ref):
    """`list.rs`의 `lis_keep` 그대로 — 옛 인덱스 열에서 `keep`을 만든다."""
    retained = [t for t in take_old if t < old_len]
    sel, _, _, _ = lis_indices_traced(retained, pp=pp, check=False)
    keep = [False] * len(take_old)
    seen = 0
    for i, t in enumerate(take_old):
        if t < old_len:
            if seen in sel:
                keep[i] = True
            seen += 1
    return keep


def take_order_preserved(old_len, take_old, keep):
    """`loom_flush_core::take_order_preserved` 그대로 (2변수 양화자)."""
    for a in range(len(take_old)):
        for b in range(a + 1, len(take_old)):
            if (keep[a] and take_old[a] < old_len
                    and keep[b] and take_old[b] < old_len):
                if not (take_old[a] < take_old[b]):
                    return False
    return True


# ---------------------------------------------------------------------------
# 입력 공간
# ---------------------------------------------------------------------------


def all_permutations(nmax):
    for n in range(0, nmax + 1):
        for p in permutations(range(n)):
            yield list(p)


def all_with_duplicates():
    """중복이 **있을 수 있는** 열 — 알파벳을 좁혀 길이를 늘린다."""
    for n in range(1, 9):
        for t in product(range(2), repeat=n):
            yield list(t)
    for n in range(1, 8):
        for t in product(range(3), repeat=n):
            yield list(t)
    for n in range(1, 7):
        for t in product(range(4), repeat=n):
            yield list(t)
    # 값이 인덱스 범위 밖인 경우도 — `retained_old_indices`는 옛 인덱스지만
    # 검증 크레이트는 값 범위를 가정하지 않아야 한다.
    for n in range(1, 6):
        for t in product([0, 5, 9, 100], repeat=n):
            yield list(t)


# ---------------------------------------------------------------------------
# 1/2. 불변식 I1~I5
# ---------------------------------------------------------------------------


def check_invariants():
    perm_cnt = dup_cnt = 0
    viol = []
    for seq in all_permutations(7):
        perm_cnt += 1
        try:
            lis_indices_traced(seq)
        except InvariantViolation as e:
            viol.append(str(e))
    for seq in all_with_duplicates():
        dup_cnt += 1
        try:
            lis_indices_traced(seq)
        except InvariantViolation as e:
            viol.append(str(e))
    print(f"1/2. I1~I5 매 스텝 검사 — 전 순열(n≤7) {perm_cnt}건 + "
          f"중복 포함 {dup_cnt}건 = {perm_cnt + dup_cnt}건")
    if viol:
        print(f"     **위반 {len(viol)}건** — 최초 3건:")
        for v in viol[:3]:
            print(f"       {v}")
    else:
        print(f"     위반 0건 (I1 pred 국소성, I2 범위, I3 tails 값 엄격증가, "
              f"I4 길이 단조, I5 사슬 엄격감소)")
    return len(viol)


def check_strictness_with_duplicates():
    """중복이 있어도 **선택된 값들**이 엄격히 증가하는가 — `<` 술어의 결과."""
    bad = 0
    tot = 0
    for seq in all_with_duplicates():
        sel, chain, _, _ = lis_indices_traced(seq, check=False)
        vals = [seq[k] for k in sorted(sel)]
        tot += 1
        if any(vals[a] >= vals[a + 1] for a in range(len(vals) - 1)):
            bad += 1
            if bad <= 3:
                print(f"     반례 seq={seq} 선택={sorted(sel)} 값={vals}")
    print(f"     중복 포함 {tot}건에서 선택된 값이 엄격증가가 **아닌** 경우: {bad}건")
    return bad


# ---------------------------------------------------------------------------
# 3. `partition_point` ↔ 명시적 이분 탐색
# ---------------------------------------------------------------------------


def check_partition_point_equiv():
    """두 층에서 대조한다: (a) 호출 하나하나, (b) 최종 선택 결과."""
    call_mismatch = 0
    call_total = 0
    result_mismatch = 0
    seq_total = 0

    def pp_compare(tails, seq, val):
        nonlocal call_mismatch, call_total
        a = partition_point_ref(tails, seq, val)
        b = partition_point_binsearch(tails, seq, val)
        call_total += 1
        if a != b:
            call_mismatch += 1
            if call_mismatch <= 3:
                print(f"     호출 불일치 tails={tails} vals={[seq[t] for t in tails]} "
                      f"val={val} ref={a} bin={b}")
        return a

    space = list(all_permutations(7)) + list(all_with_duplicates())
    for seq in space:
        seq_total += 1
        ref_sel, _, _, _ = lis_indices_traced(seq, pp=pp_compare, check=False)
        bin_sel, _, _, _ = lis_indices_traced(seq, pp=partition_point_binsearch,
                                              check=False)
        if ref_sel != bin_sel:
            result_mismatch += 1
            if result_mismatch <= 3:
                print(f"     결과 불일치 seq={seq} ref={sorted(ref_sel)} "
                      f"bin={sorted(bin_sel)}")
    print(f"3. partition_point ↔ 명시적 이분 탐색: 호출 {call_total}건 중 "
          f"불일치 {call_mismatch}건, 열 {seq_total}건 중 선택 결과 불일치 "
          f"{result_mismatch}건")
    return call_mismatch + result_mismatch


# ---------------------------------------------------------------------------
# 4. HashSet ↔ Vec<bool> 마스크
# ---------------------------------------------------------------------------


def check_mask_equiv():
    bad = 0
    tot = 0
    for seq in list(all_permutations(7)) + list(all_with_duplicates()):
        tot += 1
        sel, _, _, _ = lis_indices_traced(seq, check=False)
        mask = lis_mask(seq)
        if [k for k in range(len(seq)) if mask[k]] != sorted(sel):
            bad += 1
    print(f"4. HashSet ↔ Vec<bool> 마스크: 열 {tot}건 중 불일치 {bad}건")
    return bad


# ---------------------------------------------------------------------------
# 5. take_old → retained → lis → keep → take_order_preserved (사슬 전체)
# ---------------------------------------------------------------------------


def check_P_full_chain():
    """`old_len`을 m으로 두고, `take_old[i] ∈ {0..m-1} ∪ {fresh}`를 전수.

    fresh 값은 `>= old_len`이기만 하면 `keep` 구성에 똑같이 영향하므로
    대표값 하나(m)만 쓴다. 중복을 **막지 않는다** — 검증 크레이트에
    `no_duplicates`를 걸 필요가 있는지 정하기 위해서다."""
    tot = viol = 0
    dup_tot = 0
    first_bad = None
    for m in range(0, 5):
        for n in range(0, 7):
            if (m + 1) ** n > 40000:
                continue
            for t in product(range(m + 1), repeat=n):
                take_old = list(t)
                keep = lis_keep_from_take_old(take_old, m)
                tot += 1
                retained = [x for x in take_old if x < m]
                if len(set(retained)) != len(retained):
                    dup_tot += 1
                if not take_order_preserved(m, take_old, keep):
                    viol += 1
                    if first_bad is None:
                        first_bad = (m, take_old, keep)
    print(f"5. 사슬 전체 (take_old → retained → lis → keep → P): {tot}건 "
          f"(그중 retained에 중복이 있는 것 {dup_tot}건)")
    if viol:
        print(f"   **P 위반 {viol}건** — 최소 반례 old_len={first_bad[0]} "
              f"take_old={first_bad[1]} keep={first_bad[2]}")
    else:
        print(f"   P 위반 0건 — `take_order_preserved`가 사후조건으로 성립한다")
    return viol, first_bad


def check_P_injective_only():
    """실제 호출 조건(retained가 단사 — 옛 인덱스라 중복 없음)만 따로."""
    tot = viol = 0
    for m in range(0, 6):
        for n in range(0, 7):
            if (m + 1) ** n > 60000:
                continue
            for t in product(range(m + 1), repeat=n):
                take_old = list(t)
                retained = [x for x in take_old if x < m]
                if len(set(retained)) != len(retained):
                    continue
                keep = lis_keep_from_take_old(take_old, m)
                tot += 1
                if not take_order_preserved(m, take_old, keep):
                    viol += 1
    print(f"   [retained 단사만] {tot}건 → P 위반 {viol}건")
    return viol


if __name__ == "__main__":
    v1 = check_invariants()
    v2 = check_strictness_with_duplicates()
    print()
    v3 = check_partition_point_equiv()
    print()
    v4 = check_mask_equiv()
    print()
    v5, bad = check_P_full_chain()
    v6 = check_P_injective_only()
    print()
    ok = (v1 == 0 and v2 == 0 and v3 == 0 and v4 == 0 and v5 == 0 and v6 == 0)
    if ok:
        print("결론: I1~I5는 중복 있는 열까지 매 스텝 성립하고, 명시적 이분 탐색은")
        print("      `partition_point`와 결과가 같으며, 사슬 전체가 P를 세운다.")
        print("      → 검증 크레이트에 `no_duplicates` 사전조건은 **불필요**하다.")
    else:
        print("결론: 예상과 다른 결과 — 위 수치와 반례를 확인할 것.")
