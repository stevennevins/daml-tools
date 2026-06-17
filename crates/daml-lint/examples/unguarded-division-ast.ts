import type { Choice, Expr, SrcPos, Statement, Template } from "./daml-lint";

// The acceptance test for the expression AST: reimplements the
// unguarded-division builtin as a custom rule using ONLY typed nodes —
// no body_raw, no raw, no string matching on source text.
//
// Finds division expressions in choice bodies and reports them unless a
// preceding statement asserts the denominator non-zero / positive, or an
// enclosing `if denom /= 0 then ...` guards it.
// Compile: npx esbuild examples/unguarded-division-ast.ts --bundle --outfile=examples/dist/unguarded-division-ast.js

const NAME = "unguarded-division-ast";
const SEVERITY = "high";
const DESCRIPTION = "Division whose denominator has no prior non-zero assertion (AST rule)";

/** Render a denominator expression to a comparable key: variable paths
 *  like `x`, `transfer.amount` compare structurally. */
function exprKey(e: Expr): string | null {
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

/** Collect division denominators, marking each as guarded if its key is in
 *  scope (asserted earlier, or guarded by an enclosing `if`). */
function divisions(
  e: Expr,
  out: { denom: Expr; span: SrcPos; guarded: boolean }[],
  guarded: Set<string>,
): void {
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
    // then-branch. The else-branch keeps the outer scope.
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
    // handled at statement level by the caller
  }
}

/** Does this condition bound `key` away from zero? Recognizes `key > 0`,
 *  `0 < key`, `key /= 0` — under top-level `&&` only. An upper bound
 *  (`key < N`, `key <= N`) or `>= 0` does NOT prove non-zero, and a guard
 *  under `||` is not guaranteed. */
function guardsKey(cond: Expr, key: string): boolean {
  if (!("BinOp" in cond)) {
    return false;
  }
  const b = cond.BinOp;
  if (b.op === "&&") {
    return guardsKey(b.lhs, key) || guardsKey(b.rhs, key);
  }
  const lk = exprKey(b.lhs);
  const rk = exprKey(b.rhs);
  const isZero = (e: Expr): boolean =>
    "Lit" in e && (e.Lit.kind === "Int" || e.Lit.kind === "Decimal") && parseFloat(e.Lit.value) === 0;
  if (b.op === ">") return lk === key && isZero(b.rhs);
  if (b.op === "<") return rk === key && isZero(b.lhs);
  if (b.op === "/=" || b.op === "!=") {
    return (lk === key && isZero(b.rhs)) || (rk === key && isZero(b.lhs));
  }
  return false;
}

/** Every key the condition guards non-zero, accumulated into `acc`. */
function guardedKeysOf(cond: Expr, acc: Set<string>): void {
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

/** Expressions reachable from a statement, for division scanning. */
function stmtExprs(stmt: Statement): Expr[] {
  if ("Let" in stmt) return [stmt.Let.value];
  if ("Assert" in stmt) return [stmt.Assert.condition_expr];
  if ("Other" in stmt) return [stmt.Other.expr];
  if ("Create" in stmt) return [stmt.Create.argument];
  if ("Exercise" in stmt) {
    return stmt.Exercise.argument !== null ? [stmt.Exercise.argument] : [];
  }
  return [];
}

function checkStatements(stmts: Statement[], choiceName: string): void {
  const guarded = new Set<string>();
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
    const found: { denom: Expr; span: SrcPos; guarded: boolean }[] = [];
    for (const e of stmtExprs(stmt)) {
      divisions(e, found, guarded);
    }
    for (const d of found) {
      // Literal denominators are safe; a guarded denominator is proven non-zero.
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

function on_choice(choice: Choice, _template: Template): void {
  checkStatements(choice.body, choice.name);
}

globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
