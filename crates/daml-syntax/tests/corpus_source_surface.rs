//! Source-facing `daml-syntax` invariants over the vendored daml-finance corpus.
//!
//! This exercises the public [`daml_syntax::SourceFile`] surface on real Daml
//! sources: clean diagnostics, token/trivia losslessness, layout availability,
//! and parser-span conversion for module spans.

#![allow(clippy::unwrap_used)]

use daml_parser::lexer::render_lossless;
use daml_syntax::SourceFile;
use std::path::{Path, PathBuf};

fn corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/daml-finance/daml")
}

/// True if the vendored corpus is present. Off-workspace published-crate test
/// runs may skip, but CI must fail loudly so a missing corpus cannot pass green.
fn corpus_present() -> bool {
    let root = corpus_root();
    if root.exists() {
        return true;
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "vendored corpus missing under CI (was it committed?): {}",
        root.display()
    );
    eprintln!("corpus absent (published crate?), skipping");
    false
}

fn collect_daml_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_daml_files(&path, files)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "daml")
        {
            files.push(path);
        }
    }
    Ok(())
}

#[test]
fn corpus_source_surface() {
    if !corpus_present() {
        return;
    }

    let root = corpus_root();
    let mut files = Vec::new();
    collect_daml_files(&root, &mut files).unwrap();
    files.sort();
    assert!(
        files.len() > 600,
        "daml-finance corpus incomplete: {} files under {}",
        files.len(),
        root.display()
    );

    let mut failures = Vec::new();
    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                failures.push(format!("{}: read failed: {error}", path.display()));
                continue;
            }
        };

        let Ok(file) = std::panic::catch_unwind(|| SourceFile::parse(&source)) else {
            failures.push(format!("{}: SourceFile::parse panicked", path.display()));
            continue;
        };

        if !file.diagnostics().is_empty() {
            failures.push(format!(
                "{}: diagnostics: {}",
                path.display(),
                file.diagnostics()
                    .iter()
                    .map(|diagnostic| format!(
                        "{:?}@{}:{} {}",
                        diagnostic.category(),
                        diagnostic.line(),
                        diagnostic.column(),
                        diagnostic.message()
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }

        if let Err(error) = render_lossless(&source, file.tokens(), file.trivia()) {
            failures.push(format!(
                "{}: token/trivia lossless: {error}",
                path.display()
            ));
        }

        if !source.trim().is_empty() && file.laid_out_tokens().is_empty() {
            failures.push(format!(
                "{}: non-empty source produced no laid-out tokens",
                path.display()
            ));
        }

        for (label, span) in [
            ("module", file.module().span),
            ("module header", file.module().header),
        ] {
            match file.try_parser_span_to_text_range(span) {
                Ok(range) => {
                    if usize::from(range.end()) > source.len() {
                        failures.push(format!(
                            "{}: {label} range {:?} exceeds source length {}",
                            path.display(),
                            range,
                            source.len()
                        ));
                    }
                }
                Err(error) => failures.push(format!(
                    "{}: {label} span {:?} does not map to source: {error}",
                    path.display(),
                    span
                )),
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} / {} corpus files failed SourceFile surface checks:\n{}",
        failures.len(),
        files.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
