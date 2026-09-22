//! **참조 모델** — 후보 구현과 **독립적으로** 기대 DOM을 계산한다.
//!
//! §5 불변식 1이 "최종 DOM 순서·내용 = 참조 모델(**후보 의미론별**)"이라고
//! 쓴 그대로, 모델은 후보 의미론을 하나 받는다: row 정체성이 **유지되는가**
//! (D/E) 아니면 갱신마다 **재생성되는가**(K). 그 차이가 실제로 DOM에 나타나기
//! 때문이다 — 재생성되면 "한 번 열렸다가 숨겨진" 상태가 사라진다.
//!
//! 모델은 픽스처(`demo/nested-list-derived.tsx`)의 템플릿을 **손으로** 옮겨 적은
//! 것이고, 컴파일러 산출물을 참조하지 않는다. 둘이 같은 코드에서 유도되면
//! 대조가 자기 자신과의 대조가 되어 무의미하다.

/// 픽스처의 조건부 술어: `open() && it.length > 1`.
pub fn predicate(item: &str) -> bool {
    item.chars().count() > 1
}

/// 픽스처의 안쪽 리스트: `inner().slice(0, it.length)`.
pub fn take_count(item: &str) -> usize {
    item.chars().count()
}

/// row 정체성이 유지되는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowIdentity {
    /// D/E — 키가 같으면 같은 row가 살아남는다.
    Retained,
    /// K — 리스트가 바뀔 때마다 전부 despawn 후 재생성.
    Recreated,
    /// C — 슬롯이 **자리에 고정**되고 값만 관통한다. row 정체성 = 위치.
    /// 그래서 이 후보만 "같은 row의 아이템이 바뀐다"가 일어나고, 아이템에서
    /// 파생된 술어·slice 길이도 함께 바뀐다(`RowSpawner::revalue`).
    Slots,
}

#[derive(Debug, Clone)]
struct Row {
    item: String,
    /// 조건부 내용이 한 번이라도 만들어졌는가(`CondBlock.children`이 비지 않았는가).
    materialized: bool,
    /// 만들어진 뒤의 표시 상태.
    visible: bool,
    /// **숨겨져 있는 동안 내부 리스트가 바뀌었는가.** §5 불변식 4가 겨냥하는
    /// 시나리오가 실제로 일어났는지 세기 위한 것 — 이 플래그가 켜진 채 다시
    /// 보이게 되는 전이가 한 번도 없으면, 불변식 4는 **검사된 적이 없다**.
    /// 그것을 "통과"로 보고하면 공허한 확인이 된다.
    stale_while_hidden: bool,
}

/// 앱 상태의 참조 모델. 조작을 그대로 먹이면 기대 DOM을 낸다.
#[derive(Debug, Clone)]
pub struct Model {
    identity: RowIdentity,
    pub outer: Vec<String>,
    pub open: bool,
    pub inner: Vec<String>,
    rows: Vec<Row>,
    /// 이 실행에서 "숨은 동안 바뀐 내부 리스트가 다시 보였다" 전이가 몇 번
    /// 일어났는가. 0이면 불변식 4의 시나리오가 한 번도 안 나온 것이다.
    pub catch_up_events: usize,
    /// 방금 tick에서 그 전이가 일어났는가(계열 귀속용).
    pub catch_up_this_step: bool,
}

impl Model {
    pub fn new(identity: RowIdentity, outer: Vec<String>, open: bool, inner: Vec<String>) -> Self {
        let mut m = Model {
            identity,
            outer,
            open,
            inner,
            rows: Vec::new(),
            catch_up_events: 0,
            catch_up_this_step: false,
        };
        m.rows = m.outer.iter().map(|it| m.fresh_row(it)).collect();
        m
    }

    /// 새로 태어나는 row. spawn 시점의 조건 값이 그대로 굳는다 — 생성 함수가
    /// `site.cond && derived_flag`로 `CondBlock`을 채운 채/빈 채 만들기 때문.
    fn fresh_row(&self, item: &str) -> Row {
        let want = self.open && predicate(item);
        Row { item: item.to_string(), materialized: want, visible: true, stale_while_hidden: false }
    }

    /// 외부 리스트가 바뀌었다.
    pub fn set_outer(&mut self, next: Vec<String>) {
        if self.identity == RowIdentity::Slots {
            // 슬롯 고정: 공통 구간은 값만 관통하고, 꼬리만 늘거나 줄어든다.
            // 값이 바뀌면 그 row의 술어·slice 길이도 바뀌므로 조건 전이를
            // 다시 적용한다(구현 쪽 `revalue`가 하는 일과 같은 규칙).
            let mut events = 0;
            let common = self.rows.len().min(next.len());
            for i in 0..common {
                let before = self.open && predicate(&self.rows[i].item);
                if self.rows[i].item != next[i] {
                    self.rows[i].item = next[i].clone();
                }
                let want = self.open && predicate(&self.rows[i].item);
                events += Self::apply_want(&mut self.rows[i], before, want);
            }
            if next.len() > self.rows.len() {
                for item in &next[common..] {
                    self.rows.push(self.fresh_row(item));
                }
            } else {
                self.rows.truncate(next.len());
            }
            self.catch_up_events += events;
            self.catch_up_this_step = events > 0;
            self.outer = next;
            return;
        }
        self.rows = match self.identity {
            RowIdentity::Slots => unreachable!("위에서 처리했다"),
            // **§10 "K: 디스패치 규칙"의 네 분기**(2026-09-22). 그 전에는 여기가
            // 무조건 전량 재생성이었는데, 그것은 조항의 (4)번 분기 하나일 뿐이다.
            // §5는 참조 모델을 "**후보 자신의 명세 의미론**"에 두라고 규정하므로,
            // K의 명세가 네 분기인 이상 모델도 네 분기여야 한다 — 안 그러면 모델이
            // 구현을 "틀렸다"고 잡는 게 아니라 **조항 준수를 틀렸다고 잡는다**.
            RowIdentity::Recreated => {
                let common = self.outer.len().min(next.len());
                let mut i = 0;
                while i < common && self.outer[i] == next[i] {
                    i += 1;
                }
                if i < common {
                    // (4) 그 외 → 전량 재생성. 숨겨진 채 쌓인 상태도 함께 사라진다.
                    next.iter().map(|it| self.fresh_row(it)).collect()
                } else if self.outer.len() == next.len() {
                    // (2) 완전 동등 → 아무 일도 없다(row 상태 그대로).
                    std::mem::take(&mut self.rows)
                } else if next.is_empty() {
                    // (3) 전량 삭제.
                    Vec::new()
                } else if next.len() > self.outer.len() {
                    // (3) 순수 append → 꼬리만 새로 난다.
                    let mut keep = std::mem::take(&mut self.rows);
                    let from = self.outer.len();
                    for item in &next[from..] {
                        keep.push(self.fresh_row(item));
                    }
                    keep
                } else {
                    // (3) 순수 truncate → 꼬리만 사라진다.
                    let mut keep = std::mem::take(&mut self.rows);
                    keep.truncate(next.len());
                    keep
                }
            }
            RowIdentity::Retained => {
                // 키(=아이템 텍스트) 매칭. 같은 키가 여러 번 나오면 앞에서부터
                // 하나씩 소비한다 — `match_keys`가 하는 것과 같은 규칙.
                let mut pool: Vec<Option<Row>> = self.rows.iter().cloned().map(Some).collect();
                next.iter()
                    .map(|it| {
                        match pool.iter_mut().find(|r| r.as_ref().is_some_and(|r| &r.item == it)) {
                            Some(slot) => slot.take().unwrap(),
                            None => self.fresh_row(it),
                        }
                    })
                    .collect()
            }
        };
        self.outer = next;
    }

    /// row 하나의 실효 조건이 `before`에서 `want`로 바뀌었을 때의 전이.
    /// 토글(전 row 동시)과 값 관통(C, row 하나)이 **같은 규칙**을 타야 하므로
    /// 한 곳에 둔다 — 두 벌로 두면 한쪽만 고치는 사고가 난다.
    fn apply_want(row: &mut Row, before: bool, want: bool) -> usize {
        if want == before {
            return 0;
        }
        if !row.materialized {
            if want {
                row.materialized = true;
                row.visible = true;
            }
            return 0;
        }
        let mut events = 0;
        if want && row.stale_while_hidden {
            // 숨은 동안 바뀐 내용이 이제 보인다 — 불변식 4의 시나리오.
            events += 1;
            row.stale_while_hidden = false;
        }
        row.visible = want;
        events
    }

    /// 조건부가 토글됐다. row별 실효값은 `open && predicate(item)`이고,
    /// **그 값이 바뀐 row에서만** 생성/표시 전환이 일어난다(생성 함수의 쿼리가
    /// `Changed<Signal<bool>>`이므로).
    pub fn set_open(&mut self, open: bool) {
        let before: Vec<bool> = self.rows.iter().map(|r| self.open && predicate(&r.item)).collect();
        self.open = open;
        let mut events = 0;
        for (i, row) in self.rows.iter_mut().enumerate() {
            let want = open && predicate(&row.item);
            events += Self::apply_want(row, before[i], want);
        }
        self.catch_up_events += events;
        self.catch_up_this_step = events > 0;
    }

    pub fn set_inner(&mut self, inner: Vec<String>) {
        if inner != self.inner {
            for row in &mut self.rows {
                if row.materialized && !row.visible {
                    row.stale_while_hidden = true;
                }
            }
        }
        self.inner = inner;
    }

    /// 이번 tick에 catch-up 전이가 없었음을 표시(다른 조작 뒤 초기화).
    pub fn clear_step_flag(&mut self) {
        self.catch_up_this_step = false;
    }

    /// `<ul class="outer">`의 자식들. 테스트가 비교하는 문자열.
    pub fn render_rows(&self) -> String {
        let mut out = String::new();
        for row in &self.rows {
            let panel = if row.materialized && !row.visible {
                "<div class=\"panel\" hidden>"
            } else {
                "<div class=\"panel\">"
            };
            out.push_str(&format!("<li>{}{panel}", row.item));
            if row.materialized {
                let take = take_count(&row.item).min(self.inner.len());
                out.push_str("<ul class=\"inner\">");
                for x in &self.inner[..take] {
                    out.push_str(&format!("<li>{x}</li>"));
                }
                out.push_str("</ul>");
            }
            out.push_str("</div></li>");
        }
        out
    }

    /// 지금 보이는(= 내용이 있고 숨겨지지 않은) row의 아이템들.
    pub fn visible_rows(&self) -> Vec<String> {
        self.rows
            .iter()
            .filter(|r| r.materialized && r.visible)
            .map(|r| r.item.clone())
            .collect()
    }
}
