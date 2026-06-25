use daml_syntax::{CharColumn, SourceFile};

fn main() {
    let file = SourceFile::parse("module M where\nfoo = @\n");
    let diagnostic = file.diagnostics().first().unwrap();
    let _: Option<CharColumn> = diagnostic.end_column();
}
