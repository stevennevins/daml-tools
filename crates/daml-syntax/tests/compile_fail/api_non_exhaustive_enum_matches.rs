use daml_syntax::{
    CoordinateRangeErrorKind, DiagnosticEndColumn, ParserSpanToTextRangeErrorKind, SourceFile,
};

fn main() {
    let file = SourceFile::parse("module M where\nfoo = @\n");
    let diagnostic = file.diagnostics().first().unwrap();

    match diagnostic.end_column() {
        DiagnosticEndColumn::SameLineEnd(_) => {}
        DiagnosticEndColumn::Multiline => {}
        DiagnosticEndColumn::EmptySpan => {}
    }

    let kind = CoordinateRangeErrorKind::LineOutOfRange;
    match kind {
        CoordinateRangeErrorKind::LineOutOfRange => {}
        CoordinateRangeErrorKind::ColumnOutOfRange => {}
    }

    let span_kind = ParserSpanToTextRangeErrorKind::OutOfBounds;
    match span_kind {
        ParserSpanToTextRangeErrorKind::OutOfBounds => {}
        ParserSpanToTextRangeErrorKind::InvertedSpan => {}
        ParserSpanToTextRangeErrorKind::NonUtf8Boundary => {}
        ParserSpanToTextRangeErrorKind::TextSizeOverflow => {}
    }
}
