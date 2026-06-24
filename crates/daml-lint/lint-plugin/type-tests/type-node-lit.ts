import type { SourceSpan, TypeNode } from "../dist/index";

const span: SourceSpan = {
  file: "Test.daml",
  line: 1,
  column: 1,
  start: 0,
  end: 5,
  byte_start: 0,
  byte_end: 5,
};

function hasFieldName(ty: TypeNode): string | null {
  if (!("App" in ty)) {
    return null;
  }
  const firstArg = ty.App.args[0];
  if (!("Lit" in firstArg)) {
    return null;
  }
  return firstArg.Lit.value;
}

const hasFieldType: TypeNode = {
  App: {
    head: { Con: { qualifier: null, name: "HasField", span } },
    args: [
      { Lit: { kind: "Text", value: "cid", span } },
      { Var: { name: "t", span } },
      { App: { head: { Con: { qualifier: null, name: "ContractId", span } }, args: [{ Con: { qualifier: null, name: "Asset", span } }], span } },
    ],
    span,
  },
};

const fieldName: string | null = hasFieldName(hasFieldType);

void fieldName;
