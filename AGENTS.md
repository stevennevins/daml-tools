# AGENTS.md

## Startup Check

At the start of each task, fetch `origin/main` and verify the worktree `HEAD`
is based on the current remote main before editing. If the checkout is clean and
behind or detached on an old commit, update it to `origin/main`; if local changes
or an intentional branch make that unsafe, stop and surface the mismatch before
writing files.
