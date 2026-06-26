use crate::{FormatRule, FormatRuleSet, ImportOrder};
use serde::Deserialize;
use serde_yaml::Value;
use std::collections::BTreeMap;
use std::error::Error;
use std::path::{Component, Path, PathBuf};

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
    InvalidImportOrder {
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
            Self::InvalidImportOrder { value } => {
                write!(
                    f,
                    "formatter import-order must be organize/preserve (got {value})"
                )
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
            Self::UnknownGroup { .. }
            | Self::InvalidRuleSetting { .. }
            | Self::InvalidImportOrder { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatConfig {
    groups: Vec<String>,
    rules: BTreeMap<String, RuleSwitch>,
    import_order: Option<ImportOrder>,
    ignore: Vec<IgnorePattern>,
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
        let Some(raw_fmt) = raw.daml_tools.and_then(|tools| tools.fmt) else {
            return Ok(None);
        };
        let config_dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
        Ok(Some(Self::from_raw(raw_fmt, &config_dir)?))
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

    /// Import declaration ordering strategy configured in `daml.yaml`, if any.
    #[must_use]
    pub const fn import_order(&self) -> Option<ImportOrder> {
        self.import_order
    }

    /// File ignore patterns configured under `daml-tools.fmt.ignore`.
    #[must_use]
    pub fn ignore_patterns(&self) -> &[IgnorePattern] {
        &self.ignore
    }

    fn from_raw(raw: RawFmtConfig, config_dir: &Path) -> Result<Self, FormatConfigError> {
        let import_order = raw
            .import_order
            .as_deref()
            .map(parse_import_order)
            .transpose()?;
        let ignore = raw
            .ignore
            .into_iter()
            .map(|pattern| IgnorePattern::new(config_dir.to_path_buf(), pattern))
            .collect();
        Ok(Self {
            groups: raw.groups,
            rules: raw.rules,
            import_order,
            ignore,
        })
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
    import_order: Option<String>,
    #[serde(default)]
    groups: Vec<String>,
    #[serde(default)]
    rules: BTreeMap<String, RuleSwitch>,
    #[serde(default)]
    ignore: Vec<String>,
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

/// Load ignore patterns from a formatter ignore file.
///
/// Blank lines and lines whose first non-whitespace character is `#` are
/// ignored. Patterns are resolved relative to the ignore file's directory.
///
/// # Errors
///
/// Returns [`FormatConfigError`] when the ignore file cannot be read.
pub fn load_ignore_file(path: &Path) -> Result<Vec<IgnorePattern>, FormatConfigError> {
    let source = std::fs::read_to_string(path).map_err(|source| FormatConfigError::ReadConfig {
        path: path.to_path_buf(),
        source,
    })?;
    let base_dir = path.parent().map_or_else(PathBuf::new, Path::to_path_buf);
    Ok(source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#'))
                .then(|| IgnorePattern::new(base_dir.clone(), line.to_string()))
        })
        .collect())
}

/// A formatter ignore pattern resolved relative to the config or ignore file
/// that declared it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IgnorePattern {
    base_dir: PathBuf,
    pattern: String,
}

impl IgnorePattern {
    /// Create an ignore pattern relative to `base_dir`.
    #[must_use]
    pub const fn new(base_dir: PathBuf, pattern: String) -> Self {
        Self { base_dir, pattern }
    }

    /// Returns true when `path` matches this pattern.
    #[must_use]
    pub fn is_match(&self, path: &Path) -> bool {
        let pattern = self.pattern.trim();
        let anchored_to_base = pattern.starts_with('/');
        let anchored = pattern.strip_prefix('/').unwrap_or(pattern);
        let (pattern_base, normalized_pattern) =
            normalize_ignore_pattern_base(&self.base_dir, anchored);
        let base = absolute_lexical(&pattern_base);
        let file = absolute_lexical(path);
        let relative = file.strip_prefix(&base).unwrap_or(&file);
        let relative = path_to_slash(relative);

        if normalized_pattern.ends_with('/') {
            let dir = normalized_pattern.trim_end_matches('/');
            return relative == dir || relative.starts_with(&format!("{dir}/"));
        }

        if !has_glob(&normalized_pattern) {
            if anchored_to_base || normalized_pattern.contains('/') {
                return relative == normalized_pattern;
            }
            return relative
                .split('/')
                .any(|component| component == normalized_pattern);
        }

        if anchored_to_base || normalized_pattern.contains('/') {
            glob_matches(&normalized_pattern, &relative)
        } else {
            relative
                .split('/')
                .any(|component| glob_matches(&normalized_pattern, component))
        }
    }
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

fn parse_import_order(value: &str) -> Result<ImportOrder, FormatConfigError> {
    match value {
        "organize" => Ok(ImportOrder::Organize),
        "preserve" => Ok(ImportOrder::Preserve),
        _ => Err(FormatConfigError::InvalidImportOrder {
            value: value.to_string(),
        }),
    }
}

fn value_label(value: &Value) -> String {
    serde_yaml::to_string(value)
        .map(|text| text.trim().to_string())
        .unwrap_or_else(|_| "value".to_string())
}

fn absolute_lexical(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    normalize_path(&absolute)
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn normalize_slashes(path: &str) -> String {
    path.replace('\\', "/")
}

fn normalize_ignore_pattern_base(base_dir: &Path, pattern: &str) -> (PathBuf, String) {
    let normalized = normalize_slashes(pattern);
    let has_trailing_slash = normalized.ends_with('/');
    let mut base = base_dir.to_path_buf();
    let mut components = Vec::new();
    for component in normalized.split('/') {
        match component {
            "" | "." => {}
            ".." if components.is_empty() => {
                base.pop();
            }
            ".." => {
                components.pop();
            }
            other => components.push(other.to_string()),
        }
    }
    let mut pattern = components.join("/");
    if has_trailing_slash && !pattern.is_empty() {
        pattern.push('/');
    }
    (base, pattern)
}

fn has_glob(pattern: &str) -> bool {
    pattern.contains('*')
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern = pattern.as_bytes();
    let text = text.as_bytes();
    let mut memo = vec![vec![None; text.len() + 1]; pattern.len() + 1];
    glob_matches_at(pattern, text, 0, 0, &mut memo)
}

fn glob_matches_at(
    pattern: &[u8],
    text: &[u8],
    pattern_index: usize,
    text_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][text_index] {
        return result;
    }
    let result = if pattern_index == pattern.len() {
        text_index == text.len()
    } else if pattern[pattern_index] == b'*' {
        let is_double_star = pattern.get(pattern_index + 1) == Some(&b'*');
        if is_double_star {
            glob_matches_at(pattern, text, pattern_index + 2, text_index, memo)
                || (text_index < text.len()
                    && glob_matches_at(pattern, text, pattern_index, text_index + 1, memo))
        } else {
            glob_matches_at(pattern, text, pattern_index + 1, text_index, memo)
                || (text_index < text.len()
                    && text[text_index] != b'/'
                    && glob_matches_at(pattern, text, pattern_index, text_index + 1, memo))
        }
    } else {
        text_index < text.len()
            && pattern[pattern_index] == text[text_index]
            && glob_matches_at(pattern, text, pattern_index + 1, text_index + 1, memo)
    };
    memo[pattern_index][text_index] = Some(result);
    result
}
