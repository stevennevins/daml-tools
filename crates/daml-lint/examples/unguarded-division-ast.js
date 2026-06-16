const NAME = "unguarded-division-ast";
const SEVERITY = "high";
const DESCRIPTION = "Division whose denominator has no prior non-zero assertion (AST rule)";
function exprKey(e) {
  if ("Var" in e) {
    return e.Var.qualifier ? `${e.Var.qualifier}.${e.Var.name}` : e.Var.name;
  }
  if ("BinOp" in e && e.BinOp.op === ".") {
    const l = exprKey(e.BinOp.lhs);
    const r = exprKey(e.BinOp.rhs);
    return l !== null && r !== null ? `${l}.${r}` : null;
  }
  return null;
}
function divisions(e, out, guarded) {
  if ("BinOp" in e) {
    const b = e.BinOp;
    if (b.op === "/" || b.op === "`div`" || b.op === "`divD`") {
      const key = exprKey(b.rhs);
      out.push({ denom: b.rhs, span: b.span, guarded: key !== null && guarded.has(key) });
    }
    divisions(b.lhs, out, guarded);
    divisions(b.rhs, out, guarded);
  } else if ("App" in e) {
    divisions(e.App.func, out, guarded);
    for (const a of e.App.args) divisions(a, out, guarded);
  } else if ("Neg" in e) {
    divisions(e.Neg.expr, out, guarded);
  } else if ("Lambda" in e) {
    divisions(e.Lambda.body, out, guarded);
  } else if ("If" in e) {
    divisions(e.If.cond, out, guarded);
    // `if denom /= 0 then a / denom else ...` — the condition guards the
    // then-branch. The else-branch runs when the guard is false, so it keeps
    // the outer scope.
    const inner = new Set(guarded);
    guardedKeysOf(e.If.cond, inner);
    divisions(e.If.then_branch, out, inner);
    divisions(e.If.else_branch, out, guarded);
  } else if ("Case" in e) {
    divisions(e.Case.scrutinee, out, guarded);
    for (const alt of e.Case.alts) divisions(alt.body, out, guarded);
  } else if ("LetIn" in e) {
    for (const b of e.LetIn.bindings) divisions(b.value, out, guarded);
    divisions(e.LetIn.body, out, guarded);
  } else if ("Record" in e) {
    for (const f of e.Record.fields) {
      if (f.value !== null) divisions(f.value, out, guarded);
    }
  } else if ("Tuple" in e) {
    for (const i of e.Tuple.items) divisions(i, out, guarded);
  } else if ("List" in e) {
    for (const i of e.List.items) divisions(i, out, guarded);
  } else if ("DoBlock" in e) {
  }
}
function guardsKey(cond, key) {
  if (!("BinOp" in cond)) {
    return false;
  }
  const b = cond.BinOp;
  // Only a top-level `&&` conjunction is guaranteed; `||` is not.
  if (b.op === "&&") {
    return guardsKey(b.lhs, key) || guardsKey(b.rhs, key);
  }
  const lk = exprKey(b.lhs);
  const rk = exprKey(b.rhs);
  const isZero = (e) =>
    "Lit" in e && (e.Lit.kind === "Int" || e.Lit.kind === "Decimal") && parseFloat(e.Lit.value) === 0;
  // Strict direction only — `denom > 0`, `0 < denom`, `denom /= 0`. An upper
  // bound (`denom < N`, `denom <= N`) does NOT prove non-zero, nor does `>= 0`.
  if (b.op === ">") return lk === key && isZero(b.rhs);
  if (b.op === "<") return rk === key && isZero(b.lhs);
  if (b.op === "/=" || b.op === "!=") {
    return (lk === key && isZero(b.rhs)) || (rk === key && isZero(b.lhs));
  }
  return false;
}
function guardedKeysOf(cond, acc) {
  if (!("BinOp" in cond)) return;
  const b = cond.BinOp;
  if (b.op === "&&") {
    guardedKeysOf(b.lhs, acc);
    guardedKeysOf(b.rhs, acc);
    return;
  }
  for (const k of [exprKey(b.lhs), exprKey(b.rhs)]) {
    if (k !== null && guardsKey(cond, k)) acc.add(k);
  }
}
function stmtExprs(stmt) {
  if ("Let" in stmt) return [stmt.Let.value];
  if ("Assert" in stmt) return [stmt.Assert.condition_expr];
  if ("Other" in stmt) return [stmt.Other.expr];
  if ("Create" in stmt) return [stmt.Create.argument];
  if ("Exercise" in stmt) {
    return stmt.Exercise.argument !== null ? [stmt.Exercise.argument] : [];
  }
  return [];
}
function checkStatements(stmts, choiceName) {
  const guarded = /* @__PURE__ */ new Set();
  for (const stmt of stmts) {
    if ("Assert" in stmt) {
      guardedKeysOf(stmt.Assert.condition_expr, guarded);
      continue;
    }
    if ("TryCatch" in stmt) {
      checkStatements(stmt.TryCatch.try_body, choiceName);
      checkStatements(stmt.TryCatch.catch_body, choiceName);
      continue;
    }
    // An if/case keeps its arms as separate scopes; a conditional assert in one
    // arm does not guard a division in another, so scan each arm fresh.
    if ("Branch" in stmt) {
      for (const arm of stmt.Branch.arms) {
        checkStatements(arm.body, choiceName);
      }
      continue;
    }
    const found = [];
    for (const e of stmtExprs(stmt)) {
      divisions(e, found, guarded);
    }
    for (const d of found) {
      if ("Lit" in d.denom) {
        continue;
      }
      if (d.guarded) {
        continue;
      }
      const key = exprKey(d.denom);
      report(
        { span: d.span },
        `Division by '${key ?? "<complex expression>"}' in choice '${choiceName}' has no prior non-zero assertion`
      );
    }
  }
}
function on_choice(choice, _template) {
  checkStatements(choice.body, choice.name);
}
