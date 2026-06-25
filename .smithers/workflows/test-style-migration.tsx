// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: Test Style Migration
// smithers-description: Finish migrating tests toward integration-style coverage with src unit tests reserved for internal contracts.
// smithers-tags: daml, rust, testing, migration
/** @jsxImportSource smithers-orchestrator */
import { createSmithers } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const workItemIds = [
  "daml-parser-internal-unit-boundary",
  "daml-syntax-line-index-unit-boundary",
  "daml-lint-builtin-rule-integration",
  "daml-lint-script-runtime-unit-boundary",
  "daml-lint-core-internal-unit-boundary",
  "daml-fmt-layout-helper-unit-boundary",
  "daml-fmt-public-fixture-integration-boundary",
] as const;

type WorkItemId = typeof workItemIds[number];

type PackageName = "daml-parser" | "daml-syntax" | "daml-lint" | "daml-fmt";

type WorkItem = {
  id: WorkItemId;
  packageName: PackageName;
  category: string;
  title: string;
  objective: string;
  primaryFiles: string[];
  targetShape: string[];
  constraints: string[];
  validationCommands: string[];
  riskLevel: "low" | "medium" | "high";
};

const workItems: WorkItem[] = [
  {
    id: "daml-parser-internal-unit-boundary",
    packageName: "daml-parser",
    category: "internal-unit-contracts",
    title: "Verify daml-parser src tests are only private parser-phase contracts",
    objective:
      "Review remaining lexer/layout/parse/ast/ast_span src tests and move any externally observable parser behavior to integration tests; leave true private phase contracts in src.",
    primaryFiles: [
      "crates/daml-parser/src/lexer.rs",
      "crates/daml-parser/src/layout.rs",
      "crates/daml-parser/src/parse.rs",
      "crates/daml-parser/src/ast.rs",
      "crates/daml-parser/src/ast_span.rs",
      "crates/daml-parser/tests/",
    ],
    targetShape: [
      "Lexer token/trivia, layout virtual-token, and private parser-helper contracts may remain as src unit tests.",
      "Externally observable module parsing, diagnostics, recovery, projection precedence, and span behavior stays under crates/daml-parser/tests/.",
      "If no movement is needed, report the src tests that remain and why they are internal contracts.",
    ],
    constraints: [
      "Do not expose private parser functions only to move tests.",
      "Do not churn already-migrated integration tests.",
      "Do not weaken exact span, diagnostic, or parser-shape assertions.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-parser --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-syntax-line-index-unit-boundary",
    packageName: "daml-syntax",
    category: "line-index-internal-contracts",
    title: "Verify daml-syntax src tests only cover LineIndex internals",
    objective:
      "Keep SourceFile/SourceTokens/diagnostic/span-conversion behavior under integration tests and leave only LineIndex mapping internals in src/lib.rs.",
    primaryFiles: [
      "crates/daml-syntax/src/lib.rs",
      "crates/daml-syntax/src/coordinate.rs",
      "crates/daml-syntax/tests/source_api.rs",
      "crates/daml-syntax/tests/coordinate_contracts.rs",
      "crates/daml-syntax/tests/compile_fail/",
    ],
    targetShape: [
      "Source API and coordinate public contracts stay under crates/daml-syntax/tests/.",
      "src/lib.rs unit tests are limited to private LineIndex offset/line/column invariants.",
      "Any public behavior duplicated in src is moved or removed after confirming integration coverage.",
    ],
    constraints: [
      "Do not change SourceFile, SourceTokens, coordinate, or diagnostic behavior.",
      "Do not add compile-fail infrastructure beyond the existing crate-local pattern unless it is already present and minimal.",
      "Keep comments surgical; avoid annotating every test mechanically.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-syntax --locked",
      "cargo test --doc -p daml-syntax --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-lint-builtin-rule-integration",
    packageName: "daml-lint",
    category: "builtin-rule-integration",
    title: "Move daml-lint built-in rule behavior tests out of src",
    objective:
      "Relocate broad built-in JavaScript detector behavior cases from src/detectors/builtin_script_tests.rs to integration tests that exercise the crate's public built-in detector surface where practical.",
    primaryFiles: [
      "crates/daml-lint/src/detectors/mod.rs",
      "crates/daml-lint/src/detectors/builtin_script_tests.rs",
      "crates/daml-lint/src/detectors/script.rs",
      "crates/daml-lint/src/lib.rs",
      "crates/daml-lint/tests/",
      "crates/daml-lint/rules/",
    ],
    targetShape: [
      "Built-in rule input/output behavior lives in crates/daml-lint/tests/ as integration-style tests.",
      "src/detectors/mod.rs no longer wires a broad #[cfg(test)] behavior module if integration coverage replaces it.",
      "Only private script runtime contracts remain in src/detectors/script.rs.",
    ],
    constraints: [
      "Do not broaden the JS runtime or detector public API only for tests.",
      "Preserve feature gates for js-runtime/custom-rule-related tests.",
      "Do not weaken expected finding counts, severity, evidence, or message assertions.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cargo test -p daml-lint --no-default-features --features cli,js-runtime,custom-rules --locked",
    ],
    riskLevel: "high",
  },
  {
    id: "daml-lint-script-runtime-unit-boundary",
    packageName: "daml-lint",
    category: "script-runtime-internal-contracts",
    title: "Verify daml-lint script runtime src tests are private runtime contracts",
    objective:
      "Review src/detectors/script.rs tests and keep only private load/runtime/error/interrupt contracts in src; move any public detector behavior to integration tests.",
    primaryFiles: [
      "crates/daml-lint/src/detectors/script.rs",
      "crates/daml-lint/tests/custom_rule_runtime_contracts.rs",
      "crates/daml-lint/examples/",
      "crates/daml-lint/lint-plugin/",
    ],
    targetShape: [
      "Private load_script_source validation, runtime error attribution, and interrupt counter behavior may remain source-local.",
      "Script-visible node kinds, generated rule type contracts, and example rule behavior remain integration-style.",
      "Any src test retained must have a private-contract reason in the task output.",
    ],
    constraints: [
      "Do not expose private runtime hooks just to move tests.",
      "Respect no-default-feature and JS feature combinations.",
      "Do not regenerate npm artifacts unless a test move actually requires it.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cd crates/daml-lint && npm ci && npm run check:rules",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-lint-core-internal-unit-boundary",
    packageName: "daml-lint",
    category: "core-internal-contracts",
    title: "Verify daml-lint config/detector/IR src tests are narrow internal contracts",
    objective:
      "Review remaining config.rs, detector.rs, and ir.rs unit tests so src contains only private constructor/default/wrapper invariants while public parser/reporter/detector behavior is integration-style.",
    primaryFiles: [
      "crates/daml-lint/src/config.rs",
      "crates/daml-lint/src/detector.rs",
      "crates/daml-lint/src/ir.rs",
      "crates/daml-lint/tests/detector_contracts.rs",
      "crates/daml-lint/tests/parser_ir_contracts.rs",
      "crates/daml-lint/tests/reporter_contracts.rs",
      "crates/daml-lint/tests/cli.rs",
    ],
    targetShape: [
      "Public parser lowering, detector, reporter, and CLI behavior remains under crates/daml-lint/tests/.",
      "src tests stay narrow when they protect private defaults, wrappers, or construction invariants.",
      "Duplicate assertions are removed only after confirming equivalent integration coverage.",
    ],
    constraints: [
      "Do not move private helper tests if doing so requires public escape hatches.",
      "Do not bundle detector behavior changes with test relocation.",
      "Preserve no-default-feature behavior where relevant.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cargo test -p daml-lint --no-default-features --lib --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-fmt-layout-helper-unit-boundary",
    packageName: "daml-fmt",
    category: "layout-helper-internal-contracts",
    title: "Verify daml-fmt layout_ast src tests only cover private helpers",
    objective:
      "Review remaining layout_ast.rs src tests and keep only helper-specific line/comment/indent contracts in src; move any formatter output behavior to integration fixture tests.",
    primaryFiles: [
      "crates/daml-fmt/src/layout_ast.rs",
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-fmt/tests/layout_fixtures.rs",
      "crates/daml-fmt/tests/library_behavior.rs",
    ],
    targetShape: [
      "Private line/comment/indent helpers may remain source-local.",
      "Given-input/expected-output formatting behavior remains under crates/daml-fmt/tests/.",
      "No duplicate black-box formatter assertions remain in src.",
    ],
    constraints: [
      "Do not intentionally change formatter output.",
      "Do not weaken idempotence or exact-output assertions.",
      "Do not expose private layout helpers for tests.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-fmt-public-fixture-integration-boundary",
    packageName: "daml-fmt",
    category: "public-formatting-fixtures",
    title: "Verify daml-fmt public formatting behavior is integration-style",
    objective:
      "Confirm format_source/try_format_source/options/coverage and broad layout examples are covered from crates/daml-fmt/tests, not src unit tests.",
    primaryFiles: [
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-fmt/tests/library_behavior.rs",
      "crates/daml-fmt/tests/layout_fixtures.rs",
      "crates/daml-fmt/tests/coverage.rs",
      "crates/daml-fmt/tests/cli.rs",
      "crates/daml-fmt/test/diff.js",
    ],
    targetShape: [
      "Public library and CLI behavior remains integration-style under tests/.",
      "Coverage and npm differential gates stay fail-loud when required inputs are expected.",
      "Any discovered public behavior test in src is moved before this item completes.",
    ],
    constraints: [
      "Do not rewrite fixtures or formatter internals for style only.",
      "Do not alter malformed-input passthrough/rejection behavior.",
      "Keep npm differential coverage separate from Rust unit/helper tests.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked",
      "cd crates/daml-fmt && npm test",
    ],
    riskLevel: "medium",
  },
];

const scopeOutputSchema = z.looseObject({
  summary: z.string().default("Test style migration scoped."),
  assumptions: z.array(z.string()).default([]),
  plannedChanges: z.array(z.object({
    id: z.enum(workItemIds),
    packageName: z.string(),
    category: z.string(),
    expectedChange: z.string(),
  })).default([]),
  risks: z.array(z.string()).default([]),
  executionNotes: z.array(z.string()).default([]),
  workingStateDefinition: z.string().default("Each package/category item validates before the next item starts."),
});

const itemResultSchema = z.looseObject({
  id: z.enum(workItemIds).default("daml-parser-internal-unit-boundary"),
  packageName: z.string().default("daml-parser"),
  category: z.string().default("category"),
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("Work item completed."),
  filesChanged: z.array(z.string()).default([]),
  testsMoved: z.array(z.string()).default([]),
  testsLeftInSrc: z.array(z.object({
    file: z.string(),
    reason: z.string(),
  })).default([]),
  commandsRun: z.array(z.string()).default([]),
  blockers: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
  commit: z.string().nullable().default(null),
});

const itemValidationSchema = z.looseObject({
  id: z.enum(workItemIds).default("daml-parser-internal-unit-boundary"),
  packageName: z.string().default("daml-parser"),
  category: z.string().default("category"),
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

const finalOutputSchema = z.looseObject({
  status: z.enum(["completed", "partial", "blocked"]).default("partial"),
  summary: z.string().default("Test style migration workflow finished."),
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
  item: itemResultSchema,
  itemValidation: itemValidationSchema,
  finalValidation: finalValidationSchema,
  final: finalOutputSchema,
});

function formatList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

function implementationTaskId(item: WorkItem): string {
  return `test-style-migration:${item.id}:implement`;
}

function validationTaskId(item: WorkItem): string {
  return `test-style-migration:${item.id}:validate`;
}

function priorGateId(index: number): string {
  return index === 0 ? "test-style-migration:scope" : validationTaskId(workItems[index - 1]);
}

function priorGateDeps(index: number) {
  return index === 0
    ? { prior: outputs.scope }
    : { prior: outputs.itemValidation };
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

function allValidationDeps(): Record<string, typeof itemValidationSchema> {
  return Object.fromEntries(workItems.map((item) => [validationDepKey(item), outputs.itemValidation]));
}

function itemCanStart(ctx: any, index: number): boolean {
  if (index === 0) return true;
  const prior = ctx.outputMaybe("itemValidation", { nodeId: validationTaskId(workItems[index - 1]) });
  return Boolean(prior?.allPassed === true && prior?.workingTreeClean === true);
}

function validationFailed(ctx: any): boolean {
  return workItems.some((item) => {
    const validation = ctx.outputMaybe("itemValidation", { nodeId: validationTaskId(item) });
    return Boolean(validation && (validation.allPassed === false || validation.workingTreeClean === false));
  });
}

function allItemValidationsPassed(ctx: any): boolean {
  return workItems.every((item) => {
    const validation = ctx.outputMaybe("itemValidation", { nodeId: validationTaskId(item) });
    return Boolean(validation?.allPassed === true && validation?.workingTreeClean === true);
  });
}

function scopePrompt(extraContext: string): string {
  return `Plan and prepare the daml-tools test style migration from the current repository state.

Goal:
Prefer integration-style tests for externally observable behavior. Keep unit-style tests in src only for specific private/internal contracts.

Current inventory summary:
- daml-parser broad diagnostics/recovery/span/projection behavior is already under crates/daml-parser/tests; remaining src tests need internal-contract verification only.
- daml-syntax public SourceFile/SourceTokens/coordinate behavior is already under crates/daml-syntax/tests; remaining src tests should be LineIndex internals only.
- daml-lint parser/IR, corpus, adversarial, detector, reporter, and custom runtime contracts are mostly integration-style; the likely current lapse is built-in rule behavior in src/detectors/builtin_script_tests.rs.
- daml-fmt public library/layout behavior is already under crates/daml-fmt/tests; remaining src tests should be private helper contracts only.

Behavior contract:
- Read AGENTS.md and relevant crate docs before changing files in later tasks.
- Each package/category work item must be completed as its own focused update.
- After each implementation task, a separate validation task must run before the next implementation task starts.
- Each implementation and validation task should leave the repository in a working state and commit a focused increment when it reaches one.
- Do not use Smithers <Worktree> unless the user explicitly asks; this repo prefers visible diffs in the current worktree.
- Do not push or open a PR from inside this workflow.
- Do not expose private APIs only to move tests. If a test truly needs private access, keep it as a src unit test and record why.

Work items in order:
${workItems.map((item, index) => `${index + 1}. ${item.packageName} / ${item.category}: ${item.title} (${item.id})`).join("\n")}

Final validation baseline:
- cargo fmt --all -- --check
- cargo clippy --workspace --all-targets --all-features --locked
- RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
- cargo test --workspace --all-features --locked
- cd crates/daml-fmt && npm test
- bash scripts/check-lint-rules.sh

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return assumptions, plannedChanges, risks, execution notes, and the working-state definition that later tasks must preserve.`;
}

function implementationPrompt(item: WorkItem, index: number, prior: unknown, extraContext: string): string {
  const remaining = workItems.slice(index + 1).map((next) => `${next.packageName}/${next.category}: ${next.id}`);
  return `Complete one test-style migration work item in daml-tools.

Assigned item: ${item.title}
ID: ${item.id}
Package: ${item.packageName}
Category: ${item.category}
Risk: ${item.riskLevel}
Objective: ${item.objective}

Primary files/directories to inspect first:
${formatList(item.primaryFiles)}

Target shape:
${formatList(item.targetShape)}

Constraints:
${formatList(item.constraints)}

Validation commands expected for the separate validation task:
${formatList(item.validationCommands)}

Prior workflow gate output to preserve:
${JSON.stringify(prior, null, 2)}

Remaining work after this item:
${remaining.length > 0 ? formatList(remaining) : "- none"}

Execution rules:
- Make only the smallest changes needed for this package/category.
- Prefer moving tests before rewriting assertions. Improve names only when needed for clarity after the move.
- Preserve existing behavior; this workflow is about test placement and contract clarity, not feature changes.
- Run quick focused checks as needed before handing off, but do not perform the separate validation task's role here.
- If you leave any tests in src, record the exact file and why they are specific internal contracts.
- If the current repo already satisfies this item, make no code change, report the evidence, and still leave the repo clean.
- If blocked or uncertain, stop and report blockers; do not guess.
- Commit the focused update if the repo is in a working state and files changed, following Conventional Commit rules.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return id, packageName, category, status, summary, files changed, tests moved, tests intentionally left in src with reasons, commands run, blockers, follow-ups, and commit hash if committed.`;
}

function validationPrompt(item: WorkItem, implementation: unknown, extraContext: string): string {
  return `Validate one completed test-style migration work item.

Item: ${item.title}
ID: ${item.id}
Package: ${item.packageName}
Category: ${item.category}

Implementation result:
${JSON.stringify(implementation, null, 2)}

Required validation commands for this item unless genuinely impossible:
${item.validationCommands.map((command, index) => `${index + 1}. ${command}`).join("\n")}
${item.riskLevel === "high" ? "\nThis is a high-risk item: also inspect the diff for accidental behavior changes before reporting success." : ""}

Validation rules:
- Run validation separately from the implementation task.
- If a command fails, diagnose whether it was caused by this work item, pre-existing state, or environment.
- Fix only issues caused by this work item, rerun the affected command, and commit the validation fix if you modify files.
- Do not advance with uncommitted changes unless blocked; record git status either way.
- Never silently skip a command. Record exact skip reasons.
- Confirm whether the working tree is clean after validation.
- Do not push or open a PR.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return item id, packageName, category, allPassed, workingTreeClean, commands run with status/details, fixes applied, failures, skipped commands, summary, and commit hash if committed.`;
}

function finalValidationPrompt(implementations: unknown[], validations: unknown[], extraContext: string): string {
  return `Run final whole-repository validation for the complete test-style migration.

Implementation outputs:
${JSON.stringify(implementations, null, 2)}

Per-item validation outputs:
${JSON.stringify(validations, null, 2)}

Run these checks unless genuinely impossible; never silently skip:
1. cargo fmt --all -- --check
2. cargo clippy --workspace --all-targets --all-features --locked
3. RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps --locked
4. cargo test --workspace --all-features --locked
5. cd crates/daml-fmt && npm test
6. bash scripts/check-lint-rules.sh
7. git status --short

If checks fail, diagnose whether the failure belongs to this workflow. Fix only workflow-caused issues and rerun relevant commands. Record all commands, failures, skips, and whether the working tree is clean.

${extraContext ? `Extra user context:\n${extraContext}\n` : ""}

Return final validation status, command results, failures, skipped commands, workingTreeClean, and summary.`;
}

function finalPrompt(implementations: unknown[], validations: unknown[], finalValidation: unknown): string {
  return `Synthesize the final handoff for the test-style migration workflow.

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
- markdownBody suitable for a PR or handoff note, including tests moved, tests intentionally left in src, validation commands, and risks.`;
}

function blockedPrompt(implementations: unknown[], validations: unknown[]): string {
  return `Synthesize the blocked handoff for the test-style migration workflow.

At least one per-item validation failed or left the working tree dirty, so later package/category items were intentionally not scheduled. This preserves the requirement that each update leave the repository in a working state before the next update begins.

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
  <Workflow name="test-style-migration">
    <Sequence>
      <Task
        id="test-style-migration:scope"
        output={outputs.scope}
        agent={agents.smart}
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
          agent={agents.smartTool}
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
          output={outputs.itemValidation}
          agent={agents.smart}
          needs={{ implementation: implementationTaskId(item) }}
          deps={{ implementation: outputs.item }}
          timeoutMs={3_600_000}
          heartbeatTimeoutMs={900_000}
          continueOnFail
        >
          {(deps: any) => validationPrompt(item, deps.implementation, ctx.input.extraContext ?? "")}
        </Task>,
      ] : [])}

      {allItemValidationsPassed(ctx) ? (
        <Task
          id="test-style-migration:final-validation"
          output={outputs.finalValidation}
          agent={agents.smart}
          needs={{ ...allImplementationNeeds(), ...allValidationNeeds() }}
          deps={{ ...allImplementationDeps(), ...allValidationDeps() }}
          timeoutMs={7_200_000}
          heartbeatTimeoutMs={900_000}
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
          id="test-style-migration:blocked-final"
          output={outputs.final}
          agent={agents.smart}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {blockedPrompt(ctx.outputs.item ?? [], ctx.outputs.itemValidation ?? [])}
        </Task>
      ) : null}

      {allItemValidationsPassed(ctx) ? (
        <Task
          id="test-style-migration:final"
          output={outputs.final}
          agent={agents.smart}
          needs={{ finalValidation: "test-style-migration:final-validation", ...allImplementationNeeds(), ...allValidationNeeds() }}
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
