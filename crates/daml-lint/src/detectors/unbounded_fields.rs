use crate::detector::{Detector, Finding, Severity};
use crate::ir::DamlModule;

/// Detector #4: unbounded-fields
///
/// For each template, identify fields of type Text, TextMap a, or [a] (list).
/// Check that the ensure clause includes a size bound on these fields. Flag
/// fields with no corresponding bound.
///
/// Catches: M4 (unbounded context maps), M19 (unbounded text fields)
pub struct UnboundedFields;

impl Detector for UnboundedFields {
    fn name(&self) -> &str {
        "unbounded-fields"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn description(&self) -> &str {
        "Unbounded Text/TextMap/List field with no ensure clause bounding its size"
    }

    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        let mut findings = Vec::new();

        for template in &module.templates {
            let unbounded_fields: Vec<_> = template
                .fields
                .iter()
                .filter(|f| f.type_.is_unbounded())
                .collect();

            if unbounded_fields.is_empty() {
                continue;
            }

            let mut unguarded_names = Vec::new();

            for field in &unbounded_fields {
                let has_bound = template
                    .ensure_clause
                    .as_ref()
                    .is_some_and(|ec| ec.has_size_bound(&field.name));

                if !has_bound {
                    unguarded_names.push(field.name.clone());
                }
            }

            if !unguarded_names.is_empty() {
                let type_desc = if unguarded_names.len() == 1 {
                    let f = unbounded_fields
                        .iter()
                        .find(|f| f.name == unguarded_names[0])
                        .unwrap();
                    format!("{} field", type_display(&f.type_))
                } else {
                    "fields".to_string()
                };

                findings.push(Finding {
                    detector: self.name().to_string(),
                    severity: self.severity(),
                    file: template.span.file.clone(),
                    line: template.span.line,
                    column: template.span.column,
                    message: format!(
                        "Template '{}' has unbounded {} '{}' with no ensure clause bounding their length.",
                        template.name,
                        type_desc,
                        unguarded_names.join("', '"),
                    ),
                    evidence: format!(
                        "Fields without size bounds: {}",
                        unguarded_names.join(", ")
                    ),
                });
            }
        }

        findings
    }
}

fn type_display(t: &crate::ir::DamlType) -> &str {
    use crate::ir::DamlType;
    match t {
        DamlType::Text => "Text",
        DamlType::TextMap(_) => "TextMap",
        DamlType::List(_) => "List",
        _ => "unbounded",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_daml;
    use std::path::Path;

    #[test]
    fn test_unbounded_text_fields_triggers() {
        let source = r#"module Test where

template BuyTrafficRequest
  with
    admin : Party
    trackingId : Text
    memberId : Text
    synchronizerId : Text
    reason : Text
  where
    signatory admin
"#;
        let module = parse_daml(source, Path::new("BuyTrafficRequest.daml"));
        let findings = UnboundedFields.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("trackingId"));
        assert!(findings[0].message.contains("BuyTrafficRequest"));
    }

    #[test]
    fn test_unbounded_textmap_triggers() {
        let source = r#"module Test where

template Metadata
  with
    owner : Party
    context : TextMap Text
  where
    signatory owner
"#;
        let module = parse_daml(source, Path::new("Metadata.daml"));
        let findings = UnboundedFields.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("context"));
    }

    #[test]
    fn test_bounded_text_passes() {
        let source = r#"module Test where

template SafeRequest
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure T.length reason < 280
"#;
        let module = parse_daml(source, Path::new("Safe.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(findings.is_empty());
    }

    // Regression (sweep F25): Map/Set/GenMap are unbounded collections.
    #[test]
    fn test_map_field_is_flagged() {
        let source = r#"module Test where

template Meta
  with
    owner : Party
    ctx : Map Text Text
  where
    signatory owner
"#;
        let module = parse_daml(source, Path::new("Meta.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("ctx")),
            "{:?}",
            findings
        );
    }

    // Regression (sweep F15): bounding `reasons` must not suppress `reason`.
    #[test]
    fn test_prefix_sibling_field_still_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
    reason : Text
    reasons : Text
  where
    signatory admin
    ensure T.length reasons < 280
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'reason'")),
            "reason (no bound) must be flagged: {:?}",
            findings
        );
    }
}
