//! Shared decoding and diagnostic rules for untrusted configuration models.

use serde::{Deserialize, Deserializer};
use validator::{ValidationErrors, ValidationErrorsKind};

pub(crate) fn deserialize_optional_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

pub(crate) fn structural_validation_result(
    mut errors: ValidationErrors,
) -> Result<(), ValidationErrors> {
    if errors.is_empty() {
        return Ok(());
    }
    clear_validation_details(&mut errors);
    Err(errors)
}

// Reused derives may retain rejected values; keep only their codes and field paths.
fn clear_validation_details(errors: &mut ValidationErrors) {
    let mut fields: Vec<_> = errors.errors().keys().cloned().collect();
    fields.sort_unstable();
    for field in fields {
        if let Some(kind) = errors.errors_mut().get_mut(field.as_ref()) {
            match kind {
                ValidationErrorsKind::Struct(nested) => clear_validation_details(nested),
                ValidationErrorsKind::List(entries) => {
                    for nested in entries.values_mut() {
                        clear_validation_details(nested);
                    }
                }
                ValidationErrorsKind::Field(field_errors) => {
                    for error in field_errors {
                        error.params.clear();
                        error.message = None;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::structural_validation_result;
    use serde_json::json;
    use std::{borrow::Cow, collections::BTreeMap};
    use validator::{ValidationError, ValidationErrors, ValidationErrorsKind};

    #[test]
    fn redaction_preserves_codes_and_nested_paths_without_rejected_values() {
        let mut error = ValidationError::new("fake_static_code");
        error.message = Some(Cow::Borrowed("private_message_canary"));
        error.add_param(Cow::Borrowed("value"), &"private_value_canary");
        let mut child = ValidationErrors::new();
        child.add("field", error);
        let mut errors = ValidationErrors::new();
        errors.errors_mut().insert(
            Cow::Borrowed("object"),
            ValidationErrorsKind::Struct(Box::new(child.clone())),
        );
        errors.errors_mut().insert(
            Cow::Borrowed("items"),
            ValidationErrorsKind::List(BTreeMap::from([(2_usize, Box::new(child))])),
        );
        let redacted = structural_validation_result(errors).unwrap_err();
        assert_eq!(
            serde_json::to_value(redacted).unwrap(),
            json!({
                "object": {"field": [{"code": "fake_static_code", "message": null, "params": {}}]},
                "items": {"2": {"field": [{"code": "fake_static_code", "message": null, "params": {}}]}}
            })
        );
        structural_validation_result(ValidationErrors::new()).unwrap();
    }
}
