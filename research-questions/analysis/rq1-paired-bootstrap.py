import json
import math
import statistics
import random
from collections import defaultdict

# RQ1-benchmark-protocol.md §8 "분산 보고와 판정 어휘" — paired bootstrap on the
# median ratio (candidate / baseline), same resampled rep-indices used for both
# arms (valid because run_combination seeds the RNG per-rep-index identically
# across all candidates — see crates/loom-bench/src/lib.rs run_combination).
#
# KNOWN DEVIATION FROM THE PROTOCOL (stated, not hidden): §8's "반복 프로토콜"
# calls for 50 warmup + 200 measured reps per cell. The existing Track L data
# has only 6-30 reps depending on n (see rq1-phase1-track-l-core-grid-v5-*.json).
# The CIs below are therefore wider / less conclusive than a protocol-compliant
# run would produce -- reported as-is, not smoothed over.

N_RESAMPLES = 10_000
random.seed(1234567890)  # not a protocol PRNG requirement (that's PCG64/SHA-256
                          # per §8) -- this script's own resampling seed only,
                          # for reproducibility of this analysis pass.

def load(path):
    return json.load(open(path))

def paired_bootstrap_ratio_ci(a_samples, b_samples, ci_level):
    n = len(a_samples)
    ratios = []
    for _ in range(N_RESAMPLES):
        idx = [random.randrange(n) for _ in range(n)]
        a_med = statistics.median(a_samples[i] for i in idx)
        b_med = statistics.median(b_samples[i] for i in idx)
        if b_med <= 0:
            continue
        ratios.append(a_med / b_med)
    ratios.sort()
    if not ratios:
        return (float('nan'), float('nan'))
    lo_p = (1 - ci_level) / 2
    hi_p = 1 - lo_p
    lo = ratios[int(lo_p * len(ratios))]
    hi = ratios[min(int(hi_p * len(ratios)), len(ratios) - 1)]
    return (lo, hi)

def verdict(ci_lo, ci_hi):
    if ci_hi < 1:
        v = "우세"
    elif ci_lo > 1:
        v = "열세"
    else:
        v = None
    if ci_lo >= 1/1.1 and ci_hi <= 1.1:
        v = "등가"
    triple = ci_lo >= 3
    if v is None:
        v = "비결정"
    return v, triple

def analyze(path, label, baseline="K"):
    data = load(path)
    print(f"\n{'='*100}\n{label} (baseline={baseline}, n=10000, paired bootstrap {N_RESAMPLES} resamples)\n{'='*100}")
    by_workload = defaultdict(dict)
    for row in data:
        if row["n"] != 10000:
            continue
        by_workload[row["workload"]][row["candidate"]] = row["samples_ms"]

    for workload in ["append", "head_insert", "remove_middle", "shuffle10", "swap"]:
        cands = by_workload[workload]
        base = cands.get(baseline)
        if base is None:
            continue
        print(f"\n-- {workload} (reps={len(base)}) --")
        for name in ["C", "D", "E", "D-LIS", "E-LIS"]:
            if name not in cands:
                continue
            samples = cands[name]
            lo99, hi99 = paired_bootstrap_ratio_ci(samples, base, 0.99)
            lo95, hi95 = paired_bootstrap_ratio_ci(samples, base, 0.95)
            v, triple = verdict(lo99, hi99)
            flag = " [3배 이상]" if triple else ""
            print(f"  {name:6s} vs {baseline}: 99% CI=[{lo99:.3f}, {hi99:.3f}]  95% CI=[{lo95:.3f}, {hi95:.3f}]  판정={v}{flag}")

DATA = "/home/ybi/loom-research/research-questions/data"
analyze(f"{DATA}/rq1-phase1-track-l-core-grid-v5-chromium.json", "CHROMIUM — Track L, baseline K")
analyze(f"{DATA}/rq1-phase1-track-l-core-grid-v5-firefox.json", "FIREFOX — Track L, baseline K")

# C의 채택 조건 (a): D 대비 동등 이상. 별도 baseline=D로 재계산.
analyze(f"{DATA}/rq1-phase1-track-l-core-grid-v5-chromium.json", "CHROMIUM — C vs D (RQ1 §9 C 채택조건 a)", baseline="D")
analyze(f"{DATA}/rq1-phase1-track-l-core-grid-v5-firefox.json", "FIREFOX — C vs D (RQ1 §9 C 채택조건 a)", baseline="D")
