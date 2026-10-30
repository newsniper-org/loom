"""LIS 변형(`plan_writes`)의 **수렴 사후조건**과 그 루프 불변식 후보(J1/J2)를
전수 탐색으로 정찰한다 (2026-08-09).

## 왜 이 스크립트가 있는가

`plan_writes_converging`(비-LIS 경로)은 이미 수렴을 사후조건으로 갖고 있다:

    let d = apply_ops(current@, ops@, ops@.len() as int);
    d.len() >= target_ids@.len()
    d.subrange(d.len() - target_ids@.len(), d.len() as int) =~= target_ids@

같은 것을 `keep`이 있는 LIS 변형 `plan_writes`에도 올리려 한다. 증명에 들어가기
전에 **명제가 참인지부터** 확인하는 것이 이 스크립트의 일이다.

이웃 스크립트 `lis-keep-exhaustive.py`(2026-08-02)는 사전조건 **P**
(`kept_order_preserved`)의 필요충분성을 확정했다. 다만 그 탐색은
**`current`가 target의 retained 집합과 정확히 같은 경우만** 훑었다
(`cur = list(range(n))`, `tgt = cur의 순열`; fresh 혼합 쪽도 `c2 = list(surv)`).
즉 **`current`에만 있고 target에는 없는 원소**가 남는 경우는 한 번도 시험되지
않았다. 이 스크립트는 그 구멍을 메우는 것에서 출발한다.

## 무엇을 확인하는가

  1. P만으로 수렴 사후조건이 성립하는가 (extras 허용).
  2. 문서에 적힌 불변식 후보 J1/J2가 **매 스텝** 성립하는가.
  3. J1/J2에 빠진 조건이 있는가 — 있다면 무엇으로 메우는가.
  4. 강화형 (K*)(P와 J1을 하나로 묶은 것)이 실제로 귀납적인가.
  5. 경계: target이 빈 경우, current가 빈 경우, fresh 혼합, fresh 자리의 keep.

## 무엇을 확인하지 않는가

  - `lis_indices`의 **최적성**(방출 수 최소). 이웃 스크립트와 같은 이유로
    건전성만 본다 — 최적이 아니면 이동이 더 나올 뿐 순서는 깨지지 않는다.
  - `Chain`의 맵 표현. 여기서는 표기 시퀀스만 모형화한다(Verus 쪽에서 이미
    Chain↔adjacent 대응 4정리로 잇혀 있다).
  - 성능. 전부 O(n^2) 파이썬 리스트 연산이다.
  - 이것은 **증명이 아니다**. n이 작은 범위의 실측이고, 증명 대상이 참인지와
    어떤 불변식이 그것을 지탱하는지를 가리키는 정찰이다.

실행: `python3 formal-verification/verus/lis-convergence-exhaustive.py`
"""

import importlib.util
import os
import time
from itertools import combinations, permutations, product

# ---------------------------------------------------------------------------
# 모형은 이웃 스크립트에서 그대로 가져온다 — 두 스크립트가 서로 다른 모형을
# 보고 다른 결론을 내는 사고를 원천 차단하기 위해서다. (파일명에 하이픈이 있어
# 평범한 import가 안 되므로 importlib로 적재한다. 그쪽은 `__main__` 가드가 있어
# 적재만으로 검사가 돌지는 않는다.)
# ---------------------------------------------------------------------------

_HERE = os.path.dirname(os.path.abspath(__file__))
_SPEC = importlib.util.spec_from_file_location(
    "lis_keep_exhaustive", os.path.join(_HERE, "lis-keep-exhaustive.py")
)
_BASE = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(_BASE)

run = _BASE.run                    # `plan_writes`의 루프
place_before = _BASE.place_before  # `loom_flush_core::place_before`
lis_keep = _BASE.lis_keep          # `place_in_order_lis`가 만드는 keep
satisfies_P = _BASE.satisfies_P    # 사전조건 P
converged = _BASE.converged        # 목표 사후조건


# ---------------------------------------------------------------------------
# 스텝별 상태를 뽑는 판본 — `run`과 같은 루프여야 한다(아래 0번에서 대조).
# ---------------------------------------------------------------------------

def run_trace(current, target, fresh, keep):
    """`run`과 같은 루프. 다만 매 스텝 직후의 `(i, dom)`을 함께 돌려준다.

    `states[k] = (i, dom)`은 "인덱스 i..n-1을 처리한 직후" — 즉 Verus 루프의
    `i`가 그 값일 때 불변식이 성립해야 하는 시점이다. `states[0]`은 `(n, current)`.

    `fallbacks`는 `place_before`가 anchor를 DOM에서 못 찾아 **끝으로 미는**
    폴백 가지를 탄 횟수다. Verus 쪽 불변식 (d)가 이 가지가 도달 불가임을
    말하므로, 0이 아니면 그 불변식이 거짓이라는 뜻이다.
    """
    n = len(target)
    dom = list(current)
    anchor = None
    moves = 0
    fallbacks = 0
    states = [(n, list(dom))]
    for i in range(n - 1, -1, -1):
        e = target[i]
        emit = False
        if fresh[i]:
            emit = True
        else:
            if anchor is None:
                adj = keep[i] or (len(dom) > 0 and dom[-1] == e)
            else:
                j = dom.index(e) if e in dom else -1
                adj = keep[i] or (j >= 0 and j + 1 < len(dom) and dom[j + 1] == anchor)
            emit = not adj
        if emit:
            if anchor is not None and anchor not in dom:
                fallbacks += 1
            dom = place_before(dom, e, anchor)
            moves += 1
        anchor = e
        states.append((i, list(dom)))
    return dom, moves, states, fallbacks


# ---------------------------------------------------------------------------
# 불변식 후보와 부가 조건
# ---------------------------------------------------------------------------

def J1(dom, target, i):
    """**J1**: DOM에서 `target[i..]`에 속하는 것만 뽑으면 `target[i..]` 그대로다."""
    suf = target[i:]
    s = set(suf)
    return [x for x in dom if x in s] == suf


def J2(dom, target, i):
    """**J2**: `target[i]` 뒤에 있는 모든 원소는 `target[i..]` 또는 `target[0..i-1]`에
    속한다 (= 통틀어 target에 속한다). `i == len(target)`이면 공허하게 참."""
    n = len(target)
    if i >= n:
        return True
    if target[i] not in dom:
        return False
    p = dom.index(target[i])
    allowed = set(target)
    return all(x in allowed for x in dom[p + 1:])


def Kstar(dom, target, fresh, keep, i):
    """**(K\\*)** — P와 J1을 **하나로 묶은 강화형**.

        filter(dom, KeptSet(i) ∪ target[i..]) == KeptSeq(i) ++ target[i..]

    여기서 `KeptSeq(i)`는 아직 처리되지 않은(인덱스 < i) keep·비-fresh 원소를
    **target 순서로** 늘어놓은 것.

    - `i == n`, `dom == current`이면 이것은 **정확히 P**다(아래 1번에서 기계 대조).
    - `i == 0`이면 `KeptSeq(0)`이 비어 J1(0)과 같다.

    즉 이 한 술어가 "루프 시작 시점의 P"에서 "루프 끝 시점의 J1"까지를 잇는다.
    """
    kept = [target[j] for j in range(i) if keep[j] and not fresh[j]]
    want = kept + target[i:]
    s = set(want)
    return [x for x in dom if x in s] == want


def cond_Q(current, target):
    """**Q**: `current`의 모든 원소가 target에 나타난다 (= extras 없음).

    호출부(`list.rs`의 `d_lis_body`)는 `match_keys`가 `removed`로 가른 것을
    `current`에서 걸러내므로 이것을 항상 만족한다 — 다만 `plan_writes`의
    `requires`에는 **적혀 있지 않다**.
    (2026-10-30 주기: G0R1(2026-10-20) 이후 매처는 `list_key::match_by_key`(radix 병합)이고
    반환 계약은 옛 `match_keys`와 같다 — 위 서술은 그 매처에 그대로 성립한다.)
    """
    ts = set(target)
    return all(x in ts for x in current)


def cond_P2(current, target, fresh, keep):
    """**P2** — Q보다 약한 후보: `current`에서 **첫 keep 원소 뒤쪽**에 있는 원소는
    전부 target에 속한다. (keep이 하나도 없으면 공허하게 참 — 그때는 인접성
    검사만 도는 `plan_writes_converging`과 같은 경로다.)

    동치인 다른 표현: **extras(= target에 없는 current 원소)는 전부 keep으로
    표시된 원소들보다 앞에 있다.** 아래 `cond_P2_alt`와 기계 대조한다.
    """
    ts = set(target)
    posc = {e: k for k, e in enumerate(current)}
    kept_pos = [
        posc[target[j]]
        for j in range(len(target))
        if keep[j] and not fresh[j] and target[j] in posc
    ]
    if not kept_pos:
        return True
    m = min(kept_pos)
    return all(x in ts for x in current[m + 1:])


def cond_P2_alt(current, target, fresh, keep):
    """P2의 2변수 표현 — 모든 extra가 모든 kept 원소보다 앞에 있다."""
    ts = set(target)
    posc = {e: k for k, e in enumerate(current)}
    extras = [posc[x] for x in current if x not in ts]
    kept = [
        posc[target[j]]
        for j in range(len(target))
        if keep[j] and not fresh[j] and target[j] in posc
    ]
    return all(px < pk for px in extras for pk in kept)


def suffix_inv(dom, target, i):
    """`plan_writes_converging`이 쓰는 **접미사 불변식** — DOM의 뒤쪽 (n-i)개가
    `target[i..]`와 같다. LIS 경로에서는 중간에 깨진다는 것이 문서의 주장이고,
    아래 6번이 그것을 실측으로 못박는다(음성 대조: 새 불변식이 왜 필요한가)."""
    n = len(target)
    k = n - i
    if len(dom) < k:
        return False
    return dom[len(dom) - k:] == target[i:] if k > 0 else True


def exactly_target(dom, target):
    """수렴보다 **강한** 결론: DOM이 통째로 target과 같다."""
    return dom == target


# ---------------------------------------------------------------------------
# 탐색 공간
# ---------------------------------------------------------------------------

def cases(max_current, max_fresh, max_n):
    """`(current, target, fresh)` 삼중쌍을 훑는다.

    id를 바꿔 붙이는 것은 결과에 영향이 없으므로 `current = [0..c)`로 고정하고,
    retained 집합 `R ⊆ current`와 fresh id `100, 101, ...`을 섞은 뒤 전 순열을
    target으로 삼는다. `current \\ R`가 **extras** — 이웃 스크립트가 한 번도
    시험하지 않은 축이다.
    """
    for c in range(0, max_current + 1):
        cur = list(range(c))
        for r in range(0, c + 1):
            for R in combinations(cur, r):
                for nf in range(0, max_fresh + 1):
                    n = r + nf
                    if n > max_n:
                        continue
                    items = list(R) + [100 + k for k in range(nf)]
                    for perm in permutations(items):
                        tgt = list(perm)
                        fr = [t >= 100 for t in tgt]
                        yield cur, tgt, fr


def keeps(n, allow_keep_on_fresh, fresh):
    for k in product([False, True], repeat=n):
        if not allow_keep_on_fresh and any(k[i] and fresh[i] for i in range(n)):
            continue
        yield list(k)


# ---------------------------------------------------------------------------
# 0. 모형 일치 — `run_trace`가 정말 `run`과 같은 루프인가
# ---------------------------------------------------------------------------

def check_model_agrees(space):
    bad = 0
    total = 0
    for cur, tgt, fr in space:
        for kp in keeps(len(tgt), True, fr):
            d1, m1 = run(cur, tgt, fr, kp)
            d2, m2, _, _ = run_trace(cur, tgt, fr, kp)
            total += 1
            if d1 != d2 or m1 != m2:
                bad += 1
    print(f"0. run_trace ≡ run : {total}건 중 불일치 {bad}건")
    return bad


# ---------------------------------------------------------------------------
# 1. (K*)(n)이 정말 P인가 — 강화형이 P를 진짜로 포함하는지 기계 대조
# ---------------------------------------------------------------------------

def check_kstar_is_P(space):
    bad = 0
    total = 0
    for cur, tgt, fr in space:
        for kp in keeps(len(tgt), False, fr):
            total += 1
            if Kstar(cur, tgt, fr, kp, len(tgt)) != satisfies_P(cur, tgt, fr, kp):
                bad += 1
    print(f"1. (K*)(n) ≡ P (dom = current) : {total}건 중 불일치 {bad}건")
    return bad


# ---------------------------------------------------------------------------
# 2~5. 본 검사 — 한 번의 순회에서 전부 집계한다
# ---------------------------------------------------------------------------

class Tally:
    def __init__(self):
        self.n_P = 0                 # P를 만족하는 조합 수
        self.n_PQ = 0                # P ∧ Q
        self.n_PnotQ = 0             # P ∧ ¬Q
        self.conv_fail_P = []        # P만: 수렴 실패
        self.conv_fail_PQ = []       # P ∧ Q: 수렴 실패
        self.exact_fail_PQ = []      # P ∧ Q: dom == target 실패
        self.j1_fail_P = []          # P만: J1이 어느 스텝에서 깨짐
        self.j2_fail_P = []          # P만: J2가 어느 스텝에서 깨짐
        self.j1_fail_PQ = []
        self.j2_fail_PQ = []
        self.kstar_fail_P = []       # P만: (K*)가 어느 스텝에서 깨짐
        self.fallback_P = []         # place_before의 anchor-없음 폴백이 도달
        self.p2_fail = []            # P ∧ P2인데 수렴 실패
        self.p2_weaker = 0           # P ∧ P2 ∧ ¬Q 이면서 수렴 성공 (Q가 최소가 아님)
        self.p2_disagree = 0         # P2의 두 표현이 갈리는 경우
        self.j1j2_but_not_conv = []  # J1(0) ∧ J2(0) ∧ nodup 인데 수렴 실패
        self.suffix_broken_PQ = []   # P ∧ Q인데도 접미사 불변식이 깨진 스텝
        self.dom_escapes_target_PQ = []  # P ∧ Q인데 dom에 target 밖 원소가 있는 스텝


def size(rec):
    cur, tgt = rec[0], rec[1]
    return (len(cur) + len(tgt), len(cur), len(tgt))


def sweep(space, tally):
    for cur, tgt, fr in space:
        n = len(tgt)
        q = cond_Q(cur, tgt)
        for kp in keeps(n, True, fr):
            if not satisfies_P(cur, tgt, fr, kp):
                continue
            tally.n_P += 1
            dom, _, states, fallbacks = run_trace(cur, tgt, fr, kp)
            conv = converged(dom, tgt)
            rec = (list(cur), list(tgt), list(fr), list(kp), list(dom))

            if fallbacks:
                tally.fallback_P.append(rec)

            # --- 스텝별 불변식 ---
            j1_bad = j2_bad = ks_bad = suf_bad = esc_bad = None
            ts = set(tgt)
            for (i, d) in states:
                if j1_bad is None and not J1(d, tgt, i):
                    j1_bad = (i, list(d))
                if j2_bad is None and not J2(d, tgt, i):
                    j2_bad = (i, list(d))
                if ks_bad is None and not Kstar(d, tgt, fr, kp, i):
                    ks_bad = (i, list(d))
                if suf_bad is None and not suffix_inv(d, tgt, i):
                    suf_bad = (i, list(d))
                if esc_bad is None and any(x not in ts for x in d):
                    esc_bad = (i, list(d))
            if j1_bad:
                tally.j1_fail_P.append(rec + (j1_bad,))
            if j2_bad:
                tally.j2_fail_P.append(rec + (j2_bad,))
            if ks_bad:
                tally.kstar_fail_P.append(rec + (ks_bad,))

            # --- 수렴 ---
            if not conv:
                tally.conv_fail_P.append(rec)
            if q:
                tally.n_PQ += 1
                if not conv:
                    tally.conv_fail_PQ.append(rec)
                if not exactly_target(dom, tgt):
                    tally.exact_fail_PQ.append(rec)
                if j1_bad:
                    tally.j1_fail_PQ.append(rec + (j1_bad,))
                if j2_bad:
                    tally.j2_fail_PQ.append(rec + (j2_bad,))
                if suf_bad:
                    tally.suffix_broken_PQ.append(rec + (suf_bad,))
                if esc_bad:
                    tally.dom_escapes_target_PQ.append(rec + (esc_bad,))
            else:
                tally.n_PnotQ += 1

            # --- Q를 P2로 약화할 수 있는가 ---
            if cond_P2(cur, tgt, fr, kp) != cond_P2_alt(cur, tgt, fr, kp):
                tally.p2_disagree += 1
            if cond_P2(cur, tgt, fr, kp):
                if not conv:
                    tally.p2_fail.append(rec)
                elif not q:
                    tally.p2_weaker += 1

            # --- i=0 추출 규칙이 실제로 성립하는가 ---
            if n > 0 and J1(dom, tgt, 0) and J2(dom, tgt, 0) \
               and len(set(dom)) == len(dom) and not conv:
                tally.j1j2_but_not_conv.append(rec)


# ---------------------------------------------------------------------------
# 6. i=0 추출 규칙의 **추상** 검사 — 도달 가능한 상태만이 아니라
#    임의의 (dom, target) 쌍에서 J1(0) ∧ J2(0) ∧ 무중복 ⇒ 수렴 인가
# ---------------------------------------------------------------------------

def check_extraction_rule(max_dom=6, max_tgt=4, label="7"):
    checked = bad = 0
    pool = list(range(max_dom))
    for dl in range(1, max_dom + 1):
        for dom in permutations(pool, dl):
            dom = list(dom)
            for tl in range(1, min(max_tgt, dl) + 1):
                for tgt in permutations(dom, tl):
                    tgt = list(tgt)
                    if not (J1(dom, tgt, 0) and J2(dom, tgt, 0)):
                        continue
                    checked += 1
                    if not converged(dom, tgt):
                        bad += 1
                        if bad == 1:
                            print(f"   반례: dom={dom} target={tgt}")
    print(f"{label}. J1(0) ∧ J2(0) ∧ 무중복 ⇒ 수렴 : {checked}건 검사, 반증 {bad}건")
    return bad


# ---------------------------------------------------------------------------
# 7. J1 단독은 귀납적이지 않다 — P를 빼면 J1(i)는 성립하는데 J1(i-1)이 깨진다
# ---------------------------------------------------------------------------

def check_J1_not_inductive_without_P(max_n=4):
    witness = None
    count = 0
    for n in range(1, max_n + 1):
        cur = list(range(n))
        for perm in permutations(cur):
            tgt, fr = list(perm), [False] * n
            for kp in product([False, True], repeat=n):
                kp = list(kp)
                if satisfies_P(cur, tgt, fr, kp):
                    continue
                _, _, states, _ = run_trace(cur, tgt, fr, kp)
                for a in range(len(states) - 1):
                    i0, d0 = states[a]
                    i1, d1 = states[a + 1]
                    if J1(d0, tgt, i0) and not J1(d1, tgt, i1):
                        count += 1
                        if witness is None:
                            witness = (cur, tgt, kp, i0, d0, i1, d1)
                        break
    print(f"8. ¬P에서 'J1(i)는 참인데 J1(i-1)이 거짓'인 스텝이 있는 입력: {count}건")
    if witness:
        cur, tgt, kp, i0, d0, i1, d1 = witness
        print(f"   최소 증인: current={cur} target={tgt} keep={kp}")
        print(f"             J1({i0}) 참 (dom={d0}) → J1({i1}) 거짓 (dom={d1})")
    return count


# ---------------------------------------------------------------------------
# 8. LIS가 만든 keep으로 실제 호출 경로를 재현 (Q가 성립하는 쪽/아닌 쪽)
# ---------------------------------------------------------------------------

def check_keep_on_fresh_irrelevant(max_current=4, max_fresh=2, max_n=5):
    """`plan_writes`는 `if fresh { .. }`를 먼저 보므로 fresh 자리의 `keep[i]`는
    읽지 않는다 — 그러니 그 비트를 뒤집어도 결과가 같아야 한다. P의 정의가
    fresh를 비교 대상에서 빼는 것과 짝을 이루는 조항이라 실측으로 못박는다."""
    checked = diff = 0
    for cur, tgt, fr in cases(max_current, max_fresh, max_n):
        n = len(tgt)
        for kp in keeps(n, False, fr):          # fresh 자리는 전부 False
            base = run_trace(cur, tgt, fr, kp)[0]
            for i in range(n):
                if not fr[i]:
                    continue
                flipped = list(kp)
                flipped[i] = True
                checked += 1
                if run_trace(cur, tgt, fr, flipped)[0] != base:
                    diff += 1
    print(f"11. fresh 자리의 keep 비트를 뒤집어 결과가 달라진 경우: {checked}건 중 {diff}건")
    return diff


def check_boundaries():
    """경계 — target이 빈 경우, current가 빈 경우."""
    bad = 0
    n_empty_t = n_empty_c = 0
    for cur, tgt, fr in cases(4, 2, 5):
        for kp in keeps(len(tgt), True, fr):
            if not satisfies_P(cur, tgt, fr, kp):
                continue
            if len(tgt) == 0:
                n_empty_t += 1
                dom = run_trace(cur, tgt, fr, kp)[0]
                if not converged(dom, tgt) or dom != cur:
                    bad += 1
            if len(cur) == 0:
                n_empty_c += 1
                dom = run_trace(cur, tgt, fr, kp)[0]
                # current가 비면 target은 전부 fresh여야 한다(기존 requires).
                if all(fr) and dom != tgt:
                    bad += 1
    print(f"12. 경계 — target 빈 조합 {n_empty_t}건 / current 빈 조합 {n_empty_c}건 → 위반 {bad}건")
    return bad


def check_real_caller_shape(max_current=5, max_fresh=2, max_n=6):
    """호출부는 `current`를 retained 집합으로 걸러 넘기므로 Q가 성립한다.
    그 모양에서 LIS가 만든 keep으로 돌렸을 때 수렴하는지 본다."""
    checked = viol = 0
    for cur, tgt, fr in cases(max_current, max_fresh, max_n):
        retained = [t for t, f in zip(tgt, fr) if not f]
        cur2 = [x for x in cur if x in set(retained)]   # 호출부의 필터와 같은 것
        kp = lis_keep(cur2, tgt, fr)
        if not satisfies_P(cur2, tgt, fr, kp):
            print(f"   !! LIS가 P를 위반: current={cur2} target={tgt}")
            viol += 1
            continue
        checked += 1
        dom, _, _, _ = run_trace(cur2, tgt, fr, kp)
        if not converged(dom, tgt) or dom != tgt:
            viol += 1
            if viol <= 3:
                print(f"   !! 수렴 실패: current={cur2} target={tgt} keep={kp} dom={dom}")
    print(f"10. 호출부 모양(current = retained 집합) + LIS keep : {checked}건 → 위반 {viol}건")
    return viol


# ---------------------------------------------------------------------------

def show_documented_example():
    """문서(`plan_writes` 수렴 과제서)에 적힌 예를 그대로 재생한다:
    `current=[A,B,C]`, `target=[A,C,B]`, LIS가 {A,B}를 표시.
    접미사 불변식은 i=2에서 깨지지만 J1/(K*)는 버틴다 — 그것을 눈으로 본다."""
    A, B, C = 0, 1, 2
    cur, tgt, fr = [A, B, C], [A, C, B], [False] * 3
    kp = lis_keep(cur, tgt, fr)
    dom, moves, states, _ = run_trace(cur, tgt, fr, kp)
    print(f"6. 문서의 예 — current={cur} target={tgt} keep={kp} (LIS 산출), 방출 {moves}회")
    for (i, d) in states:
        print(f"     i={i}  dom={d}"
              f"  접미사={'참' if suffix_inv(d, tgt, i) else '**거짓**'}"
              f"  J1={'참' if J1(d, tgt, i) else '**거짓**'}"
              f"  J2={'참' if J2(d, tgt, i) else '**거짓**'}"
              f"  K*={'참' if Kstar(d, tgt, fr, kp, i) else '**거짓**'}")
    print(f"     최종 dom={dom}  수렴={'참' if converged(dom, tgt) else '**거짓**'}")


def report(name, recs, limit=3):
    if not recs:
        print(f"   {name}: 0건")
        return
    total = len(recs)
    seen, uniq = set(), []
    for r in sorted(recs, key=size):
        k = repr(r)
        if k not in seen:
            seen.add(k)
            uniq.append(r)
    recs = uniq
    print(f"   {name}: {total}건(중복 제외 {len(recs)}건) — 최소형:")
    for r in recs[:limit]:
        cur, tgt, fr, kp, dom = r[0], r[1], r[2], r[3], r[4]
        extra = f"  {r[5]}" if len(r) > 5 else ""
        print(f"     current={cur} target={tgt} fresh={fr} keep={kp} → dom={dom}{extra}")


def main():
    t0 = time.time()

    small = list(cases(3, 1, 4))
    print(f"[탐색 공간 A] current ≤ 3, fresh ≤ 1, n ≤ 4 : (current,target,fresh) {len(small)}쌍")
    check_model_agrees(small)
    check_kstar_is_P(small)
    print()

    tally = Tally()
    spaces = [(4, 2, 6), (5, 2, 6), (6, 1, 6), (6, 0, 6)]
    for (c, f, n) in spaces:
        sweep(cases(c, f, n), tally)
    print("[탐색 공간 B] (current ≤ 4, fresh ≤ 2) / (≤ 5, ≤ 2) / (≤ 6, ≤ 1) / (≤ 6, 0), 모두 n ≤ 6")
    print(f"   P를 만족하는 (current,target,fresh,keep) 조합: {tally.n_P}건")
    print(f"     그중 Q(extras 없음) 성립 {tally.n_PQ}건 / 위반 {tally.n_PnotQ}건")
    print()

    print("2. 목표 사후조건 — P **만으로** 성립하는가")
    report("P만: 수렴 실패", tally.conv_fail_P)
    print()

    print("3. 목표 사후조건 — P ∧ Q 에서")
    report("P∧Q: 수렴 실패", tally.conv_fail_PQ)
    report("P∧Q: dom == target(더 강한 형태) 실패", tally.exact_fail_PQ)
    print()

    print("4. 문서에 적힌 J1/J2가 매 스텝 루프 불변식인가")
    report("P만: J1이 깨진 스텝", tally.j1_fail_P)
    report("P만: J2가 깨진 스텝", tally.j2_fail_P)
    report("P∧Q: J1이 깨진 스텝", tally.j1_fail_PQ)
    report("P∧Q: J2가 깨진 스텝", tally.j2_fail_PQ)
    report("P∧Q: dom에 target 밖 원소가 있는 스텝 (있으면 J2가 독립 조항으로 필요)",
           tally.dom_escapes_target_PQ)
    print()

    print("5. 강화형 (K*) — P와 J1을 하나로 묶은 것")
    report("P만: (K*)가 깨진 스텝", tally.kstar_fail_P)
    print()

    show_documented_example()
    print("   [음성 대조] 접미사 불변식은 LIS 경로에서 실제로 못 쓴다:")
    report("P∧Q인데 접미사 불변식이 깨진 스텝", tally.suffix_broken_PQ)
    print()

    check_extraction_rule()
    print()
    check_J1_not_inductive_without_P()
    print()

    print("9. 부가 조건의 최소성 / anchor 폴백")
    print(f"   P2의 두 표현(첫-kept-뒤 / 모든-extra가-모든-kept-앞)이 갈린 경우:"
          f" {tally.p2_disagree}건")
    report("P ∧ P2 인데 수렴 실패", tally.p2_fail)
    print(f"   P ∧ P2 ∧ ¬Q 이면서 수렴 성공: {tally.p2_weaker}건"
          f"  (>0 이면 Q는 충분하지만 **최소는 아니다**)")
    report("place_before의 anchor-없음 폴백에 도달", tally.fallback_P)
    report("J1(0) ∧ J2(0) ∧ 무중복 인데 수렴 실패 (도달 가능 상태)",
           tally.j1j2_but_not_conv)
    print()

    check_real_caller_shape()
    check_keep_on_fresh_irrelevant()
    check_boundaries()
    print()

    ok_claim = not tally.conv_fail_PQ and not tally.exact_fail_PQ
    ok_j = not tally.j1_fail_PQ and not tally.j2_fail_PQ
    print("── 결론 ──")
    print(f"  P만으로 수렴: {'성립' if not tally.conv_fail_P else '**반증됨**'}")
    print(f"  P ∧ Q 로 수렴: {'성립' if ok_claim else '**반증됨**'}")
    print(f"  J1 (P만): {'매 스텝 성립' if not tally.j1_fail_P else '**깨짐**'}")
    print(f"  J2 (P만): {'매 스텝 성립' if not tally.j2_fail_P else '**깨짐**'}")
    print(f"  J1·J2 (P ∧ Q): {'매 스텝 성립' if ok_j else '**깨짐**'}")
    print(f"  (K*) (P만): {'매 스텝 성립' if not tally.kstar_fail_P else '**깨짐**'}")
    print(f"  소요 {time.time() - t0:.1f}초")


if __name__ == "__main__":
    main()
