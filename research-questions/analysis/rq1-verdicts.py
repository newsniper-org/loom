"""RQ1-benchmark-protocol.md §9의 **트랙 L 조항 전부**를 한 번에 기계적으로 적용한다.

왜 이 스크립트가 따로 필요한가 (2026-08-02):
지금까지 §9는 조항별로 흩어진 스크립트로 적용돼 왔고(`rq1-paired-bootstrap.py`
= C 채택 (a), `rq1-c-rejection-gate.py` = C 기각 앞 분기,
`rq1-row-depth-bootstrap.py` = row 깊이 축), **어느 스크립트도 계산하지 않는
조항이 남아 있었다** — C 채택 (b), D 기각 (iii)의 전 셀 조건, D 채택 양성 조건,
E 기각 (i)의 M2 결정 규칙, E 기각 (ii). 조항이 있는데 계산되지 않으면
"판정 안 됨"이 아니라 **"판정된 줄 알고 넘어감"**이 되기 쉽다.

여기서는 §9의 트랙 L 조항을 하나도 빠뜨리지 않고 순서대로 돌리고, 계산에
필요한 데이터가 없으면 **"미계측"으로 명시 출력**한다(조용히 건너뛰지 않는다).

트랙 T 조항(B/F)은 데이터 소스가 달라 `rq1-track-t-bf-gate.py`가 맡는다.

방법론은 기존 스크립트와 동일한 paired bootstrap(같은 rep 인덱스로 리샘플 —
§8이 "시드 열 전 후보 공유"를 규정하므로 유효). 새 방법론을 도입하지 않는
이유는 §11-3에 걸리지 않기 위함이다.

실행: `python3 research-questions/analysis/rq1-verdicts.py`
"""

import json
import random
import statistics
from collections import defaultdict

N_RESAMPLES = 10_000
CI = 0.99
random.seed(20260802)

DATA = "/home/ybi/loom-research/research-questions/data"
VERSION = "v15"

# §3.1 워크로드 이름 ↔ 하네스 이름.
W = {"W1": "swap", "W2": "shuffle", "W3": "shuffle10", "W4": "head_insert",
     "W5": "append", "W7": "value_update"}
JUDGE_WORKLOADS = ["W1", "W2", "W3", "W4"]  # §9가 판정 셀로 지목하는 넷


# ---------------------------------------------------------------------------
# 통계 — 기존 스크립트와 동일
# ---------------------------------------------------------------------------

def pct(xs, p):
    k = (len(xs) - 1) * p
    f = int(k)
    c = min(f + 1, len(xs) - 1)
    return xs[f] if f == c else xs[f] * (c - k) + xs[c] * (k - f)


def paired_ratio_ci(a, b, ci=CI):
    """a/b 중앙값 비율의 CI. a가 후보, b가 기준선."""
    n = min(len(a), len(b))
    if n == 0:
        return (float("nan"), float("nan"))
    ratios = []
    for _ in range(N_RESAMPLES):
        idx = [random.randrange(n) for _ in range(n)]
        am = statistics.median(a[i] for i in idx)
        bm = statistics.median(b[i] for i in idx)
        if bm > 0:
            ratios.append(am / bm)
    if not ratios:
        return (float("nan"), float("nan"))
    ratios.sort()
    lo_p = (1 - ci) / 2
    return pct(ratios, lo_p), pct(ratios, 1 - lo_p)


def verdict(lo, hi):
    """§8 판정 어휘. 비율 = 후보/기준선이므로 1 미만이 빠른 쪽."""
    if hi < 1:
        return "우세"
    if lo > 1:
        return "열세"
    return "비결정"


# ---------------------------------------------------------------------------
# 데이터 적재
# ---------------------------------------------------------------------------

def load(browser):
    path = f"{DATA}/rq1-phase1-track-l-core-grid-{VERSION}-{browser}.json"
    rows = json.load(open(path))
    idx = {}
    for r in rows:
        key = (r["item_type"], r["n"], r["row_bindings"], r["row_variants"],
               r["workload"], r["candidate"])
        idx[key] = r
    return rows, idx


STR, N = "String(heap)", 1000


def cell(idx, workload, bindings, cand, variants=False, n=N, item=STR):
    return idx.get((item, n, bindings, variants, W[workload], cand))


def compare(idx, workload, bindings, cand, base, **kw):
    """(판정, lo, hi, 설명) — 데이터가 없으면 판정이 '미계측'."""
    a, b = cell(idx, workload, bindings, cand, **kw), cell(idx, workload, bindings, base, **kw)
    if not a or not b:
        return ("미계측", float("nan"), float("nan"), f"{cand} 또는 {base} 셀 없음")
    lo, hi = paired_ratio_ci(a["samples_ms"], b["samples_ms"])
    return (verdict(lo, hi), lo, hi,
            f"{cand}={statistics.median(a['samples_ms']):.4f}ms "
            f"{base}={statistics.median(b['samples_ms']):.4f}ms")


def line(label, v, lo, hi, note=""):
    ci = "  CI=[  nan,   nan]" if lo != lo else f"  CI=[{lo:6.3f}, {hi:6.3f}]"
    print(f"    {label:34s} {v:6s}{ci}   {note}")


# ---------------------------------------------------------------------------
# §9 C
# ---------------------------------------------------------------------------

def gate_c(idx, browser):
    print(f"\n{'=' * 78}\n§9 C (고정 슬롯) — {browser}\n{'=' * 78}")

    print("  채택 (a): {W1,W2,W3,W4} × R1 전 셀에서 D 대비 동등 이상")
    ok_a = True
    for w in JUDGE_WORKLOADS:
        v, lo, hi, note = compare(idx, w, 1, "C", "D")
        line(f"{w} × R1  C/D", v, lo, hi, note)
        if v not in ("우세", "비결정"):
            ok_a = False
    # "동등 이상"은 우세 또는 등가. 비결정(CI가 1을 걸침)을 등가로 읽는다 —
    # §8 판정 어휘에 "등가"가 별도로 없으므로 이것이 문언에 가장 가깝다.
    print(f"    ⇒ (a) {'충족' if ok_a else '**불충족**'}")

    print("  채택 (b): {W1,W2,W3,W4} × R1 중 최소 1셀에서 K 대비 우세")
    ok_b = False
    for w in JUDGE_WORKLOADS:
        v, lo, hi, note = compare(idx, w, 1, "C", "K")
        line(f"{w} × R1  C/K", v, lo, hi, note)
        if v == "우세":
            ok_b = True
    print(f"    ⇒ (b) {'충족' if ok_b else '**불충족**'}")

    print("  채택 (c): opt-in 문법 수용 — **설계 판단, 실측 아님**(§9 문언)")

    print("  기각 앞 분기: W2 × R2에서 C/D 비율 CI 하한 ≥ 3")
    v, lo, hi, note = compare(idx, "W2", 8, "C", "D")
    fired_front = lo >= 3.0 if lo == lo else False
    line("W2 × R2  C/D", v, lo, hi, note)
    print(f"    ⇒ 앞 분기 {'**발동**' if fired_front else '미발동'}"
          f" (하한 {lo:.3f} {'≥' if fired_front else '<'} 3)")

    print("  기각 뒷 분기: W2 × R3에서 archetype move 구간 ≥ 스크립트 시간의 50%")
    r = cell(idx, "W2", 1, "C", variants=True)
    if not r or "stage_ms" not in r:
        print("    미계측 — R3 셀 또는 §7.1 단계 귀속 없음")
        fired_back = False
    else:
        st = r["stage_ms"]
        share = st["sync_point"] / st["total"] if st["total"] > 0 else 0.0
        fired_back = share >= 0.5
        print(f"    sync_point={st['sync_point']:.4f}ms / total={st['total']:.4f}ms "
              f"= {share * 100:.1f}%")
        print(f"    ⇒ 뒷 분기 {'**발동**' if fired_back else '미발동'}")

    rejected = fired_front or fired_back
    print(f"\n  ▶ C 판정: {'**기각**' if rejected else ('채택 조건 (a)(b) 충족 — (c)는 설계 판단' if (ok_a and ok_b) else '기각 미발동, 채택 조건 불충족 → 채택 불가')}")
    return {"adopt_a": ok_a, "adopt_b": ok_b, "reject": rejected}


# ---------------------------------------------------------------------------
# §9 D
# ---------------------------------------------------------------------------

def gate_d(idx, browser):
    print(f"\n{'=' * 78}\n§9 D (keyed 시퀀스 동기화) — {browser}\n{'=' * 78}")

    print("  기각 (iii) 앞항: {W1,W2,W3,W4} × {R1,R2} **전 셀**에서 K가 D 대비 동등 이상")
    k_all = True
    for bindings in (1, 8):
        for w in JUDGE_WORKLOADS:
            v, lo, hi, note = compare(idx, w, bindings, "K", "D")
            line(f"{w} × R{1 if bindings == 1 else 2}  K/D", v, lo, hi, note)
            if v not in ("우세", "비결정"):
                k_all = False
    print(f"    ⇒ 앞항 {'성립' if k_all else '**불성립**'}")

    print("  기각 (iii) 뒷항: 부속 비용 프로파일(M4/M7/M3 중 1개 이상)에서 D가 K 대비 순증")
    print("    미계측 — M4(이중 저장)·M7(resume walk)·M3는 이 하네스가 수집하지 않는다")
    print("    (§9 문언상 **두 항 모두** 성립해야 기각이므로, 뒷항 미계측이면 기각 불성립)")

    print("  채택 양성 조건: {W1,W2,W3} × R2 중 최소 1셀에서 K 대비 우세")
    ok_pos = False
    for w in ["W1", "W2", "W3"]:
        v, lo, hi, note = compare(idx, w, 8, "D", "K")
        line(f"{w} × R2  D/K", v, lo, hi, note)
        if v == "우세":
            ok_pos = True
    print(f"    ⇒ 양성 조건 {'충족' if ok_pos else '**불충족** → 후퇴 경로 판정'}")

    print(f"\n  ▶ D 판정: 기각 (iii) {'불성립(뒷항 미계측)' if k_all else '불성립(앞항 불성립)'}"
          f" / 양성 조건 {'충족' if ok_pos else '불충족'}")
    return {"reject_iii_front": k_all, "adopt_positive": ok_pos}


# ---------------------------------------------------------------------------
# §9 E
# ---------------------------------------------------------------------------

def gate_e(idx, browser):
    print(f"\n{'=' * 78}\n§9 E (OrderKey) — {browser}\n{'=' * 78}")
    print("  ※ 기각 (i)/(ii)는 **네이티브 경로**에서만 판정(§9 E-1).")
    r = cell(idx, "W2", 8, "E")
    if r and "reposition_native_cell_total" in r:
        nat, fb = r["reposition_native_cell_total"], r["reposition_fallback_cell_total"]
        print(f"    reposition 경로: 네이티브 {nat} / 폴백 {fb}"
              f" → {'네이티브 경로 확인' if fb == 0 else '**폴백 혼입 — 판정 데이터 부적격**'}")

    print("  기각 (i): 판정 셀 W1·W3 × R2 중 **어느 셀에서도** K 대비 우세를 못 보이면 기각")
    any_win = False
    for w in ["W1", "W3"]:
        v, lo, hi, note = compare(idx, w, 8, "E", "K")
        line(f"{w} × R2  E/K", v, lo, hi, note)
        if v == "우세":
            any_win = True
    fired_i = not any_win
    print(f"    ⇒ 기각 (i) {'**발동**' if fired_i else '미발동'}")

    print("  기각 (i) 병기(결정적): E의 셀당 총 DOM op 수 ≥ K의 50%이면 'DOM 연산 절감 없음'")
    for w in ["W1", "W3"]:
        e, k = cell(idx, w, 8, "E"), cell(idx, w, 8, "K")
        if not e or not k:
            print(f"    {w} × R2: 미계측")
            continue
        eo = e["m2_dom_ops"]["total_mutations"]
        ko = k["m2_dom_ops"]["total_mutations"]
        ratio = eo / ko if ko else float("nan")
        print(f"    {w} × R2  E={eo} K={ko}  비율={ratio:.3f} "
              f"→ {'**절감 없음**' if ratio >= 0.5 else '절감 있음'}")

    print("  기각 (ii): reconcile System 시간 ≥ (K의 DOM 적용 − E의 DOM 적용)")
    e, k = cell(idx, "W2", 8, "E"), cell(idx, "W2", 8, "K")
    if not e or not k or "stage_ms" not in e:
        print("    미계측")
        fired_ii = False
    else:
        recon = e["stage_ms"]["systems"]
        saving = k["stage_ms"]["dom_apply"] - e["stage_ms"]["dom_apply"]
        fired_ii = recon >= saving
        print(f"    E systems={recon:.4f}ms  절감분=K({k['stage_ms']['dom_apply']:.4f}) "
              f"− E({e['stage_ms']['dom_apply']:.4f}) = {saving:.4f}ms")
        print(f"    ⇒ 기각 (ii) {'**발동**' if fired_ii else '미발동'}")

    print(f"\n  ▶ E 판정: {'**기각**' if (fired_i or fired_ii) else '기각 미발동'}")
    return {"reject_i": fired_i, "reject_ii": fired_ii}


# ---------------------------------------------------------------------------
# W7 — 판정 셀은 아니지만 §3.4가 예고한 프로파일
# ---------------------------------------------------------------------------

def profile_w7(idx, browser):
    print(f"\n{'=' * 78}\nW7 (값 갱신) 프로파일 — {browser}   ※ §9 어느 조항에도 걸리지 않음\n{'=' * 78}")
    print("  §3.4 기대: 'C, D, E (retain 경로)' / 'K (전량 재스폰)'")
    for cand in ["K", "C", "D", "E", "D-LIS", "E-LIS"]:
        r = cell(idx, "W7", 1, cand)
        if not r:
            continue
        ops = r["m2_dom_ops"]["total_mutations"]
        med = statistics.median(r["samples_ms"])
        print(f"    {cand:6s} median={med:8.4f}ms  DOM op={ops:5d}")
    print("  ※ Phase 1의 D는 키가 아이템 표시 텍스트 자체라 값 변경 = 키 변경 —")
    print("     §3.4의 'D (retain 경로)' 기대는 별도 key 채널을 전제한 것이다(§11 개정 #3).")


# ---------------------------------------------------------------------------
# §7 M2 교차 검증 — 조항이 요구하는 대응표가 실제로 맞는지
# ---------------------------------------------------------------------------

def crosscheck_m2(idx, browser):
    print(f"\n{'=' * 78}\n§7 M2 MutationObserver 교차 검증 — {browser}\n{'=' * 78}")
    print("  대응표: setText → characterData, insert/remove/move 폴백/detach/attach → childList")
    print("  ※ 네이티브 moveBefore는 관측상 remove+insert 쌍으로 보인다(§7 M2 주석).")
    bad = 0
    for cand in ["K", "C", "D", "E"]:
        r = cell(idx, "W2", 1, cand)
        if not r or not r.get("mutation_records"):
            continue
        o, m = r["m2_dom_ops"], r["mutation_records"]
        st, rp = o["set_text_content"], o["reposition"]
        cd, cl = m["character_data"], m["child_list"]
        # C처럼 retained 슬롯 갱신만 하는 후보는 setText == characterData여야 한다.
        flag = ""
        if rp == 0 and st > 0 and cd != st:
            flag = "  ⚠ setText↔characterData 불일치"
            bad += 1
        print(f"    {cand:6s} setText={st:5d} reposition={rp:5d} | "
              f"charData={cd:5d} childList={cl:5d}{flag}")
    print(f"  ⇒ 대응표 위반 {bad}건")


# ---------------------------------------------------------------------------

def main():
    print("RQ1 §9 트랙 L 판정 — 전 조항 일괄 적용")
    print(f"데이터: rq1-phase1-track-l-core-grid-{VERSION}-{{chromium,firefox}}.json")
    print("판정은 §8 커버리지 표대로 **Chromium(1차 판정)**에서만 — Firefox는 교차 확인.")

    results = {}
    for browser, label in [("chromium", "CHROMIUM (1차 판정)"), ("firefox", "FIREFOX (교차 확인)")]:
        try:
            rows, idx = load(browser)
        except FileNotFoundError:
            print(f"\n[{label}] 데이터 파일 없음 — 건너뜀")
            continue
        r0 = rows[0]
        print(f"\n\n{'#' * 78}\n# {label}   (레코드 {len(rows)}, 워밍업 {r0.get('warmup', '?')} "
              f"+ 측정 {r0.get('reps', '?')}, 해상도 {r0.get('timer_resolution_ms', float('nan')):.4f}ms)\n{'#' * 78}")
        batched = sum(1 for r in rows if r.get("batched"))
        print(f"# §8 배치 규칙 전환 셀: {batched}/{len(rows)}")
        results[browser] = {
            "C": gate_c(idx, label),
            "D": gate_d(idx, label),
            "E": gate_e(idx, label),
        }
        profile_w7(idx, label)
        crosscheck_m2(idx, label)

    print(f"\n\n{'=' * 78}\n종합 (Chromium 1차 판정 기준)\n{'=' * 78}")
    c = results.get("chromium")
    if not c:
        print("  Chromium 데이터 없음 — 판정 불가")
        return
    print(f"  C: 채택(a)={'O' if c['C']['adopt_a'] else 'X'} "
          f"채택(b)={'O' if c['C']['adopt_b'] else 'X'} 기각={'발동' if c['C']['reject'] else '미발동'}")
    print(f"  D: 기각(iii) 앞항={'성립' if c['D']['reject_iii_front'] else '불성립'} "
          f"(뒷항 미계측 → 기각 불성립) / 양성조건={'O' if c['D']['adopt_positive'] else 'X'}")
    print(f"  E: 기각(i)={'발동' if c['E']['reject_i'] else '미발동'} "
          f"기각(ii)={'발동' if c['E']['reject_ii'] else '미발동'}")
    f = results.get("firefox")
    if f:
        for cand in ("C", "D", "E"):
            if f[cand] != c[cand]:
                print(f"  ⚠️ Firefox가 {cand}에서 다른 결과 — §11-1(b) 검토 대상 등재 사유")


if __name__ == "__main__":
    main()
