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
TOTAL_GROUPS="${TOTAL_GROUPS:-58}"
# 개정 #10(2026-09-27): 58 = §3.3 기본 셀 57 + W7×R2×heap 1. write 변형 이중 측정은 이제 별도 셀이
# 아니라 같은 셀의 후보(9후보)라 셀 수에 안 더해진다. 셀↔그룹 대응표:
#   cargo test -p loom-bench -- --ignored --nocapture print_cell_order

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
# §8 GC 통제(M-15)와 스로틀은 CDP 에이전트(`bench/e2e/cdp-agent.mjs`)가 한다(개정 #10).
# **Chromium 런은 전부** 에이전트를 붙인다 — v21까지는 스로틀 런에만 CDP가 있었고 비스로틀
# 런에는 연결 자체가 없어 M-15가 원리적으로 이행될 수 없었다. `cdp=1`은 하네스에게 "에이전트가
# 있어야 한다"를 알린다: 바인딩이 끝내 안 생기면 레코드에 `missing-binding`으로 남는다(조용히
# "에이전트 없는 런"으로 기록되지 않는다). Firefox는 CDP 수단이 없다 — §8이 스로틀에 대해 이미
# 명기한 것과 같은 한계이고, 레코드에 `unavailable`로 남는다.
# **개정 #10 후속 3(2026-10-01)**: Firefox도 에이전트를 붙인다 — CDP가 아니라 WebDriver BiDi(`ff-bidi-agent.mjs`,
# chrome 컨텍스트의 child-gc-request + Gecko 프로파일러 GC 마커). 하네스에게는 같은 `cdp=1`이다(같은 핸드셰이크).
# Firefox의 CPU 스로틀은 여전히 수단이 없다(아래 THROTTLE 검사 그대로).
CDP_Q="&cdp=1"
# `end`: 페이지가 이 그룹 이상으로 스스로 이동하지 않는다(구간 수집이 범위 밖 셀을 재지 않게).
URL="http://localhost:$PORT/demo/list_bench.html?collect=$NAME&group=$FIRST_GROUP$CELLS_Q$CDP_Q&end=$TOTAL_GROUPS"

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
#
# **`setsid`로 새 세션에서 띄운다 (2026-09-27, 실제로 당했다)**. 전에는 정리가 `kill $PID`
# 하나였고 `$PID`는 `xvfb-run` 래퍼였다 — 래퍼가 죽어도 그 아래 Xvfb와 브라우저는 고아로 살아남았다.
# 개정 #10 스모크에서 연속 실행한 네 런 중 셋이 **앞 런의 살아남은 브라우저**가 CDP 포트를 쥔 채
# 떠 있는 상태에서 시작했고, 새 에이전트가 그 옛 브라우저에 붙어 새 페이지는 GC 통제도 스로틀도
# 없이 측정됐다(레코드의 `gc.control = missing-binding`이 그것을 증언했다). 세션 하나로 묶으면
# 정리가 트리 전체(래퍼·Xvfb·브라우저·자식)를 한 번에 죽일 수 있다.
LAUNCH=(setsid)
if [ "$HEADED" = "1" ]; then
  LAUNCH=(setsid xvfb-run -a --server-args="-screen 0 1920x1080x24" env -u WAYLAND_DISPLAY)
fi

# CDP 포트가 이미 점유돼 있으면 **시작하지 않는다** — 에이전트가 그 포트의 다른 브라우저에 붙는다.
case "$BIN" in *) # 개정 #10 후속 3: Firefox도 원격 포트를 쓴다
  if (exec 3<>"/dev/tcp/127.0.0.1/$CDP_PORT") 2>/dev/null; then
    echo "[collect-stock] **CDP 포트 $CDP_PORT가 이미 열려 있다** — 앞 수집의 브라우저가 살아 있을 수 있다. 정리 후 다시 실행할 것." >&2
    exit 1
  fi ;;
esac

case "$BIN" in
  *firefox*)
    # `--no-remote` + 빈 프로필: 이미 떠 있는 사용자 세션에 붙지 않게.
    FF_FLAGS=(--no-remote --profile "$PROFILE")
    # WebDriver BiDi 원격 에이전트 + chrome(특권) 컨텍스트 허용 — 강제 GC·GC 마커용(개정 #10 후속 3).
    FF_FLAGS+=(--remote-debugging-port "$CDP_PORT" --remote-allow-system-access)
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
    # 개정 #10: 스로틀 여부와 무관하게 항상 연다(GC 통제 에이전트가 붙는다).
    CR_FLAGS+=(--remote-debugging-port="$CDP_PORT")
    "${LAUNCH[@]}" "$BIN" "${CR_FLAGS[@]}" "$URL" > "$BROWSER_LOG" 2>&1 &
    ;;
esac
PID=$!
# `setsid`는 (이 스크립트의 백그라운드 작업은 그룹 리더가 아니므로) fork 없이 exec한다 —
# `$PID`가 곧 새 세션·프로세스 그룹의 리더다.
PGID="$PID"

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
  "cdp_agent": $([ -n "$CDP_Q" ] && echo true || echo false),
  "gc_log": "$([ -n "$CDP_Q" ] && echo "bench/collected/$NAME.gc.jsonl" || echo "")",
  "url": "$URL"
}
ENVJSON
echo "[collect-stock] 환경 기록: bench/collected/$NAME.env.json"

# CDP 에이전트 — 스로틀(배수 1이면 안 건다) + 셀 시작 강제 GC + 측정 중 GC 트레이스.
# 이전 `apply-throttle.mjs`(v21)는 `framenavigated`에 반응해 페이지와 **경주하며** 걸었다.
# 에이전트는 페이지가 셀 시작에서 부를 때 걸고, 페이지는 응답까지 멈춰 기다린다.
THROTTLE_PID=""
GCLOG="$ROOT/bench/collected/$NAME.gc.jsonl"
AGENT_LOG="$ROOT/bench/collected/$NAME.agent.log"
rm -f "$GCLOG" "$AGENT_LOG"
if [ -n "$CDP_Q" ]; then
  case "$BIN" in
    *firefox*) AGENT_JS=ff-bidi-agent.mjs ;;
    *) AGENT_JS=cdp-agent.mjs ;;
  esac
  (cd "$ROOT/bench/e2e" && exec node "$AGENT_JS" "$CDP_PORT" "$THROTTLE" "$GCLOG" "$NAME") > "$AGENT_LOG" 2>&1 &
  THROTTLE_PID=$!
  echo "CDP 에이전트 시작 (pid $THROTTLE_PID, 포트 $CDP_PORT, 스로틀 ${THROTTLE}x, GC 로그 $GCLOG)"
fi
# 정리는 **절대 스크립트를 죽이면 안 된다**. 2026-08-03에 Chrome이 프로필
# 디렉터리에 아직 쓰는 중이라 `rm -rf`가 실패했고, `set -e` + trap 조합에서
# 그 실패가 스크립트 종료 코드가 되어 뒤이은 Firefox 수집이 통째로 건너뛰어졌다.
cleanup() {
  [ -n "$THROTTLE_PID" ] && kill "$THROTTLE_PID" 2>/dev/null || true
  [ -n "${MEM_PID:-}" ] && kill "$MEM_PID" 2>/dev/null || true
  local rc=$?
  # 세션 전체(래퍼·Xvfb·브라우저·자식)를 죽인다. TERM으로 기회를 주고, 남으면 KILL.
  kill -TERM -- "-$PGID" 2>/dev/null || true
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    kill -0 -- "-$PGID" 2>/dev/null || break
    sleep 1
  done
  kill -KILL -- "-$PGID" 2>/dev/null || true
  sleep 1
  rm -rf "$PROFILE" 2>/dev/null || true
  return $rc
}
trap cleanup EXIT

echo "[collect-stock] $NAME 시작 (pid=$PID, 상한 ${TIMEOUT}초)"
START=$(date +%s)
# **조각 헤더로 센다** (개정 #10 후속, 수집 전 적대적 검증이 잡음). 전에는 `"group":`을 셌는데
# 개정 #10부터 레코드마다 `"group"` 필드가 붙어 조각 하나가 1 + 레코드 수로 세어졌다 — 58셀 전수
# 수집이 12셀에서, 스로틀 12셀이 2셀에서 "완료"로 끝날 뻔했다(단일 그룹 스모크로는 안 드러났다).
# `"total_cells"`는 조각마다 정확히 한 번(하네스의 조각 머리) 나온다.
count_parts() { grep -c '"total_cells":' "$PARTS" 2>/dev/null || echo 0; }
# **기대 조각 수는 하네스가 말해 준다** (2026-09-24). 첫 조각의 `total_cells`를
# 읽는다 — 셀 목록(`cell_list()`, `?cells=core` 필터)을 아는 것은 하네스뿐이고,
# 여기 하드코딩(69)은 `CELLS=core`(20셀)에서 정상 종료를 타임아웃 실패로 만들었다.
# 조각이 아직 없으면 `TOTAL_GROUPS`로 시작하고 첫 조각이 오면 갱신한다.
expected_parts() {
  local t
  t=$(grep -o '"total_cells": [0-9]*' "$PARTS" 2>/dev/null | head -1 | grep -o '[0-9]*$')
  # `TOTAL_GROUPS`는 **상한**으로 계속 존중한다 — 부분 재수집(FIRST_GROUP/TOTAL_GROUPS로
  # 구간 지정)에서 하네스의 전체 셀 수를 그대로 쓰면 구간을 넘겨 끝까지 돈다.
  if [ -n "$t" ] && [ "$t" -lt "$TOTAL_GROUPS" ]; then
    echo $((t - FIRST_GROUP))
  else
    echo $((TOTAL_GROUPS - FIRST_GROUP))
  fi
}
# §8 "환경 명세 ... 실행 시점 기록". 2026-09-26에 머신이 스왑 44/46 GB인 상태에서
# 측정이 진행됐고, 같은 W6 K 셀이 머신 상태에 따라 **6.6배** 차이로 측정됐다
# (5,733삽입 30.1분 vs 6,000삽입 272초). 그런 압박이 있었는지를 데이터 옆에
# 남기지 않으면 나중에 "이 수치가 계산 비용인가 스왑인가"를 물을 수 없다.
# 하네스(wasm) 밖이므로 §2.5 동결 대상이 아니다 — 측정값을 바꾸지 않는다.
MEMF="$ROOT/bench/collected/$NAME.mem.jsonl"
rm -f "$MEMF"
(
  while :; do
    awk -v t="$(date +%s)" '
      /^MemAvailable:/ {a=$2}
      /^SwapTotal:/    {st=$2}
      /^SwapFree:/     {sf=$2}
      END {printf "{\"t\": %d, \"mem_avail_mb\": %d, \"swap_used_mb\": %d}\n", t, a/1024, (st-sf)/1024}
    ' /proc/meminfo >> "$MEMF"
    sleep 30
  done
) &
MEM_PID=$!

EXPECTED=$(expected_parts)
AGENT_DEADLINE=$(( $(date +%s) + ${AGENT_ATTACH_SEC:-90} ))
while [ "$(count_parts)" -lt "$(expected_parts)" ]; do
  # **에이전트가 살아 있지만 아무 페이지에도 안 붙은 상태**를 빨리 잡는다 (개정 #10 후속). 그러면
  # 셀마다 바인딩을 60초 기다린 뒤 `missing-binding`으로 GC 통제·스로틀 없이 측정되는데, 에이전트는
  # 살아 있으므로 아래 사망 검사로는 안 잡힌다.
  if [ -n "$THROTTLE_PID" ] && [ "$(date +%s)" -gt "$AGENT_DEADLINE" ] &&
     ! grep -q "페이지 타깃에 붙음" "$AGENT_LOG" 2>/dev/null; then
    echo "[collect-stock] **CDP 에이전트가 ${AGENT_ATTACH_SEC:-90}초 안에 이 수집의 페이지에 붙지 않았다** — $(tail -2 "$AGENT_LOG" 2>/dev/null | tr '\n' ' ')" >&2
    exit 1
  fi
  # 조각에 **체계적 GC 통제 실패**(바인딩 없음·셀 시작 실패)가 보이면 멈춘다 — 그 뒤 셀도 같다.
  # 측정 끝(measure-end)의 단발 실패는 그 셀의 주석만 null로 두고 계속 간다(판정기가 셀 단위로 보고).
  if [ -n "$CDP_Q" ] && grep -qE '"control": "(missing-binding|cell-start-)' "$PARTS" 2>/dev/null; then
    echo "[collect-stock] **GC 통제가 셀 시작에서 실패한 조각이 왔다** — $(grep -oE '"control": "[a-z-]+"' "$PARTS" | sort | uniq -c | tr '\n' ' ')" >&2
    exit 1
  fi
  EXPECTED=$(expected_parts)
  if ! kill -0 $PID 2>/dev/null; then
    echo "[collect-stock] **브라우저가 죽었다** — 결과 없음" >&2
    exit 1
  fi
  # **에이전트가 죽으면 즉시 멈춘다** (개정 #10). 죽은 뒤의 셀은 GC 통제·스로틀 없이 측정되고,
  # 페이지는 응답을 기다리다 상한(120초)에 걸려 `*-timeout`을 적은 채 계속 간다 — 판정기가
  # 잡기는 하지만 그 셀들은 쓸 수 없으므로 기계 시간을 버리지 않는다. v21 스로틀 런에서 감시
  # 프로세스가 죽은 것을 늦게 알아챈 사고가 있었다.
  if [ -n "$THROTTLE_PID" ] && ! kill -0 "$THROTTLE_PID" 2>/dev/null; then
    echo "[collect-stock] **CDP 에이전트가 죽었다** — $(tail -3 "$AGENT_LOG" 2>/dev/null | tr '\n' ' ')" >&2
    exit 1
  fi
  # **렌더러만 죽는 경우를 잡는다 (2026-09-26, 실제로 당했다)**: 위 `kill -0`은
  # **브라우저 프로세스**만 본다. 커널 OOM 킬러는 렌더러를 고른다(Chrome 렌더러는
  # `oom_score_adj=300`으로 스스로 최우선 희생을 자원한다) — 그러면 페이지는 죽고
  # 브라우저 프로세스는 멀쩡하므로 이 루프가 **타임아웃까지 그대로 기다린다.**
  # v21 3차 수집이 그 상태로 4시간을 서 있었고, 10시간 타임아웃까지 갈 예정이었다.
  # 진행 표식(`.progress.json`)은 하네스가 셀·W6 500삽입마다 POST하므로, 그것이
  # 굳으면 페이지가 일을 안 하고 있는 것이다. 가장 느린 정상 구간(§8 셀 상한 30분)
  # 보다 넉넉히 잡되 타임아웃보다는 훨씬 짧게.
  PROGF="$ROOT/bench/collected/$NAME.progress.json"
  if [ -f "$PROGF" ]; then
    PAGE_IDLE=$(( $(date +%s) - $(stat -c %Y "$PROGF") ))
    if [ "$PAGE_IDLE" -gt "${PAGE_STALL_SEC:-2400}" ]; then
      echo "[collect-stock] **페이지가 ${PAGE_IDLE}초간 진행 표식을 안 냈다** — 렌더러가 죽었을 가능성이 높다(커널 OOM 등). 브라우저 프로세스는 살아 있어 위 검사로는 안 잡힌다." >&2
      echo "[collect-stock] 마지막 표식: $(tail -1 "$PROGF" 2>/dev/null)" >&2
      exit 1
    fi
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
