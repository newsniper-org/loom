//! G0R1(개정 #12, 2026-10-20) — D·D-LIS·E·E-LIS의 key 표현·값 채널·매칭.
//!
//! G0 첫 실행(RQ1-dynamic-control-flow-scheduling.md "G0")이 이전 구현을 기각한 이유 셋을 이 모듈이 고친다.
//!
//! 1. **P1**: key가 `to_string()`으로 만든 `String`이었다 → 아이템 전체를 고정 hasher로 줄인 `u64`(`row_key`).
//! 2. **P3**: 매칭이 `Vec::position` + `remove`로 Θ(n·m)이었다 → 안정 LSD radix + 병합 조인(`match_by_key`).
//! 3. **정확성**: u64 key만으로는 64비트 충돌 때 잘못된 행을 재사용한다 → 행마다 값 채널 `RowValue<T>`를 두고
//!    key가 같은 후보를 `Item` 동등으로 확인한다. 충돌은 계수만 하고 결과는 항상 정확하다.
//!
//! 새 코드를 `list.rs`가 아니라 이 파일에 두는 이유: `list.rs`의 줄 번호를 인용하는 문서(§9 기록)가 많다.
//!
//! 정체성 의미가 바뀐다: "`to_string` 문자열 동등"에서 "`Item` Eq"로. 측정 타입(`ItemId`는 Display가 단사, `String`은
//! Display가 항등)에서는 둘이 같다. Display가 단사가 아닌 T에서는 의미가 다르며 의도한 차이다.
//! 전제: `a == b ⇒ row_key(a) == row_key(b)`, `cmp == Equal ⇔ ==` (derive를 쓰면 성립).

use std::hash::{BuildHasher, Hash};

use bevy_ecs::prelude::*;

/// D의 행 key. 아이템 전체의 고정 해시(`row_key`).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowKey(pub u64);

/// E의 행 key(`RowKey`와 같은 표현, 별도 타입으로 둔다 — D와 E는 같은 World에서 쓰이지 않지만 질의가 겹치지 않게).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderedKey(pub u64);

/// 값 채널. RQ1 "row entity에 값 채널(site별 newtype Signal)" 그대로 — 범용 `Signal<T>`가 아니다.
/// 범용 `Signal<String>`을 달면 `patch_input_system`·`patch_text_system::<String>`과 접근이 겹치고 교차 매칭해
/// 중복 `SetText`를 낼 수 있다(`RowBinding`이 이미 피했던 위험 — lib.rs). `RowValue<T>`는 어떤 기존 Query와도
/// 타입이 겹치지 않는다.
///
/// **N1 출처 특정**(판정례 8 준용): 들어가는 값은 spawn 또는 무조건 write 때의 `items.0[i].clone()`, 즉
/// `Signal<Vec<T>>` 원소뿐이다. DOM에서 되읽거나 `SetText` 페이로드를 캐시한 값이 아니다.
#[derive(Component, Debug, Clone)]
pub struct RowValue<T>(pub T);

/// key_op: 컴파일타임에 고정된 아이템 전체 해시(RQ1의 "u16 key_op(문법 제약 표현식)"를 대체 — 개정 #12 (ii)-2).
/// `FixedHasher`(foldhash fast, 고정 seed)라 같은 입력은 같은 출력이다. 버전·플랫폼 간 일관성은 보장하지 않는다
/// (foldhash README) — 정확성에는 영향이 없다(검증 경로가 있다). resume 직렬화 대상으로 쓰려면 버전 고정 +
/// 이식 가능한 key_op(또는 resume 때 재계산)가 선결 과제다.
#[inline]
pub fn row_key<T: Hash + ?Sized>(item: &T) -> u64 {
    bevy_platform::hash::FixedHasher.hash_one(item)
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for u64 {}
    #[cfg(test)]
    impl Sealed for super::tests::CntKey {}
}

/// 매칭 key. 비테스트 빌드의 구현체는 `u64` 하나다(P1-(i): 고정 폭 정수).
pub trait MatchKey: Copy + sealed::Sealed {
    fn radix(self) -> u64;
}
impl MatchKey for u64 {
    #[inline]
    fn radix(self) -> u64 {
        self
    }
}

/// `take_old[i] < m`이면 새 슬롯 `i`가 옛 인덱스 `take_old[i]`를 이어받는다(retained), `== m`이면 fresh.
/// 반환 계약은 이전 `match_keys`와 같다.
pub struct Selection {
    pub take_old: Vec<usize>,
    /// 64비트 key가 같은데 `Item`이 다른 run의 수(충돌). 결과의 정확성과 무관한 계측값.
    pub collisions: usize,
}

/// 안정 LSD radix(8비트 × 8패스) 정렬: `(key, 원래 인덱스)` 열을 key 오름차순으로, 같은 key 안에서는 인덱스 오름차순으로.
/// 비교 0회, O(n), 스크래치 O(n). 모든 원소의 해당 바이트가 같은 패스는 건너뛴다(안정성 불변).
fn radix_sorted<K: MatchKey>(keys: &[K]) -> Vec<(K, usize)> {
    let n = keys.len();
    let mut a: Vec<(K, usize)> = keys.iter().copied().enumerate().map(|(i, k)| (k, i)).collect();
    if n < 2 {
        return a;
    }
    let mut b = a.clone();
    for pass in 0..8u32 {
        let shift = pass * 8;
        let mut count = [0usize; 257];
        for &(k, _) in &a {
            count[((k.radix() >> shift) & 0xff) as usize + 1] += 1;
        }
        if count[1..].iter().any(|&c| c == n) {
            continue; // 이 바이트는 전부 같다 — 정렬에 기여하지 않는다
        }
        for d in 0..256 {
            count[d + 1] += count[d];
        }
        for &e in &a {
            let d = ((e.0.radix() >> shift) & 0xff) as usize;
            b[count[d]] = e;
            count[d] += 1;
        }
        std::mem::swap(&mut a, &mut b);
    }
    a
}

fn run_end<K: MatchKey>(v: &[(K, usize)], start: usize) -> usize {
    let k = v[start].0.radix();
    let mut e = start + 1;
    while e < v.len() && v[e].0.radix() == k {
        e += 1;
    }
    e
}

/// 충돌 run(같은 key에 서로 다른 `Item`이 섞임): `(Item, idx)`로 비교 정렬해 `Item` 동치류를 연속시킨 뒤,
/// 류마다 k번째 새 슬롯 ← k번째 옛 원소로 짝짓는다. **`Ord`의 선택은 류들의 나열 순서만 바꾼다** — 류 안의
/// 짝짓기(k번째 ↔ k번째)는 바꾸지 않으므로 `take_old`도 바뀌지 않는다.
fn pair_by_item_classes<K: MatchKey, T: Ord>(
    old_run: &[(K, usize)],
    old_items: &[&T],
    new_run: &[(K, usize)],
    new_items: &[&T],
    take_old: &mut [usize],
) {
    let mut o: Vec<usize> = old_run.iter().map(|&(_, j)| j).collect();
    let mut w: Vec<usize> = new_run.iter().map(|&(_, i)| i).collect();
    o.sort_by(|&x, &y| old_items[x].cmp(old_items[y]).then(x.cmp(&y)));
    w.sort_by(|&x, &y| new_items[x].cmp(new_items[y]).then(x.cmp(&y)));
    let (mut a, mut b) = (0, 0);
    while a < o.len() && b < w.len() {
        match old_items[o[a]].cmp(new_items[w[b]]) {
            std::cmp::Ordering::Less => {
                let c = old_items[o[a]];
                while a < o.len() && old_items[o[a]] == c {
                    a += 1;
                }
            }
            std::cmp::Ordering::Greater => {
                let c = new_items[w[b]];
                while b < w.len() && new_items[w[b]] == c {
                    b += 1;
                }
            }
            std::cmp::Ordering::Equal => {
                let c = old_items[o[a]];
                let (mut ae, mut be) = (a, b);
                while ae < o.len() && old_items[o[ae]] == c {
                    ae += 1;
                }
                while be < w.len() && new_items[w[be]] == c {
                    be += 1;
                }
                for (&i, &j) in w[b..be].iter().zip(&o[a..ae]) {
                    take_old[i] = j;
                }
                a = ae;
                b = be;
            }
        }
    }
}

/// key 열 두 개를 매칭한다. 입력 key는 flat POD(`&[K]`)이고 `&[&T]`는 값 채널이다(부모-자식·중첩 표현 없음).
///
/// - 충돌이 없으면 비교 정렬을 하지 않아 O(n+m)이다. `Item` 비교는 run마다 `|옛 run|-1 + |새 run|`회.
/// - 64비트 충돌 run이 있을 때만 그 run 안에서 `Item` 비교 정렬을 한다 → 최악 O(N log N).
/// - 해시 테이블을 쓰지 않으므로 순회 순서 비결정성이 출력에 새는 경로가 없다.
pub fn match_by_key<K: MatchKey, T: Ord>(
    old_keys: &[K],
    old_items: &[&T],
    new_keys: &[K],
    new_items: &[&T],
) -> Selection {
    debug_assert_eq!(old_keys.len(), old_items.len());
    debug_assert_eq!(new_keys.len(), new_items.len());
    let (m, n) = (old_keys.len(), new_keys.len());
    let old = radix_sorted(old_keys);
    let new = radix_sorted(new_keys);
    let mut take_old = vec![m; n];
    let mut collisions = 0;
    let (mut a, mut b) = (0, 0);
    while a < m && b < n {
        let (ka, kb) = (old[a].0.radix(), new[b].0.radix());
        if ka < kb {
            a = run_end(&old, a); // 옛쪽에만 있는 key → 제거
            continue;
        }
        if kb < ka {
            b = run_end(&new, b); // 새쪽에만 있는 key → fresh
            continue;
        }
        let (ae, be) = (run_end(&old, a), run_end(&new, b));
        let rep = old_items[old[a].1];
        let uniform = old[a + 1..ae].iter().all(|&(_, j)| *old_items[j] == *rep)
            && new[b..be].iter().all(|&(_, i)| *new_items[i] == *rep);
        if uniform {
            // k번째 새 슬롯(i 오름차순) ← k번째 옛 원소(j 오름차순). 남는 새 슬롯은 fresh.
            for (&(_, i), &(_, j)) in new[b..be].iter().zip(&old[a..ae]) {
                take_old[i] = j;
            }
        } else {
            collisions += 1;
            pair_by_item_classes(&old[a..ae], old_items, &new[b..be], new_items, &mut take_old);
        }
        a = ae;
        b = be;
    }
    Selection { take_old, collisions }
}

/// debug 빌드에서만 누적되는 충돌 계수기(ECS 접근이 아니므로 스케줄에 영향이 없다). 개정 #12 (ii)-5가
/// §10 E 행의 "디버그 충돌 assert"를 이것으로 바꾼다 — 정확성은 검증 경로가 보장하므로 assert가 지킬 대상이 없다.
#[cfg(debug_assertions)]
pub static KEY_COLLISIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[inline]
pub fn note_collisions(_n: usize) {
    #[cfg(debug_assertions)]
    if _n > 0 {
        KEY_COLLISIONS.fetch_add(_n, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::Cell;

    // ---- 오라클: HEAD(0ff9bd5)의 `list.rs::match_keys`를 시그니처와 본문 그대로 보존(이력) -------------------
    pub(crate) mod legacy {
        use bevy_ecs::prelude::Entity;
        pub fn match_keys(old_order: &[(String, Entity)], new_keys: &[String]) -> Vec<usize> {
            let mut pool: Vec<(String, usize)> = old_order
                .iter()
                .enumerate()
                .map(|(j, (key, _))| (key.clone(), j))
                .collect();
            let mut take_old = Vec::with_capacity(new_keys.len());
            for key in new_keys {
                if let Some(pos) = pool.iter().position(|(k, _)| k == key) {
                    let (_, j) = pool.remove(pos);
                    take_old.push(j);
                } else {
                    take_old.push(old_order.len());
                }
            }
            take_old
        }
    }

    thread_local! {
        pub(crate) static CMP: Cell<usize> = const { Cell::new(0) };
        pub(crate) static KEY_ACCESS: Cell<usize> = const { Cell::new(0) };
        pub(crate) static ITEM_CMP: Cell<usize> = const { Cell::new(0) };
    }
    fn reset() {
        CMP.set(0);
        KEY_ACCESS.set(0);
        ITEM_CMP.set(0);
    }

    /// 계수기용 제네릭판: 소스 수준에서 `legacy::match_keys`와 같은 알고리즘이고 비교만 센다.
    /// (운영 `String` 인스턴스와는 다른 기계어다 — 동일한 것은 소스 수준뿐이다.)
    fn match_keys_counting<K: PartialEq>(old: &[K], new: &[K]) -> Vec<usize> {
        let mut pool: Vec<(&K, usize)> = old.iter().enumerate().map(|(j, k)| (k, j)).collect();
        let mut take_old = Vec::with_capacity(new.len());
        for key in new {
            if let Some(pos) = pool.iter().position(|(k, _)| *k == key) {
                let (_, j) = pool.remove(pos);
                take_old.push(j);
            } else {
                take_old.push(old.len());
            }
        }
        take_old
    }

    /// 비교를 세는 key(옛 매처용).
    #[derive(Clone, Copy)]
    struct CK(u64);
    impl PartialEq for CK {
        fn eq(&self, o: &Self) -> bool {
            CMP.set(CMP.get() + 1);
            self.0 == o.0
        }
    }

    /// 접근 횟수를 세는 매칭 key(새 매처용). `radix()` 호출 횟수가 곧 key 접근 수다.
    #[derive(Clone, Copy)]
    pub(crate) struct CntKey(pub u64);
    impl MatchKey for CntKey {
        fn radix(self) -> u64 {
            KEY_ACCESS.set(KEY_ACCESS.get() + 1);
            self.0
        }
    }

    /// 비교를 세는 아이템.
    #[derive(Clone, Debug)]
    struct CntItem(u32);
    impl PartialEq for CntItem {
        fn eq(&self, o: &Self) -> bool {
            ITEM_CMP.set(ITEM_CMP.get() + 1);
            self.0 == o.0
        }
    }
    impl Eq for CntItem {}
    impl PartialOrd for CntItem {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for CntItem {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            ITEM_CMP.set(ITEM_CMP.get() + 1);
            self.0.cmp(&o.0)
        }
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() as usize) % n.max(1)
        }
    }

    fn ent() -> bevy_ecs::prelude::Entity {
        bevy_ecs::prelude::Entity::PLACEHOLDER
    }

    // 퇴화 해시들 — 모두 `a == b ⇒ hash(a) == hash(b)`를 지키면서 충돌 경로를 탄다.
    fn h_real(s: &str) -> u64 {
        row_key(s)
    }
    fn h_const(_: &str) -> u64 {
        0
    }
    fn h_low1(s: &str) -> u64 {
        row_key(s) & 1
    }
    fn h_len(s: &str) -> u64 {
        s.len() as u64
    }

    fn gen_len(rng: &mut Rng, kind: u8, max: usize) -> Vec<String> {
        let len = rng.below(max + 1);
        gen_strings(rng, kind, len)
    }

    fn gen_strings(rng: &mut Rng, kind: u8, len: usize) -> Vec<String> {
        (0..len)
            .map(|_| match kind {
                0 => format!("id-{}", rng.below(12)),
                1 => ["a", "b", "ab", ""][rng.below(4)].to_string(),
                _ => {
                    let c = (b'a' + rng.below(5) as u8) as char;
                    std::iter::repeat_n(c, 128).collect()
                }
            })
            .collect()
    }

    /// 한 입력 쌍에서 오라클과 새 매처가 같은 `take_old`를 내는지 보고, 충돌 run 수를 돌려준다.
    fn check_pair(old: &[String], new: &[String], hash: fn(&str) -> u64) -> usize {
        let old_order: Vec<(String, _)> = old.iter().map(|s| (s.clone(), ent())).collect();
        let want = legacy::match_keys(&old_order, new);
        let ok: Vec<u64> = old.iter().map(|s| hash(s)).collect();
        let nk: Vec<u64> = new.iter().map(|s| hash(s)).collect();
        let oi: Vec<&String> = old.iter().collect();
        let ni: Vec<&String> = new.iter().collect();
        let got = match_by_key(&ok, &oi, &nk, &ni);
        assert_eq!(got.take_old, want, "old={old:?} new={new:?}");
        got.collisions
    }

    #[test]
    fn match_equivalence_against_legacy_oracle() {
        let mut rng = Rng(0x2026_1020_0000_0001);
        let hashes: [(&str, fn(&str) -> u64); 4] =
            [("real", h_real), ("const0", h_const), ("low1", h_low1), ("len", h_len)];
        let (mut dup, mut fresh, mut removed, mut collided, mut cases) = (0, 0, 0, 0, 0);
        for kind in 0..3u8 {
            for &(name, h) in &hashes {
                for _ in 0..500 {
                    let old = gen_len(&mut rng, kind, 40);
                    let new = gen_len(&mut rng, kind, 40);
                    let c = check_pair(&old, &new, h);
                    cases += 1;
                    if name != "real" && c > 0 {
                        collided += 1;
                    }
                    let mut seen = std::collections::HashSet::new();
                    if new.iter().any(|s| !seen.insert(s)) {
                        dup += 1;
                    }
                    if new.iter().any(|s| !old.contains(s)) {
                        fresh += 1;
                    }
                    if old.iter().any(|s| !new.contains(s)) {
                        removed += 1;
                    }
                    // 연쇄 3스텝: 다음 old = 이번 new
                    let n2 = gen_len(&mut rng, kind, 40);
                    check_pair(&new, &n2, h);
                    let n3 = gen_len(&mut rng, kind, 40);
                    check_pair(&n2, &n3, h);
                }
            }
        }
        // 존재 단언(공허한 확인 방지) — 동치 단언은 이 사례들 위에서만 의미가 있다.
        assert!(cases > 5000, "{cases}");
        assert!(dup > 100, "중복 key 사례가 없다: {dup}");
        assert!(fresh > 100, "fresh 사례가 없다: {fresh}");
        assert!(removed > 100, "제거 사례가 없다: {removed}");
        assert!(collided > 100, "충돌 run 사례가 없다: {collided}");
    }

    #[test]
    fn counting_oracle_agrees_with_legacy() {
        let mut rng = Rng(0x2026_1020_0000_0002);
        for _ in 0..500 {
            let old = gen_len(&mut rng, 1, 29);
            let new = gen_len(&mut rng, 1, 29);
            let old_order: Vec<(String, _)> = old.iter().map(|s| (s.clone(), ent())).collect();
            assert_eq!(match_keys_counting(&old, &new), legacy::match_keys(&old_order, &new));
        }
    }

    #[test]
    fn counter_wiring_exists_and_old_matcher_is_quadratic() {
        // 배선 존재 단언: 역순 n=200에서 옛 매처의 비교 수는 정확히 n(n+1)/2 = 20,100.
        let n = 200u64;
        let old: Vec<CK> = (0..n).map(CK).collect();
        let new: Vec<CK> = (0..n).rev().map(CK).collect();
        reset();
        let _ = match_keys_counting(&old, &new);
        assert_eq!(CMP.get(), 20_100);
    }

    fn run_new(old: &[u32], new: &[u32], hash: impl Fn(u32) -> u64) -> (Selection, usize, usize) {
        let oi: Vec<CntItem> = old.iter().map(|&x| CntItem(x)).collect();
        let ni: Vec<CntItem> = new.iter().map(|&x| CntItem(x)).collect();
        let ok: Vec<CntKey> = old.iter().map(|&x| CntKey(hash(x))).collect();
        let nk: Vec<CntKey> = new.iter().map(|&x| CntKey(hash(x))).collect();
        let or: Vec<&CntItem> = oi.iter().collect();
        let nr: Vec<&CntItem> = ni.iter().collect();
        reset();
        let sel = match_by_key(&ok, &or, &nk, &nr);
        (sel, KEY_ACCESS.get(), ITEM_CMP.get())
    }

    fn real_hash(x: u32) -> u64 {
        row_key(&x)
    }

    #[test]
    fn new_matcher_is_linear_and_item_compares_are_tight() {
        // 역순·무충돌: Item 비교는 하한·상한 모두 정확히 n(run 크기 1/1).
        for &n in &[1000u32, 2000, 4000] {
            let old: Vec<u32> = (0..n).collect();
            let new: Vec<u32> = (0..n).rev().collect();
            let (sel, access, item) = run_new(&old, &new, real_hash);
            assert_eq!(sel.collisions, 0, "해시가 64비트 충돌을 냈다(n={n}) — 시드가 바뀌었나");
            assert_eq!(item, n as usize, "역순 무충돌의 Item 비교 수");
            assert!(access > 0, "key 접근 계수기가 배선되지 않았다");
            assert!(access <= 40 * (2 * n as usize), "key 접근 {access}가 선형 상한을 넘는다");
            // take_old가 역순 대응이다
            assert_eq!(sel.take_old[0], (n - 1) as usize);
        }
        // 증가율: n 두 배 → key 접근 비율 ≤ 2.4 (옛 매처는 ≥ 3.9)
        let a = |n: u32| {
            let old: Vec<u32> = (0..n).collect();
            let new: Vec<u32> = (0..n).rev().collect();
            run_new(&old, &new, real_hash).1 as f64
        };
        assert!(a(2000) / a(1000) <= 2.4 && a(4000) / a(2000) <= 2.4);
        // 옛 매처의 대조: 같은 입력에서 비율 ≥ 3.9
        let o = |n: u64| {
            let old: Vec<CK> = (0..n).map(CK).collect();
            let new: Vec<CK> = (0..n).rev().map(CK).collect();
            reset();
            let _ = match_keys_counting(&old, &new);
            CMP.get() as f64
        };
        assert!(o(2000) / o(1000) >= 3.9 && o(4000) / o(2000) >= 3.9);
    }

    #[test]
    fn new_matcher_is_linear_on_shuffle_and_w7_like_inputs() {
        let mut rng = Rng(0x2026_1020_0000_0003);
        let shuffle = |rng: &mut Rng, n: u32| {
            let mut v: Vec<u32> = (0..n).collect();
            for i in (1..v.len()).rev() {
                let j = rng.below(i + 1);
                v.swap(i, j);
            }
            v
        };
        let mut prev: Option<f64> = None;
        for &n in &[1000u32, 2000, 4000] {
            let old: Vec<u32> = (0..n).collect();
            let new = shuffle(&mut rng, n);
            let (sel, access, item) = run_new(&old, &new, real_hash);
            assert_eq!(sel.collisions, 0);
            assert!(item <= 2 * n as usize, "무충돌 Item 비교 ≤ n+m: {item}");
            if let Some(p) = prev {
                assert!(access as f64 / p <= 2.4, "key 접근 증가율 {}", access as f64 / p);
            }
            prev = Some(access as f64);
        }
        // W7형: 값의 10%를 새 값으로 바꾼다(그 key는 fresh) — 여전히 선형.
        let mut prev: Option<f64> = None;
        for &n in &[1000u32, 2000, 4000] {
            let old: Vec<u32> = (0..n).collect();
            let mut new = old.clone();
            for _ in 0..(n / 10) {
                let i = rng.below(n as usize);
                new[i] = n + rng.below(1_000_000) as u32;
            }
            let (sel, access, _) = run_new(&old, &new, real_hash);
            assert!(sel.take_old.iter().any(|&t| t == n as usize), "fresh가 없다");
            if let Some(p) = prev {
                assert!(access as f64 / p <= 2.4);
            }
            prev = Some(access as f64);
        }
    }

    #[test]
    fn degenerate_hash_collides_first_then_stays_n_log_n() {
        // 상수 해시: 모든 원소가 한 run — 충돌 경로. 먼저 존재 단언.
        for &n in &[1000u32, 2000] {
            let old: Vec<u32> = (0..n).collect();
            let new: Vec<u32> = (0..n).rev().collect();
            let (sel, _, item) = run_new(&old, &new, |_| 0);
            assert!(sel.collisions > 0, "충돌 경로가 실행되지 않았다");
            let bound = 2 * (n as usize) * ((64 - (n as u64 + 1).leading_zeros()) as usize + 2) * 2;
            assert!(item <= bound, "충돌 run Item 비교 {item} > {bound}");
            assert_eq!(sel.take_old[0], (n - 1) as usize);
        }
    }
}
