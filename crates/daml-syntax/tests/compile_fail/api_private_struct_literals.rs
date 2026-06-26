use daml_parser::lexer::{LexError, Token, Trivia};
use daml_syntax::{
    ByteColumn, ByteOffset, CoordinateRangeError, CoordinateRangeErrorKind, LineIndex, LineNumber,
    ParserSpanToTextRangeError, ParserSpanToTextRangeErrorKind, SourceFile, SourceTokens,
    Utf16Offset, Utf16Range,
};

fn main() {
    let parsed = SourceFile::parse("module M where\n");

    let _ = LineIndex {
        source_len: 0,
        line_start_bytes: vec![],
        char_offset_by_byte: vec![],
        utf16_offset_by_byte: vec![],
    };

    let _ = CoordinateRangeError {
        line: LineNumber::new(1),
        byte_column: ByteColumn::new(1),
        source_line_count: LineNumber::new(1),
        max_byte_column: None,
        kind: CoordinateRangeErrorKind::LineOutOfRange,
    };

    let _ = SourceTokens {
        tokens: Vec::<Token>::new(),
        trivia: Vec::<Trivia>::new(),
        lex_errors: Vec::<LexError>::new(),
        laid_out_tokens: std::sync::OnceLock::new(),
    };

    let _ = SourceFile {
        source: parsed.source().to_string(),
        module: parsed.module().clone(),
        diagnostics: Vec::new(),
        line_index: LineIndex::new(""),
        tokens: std::sync::OnceLock::new(),
    };

    let _ = ParserSpanToTextRangeError {
        source_len: 0,
        span_start: ByteOffset::new(0),
        span_end: ByteOffset::new(0),
        kind: ParserSpanToTextRangeErrorKind::OutOfBounds,
    };

    let _ = Utf16Range {
        start: Utf16Offset::new(0),
        end: Utf16Offset::new(0),
    };
}
