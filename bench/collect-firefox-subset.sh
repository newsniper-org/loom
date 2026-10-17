#!/usr/bin/env bash
# §8 커버리지 표의 Firefox 요구분 — "L: §9 판정 관여 셀 전부(교차 확인)" + "L 지도·스케일링 셀:
# 스케일링 셀만" — 만 **그룹 단위로** 수집한다.
#
# 왜 CELLS 모드를 새로 만들지 않나: 셀 선택 필터는 wasm 안에 있어서 모드를 추가하면 하네스
# 바이너리가 바뀐다(§2.5). 대신 드라이버의 FIRST_GROUP/TOTAL_GROUPS로 한 그룹씩 돈다.
#
# **그룹 목록(개정 #10, v22 — 16그룹)**: 셀 목록이 58셀로 바뀌어(write 변형이 같은 셀의 후보가
# 됨) 셔플 순서도 바뀌었다. 목록은 기억이 아니라 대응표에서 뽑았다:
#   cargo test -p loom-bench -- --ignored --nocapture print_cell_order
#   판정 관여 10: {W1,W2,W3,W4}×{R1,R2} POD n=1,000 (C 채택 (a)(b)·C 기각 앞 분기·D 기각 (iii)·
#                 D 양성·E 기각 (i)(ii)) + W2×R3 (C 기각 뒷 분기) + W5×R1 (D W5 부속)
#   스케일링 6:   {W2,W4}×R2×n∈{100,10,000} + W2×R1×n∈{100,10,000}
# (v21의 24그룹은 무조건 변형이 별도 셀이던 69셀 목록 기준이었다.)
#
# **개정 #10 후속 3(2026-10-17)**: 자체 루프를 버리고 `collect-per-cell.sh`의 그룹 목록 모드로 돈다 — 그룹마다
# 새 브라우저·잔존 프로세스 검사·연속 3실패 정지·로그를 bench/collected에 두는 것을 같은 코드로 얻는다.
# 그룹마다 부하 평균이 LOAD_MAX(기본 4) 이하가 될 때까지 기다린다(CPU 경합은 셀을 상한에 걸리게 한다).
#
# 사용: PREFIX=v22-firefox-bidi bash bench/collect-firefox-subset.sh
# PREFIX는 **필수**다 — 기존 수집분(v22-firefox, 1d85c4f, 에이전트 없음)을 같은 이름으로 덮어쓰지 않게.
set -u
PREFIX="${PREFIX:?PREFIX 필수 (예: v22-firefox-bidi) — 기존 수집분을 덮어쓰지 않게}"
cd "$(dirname "$0")/.."
if ls bench/collected/"$PREFIX"-g*.part.json >/dev/null 2>&1; then
  echo "**bench/collected/$PREFIX-g*.part.json이 이미 있다** — 다른 PREFIX를 쓰거나 직접 옮길 것" >&2
  exit 1
fi
GROUP_LIST="6 7 9 11 24 26 30 31 34 36 38 39 48 52 55 57" \
CELLS=all THROTTLE=1 PORT="${PORT:-8931}" LOAD_MAX="${LOAD_MAX:-4}" \
  exec bash bench/collect-per-cell.sh /usr/bin/firefox "$PREFIX" 0 0 "${PER_GROUP_SEC:-2400}"
