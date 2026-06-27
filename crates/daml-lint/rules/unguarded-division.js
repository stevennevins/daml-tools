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
function isNonzeroNumericDivisor(expr) {
  if ("Neg" in expr) return isNonzeroNumericDivisor(expr.Neg.expr);
  return isNonzeroNumericLiteral(expr);
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
function isNonzeroBound(condition, name) {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === ">") return refersTo(lhs, name) && isZeroLiteral(rhs);
  if (op === "<") return refersTo(rhs, name) && isZeroLiteral(lhs);
  if (op === "/=" || op === "!=") {
    return refersTo(lhs, name) && isZeroLiteral(rhs) || refersTo(rhs, name) && isZeroLiteral(lhs);
  }
  return false;
}
function expressionGuaranteesNonzero(condition, name) {
  return conjuncts(condition).some((part) => isNonzeroBound(part, name));
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

// rules/unguarded-division.ts
var NAME = "unguarded-division";
var SEVERITY = "high";
var DESCRIPTION = "Division without prior > 0 check on denominator";
function checkBody(statements, ensure, contextName) {
  scanStatements(statements, /* @__PURE__ */ new Set(), { ensure, contextName });
}
function scanStatements(statements, guardedDenominatorKeys, scanContext) {
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
function scanExpr(expr, guardedDenominatorKeys, scanContext) {
  const division = divisionDenominator(expr);
  if (division !== null) {
    const denominatorExpr = unwrapNumericWrapper(division.denominator);
    if (!isNonzeroNumericDivisor(denominatorExpr)) {
      const denominator = denominatorDisplay(denominatorExpr);
      const key = refString(denominatorExpr);
      const guardedByEnclosingIf = key !== null && guardedDenominatorKeys.has(key);
      const guardedByEnsure = scanContext.ensure !== null && expressionGuaranteesNonzero(scanContext.ensure.expr, denominator);
      if (!guardedByEnclosingIf && !guardedByEnsure) {
        report(
          { span: { line: division.span.line, column: 1 } },
          `Unguarded division by '${denominator}' \u2014 no prior > 0 check found in ${scanContext.contextName}.`,
          renderText(expr)
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
function collectNonzeroKeys(condition, out) {
  for (const part of conjuncts(condition)) {
    if (!("BinOp" in part)) continue;
    for (const operand of [part.BinOp.lhs, part.BinOp.rhs]) {
      const key = refString(operand);
      if (key !== null && isNonzeroBound(part, key)) out.add(key);
    }
  }
}
function collectElseNonzeroKeys(condition, out) {
  if (!("BinOp" in condition) || condition.BinOp.op !== "==") return;
  if (isZeroLiteral(condition.BinOp.lhs)) {
    const key = refString(condition.BinOp.rhs);
    if (key !== null) out.add(key);
  } else if (isZeroLiteral(condition.BinOp.rhs)) {
    const key = refString(condition.BinOp.lhs);
    if (key !== null) out.add(key);
  }
}
function divisionDenominator(expr) {
  if ("BinOp" in expr && (expr.BinOp.op === "/" || expr.BinOp.op === "`div`")) {
    return { denominator: expr.BinOp.rhs, span: expr.BinOp.span };
  }
  if ("App" in expr && expr.App.args.length >= 2 && "Var" in expr.App.func && expr.App.func.Var.qualifier === null && expr.App.func.Var.name === "div") {
    return { denominator: expr.App.args[1], span: expr.App.span };
  }
  return null;
}
function denominatorDisplay(expr) {
  if ("Var" in expr || "Con" in expr || "Lit" in expr) return renderText(expr);
  if ("BinOp" in expr && expr.BinOp.op === ".") return renderText(expr);
  return `(${renderText(expr)})`;
}
function unwrapNumericWrapper(expr) {
  if ("App" in expr && "Var" in expr.App.func && expr.App.args.length === 1) {
    if (expr.App.func.Var.name === "intToDecimal" || expr.App.func.Var.name === "intToNumeric") {
      return unwrapNumericWrapper(expr.App.args[0]);
    }
  }
  return expr;
}
function check(module) {
  for (const template of module.templates) {
    for (const choice of template.choices) {
      checkBody(choice.body, template.ensure_clause, `choice '${choice.name}'`);
    }
  }
  for (const func of module.functions) {
    checkBody(func.body, null, `function '${func.name}'`);
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, check };
