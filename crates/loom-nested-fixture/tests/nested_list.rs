//! RQ1 §5 트랙 N의 **3중 중첩**(리스트 > 조건부 > 리스트)이 실제로 돌아가는지.
//! 컴파일러가 낸 사이트별 `RowSpawner`가 row마다 조건부와 안쪽 리스트를
//! 만들고, 바깥 재정렬·토글·안쪽 편집이 서로를 망가뜨리지 않는지 본다.

use loom_nested_fixture::{generated_list as g, TreeDom};
use loom_runtime::LoomApp;

fn boot() -> (LoomApp, TreeDom, g::Signals, loom_runtime::NodeHandle) {
    let (mut dom, mount) = TreeDom::new();
    let mut app = LoomApp::new();
    let (signals, _a, _i) = g::setup(&mut app, &mut dom, mount);
    app.tick(&mut dom);
    (app, dom, signals, mount)
}

fn act(app: &mut LoomApp, dom: &mut TreeDom, s: &g::Signals, a: g::Action) {
    g::apply_action(app, s, &a);
    app.tick(dom);
}

fn outer_append() -> g::Action { g::Action::Action0("n".to_string()) }
const OUTER_ROTATE: g::Action = g::Action::Action1;
const TOGGLE: g::Action = g::Action::Action2;
fn inner_append() -> g::Action { g::Action::Action3("m".to_string()) }

/// `<ul class="outer">…</ul>` 부분만 잘라 낸다.
fn outer(dom: &TreeDom, mount: loom_runtime::NodeHandle) -> String {
    let html = dom.html(mount);
    let start = html.find("<ul class=\"outer\"").expect("outer 리스트가 없다");
    html[start..].to_string()
}

#[test]
fn closed_conditional_leaves_every_row_with_an_empty_panel() {
    let (_app, dom, _s, mount) = boot();
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<li>").count(), 3, "{html}");
    // row마다 조건부 wrapper가 하나씩, 전부 비어 있어야 한다.
    assert_eq!(html.matches("<div class=\"panel\"></div>").count(), 3, "{html}");
    assert!(!html.contains("class=\"inner\""), "닫힌 조건부의 안쪽 리스트가 그려졌다: {html}");
}

#[test]
fn opening_grows_every_row_with_its_own_inner_list() {
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    let html = outer(&dom, mount);
    // 3개 row 각각이 **자기** 안쪽 리스트를 갖는다(공유가 아니다).
    assert_eq!(html.matches("<ul class=\"inner\">").count(), 3, "{html}");
    assert_eq!(html.matches("<li>x</li><li>y</li>").count(), 3, "{html}");
}

#[test]
fn a_row_added_while_open_is_born_open_with_its_content() {
    // 사이트 Resource가 리스트 System보다 먼저 돌지 않으면 이 row만 한 틱
    // 비어 있게 된다 — 그 순서 의존이 실제로 지켜지는지.
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, outer_append());
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<ul class=\"inner\">").count(), 4, "{html}");
    assert_eq!(html.matches("<li>x</li><li>y</li>").count(), 4, "{html}");
}

#[test]
fn editing_the_inner_list_reaches_every_rows_copy() {
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, inner_append());
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<li>x</li><li>y</li><li>m</li>").count(), 3, "{html}");
}

#[test]
fn rotating_the_outer_list_carries_each_rows_subtree_with_it() {
    // 트랙 N 불변식 2의 씨앗: keyed move가 row의 **구간 전체**를 옮겨야 한다.
    // K는 전면 재생성이라 항상 옳지만, 이 픽스처가 D/E로도 돌아갈 때 이 단언이
    // 실제 판별력을 갖는다.
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, OUTER_ROTATE);
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<ul class=\"inner\">").count(), 3, "{html}");
    // row 루트의 텍스트가 회전했고, 각 row가 여전히 자기 panel을 하나씩 갖는다.
    let b = html.find("b</li>").or_else(|| html.find("b<div")).expect("b row 없음");
    let a = html.find("a</li>").or_else(|| html.find("a<div")).expect("a row 없음");
    assert!(b < a, "회전이 반영되지 않았다: {html}");
    assert_eq!(html.matches("<div class=\"panel\">").count(), 3, "{html}");
}

#[test]
fn closing_hides_every_rows_panel_without_destroying_it() {
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, TOGGLE);
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<div class=\"panel\" hidden>").count(), 3, "{html}");
    assert_eq!(html.matches("<li>x</li><li>y</li>").count(), 3, "내용이 파괴됐다: {html}");
}

#[test]
fn resume_rebuilds_the_nested_structure_from_declared_initials() {
    // row 안 조건부/중첩 리스트는 `data-loom-id`를 달 수 없다(row가 여럿이라
    // id가 유일하지 않다). 그래서 resume은 이들을 **선언된 초기값**으로
    // 되살린다 — SSR 이후의 편집 이력은 복원되지 않는다(RQ3 Q2의 "SSR 스냅샷
    // 동결"과 같은 성질, roadmap에 기록). 여기서 확인하는 것은 그 경로가
    // 실제로 돌고, 되살아난 앱이 이후 상호작용에 정상 반응한다는 것.
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    drop((app, s));

    let ids = dom.loom_id_table();
    let mut resumed = LoomApp::new();
    let s = g::resume(&mut resumed, &mut dom, |id| {
        *ids.get(id).unwrap_or_else(|| panic!("resume이 없는 `data-loom-id=\"{id}\"`를 찾았다"))
    }, &[]);

    // 선언된 초기값(open=false)으로 되살아나므로 panel은 닫혀 있다.
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<div class=\"panel\"").count(), 3, "{html}");

    // 그리고 되살아난 앱이 정상 반응한다.
    act(&mut resumed, &mut dom, &s, TOGGLE);
    let html = outer(&dom, mount);
    assert_eq!(html.matches("<ul class=\"inner\">").count(), 3, "{html}");
}
