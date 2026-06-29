import type { DamlLintRuleModule, Import } from "@daml-tools/lint-plugin";

const NAME = "unqualified-da-import";
const SEVERITY = "low";
const DESCRIPTION = "DA collection modules should be imported qualified";

const SHADOW_PRONE = ["DA.Map", "DA.TextMap", "DA.Set", "DA.List", "DA.Optional"];

function on_import(imp: Import): void {
  if (imp.qualified === "unqualified" && SHADOW_PRONE.includes(imp.module_name)) {
    report(
      imp,
      `Import '${imp.module_name}' unqualified — its names shadow Prelude; use 'import qualified ${imp.module_name}'`,
    );
  }
}

const rule: DamlLintRuleModule = { NAME, SEVERITY, DESCRIPTION, on_import };
globalThis.__daml_lint_rule = rule;
