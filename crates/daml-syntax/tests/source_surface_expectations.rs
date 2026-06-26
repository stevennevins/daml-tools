//! Compact public `SourceFile` surface expectations over curated fixtures.
//!
//! These goldens intentionally summarize source-facing facts rather than raw AST
//! debug output, so parser surface regressions show up without committing broad
//! snapshots.

#![allow(clippy::unwrap_used)]

use daml_syntax::{Diagnostic, DiagnosticEndColumn, SourceFile};

struct Fixture {
    name: &'static str,
    source: &'static str,
}

const FIXTURES: &[Fixture] = &[
    Fixture {
        name: "valid_module.daml",
        source: include_str!("fixtures/source_api/valid_module.daml"),
    },
    Fixture {
        name: "unterminated_string.daml",
        source: include_str!("fixtures/source_api/unterminated_string.daml"),
    },
    Fixture {
        name: "invalid_declaration.daml",
        source: include_str!("fixtures/source_api/invalid_declaration.daml"),
    },
    Fixture {
        name: "missing_else.daml",
        source: include_str!("fixtures/source_api/missing_else.daml"),
    },
    Fixture {
        name: "multibyte_columns.daml",
        source: include_str!("fixtures/source_api/multibyte_columns.daml"),
    },
];

#[test]
fn source_surface_expectations_match_curated_fixtures() {
    let actual = FIXTURES
        .iter()
        .map(normalized_summary)
        .collect::<Vec<_>>()
        .join("\n");

    let expected = r#"fixture=valid_module.daml
module=M
source_len=33
tokens=9
laid_out_tokens=12
diagnostics=0

fixture=unterminated_string.daml
module=M
source_len=43
tokens=9
laid_out_tokens=12
diagnostics=1
  lexical-error@2:7-same-line:8

fixture=invalid_declaration.daml
module=M
source_len=24
tokens=5
laid_out_tokens=7
diagnostics=1
  skipped-declaration@2:1-same-line:4

fixture=missing_else.daml
module=M
source_len=31
tokens=9
laid_out_tokens=11
diagnostics=1
  malformed@3:1-empty

fixture=multibyte_columns.daml
module=M
source_len=30
tokens=6
laid_out_tokens=8
diagnostics=0
"#;

    assert_eq!(actual, expected);
}

fn normalized_summary(fixture: &Fixture) -> String {
    let file = SourceFile::parse(fixture.source);
    let mut lines = vec![
        format!("fixture={}", fixture.name),
        format!("module={}", file.module().name),
        format!("source_len={}", file.source().len()),
        format!("tokens={}", file.tokens().len()),
        format!("laid_out_tokens={}", file.laid_out_tokens().len()),
        format!("diagnostics={}", file.diagnostics().len()),
    ];
    lines.extend(
        file.diagnostics()
            .iter()
            .map(|diagnostic| format!("  {}", normalized_diagnostic(diagnostic))),
    );
    lines.push(String::new());
    lines.join("\n")
}

fn normalized_diagnostic(diagnostic: &Diagnostic) -> String {
    format!(
        "{}@{}:{}-{}",
        diagnostic.category(),
        diagnostic.line(),
        diagnostic.column(),
        normalized_end_column(diagnostic.end_column())
    )
}

fn normalized_end_column(end_column: DiagnosticEndColumn) -> String {
    match end_column {
        DiagnosticEndColumn::SameLineEnd(column) => format!("same-line:{column}"),
        DiagnosticEndColumn::Multiline => "multiline".to_string(),
        DiagnosticEndColumn::EmptySpan => "empty".to_string(),
        other => format!("{other:?}"),
    }
}
