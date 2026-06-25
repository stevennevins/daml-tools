//! Span-oracle integration tests over daml-fmt's SDK corpus (`original/`).

#![allow(clippy::unwrap_used)]

/// `render_from_ast` byte-span losslessness oracle over daml-fmt's own
/// 924-file corpus (the SDK corpus the formatter is differential-tested
/// against). This is the AST-span invariant the formatter relies on for
/// verbatim span-slicing — a distinct check from the `format_ast` output
/// differential (`test/diff.js`). It lives here, in the crate that owns
/// `original/` and consumes the oracle, so daml-parser stays decoupled.
/// Runs in CI; skips gracefully when `original/` is absent (a published
/// crate off the workspace), but fails loud under CI so a missing/forgotten
/// corpus can't pass green.
#[test]
fn render_from_ast_lossless_over_corpus() {
    use daml_parser::ast_span::render_from_ast;
    use daml_syntax::SourceFile;
    use std::path::{Path, PathBuf};

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                collect(&p, out);
            } else if p.extension().is_some_and(|x| x == "daml") {
                out.push(p);
            }
        }
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("original");
    if !root.exists() {
        assert!(
            std::env::var_os("CI").is_none(),
            "corpus missing under CI (was crates/daml-fmt/original committed?): {}",
            root.display()
        );
        eprintln!("corpus absent (published crate?), skipping");
        return;
    }

    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        files.len() > 800,
        "corpus incomplete: {} files",
        files.len()
    );

    // Both byte-span oracles over the 924-file SDK corpus: render_from_ast
    // (AST span reconstruction) and render_lossless (token+trivia
    // reconstruction). The token-level oracle is otherwise only exercised
    // over the smaller daml-finance corpus.
    let mut failures = Vec::new();
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        let parsed = daml_parser::parse::parse_module(&src);
        if let Some((message, kind)) = parsed.diagnostics.iter().find_map(|diagnostic| {
            daml_syntax::try_parser_span_to_text_range(&src, diagnostic.span)
                .err()
                .map(|e| {
                    (
                        format!(
                            "diagnostic {:?}: {}",
                            diagnostic.category, diagnostic.message
                        ),
                        e,
                    )
                })
        }) {
            if std::env::var_os("DAML_FMT_CORPUS_DEBUG").is_some() {
                eprintln!(
                    "parser produced invalid span for {}: {} ({:?})",
                    f.display(),
                    message,
                    kind
                );
            }
            failures.push(format!(
                "parse diagnostic span invalid for {}: {message}",
                f.display()
            ));
            continue;
        }
        let source_file = SourceFile::parse(&src);
        if let Err(e) = render_from_ast(&src, source_file.module(), source_file.trivia()) {
            if std::env::var_os("DAML_FMT_CORPUS_DEBUG").is_some() {
                eprintln!("render_from_ast failed for {}: {}", f.display(), e);
            }
            failures.push(format!("render_from_ast {}: {}", f.display(), e));
        }
        // Lex errors drop bytes by design; losslessness is only promised for
        // files that lex clean (all 924 do).
        if source_file
            .diagnostics()
            .iter()
            .all(|diagnostic| diagnostic.category() != daml_parser::ast::DiagnosticCategory::Lex)
        {
            if let Err(e) = daml_parser::lexer::render_lossless(
                &src,
                source_file.tokens(),
                source_file.trivia(),
            ) {
                failures.push(format!("render_lossless {}: {}", f.display(), e));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} / {} files failed a span oracle:\n{}",
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
