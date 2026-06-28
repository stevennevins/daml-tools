# Run CI locally with mise, act, and gh-signoff

Use [mise](https://mise.jdx.dev/) for pinned tool versions and [act](https://github.com/nektos/act)
to run GitHub Actions workflows on Linux. The repo `.actrc` maps Linux runner labels to
digest-pinned, multi-architecture Docker images and keeps act runtime state under ignored `.act/` paths so caches and
artifacts do not get committed. We intentionally avoid `.git/act/` because this repo is often used from git worktrees where `.git` is a file, not a directory.

GitHub Actions YAML remains the source of truth. Local signoff runs the same
workflow jobs through act instead of a bespoke CI wrapper. The `signoff:*` mise
tasks in [`mise.toml`](../../mise.toml) are thin wrappers around
`act workflow_dispatch -W ... -j ...` plus the matching `gh signoff` status; they
do not duplicate CI logic. Use `MISE_LOCKED=1` locally, matching the GitHub
workflows, so drift from `mise.toml` and `mise.lock` fails loudly.

The CI and Docs workflows intentionally do not run on GitHub-hosted
`pull_request` or `push` events. They keep `workflow_dispatch` triggers so act
can execute the same YAML locally for PR signoff without spending hosted runner
minutes on every PR update or every merge to `main`. They also expose
`workflow_call` so the nightly release workflow can run the same jobs on
GitHub-hosted runners before release-plz publishes crates or updates release
PRs. Local `workflow_dispatch` uses the Linux x64 signoff smoke by default; the
nightly release gate passes `run-release-builds: true` so hosted runners also
exercise the full release build matrix.

Install tools from the committed lockfile before running signoff tasks:

```sh
MISE_LOCKED=1 mise install
```

## Activate mise for interactive shells

Use `mise run signoff:*` for the documented PR signoff commands. For ad hoc
commands that need a pinned tool, use `mise x -- ...` so the toolchain is
explicit and shell-independent. For day-to-day interactive work, you can also
activate mise in your shell so pinned tools such as `cargo`, `node`, `act`, and
`gh` are placed on `PATH` automatically when you enter the repo.

Activate mise for the current shell session:

```sh
eval "$(mise activate zsh)"
```

For bash, fish, or PowerShell, replace `zsh` with the matching shell:

- bash:

  ```sh
  eval "$(mise activate bash)"
  ```

- fish:

  ```fish
  mise activate fish | source
  ```

- PowerShell:

  ```powershell
  (& mise activate pwsh) | Out-String | Invoke-Expression
  ```

To make activation permanent, add the matching command to your shell startup
file after `mise` is installed and available on `PATH`:

```sh
echo 'eval "$(mise activate zsh)"' >> ~/.zshrc
```

After activation, install the locked tools once per checkout and verify that
commands resolve through mise:

```sh
MISE_LOCKED=1 mise install
mise which cargo
mise which act
```

If mise asks whether to trust this repo, review `mise.toml` and `mise.lock`
before running `mise trust`.

List workflows act can run:

```sh
MISE_LOCKED=1 mise x -- act -l
```

## Sign off on PR gates locally

Install the gh-signoff extension once for the GitHub CLI managed by mise:

```sh
MISE_LOCKED=1 mise x -- gh extension install basecamp/gh-signoff
```

Before signing off, make sure the checkout is clean and the current commit is
pushed to the PR branch. Each `mise run signoff:*` task first runs the matching
act job and only then creates a GitHub commit status for `HEAD`; do not sign off
for a job that failed, was skipped, or was run with a different toolchain.

Run one local task per required signoff context. The npm packaging job validates
the current Linux container platform (`linux-x64` on x86_64 hosts or
`linux-arm64` on ARM hosts); release workflows still produce the full
cross-platform set.

| Required PR context | Run this task | Underlying act job |
|---------------------|---------------|--------------------|
| `signoff/test` | `MISE_LOCKED=1 mise run signoff:test` | `.github/workflows/ci.yml` job `test` |
| `signoff/npm-package` | `MISE_LOCKED=1 mise run signoff:npm-package` | `.github/workflows/ci.yml` job `npm-package` |
| `signoff/package` | `MISE_LOCKED=1 mise run signoff:package` | `.github/workflows/ci.yml` job `package` |
| `signoff/cargo-deny` | `MISE_LOCKED=1 mise run signoff:cargo-deny` | `.github/workflows/ci.yml` job `cargo-deny` |
| `signoff/semver` | `MISE_LOCKED=1 mise run signoff:semver` | `.github/workflows/ci.yml` job `semver` |
| `signoff/build-linux-x64` | `MISE_LOCKED=1 mise run signoff:build-linux-x64` | `.github/workflows/ci.yml` job `build-linux-x64` |
| `signoff/docs` | `MISE_LOCKED=1 mise run signoff:docs` | `.github/workflows/docs.yml` job `docs` |

To run every required signoff task in sequence:

```sh
MISE_LOCKED=1 mise run signoff:all
```

The package verification job runs `git diff` to reject dirty packages. When act
runs from a git worktree, the `signoff:package` task mounts the git common
directory into the container so that the worktree `.git` file resolves
correctly.

The signoff tasks pass partial names to `gh signoff` without the `signoff/`
prefix; the extension adds that prefix when it creates the commit status.

The `signoff:package` task uses Docker `--mount` rather than `--volume` for the
git common directory so a missing host path fails loudly instead of creating an
empty directory.

Do not use local signoff for release-only guarantees that Linux Docker cannot
honestly provide. macOS and Windows platform builds stay on GitHub-hosted
runners until real local hosts exist. npm trusted publishing also stays
GitHub-hosted because its OIDC `id-token` flow depends on GitHub Actions.

## Preserve branch protection when requiring signoff

The PR merge gate should require these commit-status contexts on `main`:

- `signoff/test`
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
3. Under **Require status checks to pass before merging**, add the seven
   `signoff/...` contexts listed above.
4. Leave existing review, admin, linear-history, signed-commit, restriction,
   conversation-resolution, and other protection settings unchanged.
5. Save the rule and re-open it to confirm the required status checks include
   the signoff contexts alongside any existing GitHub Actions checks.

### API update path

Prefer the required-status-check contexts endpoint over a full branch-protection
replacement. This preserves existing app-specific GitHub Actions checks while
adding generic commit-status contexts created by gh-signoff.

```sh
owner=stevennevins
repo=daml-tools
branch=main

gh api \
  "repos/${owner}/${repo}/branches/${branch}/protection/required_status_checks" \
  > required-status-checks.before.json

gh api \
  --method POST \
  -H "Accept: application/vnd.github+json" \
  -H "X-GitHub-Api-Version: 2022-11-28" \
  "repos/${owner}/${repo}/branches/${branch}/protection/required_status_checks/contexts" \
  -f "contexts[]=signoff/test" \
  -f "contexts[]=signoff/npm-package" \
  -f "contexts[]=signoff/package" \
  -f "contexts[]=signoff/cargo-deny" \
  -f "contexts[]=signoff/semver" \
  -f "contexts[]=signoff/build-linux-x64" \
  -f "contexts[]=signoff/docs"
```

After the update, verify the required contexts:

```sh
gh api \
  "repos/${owner}/${repo}/branches/${branch}/protection/required_status_checks" \
  --jq '.contexts'
```

The signoff contexts should also appear in `.checks` with `"app_id": null`.
If required status checks are not enabled yet, use the UI path or prepare a full
branch-protection payload from a fresh GET response. Do not use
`gh signoff install` as a shortcut.

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

`signoff/build-linux-x64` compiles the Linux x64 smoke binary. On Linux
`x86_64` hosts it builds natively. On ARM hosts, run the job with the normal
host-architecture act container; the workflow installs the Linux x64 Rust target
and linker instead of relying on Docker/QEMU amd64 emulation for the whole job.

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

4. Re-run `MISE_LOCKED=1 mise x -- act -l` and at least one non-publishing job, such as `MISE_LOCKED=1 mise x -- act workflow_dispatch -W .github/workflows/docs.yml -j docs`, to confirm act still parses and executes workflows with the new pins. Use raw `act` here because this is a runner-image smoke, not a PR signoff that should create a `signoff/...` status.

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
