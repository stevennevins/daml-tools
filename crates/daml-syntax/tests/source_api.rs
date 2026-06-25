//! Integration tests for [`SourceFile`], [`SourceTokens`], diagnostics accessors,
//! and parser-span conversion through the public `daml-syntax` API.

#![allow(clippy::unwrap_used)]

use daml_parser::ast::DiagnosticCategory;
use daml_parser::ast::Span as ParserSpan;
use daml_parser::ast_span::render_from_ast;
use daml_parser::lexer::render_lossless;
use daml_syntax::{
    try_parser_span_to_text_range, ByteOffset, DiagnosticEndColumn, ParserSpanToTextRangeErrorKind,
    SourceFile, SourceTokens, TextRange,
};

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
fn source_tokens_clone_and_equality_ignore_lazy_layout_cache_state() {
    let source = "module M where\nfoo : Int\nfoo = 1\n";
    let cached = SourceTokens::lex(source);
    let uncached = SourceTokens::lex(source);

    assert!(!cached.laid_out_tokens().is_empty());
    assert_eq!(cached, uncached);
    assert_eq!(cached.clone(), cached);
}

#[test]
fn source_file_clone_and_equality_ignore_lazy_token_cache_state() {
    let source = "module M where\nfoo : Int\nfoo = 1\n";
    let cached = SourceFile::parse(source);
    let uncached = SourceFile::parse(source);

    assert!(!cached.tokens().is_empty());
    assert_eq!(cached, uncached);
    assert_eq!(cached.clone(), cached);
}

#[test]
fn malformed_source_keeps_source_file_and_diagnostics() {
    let file = SourceFile::parse("module M where\nfoo = \"unterminated\nbar = 1\n");

    assert_eq!(file.module().name, "M");
    assert!(file
        .diagnostics()
        .iter()
        .any(|diagnostic| diagnostic.category() == DiagnosticCategory::Lex));
}

#[test]
fn converts_parser_spans_to_text_ranges() {
    let file = SourceFile::parse("module M where\nfoo = 1\n");
    let source_len = file.source().len();
    let range = file.parser_span_to_text_range(ParserSpan::from_usize(0, source_len));

    assert_eq!(
        range,
        TextRange::new(0.into(), source_len.try_into().unwrap())
    );
}

#[test]
fn try_parser_span_to_text_range_rejects_out_of_bounds_spans() {
    let source = "module M where\nfoo = 1\n";
    let err = try_parser_span_to_text_range(source, ParserSpan::from_usize(0, source.len() + 1))
        .unwrap_err();
    assert_eq!(err.kind(), ParserSpanToTextRangeErrorKind::OutOfBounds);
    assert_eq!(
        err.to_string(),
        format!(
            "parser span [0, {}) is invalid: out of bounds for source length {}",
            source.len() + 1,
            source.len()
        )
    );
    assert_eq!(err.source_len_bytes(), ByteOffset::new(source.len()));
    assert_eq!(err.span_start(), daml_syntax::ByteOffset::new(0));
    assert_eq!(
        err.span_end(),
        daml_syntax::ByteOffset::new(source.len() + 1)
    );
    assert_eq!(usize::from(err.span_start()), 0);
    assert_eq!(usize::from(err.span_end()), source.len() + 1);
}

#[test]
fn try_parser_span_to_text_range_reports_inverted_spans() {
    let source = "abc";
    let err = try_parser_span_to_text_range(source, ParserSpan::from_usize(2, 1)).unwrap_err();
    assert_eq!(err.kind(), ParserSpanToTextRangeErrorKind::InvertedSpan);
    assert_eq!(
        err.to_string(),
        "parser span [2, 1) is invalid: start after end for source length 3"
    );
    assert_eq!(err.source_len_bytes(), ByteOffset::new(source.len()));
    assert_eq!(err.span_start(), daml_syntax::ByteOffset::new(2));
    assert_eq!(err.span_end(), daml_syntax::ByteOffset::new(1));
}

#[test]
fn try_parser_span_to_text_range_rejects_non_utf8_boundary_spans() {
    let source = "a😀b";
    let err = try_parser_span_to_text_range(source, ParserSpan::from_usize(1, 2)).unwrap_err();
    assert_eq!(err.kind(), ParserSpanToTextRangeErrorKind::NonUtf8Boundary);

    assert_eq!(
        err.to_string(),
        "parser span [1, 2) is invalid: not on a UTF-8 boundary for source length 6"
    );
    assert_eq!(err.source_len_bytes(), ByteOffset::new(source.len()));
    assert_eq!(err.span_start(), daml_syntax::ByteOffset::new(1));
    assert_eq!(err.span_end(), daml_syntax::ByteOffset::new(2));
}

#[test]
fn diagnostics_are_read_through_accessors_not_field_literals() {
    let file = SourceFile::parse("module M where\nfoo = \"unterminated\n");

    let diagnostic = file
        .diagnostics()
        .first()
        .expect("malformed source should surface diagnostics");
    assert_eq!(diagnostic.category(), DiagnosticCategory::Lex);
    assert!(!diagnostic.message().is_empty());
    assert!(diagnostic.range().start() <= diagnostic.range().end());
    assert!(usize::from(diagnostic.line()) >= 1);
    assert!(usize::from(diagnostic.column()) >= 1);
    assert!(matches!(
        diagnostic.end_column(),
        DiagnosticEndColumn::SameLineEnd(_)
            | DiagnosticEndColumn::Multiline
            | DiagnosticEndColumn::EmptySpan
    ));
}

#[test]
fn parser_span_error_kind_displays_specific_reason() {
    assert_eq!(
        ParserSpanToTextRangeErrorKind::OutOfBounds.to_string(),
        "out of bounds"
    );
    assert_eq!(
        ParserSpanToTextRangeErrorKind::InvertedSpan.to_string(),
        "start after end"
    );
    assert_eq!(
        ParserSpanToTextRangeErrorKind::NonUtf8Boundary.to_string(),
        "not on a UTF-8 boundary"
    );
    assert_eq!(
        ParserSpanToTextRangeErrorKind::TextSizeOverflow.to_string(),
        "text-size overflow"
    );
}

#[test]
fn try_parser_span_to_text_range_succeeds_for_valid_span() {
    let source = "module M where\nfoo = 1\n";
    let range = try_parser_span_to_text_range(source, ParserSpan::from_usize(0, 5))
        .expect("span should be valid");
    assert_eq!(range, TextRange::new(0.into(), 5.into()));
}
