# Workspace architecture

`daml-tools` is organized around one shared syntax foundation and two
independent tools built on top of it.

The central crate is `daml-parser`: a zero-dependency, pure-Rust lexer, layout
resolver, and parser for Daml. Both `daml-lint` and `daml-fmt` depend on it,
but they use it for different purposes. The linter reads meaning from the
parsed tree. The formatter uses the same tree, plus preserved source trivia, to
rewrite layout without losing source text.

This shape keeps the workspace small but deliberately asymmetric:

```text
daml-parser -> daml-lint
daml-parser -> daml-fmt
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

`daml-fmt` must depend on `daml-parser` only. The formatter needs syntax and
source preservation; it does not need lint rules, lint IR, detector policy,
reporting, CLI behavior, or custom-rule runtime support.

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
and lossless reconstruction, sit below both tools. Tool-specific behavior sits
above that line:

- `daml-lint` lowers parser output into lint-oriented IR and detectors.
- `daml-fmt` walks parser output, applies modeled layout decisions, and passes
  unmodeled syntax through safely.

This structure keeps the repo extensible without adding speculative
abstraction. New Daml tooling should start by asking whether it needs syntax,
policy, or layout. Syntax belongs in `daml-parser`; policy belongs in a tool
such as `daml-lint`; layout belongs in `daml-fmt`.
