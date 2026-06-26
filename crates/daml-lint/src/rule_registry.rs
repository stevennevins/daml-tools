//! Built-in rule ids and lint group expansion.

use std::collections::BTreeSet;

/// Stable kebab-case ids for built-in detectors shipped with `daml-lint`.
pub const BUILTIN_RULE_IDS: &[&str] = &[
    "missing-ensure-decimal",
    "unguarded-division",
    "head-of-list-query",
    "unbounded-fields",
    "missing-positive-amount",
    "archive-before-execute",
];

/// Built-in lint group names.
pub const BUILTIN_GROUP_ALL: &str = "all";
pub const BUILTIN_GROUP_OFF: &str = "off";
pub const BUILTIN_GROUP_RECOMMENDED: &str = "recommended";

/// Return the built-in rule ids enabled by the `recommended` group.
#[must_use]
pub fn recommended_builtin_rule_ids() -> BTreeSet<String> {
    BUILTIN_RULE_IDS
        .iter()
        .map(|id| (*id).to_string())
        .collect()
}

/// Expand a built-in lint group id to rule ids.
///
/// Returns `None` when `group` is not a built-in group name.
#[must_use]
pub fn expand_builtin_group(group: &str) -> Option<BTreeSet<String>> {
    match group {
        BUILTIN_GROUP_OFF => Some(BTreeSet::new()),
        BUILTIN_GROUP_RECOMMENDED | BUILTIN_GROUP_ALL => Some(recommended_builtin_rule_ids()),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn recommended_and_all_expand_to_same_builtin_set() {
        let recommended = expand_builtin_group(BUILTIN_GROUP_RECOMMENDED).unwrap();
        let all = expand_builtin_group(BUILTIN_GROUP_ALL).unwrap();
        assert_eq!(recommended, all);
        assert_eq!(recommended.len(), BUILTIN_RULE_IDS.len());
    }

    #[test]
    fn off_group_is_empty() {
        assert!(expand_builtin_group(BUILTIN_GROUP_OFF).unwrap().is_empty());
    }
}
