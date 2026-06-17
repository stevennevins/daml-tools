use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daml-lint"))
}

fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "daml-lint-cli-{}-{}-{}",
        std::process::id(),
        id,
        name
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

fn temp_dir(name: &str) -> std::path::PathBuf {
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "daml-lint-cli-{}-{}-{}",
        std::process::id(),
        id,
        name
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn clean_file() -> std::path::PathBuf {
    temp_file(
        "clean.daml",
        r#"module Clean where

template Holding
  with
    owner : Party
    amount : Decimal
  where
    signatory owner
    ensure amount > 0.0

    choice Transfer : ContractId Holding
      with
        newOwner : Party
      controller owner
      do
        create this with owner = newOwner
"#,
    )
}

#[test]
#[cfg(feature = "custom-rules")]
fn installed_plugin_manifest_rule_can_be_enabled_from_config() {
    let project = temp_dir("plugin-project");
    std::fs::create_dir_all(project.join("node_modules/daml-lint-plugin-template/dist")).unwrap();
    std::fs::write(
        project.join(".daml-lint.json"),
        r#"{
  "plugins": ["template"],
  "rules": {
    "template/template-name-blocklist": ["medium", { "names": ["Iou"] }]
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("node_modules/daml-lint-plugin-template/package.json"),
        r#"{
  "name": "daml-lint-plugin-template",
  "version": "1.0.0",
  "damlLint": {
    "rules": {
      "template-name-blocklist": "dist/template-name-blocklist.js"
    }
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("node_modules/daml-lint-plugin-template/dist/template-name-blocklist.js"),
        r#"
const NAME = "template-name-blocklist";
const SEVERITY = "low";

function on_template(template) {
  const config = typeof CONFIG === "object" && CONFIG !== null ? CONFIG : {};
  const names = Array.isArray(config.names) ? config.names : [];
  if (names.includes(template.name)) {
    report(template, `Template '${template.name}' is blocked by config`);
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("Iou.daml"),
        r#"module Iou where

template Iou
  with
    owner : Party
  where
    signatory owner
    ensure True
"#,
    )
    .unwrap();

    let output = cmd()
        .current_dir(&project)
        .arg("Iou.daml")
        .arg("--fail-on")
        .arg("medium")
        .output()
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("MEDIUM"), "stdout was:\n{stdout}");
    assert!(
        stdout.contains("template/template-name-blocklist"),
        "stdout was:\n{stdout}"
    );
    assert!(
        stdout.contains("blocked by config"),
        "stdout was:\n{stdout}"
    );
}

#[test]
#[cfg(feature = "custom-rules")]
fn config_can_disable_builtin_rule() {
    let project = temp_dir("disabled-builtin-project");
    std::fs::write(
        project.join(".daml-lint.json"),
        r#"{
  "rules": {
    "missing-ensure-decimal": "off"
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("Iou.daml"),
        r#"module Iou where

template Iou
  with
    issuer : Party
    amount : Decimal
  where
    signatory issuer
"#,
    )
    .unwrap();

    let output = cmd()
        .current_dir(&project)
        .arg("Iou.daml")
        .arg("--fail-on")
        .arg("info")
        .output()
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No findings."));
}

#[test]
#[cfg(feature = "custom-rules")]
fn config_unknown_enabled_rule_exits_two() {
    let project = temp_dir("unknown-rule-project");
    std::fs::write(
        project.join(".daml-lint.json"),
        r#"{
  "rules": {
    "unknown-rule": "medium"
  }
}
"#,
    )
    .unwrap();
    std::fs::write(
        project.join("Clean.daml"),
        r#"module Clean where

template Holding
  with
    owner : Party
  where
    signatory owner
    ensure True
"#,
    )
    .unwrap();

    let output = cmd()
        .current_dir(&project)
        .arg("Clean.daml")
        .output()
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown-rule"), "stderr was:\n{stderr}");
}

#[test]
fn help_exits_successfully() {
    let output = cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Static analysis scanner"));
}

#[cfg(not(feature = "custom-rules"))]
#[test]
fn help_omits_rules_when_custom_rules_disabled() {
    let output = cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("--rules"));
}

#[test]
fn clean_file_exits_zero() {
    let path = clean_file();
    let output = cmd().arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("No findings."));
}

#[test]
fn findings_at_threshold_exit_one() {
    let path = temp_file(
        "finding.daml",
        r#"module Bad where

template Iou
  with
    issuer : Party
    amount : Decimal
  where
    signatory issuer
"#,
    );
    let output = cmd()
        .arg(&path)
        .arg("--fail-on")
        .arg("high")
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("missing-ensure-decimal"));
}

#[test]
fn parse_error_exits_three() {
    let path = temp_file("malformed.daml", "module Bad where\nf = \"unterminated\n");
    let output = cmd().arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&output.stderr).contains("daml-lint: parse"));
}

#[test]
#[cfg(feature = "custom-rules")]
fn custom_rule_runtime_error_exits_two() {
    let path = clean_file();
    let rule = temp_file(
        "runtime-rule.js",
        r#"
const NAME = "runtime-boom";
const SEVERITY = "low";
function on_template(template) {
  template.does.not.exist;
}
"#,
    );
    let output = cmd().arg(&path).arg("--rules").arg(&rule).output().unwrap();
    std::fs::remove_file(&path).ok();
    std::fs::remove_file(&rule).ok();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("runtime-boom"), "stderr was:\n{stderr}");
}
