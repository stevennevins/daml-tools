use crate::format_rules::{FormatRule, FormatRuleSet};
use serde::Deserialize;
use std::collections::BTreeSet;
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
    MissingFmtSection {
        path: PathBuf,
    },
    UnknownRuleId {
        rule_id: String,
    },
    ImportOrderConflict,
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
            Self::MissingFmtSection { path } => write!(
                f,
                "config {} is missing daml-tools.fmt section",
                path.display()
            ),
            Self::UnknownRuleId { rule_id } => write!(f, "unknown rule '{rule_id}'"),
            Self::ImportOrderConflict => write!(
                f,
                "--rule import-order conflicts with --preserve-import-order"
            ),
        }
    }
}

impl Error for ConfigError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MissingCurrentDir { source } | Self::ReadConfig { source, .. } => Some(source),
            Self::ParseConfig { source, .. } => Some(source),
            Self::MissingFmtSection { .. }
            | Self::UnknownRuleId { .. }
            | Self::ImportOrderConflict => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleSelectionSource {
    Default,
    Config,
    Cli,
}

#[derive(Debug, Default)]
pub struct FmtConfig {
    config_rules: Option<Vec<String>>,
}

impl FmtConfig {
    /// Read `daml-tools.fmt` from YAML config, returning defaults when missing.
    ///
    /// Discovery checks `./daml.yaml` then `./daml.yml` unless `--config` is set.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] when the config file cannot be read or parsed, or
    /// when an explicit `--config` file is missing `daml-tools.fmt`.
    #[must_use = "propagate config read/parse failures"]
    pub fn load(explicit_path: Option<&Path>) -> Result<Self, ConfigError> {
        let Some(path) = find_config_path(explicit_path)? else {
            return Ok(Self::default());
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
        let Some(fmt) = raw.daml_tools.and_then(|root| root.fmt) else {
            if explicit_path.is_some() {
                return Err(ConfigError::MissingFmtSection { path });
            }
            return Ok(Self::default());
        };

        Ok(Self {
            config_rules: fmt.rules,
        })
    }

    /// Resolve selected formatter rules from config, CLI overrides, and flags.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError`] for unknown rule ids or for `--rule import-order`
    /// combined with `--preserve-import-order`.
    #[must_use = "handle rule selection errors before formatting"]
    pub fn resolve_effective_rules(
        &self,
        cli_rules: &[String],
        preserve_import_order: bool,
    ) -> Result<(FormatRuleSet, RuleSelectionSource), ConfigError> {
        let (mut selected, source) = if !cli_rules.is_empty() {
            let rules = parse_rule_ids(cli_rules)?;
            (rules, RuleSelectionSource::Cli)
        } else if let Some(config_rules) = &self.config_rules {
            let rules = parse_rule_ids(config_rules)?;
            (rules, RuleSelectionSource::Config)
        } else {
            (FormatRuleSet::all(), RuleSelectionSource::Default)
        };

        if preserve_import_order {
            if cli_rules.iter().any(|rule_id| rule_id == "import-order") {
                return Err(ConfigError::ImportOrderConflict);
            }
            selected = FormatRuleSet::from_ids(
                &selected
                    .iter()
                    .filter(|rule| *rule != FormatRule::ImportOrder)
                    .collect::<Vec<_>>(),
            );
        }

        Ok((selected, source))
    }
}

#[derive(Debug, Deserialize)]
struct DamlToolsFile {
    #[serde(rename = "daml-tools")]
    daml_tools: Option<DamlToolsRoot>,
}

#[derive(Debug, Deserialize)]
struct DamlToolsRoot {
    fmt: Option<RawFmtConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct RawFmtConfig {
    rules: Option<Vec<String>>,
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

fn parse_rule_ids(rule_ids: &[String]) -> Result<FormatRuleSet, ConfigError> {
    let mut seen = BTreeSet::new();
    let mut parsed = Vec::new();
    for rule_id in rule_ids {
        let Some(rule) = FormatRule::parse_id(rule_id) else {
            return Err(ConfigError::UnknownRuleId {
                rule_id: rule_id.clone(),
            });
        };
        if seen.insert(rule) {
            parsed.push(rule);
        }
    }
    Ok(FormatRuleSet::from_ids(&parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_rule_set_includes_all_rules_in_order() {
        let rules = FormatRuleSet::all();
        assert_eq!(rules.iter().collect::<Vec<_>>(), FormatRule::ALL.to_vec());
    }

    #[test]
    fn preserve_import_order_conflicts_with_explicit_cli_import_order() {
        let config = FmtConfig::default();
        let err = config
            .resolve_effective_rules(&["import-order".to_string()], true)
            .expect_err("explicit import-order with preserve flag must fail");
        assert!(matches!(err, ConfigError::ImportOrderConflict));
    }

    #[test]
    fn preserve_import_order_removes_import_order_from_defaults() {
        let config = FmtConfig::default();
        let (rules, source) = config
            .resolve_effective_rules(&[], true)
            .expect("default selection should succeed");
        assert_eq!(source, RuleSelectionSource::Default);
        assert!(!rules.contains(FormatRule::ImportOrder));
        assert!(rules.contains(FormatRule::GapNormalization));
    }

    #[test]
    fn cli_rules_replace_config_rules() {
        let config = FmtConfig {
            config_rules: Some(vec![
                "structural-layout".to_string(),
                "import-order".to_string(),
            ]),
        };
        let (rules, source) = config
            .resolve_effective_rules(&["gap-normalization".to_string()], false)
            .expect("cli rule selection should succeed");
        assert_eq!(source, RuleSelectionSource::Cli);
        assert_eq!(
            rules.iter().collect::<Vec<_>>(),
            vec![FormatRule::GapNormalization]
        );
    }
}
