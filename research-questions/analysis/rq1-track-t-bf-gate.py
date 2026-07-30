"""RQ1-benchmark-protocol.md §9 B/F 조항을 트랙 T f축 캠페인 데이터에 적용.

오늘 적용 **가능한** 것과 **불가능한** 것을 먼저 못박는다:

- **불가능 — F 기각 1차 분기**: "F-i/ii/iii 각각에 대해, 해당 변형이 B를 이기는
  셀이 통상 구간(f ∈ [0.1,10]/s × s ∈ [1,100]) 안에 하나도 없으면 그 변형 기각".
  `cond.rs`에는 F-i(`CondEffective` 값 필터)/F-ii(detached DOM 계속 패치)/
  F-iii(`Without<Frozen>` marker)가 **구현 자체가 없다**. 오늘의 F는 자기 doc
  comment가 밝히듯 "CSS 은닉(v-show형) 대조군을 겸하는" 변형이라, 그걸 F-i/ii/iii
  자리에 대입해 판정하면 카테고리 오류다.

- **가능 — "또는" 분기**: "**또는** B의 spawn 비용이 전 그리드에서 < 1ms/100
  entity — 단 **비스로틀과 4× 스로틀 양쪽 모두에서 성립할 때만 발동**. 두 경우
  모두 결론 = B 유지." 이 분기는 **B만 측정하면 된다**. §11 개정 #1(2026-07-28)이
  이 비율을 재는 셀을 **s=100 행으로 고정**했다(개정 사유·공평성 진술은
  RQ1-benchmark-protocol.md §11-2 참고).

따라서 이 스크립트는 "또는" 분기만 기계적으로 적용하고, 1차 분기는 적용 불가로
명시 출력한다.

사용법:
    python3 rq1-track-t-bf-gate.py \
        --unthrottled ../data/rq1-track-t-frequency-chromium-b-unthrottled.json \
        --throttled   ../data/rq1-track-t-frequency-chromium-b-throttled4x.json
"""

import argparse
import json
import statistics

# §11 개정 #1 — 이 비율을 재는 셀.
JUDGMENT_SIZE = 100
# §9 "또는" 분기의 사전 등록 임계값.
THRESHOLD_MS_PER_100_ENTITY = 1.0
# §8 타이머 조항의 배치 규칙: median 추정치 < 해상도 × 50이면 배치 측정 전환.
BATCHING_RULE_MULTIPLIER = 50


def load(path):
    with open(path) as f:
        return json.load(f)


def cells(payload):
    """(f_hz, size) -> 그 셀의 전 시행 toggle_on 표본을 합친 리스트."""
    out = {}
    for r in payload["records"]:
        out.setdefault((r["f_hz"], r["size"]), []).extend(r["toggle_on_ms"])
    return out


def resolutions(payload):
    return sorted({r["timer_resolution_ms"] for r in payload["records"]})


def isolated_ok(payload):
    return all(r["cross_origin_isolated"] for r in payload["records"])


def report_arm(payload, label):
    print(f"\n{'=' * 78}\n{label}\n{'=' * 78}")
    print(f"  격자: {payload['grid']}   시행/셀: {payload['trials_per_cell']}   "
          f"창: {payload['window_ms']:.0f}ms   스로틀: {payload['cpu_throttle_rate']}×")
    print(f"  사전 등록 조건 충족 실행인가: {payload['is_preregistered_run']}")
    res = resolutions(payload)
    print(f"  실측 타이머 해상도: {res} ms   crossOriginIsolated 전 셀 참: {isolated_ok(payload)}")
    if not isolated_ok(payload):
        print("  ⚠️  §8 타이머 조항 미충족 데이터 — 판정에 쓸 수 없다.")

    batching_threshold = max(res) * BATCHING_RULE_MULTIPLIER
    c = cells(payload)

    print(f"\n  -- spawn 비용 프로파일 (toggle_on = B의 spawn 경로) --")
    print(f"  {'f(/s)':>7} {'s':>6} {'n':>5} {'median(ms)':>11} {'ms/100ent':>10}  배치규칙")
    judgment_rows = []
    for (f_hz, size) in sorted(c):
        samples = c[(f_hz, size)]
        if not samples:
            print(f"  {f_hz:7.1f} {size:6d} {0:5d} {'—':>11} {'—':>10}  표본 없음")
            continue
        med = statistics.median(samples)
        per100 = med / size * 100
        needs_batching = med < batching_threshold
        flag = "필요(해상도 바닥)" if needs_batching else "불필요"
        mark = " ←판정셀" if size == JUDGMENT_SIZE else ""
        print(f"  {f_hz:7.1f} {size:6d} {len(samples):5d} {med:11.4f} {per100:10.4f}  {flag}{mark}")
        if size == JUDGMENT_SIZE:
            judgment_rows.append((f_hz, med, per100, needs_batching))

    if not judgment_rows:
        print(f"\n  ⚠️  s={JUDGMENT_SIZE} 행이 데이터에 없다 — 이 실행으로는 판정 불가.")
        return None

    worst = max(r[2] for r in judgment_rows)
    any_batching = any(r[3] for r in judgment_rows)
    holds = worst < THRESHOLD_MS_PER_100_ENTITY
    print(f"\n  s={JUDGMENT_SIZE} 행 최댓값: {worst:.4f} ms/100 entity "
          f"(임계값 {THRESHOLD_MS_PER_100_ENTITY})  ⇒ {'성립' if holds else '불성립'}")
    if any_batching:
        print("  ⚠️  판정 셀 중 일부가 §8 배치 규칙 발동 구간(median < 해상도×50) — "
              "그 셀의 median은 해상도 바닥에 눌려 있어 신뢰도가 낮다.")
    return {"holds": holds, "worst_per100": worst, "needs_batching": any_batching}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--unthrottled", required=True)
    ap.add_argument("--throttled", default=None)
    args = ap.parse_args()

    un = report_arm(load(args.unthrottled), "비스로틀 (1×)")
    th = report_arm(load(args.throttled), "4× CPU 스로틀") if args.throttled else None

    print(f"\n{'=' * 78}\n§9 B/F 조항 적용 결과\n{'=' * 78}")
    print("""
  [1차 분기 — F-i/ii/iii 각각의 통상 구간 승리 셀 부재 여부]
    **적용 불가**. cond.rs에 F-i(CondEffective 값 필터)/F-ii(detached DOM 계속
    패치)/F-iii(Without<Frozen> marker)가 구현돼 있지 않다. 오늘의 F는 CSS
    은닉(v-show형) 대조군을 겸하는 변형이므로 그 자리에 대입할 수 없다.
""")
    print("  [\"또는\" 분기 — B의 spawn 비용 < 1ms/100 entity, 양쪽 스로틀 조건 모두]")
    if un is None:
        print("    판정 불가 — 비스로틀 데이터에 판정 셀이 없다.")
        return
    print(f"    비스로틀 : {un['worst_per100']:.4f} ms/100ent  → {'성립' if un['holds'] else '불성립'}")
    if th is None:
        print("    4× 스로틀: **미측정** — §9가 '비스로틀과 4× 스로틀 양쪽 모두에서")
        print("               성립할 때만 발동'을 명시하므로, 이것만으로는 분기가")
        print("               발동하지 않는다(발동 조건 불성립, 어느 쪽으로도 불계상).")
        return
    print(f"    4× 스로틀: {th['worst_per100']:.4f} ms/100ent  → {'성립' if th['holds'] else '불성립'}")

    fired = un["holds"] and th["holds"]
    print()
    if fired:
        print("    ⇒ **분기 발동** — §9 문언대로 결론 = **B 유지**(F 계열 기각).")
        print("       단, 이 결론은 F-i/ii/iii를 실측해서가 아니라 'B가 충분히 싸다'는")
        print("       충분조건으로 도달한 것임을 판정 서술에 함께 남길 것.")
    else:
        print("    ⇒ 분기 미발동 — B/F 판정은 1차 분기에 달려 있고, 그건 F-i/ii/iii")
        print("       구현 없이는 적용 불가하므로 트랙 T는 **여전히 판정 불가**다.")
    if un["needs_batching"] or (th and th["needs_batching"]):
        print("\n    ⚠️  판정 셀에 §8 배치 규칙 발동 구간이 포함됨 — 위 판정을 확정하기 전에")
        print("        해당 셀을 배치 측정(k=10 묶음)으로 재수집해야 한다.")


if __name__ == "__main__":
    main()
