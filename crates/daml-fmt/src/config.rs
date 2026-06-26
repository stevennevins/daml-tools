use crate::{FormatRule, FormatRuleSet};
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Path, PathBuf};

/// Error returned when formatter configuration cannot be loaded or resolved.
#[derive(Debug)]
pub enum FormatConfigError {
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
    UnknownRule {
        rule: String,
        source: crate::FormatRuleParseError,
    },
    InvalidRuleSetting {
        rule: String,
        value: String,
    },
}

impl std::fmt::Display for FormatConfigError {
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
            Self::UnknownGroup { group } => {
                write!(f, "unknown formatter rule group '{group}'")
            }
            Self::UnknownRule { rule, source } => {
                write!(f, "unknown formatter rule '{rule}': {source}")
            }
            Self::InvalidRuleSetting { rule, value } => {
                write!(f, "formatter rule '{rule}' must be on/off (got {value})")
            }
        }
    }
}

impl Error for FormatConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingCurrentDir { source } | Self::ReadConfig { source, .. } => Some(source),
            Self::ParseConfig { source, .. } => Some(source),
            Self::UnknownRule { source, .. } => Some(source),
            Self::UnknownGroup { .. } | Self::InvalidRuleSetting { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatConfig {
    groups: Vec<String>,
    rules: BTreeMap<String, RuleSwitch>,
}

impl FormatConfig {
    /// Load formatter config from an explicit YAML path or `./daml.yaml`.
    ///
    /// # Errors
    ///
    /// Returns [`FormatConfigError`] when the current directory cannot be
    /// resolved, the selected YAML file cannot be read, or the YAML is invalid.
    pub fn load(explicit_path: Option<&Path>) -> Result<Option<Self>, FormatConfigError> {
        let Some(path) = find_config_path(explicit_path)? else {
            return Ok(None);
        };
        let source =
            std::fs::read_to_string(&path).map_err(|source| FormatConfigError::ReadConfig {
                path: path.clone(),
                source,
            })?;
        let raw: RawRoot =
            serde_yaml::from_str(&source).map_err(|source| FormatConfigError::ParseConfig {
                path: path.clone(),
                source,
            })?;
        Ok(raw.daml_tools.and_then(|tools| tools.fmt).map(Into::into))
    }

    /// Resolve this config into an effective formatter rule set.
    ///
    /// # Errors
    ///
    /// Returns [`FormatConfigError`] when the config names an unknown rule group
    /// or formatter rule.
    pub fn rules(&self) -> Result<FormatRuleSet, FormatConfigError> {
        let mut rules = if self.groups.is_empty() {
            FormatRuleSet::all()
        } else {
            let mut set = FormatRuleSet::none();
            for group in &self.groups {
                add_group(&mut set, group)?;
            }
            set
        };
        for (rule_id, switch) in &self.rules {
            let rule = parse_rule(rule_id)?;
            if switch.enabled {
                rules.insert(rule);
            } else {
                rules.remove(rule);
            }
        }
        Ok(rules)
    }
}

#[derive(Debug, Deserialize)]
struct RawRoot {
    #[serde(rename = "daml-tools")]
    daml_tools: Option<RawDamlTools>,
}

#[derive(Debug, Deserialize)]
struct RawDamlTools {
    fmt: Option<RawFmtConfig>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawFmtConfig {
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    rules: BTreeMap<String, RuleSwitch>,
}

impl From<RawFmtConfig> for FormatConfig {
    fn from(raw: RawFmtConfig) -> Self {
        Self {
            groups: raw.groups,
            rules: raw.rules,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuleSwitch {
    enabled: bool,
}

impl<'de> Deserialize<'de> for RuleSwitch {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::Bool(enabled) => Ok(Self { enabled }),
            Value::String(text) => match text.to_lowercase().as_str() {
                "on" | "true" | "error" => Ok(Self { enabled: true }),
                "off" | "false" => Ok(Self { enabled: false }),
                _ => Err(serde::de::Error::custom(format!(
                    "expected on/off for formatter rule setting, got {text}"
                ))),
            },
            other => Err(serde::de::Error::custom(format!(
                "expected on/off for formatter rule setting, got {}",
                value_label(&other)
            ))),
        }
    }
}

fn find_config_path(explicit_path: Option<&Path>) -> Result<Option<PathBuf>, FormatConfigError> {
    if let Some(path) = explicit_path {
        return Ok(Some(path.to_path_buf()));
    }
    let path = std::env::current_dir()
        .map_err(|source| FormatConfigError::MissingCurrentDir { source })?
        .join("daml.yaml");
    Ok(path.is_file().then_some(path))
}

/// Resolve CLI rule/group selections into an effective formatter rule set.
///
/// Returns `Ok(None)` when no CLI rule or group selection was supplied.
///
/// # Errors
///
/// Returns [`FormatConfigError`] when a CLI rule or group id is unknown.
pub fn rules_from_cli(
    rule_ids: &[String],
    group_ids: &[String],
) -> Result<Option<FormatRuleSet>, FormatConfigError> {
    if rule_ids.is_empty() && group_ids.is_empty() {
        return Ok(None);
    }
    let mut rules = FormatRuleSet::none();
    for group in group_ids {
        add_group(&mut rules, group)?;
    }
    for rule_id in rule_ids {
        rules.insert(parse_rule(rule_id)?);
    }
    Ok(Some(rules))
}

fn add_group(rules: &mut FormatRuleSet, group: &str) -> Result<(), FormatConfigError> {
    match group {
        "all" => {
            *rules = FormatRuleSet::all();
            Ok(())
        }
        _ => Err(FormatConfigError::UnknownGroup {
            group: group.to_string(),
        }),
    }
}

fn parse_rule(rule_id: &str) -> Result<FormatRule, FormatConfigError> {
    rule_id
        .parse::<FormatRule>()
        .map_err(|source| FormatConfigError::UnknownRule {
            rule: rule_id.to_string(),
            source,
        })
}

fn value_label(value: &Value) -> String {
    serde_yaml::to_string(value)
        .map(|text| text.trim().to_string())
        .unwrap_or_else(|_| "value".to_string())
}
