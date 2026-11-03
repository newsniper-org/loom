#!/usr/bin/env python3
"""G1 원장 분석 (RQ1 프로토콜 개정 #12 후속 8 — 통과 기준과 비공허 단언).

사용: python3 bench/g1/analyze-g1.py <원장.json> [<원장.json> ...]   (같은 브라우저·빌드의 분할들을 함께 넘긴다)

판정 순서는 등재 문언대로 **비공허 단언이 먼저**다. 하나라도 깨지면 "검사가 공허하다"로 끝내고 통과/불통과를 말하지 않는다.
- 분할 합집합이 셀 목록 전체를 덮는다(트랙 L·T 모두, 중복 없음), 오류로 끝난 셀이 없다.
- 셀마다 인스턴스별 전이 수 > 0, 내용 신탁 수 > 0.
- 정체성: 인스턴스 의미론별로 "같은 노드" > 0 이고 "새 노드" > 0 (전체 합 기준).
- K: NoOp·Append·Truncate·Clear·Full 각 ≥ 1 (음성 대조 셀 제외).
- E·E-LIS: OrderKey 검사 수 > 0, 이동이 있었던 전이 수 > 0.
- 트랙 T: arm마다 참→거짓·거짓→참·참→참 각 ≥ 1, 값 변경을 겪은 arm(B 제외)은 꺼진 중 값 변경 ≥ 1, catch-up 검사 ≥ 1.
- 음성 대조: neg 셀의 인스턴스 **전부**가 정체성 불일치 > 0 (틀린 의미론을 실제로 잡는다).
"""
import collections, json, sys

MISMATCH = ("content_mismatch", "anchor_mismatch", "count_mismatch", "ident_mismatch", "orderkey_mismatch")


def main(paths):
    ls, ts, errs, metas = {}, {}, [], []
    for p in paths:
        d = json.load(open(p))
        metas.append(d["meta"])
        for c in d["track_l"]:
            assert c["cell"] not in ls, f"트랙 L 셀 중복: {c['cell']}"
            ls[c["cell"]] = c
        for c in d["track_t"]:
            assert c["cell"] not in ts, f"트랙 T 셀 중복: {c['cell']}"
            ts[c["cell"]] = c
        errs += d["errors"]
    shas = {m["wasm_sha256"] for m in metas}
    exp_l, exp_t = metas[0]["cells"]["track_l"], metas[0]["cells"]["track_t"]
    vac = []
    if len(shas) != 1:
        vac.append(f"분할의 산출물 sha가 다르다: {shas}")
    if len(ls) != exp_l or len(ts) != exp_t:
        vac.append(f"셀 수 L {len(ls)}/{exp_l}, T {len(ts)}/{exp_t}")
    if errs:
        vac.append(f"오류로 끝난 셀 {len(errs)}개")
    tot = collections.Counter()
    sem_same, sem_new = collections.Counter(), collections.Counter()
    for cell, c in ls.items():
        neg = cell.endswith("|neg")
        for name, v in c["instances"].items():
            s = v["stats"]
            if s["transitions"] == 0:
                vac.append(f"{cell} {name}: 전이 0")
            if not neg and s["content_checked"] == 0:
                vac.append(f"{cell} {name}: 내용 신탁 0")
            if neg:
                if s["ident_mismatch"] == 0:
                    vac.append(f"음성 대조가 불일치를 못 잡았다: {cell} {name} ({v['sem']})")
                continue
            sem_same[v["sem"]] += s["ident_same"]
            sem_new[v["sem"]] += s["ident_new"]
            if name.startswith("K"):
                for k, n in s["k"].items():
                    tot["k_" + k] += n
            if name.startswith("E"):
                tot["e_orderkey"] += s["orderkey_checked"]
                tot["e_moved"] += s["moved_ops"]
            for m in MISMATCH:
                tot[m] += s[m]
            tot["transitions"] += s["transitions"]
            tot["content_checked"] += s["content_checked"]
            tot["unchecked_steps"] += s.get("unchecked_steps", 0)
    for sem in ("Keyed", "Slots", "K"):
        if sem_same[sem] == 0 or sem_new[sem] == 0:
            vac.append(f"정체성 {sem}: 같은 노드 {sem_same[sem]}, 새 노드 {sem_new[sem]}")
    for k in ("noop", "append", "truncate", "clear", "full"):
        if tot["k_" + k] == 0:
            vac.append(f"K 분기 {k} 발동 0")
    if tot["e_orderkey"] == 0 or tot["e_moved"] == 0:
        vac.append(f"E: OrderKey 검사 {tot['e_orderkey']}, 이동 {tot['e_moved']}")
    tarm = collections.defaultdict(collections.Counter)
    for cell, c in ts.items():
        arm = cell.split("|")[0]
        for k in ("on", "off", "true_true", "change_while_off", "catchup_checked", "mismatch", "redundant_writes", "settle_writes"):
            tarm[arm][k] += c[k]
    for arm, cnt in tarm.items():
        for k in ("on", "off", "true_true"):
            if cnt[k] == 0:
                vac.append(f"트랙 T {arm}: {k} 0")
        if arm != "B" and (cnt["change_while_off"] == 0 or cnt["catchup_checked"] == 0):
            vac.append(f"트랙 T {arm}: 꺼진 중 값 변경 {cnt['change_while_off']}, catch-up {cnt['catchup_checked']}")

    print(f"원장 {len(paths)}개, 산출물 {sorted(shas)}, 브라우저 {metas[0].get('browser')} {metas[0].get('browser_version')}, pkg {metas[0]['pkg']}, "
          f"debug_assertions {metas[0]['g1_meta_start']['debug_assertions']}, 폴백 강제 {metas[0]['fallback_forced']}")
    print(f"트랙 L 셀 {len(ls)}/{exp_l}, 전이 {tot['transitions']}, 내용 신탁 {tot['content_checked']}, W6 미검사 스텝 {tot['unchecked_steps']}; "
          f"정체성 같은/새 {dict(sem_same)}/{dict(sem_new)}; K {[(k, tot['k_'+k]) for k in ('noop','append','truncate','clear','full')]}; "
          f"E OrderKey {tot['e_orderkey']} 이동 {tot['e_moved']}")
    print(f"트랙 T 셀 {len(ts)}/{exp_t}: " + "; ".join(f"{a} {dict(c)}" for a, c in sorted(tarm.items())))
    stale = [ (cell, c["stale_lref"]) for cell, c in ts.items() if c.get("stale_lref") ]
    print("stale LRef(B, 기록만):", {k: sum(s[k] for _, s in stale) for k in ("checked", "resolved", "index_reused")})
    if vac:
        print("\n**비공허 단언 실패 — 판정하지 않는다**:")
        for v in vac[:40]:
            print("  -", v)
        return 2
    bad_l = collections.Counter()
    for cell, c in ls.items():
        if cell.endswith("|neg"):
            continue
        for name, v in c["instances"].items():
            ms = [m for m in MISMATCH if v["stats"][m]]
            if ms:
                bad_l[(name, cell.split("|")[2], tuple(ms))] += 1
    bad_t = {cell: (c["mismatch"], c["redundant_writes"], c["failures"][:1]) for cell, c in ts.items() if c["mismatch"]}
    print(f"\n비공허 단언 전부 성립. 트랙 L 불일치 {sum(tot[m] for m in MISMATCH)}건 {dict(bad_l)}")
    print(f"트랙 T 불일치 셀 {len(bad_t)}개:")
    for cell, x in sorted(bad_t.items()):
        print("  ", cell, x)
    ok = not bad_l and not bad_t
    print("\nG1 판정:", "통과" if ok else "불통과(불일치 있음)")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
