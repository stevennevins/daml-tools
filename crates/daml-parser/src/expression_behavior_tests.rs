//! Public AST behavior for expressions, do-statements, and parser-owned
//! recovery gaps previously covered indirectly by downstream crates.
//!
//! These tests enter through `parse_module` and assert public AST fields and
//! spans only; linter action classification remains downstream-owned.

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

fn first_function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
    module
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Function(function) if function.name == name => Some(function),
            _ => None,
        })
        .unwrap_or_else(|| panic!("function {name} not found"))
}

fn first_template<'a>(module: &'a Module, name: &str) -> &'a TemplateDecl {
    module
        .decls
        .iter()
        .find_map(|decl| match decl {
            Decl::Template(template) if template.name == name => Some(template),
            _ => None,
        })
        .unwrap_or_else(|| panic!("template {name} not found"))
}

fn choice<'a>(template: &'a TemplateDecl, name: &str) -> &'a ChoiceDecl {
    template
        .body
        .iter()
        .find_map(|item| match item {
            TemplateBodyDecl::Choice(choice) if choice.name == name => Some(choice),
            _ => None,
        })
        .unwrap_or_else(|| panic!("choice {name} not found"))
}

fn var_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var { name, .. } => Some(name.as_str()),
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
fn expression_comments_and_strings_do_not_create_ast_structure() {
    let module = parse_ok(
        "module M where\n\
         -- template Fake with choice Evil : () controller attacker\n\
         {- template Hidden\n\
              with\n\
                x : Party\n\
         -}\n\
         f = \"template Fake with x : Party where signatory attacker\"\n\
         g = \"exercise cid Evil\"\n",
    );

    assert!(
        !module
            .decls
            .iter()
            .any(|decl| matches!(decl, Decl::Template(_))),
        "comments or strings must not become template declarations"
    );
    assert!(matches!(
        &first_function(&module, "f").equations[0].body,
        Expr::Lit {
            kind: LitKind::Text,
            text,
            ..
        } if text.contains("template Fake")
    ));
    assert!(matches!(
        &first_function(&module, "g").equations[0].body,
        Expr::Lit {
            kind: LitKind::Text,
            text,
            ..
        } if text == "exercise cid Evil"
    ));
}

#[test]
fn string_with_escaped_quotes_and_comment_markers_stays_literal_and_next_decl_survives() {
    let module = parse_ok(
        r#"module M where
f = "a \" -- not a comment {- not a block"
g = 2
"#,
    );

    assert!(matches!(
        &first_function(&module, "f").equations[0].body,
        Expr::Lit {
            kind: LitKind::Text,
            text,
            ..
        } if text.contains("-- not a comment") && text.contains("{- not a block")
    ));
    assert!(matches!(
        &first_function(&module, "g").equations[0].body,
        Expr::Lit {
            kind: LitKind::Int,
            text,
            ..
        } if text == "2"
    ));
}

#[test]
fn operator_that_looks_like_comment_stays_operator_in_expression_ast() {
    let module = parse_ok("module M where\nf = a --> b\ng = c --- this is a comment\n");

    assert!(matches!(
        &first_function(&module, "f").equations[0].body,
        Expr::BinOp { op, lhs, rhs, .. }
            if op == "-->" && var_name(lhs) == Some("a") && var_name(rhs) == Some("b")
    ));
    assert!(matches!(
        &first_function(&module, "g").equations[0].body,
        Expr::Var { name, .. } if name == "c"
    ));
}

#[test]
fn view_patterns_keep_pattern_side_and_merge_equations() {
    let module = parse_ok(concat!(
        "module M where\n",
        "f (T.isInfixOf \"x\" -> True) = 1\n",
        "f (fromAny @T1 -> Some cid) = 2\n",
    ));
    let f = first_function(&module, "f");

    assert_eq!(f.equations.len(), 2);
    assert!(matches!(
        f.equations[0].params.as_slice(),
        [Pat::Con { name, args, .. }] if name == "True" && args.is_empty()
    ));
    assert!(matches!(
        f.equations[1].params.as_slice(),
        [Pat::Con { name, args, .. }]
            if name == "Some" && matches!(args.as_slice(), [Pat::Var { name, .. }] if name == "cid")
    ));
}

#[test]
fn annotated_parameter_function_type_is_not_view_pattern() {
    let module = parse_ok(concat!(
        "module M where\n",
        "applyFilter (filter : Int -> Int -> Bool) (xs : [Int]) : [Int] = xs\n",
    ));
    let apply_filter = first_function(&module, "applyFilter");

    assert_eq!(apply_filter.equations.len(), 1);
    assert_eq!(apply_filter.equations[0].params.len(), 2);
    assert!(matches!(
        &apply_filter.equations[0].params[0],
        Pat::Var { name, .. } if name == "filter"
    ));
}

#[test]
fn lambda_case_and_lazy_lambda_pattern_preserve_surrounding_decls() {
    let module = parse_ok(concat!(
        "module M where\n",
        "f = \\case\n",
        "    x :: _ -> x\n",
        "    [] -> 0\n",
        "g = foldr (\\(a, b) ~(as, bs) -> (a :: as, b :: bs)) ([], [])\n",
    ));

    assert_eq!(function_names(&module), vec!["f", "g"]);
    assert!(matches!(
        &first_function(&module, "f").equations[0].body,
        Expr::Lambda { body, .. }
            if matches!(body.as_ref(), Expr::Case { alts, .. } if alts.len() == 2)
    ));
    assert!(matches!(
        &first_function(&module, "g").equations[0].body,
        Expr::App { func, args, .. } if var_name(func) == Some("foldr") && args.len() == 2
    ));
}

#[test]
fn operator_equation_patterns_are_skipped_and_following_decl_survives() {
    let (module, diagnostics) = parse_module(concat!(
        "module M where\n",
        "[] !! _ = error \"index\"\n",
        "(x :: _) !! 0 = x\n",
        "None <?> s = invalid s\n",
        "Some v <?> _ = pure v\n",
        "after = 1\n",
    ));

    assert!(
        diagnostics.is_empty(),
        "operator-pattern syntax should be skipped cleanly: {diagnostics:?}"
    );
    assert_eq!(function_names(&module), vec!["after"]);
}

#[test]
fn guarded_case_with_pattern_guard_keeps_alternative_body() {
    let module = parse_ok(concat!(
        "module M where\n",
        "f x = case x of\n",
        "  Left cmd\n",
        "    | cmd.name == \"Submit\"\n",
        "    , Some y <- cmd.detail\n",
        "    -> y\n",
        "  _ -> 0\n",
    ));
    let body = &first_function(&module, "f").equations[0].body;

    assert!(matches!(
        body,
        Expr::Case { alts, .. }
            if alts.len() == 2 && matches!(&alts[0].body, Expr::Var { name, .. } if name == "y")
    ));
}

#[test]
fn where_block_skips_operator_bindings_and_preserves_later_decls() {
    let (module, diagnostics) = parse_module(concat!(
        "module M where\n",
        "f x = implode x\n",
        "  where\n",
        "    implode : [Text] -> Text = primitive @\"BEImplodeText\"\n",
        "    (==) : Text -> Text -> Bool = primitive @\"BEEqual\"\n",
        "    helper y = y\n",
        "g = 2\n",
    ));

    assert!(
        diagnostics.is_empty(),
        "where operator binding should be skipped cleanly: {diagnostics:?}"
    );
    assert_eq!(function_names(&module), vec!["f", "g"]);
    assert!(first_function(&module, "f").equations[0]
        .where_bindings
        .iter()
        .any(|binding| matches!(&binding.pat, Pat::Var { name, .. } if name == "helper")));
}

#[test]
fn do_bind_preserves_exercise_with_record_payload() {
    let module = parse_ok(concat!(
        "module M where\n",
        "template Foo\n",
        "  with\n",
        "    owner : Party\n",
        "  where\n",
        "    signatory owner\n",
        "    choice Go : ()\n",
        "      controller owner\n",
        "      do\n",
        "        result <- exercise optionCid Elect with electorParty = owner\n",
        "        pure result\n",
    ));
    let go = choice(first_template(&module, "Foo"), "Go");
    let Some(Expr::Do { stmts, .. }) = &go.body else {
        panic!("choice body must parse as do block: {:?}", go.body);
    };

    let DoStmt::Bind {
        pat, expr, span, ..
    } = &stmts[0]
    else {
        panic!("first do statement must be a bind: {:?}", stmts[0]);
    };
    assert_eq!(
        text(
            concat!(
                "module M where\n",
                "template Foo\n",
                "  with\n",
                "    owner : Party\n",
                "  where\n",
                "    signatory owner\n",
                "    choice Go : ()\n",
                "      controller owner\n",
                "      do\n",
                "        result <- exercise optionCid Elect with electorParty = owner\n",
                "        pure result\n",
            ),
            *span,
        ),
        "result <- exercise optionCid Elect with electorParty = owner"
    );
    assert!(matches!(pat, Pat::Var { name, .. } if name == "result"));

    let Expr::App { func, args, .. } = expr else {
        panic!("exercise statement must parse as an application: {expr:?}");
    };
    assert_eq!(var_name(func), Some("exercise"));
    assert_eq!(args.len(), 2);
    assert_eq!(var_name(&args[0]), Some("optionCid"));

    let Expr::Record { base, fields, .. } = &args[1] else {
        panic!(
            "choice argument must parse as record payload: {:?}",
            args[1]
        );
    };
    assert!(matches!(base.as_ref(), Expr::Con { name, .. } if name == "Elect"));
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "electorParty");
    assert!(matches!(
        fields[0].value.as_ref(),
        Some(Expr::Var { name, .. }) if name == "owner"
    ));
}

#[test]
fn do_statement_preserves_create_record_payload() {
    let module = parse_ok(concat!(
        "module M where\n",
        "template Foo\n",
        "  with\n",
        "    owner : Party\n",
        "  where\n",
        "    signatory owner\n",
        "    choice Transfer : ContractId Foo\n",
        "      with\n",
        "        newOwner : Party\n",
        "      controller owner\n",
        "      do\n",
        "        create this with owner = newOwner\n",
    ));
    let transfer = choice(first_template(&module, "Foo"), "Transfer");
    let Some(Expr::Do { stmts, .. }) = &transfer.body else {
        panic!("choice body must parse as do block: {:?}", transfer.body);
    };

    let DoStmt::Expr { expr, .. } = &stmts[0] else {
        panic!("create statement must be a bare expression: {:?}", stmts[0]);
    };
    let Expr::App { func, args, .. } = expr else {
        panic!("create statement must parse as an application: {expr:?}");
    };
    assert_eq!(var_name(func), Some("create"));
    assert_eq!(args.len(), 1);
    let Expr::Record { base, fields, .. } = &args[0] else {
        panic!(
            "create argument must parse as a record update: {:?}",
            args[0]
        );
    };
    assert!(matches!(base.as_ref(), Expr::Var { name, .. } if name == "this"));
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "owner");
    assert!(matches!(
        fields[0].value.as_ref(),
        Some(Expr::Var { name, .. }) if name == "newOwner"
    ));
}

#[test]
fn do_bind_preserves_exercise_without_payload_as_plain_application_args() {
    let module = parse_ok(concat!(
        "module M where\n",
        "template Foo\n",
        "  with\n",
        "    owner : Party\n",
        "  where\n",
        "    signatory owner\n",
        "    choice Go : ()\n",
        "      controller owner\n",
        "      do\n",
        "        result <- exercise optionCid Elect\n",
        "        pure result\n",
    ));
    let go = choice(first_template(&module, "Foo"), "Go");
    let Some(Expr::Do { stmts, .. }) = &go.body else {
        panic!("choice body must parse as do block: {:?}", go.body);
    };

    let DoStmt::Bind { expr, .. } = &stmts[0] else {
        panic!("first do statement must be a bind: {:?}", stmts[0]);
    };
    let Expr::App { func, args, .. } = expr else {
        panic!("exercise statement must parse as an application: {expr:?}");
    };
    assert_eq!(var_name(func), Some("exercise"));
    assert_eq!(args.len(), 2);
    assert_eq!(var_name(&args[0]), Some("optionCid"));
    assert!(matches!(&args[1], Expr::Con { name, .. } if name == "Elect"));
}

#[test]
fn record_construction_and_update_share_record_ast_shape() {
    let module = parse_ok(concat!(
        "module M where\n",
        "make owner = Foo with owner = owner\n",
        "change this newOwner = this with owner = newOwner\n",
    ));

    let make_body = &first_function(&module, "make").equations[0].body;
    let Expr::Record {
        base: make_base,
        fields: make_fields,
        ..
    } = make_body
    else {
        panic!("constructor with must parse as record expression: {make_body:?}");
    };
    assert!(matches!(make_base.as_ref(), Expr::Con { name, .. } if name == "Foo"));
    assert_eq!(make_fields[0].name, "owner");

    let change_body = &first_function(&module, "change").equations[0].body;
    let Expr::Record {
        base: change_base,
        fields: change_fields,
        ..
    } = change_body
    else {
        panic!("update with must parse as record expression: {change_body:?}");
    };
    assert!(matches!(change_base.as_ref(), Expr::Var { name, .. } if name == "this"));
    assert_eq!(change_fields[0].name, "owner");
}

#[test]
fn nested_control_flow_has_public_ast_nodes() {
    let module = parse_ok(concat!(
        "module M where\n",
        "f x = do\n",
        "  let y = if x then 1 else 2\n",
        "  z <- try (pure y) catch Some e -> pure 0\n",
        "  case z of\n",
        "    0 -> pure y\n",
        "    _ -> pure z\n",
    ));
    let body = &first_function(&module, "f").equations[0].body;
    let Expr::Do { stmts, .. } = body else {
        panic!("function body must parse as do block: {body:?}");
    };
    assert_eq!(stmts.len(), 3);

    let DoStmt::Let { bindings, .. } = &stmts[0] else {
        panic!("first statement must be a do-let: {:?}", stmts[0]);
    };
    assert!(matches!(bindings[0].expr, Expr::If { .. }));

    let DoStmt::Bind { expr, .. } = &stmts[1] else {
        panic!(
            "second statement must bind the try expression: {:?}",
            stmts[1]
        );
    };
    assert!(matches!(expr, Expr::Try { handlers, .. } if handlers.len() == 1));

    let DoStmt::Expr { expr, .. } = &stmts[2] else {
        panic!(
            "third statement must be the case expression: {:?}",
            stmts[2]
        );
    };
    assert!(matches!(expr, Expr::Case { alts, .. } if alts.len() == 2));
}

#[test]
fn do_block_preserves_let_bind_and_expression_statement_boundaries() {
    let module = parse_ok(concat!(
        "module M where\n",
        "f key = do\n",
        "  let go x = archive x\n",
        "  cid <- fetch key\n",
        "  go cid\n",
        "  pure ()\n",
    ));
    let body = &first_function(&module, "f").equations[0].body;
    let Expr::Do { stmts, .. } = body else {
        panic!("function body must parse as do block: {body:?}");
    };
    assert_eq!(stmts.len(), 4);

    let DoStmt::Let { bindings, .. } = &stmts[0] else {
        panic!("first statement must be a do-let: {:?}", stmts[0]);
    };
    assert!(matches!(&bindings[0].pat, Pat::Var { name, .. } if name == "go"));
    assert_eq!(bindings[0].params.len(), 1);
    assert!(
        matches!(&bindings[0].expr, Expr::App { func, .. } if var_name(func) == Some("archive"))
    );

    let DoStmt::Bind { pat, expr, .. } = &stmts[1] else {
        panic!("second statement must bind fetched cid: {:?}", stmts[1]);
    };
    assert!(matches!(pat, Pat::Var { name, .. } if name == "cid"));
    assert!(matches!(expr, Expr::App { func, .. } if var_name(func) == Some("fetch")));

    let DoStmt::Expr { expr, .. } = &stmts[2] else {
        panic!(
            "third statement must be a helper call expression: {:?}",
            stmts[2]
        );
    };
    assert!(matches!(expr, Expr::App { func, .. } if var_name(func) == Some("go")));

    let DoStmt::Expr { expr, .. } = &stmts[3] else {
        panic!("fourth statement must be a pure expression: {:?}", stmts[3]);
    };
    assert!(matches!(expr, Expr::App { func, .. } if var_name(func) == Some("pure")));
}

#[test]
fn dollar_application_keeps_record_payload_on_rhs() {
    let module = parse_ok("module M where\nsubmit p = create $ Foo with owner = p\n");
    let body = &first_function(&module, "submit").equations[0].body;
    let Expr::BinOp { op, lhs, rhs, .. } = body else {
        panic!("dollar application must parse as binary operator: {body:?}");
    };
    assert_eq!(op, "$");
    assert_eq!(var_name(lhs), Some("create"));

    let Expr::Record { base, fields, .. } = rhs.as_ref() else {
        panic!("dollar rhs must keep record payload: {rhs:?}");
    };
    assert!(matches!(base.as_ref(), Expr::Con { name, .. } if name == "Foo"));
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "owner");
    assert!(matches!(
        fields[0].value.as_ref(),
        Some(Expr::Var { name, .. }) if name == "p"
    ));
}

#[test]
fn try_catch_expression_has_body_and_handlers() {
    let module = parse_ok("module M where\nsubmit cid = try archive cid catch _ -> pure ()\n");
    let body = &first_function(&module, "submit").equations[0].body;
    let Expr::Try {
        body: try_body,
        handlers,
        ..
    } = body
    else {
        panic!("try/catch must parse as Expr::Try: {body:?}");
    };
    assert!(
        matches!(try_body.as_ref(), Expr::App { func, .. } if var_name(func) == Some("archive"))
    );
    assert_eq!(handlers.len(), 1);
    assert!(matches!(handlers[0].pat, Pat::Wild { .. }));
    assert!(matches!(
        &handlers[0].body,
        Expr::App { func, .. } if var_name(func) == Some("pure")
    ));
}

#[test]
fn lambda_case_is_a_lambda_wrapping_case_ast() {
    let module = parse_ok(concat!(
        "module M where\n",
        "f = \\case\n",
        "    Some x -> x\n",
        "    None -> 0\n",
    ));
    let body = &first_function(&module, "f").equations[0].body;
    let Expr::Lambda { params, body, .. } = body else {
        panic!("lambda-case must parse as lambda: {body:?}");
    };
    assert_eq!(params.len(), 1);
    assert!(matches!(params[0], Pat::Var { ref name, .. } if name == "_"));
    assert!(matches!(body.as_ref(), Expr::Case { alts, .. } if alts.len() == 2));
}
