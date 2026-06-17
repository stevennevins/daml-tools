pub mod archive_before_execute;
pub mod ensure_decimal;
pub mod head_of_list;
pub mod positive_amount;
#[cfg(feature = "js-runtime")]
pub mod script;
pub mod unbounded_fields;
pub mod unguarded_division;

#[cfg(all(test, feature = "js-runtime"))]
mod builtin_script_tests;

#[cfg(feature = "js-runtime")]
use crate::detector::Detector;

/// Built-in detectors shipped with `daml-lint`.
#[cfg(feature = "js-runtime")]
pub fn create_builtin_detectors() -> Vec<Box<dyn Detector>> {
    vec![
        built_in_rule(
            "rules/missing-ensure-decimal.js",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/rules/missing-ensure-decimal.js"
            )),
        ),
        built_in_rule(
            "rules/unguarded-division.js",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/rules/unguarded-division.js"
            )),
        ),
        built_in_rule(
            "rules/head-of-list-query.js",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/rules/head-of-list-query.js"
            )),
        ),
        built_in_rule(
            "rules/unbounded-fields.js",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/rules/unbounded-fields.js"
            )),
        ),
        built_in_rule(
            "rules/missing-positive-amount.js",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/rules/missing-positive-amount.js"
            )),
        ),
        built_in_rule(
            "rules/archive-before-execute.js",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/rules/archive-before-execute.js"
            )),
        ),
    ]
}

#[cfg(feature = "js-runtime")]
fn built_in_rule(label: &str, source: &str) -> Box<dyn Detector> {
    script::load_script_source(label, source)
        .unwrap_or_else(|e| panic!("invalid embedded built-in rule {label}: {e}"))
}
