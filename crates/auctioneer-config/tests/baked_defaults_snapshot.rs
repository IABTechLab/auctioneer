//! Snapshot of the BAKED defaults, recorded from the two
//! `data/` files that shipped before they were folded into
//! `auctioneer.toml`.
//!
//! Moving defaults between files is the kind of change that can lose a
//! value without any test noticing: every consumer reads
//! `host_default()` / `account_default()` and would happily serve a
//! silently-changed default. This pins the whole structure by value,
//! so a dropped key, a retyped field or a transposed number fails
//! here rather than in production privacy behaviour.
//!
//! These literals are the CONTRACT, not a convenience. Updating one
//! means deliberately changing a shipped default, and the diff has to
//! say so.

#[cfg(test)]
mod tests {
    use auctioneer_config::{account_default, host_default};

    /// Pre-existing defaults remain fixed; privacy enforcement adds outbound and funded
    /// response/parse caps, and user sync adds explicit sync policy.
    ///
    /// Raw strings, not `serde_json::json!`, here and for
    /// `ACCOUNT_SNAPSHOT`: both are compared with `assert_eq!` against
    /// `serde_json::to_string`, so the assertion is on the exact
    /// serialized bytes. A `Value` comparison would stop catching a
    /// retyped field that still compares equal as JSON, which is the
    /// drift this file exists to catch.
    const HOST_SNAPSHOT: &str = concat!(
        r#"{"gdpr":{"enabled":true,"default_value":"0","host_vendor_id":0,"#,
        r#""tcf2":{"enabled":false,"purpose_one_treatment":{"enabled":true,"access_allowed":true},"#,
        r#""purposes":{},"special_features":{}},"#,
        r#""non_standard_publishers":[]},"ccpa":{"enforce":false},"#,
        r#""lmt":{"enforce":false},"host_cookie":{"cookie_name":"auctioneer_uid","family":"auctioneer","#,
        r#""domain":null,"ttl_days":90,"max_cookie_size_bytes":0,"optout_cookie":null},"#,
        r#""user_sync":{"external_url":"https://sync.example.test","redirect_url":null,"#,
        r#""cooperative":{"enabled_by_default":false},"priority_groups":[],"allowed_origins":[]},"#,
        r#""status_response":"","resource_limits":{"default_tmax_ms":1000,"#,
        r#""max_tmax_ms":2000,"max_bidder_response_bytes":262144,"#,
        r#""max_bidders_per_auction":50,"max_outbound_requests":100,"#,
        r#""max_outbound_raw_bytes":262144,"max_outbound_request_bytes":262144,"#,
        r#""max_transport_bytes":262144,"max_decoded_bytes":262144,"#,
        r#""max_parsed_nodes":65536},"aliases":{}}"#,
    );

    /// Pre-existing account defaults remain fixed; user sync adds sync policy.
    const ACCOUNT_SNAPSHOT: &str = concat!(
        r#"{"disabled":false,"debug_allow":false,"bid_rounding":"down","#,
        r#""targeting_prefix":"hb","truncate_target_attribute":null,"#,
        r#""gdpr":{"enabled":"unset","channel_enabled":{"web":"unset","#,
        r#""app":"unset","amp":"unset","video":"unset","dooh":"unset"},"#,
        r#""purposes":{},"special_features":{},"basic_enforcement_vendors":[],"#,
        r#""purpose_one_treatment":{"enabled":null,"access_allowed":null}},"#,
        r#""cookie_sync":{"default_limit":null,"max_limit":null,"default_coop_sync":null,"priority_groups":null},"#,
        r#""ccpa":{"enabled":"unset","channel_enabled":{"web":"unset","#,
        r#""app":"unset","amp":"unset","video":"unset","dooh":"unset"}},"#,
        r#""privacy":{"ipv4":{"anon_keep_bits":24},"ipv6":{"anon_keep_bits":56},"allowactivities":{}}}"#,
    );

    #[test]
    fn baked_defaults_match_the_pre_fold_values() {
        let host = serde_json::to_string(host_default()).expect("host defaults serialise");
        assert_eq!(
            host, HOST_SNAPSHOT,
            "the baked host defaults changed without updating the intentional default contract"
        );

        let account = serde_json::to_string(account_default()).expect("account defaults serialise");
        assert_eq!(
            account, ACCOUNT_SNAPSHOT,
            "the baked account defaults changed without updating the intentional default contract"
        );
    }

    /// The keys that motivated the fold: `[runtime.auction]` overrides
    /// these, and before the fold the default and its override lived in
    /// different files under different section names, free to disagree.
    #[test]
    fn the_duplicated_auction_limits_keep_their_shipped_values() {
        let limits = &host_default().resource_limits;
        assert_eq!(limits.default_tmax_ms, 1000);
        assert_eq!(limits.max_tmax_ms, 2000);
        // 256 KiB. Spelled in hex because the workspace's clippy
        // gate prefers it for round powers of two.
        assert_eq!(limits.max_bidder_response_bytes, 0x0004_0000);
        assert_eq!(limits.max_bidders_per_auction, 50);
        assert_eq!(limits.max_outbound_requests, 100);
        // The five byte/node caps each adapter's boot envelope also
        // declares. These are the figures every shipped envelope funds,
        // so the baked row and the envelope agree and the resolver's
        // clamp is a no-op at the default. Lowering one here would
        // change what every deployment charges, which is a value change
        // and not a surface change, pinned so it cannot happen by
        // accident.
        assert_eq!(limits.max_outbound_raw_bytes, 0x0004_0000);
        assert_eq!(limits.max_outbound_request_bytes, 0x0004_0000);
        assert_eq!(limits.max_transport_bytes, 0x0004_0000);
        assert_eq!(limits.max_decoded_bytes, 0x0004_0000);
        assert_eq!(limits.max_parsed_nodes, 0x0001_0000);
    }
}
