# 공식 검증 스코프

RQ5 참고 후 읽을 것. 여기 있는 파일들은 **스켈레톤**이지 증명이 아님. 병익님의
descript→verify→implement 워크플로우를 따라, 설계(descript)가 RQ1/RQ3/RQ4/RQ6에서
아직 수렴하지 않은 상태에서 성급하게 증명(verify)부터 시작하지 않기 위해 의도적으로
얇게 유지함.

## 지금 스켈레톤이 있는 것

1. `lean4/SupervisionTree.stub.lean` — attach/detach 상태 기계의 불변식 스케치
   (RQ5의 후보 #1에 대응). 상태 정의만 있고 정리(theorem)와 증명은 비어있음.
2. `tla/SchedulerSafety.stub.tla` — read/write 선언 기반 무충돌 스케줄링의 안전성 속성
   틀 (RQ5의 후보 #2). 단 RQ5에서 지적했듯, 이게 Rust 타입 시스템으로 이미 보장되는
   속성의 중복 증명인지부터 먼저 판단 필요.

## 아직 스켈레톤조차 없는 것

- **Resumability 건전성** (RQ5의 후보 #3, 가장 가치 있는 정리). "resume 결과 = hydration
  했다면 나왔을 결과"의 formal한 정의부터 없음. 이건 04/RQ2의 직렬화 포맷이 어느 정도
  구체화된 뒤에나 정의를 시작할 수 있음.

## 검증 시작 조건

- Lean4: RQ3(Island 중첩 규칙)가 최소 잠정안이라도 정해진 뒤 착수
- TLA+: Phase 1 컴파일러 프로토타입이 실제로 System을 생성하기 시작한 뒤, 생성된 System들의
  실제 접근 패턴 예시를 가지고 착수 (추상적으로 먼저 증명하고 나중에 끼워맞추지 말 것)
