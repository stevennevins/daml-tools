# Contributing

Use this guide to prepare a checkout and run the local gates before CI.

For tool usage, start with the root [README](README.md) or the documentation
map in [`docs/`](docs/).

## Prerequisites

Install:

- Rust 1.87.0 or newer
- the Rust `rustfmt` and `clippy` components
- Node.js 18 or newer
- `uv`, for installing `prek`
- `cargo-semver-checks`
- `cargo-npm` 0.1.2, for local npm CLI package generation

```sh
rustup component add rustfmt clippy
uv tool install prek
cargo install cargo-semver-checks --locked
cargo install cargo-npm --version 0.1.2 --locked
```

CI currently uses Node.js 22, but the local formatter and linter checks require
Node.js 18 or newer.

## Enable Git hooks

This repo tracks Git hook shims in `.githooks` and manages hook behavior with
[`prek.toml`](prek.toml). Enable the hooks in a new checkout:

```sh
git config core.hooksPath .githooks
prek prepare-hooks
```

## Run commit checks

Run the same lint gates as the pre-commit hook:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

The commit message hook enforces Conventional Commits so release-plz can derive
changelogs and version bumps.

## Run push checks

Run the heavier gates before pushing:

```sh
cargo test --workspace --all-features --locked
(cd crates/daml-fmt && npm test)
for package in daml-parser daml-lint daml-fmt; do
  cargo semver-checks check-release --package "$package"
done
```

The formatter `npm test` command runs `node test/diff.js`, the same 924-file
differential test used by the pre-push hook.

## Run focused checks

When changing shipped lint rules or rule generation, check the generated rules:

```sh
(cd crates/daml-lint && npm ci && npm run check:rules)
```

When changing `daml-lint` feature flags or optional runtime code, run the
feature split tested by CI:

```sh
cargo test -p daml-lint --no-default-features --lib --locked
cargo test -p daml-lint --no-default-features --features js-runtime --lib --locked
cargo test -p daml-lint --no-default-features --features custom-rules --lib --locked
cargo test -p daml-lint --no-default-features --features cli,js-runtime --locked
cargo test -p daml-lint --no-default-features --features cli,js-runtime,custom-rules --locked
```

When changing dependencies or license policy, run:

```sh
cargo deny check
```

## Run corpus-backed checks

The parser/layout integration tests use a vendored
[daml-finance](https://github.com/digital-asset/daml-finance) corpus under
[`corpus/daml-finance/`](corpus/daml-finance/) (634 real `.daml` files), shared
at the workspace root by `daml-parser` and `daml-lint`.

The formatter is differential-tested over 924 files from `crates/daml-fmt`:

```sh
cd crates/daml-fmt
npm test
```

For formatter-specific verification flows, see
[`docs/how-to/verify-formatter-change.md`](docs/how-to/verify-formatter-change.md).

## Prepare versioning and releases

Each crate is versioned independently. Before the first crates.io baseline,
`cargo-semver-checks` runs in CI and pre-push hooks. While the crates are
pre-1.0, breaking public API changes use 0.x minor bumps and patch releases
stay compatible.

Releases are driven by [release-plz](release-plz.toml) in dependency order:
`daml-parser` first, then `daml-lint` and `daml-fmt`. The GitHub workflow
expects a crates.io `CARGO_REGISTRY_TOKEN`; set `RELEASE_PLZ_TOKEN` to a PAT if
release PRs or tags must trigger follow-on workflows.

CLI release archives and SHA-256 files are built for Linux x64, Linux ARM64,
macOS ARM64, and Windows x64 when `daml-lint-v*` or `daml-fmt-v*` tags are
published.
