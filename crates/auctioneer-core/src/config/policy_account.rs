//! Staged, explicit-context account policy resolution, separate from serving.

use std::collections::BTreeMap;

use auctioneer_config_data::types::{AccountConfig, PurposeOneTreatmentOverride, TriState};

use super::{
    AccountGdprOverride, ChannelEnabledOverride, PurposeEnforcementOverride,
    SpecialFeatureEnforcementOverride,
    override_value::FieldOverride,
    policy::{AccountGdprPolicy, AccountPolicy, OperatorPolicy},
    resolve::{AccountSelection, merge_account_gdpr, merge_account_privacy, merge_channels},
};

/// Purely compose product defaults, deployment defaults and a selected account.
///
/// This is non-admitting: it neither validates policy nor consults boot globals,
/// installs capacity or authorizes serving. Default selection never looks up a
/// synthesized account identity. Response caps are resolved separately.
#[must_use]
pub fn resolve_policy_account(
    product: &AccountConfig,
    policy: &OperatorPolicy,
    selection: &AccountSelection,
) -> AccountConfig {
    let mut account = product.clone();
    merge_policy_account(&mut account, &policy.account_defaults);
    if let AccountSelection::Named(id) = selection
        && let Some(overlay) = policy.accounts.get(id)
    {
        merge_policy_account(&mut account, overlay);
    }
    account
}

pub(super) fn merge_policy_account(base: &mut AccountConfig, overlay: &AccountPolicy) {
    if let Some(value) = overlay.disabled {
        base.disabled = value;
    }
    if let Some(value) = overlay.debug_allow {
        base.debug_allow = value;
    }
    if let Some(value) = overlay.bid_rounding.as_ref() {
        base.bid_rounding.clone_from(value);
    }
    if let Some(value) = overlay.targeting_prefix.as_ref() {
        base.targeting_prefix.clone_from(value);
    }
    apply_optional_override(
        &mut base.truncate_target_attribute,
        &overlay.truncate_target_attribute,
    );
    if let Some(sync) = overlay.cookie_sync.as_ref() {
        apply_optional_override(&mut base.cookie_sync.default_limit, &sync.default_limit);
        apply_optional_override(&mut base.cookie_sync.max_limit, &sync.max_limit);
        apply_optional_override(
            &mut base.cookie_sync.default_coop_sync,
            &sync.default_coop_sync,
        );
        apply_optional_override(&mut base.cookie_sync.priority_groups, &sync.priority_groups);
    }
    if let Some(gdpr) = overlay.gdpr.as_ref() {
        merge_account_gdpr(&mut base.gdpr, gdpr);
        if let Some(treatment) = gdpr.purpose_one_treatment.as_ref() {
            apply_optional_override(
                &mut base.gdpr.purpose_one_treatment.enabled,
                &treatment.enabled,
            );
            apply_optional_override(
                &mut base.gdpr.purpose_one_treatment.access_allowed,
                &treatment.access_allowed,
            );
        }
    }
    if let Some(ccpa) = overlay.ccpa.as_ref() {
        if let Some(value) = ccpa.enabled {
            base.ccpa.enabled = value;
        }
        if let Some(channels) = ccpa.channel_enabled.as_ref() {
            merge_channels(&mut base.ccpa.channel_enabled, channels);
        }
    }
    if let Some(privacy) = overlay.privacy.as_ref() {
        merge_account_privacy(base, privacy);
    }
}

pub(super) fn apply_optional_override<T: Clone>(base: &mut Option<T>, overlay: &FieldOverride<T>) {
    match overlay {
        FieldOverride::Inherit => {}
        FieldOverride::Value(value) => *base = Some(value.clone()),
        FieldOverride::Clear => *base = None,
    }
}

/// Borrowed inputs for the existing GDPR merger, without an owned DTO conversion.
pub(super) struct AccountGdprFields<'a> {
    pub enabled: Option<TriState>,
    pub purpose_one_treatment: Option<PurposeOneTreatmentOverride>,
    pub channel_enabled: Option<&'a ChannelEnabledOverride>,
    pub purposes: Option<&'a BTreeMap<u8, PurposeEnforcementOverride>>,
    pub special_features: Option<&'a BTreeMap<u8, SpecialFeatureEnforcementOverride>>,
    pub basic_enforcement_vendors: Option<&'a Vec<String>>,
}

impl<'a> From<&'a AccountGdprOverride> for AccountGdprFields<'a> {
    fn from(overlay: &'a AccountGdprOverride) -> Self {
        Self {
            enabled: overlay.enabled,
            purpose_one_treatment: overlay.purpose_one_treatment,
            channel_enabled: overlay.channel_enabled.as_ref(),
            purposes: overlay.purposes.as_ref(),
            special_features: overlay.special_features.as_ref(),
            basic_enforcement_vendors: overlay.basic_enforcement_vendors.as_ref(),
        }
    }
}

impl<'a> From<&'a AccountGdprPolicy> for AccountGdprFields<'a> {
    fn from(overlay: &'a AccountGdprPolicy) -> Self {
        Self {
            enabled: overlay.enabled,
            // Policy treatment leaves use their clear-aware merge above.
            purpose_one_treatment: None,
            channel_enabled: overlay.channel_enabled.as_ref(),
            purposes: overlay.purposes.as_ref(),
            special_features: overlay.special_features.as_ref(),
            basic_enforcement_vendors: overlay.basic_enforcement_vendors.as_ref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_policy_account;
    use crate::config::{policy::OperatorPolicy, resolve::AccountSelection};
    use auctioneer_config_data::{
        activity::{Activity, Rule},
        types::{AccountConfig, EnforceAlgo, TriState},
    };
    use serde_json::{Value, json};

    fn fake_product() -> AccountConfig {
        serde_json::from_value(json!({
            "disabled": true,
            "debug_allow": true,
            "bid_rounding": "fake_product_rounding",
            "targeting_prefix": "fake_product_prefix",
            "truncate_target_attribute": 18_usize,
            "cookie_sync": {
                "default_limit": 5_i64,
                "max_limit": 9_i64,
                "default_coop_sync": true,
                "priority_groups": [["fake_product_b", "fake_product_a"]]
            },
            "gdpr": {
                "enabled": "enabled",
                "channel_enabled": {
                    "web": "enabled", "app": "enabled", "video": "enabled",
                    "amp": "enabled", "dooh": "enabled"
                },
                "purpose_one_treatment": {"enabled": false, "access_allowed": false},
                "purposes": {
                    "1": {
                        "enforce_algo": "full", "enforce_purpose": "enabled",
                        "enforce_vendors": "enabled", "vendor_exceptions": ["fake_product_a"]
                    },
                    "2": {
                        "enforce_algo": "basic", "enforce_purpose": "disabled",
                        "enforce_vendors": "enabled", "vendor_exceptions": ["fake_product_b"]
                    }
                },
                "special_features": {
                    "1": {"enforce": "enabled", "vendor_exceptions": ["fake_product_a"]},
                    "2": {"enforce": "disabled", "vendor_exceptions": ["fake_product_b"]}
                },
                "basic_enforcement_vendors": ["fake_product_b", "fake_product_a"]
            },
            "ccpa": {
                "enabled": "enabled",
                "channel_enabled": {
                    "web": "enabled", "app": "enabled", "video": "enabled",
                    "amp": "enabled", "dooh": "enabled"
                }
            },
            "privacy": {
                "ipv4": {"anon_keep_bits": 24_u8},
                "ipv6": {"anon_keep_bits": 56_u8},
                "allowactivities": {
                    "fetchBids": {
                        "default": false,
                        "rules": [
                            {"allow": true, "condition": {"componentName": ["fake_product_a"]}},
                            {"allow": false, "condition": {"componentName": ["fake_product_b"]}}
                        ]
                    },
                    "syncUser": {"default": false, "rules": [{"allow": false}]}
                }
            }
        }))
        .expect("fake product account")
    }

    fn policy(document: Value) -> OperatorPolicy {
        serde_json::from_value(document).expect("fake operator policy")
    }

    fn nullable_values(account: &AccountConfig) -> Value {
        json!([
            account.truncate_target_attribute,
            account.cookie_sync.default_limit,
            account.cookie_sync.max_limit,
            account.cookie_sync.default_coop_sync,
            account.cookie_sync.priority_groups,
            account.gdpr.purpose_one_treatment.enabled,
            account.gdpr.purpose_one_treatment.access_allowed
        ])
    }

    #[test]
    fn omitted_policy_preserves_explicit_product_fields() {
        let product = fake_product();
        let result = resolve_policy_account(
            &product,
            &policy(json!({"schema_version": 1_u64})),
            &AccountSelection::Default,
        );
        assert!(result.disabled);
        assert!(result.debug_allow);
        assert_eq!(result.bid_rounding, "fake_product_rounding");
        assert_eq!(result.targeting_prefix, "fake_product_prefix");
        assert_eq!(
            nullable_values(&result),
            json!([
                18_usize,
                5_i64,
                9_i64,
                true,
                [["fake_product_b", "fake_product_a"]],
                false,
                false
            ])
        );
        assert_eq!(result.gdpr.purposes.len(), 2);
        assert_eq!(result.privacy.ipv4.anon_keep_bits, 24);
        assert_eq!(result.privacy.ipv6.anon_keep_bits, 56);
    }

    #[test]
    fn scalars_compose_product_defaults_and_only_the_selected_named_account() {
        let product = fake_product();
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"disabled": false, "targeting_prefix": "fake_defaults"},
            "accounts": {
                "fake_account": {"debug_allow": false, "targeting_prefix": "", "bid_rounding": ""},
                "fake_other": {"disabled": true, "targeting_prefix": "fake_other"}
            }
        }));
        let defaults = resolve_policy_account(&product, &policy, &AccountSelection::Default);
        assert!(!defaults.disabled);
        assert!(defaults.debug_allow);
        assert_eq!(defaults.targeting_prefix, "fake_defaults");
        assert_eq!(defaults.bid_rounding, "fake_product_rounding");

        let named = resolve_policy_account(
            &product,
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert!(!named.disabled);
        assert!(!named.debug_allow);
        assert_eq!(named.targeting_prefix, "");
        assert_eq!(named.bid_rounding, "");
        assert_eq!(named.truncate_target_attribute, Some(18));
        assert!(product.disabled);
        assert!(product.debug_allow);
        assert_eq!(product.targeting_prefix, "fake_product_prefix");
    }

    #[test]
    fn default_and_absent_names_never_select_the_unknown_account() {
        let product = fake_product();
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"disabled": false, "targeting_prefix": "fake_defaults"},
            "accounts": {"unknown": {"disabled": true, "targeting_prefix": "fake_unknown"}}
        }));
        for selection in [
            AccountSelection::Default,
            AccountSelection::Named("fake_absent".into()),
        ] {
            let result = resolve_policy_account(&product, &policy, &selection);
            assert!(!result.disabled);
            assert_eq!(result.targeting_prefix, "fake_defaults");
        }
        let named = resolve_policy_account(
            &product,
            &policy,
            &AccountSelection::Named("unknown".into()),
        );
        assert!(named.disabled);
        assert_eq!(named.targeting_prefix, "fake_unknown");
    }

    fn assert_clears_from_nonempty_parent(named: bool) {
        let paths = [
            "/truncate_target_attribute",
            "/cookie_sync/default_limit",
            "/cookie_sync/max_limit",
            "/cookie_sync/default_coop_sync",
            "/cookie_sync/priority_groups",
            "/gdpr/purpose_one_treatment/enabled",
            "/gdpr/purpose_one_treatment/access_allowed",
        ];
        for (index, path) in paths.into_iter().enumerate() {
            let product = fake_product();
            let mut document = json!({
                "schema_version": 1_u64,
                "account_defaults": {},
                "accounts": {"fake_account": {}}
            });
            let expected_parent = if named {
                document["account_defaults"] = json!({
                    "truncate_target_attribute": 16_usize,
                    "cookie_sync": {
                        "default_limit": 7_i64, "max_limit": 11_i64, "default_coop_sync": false,
                        "priority_groups": [["fake_defaults_b", "fake_defaults_a"]]
                    },
                    "gdpr": {"purpose_one_treatment": {"enabled": true, "access_allowed": true}}
                });
                json!([
                    16_usize,
                    7_i64,
                    11_i64,
                    false,
                    [["fake_defaults_b", "fake_defaults_a"]],
                    true,
                    true
                ])
            } else {
                json!([
                    18_usize,
                    5_i64,
                    9_i64,
                    true,
                    [["fake_product_b", "fake_product_a"]],
                    false,
                    false
                ])
            };
            let selection = if named {
                AccountSelection::Named("fake_account".into())
            } else {
                AccountSelection::Default
            };
            let before = resolve_policy_account(&product, &policy(document.clone()), &selection);
            assert_eq!(
                nullable_values(&before),
                expected_parent,
                "parent for {path}"
            );
            assert!(
                !nullable_values(&before)[index].is_null(),
                "nonempty parent for {path}"
            );

            let overlay = if named {
                &mut document["accounts"]["fake_account"]
            } else {
                &mut document["account_defaults"]
            };
            *overlay = json!({"cookie_sync": {}, "gdpr": {"purpose_one_treatment": {}}});
            let (parent, field) = path.rsplit_once('/').expect("clear field path");
            overlay
                .pointer_mut(parent)
                .expect("clear parent object")
                .as_object_mut()
                .expect("object")
                .insert(field.to_owned(), json!({"clear": true}));
            let result = resolve_policy_account(&product, &policy(document), &selection);
            let mut expected = expected_parent;
            expected[index] = json!(null);
            assert_eq!(nullable_values(&result), expected, "clear only {path}");
            assert_eq!(
                nullable_values(&product),
                json!([
                    18_usize,
                    5_i64,
                    9_i64,
                    true,
                    [["fake_product_b", "fake_product_a"]],
                    false,
                    false
                ])
            );
        }
    }

    #[test]
    fn account_defaults_clear_each_optional_leaf_from_nonempty_product() {
        assert_clears_from_nonempty_parent(false);
    }

    #[test]
    fn named_account_clears_each_optional_leaf_from_nonempty_defaults() {
        assert_clears_from_nonempty_parent(true);
    }

    #[test]
    fn named_values_can_replace_cleared_defaults_with_zero_false_and_empty_arrays() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {
                "truncate_target_attribute": {"clear": true},
                "cookie_sync": {
                    "default_limit": {"clear": true}, "max_limit": {"clear": true},
                    "default_coop_sync": {"clear": true}, "priority_groups": {"clear": true}
                },
                "gdpr": {"purpose_one_treatment": {
                    "enabled": {"clear": true}, "access_allowed": {"clear": true}
                }}
            },
            "accounts": {"fake_account": {
                "truncate_target_attribute": 0_usize,
                "cookie_sync": {"default_limit": 0_i64, "max_limit": 0_i64,
                    "default_coop_sync": false, "priority_groups": []},
                "gdpr": {"purpose_one_treatment": {"enabled": false, "access_allowed": false}}
            }}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(
            nullable_values(&result),
            json!([0_usize, 0_i64, 0_i64, false, [], false, false])
        );
    }

    #[test]
    fn omitted_clearable_fields_inherit_defaults_through_empty_objects() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {
                "truncate_target_attribute": 16_usize,
                "cookie_sync": {"default_limit": 7_i64, "max_limit": 11_i64,
                    "default_coop_sync": false, "priority_groups": []},
                "gdpr": {"purpose_one_treatment": {"enabled": true, "access_allowed": true}}
            },
            "accounts": {"fake_account": {"cookie_sync": {}, "gdpr": {"purpose_one_treatment": {}}}}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(
            nullable_values(&result),
            json!([16_usize, 7_i64, 11_i64, false, [], true, true])
        );
    }

    #[test]
    fn gdpr_keyed_entries_merge_fieldwise_and_preserve_unset_and_siblings() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"gdpr": {
                "enabled": "disabled",
                "purposes": {"1": {"enforce_algo": "basic", "enforce_vendors": "disabled"}},
                "special_features": {"1": {"enforce": "disabled"}}
            }},
            "accounts": {"fake_account": {"gdpr": {
                "enabled": "unset",
                "purposes": {"1": {"enforce_purpose": "unset"}, "3": {"enforce_vendors": "enabled"}},
                "special_features": {"1": {"enforce": "unset"}, "3": {"vendor_exceptions": ["fake_named"]}}
            }}}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(result.gdpr.enabled, TriState::Unset);
        assert_eq!(result.gdpr.purposes.len(), 3);
        let first = &result.gdpr.purposes[&1];
        assert_eq!(first.enforce_algo, EnforceAlgo::Basic);
        assert_eq!(first.enforce_purpose, TriState::Unset);
        assert_eq!(first.enforce_vendors, TriState::Disabled);
        assert_eq!(first.vendor_exceptions, ["fake_product_a"]);
        let sibling = &result.gdpr.purposes[&2];
        assert_eq!(sibling.enforce_algo, EnforceAlgo::Basic);
        assert_eq!(sibling.enforce_purpose, TriState::Disabled);
        assert_eq!(sibling.enforce_vendors, TriState::Enabled);
        assert_eq!(sibling.vendor_exceptions, ["fake_product_b"]);
        let inserted = &result.gdpr.purposes[&3];
        assert_eq!(inserted.enforce_algo, EnforceAlgo::Full);
        assert_eq!(inserted.enforce_purpose, TriState::Unset);
        assert_eq!(inserted.enforce_vendors, TriState::Enabled);
        assert!(inserted.vendor_exceptions.is_empty());
        assert_eq!(result.gdpr.special_features.len(), 3);
        assert_eq!(result.gdpr.special_features[&1].enforce, TriState::Unset);
        assert_eq!(
            result.gdpr.special_features[&1].vendor_exceptions,
            ["fake_product_a"]
        );
        assert_eq!(result.gdpr.special_features[&2].enforce, TriState::Disabled);
        assert_eq!(
            result.gdpr.special_features[&2].vendor_exceptions,
            ["fake_product_b"]
        );
        assert_eq!(result.gdpr.special_features[&3].enforce, TriState::Unset);
        assert_eq!(
            result.gdpr.special_features[&3].vendor_exceptions,
            ["fake_named"]
        );
    }

    #[test]
    fn nested_gdpr_and_ccpa_channels_merge_independently() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {
                "gdpr": {"channel_enabled": {"web": "disabled", "app": "disabled", "video": "disabled"}},
                "ccpa": {"enabled": "disabled", "channel_enabled": {"web": "disabled", "app": "disabled", "amp": "disabled"}}
            },
            "accounts": {"fake_account": {
                "gdpr": {"channel_enabled": {"web": "unset", "amp": "disabled"}},
                "ccpa": {"enabled": "unset", "channel_enabled": {"app": "unset", "dooh": "disabled"}}
            }}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        let gdpr = result.gdpr.channel_enabled;
        assert_eq!(
            [gdpr.web, gdpr.app, gdpr.video, gdpr.amp, gdpr.dooh],
            [
                TriState::Unset,
                TriState::Disabled,
                TriState::Disabled,
                TriState::Disabled,
                TriState::Enabled,
            ]
        );
        assert_eq!(result.ccpa.enabled, TriState::Unset);
        let ccpa = result.ccpa.channel_enabled;
        assert_eq!(
            [ccpa.web, ccpa.app, ccpa.video, ccpa.amp, ccpa.dooh],
            [
                TriState::Disabled,
                TriState::Unset,
                TriState::Enabled,
                TriState::Disabled,
                TriState::Disabled,
            ]
        );
    }

    #[test]
    fn empty_enforcement_maps_do_not_delete_parent_entries() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"gdpr": {
                "purposes": {"1": {"enforce_vendors": "disabled"}},
                "special_features": {"1": {"enforce": "unset"}}
            }},
            "accounts": {"fake_account": {"gdpr": {"purposes": {}, "special_features": {}}}}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(result.gdpr.purposes.len(), 2);
        assert_eq!(result.gdpr.purposes[&1].enforce_vendors, TriState::Disabled);
        assert_eq!(
            result.gdpr.purposes[&2].vendor_exceptions,
            ["fake_product_b"]
        );
        assert_eq!(result.gdpr.special_features.len(), 2);
        assert_eq!(result.gdpr.special_features[&1].enforce, TriState::Unset);
        assert_eq!(
            result.gdpr.special_features[&2].vendor_exceptions,
            ["fake_product_b"]
        );
    }

    #[test]
    fn ordered_arrays_replace_instead_of_appending_at_both_override_layers() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {
                "cookie_sync": {"priority_groups": [["fake_defaults_b", "fake_defaults_a"], ["fake_defaults_c"]]},
                "gdpr": {
                    "basic_enforcement_vendors": ["fake_defaults_b", "fake_defaults_a"],
                    "purposes": {"1": {"vendor_exceptions": ["fake_defaults_b", "fake_defaults_a"]}},
                    "special_features": {"1": {"vendor_exceptions": ["fake_defaults_b", "fake_defaults_a"]}}
                }
            },
            "accounts": {"fake_account": {
                "cookie_sync": {"priority_groups": [["fake_named_b"], ["fake_named_a"]]},
                "gdpr": {
                    "basic_enforcement_vendors": ["fake_named_b", "fake_named_a"],
                    "purposes": {"1": {"vendor_exceptions": ["fake_named_b", "fake_named_a"]}},
                    "special_features": {"1": {"vendor_exceptions": ["fake_named_b", "fake_named_a"]}}
                }
            }}
        }));
        let defaults = resolve_policy_account(&fake_product(), &policy, &AccountSelection::Default);
        assert_eq!(
            defaults.cookie_sync.priority_groups,
            Some(vec![
                vec!["fake_defaults_b".into(), "fake_defaults_a".into()],
                vec!["fake_defaults_c".into()],
            ])
        );
        assert_eq!(
            defaults.gdpr.basic_enforcement_vendors,
            ["fake_defaults_b", "fake_defaults_a"]
        );
        assert_eq!(
            defaults.gdpr.purposes[&1].vendor_exceptions,
            ["fake_defaults_b", "fake_defaults_a"]
        );
        assert_eq!(
            defaults.gdpr.special_features[&1].vendor_exceptions,
            ["fake_defaults_b", "fake_defaults_a"]
        );
        let named = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(
            named.cookie_sync.priority_groups,
            Some(vec![
                vec!["fake_named_b".into()],
                vec!["fake_named_a".into()],
            ])
        );
        assert_eq!(
            named.gdpr.basic_enforcement_vendors,
            ["fake_named_b", "fake_named_a"]
        );
        assert_eq!(
            named.gdpr.purposes[&1].vendor_exceptions,
            ["fake_named_b", "fake_named_a"]
        );
        assert_eq!(
            named.gdpr.special_features[&1].vendor_exceptions,
            ["fake_named_b", "fake_named_a"]
        );
    }

    #[test]
    fn empty_arrays_remove_nonempty_parent_arrays_without_touching_siblings() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"gdpr": {"basic_enforcement_vendors": ["fake_defaults"]}},
            "accounts": {"fake_account": {
                "cookie_sync": {"priority_groups": []},
                "gdpr": {"basic_enforcement_vendors": [],
                    "purposes": {"1": {"vendor_exceptions": []}},
                    "special_features": {"1": {"vendor_exceptions": []}}}
            }}
        }));
        let before = resolve_policy_account(&fake_product(), &policy, &AccountSelection::Default);
        assert_eq!(before.gdpr.basic_enforcement_vendors, ["fake_defaults"]);
        assert_eq!(
            before.gdpr.purposes[&1].vendor_exceptions,
            ["fake_product_a"]
        );
        assert_eq!(
            before.gdpr.special_features[&1].vendor_exceptions,
            ["fake_product_a"]
        );
        assert_eq!(
            before.cookie_sync.priority_groups,
            Some(vec![vec!["fake_product_b".into(), "fake_product_a".into()]])
        );
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(result.cookie_sync.priority_groups, Some(Vec::new()));
        assert!(result.gdpr.basic_enforcement_vendors.is_empty());
        assert!(result.gdpr.purposes[&1].vendor_exceptions.is_empty());
        assert!(
            result.gdpr.special_features[&1]
                .vendor_exceptions
                .is_empty()
        );
        assert_eq!(
            result.gdpr.purposes[&2].vendor_exceptions,
            ["fake_product_b"]
        );
        assert_eq!(
            result.gdpr.special_features[&2].vendor_exceptions,
            ["fake_product_b"]
        );
    }

    #[test]
    fn privacy_families_compose_without_overwriting_the_other_family() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"privacy": {"ipv4": {"anon_keep_bits": 16_u8}}},
            "accounts": {"fake_account": {"privacy": {"ipv4": {}, "ipv6": {"anon_keep_bits": 0_u8}}}}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        assert_eq!(result.privacy.ipv4.anon_keep_bits, 16);
        assert_eq!(result.privacy.ipv6.anon_keep_bits, 0);
    }

    #[test]
    fn child_empty_activity_rules_preserve_parent_false_default() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"privacy": {"allowactivities": {
                "fetchBids": {"default": false, "rules": [{"allow": true}]}
            }}},
            "accounts": {"fake_account": {"privacy": {"allowactivities": {"fetchBids": {"rules": []}}}}}
        }));
        let before = resolve_policy_account(&fake_product(), &policy, &AccountSelection::Default);
        assert_eq!(
            before.privacy.allowactivities[&Activity::FetchBids].default,
            Some(false)
        );
        assert_eq!(
            before.privacy.allowactivities[&Activity::FetchBids]
                .rules
                .as_ref(),
            [Rule {
                allow: true,
                condition: None
            }]
        );
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        let activity = &result.privacy.allowactivities[&Activity::FetchBids];
        assert_eq!(activity.default, Some(false));
        assert!(activity.rules.is_empty());
        assert_eq!(
            result.privacy.allowactivities[&Activity::SyncUser].default,
            Some(false)
        );
        assert_eq!(
            result.privacy.allowactivities[&Activity::SyncUser]
                .rules
                .as_ref(),
            [Rule {
                allow: false,
                condition: None
            }]
        );
    }

    #[test]
    fn activity_defaults_rules_and_keys_remain_independent() {
        let policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"privacy": {"allowactivities": {
                "fetchBids": {"default": true},
                "syncUser": {"rules": [{"allow": true}]},
                "transmitTid": {"default": false, "rules": [{"allow": false}]}
            }}},
            "accounts": {"fake_account": {"privacy": {"allowactivities": {
                "fetchBids": {"default": false},
                "syncUser": {"default": true},
                "transmitTid": {"rules": [{"allow": true}, {"allow": false}]}
            }}}}
        }));
        let result = resolve_policy_account(
            &fake_product(),
            &policy,
            &AccountSelection::Named("fake_account".into()),
        );
        let fetch = &result.privacy.allowactivities[&Activity::FetchBids];
        assert_eq!(fetch.default, Some(false));
        assert_eq!(
            serde_json::to_value(&fetch.rules).expect("rules"),
            json!([
                {"allow": true, "condition": {"componentName": ["fake_product_a"], "componentType": []}},
                {"allow": false, "condition": {"componentName": ["fake_product_b"], "componentType": []}}
            ])
        );
        let sync = &result.privacy.allowactivities[&Activity::SyncUser];
        assert_eq!(sync.default, Some(true));
        assert_eq!(
            sync.rules.as_ref(),
            [Rule {
                allow: true,
                condition: None
            }]
        );
        let tid = &result.privacy.allowactivities[&Activity::TransmitTid];
        assert_eq!(tid.default, Some(false));
        assert_eq!(
            tid.rules.as_ref(),
            [
                Rule {
                    allow: true,
                    condition: None
                },
                Rule {
                    allow: false,
                    condition: None
                }
            ]
        );
        assert_eq!(result.privacy.allowactivities.len(), 3);
    }

    #[test]
    fn two_product_contexts_and_replacement_documents_have_no_ambient_state() {
        let first_product = fake_product();
        let mut second_product = fake_product();
        second_product.targeting_prefix = "fake_second_product".into();
        second_product.cookie_sync.max_limit = Some(31);
        second_product.privacy.ipv6.anon_keep_bits = 80;
        let document_policy = policy(json!({
            "schema_version": 1_u64,
            "account_defaults": {"debug_allow": false},
            "accounts": {"fake_account": {"disabled": false}}
        }));
        let selection = AccountSelection::Named("fake_account".into());
        let first = resolve_policy_account(&first_product, &document_policy, &selection);
        let second = resolve_policy_account(&second_product, &document_policy, &selection);
        let first_again = resolve_policy_account(&first_product, &document_policy, &selection);
        for result in [&first, &first_again] {
            assert!(!result.disabled);
            assert!(!result.debug_allow);
            assert_eq!(result.targeting_prefix, "fake_product_prefix");
            assert_eq!(result.cookie_sync.max_limit, Some(9));
            assert_eq!(result.privacy.ipv6.anon_keep_bits, 56);
        }
        assert!(!second.disabled);
        assert!(!second.debug_allow);
        assert_eq!(second.targeting_prefix, "fake_second_product");
        assert_eq!(second.cookie_sync.max_limit, Some(31));
        assert_eq!(second.privacy.ipv6.anon_keep_bits, 80);
        let replacement = policy(json!({"schema_version": 1_u64}));
        let removed = resolve_policy_account(&first_product, &replacement, &selection);
        assert!(removed.disabled);
        assert!(removed.debug_allow);
        assert_eq!(removed.targeting_prefix, "fake_product_prefix");
    }
}
