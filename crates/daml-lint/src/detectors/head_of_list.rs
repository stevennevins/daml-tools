use crate::detector::{Detector, Finding, Severity};
use crate::ir::{DamlModule, Expr, Statement};
use std::collections::HashSet;

/// Detector #3: head-of-list-query
///
/// `query` / `queryFilter` / `queryContractId` / `queryInterface` return their
/// results in a non-deterministic order. Picking the first one — a `x :: _`
/// cons pattern, a single-element `[x]` pattern, `head`/`last`, or `!!` — is a
/// latent bug. The same applies to a destructuring bind straight from the query
/// (`[x] <- query`, `(x :: _) <- query`) and to a head/last mapped over the
/// result (`head <$> query`, `fmap head (query ...)`).
///
/// The detector first tracks, structurally, which `let`/binders hold a raw
/// query result through the statement stream, then flags head-of-list uses of
/// *those bindings only*. Tracking is last-binding-wins: re-binding a name to a
/// derived value (`let raw = sortOn snd raw`, `results <- pure (...)`) clears
/// it, so the deterministic head of a sorted list is not flagged, while a
/// re-query keeps it tracked. Pure aliases (`let alias = results`, including
/// chains) are propagated to a fixpoint. There is no proximity heuristic: a
/// list DERIVED from the query (sorted, filtered into a new binding), an
/// unrelated list, or a `mapA` over the whole result is never flagged, and each
/// unsafe use is reported once.
///
/// Every decision is structural — nothing reads the body text. `head` / `last` /
/// `!!` are matched on the `Expr` tree; the `case` head/single-element patterns
/// are read off the `Statement::Branch` a statement-position case lowers to,
/// which carries the scrutinee `Expr` and each arm's rendered pattern. Nesting
/// is structural too: a nested `case <other> of` is its own `Branch` with its
/// own scrutinee, so its alts never attach to the outer query scrutinee.
///
/// Catches: G5 (head-of-list on unordered query results)
pub struct HeadOfListQuery;

/// The query primitives whose results are unordered.
const QUERY_FUNCS: [&str; 4] = ["query", "queryFilter", "queryContractId", "queryInterface"];

struct HeadScanContext<'a> {
    file: &'a std::path::Path,
    context: &'a str,
}

impl Detector for HeadOfListQuery {
    fn name(&self) -> &str {
        "head-of-list-query"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn description(&self) -> &str {
        "Head-of-list pattern on query result (non-deterministic order)"
    }

    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        let mut findings = Vec::new();

        // (statements, context)
        let mut bodies: Vec<(&[Statement], String)> = Vec::new();
        for template in &module.templates {
            for choice in &template.choices {
                bodies.push((&choice.body, format!("choice '{}'", choice.name)));
            }
        }
        for func in &module.functions {
            bodies.push((&func.body, format!("function '{}'", func.name)));
        }

        for (statements, context) in bodies {
            let mut query_binders: HashSet<String> = HashSet::new();
            self.scan_statements(
                statements,
                &mut query_binders,
                &HeadScanContext {
                    file: &module.file,
                    context: &context,
                },
                &mut findings,
            );
        }

        findings
    }
}

impl HeadOfListQuery {
    /// Scan a statement scope in source order. Query-result tracking is updated
    /// only after the current statement has been inspected, so `head results`
    /// before a later safe rebind still sees `results` as a raw query result.
    fn scan_statements(
        &self,
        statements: &[Statement],
        query_binders: &mut HashSet<String>,
        scan_context: &HeadScanContext<'_>,
        findings: &mut Vec<Finding>,
    ) {
        for statement in statements {
            for expr in crate::ir::statement_exprs(statement) {
                self.scan_expr(expr, query_binders, scan_context, findings);
            }

            match statement {
                Statement::TryCatch {
                    try_body,
                    catch_body,
                    ..
                } => {
                    let mut try_binders = query_binders.clone();
                    self.scan_statements(try_body, &mut try_binders, scan_context, findings);
                    let mut catch_binders = query_binders.clone();
                    self.scan_statements(catch_body, &mut catch_binders, scan_context, findings);
                }
                Statement::Branch {
                    scrutinee: Some(scrutinee),
                    arms,
                    span,
                } => {
                    self.scan_branch_patterns(
                        scrutinee,
                        arms,
                        *span,
                        query_binders,
                        scan_context,
                        findings,
                    );
                    for arm in arms {
                        let mut arm_binders = query_binders.clone();
                        self.scan_statements(&arm.body, &mut arm_binders, scan_context, findings);
                    }
                }
                Statement::Branch { arms, .. } => {
                    for arm in arms {
                        let mut arm_binders = query_binders.clone();
                        self.scan_statements(&arm.body, &mut arm_binders, scan_context, findings);
                    }
                }
                _ => {}
            }

            self.update_query_binding(statement, query_binders, scan_context, findings);
        }
    }

    fn scan_expr(
        &self,
        expr: &Expr,
        query_binders: &HashSet<String>,
        scan_context: &HeadScanContext<'_>,
        findings: &mut Vec<Finding>,
    ) {
        let on_query = |e: &Expr| matches!(e.ref_string(), Some(r) if query_binders.contains(&r));

        match expr {
            // `head xs` / `DA.List.head xs` / `L.last xs` (any qualifier).
            Expr::App { func, args, span }
                if args.len() == 1 && head_or_last(func).is_some() && on_query(&args[0]) =>
            {
                let f = head_or_last(func).unwrap();
                findings.push(self.finding(
                    scan_context.file,
                    span.line,
                    format!(
                        "`{}` on query result in {}. Query results have \
                         non-deterministic order.",
                        f, scan_context.context
                    ),
                    format!("{} {}", f, args[0].render_text()),
                ));
            }
            // `head $ xs` — `$` is application; the denominator-free idiom.
            Expr::BinOp { op, lhs, rhs, span } if op == "$" && on_query(rhs) => {
                if let Some(f) = head_or_last(lhs) {
                    findings.push(self.finding(
                        scan_context.file,
                        span.line,
                        format!(
                            "`{} $` on query result in {}. Query results have \
                             non-deterministic order.",
                            f, scan_context.context
                        ),
                        format!("{} $ {}", f, rhs.render_text()),
                    ));
                }
            }
            Expr::BinOp { op, lhs, span, .. } if op == "!!" && on_query(lhs) => {
                findings.push(self.finding(
                    scan_context.file,
                    span.line,
                    format!(
                        "Index `!!` into query result in {}. Query results have \
                         non-deterministic order.",
                        scan_context.context
                    ),
                    format!("{} !!", lhs.render_text()),
                ));
            }
            Expr::DoBlock { statements, .. } => {
                let mut nested_binders = query_binders.clone();
                self.scan_statements(statements, &mut nested_binders, scan_context, findings);
            }
            _ => {}
        }

        for child_expr in crate::ir::child_exprs(expr) {
            self.scan_expr(child_expr, query_binders, scan_context, findings);
        }
    }

    /// Apply last-binding-wins query-result tracking for the statement that was
    /// just scanned.
    fn update_query_binding(
        &self,
        statement: &Statement,
        query_binders: &mut HashSet<String>,
        scan_context: &HeadScanContext<'_>,
        findings: &mut Vec<Finding>,
    ) {
        let Some((name, value)) = binder_and_value(statement) else {
            return;
        };

        if !is_plain_identifier(name) {
            // A destructuring bind STRAIGHT from a query is the canonical
            // "expect exactly one" bug: `[x] <- query` crashes on 0/2+,
            // `(x :: _) <- query` picks a non-deterministic head.
            if is_query_app(value) {
                self.flag_destructure_bind(
                    name,
                    stmt_line(statement),
                    scan_context.file,
                    scan_context.context,
                    findings,
                );
            }
            return;
        }

        if is_query_app(value) {
            query_binders.insert(name.to_string());
        } else if let Some(src) = value.ref_string() {
            if query_binders.contains(&src) {
                query_binders.insert(name.to_string());
            } else {
                query_binders.remove(name);
            }
        } else {
            // `head <$> query` / `fmap head (query ...)`: the picked element
            // binds directly, no intermediate list. The binder holds a single
            // non-deterministic element, so flag it and do not track the binder.
            if let Some(f) = fmap_head_of_query(value) {
                findings.push(self.finding(
                    scan_context.file,
                    stmt_line(statement),
                    format!(
                        "`{}` over query result in {}. Query results \
                         have non-deterministic order.",
                        f, scan_context.context
                    ),
                    format!("{} <$> query", f),
                ));
            }
            query_binders.remove(name);
        }
    }

    fn finding(
        &self,
        file: &std::path::Path,
        line: usize,
        message: String,
        evidence: String,
    ) -> Finding {
        Finding {
            detector: self.name().to_string(),
            severity: self.severity(),
            file: file.to_path_buf(),
            line,
            column: 1,
            message,
            evidence,
        }
    }

    /// Flag each head / single-element `case` alternative whose case scrutinizes
    /// a tracked query binding.
    fn scan_branch_patterns(
        &self,
        scrutinee: &Expr,
        arms: &[crate::ir::BranchArm],
        span: crate::ir::SrcPos,
        query_binders: &HashSet<String>,
        scan_context: &HeadScanContext<'_>,
        findings: &mut Vec<Finding>,
    ) {
        if !matches!(scrutinee.ref_string(), Some(r) if query_binders.contains(&r)) {
            return;
        }
        for arm in arms {
            let Some(pattern) = arm.pattern.as_deref() else {
                continue;
            };
            // The parser renders a cons pattern in prefix form (`(:: x _)`),
            // so the binder matcher — not the infix one — is the right test.
            if is_cons_head_binder(pattern) {
                findings.push(self.finding(
                    scan_context.file,
                    span.line,
                    format!(
                        "Head-of-list pattern '{}' on query result in {}. \
                         Query results have non-deterministic order.",
                        pattern, scan_context.context
                    ),
                    pattern.to_string(),
                ));
            } else if is_singleton_list_pattern(pattern) {
                findings.push(self.finding(
                    scan_context.file,
                    span.line,
                    format!(
                        "Single-element list pattern '{}' on query result in {}. \
                         Crashes on 0 or 2+ results.",
                        pattern, scan_context.context
                    ),
                    pattern.to_string(),
                ));
            }
        }
    }

    /// Flag a destructuring monadic bind `[x] <- query` / `(x :: _) <- query`
    /// (the binder PATTERN is the head-of-list). The binder is rendered source
    /// text: a single-element list `[x]`, or the parser's prefix form of a cons
    /// (`(:: x _)`).
    fn flag_destructure_bind(
        &self,
        binder: &str,
        line: usize,
        file: &std::path::Path,
        context: &str,
        findings: &mut Vec<Finding>,
    ) {
        if is_cons_head_binder(binder) {
            findings.push(self.finding(
                file,
                line,
                format!(
                    "Head-of-list bind '{} <- query' in {}. \
                     Query results have non-deterministic order.",
                    binder, context
                ),
                binder.to_string(),
            ));
        } else if is_singleton_list_pattern(binder) {
            findings.push(self.finding(
                file,
                line,
                format!(
                    "Single-element list bind '{} <- query' in {}. \
                     Crashes on 0 or 2+ results.",
                    binder, context
                ),
                binder.to_string(),
            ));
        }
    }
}

/// The (binder, bound-value) of a statement, if it binds a name.
fn binder_and_value(s: &Statement) -> Option<(&str, &Expr)> {
    match s {
        Statement::Other {
            binder: Some(name),
            expr,
            ..
        } => Some((name, expr)),
        Statement::Let { name, value, .. } => Some((name, value)),
        _ => None,
    }
}

/// The 1-based source line of a binding statement.
const fn stmt_line(s: &Statement) -> usize {
    match s {
        Statement::Other { span, .. } | Statement::Let { span, .. } => span.line,
        _ => 0,
    }
}

/// True if `name` is a single bare identifier (a list-holder name), not a
/// destructuring pattern like `[x]` or the prefix-rendered cons `(:: x _)`.
fn is_plain_identifier(name: &str) -> bool {
    let t = name.trim();
    !t.is_empty()
        && t.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'\'')
}

/// True if `binder` is the parser's prefix rendering of a cons that grabs the
/// head and discards the tail: `(:: x _)`. A tail-binding cons (`(:: x rest)`)
/// is proper iteration and is NOT matched.
fn is_cons_head_binder(binder: &str) -> bool {
    let trimmed = binder.trim();
    let inner = trimmed
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .map_or_else(|| trimmed, str::trim);
    let rest = match inner.strip_prefix("::") {
        Some(r) => r.trim(),
        None => return false,
    };
    // `:: <head> <tail>` — the tail (last whitespace-separated atom) is `_`.
    matches!(rest.rsplit_once(char::is_whitespace), Some((_, tail)) if tail.trim() == "_")
}

/// True if the application spine's head is an unqualified query primitive.
fn is_query_app(e: &Expr) -> bool {
    matches!(e.application_head(), Expr::Var { name, qualifier: None, .. } if QUERY_FUNCS.contains(&name.as_str()))
}

/// `Some("head")` / `Some("last")` if `func` is that list head/tail selector,
/// at any qualifier (`head`, `DA.List.head`, `L.last`).
fn head_or_last(func: &Expr) -> Option<&str> {
    match func {
        Expr::Var { name, .. } if name == "head" || name == "last" => Some(name),
        _ => None,
    }
}

/// `Some("head")` / `Some("last")` if `e` maps a head/tail selector over a query
/// result without an intermediate binding: `head <$> query ...` (a `<$>` BinOp)
/// or `fmap head (query ...)` (an `fmap`/`<$>`-equivalent App). The fmapped
/// element is a single non-deterministic pick, the same bug as `head results`.
fn fmap_head_of_query(e: &Expr) -> Option<&str> {
    match e {
        // `head <$> query ...`
        Expr::BinOp { op, lhs, rhs, .. } if op == "<$>" => {
            head_or_last(lhs).filter(|_| is_query_app(rhs))
        }
        // `fmap head (query ...)`
        Expr::App { func, args, .. }
            if args.len() == 2
                && matches!(func.as_ref(), Expr::Var { name, .. } if name == "fmap") =>
        {
            head_or_last(&args[0]).filter(|_| is_query_app(&args[1]))
        }
        _ => None,
    }
}

/// A single-element list pattern `[x]` / `[(a, b)]` — crashes unless the query
/// returns exactly one result. Not `[]` (empty) and not `[a, b]` (fixed many).
fn is_singleton_list_pattern(pattern: &str) -> bool {
    let inner = match pattern
        .trim()
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
    {
        Some(i) => i.trim(),
        None => return false,
    };
    !inner.is_empty() && !has_top_level_comma(inner)
}

/// A top-level (not inside `()` / `[]`) comma — a list of several elements.
fn has_top_level_comma(s: &str) -> bool {
    let mut depth = 0i32;
    for b in s.bytes() {
        match b {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_daml;
    use std::path::Path;

    #[test]
    fn test_head_of_list_cons_pattern() {
        let source = r#"module Test where

getFeaturedAppRight owner = do
  results <- queryFilter @FeaturedAppRight (\r -> r.provider == owner)
  case results of
    (rightCid, _) :: _ -> do
      pure (Some rightCid)
    [] -> pure None
"#;
        let module = parse_daml(source, Path::new("AmuletRegistry.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("::"));
    }

    #[test]
    fn test_head_of_list_singleton_pattern() {
        let source = r#"module Test where

getTransferFactory owner = do
  results <- query @TransferFactory owner
  case results of
    [(rulesCid, _)] -> pure rulesCid
"#;
        let module = parse_daml(source, Path::new("SimpleRegistry.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_safe_query_usage_passes() {
        let source = r#"module Test where

getAllFactories owner = do
  results <- query @TransferFactory owner
  mapA (\(cid, _) -> fetch cid) results
"#;
        let module = parse_daml(source, Path::new("Safe.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(findings.is_empty());
    }

    // Regression (round-3 F18): one unsafe use is reported exactly once, not
    // twice by two overlapping scans.
    #[test]
    fn test_cons_pattern_reported_once() {
        let source = r#"module Test where

getOne owner = do
  results <- query @Foo owner
  case results of
    x :: _ -> pure (Some x)
    [] -> pure None
"#;
        let module = parse_daml(source, Path::new("Once.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert_eq!(findings.len(), 1, "exactly one finding: {:?}", findings);
    }

    // Regression (round-3 F19/F20): a `head` on a NON-query list is not flagged.
    #[test]
    fn test_head_on_non_query_list_is_ignored() {
        let source = r#"module Test where

firstOf xs = pure (head xs)
"#;
        let module = parse_daml(source, Path::new("Plain.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "head on an arbitrary list is not a query bug: {:?}",
            findings
        );
    }

    // Regression (round-3 F21): `head` / `!!` applied to a query binding flag.
    #[test]
    fn test_head_and_index_on_query_binding_flag() {
        let head_src = r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (head results)
"#;
        let m = parse_daml(head_src, Path::new("Head.daml"));
        assert!(
            HeadOfListQuery
                .detect(&m)
                .iter()
                .any(|f| f.message.contains("head")),
            "head on a query result must flag"
        );

        let idx_src = r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (results !! 0)
"#;
        let m = parse_daml(idx_src, Path::new("Index.daml"));
        assert!(
            !HeadOfListQuery.detect(&m).is_empty(),
            "`results !! 0` on a query result must flag"
        );
    }

    // Regression (audit round-3): `head $ results` (the `$` application idiom)
    // on a query result is flagged, but `head $ sortOn f results` (derived) is
    // not.
    #[test]
    fn test_head_dollar_on_query_binding() {
        let flagged = r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (head $ results)
"#;
        let m = parse_daml(flagged, Path::new("Dollar.daml"));
        assert!(
            !HeadOfListQuery.detect(&m).is_empty(),
            "head $ results on a query result must flag"
        );

        let safe = r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (head $ sortOn snd results)
"#;
        let m = parse_daml(safe, Path::new("DollarSorted.daml"));
        assert!(
            HeadOfListQuery.detect(&m).is_empty(),
            "head $ sortOn ... results is deterministic: {:?}",
            HeadOfListQuery.detect(&m)
        );
    }

    // Regression (audit round-3): a qualified `DA.List.head results` is flagged.
    #[test]
    fn test_qualified_head_on_query_binding() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (DA.List.head results)
"#;
        let m = parse_daml(source, Path::new("Qual.daml"));
        assert!(
            !HeadOfListQuery.detect(&m).is_empty(),
            "DA.List.head on a query result must flag"
        );
    }

    // Regression (round-3 F32): a list DERIVED from the query (e.g. sorted) is a
    // new binding, not the raw query result, so taking its head is deterministic
    // and must NOT be flagged.
    #[test]
    fn test_head_of_sorted_query_result_is_not_flagged() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  let sorted = sortOn snd results
  case sorted of
    x :: _ -> pure (Some x)
    [] -> pure None
"#;
        let module = parse_daml(source, Path::new("Sorted.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "head of a sorted (deterministic) list is safe: {:?}",
            findings
        );
    }

    // A recursive cons that binds the tail is legitimate iteration, not a
    // head-of-list bug — only `:: _` (tail discarded) is flagged.
    #[test]
    fn test_recursive_cons_binding_tail_is_not_flagged() {
        let source = r#"module Test where

go owner = do
  results <- query @Foo owner
  case results of
    x :: rest -> process x rest
    [] -> pure ()
"#;
        let module = parse_daml(source, Path::new("Rec.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "`x :: rest` binds the tail (proper recursion), not head-of-list: {:?}",
            findings
        );
    }

    // Regression (audit F8): a monadic destructuring bind directly from a query
    // is the canonical "expect exactly one" bug. `[x] <- query` crashes on 0 or
    // 2+ results; `(x :: _) <- query` silently picks a non-deterministic head.
    #[test]
    fn test_singleton_bind_from_query_is_flagged() {
        let source = r#"module Test where

pick owner = do
  [theOne] <- query @Foo owner
  pure theOne
"#;
        let module = parse_daml(source, Path::new("BindOne.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert_eq!(
            findings.len(),
            1,
            "`[theOne] <- query` is a single-element destructure: {:?}",
            findings
        );
        assert!(findings[0].message.contains("Single-element"));
    }

    #[test]
    fn test_cons_head_bind_from_query_is_flagged() {
        let source = r#"module Test where

pick owner = do
  (x :: _) <- query @Foo owner
  pure x
"#;
        let module = parse_daml(source, Path::new("BindCons.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert_eq!(
            findings.len(),
            1,
            "`(x :: _) <- query` discards the tail (head-of-list): {:?}",
            findings
        );
    }

    // The bind forms that are NOT head-of-list stay unflagged: a fixed-many
    // `[a, b] <-`, a tail-binding `(x :: rest) <-`, and a plain `results <-`.
    #[test]
    fn test_safe_binds_from_query_are_not_flagged() {
        for src in [
            "module Test where\n\nf owner = do\n  [a, b] <- query @Foo owner\n  pure (a, b)\n",
            "module Test where\n\nf owner = do\n  (x :: rest) <- query @Foo owner\n  process x rest\n",
            "module Test where\n\nf owner = do\n  results <- query @Foo owner\n  mapA fetch results\n",
        ] {
            let module = parse_daml(src, Path::new("SafeBind.daml"));
            let findings = HeadOfListQuery.detect(&module);
            assert!(
                findings.is_empty(),
                "safe bind must not flag for {src:?}: {:?}",
                findings
            );
        }
    }

    // Regression (audit F7): a nested `case` on an UNRELATED local list inside a
    // query-result case branch must not be attributed to the outer query
    // scrutinee. The outer `case results of` has only safe alts (`_`); the
    // `first :: _` head belongs to the inner `case names of`.
    #[test]
    fn test_nested_case_on_local_list_is_not_flagged() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  case results of
    _ -> do
      let names = ["a", "b"]
      case names of
        first :: _ -> pure (Some first)
        [] -> pure None
"#;
        let module = parse_daml(source, Path::new("Nested.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "head of a fixed local list inside a query-case branch is safe: {:?}",
            findings
        );
    }

    // The dual of F7: a nested case on a genuine query result is still flagged
    // exactly once (and attributed to the inner, query-bound scrutinee).
    #[test]
    fn test_nested_case_on_query_result_flags_once() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  case owner of
    _ -> do
      case results of
        first :: _ -> pure (Some first)
        [] -> pure None
"#;
        let module = parse_daml(source, Path::new("NestedQ.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert_eq!(
            findings.len(),
            1,
            "nested case on the query result is one finding: {:?}",
            findings
        );
    }

    // Regression (audit F22): a query-bound name re-bound to a derived (sorted)
    // list before `head` is taken is deterministic — the rebind clears the
    // query-result tracking, so the shadowed `head` must NOT be flagged.
    #[test]
    fn test_rebind_to_sorted_list_is_not_flagged() {
        // monadic re-bind: `results <- pure (sortOn ...)`
        let monadic = r#"module Test where

pick owner = do
  results <- query @Foo owner
  results <- pure (sortOn snd results)
  pure (head results)
"#;
        let module = parse_daml(monadic, Path::new("Rebind.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "`head` of a re-bound sorted list is deterministic: {:?}",
            findings
        );

        // `let` re-bind shadowing the same name.
        let let_rebind = r#"module Test where

pick owner = do
  raw <- query @Foo owner
  let raw = sortOn snd raw
  pure (head raw)
"#;
        let module = parse_daml(let_rebind, Path::new("RebindLet.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "`let raw = sortOn snd raw` clears the query tracking: {:?}",
            findings
        );
    }

    // Regression (release audit): a later safe rebind must not erase an earlier
    // unsafe head-of-list use. The detector has to scan with the binding
    // environment current at each statement, not with the final environment.
    #[test]
    fn test_head_before_sorted_rebind_is_still_flagged() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  first <- pure (head results)
  results <- pure (sortOn snd results)
  pure first
"#;
        let module = parse_daml(source, Path::new("HeadBeforeRebind.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("head")),
            "`head results` before a safe rebind is still unsafe: {:?}",
            findings
        );
    }

    // A re-bind to ANOTHER query (re-query) keeps flagging — the name is still a
    // raw query result.
    #[test]
    fn test_requery_rebind_still_flags() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  results <- query @Bar owner
  pure (head results)
"#;
        let module = parse_daml(source, Path::new("Requery.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            !findings.is_empty(),
            "a re-query is still a raw query result: {:?}",
            findings
        );
    }

    // Regression (audit F23): a pure alias of the raw query result
    // (`let alias = results`) carries the head-of-list bug — `head alias` is the
    // same non-deterministic head. Chains (`let b = a`) are followed too.
    #[test]
    fn test_alias_of_query_result_is_flagged() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  let alias = results
  pure (head alias)
"#;
        let module = parse_daml(source, Path::new("Alias.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            !findings.is_empty(),
            "`head` of a direct alias of the query result must flag: {:?}",
            findings
        );

        let chained = r#"module Test where

pick owner = do
  results <- query @Foo owner
  let a = results
  let b = a
  pure (head b)
"#;
        let module = parse_daml(chained, Path::new("AliasChain.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            !findings.is_empty(),
            "an alias chain to the query result must flag: {:?}",
            findings
        );
    }

    // A DERIVED binding (sorted/filtered) is NOT a pure alias and stays
    // unflagged — alias propagation must only follow bare references.
    #[test]
    fn test_derived_binding_is_not_an_alias() {
        let source = r#"module Test where

pick owner = do
  results <- query @Foo owner
  let sorted = sortOn snd results
  pure (head sorted)
"#;
        let module = parse_daml(source, Path::new("Derived.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "a sorted derivation is not a pure alias: {:?}",
            findings
        );
    }

    // Regression (audit F24): the type-correct fmap forms `head <$> query` and
    // `fmap head (query ...)` pick a single non-deterministic element, the same
    // bug as `head results`. (The literal `head (query ...)` repro is a type
    // error — query returns an Update, not a list — and is intentionally not
    // chased.)
    #[test]
    fn test_fmap_head_over_query_is_flagged() {
        for src in [
            "module Test where\n\npick owner = do\n  x <- head <$> query @Foo owner\n  pure x\n",
            "module Test where\n\npick owner = do\n  x <- fmap head (query @Foo owner)\n  pure x\n",
            "module Test where\n\npick owner = do\n  x <- last <$> query @Foo owner\n  pure x\n",
        ] {
            let module = parse_daml(src, Path::new("Fmap.daml"));
            let findings = HeadOfListQuery.detect(&module);
            assert_eq!(
                findings.len(),
                1,
                "fmap head over a query must flag for {src:?}: {:?}",
                findings
            );
        }
    }

    // An order-preserving fmap that is NOT head/last (`map f <$> query`,
    // `sortOn snd <$> query`) keeps the whole list and is not a head-of-list bug.
    #[test]
    fn test_fmap_non_head_over_query_is_not_flagged() {
        let source = r#"module Test where

pick owner = do
  xs <- sortOn snd <$> query @Foo owner
  pure xs
"#;
        let module = parse_daml(source, Path::new("FmapSafe.daml"));
        let findings = HeadOfListQuery.detect(&module);
        assert!(
            findings.is_empty(),
            "a non-head fmap over a query keeps the whole list: {:?}",
            findings
        );
    }
}
