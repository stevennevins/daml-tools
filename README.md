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
| [`daml-lint`](crates/daml-lint) | lib + CLI | Static analysis scanner — lowers the AST to an IR and runs detectors. |
| [`daml-fmt`](crates/daml-fmt) | lib + CLI | Canonical code formatter, differential-tested against a compiler-verified corpus. |

## The shape

```
daml-parser  ◄── daml-lint   (parser + rules/IR/detectors)
     ▲
     └──────────  daml-fmt    (parser ONLY — never depends on daml-lint)
```

Both tools sit on `daml-parser`. The formatter deliberately does **not** depend
on the linter — it only wants the parse tree, not the rules engine. That
boundary is enforced:

```sh
cargo tree -p daml-fmt | grep daml-lint   # prints nothing
```

The shared tree is **lossless** (keeps every comment and whitespace run as
trivia), so the formatter can re-print layout byte-faithfully while the linter
ignores trivia and reads meaning. One tree, two readers.

## Build & test

```sh
cargo build --workspace
cargo test  --workspace
```

## Commit messages

This repo uses Conventional Commits so release-plz can derive changelogs and
version bumps. Enable the versioned commit hook in new checkouts with:

```sh
git config core.hooksPath .githooks
```

The parser/layout integration tests use a vendored
[daml-finance](https://github.com/digital-asset/daml-finance) corpus under
[`corpus/daml-finance/`](corpus/daml-finance/) (634 real `.daml` files), shared
at the workspace root by `daml-parser` and `daml-lint`. The formatter is
differential-tested over 924 files (`cd crates/daml-fmt && npm test`).

## Install the CLIs

```sh
cargo install daml-lint
cargo install daml-fmt
```

## Versioning & release

Each crate is versioned independently. Before the first crates.io baseline,
`cargo-semver-checks` is a soft CI signal; after 0.1.0 is published it should be
made a hard gate. While the crates are pre-1.0, breaking public API changes use
0.x minor bumps and patch releases stay compatible.

Releases are driven by [release-plz](release-plz.toml) in dependency order
(parser first, then lint + fmt). The GitHub workflow expects a crates.io
`CARGO_REGISTRY_TOKEN`; set `RELEASE_PLZ_TOKEN` to a PAT if release PRs/tags
must trigger follow-on workflows. CLI release archives and SHA-256 files are
built for Linux x64, macOS ARM64, and Windows x64 when `daml-lint-v*` or
`daml-fmt-v*` tags are published.

## License

AGPL-3.0-only. See [LICENSE](LICENSE). The vendored corpus keeps its own
upstream license — see [`corpus/daml-finance/README.md`](corpus/daml-finance/README.md).
