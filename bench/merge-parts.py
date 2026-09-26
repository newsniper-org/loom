#!/usr/bin/env python3
"""여러 조각 파일을 group 순으로 병합해 §9 판정기가 읽는 데이터 파일을 만든다.

    python3 bench/merge-parts.py <조각파일...> [--out <경로>] [--expect 69]

조각은 여러 줄에 걸친 JSON이 이어 붙은 스트림이라 줄 단위로 못 읽는다
(`collect-stock.sh`의 병합 단계와 같은 디코더를 쓴다).

**그룹 번호가 겹치거나 빠지면 실패한다.** 수집이 나뉘었을 때 조용히 일부를
덮어쓰거나 빠뜨리면 "표본이 적은 셀"이 아니라 "없는 셀"이 되고, 판정기는 그것을
미계측으로 읽는다 — 왜 없는지는 알 수 없게 된다.
"""
import json
import sys
from collections import Counter

args = [a for a in sys.argv[1:]]
out = "research-questions/data/rq1-phase1-track-l-core-grid-v21-chromium.json"
expect = None
if "--out" in args:
    i = args.index("--out"); out = args[i + 1]; del args[i:i + 2]
if "--expect" in args:
    i = args.index("--expect"); expect = int(args[i + 1]); del args[i:i + 2]
if not args:
    print(__doc__); sys.exit(2)

dec = json.JSONDecoder()
parts = []
for path in args:
    p = path if "/" in path else f"bench/collected/{path}"
    raw = open(p).read()
    i, n0 = 0, len(parts)
    seg = p.rsplit("/", 1)[-1]
    while i < len(raw):
        while i < len(raw) and raw[i].isspace():
            i += 1
        if i >= len(raw):
            break
        obj, i = dec.raw_decode(raw, i)
        # **각 레코드에 출처 구간을 찍는다.** 수집이 나뉘면 구간마다 머신 상태가
        # 다를 수 있고 그 차이가 수치에 들어간다 — 2026-09-26 v21이 실제로 그랬다
        # (1차 구간은 스왑 44/46 GB 압박, 재개 구간은 여유). 특히 §8이 (후보 ×
        # 시행)마다 셀 하나로 정의한 W6는 시행끼리도 다른 시각에 돌므로 **같은
        # 후보의 시행들조차 서로 비교가 안 될 수 있다.** 그 사실을 README에만
        # 적으면 데이터를 직접 읽는 쪽은 모른다. 하네스가 아니라 병합 시점에 붙는
        # 메타데이터이므로 §2.5 동결과 무관하다.
        for r in obj.get("track_l", []):
            r["_segment"] = seg
        for r in obj.get("track_t", []):
            r["_segment"] = seg
        parts.append(obj)
    print(f"  {p}: 조각 {len(parts) - n0}개  (구간 표식 _segment={seg!r})")

groups = [p["group"] for p in parts]
dup = {g for g in groups if groups.count(g) > 1}
if dup:
    sys.exit(f"**그룹 번호 중복 {sorted(dup)}** — 같은 셀을 두 번 세게 된다")
missing = [g for g in range(min(groups), max(groups) + 1) if g not in groups]
if missing:
    sys.exit(f"**그룹 번호 누락 {missing}** — 그 셀은 판정기에서 미계측이 된다")
if expect is not None and len(parts) != expect:
    sys.exit(f"**조각 {len(parts)}개, 기대 {expect}개** — 구간이 덜 모였다")

parts.sort(key=lambda p: p["group"])
track_l = [r for p in parts for r in p["track_l"]]
track_t = [r for p in parts for r in p["track_t"]]
json.dump({"track_l": track_l, "track_t": track_t}, open(out, "w"), indent=1)
print(f"  → {out}: 그룹 {len(parts)} ({min(groups)}..{max(groups)}), "
      f"트랙 L {len(track_l)} / 트랙 T {len(track_t)} 레코드")

segs = Counter(r["_segment"] for r in track_l)
if len(segs) > 1:
    print("  ⚠️ 여러 구간에서 병합됨 — 구간마다 머신 상태가 다를 수 있다:")
    for k, v in sorted(segs.items()):
        print(f"      {k}: {v} 레코드")
    print("     특히 W6는 (후보 × 시행)마다 셀 하나라 시행끼리도 다른 시각에 돈다 —")
    print("     같은 후보의 시행들조차 서로 비교가 안 될 수 있다.")
