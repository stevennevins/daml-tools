//! Integration tests for constructor record pattern field structure.

use daml_parser::ast::*;
use daml_parser::parse::parse_module;

fn parse(src: &str) -> (Module, Vec<ParseDiagnostic>) {
    parse_module(src).into_parts()
}

fn case_alt_pattern<'a>(module: &'a Module, function: &str) -> &'a Pat {
    let Expr::Case { alts, .. } = function_body(module, function) else {
        panic!("expected case expression for {function}");
    };
    &alts[0].pat
}

fn function_body<'a>(module: &'a Module, name: &str) -> &'a Expr {
    module
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == name => Some(&f.equations[0].body),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function {name}"))
}

#[test]
fn brace_record_pattern_preserves_fields() {
    let (module, diagnostics) = parse(
        "module M where
f = case x of
  Foo { x, y = z, .. } -> ()
",
    );

    assert!(diagnostics.is_empty());
    let Pat::Record {
        qualifier,
        name,
        syntax,
        fields,
        ..
    } = case_alt_pattern(&module, "f")
    else {
        panic!("expected record pattern");
    };
    assert_eq!(qualifier.as_ref(), None);
    assert_eq!(name.as_str(), "Foo");
    assert_eq!(*syntax, RecordPatternSyntax::Braces);
    assert_eq!(fields.len(), 3);
    assert!(matches!(
        &fields[0],
        PatFieldAssign::Pun { name, .. } if name.as_str() == "x"
    ));
    assert!(matches!(
        &fields[1],
        PatFieldAssign::Assign { name, pat: Pat::Var { name: bound, .. }, .. }
            if name.as_str() == "y" && bound.as_str() == "z"
    ));
    assert!(matches!(&fields[2], PatFieldAssign::Wildcard { .. }));
}

#[test]
fn with_record_pattern_preserves_fields() {
    let (module, diagnostics) = parse(
        "module M where
f = case x of
  Foo with claim; tag = Some t -> ()
",
    );

    assert!(diagnostics.is_empty());
    let Pat::Record {
        name,
        syntax,
        fields,
        ..
    } = case_alt_pattern(&module, "f")
    else {
        panic!("expected record pattern");
    };
    assert_eq!(name.as_str(), "Foo");
    assert_eq!(*syntax, RecordPatternSyntax::With);
    assert_eq!(fields.len(), 2);
    assert!(matches!(
        &fields[0],
        PatFieldAssign::Pun { name, .. } if name.as_str() == "claim"
    ));
    assert!(matches!(
        &fields[1],
        PatFieldAssign::Assign {
            name,
            pat: Pat::Con { name: con, args, .. },
            ..
        } if name.as_str() == "tag" && con.as_str() == "Some" && args.len() == 1
    ));
}

#[test]
fn qualified_constructor_record_pattern_is_preserved() {
    let (module, diagnostics) = parse(
        "module M where
f = case x of
  Mod.Foo { field = _ } -> ()
",
    );

    assert!(diagnostics.is_empty());
    let Pat::Record {
        qualifier,
        name,
        fields,
        ..
    } = case_alt_pattern(&module, "f")
    else {
        panic!("expected record pattern");
    };
    assert_eq!(qualifier.as_ref().map(|q| q.as_str()), Some("Mod"));
    assert_eq!(name.as_str(), "Foo");
    assert!(matches!(
        &fields[0],
        PatFieldAssign::Assign { name, pat: Pat::Wild { .. }, .. } if name.as_str() == "field"
    ));
}

#[test]
fn positional_constructor_pattern_stays_con() {
    let (module, diagnostics) = parse(
        "module M where
f = case x of
  Foo bar baz -> ()
",
    );

    assert!(diagnostics.is_empty());
    let Pat::Con { name, args, .. } = case_alt_pattern(&module, "f") else {
        panic!("expected positional constructor pattern");
    };
    assert_eq!(name.as_str(), "Foo");
    assert_eq!(args.len(), 2);
    assert!(matches!(&args[0], Pat::Var { name, .. } if name.as_str() == "bar"));
    assert!(matches!(&args[1], Pat::Var { name, .. } if name.as_str() == "baz"));
}

#[test]
fn record_pattern_fields_have_spans() {
    let source = "module M where\nf = case x of\n  Foo { x } -> ()\n";
    let (module, diagnostics) = parse(source);

    assert!(diagnostics.is_empty());
    let pat = case_alt_pattern(&module, "f");
    let Pat::Record { span, fields, .. } = pat else {
        panic!("expected record pattern");
    };
    assert!(span.is_valid());
    assert!(span.contains(&fields[0].span()));
    assert_eq!(pat.render(), "Foo { x }");
}
