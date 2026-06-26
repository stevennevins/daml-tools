#![allow(clippy::unwrap_used, dead_code)]

use daml_lint::detector::Finding;
use std::path::{Path, PathBuf};

pub fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

pub fn read_golden(name: &str) -> String {
    std::fs::read_to_string(golden_path(name))
        .unwrap_or_else(|e| panic!("missing golden fixture {name}: {e}"))
}

pub fn assert_golden_normalized(name: &str, actual: &str, normalize: fn(&str) -> String) {
    let expected = read_golden(name).trim_end().to_string();
    let actual = normalize(actual).trim_end().to_string();
    assert_eq!(
        actual, expected,
        "golden mismatch for {name}\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
    );
}

pub fn normalize_abs_paths(text: &str) -> String {
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
    path.starts_with('/') || path.contains("daml-lint-cli-") || path.starts_with("bad-")
}

pub fn normalize_path_string(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            if name.starts_with("daml-lint-cli-") || name.starts_with("bad-") {
                "<PATH>".to_string()
            } else {
                name.to_string()
            }
        })
        .unwrap_or_else(|| "<PATH>".to_string())
}

pub fn normalize_markdown(text: &str) -> String {
    normalize_abs_paths(text)
}

pub fn normalize_json_report(text: &str) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(text).expect("json report should parse");
    normalize_json_value(&mut value);
    serde_json::to_string_pretty(&value).expect("normalized json should serialize")
}

fn normalize_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            if map.get("tool").and_then(|v| v.as_str()) == Some("daml-lint") {
                map.insert(
                    "version".to_string(),
                    serde_json::Value::String("<VERSION>".to_string()),
                );
            }
            for (key, child) in map.iter_mut() {
                if key == "file" {
                    if let Some(file) = child.as_str() {
                        *child = serde_json::Value::String(normalize_path_string(file));
                    }
                } else {
                    normalize_json_value(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                normalize_json_value(item);
            }
        }
        _ => {}
    }
}

pub fn compact_sarif_report(text: &str) -> String {
    let sarif: serde_json::Value = serde_json::from_str(text).expect("sarif should parse");
    let run = &sarif["runs"][0];
    let inv = &run["invocations"][0];

    let notifications: Vec<serde_json::Value> = inv["toolExecutionNotifications"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|note| {
            let loc = &note["locations"][0]["physicalLocation"];
            let region = &loc["region"];
            let mut obj = serde_json::Map::new();
            obj.insert("level".to_string(), note["level"].clone());
            obj.insert("message".to_string(), note["message"]["text"].clone());
            obj.insert(
                "category".to_string(),
                note["properties"]["category"].clone(),
            );
            obj.insert(
                "file".to_string(),
                serde_json::Value::String(normalize_path_string(
                    loc["artifactLocation"]["uri"].as_str().unwrap_or(""),
                )),
            );
            obj.insert("startLine".to_string(), region["startLine"].clone());
            obj.insert("startColumn".to_string(), region["startColumn"].clone());
            if !region["endColumn"].is_null() {
                obj.insert("endColumn".to_string(), region["endColumn"].clone());
            }
            serde_json::Value::Object(obj)
        })
        .collect();

    let results: Vec<serde_json::Value> = run["results"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|result| {
            let loc = &result["locations"][0]["physicalLocation"];
            let region = &loc["region"];
            serde_json::json!({
                "ruleId": result["ruleId"],
                "level": result["level"],
                "message": result["message"]["text"],
                "file": normalize_path_string(loc["artifactLocation"]["uri"].as_str().unwrap_or("")),
                "startLine": region["startLine"],
                "startColumn": region["startColumn"],
                "evidence": result["properties"]["evidence"],
            })
        })
        .collect();

    let rules: Vec<serde_json::Value> = run["tool"]["driver"]["rules"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .map(|rule| {
            serde_json::json!({
                "id": rule["id"],
                "level": rule["defaultConfiguration"]["level"],
            })
        })
        .collect();

    let compact = serde_json::json!({
        "executionSuccessful": inv["executionSuccessful"],
        "notifications": notifications,
        "results": results,
        "rules": rules,
    });
    serde_json::to_string_pretty(&compact).expect("compact sarif should serialize")
}

pub fn normalize_cli_stderr(text: &str) -> String {
    text.lines()
        .filter(|line| {
            !line.starts_with("daml-lint: scanning ") && !line.starts_with("daml-lint: parse [")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn normalize_cli_stdout(text: &str) -> String {
    normalize_markdown(text)
}

fn escape_summary_field(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', "\\n")
}

pub fn compact_builtin_finding_summary(finding: &Finding) -> String {
    let file = finding
        .file
        .file_name()
        .and_then(|name| name.to_str())
        .map(normalize_path_string)
        .unwrap_or_else(|| "<PATH>".to_string());
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        finding.detector,
        finding.severity,
        file,
        usize::from(finding.line),
        usize::from(finding.column),
        escape_summary_field(&finding.message),
        escape_summary_field(&finding.evidence),
    )
}

pub fn compact_builtin_findings_summary(findings: &[Finding]) -> String {
    let mut lines: Vec<String> = findings
        .iter()
        .map(compact_builtin_finding_summary)
        .collect();
    lines.sort();
    lines.join("\n")
}

pub fn builtin_summary_golden_name(rule_name: &str, case_name: &str) -> String {
    let slug = case_name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .fold(String::new(), |mut slug, ch| {
            if ch == '_' && slug.ends_with('_') {
                return slug;
            }
            slug.push(ch);
            slug
        })
        .trim_matches('_')
        .to_string();
    format!("builtin_{rule_name}_{slug}.txt")
}

pub fn assert_builtin_summary_golden(rule_name: &str, case_name: &str, findings: &[Finding]) {
    let golden_name = builtin_summary_golden_name(rule_name, case_name);
    let actual = compact_builtin_findings_summary(findings);
    if std::env::var_os("UPDATE_BUILTIN_GOLDENS").is_some() {
        std::fs::write(golden_path(&golden_name), format!("{actual}\n")).unwrap_or_else(|e| {
            panic!("failed to write builtin summary golden {golden_name}: {e}")
        });
        return;
    }
    assert_golden_normalized(&golden_name, &actual, |text| text.to_string());
}
