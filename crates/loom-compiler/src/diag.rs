//! 컴파일러 진단 (Phase 7a C1, 2026-10-17 신설).
//!
//! 사용자가 고칠 수 있는 오류·경고를 `panic!` 대신 값으로 돌려주기 위한 얇은 층.
//! 렌더링은 **swc_common 24의 기존 진단 기계를 그대로 재사용**한다
//! (`Handler` + `EmitterWriter` → rustc 모양의 `파일:줄:열` + 소스 발췌 + `^^^`
//! 강조). 새 의존성·`tty-emitter` feature는 쓰지 않는다 — Cargo.lock이 바뀌면
//! Phase 7a 바이트 동일 게이트의 D(허용 목록)에 걸린다.
//!
//! 이 모듈은 진단을 **모으고 렌더링만** 한다. 어떤 입력을 거부할지는
//! lower/check 쪽 결정이다(설계: design-7a §3).

use std::fmt;
use std::io::Write;
use std::sync::{Arc, Mutex};

use swc_common::errors::{DiagnosticId, Handler, SourceMapper, SourceMapperDyn};
use swc_common::errors::emitter::EmitterWriter;
use swc_common::sync::Lrc;
use swc_common::{SourceMap, Span};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
}

/// 진단 하나. `code`는 `LOOM-Rnn`/`LOOM-Wnn` 같은 안정 식별자다 — 테스트와
/// 문서(docs/09)는 문구가 아니라 이 코드로 대조한다.
#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    /// 주 span. 렌더 결과의 `파일:줄:열`이 이 span의 시작을 가리킨다.
    pub span: Span,
    /// 보조 위치(예: 같은 attr의 첫 번째 출현).
    pub labels: Vec<(Span, String)>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(code, span, message)
        }
    }

    pub fn with_label(mut self, span: Span, label: impl Into<String>) -> Self {
        self.labels.push((span, label.into()));
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// 한 번의 컴파일에서 나온 진단 묶음. 렌더링에 필요한 `SourceMap`을 함께
/// 들고 있다(span만으로는 줄·열을 계산할 수 없다).
///
/// 항목은 `(span.lo, code)` 순으로 정렬해 보관한다(설계 §3.4 결정성 —
/// 검사가 어떤 순서로 진단을 밀어 넣든 렌더 결과는 입력에만 의존).
#[derive(Clone)]
pub struct Diagnostics {
    cm: Lrc<SourceMap>,
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub(crate) fn new(cm: Lrc<SourceMap>) -> Self {
        Diagnostics { cm, items: Vec::new() }
    }

    pub(crate) fn push(&mut self, d: Diagnostic) {
        // 안정 정렬 삽입: 같은 (lo, code)끼리는 들어온 순서를 유지한다.
        let key = (d.span.lo, d.code);
        let at = self.items.partition_point(|x| (x.span.lo, x.code) <= key);
        self.items.insert(at, d);
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    /// `span`이 가리키는 원본 소스 조각(렌더 결과의 `^^^`가 강조하는 텍스트).
    /// 테스트가 "진단이 문제 구문을 정확히 가리키는가"를 단언하는 데 쓴다
    /// (7a C2 — `span != DUMMY_SP`만으로는 엉뚱한 위치도 통과한다).
    pub fn source_text(&self, span: Span) -> Option<String> {
        SourceMapper::span_to_snippet(&*self.cm, span).ok()
    }

    /// `span` 시작의 1-based `(줄, 열)` — 렌더 결과의 `파일:줄:열`과 같은 값.
    pub fn line_col(&self, span: Span) -> (usize, usize) {
        let loc = self.cm.lookup_char_pos(span.lo);
        (loc.line, loc.col_display + 1)
    }

    /// swc_common의 `EmitterWriter`로 렌더링한 사람용 문자열(색 없음).
    pub fn render(&self) -> String {
        let buf = SharedBuf::default();
        let sm: Lrc<SourceMapperDyn> = self.cm.clone();
        // 4번째 인자는 `teach`(swc_common-24 emitter.rs:181-186) — false.
        let emitter = EmitterWriter::new(Box::new(buf.clone()), Some(sm), false, false);
        let handler = Handler::with_emitter(true, false, Box::new(emitter));
        for d in &self.items {
            let code = DiagnosticId::Error(d.code.to_string());
            let mut b = match d.severity {
                Severity::Error => handler.struct_span_err_with_code(d.span, &d.message, code),
                Severity::Warning => handler.struct_span_warn_with_code(d.span, &d.message, code),
            };
            for (sp, label) in &d.labels {
                b.span_label(*sp, label.clone());
            }
            for n in &d.notes {
                b.note(n);
            }
            b.emit();
        }
        drop(handler);
        let bytes = buf.0.lock().unwrap().clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

impl fmt::Debug for Diagnostics {
    // `.unwrap()` 실패 메시지가 렌더된 진단 그대로 보이게 한다.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\n{}", self.render())
    }
}

impl std::error::Error for Diagnostics {}

/// 성공한 컴파일의 결과. `rust`는 Phase 7a 이전의 `String` 반환값과 바이트
/// 단위로 같다(게이트 A가 확인).
#[derive(Debug)]
pub struct Compiled {
    pub rust: String,
    pub warnings: Diagnostics,
}

#[derive(Clone, Default)]
struct SharedBuf(Arc<Mutex<Vec<u8>>>);

impl Write for SharedBuf {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
