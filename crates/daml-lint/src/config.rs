use daml_lint::detector::{ConfiguredDetector, Detector, Severity};
use daml_lint::detectors;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub enum ConfigError {
    MissingCurrentDir {
        source: std::io::Error,
    },
    ReadConfig {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseConfig {
        path: PathBuf,
        source: serde_json::Error,
    },
    PluginResolveFailed {
        plugin: String,
        tried: Vec<PathBuf>,
    },
    PluginManifestMissingSection {
        plugin: String,
        path: PathBuf,
    },
    PluginManifestRead {
        path: PathBuf,
        source: std::io::Error,
    },
    PluginManifestParse {
        path: PathBuf,
        source: serde_json::Error,
    },
    UnknownRuleId {
        rule_id: String,
    },
    MissingPluginRule {
        plugin: String,
        rule: String,
        package_json: PathBuf,
    },
    RuleNameMismatch {
        plugin: String,
        rule: String,
        script: PathBuf,
        name: String,
    },
    RuleLoadFailed {
        plugin: String,
        rule: String,
        path: PathBuf,
        source: Box<crate::detectors::script::ScriptLoadError>,
    },
    RuleSettingMissingSeverity {
        value: String,
    },
    RuleSettingInvalidSeverity {
        value: String,
    },
    RuleSettingInvalidType {
        kind: String,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingCurrentDir { source } => {
                write!(f, "could not resolve current directory: {source}")
            }
            Self::ReadConfig { path, source } => {
                write!(f, "could not read config {}: {source}", path.display())
            }
            Self::ParseConfig { path, source } => {
                write!(f, "invalid config {}: {source}", path.display())
            }
            Self::PluginResolveFailed { plugin, tried } => {
                let tried = tried
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(f, "could not resolve plugin '{plugin}'. Tried: {tried}")
            }
            Self::PluginManifestMissingSection { plugin, path } => write!(
                f,
                "plugin '{plugin}' package {} is missing damlLint.rules",
                path.display()
            ),
            Self::PluginManifestRead { path, source } => {
                write!(f, "could not read {}: {source}", path.display())
            }
            Self::PluginManifestParse { path, source } => {
                write!(f, "invalid {}: {source}", path.display())
            }
            Self::UnknownRuleId { rule_id } => {
                write!(f, "configures unknown rule '{rule_id}'")
            }
            Self::MissingPluginRule {
                plugin,
                rule,
                package_json,
            } => write!(
                f,
                "plugin '{plugin}' does not declare rule '{rule}' in {}",
                package_json.display()
            ),
            Self::RuleNameMismatch {
                plugin,
                rule,
                script,
                name,
            } => write!(
                f,
                "plugin '{plugin}' declares rule '{rule}' but {} defines NAME '{name}'",
                script.display()
            ),
            Self::RuleLoadFailed {
                plugin,
                rule,
                path,
                source,
            } => write!(
                f,
                "plugin '{plugin}' rule '{rule}' failed to load from {}: {source}",
                path.display()
            ),
            Self::RuleSettingMissingSeverity { value } => {
                write!(
                    f,
                    "rule setting array must include a severity or 'off': {value}"
                )
            }
            Self::RuleSettingInvalidSeverity { value } => {
                write!(
                    f,
                    "rule setting must be a severity, 'off', or an array containing one: {value}",
                )
            }
            Self::RuleSettingInvalidType { kind } => {
                write!(
                    f,
                    "rule setting must be a severity, 'off', or an array (got {kind})"
                )
            }
        }
    }
}

impl Error for ConfigError {}

#[derive(Debug)]
pub struct LintConfig {
    base_dir: PathBuf,
    plugin_paths: Vec<PathBuf>,
    plugins: Vec<String>,
    rules: BTreeMap<String, RuleSetting>,
}

impl LintConfig {
    /// Read and validate `.daml-lint.json`, returning defaults when missing.
    ///
    /// Returns:
    /// - `Ok` with an explicit config loaded from `explicit_path`, or
    ///   defaults when that file is absent.
    /// - `Err` when the config path is unreadable, JSON is invalid, or plugin
    ///   metadata cannot be parsed.
    #[must_use = "propagate config read/parse failures"]
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, ConfigError> {
        let Some(path) = find_config_path(explicit_path)? else {
            return Self::default_for_cwd();
        };

        let source = std::fs::read_to_string(&path).map_err(|source| ConfigError::ReadConfig {
            path: path.clone(),
            source,
        })?;
        let raw: RawConfig =
            serde_json::from_str(&source).map_err(|source| ConfigError::ParseConfig {
                path: path.clone(),
                source,
            })?;
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

    /// Load configured plugins into detector objects.
    ///
    /// Returns `Err` if plugin resolution fails, manifest loading fails, or any
    /// enabled plugin script cannot be loaded.
    #[must_use = "load plugin detectors and handle failures before linting"]
    pub fn load_plugin_detectors(&self) -> Result<Vec<Box<dyn Detector>>, ConfigError> {
        let mut detectors: Vec<Box<dyn Detector>> = Vec::new();
        for plugin in &self.plugins {
            let package_dir = self.resolve_plugin_package(plugin)?;
            let manifest = read_plugin_manifest(plugin, &package_dir)?;
            let namespace = plugin_namespace(plugin);

            for (rule_name, rule_id, setting) in self.enabled_rules_for_namespace(&namespace) {
                let Some(rule_path) = manifest.rules.get(rule_name) else {
                    return Err(ConfigError::MissingPluginRule {
                        plugin: plugin.to_string(),
                        rule: rule_name.to_string(),
                        package_json: package_dir.join("package.json"),
                    });
                };
                let script_path = package_dir.join(rule_path);
                let detector =
                    detectors::script::load_script_with_options(&script_path, &setting.options)
                        .map_err(|source| ConfigError::RuleLoadFailed {
                            plugin: plugin.to_string(),
                            rule: rule_name.to_string(),
                            path: script_path.clone(),
                            source: Box::new(source),
                        })?;
                if detector.name() != rule_name {
                    return Err(ConfigError::RuleNameMismatch {
                        plugin: plugin.to_string(),
                        rule: rule_name.to_string(),
                        script: script_path,
                        name: detector.name().to_string(),
                    });
                }
                detectors.push(Box::new(ConfiguredDetector::with_name(
                    detector,
                    rule_id.to_string(),
                )));
            }
        }
        Ok(detectors)
    }

    /// Apply configuration-level severity/enablement overrides to detectors in
    /// declaration order.
    #[must_use]
    pub fn apply_rule_settings(&self, detectors: Vec<Box<dyn Detector>>) -> Vec<Box<dyn Detector>> {
        detectors
            .into_iter()
            .filter_map(|detector| {
                let setting = self.rules.get(detector.name());
                if setting.is_some_and(|setting| !setting.enabled) {
                    return None;
                }
                let severity = setting.and_then(|setting| setting.severity);
                if let Some(severity) = severity {
                    let configured: Box<dyn Detector> =
                        Box::new(ConfiguredDetector::with_severity(detector, severity));
                    Some(configured)
                } else {
                    Some(detector)
                }
            })
            .collect()
    }

    /// Validate every enabled rule-id in config against the concrete detector set.
    ///
    /// Returns `Err` when the config references a rule-id that does not exist in
    /// `detectors`.
    #[must_use = "handle configuration validation errors before scanning"]
    pub fn validate_rule_settings(
        &self,
        detectors: &[Box<dyn Detector>],
    ) -> Result<(), ConfigError> {
        let detector_names: BTreeSet<&str> =
            detectors.iter().map(|detector| detector.name()).collect();
        for (rule_id, setting) in &self.rules {
            if setting.enabled && !detector_names.contains(rule_id.as_str()) {
                return Err(ConfigError::UnknownRuleId {
                    rule_id: rule_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn default_for_cwd() -> Result<Self, ConfigError> {
        Ok(Self {
            base_dir: std::env::current_dir()
                .map_err(|source| ConfigError::MissingCurrentDir { source })?,
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

    fn resolve_plugin_package(&self, plugin: &str) -> Result<PathBuf, ConfigError> {
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

        Err(ConfigError::PluginResolveFailed {
            plugin: plugin.to_string(),
            tried,
        })
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
    fn from_value(value: Value) -> Result<Self, ConfigError> {
        match value {
            Value::Array(items) => Self::from_array(&items),
            level => {
                Self::from_level_value(&level).map(|level| Self::from_level(level, empty_options()))
            }
        }
    }

    fn from_array(items: &[Value]) -> Result<Self, ConfigError> {
        let Some((level_value, option_values)) = items.split_first() else {
            return Err(ConfigError::RuleSettingMissingSeverity {
                value: "[]".to_string(),
            });
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

    fn from_level_value(value: &Value) -> Result<RuleLevel, ConfigError> {
        match value {
            Value::String(level) => parse_level_string(level),
            _ => Err(ConfigError::RuleSettingInvalidType {
                kind: serde_json::to_string(value).unwrap_or_else(|_| "value".to_string()),
            }),
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

fn find_config_path(explicit_path: Option<&Path>) -> Result<Option<PathBuf>, ConfigError> {
    if let Some(path) = explicit_path {
        return Ok(Some(path.to_path_buf()));
    }

    let path = std::env::current_dir()
        .map_err(|source| ConfigError::MissingCurrentDir { source })?
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

fn read_plugin_manifest(plugin: &str, package_dir: &Path) -> Result<PluginManifest, ConfigError> {
    let package_json_path = package_dir.join("package.json");
    let source = std::fs::read_to_string(&package_json_path).map_err(|source| {
        ConfigError::PluginManifestRead {
            path: package_json_path.clone(),
            source,
        }
    })?;
    let package_json: PackageJson =
        serde_json::from_str(&source).map_err(|source| ConfigError::PluginManifestParse {
            path: package_json_path.clone(),
            source,
        })?;
    package_json
        .daml_lint
        .ok_or_else(|| ConfigError::PluginManifestMissingSection {
            plugin: plugin.to_string(),
            path: package_json_path,
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

fn parse_level_string(level: &str) -> Result<RuleLevel, ConfigError> {
    match level.to_lowercase().as_str() {
        "off" => Ok(RuleLevel::Off),
        "critical" => Ok(RuleLevel::Severity(Severity::Critical)),
        "high" => Ok(RuleLevel::Severity(Severity::High)),
        "medium" => Ok(RuleLevel::Severity(Severity::Medium)),
        "low" => Ok(RuleLevel::Severity(Severity::Low)),
        "info" => Ok(RuleLevel::Severity(Severity::Info)),
        _ => Err(ConfigError::RuleSettingInvalidSeverity {
            value: format!("{level} (expected one of critical|high|medium|low|info|off)"),
        }),
    }
}

fn empty_options() -> Value {
    Value::Object(serde_json::Map::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_setting_parses_canonical_severities_and_off() {
        let off = RuleSetting::from_value(serde_json::json!("off")).unwrap();
        assert!(!off.enabled);
        assert_eq!(off.severity, None);
        assert_eq!(off.options, serde_json::json!({}));

        let high = RuleSetting::from_value(serde_json::json!("high")).unwrap();
        assert!(high.enabled);
        assert_eq!(high.severity, Some(Severity::High));
        assert_eq!(high.options, serde_json::json!({}));

        let medium_with_options =
            RuleSetting::from_value(serde_json::json!(["medium", { "names": ["Iou"] }])).unwrap();
        assert!(medium_with_options.enabled);
        assert_eq!(medium_with_options.severity, Some(Severity::Medium));
        assert_eq!(
            medium_with_options.options,
            serde_json::json!({ "names": ["Iou"] })
        );
    }

    #[test]
    fn rule_setting_rejects_numeric_levels_and_legacy_aliases() {
        assert!(matches!(
            RuleSetting::from_value(serde_json::json!(1)),
            Err(ConfigError::RuleSettingInvalidType { .. })
        ));
        assert!(matches!(
            RuleSetting::from_value(serde_json::json!("warn")),
            Err(ConfigError::RuleSettingInvalidSeverity { .. })
        ));
        assert!(matches!(
            RuleSetting::from_value(serde_json::json!("error")),
            Err(ConfigError::RuleSettingInvalidSeverity { .. })
        ));
    }

    #[test]
    fn rule_setting_requires_initial_severity_or_off() {
        assert!(matches!(
            RuleSetting::from_value(serde_json::json!([])),
            Err(ConfigError::RuleSettingMissingSeverity { .. })
        ));
    }
}
