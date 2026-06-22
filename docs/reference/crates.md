# Crate reference

The workspace contains four independently versioned crates. Workspace
membership is declared in [`Cargo.toml`](../../Cargo.toml).

## Workspace metadata

| Field | Value |
|-------|-------|
| Edition | `2021` |
| Rust version | `1.87.0` |
| License | `AGPL-3.0-only` |
| Repository | `https://github.com/stevennevins/daml-tools` |

## Workspace members

| Crate | Version | Kind | Package description |
|-------|---------|------|---------------------|
| [`daml-parser`](../../crates/daml-parser) | `0.2.3` | library | Lossless lexer, layout resolver, and parser for the Daml smart-contract language. |
| [`daml-syntax`](../../crates/daml-syntax) | `0.1.0` | library | Shared parsed-source surface for Daml tools. |
| [`daml-lint`](../../crates/daml-lint) | `0.3.11` | library and CLI | Static analysis scanner for Daml smart contracts. |
| [`daml-fmt`](../../crates/daml-fmt) | `0.2.8` | library and CLI | Canonical code formatter for the Daml smart-contract language, built on shared syntax. |

## `daml-parser`

Manifest: [`crates/daml-parser/Cargo.toml`](../../crates/daml-parser/Cargo.toml)

Library root: [`crates/daml-parser/src/lib.rs`](../../crates/daml-parser/src/lib.rs)

README: [`crates/daml-parser/README.md`](../../crates/daml-parser/README.md)

`daml-parser` has no external dependencies. It is the low-level foundation used
by `daml-syntax`.

### Public modules

| Module | Description |
|--------|-------------|
| `ast` | Syntax tree types with byte spans on parsed nodes. |
| `ast_span` | AST byte-span reconstruction, including `render_from_ast`. |
| `layout` | Haskell-style offside-rule layout resolution into virtual layout tokens. |
| `lexer` | Tokenization, trivia, positions, lexical diagnostics, and lossless rendering. |
| `parse` | Parser entry points, including `parse_module`, returning a module and diagnostics. |

The normal construction path is parser-created AST values. The AST modules are
public for inspection by tools.

## `daml-syntax`

Manifest: [`crates/daml-syntax/Cargo.toml`](../../crates/daml-syntax/Cargo.toml)

Library root: [`crates/daml-syntax/src/lib.rs`](../../crates/daml-syntax/src/lib.rs)

README: [`crates/daml-syntax/README.md`](../../crates/daml-syntax/README.md)

`daml-syntax` depends on `daml-parser` and `text-size`. It does not depend on
`daml-lint`, `daml-fmt`, `serde`, `serde_json`, `clap`, or `rquickjs`.

### Public API

| Item | Description |
|------|-------------|
| `SourceFile` | Parsed source plus diagnostics, line index, tokens, trivia, laid-out tokens, and parser-span conversion. |
| `SourceTokens` | Tokenized source for callers that need tokens, trivia, lex errors, or laid-out tokens without a full parse. |
| `LineIndex` | Byte, line/column, and UTF-16 offset mapping over one source string. |
| `Diagnostic` | Parser diagnostic with source range, line/column, message, and category. |
| `LineCol` | 1-based line and column pair. |
| `TextRange`, `TextSize` | Re-exported `text-size` range and offset types used by public range APIs. |

## `daml-lint`

Manifest: [`crates/daml-lint/Cargo.toml`](../../crates/daml-lint/Cargo.toml)

Library root: [`crates/daml-lint/src/lib.rs`](../../crates/daml-lint/src/lib.rs)

README: [`crates/daml-lint/README.md`](../../crates/daml-lint/README.md)

`daml-lint` depends on `daml-parser`, `daml-syntax`, `serde`, and
`serde_json`. `clap` and `rquickjs` are optional dependencies controlled by
features.

### Features

| Feature | Default | Enables |
|---------|---------|---------|
| `cli` | Yes | The clap-based `daml-lint` binary dependency. |
| `js-runtime` | Yes | QuickJS-backed rule runtime for shipped built-ins. |
| `custom-rules` | Yes | User-provided JavaScript AST rule loading through `--rules` and configured plugin packages when `js-runtime` is enabled. |
| `default` | Yes | `cli`, `js-runtime`, and `custom-rules`. |

The `daml-lint` binary requires both `cli` and `js-runtime`. The
`custom-rules` feature enables the external rule loading surface; it does not
enable QuickJS by itself. Shipped built-ins are authored in TypeScript and
embedded as generated JavaScript; no TypeScript toolchain is required at
runtime. With `default-features = false`, the crate provides parser lowering
and the rule-facing IR without pulling in clap or QuickJS.

### Public modules

| Module | Description |
|--------|-------------|
| `detector` | Detector trait, `Finding`, `Severity`, `DetectError`, and severity parsing. |
| `detectors` | Built-in detector registration through `create_builtin_detectors` when `js-runtime` is enabled. |
| `detectors::script` | JavaScript rule runtime support when `js-runtime` is enabled; file loading is available with `custom-rules`. |
| `ir` | Rule-facing Daml intermediate representation. |
| `parser` | Lowering from `daml-syntax` parsed source to the linter IR, including parse diagnostics. |
| `reporter` | Markdown, JSON, and SARIF report formatting plus exit-code support. |

### Built-in detectors

| Detector | Severity |
|----------|----------|
| `missing-ensure-decimal` | High |
| `unguarded-division` | High |
| `missing-positive-amount` | High |
| `archive-before-execute` | High |
| `head-of-list-query` | Medium |
| `unbounded-fields` | Medium |

## `daml-fmt`

Manifest: [`crates/daml-fmt/Cargo.toml`](../../crates/daml-fmt/Cargo.toml)

Library root: [`crates/daml-fmt/src/lib.rs`](../../crates/daml-fmt/src/lib.rs)

README: [`crates/daml-fmt/README.md`](../../crates/daml-fmt/README.md)

`daml-fmt` depends on `daml-parser` and `daml-syntax`. It does not depend on
`daml-lint`.

### Features

| Feature | Default | Enables |
|---------|---------|---------|
| `dev-tools` | No | Workspace-only corpus harness binaries. |

### Public library API

| Item | Visibility | Description |
|------|------------|-------------|
| `format_source(src: &str) -> String` | Public | Formats Daml source with the AST-driven formatter. |
| `format_source_with_options(src: &str, options: FormatOptions) -> String` | Public | Formats Daml source with explicit formatter options. |
| `FormatOptions` | Public | Controls formatter behavior. `organize_imports` defaults to `true`; disabling it preserves source import order. |
| `lex_diagnostics(src: &str) -> Vec<String>` | Public | Returns lexer diagnostic strings for malformed source. |
| `coverage(src: &str) -> (usize, usize)` | Public | Counts formatter structural edit candidates over modeled constructs. |

The formatter backend is implemented in the private `layout_ast` module.

### Binaries

| Binary | Feature gate | Package role |
|--------|--------------|--------------|
| `daml-fmt` | none | Published formatter CLI. |
| `lossless-check` | `dev-tools` | Verifies lexer lossless reconstruction over explicit file or directory inputs. |
| `ast-check` | `dev-tools` | Verifies AST-span reconstruction over explicit file or directory inputs. |
| `coverage` | `dev-tools` | Reports structural edit candidates over modeled formatter constructs. |

The `dev-tools` binaries are not installed by `cargo install daml-fmt` unless
the `dev-tools` feature is explicitly enabled.

## Versioning and API stability

All four crates are pre-1.0. Public API breaking changes use 0.x minor
releases; patch releases should remain compatible.
