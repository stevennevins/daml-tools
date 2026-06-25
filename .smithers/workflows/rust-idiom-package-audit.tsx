// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: Rust Idiom Package Audit
// smithers-description: Review each Rust package/category pair for idiomatic Rust API, error, interoperability, and documentation gaps, then verify and report findings.
// smithers-tags: daml, rust, audit, api-design
/** @jsxImportSource smithers-orchestrator */
import { createSmithers, Parallel, Sequence, Task } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const packageIds = ["daml-parser", "daml-syntax", "daml-lint", "daml-fmt"] as const;
const categoryIds = ["type-safety", "error-handling", "interoperability", "documentation"] as const;

type PackageId = typeof packageIds[number];
type CategoryId = typeof categoryIds[number];

type PackageReview = {
  packageId: PackageId;
  packagePath: string;
  categoryId: CategoryId;
  categoryFocus: string;
  primaryFiles: string[];
};

const categoryFocus: Record<CategoryId, string> = {
  "type-safety": [
    "newtypes for distinct concepts and invariants",
    "custom enums/structs instead of ambiguous bool/Option/string/usize public arguments",
    "bitflags for true flag sets",
    "builders for complex construction",
    "avoid mixing byte, char, UTF-16, line, and column spaces",
  ].join("; "),
  "error-handling": [
    "Result for expected/recoverable failures",
    "documented public panic contracts only for impossible states or caller contract violations",
    "meaningful typed errors implementing Error/Display/Send/Sync",
    "avoid stringly nested errors and Result<_, ()>",
    "preserve source chains where useful",
  ].join("; "),
  interoperability: [
    "common trait impls/derives for public types where semantically valid",
    "standard conversion traits From/TryFrom/AsRef/AsMut instead of ad-hoc helpers",
    "Display for user-facing enums/errors",
    "FromIterator/Extend for custom collection types where applicable",
    "reader/writer APIs take R: Read / W: Write by value where applicable",
  ].join("; "),
  documentation: [
    "crate-level docs and feature docs",
    "field/variant contract docs for public DTOs and AST/IR shapes",
    "documented errors and panics",
    "realistic examples/doctests where useful",
    "docs must match current behavior",
  ].join("; "),
};

const packagePrimaryFiles: Record<PackageId, string[]> = {
  "daml-parser": [
    "crates/daml-parser/src/lib.rs",
    "crates/daml-parser/src/ast.rs",
    "crates/daml-parser/src/lexer.rs",
    "crates/daml-parser/src/parse.rs",
    "crates/daml-parser/src/ast_span.rs",
    "crates/daml-parser/README.md",
    "crates/daml-parser/Cargo.toml",
  ],
  "daml-syntax": [
    "crates/daml-syntax/src/lib.rs",
    "crates/daml-syntax/src/coordinate.rs",
    "crates/daml-syntax/README.md",
    "crates/daml-syntax/Cargo.toml",
  ],
  "daml-lint": [
    "crates/daml-lint/src/lib.rs",
    "crates/daml-lint/src/detector.rs",
    "crates/daml-lint/src/ir.rs",
    "crates/daml-lint/src/parser.rs",
    "crates/daml-lint/src/reporter.rs",
    "crates/daml-lint/src/config.rs",
    "crates/daml-lint/src/detectors/mod.rs",
    "crates/daml-lint/src/detectors/script.rs",
    "crates/daml-lint/README.md",
    "crates/daml-lint/Cargo.toml",
  ],
  "daml-fmt": [
    "crates/daml-fmt/src/lib.rs",
    "crates/daml-fmt/src/layout_ast.rs",
    "crates/daml-fmt/src/bin",
    "crates/daml-fmt/tests",
    "crates/daml-fmt/README.md",
    "crates/daml-fmt/Cargo.toml",
  ],
};

const reviewItems: PackageReview[] = packageIds.flatMap((packageId) =>
  categoryIds.map((categoryId) => ({
    packageId,
    packagePath: `crates/${packageId}`,
    categoryId,
    categoryFocus: categoryFocus[categoryId],
    primaryFiles: packagePrimaryFiles[packageId],
  })),
);

const inputSchema = z.object({
  reportPath: z.string().default("artifacts/rust-idiom-package-audit.md"),
  extraContext: z.string().default(""),
  maxConcurrency: z.number().int().default(4),
});

const findingSchema = z.object({
  id: z.string(),
  severity: z.enum(["high", "medium", "low", "info"]),
  packageId: z.enum(packageIds),
  categoryId: z.enum(categoryIds),
  file: z.string(),
  line: z.number().int().nullable().default(null),
  symbol: z.string().nullable().default(null),
  principle: z.string(),
  evidence: z.string(),
  recommendation: z.string(),
  actionable: z.boolean(),
  confidence: z.enum(["high", "medium", "low"]),
});

const packageCategoryAuditSchema = z.object({
  packageId: z.enum(packageIds),
  categoryId: z.enum(categoryIds),
  status: z.enum(["completed", "partial", "blocked"]),
  summary: z.string(),
  filesInspected: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  findings: z.array(findingSchema).default([]),
  blockers: z.array(z.string()).default([]),
});

const reportSchema = z.object({
  reportPath: z.string(),
  summary: z.string(),
  highPriorityFindings: z.array(findingSchema).default([]),
  actionableFindings: z.array(findingSchema).default([]),
  nonActionableFindings: z.array(findingSchema).default([]),
  markdownBody: z.string(),
  commandsRun: z.array(z.string()).default([]),
});

const verificationSchema = z.object({
  verified: z.boolean(),
  summary: z.string(),
  checkedFindings: z.array(z.object({
    findingId: z.string(),
    status: z.enum(["verified", "rejected", "needs-human", "not-checked"]),
    evidence: z.string(),
  })).default([]),
  rejectedFindingIds: z.array(z.string()).default([]),
  remainingUncertainty: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  reportPath: z.string(),
});

const { Workflow, smithers, outputs } = createSmithers({
  input: inputSchema,
  packageCategoryAudit: packageCategoryAuditSchema,
  report: reportSchema,
  verification: verificationSchema,
});

function slug(item: PackageReview): string {
  return `${item.packageId}:${item.categoryId}`;
}

function nodeId(item: PackageReview): string {
  return `rust-idiom-audit:${slug(item)}`;
}

function formatList(items: string[]): string {
  return items.map((item) => `- ${item}`).join("\n");
}

function auditPrompt(item: PackageReview, extraContext: string): string {
  return `Read-only Rust idiom audit for one package/category pair in daml-tools.

Package: ${item.packageId}
Package path: ${item.packagePath}
Category: ${item.categoryId}
Category focus: ${item.categoryFocus}

Primary files/directories to inspect first:
${formatList(item.primaryFiles)}

Required source-backed principles:
- Rust API Guidelines type safety: custom domain types/newtypes, no ambiguous bool/Option/primitives where types should encode meaning.
- Rust Book error handling: return Result for expected/recoverable failure; panic only for impossible states/tests/examples/contract violations.
- Rust API Guidelines interoperability: common traits, standard conversion traits, meaningful error types.
- Rust API Guidelines documentation: public contracts, examples, errors, panics, features.

Rules:
- Do not edit files.
- Inspect exports, immediate callers, tests, and docs before reporting a finding.
- Avoid speculative findings. If evidence is weak, lower confidence or omit it.
- Distinguish actionable code/doc changes from non-actionable observations.
- Treat test-only unwrap/expect as acceptable unless it hides the behavior under test.
- Use absolute or repo-relative file paths and line numbers where practical.

${extraContext ? `Extra context:\n${extraContext}\n` : ""}

Return a structured audit for only ${item.packageId} / ${item.categoryId}.`;
}

function allAuditNeeds(): Record<string, string> {
  return Object.fromEntries(reviewItems.map((item) => [slug(item), nodeId(item)]));
}

function allAuditDeps(): Record<string, typeof packageCategoryAuditSchema> {
  return Object.fromEntries(reviewItems.map((item) => [slug(item), outputs.packageCategoryAudit]));
}

function reportPrompt(results: unknown[], reportPath: string, extraContext: string): string {
  return `Synthesize the Rust idiom package/category audit results into a durable report.

Report path to write: ${reportPath}

Inputs:
${JSON.stringify(results, null, 2)}

Report requirements:
- Create or update ${reportPath} with the markdown report.
- Preserve package and category boundaries.
- Deduplicate findings that appear in multiple categories.
- Prioritize high-confidence actionable changes.
- Include a section mapping actionable findings to implementation slices by package/category.
- Include commands run and any blockers or uncertainty.
- Do not edit source code other than writing the report artifact.

${extraContext ? `Extra context:\n${extraContext}\n` : ""}

Return the reportPath, summary, highPriorityFindings, actionableFindings, nonActionableFindings, markdownBody, and commandsRun.`;
}

function verificationPrompt(report: unknown, reportPath: string, extraContext: string): string {
  return `Verify the Rust idiom audit report before implementation.

Report path: ${reportPath}
Report output:
${JSON.stringify(report, null, 2)}

Verification rules:
- Read ${reportPath} and sample-check every high-priority finding plus at least one actionable finding from each package/category where findings exist.
- Use deterministic search tools and code inspection. Run cargo clippy --workspace --all-targets -- -D warnings if practical.
- Reject findings that are unsupported, stale, or contradicted by code.
- Do not edit source code. If the report file itself contains incorrect findings, update only ${reportPath} to mark or remove rejected findings.
- If verification cannot be completed, set verified=false and explain exactly why.

${extraContext ? `Extra context:\n${extraContext}\n` : ""}

Return whether the report is verified, checked finding statuses, rejected IDs, remaining uncertainty, commandsRun, and reportPath.`;
}

export default smithers((ctx) => {
  const reportPath = ctx.input.reportPath ?? "artifacts/rust-idiom-package-audit.md";
  const extraContext = ctx.input.extraContext ?? "";
  const maxConcurrency = ctx.input.maxConcurrency ?? 4;

  return (
    <Workflow name="rust-idiom-package-audit">
      <Sequence>
        <Parallel maxConcurrency={maxConcurrency}>
          {reviewItems.map((item) => (
            <Task
              key={slug(item)}
              id={nodeId(item)}
              output={outputs.packageCategoryAudit}
              agent={agents.smart}
              timeoutMs={1_800_000}
              heartbeatTimeoutMs={600_000}
              continueOnFail
            >
              {auditPrompt(item, extraContext)}
            </Task>
          ))}
        </Parallel>

        <Task
          id="rust-idiom-audit:report"
          output={outputs.report}
          agent={agents.smartTool}
          needs={allAuditNeeds()}
          deps={allAuditDeps()}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {(deps: Record<string, unknown>) => reportPrompt(
            reviewItems.map((item) => deps[slug(item)]),
            reportPath,
            extraContext,
          )}
        </Task>

        <Task
          id="rust-idiom-audit:verify"
          output={outputs.verification}
          agent={agents.smart}
          needs={{ report: "rust-idiom-audit:report" }}
          deps={{ report: outputs.report }}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {(deps: { report: unknown }) => verificationPrompt(deps.report, reportPath, extraContext)}
        </Task>
      </Sequence>
    </Workflow>
  );
});
