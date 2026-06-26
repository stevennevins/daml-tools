use daml_lint::detector::{ConfiguredDetector, Detector, Severity};
#[cfg(feature = "custom-rules")]
use daml_lint::detectors;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::path::{Path, PathBuf};

#[derive(Debug)]
#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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
    UnknownGroup {
        group: String,
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
            Self::UnknownGroup { group } => write!(f, "unknown lint rule group '{group}'"),
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
                    "rule setting array must include a severity, 'on', or 'off': {value}"
                )
            }
            Self::RuleSettingInvalidSeverity { value } => {
                write!(
                    f,
                    "rule setting must be a severity, 'on', 'off', or an array containing one: {value}",
                )
            }
            Self::RuleSettingInvalidType { kind } => {
                write!(
                    f,
                    "rule setting must be a severity, 'on', 'off', or an array (got {kind})"
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
            Self::UnknownGroup { .. }
            | Self::PluginResolveFailed { .. }
            | Self::PluginManifestMissingSection { .. }
            | Self::UnknownRuleId { .. }
            | Self::MissingPluginRule { .. }
            | Self::RuleNameMismatch { .. }
            | Self::RuleSettingMissingSeverity { .. }
            | Self::RuleSettingInvalidSeverity { .. }
            | Self::RuleSettingInvalidType { .. } => None,
        }
    }
}

#[derive(Debug)]
#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
pub struct LintConfig {
    base_dir: PathBuf,
    plugin_paths: Vec<PathBuf>,
    plugins: Vec<String>,
    groups: Vec<String>,
    rules: BTreeMap<String, RuleSetting>,
}

impl LintConfig {
    /// Read `daml-tools.lint` from `./daml.yaml`, returning defaults when missing.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the selected config path cannot be read or
    /// parsed, or when the current directory cannot be resolved.
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, ConfigError> {
        let Some(path) = find_config_path(explicit_path)? else {
            return Self::default_for_cwd();
        };

        let source = std::fs::read_to_string(&path).map_err(|source| ConfigError::ReadConfig {
            path: path.clone(),
            source,
        })?;
        let raw: RawRoot =
            serde_yaml::from_str(&source).map_err(|source| ConfigError::ParseConfig {
                path: path.clone(),
                source,
            })?;
        let base_dir = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        let raw = raw
            .daml_tools
            .and_then(|tools| tools.lint)
            .unwrap_or_default();
        let plugin_paths = raw
            .plugin_paths
            .into_iter()
            .map(|path| resolve_config_path(&base_dir, path))
            .collect();

        Ok(Self {
            base_dir,
            plugin_paths,
            plugins: raw.plugins,
            groups: raw.groups,
            rules: raw.rules,
        })
    }

    /// Load configured plugins into detector objects.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] if plugin resolution, manifest loading, or rule
    /// script loading fails.
    #[cfg(feature = "custom-rules")]
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

    /// Return no plugin detectors when custom rule support is disabled.
    ///
    /// # Errors
    ///
    /// This implementation does not fail; it returns `Ok([])`.
    #[cfg(not(feature = "custom-rules"))]
    pub fn load_plugin_detectors(&self) -> Result<Vec<Box<dyn Detector>>, ConfigError> {
        Ok(Vec::new())
    }

    /// Resolve config/CLI selection against loaded detector names.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a configured or CLI group is unknown.
    pub fn resolve_selection(
        &self,
        cli_selection: Option<RuleSelection>,
        detector_names: &BTreeSet<String>,
    ) -> Result<Option<RuleSelection>, ConfigError> {
        if cli_selection.is_some() {
            return Ok(cli_selection);
        }
        if self.groups.is_empty() && self.rules.is_empty() {
            return Ok(None);
        }
        let mut selected = if self.groups.is_empty() {
            detector_names.clone()
        } else {
            let mut selected = BTreeSet::new();
            for group in &self.groups {
                add_group(&mut selected, group, detector_names)?;
            }
            selected
        };
        for (rule_id, setting) in &self.rules {
            if setting.enabled {
                selected.insert(rule_id.clone());
            } else {
                selected.remove(rule_id);
            }
        }
        Ok(Some(RuleSelection { rule_ids: selected }))
    }

    /// Apply configuration-level severity/enablement overrides to detectors in
    /// declaration order.
    #[must_use]
    pub fn apply_rule_settings(
        &self,
        detectors: Vec<Box<dyn Detector>>,
        force_enabled: Option<&RuleSelection>,
    ) -> Vec<Box<dyn Detector>> {
        detectors
            .into_iter()
            .filter_map(|detector| {
                let setting = self.rules.get(detector.name());
                let is_force_enabled =
                    force_enabled.is_some_and(|selection| selection.contains(detector.name()));
                if setting.is_some_and(|setting| !setting.enabled) && !is_force_enabled {
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

    /// Validate every rule-id in config against the concrete detector set.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the config references a rule-id that does
    /// not exist in `detector_names`.
    pub fn validate_rule_settings(
        &self,
        detector_names: &BTreeSet<String>,
    ) -> Result<(), ConfigError> {
        for rule_id in self.rules.keys() {
            if !detector_names.contains(rule_id) {
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

    #[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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

    #[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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

    #[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleSelection {
    rule_ids: BTreeSet<String>,
}

impl RuleSelection {
    /// Resolve CLI rule/group ids into a selection.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when a CLI group is unknown.
    pub fn from_cli(
        rule_ids: &[String],
        group_ids: &[String],
        detector_names: &BTreeSet<String>,
    ) -> Result<Option<Self>, ConfigError> {
        if rule_ids.is_empty() && group_ids.is_empty() {
            return Ok(None);
        }
        let mut selected = BTreeSet::new();
        for group in group_ids {
            add_group(&mut selected, group, detector_names)?;
        }
        selected.extend(rule_ids.iter().cloned());
        Ok(Some(Self { rule_ids: selected }))
    }

    #[must_use]
    pub fn contains(&self, rule_id: &str) -> bool {
        self.rule_ids.contains(rule_id)
    }

    /// Validate selected rule ids against loaded detectors.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the selection references an unknown rule id.
    pub fn validate(&self, detector_names: &BTreeSet<String>) -> Result<(), ConfigError> {
        for rule_id in &self.rule_ids {
            if !detector_names.contains(rule_id) {
                return Err(ConfigError::UnknownRuleId {
                    rule_id: rule_id.clone(),
                });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct RawRoot {
    #[serde(rename = "daml-tools")]
    daml_tools: Option<RawDamlTools>,
}

#[derive(Debug, Deserialize)]
struct RawDamlTools {
    lint: Option<RawConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawConfig {
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
#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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
            RuleLevel::On => Self {
                enabled: true,
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
    On,
    Severity(Severity),
}

#[derive(Debug, Deserialize)]
#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
struct PackageJson {
    #[serde(rename = "damlLint")]
    daml_lint: Option<PluginManifest>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
struct PluginManifest {
    rules: BTreeMap<String, PathBuf>,
}

fn find_config_path(explicit_path: Option<&Path>) -> Result<Option<PathBuf>, ConfigError> {
    if let Some(path) = explicit_path {
        return Ok(Some(path.to_path_buf()));
    }

    let path = std::env::current_dir()
        .map_err(|source| ConfigError::MissingCurrentDir { source })?
        .join("daml.yaml");
    Ok(path.is_file().then_some(path))
}

fn resolve_config_path(base_dir: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        base_dir.join(path)
    }
}

#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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

#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
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

#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
fn plugin_namespace(plugin: &str) -> String {
    if let Some((scope, package)) = plugin.split_once('/') {
        format!("{scope}/{}", strip_plugin_prefix(package))
    } else {
        strip_plugin_prefix(plugin).to_string()
    }
}

#[cfg_attr(not(feature = "custom-rules"), allow(dead_code))]
fn strip_plugin_prefix(package: &str) -> &str {
    package.strip_prefix("daml-lint-plugin-").unwrap_or(package)
}

fn add_group(
    selected: &mut BTreeSet<String>,
    group: &str,
    detector_names: &BTreeSet<String>,
) -> Result<(), ConfigError> {
    match group {
        "all" | "recommended" => {
            selected.extend(detector_names.iter().cloned());
            Ok(())
        }
        _ => Err(ConfigError::UnknownGroup {
            group: group.to_string(),
        }),
    }
}

fn parse_level_string(level: &str) -> Result<RuleLevel, ConfigError> {
    match level.to_lowercase().as_str() {
        "off" => Ok(RuleLevel::Off),
        "on" => Ok(RuleLevel::On),
        "critical" => Ok(RuleLevel::Severity(Severity::Critical)),
        "error" | "high" => Ok(RuleLevel::Severity(Severity::High)),
        "warning" | "medium" => Ok(RuleLevel::Severity(Severity::Medium)),
        "low" => Ok(RuleLevel::Severity(Severity::Low)),
        "info" => Ok(RuleLevel::Severity(Severity::Info)),
        _ => Err(ConfigError::RuleSettingInvalidSeverity {
            value: format!(
                "{level} (expected one of critical|high|medium|low|info|error|warning|on|off)"
            ),
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
    fn rule_setting_parses_canonical_severities_aliases_on_and_off() {
        let off = RuleSetting::from_value(serde_json::json!("off")).unwrap();
        assert!(!off.enabled);
        assert_eq!(off.severity, None);
        assert_eq!(off.options, serde_json::json!({}));

        let on = RuleSetting::from_value(serde_json::json!("on")).unwrap();
        assert!(on.enabled);
        assert_eq!(on.severity, None);

        let high = RuleSetting::from_value(serde_json::json!("high")).unwrap();
        assert!(high.enabled);
        assert_eq!(high.severity, Some(Severity::High));

        let error = RuleSetting::from_value(serde_json::json!("error")).unwrap();
        assert_eq!(error.severity, Some(Severity::High));

        let warning = RuleSetting::from_value(serde_json::json!("warning")).unwrap();
        assert_eq!(warning.severity, Some(Severity::Medium));

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
}
