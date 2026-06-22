# Formatter scoreboard

All rows measured with the same harness over the same 924-file corpus
(`corpus/desugar-ok.txt`, originals in `original/`, SDK 3.4.11):

1. **parse-ok** — formatted file still desugars (`daml damlc desugar`)
2. **equivalent** — desugar output equivalent to the original's; import
   declarations are sorted before comparison because default import organization
   may change package identity without changing source-level imports or program
   bodies
3. **semantics-changed** — parses but desugars differently (silent corruption)
4. **idempotent** — format(format(x)) == format(x)

| Formatter | parse-ok | equivalent | semantics-changed | parse-broken | crashes | idempotent |
|---|---|---|---|---|---|---|
| LimeChain damlfmt 0.0.5 (pristine) | 420 | 372 | 48 | 504 | 0 | 920/924 |
| LimeChain + grug patches (2026-06-12) | 921 | 921 | 0 | 3 | 0 | 924/924 |
| daml-fmt LimeChain-port-on-daml-lint (2026-06-13) | 924 | 924 | 0 | 0 | 0 | 924/924 |
| **daml-fmt AST formatter — own pattern (2026-06-19)** | **924** | **924** | **0** | **0** | **0** | **924/924** |

The pristine row is the published tool's true quality; its VS Code extension
hides the failures by refusing edits that fail its own desugar check.
The patched row is the ceiling of the regex-heuristic approach after ~15
root-cause fixes — the remaining 3 failures are one family (the
deeper-with field-indent conflict) that needs a real parser.

The **AST formatter row is the shipped backend** (`src/layout_ast.rs`,
`format_source` -> `format_ast`). It is OUR OWN design on the `daml-syntax`
seam over the parser pipeline (lexer → offside layout → recursive-descent AST), with **no LimeChain
derivative** — the authorized port (`src/layout.rs`) was deleted once this
landed. Mechanism: walk the AST and reindent each modeled construct's child
lines to a canonical column, with pure reindent passes gated on the laid-out
token stream (`same_tokens` via `daml_syntax::SourceTokens::laid_out_tokens`).
Modeled: module/export/import continuations; `do`-blocks (including a `do`
opening with `let`) → `do_col + 2`; `if`/`then`/`else`; `case … of` alts;
`let … in`; `Con with` construction fields and record-update fields; the
structured `template`/`interface` body rule; choice internals;
`data`/type/exception declaration ladders; class/instance head-line `where`
bodies aligned to their established body column; function guards/where
bindings; `try`/`catch` handlers; explicit tuple/list continuations; long
applications; infix chains; lambdas; inline `if`/`case`/`let`/record
construction forms; and default import organization. On top runs the
token-gated whitespace + colon-spacing + blank-line normalization.
Structural candidate metric:
`cd crates/daml-fmt && cargo run --features dev-tools --bin coverage -- original` reports
1949 structural edit candidates / 8256 modeled constructs across the 924-file
corpus (313 files with candidates). This is not a percentage: one construct can
produce multiple edits, and canonical constructs produce none.

Note: byte-match to a reference baseline is RETIRED as the metric. The
formatter makes its own consistent
layout decisions and DIVERGES from the LimeChain `expected/` — `expected/` is
now a snapshot of THIS formatter's output (regenerated 2026-06-16). One
deliberate divergence worth recording: `default modify : T` now normalizes its
colon like any other type-annotation colon (`default modify: T`); the old
token-only heuristic kept that space as an undecidable edge case. The desugar
oracle vouches it is meaning-preserving.

Measure coverage with:

```sh
cd crates/daml-fmt && cargo run --features dev-tools --bin coverage -- original
```

This reports structural edit candidates over modeled constructs and replaces the
retired `score` bin. Also run `tools/verify-rust.sh` (default desugar subset and
idempotence; `--desugar` for the full oracle) +
`npm test` (SDK-free expected/ + idempotence over the Rust binary).

Reproduce: pristine vsix → /tmp dir (marketplace vspackage endpoint, gunzip,
unzip), generate outputs with the corpus inputs, then run the parse +
equivalence sweeps (xargs patterns in git history / CLAUDE.md).
