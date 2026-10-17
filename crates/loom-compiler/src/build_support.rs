//! `build.rs` 쪽 공용 도우미 (Phase 7a C1, 2026-10-17 신설).
//!
//! 13개 소비자 `build.rs`가 컴파일러를 부르는 방식을 한 곳에 모은다.
//! - `expect_for_build_rs`: `Result<Compiled, Diagnostics>`를 build.rs 관례로
//!   푼다(경고는 `cargo:warning=`, 오류는 렌더된 진단으로 panic).
//! - `largeapp_synth_source`: RQ6 §4.4 largeapp 축의 합성 소스. 원래
//!   `crates/loom-largeapp-axis-flat/build.rs`와
//!   `crates/loom-largeapp-axis-island-target/build.rs`에 **문자열 그대로
//!   복제**돼 있던 `synth_source`를 옮긴 것이다(두 팔이 같은 그래프를 봐야
//!   비교가 성립하므로, 복제보다 단일 정의가 그 전제를 구조적으로 보장한다).
//!   출력 문자열은 이관 전과 바이트 단위로 같다 — 게이트 A가 확인한다.

use crate::diag::{Compiled, Diagnostics};

/// build.rs 전용: 성공이면 생성된 Rust 소스를 돌려주고, 경고는 줄마다
/// `cargo:warning=`으로 내보낸다(`cargo:warning`은 한 줄만 받는다). 오류면
/// 렌더된 진단 전체로 panic한다 — build.rs에서 panic은 빌드 실패로 보고된다.
pub fn expect_for_build_rs(result: Result<Compiled, Diagnostics>) -> String {
    match result {
        Ok(c) => {
            if !c.warnings.is_empty() {
                for line in c.warnings.render().lines() {
                    println!("cargo:warning={line}");
                }
            }
            c.rust
        }
        Err(d) => panic!("\n{}", d.render()),
    }
}

/// largeapp 축 합성 소스의 가상 파일 이름(진단의 `파일:줄:열`에 쓰인다).
pub const LARGEAPP_SYNTH_FILE_NAME: &str = "largeapp-synth.tsx";

/// RQ6 §4.4 "원본 앱 규모" 축의 합성 소스: TargetWidget 하나 + 무관한
/// filler `n_fillers`개를 전부 `client:load`로 참조하는 App.
/// (이관 전 정의: da4a9bc의 crates/loom-largeapp-axis-flat/build.rs:18-34 —
/// 한 글자도 바꾸지 않았다.)
pub fn largeapp_synth_source(n_fillers: usize) -> String {
    let mut src = String::new();
    src.push_str(
        "function TargetWidget() {\n  const count = signal(0);\n  return <div><button onClick={() => count.increment()}>+</button><span>{count()}</span></div>;\n}\n",
    );
    for i in 0..n_fillers {
        src.push_str(&format!(
            "function Filler{i}() {{\n  const c{i} = signal(0);\n  return <button onClick={{() => c{i}.increment()}}>{{c{i}()}}</button>;\n}}\n"
        ));
    }
    src.push_str("export function App() {\n  return (\n    <div>\n      <TargetWidget client:load />\n");
    for i in 0..n_fillers {
        src.push_str(&format!("      <Filler{i} client:load />\n"));
    }
    src.push_str("    </div>\n  );\n}\n");
    src
}
