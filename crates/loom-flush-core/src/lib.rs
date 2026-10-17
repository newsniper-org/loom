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
//!
//! **2026-10-17 정정**: 위의 "`cargo verus verify`가 검증하는 코드다"는
//! 명령이 낡았다. 2026-08-02 마이그레이션 이후 `cargo verus verify`는 vstd
//! 불일치로 실행되지 않고(이 크레이트 Cargo.toml 주석과
//! `formal-verification/verus/README.md`), 실제 검증 명령은
//! `verus --crate-type=lib crates/loom-flush-core/src/lib.rs`다. 기준선은
//! 2026-08-11의 **90 verified, 0 errors**(LIS 건전성 포함). 범위의 한계도 함께
//! 적어 둔다: 덮는 것은 D/E 계열 경로뿐(C·K 제외)이고, 적용 루프는 spec
//! 함수(`apply_ops`)로 모델링돼 있으며, 원시 입력의 구조적 사전조건 넷은
//! 호출부(`list.rs`)의 `debug_assert!`로만 지켜진다(미검증). LIS 최적성은
//! 증명하지 않았다.
//!
//! 또 위의 "`list.rs`의 `place_in_order`가 이 크레이트의 `Chain`을 호출한다"도
//! 낡았다. 2026-07-28~08-11 단계적 이관으로 방출 루프(`plan_writes`/
//! `plan_writes_converging`), `current`/`target` 조립(`plan_inputs`,
//! `plan_from_selection`), LIS `keep` 조립(`lis_keep`,
//! `plan_lis_from_selection_auto`)이 모두 이 크레이트 안으로 들어왔고,
//! `place_in_order`/`place_in_order_lis`는 2026-08-11 사라졌다. `list.rs`는
//! `Chain`을 직접 만지지 않고 이 크레이트가 돌려준 계획을 `emit_plan`으로
//! `DomWrite`에 옮겨 적기만 한다. 아래 본문의 "`place_in_order`의 …" 표현은
//! 이 코드가 **어디서 옮겨 왔는지**를 가리키는 출처 표기로 읽을 것.

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
    ///
    /// **`opaque`인 이유**(2026-08-08). `closed`는 *모듈 밖*에서만 본문을
    /// 가린다. 이 크레이트는 전부 한 모듈이라 `plan_writes`/
    /// `plan_writes_converging`의 루프 불변식에 있는 `chain.well_formed()`가
    /// 매 반복 `chain_denotes`의 **8개 논리곱 — 그중 4개가 양화자, 하나는
    /// `no_duplicates`** — 으로 펼쳐지고 있었다. B1에서 걷어낸 바로 그
    /// 2변수 양화자가 `well_formed()`를 통해 뒷문으로 다시 들어온 것이다
    /// (README의 "가설 ②"가 겨냥했던 지점 — 그때는 `chain_denotes` **안쪽**의
    /// `no_duplicates`만 가렸고, 그러면 나머지 7개 논리곱은 그대로 남는다).
    ///
    /// 불투명으로 두면 두 루프는 "well_formed다"라는 **명제만** 들고 다닌다.
    /// 펼치는 것은 그것이 실제로 필요한 `Chain`의 메서드 안뿐이고, 그쪽은
    /// 루프가 아니라 직선 코드라 감당된다.
    #[verifier::opaque]
    pub closed spec fn well_formed(&self) -> bool {
        chain_denotes(self.next@, self.prev@, self.head, self.tail, self.denotation@)
    }

    /// 빈 chain.
    pub fn new() -> (r: Chain)
        ensures r.well_formed(), r.denotes() =~= Seq::<u64>::empty(),
    {
        broadcast use group_hash_axioms;
        reveal(Chain::well_formed);
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
        reveal(Chain::well_formed);
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
        reveal(Chain::well_formed);
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
        reveal(Chain::well_formed);
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
        reveal(Chain::well_formed);
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
        reveal(Chain::well_formed);
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
        reveal(Chain::well_formed);
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
///
/// **`rlimit(60)`인 이유**(2026-08-09). 이 정리는 기본 rlimit 경계에 **붙어
/// 있다** — 모듈에 새 심볼(`pub proof fn` 하나)을 추가하기만 해도 재현성 있게
/// 넘어간다. 자기 자신의 증명은 그대로이고 예산만 올린 것이므로 `assume` 류
/// 우회가 아니며, 이 파일은 이미 `rlimit(400)/(2000)/(4000)`을 쓰고 있다.
#[verifier::rlimit(60)]
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
/// `plan_writes`(LIS 변형용, `keep` 있음)와 갈라 둔 이유는 **불변식의 모양이
/// 실제로 다르기 때문**이다. 두 경로를 한 함수에 담으면 불변식이 전부
/// 조건부가 되어 SMT가 감당하지 못한다(2026-07-28에 실제로 그래서 실패했다 —
/// `formal-verification/verus/README.md` 참고).
///
/// **2026-08-09 정정**: 이 주석은 원래 "LIS 쪽은 수렴을 주장할 수 없다"고
/// 적고 있었다. 이제 `plan_writes`도 **같은 모양의 수렴 사후조건을 갖는다.**
/// 다만 두 경로는 여전히 다르고, 차이는 두 가지다.
///
/// 1. **사전조건**: LIS 쪽은 `current`에 target 밖 원소가 없어야 한다(조건 Q).
///    이쪽은 필요 없다 — 비인접이면 무조건 옮기므로 외래 원소가 저절로 앞으로
///    밀려나기 때문이다. Q는 `keep` 단락이 사 오는 비용이다.
/// 2. **불변식**: 이쪽은 **접미사** 불변식("k개 처리 후 DOM의 뒤쪽 k개 =
///    target의 뒤쪽 k개")으로 닫힌다. LIS 쪽에서는 그것이 중간에 깨져서
///    분할점을 함께 든 **부분수열** 불변식(`split_inv`)을 써야 한다.
///
/// 여전히 미증명인 것은 `lis_indices`가 사전조건 P를 세운다는 것(LIS 건전성)
/// 이고, 그건 호출부의 책임으로 남아 있다.
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
                // **`well_formed()`를 불투명으로 돌린 뒤 필요해진 두 줄**
                // (2026-08-08). 이 시점에 `e`는 표기에서 빠져 있고(Retained면
                // `unlink`가 뺐고, Fresh면 애초에 없었다) 따라서 `place_before`의
                // 첫 단계 `remove_value(e)`는 항등이다. 전에는 `chain_denotes`가
                // 문맥에 펼쳐져 있어 프루버가 이걸 곁다리로 주웠는데, 이제는
                // 명시해야 한다.
                assert(!d_mid.contains(e));
                lemma_remove_value_absent(d_mid, e);
                if i < n { lemma_place_extends_suffix(d_mid, k_old, e); }
            }
            match next_anchor {
                Some(a) => {
                    proof { d_mid.index_of_first_ensures(a); }
                    let ghost j0 = d_mid.index_of_first(a).unwrap();
                    chain.insert_before(e, a);
                    proof {
                        // `insert_before`의 사후조건(표기 = `insert(j0, e)`)과
                        // `place_before`의 정의를 명시적으로 잇는다 — 위
                        // `remove_value` 항등이 있어야 두 항이 같아진다.
                        assert(chain.denotes() =~= place_before(d_mid, e, Some(a)));
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

/// **`current` 안에서의 위치** — 사전조건 P를 쓰기 위한 보조.
///
/// `Seq::index_of_first`는 `Option<int>`라 P의 본문에서 매번 언랩하면
/// `->Some_0` 항이 양화자 몸통에 섞여 트리거가 지저분해진다. 중복 없는
/// 시퀀스에서는 위치가 유일하므로, **언랩을 여기 한 곳에 가둔 정수 함수**를
/// 쓴다. `s`에 `x`가 없을 때의 값은 규정하지 않는다 — P가 이 함수를 쓰는
/// 자리(= `keep`으로 표시된 Retained 원소)는 `plan_writes`의 기존 사전조건이
/// `current@.contains(...)`를 이미 보장한다.
///
/// **`open`으로 두는 이유**: 이 함수는 아래 `kept_order_preserved`의 몸통
/// **안에서만** 나타나고 그쪽이 불투명하므로, 이 정의 공리는 P를 `reveal`하기
/// 전에는 어떤 항과도 매칭되지 않는다(트리거가 될 `pos(..)` 항 자체가 문맥에
/// 뜨지 않는다). 굳이 불투명으로 이중으로 가둘 이유가 없다.
pub open spec fn pos(s: Seq<u64>, x: u64) -> int {
    s.index_of_first(x).unwrap()
}

/// **사전조건 P** — LIS 변형이 넘기는 `keep`이 지켜야 하는 것.
///
/// > `keep`으로 표시된(그리고 fresh가 아닌) 원소들의 `current` 내 상대 순서가
/// > target 순서와 같다.
///
/// `formal-verification/verus/lis-keep-exhaustive.py`의 전수 탐색(2026-08-02)이
/// 이것이 **필요충분**임을 확정했다: P를 만족하는 keep 조합 28,215건에서 순서
/// 위반 0건, P를 위반하는 2,484건에서 **100%** 순서가 깨졌다. 지금까지 이
/// 명제는 `place_in_order_lis`의 `debug_assert!`와 주석에만 있었다.
///
/// **불투명인 이유**: 본문이 `a`, `b` 두 인덱스에 대한 2변수 양화자다 —
/// `Seq::no_duplicates()`가 이 모듈에서 인스턴스화의 88%를 먹었던 바로 그
/// 형태. 래퍼로 가두면 `plan_writes`의 검증 조건은 "P가 성립한다"는 **명제만**
/// 들고 다니고 양화자는 보지 않는다(`seq_nodup`과 같은 처방).
#[verifier::opaque]
pub open spec fn kept_order_preserved(
    current: Seq<u64>,
    target_ids: Seq<u64>,
    target_fresh: Seq<bool>,
    keep: Seq<bool>,
) -> bool {
    forall|a: int, b: int| #![trigger keep[a], keep[b]]
        0 <= a < b < target_ids.len()
            && keep[a] && !target_fresh[a]
            && keep[b] && !target_fresh[b]
            ==> pos(current, target_ids[a]) < pos(current, target_ids[b])
}

/// **음성 대조 — P가 공허하지 않음**.
///
/// "통과했다"만으로는 명세가 실제로 무언가를 말하는지 알 수 없다(이 저장소의
/// 음성 대조 관행, `formal-verification/verus/README.md`). `requires`는 약할수록
/// 통과하기 쉬우므로, **항상 참인 술어를 사전조건에 얹는 것은 아무것도 하지
/// 않는 것과 같다** — 그래서 P가 실제로 무언가를 *거절*하는지 못박는다.
///
/// `current = [7, 9]`인데 target이 `[9, 7]`이고 둘 다 Retained·`keep` 표시라면,
/// 표시된 원소들의 `current` 내 순서(7 → 9)가 target 순서(9 → 7)와 **반대**이므로
/// P는 거짓이어야 한다. 전수 탐색이 "P 위반 → 100% 순서가 깨진다"고 말한 바로
/// 그 모양이고, 실제로 이 입력에서 LIS는 둘 중 하나만 표시한다.
pub proof fn lemma_kept_order_rejects_inversion()
    ensures
        !kept_order_preserved(
            seq![7u64, 9u64],
            seq![9u64, 7u64],
            seq![false, false],
            seq![true, true],
        ),
{
    reveal(kept_order_preserved);
    let current = seq![7u64, 9u64];
    let target_ids = seq![9u64, 7u64];
    let keep = seq![true, true];
    assert(current[0] == 7u64);
    assert(current[1] == 9u64);
    assert(current.contains(9u64));
    assert(current.contains(7u64));
    current.index_of_first_ensures(9u64);
    current.index_of_first_ensures(7u64);
    assert(pos(current, 9u64) == 1);
    assert(pos(current, 7u64) == 0);
    assert(target_ids[0] == 9u64);
    assert(target_ids[1] == 7u64);
    // 트리거(`keep[a]`, `keep[b]`)에 맞는 ground term을 세워 (a, b) = (0, 1)
    // 인스턴스화를 끌어낸다 — 그 인스턴스가 `1 < 0`이라 P가 거짓이 된다.
    assert(keep[0] == true);
    assert(keep[1] == true);
}

/// **양성 대조 — P가 항상 거짓인 술어가 아님** (2026-08-08 추가).
///
/// 위 음성 대조만으로는 부족하다. `requires`가 **항상 거짓**이어도 그 정리는
/// 통과하는데, 항상 거짓인 사전조건은 `plan_writes`를 검증 코드에서 아예
/// 호출 불가능하게 만든다 — 통과하지만 쓸모없는, 다른 종류의 공허다.
/// 그래서 **같은 `current`/`target_ids` 위에서 `keep`만 바꿔** 값이 실제로
/// 갈리는 것을 기계검사한다.
pub proof fn lemma_kept_order_accepts_increasing()
    ensures
        kept_order_preserved(
            seq![7u64, 9u64],
            seq![9u64, 7u64],
            seq![false, false],
            seq![false, true],
        ),
{
    reveal(kept_order_preserved);
    // `keep = [false, true]` — 표시된 것이 하나뿐이라 비교할 쌍이 없다.
    // 위 `lemma_kept_order_rejects_inversion`과 `current`/`target_ids`가
    // **완전히 같고** `keep`만 다른데 값이 갈린다는 것이 요점이다.
}

/// **`!fresh` 조항이 실제로 무언가를 하는지** (2026-08-08 추가).
///
/// 이것이 없으면 P의 정의에서 `&& !target_fresh[a] && !target_fresh[b]`를
/// 통째로 지워도 다른 모든 항목이 그대로 통과한다 — 즉 그 조항은 검사되지
/// 않는 채 남는다. 여기서는 **역전된 두 원소 중 하나를 fresh로 표시**해
/// 조항이 그 쌍을 실제로 제외하는 것을 확인한다.
///
/// (fresh 원소는 아직 DOM에 없으므로 `current` 내 위치를 말할 수 없다 —
/// `plan_writes`의 기존 `requires`가 그것을 이미 못박고 있고, P가 그 원소를
/// 비교 대상에서 빼는 이유가 그것이다.)
pub proof fn lemma_kept_order_ignores_fresh()
    ensures
        // 같은 역전(9가 7보다 앞) 인데 9 쪽이 fresh라 쌍이 성립하지 않는다.
        kept_order_preserved(
            seq![7u64, 9u64],
            seq![9u64, 7u64],
            seq![true, false],
            seq![true, true],
        ),
{
    reveal(kept_order_preserved);
}

// ---------------------------------------------------------------------------
// LIS 경로의 수렴 증명 기계 — `Seq::filter` 기반 (2026-08-09)
//
// **왜 접미사가 아니라 filter인가**. `plan_writes_converging`의 증명은
// "k개 처리 후 DOM의 뒤쪽 k개 = target의 뒤쪽 k개"라는 **접미사 불변식**에
// 얹혀 있는데, LIS 경로에서는 이것이 중간에 깨진다(최소 반례
// `current=[0,1] target=[1,0] keep=[false,true]`). keep으로 스킵된 원소와
// target 접미사 사이에 다른 원소가 끼기 때문이다. 그런데도 최종 결과가 맞는
// 이유는 끼어든 것이 전부 `target[0..i-1]`에 속해 나중에 왼쪽으로 치워지기
// 때문이고, 그 사실을 붙잡는 술어는 접미사가 아니라 **부분수열**이다.
//
// 여기서 쓰는 불변식은 부분수열형을 **분할점 `sp`와 함께** 들고 다니는 판본
// (`split_inv`)이다. `sp`는 DOM 안에서 `target[i]`가 있는 자리이고,
//
//   * `sp` **앞**을 아직 처리하지 않은 kept 원소로 거르면 `keptseq(i)`,
//   * `sp` **뒤**를 `target[i..]`로 거르면 `target[i..]` 그대로.
//
// 분할점을 명시적으로 들고 다니는 것이 요점이다. 분할점 없이
// "filter(dom, kept ∪ target[i..]) == keptseq ++ target[i..]"만 들고 있으면
// 매 스텝 `A + B`의 쪼개는 자리를 다시 찾아야 하는데(그러려면 filter 결과의
// 무중복성이 필요하다), `sp`를 함께 옮기면 세 분기 모두 자리가 **주어진다**.
// ---------------------------------------------------------------------------

/// `s`의 원소를 전부 걸러내지 않으면 filter는 빈 것. (vstd의
/// `lemma_all_neg_filter_empty`는 `Seq::all` 형태라 인덱스 형태로 쓰려면
/// 감싸야 한다 — 직접 쓰는 편이 짧다.)
pub proof fn lemma_filter_none(s: Seq<u64>, p: spec_fn(u64) -> bool)
    requires forall|j: int| 0 <= j < s.len() ==> !p(#[trigger] s[j]),
    ensures s.filter(p) =~= Seq::<u64>::empty(),
    decreases s.len(),
{
    reveal(Seq::<_>::filter);
    if s.len() > 0 {
        let r = s.drop_last();
        assert forall|j: int| 0 <= j < r.len() implies !p(#[trigger] r[j]) by { assert(r[j] == s[j]); }
        lemma_filter_none(r, p);
        assert(s.last() == s[s.len() - 1]);
    }
}

/// 두 술어가 원소마다 일치하면 filter도 같다. **술어 확장 단계의 핵심**이고
/// vstd에 없다.
pub proof fn lemma_filter_agree(s: Seq<u64>, p: spec_fn(u64) -> bool, q: spec_fn(u64) -> bool)
    requires forall|j: int| 0 <= j < s.len() ==> p(#[trigger] s[j]) == q(s[j]),
    ensures s.filter(p) =~= s.filter(q),
    decreases s.len(),
{
    reveal(Seq::<_>::filter);
    if s.len() > 0 {
        let r = s.drop_last();
        assert forall|j: int| 0 <= j < r.len() implies p(#[trigger] r[j]) == q(r[j]) by {
            assert(r[j] == s[j]);
        }
        lemma_filter_agree(r, p, q);
        assert(s.last() == s[s.len() - 1]);
    }
}

/// 전부 술어를 만족하면 filter는 항등 — **끝맺음**이 이것으로 닫힌다.
pub proof fn lemma_filter_all(s: Seq<u64>, p: spec_fn(u64) -> bool)
    requires forall|j: int| 0 <= j < s.len() ==> p(#[trigger] s[j]),
    ensures s.filter(p) =~= s,
    decreases s.len(),
{
    reveal(Seq::<_>::filter);
    if s.len() > 0 {
        let r = s.drop_last();
        assert forall|j: int| 0 <= j < r.len() implies p(#[trigger] r[j]) by { assert(r[j] == s[j]); }
        lemma_filter_all(r, p);
        assert(s.last() == s[s.len() - 1]);
        assert(r.push(s.last()) =~= s);
    }
}

/// 싱글턴 filter. `reveal_with_fuel` 두 걸음으로 풀리므로 vstd의
/// `lemma_filter_push`(= `group_seq_extra`, 기본 broadcast **아님**)를 한
/// 번도 들여오지 않아도 된다 — 모듈에 새 broadcast 트리거를 붙이지 않는다는
/// 뜻이고, 이 파일의 자원 이력상 그게 중요하다.
pub proof fn lemma_filter_singleton(e: u64, p: spec_fn(u64) -> bool)
    ensures seq![e].filter(p) =~= if p(e) { seq![e] } else { Seq::<u64>::empty() },
{
    reveal_with_fuel(Seq::<_>::filter, 2);
    assert(seq![e].drop_last() =~= Seq::<u64>::empty());
    assert(Seq::<u64>::empty().filter(p) =~= Seq::<u64>::empty());
}

/// 술어를 만족하지 않는 원소를 지워도 filter는 그대로.
pub proof fn lemma_filter_remove_nonmatching(s: Seq<u64>, i: int, p: spec_fn(u64) -> bool)
    requires 0 <= i < s.len(), !p(s[i]),
    ensures s.remove(i).filter(p) =~= s.filter(p),
{
    let a = s.subrange(0, i);
    let b = s.subrange(i + 1, s.len() as int);
    assert(s =~= a + seq![s[i]] + b);
    assert(s.remove(i) =~= a + b);
    lemma_filter_singleton(s[i], p);
}

/// **filter의 마지막 원소에서 쪼개기** — keep 스킵 분기의 심장.
/// `s.filter(p) == x.push(e)`이면 `e`는 `s` 안 어딘가(`q`)에 있고, 그 앞은
/// `x`로 걸러지며 그 뒤에서는 아무것도 걸리지 않는다. filter가 `drop_last`
/// 재귀라 오른쪽에서 왼쪽으로 가는 이 귀납과 정확히 맞물린다.
pub proof fn lemma_filter_split_last(s: Seq<u64>, p: spec_fn(u64) -> bool, x: Seq<u64>, e: u64)
    -> (q: int)
    requires s.filter(p) =~= x.push(e),
    ensures
        0 <= q < s.len(),
        s[q] == e,
        s.subrange(0, q).filter(p) =~= x,
        s.subrange(q + 1, s.len() as int).filter(p) =~= Seq::<u64>::empty(),
    decreases s.len(),
{
    reveal(Seq::<_>::filter);
    assert(s.len() > 0) by {
        if s.len() == 0 { assert(s.filter(p).len() == 0); assert(x.push(e).len() > 0); }
    }
    let r = s.drop_last();
    assert(s.last() == s[s.len() - 1]);
    if p(s.last()) {
        assert(s.filter(p) =~= r.filter(p).push(s.last()));
        assert(r.filter(p).len() == x.len());
        assert(s.last() == e) by {
            assert(r.filter(p).push(s.last())[x.len() as int] == s.last());
            assert(x.push(e)[x.len() as int] == e);
        }
        assert(r.filter(p) =~= x) by {
            assert forall|m: int| 0 <= m < x.len() implies r.filter(p)[m] == x[m] by {
                assert(r.filter(p).push(s.last())[m] == r.filter(p)[m]);
                assert(x.push(e)[m] == x[m]);
            }
        }
        let q = s.len() - 1;
        assert(s.subrange(0, q) =~= r);
        assert(s.subrange(q + 1, s.len() as int) =~= Seq::<u64>::empty());
        assert(Seq::<u64>::empty().filter(p) =~= Seq::<u64>::empty());
        q
    } else {
        assert(s.filter(p) =~= r.filter(p));
        let q = lemma_filter_split_last(r, p, x, e);
        assert(s[q] == r[q]);
        assert(s.subrange(0, q) =~= r.subrange(0, q));
        assert(s.subrange(q + 1, s.len() as int)
               =~= r.subrange(q + 1, r.len() as int) + seq![s.last()]);
        lemma_filter_singleton(s.last(), p);
        q
    }
}

/// 아직 처리하지 않은(target 인덱스 < `i`) **kept 원소**인가 — 값 위의 술어.
/// `keep`은 target 인덱스 위에 있고 filter의 술어는 값 위에 있으므로 존재
/// 양화가 하나 들어간다(`target_ids`가 무중복이라 증인 `j`는 유일).
pub open spec fn kept_lt(t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, x: u64) -> bool {
    exists|j: int| 0 <= j < i && #[trigger] t[j] == x && kp[j] && !fr[j]
}

/// `target[i..]`에 속하는가.
pub open spec fn in_suffix(t: Seq<u64>, i: int, x: u64) -> bool {
    exists|j: int| i <= j < t.len() && #[trigger] t[j] == x
}

/// 불변식이 보는 원소 집합: 아직 처리 안 한 kept 원소 ∪ `target[i..]`.
pub open spec fn sel(t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int) -> spec_fn(u64) -> bool {
    |x: u64| kept_lt(t, fr, kp, i, x) || in_suffix(t, i, x)
}

/// 아직 처리하지 않은 kept 원소들을 **`current` 순서로** 늘어놓은 것.
///
/// **`target` 순서가 아니라 `current` 순서로 정의한 것이 이 증명의 설계점**이다.
/// target 쪽으로 정의하면 루프 진입 시 `current.filter(kept) == target.filter(kept)`
/// 를 세워야 하고 그건 "순서 보존 선택 ⟹ filter 동일"이라는 별도의 무거운
/// 정리다. `current` 쪽으로 정의하면 진입 조건이 **항진명제**가 되고, P는
/// keep 스킵 한 걸음(`lemma_keptseq_push`)에서만 소비된다.
pub open spec fn keptseq(c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int) -> Seq<u64> {
    c.filter(|x: u64| kept_lt(t, fr, kp, i, x))
}

/// **LIS 경로의 루프 불변식** — 분할점 `sp`를 명시적으로 들고 다니는 부분수열형.
///
/// **불투명인 이유**: 본문에 filter 항이 둘 있고 그 술어 안에 존재 양화가
/// 둘 들어간다. `seq_nodup`/`Chain::well_formed`와 같은 처방으로, 루프는
/// "불변식이 성립한다"는 명제만 들고 다니고 펼치는 것은 아래 단계 보조정리
/// 안뿐이다.
#[verifier::opaque]
pub open spec fn split_inv(
    d: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, sp: int,
) -> bool {
    &&& 0 <= sp <= d.len()
    &&& (i < t.len() ==> sp < d.len() && d[sp] == t[i])
    &&& (i >= t.len() ==> sp == d.len())
    &&& d.subrange(0, sp).filter(sel(t, fr, kp, i)) =~= keptseq(c, t, fr, kp, i)
    &&& d.subrange(sp, d.len() as int).filter(sel(t, fr, kp, i)) =~= t.subrange(i, t.len() as int)
}

/// 불변식에서 **분할점의 위치 정보만** 꺼낸다 — 루프가 filter 항을 보지
/// 않고 anchor의 자리를 알 수 있게 하는 투영.
pub proof fn lemma_split_inv_anchor(
    d: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, sp: int,
)
    requires split_inv(d, c, t, fr, kp, i, sp),
    ensures
        0 <= sp <= d.len(),
        i < t.len() ==> sp < d.len() && d[sp] == t[i],
        i >= t.len() ==> sp == d.len(),
{ reveal(split_inv); }

/// **루프 진입** — `d == current`, `i == n`, `sp == |current|`에서 성립.
/// `keptseq`를 `current` 위에 정의한 덕에 술어 일치 하나로 끝난다.
pub proof fn lemma_split_inv_init(c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>)
    ensures split_inv(c, c, t, fr, kp, t.len() as int, c.len() as int),
{
    reveal(split_inv);
    let s = sel(t, fr, kp, t.len() as int);
    let k = |x: u64| kept_lt(t, fr, kp, t.len() as int, x);
    assert(c.subrange(0, c.len() as int) =~= c);
    assert forall|m: int| 0 <= m < c.len() implies s(#[trigger] c[m]) == k(c[m]) by {
        if in_suffix(t, t.len() as int, c[m]) {
            let j = choose|j: int| t.len() <= j < t.len() && #[trigger] t[j] == c[m];
            assert(t.len() <= j < t.len());
        }
    }
    lemma_filter_agree(c, s, k);
    assert(c.subrange(c.len() as int, c.len() as int) =~= Seq::<u64>::empty());
    assert(Seq::<u64>::empty().filter(s) =~= Seq::<u64>::empty());
    assert(t.subrange(t.len() as int, t.len() as int) =~= Seq::<u64>::empty());
}

/// `target[i-1]`은 `sel(i)`에 걸리지 않는다 — 이동/인접스킵 분기의 전제.
/// (kept가 아니므로 `kept_lt(i)`에 없고, 무중복이므로 `target[i..]`에도 없다.)
pub proof fn lemma_sel_not_e(t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        !(kp[i - 1] && !fr[i - 1]),
        seq_nodup(t),
    ensures !sel(t, fr, kp, i)(t[i - 1]),
{
    lemma_nodup_unwrap(t);
    if kept_lt(t, fr, kp, i, t[i - 1]) {
        let j = choose|j: int| 0 <= j < i && #[trigger] t[j] == t[i - 1] && kp[j] && !fr[j];
        assert(t[j] == t[i - 1]);
        assert(j == i - 1);
    }
    if in_suffix(t, i, t[i - 1]) {
        let j = choose|j: int| i <= j < t.len() && #[trigger] t[j] == t[i - 1];
        assert(t[j] == t[i - 1]);
        assert(j == i - 1);
    }
}

/// kept 원소를 처리할 때 술어 집합은 **변하지 않는다**(`e`가 kept 쪽에서
/// 접미사 쪽으로 옮겨갈 뿐).
pub proof fn lemma_sel_same_kept(t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        kp[i - 1], !fr[i - 1],
    ensures forall|x: u64| sel(t, fr, kp, i - 1)(x) == #[trigger] sel(t, fr, kp, i)(x),
{
    assert forall|x: u64| sel(t, fr, kp, i - 1)(x) == #[trigger] sel(t, fr, kp, i)(x) by {
        if sel(t, fr, kp, i - 1)(x) {
            if kept_lt(t, fr, kp, i - 1, x) {
                let j = choose|j: int| 0 <= j < i - 1 && #[trigger] t[j] == x && kp[j] && !fr[j];
                assert(0 <= j < i && t[j] == x && kp[j] && !fr[j]);
            } else {
                let j = choose|j: int| i - 1 <= j < t.len() && #[trigger] t[j] == x;
                if j == i - 1 {
                    assert(0 <= (i - 1) < i && t[i - 1] == x && kp[i - 1] && !fr[i - 1]);
                } else {
                    assert(i <= j < t.len() && t[j] == x);
                }
            }
        }
        if sel(t, fr, kp, i)(x) {
            if kept_lt(t, fr, kp, i, x) {
                let j = choose|j: int| 0 <= j < i && #[trigger] t[j] == x && kp[j] && !fr[j];
                if j == i - 1 { assert(i - 1 <= j < t.len() && t[j] == x); }
                else { assert(0 <= j < i - 1 && t[j] == x && kp[j] && !fr[j]); }
            } else {
                let j = choose|j: int| i <= j < t.len() && #[trigger] t[j] == x;
                assert(i - 1 <= j < t.len() && t[j] == x);
            }
        }
    }
}

/// kept가 아닌 원소를 처리할 때 술어 집합은 `{e}`만큼 **넓어진다**.
pub proof fn lemma_sel_widen_notkept(t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        !(kp[i - 1] && !fr[i - 1]),
        seq_nodup(t),
    ensures
        forall|x: u64| sel(t, fr, kp, i - 1)(x)
            == (#[trigger] sel(t, fr, kp, i)(x) || x == t[i - 1]),
        !sel(t, fr, kp, i)(t[i - 1]),
{
    lemma_sel_not_e(t, fr, kp, i);
    assert forall|x: u64| sel(t, fr, kp, i - 1)(x)
        == (#[trigger] sel(t, fr, kp, i)(x) || x == t[i - 1]) by {
        if sel(t, fr, kp, i - 1)(x) {
            if kept_lt(t, fr, kp, i - 1, x) {
                let j = choose|j: int| 0 <= j < i - 1 && #[trigger] t[j] == x && kp[j] && !fr[j];
                assert(0 <= j < i && t[j] == x && kp[j] && !fr[j]);
            } else {
                let j = choose|j: int| i - 1 <= j < t.len() && #[trigger] t[j] == x;
                if j == i - 1 { assert(x == t[i - 1]); }
                else { assert(i <= j < t.len() && t[j] == x); }
            }
        }
        if sel(t, fr, kp, i)(x) {
            if kept_lt(t, fr, kp, i, x) {
                let j = choose|j: int| 0 <= j < i && #[trigger] t[j] == x && kp[j] && !fr[j];
                assert(j != i - 1);
                assert(0 <= j < i - 1 && t[j] == x && kp[j] && !fr[j]);
            } else {
                let j = choose|j: int| i <= j < t.len() && #[trigger] t[j] == x;
                assert(i - 1 <= j < t.len() && t[j] == x);
            }
        }
        if x == t[i - 1] {
            assert(i - 1 <= i - 1 < t.len() && t[i - 1] == x);
        }
    }
}

pub proof fn lemma_suffix_split(t: Seq<u64>, i: int)
    requires 0 < i <= t.len(),
    ensures t.subrange(i - 1, t.len() as int) =~= seq![t[i - 1]] + t.subrange(i, t.len() as int),
{
}

/// 중복이 없으면 `index_of_first`는 바로 그 자리를 돌려준다.
pub proof fn lemma_index_of_first_unique(d: Seq<u64>, k: int)
    requires seq_nodup(d), 0 <= k < d.len(),
    ensures d.index_of_first(d[k]) == Some(k), pos(d, d[k]) == k, d.contains(d[k]),
{
    lemma_nodup_unwrap(d);
    assert(d.contains(d[k]));
    d.index_of_first_ensures(d[k]);
    let idx = d.index_of_first(d[k]).unwrap();
    assert(d[idx] == d[k]);
    assert(idx == k);
}

/// 끝에 끼우는 것은 덧붙이는 것 — 꼬리(`i == n`) 분기를 삽입 분기와 같은
/// 모양으로 다루기 위한 다리.
pub proof fn lemma_insert_at_end(d: Seq<u64>, e: u64)
    ensures d.insert(d.len() as int, e) =~= d.push(e),
{ d.insert_ensures(d.len() as int, e); }

/// 인접성 판정이 참이면 `e`는 분할점 **바로 앞**에 있다.
pub proof fn lemma_adjacent_at_split(d: Seq<u64>, sp: int, e: u64, a: u64)
    requires seq_nodup(d), 0 <= sp < d.len(), d[sp] == a, adjacent(d, e, a),
    ensures sp >= 1, d[sp - 1] == e,
{
    lemma_nodup_unwrap(d);
    let p = choose|p: int| 0 <= p < d.len() - 1 && #[trigger] d[p] == e && d[p + 1] == a;
    assert(d[p + 1] == d[sp]);
    assert(p + 1 == sp);
}

/// **`keptseq` 한 걸음 (kept 원소) — 이 모듈에서 P가 소비되는 유일한 지점**.
///
/// `e = target[i-1]`이 kept이면 `keptseq(i)`의 **마지막 원소**다. 왜냐하면
/// 아직 남은 kept 원소들(인덱스 < i-1)은 P에 의해 `current` 안에서 `e`보다
/// 앞에 있기 때문이다. 이 한 줄이 "P는 `current`의 위치를 말하고 불변식은
/// DOM의 위치를 말한다"는 간극을 메운다 — `keptseq`를 `current` 위에
/// 정의했으므로 P를 그대로 쓸 수 있다.
pub proof fn lemma_keptseq_push(
    c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int,
)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        kp[i - 1], !fr[i - 1],
        seq_nodup(c), seq_nodup(t),
        c.contains(t[i - 1]),
        kept_order_preserved(c, t, fr, kp),
    ensures
        keptseq(c, t, fr, kp, i) =~= keptseq(c, t, fr, kp, i - 1).push(t[i - 1]),
{
    reveal(kept_order_preserved);
    lemma_nodup_unwrap(c);
    lemma_nodup_unwrap(t);
    let e = t[i - 1];
    let ki = |x: u64| kept_lt(t, fr, kp, i, x);
    let km = |x: u64| kept_lt(t, fr, kp, i - 1, x);

    c.index_of_first_ensures(e);
    let pe = c.index_of_first(e).unwrap();
    assert(c[pe] == e);
    let c1 = c.subrange(0, pe);
    let c2 = c.subrange(pe + 1, c.len() as int);
    assert(c =~= c1 + seq![e] + c2);
    lemma_index_of_first_unique(c, pe);
    assert(pos(c, e) == pe);

    // (1) `e` 뒤에는 kept 원소가 없다 — **P가 주는 것**.
    assert forall|m: int| 0 <= m < c2.len() implies !ki(#[trigger] c2[m]) by {
        assert(c2[m] == c[pe + 1 + m]);
        if kept_lt(t, fr, kp, i, c2[m]) {
            let j = choose|j: int| 0 <= j < i && #[trigger] t[j] == c2[m] && kp[j] && !fr[j];
            lemma_index_of_first_unique(c, pe + 1 + m);
            assert(pos(c, c2[m]) == pe + 1 + m);
            assert(c2[m] != e);
            assert(j != i - 1);
            assert(0 <= j < i - 1 < t.len() && kp[j] && !fr[j] && kp[i - 1] && !fr[i - 1]);
            assert(pos(c, t[j]) < pos(c, t[i - 1]));
        }
    }
    assert forall|m: int| 0 <= m < c2.len() implies !km(#[trigger] c2[m]) by {
        assert(!ki(c2[m]));
        if kept_lt(t, fr, kp, i - 1, c2[m]) {
            let j = choose|j: int| 0 <= j < i - 1 && #[trigger] t[j] == c2[m] && kp[j] && !fr[j];
            assert(0 <= j < i && t[j] == c2[m] && kp[j] && !fr[j]);
        }
    }
    lemma_filter_none(c2, ki);
    lemma_filter_none(c2, km);

    // (2) `e` 앞에서는 두 술어가 일치한다(차이는 `j == i-1`뿐이고 그건 `e`다).
    assert forall|m: int| 0 <= m < c1.len() implies ki(#[trigger] c1[m]) == km(c1[m]) by {
        assert(c1[m] == c[m]);
        lemma_index_of_first_unique(c, m);
        assert(c1[m] != e);
        if kept_lt(t, fr, kp, i, c1[m]) {
            let j = choose|j: int| 0 <= j < i && #[trigger] t[j] == c1[m] && kp[j] && !fr[j];
            assert(j != i - 1);
            assert(0 <= j < i - 1 && t[j] == c1[m] && kp[j] && !fr[j]);
        }
        if kept_lt(t, fr, kp, i - 1, c1[m]) {
            let j = choose|j: int| 0 <= j < i - 1 && #[trigger] t[j] == c1[m] && kp[j] && !fr[j];
            assert(0 <= j < i && t[j] == c1[m] && kp[j] && !fr[j]);
        }
    }
    lemma_filter_agree(c1, ki, km);

    assert(ki(e)) by { assert(0 <= i - 1 < i && t[i - 1] == e && kp[i - 1] && !fr[i - 1]); }
    assert(!km(e)) by {
        if kept_lt(t, fr, kp, i - 1, e) {
            let j = choose|j: int| 0 <= j < i - 1 && #[trigger] t[j] == e && kp[j] && !fr[j];
            assert(t[j] == t[i - 1]);
            assert(j == i - 1);
        }
    }
    lemma_filter_singleton(e, ki);
    lemma_filter_singleton(e, km);
    assert(keptseq(c, t, fr, kp, i - 1) + seq![e] =~= keptseq(c, t, fr, kp, i - 1).push(e));
}

/// kept가 아닌 원소를 처리하면 `keptseq`는 변하지 않는다.
pub proof fn lemma_keptseq_same(
    c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int,
)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        !(kp[i - 1] && !fr[i - 1]),
    ensures keptseq(c, t, fr, kp, i - 1) =~= keptseq(c, t, fr, kp, i),
{
    let ki = |x: u64| kept_lt(t, fr, kp, i, x);
    let km = |x: u64| kept_lt(t, fr, kp, i - 1, x);
    assert forall|m: int| 0 <= m < c.len() implies km(#[trigger] c[m]) == ki(c[m]) by {
        if kept_lt(t, fr, kp, i, c[m]) {
            let j = choose|j: int| 0 <= j < i && #[trigger] t[j] == c[m] && kp[j] && !fr[j];
            assert(j != i - 1);
            assert(0 <= j < i - 1 && t[j] == c[m] && kp[j] && !fr[j]);
        }
        if kept_lt(t, fr, kp, i - 1, c[m]) {
            let j = choose|j: int| 0 <= j < i - 1 && #[trigger] t[j] == c[m] && kp[j] && !fr[j];
            assert(0 <= j < i && t[j] == c[m] && kp[j] && !fr[j]);
        }
    }
    lemma_filter_agree(c, km, ki);
}

/// **분기 ① — keep 스킵**. DOM이 그대로이고 술어 집합도 그대로이므로,
/// 새 분할점은 "`keptseq(i)`의 마지막 원소 `e`가 DOM에서 있는 자리"다.
/// 그 자리를 찾아 주는 것이 `lemma_filter_split_last`이고, `e`가 마지막
/// 원소라는 사실을 주는 것이 P(`lemma_keptseq_push`)다.
pub proof fn lemma_step_keep_skip(
    d: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, sp: int,
) -> (spn: int)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        kp[i - 1], !fr[i - 1],
        seq_nodup(c), seq_nodup(t),
        c.contains(t[i - 1]),
        kept_order_preserved(c, t, fr, kp),
        split_inv(d, c, t, fr, kp, i, sp),
    ensures split_inv(d, c, t, fr, kp, i - 1, spn),
{
    reveal(split_inv);
    let e = t[i - 1];
    let s_new = sel(t, fr, kp, i - 1);
    let s_old = sel(t, fr, kp, i);
    lemma_sel_same_kept(t, fr, kp, i);
    lemma_keptseq_push(c, t, fr, kp, i);

    let pre = d.subrange(0, sp);
    let q = lemma_filter_split_last(pre, s_old, keptseq(c, t, fr, kp, i - 1), e);
    assert(pre[q] == d[q]);

    assert(d.subrange(0, q) =~= pre.subrange(0, q));
    lemma_filter_agree(d.subrange(0, q), s_new, s_old);

    let gap = d.subrange(q + 1, sp);
    assert(gap =~= pre.subrange(q + 1, pre.len() as int));
    assert(d.subrange(q, d.len() as int)
           =~= seq![e] + gap + d.subrange(sp, d.len() as int));
    lemma_filter_agree(d.subrange(q, d.len() as int), s_new, s_old);
    assert(s_old(e)) by { assert(0 <= i - 1 < i && t[i - 1] == e && kp[i - 1] && !fr[i - 1]); }
    lemma_filter_singleton(e, s_old);
    lemma_suffix_split(t, i);
    q
}

/// **분기 ② — 인접 스킵**. DOM은 그대로이고 `e`가 분할점 바로 앞이므로
/// 새 분할점은 `sp - 1`이다.
pub proof fn lemma_step_adj_skip(
    d: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, sp: int,
) -> (spn: int)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        !(kp[i - 1] && !fr[i - 1]),
        seq_nodup(t), seq_nodup(d),
        split_inv(d, c, t, fr, kp, i, sp),
        sp >= 1, d[sp - 1] == t[i - 1],
    ensures split_inv(d, c, t, fr, kp, i - 1, spn), spn == sp - 1,
{
    reveal(split_inv);
    lemma_nodup_unwrap(d);
    let e = t[i - 1];
    let s_new = sel(t, fr, kp, i - 1);
    let s_old = sel(t, fr, kp, i);
    lemma_sel_widen_notkept(t, fr, kp, i);
    lemma_keptseq_same(c, t, fr, kp, i);

    assert forall|m: int| 0 <= m < sp - 1 implies s_new(#[trigger] d.subrange(0, sp - 1)[m])
        == s_old(d.subrange(0, sp - 1)[m]) by {
        assert(d.subrange(0, sp - 1)[m] == d[m]);
        assert(d[m] != d[sp - 1]);
    }
    lemma_filter_agree(d.subrange(0, sp - 1), s_new, s_old);
    assert(d.subrange(0, sp) =~= d.subrange(0, sp - 1) + seq![e]);
    lemma_filter_singleton(e, s_old);

    assert(d.subrange(sp - 1, d.len() as int) =~= seq![e] + d.subrange(sp, d.len() as int));
    assert forall|m: int| 0 <= m < d.subrange(sp, d.len() as int).len()
        implies s_new(#[trigger] d.subrange(sp, d.len() as int)[m])
            == s_old(d.subrange(sp, d.len() as int)[m]) by {
        assert(d.subrange(sp, d.len() as int)[m] == d[sp + m]);
        assert(d[sp + m] != d[sp - 1]);
    }
    lemma_filter_agree(d.subrange(sp, d.len() as int), s_new, s_old);
    lemma_filter_singleton(e, s_new);
    assert(s_new(e));
    lemma_suffix_split(t, i);
    sp - 1
}

/// **이동 분기 전반 — `unlink` 뒤에도 분할이 살아 있다**. `e`는 술어 집합에
/// 걸리지 않으므로 양쪽 filter 값은 그대로이고, 분할점만 `e`가 앞쪽에
/// 있었는지에 따라 한 칸 당겨진다.
pub proof fn lemma_split_after_remove(
    d: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, sp: int, e: u64,
) -> (sp1: int)
    requires
        0 <= i <= t.len(),
        seq_nodup(d),
        split_inv(d, c, t, fr, kp, i, sp),
        !sel(t, fr, kp, i)(e),
        i < t.len() ==> t[i] != e,
    ensures split_inv(d.remove_value(e), c, t, fr, kp, i, sp1),
{
    reveal(split_inv);
    lemma_nodup_unwrap(d);
    let s_old = sel(t, fr, kp, i);
    let d1 = d.remove_value(e);
    d.index_of_first_ensures(e);
    if !d.contains(e) {
        assert(d1 =~= d);
        sp
    } else {
        let pe = d.index_of_first(e).unwrap();
        assert(d[pe] == e);
        assert(d1 =~= d.remove(pe));
        d.remove_ensures(pe);
        assert(pe != sp);
        if pe < sp {
            let sp1 = sp - 1;
            assert(d.subrange(0, sp).remove(pe) =~= d1.subrange(0, sp1)) by {
                assert forall|m: int| 0 <= m < sp1 implies
                    d.subrange(0, sp).remove(pe)[m] == d1.subrange(0, sp1)[m] by {
                    if m < pe { assert(d.subrange(0, sp).remove(pe)[m] == d.subrange(0, sp)[m]); }
                    else { assert(d.subrange(0, sp).remove(pe)[m] == d.subrange(0, sp)[m + 1]); }
                }
            }
            assert(d.subrange(0, sp)[pe] == e);
            lemma_filter_remove_nonmatching(d.subrange(0, sp), pe, s_old);
            assert(d1.subrange(sp1, d1.len() as int) =~= d.subrange(sp, d.len() as int)) by {
                assert forall|m: int| 0 <= m < d.len() - sp implies
                    d1.subrange(sp1, d1.len() as int)[m] == d.subrange(sp, d.len() as int)[m] by {
                    assert(d1[sp1 + m] == d[sp + m]);
                }
            }
            sp1
        } else {
            let sp1 = sp;
            assert(d1.subrange(0, sp1) =~= d.subrange(0, sp)) by {
                assert forall|m: int| 0 <= m < sp implies d1[m] == d[m] by { }
            }
            let tail = d.subrange(sp, d.len() as int);
            assert(tail[pe - sp] == e);
            assert(tail.remove(pe - sp) =~= d1.subrange(sp1, d1.len() as int)) by {
                assert forall|m: int| 0 <= m < tail.len() - 1 implies
                    tail.remove(pe - sp)[m] == d1.subrange(sp1, d1.len() as int)[m] by {
                    if m < pe - sp { assert(tail.remove(pe - sp)[m] == tail[m]); }
                    else { assert(tail.remove(pe - sp)[m] == tail[m + 1]); }
                }
            }
            lemma_filter_remove_nonmatching(tail, pe - sp, s_old);
            sp1
        }
    }
}

/// **분기 ③ — 삽입**. `e`를 분할점에 끼우면 그 자리가 곧 새 분할점이다.
/// 꼬리(`i == n`)의 `push_back`도 `lemma_insert_at_end`를 거쳐 같은 모양이
/// 되므로 분기가 하나로 합쳐진다.
pub proof fn lemma_step_insert(
    d1: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, i: int, sp1: int, e: u64,
)
    requires
        0 < i <= t.len(), fr.len() == t.len(), kp.len() == t.len(),
        !(kp[i - 1] && !fr[i - 1]), e == t[i - 1],
        seq_nodup(t), !d1.contains(e),
        split_inv(d1, c, t, fr, kp, i, sp1),
    ensures split_inv(d1.insert(sp1, e), c, t, fr, kp, i - 1, sp1),
{
    reveal(split_inv);
    let s_new = sel(t, fr, kp, i - 1);
    let s_old = sel(t, fr, kp, i);
    lemma_sel_widen_notkept(t, fr, kp, i);
    lemma_keptseq_same(c, t, fr, kp, i);

    let a1 = d1.subrange(0, sp1);
    let b1 = d1.subrange(sp1, d1.len() as int);
    let d2 = d1.insert(sp1, e);
    d1.insert_ensures(sp1, e);
    assert(d2 =~= a1 + seq![e] + b1);
    assert(d2.subrange(0, sp1) =~= a1);
    assert(d2.subrange(sp1, d2.len() as int) =~= seq![e] + b1);
    assert(d2[sp1] == e);

    assert forall|m: int| 0 <= m < a1.len() implies s_new(#[trigger] a1[m]) == s_old(a1[m]) by {
        assert(a1[m] == d1[m]);
        assert(d1.contains(d1[m]));
    }
    lemma_filter_agree(a1, s_new, s_old);
    assert forall|m: int| 0 <= m < b1.len() implies s_new(#[trigger] b1[m]) == s_old(b1[m]) by {
        assert(b1[m] == d1[sp1 + m]);
        assert(d1.contains(d1[sp1 + m]));
    }
    lemma_filter_agree(b1, s_new, s_old);
    assert(s_new(e));
    lemma_filter_singleton(e, s_new);
    lemma_suffix_split(t, i);
}

/// **끝맺음** — `i == 0`의 불변식 + 사전조건 Q ⟹ DOM이 곧 target.
///
/// Q(`current ⊆ target`)가 없으면 이 정리는 **거짓**이다. 최소 반례:
/// `current = [0, 1]`, `target = [0]`, `keep = [true]` — P는 공허하게 참,
/// 기존 `requires`도 전부 참인데 `keep[0]`이 인접성 검사를 단락하므로 op가
/// 하나도 나오지 않아 DOM은 `[0, 1]`로 남는다. `target`에 없는 원소(extras)가
/// `current`에 있으면 LIS 경로는 그것을 치우지 않는다.
pub proof fn lemma_endgame(
    d: Seq<u64>, c: Seq<u64>, t: Seq<u64>, fr: Seq<bool>, kp: Seq<bool>, sp: int,
)
    requires
        split_inv(d, c, t, fr, kp, 0, sp),
        forall|j: int| 0 <= j < d.len() ==> #[trigger] t.contains(d[j]),
    ensures d =~= t,
{
    reveal(split_inv);
    let s0 = sel(t, fr, kp, 0);
    let k0 = |x: u64| kept_lt(t, fr, kp, 0, x);
    assert forall|m: int| 0 <= m < c.len() implies !k0(#[trigger] c[m]) by { }
    lemma_filter_none(c, k0);
    assert(t.subrange(0, t.len() as int) =~= t);
    assert(d =~= d.subrange(0, sp) + d.subrange(sp, d.len() as int));
    assert(d.filter(s0) =~= t);
    assert forall|j: int| 0 <= j < d.len() implies s0(#[trigger] d[j]) by {
        assert(t.contains(d[j]));
        let m = choose|m: int| 0 <= m < t.len() && t[m] == d[j];
        assert(0 <= m < t.len() && t[m] == d[j]);
    }
    lemma_filter_all(d, s0);
}

// **증인 교체(2026-08-09)**: 처음 판본의 증인은 `current == target == [7,9]`라
// 실행이 퇴화했다(방출 op 0개 — 아무것도 시험하지 않는다). 아래 증인은 P를
// 실제로 소비하고(표시 둘), Fresh를 포함하며, `current != target`이다.
/// **음성 대조 — 새 사전조건 Q를 얹은 뒤에도 `plan_writes`의 계약이 공허하지
/// 않다**.
///
/// `requires`가 서로 모순이면 `ensures`가 무엇이든 통과한다. Q는 계약을
/// **좁히는** 조항이므로, 좁힌 뒤에도 실제로 만족 가능한 입력이 남아 있음을
/// 못박지 않으면 "수렴을 증명했다"는 말이 의미를 갖지 못한다. 아래는 그
/// 증인이고, `plan_writes`의 사전조건 **일곱 개 전부**를 한 인스턴스에 대해
/// 동시에 세운다 — 즉 이 정리가 통과하면 사전조건 집합은 만족 가능하다.
///
/// 증인은 일부러 자명하지 않게 골랐다: `current != target`(신규 원소가
/// 하나 있다), `keep`이 실제로 두 개 켜져 있어 P가 공허하지 않다.
pub proof fn lemma_plan_writes_requires_satisfiable()
    ensures
        ({
            let cur = seq![7u64, 9u64];
            let t = seq![5u64, 7u64, 9u64];
            let fr = seq![true, false, false];
            let kp = seq![false, true, true];
            &&& cur.no_duplicates()
            &&& t.no_duplicates()
            &&& fr.len() == t.len()
            &&& kp.len() == t.len()
            &&& forall|i: int| #![trigger fr[i]]
                    0 <= i < t.len() ==>
                        (fr[i] ==> !cur.contains(t[i])) && (!fr[i] ==> cur.contains(t[i]))
            &&& kept_order_preserved(cur, t, fr, kp)
            &&& forall|x: u64| #![trigger cur.contains(x)] cur.contains(x) ==> t.contains(x)
            // 그리고 P는 여기서 공허하지 않다 — 실제로 비교되는 쌍이 있다.
            &&& kp[1] && !fr[1] && kp[2] && !fr[2]
        }),
{
    let cur = seq![7u64, 9u64];
    let t = seq![5u64, 7u64, 9u64];
    assert(cur[0] == 7 && cur[1] == 9);
    assert(t[0] == 5 && t[1] == 7 && t[2] == 9);
    assert(cur.contains(7u64)) by { assert(cur[0] == 7u64); }
    assert(cur.contains(9u64)) by { assert(cur[1] == 9u64); }
    assert(t.contains(7u64)) by { assert(t[1] == 7u64); }
    assert(t.contains(9u64)) by { assert(t[2] == 9u64); }
    assert(!cur.contains(5u64)) by {
        if cur.contains(5u64) {
            let m = choose|m: int| 0 <= m < cur.len() && cur[m] == 5u64;
            assert(cur[m] == 5u64);
        }
    }
    assert forall|x: u64| #![trigger cur.contains(x)] cur.contains(x) implies t.contains(x) by {
        let m = choose|m: int| 0 <= m < cur.len() && cur[m] == x;
        assert(cur[m] == x);
    }
    cur.index_of_first_ensures(7u64);
    cur.index_of_first_ensures(9u64);
    assert(pos(cur, 7u64) == 0);
    assert(pos(cur, 9u64) == 1);
    reveal(kept_order_preserved);
}
/// **음성 대조 — 새 사전조건 Q가 실제로 무언가를 거절한다** (2026-08-09).
///
/// `requires`는 약할수록 통과하기 쉬우므로, 항상 참인 조항을 얹는 것은 아무것도
/// 하지 않는 것과 같다. Q가 진짜 조항임을, 그리고 **Q 없이는 목표 사후조건이
/// 실제로 거짓임**을 여기서 기계검사한다. 정찰이 찾아낸 최소 반례:
///
/// ```text
/// current = [0, 1]   target_ids = [0]   target_fresh = [false]   keep = [true]
/// ```
///
/// P는 표시된 원소가 하나뿐이라 공허하게 참이고 기존 `requires`도 전부
/// 성립하는데, `keep[0]`이 인접성 검사를 단락하므로 op가 하나도 나오지 않는다.
/// 그러면 DOM은 `[0, 1]` 그대로이고 뒤쪽 1개는 `[1] != [0]`이다.
pub proof fn lemma_q_rejects_extras()
    ensures
        // (1) Q는 이 `current`/`target` 짝을 거절한다 — 증인 `1`을 직접 낸다
        //     (`x = 1`이 Q의 함의를 깨므로 Q 전체가 거짓이다).
        seq![0u64, 1u64].contains(1u64) && !seq![0u64].contains(1u64),
        // (2) 그리고 그 입력에서 op를 하나도 내지 않으면(= `keep` 단락이 하는
        //     일이 정확히 그것이다) 목표 사후조건이 **거짓**이 된다.
        !(apply_ops(seq![0u64, 1u64], Seq::<PlanOp>::empty(), 0).subrange(1, 2)
            =~= seq![0u64]),
{
    let c = seq![0u64, 1u64];
    assert(c[1] == 1u64);
    assert(c.contains(1u64));
    assert(!seq![0u64].contains(1u64)) by {
        assert forall|j: int| 0 <= j < seq![0u64].len() implies seq![0u64][j] != 1u64 by {
            assert(seq![0u64][0] == 0u64);
        }
    }
    assert(apply_ops(c, Seq::<PlanOp>::empty(), 0) =~= c);
    assert(c.subrange(1, 2)[0] == 1u64);
    assert(seq![0u64][0] == 0u64);
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
        // **사전조건 P** — `keep[i] || <인접성 검사>`의 **단락**이 정당하려면
        // 표시된 원소들의 `current` 내 상대 순서가 target 순서와 같아야 한다.
        // 호출부(`list.rs`의 `place_in_order_lis`)가 `lis_indices`로 이것을
        // 세운다. 지금까지 디버그 빌드 `debug_assert!`로만 있던 것을 여기
        // **기계가 읽는 계약**으로 올렸다(2026-08-08).
        // (2026-10-17 정정: 그 호출부는 2026-08-11 사라졌다 — 지금 P는 이
        // 크레이트의 `lis_keep` 사후조건으로 증명되고, `plan_lis_from_selection`
        // 이 같은 검증 단위 안에서 이 `requires`에 물린다.)
        kept_order_preserved(current@, target_ids@, target_fresh@, keep@),
        // **사전조건 Q** — `current`에 target 밖 원소(extras)가 없다 (2026-08-09 신설).
        //
        // 기존 조항의 **약화가 아니라 새 조항의 추가**다. 그리고 이것 없이는
        // 아래 수렴 사후조건이 **거짓**이다 — `lemma_endgame`의 주석에 최소
        // 반례를 적어 뒀다. 비-LIS 경로(`plan_writes_converging`)는 이 조항이
        // 필요 없다: 거기서는 비인접이면 무조건 옮기므로 외래 원소가 저절로
        // 앞으로 밀려난다. **두 경로의 진짜 비대칭**이고, LIS의 `keep` 단락이
        // 사 오는 비용이다.
        //
        // 호출부(`crates/loom-runtime/src/list.rs`의 `d_lis_body`)는 이미 항상
        // 만족한다 — `match_keys`가 old를 retained/removed로 가르고
        // `current = old_order − removed`를 만들므로 `current`는 정확히
        // retained 집합이다. 지금까지 계약에 적혀 있지 않았을 뿐이다.
        forall|x: u64| #![trigger current@.contains(x)]
            current@.contains(x) ==> target_ids@.contains(x),
    ensures
        // 방출된 op의 인덱스는 전부 유효하고, spawn 여부는 target과 일치한다.
        forall|k: int| #![trigger ops@[k]]
            0 <= k < ops@.len() ==>
                ops@[k].idx < target_ids@.len()
                && ops@[k].spawn == target_fresh@[ops@[k].idx as int],
        // **수렴(RQ5 #4 정리 (2)) — LIS 경로**. `plan_writes_converging`이
        // 갖고 있는 것과 **글자 그대로 같은 모양**이다: 방출된 op를 순서대로
        // 적용하면 DOM의 뒤쪽 |target|개가 목표 순서와 정확히 일치한다.
        ({
            let d = apply_ops(current@, ops@, ops@.len() as int);
            &&& d.len() >= target_ids@.len()
            &&& d.subrange(d.len() - target_ids@.len(), d.len() as int) =~= target_ids@
        }),
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
    // **분할점** — DOM 안에서 `target[i]`가 있는 자리(`i == n`이면 끝).
    // 부분수열 불변식 `split_inv`가 이 자리를 기준으로 앞/뒤를 나눈다.
    let ghost mut sp: int = current@.len() as int;
    proof { lemma_split_inv_init(current@, target_ids@, target_fresh@, keep@); }

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
            // 사전조건 P·Q는 루프 안에서 자동으로 살아 있지 않다(Verus의
            // `while`은 불변식만으로 모듈러 검증된다) — 다시 적는다. 둘 다
            // 명제 하나 또는 1변수 양화자라 부담이 없다.
            kept_order_preserved(current@, target_ids@, target_fresh@, keep@),
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
            // (e) chain 표기 = 방출한 op를 순서대로 적용한 결과. 사후조건이
            // `apply_ops`로 말하므로 이 배선이 있어야 한다.
            chain.denotes() =~= apply_ops(current@, ops@, ops@.len() as int),
            // (f) **부분수열 불변식** — 접미사 불변식이 LIS 경로에서 깨지는
            // 자리를 대신한다. 불투명이라 루프는 명제만 들고 다닌다.
            split_inv(chain.denotes(), current@, target_ids@, target_fresh@, keep@, i as int, sp),
        decreases i,
    {
        // `i`는 루프 **끝에서만** 줄인다(`plan_writes_converging`과 같은 형태).
        // 새로 들어온 `split_inv`가 `i`와 `i-1`을 함께 언급하므로, 앞에서
        // 줄이면 불변식의 인덱스와 본문의 인덱스를 프루버가 잇지 못한다.
        let idx = i - 1;
        let e = target_ids[idx];
        let fresh = target_fresh[idx];
        let ghost prev_denotes = chain.denotes();
        let ghost prev_ops = ops@;
        let ghost sp_old = sp;

        proof {
            chain.no_dup_opaque();
            // 불변식에서 **분할점의 위치 정보만** 꺼낸다 — filter 항은 보지 않는다.
            lemma_split_inv_anchor(prev_denotes, current@, target_ids@, target_fresh@,
                                   keep@, i as int, sp_old);
            // e는 target의 idx번째이고 중복이 없으므로 [idx+1, n) 구간에 없다.
            assert(!target_ids@.subrange(idx as int + 1, n as int).contains(e)) by {
                reveal(seq_nodup);
                let sub = target_ids@.subrange(idx as int + 1, n as int);
                assert forall|j: int| #![trigger sub[j]]
                    0 <= j < sub.len() implies sub[j] != e by {
                    assert(sub[j] == target_ids@[idx as int + 1 + j]);
                }
            }
        }

        if fresh {
            // Fresh: chain에 없음이 (c)로부터 따라온다 → insert 사전조건 충족.
            proof {
                lemma_sel_not_e(target_ids@, target_fresh@, keep@, i as int);
                lemma_remove_value_absent(prev_denotes, e);
            }
            match next_anchor {
                Some(a) => {
                    proof {
                        // anchor는 `target[i]`이고 분할점이 바로 그 자리다 —
                        // 중복이 없으므로 `index_of_first`가 `sp_old`를 준다.
                        assert(a == target_ids@[i as int]);
                        assert(prev_denotes[sp_old] == a);
                        lemma_index_of_first_unique(prev_denotes, sp_old);
                        lemma_insert_has_new(prev_denotes, sp_old, e);
                    }
                    chain.insert_before(e, a);
                    proof {
                        assert(chain.denotes() =~= prev_denotes.insert(sp_old, e));
                        lemma_step_insert(prev_denotes, current@, target_ids@, target_fresh@,
                                          keep@, i as int, sp_old, e);
                        lemma_insert_same_plus(prev_denotes, sp_old, e);
                        assert(chain.denotes() =~= place_before(prev_denotes, e, Some(a)));
                    }
                }
                None => {
                    proof { assert(sp_old == prev_denotes.len()); }
                    chain.push_back(e);
                    proof {
                        // 꼬리 덧붙이기도 "분할점에 끼우기"의 특수 경우다.
                        lemma_insert_at_end(prev_denotes, e);
                        assert(chain.denotes() =~= prev_denotes.insert(sp_old, e));
                        lemma_step_insert(prev_denotes, current@, target_ids@, target_fresh@,
                                          keep@, i as int, sp_old, e);
                        lemma_push_same_plus(prev_denotes, e);
                        assert(chain.denotes() =~= place_before(prev_denotes, e, None));
                    }
                }
            }
            assert(same_plus(chain.denotes(), prev_denotes, e));
            ops.push(PlanOp { idx, id: e, spawn: true, before: next_anchor });
            proof {
                lemma_step_keeps_link(current@, prev_ops, ops@, prev_denotes, chain.denotes(),
                    PlanOp { idx, id: e, spawn: true, before: next_anchor });
            }
        } else {
            // `keep[i]`는 호출부(LIS 변형)가 "이 원소는 이미 상대 순서가
            // 맞으니 건드리지 말라"고 표시한 것. 스킵은 write를 내지 않고
            // chain도 그대로 두므로 chain은 계속 DOM과 일치한다 — 다만
            // **LIS 판단 자체의 정당성은 여기서 검증하지 않는다**(호출부 책임).
            let already_adjacent = keep[idx] || match next_anchor {
                Some(a) => chain.next_is(e, a),
                None => chain.is_tail(e),
            };
            if !already_adjacent {
                // Retained: (b)로부터 chain에 있음 → unlink 사전조건 충족.
                proof {
                    assert(!keep@[idx as int]);
                    lemma_sel_not_e(target_ids@, target_fresh@, keep@, i as int);
                    if i < n { assert(target_ids@[i as int] != e) by { reveal(seq_nodup); } }
                }
                chain.unlink(e);
                proof {
                    // 중복이 없었으므로 unlink로 e는 완전히 사라졌다 —
                    // 이어지는 삽입의 사전조건(!contains(e))이 여기서 나온다.
                    lemma_remove_value_no_dup(prev_denotes, e);
                    assert(!chain.denotes().contains(e));
                    // anchor는 e와 다른 원소이므로 unlink에도 살아남는다.
                    if let Some(a) = next_anchor {
                        // anchor는 target[i], e는 target[i-1] — 자리가 다르므로
                        // 중복 없음에서 서로 다르다. 양화자는 이 `by` 블록
                        // 안에서만 펼친다(불투명화의 의미를 지키기 위해).
                        assert(a != e) by { reveal(seq_nodup); }
                        lemma_remove_value_keeps_other(prev_denotes, e, a);
                    }
                    // **분할점을 옮긴다** — `e`는 술어 집합에 걸리지 않으므로
                    // 양쪽 filter 값은 그대로이고, `e`가 앞쪽에 있었으면
                    // 분할점만 한 칸 당겨진다.
                    sp = lemma_split_after_remove(prev_denotes, current@, target_ids@,
                             target_fresh@, keep@, i as int, sp_old, e);
                    lemma_split_inv_anchor(chain.denotes(), current@, target_ids@,
                                           target_fresh@, keep@, i as int, sp);
                }
                let ghost mid = chain.denotes();
                match next_anchor {
                    Some(a) => {
                        proof {
                            assert(mid[sp] == a);
                            lemma_index_of_first_unique(mid, sp);
                            lemma_insert_has_new(mid, sp, e);
                        }
                        chain.insert_before(e, a);
                        proof {
                            assert(chain.denotes() =~= mid.insert(sp, e));
                            lemma_step_insert(mid, current@, target_ids@, target_fresh@,
                                              keep@, i as int, sp, e);
                            lemma_insert_same_plus(mid, sp, e);
                            lemma_remove_then_place_same_plus(prev_denotes, mid,
                                                              chain.denotes(), e);
                            assert(chain.denotes() =~= place_before(prev_denotes, e, Some(a)));
                        }
                    }
                    None => {
                        proof { assert(sp == mid.len()); }
                        chain.push_back(e);
                        proof {
                            lemma_insert_at_end(mid, e);
                            assert(chain.denotes() =~= mid.insert(sp, e));
                            lemma_step_insert(mid, current@, target_ids@, target_fresh@,
                                              keep@, i as int, sp, e);
                            lemma_push_same_plus(mid, e);
                            lemma_remove_then_place_same_plus(prev_denotes, mid,
                                                              chain.denotes(), e);
                            assert(chain.denotes() =~= place_before(prev_denotes, e, None));
                        }
                    }
                }
                assert(same_plus(chain.denotes(), prev_denotes, e));
                ops.push(PlanOp { idx, id: e, spawn: false, before: next_anchor });
                proof {
                    lemma_step_keeps_link(current@, prev_ops, ops@, prev_denotes,
                        chain.denotes(),
                        PlanOp { idx, id: e, spawn: false, before: next_anchor });
                }
            } else {
                // 스킵: 표기 불변. e는 Retained이므로 이미 들어 있다.
                assert(current@.contains(e));
                assert(same_plus(chain.denotes(), prev_denotes, e));
                proof {
                    // 스킵은 **두 가지 이유**로 일어나고 분할점이 옮겨가는
                    // 방식이 서로 다르다.
                    if keep@[idx as int] {
                        // ① `keep` 단락 — 인접성은 성립하지 않을 수 있다.
                        //    `e`는 `keptseq(i)`의 **마지막** 원소이고(P가 주는
                        //    것), 그 자리가 새 분할점이다.
                        sp = lemma_step_keep_skip(prev_denotes, current@, target_ids@,
                                 target_fresh@, keep@, i as int, sp_old);
                    } else {
                        // ② 인접성 판정이 참이었다 — `e`가 분할점 바로 앞.
                        match next_anchor {
                            Some(a) => {
                                assert(adjacent(prev_denotes, e, a));
                                lemma_adjacent_at_split(prev_denotes, sp_old, e, a);
                            }
                            None => {
                                assert(prev_denotes[prev_denotes.len() - 1] == e);
                                assert(sp_old == prev_denotes.len());
                            }
                        }
                        sp = lemma_step_adj_skip(prev_denotes, current@, target_ids@,
                                 target_fresh@, keep@, i as int, sp_old);
                    }
                }
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
            let sub_new = target_ids@.subrange(idx as int, n as int);
            let sub_old = target_ids@.subrange(idx as int + 1, n as int);
            assert(sub_new[0] == e);
            assert(sub_new.contains(e));
            // 이전 구간은 새 구간의 부분이다 — 불변식 (c)를 한 칸 넓히는 단계.
            assert forall|x: u64| sub_old.contains(x) implies sub_new.contains(x) by {
                let k = choose|k: int| 0 <= k < sub_old.len() && sub_old[k] == x;
                assert(sub_old[k] == target_ids@[idx as int + 1 + k]);
                assert(sub_new[k + 1] == target_ids@[idx as int + 1 + k]);
            }
            assert forall|x: u64| #![trigger chain.denotes().contains(x)]
                chain.denotes().contains(x)
                implies current@.contains(x) || sub_new.contains(x) by {
                if chain.denotes().contains(x) && x != e {
                    assert(prev_denotes.contains(x));
                }
            }
            assert forall|j: int| #![trigger target_ids@[j]]
                idx <= j < n implies chain.denotes().contains(target_ids@[j]) by {
                if j > idx { assert(prev_denotes.contains(target_ids@[j])); }
            }
        }
        next_anchor = Some(e);
        i = idx;
    }
    proof {
        // **끝맺음**. `i == 0`이므로 (f)는 "DOM에서 target 원소만 뽑으면
        // target 그대로"를 말하고, Q는 "DOM에 target 밖 원소가 없다"를 준다.
        // 둘이 합쳐지면 filter가 항등이 되어 **DOM == target**이 나온다.
        let d = chain.denotes();
        assert(target_ids@.subrange(0, n as int) =~= target_ids@);
        assert forall|j: int| 0 <= j < d.len() implies
            target_ids@.contains(#[trigger] d[j]) by {
            assert(d.contains(d[j]));
        }
        lemma_endgame(d, current@, target_ids@, target_fresh@, keep@, sp);
        assert(d =~= target_ids@);
        assert(d.subrange(d.len() - n as int, d.len() as int) =~= target_ids@);
    }
    ops
}


// ============================================================================
// 경계 — 매칭 결과(인덱스 선택)에서 `plan_writes`의 입력을 **함께** 구성한다
// ============================================================================

/// `current`가 **옛 DOM 순서를 보존한다**는 사실. 사전조건 P를
/// `take_old`(인덱스 선택) 항으로 번역할 때 쓰는 재료다.
///
/// **불투명인 이유**: 본문이 `pos(..)` 항을 만든다. `pos`의 정의 주석이
/// 말하듯 그 항이 모듈 문맥에 그냥 뜨면 `kept_order_preserved`를 불투명으로
/// 가둔 격리가 무너진다 — 여기서도 같은 처방을 쓴다.
#[verifier::opaque]
pub open spec fn selection_order_preserved(
    current: Seq<u64>,
    old_ids: Seq<u64>,
    take_old: Seq<usize>,
    target_ids: Seq<u64>,
) -> bool {
    forall|a: int, b: int| #![trigger take_old[a], take_old[b]]
        0 <= a < target_ids.len() && 0 <= b < target_ids.len()
            && take_old[a] < old_ids.len() && take_old[b] < old_ids.len()
            && take_old[a] < take_old[b]
            ==> pos(current, target_ids[a]) < pos(current, target_ids[b])
}

/// `plan_writes`/`plan_writes_converging`에 넘길 입력 3종 + despawn 대상.
pub struct PlanInputs {
    pub current: Vec<u64>,
    pub removed: Vec<u64>,
    pub target_ids: Vec<u64>,
    pub target_fresh: Vec<bool>,
}

/// **경계 함수** — "새 슬롯 i가 옛 슬롯 몇 번을 가져가는가"(`take_old`)만 받아
/// `current`와 `target_ids`/`target_fresh`를 **함께** 만든다.
///
/// 요점은 이것이다: 두 벡터가 같은 선택에서 같이 나오므로 **사전조건 Q**
/// (`current`에 target 밖 원소가 없다)가 *구조적으로* 참이다 — 매칭이 옳든
/// 그르든 상관없다. 키 비교(String)는 호출부(비검증)에 남고, 이 함수는
/// 인덱스만 본다.
pub fn plan_inputs(
    old_ids: &Vec<u64>,
    take_old: &Vec<usize>,
    fresh_ids: &Vec<u64>,
) -> (r: PlanInputs)
    requires
        old_ids@.no_duplicates(),
        fresh_ids@.len() == take_old@.len(),
        forall|i: int| #![trigger take_old@[i]]
            0 <= i < take_old@.len() && take_old@[i] >= old_ids@.len()
                ==> !old_ids@.contains(fresh_ids@[i]),
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] < old_ids@.len() && take_old@[b] < old_ids@.len()
                ==> take_old@[a] != take_old@[b],
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] >= old_ids@.len() && take_old@[b] >= old_ids@.len()
                ==> fresh_ids@[a] != fresh_ids@[b],
    ensures
        r.target_ids@.len() == take_old@.len(),
        r.target_fresh@.len() == r.target_ids@.len(),
        forall|i: int| #![trigger r.target_fresh@[i]]
            0 <= i < take_old@.len() ==> r.target_fresh@[i] == (take_old@[i] >= old_ids@.len()),
        forall|i: int| #![trigger r.target_ids@[i]]
            0 <= i < take_old@.len() ==>
                r.target_ids@[i] == if take_old@[i] < old_ids@.len() {
                    old_ids@[take_old@[i] as int]
                } else {
                    fresh_ids@[i]
                },
        // **불투명 형태로 낸다**: raw `no_duplicates`는 2변수 양화자라 이
        // 사후조건이 모듈 문맥에 트리거를 더한다(이 모듈이 실제로 그것 때문에
        // 128초를 태운 적이 있다). 소비자(두 래퍼)가 `lemma_nodup_unwrap`으로
        // 자기 안에서만 편다.
        seq_nodup(r.current@),
        seq_nodup(r.target_ids@),
        forall|x: u64| #![trigger r.current@.contains(x)]
            r.current@.contains(x) ==> r.target_ids@.contains(x),
        forall|i: int| #![trigger r.target_fresh@[i]]
            0 <= i < r.target_ids@.len() ==>
                (r.target_fresh@[i] ==> !r.current@.contains(r.target_ids@[i]))
                && (!r.target_fresh@[i] ==> r.current@.contains(r.target_ids@[i])),
        forall|x: u64| #![trigger r.removed@.contains(x)]
            r.removed@.contains(x) ==> !r.current@.contains(x),
        selection_order_preserved(r.current@, old_ids@, take_old@, r.target_ids@),
{
    let m = old_ids.len();
    let n = take_old.len();
    proof { lemma_nodup_wrap(old_ids@); }

    // ---- 1. taker[j] = 옛 슬롯 j를 가져간 새 인덱스(없으면 n) ----
    let mut taker: Vec<usize> = Vec::with_capacity(m);
    let mut j: usize = 0;
    while j < m
        invariant
            j <= m,
            taker@.len() == j,
            forall|k: int| #![trigger taker@[k]] 0 <= k < j ==> taker@[k] == n,
        decreases m - j,
    {
        taker.push(n);
        j = j + 1;
    }

    let mut i: usize = 0;
    while i < n
        invariant
            i <= n,
            n == take_old@.len(),
            m == old_ids@.len(),
            taker@.len() == m,
            forall|k: int| #![trigger taker@[k]]
                0 <= k < m ==> taker@[k] <= n,
            forall|k: int| #![trigger taker@[k]]
                0 <= k < m && taker@[k] < n ==> take_old@[taker@[k] as int] == k,
            forall|t: int| #![trigger take_old@[t]]
                0 <= t < i && take_old@[t] < m ==> taker@[take_old@[t] as int] < n,
        decreases n - i,
    {
        let t = take_old[i];
        if t < m {
            taker.set(t, i);
        }
        i = i + 1;
    }

    // ---- 2. current / removed — 옛 DOM 순서 그대로 가른다 ----
    let mut current: Vec<u64> = Vec::new();
    let mut removed: Vec<u64> = Vec::new();
    // `rank[k]` = `current[k]`가 온 옛 인덱스(`rrank`는 removed 쪽),
    // `whr[q]` = 옛 슬롯 q가 `current`에서 놓인 자리(선택된 q에서만 유효).
    let ghost mut rank: Seq<int> = Seq::empty();
    let ghost mut rrank: Seq<int> = Seq::empty();
    let ghost mut whr: Seq<int> = Seq::empty();
    proof { lemma_nodup_wrap(current@); }
    let mut j: usize = 0;
    while j < m
        invariant
            j <= m,
            m == old_ids@.len(),
            taker@.len() == m,
            whr.len() == j,
            seq_nodup(old_ids@),
            rank.len() == current@.len(),
            rrank.len() == removed@.len(),
            forall|k: int| #![trigger rank[k]]
                0 <= k < rank.len() ==>
                    0 <= rank[k] < j && old_ids@[rank[k]] == current@[k]
                    && taker@[rank[k] as int] < n,
            forall|k: int| #![trigger rrank[k]]
                0 <= k < rrank.len() ==>
                    0 <= rrank[k] < j && old_ids@[rrank[k]] == removed@[k]
                    && taker@[rrank[k] as int] >= n,
            seq_nodup(current@),
            forall|q: int| #![trigger taker@[q]]
                0 <= q < j && taker@[q] < n ==>
                    current@.contains(old_ids@[q])
                    && 0 <= whr[q] < current@.len()
                    && current@[whr[q]] == old_ids@[q],
            forall|q1: int, q2: int| #![trigger whr[q1], whr[q2]]
                0 <= q1 < q2 < j && taker@[q1] < n && taker@[q2] < n ==> whr[q1] < whr[q2],
            forall|x: u64| #![trigger current@.contains(x)]
                current@.contains(x) ==> old_ids@.contains(x),
        decreases m - j,
    {
        let ghost old_current = current@;
        let ghost e = old_ids@[j as int];
        if taker[j] < n {
            proof {
                assert(!current@.contains(e)) by {
                    lemma_nodup_unwrap(old_ids@);
                    if current@.contains(e) {
                        let k = choose|k: int| 0 <= k < current@.len() && current@[k] == e;
                        assert(0 <= rank[k] < j);
                        assert(old_ids@[rank[k]] == old_ids@[j as int]);
                        assert(false);
                    }
                }
                assert(seq_nodup(current@.push(e))) by {
                    lemma_nodup_unwrap(current@);
                    lemma_insert_no_dup(current@, current@.len() as int, e);
                    assert(current@.insert(current@.len() as int, e) =~= current@.push(e));
                    lemma_nodup_wrap(current@.push(e));
                }
                whr = whr.push(current@.len() as int);
            }
            current.push(old_ids[j]);
            proof {
                rank = rank.push(j as int);
                assert forall|x: u64| #[trigger] current@.contains(x) implies
                    old_ids@.contains(x) by {
                    lemma_push_only(old_current, e, x);
                    if x == e {
                        assert(old_ids@.contains(old_ids@[j as int]));
                    }
                }
                assert forall|q: int| 0 <= q < j + 1 && #[trigger] taker@[q] < n implies
                    current@.contains(old_ids@[q])
                    && 0 <= whr[q] < current@.len()
                    && current@[whr[q]] == old_ids@[q] by {
                    if q < j as int {
                        lemma_push_keeps(old_current, e, old_ids@[q]);
                    } else {
                        assert(current@[old_current.len() as int] == e);
                        assert(current@.contains(e));
                    }
                }
            }
        } else {
            removed.push(old_ids[j]);
            proof {
                rrank = rrank.push(j as int);
                whr = whr.push(0);
            }
        }
        j = j + 1;
    }

    // ---- 3. target_ids / target_fresh ----
    let mut target_ids: Vec<u64> = Vec::with_capacity(n);
    let mut target_fresh: Vec<bool> = Vec::with_capacity(n);
    let mut i: usize = 0;
    while i < n
        invariant
            i <= n,
            n == take_old@.len(),
            m == old_ids@.len(),
            fresh_ids@.len() == n,
            target_ids@.len() == i,
            target_fresh@.len() == i,
            forall|k: int| #![trigger target_fresh@[k]]
                0 <= k < i ==> target_fresh@[k] == (take_old@[k] >= m),
            forall|k: int| #![trigger target_ids@[k]]
                0 <= k < i ==>
                    target_ids@[k] == if take_old@[k] < m {
                        old_ids@[take_old@[k] as int]
                    } else {
                        fresh_ids@[k]
                    },
        decreases n - i,
    {
        let t = take_old[i];
        if t < m {
            target_ids.push(old_ids[t]);
            target_fresh.push(false);
        } else {
            target_ids.push(fresh_ids[i]);
            target_fresh.push(true);
        }
        i = i + 1;
    }

    let r = PlanInputs { current, removed, target_ids, target_fresh };

    proof {
        lemma_nodup_unwrap(r.current@);
        lemma_nodup_unwrap(old_ids@);
        lemma_nodup_wrap(r.current@);

        // (Q) `current`의 원소는 전부 `target_ids`에 있다 — **구조적으로**.
        assert forall|x: u64| #[trigger] r.current@.contains(x) implies
            r.target_ids@.contains(x) by {
            let k = choose|k: int| 0 <= k < r.current@.len() && r.current@[k] == x;
            let q = rank[k];
            let i0 = taker@[q] as int;
            assert(0 <= i0 < n);
            assert(take_old@[i0] == q);
            assert(r.target_ids@[i0] == old_ids@[q]);
            assert(r.target_ids@.contains(r.target_ids@[i0]));
        }

        // (fresh ↔ contains)
        assert forall|i2: int| 0 <= i2 < r.target_ids@.len() implies
            (#[trigger] r.target_fresh@[i2] ==> !r.current@.contains(r.target_ids@[i2]))
            && (!r.target_fresh@[i2] ==> r.current@.contains(r.target_ids@[i2])) by {
            if take_old@[i2] < m {
                assert(taker@[take_old@[i2] as int] < n);
                assert(r.current@.contains(old_ids@[take_old@[i2] as int]));
            } else {
                assert(!old_ids@.contains(fresh_ids@[i2]));
            }
        }

        // (target_ids no_duplicates) — 네 갈래 경우 분석.
        assert forall|a: int, b: int|
            0 <= a < r.target_ids@.len() && 0 <= b < r.target_ids@.len() && a != b implies
            r.target_ids@[a] != r.target_ids@[b] by {
            let ta = take_old@[a];
            let tb = take_old@[b];
            if ta < m && tb < m {
                assert(ta != tb);
                assert(old_ids@[ta as int] != old_ids@[tb as int]);
            } else if ta >= m && tb >= m {
                assert(fresh_ids@[a] != fresh_ids@[b]);
            } else if ta < m {
                assert(!old_ids@.contains(fresh_ids@[b]));
                assert(old_ids@.contains(old_ids@[ta as int]));
            } else {
                assert(!old_ids@.contains(fresh_ids@[a]));
                assert(old_ids@.contains(old_ids@[tb as int]));
            }
        }

        lemma_nodup_wrap(r.target_ids@);

        // (removed ∩ current = ∅) — 같은 옛 슬롯을 두 번 쓸 수 없다.
        assert forall|x: u64| #[trigger] r.removed@.contains(x) implies
            !r.current@.contains(x) by {
            let k = choose|k: int| 0 <= k < r.removed@.len() && r.removed@[k] == x;
            if r.current@.contains(x) {
                let k2 = choose|k2: int| 0 <= k2 < r.current@.len() && r.current@[k2] == x;
                assert(old_ids@[rrank[k]] == old_ids@[rank[k2]]);
                assert(taker@[rank[k2] as int] < n);
                assert(rrank[k] != rank[k2]);
                assert(false);
            }
        }

        // (옛 순서 보존) — `current` 안의 자리 순서가 옛 인덱스 순서와 같다.
        assert(selection_order_preserved(r.current@, old_ids@, take_old@, r.target_ids@)) by {
            reveal(selection_order_preserved);
            lemma_nodup_unwrap(r.current@);
            assert forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
                0 <= a < r.target_ids@.len() && 0 <= b < r.target_ids@.len()
                && take_old@[a] < old_ids@.len() && take_old@[b] < old_ids@.len()
                && take_old@[a] < take_old@[b]
                implies pos(r.current@, r.target_ids@[a]) < pos(r.current@, r.target_ids@[b]) by {
                let qa = take_old@[a] as int;
                let qb = take_old@[b] as int;
                assert(taker@[qa] < n);
                assert(taker@[qb] < n);
                r.current@.index_of_first_ensures(old_ids@[qa]);
                r.current@.index_of_first_ensures(old_ids@[qb]);
                assert(pos(r.current@, old_ids@[qa]) == whr[qa]);
                assert(pos(r.current@, old_ids@[qb]) == whr[qb]);
                assert(whr[qa] < whr[qb]);
            }
        }
    }

    r
}

pub struct SelectedPlan {
    pub current: Vec<u64>,
    pub removed: Vec<u64>,
    pub target_ids: Vec<u64>,
    pub target_fresh: Vec<bool>,
    pub ops: Vec<PlanOp>,
}

pub fn plan_from_selection(
    old_ids: &Vec<u64>,
    take_old: &Vec<usize>,
    fresh_ids: &Vec<u64>,
) -> (r: SelectedPlan)
    requires
        old_ids@.no_duplicates(),
        fresh_ids@.len() == take_old@.len(),
        forall|i: int| #![trigger take_old@[i]]
            0 <= i < take_old@.len() && take_old@[i] >= old_ids@.len()
                ==> !old_ids@.contains(fresh_ids@[i]),
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] < old_ids@.len() && take_old@[b] < old_ids@.len()
                ==> take_old@[a] != take_old@[b],
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] >= old_ids@.len() && take_old@[b] >= old_ids@.len()
                ==> fresh_ids@[a] != fresh_ids@[b],
    ensures
        r.target_ids@.len() == take_old@.len(),
        r.target_fresh@.len() == r.target_ids@.len(),
        forall|i: int| #![trigger r.target_fresh@[i]]
            0 <= i < take_old@.len() ==> r.target_fresh@[i] == (take_old@[i] >= old_ids@.len()),
        forall|i: int| #![trigger r.target_ids@[i]]
            0 <= i < take_old@.len() ==>
                r.target_ids@[i] == if take_old@[i] < old_ids@.len() {
                    old_ids@[take_old@[i] as int]
                } else {
                    fresh_ids@[i]
                },
        forall|k: int| #![trigger r.ops@[k]]
            0 <= k < r.ops@.len() ==>
                r.ops@[k].idx < r.target_ids@.len()
                && r.ops@[k].spawn == r.target_fresh@[r.ops@[k].idx as int],
        ({
            let d = apply_ops(r.current@, r.ops@, r.ops@.len() as int);
            &&& d.len() >= r.target_ids@.len()
            &&& d.subrange(d.len() - r.target_ids@.len(), d.len() as int) =~= r.target_ids@
        }),
        forall|x: u64| #![trigger r.removed@.contains(x)]
            r.removed@.contains(x) ==> !r.current@.contains(x),
{
    let pi = plan_inputs(old_ids, take_old, fresh_ids);
    proof { lemma_nodup_unwrap(pi.current@); lemma_nodup_unwrap(pi.target_ids@); }
    let ops = plan_writes_converging(&pi.current, &pi.target_ids, &pi.target_fresh);
    SelectedPlan {
        current: pi.current,
        removed: pi.removed,
        target_ids: pi.target_ids,
        target_fresh: pi.target_fresh,
        ops,
    }
}

/// **사전조건 P를 `take_old` 항으로 옮긴 것**. `keep`으로 표시된 retained
/// 슬롯들의 **옛 인덱스가 증가**해야 한다.
///
/// `current` 안의 자리 순서는 옛 인덱스 순서와 같으므로
/// (`selection_order_preserved`) 이것은 `kept_order_preserved`와 같은 말이고,
/// 그 번역은 `plan_lis_from_selection` 안에서 기계검사된다.
///
/// **불투명인 이유**: `kept_order_preserved`와 같다 — 2변수 양화자다.
#[verifier::opaque]
pub open spec fn take_order_preserved(old_len: nat, take_old: Seq<usize>, keep: Seq<bool>) -> bool {
    forall|a: int, b: int| #![trigger keep[a], keep[b]]
        0 <= a < b < take_old.len()
            && keep[a] && take_old[a] < old_len
            && keep[b] && take_old[b] < old_len
            ==> take_old[a] < take_old[b]
}

/// **음성 대조 — 번역된 P가 공허하지 않음**. 표시된 두 슬롯이 옛 인덱스를
/// 거꾸로(1 → 0) 가져가면 거짓이어야 한다. `kept_order_preserved`의 음성
/// 대조(`lemma_kept_order_rejects_inversion`)와 같은 모양의 반례다.
pub proof fn lemma_take_order_rejects_inversion()
    ensures
        !take_order_preserved(2, seq![1usize, 0usize], seq![true, true]),
{
    reveal(take_order_preserved);
    let take_old = seq![1usize, 0usize];
    let keep = seq![true, true];
    assert(take_old[0] == 1usize);
    assert(take_old[1] == 0usize);
    assert(keep[0] == true);
    assert(keep[1] == true);
}

/// **양성 대조 — 항상 거짓인 술어가 아님**. 같은 자리에서 `take_old`만
/// 증가시키면 참이 된다(= 이 사전조건을 만족시키는 입력이 실제로 있다).
pub proof fn lemma_take_order_accepts_increasing()
    ensures
        take_order_preserved(2, seq![0usize, 1usize], seq![true, true]),
{
    reveal(take_order_preserved);
    let take_old = seq![0usize, 1usize];
    let keep = seq![true, true];
    assert forall|a: int, b: int| #![trigger keep[a], keep[b]]
        0 <= a < b < take_old.len()
            && keep[a] && take_old[a] < 2nat
            && keep[b] && take_old[b] < 2nat
        implies take_old[a] < take_old[b] by {
        assert(a == 0 && b == 1);
    }
}

// ============================================================================
// P를 **세우는** 쪽 — LIS 표시(`keep`)의 생성 (2026-08-11, 4단계)
// ============================================================================
//
// 여기까지 P(`take_order_preserved`)는 `plan_lis_from_selection`의 `requires`로
// **가정**돼 있었고, 그것을 실제로 세우는 `lis_indices`는 `loom-runtime`의
// 비검증 코드에 있었다 — 릴리스에서 사라지는 `debug_assert!` 하나가 유일한
// 확인이었다. 아래 `lis_keep`이 그 알고리즘을 그대로 옮겨 P를 **사후조건으로**
// 증명한다. 사슬이 사후조건 → 사전조건으로 닫히는 지점은
// `plan_lis_from_selection_auto`다.
//
// **건전성만 증명한다. 최적성(정말 가장 긴 증가 부분수열인가)은 증명하지
// 않는다** — 최적이 아니면 이동이 더 나올 뿐 순서는 깨지지 않으므로 성능
// 성질이지 정합성 성질이 아니다. 필요한 것은 predecessor 사슬이 증가
// 부분수열이라는 국소 불변식뿐이고, 그것이 아래 세 술어다.
//
// **알고리즘은 한 글자도 바뀌지 않았다**(실격 조건 1번). 옛 코드는
// `take_old`에서 retained 슬롯만 뽑은 `retained_old_indices`에 LIS를 돌린 뒤
// 순위(rank)로 되짚어 `keep`을 만들었는데, 여기서는 `take_old`를 직접 훑되
// 비-retained 슬롯을 **건너뛴다**. 처리되는 값의 열과 순서가 같고 비교가
// `sequence[ti] < val` 하나뿐이므로 `tails`/`pred`가 담는 인덱스가 순서
// 동형으로 대응하고, 고르는 집합이 글자 그대로 같다 — 대신 순위 되짚기라는
// 간접 참조 한 겹이 없어져 증명이 짧아진다. 전수 대조로 확인했다(순열 n≤7,
// fresh 혼합 전수, 중복 값 열, 단사+fresh 실제 모양 — 30,153건, 불일치 0건).

/// `tails`가 담는 것은 `sequence`의 **인덱스**다 — 범위 술어를 bound로
/// 매개변수화해 두면 "`sequence` 안"과 "아직 훑지 않은 `i` 앞"을 같은 술어로
/// 쓸 수 있다. 1변수 양화자라 불투명화가 필요 없다.
spec fn tails_bounded(tails: Seq<usize>, bound: int) -> bool {
    forall|k: int| #![trigger tails[k]] 0 <= k < tails.len() ==> tails[k] < bound
}

/// I3 — `tails`가 가리키는 **값**이 엄격히 증가한다.
///
/// **불투명인 이유**: `seq_nodup`·`kept_order_preserved`와 같다 — 2변수
/// 양화자이고, 게다가 `seq[tails[k]]`라는 **이중 인덱싱**이라 루프 문맥에
/// 그냥 두면 트리거가 폭발한다. 펼치는 곳은 이분 탐색의 두 가지와 보존
/// 정리 둘 안뿐이다.
#[verifier::opaque]
spec fn tails_sorted(seq: Seq<usize>, tails: Seq<usize>) -> bool {
    forall|k1: int, k2: int| #![trigger tails[k1], tails[k2]]
        0 <= k1 < k2 < tails.len()
            ==> seq[tails[k1] as int] < seq[tails[k2] as int]
}

/// I1 — predecessor의 **국소** 불변식. 이것 하나가 "사슬을 따라가면 인덱스도
/// 값도 엄격히 감소한다"의 전부이고, 뒤집으면 증가 부분수열이다. 1변수
/// 양화자이므로 루프에 그대로 들고 다녀도 된다.
spec fn pred_local(seq: Seq<usize>, pred: Seq<Option<usize>>) -> bool {
    forall|k: int| #![trigger pred[k]]
        0 <= k < pred.len() && pred[k] is Some
            ==> pred[k]->0 < k && seq[pred[k]->0 as int] < seq[k]
}

/// `tails.partition_point(|&ti| sequence[ti] < val)`과 **의미가 같은** 명시적
/// 이분 탐색.
///
/// **왜 `partition_point`를 그대로 못 쓰는가**: vstd에 스펙이 없다
/// (`core::slice::...::partition_point` is not supported). Verus가 제시하는
/// 유일한 해결책이 `assume_specification`인데 그것은 이 작업의 금지 사항이다.
/// 술어가 `<=`가 아니라 **`<`**이므로 결과는 lower bound이고, 아래 세 사후조건이
/// 그 의미를 그대로 적은 것이다. `mid = lo + (hi - lo) / 2`라 오버플로가 없고
/// 스텝당 O(log n) — 전체 O(n log n)이 유지된다.
fn lower_bound(sequence: &Vec<usize>, tails: &Vec<usize>, val: usize) -> (pos: usize)
    requires
        tails_bounded(tails@, sequence@.len() as int),
        tails_sorted(sequence@, tails@),
    ensures
        pos <= tails@.len(),
        forall|k: int| #![trigger tails@[k]]
            0 <= k < pos ==> sequence@[tails@[k] as int] < val,
        forall|k: int| #![trigger tails@[k]]
            pos <= k < tails@.len() ==> sequence@[tails@[k] as int] >= val,
{
    let mut lo: usize = 0;
    let mut hi: usize = tails.len();
    while lo < hi
        invariant
            lo <= hi,
            hi <= tails@.len(),
            tails_bounded(tails@, sequence@.len() as int),
            tails_sorted(sequence@, tails@),
            forall|k: int| #![trigger tails@[k]]
                0 <= k < lo ==> sequence@[tails@[k] as int] < val,
            forall|k: int| #![trigger tails@[k]]
                hi <= k < tails@.len() ==> sequence@[tails@[k] as int] >= val,
        decreases hi - lo,
    {
        let mid = lo + (hi - lo) / 2;
        let ti = tails[mid];
        if sequence[ti] < val {
            proof { reveal(tails_sorted); }
            lo = mid + 1;
        } else {
            proof { reveal(tails_sorted); }
            hi = mid;
        }
    }
    lo
}

/// I3 보존 — 교체 가지(`tails[pos] = i`). 핵심은 `k1 == pos` 경우다:
/// `seq[tails[pos]] >= val`(이분 탐색의 오른쪽 사후조건)과 기존 정렬
/// `seq[tails[pos]] < seq[tails[k2]]`를 겹치면 `val < seq[tails[k2]]`가
/// **엄격하게** 나온다 — 그래서 중복 값이 있어도 I3가 엄격한 채로 유지된다.
proof fn lemma_sorted_after_set(
    seq: Seq<usize>, tails: Seq<usize>, pos: int, i: usize, val: usize,
)
    requires
        tails_sorted(seq, tails),
        0 <= pos < tails.len(),
        i < seq.len(),
        seq[i as int] == val,
        tails_bounded(tails, seq.len() as int),
        forall|k: int| #![trigger tails[k]]
            0 <= k < pos ==> seq[tails[k] as int] < val,
        forall|k: int| #![trigger tails[k]]
            pos <= k < tails.len() ==> seq[tails[k] as int] >= val,
    ensures
        tails_sorted(seq, tails.update(pos, i)),
{
    reveal(tails_sorted);
    let nt = tails.update(pos, i);
    assert forall|k1: int, k2: int| #![trigger nt[k1], nt[k2]]
        0 <= k1 < k2 < nt.len()
        implies seq[nt[k1] as int] < seq[nt[k2] as int] by {
        assert(nt[k1] == if k1 == pos { i } else { tails[k1] });
        assert(nt[k2] == if k2 == pos { i } else { tails[k2] });
        if k1 == pos {
            assert(seq[tails[pos] as int] < seq[tails[k2] as int]);
            assert(seq[tails[pos] as int] >= val);
        } else if k2 == pos {
            assert(seq[tails[k1] as int] < val);
        } else {
            assert(seq[tails[k1] as int] < seq[tails[k2] as int]);
        }
    }
}

/// I3 보존 — 연장 가지(`tails.push(i)`). 이쪽은 이분 탐색의 왼쪽 사후조건이
/// `pos == tails.len()`에서 모든 `k`를 덮으므로 그대로 나온다.
proof fn lemma_sorted_after_push(
    seq: Seq<usize>, tails: Seq<usize>, i: usize, val: usize,
)
    requires
        tails_sorted(seq, tails),
        i < seq.len(),
        seq[i as int] == val,
        tails_bounded(tails, seq.len() as int),
        forall|k: int| #![trigger tails[k]]
            0 <= k < tails.len() ==> seq[tails[k] as int] < val,
    ensures
        tails_sorted(seq, tails.push(i)),
{
    reveal(tails_sorted);
    let nt = tails.push(i);
    assert forall|k1: int, k2: int| #![trigger nt[k1], nt[k2]]
        0 <= k1 < k2 < nt.len()
        implies seq[nt[k1] as int] < seq[nt[k2] as int] by {
        assert(nt[k1] == tails[k1]);
        if k2 < tails.len() {
            assert(nt[k2] == tails[k2]);
            assert(seq[tails[k1] as int] < seq[tails[k2] as int]);
        } else {
            assert(nt[k2] == i);
            assert(seq[tails[k1] as int] < val);
        }
    }
}

/// **사전조건 P를 세우는 함수** — patience sorting + predecessor 재구성.
/// `loom-runtime`에 있던 `lis_indices` + `keep` 조립을 그대로 옮긴 것이고,
/// 이제 P가 `ensures`로 **증명**된다.
///
/// `take_old[i] >= old_len`인 슬롯(fresh)은 옛 자리가 없으므로 건너뛴다 —
/// 옛 코드가 `retained_old_indices`로 걸러 내던 것과 같다.
///
/// 사전조건이 **없다**: 빈 열은 루프가 그냥 안 돌고, 중복 값이 있어도
/// 무해하다(이분 탐색 술어가 `<`라 사슬이 strictly increasing이다). 그래서
/// `seq_nodup` 같은 2변수 양화자를 `requires`에 걸 필요가 없다 — 걸었다면
/// 이 모듈이 전에 겪은 트리거 폭발을 다시 샀을 것이다.
///
/// 실제 반환값이 무엇인지(= 최적성)는 규정하지 않는다. 규정하는 것은
/// "표시된 것들의 옛 인덱스가 target 순서로 증가한다" 하나다.
pub fn lis_keep(take_old: &Vec<usize>, old_len: usize) -> (keep: Vec<bool>)
    ensures
        keep@.len() == take_old@.len(),
        take_order_preserved(old_len as nat, take_old@, keep@),
{
    let n = take_old.len();

    let mut pred: Vec<Option<usize>> = Vec::with_capacity(n);
    let mut j: usize = 0;
    while j < n
        invariant
            j <= n,
            pred@.len() == j,
            forall|k: int| #![trigger pred@[k]] 0 <= k < j ==> pred@[k] is None,
        decreases n - j,
    {
        pred.push(None);
        j = j + 1;
    }

    // I2/I3/I1을 루프 불변식으로 들고 간다. `tails_bounded(_, i)`가 I1의
    // `p < i`를 세우는 재료이고, `tails_bounded(_, n)`은 이분 탐색의
    // 사전조건이다.
    let mut tails: Vec<usize> = Vec::new();
    let mut i: usize = 0;
    proof { reveal(tails_sorted); }
    while i < n
        invariant
            i <= n,
            n == take_old@.len(),
            pred@.len() == n,
            pred_local(take_old@, pred@),
            tails_bounded(tails@, i as int),
            tails_bounded(tails@, n as int),
            tails_sorted(take_old@, tails@),
        decreases n - i,
    {
        let t = take_old[i];
        if t < old_len {
            let pos = lower_bound(take_old, &tails, t);
            if pos > 0 {
                let p = tails[pos - 1];
                let ghost old_pred = pred@;
                // I1을 이 자리에서 한 번 세우면 끝이다 — `pred[i]`는 이후
                // 다시 쓰이지 않는다.
                assert(p < i);
                assert(take_old@[p as int] < t);
                pred.set(i, Some(p));
                proof {
                    assert forall|k: int| #![trigger pred@[k]]
                        0 <= k < pred@.len() && pred@[k] is Some
                        implies pred@[k]->0 < k
                            && take_old@[pred@[k]->0 as int] < take_old@[k] by {
                        if k != i as int {
                            assert(pred@[k] == old_pred[k]);
                        }
                    }
                }
            }
            if pos == tails.len() {
                proof { lemma_sorted_after_push(take_old@, tails@, i, t); }
                tails.push(i);
            } else {
                proof { lemma_sorted_after_set(take_old@, tails@, pos as int, i, t); }
                tails.set(pos, i);
            }
        }
        i = i + 1;
    }

    let mut keep: Vec<bool> = Vec::with_capacity(n);
    let mut j2: usize = 0;
    while j2 < n
        invariant
            j2 <= n,
            keep@.len() == j2,
            forall|k: int| #![trigger keep@[k]] 0 <= k < j2 ==> !keep@[k],
        decreases n - j2,
    {
        keep.push(false);
        j2 = j2 + 1;
    }

    // 사슬 되짚기. `tails`가 비면 `cursor`가 처음부터 `None`이라 루프가 안
    // 돌고 `keep`은 전부 false — 갈래를 여기서 합쳐 두면 뒤에서 조인이 없다.
    let mut cursor: Option<usize> = if tails.len() > 0 {
        Some(tails[tails.len() - 1])
    } else {
        None
    };

    // 갱신 부담은 **1변수** 불변식이 진다("표시된 모든 m이 cursor보다 뒤에
    // 있고 값도 크다"). 그러면 `keep[cursor] = true`인 시점에 새 원소 대 기존
    // 원소가 전부 덮이므로 2변수 쌍별 성질이 자동 보존된다. 종료 척도는
    // I1의 `p < i`가 그대로 준다.
    while cursor.is_some()
        invariant
            keep@.len() == n,
            n == take_old@.len(),
            pred@.len() == n,
            pred_local(take_old@, pred@),
            cursor is Some ==> cursor->0 < n,
            forall|m: int| #![trigger keep@[m]]
                0 <= m < n && keep@[m] && cursor is Some
                    ==> cursor->0 < m && take_old@[cursor->0 as int] < take_old@[m],
            forall|a: int, b: int| #![trigger keep@[a], keep@[b]]
                0 <= a < b < n && keep@[a] && keep@[b]
                    ==> take_old@[a] < take_old@[b],
        decreases if cursor is Some { cursor->0 + 1 } else { 0 },
    {
        let idx = cursor.unwrap();
        keep.set(idx, true);
        cursor = pred[idx];
    }

    // 쌍별 성질은 P보다 **강하다**(P는 `take_old[_] < old_len`이라는 가정을
    // 더 달고 있다). 불투명 래퍼를 여기서 한 번만 편다.
    proof { reveal(take_order_preserved); }
    keep
}

/// LIS 경로(후보 D-LIS·E-LIS) — 같은 선택에 LIS의 "유지" 표시(`keep`)를 얹어
/// `plan_writes`까지 간다.
///
/// **P만 남는다**: 다른 사전조건 다섯(Q 포함)은 `plan_inputs`가 세우고, P는
/// 여기 `take_order_preserved`로 남는다. P 자체(= `lis_indices`가 그것을
/// 세우는가)는 여전히 검증 밖이고, 그것만이 남은 미검증 계약이다.
pub fn plan_lis_from_selection(
    old_ids: &Vec<u64>,
    take_old: &Vec<usize>,
    fresh_ids: &Vec<u64>,
    keep: &Vec<bool>,
) -> (r: SelectedPlan)
    requires
        old_ids@.no_duplicates(),
        fresh_ids@.len() == take_old@.len(),
        keep@.len() == take_old@.len(),
        forall|i: int| #![trigger take_old@[i]]
            0 <= i < take_old@.len() && take_old@[i] >= old_ids@.len()
                ==> !old_ids@.contains(fresh_ids@[i]),
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] < old_ids@.len() && take_old@[b] < old_ids@.len()
                ==> take_old@[a] != take_old@[b],
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] >= old_ids@.len() && take_old@[b] >= old_ids@.len()
                ==> fresh_ids@[a] != fresh_ids@[b],
        take_order_preserved(old_ids@.len(), take_old@, keep@),
    ensures
        r.target_ids@.len() == take_old@.len(),
        r.target_fresh@.len() == r.target_ids@.len(),
        forall|i: int| #![trigger r.target_fresh@[i]]
            0 <= i < take_old@.len() ==> r.target_fresh@[i] == (take_old@[i] >= old_ids@.len()),
        forall|i: int| #![trigger r.target_ids@[i]]
            0 <= i < take_old@.len() ==>
                r.target_ids@[i] == if take_old@[i] < old_ids@.len() {
                    old_ids@[take_old@[i] as int]
                } else {
                    fresh_ids@[i]
                },
        forall|k: int| #![trigger r.ops@[k]]
            0 <= k < r.ops@.len() ==>
                r.ops@[k].idx < r.target_ids@.len()
                && r.ops@[k].spawn == r.target_fresh@[r.ops@[k].idx as int],
        ({
            let d = apply_ops(r.current@, r.ops@, r.ops@.len() as int);
            &&& d.len() >= r.target_ids@.len()
            &&& d.subrange(d.len() - r.target_ids@.len(), d.len() as int) =~= r.target_ids@
        }),
        forall|x: u64| #![trigger r.removed@.contains(x)]
            r.removed@.contains(x) ==> !r.current@.contains(x),
{
    let pi = plan_inputs(old_ids, take_old, fresh_ids);
    proof {
        // P 번역 — 여기서만 세 불투명 술어를 편다.
        reveal(take_order_preserved);
        reveal(selection_order_preserved);
        reveal(kept_order_preserved);
        lemma_nodup_unwrap(pi.current@);
        lemma_nodup_unwrap(pi.target_ids@);
        assert forall|a: int, b: int| #![trigger keep@[a], keep@[b]]
            0 <= a < b < pi.target_ids@.len()
                && keep@[a] && !pi.target_fresh@[a]
                && keep@[b] && !pi.target_fresh@[b]
            implies pos(pi.current@, pi.target_ids@[a]) < pos(pi.current@, pi.target_ids@[b]) by {
            assert(take_old@[a] < old_ids@.len());
            assert(take_old@[b] < old_ids@.len());
            assert(take_old@[a] < take_old@[b]);
        }
    }
    let ops = plan_writes(&pi.current, &pi.target_ids, &pi.target_fresh, keep);
    SelectedPlan {
        current: pi.current,
        removed: pi.removed,
        target_ids: pi.target_ids,
        target_fresh: pi.target_fresh,
        ops,
    }
}

/// **사슬이 닫히는 지점** (2026-08-11, 4단계). `keep`을 밖에서 받지 않고
/// `lis_keep`으로 **여기서 만든다** — 그래서 `lis_keep`의 사후조건
/// `take_order_preserved`가 바로 아래 `plan_lis_from_selection`의 동명
/// `requires`를 **Verus가 검사하는 자리에서** 충족한다.
///
/// 이 한 겹이 있어야 하는 이유: 값만 넘기고 사람 눈으로 잇는 것은 사슬을 닫지
/// 못한다. `loom-runtime`은 검증 대상이 아니므로 거기서
/// `lis_keep(...)` → `plan_lis_from_selection(..., &keep)`으로 이어 봐야 두
/// 계약 사이는 아무도 검사하지 않는다 — 예전 `debug_assert!`와 같은 종류의
/// 틈이 이름만 바꿔 남는다. 두 호출이 **한 검증 단위 안에** 있어야 한다.
///
/// 그래서 `loom-runtime`이 부르는 것은 이 함수뿐이고, 거기에는 P에 대한
/// 계약이 아예 나타나지 않는다(`keep`이 인자가 아니므로 넘길 것이 없다).
/// 나머지 다섯 사전조건은 `plan_lis_from_selection`의 것을 그대로 물려받는다 —
/// 그것들은 `match_keys` 경계에서 이미 세워져 있다(2026-08-11, 2단계).
pub fn plan_lis_from_selection_auto(
    old_ids: &Vec<u64>,
    take_old: &Vec<usize>,
    fresh_ids: &Vec<u64>,
) -> (r: SelectedPlan)
    requires
        old_ids@.no_duplicates(),
        fresh_ids@.len() == take_old@.len(),
        forall|i: int| #![trigger take_old@[i]]
            0 <= i < take_old@.len() && take_old@[i] >= old_ids@.len()
                ==> !old_ids@.contains(fresh_ids@[i]),
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] < old_ids@.len() && take_old@[b] < old_ids@.len()
                ==> take_old@[a] != take_old@[b],
        forall|a: int, b: int| #![trigger take_old@[a], take_old@[b]]
            0 <= a < b < take_old@.len()
                && take_old@[a] >= old_ids@.len() && take_old@[b] >= old_ids@.len()
                ==> fresh_ids@[a] != fresh_ids@[b],
    ensures
        r.target_ids@.len() == take_old@.len(),
        r.target_fresh@.len() == r.target_ids@.len(),
        forall|i: int| #![trigger r.target_fresh@[i]]
            0 <= i < take_old@.len() ==> r.target_fresh@[i] == (take_old@[i] >= old_ids@.len()),
        forall|i: int| #![trigger r.target_ids@[i]]
            0 <= i < take_old@.len() ==>
                r.target_ids@[i] == if take_old@[i] < old_ids@.len() {
                    old_ids@[take_old@[i] as int]
                } else {
                    fresh_ids@[i]
                },
        forall|k: int| #![trigger r.ops@[k]]
            0 <= k < r.ops@.len() ==>
                r.ops@[k].idx < r.target_ids@.len()
                && r.ops@[k].spawn == r.target_fresh@[r.ops@[k].idx as int],
        ({
            let d = apply_ops(r.current@, r.ops@, r.ops@.len() as int);
            &&& d.len() >= r.target_ids@.len()
            &&& d.subrange(d.len() - r.target_ids@.len(), d.len() as int) =~= r.target_ids@
        }),
        forall|x: u64| #![trigger r.removed@.contains(x)]
            r.removed@.contains(x) ==> !r.current@.contains(x),
{
    let keep = lis_keep(take_old, old_ids.len());
    plan_lis_from_selection(old_ids, take_old, fresh_ids, &keep)
}

} // verus!
