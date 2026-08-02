import json
import statistics
import random
from collections import defaultdict

# RQ1-benchmark-protocol.md §9's D/E/C decision gates, applied mechanically
# to the row-depth axis (R1/R2) data collected 2026-07-24. Same paired
# bootstrap methodology as rq1-paired-bootstrap.py (same-rep-index
# resampling is valid because crates/loom-bench seeds the RNG identically
# per rep across all candidates).

N_RESAMPLES = 10_000
random.seed(20260724)


def load(path):
    return json.load(open(path))


def pct(sorted_samples, p):
    k = (len(sorted_samples) - 1) * p
    f = int(k)
    c = min(f + 1, len(sorted_samples) - 1)
    if f == c:
        return sorted_samples[f]
    return sorted_samples[f] * (c - k) + sorted_samples[c] * (k - f)


def paired_bootstrap_ratio_ci(a, b, ci_level):
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


def verdict(lo, hi):
    if hi < 1:
        return "우세"
    if lo > 1:
        return "열세"
    if lo >= 1 / 1.1 and hi <= 1.1:
        return "등가"
    return "비결정"


def analyze(path, label):
    data = load(path)
    by_key = {}
    for row in data:
        if row["n"] != 1000 or row["item_type"] != "String(heap)":
            continue
        by_key[(row["row_bindings"], row["workload"], row["candidate"])] = row["samples_ms"]

    print(f"\n{'=' * 90}\n{label}\n{'=' * 90}")

    print("\n-- D vs K, E vs K at R2 (RQ1 §9's exact D/E decision cells) --")
    for rb in [1, 8]:
        for wl in ["swap", "shuffle", "shuffle10"]:
            for cand in ["D", "E"]:
                a = by_key.get((rb, wl, cand))
                k = by_key.get((rb, wl, "K"))
                if not a or not k:
                    continue
                lo, hi = paired_bootstrap_ratio_ci(a, k, 0.99)
                print(f"  row_bindings={rb} {wl:10s} {cand} vs K: CI=[{lo:.3f},{hi:.3f}] {verdict(lo, hi)}")

    print("\n-- C vs D (조건 a), C vs K (조건 b) at R1, {W1,W2,W3,W4} --")
    for wl in ["swap", "shuffle", "shuffle10", "head_insert"]:
        c = by_key.get((1, wl, "C"))
        d = by_key.get((1, wl, "D"))
        k = by_key.get((1, wl, "K"))
        if not c or not d or not k:
            continue
        lo_d, hi_d = paired_bootstrap_ratio_ci(c, d, 0.99)
        lo_k, hi_k = paired_bootstrap_ratio_ci(c, k, 0.99)
        print(
            f"  {wl:12s} C vs D: CI=[{lo_d:.3f},{hi_d:.3f}] {verdict(lo_d, hi_d):6s} | "
            f"C vs K: CI=[{lo_k:.3f},{hi_k:.3f}] {verdict(lo_k, hi_k)}"
        )


DATA = "/home/ybi/loom-research/research-questions/data"
analyze(f"{DATA}/rq1-phase1-track-l-core-grid-v14-chromium.json", "CHROMIUM")
analyze(f"{DATA}/rq1-phase1-track-l-core-grid-v14-firefox.json", "FIREFOX")
