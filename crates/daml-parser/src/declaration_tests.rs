//! Public AST behavior for declarations, templates, interfaces, and choices.
//!
//! These tests pin parser-owned structure that downstream crates consume. They
//! intentionally enter through `parse_module` and assert public AST fields only.

#![cfg(test)]

use crate::ast::*;
use crate::parse::parse_module;

fn parse_ok(src: &str) -> Module {
    let (module, diagnostics) = parse_module(src);
    assert!(
        diagnostics.is_empty(),
        "unexpected diagnostics: {diagnostics:?}"
    );
    module
}

fn text(src: &str, span: Span) -> &str {
    &src[span.start..span.end]
}

fn template<'a>(module: &'a Module, name: &str) -> &'a TemplateDecl {
    module
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Template(template) if template.name == name => Some(template),
            _ => None,
        })
        .unwrap_or_else(|| panic!("template {name} not found"))
}

fn interface<'a>(module: &'a Module, name: &str) -> &'a InterfaceDecl {
    module
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Interface(interface) if interface.name == name => Some(interface),
            _ => None,
        })
        .unwrap_or_else(|| panic!("interface {name} not found"))
}

fn choices(template: &TemplateDecl) -> Vec<&ChoiceDecl> {
    template
        .body
        .iter()
        .filter_map(|item| match item {
            TemplateBodyDecl::Choice(choice) => Some(choice),
            _ => None,
        })
        .collect()
}

fn choice<'a>(choices: &'a [&ChoiceDecl], name: &str) -> &'a ChoiceDecl {
    choices
        .iter()
        .copied()
        .find(|choice| choice.name == name)
        .unwrap_or_else(|| panic!("choice {name} not found"))
}

fn expr_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var { name, .. } | Expr::Con { name, .. } => Some(name),
        _ => None,
    }
}

fn function_names(module: &Module) -> Vec<&str> {
    module
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Function(function) => Some(function.name.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn template_body_clauses_are_structured() {
    let src = r#"module Test where

template Holding
  with
    owner : Party
    amount : Decimal
  where
    signatory owner
    observer owner
    ensure amount > 0.0
    key owner : Party
    maintainer owner

    choice Transfer : ContractId Holding
      with
        newOwner : Party
      observer newOwner
      controller owner
      do
        create this with owner = newOwner
"#;
    let module = parse_ok(src);
    let holding = template(&module, "Holding");

    assert_eq!(holding.fields.len(), 2);
    assert_eq!(holding.fields[0].name, "owner");
    assert_eq!(text(src, holding.fields[0].span), "owner : Party");
    assert!(matches!(
        holding.fields[0].ty.as_ref(),
        Some(Type::Con { name, .. }) if name == "Party"
    ));
    assert_eq!(holding.fields[1].name, "amount");
    assert!(matches!(
        holding.fields[1].ty.as_ref(),
        Some(Type::Con { name, .. }) if name == "Decimal"
    ));

    assert!(holding.body.iter().any(|item| matches!(
        item,
        TemplateBodyDecl::Signatory { parties, span, .. }
            if parties.len() == 1
                && expr_name(&parties[0]) == Some("owner")
                && text(src, *span) == "signatory owner"
    )));
    assert!(holding.body.iter().any(|item| matches!(
        item,
        TemplateBodyDecl::Observer { parties, .. }
            if parties.len() == 1 && expr_name(&parties[0]) == Some("owner")
    )));
    assert!(holding.body.iter().any(|item| matches!(
        item,
        TemplateBodyDecl::Ensure {
            expr: Expr::BinOp { op, .. },
            ..
        } if op == ">"
    )));
    assert!(holding.body.iter().any(|item| matches!(
        item,
        TemplateBodyDecl::Key {
            expr,
            ty: Some(Type::Con { name, .. }),
            ..
        } if expr_name(expr) == Some("owner") && name == "Party"
    )));
    assert!(holding.body.iter().any(|item| matches!(
        item,
        TemplateBodyDecl::Maintainer { expr, .. } if expr_name(expr) == Some("owner")
    )));

    let template_choices = choices(holding);
    let transfer = choice(&template_choices, "Transfer");
    assert_eq!(transfer.consuming, Consuming::Consuming);
    assert_eq!(
        text(src, transfer.span),
        "choice Transfer : ContractId Holding\n      with\n        newOwner : Party\n      observer newOwner\n      controller owner\n      do\n        create this with owner = newOwner"
    );
    assert!(matches!(
        transfer.return_ty.as_ref(),
        Some(Type::App(head, args, _))
            if matches!(head.as_ref(), Type::Con { name, .. } if name == "ContractId")
                && args.len() == 1
    ));
    assert_eq!(transfer.params.len(), 1);
    assert_eq!(transfer.params[0].name, "newOwner");
    assert!(matches!(
        transfer.params[0].ty.as_ref(),
        Some(Type::Con { name, .. }) if name == "Party"
    ));
    assert_eq!(transfer.observers.len(), 1);
    assert_eq!(expr_name(&transfer.observers[0]), Some("newOwner"));
    assert_eq!(transfer.controllers.len(), 1);
    assert_eq!(expr_name(&transfer.controllers[0]), Some("owner"));
    assert!(matches!(transfer.body, Some(Expr::Do { .. })));
}

#[test]
fn choice_consumption_keywords_are_preserved() {
    let src = r#"module Test where

template Foo
  with
    owner : Party
  where
    signatory owner

    preconsuming choice Drain : ()
      controller owner
      do pure ()

    postconsuming choice Close : ()
      controller owner
      do pure ()

    nonconsuming choice Peek : ()
      controller owner
      do pure ()

    choice Normal : ()
      controller owner
      do pure ()
"#;
    let module = parse_ok(src);
    let template_choices = choices(template(&module, "Foo"));

    let drain = choice(&template_choices, "Drain");
    assert_eq!(drain.consuming, Consuming::PreConsuming);
    assert!(text(src, drain.span).starts_with("preconsuming choice Drain"));

    let close = choice(&template_choices, "Close");
    assert_eq!(close.consuming, Consuming::PostConsuming);
    assert!(text(src, close.span).starts_with("postconsuming choice Close"));

    let peek = choice(&template_choices, "Peek");
    assert_eq!(peek.consuming, Consuming::NonConsuming);
    assert!(text(src, peek.span).starts_with("nonconsuming choice Peek"));

    let normal = choice(&template_choices, "Normal");
    assert_eq!(normal.consuming, Consuming::Consuming);
    assert!(text(src, normal.span).starts_with("choice Normal"));
}

#[test]
fn empty_choice_with_block_does_not_swallow_controller() {
    let src = concat!(
        "module Test where\n",
        "template Foo\n",
        "  with\n",
        "    owner : Party\n",
        "  where\n",
        "    signatory owner\n",
        "    choice F : ()\n",
        "      with -- superfluous, no fields\n",
        "      controller owner\n",
        "      do pure ()\n",
    );
    let module = parse_ok(src);
    let template_choices = choices(template(&module, "Foo"));
    let f = choice(&template_choices, "F");

    assert!(f.params.is_empty());
    assert_eq!(f.controllers.len(), 1);
    assert_eq!(expr_name(&f.controllers[0]), Some("owner"));
    assert!(matches!(f.body, Some(Expr::Do { .. })));
}

#[test]
fn compact_template_and_choice_headers_survive_layout() {
    let src = concat!(
        "module Test where\n",
        "template SingleLine with p : Party where\n",
        "  signatory p\n",
        "\n",
        "template Compact\n",
        "  with\n",
        "    p : Party\n",
        "  where\n",
        "    signatory p\n",
        "    choice Ham : ContractId Compact with\n",
        "      controller p\n",
        "      do pure self\n",
        "    choice Spam : () with\n",
        "        extra : Party\n",
        "      controller p\n",
        "      do pure ()\n",
    );
    let module = parse_ok(src);

    let single_line = template(&module, "SingleLine");
    assert_eq!(single_line.fields.len(), 1);
    assert_eq!(single_line.fields[0].name, "p");
    assert!(matches!(
        single_line.body.as_slice(),
        [TemplateBodyDecl::Signatory { parties, .. }]
            if parties.len() == 1 && expr_name(&parties[0]) == Some("p")
    ));

    let compact_choices = choices(template(&module, "Compact"));
    assert_eq!(
        compact_choices
            .iter()
            .map(|choice| choice.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Ham", "Spam"]
    );
    assert!(choice(&compact_choices, "Ham").params.is_empty());
    assert_eq!(choice(&compact_choices, "Spam").params[0].name, "extra");
}

#[test]
fn template_layout_survives_tabs_comments_unicode_and_crlf() {
    let tabbed = parse_ok(
        "module M where\n\
         template Tabbed\n\
         \twith\n\
         \t\tx : Party\n\
         \twhere\n\
         \t\tsignatory x\n",
    );
    let tabbed_template = template(&tabbed, "Tabbed");
    assert_eq!(tabbed_template.fields.len(), 1);
    assert_eq!(tabbed_template.fields[0].name, "x");
    assert!(matches!(
        tabbed_template.body.as_slice(),
        [TemplateBodyDecl::Signatory { parties, .. }]
            if parties.len() == 1 && expr_name(&parties[0]) == Some("x")
    ));

    let commented = parse_ok(concat!(
        "module M where\n",
        "template Commented\n",
        "  -- fields below\n",
        "  with\n",
        "    -- the owner\n",
        "    owner : Party\n",
        "  where\n",
        "    signatory owner\n",
    ));
    assert_eq!(template(&commented, "Commented").fields[0].name, "owner");

    let unicode = parse_ok(
        "module M where\n\
         template Vertrag\n  with\n    eigentümer : Party\n    größe : Decimal\n  where\n    signatory eigentümer\n",
    );
    let vertrag = template(&unicode, "Vertrag");
    assert_eq!(
        vertrag
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        vec!["eigentümer", "größe"]
    );
    assert!(matches!(
        vertrag.body.as_slice(),
        [TemplateBodyDecl::Signatory { parties, .. }]
            if parties.len() == 1 && expr_name(&parties[0]) == Some("eigentümer")
    ));

    let crlf = parse_ok(
        "module M where\r\n\r\ntemplate Windows\r\n  with\r\n    x : Party\r\n  where\r\n    signatory x\r\n",
    );
    assert_eq!(template(&crlf, "Windows").fields[0].name, "x");
}

#[test]
fn cpp_directives_are_trivia_between_declarations() {
    let module = parse_ok(concat!(
        "module M where\n",
        "#ifdef DAML_BIGNUMERIC\n",
        "f = 1\n",
        "#endif\n",
        "g = 2\n",
    ));

    assert_eq!(function_names(&module), vec!["f", "g"]);
}

#[test]
fn interface_declarations_keep_methods_choices_and_requires() {
    let src = r#"module Test where

interface Account requires Lockable.I, Disclosure.I where
  viewtype AccountView
  getOwner : Party
  getBalance : Decimal

  nonconsuming choice GetView : AccountView
    with
      viewer : Party
    controller viewer
    do
      pure (view this)
"#;
    let module = parse_ok(src);
    let account = interface(&module, "Account");

    assert_eq!(account.requires, vec!["Lockable.I", "Disclosure.I"]);
    assert_eq!(account.viewtype.as_deref(), Some("AccountView"));
    assert_eq!(
        account
            .methods
            .iter()
            .map(|method| method.name.as_str())
            .collect::<Vec<_>>(),
        vec!["getOwner", "getBalance"]
    );
    assert_eq!(text(src, account.methods[0].span), "getOwner : Party");
    assert!(matches!(
        account.methods[0].ty.as_ref(),
        Some(Type::Con { name, .. }) if name == "Party"
    ));
    assert!(matches!(
        account.methods[1].ty.as_ref(),
        Some(Type::Con { name, .. }) if name == "Decimal"
    ));
    assert_eq!(account.choices.len(), 1);
    assert_eq!(account.choices[0].name, "GetView");
    assert_eq!(account.choices[0].consuming, Consuming::NonConsuming);
    assert!(matches!(
        account.choices[0].return_ty.as_ref(),
        Some(Type::Con { name, .. }) if name == "AccountView"
    ));
    assert_eq!(account.choices[0].params[0].name, "viewer");
    assert_eq!(
        expr_name(&account.choices[0].controllers[0]),
        Some("viewer")
    );
    assert!(matches!(account.choices[0].body, Some(Expr::Do { .. })));

    let function_names = function_names(&module);
    assert!(
        function_names.is_empty(),
        "interface method signatures must stay inside the interface: {function_names:?}"
    );
}

#[test]
fn template_body_contains_interface_instance_methods() {
    let src = r#"module Test where

template Reference
  with
    owner : Party
  where
    signatory owner

    interface instance Disclosure.I for Reference where
      setObservers observers = pure ()
      addObservers observers = pure ()
"#;
    let module = parse_ok(src);
    let reference = template(&module, "Reference");
    let instance = reference
        .body
        .iter()
        .find_map(|item| match item {
            TemplateBodyDecl::InterfaceInstance(instance) => Some(instance),
            _ => None,
        })
        .expect("interface instance");

    assert_eq!(instance.interface_name, "Disclosure.I");
    assert_eq!(instance.for_template, "Reference");
    assert_eq!(
        text(src, instance.span),
        "interface instance Disclosure.I for Reference where\n      setObservers observers = pure ()\n      addObservers observers = pure ()"
    );
    assert_eq!(instance.methods.len(), 2);
    assert!(matches!(
        &instance.methods[0].pat,
        Pat::Var { name, .. } if name == "setObservers"
    ));
    assert_eq!(instance.methods[0].params.len(), 1);
    assert!(matches!(
        &instance.methods[1].pat,
        Pat::Var { name, .. } if name == "addObservers"
    ));
}

#[test]
fn declaration_type_bearing_nodes_expose_structured_types_and_spans() {
    let src = r#"module Test where

template Typed
  with
    owner : Party
    asset : ContractId Asset
  where
    signatory owner
    key (owner, asset) : (Party, ContractId Asset)
    maintainer owner

    choice FetchAsset : Optional (ContractId Asset)
      with
        viewer : Party
      controller viewer
      do pure None

interface I where
  viewtype IView
  byKey : Party -> Optional (ContractId Asset)
"#;
    let module = parse_ok(src);
    let typed = template(&module, "Typed");

    assert_eq!(text(src, typed.fields[1].span), "asset : ContractId Asset");
    assert!(matches!(
        typed.fields[1].ty.as_ref(),
        Some(Type::App(head, args, _))
            if matches!(head.as_ref(), Type::Con { name, .. } if name == "ContractId")
                && matches!(args.as_slice(), [Type::Con { name, .. }] if name == "Asset")
    ));

    let key_ty = typed
        .body
        .iter()
        .find_map(|item| match item {
            TemplateBodyDecl::Key { ty, span, .. } => Some((ty.as_ref(), *span)),
            _ => None,
        })
        .expect("key clause");
    assert_eq!(
        text(src, key_ty.1),
        "key (owner, asset) : (Party, ContractId Asset)"
    );
    assert!(matches!(
        key_ty.0,
        Some(Type::Tuple(items, _))
            if items.len() == 2
                && matches!(&items[0], Type::Con { name, .. } if name == "Party")
                && matches!(&items[1], Type::App(head, _, _) if matches!(head.as_ref(), Type::Con { name, .. } if name == "ContractId"))
    ));

    let typed_choices = choices(typed);
    let fetch_asset = choice(&typed_choices, "FetchAsset");
    assert_eq!(text(src, fetch_asset.params[0].span), "viewer : Party");
    assert!(matches!(
        fetch_asset.return_ty.as_ref(),
        Some(Type::App(head, args, _))
            if matches!(head.as_ref(), Type::Con { name, .. } if name == "Optional")
                && args.len() == 1
    ));

    let iface = interface(&module, "I");
    assert_eq!(
        text(src, iface.methods[0].span),
        "byKey : Party -> Optional (ContractId Asset)"
    );
    assert!(matches!(
        iface.methods[0].ty.as_ref(),
        Some(Type::Fun(param, result, _))
            if matches!(param.as_ref(), Type::Con { name, .. } if name == "Party")
                && matches!(result.as_ref(), Type::App(head, _, _) if matches!(head.as_ref(), Type::Con { name, .. } if name == "Optional"))
    ));
}
