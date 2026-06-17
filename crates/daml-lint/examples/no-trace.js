// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.
const NAME = "no-trace";
const SEVERITY = "low";
const DESCRIPTION = "Debug trace left in code";
function check(m) {
  const source = m.source.replace(/\{-[\s\S]*?-\}/g, (s) => s.replace(/[^\n]/g, " "));
  source.split("\n").forEach((line, idx) => {
    const code = line.split("--")[0].replace(/"(\\.|[^"\\])*"/g, '""');
    if (/\btrace(Raw|Id|State)?\b/.test(code)) {
      report(idx + 1, "Remove debug trace calls before deploying");
    }
  });
}
