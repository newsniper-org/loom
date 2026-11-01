#!/bin/bash
# probe.py를 WebKit으로 돌린다(2026-11-01). 호스트(Arch/CachyOS)에서는 Playwright WebKit이 의존 라이브러리 SONAME 불일치로 뜨지 않으므로
# Playwright 공식 Ubuntu 이미지 안에서 같은 probe.py를 실행한다. 컨테이너는 ~/run-podman-on-t9.sh로 띄운다(CLAUDE.md).
# 이미지에는 브라우저만 있고 Python 패키지는 없어서 호스트와 같은 버전(1.61.0)을 설치한다. pip 캐시는 빌드 캐시 드라이브에.
# 사용: tools/surface-probe/run-webkit.sh > 결과.json   (pkg/는 미리 빌드돼 있어야 한다)
set -euo pipefail
cd "$(dirname "$0")"
PW=1.61.0
PIPC="$(bash ../build-cache.sh)/docker-webkit-pip"; mkdir -p "$PIPC"
"$HOME/run-podman-on-t9.sh" run --rm --network=host -e BROWSER=webkit \
  -v "$PWD:/work:ro" -v "$PIPC:/root/.cache/pip" -w /work \
  "mcr.microsoft.com/playwright/python:v$PW-noble" \
  bash -c "pip install --break-system-packages -q playwright==$PW >&2 && python3 probe.py"
