//! codegen 앞의 사전 검사 (Phase 7a C3, 2026-10-17 신설).
//!
//! **무엇을 하는가.** codegen.rs에는 사용자가 실제로 도달할 수 있는 `panic!`
//! 거부가 여럿 있다 — 모드에 따라 달라지는 것(Flatten signal 이름 유일성,
//! Islands의 미참조 헬퍼·`<input>`, 단일 Island 대상 이름, 완전 정적 Island)과
//! 노드 모양에 대한 것(혼합 자식, 조건부 안/row 템플릿 안의 미지원 노드 등).
//! 이 모듈은 그 판정을 **같은 순회 순서로 복제**해 codegen이 패닉하기 전에
//! `Diagnostic`(문제 구문의 span 포함)으로 돌려준다.
//!
//! **codegen은 바꾸지 않는다**(design-7a §3.3). codegen의 패닉은 내부
//! 백스톱으로 그대로 남는다. 그래서 codegen의 Ok 경로 소스가 한 글자도 바뀌지
//! 않고, 방출 바이트는 구조적으로 보존된다(게이트 A는 확인 역할).
//!
//! **복제 표류 방지.** lib.rs의 `check_backstop` 테스트가 codegen의 사용자 도달
//! 패닉마다 (1) 같은 입력을 codegen에 직접 넣으면 그 패닉이 실제로 나는지(존재
//! 단언 — 입력이 정말 그 패닉에 도달하는지), (2) 공개 API는 패닉 없이 이
//! 모듈의 진단을 돌려주는지를 함께 단언한다. codegen에 새 사용자 도달 거부를
//! 더하면 이 모듈과 그 테스트에 행을 더해야 한다(docs/02 진단 절의 규칙).
//!
//! **이 모듈이 다루지 않는 것(숨기지 않음).**
//! - 7a가 새로 더하는 거부 중 G01·R46(c)는 C7에서, R15·R16·R12 리스트는 C8에서
//!   더했다(아래). 그 밖에 여기 있는 것은 7a 이전부터 codegen이 패닉으로 거부하던
//!   것뿐이며, 판정 범위를 넓히지 않았다(넓히면 지금 컴파일되는 입력이 거부된다).
//! - codegen.rs:1271 "signal `…` was never spawned"(선언만 하고 표시하지 않은
//!   signal)도 사용자가 도달하지만, design-7a §1.1 R07U로 C7에서 lower 그래프
//!   검사가 막는다(lower.rs `check_binding_sites`).
//! - codegen.rs:620-627(Islands의 미참조 헬퍼)은 C7부터 lower의 R47(모드 무관
//!   도달성 검사)이 먼저 막는다. 그래서 C3에서 여기 복제했던 같은 판정은 걷었다
//!   — 루트에서 도달할 수 있는 헬퍼는 반드시 어떤 `ComponentRef`가 참조하므로
//!   `activation_for`가 `None`일 수 없다.
//!
//! **C7(2026-10-17)에서 더한 7a 거부.**
//! - `LOOM-G01`: 단일 컴포넌트 경로(`compile_tsx_to_rust`/`_with_candidate`)의
//!   루트에 `ComponentRef`가 있으면 오류다. codegen은 이것을 받아
//!   `super::{c}_generated::…`를 부르는 코드를 내지만 그 모듈은 생성되지 않아
//!   rustc 단계에서야 실패했다.
//! - `LOOM-R46`(c): Islands·단일 Island 경로에서 루트가 아닌 두 컴포넌트 이름이
//!   codegen의 `sanitize_mod_name`을 거쳐 같은 모듈 이름이 되면 오류다(예:
//!   `ProductCard`와 `Product_card` → `product_card`). design-7a는 "check.rs에서
//!   같은 함수를 호출"이라 했지만, 그 함수는 codegen.rs의 private 함수라 부르려면
//!   codegen.rs의 가시성을 바꿔야 한다(C3의 "codegen 소스 무변경"이 깨진다). 그래서
//!   `mod_name`으로 **같은 규칙을 다시 구현**했고, 표류는 lib.rs의
//!   `c7_graph_rejections` 테스트가 codegen이 실제로 방출한 `pub mod …_generated`
//!   이름과 대조해 막는다.
//!
//! **C8(2026-10-17)에서 더한 7a 거부.**
//! - `LOOM-R15`: 단일 Island 경로(`compile_tsx_to_single_island_rust`)의 대상
//!   컴포넌트에 `<input>` 양방향 binding이 있으면 오류다. codegen의
//!   `generate_single_island_client`는 이 검사가 없어 click 리스너만 연결한
//!   진입점을 냈다(값이 `""`로 고정되고, 클릭하면 지워진다). 찾기는
//!   `first_input_binding` — List/Cond 안까지 **모든 노드**를 내려간다(codegen의
//!   `collect_actions`는 그 안으로 내려가지 않으므로 재사용하지 않는다).
//!   다중 Islands의 같은 거부(codegen.rs:630-638, C3에서 이 모듈로 복제, 코드
//!   `LOOM-L00`·문구 유지)도 같은 순회기로 바꿨다. List/Cond 안의 `<input>`은
//!   어차피 뒤의 노드 모양 검사(2031·2290·2325)가 `L00`으로 거부하던 입력이라,
//!   받아 주는 입력은 늘거나 줄지 않고 보고되는 문구만 원인(`<input>`)에 맞춰진다.
//! - `LOOM-R16`: Islands·단일 Island 경로에서 같은 컴포넌트를 가리키는
//!   `ComponentRef`가 그래프 전체에 2개 이상이면 오류다(island id·chunk가 이름에서
//!   파생되어 충돌하고, `ComponentGraph::activation_for`의 `find_map`이 첫 참조의
//!   디렉티브를 둘째에도 쓴다). 집계는 컴포넌트 선언 순 × pre-order의 `Vec`이다
//!   (§3.4). **Flatten에는 더하지 않는다**: Flatten은 참조를 펼치므로 island id가
//!   없고, signal이 있는 컴포넌트의 중복 참조는 이미 552(이름 유일성)가
//!   거부하며, signal 없는 컴포넌트의 중복 참조는 의미상 무해하다.
//! - `LOOM-R12`(리스트): 조건부 `{c() && <tag>…</tag>}`의 then 엘리먼트 자식이
//!   리스트 binding 하나뿐이면(직계 리스트) 오류다(모드 무관). codegen의
//!   CondBinding arm(`[JsxNode::ListBinding { .. }]`)은 wrapper를 곧 리스트
//!   노드로 써서 resume에서 같은 DOM 노드를 Bool 패스와 리스트 패스가 두 번
//!   adopt하고, SSR 시점에 열렸는데 리스트가 비면 열림 상태를 거짓으로
//!   복원한다(design-7a §1.1 R12 리스트). 직계 **텍스트**(`[TextBinding]`)는 C9가
//!   구현으로 고친다.
//! - lower가 이미 막아 codegen에서 도달할 수 없는 거부(signal 종류 검사 2017·
//!   2820·3059·3295·3000, `unknown signal` 조회 등)와 앞선 검사에 가려지는
//!   거부(2172는 2023에, 2481은 2290에)는 복제하지 않는다.
//!
//! 진단 코드는 lower의 7a 이전 거부와 같은 `LOOM-L00`(지원 범위 밖)이다. 문구는
//! codegen 패닉 문구를 유지하되, 앞의 `loomc: ` 접두와 끝의 AST `Debug` 덤프는
//! 뺐다(C2와 같은 규칙 — span 강조가 해당 소스를 이미 보여 준다).
//!
//! 결정성(§3.4): `HashMap`은 노드 → span **조회 전용**이다. 순회는 모두 IR의
//! `Vec` 순서를 따른다.

use std::collections::HashMap;

use swc_common::{Span, DUMMY_SP};

use crate::codegen::GraphMode;
use crate::diag::Diagnostic;
use crate::ir::{Component, ComponentGraph, IslandActivation, JsxNode, SignalDecl, SignalInit};
use crate::lower::{NodeSpan, SpanTable, L00, R46};

/// 단일 컴포넌트 경로의 컴포넌트 참조(7a C7).
pub(crate) const G01: &str = "LOOM-G01";
/// 단일 Island 대상의 `<input>` 양방향 binding(7a C8).
pub(crate) const R15: &str = "LOOM-R15";
/// Islands·단일 Island에서 같은 컴포넌트의 중복 참조(7a C8).
pub(crate) const R16: &str = "LOOM-R16";
/// 조건부의 직계 리스트 binding(7a C8).
pub(crate) const R12: &str = "LOOM-R12";

/// codegen.rs `sanitize_mod_name`과 **같은 규칙**(PascalCase → snake_case:
/// 둘째 글자부터 대문자 앞에 `_`, 대문자는 소문자로). 위 모듈 문서의 R46(c) 참고.
pub(crate) fn mod_name(component_name: &str) -> String {
    let mut out = String::new();
    for (i, c) in component_name.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

type CResult = Result<(), Diagnostic>;

/// 어떤 공개 API가 codegen의 어느 진입점을 부르는가.
pub(crate) enum Target<'a> {
    /// `compile_tsx_to_rust`/`_with_candidate` — `codegen::generate*`로 루트
    /// 컴포넌트 하나만 생성한다(헬퍼는 생성되지 않는다).
    RootOnly,
    /// `compile_tsx_to_rust_graph`.
    Graph(&'a GraphMode),
    /// `compile_tsx_to_single_island_rust`.
    SingleIsland(&'a str),
}

pub(crate) fn check(graph: &ComponentGraph, spans: &SpanTable, target: Target<'_>) -> CResult {
    let flatten = matches!(target, Target::Graph(GraphMode::Flatten));
    let mut cx = Cx::new(graph, spans, flatten);
    match target {
        Target::RootOnly => {
            let root = graph.root_component();
            // G01: 이 경로는 루트 하나만 생성한다 — 참조된 컴포넌트의 모듈이 없다.
            let mut order = Vec::new();
            preorder(&root.root, &mut order);
            if let Some(r @ JsxNode::ComponentRef { component, .. }) =
                order.into_iter().find(|n| matches!(n, JsxNode::ComponentRef { .. }))
            {
                return Err(Diagnostic::error(
                    G01,
                    cx.sp(r),
                    format!(
                        "단일 컴포넌트 경로는 컴포넌트 참조를 컴파일하지 않습니다(`<{component}>`). `compile_tsx_to_rust_graph`를 쓰십시오"
                    ),
                ));
            }
            cx.component(root, false)
        }
        Target::Graph(GraphMode::Flatten) => {
            // codegen의 `flatten_graph`가 생성 전에 그래프 전체를 한 번 펼친다 —
            // 같은 순서로 이름 유일성(552)을 먼저 본다.
            let root = graph.root_component();
            let mut sigs: Vec<&SignalDecl> = root.signals.iter().collect();
            let mut stack = vec![root.name.as_str()];
            cx.flatten_pass(&root.root, &mut stack, &mut sigs)?;
            cx.flat_signals = Some(sigs);
            cx.component(root, false)
        }
        Target::Graph(GraphMode::Islands) => {
            cx.mod_name_collision()?;
            cx.duplicate_reference()?;
            // codegen의 `generate_islands`와 같은 순서: 컴포넌트 선언 순으로,
            // 루트는 Island 아님, 나머지는 (625 — C7부터 lower의 R47이 흡수) →
            // 634 → 생성.
            for (i, component) in graph.components.iter().enumerate() {
                if component.name == graph.root {
                    cx.component(component, false)?;
                    continue;
                }
                if let Some(input) = first_input_binding(&component.root) {
                    return Err(Diagnostic::error(
                        L00,
                        cx.sp(input),
                        format!(
                            "component `{}` is Island-activated but contains an <input> two-way binding — 증분 4 scope: compiled Island entry points wire `click` listeners only (see generate_island_entry_body's doc comment); not yet supported",
                            component.name
                        ),
                    )
                    .with_label(cx.component_span(i), "이 Island 컴포넌트"));
                }
                cx.component(component, true)?;
            }
            Ok(())
        }
        Target::SingleIsland(name) => {
            cx.mod_name_collision()?;
            cx.duplicate_reference()?;
            let Some((i, component)) = graph.components.iter().enumerate().find(|(_, c)| c.name == name) else {
                // 소스 안의 위치가 아니라 호출 인자(build.rs의 대상 이름) 오류다.
                return Err(Diagnostic::error(
                    L00,
                    DUMMY_SP,
                    format!("generate_single_island_client: no component named `{name}` in this graph"),
                ));
            };
            if graph.activation_for(&component.name).is_none() {
                return Err(Diagnostic::error(
                    L00,
                    cx.component_span(i),
                    format!("generate_single_island_client: component `{name}` is not Island-activated"),
                ));
            }
            // R15: codegen의 단일 Island 경로에는 다중 Islands(634)의 input 가드가 없다.
            if let Some(input) = first_input_binding(&component.root) {
                return Err(Diagnostic::error(
                    R15,
                    cx.sp(input),
                    format!(
                        "Island `{name}`를 단일 Island 클라이언트로 컴파일할 때 <input> 양방향 binding은 지원하지 않습니다. 진입점이 click만 연결해 값이 \"\"로 고정되고, 클릭하면 지워집니다"
                    ),
                )
                .with_label(cx.component_span(i), "이 Island 컴포넌트"));
            }
            cx.component(component, true)
        }
    }
}

struct Cx<'g> {
    graph: &'g ComponentGraph,
    spans: &'g SpanTable,
    /// IR 노드 주소 → 소스 위치(조회 전용).
    at: HashMap<*const JsxNode, NodeSpan>,
    flatten: bool,
    /// Flatten 모드에서 codegen이 쓰는 합쳐진 signal 목록(루트 + 펼친 순서).
    flat_signals: Option<Vec<&'g SignalDecl>>,
}

impl<'g> Cx<'g> {
    fn new(graph: &'g ComponentGraph, spans: &'g SpanTable, flatten: bool) -> Self {
        assert_eq!(
            graph.components.len(),
            spans.nodes.len(),
            "loomc internal error: SpanTable.nodes is not aligned with ComponentGraph.components"
        );
        let mut at = HashMap::new();
        for (component, list) in graph.components.iter().zip(&spans.nodes) {
            let mut order = Vec::new();
            preorder(&component.root, &mut order);
            assert_eq!(
                order.len(),
                list.len(),
                "loomc internal error: lower recorded {} node spans for `{}` but its IR has {} nodes",
                list.len(),
                component.name,
                order.len()
            );
            for (node, span) in order.into_iter().zip(list) {
                at.insert(node as *const JsxNode, *span);
            }
        }
        Cx { graph, spans, at, flatten, flat_signals: None }
    }

    fn node_span(&self, node: &JsxNode) -> NodeSpan {
        *self
            .at
            .get(&(node as *const JsxNode))
            .expect("loomc internal error: IR node without a recorded span")
    }

    fn sp(&self, node: &JsxNode) -> Span {
        self.node_span(node).span
    }

    /// `Element`의 `onClick` attr span(없으면 태그 이름).
    fn event_sp(&self, node: &JsxNode) -> Span {
        let s = self.node_span(node);
        s.event.unwrap_or(s.span)
    }

    fn component_span(&self, index: usize) -> Span {
        self.spans.components.get(index).map(|(_, s)| *s).unwrap_or(DUMMY_SP)
    }

    /// R46(c): 루트가 아닌 컴포넌트들의 모듈 이름 충돌(선언 순으로 첫 충돌).
    fn mod_name_collision(&self) -> CResult {
        let named: Vec<(usize, &'g Component, String)> = self
            .graph
            .components
            .iter()
            .enumerate()
            .filter(|(_, c)| c.name != self.graph.root)
            .map(|(i, c)| (i, c, mod_name(&c.name)))
            .collect();
        for (k, (i, c, m)) in named.iter().enumerate() {
            if let Some((j, first, _)) = named[..k].iter().find(|(_, _, m0)| m0 == m) {
                return Err(Diagnostic::error(
                    R46,
                    self.component_span(*i),
                    format!("`{}`와 `{}`가 같은 모듈 이름 `{m}`으로 변환됩니다", first.name, c.name),
                )
                .with_label(self.component_span(*j), format!("`{}`도 `{m}`", first.name)));
            }
        }
        Ok(())
    }

    /// R16: 같은 컴포넌트를 가리키는 `ComponentRef`가 둘 이상(선언 순 × pre-order로
    /// 첫 중복). 주 span은 둘째 참조, 두 사이트 모두에 label.
    fn duplicate_reference(&self) -> CResult {
        let mut refs: Vec<(&'g str, IslandActivation, &'g JsxNode)> = Vec::new();
        for component in &self.graph.components {
            let mut order = Vec::new();
            preorder(&component.root, &mut order);
            for node in order {
                if let JsxNode::ComponentRef { component: target, activation, .. } = node {
                    if let Some((_, first_act, first)) = refs.iter().find(|(c, _, _)| *c == target.as_str()) {
                        return Err(Diagnostic::error(
                            R16,
                            self.sp(node),
                            format!(
                                "컴포넌트 `{target}`가 두 번 참조됐습니다. Islands에서는 둘째 참조가 첫 참조의 디렉티브·island id를 공유합니다"
                            ),
                        )
                        .with_label(self.sp(first), format!("첫 참조(client:{})", first_act.as_attr_value()))
                        .with_label(self.sp(node), format!("둘째 참조(client:{})", activation.as_attr_value())));
                    }
                    refs.push((target.as_str(), *activation, node));
                }
            }
        }
        Ok(())
    }

    fn find_component(&self, name: &str) -> &'g Component {
        self.graph
            .components
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("loomc internal error: ComponentRef names unknown component `{name}`"))
    }

    /// Flatten 모드에서는 `ComponentRef`가 대상 루트로 치환된다(codegen의
    /// `flatten_node`). 다른 모드에서는 그대로 둔다. 순환은 `flatten_pass`가
    /// 먼저 거부하므로 여기서는 생기지 않는다(그래도 횟수로 막아 둔다).
    fn res(&self, mut node: &'g JsxNode) -> &'g JsxNode {
        if !self.flatten {
            return node;
        }
        for _ in 0..=self.graph.components.len() {
            match node {
                JsxNode::ComponentRef { component, .. } => node = &self.find_component(component).root,
                _ => return node,
            }
        }
        panic!("loomc internal error: ComponentRef cycle survived flatten_pass")
    }

    fn signal(&self, owner: &'g Component, name: &str) -> Option<&'g SignalDecl> {
        match &self.flat_signals {
            Some(sigs) => sigs.iter().copied().find(|s| s.name == name),
            None => owner.signals.iter().find(|s| s.name == name),
        }
    }

    /// codegen의 `try_render_fully_static`과 같은 판정(Flatten에서는 펼친 뒤).
    fn fully_static(&self, node: &'g JsxNode) -> bool {
        match self.res(node) {
            JsxNode::StaticText(_) => true,
            JsxNode::Element { action: None, children, .. } => children.iter().all(|c| self.fully_static(c)),
            _ => false,
        }
    }

    fn is_dynamic_binding(&self, node: &'g JsxNode) -> bool {
        matches!(self.res(node), JsxNode::TextBinding { .. } | JsxNode::ListBinding { .. })
    }

    fn sole_dynamic_binding(&self, children: &'g [JsxNode]) -> bool {
        matches!(children, [only] if self.is_dynamic_binding(only))
    }

    fn row_item_only(&self, children: &'g [JsxNode]) -> bool {
        matches!(children, [only] if matches!(self.res(only), JsxNode::RowItemText))
    }

    // ---- codegen.rs:552 (`flatten_node`) ----

    fn flatten_pass(
        &self,
        node: &'g JsxNode,
        stack: &mut Vec<&'g str>,
        sigs: &mut Vec<&'g SignalDecl>,
    ) -> CResult {
        match node {
            JsxNode::Element { children, .. } => {
                for c in children {
                    self.flatten_pass(c, stack, sigs)?;
                }
                Ok(())
            }
            JsxNode::ListBinding { row, .. } => self.flatten_pass(row, stack, sigs),
            JsxNode::CondBinding { then_branch, .. } => self.flatten_pass(then_branch, stack, sigs),
            JsxNode::ComponentRef { component: ref_name, .. } => {
                let target = self.find_component(ref_name);
                // codegen의 `flatten_node`는 순환을 만나면 끝없이 재귀한다(스택
                // 넘침으로 프로세스가 죽는다 — 패닉도 아니다). 검사가 끝나야
                // 하므로 여기서 거부한다. 신호가 있는 순환은 아래 552에 먼저
                // 걸리던 것과 같은 입력이다.
                if stack.contains(&target.name.as_str()) {
                    return Err(Diagnostic::error(
                        L00,
                        self.sp(node),
                        format!(
                            "GraphMode::Flatten cannot inline a component reference cycle (`{}` → … → `{}`)",
                            stack.join("` → `"),
                            target.name
                        ),
                    ));
                }
                for s in &target.signals {
                    if sigs.iter().any(|existing| existing.name == s.name) {
                        let mut d = Diagnostic::error(
                            L00,
                            self.sp(node),
                            format!(
                                "GraphMode::Flatten requires every signal name to be globally unique across the whole ComponentGraph (found a second declaration of `{}`, referenced via component `{}`) — automatic renaming is a stated scope cut, not implemented",
                                s.name, target.name
                            ),
                        );
                        if let Some((_, _, decl)) = self
                            .spans
                            .signals
                            .iter()
                            .find(|(c, n, _)| *c == target.name && *n == s.name)
                        {
                            d = d.with_label(*decl, format!("`{}` 안의 `{}` 선언", target.name, s.name));
                        }
                        return Err(d);
                    }
                }
                sigs.extend(target.signals.iter());
                stack.push(target.name.as_str());
                self.flatten_pass(&target.root, stack, sigs)?;
                stack.pop();
                Ok(())
            }
            JsxNode::StaticText(_)
            | JsxNode::TextBinding { .. }
            | JsxNode::RowItemText
            | JsxNode::InputBinding { .. }
            | JsxNode::SharedValueBinding { .. } => Ok(()),
        }
    }

    // ---- `generate_component_inner` ----

    fn component(&self, component: &'g Component, island: bool) -> CResult {
        // codegen.rs:1196
        if island && self.fully_static(&component.root) {
            let i = self.graph.components.iter().position(|c| std::ptr::eq(c, component)).unwrap_or(0);
            return Err(Diagnostic::error(
                L00,
                self.component_span(i),
                format!(
                    "component `{}` is Island-activated but has no dynamic content at all — a fully static subtree collapses into one `append_static_html` call with no addressable root node to tag with `data-loom-island*` attributes (see try_render_fully_static's doc comment); remove the `client:` directive if this component is meant to be pure static markup",
                    component.name
                ),
            ));
        }
        self.eager(component, &component.root)
    }

    // ---- `lower_node` (eager setup 경로) ----

    fn eager(&self, owner: &'g Component, node: &'g JsxNode) -> CResult {
        let node = self.res(node);
        if matches!(node, JsxNode::Element { .. }) && self.fully_static(node) {
            return Ok(());
        }
        match node {
            JsxNode::TextBinding { signal } => self.text_is_i32(owner, node, signal),
            JsxNode::ListBinding { signal, row, .. } => {
                // codegen.rs:2811
                let row = self.res(row);
                let JsxNode::Element { children, .. } = row else {
                    return Err(self.row_not_element(row));
                };
                if self.row_item_only(children) {
                    return Ok(());
                }
                self.rich_row(signal, row, children)
            }
            JsxNode::CondBinding { cond, then_branch, .. } => self.cond_site(owner, node, cond, then_branch),
            JsxNode::Element { tag, children, .. } => {
                // codegen.rs:3336
                if children.len() > 1 {
                    if let Some(dynamic) = children.iter().find(|c| self.is_dynamic_binding(c)) {
                        return Err(Diagnostic::error(
                            L00,
                            self.sp(node),
                            format!(
                                "<{tag}> mixes a dynamic binding with other children — Phase 1 requires an element's content to be either all-static or exactly one dynamic binding. Split `{{...}}` into its own wrapping element."
                            ),
                        )
                        .with_label(self.sp(self.res(dynamic)), "동적 binding"));
                    }
                }
                for c in children {
                    self.eager(owner, c)?;
                }
                Ok(())
            }
            JsxNode::StaticText(_)
            | JsxNode::RowItemText
            | JsxNode::InputBinding { .. }
            | JsxNode::ComponentRef { .. }
            | JsxNode::SharedValueBinding { .. } => Ok(()),
        }
    }

    /// codegen.rs:2798(eager)·2003(조건부 안).
    fn text_is_i32(&self, owner: &'g Component, node: &'g JsxNode, signal: &str) -> CResult {
        match self.signal(owner, signal).map(|s| &s.init) {
            Some(SignalInit::I32(_)) | None => Ok(()),
            Some(_) => Err(Diagnostic::error(
                L00,
                self.sp(node),
                format!("`{{{signal}()}}` text binding requires an I32 signal"),
            )),
        }
    }

    /// codegen.rs:2811(eager)·2009(조건부 안).
    fn row_not_element(&self, row: &'g JsxNode) -> Diagnostic {
        Diagnostic::error(L00, self.sp(row), "list row template must be a single element")
    }

    // ---- 조건부 사이트 (`CondBinding` arm + `collect_lazy_slots` + `emit_lazy_subtree`) ----

    fn cond_site(&self, owner: &'g Component, node: &'g JsxNode, cond: &str, then_branch: &'g JsxNode) -> CResult {
        let then_branch = self.res(then_branch);
        // lower가 then 가지를 일반 엘리먼트로 강제한다(codegen.rs:3037은 내부 백스톱).
        let JsxNode::Element { tag, action, children, .. } = then_branch else {
            return Ok(());
        };
        // codegen.rs:3040
        if action.is_some() {
            return Err(Diagnostic::error(
                L00,
                self.event_sp(then_branch),
                format!(
                    "Phase 1 conditional content may not carry its own `onClick` — `{{{cond}() && <{tag} onClick=...>}}` is out of scope (the toggle button must be a separate sibling element, see codegen.rs's CondBinding arm)"
                ),
            )
            .with_label(self.sp(node), "조건부"));
        }
        for c in children {
            self.lazy_collect(owner, cond, c)?;
        }
        // R12(리스트): codegen의 `[JsxNode::ListBinding { .. }]` arm — wrapper가 곧
        // 리스트 노드가 되는 직계형. (직계 텍스트는 C9가 구현으로 고친다.)
        if let [only] = children.as_slice() {
            let only = self.res(only);
            if matches!(only, JsxNode::ListBinding { .. }) {
                return Err(Diagnostic::error(
                    R12,
                    self.sp(then_branch),
                    format!(
                        "조건부 `{{{cond}() && …}}`의 자식이 리스트 하나뿐입니다. 지금은 resume 대상이 어긋납니다. `<div>`로 한 번 감싸십시오(직계 형태 지원은 7b)"
                    ),
                )
                .with_label(self.sp(only), "직계 리스트 binding")
                .with_label(self.sp(node), "조건부"));
            }
        }
        if !self.sole_dynamic_binding(children) {
            for c in children {
                self.lazy_emit(cond, c)?;
            }
        }
        Ok(())
    }

    /// `collect_lazy_slots_node`.
    fn lazy_collect(&self, owner: &'g Component, cond: &str, node: &'g JsxNode) -> CResult {
        let node = self.res(node);
        if matches!(node, JsxNode::Element { .. }) && self.fully_static(node) {
            return Ok(());
        }
        match node {
            JsxNode::StaticText(_) => Ok(()),
            JsxNode::TextBinding { signal } => self.text_is_i32(owner, node, signal),
            JsxNode::ListBinding { row, .. } => {
                let row = self.res(row);
                let JsxNode::Element { children, .. } = row else {
                    return Err(self.row_not_element(row));
                };
                // codegen.rs:2012
                if !self.row_item_only(children) {
                    return Err(Diagnostic::error(
                        L00,
                        self.sp(row),
                        "Phase 1 only supports list rows shaped exactly as `<tag>{item}</tag>` (single text child referencing the row variable)",
                    ));
                }
                Ok(())
            }
            JsxNode::Element { children, action, .. } => {
                // codegen.rs:2023
                if action.is_some() {
                    return Err(self.cond_inner_onclick(cond, node));
                }
                for c in children {
                    self.lazy_collect(owner, cond, c)?;
                }
                Ok(())
            }
            // codegen.rs:2031
            _ => Err(Diagnostic::error(
                L00,
                self.sp(node),
                format!(
                    "`{{{cond}() && ...}}` 안에서 아직 지원하지 않는 노드다 — 지원 범위는 정적 마크업, `{{signal()}}`, `{{signal().map(...)}}`, 그리고 그것들을 감싸는 엘리먼트다"
                ),
            )),
        }
    }

    fn cond_inner_onclick(&self, cond: &str, element: &'g JsxNode) -> Diagnostic {
        Diagnostic::error(
            L00,
            self.event_sp(element),
            format!(
                "`{{{cond}() && ...}}` 안의 엘리먼트는 아직 자기 `onClick`을 가질 수 없다 — 지연 생성된 노드에 LRef 속성을 심는 경로가 아직 없다(codegen.rs의 `emit_lazy_subtree` 참고)"
            ),
        )
    }

    /// `emit_lazy_subtree`. `lazy_collect`가 먼저 통과한 트리에만 불린다.
    fn lazy_emit(&self, cond: &str, node: &'g JsxNode) -> CResult {
        let node = self.res(node);
        if matches!(node, JsxNode::Element { .. }) && self.fully_static(node) {
            return Ok(());
        }
        match node {
            JsxNode::StaticText(_) => Ok(()),
            JsxNode::Element { action, children, .. } => {
                // codegen.rs:2172 — 2023에 가려져 여기서는 나지 않는다(같은 판정).
                if action.is_some() {
                    return Err(self.cond_inner_onclick(cond, node));
                }
                if self.sole_dynamic_binding(children) {
                    return Ok(());
                }
                for c in children {
                    self.lazy_emit(cond, c)?;
                }
                Ok(())
            }
            // codegen.rs:2222 — 실제로 여기 오는 것은 형제가 있는 `{s()}`/
            // `{xs().map(...)}`뿐이다(나머지는 `lazy_collect`가 먼저 거부).
            _ => Err(Diagnostic::error(
                L00,
                self.sp(node),
                format!(
                    "`{{{cond}() && ...}}` 안에서 아직 지원하지 않는 노드다 — 지원 범위는 정적 마크업, `{{signal()}}`, `{{signal().map(...)}}`, 그리고 그것들을 감싸는 엘리먼트다 (`{{signal()}}`/`{{signal().map(...)}}`는 자기 엘리먼트의 유일한 자식이어야 한다)"
                ),
            )),
        }
    }

    // ---- row 템플릿 (`collect_row_mirrors` + `emit_row_child`/`emit_cond_child`) ----

    fn rich_row(&self, signal: &str, row: &'g JsxNode, children: &'g [JsxNode]) -> CResult {
        // codegen.rs:2263
        let rest = match children {
            [first, rest @ ..] if matches!(self.res(first), JsxNode::RowItemText) => rest,
            _ => {
                return Err(Diagnostic::error(
                    L00,
                    self.sp(row),
                    format!(
                        "`{signal}`의 row 템플릿은 `{{item}}`으로 시작해야 한다 — row 루트 엘리먼트가 아이템 값을 담는 텍스트 노드를 갖는다는 것이 후보 C의 값 교체 경로(`SetText`)가 기대하는 모양이다"
                    ),
                ))
            }
        };
        let mut cond_seen: Option<&'g JsxNode> = None;
        let mut list_seen: Option<&'g JsxNode> = None;
        for child in rest {
            let child = self.res(child);
            match child {
                JsxNode::StaticText(_) => {}
                JsxNode::CondBinding { then_branch, .. } => {
                    // codegen.rs:2273
                    if let Some(first) = cond_seen {
                        return Err(Diagnostic::error(
                            L00,
                            self.sp(child),
                            format!("row 템플릿 안의 조건부는 아직 하나만 지원한다 (`{signal}`)"),
                        )
                        .with_label(self.sp(first), "첫 번째 조건부"));
                    }
                    cond_seen = Some(child);
                    let wrapper = self.res(then_branch);
                    let JsxNode::Element { children: inner, .. } = wrapper else {
                        continue; // lower가 보장(codegen.rs:2277은 내부 백스톱)
                    };
                    // codegen.rs:2281
                    if matches!(inner.as_slice(), [only] if matches!(self.res(only), JsxNode::ListBinding { .. } | JsxNode::TextBinding { .. }))
                    {
                        return Err(Diagnostic::error(
                            L00,
                            self.sp(wrapper),
                            format!(
                                "row 안 조건부의 wrapper를 binding 노드로 바로 쓸 수는 없다 (`{signal}`) — 안쪽을 엘리먼트로 한 겹 감쌀 것(예: `{{c() && <div><ul>{{xs().map(...)}}</ul></div>}}`). 이유: row는 런타임에 생기므로 wrapper의 핸들이 방출 시점에 없고, entity로만 지목할 수 있다"
                            ),
                        ));
                    }
                    for n in inner {
                        self.row_mirror(signal, n, &mut list_seen)?;
                    }
                }
                _ if self.fully_static(child) => {}
                // codegen.rs:2290
                _ => {
                    return Err(Diagnostic::error(
                        L00,
                        self.sp(child),
                        format!(
                            "`{signal}`의 row 템플릿에서 아직 지원하지 않는 노드다 — 지원 범위는 `{{item}}`(첫 자식), 정적 마크업, 그리고 조건부 하나다"
                        ),
                    ))
                }
            }
        }
        // 방출(`emit_row_child` → `emit_cond_child`)은 수집이 다 끝난 뒤다.
        for child in rest {
            if let JsxNode::CondBinding { then_branch, .. } = self.res(child) {
                if let JsxNode::Element { children: inner, .. } = self.res(then_branch) {
                    for n in inner {
                        self.row_cond_emit(n)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// `collect_row_mirror_node`.
    fn row_mirror(&self, signal: &str, node: &'g JsxNode, list_seen: &mut Option<&'g JsxNode>) -> CResult {
        let node = self.res(node);
        if self.fully_static(node) {
            return Ok(());
        }
        match node {
            JsxNode::StaticText(_) => Ok(()),
            JsxNode::ListBinding { row, .. } => {
                let row = self.res(row);
                // codegen.rs:2307
                let JsxNode::Element { children, .. } = row else {
                    return Err(self.row_not_element(row));
                };
                // codegen.rs:2310
                if !self.row_item_only(children) {
                    return Err(Diagnostic::error(
                        L00,
                        self.sp(row),
                        format!("row 안 중첩 리스트의 row 템플릿은 `<tag>{{item}}</tag>` 형태여야 한다 (`{signal}`)"),
                    ));
                }
                // codegen.rs:2313
                if let Some(first) = *list_seen {
                    return Err(Diagnostic::error(
                        L00,
                        self.sp(node),
                        format!("row 안 조건부 속 리스트는 아직 하나만 지원한다 (`{signal}`)"),
                    )
                    .with_label(self.sp(first), "첫 번째 리스트"));
                }
                *list_seen = Some(node);
                Ok(())
            }
            JsxNode::Element { action, children, .. } => {
                // codegen.rs:2319
                if action.is_some() {
                    return Err(Diagnostic::error(
                        L00,
                        self.event_sp(node),
                        format!("row 템플릿 안의 엘리먼트는 아직 자기 `onClick`을 가질 수 없다 (`{signal}`)"),
                    ));
                }
                for c in children {
                    self.row_mirror(signal, c, list_seen)?;
                }
                Ok(())
            }
            // codegen.rs:2325
            _ => Err(Diagnostic::error(
                L00,
                self.sp(node),
                format!(
                    "`{signal}`의 row 안 조건부에서 아직 지원하지 않는 노드다 — 지원 범위는 정적 마크업과 중첩 리스트 하나다"
                ),
            )),
        }
    }

    /// `emit_cond_child`. `row_mirror`가 먼저 통과한 트리에만 불린다.
    fn row_cond_emit(&self, node: &'g JsxNode) -> CResult {
        let node = self.res(node);
        if self.fully_static(node) {
            return Ok(());
        }
        match node {
            JsxNode::StaticText(_) => Ok(()),
            JsxNode::Element { children, .. } => {
                if matches!(children.as_slice(), [only] if matches!(self.res(only), JsxNode::ListBinding { .. })) {
                    return Ok(());
                }
                for c in children {
                    self.row_cond_emit(c)?;
                }
                Ok(())
            }
            // codegen.rs:2560 — codegen 문구는 "internal error"지만 사용자가
            // 도달한다: 형제가 있는 중첩 리스트(`<ul>{xs().map(...)} x</ul>`).
            _ => Err(Diagnostic::error(
                L00,
                self.sp(node),
                "row 안 조건부의 중첩 리스트 `{xs().map(...)}`는 자기 엘리먼트의 유일한 자식이어야 한다(예: `<ul>{xs().map(...)}</ul>`)",
            )),
        }
    }
}

/// IR pre-order — lower가 `SpanTable::nodes`를 기록한 순서와 같다.
fn preorder<'g>(node: &'g JsxNode, out: &mut Vec<&'g JsxNode>) {
    out.push(node);
    match node {
        JsxNode::Element { children, .. } => {
            for c in children {
                preorder(c, out);
            }
        }
        JsxNode::ListBinding { row, .. } => preorder(row, out),
        JsxNode::CondBinding { then_branch, .. } => preorder(then_branch, out),
        JsxNode::StaticText(_)
        | JsxNode::TextBinding { .. }
        | JsxNode::RowItemText
        | JsxNode::InputBinding { .. }
        | JsxNode::ComponentRef { .. }
        | JsxNode::SharedValueBinding { .. } => {}
    }
}

/// 컴포넌트 트리의 **모든 노드**(List row·Cond then 가지 안 포함)를 pre-order로
/// 내려가 첫 `InputBinding`을 찾는다(R15, 7a C8 — design-7a 검토 20). C3판은
/// codegen.rs:634의 `collect_actions`와 같은 범위(`Element` 자식만)였다.
/// `ComponentRef`는 다른 컴포넌트라 내려가지 않는다(그 컴포넌트는 따로 검사된다).
fn first_input_binding(node: &JsxNode) -> Option<&JsxNode> {
    let mut order = Vec::new();
    preorder(node, &mut order);
    order.into_iter().find(|n| matches!(n, JsxNode::InputBinding { .. }))
}
