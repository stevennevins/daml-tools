# Release the workspace

Use this flow to publish Rust crates, the `@daml-tools/lint-plugin` npm
package, npm-distributed CLI packages, and GitHub release artifacts.

## Check release authentication

Confirm the required GitHub secrets exist:

```sh
gh secret list --repo stevennevins/daml-tools
```

Required secrets:

| Secret | Purpose |
|--------|---------|
| `CARGO_REGISTRY_TOKEN` | Lets release-plz publish crates to crates.io. |
| `RELEASE_PLZ_TOKEN` | Lets release-plz-created tags trigger follow-up workflows. |

Optional bootstrap secret:

| Secret | Purpose |
|--------|---------|
| `NPM_TOKEN` | Lets a manually dispatched npm publish workflow use a short-lived npm granular access token for the first publish of brand-new npm packages, before trusted publishing is configured. Remove it after bootstrap. |

The npm packages use npm trusted publishing, so they do not need an npm token in
GitHub secrets. Configure trusted publishing on npmjs.com for every published
package:

- `@daml-tools/lint-plugin`
- `@daml-tools/daml-lint`
- `@daml-tools/daml-lint-darwin-arm64`
- `@daml-tools/daml-lint-linux-x64`
- `@daml-tools/daml-lint-win32-x64`
- `@daml-tools/daml-fmt`
- `@daml-tools/daml-fmt-darwin-arm64`
- `@daml-tools/daml-fmt-linux-x64`
- `@daml-tools/daml-fmt-win32-x64`

For the first CLI npm release, the new packages may not exist yet on npm. If
npm requires an existing package before trusted publishing can be configured,
add a temporary `NPM_TOKEN` granular-access-token secret, dispatch the npm
publish workflow manually with `use_npm_token=true`, publish the first versions,
configure trusted publishing for each package, then remove the token. Normal tag
workflows ignore `NPM_TOKEN` and use trusted publishing.

Do not use the already-created `daml-lint-v0.3.3` or `daml-fmt-v0.2.1` tags to
bootstrap the CLI npm packages. Those tags predate the `crates/*/npm` package
layout. Let release-plz create new tags after this packaging change lands.

## Review the release PR

Release-plz opens a `chore: release` PR after semver-relevant changes land on
`main`. Normal `docs:`, `ci:`, and `chore:` commits do not prepare releases;
`feat:`, `fix:`, `perf:`, `refactor:`, `security:`, and breaking `!` commits
do.

For `daml-lint` release PRs, the Release-plz workflow also syncs the npm
demo/rules package, lockfile, public plugin package, template dependency, and
`@daml-tools/daml-lint` CLI package metadata into the release PR.

For `daml-fmt` release PRs, the Release-plz workflow syncs the private test
package and `@daml-tools/daml-fmt` CLI package metadata into the release PR.

Before merging it, verify:

- Rust crate versions and changelogs are correct.
- `crates/daml-lint/package.json`,
  `crates/daml-lint/package-lock.json`, and
  `crates/daml-lint/lint-plugin/package.json` match the `daml-lint` crate
  version when `daml-lint` is being released.
- `crates/daml-lint/lint-plugin/templates/project/package.json` depends on the
  new `@daml-tools/lint-plugin` version.
- `crates/daml-lint/npm/**/package.json` matches the `daml-lint` crate version
  when `daml-lint` is being released.
- `crates/daml-fmt/package.json` and `crates/daml-fmt/npm/**/package.json`
  match the `daml-fmt` crate version when `daml-fmt` is being released.
- CI is green.

CI fails if npm CLI package metadata does not match the Cargo package versions
or if the wrapper packages cannot be packed and installed as dev dependencies.

## Merge the release PR

Merge the release PR into `main`.

This triggers the release flow:

1. `Release-plz` publishes changed Rust crates to crates.io.
2. Release-plz creates GitHub tags and releases.
3. `daml-lint-v*` tags publish `@daml-tools/lint-plugin` to npm.
4. `daml-lint-v*` tags publish the `@daml-tools/daml-lint-*` platform packages,
   then the `@daml-tools/daml-lint` wrapper package.
5. `daml-fmt-v*` tags publish the `@daml-tools/daml-fmt-*` platform packages,
   then the `@daml-tools/daml-fmt` wrapper package.
6. `daml-lint-v*` and `daml-fmt-v*` tags upload CLI archives to GitHub
   releases.

Release-plz creates a follow-up release PR only after the release job finishes.

## Verify the release

Check workflow status:

```sh
gh run list --repo stevennevins/daml-tools --limit 12
```

Check crates.io:

```sh
cargo search daml-parser --limit 3
cargo search daml-lint --limit 3
cargo search daml-fmt --limit 3
```

Check npm:

```sh
npm view @daml-tools/lint-plugin version dist-tags time.modified --prefer-online
npm view @daml-tools/daml-lint version dist-tags time.modified --prefer-online
npm view @daml-tools/daml-fmt version dist-tags time.modified --prefer-online
```

Check GitHub release assets:

```sh
gh release view daml-lint-vX.Y.Z --repo stevennevins/daml-tools --json assets,url
gh release view daml-fmt-vX.Y.Z --repo stevennevins/daml-tools --json assets,url
```

## Recover a failed npm publish

If the `Publish npm Packages` workflow fails after the tag exists, rerun it with
the release tag:

```sh
gh workflow run npm-publish.yml --repo stevennevins/daml-tools --ref main -f tag=daml-lint-vX.Y.Z
gh workflow run npm-publish.yml --repo stevennevins/daml-tools --ref main -f tag=daml-fmt-vX.Y.Z
```

The npm workflow checks out the requested tag, verifies package versions, skips
any package version that already exists on npm, builds the tagged CLI binary for
each missing supported platform package, publishes the missing platform
packages, and then publishes the wrapper package if it is missing.

Only set `use_npm_token=true` while recovering the first bootstrap publish of a
brand-new npm package. Leave it unset for normal trusted publishing reruns.

Do not add `--provenance` while this repository is private. npm trusted
publishing works from this repo, but npm rejects provenance bundles whose
GitHub source repository is private.

## Recover failed release artifacts

If release artifact upload fails after a tag exists, rerun the artifact workflow
with that tag:

```sh
gh workflow run release-artifacts.yml --repo stevennevins/daml-tools --ref main -f tag=daml-lint-vX.Y.Z
gh workflow run release-artifacts.yml --repo stevennevins/daml-tools --ref main -f tag=daml-fmt-vX.Y.Z
```

The artifact workflow checks out the requested tag and uploads archives to that
release.
