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
}

#[derive(Debug, Clone)]
struct Row {
    item: String,
    /// 조건부 내용이 한 번이라도 만들어졌는가(`CondBlock.children`이 비지 않았는가).
    materialized: bool,
    /// 만들어진 뒤의 표시 상태.
    visible: bool,
}

/// 앱 상태의 참조 모델. 조작을 그대로 먹이면 기대 DOM을 낸다.
#[derive(Debug, Clone)]
pub struct Model {
    identity: RowIdentity,
    pub outer: Vec<String>,
    pub open: bool,
    pub inner: Vec<String>,
    rows: Vec<Row>,
}

impl Model {
    pub fn new(identity: RowIdentity, outer: Vec<String>, open: bool, inner: Vec<String>) -> Self {
        let mut m = Model { identity, outer, open, inner, rows: Vec::new() };
        m.rows = m.outer.iter().map(|it| m.fresh_row(it)).collect();
        m
    }

    /// 새로 태어나는 row. spawn 시점의 조건 값이 그대로 굳는다 — 생성 함수가
    /// `site.cond && derived_flag`로 `CondBlock`을 채운 채/빈 채 만들기 때문.
    fn fresh_row(&self, item: &str) -> Row {
        let want = self.open && predicate(item);
        Row { item: item.to_string(), materialized: want, visible: true }
    }

    /// 외부 리스트가 바뀌었다.
    pub fn set_outer(&mut self, next: Vec<String>) {
        self.rows = match self.identity {
            RowIdentity::Recreated => next.iter().map(|it| self.fresh_row(it)).collect(),
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

    /// 조건부가 토글됐다. row별 실효값은 `open && predicate(item)`이고,
    /// **그 값이 바뀐 row에서만** 생성/표시 전환이 일어난다(생성 함수의 쿼리가
    /// `Changed<Signal<bool>>`이므로).
    pub fn set_open(&mut self, open: bool) {
        let before: Vec<bool> = self.rows.iter().map(|r| self.open && predicate(&r.item)).collect();
        self.open = open;
        for (i, row) in self.rows.iter_mut().enumerate() {
            let want = open && predicate(&row.item);
            if want == before[i] {
                continue;
            }
            if !row.materialized {
                if want {
                    row.materialized = true;
                    row.visible = true;
                }
            } else {
                row.visible = want;
            }
        }
    }

    pub fn set_inner(&mut self, inner: Vec<String>) {
        self.inner = inner;
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
