//! Integration tests for parse diagnostics: recovery categories, byte-span end
//! positions, and the guarantee that a malformed item does not abort later
//! declarations.

use daml_parser::ast::{Decl, DiagnosticCategory};
use daml_parser::parse::{parse_module, MAX_RECURSION_DEPTH};

const TEST_RECURSION_DEPTH: usize = MAX_RECURSION_DEPTH as usize + 172;

fn diags(src: &str) -> Vec<daml_parser::ast::ParseDiagnostic> {
    parse_module(src).diagnostics
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
    let (module, ds) = parse_module(src).into_parts();
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
fn legacy_controller_can_syntax_is_unsupported() {
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
fn malformed_expression_is_categorized_malformed() {
    // The common recovery case: a recognized declaration whose body is broken.
    // `if x then 1` is missing its `else`, so the parser emits a `Malformed`
    // diagnostic (not `SkippedDecl`/`UnsupportedSyntax`) yet the `f` declaration
    // is still produced. `Malformed` is the most-emitted category and must have
    // a behavior test, not only an `as_str` string-mapping check.
    let src = "module M where\nf = if x then 1\n";
    let (module, ds) = parse_module(src).into_parts();
    assert!(
        ds.iter()
            .any(|d| d.category == DiagnosticCategory::Malformed),
        "missing `else` must surface a Malformed diagnostic, got {:?}",
        ds.iter()
            .map(|d| (d.category, &d.message))
            .collect::<Vec<_>>()
    );
    assert!(
        module
            .decls
            .iter()
            .any(|d| matches!(d, Decl::Function(f) if f.name == "f")),
        "the malformed body must not drop the surrounding `f` declaration"
    );
}

#[test]
fn malformed_type_annotation_is_reported_as_malformed() {
    let src = r#"module M where
template T
  with
    owner : %
  where
    signatory owner
"#;
    let (module, ds) = parse_module(src).into_parts();
    let malformed = ds
        .iter()
        .find(|d| {
            d.message == "malformed field type annotation"
                && d.category == DiagnosticCategory::Malformed
        })
        .or_else(|| {
            ds.iter().find(|d| {
                d.category == DiagnosticCategory::Malformed
                    && d.message.contains("field type annotation")
            })
        })
        .expect("malformed type annotation diagnostic");
    assert_eq!(
        malformed.span.get(src),
        Some("%"),
        "diagnostic should pin malformed field annotation token"
    );
    assert!(module
        .decls
        .iter()
        .any(|decl| matches!(decl, Decl::Template(template) if template.name == "T")),);
}

#[test]
fn malformed_type_annotation_is_not_reported_for_function_signature_with_body() {
    let src = r#"module M where
f : Int = 1
"#;
    let (module, ds) = parse_module(src).into_parts();
    assert!(
        !ds.iter()
            .any(|d| d.message == "malformed function type annotation"),
        "a typed function declaration with a body should remain parseable: {ds:#?}",
    );
    assert!(module
        .decls
        .iter()
        .any(|decl| matches!(decl, Decl::Function(function) if function.name == "f")),);
}

#[test]
fn malformed_field_annotation_is_not_reported_for_inline_fields() {
    let src = r#"module M where
template T
  with
    a : Int, b : Int
    c : Text; d : Text
  where
    signatory a
"#;
    let (_module, ds) = parse_module(src).into_parts();
    assert!(
        !ds.iter()
            .any(|d| d.message == "malformed field type annotation"),
        "inline field declarations should not emit malformed type diagnostics: {ds:#?}",
    );
}

#[test]
fn malformed_type_annotation_is_not_reported_for_section_constructor() {
    let src = r#"module M where
t : (,) Int Text = (1, "a")
"#;
    let (_module, ds) = parse_module(src).into_parts();
    assert!(
        !ds.iter()
            .any(|d| d.message == "malformed function type annotation"),
        "section constructor type annotations should be tolerated: {ds:#?}",
    );
}

#[test]
fn malformed_section_constructor_with_unclosed_list_is_reported() {
    let src = r#"module M where
f : (,) Int [Text
"#;
    let (_module, ds) = parse_module(src).into_parts();
    assert!(
        ds.iter()
            .any(|d| d.message == "malformed function type annotation"),
        "unclosed list argument in section constructor should be malformed: {ds:#?}",
    );
}

#[test]
fn malformed_section_constructor_with_unclosed_paren_is_reported() {
    let src = r#"module M where
f : (,) (Int
"#;
    let (_module, ds) = parse_module(src).into_parts();
    assert!(
        ds.iter()
            .any(|d| d.message == "malformed function type annotation"),
        "unclosed type argument paren in section constructor should be malformed: {ds:#?}",
    );
}

#[test]
fn malformed_section_constructor_with_trailing_comma_argument_is_reported() {
    let src = r#"module M where
f : (,) Int (Text,)
"#;
    let (_module, ds) = parse_module(src).into_parts();
    assert!(
        ds.iter()
            .any(|d| d.message == "malformed function type annotation"),
        "section-constructor argument `(Text,)` must be rejected: {ds:#?}",
    );
}

#[test]
fn deep_nesting_emits_recursion_limit_and_does_not_panic() {
    // Well past the parser's recursion bound. The parser must not overflow the
    // stack; it degrades and reports the truncation.
    let depth = TEST_RECURSION_DEPTH;
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
    let nest = "(".repeat(TEST_RECURSION_DEPTH);
    let unnest = ")".repeat(TEST_RECURSION_DEPTH);
    let src = format!("module M where\ng = {nest}1{unnest}\nh = {nest}2{unnest}\n");
    let diagnostics = diags(&src);
    let recursion_limit_spans = diagnostics
        .iter()
        .filter(|d| d.category == DiagnosticCategory::RecursionLimit)
        .map(|d| d.span)
        .collect::<Vec<_>>();
    let g_start = src.find("g = ").expect("g declaration start");
    let h_start = src.find("h = ").expect("h declaration start");
    assert!(
        recursion_limit_spans
            .iter()
            .any(|span| span.start >= g_start && span.start < h_start),
        "g declaration must report its own recursion-limit, got {recursion_limit_spans:?}"
    );
    assert!(
        recursion_limit_spans
            .iter()
            .any(|span| span.start >= h_start),
        "h declaration must report its own recursion-limit, got {recursion_limit_spans:?}"
    );
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
    let lex = ds
        .into_iter()
        .find(|d| d.category == DiagnosticCategory::Lex)
        .expect("unterminated-string lexical diagnostic");
    assert!(
        lex.span.end > lex.span.start,
        "lex span should be non-zero: {:?}",
        lex.span
    );
    assert_eq!(
        &src[lex.span.start..lex.span.end],
        "\"",
        "unterminated string span must point at opening quote"
    );

    let src = "module M where\nf = \"bad \\q\"\n";
    let ds = diags(src);
    let lex = ds
        .iter()
        .find(|d| d.category == DiagnosticCategory::Lex)
        .expect("invalid-escape lexical diagnostic");
    assert!(
        lex.span.end > lex.span.start,
        "invalid-escape lex span should be non-zero: {:?}",
        lex.span
    );
    assert_eq!(&src[lex.span.start..lex.span.end], "\\q");
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
