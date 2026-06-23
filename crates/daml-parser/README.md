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

## Documentation

The workspace documentation is organized under
[`docs`](https://github.com/stevennevins/daml-tools/blob/main/docs/README.md):

- [Crate reference](https://github.com/stevennevins/daml-tools/blob/main/docs/reference/crates.md) for workspace package facts
- [Workspace architecture](https://github.com/stevennevins/daml-tools/blob/main/docs/explanation/workspace-architecture.md) for
  how `daml-parser`, `daml-lint`, and `daml-fmt` relate
- [CLI reference](https://github.com/stevennevins/daml-tools/blob/main/docs/reference/cli.md) for the tools built on top of
  this parser

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
daml-parser = "0.3"
```

```rust
use daml_parser::parse::parse_module;

let (module, diagnostics) = parse_module("module M where\nfoo : Int\nfoo = 1\n");
assert!(diagnostics.is_empty());
```

## Choosing a parser layer

`parse::parse_module` is the normal entry point when you want a typed syntax
tree. It runs the full pipeline:

1. `lexer::lex`
2. `layout::resolve_layout`
3. recursive-descent parsing into `ast::Module`

Lower layers are public when a consumer needs more control:

| Layer | Use when you need |
|-------|-------------------|
| `lexer::lex_with_trivia` | source tokens plus comments, CPP directives, blank-line trivia, and lexical diagnostics |
| `layout::resolve_layout` | the token stream after Daml's offside rule has inserted virtual layout tokens |
| `parse::parse_module` | a typed `ast::Module` with imports, declarations, expressions, types, byte spans, and parse diagnostics |
| `ast_span::render_from_ast` | an oracle that checks AST spans plus trivia can reconstruct the source bytes |

The parser does not prescribe what downstream crates do with the tree. A
consumer can inspect the AST directly, lower it into its own representation, or
combine the AST with tokens and trivia for source-preserving work.

## Reading parser output

`parse_module` returns `(Module, Vec<ParseDiagnostic>)`. Diagnostics are not
fatal; the parser records what it could understand and keeps going.

```rust
use daml_parser::ast::{Decl, DiagnosticCategory};
use daml_parser::parse::parse_module;

let source = "\
module M where

template Account
  with
    owner : Party
  where
    signatory owner
";

let (module, diagnostics) = parse_module(source);

let has_lex_errors = diagnostics
    .iter()
    .any(|d| d.category == DiagnosticCategory::Lex);

let templates: Vec<_> = module
    .decls
    .iter()
    .filter_map(|decl| match decl {
        Decl::Template(template) => Some((&template.name, template.span)),
        _ => None,
    })
    .collect();

assert!(!has_lex_errors);
assert_eq!(templates[0].0.as_str(), "Account");
```

Every AST node that represents source text carries a 1-based `Pos` and a byte
`Span`. Use spans when you need an exact source slice:

```rust
let snippet = source
    .get(templates[0].1.start..templates[0].1.end)
    .expect("parser spans are UTF-8 boundaries");

assert!(snippet.starts_with("template Account"));
```

## Diagnostics and partial structure

`ParseDiagnostic::category` separates different kinds of recovery:

| Category | Meaning |
|----------|---------|
| `Lex` | the lexer found malformed source such as an unterminated string or block comment |
| `Malformed` | an expression, pattern, or expected token was malformed inside an otherwise recognized construct |
| `SkippedDecl` | a top-level declaration could not be parsed and was skipped to the next item |
| `UnsupportedSyntax` | the source used syntax this parser intentionally does not model yet |
| `RecursionLimit` | deeply nested input exceeded the parser recursion bound and was degraded to raw text |

Partial structure is explicit in the AST. For example:

- `Decl::Unknown` preserves raw top-level declaration text.
- `Expr::Error` preserves raw expression text.
- `Pat::Other` preserves raw pattern text.
- `TemplateBodyDecl::Other` preserves template body items that are not modeled.

Consumers can decide how much diagnostic tolerance is acceptable for their own
use. The parser itself does not treat diagnostics as a process-level failure.

## Source positions, spans, and trivia

`Span` values are byte offsets into the original source: `[start, end)`.
Virtual layout tokens have zero-width spans and are skipped when AST node spans
are computed. Comments and whitespace live in lexer trivia rather than AST
nodes.

Use the lexer-level oracle when you need to prove tokens and trivia still cover
the source:

```rust
use daml_parser::lexer::{lex_with_trivia, render_lossless};

let (tokens, trivia, errors) = lex_with_trivia(source);
assert!(errors.is_empty());
assert_eq!(render_lossless(source, &tokens, &trivia).unwrap(), source);
```

Use the AST-level oracle when you need to prove parsed node spans and trivia can
still reconstruct the source:

```rust
use daml_parser::ast_span::render_from_ast;
use daml_parser::lexer::lex_with_trivia;
use daml_parser::parse::parse_module;

let (_, trivia, _) = lex_with_trivia(source);
let (module, _) = parse_module(source);
assert_eq!(render_from_ast(source, &module, &trivia).unwrap(), source);
```

## Layout-aware tokens

Daml uses indentation-sensitive layout. `layout::resolve_layout` converts the
raw token stream into a laid-out token stream by inserting zero-width virtual
tokens:

| Token | Meaning |
|-------|---------|
| `Tok::VLBrace` | start of an implicit layout block |
| `Tok::VSemi` | separator between items in an implicit layout block |
| `Tok::VRBrace` | end of an implicit layout block |

This is useful when a consumer wants parser-equivalent token boundaries without
building the full AST.

```rust
use daml_parser::layout::resolve_layout;
use daml_parser::lexer::{lex, Tok};

let (tokens, errors) = lex(source);
assert!(errors.is_empty());

let laid_out = resolve_layout(tokens);
assert!(laid_out.iter().any(|t| matches!(t.tok, Tok::VLBrace)));
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

`daml-parser` is pre-1.0. The supported public entry points are the modules
listed above, with `parse::parse_module` as the normal start. The AST is public
so tools can inspect parser output; downstream code should prefer parser-created
trees over manual construction. Breaking public API changes use 0.x minor bumps,
and patch releases should stay compatible. `cargo-semver-checks` is a soft CI
signal until the first crates.io baseline exists.

`daml-parser` is syntax-only. It does not perform name resolution, package
resolution, type checking, scenario/script execution, or authorization analysis.
Consumers that need those concepts should derive them above the syntax layer or
delegate them to the Daml toolchain.

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
