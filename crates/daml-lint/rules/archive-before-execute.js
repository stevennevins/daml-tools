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

// rules/archive-before-execute.ts
var NAME = "archive-before-execute";
var SEVERITY = "high";
var DESCRIPTION = "Contract archived before try/catch \u2014 archived contract lost if execution fails";
function checkStatements(statements) {
  const pending = [];
  for (let index = 0; index < statements.length; index++) {
    const statement = statements[index];
    if ("Archive" in statement) {
      pending.push({
        line: statement.Archive.span.line,
        kind: isFetchAndArchive(statements, index) ? "fetchAndArchive" : "archive",
        cid: renderText(statement.Archive.cid)
      });
    } else if ("Exercise" in statement && (statement.Exercise.choice_name === "Archive" || statement.Exercise.choice_name.endsWith(".Archive"))) {
      pending.push({
        line: statement.Exercise.span.line,
        kind: "archive",
        cid: renderText(statement.Exercise.cid)
      });
    } else if ("TryCatch" in statement) {
      for (const archived of pending.splice(0)) {
        reportArchive(archived, statement.TryCatch.span.line);
      }
      checkStatements(statement.TryCatch.try_body);
      checkStatements(statement.TryCatch.catch_body);
    } else if ("Branch" in statement) {
      for (const arm of statement.Branch.arms) checkStatements(arm.body);
    }
  }
}
function isFetchAndArchive(statements, index) {
  const statement = statements[index];
  const next = statements[index + 1];
  return "Archive" in statement && next !== void 0 && "Fetch" in next && next.Fetch.span.line === statement.Archive.span.line && JSON.stringify(next.Fetch.cid) === JSON.stringify(statement.Archive.cid);
}
function reportArchive(archived, tryLine) {
  report(
    { span: { line: archived.line, column: 1 } },
    `Contract archived via '${archived.kind}' at line ${archived.line} before try/catch block at line ${tryLine}. If execution fails, the archived contract is permanently consumed.`,
    `${archived.kind} ${archived.cid.trim()}
  ...
  try do ...`
  );
}
function on_choice(choice, _template) {
  checkStatements(choice.body);
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
