import { OpenCodeAgent as SmithersOpenCodeAgent } from "smithers-orchestrator";

// Built-in OpenCode CLI agent (cliEngine: "opencode").
// Do not pin cwd here; Smithers supplies the launch root or Worktree root.
export const OpenCodeAgent = new SmithersOpenCodeAgent({
  model: "anthropic/claude-fable-5",
  // agentName: "build",
  // systemPrompt: "Add shared instructions for every OpenCode run.",
  // yolo: true,
});
