// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.
const NAME = "no-create-in-nonconsuming";
const SEVERITY = "medium";
const DESCRIPTION = "Nonconsuming choices should not create contracts";
function creates(stmts) {
  for (const stmt of stmts) {
    if ("Create" in stmt) {
      return true;
    }
    if ("TryCatch" in stmt) {
      const tc = stmt.TryCatch;
      if (creates(tc.try_body) || creates(tc.catch_body)) {
        return true;
      }
    }
    if ("Branch" in stmt) {
      const br = stmt.Branch;
      if (br.arms.some((arm) => creates(arm.body))) {
        return true;
      }
    }
  }
  return false;
}
function on_choice(choice, template) {
  if (!choice.consuming && creates(choice.body)) {
    report(
      choice,
      `Nonconsuming choice '${choice.name}' on template '${template.name}' creates contracts \u2014 repeated exercise fans out unbounded copies`
    );
  }
}
