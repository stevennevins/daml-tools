// Flag leftover debug trace calls — a simple banned-token rule written
// as a whole-module check over the raw source.
// Compile: npx esbuild no-trace.ts --outfile=no-trace.js

const NAME = "no-trace";
const SEVERITY = "low";
const DESCRIPTION = "Debug trace left in code";

function check(m: DamlModule): void {
  m.source.split("\n").forEach((line, idx) => {
    // Match `trace`/`traceRaw`/`traceId`/`traceState` as whole identifiers
    // (not `retraceCount`), ignoring line comments.
    const code = line.split("--")[0];
    if (/\btrace(Raw|Id|State)?\b/.test(code)) {
      report(idx + 1, "Remove debug trace calls before deploying");
    }
  });
}
