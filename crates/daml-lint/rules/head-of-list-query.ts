import { childExprs, refString, renderText, statementExprs } from "./_helpers";

const NAME = "head-of-list-query";
const SEVERITY = "medium";
const DESCRIPTION = "Head-of-list pattern on query result (non-deterministic order)";

const QUERY_FUNCS = ["query", "queryFilter", "queryContractId", "queryInterface"];

interface HeadScanContext {
  context: string;
}

function check(module: DamlModule): void {
  for (const template of module.templates) {
    for (const choice of template.choices) {
      scanStatements(choice.body, new Set(), { context: `choice '${choice.name}'` });
    }
  }
  for (const func of module.functions) {
    scanStatements(func.body, new Set(), { context: `function '${func.name}'` });
  }
}

function scanStatements(statements: Statement[], queryBinders: Set<string>, scanContext: HeadScanContext): void {
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

function scanExpr(expr: Expr, queryBinders: Set<string>, scanContext: HeadScanContext): void {
  const onQuery = (candidate: Expr): boolean => {
    const ref = refString(candidate);
    return ref !== null && queryBinders.has(ref);
  };

  if ("App" in expr && expr.App.args.length === 1 && headOrLast(expr.App.func) !== null && onQuery(expr.App.args[0])) {
    const selector = headOrLast(expr.App.func)!;
    report(
      { span: { line: expr.App.span.line, column: 1 } },
      `\`${selector}\` on query result in ${scanContext.context}. Query results have non-deterministic order.`,
      `${selector} ${renderText(expr.App.args[0])}`,
    );
  } else if ("BinOp" in expr && expr.BinOp.op === "$" && onQuery(expr.BinOp.rhs)) {
    const selector = headOrLast(expr.BinOp.lhs);
    if (selector !== null) {
      report(
        { span: { line: expr.BinOp.span.line, column: 1 } },
        `\`${selector} $\` on query result in ${scanContext.context}. Query results have non-deterministic order.`,
        `${selector} $ ${renderText(expr.BinOp.rhs)}`,
      );
    }
  } else if ("BinOp" in expr && expr.BinOp.op === "!!" && onQuery(expr.BinOp.lhs)) {
    report(
      { span: { line: expr.BinOp.span.line, column: 1 } },
      `Index \`!!\` into query result in ${scanContext.context}. Query results have non-deterministic order.`,
      `${renderText(expr.BinOp.lhs)} !!`,
    );
  } else if ("DoBlock" in expr) {
    scanStatements(expr.DoBlock.statements, new Set(queryBinders), scanContext);
  }

  for (const child of childExprs(expr)) scanExpr(child, queryBinders, scanContext);
}

function updateQueryBinding(statement: Statement, queryBinders: Set<string>, scanContext: HeadScanContext): void {
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
          `${selector} <$> query`,
        );
      }
      queryBinders.delete(name);
    }
  }
}

function scanBranchPatterns(
  scrutinee: Expr,
  arms: BranchArm[],
  span: SrcPos,
  queryBinders: Set<string>,
  scanContext: HeadScanContext,
): void {
  const ref = refString(scrutinee);
  if (ref === null || !queryBinders.has(ref)) return;

  for (const arm of arms) {
    if (arm.pattern === null) continue;
    if (isConsHeadBinder(arm.pattern)) {
      report(
        { span: { line: span.line, column: 1 } },
        `Head-of-list pattern '${arm.pattern}' on query result in ${scanContext.context}. Query results have non-deterministic order.`,
        arm.pattern,
      );
    } else if (isSingletonListPattern(arm.pattern)) {
      report(
        { span: { line: span.line, column: 1 } },
        `Single-element list pattern '${arm.pattern}' on query result in ${scanContext.context}. Crashes on 0 or 2+ results.`,
        arm.pattern,
      );
    }
  }
}

function flagDestructureBind(binder: string, line: number, scanContext: HeadScanContext): void {
  if (isConsHeadBinder(binder)) {
    report(
      { span: { line, column: 1 } },
      `Head-of-list bind '${binder} <- query' in ${scanContext.context}. Query results have non-deterministic order.`,
      binder,
    );
  } else if (isSingletonListPattern(binder)) {
    report(
      { span: { line, column: 1 } },
      `Single-element list bind '${binder} <- query' in ${scanContext.context}. Crashes on 0 or 2+ results.`,
      binder,
    );
  }
}

function binderAndValue(statement: Statement): { name: string; value: Expr; line: number } | null {
  if ("Other" in statement && statement.Other.binder !== null) {
    return { name: statement.Other.binder, value: statement.Other.expr, line: statement.Other.span.line };
  }
  if ("Let" in statement) return { name: statement.Let.name, value: statement.Let.value, line: statement.Let.span.line };
  return null;
}

function isPlainIdentifier(name: string): boolean {
  return /^[A-Za-z0-9_']+$/.test(name.trim());
}

function isConsHeadBinder(binder: string): boolean {
  const trimmed = binder.trim();
  const inner = trimmed.startsWith("(") && trimmed.endsWith(")")
    ? trimmed.slice(1, -1).trim()
    : trimmed;
  if (!inner.startsWith("::")) return false;
  const parts = inner.slice(2).trim().split(/\s+/);
  return parts.length >= 2 && parts[parts.length - 1] === "_";
}

function applicationHead(expr: Expr): Expr {
  return "App" in expr ? applicationHead(expr.App.func) : expr;
}

function isQueryApp(expr: Expr): boolean {
  const head = applicationHead(expr);
  return "Var" in head && head.Var.qualifier === null && QUERY_FUNCS.includes(head.Var.name);
}

function headOrLast(expr: Expr): "head" | "last" | null {
  return "Var" in expr && (expr.Var.name === "head" || expr.Var.name === "last") ? expr.Var.name : null;
}

function fmapHeadOfQuery(expr: Expr): "head" | "last" | null {
  if ("BinOp" in expr && expr.BinOp.op === "<$>") {
    const selector = headOrLast(expr.BinOp.lhs);
    return selector !== null && isQueryApp(expr.BinOp.rhs) ? selector : null;
  }
  if ("App" in expr
    && expr.App.args.length === 2
    && "Var" in expr.App.func
    && expr.App.func.Var.name === "fmap") {
    const selector = headOrLast(expr.App.args[0]);
    return selector !== null && isQueryApp(expr.App.args[1]) ? selector : null;
  }
  return null;
}

function isSingletonListPattern(pattern: string): boolean {
  const trimmed = pattern.trim();
  if (!trimmed.startsWith("[") || !trimmed.endsWith("]")) return false;
  const inner = trimmed.slice(1, -1).trim();
  return inner.length > 0 && !hasTopLevelComma(inner);
}

function hasTopLevelComma(source: string): boolean {
  let depth = 0;
  for (const char of source) {
    if (char === "(" || char === "[") depth += 1;
    else if (char === ")" || char === "]") depth -= 1;
    else if (char === "," && depth === 0) return true;
  }
  return false;
}

// QuickJS discovers rule metadata and visitors by evaluating these names.
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, check };
