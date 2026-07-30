#!/usr/bin/env python3
"""저장소 루트를 정적 서빙하되 COOP/COEP 헤더를 붙이는 최소 서버.

`RQ1-benchmark-protocol.md` §8 타이머 조항이 "COOP/COEP 크로스오리진 격리로
고해상도 타이머 확보"를 **사전 등록**해 두었는데, 지금까지 벤치를 띄우던
`python3 -m http.server`는 이 헤더를 못 붙인다. 격리가 없으면
`performance.now()`가 100µs로 클램프되고(Chromium 기준), 트랙 T의 토글 비용은
원래 그 클램프 바로 위에서 놀기 때문에(RQ1의 "묻혀 판별 불가" 기록)
사실상 측정 자체가 안 된다 — 즉 이 파일이 없으면 §8을 지킨 트랙 T 측정이
불가능하다.

    python3 bench/serve-isolated.py [PORT]

기본 포트 8931(기존 e2e 하네스와 동일).
"""

import http.server
import os
import sys

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


class IsolatedHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=REPO_ROOT, **kwargs)

    def end_headers(self):
        # 이 둘이 함께 있어야 `crossOriginIsolated === true`가 되고, 그래야
        # 브라우저가 타이머 클램프를 푼다.
        self.send_header("Cross-Origin-Opener-Policy", "same-origin")
        self.send_header("Cross-Origin-Embedder-Policy", "require-corp")
        # 벤치 결과가 캐시된 이전 wasm으로 오염되지 않게.
        self.send_header("Cache-Control", "no-store")
        super().end_headers()

    def log_message(self, fmt, *args):
        pass  # 측정 중 콘솔 소음 제거


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8931
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), IsolatedHandler) as httpd:
        print(f"serving {REPO_ROOT} on http://127.0.0.1:{port} (COOP/COEP on)")
        httpd.serve_forever()
