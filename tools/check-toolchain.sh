#!/bin/bash
# 고정 버전(tools/toolchain-pins.env)과 실제 설치를 대조한다. 빌드·검증·측정 산출물을 만들기 전에 돌린다.
cd "$(dirname "$0")/.." || exit 2
. tools/toolchain-pins.env
bad=0
chk() { if [ "$2" != "$3" ]; then echo "불일치 $1: 고정=$2 실제=$3" >&2; bad=1; else echo "ok $1: $3"; fi; }
chk rustc "$RUSTC" "$(rustc --version)"
chk verus "$VERUS" "$(verus --version 2>&1 | sed -n 's/.*Version: //p')"
chk wasm-pack "$WASM_PACK" "$(wasm-pack --version)"
WB=$(for d in "$HOME"/.cache/.wasm-pack/wasm-bindgen-*/; do "$d/wasm-bindgen" --version | grep -q "$WASM_BINDGEN" && echo found; done | head -1)
chk wasm-bindgen "found" "${WB:-missing}"
chk Cargo.lock-wasm-bindgen "$WASM_BINDGEN" "$(grep -A1 '^name = "wasm-bindgen"$' Cargo.lock | sed -n 's/version = "\(.*\)"/\1/p')"
# rust-src 유무는 경로 remap 없이 빌드한 wasm 바이트를 바꾼다(toolchain-pins.env 주석). 2026-10-30 정정: 동결 빌드
# (build-frozen-bench.sh)가 remap으로 이를 무력화하므로 실패 조건이 아니라 정보로만 출력한다. 활성 툴체인 기준.
echo "info rust-src: $(rustup component list --installed 2>/dev/null | grep -qx 'rust-src' && echo present || echo absent) (동결 빌드는 경로 remap으로 무관)"
exit $bad
