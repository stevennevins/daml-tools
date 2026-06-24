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
  "daml-lint-public-api",
  "parser-type-literals",
  "syntax-coordinate-newtypes",
  "format-options-api",
  "msrv-package-metadata",
  "unwrap-used-lint",
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
    id: "daml-lint-public-api",
    title: "daml-lint public API reshaping",
    objective:
      "Review and complete the broader daml-lint public API cleanup around diagnostic categories, parse result API shape, rule settings, and severity ordering.",
    primaryFiles: [
      "crates/daml-lint/src/lib.rs",
      "crates/daml-lint/src/parser.rs",
      "crates/daml-lint/src/detector.rs",
      "crates/daml-lint/src/config.rs",
      "crates/daml-lint/src/reporter.rs",
      "crates/daml-lint/README.md",
      "crates/daml-lint/lint-plugin",
    ],
    constraints: [
      "Read public exports and immediate internal/CLI/plugin callers before changing the API.",
      "Prefer clean breaks over compatibility shims; document any intentional breaking change in rustdoc/README where downstream callers need to know.",
      "Use Rust SemVer guidance when deciding whether #[non_exhaustive], enum variants, public fields, or ordering changes are acceptable in this pre-1.0 crate.",
      "Keep the slice focused on diagnostic categories, parse result API, rule settings, and severity ordering; do not redesign unrelated detectors.",
    ],
    validationHints: [
      "Add or update tests that encode why severity ordering and rule-setting parsing matter.",
      "Verify CLI/reporting/plugin type generation still compiles or clearly record any generated-file step that is needed.",
    ],
  },
  {
    id: "parser-type-literals",
    title: "Parser type literal modeling",
    objective:
      "Model Daml type literals beyond the current safe malformed-annotation diagnostics so callers get structured AST coverage instead of only diagnostics where possible.",
    primaryFiles: [
      "crates/daml-parser/src/ast.rs",
      "crates/daml-parser/src/parse.rs",
      "crates/daml-parser/src/diag_tests.rs",
      "crates/daml-parser/src/projection_tests.rs",
      "crates/daml-lint/src/parser.rs",
      "crates/daml-lint/src/ir.rs",
    ],
    constraints: [
      "Read existing type AST modeling and parser tests before adding grammar coverage.",
      "Do not broaden malformed annotation behavior accidentally; preserve diagnostics that intentionally protect unsupported syntax.",
      "Make the smallest AST/API addition that represents the type literal cases found during audit.",
    ],
    validationHints: [
      "Add parser tests that would fail if type literals collapse back to malformed-only diagnostics.",
      "Run parser and lint tests that exercise type projection into lint IR.",
    ],
  },
  {
    id: "syntax-coordinate-newtypes",
    title: "daml-syntax coordinate/domain newtypes",
    objective:
      "Introduce or complete domain-specific coordinate newtypes in daml-syntax so byte offsets, line/column positions, and source-domain identifiers are harder to mix up.",
    primaryFiles: [
      "crates/daml-syntax/src/lib.rs",
      "crates/daml-lint/src/ir.rs",
      "crates/daml-lint/src/parser.rs",
      "crates/daml-parser/src/ast_span.rs",
      "crates/daml-parser/src/span_tests.rs",
    ],
    constraints: [
      "Read all SourceFile, LineIndex, span, and coordinate exports/callers before editing.",
      "Prefer typed clean breaks over adapters or duplicate raw usize APIs unless a raw API is demonstrably needed internally.",
      "Keep conversions explicit at crate boundaries and avoid refactoring unrelated syntax representation.",
    ],
    validationHints: [
      "Add or update tests that catch mixed byte/line/column/domain usage.",
      "Compile all crates that currently consume daml-syntax span/coordinate APIs.",
    ],
  },
  {
    id: "format-options-api",
    title: "FormatOptions builder and non-exhaustive API decision",
    objective:
      "Review FormatOptions as a public API and either add a narrow builder/non_exhaustive design or document why the current exhaustive struct API is intentional.",
    primaryFiles: [
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-fmt/Cargo.toml",
      "crates/daml-fmt/tests/cli.rs",
      "README.md",
    ],
    constraints: [
      "Use Cargo SemVer guidance: adding #[non_exhaustive] to an existing public struct is a breaking change; if used, make the break explicit.",
      "Do not change formatter output intentionally.",
      "Keep the API ergonomic for simple callers; avoid a large builder abstraction if Default plus documented struct fields remains the simpler choice.",
    ],
    validationHints: [
      "Add tests or doctests for the intended FormatOptions construction style if the API changes.",
      "Run formatter tests and verify formatted snapshots/corpus output do not change unless explicitly justified.",
    ],
  },
  {
    id: "msrv-package-metadata",
    title: "Per-crate MSRV and package metadata audit",
    objective:
      "Audit each Rust crate's rust-version and package metadata against Cargo publishing guidance, then patch high-value gaps consistently.",
    primaryFiles: [
      "Cargo.toml",
      "crates/*/Cargo.toml",
      "crates/*/README.md",
      "README.md",
    ],
    constraints: [
      "Use Cargo manifest/workspace inheritance guidance for rust-version, license, repository, readme, keywords, categories, and package include/exclude decisions.",
      "Do not bump MSRV speculatively; if code requires the current workspace MSRV, record the evidence.",
      "Keep metadata consistent across crates but preserve legitimate crate-specific differences.",
    ],
    validationHints: [
      "Run cargo metadata and cargo package --list or --dry-run where practical for changed crates.",
      "Document any validation skipped due to time or external packaging/network constraints.",
    ],
  },
  {
    id: "unwrap-used-lint",
    title: "clippy unwrap_used readiness and lint policy",
    objective:
      "Audit whether the workspace is ready for clippy::unwrap_used = deny; enable it only if the resulting changes are surgical and validated, otherwise report the exact blockers.",
    primaryFiles: [
      "Cargo.toml",
      "clippy.toml",
      "crates/*/src/**/*.rs",
      "crates/*/tests/**/*.rs",
    ],
    constraints: [
      "Treat unwrap_used = deny as optional unless the codebase is ready; do not perform broad mechanical churn to force readiness.",
      "Prefer intent-preserving Result propagation, expect messages with invariant rationale, or test-only allowances where those match existing conventions.",
      "Do not hide panics with lossy fallbacks. Fail loud if an unwrap represents an unresolved design decision.",
    ],
    validationHints: [
      "Run cargo clippy --workspace --all-targets -- -D warnings with unwrap_used denied if enabled.",
      "Record representative remaining unwrap blockers if the lint is not enabled.",
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
  id: z.enum(followUpIds).default("daml-lint-public-api"),
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
        agent={agents.smart}
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
        agent={agents.smart}
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
