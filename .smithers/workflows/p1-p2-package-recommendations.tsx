// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: P1/P2 Package Recommendations
// smithers-description: Implement the remaining package test recommendations, including idiomatic fixture moves.
// smithers-tags: daml,rust,testing,p1,p2,fixtures
/** @jsxImportSource smithers-orchestrator */
import { createSmithers } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const workItemIds = [
  "fixture-layout-normalization",
  "daml-fmt-manifest-integrity",
  "daml-lint-output-goldens",
  "daml-lint-builtin-summary-goldens",
  "daml-fmt-fixture-and-cli-goldens",
  "daml-parser-diagnostic-goldens",
  "p2-doctests-and-sdk-corpus",
] as const;

type WorkItemId = typeof workItemIds[number];
type RiskLevel = "low" | "medium" | "high";

type WorkItem = {
  id: WorkItemId;
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
    id: "fixture-layout-normalization",
    title: "Move package-specific small fixtures to tests/fixtures",
    objective:
      "Standardize small package-specific fixtures under crates/<crate>/tests/fixtures while leaving shared and large corpus assets in place.",
    primaryFiles: [
      "crates/daml-lint/test-fixtures/",
      "crates/daml-lint/tests/",
      "crates/daml-fmt/corpus/gap-cases/",
      "crates/daml-fmt/tests/library_behavior.rs",
      "crates/daml-fmt/tools/verify-gap-cases.sh",
    ],
    acceptanceCriteria: [
      "Move crates/daml-lint/test-fixtures to crates/daml-lint/tests/fixtures and update all references.",
      "Move crates/daml-fmt/corpus/gap-cases to crates/daml-fmt/tests/fixtures/gap-cases and update tests/scripts.",
      "Do not move corpus/daml-finance or the large formatter original/expected compiler corpus.",
      "Fail-loud CI fixture behavior from P0 is preserved after path updates.",
    ],
    constraints: [
      "Use git mv or equivalent so history is clear.",
      "Do not rename the large formatter original/, expected/, or corpus manifest files in this item.",
      "Keep path helper code minimal; introduce tests/common only if multiple tests need the same helper after this move.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cargo test -p daml-fmt --all-features --locked",
      "cd crates/daml-fmt && node test/diff.js",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-fmt-manifest-integrity",
    title: "Add formatter corpus manifest integrity checks",
    objective:
      "Make crates/daml-fmt/test/diff.js fail loudly when original/, expected/, or corpus/desugar-ok.txt are missing, stale, or incomplete.",
    primaryFiles: [
      "crates/daml-fmt/test/diff.js",
      "crates/daml-fmt/original/",
      "crates/daml-fmt/expected/",
      "crates/daml-fmt/corpus/desugar-ok.txt",
    ],
    acceptanceCriteria: [
      "Every manifest path exists in original/ and expected/.",
      "Every original file in the tested set has a matching expected file.",
      "No stale expected files exist outside the tested set.",
      "Missing original/, expected/, or manifest files fail the differential script loudly.",
    ],
    constraints: [
      "Do not migrate the 924-file corpus to snapshots.",
      "Do not move or rename original/ and expected/ unless unavoidable; prefer integrity checks.",
      "Keep script output concise but actionable.",
    ],
    validationCommands: [
      "cd crates/daml-fmt && node test/diff.js",
      "cargo test -p daml-fmt --all-features --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-lint-output-goldens",
    title: "Add compact normalized goldens for daml-lint user-facing output",
    objective:
      "Pin markdown, JSON, SARIF, parse-error, unknown config/rule behavior with compact normalized golden fixtures.",
    primaryFiles: [
      "crates/daml-lint/tests/cli.rs",
      "crates/daml-lint/tests/reporter_contracts.rs",
      "crates/daml-lint/tests/fixtures/",
      "crates/daml-lint/tests/golden/",
    ],
    acceptanceCriteria: [
      "Add compact golden fixtures under crates/daml-lint/tests/golden/ for user-facing output.",
      "Normalize volatile fields such as absolute paths, temp directories, ordering noise, and SARIF metadata where appropriate.",
      "Keep status-code and JSON/SARIF field assertions semantic rather than replacing them with snapshots.",
      "Cover markdown, JSON, SARIF, parse errors, and unknown config/rule behavior.",
    ],
    constraints: [
      "Do not snapshot raw SARIF with volatile metadata.",
      "Do not snapshot whole real corpora or whole modules.",
      "Do not weaken existing semantic assertions.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked --test cli --test reporter_contracts",
      "cargo test -p daml-lint --all-features --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-lint-builtin-summary-goldens",
    title: "Pin built-in detector output with compact golden summaries",
    objective:
      "Add compact normalized summary goldens for built-in detector findings without snapshotting whole modules.",
    primaryFiles: [
      "crates/daml-lint/tests/builtin_rule_behavior.rs",
      "crates/daml-lint/tests/fixtures/",
      "crates/daml-lint/tests/golden/",
    ],
    acceptanceCriteria: [
      "Generate or assert compact summaries shaped as rule|severity|file|line|column|message|evidence.",
      "Use moved fixtures under crates/daml-lint/tests/fixtures where fixture files are needed.",
      "Avoid whole-module snapshots; preserve existing count/severity/evidence semantic checks.",
    ],
    constraints: [
      "Do not change detector behavior to match goldens.",
      "Do not weaken expected finding count tables.",
      "Respect js-runtime feature gating.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked --test builtin_rule_behavior",
      "cargo test -p daml-lint --all-features --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-fmt-fixture-and-cli-goldens",
    title: "Move bulky formatter expectations to paired fixtures and add CLI goldens",
    objective:
      "Move only bulky formatter expected strings from Rust source to paired fixtures and add compact CLI output golden coverage.",
    primaryFiles: [
      "crates/daml-fmt/tests/layout_fixtures.rs",
      "crates/daml-fmt/tests/cli.rs",
      "crates/daml-fmt/tests/fixtures/",
    ],
    acceptanceCriteria: [
      "Bulky layout cases use paired fixture files under crates/daml-fmt/tests/fixtures, not large inline strings.",
      "Small rule-intent tests remain direct assert_eq! where clearer.",
      "CLI golden coverage includes help, version if available, invalid flags, parser diagnostics, and check/write status.",
      "Mutation-safety tests remain semantic.",
    ],
    constraints: [
      "Do not introduce insta or whole-corpus snapshots.",
      "Do not alter formatter output semantics.",
      "Keep fixture names descriptive and local to daml-fmt.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked --test layout_fixtures --test cli",
      "cargo test -p daml-fmt --all-features --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-parser-diagnostic-goldens",
    title: "Add normalized parser diagnostic goldens",
    objective:
      "Add small normalized user-facing diagnostic stream goldens for daml-parser diagnostics.",
    primaryFiles: [
      "crates/daml-parser/tests/diagnostics_recovery.rs",
      "crates/daml-parser/tests/",
      "crates/daml-parser/tests/fixtures/",
    ],
    acceptanceCriteria: [
      "Add compact normalized diagnostic golden coverage under daml-parser tests.",
      "Snapshot only user-facing diagnostic streams/categories/spans/messages, not full AST/debug output.",
      "Keep parser precedence, type wiring, and span round-trip tests semantic.",
    ],
    constraints: [
      "Do not alter parser behavior just to satisfy a golden.",
      "Do not snapshot full ASTs or DamlModules.",
      "Prefer a tiny local normalizer over adding broad abstractions.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-parser --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "p2-doctests-and-sdk-corpus",
    title: "Add targeted doctests and normalize optional SDK corpus checks",
    objective:
      "Add targeted doctests for fallible public APIs and make optional SDK-style corpus checks explicitly enabled/ignored rather than silently local-state dependent.",
    primaryFiles: [
      "crates/daml-parser/src/lib.rs",
      "crates/daml-syntax/src/lib.rs",
      "crates/daml-lint/src/lib.rs",
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-lint/tests/corpus_integration.rs",
      "crates/daml-lint/tests/",
    ],
    acceptanceCriteria: [
      "Add targeted doctests for fallible parser, syntax, lint, and formatter public APIs where missing.",
      "Optional SDK corpus checks are controlled by explicit env vars, ignored tests, or clear fail/skip policy rather than silently depending on /tmp/daml-repo.",
      "Add tests/common/mod.rs only if actual shared helper reuse emerged from earlier items; otherwise do not add it.",
    ],
    constraints: [
      "Do not add compatibility shims or fallback behavior beyond explicit test gating.",
      "Do not expand public API solely for doctests.",
      "Keep doctests small and focused on fallible behavior.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test --doc -p daml-parser --locked",
      "cargo test --doc -p daml-syntax --locked",
      "cargo test --doc -p daml-lint --locked",
      "cargo test --doc -p daml-fmt --locked",
      "cargo test -p daml-lint --all-features --locked",
      "cargo test --workspace --all-features --locked",
    ],
    riskLevel: "medium",
  },
];

const scopeSchema = z.looseObject({
  summary: z.string().default("P1/P2 recommendations scoped."),
  assumptions: z.array(z.string()).default([]),
  plannedOrder: z.array(z.enum(workItemIds)).default([]),
  risks: z.array(z.string()).default([]),
  fixturePolicy: z.string().default(""),
});

const itemSchema = z.looseObject({
  id: z.enum(workItemIds).default("fixture-layout-normalization"),
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("Work item completed."),
  filesChanged: z.array(z.string()).default([]),
  fixturesMoved: z.array(z.string()).default([]),
  testsAddedOrUpdated: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  blockers: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
  commit: z.string().nullable().default(null),
});

const validationSchema = z.looseObject({
  id: z.enum(workItemIds).default("fixture-layout-normalization"),
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
  summary: z.string().default("P1/P2 recommendations finished."),
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
  scope: scopeSchema,
  item: itemSchema,
  validation: validationSchema,
  finalValidation: finalValidationSchema,
  final: finalSchema,
});

function formatList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

function implementationTaskId(item: WorkItem): string {
  return `p1p2:${item.id}:implement`;
}

function validationTaskId(item: WorkItem): string {
  return `p1p2:${item.id}:validate`;
}

function priorGateId(index: number): string {
  return index === 0 ? "p1p2:scope" : validationTaskId(workItems[index - 1]);
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

function allImplementationDeps(): Record<string, typeof itemSchema> {
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
  return `Inventory and scope the remaining P1/P2 package recommendations for daml-tools.

Assumptions:
- P0 items are already implemented on this branch.
- Continue on the current branch and update the existing PR; do not open a second PR from inside the workflow.
- The user explicitly asked to move fixtures to idiomatic patterns.
- Use Cursor Composer 2.5 via agents.fastSmart; do not route any task to gpt-5.3-codex-spark.

Fixture policy:
- Shared large corpus stays at workspace root: corpus/daml-finance.
- Package-specific small fixtures move to crates/<crate>/tests/fixtures.
- Formatter compiler corpus stays crate-owned: crates/daml-fmt/original, expected, and corpus/*.txt. Prefer manifest integrity checks over renaming it.
- Do not solve CI safety by moving shared corpora.

Work items, in order:
${workItems.map((item, index) => `${index + 1}. ${item.id}: ${item.title}`).join("\n")}

Success criteria:
- Each work item is implemented as a focused slice and committed when working.
- Each item has a separate validation gate before the next item starts.
- P1/P2 items that are genuinely not applicable must be explained with evidence, not silently skipped.
- Final validation passes with a clean tree.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return summary, assumptions, plannedOrder, risks, and fixturePolicy.`;
}

function implementationPrompt(item: WorkItem, index: number, prior: unknown, extraContext: string): string {
  const remaining = workItems.slice(index + 1).map((next) => `${next.id}: ${next.title}`);
  return `Implement one remaining P1/P2 package recommendation slice in daml-tools.

Read first:
- AGENTS.md
- Primary files/directories below
- Immediate callers/exports and nearby tests before writing code

Assigned item: ${item.title}
ID: ${item.id}
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

Remaining work after this item:
${remaining.length > 0 ? formatList(remaining) : "- none"}

Execution rules:
- Make the smallest surgical change that satisfies this item.
- Match existing code style and test conventions.
- Prefer compact semantic goldens over broad snapshots.
- Do not move shared corpus/daml-finance.
- Do not move large formatter original/expected corpus unless the item explicitly requires it; it does not.
- Run focused checks as useful, but do not replace the separate validation task.
- If the repo already satisfies part of this item, report evidence and only change what remains.
- If blocked or uncertain, stop and report exact blockers; do not guess.
- If files changed and focused checks pass, commit the focused update using Conventional Commit format.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return id, status, summary, filesChanged, fixturesMoved, testsAddedOrUpdated, commandsRun, blockers, followUps, and commit hash if committed.`;
}

function validationPrompt(item: WorkItem, implementation: unknown, extraContext: string): string {
  return `Validate one P1/P2 package recommendation slice after implementation.

Item: ${item.title}
ID: ${item.id}

Implementation result:
${JSON.stringify(implementation, null, 2)}

Required validation commands unless genuinely impossible:
${item.validationCommands.map((command, index) => `${index + 1}. ${command}`).join("\n")}

Validation rules:
- Run validation separately from implementation.
- Never silently skip a command; record exact skip reasons.
- If a command fails, diagnose whether it was caused by this item, pre-existing state, or environment.
- Fix only issues caused by this item and rerun affected commands.
- For fixture moves, grep for stale old paths before reporting success.
- Confirm git status. The working tree must be clean to pass this gate.
- If you modify files and checks pass, commit the validation fix using Conventional Commit format.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return id, allPassed, workingTreeClean, summary, commandsRun, fixesApplied, failures, skipped, and commit hash if committed.`;
}

function finalValidationPrompt(implementations: unknown[], validations: unknown[], extraContext: string): string {
  return `Run final validation for the completed P1/P2 package recommendations.

Implementation outputs:
${JSON.stringify(implementations, null, 2)}

Per-item validation outputs:
${JSON.stringify(validations, null, 2)}

Run these checks unless genuinely impossible; never silently skip:
1. cargo fmt --all -- --check
2. cargo test -p daml-parser --locked
3. cargo test -p daml-syntax --locked
4. cargo test -p daml-lint --all-features --locked
5. cargo test -p daml-fmt --all-features --locked
6. cargo test --workspace --all-features --locked
7. cd crates/daml-fmt && node test/diff.js
8. git status --short

If checks fail, diagnose whether the failure belongs to this workflow. Fix only workflow-caused issues and rerun relevant commands. Record all commands, failures, skips, and whether the working tree is clean.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return final validation status, command results, failures, skipped commands, workingTreeClean, and summary.`;
}

function finalPrompt(implementations: unknown[], validations: unknown[], finalValidation: unknown): string {
  return `Synthesize the final handoff for the P1/P2 package recommendations workflow.

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
- markdownBody suitable for updating PR #118, including fixture moves, goldens, doctests, validation commands, and risks.`;
}

function blockedPrompt(implementations: unknown[], validations: unknown[]): string {
  return `Synthesize the blocked handoff for the P1/P2 package recommendations workflow.

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
  <Workflow name="p1-p2-package-recommendations">
    <Sequence>
      <Task
        id="p1p2:scope"
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
          id="p1p2:final-validation"
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
          id="p1p2:blocked-final"
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
          id="p1p2:final"
          output={outputs.final}
          agent={agents.fastSmart}
          needs={{ finalValidation: "p1p2:final-validation", ...allImplementationNeeds(), ...allValidationNeeds() }}
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
