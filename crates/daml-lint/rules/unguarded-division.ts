import type { DamlModule, EnsureClause, Expr, SrcPos, Statement } from "../examples/daml-lint";
import {
  childExprs,
  conjuncts,
  expressionGuaranteesNonzero,
  isNonzeroBound,
  isNonzeroNumericDivisor,
  isZeroLiteral,
  refString,
  renderText,
  statementExprs,
} from "./_helpers";

const NAME = "unguarded-division";
const SEVERITY = "high";
const DESCRIPTION = "Division without prior > 0 check on denominator";

interface ScanContext {
  ensure: EnsureClause | null;
  contextName: string;
}

function checkBody(statements: Statement[], ensure: EnsureClause | null, contextName: string): void {
  scanStatements(statements, new Set(), { ensure, contextName });
}

function scanStatements(statements: Statement[], guardedDenominatorKeys: Set<string>, scanContext: ScanContext): void {
  const currentGuardedKeys = new Set(guardedDenominatorKeys);
  for (const statement of statements) {
    for (const expr of statementExprs(statement)) {
      scanExpr(expr, currentGuardedKeys, scanContext);
    }

    if ("TryCatch" in statement) {
      scanStatements(statement.TryCatch.try_body, currentGuardedKeys, scanContext);
      scanStatements(statement.TryCatch.catch_body, currentGuardedKeys, scanContext);
    } else if ("Branch" in statement) {
      for (const arm of statement.Branch.arms) scanStatements(arm.body, currentGuardedKeys, scanContext);
    }

    if ("Assert" in statement) collectNonzeroKeys(statement.Assert.condition_expr, currentGuardedKeys);
  }
}

function scanExpr(expr: Expr, guardedDenominatorKeys: Set<string>, scanContext: ScanContext): void {
  const division = divisionDenominator(expr);
  if (division !== null) {
    const denominatorExpr = unwrapNumericWrapper(division.denominator);
    if (!isNonzeroNumericDivisor(denominatorExpr)) {
      const denominator = denominatorDisplay(denominatorExpr);
      const key = refString(denominatorExpr);
      const guardedByEnclosingIf = key !== null && guardedDenominatorKeys.has(key);
      const guardedByEnsure = scanContext.ensure !== null
        && expressionGuaranteesNonzero(scanContext.ensure.expr, denominator);
      if (!guardedByEnclosingIf && !guardedByEnsure) {
        report(
          { span: { line: division.span.line, column: 1 } },
          `Unguarded division by '${denominator}' — no prior > 0 check found in ${scanContext.contextName}.`,
          renderText(expr),
        );
      }
    }
  }

  if ("If" in expr) {
    scanExpr(expr.If.cond, guardedDenominatorKeys, scanContext);
    const thenGuarded = new Set(guardedDenominatorKeys);
    collectNonzeroKeys(expr.If.cond, thenGuarded);
    scanExpr(expr.If.then_branch, thenGuarded, scanContext);
    const elseGuarded = new Set(guardedDenominatorKeys);
    collectElseNonzeroKeys(expr.If.cond, elseGuarded);
    scanExpr(expr.If.else_branch, elseGuarded, scanContext);
  } else if ("DoBlock" in expr) {
    scanStatements(expr.DoBlock.statements, guardedDenominatorKeys, scanContext);
  } else {
    for (const child of childExprs(expr)) scanExpr(child, guardedDenominatorKeys, scanContext);
  }
}

function collectNonzeroKeys(condition: Expr, out: Set<string>): void {
  for (const part of conjuncts(condition)) {
    if (!("BinOp" in part)) continue;
    for (const operand of [part.BinOp.lhs, part.BinOp.rhs]) {
      const key = refString(operand);
      if (key !== null && isNonzeroBound(part, key)) out.add(key);
    }
  }
}

function collectElseNonzeroKeys(condition: Expr, out: Set<string>): void {
  if (!("BinOp" in condition) || condition.BinOp.op !== "==") return;
  if (isZeroLiteral(condition.BinOp.lhs)) {
    const key = refString(condition.BinOp.rhs);
    if (key !== null) out.add(key);
  } else if (isZeroLiteral(condition.BinOp.rhs)) {
    const key = refString(condition.BinOp.lhs);
    if (key !== null) out.add(key);
  }
}

function divisionDenominator(expr: Expr): { denominator: Expr; span: SrcPos } | null {
  if ("BinOp" in expr && (expr.BinOp.op === "/" || expr.BinOp.op === "`div`")) {
    return { denominator: expr.BinOp.rhs, span: expr.BinOp.span };
  }
  if ("App" in expr
    && expr.App.args.length >= 2
    && "Var" in expr.App.func
    && expr.App.func.Var.qualifier === null
    && expr.App.func.Var.name === "div") {
    return { denominator: expr.App.args[1], span: expr.App.span };
  }
  return null;
}

function denominatorDisplay(expr: Expr): string {
  if ("Var" in expr || "Con" in expr || "Lit" in expr) return renderText(expr);
  if ("BinOp" in expr && expr.BinOp.op === ".") return renderText(expr);
  return `(${renderText(expr)})`;
}

function unwrapNumericWrapper(expr: Expr): Expr {
  if ("App" in expr && "Var" in expr.App.func && expr.App.args.length === 1) {
    if (expr.App.func.Var.name === "intToDecimal" || expr.App.func.Var.name === "intToNumeric") {
      return unwrapNumericWrapper(expr.App.args[0]);
    }
  }
  return expr;
}

function check(module: DamlModule): void {
  for (const template of module.templates) {
    for (const choice of template.choices) {
      checkBody(choice.body, template.ensure_clause, `choice '${choice.name}'`);
    }
  }
  for (const func of module.functions) {
    checkBody(func.body, null, `function '${func.name}'`);
  }
}

// QuickJS discovers rule metadata and visitors by evaluating these names.
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, check };
