// RQ5 후보 #4 — flush 규약 v0의 순서 보장.
//
// `crates/loom-runtime/src/list.rs`의 `place_in_order`가 방출하는 write 시퀀스와
// `crates/loom-runtime/src/lib.rs`의 `LoomApp::tick` 적용 루프를 모델링해,
// RQ1-benchmark-protocol.md §5 트랙 N 불변식 5가 요구하는 성질을 증명한다.
//
// **증명 의무(RQ5 #4)**
//   1. anchor 선행성: `before: Some(a)`를 실은 모든 write에 대해, 그 write가
//      적용되는 시점에 `a`는 이미 DOM에 존재한다.
//   2. 순서 수렴: (1) 하에서 적용 루프의 최종 순서 = target 순서.
//
// **왜 (1)이 결정적인가**: `LoomApp::tick`의 적용 루프에는 anchor 해석 실패 시
// 조용히 폴백하는 경로가 둘 있다(SpawnRow는 reposition을 건너뛰어 새 노드가
// 끝에 남고, Reposition은 before=None이 되어 끝으로 이동). 에러도 로그도 없다.
// 즉 순서 정합성 전체가 (1) 하나에 걸려 있고, (1)은 `place_in_order`가 target을
// **역순으로** 순회한다는 사실에서 나온다 — `.rev()` 한 글자가 load-bearing이다.
//
// 실행: `verus formal-verification/verus/flush_order.rs`

use vstd::prelude::*;
use vstd::seq_lib::*;
use vstd::hash_map::*;
use vstd::std_specs::hash::*;

verus! {

/// `place_in_order`의 `Slot`에 대응. `Retained`는 이미 DOM에 있는 row,
/// `Fresh`는 이번 배치에서 새로 spawn되는 row.
pub enum Slot {
    Retained(usize),
    Fresh(usize),
}

impl Slot {
    pub open spec fn id(self) -> usize {
        match self {
            Slot::Retained(e) => e,
            Slot::Fresh(e) => e,
        }
    }

    pub open spec fn is_fresh(self) -> bool {
        match self {
            Slot::Fresh(_) => true,
            Slot::Retained(_) => false,
        }
    }
}

/// `DomWrite`의 순서 관련 부분만 추린 것. `AppendStaticChildren` 등 순서에
/// 영향을 주지 않는 variant는 모델에서 제외한다(적용 루프가 그것들을 처리해도
/// 자식 순서는 바뀌지 않으므로 성질 (1)/(2)에 무관).
pub enum Write {
    /// `DomWrite::SpawnRow { entity, before, .. }`
    Spawn(usize, Option<usize>),
    /// `DomWrite::Reposition { entity, before, .. }`
    Move(usize, Option<usize>),
}

impl Write {
    pub open spec fn id(self) -> usize {
        match self {
            Write::Spawn(e, _) => e,
            Write::Move(e, _) => e,
        }
    }

    pub open spec fn before(self) -> Option<usize> {
        match self {
            Write::Spawn(_, b) => b,
            Write::Move(_, b) => b,
        }
    }

    pub open spec fn is_spawn(self) -> bool {
        match self {
            Write::Spawn(_, _) => true,
            Write::Move(_, _) => false,
        }
    }
}

/// "적용 시점에 `id`가 DOM에 존재하는가" — `LoomApp::tick`의
/// `self.dom_target(e)`가 `Some`을 돌려주는 조건의 모델. 두 경로로 존재할 수
/// 있다: (a) 배치 시작 전부터 DOM에 있었다(= `current`에 포함, Retained),
/// (b) 이 배치의 **더 앞선** write가 spawn했다.
pub open spec fn resolvable_at(
    current: Seq<usize>,
    writes: Seq<Write>,
    k: int,
    id: usize,
) -> bool {
    current.contains(id) || exists|j: int|
        0 <= j < k && (#[trigger] writes[j]).is_spawn() && writes[j].id() == id
}

/// **증명 의무 (1)** 의 형식화: 모든 write에 대해, 그것이 anchor를 참조한다면
/// 그 anchor는 그 write가 적용되는 시점에 이미 해석 가능하다.
pub open spec fn anchors_always_resolvable(current: Seq<usize>, writes: Seq<Write>) -> bool {
    forall|k: int| #![trigger writes[k]]
        0 <= k < writes.len() ==> match writes[k].before() {
            Some(a) => resolvable_at(current, writes, k, a),
            None => true,
        }
}

// ---------------------------------------------------------------------------
// 방출: `place_in_order`의 구조를 그대로 옮긴 것
// ---------------------------------------------------------------------------

/// `place_in_order`가 하는 일 중 **순서에 관한 부분만** 옮긴 것. 원본과의
/// 대응은 아래와 같다:
///
/// ```text
/// for slot in target.iter().rev() {          // ← 여기서 i를 내려가며 순회
///     Fresh    => push SpawnRow { before: next_anchor }
///     Retained => (인접하지 않으면) push Reposition { before: next_anchor }
///     next_anchor = Some(entity);
/// }
/// ```
///
/// **의도적으로 다르게 둔 것 하나**: 원본의 `already_adjacent` 스킵 최적화를
/// 모델에서는 **생략**하고 Retained도 항상 `Move`를 방출한다. 이유는 스킵이
/// write를 **줄이기만** 하기 때문이다 — 방출되지 않은 write는 anchor를 참조할
/// 수도 없으므로 성질 (1)을 깨뜨릴 수 없다(스킵된 row는 Retained이므로
/// `current.contains`로 계속 해석 가능하다). 즉 여기서 증명하는 것은
/// **스킵 이전의 상위집합**이고, 상위집합에서 (1)이 성립하면 부분집합에서도
/// 성립한다. 이 논증 자체를 정리로 만드는 것은 남은 과제로 명시한다.
pub fn emit_writes(target: &Vec<Slot>) -> (writes: Vec<Write>)
    ensures
        writes@.len() == target@.len(),
        // 방출 순서는 target의 역순 — 이게 정리 (1)의 근거다.
        forall|k: int| #![trigger writes@[k]]
            0 <= k < writes@.len() ==> writes@[k].id() == target@[target@.len() - 1 - k].id(),
        forall|k: int| #![trigger writes@[k]]
            0 <= k < writes@.len() ==> writes@[k].is_spawn() == target@[target@.len() - 1 - k].is_fresh(),
        // 각 write의 anchor는 "target에서 자기 바로 오른쪽" 원소다.
        forall|k: int| #![trigger writes@[k]]
            0 <= k < writes@.len() ==> writes@[k].before() == if k == 0 {
                None::<usize>
            } else {
                Some(target@[target@.len() - k].id())
            },
{
    let n = target.len();
    let mut writes: Vec<Write> = Vec::new();
    let mut next_anchor: Option<usize> = None;
    let mut i: usize = n;

    while i > 0
        invariant
            i <= n,
            n == target@.len(),
            writes@.len() == n - i,
            next_anchor == if i == n { None::<usize> } else { Some(target@[i as int].id()) },
            forall|k: int| #![trigger writes@[k]]
                0 <= k < writes@.len() ==> writes@[k].id() == target@[n - 1 - k].id(),
            forall|k: int| #![trigger writes@[k]]
                0 <= k < writes@.len() ==> writes@[k].is_spawn() == target@[n - 1 - k].is_fresh(),
            forall|k: int| #![trigger writes@[k]]
                0 <= k < writes@.len() ==> writes@[k].before() == if k == 0 {
                    None::<usize>
                } else {
                    Some(target@[n - k].id())
                },
        decreases i,
    {
        i = i - 1;
        let slot = &target[i];
        let e = match slot {
            Slot::Retained(x) => *x,
            Slot::Fresh(x) => *x,
        };
        let w = match slot {
            Slot::Fresh(_) => Write::Spawn(e, next_anchor),
            Slot::Retained(_) => Write::Move(e, next_anchor),
        };
        writes.push(w);
        next_anchor = Some(e);
    }
    writes
}

// ---------------------------------------------------------------------------
// 정리 (1): anchor 선행성
// ---------------------------------------------------------------------------

/// **RQ5 후보 #4의 증명 의무 (1)**.
///
/// 전제: `target`의 모든 `Retained` 원소는 배치 시작 시점의 DOM(`current`)에
/// 이미 있다 — 이것은 `place_in_order` 호출부(`list_reconcile_d_system` 등)가
/// `Slot::Retained`를 "매칭에서 살아남은 기존 entity"로만 만들기 때문에
/// 성립하는, 호출부가 지키는 전제조건이다.
///
/// 결론: `emit_writes`가 낸 시퀀스에서 anchor는 **항상** 해석 가능하다.
/// 따라서 `LoomApp::tick`의 두 폴백 경로(anchor 해석 실패)는 **한 번도
/// 실행되지 않는다**.
pub proof fn anchor_precedence(current: Seq<usize>, target: Seq<Slot>, writes: Seq<Write>)
    requires
        writes.len() == target.len(),
        forall|k: int| #![trigger writes[k]]
            0 <= k < writes.len() ==> writes[k].id() == target[target.len() - 1 - k].id(),
        forall|k: int| #![trigger writes[k]]
            0 <= k < writes.len() ==> writes[k].is_spawn() == target[target.len() - 1 - k].is_fresh(),
        forall|k: int| #![trigger writes[k]]
            0 <= k < writes.len() ==> writes[k].before() == if k == 0 {
                None::<usize>
            } else {
                Some(target[target.len() - k].id())
            },
        // 호출부가 지키는 전제: Retained는 이미 DOM에 있다.
        forall|m: int| #![trigger target[m]]
            0 <= m < target.len() && !target[m].is_fresh() ==> current.contains(target[m].id()),
    ensures
        anchors_always_resolvable(current, writes),
{
    assert forall|k: int| #![trigger writes[k]]
        0 <= k < writes.len() implies match writes[k].before() {
            Some(a) => resolvable_at(current, writes, k, a),
            None => true,
        }
    by {
        if k > 0 {
            // anchor는 target[len - k], 즉 target에서 이 write의 원소
            // (target[len - 1 - k])의 **바로 오른쪽** 원소다.
            let m = target.len() - k;
            let a = target[m].id();
            if target[m].is_fresh() {
                // Fresh였다면 그 Spawn은 writes[k-1]에 있다 — k보다 **앞선다**.
                // (역순 순회의 직접적 귀결: target 인덱스 m = len-1-(k-1))
                assert(writes[k - 1].id() == target[target.len() - 1 - (k - 1)].id());
                assert(target.len() - 1 - (k - 1) == m);
                assert(writes[k - 1].is_spawn());
                assert(0 <= k - 1 < k);
            } else {
                // Retained였다면 전제에 의해 배치 시작부터 DOM에 있다.
                assert(current.contains(a));
            }
        }
    }
}


// ===========================================================================
// 정리 (2) — 순서 수렴
// ===========================================================================


/// 적용 루프의 한 write에 대응하는 DOM 연산.
/// `LoomApp::tick`의 `SpawnRow`(생성 후 anchor 앞으로 이동)와
/// `Reposition`(anchor 앞으로 이동) 둘 다 "e를 DOM에서 떼어내 anchor 바로
/// 앞에 놓는다"로 같아진다 — Spawn은 떼어낼 게 없다는 점만 다르다.
pub open spec fn place_before(dom: Seq<usize>, e: usize, before: Option<usize>) -> Seq<usize> {
    let d = dom.remove_value(e);
    match before {
        None => d.push(e),
        Some(a) => match d.index_of_first(a) {
            Some(i) => d.insert(i, e),
            // 정리 (1)에 의해 도달 불가한 폴백(= tick의 조용한 폴백 경로).
            None => d.push(e),
        },
    }
}

/// **L1**: 뒤쪽 조각에 `e`가 없으면, 이어붙인 시퀀스에서 첫 occurrence를 지우는
/// 것은 앞쪽 조각에서만 지우는 것과 같다.
pub proof fn lemma_remove_value_in_prefix(p: Seq<usize>, s: Seq<usize>, e: usize)
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
pub proof fn lemma_index_of_first_at_boundary(p: Seq<usize>, s: Seq<usize>, a: usize)
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
pub proof fn lemma_insert_at_boundary(p: Seq<usize>, s: Seq<usize>, e: usize)
    ensures (p + s).insert(p.len() as int, e) =~= p.push(e) + s,
{
    assert((p + s).subrange(0, p.len() as int) =~= p);
    assert((p + s).subrange(p.len() as int, (p + s).len() as int) =~= s);
}


/// **L4**: 원소를 지워도 "원래 없던 것은 여전히 없다".
pub proof fn lemma_remove_value_preserves_absence(s: Seq<usize>, e: usize, x: usize)
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

/// **L5**: 중복 없는 시퀀스에서 첫 occurrence를 지우면 그 값은 사라지고,
/// 중복 없음도 유지된다.
pub proof fn lemma_remove_value_no_dup(s: Seq<usize>, e: usize)
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

/// **L6**: 없던 원소를 끼워 넣으면 중복 없음이 유지된다.
pub proof fn lemma_insert_no_dup(s: Seq<usize>, i: int, e: usize)
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
pub proof fn lemma_place_extends_suffix(d: Seq<usize>, k: int, e: usize)
    requires
        d.no_duplicates(),
        0 < k <= d.len(),
        !d.subrange(d.len() - k, d.len() as int).contains(e),
    ensures
        ({
            let suf = d.subrange(d.len() - k, d.len() as int);
            let r = place_before(d, e, Some(suf[0]));
            &&& r.no_duplicates()
            &&& r.len() >= k + 1
            &&& r.subrange(r.len() - (k + 1), r.len() as int) =~= seq![e] + suf
        }),
{
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
}


/// `emit_writes`가 낸 시퀀스의 앞 `k`개를 적용한 결과.
/// `emit_writes`의 사후조건(writes[j].id() == t[n-1-j],
/// writes[j].before() == if j==0 {None} else {Some(t[n-j].id())})을 `t`(=목표
/// 순서의 id열)만으로 다시 쓴 것 — 정리 (1)에 의해 anchor는 항상 해석되므로
/// `place_before`의 `None` 폴백 가지는 도달하지 않는다.
pub open spec fn apply_k(dom: Seq<usize>, t: Seq<usize>, k: int) -> Seq<usize>
    decreases k
{
    if k <= 0 {
        dom
    } else {
        let prev = apply_k(dom, t, k - 1);
        let e = t[t.len() - k];
        if k == 1 {
            place_before(prev, e, None)
        } else {
            place_before(prev, e, Some(t[t.len() - k + 1]))
        }
    }
}

/// **정리 (2) — 순서 수렴 (귀납 핵심)**. `k`개의 write를 적용하고 나면 DOM의
/// 뒤쪽 `k`개가 목표 순서의 뒤쪽 `k`개와 정확히 같다.
pub proof fn suffix_invariant(dom: Seq<usize>, t: Seq<usize>, k: int)
    requires
        dom.no_duplicates(),
        t.no_duplicates(),
        0 <= k <= t.len(),
    ensures
        ({
            let r = apply_k(dom, t, k);
            &&& r.no_duplicates()
            &&& r.len() >= k
            &&& r.subrange(r.len() - k, r.len() as int) =~= t.subrange(t.len() - k, t.len() as int)
        }),
    decreases k,
{
    if k <= 0 {
        let r = apply_k(dom, t, k);
        assert(r =~= dom);
        assert(r.subrange(r.len() - 0, r.len() as int) =~= t.subrange(t.len() as int, t.len() as int));
    } else {
        suffix_invariant(dom, t, k - 1);
        let d = apply_k(dom, t, k - 1);
        let e = t[t.len() - k];
        let r = apply_k(dom, t, k);

        if k == 1 {
            lemma_remove_value_no_dup(d, e);
            let d2 = d.remove_value(e);
            assert(r =~= d2.push(e));
            lemma_insert_no_dup(d2, d2.len() as int, e);
            assert(d2.insert(d2.len() as int, e) =~= d2.push(e));
            assert(r.len() >= 1);
            assert(r.subrange(r.len() - 1, r.len() as int) =~= t.subrange(t.len() - 1, t.len() as int)) by {
                assert(r[r.len() - 1] == e);
            }
        } else {
            // 이전 단계의 접미사가 곧 이번 anchor의 근거다.
            assert(d.subrange(d.len() - (k - 1), d.len() as int)
                   =~= t.subrange(t.len() - (k - 1), t.len() as int));
            let suf = d.subrange(d.len() - (k - 1), d.len() as int);
            assert(suf[0] == t[t.len() - k + 1]);
            // e는 접미사에 없다 — t에 중복이 없고 e의 인덱스가 접미사 범위 밖이므로.
            assert(!suf.contains(e)) by {
                assert forall|j: int| 0 <= j < suf.len() implies suf[j] != e by {
                    assert(suf[j] == t[t.len() - (k - 1) + j]);
                    assert(t.len() - (k - 1) + j != t.len() - k);
                }
            }
            lemma_place_extends_suffix(d, k - 1, e);
            assert(r.subrange(r.len() - k, r.len() as int) =~= seq![e] + suf);
            assert(seq![e] + suf =~= t.subrange(t.len() - k, t.len() as int)) by {
                assert forall|j: int| 0 <= j < k
                    implies (seq![e] + suf)[j] == t.subrange(t.len() - k, t.len() as int)[j] by {
                    if j == 0 { } else { assert(suf[j - 1] == t[t.len() - (k - 1) + j - 1]); }
                }
            }
        }
    }
}


/// **정리 (2) — 순서 수렴**. `emit_writes`가 낸 write를 전부 적용하고 나면,
/// DOM의 **뒤쪽 |target|개가 목표 순서와 정확히 일치**한다.
///
/// 접미사로 서술하는 이유(과장하지 않기 위해): `place_in_order`의 호출부는
/// target에 없는 기존 row를 **별도 경로로** 제거하므로, 이 함수만 놓고 보면
/// 제거되지 않은 잔여 원소가 앞쪽에 남을 수 있다. 그 잔여를 모델링하지 않고도
/// 말할 수 있는 가장 강한 참인 명제가 "뒤쪽 n개 = target"이다.
pub proof fn order_converges(dom: Seq<usize>, t: Seq<usize>)
    requires dom.no_duplicates(), t.no_duplicates(),
    ensures
        ({
            let r = apply_k(dom, t, t.len() as int);
            &&& r.len() >= t.len()
            &&& r.subrange(r.len() - t.len(), r.len() as int) =~= t
        }),
{
    suffix_invariant(dom, t, t.len() as int);
    assert(t.subrange(0, t.len() as int) =~= t);
}

/// **따름정리**: 잔여 원소가 없으면(길이가 같으면) 최종 DOM이 목표 순서 **그
/// 자체**다. 순수 재정렬 워크로드(W1~W4 — §9 판정 셀들)가 정확히 이 경우다.
pub proof fn order_converges_exactly(dom: Seq<usize>, t: Seq<usize>)
    requires
        dom.no_duplicates(),
        t.no_duplicates(),
        apply_k(dom, t, t.len() as int).len() == t.len(),
    ensures
        apply_k(dom, t, t.len() as int) =~= t,
{
    order_converges(dom, t);
    let r = apply_k(dom, t, t.len() as int);
    assert(r.subrange(0, r.len() as int) =~= r);
}



// ===========================================================================
// already_adjacent 스킵 최적화의 안전성
// ===========================================================================

/// `place_in_order`의 `already_adjacent` 판정 중 anchor가 있는 쪽:
/// "`e`가 `d` 안에서 `a` **바로 앞**에 있는가". 원본은 이 질의를 `Chain`
/// (`next_of`)으로 O(1)에 하지만, `Chain`은 지금까지 방출한 write를 적용한
/// 뒤의 순서를 그대로 추적하는 자료구조이므로 의미는 이 술어와 같다.
pub open spec fn adjacent(d: Seq<usize>, e: usize, a: usize) -> bool {
    exists|i: int| 0 <= i < d.len() - 1 && #[trigger] d[i] == e && d[i + 1] == a
}

/// **스킵 안전성 (anchor 있는 경우)**: 이미 인접해 있으면 그 write는 **항등**
/// 이다 — 즉 방출하지 않아도 DOM은 같다. 이것이 `already_adjacent` 분기가
/// 정합성을 해치지 않는 이유다.
pub proof fn lemma_skip_is_noop(d: Seq<usize>, e: usize, a: usize)
    requires d.no_duplicates(), adjacent(d, e, a),
    ensures place_before(d, e, Some(a)) =~= d,
{
    let i = choose|i: int| 0 <= i < d.len() - 1 && #[trigger] d[i] == e && d[i + 1] == a;
    d.index_of_first_ensures(e);
    // e의 첫 occurrence는 i다 (중복이 없으므로).
    assert(d.index_of_first(e) == Some(i)) by {
        assert(d.contains(e));
        let j = d.index_of_first(e).unwrap();
        assert(d[j] == e);
        assert(j == i);
    }
    let d2 = d.remove(i);
    assert(d.remove_value(e) =~= d2);
    d.remove_ensures(i);

    // d2에서 a의 첫 위치는 i다.
    assert(d2[i] == a);
    assert(d2.index_of_first(a) == Some(i)) by {
        d2.index_of_first_ensures(a);
        assert(d2.contains(a));
        let j = d2.index_of_first(a).unwrap();
        assert(d2[j] == a);
        // d2[j]는 d[j] (j<i) 또는 d[j+1] (j>=i) 이고, 중복이 없으므로 j==i.
        if j < i {
            assert(d2[j] == d[j]);
            assert(d[j] != d[i + 1]);
        } else if j > i {
            assert(d2[j] == d[j + 1]);
            assert(d[j + 1] != d[i + 1]);
        }
    }

    // i 위치에 e를 다시 끼우면 원래 d가 된다.
    assert(d2.insert(i, e) =~= d) by {
        assert(d2.insert(i, e).len() == d.len());
        assert forall|j: int| 0 <= j < d.len() implies d2.insert(i, e)[j] == d[j] by {
            d2.insert_ensures(i, e);
            if j < i {
                assert(d2.insert(i, e)[j] == d2[j]);
                assert(d2[j] == d[j]);
            } else if j == i {
                assert(d2.insert(i, e)[i] == e);
            } else {
                assert(d2.insert(i, e)[j] == d2[j - 1]);
                assert(d2[j - 1] == d[j]);
            }
        }
    }
}

/// **스킵 안전성 (anchor 없는 경우)**: `next_anchor == None`일 때 원본은
/// `chain.is_tail(entity)`로 판정한다 — 이미 꼬리면 그 write도 항등이다.
pub proof fn lemma_skip_is_noop_tail(d: Seq<usize>, e: usize)
    requires d.no_duplicates(), d.len() > 0, d[d.len() - 1] == e,
    ensures place_before(d, e, None) =~= d,
{
    let i = d.len() - 1;
    d.index_of_first_ensures(e);
    assert(d.index_of_first(e) == Some(i)) by {
        assert(d.contains(e));
        let j = d.index_of_first(e).unwrap();
        assert(d[j] == e);
        assert(j == i);
    }
    d.remove_ensures(i);
    let d2 = d.remove(i);
    assert(d.remove_value(e) =~= d2);
    assert(d2.push(e) =~= d) by {
        assert forall|j: int| 0 <= j < d.len() implies d2.push(e)[j] == d[j] by {
            if j < i { assert(d2.push(e)[j] == d2[j]); assert(d2[j] == d[j]); }
            else { assert(d2.push(e)[i] == e); }
        }
    }
}


/// `place_in_order`가 **실제로** 하는 것 — `already_adjacent`이면 write를
/// 방출하지 않는다. `apply_k`(스킵 없는 상위집합)와 대비되는, 원본 코드에
/// 대응하는 버전이다.
pub open spec fn apply_k_opt(dom: Seq<usize>, t: Seq<usize>, k: int) -> Seq<usize>
    decreases k
{
    if k <= 0 {
        dom
    } else {
        let prev = apply_k_opt(dom, t, k - 1);
        let e = t[t.len() - k];
        if k == 1 {
            // 원본의 `chain.is_tail(entity)` 분기.
            if prev.len() > 0 && prev[prev.len() - 1] == e {
                prev
            } else {
                place_before(prev, e, None)
            }
        } else {
            // 원본의 `chain.next_of(entity) == Some(a)` 분기.
            let a = t[t.len() - k + 1];
            if adjacent(prev, e, a) {
                prev
            } else {
                place_before(prev, e, Some(a))
            }
        }
    }
}

/// **스킵 최적화 안전성 (본정리)**: 스킵하는 실제 구현과 스킵하지 않는
/// 상위집합이 **같은 DOM**을 낸다. 따라서 정리 (2)가 상위집합에 대해 증명된
/// 것으로 충분하며, 최적화가 순서 정합성을 바꾸지 않는다.
pub proof fn skip_optimization_is_safe(dom: Seq<usize>, t: Seq<usize>, k: int)
    requires
        dom.no_duplicates(),
        t.no_duplicates(),
        0 <= k <= t.len(),
    ensures
        apply_k_opt(dom, t, k) =~= apply_k(dom, t, k),
    decreases k,
{
    if k <= 0 {
    } else {
        skip_optimization_is_safe(dom, t, k - 1);
        suffix_invariant(dom, t, k - 1);
        let prev = apply_k(dom, t, k - 1);
        assert(apply_k_opt(dom, t, k - 1) == prev);
        let e = t[t.len() - k];
        if k == 1 {
            if prev.len() > 0 && prev[prev.len() - 1] == e {
                lemma_skip_is_noop_tail(prev, e);
            }
        } else {
            let a = t[t.len() - k + 1];
            if adjacent(prev, e, a) {
                lemma_skip_is_noop(prev, e, a);
            }
        }
    }
}

/// **따름정리**: 실제 구현(스킵 포함)도 목표 순서로 수렴한다.
pub proof fn order_converges_with_skip(dom: Seq<usize>, t: Seq<usize>)
    requires dom.no_duplicates(), t.no_duplicates(),
    ensures
        ({
            let r = apply_k_opt(dom, t, t.len() as int);
            &&& r.len() >= t.len()
            &&& r.subrange(r.len() - t.len(), r.len() as int) =~= t
        }),
{
    skip_optimization_is_safe(dom, t, t.len() as int);
    order_converges(dom, t);
}


// ===========================================================================
// Chain ↔ adjacent 대응 (모델-구현 결선)
// ===========================================================================


/// `list.rs`의 `Chain`이 **어떤 시퀀스를 표기하는가**를 규정한다.
/// `Chain`은 head/tail + next/prev 두 맵으로 순서를 들고 있는데, 그것이
/// 시퀀스 `s`를 올바르게 표기한다는 것이 이 술어다. 도달가능성/무순환을
/// 따로 논증하지 않는 이유는 `s`를 고스트로 함께 들고 다니기 때문이다 —
/// 표현이 아니라 **표현과 의미의 대응**을 불변식으로 유지한다.
pub open spec fn chain_denotes(
    next: Map<int, usize>,
    prev: Map<int, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    s: Seq<usize>,
) -> bool {
    &&& s.no_duplicates()
    &&& (if s.len() == 0 { head is None } else { head == Some(s[0]) })
    &&& (if s.len() == 0 { tail is None } else { tail == Some(s[s.len() - 1]) })
    // next: s[i] -> s[i+1], 마지막 원소는 next 없음
    &&& forall|i: int| #![trigger s[i]]
            0 <= i < s.len() - 1 ==> next.contains_key(s[i] as int) && next[s[i] as int] == s[i + 1]
    &&& !(s.len() > 0 && next.contains_key(s[s.len() - 1] as int))
    // prev: s[i] -> s[i-1], 첫 원소는 prev 없음
    &&& forall|i: int| #![trigger s[i]]
            0 < i < s.len() ==> prev.contains_key(s[i] as int) && prev[s[i] as int] == s[i - 1]
    &&& !(s.len() > 0 && prev.contains_key(s[0] as int))
    // 맵의 정의역은 시퀀스 밖으로 새지 않는다 (없는 원소를 조회하면 None).
    &&& forall|k: int| #![trigger next.contains_key(k)]
            next.contains_key(k) ==> exists|i: int| 0 <= i < s.len() - 1 && s[i] as int == k
    &&& forall|k: int| #![trigger prev.contains_key(k)]
            prev.contains_key(k) ==> exists|i: int| 0 < i < s.len() && s[i] as int == k
}

/// **Chain ↔ adjacent 대응 (질의 방향)**: `Chain::next_of(e) == Some(a)`는
/// 표기 시퀀스에서 `e`가 `a` 바로 앞에 있다는 것과 **동치**다.
///
/// 이것이 `place_in_order`의 `already_adjacent` 판정이 정당한 이유다 —
/// 원본은 O(1) 맵 조회로 판정하고, 정리 (2)의 스킵 안전성 증명은 시퀀스상의
/// 인접성을 가정하는데, 그 둘이 같은 것임을 여기서 잇는다.
pub proof fn chain_next_of_iff_adjacent(
    next: Map<int, usize>,
    prev: Map<int, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    s: Seq<usize>,
    e: usize,
    a: usize,
)
    requires chain_denotes(next, prev, head, tail, s),
    ensures
        (next.contains_key(e as int) && next[e as int] == a) <==> adjacent(s, e, a),
{
    // (⇐) 인접하면 next가 그렇게 매핑한다 — chain_denotes의 첫 forall 그대로.
    if adjacent(s, e, a) {
        let i = choose|i: int| 0 <= i < s.len() - 1 && #[trigger] s[i] == e && s[i + 1] == a;
        assert(next.contains_key(s[i] as int));
        assert(next[s[i] as int] == s[i + 1]);
    }
    // (⇒) next에 있으면 그 키는 시퀀스 안의 어떤 s[i](i < len-1)이고,
    //     그 자리에서 next의 값은 s[i+1]이다. 중복이 없으므로 e의 자리는 유일.
    if next.contains_key(e as int) && next[e as int] == a {
        let i = choose|i: int| 0 <= i < s.len() - 1 && s[i] as int == e as int;
        assert(s[i] == e);
        assert(next[s[i] as int] == s[i + 1]);
        assert(s[i + 1] == a);
        assert(adjacent(s, e, a));
    }
}

/// **Chain ↔ tail 대응**: `Chain::is_tail(e)`는 표기 시퀀스의 마지막 원소가
/// `e`라는 것과 동치다 — `next_anchor == None`일 때의 스킵 판정 근거.
pub proof fn chain_is_tail_iff_last(
    next: Map<int, usize>,
    prev: Map<int, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    s: Seq<usize>,
    e: usize,
)
    requires chain_denotes(next, prev, head, tail, s),
    ensures (tail == Some(e)) <==> (s.len() > 0 && s[s.len() - 1] == e),
{
}


/// `Chain::unlink`의 맵 조작을 그대로 옮긴 것(`list.rs` 참고).
/// 네 갈래는 원본의 `match (p, n)`과 1:1이다.
pub open spec fn unlink_next(next: Map<int, usize>, prev: Map<int, usize>, e: usize) -> Map<int, usize> {
    let has_p = prev.contains_key(e as int);
    let has_n = next.contains_key(e as int);
    let n0 = next.remove(e as int);
    if has_p && has_n {
        n0.insert(prev[e as int] as int, next[e as int])
    } else if has_p && !has_n {
        n0.remove(prev[e as int] as int)
    } else {
        n0
    }
}

pub open spec fn unlink_prev(next: Map<int, usize>, prev: Map<int, usize>, e: usize) -> Map<int, usize> {
    let has_p = prev.contains_key(e as int);
    let has_n = next.contains_key(e as int);
    let p0 = prev.remove(e as int);
    if has_p && has_n {
        p0.insert(next[e as int] as int, prev[e as int])
    } else if !has_p && has_n {
        p0.remove(next[e as int] as int)
    } else {
        p0
    }
}

pub open spec fn unlink_head(next: Map<int, usize>, prev: Map<int, usize>, head: Option<usize>, e: usize) -> Option<usize> {
    let has_p = prev.contains_key(e as int);
    let has_n = next.contains_key(e as int);
    if !has_p && has_n { Some(next[e as int]) }
    else if !has_p && !has_n { None }
    else { head }
}

pub open spec fn unlink_tail(next: Map<int, usize>, prev: Map<int, usize>, tail: Option<usize>, e: usize) -> Option<usize> {
    let has_p = prev.contains_key(e as int);
    let has_n = next.contains_key(e as int);
    if has_p && !has_n { Some(prev[e as int]) }
    else if !has_p && !has_n { None }
    else { tail }
}

/// **Chain ↔ adjacent 대응 (변이 방향, unlink)**: 표기 `s`인 chain에서
/// `e`를 unlink하면 표기가 `s.remove_value(e)`가 된다.
pub proof fn chain_unlink_denotes_remove(
    next: Map<int, usize>,
    prev: Map<int, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    s: Seq<usize>,
    e: usize,
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
    assert(prev.contains_key(e as int) <==> i > 0) by {
        if i > 0 { assert(prev.contains_key(s[i] as int)); }
        if i == 0 { assert(!prev.contains_key(s[0] as int)); }
    }
    assert(next.contains_key(e as int) <==> i < s.len() - 1) by {
        if i < s.len() - 1 { assert(next.contains_key(s[i] as int)); }
        if i == s.len() - 1 { assert(!next.contains_key(s[s.len() - 1] as int)); }
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
        0 <= j < t.len() - 1 implies n2.contains_key(t[j] as int) && n2[t[j] as int] == t[j + 1] by {
        if j < i - 1 {
            assert(t[j] == s[j] && t[j + 1] == s[j + 1]);
            assert(next.contains_key(s[j] as int) && next[s[j] as int] == s[j + 1]);
            assert(s[j] != s[i]);
        } else if j == i - 1 {
            assert(t[j] == s[i - 1] && t[j + 1] == s[i + 1]);
        } else {
            assert(t[j] == s[j + 1] && t[j + 1] == s[j + 2]);
            assert(next.contains_key(s[j + 1] as int) && next[s[j + 1] as int] == s[j + 2]);
            assert(s[j + 1] != s[i]);
        }
    }
    assert(!(t.len() > 0 && n2.contains_key(t[t.len() - 1] as int))) by {
        if t.len() > 0 {
            if i < s.len() - 1 {
                // e가 마지막이 아니면 t의 끝은 그대로 s의 끝이고,
                // next는 원래 그 키를 갖지 않았다. 삽입 키 s[i-1]도 그것과 다르다.
                assert(t[t.len() - 1] == s[s.len() - 1]);
                assert(!next.contains_key(s[s.len() - 1] as int));
            } else {
                // e가 마지막이면 t의 끝은 s[i-1]이고, unlink가 그 키를 지웠다.
                assert(t[t.len() - 1] == s[i - 1]);
            }
        }
    }

    // prev 방향
    assert forall|j: int| #![trigger t[j]]
        0 < j < t.len() implies p2.contains_key(t[j] as int) && p2[t[j] as int] == t[j - 1] by {
        if j < i {
            assert(t[j] == s[j] && t[j - 1] == s[j - 1]);
            assert(prev.contains_key(s[j] as int) && prev[s[j] as int] == s[j - 1]);
            assert(s[j] != s[i]);
        } else if j == i {
            assert(t[j] == s[i + 1] && t[j - 1] == s[i - 1]);
        } else {
            assert(t[j] == s[j + 1] && t[j - 1] == s[j]);
            assert(prev.contains_key(s[j + 1] as int) && prev[s[j + 1] as int] == s[j]);
            assert(s[j + 1] != s[i]);
        }
    }
    assert(!(t.len() > 0 && p2.contains_key(t[0] as int))) by {
        if t.len() > 0 {
            if i > 0 {
                // e가 첫 원소가 아니면 t의 머리는 그대로 s[0]이고,
                // prev는 원래 그 키를 갖지 않았다. 삽입 키 s[i+1]도 그것과 다르다.
                assert(t[0] == s[0]);
                assert(!prev.contains_key(s[0] as int));
            } else {
                // e가 첫 원소면 t의 머리는 s[1]이고, unlink가 그 키를 지웠다.
                assert(t[0] == s[1]);
            }
        }
    }

    // 정의역이 새지 않음
    assert forall|k: int| #![trigger p2.contains_key(k)]
        p2.contains_key(k) implies exists|j: int| 0 < j < t.len() && t[j] as int == k by {
        if p2.contains_key(k) {
            if i > 0 && i < s.len() - 1 && k == s[i + 1] as int {
                assert(t[i] as int == k);
                assert(0 < i < t.len());
            } else {
                let j0 = choose|j0: int| 0 < j0 < s.len() && s[j0] as int == k;
                assert(s[j0] as int == k);
                assert(j0 != i);
                let j = if j0 < i { j0 } else { j0 - 1 };
                assert(t[j] as int == k);
                assert(0 < j < t.len());
            }
        }
    }
    assert forall|k: int| #![trigger n2.contains_key(k)]
        n2.contains_key(k) implies exists|j: int| 0 <= j < t.len() - 1 && t[j] as int == k by {
        if n2.contains_key(k) {
            if i > 0 && i < s.len() - 1 && k == s[i - 1] as int {
                assert(t[i - 1] as int == k);
                assert(0 <= i - 1 < t.len() - 1);
            } else {
                let j0 = choose|j0: int| 0 <= j0 < s.len() - 1 && s[j0] as int == k;
                assert(s[j0] as int == k);
                assert(j0 != i);
                let j = if j0 < i { j0 } else { j0 - 1 };
                assert(t[j] as int == k);
                assert(0 <= j < t.len() - 1);
            }
        }
    }
}


/// `Chain::insert_before(e, Some(a))`의 맵 조작을 그대로 옮긴 것.
pub open spec fn ib_next(next: Map<int, usize>, prev: Map<int, usize>, e: usize, a: usize) -> Map<int, usize> {
    let n1 = if prev.contains_key(a as int) { next.insert(prev[a as int] as int, e) } else { next };
    n1.insert(e as int, a)
}

pub open spec fn ib_prev(prev: Map<int, usize>, e: usize, a: usize) -> Map<int, usize> {
    let p1 = if prev.contains_key(a as int) { prev.insert(e as int, prev[a as int]) } else { prev };
    p1.insert(a as int, e)
}

pub open spec fn ib_head(prev: Map<int, usize>, head: Option<usize>, e: usize, a: usize) -> Option<usize> {
    if prev.contains_key(a as int) { head } else { Some(e) }
}

/// **Chain ↔ adjacent 대응 (변이 방향, insert_before)**: 표기 `s`인 chain에
/// (아직 없는) `e`를 `a` 바로 앞에 끼우면 표기가 `s.insert(j, e)`가 된다
/// (`j`는 `a`의 위치).
pub proof fn chain_insert_before_denotes_insert(
    next: Map<int, usize>,
    prev: Map<int, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    s: Seq<usize>,
    e: usize,
    a: usize,
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

    assert(prev.contains_key(a as int) <==> j > 0) by {
        if j > 0 { assert(prev.contains_key(s[j] as int)); }
        if j == 0 { assert(!prev.contains_key(s[0] as int)); }
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
        0 <= x < t.len() - 1 implies n2.contains_key(t[x] as int) && n2[t[x] as int] == t[x + 1] by {
        if x == j {
            assert(t[x] == e && t[x + 1] == a);
        } else if x == j - 1 {
            assert(t[x] == s[j - 1] && t[x + 1] == e);
        } else if x < j - 1 {
            assert(t[x] == s[x] && t[x + 1] == s[x + 1]);
            assert(next.contains_key(s[x] as int) && next[s[x] as int] == s[x + 1]);
            assert(s[x] != e && s[x] != s[j - 1]);
        } else {
            assert(t[x] == s[x - 1] && t[x + 1] == s[x]);
            assert(next.contains_key(s[x - 1] as int) && next[s[x - 1] as int] == s[x]);
            assert(s[x - 1] != e);
            if j > 0 { assert(s[x - 1] != s[j - 1]); }
        }
    }
    assert(!(t.len() > 0 && n2.contains_key(t[t.len() - 1] as int))) by {
        assert(t[t.len() - 1] == s[s.len() - 1]);
        assert(!next.contains_key(s[s.len() - 1] as int));
        assert(s[s.len() - 1] != e);
        if j > 0 { assert(s[s.len() - 1] != s[j - 1]); }
    }

    assert forall|x: int| #![trigger t[x]]
        0 < x < t.len() implies p2.contains_key(t[x] as int) && p2[t[x] as int] == t[x - 1] by {
        if x == j {
            assert(t[x] == e && t[x - 1] == s[j - 1]);
        } else if x == j + 1 {
            assert(t[x] == a && t[x - 1] == e);
        } else if x < j {
            assert(t[x] == s[x] && t[x - 1] == s[x - 1]);
            assert(prev.contains_key(s[x] as int) && prev[s[x] as int] == s[x - 1]);
            assert(s[x] != e && s[x] != a);
        } else {
            assert(t[x] == s[x - 1] && t[x - 1] == s[x - 2]);
            assert(prev.contains_key(s[x - 1] as int) && prev[s[x - 1] as int] == s[x - 2]);
            assert(s[x - 1] != e && s[x - 1] != a);
        }
    }
    assert(!(t.len() > 0 && p2.contains_key(t[0] as int))) by {
        if j > 0 {
            assert(t[0] == s[0]);
            assert(!prev.contains_key(s[0] as int));
            assert(s[0] != e && s[0] != a);
        } else {
            assert(t[0] == e);
        }
    }

    assert forall|k: int| #![trigger p2.contains_key(k)]
        p2.contains_key(k) implies exists|x: int| 0 < x < t.len() && t[x] as int == k by {
        if p2.contains_key(k) {
            if k == a as int {
                assert(t[j + 1] as int == k); assert(0 < j + 1 < t.len());
            } else if j > 0 && k == e as int {
                assert(t[j] as int == k); assert(0 < j < t.len());
            } else {
                let x0 = choose|x0: int| 0 < x0 < s.len() && s[x0] as int == k;
                let x = if x0 < j { x0 } else { x0 + 1 };
                assert(t[x] as int == k); assert(0 < x < t.len());
            }
        }
    }
    assert forall|k: int| #![trigger n2.contains_key(k)]
        n2.contains_key(k) implies exists|x: int| 0 <= x < t.len() - 1 && t[x] as int == k by {
        if n2.contains_key(k) {
            if k == e as int {
                assert(t[j] as int == k); assert(0 <= j < t.len() - 1);
            } else if j > 0 && k == s[j - 1] as int {
                assert(t[j - 1] as int == k); assert(0 <= j - 1 < t.len() - 1);
            } else {
                let x0 = choose|x0: int| 0 <= x0 < s.len() - 1 && s[x0] as int == k;
                let x = if x0 < j { x0 } else { x0 + 1 };
                assert(t[x] as int == k); assert(0 <= x < t.len() - 1);
            }
        }
    }
}


/// **합성 — Chain ↔ `place_before` 대응 (본정리)**: `place_in_order`가
/// 비인접 Retained에 대해 하는 `unlink(e)` → `insert_before(e, Some(a))`
/// 시퀀스는, 표기 수준에서 정확히 `place_before(s, e, Some(a))`다.
///
/// 이로써 정리 (2)/스킵 안전성이 `place_before` 위에서 증명한 모든 것이
/// **원본의 `Chain` 조작에 그대로 적용**된다 — 앞선 라운드에서 "코드를 읽어
/// 확인했을 뿐 정리로 잇지 않았다"고 남겨 둔 간극이 여기서 닫힌다.
pub proof fn chain_ops_match_place_before(
    next: Map<int, usize>,
    prev: Map<int, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    s: Seq<usize>,
    e: usize,
    a: usize,
)
    requires
        chain_denotes(next, prev, head, tail, s),
        s.contains(e),
        s.remove_value(e).contains(a),
    ensures
        ({
            let s1 = s.remove_value(e);
            let n1 = unlink_next(next, prev, e);
            let p1 = unlink_prev(next, prev, e);
            let h1 = unlink_head(next, prev, head, e);
            let t1 = unlink_tail(next, prev, tail, e);
            let j = s1.index_of_first(a).unwrap();
            &&& chain_denotes(
                    ib_next(n1, p1, e, a), ib_prev(p1, e, a), ib_head(p1, h1, e, a), t1,
                    place_before(s, e, Some(a)),
                )
        }),
{
    chain_unlink_denotes_remove(next, prev, head, tail, s, e);
    let s1 = s.remove_value(e);
    let n1 = unlink_next(next, prev, e);
    let p1 = unlink_prev(next, prev, e);
    let h1 = unlink_head(next, prev, head, e);
    let t1 = unlink_tail(next, prev, tail, e);
    s1.index_of_first_ensures(a);
    let j = s1.index_of_first(a).unwrap();
    lemma_remove_value_no_dup(s, e);
    assert(!s1.contains(e));
    chain_insert_before_denotes_insert(n1, p1, h1, t1, s1, e, a, j);
    assert(place_before(s, e, Some(a)) =~= s1.insert(j, e));
}


fn main() {}

}
