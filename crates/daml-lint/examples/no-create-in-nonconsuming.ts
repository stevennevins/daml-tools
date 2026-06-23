import type { Choice, Statement, Template } from "./daml-lint";
import { walkBodyStatements } from "../rules/_helpers";

// Nonconsuming choices that create contracts can be exercised repeatedly on
// the same contract, fanning out unbounded copies. Walks choice body
// statements, recursing into try/catch blocks.
// Compile: npx esbuild examples/no-create-in-nonconsuming.ts --bundle --outfile=examples/dist/no-create-in-nonconsuming.js

const NAME = "no-create-in-nonconsuming";
const SEVERITY = "medium";
const DESCRIPTION = "Nonconsuming choices should not create contracts";

function creates(stmts: Statement[]): boolean {
  let found = false;
  walkBodyStatements(stmts, (stmt) => {
    if ("Create" in stmt) {
      found = true;
    }
  });
  return found;
}

function on_choice(choice: Choice, template: Template): void {
  if (choice.consuming === "non-consuming" && creates(choice.body)) {
    report(
      choice,
      `Nonconsuming choice '${choice.name}' on template '${template.name}' creates contracts — repeated exercise fans out unbounded copies`
    );
  }
}

globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
