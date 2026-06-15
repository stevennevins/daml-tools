// Compiled from no-trace.ts — this is the file you pass to --rules.

const NAME = "no-trace";
const SEVERITY = "low";
const DESCRIPTION = "Debug trace left in code";

function check(m) {
  m.source.split("\n").forEach((line, idx) => {
    // Match `trace`/`traceRaw`/`traceId`/`traceState` as whole identifiers
    // (not `retraceCount`), ignoring line comments.
    const code = line.split("--")[0];
    if (/\btrace(Raw|Id|State)?\b/.test(code)) {
      report(idx + 1, "Remove debug trace calls before deploying");
    }
  });
}
