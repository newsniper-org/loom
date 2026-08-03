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

def integrity_checks(rows, browser):
    """판정을 내리기 **전에** 데이터가 조항이 요구하는 조건에서 나왔는지 본다.

    2026-08-03의 적대적 감사가 확증한 결함들이 재발했는지를 데이터만 보고
    판별할 수 있는 항목들이다 — 하네스가 고쳐졌다는 것과 이 수집분이 고쳐진
    하네스에서 나왔다는 것은 다른 명제다.
    """
    print(f"\n{'=' * 78}\n무결성 점검 (판정 전) — {browser}\n{'=' * 78}")
    problems = []

    # (1) 노드 슬롯 누수 — 상주가 n에 비례해야 하고 tick 수에 비례하면 안 된다.
    worst = max(rows, key=lambda r: r["live_node_slots"] / max(r["n"], 1))
    ratio = worst["live_node_slots"] / worst["n"]
    ok = ratio < 20
    print(f"  노드 슬롯 상주 최대 비율: {ratio:.2f}× (n={worst['n']}, {worst['candidate']}/{worst['workload']})")
    print(f"    ⇒ {'정상' if ok else '**누수 의심**'} (수리 전이면 tick 수 × n으로 자란다)")
    if not ok:
        problems.append("노드 슬롯 누수 의심")

    # (2) 배치 모드가 셀 안에서 갈리면 paired bootstrap의 전제가 깨진다(§8).
    from collections import defaultdict
    modes = defaultdict(set)
    for r in rows:
        modes[(r["item_type"], r["n"], r["row_bindings"], r["row_variants"], r["workload"])].add(
            r.get("batch_mode", "?"))
    split = [k for k, v in modes.items() if len(v) > 1]
    print(f"  배치 모드가 후보 간에 갈린 셀: {len(split)}/{len(modes)}")
    print(f"    ⇒ {'정상' if not split else '**§8 시드 열 공유 전제 붕괴**'}")
    if split:
        problems.append("배치 모드 셀 내 분기")
        for k in split[:3]:
            print(f"      {k}: {modes[k]}")
    dist = defaultdict(int)
    for r in rows:
        dist[r.get("batch_mode", "?")] += 1
    print(f"    모드 분포: {dict(dist)}")

    # (3) §8 셀 상한.
    over = [r for r in rows if r.get("over_cell_cap")]
    print(f"  §8 셀 상한(30분) 초과: {len(over)}건 — 최대 셀 벽시계 "
          f"{max(r['cell_wall_ms'] for r in rows) / 1000:.1f}초")
    if over:
        problems.append("셀 상한 초과")

    # (4) §8 타이머 조항.
    #
    # **엔진마다 격리 상태의 하한이 다르다**(2026-08-03 실측으로 확인):
    #   Chromium 149: crossOriginIsolated=true → 0.005ms (5µs)
    #   Firefox 151:  crossOriginIsolated=true → 0.020ms (20µs)
    # 처음엔 임계를 0.01ms로 두었다가 Firefox를 "클램프됨"으로 **오탐**했다.
    # 격리가 안 걸리면 Chromium은 0.1ms, Firefox는 1ms 수준으로 훨씬 커지므로
    # 임계는 0.05ms면 충분히 갈린다. (근본 수리는 `cross_origin_isolated`를
    # 트랙 L 레코드에도 싣는 것 — 트랙 T는 이미 싣고 있다. 그건 하네스 변경이라
    # §2.5상 재수집을 부르므로 다음 라운드로 미룬다.)
    res = {r["timer_resolution_ms"] for r in rows}
    clamped = max(res) > 0.05
    print(f"  타이머 해상도: {sorted(res)} ms — "
          f"{'격리 정상(엔진별 하한: Chromium 0.005 / Firefox 0.020)' if not clamped else '**클램프됨(COOP/COEP 미적용?)**'}")
    if clamped:
        problems.append("타이머 클램프")

    # (5) §8 표본 수.
    reps = {r["reps"] for r in rows}
    warm = {r.get("warmup") for r in rows}
    print(f"  표본: 워밍업 {sorted(warm)} + 측정 {sorted(reps)} "
          f"— {'§8 준수' if reps == {200} and warm == {50} else '**§8 미달**'}")
    if reps != {200} or warm != {50}:
        problems.append("표본 수 §8 미달")

    print(f"\n  ⇒ 무결성 문제 {len(problems)}건" + (f": {problems}" if problems else ""))
    return problems


def crosscheck_m2(idx, browser):
    print(f"\n{'=' * 78}\n§7 M2 MutationObserver 교차 검증 — {browser}\n{'=' * 78}")
    print("  대응표: setText → characterData, insert/remove/move 폴백/detach/attach → childList")
    print("  ※ 네이티브 moveBefore는 관측상 remove+insert 쌍으로 보인다(§7 M2 주석).")
    print("  ※ **새로 만든 엘리먼트**에 대한 setText는 텍스트 자식이 없어")
    print("     `textContent=` 경로로 가므로 childList로 관측된다 — K가 그 경우다.")
    bad = 0
    for cand in ["K", "C", "D", "E"]:
        r = cell(idx, "W2", 1, cand)
        if not r or not r.get("mutation_records"):
            continue
        o, m = r["m2_dom_ops"], r["mutation_records"]
        st, rp, ce = o["set_text_content"], o["reposition"], o["create_element"]
        cd, cl = m["character_data"], m["child_list"]
        # 대응표가 성립해야 하는 것은 **retained 슬롯 값 갱신**뿐이다.
        # 엘리먼트를 새로 만드는 후보(K)는 구조적으로 childList로 간다.
        flag = ""
        if rp == 0 and ce == 0 and st > 0:
            miss = abs(cd - st)
            if miss > max(1, st // 100):  # 1% 초과만 구조적 불일치로 본다
                flag = f"  ⚠ setText↔characterData 불일치 ({miss}건)"
                bad += 1
            elif miss > 0:
                flag = f"  (오차 {miss}건 — 1% 이내, 병기만)"
        print(f"    {cand:6s} setText={st:5d} reposition={rp:5d} create={ce:5d} | "
              f"charData={cd:5d} childList={cl:5d}{flag}")
    print(f"  ⇒ 구조적 대응표 위반 {bad}건")


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
        integrity_checks(rows, label)
        results[browser] = {
            "_idx": idx,
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
                print(f"  ⚠️ Firefox가 {cand}에서 다른 게이트 결론 — §11-1(b) 검토 대상 등재 사유")
        cross_browser_direction_check(results)


def cross_browser_direction_check(results):
    """§8: "Firefox 교차 확인이 우세 방향을 뒤집으면 자동 판정 변경이 아니라
    §11-1(b) 검토 대상 등재."

    **게이트 결론이 같아도 셀 단위 방향은 뒤집힐 수 있다.** 게이트 dict만
    비교하면 그걸 놓친다 — v15에서 실제로 게이트 셋은 전부 일치하는데 비교쌍
    40건 중 9건이 정반대로 뒤집혔다(2026-08-03). 그래서 셀 단위로 따로 센다.
    """
    print(f"\n{'-' * 78}\n브라우저 간 우세 방향 대조 (§8 — 뒤집히면 §11-1(b) 등재 사유)\n{'-' * 78}")
    ci, fi = results["chromium"]["_idx"], results["firefox"]["_idx"]
    pairs = [("C", "D", "C 채택(a)"), ("C", "K", "C 채택(b)"),
             ("K", "D", "D 기각(iii)앞항"), ("D", "K", "D 양성"), ("E", "K", "E 기각(i)")]
    total = mismatch = opposite = 0
    for w in JUDGE_WORKLOADS:
        for bindings in (1, 8):
            for a, b, tag in pairs:
                ka, kb = cell(ci, w, bindings, a), cell(ci, w, bindings, b)
                fa, fb = cell(fi, w, bindings, a), cell(fi, w, bindings, b)
                if not (ka and kb and fa and fb):
                    continue
                total += 1
                vc = verdict(*paired_ratio_ci(ka["samples_ms"], kb["samples_ms"]))
                vf = verdict(*paired_ratio_ci(fa["samples_ms"], fb["samples_ms"]))
                if vc == vf:
                    continue
                mismatch += 1
                hard = {vc, vf} == {"우세", "열세"}
                opposite += hard
                mark = "⚠️ 정반대" if hard else "  (한쪽 비결정)"
                print(f"  {mark}  {w}×R{1 if bindings == 1 else 2:1d} {a}/{b:2s}  "
                      f"Chromium={vc:4s} → Firefox={vf:4s}   [{tag}]")
    print(f"\n  비교쌍 {total}건: 어휘 불일치 {mismatch}건, 그중 **정반대** {opposite}건")
    if opposite:
        print("  ⇒ §11-1(b) **검토 대상 등재 사유 발생**. 판정은 §8대로 Chromium 단독으로")
        print("     유지하되(브라우저 간 풀링·평균 금지), 이 목록을 RQ1에 병기할 것.")


if __name__ == "__main__":
    main()
