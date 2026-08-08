//! 조작 시퀀스 생성기 — §5의 "외부 리스트 ops + 조건부 토글 + 내부 리스트 ops의
//! 무작위 인터리브 시퀀스(길이 ≤200)".
//!
//! 시드는 전부 기록된다. 반례가 나오면 시드 + 최소화된 시퀀스가 그대로 재현
//! 재료가 되어야 하기 때문(§5 "시드 기록, shrinking으로 최소 반례").

/// PCG64 (XSL RR 128/64) — 하네스(`loom-bench`)와 같은 스킴을 쓴다. 표준 라이브러리
/// 난수가 없고, 재현 가능해야 하며, 시드 유도가 결정적이어야 한다.
pub struct Pcg64 {
    state: u128,
    inc: u128,
}

impl Pcg64 {
    pub fn from_seed(seed: u64) -> Self {
        let mut r = Pcg64 { state: 0, inc: (u128::from(seed) << 1) | 1 };
        r.next_u64();
        r.state = r.state.wrapping_add(u128::from(seed));
        r.next_u64();
        r
    }

    pub fn next_u64(&mut self) -> u64 {
        self.state = self
            .state
            .wrapping_mul(0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645)
            .wrapping_add(self.inc);
        let rot = (self.state >> 122) as u32;
        let xsl = ((self.state >> 64) as u64) ^ (self.state as u64);
        xsl.rotate_right(rot)
    }

    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }
}

/// 한 조작. 어디에 적용되는지가 이름에 들어 있다 — 반례를 읽을 때 그것부터
/// 보게 되기 때문.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    OuterInsert { at: usize, item: String },
    OuterRemove { at: usize },
    OuterMove { from: usize, to: usize },
    OuterSwap { a: usize, b: usize },
    OuterReverse,
    Toggle,
    InnerInsert { at: usize, item: String },
    InnerRemove { at: usize },
    InnerMove { from: usize, to: usize },
}

/// n ≤ 50 (§5). 아이템 길이가 1~4로 섞이는 것이 중요하다 — 술어(`length > 1`)와
/// slice 길이가 아이템에서 파생되므로, 길이가 균일하면 구간이 균일해져 불변식 2의
/// 판별력이 사라진다(2026-08-07에 확인한 그 문제).
pub const MAX_OUTER: usize = 50;
pub const MAX_INNER: usize = 50;

fn mint(rng: &mut Pcg64, counter: &mut usize) -> String {
    *counter += 1;
    let len = 1 + rng.below(4);
    let ch = (b'a' + (*counter % 26) as u8) as char;
    let mut s: String = std::iter::repeat(ch).take(len).collect();
    s.push_str(&counter.to_string());
    s
}

/// 길이 `len` 이하의 시퀀스를 만든다. 인덱스는 **생성 시점의 길이 추정치**로
/// 뽑되, 적용 시점에 다시 clamp한다 — shrinking으로 앞쪽 op가 빠지면 길이가
/// 달라지기 때문(그래도 시퀀스가 유효하게 남아야 최소화가 진행된다).
pub fn sequence(seed: u64, len: usize, outer0: usize, inner0: usize) -> Vec<Op> {
    let mut rng = Pcg64::from_seed(seed);
    let mut counter = 0usize;
    let (mut outer, mut inner) = (outer0, inner0);
    let mut ops = Vec::with_capacity(len);
    for _ in 0..len {
        let op = match rng.below(9) {
            0 if outer < MAX_OUTER => {
                outer += 1;
                Op::OuterInsert { at: rng.below(outer), item: mint(&mut rng, &mut counter) }
            }
            1 if outer > 0 => {
                let at = rng.below(outer);
                outer -= 1;
                Op::OuterRemove { at }
            }
            2 if outer > 1 => Op::OuterMove { from: rng.below(outer), to: rng.below(outer) },
            3 if outer > 1 => Op::OuterSwap { a: rng.below(outer), b: rng.below(outer) },
            4 if outer > 1 => Op::OuterReverse,
            5 => Op::Toggle,
            6 if inner < MAX_INNER => {
                inner += 1;
                Op::InnerInsert { at: rng.below(inner), item: mint(&mut rng, &mut counter) }
            }
            7 if inner > 0 => {
                let at = rng.below(inner);
                inner -= 1;
                Op::InnerRemove { at }
            }
            8 if inner > 1 => Op::InnerMove { from: rng.below(inner), to: rng.below(inner) },
            // 조건이 안 맞으면 토글로 떨어진다 — 시퀀스 길이를 유지하기 위해서.
            _ => Op::Toggle,
        };
        ops.push(op);
    }
    ops
}

/// 조작을 실제 상태에 적용한다. **모델과 앱이 같은 함수를 쓴다** — 조작 해석이
/// 갈리면 두 쪽이 다른 것을 하고 있는데도 "불일치"로만 보이기 때문.
pub fn apply(op: &Op, outer: &mut Vec<String>, open: &mut bool, inner: &mut Vec<String>) {
    fn idx(i: usize, len: usize) -> usize {
        if len == 0 { 0 } else { i.min(len - 1) }
    }
    match op {
        Op::OuterInsert { at, item } => {
            if outer.len() < MAX_OUTER {
                let at = (*at).min(outer.len());
                outer.insert(at, item.clone());
            }
        }
        Op::OuterRemove { at } => {
            if !outer.is_empty() {
                outer.remove(idx(*at, outer.len()));
            }
        }
        Op::OuterMove { from, to } => {
            if outer.len() > 1 {
                let f = idx(*from, outer.len());
                let v = outer.remove(f);
                let t = (*to).min(outer.len());
                outer.insert(t, v);
            }
        }
        Op::OuterSwap { a, b } => {
            if outer.len() > 1 {
                let (a, b) = (idx(*a, outer.len()), idx(*b, outer.len()));
                outer.swap(a, b);
            }
        }
        Op::OuterReverse => outer.reverse(),
        Op::Toggle => *open = !*open,
        Op::InnerInsert { at, item } => {
            if inner.len() < MAX_INNER {
                let at = (*at).min(inner.len());
                inner.insert(at, item.clone());
            }
        }
        Op::InnerRemove { at } => {
            if !inner.is_empty() {
                inner.remove(idx(*at, inner.len()));
            }
        }
        Op::InnerMove { from, to } => {
            if inner.len() > 1 {
                let f = idx(*from, inner.len());
                let v = inner.remove(f);
                let t = (*to).min(inner.len());
                inner.insert(t, v);
            }
        }
    }
}
