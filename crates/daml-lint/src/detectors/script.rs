use crate::detector::{parse_severity, DetectError, Detector, Finding, FindingLocation, Severity};
use crate::ir::DamlModule;
use daml_syntax::{CharColumn, LineNumber};
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
///
/// # Errors
///
/// Returns [`ScriptLoadError`] when the script cannot be read, initialized, or
/// validated.
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
///
/// # Errors
///
/// Returns [`ScriptLoadError`] when the script cannot be read, initialized, or
/// validated with the supplied `options`.
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
            parse_severity(&severity_str).map_err(|err| ScriptLoadError::UnknownSeverity {
                name: name.to_string(),
                source: err.to_string(),
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
                    FindingLocation::new(
                        module.file.clone(),
                        LineNumber::new(*line),
                        CharColumn::new(*column),
                    ),
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
#[allow(clippy::unwrap_used)]
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
}
