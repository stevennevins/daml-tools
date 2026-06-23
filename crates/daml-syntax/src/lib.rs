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

pub use text_size::{TextRange, TextSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineCol {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub range: TextRange,
    pub line: usize,
    pub column: usize,
    pub end_column: Option<usize>,
    pub message: String,
    pub category: DiagnosticCategory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineIndex {
    source_len: usize,
    line_start_bytes: Vec<usize>,
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
            utf16_offset_by_byte,
        }
    }

    #[must_use]
    pub fn line_col(&self, offset: TextSize) -> LineCol {
        let byte = usize::from(offset).min(self.source_len);
        let line_idx = match self.line_start_bytes.binary_search(&byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        LineCol {
            line: line_idx + 1,
            column: byte - self.line_start_bytes[line_idx] + 1,
        }
    }

    #[must_use]
    pub fn char_line_col(&self, source: &str, offset: TextSize) -> LineCol {
        let mut byte = usize::from(offset).min(self.source_len);
        while !source.is_char_boundary(byte) {
            byte = byte.saturating_sub(1);
        }
        let line_idx = match self.line_start_bytes.binary_search(&byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_start_bytes[line_idx];
        LineCol {
            line: line_idx + 1,
            column: source[line_start..byte].chars().count() + 1,
        }
    }

    #[must_use]
    pub fn utf16_col(&self, line: usize, byte_col: usize) -> usize {
        let line_start = self
            .line_start_bytes
            .get(line.saturating_sub(1))
            .copied()
            .unwrap_or(self.source_len);
        let byte = line_start
            .saturating_add(byte_col.saturating_sub(1))
            .min(self.source_len);
        self.utf16_offset_by_byte[byte] - self.utf16_offset_by_byte[line_start]
    }

    #[must_use]
    pub fn utf16_range(&self, range: TextRange) -> (usize, usize) {
        let start = usize::from(range.start()).min(self.source_len);
        let end = usize::from(range.end()).min(self.source_len).max(start);
        (
            self.utf16_offset_by_byte[start],
            self.utf16_offset_by_byte[end],
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
            .get_or_init(|| resolve_layout(self.tokens.clone()))
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
                let range = parser_span_to_text_range(source, diagnostic.span);
                let start = range.start();
                let end_column = source
                    .get(usize::from(range.start())..usize::from(range.end()))
                    .filter(|s| !s.is_empty() && !s.contains('\n'))
                    .map(|s| diagnostic.pos.column + s.chars().count());
                Diagnostic {
                    range,
                    line: line_index.char_line_col(source, start).line,
                    column: diagnostic.pos.column,
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

    #[must_use]
    pub fn parser_span_to_text_range(&self, span: ParserSpan) -> TextRange {
        parser_span_to_text_range(&self.source, span)
    }

    fn source_tokens(&self) -> &SourceTokens {
        self.tokens.get_or_init(|| SourceTokens::lex(&self.source))
    }
}

#[must_use]
pub fn parser_span_to_text_range(source: &str, span: ParserSpan) -> TextRange {
    let start = span.start.min(source.len());
    let end = span.end.min(source.len()).max(start);
    TextRange::new(
        TextSize::try_from(start).unwrap(),
        TextSize::try_from(end).unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use daml_parser::ast_span::render_from_ast;
    use daml_parser::lexer::render_lossless;

    #[test]
    fn maps_empty_source_to_first_line() {
        let index = LineIndex::new("");

        assert_eq!(index.line_col(0.into()), LineCol { line: 1, column: 1 });
        assert_eq!(index.utf16_range(TextRange::empty(0.into())), (0, 0));
    }

    #[test]
    fn maps_ascii_byte_lines() {
        let source = "module M where\nfoo = 1\n";
        let index = LineIndex::new(source);

        assert_eq!(index.line_col(15.into()), LineCol { line: 2, column: 1 });
        assert_eq!(index.utf16_col(2, 4), 3);
    }

    #[test]
    fn maps_utf8_and_utf16_offsets() {
        let source = "a😀b\nz";
        let index = LineIndex::new(source);

        assert_eq!(
            index.utf16_range(TextRange::new(0.into(), 6.into())),
            (0, 4)
        );
        assert_eq!(index.utf16_col(1, 6), 3);
        assert_eq!(
            index.char_line_col(source, 5.into()),
            LineCol { line: 1, column: 3 }
        );
    }

    #[test]
    fn char_line_col_snaps_to_previous_utf8_boundary() {
        let source = "a😀b";
        let index = LineIndex::new(source);

        // Offset 3 is inside the 4-byte 😀 sequence (1..5), so we expect snapping to 1.
        assert_eq!(
            index.char_line_col(source, 3.into()),
            LineCol { line: 1, column: 2 }
        );
    }

    #[test]
    fn preserves_trailing_newline_line_start() {
        let index = LineIndex::new("a\n");

        assert_eq!(index.line_col(2.into()), LineCol { line: 2, column: 1 });
    }

    #[test]
    fn treats_crlf_as_bytes_without_normalization() {
        let index = LineIndex::new("a\r\nb");

        assert_eq!(index.line_col(3.into()), LineCol { line: 2, column: 1 });
    }

    #[test]
    fn clamps_ranges_to_source_end() {
        let index = LineIndex::new("abc");
        let range = TextRange::new(1.into(), 99.into());

        assert_eq!(index.utf16_range(range), (1, 3));
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
        let range = file.parser_span_to_text_range(ParserSpan::new(0, 99));

        assert_eq!(
            range,
            TextRange::new(0.into(), file.source().len().try_into().unwrap())
        );
    }
}
