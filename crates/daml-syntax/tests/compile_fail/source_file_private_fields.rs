use daml_syntax::SourceFile;

fn main() {
    let file = SourceFile::parse("module M where\nfoo = 1\n");
    let SourceFile { source, .. } = file;
    let _ = source;
}
