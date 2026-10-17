#!/usr/bin/env bash
# Phase 7a 컴파일러 바이트 동일 게이트 (RQ1 프로토콜 개정 #11, design-7a §2).
#
# 커밋마다 (BASE = 부모, 후보 = 스테이징된 인덱스):
#   git add -A
#   tools/compiler-gate/check-compiler-outputs.sh run --work "$G" --base HEAD --cand-from-index \
#       --tiers A,B,D [--expect-compiler-change] --record research-questions/data/phase7a-byte-gate-<date>-C<n>.json
#   git add <record>; git commit ...
#   tools/compiler-gate/check-compiler-outputs.sh verify-commit --work "$G" --record <record>
# 7a 끝: run --base da4a9bc --cand HEAD --tiers A,B,C,D --base2 --record ...-final.json
# 자체 시험: check-compiler-outputs.sh self-test --side "$G/sides/base" [--record <record>]
#
# $G는 스크래치 아래 고정 절대경로여야 한다(base와 cand가 같은 $G/src·$G/target에서 빌드돼야
# wasm에 박히는 OUT_DIR 경로가 같다 — design-7a 위험 1). 종료 코드: 0 통과, 1 비교 실패,
# 2 하네스 오류(개수 단언 등), 3 재현성 전제 불충족.
set -euo pipefail
exec python3 "$(dirname "$(readlink -f "$0")")/gate.py" "$@"
