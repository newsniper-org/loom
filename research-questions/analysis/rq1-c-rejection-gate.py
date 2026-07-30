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

⚠️ **그러나 이 스크립트가 내는 수치를 §9 판정에 그대로 쓰면 안 된다.**
계산해 보니 C가 D보다 3~10배 *빠르게* 나오는데(기각 임계값과 정반대 방향),
그 원인은 C가 실제로 빠른 게 아니라 **R2 축이 기각 조항이 겨냥하는 비용을
모델링하지 않기 때문**이다 — `DomWrite::AppendStaticChildren`의 doc comment가
스스로 밝히듯 이 축은 row *생성* 비용만 깊이에 비례시키고, C의
"O(n×binding) retained-slot value-patch" 비용은 재현하지 않는다
(`list_reconcile_c_system`도 유지 슬롯에 대해 primary text 1개만 재기록한다).
즉 §11-1(b) "측정 방법 결함" 상태다. 상세와 필요한 수리는
`RQ1-dynamic-control-flow-scheduling.md`의 "C 기각 조건 재검토"(2026-07-28) 절 참고.

이 스크립트는 그 사실을 **기록**하기 위해 남긴다 — 수치를 산출하되 판정으로
쓰지 못하게 출력에 경고를 함께 찍는다.

방법론은 `rq1-paired-bootstrap.py`/`rq1-row-depth-bootstrap.py`와 동일한
paired bootstrap(같은 rep 인덱스로 리샘플 — `loom-bench`가 후보마다 RNG를
동일 시드로 돌리므로 유효)을 그대로 재사용한다. 새 방법론을 도입하지 않는
이유는 §11-3("같은 데이터에 새 기준을 소급 적용한 판정은 탐색적 관찰로만
표기 가능")에 걸리지 않게 하기 위함이다.
"""

import json
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
    data = json.load(open(path))
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
    ("CHROMIUM (1차 판정)", f"{DATA}/rq1-phase1-track-l-core-grid-v6-chromium.json"),
    ("FIREFOX (교차 확인)", f"{DATA}/rq1-phase1-track-l-core-grid-v6-firefox.json"),
]:
    r = analyze(path, browser)
    if r:
        results.append(r)

print(f"\n{'=' * 78}\n종합 — 이 수치는 §9 판정에 쓸 수 없다\n{'=' * 78}")
print("""
  위 비율은 임계값(하한 ≥ 3)과 **정반대 방향**으로 나온다(C가 D보다 빠름).
  그러나 이는 C가 실제로 빠르다는 뜻이 아니라, R2 축이 §9 C 기각 조항이
  겨냥하는 비용을 재현하지 않는다는 뜻이다:

    · DomWrite::AppendStaticChildren = 추가 binding을 spawn 시 1회 만드는
      **정적 자식**으로 모델링 (자기 doc comment가 "this axis doesn't test
      that half of R2's prediction"이라고 명시).
    · list_reconcile_c_system = 유지 슬롯에 대해 primary text 1개만 재기록,
      추가 binding은 spawn 경로에서만 생성.
    → 길이 불변 shuffle에서 C의 R2 비용 ≈ R1 비용. RQ1이 예측한
      "O(n×binding) retained-slot value-patch" 비용이 계측에 부재.

  ⇒ 앞 분기의 정확한 상태는 "R3가 없어 미검증"이 아니라
     **§11-1(b) 측정 방법 결함으로 측정 무효**.

  수리: R2 축의 추가 binding을 "정적 텍스트"에서 "슬롯 값에서 파생되어 값이
  바뀌면 전부 재기록되는 값 파생 binding"으로 바꾼 뒤 해당 셀 재실행.
  (keyed 후보 D/E는 retained row의 아이템이 안 바뀌므로 재기록 불필요 —
   그 비대칭이 곧 RQ1의 예측이고, 지금은 그게 안 보인다.)

  기존 D/E 판정에는 영향 없음: D 기각(iii)/E 기각(i)이 쓰는 비대칭
  ("K는 매 tick 전량 재생성, D/E는 retained row 무접촉")은 정적 자식
  모델로도 정확히 재현되므로 그 결론은 그대로 유효하다.

  ※ 판정은 §8 커버리지 표대로 Chromium(1차 판정)에서만 — Firefox는 교차 확인.
""")
