import { BaseCliAgent, pushFlag, type BaseCliAgentOptions } from "@smithers-orchestrator/agents/BaseCliAgent";

type CursorAgentOptions = BaseCliAgentOptions & {
  binary?: string;
  mode?: "ask" | "plan";
  trust?: boolean;
  sandbox?: "enabled" | "disabled";
};

/**
 * Local Cursor CLI agent wrapper.
 *
 * The Cursor CLI is installed as `agent` on this machine. Keep cwd unset on the
 * provider instance and pass Smithers' task root via `--workspace` so normal
 * runs and Worktree-scoped runs execute in the correct checkout.
 */
export class CursorAgent extends BaseCliAgent {
  readonly opts: CursorAgentOptions;
  readonly cliEngine = "cursor";

  constructor(opts: CursorAgentOptions = {}) {
    super(opts);
    this.opts = opts;
    this.capabilities = {
      version: 1,
      engine: "cursor",
      runtimeTools: {},
      mcp: {
        bootstrap: "project-config",
        supportsProjectScope: true,
        supportsUserScope: true,
      },
      skills: {
        supportsSkills: false,
        smithersSkillIds: [],
      },
      humanInteraction: {
        supportsUiRequests: false,
        methods: [],
      },
      builtIns: ["read", "write", "edit", "bash", "grep", "list"],
    };
  }

  async buildCommand(params: { prompt: string; systemPrompt?: string; cwd: string; options: any }) {
    const resumeSession =
      typeof params.options?.resumeSession === "string" ? params.options.resumeSession : undefined;
    const args = ["--print", "--output-format", "json"];

    pushFlag(args, "--model", this.opts.model ?? this.model);
    pushFlag(args, "--workspace", params.cwd);
    pushFlag(args, "--mode", this.opts.mode);
    pushFlag(args, "--sandbox", this.opts.sandbox);

    if (this.opts.trust ?? true) {
      args.push("--trust");
    }

    if (this.opts.yolo ?? this.yolo) {
      args.push("--force");
    }

    if (resumeSession) {
      args.push("--resume", resumeSession);
    }

    if (this.extraArgs?.length) {
      args.push(...this.extraArgs);
    }

    const systemPrefix = params.systemPrompt ? `${params.systemPrompt}\n\n` : "";
    const fullPrompt = `${systemPrefix}${params.prompt ?? ""}`;
    if (fullPrompt) {
      args.push(fullPrompt);
    }

    return {
      command: this.opts.binary ?? "agent",
      args,
      outputFormat: "json",
      stdoutErrorPatterns: [/^error:/im, /^fatal:/im],
    };
  }
}

export const CursorAgentProvider = new CursorAgent({
  // Keep model unset by default so Cursor uses the user's configured default.
  sandbox: "disabled",
});
