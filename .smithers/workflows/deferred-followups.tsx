// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: Daml Deferred Follow-ups
// smithers-description: Implement the deferred Rust API cleanup follow-ups as focused, validated slices.
// smithers-tags: daml, rust, cleanup, follow-up
/** @jsxImportSource smithers-orchestrator */
import { createSmithers } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const followUpIds = [
  "line-index-coupling",
  "public-ir-non-exhaustive",
  "parser-bool-flags",
  "formatter-bool-tuple-cleanup",
  "must-use-error-docs",
] as const;

type FollowUpId = typeof followUpIds[number];

type FollowUp = {
  id: FollowUpId;
  title: string;
  objective: string;
  primaryFiles: string[];
  constraints: string[];
  validationHints: string[];
};

const followUps: FollowUp[] = [
  {
    id: "line-index-coupling",
    title: "LineIndex source/index coupling redesign",
    objective:
      "Redesign the SourceFile/LineIndex relationship so callers cannot accidentally compute locations with a LineIndex built from different source text.",
    primaryFiles: [
      "crates/daml-syntax/src",
      "crates/daml-lint/src/ir.rs",
      "crates/daml-lint/src/parser.rs",
    ],
    constraints: [
      "Read SourceFile and LineIndex exports, constructors, and immediate callers before editing.",
      "Prefer a clean break over compatibility shims; make mismatched source/index usage impossible or visibly hard.",
      "Keep the redesign narrow; do not refactor unrelated syntax or lint IR code.",
    ],
    validationHints: [
      "Add or update tests that would fail if spans were resolved with a mismatched source/index pair.",
      "Check all callers that currently pass SourceFile and LineIndex separately.",
    ],
  },
  {
    id: "public-ir-non-exhaustive",
    title: "Public IR #[non_exhaustive] decision",
    objective:
      "Audit the public daml-lint IR surface and either apply #[non_exhaustive] consistently where forward extension is intended or document why specific types stay exhaustive.",
    primaryFiles: ["crates/daml-lint/src/ir.rs", "crates/daml-lint/src/lib.rs"],
    constraints: [
      "Make an explicit decision rather than copying attributes mechanically.",
      "Use a clean break if public pattern matching must change; do not add compatibility adapters.",
      "Update rustdoc/module docs so downstream users know the intended matching contract.",
    ],
    validationHints: [
      "Compile public examples/tests after the decision.",
      "Look for exported enums/structs that encode parsed Daml IR and may grow over time.",
    ],
  },
  {
    id: "parser-bool-flags",
    title: "Parser bool-flag cleanup",
    objective:
      "Replace ambiguous private parser boolean flags with named intent-bearing types or small option structs where that improves readability.",
    primaryFiles: ["crates/daml-parser/src/parse.rs"],
    constraints: [
      "Target private flags such as parsing mode booleans; leave simple predicate returns alone.",
      "Do not broaden parser behavior or grammar support while cleaning names.",
      "Keep changes local to parser control-flow readability unless immediate tests/callers require updates.",
    ],
    validationHints: [
      "Parser behavior tests should remain unchanged except for added regression coverage around any touched mode flag.",
      "Prefer enums/newtypes with names that make call sites self-documenting.",
    ],
  },
  {
    id: "formatter-bool-tuple-cleanup",
    title: "Formatter private bool/tuple cleanup",
    objective:
      "Replace private formatter boolean flags and opaque tuple records with named structs/enums where the tuple or flag meaning is currently unclear.",
    primaryFiles: ["crates/daml-fmt/src/lib.rs", "crates/daml-fmt/src/layout_ast.rs"],
    constraints: [
      "Focus on private implementation clarity; do not alter formatted output intentionally.",
      "Avoid speculative formatter architecture changes.",
      "Use named fields for tuple elements that represent token/layout state.",
    ],
    validationHints: [
      "Run formatter snapshot/corpus tests and confirm expected output files do not change unless a real bug is intentionally fixed.",
      "Add a focused test only if the cleanup exposes a previously unencoded invariant.",
    ],
  },
  {
    id: "must-use-error-docs",
    title: "Broader #[must_use]/error-doc pass",
    objective:
      "Audit public APIs across the workspace for missing #[must_use] attributes and incomplete error documentation, then patch the highest-value gaps.",
    primaryFiles: [
      "crates/daml-parser/src",
      "crates/daml-syntax/src",
      "crates/daml-lint/src",
      "crates/daml-fmt/src",
    ],
    constraints: [
      "Prioritize public constructors, accessors, analysis results, parse/format/lint outputs, and error-returning APIs.",
      "Do not churn private helpers or add ceremonial attributes where ignoring the value is harmless.",
      "Keep rustdoc concise and accurate; document when functions can error instead of restating signatures.",
    ],
    validationHints: [
      "Run cargo doc with workspace rustdoc lints enabled.",
      "Add doc tests only when they clarify public usage and are cheap to maintain.",
    ],
  },
];

const scopeOutputSchema = z.looseObject({
  summary: z.string().default("Scoped deferred follow-ups."),
  assumptions: z.array(z.string()).default([]),
  executionNotes: z.array(z.string()).default([]),
  risks: z.array(z.string()).default([]),
  validationCommands: z.array(z.string()).default([]),
});

const followUpResultSchema = z.looseObject({
  id: z.enum(followUpIds).default("line-index-coupling"),
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("Follow-up task completed."),
  filesChanged: z.array(z.string()).default([]),
  testsAddedOrUpdated: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  blockers: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
});

const validationOutputSchema = z.looseObject({
  allPassed: z.boolean().default(false),
  summary: z.string().default("Validation completed."),
  commandsRun: z.array(z.object({
    command: z.string(),
    status: z.enum(["passed", "failed", "skipped"]),
    details: z.string().nullable().default(null),
  })).default([]),
  failures: z.array(z.string()).default([]),
  skipped: z.array(z.string()).default([]),
});

const finalOutputSchema = z.looseObject({
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("Deferred follow-ups workflow finished."),
  completed: z.array(z.string()).default([]),
  blocked: z.array(z.string()).default([]),
  validationPassed: z.boolean().default(false),
  nextActions: z.array(z.string()).default([]),
  markdownBody: z.string().default(""),
});

const inputSchema = z.object({
  extraContext: z.string().default(""),
});

const { Workflow, Task, Sequence, smithers, outputs } = createSmithers({
  input: inputSchema,
  scope: scopeOutputSchema,
  followUp: followUpResultSchema,
  validation: validationOutputSchema,
  final: finalOutputSchema,
});

function taskId(followUp: FollowUp): string {
  return `deferred-followups:${followUp.id}`;
}

function priorTaskId(index: number): string {
  return index === 0 ? "deferred-followups:scope" : taskId(followUps[index - 1]);
}

function priorDeps(index: number) {
  return index === 0
    ? { prior: outputs.scope }
    : { prior: outputs.followUp };
}

function formatList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

function scopePrompt(extraContext: string): string {
  return `You are preparing a focused implementation run for deferred daml-tools follow-up work.

Behavior contract:
- Read AGENTS.md, README.md, Cargo.toml, and the relevant crate exports before changing files in later tasks.
- Bias toward small, surgical changes and clean breaks. Do not add backward-compatibility shims unless the user explicitly asks.
- Surface uncertainty and blockers loudly. If a task becomes ambiguous or irreversible, use smithers ask-human rather than guessing.
- Do not push or open a PR from inside this workflow.

Deferred follow-ups to execute in this order:
${followUps.map((item, index) => `${index + 1}. ${item.title} (${item.id})`).join("\n")}

Validation baseline to preserve:
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets -- -D warnings
- cargo test --workspace
- cargo doc --workspace --no-deps

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return scope notes, assumptions, risks, and any execution notes that each later task should preserve.`;
}

function followUpPrompt(followUp: FollowUp, index: number, extraContext: string, prior: unknown): string {
  const remaining = followUps.slice(index + 1).map((item) => `${item.id}: ${item.title}`);
  return `Implement one deferred follow-up in daml-tools.

Assigned follow-up: ${followUp.title}
ID: ${followUp.id}
Objective: ${followUp.objective}

Primary files/directories to inspect first:
${formatList(followUp.primaryFiles)}

Constraints:
${formatList(followUp.constraints)}

Validation intent:
${formatList(followUp.validationHints)}

Prior workflow output to preserve:
${JSON.stringify(prior, null, 2)}

Remaining follow-ups after this task:
${remaining.length > 0 ? formatList(remaining) : "- none"}

Execution rules:
- Touch only files needed for this assigned follow-up and immediate tests/docs.
- Use deterministic tools for search, formatting, and tests; do not use model judgment for routing or mechanical transforms.
- Commit a small focused increment if this task reaches a working state, following repo Conventional Commit rules.
- If this task cannot be completed safely, report blockers and leave a clear handoff for later tasks.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return the exact files changed, tests/docs updated, commands run, blockers, and any follow-ups.`;
}

function validationPrompt(results: unknown[], extraContext: string): string {
  return `Validate the completed deferred follow-up work.

Task results:
${JSON.stringify(results, null, 2)}

Run these checks unless the environment makes one impossible; never silently skip:
1. cargo fmt --all -- --check
2. cargo clippy --workspace --all-targets -- -D warnings
3. cargo test --workspace
4. cargo doc --workspace --no-deps

If any check fails, diagnose whether the failure belongs to the workflow changes or pre-existing/environmental state. Fix only issues caused by this workflow's changes. If a check is skipped, record the exact reason.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return all commands, pass/fail/skipped status, failures, and a concise validation summary.`;
}

function finalPrompt(results: unknown[], validation: unknown): string {
  return `Synthesize the final status for the deferred follow-ups workflow.

Follow-up results:
${JSON.stringify(results, null, 2)}

Validation result:
${JSON.stringify(validation, null, 2)}

Return:
- overall status: completed, partial, or blocked
- completed and blocked follow-up IDs
- whether validation passed
- remaining risks and next actions
- a Markdown body suitable for a PR or handoff note.`;
}

function allFollowUpNeeds(): Record<string, string> {
  return Object.fromEntries(followUps.map((followUp) => [followUp.id, taskId(followUp)]));
}

function allFollowUpDeps(): Record<string, typeof followUpResultSchema> {
  return Object.fromEntries(followUps.map((followUp) => [followUp.id, outputs.followUp]));
}

export default smithers((ctx) => (
  <Workflow name="deferred-followups">
    <Sequence>
      <Task
        id="deferred-followups:scope"
        output={outputs.scope}
        agent={agents.smartTool}
        timeoutMs={1_800_000}
        heartbeatTimeoutMs={600_000}
      >
        {scopePrompt(ctx.input.extraContext ?? "")}
      </Task>

      {followUps.map((followUp, index) => (
        <Task
          key={followUp.id}
          id={taskId(followUp)}
          output={outputs.followUp}
          agent={agents.smartTool}
          needs={{ prior: priorTaskId(index) }}
          deps={priorDeps(index)}
          timeoutMs={3_600_000}
          heartbeatTimeoutMs={900_000}
          continueOnFail
        >
          {(deps: any) => followUpPrompt(
            followUp,
            index,
            ctx.input.extraContext ?? "",
            deps.prior
          )}
        </Task>
      ))}

      <Task
        id="deferred-followups:validate"
        output={outputs.validation}
        agent={agents.smartTool}
        needs={allFollowUpNeeds()}
        deps={allFollowUpDeps()}
        timeoutMs={3_600_000}
        heartbeatTimeoutMs={900_000}
      >
        {(deps: Record<string, unknown>) => validationPrompt(
          followUps.map((followUp) => deps[followUp.id]),
          ctx.input.extraContext ?? "",
        )}
      </Task>

      <Task
        id="deferred-followups:final"
        output={outputs.final}
        agent={agents.smart}
        needs={{ validation: "deferred-followups:validate", ...allFollowUpNeeds() }}
        deps={{ validation: outputs.validation, ...allFollowUpDeps() }}
        timeoutMs={1_800_000}
        heartbeatTimeoutMs={600_000}
      >
        {(deps: Record<string, unknown>) => finalPrompt(
          followUps.map((followUp) => deps[followUp.id]),
          deps.validation,
        )}
      </Task>
    </Sequence>
  </Workflow>
));
