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

fn get_function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
    module
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function declaration {name}"))
}

#[test]
fn fixity_declarations_are_structured_not_unknown() {
    let (module, diagnostics) = parse(
        "module M where
infixr 1 <=<, >=>
infix 4 ===
infixr 5 `Pair`
",
    );

    assert!(diagnostics.is_empty());
    assert_eq!(module.decls.len(), 3);
    let Decl::Fixity(first) = &module.decls[0] else {
        panic!("expected fixity declaration");
    };
    assert_eq!(first.assoc, FixityAssoc::InfixR);
    assert_eq!(first.precedence, 1);
    assert_eq!(first.operators.len(), 2);
    assert!(matches!(
        &first.operators[0],
        FixityTarget::Operator(op) if op.as_str() == "<=<"
    ));
    assert!(matches!(
        &first.operators[1],
        FixityTarget::Operator(op) if op.as_str() == ">=>"
    ));

    let Decl::Fixity(second) = &module.decls[1] else {
        panic!("expected fixity declaration");
    };
    assert_eq!(second.assoc, FixityAssoc::Infix);
    assert_eq!(second.precedence, 4);

    let Decl::Fixity(third) = &module.decls[2] else {
        panic!("expected fixity declaration");
    };
    assert!(matches!(
        &third.operators[0],
        FixityTarget::Backtick(name) if name.as_str() == "Pair"
    ));
    assert!(
        !module
            .decls
            .iter()
            .any(|d| matches!(d, Decl::Unknown { .. })),
        "stdlib-style fixity metadata must not degrade to Decl::Unknown"
    );
}

#[test]
fn operator_signatures_and_equations_are_function_decls() {
    let (module, diagnostics) = parse(
        "module M where
(>=>) : Action m => (a -> m b) -> (b -> m c) -> (a -> m c)
f >=> g = \\x -> f x >>= g
(<=<) = flip (>=>)
",
    );

    assert!(diagnostics.is_empty());
    let ge = get_function(&module, ">=>");
    assert!(ge.ty.as_type().is_some());
    assert_eq!(ge.equations.len(), 1);
    assert_eq!(ge.equations[0].params.len(), 2);
    assert!(matches!(
        &ge.equations[0].params[0],
        Pat::Var { name, .. } if name.as_str() == "f"
    ));
    assert!(matches!(
        &ge.equations[0].params[1],
        Pat::Var { name, .. } if name.as_str() == "g"
    ));

    let le = get_function(&module, "<=<");
    assert_eq!(le.equations.len(), 1);
    assert!(le.equations[0].params.is_empty());
    assert!(
        !module
            .decls
            .iter()
            .any(|d| matches!(d, Decl::Unknown { .. })),
        "operator declarations must not degrade to Decl::Unknown"
    );
}

fn get_template<'a>(module: &'a Module, name: &str) -> &'a TemplateDecl {
    module
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Template(t) if t.name.as_str() == name => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing template declaration {name}"))
}

fn get_template_choice<'a>(module: &'a Module, template: &str, choice: &str) -> &'a ChoiceDecl {
    get_template(module, template)
        .body
        .iter()
        .find_map(|d| match d {
            TemplateBodyDecl::Choice(c) if c.name.as_str() == choice => Some(c),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing choice {choice} on template {template}"))
}

const fn expr_var_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var { name, .. } => Some(name.as_str()),
        _ => None,
    }
}

#[test]
fn choice_metadata_supports_direct_where_and_authority_clauses() {
    let (module, diagnostics) = parse(
        "module M where
template T
  with
    p: Party
  where
    signatory p

    choice OldDirect : ()
      observer obs
      controller ctrl
      do pure ()

    choice BracedWhere : () where { controller ctrl } do pure ()

    choice LayoutWhere : ()
      where
        controller ctrl
      do pure ()

    choice WithAuthority : ()
      where
        authority auth
        controller ctrl
        observer obs
      do pure ()

    choice BracedAuthority : () where { observer obs; authority auth; controller ctrl } do pure ()
",
    );

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );

    let old = get_template_choice(&module, "T", "OldDirect");
    assert_eq!(old.observers.len(), 1);
    assert_eq!(expr_var_name(&old.observers[0]), Some("obs"));
    assert_eq!(old.controllers.len(), 1);
    assert_eq!(expr_var_name(&old.controllers[0]), Some("ctrl"));
    assert!(old.authority_exprs.is_empty());
    assert!(matches!(old.body.as_ref(), Some(Expr::Do { .. })));
    assert!(old.span.is_valid());

    let braced = get_template_choice(&module, "T", "BracedWhere");
    assert_eq!(braced.controllers.len(), 1);
    assert_eq!(expr_var_name(&braced.controllers[0]), Some("ctrl"));
    assert!(braced.observers.is_empty());
    assert!(matches!(braced.body.as_ref(), Some(Expr::Do { .. })));

    let layout = get_template_choice(&module, "T", "LayoutWhere");
    assert_eq!(layout.controllers.len(), 1);
    assert_eq!(expr_var_name(&layout.controllers[0]), Some("ctrl"));
    assert!(matches!(layout.body.as_ref(), Some(Expr::Do { .. })));

    let authority = get_template_choice(&module, "T", "WithAuthority");
    assert_eq!(expr_var_name(&authority.authority_exprs[0]), Some("auth"));
    assert_eq!(expr_var_name(&authority.controllers[0]), Some("ctrl"));
    assert_eq!(expr_var_name(&authority.observers[0]), Some("obs"));
    assert!(matches!(authority.body.as_ref(), Some(Expr::Do { .. })));

    let braced_auth = get_template_choice(&module, "T", "BracedAuthority");
    assert_eq!(expr_var_name(&braced_auth.observers[0]), Some("obs"));
    assert_eq!(expr_var_name(&braced_auth.authority_exprs[0]), Some("auth"));
    assert_eq!(expr_var_name(&braced_auth.controllers[0]), Some("ctrl"));
}

#[test]
fn pattern_synonyms_are_explicit_unsupported_syntax() {
    let (module, diagnostics) = parse(
        "module M where
pattern Nil = []
",
    );

    assert_eq!(module.decls.len(), 1);
    let Decl::UnsupportedSyntax {
        kind: UnsupportedSyntaxKind::PatternSynonym,
        ..
    } = &module.decls[0]
    else {
        panic!("pattern synonyms must surface as explicit unsupported syntax");
    };
    assert!(
        diagnostics.is_empty(),
        "explicit unsupported AST nodes should not make lossless corpus files diagnostically invalid"
    );
}
