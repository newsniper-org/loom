#!/usr/bin/env bash
# Runs this harness's WebKit project inside an Ubuntu 24.04 ("noble")
# container matching Playwright's own WebKit prebuild target — works around
# a real ABI mismatch on Arch/CachyOS hosts (rolling-release libxml2/etc.
# SONAMEs don't match what Playwright's Ubuntu-targeted WebKit binary
# expects; no amount of host package installation fixes this, confirmed by
# actually trying). Requires the host's demo server already running on
# localhost:8931 (see repo root README/roadmap for how) — `--network=host`
# lets the container reach it directly.
#
# Usage: ./run-webkit-in-docker.sh [extra playwright args, e.g. a spec path]
set -euo pipefail
cd "$(dirname "$0")"

IMAGE="mcr.microsoft.com/playwright:v1.61.0-noble"
PW_VERSION="1.61.0"

# 컨테이너 안 npm 캐시는 빌드 캐시 드라이브에 둔다(2026-11-01, tools/build-cache.sh) — 매 실행 재다운로드를 피한다.
NPM_CACHE="$(bash ../../tools/build-cache.sh)/docker-webkit-npm"
mkdir -p "$NPM_CACHE"

docker run --rm --network=host \
  -v "$PWD:/work" -w /work \
  -v "$NPM_CACHE:/root/.npm" \
  "$IMAGE" \
  bash -c "npm install @playwright/test@${PW_VERSION} --no-save >/dev/null 2>&1 && npx playwright test --project=webkit $*"
