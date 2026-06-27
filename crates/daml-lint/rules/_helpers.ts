import type { CaseAlt, Expr, Statement, TypeNode } from "../examples/daml-lint";

function unwrapConstrainedType(typeNode: TypeNode): TypeNode {
  return "Constrained" in typeNode ? unwrapConstrainedType(typeNode.Constrained.body) : typeNode;
}

function typeHeadName(typeNode: TypeNode): string | null {
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("Con" in unwrapped) return unwrapped.Con.name;
  if ("App" in unwrapped) return typeHeadName(unwrapped.App.head);
  return null;
}

export function isMoneyType(typeNode: TypeNode | null): boolean {
  if (typeNode === null) return false;
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("Con" in unwrapped) {
    return unwrapped.Con.name === "Decimal" || unwrapped.Con.name === "Numeric";
  }
  return "App" in unwrapped && typeHeadName(unwrapped.App.head) === "Numeric";
}

export function isUnboundedType(typeNode: TypeNode | null): boolean {
  if (typeNode === null) return false;
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("Con" in unwrapped) return unwrapped.Con.name === "Text";
  if ("List" in unwrapped) return true;
  if (!("App" in unwrapped)) return false;

  const head = typeHeadName(unwrapped.App.head);
  if (head === "Optional") {
    return isUnboundedType(unwrapped.App.args[0] ?? null);
  }
  return head === "TextMap" || head === "Map" || head === "GenMap" || head === "Set";
}

export function isListType(typeNode: TypeNode | null): boolean {
  if (typeNode === null) return false;
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("List" in unwrapped) return true;
  return "App" in unwrapped && typeHeadName(unwrapped.App.head) === "Set";
}

export function typeDisplay(typeNode: TypeNode | null): string {
  if (typeNode === null) return "unbounded";
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("Con" in unwrapped && unwrapped.Con.name === "Text") return "Text";
  if ("List" in unwrapped) return "List";
  if (!("App" in unwrapped)) return "unbounded";

  const head = typeHeadName(unwrapped.App.head);
  if (head === "TextMap") return "TextMap";
  if (head === "Map" || head === "GenMap") return "Map";
  if (head === "Set") return "List";
  if (head === "Optional") return typeDisplay(unwrapped.App.args[0] ?? null);
  return "unbounded";
}

export function refString(expr: Expr): string | null {
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

function stripImplicitSelf(name: string): string {
  if (name.startsWith("this.")) return name.slice("this.".length);
  if (name.startsWith("self.")) return name.slice("self.".length);
  return name;
}

export function refersTo(expr: Expr, name: string): boolean {
  const ref = refString(expr);
  return ref !== null && (ref === name || stripImplicitSelf(ref) === stripImplicitSelf(name));
}

export function conjuncts(expr: Expr): Expr[] {
  if ("BinOp" in expr && expr.BinOp.op === "&&") {
    return [...conjuncts(expr.BinOp.lhs), ...conjuncts(expr.BinOp.rhs)];
  }
  return [expr];
}

export function isZeroLiteral(expr: Expr): boolean {
  if (!("Lit" in expr) || (expr.Lit.kind !== "Int" && expr.Lit.kind !== "Decimal")) return false;
  const value = expr.Lit.value.trim();
  return value.length > 0 && value.includes("0") && /^[0.]+$/.test(value);
}

export function isNonzeroNumericLiteral(expr: Expr): boolean {
  return "Lit" in expr && (expr.Lit.kind === "Int" || expr.Lit.kind === "Decimal") && !isZeroLiteral(expr);
}

export function isNonzeroNumericDivisor(expr: Expr): boolean {
  if ("Neg" in expr) return isNonzeroNumericDivisor(expr.Neg.expr);
  return isNonzeroNumericLiteral(expr);
}

export function isNonnegativeNumericLiteral(expr: Expr): boolean {
  return "Lit" in expr && (expr.Lit.kind === "Int" || expr.Lit.kind === "Decimal");
}

export function renderText(expr: Expr): string {
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

export function isNonnegativeBound(condition: Expr, name: string): boolean {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === ">" || op === ">=") return refersTo(lhs, name) && isNonnegativeNumericLiteral(rhs);
  if (op === "<" || op === "<=") return refersTo(rhs, name) && isNonnegativeNumericLiteral(lhs);
  if (op === "==") {
    return (refersTo(lhs, name) && isNonzeroNumericLiteral(rhs))
      || (refersTo(rhs, name) && isNonzeroNumericLiteral(lhs));
  }
  return false;
}

export function isNonzeroBound(condition: Expr, name: string): boolean {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === ">") return refersTo(lhs, name) && isZeroLiteral(rhs);
  if (op === "<") return refersTo(rhs, name) && isZeroLiteral(lhs);
  if (op === "/=" || op === "!=") {
    return (refersTo(lhs, name) && isZeroLiteral(rhs)) || (refersTo(rhs, name) && isZeroLiteral(lhs));
  }
  return false;
}

export function isStrictPositiveBound(condition: Expr, name: string): boolean {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === ">") return refersTo(lhs, name) && isNonnegativeNumericLiteral(rhs);
  if (op === ">=") return refersTo(lhs, name) && isNonzeroNumericLiteral(rhs);
  if (op === "<") return refersTo(rhs, name) && isNonnegativeNumericLiteral(lhs);
  if (op === "<=") return refersTo(rhs, name) && isNonzeroNumericLiteral(lhs);
  return false;
}

export function expressionGuaranteesNonnegative(condition: Expr, name: string): boolean {
  return conjuncts(condition).some((part) => isNonnegativeBound(part, name));
}

export function expressionGuaranteesStrictPositive(condition: Expr, name: string): boolean {
  return conjuncts(condition).some((part) => isStrictPositiveBound(part, name));
}

export function expressionGuaranteesNonzero(condition: Expr, name: string): boolean {
  return conjuncts(condition).some((part) => isNonzeroBound(part, name));
}

function isSizeCall(func: Expr, args: Expr[], name: string): boolean {
  return "Var" in func
    && (func.Var.name === "length" || func.Var.name === "size")
    && args.length === 1
    && refersTo(args[0], name);
}

function isSizeApp(expr: Expr, name: string): boolean {
  if ("App" in expr) return isSizeCall(expr.App.func, expr.App.args, name);
  if (!("BinOp" in expr) || expr.BinOp.op !== ".") return false;

  const lhs = expr.BinOp.lhs;
  if (!("App" in lhs) || lhs.App.args.length !== 1 || !("Var" in lhs.App.func)) return false;
  if (lhs.App.func.Var.name !== "length" && lhs.App.func.Var.name !== "size") return false;

  const base = refString(lhs.App.args[0]);
  const field = refString(expr.BinOp.rhs);
  if (base === null || field === null) return false;
  return `${base}.${field}` === name || ((base === "this" || base === "self") && refersTo(expr.BinOp.rhs, name));
}

function isConstantSizeBound(expr: Expr, fieldNames: string[]): boolean {
  if (isNonnegativeNumericLiteral(expr)) return true;
  if (refString(expr) === null) return false;
  return !fieldNames.some((fieldName) => refersTo(expr, fieldName));
}

function isSizeUpperBound(condition: Expr, name: string, fieldNames: string[]): boolean {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === "<" || op === "<=") return isSizeApp(lhs, name) && isConstantSizeBound(rhs, fieldNames);
  if (op === ">" || op === ">=") return isSizeApp(rhs, name) && isConstantSizeBound(lhs, fieldNames);
  if (op === "==") {
    return (isSizeApp(lhs, name) && isConstantSizeBound(rhs, fieldNames))
      || (isSizeApp(rhs, name) && isConstantSizeBound(lhs, fieldNames));
  }
  return false;
}

export function expressionHasSizeUpperBound(condition: Expr, name: string, fieldNames: string[]): boolean {
  return conjuncts(condition).some((part) => isSizeUpperBound(part, name, fieldNames));
}

function isNullApp(expr: Expr, name: string): boolean {
  return ("App" in expr
    && "Var" in expr.App.func
    && expr.App.func.Var.name === "null"
    && expr.App.args.length === 1
    && refersTo(expr.App.args[0], name))
    || ("BinOp" in expr
      && expr.BinOp.op === "$"
      && "Var" in expr.BinOp.lhs
      && expr.BinOp.lhs.Var.name === "null"
      && refersTo(expr.BinOp.rhs, name));
}

function isNonemptyBound(condition: Expr, name: string): boolean {
  if ("BinOp" in condition) {
    const { op, lhs, rhs } = condition.BinOp;
    if (op === ">") return isSizeApp(lhs, name) && isZeroLiteral(rhs);
    if (op === ">=") return isSizeApp(lhs, name) && isNonzeroNumericLiteral(rhs);
    if (op === "<") return isSizeApp(rhs, name) && isZeroLiteral(lhs);
    if (op === "<=") return isSizeApp(rhs, name) && isNonzeroNumericLiteral(lhs);
    if (op === "/=" || op === "!=") {
      return (isSizeApp(lhs, name) && isZeroLiteral(rhs)) || (isSizeApp(rhs, name) && isZeroLiteral(lhs));
    }
    if (op === "$") return "Var" in lhs && lhs.Var.name === "not" && isNullApp(rhs, name);
  }
  return "App" in condition
    && "Var" in condition.App.func
    && condition.App.func.Var.name === "not"
    && condition.App.args.length === 1
    && isNullApp(condition.App.args[0], name);
}

export function expressionGuaranteesNonempty(condition: Expr, name: string): boolean {
  return conjuncts(condition).some((part) => isNonemptyBound(part, name));
}

function isCeilingOperand(expr: Expr): boolean {
  return isNonnegativeNumericLiteral(expr) || refString(expr) !== null;
}

export function expressionHasSizeCeilingBound(condition: Expr, name: string): boolean {
  return conjuncts(condition).some((part) => {
    if (!("BinOp" in part)) return false;
    const { op, lhs, rhs } = part.BinOp;
    if (op === "<" || op === "<=") return isSizeApp(lhs, name) && isCeilingOperand(rhs);
    if (op === ">" || op === ">=") return isSizeApp(rhs, name) && isCeilingOperand(lhs);
    return false;
  });
}

export function statementExprs(statement: Statement): Expr[] {
  if ("Let" in statement) return [statement.Let.value];
  if ("Assert" in statement) return [statement.Assert.condition_expr];
  if ("Fetch" in statement) return [statement.Fetch.cid];
  if ("Archive" in statement) return [statement.Archive.cid];
  if ("Create" in statement) return [statement.Create.argument];
  if ("Exercise" in statement) {
    return statement.Exercise.argument === null
      ? [statement.Exercise.cid]
      : [statement.Exercise.cid, statement.Exercise.argument];
  }
  if ("Other" in statement) return [statement.Other.expr];
  if ("Branch" in statement) return statement.Branch.scrutinee === null ? [] : [statement.Branch.scrutinee];
  return [];
}

function caseAltExprs(alt: CaseAlt): Expr[] {
  const branchBodies = alt.branches.length > 0
    ? alt.branches.flatMap((branch) => [
      ...branch.guards.flatMap((guard) => ("Bool" in guard ? [guard.Bool.expr] : [guard.Pattern.expr])),
      branch.body,
    ])
    : [alt.body];
  return [...branchBodies, ...alt.where_bindings.map((binding) => binding.value)];
}

export function childExprs(expr: Expr): Expr[] {
  if ("App" in expr) return [expr.App.func, ...expr.App.args];
  if ("BinOp" in expr) return [expr.BinOp.lhs, expr.BinOp.rhs];
  if ("Neg" in expr) return [expr.Neg.expr];
  if ("Lambda" in expr) return [expr.Lambda.body];
  if ("If" in expr) return [expr.If.cond, expr.If.then_branch, expr.If.else_branch];
  if ("Case" in expr) {
    return [
      expr.Case.scrutinee,
      ...expr.Case.alts.flatMap((alt) => caseAltExprs(alt)),
    ];
  }
  if ("LetIn" in expr) return [...expr.LetIn.bindings.map((binding) => binding.value), expr.LetIn.body];
  if ("Record" in expr) return [expr.Record.base, ...expr.Record.fields.flatMap((field) => field.value === null ? [] : [field.value])];
  if ("Tuple" in expr) return expr.Tuple.items;
  if ("List" in expr) return expr.List.items;
  return [];
}

function walkExpression(expr: Expr, visit: (expr: Expr) => void): void {
  visit(expr);
  if ("DoBlock" in expr) walkBodyExprs(expr.DoBlock.statements, visit);
  for (const child of childExprs(expr)) walkExpression(child, visit);
}

export function forEachSubexpr(expr: Expr, visit: (expr: Expr) => void): void {
  walkExpression(expr, visit);
}

export function walkBodyExprs(statements: Statement[], visit: (expr: Expr) => void): void {
  for (const statement of statements) {
    for (const expr of statementExprs(statement)) walkExpression(expr, visit);
    if ("TryCatch" in statement) {
      walkBodyExprs(statement.TryCatch.try_body, visit);
      walkBodyExprs(statement.TryCatch.catch_body, visit);
    } else if ("Branch" in statement) {
      for (const arm of statement.Branch.arms) walkBodyExprs(arm.body, visit);
    }
  }
}

export function walkUnconditionalStatements(statements: Statement[], visit: (statement: Statement) => void): void {
  for (const statement of statements) {
    visit(statement);
    if ("TryCatch" in statement) {
      walkUnconditionalStatements(statement.TryCatch.try_body, visit);
      walkUnconditionalStatements(statement.TryCatch.catch_body, visit);
    }
  }
}

export function walkBodyStatements(statements: Statement[], visit: (statement: Statement) => void): void {
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

function walkNestedDoStatements(expr: Expr, visit: (statement: Statement) => void): void {
  if ("DoBlock" in expr) walkBodyStatements(expr.DoBlock.statements, visit);
  for (const child of childExprs(expr)) walkNestedDoStatements(child, visit);
}
