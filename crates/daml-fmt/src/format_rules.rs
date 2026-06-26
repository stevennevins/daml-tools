use std::fmt;

/// Coarse formatter rule units applied in deterministic fixed order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FormatRule {
    StructuralLayout,
    ImportOrder,
    LayoutRewrites,
    GapNormalization,
}

impl FormatRule {
    /// All formatter rules in application order.
    pub const ALL: [Self; 4] = [
        Self::StructuralLayout,
        Self::ImportOrder,
        Self::LayoutRewrites,
        Self::GapNormalization,
    ];

    /// Stable kebab-case rule id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::StructuralLayout => "structural-layout",
            Self::ImportOrder => "import-order",
            Self::LayoutRewrites => "layout-rewrites",
            Self::GapNormalization => "gap-normalization",
        }
    }

    /// Parse a rule id string.
    #[must_use]
    pub fn parse_id(id: &str) -> Option<Self> {
        match id {
            "structural-layout" => Some(Self::StructuralLayout),
            "import-order" => Some(Self::ImportOrder),
            "layout-rewrites" => Some(Self::LayoutRewrites),
            "gap-normalization" => Some(Self::GapNormalization),
            _ => None,
        }
    }
}

impl fmt::Display for FormatRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Selected formatter rules in deterministic application order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatRuleSet {
    bits: u8,
}

impl FormatRuleSet {
    const STRUCTURAL_LAYOUT_BIT: u8 = 1 << 0;
    const IMPORT_ORDER_BIT: u8 = 1 << 1;
    const LAYOUT_REWRITES_BIT: u8 = 1 << 2;
    const GAP_NORMALIZATION_BIT: u8 = 1 << 3;
    const ALL_BITS: u8 = Self::STRUCTURAL_LAYOUT_BIT
        | Self::IMPORT_ORDER_BIT
        | Self::LAYOUT_REWRITES_BIT
        | Self::GAP_NORMALIZATION_BIT;

    /// The default full formatter rule set.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            bits: Self::ALL_BITS,
        }
    }

    /// Build a rule set from explicit rule ids, preserving deterministic order.
    #[must_use]
    pub fn from_ids(ids: &[FormatRule]) -> Self {
        let mut bits = 0;
        for rule in ids {
            bits |= Self::bit_for(*rule);
        }
        Self { bits }
    }

    /// Whether `rule` is selected.
    #[must_use]
    pub const fn contains(&self, rule: FormatRule) -> bool {
        self.bits & Self::bit_for(rule) != 0
    }

    /// Selected rules in application order.
    pub fn iter(&self) -> impl Iterator<Item = FormatRule> + '_ {
        FormatRule::ALL
            .iter()
            .copied()
            .filter(|rule| self.contains(*rule))
    }

    const fn bit_for(rule: FormatRule) -> u8 {
        match rule {
            FormatRule::StructuralLayout => Self::STRUCTURAL_LAYOUT_BIT,
            FormatRule::ImportOrder => Self::IMPORT_ORDER_BIT,
            FormatRule::LayoutRewrites => Self::LAYOUT_REWRITES_BIT,
            FormatRule::GapNormalization => Self::GAP_NORMALIZATION_BIT,
        }
    }
}

impl Default for FormatRuleSet {
    fn default() -> Self {
        Self::all()
    }
}
