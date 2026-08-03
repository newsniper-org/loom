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

    def do_POST(self):
        """`/__collect?name=<파일명>`로 수집 결과를 받아 저장한다.

        **왜 필요한가 (2026-08-03)**: 지금까지 결과 추출은 Playwright가
        `#results`를 읽는 방식이었고, 그래서 **Playwright가 번들한 브라우저
        빌드**로만 측정할 수 있었다. 그런데 조사 결과 Playwright의 Firefox
        빌드가 stock Firefox와 크게 다르다는 것이 밝혀졌다 — 같은 wasm
        바이너리로 stock Firefox 150.0.1은 Chromium과 동률(0.91×)인데
        Playwright 빌드는 8.1~8.4배 느리다(그 바이너리를 Playwright 없이
        직접 띄워도 동일하므로 드라이버가 아니라 빌드의 성질).

        이 엔드포인트가 있으면 **아무 브라우저나** `--headless`로 띄워서
        측정할 수 있다. 측정이 전부 끝난 뒤 한 번 호출되므로 측정 구간에는
        영향이 없다.
        """
        from urllib.parse import urlparse, parse_qs

        q = parse_qs(urlparse(self.path).query)
        name = (q.get("name") or ["collect.json"])[0]
        # 경로 탈출 차단 — 파일명만 받는다.
        name = os.path.basename(name)
        if not name.endswith(".json"):
            name += ".json"
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length)
        out_dir = os.path.join(REPO_ROOT, "bench", "collected")
        os.makedirs(out_dir, exist_ok=True)
        # `.progress`로 끝나는 이름은 **append**한다 — 조합마다 오는 진행
        # 보고라 마지막 것만 남으면 어디서 느려졌는지 알 수 없다.
        if name.endswith(".part.json"):
            # 그룹 모드 조각 — 한 줄씩 append하고, 마지막 그룹이 오면
            # 호출부(collect-stock.sh)가 합친다.
            with open(os.path.join(out_dir, name), "ab") as fh:
                fh.write(body + b"\n")
        elif name.endswith(".progress.json"):
            with open(os.path.join(out_dir, name), "ab") as fh:
                fh.write(body + b"\n")
        else:
            with open(os.path.join(out_dir, name), "wb") as fh:
                fh.write(body)
        self.send_response(204)
        self.end_headers()
        print(f"[collect] {name} ← {len(body)} bytes", file=sys.stderr, flush=True)

    def log_message(self, fmt, *args):
        pass  # 측정 중 콘솔 소음 제거


if __name__ == "__main__":
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8931
    with http.server.ThreadingHTTPServer(("127.0.0.1", port), IsolatedHandler) as httpd:
        print(f"serving {REPO_ROOT} on http://127.0.0.1:{port} (COOP/COEP on)")
        httpd.serve_forever()
