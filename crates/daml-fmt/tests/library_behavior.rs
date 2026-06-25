//! Integration tests for the public `daml-fmt` formatter API: [`format_source`],
//! [`try_format_source`], diagnostics accessors, [`FormatOptions`], and
//! [`coverage`].

#![allow(clippy::unwrap_used)]

use daml_fmt::{
    coverage, format_source, lex_diagnostics, source_diagnostics, try_format_source, FormatOptions,
    ImportOrder,
};
use daml_parser::ast::DiagnosticCategory;
use daml_syntax::Coordinate;

#[test]
fn format_options_default_matches_new() {
    assert_eq!(FormatOptions::default(), FormatOptions::new());
}

#[test]
fn format_options_can_be_created_and_built() {
    let options = FormatOptions::new().with_import_order(ImportOrder::Preserve);

    assert_eq!(options.import_order(), ImportOrder::Preserve);
}

#[test]
fn format_coverage_counts_modeled_constructs_independently_of_edit_candidates() {
    let canonical = "module M where\nmain = do\n  pass\n";
    let canonical_coverage = coverage(canonical);
    assert!(
        canonical_coverage.modeled_constructs() >= 1,
        "do expressions are counted as modeled constructs"
    );

    let messy = "module M where\nmain = do\n    pass\n";
    let messy_coverage = coverage(messy);
    assert!(
        messy_coverage.edit_candidates() > 0,
        "over-indented do body should surface structural edit candidates"
    );
    assert!(messy_coverage.modeled_constructs() >= 1);
}

#[test]
fn clean_source_has_no_lex_diagnostics() {
    assert!(lex_diagnostics("module M where\nfoo : Int\nfoo = 1\n").is_empty());
}

#[test]
fn parser_diagnostics_are_reported() {
    let src = "module M where\nfoo = if x then 1\n";
    assert!(!source_diagnostics(src).is_empty());
}

#[test]
fn cpp_conditionals_do_not_surface_parser_recovery_diagnostics() {
    let src = "module A where\n#if defined(foo)\nmodule B where\n#else\nmodule C where\n#endif\n";
    assert!(source_diagnostics(src).is_empty());
}

#[test]
fn unterminated_string_is_diagnosed() {
    // Malformed input must be flagged so a format "success" is not mistaken
    // for parse success; output stays a verbatim passthrough.
    let src = "module M where\nx = \"oops\n";
    let diags = lex_diagnostics(src);
    assert!(!diags.is_empty(), "expected a diagnostic, got none");
    let diagnostic = &diags[0];
    assert_eq!(diagnostic.line().get(), 2);
    assert_eq!(diagnostic.column().get(), 5);
    assert_eq!(diagnostic.category(), DiagnosticCategory::Lex);
    assert!(diagnostic.message().contains("unterminated string"));
    assert_eq!(format_source(src), src); // byte-faithful passthrough
}

#[test]
fn parser_diagnostic_exposes_typed_fields() {
    let src = "module M where\nfoo = if x then 1\n";
    let diagnostic = source_diagnostics(src)
        .into_iter()
        .next()
        .expect("expected parser diagnostic");
    assert!(diagnostic.line().get() >= 1);
    assert!(diagnostic.column().get() >= 1);
    assert_ne!(diagnostic.category(), DiagnosticCategory::Lex);
    assert!(!diagnostic.message().is_empty());
    assert!(diagnostic
        .to_string()
        .contains(diagnostic.category().as_str()));
}

#[test]
fn try_format_rejects_malformed_input_and_accepts_valid_source() {
    let malformed = "module M where\nfoo = if x then 1\n";
    let err = try_format_source(malformed).expect_err("malformed input must fail");
    assert!(!err.diagnostics().is_empty());

    let valid = "module M where\nfoo: Int\nfoo = 1\n";
    let formatted = try_format_source(valid).expect("valid source must format");
    assert_eq!(formatted, "module M where\nfoo: Int\nfoo = 1\n");
}

#[test]
fn interior_blank_runs_collapse_to_one_blank_line() {
    let src = "module M where\n\n\n\nx = 1\n";
    assert_eq!(format_source(src), "module M where\n\nx = 1\n");
}

#[test]
fn gap_cases_format_to_expected_output() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/gap-cases");
    let bad_dir = root.join("bad");
    let good_dir = root.join("good");
    if !bad_dir.exists() || !good_dir.exists() {
        eprintln!("gap cases corpus missing (published crate test fixture), skipping");
        return;
    }
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&bad_dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "daml") {
            continue;
        }
        let name = path.file_name().unwrap();
        let bad = std::fs::read_to_string(&path).unwrap();
        let good = std::fs::read_to_string(good_dir.join(name)).unwrap();
        let formatted = format_source(&bad);
        assert_eq!(formatted, good, "gap fixture mismatch: {}", path.display());
        assert_eq!(
            format_source(&good),
            good,
            "gap fixture not idempotent: {}",
            path.display()
        );
        checked += 1;
    }
    assert_eq!(checked, 9, "unexpected gap fixture count");
}
