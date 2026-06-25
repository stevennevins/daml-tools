// smithers-source: local
// smithers-metadata-version: 1
// smithers-display-name: Test Style Migration
// smithers-description: Migrate tests toward integration-style coverage with src unit tests reserved for internal contracts.
// smithers-tags: daml, rust, testing, migration
/** @jsxImportSource smithers-orchestrator */
import { createSmithers } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const workItemIds = [
  "daml-parser-diagnostics-recovery",
  "daml-parser-spans-projection",
  "daml-parser-internal-unit-boundary",
  "daml-syntax-source-api",
  "daml-syntax-coordinate-contracts",
  "daml-lint-parser-ir-contracts",
  "daml-lint-corpus-adversarial",
  "daml-lint-custom-rule-runtime-contracts",
  "daml-lint-internal-unit-boundary",
  "daml-fmt-library-behavior",
  "daml-fmt-layout-fixtures",
  "daml-fmt-internal-unit-boundary",
] as const;

type WorkItemId = typeof workItemIds[number];

type WorkItem = {
  id: WorkItemId;
  packageName: "daml-parser" | "daml-syntax" | "daml-lint" | "daml-fmt";
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
    id: "daml-parser-diagnostics-recovery",
    packageName: "daml-parser",
    category: "diagnostics-and-recovery",
    title: "Move parser diagnostic and recovery behavior tests out of src",
    objective:
      "Migrate diagnostic/recovery behavior tests that use public parse_module APIs from src-only modules into daml-parser integration tests.",
    primaryFiles: [
      "crates/daml-parser/src/diag_tests.rs",
      "crates/daml-parser/src/parse.rs",
      "crates/daml-parser/src/lib.rs",
      "crates/daml-parser/tests/",
    ],
    targetShape: [
      "Create focused integration test files under crates/daml-parser/tests/ for public diagnostic/recovery behavior.",
      "Keep src-local tests only when they require private parser helpers or intentionally pin private parser internals.",
      "Remove migrated src test module wiring from src/lib.rs when no longer needed.",
    ],
    constraints: [
      "Do not expose private parser functions only to make tests move.",
      "Preserve diagnostic category/message intent; avoid broad parser behavior changes.",
      "If a test cannot move through public APIs, leave it in src and record why in the task output.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-parser --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-parser-spans-projection",
    packageName: "daml-parser",
    category: "span-losslessness-and-projection",
    title: "Move parser span/losslessness/projection behavior tests to integration tests",
    objective:
      "Migrate span oracle, projection precedence, and AST public-shape behavior tests that operate through public parser APIs into daml-parser integration tests.",
    primaryFiles: [
      "crates/daml-parser/src/span_tests.rs",
      "crates/daml-parser/src/projection_tests.rs",
      "crates/daml-parser/src/ast_span.rs",
      "crates/daml-parser/src/ast.rs",
      "crates/daml-parser/tests/",
    ],
    targetShape: [
      "Add integration tests for render_from_ast, span tightness, and projection precedence where public APIs suffice.",
      "Keep only true ast_span/private-helper unit tests in src.",
      "Preserve corpus guards and fail-loud CI behavior when corpus files are expected.",
    ],
    constraints: [
      "Do not weaken span exactness assertions to make migration easy.",
      "Do not relocate vendored corpus data.",
      "Keep parser and formatter crate ownership boundaries intact.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-parser --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-parser-internal-unit-boundary",
    packageName: "daml-parser",
    category: "internal-unit-contracts",
    title: "Prune daml-parser src tests to explicit internal contracts",
    objective:
      "Review remaining lexer/layout/parse src tests and leave only narrow tests for private phase contracts or low-level token/layout invariants.",
    primaryFiles: [
      "crates/daml-parser/src/lexer.rs",
      "crates/daml-parser/src/layout.rs",
      "crates/daml-parser/src/parse.rs",
      "crates/daml-parser/src/ast.rs",
      "crates/daml-parser/src/ast_span.rs",
    ],
    targetShape: [
      "Keep lexer tokenization/trivia and layout virtual-token tests in src when they pin implementation phase contracts.",
      "Move any remaining externally observable parse behavior to integration tests.",
      "Add comments only where needed to explain why a src test remains unit-style.",
    ],
    constraints: [
      "Do not churn working low-level tests just for file location purity.",
      "Avoid broad renames or fixture rewrites.",
      "Keep comments surgical; do not annotate every test mechanically.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-parser --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-syntax-source-api",
    packageName: "daml-syntax",
    category: "public-source-api",
    title: "Move daml-syntax SourceFile/SourceTokens public API tests to integration tests",
    objective:
      "Migrate daml-syntax tests for SourceFile, SourceTokens, diagnostics, and parser span conversion into integration tests that exercise the public crate API.",
    primaryFiles: [
      "crates/daml-syntax/src/lib.rs",
      "crates/daml-syntax/tests/",
      "crates/daml-syntax/src/coordinate.rs",
    ],
    targetShape: [
      "Add crates/daml-syntax/tests/source_api.rs or similarly focused integration tests.",
      "Leave src tests only for private LineIndex implementation details that cannot be observed publicly.",
      "Keep rustdoc examples aligned with moved public API coverage.",
    ],
    constraints: [
      "Do not change public SourceFile behavior while moving tests.",
      "Do not add compatibility shims for old test paths.",
      "Keep assertions on diagnostics accessors and range errors intact.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-syntax --locked",
      "cargo test --doc -p daml-syntax --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-syntax-coordinate-contracts",
    packageName: "daml-syntax",
    category: "coordinate-contracts",
    title: "Move coordinate public-contract checks to integration/compile-style tests",
    objective:
      "Convert coordinate newtype public API checks into integration tests, and use compile-style coverage for non-interchangeability if practical.",
    primaryFiles: [
      "crates/daml-syntax/src/coordinate.rs",
      "crates/daml-syntax/src/lib.rs",
      "crates/daml-syntax/Cargo.toml",
      "crates/daml-syntax/tests/",
    ],
    targetShape: [
      "Integration tests cover one-based coordinate constructors and TextSize conversions through public exports.",
      "If compile-fail infrastructure is added, keep it minimal and crate-local.",
      "If compile-fail is too much churn, record that the runtime public tests are the completed slice and leave type-safety compile coverage as follow-up.",
    ],
    constraints: [
      "Do not add a new dev dependency unless the compile-fail value justifies it.",
      "Do not alter coordinate semantics.",
      "Keep public API construction clean and explicit.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-syntax --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-lint-parser-ir-contracts",
    packageName: "daml-lint",
    category: "parser-ir-contracts",
    title: "Move daml-lint parser lowering and IR contract tests to integration tests",
    objective:
      "Migrate tests that parse Daml source and assert rule-facing IR shape from src/parser.rs into daml-lint integration tests using public parse_daml_with_diagnostics.",
    primaryFiles: [
      "crates/daml-lint/src/parser.rs",
      "crates/daml-lint/src/ir.rs",
      "crates/daml-lint/src/lib.rs",
      "crates/daml-lint/tests/",
    ],
    targetShape: [
      "Public IR contract behavior lives under crates/daml-lint/tests/.",
      "src/parser.rs keeps only private lower_* unit tests if any are truly needed.",
      "Test helper construction uses parse_daml_with_diagnostics rather than cfg(test)-only parse_daml when possible.",
    ],
    constraints: [
      "Do not make parse_daml public just for tests; use existing public ParseResult API.",
      "Do not weaken structured TypeNode/Expr assertions.",
      "Preserve no-default-feature behavior where relevant.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cargo test -p daml-lint --no-default-features --lib --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-lint-corpus-adversarial",
    packageName: "daml-lint",
    category: "corpus-and-adversarial-integration",
    title: "Move daml-lint corpus and adversarial behavior tests out of src",
    objective:
      "Relocate corpus-backed and hostile-input parser/lowering behavior tests from src modules to integration tests.",
    primaryFiles: [
      "crates/daml-lint/src/adversarial_tests.rs",
      "crates/daml-lint/src/corpus_tests.rs",
      "crates/daml-lint/src/lib.rs",
      "crates/daml-lint/tests/",
      "corpus/daml-finance/",
    ],
    targetShape: [
      "Integration tests retain the existing corpus-present guard and CI fail-loud behavior.",
      "Hostile-input tests remain end-to-end through public parsing/lowering APIs.",
      "src/lib.rs no longer wires these broad integration-style modules once moved.",
    ],
    constraints: [
      "Do not delete or reshape corpus facts during relocation.",
      "Do not hide skipped corpus tests under CI.",
      "Keep performance guard intent, but avoid introducing flaky timing thresholds beyond current behavior.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-lint-custom-rule-runtime-contracts",
    packageName: "daml-lint",
    category: "custom-rule-runtime-contracts",
    title: "Split custom rule runtime tests between integration behavior and private runtime unit tests",
    objective:
      "Move JS rule visitor/runtime-surface behavior tests to integration tests while keeping private runtime safety tests in src/detectors/script.rs.",
    primaryFiles: [
      "crates/daml-lint/src/detectors/script.rs",
      "crates/daml-lint/tests/",
      "crates/daml-lint/examples/",
      "crates/daml-lint/lint-plugin/",
      "crates/daml-lint/package.json",
    ],
    targetShape: [
      "Integration tests cover script-visible node kinds, generated .d.ts contract, and shipped example rule behavior through public/CLI surfaces where practical.",
      "src/detectors/script.rs keeps tests for private load_script_source, interrupt counters, and low-level runtime error attribution.",
      "Generated rule artifacts stay in sync if tests require npm generation.",
    ],
    constraints: [
      "Respect feature gates: custom-rule tests must compile under the intended feature combinations.",
      "Do not broaden the JS runtime public API to enable test relocation.",
      "If a test must remain source-local because it uses private runtime hooks, document that in the task output rather than forcing a public escape hatch.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cd crates/daml-lint && npm ci && npm run check:rules",
      "cargo test -p daml-lint --no-default-features --features cli,js-runtime,custom-rules --locked",
    ],
    riskLevel: "high",
  },
  {
    id: "daml-lint-internal-unit-boundary",
    packageName: "daml-lint",
    category: "internal-unit-contracts",
    title: "Prune daml-lint src tests to explicit detector/config/reporter contracts",
    objective:
      "Review remaining daml-lint src tests and keep only focused internal contracts for config parsing, reporter formatting internals, detector wrappers, and private runtime hooks.",
    primaryFiles: [
      "crates/daml-lint/src/config.rs",
      "crates/daml-lint/src/reporter.rs",
      "crates/daml-lint/src/detector.rs",
      "crates/daml-lint/src/detectors/script.rs",
      "crates/daml-lint/tests/cli.rs",
    ],
    targetShape: [
      "src tests are narrow and clearly tied to private/internal contracts.",
      "Externally observable CLI/reporting behavior remains covered by tests under crates/daml-lint/tests/.",
      "No broad formatting or detector rewrites are bundled into this cleanup.",
    ],
    constraints: [
      "Do not move private formatter function tests if doing so would require making private helpers public.",
      "Do not duplicate the same assertion in both src and integration tests unless it guards different failure modes.",
      "Preserve existing feature-gated test behavior.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-lint --all-features --locked",
      "cargo test -p daml-lint --no-default-features --lib --locked",
    ],
    riskLevel: "low",
  },
  {
    id: "daml-fmt-library-behavior",
    packageName: "daml-fmt",
    category: "library-api-behavior",
    title: "Move daml-fmt public library behavior tests to integration tests",
    objective:
      "Migrate format_source, try_format_source, diagnostics, FormatOptions, and coverage public behavior tests from src/lib.rs into daml-fmt integration tests.",
    primaryFiles: [
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-fmt/tests/",
      "crates/daml-fmt/src/layout_ast.rs",
    ],
    targetShape: [
      "Public formatter API behavior is asserted from crates/daml-fmt/tests/.",
      "src/lib.rs keeps only private helper tests such as normalize_final_newline if still needed.",
      "Corpus/span-oracle tests live where ownership is clearest and remain fail-loud under CI.",
    ],
    constraints: [
      "Do not intentionally change formatter output.",
      "Do not remove malformed-input passthrough/rejection coverage.",
      "Avoid broad fixture churn; move tests before improving them.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked",
    ],
    riskLevel: "medium",
  },
  {
    id: "daml-fmt-layout-fixtures",
    packageName: "daml-fmt",
    category: "layout-formatting-fixtures",
    title: "Move formatter layout examples into integration-style fixture tests",
    objective:
      "Relocate broad layout_ast formatting examples from implementation-local tests to black-box integration tests driven by public formatter APIs or fixture tables.",
    primaryFiles: [
      "crates/daml-fmt/src/layout_ast.rs",
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-fmt/tests/",
      "crates/daml-fmt/corpus/",
      "crates/daml-fmt/test/diff.js",
    ],
    targetShape: [
      "Given-input/expected-output formatter behavior is exercised under crates/daml-fmt/tests/ or fixtures, not inside layout_ast.rs.",
      "Private helper tests remain in layout_ast.rs only for helper-specific behavior.",
      "Idempotence expectations stay explicit in integration tests for cases where that is the business rule.",
    ],
    constraints: [
      "Do not rewrite the formatter backend as part of test relocation.",
      "Do not weaken idempotence or exact-output assertions.",
      "Keep npm differential coverage as a separate external corpus gate.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked",
      "cd crates/daml-fmt && npm test",
    ],
    riskLevel: "high",
  },
  {
    id: "daml-fmt-internal-unit-boundary",
    packageName: "daml-fmt",
    category: "internal-unit-contracts",
    title: "Prune daml-fmt src tests to private helper contracts",
    objective:
      "Review remaining daml-fmt src tests and leave only tests for private helpers such as line/comment detection, indentation helpers, import organization internals, and newline normalization.",
    primaryFiles: [
      "crates/daml-fmt/src/lib.rs",
      "crates/daml-fmt/src/layout_ast.rs",
      "crates/daml-fmt/tests/",
    ],
    targetShape: [
      "src tests are small, helper-specific, and not duplicate black-box formatter tests.",
      "Public API behavior is covered from tests/.",
      "Any retained src test has a clear private-contract reason in the task output.",
    ],
    constraints: [
      "Do not expose private layout helpers for tests.",
      "Do not change formatter output intentionally.",
      "Keep cleanup surgical after the larger fixture migration.",
    ],
    validationCommands: [
      "cargo fmt --all -- --check",
      "cargo test -p daml-fmt --all-features --locked",
    ],
    riskLevel: "low",
  },
];

const scopeOutputSchema = z.looseObject({
  summary: z.string().default("Test style migration scoped."),
  assumptions: z.array(z.string()).default([]),
  risks: z.array(z.string()).default([]),
  executionNotes: z.array(z.string()).default([]),
  workingStateDefinition: z.string().default("Each item validates before the next item starts."),
});

const itemResultSchema = z.looseObject({
  id: z.enum(workItemIds).default("daml-parser-diagnostics-recovery"),
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
  id: z.enum(workItemIds).default("daml-parser-diagnostics-recovery"),
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
  return `Plan and prepare the daml-tools test style migration.

Goal:
Move broad externally observable behavior tests toward integration-style tests under crate tests/ directories. Keep unit-style tests in src only for specific private/internal contracts.

Behavior contract:
- Read AGENTS.md and this repo's test/contribution docs before changing files in later tasks.
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

Return scope assumptions, risks, execution notes, and the definition of a working state that later tasks must preserve.`;
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
- If blocked or uncertain, stop and report blockers; do not guess.
- Commit the focused update if the repo is in a working state, following Conventional Commit rules.
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
