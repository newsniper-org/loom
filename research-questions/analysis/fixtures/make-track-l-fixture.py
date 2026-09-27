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

모드:
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
for r in list(rows):
    if r["workload"] in ("swap", "shuffle", "shuffle10", "head_insert", "replace50", "replace100"):
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
    for r in rows:
        if (r["workload"] == "swap" and r["row_bindings"] == 1 and r["n"] == 1000
                and r["write_variant"] == "Conditional" and not r["row_variants"]):
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

os.makedirs(out_dir, exist_ok=True)
if THROTTLE_ROWS is not None:
    with open(os.path.join(out_dir, "rq1-phase1-track-l-core-grid-v21-chromium-throttle4.json"), "w") as f:
        json.dump({"track_l": THROTTLE_ROWS, "track_t": []}, f)
with open(os.path.join(out_dir, "rq1-phase1-track-l-core-grid-v21-chromium.json"), "w") as f:
    json.dump({"track_l": rows, "track_t": []}, f)
print(len(rows), "rows ->", out_dir)
