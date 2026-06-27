//! Counterexample coverage for source syntax added in the parser audit sweep.
//!
//! These tests pin adjacent syntax that must *not* be over-classified as one of
//! the newly supported shapes.

use daml_parser::ast::*;
use daml_parser::parse::parse_module;

fn parse(src: &str) -> (Module, Vec<ParseDiagnostic>) {
    parse_module(src).into_parts()
}

fn function_body<'a>(module: &'a Module, name: &str) -> &'a Expr {
    module
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Function(function) if function.name == name => Some(&function.equations[0].body),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function {name}"))
}

fn first_template<'a>(module: &'a Module, name: &str) -> &'a TemplateDecl {
    module
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Template(template) if template.name.as_str() == name => Some(template),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing template {name}"))
}

fn first_choice<'a>(module: &'a Module, template: &str, choice: &str) -> &'a ChoiceDecl {
    first_template(module, template)
        .body
        .iter()
        .find_map(|decl| match decl {
            TemplateBodyDecl::Choice(choice_decl) if choice_decl.name.as_str() == choice => {
                Some(choice_decl)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing choice {choice}"))
}

fn first_interface_instance(module: &Module) -> &InterfaceInstanceDecl {
    first_template(module, "Asset")
        .body
        .iter()
        .find_map(|decl| match decl {
            TemplateBodyDecl::InterfaceInstance(instance) => Some(instance),
            _ => None,
        })
        .expect("missing interface instance")
}

fn as_binop<'a>(expr: &'a Expr, op: &str) -> (&'a Expr, &'a Expr) {
    match expr {
        Expr::BinOp {
            op: actual,
            lhs,
            rhs,
            ..
        } if actual.as_str() == op => (lhs.as_ref(), rhs.as_ref()),
        other => panic!("expected BinOp {op}, got {other:?}"),
    }
}

#[test]
fn choice_without_metadata_does_not_fabricate_authority_or_observers() {
    let (module, diagnostics) = parse(
        "module M where
template T
  with
    p: Party
  where
    signatory p

    choice Plain : ()
      do pure ()
",
    );

    assert!(diagnostics.is_empty());
    let choice = first_choice(&module, "T", "Plain");
    assert!(choice.controllers.is_empty());
    assert!(choice.observers.is_empty());
    assert!(choice.authority_exprs.is_empty());
    assert!(matches!(choice.body.as_ref(), Some(Expr::Do { .. })));
}

#[test]
fn only_exact_interface_instance_view_binding_is_distinguished() {
    let (module, diagnostics) = parse(
        "module M where
template Asset
  with
    owner : Party
  where
    signatory owner
    interface instance Token for Asset where
      viewed = owner
      view = EmptyInterfaceView
      review = owner
",
    );

    assert!(diagnostics.is_empty());
    let instance = first_interface_instance(&module);
    assert_eq!(instance.items.len(), 3);
    assert!(matches!(
        &instance.items[0],
        InterfaceInstanceBodyItem::Method(Binding { pat: Pat::Var { name, .. }, .. })
            if name.as_str() == "viewed"
    ));
    assert!(matches!(
        &instance.items[1],
        InterfaceInstanceBodyItem::View { .. }
    ));
    assert!(matches!(
        &instance.items[2],
        InterfaceInstanceBodyItem::Method(Binding { pat: Pat::Var { name, .. }, .. })
            if name.as_str() == "review"
    ));
}

#[test]
fn unguarded_case_alternatives_remain_single_unguarded_branches() {
    let (module, diagnostics) = parse(
        "module M where
f x = case x of
  Some y -> y
  None -> 0
",
    );

    assert!(diagnostics.is_empty());
    let Expr::Case { alts, .. } = function_body(&module, "f") else {
        panic!("expected case expression");
    };
    assert_eq!(alts.len(), 2);
    for alt in alts {
        assert_eq!(alt.branches.len(), 1);
        assert!(alt.branches[0].guards.is_empty());
        assert!(alt.where_bindings.is_empty());
        assert_eq!(alt.body, alt.branches[0].body);
    }
}

#[test]
fn undeclared_operator_keeps_default_grouping() {
    let (module, diagnostics) = parse(
        "module M where
f = a + b %% c
",
    );

    assert!(diagnostics.is_empty());
    let (plus_lhs, plus_rhs) = as_binop(function_body(&module, "f"), "+");
    assert!(matches!(plus_lhs, Expr::Var { name, .. } if name.as_str() == "a"));
    let (pct_lhs, pct_rhs) = as_binop(plus_rhs, "%%");
    assert!(matches!(pct_lhs, Expr::Var { name, .. } if name.as_str() == "b"));
    assert!(matches!(pct_rhs, Expr::Var { name, .. } if name.as_str() == "c"));
}

#[test]
fn record_expression_and_positional_pattern_do_not_become_record_patterns() {
    let (module, diagnostics) = parse(
        "module M where
make x = Foo with field = x
match y = case y of
  Foo a b -> a
",
    );

    assert!(diagnostics.is_empty());
    assert!(matches!(
        function_body(&module, "make"),
        Expr::Record { .. }
    ));
    let Expr::Case { alts, .. } = function_body(&module, "match") else {
        panic!("expected case expression");
    };
    assert!(matches!(
        &alts[0].pat,
        Pat::Con { name, args, .. } if name.as_str() == "Foo" && args.len() == 2
    ));
}

#[test]
fn ordinary_imports_have_no_package_label() {
    let (module, diagnostics) = parse(
        "module M where
import qualified Foo.Bar as FB
import DA.Map qualified as Map
",
    );

    assert!(diagnostics.is_empty());
    assert_eq!(module.imports.len(), 2);
    assert!(module
        .imports
        .iter()
        .all(|import| import.package_label.is_none()));
    assert_eq!(module.imports[0].style, ImportStyle::Qualified);
    assert_eq!(module.imports[1].style, ImportStyle::Qualified);
}
