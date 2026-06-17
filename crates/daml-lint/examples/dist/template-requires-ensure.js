// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.

// examples/template-requires-ensure.ts
var NAME = "template-requires-ensure";
var SEVERITY = "medium";
var DESCRIPTION = "Every template must declare an ensure clause";
function on_template(template) {
  if (template.ensure_clause === null) {
    report(template, `Template '${template.name}' has no ensure clause`);
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_template };
