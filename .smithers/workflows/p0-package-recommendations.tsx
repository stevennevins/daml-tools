// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: P0 Package Recommendations
// smithers-description: Implement the P0 package test-safety recommendations with sequential validation gates.
// smithers-tags: daml, rust, testing, p0
/** @jsxImportSource smithers-orchestrator */
import { createSmithers } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const workItemIds = [
  "daml-parser-corpus-fail-loud",
  "daml-syntax-finance-corpus-surface",
  "daml-syntax-trybuild-api-shape",
  "daml-fmt-gap-cases-fail-loud",
] as const;

type WorkItemId = typeof workItemIds[number];
type RiskLevel = "low" | "medium";

type WorkItem = {
  id: WorkItemId;
  packageName: "daml-parser" | "daml-syntax" | "daml-fmt";
  title: string;
  objective: string;
  primaryFiles: string[];
  acceptanceCriteria: string[];
  constraints: string[];
  validationCommands: string[];
  riskLevel: RiskLevel;
};

const workItems: WorkItem[] = [
  {
    id: "daml-parser-corpus-fail-loud",
    packageName: "daml-parser",
    title: "Make parser corpus presence fail loud consistently in CI",
    objective:
      "Ensure daml-parser corpus/oracle tests fail loudly when the shared corpus is expected but unavailable in CI or a workspace checkout, while preserving legitimate published-crate skips.",
    primaryFiles: [
      "crates/daml-parser/src/layout.rs",
      "crates/daml-parser/tests/span_losslessness.rs",
      "corpus/daml-finance/",
    ],
    acceptanceCriteria: [
      "Corpus/oracle coverage does not silently pass in CI when corpus/daml-finance is absent.",
      "The check remains a semantic corpus invariant/oracle test, not a snapshot test.",
      "Published-crate or non-workspace scenarios skip only when the corpus is legitimately unavailable.",
    ],
    constraints: [
      "Do not move the shared corpus.",
      "Do not weaken existing parser span, trivia, or layout assertions.",
      "Avoid creating shared helpers unless real reuse already exists or emerges in this item.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-parser --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-syntax-finance-corpus-surface",
    packageName: "daml-syntax",
    title: "Add real daml-finance corpus surface test for daml-syntax",
    objective:
      "Add an integration test over corpus/daml-finance that exercises SourceFile::parse and public syntax surface invariants.",
    primaryFiles: [
      "crates/daml-syntax/src/lib.rs",
      "crates/daml-syntax/tests/",
      "corpus/daml-finance/",
    ],
    acceptanceCriteria: [
      "A new or expanded integration test parses known-clean daml-finance sources with SourceFile::parse.",
      "The test checks token/trivia losslessness, range bounds, and no diagnostics for the known-clean corpus.",
      "Missing corpus behavior follows the shared policy: fail loudly in CI/workspace, skip only when legitimately unavailable outside the workspace.",
    ],
    constraints: [
      "Do not move the shared corpus.",
      "Do not snapshot full ASTs, modules, or real corpora.",
      "Do not change SourceFile, token, coordinate, or diagnostic public behavior except to fix a discovered bug required by the test.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-syntax --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-syntax-trybuild-api-shape",
    packageName: "daml-syntax",
    title: "Expand daml-syntax trybuild API-shape tests",
    objective:
      "Add compile-fail/UI coverage for daml-syntax public API construction boundaries, private fields, and non-exhaustive matching behavior.",
    primaryFiles: [
      "crates/daml-syntax/tests/compile_fail/",
      "crates/daml-syntax/tests/",
      "crates/daml-syntax/src/",
    ],
    acceptanceCriteria: [
      "Trybuild cases cover private field construction boundaries for public types where applicable.",
      "Trybuild cases cover #[non_exhaustive] matching or construction boundaries where applicable.",
      "Any .stderr golden updates are intentional, minimal, and explained in the task output.",
    ],
    constraints: [
      "Do not broaden public constructors or expose fields only for tests.",
      "Do not churn unrelated .stderr files.",
      "Keep compile-fail tests focused on API shape, not broad runtime behavior.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-syntax --test compile_fail --locked",
      "cargo test -p daml-syntax --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-fmt-gap-cases-fail-loud",
    packageName: "daml-fmt",
    title: "Make gap-case formatter fixtures fail loud when absent",
    objective:
      "Ensure gap_cases_format_to_expected_output fails loudly in CI/workspace checkouts when its fixtures are absent.",
    primaryFiles: [
      "crates/daml-fmt/tests/library_behavior.rs",
      "crates/daml-fmt/tests/fixtures/",
      "crates/daml-fmt/corpus/",
    ],
    acceptanceCriteria: [
      "Fixture absence is reported as a hard failure when the repository checkout expects the fixtures.",
      "The test remains a fixture availability safety check, not a snapshot migration.",
      "Legitimate published-crate or non-workspace skips remain explicit and documented in the assertion path.",
    ],
    constraints: [
      "Do not migrate the formatter corpus wholesale to snapshots or insta.",
      "Do not move large formatter compiler fixtures as part of this item.",
      "Do not alter formatter output semantics.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked gap_cases_format_to_expected_output",
      "cargo test -p daml-fmt --all-features --locked",
    ],
    riskLevel: "low",
  },
];

const scopeOutputSchema = z.looseObject({
  summary: z.string().default("P0 package recommendations scoped."),
  assumptions: z.array(z.string()).default([]),
  plannedOrder: z.array(z.enum(workItemIds)).default([]),
  risks: z.array(z.string()).default([]),
  outOfScope: z.array(z.string()).default([]),
});

const itemResultSchema = z.looseObject({
  id: z.enum(workItemIds).default("daml-parser-corpus-fail-loud"),
  packageName: z.string().default("daml-parser"),
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("Work item completed."),
  filesChanged: z.array(z.string()).default([]),
  testsAddedOrUpdated: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  blockers: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
  commit: z.string().nullable().default(null),
});

const validationSchema = z.looseObject({
  id: z.enum(workItemIds).default("daml-parser-corpus-fail-loud"),
  packageName: z.string().default("daml-parser"),
  allPassed: z.boolean().default(false),
  workingTreeClean: z.boolean().default(false),
  summary: z.string().default("Validation completed."),
  commandsRun: z.array(z.object({
    command: z.string(),
    status: z.enum(["passed", "failed", "skipped"]),
    details: z.string().nullable().default(null),
  })).default([]),
  fixesApplied: z.array(z.string()).default([]),
  failures: z.array(z.string()).default([]),
  skipped: z.array(z.string()).default([]),
  commit: z.string().nullable().default(null),
});

const finalValidationSchema = z.looseObject({
  allPassed: z.boolean().default(false),
  workingTreeClean: z.boolean().default(false),
  summary: z.string().default("Final validation completed."),
  commandsRun: z.array(z.object({
    command: z.string(),
    status: z.enum(["passed", "failed", "skipped"]),
    details: z.string().nullable().default(null),
  })).default([]),
  failures: z.array(z.string()).default([]),
  skipped: z.array(z.string()).default([]),
});

const finalSchema = z.looseObject({
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("P0 package recommendations finished."),
  completed: z.array(z.enum(workItemIds)).default([]),
  blocked: z.array(z.enum(workItemIds)).default([]),
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
  item: itemResultSchema,
  validation: validationSchema,
  finalValidation: finalValidationSchema,
  final: finalSchema,
});

function formatList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

function implementationTaskId(item: WorkItem): string {
  return `p0-package:${item.id}:implement`;
}

function validationTaskId(item: WorkItem): string {
  return `p0-package:${item.id}:validate`;
}

function priorGateId(index: number): string {
  return index === 0 ? "p0-package:scope" : validationTaskId(workItems[index - 1]);
}

function priorGateDeps(index: number) {
  return index === 0 ? { prior: outputs.scope } : { prior: outputs.validation };
}

function implementationDepKey(item: WorkItem): string {
  return `implementation_${item.id}`;
}

function validationDepKey(item: WorkItem): string {
  return `validation_${item.id}`;
}

function allImplementationNeeds(): Record<string, string> {
  return Object.fromEntries(workItems.map((item) => [implementationDepKey(item), implementationTaskId(item)]));
}

function allImplementationDeps(): Record<string, typeof itemResultSchema> {
  return Object.fromEntries(workItems.map((item) => [implementationDepKey(item), outputs.item]));
}

function allValidationNeeds(): Record<string, string> {
  return Object.fromEntries(workItems.map((item) => [validationDepKey(item), validationTaskId(item)]));
}

function allValidationDeps(): Record<string, typeof validationSchema> {
  return Object.fromEntries(workItems.map((item) => [validationDepKey(item), outputs.validation]));
}

function itemCanStart(ctx: any, index: number): boolean {
  if (index === 0) return true;
  const prior = ctx.outputMaybe("validation", { nodeId: validationTaskId(workItems[index - 1]) });
  return Boolean(prior?.allPassed === true && prior?.workingTreeClean === true);
}

function validationFailed(ctx: any): boolean {
  return workItems.some((item) => {
    const validation = ctx.outputMaybe("validation", { nodeId: validationTaskId(item) });
    return Boolean(validation && (validation.allPassed === false || validation.workingTreeClean === false));
  });
}

function allItemValidationsPassed(ctx: any): boolean {
  return workItems.every((item) => {
    const validation = ctx.outputMaybe("validation", { nodeId: validationTaskId(item) });
    return Boolean(validation?.allPassed === true && validation?.workingTreeClean === true);
  });
}

function scopePrompt(extraContext: string): string {
  return `Inventory and scope the P0 package recommendations for daml-tools before implementation.

Assumptions:
- The attached recommendation list means do the P0 items only; P1/P2 are out of scope for this workflow.
- Use the repo's existing Rust and test conventions.
- Use the current checkout directly. Do not create Smithers-managed worktrees.
- The fastSmart agent is Cursor Composer 2.5; do not route tasks to Codex Spark.

Work items, in order:
${workItems.map((item, index) => `${index + 1}. ${item.id}: ${item.title}`).join("\n")}

Success criteria:
- Each item implements the stated acceptance criteria.
- Each item has a separate validation gate before the next item starts.
- Missing corpus or fixture behavior fails loudly in CI/workspace checkouts and only skips explicitly for legitimate non-workspace/published-crate contexts.
- Tests remain semantic/oracle checks. Do not snapshot full ASTs, modules, raw SARIF, or real corpora.
- Commit focused working increments when files change and checks pass.

Out of scope:
- P1/P2 recommendations.
- Moving the shared root corpus.
- Wholesale fixture/corpus renames.
- Formatter snapshot migration.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return summary, assumptions, plannedOrder, risks, and outOfScope.`;
}

function implementationPrompt(item: WorkItem, index: number, prior: unknown, extraContext: string): string {
  const remaining = workItems.slice(index + 1).map((next) => `${next.id}: ${next.title}`);
  return `Implement one P0 package recommendation in daml-tools.

Read first:
- AGENTS.md
- The primary files/directories listed below
- Immediate callers/exports and existing nearby tests before writing code

Assigned item: ${item.title}
ID: ${item.id}
Package: ${item.packageName}
Risk: ${item.riskLevel}
Objective: ${item.objective}

Primary files/directories:
${formatList(item.primaryFiles)}

Acceptance criteria:
${formatList(item.acceptanceCriteria)}

Constraints:
${formatList(item.constraints)}

Validation commands for the separate validation task:
${formatList(item.validationCommands)}

Prior workflow gate output to preserve:
${JSON.stringify(prior, null, 2)}

Remaining P0 work after this item:
${remaining.length > 0 ? formatList(remaining) : "- none"}

Execution rules:
- Make the smallest surgical change that satisfies this item.
- Match existing code style and test conventions.
- Prefer semantic assertions over snapshots.
- Run focused checks as useful, but do not replace the separate validation task.
- If the repo already satisfies this item, make no code change and report evidence.
- If blocked or uncertain, stop and report exact blockers; do not guess.
- If files changed and focused checks pass, commit the focused update using Conventional Commit format.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return id, packageName, status, summary, filesChanged, testsAddedOrUpdated, commandsRun, blockers, followUps, and commit hash if committed.`;
}

function validationPrompt(item: WorkItem, implementation: unknown, extraContext: string): string {
  return `Validate one P0 package recommendation after implementation.

Item: ${item.title}
ID: ${item.id}
Package: ${item.packageName}

Implementation result:
${JSON.stringify(implementation, null, 2)}

Required validation commands unless genuinely impossible:
${item.validationCommands.map((command, index) => `${index + 1}. ${command}`).join("\n")}
${item.riskLevel === "medium" ? "\nThis is medium risk: also inspect the diff for accidental API or behavior changes before reporting success." : ""}

Validation rules:
- Run validation separately from implementation.
- Never silently skip a command; record exact skip reasons.
- If a command fails, diagnose whether it was caused by this item, pre-existing state, or environment.
- Fix only issues caused by this item and rerun affected commands.
- Confirm git status. The working tree must be clean to pass this gate.
- If you modify files and checks pass, commit the validation fix using Conventional Commit format.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return id, packageName, allPassed, workingTreeClean, summary, commandsRun, fixesApplied, failures, skipped, and commit hash if committed.`;
}

function finalValidationPrompt(implementations: unknown[], validations: unknown[], extraContext: string): string {
  return `Run final validation for all completed P0 package recommendations.

Implementation outputs:
${JSON.stringify(implementations, null, 2)}

Per-item validation outputs:
${JSON.stringify(validations, null, 2)}

Run these checks unless genuinely impossible; never silently skip:
1. cargo fmt --all -- --check
2. cargo test -p daml-parser --locked
3. cargo test -p daml-syntax --locked
4. cargo test -p daml-fmt --all-features --locked
5. git status --short

If checks fail, diagnose whether the failure belongs to this workflow. Fix only workflow-caused issues and rerun relevant commands. Record all commands, failures, skips, and whether the working tree is clean.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return final validation status, command results, failures, skipped commands, workingTreeClean, and summary.`;
}

function finalPrompt(implementations: unknown[], validations: unknown[], finalValidation: unknown): string {
  return `Synthesize the final handoff for the P0 package recommendations workflow.

Implementation outputs:
${JSON.stringify(implementations, null, 2)}

Per-item validation outputs:
${JSON.stringify(validations, null, 2)}

Final validation output:
${JSON.stringify(finalValidation, null, 2)}

Return:
- status: completed, partial, or blocked
- concise summary
- completed and blocked item IDs
- whether final validation passed
- next actions
- markdownBody suitable for a PR or handoff note, including tests added/updated, validation commands, and risks.`;
}

function blockedPrompt(implementations: unknown[], validations: unknown[]): string {
  return `Synthesize the blocked handoff for the P0 package recommendations workflow.

At least one per-item validation failed or left the working tree dirty, so later items were intentionally not scheduled.

Implementation outputs so far:
${JSON.stringify(implementations, null, 2)}

Per-item validation outputs so far:
${JSON.stringify(validations, null, 2)}

Return:
- status: blocked
- concise summary
- completed and blocked item IDs
- validationPassed: false
- next actions needed to unblock the workflow
- markdownBody suitable for a handoff note.`;
}

export default smithers((ctx) => (
  <Workflow name="p0-package-recommendations">
    <Sequence>
      <Task
        id="p0-package:scope"
        output={outputs.scope}
        agent={agents.fastSmart}
        timeoutMs={1_800_000}
        heartbeatTimeoutMs={600_000}
      >
        {scopePrompt(ctx.input.extraContext ?? "")}
      </Task>

      {workItems.flatMap((item, index) => itemCanStart(ctx, index) ? [
        <Task
          key={`${item.id}:implement`}
          id={implementationTaskId(item)}
          output={outputs.item}
          agent={agents.fastSmart}
          needs={{ prior: priorGateId(index) }}
          deps={priorGateDeps(index)}
          timeoutMs={3_600_000}
          heartbeatTimeoutMs={900_000}
          continueOnFail
        >
          {(deps: any) => implementationPrompt(item, index, deps.prior, ctx.input.extraContext ?? "")}
        </Task>,
        <Task
          key={`${item.id}:validate`}
          id={validationTaskId(item)}
          output={outputs.validation}
          agent={agents.fastSmart}
          needs={{ implementation: implementationTaskId(item) }}
          deps={{ implementation: outputs.item }}
          timeoutMs={3_600_000}
          heartbeatTimeoutMs={900_000}
          retries={2}
          continueOnFail
        >
          {(deps: any) => validationPrompt(item, deps.implementation, ctx.input.extraContext ?? "")}
        </Task>,
      ] : [])}

      {allItemValidationsPassed(ctx) ? (
        <Task
          id="p0-package:final-validation"
          output={outputs.finalValidation}
          agent={agents.fastSmart}
          needs={{ ...allImplementationNeeds(), ...allValidationNeeds() }}
          deps={{ ...allImplementationDeps(), ...allValidationDeps() }}
          timeoutMs={7_200_000}
          heartbeatTimeoutMs={900_000}
          retries={2}
        >
          {(deps: Record<string, unknown>) => finalValidationPrompt(
            workItems.map((item) => deps[implementationDepKey(item)]),
            workItems.map((item) => deps[validationDepKey(item)]),
            ctx.input.extraContext ?? "",
          )}
        </Task>
      ) : null}

      {validationFailed(ctx) ? (
        <Task
          id="p0-package:blocked-final"
          output={outputs.final}
          agent={agents.fastSmart}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {blockedPrompt(ctx.outputs.item ?? [], ctx.outputs.validation ?? [])}
        </Task>
      ) : null}

      {allItemValidationsPassed(ctx) ? (
        <Task
          id="p0-package:final"
          output={outputs.final}
          agent={agents.fastSmart}
          needs={{ finalValidation: "p0-package:final-validation", ...allImplementationNeeds(), ...allValidationNeeds() }}
          deps={{ finalValidation: outputs.finalValidation, ...allImplementationDeps(), ...allValidationDeps() }}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {(deps: Record<string, unknown>) => finalPrompt(
            workItems.map((item) => deps[implementationDepKey(item)]),
            workItems.map((item) => deps[validationDepKey(item)]),
            deps.finalValidation,
          )}
        </Task>
      ) : null}
    </Sequence>
  </Workflow>
));
