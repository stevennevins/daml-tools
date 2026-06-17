import type { DamlModule } from "./daml-lint";

// Flag leftover debug trace calls — a simple banned-token rule written
// as a whole-module check over the raw source.
// Compile: npx esbuild examples/no-trace.ts --bundle --outfile=examples/dist/no-trace.js

const NAME = "no-trace";
const SEVERITY = "low";
const DESCRIPTION = "Debug trace left in code";

function check(m: DamlModule): void {
  // Blank out `{- ... -}` block comments first, replacing every non-newline
  // character with a space so the trace token vanishes while line numbers stay
  // aligned. (Non-nested; Daml allows nesting, but a single pass clears the
  // common case.)
  const source = m.source.replace(/\{-[\s\S]*?-\}/g, (s) => s.replace(/[^\n]/g, " "));
  source.split("\n").forEach((line, idx) => {
    // Strip line comments, then blank out double-quoted string literals so a
    // `trace` word inside text (e.g. "please trace this") is not flagged.
    const code = line.split("--")[0].replace(/"(\\.|[^"\\])*"/g, '""');
    // Match `trace`/`traceRaw`/`traceId`/`traceState` as whole identifiers
    // (not `retraceCount`).
    if (/\btrace(Raw|Id|State)?\b/.test(code)) {
      report(idx + 1, "Remove debug trace calls before deploying");
    }
  });
}

globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, check };
