//! RQ5 후보 #4 — flush 규약 v0의 순서 보장. **검증된 코드 = 배포되는 코드.**
//!
//! `formal-verification/verus/flush_order.rs`가 증명한 정리들(anchor 선행성,
//! 순서 수렴, 스킵 안전성, Chain↔adjacent 대응)의 대상을 **모델에서 실제
//! 실행 코드로 옮긴 것**이다. `crates/loom-runtime/src/list.rs`의
//! `place_in_order`가 이 크레이트의 `Chain`을 호출한다 — 즉 여기 있는 코드가
//! 곧 배포되는 코드이고, 동시에 `cargo verus verify`가 검증하는 코드다.
//!
//! **왜 별도 크레이트인가**: `cargo verus verify`는 크레이트 전체를 검증
//! 대상으로 보는데 `loom-runtime`은 `bevy_ecs`/`web-sys`와 얽혀 있어 Verus가
//! 추론할 수 없는 코드가 대부분이다. 순수 알고리즘만 분리하면 그 전체를
//! 검증할 수 있다.
//!
//! **id 표현**: `bevy_ecs::entity::Entity`는 Verus가 다룰 수 없는 불투명
//! 타입이므로, 호출부가 `Entity::to_bits()`로 `u64`를 만들어 넘긴다. 그
//! 변환은 단사(injective)이므로 동일성·구별성이 보존된다.

#![cfg_attr(verus_keep_ghost, allow(unused_imports))]

use vstd::prelude::*;
use vstd::hash_map::*;
// `seq_lib`/`std_specs`는 **검증 시에만 존재하는 ghost 모듈**이다 — 일반
// `cargo build`에서는 컴파일에서 지워지므로 `use`가 남아 있으면 에러가 난다.
// Verus가 검증 중에만 켜는 `verus_only` cfg로 가드한다(생성 템플릿의 안내
// 그대로). `prelude`/`hash_map`은 `HashMapWithView`라는 실제 런타임 타입을
// 제공하므로 가드하지 않는다.
#[cfg(verus_only)]
use vstd::seq_lib::*;
#[cfg(verus_only)]
use vstd::std_specs::hash::*;

verus! {

/// `Chain`의 두 맵 + head/tail이 **어떤 시퀀스를 표기하는가**의 규정.
/// `formal-verification/verus/flush_order.rs`의 동명 술어와 같은 내용이며,
/// 여기서는 실제 `HashMapWithView`의 뷰에 대해 쓴다.
pub open spec fn chain_denotes(
    next: Map<u64, u64>,
    prev: Map<u64, u64>,
    head: Option<u64>,
    tail: Option<u64>,
    s: Seq<u64>,
) -> bool {
    &&& s.no_duplicates()
    &&& (if s.len() == 0 { head is None } else { head == Some(s[0]) })
    &&& (if s.len() == 0 { tail is None } else { tail == Some(s[s.len() - 1]) })
    &&& forall|i: int| #![trigger s[i]]
            0 <= i < s.len() - 1 ==> next.contains_key(s[i]) && next[s[i]] == s[i + 1]
    &&& !(s.len() > 0 && next.contains_key(s[s.len() - 1]))
    &&& forall|i: int| #![trigger s[i]]
            0 < i < s.len() ==> prev.contains_key(s[i]) && prev[s[i]] == s[i - 1]
    &&& !(s.len() > 0 && prev.contains_key(s[0]))
    &&& forall|k: u64| #![trigger next.contains_key(k)]
            next.contains_key(k) ==> exists|i: int| 0 <= i < s.len() - 1 && s[i] == k
    &&& forall|k: u64| #![trigger prev.contains_key(k)]
            prev.contains_key(k) ==> exists|i: int| 0 < i < s.len() && s[i] == k
}

/// `d` 안에서 `e`가 `a` 바로 앞에 있는가 — `already_adjacent` 판정의 의미.
pub open spec fn adjacent(d: Seq<u64>, e: u64, a: u64) -> bool {
    exists|i: int| 0 <= i < d.len() - 1 && #[trigger] d[i] == e && d[i + 1] == a
}

/// `list.rs`의 `Chain`을 그대로 옮긴 것 — 다만 표기 시퀀스를 **고스트 필드로
/// 함께 들고 다닌다**. 런타임 표현은 여전히 두 개의 해시맵 + head/tail이므로
/// 성능 특성이 원본과 같고(§2.5 관심사), 고스트는 컴파일 시점에 지워진다.
pub struct Chain {
    next: HashMapWithView<u64, u64>,
    prev: HashMapWithView<u64, u64>,
    head: Option<u64>,
    tail: Option<u64>,
    /// 이 chain이 표기하는 순서. 실행에는 존재하지 않는다.
    /// private인 이유: 필드가 하나라도 private이면 Verus는 그 데이터타입을
    /// 불투명으로 보아 **공개 계약(`ensures`) 안에서 필드 접근을 금지**한다.
    /// 그래서 표기는 필드가 아니라 아래 `denotes()` 접근자로 노출한다.
    denotation: Ghost<Seq<u64>>,
}

impl Chain {
    /// 이 chain이 표기하는 순서 — 공개 계약에서 쓰는 접근자.
    pub closed spec fn denotes(&self) -> Seq<u64> {
        self.denotation@
    }

    /// 표현과 의미가 어긋나지 않았다는 불변식. `closed`인 이유: 본문이
    /// private 필드(`next`/`prev`/`head`/`tail`)를 참조하므로 공개할 수 없고,
    /// 공개할 필요도 없다 — 호출부는 `new()`의 사후조건으로 이것을 얻고
    /// 각 메서드의 사후조건으로 유지만 하면 된다(캡슐화).
    pub closed spec fn well_formed(&self) -> bool {
        chain_denotes(self.next@, self.prev@, self.head, self.tail, self.denotation@)
    }

    /// 빈 chain.
    pub fn new() -> (r: Chain)
        ensures r.well_formed(), r.denotes() =~= Seq::<u64>::empty(),
    {
        broadcast use group_hash_axioms;
        Chain {
            next: HashMapWithView::new(),
            prev: HashMapWithView::new(),
            head: None,
            tail: None,
            denotation: Ghost(Seq::empty()),
        }
    }

    /// `already_adjacent`의 anchor 있는 갈래 — 원본의
    /// `chain.next_of(e) == Some(a)`.
    ///
    /// 사후조건이 곧 **Chain↔adjacent 대응(질의 방향)**이다: O(1) 맵 조회의
    /// 결과가 표기 시퀀스상의 인접성과 동치임을 여기서 못박는다.
    pub fn next_is(&self, e: u64, a: u64) -> (r: bool)
        requires self.well_formed(),
        ensures r == adjacent(self.denotes(), e, a),
    {
        let res = match self.next.get(&e) {
            Some(n) => *n == a,
            None => false,
        };
        proof {
            let d = self.denotation@;
            if adjacent(d, e, a) {
                let i = choose|i: int| 0 <= i < d.len() - 1 && #[trigger] d[i] == e && d[i + 1] == a;
                assert(self.next@.contains_key(d[i]));
                assert(self.next@[d[i]] == d[i + 1]);
            }
            if self.next@.contains_key(e) && self.next@[e] == a {
                let i = choose|i: int| 0 <= i < d.len() - 1 && d[i] == e;
                assert(d[i] == e);
                assert(self.next@[d[i]] == d[i + 1]);
                assert(d[i + 1] == a);
                assert(adjacent(d, e, a));
            }
        }
        res
    }


    /// 원본 `Chain::unlink`와 **같은 코드**. 사후조건이 표기 수준의 효과를
    /// 못박는다: 표기가 `s.remove_value(e)`가 된다.
    pub fn unlink(&mut self, e: u64)
        requires
            old(self).well_formed(),
            old(self).denotes().contains(e),
        ensures
            final(self).well_formed(),
            final(self).denotes() =~= old(self).denotes().remove_value(e),
    {
        proof { chain_unlink_denotes_remove(self.next@, self.prev@, self.head, self.tail, self.denotation@, e); }
        let p = self.prev.remove(&e);
        let n = self.next.remove(&e);
        match (p, n) {
            (Some(p), Some(n)) => {
                self.next.insert(p, n);
                self.prev.insert(n, p);
            }
            (Some(p), None) => {
                self.next.remove(&p);
                self.tail = Some(p);
            }
            (None, Some(n)) => {
                self.prev.remove(&n);
                self.head = Some(n);
            }
            (None, None) => {
                self.head = None;
                self.tail = None;
            }
        }
        self.denotation = Ghost(self.denotation@.remove_value(e));
    }

    /// 원본 `Chain::insert_before`의 anchor 있는 갈래와 **같은 코드**.
    /// 표기가 `s.insert(j, e)`(j = a의 위치)가 된다.
    pub fn insert_before(&mut self, e: u64, a: u64)
        requires
            old(self).well_formed(),
            !old(self).denotes().contains(e),
            old(self).denotes().contains(a),
        ensures
            final(self).well_formed(),
            final(self).denotes() =~= old(self).denotes().insert(
                old(self).denotes().index_of_first(a).unwrap(), e),
    {
        proof {
            self.denotation@.index_of_first_ensures(a);
            let j = self.denotation@.index_of_first(a).unwrap();
            chain_insert_before_denotes_insert(
                self.next@, self.prev@, self.head, self.tail, self.denotation@, e, a, j);
        }
        // `Option::copied`도 ref 패턴도 Verus 미지원이라 역참조로 푼다(동작 동일).
        let pa = self.prev.get(&a);
        if pa.is_some() {
            let p = *pa.unwrap();
            self.next.insert(p, e);
            self.prev.insert(e, p);
        } else {
            self.head = Some(e);
        }
        self.next.insert(e, a);
        self.prev.insert(a, e);
        self.denotation = Ghost(
            self.denotation@.insert(self.denotation@.index_of_first(a).unwrap(), e));
    }


    /// 꼬리에 덧붙이기 — 원본 `insert_before(e, None)` 갈래와 같은 코드.
    pub fn push_back(&mut self, e: u64)
        requires old(self).well_formed(), !old(self).denotes().contains(e),
        ensures final(self).well_formed(), final(self).denotes() =~= old(self).denotes().push(e),
    {
        proof { chain_push_back_denotes(self.next@, self.prev@, self.head, self.tail, self.denotation@, e); }
        match self.tail {
            Some(t) => {
                self.next.insert(t, e);
                self.prev.insert(e, t);
            }
            None => self.head = Some(e),
        }
        self.tail = Some(e);
        self.denotation = Ghost(self.denotation@.push(e));
    }

    /// 원본 `Chain::from_slice` — 현재 순서로 chain을 만든다.
    pub fn from_slice(items: &Vec<u64>) -> (r: Chain)
        requires items@.no_duplicates(),
        ensures r.well_formed(), r.denotes() =~= items@,
    {
        let mut c = Chain::new();
        let mut i: usize = 0;
        while i < items.len()
            invariant
                i <= items.len(),
                items@.no_duplicates(),
                c.well_formed(),
                c.denotes() =~= items@.subrange(0, i as int),
            decreases items.len() - i,
        {
            proof {
                assert(!items@.subrange(0, i as int).contains(items@[i as int])) by {
                    assert forall|j: int| 0 <= j < i implies items@.subrange(0, i as int)[j] != items@[i as int] by {
                        assert(items@.subrange(0, i as int)[j] == items@[j]);
                    }
                }
            }
            c.push_back(items[i]);
            proof {
                assert(items@.subrange(0, i as int).push(items@[i as int])
                       =~= items@.subrange(0, i as int + 1));
            }
            i = i + 1;
        }
        proof { assert(items@.subrange(0, items@.len() as int) =~= items@); }
        c
    }

    /// `already_adjacent`의 anchor 없는 갈래 — 원본의 `chain.is_tail(e)`.
    pub fn is_tail(&self, e: u64) -> (r: bool)
        requires self.well_formed(),
        ensures r == (self.denotes().len() > 0 && self.denotes()[self.denotes().len() - 1] == e),
    {
        match self.tail {
            Some(t) => t == e,
            None => false,
        }
    }

}

/// **L5**: 중복 없는 시퀀스에서 첫 occurrence를 지우면 그 값은 사라지고,
/// 중복 없음도 유지된다.
pub proof fn lemma_remove_value_no_dup(s: Seq<u64>, e: u64)
    requires s.no_duplicates(),
    ensures
        s.remove_value(e).no_duplicates(),
        !s.remove_value(e).contains(e),
{
    s.index_of_first_ensures(e);
    if let Some(i) = s.index_of_first(e) {
        s.remove_ensures(i);
        assert forall|j: int| 0 <= j < s.remove(i).len() implies s.remove(i)[j] != e by {
            if j < i { assert(s.remove(i)[j] == s[j]); assert(s[j] != s[i]); }
            else { assert(s.remove(i)[j] == s[j + 1]); assert(s[j + 1] != s[i]); }
        }
        assert forall|j: int, l: int|
            0 <= j < s.remove(i).len() && 0 <= l < s.remove(i).len() && j != l
            implies s.remove(i)[j] != s.remove(i)[l] by {
            let jj = if j < i { j } else { j + 1 };
            let ll = if l < i { l } else { l + 1 };
            assert(s.remove(i)[j] == s[jj]);
            assert(s.remove(i)[l] == s[ll]);
            assert(jj != ll);
        }
    }
}


/// `Chain::unlink`의 맵 조작을 그대로 옮긴 것(`list.rs` 참고).
/// 네 갈래는 원본의 `match (p, n)`과 1:1이다.
pub open spec fn unlink_next(next: Map<u64, u64>, prev: Map<u64, u64>, e: u64) -> Map<u64, u64> {
    let has_p = prev.contains_key(e);
    let has_n = next.contains_key(e);
    let n0 = next.remove(e);
    if has_p && has_n {
        n0.insert(prev[e], next[e])
    } else if has_p && !has_n {
        n0.remove(prev[e])
    } else {
        n0
    }
}

pub open spec fn unlink_prev(next: Map<u64, u64>, prev: Map<u64, u64>, e: u64) -> Map<u64, u64> {
    let has_p = prev.contains_key(e);
    let has_n = next.contains_key(e);
    let p0 = prev.remove(e);
    if has_p && has_n {
        p0.insert(next[e], prev[e])
    } else if !has_p && has_n {
        p0.remove(next[e])
    } else {
        p0
    }
}

pub open spec fn unlink_head(next: Map<u64, u64>, prev: Map<u64, u64>, head: Option<u64>, e: u64) -> Option<u64> {
    let has_p = prev.contains_key(e);
    let has_n = next.contains_key(e);
    if !has_p && has_n { Some(next[e]) }
    else if !has_p && !has_n { None }
    else { head }
}

pub open spec fn unlink_tail(next: Map<u64, u64>, prev: Map<u64, u64>, tail: Option<u64>, e: u64) -> Option<u64> {
    let has_p = prev.contains_key(e);
    let has_n = next.contains_key(e);
    if has_p && !has_n { Some(prev[e]) }
    else if !has_p && !has_n { None }
    else { tail }
}

/// **Chain ↔ adjacent 대응 (변이 방향, unlink)**: 표기 `s`인 chain에서
/// `e`를 unlink하면 표기가 `s.remove_value(e)`가 된다.
pub proof fn chain_unlink_denotes_remove(
    next: Map<u64, u64>,
    prev: Map<u64, u64>,
    head: Option<u64>,
    tail: Option<u64>,
    s: Seq<u64>,
    e: u64,
)
    requires
        chain_denotes(next, prev, head, tail, s),
        s.contains(e),
    ensures
        chain_denotes(
            unlink_next(next, prev, e),
            unlink_prev(next, prev, e),
            unlink_head(next, prev, head, e),
            unlink_tail(next, prev, tail, e),
            s.remove_value(e),
        ),
{
    s.index_of_first_ensures(e);
    let i = s.index_of_first(e).unwrap();
    assert(s[i] == e);
    let t = s.remove(i);
    assert(s.remove_value(e) =~= t);
    s.remove_ensures(i);

    // e의 앞/뒤 존재 여부는 곧 i의 위치다.
    assert(prev.contains_key(e) <==> i > 0) by {
        if i > 0 { assert(prev.contains_key(s[i])); }
        if i == 0 { assert(!prev.contains_key(s[0])); }
    }
    assert(next.contains_key(e) <==> i < s.len() - 1) by {
        if i < s.len() - 1 { assert(next.contains_key(s[i])); }
        if i == s.len() - 1 { assert(!next.contains_key(s[s.len() - 1])); }
    }
    assert(t.no_duplicates()) by {
        assert forall|j: int, l: int|
            0 <= j < t.len() && 0 <= l < t.len() && j != l implies t[j] != t[l] by {
            let jj = if j < i { j } else { j + 1 };
            let ll = if l < i { l } else { l + 1 };
            assert(t[j] == s[jj]);
            assert(t[l] == s[ll]);
            assert(jj != ll);
        }
    }

    let n2 = unlink_next(next, prev, e);
    let p2 = unlink_prev(next, prev, e);
    let h2 = unlink_head(next, prev, head, e);
    let t2 = unlink_tail(next, prev, tail, e);

    // head/tail
    assert(if t.len() == 0 { h2 is None } else { h2 == Some(t[0]) });
    assert(if t.len() == 0 { t2 is None } else { t2 == Some(t[t.len() - 1]) });

    // next 방향
    assert forall|j: int| #![trigger t[j]]
        0 <= j < t.len() - 1 implies n2.contains_key(t[j]) && n2[t[j]] == t[j + 1] by {
        if j < i - 1 {
            assert(t[j] == s[j] && t[j + 1] == s[j + 1]);
            assert(next.contains_key(s[j]) && next[s[j]] == s[j + 1]);
            assert(s[j] != s[i]);
        } else if j == i - 1 {
            assert(t[j] == s[i - 1] && t[j + 1] == s[i + 1]);
        } else {
            assert(t[j] == s[j + 1] && t[j + 1] == s[j + 2]);
            assert(next.contains_key(s[j + 1]) && next[s[j + 1]] == s[j + 2]);
            assert(s[j + 1] != s[i]);
        }
    }
    assert(!(t.len() > 0 && n2.contains_key(t[t.len() - 1]))) by {
        if t.len() > 0 {
            if i < s.len() - 1 {
                // e가 마지막이 아니면 t의 끝은 그대로 s의 끝이고,
                // next는 원래 그 키를 갖지 않았다. 삽입 키 s[i-1]도 그것과 다르다.
                assert(t[t.len() - 1] == s[s.len() - 1]);
                assert(!next.contains_key(s[s.len() - 1]));
            } else {
                // e가 마지막이면 t의 끝은 s[i-1]이고, unlink가 그 키를 지웠다.
                assert(t[t.len() - 1] == s[i - 1]);
            }
        }
    }

    // prev 방향
    assert forall|j: int| #![trigger t[j]]
        0 < j < t.len() implies p2.contains_key(t[j]) && p2[t[j]] == t[j - 1] by {
        if j < i {
            assert(t[j] == s[j] && t[j - 1] == s[j - 1]);
            assert(prev.contains_key(s[j]) && prev[s[j]] == s[j - 1]);
            assert(s[j] != s[i]);
        } else if j == i {
            assert(t[j] == s[i + 1] && t[j - 1] == s[i - 1]);
        } else {
            assert(t[j] == s[j + 1] && t[j - 1] == s[j]);
            assert(prev.contains_key(s[j + 1]) && prev[s[j + 1]] == s[j]);
            assert(s[j + 1] != s[i]);
        }
    }
    assert(!(t.len() > 0 && p2.contains_key(t[0]))) by {
        if t.len() > 0 {
            if i > 0 {
                // e가 첫 원소가 아니면 t의 머리는 그대로 s[0]이고,
                // prev는 원래 그 키를 갖지 않았다. 삽입 키 s[i+1]도 그것과 다르다.
                assert(t[0] == s[0]);
                assert(!prev.contains_key(s[0]));
            } else {
                // e가 첫 원소면 t의 머리는 s[1]이고, unlink가 그 키를 지웠다.
                assert(t[0] == s[1]);
            }
        }
    }

    // 정의역이 새지 않음
    assert forall|k: u64| #![trigger p2.contains_key(k)]
        p2.contains_key(k) implies exists|j: int| 0 < j < t.len() && t[j] == k by {
        if p2.contains_key(k) {
            if i > 0 && i < s.len() - 1 && k == s[i + 1] {
                assert(t[i] == k);
                assert(0 < i < t.len());
            } else {
                let j0 = choose|j0: int| 0 < j0 < s.len() && s[j0] == k;
                assert(s[j0] == k);
                assert(j0 != i);
                let j = if j0 < i { j0 } else { j0 - 1 };
                assert(t[j] == k);
                assert(0 < j < t.len());
            }
        }
    }
    assert forall|k: u64| #![trigger n2.contains_key(k)]
        n2.contains_key(k) implies exists|j: int| 0 <= j < t.len() - 1 && t[j] == k by {
        if n2.contains_key(k) {
            if i > 0 && i < s.len() - 1 && k == s[i - 1] {
                assert(t[i - 1] == k);
                assert(0 <= i - 1 < t.len() - 1);
            } else {
                let j0 = choose|j0: int| 0 <= j0 < s.len() - 1 && s[j0] == k;
                assert(s[j0] == k);
                assert(j0 != i);
                let j = if j0 < i { j0 } else { j0 - 1 };
                assert(t[j] == k);
                assert(0 <= j < t.len() - 1);
            }
        }
    }
}


/// `Chain::insert_before(e, Some(a))`의 맵 조작을 그대로 옮긴 것.
pub open spec fn ib_next(next: Map<u64, u64>, prev: Map<u64, u64>, e: u64, a: u64) -> Map<u64, u64> {
    let n1 = if prev.contains_key(a) { next.insert(prev[a], e) } else { next };
    n1.insert(e, a)
}

pub open spec fn ib_prev(prev: Map<u64, u64>, e: u64, a: u64) -> Map<u64, u64> {
    let p1 = if prev.contains_key(a) { prev.insert(e, prev[a]) } else { prev };
    p1.insert(a, e)
}

pub open spec fn ib_head(prev: Map<u64, u64>, head: Option<u64>, e: u64, a: u64) -> Option<u64> {
    if prev.contains_key(a) { head } else { Some(e) }
}

/// **Chain ↔ adjacent 대응 (변이 방향, insert_before)**: 표기 `s`인 chain에
/// (아직 없는) `e`를 `a` 바로 앞에 끼우면 표기가 `s.insert(j, e)`가 된다
/// (`j`는 `a`의 위치).
pub proof fn chain_insert_before_denotes_insert(
    next: Map<u64, u64>,
    prev: Map<u64, u64>,
    head: Option<u64>,
    tail: Option<u64>,
    s: Seq<u64>,
    e: u64,
    a: u64,
    j: int,
)
    requires
        chain_denotes(next, prev, head, tail, s),
        !s.contains(e),
        0 <= j < s.len(),
        s[j] == a,
    ensures
        chain_denotes(
            ib_next(next, prev, e, a),
            ib_prev(prev, e, a),
            ib_head(prev, head, e, a),
            tail,
            s.insert(j, e),
        ),
{
    let t = s.insert(j, e);
    s.insert_ensures(j, e);
    let n2 = ib_next(next, prev, e, a);
    let p2 = ib_prev(prev, e, a);
    let h2 = ib_head(prev, head, e, a);

    assert(prev.contains_key(a) <==> j > 0) by {
        if j > 0 { assert(prev.contains_key(s[j])); }
        if j == 0 { assert(!prev.contains_key(s[0])); }
    }

    assert(t.no_duplicates()) by {
        assert forall|x: int, y: int|
            0 <= x < t.len() && 0 <= y < t.len() && x != y implies t[x] != t[y] by {
            if x == j { assert(t[x] == e); if y < j { assert(t[y] == s[y]); } else { assert(t[y] == s[y - 1]); } }
            else if y == j { assert(t[y] == e); if x < j { assert(t[x] == s[x]); } else { assert(t[x] == s[x - 1]); } }
            else {
                let xx = if x < j { x } else { x - 1 };
                let yy = if y < j { y } else { y - 1 };
                assert(t[x] == s[xx]); assert(t[y] == s[yy]); assert(xx != yy);
            }
        }
    }

    assert(h2 == Some(t[0])) by {
        if j > 0 { assert(t[0] == s[0]); } else { assert(t[0] == e); }
    }
    assert(tail == Some(t[t.len() - 1])) by {
        assert(t[t.len() - 1] == s[s.len() - 1]);
    }

    assert forall|x: int| #![trigger t[x]]
        0 <= x < t.len() - 1 implies n2.contains_key(t[x]) && n2[t[x]] == t[x + 1] by {
        if x == j {
            assert(t[x] == e && t[x + 1] == a);
        } else if x == j - 1 {
            assert(t[x] == s[j - 1] && t[x + 1] == e);
        } else if x < j - 1 {
            assert(t[x] == s[x] && t[x + 1] == s[x + 1]);
            assert(next.contains_key(s[x]) && next[s[x]] == s[x + 1]);
            assert(s[x] != e && s[x] != s[j - 1]);
        } else {
            assert(t[x] == s[x - 1] && t[x + 1] == s[x]);
            assert(next.contains_key(s[x - 1]) && next[s[x - 1]] == s[x]);
            assert(s[x - 1] != e);
            if j > 0 { assert(s[x - 1] != s[j - 1]); }
        }
    }
    assert(!(t.len() > 0 && n2.contains_key(t[t.len() - 1]))) by {
        assert(t[t.len() - 1] == s[s.len() - 1]);
        assert(!next.contains_key(s[s.len() - 1]));
        assert(s[s.len() - 1] != e);
        if j > 0 { assert(s[s.len() - 1] != s[j - 1]); }
    }

    assert forall|x: int| #![trigger t[x]]
        0 < x < t.len() implies p2.contains_key(t[x]) && p2[t[x]] == t[x - 1] by {
        if x == j {
            assert(t[x] == e && t[x - 1] == s[j - 1]);
        } else if x == j + 1 {
            assert(t[x] == a && t[x - 1] == e);
        } else if x < j {
            assert(t[x] == s[x] && t[x - 1] == s[x - 1]);
            assert(prev.contains_key(s[x]) && prev[s[x]] == s[x - 1]);
            assert(s[x] != e && s[x] != a);
        } else {
            assert(t[x] == s[x - 1] && t[x - 1] == s[x - 2]);
            assert(prev.contains_key(s[x - 1]) && prev[s[x - 1]] == s[x - 2]);
            assert(s[x - 1] != e && s[x - 1] != a);
        }
    }
    assert(!(t.len() > 0 && p2.contains_key(t[0]))) by {
        if j > 0 {
            assert(t[0] == s[0]);
            assert(!prev.contains_key(s[0]));
            assert(s[0] != e && s[0] != a);
        } else {
            assert(t[0] == e);
        }
    }

    assert forall|k: u64| #![trigger p2.contains_key(k)]
        p2.contains_key(k) implies exists|x: int| 0 < x < t.len() && t[x] == k by {
        if p2.contains_key(k) {
            if k == a {
                assert(t[j + 1] == k); assert(0 < j + 1 < t.len());
            } else if j > 0 && k == e {
                assert(t[j] == k); assert(0 < j < t.len());
            } else {
                let x0 = choose|x0: int| 0 < x0 < s.len() && s[x0] == k;
                let x = if x0 < j { x0 } else { x0 + 1 };
                assert(t[x] == k); assert(0 < x < t.len());
            }
        }
    }
    assert forall|k: u64| #![trigger n2.contains_key(k)]
        n2.contains_key(k) implies exists|x: int| 0 <= x < t.len() - 1 && t[x] == k by {
        if n2.contains_key(k) {
            if k == e {
                assert(t[j] == k); assert(0 <= j < t.len() - 1);
            } else if j > 0 && k == s[j - 1] {
                assert(t[j - 1] == k); assert(0 <= j - 1 < t.len() - 1);
            } else {
                let x0 = choose|x0: int| 0 <= x0 < s.len() - 1 && s[x0] == k;
                let x = if x0 < j { x0 } else { x0 + 1 };
                assert(t[x] == k); assert(0 <= x < t.len() - 1);
            }
        }
    }
}




/// 꼬리에 덧붙이기의 표기 효과 — 원본 `insert_before(e, None)` 갈래에 대응.
pub proof fn chain_push_back_denotes(
    next: Map<u64, u64>, prev: Map<u64, u64>,
    head: Option<u64>, tail: Option<u64>, s: Seq<u64>, e: u64,
)
    requires chain_denotes(next, prev, head, tail, s), !s.contains(e),
    ensures
        chain_denotes(
            if s.len() > 0 { next.insert(s[s.len() - 1], e) } else { next },
            if s.len() > 0 { prev.insert(e, s[s.len() - 1]) } else { prev },
            if s.len() > 0 { head } else { Some(e) },
            Some(e),
            s.push(e),
        ),
{
    let t = s.push(e);
    let n2 = if s.len() > 0 { next.insert(s[s.len() - 1], e) } else { next };
    let p2 = if s.len() > 0 { prev.insert(e, s[s.len() - 1]) } else { prev };

    assert(t.no_duplicates()) by {
        assert forall|x: int, y: int|
            0 <= x < t.len() && 0 <= y < t.len() && x != y implies t[x] != t[y] by {
            if x < s.len() && y < s.len() { assert(t[x] == s[x] && t[y] == s[y]); }
            else if x == s.len() { assert(t[x] == e); assert(t[y] == s[y]); }
            else { assert(t[y] == e); assert(t[x] == s[x]); }
        }
    }
    assert forall|i: int| #![trigger t[i]]
        0 <= i < t.len() - 1 implies n2.contains_key(t[i]) && n2[t[i]] == t[i + 1] by {
        if i < s.len() - 1 {
            assert(t[i] == s[i] && t[i + 1] == s[i + 1]);
            assert(next.contains_key(s[i]) && next[s[i]] == s[i + 1]);
            assert(s[i] != s[s.len() - 1]);
        } else {
            assert(t[i] == s[s.len() - 1] && t[i + 1] == e);
        }
    }
    assert(!(t.len() > 0 && n2.contains_key(t[t.len() - 1]))) by {
        assert(t[t.len() - 1] == e);
        assert(!next.contains_key(e));
        if s.len() > 0 { assert(e != s[s.len() - 1]); }
    }
    assert forall|i: int| #![trigger t[i]]
        0 < i < t.len() implies p2.contains_key(t[i]) && p2[t[i]] == t[i - 1] by {
        if i < s.len() {
            assert(t[i] == s[i] && t[i - 1] == s[i - 1]);
            assert(prev.contains_key(s[i]) && prev[s[i]] == s[i - 1]);
            assert(s[i] != e);
        } else {
            assert(t[i] == e && t[i - 1] == s[s.len() - 1]);
        }
    }
    assert(!(t.len() > 0 && p2.contains_key(t[0]))) by {
        if s.len() > 0 { assert(t[0] == s[0]); assert(s[0] != e); }
        else { assert(t[0] == e); }
    }
    assert forall|k: u64| #![trigger n2.contains_key(k)]
        n2.contains_key(k) implies exists|i: int| 0 <= i < t.len() - 1 && t[i] == k by {
        if n2.contains_key(k) {
            if s.len() > 0 && k == s[s.len() - 1] {
                assert(t[s.len() - 1] == k); assert(0 <= s.len() - 1 < t.len() - 1);
            } else {
                let i = choose|i: int| 0 <= i < s.len() - 1 && s[i] == k;
                assert(t[i] == k);
            }
        }
    }
    assert forall|k: u64| #![trigger p2.contains_key(k)]
        p2.contains_key(k) implies exists|i: int| 0 < i < t.len() && t[i] == k by {
        if p2.contains_key(k) {
            if k == e { assert(t[s.len() as int] == k); assert(0 < s.len() < t.len()); }
            else {
                let i = choose|i: int| 0 < i < s.len() && s[i] == k;
                assert(t[i] == k);
            }
        }
    }
}

} // verus!
