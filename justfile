# loom-research 작업 명령 모음 (2026-10-30). `just`로 목록을 본다.
#
# 원칙: 여기 있는 것은 개발·검증 명령뿐이다. 트랙 L/T 데이터 수집(`bench/collect-*.sh`)은 병익님 지시로만 시작하므로
# 일부러 넣지 않았다. 측정·동결 산출물은 `frozen-bench`로만 만든다(CLAUDE.md, RQ1 프로토콜 개정 #12 후속 6·7).

set shell := ["bash", "-euo", "pipefail", "-c"]

root := justfile_directory()

# 빌드 캐시(2026-11-01): 이 프로젝트의 모든 빌드 캐시는 /run/media/ybi/T9/.build-cache/<저장소명>/ 에 둔다
# (worktree는 <저장소명>-wt-<이름>/). `target`·`tools/surface-probe/target`은 그곳을 가리키는 링크이고(`just cache`가 맞춘다),
# 파이썬 바이트코드 캐시도 그 아래로 보낸다. T9가 마운트돼 있지 않으면 아래 경로 계산이 실패한다.
cache := `bash tools/build-cache.sh`
export PYTHONPYCACHEPREFIX := cache / "pycache"

# 레시피 목록
default:
    @just --list --unsorted

# 빌드 캐시 링크(target 등 → T9) 확인·생성. 실제 디렉터리가 있으면 캐시로 옮긴 뒤 링크로 바꾼다
cache:
    bash tools/build-cache.sh --link

# 고정 버전(tools/toolchain-pins.env)과 실제 설치 대조 — 빌드·검증 전에
toolchain:
    bash tools/check-toolchain.sh

# 워크스페이스 전체 테스트
test *args: cache
    cargo test --workspace {{args}}

# 워크스페이스 전체 타입 검사(테스트·예제 포함)
check: cache
    cargo check --workspace --all-targets

# Verus: 배포 코드(loom-flush-core, 90 verified) + 모델(flush_order.rs, 28 verified). rlimit 60은 2026-10-26부터 필수
verus: toolchain
    verus --rlimit 60 --crate-type=lib crates/loom-flush-core/src/lib.rs
    verus --rlimit 60 formal-verification/verus/flush_order.rs

# Lean 4 모델 증명 둘(무출력이면 통과)
lean:
    cd formal-verification/lean4 && lean SupervisionTree.lean && lean ResumabilitySoundness.lean

# LIS·match_by_key 모형 전수 검사 스크립트 5개(증명 아님)
exhaustive:
    for f in formal-verification/verus/*-exhaustive.py; do echo "== $f"; python3 "$f"; done

# RQ1 판정기 합성 픽스처 자기점검(18모드 + 차분 검사). 오래 걸린다 — 2026-10-30 실측 41분(판정기 실행 20회)
selfcheck:
    python3 research-questions/analysis/fixtures/selfcheck.py

# 트랙 N 축소 실행(기본 40케이스). 등재 실행은 --cases 10000 — 그건 `cargo run`을 직접 쓸 것
track-n cases="40": cache
    cargo run --release -p loom-track-n -- --cases {{cases}}

# 위 검증 전부(데이터 수집 없음)
verify: toolchain test verus lean selfcheck

# 데모(demo/index.html)용 wasm을 demo/pkg에 빌드 — 개발용, 측정·동결에 쓰지 말 것
demo: cache
    wasm-pack build crates/loom-demo --target web --out-dir {{root}}/demo/pkg

# 측정·동결 산출물 빌드(빈 target + 경로 remap + 오염 단언). 예: just frozen-bench . /abs/out
frozen-bench src out:
    bash tools/build-frozen-bench.sh {{src}} {{out}}

# 저장소 루트를 COOP/COEP 헤더와 함께 서빙(기본 8931)
serve port="8931":
    python3 bench/serve-isolated.py {{port}}
