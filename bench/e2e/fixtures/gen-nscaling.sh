#!/usr/bin/env bash
# RQ6 §4.4 N-스케일링 픽스처 생성 — 생성기 선택 설정(2026-10-17 병익님 결정: Node·Rust 두 생성기 공존).
#
# 설정: LOOM_NSCALING_GEN=node|rust (기본 node — Task #27이 실제로 쓴 생성기). 인자로 줘도 된다.
#   bash bench/e2e/fixtures/gen-nscaling.sh          # 설정값(기본 node)
#   LOOM_NSCALING_GEN=rust bash bench/e2e/fixtures/gen-nscaling.sh
#   bash bench/e2e/fixtures/gen-nscaling.sh rust
# 두 생성기는 바이트 단위로 같은 n{1,10,50}.html을 낸다. 그 사실과 마커가 컴파일러 산출물과 맞는다는 사실은
# `cargo test -p loom-rq6-fixture-gen`(마커 어긋남 가드)이 지킨다 — 생성 뒤 GUARD=1이면 그 가드를 돌린다.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../../.." && pwd)"
GEN="${1:-${LOOM_NSCALING_GEN:-node}}"
case "$GEN" in
  node) node "$ROOT/bench/e2e/fixtures/gen-nscaling-page.mjs" ;;
  rust) (cd "$ROOT" && cargo run -q -p loom-rq6-fixture-gen --bin gen_nscaling) ;;
  *) echo "LOOM_NSCALING_GEN은 node 또는 rust여야 한다: '$GEN'" >&2; exit 2 ;;
esac
echo "[gen-nscaling] 생성기=$GEN"
if [ "${GUARD:-0}" = "1" ]; then
  (cd "$ROOT" && cargo test -q -p loom-rq6-fixture-gen)
fi
