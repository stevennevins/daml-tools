# Release the workspace

Use this flow to publish Rust crates, the `@daml-tools/lint-plugin` npm
package, and CLI release artifacts.

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

The npm package uses npm trusted publishing, so it does not need an npm token in
GitHub secrets.

## Review the release PR

Release-plz opens a `chore: release` PR after changes land on `main`.

Before merging it, verify:

- Rust crate versions and changelogs are correct.
- `crates/daml-lint/lint-plugin/package.json` matches the `daml-lint` crate
  version when `daml-lint` is being released.
- `crates/daml-lint/lint-plugin/templates/project/package.json` depends on the
  new `@daml-tools/lint-plugin` version.
- CI is green.

The `daml-lint` CI check fails if the npm package version does not match the
`daml-lint` crate version.

## Merge the release PR

Merge the release PR into `main`.

This triggers the release flow:

1. `Release-plz` publishes changed Rust crates to crates.io.
2. Release-plz creates GitHub tags and releases.
3. `daml-lint-v*` tags publish `@daml-tools/lint-plugin` to npm.
4. `daml-lint-v*` and `daml-fmt-v*` tags upload CLI archives to GitHub
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
```

Check GitHub release assets:

```sh
gh release view daml-lint-vX.Y.Z --repo stevennevins/daml-tools --json assets,url
gh release view daml-fmt-vX.Y.Z --repo stevennevins/daml-tools --json assets,url
```

## Recover a failed npm publish

If the `Publish npm Package` workflow fails after the tag exists, rerun it with
the release tag:

```sh
gh workflow run npm-publish.yml --repo stevennevins/daml-tools --ref main -f tag=daml-lint-vX.Y.Z
```

The npm workflow checks out the requested tag and verifies the package version
before publishing.

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
