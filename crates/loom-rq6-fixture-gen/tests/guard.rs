//! 마커 어긋남 가드 + 두 생성기 바이트 동일성(2026-10-17 병익님 결정 — `src/lib.rs` 머리 주석).
use loom_rq6_fixture_gen::{page, quantity_picker_block, NS};
use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn committed_fixtures_are_reproduced_byte_for_byte() {
    for n in NS {
        let path = root().join(format!("demo/islands/nscaling/n{n}.html"));
        let committed = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(page(n).into_bytes(), committed, "Rust 생성기가 커밋된 {} 와 다르다", path.display());
    }
}

#[test]
fn node_and_rust_generators_are_byte_identical() {
    // Node가 없으면 **실패한다**(건너뛰지 않는다) — 건너뛰면 "동일하다"가 확인 없이 통과로 읽힌다.
    let tmp = std::env::temp_dir().join(format!("loom-nscaling-node-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    let st = std::process::Command::new("node")
        .arg(root().join("bench/e2e/fixtures/gen-nscaling-page.mjs"))
        .env("LOOM_NSCALING_OUT", &tmp)
        .output()
        .expect("node 실행 실패 — 이 가드는 Node 생성기가 있어야 의미가 있다");
    assert!(st.status.success(), "Node 생성기 실패: {}", String::from_utf8_lossy(&st.stderr));
    for n in NS {
        let node = std::fs::read(tmp.join(format!("n{n}.html"))).expect("Node 출력 없음");
        assert_eq!(page(n).into_bytes(), node, "N={n}: Node와 Rust 생성기 출력이 다르다");
    }
    // 존재 단언: Node가 실제로 **임시 디렉터리에** 썼다(기본 경로로 써서 커밋본과 비교된 게 아니다).
    assert!(String::from_utf8_lossy(&st.stdout).contains(&*tmp.to_string_lossy()), "{}", String::from_utf8_lossy(&st.stdout));
    let _ = std::fs::remove_dir_all(&tmp);
}

/// `name="value"` 속성값(첫 번째)을 뽑는다. 없으면 None.
fn attr<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let i = s.find(&key)? + key.len();
    Some(&s[i..i + s[i..].find('"')?])
}

/// 블록 하나에서 컴파일러 산출물과 같아야 하는 마커 묶음.
fn markers(block: &str) -> (Option<&str>, Option<&str>, Option<&str>, Option<&str>, Option<&str>) {
    (
        attr(block, "data-loom-activation"),
        attr(block, "data-loom-chunk"),
        attr(block, "data-loom-ctor"),
        attr(block, "data-loom-lref"),
        attr(block, "data-loom-id"),
    )
}

#[test]
fn markers_match_compiler_output() {
    // 컴파일러가 실제로 내는 Island SSR(GraphMode::Islands) — 손으로 적은 생성기 문자열이 이것과 어긋나면
    // N-스케일링 픽스처는 현재 청크가 받지 못하는 마커를 싣게 된다(RQ6 §10-1(b) 픽스처 유효성).
    let ssr = loom_widget_embed::render_html();
    let ctor = "data-loom-ctor=\"QuantityPickerIsland\"";
    // 존재 단언 먼저: 대조 대상이 실제로 있다(없어서 "어긋남 없음"이 되는 것을 막는다).
    assert_eq!(ssr.matches(ctor).count(), 1, "컴파일러 SSR에 QuantityPicker Island가 정확히 하나여야 한다:\n{ssr}");
    let start = ssr[..ssr.find(ctor).unwrap()].rfind("<div ").unwrap();
    let end = start + ssr[start..].find("</div>").unwrap();
    let compiled = &ssr[start..end];
    let got = markers(compiled);
    assert!(got.0.is_some() && got.1.is_some() && got.3.is_some() && got.4.is_some(), "컴파일러 블록에서 마커를 못 읽었다: {compiled}");
    assert!(attr(compiled, "data-loom-island-id").is_some_and(|id| id == "quantity-picker"), "{compiled}");
    for i in [0, 1, 49] {
        let block = quantity_picker_block(i);
        assert_eq!(markers(&block), got, "생성기 블록 #{i}의 마커가 컴파일러 산출물과 어긋났다:\n생성기: {block}\n컴파일러: {compiled}");
        assert_eq!(attr(&block, "data-loom-island-id"), Some(format!("quantity-picker-{i}").as_str()));
    }
}
