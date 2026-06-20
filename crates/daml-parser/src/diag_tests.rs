//! Tests for parse diagnostics: recovery categories, byte-span end positions,
//! and the guarantee that a malformed item does not abort later declarations.

#![cfg(test)]

use crate::ast::{Decl, DiagnosticCategory};
use crate::parse::parse_module;

fn diags(src: &str) -> Vec<crate::ast::ParseDiagnostic> {
    parse_module(src).1
}

#[test]
fn skipped_declaration_does_not_abort_later_template() {
    // A junk top-level token must be skipped (with a `SkippedDecl` diagnostic),
    // and the well-formed template AFTER it must still appear in the AST — the
    // whole point of per-declaration recovery.
    let src = "module M where\n\
               %%% junk\n\
               template Good\n  \
               with\n    o : Party\n  \
               where\n    signatory o\n";
    let (module, ds) = parse_module(src);
    assert!(
        ds.iter()
            .any(|d| d.category == DiagnosticCategory::SkippedDecl),
        "expected a skipped-declaration diagnostic, got {:?}",
        ds.iter()
            .map(|d| (d.category, &d.message))
            .collect::<Vec<_>>()
    );
    assert!(
        module
            .decls
            .iter()
            .any(|d| matches!(d, Decl::Template(t) if t.name == "Good")),
        "template after the junk decl must still parse"
    );
}

#[test]
fn legacy_controller_can_is_unsupported_syntax() {
    let src = "module M where\n\
               template T\n  \
               with\n    o : Party\n  \
               where\n    signatory o\n    \
               controller o can\n      \
               Foo : ()\n        do pure ()\n";
    let ds = diags(src);
    assert!(
        ds.iter()
            .any(|d| d.category == DiagnosticCategory::UnsupportedSyntax),
        "legacy controller-can must be flagged unsupported, got {:?}",
        ds.iter()
            .map(|d| (d.category, &d.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn deep_nesting_emits_recursion_limit_and_does_not_panic() {
    // Well past MAX_DEPTH (128). The parser must not overflow the stack; it
    // degrades and reports the truncation.
    let depth = 300;
    let src = format!(
        "module M where\nf = {}1{}\n",
        "(".repeat(depth),
        ")".repeat(depth)
    );
    let ds = diags(&src);
    assert!(
        ds.iter()
            .any(|d| d.category == DiagnosticCategory::RecursionLimit),
        "deep nesting must report recursion-limit, got categories {:?}",
        ds.iter().map(|d| d.category).collect::<Vec<_>>()
    );
}

#[test]
fn each_deep_declaration_reports_its_own_recursion_limit() {
    // Two independently over-deep declarations must EACH report a truncation —
    // a later declaration's degraded region must not be silently dropped just
    // because an earlier one already tripped the limit.
    let nest = "(".repeat(300);
    let unnest = ")".repeat(300);
    let src = format!("module M where\ng = {nest}1{unnest}\nh = {nest}2{unnest}\n");
    let count = diags(&src)
        .iter()
        .filter(|d| d.category == DiagnosticCategory::RecursionLimit)
        .count();
    assert!(
        count >= 2,
        "each over-deep declaration must report its own recursion-limit; got {count}"
    );
}

#[test]
fn deep_pattern_nesting_does_not_panic() {
    let mut src = String::from("module M where\nf ");
    src.push_str(&"(Just ".repeat(2000));
    src.push('x');
    src.push_str(&")".repeat(2000));
    src.push_str(" = 1\n");

    let _ = parse_module(&src);
}

#[test]
fn hostile_incomplete_inputs_terminate() {
    for hostile in [
        "module M where\nf = \"never closed\ng = 2\n",
        "module M where\n{- never closed",
        "module M where\nf = (((((\n",
        "module M where\ntemplate T\n  with\n",
        "module M where\nf = do\n",
        "module M where\nf = let x = \n",
        "template",
        "",
        "\n\n\n",
        "-- only a comment\n",
        "\u{FEFF}module M where\nf = 1\n",
    ] {
        let _ = parse_module(hostile);
    }
}

#[test]
fn huge_single_line_expression_terminates() {
    let mut src = String::from("module M where\nf = ");
    for i in 0..20_000 {
        src.push_str(&format!("g{i} "));
    }
    src.push('\n');

    let (module, _) = parse_module(&src);
    assert!(
        module
            .decls
            .iter()
            .any(|decl| matches!(decl, Decl::Function(function) if function.name == "f")),
        "huge single-line expression should still produce the owning function"
    );
}

#[test]
fn large_module_parses_clean() {
    let mut src = String::from("module Big where\n\n");
    for i in 0..1000 {
        src.push_str(&format!(
            "template T{i}\n  with\n    owner : Party\n    amount : Decimal\n  where\n    signatory owner\n    ensure amount > 0.0\n\n    choice C{i} : ()\n      controller owner\n      do\n        pure ()\n\n"
        ));
    }
    assert!(src.lines().count() > 10_000);

    let (module, ds) = parse_module(&src);
    assert!(ds.is_empty(), "large module diagnostics: {ds:?}");
    let templates = module
        .decls
        .iter()
        .filter(|decl| matches!(decl, Decl::Template(_)))
        .count();
    assert_eq!(templates, 1000);
}

#[test]
fn lex_error_span_is_tab_correct() {
    // The lexer's columns are tab-aware; the byte span must replay that, so a
    // lex error on a tab-indented line points at the real offending byte (a
    // tab-naive mapping would land at EOF or mid-line).
    let src = "module M where\n\tx = \"unterminated\n";
    let ds = diags(src);
    let lex = ds
        .iter()
        .find(|d| d.category == DiagnosticCategory::Lex)
        .expect("lexical-error diagnostic");
    assert_eq!(
        &src[lex.span.start..lex.span.start + 1],
        "\"",
        "lex span must point at the opening quote, not a tab-naive offset"
    );
}

#[test]
fn diagnostic_span_pins_the_offending_token() {
    // The added byte span gives an END, not just a start: a skipped junk token
    // must have a non-empty span covering its bytes.
    let src = "module M where\n%%% junk\n";
    let ds = diags(src);
    let skipped = ds
        .iter()
        .find(|d| d.category == DiagnosticCategory::SkippedDecl)
        .expect("skipped-declaration diagnostic");
    assert!(
        skipped.span.end > skipped.span.start,
        "span must have a real end: {:?}",
        skipped.span
    );
    // The span points at the junk, not past it into the source.
    assert!(skipped.span.end <= src.len());
    assert_eq!(&src[skipped.span.start..skipped.span.end], "%%%");
}

#[test]
fn lexical_error_is_categorized_lex() {
    // An unterminated string is a lexer error: it must be carried through as a
    // `Lex` diagnostic with a position, not lost.
    let src = "module M where\nf = \"unterminated\n";
    let ds = diags(src);
    assert!(
        ds.iter().any(|d| d.category == DiagnosticCategory::Lex),
        "unterminated string must surface a lexical-error diagnostic, got {:?}",
        ds.iter()
            .map(|d| (d.category, &d.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn category_tags_are_stable_kebab_case() {
    // The machine-readable tags are a contract for JSON/SARIF consumers.
    assert_eq!(
        DiagnosticCategory::SkippedDecl.as_str(),
        "skipped-declaration"
    );
    assert_eq!(DiagnosticCategory::Malformed.as_str(), "malformed");
    assert_eq!(
        DiagnosticCategory::UnsupportedSyntax.as_str(),
        "unsupported-syntax"
    );
    assert_eq!(
        DiagnosticCategory::RecursionLimit.as_str(),
        "recursion-limit"
    );
    assert_eq!(DiagnosticCategory::Lex.as_str(), "lexical-error");
}
