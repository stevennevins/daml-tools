# AGENTS.md

## Startup Check

At the start of each task, fetch `origin/main` and verify the worktree `HEAD`
is based on the current remote main before editing. If the checkout is clean and
behind or detached on an old commit, update it to `origin/main`; if local changes
or an intentional branch make that unsafe, stop and surface the mismatch before
writing files.

## Smithers Improvement Feedback

When using Smithers in this repository, actively notice workflow, component,
agent-routing, prompt, validation, or observability improvements that would make
future runs safer or more effective. Surface those opportunities to the user in
plain language, and suggest concrete AGENTS.md, workflow, component, or prompt
changes instead of silently working around recurring friction.

## Smithers Worktree Preference

Codex sessions for this repository already run inside fresh git worktrees. Do
not wrap Smithers tasks in Smithers `<Worktree>` by default, because that hides
diffs from the user's editor. Use Smithers `<Worktree>` only when the user
explicitly asks for additional Smithers-managed isolation.
