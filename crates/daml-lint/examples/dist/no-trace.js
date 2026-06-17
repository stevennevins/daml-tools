// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.

// examples/no-trace.ts
var NAME = "no-trace";
var SEVERITY = "low";
var DESCRIPTION = "Debug trace left in code";
function check(m) {
  const source = m.source.replace(/\{-[\s\S]*?-\}/g, (s) => s.replace(/[^\n]/g, " "));
  source.split("\n").forEach((line, idx) => {
    const code = line.split("--")[0].replace(/"(\\.|[^"\\])*"/g, '""');
    if (/\btrace(Raw|Id|State)?\b/.test(code)) {
      report(idx + 1, "Remove debug trace calls before deploying");
    }
  });
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, check };
