//! Package-qualified imports preserve the source package string literal.

use daml_parser::ast::*;
use daml_parser::ast_span::render_from_ast;
use daml_parser::lexer::lex_with_trivia;
use daml_parser::parse::parse_module;

fn parse(src: &str) -> (Module, Vec<ParseDiagnostic>) {
    parse_module(src).into_parts()
}

#[test]
fn package_qualified_import_preserves_label_module_and_style() {
    let src = "module M where
import \"foo\" X
import qualified \"bar-baz\" Y.Z as FooZ
import \"pkg\" DA.Map qualified as Map
";
    let (module, diagnostics) = parse(src);

    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    assert_eq!(module.imports.len(), 3);

    let first = &module.imports[0];
    assert_eq!(first.module_name.as_str(), "X");
    assert_eq!(first.style, ImportStyle::Unqualified);
    assert!(first.alias.is_none());
    let label = first.package_label.as_ref().expect("package label");
    assert_eq!(label.value, "foo");
    assert_eq!(
        label.span.get(src).expect("package label span in source"),
        "\"foo\""
    );

    let second = &module.imports[1];
    assert_eq!(second.module_name.as_str(), "Y.Z");
    assert_eq!(second.style, ImportStyle::Qualified);
    assert_eq!(second.alias.as_ref().map(|a| a.as_str()), Some("FooZ"));
    assert_eq!(
        second.package_label.as_ref().map(|l| l.value.as_str()),
        Some("bar-baz")
    );

    let third = &module.imports[2];
    assert_eq!(third.module_name.as_str(), "DA.Map");
    assert_eq!(third.style, ImportStyle::Qualified);
    assert_eq!(third.alias.as_ref().map(|a| a.as_str()), Some("Map"));
    assert_eq!(
        third.package_label.as_ref().map(|l| l.value.as_str()),
        Some("pkg")
    );
}

#[test]
fn render_from_ast_roundtrips_package_qualified_import() {
    let source = "module M where\nimport qualified \"upgrades-example\" Main as V1\n";
    let (module, diagnostics) = parse(source);
    assert!(diagnostics.is_empty());
    let (_, trivia, _) = lex_with_trivia(source).into_parts();
    assert_eq!(
        render_from_ast(source, &module, &trivia).as_deref(),
        Ok(source)
    );
    let label = module.imports[0]
        .package_label
        .as_ref()
        .expect("package label span");
    assert_eq!(
        &source[label.span.start_usize()..label.span.end_usize()],
        "\"upgrades-example\""
    );
}
