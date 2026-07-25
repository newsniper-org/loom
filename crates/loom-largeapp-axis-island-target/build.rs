//! RQ6 §4.4 "원본 앱 규모" 축(Task #27) — Island 팔의 절반: `crates/
//! loom-largeapp-axis-flat/build.rs`와 **동일한 합성 그래프**(TargetWidget +
//! filler 50개, 문자열 그대로 복제 — 두 크레이트가 정확히 같은 그래프를
//! 봐야 비교가 성립하므로)에서 `generate_single_island_client`로
//! TargetWidget **하나만** 추출한다. 이 크레이트의 wasm 산출물 크기가
//! filler 개수(N=50)와 무관하게 `demo/widget-embed`의 단독 QuantityPicker
//! 추출본과 비슷한 크기로 남아 있는지가 이 축의 핵심 질문.

use std::path::PathBuf;

fn synth_source(n_fillers: usize) -> String {
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

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let source = synth_source(50);
    let generated = loom_compiler::compile_tsx_to_single_island_rust(&source, "TargetWidget");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    std::fs::write(PathBuf::from(&out_dir).join("generated.rs"), generated)
        .unwrap_or_else(|e| panic!("build.rs: failed to write generated.rs: {e}"));
}
