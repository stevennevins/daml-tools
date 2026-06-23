import { AntigravityAgent as SmithersAntigravityAgent } from "smithers-orchestrator";

// Built-in Antigravity CLI agent (cliEngine: "antigravity").
// Do not pin cwd here; Smithers supplies the launch root or Worktree root.
export const AntigravityAgent = new SmithersAntigravityAgent({
  // model: "Gemini 3.1 Pro (high)",
  // systemPrompt: "Add shared instructions for every Antigravity run.",
  // dangerouslySkipPermissions: true,
  // allowedTools: ["read_file", "write_file"],
});
