---
name: daml-tools-release
description: Run the daml-tools release and prerelease process end to end. Use when preparing, triggering, merging, monitoring, troubleshooting, or verifying Rust crate releases, cargo-npm CLI npm packages, @daml-tools/lint-plugin, GitHub release artifacts, or npm @next prereleases in stevennevins/daml-tools.
---

# Daml Tools Release

Use this workflow from a clean checkout of `stevennevins/daml-tools`. The
canonical contributor guide is `developer-docs/how-to/release.md`; this skill is the
agent runbook for executing releases without missing registry, dist-tag, and
follow-up PR checks.

## Prepare

1. Fetch first: `git fetch --all --tags --prune`.
2. Confirm `gh auth status` has `repo` and `workflow` scopes.
3. Inspect current state:
   - `git status -sb --branch`
   - `gh pr list --state open --json number,title,headRefName,url`
   - `gh run list --repo stevennevins/daml-tools --limit 12`
4. Check `gh secret list --repo stevennevins/daml-tools` includes
   `CARGO_REGISTRY_TOKEN` and `RELEASE_PLZ_TOKEN`. `NPM_TOKEN` is optional and
   only for a manual `use_npm_token=true` bootstrap/fallback dispatch.
5. Do not add `npm --provenance` while the repository is private.

## Local Validation

Run the smallest local gate that proves the release-sensitive behavior, then run
broader checks that are practical before pushing:

```sh
actionlint .github/workflows/*.yml
cargo fmt --all --check
cargo test --workspace --all-features --locked
(cd crates/daml-lint && npm ci && npm run check:rules)
```

For CLI npm packaging changes, use pinned cargo-npm:

```sh
cargo install cargo-npm --version 0.1.2 --locked
target="$(rustc -vV | sed -n 's/^host: //p')"
cargo build --release --locked --target "$target" --bin daml-lint --bin daml-fmt
cargo npm generate -p daml-lint --infer-targets --out-dir /tmp/daml-lint-npm --clean
cargo npm generate -p daml-fmt --infer-targets --out-dir /tmp/daml-fmt-npm --clean
```

Inspect generated wrapper package JSON for `bin`, `engines.node >=18`, and
version-pinned `optionalDependencies`. Locally, run only host-compatible
generated binaries. CI proves all supported targets: Linux x64, Linux ARM64,
macOS ARM64, and Windows x64.

For release-plz or public Rust API changes, validate the whole publishable
dependency closure before pushing:

1. Identify every changed crate and every workspace crate that publicly exposes
   it through dependencies, re-exports, public types, generated npm metadata, or
   release configuration.
2. Bump every affected crate version in the same release chain. Do not bump only
   the leaf crate if crates.io package verification will resolve an older
   published dependency.
3. Check that `release-plz.toml`, CI semver matrices, local semver scripts,
   `Cargo.toml`, `Cargo.lock`, generated npm package files, and starter
   templates name the same intended versions.
4. Run `cargo package` for each affected crate when practical. If a package
   cannot verify until another unpublished crate in the same release chain is on
   crates.io, record that explicitly and rely on release-plz ordering plus the
   release configuration check.

## Normal Release

Make the smallest semver-relevant change needed to trigger release-plz. Use a
Conventional Commit type matched by `release-plz.toml`: `feat`, `fix`, `perf`,
`refactor`, `security`, or a breaking `!` commit. Docs, CI, and chore commits
must not trigger crate releases.

Push a branch and open a PR. After CI is green, merge it to `main`.
`.github/workflows/release-plz.yml` then:

1. Publishes changed Rust crates to crates.io.
2. Opens or updates the follow-up release PR.
3. For `daml-lint` only, syncs npm metadata for the lint-plugin package and
   starter template. Do not sync CLI npm metadata; cargo-npm generates CLI npm
   packages from `Cargo.toml`.

When release-plz opens the release PR, verify crate versions, changelogs, and
the lint-plugin npm metadata when `daml-lint` is released:

```sh
crates/daml-lint/package.json
crates/daml-lint/package-lock.json
crates/daml-lint/lint-plugin/package.json
crates/daml-lint/lint-plugin/templates/project/package.json
```

Merge the release PR after CI is green. The resulting `daml-lint-v*` and
`daml-fmt-v*` tags trigger:

- `npm-publish.yml`: cargo-npm publishes platform packages first, then the
  wrapper package. Plain `X.Y.Z` versions publish under `latest`.
- `npm-publish.yml`: `daml-lint-v*` also publishes `@daml-tools/lint-plugin`.
- `release-artifacts.yml`: uploads CLI archives and checksums to the GitHub
  release.

Use these reusable gates for PR-to-release automation instead of treating the
process as one long wait:

1. Publish PR: commit the focused diff, let hooks run, push, open a draft PR,
   and record the PR URL and head SHA.
2. Wait for PR checks: poll until all required checks report pass/fail. If a
   check fails, inspect logs before changing code; do not guess from the check
   name.
3. Mark ready: mark the PR ready only after required checks pass.
4. Merge: merge through GitHub and record the target branch SHA. Do not depend
   on the local `main` checkout being available in this worktree.
5. Watch release-plz on `main`: inspect any failed release-plz run logs. If it
   opens or updates a release PR, monitor that PR's checks separately.
6. Merge release PR: merge only after release PR CI is green, then watch the tag
   workflows and verify registries/releases.

## Prerelease

Use prereleases to test CLI npm distribution under `@next` without moving
`latest`. This does not publish crates to crates.io and does not run
release-plz.

1. Choose a version that cannot collide with existing npm packages, for example
   `0.2.6-rc.0`.
2. On a branch, bump exactly the crate being tested:

   ```sh
   cargo set-version -p daml-fmt 0.2.6-rc.0
   # or edit crates/daml-fmt/Cargo.toml and run cargo build to refresh Cargo.lock
   ```

   For a `daml-lint` prerelease that also tests `@daml-tools/lint-plugin`, sync
   lint-plugin metadata:

   ```sh
   (cd crates/daml-lint && node tools/sync-lint-plugin-version.mjs)
   ```

   Do not create or edit `crates/*/npm`; cargo-npm generates CLI npm packages.

3. Validate locally, commit, push the branch, create the tag at that commit, and
   push the tag:

   ```sh
   git tag daml-fmt-v0.2.6-rc.0
   git push origin HEAD
   git push origin daml-fmt-v0.2.6-rc.0
   ```

   If trusted publishing needs fallback auth, dispatch from the tag instead:

   ```sh
   gh workflow run npm-publish.yml --repo stevennevins/daml-tools \
     --ref daml-fmt-v0.2.6-rc.0 \
     -f tag=daml-fmt-v0.2.6-rc.0 \
     -f use_npm_token=true
   ```

4. Watch `npm-publish.yml` and `release-artifacts.yml`. A semver prerelease
   publishes CLI npm packages under `next`, keeps `latest` unchanged, and marks
   the GitHub release as a prerelease.

5. Verify on the registry:

   ```sh
   npm view @daml-tools/daml-fmt dist-tags --prefer-online
   npm view @daml-tools/daml-fmt-linux-x64@0.2.6-rc.0 version --prefer-online
   tmp="$(mktemp -d)" && cd "$tmp"
   npm init -y >/dev/null
   npm install --save-dev @daml-tools/daml-fmt@next
   npx --no-install daml-fmt --version
   ```

If a prerelease publish partially fails, inspect the registry state before
retrying. The workflow skips generated package versions that already exist,
publishes missing platform packages first, then publishes the wrapper.

If old tooling already published platform packages for a version but missed the
wrapper package, recover through the workflow with `use_npm_token=true` only
after confirming which immutable package versions already exist.

## Monitor And Recover

Do not stop after merge or tag push. Watch the workflow chain:

```sh
gh run list --repo stevennevins/daml-tools --limit 20
gh pr list --state open --json number,title,headRefName,url
```

Inspect failures with:

```sh
gh run view <run-id> --log
```

If npm publishing failed after a tag exists, rerun from the tag:

```sh
gh workflow run npm-publish.yml --repo stevennevins/daml-tools \
  --ref daml-lint-vX.Y.Z -f tag=daml-lint-vX.Y.Z
gh workflow run npm-publish.yml --repo stevennevins/daml-tools \
  --ref daml-fmt-vX.Y.Z -f tag=daml-fmt-vX.Y.Z
```

If release artifacts failed, rerun:

```sh
gh workflow run release-artifacts.yml --repo stevennevins/daml-tools \
  --ref daml-lint-vX.Y.Z -f tag=daml-lint-vX.Y.Z
gh workflow run release-artifacts.yml --repo stevennevins/daml-tools \
  --ref daml-fmt-vX.Y.Z -f tag=daml-fmt-vX.Y.Z
```

Do not dispatch a first-time publish from a branch. The npm workflow refuses to
publish missing package versions unless the checked-out ref is the release tag.

## Verify Publication

Collect direct evidence after workflows complete:

```sh
cargo search daml-parser --limit 3
cargo search daml-lint --limit 3
cargo search daml-fmt --limit 3
npm view @daml-tools/lint-plugin version dist-tags time.modified --prefer-online
npm view @daml-tools/daml-lint version dist-tags time.modified --prefer-online
npm view @daml-tools/daml-fmt version dist-tags time.modified --prefer-online
gh release view daml-lint-vX.Y.Z --repo stevennevins/daml-tools --json assets,publishedAt
gh release view daml-fmt-vX.Y.Z --repo stevennevins/daml-tools --json assets,publishedAt
```

A release is complete only when the expected crates.io versions, npm versions
and dist-tags, GitHub releases, release assets, CI runs, and follow-up release
PR state are all verified against current external state.
