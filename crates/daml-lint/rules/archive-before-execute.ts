import { renderText } from "./_helpers";

const NAME = "archive-before-execute";
const SEVERITY = "high";
const DESCRIPTION = "Contract archived before try/catch — archived contract lost if execution fails";

interface Archived {
  line: number;
  kind: "archive" | "fetchAndArchive";
  cid: string;
}

function checkStatements(statements: Statement[]): void {
  const pending: Archived[] = [];

  for (let index = 0; index < statements.length; index++) {
    const statement = statements[index];
    if ("Archive" in statement) {
      pending.push({
        line: statement.Archive.span.line,
        kind: isFetchAndArchive(statements, index) ? "fetchAndArchive" : "archive",
        cid: renderText(statement.Archive.cid),
      });
    } else if ("Exercise" in statement
      && (statement.Exercise.choice_name === "Archive" || statement.Exercise.choice_name.endsWith(".Archive"))) {
      pending.push({
        line: statement.Exercise.span.line,
        kind: "archive",
        cid: renderText(statement.Exercise.cid),
      });
    } else if ("TryCatch" in statement) {
      for (const archived of pending.splice(0)) {
        reportArchive(archived, statement.TryCatch.span.line);
      }
      checkStatements(statement.TryCatch.try_body);
      checkStatements(statement.TryCatch.catch_body);
    } else if ("Branch" in statement) {
      for (const arm of statement.Branch.arms) checkStatements(arm.body);
    }
  }
}

function isFetchAndArchive(statements: Statement[], index: number): boolean {
  const statement = statements[index];
  const next = statements[index + 1];
  return "Archive" in statement
    && next !== undefined
    && "Fetch" in next
    && next.Fetch.span.line === statement.Archive.span.line
    && JSON.stringify(next.Fetch.cid) === JSON.stringify(statement.Archive.cid);
}

function reportArchive(archived: Archived, tryLine: number): void {
  report(
    { span: { line: archived.line, column: 1 } },
    `Contract archived via '${archived.kind}' at line ${archived.line} before try/catch block at line ${tryLine}. If execution fails, the archived contract is permanently consumed.`,
    `${archived.kind} ${archived.cid.trim()}\n  ...\n  try do ...`,
  );
}

function on_choice(choice: Choice, _template: Template): void {
  checkStatements(choice.body);
}

// QuickJS discovers rule metadata and visitors by evaluating these names.
globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_choice };
