// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.
const NAME = "function-ledger-actions";
const SEVERITY = "info";
const DESCRIPTION = "Top-level functions performing archive/exercise \u2014 verify authorization is audited";
function ledgerActions(stmts) {
  const found = [];
  for (const stmt of stmts) {
    if ("Archive" in stmt) {
      found.push("archive");
    }
    if ("Exercise" in stmt) {
      found.push("exercise");
    }
    if ("TryCatch" in stmt) {
      const tc = stmt.TryCatch;
      found.push(...ledgerActions(tc.try_body), ...ledgerActions(tc.catch_body));
    }
    if ("Branch" in stmt) {
      const br = stmt.Branch;
      for (const arm of br.arms) {
        found.push(...ledgerActions(arm.body));
      }
    }
  }
  return found;
}
function on_function(fn) {
  const actions = ledgerActions(fn.body);
  if (actions.length > 0) {
    report(fn, `Function '${fn.name}' performs ledger actions (${actions.join(", ")}) outside a choice`);
  }
}
