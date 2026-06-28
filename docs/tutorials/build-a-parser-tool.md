# Build a tiny Daml mutation-testing tool with daml-parser

This tutorial builds a minimal Rust binary that uses [`daml-parser`](https://docs.rs/daml-parser)
to parse Daml source, apply one byte-level mutation at a template span, and
re-parse to see whether diagnostics change. Full mutation testing — mutant
generation, coverage, equivalent-mutant detection, and test orchestration — is
out of scope.

## Prerequisites

You need Rust 1.96 or newer and a shell.

## Create the project

```sh
cargo new daml-mutate-demo
cd daml-mutate-demo
```

Add `daml-parser` from crates.io:

```toml
[dependencies]
daml-parser = "0.10.1"
```

Create a sample module `sample.daml`:

```daml
module Demo where

template Account
  with
    owner : Party
  where
    signatory owner
```

## Write `src/main.rs`

Replace the generated `src/main.rs` with the complete program below. It reads a
Daml file path from the command line, parses with
[`parse::parse_module`](https://docs.rs/daml-parser/latest/daml_parser/parse/fn.parse_module.html),
collects template spans from the AST, applies one byte-level mutation, and
re-parses to compare diagnostics.

```rust
use std::env;
use std::process;

use daml_parser::ast::{Decl, Span};
use daml_parser::parse::parse_module;

fn main() {
    let path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("usage: daml-mutate-demo <file.daml>");
            process::exit(1);
        }
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("failed to read {path}: {err}");
            process::exit(1);
        }
    };

    let baseline = parse_module(&source);
    println!(
        "baseline diagnostics: {}",
        baseline.diagnostics.len()
    );

    let spans = template_spans(&baseline.module);
    let Some(span) = spans.first().copied() else {
        println!("no template declarations found");
        process::exit(0);
    };

    let Some(mutated_source) = flip_first_alpha(&source, span) else {
        println!("no mutation applied");
        process::exit(0);
    };

    let after = parse_module(&mutated_source);
    println!(
        "after mutation diagnostics: {}",
        after.diagnostics.len()
    );

    for diagnostic in after.diagnostics {
        println!("{}", diagnostic.message());
    }
}

fn template_spans(module: &daml_parser::ast::Module) -> Vec<Span> {
    module
        .decls
        .iter()
        .filter_map(|decl| match decl {
            Decl::Template(template) => Some(template.span),
            _ => None,
        })
        .collect()
}

fn flip_first_alpha(source: &str, span: Span) -> Option<String> {
    let start = span.start_usize();
    let end = span.end_usize();
    let slice = source.get(start..end)?;
    for (offset, ch) in slice.char_indices() {
        if ch.is_ascii_alphabetic() {
            let idx = start + offset;
            let flipped = if ch.is_ascii_uppercase() {
                ch.to_ascii_lowercase()
            } else {
                ch.to_ascii_uppercase()
            };
            let mut mutated = source.to_string();
            mutated.replace_range(idx..idx + ch.len_utf8(), &flipped.to_string());
            return Some(mutated);
        }
    }
    None
}
```

## How the pieces fit together

### Tolerant vs strict parsing

[`parse_module`](https://docs.rs/daml-parser/latest/daml_parser/parse/fn.parse_module.html)
always returns a [`ParseModuleResult`](https://docs.rs/daml-parser/latest/daml_parser/parse/struct.ParseModuleResult.html)
with a module tree and diagnostics. For fail-fast callers,
[`parse_module_strict`](https://docs.rs/daml-parser/latest/daml_parser/parse/fn.parse_module_strict.html)
and [`ParseModuleResult::into_result`](https://docs.rs/daml-parser/latest/daml_parser/parse/struct.ParseModuleResult.html)
wrap the same tolerant parse and return `Err` when diagnostics are non-empty.

### Template spans

Walk top-level declarations and collect template spans. Templates carry a byte
[`Span`](https://docs.rs/daml-parser/latest/daml_parser/ast/struct.Span.html)
covering the full `template … where …` item. Use `Span::get(source)` when you
need the exact UTF-8 slice for a node.

### Source-oriented mutations

The toy mutation flips the first alphabetic character inside a template span —
for example `template` becomes `Template`, which produces a new parse
diagnostic. Keep mutations source-oriented: `daml-parser` models syntax and byte
spans, not LF package metadata or compiler desugaring.

Inspect [`ParseDiagnostic::category`](https://docs.rs/daml-parser/latest/daml_parser/ast/struct.ParseDiagnostic.html)
for stable grouping (`Lex`, `Malformed`, `SkippedDecl`, and so on). A real
mutation-testing tool would track which mutants survive your test suite; here
we only demonstrate parser-backed mutation and diagnostic delta.

## Optional losslessness check

When you mutate inside a span that should remain structurally valid, you can
prove the baseline tree still covers the source bytes with
[`ast_span::render_from_ast`](https://docs.rs/daml-parser/latest/daml_parser/ast_span/fn.render_from_ast.html)
and lexer trivia from [`lexer::lex_with_trivia`](https://docs.rs/daml-parser/latest/daml_parser/lexer/fn.lex_with_trivia.html).
That oracle is useful when building mutants that preserve comments and layout.

## Run the demo

```sh
cargo run -- sample.daml
```

Expect output similar to:

```text
baseline diagnostics: 0
after mutation diagnostics: 1
unrecognized declaration: Some(UpperId { qualifier: None, name: Identifier("Template") })
```

The baseline parse succeeds on the sample file and the flipped-name mutation
produces at least one new diagnostic.

## Next steps

- Read the [`daml-parser` crate README](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-parser/README.md) for lexer, layout, and AST module boundaries.
- Use [`daml-syntax`](https://docs.rs/daml-syntax) when you need line indexes and UTF-16 ranges on top of parser output.
- Use [`daml-lint`](https://docs.rs/daml-lint) for rule-facing IR and static analysis instead of hand-rolling detectors.
