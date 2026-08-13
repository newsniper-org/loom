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
# **래퍼를 우회한다 (2026-08-12, 실제로 당했다)**: `/usr/bin/google-chrome-stable`은
# `~/.config/chrome-flags.conf`를 읽어 사용자의 개인 플래그를 주입하는 bash 래퍼다.
# 이 머신에서는 거기 `--ozone-platform-hint=auto`가 있어 Chrome이 Wayland를 잡았고
# (그래서 xvfb 안인데도 사용자 데스크톱에 창이 떴다), `--enable-features=Vulkan,
# DefaultANGLEVulkan,VulkanFromANGLE`도 함께 들어와 **headed 측정의 래스터 경로**를
# 바꿨다. §8이 요구하는 것은 stock **빌드**(개정 #4)이지 사용자의 개인 설정이 아니므로,
# 래퍼가 있으면 실제 바이너리로 내려간다. `NO_UNWRAP=1`로 끌 수 있다.
if [ "${NO_UNWRAP:-0}" != "1" ] && head -c 2 "$BIN" 2>/dev/null | grep -q '#!'; then
  for CAND in /opt/google/chrome/chrome /usr/lib/chromium/chromium /usr/lib/firefox/firefox; do
    if [ -x "$CAND" ] && [ "$(basename "$BIN")" != "$(basename "$CAND")" ] &&
       case "$BIN" in *"$(basename "$CAND" | cut -c1-6)"*) true;; *) false;; esac; then
      echo "[collect-stock] 래퍼 우회: $BIN → $CAND (개인 플래그 주입 회피)"
      BIN="$CAND"
      break
    fi
  done
fi
NAME="${2:?출력 이름 (확장자 없이)}"
TIMEOUT="${3:-10800}"
PORT="${PORT:-8931}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/bench/collected/$NAME.json"
PROFILE="$(mktemp -d)"

PARTS="$ROOT/bench/collected/$NAME.part.json"
rm -f "$OUT" "$ROOT/bench/collected/$NAME.progress.json" "$PARTS"
# §8 "셀마다 fresh page load": 그룹 모드로 시작하면 페이지가 (조합 × 워크로드)
# 하나씩 돌고 스스로 다음 그룹으로 이동한다. 전 그룹(6 조합 × 7 워크로드 = 42)이
# 오면 합친다.
# `GROUPS`는 bash 내장 특수 변수(사용자 그룹 ID 배열)라 대입이 먹지 않는다.
# 부분 재수집(2026-08-08): 조합 하나만 다시 재야 할 때가 있다 — §11-2 개정 #8이
# R3 조합(variants=true)만 무효화했고, 다른 조합의 방출 write 열이 불변임은
# `crates/loom-runtime/tests/write_identity.rs`의 골든 비교로 확인됐다.
# `FIRST_GROUP`/`TOTAL_GROUPS`를 주면 그 구간만 돈다.
#   조합 인덱스: 0~2 POD n∈{100,1000,10000} / 3 String / 4 R2 / 5 R3
#   그룹 = 조합*7 + 워크로드  (워크로드 7종)
# 예) R3만: FIRST_GROUP=35 TOTAL_GROUPS=42 bench/collect-stock.sh ...
FIRST_GROUP="${FIRST_GROUP:-0}"
# 개정 #9로 셀 목록이 §3.3 문언 그대로가 됐다: 기본 57(핵심격자 12 + W10 4 +
# 스케일링 6 + R3 1 + W6 30 + W8 1 + W9 1 + W11 1 + heap 1) + write 변형 이중 12
# = 69. 셀 수가 바뀌면 여기도 바꿔야 한다 — 하네스가 범위 밖 인덱스에서 빈
# 결과를 내므로 그것으로 대조할 것.
TOTAL_GROUPS="${TOTAL_GROUPS:-69}"

# -----------------------------------------------------------------------------
# §8 "격리": "성능 측정은 **headed 자동화**(CDP/WebDriver), 정합성 게이트는
# headless 허용." — 개정 #9-7로 이행. 표시 장치가 없는 머신에서는 `xvfb-run`이
# 진짜 디스플레이를 준다(headless 모드와 달리 합성·래스터 경로가 실제로 돈다).
#
# §8 "하드웨어·스로틀": "저사양 프록시: Chromium CDP **4× CPU throttle**을
# (i) 트랙 L 핵심 격자 ... 에 추가." — `THROTTLE=4`로 켠다. stock 브라우저를
# `--remote-debugging-port`로 띄우고 `bench/e2e/apply-throttle.mjs`가 붙어서
# 셀마다(= 페이지 이동마다) 다시 건다. 브라우저 빌드는 stock 그대로다(개정 #4).
#
# Firefox/WebKit은 §8이 "스로틀 수단 부재로 비스로틀만"으로 이미 한계를 명기했다.
HEADED="${HEADED:-1}"
THROTTLE="${THROTTLE:-1}"
CDP_PORT="${CDP_PORT:-9222}"
# `CELLS=core`면 §8의 스로틀 조항이 지목한 핵심 격자만 돈다(스로틀 통과용).
CELLS="${CELLS:-all}"
CELLS_Q=""
[ "$CELLS" = "core" ] && CELLS_Q="&cells=core"
URL="http://localhost:$PORT/demo/list_bench.html?collect=$NAME&group=$FIRST_GROUP$CELLS_Q"

if [ "$THROTTLE" != "1" ]; then
  case "$BIN" in
    *firefox*)
      echo "THROTTLE은 Chromium CDP 전용이다 — §8이 Firefox/WebKit은 비스로틀만으로 명기" >&2
      exit 1
      ;;
  esac
fi

# **브라우저 출력을 버리지 않는다 (2026-08-12, 실제로 당했다)**: 2026-08-12까지
# `>/dev/null 2>&1`이었다. 그래서 wasm이 패닉하거나 트랩하면 그 메시지가 어디에도
# 안 남고, 페이지는 **조용히 0 CPU로 멈춘 것처럼** 보인다 — "느린 셀이 도는 중"과
# "죽었는데 아무도 모름"이 구별되지 않는다. 실제로 v21 1차 수집이 그 상태로
# 11분을 서 있었고, 원인을 찾으려 해도 증거가 이미 버려진 뒤였다.
BROWSER_LOG="$ROOT/bench/collected/$NAME.browser.log"
rm -f "$BROWSER_LOG"

# headed면 xvfb로 감싼다. `-a`는 빈 디스플레이 번호를 알아서 고른다.
#
# **`env -u WAYLAND_DISPLAY`가 필수다 (2026-08-12, 실제로 당했다)**: `xvfb-run`은
# `DISPLAY`만 가상 디스플레이로 바꾸고 `WAYLAND_DISPLAY`는 건드리지 않는다.
# Wayland 세션에서는 Chrome이 그쪽을 먼저 잡으므로, xvfb로 감쌌는데도 **사용자의
# 실제 데스크톱에 창이 뜬다**. 그러면 (1) 사용자 작업을 방해하고, (2) 그 창이
# 닫히면 캠페인이 통째로 죽고, (3) 무엇보다 §8 "격리"(새 프로필, 확장 없음)가
# 깨진다 — 사용자가 쓰고 있는 컴포지터·다른 창·포커스 변화가 측정에 섞인다.
# `--ozone-platform=x11`은 그 위에 한 겹 더 못을 박는 것이다.
LAUNCH=()
if [ "$HEADED" = "1" ]; then
  LAUNCH=(xvfb-run -a --server-args="-screen 0 1920x1080x24" env -u WAYLAND_DISPLAY)
fi

case "$BIN" in
  *firefox*)
    # `--no-remote` + 빈 프로필: 이미 떠 있는 사용자 세션에 붙지 않게.
    FF_FLAGS=(--no-remote --profile "$PROFILE")
    # Firefox도 같은 이유로 X11을 강제한다.
    [ "$HEADED" = "1" ] && export MOZ_ENABLE_WAYLAND=0
    [ "$HEADED" = "1" ] || FF_FLAGS+=(--headless)
    "${LAUNCH[@]}" "$BIN" "${FF_FLAGS[@]}" "$URL" > "$BROWSER_LOG" 2>&1 &
    ;;
  *)
    CR_FLAGS=(--no-first-run --no-default-browser-check
              --user-data-dir="$PROFILE" --enable-features=SharedArrayBuffer)
    [ "$HEADED" = "1" ] && CR_FLAGS+=(--ozone-platform=x11)
    [ "$HEADED" = "1" ] || CR_FLAGS+=(--headless=new)
    [ "$THROTTLE" = "1" ] || CR_FLAGS+=(--remote-debugging-port="$CDP_PORT")
    "${LAUNCH[@]}" "$BIN" "${CR_FLAGS[@]}" "$URL" > "$BROWSER_LOG" 2>&1 &
    ;;
esac
PID=$!

# §8 "버전은 실행 시점 기록" + §2.5 "공유 기반 동결" — 무엇으로 어떻게 쟀는지를
# 데이터 옆에 남긴다. 이게 없으면 다음 사람이 "stock 브라우저"라는 말만 보고
# 개인 플래그가 섞였는지 알 수 없다(실제로 그 일이 있었다).
cat > "$ROOT/bench/collected/$NAME.env.json" <<ENVJSON
{
  "collected_at_unix": $(date +%s),
  "browser_binary": "$BIN",
  "browser_version": "$("$BIN" --version 2>/dev/null | head -1)",
  "headed": "$HEADED",
  "throttle_rate": "$THROTTLE",
  "cells": "$CELLS",
  "first_group": $FIRST_GROUP,
  "total_groups": $TOTAL_GROUPS,
  "xvfb": "$([ "$HEADED" = "1" ] && echo "xvfb-run -a -screen 0 1920x1080x24, WAYLAND_DISPLAY unset" || echo "none")",
  "git_commit": "$(cd "$ROOT" && git rev-parse HEAD)",
  "git_dirty": $(cd "$ROOT" && [ -z "$(git status --porcelain)" ] && echo false || echo true),
  "url": "$URL"
}
ENVJSON
echo "[collect-stock] 환경 기록: bench/collected/$NAME.env.json"

THROTTLE_PID=""
if [ "$THROTTLE" != "1" ]; then
  (cd "$ROOT/bench/e2e" && node ../../bench/e2e/apply-throttle.mjs "$CDP_PORT" "$THROTTLE") &
  THROTTLE_PID=$!
  echo "4× CPU 스로틀 감시 시작 (pid $THROTTLE_PID, 포트 $CDP_PORT)"
fi
# 정리는 **절대 스크립트를 죽이면 안 된다**. 2026-08-03에 Chrome이 프로필
# 디렉터리에 아직 쓰는 중이라 `rm -rf`가 실패했고, `set -e` + trap 조합에서
# 그 실패가 스크립트 종료 코드가 되어 뒤이은 Firefox 수집이 통째로 건너뛰어졌다.
cleanup() {
  [ -n "$THROTTLE_PID" ] && kill "$THROTTLE_PID" 2>/dev/null || true
  local rc=$?
  kill "$PID" 2>/dev/null || true
  sleep 1
  rm -rf "$PROFILE" 2>/dev/null || true
  return $rc
}
trap cleanup EXIT

echo "[collect-stock] $NAME 시작 (pid=$PID, 상한 ${TIMEOUT}초)"
START=$(date +%s)
count_parts() { grep -c '"group":' "$PARTS" 2>/dev/null || echo 0; }
# 부분 재수집이면 기대 조각 수는 구간 길이다 — 전체 수와 비교하면 영원히 안 끝난다.
EXPECTED=$((TOTAL_GROUPS - FIRST_GROUP))
while [ "$(count_parts)" -lt "$EXPECTED" ]; do
  if ! kill -0 $PID 2>/dev/null; then
    echo "[collect-stock] **브라우저가 죽었다** — 결과 없음" >&2
    exit 1
  fi
  NOW=$(date +%s); EL=$((NOW-START))
  if [ "$EL" -gt "$TIMEOUT" ]; then
    echo "[collect-stock] 타임아웃 ${TIMEOUT}초 초과" >&2
    exit 1
  fi
  # **`EL % 60`이 아니라 "분이 넘어갔는가"로 판정한다 (2026-08-12 수리)**.
  # `sleep 5`는 정확히 5초가 아니고 루프 본문(grep 두 번, tail 한 번)도 시간을
  # 먹으므로 `EL`이 60의 배수를 **건너뛰기** 시작한다. 실측: 1~11분은 매분
  # 찍히다가 그 뒤 25분간 한 줄도 안 찍혔고, 그동안 수집은 멀쩡히 돌고 있었다.
  # 침묵이 정상 동작과 구별되지 않으면 heartbeat이 아니다.
  MIN=$((EL / 60))
  if [ "$MIN" -gt "${LAST_MIN:-0}" ]; then
    LAST_MIN="$MIN"
    DONE="$(count_parts)"
    PROG="$(tail -1 "$ROOT/bench/collected/$NAME.progress.json" 2>/dev/null || echo '')"
    echo "[collect-stock] ${MIN}분 경과 — 그룹 $DONE/$EXPECTED  $PROG"
  fi
  sleep 5
done
# 조각을 하나로 합친다.
python3 - "$PARTS" "$OUT" <<'PYEOF'
import json, sys
# 조각은 여러 줄에 걸친 JSON이 이어 붙은 스트림이다 — 줄 단위로 못 읽는다.
dec = json.JSONDecoder()
raw = open(sys.argv[1]).read()
parts, i = [], 0
while i < len(raw):
    while i < len(raw) and raw[i].isspace():
        i += 1
    if i >= len(raw):
        break
    obj, i = dec.raw_decode(raw, i)
    parts.append(obj)
parts.sort(key=lambda p: p["group"])
track_l, track_t = [], []
for p in parts:
    track_l.extend(p["track_l"])
    track_t.extend(p["track_t"])
json.dump({"track_l": track_l, "track_t": track_t}, open(sys.argv[2], "w"), indent=1)
print(f"  합침: 그룹 {len(parts)}개 → 트랙 L {len(track_l)} 레코드, 트랙 T {len(track_t)} 레코드")
PYEOF
echo "[collect-stock] $NAME 완료 ($((($(date +%s)-START)/60))분) → $OUT"
exit 0
