# Detector accuracy roadmap

## Round 3 — DONE (Expr-AST migration + follow-up audit fixes)

All five work items below landed, each TDD. The follow-up adversarial audit was
then run and its confirmed high/medium findings were fixed in the same pass:

- **unguarded-division** is now FULLY structural (walks `BinOp "/"`, infix
  `` `div` ``, prefix `div x y`). A `/` in a string/comment is not a division;
  line-wrapped division is one node; denominators are real sub-expressions
  (`intToDecimal n` → `n`, `(a+b)` reported whole). Guards match the assert
  `condition_expr` and the template `ensure` structurally; `||`/`not`/`>= 0` are
  rejected; the `if denom /= 0 then …` / `if denom == 0 then … else …` idioms are
  recognized.
- **ensure_decimal / unbounded_fields / positive_amount / head_of_list /
  archive_before_execute** all decide on the `Expr`/statement tree now; the
  text-scan helpers (`code_only`, `contains_*`, per-line denominator scan) are
  deleted.
- **example rule** `unguarded-division-ast.js` matches the corrected detector.

Two adversarial audits + two fix passes ran:

- Audit #1: **1 high + 24 medium** → all fixed by hand (the text→AST migration
  proper, plus string/comment/whitespace/structural-guard fixes).
- Audit #2: **0 high + 21 medium + 8 low** → all 29 fixed by the
  `detector-accuracy-fix` workflow (one fixer per cluster, sequential, TDD).
  This pass also closed the deepest root: the parser's `collect_actions` no
  longer **flattens** control flow — `if`/`case` lower to a structured
  `Statement::Branch { arms }` (each arm an independent scope), let-bound
  helpers expand at the **call site**, and qualified `DA.Assert.assertMsg`
  guards are recognized. `daml-lint.d.ts` and the example rules document/handle
  `Branch`.

Total after both passes: **254 tests** green, fmt/clippy `-D warnings`/`cargo
deny` clean, daml-fmt differential **924/924**. The accepted limitations
(low/nit or design choices) are listed under "Known limitations".

The fix workflow is reusable: `Workflow({ name: "detector-accuracy-fix",
args: { auditFile, clusters: [...] } })` — see `.claude/workflows/`.

## History

- **Audit-8** (`2e12836`) — 8 reported bugs (consuming flag, line offsets, division
  guards, archive comment/structured, paren types, stale harness).
- **Sweep hardening** (`154fbf5`) — the systemic substring→token matching, the
  `Numeric`/`Map`/`Set` money/collection types, multi-division, exercise-Archive,
  positive-amount strictness, no-trace word boundary. +19 regression tests.
- **Round 3** (this pass) — the Expr-AST migration above.

## North star for this round

**Move guard / bound / ordering decisions onto the structured `Expr` AST.**

The parser already produces a typed `ir::Expr` (BinOp, App, Lit, Var, …) on
every ensure clause, assert condition, and statement. Walking that tree is
strictly more correct than substring-matching the raw text. `raw_text` should
remain only for *display* (evidence strings), never for *decisions*.

When a decision is made on `Expr` instead of text, a whole class of bugs
disappears at once: inverted conditions (`not (x > 0)`), disjunction
(`a || b`), operator-direction (`length f < N` vs `length f > 0`), and lexical
noise (comments / string literals) all stop mattering, because the tree
carries the real meaning.

## Work items (the deferred sweep findings)

Each item closes one or more findings from the sweep. Do every one **TDD**: a
failing regression test first (RED), then the fix (GREEN).

### 1. Expr-based ensure analysis — closes F8, F17, F31, finishes F35
`EnsureClause::has_positive_bound` / `has_size_bound` walk `self.expr`:
- Only count comparisons reachable through top-level `&&` conjunction; treat
  `not (...)`, `||`, and anything under them as NOT guaranteeing (F8).
- Distinguish strict `>` from `>=`; for size, require an *upper* bound
  (`length f < N` / `<=`), not a mere lower bound `length f > 0` (F17).
- Ignore `Expr` string-literal nodes so a field named only inside a string is
  not "bounded" (F31).
- Field references compared structurally (`Var`/record-access), retiring the
  remaining token-string matching (finishes F35's intent).

### 2. Expr-based division guards + template ensure — closes F24
`UnguardedDivision::has_prior_guard` matches the denominator against assert
`condition_expr` structurally, and threads the enclosing `template.ensure_clause`
in so a denominator bounded by `ensure` is treated as guarded.

### 3. head-of-list redesign — closes F18, F19, F20, F21, F32
Replace the proximity heuristic: track which `let`/binders come from a
`queryFilter`/`query` call through the statement stream, and flag `head` /
`last` / `(!!)` / single-element patterns applied to *those* bindings. No
proximity, no double-reports, no flagging of sorted or unrelated lists.

### 4. positive-amount defensive guards — closes F14
Recognize inverse rejection guards (`when (amount <= 0) abort`,
`assertMsg ... (amount > 0)` already handled) on the `Expr` so a safe
defensive check is not a false positive.

### 5. prefix `div x y` — finishes F22
Detect the prefix application form of integer division (second argument is the
denominator), alongside the infix `` `div` `` and `/` already handled.

## Done when

- Each finding above has a failing-first regression test, now green.
- The full gauntlet is green:
  - `cargo test --workspace --all-features`
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo publish --workspace --dry-run --allow-dirty` (all three crates)
  - `cargo deny check`
  - daml-fmt differential: `cd crates/daml-fmt && node test/diff.js` → 924/924
- The follow-up audit (below) reports **0 new high/medium** accuracy findings
  (low/nit and documented design-choices are acceptable).

## Follow-up audit (the workflow)

Re-run the adversarial accuracy audit after the fixes land:

```
Workflow({ name: "detector-accuracy-audit",
           args: { fixed: "round-3: Expr-based ensure/division guards, head-of-list rewrite, defensive amount guards, prefix div" } })
```

(Or `Workflow({ scriptPath: ".claude/workflows/detector-accuracy-audit.js" })`.)
It fans out one hunter per detector + the IR substrate + the example rules,
each probing the **built binary** empirically, then independently verifies every
candidate before reporting. Pass `args.fixed` so it doesn't re-report what this
round closed; `args.targets` to scope to a subset.

## Ready-to-paste goal

```
/goal Implement the next round of daml-lint detector-accuracy fixes per
crates/daml-lint/docs/accuracy-roadmap.md: move guard/bound/ordering analysis
onto the structured Expr AST and close the deferred findings (Expr-based ensure
analysis, Expr-based division guards + template ensure, head-of-list redesign,
positive-amount defensive guards, prefix div), each with a failing-first
regression test. Then run the detector-accuracy-audit workflow (args.fixed
describing this round) and iterate until it reports 0 new high/medium findings
and the full gauntlet is green.
```

## Known limitations (accepted — design choice)

The audit-fix passes closed the head_of_list, no-trace, ensure-`==`, and
conditional-flattening items that earlier rounds had deferred. What remains is a
small set of deliberate scoping decisions:

- **let-helper resolution is scope-local** — a let-bound helper that performs a
  ledger action is expanded at its call site only within the same `do` block; a
  helper invoked solely inside a nested `do`/`try` is not expanded. This never
  adds a false positive (it can only under-report), and matches the audit's
  narrowest-correct guidance.
- **parser `.` precedence** — record projection `.` lexes as an operator looser
  than application, so `length this.note` parses as `(length this).note`. The
  detectors compensate for the common `this.`/`self.` case; the general fix is a
  parser-precedence change, deferred to keep the formatter's 924/924 differential
  stable.
- **`unqualified-da-import.js`** (example) flags an empty import list
  `import DA.Map ()` — a teaching template, not a core detector. (low)
