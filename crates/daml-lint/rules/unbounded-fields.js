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
function isUnboundedType(typeNode) {
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
function typeDisplay(typeNode) {
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
function isConstantSizeBound(expr, fieldNames) {
  if (isNonnegativeNumericLiteral(expr)) return true;
  if (refString(expr) === null) return false;
  return !fieldNames.some((fieldName) => refersTo(expr, fieldName));
}
function isSizeUpperBound(condition, name, fieldNames) {
  if (!("BinOp" in condition)) return false;
  const { op, lhs, rhs } = condition.BinOp;
  if (op === "<" || op === "<=") return isSizeApp(lhs, name) && isConstantSizeBound(rhs, fieldNames);
  if (op === ">" || op === ">=") return isSizeApp(rhs, name) && isConstantSizeBound(lhs, fieldNames);
  if (op === "==") {
    return isSizeApp(lhs, name) && isConstantSizeBound(rhs, fieldNames) || isSizeApp(rhs, name) && isConstantSizeBound(lhs, fieldNames);
  }
  return false;
}
function expressionHasSizeUpperBound(condition, name, fieldNames) {
  return conjuncts(condition).some((part) => isSizeUpperBound(part, name, fieldNames));
}

// rules/unbounded-fields.ts
var NAME = "unbounded-fields";
var SEVERITY = "medium";
var DESCRIPTION = "Unbounded Text/TextMap/List field with no ensure clause bounding its size";
function on_template(template) {
  const unboundedFields = template.fields.filter((field) => isUnboundedType(field.type_));
  if (unboundedFields.length === 0) return;
  const fieldNames = template.fields.map((field) => field.name);
  const unguardedNames = [];
  for (const field of unboundedFields) {
    const hasBound = template.ensure_clause !== null && expressionHasSizeUpperBound(template.ensure_clause.expr, field.name, fieldNames);
    if (!hasBound) unguardedNames.push(field.name);
  }
  if (unguardedNames.length === 0) return;
  const typeDesc = unguardedNames.length === 1 ? `${typeDisplay(unboundedFields.find((field) => field.name === unguardedNames[0])?.type_ ?? null)} field` : "fields";
  report(
    template,
    `Template '${template.name}' has unbounded ${typeDesc} '${unguardedNames.join("', '")}' with no ensure clause bounding their length.`,
    `Fields without size bounds: ${unguardedNames.join(", ")}`
  );
}
