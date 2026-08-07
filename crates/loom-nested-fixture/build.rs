//! `demo/nested-conditional.tsx`를 실제 loomc 파이프라인으로 컴파일한다.
//! 손으로 옮겨 적은 "컴파일러가 냈을 법한" 코드가 아니라 진짜 산출물이어야
//! 이 크레이트의 테스트가 의미를 갖는다.

use std::path::PathBuf;

fn main() {
    for (tsx, out) in [
        ("nested-conditional.tsx", "generated.rs"),
        ("nested-list.tsx", "generated_list.rs"),
        ("nested-list-derived.tsx", "generated_derived.rs"),
    ] {
        let source_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo").join(tsx);
        println!("cargo:rerun-if-changed={}", source_path.display());
        let source = std::fs::read_to_string(&source_path).unwrap_or_else(|e| {
            panic!("loom-nested-fixture/build.rs: failed to read {source_path:?}: {e}")
        });
        let generated = loom_compiler::compile_tsx_to_rust(&source);
        let dest = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(out);
        std::fs::write(&dest, generated).unwrap_or_else(|e| {
            panic!("loom-nested-fixture/build.rs: failed to write {dest:?}: {e}")
        });
    }
}
