//! Compact normalized diagnostic stream goldens for tolerant parse output.
//!
//! Snapshots category, position, message, and source span text only — not AST
//! structure or typed [`ParseDiagnosticKind`] payloads.

#![allow(clippy::unwrap_used)]

use daml_parser::ast::DiagnosticCategory;
use daml_parser::parse::parse_module;
use std::fmt::Write as _;
use std::path::PathBuf;

const CASES: &[&str] = &[
    "skipped_declaration",
    "missing_else",
    "legacy_controller_can",
    "unterminated_string",
    "invalid_escape",
    "malformed_field_type",
];

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diagnostics")
}

fn golden_path(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/diagnostics")
        .join(format!("{case}.txt"))
}

fn read_fixture(case: &str) -> String {
    let path = fixtures_dir().join(format!("{case}.daml"));
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing diagnostic fixture {}: {e}", path.display()))
}

fn read_golden(case: &str) -> String {
    let path = golden_path(case);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing diagnostic golden {}: {e}", path.display()))
}

fn format_line(
    category: DiagnosticCategory,
    line: usize,
    column: usize,
    message: &str,
    span_text: &str,
) -> String {
    let mut out = String::new();
    if category == DiagnosticCategory::Lex {
        write!(out, "{line}:{column}: {message}").unwrap();
    } else {
        write!(out, "{line}:{column}: [{category}] {message}").unwrap();
    }
    if !span_text.is_empty() {
        write!(out, " @{span_text:?}").unwrap();
    }
    out
}

fn format_diagnostic_stream(src: &str) -> String {
    parse_module(src)
        .diagnostics
        .iter()
        .map(|diagnostic| {
            let span_text = diagnostic.span.get(src).unwrap_or("");
            format_line(
                diagnostic.category,
                diagnostic.pos.line,
                diagnostic.pos.column,
                &diagnostic.message,
                span_text,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn assert_diagnostic_golden(case: &str) {
    let src = read_fixture(case);
    let actual = format_diagnostic_stream(&src);
    if std::env::var("UPDATE_GOLDEN").is_ok() {
        std::fs::create_dir_all(golden_path(case).parent().unwrap()).unwrap();
        std::fs::write(golden_path(case), format!("{actual}\n")).unwrap_or_else(|e| {
            panic!(
                "failed to write diagnostic golden {}: {e}",
                golden_path(case).display()
            )
        });
    }
    let expected = read_golden(case).trim_end().to_string();
    assert_eq!(
        actual, expected,
        "diagnostic golden mismatch for {case}\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
    );
}

#[test]
fn diagnostic_stream_goldens_match_normalized_user_output() {
    for case in CASES {
        assert_diagnostic_golden(case);
    }
}
