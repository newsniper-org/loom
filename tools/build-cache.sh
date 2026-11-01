#!/bin/bash
# 이 체크아웃의 빌드 캐시 디렉터리를 출력한다(2026-11-01 병익님 지시: 프로젝트의 모든 빌드 캐시는 T9 드라이브에 둔다).
#   주 체크아웃       → /run/media/ybi/T9/.build-cache/loom-research
#   링크된 worktree   → /run/media/ybi/T9/.build-cache/loom-research-wt-<worktree 이름>
# (`/run/media/ybi/T9/.build-cache/README.md`의 다른 저장소 규약과 같다. 새 worktree는 post-checkout 훅이 링크를 만든다.)
# `LOOM_BUILD_CACHE`로 덮어쓸 수 있다.
#
# 사용: tools/build-cache.sh          캐시 디렉터리 경로만 출력(없으면 만든다)
#       tools/build-cache.sh --link   아래 LINKS의 각 경로를 캐시 안 디렉터리로 가리키는 심볼릭 링크로 맞춘다.
#                                     실제 디렉터리가 이미 있으면 내용을 캐시로 옮긴 뒤 링크로 바꾼다.
#
# 왜 CARGO_TARGET_DIR을 쓰지 않고 `target` 링크인가: 맨 `cargo`와 `just`가 같은 경로 문자열로 같은 target을 보게 하려는 것이다.
# 경로 문자열이 다르면 OUT_DIR 등 경로가 달라져 둘이 번갈아 서로의 빌드를 무효화할 수 있다.
# 측정·동결 산출물은 이것과 별개로 빈 target에서 만든다(`tools/build-frozen-bench.sh`, 위치만 이 캐시 아래).
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
DRIVE=/run/media/ybi/T9/.build-cache
LINKS=(target tools/surface-probe/target)

if [ -n "${LOOM_BUILD_CACHE:-}" ]; then
  CACHE=$LOOM_BUILD_CACHE
else
  [ -d "$DRIVE" ] || { echo "빌드 캐시 드라이브가 없다: $DRIVE (T9 미마운트?)" >&2; exit 2; }
  common=$(git -C "$ROOT" rev-parse --path-format=absolute --git-common-dir)
  main=$(dirname "$common")
  if [ "$ROOT" = "$main" ]; then CACHE="$DRIVE/$(basename "$main")"
  else CACHE="$DRIVE/$(basename "$main")-wt-$(basename "$ROOT")"; fi
fi
mkdir -p "$CACHE"

if [ "${1:-}" = --link ]; then
  for rel in "${LINKS[@]}"; do
    p="$ROOT/$rel"; dest="$CACHE/${rel//\//-}"
    [ -d "$(dirname "$p")" ] || continue
    if [ -L "$p" ]; then
      [ "$(readlink "$p")" = "$dest" ] && continue
      echo "$rel 링크가 다른 곳을 가리킨다: $(readlink "$p") (기대: $dest) — 손으로 확인할 것" >&2; exit 1
    fi
    mkdir -p "$dest"
    if [ -d "$p" ]; then
      echo "$rel → $dest 로 옮기는 중($(du -sh "$p" | cut -f1))" >&2
      cp -a "$p/." "$dest/" && rm -rf "$p"
    fi
    ln -s "$dest" "$p"
    echo "$rel → $dest" >&2
  done
fi
echo "$CACHE"
