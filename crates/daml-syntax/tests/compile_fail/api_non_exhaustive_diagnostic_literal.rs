use daml_parser::ast::DiagnosticCategory;
use daml_syntax::{CharColumn, Diagnostic, DiagnosticEndColumn, LineNumber, TextRange};

fn main() {
    let _ = Diagnostic {
        range: TextRange::empty(0.into()),
        line: LineNumber::new(1),
        column: CharColumn::new(1),
        end_column: DiagnosticEndColumn::EmptySpan,
        message: String::from("msg"),
        category: DiagnosticCategory::Lex,
    };
}
