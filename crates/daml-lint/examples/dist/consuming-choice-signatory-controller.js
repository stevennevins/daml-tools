// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.

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

// examples/consuming-choice-signatory-controller.ts
var NAME = "consuming-choice-signatory-controller";
var SEVERITY = "medium";
var DESCRIPTION = "Consuming choices should have at least one signatory controller";
function on_choice(choice, template) {
  if (!choice.consuming) {
    return;
  }
  const signatories = partyExprs(template.signatory_exprs).map(renderText);
  if (partyExprs(choice.controller_exprs).some((c) => {
    const text = renderText(c);
    return text === "signatory this" || text.startsWith("signatory ") || signatories.includes(text);
  })) {
    return;
  }
  report(choice, `Consuming choice '${choice.name}' has no signatory among its controllers`);
}
function partyExprs(exprs) {
  return exprs.flatMap((e) => "List" in e ? e.List.items : [e]);
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
