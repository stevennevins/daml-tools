use std::fmt;
use std::str::FromStr;

const IMPORTS: u8 = 1 << 0;
const LAYOUT: u8 = 1 << 1;
const SPACING: u8 = 1 << 2;
const SYNTAX_NORMALIZATION: u8 = 1 << 3;
const ALL: u8 = IMPORTS | LAYOUT | SPACING | SYNTAX_NORMALIZATION;

/// A discrete formatter rule that can be enabled or disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum FormatRule {
    /// Organize import declarations into formatter-defined groups.
    Imports,
    /// Apply AST-guided structural layout and indentation.
    Layout,
    /// Normalize whitespace gaps and type-annotation colon spacing.
    Spacing,
    /// Rewrite layout forms into canonical multiline shapes.
    SyntaxNormalization,
}

impl FormatRule {
    /// Stable kebab-case CLI/config id for this formatter rule.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Imports => "imports",
            Self::Layout => "layout",
            Self::Spacing => "spacing",
            Self::SyntaxNormalization => "syntax-normalization",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::Imports => IMPORTS,
            Self::Layout => LAYOUT,
            Self::Spacing => SPACING,
            Self::SyntaxNormalization => SYNTAX_NORMALIZATION,
        }
    }
}

impl fmt::Display for FormatRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Error returned when parsing an unknown formatter rule id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatRuleParseError {
    value: String,
}

impl FormatRuleParseError {
    /// Unsupported formatter rule id.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for FormatRuleParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown formatter rule '{}' (expected one of imports|layout|spacing|syntax-normalization)",
            self.value
        )
    }
}

impl std::error::Error for FormatRuleParseError {}

impl FromStr for FormatRule {
    type Err = FormatRuleParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "imports" => Ok(Self::Imports),
            "layout" => Ok(Self::Layout),
            "spacing" => Ok(Self::Spacing),
            "syntax-normalization" => Ok(Self::SyntaxNormalization),
            _ => Err(FormatRuleParseError {
                value: value.to_string(),
            }),
        }
    }
}

/// A normalized set of formatter rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatRuleSet {
    bits: u8,
}

impl Default for FormatRuleSet {
    fn default() -> Self {
        Self::all()
    }
}

impl FormatRuleSet {
    /// All formatter rules enabled.
    #[must_use]
    pub const fn all() -> Self {
        Self { bits: ALL }
    }

    /// No formatter rules enabled.
    #[must_use]
    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    /// Build a normalized rule set from rule ids.
    #[must_use]
    pub fn from_rules(rules: impl IntoIterator<Item = FormatRule>) -> Self {
        let mut set = Self::none();
        for rule in rules {
            set.insert(rule);
        }
        set
    }

    /// Enable one formatter rule.
    pub const fn insert(&mut self, rule: FormatRule) {
        self.bits |= rule.bit();
    }

    /// Disable one formatter rule.
    pub const fn remove(&mut self, rule: FormatRule) {
        self.bits &= !rule.bit();
    }

    /// Returns true when `rule` is enabled.
    #[must_use]
    pub const fn contains(self, rule: FormatRule) -> bool {
        self.bits & rule.bit() != 0
    }
}
