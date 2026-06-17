import type { DamlLintRuleModule } from "../dist/index";

// @ts-expect-error A rule module must expose at least one visitor hook.
const missingVisitor: DamlLintRuleModule = {
  NAME: "missing-visitor",
  SEVERITY: "low",
};

void missingVisitor;
