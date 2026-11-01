#!/bin/bash
# v23 동결·귀속 대조 산출물(`pkg-bench`)을 **빈 target에서** 빌드한다(프로토콜 개정 #12 후속 6).
# 사용: tools/build-frozen-bench.sh <소스 루트> <출력 디렉터리(절대 경로)>
#
# 왜 빈 target인가: 기존 `target/`에는 툴체인 고정(2026-10-26) 이전에 `stable` 툴체인(rust-src 설치)으로 컴파일된
# 의존성 산출물이 남아 있었고, cargo는 rustc 버전이 같으면 그것을 재사용했다 — 그 결과 고정 아래서 만든 wasm에
# `~/.rustup/toolchains/stable-…` 경로가 섞였다. 빈 target에서 만든 산출물은 target 경로와 무관하게 재현된다(같은 소스에서
# 서로 다른 빈 디렉터리 두 곳 → 같은 sha256, 2026-10-30).
set -euo pipefail
SRC=$(cd "$1" && pwd); OUT=$2
case "$OUT" in /*) ;; *) echo "출력 디렉터리는 절대 경로여야 한다: $OUT" >&2; exit 2;; esac
HERE=$(cd "$(dirname "$0")" && pwd)
# 고정 검사는 이 스크립트가 있는 트리의 것을 쓴다(P1 소스 트리에는 검사 스크립트가 없다). 툴체인은 SRC의
# rust-toolchain.toml을 따르므로 SRC 안에서 돈다.
(cd "$SRC" && bash "$HERE/check-toolchain.sh")
grep -qx 'channel = "1.99.1"' "$SRC/rust-toolchain.toml" || { echo "$SRC에 툴체인 고정 파일이 없다" >&2; exit 2; }
# 경로 remap(2026-10-30, 개정 #12 후속 7): 활성 툴체인에 rust-src가 있으면 rustc가 std의 `/rustc/<해시>` 경로를 로컬
# sysroot 경로로 되돌려 wasm에 박는다. 그 경로를 다시 `/rustc/<해시>`로 remap하면 rust-src 유무와 무관하게 같은 바이트가 나온다
# (rust-src 있음 + remap = 동결본 b509135f…·56495615…, 빈 target 두 곳). RUSTFLAGS는 환경값을 덮어쓴다 — 바깥 설정이 끼지 않게.
SYSROOT=$(cd "$SRC" && rustc --print sysroot)
HASH=$(cd "$SRC" && rustc -vV | sed -n 's/^commit-hash: //p')
[ ${#HASH} -eq 40 ] || { echo "rustc 커밋 해시를 못 읽었다: $HASH" >&2; exit 2; }
# 빈 target은 빌드 캐시 드라이브 아래에 만든다(2026-11-01, tools/build-cache.sh). target 경로는 바이트에 안 들어간다(후속 6).
TGT=$(mktemp -d -p "$(bash "$HERE/build-cache.sh")" frozen-target.XXXXXX)
trap 'rm -rf "$TGT"' EXIT
(cd "$SRC" && CARGO_TARGET_DIR="$TGT" RUSTFLAGS="--remap-path-prefix=$SYSROOT/lib/rustlib/src/rust=/rustc/$HASH" \
  wasm-pack build crates/loom-bench --target web --release --out-dir "$OUT")
W="$OUT/loom_bench_bg.wasm"
# 존재 단언 먼저: std 경로가 실제로 실려 있어야 아래 부재 단언이 공허하지 않다.
n_rustc=$(strings -n 8 "$W" | grep -c '^/rustc/[0-9a-f]\{40\}/library/' || true)
n_local=$(strings -n 8 "$W" | grep -c '/toolchains/' || true)
[ "$n_rustc" -gt 0 ] || { echo "std 경로(/rustc/<해시>/library/)가 하나도 없다 — 검사가 공허하다" >&2; exit 1; }
[ "$n_local" -eq 0 ] || { echo "로컬 툴체인 경로가 $n_local개 섞였다 — 오염된 산출물" >&2; exit 1; }
echo "std 경로 /rustc/<해시> ${n_rustc}개, 로컬 툴체인 경로 0개"
sha256sum "$W" "$OUT/loom_bench.js"
