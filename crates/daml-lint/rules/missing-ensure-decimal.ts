import type { Template } from "../examples/daml-lint";
import { expressionGuaranteesNonnegative, isMoneyType } from "./_helpers";

const NAME = "missing-ensure-decimal";
const SEVERITY = "high";
const DESCRIPTION = "Template has Decimal field with no positivity bound in its ensure clause";

function on_template(template: Template): void {
  const decimalFields = template.fields.filter((field) => isMoneyType(field.type_));
  for (const field of decimalFields) {
    const hasBound = template.ensure_clause !== null
      && expressionGuaranteesNonnegative(template.ensure_clause.expr, field.name);
    if (hasBound) continue;

    const evidence = template.ensure_clause === null
      ? `${field.name} : Decimal  -- no ensure clause found`
      : `${field.name} : Decimal  -- ensure clause does not bound this field`;
    report(
      template,
      `Template '${template.name}' has Decimal field '${field.name}' with no positivity bound (e.g. \`${field.name} > 0\`) in its ensure clause.`,
      evidence,
    );
  }
}

// QuickJS discovers rule metadata and visitors by evaluating these names.
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_template };
