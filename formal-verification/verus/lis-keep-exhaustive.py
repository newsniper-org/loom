"""LIS 변형의 `keep`에 대한 사전조건 P를 전수 탐색으로 확정한다 (2026-08-02).

`crates/loom-runtime/src/list.rs`의 `place_in_order_lis`는 `plan_writes`에
`keep`을 넘기고, `plan_writes`는 `keep[i] || <인접성 검사>`로 **단락**한다.
즉 `keep`이 틀리면 인접성 검사가 아예 돌지 않고 그 원소는 제자리에 남는다.

이 스크립트가 답하는 것 셋:

  1. `keep`을 없애고 인접성 검사만 써도 되는가? — **아니다**. 방출량이 크게
     다르다(swap-ends에서 2 vs 999).
  2. `keep`이 지켜야 하는 조건은 정확히 무엇인가? — **P**: 표시된 원소들의
     `current` 내 상대 순서가 target 순서와 같다.
  3. P는 충분한가, 필요한가? — **둘 다**. P를 만족하면 위반 0건, P를 위반하면
     100% 순서가 깨진다.

`plan_writes`의 수렴은 아직 **증명되지 않았다**. 왜 아직인지(필요한 불변식의
모양, 그리고 그 전에 걷어야 하는 rlimit 벽)는 이 디렉터리의 README를 볼 것.
이 스크립트는 증명이 아니라 **증명 대상이 참임을 뒷받침하는 실측**이다.

실행: `python3 formal-verification/verus/lis-keep-exhaustive.py`
"""

from itertools import combinations, permutations, product
import random

# ---------------------------------------------------------------------------
# 대상 코드의 모형 — `crates/loom-flush-core`의 `place_before`/`apply_ops`와
# 같은 의미. 여기서 재구현하는 이유는 검증기 없이 전수 탐색을 하기 위함이다.
# ---------------------------------------------------------------------------


def lis_indices(seq):
    """`list.rs`의 `lis_indices`를 그대로 옮긴 것 (patience sorting)."""
    if not seq:
        return set()
    tails, pred = [], [None] * len(seq)
    for i, val in enumerate(seq):
        # Rust의 `tails.partition_point(|&ti| seq[ti] < val)`
        lo, hi = 0, len(tails)
        while lo < hi:
            mid = (lo + hi) // 2
            if seq[tails[mid]] < val:
                lo = mid + 1
            else:
                hi = mid
        pos = lo
        if pos > 0:
            pred[i] = tails[pos - 1]
        if pos == len(tails):
            tails.append(i)
        else:
            tails[pos] = i
    res, cur = set(), (tails[-1] if tails else None)
    while cur is not None:
        res.add(cur)
        cur = pred[cur]
    return res


def place_before(dom, e, before):
    """`loom_flush_core::place_before` — anchor 해석 실패 시 끝으로 가는
    폴백 가지까지 그대로 옮겼다."""
    d = list(dom)
    if e in d:
        d.remove(e)
    if before is None:
        d.append(e)
        return d
    if before in d:
        d.insert(d.index(before), e)
    else:
        d.append(e)
    return d


def run(current, target, fresh, keep):
    """`plan_writes`의 루프 — target을 역순으로 순회하며 write를 방출/스킵."""
    dom, chain, anchor, moves = list(current), list(current), None, 0
    for i in range(len(target) - 1, -1, -1):
        e = target[i]
        if fresh[i]:
            dom = place_before(dom, e, anchor)
            chain = list(dom)
            moves += 1
        else:
            if anchor is None:
                adj = keep[i] or (len(chain) > 0 and chain[-1] == e)
            else:
                j = chain.index(e)
                adj = keep[i] or (j + 1 < len(chain) and chain[j + 1] == anchor)
            if not adj:
                dom = place_before(dom, e, anchor)
                chain = list(dom)
                moves += 1
        anchor = e
    return dom, moves


def lis_keep(current, target, fresh):
    """`place_in_order_lis`가 만드는 `keep` 벡터."""
    old_index = {e: i for i, e in enumerate(current)}
    retained_old = [old_index[t] for t, f in zip(target, fresh) if not f]
    lis = lis_indices(retained_old)
    keep, seen = [], 0
    for t, f in zip(target, fresh):
        if f:
            keep.append(False)
        else:
            keep.append(seen in lis)
            seen += 1
    return keep


def satisfies_P(current, target, fresh, keep):
    """**P**: `keep`으로 표시된 원소들의 `current` 내 상대 순서가 target 순서와 같다."""
    kept = [target[i] for i in range(len(target)) if keep[i] and not fresh[i]]
    pos = {e: i for i, e in enumerate(current)}
    return all(pos[kept[a]] < pos[kept[a + 1]] for a in range(len(kept) - 1))


def converged(dom, target):
    return dom[len(dom) - len(target):] == target


# ---------------------------------------------------------------------------
# 1. `keep`은 지워도 되는 것이 아니다
# ---------------------------------------------------------------------------

def check_keep_matters():
    diff = 0
    for n in range(1, 8):
        cur = list(range(n))
        for perm in permutations(cur):
            tgt, fr = list(perm), [False] * n
            a = run(cur, tgt, fr, lis_keep(cur, tgt, fr))[1]
            b = run(cur, tgt, fr, [False] * n)[1]
            if a != b:
                diff += 1
    print(f"1. n=1..7 전 순열 중 keep 유/무로 방출량이 다른 경우: {diff}건")

    random.seed(20260802)
    cur = list(range(1000))
    cases = {
        "shuffle": random.sample(cur, 1000),
        "swap ends": [cur[-1]] + cur[1:-1] + [cur[0]],
        "head→tail": cur[1:] + [cur[0]],
        "reverse": cur[::-1],
    }
    for label, tgt in cases.items():
        fr = [False] * 1000
        a = run(cur, tgt, fr, lis_keep(cur, tgt, fr))[1]
        b = run(cur, tgt, fr, [False] * 1000)[1]
        print(f"   n=1000 {label:11s} keep={a:4d}  인접성만={b:4d}")


# ---------------------------------------------------------------------------
# 2/3. P는 충분하고 필요하다
# ---------------------------------------------------------------------------

def check_P_sufficient_all_retained():
    checked = viol = 0
    for n in range(1, 7):
        cur = list(range(n))
        for perm in permutations(cur):
            tgt, fr = list(perm), [False] * n
            for keep in product([False, True], repeat=n):
                if not satisfies_P(cur, tgt, fr, keep):
                    continue
                checked += 1
                if not converged(run(cur, tgt, fr, list(keep))[0], tgt):
                    viol += 1
    print(f"2. [전부 retained] P 만족 조합 {checked}건 → 순서 위반 {viol}건")
    return viol


def check_P_sufficient_with_fresh():
    checked = viol = 0
    for n in range(1, 5):
        cur = list(range(n))
        for r in range(0, n + 1):
            for surv in combinations(cur, r):
                c2 = list(surv)
                for nf in range(0, 3):
                    items = list(surv) + list(range(100, 100 + nf))
                    for arr in set(permutations(range(len(items)))):
                        tgt = [items[k] for k in arr]
                        fr = [t >= 100 for t in tgt]
                        for keep in product([False, True], repeat=len(tgt)):
                            if any(keep[i] and fr[i] for i in range(len(tgt))):
                                continue
                            if not satisfies_P(c2, tgt, fr, keep):
                                continue
                            checked += 1
                            if not converged(run(c2, tgt, fr, list(keep))[0], tgt):
                                viol += 1
    print(f"   [fresh 혼합]  P 만족 조합 {checked}건 → 순서 위반 {viol}건")
    return viol


def check_P_necessary():
    tot = brk = 0
    for n in range(2, 6):
        cur = list(range(n))
        for perm in permutations(cur):
            tgt, fr = list(perm), [False] * n
            for keep in product([False, True], repeat=n):
                if satisfies_P(cur, tgt, fr, keep):
                    continue
                tot += 1
                if not converged(run(cur, tgt, fr, list(keep))[0], tgt):
                    brk += 1
    pct = 100 * brk / max(tot, 1)
    print(f"3. P 위반 조합 {tot}건 → 실제로 순서가 깨진 것 {brk}건 ({pct:.1f}%)")
    return brk, tot


# ---------------------------------------------------------------------------
# 4. `lis_indices`가 실제로 P를 세우는가 (건전성 — 최적성은 검사하지 않는다)
# ---------------------------------------------------------------------------

def check_lis_establishes_P():
    viol = 0
    for n in range(1, 7):
        cur = list(range(n))
        for perm in permutations(cur):
            tgt, fr = list(perm), [False] * n
            if not satisfies_P(cur, tgt, fr, lis_keep(cur, tgt, fr)):
                viol += 1
    print(f"4. lis_indices가 만든 keep이 P를 위반한 경우 (n=1..6 전 순열): {viol}건")
    return viol


if __name__ == "__main__":
    check_keep_matters()
    print()
    v1 = check_P_sufficient_all_retained()
    v2 = check_P_sufficient_with_fresh()
    brk, tot = check_P_necessary()
    v4 = check_lis_establishes_P()
    print()
    ok = (v1 == 0 and v2 == 0 and brk == tot and v4 == 0)
    print("결론: P는 충분조건이자 필요조건이다." if ok
          else "결론: 예상과 다른 결과 — 위 수치를 확인할 것.")
