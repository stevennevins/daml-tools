# Detector accuracy roadmap — next round

## Where we are

Two accuracy passes have landed on `main`'s branch:

- **Audit-8** (`2e12836`) — 8 reported bugs (consuming flag, line offsets, division
  guards, archive comment/structured, paren types, stale harness).
- **Sweep hardening** (`154fbf5`) — the systemic substring→token matching, the
  `Numeric`/`Map`/`Set` money/collection types, multi-division, exercise-Archive,
  positive-amount strictness, no-trace word boundary. +19 regression tests.

Both hardened the *worst* of the text-heuristic matching. The detectors still
lean on scanning `raw_text` / `body_raw` plus a partial IR.

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
