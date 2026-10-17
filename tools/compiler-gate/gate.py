#!/usr/bin/env python3
"""Phase 7a 컴파일러 바이트 동일 게이트 (RQ1 프로토콜 개정 #11).

진입점은 같은 디렉터리의 check-compiler-outputs.sh다. 사용법은 그 파일 머리말과 `--help`.

계층 (design-7a §2.2):
  A  호스트 OUT_DIR 생성 소스 22파일 (13 크레이트)
  B  실행 출력 증거 5건 + 소비자 테스트 1건(가드)
  C  wasm 23파일 (증거 21, 가드 2)
  D  허용 목록 가드 (git diff --name-only BASE CAND)

종료 코드: 0 통과, 1 비교 실패(바이트 다름 / D 위반), 2 하네스·전제 오류(개수 단언 실패 등),
           3 재현성 전제 불충족(base != base2, 개정 #11 적용 불가).
"""

import argparse
import fnmatch
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST = os.path.join(HERE, "manifest")
ALLOWLIST = os.path.join(HERE, "allowlist")

# 개수 단언(개정 #11 조건 1). manifest가 이 수와 다르면 하네스가 실패한다.
EXPECT = {"A": 22, "B": 5, "BT": 1, "C": 23}
EXPECT_C_EVIDENCE = 21
EXPECT_C_GUARD = 2
EXPECT_CONSUMER_CRATES = 13
COMPILER_PKG = "loom-compiler"
TOOLCHAIN_TRIPLE_WASM = "wasm32-unknown-unknown"
C11_BOOTSTRAP = "demo/resume/bootstrap.js"
JS_RECORD = [
    "demo/resume/bootstrap.js",
    "demo/widget-embed/flat/bootstrap-flat.js",
    "demo/resume/registry.js",
    "demo/resume/shared-store.js",
]
SCOPE = (
    "이 게이트는 'Phase 7a가 BASE 대비 컴파일러 소비자 산출물을 바꾸지 않았다'를 증명한다. "
    "동결 기반(트랙 L·T a55ba87, pkg-bench fb81633a…; 트랙 N 인증 기반 98f2d3d)과 같다는 것은 증명하지 않는다. "
    "HEAD(da4a9bc)는 이미 동결 기반이 아니다(HEAD에서 다시 빌드한 pkg-bench는 99bd8248…). "
    "L·T의 근거는 측정이 쓰는 demo/pkg-bench와 loom-bench 소스가 7a에서 바뀌지 않았다는 것이고 D 허용 목록이 보증한다. "
    "트랙 N은 G0 새 기반 캠페인에서 G2를 다시 받는다. (design-7a §2.6)"
)


class GateError(Exception):
    """하네스·전제 오류(종료 코드 2)."""


class ReproError(Exception):
    """재현성 전제 불충족(종료 코드 3)."""


def log(msg):
    print(f"[gate {time.strftime('%H:%M:%S')}] {msg}", file=sys.stderr, flush=True)


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


# ---------------------------------------------------------------- manifest

def load_manifest(path=MANIFEST):
    rows = []
    ids = set()
    with open(path, encoding="utf-8") as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) < 4:
                raise GateError(f"manifest:{lineno}: 필드 부족: {raw.rstrip()}")
            tier, rid, role, kind, *args = parts
            if rid in ids:
                raise GateError(f"manifest:{lineno}: 중복 id {rid}")
            ids.add(rid)
            if role not in ("evidence", "guard"):
                raise GateError(f"manifest:{lineno}: role {role}")
            arity = {"outdir": 2, "stdout": 2, "argfile": 3, "test": 1, "wasmpack": 3, "ficwasm": 2}
            if kind not in arity or len(args) != arity[kind]:
                raise GateError(f"manifest:{lineno}: kind/인자 오류: {raw.rstrip()}")
            rows.append({"tier": tier, "id": rid, "role": role, "kind": kind, "args": args})
    counts = {t: sum(1 for r in rows if r["tier"] == t) for t in EXPECT}
    if counts != EXPECT:
        raise GateError(f"manifest 개수 단언 실패: {counts} != {EXPECT}")
    c_ev = sum(1 for r in rows if r["tier"] == "C" and r["role"] == "evidence")
    c_gd = sum(1 for r in rows if r["tier"] == "C" and r["role"] == "guard")
    if (c_ev, c_gd) != (EXPECT_C_EVIDENCE, EXPECT_C_GUARD):
        raise GateError(f"manifest C 증거/가드 개수 단언 실패: {c_ev}/{c_gd}")
    unknown = {r["tier"] for r in rows} - set(EXPECT)
    if unknown:
        raise GateError(f"manifest 알 수 없는 tier {unknown}")
    return rows


def a_crates(rows):
    return sorted({r["args"][0] for r in rows if r["tier"] == "A"})


# ---------------------------------------------------------------- allowlist (D)

def load_allowlist(path=ALLOWLIST):
    rules = []
    with open(path, encoding="utf-8") as f:
        for raw in f:
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            kind, pat = line.split(None, 1)
            if kind not in ("prefix", "exact", "glob"):
                raise GateError(f"allowlist: 알 수 없는 규칙 {kind}")
            rules.append((kind, pat.strip()))
    return rules


def path_allowed(path, rules, allow_c11=False):
    if allow_c11 and path == C11_BOOTSTRAP:
        return True
    for kind, pat in rules:
        if kind == "prefix" and path.startswith(pat):
            return True
        if kind == "exact" and path == pat:
            return True
        if kind == "glob":
            ps, ss = pat.split("/"), path.split("/")
            if len(ps) == len(ss) and all(fnmatch.fnmatchcase(s, p) for s, p in zip(ss, ps)):
                return True
    return False


def d_check(paths, rules, allow_c11=False):
    violations = [p for p in paths if not path_allowed(p, rules, allow_c11)]
    return {"paths": len(paths), "violations": violations}


# ---------------------------------------------------------------- git helpers

def git(repo, *args, env=None):
    return subprocess.run(["git", "-C", repo, *args], check=True, capture_output=True,
                          text=True, env=env).stdout.strip()


def rev_parse(repo, rev):
    return git(repo, "rev-parse", "--verify", rev + "^{commit}")


# ---------------------------------------------------------------- build env

def build_env(target_dir, extra=None):
    env = dict(os.environ)
    for k in list(env):
        if k in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
                 "CARGO_BUILD_RUSTFLAGS", "CARGO_INCREMENTAL", "RUSTDOCFLAGS") or k.startswith("CARGO_PROFILE_"):
            del env[k]
    env["CARGO_TARGET_DIR"] = target_dir
    env["CARGO_TERM_COLOR"] = "never"
    if extra:
        env.update(extra)
    return env


def run(cmd, cwd, env, capture=True, log_path=None):
    """nice로 실행. capture=True면 stdout 바이트를 돌려준다. stderr는 log_path에 붙인다."""
    full = ["nice", "-n", "15"] + cmd
    log(f"$ {' '.join(cmd)}")
    with open(log_path, "ab") if log_path else open(os.devnull, "wb") as errf:
        if log_path:
            errf.write(f"\n$ {' '.join(cmd)}\n".encode())
            errf.flush()
        p = subprocess.run(full, cwd=cwd, env=env, stdout=subprocess.PIPE if capture else errf,
                           stderr=errf)
    if p.returncode != 0:
        raise GateError(f"명령 실패({p.returncode}): {' '.join(cmd)} (로그 {log_path})")
    return p.stdout if capture else b""


def cargo_json_messages(out_bytes):
    msgs = []
    for line in out_bytes.decode("utf-8", "replace").splitlines():
        line = line.strip()
        if line.startswith("{"):
            try:
                msgs.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return msgs


def outdirs_from_messages(msgs, ids_by_name):
    """build-script-executed에서 크레이트별 out_dir 목록(글롭 쓰지 않음)."""
    by_id = {v: k for k, v in ids_by_name.items()}
    found = {}
    for m in msgs:
        if m.get("reason") == "build-script-executed" and m.get("package_id") in by_id:
            found.setdefault(by_id[m["package_id"]], []).append(m["out_dir"])
    return found


# ---------------------------------------------------------------- toolchain

def toolchain_info(src):
    def out(cmd):
        try:
            return subprocess.run(cmd, cwd=src, capture_output=True, text=True).stdout.strip()
        except FileNotFoundError:
            return None
    info = {
        "rustc_vV": out(["rustc", "-Vv"]),
        "cargo_V": out(["cargo", "-V"]),
        "wasm_pack_V": out(["wasm-pack", "-V"]),
        "rustup_active_toolchain": out(["rustup", "show", "active-toolchain"]),
        "python": sys.version.split()[0],
    }
    cache = os.path.expanduser("~/.cache/.wasm-pack")
    opts = []
    if os.path.isdir(cache):
        for d in sorted(os.listdir(cache)):
            if d.startswith("wasm-opt-"):
                for cand in ("bin/wasm-opt", "wasm-opt"):
                    p = os.path.join(cache, d, cand)
                    if os.path.exists(p):
                        opts.append({"path": p, "version": out([p, "--version"])})
            if d.startswith("wasm-bindgen-"):
                p = os.path.join(cache, d, "wasm-bindgen")
                if os.path.exists(p):
                    opts.append({"path": p, "version": out([p, "--version"])})
    info["wasm_pack_cache_tools"] = opts
    lock = os.path.join(src, "Cargo.lock")
    wb = []
    if os.path.exists(lock):
        with open(lock) as f:
            txt = f.read()
        for block in txt.split("[[package]]"):
            if 'name = "wasm-bindgen"' in block:
                for l in block.splitlines():
                    if l.startswith("version"):
                        wb.append(l.split("=", 1)[1].strip().strip('"'))
    info["cargo_lock_wasm_bindgen"] = wb
    try:
        info["loadavg"] = open("/proc/loadavg").read().strip()
    except OSError:
        info["loadavg"] = None
    info["env_cargo_rust"] = {k: v for k, v in os.environ.items() if k.startswith(("CARGO", "RUST"))}
    return info


# ---------------------------------------------------------------- side

class Side:
    def __init__(self, G, rows, tiers, label, rev):
        self.G, self.rows, self.tiers, self.label, self.rev = G, rows, tiers, label, rev
        self.src = os.path.join(G, "src")
        self.target = os.path.join(G, "target")
        self.target_fic = os.path.join(G, "target-fic")
        self.dir = os.path.join(G, "sides", label)
        self.rowdir = os.path.join(self.dir, "rows")
        self.log = os.path.join(self.dir, "build.log")
        self.result = {"label": label, "rev": rev, "rows": {}, "outdirs": {}, "tiers": sorted(tiers)}

    def put(self, row, src_path):
        dst = os.path.join(self.rowdir, row["id"])
        shutil.copyfile(src_path, dst)
        size = os.path.getsize(dst)
        if size <= 0:
            raise GateError(f"{row['id']}: 크기 0")
        self.result["rows"][row["id"]] = {"sha256": sha256_file(dst), "size": size,
                                          "tier": row["tier"], "role": row["role"],
                                          "desc": " ".join([row["kind"], *row["args"]])}

    def put_bytes(self, row, data):
        p = os.path.join(self.rowdir, row["id"] + ".tmp")
        with open(p, "wb") as f:
            f.write(data)
        self.put(row, p)
        os.remove(p)

    def checkout(self):
        git(self.src, "checkout", "--quiet", "--detach", "--force", self.rev)
        git(self.src, "clean", "-fdxq")
        if git(self.src, "status", "--porcelain"):
            raise GateError("src 체크아웃이 clean이 아님")
        if git(self.src, "rev-parse", "HEAD") != self.rev:
            raise GateError("src HEAD != rev")

    def metadata_ids(self):
        out = subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=self.src,
                             env=build_env(self.target), check=True, capture_output=True).stdout
        meta = json.loads(out)
        consumers = {}
        for p in meta["packages"]:
            for d in p.get("dependencies", []):
                if d["name"] == COMPILER_PKG and d.get("kind") == "build":
                    consumers[p["name"]] = p["id"]
        all_ids = {p["name"]: p["id"] for p in meta["packages"]}
        return consumers, all_ids

    def build(self):
        if os.path.exists(self.dir):
            shutil.rmtree(self.dir)
        os.makedirs(self.rowdir)
        self.checkout()
        for t in (self.target, self.target_fic):
            shutil.rmtree(t, ignore_errors=True)   # 매번 새 target (design §2.3-2)
        consumers, all_ids = self.metadata_ids()
        want = a_crates(self.rows)
        if sorted(consumers) != want or len(want) != EXPECT_CONSUMER_CRATES:
            raise GateError(f"컴파일러 소비자 집합 불일치(양방향): metadata={sorted(consumers)} manifest={want}")
        self.result["consumer_crates"] = sorted(consumers)
        env = build_env(self.target)
        t0 = time.time()

        # ---- A
        out = run(["cargo", "build", "--workspace", "--lib", "--bins", "--message-format=json"],
                  self.src, env, log_path=self.log)
        dirs = outdirs_from_messages(cargo_json_messages(out), consumers)
        for c in want:
            if len(dirs.get(c, [])) != 1:
                raise GateError(f"A: {c}의 호스트 out_dir 개수 {len(dirs.get(c, []))} != 1")
        self.result["outdirs"]["host"] = {c: dirs[c][0] for c in want}
        files_in_outdirs = sum(len([f for f in os.listdir(dirs[c][0])
                                    if os.path.isfile(os.path.join(dirs[c][0], f))]) for c in want)
        a_rows = [r for r in self.rows if r["tier"] == "A"]
        if files_in_outdirs != len(a_rows):
            raise GateError(f"A: out_dir 파일 수 {files_in_outdirs} != manifest {len(a_rows)}")
        for r in a_rows:
            crate, fname = r["args"]
            p = os.path.join(dirs[crate][0], fname)
            if not os.path.isfile(p):
                raise GateError(f"A: {r['id']} {p} 없음")
            self.put(r, p)
        self.result["timing_A_s"] = round(time.time() - t0, 1)

        # ---- B
        if "B" in self.tiers:
            tdir = os.path.join(self.G, "tmp")
            shutil.rmtree(tdir, ignore_errors=True)
            os.makedirs(tdir)
            for r in self.rows:
                if r["tier"] != "B":
                    continue
                if r["kind"] == "stdout":
                    crate, binn = r["args"]
                    data = run(["cargo", "run", "-q", "-p", crate, "--bin", binn], self.src, env,
                               log_path=self.log)
                    self.put_bytes(r, data)
                elif r["kind"] == "argfile":
                    crate, binn, fname = r["args"]
                    p = os.path.join(tdir, fname)
                    run(["cargo", "run", "-q", "-p", crate, "--bin", binn, "--", p], self.src, env,
                        log_path=self.log)
                    self.put(r, p)
            # bin 이름 충돌(print_ssr/print_ssr_flat이 두 크레이트에 있음) 탐지: 4개 stdout은 서로 달라야 한다.
            shas = [self.result["rows"][r["id"]]["sha256"] for r in self.rows
                    if r["tier"] == "B" and r["kind"] == "stdout"]
            if len(set(shas)) != len(shas):
                raise GateError("B: stdout 행끼리 sha가 같음 — bin 이름 충돌로 다른 크레이트 바이너리를 실행했을 수 있음")
            for r in self.rows:
                if r["tier"] == "BT":
                    (crate,) = r["args"]
                    run(["cargo", "test", "-q", "-p", crate], self.src, env, capture=False, log_path=self.log)
                    self.result["rows"][r["id"]] = {"tier": "BT", "role": r["role"], "pass": True,
                                                    "desc": f"cargo test -p {crate}"}
            self.result["timing_B_s"] = round(time.time() - t0, 1)

        # ---- C
        if "C" in self.tiers:
            c_rows = [r for r in self.rows if r["tier"] == "C"]
            wp = sorted({r["args"][0] for r in c_rows if r["kind"] == "wasmpack"})
            wp_consumers = [c for c in wp if c in consumers]
            # wasm 쪽 out_dir 수집(같은 $G/target)
            cmd = ["cargo", "build", "--target", TOOLCHAIN_TRIPLE_WASM, "--release", "--lib",
                   "--message-format=json"]
            for c in wp_consumers:
                cmd += ["-p", c]
            out = run(cmd, self.src, env, log_path=self.log)
            wdirs = outdirs_from_messages(cargo_json_messages(out), consumers)
            for c in wp_consumers:
                if len(wdirs.get(c, [])) != 1:
                    raise GateError(f"C: {c}의 wasm out_dir 개수 {len(wdirs.get(c, []))} != 1")
            self.result["outdirs"]["wasm"] = {c: wdirs[c][0] for c in wp_consumers}
            pkgout = os.path.join(self.G, "pkgout")
            shutil.rmtree(pkgout, ignore_errors=True)
            built = set()
            for r in c_rows:
                if r["kind"] != "wasmpack":
                    continue
                crate, name, suffix = r["args"]
                od = os.path.join(pkgout, crate)
                if (crate, name) not in built:
                    run(["wasm-pack", "build", f"crates/{crate}", "--target", "web", "--release",
                         "--out-dir", od, "--out-name", name], self.src, env, capture=False,
                        log_path=self.log)
                    built.add((crate, name))
                self.put(r, os.path.join(od, name + suffix))
            # design-7a §2.2는 RUSTFLAGS='--cfg getrandom_backend="custom"'을 적었지만 da4a9bc에서 그 명령은
            # 빌드되지 않는다(getrandom 0.4.3: 워크스페이스가 켠 wasm_js feature와 custom cfg가 겹쳐
            # `Error::WEB_CRYPTO` 미정의, E0599). crates/loom-fic/examples/wasm_in_process.py가 적은 대로
            # RUSTFLAGS 없는 `cargo build --target wasm32-unknown-unknown`을 쓴다. target은 그대로 분리한다.
            fenv = build_env(self.target_fic)
            for r in c_rows:
                if r["kind"] != "ficwasm":
                    continue
                crate, fname = r["args"]
                out = run(["cargo", "build", "-p", crate, "--target", TOOLCHAIN_TRIPLE_WASM, "--release",
                           "--lib", "--message-format=json"], self.src, fenv, log_path=self.log)
                fd = outdirs_from_messages(cargo_json_messages(out), consumers)
                if len(fd.get(crate, [])) != 1:
                    raise GateError(f"C: {crate} fic wasm out_dir 개수 != 1")
                self.result["outdirs"].setdefault("wasm", {})[crate + "(fic)"] = fd[crate][0]
                self.put(r, os.path.join(self.target_fic, TOOLCHAIN_TRIPLE_WASM, "release", fname))
            self.result["timing_C_s"] = round(time.time() - t0, 1)

        # 빌드가 소스 트리를 건드리지 않았는지(Cargo.lock 갱신 등)
        dirty = git(self.src, "status", "--porcelain")
        if dirty:
            raise GateError(f"빌드 후 src가 더러워짐:\n{dirty}")
        self.result["js_sha256"] = {p: sha256_file(os.path.join(self.src, p)) for p in JS_RECORD}
        self.result["toolchain"] = toolchain_info(self.src)
        self.result["counts"] = count_rows(self.result["rows"])
        assert_side_complete(self.result, self.rows, self.tiers)
        with open(os.path.join(self.dir, "side.json"), "w") as f:
            json.dump(self.result, f, ensure_ascii=False, indent=1, sort_keys=True)
        log(f"side {self.label} 완료: {self.result['counts']}")
        return self.result


def count_rows(rows):
    c = {}
    for v in rows.values():
        key = v["tier"] if v["tier"] != "C" else f"C_{v['role']}"
        c[key] = c.get(key, 0) + 1
    return c


def assert_side_complete(side, rows, tiers):
    for r in rows:
        tier = "B" if r["tier"] == "BT" else r["tier"]
        if tier in tiers and r["id"] not in side["rows"]:
            raise GateError(f"side {side.get('label')}: 행 {r['id']} 누락")
    want_ids = {r["id"] for r in rows if ("B" if r["tier"] == "BT" else r["tier"]) in tiers}
    extra = set(side["rows"]) - want_ids
    if extra:
        raise GateError(f"side {side.get('label')}: manifest 밖 행 {sorted(extra)}")


def load_side(path):
    with open(os.path.join(path, "side.json")) as f:
        s = json.load(f)
    s["_dir"] = path
    return s


def rehash_side(side):
    """side.json을 믿지 않고 rows/ 파일을 다시 해시한다(self-test가 1바이트 변조를 잡도록)."""
    out = {}
    for rid, v in side["rows"].items():
        if v["tier"] == "BT":
            out[rid] = "pass" if v.get("pass") else "fail"
            continue
        p = os.path.join(side["_dir"], "rows", rid)
        out[rid] = sha256_file(p) if os.path.isfile(p) else None
    return out


def compare(a, b, rows, tiers):
    """행별 비교. 반환: (요약 dict, 실패 목록)."""
    assert_side_complete(a, rows, tiers)
    assert_side_complete(b, rows, tiers)
    ha, hb = rehash_side(a), rehash_side(b)
    fails, pairs = [], {}
    for r in rows:
        tier = "B" if r["tier"] == "BT" else r["tier"]
        if tier not in tiers:
            continue
        x, y = ha.get(r["id"]), hb.get(r["id"])
        ok = x is not None and x == y and (r["tier"] != "BT" or x == "pass")
        pairs[r["id"]] = {"tier": r["tier"], "role": r["role"], "desc": " ".join([r["kind"], *r["args"]]),
                          a["label"]: x, b["label"]: y, "equal": ok}
        if not ok:
            fails.append(r["id"])
    # out_dir 절대경로 동일성(위험 1)
    od = {}
    for kind in ("host", "wasm"):
        if kind in a.get("outdirs", {}) or kind in b.get("outdirs", {}):
            same = a["outdirs"].get(kind) == b["outdirs"].get(kind)
            od[kind] = same
            if not same:
                fails.append(f"outdirs:{kind}")
    def cnt(tier, role=None):
        sel = [r for r in rows if r["tier"] == tier and (role is None or r["role"] == role) and r["id"] in pairs]
        return sum(1 for r in sel if pairs[r["id"]]["equal"]), len(sel)
    summary = {"pairs": pairs, "outdir_equal": od, "failures": fails}
    parts = []
    if "A" in tiers:
        e, n = cnt("A"); parts.append(f"A {e}/{n}")
        summary["A"] = [e, n]
    if "B" in tiers:
        e, n = cnt("B"); te, tn = cnt("BT")
        parts.append(f"B {e}/{n}(+소비자 테스트 {te}/{tn})")
        summary["B"] = [e, n]; summary["BT"] = [te, tn]
    if "C" in tiers:
        e, n = cnt("C"); ee, en = cnt("C", "evidence"); ge, gn = cnt("C", "guard")
        parts.append(f"C {e}/{n}(증거 {ee}/{en}, 가드 {ge}/{gn})")
        summary["C"] = [e, n]
    summary["line"] = "  ".join(parts)
    return summary, fails


# ---------------------------------------------------------------- self-test

def self_test(side_dir):
    """design §2.3-6: 변조 1건 → 정확히 1실패, 행 삭제 → 개수 단언 실패, 허용 밖 경로 → D 실패."""
    rows = load_manifest()
    base = load_side(side_dir)
    tiers = set(base["tiers"])
    results = []

    def check(name, cond):
        results.append((name, bool(cond)))
        log(f"self-test {'PASS' if cond else 'FAIL'}: {name}")

    with tempfile.TemporaryDirectory(dir=os.path.dirname(os.path.abspath(side_dir))) as td:
        cp = os.path.join(td, "copy")
        shutil.copytree(side_dir, cp)
        copy = load_side(cp)
        copy["label"] = "copy"
        s, f = compare(base, copy, rows, tiers)
        check("변조 없는 사본은 0 실패", f == [])
        # 1바이트 변조(행마다 한 번씩: 모든 행에서 정확히 그 1건이 잡히는지)
        flip_ok = True
        for rid, v in base["rows"].items():
            if v["tier"] == "BT":
                continue
            p = os.path.join(cp, "rows", rid)
            with open(p, "r+b") as fh:
                b0 = fh.read(1)
                fh.seek(0)
                fh.write(bytes([b0[0] ^ 0x01]))
            _, f = compare(base, copy, rows, tiers)
            if f != [rid]:
                flip_ok = False
                log(f"  변조 {rid} → 실패 목록 {f}")
            with open(p, "r+b") as fh:
                fh.write(b0)
        check(f"모든 행({sum(1 for v in base['rows'].values() if v['tier'] != 'BT')}개)에서 1바이트 변조가 정확히 그 1건으로 잡힘", flip_ok)
        # 소비자 테스트 실패 기록 → 실패
        bt = [rid for rid, v in base["rows"].items() if v["tier"] == "BT"]
        if bt:
            copy["rows"][bt[0]] = dict(copy["rows"][bt[0]], **{"pass": False})
            _, f = compare(base, copy, rows, tiers)
            check("소비자 테스트 실패가 실패로 잡힘", f == [bt[0]])
            copy["rows"][bt[0]]["pass"] = True
        # side에서 행 파일 하나 삭제 → 실패
        some = next(rid for rid, v in base["rows"].items() if v["tier"] != "BT")
        os.rename(os.path.join(cp, "rows", some), os.path.join(td, "hold"))
        _, f = compare(base, copy, rows, tiers)
        check("행 파일 삭제가 실패로 잡힘", some in f)
        os.rename(os.path.join(td, "hold"), os.path.join(cp, "rows", some))
        # side.json에서 행 하나 삭제 → 개수(완전성) 단언 실패
        saved = copy["rows"].pop(some)
        try:
            compare(base, copy, rows, tiers)
            check("side 행 누락이 완전성 단언으로 실패", False)
        except GateError:
            check("side 행 누락이 완전성 단언으로 실패", True)
        copy["rows"][some] = saved
        # out_dir 경로 변경 → 실패
        if "host" in copy.get("outdirs", {}):
            k = sorted(copy["outdirs"]["host"])[0]
            orig = copy["outdirs"]["host"][k]
            copy["outdirs"]["host"][k] = orig + "-x"
            _, f = compare(base, copy, rows, tiers)
            check("호스트 out_dir 경로 차이가 실패로 잡힘", "outdirs:host" in f)
            copy["outdirs"]["host"][k] = orig
        # manifest 행 삭제 → 개수 단언 실패
        with open(MANIFEST, encoding="utf-8") as fh:
            lines = fh.readlines()
        for tier in ("A", "B", "C"):
            idx = next(i for i, l in enumerate(lines) if l.startswith(f"{tier} "))
            mp = os.path.join(td, f"manifest-{tier}")
            with open(mp, "w", encoding="utf-8") as fh:
                fh.writelines(lines[:idx] + lines[idx + 1:])
            try:
                load_manifest(mp)
                check(f"manifest {tier} 행 삭제가 개수 단언으로 실패", False)
            except GateError:
                check(f"manifest {tier} 행 삭제가 개수 단언으로 실패", True)
        # D: 가짜 diff 목록
        rules = load_allowlist()
        ok_paths = ["crates/loom-compiler/src/lib.rs", "crates/loom-demo/build.rs", "docs/09-x.md",
                    "research-questions/RQ1-benchmark-protocol.md",
                    "research-questions/data/phase7a-byte-gate-2026-10-17-C0.json",
                    "tools/compiler-gate/gate.py", "roadmap/PHASES.md"]
        check("D: 허용 경로만 → 위반 0", d_check(ok_paths, rules)["violations"] == [])
        bad = ["Cargo.lock", "Cargo.toml", "crates/loom-compiler-x/src/lib.rs", "crates/loom-runtime/src/web.rs",
               "crates/loom-bench/src/lib.rs", "crates/loom-demo/Cargo.toml", "crates/loom-demo/src/lib.rs",
               "demo/widget-embed.tsx", C11_BOOTSTRAP, "bench/e2e/run.mjs",
               "research-questions/analysis/rq1-verdicts.py", "research-questions/data/rq6-x.json",
               "research-questions/data/sub/x.md"]
        for p in bad:
            check(f"D: 허용 밖 경로 {p} → 위반", d_check(ok_paths + [p], rules)["violations"] == [p])
        check("D: C11 예외는 플래그가 있을 때만", d_check([C11_BOOTSTRAP], rules, allow_c11=True)["violations"] == [])
    failed = [n for n, ok in results if not ok]
    return {"checks": len(results), "failed": failed, "side": os.path.abspath(side_dir)}


# ---------------------------------------------------------------- main

def ensure_src(G, repo):
    """$G/src: 공유 객체 클론(design은 `git worktree add`; 주 저장소 .git/worktrees에 등록하지 않으려고
    --shared 클론으로 대체 — 같은 고정 경로·같은 객체라 격리 효과는 같다)."""
    src = os.path.join(G, "src")
    if not os.path.isdir(os.path.join(src, ".git")):
        os.makedirs(G, exist_ok=True)
        common = git(repo, "rev-parse", "--path-format=absolute", "--git-common-dir")
        subprocess.run(["git", "clone", "--quiet", "--shared", "--no-checkout", common, src], check=True)
    return src


def cmd_run(args):
    G = os.path.abspath(args.work)
    repo = os.path.abspath(args.repo)
    rows = load_manifest()
    tiers = set(args.tiers.split(","))
    if not tiers <= {"A", "B", "C", "D"}:
        raise GateError(f"tiers {tiers}")
    if "C" in tiers or "B" in tiers:
        tiers.add("A")  # B·C는 A의 빌드·out_dir 단언 위에서 돈다
    if args.cand_from_index:
        # 작업 트리 == 인덱스여야 후보 트리가 실제 작업 내용과 같다(추적 안 하는 파일은 후보에 들어가지 않는다).
        if subprocess.run(["git", "-C", repo, "diff", "--quiet"]).returncode != 0:
            raise GateError("작업 트리에 스테이징 안 된 변경이 있음")
    elif git(repo, "status", "--porcelain", "--untracked-files=no"):
        raise GateError("저장소 트리가 clean이 아님")
    base = rev_parse(repo, args.base)
    record = {"scope": SCOPE, "date": time.strftime("%Y-%m-%d"), "tiers": sorted(tiers),
              "base": base, "work_dir": G}
    if args.cand_from_index:
        tree = git(repo, "write-tree")
        parent = rev_parse(repo, "HEAD")
        cand = git(repo, "commit-tree", tree, "-p", parent, "-m", "phase7a gate candidate (temp)")
        record.update({"parent": parent, "cand_tree": tree, "cand_temp_commit": cand,
                       "cand_note": "후보 = 인덱스의 write-tree(기록 JSON 자신은 제외). 커밋 뒤 "
                                    "`verify-commit`이 HEAD 트리에서 기록 파일을 뺀 트리 == cand_tree 를 단언한다."})
    else:
        cand = rev_parse(repo, args.cand)
        record.update({"parent": git(repo, "rev-parse", cand + "^"), "cand_commit": cand,
                       "cand_tree": git(repo, "rev-parse", cand + "^{tree}")})
    record["compiler_subtree"] = {"base": git(repo, "rev-parse", f"{base}:crates/loom-compiler"),
                                  "cand": git(repo, "rev-parse", f"{cand}:crates/loom-compiler")}
    exit_code = 0

    # D
    if "D" in tiers:
        paths = git(repo, "diff", "--name-only", base, cand).splitlines()
        d = d_check(paths, load_allowlist(), args.allow_c11_bootstrap)
        if args.expect_compiler_change:
            d["compiler_diff_exists"] = bool(subprocess.run(
                ["git", "-C", repo, "diff", "--quiet", base, cand, "--", "crates/loom-compiler"]).returncode)
            if not d["compiler_diff_exists"]:
                d["violations"].append("<존재 단언 실패: crates/loom-compiler diff 없음>")
        d["changed_paths"] = paths
        record["D"] = d
        log(f"D: 경로 {d['paths']}, 위반 {len(d['violations'])}")
        if d["violations"]:
            exit_code = 1

    build_tiers = tiers - {"D"}
    if build_tiers:
        ensure_src(G, repo)
        sides = {}
        plan = [("base", base)] + ([("base2", base)] if args.base2 else []) + [("cand", cand)]
        for label, rev in plan:
            if args.reuse and label in args.reuse.split(","):
                sides[label] = load_side(os.path.join(G, "sides", label))
                if sides[label]["rev"] != rev or set(sides[label]["tiers"]) != build_tiers:
                    raise GateError(f"재사용 side {label}의 rev/tiers 불일치")
                log(f"side {label} 재사용")
            else:
                Side(G, rows, build_tiers, label, rev).build()
                sides[label] = load_side(os.path.join(G, "sides", label))
        record["toolchain"] = sides["base"]["toolchain"]
        record["js_sha256"] = {k: sides[k]["js_sha256"] for k in sides}
        record["outdirs"] = sides["base"]["outdirs"]
        record["timing_s"] = {k: {t: sides[k].get(f"timing_{t}_s") for t in "ABC"} for k in sides}
        if args.base2:
            s2, f2 = compare(sides["base"], sides["base2"], rows, build_tiers)
            record["reproducibility"] = {"line": s2["line"], "failures": f2, "outdir_equal": s2["outdir_equal"],
                                         "pairs": s2["pairs"]}
            log(f"재현성 base vs base2: {s2['line']}  실패 {f2}")
            if f2:
                record["result"] = "재현성 전제 불충족, 개정 #11 적용 불가"
                write_record(args.record, record)
                raise ReproError("재현성 전제 불충족, 개정 #11 적용 불가")
        s, f = compare(sides["base"], sides["cand"], rows, build_tiers)
        record["comparison"] = {"line": s["line"], "failures": f, "outdir_equal": s["outdir_equal"],
                                "counts": {k: s[k] for k in ("A", "B", "BT", "C") if k in s}}
        record["rows"] = s["pairs"]
        if f:
            exit_code = 1
    line = record.get("comparison", {}).get("line", "")
    if "D" in tiers:
        line += f"  D 위반 {len(record['D']['violations'])}"
    record["summary"] = line.strip()
    record["result"] = "PASS" if exit_code == 0 else "FAIL"
    write_record(args.record, record)
    print(record["summary"] + f"  => {record['result']}")
    return exit_code


def write_record(path, record):
    if path:
        os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
        with open(path, "w", encoding="utf-8") as f:
            json.dump(record, f, ensure_ascii=False, indent=1, sort_keys=True)
            f.write("\n")
        log(f"기록: {path}")


def cmd_verify_commit(args):
    """커밋 뒤: HEAD 트리에서 기록 파일을 뺀 트리 == 기록의 cand_tree, HEAD^ == 기록의 parent."""
    repo = os.path.abspath(args.repo)
    rec_rel = os.path.relpath(os.path.abspath(args.record), repo)
    with open(args.record, encoding="utf-8") as f:
        rec = json.load(f)
    head = rev_parse(repo, args.rev)
    env = dict(os.environ)
    with tempfile.NamedTemporaryFile(dir=os.path.join(args.work), delete=False) as tf:
        idx = tf.name
    try:
        env["GIT_INDEX_FILE"] = idx
        git(repo, "read-tree", head, env=env)
        git(repo, "rm", "--cached", "--quiet", rec_rel, env=env)
        tree = git(repo, "write-tree", env=env)
    finally:
        os.remove(idx)
    parent = git(repo, "rev-parse", head + "^")
    ok = tree == rec["cand_tree"] and parent == rec["parent"]
    print(f"verify-commit: HEAD={head} 트리(기록 제외)={tree} 기록 cand_tree={rec['cand_tree']} "
          f"parent={parent} 기록 parent={rec['parent']} => {'OK' if ok else 'MISMATCH'}")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("run")
    r.add_argument("--repo", default=os.getcwd())
    r.add_argument("--work", required=True, help="$G: 고정 절대경로(src/, target/, sides/)")
    r.add_argument("--base", required=True)
    g = r.add_mutually_exclusive_group(required=True)
    g.add_argument("--cand")
    g.add_argument("--cand-from-index", action="store_true")
    r.add_argument("--tiers", default="A,B,D")
    r.add_argument("--base2", action="store_true")
    r.add_argument("--record")
    r.add_argument("--reuse", help="쉼표 목록: 이전 실행의 side를 다시 쓴다(rev·tiers 일치 단언)")
    r.add_argument("--expect-compiler-change", action="store_true",
                   help="존재 단언: crates/loom-compiler diff가 있어야 함(C1–C9, C11)")
    r.add_argument("--allow-c11-bootstrap", action="store_true")
    s = sub.add_parser("self-test")
    s.add_argument("--side", required=True)
    s.add_argument("--record")
    v = sub.add_parser("verify-commit")
    v.add_argument("--repo", default=os.getcwd())
    v.add_argument("--work", required=True)
    v.add_argument("--record", required=True)
    v.add_argument("--rev", default="HEAD")
    m = sub.add_parser("check-manifest")
    a = ap.parse_args()
    try:
        if a.cmd == "run":
            return cmd_run(a)
        if a.cmd == "self-test":
            res = self_test(a.side)
            print(json.dumps(res, ensure_ascii=False))
            if a.record:
                with open(a.record, encoding="utf-8") as f:
                    rec = json.load(f)
                rec["self_test"] = res
                write_record(a.record, rec)
            return 0 if not res["failed"] else 1
        if a.cmd == "verify-commit":
            return cmd_verify_commit(a)
        if a.cmd == "check-manifest":
            rows = load_manifest()
            print(f"manifest OK: {len(rows)}행 {EXPECT}")
            return 0
    except ReproError as e:
        print(f"재현성 전제 불충족, 개정 #11 적용 불가: {e}", file=sys.stderr)
        return 3
    except GateError as e:
        print(f"하네스 오류: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
