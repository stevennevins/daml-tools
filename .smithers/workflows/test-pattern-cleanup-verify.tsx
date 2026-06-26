// smithers-source: local
// smithers-display-name: Test Pattern Cleanup Verify
// smithers-description: Run a surgical Rust cleanup pass and command verification over the current test-pattern diff.
// smithers-tags: rust,testing,cleanup,verification
/** @jsxImportSource smithers-orchestrator */
import { createSmithers, Sequence, Task } from "smithers-orchestrator";
import { z } from "zod/v4";
import { agents } from "../agents";

const DEFAULT_CONTEXT =
  "Verify and clean up the current test-pattern changes for real Daml compiler fixtures, corpus presence checks, and snapshot/golden-style expectations.";

const DEFAULT_COMMANDS = [
  "cargo fmt --all -- --check",
  "cargo test --workspace --locked",
  "cargo clippy --workspace --all-targets --all-features --locked",
  "cd crates/daml-fmt && npm test",
  "git diff --check",
  "test ! -e crates/daml-lint/test-fixtures",
  "test -f crates/daml-lint/tests/fixtures/good_patterns.daml",
  "test -f crates/daml-lint/tests/fixtures/bad_patterns.daml",
  "test -d corpus/daml-finance",
] as const;

const inputSchema = z.object({
  context: z.string().default(DEFAULT_CONTEXT),
  commands: z.array(z.string()).default([...DEFAULT_COMMANDS]),
});

const cleanupOutputSchema = z.object({
  status: z.enum(["completed", "partial", "blocked"]),
  summary: z.string(),
  filesChanged: z.array(z.string()).default([]),
  commandsRun: z.array(z.string()).default([]),
  risksOrBlockers: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
});

const commandResultSchema = z.object({
  command: z.string(),
  status: z.enum(["passed", "failed", "skipped"]),
  details: z.string().nullable().default(null),
});

const validationOutputSchema = z.object({
  allPassed: z.boolean(),
  commands: z.array(commandResultSchema),
  failures: z.array(z.string()).default([]),
  skipped: z.array(z.string()).default([]),
  status: z.string(),
  diffSummary: z.string(),
  nameStatus: z.string(),
});

const reviewOutputSchema = z.object({
  approved: z.boolean(),
  summary: z.string(),
  issues: z.array(z.object({
    priority: z.enum(["P0", "P1", "P2"]),
    file: z.string().nullable().default(null),
    summary: z.string(),
    suggestedFix: z.string().nullable().default(null),
  })).default([]),
  validationNotes: z.array(z.string()).default([]),
  followUps: z.array(z.string()).default([]),
});

const { Workflow, smithers, outputs } = createSmithers({
  input: inputSchema,
  cleanup: cleanupOutputSchema,
  validation: validationOutputSchema,
  review: reviewOutputSchema,
});

function cleanupPrompt(context: string) {
  return `Run a surgical cleanup pass over the current daml-tools test-pattern diff.

Context:
${context}

Scope:
- Review the current git diff and untracked files.
- Keep the first-step organization intent intact: daml-lint fixtures belong under crates/daml-lint/tests/fixtures, not test-fixtures.
- Keep compiler/corpus fixture verification fail-loud in CI and skippable only for published/off-workspace contexts.
- Preserve golden/surface expectation tests when they encode public behavior; do not convert them to brittle inline implementation checks.
- Avoid broad refactors, compatibility shims, or speculative cleanup.
- Do not refactor unrelated test utilities or temp-file cleanup patterns unless a required validation command proves they are broken.
- Do not edit crates/daml-fmt/src/config.rs or add source-local unit tests; this workflow is scoped to fixture/corpus/golden verification cleanup.

Required cleanup behavior:
- Fix formatting or obviously accidental artifacts if present.
- Confirm each untracked source/fixture file is intentional; remove only stale scratch files.
- Prefer deterministic commands for formatting/checks; do not push or open a PR.
- If blocked or unsure, stop and report blockers instead of guessing.

Return JSON matching the schema with status, summary, filesChanged, commandsRun, risksOrBlockers, and followUps.`;
}

function validationPrompt(
  context: string,
  commands: readonly string[],
  cleanup: z.infer<typeof cleanupOutputSchema>,
) {
  return `Run the verification command suite for the current daml-tools test-pattern diff.

Context:
${context}

Cleanup output to verify:
\`\`\`json
${JSON.stringify(cleanup, null, 2)}
\`\`\`

Required commands, in order; do not silently skip:
${commands.map((command, index) => `${index + 1}. ${command}`).join("\n")}

Also capture:
- git status --short
- git diff --stat
- git diff --name-status plus untracked files from git ls-files --others --exclude-standard

Validation rules:
- Do not edit files in this validation node.
- Record a command as skipped only if it is genuinely impossible, and explain why.
- If any command fails, include the failing command and the important tail of output in failures/details.
- Treat dirty git status as expected for the feature diff; fail only for unexpected scratch/runtime artifacts or command failures.
- Flag accidental .smithers runtime churn, but allow .smithers/workflows/test-pattern-cleanup-verify.tsx because this workflow is part of the requested verification work.

Return JSON matching the schema: allPassed, commands, failures, skipped, status, diffSummary, and nameStatus.`;
}

function reviewPrompt(
  context: string,
  cleanup: z.infer<typeof cleanupOutputSchema>,
  validation: z.infer<typeof validationOutputSchema>,
) {
  return `Review the cleanup and verification results for the current Rust test-pattern diff. Do not edit files.

Context:
${context}

Cleanup output:
\`\`\`json
${JSON.stringify(cleanup, null, 2)}
\`\`\`

Validation output:
\`\`\`json
${JSON.stringify(validation, null, 2)}
\`\`\`

Review rubric:
- Approve only if validation.allPassed is true.
- Check that fixture/corpus missing-file behavior fails loudly in CI without breaking published crate contexts.
- Check that golden/snapshot-style expectations are used where they document public CLI/source surfaces, not private implementation details.
- Flag accidental .smithers runtime/source churn except for this workflow file.
- Flag unnecessary abstractions, broad refactors, or weakened tests.

Return JSON matching the schema.`;
}

export default smithers((ctx) => (
  <Workflow name="test-pattern-cleanup-verify">
    <Sequence>
      <Task
        id="test-pattern-cleanup:cleanup"
        output={outputs.cleanup}
        agent={agents.smartTool}
        timeoutMs={1_800_000}
        heartbeatTimeoutMs={600_000}
        retries={1}
      >
        {cleanupPrompt(ctx.input.context ?? DEFAULT_CONTEXT)}
      </Task>

      <Task
        id="test-pattern-cleanup:validation"
        output={outputs.validation}
        agent={agents.smart}
        needs={{ cleanup: "test-pattern-cleanup:cleanup" }}
        deps={{ cleanup: outputs.cleanup }}
        timeoutMs={7_200_000}
        heartbeatTimeoutMs={900_000}
        retries={1}
      >
        {(deps) => validationPrompt(
          ctx.input.context ?? DEFAULT_CONTEXT,
          ctx.input.commands ?? [...DEFAULT_COMMANDS],
          deps.cleanup,
        )}
      </Task>

      <Task
        id="test-pattern-cleanup:review"
        output={outputs.review}
        agent={agents.smart}
        needs={{
          cleanup: "test-pattern-cleanup:cleanup",
          validation: "test-pattern-cleanup:validation",
        }}
        deps={{
          cleanup: outputs.cleanup,
          validation: outputs.validation,
        }}
        timeoutMs={1_800_000}
        heartbeatTimeoutMs={600_000}
        retries={1}
      >
        {(deps) => reviewPrompt(ctx.input.context ?? DEFAULT_CONTEXT, deps.cleanup, deps.validation)}
      </Task>
    </Sequence>
  </Workflow>
));
