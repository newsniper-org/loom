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
# 그룹마다 브라우저를 새로 띄운다 — §8 "셀마다 fresh page load"보다 강한 격리다. 드라이버가
# `&end=`를 넘기므로 페이지가 범위 밖 그룹으로 스스로 이동하지 않고, 정리는 세션 전체를 죽인다
# (둘 다 2026-09-27 스모크에서 드러난 결함의 수리 — 전에는 구간 수집 뒤 브라우저가 살아남아
# 다음 그룹을 계속 쟀다).
set -u
cd "$(dirname "$0")/.."
JOB=/home/ybi/.claude/jobs/fcb33b66/tmp
for g in 6 7 9 11 24 26 30 31 34 36 38 39 48 52 55 57; do
  echo "=== $(date +%H:%M:%S) 그룹 $g 시작 ==="
  FIRST_GROUP=$g TOTAL_GROUPS=$((g+1)) PORT=${PORT:-8931} \
    bash bench/collect-stock.sh /usr/bin/firefox v22-firefox-g$g 2400 > $JOB/ff-g$g.log 2>&1
  rc=$?
  echo "=== $(date +%H:%M:%S) 그룹 $g rc=$rc $(tail -1 $JOB/ff-g$g.log) ==="
  # 한 그룹의 실패로 전체를 멈추지 않는다 — 실패는 병합 단계의 --groups 검사가 잡는다.
done
echo "=== $(date +%H:%M:%S) Firefox 부분 집합 수집 종료 ==="
