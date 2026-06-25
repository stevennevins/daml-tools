// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: Rust Idiom Package Audit
// smithers-description: Review each Rust package/category pair for idiomatic Rust API, error, interoperability, and documentation gaps, then verify and report findings.
// smithers-tags: daml, rust, audit, api-design
/** @jsxImportSource smithers-orchestrator */
import { createSmithers, Parallel, Sequence, Task } from "smithers-orchestrator";
import { dirname } from "node:path";
import { mkdirSync, writeFileSync } from "node:fs";
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

type Finding = z.infer<typeof findingSchema>;
type PackageCategoryAudit = z.infer<typeof packageCategoryAuditSchema>;

function isFinding(value: unknown): value is Finding {
  if (typeof value !== "object" || value === null) return false;
  const maybe = value as Partial<Finding>;
  return typeof maybe.id === "string"
    && typeof maybe.packageId === "string"
    && typeof maybe.categoryId === "string"
    && typeof maybe.file === "string"
    && typeof maybe.principle === "string"
    && typeof maybe.evidence === "string"
    && typeof maybe.recommendation === "string"
    && typeof maybe.actionable === "boolean";
}

function asAudit(value: unknown): PackageCategoryAudit {
  return packageCategoryAuditSchema.parse(value);
}

function uniqueById(findings: Finding[]): Finding[] {
  const seen = new Set<string>();
  const deduped: Finding[] = [];
  for (const finding of findings) {
    if (seen.has(finding.id)) continue;
    seen.add(finding.id);
    deduped.push(finding);
  }
  return deduped;
}

function severityRank(finding: Finding): number {
  return { high: 0, medium: 1, low: 2, info: 3 }[finding.severity] ?? 4;
}

function findingLocation(finding: Finding): string {
  const suffix = finding.line == null ? "" : `:${finding.line}`;
  const symbol = finding.symbol ? ` (${finding.symbol})` : "";
  return `${finding.file}${suffix}${symbol}`;
}

function findingMarkdown(finding: Finding): string {
  return [
    `### ${finding.id} — ${finding.severity.toUpperCase()} — ${finding.packageId} / ${finding.categoryId}`,
    "",
    `- Location: \`${findingLocation(finding)}\``,
    `- Actionable: ${finding.actionable ? "yes" : "no"}`,
    `- Confidence: ${finding.confidence}`,
    `- Principle: ${finding.principle}`,
    `- Evidence: ${finding.evidence}`,
    `- Recommendation: ${finding.recommendation}`,
    "",
  ].join("\n");
}

function buildReport(results: unknown[], reportPath: string) {
  const audits = results.map(asAudit);
  const allFindings = uniqueById(audits.flatMap((audit) => audit.findings.filter(isFinding)));
  const sortedFindings = [...allFindings].sort((a, b) =>
    severityRank(a) - severityRank(b)
      || a.packageId.localeCompare(b.packageId)
      || a.categoryId.localeCompare(b.categoryId)
      || a.id.localeCompare(b.id),
  );
  const highPriorityFindings = sortedFindings.filter((finding) => finding.severity === "high");
  const actionableFindings = sortedFindings.filter((finding) => finding.actionable);
  const nonActionableFindings = sortedFindings.filter((finding) => !finding.actionable);

  const bySlice = reviewItems.map((item) => {
    const findings = sortedFindings.filter(
      (finding) => finding.packageId === item.packageId && finding.categoryId === item.categoryId,
    );
    return { item, findings };
  });

  const markdownBody = [
    "# Rust Idiom Package Audit",
    "",
    "Generated by the Smithers `rust-idiom-package-audit` workflow.",
    "",
    "## Summary",
    "",
    `- Package/category tasks: ${audits.length}`,
    `- Findings: ${sortedFindings.length}`,
    `- Actionable findings: ${actionableFindings.length}`,
    `- High-priority findings: ${highPriorityFindings.length}`,
    `- Blocked/partial slices: ${audits.filter((audit) => audit.status !== "completed").length}`,
    "",
    "## Actionable implementation slices",
    "",
    ...bySlice.flatMap(({ item, findings }) => {
      const actionable = findings.filter((finding) => finding.actionable);
      return [
        `### ${item.packageId} / ${item.categoryId}`,
        "",
        actionable.length === 0
          ? "- No verified actionable findings from the audit stage."
          : actionable.map((finding) => `- ${finding.id} (${finding.severity}, ${finding.confidence}): ${finding.recommendation}`).join("\n"),
        "",
      ];
    }),
    "## High-priority findings",
    "",
    highPriorityFindings.length === 0
      ? "No high-priority findings were reported."
      : highPriorityFindings.map(findingMarkdown).join("\n"),
    "",
    "## All findings",
    "",
    sortedFindings.length === 0
      ? "No findings were reported."
      : sortedFindings.map(findingMarkdown).join("\n"),
    "",
    "## Package/category summaries",
    "",
    ...audits.map((audit) => [
      `### ${audit.packageId} / ${audit.categoryId}`,
      "",
      `- Status: ${audit.status}`,
      `- Findings: ${audit.findings.length}`,
      `- Files inspected: ${audit.filesInspected.length}`,
      audit.blockers.length === 0 ? "- Blockers: none" : `- Blockers: ${audit.blockers.join("; ")}`,
      "",
      audit.summary,
      "",
    ].join("\n")),
  ].join("\n");

  mkdirSync(dirname(reportPath), { recursive: true });
  writeFileSync(reportPath, markdownBody);

  return {
    reportPath,
    summary: `Audited ${audits.length} package/category slices and found ${sortedFindings.length} findings (${actionableFindings.length} actionable).`,
    highPriorityFindings,
    actionableFindings,
    nonActionableFindings,
    markdownBody,
    commandsRun: [`wrote ${reportPath}`],
  };
}

function verificationPrompt(reportPath: string, extraContext: string): string {
  return `Verify the Rust idiom audit report before implementation.

Report path: ${reportPath}

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
          needs={allAuditNeeds()}
          deps={allAuditDeps()}
          timeoutMs={1_800_000}
          heartbeatTimeoutMs={600_000}
        >
          {(deps: Record<string, unknown>) => buildReport(
            reviewItems.map((item) => deps[slug(item)]),
            reportPath,
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
          {() => verificationPrompt(reportPath, extraContext)}
        </Task>
      </Sequence>
    </Workflow>
  );
});
