# daml-fmt

[![CI](https://github.com/stevennevins/daml-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/stevennevins/daml-tools/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

A code formatter for Daml (3.x / SDK 3.4.x), differential-tested against a
compiler-verified corpus. No official Daml formatter exists; this project
builds one and proves it safe with the compiler itself as the oracle.

Part of the [daml-tools](https://github.com/stevennevins/daml-tools) workspace.
The formatter is **Rust on the [`daml-parser`](https://crates.io/crates/daml-parser) crate** (lexer →
offside-rule layout → recursive-descent AST). It is an **AST-driven, own-design** layout —
it walks the parse tree, reindents the constructs it models, and passes
everything else through verbatim. Every change is gated on the laid-out token
stream, so it is **desugar-safe by construction**: a formatted file desugars
byte-identically to the original.

Results over the 924-file corpus (`corpus/SCOREBOARD.md` is the board):

| tier | result |
|---|---|
| desugar byte-identical (semantics proven unchanged) | 924 / 924 |
| parses | 924 / 924 |
| semantics silently changed | 0 |
| idempotent (`format(format(x)) == format(x)`) | 924 / 924 |

What it lays out today: `do`-block indentation, `if`/`then`/`else` clauses,
`case` alternatives, `let … in` bindings, constructor `with` fields,
template/interface bodies, trailing-whitespace + final newline, and
type-annotation colon spacing (`x : T` → `x: T`). Unmodeled constructs (guards,
record updates, `data` declarations, type definitions, expression
continuations) pass through verbatim. It makes its own consistent layout
decisions and does not aim to match any other formatter's output.

## Build & install

daml-fmt depends only on the [`daml-parser`](https://crates.io/crates/daml-parser) crate (the shared
lexer + offside layout + parser), never on `daml-lint`. Both live in the
[daml-tools](https://github.com/stevennevins/daml-tools) workspace, so a normal
workspace checkout has everything it needs.

```sh
cargo install daml-fmt                       # from crates.io
# or from the workspace repo:
cargo install --git https://github.com/stevennevins/daml-tools daml-fmt
# or from a local checkout:
git clone https://github.com/stevennevins/daml-tools.git
cd daml-tools
cargo build --release --bin daml-fmt         # target/release/daml-fmt
cargo install --path crates/daml-fmt         # puts daml-fmt on your PATH
```

## Usage

```sh
daml-fmt Foo.daml              # formatted source to stdout
daml-fmt -w src/**/*.daml      # rewrite files in place
daml-fmt --check src/*.daml    # list unformatted files, exit 1 if any
cat Foo.daml | daml-fmt        # stdin → stdout
```

Exit codes: 0 ok, 1 `--check` found unformatted files, 2 error.

## Workspace-Only Tests

These commands require a full repository checkout. The published crate excludes
the corpus, baselines, scripts, and contributor notes.

Fast tier (no SDK, ~seconds — formats all 924 corpus files via the release
binary, compares to the committed `expected/` baseline, checks idempotence):

```sh
npm test                  # node test/diff.js, drives target/release/daml-fmt
cargo test                # unit tests for the layout helpers
```

The real semantic bar is the desugar oracle: the formatted file must desugar
byte-identically to the original (`daml damlc desugar`). The default verifier
runs that oracle on a curated subset and keeps full-corpus idempotence:

```sh
tools/verify-rust.sh               # needs Daml SDK 3.4.11 on PATH for desugar
tools/verify-rust.sh --desugar     # full-corpus desugar sweep
```

The structural candidate metric (edit candidates over modeled constructs):

```sh
cargo run --features dev-tools --bin coverage -- original
```

Parser round-trip dev tools are also workspace-only and require explicit input
paths:

```sh
cargo run --features dev-tools --bin lossless-check -- original
cargo run --features dev-tools --bin ast-check -- original
```

## Regenerating the Baseline

`expected/` is a snapshot of this formatter's own output over `original/`.
After a deliberate formatter change, regenerate it, review the diff, commit:

```sh
tools/gen-expected.sh
```

## Repo layout

- `src/lib.rs` — `format_source` (entry) + the token-gated whitespace/colon
  normalization. `src/layout_ast.rs` — the AST-driven layout backend.
- `src/bin/daml-fmt.rs` — the CLI (the only published binary). Dev-only bins
  behind the `dev-tools` feature: `coverage` (metric), `lossless-check` /
  `ast-check` (parser round-trip checks). The dev-tool bins require explicit
  input paths because their default corpus is not included in the package.
- `original/` — 924 corpus files from digital-asset/daml (Apache-2.0 upstream; some example files carry no per-file SPDX header), all
  verified to desugar clean. The formatting test cases.
- `expected/` — the formatter's output over the corpus, the regression
  baseline (regenerate with `tools/gen-expected.sh`).
- `corpus/` — manifests and `SCOREBOARD.md` (the target board).
- `tools/` — `verify-rust.sh` (desugar subset + idempotence by default; full
  desugar with `--desugar`), `gen-expected.sh`.
- `test/diff.js` — the differential harness (`npm test`).

`CLAUDE.md` holds the full project plan, verification commands, and the
formatter rules (the desugar oracle outranks everything; comments are sacred;
never dedent an indented line to column 0).

## License

AGPL-3.0-only. See [LICENSE](LICENSE). The vendored `original/` corpus keeps its
own upstream Apache-2.0 license (from digital-asset/daml).
