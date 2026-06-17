import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

const checkOnly = process.argv.includes("--check");
const sourcePath = "examples/daml-lint.d.ts";
const targetPath = "lint-plugin/dist/index.d.ts";

const sourceHeader = `// TypeScript contract for daml-lint custom rule authoring.
//
// Examples and built-in rules use these types while authoring TypeScript.
// Bundle the rule to JavaScript before passing it to daml-lint --rules.
// Node shapes mirror src/ir.rs.`;

const targetHeader = `// Type definitions for daml-lint custom rule authoring packages.
//
// Import these types from the published package when writing external custom
// rules, compile TypeScript to bundled JavaScript, and pass the .js file to
// daml-lint --rules. Node shapes mirror daml-lint src/ir.rs.`;

const renames = [
  ["RuleVisitorModule", "DamlLintRuleVisitorModule"],
  ["RuleVisitor", "DamlLintRuleVisitor"],
  ["RuleSeverity", "DamlLintRuleSeverity"],
  ["RuleModule", "DamlLintRuleModule"],
  ["ReportTarget", "DamlLintReportTarget"],
];

function replaceIdentifier(text, from, to) {
  return text.replace(new RegExp(`\\b${from}\\b`, "g"), to);
}

function generatedTypes() {
  let output = readFileSync(sourcePath, "utf8");
  if (!output.includes(sourceHeader)) {
    throw new Error(`${sourcePath} header changed; update tools/sync-lint-plugin-types.mjs`);
  }
  output = output.replace(sourceHeader, targetHeader);
  for (const [from, to] of renames) {
    output = replaceIdentifier(output, from, to);
  }
  return output;
}

const expected = generatedTypes();

if (checkOnly) {
  const current = readFileSync(targetPath, "utf8");
  if (current !== expected) {
    console.error(`${targetPath} is out of sync with ${sourcePath}`);
    console.error("Run `npm run build:lint-plugin-types` from crates/daml-lint.");
    process.exit(1);
  }
} else if (!existsSync(targetPath) || readFileSync(targetPath, "utf8") !== expected) {
  mkdirSync(dirname(targetPath), { recursive: true });
  writeFileSync(targetPath, expected);
}
