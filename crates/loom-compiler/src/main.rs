//! `loomc` bin — **빈 스텁**(2026-10-17 주석 추가, 코드 무변경).
//!
//! Cargo.toml이 `[[bin]] loomc`로 등록하지만 본체는 git 이력상 첫 커밋
//! (2026-07-24)부터 비어 있다. 컴파일러는 각 크레이트의 `build.rs`가 라이브러리 함수
//! (`compile_tsx_to_rust*`, `compile_tsx_to_single_island_rust` 등)를 직접 호출하는
//! 방식으로만 돈다. 사람이 산출물을 들여다볼 때는 `examples/print_demo.rs`.
//! CLI 표면을 만들지는 2026-10-17 현재 결정 기록이 없다.
fn main() {}
