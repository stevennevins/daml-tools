//! Integration tests for type annotation wiring through full module parsing.

use daml_parser::ast::*;
use daml_parser::parse::parse_module;

fn con(name: &str) -> Type {
    Type::Con {
        qualifier: None,
        name: name.into(),
        span: Span::default(),
    }
}

fn app(head: Type, args: Vec<Type>) -> Type {
    Type::App(Box::new(head), args, Span::default())
}

#[test]
fn ty_is_populated_through_real_parse() {
    let src = r#"module M where
template T
  with
    owner : Party
    held : ContractId Asset
  where
    signatory owner
    choice Go : Optional (ContractId Asset)
      controller owner
      do
        pure None
"#;
    let (m, _) = parse_module(src).into_parts();
    let t = match &m.decls[0] {
        Decl::Template(t) => t,
        other => panic!("expected template, got {other:?}"),
    };
    assert_eq!(t.fields[0].ty, TypeAnnotation::Present(con("Party")));
    assert_eq!(
        t.fields[1].ty,
        TypeAnnotation::Present(app(con("ContractId"), vec![con("Asset")]))
    );
    let choice = match &t
        .body
        .iter()
        .find(|d| matches!(d, TemplateBodyDecl::Choice(_)))
    {
        Some(TemplateBodyDecl::Choice(c)) => (*c).clone(),
        _ => panic!("expected choice"),
    };
    assert_eq!(
        choice.return_ty,
        TypeAnnotation::Present(app(
            con("Optional"),
            vec![app(con("ContractId"), vec![con("Asset")])]
        ))
    );
}

#[test]
fn ty_is_populated_on_key_and_interface_method() {
    let src = r#"module M where
template T
  with
    owner : Party
  where
    signatory owner
    key owner : Party
    maintainer owner

interface I where
  getAmount : Numeric 10
"#;
    let (m, _) = parse_module(src).into_parts();
    let t = match &m.decls[0] {
        Decl::Template(t) => t,
        other => panic!("expected template, got {other:?}"),
    };
    let key_ty = t.body.iter().find_map(|d| match d {
        TemplateBodyDecl::Key { ty, .. } => Some(ty.clone()),
        _ => None,
    });
    assert_eq!(key_ty, Some(TypeAnnotation::Present(con("Party"))));

    let iface = match &m.decls[1] {
        Decl::Interface(i) => i,
        other => panic!("expected interface, got {other:?}"),
    };
    assert_eq!(iface.methods[0].ty, TypeAnnotation::Present(con("Numeric")));
}

#[test]
fn interface_instance_with_for_sets_explicit_template() {
    let src = r#"module M where
template Account
  with
    owner : Party
  where
    signatory owner
    interface instance Disclosure.I for Account where
      disclose = owner
"#;
    let (module, diagnostics) = parse_module(src).into_parts();
    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
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
        .expect("template body should contain an interface instance");
    assert_eq!(instance.interface_name.as_str(), "Disclosure.I");
    assert_eq!(
        instance.for_template.as_ref().map(ModuleName::as_str),
        Some("Account")
    );
}

#[test]
fn interface_instance_without_for_leaves_template_absent() {
    let src = r#"module M where
template Account
  with
    owner : Party
  where
    signatory owner
    interface instance Disclosure.I where
      disclose = owner
"#;
    let (module, diagnostics) = parse_module(src).into_parts();
    assert!(
        diagnostics.is_empty(),
        "omitted 'for' is valid inside a template, got {diagnostics:?}"
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
        .expect("template body should contain an interface instance");
    assert_eq!(instance.interface_name.as_str(), "Disclosure.I");
    assert!(
        instance.for_template.is_none(),
        "omitted 'for' must be None, not an empty ModuleName sentinel"
    );
}

#[test]
fn headerless_file_keeps_legacy_unknown_name_fallback() {
    let (module, _diagnostics) = parse_module("f = 1\n").into_parts();

    assert_eq!(module.name, "Unknown");
    assert!(
        module
            .decls
            .iter()
            .any(|decl| matches!(decl, Decl::Function(function) if function.name == "f")),
        "expected function declaration to be parsed: {:?}",
        module.decls
    );
}
