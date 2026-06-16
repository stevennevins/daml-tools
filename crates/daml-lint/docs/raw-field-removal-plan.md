# Raw custom-rule field removal

Status: complete for the breaking custom-rule surface.

The serialized custom-rule IR no longer carries compatibility-only rendered
fields:

- `Choice.body_raw`, `DamlFunction.body_raw`
- `EnsureClause.raw_text`
- `Statement.Let.expr`
- `Statement.Assert.condition`
- `Statement.Fetch.cid_expr`, `Statement.Archive.cid_expr`,
  `Statement.Exercise.cid_expr`
- `Statement.Create.raw`, `Statement.Exercise.raw`
- rendered party-name aliases `choice.controllers`, `template.signatories`, and
  `template.observers`

Rules should use the structured replacements in `examples/daml-lint.d.ts`:
statement `Expr` payloads, `controller_exprs`, `signatory_exprs`,
`observer_exprs`, and `TypeNode` payloads. `DamlModule.ir_version == 3` marks
this structured-only contract.

`Statement.Other.raw` and `Expr.Unknown.raw` remain intentional escape hatches
for syntax that has no structured IR representation.
