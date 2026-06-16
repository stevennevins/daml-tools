use crate::detector::{Detector, Finding, Severity};
use crate::ir::DamlModule;

/// Detector #4: unbounded-fields
///
/// For each template, identify fields of type `Text`, `TextMap a`, or `[a]` (list).
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
                .filter(|f| f.daml_type.is_unbounded())
                .collect();

            if unbounded_fields.is_empty() {
                continue;
            }

            // A size bound against a SIBLING field is attacker-controlled, so the
            // bound check needs the template's full field-name set.
            let field_names: Vec<String> = template.fields.iter().map(|f| f.name.clone()).collect();

            let mut unguarded_names = Vec::new();

            for field in &unbounded_fields {
                let has_bound = template
                    .ensure_clause
                    .as_ref()
                    .is_some_and(|ec| ec.has_size_bound(&field.name, &field_names));

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
                    format!("{} field", type_display(&f.daml_type))
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
        DamlType::Map(_, _) => "Map",
        // An Optional wrapper reports the kind of collection it carries.
        DamlType::Optional(inner) => type_display(inner),
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

    // Regression (round-3 F17): a LOWER bound on length (`length reason > 0`)
    // does not bound the size from above — the field is still unbounded.
    #[test]
    fn test_lower_length_bound_is_not_a_size_bound() {
        let source = r#"module Test where

template T
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure length reason > 0
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("reason")),
            "length reason > 0 is a lower bound; size still unbounded: {:?}",
            findings
        );
    }

    // An upper bound the other way round (`280 > length reason`) still counts.
    #[test]
    fn test_flipped_upper_length_bound_passes() {
        let source = r#"module Test where

template T
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure 280 > length reason
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.is_empty(),
            "280 > length reason bounds size: {:?}",
            findings
        );
    }

    // Regression (round-3 F31): the field name appearing inside a STRING literal
    // must not be read as a size bound. The tree ignores string contents.
    #[test]
    fn test_field_in_string_literal_is_not_bounded() {
        let source = r#"module Test where

template T
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure reason /= "length reason here"
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("reason")),
            "a `length reason` substring inside a string is not a real bound: {:?}",
            findings
        );
    }

    // Regression (audit round-3): an exact-size constraint `length tags == N`
    // bounds the size, so the field is not unbounded.
    #[test]
    fn test_exact_size_constraint_passes() {
        let source = r#"module Test where

template T
  with
    admin : Party
    tags : [Text]
  where
    signatory admin
    ensure length tags == 3
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        assert!(
            UnboundedFields.detect(&module).is_empty(),
            "length tags == 3 bounds the size: {:?}",
            UnboundedFields.detect(&module)
        );
    }

    // Regression (audit round-3): a size bound written through `this.` —
    // `length this.note < N`, which the parser reads as `(length this).note` —
    // still bounds the field.
    #[test]
    fn test_size_bound_through_this_passes() {
        let source = r#"module Test where

template T
  with
    admin : Party
    note : Text
  where
    signatory admin
    ensure length this.note < 280
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        assert!(
            UnboundedFields.detect(&module).is_empty(),
            "length this.note < 280 bounds note: {:?}",
            UnboundedFields.detect(&module)
        );
    }

    // Regression (audit round-3): an Optional-wrapped collection is still
    // unbounded when present.
    #[test]
    fn test_optional_collection_is_flagged() {
        let source = r#"module Test where

template T
  with
    owner : Party
    note : Optional Text
  where
    signatory owner
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("note")),
            "Optional Text is unbounded: {:?}",
            findings
        );
    }

    // Regression (audit round-3): a Map field reads grammatically (no
    // "unbounded unbounded field").
    #[test]
    fn test_map_field_message_is_grammatical() {
        let source = r#"module Test where

template T
  with
    owner : Party
    ctx : Map Text Int
  where
    signatory owner
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("unbounded Map field")),
            "Map field should read 'unbounded Map field': {:?}",
            findings
        );
    }

    // Regression (audit finding 5): `length tags < cap` where `cap` is a
    // SIBLING template field does NOT bound the size — the contract creator
    // sets the whole payload, so `cap` is attacker-controlled.
    #[test]
    fn test_sibling_field_bound_still_flagged() {
        let source = r#"module Test where

template T
  with
    owner : Party
    tags : [Text]
    cap : Int
  where
    signatory owner
    ensure length tags < cap
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("tags")),
            "length tags < cap (cap is a sibling field) does not bound size: {:?}",
            findings
        );
    }

    // Regression (audit finding 5): the flipped form `cap > length tags` with a
    // sibling-field bound is also unbounded.
    #[test]
    fn test_flipped_sibling_field_bound_still_flagged() {
        let source = r#"module Test where

template T
  with
    owner : Party
    tags : [Text]
    cap : Int
  where
    signatory owner
    ensure cap > length tags
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("tags")),
            "cap > length tags (cap is a sibling field) does not bound size: {:?}",
            findings
        );
    }

    // Regression (audit finding 5): the Map.size path with a sibling-field bound
    // is unbounded too.
    #[test]
    fn test_mapsize_sibling_field_bound_still_flagged() {
        let source = r#"module Test where

template T
  with
    owner : Party
    ctx : Map Text Text
    maxEntries : Int
  where
    signatory owner
    ensure Map.size ctx <= maxEntries
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("ctx")),
            "Map.size ctx <= maxEntries (maxEntries is a sibling field) does not bound size: {:?}",
            findings
        );
    }

    // Regression (audit finding 5): a module-level constant bound
    // (`length tags < maxTags`) is a real, non-attacker-controlled bound and
    // must keep passing — only sibling fields are rejected.
    #[test]
    fn test_module_constant_bound_passes() {
        let source = r#"module Test where

maxTags : Int
maxTags = 100

template T
  with
    owner : Party
    tags : [Text]
  where
    signatory owner
    ensure length tags < maxTags
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        assert!(
            UnboundedFields.detect(&module).is_empty(),
            "length tags < maxTags (module constant) bounds the size: {:?}",
            UnboundedFields.detect(&module)
        );
    }

    // Regression (audit finding 6): `length a == length b` forces only EQUAL
    // length — both lists can still grow without limit, so both are flagged.
    #[test]
    fn test_relational_length_equality_flags_both() {
        let source = r#"module Test where

template T
  with
    owner : Party
    a : [Text]
    b : [Text]
  where
    signatory owner
    ensure length a == length b
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'a'")),
            "length a == length b leaves a unbounded: {:?}",
            findings
        );
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("'b'") || f.message.contains(" b'")),
            "length a == length b leaves b unbounded: {:?}",
            findings
        );
    }

    // Regression (audit finding 6): a relational `<` between two lengths bounds
    // neither field.
    #[test]
    fn test_relational_length_less_than_flags_both() {
        let source = r#"module Test where

template T
  with
    owner : Party
    a : [Text]
    b : [Text]
  where
    signatory owner
    ensure length a < length b
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'a'")),
            "length a < length b leaves a unbounded: {:?}",
            findings
        );
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("'b'") || f.message.contains(" b'")),
            "length a < length b leaves b unbounded: {:?}",
            findings
        );
    }

    // Regression (audit finding 6): a relational `>` between two lengths bounds
    // neither field.
    #[test]
    fn test_relational_length_greater_than_flags_both() {
        let source = r#"module Test where

template T
  with
    owner : Party
    a : [Text]
    b : [Text]
  where
    signatory owner
    ensure length a > length b
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = UnboundedFields.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'a'")),
            "length a > length b leaves a unbounded: {:?}",
            findings
        );
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("'b'") || f.message.contains(" b'")),
            "length a > length b leaves b unbounded: {:?}",
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
