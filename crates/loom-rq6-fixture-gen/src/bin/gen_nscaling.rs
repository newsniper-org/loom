//! `cargo run -p loom-rq6-fixture-gen --bin gen_nscaling [출력 디렉터리]`
//! 기본 출력: <workspace>/demo/islands/nscaling/ (Node 판과 같은 위치·같은 파일 이름).
//! 직접 부르기보다 `bench/e2e/fixtures/gen-nscaling.sh`(생성기 선택 설정)를 쓸 것.
use std::path::PathBuf;

fn main() {
    let out: PathBuf = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../demo/islands/nscaling")
    });
    std::fs::create_dir_all(&out).expect("출력 디렉터리 생성 실패");
    for n in loom_rq6_fixture_gen::NS {
        let path = out.join(format!("n{n}.html"));
        std::fs::write(&path, loom_rq6_fixture_gen::page(n)).expect("쓰기 실패");
        println!("wrote {}", path.display());
    }
}
