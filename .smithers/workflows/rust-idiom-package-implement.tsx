// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: Rust Idiom Package Implement
// smithers-description: Implement verified actionable Rust idiom findings by package/category, then validate and summarize.
// smithers-tags: daml, rust, implementation, api-design
/** @jsxImportSource smithers-orchestrator */
import { createSmithers, Sequence, Task } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const packageIds = ["daml-parser", "daml-syntax", "daml-lint", "daml-fmt"] as const;
const categoryIds = ["type-safety", "error-handling", "interoperability", "documentation"] as const;

type PackageId = typeof packageIds[number];
type CategoryId = typeof categoryIds[number];

type ImplementationItem = {
  packageId: PackageId;
  categoryId: CategoryId;
  title: string;
  primaryFiles: string[];
  validationHints: string[];
};

const packagePrimaryFiles: Record<PackageId, string[]> = {
  "daml-parser": [
    "crates/daml-parser/src/lib.rs",
    "crates/daml-parser/src/ast.rs",
    "crates/daml-parser/src/lexer.rs",
    "crates/daml-parser/src/parse.rs",
    "crates/daml-parser/src/ast_span.rs",
    "crates/daml-parser/tests",
    "crates/daml-parser/README.md",
  ],
  "daml-syntax": [
    "crates/daml-syntax/src/lib.rs",
    "crates/daml-syntax/src/coordinate.rs",
    "crates/daml-syntax/tests",
    "crates/daml-syntax/README.md",
  ],
  "daml-lint": [
    "crates/daml-lint/src/lib.rs",
    "crates/daml-lint/src/detector.rs",
    "crates/daml-lint/src/ir.rs",
    "crates/daml-lint/src/parser.rs",
    "crates/daml-lint/src/reporter.rs",
    "crates/daml-lint/src/config.rs",
    "crates/daml-lint/src/detectors",
    "crates/daml-lint/tests",
    "crates/daml-lint/README.md",
  ],
  "daml-fmt": [
    "crates/daml-fmt/src/lib.rs",
    "crates/daml-fmt/src/layout_ast.rs",
    "crates/daml-fmt/src/bin",
    "crates/daml-fmt/tests",
    "crates/daml-fmt/README.md",
  ],
};

const categoryValidationHints: Record<CategoryId, string[]> = {
  "type-safety": [
    "Add/update tests that fail if distinct coordinate/span concepts can be mixed accidentally.",
    "Run cargo test for affected crates and cargo clippy --workspace --all-targets -- -D warnings.",
  ],
  "error-handling": [
    "Add/update tests for recoverable failures and typed error propagation.",
    "Run cargo test for affected crates and cargo clippy --workspace --all-targets -- -D warnings.",
  ],
  interoperability: [
    "Add compile-time/tests for trait/conversion behavior where the change is public API.",
    "Run cargo test for affected crates and cargo clippy --workspace --all-targets -- -D warnings.",
  ],
  documentation: [
    "Run cargo doc --workspace --no-deps when docs change.",
    "Run cargo test --doc --workspace if doctests/examples are added or changed.",
  ],
};

const implementationItems: ImplementationItem[] = packageIds.flatMap((packageId) =>
  categoryIds.map((categoryId) => ({
    packageId,
    categoryId,
    title: `${packageId} ${categoryId} fixes`,
    primaryFiles: packagePrimaryFiles[packageId],
    validationHints: categoryValidationHints[categoryId],
  })),
);

const inputSchema = z.object({
  reportPath: z.string().default("artifacts/rust-idiom-package-audit.md"),
  extraContext: z.string().default(""),
  dryRun: z.boolean().default(false),
});

const scopeSchema = z.object({
  reportPath: z.string(),
  summary: z.string(),
  actionableBySlice: z.array(z.object({
    packageId: z.enum(packageIds),
    categoryId: z.enum(categoryIds),
    findingIds: z.array(z.string()).default([]),
    recommendedChanges: z.array(z.string()).default([]),
    risk: z.enum(["high", "medium", "low", "none"]),
  })).default([]),
  skippedSlices: z.array(z.object({
    packageId: z.enum(packageIds),
    categoryId: z.enum(categoryIds),
    reason: z.string(),
  })).default([]),
  globalConstraints: z.array(z.string()).default([]),
});

const implementationResultSchema = z.object({
  packageId: z.enum(packageIds),
  categoryId: z.enum(categoryIds),
  status: z.enum(["completed", "partial", "blocked", "skipped"]),
  summary: z.string(),
  findingIdsAddressed: z.array(z.string()).default([]),
  filesChanged: z.array(z.string()).default([]),
  testsAddedOrUpdated: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  blockers: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
  committed: z.boolean().default(false),
});

const validationSchema = z.object({
  allPassed: z.boolean(),
  summary: z.string(),
  commandsRun: z.array(z.object({
    command: z.string(),
    status: z.enum(["passed", "failed", "skipped"]),
    details: z.string().nullable().default(null),
  })).default([]),
  failures: z.array(z.string()).default([]),
  skipped: z.array(z.string()).default([]),
});

const finalSchema = z.object({
  status: z.enum(["completed", "partial", "blocked"]),
  summary: z.string(),
  completed: z.array(z.string()).default([]),
  blocked: z.array(z.string()).default([]),
  skipped: z.array(z.string()).default([]),
  validationPassed: z.boolean(),
  nextActions: z.array(z.string()).default([]),
  markdownBody: z.string(),
});

const { Workflow, smithers, outputs } = createSmithers({
  input: inputSchema,
  scope: scopeSchema,
  implementation: implementationResultSchema,
  validation: validationSchema,
  final: finalSchema,
});

function slug(item: ImplementationItem): string {
  return `${item.packageId}:${item.categoryId}`;
}

function nodeId(item: ImplementationItem): string {
  return `rust-idiom-implement:${slug(item)}`;
}

function priorNodeId(index: number): string {
  return index === 0 ? "rust-idiom-implement:scope" : nodeId(implementationItems[index - 1]);
}

function priorDeps(index: number) {
  return index === 0 ? { prior: outputs.scope } : { prior: outputs.implementation };
}

function formatList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

function scopePrompt(reportPath: string, extraContext: string, dryRun: boolean): string {
  return `Read the verified Rust idiom audit report and plan implementation slices.

Report path: ${reportPath}
Dry run: ${dryRun ? "true" : "false"}

Rules:
- Read AGENTS.md and the report before planning.
- Only verified, actionable findings should be implemented.
- Preserve the report's package/category boundaries.
- Prefer clean breaks over compatibility shims unless the report explicitly says otherwise.
- Surface high-risk public API changes as blockers or follow-ups rather than guessing.
- Do not push or open a PR from inside this workflow.

${extraContext ? `Extra context:\n${extraContext}\n` : ""}

Return actionableBySlice for every package/category slice that has verified actionable work, skippedSlices for the rest, and global constraints every implementation task must preserve.`;
}

function implementationPrompt(
  item: ImplementationItem,
  index: number,
  reportPath: string,
  extraContext: string,
  dryRun: boolean,
  prior: unknown,
): string {
  return `Implement one verified Rust idiom report slice in daml-tools.

Slice: ${item.title}
Package: ${item.packageId}
Category: ${item.categoryId}
Report path: ${reportPath}
Dry run: ${dryRun ? "true" : "false"}

Primary files/directories to inspect first:
${formatList(item.primaryFiles)}

Validation hints:
${formatList(item.validationHints)}

Prior workflow output to preserve:
${JSON.stringify(prior, null, 2)}

Execution rules:
- Read AGENTS.md, ${reportPath}, package exports, immediate callers, and existing tests before editing.
- If the report has no verified actionable findings for this slice, return status "skipped" and do not edit files.
- If dryRun is true, do not edit files; return the exact patch plan and blockers.
- Touch only files needed for this slice plus immediate tests/docs.
- Prefer small, surgical, clean-break changes. Do not add compatibility shims unless explicitly justified by the report.
- Add or update tests/docs that encode the intent of the change.
- Run relevant tests/checks for the slice when practical.
- If changes are made and the slice reaches a working state, create a small focused git commit following repo Conventional Commit rules. Do not push or open a PR.
- If uncertain or blocked on a public API compatibility decision, use smithers ask-human rather than guessing.

${extraContext ? `Extra context:\n${extraContext}\n` : ""}

Return packageId, categoryId, status, summary, findingIdsAddressed, filesChanged, testsAddedOrUpdated, commandsRun, blockers, followUps, and committed.`;
}

function allImplementationNeeds(): Record<string, string> {
  return Object.fromEntries(implementationItems.map((item) => [slug(item), nodeId(item)]));
}

function allImplementationDeps(): Record<string, typeof implementationResultSchema> {
  return Object.fromEntries(implementationItems.map((item) => [slug(item), outputs.implementation]));
}

function validationPrompt(results: unknown[], reportPath: string, extraContext: string): string {
  return `Validate all Rust idiom implementation slices.

Report path: ${reportPath}
Implementation results:
${JSON.stringify(results, null, 2)}

Run these checks unless impossible; never silently skip:
1. cargo fmt --all -- --check
2. cargo clippy --workspace --all-targets -- -D warnings
3. cargo test --workspace
4. cargo doc --workspace --no-deps
5. git status --short --branch

If a check fails, diagnose whether it was caused by this workflow. Fix only workflow-caused failures. If skipped, record the exact reason.

${extraContext ? `Extra context:\n${extraContext}\n` : ""}

Return allPassed, summary, commandsRun with status, failures, and skipped.`;
}

function finalPrompt(results: unknown[], validation: unknown, reportPath: string): string {
  return `Synthesize the final implementation status.

Report path: ${reportPath}
Implementation results:
${JSON.stringify(results, null, 2)}

Validation:
${JSON.stringify(validation, null, 2)}

Return status, summary, completed/blocked/skipped slice IDs, validationPassed, nextActions, and markdownBody suitable for a PR description.`;
}

export default smithers((ctx) => {
  const reportPath = ctx.input.reportPath ?? "artifacts/rust-idiom-package-audit.md";
  const extraContext = ctx.input.extraContext ?? "";
  const dryRun = ctx.input.dryRun ?? false;

  return (
    <Workflow name="rust-idiom-package-implement">
      <Sequence>
        <Task
          id="rust-idiom-implement:scope"
          output={outputs.scope}
          agent={agents.smart}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {scopePrompt(reportPath, extraContext, dryRun)}
        </Task>

        {implementationItems.map((item, index) => (
          <Task
            key={slug(item)}
            id={nodeId(item)}
            output={outputs.implementation}
            agent={agents.smartTool}
            needs={{ prior: priorNodeId(index) }}
            deps={priorDeps(index)}
            timeoutMs={3_600_000}
            heartbeatTimeoutMs={900_000}
            continueOnFail
          >
            {(deps: { prior: unknown }) => implementationPrompt(
              item,
              index,
              reportPath,
              extraContext,
              dryRun,
              deps.prior,
            )}
          </Task>
        ))}

        <Task
          id="rust-idiom-implement:validate"
          output={outputs.validation}
          agent={agents.smart}
          needs={allImplementationNeeds()}
          deps={allImplementationDeps()}
          timeoutMs={3_600_000}
          heartbeatTimeoutMs={900_000}
        >
          {(deps: Record<string, unknown>) => validationPrompt(
            implementationItems.map((item) => deps[slug(item)]),
            reportPath,
            extraContext,
          )}
        </Task>

        <Task
          id="rust-idiom-implement:final"
          output={outputs.final}
          agent={agents.smart}
          needs={{ validation: "rust-idiom-implement:validate", ...allImplementationNeeds() }}
          deps={{ validation: outputs.validation, ...allImplementationDeps() }}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {(deps: Record<string, unknown>) => finalPrompt(
            implementationItems.map((item) => deps[slug(item)]),
            deps.validation,
            reportPath,
          )}
        </Task>
      </Sequence>
    </Workflow>
  );
});
