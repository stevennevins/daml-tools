#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
const pluginName = packageJson.name.startsWith("daml-lint-plugin-")
  ? packageJson.name.slice("daml-lint-plugin-".length)
  : packageJson.name;
const ruleIds = Object.keys(packageJson.damlLint?.rules ?? {}).map(
  (rule) => `${pluginName}/${rule}`,
);

function runLint(fixture) {
  const result = spawnSync(
    "npx",
    ["daml-lint", fixture, "--format", "json", "--fail-on", "info"],
    {
      encoding: "utf8",
      shell: process.platform === "win32",
    },
  );

  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }

  let findings = [];
  if (result.stdout.trim()) {
    try {
      const payload = JSON.parse(result.stdout);
      findings = Array.isArray(payload.findings) ? payload.findings : [];
    } catch (error) {
      console.error(`Failed to parse daml-lint JSON output for ${fixture}:`);
      console.error(result.stdout);
      console.error(error);
      process.exit(1);
    }
  }

  return {
    status: result.status ?? 1,
    findings,
    stderr: result.stderr ?? "",
  };
}

function assertRuleFindings(label, result) {
  const reportedRuleIds = new Set(result.findings.map((finding) => finding.detector));
  const missing = ruleIds.filter((ruleId) => !reportedRuleIds.has(ruleId));

  if (missing.length > 0) {
    console.error(`${label} is missing findings for: ${missing.join(", ")}`);
    console.error(result.stderr);
    console.error(result.findings);
    process.exit(1);
  }
}

const violations = runLint("fixtures/violations.daml");
if (violations.status !== 1) {
  console.error("fixtures/violations.daml should report custom-rule findings and exit 1.");
  console.error(violations.stderr);
  console.error(violations.findings);
  process.exit(1);
}
assertRuleFindings("fixtures/violations.daml", violations);

const clean = runLint("fixtures/clean.daml");
const customFindings = clean.findings.filter((finding) => ruleIds.includes(finding.detector));
if (clean.status !== 0 || customFindings.length > 0) {
  console.error("fixtures/clean.daml should not report custom-rule findings.");
  console.error(clean.stderr);
  console.error(customFindings);
  process.exit(1);
}

console.log(`Verified ${ruleIds.length} custom rules against violation and clean fixtures.`);
