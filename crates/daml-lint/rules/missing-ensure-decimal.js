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
function isNonnegativeBound(condition, name) {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === ">" || op === ">=") return refersTo(lhs, name) && isNonnegativeNumericLiteral(rhs);
  if (op === "<" || op === "<=") return refersTo(rhs, name) && isNonnegativeNumericLiteral(lhs);
  if (op === "==") {
    return refersTo(lhs, name) && isNonzeroNumericLiteral(rhs) || refersTo(rhs, name) && isNonzeroNumericLiteral(lhs);
  }
  return false;
}
function expressionGuaranteesNonnegative(condition, name) {
  return conjuncts(condition).some((part) => isNonnegativeBound(part, name));
}

// rules/missing-ensure-decimal.ts
var NAME = "missing-ensure-decimal";
var SEVERITY = "high";
var DESCRIPTION = "Template has Decimal field with no positivity bound in its ensure clause";
function on_template(template) {
  const decimalFields = template.fields.filter((field) => isMoneyType(field.type_));
  for (const field of decimalFields) {
    const hasBound = template.ensure_clause !== null && expressionGuaranteesNonnegative(template.ensure_clause.expr, field.name);
    if (hasBound) continue;
    const evidence = template.ensure_clause === null ? `${field.name} : Decimal  -- no ensure clause found` : `${field.name} : Decimal  -- ensure clause does not bound this field`;
    report(
      template,
      `Template '${template.name}' has Decimal field '${field.name}' with no positivity bound (e.g. \`${field.name} > 0\`) in its ensure clause.`,
      evidence
    );
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_template };
