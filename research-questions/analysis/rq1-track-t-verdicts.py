"""RQ1-benchmark-protocol.md §9 B/F 조항 **전부**를 트랙 T v2 캠페인에 기계 적용.

이전의 `rq1-track-t-bf-gate.py`는 "또는" 분기만 적용할 수 있었다 — F 3변형이
구현 자체가 없었기 때문이다. v2 캠페인(2026-08-05~06)은 6 arm 전부 × 통상
구간 × 변경률 축 × 4× 스로틀 × catch-up 축을 §8 표본 규정(셀당 10시행 ×
60초, 시행마다 fresh page load)대로 수집했으므로 이제 전 조항이 적용 가능하다.

## 판정 셀 집합 — 사전 해석을 먼저 못박는다

§9 1차 분기의 통상 구간은 f∈[0.1,10] × s∈[1,100]이고, §4.1의 attached 변경률
축(v1.1 — B-2)은 **s∈{100,1000} 행**에 사전 등록됐다. 따라서:

  - s∈{1,10} 행: rate=0이 정본 조건 (rate>0 수집분은 프로파일 병기)
  - s=100 행: rate∈{0,10,100} 각각이 정본 조건 (그 행에만 축이 등록됐으므로)

→ 판정 조건 = 6개((f,s∈{1,10}) × rate=0) + 9개((f,100) × 3 rate) = 15개.
"셀"은 (f,s) 9개이고, 어느 판정 조건에서든 이기면 그 셀에서 이긴 것이다.

## 방법론 이탈 명시 (숨기지 않음)

§8 판정 어휘는 "시드-쌍대(paired) 부트스트랩"인데, 그 쌍대의 전제(시드 열
전 후보 공유)는 트랙 L의 것이다. 트랙 T는 시행마다 fresh page이고 arm 간에
공유되는 난수열이 없어 **짝지을 근거 자체가 없다** — 시행 인덱스로 짝지으면
트랙 L에서 지적된 것과 같은 무의미한 짝짓기가 된다. 따라서 여기서는
**비쌍대(unpaired) 부트스트랩**(양쪽을 독립 리샘플)을 쓰고, 이 이탈을 §11-2에
기록할 것을 출력에 명시한다. 비쌍대는 CI가 넓어지는 방향(보수적)이다.

표본 단위: J-a = 시행별 toggle_on median(시행 10개 → 표본 10개),
J-b = 시행별 j_b_total_script_ms(표본 10개).

실행: `python3 research-questions/analysis/rq1-track-t-verdicts.py`
"""

import json
import random
import statistics

DATA = "/home/ybi/loom-research/research-questions/data"
N_RESAMPLES = 10_000
CI = 0.99
EQ_LO, EQ_HI = 1 / 1.1, 1.1
random.seed(20260806)

NORMAL_F = (0.1, 1.0, 10.0)
NORMAL_S = (1, 10, 100)
RATES = (0, 10, 100)
F_VARIANTS = ("F-i", "F-ii", "F-iii")
ALL_ARMS = ("B", "F", "F-i", "F-ii", "F-iii", "lazy")

# §11 개정 #1 — "또는" 분기의 판정 셀.
OR_BRANCH_SIZE = 100
OR_THRESHOLD_MS = 1.0


def load(rate):
    return json.load(open(f"{DATA}/rq1-track-t-v2-normal-rate{rate}-chromium.json"))["records"]


def pct(xs, p):
    k = (len(xs) - 1) * p
    f = int(k)
    c = min(f + 1, len(xs) - 1)
    return xs[f] if f == c else xs[f] * (c - k) + xs[c] * (k - f)


def unpaired_ratio_ci(a, b, ci=CI):
    """median(a)/median(b)의 비쌍대 부트스트랩 CI (모듈 docstring의 이탈 명시)."""
    ratios = []
    for _ in range(N_RESAMPLES):
        am = statistics.median(random.choices(a, k=len(a)))
        bm = statistics.median(random.choices(b, k=len(b)))
        if bm > 0:
            ratios.append(am / bm)
    if not ratios:
        return float("nan"), float("nan")
    ratios.sort()
    lo_p = (1 - ci) / 2
    return pct(ratios, lo_p), pct(ratios, 1 - lo_p)


def verdict(lo, hi):
    if hi < 1:
        return "우세"
    if lo > 1:
        return "열세"
    if lo >= EQ_LO and hi <= EQ_HI:
        return "등가"
    return "비결정"


def arm_samples(records, arm, f_hz, size):
    """(J-a 표본 10개, J-b 표본 10개, 은닉 CPU 표본 10개)."""
    rs = [r for r in records
          if r["arm"] == arm and r["size"] == size and abs(r["f_hz"] - f_hz) < 1e-9]
    ja = [statistics.median(r["toggle_on_ms"]) for r in rs if r["toggle_on_ms"]]
    jb = [r["j_b_total_script_ms"] for r in rs]
    hid = [r["residency_hidden_ms"] for r in rs]
    return ja, jb, hid


def cell_beats(records, x, y, f_hz, size):
    """§9 판정 지표: "X가 Y를 이긴다" = 그 셀에서 J-a·J-b **모두 우세**.
    반환: (이겼는가, J-a 어휘, J-b 어휘)."""
    xa, xb, _ = arm_samples(records, x, f_hz, size)
    ya, yb, _ = arm_samples(records, y, f_hz, size)
    if not xa or not ya:
        return False, "미계측", "미계측"
    va = verdict(*unpaired_ratio_ci(xa, ya))
    vb = verdict(*unpaired_ratio_ci(xb, yb))
    return (va == "우세" and vb == "우세"), va, vb


def judgment_conditions():
    """모듈 docstring의 판정 조건 15개: (rate, f, s)."""
    out = []
    for f in NORMAL_F:
        for s in NORMAL_S:
            if s == 100:
                out.extend((rate, f, s) for rate in RATES)
            else:
                out.append((0, f, s))
    return out


def main():
    print("RQ1 §9 B/F 판정 — 트랙 T v2 캠페인 (Chromium 1차)")
    print("방법론 이탈: 비쌍대 부트스트랩(트랙 T에는 쌍대의 전제가 없음 — §11-2 기록 대상)\n")

    by_rate = {rate: load(rate) for rate in RATES}

    # ---------------------------------------------------------------- 무결성
    print("=" * 78)
    print("무결성 점검")
    print("=" * 78)
    problems = []
    for rate, recs in by_rate.items():
        n = len(recs)
        iso = sum(1 for r in recs if not r["cross_origin_isolated"])
        if n != 540:
            problems.append(f"rate={rate}: 레코드 {n} ≠ 540")
        if iso:
            problems.append(f"rate={rate}: 격리 미적용 {iso}건")
    arms_seen = {r["arm"] for recs in by_rate.values() for r in recs}
    if arms_seen != set(ALL_ARMS):
        problems.append(f"arm 집합 불일치: {sorted(arms_seen)}")
    print("  " + ("문제 없음" if not problems else "\n  ".join(problems)))

    # ------------------------------------------------- F 기각 1차 분기
    print()
    print("=" * 78)
    print("§9 F 기각 1차 분기 — 변형별 'B를 이기는 셀'(J-a·J-b 모두 우세) 지도")
    print("=" * 78)
    conditions = judgment_conditions()
    wins = {v: [] for v in F_VARIANTS}
    for v in F_VARIANTS:
        print(f"\n  [{v}]")
        for (rate, f, s) in conditions:
            beat, va, vb = cell_beats(by_rate[rate], v, "B", f, s)
            mark = "★승" if beat else "  "
            print(f"    {mark} f={f:<4} s={s:<5} rate={rate:<3}  J-a={va:4s} J-b={vb:4s}")
            if beat:
                wins[v].append((rate, f, s))
        n_cells = len({(f, s) for (_, f, s) in wins[v]})
        fired = n_cells == 0
        print(f"    ⇒ B-승리 셀 {n_cells}/9  →  1차 분기 {'**발동(기각)**' if fired else '미발동'}")

    # ------------------------------------------------- "또는" 분기
    print()
    print("=" * 78)
    print('§9 "또는" 분기 — B spawn < 1ms/100 entity (s=100, 개정 #1), 양쪽 스로틀')
    print("=" * 78)
    thr = json.load(open(f"{DATA}/rq1-track-t-v2-throttled4x-chromium.json"))["records"]
    sides = {}
    for label, recs in [("비스로틀(rate=0)", by_rate[0]), ("4× 스로틀", thr)]:
        cell_medians = []
        for f in NORMAL_F:
            rs = [r for r in recs if r["arm"] == "B" and r["size"] == OR_BRANCH_SIZE
                  and abs(r["f_hz"] - f) < 1e-9]
            ons = [statistics.median(r["toggle_on_ms"]) for r in rs if r["toggle_on_ms"]]
            if ons:
                m = statistics.median(ons)
                cell_medians.append((f, m))
                print(f"  {label:16s} f={f:<4} B 토글-on median = {m:.3f}ms")
        sides[label] = all(m < OR_THRESHOLD_MS for _, m in cell_medians) and bool(cell_medians)
    or_fired = all(sides.values())
    print(f"  ⇒ 비스로틀 {'성립' if sides.get('비스로틀(rate=0)') else '불성립'}, "
          f"4× 스로틀 {'성립' if sides.get('4× 스로틀') else '불성립'}"
          f" → \"또는\" 분기 {'**발동** (결론 = B 유지)' if or_fired else '미발동'}")

    # ------------------------------------------------- CSS 처리 규칙
    print()
    print("=" * 78)
    print("§9 CSS 은닉 처리 규칙 (K-3)")
    print("=" * 78)
    # (1) CSS가 F 3변형 전부를 전 구간·전 판정 항목에서 지배하는가.
    #     판정 항목 집합 {J-a, J-b, 동결/은닉 중 누적 CPU(M6), 상주 메모리(M5)}
    #     — M5는 이 캠페인이 수집하지 않으므로(드라이버 known_gaps) 규칙 (1)은
    #     **완전 적용 불가**. 나머지 세 항목으로 "부분 판정"만 내고 그 사실을
    #     명시한다 — 세 항목에서 이미 지배가 깨지면 M5 없이도 결론이 난다.
    print("  (1) 지배 검사 — M5(상주 메모리) 미수집으로 **부분 적용**(J-a/J-b/은닉 CPU만).")
    dominated_all = True
    for v in F_VARIANTS:
        broken = None
        for (rate, f, s) in conditions:
            recs = by_rate[rate]
            ca, cb, ch = arm_samples(recs, "F", f, s)
            va_, vb_, vh = arm_samples(recs, v, f, s)
            if not ca or not va_:
                continue
            for (name, cx, vx) in (("J-a", ca, va_), ("J-b", cb, vb_), ("은닉CPU", ch, vh)):
                lo, hi = unpaired_ratio_ci(cx, vx)
                if verdict(lo, hi) not in ("우세", "등가"):
                    broken = (name, rate, f, s, verdict(lo, hi))
                    break
            if broken:
                break
        if broken:
            dominated_all = False
            print(f"    CSS vs {v}: 지배 깨짐 — {broken[0]} @ f={broken[2]} s={broken[3]} "
                  f"rate={broken[1]} ({broken[4]})")
        else:
            print(f"    CSS vs {v}: 세 항목 전 조건 지배 (M5 미수집이라 완전 판정은 불가)")
    if dominated_all:
        print("    ⇒ 세 항목 기준 지배 성립 — 규칙 (1)의 완전 판정은 M5 수집 후에만 가능")
    else:
        print("    ⇒ 지배 불성립 — F 기계장치 도입 근거는 M5 없이도 **상실되지 않음** 확정")

    # (2) 거울상: F의 B-승리 셀 과반에서 CSS가 F 대비 동등 이상인가.
    print("  (2) 거울상 의무 검사 — F 변형의 B-승리 셀에서 CSS가 동등 이상인가.")
    for v in F_VARIANTS:
        if not wins[v]:
            print(f"    {v}: B-승리 셀 없음 → 거울상 검사 대상 아님")
            continue
        css_ge = 0
        for (rate, f, s) in wins[v]:
            recs = by_rate[rate]
            ca, cb, _ = arm_samples(recs, "F", f, s)
            va_, vb_, _ = arm_samples(recs, v, f, s)
            la, ha = unpaired_ratio_ci(ca, va_)
            lb, hb = unpaired_ratio_ci(cb, vb_)
            if verdict(la, ha) in ("우세", "등가") and verdict(lb, hb) in ("우세", "등가"):
                css_ge += 1
        majority = css_ge * 2 > len(wins[v])
        print(f"    {v}: B-승리 {len(wins[v])}조건 중 CSS 동등 이상 {css_ge} "
              f"→ 거울상 의무 {'**발동** (F 채택에 의미론 결격 병기 선결)' if majority else '미발동'}")

    print()
    print("=" * 78)
    print("종합")
    print("=" * 78)
    for v in F_VARIANTS:
        n_cells = len({(f, s) for (_, f, s) in wins[v]})
        print(f"  {v}: 1차 분기 {'기각' if n_cells == 0 else f'생존(B-승리 {n_cells}셀)'}")
    print(f"  \"또는\" 분기: {'발동 — 결론 = B 유지(F 계열 기각)' if or_fired else '미발동'}")
    if or_fired:
        print("  ※ 두 분기가 상충하면(변형이 1차 분기를 생존했는데 \"또는\"이 발동)")
        print("     §9 문언상 \"또는\"은 어느 쪽이든 발동하면 결론 = B 유지다.")
        print("     단, 이전 라운드의 미발동(스로틀 3.598ms)과 뒤집힌 경위 —")
        print("     구 측정이 노드 슬롯 누수 하네스에서 나온 점 — 를 §11-2에 함께 기록할 것.")


if __name__ == "__main__":
    main()
