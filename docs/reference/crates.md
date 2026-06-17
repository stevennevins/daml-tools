# Crate reference

The workspace contains three independently versioned crates. Workspace
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
| [`daml-parser`](../../crates/daml-parser) | `0.2.0` | library | Lossless lexer, layout resolver, and parser for the Daml smart-contract language. |
| [`daml-lint`](../../crates/daml-lint) | `0.2.0` | library and CLI | Static analysis scanner for Daml smart contracts. |
| [`daml-fmt`](../../crates/daml-fmt) | `0.2.0` | library and CLI | Canonical code formatter for the Daml smart-contract language, built on `daml-parser`. |

## `daml-parser`

Manifest: [`crates/daml-parser/Cargo.toml`](../../crates/daml-parser/Cargo.toml)

Library root: [`crates/daml-parser/src/lib.rs`](../../crates/daml-parser/src/lib.rs)

README: [`crates/daml-parser/README.md`](../../crates/daml-parser/README.md)

`daml-parser` has no external dependencies. It is the shared foundation used by
`daml-lint` and `daml-fmt`.

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

## `daml-lint`

Manifest: [`crates/daml-lint/Cargo.toml`](../../crates/daml-lint/Cargo.toml)

Library root: [`crates/daml-lint/src/lib.rs`](../../crates/daml-lint/src/lib.rs)

README: [`crates/daml-lint/README.md`](../../crates/daml-lint/README.md)

`daml-lint` depends on `daml-parser`, `serde`, and `serde_json`. `clap` and
`rquickjs` are optional dependencies controlled by features.

### Features

| Feature | Default | Enables |
|---------|---------|---------|
| `cli` | Yes | The clap-based `daml-lint` binary dependency. |
| `custom-rules` | Yes | JavaScript AST rules through the QuickJS runtime. |
| `default` | Yes | `cli` and `custom-rules`. |

The `daml-lint` binary requires both `cli` and `custom-rules`. With
`default-features = false`, the crate provides the library, built-in detectors,
and rule-facing IR without pulling in clap or QuickJS.

### Public modules

| Module | Description |
|--------|-------------|
| `detector` | Detector trait, `Finding`, `Severity`, `DetectError`, and severity parsing. |
| `detectors` | Built-in detector modules and `create_builtin_detectors`. |
| `detectors::script` | JavaScript custom-rule detector support when `custom-rules` is enabled. |
| `ir` | Rule-facing Daml intermediate representation. |
| `parser` | Lowering from `daml-parser` AST to the linter IR, including parse diagnostics. |
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

`daml-fmt` depends on `daml-parser` only. It does not depend on `daml-lint`.

### Features

| Feature | Default | Enables |
|---------|---------|---------|
| `dev-tools` | No | Workspace-only corpus harness binaries. |

### Public library API

| Item | Visibility | Description |
|------|------------|-------------|
| `format_source(src: &str) -> String` | Public | Formats Daml source with the AST-driven formatter. |
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

All three crates are pre-1.0. The current manifest version is `0.2.0` for each
crate. Public API breaking changes use 0.x minor releases; patch releases
should remain compatible.
