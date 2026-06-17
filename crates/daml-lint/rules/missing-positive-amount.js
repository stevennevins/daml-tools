// rules/_helpers.ts
function unwrapConstrainedType(typeNode) {
  return "Constrained" in typeNode ? unwrapConstrainedType(typeNode.Constrained.body) : typeNode;
}
function typeHeadName(typeNode) {
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("Con" in unwrapped) return unwrapped.Con.name;
  if ("App" in unwrapped) return typeHeadName(unwrapped.App.head);
  return null;
}
function isMoneyType(typeNode) {
  if (typeNode === null) return false;
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("Con" in unwrapped) {
    return unwrapped.Con.name === "Decimal" || unwrapped.Con.name === "Numeric";
  }
  return "App" in unwrapped && typeHeadName(unwrapped.App.head) === "Numeric";
}
function isListType(typeNode) {
  if (typeNode === null) return false;
  const unwrapped = unwrapConstrainedType(typeNode);
  if ("List" in unwrapped) return true;
  return "App" in unwrapped && typeHeadName(unwrapped.App.head) === "Set";
}
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
function stripImplicitSelf(name) {
  if (name.startsWith("this.")) return name.slice("this.".length);
  if (name.startsWith("self.")) return name.slice("self.".length);
  return name;
}
function refersTo(expr, name) {
  const ref = refString(expr);
  return ref !== null && (ref === name || stripImplicitSelf(ref) === stripImplicitSelf(name));
}
function conjuncts(expr) {
  if ("BinOp" in expr && expr.BinOp.op === "&&") {
    return [...conjuncts(expr.BinOp.lhs), ...conjuncts(expr.BinOp.rhs)];
  }
  return [expr];
}
function isZeroLiteral(expr) {
  if (!("Lit" in expr) || expr.Lit.kind !== "Int" && expr.Lit.kind !== "Decimal") return false;
  const value = expr.Lit.value.trim();
  return value.length > 0 && value.includes("0") && /^[0.]+$/.test(value);
}
function isNonzeroNumericLiteral(expr) {
  return "Lit" in expr && (expr.Lit.kind === "Int" || expr.Lit.kind === "Decimal") && !isZeroLiteral(expr);
}
function isNonnegativeNumericLiteral(expr) {
  return "Lit" in expr && (expr.Lit.kind === "Int" || expr.Lit.kind === "Decimal");
}
function isStrictPositiveBound(condition, name) {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === ">") return refersTo(lhs, name) && isNonnegativeNumericLiteral(rhs);
  if (op === ">=") return refersTo(lhs, name) && isNonzeroNumericLiteral(rhs);
  if (op === "<") return refersTo(rhs, name) && isNonnegativeNumericLiteral(lhs);
  if (op === "<=") return refersTo(rhs, name) && isNonzeroNumericLiteral(lhs);
  return false;
}
function expressionGuaranteesStrictPositive(condition, name) {
  return conjuncts(condition).some((part) => isStrictPositiveBound(part, name));
}
function isSizeCall(func, args, name) {
  return "Var" in func && (func.Var.name === "length" || func.Var.name === "size") && args.length === 1 && refersTo(args[0], name);
}
function isSizeApp(expr, name) {
  if ("App" in expr) return isSizeCall(expr.App.func, expr.App.args, name);
  if (!("BinOp" in expr) || expr.BinOp.op !== ".") return false;
  const lhs = expr.BinOp.lhs;
  if (!("App" in lhs) || lhs.App.args.length !== 1 || !("Var" in lhs.App.func)) return false;
  if (lhs.App.func.Var.name !== "length" && lhs.App.func.Var.name !== "size") return false;
  const base = refString(lhs.App.args[0]);
  const field = refString(expr.BinOp.rhs);
  if (base === null || field === null) return false;
  return `${base}.${field}` === name || (base === "this" || base === "self") && refersTo(expr.BinOp.rhs, name);
}
function isNullApp(expr, name) {
  return "App" in expr && "Var" in expr.App.func && expr.App.func.Var.name === "null" && expr.App.args.length === 1 && refersTo(expr.App.args[0], name) || "BinOp" in expr && expr.BinOp.op === "$" && "Var" in expr.BinOp.lhs && expr.BinOp.lhs.Var.name === "null" && refersTo(expr.BinOp.rhs, name);
}
function isNonemptyBound(condition, name) {
  if ("BinOp" in condition) {
    const { op, lhs, rhs } = condition.BinOp;
    if (op === ">") return isSizeApp(lhs, name) && isZeroLiteral(rhs);
    if (op === ">=") return isSizeApp(lhs, name) && isNonzeroNumericLiteral(rhs);
    if (op === "<") return isSizeApp(rhs, name) && isZeroLiteral(lhs);
    if (op === "<=") return isSizeApp(rhs, name) && isNonzeroNumericLiteral(lhs);
    if (op === "/=" || op === "!=") {
      return isSizeApp(lhs, name) && isZeroLiteral(rhs) || isSizeApp(rhs, name) && isZeroLiteral(lhs);
    }
    if (op === "$") return "Var" in lhs && lhs.Var.name === "not" && isNullApp(rhs, name);
  }
  return "App" in condition && "Var" in condition.App.func && condition.App.func.Var.name === "not" && condition.App.args.length === 1 && isNullApp(condition.App.args[0], name);
}
function expressionGuaranteesNonempty(condition, name) {
  return conjuncts(condition).some((part) => isNonemptyBound(part, name));
}
function isCeilingOperand(expr) {
  return isNonnegativeNumericLiteral(expr) || refString(expr) !== null;
}
function expressionHasSizeCeilingBound(condition, name) {
  return conjuncts(condition).some((part) => {
    if (!("BinOp" in part)) return false;
    const { op, lhs, rhs } = part.BinOp;
    if (op === "<" || op === "<=") return isSizeApp(lhs, name) && isCeilingOperand(rhs);
    if (op === ">" || op === ">=") return isSizeApp(rhs, name) && isCeilingOperand(lhs);
    return false;
  });
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
function walkExpression(expr, visit) {
  visit(expr);
  if ("DoBlock" in expr) walkBodyExprs(expr.DoBlock.statements, visit);
  for (const child of childExprs(expr)) walkExpression(child, visit);
}
function forEachSubexpr(expr, visit) {
  walkExpression(expr, visit);
}
function walkBodyExprs(statements, visit) {
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
function walkUnconditionalStatements(statements, visit) {
  for (const statement of statements) {
    visit(statement);
    if ("TryCatch" in statement) {
      walkUnconditionalStatements(statement.TryCatch.try_body, visit);
      walkUnconditionalStatements(statement.TryCatch.catch_body, visit);
    }
  }
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

// rules/missing-positive-amount.ts
var NAME = "missing-positive-amount";
var SEVERITY = "high";
var DESCRIPTION = "Choice accepts amount/transfer parameter without positive-value or non-empty check";
function hasPositiveAmountCheck(body, param) {
  let found = false;
  walkUnconditionalStatements(body, (statement) => {
    if ("Assert" in statement && expressionGuaranteesStrictPositive(statement.Assert.condition_expr, param)) {
      found = true;
    }
  });
  return found || hasDefensiveAmountGuard(body, param);
}
function hasDefensiveAmountGuard(body, param) {
  let found = false;
  walkBodyExprs(body, (expr) => {
    if (found || !("App" in expr) || !("Var" in expr.App.func) || expr.App.func.Var.qualifier !== null) return;
    const [condition, action] = expr.App.args;
    if (condition === void 0 || action === void 0) return;
    if (expr.App.func.Var.name === "when" && isNonpositiveTest(condition, param) && aborts(action)) {
      found = true;
    }
    if (expr.App.func.Var.name === "unless" && isStrictPositiveTest(condition, param) && aborts(action)) {
      found = true;
    }
  });
  if (found) return true;
  walkBodyStatements(body, (statement) => {
    if (!("Branch" in statement) || statement.Branch.scrutinee === null) return;
    const { scrutinee, arms } = statement.Branch;
    if (arms.length !== 2 || !arms.every((arm) => arm.pattern === null)) return;
    if (isNonpositiveTest(scrutinee, param) && bodyAborts(arms[0].body) || isStrictPositiveTest(scrutinee, param) && bodyAborts(arms[1].body)) {
      found = true;
    }
  });
  return found;
}
function bodyAborts(body) {
  let found = false;
  walkBodyExprs(body, (expr) => {
    if (aborts(expr)) found = true;
  });
  return found;
}
function isNonpositiveTest(condition, param) {
  if ("BinOp" in condition) {
    const { op, lhs, rhs } = condition.BinOp;
    if (op === "<=") return refersTo(lhs, param) && isZeroLiteral(rhs);
    if (op === ">=") return refersTo(rhs, param) && isZeroLiteral(lhs);
    return false;
  }
  return "App" in condition && "Var" in condition.App.func && condition.App.func.Var.name === "not" && condition.App.args.length === 1 && isStrictPositiveTest(condition.App.args[0], param);
}
function isStrictPositiveTest(condition, param) {
  return isStrictPositiveBound(condition, param);
}
function aborts(action) {
  let found = false;
  forEachSubexpr(action, (expr) => {
    if ("Var" in expr && expr.Var.qualifier === null && (expr.Var.name === "abort" || expr.Var.name === "error" || expr.Var.name === "fail" || expr.Var.name === "assertFail")) {
      found = true;
    }
  });
  return found;
}
function hasNonemptyListCheck(body, param) {
  let found = false;
  walkBodyStatements(body, (statement) => {
    if ("Assert" in statement && expressionGuaranteesNonempty(statement.Assert.condition_expr, param)) {
      found = true;
    }
  });
  return found;
}
function hasMaxOnlyCountCheck(body, field) {
  let hasUpper = false;
  let hasLower = false;
  walkBodyStatements(body, (statement) => {
    if (!("Assert" in statement)) return;
    if (expressionHasSizeCeilingBound(statement.Assert.condition_expr, field)) hasUpper = true;
    if (expressionGuaranteesNonempty(statement.Assert.condition_expr, field)) hasLower = true;
  });
  return hasUpper && !hasLower;
}
function on_choice(choice, _template) {
  const amountParams = choice.parameters.filter((param) => {
    const lowerName = param.name.toLowerCase();
    return isMoneyType(param.type_) && (lowerName.includes("amount") || lowerName === "quantity" || lowerName === "price");
  });
  for (const param of amountParams) {
    if (hasPositiveAmountCheck(choice.body, param.name)) continue;
    report(
      choice,
      `Choice '${choice.name}' accepts Decimal parameter '${param.name}' without asserting > 0.`,
      `${param.name} : Decimal  -- no positive-amount check`
    );
  }
  const listParams = choice.parameters.filter((param) => {
    const lowerName = param.name.toLowerCase();
    return isListType(param.type_) && (lowerName.includes("input") || lowerName.includes("holding") || lowerName.includes("cids"));
  });
  for (const param of listParams) {
    if (hasNonemptyListCheck(choice.body, param.name)) continue;
    report(
      choice,
      `Choice '${choice.name}' accepts list parameter '${param.name}' but has no minimum-length check.`,
      `No 'not $ null ${param.name}' or min-length check`
    );
  }
  for (const field of ["transfer.inputHoldingCids", "transfer.inputs"]) {
    if (!hasMaxOnlyCountCheck(choice.body, field)) continue;
    report(
      choice,
      `Choice '${choice.name}' checks max input count but not min. Empty inputs allowed.`,
      `Bounds '${field}' from above but never asserts it is non-empty`
    );
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
