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
3. `noise_not_neutral` — K만 스톨을 맞아 상대 꼬리가 두꺼운 상태. §9가 읽는
   비율은 "머신 소음이 후보 중립"이라는 전제 위에 있고, 그 전제는 §8의 rep 단위
   라운드로빈이 세운다. 그 전제가 실제로 성립하는지는 아무도 확인하지 않았다 —
   2026-09-26에 나는 "압박이 K를 더 벌한다"고 추측하고 그것을 근거로 수집 중단을
   권고했다가 데이터로 반박됐다(K의 상대 꼬리가 오히려 얇았다).
4. `batch_mix` — §8 배치 규칙이 같은 논리 셀의 두 write 변형을 다른 모드로 재고,
   §9 서두의 변형 선택이 기준선과 다른 모드의 레코드를 데려온 상태. 짝짓기는
   유효하지만(차원 동일, 같은 rep = 같은 target) 정밀도가 다르므로 보고돼야 한다.
4. `missing_cell` — "판정 불가"가 `▶` 요약 줄에서 "불충족"으로 접히지 않는다.
5. `vocab_overlap` — §8 어휘 겹침 기록에 **브라우저·조항 맥락이 실제로 붙는다**.
   (2026-09-24 첫 수리는 `where`에 `[{browser}]`를 찍으면서 그 키를 아무도 넣지
   않아 전 기록이 `[?]`였다. 그리고 프로파일 절이 앞 게이트의 조항 태그를
   물려받아 `W8×R1`이 "E 기각 (i)"로 찍혔다 — 판정이 아닌 것에 판정 조항
   이름이 붙었다.)

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
        # 시퀀스형 셀의 시간 스케일 표식이 실제로 보고되는가(2026-09-25).
        "last_step_of_sequence (스텝 100)",
        "samples_ms와 스케일 다름(나누지 말 것)",
        # §3.3 스케일링 행이 보고되는가 — 2026-09-26까지 읽는 보고서가 없었다.
        "§3.3 스케일링 프로파일",
        "C/K median 비율 외삽",
        # **"단조"로 단언하지 않는다**: 비단조 분기 문구가 `**비단조**`라서
        # `단조`를 부분문자열로 포함한다 — 어느 분기든 통과하는 공허한 단언이었고
        # 음성 대조(끝점 비교로 되돌리기)가 실패하지 않아서 드러났다. 단조성은
        # `scaling_nonmono` 모드에서 분기 고유 문구로 본다.
        "n=100:",
        # 정상 데이터에서는 정지 흔적이 없어야 한다(오탐 없음).
        "정지 흔적(벽시계 ≫ 기대): 0건",
        # 합성 데이터는 후보별 노이즈가 동일하므로 소음이 후보 중립이어야 한다.
        "중립(§9 비율의 전제 성립)",
        # 개정 #10(v22 형식) — 존재 단언부터. 새 절이 **실제로 돈다**.
        "셀 구성(개정 #10): 셀",
        "이중 측정 16개(9후보)",
        # 같은 셀 짝이 실제로 세어진다(0이면 아래 "건넌 짝 0"이 공허하다).
        "판별 불가(셀 소속 필드 없음 — v21 이전 형식) 0",
        "GC 통제(M-15): 셀",
        "강제 GC 관측:",
        "§8 GC 주석 — 판정 셀",
        "rep별 sync_point/total — median",
        "rep별 margin=(K.dom−E.dom)−E.systems",
        "**셀을 건넌 짝 0**",
        "W7 × R2 × heap:",
        # GC 주석의 **내용**이 실린다(존재만이 아니라 — 픽스처는 17번째 표본마다 겹침 → 200 중 12).
        "K 12/200표본",
        # write 변형이 행동(M2)을 바꾼 셀의 존재 단언이 돈다.
        "write 변형이 M2를 바꾼 이중 측정 셀 — C: 16/16",
    ], [
        "stage_scope 미기재",
        "판정 불가(데이터 없음)",
        "상한초과",
        # v22 형식에서 옛 경로 문구가 나오면 새 필드를 안 읽고 있는 것이다. ("v21 이전 형식"
        # 자체는 같은 셀 보고 줄의 **라벨**에도 들어 있어 금지하면 거짓 실패가 난다 — 옛 경로에서만
        # 나오는 문구로 좁힌다.)
        "n=1 점값",
        "레코드에 `gc` 필드가 없다",
        "셀 소속 필드(`group`) 없음",
        "W7 × R2 × heap: 미계측",
    ]),
    ("legacy_v21", [
        # 옛 수집분도 끝까지 돌고, 옛 경로임을 **밝힌다**.
        "▶ C 판정:",
        "▶ E 판정:",
        "**n=1 점값**(마지막 측정 rep — v21 이전 형식)",
        "셀 소속 필드(`group`) 없음 — v21 이전 형식",
        "판별 불가(셀 소속 필드 없음 — v21 이전 형식)",
        "레코드에 `gc` 필드가 없다 — **v21 이전 형식**",
    ], [
        "rep별 sync_point/total",
        "gc 필드 없음 — 개정 #10 하네스 수집분이 아니다",
    ]),
    ("cross_cell", [
        # 옛 하네스처럼 변형이 다른 셀에서 온 짝을 **잡는다**.
        "⚠️**셀을 건넌 짝**",
        "C의 두 변형이 한 셀에 다 있지 않다",
        "개정 #10 하네스에서는 판정 짝이 셀을 건널 수 없다",
    ], [
        "**셀을 건넌 짝 0**",
    ]),
    ("gc_missing", [
        "Chromium인데 GC 통제가 이행되지 않은 셀 3개",
        "비스로틀 열인데 스로틀 배수가 1이 아닌 셀 1개",
        "강제 GC가 트레이스에서 관측되지 않은 셀 1개",
        "GC 로그가 불완전한 셀 1개",
        # 통제가 없던 셀을 "관측"으로 세지 않는다(3 missing + 1 미관측 → 28셀 중 24).
        "강제 GC 관측: 24/28셀",
        "주석없음(missing-binding)",
    ], [
        "레코드에 `gc` 필드가 없다",
    ]),
    ("e_ii_margin", [
        # 개정 #10 조작화: 짝지은 margin의 median(+0.15) > 0 → 미발동. 점값(0.5 ≥ 0.2)이면 발동이었다.
        "rep별 margin=(K.dom−E.dom)−E.systems  median 0.1500ms",
        "⇒ 기각 (ii) 미발동",
    ], [
        "⇒ 기각 (ii) **발동**",
    ]),
    ("throttle_not_applied", [
        "**4×가 아닌 셀이 있다 — 그 셀은 스로틀 데이터가 아니다**",
    ], [
        "→ 전 셀 4×",
    ]),
    ("write_axis_inert", [
        "write 변형이 M2를 바꾼 이중 측정 셀이 **없다**",
    ], [
        "write 변형이 M2를 바꾼 이중 측정 셀 — C: 16/16",
    ]),
    ("stage_median", [
        # 판정은 rep별 median(30%)을 읽는다 — 마지막 rep 점값(60%)이면 발동이었다.
        "rep별 sync_point/total — median 30.0%",
        "마지막 rep 점값 60.0% — v21의 판정 방식, 판정에 안 씀",
        "⇒ 뒷 분기 미발동",
    ], [
        "⇒ 뒷 분기 **발동**",
    ]),
    ("partial_cap", [
        "**실제** [3, 200]",
        "상한 초과 9개",  # v22: 상한에 걸린 셀 하나 = 9후보(v21 형식이면 두 셀 12)
        "§8상 측정 불가 셀이라 판정에 못 쓴다",
        # 상한 초과 셀이 이중 측정 변형 선택에서도 배제돼야 한다.
        "선택 불가",
    ], [
        # 선언 필드만 읽던 시절의 문구. 표본이 3개인데 "§8 준수"를 찍었다.
        "측정 [200] — §8 준수",
    ]),
    ("vocab_overlap", [
        # 판정 조항의 겹침 — 조항 이름이 붙어야 한다.
        "C 채택 (b) — W1×R1 C/K",
        # 판정이 아닌 절은 그렇게 표시돼야 한다.
        "개정 #9 신설 셀 프로파일(판정 아님) — W8×R1 C/K",
        "[CHROMIUM (1차 판정)]",
        # 우선순위가 가르는 조항이 넷이라는 것(2026-09-25 정정)을 절이 말하는가.
        "우세∩등가에서 갈리는 조항: C 채택 (b) · D 채택 양성 조건 · D W5 부속 · E 기각 (i)",
        # 2026-09-27: 열세∩등가에서는 '동등 이상' 조항도 갈린다 — 그 사실을 찍는가.
        "열세∩등가에서 갈리는 조항: 위 넷 + C 채택 (a) · D 기각 (iii) 앞항",
    ], [
        "브라우저?",
        "조항?",
        "맥락이 안 붙은 기록",
    ]),
    ("throttle_threshold", [
        # 스로틀 경로가 실제로 돈다.
        "4× CPU 스로틀 — 강건성 확인 (판정 아님)",
        "스로틀 적용 확인",
        # 어휘 대조는 침묵한다 — "열세 → 열세"라서. 이게 등재한 구멍의 절반이다.
        "방향이 갈린 판정 셀 없음",
        # 게이트 구성 요소 대조는 잡는다 — 구멍의 나머지 절반이자 보강의 존재 이유.
        "⚠️ C 기각 앞 분기",
        "게이트 결론이 갈린 요소 1개",
        # 스로틀 쪽 게이트 줄은 판정으로 읽히지 않게 표시된다.
        "▷ C (강건성 확인 — 판정 아님)",
        # 범위 밖 요소를 거짓 불일치로 세지 않는다.
        "기각 뒷 분기 (W2 × R3 — 핵심 격자 밖) — 범위 밖",
        # 개정 #10: 스로틀이 걸렸다는 증거가 레코드에 있다.
        "셀 시작 시 걸린 배수 {4: ",
        "전 셀 4×",
    ], [
        "범위 안의 게이트 구성 요소 전부 결론 동일",
    ]),
    ("noise_not_neutral", [
        "**특정 후보가 스톨을 더 맞았다**",
        "머신 소음이 후보 중립이 아님(K",
    ], [
        "중립(§9 비율의 전제 성립)",
    ]),
    ("stalled_cell", [
        # 정지 흔적 검사가 실제로 울리는가(공허성 방지).
        "정지 흔적(벽시계 ≫ 기대): 1건",
        "W1 n=1000 R1 inner=0",
        "정지 흔적 1셀 — 측정 중 머신 압박 의심",
    ], [
        "정지 흔적(벽시계 ≫ 기대): 0건",
    ]),
    ("scaling_nonmono", [
        # 비단조 분기 고유 문구 — 끝점 비교로 되돌리면 사라진다.
        "방향을 말할 수 없다",
        "끝점만 보면",
    ], [
        # **"단조 …" 문구를 금지하지 않는다**: 이 모드는 W2×R2만 비단조로 만들고
        # 나머지 두 계열(W2×R1, W4×R2)은 단조일 수 있다 — 전역 금지는 그 정상
        # 출력에 걸려 거짓 실패를 낸다(실제로 그랬다). 분기 고유 문구 두 개로
        # 충분하다: 끝점 비교로 되돌리면 둘 다 사라진다(음성 대조로 확인).
    ]),
    ("batch_mix", [
        # 변형 선택 줄이 두 변형의 배치 모드를 말하는가.
        "**두 변형의 배치 모드가 다르다**",
        # 판정 줄에 혼용이 병기되는가.
        "⚠️배치 모드 혼용",
        # 전용 보고 절이 도는가 + 짝짓기가 유효하다는 근거를 함께 적는가.
        "§8 배치 모드 혼용 (판정 짝의 두 레코드가 다른 모드)",
        "같은 rep = 같은 target",
    ], [
        "없음 — 모든 판정 짝이 같은 배치 모드에서 왔다.",
    ]),
    ("firefox_bidi", [
        "RQ1_FIREFOX_FILE 지정",
        "(에이전트 기대)",
        "Firefox 강제 GC 구간에 GCSlice(일시정지) 마커가 없는 셀 1개",
        "Firefox 강제 GC 구간에 CC가 관측되지 않은 셀 1개",
        "Firefox GC 로그 출처: ['gecko-profiler",
        # 상한 셀의 뒤집힌 방향은 측정 결함이다 — 대조에서 빠졌다는 줄이 있어야 한다.
        "(측정 불가)  W1×R1",
    ], [
        "⚠️ 정반대  W1×R1",
        "Firefox(에이전트 없는 수집분)",
    ]),
    ("firefox_wrong_file", [
        "Firefox BiDi 에이전트 수집분을 기대했는데 GC 통제가 된 셀이 없다",
    ], [
        "Firefox GC 로그 출처",
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


# (모드, 대조 모드, 같아야 하나) — 문구가 아니라 **출력 전체**를 비교하는 차등 성질.
#
# `d_stage_inert` (2026-10-30, 판정기 사전 점검 — G0 재심사 발견 6): D 무조건 변형은 SetText 방출을
# `systems`에서 `row_patch`로 옮긴다. 그 이동이 판정을 기울이지 않으려면 **D·D-LIS의 단계 시간을 읽는
# 절이 하나도 없어야** 한다(D의 판정은 전부 `samples_ms` = tick 전체 M1(a)다). D·D-LIS 두 변형의 단계
# 시간을 1000배로 만들어도 출력이 `base`와 바이트 단위로 같아야 한다.
# `e_stage_control`은 그 비교가 공허하지 않다는 대조다 — 같은 변환을 E-LIS에 걸면(E 기각 (ii)가 읽는다)
# 출력이 **달라져야** 한다.
DIFF_CASES = [
    ("d_stage_inert", "base", True),
    ("e_stage_control", "base", False),
]


def run(mode, tmp):
    d = os.path.join(tmp, mode)
    subprocess.run([sys.executable, GEN, d, mode], check=True,
                   stdout=subprocess.DEVNULL)
    env = dict(os.environ, RQ1_DATA_DIR=d)
    # 옛 형식 모드는 v21 파일 이름으로 쓰인다(생성기의 `LEGACY`와 같은 목록).
    if mode in ("legacy_v21", "batch_mix"):
        env["RQ1_VERSION"] = "v21"
    else:
        env.pop("RQ1_VERSION", None)
    if mode.startswith("firefox"):
        env["RQ1_FIREFOX_FILE"] = os.path.join(d, "rq1-phase1-track-l-core-grid-v22-firefox-bidi.json")
    else:
        env.pop("RQ1_FIREFOX_FILE", None)
        env.pop("RQ1_FIREFOX_EXPECT_AGENT", None)
    p = subprocess.run([sys.executable, VERDICTS], env=env,
                       capture_output=True, text=True)
    # 데이터 경로는 모드마다 다르다 — 차등 비교가 경로 문자열에서 갈리지 않게 지운다.
    return p.returncode, (p.stdout + p.stderr).replace(d, "<DATA>")


def main():
    # `--only <모드>`로 한 모드만 돌린다. 음성 대조는 보통 한 모드만 보는데
    # 전 모드를 돌리면 5배 걸리고, 느리면 대조를 건너뛰게 된다.
    only = None
    if "--only" in sys.argv:
        only = sys.argv[sys.argv.index("--only") + 1]
    cases = [c for c in CASES if only is None or c[0] == only]
    diffs = [c for c in DIFF_CASES if only is None or c[0] == only]
    if not cases and not diffs:
        print(f"모드 {only!r} 없음. 가능: {[c[0] for c in CASES] + [c[0] for c in DIFF_CASES]}")
        sys.exit(2)
    fails = []
    outs = {}
    with tempfile.TemporaryDirectory() as tmp:
        for mode, must, must_not in cases:
            rc, out = run(mode, tmp)
            outs[mode] = (rc, out)
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
        for mode, ref, same in diffs:
            for m in (mode, ref):
                if m not in outs:
                    outs[m] = run(m, tmp)
            (rc, out), (rrc, rout) = outs[mode], outs[ref]
            if rc != 0 or rrc != 0:
                fails.append(f"{mode}: 종료코드 {rc} / 대조 {ref} {rrc}\n{out[-2000:]}")
                continue
            if (out == rout) != same:
                if same:
                    diff = [(a, b) for a, b in zip(out.splitlines(), rout.splitlines()) if a != b][:5]
                    fails.append(f"{mode}: `{ref}`와 출력이 달라졌다 — 이 단계 시간을 읽는 절이 있다: {diff}")
                else:
                    fails.append(f"{mode}: `{ref}`와 출력이 같다 — 대조가 공허하다")
                continue
            print(f"  {mode}: 출력 {'= ' if same else '≠ '}{ref} (차등 성질)")
    if fails:
        print("\n실패:")
        for f in fails:
            print(f"  - {f}")
        sys.exit(1)
    print("전 성질 통과")


if __name__ == "__main__":
    main()
