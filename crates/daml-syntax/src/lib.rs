//! Shared parsed-source surface for Daml tools.
//!
//! `daml-parser` stays the low-level lexer/layout/parser implementation.
//! This crate owns the source-facing facts tools need around that parser:
//! diagnostics, line/UTF-16 mapping, tokens, trivia, laid-out tokens, and
//! conversion from parser byte spans to `text-size` ranges.
//!
//! ```rust
//! use daml_syntax::{parser_span_to_text_range, SourceFile};
//!
//! let source = "module M where\nfoo : Int\nfoo = 1\n";
//! let file = SourceFile::parse(source);
//!
//! assert_eq!(file.module().name, "M");
//! assert!(file.diagnostics().is_empty());
//! assert!(!file.tokens().is_empty());
//! assert!(!file.laid_out_tokens().is_empty());
//!
//! let header_range = parser_span_to_text_range(source, file.module().header);
//! assert_eq!(usize::from(header_range.start()), 0);
//! assert_eq!(header_range, file.parser_span_to_text_range(file.module().header));
//! ```

use daml_parser::ast::{DiagnosticCategory, Module, Span as ParserSpan};
use daml_parser::layout::resolve_layout;
use daml_parser::lexer::{lex_with_trivia, LexError, Token, Trivia};
use daml_parser::parse::parse_module;
use std::sync::OnceLock;

pub mod coordinate;

pub use coordinate::{
    ByteColumn, ByteLineCol, ByteOffset, CharColumn, CharLineCol, Coordinate, LineNumber,
    Utf16Offset,
};
pub use text_size::{TextRange, TextSize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: TextRange,
    pub line: LineNumber,
    pub column: CharColumn,
    pub end_column: Option<CharColumn>,
    pub message: String,
    pub category: DiagnosticCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    source_len: usize,
    line_start_bytes: Vec<usize>,
    char_offset_by_byte: Vec<usize>,
    utf16_offset_by_byte: Vec<usize>,
}

impl LineIndex {
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut line_start_bytes = vec![0];
        for (idx, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_start_bytes.push(idx + 1);
            }
        }

        let mut char_offset_by_byte = vec![0; source.len() + 1];
        let mut char_count = 0usize;
        let mut prev = 0usize;
        for (idx, ch) in source.char_indices() {
            for slot in char_offset_by_byte.iter_mut().take(idx).skip(prev) {
                *slot = char_count;
            }
            let char_end = idx + ch.len_utf8();
            for slot in char_offset_by_byte.iter_mut().take(char_end).skip(idx) {
                *slot = char_count;
            }
            char_count += 1;
            prev = char_end;
        }
        for slot in char_offset_by_byte
            .iter_mut()
            .take(source.len() + 1)
            .skip(prev)
        {
            *slot = char_count;
        }

        let mut utf16_offset_by_byte = vec![0; source.len() + 1];
        let mut utf16 = 0usize;
        let mut prev = 0usize;
        for (idx, ch) in source.char_indices() {
            for slot in utf16_offset_by_byte.iter_mut().take(idx).skip(prev) {
                *slot = utf16;
            }
            let char_end = idx + ch.len_utf8();
            for slot in utf16_offset_by_byte.iter_mut().take(char_end).skip(idx) {
                *slot = utf16;
            }
            utf16 += ch.len_utf16();
            prev = char_end;
        }
        for slot in utf16_offset_by_byte
            .iter_mut()
            .take(source.len() + 1)
            .skip(prev)
        {
            *slot = utf16;
        }

        Self {
            source_len: source.len(),
            line_start_bytes,
            char_offset_by_byte,
            utf16_offset_by_byte,
        }
    }

    #[must_use]
    pub fn line_col(&self, offset: TextSize) -> ByteLineCol {
        let byte = usize::from(offset).min(self.source_len);
        let line_idx = match self.line_start_bytes.binary_search(&byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        ByteLineCol {
            line: LineNumber::new(line_idx + 1),
            column: ByteColumn::new(byte - self.line_start_bytes[line_idx] + 1),
        }
    }

    #[must_use]
    pub fn char_line_col(&self, offset: TextSize) -> CharLineCol {
        let byte = usize::from(offset).min(self.source_len);
        let line_idx = match self.line_start_bytes.binary_search(&byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_start_bytes[line_idx];
        CharLineCol {
            line: LineNumber::new(line_idx + 1),
            column: CharColumn::new(
                self.char_offset_by_byte[byte] - self.char_offset_by_byte[line_start] + 1,
            ),
        }
    }

    #[must_use]
    pub fn utf16_col(&self, line: LineNumber, byte_col: ByteColumn) -> Utf16Offset {
        let line_start = self
            .line_start_bytes
            .get(line.get().saturating_sub(1))
            .copied()
            .unwrap_or(self.source_len);
        let byte = line_start
            .saturating_add(byte_col.get().saturating_sub(1))
            .min(self.source_len);
        Utf16Offset::new(self.utf16_offset_by_byte[byte] - self.utf16_offset_by_byte[line_start])
    }

    #[must_use]
    pub fn utf16_range(&self, range: TextRange) -> (Utf16Offset, Utf16Offset) {
        let start = usize::from(range.start()).min(self.source_len);
        let end = usize::from(range.end()).min(self.source_len).max(start);
        (
            Utf16Offset::new(self.utf16_offset_by_byte[start]),
            Utf16Offset::new(self.utf16_offset_by_byte[end]),
        )
    }
}

#[derive(Debug)]
pub struct SourceTokens {
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    lex_errors: Vec<LexError>,
    laid_out_tokens: OnceLock<Vec<Token>>,
}

impl SourceTokens {
    #[must_use]
    pub fn lex(source: &str) -> Self {
        let lexed = lex_with_trivia(source);
        Self {
            tokens: lexed.tokens,
            trivia: lexed.trivia,
            lex_errors: lexed.errors,
            laid_out_tokens: OnceLock::new(),
        }
    }

    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    #[must_use]
    pub fn trivia(&self) -> &[Trivia] {
        &self.trivia
    }

    #[must_use]
    pub fn lex_errors(&self) -> &[LexError] {
        &self.lex_errors
    }

    #[must_use]
    pub fn laid_out_tokens(&self) -> &[Token] {
        self.laid_out_tokens
            .get_or_init(|| resolve_layout(self.tokens.as_slice()))
    }
}

#[derive(Debug)]
pub struct SourceFile {
    source: String,
    module: Module,
    diagnostics: Vec<Diagnostic>,
    line_index: LineIndex,
    tokens: OnceLock<SourceTokens>,
}

impl SourceFile {
    #[must_use]
    pub fn parse(source: &str) -> Self {
        let parsed = parse_module(source);
        let line_index = LineIndex::new(source);
        let diagnostics = parsed
            .diagnostics
            .into_iter()
            .map(|diagnostic| {
                let range = try_parser_span_to_text_range(source, diagnostic.span)
                    .expect("parser span in diagnostic must map to source bytes");
                let start = range.start();
                let end_column = source
                    .get(usize::from(range.start())..usize::from(range.end()))
                    .filter(|s| !s.is_empty() && !s.contains('\n'))
                    .map(|s| CharColumn::new(diagnostic.pos.column + s.chars().count()));
                let start_pos = line_index.char_line_col(start);
                Diagnostic {
                    range,
                    line: start_pos.line,
                    column: CharColumn::new(diagnostic.pos.column),
                    end_column,
                    message: diagnostic.message,
                    category: diagnostic.category,
                }
            })
            .collect();

        Self {
            source: source.to_string(),
            module: parsed.module,
            diagnostics,
            line_index,
            tokens: OnceLock::new(),
        }
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    #[must_use]
    pub const fn module(&self) -> &Module {
        &self.module
    }

    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        self.source_tokens().tokens()
    }

    #[must_use]
    pub fn trivia(&self) -> &[Trivia] {
        self.source_tokens().trivia()
    }

    #[must_use]
    pub fn laid_out_tokens(&self) -> &[Token] {
        self.source_tokens().laid_out_tokens()
    }

    /// Convert a parser span from this source into a `text-size` byte range.
    ///
    /// This is the convenience API for spans that originate from this source file
    /// and are expected to be valid.
    ///
    /// # Panics
    ///
    /// Panics when `span` does not map to valid UTF-8 source bytes in this
    /// source.
    #[must_use]
    pub fn parser_span_to_text_range(&self, span: ParserSpan) -> TextRange {
        self.try_parser_span_to_text_range(span)
            .expect("parser span must map to a valid UTF-8 range in source")
    }

    /// Try to convert a parser span from this source into a `text-size` byte
    /// range.
    ///
    /// This fallible API is the preferred choice for spans from external or
    /// untrusted sources where offsets may be invalid. Use
    /// [`SourceFile::parser_span_to_text_range`] for spans that originate from
    /// this source and are expected to map to valid UTF-8 bytes.
    #[must_use = "handle invalid span offsets before using the range"]
    pub fn try_parser_span_to_text_range(
        &self,
        span: ParserSpan,
    ) -> Result<TextRange, ParserSpanToTextRangeError> {
        try_parser_span_to_text_range(&self.source, span)
    }

    fn source_tokens(&self) -> &SourceTokens {
        self.tokens.get_or_init(|| SourceTokens::lex(&self.source))
    }
}

/// Convert a parser span into a `text-size` byte range for an arbitrary source
/// string.
///
/// This is a convenience wrapper around [`try_parser_span_to_text_range`] for
/// spans that are expected to be valid.
///
/// # Panics
///
/// Panics when `span` does not map to valid UTF-8 source bytes in `source`.
#[must_use]
pub fn parser_span_to_text_range(source: &str, span: ParserSpan) -> TextRange {
    try_parser_span_to_text_range(source, span)
        .expect("parser span must map to a valid UTF-8 range")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParserSpanToTextRangeErrorKind {
    OutOfBounds,
    InvertedSpan,
    NonUtf8Boundary,
    TextSizeOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserSpanToTextRangeError {
    source_len: usize,
    span_start: usize,
    span_end: usize,
    kind: ParserSpanToTextRangeErrorKind,
}

impl ParserSpanToTextRangeError {
    #[must_use]
    pub const fn source_len(&self) -> usize {
        self.source_len
    }

    #[must_use]
    pub const fn span_start(&self) -> usize {
        self.span_start
    }

    #[must_use]
    pub const fn span_end(&self) -> usize {
        self.span_end
    }

    #[must_use]
    pub const fn kind(&self) -> ParserSpanToTextRangeErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ParserSpanToTextRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ParserSpanToTextRangeErrorKind::TextSizeOverflow => write!(
                f,
                "parser span [{}, {}) cannot be represented as a text-size value",
                self.span_start, self.span_end
            ),
            _ => write!(
                f,
                "parser span [{}, {}) is invalid for source length {}",
                self.span_start, self.span_end, self.source_len
            ),
        }
    }
}

impl std::error::Error for ParserSpanToTextRangeError {}

/// Try to convert a parser span into a `text-size` byte range.
///
/// This is the fallible API and should be used for spans sourced outside
/// `SourceFile` where invalid offsets are possible; offsets must be valid
/// UTF-8 character boundaries.
#[must_use = "handle invalid span offsets before converting"]
pub fn try_parser_span_to_text_range(
    source: &str,
    span: ParserSpan,
) -> Result<TextRange, ParserSpanToTextRangeError> {
    let source_len = source.len();
    if span.start > source_len || span.end > source_len {
        return Err(ParserSpanToTextRangeError {
            source_len,
            span_start: span.start,
            span_end: span.end,
            kind: ParserSpanToTextRangeErrorKind::OutOfBounds,
        });
    }
    if span.start > span.end {
        return Err(ParserSpanToTextRangeError {
            source_len,
            span_start: span.start,
            span_end: span.end,
            kind: ParserSpanToTextRangeErrorKind::InvertedSpan,
        });
    }
    if !source.is_char_boundary(span.start) || !source.is_char_boundary(span.end) {
        return Err(ParserSpanToTextRangeError {
            source_len,
            span_start: span.start,
            span_end: span.end,
            kind: ParserSpanToTextRangeErrorKind::NonUtf8Boundary,
        });
    }
    Ok(TextRange::new(
        TextSize::try_from(span.start).map_err(|_| ParserSpanToTextRangeError {
            source_len,
            span_start: span.start,
            span_end: span.end,
            kind: ParserSpanToTextRangeErrorKind::TextSizeOverflow,
        })?,
        TextSize::try_from(span.end).map_err(|_| ParserSpanToTextRangeError {
            source_len,
            span_start: span.start,
            span_end: span.end,
            kind: ParserSpanToTextRangeErrorKind::TextSizeOverflow,
        })?,
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use daml_parser::ast_span::render_from_ast;
    use daml_parser::lexer::render_lossless;

    #[test]
    fn maps_empty_source_to_first_line() {
        let index = LineIndex::new("");

        assert_eq!(
            index.line_col(0.into()),
            ByteLineCol {
                line: LineNumber::new(1),
                column: ByteColumn::new(1),
            }
        );
        assert_eq!(
            index.utf16_range(TextRange::empty(0.into())),
            (Utf16Offset::new(0), Utf16Offset::new(0))
        );
    }

    #[test]
    fn maps_ascii_byte_lines() {
        let source = "module M where\nfoo = 1\n";
        let index = LineIndex::new(source);

        assert_eq!(
            index.line_col(15.into()),
            ByteLineCol {
                line: LineNumber::new(2),
                column: ByteColumn::new(1),
            }
        );
        assert_eq!(
            index.utf16_col(LineNumber::new(2), ByteColumn::new(4)),
            Utf16Offset::new(3)
        );
    }

    #[test]
    fn maps_utf8_and_utf16_offsets() {
        let source = "a😀b\nz";
        let index = LineIndex::new(source);

        assert_eq!(
            index.utf16_range(TextRange::new(0.into(), 6.into())),
            (Utf16Offset::new(0), Utf16Offset::new(4))
        );
        assert_eq!(
            index.utf16_col(LineNumber::new(1), ByteColumn::new(6)),
            Utf16Offset::new(3)
        );
        assert_eq!(
            index.char_line_col(5.into()),
            CharLineCol {
                line: LineNumber::new(1),
                column: CharColumn::new(3),
            }
        );
    }

    #[test]
    fn char_line_col_snaps_to_previous_utf8_boundary() {
        let source = "a😀b";
        let index = LineIndex::new(source);

        // Offset 3 is inside the 4-byte 😀 sequence (1..5), so we expect snapping to 1.
        assert_eq!(
            index.char_line_col(3.into()),
            CharLineCol {
                line: LineNumber::new(1),
                column: CharColumn::new(2),
            }
        );
    }

    #[test]
    fn preserves_trailing_newline_line_start() {
        let index = LineIndex::new("a\n");

        assert_eq!(
            index.line_col(2.into()),
            ByteLineCol {
                line: LineNumber::new(2),
                column: ByteColumn::new(1),
            }
        );
    }

    #[test]
    fn treats_crlf_as_bytes_without_normalization() {
        let index = LineIndex::new("a\r\nb");

        assert_eq!(
            index.line_col(3.into()),
            ByteLineCol {
                line: LineNumber::new(2),
                column: ByteColumn::new(1),
            }
        );
    }

    #[test]
    fn clamps_ranges_to_source_end() {
        let index = LineIndex::new("abc");
        let range = TextRange::new(1.into(), 99.into());

        assert_eq!(
            index.utf16_range(range),
            (Utf16Offset::new(1), Utf16Offset::new(3))
        );
    }

    #[test]
    fn byte_and_char_line_col_columns_differ_for_multibyte_utf8() {
        let source = "😀\n";
        let index = LineIndex::new(source);
        let offset = TextSize::from(4);

        let byte_pos = index.line_col(offset);
        let char_pos = index.char_line_col(offset);

        assert_eq!(byte_pos.line, char_pos.line);
        assert_ne!(byte_pos.column.get(), char_pos.column.get());
        assert_eq!(byte_pos.column, ByteColumn::new(5));
        assert_eq!(char_pos.column, CharColumn::new(2));
    }

    #[test]
    fn source_file_exposes_parser_pipeline_facts() {
        let source = "module M where\nfoo : Int\nfoo = 1\n";
        let file = SourceFile::parse(source);

        assert_eq!(file.source(), source);
        assert_eq!(file.module().name, "M");
        assert!(file.diagnostics().is_empty());
        assert!(!file.tokens().is_empty());
        assert!(!file.laid_out_tokens().is_empty());
        assert_eq!(
            render_lossless(source, file.tokens(), file.trivia()).as_deref(),
            Ok(source)
        );
        assert_eq!(
            render_from_ast(source, file.module(), file.trivia()).as_deref(),
            Ok(source)
        );
    }

    #[test]
    fn source_tokens_exposes_lex_only_pipeline_facts() {
        let source = "module M where\nfoo : Int\nfoo = 1\n";
        let tokens = SourceTokens::lex(source);

        assert!(tokens.lex_errors().is_empty());
        assert!(!tokens.tokens().is_empty());
        assert!(!tokens.laid_out_tokens().is_empty());
        assert_eq!(
            render_lossless(source, tokens.tokens(), tokens.trivia()).as_deref(),
            Ok(source)
        );
    }

    #[test]
    fn malformed_source_keeps_source_file_and_diagnostics() {
        let file = SourceFile::parse("module M where\nfoo = \"unterminated\nbar = 1\n");

        assert_eq!(file.module().name, "M");
        assert!(file
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.category == DiagnosticCategory::Lex));
    }

    #[test]
    fn converts_parser_spans_to_text_ranges() {
        let file = SourceFile::parse("module M where\nfoo = 1\n");
        let source_len = file.source().len();
        let range = file.parser_span_to_text_range(ParserSpan::new(0, source_len));

        assert_eq!(
            range,
            TextRange::new(0.into(), source_len.try_into().unwrap())
        );
    }

    #[test]
    fn try_parser_span_to_text_range_rejects_out_of_bounds_spans() {
        let source = "module M where\nfoo = 1\n";
        let err = try_parser_span_to_text_range(source, ParserSpan::new(0, source.len() + 1))
            .unwrap_err();
        assert_eq!(err.kind(), ParserSpanToTextRangeErrorKind::OutOfBounds);
        assert_eq!(
            err.to_string(),
            format!(
                "parser span [0, {}) is invalid for source length {}",
                source.len() + 1,
                source.len()
            )
        );
        assert_eq!(err.source_len(), source.len());
        assert_eq!(err.span_start(), 0);
        assert_eq!(err.span_end(), source.len() + 1);
    }

    #[test]
    fn try_parser_span_to_text_range_reports_inverted_spans() {
        let source = "abc";
        let err = try_parser_span_to_text_range(source, ParserSpan::new(2, 1)).unwrap_err();
        assert_eq!(err.kind(), ParserSpanToTextRangeErrorKind::InvertedSpan);
        assert_eq!(
            err.to_string(),
            "parser span [2, 1) is invalid for source length 3"
        );
        assert_eq!(err.source_len(), source.len());
        assert_eq!(err.span_start(), 2);
        assert_eq!(err.span_end(), 1);
    }

    #[test]
    fn try_parser_span_to_text_range_rejects_non_utf8_boundary_spans() {
        let source = "a😀b";
        let err = try_parser_span_to_text_range(source, ParserSpan::new(1, 2)).unwrap_err();
        assert_eq!(err.kind(), ParserSpanToTextRangeErrorKind::NonUtf8Boundary);

        assert_eq!(
            err.to_string(),
            "parser span [1, 2) is invalid for source length 6"
        );
        assert_eq!(err.source_len(), source.len());
        assert_eq!(err.span_start(), 1);
        assert_eq!(err.span_end(), 2);
    }

    #[test]
    fn try_parser_span_to_text_range_succeeds_for_valid_span() {
        let source = "module M where\nfoo = 1\n";
        let range = try_parser_span_to_text_range(source, ParserSpan::new(0, 5))
            .expect("span should be valid");
        assert_eq!(range, TextRange::new(0.into(), 5.into()));
    }
}
