// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.

// rules/_helpers.ts
function statementExprs(statement) {
  if ("Let" in statement) return [statement.Let.value];
  if ("Assert" in statement) return [statement.Assert.condition_expr];
  if ("Fetch" in statement) return [statement.Fetch.cid];
  if ("Archive" in statement) return [statement.Archive.cid];
  if ("Create" in statement) return [statement.Create.argument];
  if ("Exercise" in statement) {
    return statement.Exercise.argument === null ? [statement.Exercise.cid] : [statement.Exercise.cid, statement.Exercise.argument];
  }
  if ("Other" in statement) return [statement.Other.expr];
  if ("Branch" in statement) return statement.Branch.scrutinee === null ? [] : [statement.Branch.scrutinee];
  return [];
}
function caseAltExprs(alt) {
  const branchBodies = alt.branches.length > 0 ? alt.branches.flatMap((branch) => [
    ...branch.guards.flatMap((guard) => "Bool" in guard ? [guard.Bool.expr] : [guard.Pattern.expr]),
    branch.body
  ]) : [alt.body];
  return [...branchBodies, ...alt.where_bindings.map((binding) => binding.value)];
}
function childExprs(expr) {
  if ("App" in expr) return [expr.App.func, ...expr.App.args];
  if ("BinOp" in expr) return [expr.BinOp.lhs, expr.BinOp.rhs];
  if ("Neg" in expr) return [expr.Neg.expr];
  if ("Lambda" in expr) return [expr.Lambda.body];
  if ("If" in expr) return [expr.If.cond, expr.If.then_branch, expr.If.else_branch];
  if ("Case" in expr) {
    return [
      expr.Case.scrutinee,
      ...expr.Case.alts.flatMap((alt) => caseAltExprs(alt))
    ];
  }
  if ("LetIn" in expr) return [...expr.LetIn.bindings.map((binding) => binding.value), expr.LetIn.body];
  if ("Record" in expr) return [expr.Record.base, ...expr.Record.fields.flatMap((field) => field.value === null ? [] : [field.value])];
  if ("Tuple" in expr) return expr.Tuple.items;
  if ("List" in expr) return expr.List.items;
  return [];
}
function walkBodyStatements(statements, visit) {
  for (const statement of statements) {
    visit(statement);
    if ("TryCatch" in statement) {
      walkBodyStatements(statement.TryCatch.try_body, visit);
      walkBodyStatements(statement.TryCatch.catch_body, visit);
    } else if ("Branch" in statement) {
      for (const arm of statement.Branch.arms) walkBodyStatements(arm.body, visit);
    }
    for (const expr of statementExprs(statement)) walkNestedDoStatements(expr, visit);
  }
}
function walkNestedDoStatements(expr, visit) {
  if ("DoBlock" in expr) walkBodyStatements(expr.DoBlock.statements, visit);
  for (const child of childExprs(expr)) walkNestedDoStatements(child, visit);
}

// examples/function-ledger-actions.ts
var NAME = "function-ledger-actions";
var SEVERITY = "info";
var DESCRIPTION = "Top-level functions performing archive/exercise \u2014 verify authorization is audited";
function ledgerActions(stmts) {
  const found = [];
  walkBodyStatements(stmts, (stmt) => {
    if ("Archive" in stmt) {
      found.push("archive");
    }
    if ("Exercise" in stmt) {
      found.push("exercise");
    }
  });
  return found;
}
function on_function(fn) {
  const actions = ledgerActions(fn.body);
  if (actions.length > 0) {
    report(fn, `Function '${fn.name}' performs ledger actions (${actions.join(", ")}) outside a choice`);
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_function };
