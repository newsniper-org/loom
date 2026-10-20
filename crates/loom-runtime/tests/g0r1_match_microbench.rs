//! G0R1 C3 — 사전 등록 마이크로벤치(개정 #12 "매칭 구현 사전 선언").
//!
//! radix 병합(`match_by_key`) vs 해시맵형(`HashMap<u64, Vec<usize>>` + 통과 hasher — key가 이미 해시이므로 최선형,
//! 버킷 안에서 Item 동등 검증)을
//! W2 셔플·역순 × n∈{1,000, 10,000} × {ItemId형 u32, String 128B}에서 비교한다. 해시맵형이 어느 한 조건이라도
//! 1.10배를 넘게 빠르면 C4로 넘어가지 않고 판정례 (마)를 병익님께 올린다.
//!
//! 실행: `cargo test -p loom-runtime --release --test g0r1_match_microbench -- --ignored --nocapture`
//! 두 구현이 같은 `take_old`를 내는지 먼저 단언한다(공허한 속도 비교 방지). seed 고정, 반복 min-of-R.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::time::Instant;

use loom_runtime::list_key::{match_by_key, row_key};

/// key가 이미 64비트 해시이므로 해시맵 쪽에 최선형을 준다: SipHash 대신 통과 hasher(상위 비트를 섞는 곱 1회).
#[derive(Default)]
struct PassHasher(u64);
impl Hasher for PassHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, _: &[u8]) {
        unreachable!()
    }
    fn write_u64(&mut self, k: u64) {
        self.0 = k.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}
type PassMap<V> = HashMap<u64, V, BuildHasherDefault<PassHasher>>;

fn hashmap_match<T: Ord + Eq>(ok: &[u64], oi: &[&T], nk: &[u64], ni: &[&T]) -> Vec<usize> {
    let m = ok.len();
    let mut map: PassMap<Vec<usize>> = PassMap::with_capacity_and_hasher(m, Default::default());
    for (j, k) in ok.iter().enumerate() {
        map.entry(*k).or_default().push(j);
    }
    // 소비는 앞에서부터(j 오름차순) — 같은 의미를 맞추려고 VecDeque 대신 cursor 없이 retain한다.
    let mut cursor: PassMap<usize> = PassMap::with_capacity_and_hasher(m, Default::default());
    let mut taken = vec![false; m];
    let mut out = Vec::with_capacity(nk.len());
    for (i, k) in nk.iter().enumerate() {
        let mut found = m;
        if let Some(bucket) = map.get(k) {
            let c = cursor.entry(*k).or_insert(0);
            // 버킷 안에서 아직 소비되지 않은 첫 Item 동등 원소
            let mut p = *c;
            while p < bucket.len() {
                let j = bucket[p];
                if !taken[j] && *oi[j] == *ni[i] {
                    found = j;
                    taken[j] = true;
                    if p == *c {
                        *c += 1;
                        while *c < bucket.len() && taken[bucket[*c]] {
                            *c += 1;
                        }
                    }
                    break;
                }
                p += 1;
            }
        }
        out.push(found);
    }
    out
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn perm(rng: &mut Rng, n: usize, reverse: bool) -> Vec<usize> {
    let mut v: Vec<usize> = (0..n).collect();
    if reverse {
        v.reverse();
    } else {
        for i in (1..n).rev() {
            let j = (rng.next() as usize) % (i + 1);
            v.swap(i, j);
        }
    }
    v
}

fn bench<T: Ord + Eq + std::hash::Hash>(items: &[T], p: &[usize], reps: usize) -> (f64, f64) {
    let old: Vec<&T> = items.iter().collect();
    let new: Vec<&T> = p.iter().map(|&i| &items[i]).collect();
    let ok: Vec<u64> = old.iter().map(|x| row_key(*x)).collect();
    let nk: Vec<u64> = new.iter().map(|x| row_key(*x)).collect();
    // 동치 단언
    let a = match_by_key(&ok, &old, &nk, &new).take_old;
    let b = hashmap_match(&ok, &old, &nk, &new);
    assert_eq!(a, b, "두 구현의 take_old가 다르다");
    let (mut best_a, mut best_b) = (f64::MAX, f64::MAX);
    for _ in 0..reps {
        // 번갈아 실행해 드리프트를 같이 맞는다
        let t = Instant::now();
        let r = match_by_key(&ok, &old, &nk, &new);
        best_a = best_a.min(t.elapsed().as_secs_f64() * 1e6);
        std::hint::black_box(r);
        let t = Instant::now();
        let r = hashmap_match(&ok, &old, &nk, &new);
        best_b = best_b.min(t.elapsed().as_secs_f64() * 1e6);
        std::hint::black_box(r);
    }
    (best_a, best_b)
}

#[test]
#[ignore]
fn radix_merge_vs_hashmap() {
    let mut rng = Rng(0x2026_1020_0000_00c3);
    println!("cell,n,item,radix_us,hashmap_us,ratio_radix_over_hash");
    let mut worst: f64 = 0.0;
    for &n in &[1_000usize, 10_000] {
        for (wname, reverse) in [("W2-shuffle", false), ("W2-reverse", true)] {
            let p = perm(&mut rng, n, reverse);
            let ids: Vec<u32> = (0..n as u32).collect();
            let strs: Vec<String> = (0..n)
                .map(|i| {
                    let mut s = format!("id-{i:08}-");
                    while s.len() < 128 {
                        s.push((b'a' + (i % 23) as u8) as char);
                    }
                    s
                })
                .collect();
            let reps = if n >= 10_000 { 41 } else { 201 };
            let (ra, rb) = bench(&ids, &p, reps);
            println!("{wname},{n},ItemId,{ra:.1},{rb:.1},{:.3}", ra / rb);
            worst = worst.max(ra / rb);
            let (sa, sb) = bench(&strs, &p, reps);
            println!("{wname},{n},String128,{sa:.1},{sb:.1},{:.3}", sa / sb);
            worst = worst.max(sa / sb);
        }
    }
    println!("worst ratio radix/hash = {worst:.3}  (1.10 초과 = 해시맵형이 1.10배 넘게 우세 → 판정례 (마) 선행)");
}
