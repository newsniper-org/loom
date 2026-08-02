//! RQ1-benchmark-protocol.md **§7 M2 "DOM 연산 수"** 계측.
//!
//! §7 M2의 정의: "단일 디스패치 지점에서 op 종류별(insertBefore/moveBefore/
//! removeChild/setText/setAttr/detach/attach) 카운트. 셀당 1회는
//! MutationObserver(childList + characterData + attributes(+attributeOldValue),
//! subtree)로 교차 검증. … M2는 시드 고정 하에 결정적 수치 — 통계 검정 비적용,
//! 사전 선언된 결정 규칙(§9 E)만 적용."
//!
//! **왜 이제야 만드는가(정직한 경위)**: 2026-07-28 계측 커버리지 감사에서
//! M2가 **전면 미계측**임이 드러났다 — 하네스에 카운터도 MutationObserver도
//! 없었다. 그런데 §9 E 기각 (i)은 "DOM 연산 수(M2)는 결정적 보조 지표로 강등:
//! E의 셀당 총 DOM op 수 ≥ K의 50%이면 'DOM 연산 절감 없음' 결정적 판정(통계
//! 비적용, **병기**)"으로 그 산출을 명시적으로 요구한다. E 기각 (i) 자체는
//! M1(a)로 이미 미발동 판정됐으므로 그 판정이 소급 무효가 되지는 않으나,
//! 함께 실려야 할 지표가 통째로 빠진 채 보고돼 온 상태였다. 상세는
//! `research-questions/RQ1-dynamic-control-flow-scheduling.md`의 "계측 커버리지
//! 감사"(2026-07-28) 절.
//!
//! **설계**: `DomOps`가 트레잇이므로 후보 코드를 한 줄도 건드리지 않고
//! 데코레이터 하나로 끝난다(§2.4 "후보 코드 무침습"과 정합 — §7.1 단계 귀속
//! 계측이 마커 System을 쓴 것과 같은 원리). `LoomApp::tick`도, 후보 System도,
//! `DomWrite` 열거형도 무변경.

use crate::{DomOps, NodeHandle};

/// `DomOps` 호출 횟수를 op 종류별로 센 것. §7 M2가 열거한 op 종류
/// (insertBefore/moveBefore/removeChild/setText/setAttr/detach/attach)와
/// 이 트레잇의 메서드는 1:1이 아니므로, **트레잇 메서드 단위로 세고**
/// 대응 관계를 아래 `mutation_observer_channel`에 명시한다 — 프로토콜이
/// 요구하는 MutationObserver 교차 검증 시 어느 채널로 관측될지가 그 대응표다.
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomOpCounts {
    pub create_element: u32,
    pub append_static_text: u32,
    pub append_static_html: u32,
    pub set_text_content: u32,
    pub clear_children: u32,
    pub remove_node: u32,
    pub reposition: u32,
    pub set_visible: u32,
    pub set_attribute: u32,
    pub set_input_value: u32,
    // --- 읽기 전용(변이 아님) — 총계에 넣지 않는다 ---
    pub adopt_children: u32,
    pub get_input_value: u32,
}

impl DomOpCounts {
    /// §9 E 기각 (i)의 "셀당 총 DOM op 수"가 가리키는 값 — **DOM을 변이시키는
    /// 호출만** 합산한다. `adopt_children`/`get_input_value`는 resume 전용
    /// 읽기 연산이라 제외(둘 다 Phase 1 후보 경로에서는 호출되지도 않는다).
    pub fn total_mutations(&self) -> u32 {
        self.create_element
            + self.append_static_text
            + self.append_static_html
            + self.set_text_content
            + self.clear_children
            + self.remove_node
            + self.reposition
            + self.set_visible
            + self.set_attribute
            + self.set_input_value
    }

    /// §7 M2의 "대응표"(insert/remove/move 폴백/detach/attach → childList,
    /// setText → characterData, setAttr → attributes)를 이 트레잇의 메서드
    /// 이름에 적용한 것. MutationObserver 교차 검증 시 각 카운트가 어느
    /// 채널에서 관측돼야 하는지를 코드로 고정해 둔다.
    pub fn mutation_observer_channel(op: &str) -> &'static str {
        match op {
            "create_element" | "append_static_text" | "append_static_html" | "clear_children"
            | "remove_node" | "reposition" => "childList",
            // `textContent` 대입은 기존 자식을 통째로 갈아치우므로 실제
            // 관측은 childList로도 잡힐 수 있다 — §7 M2의 대응표가
            // characterData로 지정했으므로 그대로 두되 이 어긋남을 명시한다.
            "set_text_content" => "characterData",
            "set_visible" | "set_attribute" => "attributes",
            // `element.value = ...`는 **property** 대입이라 MutationObserver로
            // 아예 관측되지 않는다(§7 M2가 다루지 않는 경우 — 교차 검증
            // 불가 항목으로 보고).
            "set_input_value" => "unobservable(property)",
            _ => "n/a",
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"create_element\": {}, \"append_static_text\": {}, \"append_static_html\": {}, \
             \"set_text_content\": {}, \"clear_children\": {}, \"remove_node\": {}, \
             \"reposition\": {}, \"set_visible\": {}, \"set_attribute\": {}, \
             \"set_input_value\": {}, \"total_mutations\": {}}}",
            self.create_element,
            self.append_static_text,
            self.append_static_html,
            self.set_text_content,
            self.clear_children,
            self.remove_node,
            self.reposition,
            self.set_visible,
            self.set_attribute,
            self.set_input_value,
            self.total_mutations(),
        )
    }
}

/// 임의의 `DomOps` 구현을 감싸 호출 수를 세는 데코레이터. 후보 코드도
/// `LoomApp`도 이 타입의 존재를 모른다 — 하네스가 `BrowserDom` 대신 이걸
/// 넘기기만 하면 된다.
///
/// **네이티브/폴백 분해 (2026-08-02 해소)**: §7 M2가 요구하는 `moveBefore`
/// 네이티브 vs `insertBefore` 폴백 구분은 `web::BrowserDom::reposition`
/// **안에서** 일어나 바깥 데코레이터에서는 보이지 않는다. 그래서 `DomOps`
/// 트레잇에 `reposition_split()`을 얹고 `BrowserDom`이 직접 세도록 했다 —
/// 여기서는 그것을 그대로 위임한다. (§9 E의 "채택 시 조건부 제약" 보고에
/// 필요: E의 이점이 네이티브 경로에만 있는지 폴백에도 남는지.)
///
/// §7 M2의 주석 요구사항 하나는 여전히 유효하다: **네이티브 `moveBefore`가
/// MutationObserver에는 remove+insert 쌍으로 보일 수 있다**. 따라서 아래
/// 교차 검증에서 childList 레코드 수가 이쪽 카운트보다 클 수 있고, 그건
/// 불일치가 아니라 관측 채널의 성질이다.
pub struct CountingDom<D: DomOps> {
    inner: D,
    counts: DomOpCounts,
}

impl<D: DomOps> CountingDom<D> {
    pub fn new(inner: D) -> Self {
        CountingDom { inner, counts: DomOpCounts::default() }
    }

    pub fn counts(&self) -> DomOpCounts {
        self.counts
    }

    /// 반복(rep) 경계에서 0으로 되돌린다 — §8의 "각 반복 전 초기 상태 복원"과
    /// 같은 지점에서 부르라고 만든 것. 셋업 단계의 op가 측정 반복에 새는
    /// 것을 막는다.
    pub fn reset_counts(&mut self) {
        self.counts = DomOpCounts::default();
    }

    pub fn inner_mut(&mut self) -> &mut D {
        &mut self.inner
    }

    pub fn into_inner(self) -> D {
        self.inner
    }
}

impl<D: DomOps> DomOps for CountingDom<D> {
    fn create_element(&mut self, parent: NodeHandle, tag: &str, class: Option<&str>) -> NodeHandle {
        self.counts.create_element += 1;
        self.inner.create_element(parent, tag, class)
    }

    fn append_static_text(&mut self, parent: NodeHandle, text: &str) {
        self.counts.append_static_text += 1;
        self.inner.append_static_text(parent, text);
    }

    fn append_static_html(&mut self, parent: NodeHandle, html: &str) {
        self.counts.append_static_html += 1;
        self.inner.append_static_html(parent, html);
    }

    fn set_text_content(&mut self, target: NodeHandle, text: &str) {
        self.counts.set_text_content += 1;
        self.inner.set_text_content(target, text);
    }

    fn clear_children(&mut self, parent: NodeHandle) {
        self.counts.clear_children += 1;
        self.inner.clear_children(parent);
    }

    fn remove_node(&mut self, node: NodeHandle) {
        self.counts.remove_node += 1;
        self.inner.remove_node(node);
    }

    fn reposition(&mut self, parent: NodeHandle, node: NodeHandle, before: Option<NodeHandle>) {
        self.counts.reposition += 1;
        self.inner.reposition(parent, node, before);
    }

    fn reposition_split(&self) -> (u64, u64) {
        self.inner.reposition_split()
    }

    fn set_visible(&mut self, node: NodeHandle, visible: bool) {
        self.counts.set_visible += 1;
        self.inner.set_visible(node, visible);
    }

    fn set_attribute(&mut self, node: NodeHandle, name: &str, value: &str) {
        self.counts.set_attribute += 1;
        self.inner.set_attribute(node, name, value);
    }

    fn adopt_children(&mut self, parent: NodeHandle) -> Vec<(NodeHandle, String)> {
        self.counts.adopt_children += 1;
        self.inner.adopt_children(parent)
    }

    fn set_input_value(&mut self, node: NodeHandle, value: &str) {
        self.counts.set_input_value += 1;
        self.inner.set_input_value(node, value);
    }

    fn get_input_value(&mut self, node: NodeHandle) -> String {
        self.counts.get_input_value += 1;
        self.inner.get_input_value(node)
    }
}
