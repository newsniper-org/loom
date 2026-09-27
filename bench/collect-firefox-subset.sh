#!/usr/bin/env bash
# §8 커버리지 표의 Firefox 요구분 — "L: §9 판정 관여 셀 전부(교차 확인)" + "L 지도·스케일링 셀:
# 스케일링 셀만" — 만 **그룹 단위로** 수집한다.
#
# 왜 CELLS 모드를 새로 만들지 않나: 셀 선택 필터는 wasm 안에 있어서 모드를 추가하면 하네스
# 바이너리가 바뀐다. §2.5("하네스 변경은 캠페인 무효")상 v21 캠페인 도중 빌드를 갈아 끼우지
# 않는다 — 2026-09-26에 W6 계측 추가를 같은 이유로 되돌렸다. 대신 드라이버의 기존
# FIRST_GROUP/TOTAL_GROUPS로 한 그룹씩 돈다(스모크 때 쓴 방식). 그룹 번호는 기억이 아니라
# **v21 크롬 수집의 실제 조각 파일**에서 뽑았다(그룹 → 셀 매핑은 같은 바이너리에서 결정적).
#
# 그룹마다 브라우저를 새로 띄운다 — §8 "셀마다 fresh page load"보다 강한 격리다.
# 그룹 목록(24): 2 4 11 14 15 16 17 18 19 22 23 25 28 32 41 45 46 48 50 52 53 57 60 68
set -u
cd "$(dirname "$0")/.."
JOB=/home/ybi/.claude/jobs/fcb33b66/tmp
for g in 2 4 11 14 15 16 17 18 19 22 23 25 28 32 41 45 46 48 50 52 53 57 60 68; do
  echo "=== $(date +%H:%M:%S) 그룹 $g 시작 ==="
  FIRST_GROUP=$g TOTAL_GROUPS=$((g+1)) PORT=${PORT:-8931} \
    bash bench/collect-stock.sh /usr/bin/firefox v21-firefox-g$g 2400 > $JOB/ff-g$g.log 2>&1
  rc=$?
  echo "=== $(date +%H:%M:%S) 그룹 $g rc=$rc $(tail -1 $JOB/ff-g$g.log) ==="
  # 한 그룹의 실패로 전체를 멈추지 않는다 — 실패는 병합 단계의 --groups 검사가 잡는다.
done
echo "=== $(date +%H:%M:%S) Firefox 부분 집합 수집 종료 ==="
