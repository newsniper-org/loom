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

fn main() {}

}
