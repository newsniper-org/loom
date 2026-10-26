#!/usr/bin/env python3
"""두 wasm(이름 구역 있는 진단 빌드)의 **함수 단위 diff** — Phase 7c 설계 §5(b).

목적: 공유 크레이트(`loom-runtime`)를 바꾼 뒤 "측정 arm이 실행하는 함수가 바뀌었는가"를 사람이 검토할 수 있는 목록으로 만든다.
**이 도구의 출력은 개정 #11 면제의 근거가 아니다** — 측정 산출물은 어차피 바이트가 달라진다. 이 도구는 "K/C가 자연 대조군이다"라는 공평성
서술을 지지하거나(허용 목록 밖의 변경이 0) 반박한다(있으면 프로토콜 개정 #12 후속 1의 사전 선언 규칙 (a)가 발동).

정규화(이 셋을 하지 않으면 함수가 추가될 때마다 전부 달라 보인다):
  1. 인덱스 주석 `(;N;)` 제거 — 함수 번호는 추가/삭제로 밀린다. 호출은 이름(`call $name`)으로 남는다.
  2. `(type N)`·`call_indirect (type N)`의 N을 시그니처 텍스트로 치환 — 타입 번호도 밀린다.
  3. 데이터 세그먼트 주소 범위 안의 `i32.const`를 `<DATA>`로 마스킹 — panic Location 문자열의 주소가 데이터가 늘면 밀린다
     (개정 #11의 189바이트 사례). 이 마스킹은 정당한 변경(문자열을 가리키는 주소)을 숨길 수 있으므로 마스킹 개수를 함께 보고한다.

공허한 확인 방지: 총 함수 수 대조(동일+변경+삭제 = A 전체, 동일+변경+추가 = B 전체), 이름이 있는 빌드인지(이름 구역 없음 → 실패),
`--require-changed REGEX`로 "반드시 변경돼야 하는 함수"(예: `LoomApp.*tick`)가 실제로 변경 목록에 있는지 단언.

사용:
  wasm_func_diff.py A.wasm B.wasm --out report.json [--require-changed REGEX ...] [--allow allow.txt]
allow.txt: 한 줄에 `정규식<TAB>사유`. 변경된 함수가 하나라도 어느 줄에도 안 걸리면 종료 코드 1 ("허용 목록 밖의 변경").
"""
import argparse
import difflib
import hashlib
import json
import re
import subprocess
import sys


def wasm_print(path):
    return subprocess.run(["wasm-tools", "print", path], check=True, capture_output=True, text=True).stdout


def lit_len(s):
    """WAT 문자열 리터럴(따옴표 안쪽)의 바이트 길이."""
    n = 0
    i = 0
    while i < len(s):
        c = s[i]
        if c != "\\":
            n += len(c.encode())
            i += 1
            continue
        d = s[i + 1]
        if d in "ntr\\\"'":
            n += 1
            i += 2
        elif d == "u":
            j = s.index("}", i)
            cp = int(s[i + 3 : j], 16)
            n += len(chr(cp).encode())
            i = j + 1
        else:  # \hh
            n += 1
            i += 3
    return n


def parse(path):
    text = wasm_print(path)
    lines = text.split("\n")
    types = {}
    for ln in lines:
        m = re.match(r"\s*\(type \(;(\d+);\) \(func(.*)\)\)\s*$", ln)
        if m:
            types[int(m.group(1))] = m.group(2).strip()
    # 데이터 주소 범위
    lo, hi = None, 0
    for ln in lines:
        m = re.match(r'\s*\(data (?:\$\S+ )?(?:\(;\d+;\) )?\(i32\.const (\d+)\) "(.*)"\)\s*$', ln)
        if m:
            off = int(m.group(1))
            end = off + lit_len(m.group(2))
            lo = off if lo is None else min(lo, off)
            hi = max(hi, end)
    table = {}
    for ln in lines:
        m = re.match(r"\s*\(elem (?:\(;\d+;\) )?\(i32\.const (\d+)\) func (.*)\)\s*$", ln)
        if m:
            off = int(m.group(1))
            names = re.findall(r'\$"(?:[^"\\]|\\.)*"|\$\S+', m.group(2))
            for k, nme in enumerate(names):
                table[off + k] = nme
    funcs = {}
    order = []
    i = 0
    named = 0
    while i < len(lines):
        ln = lines[i]
        m = re.match(r"  \(func (\$\S+)? ?\(;(\d+);\)(.*)$", ln)
        if m is None and ln.startswith("  (func "):
            m = re.match(r"  \(func (\$\S+)?(.*)$", ln)
        if ln.startswith("  (func "):
            j = i
            # 본문 없는 함수는 한 줄로 출력된다(`(func $f (;N;) (type 0))`) — 괄호가 그 줄에서 닫히면 한 줄 함수다.
            # 이 검사가 없으면 한 줄 함수가 뒤따르는 함수들을 삼킨다(첫 판은 3개를 놓쳤다: 총 함수 수 대조로 발견).
            q = re.sub(r'"(?:[^"\\]|\\.)*"', '""', ln)
            single = q.count("(") == q.count(")")
            while not single and lines[j] != "  )":
                j += 1
                if j >= len(lines):
                    raise SystemExit(f"{path}: 함수 끝을 찾지 못함 (줄 {i})")
            body = lines[i : j + 1]
            nm = re.match(r'  \(func (\$"(?:[^"\\]|\\.)*"|\$\S+)', ln)
            name = nm.group(1) if nm else None
            if name is not None and not re.match(r"\$func\d+$", name):
                named += 1
            idx = re.search(r"\(;(\d+);\)", ln)
            key = name if name and not re.match(r"\$func\d+$", name) else f"#{idx.group(1) if idx else len(order)}"
            # crate 식별 해시 `[85ab15f5d95f7a68]`는 Cargo.toml 의존성이 바뀌면(-C metadata) 함수 내용과 무관하게 바뀐다 — 이름에서 뺀다.
            key = re.sub(r"\[[0-9a-f]{16}\]", "", key)
            base = key
            n = 2
            while key in funcs:
                key = f"{base}~{n}"
                n += 1
            funcs[key] = body
            order.append(key)
            i = j + 1
        else:
            i += 1
    return funcs, order, types, (lo, hi), named, table


def normalize(body, types, rng):
    lo, hi = rng
    masked = 0
    out = []
    for ln in body:
        ln = re.sub(r"\(;\d+;\)", "", ln)
        ln = re.sub(r"\(type (\d+)\)", lambda m: "(sig " + types.get(int(m.group(1)), "?") + ")", ln)
        if lo is not None:
            def mask32(m):
                nonlocal masked
                v = int(m.group(1))
                if lo <= v < hi:
                    masked += 1
                    return "i32.const <DATA>"
                return m.group(0)

            def mask_off(m):
                nonlocal masked
                v = int(m.group(2))
                if lo <= v < hi:
                    masked += 1
                    return m.group(1) + "offset=<DATA>"
                return m.group(0)

            def mask64(m):
                # (길이 << 32) | 주소 로 묶인 &str 상수, 또는 반대 — 어느 반쪽이든 데이터 범위 안이면 그 반쪽을 가린다.
                nonlocal masked
                v = int(m.group(1))
                low, high = v & 0xFFFFFFFF, v >> 32
                if lo <= low < hi:
                    masked += 1
                    return f"i64.const <hi={high},DATA>"
                if lo <= high < hi:
                    masked += 1
                    return f"i64.const <DATA,lo={low}>"
                return m.group(0)

            ln = re.sub(r"i32\.const (\d+)\b", mask32, ln)
            ln = re.sub(r"(\b(?:i32|i64|f32|f64)\.(?:load|store)\w* )offset=(\d+)", mask_off, ln)
            ln = re.sub(r"i64\.const (\d+)\b", mask64, ln)
        out.append(re.sub(r"[ ]+", " ", ln).rstrip())
    return out, masked


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("--out", required=True)
    ap.add_argument("--require-changed", action="append", default=[])
    ap.add_argument("--allow")
    ap.add_argument("--max-diff-lines", type=int, default=60)
    args = ap.parse_args()

    def defined_funcs(path):
        out = subprocess.run(["wasm-tools", "objdump", path], check=True, capture_output=True, text=True).stdout
        m = re.search(r"^\s*code\s*\|.*\|\s*(\d+) count", out, re.M)
        return int(m.group(1))

    fa, oa, ta, ra, na, tba = parse(args.a)
    fb, ob, tb, rb, nb, tbb = parse(args.b)
    for label, path, f in (("A", args.a, fa), ("B", args.b, fb)):
        if defined_funcs(path) != len(f):
            raise SystemExit(f"{label}: 파서가 읽은 함수 수({len(f)})가 wasm 코드 구역의 함수 수({defined_funcs(path)})와 다르다 — 파서가 함수를 놓쳤다")
    for label, n, f in (("A", na, fa), ("B", nb, fb)):
        if len(f) == 0 or n < len(f) * 0.9:
            raise SystemExit(f"{label}: 이름 구역이 없거나 부족하다({n}/{len(f)}) — 진단 빌드(--profiling)가 아니다. 이름 없이는 함수 신원을 못 잡는다.")

    norm_a, norm_b, masked_a, masked_b = {}, {}, 0, 0
    for k, body in fa.items():
        norm_a[k], m = normalize(body, ta, ra)
        masked_a += m
    for k, body in fb.items():
        norm_b[k], m = normalize(body, tb, rb)
        masked_b += m

    ka, kb = set(norm_a), set(norm_b)

    forgiven = 0

    def equal_mod_table(x, y):
        """텍스트가 다른 줄 쌍이 모두 `i32.const X` / `i32.const Y`이고 두 빌드의 함수 테이블에서 같은 함수를 가리키면 같다.
        (함수 포인터는 테이블 인덱스로 표현되고, 함수가 테이블에 추가되면 인덱스가 밀린다 — 코드 변경이 아니다.)
        텍스트가 같으면 이 경로를 타지 않으므로, 평범한 정수 상수가 우연히 테이블 인덱스와 같아도 '같음'을 '다름'으로 바꾸지 않는다."""
        nonlocal forgiven
        if len(x) != len(y):
            return False
        n = 0
        for lx, ly in zip(x, y):
            if lx == ly:
                continue
            mx = re.fullmatch(r"\s*i32\.const (\d+)", lx)
            my = re.fullmatch(r"\s*i32\.const (\d+)", ly)
            if not (mx and my):
                return False
            na_, nb_ = tba.get(int(mx.group(1))), tbb.get(int(my.group(1)))
            if na_ is None or na_ != nb_:
                return False
            n += 1
        forgiven += n
        return True

    same = sorted(k for k in ka & kb if norm_a[k] == norm_b[k] or equal_mod_table(norm_a[k], norm_b[k]))
    changed = sorted(k for k in ka & kb if k not in set(same))
    added = sorted(kb - ka)
    removed = sorted(ka - kb)

    # 존재(개수) 단언
    assert len(same) + len(changed) + len(removed) == len(fa), "A 전체 수와 맞지 않는다"
    assert len(same) + len(changed) + len(added) == len(fb), "B 전체 수와 맞지 않는다"

    report = {
        "a": args.a,
        "b": args.b,
        "counts": {
            "a_total": len(fa),
            "b_total": len(fb),
            "identical": len(same),
            "changed": len(changed),
            "added": len(added),
            "removed": len(removed),
        },
        "data_range_a": ra,
        "data_range_b": rb,
        "masked_data_consts": {"a": masked_a, "b": masked_b},
        "forgiven_table_index_diffs": forgiven,
        "changed": [],
        "added": added,
        "removed": removed,
    }
    def skeleton(body):
        # 숫자 즉치(offset=, const, 정렬 hint 등)를 가린 명령 골격. 골격이 같고 즉치만 다르면 "상수만 변경"(구조체 레이아웃·주소 이동).
        out = []
        for ln in body:
            ln = re.sub(r"(?<![\w$])-?\d+(?![\w$])", "#", ln)
            ln = re.sub(r"=#", "=#", ln)
            out.append(ln)
        return out

    for k in changed:
        d = list(difflib.unified_diff(norm_a[k], norm_b[k], "A", "B", n=1, lineterm=""))
        report["changed"].append(
            {
                "name": k,
                "a_lines": len(norm_a[k]),
                "b_lines": len(norm_b[k]),
                "diff_lines": len(d),
                "kind": "constants_only" if skeleton(norm_a[k]) == skeleton(norm_b[k]) else "structural",
                "sha_a": hashlib.sha256("\n".join(norm_a[k]).encode()).hexdigest()[:16],
                "sha_b": hashlib.sha256("\n".join(norm_b[k]).encode()).hexdigest()[:16],
                "diff_head": d[: args.max_diff_lines],
            }
        )

    ok = True
    for pat in args.require_changed:
        hit = [k for k in changed if re.search(pat, k)]
        report.setdefault("required_changed", {})[pat] = hit
        if not hit:
            print(f"존재 단언 실패: 변경 목록에 `{pat}`가 없다 — 도구가 그 함수 변경을 보지 못하고 있다", file=sys.stderr)
            ok = False

    unallowed = []
    if args.allow:
        rules = []
        for ln in open(args.allow, encoding="utf-8"):
            ln = ln.rstrip("\n")
            if not ln or ln.startswith("#"):
                continue
            pat, _, why = ln.partition("\t")
            rules.append((re.compile(pat), why))
        for k in changed:
            hit = [why for r, why in rules if r.search(k)]
            for c in report["changed"]:
                if c["name"] == k:
                    c["allowed_by"] = hit[0] if hit else None
            if not hit:
                unallowed.append(k)
        report["unallowed_changed"] = unallowed
        if unallowed:
            ok = False

    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=1)
    c = report["counts"]
    report["counts"]["changed_constants_only"] = sum(1 for x in report["changed"] if x["kind"] == "constants_only")
    report["counts"]["changed_structural"] = sum(1 for x in report["changed"] if x["kind"] == "structural")
    with open(args.out, "w", encoding="utf-8") as f:
        json.dump(report, f, ensure_ascii=False, indent=1)
    print(
        f"A={c['a_total']} B={c['b_total']} 동일={c['identical']} 변경={c['changed']} 추가={c['added']} 삭제={c['removed']} "
        f"(마스킹 A={masked_a} B={masked_b}, 테이블 인덱스 이동 용서 {forgiven}곳)"
    )
    if unallowed:
        print(f"허용 목록 밖의 변경 {len(unallowed)}건:", file=sys.stderr)
        for k in unallowed:
            print("  ", k, file=sys.stderr)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
