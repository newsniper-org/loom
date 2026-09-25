"""RQ1-benchmark-protocol.md §9의 **트랙 L 조항 전부**를 한 번에 기계적으로 적용한다.

왜 이 스크립트가 따로 필요한가 (2026-08-02):
지금까지 §9는 조항별로 흩어진 스크립트로 적용돼 왔고(`rq1-paired-bootstrap.py`
= C 채택 (a), `rq1-c-rejection-gate.py` = C 기각 앞 분기,
`rq1-row-depth-bootstrap.py` = row 깊이 축), **어느 스크립트도 계산하지 않는
조항이 남아 있었다** — C 채택 (b), D 기각 (iii)의 전 셀 조건, D 채택 양성 조건,
E 기각 (i)의 M2 결정 규칙, E 기각 (ii). 조항이 있는데 계산되지 않으면
"판정 안 됨"이 아니라 **"판정된 줄 알고 넘어감"**이 되기 쉽다.

여기서는 §9의 트랙 L 조항을 하나도 빠뜨리지 않고 순서대로 돌리고, 계산에
필요한 데이터가 없으면 **"미계측"으로 명시 출력**한다(조용히 건너뛰지 않는다).

트랙 T 조항(B/F)은 데이터 소스가 달라 `rq1-track-t-bf-gate.py`가 맡는다.

방법론은 기존 스크립트와 동일한 paired bootstrap(같은 rep 인덱스로 리샘플 —
§8이 "시드 열 전 후보 공유"를 규정하므로 유효). 새 방법론을 도입하지 않는
이유는 §11-3에 걸리지 않기 위함이다.

실행: `python3 research-questions/analysis/rq1-verdicts.py`
"""

import json
import os
import random
import statistics
from collections import defaultdict

N_RESAMPLES = 10_000
CI = 0.99
random.seed(20260802)

# `RQ1_DATA_DIR`로 덮어쓸 수 있다 — **합성 픽스처로 이 스크립트 자신을 검증**하기
# 위해서다(개정 #9로 로더·변형 선택·신설 프로파일이 전부 새 코드가 됐고, 실제
# 데이터가 오기 전에 그것들이 도는지 확인해야 한다). 판정 실행은 언제나 기본
# 경로를 쓰고, 출력 머리에 어느 디렉터리를 읽었는지 찍는다.
DATA = os.environ.get("RQ1_DATA_DIR",
                      "/home/ybi/loom-research/research-questions/data")
# §11-2 **개정 #9(2026-08-12)로 트랙 L 전 수집분이 무효**다 — v1~v18(양 브라우저)과
# v19-r3 전부. 개정 #8 때처럼 조합 하나만 오버레이하는 방식은 이번엔 성립하지
# 않는다: R2를 조항 원문(반응 바인딩 8개 + 중첩 조건부 1개)으로, heap 아이템을
# 128B로 바꾼 것이 **기존 셀의 수치 자체를 바꾸고**, §9 C 판정 하나가 R1 셀
# (채택 a)과 R2 셀(기각)을 한 판정식 안에서 만나게 하기 때문이다.
VERSION = "v21"
# 옛 오버레이 기제는 남기지 않는다 — 전면 재수집이라 섞을 기반이 없다.
# (개정 #8의 R3 오버레이 주석은 git 이력에 남아 있다.)
INVALIDATED = [f"v{i}" for i in range(1, 19)] + ["v19-r3", "v20-chromium-headed"]

# §3.1 워크로드 이름 ↔ 하네스 이름.
W = {"W1": "swap", "W2": "shuffle", "W3": "shuffle10", "W4": "head_insert",
     "W5": "append", "W6": "stress", "W7": "value_update", "W8": "head_insert",
     "W9": "clear", "W10a": "replace50", "W10b": "replace100",
     "W11": "remove_middle"}
JUDGE_WORKLOADS = ["W1", "W2", "W3", "W4"]  # §9가 판정 셀로 지목하는 넷


# ---------------------------------------------------------------------------
# 통계 — 기존 스크립트와 동일
# ---------------------------------------------------------------------------

def pct(xs, p):
    k = (len(xs) - 1) * p
    f = int(k)
    c = min(f + 1, len(xs) - 1)
    return xs[f] if f == c else xs[f] * (c - k) + xs[c] * (k - f)


def paired_ratio_ci(a, b, ci=CI):
    """a/b 중앙값 비율의 CI. a가 후보, b가 기준선."""
    n = min(len(a), len(b))
    if n == 0:
        return (float("nan"), float("nan"))
    ratios = []
    for _ in range(N_RESAMPLES):
        idx = [random.randrange(n) for _ in range(n)]
        am = statistics.median(a[i] for i in idx)
        bm = statistics.median(b[i] for i in idx)
        if bm > 0:
            ratios.append(am / bm)
    if not ratios:
        return (float("nan"), float("nan"))
    ratios.sort()
    lo_p = (1 - ci) / 2
    return pct(ratios, lo_p), pct(ratios, 1 - lo_p)


# §8 판정 어휘의 등가 대역 — **사전 등록 수치**.
EQ_LO, EQ_HI = 1 / 1.1, 1.1


# 우세/열세로 판정됐지만 등가 대역 안에도 든 (판정, lo, hi) 전부.
VOCAB_OVERLAP = []
# `verdict()`를 부르는 쪽이 자기 맥락을 넣어 둔다(셀·브라우저·조항). 안 넣으면
# 겹침 기록이 'CI 숫자'만 남아 어느 판정이 갈렸는지 알 수 없다.
#
# **`browser`와 `clause`는 반드시 바깥에서 설정돼야 한다** — 2026-09-24 첫 수리는
# `where`에 `[{browser}]`를 찍으면서 그 키를 **아무도 넣지 않았다**. 그래서 전
# 기록이 `[?]`였다. 맥락을 적는다고 선언만 하고 안 적는 것은 계측이 자기가 재려던
# 것을 안 재는 것과 같은 형태다. `set_ctx()`를 통해서만 만지고, 아래
# `report_vocab_overlap()`이 미설정 상태를 발견하면 경고한다.
VOCAB_CTX = {"where": "(미지정)", "browser": None, "clause": None}
# 판정 비교에서 배치 모드가 갈린 짝 — §9 서두의 변형 선택이 데려올 수 있다.
BATCH_MIX = []


def set_ctx(**kw):
    VOCAB_CTX.update(kw)


def ctx_where(cell_desc):
    """`verdict()`가 겹침을 기록할 때 붙일 한 줄 — 브라우저 · 조항 · 셀."""
    b = VOCAB_CTX["browser"] or "브라우저?"
    c = VOCAB_CTX["clause"] or "조항?"
    return f"[{b}] {c} — {cell_desc}"


def verdict(lo, hi):
    """§8 "판정 어휘(로그 척도 비율 통일)"를 문언 그대로.

      우세 = CI 전체 < 1 / 열세 = CI 전체 > 1
      등가 = CI ⊂ [1/1.1, 1.1]  ← **사전 등록 수치**
      어느 것도 아니면 판정 비결정 — "발동 조건 불성립 처리(어느 쪽으로도 불계상)"

    2026-08-04 정정: 이전 판은 등가 분기가 아예 없어 **비결정을 등가로 대신
    읽었다**. 주석에 "§8 판정 어휘에 '등가'가 별도로 없으므로"라고 적었는데
    사실이 아니었다 — §8은 등가를 CI ⊂ [1/1.1, 1.1]로 명시하고 §9 C 채택 (a)도
    "동등 이상(**우세 또는 등가**)"로 괄호까지 달아 두었다. 이 치환은 채택
    조항에서는 후보에 유리하게, 기각 조항에서는 불리하게 작동해 방향이
    후보별로 갈렸고, 사전 등록 수치를 우회한 사후 재해석이라 §11-1(d)에도
    걸린다.
    """
    # **§8 어휘가 겹치는 구간**(2026-09-24 등재): CI 전체가 1의 한쪽에 있으면서 동시에
    # [1/1.1, 1.1] 안에 들 수 있다(예: [0.95, 0.99]). §8은 셋을 나열만 하고 우선순위를
    # 안 적었다. 여기서는 **우세/열세 우선**으로 읽는다 — "CI 전체 < 1"은 방향이
    # 확정됐다는 진술이고 등가는 크기가 작다는 진술이라, 둘 다 참이면 방향 쪽이 더
    # 강한 정보다(프로토콜 §11 "개정 #9 후속 — 어휘 우선순위" 참고, 수집 전 등재).
    # 겹친 경우는 **전부 기록**해 보고서 말미에 찍는다 — E 기각 (i)처럼 "우세가
    # 아니면 기각"인 조항에서는 이 선택이 판정을 가른다.
    if hi < 1 or lo > 1:
        v = "우세" if hi < 1 else "열세"
        if lo >= EQ_LO and hi <= EQ_HI:
            VOCAB_OVERLAP.append((v, lo, hi, VOCAB_CTX["where"]))
        return v
    if lo >= EQ_LO and hi <= EQ_HI:
        return "등가"
    return "비결정"


def unresolved(v):
    """결론 불가 표식인가 — 판정 문자열의 §8 미계측/상한초과, 그리고 `None`.

    **`None`을 여기 포함하는 이유 (2026-09-24)**: 이 두 함수는 원래 `compare()`가
    돌려주는 **판정 문자열**만 받았는데, 게이트의 `▶` 줄에서 앞서 접힌
    tri-state 불리언(`True`/`False`/`None`)을 다시 접는 데도 쓰기 시작했다.
    `None in UNRESOLVED`는 거짓이므로 그대로 두면 **판정 불가가 결정적
    "미발동"·"불충족"으로 접힌다** — 즉 `say()`로 애써 구분해 둔 것을 한 단계
    위에서 스스로 지운다. 합성 픽스처 `fx-missing_cell`에서 채택 (a)가
    "판정 불가(데이터 없음)"인데 `▶ C 판정`은 "채택 조건 불충족 → 채택 불가"로
    찍힌 것이 그 증거였다(수리 전 실측).
    """
    return v is None or v in UNRESOLVED


def resolve_all(verdicts, pred):
    """**전 셀** 조건. 측정된 셀 하나라도 실패하면 거짓(결정적 — 없는 데이터가
    되살릴 수 없다). 실패가 없는데 결론 불가 셀이 있으면 `None`(판정 불가)."""
    if any(not unresolved(v) and not pred(v) for v in verdicts):
        return False
    if any(unresolved(v) for v in verdicts):
        return None
    return True


def resolve_any(verdicts, pred):
    """**최소 1셀** 조건. 측정된 셀 하나라도 맞으면 참(결정적). 맞는 셀이 없는데
    결론 불가 셀이 있으면 `None`(판정 불가)."""
    if any(not unresolved(v) and pred(v) for v in verdicts):
        return True
    if any(unresolved(v) for v in verdicts):
        return None
    return False


def say(flag, yes, no):
    """`None`(판정 불가)을 충족/불충족 어느 쪽으로도 접지 않고 그대로 보여 준다."""
    return "**판정 불가(데이터 없음)**" if flag is None else (yes if flag else no)


def at_least_equal(v):
    """§9의 "동등 이상" = 우세 또는 등가. 비결정은 **불계상**이므로 불충족."""
    return v in ("우세", "등가")


# ---------------------------------------------------------------------------
# 데이터 적재
# ---------------------------------------------------------------------------

def _read(path, base_tag):
    payload = json.load(open(path))
    # v16부터 stock 브라우저 수집 경로가 `{track_l, track_t}`로 감싸 보낸다
    # (§11 개정 #4 — Playwright 번들 Firefox가 대표 빌드가 아니어서 신설한 경로).
    rows = payload["track_l"] if isinstance(payload, dict) else payload
    for r in rows:
        r["_base"] = base_tag
    return rows


def check_labels(rows, browser):
    """**이 스크립트가 찾는 문자열이 데이터에 실제로 있는가.**

    판정 코드는 `item_type`/`candidate`/`workload`/`write_variant`를 **문자열로**
    조회한다. 하네스가 그 라벨을 바꾸면 조회가 `None`을 돌려주고, 해당 절은
    **오류가 아니라 빈 절**이 된다 — 읽는 사람에게는 "잴 게 없었다"로 보인다.
    2026-09-22에 실제로 그랬다(heap 아이템 라벨이 128B로 바뀐 것을 이 스크립트가
    못 따라가 W7 heap 프로파일이 통째로 사라져 있었다).

    여기서는 **찾는 라벨 중 데이터에 하나도 없는 것**을 전부 찍는다. 데이터에
    그 셀이 원래 없을 수도 있으므로 중단하지 않고 경고만 하되, 조용하지는 않게 한다.
    """
    present = {
        "item_type": {r.get("item_type") for r in rows},
        "candidate": {r.get("candidate") for r in rows},
        "workload": {r.get("workload") for r in rows},
        "write_variant": {r.get("write_variant", "Conditional") for r in rows},
    }
    expected = {
        "item_type": {POD, STR},
        "candidate": {C, D, D_NOLIS, E, K, "E-LIS"},
        "workload": set(W.values()),
        "write_variant": {"Conditional", "Unconditional"},
    }
    missing = {
        field: sorted(v for v in want if v not in present[field])
        for field, want in expected.items()
    }
    missing = {f: v for f, v in missing.items() if v}
    if missing:
        print(f"  [{browser}] ⚠️ 이 스크립트가 찾는 라벨 중 데이터에 없는 것:")
        for field, vals in missing.items():
            found = sorted(x for x in present[field] if x)
            print(f"      {field}: 찾는데 없음 {vals}  /  데이터에 있는 것 {found}")
        print("      → 라벨이 바뀌었으면 상수를 고칠 것. 안 고치면 해당 절이 "
              "오류가 아니라 **빈 절**이 되어 '잴 게 없었다'로 읽힌다.")


def load(browser):
    path = f"{DATA}/rq1-phase1-track-l-core-grid-{VERSION}-{browser}.json"
    if not os.path.exists(path):
        print(f"  [{browser}] {VERSION} 수집분이 아직 없다 — 판정 불가")
        return [], {}
    rows = _read(path, VERSION)

    # 개정 #9로 무효화된 수집분이 디렉터리에 남아 있는 것은 §11-5(append-only)
    # 때문이지 쓸 수 있다는 뜻이 아니다. **읽지 않는다**는 사실을 매 실행 찍는다 —
    # 조용하면 다음 사람이 "왜 옛 데이터를 안 쓰지"를 물을 데가 없다.
    stale = [v for v in INVALIDATED
             if os.path.exists(f"{DATA}/rq1-phase1-track-l-core-grid-{v}-{browser}.json")]
    if stale:
        print(f"  [{browser}] 개정 #9로 무효인 수집분 {len(stale)}건은 읽지 않는다: "
              f"{', '.join(stale[:4])}{' …' if len(stale) > 4 else ''}")

    check_labels(rows, browser)

    idx = {}
    for r in rows:
        # W6은 인덱스에 안 넣는다: 후보당 **독립 5시행**이라 같은 키가 다섯 번
        # 나오고(§8 "반복 개념 없음"), `samples_ms`가 없어 paired 비교의 대상도
        # 아니다. `profile_new_cells`가 `rows`에서 직접 읽는다.
        if r.get("clause") == "W6" or r.get("workload") == "stress":
            continue
        # **write 변형이 키에 들어간다**(개정 #9-6). 안 넣으면 같은 셀의 두 변형이
        # 서로를 덮어써서 어느 쪽이 남았는지가 실행마다 달라진다.
        key = (r["item_type"], r["n"], r["row_bindings"], r.get("row_inner", 0),
               r["row_variants"], r["workload"], r["candidate"],
               r.get("write_variant", "Conditional"))
        if key in idx:
            raise SystemExit(f"셀 키 충돌 — 같은 셀이 두 번 들어왔다: {key}")
        idx[key] = r
    return rows, idx


# ---------------------------------------------------------------------------
# 후보 이름 매핑 — §10의 사전 등록 고정
# ---------------------------------------------------------------------------
#
# §10 "D: 이동 최소화 (LIS vs 순열 매칭만)" 행은 **LIS 고정**이다:
#   "근거: (1) G0 통과가 'LIS 포함 keyed reconcile' 명명에 대해 내려짐
#    (docs/03 판정례 1, N5 구현 특정성 — **무LIS 변형은 재심사 대상**),
#    (2) O(이동 수) 주장은 LIS 없이 불성립(RQ1 확정)."
#
# 그런데 하네스의 후보 이름 "D"는 `place_in_order`(무LIS)이고 "D-LIS"가
# `place_in_order_lis`다. 즉 **사전 등록된 D는 데이터의 D-LIS**다.
# 2026-08-04까지 이 스크립트는 §9 D 조항 전부와 C 채택 (a)의 기준선을 무LIS
# 쪽으로 읽고 있었다 — §10이 재심사 대상으로 못 박은 변형에 대해 판정을
# 내리고 있었던 셈이다.
#
# E는 §10에 같은 고정이 없으므로 하네스의 "E"(무LIS)를 그대로 쓰고, "E-LIS"는
# 변형 프로파일로 병기한다.
D = "D-LIS"          # §10 D-4: LIS 고정
D_NOLIS = "D"        # 무LIS 변형 — §11-1(c)로 W2×R1 1셀 한정 추가 측정 지위
E = "E"
K = "K"
C = "C"

# **하네스의 `item_type` 문자열과 글자 단위로 일치해야 한다.** 개정 #9-10으로
# heap 아이템이 128B가 되면서 하네스 라벨이 "String(heap)" → "String(heap 128B)"로
# 바뀌었는데 이 상수가 안 따라가, §3.2가 요구한 **W7의 heap 쪽 절반이 조용히 빈
# 절**이 되고 있었다(2026-09-22 감사에서 발견). 아래 `check_labels`가 그 종류의
# 어긋남을 다시는 조용히 지나가지 못하게 한다.
STR, N = "String(heap 128B)", 1000
# §3.2 "Item 타입 축: ... **기본 POD**, W7과 트랙 R에서 양쪽."
# 즉 §3.3 핵심 격자(= §9 판정 셀 전부)의 기본 아이템 타입은 POD다.
POD = "ItemId(POD u32)"


# §3.3이 write 변형 이중 측정을 요구한 셀 집합 — "{W1,W2,W3,W4} × {R1,R2} 및 W10".
DOUBLED_WORKLOADS = {"W1", "W2", "W3", "W4", "W10a", "W10b"}
# §10이 그 축을 등재한 후보는 C·D뿐이다. K·E의 Unconditional 레코드가 수집돼도
# 판정에 쓰지 않는다(같은 셀을 6후보로 함께 돌린 부산물일 뿐).
DOUBLED_CANDIDATES = {"C", "D", "D-LIS"}

# 어느 셀에서 어느 변형이 골라졌는지 — 보고서 말미에 전부 찍는다.
VARIANT_LOG = []
# 선택은 **셀당 한 번만** 계산한다. 게이트 여러 개가 같은 셀을 조회하는데,
# 매번 다시 고르면 (1) 로그가 중복되고 (2) 부트스트랩이 재실행돼 같은 셀의 CI가
# 실행마다 조금씩 달라진 채 보고서에 여러 줄로 남는다 — 읽는 사람이 "왜 같은
# 셀의 CI가 다르지"를 물을 데가 없다. 선택 자체는 median 비교라 결정적이다.
_VARIANT_CACHE = {}


def _raw(idx, workload, bindings, cand, variants, n, item, inner, wv):
    return idx.get((item, n, bindings, inner, variants, W[workload], cand, wv))


def cell(idx, workload, bindings, cand, variants=False, n=N, item=POD, inner=0):
    """판정용 셀 하나.

    §9 서두 "이중 측정 셀의 판정은 각 후보의 **셀별 우세 write 변형** 기준(양
    변형이 모두 측정되므로 선택은 기계적 — 사후 재량 없음)". 조작화는 개정 #9
    후속 등재(2026-08-12, **수집 전**)대로 **median이 더 작은 변형**이고, 동률이면
    `Conditional`(2026-08-12 이전의 유일한 구현)이다.

    두 변형의 paired CI를 함께 기록한다 — 선택이 등가 구간에서 일어났으면 그
    사실이 보고서에 남아야 한다.
    """
    cond = _raw(idx, workload, bindings, cand, variants, n, item, inner, "Conditional")
    # §3.3 "판정에 걸린 {W1,W2,W3,W4} × {R1,R2} 및 W10" — POD, n=1,000, 중첩 없음,
    # R3 아님. 이 밖의 셀(예: R3 셀의 W2, 스케일링 셀)은 이중 측정 대상이 아니므로
    # 무조건 변형이 없는 것이 **정상**이고 "미이행"으로 찍으면 거짓 감사 줄이 된다
    # (2026-09-22 감사 발견 #19).
    in_doubled_set = (item == POD and n == N and inner == 0 and not variants)
    if not in_doubled_set or workload not in DOUBLED_WORKLOADS or cand not in DOUBLED_CANDIDATES:
        return cond
    ck = (id(idx), workload, bindings, cand, variants, n, item, inner)
    if ck in _VARIANT_CACHE:
        return _VARIANT_CACHE[ck]
    unc = _raw(idx, workload, bindings, cand, variants, n, item, inner, "Unconditional")
    if cond is None or unc is None:
        # 한쪽이 없으면 이중 측정이 이행되지 않은 것이다 — 조용히 다른 쪽을
        # 쓰지 않고, 있는 것을 돌려주되 사실을 남긴다.
        if cond is not None and unc is None:
            VARIANT_LOG.append((workload, bindings, cand, "Conditional",
                                "무조건 변형 셀 없음 — 이중 측정 미이행"))
        _VARIANT_CACHE[ck] = cond if cond is not None else unc
        return _VARIANT_CACHE[ck]
    # **변형 선택 전에 쓸 수 있는 레코드인지 본다.** 2026-09-22 수리는 `compare()`
    # 에만 가드를 뒀는데, 이중 측정 셀은 여기서 먼저 median/부트스트랩을 돌리므로
    # 둘 중 하나가 표본 0개면 그대로 `StatisticsError`로 죽었다(#13 수리의 잔존 경로).
    bad_cond, bad_unc = unusable(cond), unusable(unc)
    if bad_cond or bad_unc:
        usable = None if (bad_cond and bad_unc) else (unc if bad_cond else cond)
        VARIANT_LOG.append((workload, bindings, cand, "선택 불가",
                            f"cond={bad_cond or 'ok'} / uncond={bad_unc or 'ok'} — "
                            f"{'양쪽 다 판정 불가' if usable is None else '쓸 수 있는 쪽으로 폴백'}"))
        _VARIANT_CACHE[ck] = usable if usable is not None else cond
        return _VARIANT_CACHE[ck]
    mc, mu = statistics.median(cond["samples_ms"]), statistics.median(unc["samples_ms"])
    keep = VOCAB_CTX["clause"]
    set_ctx(clause="§9 서두 '셀별 우세 write 변형'")
    VOCAB_CTX["where"] = ctx_where(
        f"{workload}×R{1 if bindings == 1 else 2} {cand} 변형선택(판정 아님)")
    lo, hi = paired_ratio_ci(unc["samples_ms"], cond["samples_ms"])
    set_ctx(clause=keep)
    pick = "Unconditional" if mu < mc else "Conditional"
    # **배치 모드를 함께 남긴다** (2026-09-25). §8 배치 규칙은 셀마다 파일럿으로
    # 정하는데(`pilot_min < 해상도×50`), 무조건 변형은 가장 싼 후보를 임계값 위로
    # 밀어 올리므로 **같은 논리 셀의 두 변형이 서로 다른 모드로 측정될 수 있다**
    # (실측: W1×R1에서 조건부 alternate / 무조건 none). 표본의 차원은 같지만
    # (§8 "k회 묶음 총 시간/k = 1측정치") 정밀도가 10배 다르므로, 선택이 어느
    # 정밀도에서 일어났는지가 기록에 남아야 한다.
    bm = f" [배치 cond={cond.get('batch_mode', '?')} / uncond={unc.get('batch_mode', '?')}]"
    if cond.get("batch_mode") != unc.get("batch_mode"):
        bm += " ← **두 변형의 배치 모드가 다르다**"
    VARIANT_LOG.append((workload, bindings, cand, pick,
                        f"cond={mc:.4f} uncond={mu:.4f}  "
                        f"uncond/cond CI=[{lo:.3f}, {hi:.3f}] {verdict(lo, hi)}{bm}"))
    _VARIANT_CACHE[ck] = unc if pick == "Unconditional" else cond
    return _VARIANT_CACHE[ck]


# 조건 판정에서 **결론으로 쓸 수 없는** 판정값들. §9는 기각/충족 조건의 해당
# 범위를 "등가·열세·비결정"처럼 **열거**하는데 그 목록에 미계측은 없다 — 데이터가
# 없다는 것은 조건이 충족됐다는 뜻도, 안 됐다는 뜻도 아니다.
UNRESOLVED = ("미계측", "상한초과")


def unusable(r):
    """이 레코드로 판정을 낼 수 없는가 — 그 사유(없으면 None).

    **표본이 0개인지가 아니라 `over_cell_cap`인지로 판정한다**(2026-09-24). §8은
    상한 초과를 "'측정 불가(상한 초과)' 프로파일 등재 — 그 자체가 유효한 결과
    데이터"로 규정했다. 즉 그 셀은 **판정 셀이 아니다**. 상한은 측정 루프 안에서도
    발동하므로(loom-bench의 `break 'measure`) 표본이 1~199개인 상한 초과 레코드가
    나올 수 있는데, 2026-09-22 수리는 `samples_ms`가 빈 경우만 걸러서 그런 셀이
    **정상 판정 셀로 들어갔다** — 표본 3개짜리 CI가 §9 조항에 그대로 먹혔다.
    """
    if not r["samples_ms"]:
        return "표본 0개"
    if r.get("over_cell_cap"):
        return f"상한 초과(표본 {len(r['samples_ms'])}/{r.get('reps', '?')})"
    return None


def compare(idx, workload, bindings, cand, base, **kw):
    """(판정, lo, hi, 설명) — 데이터가 없거나 상한 초과 셀이면 결론 불가."""
    a, b = cell(idx, workload, bindings, cand, **kw), cell(idx, workload, bindings, base, **kw)
    if not a or not b:
        return ("미계측", float("nan"), float("nan"), f"{cand} 또는 {base} 셀 없음")
    VOCAB_CTX["where"] = ctx_where(
        f"{workload}×R{1 if bindings == 1 else 2} {cand}/{base}")
    bad = [f"{n}: {why}" for r, n in ((a, cand), (b, base)) if (why := unusable(r))]
    if bad:
        return ("상한초과", float("nan"), float("nan"),
                f"{' / '.join(bad)} — §8상 측정 불가 셀이라 판정에 못 쓴다")
    # **짝짓는 두 레코드의 배치 모드가 갈렸는가** (2026-09-25). 이게 일어나는
    # 경로는 하나다: §9 서두가 이중 측정 셀에서 후보별로 우세 변형을 고르는데,
    # 고른 변형의 셀이 기준선 후보(K는 이중 측정 대상이 아니라 언제나 조건부)의
    # 셀과 다른 배치 모드일 때. §8 정의상 표본의 차원은 같고 rep 인덱스가 같으면
    # target도 같으므로(시드가 변형을 안 포함한다 — 2026-09-24 수리) **짝짓기 자체는
    # 유효하다**. 갈리는 것은 정밀도(배치 쪽 분산이 1/k)이고, 그것은 CI 폭에
    # 들어간다. 그래도 판정문에 병기한다 — 읽는 사람이 두 median을 같은 정밀도로
    # 읽으면 안 되기 때문이다.
    mix = ""
    if a.get("batch_mode") != b.get("batch_mode"):
        mix = (f"  ⚠️배치 모드 혼용({cand}={a.get('batch_mode')} / "
               f"{base}={b.get('batch_mode')})  ")
        BATCH_MIX.append((VOCAB_CTX["where"], cand, a.get("batch_mode"),
                          base, b.get("batch_mode"),
                          a.get("write_variant"), b.get("write_variant")))
    lo, hi = paired_ratio_ci(a["samples_ms"], b["samples_ms"])
    return (verdict(lo, hi), lo, hi,
            f"{mix}{cand}={statistics.median(a['samples_ms']):.4f}ms "
            f"{base}={statistics.median(b['samples_ms']):.4f}ms")


def line(label, v, lo, hi, note=""):
    ci = "  CI=[  nan,   nan]" if lo != lo else f"  CI=[{lo:6.3f}, {hi:6.3f}]"
    print(f"    {label:34s} {v:6s}{ci}   {note}")


# ---------------------------------------------------------------------------
# §9 C
# ---------------------------------------------------------------------------

def gate_c(idx, browser):
    set_ctx(browser=browser, clause="§9 C (절 미지정)")
    print(f"\n{'=' * 78}\n§9 C (고정 슬롯) — {browser}\n{'=' * 78}")

    set_ctx(clause="C 채택 (a)")
    print("  채택 (a): {W1,W2,W3,W4} × R1 전 셀에서 D 대비 동등 이상")
    vs_a = []
    for w in JUDGE_WORKLOADS:
        v, lo, hi, note = compare(idx, w, 1, C, D)
        line(f"{w} × R1  C/D", v, lo, hi, note)
        vs_a.append(v)
    ok_a = resolve_all(vs_a, at_least_equal)
    print(f"    ⇒ (a) {say(ok_a, '충족', '**불충족**')}")

    set_ctx(clause="C 채택 (b)")
    print("  채택 (b): {W1,W2,W3,W4} × R1 중 최소 1셀에서 K 대비 우세")
    vs_b = []
    for w in JUDGE_WORKLOADS:
        v, lo, hi, note = compare(idx, w, 1, C, K)
        line(f"{w} × R1  C/K", v, lo, hi, note)
        vs_b.append(v)
    ok_b = resolve_any(vs_b, lambda v: v == "우세")
    print(f"    ⇒ (b) {say(ok_b, '충족', '**불충족**')}")

    print("  채택 (c): opt-in 문법 수용 — **설계 판단, 실측 아님**(§9 문언)")

    set_ctx(clause="C 기각 앞 분기")
    print("  기각 앞 분기: W2 × R2에서 C/D 비율 CI 하한 ≥ 3")
    v, lo, hi, note = compare(idx, "W2", 8, C, D)
    line("W2 × R2  C/D", v, lo, hi, note)
    # 데이터가 없으면(NaN) **미발동이 아니라 판정 불가**다 — 2026-09-24까지
    # `lo >= 3.0 if lo == lo else False`라 부재가 그대로 "기각 미발동"이 됐다.
    fired_front = None if v in UNRESOLVED else lo >= 3.0
    print(f"    ⇒ 앞 분기 {say(fired_front, '**발동**', '미발동')}"
          + ("" if fired_front is None else f" (하한 {lo:.3f} {'≥' if fired_front else '<'} 3)"))

    print("  기각 뒷 분기: W2 × R3에서 archetype move 구간 ≥ 스크립트 시간의 50%")
    print("    (주의: R3 셀은 C만 유효. D/D-LIS/E/E-LIS/K는 2026-08-07까지 축이")
    print("     적용되지 않아 §11-2 개정 #8로 무효 처리됨 — 이 분기는 C 자신의")
    print("     절대 임계값이므로 영향 없다)")
    r = cell(idx, "W2", 1, C, variants=True)
    if not r or "stage_ms" not in r:
        print("    미계측 — R3 셀 또는 §7.1 단계 귀속 없음")
        fired_back = None
    else:
        st = r["stage_ms"]
        share = st["sync_point"] / st["total"] if st["total"] > 0 else 0.0
        fired_back = share >= 0.5
        print(f"    sync_point={st['sync_point']:.4f}ms / total={st['total']:.4f}ms "
              f"= {share * 100:.1f}%")
        print(f"    ⇒ 뒷 분기 {'**발동**' if fired_back else '미발동'}")

    # 어느 한 분기라도 발동이면 기각(결정적, "또는"), 둘 다 미발동이면 미발동,
    # 그 외(한쪽이 판정 불가)는 판정 불가. `or`로 접으면 `None`이 미발동이 된다.
    rejected = resolve_any([fired_front, fired_back], lambda x: x is True)
    # `None`(판정 불가)을 "불충족"으로 접지 않는다.
    adopt_ab = resolve_all([ok_a, ok_b], lambda x: x is True)
    if rejected is True:
        c_line = "**기각**"
    elif rejected is None:
        c_line = "기각 **판정 불가(데이터 없음)**"
    elif adopt_ab is None:
        # 기각은 결정적으로 미발동인데 채택 조건만 데이터가 없는 경우 — 두 사실을
        # 함께 찍는다. `say()` 하나로 접으면 "기각 미발동"이 사라진다.
        c_line = "기각 미발동 — 채택 조건 **판정 불가(데이터 없음)**"
    else:
        c_line = ("채택 조건 (a)(b) 충족 — (c)는 설계 판단" if adopt_ab
                  else "기각 미발동, 채택 조건 불충족 → 채택 불가")
    print(f"\n  ▶ C 판정: {c_line}")
    return {"adopt_a": ok_a, "adopt_b": ok_b, "reject": rejected}


# ---------------------------------------------------------------------------
# §9 D
# ---------------------------------------------------------------------------

def gate_d(idx, browser):
    set_ctx(browser=browser, clause="§9 D (절 미지정)")
    print(f"\n{'=' * 78}\n§9 D (keyed 시퀀스 동기화) — {browser}\n{'=' * 78}")

    set_ctx(clause="D 기각 (iii) 앞항")
    print("  기각 (iii) 앞항: {W1,W2,W3,W4} × {R1,R2} **전 셀**에서 K가 D 대비 동등 이상")
    vs_front = []
    for bindings in (1, 8):
        for w in JUDGE_WORKLOADS:
            v, lo, hi, note = compare(idx, w, bindings, K, D)
            line(f"{w} × R{1 if bindings == 1 else 2}  K/D", v, lo, hi, note)
            vs_front.append(v)
    k_all = resolve_all(vs_front, at_least_equal)
    print(f"    ⇒ 앞항 {say(k_all, '성립', '**불성립**')}")

    print("  기각 (iii) 뒷항: 부속 비용 프로파일(M4/M7/M3 중 1개 이상)에서 D가 K 대비 순증")
    print("    미계측 — M4(이중 저장)·M7(resume walk)·M3는 이 하네스가 수집하지 않는다")
    print("    (§9 문언상 **두 항 모두** 성립해야 기각이므로, 뒷항 미계측이면 기각 불성립)")

    set_ctx(clause="D 채택 양성 조건")
    print("  채택 양성 조건: {W1,W2,W3} × R2 중 최소 1셀에서 K 대비 우세")
    vs_pos = []
    for w in ["W1", "W2", "W3"]:
        v, lo, hi, note = compare(idx, w, 8, D, K)
        vs_pos.append(v)
        line(f"{w} × R2  D/K", v, lo, hi, note)
    ok_pos = resolve_any(vs_pos, lambda v: v == "우세")
    print(f"    ⇒ 양성 조건 {say(ok_pos, '충족', '**불충족** → 후퇴 경로 판정')}")

    # §9 D "채택 추가 선결": "§5 FV 계류(D·E 공통), **W5에서 K/C 대비 열세
    # 확인 시 unkeyed 정적 opt-out 경로 명세를 채택 조건에 부속**".
    # 데이터가 있는데 계산되지 않고 있었다(2026-08-04 감사) — 이걸 빼면 D가
    # 조건 없이 채택 경로에 있는 것처럼 읽힌다.
    set_ctx(clause="D W5 부속")
    print("  채택 추가 선결: W5(append)에서 K/C 대비 열세면 unkeyed opt-out 명세 부속")
    w5_worse, w5_vs = [], []
    for base in (K, C):
        v, lo, hi, note = compare(idx, "W5", 1, D, base)
        line(f"W5 × R1  D/{base}", v, lo, hi, note)
        w5_vs.append(v)
        if v == "열세":
            w5_worse.append(base)
    # "K/C 대비 열세 확인 시" — 열세가 하나라도 확인되면 결정적(발동), 열세가 없고
    # 결론 불가 셀이 있으면 판정 불가.
    w5_rider = resolve_any(w5_vs, lambda v: v == "열세")
    print(f"    ⇒ {say(w5_rider, '**부속 조건 발동**' + (' (' + ', '.join(w5_worse) + ' 대비 열세)' if w5_worse else ''), '미발동')}")
    print("    ※ §5 FV 계류(D·E 공통)는 이 스크립트 범위 밖 — 채택은 그것까지 통과해야 한다.")

    front_txt = say(k_all, "앞항 성립 — 그러나 뒷항 미계측이라 기각 불성립", "불성립(앞항 불성립)")
    print(f"\n  ▶ D 판정: 기각 (iii) {front_txt}"
          f" / 양성 조건 {say(ok_pos, '충족', '불충족')}"
          f" / W5 부속 {say(w5_rider, '발동', '미발동')}")
    return {"reject_iii_front": k_all, "adopt_positive": ok_pos, "w5_rider": w5_rider}


# ---------------------------------------------------------------------------
# §9 E
# ---------------------------------------------------------------------------

def gate_e(idx, browser):
    set_ctx(browser=browser, clause="§9 E (절 미지정)")
    print(f"\n{'=' * 78}\n§9 E (OrderKey) — {browser}\n{'=' * 78}")
    print("  ※ 기각 (i)/(ii)는 **네이티브 경로**에서만 판정(§9 E-1).")
    r = cell(idx, "W1", 8, E)  # §9 E-1 확인은 판정 셀에서
    if r and "reposition_native_cell_total" in r:
        nat, fb = r["reposition_native_cell_total"], r["reposition_fallback_cell_total"]
        print(f"    reposition 경로: 네이티브 {nat} / 폴백 {fb}"
              f" → {'네이티브 경로 확인' if fb == 0 else '**폴백 혼입 — 판정 데이터 부적격**'}")

    set_ctx(clause="E 기각 (i)")
    print("  기각 (i): 판정 셀 W1·W3 × R2 중 **어느 셀에서도** K 대비 우세를 못 보이면 기각")
    vs_i = []
    for w in ["W1", "W3"]:
        v, lo, hi, note = compare(idx, w, 8, E, K)
        line(f"{w} × R2  E/K", v, lo, hi, note)
        vs_i.append(v)
    # **데이터 부재로 기각시키지 않는다**: §9 E 기각 (i)은 해당 범위를
    # "등가·열세·비결정"으로 **열거**했고 그 목록에 미계측은 없다. 2026-09-22까지
    # `fired_i = not any_win`이라 셀이 비면 그대로 E가 기각됐다.
    any_win = resolve_any(vs_i, lambda v: v == "우세")
    fired_i = None if any_win is None else not any_win
    print(f"    ⇒ 기각 (i) {say(fired_i, '**발동**', '미발동')}")

    print("  기각 (i) 병기(결정적): E의 셀당 총 DOM op 수 ≥ K의 50%이면 'DOM 연산 절감 없음'")
    for w in ["W1", "W3"]:
        e, k = cell(idx, w, 8, E), cell(idx, w, 8, K)
        if not e or not k:
            print(f"    {w} × R2: 미계측")
            continue
        eo = e["m2_dom_ops"]["total_mutations"]
        ko = k["m2_dom_ops"]["total_mutations"]
        ratio = eo / ko if ko else float("nan")
        print(f"    {w} × R2  E={eo} K={ko}  비율={ratio:.3f} "
              f"→ {'**절감 없음**' if ratio >= 0.5 else '절감 있음'}")

    # §9 E는 판정 셀을 "W1·W3(× R2, n=1,000)"으로 한 번 지정하고 기각 (ii)에
    # 셀을 따로 쓰지 않는다 — 같은 후보 항목 안의 그 지정이 (ii)에도 적용된다고
    # 읽는 것이 문언에 가장 가깝다. 2026-08-04까지 이 스크립트는 §9 어디에서도
    # E의 판정 셀로 지정된 적 없는 W2×R2 하나만 썼다(그건 C 기각 앞 분기의
    # 셀이다) — 결과 확인 후 셀을 고르는 자유도가 남아 있었고, §11-3이 막으려는
    # 형태다.
    print("  기각 (ii): reconcile System 시간 ≥ (K의 DOM 적용 − E의 DOM 적용)  [판정 셀 W1·W3 × R2]")
    ii_hits = []
    for w in ["W1", "W3"]:
        e, k = cell(idx, w, 8, E), cell(idx, w, 8, K)
        if not e or not k or "stage_ms" not in e:
            print(f"    {w} × R2: 미계측")
            ii_hits.append("미계측")
            continue
        if unusable(e) or unusable(k):
            print(f"    {w} × R2: 상한 초과 셀 — 판정 불가")
            ii_hits.append("상한초과")
            continue
        recon = e["stage_ms"]["systems"]
        saving = k["stage_ms"]["dom_apply"] - e["stage_ms"]["dom_apply"]
        hit = recon >= saving
        ii_hits.append("발동" if hit else "미발동")
        print(f"    {w} × R2  E systems={recon:.4f}ms  절감분=K({k['stage_ms']['dom_apply']:.4f}) "
              f"− E({e['stage_ms']['dom_apply']:.4f}) = {saving:.4f}ms → {'**발동**' if hit else '미발동'}")
    fired_ii = resolve_any(ii_hits, lambda v: v == "발동")
    print(f"    ⇒ 기각 (ii) {say(fired_ii, '**발동**', '미발동')}")

    # ▶ 줄도 `None`을 접지 않는다 — 어느 한쪽이 발동이면 기각(결정적), 둘 다
    # 미발동이면 미발동, 그 외(한쪽이 판정 불가)는 판정 불가.
    e_reject = resolve_any([fired_i, fired_ii], lambda x: x is True)
    print(f"\n  ▶ E 판정: {say(e_reject, '**기각**', '기각 미발동')}")
    return {"reject_i": fired_i, "reject_ii": fired_ii}


# ---------------------------------------------------------------------------
# W7 — 판정 셀은 아니지만 §3.4가 예고한 프로파일
# ---------------------------------------------------------------------------

def profile_new_cells(rows, idx, browser):
    """개정 #9로 신설된 셀들 — **전부 프로파일 전용이다**.

    §9의 어느 조항도 W6·W8·W9·W10·W11을 판정에 쓰지 않는다. 그래도 찍는 이유는
    §3.4가 이 셀들을 "각 후보의 등록된 최선/최악 구간"으로 지목했기 때문이다 —
    원칙 2(어느 후보든 기록된 최선/최악 구간이 행렬에 있어야 한다)의 이행 증거다.
    """
    set_ctx(browser=browser, clause="개정 #9 신설 셀 프로파일(판정 아님)")
    print(f"\n{'-' * 78}\n개정 #9 신설 셀 프로파일 (판정 비참여)\n{'-' * 78}")

    # W6 — §7 M1의 보고 규칙: amortized와 worst-case를 **같은 표에** 병기.
    w6 = [r for r in rows if r.get("clause") == "W6"]
    if w6:
        print("  W6 스트레스 (n=0 → head-insert 10^4, 독립 5시행)")
        print(f"    {'후보':8s} {'시행':>4s} {'삽입':>12s} {'총(ms)':>12s} "
              f"{'삽입당 median':>14s} {'단발 최대 flush':>16s}  상한")
        for r in sorted(w6, key=lambda x: (x["candidate"], x.get("trial", 0))):
            capped = "초과" if r.get("over_cell_cap") else ""
            print(f"    {r['candidate']:8s} {r.get('trial', 0):4d} "
                  f"{r.get('inserts_completed', 0):>6d}/{r.get('inserts_target', 0):<5d} "
                  f"{r.get('amortized_total_ms', float('nan')):12.1f} "
                  f"{r.get('per_insert_median_ms', float('nan')):14.5f} "
                  f"{r.get('max_single_flush_ms', float('nan')):16.5f}  {capped}")
        # §3.3 "E는 재라벨 이벤트 전수 기록" — 개정 #9 후속 등재대로 **해당 없음**이다.
        if all(r.get("relabel_events") is None for r in w6):
            print("    * relabel_events = null(해당 없음). Phase 1의 E는 분수 인덱싱이")
            print("      아니라 조밀 재태그라 '재라벨 이벤트'가 구현에 존재하지 않는다.")
            print("      0이 아니라 null인 이유는 0이면 '한 번도 안 일어났다'로 읽혀")
            print("      메커니즘 부재를 성질로 오독하는 공허한 확인이 되기 때문이다.")
        # 상한 초과는 §8상 **유효한 결과 데이터**다 — 숨기지 않고 세어 둔다.
        capped = [r for r in w6 if r.get("over_cell_cap")]
        if capped:
            names = sorted({r["candidate"] for r in capped})
            print(f"    * 셀 상한(30분) 초과: {len(capped)}셀 / 후보 {', '.join(names)}")
            print("      §8: '측정 불가(상한 초과)' 프로파일 등재 — 그 자체가 유효한 결과다.")

    # W8 — C의 O(n_out×n_in) 증폭 표적. 중첩 축이므로 inner=100으로 조회한다.
    print("  W8 중첩 증폭 (외부 100 × 내부 100, 외부 head-insert)")
    base = cell(idx, "W8", 1, K, n=100, inner=100)
    if not base:
        print("    미계측")
    else:
        for cand in (C, D, D_NOLIS, E, "E-LIS"):
            v, lo, hi, note = compare(idx, "W8", 1, cand, K, n=100, inner=100)
            line(f"{cand} vs K", v, lo, hi, note)

    # W9 — §3.4가 K의 **등록된 유일한 유리 구간**으로 잡은 셀.
    print("  W9 clear (전량 삭제 — K·C의 fast path 표적)")
    if not cell(idx, "W9", 1, K):
        print("    미계측")
    else:
        for cand in (C, D, E):
            v, lo, hi, note = compare(idx, "W9", 1, cand, K)
            line(f"{cand} vs K", v, lo, hi, note)

    # W10 — §9가 "판정 셀이 아님, 채택 기준 완화 방향 사용 금지"로 못박은 셀.
    print("  W10 아이템 교체 (§9: 판정 셀 아님 — 지도·프로파일 전용)")
    for wl, rate in (("W10a", "50%"), ("W10b", "100%")):
        for rb in (1, 8):
            if not cell(idx, wl, rb, K):
                continue
            for cand in (C, D):
                v, lo, hi, note = compare(idx, wl, rb, cand, K)
                line(f"{cand} vs K  R{rb} 교체율 {rate}", v, lo, hi, note)

    # W11 — 개정 #9-8로 등재된 프로파일 전용 셀.
    print("  W11 중간 삭제 (개정 #9-8 등재 — 프로파일 전용)")
    if not cell(idx, "W11", 1, K):
        print("    미계측")
    else:
        for cand in (C, D, E):
            v, lo, hi, note = compare(idx, "W11", 1, cand, K)
            line(f"{cand} vs K", v, lo, hi, note)


def report_vocab_overlap():
    """§8 어휘 겹침(우세/열세이면서 등가 대역 안)이 실제로 몇 번 있었는가."""
    print(f"\n{'-' * 78}\n§8 판정 어휘 겹침 (우세/열세 ∩ 등가 — 우세/열세 우선으로 읽음)\n{'-' * 78}")
    if not VOCAB_OVERLAP:
        print("  없음 — 이번 데이터에서는 우선순위 선택이 어떤 판정에도 영향을 주지 않았다.")
        return
    # 개정 #9 후속 등재의 정정(2026-09-25): 우선순위가 가르는 조항은 E 기각 (i)
    # 하나가 아니라 넷이다 — C 채택 (b), D 채택 양성 조건, D W5 부속, E 기각 (i).
    # "동등 이상"을 요구하는 C 채택 (a)·D 기각 (iii) 앞항은 어느 우선순위로 읽어도
    # 충족이므로 영향이 없다.
    print(f"  {len(VOCAB_OVERLAP)}건. 이 셀들은 등가로 읽었다면 판정이 달라질 수 있었다.")
    print("  어휘 배타를 요구해 실제로 갈리는 조항: C 채택 (b) · D 채택 양성 조건 · "
          "D W5 부속 · E 기각 (i).")
    print("  (C 채택 (a)·D 기각 (iii) 앞항은 '동등 이상' = 우세 ∪ 등가라 무영향, "
          "C 기각 앞 분기·E 기각 (ii)는 어휘가 아니라 임계값 비교라 무관.)")
    blind = 0
    for v, lo, hi, where in VOCAB_OVERLAP:
        if "브라우저?" in where or "조항?" in where:
            blind += 1
        print(f"    {v}  CI=[{lo:.3f}, {hi:.3f}]  (등가 대역 [{EQ_LO:.3f}, {EQ_HI:.3f}] 안)  {where}")
    if blind:
        print(f"  ⚠️ 맥락이 안 붙은 기록 {blind}건 — `set_ctx()`를 안 거친 판정 경로가 있다. "
              f"어느 조항이 갈렸는지 알 수 없으므로 그 경로를 찾아 태그를 달 것.")


def report_batch_mix():
    """판정 비교에서 배치 모드가 갈린 짝을 모아 보고한다.

    §8 배치 규칙은 셀마다 파일럿(`pilot_min < 해상도×50`)으로 정한다. 그런데
    무조건 write 변형은 **가장 싼 후보를 임계값 위로 밀어 올리므로** 같은 논리
    셀의 두 변형이 서로 다른 모드로 측정될 수 있다(실측 2026-09-25: W1×R1에서
    조건부 alternate / 무조건 none, C의 파일럿 median 0.138 vs 0.485 — 임계값
    0.25ms의 양쪽). §9 서두가 후보별로 우세 변형을 고르므로, 고른 변형과 기준선
    후보(K는 이중 측정 대상이 아니라 언제나 조건부)가 다른 모드에서 올 수 있다.

    **짝짓기 자체는 유효하다**: §8이 배치 표본을 "k회 묶음 총 시간/k = 1측정치"로
    정의했으므로 차원이 같고, 시드가 write 변형을 포함하지 않으므로(2026-09-24
    수리) 같은 rep 인덱스는 같은 target을 본다. 갈리는 것은 **정밀도**(배치 쪽
    분산이 1/k)이고 그것은 CI 폭에 반영된다. 그래도 보고하는 이유: 두 median을
    같은 정밀도의 값으로 읽으면 안 되고, 혼용이 실제로 일어났는지는 데이터마다
    다르기 때문이다(전부 조건부를 고르면 혼용이 아예 없다).
    """
    print(f"\n{'-' * 78}\n§8 배치 모드 혼용 (판정 짝의 두 레코드가 다른 모드)\n{'-' * 78}")
    if not BATCH_MIX:
        print("  없음 — 모든 판정 짝이 같은 배치 모드에서 왔다.")
        return
    print(f"  {len(BATCH_MIX)}건. 짝짓기는 유효하지만(§8상 차원 동일, 같은 rep = 같은 target) "
          f"정밀도가 다르다 — 두 median을 같은 정밀도로 읽지 말 것.")
    for where, cand, am, base, bm, av, bv in BATCH_MIX:
        print(f"    {where}")
        print(f"      {cand}({av})={am}  vs  {base}({bv})={bm}")


def report_variant_choices():
    """§9 서두가 요구한 "셀별 우세 write 변형"의 선택 내역 전수."""
    if not VARIANT_LOG:
        return
    print(f"\n{'-' * 78}\nwrite 변형 선택 내역 (§9 서두 — 선택은 기계적)\n{'-' * 78}")
    print("  조작화: median이 더 작은 변형(개정 #9 후속 등재, **수집 전** 결정).")
    print("  등가 구간에서 고른 경우가 있는지 CI로 확인할 것.")
    for wl, rb, cand, pick, note in VARIANT_LOG:
        print(f"    {wl:5s} R{rb} {cand:6s} → {pick:14s}  {note}")


def profile_w7(idx, browser):
    set_ctx(browser=browser, clause="W7 프로파일(판정 아님)")
    print(f"\n{'=' * 78}\nW7 (값 갱신) 프로파일 — {browser}   ※ §9 어느 조항에도 걸리지 않음\n{'=' * 78}")
    print("  §3.4 기대: 'C, D, E (retain 경로)' / 'K (전량 재스폰)'")
    for cand in ["K", "C", "D", "E", "D-LIS", "E-LIS"]:
        r = cell(idx, "W7", 1, cand, item=STR)  # §3.2: W7은 양쪽 — heap 쪽 프로파일
        if not r:
            continue
        ops = r["m2_dom_ops"]["total_mutations"]
        med = statistics.median(r["samples_ms"])
        print(f"    {cand:6s} median={med:8.4f}ms  DOM op={ops:5d}")
    print("  ※ Phase 1의 D는 키가 아이템 표시 텍스트 자체라 값 변경 = 키 변경 —")
    print("     §3.4의 'D (retain 경로)' 기대는 별도 key 채널을 전제한 것이다(§11 개정 #3).")


# ---------------------------------------------------------------------------
# §7 M2 교차 검증 — 조항이 요구하는 대응표가 실제로 맞는지
# ---------------------------------------------------------------------------

def integrity_checks(rows, browser):
    """판정을 내리기 **전에** 데이터가 조항이 요구하는 조건에서 나왔는지 본다.

    2026-08-03의 적대적 감사가 확증한 결함들이 재발했는지를 데이터만 보고
    판별할 수 있는 항목들이다 — 하네스가 고쳐졌다는 것과 이 수집분이 고쳐진
    하네스에서 나왔다는 것은 다른 명제다.
    """
    print(f"\n{'=' * 78}\n무결성 점검 (판정 전) — {browser}\n{'=' * 78}")
    problems = []

    # W6은 스키마가 다르다(§8 "반복 개념 없음" — `samples_ms`도 `live_node_slots`도
    # 없다). 아래 공통 점검은 비교 가능한 셀에만 걸고, W6은 자기 항목으로 따로 본다.
    w6_rows = [r for r in rows if r.get("clause") == "W6" or r.get("workload") == "stress"]
    rows = [r for r in rows if r not in w6_rows]
    if w6_rows:
        bad = [r for r in w6_rows
               if not r.get("over_cell_cap")
               and r.get("inserts_completed") != r.get("inserts_target")]
        if bad:
            problems.append(
                f"W6 {len(bad)}셀이 상한 초과가 아닌데 목표 삽입 수를 못 채웠다 — "
                "중간에 조용히 끊긴 것이다")
        n_capped = sum(1 for r in w6_rows if r.get("over_cell_cap"))
        print(f"  W6: {len(w6_rows)}셀 중 상한 초과 {n_capped}셀 "
              f"(§8상 '측정 불가(상한 초과)'는 유효한 결과 데이터)")
        expected = 6 * 5
        if len(w6_rows) != expected:
            problems.append(f"W6 셀이 {len(w6_rows)}개 — 6후보 × 5시행 = {expected}이어야 한다")

    # row 하나가 만드는 기대 노드 수 — R2 바인딩 span, 조건부 앵커(+자식 최대 1),
    # W8 내부 `<ul>` + 항목, R3 B-브랜치 `<em>`(절반). 2026-09-22까지 (1)은 `n`만,
    # (8)은 `1 + binding`만 세어서 W8(inner=100)이 두 검사 모두에서 **위양성**
    # 경보를 냈다(감사 발견 #7).
    def expected_nodes_per_row(r):
        b, inner, var = r["row_bindings"], r.get("row_inner", 0), r.get("row_variants", False)
        n_ = 1 + max(b - 1, 0)
        if b > 1:
            # 조건부 앵커 1 + 자식(실효값 true인 row만 1개; 하네스 라벨에서
            # 60% — `row_cond_is_shown`이 {0,1,4,5,8,9}에서 true다. 2026-09-25
            # 정정 전에는 40%로 적혀 있었다). 상한 휴리스틱이므로 row당 1로 잡는다.
            n_ += 2
        if inner > 0:
            n_ += 1 + inner
        if var:
            n_ += 1  # `<em>` (실제로는 절반, 상한으로 1)
        return n_

    # (1) 노드 슬롯 누수 — 상주가 (n × 기대 노드/row)에 비례해야 하고 tick 수에
    # 비례하면 안 된다.
    worst = max(rows, key=lambda r: r["live_node_slots"] / max(r["n"] * expected_nodes_per_row(r), 1))
    ratio = worst["live_node_slots"] / max(worst["n"] * expected_nodes_per_row(worst), 1)
    ok = ratio < 20
    print(f"  노드 슬롯 상주 최대 비율: {ratio:.2f}× (n={worst['n']}, inner={worst.get('row_inner', 0)}, "
          f"{worst['candidate']}/{worst['workload']})")
    print(f"    ⇒ {'정상' if ok else '**누수 의심**'} (수리 전이면 tick 수 × n으로 자란다)")
    if not ok:
        problems.append("노드 슬롯 누수 의심")

    # (2) 배치 모드가 셀 안에서 갈리면 paired bootstrap의 전제가 깨진다(§8).
    from collections import defaultdict
    modes = defaultdict(set)
    for r in rows:
        # 키에 `row_inner`·`write_variant`가 **들어가야 한다**(2026-09-24). 빠지면
        # 두 write 변형(서로 다른 셀이고 배치 판정도 각자 한다)이 한 키로 합쳐져
        # 거짓 "§8 시드 열 공유 전제 붕괴" 경보가 난다. 이 검사가 잡으려는 것은
        # **한 셀 안에서 후보 간** 배치 모드가 갈리는 것이다.
        modes[(r["item_type"], r["n"], r["row_bindings"], r.get("row_inner", 0),
               r["row_variants"], r.get("write_variant", "Conditional"), r["workload"])].add(
            r.get("batch_mode", "?"))
    split = [k for k, v in modes.items() if len(v) > 1]
    print(f"  배치 모드가 후보 간에 갈린 셀: {len(split)}/{len(modes)}")
    print(f"    ⇒ {'정상' if not split else '**§8 시드 열 공유 전제 붕괴**'}")
    if split:
        problems.append("배치 모드 셀 내 분기")
        for k in split[:3]:
            print(f"      {k}: {modes[k]}")
    dist = defaultdict(int)
    for r in rows:
        dist[r.get("batch_mode", "?")] += 1
    print(f"    모드 분포: {dict(dist)}")

    # (3) §8 셀 상한.
    over = [r for r in rows if r.get("over_cell_cap")]
    print(f"  §8 셀 상한(30분) 초과: {len(over)}건 — 최대 셀 벽시계 "
          f"{max(r['cell_wall_ms'] for r in rows) / 1000:.1f}초")
    if over:
        problems.append("셀 상한 초과")

    # (4) §8 타이머 조항.
    #
    # **엔진마다 격리 상태의 하한이 다르다**(2026-08-03 실측으로 확인):
    #   Chromium 149: crossOriginIsolated=true → 0.005ms (5µs)
    #   Firefox 151:  crossOriginIsolated=true → 0.020ms (20µs)
    # 처음엔 임계를 0.01ms로 두었다가 Firefox를 "클램프됨"으로 **오탐**했다.
    # 격리가 안 걸리면 Chromium은 0.1ms, Firefox는 1ms 수준으로 훨씬 커지므로
    # 임계는 0.05ms면 충분히 갈린다. (근본 수리는 `cross_origin_isolated`를
    # 트랙 L 레코드에도 싣는 것 — 트랙 T는 이미 싣고 있다. 그건 하네스 변경이라
    # §2.5상 재수집을 부르므로 다음 라운드로 미룬다.)
    res = {r["timer_resolution_ms"] for r in rows}
    clamped = max(res) > 0.05
    print(f"  타이머 해상도: {sorted(res)} ms — "
          f"{'격리 정상(엔진별 하한: Chromium 0.005 / Firefox 0.020)' if not clamped else '**클램프됨(COOP/COEP 미적용?)**'}")
    if clamped:
        problems.append("타이머 클램프")

    # (4-b) **`samples_ms`와 `stage_ms`/`m2_dom_ops`의 시간 스케일이 다른 셀.**
    # 시퀀스형(W5)은 한 표본이 100스텝 합인데 단계·op 수는 마지막 1스텝이고,
    # 배치 셀은 표본이 k회 평균인데 단계·op 수는 마지막 정방향 1회다. 판정
    # 조항들은 stage_ms **안에서의 비율**만 쓰므로 판정은 오염되지 않지만,
    # 프로파일을 읽는 사람이 두 수치를 나누면 조용히 100배 틀린다. 하네스가
    # `seq_steps`/`stage_scope`를 싣게 했으므로 여기서 그대로 보고한다.
    scoped = defaultdict(list)
    for r in rows:
        scoped[(r.get("stage_scope", "(미기재)"), r.get("seq_steps", "?"))].append(r["workload"])
    print("  단계·M2 귀속 구간(`stage_scope`) 분포:")
    for (scope, steps), wls in sorted(scoped.items(), key=lambda kv: str(kv[0])):
        mixed = " ← samples_ms와 스케일 다름(나누지 말 것)" if steps not in (1, "?") \
            else (" ← 표본은 k회 평균" if scope == "last_forward_step_of_batch" else "")
        print(f"    {scope} (스텝 {steps}): {len(wls)}건 {sorted(set(wls))}{mixed}")
    if "(미기재)" in {k[0] for k in scoped}:
        problems.append("stage_scope 미기재 — 하네스가 구간을 안 싣는 수집분이다")

    # (5) §8 표본 수 — **실제 개수**를 센다(2026-09-24). 그 전에는 선언 필드
    # `reps`(항상 200)를 읽어서, 상한 초과로 잘린 셀에서도 "§8 준수"를 찍었다.
    # 계측이 자기가 재려던 것을 안 재는 전형이다.
    reps = {r["reps"] for r in rows}
    warm = {r.get("warmup") for r in rows}
    actual = {len(r["samples_ms"]) for r in rows}
    short = [r for r in rows if len(r["samples_ms"]) != r["reps"]]
    capped = [r for r in short if r.get("over_cell_cap")]
    print(f"  표본: 워밍업 선언 {sorted(warm)} / 측정 선언 {sorted(reps)} / **실제** {sorted(actual)}")
    if reps != {200} or warm != {50}:
        problems.append("표본 수 선언값 §8 미달")
    if short:
        print(f"    선언보다 적은 셀 {len(short)}개 (그중 상한 초과 {len(capped)}개 — §8상 유효한 결과)")
        unexplained = len(short) - len(capped)
        if unexplained:
            problems.append(f"표본이 선언보다 적은데 상한 초과도 아닌 셀 {unexplained}개")
    else:
        print("    ⇒ 전 셀이 선언 표본 수를 채웠다")

    # (6) 배치 묶음 구성 — §11 개정 #5 이전의 `chain` 셀은 무효다.
    chain = [r for r in rows if r.get("batch_mode") == "chain"]
    print(f"  §11 개정 #5 이전의 chain 배치 셀: {len(chain)}건")
    if chain:
        problems.append("chain 배치 셀 혼입(개정 #5로 무효)")
        print("    ⇒ **판정에 쓸 수 없다** — chain의 이웃 전이는 워크로드가 아니다")

    # (6-b) §7 M2 결정성의 **런 내부** 검사 (2026-08-07 신설). 하네스가
    # 결정적 워크로드(swap/remove_middle)에서 첫 rep과 마지막 rep의 op 수를
    # 대조해 `m2_deterministic`으로 실어 보낸다. `null`은 "해당 없음"(난수
    # 워크로드)이지 통과가 아니다 — false만 결함이다.
    nd = [r for r in rows if r.get("m2_deterministic") is False]
    if nd:
        print(f"  §7 M2 런 내부 결정성 위반: {len(nd)}건 "
              f"— rep 사이에 상태가 새고 있다(v15의 배치 소진 결함과 같은 유형)")
        for r in nd[:6]:
            print(f"    {r['candidate']}/{r['workload']}/n={r['n']}/R{r['row_bindings']}")
    else:
        checked = sum(1 for r in rows if r.get("m2_deterministic") is True)
        if checked == 0:
            # 0건은 "통과"가 아니라 "이 필드가 없는 옛 수집분"이다. 둘을 같은
            # 문장으로 보고하면 계측 부재가 통과로 읽힌다.
            print("  §7 M2 런 내부 결정성: **미계측** — 이 수집분은 "
                  "`m2_deterministic` 필드 이전(2026-08-07 신설)이다")
            problems.append("§7 M2 런 내부 결정성 미계측(옛 수집분)")
        else:
            print(f"  §7 M2 런 내부 결정성: 위반 0건 (검사된 셀 {checked}개)")

    # (7) §7 M2 "시드 고정 하에 결정적 수치" — 같은 셀·같은 후보의 op 수는
    # 브라우저와 무관하게 같아야 한다. 다르면 시드/워크로드/배치가 어긋난 것이다.
    # 이 검사가 없어서 배치 결함 (1)(2)(3)이 데이터만으로는 잡히지 않았다.
    print("  §7 M2 결정성(브라우저 간 op 수 일치)은 종합 절에서 대조한다.")

    # (8) 노드 슬롯 임계 — 셀의 기대 노드 수는 row_bindings에 비례한다.
    worst2 = max(rows, key=lambda r: r["live_node_slots"] / max(r["n"] * expected_nodes_per_row(r), 1))
    ratio2 = worst2["live_node_slots"] / (worst2["n"] * expected_nodes_per_row(worst2))
    print(f"  노드 슬롯 / (n × 기대 노드/row): 최대 {ratio2:.2f}× "
          f"(R{worst2['row_bindings']}, inner={worst2.get('row_inner', 0)}, "
          f"{worst2['candidate']}/{worst2['workload']})")
    if ratio2 > 2.0:
        problems.append("노드 슬롯 누수 의심(binding 보정)")

    # (9) **방출 op vs 관측 op 대조** — 떨어져 나간 노드에 쓰는 결함 탐지.
    #
    # 2026-08-07에 R2 × C가 `set_text_content` 7,992회를 내는데 MutationObserver는
    # `characterData` 999건(정확히 1/8)만 관측하는 것이 발견됐다. 원인은
    # `BrowserDom::set_text_content`의 파괴적 폴백이 row의 binding span을 전부
    # 지우고, 뒤이은 쓰기가 떨어져 나간 핸들로 갔던 것 — R2 축이 재려던 비용이
    # 측정에서 통째로 사라져 있었다. 이 대조가 있었으면 첫 수집에서 잡혔다.
    #
    # 판정 규칙: reposition/create가 없는(= 순수 텍스트 갱신) 셀에서
    # 관측 characterData가 방출 setText의 절반에 못 미치면 유실로 본다.
    lost = []
    for r in rows:
        o, m = r["m2_dom_ops"], r.get("mutation_records")
        if not m or o["reposition"] or o["create_element"] or o["remove_node"]:
            continue
        st = o["set_text_content"]
        if st >= 8 and m["character_data"] * 2 < st and m["child_list"] == 0:
            lost.append((r["workload"], r["row_bindings"], r["candidate"], st, m["character_data"]))
    print(f"  방출 setText ↔ 관측 characterData 유실 셀: {len(lost)}건")
    for w, rb, c, st, cd in lost[:5]:
        print(f"    {w}/R{rb}/{c}: 방출 {st} vs 관측 {cd}")
    if lost:
        problems.append("DOM write 유실(떨어져 나간 노드에 쓰기)")

    # (10) 브라우저 빌드 식별자 — §11 개정 #4가 Firefox 열 전체를 무효화한
    # 원인(Playwright 번들 빌드)을 데이터만으로 다시 판별할 수 있어야 한다.
    ua = {r.get("user_agent") for r in rows if r.get("user_agent")}
    print(f"  브라우저 빌드 식별자: {'있음 — ' + next(iter(ua))[:70] if ua else '**없음** (개정 #4의 원인을 데이터로 재판별 불가)'}")
    if not ua:
        problems.append("브라우저 빌드 식별자 부재")

    print(f"\n  ⇒ 무결성 문제 {len(problems)}건" + (f": {problems}" if problems else ""))
    return problems


def crosscheck_m2(idx, browser):
    set_ctx(browser=browser, clause="§7 M2 브라우저 간 대조(판정 아님)")
    print(f"\n{'=' * 78}\n§7 M2 MutationObserver 교차 검증 — {browser}\n{'=' * 78}")
    print("  대응표: setText → characterData, insert/remove/move 폴백/detach/attach → childList")
    print("  ※ 네이티브 moveBefore는 관측상 remove+insert 쌍으로 보인다(§7 M2 주석).")
    print("  ※ **새로 만든 엘리먼트**에 대한 setText는 텍스트 자식이 없어")
    print("     `textContent=` 경로로 가므로 childList로 관측된다 — K가 그 경우다.")
    bad = 0
    for cand in ["K", "C", "D", "E"]:
        r = cell(idx, "W2", 1, cand)
        if not r or not r.get("mutation_records"):
            continue
        o, m = r["m2_dom_ops"], r["mutation_records"]
        st, rp, ce = o["set_text_content"], o["reposition"], o["create_element"]
        cd, cl = m["character_data"], m["child_list"]
        # 대응표가 성립해야 하는 것은 **retained 슬롯 값 갱신**뿐이다.
        # 엘리먼트를 새로 만드는 후보(K)는 구조적으로 childList로 간다.
        flag = ""
        if rp == 0 and ce == 0 and st > 0:
            miss = abs(cd - st)
            if miss > max(1, st // 100):  # 1% 초과만 구조적 불일치로 본다
                flag = f"  ⚠ setText↔characterData 불일치 ({miss}건)"
                bad += 1
            elif miss > 0:
                flag = f"  (오차 {miss}건 — 1% 이내, 병기만)"
        print(f"    {cand:6s} setText={st:5d} reposition={rp:5d} create={ce:5d} | "
              f"charData={cd:5d} childList={cl:5d}{flag}")
    print(f"  ⇒ 구조적 대응표 위반 {bad}건")


# ---------------------------------------------------------------------------

def main():
    print("RQ1 §9 트랙 L 판정 — 전 조항 일괄 적용")
    if DATA != "/home/ybi/loom-research/research-questions/data":
        print(f"  ** 주의: 기본이 아닌 데이터 경로를 읽는다 — {DATA} **")
    print(f"데이터: rq1-phase1-track-l-core-grid-{VERSION}-{{chromium,firefox}}.json")
    print("판정은 §8 커버리지 표대로 **Chromium(1차 판정)**에서만 — Firefox는 교차 확인.")

    results = {}
    for browser, label in [("chromium", "CHROMIUM (1차 판정)"), ("firefox", "FIREFOX (교차 확인)")]:
        try:
            rows, idx = load(browser)
        except FileNotFoundError:
            print(f"\n[{label}] 데이터 파일 없음 — 건너뜀")
            continue
        if not rows:
            continue
        r0 = rows[0]
        print(f"\n\n{'#' * 78}\n# {label}   (레코드 {len(rows)}, 워밍업 {r0.get('warmup', '?')} "
              f"+ 측정 {r0.get('reps', '?')}, 해상도 {r0.get('timer_resolution_ms', float('nan')):.4f}ms)\n{'#' * 78}")
        batched = sum(1 for r in rows if r.get("batched"))
        print(f"# §8 배치 규칙 전환 셀: {batched}/{len(rows)}")
        integrity_checks(rows, label)
        results[browser] = {
            "_idx": idx,
            "C": gate_c(idx, label),
            "D": gate_d(idx, label),
            "E": gate_e(idx, label),
        }
        profile_w7(idx, label)
        profile_new_cells(rows, idx, label)
        crosscheck_m2(idx, label)

    report_variant_choices()
    report_batch_mix()
    report_vocab_overlap()
    print(f"\n\n{'=' * 78}\n종합 (Chromium 1차 판정 기준)\n{'=' * 78}")
    c = results.get("chromium")
    if not c:
        print("  Chromium 데이터 없음 — 판정 불가")
        return
    # `say()`를 쓰는 이유: 판정 불가(`None`)를 여기서 "X"나 "미발동"으로 접으면
    # 앞의 게이트 출력이 애써 구분한 "데이터가 없다"와 "조건이 안 맞는다"가
    # 종합 한 줄에서 다시 뭉개진다.
    print(f"  C: 채택(a)={say(c['C']['adopt_a'], 'O', 'X')} "
          f"채택(b)={say(c['C']['adopt_b'], 'O', 'X')} 기각={say(c['C']['reject'], '발동', '미발동')}")
    print(f"  D: 기각(iii) 앞항={say(c['D']['reject_iii_front'], '성립', '불성립')} "
          f"(뒷항 미계측 → 기각 불성립) / 양성조건={say(c['D']['adopt_positive'], 'O', 'X')}")
    print(f"  E: 기각(i)={say(c['E']['reject_i'], '발동', '미발동')} "
          f"기각(ii)={say(c['E']['reject_ii'], '발동', '미발동')}")
    f = results.get("firefox")
    if f:
        for cand in ("C", "D", "E"):
            if f[cand] != c[cand]:
                print(f"  ⚠️ Firefox가 {cand}에서 다른 게이트 결론 — §11-1(b) 검토 대상 등재 사유")
        bad_cells = cross_browser_consistency(results)
        cross_browser_direction_check(results, bad_cells)


def cross_browser_consistency(results):
    """§7 M2 결정성과 배치 모드 일치를 브라우저 간에 대조한다.

    §7 M2는 "시드 고정 하에 결정적 수치"라고 못박는다 — 같은 셀·같은 후보의
    op 수는 브라우저와 무관하게 같아야 한다. 다르면 시드·워크로드·배치 중
    하나가 어긋난 것이고, **그때 우세 방향 대조는 '측정 정의가 다른 두 셀'의
    비교가 된다**(2026-08-04 감사 지적).
    """
    print(f"\n{'-' * 78}\n브라우저 간 측정 정의 일치 (방향 대조의 전제)\n{'-' * 78}")
    ci, fi = results["chromium"]["_idx"], results["firefox"]["_idx"]
    common = set(ci) & set(fi)
    mode_diff = [k for k in common
                 if ci[k].get("batch_mode") != fi[k].get("batch_mode")]
    m2_diff = [k for k in common
               if ci[k].get("batch_mode") == fi[k].get("batch_mode")
               and ci[k]["m2_dom_ops"]["total_mutations"] != fi[k]["m2_dom_ops"]["total_mutations"]]
    print(f"  배치 모드가 브라우저 간에 다른 셀: {len(mode_diff)}/{len(common)}")
    if mode_diff:
        print("    ⇒ 그 셀들의 방향 대조는 **같은 조건의 비교가 아니다**")
        # 키 모양: (item_type, n, row_bindings, row_inner, row_variants, workload,
        # candidate, write_variant) — 개정 #9의 8-튜플. 2026-09-22까지 옛 6-튜플
        # 인덱스로 찍어 후보 자리에 workload가 나왔다.
        for k in sorted(mode_diff)[:6]:
            print(f"      n={k[1]}/R{k[2]}/{k[5]}/{k[6]}/{k[7]}: "
                  f"{ci[k].get('batch_mode')} vs {fi[k].get('batch_mode')}")
    print(f"  같은 배치 모드인데 M2가 다른 셀: {len(m2_diff)} "
          f"— {'정상(§7 M2 결정성 유지)' if not m2_diff else '**시드/워크로드 어긋남**'}")
    return set(mode_diff)


def cross_browser_direction_check(results, incomparable=frozenset()):
    """§8: "Firefox 교차 확인이 우세 방향을 뒤집으면 자동 판정 변경이 아니라
    §11-1(b) 검토 대상 등재."

    `incomparable`은 더 이상 읽지 않는다(레코드의 `batch_mode`를 직접 대조) —
    호출부 호환을 위해 남겼다.

    **게이트 결론이 같아도 셀 단위 방향은 뒤집힐 수 있다.** 게이트 dict만
    비교하면 그걸 놓친다 — v15에서 실제로 게이트 셋은 전부 일치하는데 비교쌍
    40건 중 9건이 정반대로 뒤집혔다(2026-08-03). 그래서 셀 단위로 따로 센다.
    """
    print(f"\n{'-' * 78}\n브라우저 간 우세 방향 대조 (§8 — 뒤집히면 §11-1(b) 등재 사유)\n{'-' * 78}")
    ci, fi = results["chromium"]["_idx"], results["firefox"]["_idx"]
    # **후보 상수를 쓴다** — `D`는 §10 D-4로 LIS 고정이라 데이터의 "D-LIS"다.
    # 2026-09-22까지 리터럴 "D"(무LIS)로 계산하면서 라벨은 D-LIS 게이트를 가리켰다
    # (감사 발견 #15).
    pairs = [(C, D, "C 채택(a)"), (C, K, "C 채택(b)"),
             (K, D, "D 기각(iii)앞항"), (D, K, "D 양성"), (E, K, "E 기각(i)")]
    total = mismatch = opposite = 0
    for w in JUDGE_WORKLOADS:
        for bindings in (1, 8):
            for a, b, tag in pairs:
                ka, kb = cell(ci, w, bindings, a), cell(ci, w, bindings, b)
                fa, fb = cell(fi, w, bindings, a), cell(fi, w, bindings, b)
                if not (ka and kb and fa and fb):
                    continue
                # **이미 조회한 레코드로 직접 대조한다.** 2026-09-22까지 옛 6-튜플 키로
                # `incomparable`을 조회해 **한 번도 매치되지 않았다**(키가 8-튜플로
                # 바뀐 뒤 잔존 코드) — 아래 "비교 불가" 분기는 죽은 코드였고 배치
                # 모드가 갈린 셀도 그대로 §11-1(b) "정반대" 집계에 들어갔다. 게다가
                # `cell()`이 셀별 우세 write 변형을 고르므로 키를 고정해 두면
                # Unconditional이 뽑힌 셀을 놓친다(감사 발견 #3/#16).
                if ka.get("batch_mode") != fa.get("batch_mode") or kb.get("batch_mode") != fb.get("batch_mode"):
                    print(f"  (비교 불가)  {w}×R{1 if bindings == 1 else 2} {a}/{b:5s}  "
                          f"— 브라우저 간 배치 모드 불일치 "
                          f"({ka.get('batch_mode')}/{fa.get('batch_mode')}, {kb.get('batch_mode')}/{fb.get('batch_mode')})")
                    continue
                total += 1
                vc = verdict(*paired_ratio_ci(ka["samples_ms"], kb["samples_ms"]))
                vf = verdict(*paired_ratio_ci(fa["samples_ms"], fb["samples_ms"]))
                if vc == vf:
                    continue
                mismatch += 1
                hard = {vc, vf} == {"우세", "열세"}
                opposite += hard
                mark = "⚠️ 정반대" if hard else "  (한쪽 비결정)"
                print(f"  {mark}  {w}×R{1 if bindings == 1 else 2:1d} {a}/{b:2s}  "
                      f"Chromium={vc:4s} → Firefox={vf:4s}   [{tag}]")
    print(f"\n  비교쌍 {total}건: 어휘 불일치 {mismatch}건, 그중 **정반대** {opposite}건")
    if opposite:
        print("  ⇒ §11-1(b) **검토 대상 등재 사유 발생**. 판정은 §8대로 Chromium 단독으로")
        print("     유지하되(브라우저 간 풀링·평균 금지), 이 목록을 RQ1에 병기할 것.")


if __name__ == "__main__":
    main()
