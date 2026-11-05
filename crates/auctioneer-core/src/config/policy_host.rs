//! Explicit-context runtime host policy composition.

use super::{
    policy::{HostPolicy, OperatorPolicy},
    policy_account::apply_optional_override,
    resolve::{apply_host_gdpr_overlay, apply_resource_overrides, clamp_host_to_boot},
};
use auctioneer_config::{
    override_value::FieldOverride,
    types::{CooperativeConfig, HostConfig, UserSyncConfig},
};
use auctioneer_runtime::ResolvedIsolateLimits;

/// Compose caller-supplied product policy, then apply caller-supplied capacity.
/// This does not validate, install configuration or consult global boot state.
#[must_use]
pub fn resolve_policy_host(
    product: &HostConfig,
    policy: &OperatorPolicy,
    funded: &ResolvedIsolateLimits,
) -> HostConfig {
    let mut host = requested_policy_host(product, &policy.host);
    clamp_host_to_boot(&mut host, funded);
    host
}

pub(super) fn requested_policy_host(product: &HostConfig, overlay: &HostPolicy) -> HostConfig {
    let mut host = product.clone();
    apply_resource_overrides(&mut host.resource_limits, &overlay.resource_limits);
    if let Some(value) = &overlay.status_response {
        host.status_response.clone_from(value);
    }
    if let Some(gdpr) = &overlay.gdpr {
        apply_host_gdpr_overlay(&mut host, gdpr);
    }
    if let Some(value) = overlay.ccpa.as_ref().and_then(|ccpa| ccpa.enforce) {
        host.ccpa.enforce = value;
    }
    if let Some(value) = overlay.lmt.as_ref().and_then(|lmt| lmt.enforce) {
        host.lmt.enforce = value;
    }
    if let Some(cookie) = &overlay.host_cookie {
        let target = &mut host.host_cookie;
        if let Some(value) = &cookie.cookie_name {
            target.cookie_name.clone_from(value);
        }
        if let Some(value) = &cookie.family {
            target.family.clone_from(value);
        }
        if let Some(value) = cookie.ttl_days {
            target.ttl_days = value;
        }
        if let Some(value) = cookie.max_cookie_size_bytes {
            target.max_cookie_size_bytes = value;
        }
        apply_optional_override(&mut target.domain, &cookie.domain);
        match &cookie.optout_cookie {
            FieldOverride::Inherit => {}
            FieldOverride::Clear => target.optout_cookie = None,
            FieldOverride::Value(optout) => {
                let optout_target = target.optout_cookie.get_or_insert_with(Default::default);
                if let Some(value) = &optout.name {
                    optout_target.name.clone_from(value);
                }
                if let Some(value) = &optout.value {
                    optout_target.value.clone_from(value);
                }
            }
        }
    }
    match &overlay.user_sync {
        FieldOverride::Inherit => {}
        FieldOverride::Clear => host.user_sync = None,
        FieldOverride::Value(sync) => {
            let target = host.user_sync.get_or_insert_with(|| UserSyncConfig {
                external_url: String::new(),
                redirect_url: None,
                cooperative: CooperativeConfig::default(),
                priority_groups: Vec::new(),
                allowed_origins: Vec::new(),
            });
            target.external_url.clone_from(&sync.external_url);
            apply_optional_override(&mut target.redirect_url, &sync.redirect_url);
            if let Some(value) = sync
                .cooperative
                .as_ref()
                .and_then(|row| row.enabled_by_default)
            {
                target.cooperative.enabled_by_default = value;
            }
            if let Some(value) = &sync.priority_groups {
                target.priority_groups.clone_from(value);
            }
            if let Some(value) = &sync.allowed_origins {
                target.allowed_origins.clone_from(value);
            }
        }
    }
    if let Some(aliases) = &overlay.aliases {
        for (alias, base) in aliases {
            host.aliases.insert(alias.clone(), base.clone());
        }
    }
    host
}

#[cfg(test)]
mod tests {
    use super::resolve_policy_host;
    use crate::config::policy::OperatorPolicy;
    use crate::test_support::boot_parity_fixture::profile_for_adapter;
    use auctioneer_config::types::{HostConfig, OptoutCookie, TriState};
    use auctioneer_runtime::ResolvedIsolateLimits;
    use serde_json::{Value, json};

    fn policy(value: Value) -> OperatorPolicy {
        serde_json::from_value(value).expect("fake policy")
    }

    fn product() -> HostConfig {
        let mut product = auctioneer_config_data::product_host_default().clone();
        product.status_response = "fake_product_status".into();
        product.host_cookie.domain = Some("product.example.test".into());
        product.host_cookie.optout_cookie = Some(OptoutCookie {
            name: "fake_optout".into(),
            value: "fake_value".into(),
        });
        product
            .aliases
            .insert("fake_parent".into(), "fake_base".into());
        product
    }

    fn funded() -> ResolvedIsolateLimits {
        let mut row = profile_for_adapter("unequal_explicit_caps", "axum").limits;
        row.max_bidders_per_auction = 23;
        row.max_outbound_requests = 37;
        row.max_outbound_raw_bytes = 0x8000;
        row.max_outbound_request_bytes = 0x0001_0000;
        row.max_transport_bytes = 0x0002_0000;
        row.max_decoded_bytes = 0x0004_0000;
        row.max_parsed_nodes = 1009;
        row
    }

    #[test]
    fn requested_host_fields_compose_without_truthiness_or_global_state() {
        let mut product = product();
        product.gdpr.enabled = true;
        product.gdpr.host_vendor_id = Some(7);
        product.ccpa.enforce = true;
        product.lmt.enforce = true;
        let policy = policy(json!({"schema_version": 1_u64, "host": {
            "status_response": "", "gdpr": {"enabled": false, "host_vendor_id": 0_u32},
            "ccpa": {"enforce": false}, "lmt": {"enforce": false},
            "resource_limits": {
                "default_tmax_ms": 17_i64, "max_tmax_ms": 31_i64,
                "max_bidders_per_auction": 3_usize, "max_outbound_requests": 5_usize,
                "max_bidder_response_bytes": 1024_usize,
                "max_outbound_raw_bytes": 2048_u64, "max_outbound_request_bytes": 4096_u64,
                "max_transport_bytes": 8192_u64, "max_decoded_bytes": 0x4000_u64,
                "max_parsed_nodes": 19_u32
            }
        }}));
        let result = resolve_policy_host(&product, &policy, &funded());
        assert_eq!(result.status_response, "");
        assert!(!result.gdpr.enabled);
        assert_eq!(result.gdpr.host_vendor_id, Some(0));
        assert!(!result.ccpa.enforce);
        assert!(!result.lmt.enforce);
        assert_eq!(
            serde_json::to_value(&result.resource_limits).unwrap(),
            json!({
                "default_tmax_ms": 17_i64, "max_tmax_ms": 31_i64,
                "max_bidders_per_auction": 3_usize, "max_outbound_requests": 5_usize,
                "max_bidder_response_bytes": 1024_usize,
                "max_outbound_raw_bytes": 2048_u64, "max_outbound_request_bytes": 4096_u64,
                "max_transport_bytes": 8192_u64, "max_decoded_bytes": 0x4000_u64,
                "max_parsed_nodes": 19_u32
            })
        );
        assert_eq!(product.status_response, "fake_product_status");
        assert!(product.gdpr.enabled);
    }

    #[test]
    fn each_host_cap_uses_its_own_explicit_funded_limit() {
        let policy = policy(
            json!({"schema_version": 1_u64, "host": {"resource_limits": {
                "max_bidders_per_auction": 40_usize, "max_outbound_requests": 50_usize,
                "max_bidder_response_bytes": 0x0008_0000_usize,
                "max_outbound_raw_bytes": 0x0008_0000_u64, "max_outbound_request_bytes": 0x0008_0000_u64,
                "max_transport_bytes": 0x0008_0000_u64, "max_decoded_bytes": 0x0008_0000_u64,
                "max_parsed_nodes": 2000_u32
            }}}),
        );
        let first = resolve_policy_host(&product(), &policy, &funded());
        let mut other = funded();
        other.max_outbound_requests = 11;
        other.max_transport_bytes = 4096;
        let second = resolve_policy_host(&product(), &policy, &other);
        let again = resolve_policy_host(&product(), &policy, &funded());
        for result in [first, again] {
            let row = result.resource_limits;
            assert_eq!(row.max_bidders_per_auction, 23);
            assert_eq!(row.max_outbound_requests, 37);
            assert_eq!(row.max_outbound_raw_bytes, 0x8000);
            assert_eq!(row.max_outbound_request_bytes, 0x0001_0000);
            assert_eq!(row.max_transport_bytes, 0x0002_0000);
            assert_eq!(row.max_decoded_bytes, 0x0004_0000);
            assert_eq!(row.max_bidder_response_bytes, 0x0004_0000);
            assert_eq!(row.max_parsed_nodes, 1009);
        }
        assert_eq!(second.resource_limits.max_bidders_per_auction, 23);
        assert_eq!(second.resource_limits.max_outbound_requests, 11);
        assert_eq!(second.resource_limits.max_transport_bytes, 4096);
        assert_eq!(second.resource_limits.max_decoded_bytes, 0x0004_0000);
    }

    #[test]
    fn nullable_cookie_fields_clear_nonempty_product_values_independently() {
        for (field, retained) in [("domain", "optout_cookie"), ("optout_cookie", "domain")] {
            let product = product();
            let before = serde_json::to_value(&product.host_cookie).unwrap();
            assert!(!before[field].is_null());
            assert!(!before[retained].is_null());
            let policy = policy(json!({"schema_version": 1_u64, "host": {
                "host_cookie": {field: {"clear": true}}
            }}));
            let result = resolve_policy_host(&product, &policy, &funded());
            let cookie = serde_json::to_value(&result.host_cookie).unwrap();
            assert!(cookie[field].is_null());
            assert_eq!(cookie[retained], before[retained]);
            assert_eq!(cookie["cookie_name"], before["cookie_name"]);
        }
    }

    #[test]
    fn cookie_leaf_values_merge_and_keep_explicit_empty_values() {
        let policy = policy(json!({"schema_version": 1_u64, "host": {"host_cookie": {
            "cookie_name": "fake_changed", "family": "fake_family", "domain": "",
            "ttl_days": 2_i64, "max_cookie_size_bytes": 3072_usize,
            "optout_cookie": {"value": ""}
        }}}));
        let result = resolve_policy_host(&product(), &policy, &funded());
        assert_eq!(
            serde_json::to_value(&result.host_cookie).unwrap(),
            json!({
                "cookie_name": "fake_changed", "family": "fake_family", "domain": "",
                "ttl_days": 2_i64, "max_cookie_size_bytes": 3072_usize,
                "optout_cookie": {"name": "fake_optout", "value": ""}
            })
        );
    }

    #[test]
    fn sync_service_has_no_invented_origin_and_supports_explicit_removal() {
        let product = product();
        assert!(product.user_sync.is_none());
        let omitted = resolve_policy_host(&product, &OperatorPolicy::default(), &funded());
        assert!(omitted.user_sync.is_none());
        let configured_policy = policy(json!({"schema_version": 1_u64, "host": {"user_sync": {
            "external_url": "https://sync.example.test", "redirect_url": "https://sync.example.test/callback",
            "cooperative": {"enabled_by_default": false},
            "priority_groups": [["fake_b", "fake_a"]], "allowed_origins": ["https://page.example.test"]
        }}}));
        let configured = resolve_policy_host(&product, &configured_policy, &funded());
        let sync = configured.user_sync.as_ref().unwrap();
        assert_eq!(sync.external_url, "https://sync.example.test");
        assert_eq!(
            sync.redirect_url.as_deref(),
            Some("https://sync.example.test/callback")
        );
        assert!(!sync.cooperative.enabled_by_default);
        assert_eq!(sync.priority_groups, [vec!["fake_b", "fake_a"]]);
        assert_eq!(sync.allowed_origins, ["https://page.example.test"]);
        let cleared =
            policy(json!({"schema_version": 1_u64, "host": {"user_sync": {"clear": true}}}));
        assert!(
            resolve_policy_host(&configured, &cleared, &funded())
                .user_sync
                .is_none()
        );
        let clear_redirect = policy(json!({"schema_version": 1_u64, "host": {"user_sync": {
            "external_url": "https://other.example.test", "redirect_url": {"clear": true},
            "priority_groups": [], "allowed_origins": []
        }}}));
        let updated = resolve_policy_host(&configured, &clear_redirect, &funded());
        let updated_sync = updated.user_sync.unwrap();
        assert_eq!(updated_sync.external_url, "https://other.example.test");
        assert_eq!(updated_sync.redirect_url, None);
        assert!(updated_sync.priority_groups.is_empty());
        assert!(updated_sync.allowed_origins.is_empty());
    }

    #[test]
    fn aliases_and_gdpr_maps_merge_by_key_while_arrays_replace() {
        let mut product = product();
        product.gdpr.non_standard_publishers = vec!["fake_old".into()];
        product
            .gdpr
            .tcf2
            .purposes
            .entry(1)
            .or_default()
            .vendor_exceptions = vec!["fake_old".into()];
        product
            .gdpr
            .tcf2
            .purposes
            .entry(2)
            .or_default()
            .vendor_exceptions = vec!["fake_sibling".into()];
        let policy = policy(json!({"schema_version": 1_u64, "host": {
            "aliases": {"fake_new": "fake_base", "fake_parent": "fake_other"},
            "gdpr": {"non_standard_publishers": [], "tcf2": {
                "enabled": false, "purposes": {"1": {"vendor_exceptions": []}},
                "special_features": {"1": {"enforce": "disabled"}}
            }}
        }}));
        let host = resolve_policy_host(&product, &policy, &funded());
        assert_eq!(host.aliases.len(), 2);
        assert_eq!(host.aliases["fake_parent"], "fake_other");
        assert_eq!(host.aliases["fake_new"], "fake_base");
        assert!(host.gdpr.non_standard_publishers.is_empty());
        assert!(host.gdpr.tcf2.purposes[&1].vendor_exceptions.is_empty());
        assert_eq!(
            host.gdpr.tcf2.purposes[&2].vendor_exceptions,
            ["fake_sibling"]
        );
        assert_eq!(
            host.gdpr.tcf2.special_features[&1].enforce,
            TriState::Disabled
        );
        let removed = resolve_policy_host(&product, &OperatorPolicy::default(), &funded());
        assert_eq!(removed.aliases.len(), 1);
        assert!(!removed.aliases.contains_key("fake_new"));
        assert_eq!(removed.gdpr.non_standard_publishers, ["fake_old"]);
    }
}
