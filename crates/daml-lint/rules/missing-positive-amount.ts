import type { Choice, Expr, Statement, Template } from "../examples/daml-lint";
import {
  expressionGuaranteesNonempty,
  expressionGuaranteesStrictPositive,
  expressionHasSizeCeilingBound,
  forEachSubexpr,
  isListType,
  isMoneyType,
  isStrictPositiveBound,
  isZeroLiteral,
  refersTo,
  walkBodyExprs,
  walkBodyStatements,
  walkUnconditionalStatements,
} from "./_helpers";

const NAME = "missing-positive-amount";
const SEVERITY = "high";
const DESCRIPTION = "Choice accepts amount/transfer parameter without positive-value or non-empty check";

function hasPositiveAmountCheck(body: Statement[], param: string): boolean {
  let found = false;
  walkUnconditionalStatements(body, (statement) => {
    if ("Assert" in statement && expressionGuaranteesStrictPositive(statement.Assert.condition_expr, param)) {
      found = true;
    }
  });
  return found || hasDefensiveAmountGuard(body, param);
}

function hasDefensiveAmountGuard(body: Statement[], param: string): boolean {
  let found = false;
  walkBodyExprs(body, (expr) => {
    if (found || !("App" in expr) || !("Var" in expr.App.func) || expr.App.func.Var.qualifier !== null) return;
    const [condition, action] = expr.App.args;
    if (condition === undefined || action === undefined) return;
    if (expr.App.func.Var.name === "when" && isNonpositiveTest(condition, param) && aborts(action)) {
      found = true;
    }
    if (expr.App.func.Var.name === "unless" && isStrictPositiveTest(condition, param) && aborts(action)) {
      found = true;
    }
  });
  if (found) return true;

  walkBodyStatements(body, (statement) => {
    if (!("Branch" in statement) || statement.Branch.scrutinee === null) return;
    const { scrutinee, arms } = statement.Branch;
    if (arms.length !== 2 || !arms.every((arm) => arm.pattern === null)) return;
    if ((isNonpositiveTest(scrutinee, param) && bodyAborts(arms[0].body))
      || (isStrictPositiveTest(scrutinee, param) && bodyAborts(arms[1].body))) {
      found = true;
    }
  });
  return found;
}

function bodyAborts(body: Statement[]): boolean {
  let found = false;
  walkBodyExprs(body, (expr) => {
    if (aborts(expr)) found = true;
  });
  return found;
}

function isNonpositiveTest(condition: Expr, param: string): boolean {
  if ("BinOp" in condition) {
    const { op, lhs, rhs } = condition.BinOp;
    if (op === "<=") return refersTo(lhs, param) && isZeroLiteral(rhs);
    if (op === ">=") return refersTo(rhs, param) && isZeroLiteral(lhs);
    return false;
  }
  return "App" in condition
    && "Var" in condition.App.func
    && condition.App.func.Var.name === "not"
    && condition.App.args.length === 1
    && isStrictPositiveTest(condition.App.args[0], param);
}

function isStrictPositiveTest(condition: Expr, param: string): boolean {
  return isStrictPositiveBound(condition, param);
}

function aborts(action: Expr): boolean {
  let found = false;
  forEachSubexpr(action, (expr) => {
    if ("Var" in expr
      && expr.Var.qualifier === null
      && (expr.Var.name === "abort" || expr.Var.name === "error" || expr.Var.name === "fail" || expr.Var.name === "assertFail")) {
      found = true;
    }
  });
  return found;
}

function hasNonemptyListCheck(body: Statement[], param: string): boolean {
  let found = false;
  walkBodyStatements(body, (statement) => {
    if ("Assert" in statement && expressionGuaranteesNonempty(statement.Assert.condition_expr, param)) {
      found = true;
    }
  });
  return found;
}

function hasMaxOnlyCountCheck(body: Statement[], field: string): boolean {
  let hasUpper = false;
  let hasLower = false;
  walkBodyStatements(body, (statement) => {
    if (!("Assert" in statement)) return;
    if (expressionHasSizeCeilingBound(statement.Assert.condition_expr, field)) hasUpper = true;
    if (expressionGuaranteesNonempty(statement.Assert.condition_expr, field)) hasLower = true;
  });
  return hasUpper && !hasLower;
}

function on_choice(choice: Choice, _template: Template): void {
  const amountParams = choice.parameters.filter((param) => {
    const lowerName = param.name.toLowerCase();
    return isMoneyType(param.type_)
      && (lowerName.includes("amount") || lowerName === "quantity" || lowerName === "price");
  });

  for (const param of amountParams) {
    if (hasPositiveAmountCheck(choice.body, param.name)) continue;
    report(
      choice,
      `Choice '${choice.name}' accepts Decimal parameter '${param.name}' without asserting > 0.`,
      `${param.name} : Decimal  -- no positive-amount check`,
    );
  }

  const listParams = choice.parameters.filter((param) => {
    const lowerName = param.name.toLowerCase();
    return isListType(param.type_)
      && (lowerName.includes("input") || lowerName.includes("holding") || lowerName.includes("cids"));
  });

  for (const param of listParams) {
    if (hasNonemptyListCheck(choice.body, param.name)) continue;
    report(
      choice,
      `Choice '${choice.name}' accepts list parameter '${param.name}' but has no minimum-length check.`,
      `No 'not $ null ${param.name}' or min-length check`,
    );
  }

  for (const field of ["transfer.inputHoldingCids", "transfer.inputs"]) {
    if (!hasMaxOnlyCountCheck(choice.body, field)) continue;
    report(
      choice,
      `Choice '${choice.name}' checks max input count but not min. Empty inputs allowed.`,
      `Bounds '${field}' from above but never asserts it is non-empty`,
    );
  }
}

// QuickJS discovers rule metadata and visitors by evaluating these names.
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
