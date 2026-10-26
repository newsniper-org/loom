#!/bin/bash
# 이름 구역을 보존한 측정용 wasm 재생산: wasm-pack --release와 같은 파이프라인(cargo → wasm-bindgen → wasm-opt -O)에서
# 이름 구역만 남긴다. 사용: build-named.sh <레포 루트> <출력 디렉터리> <측정 wasm(wasm-pack --release 산출물)>
# 검증(공허한 확인 방지): 이름 구역을 뺀 모든 구역(types/imports/functions/.../code/data)의 바이트가 측정 wasm과 같아야 한다 —
# 같지 않으면 이 산출물은 "측정된 프로그램의 이름 달린 사본"이 아니므로 함수 diff를 쓰지 않는다.
set -euo pipefail
ROOT=$1; OUT=$2; MEASURED=$3
WB=$(for d in "$HOME"/.cache/.wasm-pack/wasm-bindgen-*/; do "$d/wasm-bindgen" --version | grep -q "0.2.126" && echo "$d/wasm-bindgen"; done | head -1)
WO=$(ls "$HOME"/.cache/.wasm-pack/wasm-opt-*/bin/wasm-opt | head -1)
mkdir -p "$OUT"
RAW="$ROOT/target/wasm32-unknown-unknown/release/loom_bench.wasm"
[ -f "$RAW" ] || { echo "cargo 산출물이 없다: $RAW" >&2; exit 2; }
"$WB" "$RAW" --out-dir "$OUT" --typescript --target web
"$WO" "$OUT/loom_bench_bg.wasm" -o "$OUT/loom_bench_bg.named.wasm" -O -g
python3 - "$OUT/loom_bench_bg.named.wasm" "$MEASURED" <<'PY'
import sys, subprocess, re
def secs(p):
    out = subprocess.run(["wasm-tools","objdump",p],capture_output=True,text=True,check=True).stdout
    data = open(p,'rb').read()
    r = {}
    for ln in out.splitlines():
        m = re.match(r'\s*(\S+(?: \S+)?)\s+\|\s+0x([0-9a-f]+) -\s+0x([0-9a-f]+)', ln)
        if m and not m.group(1).startswith('custom'):
            a,b = int(m.group(2),16), int(m.group(3),16)
            r[m.group(1)] = data[a:b]
    return r, out
a, oa = secs(sys.argv[1]); b, ob = secs(sys.argv[2])
assert set(a) == set(b) and len(a) >= 8, f"구역 집합이 다르거나 너무 적다: {sorted(a)} vs {sorted(b)}"
bad = [k for k in a if a[k] != b[k]]
print("구역", len(a), "개 비교; 다른 구역:", bad or "없음")
sys.exit(1 if bad else 0)
PY
