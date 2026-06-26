use daml_syntax::{Diagnostic, SourceFile};

fn main() {
    let file = SourceFile::parse("module M where\nfoo = @\n");
    let diagnostic = file.diagnostics().first().unwrap().clone();
    let Diagnostic { message, .. } = diagnostic;
    let _ = message;
}
