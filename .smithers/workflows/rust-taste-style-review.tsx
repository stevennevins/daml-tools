// smithers-source: local
// smithers-display-name: Rust Taste/Style Review
// smithers-description: Run deterministic Rust validation commands, then review the diff for idiom and type/API quality with gpt-5.3-codex-spark.
// smithers-tags: rust,review,quality
/** @jsxImportSource smithers-orchestrator */
import { createSmithers } from "smithers-orchestrator";
import { z } from "zod/v4";
import {
  RustTasteStyleReview,
  rustDiffOutputSchema,
  rustTasteStyleReviewOutputSchema,
  rustValidationOutputSchema,
} from "../components/RustTasteStyleReview";

const inputSchema = z.object({
  context: z
    .string()
    .default(
      "Review the current Rust diff for idiom, type/API quality, tests, docs, and scope.",
    ),
  commands: z
    .array(z.string())
    .default([
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features",
    ]),
  diffCommand: z
    .string()
    .default(
      "git diff --stat && git diff -- . ':!.smithers/node_modules' ':!node_modules'",
    ),
});

const { Workflow, smithers } = createSmithers({
  input: inputSchema,
  rustValidation: rustValidationOutputSchema,
  rustDiff: rustDiffOutputSchema,
  rustTasteStyleReview: rustTasteStyleReviewOutputSchema,
});

export default smithers((ctx) => (
  <Workflow name="rust-taste-style-review">
    <RustTasteStyleReview
      idPrefix="rust-review"
      context={
        ctx.input.context ??
        "Review the current Rust diff for idiom, type/API quality, tests, docs, and scope."
      }
      commands={
        ctx.input.commands ?? [
          "cargo fmt --all -- --check",
          "cargo clippy --workspace --all-targets --all-features -- -D warnings",
          "cargo test --workspace --all-features",
        ]
      }
      diffCommand={
        ctx.input.diffCommand ??
        "git diff --stat && git diff -- . ':!.smithers/node_modules' ':!node_modules'"
      }
    />
  </Workflow>
));
