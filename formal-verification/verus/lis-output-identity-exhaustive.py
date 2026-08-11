"""출력 동일성 전수 대조: 기존 lis_indices+rank 방식 vs 새 직접순회 방식.

기존:  retained = [t for t in take_old if t < old_len]
       lis = lis_indices(retained)            # retained-인덱스 집합
       keep[i] = (rank(i) in lis)             # rank = i 이전 retained 개수

새것:  take_old를 직접 훑되 t >= old_len 슬롯은 건너뛴다.
       tails/pred가 take_old-인덱스를 담는다. keep[i] = i가 사슬에 있음.

실격조건 1번(방출 write 바이트 동일)이 걸려 있으므로 keep 벡터가
글자 그대로 같아야 한다.
"""

import itertools
import sys


def lis_indices_orig(sequence):
    """crates/loom-runtime/src/list.rs의 lis_indices 그대로."""
    if not sequence:
        return set()
    tails = []
    predecessor = [None] * len(sequence)
    for i in range(len(sequence)):
        val = sequence[i]
        # partition_point(|&ti| sequence[ti] < val) — 정의 그대로(세기)
        pos = sum(1 for ti in tails if sequence[ti] < val)
        if pos > 0:
            predecessor[i] = tails[pos - 1]
        if pos == len(tails):
            tails.append(i)
        else:
            tails[pos] = i
    result = set()
    cursor = tails[-1] if tails else None
    while cursor is not None:
        result.add(cursor)
        cursor = predecessor[cursor]
    return result


def keep_orig(take_old, old_len):
    """lis_keep 호출부 그대로."""
    retained = [t for t in take_old if t < old_len]
    lis = lis_indices_orig(retained)
    keep = [False] * len(take_old)
    retained_seen = 0
    for i, t in enumerate(take_old):
        if t < old_len:
            if retained_seen in lis:
                keep[i] = True
            retained_seen += 1
    return keep


def keep_new(take_old, old_len):
    """검증 코어로 옮긴 판본 — take_old를 직접 훑고 비-retained는 건너뛴다."""
    n = len(take_old)
    pred = [None] * n
    tails = []
    for i in range(n):
        t = take_old[i]
        if t < old_len:
            # lower_bound(take_old, tails, t) — 명시적 이분 탐색
            lo, hi = 0, len(tails)
            while lo < hi:
                mid = lo + (hi - lo) // 2
                if take_old[tails[mid]] < t:
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
    keep = [False] * n
    cursor = tails[-1] if tails else None
    while cursor is not None:
        keep[cursor] = True
        cursor = pred[cursor]
    return keep


def check(take_old, old_len, stats):
    a = keep_orig(take_old, old_len)
    b = keep_new(take_old, old_len)
    stats["n"] += 1
    if a != b:
        stats["bad"] += 1
        if stats["bad"] <= 5:
            print(f"  불일치: take_old={take_old} old_len={old_len}\n"
                  f"          orig={a}\n          new ={b}")
        return False
    if any(a):
        stats["nonempty"] += 1
    # 부수적으로 P 자체도 확인
    last = None
    for i, t in enumerate(take_old):
        if a[i] and t < old_len:
            if last is not None and t <= last:
                stats["pviol"] += 1
                return False
            last = t
    return True


def main():
    stats = {"n": 0, "bad": 0, "nonempty": 0, "pviol": 0}

    # (1) 전 순열 n<=7, old_len = n (전부 retained)
    for n in range(0, 8):
        for perm in itertools.permutations(range(n)):
            check(list(perm), n, stats)
    print(f"(1) 전 순열 n<=7 (전부 retained)        : {stats['n']:>8}건")

    m = stats["n"]
    # (2) fresh 슬롯 섞기 — take_old 값이 old_len 이상인 슬롯이 끼어든 경우.
    #     match_keys가 실제로 내는 모양(retained는 서로 다른 옛 인덱스, fresh는
    #     old_len 이상)을 전수로 만든다.
    for old_len in range(0, 5):
        for total in range(0, 6):
            # 각 슬롯: 옛 인덱스 0..old_len-1 중 하나이거나 fresh(=old_len)
            for combo in itertools.product(range(old_len + 1), repeat=total):
                take = [c if c < old_len else old_len + 7 for c in combo]
                # 실제 입력은 retained 옛 인덱스가 서로 다르다(단사) — 그
                # 케이스와 중복 케이스를 모두 돌린다.
                check(take, old_len, stats)
    print(f"(2) fresh 혼합 전수 (중복 포함)         : {stats['n'] - m:>8}건")

    m = stats["n"]
    # (3) 중복 값 열 — retained만, 알파벳 작게
    for alpha, maxlen in ((2, 9), (3, 7), (4, 6)):
        for ln in range(0, maxlen + 1):
            for seq in itertools.product(range(alpha), repeat=ln):
                check(list(seq), alpha, stats)
    print(f"(3) 중복 값 열                          : {stats['n'] - m:>8}건")

    m = stats["n"]
    # (4) 단사 retained + fresh 섞기 (실제 match_keys 출력 모양)
    for old_len in range(0, 6):
        for total in range(0, 7):
            for chosen in itertools.product(
                [None] + list(range(old_len)), repeat=total
            ):
                used = [c for c in chosen if c is not None]
                if len(set(used)) != len(used):
                    continue
                take = [old_len + i if c is None else c
                        for i, c in enumerate(chosen)]
                check(take, old_len, stats)
    print(f"(4) 단사 retained + fresh (실제 모양)   : {stats['n'] - m:>8}건")

    print()
    print(f"총 입력                : {stats['n']}건")
    print(f"keep 벡터 불일치        : {stats['bad']}건")
    print(f"P(take_order) 위반      : {stats['pviol']}건")
    print(f"비어 있지 않은 keep     : {stats['nonempty']}건")
    return 1 if (stats["bad"] or stats["pviol"]) else 0


if __name__ == "__main__":
    sys.exit(main())
