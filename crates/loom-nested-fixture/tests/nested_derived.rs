//! **row마다 구간 크기가 다른** 3중 중첩. 균일 판본(`nested_list.rs`)이
//! 원리적으로 못 잡는 것을 겨냥한다 — 모든 row의 서브트리가 같으면 "구간
//! 크기를 엉뚱한 데서 계산하는" 결함이 상쇄되어 보이지 않는다.
//!
//! 픽스처(`demo/nested-list-derived.tsx`):
//!   outer = ["a", "bb", "ccc"], inner = ["p","q","r","s"]
//!   조건부 = `open() && it.length > 1`  → "a"는 절대 열리지 않는다
//!   안쪽 리스트 = `inner().slice(0, it.length)` → "bb"는 2개, "ccc"는 3개
//!
//! 단언은 파서 없이 **row HTML을 통째로** 비교한다 — 구간 경계를 세는 보조
//! 코드가 틀리면 테스트가 조용히 무력해지므로, 기대값을 눈으로 읽을 수 있게
//! 짓는 쪽을 택했다.

use loom_nested_fixture::{generated_derived as g, TreeDom};
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

fn outer_append() -> g::Action { g::Action::Action0("dddd".to_string()) }
const OUTER_ROTATE: g::Action = g::Action::Action1;
const TOGGLE: g::Action = g::Action::Action2;
fn inner_append() -> g::Action { g::Action::Action3("t".to_string()) }

/// `<ul class="outer">` 의 자식들만.
fn rows(dom: &TreeDom, mount: loom_runtime::NodeHandle) -> String {
    let html = dom.html(mount);
    let start = html.find("<ul class=\"outer\"").expect("outer 리스트가 없다");
    let open = html[start..].find('>').unwrap() + start + 1;
    // 바깥 `</ul>`은 **마지막** 것이다 — 안쪽 `<ul class="inner">`이 먼저 닫히므로
    // 앞에서 찾으면 첫 row에서 잘린다(실제로 그렇게 틀렸다).
    let end = html.rfind("</ul>").expect("outer 리스트가 안 닫혔다");
    html[open..end].to_string()
}

/// 기대하는 row 하나. `inner`가 비면 조건부가 열리지 않은 row다.
fn row(item: &str, inner: &[&str], hidden: bool) -> String {
    let panel = if hidden { "<div class=\"panel\" hidden>" } else { "<div class=\"panel\">" };
    if inner.is_empty() {
        format!("<li>{item}{panel}</div></li>")
    } else {
        let lis: String = inner.iter().map(|x| format!("<li>{x}</li>")).collect();
        format!("<li>{item}{panel}<ul class=\"inner\">{lis}</ul></div></li>")
    }
}

#[test]
fn closed_start_gives_every_row_an_empty_panel() {
    let (_app, dom, _s, mount) = boot();
    assert_eq!(
        rows(&dom, mount),
        format!("{}{}{}", row("a", &[], false), row("bb", &[], false), row("ccc", &[], false))
    );
}

#[test]
fn opening_gives_each_row_a_different_span() {
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    // "a"(길이 1)는 술어가 거짓이라 열리지 않는다 — 구간 0.
    // "bb"는 2, "ccc"는 3. **균일하지 않다**는 것이 이 픽스처의 존재 이유다.
    assert_eq!(
        rows(&dom, mount),
        format!(
            "{}{}{}",
            row("a", &[], false),
            row("bb", &["p", "q"], false),
            row("ccc", &["p", "q", "r"], false)
        )
    );
}

#[test]
fn the_shared_inner_signal_still_drives_every_open_row() {
    // 파생이 조작을 잡아먹지 않는지 — §5는 내부 리스트 ops가 **독립적**이길
    // 요구한다. 길이는 아이템에 묶이되 내용은 공유 signal에서 온다.
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, inner_append());
    assert_eq!(
        rows(&dom, mount),
        format!(
            "{}{}{}",
            row("a", &[], false),
            row("bb", &["p", "q"], false),
            row("ccc", &["p", "q", "r"], false)
        )
    );
}

#[test]
fn a_longer_row_added_while_open_gets_a_bigger_span() {
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, outer_append()); // "dddd" — 길이 4
    assert_eq!(
        rows(&dom, mount),
        format!(
            "{}{}{}{}",
            row("a", &[], false),
            row("bb", &["p", "q"], false),
            row("ccc", &["p", "q", "r"], false),
            row("dddd", &["p", "q", "r", "s"], false)
        )
    );
}

#[test]
fn rotating_carries_each_rows_own_span_with_it() {
    // 구간 크기가 row마다 다르므로, 이동이 구간을 잘못 계산하면 여기서 순서나
    // 크기가 어긋난다 — 균일 판본에서는 이 단언이 성립해도 판별력이 없다.
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, OUTER_ROTATE); // ["bb","ccc","a"]
    assert_eq!(
        rows(&dom, mount),
        format!(
            "{}{}{}",
            row("bb", &["p", "q"], false),
            row("ccc", &["p", "q", "r"], false),
            row("a", &[], false)
        )
    );
}

#[test]
fn closing_hides_only_the_rows_that_were_actually_open() {
    let (mut app, mut dom, s, mount) = boot();
    act(&mut app, &mut dom, &s, TOGGLE);
    act(&mut app, &mut dom, &s, TOGGLE);
    // "a"는 열린 적이 없으므로 숨길 것도 없다 — 술어가 거짓인 row의 조건부
    // signal은 애초에 변한 적이 없다(그래서 `SetVisible`도 안 나간다).
    assert_eq!(
        rows(&dom, mount),
        format!(
            "{}{}{}",
            row("a", &[], false),
            row("bb", &["p", "q"], true),
            row("ccc", &["p", "q", "r"], true)
        )
    );
    act(&mut app, &mut dom, &s, TOGGLE);
    assert_eq!(
        rows(&dom, mount),
        format!(
            "{}{}{}",
            row("a", &[], false),
            row("bb", &["p", "q"], false),
            row("ccc", &["p", "q", "r"], false)
        )
    );
}
