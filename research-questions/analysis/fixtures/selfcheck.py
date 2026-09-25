#!/usr/bin/env python3
"""`rq1-verdicts.py`가 조항대로 접히는지 합성 픽스처로 단언한다.

    python3 research-questions/analysis/fixtures/selfcheck.py

**이 파일이 지키는 성질**은 셋 다 "데이터가 정상일 때는 안 보이는 것"이다:

1. `base`  — 정상 데이터에서 종료코드 0으로 완주한다.
   (2026-09-22: 상한 초과 레코드의 `statistics.median([])`가 스크립트 전체를
   종료코드 1로 죽였다. §8은 상한 초과를 "유효한 결과 데이터"로 규정했으므로
   판정을 못 낼 뿐 실행을 막아서는 안 된다.)
2. `partial_cap` — 표본이 잘린 상한 초과 셀이 **판정에 안 들어간다**.
   무결성 절이 선언값(`reps`, 항상 200)이 아니라 **실제 표본 수**를 센다.
3. `missing_cell` — "판정 불가"가 `▶` 요약 줄에서 "불충족"으로 접히지 않는다.

각 성질에는 **있어야 하는 문구**와 **있으면 안 되는 문구**를 함께 둔다. 앞쪽만
두면 단언이 공허해진다 — 절이 빈 절이 되어도 통과한다.
"""
import os
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
GEN = os.path.join(HERE, "make-track-l-fixture.py")
VERDICTS = os.path.join(HERE, "..", "rq1-verdicts.py")

# (모드, [있어야 하는 문구], [있으면 안 되는 문구])
CASES = [
    ("base", [
        "▶ C 판정:",
        "▶ D 판정:",
        "▶ E 판정:",
        # 정상 데이터에서는 어느 판정도 데이터 부재로 막히지 않아야 한다 —
        # 이게 없으면 아래 '있으면 안 되는 문구'가 공허해진다.
        "전 셀이 선언 표본 수를 채웠다",
    ], [
        "판정 불가(데이터 없음)",
        "상한초과",
    ]),
    ("partial_cap", [
        "**실제** [3, 200]",
        "상한 초과 12개",
        "§8상 측정 불가 셀이라 판정에 못 쓴다",
        # 상한 초과 셀이 이중 측정 변형 선택에서도 배제돼야 한다.
        "선택 불가",
    ], [
        # 선언 필드만 읽던 시절의 문구. 표본이 3개인데 "§8 준수"를 찍었다.
        "측정 [200] — §8 준수",
    ]),
    ("missing_cell", [
        "⇒ (a) **판정 불가(데이터 없음)**",
        # ▶ 줄이 그 판정 불가를 보존하는가. 이 문구가 핵심이다.
        "▶ C 판정: 기각 미발동 — 채택 조건 **판정 불가(데이터 없음)**",
        "C: 채택(a)=**판정 불가(데이터 없음)**",
    ], [
        # 수리 전 실측 문구 — 같은 실행에서 (a)는 "판정 불가"인데 ▶는 "불충족"이었다.
        "▶ C 판정: 기각 미발동, 채택 조건 불충족",
    ]),
]


def run(mode, tmp):
    d = os.path.join(tmp, mode)
    subprocess.run([sys.executable, GEN, d, mode], check=True,
                   stdout=subprocess.DEVNULL)
    env = dict(os.environ, RQ1_DATA_DIR=d)
    p = subprocess.run([sys.executable, VERDICTS], env=env,
                       capture_output=True, text=True)
    return p.returncode, p.stdout + p.stderr


def main():
    fails = []
    with tempfile.TemporaryDirectory() as tmp:
        for mode, must, must_not in CASES:
            rc, out = run(mode, tmp)
            if rc != 0:
                fails.append(f"{mode}: 종료코드 {rc}\n{out[-2000:]}")
                continue
            for s in must:
                if s not in out:
                    fails.append(f"{mode}: 없어야 할 누락 — {s!r}")
            for s in must_not:
                if s in out:
                    fails.append(f"{mode}: 있으면 안 되는 문구 — {s!r}")
            print(f"  {mode}: 종료코드 0, 단언 {len(must)}+{len(must_not)}건")
    if fails:
        print("\n실패:")
        for f in fails:
            print(f"  - {f}")
        sys.exit(1)
    print("전 성질 통과")


if __name__ == "__main__":
    main()
