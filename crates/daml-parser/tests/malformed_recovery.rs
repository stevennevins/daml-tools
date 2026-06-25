//! Integration tests for malformed-input recovery through public `parse_module`.

use daml_parser::ast::{Decl, DiagnosticCategory, TemplateBodyDecl, Type};
use daml_parser::parse::parse_module;

#[test]
fn type_literal_in_function_signature_is_not_malformed() {
    let src = r#"module M where
f : HasField "observers" t PartiesMap => ()
  = ()
"#;
    let (module, diagnostics) = parse_module(src).into_parts();
    assert!(
        !diagnostics
            .iter()
            .any(|d| d.message.contains("malformed function type annotation")),
        "constraint with type string literal must parse cleanly: {diagnostics:#?}"
    );
    let function = match &module.decls[0] {
        Decl::Function(f) => f,
        other => panic!("expected function, got {other:?}"),
    };
    let ty = function.ty.as_type().expect("function signature type");
    assert!(matches!(
        ty,
        Type::Constrained(body, _)
            if matches!(
                &**body,
                Type::Unit(_)
            )
    ));
}

#[test]
fn interface_instance_missing_template_after_for_is_malformed() {
    let src = r#"module M where
template Account
  with
    owner : Party
  where
    signatory owner
    interface instance Disclosure.I for where
      disclose = owner
"#;
    let (module, diagnostics) = parse_module(src).into_parts();
    assert!(
        diagnostics.iter().any(|d| {
            d.category == DiagnosticCategory::Malformed
                && d.message == "interface instance missing template name after 'for'"
        }),
        "expected malformed diagnostic for missing template after 'for', got {diagnostics:?}"
    );
    let template = match &module.decls[0] {
        Decl::Template(t) => t,
        other => panic!("expected template, got {other:?}"),
    };
    let instance = template
        .body
        .iter()
        .find_map(|decl| match decl {
            TemplateBodyDecl::InterfaceInstance(ii) => Some(ii),
            _ => None,
        })
        .expect("parser should still recover an interface instance node");
    assert!(instance.for_template.is_none());
}

#[test]
fn malformed_guarded_equation_reports_missing_equals_and_continues() {
    let src = "module M where\nf x | x > 0\ng = 1\n";
    let (module, diagnostics) = parse_module(src).into_parts();

    assert!(
        diagnostics.iter().any(
            |diagnostic| diagnostic.message == "expected '=' after guard"
                && diagnostic.category == DiagnosticCategory::Malformed
        ),
        "expected guard diagnostic, got {diagnostics:?}"
    );
    assert!(
        module
            .decls
            .iter()
            .any(|decl| matches!(decl, Decl::Function(function) if function.name == "g")),
        "parser should recover to the following declaration: {:?}",
        module.decls
    );
}

#[test]
fn malformed_brackets_do_not_underflow_recovery_scans() {
    let src = "module M where\ntemplate T\n  with\n    owner : Party\n  where\n    key owner ) : Party\n    maintainer owner\n\nf = (]\ng = 1\n";
    let (module, _diagnostics) = parse_module(src).into_parts();

    assert_eq!(module.name, "M");
    assert!(
        module
            .decls
            .iter()
            .any(|decl| matches!(decl, Decl::Function(function) if function.name == "g")),
        "parser should recover to the following declaration: {:?}",
        module.decls
    );
}
