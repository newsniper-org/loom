//! 컴파일러가 낸 중첩 조건부 코드를 **실제로 돌려** 결과 트리를 비교한다.
//!
//! `loom-compiler`의 단위 테스트가 잡지 못하는 것을 잡는 것이 목적이다:
//! 문자열은 그럴듯한데 `DomTarget`이 붙는 시점과 `Changed`가 엇갈려 내용이
//! 영영 안 그려지는 부류의 결함(이 구현에서 실제로 설계 단계에 걸렸던 것).

use loom_nested_fixture::{generated, TreeDom};
use loom_runtime::LoomApp;

/// setup → 첫 tick 까지. 조건부는 `false`로 시작하므로 wrapper는 비어 있다.
fn boot() -> (LoomApp, TreeDom, generated::Signals, loom_runtime::NodeHandle) {
    let (mut dom, mount) = TreeDom::new();
    let mut app = LoomApp::new();
    let (signals, _actions, _input_actions) = generated::setup(&mut app, &mut dom, mount);
    app.tick(&mut dom);
    (app, dom, signals, mount)
}

fn act(app: &mut LoomApp, dom: &mut TreeDom, signals: &generated::Signals, action: generated::Action) {
    generated::apply_action(app, signals, &action);
    app.tick(dom);
}

/// 이 픽스처의 액션 이름은 codegen이 `Action0`/`Action1`/… 로 매긴다
/// (`collect_actions`의 트리 순회 순서 = toggle, +1, append).
const TOGGLE: generated::Action = generated::Action::Action0;
const INCREMENT: generated::Action = generated::Action::Action1;
/// `items.append("new")` — 리터럴 인자를 variant가 들고 다닌다(codegen이
/// 그렇게 낸다, 위 `apply_action` 참고).
fn append() -> generated::Action {
    generated::Action::Action2("new".to_string())
}

#[test]
fn hidden_conditional_renders_no_nested_content() {
    let (_app, dom, _signals, mount) = boot();
    let html = dom.html(mount);
    // wrapper는 eager하게 만들어지지만 비어 있어야 한다 — 안쪽 리스트가
    // 조건부와 무관하게 먼저 그려지면 여기서 걸린다(제네릭 K에 초기 렌더를
    // 맡겼을 때 실제로 그렇게 된다).
    assert!(
        html.contains(r#"<section class="panel" data-loom-id="open"></section>"#),
        "{html}"
    );
    assert!(!html.contains("<li>"), "숨겨진 조건부의 리스트가 그려졌다: {html}");
}

#[test]
fn opening_the_conditional_renders_the_whole_nested_subtree_at_once() {
    let (mut app, mut dom, signals, mount) = boot();
    act(&mut app, &mut dom, &signals, TOGGLE);
    let html = dom.html(mount);

    // 정적 부분은 컴파일타임에 접힌 HTML 한 덩어리로,
    assert!(html.contains("<h3>details</h3>"), "{html}");
    // 텍스트 binding은 현재 값이 **생성 시점에 구워져** 나와야 한다
    // (`Changed`를 기다리면 이 틱에는 비어 있게 된다).
    assert!(html.contains(r#"<p class="count" data-loom-id="count">0</p>"#), "{html}");
    // 그리고 안쪽 리스트의 초기 row 전부.
    assert!(
        html.contains(r#"<ul class="inner" data-loom-id="items"><li>a</li><li>b</li><li>c</li></ul>"#),
        "{html}"
    );
}

#[test]
fn nested_text_binding_patches_normally_once_shown() {
    let (mut app, mut dom, signals, mount) = boot();
    act(&mut app, &mut dom, &signals, TOGGLE);
    act(&mut app, &mut dom, &signals, INCREMENT);
    let html = dom.html(mount);
    assert!(html.contains(r#"<p class="count" data-loom-id="count">1</p>"#), "{html}");
}

#[test]
fn nested_list_reconciles_normally_once_shown() {
    let (mut app, mut dom, signals, mount) = boot();
    act(&mut app, &mut dom, &signals, TOGGLE);
    act(&mut app, &mut dom, &signals, append());
    let html = dom.html(mount);
    // 제네릭 `list_reconcile_k_system`이 이어받아야 한다 — 사이트 함수가
    // 시드한 `ListRows`가 그 System의 despawn 대상과 맞물리지 않으면 여기서
    // row가 중복되거나 사라진다.
    assert!(
        html.contains(r#"<ul class="inner" data-loom-id="items"><li>a</li><li>b</li><li>c</li><li>new</li></ul>"#),
        "{html}"
    );
}

#[test]
fn mutating_a_nested_signal_while_hidden_shows_the_current_value_when_opened() {
    // 숨어 있는 동안 값을 바꿔도, 열릴 때는 **그때의 값**이 나와야 한다.
    // (`DomTarget`이 없어서 patch System이 지나쳐 간 구간이므로, 초기값을
    // 생성 시점에 읽는 설계가 실제로 이걸 보장하는지 확인.)
    let (mut app, mut dom, signals, mount) = boot();
    act(&mut app, &mut dom, &signals, INCREMENT);
    act(&mut app, &mut dom, &signals, INCREMENT);
    assert!(!dom.html(mount).contains("class=\"count\" data-loom-id=\"count\">"), "아직 열리지 않았다");
    act(&mut app, &mut dom, &signals, TOGGLE);
    let html = dom.html(mount);
    assert!(html.contains(r#"<p class="count" data-loom-id="count">2</p>"#), "{html}");
}

#[test]
fn reclosing_hides_the_subtree_without_rebuilding_it() {
    let (mut app, mut dom, signals, mount) = boot();
    act(&mut app, &mut dom, &signals, TOGGLE); // 열림 — 내용 생성
    act(&mut app, &mut dom, &signals, TOGGLE); // 닫힘 — SetVisible만
    let html = dom.html(mount);
    assert!(html.contains("<section class=\"panel\" data-loom-id=\"open\" hidden>"), "{html}");
    // 내용은 살아 있어야 한다(재생성이 아니라 숨김) — 두 번째 열기에서
    // 중복 생성이 일어나면 아래 재-열기 단언이 깨진다.
    assert!(html.contains("<li>a</li>"), "{html}");

    act(&mut app, &mut dom, &signals, TOGGLE); // 다시 열림
    let html = dom.html(mount);
    assert_eq!(html.matches("<li>a</li>").count(), 1, "재생성으로 row가 중복됐다: {html}");
    assert!(!html.contains(" hidden>"), "{html}");
}

// ---------------------------------------------------------------------------
// resume — SSR된 DOM에 다시 붙는 경로. 조건부 안의 binding은 **SSR 시점에
// 보였는지**에 따라 두 갈래로 갈린다(codegen의 `gate` 처리). 문자열 단언만
// 있던 부분이라 실제로 돌려서 확인한다.
// ---------------------------------------------------------------------------

/// SSR 흉내: `setup()`으로 트리를 만들고 `toggle_first`만큼 토글한 뒤, 그
/// DOM 위에 **새 `LoomApp`**으로 `resume()`한다. 브라우저에서 일어나는 일과
/// 같은 순서다(SSR HTML → 청크 로드 → resume).
fn ssr_then_resume(toggle_first: bool) -> (LoomApp, TreeDom, generated::Signals, loom_runtime::NodeHandle) {
    let (mut app, mut dom, signals, mount) = boot();
    if toggle_first {
        act(&mut app, &mut dom, &signals, TOGGLE);
    }
    drop((app, signals));

    // `find_node`는 호스트 글루가 준다. 여기서는 SSR 직후 DOM에 실제로 존재하는
    // `data-loom-id`만 미리 훑어 표로 만든다 — 없는 id를 찾으면 패닉이고, 그것이
    // 곧 "숨겨진 조건부의 안쪽 노드를 찾으려 들었다"는 결함의 검출이다.
    // (표로 뜨는 이유는 단순히 `resume`이 `dom`을 가변으로 빌려 가기 때문.)
    let ids = dom.loom_id_table();
    let mut resumed = LoomApp::new();
    let signals = generated::resume(
        &mut resumed,
        &mut dom,
        |id| {
            *ids.get(id).unwrap_or_else(|| {
                panic!("resume이 SSR DOM에 없는 `data-loom-id=\"{id}\"`를 찾았다")
            })
        },
        &[],
    );
    (resumed, dom, signals, mount)
}

#[test]
fn resuming_a_hidden_conditional_never_looks_up_its_nested_nodes() {
    // 숨겨진 채 SSR됐다면 `count`/`items` 노드는 **존재하지 않는다**.
    // `find_node`를 부르면 패닉 — 이 테스트가 통과한다는 것이 곧 안 불렀다는 뜻.
    let (mut app, mut dom, signals, mount) = ssr_then_resume(false);
    assert!(!dom.html(mount).contains("<li>"), "{}", dom.html(mount));

    // 그리고 resume 이후에 처음 열어도 정상적으로 전체 서브트리가 생겨야 한다.
    act(&mut app, &mut dom, &signals, TOGGLE);
    let html = dom.html(mount);
    assert!(html.contains(r#"<p class="count" data-loom-id="count">0</p>"#), "{html}");
    assert!(
        html.contains(r#"<ul class="inner" data-loom-id="items"><li>a</li><li>b</li><li>c</li></ul>"#),
        "{html}"
    );
}

#[test]
fn resuming_a_shown_conditional_reattaches_its_nested_bindings() {
    let (mut app, mut dom, signals, mount) = ssr_then_resume(true);
    let html = dom.html(mount);
    // SSR이 남긴 row들이 그대로 살아 있어야 한다(재생성이 아니라 인수).
    assert!(html.contains("<li>a</li><li>b</li><li>c</li>"), "{html}");

    // 인수한 노드에 실제로 패치가 꽂히는지 — `DomTarget`이 SSR 노드를
    // 가리키지 않으면 값이 엉뚱한 데로 가거나 아무 데도 안 간다.
    act(&mut app, &mut dom, &signals, INCREMENT);
    let html = dom.html(mount);
    assert!(html.contains(r#"<p class="count" data-loom-id="count">1</p>"#), "{html}");

    act(&mut app, &mut dom, &signals, append());
    let html = dom.html(mount);
    assert!(html.contains("<li>a</li><li>b</li><li>c</li><li>new</li>"), "{html}");
}
