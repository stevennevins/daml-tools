# Smithers workflow pack

This directory is tracked intentionally so Git worktrees receive the same Smithers workflows, prompts, components, and repo-level configuration as the main checkout.

## Worktree rules

- Leave agent `cwd` unset. Smithers resolves task working directories as `agent.cwd ?? worktreePath ?? repoRoot`; pinning `cwd` makes agents read and write the launch checkout instead of the isolated `<Worktree>`.
- Use `ctx.worktreePath(...)` or `ctx.resolveWorktreePath(...)` in workflow helpers that need a worktree path. Do not reconstruct worktree paths from `process.cwd()`, `import.meta.dir`, or relative `../..` guesses.
- Prefer absolute `<Worktree path>` values when a deterministic location matters. Relative paths resolve against the Smithers launch root.
- Runtime state, logs, databases, sandboxes, and dependencies are local-only and ignored by `.smithers/.gitignore`.

## Local setup

Run Smithers through the project-pinned dependency:

```bash
bunx smithers-orchestrator workflow list
```

If dependencies are missing in a fresh worktree, install them inside this directory:

```bash
(cd .smithers && bun install)
```
