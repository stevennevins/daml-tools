use crate::detector::{parse_severity, DetectError, Detector, Finding, FindingLocation, Severity};
use crate::ir::DamlModule;
use rquickjs::{CatchResultExt, Context, Ctx, Function, Object, Runtime, Value};
use std::cell::RefCell;
use std::error::Error;
#[cfg(feature = "custom-rules")]
use std::path::Path;
use std::rc::Rc;

#[derive(Debug)]
pub enum ScriptLoadError {
    RuntimeInit {
        source: rquickjs::Error,
    },
    IoRead {
        path: String,
        source: std::io::Error,
    },
    MissingName {
        label: String,
    },
    MissingSeverity {
        label: String,
    },
    UnknownSeverity {
        name: String,
        source: String,
    },
    MissingVisitor {
        rule: String,
        visitors: &'static str,
    },
    RuleNameMismatch {
        name: String,
    },
    RegisterConfig {
        path: String,
        source: String,
    },
    RegisterReport {
        path: String,
        source: String,
    },
    Invoke {
        rule: String,
        visitor: String,
        source: String,
    },
    ParseNode {
        rule: String,
        source: String,
    },
    EvalError {
        path: String,
        source: String,
    },
}

impl std::fmt::Display for ScriptLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RuntimeInit { source } => write!(f, "could not create JS runtime: {source}"),
            Self::IoRead { path, source } => {
                write!(f, "could not read rules script {path}: {source}")
            }
            Self::MissingName { label } => {
                write!(f, "rules script {label}: missing `const NAME = \"...\"`")
            }
            Self::MissingSeverity { label } => write!(
                f,
                "rules script {label}: missing `const SEVERITY = \"...\"`"
            ),
            Self::UnknownSeverity { name, source } => {
                write!(f, "rule '{name}': {source}")
            }
            Self::MissingVisitor { rule, visitors } => {
                write!(
                    f,
                    "rule '{rule}': script defines none of the visitor functions ({visitors})"
                )
            }
            Self::RuleNameMismatch { name } => {
                write!(f, "{name}")
            }
            Self::RegisterConfig { path, source } => {
                write!(f, "{path}: could not parse rule CONFIG: {source}")
            }
            Self::RegisterReport { path, source } => {
                write!(f, "{path}: could not install report() helper: {source}")
            }
            Self::Invoke {
                rule,
                visitor,
                source,
            } => {
                write!(f, "rule '{rule}': {visitor} failed: {source}")
            }
            Self::ParseNode { rule, source } => {
                write!(f, "rule '{rule}': {source}")
            }
            Self::EvalError { path, source } => {
                write!(f, "invalid rules script {path}: {source}")
            }
        }
    }
}

impl Error for ScriptLoadError {}

/// AST-based custom detector: a JavaScript rule loaded via `--rules`.
///
/// Modeled on solhint custom rules: the script declares metadata constants
/// and subscribes to AST node types by defining visitor functions. Each
/// visitor receives the node as an object mirroring the IR (src/ir.rs), with
/// a `span` carrying line/column. Findings are reported with
/// `report(node, msg)`, `report(line, msg)`, or `report(node, msg, evidence)`.
///
/// const NAME = "no-foo-template";
/// const SEVERITY = "medium";
/// const DESCRIPTION = "Templates cannot be named Foo";   // optional
///
/// function `on_template(template)` {
///     if (template.name === "Foo") {
///         report(template, "Templates cannot be named Foo");
///     }
/// }
///
/// Visitors: `on_template(template)`, `on_choice(choice, template)`,
/// `on_field(field, template)`, `on_function(function)`, `on_import(import)`,
/// and check(module) for whole-module logic. Visitors must be `function`
/// declarations (arrow functions assigned to const are not discovered).
const VISITORS: &[&str] = &[
    "on_template",
    "on_choice",
    "on_field",
    "on_function",
    "on_import",
    "on_interface",
    "check",
];

/// Interrupt-handler invocations before a script is killed. `QuickJS` calls the
/// handler periodically during execution; a runaway loop must not hang CI.
const MAX_INTERRUPT_CHECKS: u64 = 100_000;

pub struct ScriptDetector {
    name: String,
    severity: Severity,
    description: String,
    path: String,
    /// One runtime+context per rule, with the script evaluated once at load
    /// time and reused across modules (a fresh `QuickJS` runtime + re-eval per
    /// file made large scans QuickJS-bound, not parser-bound). Visitor
    /// functions are stateless by contract; the per-module `report` sink is
    /// swapped in before each run.
    _runtime: Runtime,
    context: Context,
    /// Interrupt-check counter, reset per module.
    interrupt_count: Rc<std::cell::Cell<u64>>,
}

fn new_runtime() -> Result<(Runtime, Rc<std::cell::Cell<u64>>), ScriptLoadError> {
    let rt = Runtime::new().map_err(|source| ScriptLoadError::RuntimeInit { source })?;
    let count = Rc::new(std::cell::Cell::new(0u64));
    let handler_count = count.clone();
    rt.set_interrupt_handler(Some(Box::new(move || {
        handler_count.set(handler_count.get() + 1);
        handler_count.get() > MAX_INTERRUPT_CHECKS
    })));
    Ok((rt, count))
}

/// Read a top-level string constant. `const` bindings are lexical, not
/// globalThis properties, so they're read by evaluating an expression.
fn read_const(ctx: &Ctx<'_>, name: &str) -> Option<String> {
    ctx.eval::<Option<String>, _>(format!("typeof {name} === 'string' ? {name} : null"))
        .ok()
        .flatten()
}

fn invoke<'js, A: rquickjs::function::IntoArgs<'js>>(
    ctx: &Ctx<'js>,
    rule: &str,
    f: &Function<'js>,
    visitor: &'static str,
    args: A,
) -> Result<(), ScriptLoadError> {
    f.call::<_, ()>(args)
        .catch(ctx)
        .map_err(move |e| ScriptLoadError::Invoke {
            rule: rule.to_string(),
            visitor: visitor.to_string(),
            source: e.to_string(),
        })
}

fn parse_node<'js>(
    ctx: &Ctx<'js>,
    rule: &str,
    json: String,
) -> Result<Value<'js>, ScriptLoadError> {
    ctx.json_parse(json)
        .catch(ctx)
        .map_err(|e| ScriptLoadError::ParseNode {
            rule: rule.to_string(),
            source: e.to_string(),
        })
}

#[cfg(feature = "custom-rules")]
/// Load one custom rule script from disk.
///
/// Returns `Err` when the file cannot be read, JS initialization fails, script
/// execution fails, or required rule metadata is missing/invalid.
#[must_use = "handle script load failures instead of ignoring diagnostics"]
pub fn load_script(path: &Path) -> Result<Box<dyn Detector>, ScriptLoadError> {
    let options = empty_options();
    load_script_with_options(path, &options)
}

#[cfg(feature = "custom-rules")]
/// Load one custom rule script with detector `options`.
///
/// Returns `Err` for I/O errors, JS runtime initialization failures, malformed
/// JS metadata, or execution/visitor contract violations.
#[must_use = "use loaded detector or propagate load errors"]
pub fn load_script_with_options(
    path: &Path,
    options: &serde_json::Value,
) -> Result<Box<dyn Detector>, ScriptLoadError> {
    let source = std::fs::read_to_string(path).map_err(|e| ScriptLoadError::IoRead {
        path: path.display().to_string(),
        source: e,
    })?;
    load_script_source_with_options(&path.display().to_string(), &source, options)
}

pub(crate) fn load_script_source(
    label: &str,
    source: &str,
) -> Result<Box<dyn Detector>, ScriptLoadError> {
    let options = empty_options();
    load_script_source_with_options(label, source, &options)
}

pub(crate) fn load_script_source_with_options(
    label: &str,
    source: &str,
    options: &serde_json::Value,
) -> Result<Box<dyn Detector>, ScriptLoadError> {
    let (rt, interrupt_count) = new_runtime()?;
    let context = Context::full(&rt).map_err(|e| ScriptLoadError::RuntimeInit { source: e })?;
    let loaded = context.with(|ctx| {
        // report() must exist at load time so top-level code referencing it parses.
        register_report(&ctx, Rc::new(RefCell::new(Vec::new())))?;
        register_config(&ctx, options)?;
        ctx.eval::<(), _>(source.as_bytes())
            .catch(&ctx)
            .map_err(|e| ScriptLoadError::EvalError {
                path: label.to_string(),
                source: e.to_string(),
            })?;

        let name = read_const(&ctx, "NAME").ok_or_else(|| ScriptLoadError::MissingName {
            label: label.to_string(),
        })?;
        let severity_str =
            read_const(&ctx, "SEVERITY").ok_or_else(|| ScriptLoadError::MissingSeverity {
                label: label.to_string(),
            })?;
        let severity =
            parse_severity(&severity_str).ok_or_else(|| ScriptLoadError::UnknownSeverity {
                name: name.to_string(),
                source: format!(
                    "unknown severity '{severity_str}'. Use critical, high, medium, low, or info."
                ),
            })?;
        let description = read_const(&ctx, "DESCRIPTION").unwrap_or_default();

        let globals = ctx.globals();
        let has_visitor = VISITORS
            .iter()
            .any(|v| globals.get::<_, Function<'_>>(*v).is_ok());
        if !has_visitor {
            return Err(ScriptLoadError::MissingVisitor {
                rule: name,
                visitors:
                    "on_template, on_choice, on_field, on_function, on_import, on_interface, check",
            });
        }

        Ok((name, severity, description))
    });
    let (name, severity, description) = loaded?;
    let detector: Box<dyn Detector> = Box::new(ScriptDetector {
        name,
        severity,
        description,
        path: label.to_string(),
        _runtime: rt,
        context,
        interrupt_count,
    });
    Ok(detector)
}

/// (line, column, message, explicit evidence) reported by the script.
type Reported = Rc<RefCell<Vec<(usize, usize, String, Option<String>)>>>;

fn json<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).expect("IR types always serialize")
}

fn empty_options() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

fn positive_truncated_usize(value: f64) -> usize {
    if !value.is_finite() {
        return 1;
    }
    let truncated = value.trunc();
    if truncated < 1.0 {
        return 1;
    }
    format!("{truncated:.0}").parse().unwrap_or(usize::MAX)
}

fn positive_i64_to_usize(value: i64) -> usize {
    usize::try_from(value.max(1)).unwrap_or(usize::MAX)
}

fn register_config(ctx: &Ctx<'_>, options: &serde_json::Value) -> Result<(), ScriptLoadError> {
    let config_json =
        serde_json::to_string(options).map_err(|e| ScriptLoadError::RegisterConfig {
            path: "rule config".to_string(),
            source: e.to_string(),
        })?;
    let config =
        ctx.json_parse(config_json)
            .catch(ctx)
            .map_err(|e| ScriptLoadError::RegisterConfig {
                path: "rule config".to_string(),
                source: e.to_string(),
            })?;
    ctx.globals()
        .set("__daml_lint_config", config)
        .map_err(|e| ScriptLoadError::RegisterConfig {
            path: "rule config".to_string(),
            source: e.to_string(),
        })?;
    ctx.eval::<(), _>(br#"globalThis.CONFIG = globalThis.__daml_lint_config;"#)
        .map_err(|e| ScriptLoadError::RegisterConfig {
            path: "rule config".to_string(),
            source: e.to_string(),
        })
}

fn register_report(ctx: &Ctx<'_>, sink: Reported) -> Result<(), ScriptLoadError> {
    let report_impl = Function::new(
        ctx.clone(),
        move |arg: Value<'_>, message: String, evidence: Option<String>| {
            let (line, column) = location_of(&arg);
            sink.borrow_mut().push((line, column, message, evidence));
        },
    )
    .map_err(|e| ScriptLoadError::RegisterReport {
        path: "report".to_string(),
        source: e.to_string(),
    })?;
    ctx.globals()
        .set("__daml_lint_report", report_impl)
        .map_err(|e| ScriptLoadError::RegisterReport {
            path: "report".to_string(),
            source: e.to_string(),
        })?;
    ctx.eval::<(), _>(
        br#"
globalThis.report = function(arg, message, evidence) {
  if (arguments.length < 3) {
    return globalThis.__daml_lint_report(arg, message, null);
  }
  return globalThis.__daml_lint_report(arg, message, evidence);
};
"#,
    )
    .map_err(|e| ScriptLoadError::RegisterReport {
        path: "report".to_string(),
        source: e.to_string(),
    })
}

/// First argument of `report()`: a node object (location from its span) or a
/// line number.
fn location_of(arg: &Value<'_>) -> (usize, usize) {
    if let Some(line) = arg.as_number() {
        return (positive_truncated_usize(line), 1);
    }
    if let Some(obj) = arg.as_object() {
        if let Ok(span) = obj.get::<_, Object<'_>>("span") {
            let line: i64 = span.get("line").unwrap_or(1);
            let column: i64 = span.get("column").unwrap_or(1);
            return (positive_i64_to_usize(line), positive_i64_to_usize(column));
        }
    }
    (1, 1)
}

impl ScriptDetector {
    fn collect_script_findings(
        &self,
        module: &DamlModule,
    ) -> Result<Vec<Finding>, ScriptLoadError> {
        let reported: Reported = Rc::new(RefCell::new(Vec::new()));

        self.interrupt_count.set(0);
        self.context.with(|ctx| -> Result<(), ScriptLoadError> {
            // Fresh sink per module; replaces the previous report binding.
            register_report(&ctx, reported.clone())?;

            let globals = ctx.globals();
            let visitor = |name: &str| globals.get::<_, Function<'_>>(name).ok();
            let rule = self.name.as_str();

            for template in &module.templates {
                let t_json = json(template);
                if let Some(f) = visitor("on_template") {
                    let t = parse_node(&ctx, rule, t_json.clone())?;
                    invoke(&ctx, rule, &f, "on_template", (t,))?;
                }
                if let Some(f) = visitor("on_choice") {
                    for choice in &template.choices {
                        let c = parse_node(&ctx, rule, json(choice))?;
                        let t = parse_node(&ctx, rule, t_json.clone())?;
                        invoke(&ctx, rule, &f, "on_choice", (c, t))?;
                    }
                }
                if let Some(f) = visitor("on_field") {
                    for field in &template.fields {
                        let fd = parse_node(&ctx, rule, json(field))?;
                        let t = parse_node(&ctx, rule, t_json.clone())?;
                        invoke(&ctx, rule, &f, "on_field", (fd, t))?;
                    }
                }
            }
            if let Some(f) = visitor("on_function") {
                for function in &module.functions {
                    let fun = parse_node(&ctx, rule, json(function))?;
                    invoke(&ctx, rule, &f, "on_function", (fun,))?;
                }
            }
            if let Some(f) = visitor("on_interface") {
                for interface in &module.interfaces {
                    let i = parse_node(&ctx, rule, json(interface))?;
                    invoke(&ctx, rule, &f, "on_interface", (i,))?;
                }
            }
            if let Some(f) = visitor("on_import") {
                for import in &module.imports {
                    let i = parse_node(&ctx, rule, json(import))?;
                    invoke(&ctx, rule, &f, "on_import", (i,))?;
                }
            }
            if let Some(f) = visitor("check") {
                let m = parse_node(&ctx, rule, json(module))?;
                invoke(&ctx, rule, &f, "check", (m,))?;
            }
            Ok(())
        })?;

        let findings = reported
            .borrow()
            .iter()
            .map(|(line, column, message, evidence)| {
                Finding::new(
                    self.name.clone(),
                    self.severity,
                    FindingLocation::new(module.file.clone(), *line, *column),
                    message,
                    evidence.clone().unwrap_or_else(|| {
                        module
                            .source
                            .lines()
                            .nth(line.saturating_sub(1))
                            .unwrap_or("")
                            .trim()
                            .to_string()
                    }),
                )
            })
            .collect();
        Ok(findings)
    }
}

impl Detector for ScriptDetector {
    fn name(&self) -> &str {
        &self.name
    }

    fn severity(&self) -> Severity {
        self.severity
    }

    fn description(&self) -> &str {
        &self.description
    }

    fn try_detect(&self, module: &DamlModule) -> Result<Vec<Finding>, DetectError> {
        self.collect_script_findings(module)
            .map_err(|e| DetectError::new(self.name(), format!("{}: {e}", self.path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_daml;
    use std::path::Path;

    fn load_script_from_str(
        label: &str,
        script: &str,
    ) -> Result<Box<dyn Detector>, ScriptLoadError> {
        load_script_source(label, script)
    }

    const TEMPLATE_NO_ENSURE: &str = r#"module Test where

template Iou
  with
    issuer : Party
    owner : Party
    amount : Decimal
  where
    signatory issuer
    observer owner

    choice Transfer : ()
      controller owner
      do
        pure ()
"#;

    #[test]
    fn test_on_template_visitor_reports() {
        let det = load_script_from_str(
            "on-template",
            r#"
const NAME = "template-requires-ensure";
const SEVERITY = "medium";

function on_template(template) {
    if (template.ensure_clause === null) {
        report(template, `Template '${template.name}' has no ensure clause`);
    }
}
"#,
        )
        .unwrap();
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, "template-requires-ensure");
        assert_eq!(findings[0].file, Path::new("Test.daml"));
        assert_eq!(findings[0].line, 3);
        assert_eq!(findings[0].column, 1);
        assert!(findings[0].message.contains("Iou"));
        assert_eq!(findings[0].evidence, "template Iou");
    }

    #[test]
    fn test_report_accepts_explicit_evidence() {
        let det = load_script_from_str(
            "explicit-evidence",
            r#"
const NAME = "explicit-evidence";
const SEVERITY = "low";

function on_template(template) {
    report(template, "template evidence test", "custom evidence");
}
"#,
        )
        .unwrap();
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].evidence, "custom evidence");
    }

    #[test]
    fn test_dts_exposes_structured_only_contract() {
        let dts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/examples/daml-lint.d.ts"
        ))
        .expect("read daml-lint.d.ts");
        assert!(dts.contains("ir_version: 4"));
        for forbidden in [
            "body_raw",
            "raw_text",
            "cid_expr",
            "controllers: string",
            "signatories: string",
            "observers: string",
            "type_text",
            "key_type: string",
            "type_signature: string",
            "expr: string",
            "condition: string",
        ] {
            assert!(
                !dts.contains(forbidden),
                "daml-lint.d.ts must not expose removed field {forbidden:?}"
            );
        }
    }

    #[test]
    fn test_runtime_module_exposes_structured_only_contract() {
        let det = load_script_from_str(
            "structured-only-runtime",
            r#"
const NAME = "structured-only-runtime";
const SEVERITY = "low";

function has(o, k) {
  return Object.prototype.hasOwnProperty.call(o, k);
}

function checkNoOldFields(label, node, fields) {
  for (const field of fields) {
    if (has(node, field)) report(1, `${label} still exposes ${field}`);
  }
}

function spanOfType(ty) {
  const tag = Object.keys(ty)[0];
  return ty[tag].span;
}

function typeSource(m, ty) {
  const span = spanOfType(ty);
  return m.source.slice(span.start, span.end);
}

function field(template, name) {
  return template.fields.find((f) => f.name === name);
}

function check(m) {
  if (m.ir_version !== 4) report(1, `expected ir_version 4, got ${m.ir_version}`);
  const t = m.templates[0];
  checkNoOldFields("template", t, ["signatories", "observers"]);
  if (typeof t.key_type === "string") report(1, "template key_type is still a string");
  checkNoOldFields("ensure", t.ensure_clause, ["raw_text"]);
  for (const f of t.fields) {
    checkNoOldFields("field", f, ["type_text"]);
    if (typeof f.type_ === "string") report(1, `field ${f.name} type_ is still a string`);
  }
  if (typeSource(m, field(t, "maybeCid").type_) !== "Optional (ContractId T)") {
    report(1, "type span cannot recover Optional (ContractId T)");
  }
  if (typeSource(m, field(t, "precision").type_) !== "Numeric 10") {
    report(1, "type span cannot recover Numeric 10");
  }
  const c = t.choices[0];
  checkNoOldFields("choice", c, ["controllers", "body_raw", "return_type_text"]);
  if (typeof c.return_type === "string") report(1, "choice return_type is still a string");
  for (const p of c.parameters) checkNoOldFields("choice parameter", p, ["type_text"]);
  for (const stmt of c.body) {
    if ("Let" in stmt) checkNoOldFields("Let", stmt.Let, ["expr"]);
    if ("Assert" in stmt) checkNoOldFields("Assert", stmt.Assert, ["condition"]);
    if ("Fetch" in stmt) checkNoOldFields("Fetch", stmt.Fetch, ["cid_expr"]);
    if ("Archive" in stmt) checkNoOldFields("Archive", stmt.Archive, ["cid_expr"]);
    if ("Create" in stmt) checkNoOldFields("Create", stmt.Create, ["raw"]);
    if ("Exercise" in stmt) checkNoOldFields("Exercise", stmt.Exercise, ["cid_expr", "raw"]);
  }
  const fn = m.functions[0];
  checkNoOldFields("function", fn, ["body_raw", "type_text"]);
  if (typeof fn.type_signature === "string") report(1, "function type_signature is still a string");
}
"#,
        )
        .unwrap();
        let source = r#"
module RuntimeSurface where

template T
  with
    owner : Party
    cid : ContractId T
    maybeCid : Optional (ContractId T)
    amount : Decimal
    precision : Numeric 10
  where
    signatory owner
    observer owner
    ensure amount > 0.0
    key owner : Party
    maintainer key

    choice C : ContractId T
      with
        p : Party
        target : ContractId T
      controller owner
      observer p
      do
        let x = target
        assert True
        fetched <- fetch target
        archive target
        created <- create this with owner = p
        exercise target C with p = p; target = target

helper : Party -> Update ()
helper p = pure ()
"#;
        let module = parse_daml(source, Path::new("RuntimeSurface.daml"));
        assert!(det.detect(&module).is_empty());
    }

    #[test]
    fn test_on_choice_visitor_gets_template_context() {
        let det = load_script_from_str(
            "on-choice",
            r#"
const NAME = "consuming-choice-signatory-controller";
const SEVERITY = "medium";

function exprText(e) {
    if ("Var" in e) {
        const v = e.Var;
        return v.qualifier === null ? v.name : `${v.qualifier}.${v.name}`;
    }
    if ("Con" in e) {
        const c = e.Con;
        return c.qualifier === null ? c.name : `${c.qualifier}.${c.name}`;
    }
    if ("App" in e) return [exprText(e.App.func), ...e.App.args.map(exprText)].join(" ");
    if ("Unknown" in e) return e.Unknown.raw;
    return "";
}

function on_choice(choice, template) {
    if (choice.consuming === "non-consuming") {
        return;
    }
    const signatories = template.signatory_exprs.map(exprText);
    if (choice.controller_exprs.some(c => signatories.includes(exprText(c)))) {
        return;
    }
    report(choice, `Consuming choice '${choice.name}' has no signatory controller`);
}
"#,
        )
        .unwrap();
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("Transfer"));
    }

    #[test]
    fn test_check_visitor_and_line_report() {
        let det = load_script_from_str(
            "check-module",
            r#"
const NAME = "max-one-template";
const SEVERITY = "low";

function check(module) {
    if (module.templates.length > 0) {
        report(1, `Module '${module.name}' has templates`);
    }
}
"#,
        )
        .unwrap();
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
    }

    #[test]
    fn test_statement_bodies_inspectable() {
        let det = load_script_from_str(
            "statements",
            r#"
const NAME = "no-create-in-choice";
const SEVERITY = "low";

function on_choice(choice) {
    for (const stmt of choice.body) {
        if ("Create" in stmt) {
            report(choice, `Choice '${choice.name}' creates contracts`);
        }
    }
}
"#,
        )
        .unwrap();
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        det.detect(&module);
    }

    #[test]
    fn test_missing_name_rejected() {
        let result = load_script_from_str(
            "no-name",
            r#"
const SEVERITY = "low";
function on_template(t) {}
"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_no_visitor_rejected() {
        let result = load_script_from_str(
            "no-visitor",
            r#"
const NAME = "x";
const SEVERITY = "low";
"#,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_bad_severity_rejected() {
        let result = load_script_from_str(
            "bad-severity",
            r#"
const NAME = "x";
const SEVERITY = "banana";
function on_template(t) {}
"#,
        );
        match result {
            Err(e) => assert!(e.to_string().contains("banana")),
            Ok(_) => panic!("bad severity should be rejected"),
        }
    }

    #[test]
    fn test_syntax_error_rejected() {
        let result = load_script_from_str("syntax-err", "function on_template(t) {");
        assert!(result.is_err());
    }

    fn raw_detector(name: &str, source: &str) -> ScriptDetector {
        let (runtime, interrupt_count) = new_runtime().unwrap();
        let context = Context::full(&runtime).unwrap();
        context.with(|ctx| {
            register_report(&ctx, Rc::new(RefCell::new(Vec::new()))).unwrap();
            ctx.eval::<(), _>(source.as_bytes()).unwrap();
        });
        ScriptDetector {
            name: name.to_string(),
            severity: Severity::Low,
            description: String::new(),
            path: format!("{name}.js"),
            _runtime: runtime,
            context,
            interrupt_count,
        }
    }

    #[test]
    fn test_runtime_error_surfaces_rule_and_visitor() {
        let script = raw_detector("boom", r#"function on_template(t) { t.does.not.exist; }"#);
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        let err = script.collect_script_findings(&module).unwrap_err();
        let err = err.to_string();
        assert!(err.contains("boom"));
        assert!(err.contains("on_template"));
    }

    #[test]
    fn test_try_detect_returns_runtime_errors_to_library_callers() {
        let script = raw_detector("boom", r#"function on_template(t) { t.does.not.exist; }"#);
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        let err = script.try_detect(&module).unwrap_err();
        assert_eq!(err.detector(), "boom");
        assert!(err.message().contains("on_template"));
    }

    #[test]
    fn test_infinite_loop_interrupted() {
        let script = raw_detector(
            "spin",
            r#"
const NAME = "spin";
const SEVERITY = "low";
function on_template(t) { while (true) {} }
"#,
        );
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        assert!(script.collect_script_findings(&module).is_err());
    }

    /// Interrupt counter resets between modules: a long (but finite) rule
    /// run on many modules must not trip the runaway-loop guard.
    #[test]
    fn test_interrupt_counter_resets_per_module() {
        let script = raw_detector(
            "busy",
            r#"
const NAME = "busy";
const SEVERITY = "low";
function on_template(t) { let x = 0; for (let i = 0; i < 200000; i++) { x += i; } }
"#,
        );
        let module = parse_daml(TEMPLATE_NO_ENSURE, Path::new("Test.daml"));
        for _ in 0..5 {
            assert!(script.collect_script_findings(&module).is_ok());
        }
    }

    /// Exercises every script-visible node kind: all scalar and parameterized
    /// field types, ensure clauses, choice parameters, nonconsuming choices,
    /// every `Statement` variant (with `TryCatch` recursion), qualified aliased
    /// imports, and top-level functions.
    #[test]
    fn test_every_node_kind_reaches_scripts() {
        let probe = r#"module Probe where

import qualified DA.Map as Map
import DA.Time

template Probe
  with
    owner : Party
    note : Text
    amount : Decimal
    count : Int
    active : Bool
    issued : Date
    stamp : Time
    tags : [Text]
    backup : Optional Party
    parent : ContractId Probe
    scores : TextMap Int
    extra : Custom
  where
    signatory owner
    ensure amount > 0.0

    choice Reissue : ContractId Probe
      with
        newOwner : Party
      controller owner
      do
        let total = amount + 1.0
        assert (total > 0.0)
        p <- fetch parent
        archive parent
        cid <- create this with owner = newOwner
        result <- exercise cid Noop
        try do
          pure ()
        catch
          (e : AnyException) -> pure ()
        pure cid

    nonconsuming choice Noop : ()
      observer owner
      controller owner
      do
        pure ()

    key (owner, note) : (Party, Text)
    maintainer key._1

    interface instance Probeable for Probe where
      view = ProbeView owner

interface Probeable where
  viewtype ProbeView
  getProbeOwner : Party

  nonconsuming choice GetProbeView : ProbeView
    with
      viewer : Party
    controller viewer
    do
      pure (view this)

helper : Int -> Int
helper x = x + 1

describe owner xs total parts = do
  let scaled = map (\y -> y * 2) xs
  let pair = (total / parts, [total, parts])
  let label = if total > 0.0 then "pos" else show (-total)
  let picked = case xs of
        [] -> None
        h :: _ -> Some h
  let result = let inner = Map.Map in inner
  pure (FooCon with field1 = label)
"#;
        let det = load_script_from_str(
            "census",
            r#"
const NAME = "node-census";
const SEVERITY = "info";

function exprKinds(e, seen) {
  if (e === null || typeof e !== "object") return;
  const k = Object.keys(e)[0];
  seen.add("Expr:" + k);
  const p = e[k];
  for (const key of Object.keys(p)) {
    const v = p[key];
    if (key === "span") continue;
    if (Array.isArray(v)) {
      for (const item of v) {
        if (item && typeof item === "object") {
          if ("body" in item && "pattern" in item) exprKinds(item.body, seen);
          else if ("value" in item && "name" in item) {
            if (item.value !== null) exprKinds(item.value, seen);
          } else if (Object.keys(item).length === 1) {
            if (k === "DoBlock") stmtKinds([item], seen);
            else exprKinds(item, seen);
          }
        }
      }
    } else if (v && typeof v === "object") {
      exprKinds(v, seen);
    }
  }
}

function stmtKinds(stmts, seen) {
  for (const s of stmts) {
    const k = Object.keys(s)[0];
    seen.add(k);
    const p = s[k];
    if (p.binder !== undefined && p.binder !== null) seen.add("Binder");
    if (k === "TryCatch") {
      stmtKinds(p.try_body, seen);
      stmtKinds(p.catch_body, seen);
    }
    if (p.value) exprKinds(p.value, seen);
    if (p.condition_expr) exprKinds(p.condition_expr, seen);
    if (p.cid) exprKinds(p.cid, seen);
    if (p.argument) exprKinds(p.argument, seen);
    if (p.expr && typeof p.expr === "object") exprKinds(p.expr, seen);
  }
}

function typeHeadName(t) {
  if (t === null) return "Unknown";
  const tag = Object.keys(t)[0];
  if (tag === "Con") return t.Con.name;
  if (tag === "App") return typeHeadName(t.App.head);
  return tag;
}

function typeKind(t) {
  if (t === null) return "Scalar:Unknown";
  const tag = Object.keys(t)[0];
  if (tag === "Con") return "Scalar:" + t.Con.name;
  if (tag === "List") return "Param:List";
  if (tag === "App") return "Param:" + typeHeadName(t.App.head);
  return "Scalar:" + tag;
}

function check(m) {
  const seen = new Set();
  for (const t of m.templates) {
    if (t.ensure_clause !== null) {
      seen.add("Ensure");
      exprKinds(t.ensure_clause.expr, seen);
    }
    if (t.key_expr !== null) seen.add("KeyExpr");
    if (t.key_type !== null) seen.add("KeyType");
    if (t.maintainer_exprs.length > 0) seen.add("Maintainer");
    if (t.signatory_exprs.length > 0) seen.add("SignatoryExpr");
    if (t.interface_instances.length > 0) {
      seen.add("InterfaceInstance");
      if (t.interface_instances[0].methods.length > 0) seen.add("InstanceMethod");
    }
    for (const f of t.fields) {
      seen.add(typeKind(f.type_));
    }
    for (const c of t.choices) {
      if (c.parameters.length > 0) seen.add("ChoiceParams");
      if (c.consuming === "non-consuming") seen.add("Nonconsuming");
      if (c.controller_exprs.length > 0) seen.add("ControllerExpr");
      if (c.observer_exprs.length > 0) seen.add("ChoiceObserver");
      stmtKinds(c.body, seen);
    }
  }
  for (const i of m.interfaces) {
    seen.add("Interface");
    if (i.viewtype !== null) seen.add("Viewtype");
    if (i.methods.length > 0) seen.add("InterfaceMethod");
    if (i.choices.length > 0) seen.add("InterfaceChoice");
  }
  for (const i of m.imports) {
    if (i.qualified === "qualified" && i.alias !== null) seen.add("QualifiedAlias");
  }
  for (const fn of m.functions) {
    seen.add("Function");
    if (fn.type_signature !== null) seen.add("TypeSignature");
    stmtKinds(fn.body, seen);
  }
  for (const k of Array.from(seen).sort()) report(1, k);
}
"#,
        )
        .unwrap();
        let module = parse_daml(probe, Path::new("Probe.daml"));
        let seen: Vec<String> = det.detect(&module).into_iter().map(|f| f.message).collect();

        for expected in [
            "Scalar:Party",
            "Scalar:Text",
            "Scalar:Decimal",
            "Scalar:Int",
            "Scalar:Bool",
            "Scalar:Date",
            "Scalar:Time",
            "Param:List",
            "Param:Optional",
            "Param:ContractId",
            "Param:TextMap",
            "Scalar:Custom",
            "Ensure",
            "ChoiceParams",
            "Nonconsuming",
            "Let",
            "Assert",
            "Fetch",
            "Archive",
            "Create",
            "Exercise",
            "TryCatch",
            "QualifiedAlias",
            "Function",
            // v2 structured surface
            "Binder",
            "KeyExpr",
            "KeyType",
            "Maintainer",
            "SignatoryExpr",
            "ControllerExpr",
            "ChoiceObserver",
            "InterfaceInstance",
            "InstanceMethod",
            "Interface",
            "Viewtype",
            "InterfaceMethod",
            "InterfaceChoice",
            "TypeSignature",
            "Expr:Var",
            "Expr:Con",
            "Expr:Lit",
            "Expr:App",
            "Expr:BinOp",
            "Expr:Neg",
            "Expr:Lambda",
            "Expr:If",
            "Expr:Case",
            "Expr:LetIn",
            "Expr:Record",
            "Expr:Tuple",
            "Expr:List",
        ] {
            assert!(
                seen.iter().any(|m| m == expected),
                "node kind '{expected}' did not reach the script; saw: {seen:?}"
            );
        }
    }

    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_demo_scripts_load() {
        assert!(load_script(Path::new("examples/dist/template-requires-ensure.js")).is_ok());
        assert!(load_script(Path::new(
            "examples/dist/consuming-choice-signatory-controller.js"
        ))
        .is_ok());
        assert!(load_script(Path::new("examples/dist/no-trace.js")).is_ok());
        assert!(load_script(Path::new("examples/dist/no-create-in-nonconsuming.js")).is_ok());
        assert!(load_script(Path::new("examples/dist/no-bare-contractid-field.js")).is_ok());
        assert!(load_script(Path::new("examples/dist/unqualified-da-import.js")).is_ok());
        assert!(load_script(Path::new("examples/dist/function-ledger-actions.js")).is_ok());
        assert!(load_script(Path::new("examples/dist/choice-param-shadows-field.js")).is_ok());
    }

    // Regression (audit finding 19): the shipped unguarded-division-ast example
    // rule must flag `100.0 / n` when the only `assert (n /= 0.0)` lives inside
    // an `if` branch — when the branch is not taken the assert never runs, so
    // `n` can be 0. The branch-lifted assert must not count as a prior guard.
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_unguarded_division_flags_conditional_assert() {
        let det = load_script(Path::new("examples/dist/unguarded-division-ast.js")).unwrap();
        let source = r#"module CondFn where

template T
  with
    p : Party
  where
    signatory p

    choice Risky : Decimal
      with
        n : Decimal
        b : Bool
      controller p
      do
        if b then assert (n /= 0.0) else pure ()
        pure (100.0 / n)
"#;
        let module = parse_daml(source, Path::new("CondFn.daml"));
        let findings = det.detect(&module);
        assert_eq!(
            findings.len(),
            1,
            "conditional assert should not suppress the division"
        );
        assert!(findings[0].message.contains("'n'"));
    }

    // Counter-case for finding 19: an unconditional do-block assert still
    // suppresses (no false positive introduced by the fix).
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_unguarded_division_respects_unconditional_assert() {
        let det = load_script(Path::new("examples/dist/unguarded-division-ast.js")).unwrap();
        let source = r#"module Safe where

template T
  with
    p : Party
  where
    signatory p

    choice OK : Decimal
      with
        n : Decimal
      controller p
      do
        assert (n /= 0.0)
        pure (100.0 / n)
"#;
        let module = parse_daml(source, Path::new("Safe.daml"));
        assert!(det.detect(&module).is_empty());
    }

    // Regression (audit finding 20): a consuming choice controlled by an
    // ordinary party field whose NAME merely starts with "signatory" (e.g.
    // `signatoryParty`) is NOT signatory-controlled — the loose startsWith
    // substring match gave a false all-clear. It must flag.
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_signatory_controller_flags_lookalike_field() {
        let det = load_script(Path::new(
            "examples/dist/consuming-choice-signatory-controller.js",
        ))
        .unwrap();
        let source = r#"module SigFP where

template Bar
  with
    issuer : Party
    signatoryParty : Party
  where
    signatory issuer

    choice Grab : ContractId Bar
      controller signatoryParty
      do
        create this with issuer = issuer
"#;
        let module = parse_daml(source, Path::new("SigFP.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1, "signatoryParty is not a signatory");
        assert!(findings[0].message.contains("Grab"));
    }

    // Counter-case for finding 20: the legitimate flexible-controller forms
    // `controller signatory this` and `controller signatory this, obs` (both
    // serialize the keyword as exactly "signatory this") still suppress.
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_signatory_controller_allows_signatory_this() {
        let det = load_script(Path::new(
            "examples/dist/consuming-choice-signatory-controller.js",
        ))
        .unwrap();
        let source = r#"module SigOk where

template Baz
  with
    issuer : Party
    obs : Party
  where
    signatory issuer

    choice GrabA : ContractId Baz
      controller signatory this
      do
        create this with issuer = issuer

    choice GrabB : ContractId Baz
      controller signatory this, obs
      do
        create this with issuer = issuer
"#;
        let module = parse_daml(source, Path::new("SigOk.daml"));
        assert!(det.detect(&module).is_empty());
    }

    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_no_create_in_nonconsuming_descends_branch_arms() {
        let det = load_script(Path::new("examples/dist/no-create-in-nonconsuming.js")).unwrap();
        let source = r#"module BranchCreate where

template T
  with
    p : Party
  where
    signatory p

    nonconsuming choice Fork : ContractId T
      with
        flag : Bool
      controller p
      do
        if flag then do
          create this
        else do
          create this
"#;
        let module = parse_daml(source, Path::new("BranchCreate.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("Fork"));
    }

    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_function_ledger_actions_descends_branch_arms() {
        let det = load_script(Path::new("examples/dist/function-ledger-actions.js")).unwrap();
        let source = r#"module BranchLedger where

template T
  with
    p : Party
  where
    signatory p

branchArchive : ContractId T -> Bool -> Update ()
branchArchive cid flag = do
  if flag then do
    archive cid
  else do
    archive cid
"#;
        let module = parse_daml(source, Path::new("BranchLedger.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("branchArchive"));
    }

    // Regression (audit finding 27): `trace` inside a `{- ... -}` block comment
    // is not executable code and must not be flagged.
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_no_trace_ignores_block_comment() {
        let det = load_script(Path::new("examples/dist/no-trace.js")).unwrap();
        let source = r#"module BlockComment where

{- This module used to call trace for debugging.
   We removed it. -}
foo : Int
foo = 1
"#;
        let module = parse_daml(source, Path::new("BlockComment.daml"));
        assert!(det.detect(&module).is_empty());
    }

    // Regression (audit finding 28): `trace` as a word inside a Text literal is
    // not a Debug.trace call and must not be flagged.
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_no_trace_ignores_string_literal() {
        let det = load_script(Path::new("examples/dist/no-trace.js")).unwrap();
        let source = r#"module Trace2 where

msg : Text
msg = "please trace this transaction"
"#;
        let module = parse_daml(source, Path::new("Trace2.daml"));
        assert!(det.detect(&module).is_empty());
    }

    // Counter-case for findings 27/28: a real `trace` call is still flagged, so
    // the comment/string stripping did not blind the rule.
    #[cfg(feature = "custom-rules")]
    #[test]
    fn test_example_no_trace_still_flags_real_call() {
        let det = load_script(Path::new("examples/dist/no-trace.js")).unwrap();
        let source = r#"module RealTrace where

foo : Int -> Int
foo x = trace "dbg" (x + 1)
"#;
        let module = parse_daml(source, Path::new("RealTrace.daml"));
        let findings = det.detect(&module);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 4);
    }
}
