import type { DamlLintRuleModule, Template } from "@daml-tools/lint-plugin";

const NAME = "template-requires-ensure";
const SEVERITY = "medium";
const DESCRIPTION = "Every template must declare an ensure clause";

function on_template(template: Template): void {
  if (template.ensure_clause === null) {
    report(template, `Template '${template.name}' has no ensure clause`);
  }
}

const rule: DamlLintRuleModule = { NAME, SEVERITY, DESCRIPTION, on_template };
globalThis.__daml_lint_rule = rule;
