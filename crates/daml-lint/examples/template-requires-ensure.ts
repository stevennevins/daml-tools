import type { Template } from "./daml-lint";

// Every template must declare an ensure clause.
// Compile: npx esbuild examples/template-requires-ensure.ts --bundle --outfile=examples/dist/template-requires-ensure.js

const NAME = "template-requires-ensure";
const SEVERITY = "medium";
const DESCRIPTION = "Every template must declare an ensure clause";

function on_template(template: Template): void {
  if (template.ensure_clause === null) {
    report(template, `Template '${template.name}' has no ensure clause`);
  }
}

globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_template };
