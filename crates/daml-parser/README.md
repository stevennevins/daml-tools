# daml-parser

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

A **lossless** lexer, layout resolver, and parser for the
[Daml](https://www.digitalasset.com/developers) smart-contract language, in
pure Rust with **zero external dependencies**.

Part of the [daml-tools](https://github.com/stevennevins/daml-tools) workspace.
This is the shared foundation under both
[`daml-lint`](https://crates.io/crates/daml-lint) (a static analyzer) and
[`daml-fmt`](https://crates.io/crates/daml-fmt) (a code formatter). The pipeline is:

```
lexer  →  layout  →  parse
(tokens   (Haskell    (typed AST with a
 + trivia) offside     byte Span on every node)
           rule)
```

## Lossless by design

The lexer records **every** comment and whitespace run as *trivia*
(`lexer::lex_with_trivia`), so the original bytes can be reconstructed exactly
(`lexer::render_lossless`, `ast_span::render_from_ast`). One tree serves two
very different readers:

- the **linter** ignores trivia and reads meaning;
- the **formatter** keeps trivia and re-prints layout — if it dropped a comment
  it would eat your code, so losslessness is not optional.

This is the same shape the grown-up tools use (rust-analyzer, ruff, biome): one
parser, one lossless tree, many consumers.

## Usage

```toml
[dependencies]
daml-parser = "0.1"
```

```rust
use daml_parser::parse::parse_module;

let (module, diagnostics) = parse_module("module M where\nfoo : Int\nfoo = 1\n");
assert!(diagnostics.is_empty());
```

## Public modules

| Module     | What it gives you                                              |
|------------|---------------------------------------------------------------|
| `lexer`    | tokenizer, trivia, `Tok`/`Token`, `Pos`, lossless reconstruction |
| `layout`   | offside-rule resolution into virtual `{ ; }` tokens           |
| `parse`    | `parse_module` → typed AST + diagnostics                      |
| `ast`      | the syntax tree types, byte `Span` on every node              |
| `ast_span` | `render_from_ast`, the byte-span losslessness oracle          |

## Stability

`daml-parser` is a real library other crates depend on, so its public API
follows SemVer strictly — a breaking change to the public types is a major
bump, guarded in CI by `cargo-semver-checks`.

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
