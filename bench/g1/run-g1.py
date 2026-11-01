#!/usr/bin/env python3
"""RQ1 G1 정합성 게이트 드라이버 (프로토콜 개정 #12 후속 8, 설계 roadmap/rq1-g1-gate-design.md).

demo/g1.html을 띄워 G1 산출물(`demo/pkg-g1-{release,dev}`, `--features g1` 빌드)의 g1_run_l / g1_run_t를 셀마다 부르고
원장 JSON을 쓴다. 시간을 재지 않는다. 셀마다 결과를 원장에 즉시 덧쓴다(중단돼도 앞 셀은 남는다).
wasm 패닉(= dev 빌드의 debug_assert 발화 포함)은 그 셀의 오류로 기록하고 페이지를 다시 띄운다.

사용: python3 bench/g1/run-g1.py --browser chromium --pkg pkg-g1-release --out <원장.json> [--fallback] [--only l:0-5,t:0-2]
WebKit은 호스트에서 뜨지 않으므로 bench/g1/run-g1-webkit.sh(컨테이너)로 돌린다.
"""
import argparse, hashlib, http.server, json, os, socketserver, subprocess, sys, threading, time
from playwright.sync_api import sync_playwright

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))


def serve(port):
    class H(http.server.SimpleHTTPRequestHandler):
        def __init__(self, *a, **k):
            super().__init__(*a, directory=ROOT, **k)
        def log_message(self, *a):
            pass
    socketserver.TCPServer.allow_reuse_address = True
    httpd = socketserver.TCPServer(("127.0.0.1", port), H)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd


def parse_only(s):
    sel = {"l": None, "t": None}
    if not s:
        return sel
    for part in s.split(","):
        k, rng = part.split(":")
        a, _, b = rng.partition("-")
        sel[k] = set(range(int(a), int(b or a) + 1)) | (sel[k] or set())
    for k in sel:
        if sel[k] is None:
            sel[k] = set()
    return sel


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--browser", default="chromium")
    ap.add_argument("--pkg", default="pkg-g1-release")
    ap.add_argument("--out", required=True)
    ap.add_argument("--fallback", action="store_true")
    ap.add_argument("--only", default="")
    ap.add_argument("--port", type=int, default=8766)
    ap.add_argument("--shard", default="0/1", help="i/N — 셀 인덱스 mod N == i 인 셀만(병렬 실행용). 원장 meta에 기록된다.")
    a = ap.parse_args()
    wasm = os.path.join(ROOT, "demo", a.pkg, "loom_bench_bg.wasm")
    sha = hashlib.sha256(open(wasm, "rb").read()).hexdigest()
    head = subprocess.run(["git", "-C", ROOT, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(["git", "-C", ROOT, "status", "--porcelain", "--", "crates", "demo/g1.html", "bench/g1"], capture_output=True, text=True).stdout.strip())
    sel = parse_only(a.only)
    shard_i, shard_n = (int(x) for x in a.shard.split("/"))
    httpd = serve(a.port)
    url = f"http://127.0.0.1:{a.port}/demo/g1.html?pkg={a.pkg}" + ("&fallback=1" if a.fallback else "")
    ledger = {"meta": {"protocol": "RQ1 개정 #12 후속 8 (G1)", "browser": a.browser, "pkg": a.pkg, "wasm_sha256": sha,
                       "source_commit": head, "source_dirty": dirty, "fallback_forced": a.fallback, "shard": a.shard, "started": time.strftime("%Y-%m-%dT%H:%M:%S%z")},
              "track_l": [], "track_t": [], "errors": []}

    def flush():
        tmp = a.out + ".tmp"
        json.dump(ledger, open(tmp, "w"), ensure_ascii=False, indent=1)
        os.replace(tmp, a.out)

    with sync_playwright() as p:
        b = getattr(p, a.browser).launch()
        ledger["meta"]["browser_version"] = b.version
        state = {}

        def open_page():
            if state.get("pg"):
                state["pg"].close()
            pg = b.new_page()
            state["errs"] = []
            pg.on("pageerror", lambda e: state["errs"].append(str(e)))
            pg.on("console", lambda m: state["errs"].append(m.text) if m.type == "error" else None)
            pg.goto(url)
            pg.wait_for_function("window.g1_ready === true", timeout=60000)
            state["pg"] = pg
            return pg

        pg = open_page()
        ledger["meta"]["g1_meta_start"] = json.loads(pg.evaluate("window.g1.g1_meta()"))
        ledger["meta"]["native_move_before"] = pg.evaluate("window.g1_native_move_before")
        ledger["meta"]["move_before_after_page_setup"] = pg.evaluate("typeof Element.prototype.moveBefore === 'function'")
        cells = json.loads(pg.evaluate("window.g1.g1_cells()"))
        ledger["meta"]["cells"] = {"track_l": len(cells["track_l"]), "track_t": len(cells["track_t"])}
        t0 = time.time()
        for track, fn in (("track_l", "g1_run_l"), ("track_t", "g1_run_t")):
            key = track[-1]
            for i, label in enumerate(cells[track]):
                if sel[key] is not None and i not in sel[key]:
                    continue
                if i % shard_n != shard_i:
                    continue
                state["errs"].clear()
                try:
                    res = json.loads(state["pg"].evaluate(f"window.g1.{fn}({i})"))
                    res["page_errors"] = list(state["errs"])
                    ledger[track].append(res)
                except Exception as e:  # 패닉·타임아웃 — 그 셀의 오류로 기록하고 페이지를 새로 띄운다
                    ledger["errors"].append({"track": track, "index": i, "cell": label, "error": str(e)[:2000], "page_errors": list(state["errs"])[:5]})
                    open_page()
                flush()
                print(f"[{time.time()-t0:7.0f}s] {track} {i+1}/{len(cells[track])} {label}", flush=True)
        ledger["meta"]["g1_meta_end"] = json.loads(state["pg"].evaluate("window.g1.g1_meta()"))
        ledger["meta"]["finished"] = time.strftime("%Y-%m-%dT%H:%M:%S%z")
        flush()
        b.close()
    httpd.shutdown()


if __name__ == "__main__":
    main()
