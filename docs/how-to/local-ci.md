# Run CI locally with mise, act, and gh-signoff

Use [mise](https://mise.jdx.dev/) for pinned tool versions and [act](https://github.com/nektos/act)
to run GitHub Actions workflows on Linux. The repo `.actrc` maps Linux runner labels to
digest-pinned, multi-architecture Docker images and keeps act runtime state under ignored `.act/` paths so caches and
artifacts do not get committed. We intentionally avoid `.git/act/` because this repo is often used from git worktrees where `.git` is a file, not a directory.

GitHub Actions YAML remains the source of truth. Local signoff runs the same
workflow jobs through act instead of a bespoke CI wrapper. Use `MISE_LOCKED=1`
locally, matching the GitHub workflows, so drift from `mise.toml` and
`mise.lock` fails loudly.

Install tools from the committed lockfile before running act or gh-signoff:

```sh
MISE_LOCKED=1 mise install
```

List workflows act can run:

```sh
MISE_LOCKED=1 mise x -- act -l
```

## Sign off on PR gates locally

Install the gh-signoff extension once for the GitHub CLI managed by mise:

```sh
MISE_LOCKED=1 mise x -- gh extension install basecamp/gh-signoff
```

Before signing off, make sure the checkout is clean, the current commit is
pushed to the PR branch, and the matching act job exits successfully. Each
`gh signoff` command creates a GitHub commit status for `HEAD`; do not sign off
for a job that failed, was skipped, or was run with a different toolchain.

Run one local job per required signoff context:

| Required PR context | Validate with act | Create the status |
|---------------------|-------------------|-------------------|
| `signoff/test` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j test` | `MISE_LOCKED=1 mise x -- gh signoff test` |
| `signoff/msrv` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j msrv` | `MISE_LOCKED=1 mise x -- gh signoff msrv` |
| `signoff/npm-package` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j npm-package` | `MISE_LOCKED=1 mise x -- gh signoff npm-package` |
| `signoff/package` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j package` | `MISE_LOCKED=1 mise x -- gh signoff package` |
| `signoff/cargo-deny` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j cargo-deny` | `MISE_LOCKED=1 mise x -- gh signoff cargo-deny` |
| `signoff/semver` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j semver` | `MISE_LOCKED=1 mise x -- gh signoff semver` |
| `signoff/build-linux-x64` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/ci.yml -j build-pr --container-architecture linux/amd64` | `MISE_LOCKED=1 mise x -- gh signoff build-linux-x64` |
| `signoff/docs` | `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/docs.yml -j docs` | `MISE_LOCKED=1 mise x -- gh signoff docs` |

The partial name passed to `gh signoff` omits the `signoff/` prefix; the
extension adds that prefix when it creates the commit status.

Do not use local signoff for release-only guarantees that Linux Docker cannot
honestly provide. macOS and Windows platform builds stay on GitHub-hosted
runners until real local hosts exist. npm trusted publishing also stays
GitHub-hosted because its OIDC `id-token` flow depends on GitHub Actions.

## Preserve branch protection when requiring signoff

The PR merge gate should require these commit-status contexts on `main`:

- `signoff/test`
- `signoff/msrv`
- `signoff/npm-package`
- `signoff/package`
- `signoff/cargo-deny`
- `signoff/semver`
- `signoff/build-linux-x64`
- `signoff/docs`

Do **not** run `gh signoff install` or `gh signoff uninstall` on the real repo.
The current extension implementation rewrites branch protection during install
and deletes branch protection during uninstall. Preserve the existing branch
protection settings through the GitHub UI or the branch-protection API instead.

### UI update path

1. Open GitHub repository settings.
2. Go to **Branches** and edit the existing rule that protects `main`.
3. Under **Require status checks to pass before merging**, add the eight
   `signoff/...` contexts listed above.
4. Leave existing review, admin, linear-history, signed-commit, restriction,
   conversation-resolution, and other protection settings unchanged.
5. Save the rule and re-open it to confirm the required status checks include
   the signoff contexts alongside any existing GitHub Actions checks.

### API update path

Prefer the narrower required-status-checks endpoint over a full branch
protection replacement. Review the generated payload before sending the PATCH:

```sh
owner=stevennevins
repo=daml-tools
branch=main

gh api \
  "repos/${owner}/${repo}/branches/${branch}/protection/required_status_checks" \
  > required-status-checks.before.json

jq --argjson required '[
  "signoff/test",
  "signoff/msrv",
  "signoff/npm-package",
  "signoff/package",
  "signoff/cargo-deny",
  "signoff/semver",
  "signoff/build-linux-x64",
  "signoff/docs"
]' '
  {
    strict: (.strict // true),
    contexts: (((.contexts // []) + $required) | unique),
    checks: (.checks // [])
  }
' required-status-checks.before.json > required-status-checks.after.json

cat required-status-checks.after.json

# Run only after the reviewed payload preserves the existing status-check policy.
gh api \
  --method PATCH \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  "repos/${owner}/${repo}/branches/${branch}/protection/required_status_checks" \
  --input required-status-checks.after.json
```

After the update, verify the required contexts:

```sh
gh api \
  "repos/${owner}/${repo}/branches/${branch}/protection/required_status_checks" \
  --jq '.contexts'
```

If the narrow endpoint is not available because required status checks are not
enabled yet, use the UI path or prepare a full branch-protection payload from a
fresh GET response. Do not use `gh signoff install` as a shortcut.

## Trust model

gh-signoff is a maintainer assertion, not independent hosted CI. Any user with
write access and a suitable GitHub token can create successful `signoff/...`
commit statuses. Branch protection should therefore be paired with a review
policy that matches that trust level, for example requiring approval from
someone other than the PR author and keeping write access limited to trusted
maintainers.

## Runner image pins

`.actrc` maps these Linux labels only. The digests are manifest-list digests, so ordinary non-platform signoff jobs run with the host Docker architecture while still using the pinned image family:

| `runs-on` label | Image |
|-----------------|-------|
| `ubuntu-latest` | `catthehacker/ubuntu@sha256:5523ae08b8014721216e0e3a966e1b64b61b57382362282504ee59d27092a2d2` (`act-latest`) |
| `ubuntu-22.04` | `catthehacker/ubuntu@sha256:a1aa77d0719bf8f5c1b93b856bec6539d2b68102267a0dc27a4d8c01b6bc7e97` (`act-22.04`) |

macOS and Windows matrix jobs are not mapped in `.actrc`. Those platform builds stay on
GitHub-hosted runners until real macOS and Windows hosts exist for local signoff.

`signoff/build-linux-x64` is the exception: it must run as `linux/amd64`. Prefer a Linux `x86_64` host. ARM hosts may use Docker binfmt/QEMU with `--container-architecture linux/amd64`, but if emulation cannot run the pinned toolchain reliably, do not create the `signoff/build-linux-x64` status from that machine.

## Refresh runner image digests

When you intentionally want newer runner images:

1. Pull the target tag:

   ```sh
   docker pull catthehacker/ubuntu:act-latest
   docker pull catthehacker/ubuntu:act-22.04
   ```

2. Read the digest for each image:

   ```sh
   docker inspect catthehacker/ubuntu:act-latest --format='{{index .RepoDigests 0}}'
   docker inspect catthehacker/ubuntu:act-22.04 --format='{{index .RepoDigests 0}}'
   ```

3. Update the matching `-P ubuntu-…=catthehacker/ubuntu@sha256:…` lines in `.actrc`.

4. Re-run `MISE_LOCKED=1 mise x -- act -l` and at least one non-publishing job, such as `MISE_LOCKED=1 mise x -- act pull_request -W .github/workflows/docs.yml -j docs`, to confirm act still parses and executes workflows with the new pins.

Commit digest updates separately from unrelated CI changes so image drift is easy to review.

## Clear act caches

Act stores action checkouts, workflow caches, and artifact uploads under ignored `.act/` paths:

| Path | Purpose |
|------|---------|
| `.act/action-cache` | Cached actions and host workspaces |
| `.act/cache-server` | `actions/cache` server data |
| `.act/artifacts` | Artifact server uploads and downloads |

Remove all act runtime state for this repo:

```sh
rm -rf .act
```

Remove only one cache layer if you need a narrower reset, for example:

```sh
rm -rf .act/action-cache
```

After clearing caches, the next act run re-downloads actions and rebuilds cache entries.
