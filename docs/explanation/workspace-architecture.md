---
description: Why daml-tools separates parser, syntax, linter, and formatter crates around shared source invariants.
---

# Workspace architecture

Published documentation:
[https://stevennevins.github.io/daml-tools/](https://stevennevins.github.io/daml-tools/)

`daml-tools` is organized around one zero-dependency parser, one shared
source-facing syntax surface, and two independent tools built on top of them.

The central crate is `daml-parser`: a zero-dependency, pure-Rust lexer, layout
resolver, and parser for Daml. `daml-syntax` depends on it and owns the
source-facing facts shared by tools: parse diagnostics, line and UTF-16
mapping, token/trivia access, laid-out tokens, and conversion from parser byte
spans to `TextRange`.

Both `daml-lint` and `daml-fmt` depend on `daml-syntax`, but they use it for
different purposes. The linter reads meaning from the parsed tree and maps
types to its rule-facing IR. The formatter uses the same parsed source, plus
preserved trivia, to rewrite layout without losing source text.

This shape keeps the workspace small but deliberately asymmetric:

```text
daml-parser -> daml-syntax -> daml-lint
                           -> daml-fmt
```

There is no dependency from `daml-fmt` to `daml-lint`.

## Why the parser is shared

Daml has layout-sensitive syntax, so tools cannot safely reason about source
text by splitting lines or scanning tokens in isolation. The parser pipeline
owns that complexity once:

1. Lex source text into tokens and trivia.
2. Resolve Daml's offside layout into virtual layout tokens.
3. Parse the laid-out stream into a typed AST with byte spans.

Keeping that pipeline in `daml-parser` gives every higher-level tool the same
view of Daml syntax. A lint detector and a formatter rule may care about
different things, but they should not disagree about where modules, templates,
choices, expressions, comments, or layout boundaries are.

The parser is also intentionally dependency-free. That matters because it is
the lowest layer in the workspace. If the parser pulled in CLI, lint,
formatting, serialization, or JavaScript runtime dependencies, every consumer
would inherit them even when it only wanted a syntax tree.

`daml-syntax` is the narrow layer above the parser. It is allowed to depend on
`text-size` because its public API speaks in `TextRange` and `TextSize`, but it
does not own lint policy, formatter layout, CLI behavior, serialization, or a
JavaScript runtime.

## Why losslessness matters

`daml-parser` is lossless: comments and whitespace are recorded as trivia, and
AST nodes retain byte spans into the original source. This lets consumers
reconstruct the original bytes from parser output.

Losslessness is not just a formatter convenience. It is what allows one syntax
tree to serve two different readers:

- `daml-lint` can ignore trivia and focus on semantic patterns.
- `daml-fmt` can preserve comments, whitespace-sensitive regions, and unmodeled
  constructs while changing only the layout it understands.

Without losslessness, the formatter would have to choose between dropping
source details or building a separate parser-like model. Both outcomes would
weaken the workspace: comments could be lost, edge cases could diverge, and
fixes in parser behavior would not automatically benefit both tools.

## Why the formatter does not depend on the linter

`daml-fmt` must depend on `daml-syntax` and `daml-parser` only. The formatter
needs syntax and source preservation; it does not need lint rules, lint IR,
detector policy, reporting, CLI behavior, or custom-rule runtime support.

That boundary keeps formatting independent from lint policy. A formatter should
be able to format any syntactically valid Daml file without asking whether the
file follows the repository's static-analysis rules. Likewise, lint rules
should be free to evolve without changing the formatter's dependency graph or
release surface.

This separation also protects the published formatter. `daml-lint` includes
optional CLI and custom JavaScript rule machinery; pulling that into `daml-fmt`
would make the formatter heavier for no formatting benefit.

## Independent tools, shared invariants

The workspace does not treat the parser as a private implementation detail of
either tool. It is the common contract.

Parser invariants, such as tokenization, offside layout resolution, byte spans,
lossless reconstruction, diagnostics, and source range mapping sit below both
tools. Tool-specific behavior sits above that line:

- `daml-lint` lowers syntax output into lint-oriented IR and detectors.
- `daml-fmt` walks syntax output, applies modeled layout decisions, and passes
  unmodeled syntax through safely.

This structure keeps the repo extensible without adding speculative
abstraction. New Daml tooling should start by asking whether it needs parser
internals, source-facing syntax, policy, or layout. Parser internals belong in
`daml-parser`; shared source-facing syntax belongs in `daml-syntax`; policy
belongs in a tool such as `daml-lint`; layout belongs in `daml-fmt`.

## Parser scope and non-goals

`daml-parser` is source-oriented: it models Daml syntax, byte spans, and
tolerant recovery — not the LF AST or compiler internals. Downstream crates
lower or walk that tree for their own needs (`daml-lint` for rule IR,
`daml-fmt` for layout). The parser deliberately does **not** introduce LF
package metadata, `NameMap`s, qualified `PackageId` resolution, Update/internal
expression nodes, imported fixity resolution, or compiler desugaring. When a
feature is LF-only or requires name/type semantics, it belongs above the parser
layer or in the official Daml toolchain.
