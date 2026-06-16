// Compiled from consuming-choice-signatory-controller.ts — this is the file you pass to --rules.

const NAME = "consuming-choice-signatory-controller";
const SEVERITY = "medium";
const DESCRIPTION = "Consuming choices should have at least one signatory controller";

function on_choice(choice, template) {
  if (!choice.consuming) {
    return;
  }
  const signatories = partyExprs(template.signatory_exprs).map(exprText);
  if (partyExprs(choice.controller_exprs).some((c) => {
    const text = exprText(c);
    return text === "signatory this" || text.startsWith("signatory ") || signatories.includes(text);
  })) {
    return;
  }
  report(choice, `Consuming choice '${choice.name}' has no signatory among its controllers`);
}

function partyExprs(exprs) {
  return exprs.flatMap((e) => "List" in e ? e.List.items : [e]);
}

function exprText(e) {
  if ("Var" in e) {
    const v = e.Var;
    return v.qualifier === null ? v.name : `${v.qualifier}.${v.name}`;
  }
  if ("Con" in e) {
    const c = e.Con;
    return c.qualifier === null ? c.name : `${c.qualifier}.${c.name}`;
  }
  if ("App" in e) {
    return [exprText(e.App.func), ...e.App.args.map(exprText)].join(" ");
  }
  if ("Unknown" in e) {
    return e.Unknown.raw;
  }
  return "";
}
