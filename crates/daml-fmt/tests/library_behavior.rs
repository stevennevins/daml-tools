//! Integration tests for the public `daml-fmt` formatter API: [`format_source`],
//! [`try_format_source`], diagnostics accessors, [`FormatOptions`], and
//! [`coverage`].

#![allow(clippy::unwrap_used)]

use daml_fmt::{
    coverage, format_source, format_source_with_options, lex_diagnostics, source_diagnostics,
    try_format_source, FormatDiagnostic, FormatError, FormatOptions, FormatRule, FormatRuleSet,
    ImportOrder,
};
use daml_parser::ast::DiagnosticCategory;

#[test]
fn format_options_default_matches_new() {
    assert_eq!(FormatOptions::default(), FormatOptions::new());
}

#[test]
fn import_order_default_and_display_are_stable_api_contracts() {
    fn assert_default<T: Default>() {}
    fn assert_display<T: std::fmt::Display>() {}

    assert_default::<ImportOrder>();
    assert_display::<ImportOrder>();
    assert_eq!(ImportOrder::default(), ImportOrder::Organize);
    assert_eq!(ImportOrder::Organize.to_string(), "organize");
    assert_eq!(ImportOrder::Preserve.to_string(), "preserve");
}

#[test]
fn format_options_can_be_created_and_built() {
    let options = FormatOptions::new().with_import_order(ImportOrder::Preserve);

    assert_eq!(options.import_order(), ImportOrder::Preserve);
}

#[test]
fn format_rules_have_stable_cli_ids() {
    assert_eq!("imports".parse::<FormatRule>(), Ok(FormatRule::Imports));
    assert_eq!("layout".parse::<FormatRule>(), Ok(FormatRule::Layout));
    assert_eq!("spacing".parse::<FormatRule>(), Ok(FormatRule::Spacing));
    assert_eq!(
        "syntax-normalization".parse::<FormatRule>(),
        Ok(FormatRule::SyntaxNormalization)
    );
    assert!("unknown-rule".parse::<FormatRule>().is_err());
}

#[test]
fn format_options_can_limit_formatter_to_selected_rules() {
    let src = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";
    let imports_only =
        FormatOptions::new().with_rules(FormatRuleSet::from_rules([FormatRule::Imports]));
    let spacing_only =
        FormatOptions::new().with_rules(FormatRuleSet::from_rules([FormatRule::Spacing]));

    assert_eq!(
        format_source_with_options(src, imports_only),
        "module M where\n\nimport DA.List\nimport DA.Optional\n\nfoo : Int\nfoo = 1\n"
    );
    assert_eq!(
        format_source_with_options(src, spacing_only),
        "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo: Int\nfoo = 1\n"
    );
}

#[test]
fn syntax_normalization_rule_does_not_apply_pure_layout_reindent() {
    let infix = "module M where\nfoo = a\n + b\n";
    let lambda = "module M where\nfoo = \\x ->\n x\n";
    let syntax_only = FormatOptions::new()
        .with_rules(FormatRuleSet::from_rules([FormatRule::SyntaxNormalization]));

    assert_eq!(format_source_with_options(infix, syntax_only), infix);
    assert_eq!(format_source_with_options(lambda, syntax_only), lambda);
}

#[test]
fn format_coverage_counts_modeled_constructs_independently_of_edit_candidates() {
    let canonical = "module M where\nmain = do\n  pass\n";
    let canonical_coverage = coverage(canonical).expect("canonical source coverage");
    assert!(
        canonical_coverage.modeled_constructs() >= 1,
        "do expressions are counted as modeled constructs"
    );

    let messy = "module M where\nmain = do\n    pass\n";
    let messy_coverage = coverage(messy).expect("messy source coverage");
    assert!(
        messy_coverage.edit_candidates() > 0,
        "over-indented do body should surface structural edit candidates"
    );
    assert!(messy_coverage.modeled_constructs() >= 1);
}

#[test]
fn coverage_rejects_malformed_input_with_typed_diagnostics() {
    let malformed = "module M where\nfoo = if x then 1\n";
    let err = coverage(malformed).expect_err("coverage must reject malformed input");
    let diagnostic = err
        .diagnostics()
        .first()
        .expect("malformed source must include a diagnostic");

    assert_ne!(diagnostic.category(), DiagnosticCategory::Lex);
    assert!(!diagnostic.message().is_empty());
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
fn format_error_exposes_diagnostics_through_as_ref() {
    fn diagnostics_slice(error: &impl AsRef<[FormatDiagnostic]>) -> &[FormatDiagnostic] {
        error.as_ref()
    }
    fn assert_as_ref<T: AsRef<[FormatDiagnostic]>>() {}

    assert_as_ref::<FormatError>();

    let malformed = "module M where\nfoo = if x then 1\n";
    let err = try_format_source(malformed).expect_err("malformed input must fail");
    assert_eq!(diagnostics_slice(&err), err.diagnostics());
    assert!(!diagnostics_slice(&err).is_empty());
}

#[test]
fn organize_imports_leaves_sorted_block_byte_identical() {
    // Already-canonical import order must round-trip without rewriting the
    // import block, even when extra blank lines sit between groups.
    let src =
        "module M where\n\nimport Daml.Script\n\nimport DA.List\nimport DA.Optional\n\nx = []\n";
    assert_eq!(format_source(src), src);
}

#[test]
fn organize_imports_groups_and_sorts_changed_blocks() {
    let src = "module M where\n\nimport My.App\nimport DA.Optional\nimport Daml.Script\nimport DA.List\n\nx = []\n";
    let want = "module M where\n\nimport Daml.Script\n\nimport DA.List\nimport DA.Optional\n\nimport My.App\n\nx = []\n";
    assert_eq!(format_source(src), want);
}

#[test]
fn interior_blank_runs_collapse_to_one_blank_line() {
    let src = "module M where\n\n\n\nx = 1\n";
    assert_eq!(format_source(src), "module M where\n\nx = 1\n");
}

/// Formatter gap-case fixtures (`tests/fixtures/gap-cases/`). Skips gracefully when
/// absent off the workspace (e.g. a published crate), but fails loud under CI
/// so a missing/forgotten fixture tree cannot pass green.
#[test]
fn gap_cases_format_to_expected_output() {
    let root =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/gap-cases");
    let bad_dir = root.join("bad");
    let good_dir = root.join("good");
    if !bad_dir.exists() || !good_dir.exists() {
        assert!(
            std::env::var_os("CI").is_none(),
            "gap cases corpus missing under CI (was it committed?): {}",
            root.display()
        );
        eprintln!("corpus absent (published crate?), skipping");
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
