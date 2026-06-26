#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

fn read_golden(name: &str) -> String {
    std::fs::read_to_string(golden_path(name))
        .unwrap_or_else(|e| panic!("missing golden fixture {name}: {e}"))
}

fn assert_golden_normalized(name: &str, actual: &str, normalize: fn(&str) -> String) {
    let expected = read_golden(name).trim_end().to_string();
    let actual = normalize(actual).trim_end().to_string();
    assert_eq!(
        actual, expected,
        "golden mismatch for {name}\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
    );
}

fn normalize_abs_paths(text: &str) -> String {
    let mut out = text.to_string();
    let mut search_from = 0;
    while let Some(rel) = out[search_from..].find(".daml") {
        let daml_idx = search_from + rel;
        let path_start = out[..daml_idx]
            .rfind(|c: char| c.is_whitespace() || c == '`')
            .map(|i| i + 1)
            .unwrap_or(0);
        let path_end = daml_idx + ".daml".len();
        let path = &out[path_start..path_end];
        if should_normalize_path(path) {
            out.replace_range(path_start..path_end, "<PATH>");
            search_from = path_start + "<PATH>".len();
        } else {
            search_from = path_end;
        }
    }
    out
}

fn should_normalize_path(path: &str) -> bool {
    path.starts_with('/') || path.contains("daml-fmt-cli-")
}

fn normalize_path_string(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            if name.starts_with("daml-fmt-cli-") {
                "<PATH>".to_string()
            } else {
                name.to_string()
            }
        })
        .unwrap_or_else(|| "<PATH>".to_string())
}

fn normalize_cli_stderr(text: &str) -> String {
    normalize_abs_paths(text)
}

fn normalize_cli_stdout(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.ends_with(".daml") {
                normalize_path_string(line)
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daml-fmt"))
}

fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "daml-fmt-cli-{}-{}-{}",
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
        "daml-fmt-cli-{}-{}-{}",
        std::process::id(),
        id,
        name
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn help_exits_successfully() {
    let output = cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("usage: daml-fmt"));
    assert!(stderr.contains("--config <FILE>"));
    assert!(stderr.contains("--ignore-path <FILE>"));
    assert!(stderr.contains("--group <ID>"));
    assert!(stderr.contains("--rule <ID>"));
    assert_golden_normalized("cli_help_stderr.txt", &stderr, normalize_cli_stderr);
}

#[test]
fn version_exits_successfully() {
    let output = cmd().arg("--version").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_golden_normalized("cli_version_stdout.txt", &stdout, |text| text.to_string());
}

#[test]
fn unknown_option_exits_two() {
    let output = cmd().arg("--bogus").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown option"));
    assert_golden_normalized(
        "cli_unknown_option_stderr.txt",
        &stderr,
        normalize_cli_stderr,
    );
}

#[test]
fn stdin_formats_source_to_stdout() {
    let mut child = cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"module M where\nfoo : Int\nfoo = 1\n")
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\nfoo: Int\nfoo = 1\n"
    );
}

#[test]
fn check_reports_unformatted_file_with_exit_one() {
    let path = temp_file("unformatted.daml", "module M where\nfoo : Int\nfoo = 1\n");
    let output = cmd().arg("--check").arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(path.to_str().unwrap()));
    assert_golden_normalized(
        "cli_check_unformatted_stdout.txt",
        &stdout,
        normalize_cli_stdout,
    );
}

#[test]
fn check_reports_formatted_file_with_exit_zero() {
    let path = temp_file("formatted.daml", "module M where\nfoo: Int\nfoo = 1\n");
    let output = cmd().arg("--check").arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_golden_normalized(
        "cli_check_formatted_stdout.txt",
        &String::from_utf8_lossy(&output.stdout),
        normalize_cli_stdout,
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn preserve_import_order_disables_import_organization() {
    let input = "module M where\n\nimport DA.Optional\nimport Daml.Script\nimport DA.List\n\nx: Optional Text\nx = Some \"ok\"\n";
    let output = cmd()
        .arg("--preserve-import-order")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
}

#[test]
fn daml_yaml_can_preserve_import_order() {
    let project = temp_dir("import-order-config-project");
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    import-order: preserve
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport Daml.Script\nimport DA.List\n\nx: Optional Text\nx = Some \"ok\"\n";

    let output = cmd()
        .current_dir(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
}

#[test]
fn preserve_import_order_flag_overrides_daml_yaml_import_order() {
    let project = temp_dir("import-order-cli-project");
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    import-order: organize
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport Daml.Script\nimport DA.List\n\nx: Optional Text\nx = Some \"ok\"\n";

    let output = cmd()
        .current_dir(&project)
        .arg("--preserve-import-order")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
}

#[test]
fn rule_flag_runs_only_selected_formatter_rule() {
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";
    let output = cmd()
        .arg("--rule")
        .arg("imports")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\n\nimport DA.List\nimport DA.Optional\n\nfoo : Int\nfoo = 1\n"
    );
}

#[test]
fn daml_yaml_can_disable_formatter_rule() {
    let project = temp_dir("config-project");
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    groups: [all]
    rules:
      imports: off
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";

    let output = cmd()
        .current_dir(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo: Int\nfoo = 1\n"
    );
}

#[test]
fn daml_yaml_ignore_skips_matching_files_in_check_mode() {
    let project = temp_dir("config-ignore-project");
    let generated = project.join("generated");
    std::fs::create_dir(&generated).unwrap();
    let ignored = generated.join("NeedsFormatting.daml");
    std::fs::write(&ignored, "module M where\nfoo = if x then 1\n").unwrap();
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    ignore:
      - generated/**
"#,
    )
    .unwrap();

    let output = cmd()
        .current_dir(&project)
        .arg("--check")
        .arg(&ignored)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn ignore_path_skips_matching_files_in_write_mode() {
    let project = temp_dir("ignore-path-project");
    let ignored = project.join("vendor.daml");
    let original = "module M where\nfoo : Int\nfoo = 1\n";
    std::fs::write(&ignored, original).unwrap();
    std::fs::write(
        project.join(".damlfmtignore"),
        "\n# generated or vendored sources\nvendor.daml\n",
    )
    .unwrap();

    let output = cmd()
        .current_dir(&project)
        .arg("--ignore-path")
        .arg(".damlfmtignore")
        .arg("--write")
        .arg(&ignored)
        .output()
        .unwrap();
    let after = std::fs::read_to_string(&ignored).unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(after, original);
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn config_ignore_patterns_resolve_from_config_file_directory() {
    let project = temp_dir("relative-config-ignore-project");
    let config_dir = project.join("config");
    let generated_dir = project.join("generated");
    std::fs::create_dir(&config_dir).unwrap();
    std::fs::create_dir(&generated_dir).unwrap();
    let ignored = generated_dir.join("NeedsFormatting.daml");
    std::fs::write(&ignored, "module M where\nfoo = if x then 1\n").unwrap();
    std::fs::write(
        config_dir.join("formatter.yaml"),
        r#"daml-tools:
  fmt:
    ignore:
      - ../generated/**
"#,
    )
    .unwrap();

    let output = cmd()
        .current_dir(&project)
        .arg("--config")
        .arg("config/formatter.yaml")
        .arg("--check")
        .arg(&ignored)
        .output()
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}

#[test]
fn daml_yaml_can_disable_each_formatter_rule() {
    let cases = [
        (
            "imports",
            "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo: Int\nfoo = 1\n",
            "import DA.Optional\nimport DA.List",
            "foo: Int",
        ),
        (
            "layout",
            "module M where\nmain = do\n    pass\n",
            "main = do\n    pass",
            "module M where",
        ),
        (
            "spacing",
            "module M where\nfoo : Int\nfoo = 1\n",
            "foo : Int",
            "foo = 1",
        ),
        (
            "syntax-normalization",
            "module M where\ng x = if x then 1 else 2\n",
            "g x = if x then 1 else 2",
            "module M where",
        ),
    ];

    for (rule, input, disabled_rule_fragment, still_formats_fragment) in cases {
        let project = temp_dir(&format!("disable-{rule}-project"));
        std::fs::write(
            project.join("daml.yaml"),
            format!("daml-tools:\n  fmt:\n    groups: [all]\n    rules:\n      {rule}: off\n"),
        )
        .unwrap();

        let output = cmd()
            .current_dir(&project)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
                child.wait_with_output()
            })
            .unwrap();
        std::fs::remove_dir_all(&project).ok();

        assert!(output.status.success(), "rule {rule}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(disabled_rule_fragment),
            "disabled {rule} fragment missing from:\n{stdout}"
        );
        assert!(
            stdout.contains(still_formats_fragment),
            "other formatter behavior missing for {rule} from:\n{stdout}"
        );
    }
}

#[test]
fn implicit_daml_yaml_does_not_walk_parent_directories() {
    let project = temp_dir("parent-config-project");
    let child = project.join("child");
    std::fs::create_dir(&child).unwrap();
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    rules:
      imports: off
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";

    let output = cmd()
        .current_dir(&child)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("import DA.List\nimport DA.Optional"));
    assert!(stdout.contains("foo: Int"));
}

#[test]
fn cli_rule_selection_overrides_daml_yaml_selection() {
    let project = temp_dir("cli-over-config-project");
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    rules:
      imports: off
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";

    let output = cmd()
        .current_dir(&project)
        .arg("--rule")
        .arg("imports")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\n\nimport DA.List\nimport DA.Optional\n\nfoo : Int\nfoo = 1\n"
    );
}

#[test]
fn unknown_formatter_rule_exits_two() {
    let output = cmd().arg("--rule").arg("unknown-rule").output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown-rule"));
    assert_golden_normalized("cli_unknown_rule_stderr.txt", &stderr, normalize_cli_stderr);
}

#[test]
fn stdin_reports_parser_diagnostics_and_exits_two() {
    let input = "module M where\nfoo = if x then 1\n";
    let mut child = cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("daml-fmt: <stdin>"));
    assert_golden_normalized("cli_stdin_parser_stderr.txt", &stderr, normalize_cli_stderr);
}

#[test]
fn check_reports_parser_diagnostics_and_exits_two() {
    let path = temp_file(
        "parser-diagnostic-check.daml",
        "module M where\nfoo = if x then 1\n",
    );
    let output = cmd().arg("--check").arg(&path).output().unwrap();
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("daml-fmt: {}:", path.display())));
    assert_golden_normalized("cli_check_parser_stderr.txt", &stderr, normalize_cli_stderr);
    assert_eq!(source, "module M where\nfoo = if x then 1\n");
}

#[test]
fn write_reports_parser_diagnostics_and_does_not_modify_input() {
    let path = temp_file(
        "parser-diagnostic-write.daml",
        "module M where\nfoo = if x then 1\n",
    );
    let output = cmd().arg("--write").arg(&path).output().unwrap();
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains(&format!("daml-fmt: {}:", path.display())));
    assert_golden_normalized("cli_write_parser_stderr.txt", &stderr, normalize_cli_stderr);
    assert_eq!(source, "module M where\nfoo = if x then 1\n");
}

#[test]
fn write_reformats_file_in_place_with_exit_zero() {
    let path = temp_file(
        "write-formatted.daml",
        "module M where\nfoo : Int\nfoo = 1\n",
    );
    let output = cmd().arg("--write").arg(&path).output().unwrap();
    let after = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_golden_normalized(
        "cli_write_formatted_stdout.txt",
        &String::from_utf8_lossy(&output.stdout),
        normalize_cli_stdout,
    );
    assert_eq!(after, "module M where\nfoo: Int\nfoo = 1\n");
    assert_eq!(String::from_utf8_lossy(&output.stderr), "");
}
