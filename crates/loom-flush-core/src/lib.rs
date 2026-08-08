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


/// **`no_duplicates`를 가두는 불투명 래퍼**(2026-08-01, 프로파일 근거).
///
/// `Seq::no_duplicates()`는 `forall|i, j| ... ==> s[i] != s[j]` — 인덱스 **쌍**
/// 전체에 대한 2변수 양화자다. 이 사실이 루프 문맥에 떠 있으면 Z3가 O(n²)
/// 패턴을 폭발적으로 인스턴스화한다: `--profile` 결과 이 하나가 전체 인스턴스화의
/// **88%**(2,286,314 / 2,584,749)를 차지해 루프 검증이 자원 벽에 부딪혔다.
///
/// 래퍼를 `opaque`로 두면 루프는 "중복이 없다"는 **명제만** 들고 다니고
/// 양화자는 보지 않는다. 실제로 펼치는 것은 그것이 필요한 보조정리 안뿐이며,
/// 그 안의 검증 조건은 작아서 감당된다.
#[verifier::opaque]
pub open spec fn seq_nodup(s: Seq<u64>) -> bool {
    s.no_duplicates()
}

/// 래퍼를 벗긴다 — 양화자가 필요한 지점에서만 쓴다.
pub proof fn lemma_nodup_unwrap(s: Seq<u64>)
    requires seq_nodup(s),
    ensures s.no_duplicates(),
{ reveal(seq_nodup); }

/// 래퍼를 씌운다.
pub proof fn lemma_nodup_wrap(s: Seq<u64>)
    requires s.no_duplicates(),
    ensures seq_nodup(s),
{ reveal(seq_nodup); }

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


    #[verifier::rlimit(400)]
    /// `well_formed()`가 `closed`라 밖에서 본문을 못 펴므로, 필요한 사실
    /// 하나(표기에 중복이 없다)만 별도로 노출한다 — `plan_writes`가
    /// `unlink` 직후 "e가 정말 사라졌다"를 유도하는 데 쓴다.
    pub proof fn no_dup(&self)
        requires self.well_formed(),
        ensures self.denotes().no_duplicates(),
    {
    }

    /// `no_dup`의 불투명 판본 — 루프가 양화자를 보지 않도록 이쪽을 쓴다.
    pub proof fn no_dup_opaque(&self)
        requires self.well_formed(),
        ensures seq_nodup(self.denotes()),
    {
        self.no_dup();
        lemma_nodup_wrap(self.denotes());
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
    requires seq_nodup(s),
    ensures
        // **불투명 형태로 낸다**: raw `no_duplicates`를 ensures에 두면 이 정리를
        // 부르는 쪽(루프) 문맥에 2변수 양화자가 다시 주입된다 — 프로파일에서
        // 래퍼를 씌우고도 89%가 그대로였던 원인이 이것이었다.
        seq_nodup(s.remove_value(e)),
        !s.remove_value(e).contains(e),
{
    lemma_nodup_unwrap(s);
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
    lemma_nodup_wrap(s.remove_value(e));
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


// ---------------------------------------------------------------------------
// 집합 멤버십 보조정리 — `plan_writes`의 루프 불변식 유지에 쓰인다.
// ---------------------------------------------------------------------------

/// 다른 원소를 지워도 `x`는 남는다.
pub proof fn lemma_remove_value_keeps_other(s: Seq<u64>, e: u64, x: u64)
    requires s.contains(x), x != e,
    ensures s.remove_value(e).contains(x),
{
    s.index_of_first_ensures(e);
    let ix = choose|i: int| 0 <= i < s.len() && s[i] == x;
    if let Some(i) = s.index_of_first(e) {
        s.remove_ensures(i);
        assert(ix != i);
        let j = if ix < i { ix } else { ix - 1 };
        assert(s.remove(i)[j] == x);
        assert(s.remove(i).contains(x));
    }
}

/// 삽입은 기존 원소를 잃지 않는다.
pub proof fn lemma_insert_keeps(s: Seq<u64>, j: int, e: u64, x: u64)
    requires s.contains(x), 0 <= j <= s.len(),
    ensures s.insert(j, e).contains(x),
{
    s.insert_ensures(j, e);
    let ix = choose|i: int| 0 <= i < s.len() && s[i] == x;
    let k = if ix < j { ix } else { ix + 1 };
    assert(s.insert(j, e)[k] == x);
}

/// 삽입된 원소는 들어 있다.
pub proof fn lemma_insert_has_new(s: Seq<u64>, j: int, e: u64)
    requires 0 <= j <= s.len(),
    ensures s.insert(j, e).contains(e),
{
    s.insert_ensures(j, e);
    assert(s.insert(j, e)[j] == e);
}

/// 꼬리 덧붙이기도 기존 원소를 잃지 않고, 새 원소를 갖는다.
pub proof fn lemma_push_keeps(s: Seq<u64>, e: u64, x: u64)
    requires s.contains(x),
    ensures s.push(e).contains(x), s.push(e).contains(e),
{
    let ix = choose|i: int| 0 <= i < s.len() && s[i] == x;
    assert(s.push(e)[ix] == x);
    assert(s.push(e)[s.len() as int] == e);
}



/// 역방향: 삽입 결과에 있는 것은 원래 있었거나 새로 넣은 그것이다.
pub proof fn lemma_insert_only(s: Seq<u64>, j: int, e: u64, x: u64)
    requires s.insert(j, e).contains(x), 0 <= j <= s.len(),
    ensures s.contains(x) || x == e,
{
    s.insert_ensures(j, e);
    let t = s.insert(j, e);
    let k = choose|k: int| 0 <= k < t.len() && t[k] == x;
    if k != j {
        let kk = if k < j { k } else { k - 1 };
        assert(t[k] == s[kk]);
    }
}

/// 역방향: 꼬리 덧붙이기도 마찬가지.
pub proof fn lemma_push_only(s: Seq<u64>, e: u64, x: u64)
    requires s.push(e).contains(x),
    ensures s.contains(x) || x == e,
{
    let t = s.push(e);
    let k = choose|k: int| 0 <= k < t.len() && t[k] == x;
    if k < s.len() { assert(t[k] == s[k]); }
}

/// 역방향: 지우기는 새 원소를 만들지 않는다.
pub proof fn lemma_remove_value_only(s: Seq<u64>, e: u64, x: u64)
    requires s.remove_value(e).contains(x),
    ensures s.contains(x),
{
    s.index_of_first_ensures(e);
    if let Some(i) = s.index_of_first(e) {
        s.remove_ensures(i);
        let t = s.remove(i);
        let k = choose|k: int| 0 <= k < t.len() && t[k] == x;
        let kk = if k < i { k } else { k + 1 };
        assert(t[k] == s[kk]);
    }
}

/// **L1**: 뒤쪽 조각에 `e`가 없으면, 이어붙인 시퀀스에서 첫 occurrence를 지우는
/// 것은 앞쪽 조각에서만 지우는 것과 같다.
pub proof fn lemma_remove_value_in_prefix(p: Seq<u64>, s: Seq<u64>, e: u64)
    requires !s.contains(e),
    ensures (p + s).remove_value(e) =~= p.remove_value(e) + s,
{
    (p + s).index_of_first_ensures(e);
    p.index_of_first_ensures(e);
    if p.contains(e) {
        let i = p.index_of_first(e).unwrap();
        assert((p + s).index_of_first(e) == Some(i)) by {
            assert((p + s)[i] == p[i]);
            assert(forall|j: int| 0 <= j < i ==> (p + s)[j] == p[j]);
        }
        assert((p + s).remove(i) =~= p.remove(i) + s);
    } else {
        assert(!(p + s).contains(e)) by {
            assert forall|j: int| 0 <= j < (p + s).len() implies (p + s)[j] != e by {
                if j < p.len() { assert((p + s)[j] == p[j]); }
                else { assert((p + s)[j] == s[j - p.len()]); }
            }
        }
    }
}


/// **L2**: 앞쪽 조각에 `a`가 없고 뒤쪽 조각이 `a`로 시작하면, 이어붙인
/// 시퀀스에서 `a`의 첫 위치는 정확히 앞쪽 조각의 길이다.
pub proof fn lemma_index_of_first_at_boundary(p: Seq<u64>, s: Seq<u64>, a: u64)
    requires !p.contains(a), s.len() > 0, s[0] == a,
    ensures (p + s).index_of_first(a) == Some(p.len() as int),
{
    (p + s).index_of_first_ensures(a);
    assert((p + s)[p.len() as int] == a);
    assert forall|j: int| 0 <= j < p.len() implies (p + s)[j] != a by {
        assert((p + s)[j] == p[j]);
    }
}


/// **L3**: 경계 위치에 삽입하면 앞쪽 조각 끝에 붙는 것과 같다.
pub proof fn lemma_insert_at_boundary(p: Seq<u64>, s: Seq<u64>, e: u64)
    ensures (p + s).insert(p.len() as int, e) =~= p.push(e) + s,
{
    assert((p + s).subrange(0, p.len() as int) =~= p);
    assert((p + s).subrange(p.len() as int, (p + s).len() as int) =~= s);
}



/// **L4**: 원소를 지워도 "원래 없던 것은 여전히 없다".
pub proof fn lemma_remove_value_preserves_absence(s: Seq<u64>, e: u64, x: u64)
    requires !s.contains(x),
    ensures !s.remove_value(e).contains(x),
{
    s.index_of_first_ensures(e);
    if let Some(i) = s.index_of_first(e) {
        s.remove_ensures(i);
        assert forall|j: int| 0 <= j < s.remove(i).len() implies s.remove(i)[j] != x by {
            if j < i { assert(s.remove(i)[j] == s[j]); }
            else { assert(s.remove(i)[j] == s[j + 1]); }
        }
    }
}


/// **L6**: 없던 원소를 끼워 넣으면 중복 없음이 유지된다.
pub proof fn lemma_insert_no_dup(s: Seq<u64>, i: int, e: u64)
    requires s.no_duplicates(), !s.contains(e), 0 <= i <= s.len(),
    ensures s.insert(i, e).no_duplicates(),
{
    s.insert_ensures(i, e);
    let t = s.insert(i, e);
    assert forall|j: int, l: int|
        0 <= j < t.len() && 0 <= l < t.len() && j != l implies t[j] != t[l] by {
        if j == i {
            assert(t[j] == e);
            if l < i { assert(t[l] == s[l]); } else { assert(t[l] == s[l - 1]); }
        } else if l == i {
            assert(t[l] == e);
            if j < i { assert(t[j] == s[j]); } else { assert(t[j] == s[j - 1]); }
        } else {
            let jj = if j < i { j } else { j - 1 };
            let ll = if l < i { l } else { l - 1 };
            assert(t[j] == s[jj]);
            assert(t[l] == s[ll]);
            assert(jj != ll);
        }
    }
}



/// **단계 보조정리**: DOM의 뒤쪽 `k`개가 이미 목표 접미사와 같을 때,
/// "`e`를 그 접미사의 첫 원소 앞에 놓는" write 하나를 적용하면 뒤쪽 `k+1`개가
/// `[e] ++ 접미사`가 된다. 이게 귀납의 한 걸음이다.
pub proof fn lemma_place_extends_suffix(d: Seq<u64>, k: int, e: u64)
    requires
        seq_nodup(d),
        0 < k <= d.len(),
        !d.subrange(d.len() - k, d.len() as int).contains(e),
    ensures
        ({
            let suf = d.subrange(d.len() - k, d.len() as int);
            let r = place_before(d, e, Some(suf[0]));
            &&& seq_nodup(r)
            &&& r.len() >= k + 1
            &&& r.subrange(r.len() - (k + 1), r.len() as int) =~= seq![e] + suf
        }),
{
    lemma_nodup_unwrap(d);
    let p = d.subrange(0, d.len() - k);
    let suf = d.subrange(d.len() - k, d.len() as int);
    let a = suf[0];
    assert(d =~= p + suf);

    // a는 접미사의 첫 원소이므로, 중복이 없으니 앞쪽 조각에는 없다.
    assert(!p.contains(a)) by {
        assert forall|j: int| 0 <= j < p.len() implies p[j] != a by {
            assert(p[j] == d[j]);
            assert(a == d[d.len() - k]);
            assert(j != d.len() - k);
        }
    }

    lemma_remove_value_in_prefix(p, suf, e);
    lemma_remove_value_no_dup(d, e);
    let p2 = p.remove_value(e);
    assert(d.remove_value(e) =~= p2 + suf);
    lemma_remove_value_preserves_absence(p, e, a);
    lemma_index_of_first_at_boundary(p2, suf, a);
    lemma_insert_at_boundary(p2, suf, e);

    let r = place_before(d, e, Some(a));
    assert(r =~= p2.push(e) + suf);
    // `lemma_remove_value_no_dup`이 이제 불투명 형태로 내주므로 여기서 벗긴다
    // (이 proof fn 안이라 루프 문맥으로는 새지 않는다).
    lemma_nodup_unwrap(d.remove_value(e));
    lemma_insert_no_dup(d.remove_value(e), p2.len() as int, e);
    assert(r.subrange(r.len() - (k + 1), r.len() as int) =~= seq![e] + suf) by {
        assert(r.len() == p2.len() + 1 + suf.len());
        assert forall|j: int| 0 <= j < k + 1
            implies r.subrange(r.len() - (k + 1), r.len() as int)[j] == (seq![e] + suf)[j] by {
            if j == 0 {
                assert(r[p2.len() as int] == e);
            } else {
                assert(r[p2.len() + j] == suf[j - 1]);
            }
        }
    }
    lemma_nodup_wrap(place_before(d, e, Some(d.subrange(d.len() - k, d.len() as int)[0])));
}




/// `LoomApp::tick` 적용 루프 한 걸음(표기 수준).
pub open spec fn place_before(dom: Seq<u64>, e: u64, before: Option<u64>) -> Seq<u64> {
    let d = dom.remove_value(e);
    match before {
        None => d.push(e),
        Some(a) => match d.index_of_first(a) {
            Some(i) => d.insert(i, e),
            None => d.push(e),
        },
    }
}

/// 없는 원소를 지우려 하면 아무 일도 없다.
pub proof fn lemma_remove_value_absent(s: Seq<u64>, e: u64)
    requires !s.contains(e),
    ensures s.remove_value(e) =~= s,
{ s.index_of_first_ensures(e); }

/// **인접해서 스킵한 경우의 접미사 확장**.
pub proof fn lemma_adjacent_skip_extends_suffix(
    d: Seq<u64>, t: Seq<u64>, i: int, n: int, e: u64, a: u64,
)
    requires
        seq_nodup(d), seq_nodup(t), n == t.len(),
        0 <= i, i + 1 < n, e == t[i], a == t[i + 1],
        d.len() >= n - i - 1,
        d.subrange(d.len() - (n - i - 1), d.len() as int) =~= t.subrange(i + 1, n),
        adjacent(d, e, a),
    ensures
        d.len() >= n - i,
        d.subrange(d.len() - (n - i), d.len() as int) =~= t.subrange(i, n),
{
    lemma_nodup_unwrap(d); lemma_nodup_unwrap(t);
    let k = n - i - 1;
    let suf = d.subrange(d.len() - k, d.len() as int);
    assert(suf[0] == t.subrange(i + 1, n)[0]);
    assert(t.subrange(i + 1, n)[0] == t[i + 1]);
    assert(d[d.len() - k] == a);
    let j = choose|j: int| 0 <= j < d.len() - 1 && #[trigger] d[j] == e && d[j + 1] == a;
    assert(j + 1 == d.len() - k) by { assert(d[j + 1] == d[d.len() - k]); }
    assert(d.len() >= n - i);
    assert(d.subrange(d.len() - (k + 1), d.len() as int) =~= t.subrange(i, n)) by {
        assert forall|m: int| 0 <= m < k + 1 implies
            d[d.len() - (k + 1) + m] == t.subrange(i, n)[m] by {
            if m == 0 { assert(d.len() - (k + 1) == j); }
            else {
                assert(suf[m - 1] == d[d.len() - k + m - 1]);
                assert(t.subrange(i + 1, n)[m - 1] == t[i + m]);
            }
        }
    }
}

/// **꼬리라서 스킵한 경우**(i == n-1).
pub proof fn lemma_tail_skip_extends_suffix(d: Seq<u64>, t: Seq<u64>, i: int, n: int, e: u64)
    requires n == t.len(), 0 <= i, i + 1 == n, e == t[i], d.len() > 0, d[d.len() - 1] == e,
    ensures d.len() >= n - i, d.subrange(d.len() - (n - i), d.len() as int) =~= t.subrange(i, n),
{
    assert(t.subrange(i, n).len() == 1);
    assert(t.subrange(i, n)[0] == t[i]);
}

/// 비스킵 분기의 마지막 연결: `[e] ++ 이전접미사`를 `target[i..n]`으로 읽는다.
pub proof fn lemma_move_extends_suffix(
    d_new: Seq<u64>, suf_old: Seq<u64>, t: Seq<u64>, i: int, n: int, e: u64,
)
    requires
        n == t.len(), 0 <= i, i + 1 <= n, e == t[i],
        suf_old =~= t.subrange(i + 1, n),
        d_new.len() >= n - i,
        d_new.subrange(d_new.len() - (n - i), d_new.len() as int) =~= seq![e] + suf_old,
    ensures d_new.subrange(d_new.len() - (n - i), d_new.len() as int) =~= t.subrange(i, n),
{
    assert(seq![e] + suf_old =~= t.subrange(i, n)) by {
        assert forall|m: int| 0 <= m < n - i implies
            (seq![e] + suf_old)[m] == t.subrange(i, n)[m] by {
            if m == 0 { } else { assert(suf_old[m - 1] == t[i + m]); }
        }
    }
}


/// 방출된 op 열을 앞에서부터 `k`개 적용한 결과.
pub open spec fn apply_ops(dom: Seq<u64>, ops: Seq<PlanOp>, k: int) -> Seq<u64>
    decreases k
{
    if k <= 0 { dom } else { place_before(apply_ops(dom, ops, k - 1), ops[k - 1].id, ops[k - 1].before) }
}

/// `apply_ops`는 앞의 `k`개만 읽으므로 뒤에 덧붙여도 앞부분 결과는 그대로다.
pub proof fn lemma_apply_ops_prefix_stable(
    dom: Seq<u64>, ops1: Seq<PlanOp>, ops2: Seq<PlanOp>, k: int,
)
    requires
        0 <= k <= ops1.len(), k <= ops2.len(),
        forall|j: int| #![trigger ops1[j]] 0 <= j < k ==> ops1[j] == ops2[j],
    ensures apply_ops(dom, ops1, k) =~= apply_ops(dom, ops2, k),
    decreases k,
{
    if k > 0 {
        lemma_apply_ops_prefix_stable(dom, ops1, ops2, k - 1);
        assert(ops1[k - 1] == ops2[k - 1]);
    }
}

/// "chain 표기 = 방출 op 적용 결과" 불변식을 한 걸음 잇는다.
pub proof fn lemma_step_keeps_link(
    dom: Seq<u64>, prev_ops: Seq<PlanOp>, ops: Seq<PlanOp>,
    prev_denotes: Seq<u64>, now_denotes: Seq<u64>, op: PlanOp,
)
    requires
        prev_denotes =~= apply_ops(dom, prev_ops, prev_ops.len() as int),
        ops =~= prev_ops.push(op),
        now_denotes =~= place_before(prev_denotes, op.id, op.before),
    ensures now_denotes =~= apply_ops(dom, ops, ops.len() as int),
{
    lemma_apply_ops_prefix_stable(dom, prev_ops, ops, prev_ops.len() as int);
    assert(ops[prev_ops.len() as int] == op);
}

/// 삽입은 "이전 집합 ∪ {e}"를 만든다 — 루프 안의 forall 추론을 밖으로 뺀 것.
pub proof fn lemma_insert_same_plus(prev: Seq<u64>, j: int, e: u64)
    requires 0 <= j <= prev.len(),
    ensures same_plus(prev.insert(j, e), prev, e),
{
    assert forall|x: u64| #![trigger prev.insert(j, e).contains(x)]
        prev.insert(j, e).contains(x) <==> (prev.contains(x) || x == e) by {
        if prev.insert(j, e).contains(x) { lemma_insert_only(prev, j, e, x); }
        if prev.contains(x) { lemma_insert_keeps(prev, j, e, x); }
        if x == e { lemma_insert_has_new(prev, j, e); }
    }
}

/// 꼬리 덧붙이기도 마찬가지.
pub proof fn lemma_push_same_plus(prev: Seq<u64>, e: u64)
    ensures same_plus(prev.push(e), prev, e),
{
    assert forall|x: u64| #![trigger prev.push(e).contains(x)]
        prev.push(e).contains(x) <==> (prev.contains(x) || x == e) by {
        if prev.push(e).contains(x) { lemma_push_only(prev, e, x); }
        if prev.contains(x) { lemma_push_keeps(prev, e, x); }
        if x == e { assert(prev.push(e)[prev.len() as int] == e); }
    }
}

/// 지운 뒤 다시 끼우는 경우(Retained 이동)도 집합은 그대로 + e.
pub proof fn lemma_remove_then_place_same_plus(
    prev: Seq<u64>, mid: Seq<u64>, now: Seq<u64>, e: u64,
)
    requires mid =~= prev.remove_value(e), same_plus(now, mid, e),
    ensures same_plus(now, prev, e),
{
    assert forall|x: u64| #![trigger now.contains(x)]
        now.contains(x) <==> (prev.contains(x) || x == e) by {
        if now.contains(x) && x != e { lemma_remove_value_only(prev, e, x); }
        if prev.contains(x) && x != e { lemma_remove_value_keeps_other(prev, e, x); }
    }
}

/// 멤버십 불변식 (b)/(c)를 한 걸음 전진시킨다.
pub proof fn lemma_membership_step(
    prev: Seq<u64>, now: Seq<u64>, cur: Seq<u64>, t: Seq<u64>, idx: int, n: int, e: u64,
)
    requires
        n == t.len(), 0 <= idx, idx + 1 <= n, e == t[idx],
        same_plus(now, prev, e),
        forall|x: u64| #![trigger cur.contains(x)] cur.contains(x) ==> prev.contains(x),
        forall|x: u64| #![trigger prev.contains(x)]
            prev.contains(x) ==> cur.contains(x) || t.subrange(idx + 1, n).contains(x),
    ensures
        forall|x: u64| #![trigger cur.contains(x)] cur.contains(x) ==> now.contains(x),
        forall|x: u64| #![trigger now.contains(x)]
            now.contains(x) ==> cur.contains(x) || t.subrange(idx, n).contains(x),
{
    let sub_new = t.subrange(idx, n);
    let sub_old = t.subrange(idx + 1, n);
    assert(sub_new[0] == e);
    assert forall|x: u64| sub_old.contains(x) implies sub_new.contains(x) by {
        let m = choose|m: int| 0 <= m < sub_old.len() && sub_old[m] == x;
        assert(sub_new[m + 1] == sub_old[m]);
    }
}


/// **`place_in_order`의 순서 결정 — 수렴까지 증명된 판본**.
///
/// `plan_writes`(LIS 변형용, `keep` 있음)와 갈라 둔 이유는 **사실에 맞추기
/// 위해서**다. LIS가 `keep`으로 "이건 이미 제자리"라고 표시하면 그 원소는
/// 인접하지 않아도 스킵되는데, 그 판단의 정당성은 비검증 `lis_indices`에
/// 있다 — 그래서 그쪽은 수렴을 주장할 수 없다. 두 경로를 한 함수에 담으면
/// 불변식이 전부 조건부가 되어 SMT가 감당하지 못한다(2026-07-28에 실제로
/// 그래서 실패했다 — `formal-verification/verus/README.md` 참고).
#[verifier::rlimit(2000)]
pub fn plan_writes_converging(
    current: &Vec<u64>,
    target_ids: &Vec<u64>,
    target_fresh: &Vec<bool>,
) -> (ops: Vec<PlanOp>)
    requires
        current@.no_duplicates(),
        target_ids@.no_duplicates(),
        target_fresh@.len() == target_ids@.len(),
        forall|i: int| #![trigger target_fresh@[i]]
            0 <= i < target_ids@.len() ==>
                (target_fresh@[i] ==> !current@.contains(target_ids@[i]))
                && (!target_fresh@[i] ==> current@.contains(target_ids@[i])),
    ensures
        forall|k: int| #![trigger ops@[k]]
            0 <= k < ops@.len() ==>
                ops@[k].idx < target_ids@.len()
                && ops@[k].spawn == target_fresh@[ops@[k].idx as int],
        // **수렴(RQ5 #4 정리 (2))**: 방출된 op를 순서대로 적용하면 DOM의 뒤쪽
        // |target|개가 목표 순서와 정확히 일치한다.
        ({
            let d = apply_ops(current@, ops@, ops@.len() as int);
            &&& d.len() >= target_ids@.len()
            &&& d.subrange(d.len() - target_ids@.len(), d.len() as int) =~= target_ids@
        }),
{
    let n = target_ids.len();
    let mut chain = Chain::from_slice(current);
    // 불투명 형태로 바꿔 루프에 넘긴다 — 루프가 `no_duplicates`의 2변수
    // 양화자를 보지 않게 하기 위함(프로파일상 그 하나가 인스턴스화의 88%).
    proof { lemma_nodup_wrap(current@); lemma_nodup_wrap(target_ids@); }
    let mut ops: Vec<PlanOp> = Vec::new();
    let mut next_anchor: Option<u64> = None;
    let mut i: usize = n;

    while i > 0
        invariant
            i <= n,
            n == target_ids@.len(),
            target_fresh@.len() == target_ids@.len(),
            // Verus의 `while`은 불변식만으로 모듈러 검증되므로, 함수
            // `requires`의 사실이라도 여기 다시 적어야 한다.
            seq_nodup(current@),
            seq_nodup(target_ids@),
            forall|m: int| #![trigger target_fresh@[m]]
                0 <= m < n ==>
                    (target_fresh@[m] ==> !current@.contains(target_ids@[m]))
                    && (!target_fresh@[m] ==> current@.contains(target_ids@[m])),
            chain.well_formed(),
            forall|x: u64| #![trigger current@.contains(x)]
                current@.contains(x) ==> chain.denotes().contains(x),
            forall|x: u64| #![trigger chain.denotes().contains(x)]
                chain.denotes().contains(x) ==>
                    current@.contains(x) || target_ids@.subrange(i as int, n as int).contains(x),
            next_anchor == if i == n { None::<u64> } else { Some(target_ids@[i as int]) },
            forall|k: int| #![trigger ops@[k]]
                0 <= k < ops@.len() ==>
                    ops@[k].idx < n && ops@[k].spawn == target_fresh@[ops@[k].idx as int],
            chain.denotes() =~= apply_ops(current@, ops@, ops@.len() as int),
            chain.denotes().len() >= n - i,
            chain.denotes().subrange(chain.denotes().len() - (n - i) as int,
                                     chain.denotes().len() as int)
                =~= target_ids@.subrange(i as int, n as int),
        decreases i,
    {
        // `i`는 루프 **끝에서만** 줄인다 — 먼저 줄이면 불변식의 `i`(옛 값)와
        // 본문의 `i`(새 값)를 프루버가 잇지 못한다.
        let idx = i - 1;
        let e = target_ids[idx];
        let fresh = target_fresh[idx];
        let ghost prev_denotes = chain.denotes();
        let ghost prev_ops = ops@;
        let ghost k_old = (n - i) as int;
        let ghost suf_old =
            prev_denotes.subrange(prev_denotes.len() - k_old, prev_denotes.len() as int);

        proof {
            chain.no_dup_opaque();
            assert(suf_old =~= target_ids@.subrange(i as int, n as int));
            assert(!suf_old.contains(e)) by {
                // 여기서만 양화자를 펼친다 — `by` 블록 안이라 루프 본체의
                // 검증 조건으로 새지 않는다.
                lemma_nodup_unwrap(target_ids@);
                assert forall|j: int| #![trigger suf_old[j]]
                    0 <= j < suf_old.len() implies suf_old[j] != e by {
                    assert(suf_old[j] == target_ids@.subrange(i as int, n as int)[j]);
                    assert(target_ids@.subrange(i as int, n as int)[j] == target_ids@[i as int + j]);
                    assert(e == target_ids@[idx as int]);
                    assert(i as int + j != idx as int);
                }
            }
            if i < n {
                assert(suf_old[0] == target_ids@[i as int]);
                assert(prev_denotes[prev_denotes.len() - k_old] == suf_old[0]);
                assert(prev_denotes.contains(target_ids@[i as int]));
            }
        }

        let adj = match next_anchor {
            Some(a) => chain.next_is(e, a),
            None => chain.is_tail(e),
        };

        if fresh || !adj {
            if !fresh {
                proof {
                    assert(!target_fresh@[idx as int]);
                    assert(current@.contains(target_ids@[idx as int]));
                    assert(chain.denotes().contains(e));
                    lemma_remove_value_no_dup(prev_denotes, e);
                }
                chain.unlink(e);
                proof {
                    let p_old = prev_denotes.subrange(0, prev_denotes.len() - k_old);
                    assert(prev_denotes =~= p_old + suf_old);
                    lemma_remove_value_in_prefix(p_old, suf_old, e);
                    let d2 = chain.denotes();
                    assert(d2 =~= p_old.remove_value(e) + suf_old);
                    assert(d2.subrange(d2.len() - k_old, d2.len() as int) =~= suf_old) by {
                        assert forall|j: int| #![trigger suf_old[j]] 0 <= j < k_old implies
                            d2[d2.len() - k_old + j] == suf_old[j] by {
                            assert(d2[p_old.remove_value(e).len() + j] == suf_old[j]);
                        }
                    }
                    if let Some(a) = next_anchor {
                        assert(a != e);
                        lemma_remove_value_keeps_other(prev_denotes, e, a);
                    }
                }
            } else {
                proof { lemma_remove_value_absent(prev_denotes, e); }
            }
            let ghost d_mid = chain.denotes();
            proof {
                chain.no_dup_opaque();
                if i < n { lemma_place_extends_suffix(d_mid, k_old, e); }
            }
            match next_anchor {
                Some(a) => {
                    proof { d_mid.index_of_first_ensures(a); }
                    let ghost j0 = d_mid.index_of_first(a).unwrap();
                    chain.insert_before(e, a);
                    proof {
                        lemma_move_extends_suffix(chain.denotes(), suf_old, target_ids@,
                                                  idx as int, n as int, e);
                        lemma_insert_same_plus(d_mid, j0, e);
                        if !fresh {
                            lemma_remove_then_place_same_plus(prev_denotes, d_mid, chain.denotes(), e);
                        }
                    }
                }
                None => {
                    chain.push_back(e);
                    proof {
                        assert(i == n);
                        let d = chain.denotes();
                        assert(d[d.len() - 1] == e);
                        assert(target_ids@.subrange(idx as int, n as int).len() == 1);
                        assert(target_ids@.subrange(idx as int, n as int)[0] == target_ids@[idx as int]);
                        lemma_push_same_plus(d_mid, e);
                        if !fresh {
                            lemma_remove_then_place_same_plus(prev_denotes, d_mid, d, e);
                        }
                    }
                }
            }
            let ghost now_d = chain.denotes();
            ops.push(PlanOp { idx, id: e, spawn: fresh, before: next_anchor });
            proof {
                lemma_step_keeps_link(current@, prev_ops, ops@, prev_denotes, now_d,
                    PlanOp { idx, id: e, spawn: fresh, before: next_anchor });
            }
        } else {
            proof {
                assert(current@.contains(e));
                assert(same_plus(chain.denotes(), prev_denotes, e));
                match next_anchor {
                    Some(a) => lemma_adjacent_skip_extends_suffix(
                        chain.denotes(), target_ids@, idx as int, n as int, e, a),
                    None => lemma_tail_skip_extends_suffix(
                        chain.denotes(), target_ids@, idx as int, n as int, e),
                }
            }
        }

        proof {
            lemma_membership_step(prev_denotes, chain.denotes(), current@, target_ids@,
                                  idx as int, n as int, e);
        }
        next_anchor = Some(e);
        i = idx;
    }
    proof { assert(target_ids@.subrange(0, n as int) =~= target_ids@); }
    ops
}

// ---------------------------------------------------------------------------
// 방출 루프 — `place_in_order`의 순서 결정 전체
// ---------------------------------------------------------------------------

/// "표기의 원소 집합이 이전 집합 ∪ {e}가 됐다" — 세 분기가 공통으로 만족하는
/// 모양. 이것 하나로 루프 불변식 (b)/(c)/(d)를 균일하게 유도한다.
pub open spec fn same_plus(new: Seq<u64>, old: Seq<u64>, e: u64) -> bool {
    forall|x: u64| #![trigger new.contains(x)]
        new.contains(x) <==> (old.contains(x) || x == e)
}

/// 방출할 write 하나. 호출부(`list.rs`)는 `idx`로 `target` 슬롯에서
/// Entity/텍스트를 꺼내 실제 `DomWrite`를 만든다 — 여기서는 **순서 결정만**
/// 한다(무엇을 언제 어느 anchor 앞에 놓을지).
pub struct PlanOp {
    /// `target`에서의 인덱스.
    pub idx: usize,
    /// 이 op가 움직이는 entity의 id. 사후조건(`apply_ops`)이 `target`을
    /// 참조하지 않고 자립하도록 함께 싣는다.
    pub id: u64,
    /// true면 SpawnRow, false면 Reposition.
    pub spawn: bool,
    /// anchor — `None`이면 꼬리.
    pub before: Option<u64>,
}

/// **`place_in_order`의 순서 결정 전체**를 검증된 코드로 옮긴 것.
/// 반환된 `PlanOp` 열은 **방출 순서 그대로**다(= target의 역순, 스킵 제외).
///
/// 이 함수가 존재하는 이유: 이전에는 `Chain`만 검증돼 있고 그 **사전조건**을
/// 호출부(`list.rs`, 비검증 코드)가 지키는지는 기계적으로 확인되지 않았다.
/// 루프를 여기로 옮기면 `unlink`/`insert_before`/`push_back`의 사전조건이
/// **전부 이 안에서 증명으로 해소**된다.
#[verifier::rlimit(4000)]
pub fn plan_writes(
    current: &Vec<u64>,
    target_ids: &Vec<u64>,
    target_fresh: &Vec<bool>,
    keep: &Vec<bool>,
) -> (ops: Vec<PlanOp>)
    requires
        current@.no_duplicates(),
        target_ids@.no_duplicates(),
        target_fresh@.len() == target_ids@.len(),
        keep@.len() == target_ids@.len(),
        // 호출부가 지키는 전제: Fresh는 아직 DOM에 없고, Retained는 이미 있다.
        forall|i: int| #![trigger target_fresh@[i]]
            0 <= i < target_ids@.len() ==>
                (target_fresh@[i] ==> !current@.contains(target_ids@[i]))
                && (!target_fresh@[i] ==> current@.contains(target_ids@[i])),
    ensures
        // 방출된 op의 인덱스는 전부 유효하고, spawn 여부는 target과 일치한다.
        forall|k: int| #![trigger ops@[k]]
            0 <= k < ops@.len() ==>
                ops@[k].idx < target_ids@.len()
                && ops@[k].spawn == target_fresh@[ops@[k].idx as int],
{
    let n = target_ids.len();
    let mut chain = Chain::from_slice(current);
    // **불투명 형태로 바꿔 루프에 넘긴다** (2026-08-08, 프로파일 근거).
    // `plan_writes_converging`은 2026-08-01에 이 처방을 받았는데 이쪽(LIS
    // 변형)은 raw `no_duplicates`를 루프 불변식에 그대로 들고 있었다 —
    // `--profile-all`이 `vstd/seq_lib.rs:520`(= `no_duplicates`의 2변수
    // 양화자)을 전체 인스턴스화의 **82%**로 지목했다.
    //
    // 펼치는 것은 그것이 실제로 필요한 `assert ... by { }` 블록 **안쪽**뿐이다.
    // 보조정리(`proof fn`)로 빼는 안도 해 봤는데, 그 `ensures`가 모듈 문맥에
    // 새 트리거를 더해 **건드리지도 않은 `plan_writes_converging`이 rlimit
    // 크래시**를 냈다 — 이 모듈이 경계에 붙어 있다는 증거이고, 그래서 공리를
    // 늘리지 않는 `by` 블록 쪽을 택했다.
    proof { lemma_nodup_wrap(current@); lemma_nodup_wrap(target_ids@); }
    let mut ops: Vec<PlanOp> = Vec::new();
    let mut next_anchor: Option<u64> = None;
    let mut i: usize = n;

    while i > 0
        invariant
            i <= n,
            n == target_ids@.len(),
            target_fresh@.len() == target_ids@.len(),
            keep@.len() == target_ids@.len(),
            seq_nodup(current@),
            seq_nodup(target_ids@),
            forall|j: int| #![trigger target_fresh@[j]]
                0 <= j < n ==>
                    (target_fresh@[j] ==> !current@.contains(target_ids@[j]))
                    && (!target_fresh@[j] ==> current@.contains(target_ids@[j])),
            chain.well_formed(),
            // (b) 처음부터 있던 것은 사라지지 않는다.
            forall|x: u64| #![trigger current@.contains(x)]
                current@.contains(x) ==> chain.denotes().contains(x),
            // (c) 없던 것이 생기지 않는다 — chain은 current ∪ 이미 처리한 target.
            forall|x: u64| #![trigger chain.denotes().contains(x)]
                chain.denotes().contains(x) ==>
                    current@.contains(x) || target_ids@.subrange(i as int, n as int).contains(x),
            // (d) 이미 처리한 target 원소는 chain 안에 있다(= anchor 해석 가능).
            forall|j: int| #![trigger target_ids@[j]]
                i <= j < n ==> chain.denotes().contains(target_ids@[j]),
            next_anchor == if i == n { None::<u64> } else { Some(target_ids@[i as int]) },
            forall|k: int| #![trigger ops@[k]]
                0 <= k < ops@.len() ==>
                    ops@[k].idx < n && ops@[k].spawn == target_fresh@[ops@[k].idx as int],
        decreases i,
    {
        i = i - 1;
        let e = target_ids[i];
        let fresh = target_fresh[i];
        let ghost prev_denotes = chain.denotes();

        proof {
            // e는 target의 i번째이고 중복이 없으므로 [i+1, n) 구간에 없다.
            assert(!target_ids@.subrange(i as int + 1, n as int).contains(e)) by {
                reveal(seq_nodup);
                let sub = target_ids@.subrange(i as int + 1, n as int);
                assert forall|j: int| #![trigger sub[j]]
                    0 <= j < sub.len() implies sub[j] != e by {
                    assert(sub[j] == target_ids@[i as int + 1 + j]);
                }
            }
        }

        if fresh {
            // Fresh: chain에 없음이 (c)로부터 따라온다 → insert 사전조건 충족.
            match next_anchor {
                Some(a) => {
                    proof {
                        // a == target_ids[i+1] 이고, 불변식 (d)가 그것이 chain
                        // 안에 있음을 준다 → index_of_first가 범위 안.
                        assert(chain.denotes().contains(a));
                        chain.denotes().index_of_first_ensures(a);
                        lemma_insert_has_new(chain.denotes(),
                            chain.denotes().index_of_first(a).unwrap(), e);
                    }
                    let ghost j0 = chain.denotes().index_of_first(a).unwrap();
                    chain.insert_before(e, a);
                    proof {
                        assert forall|x: u64| #![trigger chain.denotes().contains(x)]
                            chain.denotes().contains(x) <==> (prev_denotes.contains(x) || x == e) by {
                            if chain.denotes().contains(x) { lemma_insert_only(prev_denotes, j0, e, x); }
                            if prev_denotes.contains(x) { lemma_insert_keeps(prev_denotes, j0, e, x); }
                            if x == e { lemma_insert_has_new(prev_denotes, j0, e); }
                        }
                    }
                }
                None => {
                    chain.push_back(e);
                    proof {
                        assert forall|x: u64| #![trigger chain.denotes().contains(x)]
                            chain.denotes().contains(x) <==> (prev_denotes.contains(x) || x == e) by {
                            if chain.denotes().contains(x) { lemma_push_only(prev_denotes, e, x); }
                            if prev_denotes.contains(x) { lemma_push_keeps(prev_denotes, e, x); }
                            if x == e { assert(prev_denotes.push(e)[prev_denotes.len() as int] == e); }
                        }
                    }
                }
            }
            assert(same_plus(chain.denotes(), prev_denotes, e));
            ops.push(PlanOp { idx: i, id: e, spawn: true, before: next_anchor });
        } else {
            // `keep[i]`는 호출부(LIS 변형)가 "이 원소는 이미 상대 순서가
            // 맞으니 건드리지 말라"고 표시한 것. 스킵은 write를 내지 않고
            // chain도 그대로 두므로 chain은 계속 DOM과 일치한다 — 다만
            // **LIS 판단 자체의 정당성은 여기서 검증하지 않는다**(호출부 책임).
            let already_adjacent = keep[i] || match next_anchor {
                Some(a) => chain.next_is(e, a),
                None => chain.is_tail(e),
            };
            if !already_adjacent {
                // Retained: (b)로부터 chain에 있음 → unlink 사전조건 충족.
                let ghost before = chain.denotes();
                proof { chain.no_dup_opaque(); }
                chain.unlink(e);
                proof {
                    // 중복이 없었으므로 unlink로 e는 완전히 사라졌다 —
                    // 이어지는 삽입의 사전조건(!contains(e))이 여기서 나온다.
                    lemma_remove_value_no_dup(before, e);
                    assert(!chain.denotes().contains(e));
                    // anchor는 e와 다른 원소이므로 unlink에도 살아남는다.
                    if let Some(a) = next_anchor {
                        // anchor는 target[i+1], e는 target[i] — 자리가 다르므로
                        // 중복 없음에서 서로 다르다. 양화자는 이 `by` 블록
                        // 안에서만 펼친다(불투명화의 의미를 지키기 위해).
                        assert(a != e) by { reveal(seq_nodup); }
                        lemma_remove_value_keeps_other(before, e, a);
                    }
                }
                let ghost mid = chain.denotes();
                match next_anchor {
                    Some(a) => {
                        proof {
                            assert(chain.denotes().contains(a));
                            chain.denotes().index_of_first_ensures(a);
                            lemma_insert_has_new(chain.denotes(),
                                chain.denotes().index_of_first(a).unwrap(), e);
                        }
                        let ghost j0 = chain.denotes().index_of_first(a).unwrap();
                        chain.insert_before(e, a);
                        proof {
                            assert forall|x: u64| #![trigger chain.denotes().contains(x)]
                                chain.denotes().contains(x) <==> (prev_denotes.contains(x) || x == e) by {
                                if chain.denotes().contains(x) {
                                    lemma_insert_only(mid, j0, e, x);
                                    if x != e { lemma_remove_value_only(prev_denotes, e, x); }
                                }
                                if prev_denotes.contains(x) && x != e {
                                    lemma_remove_value_keeps_other(prev_denotes, e, x);
                                    lemma_insert_keeps(mid, j0, e, x);
                                }
                                if x == e { lemma_insert_has_new(mid, j0, e); }
                            }
                        }
                    }
                    None => {
                        chain.push_back(e);
                        proof {
                            assert forall|x: u64| #![trigger chain.denotes().contains(x)]
                                chain.denotes().contains(x) <==> (prev_denotes.contains(x) || x == e) by {
                                if chain.denotes().contains(x) {
                                    lemma_push_only(mid, e, x);
                                    if x != e { lemma_remove_value_only(prev_denotes, e, x); }
                                }
                                if prev_denotes.contains(x) && x != e {
                                    lemma_remove_value_keeps_other(prev_denotes, e, x);
                                    lemma_push_keeps(mid, e, x);
                                }
                                if x == e { assert(mid.push(e)[mid.len() as int] == e); }
                            }
                        }
                    }
                }
                assert(same_plus(chain.denotes(), prev_denotes, e));
                ops.push(PlanOp { idx: i, id: e, spawn: false, before: next_anchor });
            } else {
                // 스킵: 표기 불변. e는 Retained이므로 이미 들어 있다.
                assert(current@.contains(e));
                assert(same_plus(chain.denotes(), prev_denotes, e));
            }
        }
        proof {
            // 세 분기 모두 결과가 같은 모양이다: 표기의 원소 집합이
            // "이전 집합 ∪ {e}"가 된다(스킵 분기는 e가 이미 있으므로 그대로).
            // 이 하나로 불변식 (b)/(c)/(d)를 균일하게 유도한다.
            assert(same_plus(chain.denotes(), prev_denotes, e));
            assert forall|x: u64| #![trigger current@.contains(x)]
                current@.contains(x) implies chain.denotes().contains(x) by {
                if current@.contains(x) {
                    assert(prev_denotes.contains(x));
                }
            }
            let sub_new = target_ids@.subrange(i as int, n as int);
            let sub_old = target_ids@.subrange(i as int + 1, n as int);
            assert(sub_new[0] == e);
            assert(sub_new.contains(e));
            // 이전 구간은 새 구간의 부분이다 — 불변식 (c)를 한 칸 넓히는 단계.
            assert forall|x: u64| sub_old.contains(x) implies sub_new.contains(x) by {
                let k = choose|k: int| 0 <= k < sub_old.len() && sub_old[k] == x;
                assert(sub_old[k] == target_ids@[i as int + 1 + k]);
                assert(sub_new[k + 1] == target_ids@[i as int + 1 + k]);
            }
            assert forall|x: u64| #![trigger chain.denotes().contains(x)]
                chain.denotes().contains(x)
                implies current@.contains(x) || sub_new.contains(x) by {
                if chain.denotes().contains(x) && x != e {
                    assert(prev_denotes.contains(x));
                }
            }
            assert forall|j: int| #![trigger target_ids@[j]]
                i <= j < n implies chain.denotes().contains(target_ids@[j]) by {
                if j > i { assert(prev_denotes.contains(target_ids@[j])); }
            }
        }
        next_anchor = Some(e);
    }
    ops
}

} // verus!
