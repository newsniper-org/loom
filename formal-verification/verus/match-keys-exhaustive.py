"""`match_keys` + `current`/`target` 구성이 경계에서 무엇을 보장하는지 전수
탐색으로 확정한다 (2026-08-11, 정찰 B).

`plan_writes`(LIS 변형)는 사전조건 **P**와 **Q**에 기대고 있는데, 지금 그 둘은
`crates/loom-runtime/src/list.rs`의 `place_in_order_lis`에서 `debug_assert!`로만
지켜진다 — 릴리스 빌드에서는 사라진다. 그 짐을 검증된 크레이트로 옮기려면 먼저
**호출부가 실제로 무엇을 보장하는지**를 알아야 한다. 이 스크립트가 그것을 잰다.

`lis-keep-exhaustive.py` / `lis-convergence-exhaustive.py`와 같은 규율:
대상 코드를 파이썬으로 옮기고 작은 입력을 전수로 훑는다. **증명이 아니라
증명 대상이 참임을 뒷받침하는 실측**이다.

재는 성질 (전부 `plan_writes`가 `requires`로 요구하거나 요구하게 될 것):

  1. **Q**       `forall e in current. target_ids.contains(e)`
  2. **NoDupC**  `current@.no_duplicates()`
  3. **NoDupT**  `target_ids@.no_duplicates()`
  4. **FreshOK** `fresh[i] ==> !current.contains(target[i])`
                 `!fresh[i] ==> current.contains(target[i])`
  5. **OrderC**  `current`가 옛 DOM 순서를 보존한다(retained만 남기고 순서 유지)
  6. **Split**   retained ⊎ removed = old  (즉 `current = old − removed`가
                 정확히 retained 집합이다)
  7. **KeyOK**   (덤) `target[i]`의 키가 `new_keys[i]`와 같다 — 중복 키를 다뤄도
                 방출되는 텍스트가 맞다는 뜻
  8. **LenOK**   (덤) `target_fresh.len() == target_ids.len()`

실행: `python3 formal-verification/verus/match-keys-exhaustive.py`
"""

from itertools import product

# ---------------------------------------------------------------------------
# 대상 코드의 모형 — `crates/loom-runtime/src/list.rs`의 `match_keys`와
# 네 호출부(d_body/d_lis_body/e_body/e_lis_body)가 공통으로 하는
# `current`/`target` 구성. 네 곳은 fresh를 만드는 방식만 다르고(D는
# `RowKey`, E는 `(OrderedKey, OrderKey)`) 여기서 재는 성질은 전부 Entity
# 수준이라 한 벌로 모형화한다.
# ---------------------------------------------------------------------------

FRESH_BASE = 1000  # 새로 spawn되는 Entity id는 옛 id와 절대 겹치지 않는다


def match_keys(old_order, new_keys):
    """`match_keys` 그대로 — first-match-wins, `pool.remove`로 소비."""
    pool = list(old_order)  # `old_order.to_vec()`
    retained = []
    for i, key in enumerate(new_keys):
        pos = next((p for p, (k, _) in enumerate(pool) if k == key), None)
        if pos is not None:
            _, entity = pool.pop(pos)  # `pool.remove(pos)`
            retained.append((i, entity))
    removed = [e for (_, e) in pool]
    return retained, removed


def build(old_order, new_keys, fresh_counter, matcher=None):
    """호출부 네 곳의 공통 부분. `match_keys` 뒤에 오는 것 전부.

    `matcher`(2026-10-20 추가): `(old_order, new_keys) -> (retained, removed)`. 기본은 옛 `match_keys`이고,
    G0R1 이후의 매처(`match_by_key`)를 끼워 같은 성질 검사를 다시 돌릴 때 쓴다(sweep 4 확장)."""
    retained, removed = (matcher or match_keys)(old_order, new_keys)

    # `current`: RemoveNode가 적용된 **뒤의** DOM 순서.
    #   old_order.iter().map(|(_, e)| *e).filter(|e| !removed.contains(e))
    current = [e for (_, e) in old_order if e not in removed]

    # `retained_at: HashMap<usize, Entity> = matched.retained.collect()`
    retained_at = dict(retained)

    # `target`: Retained면 그 entity, 아니면 새로 spawn.
    target, fresh, target_keys = [], [], []
    for i, key in enumerate(new_keys):
        if i in retained_at:
            target.append(retained_at[i])
            fresh.append(False)
        else:
            target.append(FRESH_BASE + fresh_counter[0])
            fresh_counter[0] += 1
            fresh.append(True)
        target_keys.append(key)  # D는 RowKey(key), E는 OrderedKey(key) — 같다

    return {
        "retained": retained,
        "removed": removed,
        "current": current,
        "target": target,
        "fresh": fresh,
        "target_keys": target_keys,
    }


# ---------------------------------------------------------------------------
# 성질 판정 — 각각 `plan_writes`의 `requires` 한 줄에 대응한다.
# ---------------------------------------------------------------------------

def check(old_order, new_keys, r):
    """성립하지 않는 성질의 이름 목록을 돌려준다(전부 성립하면 빈 리스트)."""
    current, target, fresh = r["current"], r["target"], r["fresh"]
    bad = []

    # 1. Q — current에 target 밖 원소(extras)가 없다.
    if not all(e in target for e in current):
        bad.append("Q")

    # 2. current@.no_duplicates()
    if len(set(current)) != len(current):
        bad.append("NoDupC")

    # 3. target_ids@.no_duplicates()
    if len(set(target)) != len(target):
        bad.append("NoDupT")

    # 4. fresh 표시와 current 소속의 대응
    for i in range(len(target)):
        if fresh[i] and target[i] in current:
            bad.append("FreshOK")
            break
        if not fresh[i] and target[i] not in current:
            bad.append("FreshOK")
            break

    # 5. current가 옛 DOM 순서를 보존한다 — old_order에서 removed만 걸러낸 것과
    #    글자 그대로 같아야 한다(부분열이기만 한 것으로는 부족하다).
    kept_in_old_order = [e for (_, e) in old_order if e not in r["removed"]]
    if current != kept_in_old_order:
        bad.append("OrderC")

    # 6. retained ⊎ removed = old (다중집합으로). `current = old − removed`가
    #    정확히 retained 집합이라는 주장의 알맹이.
    old_entities = sorted(e for (_, e) in old_order)
    split = sorted([e for (_, e) in r["retained"]] + list(r["removed"]))
    if split != old_entities:
        bad.append("Split")
    if sorted(current) != sorted(e for (_, e) in r["retained"]):
        bad.append("Split=current")

    # 7. 키 대응 — target[i]가 new_keys[i]와 같은 키를 가진 행이다.
    #    (retained면 옛 행의 키, fresh면 새로 붙이는 키)
    old_key_of = {e: k for (k, e) in old_order}
    for i in range(len(target)):
        want = new_keys[i]
        got = new_keys[i] if fresh[i] else old_key_of[target[i]]
        if got != want:
            bad.append("KeyOK")
            break

    # 8. 길이 대응
    if len(fresh) != len(target):
        bad.append("LenOK")

    return bad


# ---------------------------------------------------------------------------
# 1. 정상 입력 전수 — old_order의 Entity는 서로 다르다(호출부가 그렇게 만든다)
# ---------------------------------------------------------------------------

OLD_ALPHA = "abc"        # old 쪽 키 — 중복 키가 나오도록 좁게
NEW_ALPHA = "abcd"       # new 쪽 키 — 'd'는 old에 절대 없으므로 항상 fresh
MAXLEN = 5


def sweep_wellformed():
    tally = {}
    total = dup_new = dup_old = with_fresh = with_removed = 0
    minimal = {}

    for m in range(MAXLEN + 1):
        for old_keys in product(OLD_ALPHA, repeat=m):
            # Entity id는 서로 다르다 — `rows.0` / `OrderKeyMembers.0`이
            # 중복 Entity를 담지 않는다는 호출부의 구조적 사실.
            old_order = [(k, i) for i, k in enumerate(old_keys)]
            for n in range(MAXLEN + 1):
                for new_keys in product(NEW_ALPHA, repeat=n):
                    new_keys = list(new_keys)
                    r = build(old_order, new_keys, [0])
                    total += 1
                    if len(set(new_keys)) != len(new_keys):
                        dup_new += 1
                    if len(set(old_keys)) != len(old_keys):
                        dup_old += 1
                    if any(r["fresh"]):
                        with_fresh += 1
                    if r["removed"]:
                        with_removed += 1
                    for name in check(old_order, new_keys, r):
                        tally[name] = tally.get(name, 0) + 1
                        key = (name,)
                        cand = (m + n, old_order, new_keys)
                        if key not in minimal or cand[0] < minimal[key][0]:
                            minimal[key] = cand

    print(f"1. 정상 입력(old Entity 서로 다름) 전수 — 길이 ≤{MAXLEN}, "
          f"old 키∈{{{','.join(OLD_ALPHA)}}}, new 키∈{{{','.join(NEW_ALPHA)}}}")
    print(f"   훑은 조합 {total:,}건")
    print(f"     · new_keys에 중복 키가 있는 경우      {dup_new:,}건")
    print(f"     · old_order에 중복 키가 있는 경우     {dup_old:,}건")
    print(f"     · fresh 슬롯이 생긴 경우              {with_fresh:,}건")
    print(f"     · removed(despawn)가 생긴 경우        {with_removed:,}건")
    names = ["Q", "NoDupC", "NoDupT", "FreshOK", "OrderC", "Split",
             "Split=current", "KeyOK", "LenOK"]
    for name in names:
        c = tally.get(name, 0)
        mark = "OK" if c == 0 else "위반"
        print(f"   {name:14s} 위반 {c:,}건  [{mark}]")
        if c:
            _, oo, nk = minimal[(name,)]
            print(f"       최소 반례: old_order={oo} new_keys={nk}")
    return tally


# ---------------------------------------------------------------------------
# 2. 중복 키만 따로 — "두 번째 같은 키는 fresh가 된다"가 실제로 그런지, 그때도
#    성질이 전부 서는지. 코드 주석("Phase 1's demo doesn't exercise duplicate
#    keys")이 사실이든 아니든 **코드가 무엇을 하는지**는 확정해 둔다.
# ---------------------------------------------------------------------------

def sweep_dupkeys_only():
    total = viol = 0
    tally = {}
    for m in range(MAXLEN + 1):
        for old_keys in product(OLD_ALPHA, repeat=m):
            old_order = [(k, i) for i, k in enumerate(old_keys)]
            for n in range(MAXLEN + 1):
                for new_keys in product(NEW_ALPHA, repeat=n):
                    new_keys = list(new_keys)
                    dup_new = len(set(new_keys)) != len(new_keys)
                    dup_old = len(set(old_keys)) != len(old_keys)
                    if not (dup_new or dup_old):
                        continue
                    total += 1
                    r = build(old_order, new_keys, [0])
                    bad = check(old_order, new_keys, r)
                    if bad:
                        viol += 1
                        for b in bad:
                            tally[b] = tally.get(b, 0) + 1
    print(f"\n2. **중복 키가 실제로 있는** 조합만 {total:,}건 → 성질 위반 {viol:,}건")
    if tally:
        print(f"   깨진 성질: {tally}")

    # 첫-매치-우선 + 소비의 실제 결과를 눈으로 확인할 수 있게 몇 개 찍는다.
    print("   대표 사례 (fresh 슬롯은 F로 표시):")
    samples = [
        ([("a", 0)], ["a", "a"]),
        ([("a", 0), ("a", 1)], ["a"]),
        ([("a", 0), ("a", 1)], ["a", "a"]),
        ([("a", 0), ("b", 1)], ["b", "b", "a"]),
        ([("a", 0), ("a", 1), ("b", 2)], ["b", "a", "b"]),
    ]
    for old_order, new_keys in samples:
        r = build(old_order, new_keys, [0])
        shown = [f"F{e}" if f else str(e) for e, f in zip(r["target"], r["fresh"])]
        print(f"     old={old_order} new={new_keys}")
        print(f"       → current={r['current']} removed={r['removed']} "
              f"target=[{', '.join(shown)}]")
    return viol


# ---------------------------------------------------------------------------
# 3. 적대적 입력 — old_order에 **같은 Entity가 두 번** 들어오면 어떻게 되는가.
#    호출부는 이걸 만들지 않지만, `match_keys`를 검증 크레이트로 옮길 때
#    "무엇을 `requires`로 걸어야 하는지"를 정하려면 필요한 정보다.
# ---------------------------------------------------------------------------

def sweep_hostile_dup_entities():
    tally, minimal = {}, {}
    total = 0
    for m in range(1, 4):
        for old_keys in product("ab", repeat=m):
            for ents in product(range(2), repeat=m):
                if len(set(ents)) == m:
                    continue  # 중복 Entity가 없는 것은 1번에서 이미 봤다
                old_order = list(zip(old_keys, ents))
                for n in range(0, 4):
                    for new_keys in product("abc", repeat=n):
                        new_keys = list(new_keys)
                        total += 1
                        r = build(old_order, new_keys, [0])
                        for name in check(old_order, new_keys, r):
                            tally[name] = tally.get(name, 0) + 1
                            cand = (m + n, old_order, new_keys)
                            if name not in minimal or cand[0] < minimal[name][0]:
                                minimal[name] = cand
    print(f"\n3. 적대적: old_order에 중복 **Entity** — 조합 {total:,}건")
    if not tally:
        print("   깨진 성질 없음")
    for name, c in sorted(tally.items(), key=lambda kv: -kv[1]):
        _, oo, nk = minimal[name]
        print(f"   {name:14s} 위반 {c:,}건   최소 반례: old_order={oo} new_keys={nk}")
    return tally


# ---------------------------------------------------------------------------
# 4. 귀납 — 2·3번 성질은 "old_order의 Entity가 서로 다르다"에 의존한다.
#    그 가정이 **다음 라운드에 스스로 다시 세워지는지**(target이 곧 다음
#    old_order다: D는 `rows.0 = target`, E는 `members.0 = target`이고
#    `OrderKey`가 0..n-1로 조밀 재태그되므로 정렬 결과가 target 순서다)를
#    여러 라운드 돌려 확인한다.
# ---------------------------------------------------------------------------

def sweep_multistep(steps=3):
    counter = [0]
    total = viol = nondistinct = 0
    seqs = []
    for n in range(0, 4):
        seqs.extend([list(t) for t in product("abc", repeat=n)])

    start = [("a", 0), ("b", 1)]
    for chain in product(seqs, repeat=steps):
        old_order = list(start)
        counter[0] = 0
        for new_keys in chain:
            r = build(old_order, new_keys, counter)
            total += 1
            if check(old_order, new_keys, r):
                viol += 1
            # 다음 라운드의 old_order = 이번 target (키는 new_keys 그대로)
            old_order = list(zip(r["target_keys"], r["target"]))
            if len({e for (_, e) in old_order}) != len(old_order):
                nondistinct += 1
    print(f"\n4. 다단계(초기 old=[('a',0),('b',1)], {steps}스텝) — "
          f"reconcile {total:,}회")
    print(f"   성질 위반 {viol:,}회")
    print(f"   다음 라운드 old_order의 Entity가 서로 다르지 않게 된 경우 "
          f"{nondistinct:,}회")
    return viol, nondistinct


# ---------------------------------------------------------------------------
# 5. 도달성 — `match_keys`의 주석은 "Phase 1's demo doesn't exercise duplicate
#    keys"라고 적고 있다. **그게 지금도 사실인가?**
#
#    · 트랙 N(`crates/loom-track-n/src/ops.rs::mint`)은 아이템마다 단조증가
#      카운터를 문자열 끝에 붙인다 → 키가 항상 유일하다. 초기값
#      ["a","bb","ccc"] / ["p","q","r","s"]도 서로 다르고 minted 키는 전부
#      숫자 접미사를 가지므로 초기값과도 안 겹친다. 셰링커는 op를 **삭제만**
#      하므로 중복을 만들 수 없다. → 트랙 N은 중복 키를 만들지 **않는다**.
#
#    · 그러나 `crates/loom-bench`는 만든다. POD 축의 fresh 민터가
#      `|seed| ItemId(1_000_000 + (seed as u32 % 1_000_000))` — 공역이 100만
#      뿐이고, W7(value_update)은 한 rep에서 n/10개를 뽑는다. n=10,000이면
#      1,000개를 100만 칸에 던지는 생일 문제다. 아래는 §8이 정한 실제 시드 열
#      (SHA-256(태그‖셀ID‖rep) 하위 64비트)로 그대로 재현한 것이다.
# ---------------------------------------------------------------------------

def sweep_bench_reachability():
    import hashlib

    PROTOCOL_TAG = b"RQ1-benchmark-protocol/v1.1"
    M64, M128 = (1 << 64) - 1, (1 << 128) - 1
    MULT = 0x2360ED051FC65DA44385DF649FCCF645

    class Pcg64:  # `loom-bench`의 것과 같은 XSL-RR
        def __init__(self, seed):
            self.inc = ((0xDA3E39CB94B95BDB << 1) | 1) & M128
            self.state = 0
            self.step()
            self.state = (self.state + seed) & M128
            self.step()

        def step(self):
            self.state = (self.state * MULT + self.inc) & M128

        def next(self):
            self.step()
            x = self.state
            rot = (x >> 122) & 0x3F
            xsl = ((x >> 64) ^ (x & M64)) & M64
            return ((xsl >> rot) | (xsl << ((64 - rot) & 63))) & M64

        def below(self, n):
            zone = M64 - (M64 % n) - 1
            while True:
                v = self.next()
                if v <= zone:
                    return v % n

    def derive_seed(band, cid, rep):
        # §8: SHA-256(태그 ‖ 대역 ‖ 셀ID ‖ rep)의 마지막 8바이트를 빅엔디언으로.
        h = hashlib.sha256()
        h.update(PROTOCOL_TAG); h.update(b"\x1f")
        h.update(band.encode()); h.update(b"\x1f")
        h.update(cid.encode()); h.update(b"\x1f")
        h.update(str(rep).encode())
        return int.from_bytes(h.digest()[24:32], "big")

    def value_update(n, rng, fresh):
        """`Workload::ValueUpdate` — 부분 Fisher-Yates로 k개 인덱스를 뽑고
        그 자리에 fresh 아이템을 넣는다."""
        v = list(range(n))
        k = max(n // 10, 1)
        idx = list(range(n))
        for i in range(k):
            j = i + rng.below(n - i)
            idx[i], idx[j] = idx[j], idx[i]
        for i in idx[:k]:
            v[i] = fresh(rng.next())
        return v

    pod = lambda s: f"id-{1_000_000 + (s & 0xFFFFFFFF) % 1_000_000}"
    heap = lambda s: f"new-{s}"
    cells = [
        ("ItemId(POD u32)", 100, 1, pod),
        ("ItemId(POD u32)", 1000, 1, pod),
        ("ItemId(POD u32)", 10000, 1, pod),
        ("ItemId(POD u32)", 1000, 8, pod),
        ("String(heap)", 1000, 1, heap),
    ]
    print("\n5. 도달성 — `loom-bench`의 W7(value_update) 셀에서 중복 키가 실제로")
    print("   생성되는가 (§8이 정한 시드 열 pilot 5 + warmup 50 + measure 200):")
    worst_cell = None
    for type_name, n, rb, fresh in cells:
        cid = f"{type_name}|n={n}|R{rb}|variants=false|value_update"
        tot = dup = worst = 0
        for band, reps in (("pilot", 5), ("warmup", 50), ("measure", 200)):
            for rep in range(reps):
                v = value_update(n, Pcg64(derive_seed(band, cid, rep)), fresh)
                tot += 1
                d = len(v) - len(set(v))
                if d:
                    dup += 1
                    worst = max(worst, d)
        print(f"   {cid:50s} {dup:4d}/{tot} rep ({100*dup/tot:5.1f}%) "
              f"최대 중복 {worst}개")
        if dup and (worst_cell is None or dup > worst_cell[1]):
            worst_cell = (cid, dup)
    return worst_cell


# ---------------------------------------------------------------------------
# G0R1 (2026-10-20, 개정 #12) — `crates/loom-runtime/src/list_key.rs`의 `match_by_key` 모형과 전수 동치 조사.
#
# 모형과 Rust 소스의 대응은 **한 줄씩 읽어서 맞춘 것**이지 기계가 검사한 것이 아니다(증명 범위의 한계).
# 그래서 Rust 쪽에도 같은 동치를 무작위 오라클 테스트(`match_equivalence_against_legacy_oracle`,
# 퇴화 해시 3종 × 문자열 3종)로 따로 세운다 — 이 스크립트는 그 **작은 도메인의 전수 보강**이다.
# ---------------------------------------------------------------------------

import hashlib


def keyfn_real(item):
    """운영 `row_key` 근사 — **결정적**이어야 한다. 내장 `hash(str)`는 PYTHONHASHSEED에 따라 바뀌므로 금지(검토 F3).
    blake2b(digest_size=8). 정의는 출력에 기록한다."""
    return int.from_bytes(hashlib.blake2b(item.encode(), digest_size=8).digest(), "big")


def keyfn_const(item):
    return 0


def keyfn_parity(item):
    return ord(item[0]) % 2 if item else 0


def keyfn_pair(item):
    return 0 if item in ("a", "b") else 1


KEYFNS = [
    ("blake2b64(운영 근사)", keyfn_real),
    ("const0(전부 충돌)", keyfn_const),
    ("ord(c)%2", keyfn_parity),
    ("{a,b}->0,{c,d}->1", keyfn_pair),
]


def match_by_key_model(old_items, new_items, keyfn):
    """`match_by_key`의 한 줄씩 모형. 반환: (take_old, collisions, item_cmp).

    · 안정 LSD radix 정렬 = (key, 원래 인덱스) 사전식 정렬과 같다(안정성 + 인덱스 오름차순 입력).
    · uniform 확인의 Item 비교 수를 센다(대표 비교 포함 `|옛 run|-1 + |새 run|`).
    · 충돌 run은 (Item, idx) 정렬 뒤 Item 동치류별 k번째↔k번째."""
    m, n = len(old_items), len(new_items)
    old = sorted(((keyfn(x), j) for j, x in enumerate(old_items)))
    new = sorted(((keyfn(x), i) for i, x in enumerate(new_items)))
    take_old = [m] * n
    collisions = item_cmp = 0
    a = b = 0

    def run_end(v, s):
        e = s + 1
        while e < len(v) and v[e][0] == v[s][0]:
            e += 1
        return e

    while a < m and b < n:
        ka, kb = old[a][0], new[b][0]
        if ka < kb:
            a = run_end(old, a)
            continue
        if kb < ka:
            b = run_end(new, b)
            continue
        ae, be = run_end(old, a), run_end(new, b)
        rep = old_items[old[a][1]]
        uniform = True
        for (_, j) in old[a + 1:ae]:
            item_cmp += 1
            if old_items[j] != rep:
                uniform = False
                break
        if uniform:
            for (_, i) in new[b:be]:
                item_cmp += 1
                if new_items[i] != rep:
                    uniform = False
                    break
        if uniform:
            for (_, i), (_, j) in zip(new[b:be], old[a:ae]):
                take_old[i] = j
        else:
            collisions += 1
            o = sorted((j for (_, j) in old[a:ae]), key=lambda j: (old_items[j], j))
            w = sorted((i for (_, i) in new[b:be]), key=lambda i: (new_items[i], i))
            x = y = 0
            while x < len(o) and y < len(w):
                if old_items[o[x]] < new_items[w[y]]:
                    c = old_items[o[x]]
                    while x < len(o) and old_items[o[x]] == c:
                        x += 1
                elif old_items[o[x]] > new_items[w[y]]:
                    c = new_items[w[y]]
                    while y < len(w) and new_items[w[y]] == c:
                        y += 1
                else:
                    c = old_items[o[x]]
                    xe, ye = x, y
                    while xe < len(o) and old_items[o[xe]] == c:
                        xe += 1
                    while ye < len(w) and new_items[w[ye]] == c:
                        ye += 1
                    for i, j in zip(w[y:ye], o[x:xe]):
                        take_old[i] = j
                    x, y = xe, ye
        a, b = ae, be
    return take_old, collisions, item_cmp


def legacy_take_old(old_keys, new_keys):
    """옛 `match_keys`의 출력 `(retained, removed)`를 Rust의 `take_old`(길이 n, 기본 m)로 바꾼다."""
    old_order = [(k, i) for i, k in enumerate(old_keys)]
    retained, _ = match_keys(old_order, list(new_keys))
    take_old = [len(old_keys)] * len(new_keys)
    for i, entity in retained:
        take_old[i] = entity
    return take_old


def legacy_cmp_count(old_keys, new_keys):
    """옛 매처의 키 비교 수 — `pool.iter().position(|(k,_)| k == key)`가 센 `==` 횟수."""
    pool = list(old_keys)
    c = 0
    for key in new_keys:
        pos = None
        for p, k in enumerate(pool):
            c += 1
            if k == key:
                pos = p
                break
        if pos is not None:
            pool.pop(pos)
    return c


def sweep_equivalence():
    """6. 동치 전수 — sweep 1과 같은 도메인(old 364가지 × new 1,365가지), keyfn 4개."""
    total = 0
    exist = {"dup_new": 0, "dup_old": 0, "fresh": 0, "removed": 0}
    per_fn = {name: {"mismatch": 0, "collision_runs": 0, "cases_with_collision": 0} for name, _ in KEYFNS}
    first_bad = None
    for m in range(MAXLEN + 1):
        for old_keys in product(OLD_ALPHA, repeat=m):
            for n in range(MAXLEN + 1):
                for new_keys in product(NEW_ALPHA, repeat=n):
                    want = legacy_take_old(old_keys, new_keys)
                    total += 1
                    if len(set(new_keys)) != n:
                        exist["dup_new"] += 1
                    if len(set(old_keys)) != m:
                        exist["dup_old"] += 1
                    if any(t == m for t in want):
                        exist["fresh"] += 1
                    if len(set(want) - {m}) < m:
                        exist["removed"] += 1
                    for name, fn in KEYFNS:
                        got, coll, _ = match_by_key_model(old_keys, new_keys, fn)
                        if got != want:
                            per_fn[name]["mismatch"] += 1
                            if first_bad is None:
                                first_bad = (name, old_keys, new_keys, want, got)
                        per_fn[name]["collision_runs"] += coll
                        if coll:
                            per_fn[name]["cases_with_collision"] += 1
    print(f"\n6. 동치 전수 — `match_by_key` 모형 == 옛 `match_keys` (take_old가 글자 그대로 같은가)")
    print(f"   도메인: sweep 1과 같다(길이 ≤{MAXLEN}, old 키∈{{{','.join(OLD_ALPHA)}}}, new 키∈{{{','.join(NEW_ALPHA)}}}) "
          f"× keyfn {len(KEYFNS)}개 = {total * len(KEYFNS):,}건")
    print(f"   [존재 카운트 — 먼저] 조합 {total:,}건 중 new 중복 {exist['dup_new']:,} · old 중복 {exist['dup_old']:,} · "
          f"fresh 있음 {exist['fresh']:,} · removed 있음 {exist['removed']:,}")
    bad = 0
    for name, fn in KEYFNS:
        d = per_fn[name]
        print(f"   keyfn {name:24s} 충돌 run을 탄 조합 {d['cases_with_collision']:>9,}건 (run {d['collision_runs']:,}개)  "
              f"불일치 {d['mismatch']:,}건")
        bad += d["mismatch"]
    # 공허한 통과 방지: 충돌 경로를 타는 keyfn이 실제로 충돌 run을 만들어야 한다.
    assert per_fn["const0(전부 충돌)"]["cases_with_collision"] > 100_000, "const0이 충돌 경로를 타지 않았다"
    assert per_fn["{a,b}->0,{c,d}->1"]["cases_with_collision"] > 10_000
    assert per_fn["blake2b64(운영 근사)"]["cases_with_collision"] == 0, "blake2b 8바이트가 이 도메인에서 충돌했다"
    if first_bad:
        print(f"   최소 반례 후보: {first_bad}")
    return bad


def sweep_model_bounds():
    """7. **모형 내 상한 확인**으로 한정한다(검토 F4). 길이 ≤5 도메인이라 증가율의 근거가 될 수 없고, 파이썬의 정렬 비교 수는
    Rust와 다르다. **증가율의 근거는 Rust 쪽 `list_key.rs`의 구조 단언 테스트 하나뿐이다.**"""
    # (a) 옛 모형의 역순 비교 수 = n(n+1)/2 (존재 단언 — 이차 성장이 모형에서 보인다)
    print("\n7. 모형 내 상한 확인 (증가율의 근거가 아니다 — 위 설명)")
    for n in range(1, MAXLEN + 1):
        keys = [chr(ord("a") + i) for i in range(n)] if n <= 26 else None
        got = legacy_cmp_count(keys, list(reversed(keys)))
        assert got == n * (n + 1) // 2, (n, got)
    print(f"   (a) 옛 모형: 역순 n=1..{MAXLEN}의 키 비교 수 == n(n+1)/2  [확인]")
    # (b) 새 모형: 무충돌 keyfn(blake2b)에서 Item 비교 수 ≤ n+m, 역순이면 정확히 n
    worst = 0
    exact_ok = 0
    for m in range(MAXLEN + 1):
        for old_keys in product(OLD_ALPHA, repeat=m):
            for n in range(MAXLEN + 1):
                for new_keys in product(NEW_ALPHA, repeat=n):
                    _, coll, cmp_ = match_by_key_model(old_keys, new_keys, keyfn_real)
                    assert coll == 0
                    assert cmp_ <= n + m, (old_keys, new_keys, cmp_)
                    worst = max(worst, cmp_ - (n + m))
    for n in range(1, MAXLEN + 1):
        keys = [chr(ord("a") + i) for i in range(n)]
        _, coll, cmp_ = match_by_key_model(keys, list(reversed(keys)), keyfn_real)
        assert coll == 0 and cmp_ == n, (n, coll, cmp_)
        exact_ok += 1
    print(f"   (b) 새 모형(무충돌 keyfn): Item 비교 수 ≤ n+m 전 조합 확인(여유 최소 {-worst}), "
          f"역순 n=1..{MAXLEN}에서 정확히 n  [확인 {exact_ok}건]")


def matcher_new(old_order, new_keys):
    """`build`에 끼우는 새 매처 어댑터. keyfn은 운영 근사."""
    old_items = [k for (k, _) in old_order]
    take_old, _, _ = match_by_key_model(old_items, list(new_keys), keyfn_real)
    m = len(old_order)
    retained = [(i, old_order[t][1]) for i, t in enumerate(take_old) if t < m]
    taken = {t for t in take_old if t < m}
    removed = [e for j, (_, e) in enumerate(old_order) if j not in taken]
    return retained, removed


def sweep_multistep_new(steps=3):
    """4'. sweep 4의 새 매처 판. 새 `take_old`로 만든 `target`이 다음 라운드의 `old_order`가 될 때도 Entity 유일성이 세워지는가."""
    counter = [0]
    total = viol = nondistinct = 0
    seqs = []
    for n in range(0, 4):
        seqs.extend([list(t) for t in product("abc", repeat=n)])
    start = [("a", 0), ("b", 1)]
    for chain in product(seqs, repeat=steps):
        old_order = list(start)
        counter[0] = 0
        for new_keys in chain:
            r = build(old_order, new_keys, counter, matcher=matcher_new)
            total += 1
            if check(old_order, new_keys, r):
                viol += 1
            old_order = list(zip(r["target_keys"], r["target"]))
            if len({e for (_, e) in old_order}) != len(old_order):
                nondistinct += 1
    print(f"\n4'. 다단계(새 매처, 초기 old=[('a',0),('b',1)], {steps}스텝) — reconcile {total:,}회")
    print(f"   성질 위반 {viol:,}회, 다음 라운드 old_order의 Entity가 서로 다르지 않게 된 경우 {nondistinct:,}회")
    return viol, nondistinct


if __name__ == "__main__":
    t1 = sweep_wellformed()
    v2 = sweep_dupkeys_only()
    t3 = sweep_hostile_dup_entities()
    v4, nd4 = sweep_multistep()
    reach = sweep_bench_reachability()
    # G0R1(2026-10-20) 확장 — 새 매처 `match_by_key`의 모형
    print("\n" + "=" * 78)
    print("G0R1 확장: 새 매처(radix 병합)의 모형이 옛 `match_keys`와 같은 take_old를 내는가")
    print("keyfn 정의: " + "; ".join(f"{n}" for n, _ in KEYFNS) + "  (운영 근사 = hashlib.blake2b(item, digest_size=8) 빅엔디언)")
    mism6 = sweep_equivalence()
    sweep_model_bounds()
    v4n, nd4n = sweep_multistep_new()

    print()
    if reach:
        print(f"   → 중복 키는 **도달 가능하다**({reach[0]}에서 {reach[1]}개 rep).")
        print("     `match_keys`의 \"Phase 1's demo doesn't exercise duplicate")
        print("     keys\" 주석은 트랙 N에는 맞지만 loom-bench에는 틀렸다.")
        print("     다행히 1·2번 결과대로 성질은 전부 성립한다 — 즉 이미 수집된")
        print("     측정 데이터는 무효가 아니고, 대신 검증 크레이트로 옮길 때")
        print("     **키 유일성을 `requires`로 걸면 안 된다**(그 계약은 실제로")
        print("     위반된다). 걸어야 하는 것은 Entity 유일성뿐이다.")
    print()
    ok = (not t1) and v2 == 0 and v4 == 0 and nd4 == 0 and mism6 == 0 and v4n == 0 and nd4n == 0
    if ok:
        print("결론: 호출부(old_order의 Entity가 서로 다름)에서 Q·NoDupC·NoDupT·")
        print("      FreshOK·OrderC·Split·KeyOK·LenOK가 **전부** 성립한다.")
        print("      중복 키(new/old 양쪽)가 있어도 성립한다.")
        if t3:
            print(f"      단, old_order에 중복 Entity가 있으면 "
                  f"{sorted(t3)}가 깨진다 — `match_keys`를 검증 크레이트로")
            print("      옮길 때 이것이 유일하게 필요한 `requires`다.")
    else:
        print("결론: 예상과 다른 결과 — 위 수치를 확인할 것.")
