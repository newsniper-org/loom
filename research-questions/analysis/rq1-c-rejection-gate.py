"""RQ1-benchmark-protocol.md §9의 **C 기각 조건**을 기존 데이터에 기계적으로 적용.

배경 — 왜 이 스크립트가 따로 필요한가: `rq1-row-depth-bootstrap.py`는 C에
대해 **채택** 조건 (a)/(b)만 R1에서 계산하고 끝난다. §9의 C **기각** 조건은
그와 다른 셀을 본다:

    기각: W2 × R2에서 D 대비 M1(a) 비율 CI 하한 ≥ 3,
          **또는** W2 × R3 프로파일에서 archetype move 구간(§7.1)이
          스크립트 시간의 50% 이상

앞 분기(W2 × R2)는 **기존 원시 데이터로 바로 계산 가능**하다 —
`crates/loom-bench`의 `all_makers`가 6후보 전부를 매 조합마다 돌리므로
row_bindings=8(R2) × shuffle(W2)에 C 표본이 이미 들어 있다. 그런데 지금까지
아무도 이 셀을 계산하지 않았고, RQ1 문서에는 "기각 조건도 이번 라운드에 R3
데이터가 없어 미검증"으로만 적혀 있었다 — R3가 필요한 것은 **뒷 분기뿐**인데
앞 분기까지 함께 미검증으로 묶여 있었던 셈이다(2026-07-28 발견).

**2026-07-28 (2단계) — 측정 방법 결함이 수리되어 이제 이 수치는 유효하다.**
처음 계산했을 때는 C가 D보다 3~10배 *빠르게*(임계값과 정반대로) 나왔는데, 원인은
C가 실제로 빠른 게 아니라 R2 축이 추가 binding을 spawn 시 1회 만드는 **정적
텍스트**로 두어, 조항이 겨냥하는 "O(n×binding) retained-slot value-patch" 비용이
계측에 아예 없었기 때문이었다(§11-1(b) 측정 방법 결함). 그 축을 **값 파생
binding**으로 바꾸고(`RowBindingTargets` + `DomWrite::SetRowBindingTexts`,
슬롯 값이 바뀌면 파생 binding 전부 재기록) 재수집(v9)한 결과 방향이 뒤집혔다 —
Chromium C/D = [1.736, 2.290]으로 C가 D보다 ~2배 느리다(RQ1 예측과 일치).

방법론은 `rq1-paired-bootstrap.py`/`rq1-row-depth-bootstrap.py`와 동일한
paired bootstrap(같은 rep 인덱스로 리샘플 — `loom-bench`가 후보마다 RNG를
동일 시드로 돌리므로 유효)을 그대로 재사용한다. 새 방법론을 도입하지 않는
이유는 §11-3("같은 데이터에 새 기준을 소급 적용한 판정은 탐색적 관찰로만
표기 가능")에 걸리지 않게 하기 위함이다.
"""

import json


def _load_records(path):
    """v16부터 stock 수집 경로가 `{track_l, track_t}`로 감싸 보낸다(§11 개정 #4)."""
    p = _load_records(path)
    return p["track_l"] if isinstance(p, dict) else p

import random
import statistics

N_RESAMPLES = 10_000
random.seed(20260728)

DATA = "/home/ybi/loom-research/research-questions/data"

# §3.1의 워크로드 이름 ↔ 하네스 워크로드 이름 대응. §9 C 기각 조항이 지목하는
# 것은 W2(전체 shuffle) 하나뿐이다.
W2 = "shuffle"
R2 = 8


def pct(sorted_samples, p):
    k = (len(sorted_samples) - 1) * p
    f = int(k)
    c = min(f + 1, len(sorted_samples) - 1)
    if f == c:
        return sorted_samples[f]
    return sorted_samples[f] * (c - k) + sorted_samples[c] * (k - f)


def paired_bootstrap_ratio_ci(a, b, ci_level=0.99):
    n = len(a)
    ratios = []
    for _ in range(N_RESAMPLES):
        idx = [random.randrange(n) for _ in range(n)]
        a_med = statistics.median(a[i] for i in idx)
        b_med = statistics.median(b[i] for i in idx)
        if b_med <= 0:
            continue
        ratios.append(a_med / b_med)
    ratios.sort()
    if not ratios:
        return (float("nan"), float("nan"))
    lo_p = (1 - ci_level) / 2
    return pct(ratios, lo_p), pct(ratios, 1 - lo_p)


def analyze(path, label):
    data = _load_records(path)
    by_key = {}
    for row in data:
        if row["n"] != 1000 or row["item_type"] != "String(heap)":
            continue
        by_key[(row["row_bindings"], row["workload"], row["candidate"])] = row["samples_ms"]

    print(f"\n{'=' * 78}\n{label} — §9 C 기각 조건 앞 분기 (W2 × R2, n=1000, String)\n{'=' * 78}")

    c = by_key.get((R2, W2, "C"))
    d = by_key.get((R2, W2, "D"))
    if not c or not d:
        print("  데이터 없음 — 이 파일에는 R2×W2 셀이 없다.")
        return None

    lo, hi = paired_bootstrap_ratio_ci(c, d)
    fired = lo >= 3.0
    print(f"  C median = {statistics.median(c):.4f}ms   (n={len(c)} reps)")
    print(f"  D median = {statistics.median(d):.4f}ms   (n={len(d)} reps)")
    print(f"  C/D 비율 99% CI = [{lo:.3f}, {hi:.3f}]")
    print(f"  §9 기각 임계값: CI 하한 ≥ 3  →  하한 {lo:.3f} {'≥' if fired else '<'} 3")
    print(f"  ⇒ 앞 분기 {'**발동** (C 기각)' if fired else '미발동'}")
    return {"label": label, "ci_lo": lo, "ci_hi": hi, "fired": fired,
            "c_median_ms": statistics.median(c), "d_median_ms": statistics.median(d),
            "reps": len(c)}


results = []
for browser, path in [
    ("CHROMIUM (1차 판정)", f"{DATA}/rq1-phase1-track-l-core-grid-v16-chromium.json"),
    ("FIREFOX (교차 확인)", f"{DATA}/rq1-phase1-track-l-core-grid-v16-firefox.json"),
]:
    r = analyze(path, browser)
    if r:
        results.append(r)

print(f"\n{'=' * 78}\n종합\n{'=' * 78}")
primary = next((r for r in results if r["label"].startswith("CHROMIUM")), None)
if primary:
    if primary["fired"]:
        print("  §9 C 기각 앞 분기 **발동** — C는 기각이며, 뒷 분기(W2×R3의")
        print("  archetype move 지배)는 측정할 필요가 없어진다('또는'로 연결).")
    else:
        print(f"  §9 C 기각 앞 분기 **미발동** — C/D 비율 CI 하한 {primary['ci_lo']:.3f} < 3.")
        print("  단, 이제는 계측 결함 때문이 아니라 **실제로 3배까지 느리지 않아서**다")
        print("  (2026-07-28 R2 축 값 파생 수리 이후). 방향 자체는 RQ1 예측대로")
        print("  C가 D보다 느린 쪽으로 뒤집혔다.")
        print()
        print("  ⇒ C 기각 여부는 뒷 분기(W2 × R3 프로파일에서 archetype move 구간이")
        print("     스크립트 시간의 50% 이상)에 달려 있고, 그건 R3 row 유형 +")
        print("     §7.1 단계 귀속 계측이 둘 다 있어야 측정 가능하다(§7.1은 구현됨,")
        print("     R3 row 유형은 미구현).")
print()
print("  ※ 판정은 §8 커버리지 표대로 Chromium(1차 판정)에서만 — Firefox는 교차 확인.")
fx = next((r for r in results if r["label"].startswith("FIREFOX")), None)
if primary and fx and (primary["ci_lo"] > 1) != (fx["ci_lo"] > 1):
    print("  ⚠️ Firefox가 우세 방향을 뒤집는다 — §11-1(b) 검토 대상 등재 사유.")
