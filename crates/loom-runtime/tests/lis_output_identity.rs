//! **`lis_keep`의 출력이 옛 알고리즘과 같은가** — 이 과제의 핵심 안전망.
//!
//! `loom_flush_core::lis_keep`의 사후조건 `take_order_preserved`는 **건전성만**
//! 고정한다("표시된 것들이 증가 부분수열이다"). *어느* 증가 부분수열인지는
//! 고정하지 않는다. 즉 알고리즘을 갈아치우면 **Verus 검증은 통과하면서 방출
//! write가 달라질 수 있다** — 그러면 트랙 L 수집분이 §2.5상 무효가 된다.
//!
//! 그 경로를 막는 것은 검증이 아니라 이 대조뿐이다. `write_identity` 골든도
//! 부족하다 — 거기엔 8단계 시퀀스 하나만 들어 있다.
//!
//! 아래 `old_*`는 이관 **직전 커밋(a83960b)**의 `crates/loom-runtime/src/list.rs`
//! 에서 글자 그대로 복사한 것이다. 손으로 다시 쓴 것이 아니다 — 다시 쓰면
//! 대조가 "내가 생각한 알고리즘"과의 대조가 되어 무의미해진다.
//!
//! **`shorter`/`longer` 항이 핵심**: 사후조건만으로는 "건전하지만 더 짧은
//! 부분수열"로 도망친 회귀가 통과한다. 표시 개수까지 봐야 그것이 잡힌다.

use std::collections::HashSet;

// ---------------------------------------------------------------- OLD (HEAD)

fn old_lis_indices(sequence: &[usize]) -> HashSet<usize> {
    if sequence.is_empty() {
        return HashSet::new();
    }
    let mut tails: Vec<usize> = Vec::new();
    let mut predecessor: Vec<Option<usize>> = vec![None; sequence.len()];

    for i in 0..sequence.len() {
        let val = sequence[i];
        let pos = tails.partition_point(|&ti| sequence[ti] < val);
        if pos > 0 {
            predecessor[i] = Some(tails[pos - 1]);
        }
        if pos == tails.len() {
            tails.push(i);
        } else {
            tails[pos] = i;
        }
    }

    let mut result = HashSet::new();
    let mut cursor = tails.last().copied();
    while let Some(idx) = cursor {
        result.insert(idx);
        cursor = predecessor[idx];
    }
    result
}

fn old_lis_keep(take_old: &[usize], old_len: usize) -> Vec<bool> {
    let retained_old_indices: Vec<usize> =
        take_old.iter().copied().filter(|&t| t < old_len).collect();
    let lis = old_lis_indices(&retained_old_indices);

    let mut keep = vec![false; take_old.len()];
    let mut retained_seen = 0;
    for (i, &t) in take_old.iter().enumerate() {
        if t < old_len {
            if lis.contains(&retained_seen) {
                keep[i] = true;
            }
            retained_seen += 1;
        }
    }
    keep
}

// --------------------------------------------------------------- properties

/// Precondition P, exactly as `take_order_preserved` states it.
fn p_holds(take_old: &[usize], old_len: usize, keep: &[bool]) -> bool {
    for a in 0..take_old.len() {
        for b in (a + 1)..take_old.len() {
            if keep[a] && take_old[a] < old_len && keep[b] && take_old[b] < old_len {
                if !(take_old[a] < take_old[b]) {
                    return false;
                }
            }
        }
    }
    true
}

/// Length of the marked set restricted to retained slots — used to detect a
/// "sound but shorter" regression that P alone would happily accept.
fn marked_retained(take_old: &[usize], old_len: usize, keep: &[bool]) -> usize {
    (0..take_old.len())
        .filter(|&i| keep[i] && take_old[i] < old_len)
        .count()
}

// ------------------------------------------------------------------- driver

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

struct Stats {
    cases: usize,
    mismatch: usize,
    p_violation: usize,
    shorter: usize,
    longer: usize,
    nonempty: usize,
    first_bad: Option<(Vec<usize>, usize, Vec<bool>, Vec<bool>)>,
}
impl Stats {
    fn new() -> Self {
        Stats { cases: 0, mismatch: 0, p_violation: 0, shorter: 0, longer: 0, nonempty: 0, first_bad: None }
    }
    fn check(&mut self, take_old: &[usize], old_len: usize) {
        let want = old_lis_keep(take_old, old_len);
        let got = loom_flush_core::lis_keep(&take_old.to_vec(), old_len);
        self.cases += 1;
        assert_eq!(got.len(), take_old.len(), "length contract broken on {take_old:?} old_len={old_len}");
        if !p_holds(take_old, old_len, &got) {
            self.p_violation += 1;
        }
        let mw = marked_retained(take_old, old_len, &want);
        let mg = marked_retained(take_old, old_len, &got);
        if mg > 0 {
            self.nonempty += 1;
        }
        if mg < mw {
            self.shorter += 1;
        }
        if mg > mw {
            self.longer += 1;
        }
        if want != got {
            self.mismatch += 1;
            if self.first_bad.is_none() {
                self.first_bad =
                    Some((take_old.to_vec(), old_len, want.clone(), got.clone()));
            }
        }
    }
    fn report(&self, label: &str) {
        println!(
            "{label}: cases={} mismatch={} P_violation={} shorter_than_old={} longer_than_old={} nonempty_keep={}",
            self.cases, self.mismatch, self.p_violation, self.shorter, self.longer, self.nonempty
        );
        if let Some((t, l, w, g)) = &self.first_bad {
            println!("  FIRST MISMATCH take_old={t:?} old_len={l} old_keep={w:?} new_keep={g:?}");
        }
    }
}

fn rec(buf: &mut Vec<usize>, len: usize, alpha: usize, old_len: usize, st: &mut Stats) {
    if buf.len() == len {
        st.check(buf, old_len);
        return;
    }
    for v in 0..alpha {
        buf.push(v);
        rec(buf, len, alpha, old_len, st);
        buf.pop();
    }
}

fn perms(cur: &mut Vec<usize>, remaining: &mut Vec<usize>, st: &mut Stats, old_len: usize) {
    if remaining.is_empty() {
        st.check(cur, old_len);
        return;
    }
    for i in 0..remaining.len() {
        let v = remaining.remove(i);
        cur.push(v);
        perms(cur, remaining, st, old_len);
        cur.pop();
        remaining.insert(i, v);
    }
}

#[test]
fn verified_lis_keep_matches_the_original_algorithm_exhaustively() {
    let mut st = Stats::new();

    // (A) all permutations n<=7, fully retained
    for n in 0..=7usize {
        let mut rem: Vec<usize> = (0..n).collect();
        let mut cur = Vec::new();
        perms(&mut cur, &mut rem, &mut st, n);
    }
    st.report("A permutations n<=7 (all retained)");

    // (B) exhaustive small alphabets incl. duplicates AND fresh slots
    //     (values >= old_len are fresh)
    let mut b = Stats::new();
    for alpha in 2..=4usize {
        for len in 0..=6usize {
            if (alpha as u64).pow(len as u32) > 300_000 {
                continue;
            }
            for old_len in 0..=alpha {
                let mut buf = Vec::new();
                rec(&mut buf, len, alpha, old_len, &mut b);
            }
        }
    }
    b.report("B exhaustive alphabets 2..4, len<=6, all old_len");

    // (C) exhaustive with a wide value range so fresh slots use large ids
    let mut c = Stats::new();
    let vals = [0usize, 1, 2, 5, 9, 100];
    for len in 0..=5usize {
        for old_len in [0usize, 1, 2, 3, 6, 101] {
            let mut idx = vec![0usize; len];
            loop {
                let seq: Vec<usize> = idx.iter().map(|&i| vals[i]).collect();
                c.check(&seq, old_len);
                let mut k = 0;
                loop {
                    if k == len {
                        break;
                    }
                    idx[k] += 1;
                    if idx[k] < vals.len() {
                        break;
                    }
                    idx[k] = 0;
                    k += 1;
                }
                if k == len {
                    break;
                }
            }
        }
    }
    c.report("C wide-value exhaustive len<=5");

    // (D) large randomized sweep, long sequences (exercises deep tails/binary search)
    let mut d = Stats::new();
    let mut rng = Rng(0x5eed_1234_9abc_def1);
    for _ in 0..60_000 {
        let n = rng.below(80);
        let span = 1 + rng.below(90);
        let old_len = rng.below(span + 4);
        let seq: Vec<usize> = (0..n).map(|_| rng.below(span)).collect();
        d.check(&seq, old_len);
    }
    // injective retained part (the real shape produced by match_keys)
    for _ in 0..40_000 {
        let n = rng.below(60);
        let old_len = rng.below(n + 3);
        let mut pool: Vec<usize> = (0..old_len).collect();
        // shuffle
        for i in (1..pool.len()).rev() {
            let j = rng.below(i + 1);
            pool.swap(i, j);
        }
        let mut seq = Vec::with_capacity(n);
        let mut p = 0usize;
        for _ in 0..n {
            if p < pool.len() && rng.below(3) != 0 {
                seq.push(pool[p]);
                p += 1;
            } else {
                seq.push(old_len + rng.below(10));
            }
        }
        d.check(&seq, old_len);
    }
    d.report("D randomized (n<=80, incl. injective-retained shape)");

    let total = st.cases + b.cases + c.cases + d.cases;
    let mism = st.mismatch + b.mismatch + c.mismatch + d.mismatch;
    let pv = st.p_violation + b.p_violation + c.p_violation + d.p_violation;
    let sh = st.shorter + b.shorter + c.shorter + d.shorter;
    let lo = st.longer + b.longer + c.longer + d.longer;
    let ne = st.nonempty + b.nonempty + c.nonempty + d.nonempty;
    println!("TOTAL cases={total} mismatch={mism} P_violation={pv} shorter={sh} longer={lo} nonempty_keep={ne}");
    assert!(ne > total / 3, "corpus too degenerate: only {ne} nonempty keeps of {total}");
    assert_eq!(pv, 0, "new lis_keep violates P");
    assert_eq!(mism, 0, "new lis_keep disagrees with the HEAD implementation");
}
