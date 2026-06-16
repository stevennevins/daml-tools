# AST DevX Audit

Audit target:
`/home/snevins/code/daml-tools/.claude/worktrees/daml-tools-20260615-115231-2017259`

Date: 2026-06-16

## Assumptions And Success Criteria

- `daml-parser` should be the source of syntax truth for `daml-fmt`,
  `daml-lint`, and future static analyzers.
- Formatter changes must remain byte-safe for comments/trivia and semantically
  safe under the offside-rule token gate plus desugar oracle.
- Linter decisions should use structured parser-derived IR. Raw text should
  remain only as an intentional escape hatch for unmodeled syntax, not as a
  compatibility alias for structured nodes.
- Parser tests should be the first regression layer for syntax shape, span
  losslessness, diagnostics, recovery, and corpus edge cases.
- Dead-code suppressions and backward-compatibility shims should be justified by
  a current contract. Unjustified suppressions should be removed.

## Current State

The codebase has already moved materially toward the requested AST foundation.
Several items that were previously "future work" are now implemented and tested.

- `daml-parser` exposes a typed AST with byte spans for expressions, patterns,
  declarations, templates, interfaces, choices, imports, diagnostics, and
  structured Daml types.
- `daml-lint` lowers `daml_parser::ast` into rule-facing IR in
  `crates/daml-lint/src/parser.rs`. Built-in detector work is mostly on
  `Expr`/`Statement`, not line slicing.
- `daml-fmt` is AST-guided in `crates/daml-fmt/src/layout_ast.rs`; each
  structural edit is gated with `same_tokens`, which compares laid-out tokens
  including virtual offside tokens.
- `DamlType::from_str` is gone. `daml-parser` now fills `FieldDecl.ty`,
  `ChoiceDecl.return_ty`, and key/method type payloads with `ast::Type`; the
  linter classifies with `DamlType::from_type`.
- `Decl::TypeDef` now carries structured `constructors`, `synonym`, and
  `deriving` payloads for common `data`/`newtype`/`type` shapes.
- Record projection precedence is fixed at parser level: tight `a.b` binds
  tighter than application, while spaced `f . g` remains composition.
- `ParseDiagnostic` now includes a `span` and `DiagnosticCategory`; linter
  diagnostics expose category tags and same-line end columns.

Evidence:

- `crates/daml-parser/src/ast.rs` defines `Type`, `DataConstructor`,
  `DiagnosticCategory`, and span-bearing AST nodes.
- `crates/daml-parser/src/data_tests.rs` covers record data fields, enum
  constructors, positional constructors, `newtype`, synonyms, deriving clauses,
  opaque fallback for unmodeled constructors, and finance-corpus field exposure.
- `crates/daml-parser/src/projection_tests.rs` covers projection precedence,
  chained projections, qualified names, spaced/newline composition, assertion
  guards, span tightness, and AST round-trip.
- `crates/daml-parser/src/diag_tests.rs` covers skipped-declaration recovery,
  unsupported legacy syntax, recursion limits, lex diagnostics, and stable
  machine-readable category tags.
- `crates/daml-parser/src/span_tests.rs` runs `render_from_ast` and
  `render_lossless` over the vendored finance corpus.
- `crates/daml-lint/docs/typed-type-ast.md` records the implemented type-AST
  migration and deletion of the old string reparse helpers.
- `crates/daml-lint/docs/raw-field-removal-plan.md` documents the completed
  raw-field compatibility removal.

## Lossless Oracle Finding

The lossless oracle is practical and necessary in this codebase. It is not, by
itself, a code smell.

What it proves:

- `lexer::render_lossless` proves token + trivia reconstruction is
  byte-identical for lex-clean files.
- `ast_span::render_from_ast` proves AST spans nest, do not overlap, and cover
  every non-whitespace byte when combined with trivia.
- Formatter corpus tests use these invariants before relying on span slicing.

Why it is necessary:

- Daml layout is semantic. A whitespace-only edit can change virtual
  `VLBrace`/`VSemi`/`VRBrace` tokens and therefore change meaning.
- The formatter still intentionally passes through unmodeled constructs and
  slices original source bytes from spans. Without an AST-span oracle, a parser
  change could silently drop or overlap source bytes while downstream token
  checks still exercise only edited cases.
- SDK desugar is the strongest semantic oracle but is slower and environment
  dependent. The lossless oracles run in ordinary Rust tests and fail closer to
  parser changes.

Where it could become a smell:

- If an oracle pass were used as an excuse not to add structured AST fields for
  syntax analyzers need, that would be a problem. Current usage is healthier:
  the oracle protects byte fidelity while parser structure is added
  incrementally.

Keep this layering:

1. Parser-level AST shape and span tests.
2. Token/offside equivalence (`same_tokens`) for every formatter edit.
3. Lossless token/trivia and AST-span oracles.
4. SDK desugar equivalence as the semantic corpus gate.

## Dead Code And Compatibility Shims

Removed in this audit pass:

- The non-test public `parse_daml` helper was removed from the linter API;
  tests keep a `pub(crate)` diagnostics-free helper.
- Deprecated custom-rule raw aliases were removed:
  `Choice.body_raw`, `Function.body_raw`, `EnsureClause.raw_text`,
  `Statement.Let.expr`, `Statement.Assert.condition`,
  `Statement.Fetch/Archive/Exercise.cid_expr`, `Statement.Create.raw`, and
  `Statement.Exercise.raw`.
- Rendered party-name compatibility lists were removed from custom-rule IR:
  `choice.controllers`, `template.signatories`, and `template.observers`.
- Duplicate parser type text fields were removed after `ast::Type` gained byte
  spans.

Already removed before this pass:

- The old `DamlType::from_str` string parser and helpers are absent from
  `crates/daml-lint/src/ir.rs`; `rg` only finds historical docs.
- The broad parser module-level `dead_code` allow is absent from
  `crates/daml-parser/src/lib.rs`.

Kept intentionally, not for compatibility:

- `Statement.Other.raw` and `Expr::Unknown.raw` are not compatibility debt;
  they are the deliberate escape hatch for constructs the parser does not
  model structurally.

## Simplifications And Naming

High-value simplifications:

- `Decl::TypeDef` is now doing too much: opaque `type`/`class`/`instance`,
  structured `data`/`newtype`, synonyms, and deriving clauses. A future breaking
  parser release should split it into clearer variants such as `Data`,
  `Newtype`, `TypeSynonym`, and `OpaqueTypeDecl`, while preserving current spans.
- Parser `Type` now carries byte spans. The linter custom-rule `TypeNode`
  surface exposes source ranges with JavaScript slice offsets and parser byte
  offsets so scripts can recover exact type text from `module.source` without
  duplicate string fields.
- `daml-fmt` has grown several gated structural passes. Keep adding one pass at
  a time, but prefer extracting shared traversal/edit helpers only after the
  next repeated pattern is real. Do not introduce a generic rewrite framework
  prematurely.
- `DamlType` classification keys recognized stdlib types by tail name. This is
  an intentional accuracy tradeoff for aliases, but it can misclassify a user
  type whose tail name is `Map`, `Set`, etc. Documented tests should stay close
  to that tradeoff.

Low-value or unsafe simplifications:

- Do not collapse the parser, formatter, and linter AST types into one shared
  public mega-AST. The current parser AST plus linter IR boundary keeps custom
  rule compatibility manageable.
- Do not reintroduce raw custom-rule compatibility fields outside a versioned
  release.

## Remaining AST Work

Parser:

1. Split `Decl::TypeDef` into semantically named variants when the public API can
   break, or add additive helper accessors sooner if analyzers need them.
2. Promote more real-corpus declaration facts into parser tests, especially
   single-line record sums, explicit-brace records, deriving strategies, and
   opaque fallback cases.
3. Add adversarial parser tests for nested comments inside declarations,
   unterminated comments, CRLF in layout-sensitive constructs, tabs in layout
   blocks, and Unicode identifiers in declarations.

Formatter:

1. Keep the current oracle stack. The default `verify-rust.sh` subset is a
   practical quick gate; run full `--desugar` before claiming broad formatter
   changes.
2. Continue adding AST rules one construct at a time. Remaining high-risk
   constructs include record updates, `try`/`catch`, guards, expression
   continuations, mid-line/split `let`, and `data` declaration layout.
3. Each formatter rule should add a parser-level AST/span test first when the
   rule depends on a syntax shape not already covered.

Linter / future static analyzer:

1. Expose structured top-level data/type declarations in linter IR when a rule
   needs them; do not reparse declaration text in detectors.
2. Continue moving built-in detectors and examples away from deprecated raw
   fields.
3. Keep custom-rule API changes versioned and documented.

## Testing Left Policy

For every bug whose root cause is parser shape, add the first regression test in
`daml-parser`:

- AST shape tests for declarations, expressions, patterns, and types.
- Span tightness tests for every new node or span-bearing field.
- `render_from_ast` round-trip tests for new modeled syntax families.
- Corpus extraction tests for facts that only show up in real SDK/finance code.
- Recovery tests for malformed syntax that should not abort later declarations.

Downstream tests should then assert business behavior only: formatter layout,
linter findings, diagnostics serialization, or custom-rule API compatibility.

## Verification Recorded In This Pass

```sh
cargo test -p daml-parser
cargo test -p daml-lint --lib
cargo test -p daml-fmt --lib --features dev-tools
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo run -p daml-lint -- -f json --fail-on critical crates/daml-lint/test-fixtures/good_patterns.daml
cargo run -p daml-lint -- -f json --fail-on critical --rules crates/daml-lint/examples/no-bare-contractid-field.js crates/daml-fmt/original/sdk/compiler/damlc/tests/daml-test-files/CoerceContractId.daml
cargo run -p daml-fmt --bin daml-fmt -- --check crates/daml-lint/test-fixtures/bad_patterns.daml
```

Results:

- `cargo test -p daml-parser`: 90 passed.
- `cargo test -p daml-lint --lib`: 196 passed.
- `cargo test -p daml-fmt --lib --features dev-tools`: 38 passed.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`:
  passed.
- `cargo fmt --all --check`: passed.
- Linter manual sanity: clean fixture produced zero findings and zero parse
  errors; migrated custom rule reported the `cid : ContractId Int` field via
  structured `field.type_`.
- Formatter manual sanity: `--check` reported the intentionally unformatted bad
  fixture, and stdout formatting showed canonical colon/layout normalization.
