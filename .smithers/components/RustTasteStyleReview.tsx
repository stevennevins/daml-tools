// smithers-source: local
/** @jsxImportSource smithers-orchestrator */
import { Sequence, Task, type AgentLike } from "smithers-orchestrator";
import { bashTool } from "smithers-orchestrator/tools";
import { z } from "zod/v4";
import { agents } from "../agents";

const commandResultSchema = z.object({
  command: z.string(),
  exitCode: z.number().int(),
  output: z.string(),
});

export const rustValidationOutputSchema = z.object({
  allPassed: z.boolean(),
  commands: z.array(commandResultSchema),
  failingSummary: z.string().nullable().default(null),
});

const rustTasteStyleIssueSchema = z.object({
  priority: z.enum(["P0", "P1", "P2", "P3"]),
  file: z.string().nullable().default(null),
  summary: z.string(),
  rationale: z.string(),
  suggestedFix: z.string().nullable().default(null),
});

export const rustDiffOutputSchema = z.object({ output: z.string() });

export const rustTasteStyleReviewOutputSchema = z.object({
  approved: z.boolean(),
  summary: z.string(),
  issues: z.array(rustTasteStyleIssueSchema).default([]),
  styleNotes: z.array(z.string()).default([]),
  deferredFollowUps: z.array(z.string()).default([]),
});

export type RustTasteStyleReviewProps = {
  idPrefix: string;
  context: string;
  commands: string[];
  reviewAgents?: AgentLike[];
  diffCommand?: string;
};

const STATUS_MARKER = "__SMITHERS_STATUS__";

async function runValidationCommand(command: string) {
  const output = await bashTool("bash", [
    "-lc",
    `set +e\n${command} 2>&1\nstatus=$?\nprintf '\\n${STATUS_MARKER}%s\\n' "$status"\nexit 0`,
  ]);
  const markerIndex = output.lastIndexOf(STATUS_MARKER);
  if (markerIndex === -1) {
    return { command, exitCode: 1, output };
  }

  const commandOutput = output.slice(0, markerIndex).trimEnd();
  const statusText = output.slice(markerIndex + STATUS_MARKER.length).trim();
  const exitCode = Number.parseInt(statusText, 10);
  return {
    command,
    exitCode: Number.isNaN(exitCode) ? 1 : exitCode,
    output: commandOutput,
  };
}

function reviewPrompt(
  context: string,
  validation: z.infer<typeof rustValidationOutputSchema>,
  diff: string,
) {
  return `Review this Rust change for taste, style, and type/API quality.

Use gpt-5.3-codex-spark judgment here: prefer readable, idiomatic Rust and small surgical feedback. Do not edit files.

Rubric:
- API/type shape: avoid stringly APIs, avoid boolean blindness, prefer domain types/newtypes where useful, use Result/error types intentionally.
- Rust idiom: ownership/borrowing ergonomics, clear Option/Result/iterator usage, no needless clones/allocations/generic or async complexity.
- Package quality: feature boundaries, public dependency types, MSRV/rust-version implications.
- Tests/docs: tests encode why behavior matters; public panic/error/safety behavior is documented when relevant.
- Scope: flag unrelated refactors or speculative abstractions.

Context:
${context}

Deterministic validation:
\`\`\`json
${JSON.stringify(validation, null, 2)}
\`\`\`

Diff/context command output:
\`\`\`diff
${diff}
\`\`\`

Approve only if deterministic validation passed and the change is idiomatic, focused, and safe. Return JSON matching the schema.`;
}

export function RustTasteStyleReview({
  idPrefix,
  context,
  commands,
  reviewAgents = agents.smartTool,
  diffCommand = "git diff --stat && git diff -- . ':!.smithers/node_modules' ':!node_modules'",
}: RustTasteStyleReviewProps) {
  const validationId = `${idPrefix}:deterministic-validation`;
  const diffId = `${idPrefix}:diff`;

  return (
    <Sequence>
      <Task id={validationId} output={rustValidationOutputSchema}>
        {async () => {
          const results = [];
          for (const command of commands) {
            results.push(await runValidationCommand(command));
          }
          const failed = results.filter((result) => result.exitCode !== 0);
          return {
            allPassed: failed.length === 0,
            commands: results,
            failingSummary:
              failed.length === 0
                ? null
                : failed
                    .map(
                      (result) => `${result.command} exited ${result.exitCode}`,
                    )
                    .join("; "),
          };
        }}
      </Task>
      <Task id={diffId} output={rustDiffOutputSchema}>
        {async () => ({ output: await bashTool("bash", ["-lc", diffCommand]) })}
      </Task>
      <Task
        id={`${idPrefix}:taste-style-review`}
        output={rustTasteStyleReviewOutputSchema}
        agent={reviewAgents}
        needs={{ validation: validationId, diff: diffId }}
        deps={{
          validation: rustValidationOutputSchema,
          diff: rustDiffOutputSchema,
        }}
        timeoutMs={1_200_000}
        heartbeatTimeoutMs={600_000}
      >
        {(deps) => reviewPrompt(context, deps.validation, deps.diff.output)}
      </Task>
    </Sequence>
  );
}
