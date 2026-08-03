#!/usr/bin/env bash
# stock(공식 배포판) 브라우저로 트랙 L + 트랙 T를 수집한다.
#
# **왜 이게 필요한가 (2026-08-03)**: 지금까지 수집은 Playwright가 번들한
# 브라우저 빌드로만 가능했는데, 조사 결과 Playwright의 Firefox 빌드가 stock
# Firefox와 크게 다르다는 것이 확인됐다 — 같은 wasm 바이너리로 stock Firefox
# 150.0.1은 Chromium과 동률(0.91×)인 반면 Playwright 빌드는 8.1~8.4배 느리고,
# 그 바이너리를 Playwright 없이 직접 띄워도 동일하다(드라이버가 아니라 빌드).
# §8이 "Chromium stable / Firefox stable"을 사전 등록했으므로 stable 배포판으로
# 재는 것이 조항 문언에 맞다.
#
# 사용:
#   bench/serve-isolated.py 8931 을 띄운 뒤
#   bench/collect-stock.sh <브라우저 실행파일> <출력이름> [타임아웃초]
# 예:
#   bench/collect-stock.sh /usr/bin/firefox v16-firefox 10800
#   bench/collect-stock.sh /usr/bin/google-chrome-stable v16-chromium 10800
set -euo pipefail

BIN="${1:?브라우저 실행파일 경로}"
NAME="${2:?출력 이름 (확장자 없이)}"
TIMEOUT="${3:-10800}"
PORT="${PORT:-8931}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/bench/collected/$NAME.json"
PROFILE="$(mktemp -d)"

rm -f "$OUT"
URL="http://localhost:$PORT/demo/list_bench.html?collect=$NAME"

case "$BIN" in
  *firefox*)
    # `--no-remote` + 빈 프로필: 이미 떠 있는 사용자 세션에 붙지 않게.
    "$BIN" --headless --no-remote --profile "$PROFILE" "$URL" >/dev/null 2>&1 &
    ;;
  *)
    "$BIN" --headless=new --no-first-run --no-default-browser-check \
      --user-data-dir="$PROFILE" \
      --enable-features=SharedArrayBuffer "$URL" >/dev/null 2>&1 &
    ;;
esac
PID=$!
# 정리는 **절대 스크립트를 죽이면 안 된다**. 2026-08-03에 Chrome이 프로필
# 디렉터리에 아직 쓰는 중이라 `rm -rf`가 실패했고, `set -e` + trap 조합에서
# 그 실패가 스크립트 종료 코드가 되어 뒤이은 Firefox 수집이 통째로 건너뛰어졌다.
cleanup() {
  local rc=$?
  kill "$PID" 2>/dev/null || true
  sleep 1
  rm -rf "$PROFILE" 2>/dev/null || true
  return $rc
}
trap cleanup EXIT

echo "[collect-stock] $NAME 시작 (pid=$PID, 상한 ${TIMEOUT}초)"
START=$(date +%s)
while [ ! -f "$OUT" ]; do
  if ! kill -0 $PID 2>/dev/null; then
    echo "[collect-stock] **브라우저가 죽었다** — 결과 없음" >&2
    exit 1
  fi
  NOW=$(date +%s); EL=$((NOW-START))
  if [ "$EL" -gt "$TIMEOUT" ]; then
    echo "[collect-stock] 타임아웃 ${TIMEOUT}초 초과" >&2
    exit 1
  fi
  if [ $((EL % 60)) -eq 0 ] && [ "$EL" -gt 0 ]; then
    echo "[collect-stock] $((EL/60))분 경과"
  fi
  sleep 5
done
echo "[collect-stock] $NAME 완료 ($((($(date +%s)-START)/60))분) → $OUT"
exit 0
