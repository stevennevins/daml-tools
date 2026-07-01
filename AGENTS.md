# AGENTS.md

## Startup Check

At the start of each task, fetch `origin/main` and verify the worktree `HEAD`
is based on the current remote main before editing. If the checkout is clean and
behind or detached on an old commit, update it to `origin/main`; if local changes
or an intentional branch make that unsafe, stop and surface the mismatch before
writing files.

## CI Signoff

When asked to sign off on CI, first prove the checkout is attached to an open PR
and that local `HEAD` equals the PR head SHA. Signoff statuses only satisfy CI
when created for the latest pushed PR head. If there is no open PR, or if local
`HEAD` is stale or diverged from the PR head, stop before running expensive
signoff tasks and tell the user exactly what must be updated.

The local signoff flow requires the GitHub CLI extension:

```bash
gh extension install basecamp/gh-signoff
```

Use `mise run signoff:all` for the default signoff path. It preflights the PR
state, runs required `act` jobs sequentially, and creates `gh signoff` statuses
after local jobs pass. Use local-only or parallel signoff tasks only when the
user explicitly asks, and report that they do not by themselves prove PR CI
passed. Verify the PR-attached check rollup first; if no checks are attached to
the PR, say so explicitly instead of implying PR CI passed. Cite the exact commit
SHA and any GitHub Actions run or created signoff statuses in the final report.

## Smithers Improvement Feedback

When using Smithers in this repository, actively notice workflow, component,
agent-routing, prompt, validation, or observability improvements that would make
future runs safer or more effective. Surface those opportunities to the user in
plain language, and suggest concrete AGENTS.md, workflow, component, or prompt
changes instead of silently working around recurring friction.


## Smithers CLI Preflight

Before the first Smithers run in this repository, verify the CLI entrypoint and
local workflow dependencies. If `smithers` is not on `PATH`, invoke it as
`bunx smithers-orchestrator`. If workflow loading fails on missing Smithers
agent modules, run `bun install` in `.smithers/` once, then retry the Smithers
command.

## Smithers Worktree Preference

Codex sessions for this repository already run inside fresh git worktrees. Do
not wrap Smithers tasks in Smithers `<Worktree>` by default, because that hides
diffs from the user's editor. Use Smithers `<Worktree>` only when the user
explicitly asks for additional Smithers-managed isolation.
