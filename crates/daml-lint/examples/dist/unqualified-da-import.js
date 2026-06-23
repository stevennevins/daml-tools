// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.

// examples/unqualified-da-import.ts
var NAME = "unqualified-da-import";
var SEVERITY = "low";
var DESCRIPTION = "DA collection modules should be imported qualified";
var SHADOW_PRONE = ["DA.Map", "DA.TextMap", "DA.Set", "DA.List", "DA.Optional"];
function on_import(imp) {
  if (imp.qualified === "unqualified" && SHADOW_PRONE.includes(imp.module_name)) {
    report(imp, `Import '${imp.module_name}' unqualified \u2014 its names shadow Prelude; use 'import qualified ${imp.module_name}'`);
  }
}
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_import };
