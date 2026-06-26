//! Integration tests for [`SourceFile`] over the vendored daml-finance corpus.
//!
//! Exercises the public `daml-syntax` surface — parse, diagnostics, tokens,
//! trivia losslessness, and source-range invariants — on known-clean real-world
//! Daml sources shared at the workspace root.

#![allow(clippy::unwrap_used)]

use daml_parser::lexer::render_lossless;
use daml_syntax::{SourceFile, TextRange};
use std::path::{Path, PathBuf};

fn finance_corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/daml-finance/daml")
}

/// True when the vendored corpus is present. Absent corpus is a legitimate skip
/// off the workspace (e.g. a published crate), but under CI it must be present —
/// fail loud so a missing/forgotten corpus cannot pass green.
fn corpus_present() -> bool {
    let root = finance_corpus_root();
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

fn collect_daml_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_daml_files(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "daml") {
            out.push(path);
        }
    }
    Ok(())
}

fn range_within_source(range: TextRange, source_len: usize) -> bool {
    let start = usize::from(range.start());
    let end = usize::from(range.end());
    start <= end && end <= source_len
}

fn check_source_file_surface(file: &SourceFile) -> Result<(), String> {
    let source = file.source();
    let source_len = source.len();

    if !file.diagnostics().is_empty() {
        let messages: Vec<_> = file
            .diagnostics()
            .iter()
            .map(|d| format!("{:?}: {}", d.category(), d.message()))
            .collect();
        return Err(format!("parse diagnostics: {messages:?}"));
    }
    if file.tokens().is_empty() {
        return Err("tokens empty".to_string());
    }
    if file.laid_out_tokens().is_empty() {
        return Err("laid-out tokens empty".to_string());
    }
    if usize::from(file.line_index().source_len_bytes()) != source_len {
        return Err("line index source length mismatch".to_string());
    }

    let module = file.module();
    let module_range = file.parser_span_to_text_range(module.span);
    if !range_within_source(module_range, source_len) {
        return Err(format!("module span out of bounds: {module_range:?}"));
    }
    let header_range = file.parser_span_to_text_range(module.header);
    if !range_within_source(header_range, source_len) {
        return Err(format!(
            "module header span out of bounds: {header_range:?}"
        ));
    }

    render_lossless(source, file.tokens(), file.trivia())
        .map_err(|e| format!("token+trivia losslessness: {e}"))?;
    Ok(())
}

/// Parse every vendored daml-finance file through [`SourceFile::parse`] and
/// verify the public syntax surface invariants on the known-clean corpus.
#[test]
fn finance_corpus_source_file_surface() {
    if !corpus_present() {
        return;
    }

    let root = finance_corpus_root();
    let mut files = Vec::new();
    collect_daml_files(&root, &mut files).expect("collect finance corpus files");
    assert!(
        files.len() > 600,
        "finance corpus incomplete: {} files",
        files.len()
    );

    let mut failures = Vec::new();
    for path in &files {
        let src = match std::fs::read_to_string(path) {
            Ok(src) => src,
            Err(e) => {
                failures.push(format!(
                    "{}: failed to read corpus file: {e}",
                    path.display()
                ));
                continue;
            }
        };
        let file = SourceFile::parse(&src);
        if let Err(reason) = check_source_file_surface(&file) {
            failures.push(format!("{}: {reason}", path.display()));
        }
    }

    if !failures.is_empty() {
        let shown: Vec<_> = failures.iter().take(20).cloned().collect();
        panic!(
            "{} / {} files failed SourceFile surface invariants:\n{}",
            failures.len(),
            files.len(),
            shown.join("\n")
        );
    }
}
