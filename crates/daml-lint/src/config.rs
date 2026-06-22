use daml_lint::detector::{ConfiguredDetector, Detector, Severity};
use daml_lint::detectors;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct LintConfig {
    base_dir: PathBuf,
    plugin_paths: Vec<PathBuf>,
    plugins: Vec<String>,
    rules: BTreeMap<String, RuleSetting>,
}

impl LintConfig {
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, String> {
        let Some(path) = find_config_path(explicit_path)? else {
            return Self::default_for_cwd();
        };

        let source = std::fs::read_to_string(&path)
            .map_err(|e| format!("could not read config {}: {e}", path.display()))?;
        let raw: RawConfig = serde_json::from_str(&source)
            .map_err(|e| format!("invalid config {}: {e}", path.display()))?;
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let plugin_paths = raw
            .plugin_paths
            .into_iter()
            .map(|path| resolve_config_path(&base_dir, path))
            .collect();

        Ok(Self {
            base_dir,
            plugin_paths,
            plugins: raw.plugins,
            rules: raw.rules,
        })
    }

    pub fn load_plugin_detectors(&self) -> Result<Vec<Box<dyn Detector>>, String> {
        let mut detectors: Vec<Box<dyn Detector>> = Vec::new();
        for plugin in &self.plugins {
            let package_dir = self.resolve_plugin_package(plugin)?;
            let manifest = read_plugin_manifest(plugin, &package_dir)?;
            let namespace = plugin_namespace(plugin);

            for (rule_name, rule_id, setting) in self.enabled_rules_for_namespace(&namespace) {
                let Some(rule_path) = manifest.rules.get(rule_name) else {
                    return Err(format!(
                        "plugin '{}' does not declare rule '{}' in {}",
                        plugin,
                        rule_name,
                        package_dir.join("package.json").display()
                    ));
                };
                let script_path = package_dir.join(rule_path);
                let detector =
                    detectors::script::load_script_with_options(&script_path, &setting.options)?;
                if detector.name() != rule_name {
                    return Err(format!(
                        "plugin '{}' declares rule '{}' but {} defines NAME '{}'",
                        plugin,
                        rule_name,
                        script_path.display(),
                        detector.name()
                    ));
                }
                detectors.push(Box::new(ConfiguredDetector::new(
                    detector,
                    Some(rule_id.to_string()),
                    None,
                )));
            }
        }
        Ok(detectors)
    }

    pub fn apply_rule_settings(&self, detectors: Vec<Box<dyn Detector>>) -> Vec<Box<dyn Detector>> {
        detectors
            .into_iter()
            .filter_map(|detector| {
                let setting = self.rules.get(detector.name());
                if setting.is_some_and(|setting| !setting.enabled) {
                    return None;
                }
                let severity = setting.and_then(|setting| setting.severity);
                if severity.is_some() {
                    let configured: Box<dyn Detector> =
                        Box::new(ConfiguredDetector::new(detector, None, severity));
                    Some(configured)
                } else {
                    Some(detector)
                }
            })
            .collect()
    }

    pub fn validate_rule_settings(&self, detectors: &[Box<dyn Detector>]) -> Result<(), String> {
        let detector_names: BTreeSet<&str> =
            detectors.iter().map(|detector| detector.name()).collect();
        for (rule_id, setting) in &self.rules {
            if setting.enabled && !detector_names.contains(rule_id.as_str()) {
                return Err(format!("configures unknown rule '{rule_id}'"));
            }
        }
        Ok(())
    }

    fn default_for_cwd() -> Result<Self, String> {
        Ok(Self {
            base_dir: std::env::current_dir()
                .map_err(|e| format!("could not resolve current directory: {e}"))?,
            plugin_paths: Vec::new(),
            plugins: Vec::new(),
            rules: BTreeMap::new(),
        })
    }

    fn enabled_rules_for_namespace(
        &self,
        namespace: &str,
    ) -> impl Iterator<Item = (&str, &str, &RuleSetting)> {
        let prefix = format!("{namespace}/");
        self.rules.iter().filter_map(move |(rule_id, setting)| {
            if !setting.enabled {
                return None;
            }
            rule_id
                .strip_prefix(&prefix)
                .map(|rule_name| (rule_name, rule_id.as_str(), setting))
        })
    }

    fn resolve_plugin_package(&self, plugin: &str) -> Result<PathBuf, String> {
        let candidates = package_candidates(plugin);
        let mut tried = Vec::new();

        for package_name in &candidates {
            for package_dir in self.package_dirs(package_name) {
                tried.push(package_dir.clone());
                if package_dir.join("package.json").is_file() {
                    return Ok(package_dir);
                }
            }
        }

        let tried = tried
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        Err(format!(
            "could not resolve plugin '{plugin}'. Tried: {tried}"
        ))
    }

    fn package_dirs(&self, package_name: &str) -> Vec<PathBuf> {
        let mut dirs = vec![self.base_dir.join("node_modules").join(package_name)];
        for plugin_path in &self.plugin_paths {
            dirs.push(plugin_path.clone());
            dirs.push(plugin_path.join(package_name));
            dirs.push(plugin_path.join("node_modules").join(package_name));
        }
        dirs
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawConfig {
    #[serde(default)]
    plugin_paths: Vec<PathBuf>,
    #[serde(default)]
    plugins: Vec<String>,
    #[serde(default)]
    rules: BTreeMap<String, RuleSetting>,
}

#[derive(Debug, Clone)]
struct RuleSetting {
    enabled: bool,
    severity: Option<Severity>,
    options: Value,
}

impl RuleSetting {
    fn from_value(value: Value) -> Result<Self, String> {
        match value {
            Value::Array(items) => Self::from_array(&items),
            level => {
                Self::from_level_value(&level).map(|level| Self::from_level(level, empty_options()))
            }
        }
    }

    fn from_array(items: &[Value]) -> Result<Self, String> {
        let Some((level_value, option_values)) = items.split_first() else {
            return Err("rule setting array must include a severity or 'off'".to_string());
        };
        let level = Self::from_level_value(level_value)?;
        let options = match option_values {
            [] => empty_options(),
            [single] => single.clone(),
            many => Value::Array(many.to_vec()),
        };
        Ok(Self::from_level(level, options))
    }

    const fn from_level(level: RuleLevel, options: Value) -> Self {
        match level {
            RuleLevel::Off => Self {
                enabled: false,
                severity: None,
                options,
            },
            RuleLevel::Severity(severity) => Self {
                enabled: true,
                severity: Some(severity),
                options,
            },
        }
    }

    fn from_level_value(value: &Value) -> Result<RuleLevel, String> {
        match value {
            Value::String(level) => parse_level_string(level),
            Value::Number(level) => match level.as_u64() {
                Some(0) => Ok(RuleLevel::Off),
                Some(1) => Ok(RuleLevel::Severity(Severity::Medium)),
                Some(2) => Ok(RuleLevel::Severity(Severity::High)),
                _ => Err("numeric rule severity must be 0, 1, or 2".to_string()),
            },
            _ => Err("rule setting must be a severity, 'off', or an array".to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for RuleSetting {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        Self::from_value(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuleLevel {
    Off,
    Severity(Severity),
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    #[serde(rename = "damlLint")]
    daml_lint: Option<PluginManifest>,
}

#[derive(Debug, Deserialize)]
struct PluginManifest {
    rules: BTreeMap<String, PathBuf>,
}

fn find_config_path(explicit_path: Option<&Path>) -> Result<Option<PathBuf>, String> {
    if let Some(path) = explicit_path {
        return Ok(Some(path.to_path_buf()));
    }

    let path = std::env::current_dir()
        .map_err(|e| format!("could not resolve current directory: {e}"))?
        .join(".daml-lint.json");
    Ok(path.is_file().then_some(path))
}

fn resolve_config_path(base_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

fn read_plugin_manifest(plugin: &str, package_dir: &Path) -> Result<PluginManifest, String> {
    let package_json_path = package_dir.join("package.json");
    let source = std::fs::read_to_string(&package_json_path)
        .map_err(|e| format!("could not read {}: {e}", package_json_path.display()))?;
    let package_json: PackageJson = serde_json::from_str(&source)
        .map_err(|e| format!("invalid {}: {e}", package_json_path.display()))?;
    package_json.daml_lint.ok_or_else(|| {
        format!(
            "plugin '{plugin}' package {} is missing damlLint.rules",
            package_json_path.display()
        )
    })
}

fn package_candidates(plugin: &str) -> Vec<String> {
    if let Some((scope, package)) = plugin.split_once('/') {
        if package.starts_with("daml-lint-plugin-") {
            vec![plugin.to_string()]
        } else {
            vec![
                format!("{scope}/daml-lint-plugin-{package}"),
                plugin.to_string(),
            ]
        }
    } else if plugin.starts_with("daml-lint-plugin-") {
        vec![plugin.to_string()]
    } else {
        vec![format!("daml-lint-plugin-{plugin}"), plugin.to_string()]
    }
}

fn plugin_namespace(plugin: &str) -> String {
    if let Some((scope, package)) = plugin.split_once('/') {
        format!("{scope}/{}", strip_plugin_prefix(package))
    } else {
        strip_plugin_prefix(plugin).to_string()
    }
}

fn strip_plugin_prefix(package: &str) -> &str {
    package.strip_prefix("daml-lint-plugin-").unwrap_or(package)
}

fn parse_level_string(level: &str) -> Result<RuleLevel, String> {
    match level.to_lowercase().as_str() {
        "off" => Ok(RuleLevel::Off),
        "warn" | "warning" | "medium" => Ok(RuleLevel::Severity(Severity::Medium)),
        "error" | "high" => Ok(RuleLevel::Severity(Severity::High)),
        "critical" => Ok(RuleLevel::Severity(Severity::Critical)),
        "low" => Ok(RuleLevel::Severity(Severity::Low)),
        "info" => Ok(RuleLevel::Severity(Severity::Info)),
        _ => Err(format!(
            "unknown rule severity '{level}'. Use off, critical, high, medium, low, info, warn, or error."
        )),
    }
}

fn empty_options() -> Value {
    Value::Object(serde_json::Map::new())
}
