use daml_syntax::{CharColumn, DiagnosticEndColumn};

fn describe(end_column: DiagnosticEndColumn) -> &'static str {
    match end_column {
        DiagnosticEndColumn::SameLineEnd(_) => "same-line",
        DiagnosticEndColumn::Multiline => "multiline",
        DiagnosticEndColumn::EmptySpan => "empty",
    }
}

fn main() {
    let _ = describe(DiagnosticEndColumn::SameLineEnd(CharColumn::new(2)));
}
