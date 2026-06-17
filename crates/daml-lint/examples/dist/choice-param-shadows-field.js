// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.

// examples/choice-param-shadows-field.ts
var NAME = "choice-param-shadows-field";
var SEVERITY = "medium";
var DESCRIPTION = "Choice parameters must not shadow template field names";
function on_choice(choice, template) {
  const fieldNames = new Set(template.fields.map((f) => f.name));
  for (const param of choice.parameters) {
    if (fieldNames.has(param.name)) {
      report(
        param,
        `Parameter '${param.name}' of choice '${choice.name}' shadows a field of template '${template.name}'`
      );
    }
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
