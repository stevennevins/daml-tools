// Consuming choices should have at least one controller who is a signatory.
// Needs cross-referencing two AST node lists (controllers vs signatories).
// Compile: npx esbuild consuming-choice-signatory-controller.ts --outfile=consuming-choice-signatory-controller.js

const NAME = "consuming-choice-signatory-controller";
const SEVERITY = "medium";
const DESCRIPTION = "Consuming choices should have at least one signatory controller";

function on_choice(choice: Choice, template: Template): void {
  if (!choice.consuming) {
    return;
  }
  // `controller signatory this` is signatory-controlled by definition; it
  // serializes as exactly "signatory this" (also the leading element of a
  // multi-controller `signatory this, obs`). Match the `signatory <expr>` form
  // by its trailing space so an ordinary party field named e.g. `signatoryParty`
  // is NOT mistaken for the flexible-controller keyword.
  if (
    choice.controllers.some(
      (c) => c === "signatory this" || c.startsWith("signatory ") || template.signatories.includes(c),
    )
  ) {
    return;
  }
  report(choice, `Consuming choice '${choice.name}' has no signatory among its controllers`);
}
