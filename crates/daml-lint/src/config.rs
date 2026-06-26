use crate::rule_registry::expand_builtin_group;
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
        source: serde_yaml::Error,
    },
    MissingLintSection {
        path: PathBuf,
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
    UnknownGroupId {
        group_id: String,
    },
    MissingPluginRule {
        plugin: String,
        rule: String,
        package_json: PathBuf,
    },
    MissingPluginGroup {
        plugin: String,
        group: String,
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
            Self::MissingLintSection { path } => write!(
                f,
                "config {} is missing daml-tools.lint section",
                path.display()
            ),
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
                write!(f, "unknown rule '{rule_id}'")
            }
            Self::UnknownGroupId { group_id } => {
                write!(f, "unknown group '{group_id}'")
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
            Self::MissingPluginGroup {
                plugin,
                group,
                package_json,
            } => write!(
                f,
                "plugin '{plugin}' does not declare group '{group}' in {}",
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

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingCurrentDir { source }
            | Self::ReadConfig { source, .. }
            | Self::PluginManifestRead { source, .. } => Some(source),
            Self::ParseConfig { source, .. } => Some(source),
            Self::PluginManifestParse { source, .. } => Some(source),
            Self::RuleLoadFailed { source, .. } => Some(source.as_ref()),
            Self::PluginResolveFailed { .. }
            | Self::PluginManifestMissingSection { .. }
            | Self::MissingLintSection { .. }
            | Self::UnknownRuleId { .. }
            | Self::UnknownGroupId { .. }
            | Self::MissingPluginRule { .. }
            | Self::MissingPluginGroup { .. }
            | Self::RuleNameMismatch { .. }
            | Self::RuleSettingMissingSeverity { .. }
            | Self::RuleSettingInvalidSeverity { .. }
            | Self::RuleSettingInvalidType { .. } => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSelectionSource {
    Default,
    Config,
    Cli,
}

#[derive(Debug)]
pub struct EffectiveRuleSelection {
    pub source: RuleSelectionSource,
    pub rule_ids: BTreeSet<String>,
}

#[derive(Debug)]
pub struct LintConfig {
    base_dir: PathBuf,
    plugin_paths: Vec<PathBuf>,
    plugins: Vec<String>,
    groups: Vec<String>,
    rules: BTreeMap<String, RuleSetting>,
}

impl LintConfig {
    /// Read and validate `daml-tools.lint` from YAML config, returning defaults when missing.
    ///
    /// Discovery checks `./daml.yaml` then `./daml.yml` unless `--config` is set.
    #[must_use = "propagate config read/parse failures"]
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, ConfigError> {
        let Some(path) = find_config_path(explicit_path)? else {
            return Self::default_for_cwd();
        };

        let source = std::fs::read_to_string(&path).map_err(|source| ConfigError::ReadConfig {
            path: path.clone(),
            source,
        })?;
        let raw: DamlToolsFile =
            serde_yaml::from_str(&source).map_err(|source| ConfigError::ParseConfig {
                path: path.clone(),
                source,
            })?;
        let Some(lint) = raw.daml_tools.and_then(|root| root.lint) else {
            if explicit_path.is_some() {
                return Err(ConfigError::MissingLintSection { path });
            }
            return Self::default_for_cwd();
        };
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let plugin_paths = lint
            .plugin_paths
            .into_iter()
            .map(|path| resolve_config_path(&base_dir, path))
            .collect();

        Ok(Self {
            base_dir,
            plugin_paths,
            plugins: lint.plugins,
            groups: lint.groups,
            rules: lint.rules,
        })
    }

    /// Resolve which rule ids should run from config and optional CLI overrides.
    #[must_use = "handle rule/group selection errors before scanning"]
    pub fn resolve_effective_rules(
        &self,
        cli_groups: &[String],
        cli_rules: &[String],
    ) -> Result<EffectiveRuleSelection, ConfigError> {
        if !cli_groups.is_empty() || !cli_rules.is_empty() {
            let mut rule_ids = BTreeSet::new();
            for group in cli_groups {
                rule_ids.extend(self.expand_group(group)?);
            }
            for rule_id in cli_rules {
                rule_ids.insert(rule_id.clone());
            }
            return Ok(EffectiveRuleSelection {
                source: RuleSelectionSource::Cli,
                rule_ids,
            });
        }

        if self.groups.is_empty() {
            return Ok(EffectiveRuleSelection {
                source: RuleSelectionSource::Default,
                rule_ids: BTreeSet::new(),
            });
        }

        let mut rule_ids = BTreeSet::new();
        for group in &self.groups {
            rule_ids.extend(self.expand_group(group)?);
        }
        for (rule_id, setting) in &self.rules {
            if setting.enabled {
                rule_ids.insert(rule_id.clone());
            } else {
                rule_ids.remove(rule_id);
            }
        }
        Ok(EffectiveRuleSelection {
            source: RuleSelectionSource::Config,
            rule_ids,
        })
    }

    /// Load configured plugins into detector objects.
    #[must_use = "load plugin detectors and handle failures before linting"]
    pub fn load_plugin_detectors(
        &self,
        selection: &EffectiveRuleSelection,
    ) -> Result<Vec<Box<dyn Detector>>, ConfigError> {
        let mut detectors: Vec<Box<dyn Detector>> = Vec::new();
        for plugin in &self.plugins {
            let package_dir = self.resolve_plugin_package(plugin)?;
            let manifest = read_plugin_manifest(plugin, &package_dir)?;
            let namespace = plugin_namespace(plugin);

            for (rule_name, rule_id, setting) in
                self.plugin_rules_to_load(plugin, &namespace, selection)
            {
                let Some(rule_path) = manifest.rules.get(&rule_name) else {
                    return Err(ConfigError::MissingPluginRule {
                        plugin: plugin.to_string(),
                        rule: rule_name,
                        package_json: package_dir.join("package.json"),
                    });
                };
                let script_path = package_dir.join(rule_path);
                let detector =
                    detectors::script::load_script_with_options(&script_path, &setting.options)
                        .map_err(|source| ConfigError::RuleLoadFailed {
                            plugin: plugin.to_string(),
                            rule: rule_name.clone(),
                            path: script_path.clone(),
                            source: Box::new(source),
                        })?;
                if detector.name() != rule_name {
                    return Err(ConfigError::RuleNameMismatch {
                        plugin: plugin.to_string(),
                        rule: rule_name,
                        script: script_path,
                        name: detector.name().to_string(),
                    });
                }
                detectors.push(Box::new(ConfiguredDetector::with_name(detector, rule_id)));
            }
        }
        Ok(detectors)
    }

    /// Apply configuration-level severity overrides to detectors in declaration order.
    #[must_use]
    pub fn apply_rule_settings(
        &self,
        detectors: Vec<Box<dyn Detector>>,
        respect_disabled: bool,
    ) -> Vec<Box<dyn Detector>> {
        detectors
            .into_iter()
            .filter_map(|detector| {
                let setting = self.rules.get(detector.name());
                if respect_disabled && setting.is_some_and(|setting| !setting.enabled) {
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

    /// Filter detectors to the resolved selection when one is active.
    #[must_use]
    pub fn filter_by_selection(
        &self,
        detectors: Vec<Box<dyn Detector>>,
        selection: &EffectiveRuleSelection,
    ) -> Vec<Box<dyn Detector>> {
        if selection.source == RuleSelectionSource::Default {
            return detectors;
        }
        detectors
            .into_iter()
            .filter(|detector| selection.rule_ids.contains(detector.name()))
            .collect()
    }

    /// Validate that every selected rule id exists in the loaded detector set.
    #[must_use = "handle unknown selected rules before scanning"]
    pub fn validate_selection_against_detectors(
        selection: &EffectiveRuleSelection,
        detectors: &[Box<dyn Detector>],
    ) -> Result<(), ConfigError> {
        if selection.source == RuleSelectionSource::Default {
            return Ok(());
        }
        let detector_names: BTreeSet<&str> =
            detectors.iter().map(|detector| detector.name()).collect();
        for rule_id in &selection.rule_ids {
            if !detector_names.contains(rule_id.as_str()) {
                return Err(ConfigError::UnknownRuleId {
                    rule_id: rule_id.clone(),
                });
            }
        }
        Ok(())
    }

    /// Validate every enabled rule-id in config against the concrete detector set.
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
            groups: Vec::new(),
            rules: BTreeMap::new(),
        })
    }

    fn expand_group(&self, group: &str) -> Result<BTreeSet<String>, ConfigError> {
        if let Some(rule_ids) = expand_builtin_group(group) {
            return Ok(rule_ids);
        }

        let Some((plugin, group_name)) = group.split_once('/') else {
            return Err(ConfigError::UnknownGroupId {
                group_id: group.to_string(),
            });
        };

        let package_dir = self.resolve_plugin_package(plugin)?;
        let manifest = read_plugin_manifest(plugin, &package_dir)?;
        let Some(rule_names) = manifest.groups.get(group_name) else {
            return Err(ConfigError::MissingPluginGroup {
                plugin: plugin.to_string(),
                group: group_name.to_string(),
                package_json: package_dir.join("package.json"),
            });
        };

        let namespace = plugin_namespace(plugin);
        Ok(rule_names
            .iter()
            .map(|rule_name| format!("{namespace}/{rule_name}"))
            .collect())
    }

    fn plugin_rules_to_load(
        &self,
        _plugin: &str,
        namespace: &str,
        selection: &EffectiveRuleSelection,
    ) -> Vec<(String, String, RuleSetting)> {
        let prefix = format!("{namespace}/");
        match selection.source {
            RuleSelectionSource::Default => self
                .rules
                .iter()
                .filter_map(|(rule_id, setting)| {
                    if !setting.enabled {
                        return None;
                    }
                    let rule_name = rule_id.strip_prefix(&prefix)?;
                    Some((rule_name.to_string(), rule_id.clone(), setting.clone()))
                })
                .collect(),
            RuleSelectionSource::Config => selection
                .rule_ids
                .iter()
                .filter_map(|rule_id| {
                    let rule_name = rule_id.strip_prefix(&prefix)?;
                    let setting = self
                        .rules
                        .get(rule_id)
                        .cloned()
                        .unwrap_or_else(|| RuleSetting {
                            enabled: true,
                            severity: None,
                            options: empty_options(),
                        });
                    if setting.enabled {
                        Some((rule_name.to_string(), rule_id.clone(), setting))
                    } else {
                        None
                    }
                })
                .collect(),
            RuleSelectionSource::Cli => selection
                .rule_ids
                .iter()
                .filter_map(|rule_id| {
                    let rule_name = rule_id.strip_prefix(&prefix)?;
                    let mut setting =
                        self.rules
                            .get(rule_id)
                            .cloned()
                            .unwrap_or_else(|| RuleSetting {
                                enabled: true,
                                severity: None,
                                options: empty_options(),
                            });
                    setting.enabled = true;
                    Some((rule_name.to_string(), rule_id.clone(), setting))
                })
                .collect(),
        }
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

#[derive(Debug, Deserialize)]
struct DamlToolsFile {
    #[serde(rename = "daml-tools")]
    daml_tools: Option<DamlToolsRoot>,
}

#[derive(Debug, Deserialize)]
struct DamlToolsRoot {
    lint: Option<RawLintConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawLintConfig {
    #[serde(default)]
    plugin_paths: Vec<PathBuf>,
    #[serde(default)]
    plugins: Vec<String>,
    #[serde(default)]
    groups: Vec<String>,
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
    #[serde(default)]
    groups: BTreeMap<String, Vec<String>>,
}

fn find_config_path(explicit_path: Option<&Path>) -> Result<Option<PathBuf>, ConfigError> {
    if let Some(path) = explicit_path {
        return Ok(Some(path.to_path_buf()));
    }

    let cwd =
        std::env::current_dir().map_err(|source| ConfigError::MissingCurrentDir { source })?;
    for name in ["daml.yaml", "daml.yml"] {
        let path = cwd.join(name);
        if path.is_file() {
            return Ok(Some(path));
        }
    }
    Ok(None)
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
#[allow(clippy::unwrap_used)]
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

    #[test]
    fn config_error_exposes_recoverable_source_errors() {
        let read = ConfigError::ReadConfig {
            path: PathBuf::from("daml.yaml"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "no access"),
        };
        assert!(std::error::Error::source(&read)
            .is_some_and(|source| source.to_string() == "no access"));

        let plugin = ConfigError::RuleLoadFailed {
            plugin: "example".to_string(),
            rule: "no-foo".to_string(),
            path: PathBuf::from("rules/no-foo.js"),
            source: Box::new(crate::detectors::script::ScriptLoadError::MissingName {
                label: "rules/no-foo.js".to_string(),
            }),
        };
        assert!(std::error::Error::source(&plugin)
            .is_some_and(|source| source.is::<crate::detectors::script::ScriptLoadError>()));
    }

    #[test]
    fn config_error_has_no_fake_source_for_validation_messages() {
        let err = ConfigError::UnknownRuleId {
            rule_id: "unknown-rule".to_string(),
        };
        assert!(std::error::Error::source(&err).is_none());
    }

    #[test]
    fn yaml_lint_section_parses_kebab_case_fields() {
        let yaml = r"
daml-tools:
  lint:
    plugin-paths: [./plugins]
    plugins: [template]
    groups: [recommended]
    rules:
      missing-ensure-decimal: off
";
        let raw: DamlToolsFile = serde_yaml::from_str(yaml).unwrap();
        let lint = raw.daml_tools.unwrap().lint.unwrap();
        assert_eq!(lint.plugin_paths, vec![PathBuf::from("./plugins")]);
        assert_eq!(lint.plugins, vec!["template".to_string()]);
        assert_eq!(lint.groups, vec!["recommended".to_string()]);
        assert!(!lint.rules.get("missing-ensure-decimal").unwrap().enabled);
    }
}
