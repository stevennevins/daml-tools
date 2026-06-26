use daml_parser::ast::DiagnosticCategory;
use daml_syntax::{CharColumn, Diagnostic, DiagnosticEndColumn, LineNumber, TextRange};

fn main() {
    let _ = Diagnostic {
        range: TextRange::new(0.into(), 1.into()),
        line: LineNumber::new(1),
        column: CharColumn::new(1),
        end_column: DiagnosticEndColumn::SameLineEnd(CharColumn::new(2)),
        message: String::from("private construction should fail"),
        category: DiagnosticCategory::Malformed,
    };
}
