//! TSX (real SWC AST, parsed by `swc_ecma_parser` — no hand-rolled parser,
//! per docs/02 "왜 직접 파서를 안 짜는가") → `ir::ComponentGraph`.
//!
//! Supported subset (Phase 1/증분 4 prototype scope, documented here rather
//! than silently assumed):
//!   - exactly one *exported* `function NAME() { ... }` per module (the
//!     graph's root) — Phase 1's original restriction, kept (see
//!     `collect_component_fn_decls`'s doc comment for why 증분 4 doesn't
//!     relax this rule itself, only what surrounds it)
//!   - any number of additional, non-exported top-level `function NAME() {
//!     ... }` declarations — helper components, only meaningful if some
//!     `<Name client:load|client:visible .../>` reference (transitively
//!     reachable from the root) names them (증분 4, Task #25)
//!   - `const NAME = signal(NUMBER)` or `const NAME = signal([STRING, ...])`
//!   - a single JSX return, containing:
//!     - static tags (`div`/`span`/`ul`/`li`/`button`) with an optional
//!       static `class` attribute
//!     - static text children
//!     - `{signal()}` text bindings
//!     - `{signal().map((row) => <tag>{row}</tag>)}` list bindings. **2026-08-07
//!       이후** row 템플릿은 `{item}`으로 시작하기만 하면 그 뒤로 정적 마크업과
//!       **조건부 하나**(그 안에 정적 마크업 + 중첩 리스트 하나)를 가질 수 있다
//!       — RQ1 §5 트랙 N의 3중 중첩이 이것으로 표현된다(codegen.rs 모듈 문서의
//!       "row 템플릿의 사이트별 monomorphize" 절)
//!     - `{cond() && <tag>...</tag>}` 조건부. **2026-08-07 이후** 그 안에 정적
//!       마크업 / `{signal()}` / `{signal().map(...)}` / 그것들을 감싸는
//!       엘리먼트를 넣을 수 있다(그 전에는 정적 텍스트만)
//!     - **row 스코프 파생** (row 템플릿 안에서만):
//!       `{open() && item.length > n && <X/>}` 와
//!       `{xs().slice(0, item.length).map(...)}` — 둘 다 닫힌 형태이고,
//!       계산은 생성 코드에 인라인된다(`ir::RowPredicate`/`ir::RowSlice`)
//!     - `onClick={() => signal.increment()|.append("lit")|.rotate()}`
//!     - `<ComponentName client:load|client:visible prop={literal} .../>` —
//!       an uppercase-leading tag name is *always* classified as a
//!       component reference, never an HTML tag (JSX's own universal
//!       lexical convention — the same rule every framework in this
//!       ecosystem already uses, not something specific to one of them;
//!       reused rather than reinvented per CLAUDE.md 2-1). 증분 4 scope:
//!       the directive is *required* — a bare `<Name/>` with no `client:`
//!       directive (ordinary non-Island nested composition) is rejected
//!       with a clear "not yet supported" error (see `lower_component_ref`).
//! Anything outside this is a compile error — this is a real, narrow
//! compiler front end, not a fuzzy best-effort one.
//!
//! **Phase 7a C2 (2026-10-17)**: 거부는 더 이상 `panic!`이 아니라
//! `Err(Diagnostic)`이다(문제 구문의 AST span을 단다). 7a 이전부터 있던 거부는
//! 모두 코드 `LOOM-L00`(지원 범위 밖 저작 표면), single-writer 검사는
//! `LOOM-L01`이다 — 7a가 새로 더하는 거부(`LOOM-Rnn`, design-7a §1.1)와
//! 구별하려는 것이다. 문구는 기존 테스트의 부분 문자열이 그대로 맞도록
//! 유지했다(앞의 `loomc: ` 접두와 끝의 AST `Debug` 덤프만 뺐다 — 진단 헤더가
//! 코드를, span 강조가 해당 소스를 이미 보여 준다). 진짜 내부 불변식인
//! `unreachable!` 2곳(매치 가드가 이미 보장한 모양을 다시 벗기는 곳)만 남겼다.
//! 성공 경로의 IR은 C1과 같다 — 산출물 바이트는 게이트 A가 확인한다.
//!
//! **Phase 7a C5 (2026-10-17)**: 인자 묶음 거부를 더했다 — `LOOM-R03`
//! (`.increment()`/`.rotate()` 인자), `LOOM-R04`(`.toggle()` 인자), `LOOM-R18`
//! (`client:*` 값·중복), `LOOM-R19`(i32가 아닌 숫자 리터럴, 길이 비교의 비정수),
//! `LOOM-R26`(인자 개수 초과, `<input>` 자식). 모두 지금까지 **조용히 다른 뜻으로**
//! 컴파일되던 모양이고, 받아 주는 입력의 IR은 그대로다.
//!
//! **Phase 7a C6 (2026-10-17)**: 구조 묶음 거부를 더했다 — `LOOM-R01`(컴포넌트
//! prop·매개변수: props는 아직 전달되지 않는다), `LOOM-R02`(컴포넌트 참조의
//! children), `LOOM-R05`(`onClick` 두 번), `LOOM-R06`(같은 attr 두 번 —
//! `class`/`className`은 합쳐 센다, `<input>`의 `value`/`onInput` 포함),
//! `LOOM-R22`(`return` 뒤의 문장), `LOOM-R24`(소문자 태그가 선언된 컴포넌트
//! 이름과 같음), `LOOM-R25`(지원하지 않는 최상위 항목 — 지금까지 `_ => {}`로
//! 조용히 버려졌다), `LOOM-R27`(async/generator/타입 매개변수 함수·화살표,
//! `let`/`var`). 함께 **경고** `LOOM-W25`(쓴 `signal`/`sharedSignal`/
//! `sharedValue`를 `"loom"`에서 import하지 않음 — callee 이름마다 1건)를 낸다.
//! W25가 오류가 아닌 이유는 소비 픽스처 8회 호출(widget-embed의 `sharedValue`,
//! largeapp 합성 소스의 `signal`)이 이 모양이기 때문이다(design-7a §1.1 R25).
//! R01의 prop 검사는 기존 `lower_prop_literal`을 **먼저** 돌린 뒤에 낸다 —
//! 비리터럴 prop은 옛 문구 그대로 거부되므로, lib.rs의 M10 증거 테스트
//! (`component_ref_prop_cannot_carry_a_live_signal_value`, RQ6-verification-plan
//! 인용)의 관측 메커니즘과 문구가 바뀌지 않는다.
//!
//! **Phase 7a C7 (2026-10-17)**: 그래프 묶음 거부를 더했다 — `LOOM-R07`(한
//! signal이 binding 사이트 2곳 이상: 생성 코드의 `{s}_entity`가 가려져 앞
//! 사이트가 갱신되지 않는다), `LOOM-R07U`(표시되는 곳이 없는 signal — codegen의
//! "never spawned" 내부 오류를 승격), `LOOM-R08`(리스트 row 루트의 `onClick` —
//! 조용히 사라졌다), `LOOM-R44`(이벤트를 받는 요소 안의 이벤트 수신자·컴포넌트
//! 참조), `LOOM-R46`(Rust 키워드·`_`·ASCII 영숫자/`_` 밖 문자가 든 signal·컴포넌트
//! 이름), `LOOM-R47`(루트에서 도달할 수 없는 헬퍼 — 모드 무관). R46의 모듈 이름
//! 충돌과 G01(단일 컴포넌트 경로의 컴포넌트 참조)은 모드에 따라 달라 check.rs가 낸다.
//!
//! **2026-10-18 정정(Phase 7a 후속 F1)**: R46 진단 문구를 원인별로 사실대로 나눴다
//! (키워드·`$` = rustc 실패, 비ASCII signal = 7b에서 지원 예정 — rustc 실패라고 쓰지 않음,
//! 비ASCII 컴포넌트 = wasm-bindgen E0754). 컴포넌트 이름 키워드 검사는 걷었다.
//! `check_signal_name`/`check_component_name` 문서 참고.
//!
//! 함께 `SpanTable`을 돌려준다. IR(`ir.rs`)에 span을 넣지 않는 이유는 IR의
//! `derive(PartialEq)`와 codegen의 구조체 생성을 건드리지 않기 위해서다
//! (design-7a §3.3). 그래프 검사(single-writer, 이후 커밋의 R07/R44 등)는 이
//! 표에서 사이트 위치를 찾는다.

use crate::diag::Diagnostic;
use crate::ir::{
    ClickAction, Component, ComponentGraph, IslandActivation, JsxNode, PropLiteral, RowPredicate,
    RowSlice, SharedBinding, SignalDecl, SignalInit,
};
use std::collections::HashSet;
use swc_common::{BytePos, Span, Spanned};
use swc_ecma_ast::*;

/// 7a 이전부터 있던 "지원 범위 밖" 거부의 공용 코드.
pub(crate) const L00: &str = "LOOM-L00";
/// signal당 액션 1개(single-writer, Task #28-f) 위반.
pub(crate) const L01: &str = "LOOM-L01";

// 7a C5(2026-10-17, design-7a §1.1): 인자 묶음 — 지금까지 조용히 다른 뜻으로
// 컴파일되던 인자 모양을 거부한다.
/// `.increment()`/`.rotate()`에 인자(지금은 무시되고 +1/한 칸 회전).
pub(crate) const R03: &str = "LOOM-R03";
/// `.toggle()`에 인자(지금은 무시되고 반전).
pub(crate) const R04: &str = "LOOM-R04";
/// `client:*` 디렉티브에 값, 또는 디렉티브가 둘.
pub(crate) const R18: &str = "LOOM-R18";
/// i32 정수가 아닌 숫자 리터럴(지금은 `as i32`로 잘리거나 포화), 길이 비교의 비정수.
pub(crate) const R19: &str = "LOOM-R19";
/// 인자 개수 초과(초과분은 지금 무시), `<input>`의 자식(지금은 사라짐).
pub(crate) const R26: &str = "LOOM-R26";

// 7a C6(2026-10-17, design-7a §1.1): 구조 묶음.
/// 컴포넌트 prop·매개변수(props는 아직 전달되지 않는다 — 값이 버려지고 선언 초기값이 쓰임).
pub(crate) const R01: &str = "LOOM-R01";
/// 컴포넌트 참조의 children(전달되지 않고 SSR에서도 사라짐).
pub(crate) const R02: &str = "LOOM-R02";
/// 한 요소에 `onClick`이 두 번(앞 핸들러가 버려지고 single-writer 검사도 우회).
pub(crate) const R05: &str = "LOOM-R05";
/// 한 요소에 같은 attr가 두 번(뒤의 것만 남음). `class`/`className`은 합쳐 센다.
pub(crate) const R06: &str = "LOOM-R06";
/// `return` 뒤의 문장(지금은 마지막 `return`이 루트가 된다).
pub(crate) const R22: &str = "LOOM-R22";
/// 소문자 태그가 선언된 컴포넌트 이름과 같음(HTML 태그로 해석된다).
pub(crate) const R24: &str = "LOOM-R24";
/// 지원하지 않는 최상위 항목(지금까지 조용히 무시됐다).
pub(crate) const R25: &str = "LOOM-R25";
/// async/generator/타입 매개변수 함수·화살표, `let`/`var` 선언.
pub(crate) const R27: &str = "LOOM-R27";
/// **경고**: 쓴 callee를 `"loom"`에서 import하지 않음.
pub(crate) const W25: &str = "LOOM-W25";

// 7a C7(2026-10-17, design-7a §1.1): 그래프 묶음.
/// 한 signal이 binding 사이트 2곳 이상에 쓰임(지금은 생성 코드의 `{s}_entity`가
/// 가려져 첫 사이트가 갱신되지 않는다 — 사이트별 entity 미구현).
pub(crate) const R07: &str = "LOOM-R07";
/// 선언됐지만 표시되는 곳이 없는 signal(옛 codegen "never spawned" 내부 오류의 승격).
pub(crate) const R07U: &str = "LOOM-R07U";
/// 리스트 row 루트의 `onClick`(지금은 조용히 사라진다 — `enum Action {}`).
pub(crate) const R08: &str = "LOOM-R08";
/// 이벤트를 받는 요소 안에 또 이벤트를 받는 요소/컴포넌트 참조.
pub(crate) const R44: &str = "LOOM-R44";
/// 생성 Rust에서 쓸 수 없는 이름(Rust 키워드·예약어·`_`, ASCII 영숫자·`_` 밖의 문자).
/// (2026-10-18 F1: 문구를 원인별로 정정, 비ASCII signal 일반 지원은 7b — `check_signal_name`.)
/// 컴포넌트 이름의 모듈 이름 충돌(R46(c))은 모드 검사라 check.rs가 같은 코드로 낸다.
pub(crate) const R46: &str = "LOOM-R46";
/// 루트에서 도달할 수 없는 헬퍼 컴포넌트.
pub(crate) const R47: &str = "LOOM-R47";

/// R46(a): Rust 2024의 strict·reserved 키워드(그리고 `_`). 약한 키워드(`union`,
/// `raw`, `safe`, `macro_rules`)는 식별자로 쓸 수 있으므로 넣지 않는다. 조회 전용.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false",
    "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "abstract", "become", "box", "do", "final", "macro", "override", "priv", "try", "typeof",
    "unsized", "virtual", "yield", "_",
];

/// R46(a)(b): 이름이 생성 Rust의 식별자로 그대로 쓰일 수 있는지. signal 이름은
/// `Signals` 필드·`{s}_entity` 변수로, 컴포넌트 이름은 `{C}Island`·모듈 이름으로
/// 방출된다. `$`와 비ASCII 문자(U+200C/U+200D, 비-NFC 형태 포함)는 JS 식별자로는
/// 유효하지만 rustc가 받지 않거나(`$`) 같은 이름이 다르게 정규화된다.
///
/// **2026-10-18 정정(Phase 7a 후속 F1, 소유자 위임 결정)**: 위 판정(7a C7 —
/// "ASCII 영숫자·`_` 밖이면 거부", 진단은 "rustc에서 실패")은 한글·`café`처럼
/// rustc가 실제로 받는 signal 이름까지 "rustc 실패"라고 사실과 다르게 거부했다
/// (r46-research: 7a 이전 컴파일러 + rustc 1.98.1로 islands·flatten·single 세 경로
/// 모두 통과 측정). 그래서 둘로 나눴다.
/// - [`check_signal_name`]: 키워드·`$` 등은 rustc 실패로 거부, 비ASCII는 "7b에서 지원"으로 거부
///   (일반 판정 UAX#31 XID + NFC는 의존성 추가가 측정 산출물 바이트를 바꾸므로 7b — 함수 안 주석).
/// - [`check_component_name`]: ASCII만(wasm32에서 `#[wasm_bindgen]` 내보내기
///   `{C}Island`가 E0754로 실패). 컴포넌트 이름 키워드 검사는 걷었다 — 루트가 아닌
///   컴포넌트는 대문자로 시작해 걸리는 키워드가 `Self`뿐인데 그 방출형은
///   `self_generated`·`SelfIsland`라 막는 결함이 없었다.
///
/// signal 이름만 검사 대상이다: `sharedSignal`/`sharedValue`의 **키**는 생성 코드에
/// `{:?}` 문자열 리터럴로만 들어가므로(codegen `shared.set({key:?}, …)`,
/// `data-loom-shared-subscriber`) Rust 식별자 규칙이 걸리지 않는다.
fn check_signal_name(name: &str, span: Span) -> LResult<()> {
    let err = |msg: String| Err(Diagnostic::error(R46, span, msg));
    let shown = name.escape_default();
    if RUST_KEYWORDS.contains(&name) {
        return err(format!(
            "signal 이름 `{shown}`은 Rust 키워드라 생성 코드(`Signals` 필드)가 rustc에서 실패합니다(7b: `r#` 지원 예정)"
        ));
    }
    let Some(c) = name.chars().find(|c| !(c.is_ascii_alphanumeric() || *c == '_')) else {
        return Ok(());
    };
    if c.is_ascii() {
        // `$` 등: JS 식별자로는 유효하지만 Rust 식별자에 없는 문자 — rustc가 실제로 실패한다.
        return err(format!(
            "signal 이름 `{shown}`에 Rust 식별자에 쓸 수 없는 문자 U+{:04X}가 있어 생성 코드가 rustc에서 실패합니다(7b: 맹글링 예정)",
            c as u32
        ));
    }
    // 비ASCII: rustc가 받는 이름(한글·`café` 등)도 많지만, 받는지 가리려면 UAX#31 XID 판정과
    // NFC 판정(Rust는 식별자를 NFC로 정규화 — JS에서 다른 두 이름이 합쳐짐)이 필요하다.
    // 그 판정용 의존성(unicode-ident, unicode-normalization)을 컴파일러에 넣으면 소비자 크레이트의
    // build-dependency 해시가 바뀌어 OUT_DIR 경로가 달라지고, 그 경로가 박히는 측정 산출물(wasm)
    // 바이트가 바뀐다(2026-10-18 F1 게이트 실측: host out_dir 불일치로 FAIL) — 바이트 동일이 조건인
    // 7a에서는 넣을 수 없어 7b로 넘긴다. 그래서 "rustc 실패"라고 쓰지 않는다.
    err(format!(
        "signal 이름 `{shown}`의 비ASCII 문자 U+{:04X}는 아직 지원하지 않습니다 — 비ASCII signal 이름은 Rust 식별자(UAX#31 XID)·NFC 판정과 함께 7b에서 지원 예정입니다(rustc가 받는 이름도 있지만, 지금은 받는지 정확히 가릴 수 없어 거부합니다)",
        c as u32
    ))
}

/// R46(b) 컴포넌트 쪽: ASCII 영숫자·`_`만. 위 [`check_signal_name`] 문서 참고.
/// `{C}Island`를 wasm-bindgen으로 내보내지 않는 Flatten·루트 단일 경로에서는 비ASCII
/// 컴포넌트 이름이 wasm32 `cargo check`를 통과한다(F1 스크래치 측정) — 그 경로에서는
/// 과잉 거부임을 알고 모드 무관 규칙을 택했다(맹글링은 7b).
fn check_component_name(name: &str, span: Span) -> LResult<()> {
    let Some(c) = name.chars().find(|c| !(c.is_ascii_alphanumeric() || *c == '_')) else {
        return Ok(());
    };
    let shown = name.escape_default();
    let msg = if c.is_ascii() {
        format!("컴포넌트 이름 `{shown}`에 Rust 식별자에 쓸 수 없는 문자 U+{:04X}가 있어 생성 코드가 rustc에서 실패합니다", c as u32)
    } else {
        format!(
            "컴포넌트 이름 `{shown}`에 ASCII가 아닌 문자 U+{:04X}가 있습니다 — Islands·단일 Island 경로에서 wasm-bindgen으로 내보내는 `{shown}Island`는 wasm32에서 ASCII 식별자만 받습니다(rustc E0754 `#[no_mangle] requires ASCII identifier`). 경로마다 규칙이 갈리지 않도록 모든 모드에서 거부합니다(7b: 맹글링 예정)",
            c as u32
        )
    };
    Err(Diagnostic::error(R46, span, msg))
}

/// `"loom"`에서 import할 수 있는 이름(= 저작 표면의 callee 전부).
const LOOM_EXPORTS: [&str; 3] = ["signal", "sharedSignal", "sharedValue"];

type LResult<T> = Result<T, Diagnostic>;

fn unsupported(span: Span, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(L00, span, message)
}

/// 숫자 리터럴의 원문(없으면 값). 진단 문구용.
fn num_raw(n: &Number) -> String {
    n.raw.as_ref().map(|r| r.to_string()).unwrap_or_else(|| n.value.to_string())
}

/// R19: i32로 **정확히** 표현되는 정수 리터럴만 받는다. 받는 값에 대해서는
/// 옛 `*value as i32`와 같은 값을 돌려준다(그 범위에서 `as`는 정확하다).
fn lit_i32(n: &Number) -> LResult<i32> {
    let v = n.value;
    if v.fract() == 0.0 && v >= i32::MIN as f64 && v <= i32::MAX as f64 {
        Ok(v as i32)
    } else {
        Err(Diagnostic::error(
            R19,
            n.span,
            format!(
                "정수 리터럴만 받습니다. `{}`는 i32 정수가 아닙니다(지금은 {}로 잘리거나 포화됩니다)",
                num_raw(n),
                v as i32
            ),
        ))
    }
}

/// R19(길이 비교): 오컴파일이 아니라 **표기 정규화**다 — 길이는 정수라
/// `len > 1.5`와 `len > 1`은 같은 뜻이고 포화도 같은 결과를 낸다. 그래서
/// "잘림" 문구를 쓰지 않는다(design-7a §1.1 R19).
fn lit_usize(n: &Number) -> LResult<usize> {
    let v = n.value;
    // `usize::MAX as f64`는 2^64로 반올림되므로 `<`로 비교한다.
    if v.fract() == 0.0 && v >= 0.0 && v < usize::MAX as f64 {
        Ok(v as usize)
    } else {
        Err(Diagnostic::error(
            R19,
            n.span,
            format!("길이 비교에는 0 이상의 정수 리터럴을 쓰십시오(`{}`)", num_raw(n)),
        ))
    }
}

/// R26: 인자가 `n`개보다 많으면 첫 초과 인자를 가리켜 거부한다.
fn check_arity(callee: &str, args: &[ExprOrSpread], n: usize) -> LResult<()> {
    match args.get(n) {
        None => Ok(()),
        Some(extra) => Err(Diagnostic::error(
            R26,
            extra.span(),
            format!(
                "`{callee}`는 인자 {n}개를 받습니다(받은 것 {}개, 초과분은 지금 무시됩니다)",
                args.len()
            ),
        )),
    }
}

/// lower가 IR 밖에 따로 돌려주는 소스 위치 표(design-7a §3.3). 모든 목록은
/// **소스 순서의 `Vec`**이다(§3.4 결정성 — 해시 순회 없음).
#[derive(Debug, Default, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct SpanTable {
    /// 컴포넌트 이름 → 함수 ident span.
    pub components: Vec<(String, Span)>,
    /// (컴포넌트, signal) → 선언 ident span.
    pub signals: Vec<(String, String, Span)>,
    /// `<C client:…/>` 참조 사이트: (참조하는 컴포넌트, 참조되는 컴포넌트, 태그 이름 span).
    pub component_refs: Vec<(String, String, Span)>,
    /// signal을 표시하는 binding 사이트.
    pub bindings: Vec<BindingSite>,
    /// 이벤트 attr(`onClick`/`onInput`) 사이트.
    pub events: Vec<EventSite>,
    /// 7a C3: IR 노드별 위치. `nodes[i]`는 `ComponentGraph::components[i]`의
    /// 루트 트리를 **pre-order**(자신 → `Element` 자식들 / `ListBinding` row /
    /// `CondBinding` then 가지; `ComponentRef`는 내려가지 않음)로 훑은 순서와
    /// 1:1이다. check.rs가 같은 순서로 IR을 훑어 짝짓는다(IR에 span을 넣지
    /// 않는다는 §3.3 결정을 지키면서 노드 단위 진단 위치를 얻는 방법).
    pub nodes: Vec<Vec<NodeSpan>>,
    /// 7a C6: 저작 표면 callee(`signal`/`sharedSignal`/`sharedValue`) 사용
    /// 사이트 — callee 식별자 span. W25(import 누락 경고)가 쓴다.
    pub loom_calls: Vec<(&'static str, Span)>,
}

/// IR 노드 하나의 소스 위치(7a C3).
#[derive(Debug, Clone, Copy)]
pub struct NodeSpan {
    /// 노드를 대표하는 짧은 구문: 엘리먼트·`<input>`·컴포넌트 참조는 태그 이름,
    /// `{s()}`·`{s() && …}`는 `s()`, `{xs().map(…)}`은 `xs()`, 정적 텍스트는
    /// 앞뒤 공백을 뺀 텍스트, `{item}`은 그 식별자, `sharedValue(…)`는 호출 전체.
    pub span: Span,
    /// `Element`에 `onClick`이 있으면 그 attr 전체 span.
    pub event: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum BindingKind {
    /// `{s()}`
    Text,
    /// `{s() && <X/>}`의 조건 signal
    Cond,
    /// `{s().map(...)}`
    List,
    /// `<input value={s()} onInput=…/>`
    Input,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct BindingSite {
    pub component: String,
    pub signal: String,
    pub kind: BindingKind,
    /// 리스트 row 템플릿 안의 사이트(row 원본)인지.
    pub in_row: bool,
    /// 사이트를 이루는 식(`s()`, `s() && …`의 `s()`, `s().map(...)` 전체, `<input>` 태그 이름).
    pub span: Span,
}

#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct EventSite {
    pub component: String,
    /// 핸들러가 쓰는 signal.
    pub signal: String,
    /// `"onClick"` 또는 `"onInput"`.
    pub attr: &'static str,
    /// attr 전체(`onClick={…}`) span.
    pub span: Span,
}

/// 한 컴포넌트 본문을 lower하는 동안 들고 다니는 문맥.
struct Cx<'a> {
    component: &'a str,
    signals: &'a [SignalDecl],
    known_components: &'a HashSet<String>,
    spans: &'a mut SpanTable,
    /// 7a C7(R44): 지금 lower 중인 노드를 감싸는, 이벤트를 받는 가장 가까운
    /// 요소의 `onClick` attr span.
    event_ancestor: Option<Span>,
}

impl Cx<'_> {
    fn find_signal(&self, name: &str) -> Option<&SignalDecl> {
        self.signals.iter().find(|s| s.name == name)
    }

    fn binding(&mut self, signal: &str, kind: BindingKind, row_var: Option<&str>, span: Span) {
        self.spans.bindings.push(BindingSite {
            component: self.component.to_string(),
            signal: signal.to_string(),
            kind,
            in_row: row_var.is_some(),
            span,
        });
    }

    /// 노드 하나를 pre-order로 기록하고 그 위치를 돌려준다(7a C3). 노드를
    /// 만드는 경로마다 **자식을 lower하기 전에** 정확히 한 번 부른다.
    fn node(&mut self, span: Span) -> usize {
        let list = self.spans.nodes.last_mut().expect("lower_component_fn opens a node list first");
        list.push(NodeSpan { span, event: None });
        list.len() - 1
    }

    fn node_event(&mut self, idx: usize, span: Span) {
        let list = self.spans.nodes.last_mut().expect("lower_component_fn opens a node list first");
        list[idx].event = Some(span);
    }

    fn event(&mut self, signal: &str, attr: &'static str, span: Span) {
        self.spans.events.push(EventSite {
            component: self.component.to_string(),
            signal: signal.to_string(),
            attr,
            span,
        });
    }
}

/// lower의 결과: IR, 위치 표, 그리고 **경고**(7a C6 — 지금은 W25만). 경고는
/// 산출물에 영향이 없다(공개 API가 `Compiled::warnings`로 돌려준다).
pub fn lower_module(module: &Module) -> LResult<(ComponentGraph, SpanTable, Vec<Diagnostic>)> {
    let (root_decl, helper_decls, imported) = collect_component_fn_decls(module)?;
    let root_name = root_decl.ident.sym.to_string();

    let known_components: HashSet<String> = helper_decls
        .iter()
        .map(|f| f.ident.sym.to_string())
        .chain(std::iter::once(root_name.clone()))
        .collect();

    let mut spans = SpanTable::default();
    let mut components = Vec::new();
    components.push(lower_component_fn(root_decl, &known_components, &mut spans)?);
    for decl in &helper_decls {
        components.push(lower_component_fn(decl, &known_components, &mut spans)?);
    }

    check_one_action_per_signal(&components, &spans)?;
    check_reachable(&components, &spans)?;
    check_binding_sites(&components, &spans)?;
    let warnings = missing_import_warnings(&imported, &spans);

    Ok((
        ComponentGraph {
            components,
            root: root_name,
        },
        spans,
        warnings,
    ))
}

/// W25: 쓴 callee가 `"loom"` import에 없으면 **callee 이름마다 1건**(사이트마다
/// 내지 않는다), 가장 앞선 사용 사이트를 가리킨다. 순회는 고정 배열
/// `LOOM_EXPORTS`와 `Vec`뿐이다(§3.4 결정성 — 해시 순회 없음).
fn missing_import_warnings(imported: &[String], spans: &SpanTable) -> Vec<Diagnostic> {
    LOOM_EXPORTS
        .iter()
        .filter(|name| !imported.iter().any(|i| i == *name))
        .filter_map(|name| {
            let first = spans
                .loom_calls
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, sp)| *sp)
                .min_by_key(|sp| sp.lo)?;
            Some(Diagnostic::warning(W25, first, format!("`{name}`를 \"loom\"에서 import하지 않았습니다")))
        })
        .collect()
}

/// Task #28-f — statically enforces the "signal당 액션 1개" (single-writer)
/// invariant `formal-verification/lean4/ResumabilitySoundness.lean`'s
/// `SingleWriter` precondition depends on. **Scoped to `I32` signals only**
/// (found while wiring this up — an earlier draft checked every signal kind
/// and immediately broke `demo/counter-list.tsx`'s legitimate
/// `items.append(...)`/`items.rotate()` pair, both targeting the same
/// `StringList` signal): the Lean file's own new `dom_recovered_
/// resumability_sound` theorem (Task #28-e) proves `Bool`/`StringList`/`Str`
/// resume is safe under *any* number of writers, since it reads the DOM's
/// current state directly rather than a captured literal snapshot — only
/// `I32`'s resume path (`format_i32_capture_for_action`'s captured `v0`)
/// actually needs `SingleWriter`. Enforcing single-writer beyond `I32` would
/// reject legitimate, already-safe programs for no soundness reason. Runs
/// once per component, over `Component::action_signal_names()` (already
/// existed, unused until this call site — ir.rs's doc comment on it) cross-
/// referenced against `component.signals` for each name's `SignalInit`.
///
/// (7a C2: 판정은 그대로 IR에서 하고, 위치만 `SpanTable`의 이벤트 사이트에서
/// 찾는다 — 주 span은 두 번째 핸들러, 첫 번째에 label.)
fn check_one_action_per_signal(components: &[Component], spans: &SpanTable) -> LResult<()> {
    for component in components {
        let mut seen = HashSet::new();
        for signal_name in component.action_signal_names() {
            let is_i32 = component
                .signals
                .iter()
                .any(|s| s.name == signal_name && matches!(s.init, crate::ir::SignalInit::I32(_)));
            if !is_i32 {
                continue;
            }
            if !seen.insert(signal_name.clone()) {
                let sites: Vec<Span> = spans
                    .events
                    .iter()
                    .filter(|e| e.component == component.name && e.signal == signal_name)
                    .map(|e| e.span)
                    .collect();
                let fallback = spans
                    .components
                    .iter()
                    .find(|(n, _)| *n == component.name)
                    .map(|(_, s)| *s)
                    .unwrap_or_default();
                let primary = sites.get(1).copied().unwrap_or(fallback);
                let mut d = Diagnostic::error(
                    L01,
                    primary,
                    format!(
                        "component `{}` has more than one action-bearing binding site targeting i32 signal `{signal_name}` — \
                         signal당 액션 1개(single-writer) 제약 위반. formal-verification/lean4/ResumabilitySoundness.lean의 \
                         SingleWriter 전제가 바로 이 불변식에 의존하므로 정적으로 거부한다 — 허용하면 resume 시점에 두 \
                         독립 인스턴스가 서로 다른 값으로 갈라질 수 있다(그 파일의 반례 스케치 참고). Bool/StringList/Str \
                         시그널은 DOM-복구형이라 이 제약이 필요 없다(dom_recovered_resumability_sound 참고) — 그래서 \
                         이 검사는 i32 시그널에만 적용된다.",
                        component.name
                    ),
                );
                if let Some(first) = sites.first() {
                    d = d.with_label(*first, format!("`{signal_name}`의 첫 번째 액션 사이트"));
                }
                return Err(d);
            }
        }
    }
    Ok(())
}

/// R47(7a C7): 루트에서 `ComponentRef`를 따라 도달할 수 없는 헬퍼는 오류다
/// (모드 무관). 지금까지 Flatten·단일 컴포넌트 경로에서는 조용히 컴파일되지
/// 않았고, Islands에서만 codegen.rs:620-627이 패닉으로 거부했다 — 이 검사가 그
/// 거부를 흡수한다. 도달성은 선언 순서의 `Vec<bool>`과 작업 스택으로 판정하고,
/// 첫 미도달 헬퍼(선언 순)를 가리킨다(§3.4 결정성 — 해시 순회 없음).
fn check_reachable(components: &[Component], spans: &SpanTable) -> LResult<()> {
    fn refs<'a>(node: &'a JsxNode, out: &mut Vec<&'a str>) {
        match node {
            JsxNode::ComponentRef { component, .. } => out.push(component),
            JsxNode::Element { children, .. } => children.iter().for_each(|c| refs(c, out)),
            JsxNode::ListBinding { row, .. } => refs(row, out),
            JsxNode::CondBinding { then_branch, .. } => refs(then_branch, out),
            JsxNode::StaticText(_)
            | JsxNode::TextBinding { .. }
            | JsxNode::RowItemText
            | JsxNode::InputBinding { .. }
            | JsxNode::SharedValueBinding { .. } => {}
        }
    }
    // `lower_module`은 루트를 먼저 lower한다 — components[0]이 루트다.
    let mut seen = vec![false; components.len()];
    let mut stack = vec![0usize];
    seen[0] = true;
    while let Some(i) = stack.pop() {
        let mut out = Vec::new();
        refs(&components[i].root, &mut out);
        for name in out {
            for (j, c) in components.iter().enumerate() {
                if c.name == name && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
    }
    match seen.iter().position(|s| !s) {
        None => Ok(()),
        Some(j) => Err(Diagnostic::error(
            R47,
            spans.components.get(j).map(|(_, s)| *s).unwrap_or_default(),
            format!(
                "헬퍼 `{}`가 루트에서 도달할 수 없습니다(Flatten에서는 조용히 컴파일되지 않습니다)",
                components[j].name
            ),
        )),
    }
}

/// R07/R07U(7a C7): signal마다 binding 사이트(`SpanTable::bindings` — Text,
/// Cond의 조건, List, Input, row 템플릿 안의 cond/list 원본)를 센다. codegen은
/// 사이트마다 `let {s}_entity = …`를 방출하고 `Signals`는 마지막 것을 가리키므로,
/// 2곳 이상이면 앞 사이트들은 갱신되지 않는다(R07). 0곳이면 codegen.rs:1271의
/// "never spawned" 내부 오류에 닿는다(R07U — 사용자 오류로 승격). 순회는 컴포넌트·
/// signal 선언 순서와 `Vec`뿐이다(§3.4).
fn check_binding_sites(components: &[Component], spans: &SpanTable) -> LResult<()> {
    for component in components {
        for signal in &component.signals {
            let mut sites: Vec<Span> = spans
                .bindings
                .iter()
                .filter(|b| b.component == component.name && b.signal == signal.name)
                .map(|b| b.span)
                .collect();
            sites.sort_by_key(|s| s.lo);
            let decl = spans
                .signals
                .iter()
                .find(|(c, n, _)| *c == component.name && *n == signal.name)
                .map(|(_, _, s)| *s)
                .unwrap_or_default();
            match sites.len() {
                0 => {
                    return Err(Diagnostic::error(
                        R07U,
                        decl,
                        format!("signal `{}`가 선언됐지만 표시되는 곳이 없습니다", signal.name),
                    ))
                }
                1 => {}
                n => {
                    let mut d = Diagnostic::error(
                        R07,
                        sites[0],
                        format!(
                            "signal `{}`가 binding 사이트 {n}곳에 쓰였습니다. 지금은 첫 사이트가 갱신되지 않습니다(사이트별 entity 미구현)",
                            signal.name
                        ),
                    );
                    for (k, site) in sites.iter().enumerate() {
                        d = d.with_label(*site, format!("binding 사이트 {}/{n}", k + 1));
                    }
                    return Err(d.with_label(decl, "선언"));
                }
            }
        }
    }
    Ok(())
}

/// Replaces the Phase 1 bug where only the first exported function was kept
/// and every other top-level declaration was silently discarded with no
/// diagnostic. Scans every top-level `function` declaration in the module:
/// exactly one *exported* function is the graph's root (Phase 1's original
/// restriction — kept here rather than relaxed, since allowing multiple
/// independent exported roots per module is a different, unrelated
/// generalization no current fixture/task needs); every other, non-exported
/// `function` is a candidate helper component, only actually meaningful if
/// some `ComponentRef` (transitively reachable from the root) names it —
/// unreferenced helpers are accepted, not lint-rejected (this narrow
/// compiler doesn't dead-code-eliminate; a stated scope cut, not an
/// oversight).
///
/// (2026-10-17 정정, 7a C7: 위 "unreferenced helpers are accepted"는 더 이상
/// 참이 아니다 — 루트에서 도달할 수 없는 헬퍼는 `check_reachable`이 `LOOM-R47`로
/// 거부한다. Flatten·단일 컴포넌트 경로에서 그런 헬퍼가 진단 없이 컴파일되지
/// 않던 것이 docs/09 R47이다.)
///
/// 7a C6(R25): 옛 `_ => {}`(그 밖의 최상위 항목을 진단 없이 버림)를 명시적
/// match로 바꿨다. 받는 것은 `import {…} from "loom"`(이름 지정자만, 별칭 없이,
/// `LOOM_EXPORTS`의 부분집합), `function`, `export function`뿐이고 나머지는
/// `LOOM-R25` 오류다. 셋째 반환값은 import한 이름 목록(W25용, 소스 순서).
fn collect_component_fn_decls(module: &Module) -> LResult<(&FnDecl, Vec<&FnDecl>, Vec<String>)> {
    let mut root: Option<&FnDecl> = None;
    let mut helpers = Vec::new();
    let mut imported = Vec::new();
    for item in &module.body {
        match item {
            ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(ExportDecl { decl: Decl::Fn(f), .. })) => {
                if let Some(first) = root {
                    return Err(unsupported(
                        f.ident.span,
                        format!(
                            "more than one exported `function` found — Phase 1/증분 4 both require exactly one exported root component, got a second: `{}`",
                            f.ident.sym
                        ),
                    )
                    .with_label(first.ident.span, "첫 번째 export 함수"));
                }
                root = Some(f);
            }
            ModuleItem::Stmt(Stmt::Decl(Decl::Fn(f))) => {
                helpers.push(f);
            }
            ModuleItem::ModuleDecl(ModuleDecl::Import(import)) => {
                imported.extend(lower_loom_import(import)?);
            }
            // 2026-10-18 7a 후속 F2(병익님: R25 완화): 빈 문장 `;`은 의미가 없으므로 받는다
            // (7a C6은 이것도 R25로 거부했다 — 막는 결함이 없는 거부였다).
            ModuleItem::Stmt(Stmt::Empty(_)) => {}
            other => {
                return Err(Diagnostic::error(
                    R25,
                    other.span(),
                    format!(
                        "최상위 {}는 지원하지 않습니다(지금은 조용히 무시됩니다). 최상위에는 `import {{…}} from \"loom\"`, `function`, `export function`만 쓸 수 있습니다",
                        top_level_kind(other)
                    ),
                ))
            }
        }
    }
    let root = root.ok_or_else(|| {
        unsupported(module.span.shrink_to_lo(), "no exported `function NAME() { ... }` found")
    })?;
    Ok((root, helpers, imported))
}

/// R25: `import { signal, … } from "loom"`만 받는다. 이름 지정자의 로컬 이름을
/// 돌려준다(별칭은 거부 — lower는 callee를 이름으로 찾으므로 별칭은 조용히
/// 다른 뜻이 된다).
fn lower_loom_import(import: &ImportDecl) -> LResult<Vec<String>> {
    let r25 = |span: Span, what: String| {
        Diagnostic::error(
            R25,
            span,
            format!("{what}는 지원하지 않습니다(지금은 조용히 무시됩니다). `import {{ signal, sharedSignal, sharedValue }} from \"loom\"`의 부분집합만 받습니다"),
        )
    };
    let src = import.src.value.to_string_lossy();
    if src != "loom" {
        return Err(r25(import.src.span, format!("최상위 `import … from \"{src}\"`")));
    }
    if import.type_only {
        return Err(r25(import.span, "최상위 `import type`".to_string()));
    }
    let mut names = Vec::new();
    for spec in &import.specifiers {
        match spec {
            ImportSpecifier::Named(ImportNamedSpecifier {
                local,
                imported: None,
                is_type_only: false,
                ..
            }) if LOOM_EXPORTS.contains(&local.sym.as_ref()) => names.push(local.sym.to_string()),
            ImportSpecifier::Named(n) if n.imported.is_some() => {
                return Err(r25(n.span, "import 별칭(`… as …`)".to_string()))
            }
            ImportSpecifier::Named(n) if n.is_type_only => {
                return Err(r25(n.span, "`type` import 지정자".to_string()))
            }
            ImportSpecifier::Named(n) => {
                return Err(r25(n.span, format!("\"loom\"의 `{}` import", n.local.sym)))
            }
            ImportSpecifier::Default(d) => return Err(r25(d.span, "default import".to_string())),
            ImportSpecifier::Namespace(ns) => return Err(r25(ns.span, "namespace import(`* as …`)".to_string())),
        }
    }
    Ok(names)
}

/// R25 진단 문구용 최상위 항목 종류.
fn top_level_kind(item: &ModuleItem) -> &'static str {
    match item {
        ModuleItem::ModuleDecl(decl) => match decl {
            ModuleDecl::ExportDecl(ExportDecl { decl, .. }) => match decl {
                Decl::Var(v) => match v.kind {
                    VarDeclKind::Const => "`export const`",
                    VarDeclKind::Let => "`export let`",
                    VarDeclKind::Var => "`export var`",
                },
                Decl::Class(_) => "`export class`",
                Decl::TsTypeAlias(_) => "`export type`",
                Decl::TsInterface(_) => "`export interface`",
                Decl::TsEnum(_) => "`export enum`",
                _ => "`export` 선언",
            },
            ModuleDecl::ExportDefaultDecl(_) | ModuleDecl::ExportDefaultExpr(_) => "`export default`",
            ModuleDecl::ExportNamed(_) | ModuleDecl::ExportAll(_) => "재export(`export {…}`/`export * from`)",
            ModuleDecl::Import(_) => "`import`",
            _ => "TypeScript 모듈 선언",
        },
        ModuleItem::Stmt(stmt) => match stmt {
            Stmt::Decl(Decl::Var(v)) => match v.kind {
                VarDeclKind::Const => "`const` 선언",
                VarDeclKind::Let => "`let` 선언",
                VarDeclKind::Var => "`var` 선언",
            },
            Stmt::Decl(Decl::Class(_)) => "`class`",
            Stmt::Decl(Decl::TsTypeAlias(_)) => "`type` 별칭",
            Stmt::Decl(Decl::TsInterface(_)) => "`interface`",
            Stmt::Decl(Decl::TsEnum(_)) => "`enum`",
            Stmt::Decl(_) => "선언",
            Stmt::Empty(_) => "빈 문장(`;`)",
            _ => "문장",
        },
    }
}

fn lower_component_fn(
    fn_decl: &FnDecl,
    known_components: &HashSet<String>,
    spans: &mut SpanTable,
) -> LResult<Component> {
    let component_name = fn_decl.ident.sym.to_string();
    spans.components.push((component_name.clone(), fn_decl.ident.span));
    spans.nodes.push(Vec::new());
    check_component_name(&component_name, fn_decl.ident.span)?;
    // R01(매개변수): props는 아직 전달되지 않는다 — 매개변수를 받아 두면 저작자는
    // 값이 온다고 믿지만 실제로는 아무것도 오지 않는다.
    if let Some(first) = fn_decl.function.params.first() {
        return Err(Diagnostic::error(
            R01,
            first.span,
            format!("컴포넌트 함수 `{component_name}`는 매개변수를 받을 수 없습니다(props 미지원, 7d)"),
        ));
    }
    // R27: async/generator/타입 매개변수 컴포넌트는 지금 그 성질이 조용히 무시된다.
    if fn_decl.function.is_async {
        return Err(r27_async(fn_decl.function.span, "컴포넌트"));
    }
    if fn_decl.function.is_generator {
        return Err(Diagnostic::error(R27, fn_decl.ident.span, "generator(`function*`) 컴포넌트는 지원하지 않습니다"));
    }
    if let Some(tp) = &fn_decl.function.type_params {
        return Err(r27_type_params(tp.span));
    }
    let body = fn_decl
        .function
        .body
        .as_ref()
        .ok_or_else(|| unsupported(fn_decl.ident.span, "component function has no body"))?;

    let mut signals = Vec::new();
    let mut shared_bindings = Vec::new();
    let mut root = None;

    for stmt in &body.stmts {
        // R22: 첫 `return`만 루트가 될 수 있다(옛 동작은 마지막 `return`이 루트,
        // 그 사이 선언도 조용히 받아 줬다).
        // (2026-10-18 F2: 빈 문장 `;`은 return 뒤에 와도 의미가 없으므로 R22 대상이 아니다.)
        if root.is_some() && !matches!(stmt, Stmt::Empty(_)) {
            return Err(Diagnostic::error(
                R22,
                stmt.span(),
                "`return` 뒤에 문장이 있습니다. 첫 `return`만 루트가 될 수 있습니다(지금은 마지막 것이 쓰입니다)",
            ));
        }
        match stmt {
            Stmt::Decl(Decl::Var(var_decl)) => {
                if var_decl.kind != VarDeclKind::Const {
                    let kw = if var_decl.kind == VarDeclKind::Let { "let" } else { "var" };
                    return Err(Diagnostic::error(
                        R27,
                        Span::new(var_decl.span.lo, var_decl.span.lo + BytePos(kw.len() as u32)),
                        format!("`{kw}` 대신 `const`를 쓰십시오(signal 핸들은 재할당되지 않습니다)"),
                    ));
                }
                for decl in &var_decl.decls {
                    if let Some((signal, shared)) = try_lower_shared_signal_decl(decl)? {
                        check_signal_name(&signal.name, decl.name.span())?;
                        spans.signals.push((component_name.clone(), signal.name.clone(), decl.name.span()));
                        spans.loom_calls.push(("sharedSignal", callee_span(decl)));
                        signals.push(signal);
                        shared_bindings.push(shared);
                    } else if let Some(signal) = try_lower_signal_decl(decl)? {
                        check_signal_name(&signal.name, decl.name.span())?;
                        spans.signals.push((component_name.clone(), signal.name.clone(), decl.name.span()));
                        spans.loom_calls.push(("signal", callee_span(decl)));
                        signals.push(signal);
                    } else {
                        return Err(unsupported(
                            decl.span,
                            "unsupported `const` declaration in component body (only `const x = signal(...)`/`const x = sharedSignal(\"key\", ...)` is supported)",
                        ));
                    }
                }
            }
            Stmt::Return(ReturnStmt { arg: Some(arg), .. }) => {
                let jsx = unwrap_paren(arg);
                let Expr::JSXElement(el) = jsx else {
                    return Err(unsupported(jsx.span(), "component must `return` a single JSX element"));
                };
                // (7a C3의 "return이 둘이면 노드 표를 비움" 처리는 C6의 R22
                // 거부로 필요 없어져 걷었다 — 루트는 이제 항상 첫 `return`이다.)
                let mut cx = Cx {
                    component: &component_name,
                    signals: &signals,
                    known_components,
                    spans,
                    event_ancestor: None,
                };
                root = Some(lower_element(el, &mut cx, None)?);
            }
            // 2026-10-18 7a 후속 F2: 컴포넌트 본문의 빈 문장 `;`도 의미가 없으므로 받는다.
            Stmt::Empty(_) => {}
            other => return Err(unsupported(other.span(), "unsupported statement in component body")),
        }
    }

    Ok(Component {
        name: component_name,
        signals,
        shared_bindings,
        root: root.ok_or_else(|| unsupported(fn_decl.ident.span, "component has no `return` statement"))?,
    })
}

/// `const cartCount = sharedSignal("cartCount", 0)` — tried *before*
/// `try_lower_signal_decl` (matches the callee name `sharedSignal` instead
/// of `signal`), producing both a normal `SignalDecl` (the writer role
/// reuses every existing local-signal codegen path unchanged) and a
/// `SharedBinding` recording that this signal should also be published to
/// `__loomShared`. `Ok(None)` (not an error) if the callee isn't
/// `sharedSignal` at all, so `lower_component_fn` can fall through to the
/// ordinary `signal(...)` case.
fn try_lower_shared_signal_decl(decl: &VarDeclarator) -> LResult<Option<(SignalDecl, SharedBinding)>> {
    let Pat::Ident(BindingIdent { id, .. }) = &decl.name else {
        return Ok(None);
    };
    let Some(init) = decl.init.as_deref() else {
        return Ok(None);
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = init
    else {
        return Ok(None);
    };
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        return Ok(None);
    };
    if sym != "sharedSignal" {
        return Ok(None);
    }
    let [key_arg, initial_arg] = args.as_slice() else {
        return Err(unsupported(
            *call_span,
            "sharedSignal() requires exactly two arguments, e.g. sharedSignal(\"cartCount\", 0)",
        ));
    };
    let Expr::Lit(Lit::Str(key)) = unwrap_paren(&key_arg.expr) else {
        return Err(unsupported(
            key_arg.expr.span(),
            "sharedSignal() requires a string key and an integer initial value, e.g. sharedSignal(\"cartCount\", 0)",
        ));
    };
    let Expr::Lit(Lit::Num(initial)) = unwrap_paren(&initial_arg.expr) else {
        return Err(unsupported(
            initial_arg.expr.span(),
            "sharedSignal() requires a string key and an integer initial value, e.g. sharedSignal(\"cartCount\", 0)",
        ));
    };
    let initial = lit_i32(initial)?;
    let name = id.sym.to_string();
    let key = key.value.to_string_lossy().into_owned();
    Ok(Some((
        SignalDecl {
            name: name.clone(),
            init: SignalInit::I32(initial),
        },
        SharedBinding { signal: name, key },
    )))
}

/// `const x = signal(…)`/`sharedSignal(…)` 선언의 callee 식별자 span(W25용).
/// `try_lower_*_signal_decl`이 `Some`을 돌려준 선언에만 부른다.
fn callee_span(decl: &VarDeclarator) -> Span {
    match decl.init.as_deref() {
        Some(Expr::Call(CallExpr { callee: Callee::Expr(callee), .. })) => callee.span(),
        _ => decl.span,
    }
}

/// R27: `async` 함수·화살표. 주 span은 `async` 키워드(함수·화살표 span의 앞 5바이트).
fn r27_async(fn_span: Span, what: &str) -> Diagnostic {
    Diagnostic::error(
        R27,
        Span::new(fn_span.lo, fn_span.lo + BytePos(5)),
        format!("async {what}는 지원하지 않습니다(지금은 `async`가 조용히 무시됩니다)"),
    )
}

/// R27: 타입 매개변수(`<T>`).
fn r27_type_params(span: Span) -> Diagnostic {
    Diagnostic::error(R27, span, "타입 매개변수는 지원하지 않습니다")
}

/// R27(화살표): `onClick`/`onInput`/`.map(...)`의 화살표가 async·generator이거나
/// 타입 매개변수를 가지면 거부한다. 반환 타입 주석은 의미가 없으므로 받아 준다.
fn check_arrow(arrow: &ArrowExpr, what: &str) -> LResult<()> {
    if arrow.is_async {
        return Err(r27_async(arrow.span, what));
    }
    if arrow.is_generator {
        return Err(Diagnostic::error(R27, arrow.span, format!("generator {what}는 지원하지 않습니다")));
    }
    if let Some(tp) = &arrow.type_params {
        return Err(r27_type_params(tp.span));
    }
    Ok(())
}

fn unwrap_paren(expr: &Expr) -> &Expr {
    match expr {
        Expr::Paren(ParenExpr { expr, .. }) => unwrap_paren(expr),
        other => other,
    }
}

/// 배열 리터럴의 원소를 문자열 리터럴로만 받는다(`signal([...])`와 prop 공용).
fn lower_string_array(elems: &[Option<ExprOrSpread>], array_span: Span, what: &str) -> LResult<Vec<String>> {
    elems
        .iter()
        .map(|e| {
            let e = e
                .as_ref()
                .ok_or_else(|| unsupported(array_span, "sparse array literals unsupported"))?;
            match unwrap_paren(&e.expr) {
                Expr::Lit(Lit::Str(s)) => Ok(s.value.to_string_lossy().into_owned()),
                other => Err(unsupported(other.span(), format!("{what} array elements must be string literals"))),
            }
        })
        .collect()
}

fn try_lower_signal_decl(decl: &VarDeclarator) -> LResult<Option<SignalDecl>> {
    let Pat::Ident(BindingIdent { id, .. }) = &decl.name else {
        return Ok(None);
    };
    let Some(init) = decl.init.as_deref() else {
        return Ok(None);
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = init
    else {
        return Ok(None);
    };
    let Expr::Ident(Ident { sym, .. }) = callee.as_ref() else {
        return Ok(None);
    };
    if sym != "signal" {
        return Ok(None);
    }
    let arg = args
        .first()
        .ok_or_else(|| unsupported(*call_span, "signal() requires one argument"))?;
    check_arity("signal()", args, 1)?;
    let init = match unwrap_paren(&arg.expr) {
        Expr::Lit(Lit::Num(n)) => SignalInit::I32(lit_i32(n)?),
        Expr::Lit(Lit::Bool(b)) => SignalInit::Bool(b.value),
        Expr::Lit(Lit::Str(s)) => SignalInit::Str(s.value.to_string_lossy().into_owned()),
        Expr::Array(ArrayLit { elems, span }) => {
            SignalInit::StringList(lower_string_array(elems, *span, "signal([...])")?)
        }
        other => {
            return Err(unsupported(
                other.span(),
                "signal(...) initializer must be a number, boolean, string, or string-array literal",
            ))
        }
    };
    Ok(Some(SignalDecl {
        name: id.sym.to_string(),
        init,
    }))
}

/// `row_var`: `Some(name)` while lowering a list row template, so a bare
/// `{name}` reference resolves to `JsxNode::RowItemText` instead of being
/// looked up as a top-level signal. `cx.known_components`: every component
/// name declared anywhere in this module (root + helpers) — used to decide
/// whether an uppercase-leading tag is a real component reference (and to
/// give a clear error if it names nothing declared) rather than treating
/// every tag as a bare HTML string.
fn lower_element(el: &JSXElement, cx: &mut Cx<'_>, row_var: Option<&str>) -> LResult<JsxNode> {
    let JSXElementName::Ident(Ident { sym: tag, span: tag_span, .. }) = &el.opening.name else {
        return Err(unsupported(el.opening.name.span(), "only plain tag names are supported"));
    };
    let tag = tag.to_string();
    let node_idx = cx.node(*tag_span);

    if tag.chars().next().is_some_and(|c| c.is_uppercase()) {
        if !cx.known_components.contains(&tag) {
            return Err(unsupported(
                *tag_span,
                format!(
                    "`<{tag}/>` looks like a component reference (uppercase-leading name) but no `function {tag}()` is declared in this module"
                ),
            ));
        }
        if let Some(outer) = cx.event_ancestor {
            return Err(r44(*tag_span, outer, &format!("컴포넌트 참조 `<{tag}>`")));
        }
        let node = lower_component_ref(el, &tag)?;
        cx.spans
            .component_refs
            .push((cx.component.to_string(), tag.clone(), *tag_span));
        return Ok(node);
    }

    // R24: 소문자 태그는 언제나 HTML 태그로 해석된다 — 같은 이름의 컴포넌트
    // 함수가 있으면 저작자는 그것을 부른다고 믿지만 조용히 빈 HTML 요소가 된다.
    if cx.known_components.contains(&tag) {
        return Err(Diagnostic::error(
            R24,
            *tag_span,
            format!(
                "`<{tag}/>`는 HTML 태그로 해석되는데 `function {tag}()`가 선언돼 있습니다. 컴포넌트 이름은 대문자로 시작해야 합니다"
            ),
        ));
    }

    if tag == "input" {
        let node = lower_input_binding(el, cx)?;
        if let JsxNode::InputBinding { signal } = &node {
            cx.binding(signal, BindingKind::Input, row_var, *tag_span);
        }
        return Ok(node);
    }

    let mut class = None;
    let mut action = None;
    // R05/R06: 앞선 출현의 attr span(진단 label용).
    let mut class_at: Option<Span> = None;
    let mut onclick_at: Option<Span> = None;
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            return Err(unsupported(attr.span(), "spread attributes are not supported"));
        };
        let JSXAttrName::Ident(IdentName { sym: name, .. }) = &attr.name else {
            return Err(unsupported(attr.name.span(), "namespaced attributes are not supported"));
        };
        match name.as_ref() {
            "class" | "className" => {
                if let Some(first) = class_at {
                    return Err(duplicate_attr(&tag, name, attr.span, first));
                }
                class_at = Some(attr.span);
                let Some(JSXAttrValue::Str(s)) = &attr.value else {
                    return Err(unsupported(attr.span, "`class` must be a static string literal"));
                };
                class = Some(s.value.to_string_lossy().into_owned());
            }
            "onClick" => {
                if let Some(first) = onclick_at {
                    return Err(Diagnostic::error(
                        R05,
                        attr.span,
                        format!("<{tag}>에 `onClick`이 두 번 있습니다. 앞 핸들러가 버려지고 single-writer 검사도 우회됩니다"),
                    )
                    .with_label(first, "첫 번째 `onClick`"));
                }
                onclick_at = Some(attr.span);
                if let Some(outer) = cx.event_ancestor {
                    return Err(r44(attr.span, outer, &format!("<{tag}>의 `onClick`")));
                }
                let a = lower_click_action(attr, cx)?;
                cx.event(a.signal_name(), "onClick", attr.span);
                cx.node_event(node_idx, attr.span);
                action = Some(a);
            }
            other => {
                return Err(unsupported(
                    attr.name.span(),
                    format!("unsupported attribute `{other}` on <{tag}>"),
                ))
            }
        }
    }

    let mut children = Vec::new();
    let outer_event = cx.event_ancestor;
    if onclick_at.is_some() {
        cx.event_ancestor = onclick_at;
    }
    for child in &el.children {
        if let Some(node) = lower_child(child, cx, row_var)? {
            children.push(node);
        }
    }
    cx.event_ancestor = outer_event;

    Ok(JsxNode::Element {
        tag,
        class,
        action,
        children,
    })
}

/// R44: 이벤트를 받는 요소 안의 이벤트 수신자. 주 span은 안쪽, 바깥 `onClick`에 label.
fn r44(inner: Span, outer: Span, what: &str) -> Diagnostic {
    Diagnostic::error(
        R44,
        inner,
        format!(
            "이벤트를 받는 요소 안에 또 이벤트를 받는 요소가 있습니다({what}). 진입점마다 의미가 다릅니다(bootstrap은 가장 안쪽 하나만, Island 진입점은 버블링으로 둘 다)"
        ),
    )
    .with_label(outer, "바깥 `onClick`")
}

/// R06: 같은 attr가 두 번(`class`/`className`은 같은 attr로 센다). 주 span은
/// 뒤 attr, 첫 출현에 label.
fn duplicate_attr(tag: &str, attr: &str, span: Span, first: Span) -> Diagnostic {
    Diagnostic::error(R06, span, format!("<{tag}>에 `{attr}`가 두 번 있습니다(뒤의 것만 남습니다)"))
        .with_label(first, "첫 번째 출현")
}

/// Parses `<ComponentName client:load|client:visible prop={literal} .../>`
/// into `JsxNode::ComponentRef`. This is the **only** place `client:`-
/// namespaced attributes are accepted — plain `Element`/`InputBinding`
/// still reject every namespaced attribute unchanged (their own errors,
/// untouched by this function).
fn lower_component_ref(el: &JSXElement, name: &str) -> LResult<JsxNode> {
    let mut activation = None;
    // R18: 첫 디렉티브의 (원문 이름, attr 이름 span) — 둘째 디렉티브 진단의 label용.
    let mut first_directive: Option<(String, Span)> = None;
    // 7a C6: prop은 R01로 모두 거부되므로 IR의 `props`는 언제나 비어 있다
    // (IR 모양은 바꾸지 않는다 — 7d에서 props를 설계할 때 다시 채운다).
    let props = Vec::new();
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            return Err(unsupported(attr.span(), "spread attributes are not supported"));
        };
        match &attr.name {
            JSXAttrName::JSXNamespacedName(JSXNamespacedName {
                ns, name: attr_name, ..
            }) => {
                if ns.sym.as_ref() != "client" {
                    return Err(unsupported(
                        ns.span,
                        format!(
                            "unsupported attribute namespace `{}:` on <{name}> (only `client:` is supported)",
                            ns.sym
                        ),
                    ));
                }
                let parsed = match attr_name.sym.as_ref() {
                    "load" => IslandActivation::Load,
                    "visible" => IslandActivation::Visible,
                    other => {
                        return Err(unsupported(
                            attr.name.span(),
                            format!(
                                "unsupported `client:{other}` directive on <{name}> (supported: client:load, client:visible)"
                            ),
                        ))
                    }
                };
                let directive = format!("client:{}", attr_name.sym);
                if let Some(value) = &attr.value {
                    return Err(Diagnostic::error(
                        R18,
                        value.span(),
                        format!("`{directive}`는 값을 받지 않습니다(지금은 값이 무시됩니다)"),
                    ));
                }
                if let Some((first, first_span)) = &first_directive {
                    return Err(Diagnostic::error(
                        R18,
                        attr.name.span(),
                        format!("<{name}>에 디렉티브가 둘입니다(`{first}` + `{directive}`). 지금은 뒤의 것만 쓰입니다"),
                    )
                    .with_label(*first_span, format!("첫 디렉티브 `{first}`")));
                }
                first_directive = Some((directive, attr.name.span()));
                activation = Some(parsed);
            }
            JSXAttrName::Ident(IdentName { sym: prop_name, .. }) => {
                let value = attr.value.as_ref().ok_or_else(|| {
                    unsupported(attr.span, format!("prop `{prop_name}` on <{name}> requires a value"))
                })?;
                // R01(prop): 기존 리터럴 검사를 **먼저** 돌린다 — 비리터럴 prop은
                // 옛 문구 그대로 거부된다(lib.rs M10 증거 테스트의 관측 메커니즘
                // 유지, design-7a §1.1 R1a). 리터럴이면 그다음에 R01이다.
                lower_prop_literal(value)?;
                return Err(Diagnostic::error(
                    R01,
                    attr.span,
                    format!(
                        "컴포넌트 prop `{prop_name}`는 아직 전달되지 않습니다. 지금은 값이 버려지고 선언 초기값이 쓰입니다. props는 CAP 조건 (2)(docs/02:199-205)와 얽혀 있어 7d에서 RQ를 먼저 세웁니다"
                    ),
                ));
            }
        }
    }
    // R02: children은 전달되지 않고 SSR에서도 사라진다. 공백뿐인 JSXText 말고는
    // (`{/* 주석 */}` 포함) 모두 거부한다 — 일반 요소의 JSXEmptyExpr 거부와 같은 경계.
    if let Some(child) = el
        .children
        .iter()
        .find(|c| !matches!(c, JSXElementChild::JSXText(t) if t.value.trim().is_empty()))
    {
        return Err(Diagnostic::error(
            R02,
            child.span(),
            format!(
                "`<{name} client:…>`의 children은 전달되지 않고 SSR에서도 사라집니다. `<{name} client:load />`처럼 자기 닫힘으로 쓰십시오(슬롯은 7d)"
            ),
        ));
    }
    let activation = activation.ok_or_else(|| {
        unsupported(
            el.opening.name.span(),
            format!(
                "<{name}/> has no `client:load`/`client:visible` directive — 증분 4 scope: ordinary (non-Island) component references are not yet supported, every ComponentRef must currently carry an activation"
            ),
        )
    })?;
    Ok(JsxNode::ComponentRef {
        component: name.to_string(),
        activation,
        props,
    })
}

/// A prop literal must be exactly one of the same shapes `signal(...)`'s own
/// initializer already allows (see `try_lower_signal_decl`) — Phase 1's "no
/// non-literal initial values anywhere" restriction, extended rather than
/// broken (see `ir::PropLiteral`'s doc comment).
fn lower_prop_literal(value: &JSXAttrValue) -> LResult<PropLiteral> {
    match value {
        JSXAttrValue::Str(s) => Ok(PropLiteral::Str(s.value.to_string_lossy().into_owned())),
        JSXAttrValue::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => match unwrap_paren(expr) {
            Expr::Lit(Lit::Num(n)) => Ok(PropLiteral::I32(lit_i32(n)?)),
            Expr::Lit(Lit::Bool(b)) => Ok(PropLiteral::Bool(b.value)),
            Expr::Lit(Lit::Str(s)) => Ok(PropLiteral::Str(s.value.to_string_lossy().into_owned())),
            Expr::Array(ArrayLit { elems, span }) => {
                Ok(PropLiteral::StringList(lower_string_array(elems, *span, "prop")?))
            }
            other => Err(unsupported(
                other.span(),
                "component prop value must be a number, boolean, string, or string-array literal",
            )),
        },
        other => Err(unsupported(other.span(), "unsupported component prop value shape")),
    }
}

/// `<input value={s()} onInput={(e) => s.set(e.target.value)}/>` — Phase 1's
/// two-way text binding. Handled as its own `JsxNode` (not a generic
/// `Element`, which has no slot for "this element's value and its update
/// handler are the same binding site") — the same reason `{cond() && <X/>}`
/// got its own `JsxNode::CondBinding` rather than riding on `Element`.
fn lower_input_binding(el: &JSXElement, cx: &mut Cx<'_>) -> LResult<JsxNode> {
    let mut value_signal: Option<(String, Span)> = None;
    let mut oninput_signal: Option<(String, Span)> = None;
    for attr in &el.opening.attrs {
        let JSXAttrOrSpread::JSXAttr(attr) = attr else {
            return Err(unsupported(attr.span(), "spread attributes are not supported"));
        };
        let JSXAttrName::Ident(IdentName { sym: name, .. }) = &attr.name else {
            return Err(unsupported(attr.name.span(), "namespaced attributes are not supported"));
        };
        // R06: 같은 attr가 두 번이면 지금은 뒤의 것만 남는다.
        let earlier = match name.as_ref() {
            "value" => value_signal.as_ref(),
            "onInput" => oninput_signal.as_ref(),
            _ => None,
        };
        if let Some((_, first)) = earlier {
            return Err(duplicate_attr("input", name, attr.span, *first));
        }
        match name.as_ref() {
            "value" => value_signal = Some((lower_input_value_attr(attr, cx)?, attr.span)),
            "onInput" => oninput_signal = Some((lower_input_oninput_attr(attr, cx)?, attr.span)),
            other => {
                return Err(unsupported(
                    attr.name.span(),
                    format!(
                        "<input> only supports `value`/`onInput` (Phase 1 scope: two-way text binding only), got `{other}`"
                    ),
                ))
            }
        }
    }
    let input_span = el.opening.name.span();
    let (value_signal, value_span) = value_signal
        .ok_or_else(|| unsupported(input_span, "<input> requires a `value={signalName()}` attribute"))?;
    let (oninput_signal, oninput_span) = oninput_signal.ok_or_else(|| {
        unsupported(input_span, "<input> requires an `onInput={(e) => signalName.set(e.target.value)}` attribute")
    })?;
    if value_signal != oninput_signal {
        return Err(unsupported(
            oninput_span,
            format!(
                "<input>'s `value` and `onInput` must reference the same signal, got `{value_signal}` and `{oninput_signal}`"
            ),
        )
        .with_label(value_span, format!("`value`는 `{value_signal}`")));
    }
    // R26: `<input …>x</input>`의 자식은 지금 조용히 사라진다. 공백뿐인 JSXText
    // 말고는(`{/* 주석 */}` 포함) 모두 거부한다 — R02와 같은 경계.
    if let Some(child) = el.children.iter().find(|c| !matches!(c, JSXElementChild::JSXText(t) if t.value.trim().is_empty())) {
        return Err(Diagnostic::error(R26, child.span(), "<input>은 자식을 가질 수 없습니다(지금은 자식이 조용히 사라집니다)"));
    }
    if let Some(outer) = cx.event_ancestor {
        return Err(r44(oninput_span, outer, "<input>의 `onInput`"));
    }
    cx.event(&oninput_signal, "onInput", oninput_span);
    Ok(JsxNode::InputBinding { signal: value_signal })
}

fn lower_input_value_attr(attr: &JSXAttr, cx: &Cx<'_>) -> LResult<String> {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        return Err(unsupported(attr.span, "<input>'s `value` must be `{signalName()}`"));
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        ..
    }) = unwrap_paren(expr)
    else {
        return Err(unsupported(expr.span(), "<input>'s `value` must be a bare `signalName()` call"));
    };
    if let Some(first) = args.first() {
        return Err(unsupported(
            first.expr.span(),
            "`signalName()` in <input>'s `value` must take no arguments",
        ));
    }
    let Expr::Ident(Ident { sym, span: name_span, .. }) = callee.as_ref() else {
        return Err(unsupported(callee.span(), "<input>'s `value` must call a bare signal identifier"));
    };
    let name = sym.to_string();
    let signal = cx
        .find_signal(&name)
        .ok_or_else(|| unsupported(*name_span, format!("<input>'s `value` references unknown signal `{name}`")))?;
    if !matches!(signal.init, SignalInit::Str(_)) {
        return Err(unsupported(
            *name_span,
            format!("<input>'s `value` requires a Str signal, `{name}` is not one"),
        ));
    }
    Ok(name)
}

fn lower_input_oninput_attr(attr: &JSXAttr, cx: &Cx<'_>) -> LResult<String> {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        return Err(unsupported(attr.span, "<input>'s `onInput` must be `{(e) => ...}`"));
    };
    let Expr::Arrow(arrow @ ArrowExpr { body, params, span: arrow_span, .. }) = unwrap_paren(expr) else {
        return Err(unsupported(expr.span(), "<input>'s `onInput` must be an arrow function"));
    };
    check_arrow(arrow, "`onInput` 핸들러")?;
    let [Pat::Ident(BindingIdent { id: event_param, .. })] = params.as_slice() else {
        return Err(unsupported(
            *arrow_span,
            "<input>'s `onInput` arrow must take exactly one parameter, e.g. `(e) => ...`",
        ));
    };
    let event_name = event_param.sym.to_string();
    let BlockStmtOrExpr::Expr(body_expr) = body.as_ref() else {
        return Err(unsupported(
            body.span(),
            "<input>'s `onInput` arrow body must be a single expression `(e) => signal.set(e.target.value)` (not a block `{ ... }`)",
        ));
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = unwrap_paren(body_expr)
    else {
        return Err(unsupported(body_expr.span(), "<input>'s `onInput` body must be a single `signal.set(...)` call"));
    };
    let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
        return Err(unsupported(
            callee.span(),
            "<input>'s `onInput` body must call a method on a signal, e.g. `signal.set(...)`",
        ));
    };
    let Expr::Ident(Ident { sym: signal_name, span: signal_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "<input>'s `onInput` target must be a bare signal identifier"));
    };
    let signal_name = signal_name.to_string();
    let MemberProp::Ident(IdentName { sym: method, span: method_span }) = prop else {
        return Err(unsupported(prop.span(), "unsupported method reference in <input>'s `onInput`"));
    };
    if method != "set" {
        return Err(unsupported(
            *method_span,
            format!("<input>'s `onInput` must call `.set(...)`, got `.{method}(...)`"),
        ));
    }
    let arg = args
        .first()
        .ok_or_else(|| unsupported(*call_span, "`.set(...)` requires one argument, `e.target.value`"))?;
    check_arity(".set()", args, 1)?;
    let wrong_arg = || format!("<input>'s `.set(...)` argument must be `{event_name}.target.value`");
    let Expr::Member(MemberExpr {
        obj: target_obj,
        prop: value_prop,
        ..
    }) = unwrap_paren(&arg.expr)
    else {
        return Err(unsupported(arg.expr.span(), wrong_arg()));
    };
    let MemberProp::Ident(IdentName { sym: value_sym, span: value_span }) = value_prop else {
        return Err(unsupported(value_prop.span(), wrong_arg()));
    };
    if value_sym != "value" {
        return Err(unsupported(
            *value_span,
            format!("<input>'s `.set(...)` argument must end in `.value`, got `.{value_sym}`"),
        ));
    }
    let Expr::Member(MemberExpr {
        obj: event_obj,
        prop: target_prop,
        ..
    }) = target_obj.as_ref()
    else {
        return Err(unsupported(target_obj.span(), wrong_arg()));
    };
    let MemberProp::Ident(IdentName { sym: target_sym, span: target_span }) = target_prop else {
        return Err(unsupported(target_prop.span(), wrong_arg()));
    };
    if target_sym != "target" {
        return Err(unsupported(
            *target_span,
            format!(
                "<input>'s `.set(...)` argument must be `{event_name}.target.value`, got `.{target_sym}` instead of `.target`"
            ),
        ));
    }
    let Expr::Ident(Ident { sym: event_ref, span: event_ref_span, .. }) = event_obj.as_ref() else {
        return Err(unsupported(
            event_obj.span(),
            format!("<input>'s `.set(...)` argument must reference the arrow's own event parameter `{event_name}`"),
        ));
    };
    if event_ref.as_ref() != event_name {
        return Err(unsupported(
            *event_ref_span,
            format!(
                "<input>'s `.set(...)` argument must reference the arrow's own event parameter `{event_name}`, got `{event_ref}`"
            ),
        ));
    }
    let signal = cx.find_signal(&signal_name).ok_or_else(|| {
        unsupported(*signal_span, format!("<input>'s `onInput` references unknown signal `{signal_name}`"))
    })?;
    if !matches!(signal.init, SignalInit::Str(_)) {
        return Err(unsupported(
            *signal_span,
            format!("<input>'s `onInput` requires a Str signal, `{signal_name}` is not one"),
        ));
    }
    Ok(signal_name)
}

/// R03/R04: 인자를 받지 않는 액션 메서드에 인자가 있으면 첫 인자를 가리켜 거부한다.
fn no_action_args(code: &'static str, args: &[ExprOrSpread], message: impl FnOnce() -> String) -> LResult<()> {
    match args.first() {
        None => Ok(()),
        Some(first) => Err(Diagnostic::error(code, first.span(), message())),
    }
}

fn lower_click_action(attr: &JSXAttr, cx: &Cx<'_>) -> LResult<ClickAction> {
    let Some(JSXAttrValue::JSXExprContainer(JSXExprContainer {
        expr: JSXExpr::Expr(expr),
        ..
    })) = &attr.value
    else {
        return Err(unsupported(attr.span, "`onClick` must be `{() => ...}`"));
    };
    let Expr::Arrow(arrow @ ArrowExpr { body, params, .. }) = unwrap_paren(expr) else {
        return Err(unsupported(expr.span(), "`onClick` must be an arrow function"));
    };
    check_arrow(arrow, "`onClick` 핸들러")?;
    if let Some(first) = params.first() {
        return Err(unsupported(first.span(), "`onClick` arrow function must take no parameters"));
    }
    let BlockStmtOrExpr::Expr(body_expr) = body.as_ref() else {
        return Err(unsupported(
            body.span(),
            "`onClick` arrow body must be a single expression, e.g. `() => x.increment()` (not a block `{ ... }`)",
        ));
    };
    let Expr::Call(CallExpr {
        callee: Callee::Expr(callee),
        args,
        span: call_span,
        ..
    }) = unwrap_paren(body_expr)
    else {
        return Err(unsupported(
            body_expr.span(),
            "`onClick` body must be a single method call like `signal.increment()`",
        ));
    };
    let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
        return Err(unsupported(
            callee.span(),
            "`onClick` body must call a method on a signal, e.g. `signal.increment()`",
        ));
    };
    let Expr::Ident(Ident { sym: signal_name, span: signal_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "`onClick` target must be a bare signal identifier"));
    };
    let signal_name = signal_name.to_string();
    let signal = cx
        .find_signal(&signal_name)
        .ok_or_else(|| unsupported(*signal_span, format!("`onClick` references unknown signal `{signal_name}`")))?;
    let MemberProp::Ident(IdentName { sym: method, span: method_span }) = prop else {
        return Err(unsupported(prop.span(), "unsupported method reference in `onClick`"));
    };
    let require = |ok: bool, msg: String| -> LResult<()> {
        if ok {
            Ok(())
        } else {
            Err(unsupported(*signal_span, msg))
        }
    };

    match method.as_ref() {
        "increment" => {
            require(
                matches!(signal.init, SignalInit::I32(_)),
                format!("`.increment()` requires an I32 signal, `{signal_name}` is not one"),
            )?;
            no_action_args(R03, args, || {
                format!("`.increment()`는 인자를 받지 않습니다. `{signal_name}.increment(…)`는 지금 +1로 컴파일됩니다(액션 인자는 7b)")
            })?;
            Ok(ClickAction::Increment { signal: signal_name })
        }
        "rotate" => {
            require(
                matches!(signal.init, SignalInit::StringList(_)),
                format!("`.rotate()` requires a string-list signal, `{signal_name}` is not one"),
            )?;
            no_action_args(R03, args, || {
                format!("`.rotate()`는 인자를 받지 않습니다. `{signal_name}.rotate(…)`는 지금 한 칸 회전으로 컴파일됩니다(액션 인자는 7b)")
            })?;
            Ok(ClickAction::Rotate { signal: signal_name })
        }
        "append" => {
            require(
                matches!(signal.init, SignalInit::StringList(_)),
                format!("`.append(...)` requires a string-list signal, `{signal_name}` is not one"),
            )?;
            let arg = args
                .first()
                .ok_or_else(|| unsupported(*call_span, "`.append(...)` requires one string-literal argument"))?;
            check_arity(".append()", args, 1)?;
            let Expr::Lit(Lit::Str(s)) = unwrap_paren(&arg.expr) else {
                return Err(unsupported(
                    arg.expr.span(),
                    "`.append(...)` argument must be a string literal (Phase 1 scope: no dynamic expressions)",
                ));
            };
            Ok(ClickAction::Append {
                signal: signal_name,
                literal: s.value.to_string_lossy().into_owned(),
            })
        }
        "toggle" => {
            require(
                matches!(signal.init, SignalInit::Bool(_)),
                format!("`.toggle()` requires a Bool signal, `{signal_name}` is not one"),
            )?;
            no_action_args(R04, args, || {
                format!("`.toggle()`은 인자를 받지 않습니다. `{signal_name}.toggle(…)`는 지금 반전으로 컴파일됩니다")
            })?;
            Ok(ClickAction::Toggle { signal: signal_name })
        }
        other => Err(unsupported(
            *method_span,
            format!("unsupported signal method `.{other}()` (supported: increment, append, rotate, toggle)"),
        )),
    }
}

fn lower_child(child: &JSXElementChild, cx: &mut Cx<'_>, row_var: Option<&str>) -> LResult<Option<JsxNode>> {
    match child {
        JSXElementChild::JSXText(text) => {
            let trimmed = text.value.trim();
            if trimmed.is_empty() {
                Ok(None)
            } else {
                let raw: &str = &text.raw;
                let lead = (raw.len() - raw.trim_start().len()) as u32;
                let trail = (raw.len() - raw.trim_end().len()) as u32;
                cx.node(Span::new(text.span.lo + BytePos(lead), text.span.hi - BytePos(trail)));
                Ok(Some(JsxNode::StaticText(trimmed.to_string())))
            }
        }
        JSXElementChild::JSXElement(el) => Ok(Some(lower_element(el, cx, row_var)?)),
        JSXElementChild::JSXExprContainer(JSXExprContainer {
            expr: JSXExpr::Expr(expr),
            ..
        }) => Ok(Some(lower_expr_container(expr, cx, row_var)?)),
        other => Err(unsupported(other.span(), "unsupported JSX child")),
    }
}

fn lower_expr_container(expr: &Expr, cx: &mut Cx<'_>, row_var: Option<&str>) -> LResult<JsxNode> {
    match unwrap_paren(expr) {
        // `{item}` inside a list row template.
        Expr::Ident(Ident { sym, span, .. }) if row_var == Some(sym.as_ref()) => {
            cx.node(*span);
            Ok(JsxNode::RowItemText)
        }

        // `{sharedValue("cartCount", 0)}` — read-only cross-widget display,
        // no local signal lookup at all (unlike every other binding here) —
        // that absence is itself the point (see `JsxNode::SharedValueBinding`'s
        // doc comment).
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            span: call_span,
            ..
        }) if matches!(callee.as_ref(), Expr::Ident(Ident { sym, .. }) if sym == "sharedValue") => {
            let [key_arg, initial_arg] = args.as_slice() else {
                return Err(unsupported(
                    *call_span,
                    "sharedValue() requires exactly two arguments, e.g. sharedValue(\"cartCount\", 0)",
                ));
            };
            let Expr::Lit(Lit::Str(key)) = unwrap_paren(&key_arg.expr) else {
                return Err(unsupported(
                    key_arg.expr.span(),
                    "sharedValue() requires a string key and an integer initial value, e.g. sharedValue(\"cartCount\", 0)",
                ));
            };
            let Expr::Lit(Lit::Num(initial)) = unwrap_paren(&initial_arg.expr) else {
                return Err(unsupported(
                    initial_arg.expr.span(),
                    "sharedValue() requires a string key and an integer initial value, e.g. sharedValue(\"cartCount\", 0)",
                ));
            };
            let initial = lit_i32(initial)?;
            cx.node(*call_span);
            cx.spans.loom_calls.push(("sharedValue", callee.span()));
            Ok(JsxNode::SharedValueBinding {
                key: key.value.to_string_lossy().into_owned(),
                initial,
            })
        }

        // `{cond() && <tag>...</tag>}` — compiles to RQ1 Track T candidate F.
        // (2026-10-17 note: "candidate F" is the Phase 1 label. `cond_reconcile_f_system` is the CSS-hide preserve variant, i.e. the protocol's CSS 은닉 (v-show) control arm — not the F family F-i/ii/iii added 2026-07-31. See cond.rs header.)
        Expr::Bin(BinExpr {
            op: BinaryOp::LogicalAnd,
            left,
            right,
            ..
        }) => {
            // `{open() && item.length > 2 && <X/>}` — SWC는 `&&`를 좌결합으로
            // 파싱하므로 `left`가 다시 `&&`이면 그 오른쪽이 **row 술어**다.
            // row 템플릿 밖에서는 애초에 row 변수가 없으므로 자연히 거부된다.
            let (left, row_predicate) = match unwrap_paren(left) {
                Expr::Bin(BinExpr {
                    op: BinaryOp::LogicalAnd,
                    left: inner_left,
                    right: inner_right,
                    ..
                }) => {
                    let row_var = row_var.ok_or_else(|| {
                        unsupported(
                            inner_right.span(),
                            "`a() && b && <X/>` 형태의 가운데 항은 row 템플릿 안에서만 쓸 수 있다 — 바깥에서는 조건 signal 하나만 받는다",
                        )
                    })?;
                    (inner_left.as_ref(), Some(parse_row_predicate(inner_right, row_var)?))
                }
                other => (other, None),
            };
            let Expr::Call(CallExpr {
                callee: Callee::Expr(callee),
                args,
                span: cond_call_span,
                ..
            }) = unwrap_paren(left)
            else {
                return Err(unsupported(
                    left.span(),
                    "conditional's left side must be a bare `signalName()` boolean read",
                ));
            };
            if let Some(first) = args.first() {
                return Err(unsupported(
                    first.expr.span(),
                    "`signalName()` in a conditional must take no arguments",
                ));
            }
            let Expr::Ident(Ident { sym: cond_name, span: cond_name_span, .. }) = callee.as_ref() else {
                return Err(unsupported(
                    callee.span(),
                    "conditional's left side must be a bare signal identifier call",
                ));
            };
            let cond_name = cond_name.to_string();
            let signal = cx.find_signal(&cond_name).ok_or_else(|| {
                unsupported(
                    *cond_name_span,
                    format!("`{{{cond_name}() && ...}}` references unknown signal `{cond_name}`"),
                )
            })?;
            if !matches!(signal.init, SignalInit::Bool(_)) {
                return Err(unsupported(
                    *cond_name_span,
                    format!("`&&`-conditional requires a Bool signal, `{cond_name}` is not one"),
                ));
            }
            let cond_call_span = *cond_call_span;

            let Expr::JSXElement(then_el) = unwrap_paren(right) else {
                return Err(unsupported(right.span(), "conditional's right side must be a single JSX element"));
            };
            cx.node(cond_call_span);
            let then_branch = lower_element(then_el, cx, row_var)?;
            // 7a C2: 옛 `unreachable!()`. `<Widget client:load/>`나 `<input>`을
            // 오른쪽에 쓰면 사용자가 실제로 도달하므로 정식 진단으로 바꿨다.
            let JsxNode::Element { children, .. } = &then_branch else {
                return Err(unsupported(
                    then_el.opening.name.span(),
                    format!(
                        "conditional's right side must be a plain HTML element (`{{{cond_name}() && <div>…</div>}}`), not a component reference or <input>"
                    ),
                ));
            };
            // **2026-08-07**: 여기 있던 "정적 자식만 허용" 거부를 걷었다.
            // 조건부 안의 `{signal()}`/`{signal().map(...)}`/중첩 엘리먼트는
            // 이제 codegen이 사이트별 전용 함수로 방출한다(codegen.rs의
            // `emit_lazy_subtree`). 거부가 남아 있으면 RQ1 §5 트랙 N이 요구하는
            // 3중 중첩 픽스처를 **컴파일러가 생성할 수 없고**, §2.3-1이
            // "컴파일러가 생성할 수 없는 형태의 코드 금지"라 손코딩 우회도
            // 막혀 있었다.
            //
            // 아직 못 받는 것들은 codegen 쪽에서 각각 이유를 붙여 거부한다
            // (`collect_lazy_slots_node`/`emit_lazy_subtree`의 패닉 문구):
            // 조건부 안의 중첩 조건부, `<input>` 양방향 binding, 자체 `onClick`,
            // `<Component/>` 참조, `sharedValue(...)`.
            for (child, child_src) in children.iter().zip(dynamic_or_element_children(then_el)) {
                if matches!(child, JsxNode::CondBinding { .. }) {
                    return Err(unsupported(
                        child_src,
                        format!(
                            "`{{{cond_name}() && ...}}` 안에 또 조건부를 넣는 것은 아직 지원하지 않는다 — 안쪽 사이트도 자기 marker/전용 함수/`CondNested`가 필요한데, 그 중첩 생성 경로가 아직 없다(codegen.rs의 `CondBinding` arm 참고)"
                        ),
                    ));
                }
            }
            // 사이트 기록은 then 가지를 다 내려간 **뒤**다 — 그래야 이 조건부의
            // 사이트가 안쪽 사이트보다 뒤에 쌓인다. 순서가 아니라 span으로
            // 찾으므로 무관하지만, 기록 위치를 한 곳으로 고정해 둔다.
            cx.binding(&cond_name, BindingKind::Cond, row_var, cond_call_span);
            Ok(JsxNode::CondBinding {
                cond: cond_name,
                row_predicate,
                then_branch: Box::new(then_branch),
            })
        }

        // `{signalName()}` — a bare zero-arg call to a known signal.
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            span: call_span,
            ..
        }) if args.is_empty() && matches!(callee.as_ref(), Expr::Ident(_)) => {
            let Expr::Ident(Ident { sym, span: name_span, .. }) = callee.as_ref() else {
                unreachable!()
            };
            let name = sym.to_string();
            if cx.find_signal(&name).is_some() {
                cx.node(*call_span);
                cx.binding(&name, BindingKind::Text, row_var, *call_span);
                Ok(JsxNode::TextBinding { signal: name })
            } else {
                Err(unsupported(*name_span, format!("`{{{name}()}}` does not reference a declared signal")))
            }
        }

        // `{signalName().map((row) => <el>{row}</el>)}`
        Expr::Call(CallExpr {
            callee: Callee::Expr(callee),
            args,
            span: map_call_span,
            ..
        }) => {
            let Expr::Member(MemberExpr { obj, prop, .. }) = callee.as_ref() else {
                return Err(unsupported(
                    callee.span(),
                    "unsupported dynamic expression in JSX (only `signal()`, or `signal().map(...)`, are supported)",
                ));
            };
            let MemberProp::Ident(IdentName { sym: method, span: method_span }) = prop else {
                return Err(unsupported(prop.span(), "unsupported member call in JSX"));
            };
            if method != "map" {
                return Err(unsupported(
                    *method_span,
                    format!("only `.map(...)` is supported on a list signal read, got `.{method}(...)`"),
                ));
            }
            // `xs().slice(0, item.length).map(...)` — row 변수로 좁힌 리스트
            // (2026-08-07). `.slice`가 끼어 있으면 한 겹 벗기고 그 사실을 기록한다.
            let (obj, row_slice) = match obj.as_ref() {
                Expr::Call(CallExpr { callee: Callee::Expr(c), args: slice_args, span: slice_span, .. })
                    if matches!(c.as_ref(), Expr::Member(MemberExpr { prop: MemberProp::Ident(IdentName { sym, .. }), .. }) if sym == "slice") =>
                {
                    let Expr::Member(MemberExpr { obj: slice_obj, .. }) = c.as_ref() else {
                        unreachable!()
                    };
                    let row_var = row_var.ok_or_else(|| {
                        unsupported(
                            *slice_span,
                            "`.slice(...)`는 row 템플릿 안에서만 쓸 수 있다 — row 변수로 좁히는 것이 유일한 용도다",
                        )
                    })?;
                    parse_row_slice(slice_args, row_var, *slice_span)?;
                    (slice_obj.as_ref(), Some(RowSlice::PrefixByItemLen))
                }
                other => (other, None),
            };
            let Expr::Call(CallExpr {
                callee: Callee::Expr(inner_callee),
                args: inner_args,
                span: list_read_span,
                ..
            }) = obj
            else {
                return Err(unsupported(obj.span(), "`.map(...)` must be called directly on `signalName()`"));
            };
            if let Some(first) = inner_args.first() {
                return Err(unsupported(
                    first.expr.span(),
                    "`signalName()` must be called with no arguments before `.map(...)`",
                ));
            }
            let Expr::Ident(Ident { sym: signal_name, span: signal_span, .. }) = inner_callee.as_ref() else {
                return Err(unsupported(inner_callee.span(), "`.map(...)` must be called on a bare `signalName()`"));
            };
            let signal_name = signal_name.to_string();
            let signal = cx
                .find_signal(&signal_name)
                .ok_or_else(|| unsupported(*signal_span, format!("`.map(...)` on unknown signal `{signal_name}`")))?;
            if !matches!(signal.init, SignalInit::StringList(_)) {
                return Err(unsupported(
                    *signal_span,
                    format!("`.map(...)` requires a string-list signal, `{signal_name}` is not one"),
                ));
            }

            let map_arg = args
                .first()
                .ok_or_else(|| unsupported(*map_call_span, "`.map(...)` requires one arrow-function argument"))?;
            check_arity(".map()", args, 1)?;
            let Expr::Arrow(
                arrow @ ArrowExpr {
                    params,
                    body: row_body,
                    span: arrow_span,
                    ..
                },
            ) = unwrap_paren(&map_arg.expr)
            else {
                return Err(unsupported(map_arg.expr.span(), "`.map(...)` argument must be an arrow function"));
            };
            check_arrow(arrow, "`.map(...)` row 함수")?;
            let [Pat::Ident(BindingIdent { id: row_ident, .. })] = params.as_slice() else {
                return Err(unsupported(
                    *arrow_span,
                    "`.map((row) => ...)` must take exactly one bare identifier parameter",
                ));
            };
            let row_var_name = row_ident.sym.to_string();
            let BlockStmtOrExpr::Expr(row_expr) = row_body.as_ref() else {
                return Err(unsupported(
                    row_body.span(),
                    "`.map(...)` row template must be a single JSX expression, not a block",
                ));
            };
            let Expr::JSXElement(row_el) = unwrap_paren(row_expr) else {
                return Err(unsupported(row_expr.span(), "`.map(...)` row template must be a JSX element"));
            };
            // R08: row 루트의 `onClick`은 지금 조용히 사라진다(row 템플릿의 액션은
            // 생성되지 않아 `enum Action {}`가 된다). row 안쪽 요소의 `onClick`은
            // check.rs(codegen.rs:2319 복제)와 R44가 따로 막는다.
            if let Some(onclick) = row_el.opening.attrs.iter().find_map(|a| match a {
                JSXAttrOrSpread::JSXAttr(attr)
                    if matches!(&attr.name, JSXAttrName::Ident(IdentName { sym, .. }) if sym == "onClick") =>
                {
                    Some(attr.span)
                }
                _ => None,
            }) {
                let tag = match &row_el.opening.name {
                    JSXElementName::Ident(Ident { sym, .. }) => sym.to_string(),
                    _ => "…".to_string(),
                };
                return Err(Diagnostic::error(
                    R08,
                    onclick,
                    format!(
                        "리스트 row 루트 `<{tag}>`의 `onClick`은 지금 조용히 사라집니다(`enum Action {{}}`). row 안 이벤트는 key 표면(G0 P1) 이후 7d에서 다룹니다"
                    ),
                ));
            }
            cx.node(*list_read_span);
            let row = lower_element(row_el, cx, Some(&row_var_name))?;
            cx.binding(&signal_name, BindingKind::List, row_var, *map_call_span);

            Ok(JsxNode::ListBinding {
                signal: signal_name,
                row_var: row_var_name,
                row: Box::new(row),
                row_slice,
            })
        }
        other => Err(unsupported(other.span(), "unsupported JSX expression")),
    }
}

/// `lower_element`가 IR 자식을 만드는 것과 **같은 규칙**(공백뿐인 JSXText는
/// 건너뜀)으로 원본 JSX 자식의 span을 나열한다 — IR 자식과 1:1로 짝지어
/// 진단 위치를 찾는 데만 쓴다. (`lower_child`가 성공한 뒤에만 불리므로
/// 나머지 자식 종류는 이미 걸러져 있다.)
fn dynamic_or_element_children(el: &JSXElement) -> impl Iterator<Item = Span> + '_ {
    el.children.iter().filter_map(|child| match child {
        JSXElementChild::JSXText(text) if text.value.trim().is_empty() => None,
        other => Some(other.span()),
    })
}

/// `item.length > n` 한 형태만 받는다. 표현식 언어를 여는 것이 목적이 아니라,
/// **row마다 구조가 달라지는 픽스처**를 컴파일러가 낼 수 있게 하는 것이 목적이다
/// (RQ1 §5 트랙 N — 구간 크기가 row마다 같으면 못 잡는 결함이 있다).
fn parse_row_predicate(expr: &Expr, row_var: &str) -> LResult<RowPredicate> {
    let Expr::Bin(BinExpr { op: BinaryOp::Gt, left, right, .. }) = unwrap_paren(expr) else {
        return Err(unsupported(
            expr.span(),
            format!("row 술어는 `{row_var}.length > <정수>` 형태여야 한다"),
        ));
    };
    let Expr::Member(MemberExpr { obj, prop: MemberProp::Ident(IdentName { sym, span: prop_span }), .. }) =
        unwrap_paren(left)
    else {
        return Err(unsupported(left.span(), format!("row 술어의 왼쪽은 `{row_var}.length` 여야 한다")));
    };
    if sym != "length" {
        return Err(unsupported(
            *prop_span,
            format!("row 술어가 지원하는 속성은 `.length` 뿐이다 (`.{sym}` 받음)"),
        ));
    }
    let Expr::Ident(Ident { sym: obj_name, span: obj_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "row 술어의 왼쪽은 row 변수여야 한다"));
    };
    if obj_name != row_var {
        return Err(unsupported(
            *obj_span,
            format!("row 술어는 이 row의 변수(`{row_var}`)를 써야 한다 — `{obj_name}` 받음"),
        ));
    }
    let Expr::Lit(Lit::Num(n)) = unwrap_paren(right) else {
        return Err(unsupported(right.span(), "row 술어의 오른쪽은 정수 리터럴이어야 한다"));
    };
    Ok(RowPredicate::LenGt(lit_usize(n)?))
}

/// `.slice(0, item.length)` 한 형태만. 인자 모양을 실제로 검사한다 —
/// 통과시켜 놓고 다른 의미로 코드를 내면 저작 표면과 산출물이 조용히 어긋난다.
fn parse_row_slice(args: &[ExprOrSpread], row_var: &str, slice_span: Span) -> LResult<()> {
    let [start, end] = args else {
        return Err(unsupported(
            slice_span,
            format!("`.slice(...)`는 `slice(0, {row_var}.length)` 형태여야 한다 (인자 2개)"),
        ));
    };
    let Expr::Lit(Lit::Num(Number { value: 0.0, .. })) = unwrap_paren(&start.expr) else {
        return Err(unsupported(start.expr.span(), "`.slice(...)`의 첫 인자는 `0` 이어야 한다"));
    };
    let Expr::Member(MemberExpr { obj, prop: MemberProp::Ident(IdentName { sym, span: prop_span }), .. }) =
        unwrap_paren(&end.expr)
    else {
        return Err(unsupported(
            end.expr.span(),
            format!("`.slice(...)`의 둘째 인자는 `{row_var}.length` 여야 한다"),
        ));
    };
    if sym != "length" {
        return Err(unsupported(*prop_span, "`.slice(...)`의 둘째 인자는 `.length` 여야 한다"));
    }
    let Expr::Ident(Ident { sym: obj_name, span: obj_span, .. }) = obj.as_ref() else {
        return Err(unsupported(obj.span(), "`.slice(...)`의 둘째 인자는 row 변수여야 한다"));
    };
    if obj_name != row_var {
        return Err(unsupported(
            *obj_span,
            format!("`.slice(...)`는 이 row의 변수(`{row_var}`)를 써야 한다 — `{obj_name}` 받음"),
        ));
    }
    Ok(())
}
