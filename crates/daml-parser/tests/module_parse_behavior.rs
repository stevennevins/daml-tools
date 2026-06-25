//! Integration tests for module-level parse behavior observable through
//! [`daml_parser::parse::parse_module`].

use daml_parser::ast::*;
use daml_parser::parse::parse_module;

fn parse(src: &str) -> (Module, Vec<ParseDiagnostic>) {
    parse_module(src).into_parts()
}

fn get_first_equation_body<'a>(module: &'a Module, name: &str) -> &'a Expr {
    let function = module
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function declaration {name}"));
    let first_equation = function
        .equations
        .first()
        .unwrap_or_else(|| panic!("missing equation for function {name}"));
    &first_equation.body
}

#[test]
fn import_style_distinguishes_qualified_prefix_and_postfix() {
    let (module, diagnostics) = parse(
        "module M where
import qualified Foo.Bar as FB
import DA.Map qualified as Map
import Baz as B",
    );

    assert!(diagnostics.is_empty());
    assert_eq!(
        module.imports.iter().map(|i| i.style).collect::<Vec<_>>(),
        vec![
            ImportStyle::Qualified,
            ImportStyle::Qualified,
            ImportStyle::Unqualified,
        ]
    );
}

#[test]
fn expression_sections_use_distinct_ast_shapes() {
    let (module, diagnostics) = parse(
        "module M where
f = (+ 1)
g = (+)
",
    );

    assert!(diagnostics.is_empty());
    assert!(matches!(
        get_first_equation_body(&module, "f"),
        Expr::RightSection { .. }
    ));
    assert!(matches!(
        get_first_equation_body(&module, "g"),
        Expr::OperatorRef { .. }
    ));
}

#[test]
fn record_fields_use_explicit_ast_shapes() {
    let (module, diagnostics) = parse(
        "module M where
f owner = T with owner; count = 1; ..
",
    );

    assert!(diagnostics.is_empty());
    let Expr::Record { fields, .. } = get_first_equation_body(&module, "f") else {
        panic!("expected record expression");
    };
    assert!(matches!(
        &fields[0],
        FieldAssign::Pun { name, .. } if name.as_str() == "owner"
    ));
    assert!(matches!(
        &fields[1],
        FieldAssign::Assign { name, value: Expr::Lit { .. }, .. } if name.as_str() == "count"
    ));
    assert!(matches!(&fields[2], FieldAssign::Wildcard { .. }));
    assert_eq!(
        fields[2].name(),
        None,
        "wildcards must not use '..' as a fake field name"
    );
}

#[test]
fn do_expr_is_allowed_for_top_level_expression_parsing() {
    let (module, diagnostics) = parse(
        "module M where
f = do
  pure True
",
    );

    assert!(diagnostics.is_empty());
    assert!(matches!(
        get_first_equation_body(&module, "f"),
        Expr::Do { .. }
    ));
}

#[test]
fn do_expr_is_disallowed_for_case_scrutinee_parsing() {
    let (module, diagnostics) = parse(
        "module M where
f = case do 1 of
  x -> x
",
    );

    assert!(diagnostics
        .iter()
        .any(|d| d.message == "expected 'of' in case expression"));
    assert!(matches!(
        get_first_equation_body(&module, "f"),
        Expr::Error { .. }
    ));
}
