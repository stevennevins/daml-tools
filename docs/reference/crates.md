---
description: Published Rust crates and npm packages in daml-tools, with their intended consumer use cases.
---

# Crate and package reference

The workspace contains four independently versioned Rust crates and npm packages
for CLI distribution and custom lint rules. Workspace membership is declared in
[`Cargo.toml`](https://github.com/stevennevins/daml-tools/blob/main/Cargo.toml).

## Workspace metadata

| Field | Value |
|-------|-------|
| Edition | `2021` |
| Rust version | `1.96.0` |
| License | `AGPL-3.0-only` |
| Repository | `https://github.com/stevennevins/daml-tools` |
| Homepage | `https://github.com/stevennevins/daml-tools` |
| Documentation site | `https://stevennevins.github.io/daml-tools/` |

All workspace members inherit `edition`, `rust-version`, `license`, `repository`,
`homepage`, and `authors` from `[workspace.package]` in the root
[`Cargo.toml`](https://github.com/stevennevins/daml-tools/blob/main/Cargo.toml).
Each crate sets its own `documentation` URL on docs.rs and keeps crate-specific
`description`, `keywords`, `categories`, and `exclude` lists.

### MSRV evidence

The workspace MSRV is `1.96.0`, enforced by the `msrv` job in
[`.github/workflows/ci.yml`](https://github.com/stevennevins/daml-tools/blob/main/.github/workflows/ci.yml).
It is not raised speculatively:

| Constraint | Source | Declared MSRV |
|------------|--------|---------------|
| QuickJS rule runtime | `rquickjs` 0.12 (`daml-lint` optional dep) | `1.87` |
| CLI argument parsing | `clap` 4.6 (`daml-lint` optional dep) | `1.85` |

`daml-parser` has no external Rust dependencies. `daml-fmt` uses the shared
syntax stack plus `serde`/`serde_yaml` for `daml.yaml` config. Both share the
workspace MSRV so `cargo install` and CI stay aligned.

### Published package contents

| Crate | `exclude` highlights | Notes |
|-------|----------------------|-------|
| `daml-parser` | _(none)_ | Ships `LICENSE`, `README.md`, `CHANGELOG.md`, `src/`, integration tests, fixtures, and goldens. |
| `daml-syntax` | _(none)_ | Ships `LICENSE`, `README.md`, `CHANGELOG.md`, `src/`, and integration tests. |
| `daml-lint` | `tests/fixtures/`, `docs/`, `tools/`, `lint-plugin/`, npm metadata, `rules/*.ts` | Keeps `examples/` and compiled `rules/*.js` (embedded via `include_str!`). |
| `daml-fmt` | corpus, differential-test trees, dev scripts | Keeps the published `daml-fmt` binary, integration tests, layout fixtures, and goldens. |

## Workspace members

| Crate | Version | Kind | Package description |
|-------|---------|------|---------------------|
| [`daml-parser`](https://github.com/stevennevins/daml-tools/tree/main/crates/daml-parser) | `0.10.1` | library | Lossless lexer, layout resolver, and parser for the Daml smart-contract language. |
| [`daml-syntax`](https://github.com/stevennevins/daml-tools/tree/main/crates/daml-syntax) | `0.9.1` | library | Shared parsed-source surface for Daml tools. |
| [`daml-lint`](https://github.com/stevennevins/daml-tools/tree/main/crates/daml-lint) | `0.9.4` | library and CLI | Static analysis scanner for Daml smart contracts. |
| [`daml-fmt`](https://github.com/stevennevins/daml-tools/tree/main/crates/daml-fmt) | `0.7.4` | library and CLI | Canonical code formatter for the Daml smart-contract language, built on shared syntax. |

### Per-crate docs.rs URLs

| Crate | Documentation |
|-------|---------------|
| `daml-parser` | [docs.rs/daml-parser](https://docs.rs/daml-parser) |
| `daml-syntax` | [docs.rs/daml-syntax](https://docs.rs/daml-syntax) |
| `daml-lint` | [docs.rs/daml-lint](https://docs.rs/daml-lint) |
| `daml-fmt` | [docs.rs/daml-fmt](https://docs.rs/daml-fmt) |

## npm packages

| Package | Version | Role |
|---------|---------|------|
| [`@daml-tools/daml-lint`](https://www.npmjs.com/package/@daml-tools/daml-lint) | `0.9.4` | Wrapper that installs the platform `daml-lint` binary for Node projects. |
| [`@daml-tools/daml-fmt`](https://www.npmjs.com/package/@daml-tools/daml-fmt) | `0.7.4` | Wrapper that installs the platform `daml-fmt` binary for Node projects. |
| [`@daml-tools/lint-plugin`](https://www.npmjs.com/package/@daml-tools/lint-plugin) | `0.9.4` | TypeScript contract and starter templates for custom lint rule plugins. |

Platform-specific CLI packages (`@daml-tools/daml-lint-linux-x64`, `@daml-tools/daml-fmt-darwin-arm64`, and similar) are generated at publish time from cargo-npm metadata in each crate manifest. They are not edited by hand in the repository.

## `daml-parser`

Manifest: [`crates/daml-parser/Cargo.toml`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-parser/Cargo.toml)

Library root: [`crates/daml-parser/src/lib.rs`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-parser/src/lib.rs)

README: [`crates/daml-parser/README.md`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-parser/README.md)

API docs: [docs.rs/daml-parser](https://docs.rs/daml-parser)

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

`ChoiceDecl.authority_exprs` and braced/layout `where` metadata blocks preserve
source choice `controller`, `observer`, and `authority` clauses; lint IR
`Choice.authority_exprs` exposes them (`ir_version: 5`). `InterfaceInstanceDecl.items`
is a source-ordered `InterfaceInstanceBodyItem` list that distinguishes
`view = ...` from method implementations; lint `InterfaceInstance.view_expr`
is separate from `methods` (`ir_version: 6`). `Alt` case alternatives retain
source-ordered `branches` with boolean/pattern guards and alternative-local
`where_bindings`; lint `CaseAlt` mirrors that shape (`ir_version: 7`).
Module-level `Decl::Fixity` declarations drive in-module expression grouping
(imported fixity is out of scope). `Pat::Record` models brace/`with` record
pattern fields while positional `Pat::Con` patterns stay unchanged. `ImportDecl.package_label`
preserves package-qualified import string literals from source; lint
`Import.package_label` mirrors the decoded label (`ir_version: 8`).

`daml-parser` does not model LF package metadata, `NameMap`s, qualified
`PackageId`s, Update/internal expression nodes, or compiler desugaring.

## `daml-syntax`

Manifest: [`crates/daml-syntax/Cargo.toml`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-syntax/Cargo.toml)

Library root: [`crates/daml-syntax/src/lib.rs`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-syntax/src/lib.rs)

README: [`crates/daml-syntax/README.md`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-syntax/README.md)

API docs: [docs.rs/daml-syntax](https://docs.rs/daml-syntax)

`daml-syntax` depends on `daml-parser` and `text-size`. It does not depend on
`daml-lint`, `daml-fmt`, `serde`, `serde_json`, `clap`, or `rquickjs`.

### Public API

| Item | Description |
|------|-------------|
| `SourceFile` | Parsed source plus diagnostics, line index, tokens, trivia, laid-out tokens, and parser-span conversion; implements clone/equality independent of lazy token-cache state. |
| `SourceTokens` | Tokenized source for callers that need tokens, trivia, lex errors, or laid-out tokens without a full parse; implements clone/equality independent of lazy layout-cache state. |
| `LineIndex` | Byte, line/column, and fallible UTF-16 column mapping over one source string. |
| `Diagnostic` | Parser diagnostic with source range, line/column, named end-column shape, message, and category. Read through accessors; constructed by `SourceFile::parse`. |
| `ByteLineCol`, `CharLineCol` | 1-based line/column pairs that distinguish byte columns from Unicode scalar columns. |
| Coordinate newtypes | `LineNumber`, `ByteColumn`, `CharColumn`, `ByteOffset`, and `Utf16Offset` support standard conversion traits where valid; use `usize::from(coordinate)` for raw extraction. |
| `CoordinateRangeError` | Typed error for line or byte-column coordinates outside a `LineIndex`. |
| `InvalidOneBasedCoordinate` | Typed error returned by `TryFrom<usize>` for zero one-based coordinates. |
| `DiagnosticEndColumn` | Same-line, multi-line, or empty-span end-column shape for diagnostics. |
| `Utf16Range` | Named start/end range in UTF-16 code units for JavaScript-style string offsets. |
| `TextRange`, `TextSize` | Re-exported `text-size` range and offset types used by public range APIs. |

## `daml-lint`

Manifest: [`crates/daml-lint/Cargo.toml`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/Cargo.toml)

Library root: [`crates/daml-lint/src/lib.rs`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/src/lib.rs)

README: [`crates/daml-lint/README.md`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/README.md)

API docs: [docs.rs/daml-lint](https://docs.rs/daml-lint)

`daml-lint` depends on `daml-parser`, `daml-syntax`, `serde`, and
`serde_json`. `clap` and `rquickjs` are optional dependencies controlled by
features.

### Features

| Feature | Default | Enables |
|---------|---------|---------|
| `cli` | Yes | The clap-based `daml-lint` binary dependency. |
| `js-runtime` | Yes | QuickJS-backed rule runtime for shipped built-ins. |
| `custom-rules` | Yes | User-provided JavaScript AST rule loading through `--rules` and configured plugin packages; implies `js-runtime`. |
| `default` | Yes | `cli`, `js-runtime`, and `custom-rules`. |

The `daml-lint` binary requires both `cli` and `js-runtime`. The `cli` feature
implies `js-runtime`. The `custom-rules` feature implies `js-runtime` and
enables the external rule loading surface beyond shipped built-ins. Shipped
built-ins are authored in TypeScript and
embedded as generated JavaScript; no TypeScript toolchain is required at
runtime. With `default-features = false`, the crate provides parser lowering
and the rule-facing IR without pulling in clap or QuickJS.

Rule-facing IR fields for domain enums are strings (for example, choice
`consuming` and import `qualified`) to make custom-rule contracts explicit and
stable.

### Public modules

| Module | Description |
|--------|-------------|
| `detector` | Detector trait, `Finding`, `Severity`, `DetectError`, and severity parsing. |
| `detectors` | Built-in detector registration through `create_builtin_detectors` when `js-runtime` is enabled. |
| `detectors::script` | JavaScript rule runtime support when `js-runtime` is enabled; file loading is available with `custom-rules`. |
| `ir` | Rule-facing Daml intermediate representation. |
| `parser` | Lowering from `daml-syntax` parsed source to the linter IR. Key types: `parse_daml_with_diagnostics`, `ParseResult` (`module`, `diagnostics`), `ParseDiagnostic`, and stable `ParseDiagnosticCategory` tags (`lexical-error`, `malformed`, `unsupported-syntax`, …). |
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

## `@daml-tools/lint-plugin`

Package manifest: [`crates/daml-lint/lint-plugin/package.json`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/lint-plugin/package.json)

README: [`crates/daml-lint/lint-plugin/README.md`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/lint-plugin/README.md)

The npm package publishes TypeScript types for the rule-facing IR,
`DamlLintRuleModule`, global `report`, and global `CONFIG`. It does not ship
runtime helpers. See [Custom rule contract](./daml-lint-custom-rule-contract.md)
for the JavaScript runtime contract.

## `daml-fmt`

Manifest: [`crates/daml-fmt/Cargo.toml`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/Cargo.toml)

Library root: [`crates/daml-fmt/src/lib.rs`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/src/lib.rs)

README: [`crates/daml-fmt/README.md`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/README.md)

API docs: [docs.rs/daml-fmt](https://docs.rs/daml-fmt)

`daml-fmt` depends on `daml-parser`, `daml-syntax`, `serde`, and `serde_yaml`.
It does not depend on `daml-lint`.

### Features

| Feature | Default | Enables |
|---------|---------|---------|
| `dev-tools` | No | Workspace-only corpus harness binaries. |

### Public library API

| Item | Visibility | Description |
|------|------------|-------------|
| `format_source(src: &str) -> String` | Public | Formats Daml source with the AST-driven formatter. |
| `format_source_with_options(src: &str, options: FormatOptions) -> String` | Public | Formats Daml source with explicit formatter options. |
| `try_format_source(src: &str) -> Result<String, FormatError>` | Public | Formats Daml source with default options, rejecting diagnostics reported by `source_diagnostics`. |
| `try_format_source_with_options(src: &str, options: FormatOptions) -> Result<String, FormatError>` | Public | Formats Daml source with explicit options, rejecting diagnostics reported by `source_diagnostics`. |
| `FormatOptions` | Public | Formatter switches. Prefer `Default`/`new()`/`with_*` for forward-compatible construction. |
| `FormatRule` | Public | Discrete formatter rule ids: `imports`, `layout`, `spacing`, and `syntax-normalization`. Implements string parsing/display for CLI/config selection. |
| `FormatRuleSet` | Public | Set of enabled formatter rules with `all()`, `none()`, `from_rules`, `contains`, `insert`, and `remove` helpers. |
| `ImportOrder` | Public | Import ordering strategy (`Organize` default, `Preserve` via CLI `--preserve-import-order`). Implements `Default` and `Display`; `#[non_exhaustive]`. |
| `FormatDiagnostic` | Public | Typed formatter diagnostic. Access line, column, category, and message through accessors. |
| `FormatError` | Public | Formatting or coverage rejection error. Implements `Display`, `std::error::Error`, and `AsRef<[FormatDiagnostic]>`; access typed diagnostics through `diagnostics()`. |
| `lex_diagnostics(src: &str) -> Vec<FormatDiagnostic>` | Public | Returns typed lexer diagnostics for malformed source. |
| `source_diagnostics(src: &str) -> Vec<FormatDiagnostic>` | Public | Returns typed lexer and parser diagnostics for malformed source. |
| `FormatCoverage` | Public | Structural edit-candidate and modeled-construct counts from `coverage`. Read through `edit_candidates()` and `modeled_constructs()`. |
| `coverage(src: &str) -> Result<FormatCoverage, FormatError>` | Public | Counts formatter structural edit candidates over modeled constructs, rejecting diagnostics reported by `source_diagnostics`. |
| `config` module | Public | YAML config loader and CLI rule/group resolution helpers for `daml-tools.fmt`. |

The formatter backend is implemented in the private `layout_ast` module.

`ImportOrder::default()` is `Organize`; `ImportOrder` displays as stable
lowercase labels (`organize` or `preserve`) and is `#[non_exhaustive]` so new
strategies can be added without breaking downstream `match` arms.
`FormatOptions` keeps fields private and adds new switches through
`Default`/`new()` plus `with_*` helpers.

The CLI can read `daml-tools.fmt.import-order` from `./daml.yaml` and can skip
file arguments with `daml-tools.fmt.ignore` or repeatable
`--ignore-path <FILE>` ignore files. Formatter config discovery is cwd-only: it
checks exactly `./daml.yaml` unless `--config <FILE>` is supplied.

`source_diagnostics` intentionally suppresses skipped-declaration parser recovery
and alternate `module` declarations inside inactive CPP branches because those
branches are not preprocessed before parsing. Active malformed parser
diagnostics and lexical diagnostics are still returned and still cause
`try_format_source*` and `coverage` to fail.

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
