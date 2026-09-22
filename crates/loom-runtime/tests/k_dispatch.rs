//! §10 "K: 디스패치 규칙"의 네 분기가 조문대로인지.
//!
//! 2026-09-22까지 `k_body`에는 **(4)번밖에 없었다** — 무조건 전량 despawn+respawn.
//! 그래서 §3.4가 K의 등록된 유리 구간으로 잡아 둔 W5(append-only)와 W9(clear)에서
//! 대조군이 자기 최선형이 아니라 **최악형**으로 측정되고 있었다(원칙 1 위반).

use loom_runtime::{k_dispatch, KDispatch};

#[test]
fn clause_branch_2_identical_is_noop() {
    assert_eq!(k_dispatch(&[1, 2, 3], &[1, 2, 3]), KDispatch::NoOp);
    assert_eq!(k_dispatch::<i32>(&[], &[]), KDispatch::NoOp);
}

#[test]
fn clause_branch_3_pure_append_and_truncate() {
    // W5(append-only)가 타야 하는 분기.
    assert_eq!(k_dispatch(&[1, 2], &[1, 2, 3, 4]), KDispatch::Append(2));
    assert_eq!(k_dispatch::<i32>(&[], &[1]), KDispatch::Append(0));
    // truncate.
    assert_eq!(k_dispatch(&[1, 2, 3, 4], &[1, 2]), KDispatch::Truncate(2));
    // W9(clear)가 타야 하는 분기 — 조문: "전량 삭제는 clear_children".
    assert_eq!(k_dispatch(&[1, 2, 3], &[] as &[i32]), KDispatch::Clear);
}

#[test]
fn clause_branch_4_anything_else_is_full_respawn() {
    // 재정렬(W1/W2/W3) — 위치가 어긋나므로 전량.
    assert_eq!(k_dispatch(&[1, 2, 3], &[3, 2, 1]), KDispatch::Full);
    // head-insert(W4) — 첫 위치부터 어긋난다. append처럼 보이지만 prefix가 아니다.
    assert_eq!(k_dispatch(&[1, 2], &[9, 1, 2]), KDispatch::Full);
    // 값 변경(W7) — 조문이 "값-변경 retain 분기는 두지 않는다"고 못박았다.
    assert_eq!(k_dispatch(&[1, 2, 3], &[1, 9, 3]), KDispatch::Full);
    // 아이템 교체(W10).
    assert_eq!(k_dispatch(&[1, 2], &[7, 8]), KDispatch::Full);
    // 꼬리가 다른 truncate — prefix가 아니므로 전량.
    assert_eq!(k_dispatch(&[1, 2, 3], &[1, 9]), KDispatch::Full);
}

#[test]
fn scan_is_positional_not_matching() {
    // **매칭이면** [2,3]이 [1,2,3]의 부분열이므로 "앞에 1 삽입"으로 보일 것이다.
    // 위치 대응 스캔은 그런 판단을 하지 않는다 — 인덱스 0에서 2 != 1이므로 Full.
    // 이 구별이 §10의 "매칭·검색 없음"과 docs/03 판정례 4의 대상외 지위를 세운다.
    assert_eq!(k_dispatch(&[2, 3], &[1, 2, 3]), KDispatch::Full);
    // 같은 값이 여러 번 나와도 위치로만 본다.
    assert_eq!(k_dispatch(&[1, 1, 2], &[1, 1, 2, 1]), KDispatch::Append(3));
}
