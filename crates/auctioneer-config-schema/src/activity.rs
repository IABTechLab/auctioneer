//! Shared baked/runtime activity schema. Evaluation borrows these immutable rules.

use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::num::NonZeroU32;
use std::sync::Arc;

pub const MAX_RULES_PER_ACTIVITY: usize = 128;
pub const MAX_CONDITION_ENTRIES: usize = 128;
pub const MAX_COMPONENT_NAME_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Activity {
    FetchBids,
    TransmitUfpd,
    TransmitPreciseGeo,
    TransmitTid,
    SyncUser,
    ReportAnalytics,
    EnrichUfpd,
    TransmitUniqueRequestIds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ComponentType {
    Bidder,
    Analytics,
    Rtd,
    General,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "serde's default in-place deserializer correctly replaces this copyable enum"
)]
impl<'de> Deserialize<'de> for ComponentType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        for (name, kind) in [
            ("bidder", Self::Bidder),
            ("analytics", Self::Analytics),
            ("rtd", Self::Rtd),
            ("general", Self::General),
        ] {
            if simple_fold_eq(&value, name) {
                return Ok(kind);
            }
        }
        Err(D::Error::unknown_variant(
            &value,
            &["bidder", "analytics", "rtd", "general"],
        ))
    }
}

/// Unicode 15 simple folding, qualified against Go 1.23.12 `strings.EqualFold`.
/// Does not normalize, expand characters, or allocate folded strings.
#[must_use]
pub fn simple_fold_eq(left: &str, right: &str) -> bool {
    fn folded(c: char) -> u32 {
        unicode_case_mapping::case_folded(c).map_or(u32::from(c), NonZeroU32::get)
    }
    left.chars().map(folded).eq(right.chars().map(folded))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields, rename_all = "camelCase")]
pub struct Condition {
    pub component_name: Vec<String>,
    pub component_type: Vec<ComponentType>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub allow: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActivityConfig {
    /// `None` means allow when no rule matches.
    pub default: Option<bool>,
    pub rules: Arc<[Rule]>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ActivityOverride {
    pub default: Option<bool>,
    /// `None` inherits; `Some([])` clears the entire inherited rule array.
    pub rules: Option<Arc<[Rule]>>,
}

pub type ActivityConfigs = Arc<BTreeMap<Activity, ActivityConfig>>;
pub type ActivityOverrides = BTreeMap<Activity, ActivityOverride>;

/// Validate the effective or baked activity map.
///
/// # Errors
/// Returns a fixed diagnostic when any per-activity/per-rule bound is exceeded.
pub fn validate_activities(
    activities: &BTreeMap<Activity, ActivityConfig>,
) -> Result<(), &'static str> {
    for config in activities.values() {
        validate_rules(&config.rules)?;
    }
    Ok(())
}

/// Validate supplied runtime rules before merging with baked settings.
///
/// # Errors
/// Returns a fixed diagnostic when a supplied rule array exceeds a bound.
pub fn validate_activity_overrides(overrides: &ActivityOverrides) -> Result<(), &'static str> {
    for config in overrides.values() {
        if let Some(rules) = config.rules.as_ref() {
            validate_rules(rules)?;
        }
    }
    Ok(())
}

fn validate_rules(rules: &[Rule]) -> Result<(), &'static str> {
    if rules.len() > MAX_RULES_PER_ACTIVITY {
        return Err("activity rules exceed 128 entries");
    }
    for rule in rules {
        if let Some(condition) = rule.condition.as_ref() {
            if condition
                .component_name
                .len()
                .saturating_add(condition.component_type.len())
                > MAX_CONDITION_ENTRIES
            {
                return Err("activity condition exceeds 128 combined entries");
            }
            if condition
                .component_name
                .iter()
                .any(|name| name.len() > MAX_COMPONENT_NAME_BYTES)
            {
                return Err("activity component name exceeds 128 UTF-8 bytes");
            }
        }
    }
    Ok(())
}

/// Merge only supplied fields. At most eight map entries are copied; rule contents stay shared.
pub fn merge_activities(base: &mut ActivityConfigs, overrides: &ActivityOverrides) {
    if overrides.is_empty() {
        return;
    }
    let entries = Arc::make_mut(base);
    for (&activity, overlay) in overrides {
        let entry = entries.entry(activity).or_default();
        if let Some(value) = overlay.default {
            entry.default = Some(value);
        }
        if let Some(rules) = overlay.rules.as_ref() {
            entry.rules = Arc::clone(rules);
        }
    }
}
