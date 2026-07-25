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

docker run --rm --network=host \
  -v "$PWD:/work" -w /work \
  "$IMAGE" \
  bash -c "npm install @playwright/test@${PW_VERSION} --no-save >/dev/null 2>&1 && npx playwright test --project=webkit $*"
