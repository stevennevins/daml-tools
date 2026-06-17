// Compiled from TypeScript; pass this JavaScript file to daml-lint --rules.
const NAME = "no-bare-contractid-field";
const SEVERITY = "low";
const DESCRIPTION = "Template fields holding ContractIds risk dangling references";
function containsContractId(ty) {
  if (ty === null) {
    return false;
  }
  if ("App" in ty) {
    return isCon(ty.App.head, "ContractId") || containsContractId(ty.App.head) || ty.App.args.some(containsContractId);
  }
  if ("List" in ty) {
    return containsContractId(ty.List.inner);
  }
  if ("Tuple" in ty) {
    return ty.Tuple.items.some(containsContractId);
  }
  if ("Fun" in ty) {
    return containsContractId(ty.Fun.param) || containsContractId(ty.Fun.result);
  }
  if ("Constrained" in ty) {
    return containsContractId(ty.Constrained.body);
  }
  return false;
}
function isCon(ty, name) {
  return "Con" in ty && ty.Con.name === name;
}
function on_field(field, template) {
  if (containsContractId(field.type_)) {
    report(
      field,
      `Field '${field.name}' on template '${template.name}' stores a ContractId \u2014 consider a contract key lookup to avoid dangling references`
    );
  }
}
