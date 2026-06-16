# Breaking-release plan: removing the raw custom-rule IR fields

The custom-rule IR (`src/ir.rs`, surfaced to rule scripts and documented in
`examples/daml-lint.d.ts`) still carries v1 **raw-text** fields beside the
structured `Expr`/`Statement` payloads. They are reconstructed from the real
parse tree so existing JavaScript rules keep working, but they are deprecated:
new rules should match on structure, not substrings. This file is the plan for
retiring them in a future breaking release.

## Fields in scope (deprecated, to be removed/feature-gated)

| Field | Structured replacement |
|---|---|
| `Choice.body_raw`, `DamlFunction.body_raw` | `body` (`Statement[]`) |
| `EnsureClause.raw_text` | `ensure_clause.expr` (`Expr`) |
| `Statement.Let.expr` | `Let.value` (`Expr`) |
| `Statement.Assert.condition` | `Assert.condition_expr` (`Expr`) |
| `Statement.Fetch/Archive/Exercise.cid_expr` | `.cid` (`Expr`) |
| `Statement.Create.raw` | `template_name` + `argument` (`Expr`) |
| `Statement.Exercise.raw` | `cid` + `choice_name` + `argument` (`Expr`) |

NOT in scope — these are the deliberate raw-source escape hatch and STAY:
`Statement.Other.raw`, the `Unknown` expression's `raw`, and the rendered
party-name lists (`choice.controllers`, `template.signatories`,
`template.observers`). The shipped example
[`no-trace.ts`](../examples/no-trace.ts) matches source text by design, and
[`consuming-choice-signatory-controller.ts`](../examples/consuming-choice-signatory-controller.ts)
matches the party-name strings — both are supported uses, not deprecations.

## Stages

1. **Now (current minor line).** Keep every field. They are marked
   `@deprecated` in `daml-lint.d.ts` and noted in the README migration table.
   All built-in detectors and examples except the explicitly raw-source ones
   read the structured fields (done — see `examples/unguarded-division-ast.ts`).

2. **One more minor release (deprecation window).** No removals. Add a
   one-line runtime notice to stderr the first time a loaded rule script reads a
   deprecated field, if cheap to detect; otherwise rely on the `@deprecated`
   JSDoc surfacing in editors. Re-confirm the corpus rules still pass on
   structured fields only.

3. **Next breaking release (`0.x` → `0.(x+1)` pre-1.0, or the `1.0` cut).**
   Remove the in-scope raw fields from `src/ir.rs` and `daml-lint.d.ts`, OR put
   them behind a `raw-compat` cargo feature (default-off) so a consumer who
   still needs them can opt in for one further release. Bump the IR/contract
   version note at the top of `daml-lint.d.ts`. Ship the README migration table
   as the upgrade guide.

## Compatibility commitment

Do not remove a raw field before this breaking release: they are part of the
public rule-facing contract. The migration table in the README is the
authoritative raw→structured mapping; keep it in sync with `src/ir.rs` and the
`.d.ts` if any field is added or renamed before removal.
