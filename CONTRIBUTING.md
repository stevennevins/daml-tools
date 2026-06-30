# Contributing

Use this guide to prepare a checkout and run the local gates before CI.

For tool usage, start with the root [README](README.md) or the published documentation
site at [https://stevennevins.github.io/daml-tools/](https://stevennevins.github.io/daml-tools/).

## Prerequisites

**[mise](https://mise.jdx.dev/) is the canonical toolchain** for version alignment
with CI and local signoff. [`mise.toml`](mise.toml) and [`mise.lock`](mise.lock)
pin Rust 1.96.0 (with `rustfmt` and `clippy`), Node.js 22, npm, `cargo-npm`,
`cargo-semver-checks`, `cargo-deny`, `act`, `gh`, `lychee`, and related tools.

Install these **outside mise**:

- **[uv](https://docs.astral.sh/uv/)** — installs `prek` for Git hook management
- **Docker** — required for maintainer signoff through [act](https://github.com/nektos/act)
- **`gh-signoff`** — GitHub CLI extension for maintainer PR signoff (see
  [`developer-docs/how-to/local-ci.md`](developer-docs/how-to/local-ci.md))

If you cannot use mise, install Rust 1.96.0 or newer with the `rustfmt` and
`clippy` components, Node.js 18 or newer, `cargo-semver-checks`, and
`cargo-npm` 0.1.2 manually. CI uses Node.js 22; the formatter and linter checks
require Node.js 18 or newer.

```sh
rustup component add rustfmt clippy
uv tool install prek
cargo install cargo-semver-checks --locked
cargo install cargo-npm --version 0.1.2 --locked
```

## Bootstrap checklist (fresh checkout)

Follow this path from a clean clone to a verified contributor environment:

1. **Install mise** if it is not already on `PATH` (see
   [`developer-docs/how-to/local-ci.md`](developer-docs/how-to/local-ci.md) for
   platform install and shell activation).
2. **Trust and activate mise** in the repo, then install locked tools:

   ```sh
   mise trust
   eval "$(mise activate zsh)"   # use bash/fish/pwsh activation when appropriate
   export MISE_LOCKED=1
   mise install
   ```

3. **Verify the toolchain** resolves through mise and includes formatting/lint
   components:

   ```sh
   mise which cargo
   mise which node
   rustc --version
   rustfmt --version
   cargo clippy --version
   mise ls    # same output as mise list
   ```

   If `rustfmt --version` or `cargo clippy --version` fails, ensure the Rust
   toolchain includes those components (`mise.toml` requests them; re-run
   `mise install` after trusting the repo).

4. **Install out-of-mise prerequisites**:

   ```sh
   uv tool install prek
   ```

   For maintainer signoff, also ensure Docker is running and install
   `gh extension install basecamp/gh-signoff` after `mise install` provides
   `gh`.

5. **Enable Git hooks**:

   ```sh
   git config core.hooksPath .githooks
   prek prepare-hooks
   ```

6. **Run Tier 0 — Bootstrap** (see [Verification tiers](#verification-tiers)):

   ```sh
   cargo build --workspace
   cargo test --workspace --all-features --locked
   ```

## Verification tiers

Use these tiers to match local work to CI boundaries. Maintainer signoff details
stay in [`developer-docs/how-to/local-ci.md`](developer-docs/how-to/local-ci.md).

| Tier | When | Commands / scope | CI overlap |
|------|------|------------------|------------|
| **0 — Bootstrap** | Fresh checkout | Toolchain verify, hooks, `cargo build` + `cargo test` | — |
| **1 — Commit** | Every commit | `cargo fmt`, `cargo clippy`, `cargo doc` (pre-commit hook) | Partial `test` job |
| **2 — Push** | Before `git push` | Workspace tests, formatter `npm test`, semver, `check-package.sh` | `test`, `semver`, `package` |
| **3 — Focused** | Touching specific areas | Lint rules, `cargo deny`, feature splits, corpus checks | `cargo-deny`, targeted tests |
| **4 — Docs** | Doc/README/version metadata | `npm run build --prefix docs`; docs workflow link/ version checks | `docs.yml` |
| **5 — Maintainer signoff** | Required PR merge gate | `mise run signoff:*` via act + `gh signoff` | Full `ci.yml` + `docs.yml` jobs |

**Tier 4 — Docs** also covers the VitePress dev dependency tree. `npm audit
--prefix docs` may report known transitive advisories (for example through
VitePress/Vite/esbuild) while `npm run build --prefix docs` still passes. Treat
audit output as informational unless CI adds an audit gate; re-check after
VitePress or Vite upgrades.

**Tier 5** is for maintainers applying required PR signoff contexts. Contributors
can land changes after Tiers 1–2 (and Tier 3–4 when their change touches those
areas) without running act locally.

## Enable Git hooks

This repo tracks Git hook shims in `.githooks` and manages hook behavior with
[`prek.toml`](prek.toml). Enable the hooks in a new checkout:

```sh
git config core.hooksPath .githooks
prek prepare-hooks
```

## Run commit checks (Tier 1)

Run the same lint gates as the pre-commit hook:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
```

The commit message hook enforces Conventional Commits so release-plz can derive
changelogs and version bumps.

## Run push checks (Tier 2)

Run the heavier gates before pushing:

```sh
cargo test --workspace --all-features --locked
(cd crates/daml-fmt && npm test)
for package in daml-parser daml-syntax daml-lint daml-fmt; do
  cargo semver-checks check-release --package "$package"
done
bash scripts/check-package.sh
```

`scripts/check-package.sh` runs `cargo package --verify` for every published
crate. `daml-parser` always verifies because it has no internal registry
dependencies. Downstream crates verify only after the workspace `daml-parser`
version is on crates.io, so a raised parser lower bound is published before
`daml-lint` or `daml-fmt` package verification can pass.

Doc and version consistency checks run in `.github/workflows/docs.yml` on
doc/README/version-metadata changes. That workflow checks crate versions in
`docs/reference/crates.md`, crate README dependency snippets, and
`crates/daml-fmt/package.json`, plus offline markdown link checking with
lychee. `daml-lint` npm package versions are checked by `npm run check:rules`
in CI instead.

The formatter `npm test` command runs `node test/diff.js`, the same 924-file
differential test used by the pre-push hook.

Maintainers applying required PR signoffs use **Tier 5** in
[`developer-docs/how-to/local-ci.md`](developer-docs/how-to/local-ci.md). Set
`MISE_LOCKED=1`, run `mise run signoff:*` tasks, and create matching
`signoff/...` commit statuses. Do not use `gh signoff install` or
`gh signoff uninstall` on the real repo.

## Local npm CLI package smoke (maintainer / release)

The `signoff/npm-package` CI job validates generated npm wrapper and platform
packages on the host Linux triple. Run this locally before release work or when
changing npm packaging:

```sh
target="$(rustc -vV | sed -n 's/^host: //p')"
case "${target}" in
  x86_64-unknown-linux-gnu) npm_platform="linux-x64" ;;
  aarch64-unknown-linux-gnu) npm_platform="linux-arm64" ;;
  *) echo "unsupported npm package smoke target: ${target}" >&2; exit 1 ;;
esac
cargo build --release --locked --target "${target}" --bin daml-lint --bin daml-fmt
for tool in daml-lint daml-fmt; do
  out="target/npm-ci/${tool}"
  cargo npm generate -p "${tool}" --target "${target}" --out-dir "${out}" --clean
  node -e "const p=require('./'+process.argv[1]); if(!p.bin||!p.optionalDependencies||!Object.keys(p.optionalDependencies).length)throw new Error('wrapper missing bin/optionalDependencies')" "${out}/@daml-tools/${tool}/package.json"
  bin="${out}/@daml-tools/${tool}-${npm_platform}/${tool}"
  test -x "${bin}"
  "${bin}" --version
done
```

Or run the matching act job: `mise run signoff:ci:npm-package` (Tier 5).

## Optional Daml SDK 3.4.11

The Daml SDK is **not required** for default contributor work or CI signoff.
Workspace tests, the formatter differential harness (`npm test` in
`crates/daml-fmt`), and hosted CI do not depend on a local `daml` install.

Install Daml SDK **3.4.11** (`daml` on `PATH`, verify with `daml version`) only
when you need the formatter **desugar oracle** — for example release candidates
or risky `daml-fmt` changes. See
[`developer-docs/how-to/verify-formatter-change.md`](developer-docs/how-to/verify-formatter-change.md)
and [`developer-docs/how-to/release.md`](developer-docs/how-to/release.md).

Recent SDK builds may print a legacy-assistant deprecation warning; the repo's
desugar commands pass `--no-legacy-assistant-warning` to `damlc` where needed.
Use your platform's Daml 3.4.x install path; no brittle installer steps are
duplicated here.

```sh
bash crates/daml-fmt/tools/verify-rust.sh              # idempotence + curated desugar subset
bash crates/daml-fmt/tools/verify-rust.sh --desugar    # full corpus before risky fmt releases
```

Without the SDK, `tools/verify-rust.sh` skips desugar checks and
`npm test` remains the default formatter gate.

## Run focused checks (Tier 3)

When changing shipped lint rules or rule generation, check the generated rules:

```sh
bash scripts/check-lint-rules.sh
```

`scripts/check-lint-rules.sh` runs `npm run check:rules` from `crates/daml-lint`.
The npm gate snapshots generated rule artifacts before rebuilding them and fails
if the rebuild changes any bytes, so the wrapper does not stage paths or disturb
a partially staged index. That proves TypeScript custom-rule contracts and
regenerated outputs are in sync during review even when `examples/daml-lint.d.ts` and
`lint-plugin/dist/index.d.ts` are intentionally uncommitted.

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

## Run docs checks (Tier 4)

When changing published docs, crate version metadata, or README dependency
snippets checked by CI:

```sh
npm ci --prefix docs
npm run build --prefix docs
```

The docs workflow also runs offline link checking with `lychee` and version
consistency checks; see `.github/workflows/docs.yml` for the full gate list.

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
[`developer-docs/how-to/verify-formatter-change.md`](developer-docs/how-to/verify-formatter-change.md).

## Troubleshooting bootstrap

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `rustfmt` or `clippy` not found | Rust components missing from active toolchain | Re-run `mise install` with `MISE_LOCKED=1`; confirm `rustfmt --version` and `cargo clippy --version` |
| Wrong Rust/Node version vs CI | Commands not resolving through mise | Run `mise which cargo` and `mise which node`; activate mise in the current shell |
| `prek` not found | `uv` tool not installed | `uv tool install prek` |
| act signoff fails immediately | Docker not running or `gh-signoff` missing | Start Docker; `gh extension install basecamp/gh-signoff` |
| Formatter tests pass but release blocked | Desugar oracle needs SDK | Optional: install Daml SDK 3.4.11 and run `tools/verify-rust.sh --desugar` |

## Prepare versioning and releases

Each crate is versioned independently. `cargo-semver-checks` is a blocking CI
and pre-push gate for published crates, so public API compatibility breaks must
be resolved before merging. While the crates are pre-1.0, intentional breaking
public API changes use 0.x minor bumps and patch releases stay compatible.

Releases are driven by [release-plz](release-plz.toml) in dependency order:
`daml-parser` first, then `daml-syntax`, then `daml-lint` and `daml-fmt`. The GitHub workflow
expects a crates.io `CARGO_REGISTRY_TOKEN`; set `RELEASE_PLZ_TOKEN` to a PAT if
release PRs or tags must trigger follow-on workflows.

CLI release archives and SHA-256 files are built for Linux x64, Linux ARM64,
macOS ARM64, and Windows x64 when `daml-lint-v*` or `daml-fmt-v*` tags are
published. Those GitHub release assets are `daml-tools-*` bundles; parser
releases publish the Rust crate and release notes, but no binary assets.
