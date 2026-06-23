# Agent Config

These files export the configured agent instances used by Smithers workflows.

- `claude-code.ts`, `codex.ts`, `opencode.ts`, and `antigravity.ts` are repo-owned defaults.
- Edit them to pin models, add a shared `systemPrompt`, or enable engine-specific flags.
- Do **not** set `cwd` on agents that may run inside `<Worktree>`; Smithers supplies the launch root or isolated worktree root. A pinned `cwd` overrides worktree isolation.
- `index.ts` re-exports all four so root-level files can import from `./agents`.

Examples:

```ts
import { ClaudeCodeAgent } from "./agents";
import { CodexAgent } from "./agents/codex";
import { OpenCodeAgent } from "./agents/opencode";
import { AntigravityAgent } from "./agents/antigravity";
```

Inside `.smithers/workflows/*`, use `../agents` or `../agents/<name>` instead.

`smithers init` and `smithers init --agents-only` only create missing files in this directory.
Existing files here are left alone so custom agent config is preserved.
