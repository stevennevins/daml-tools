// rules/_helpers.ts
function refString(expr) {
  if ("Var" in expr) {
    return expr.Var.qualifier === null ? expr.Var.name : `${expr.Var.qualifier}.${expr.Var.name}`;
  }
  if ("Con" in expr) {
    return expr.Con.qualifier === null ? expr.Con.name : `${expr.Con.qualifier}.${expr.Con.name}`;
  }
  if ("BinOp" in expr && expr.BinOp.op === ".") {
    const lhs = refString(expr.BinOp.lhs);
    const rhs = refString(expr.BinOp.rhs);
    return lhs !== null && rhs !== null ? `${lhs}.${rhs}` : null;
  }
  return null;
}
function renderText(expr) {
  if ("Var" in expr || "Con" in expr) return refString(expr) ?? "";
  if ("Lit" in expr) return expr.Lit.value;
  if ("Neg" in expr) return `-${renderText(expr.Neg.expr)}`;
  if ("BinOp" in expr && expr.BinOp.op === ".") {
    return `${renderText(expr.BinOp.lhs)}.${renderText(expr.BinOp.rhs)}`;
  }
  if ("BinOp" in expr) return `${renderText(expr.BinOp.lhs)} ${expr.BinOp.op} ${renderText(expr.BinOp.rhs)}`;
  if ("App" in expr) return [renderText(expr.App.func), ...expr.App.args.map(renderText)].join(" ");
  if ("Tuple" in expr) return `(${expr.Tuple.items.map(renderText).join(", ")})`;
  if ("List" in expr) return `[${expr.List.items.map(renderText).join(", ")}]`;
  if ("Unknown" in expr) return expr.Unknown.raw;
  return "...";
}
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
function childExprs(expr) {
  if ("App" in expr) return [expr.App.func, ...expr.App.args];
  if ("BinOp" in expr) return [expr.BinOp.lhs, expr.BinOp.rhs];
  if ("Neg" in expr) return [expr.Neg.expr];
  if ("Lambda" in expr) return [expr.Lambda.body];
  if ("If" in expr) return [expr.If.cond, expr.If.then_branch, expr.If.else_branch];
  if ("Case" in expr) return [expr.Case.scrutinee, ...expr.Case.alts.map((alt) => alt.body)];
  if ("LetIn" in expr) return [...expr.LetIn.bindings.map((binding) => binding.value), expr.LetIn.body];
  if ("Record" in expr) return [expr.Record.base, ...expr.Record.fields.flatMap((field) => field.value === null ? [] : [field.value])];
  if ("Tuple" in expr) return expr.Tuple.items;
  if ("List" in expr) return expr.List.items;
  return [];
}

// rules/head-of-list-query.ts
var NAME = "head-of-list-query";
var SEVERITY = "medium";
var DESCRIPTION = "Head-of-list pattern on query result (non-deterministic order)";
var QUERY_FUNCS = ["query", "queryFilter", "queryContractId", "queryInterface"];
function check(module) {
  for (const template of module.templates) {
    for (const choice of template.choices) {
      scanStatements(choice.body, /* @__PURE__ */ new Set(), { context: `choice '${choice.name}'` });
    }
  }
  for (const func of module.functions) {
    scanStatements(func.body, /* @__PURE__ */ new Set(), { context: `function '${func.name}'` });
  }
}
function scanStatements(statements, queryBinders, scanContext) {
  for (const statement of statements) {
    for (const expr of statementExprs(statement)) scanExpr(expr, queryBinders, scanContext);
    if ("TryCatch" in statement) {
      scanStatements(statement.TryCatch.try_body, new Set(queryBinders), scanContext);
      scanStatements(statement.TryCatch.catch_body, new Set(queryBinders), scanContext);
    } else if ("Branch" in statement) {
      if (statement.Branch.scrutinee !== null) {
        scanBranchPatterns(statement.Branch.scrutinee, statement.Branch.arms, statement.Branch.span, queryBinders, scanContext);
      }
      for (const arm of statement.Branch.arms) scanStatements(arm.body, new Set(queryBinders), scanContext);
    }
    updateQueryBinding(statement, queryBinders, scanContext);
  }
}
function scanExpr(expr, queryBinders, scanContext) {
  const onQuery = (candidate) => {
    const ref = refString(candidate);
    return ref !== null && queryBinders.has(ref);
  };
  if ("App" in expr && expr.App.args.length === 1 && headOrLast(expr.App.func) !== null && onQuery(expr.App.args[0])) {
    const selector = headOrLast(expr.App.func);
    report(
      { span: { line: expr.App.span.line, column: 1 } },
      `\`${selector}\` on query result in ${scanContext.context}. Query results have non-deterministic order.`,
      `${selector} ${renderText(expr.App.args[0])}`
    );
  } else if ("BinOp" in expr && expr.BinOp.op === "$" && onQuery(expr.BinOp.rhs)) {
    const selector = headOrLast(expr.BinOp.lhs);
    if (selector !== null) {
      report(
        { span: { line: expr.BinOp.span.line, column: 1 } },
        `\`${selector} $\` on query result in ${scanContext.context}. Query results have non-deterministic order.`,
        `${selector} $ ${renderText(expr.BinOp.rhs)}`
      );
    }
  } else if ("BinOp" in expr && expr.BinOp.op === "!!" && onQuery(expr.BinOp.lhs)) {
    report(
      { span: { line: expr.BinOp.span.line, column: 1 } },
      `Index \`!!\` into query result in ${scanContext.context}. Query results have non-deterministic order.`,
      `${renderText(expr.BinOp.lhs)} !!`
    );
  } else if ("DoBlock" in expr) {
    scanStatements(expr.DoBlock.statements, new Set(queryBinders), scanContext);
  }
  for (const child of childExprs(expr)) scanExpr(child, queryBinders, scanContext);
}
function updateQueryBinding(statement, queryBinders, scanContext) {
  const binding = binderAndValue(statement);
  if (binding === null) return;
  const { name, value, line } = binding;
  if (!isPlainIdentifier(name)) {
    if (isQueryApp(value)) flagDestructureBind(name, line, scanContext);
    return;
  }
  if (isQueryApp(value)) {
    queryBinders.add(name);
  } else {
    const source = refString(value);
    if (source !== null) {
      if (queryBinders.has(source)) queryBinders.add(name);
      else queryBinders.delete(name);
    } else {
      const selector = fmapHeadOfQuery(value);
      if (selector !== null) {
        report(
          { span: { line, column: 1 } },
          `\`${selector}\` over query result in ${scanContext.context}. Query results have non-deterministic order.`,
          `${selector} <$> query`
        );
      }
      queryBinders.delete(name);
    }
  }
}
function scanBranchPatterns(scrutinee, arms, span, queryBinders, scanContext) {
  const ref = refString(scrutinee);
  if (ref === null || !queryBinders.has(ref)) return;
  for (const arm of arms) {
    if (arm.pattern === null) continue;
    if (isConsHeadBinder(arm.pattern)) {
      report(
        { span: { line: span.line, column: 1 } },
        `Head-of-list pattern '${arm.pattern}' on query result in ${scanContext.context}. Query results have non-deterministic order.`,
        arm.pattern
      );
    } else if (isSingletonListPattern(arm.pattern)) {
      report(
        { span: { line: span.line, column: 1 } },
        `Single-element list pattern '${arm.pattern}' on query result in ${scanContext.context}. Crashes on 0 or 2+ results.`,
        arm.pattern
      );
    }
  }
}
function flagDestructureBind(binder, line, scanContext) {
  if (isConsHeadBinder(binder)) {
    report(
      { span: { line, column: 1 } },
      `Head-of-list bind '${binder} <- query' in ${scanContext.context}. Query results have non-deterministic order.`,
      binder
    );
  } else if (isSingletonListPattern(binder)) {
    report(
      { span: { line, column: 1 } },
      `Single-element list bind '${binder} <- query' in ${scanContext.context}. Crashes on 0 or 2+ results.`,
      binder
    );
  }
}
function binderAndValue(statement) {
  if ("Other" in statement && statement.Other.binder !== null) {
    return { name: statement.Other.binder, value: statement.Other.expr, line: statement.Other.span.line };
  }
  if ("Let" in statement) return { name: statement.Let.name, value: statement.Let.value, line: statement.Let.span.line };
  return null;
}
function isPlainIdentifier(name) {
  return /^[A-Za-z0-9_']+$/.test(name.trim());
}
function isConsHeadBinder(binder) {
  const trimmed = binder.trim();
  const inner = trimmed.startsWith("(") && trimmed.endsWith(")") ? trimmed.slice(1, -1).trim() : trimmed;
  if (!inner.startsWith("::")) return false;
  const parts = inner.slice(2).trim().split(/\s+/);
  return parts.length >= 2 && parts[parts.length - 1] === "_";
}
function applicationHead(expr) {
  return "App" in expr ? applicationHead(expr.App.func) : expr;
}
function isQueryApp(expr) {
  const head = applicationHead(expr);
  return "Var" in head && head.Var.qualifier === null && QUERY_FUNCS.includes(head.Var.name);
}
function headOrLast(expr) {
  return "Var" in expr && (expr.Var.name === "head" || expr.Var.name === "last") ? expr.Var.name : null;
}
function fmapHeadOfQuery(expr) {
  if ("BinOp" in expr && expr.BinOp.op === "<$>") {
    const selector = headOrLast(expr.BinOp.lhs);
    return selector !== null && isQueryApp(expr.BinOp.rhs) ? selector : null;
  }
  if ("App" in expr && expr.App.args.length === 2 && "Var" in expr.App.func && expr.App.func.Var.name === "fmap") {
    const selector = headOrLast(expr.App.args[0]);
    return selector !== null && isQueryApp(expr.App.args[1]) ? selector : null;
  }
  return null;
}
function isSingletonListPattern(pattern) {
  const trimmed = pattern.trim();
  if (!trimmed.startsWith("[") || !trimmed.endsWith("]")) return false;
  const inner = trimmed.slice(1, -1).trim();
  return inner.length > 0 && !hasTopLevelComma(inner);
}
function hasTopLevelComma(source) {
  let depth = 0;
  for (const char of source) {
    if (char === "(" || char === "[") depth += 1;
    else if (char === ")" || char === "]") depth -= 1;
    else if (char === "," && depth === 0) return true;
  }
  return false;
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, check };
