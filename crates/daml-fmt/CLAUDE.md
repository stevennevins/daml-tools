# daml-fmt

Goal: a real, publishable code formatter for Daml (3.x / SDK 3.4.x), proven
safe with the compiler itself as the oracle. No official Daml formatter exists.

## Current state

The shipped formatter is an **AST-driven, own-design** layout in Rust, built on
the `daml-parser` pipeline (lexer → offside-rule layout →
recursive-descent AST). Over the 924-file corpus it is **924/924
desugar-equivalent and 924/924 idempotent**.

It models a few constructs canonically and passes everything else through
verbatim. Today: `do`-block indentation, trailing-whitespace + single final
newline, and type-annotation colon spacing (`x : T` → `x: T`). It makes its own
consistent decisions; it does not target any other formatter's output. The
`expected/` tree is a snapshot of *this* formatter's output, used as the
regression baseline.

`corpus/SCOREBOARD.md` is the tracked board; update it on any re-measure.

## How it works (architecture)

The whole design is one safe mechanism applied per construct:

1. **Walk the AST** (`daml_parser::parse::parse_module`). Every node carries a
   byte `Span{start,end}`.
2. **Reindent a block's child lines** to a canonical column (e.g. a `do`-block's
   statements to `do_col + 2`). The block's *anchor* line never moves, so the
   rule is a fixpoint → idempotent by construction. Children shift by one
   uniform delta, so nested blocks ride along.
3. **Slice node CONTENT verbatim from spans** — never re-assemble from AST
   fields. (`FieldAssign.name` drops dotted-update tails like `r with a.b = x`,
   so reconstructing would silently change meaning.)
4. **Gate every candidate** on `same_tokens`: `resolve_layout(lex(out)) ==
   resolve_layout(lex(in))`, i.e. the laid-out token stream *including* the
   offside virtuals (VLBrace/VSemi/VRBrace) is identical. Identical laid-out
   tokens ⇒ identical parse ⇒ identical desugar, so any accepted edit is
   **desugar-safe by construction**.
5. **Fall back to verbatim** when the gate rejects or a node isn't modeled.
   Unmodeled constructs pass through unchanged — safe, and lets us land one
   construct at a time.

On top of the structural pass runs the proven token-gated whitespace + colon
normalization (`normalize_gaps` in `src/lib.rs`).

## Codebase structure

- `src/lib.rs` — `format_source` (the entry point; delegates to the AST
  backend) and `normalize_gaps` (the shared whitespace/colon pass).
- `src/layout_ast.rs` — the AST-driven layout backend (`format_ast`), the
  `do`-block rule, comment-awareness, and the `same_tokens` gate. Unit-tested.
- `src/bin/daml-fmt.rs` — the CLI (`<file...>`, `-w`, `--check`, stdin).
- `src/bin/coverage.rs` — structural edit candidates over modeled constructs.
- `src/bin/lossless-check.rs`, `src/bin/ast-check.rs` — parser round-trip
  checks (token+trivia losslessness; AST byte-faithful reconstruction).
- `original/` — 924 corpus files from digital-asset/daml (Apache-2.0 upstream; some example files carry no per-file SPDX header), all
  verified to desugar clean. The formatting test cases.
- `expected/` — snapshot of the formatter's output over `original/`; the
  regression baseline. Regenerate with `tools/gen-expected.sh`.
- `corpus/` — `SCOREBOARD.md` + manifests. `desugar-ok.txt` is the 924-file
  test manifest; `excluded-error-annotated.txt` / `desugar-fail.txt` are the
  corpus-construction filters.
- `tools/verify-rust.sh` — idempotence plus default desugar subset; `--desugar`
  runs the full-corpus oracle sweep.
- `tools/gen-expected.sh` — regenerate `expected/` from the release binary.
- `test/diff.js` — the SDK-free differential harness (`npm test`): formats all
  924 originals via the release binary, hard-fails on any diff vs `expected/`
  or non-idempotence.

The corpus source is a clone of digital-asset/daml at commit
`7168e37a7257a995053aa68f886112363f132e83` (regenerate at /tmp/daml-corpus with
`git clone` + `git checkout`, then delete the files named in
`corpus/excluded-error-annotated.txt` and `corpus/desugar-fail.txt`).

## Verify commands

Fast tier (no SDK): `npm test` (expected/ + idempotence over the binary) and
`cargo test` (unit tests). Coverage metric: `cargo run --features dev-tools --bin coverage -- original`.

The real semantic bar is the desugar oracle — a formatted file must desugar
byte-identically to the original. The default verifier runs a curated desugar
subset plus full-corpus idempotence; full-corpus desugar remains explicit.
Requires Daml SDK 3.4.11 (`daml version`):

    tools/verify-rust.sh               # idempotence + desugar subset
    tools/verify-rust.sh --desugar     # full-corpus desugar sweep

Desugar one file by hand (run from its directory; filename must match the
module name):

    daml --no-legacy-assistant-warning damlc desugar <file> -o - >/dev/null

After a deliberate formatter change: `tools/gen-expected.sh`, review the diff,
re-run the sweeps, update `SCOREBOARD.md` + the corpus manifests, commit.

## Invariant rules (do not break)

- **The desugar oracle outranks everything.** Parse-level checks hide silent
  semantic changes — always run the equivalence tier before claiming success.
- **Every emitted edit must pass the `same_tokens` gate.** Anything that can't
  is a verbatim span. This is the structural desugar-safety guarantee.
- **Comments are sacred:** never move or re-indent a comment; never measure a
  block's indentation from a comment line; block-comment interiors pass through
  byte-for-byte.
- **Never dedent an originally-indented line to column 0** — column 0 closes
  every layout block.
- **Content comes from spans, never re-assembled AST fields** (dotted updates,
  punned fields, etc. would be silently dropped).
- **Tabs / CRLF:** leave tab-indented lines verbatim; preserve the file's
  newline style. Don't mix.
- Each layout rule should cite a corpus file where it applies.

## Forward-looking improvements

Each new construct = a canonical column rule + verbatim content slicing,
landed behind the gate, verified `--desugar` 924/924 + idempotent, with the
structural candidate metric rising.

Landed (each its own gated structural pass, iterated to a fixpoint,
`--desugar`-clean + idempotent):

- **do-block** + **`do let`**: statements to `do_col + 2`.
- **if/then/else**: `then`/`else` to `if_col + 2`.
- **case/of** (+ line-leading `\case`): alternatives to `case_col + 2`.
- **let-in expression** (line-leading `let`): bindings to `let_col + 2`.
- **`Con with` construction**: fields to `con_col + 2` (record UPDATES
  `expr with` stay verbatim — they hang-align inconsistently).
- **template / interface bodies** — the one STRUCTURED rule: a template's
  `with`/`where` keywords go to `head + 2` and the field / signatory-choice-decl
  blocks to `head + 4` (TWO different deltas, so a 4-space ladder collapses to
  the canonical 2-space one — a uniform-shift attempt was reverted because it
  left fields at `with + 4`); an interface's inline-`where` body sits at
  `head + 2`. Choice `do`-bodies are canonicalized by the do-pass; a choice's
  own internal 4-space ladder is only partly canonicalized (its header rides the
  decl-block shift) — deeper recursion is future work.

Note: the landed anchor for the single-block rules is `line_indent + 2`, NOT the
`with_col + 2` / `let_col + 4` the earlier notes guessed — the corpus uses the
line-indent ladder (e.g. `createCmd Foo with` at indent 6 has fields at 8), and
the formatter follows the corpus, not the guess.

Still candidates:

- **deep choice-body canonicalization** (a choice's own `with`/`controller`/`do`
  internal ladder), **`try`/`catch` handler layout** — left verbatim today.
- **`let … in` mid-line / split**, **`case`/template `where` that would dedent
  below offside**: left verbatim by guards (the gate would reject them anyway).
- **guards** / **blank-line collapsing** (3+ → 2).

Hard-won guidance for the structural rules: a per-line indent rule that isn't
offside-aware breaks desugar (a naive `line_indent + 2` dedents blocks below
what the offside rule requires). Model the offside structure, slice content
from spans, prove every step on the desugar oracle, and never trust a green
idempotence number without `--desugar`.

Provenance (toolchain, prior heuristic baseline, the retired LimeChain port and
the reverted with-reindent experiments) lives in `git log`, not here.
