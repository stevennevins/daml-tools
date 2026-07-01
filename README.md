# daml-tools

[![CI](https://github.com/stevennevins/daml-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/stevennevins/daml-tools/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

> [!WARNING]
> Experimental. Not intended for production use.

A Cargo workspace of pure-Rust tooling for the
[Daml](https://www.digitalasset.com/developers) smart-contract language, built
on one shared, **lossless** parser.

| Crate | Kind | What it is |
|-------|------|------------|
| [`daml-parser`](crates/daml-parser) | library | Lossless lexer + offside layout + parser. The shared foundation. Zero deps. |
| [`daml-syntax`](crates/daml-syntax) | library | Shared parsed-source surface: diagnostics, line mapping, tokens, trivia, and ranges. |
| [`daml-lint`](crates/daml-lint) | lib + CLI | Static analysis scanner — lowers the AST to an IR and runs detectors. |
| [`daml-fmt`](crates/daml-fmt) | lib + CLI | Canonical code formatter, differential-tested against a compiler-verified corpus. |

## Quickstart

```sh
cargo build --workspace
cargo test --workspace

cargo run -p daml-fmt --bin daml-fmt -- --help
cargo run -p daml-lint -- --help
```

For a guided first pass through the tools, start with
[First run](https://stevennevins.github.io/daml-tools/tutorials/first-run.html).
For contributor setup and local verification, follow the
[Bootstrap checklist (fresh checkout)](CONTRIBUTING.md#bootstrap-checklist-fresh-checkout)
in [`CONTRIBUTING.md`](CONTRIBUTING.md).

Maintainer PR signoff (Tier 5) is documented in
[`developer-docs/how-to/local-ci.md`](developer-docs/how-to/local-ci.md).

## The shape

```
daml-parser  ◄── daml-syntax  ◄── daml-lint   (syntax + rules/IR/detectors)
                    ▲
                    └──────────  daml-fmt     (syntax + layout — never depends on daml-lint)
```

Both tools sit on `daml-syntax`, which wraps `daml-parser` with source-facing
diagnostics, line/UTF-16 mapping, token/trivia access, and range conversion.
The formatter deliberately does **not** depend on the linter — it only wants
syntax facts and layout, not the rules engine. That boundary is enforced:

```sh
cargo tree -p daml-fmt | grep daml-lint   # prints nothing
```

The shared tree is **lossless** (keeps every comment and whitespace run as
trivia), so the formatter can re-print layout byte-faithfully while the linter
ignores trivia and reads meaning. One tree, two readers.

## Documentation

The published documentation site is at
[https://stevennevins.github.io/daml-tools/](https://stevennevins.github.io/daml-tools/).
It is organized by consumer need: tutorials for first success, how-to guides for
package usage, reference for technical facts, and explanations for the design
behind the tools. The published site also exposes [`llms.txt`](https://stevennevins.github.io/daml-tools/llms.txt)
and [`llms-full.txt`](https://stevennevins.github.io/daml-tools/llms-full.txt)
for AI assistants and tools that prefer Markdown-first documentation. Repository
setup, local CI, and release runbooks live in [`developer-docs/`](developer-docs/).
Rust API documentation remains on [docs.rs](https://docs.rs/releases/search?query=daml-).

## Install the CLIs

For JavaScript/TypeScript projects that want the tools as dev dependencies:

```sh
npm install --save-dev @daml-tools/daml-lint @daml-tools/daml-fmt
npx daml-lint ./daml
npx daml-fmt --check ./daml
```

The npm packages currently ship native binaries for macOS arm64, Linux
x64/arm64 glibc 2.35 or newer, and Windows x64. Use the Cargo install path on
other platforms, including Intel macOS and Alpine/musl Linux.

Or install from crates.io:

```sh
cargo install daml-lint
cargo install daml-fmt
```

## License

AGPL-3.0-only. See [LICENSE](LICENSE). The vendored corpus keeps its own
upstream license — see [`corpus/daml-finance/README.md`](corpus/daml-finance/README.md).
