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
everything else through verbatim. Pure reindentation remains gated on the
laid-out token stream; layout-organizing rules are checked by the compiler
desugar oracle.

Results over the 924-file corpus:

| tier | result |
|---|---|
| desugar-equivalent, import-order normalized | 924 / 924 |
| parses | 924 / 924 |
| semantics silently changed | 0 |
| idempotent (`format(format(x)) == format(x)`) | 924 / 924 |

What it lays out today: module/import continuations, `do`-block indentation,
`if`/`then`/`else` clauses, `case` alternatives, `let … in` bindings,
constructor `with` fields, record-update fields, template/interface bodies,
choice internals, declaration ladders, class/instance body-column alignment,
function guards/where bindings, `try`/`catch` handlers, explicit tuple/list
continuations, long applications, infix chains, lambdas, inline `if`/`case`/
`let`/record construction forms, trailing-whitespace + blank-line/final-newline
normalization, import organization, and type-annotation colon spacing
(`x : T` → `x: T`). It makes its own consistent layout decisions and does not
aim to match any other formatter's output.

Import organization is enabled by default. Reordering import declarations can
change Daml package identity even when the formatted source denotes the same
imports; use `--preserve-import-order` or set `daml-tools.fmt.import-order` to
`preserve` when package identity stability matters.

## Documentation

The workspace documentation is published at
[https://stevennevins.github.io/daml-tools/](https://stevennevins.github.io/daml-tools/):

- [Format Daml source](https://stevennevins.github.io/daml-tools/how-to/format-daml.html) for CLI usage patterns
- [Verify a formatter change](https://stevennevins.github.io/daml-tools/how-to/verify-formatter-change.html) for
  corpus, baseline, and audit commands
- [CLI reference](https://stevennevins.github.io/daml-tools/reference/cli.html) for options and exit codes
- [Crate reference](https://stevennevins.github.io/daml-tools/reference/crates.html) for features, binaries, and
  public API
- [Formatter verification model](https://stevennevins.github.io/daml-tools/explanation/formatter-verification.html)
  for the token/desugar/idempotence safety story

Rust API documentation: [docs.rs/daml-fmt](https://docs.rs/daml-fmt)

## Build & install

daml-fmt depends only on the [`daml-parser`](https://crates.io/crates/daml-parser) and
[`daml-syntax`](https://crates.io/crates/daml-syntax) crates (the shared lexer, layout,
and source-map pipeline), never on `daml-lint`. Both live in the
[daml-tools](https://github.com/stevennevins/daml-tools) workspace, so a normal
workspace checkout has everything it needs.

For JavaScript/TypeScript projects that want `daml-fmt` as a dev dependency:

```sh
npm install --save-dev @daml-tools/daml-fmt
npx daml-fmt --check ./daml
```

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
daml-fmt Foo.daml                                  # formatted source to stdout
find src -name '*.daml' -exec daml-fmt -w {} +     # rewrite files in place
find src -name '*.daml' -exec daml-fmt --check {} + # list unformatted files
daml-fmt --preserve-import-order Foo.daml          # format without import sorting
find src -name '*.daml' -exec daml-fmt --ignore-path .damlfmtignore --check {} + # skip generated/vendored files
daml-fmt --rule imports Foo.daml                   # run only import organization
daml-fmt --rule spacing Foo.daml                   # run only whitespace/colon spacing
cat Foo.daml | daml-fmt                            # stdin -> stdout
```

Exit codes: 0 ok, 1 `--check` found unformatted files, 2 error.

Rule ids are `imports`, `layout`, `spacing`, and `syntax-normalization`.
`daml-fmt` reads `./daml.yaml` when present:

```yaml
daml-tools:
  fmt:
    import-order: preserve
    ignore:
      - generated/**
      - vendor.daml
    groups: [all]
    rules:
      imports: off
      layout: on
      spacing: on
      syntax-normalization: on
```

Default config discovery checks exactly `./daml.yaml` in the current working
directory; it does not walk parent directories. CLI `--rule`/`--group`
selection overrides config selection, and `--preserve-import-order` overrides
`import-order`.

`daml-tools.fmt.ignore` entries resolve relative to the config file directory.
Repeatable `--ignore-path <FILE>` entries resolve patterns relative to each
ignore file's directory. Ignore files support blank lines, `#` comments, exact
paths, directory prefixes ending in `/`, leading `/`, `*`, and `**`; this is a
small gitignore-like subset.

## Library API

`daml-fmt` is also a Rust library. The primary entry points are
`format_source` (defaults) and `format_source_with_options`. Use
`try_format_source` / `try_format_source_with_options` when callers need a typed
[`FormatError`] instead of a byte-faithful passthrough on malformed input.
`source_diagnostics` and `lex_diagnostics` return typed [`FormatDiagnostic`] values.

```rust
use daml_fmt::{
    format_source, format_source_with_options, try_format_source, FormatOptions, FormatRule,
    FormatRuleSet, ImportOrder,
};

let formatted = format_source("module M where\nfoo : Int\nfoo = 1\n");

let preserved = format_source_with_options(
    "module M where\nimport DA.List\nimport DA.Optional\n\nx = []\n",
    FormatOptions::new().with_import_order(ImportOrder::Preserve),
);

let imports_only = format_source_with_options(
    "module M where\nimport DA.Optional\nimport DA.List\n\nx : Int\nx = 1\n",
    FormatOptions::new().with_rules(FormatRuleSet::from_rules([FormatRule::Imports])),
);

let checked = try_format_source("module M where\nfoo: Int\nfoo = 1\n").expect("valid source");
assert_eq!(checked, formatted);
assert!(imports_only.contains("import DA.List\nimport DA.Optional"));
```

`ImportOrder` implements `Default` (`Organize`) and `Display` (`organize` /
`preserve`), and is `#[non_exhaustive]` for forward-compatible `match` arms.
`FormatError` exposes its typed diagnostic slice through both `diagnostics()`
and `AsRef<[FormatDiagnostic]>`.
`FormatOptions` uses private fields: construct options with `Default`/`new()` and
`with_*` helpers so new switches can ship with defaults without breaking callers.

See [crate reference](https://stevennevins.github.io/daml-tools/reference/crates.html) for the full public API.

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
byte-identically to the original (`daml damlc desugar`), except import
organization may reorder import declarations and change package identity. For
that rule, the verifier compares desugar output with import lines sorted, so
the body and import set must still match. The default verifier runs that oracle
on a curated subset and keeps full-corpus idempotence:

```sh
tools/verify-rust.sh               # needs Daml SDK 3.4.11 on PATH for desugar
tools/verify-rust.sh --desugar     # full-corpus desugar sweep
```

Review-oriented full-corpus audit packets:

```sh
npm run audit                       # writes target/daml-fmt-audit
```

See [audit workflow](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/docs/audit-workflow.md) for the 25-sample subagent review workflow.

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
- `corpus/` — corpus manifests.
- `tools/` — `verify-rust.sh` (desugar subset + idempotence by default; full
  desugar with `--desugar`), `gen-expected.sh`.
- `test/diff.js` — the differential harness (`npm test`).

`CLAUDE.md` holds the full project plan, verification commands, and the
formatter rules (the desugar oracle outranks everything; comments are sacred;
never dedent an indented line to column 0).

## License

AGPL-3.0-only. See [LICENSE](LICENSE). The vendored `original/` corpus keeps its
own upstream Apache-2.0 license (from digital-asset/daml).
