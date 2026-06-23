import type { Choice, Expr, Template } from "./daml-lint";
import { renderText } from "../rules/_helpers";

// Consuming choices should have at least one controller who is a signatory.
// Needs cross-referencing two structured AST node lists.
// Compile: npx esbuild examples/consuming-choice-signatory-controller.ts --bundle --outfile=examples/dist/consuming-choice-signatory-controller.js

const NAME = "consuming-choice-signatory-controller";
const SEVERITY = "medium";
const DESCRIPTION = "Consuming choices should have at least one signatory controller";

function on_choice(choice: Choice, template: Template): void {
  if (choice.consuming === "non-consuming") {
    return;
  }
  const signatories = partyExprs(template.signatory_exprs).map(renderText);
  if (partyExprs(choice.controller_exprs).some((c) => {
    const text = renderText(c);
    return text === "signatory this" || text.startsWith("signatory ") || signatories.includes(text);
  })) {
    return;
  }
  report(choice, `Consuming choice '${choice.name}' has no signatory among its controllers`);
}

function partyExprs(exprs: Expr[]): Expr[] {
  return exprs.flatMap((e) => ("List" in e ? e.List.items : [e]));
}

globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
