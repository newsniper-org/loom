"""`rq1-verdicts.py` 자신을 검증하는 **합성** 트랙 L 픽스처 생성기.

**이건 측정 데이터가 아니다.** `research-questions/data/`에 절대 쓰지 말 것 —
숫자는 `random.gauss`로 만든 것이고, 여기 있는 유일한 목적은 판정 스크립트가
조항이 시키는 대로 접히는지 보는 것이다.

**왜 저장소에 있나 (2026-09-24)**: 판정 스크립트의 결함은 전부 **데이터가
정상인 경로에서는 안 보이는 것들**이었다 — 셀이 없을 때, §8 상한에 걸렸을 때,
라벨이 바뀌었을 때만 드러난다. 그런 상태의 실측 데이터는 재현하려면 수십 분
수집을 다시 해야 하므로(그리고 §2.5상 마음대로 만들 수도 없다) 합성 픽스처가
유일한 재현 수단이다. 이걸 스크래치패드에 두었던 동안 같은 부류의 결함
("판정 불가"를 "불충족"으로 접는 것)이 **세 번** 재발했다.

    python3 make-track-l-fixture.py <출력디렉터리> [base|partial_cap|missing_cell]
    RQ1_DATA_DIR=<출력디렉터리> python3 ../rq1-verdicts.py

`selfcheck.py`가 세 모드를 다 돌려 기대 문구를 단언한다.

**형식 (2026-09-27, 개정 #10)**: 기본은 v22 형식이다 — 셀 소속(`group`·`cell_id`), 이중 측정 셀에서
C·D·D-LIS 두 변형이 **같은 셀**(9후보), rep별 단계 시간(`stage_samples_ms`), GC 통제(`gc`). `legacy_v21`과
`batch_mix`는 v21 형식(변형마다 별도 셀, 새 필드 없음)으로 쓴다 — 옛 수집분도 판정기가 계속 읽어야 하고,
배치 모드 혼용은 옛 형식에서만 생길 수 있기 때문이다.

모드:
  legacy_v21   — v21 형식 그대로. 판정기가 새 필드 없이도 완주하고, 옛 경로임을 줄마다 밝히는가.
  cross_cell   — v22 형식인데 C의 무조건 변형이 **다른 셀(그룹)**에 있다(옛 하네스처럼). 셀 구성 점검과
                 같은 셀 짝 보고가 그것을 잡는가.
  gc_missing   — Chromium인데 GC 통제가 안 된 셀(바인딩 없음)·강제 GC 미관측 셀·로그 유실 셀이 섞였다.
  e_ii_margin  — E 기각 (ii): rep별 짝지은 margin의 median은 +0.15ms(미발동)인데 마지막 rep 점값으로는
                 발동. 판정이 개정 #10 조작화(짝지은 margin의 median ≤ 0)를 읽는가 — 부호를 뒤집으면 깨진다.
  throttle_not_applied — 스로틀 파일인데 두 셀의 셀 시작 배수가 1(에이전트 인자 실수 등). 잡는가.
  write_axis_inert — 무조건 변형의 M2가 조건부와 같다(축이 행동에 안 닿는 상태). 존재 단언이 잡는가.
  stage_median — C의 W2×R3에서 마지막 rep 점값은 sync_point 60%(v21 방식이면 기각 뒷 분기 발동)인데
                 rep별 median은 30%. 판정이 rep별 median을 읽는가.
  throttle_threshold — 비스로틀 C/D 2.5 → 스로틀 3.5. 어휘는 "열세→열세"로 그대로인데
                 C 기각 앞 분기(하한 ≥ 3)만 스로틀에서 발동한다. 어휘 대조는 침묵하고
                 게이트 구성 요소 대조만 울려야 한다(2026-09-27 등재한 구멍).
  noise_not_neutral — K만 스톨을 맞아 상대 꼬리가 두꺼운 상태. 머신 소음의
                 후보 중립성 검사가 그것을 잡는지 본다.
  stalled_cell — 한 셀의 벽시계만 기대치의 수십 배(표본은 정상). 측정 중 머신
                 압박으로 멈춰 있던 셀을 정지 흔적 검사가 잡는지 본다.
  scaling_nonmono — C/K 비율이 n에 따라 비단조(0.30 → 0.25 → 0.28). 외삽 문장이
                 끝점만 보고 방향을 말하지 않는지 본다.
  batch_mix    — 같은 논리 셀의 두 write 변형이 다른 배치 모드로 측정된 상태.
                 §9 서두의 변형 선택이 기준선과 다른 모드의 레코드를 데려온다.
  vocab_overlap — C/K 비율이 우세 ∩ 등가 겹침 구간에 드는 상태. 겹침 기록에
                 브라우저·조항 맥락이 실제로 붙는지 본다.
  firefox_bidi — (개정 #10 후속 3) Firefox BiDi 수집분(`…-firefox-bidi.json`)을 함께 쓴다. 한 셀은 강제 GC 구간에
                 GCSlice가 없고, 한 셀은 CC가 없으며, W1×R1 K는 셀 상한에 걸린 채 방향이 뒤집힌다 — 존재 단언
                 둘이 울리고, 상한 셀은 방향 대조에서 "(측정 불가)"로 빠져야 한다("정반대"로 세면 안 된다).
  firefox_wrong_file — BiDi 이름의 파일인데 내용은 에이전트 없는 수집분(전 셀 unavailable). 조용히 "통제 없는
                 수집분"으로 넘어가지 않고 문제로 올리는가.
  base         — 전 셀 정상. 스크립트가 종료코드 0으로 완주해야 한다.
  partial_cap  — W2 × R2 판정 셀이 표본 3개에서 §8 셀 상한에 걸린 상태.
                 **표본이 0개가 아니다** — 상한은 측정 루프 안에서도 발동하므로
                 (loom-bench의 `break \'measure`) 1~199개짜리 상한 초과 레코드가
                 실제로 나온다. 그런 셀이 정상 판정 셀로 들어가면 표본 3개짜리
                 CI가 §9 조항에 그대로 먹힌다.
  missing_cell — D-LIS의 W1 × R1 셀이 아예 없는 상태(수집 중단·부분 재수집).
                 채택 (a)가 "판정 불가"여야 하고, 그것이 `▶` 줄에서 다시
                 "불충족"으로 접히면 안 된다.
"""
import json, random, sys, os

out_dir = sys.argv[1]
mode = sys.argv[2] if len(sys.argv) > 2 else "base"
random.seed(1)
LEGACY = mode in ("legacy_v21", "batch_mix")
VERSION = "v21" if LEGACY else "v22"

POD, STR = "ItemId(POD u32)", "String(heap 128B)"
CANDS = ["K", "C", "D", "D-LIS", "E", "E-LIS"]
CORE = ["swap", "shuffle", "shuffle10", "head_insert", "append", "value_update"]
CLAUSE = {"swap": "W1", "shuffle": "W2", "shuffle10": "W3", "head_insert": "W4",
          "append": "W5", "value_update": "W7", "replace50": "W10a", "replace100": "W10b",
          "clear": "W9", "remove_middle": "W11"}

# per-candidate base cost multiplier
MULT = {"K": 1.0, "C": 0.7, "D": 0.9, "D-LIS": 0.85, "E": 0.95, "E-LIS": 0.97}


def rec(cand, wl, item, n, rb, ri, rv, wv, reps=200, over=False, mult=None, noise=0.05):
    m = MULT[cand] if mult is None else mult
    base = 1.0 * (n / 1000) * (1 + 0.5 * (rb - 1)) * m
    samples = [round(base * (1 + random.gauss(0, noise)), 4) for _ in range(reps)]
    nodes_per_row = 1 + max(rb - 1, 0) + (2 if rb > 1 else 0) + ((1 + ri) if ri > 0 else 0) + (1 if rv else 0)
    return {
        "candidate": cand, "workload": wl, "item_type": item, "n": n, "row_bindings": rb,
        "reps": 200, "warmup": 50, "samples_ms": samples,
        "m2_dom_ops": {"total_mutations": 100, "reposition": 0, "create_element": 0,
                       "remove_node": 0, "set_text_content": 100},
        "m2_deterministic": True if wl in ("swap", "remove_middle") else None,
        "row_variants": rv, "row_inner": ri, "clause": CLAUSE.get(wl, "?"), "write_variant": wv,
        "timer_resolution_ms": 0.005, "batched": False, "batch_mode": "none", "batch_k": 1,
        "reposition_native_cell_total": 10, "reposition_fallback_cell_total": 0,
        "m1b_frame_ms": 3.0, "m1b_reps": 1,
        "mutation_records": {"character_data": 100, "child_list": 0, "attributes": 0},
        # 하네스가 싣는 시간 스케일 표식 — W5(append)만 시퀀스형 100스텝이다.
        "seq_steps": 100 if wl == "append" else 1,
        "stage_scope": "last_step_of_sequence" if wl == "append" else "the_measured_step",
        "cell_wall_ms": 1000.0, "over_cell_cap": over, "live_node_slots": int(n * nodes_per_row * 0.95) + 1,
        "user_agent": "Mozilla/5.0 synthetic", "cross_origin_isolated": True,
        "stage_ms": {"systems": 0.1, "sync_point": 0.1, "dom_apply": 0.5, "row_patch": 0.1, "total": 1.0},
    }


rows = []
for rb in (1, 8):
    for wl in CORE:
        for c in CANDS:
            rows.append(rec(c, wl, POD, 1000, rb, 0, False, "Conditional"))
    for wl in ("replace50", "replace100"):
        for c in CANDS:
            rows.append(rec(c, wl, POD, 1000, rb, 0, False, "Conditional"))
# doubled Unconditional
if LEGACY:
    # v21: 무조건 변형이 **별도 셀** — 6후보 전부가 한 벌 더.
    for r in list(rows):
        if r["workload"] in ("swap", "shuffle", "shuffle10", "head_insert", "replace50", "replace100"):
            rr = json.loads(json.dumps(r))
            rr["write_variant"] = "Unconditional"
            rows.append(rr)
else:
    # v22(개정 #10): **같은 셀**에 C·D·D-LIS의 무조건 변형만 더해진다. W7도 이중 측정(§10).
    for r in list(rows):
        if (r["workload"] in ("swap", "shuffle", "shuffle10", "head_insert", "replace50",
                              "replace100", "value_update")
                and r["candidate"] in ("C", "D", "D-LIS")):
            rr = json.loads(json.dumps(r))
            rr["write_variant"] = "Unconditional"
            rows.append(rr)
# scaling, R3, W8, W9, W11, heap
for n in (100, 10000):
    for c in CANDS:
        rows.append(rec(c, "shuffle", POD, n, 8, 0, False, "Conditional"))
        rows.append(rec(c, "head_insert", POD, n, 8, 0, False, "Conditional"))
        rows.append(rec(c, "shuffle", POD, n, 1, 0, False, "Conditional"))
for c in CANDS:
    rows.append(rec(c, "shuffle", POD, 1000, 1, 0, True, "Conditional"))
    rows.append(rec(c, "head_insert", POD, 100, 1, 100, False, "Conditional"))
    rows.append(rec(c, "clear", POD, 1000, 1, 0, False, "Conditional"))
    rows.append(rec(c, "remove_middle", POD, 1000, 1, 0, False, "Conditional"))
    rows.append(rec(c, "value_update", STR, 1000, 1, 0, False, "Conditional"))
    if not LEGACY:
        # 개정 #10: W7 × R2 × heap 신설 + heap W7도 C·D 양 변형.
        rows.append(rec(c, "value_update", STR, 1000, 8, 0, False, "Conditional"))
        if c in ("C", "D", "D-LIS"):
            rows.append(rec(c, "value_update", STR, 1000, 1, 0, False, "Unconditional"))
            rows.append(rec(c, "value_update", STR, 1000, 8, 0, False, "Unconditional"))

if mode == "partial_cap":
    # W2 x R2 judgment cell hit the §8 cap after 3 reps for every candidate (round-robin).
    for r in rows:
        if r["workload"] == "shuffle" and r["row_bindings"] == 8 and r["n"] == 1000 and not r["row_variants"]:
            r["over_cell_cap"] = True
            r["samples_ms"] = r["samples_ms"][:3]
            r["cell_wall_ms"] = 1_800_500.0
if mode == "noise_not_neutral":
    # **K만** 스톨을 맞은 상태: 표본의 10%를 5배로 늘려 상대 꼬리만 두껍게 한다
    # (median은 거의 그대로). 머신 소음이 후보 중립이라는 §9 비율의 전제가
    # 깨진 형태 — 라운드로빈 보호가 실제로 성립하는지를 검사가 보는가.
    random.seed(11)
    for r in rows:
        if r["candidate"] == "K" and r.get("samples_ms"):
            sm = r["samples_ms"]
            for _ in range(max(1, len(sm) // 10)):
                j = random.randrange(len(sm))
                sm[j] = sm[j] * 5.0
if mode == "stalled_cell":
    # 한 셀의 벽시계만 기대치의 수십 배로 만든다(표본은 그대로) — 스왑 스래싱·
    # GC·OS 스케줄링으로 측정 중 멈춰 있던 셀의 형태. 표본이 정상인데 벽시계만
    # 크므로 §8 상한 항목으로는 안 잡히고, 정지 흔적 검사만 잡을 수 있다.
    # v22 형식에서는 두 변형이 한 셀이므로 **셀 전체**(9후보)가 같은 벽시계를 싣는다.
    for r in rows:
        if (r["workload"] == "swap" and r["row_bindings"] == 1 and r["n"] == 1000
                and (not LEGACY or r["write_variant"] == "Conditional") and not r["row_variants"]
                and r["item_type"] == POD):
            r["cell_wall_ms"] = 900_000.0
if mode == "scaling_nonmono":
    # C/K 비율이 n에 따라 **비단조**가 되게 만든다(0.30 → 0.25 → 0.28). 끝점만
    # 비교하면 "감소 = C가 유리해진다"로 읽히지만 가운데가 더 낮으므로 추세가
    # 아니다. 외삽 문장이 단조성을 확인한 뒤에만 방향을 말하는지 본다.
    TARGET = {100: 0.30, 1000: 0.25, 10000: 0.28}
    kmed = {}
    for r in rows:
        if (r["candidate"] == "K" and r["workload"] == "shuffle"
                and r["row_bindings"] == 8 and not r["row_variants"]):
            kmed[r["n"]] = sorted(r["samples_ms"])[len(r["samples_ms"]) // 2]
    for r in rows:
        if (r["candidate"] == "C" and r["workload"] == "shuffle"
                and r["row_bindings"] == 8 and not r["row_variants"]
                and r["n"] in TARGET and r["n"] in kmed):
            want = kmed[r["n"]] * TARGET[r["n"]]
            cur = sorted(r["samples_ms"])[len(r["samples_ms"]) // 2]
            r["samples_ms"] = [v * want / cur for v in r["samples_ms"]]
if mode == "batch_mix":
    # §8 배치 규칙이 같은 논리 셀의 두 변형을 다른 모드로 재는 상태(실측 2026-09-25:
    # W1×R1에서 조건부 alternate / 무조건 none). 게다가 C가 W1에서 **무조건 변형을
    # 고르도록** 만들어(무조건을 더 빠르게) 기준선 K(언제나 조건부, 배치)와 짝이
    # 갈리게 한다 — 그것이 §9 서두가 실제로 데려올 수 있는 형태다.
    for r in rows:
        if r["workload"] in ("swap", "shuffle", "shuffle10", "head_insert",
                             "replace50", "replace100"):
            if r["write_variant"] == "Conditional":
                r["batched"], r["batch_mode"], r["batch_k"] = True, "alternate", 10
            else:
                r["batched"], r["batch_mode"], r["batch_k"] = False, "none", 1
        if (r["candidate"] == "C" and r["workload"] == "swap"
                and r["write_variant"] == "Unconditional" and r["n"] == 1000):
            r["samples_ms"] = [v * 0.5 for v in r["samples_ms"]]
if mode == "vocab_overlap":
    # C/K 비율이 등가 대역 [1/1.1, 1.1] **안**이면서 CI 전체가 1 미만이 되도록
    # 맞춘다 — §8이 열거한 우세와 등가가 **둘 다** 참인 구간. 판정기가 어느 쪽을
    # 먼저 읽는지가 C 채택 (b)를 가른다.
    for r in rows:
        if r["candidate"] == "C":
            base = 0.96 * (r["n"] / 1000) * (1 + 0.5 * (r["row_bindings"] - 1))
            random.seed(7)
            r["samples_ms"] = [round(base * (1 + random.gauss(0, 0.004)), 5)
                               for _ in range(200)]
if mode == "missing_cell":
    # remove D-LIS W1 x R1 (both variants) -> C 채택(a) has an unmeasured cell
    rows = [r for r in rows if not (r["candidate"] == "D-LIS" and r["workload"] == "swap"
                                    and r["row_bindings"] == 1 and r["n"] == 1000)]

THROTTLE_ROWS = None
if mode == "throttle_not_applied":
    CORE_ = {"swap", "shuffle", "shuffle10", "head_insert", "append", "value_update"}
    THROTTLE_ROWS = []
    for r in rows:
        if (r["n"] == 1000 and r["item_type"] == POD and not r["row_variants"]
                and r.get("row_inner", 0) == 0 and r["workload"] in CORE_):
            t = json.loads(json.dumps(r))
            t["samples_ms"] = [v * 3.45 for v in t["samples_ms"]]
            THROTTLE_ROWS.append(t)
if mode == "throttle_threshold":
    # **어휘는 안 바뀌는데 임계 발동만 갈리는** 상태 — 2026-09-27 등재한 구멍 그 자체.
    # 비스로틀: W2×R2 C/D-LIS = 2.5 (열세, C 기각 앞 분기 임계 3 미만 → 미발동)
    # 스로틀:   W2×R2 C/D-LIS = 3.5 (열세, 임계 3 이상 → 발동)
    # 어휘 대조는 "열세 → 열세"라 침묵하고, 게이트 구성 요소 대조만 울려야 한다.
    # C는 이중 측정 후보라 **두 write 변형을 모두** 바꿔야 한다 — 한쪽만 바꾸면
    # §9 서두의 셀별 우세 변형 선택이 안 바꾼 쪽(더 빠른 쪽)을 고른다.
    def w2r2(r, cand):
        return (r["candidate"] == cand and r["workload"] == "shuffle"
                and r["row_bindings"] == 8 and r["n"] == 1000 and not r["row_variants"])
    dl = {r["write_variant"]: r["samples_ms"] for r in rows if w2r2(r, "D-LIS")}
    for r in rows:
        if w2r2(r, "C"):
            r["samples_ms"] = [v * 2.5 for v in dl[r["write_variant"]]]
    # 스로틀 파일: §8 범위(핵심 격자)만, 전 표본 ×3.45 — 비율이 그대로 보존된다.
    CORE = {"swap", "shuffle", "shuffle10", "head_insert", "append", "value_update"}
    THROTTLE_ROWS = []
    for r in rows:
        if (r["n"] == 1000 and r["item_type"] == POD and not r["row_variants"]
                and r.get("row_inner", 0) == 0 and r["workload"] in CORE):
            t = json.loads(json.dumps(r))
            t["samples_ms"] = [v * 3.45 for v in t["samples_ms"]]
            THROTTLE_ROWS.append(t)
    tdl = {r["write_variant"]: r["samples_ms"] for r in THROTTLE_ROWS if w2r2(r, "D-LIS")}
    for t in THROTTLE_ROWS:
        if w2r2(t, "C"):
            t["samples_ms"] = [v * 3.5 for v in tdl[t["write_variant"]]]



def modernize(rows, throttle_rate=1):
    """v22(개정 #10) 필드를 붙인다 — 모드별 변형이 끝난 **뒤에** 부른다(표본 수가 바뀌었을 수 있다)."""
    keyf = lambda r: (r["item_type"], r["n"], r["row_bindings"], r.get("row_inner", 0),
                      r["row_variants"], r["workload"])
    groups = {}
    for r in rows:
        groups.setdefault(keyf(r), len(groups))
    members = {}
    for r in rows:
        members.setdefault(keyf(r), []).append(r)
    rng = random.Random(99)
    for r in rows:
        k = keyf(r)
        g = groups[k]
        r["group"] = g
        r["cell_id"] = f"{r['item_type']}|n={r['n']}|R{r['row_bindings']}|inner={r.get('row_inner', 0)}|variants={r['row_variants']}|{r['workload']}"
        r["cell_candidates"] = len(members[k])
        r["write_axis_in_cell"] = any(x["write_variant"] == "Unconditional" for x in members[k])
        r["windows_per_sample"] = 1
        r["stage_samples_scope"] = "sum_over_sequence_steps" if r["workload"] == "append" else "the_measured_step"
        m = len(r["samples_ms"])
        r["stage_samples_ms"] = {key: [round(v * (1 + rng.gauss(0, 0.02)), 5) for _ in range(m)]
                                 for key, v in r["stage_ms"].items()}
        ov = [0.0] * m
        for j in range(0, m, 17):
            ov[j] = 0.01
        # 실제 하네스처럼 C·D·D-LIS의 무조건 변형은 M2가 다르다(스모크: C set_text 2 → 1000).
        if (r["write_variant"] == "Unconditional" and r["candidate"] in ("C", "D", "D-LIS")
                and mode != "write_axis_inert"):
            r["m2_dom_ops"] = dict(r["m2_dom_ops"], total_mutations=1000, set_text_content=1000)
            r["mutation_records"] = dict(r["mutation_records"], character_data=1000)
        r["gc"] = {"control": "cdp", "source": "cdp-trace v8+cppgc renderer-main",
                   "cell_start": {"ok": True, "throttle_rate": throttle_rate, "tracing": "v8,cppgc"},
                   "forced": {"ok": True, "method": "HeapProfiler.collectGarbage"},
                   "segments": [{"ok": True, "data_loss": False, "sync_spread_ms": 0.01,
                                 "n_events": 120, "forced_majorgc_events": 2}],
                   "log_complete": True, "overlap_unit": "ms per sample (same scale as samples_ms)",
                   "with_gc": sum(1 for x in ov if x > 0), "overlap_ms": ov}


if not LEGACY:
    modernize(rows)
    if THROTTLE_ROWS is not None:
        modernize(THROTTLE_ROWS, throttle_rate=4)

if mode == "cross_cell":
    # 옛 하네스처럼 C의 무조건 변형이 **다른 셀**에서 온 상태 — W2×R1 셀의 C/Uncond만 새 그룹으로 옮긴다.
    # C가 W2에서 무조건 변형을 고르도록 더 빠르게 만들어 판정 짝(C/D, C/K)이 실제로 셀을 건너게 한다.
    newg = max(r["group"] for r in rows) + 1
    for r in rows:
        if (r["candidate"] == "C" and r["workload"] == "shuffle" and r["row_bindings"] == 1
                and r["n"] == 1000 and not r["row_variants"] and r["write_variant"] == "Unconditional"):
            r["group"] = newg
            r["samples_ms"] = [v * 0.5 for v in r["samples_ms"]]
if mode == "gc_missing":
    gs = sorted({r["group"] for r in rows if r["workload"] != "stress"})
    for r in rows:
        if r["group"] in gs[:3]:
            r["gc"] = dict(r["gc"], control="missing-binding", cell_start=None, forced=None,
                           segments=[], log_complete=False, with_gc=None, overlap_ms=None)
        elif r["group"] == gs[3]:
            r["gc"] = dict(r["gc"], segments=[dict(r["gc"]["segments"][0], forced_majorgc_events=0)])
        elif r["group"] == gs[4]:
            r["gc"] = dict(r["gc"], segments=[dict(r["gc"]["segments"][0], data_loss=True)],
                           log_complete=False, with_gc=None, overlap_ms=None)
        elif r["group"] == gs[5]:
            # 비스로틀 1차 열인데 스로틀이 걸린 셀(에이전트 인자 실수 등).
            r["gc"] = dict(r["gc"], cell_start=dict(r["gc"]["cell_start"], throttle_rate=4))
if mode == "throttle_not_applied":
    tg = sorted({t["group"] for t in THROTTLE_ROWS})[:2]
    for t in THROTTLE_ROWS:
        if t["group"] in tg:
            t["gc"] = dict(t["gc"], cell_start=dict(t["gc"]["cell_start"], throttle_rate=1))
if mode == "e_ii_margin":
    for r in rows:
        if (r["workload"] in ("swap", "shuffle10") and r["row_bindings"] == 8 and r["n"] == 1000
                and not r["row_variants"] and r["item_type"] == POD):
            m = len(r["samples_ms"])
            if r["candidate"] == "E-LIS":
                r["stage_samples_ms"]["systems"] = [0.05] * m
                r["stage_samples_ms"]["dom_apply"] = [0.3] * m
                r["stage_ms"] = dict(r["stage_ms"], systems=0.5, dom_apply=0.3)   # 마지막 rep: 발동
            elif r["candidate"] == "K":
                r["stage_samples_ms"]["dom_apply"] = [0.5] * m
                r["stage_ms"] = dict(r["stage_ms"], dom_apply=0.5)
# 판정기 사전 점검(2026-10-30, G0 재심사 발견 6): D 무조건 변형은 SetText 방출을 `systems`에서 `row_patch`로
# 옮긴다. **D·D-LIS의 단계 시간을 읽는 판정 조항이 없어야** 그 이동이 판정을 기울이지 않는다. 두 변형 모두의
# 비-total 단계 시간을 1000배로 만든다 — 어느 절이든 D의 단계 시간을 읽으면 출력이 `base`와 달라진다.
# `e_stage_control`은 같은 변환을 E-LIS에 건다(E 기각 (ii)가 읽으므로 출력이 **달라져야** 한다 — 대조가 공허하지 않다는 증거).
STAGE_SCALED = {"d_stage_inert": ("D", "D-LIS"), "e_stage_control": ("E-LIS",)}.get(mode)
if STAGE_SCALED:
    hit = 0
    for r in rows:
        if r["candidate"] in STAGE_SCALED:
            hit += 1
            for key in ("systems", "row_patch", "sync_point", "dom_apply"):
                r["stage_ms"] = dict(r["stage_ms"], **{key: r["stage_ms"][key] * 1000})
                r["stage_samples_ms"][key] = [v * 1000 for v in r["stage_samples_ms"][key]]
    assert hit > 0, f"{mode}: 바꾼 레코드가 0개 — 대조가 공허하다"
    print(f"{mode}: 단계 시간을 바꾼 레코드 {hit}개")
if mode == "stage_median":
    for r in rows:
        if r["candidate"] == "C" and r["row_variants"] and r["workload"] == "shuffle":
            m = len(r["samples_ms"])
            r["stage_ms"] = dict(r["stage_ms"], sync_point=0.6, total=1.0)       # 마지막 rep: 60%
            r["stage_samples_ms"]["total"] = [1.0] * m
            r["stage_samples_ms"]["sync_point"] = [0.3] * (m - 1) + [0.6]      # rep별 median: 30%

FF_ROWS = None
if mode in ("firefox_bidi", "firefox_wrong_file"):
    import copy
    FF_ROWS = copy.deepcopy(rows)
    fgs = sorted({r["group"] for r in FF_ROWS if r["workload"] != "stress"})
    for r in FF_ROWS:
        g = r["gc"]
        if mode == "firefox_wrong_file":
            r["gc"] = {"control": "unavailable", "source": g["source"], "cell_start": None, "forced": None,
                       "segments": [], "log_complete": False, "with_gc": None, "overlap_ms": None}
            continue
        seg = dict(g["segments"][0], forced_majorgc_events=1, forced_pause_events=3, forced_cc_events=2)
        if r["group"] == fgs[0]:
            seg["forced_pause_events"] = 0
        elif r["group"] == fgs[1]:
            seg["forced_cc_events"] = 0
        r["gc"] = dict(g, cell_start={"ok": True, "throttle_rate": 1, "agent": "ff-bidi-agent",
                                      "tracing": "gecko-profiler GeckoMain markers via WebDriver BiDi; union = GCSlice|GCMinor|CCSlice"},
                       forced={"ok": True, "method": "child-gc-request → child-cc-request → child-gc-request"},
                       segments=[seg])
        if (r["candidate"] == "K" and r["workload"] == "swap" and r["row_bindings"] == 1 and r["n"] == 1000
                and not r["row_variants"] and r["item_type"] == POD):
            r["over_cell_cap"] = True
            r["samples_ms"] = [v * 0.3 for v in r["samples_ms"][:40]]
            r["cell_wall_ms"] = 1_800_500.0

os.makedirs(out_dir, exist_ok=True)
if FF_ROWS is not None:
    with open(os.path.join(out_dir, f"rq1-phase1-track-l-core-grid-{VERSION}-firefox-bidi.json"), "w") as f:
        json.dump({"track_l": FF_ROWS, "track_t": []}, f)
if THROTTLE_ROWS is not None:
    with open(os.path.join(out_dir, f"rq1-phase1-track-l-core-grid-{VERSION}-chromium-throttle4.json"), "w") as f:
        json.dump({"track_l": THROTTLE_ROWS, "track_t": []}, f)
with open(os.path.join(out_dir, f"rq1-phase1-track-l-core-grid-{VERSION}-chromium.json"), "w") as f:
    json.dump({"track_l": rows, "track_t": []}, f)
print(len(rows), "rows ->", out_dir, VERSION)
