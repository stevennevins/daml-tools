import { expressionHasSizeUpperBound, isUnboundedType, typeDisplay } from "./_helpers";

const NAME = "unbounded-fields";
const SEVERITY = "medium";
const DESCRIPTION = "Unbounded Text/TextMap/List field with no ensure clause bounding its size";

function on_template(template: Template): void {
  const unboundedFields = template.fields.filter((field) => isUnboundedType(field.type_));
  if (unboundedFields.length === 0) return;

  const fieldNames = template.fields.map((field) => field.name);
  const unguardedNames: string[] = [];

  for (const field of unboundedFields) {
    const hasBound = template.ensure_clause !== null
      && expressionHasSizeUpperBound(template.ensure_clause.expr, field.name, fieldNames);
    if (!hasBound) unguardedNames.push(field.name);
  }

  if (unguardedNames.length === 0) return;

  const typeDesc = unguardedNames.length === 1
    ? `${typeDisplay(unboundedFields.find((field) => field.name === unguardedNames[0])?.type_ ?? null)} field`
    : "fields";

  report(
    template,
    `Template '${template.name}' has unbounded ${typeDesc} '${unguardedNames.join("', '")}' with no ensure clause bounding their length.`,
    `Fields without size bounds: ${unguardedNames.join(", ")}`,
  );
}

// QuickJS discovers rule metadata and visitors by evaluating these names.
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_template };
