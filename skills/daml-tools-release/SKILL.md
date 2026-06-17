---
name: daml-tools-release
description: Run the daml-tools release process end to end. Use when preparing, triggering, merging, monitoring, troubleshooting, or verifying releases for the Rust crates, GitHub release artifacts, and @daml-tools/lint-plugin npm package in stevennevins/daml-tools.
---

# Daml Tools Release

## Overview

Use this workflow from a clean checkout of `stevennevins/daml-tools`. The
canonical contributor guide is `docs/how-to/release.md`; this skill is the
agent runbook for executing it without missing the registry and follow-up PR
checks.

## Prepare

1. Fetch first: `git fetch --all --tags --prune`.
2. Confirm `gh auth status` has `repo` and `workflow` scopes.
3. Inspect current state:
   - `git status -sb --branch`
   - `gh pr list --state open --json number,title,headRefName,url`
   - `gh run list --repo stevennevins/daml-tools --limit 12`
4. Check `gh secret list --repo stevennevins/daml-tools` includes
   `CARGO_REGISTRY_TOKEN` and `RELEASE_PLZ_TOKEN`.

## Change

Make the smallest semver-relevant change needed to trigger release-plz. Use a
Conventional Commit type matched by `release-plz.toml`:
`feat`, `fix`, `perf`, `refactor`, `security`, or a breaking `!` commit.
Docs, CI, and chore commits must not trigger crate releases.

For `daml-lint`, keep all npm metadata synchronized with the Rust crate:

```sh
cd crates/daml-lint
npm run build:lint-plugin-version
npm run check:rules
```

The sync/check covers:

- `crates/daml-lint/package.json`
- `crates/daml-lint/package-lock.json`
- `crates/daml-lint/lint-plugin/package.json`
- `crates/daml-lint/lint-plugin/templates/project/package.json`

## Validate Locally

Run the smallest local gate that proves the release-sensitive behavior, then run
the broader checks that are practical before pushing:

```sh
cargo fmt --all --check
cargo test --workspace --all-features --locked
cd crates/daml-lint && npm run check:rules
```

Use `cargo package -p <crate> --allow-dirty` or `cargo publish -p <crate>
--dry-run --allow-dirty` when packaging metadata is part of the change.

## Publish Through PR

Push a branch and open a PR. After CI is green, merge it to `main`. The push to
`main` triggers `.github/workflows/release-plz.yml`:

1. `release-plz release` publishes crate versions whose release PRs were merged.
2. `release-plz release-pr` opens or updates the next release PR.
3. For `daml-lint`, the workflow commits npm metadata sync into the release PR.

When release-plz opens the release PR, verify crate versions, changelogs, and
all npm metadata files above, then merge the release PR.

## Monitor

Do not stop after merge. Watch the workflow chain:

```sh
gh run list --repo stevennevins/daml-tools --limit 20
gh pr list --state open --json number,title,headRefName,url
```

For failures, inspect logs with:

```sh
gh run view <run-id> --log
```

If a tag exists but npm publishing failed, rerun:

```sh
gh workflow run npm-publish.yml --repo stevennevins/daml-tools --ref main -f tag=daml-lint-vX.Y.Z
```

If release artifacts failed, rerun:

```sh
gh workflow run release-artifacts.yml --repo stevennevins/daml-tools --ref main -f tag=daml-lint-vX.Y.Z
gh workflow run release-artifacts.yml --repo stevennevins/daml-tools --ref main -f tag=daml-fmt-vX.Y.Z
```

Do not add npm provenance while this repository is private.

## Verify Publication

Collect direct evidence after workflows complete:

```sh
cargo search daml-parser --limit 3
cargo search daml-lint --limit 3
cargo search daml-fmt --limit 3
npm view @daml-tools/lint-plugin version dist-tags time.modified --prefer-online
gh release view daml-lint-vX.Y.Z --repo stevennevins/daml-tools --json assets,publishedAt
gh release view daml-fmt-vX.Y.Z --repo stevennevins/daml-tools --json assets,publishedAt
```

A release is complete only when the expected crates.io versions, npm version,
GitHub releases, release assets, CI runs, and follow-up release PR state are all
verified against current external state.
