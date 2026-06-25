//! Integration tests for strict vs tolerant `parse_module` contract.

use daml_parser::ast::{Decl, DiagnosticCategory};
use daml_parser::parse::{parse_module, parse_module_strict};

#[test]
fn strict_parse_accepts_clean_module() {
    let src = "module M where\nfoo : Int\nfoo = 1\n";
    let module = parse_module_strict(src).expect("clean source must parse strictly");
    assert_eq!(module.name, "M");
}

#[test]
fn strict_parse_rejects_malformed_source_while_tolerant_parse_keeps_diagnostics() {
    let src = "module M where\nf x | x > 0\ng = 1\n";
    let tolerant = parse_module(src);
    assert!(
        tolerant
            .diagnostics
            .iter()
            .any(|d| d.category == DiagnosticCategory::Malformed),
        "tolerant parse must record malformed guard without aborting"
    );

    let err = parse_module_strict(src).expect_err("malformed guard must fail strict parse");
    assert_eq!(err.diagnostics(), &tolerant.diagnostics);
    assert!(
        err.module()
            .decls
            .iter()
            .any(|decl| matches!(decl, Decl::Function(f) if f.name == "g")),
        "strict error should still carry the partial module tolerant parsing produced"
    );
}

#[test]
fn into_result_matches_parse_module_strict() {
    let src = "module M where\n%%% junk\n";
    let tolerant = parse_module(src);
    assert!(!tolerant.diagnostics.is_empty());

    let strict = parse_module_strict(src).expect_err("junk decl must fail strict parse");
    let converted = tolerant
        .into_result()
        .expect_err("same diagnostics via into_result");
    assert_eq!(strict.diagnostics(), converted.diagnostics());
}
