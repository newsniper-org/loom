//! 생성기 커버리지 보고 — §5의 n≤50 영역을 실제로 쓰는지.
use loom_track_n::ops::{self, Op};

#[test]
fn generator_reaches_the_registered_size_bound() {
    let (mut hi_o, mut hi_i) = (Vec::new(), Vec::new());
    let (mut sum_o, mut sum_i, mut n) = (0usize, 0usize, 0usize);
    for s in 0..400u64 {
        let seq = ops::sequence(20_260_808 * 1_000_003 + 7_919 + s, 200, 3, 4);
        let (mut o, mut i, mut open) = (
            vec!["a".to_string(), "bb".to_string(), "ccc".to_string()],
            vec!["p".to_string(), "q".to_string(), "r".to_string(), "s".to_string()],
            false,
        );
        let (mut ho, mut hii) = (o.len(), i.len());
        for op in &seq {
            ops::apply(op, &mut o, &mut open, &mut i);
            ho = ho.max(o.len());
            hii = hii.max(i.len());
            sum_o += o.len();
            sum_i += i.len();
            n += 1;
        }
        hi_o.push(ho);
        hi_i.push(hii);
    }
    hi_o.sort_unstable();
    hi_i.sort_unstable();
    let med = |v: &Vec<usize>| v[v.len() / 2];
    let near_cap = |v: &Vec<usize>| v.iter().filter(|x| **x >= 40).count() as f64 / v.len() as f64;
    println!(
        "외부: 최대도달 중앙 {} / 최대 {} / ≥40 비율 {:.1}% / 평균 길이 {:.1}",
        med(&hi_o), hi_o[hi_o.len() - 1], near_cap(&hi_o) * 100.0, sum_o as f64 / n as f64
    );
    println!(
        "내부: 최대도달 중앙 {} / 최대 {} / ≥40 비율 {:.1}% / 평균 길이 {:.1}",
        med(&hi_i), hi_i[hi_i.len() - 1], near_cap(&hi_i) * 100.0, sum_i as f64 / n as f64
    );
    assert!(hi_o[hi_o.len() - 1] >= 45, "외부 리스트가 상한 근처에 닿지 않는다");
    assert!(hi_i[hi_i.len() - 1] >= 45, "내부 리스트가 상한 근처에 닿지 않는다");
    assert!(near_cap(&hi_o) > 0.10, "큰 외부 리스트를 쓰는 시퀀스가 너무 적다");
    let _ = |_: &Op| ();
}
