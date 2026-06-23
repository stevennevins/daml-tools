// smithers-source: generated
// Account providers (camelCase labels) come from ~/.smithers/accounts.json — managed via `smithers agent add|list|remove`.
import { homedir } from "node:os";
import path from "node:path";
import { type AgentLike } from "smithers-orchestrator";
import { CodexAgent, SubscriptionCodexAgent } from "./agents/codex";

export { ClaudeCodeAgent } from "./agents/claude-code";
export { CodexAgent, SubscriptionCodexAgent } from "./agents/codex";

const codexConfigDir = path.join(homedir(), ".codex");

export const providers = {
  codexPlanner: new SubscriptionCodexAgent({ model: "gpt-5.5", configDir: codexConfigDir, skipGitRepoCheck: true }),
  codexSpark: new SubscriptionCodexAgent({ model: "gpt-5.3-codex-spark", configDir: codexConfigDir, skipGitRepoCheck: true }),
  codexDefault: CodexAgent,
} as const;

export const agents = {
  cheapFast: [providers.codexSpark],
  smart: [providers.codexPlanner],
  smartTool: [providers.codexSpark],
} as const satisfies Record<string, AgentLike[]>;
