#!/usr/bin/env bash
# **셀마다 새 브라우저** (개정 #10 후속 2, 2026-09-27) — 트랙 L 수집의 기본 경로.
#
# 왜: `collect-stock.sh` 한 번은 브라우저 하나를 띄우고 페이지가 스스로 다음 그룹으로 이동한다. §8의
# "셀마다 fresh page load"는 문언대로 지켜지지만 **렌더러 프로세스는 셀을 건너 재사용된다**. v22 첫
# 수집(2026-09-27)이 그 상태에서 W6 K 셀마다 렌더러 10~14GB·GC 비중 98%의 나선에 빠져 23/58에서
# 정지 감지로 멈췄다. 같은 셀을 새 브라우저의 첫 셀로 돌리면 10,000삽입을 112초·425MB에 끝낸다
# (재현 3회: 정지 수집의 그룹 14·23, 드라이버 그대로 21→22→23 재현 — 앞 셀 뒤에는 2,500삽입에 이미
# 4.7GB). 앞 셀이 남기는 것은 **메모리가 아니라 프로세스 상태**다(셀 사이에 RSS·시스템 가용 메모리는
# 회복된다). 메커니즘은 미확인이고(bfcache 가설은 반박됐다), 이 경로는 메커니즘과 무관하게 셀 간
# 상태 전이를 끊는다.
#
# 사용:
#   bench/collect-per-cell.sh <브라우저> <수집이름> <첫 그룹> <끝 그룹(제외)> [그룹당 상한초]
#   (THROTTLE=4 CELLS=core 등 collect-stock.sh의 환경 변수는 그대로 전달된다)
# 그룹 g의 산출물은 bench/collected/<수집이름>-g<g>.* 이고, 끝나면 조각을 합친다:
#   python3 bench/merge-parts.py bench/collected/<수집이름>-g*.part.json --expect <그룹 수> --out <경로>
#
# 한 그룹이 실패해도 **다음 그룹으로 간다** — 셀마다 새 브라우저라 실패가 다음 셀을 오염시키지 않는다.
# 실패 그룹은 끝에 목록으로 찍고(병합의 --expect가 다시 잡는다), 재실행은 지시가 있을 때만 한다.
# 연속 3그룹이 실패하면 체계적 결함(에이전트·서버·브라우저)으로 보고 멈춘다.
set -u
BIN="${1:?브라우저}"; BASE="${2:?수집 이름}"; FIRST="${3:?첫 그룹}"; END="${4:?끝 그룹(제외)}"
PER="${5:-3600}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
FAILED=(); STREAK=0
echo "[per-cell] $BASE 그룹 $FIRST..$((END-1)) 시작 $(date +%T) (그룹당 상한 ${PER}초)"
for ((g = FIRST; g < END; g++)); do
  t0=$(date +%s)
  FIRST_GROUP=$g TOTAL_GROUPS=$((g + 1)) bash bench/collect-stock.sh "$BIN" "$BASE-g$g" "$PER" \
    > "bench/collected/$BASE-g$g.driver.log" 2>&1
  rc=$?
  n=$(grep -c '"total_cells":' "bench/collected/$BASE-g$g.part.json" 2>/dev/null || echo 0)
  left=$(ps -eo args | grep -E "user-data-dir=/tmp/tmp|profile /tmp/tmp|Xvfb :" | grep -v grep | wc -l)
  echo "[per-cell] 그룹 $g rc=$rc 조각=$n $(( $(date +%s) - t0 ))초 잔존=$left — $(tail -1 "bench/collected/$BASE-g$g.driver.log")"
  if [ "$rc" -ne 0 ] || [ "$n" -ne 1 ]; then
    FAILED+=("$g"); STREAK=$((STREAK + 1))
    if [ "$STREAK" -ge 3 ]; then
      echo "[per-cell] **연속 3그룹 실패 — 체계적 결함으로 보고 멈춘다** (실패: ${FAILED[*]})"
      exit 2
    fi
  else
    STREAK=0
  fi
  if [ "$left" -ne 0 ]; then
    echo "[per-cell] **잔존 하네스 프로세스 $left개 — 다음 그룹이 오염될 수 있어 멈춘다**"
    exit 3
  fi
done
if [ "${#FAILED[@]}" -gt 0 ]; then
  echo "[per-cell] $BASE 끝 $(date +%T) — **실패 그룹: ${FAILED[*]}**"
  exit 1
fi
echo "[per-cell] $BASE 전 그룹 완료 $(date +%T)"
