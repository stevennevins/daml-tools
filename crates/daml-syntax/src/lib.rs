//! Shared parsed-source surface for Daml tools.
//!
//! `daml-parser` stays the low-level lexer/layout/parser implementation.
//! This crate owns the source-facing facts tools need around that parser:
//! diagnostics, line/UTF-16 mapping, tokens, trivia, laid-out tokens, and
//! conversion from parser byte spans to `text-size` ranges.
//!
//! # Public dependencies
//!
//! This crate intentionally exposes types from `daml-parser` and the
//! `text-size` crate in its public API. Downstream `SemVer` expectations include
//! compatible major versions of those dependencies when their types appear in
//! function signatures or re-exports:
//!
//! - [`daml_parser::ast`], [`daml_parser::lexer`], and related parser types
//!   used by [`SourceFile`] and span conversion helpers.
//! - [`TextRange`] and [`TextSize`], re-exported from `text-size` for source
//!   ranges and offsets.
//!
//! Coordinate newtypes such as [`LineNumber`], [`ByteColumn`], and
//! [`CharColumn`] are 1-based and reject zero via [`LineNumber::try_new`] or
//! `TryFrom<usize>`; 0-based offsets use [`ByteOffset`] and [`Utf16Offset`].
//! Use `usize::from(coordinate)` for explicit raw extraction.
//! UTF-16 column lookup returns [`CoordinateRangeError`] for line or column
//! coordinates outside a source, and UTF-16 ranges use [`Utf16Range`] so
//! JavaScript string slices cannot be mistaken for byte ranges.
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
    ByteColumn, ByteLineCol, ByteOffset, CharColumn, CharLineCol, InvalidOneBasedCoordinate,
    LineNumber, Utf16Offset, Utf16Range,
};
pub use text_size::{TextRange, TextSize};

/// A parser or lexer diagnostic anchored in source text.
///
/// Constructed by [`SourceFile::parse`]; read fields through accessors so future
/// metadata (severity, codes, notes) can be added without breaking callers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Diagnostic {
    range: TextRange,
    line: LineNumber,
    column: CharColumn,
    end_column: DiagnosticEndColumn,
    message: String,
    category: DiagnosticCategory,
}

/// End-column shape for a diagnostic span.
///
/// The end column is an exclusive 1-based Unicode-scalar column only when a
/// diagnostic covers non-empty text on one line. Multi-line and empty spans are
/// separate variants so callers cannot silently treat both cases as `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticEndColumn {
    /// Non-empty single-line diagnostic ending at this exclusive column.
    SameLineEnd(CharColumn),
    /// Diagnostic span crosses at least one newline.
    Multiline,
    /// Diagnostic span is zero-width.
    EmptySpan,
}

impl Diagnostic {
    /// Byte range of the diagnostic in source text.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// 1-based line number of the diagnostic start.
    #[must_use]
    pub const fn line(&self) -> LineNumber {
        self.line
    }

    /// 1-based character column of the diagnostic start.
    #[must_use]
    pub const fn column(&self) -> CharColumn {
        self.column
    }

    /// Shape of the diagnostic span end.
    #[must_use]
    pub const fn end_column(&self) -> DiagnosticEndColumn {
        self.end_column
    }

    /// Human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Parser diagnostic category from `daml-parser`.
    #[must_use]
    pub const fn category(&self) -> DiagnosticCategory {
        self.category
    }
}

/// Precomputed line, character, and UTF-16 offset tables for a source string.
///
/// Offsets passed to lookup methods are clamped to the source length. Returned
/// line and column coordinates are always valid 1-based values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    source_len: usize,
    line_start_bytes: Vec<usize>,
    char_offset_by_byte: Vec<usize>,
    utf16_offset_by_byte: Vec<usize>,
}

impl LineIndex {
    /// Builds line and column lookup tables for `source`.
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

    /// Source length in bytes, represented as the EOF byte offset.
    #[must_use]
    pub const fn source_len_bytes(&self) -> ByteOffset {
        ByteOffset::new(self.source_len)
    }

    /// Maps a byte offset to a 1-based line and byte column, clamping past EOF.
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

    /// Maps a byte offset to a 1-based line and Unicode scalar column.
    ///
    /// Non-UTF-8-boundary offsets snap to the previous character boundary.
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

    /// Returns the UTF-16 code-unit offset from the start of `line` to `byte_col`.
    ///
    /// Both coordinates must be within this source. Zero cannot be represented
    /// by [`LineNumber`] or [`ByteColumn`], and this method returns a typed
    /// error instead of clamping lines or columns past the source.
    ///
    /// # Errors
    ///
    /// Returns [`CoordinateRangeError`] when `line` is past the source line
    /// count or `byte_col` is past the end column for that line.
    #[must_use = "handle invalid source coordinates before using the UTF-16 offset"]
    pub fn utf16_col(
        &self,
        line: LineNumber,
        byte_col: ByteColumn,
    ) -> Result<Utf16Offset, CoordinateRangeError> {
        let line_idx = line.get() - 1;
        let Some(line_start) = self.line_start_bytes.get(line_idx).copied() else {
            return Err(CoordinateRangeError {
                line,
                byte_column: byte_col,
                source_line_count: LineNumber::new(self.line_start_bytes.len()),
                max_byte_column: None,
                kind: CoordinateRangeErrorKind::LineOutOfRange,
            });
        };

        let line_end = self.line_end_byte(line_idx);
        let max_byte_column = ByteColumn::new(line_end - line_start + 1);
        let byte = byte_col
            .get()
            .checked_sub(1)
            .and_then(|column_offset| line_start.checked_add(column_offset));
        let Some(byte) = byte.filter(|byte| *byte <= line_end) else {
            return Err(CoordinateRangeError {
                line,
                byte_column: byte_col,
                source_line_count: LineNumber::new(self.line_start_bytes.len()),
                max_byte_column: Some(max_byte_column),
                kind: CoordinateRangeErrorKind::ColumnOutOfRange,
            });
        };

        Ok(Utf16Offset::new(
            self.utf16_offset_by_byte[byte] - self.utf16_offset_by_byte[line_start],
        ))
    }

    /// Maps a byte range to UTF-16 start/end offsets, clamping to source bounds.
    #[must_use]
    pub fn utf16_range(&self, range: TextRange) -> Utf16Range {
        let start = usize::from(range.start()).min(self.source_len);
        let end = usize::from(range.end()).min(self.source_len).max(start);
        Utf16Range::new(
            Utf16Offset::new(self.utf16_offset_by_byte[start]),
            Utf16Offset::new(self.utf16_offset_by_byte[end]),
        )
    }

    fn line_end_byte(&self, line_idx: usize) -> usize {
        self.line_start_bytes
            .get(line_idx + 1)
            .map_or(self.source_len, |next_line_start| next_line_start - 1)
    }
}

/// Failure kind when resolving a line and byte column in a [`LineIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CoordinateRangeErrorKind {
    /// Requested line is past the source line count.
    LineOutOfRange,
    /// Requested byte column is past the end column for its line.
    ColumnOutOfRange,
}

/// Error returned when a source coordinate is outside a [`LineIndex`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoordinateRangeError {
    line: LineNumber,
    byte_column: ByteColumn,
    source_line_count: LineNumber,
    max_byte_column: Option<ByteColumn>,
    kind: CoordinateRangeErrorKind,
}

impl CoordinateRangeError {
    /// Requested 1-based line number.
    #[must_use]
    pub const fn line(&self) -> LineNumber {
        self.line
    }

    /// Requested 1-based byte column.
    #[must_use]
    pub const fn byte_column(&self) -> ByteColumn {
        self.byte_column
    }

    /// Number of lines known to the [`LineIndex`].
    #[must_use]
    pub const fn source_line_count(&self) -> LineNumber {
        self.source_line_count
    }

    /// Last valid 1-based byte column on the requested line, when the line exists.
    #[must_use]
    pub const fn max_byte_column(&self) -> Option<ByteColumn> {
        self.max_byte_column
    }

    /// Specific reason coordinate resolution failed.
    #[must_use]
    pub const fn kind(&self) -> CoordinateRangeErrorKind {
        self.kind
    }
}

impl std::fmt::Display for CoordinateRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            CoordinateRangeErrorKind::LineOutOfRange => write!(
                f,
                "line {} is outside source line range 1..={}",
                self.line, self.source_line_count
            ),
            CoordinateRangeErrorKind::ColumnOutOfRange => {
                if let Some(max_byte_column) = self.max_byte_column {
                    write!(
                        f,
                        "byte column {} is outside valid range 1..={} on line {}",
                        self.byte_column, max_byte_column, self.line
                    )
                } else {
                    write!(
                        f,
                        "byte column {} is outside the valid range on line {}",
                        self.byte_column, self.line
                    )
                }
            }
        }
    }
}

impl std::error::Error for CoordinateRangeError {}

/// Lexer output for a source string without running the full module parser.
///
/// Use [`SourceFile`] when you also need the AST and parse diagnostics.
#[derive(Debug)]
pub struct SourceTokens {
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    lex_errors: Vec<LexError>,
    laid_out_tokens: OnceLock<Vec<Token>>,
}

impl Clone for SourceTokens {
    fn clone(&self) -> Self {
        let cloned = Self {
            tokens: self.tokens.clone(),
            trivia: self.trivia.clone(),
            lex_errors: self.lex_errors.clone(),
            laid_out_tokens: OnceLock::new(),
        };
        if let Some(laid_out_tokens) = self.laid_out_tokens.get() {
            let _ = cloned.laid_out_tokens.set(laid_out_tokens.clone());
        }
        cloned
    }
}

impl PartialEq for SourceTokens {
    fn eq(&self, other: &Self) -> bool {
        self.tokens == other.tokens
            && self.trivia == other.trivia
            && self.lex_errors == other.lex_errors
    }
}

impl Eq for SourceTokens {}

impl SourceTokens {
    /// Lexes `source` and records trivia and lexer errors.
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

    /// Raw lexer tokens in source order.
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        &self.tokens
    }

    /// Whitespace and comment trivia between tokens.
    #[must_use]
    pub fn trivia(&self) -> &[Trivia] {
        &self.trivia
    }

    /// Lexer diagnostics for malformed input.
    #[must_use]
    pub fn lex_errors(&self) -> &[LexError] {
        &self.lex_errors
    }

    /// Tokens after layout resolution (virtual braces and semicolons inserted).
    #[must_use]
    pub fn laid_out_tokens(&self) -> &[Token] {
        self.laid_out_tokens
            .get_or_init(|| resolve_layout(self.tokens.clone()))
    }
}

/// Parsed Daml module with diagnostics, line index, and lazy token access.
#[derive(Debug)]
pub struct SourceFile {
    source: String,
    module: Module,
    diagnostics: Vec<Diagnostic>,
    line_index: LineIndex,
    tokens: OnceLock<SourceTokens>,
}

impl Clone for SourceFile {
    fn clone(&self) -> Self {
        let cloned = Self {
            source: self.source.clone(),
            module: self.module.clone(),
            diagnostics: self.diagnostics.clone(),
            line_index: self.line_index.clone(),
            tokens: OnceLock::new(),
        };
        if let Some(tokens) = self.tokens.get() {
            let _ = cloned.tokens.set(tokens.clone());
        }
        cloned
    }
}

impl PartialEq for SourceFile {
    fn eq(&self, other: &Self) -> bool {
        self.source == other.source
            && self.module == other.module
            && self.diagnostics == other.diagnostics
            && self.line_index == other.line_index
    }
}

impl Eq for SourceFile {}

impl SourceFile {
    /// Parses `source` into a module AST and source-facing presentation data.
    ///
    /// Malformed input still returns a partial module and surfaces diagnostics;
    /// this function does not fail with `Result`.
    ///
    /// # Panics
    ///
    /// Panics when a parser diagnostic span does not map to valid UTF-8 source
    /// bytes in `source`.
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
                let start_pos = line_index.char_line_col(start);
                let end_column = diagnostic_end_column(source, range, start_pos.column);
                Diagnostic {
                    range,
                    line: start_pos.line,
                    column: start_pos.column,
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

    /// Original source text this file was parsed from.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Parsed module AST from `daml-parser`.
    #[must_use]
    pub const fn module(&self) -> &Module {
        &self.module
    }

    /// Parse and lexer diagnostics anchored in source text.
    #[must_use]
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    /// Line, column, and UTF-16 lookup tables for this source.
    #[must_use]
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }

    /// Raw lexer tokens for this source (lazy lex on first access).
    #[must_use]
    pub fn tokens(&self) -> &[Token] {
        self.source_tokens().tokens()
    }

    /// Whitespace and comment trivia for this source.
    #[must_use]
    pub fn trivia(&self) -> &[Trivia] {
        self.source_tokens().trivia()
    }

    /// Layout-resolved tokens for this source.
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
    ///
    /// # Errors
    ///
    /// Returns [`ParserSpanToTextRangeError`] when `span` is out of bounds,
    /// inverted, not on a UTF-8 boundary, or cannot fit in [`TextSize`].
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

fn diagnostic_end_column(
    source: &str,
    range: TextRange,
    start_column: CharColumn,
) -> DiagnosticEndColumn {
    let span_text = source
        .get(usize::from(range.start())..usize::from(range.end()))
        .expect("validated parser span should slice source");
    if span_text.is_empty() {
        DiagnosticEndColumn::EmptySpan
    } else if span_text.contains('\n') {
        DiagnosticEndColumn::Multiline
    } else {
        DiagnosticEndColumn::SameLineEnd(CharColumn::new(
            start_column.get() + span_text.chars().count(),
        ))
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

/// Failure kind when converting a parser span to a [`TextRange`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParserSpanToTextRangeErrorKind {
    /// Span endpoint exceeds the source length.
    OutOfBounds,
    /// Span start is greater than end.
    InvertedSpan,
    /// Span endpoint is not on a UTF-8 character boundary.
    NonUtf8Boundary,
    /// Span endpoint cannot fit in [`TextSize`].
    TextSizeOverflow,
}

impl std::fmt::Display for ParserSpanToTextRangeErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutOfBounds => f.write_str("out of bounds"),
            Self::InvertedSpan => f.write_str("start after end"),
            Self::NonUtf8Boundary => f.write_str("not on a UTF-8 boundary"),
            Self::TextSizeOverflow => f.write_str("text-size overflow"),
        }
    }
}

/// Error returned when a parser span cannot be converted to a [`TextRange`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserSpanToTextRangeError {
    source_len: usize,
    span_start: ByteOffset,
    span_end: ByteOffset,
    kind: ParserSpanToTextRangeErrorKind,
}

impl ParserSpanToTextRangeError {
    /// Length of the source string the span was checked against.
    #[must_use]
    pub const fn source_len_bytes(&self) -> ByteOffset {
        ByteOffset::new(self.source_len)
    }

    /// Inclusive start byte offset from the parser span.
    #[must_use]
    pub const fn span_start(&self) -> ByteOffset {
        self.span_start
    }

    /// Exclusive end byte offset from the parser span.
    #[must_use]
    pub const fn span_end(&self) -> ByteOffset {
        self.span_end
    }

    /// Specific reason the conversion failed.
    #[must_use]
    pub const fn kind(&self) -> ParserSpanToTextRangeErrorKind {
        self.kind
    }
}

impl std::fmt::Display for ParserSpanToTextRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.kind {
            ParserSpanToTextRangeErrorKind::OutOfBounds
            | ParserSpanToTextRangeErrorKind::InvertedSpan
            | ParserSpanToTextRangeErrorKind::NonUtf8Boundary => write!(
                f,
                "parser span [{}, {}) is invalid: {} for source length {}",
                self.span_start.get(),
                self.span_end.get(),
                self.kind,
                self.source_len
            ),
            ParserSpanToTextRangeErrorKind::TextSizeOverflow => write!(
                f,
                "parser span [{}, {}) is invalid: {}; endpoints cannot be represented as text-size values",
                self.span_start.get(),
                self.span_end.get(),
                self.kind
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
///
/// # Errors
///
/// Returns [`ParserSpanToTextRangeError`] when `span` is out of bounds,
/// inverted, not on a UTF-8 boundary, or cannot fit in [`TextSize`].
#[must_use = "handle invalid span offsets before converting"]
pub fn try_parser_span_to_text_range(
    source: &str,
    span: ParserSpan,
) -> Result<TextRange, ParserSpanToTextRangeError> {
    let source_len = source.len();
    if span.start_usize() > source_len || span.end_usize() > source_len {
        return Err(ParserSpanToTextRangeError {
            source_len,
            span_start: ByteOffset::new(span.start_usize()),
            span_end: ByteOffset::new(span.end_usize()),
            kind: ParserSpanToTextRangeErrorKind::OutOfBounds,
        });
    }
    if span.start > span.end {
        return Err(ParserSpanToTextRangeError {
            source_len,
            span_start: ByteOffset::new(span.start_usize()),
            span_end: ByteOffset::new(span.end_usize()),
            kind: ParserSpanToTextRangeErrorKind::InvertedSpan,
        });
    }
    if !source.is_char_boundary(span.start_usize()) || !source.is_char_boundary(span.end_usize()) {
        return Err(ParserSpanToTextRangeError {
            source_len,
            span_start: ByteOffset::new(span.start_usize()),
            span_end: ByteOffset::new(span.end_usize()),
            kind: ParserSpanToTextRangeErrorKind::NonUtf8Boundary,
        });
    }
    Ok(TextRange::new(
        TextSize::try_from(span.start_usize()).map_err(|_| ParserSpanToTextRangeError {
            source_len,
            span_start: ByteOffset::new(span.start_usize()),
            span_end: ByteOffset::new(span.end_usize()),
            kind: ParserSpanToTextRangeErrorKind::TextSizeOverflow,
        })?,
        TextSize::try_from(span.end_usize()).map_err(|_| ParserSpanToTextRangeError {
            source_len,
            span_start: ByteOffset::new(span.start_usize()),
            span_end: ByteOffset::new(span.end_usize()),
            kind: ParserSpanToTextRangeErrorKind::TextSizeOverflow,
        })?,
    ))
}

// README examples are compile-tested by `cargo test -p daml-syntax --doc`.
#[doc(hidden)]
mod readme_examples {
    //! ```rust
    //! use daml_syntax::{LineNumber, SourceFile, SourceTokens};
    //!
    //! let source = "module M where\nfoo : Int\nfoo = 1\n";
    //! let file = SourceFile::parse(source);
    //!
    //! assert!(file.diagnostics().is_empty());
    //! assert_eq!(file.module().name, "M");
    //! assert_eq!(file.line_index().line_col(0.into()).line, LineNumber::new(1));
    //!
    //! let tokens = SourceTokens::lex(source);
    //!
    //! assert!(tokens.lex_errors().is_empty());
    //! assert!(!tokens.laid_out_tokens().is_empty());
    //! ```
}

// Unit tests for [`LineIndex`] mapping internals stay here; [`SourceFile`],
// [`SourceTokens`], diagnostics, and span-conversion behavior live in integration tests.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn maps_empty_source_to_first_line() {
        let index = LineIndex::new("");
        assert_eq!(index.source_len_bytes(), ByteOffset::new(0));

        assert_eq!(
            index.line_col(0.into()),
            ByteLineCol {
                line: LineNumber::new(1),
                column: ByteColumn::new(1),
            }
        );
        assert_eq!(
            index.utf16_range(TextRange::empty(0.into())),
            Utf16Range::new(Utf16Offset::new(0), Utf16Offset::new(0))
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
            Ok(Utf16Offset::new(3))
        );
    }

    #[test]
    fn maps_utf8_and_utf16_offsets() {
        let source = "a😀b\nz";
        let index = LineIndex::new(source);

        assert_eq!(
            index.utf16_range(TextRange::new(0.into(), 6.into())),
            Utf16Range::new(Utf16Offset::new(0), Utf16Offset::new(4))
        );
        assert_eq!(
            index.utf16_col(LineNumber::new(1), ByteColumn::new(6)),
            Ok(Utf16Offset::new(3))
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
            Utf16Range::new(Utf16Offset::new(1), Utf16Offset::new(3))
        );
    }

    #[test]
    fn utf16_col_reports_line_past_source() {
        let index = LineIndex::new("abc\n");

        let err = index
            .utf16_col(LineNumber::new(3), ByteColumn::new(1))
            .unwrap_err();

        assert_eq!(err.kind(), CoordinateRangeErrorKind::LineOutOfRange);
        assert_eq!(err.line(), LineNumber::new(3));
        assert_eq!(err.byte_column(), ByteColumn::new(1));
        assert_eq!(err.source_line_count(), LineNumber::new(2));
        assert_eq!(err.max_byte_column(), None);
        assert_eq!(err.to_string(), "line 3 is outside source line range 1..=2");
    }

    #[test]
    fn utf16_col_reports_column_past_line_without_clamping_to_eof() {
        let index = LineIndex::new("abc\nz");

        let err = index
            .utf16_col(LineNumber::new(1), ByteColumn::new(5))
            .unwrap_err();

        assert_eq!(err.kind(), CoordinateRangeErrorKind::ColumnOutOfRange);
        assert_eq!(err.line(), LineNumber::new(1));
        assert_eq!(err.byte_column(), ByteColumn::new(5));
        assert_eq!(err.source_line_count(), LineNumber::new(2));
        assert_eq!(err.max_byte_column(), Some(ByteColumn::new(4)));
        assert_eq!(
            err.to_string(),
            "byte column 5 is outside valid range 1..=4 on line 1"
        );
    }

    #[test]
    fn diagnostic_end_column_names_same_line_multiline_and_empty_spans() {
        let source = "abc\ndef";

        assert_eq!(
            diagnostic_end_column(
                source,
                TextRange::new(1.into(), 3.into()),
                CharColumn::new(2)
            ),
            DiagnosticEndColumn::SameLineEnd(CharColumn::new(4))
        );
        assert_eq!(
            diagnostic_end_column(
                source,
                TextRange::new(1.into(), 5.into()),
                CharColumn::new(2)
            ),
            DiagnosticEndColumn::Multiline
        );
        assert_eq!(
            diagnostic_end_column(source, TextRange::empty(1.into()), CharColumn::new(2)),
            DiagnosticEndColumn::EmptySpan
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
    fn parser_span_text_size_overflow_display_includes_kind() {
        let text_size_overflow_start = usize::try_from(u32::MAX).unwrap() + 1;
        let text_size_overflow_end = usize::try_from(u32::MAX).unwrap() + 2;
        let err = ParserSpanToTextRangeError {
            source_len: usize::MAX,
            span_start: ByteOffset::new(text_size_overflow_start),
            span_end: ByteOffset::new(text_size_overflow_end),
            kind: ParserSpanToTextRangeErrorKind::TextSizeOverflow,
        };

        assert_eq!(err.kind().to_string(), "text-size overflow");
        assert_eq!(
            err.to_string(),
            format!(
                "parser span [{text_size_overflow_start}, {text_size_overflow_end}) is invalid: text-size overflow; endpoints cannot be represented as text-size values"
            )
        );
    }
}
