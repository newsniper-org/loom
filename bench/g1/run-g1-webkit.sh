#!/bin/bash
# G1 드라이버(bench/g1/run-g1.py)를 WebKit으로 돌린다(RQ1 개정 #12 후속 8 — WebKit은 E의 실폴백 경로, 제약 보고용).
# 호스트(Arch/CachyOS)에서는 Playwright WebKit이 뜨지 않으므로 Playwright 공식 Ubuntu 이미지 안에서 돌린다(tools/surface-probe/run-webkit.sh와 같은 방식).
# 컨테이너는 ~/run-podman-on-t9.sh로 띄운다(CLAUDE.md). 저장소는 같은 경로로 마운트해 원장 경로·git 정보가 호스트와 같게 한다.
# 사용: bench/g1/run-g1-webkit.sh <run-g1.py 인자...>   (--browser webkit은 이 스크립트가 붙인다)
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
PW=1.61.0
PIPC="$(bash "$ROOT/tools/build-cache.sh")/docker-webkit-pip"; mkdir -p "$PIPC"
exec "$HOME/run-podman-on-t9.sh" run --rm --network=host --userns=keep-id \
  -v "$ROOT:$ROOT" -v "$PIPC:/tmp/pipcache" -w "$ROOT" -e PIP_CACHE_DIR=/tmp/pipcache -e HOME=/tmp \
  "mcr.microsoft.com/playwright/python:v$PW-noble" \
  bash -c "pip install --user --break-system-packages -q playwright==$PW >&2 && python3 bench/g1/run-g1.py --browser webkit $*"
