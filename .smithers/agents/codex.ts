import { existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { CodexAgent as SmithersCodexAgent } from "smithers-orchestrator";

type CodexOptions = ConstructorParameters<typeof SmithersCodexAgent>[0];

class SubscriptionCodexAgent extends SmithersCodexAgent {
  private readonly configDirForAuth: string;
  private readonly apiKeyForAuth?: string;

  constructor(opts: CodexOptions = {}) {
    super(opts);
    this.configDirForAuth = opts.configDir ?? path.join(homedir(), ".codex");
    this.apiKeyForAuth = opts.apiKey;
  }

  async preflight(options?: Parameters<SmithersCodexAgent["preflight"]>[0]) {
    const hasApiKey = Boolean(this.apiKeyForAuth ?? process.env.OPENAI_API_KEY);
    const hasSubscriptionAuth = existsSync(path.join(this.configDirForAuth, "auth.json"));

    if (!hasApiKey && hasSubscriptionAuth) {
      return;
    }

    return super.preflight(options);
  }
}

// Built-in Codex CLI agent (cliEngine: "codex").
// Uses local Codex subscription auth via ~/.codex/auth.json.
// Do not pin cwd here; Smithers supplies the launch root or Worktree root.
export const CodexAgent = new SubscriptionCodexAgent({
  model: "gpt-5.3-codex-spark",
  configDir: path.join(homedir(), ".codex"),
  skipGitRepoCheck: true,
});

export { SubscriptionCodexAgent };
