//! Shared decoding and diagnostic rules for untrusted configuration models.

use std::{borrow::Borrow, collections::BTreeMap, sync::Arc};

use crate::activity::{Activity, ActivityOverride, ActivityOverrides, Condition, Rule};
use crate::types::PurposeOneTreatmentOverride;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    ser::{Error as _, SerializeMap as _},
};
use validator::{ValidationErrors, ValidationErrorsKind};

pub(crate) fn deserialize_optional_value<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

// Remote derives keep strict operator input separate from effective/baked leaf
// defaults, without parsing a second value tree or changing their public types.
#[derive(Deserialize, Serialize)]
#[serde(remote = "PurposeOneTreatmentOverride", deny_unknown_fields)]
struct TreatmentFields {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    enabled: Option<bool>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    access_allowed: Option<bool>,
}

#[derive(Deserialize)]
struct TreatmentInput(#[serde(with = "TreatmentFields")] PurposeOneTreatmentOverride);

pub(crate) fn deserialize_optional_treatment<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<PurposeOneTreatmentOverride>, D::Error> {
    TreatmentInput::deserialize(deserializer).map(|input| Some(input.0))
}

pub(crate) fn serialize_optional_treatment<S: Serializer>(
    value: impl Borrow<Option<PurposeOneTreatmentOverride>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value.borrow() {
        Some(treatment) => TreatmentFields::serialize(treatment, serializer),
        None => Err(S::Error::custom("an absent treatment must be omitted")),
    }
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "ActivityOverride", deny_unknown_fields)]
struct ActivityFields {
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    default: Option<bool>,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_rules",
        serialize_with = "serialize_optional_rules",
        skip_serializing_if = "Option::is_none"
    )]
    rules: Option<Arc<[Rule]>>,
}

#[derive(Deserialize)]
struct ActivityInput(#[serde(with = "ActivityFields")] ActivityOverride);

#[derive(Serialize)]
struct ActivityRef<'a>(#[serde(with = "ActivityFields")] &'a ActivityOverride);

#[derive(Deserialize, Serialize)]
#[serde(remote = "Rule", deny_unknown_fields)]
struct RuleFields {
    allow: bool,
    #[serde(
        default,
        deserialize_with = "deserialize_optional_value",
        skip_serializing_if = "Option::is_none"
    )]
    condition: Option<Condition>,
}

#[derive(Deserialize)]
struct RuleInput(#[serde(with = "RuleFields")] Rule);

#[derive(Serialize)]
struct RuleRef<'a>(#[serde(with = "RuleFields")] &'a Rule);

fn deserialize_optional_rules<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Arc<[Rule]>>, D::Error> {
    Vec::<RuleInput>::deserialize(deserializer)
        .map(|rules| Some(rules.into_iter().map(|rule| rule.0).collect()))
}

fn serialize_optional_rules<S: Serializer>(
    value: impl Borrow<Option<Arc<[Rule]>>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value.borrow() {
        Some(rules) => serializer.collect_seq(rules.iter().map(RuleRef)),
        None => Err(S::Error::custom("absent activity rules must be omitted")),
    }
}

pub(crate) fn deserialize_optional_activities<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<ActivityOverrides>, D::Error> {
    BTreeMap::<Activity, ActivityInput>::deserialize(deserializer).map(|activities| {
        Some(
            activities
                .into_iter()
                .map(|(key, value)| (key, value.0))
                .collect(),
        )
    })
}

pub(crate) fn serialize_optional_activities<S: Serializer>(
    value: impl Borrow<Option<ActivityOverrides>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let Some(activities) = value.borrow() else {
        return Err(S::Error::custom("absent activities must be omitted"));
    };
    let mut map = serializer.serialize_map(Some(activities.len()))?;
    for (key, activity) in activities {
        map.serialize_entry(key, &ActivityRef(activity))?;
    }
    map.end()
}

pub(crate) fn serialize_optional_u8_keyed_map<S, V>(
    value: impl Borrow<Option<BTreeMap<u8, V>>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    V: Serialize,
{
    struct EnforcementKey(u8);
    impl Serialize for EnforcementKey {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            serializer.collect_str(&self.0)
        }
    }
    let Some(entries) = value.borrow() else {
        return Err(S::Error::custom(
            "an absent enforcement map must be omitted",
        ));
    };
    let mut map = serializer.serialize_map(Some(entries.len()))?;
    for (key, entry) in entries {
        map.serialize_entry(&EnforcementKey(*key), entry)?;
    }
    map.end()
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
