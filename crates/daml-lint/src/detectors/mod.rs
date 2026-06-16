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
