pub mod archive_before_execute;
pub mod ensure_decimal;
pub mod head_of_list;
pub mod positive_amount;
// The custom-rule JS engine (rquickjs) is optional; a library consumer that
// only wants the built-in detectors does not pull it in.
#[cfg(feature = "custom-rules")]
pub mod script;
pub mod unbounded_fields;
pub mod unguarded_division;

use crate::detector::Detector;
use archive_before_execute::ArchiveBeforeExecute;
use ensure_decimal::MissingEnsureDecimal;
use head_of_list::HeadOfListQuery;
use positive_amount::MissingPositiveAmount;
use unbounded_fields::UnboundedFields;
use unguarded_division::UnguardedDivision;

/// Built-in detectors shipped with `daml-lint`.
pub fn create_builtin_detectors() -> Vec<Box<dyn Detector>> {
    vec![
        Box::new(MissingEnsureDecimal),
        Box::new(UnguardedDivision),
        Box::new(HeadOfListQuery),
        Box::new(UnboundedFields),
        Box::new(MissingPositiveAmount),
        Box::new(ArchiveBeforeExecute),
    ]
}
