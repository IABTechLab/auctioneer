//! Every commented operator example is a direct, versioned runtime policy.
//! Prose comments must not begin with `<ident> =`: the extraction below
//! deliberately treats that spelling as an uncommentable assignment.

#[cfg(test)]
mod tests {
    use auctioneer_config_data::types::TriState;
    use auctioneer_config_data::{account_default, host_default};
    use auctioneer_core::config::AuctioneerAppConfig;
    use auctioneer_core::config::resolve::{resolve_host, resolved_account};
    use validator::Validate as _;

    const REFERENCE: &str = include_str!("../../../auctioneer.toml");

    /// `<ident> = ...` at the very start of the comment body — the shape
    /// an operator uncomments. Prose is rejected because its key half
    /// carries spaces, punctuation, or backticks.
    fn looks_like_assignment(body: &str) -> bool {
        let Some((raw_key, _value)) = body.split_once('=') else {
            return false;
        };
        let key = raw_key.trim_end();
        !key.is_empty()
            && key.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-')
    }

    /// Uncomment every line that is an example rather than prose: a
    /// `[table.header]` on its own, or an assignment. Live (already
    /// uncommented) lines pass through, so the result is the document a
    /// deployer would hold after enabling every documented knob.
    fn uncommented(src: &str) -> String {
        let mut out = String::new();
        for raw in src.lines() {
            let Some(commented) = raw.trim().strip_prefix('#') else {
                out.push_str(raw);
                out.push('\n');
                continue;
            };
            let body = commented.trim();
            let is_table = body.starts_with('[') && body.ends_with(']');
            if is_table || looks_like_assignment(body) {
                out.push_str(body);
                out.push('\n');
            }
        }
        out
    }

    /// The whole direct policy with every example enabled.
    fn uncommented_root() -> toml::Table {
        let document = uncommented(REFERENCE);
        toml::from_str(&document).unwrap_or_else(|err| {
            panic!("the uncommented reference document must parse as TOML: {err}\n---\n{document}")
        })
    }

    #[test]
    fn every_commented_runtime_example_in_auctioneer_toml_is_valid_config() {
        let root = uncommented_root();
        assert_eq!(
            root.get("schema_version").and_then(toml::Value::as_integer),
            Some(1)
        );
        for forbidden in ["baked", "startup", "runtime"] {
            assert!(
                !root.contains_key(forbidden),
                "operator policy must be direct"
            );
        }
        let rendered = toml::to_string(&root).expect("the direct policy re-serialises");

        // Guard the extraction itself: a rule that dropped everything
        // would leave an empty document, and an empty document parses.
        for required in [
            "[host.gdpr.tcf2]",
            "[host.gdpr.tcf2.purposes.2]",
            "[host.gdpr.tcf2.special_features.1]",
            "[host.aliases]",
            "[accounts.fake_publisher]",
            "[accounts.fake_publisher.gdpr.channel_enabled]",
            "[accounts.fake_publisher.privacy.ipv4]",
            "[host.ccpa]",
            "[host.lmt]",
            "[accounts.fake_publisher.ccpa.channel_enabled]",
            "[accounts.fake_publisher.privacy.allowactivities.transmitUfpd]",
            "[[accounts.fake_publisher.privacy.allowactivities.transmitUfpd.rules]]",
        ] {
            assert!(
                rendered.contains(required),
                "the extraction dropped `{required}` from the direct policy, so this test \
                 would prove nothing. Got:\n{rendered}",
            );
        }

        let cfg: AuctioneerAppConfig = root.try_into().unwrap_or_else(|err| {
            panic!("every commented policy example must parse when uncommented: {err}")
        });
        cfg.validate().unwrap_or_else(|err| {
            panic!("every commented example must also pass validation: {err}")
        });

        assert_eq!(
            cfg.host.ccpa.as_ref().and_then(|ccpa| ccpa.enforce),
            Some(true)
        );
        assert_eq!(
            cfg.host.lmt.as_ref().and_then(|lmt| lmt.enforce),
            Some(true)
        );

        // The two outbound overrides are the newest `[auction]` keys
        // and the pair `deny_unknown_fields` rejected until they were
        // added, so their presence in the uncommented document is what
        // proves the reference file documents the surface rather than
        // just the code carrying it.
        assert_eq!(
            (
                cfg.host.resource_limits.max_outbound_raw_bytes,
                cfg.host.resource_limits.max_outbound_request_bytes,
            ),
            (Some(0x0004_0000), Some(0x0004_0000)),
            "`auctioneer.toml` must document both outbound overrides with their baked \
             defaults; an undocumented key is a surface no deployer can find",
        );

        // Spot-check that the parsed values are the documented ones,
        // not silently-ignored unknown keys — `deny_unknown_fields`
        // already rejects those, so this pins the shape instead.
        assert_eq!(
            cfg.host
                .gdpr
                .as_ref()
                .and_then(|gdpr| gdpr.tcf2.as_ref())
                .and_then(|tcf2| tcf2.enabled),
            Some(true),
        );
        assert!(
            cfg.accounts["fake_publisher"]
                .gdpr
                .as_ref()
                .and_then(|gdpr| gdpr.channel_enabled.as_ref())
                .is_some_and(|channels| channels.web.is_some()),
            "the channel_enabled example must yield a parsed `web` tri-state",
        );
    }

    #[test]
    fn operator_examples_resolve_without_changing_neutral_product_defaults() {
        let root = uncommented_root();
        let cfg: AuctioneerAppConfig = root.try_into().expect("direct reference policy");
        let product_host = host_default();
        let product_account = account_default();
        assert!(product_host.user_sync.is_none());
        assert!(product_host.aliases.is_empty());
        assert_eq!(product_host.status_response, "");
        assert!(!product_host.ccpa.enforce);
        assert!(!product_host.lmt.enforce);
        assert_eq!(product_account.targeting_prefix, "hb");
        assert_eq!(product_account.privacy.ipv6.anon_keep_bits, 56);
        let host = resolve_host(&cfg, product_host);
        assert!(host.ccpa.enforce);
        assert!(host.lmt.enforce);
        assert!(host.gdpr.enabled);
        assert_eq!(host.resource_limits.max_tmax_ms, 2000);
        assert_eq!(host.resource_limits.default_tmax_ms, 1000);
        let account = resolved_account(&cfg, product_account);
        assert_eq!(account.bid_rounding, "down");
        assert_eq!(account.targeting_prefix, "hb");
        assert_eq!(account.gdpr.enabled, TriState::Unset);
        assert_eq!(account.privacy.ipv6.anon_keep_bits, 56);
        assert_eq!(
            serde_json::to_value(product_host).unwrap(),
            serde_json::to_value(auctioneer_config_data::product_host_default()).unwrap()
        );
        assert_eq!(
            serde_json::to_value(product_account).unwrap(),
            serde_json::to_value(auctioneer_config_data::product_account_default()).unwrap()
        );
    }

    /// The overlay semantic the whole fold rests on: a pushed blob
    /// setting ONE key still inherits every other value from the baked
    /// layer. Making the blob total would break that — a deploy with no
    /// blob would then have no `gdpr.enabled` at all.
    #[test]
    fn partial_overlay_still_falls_through_to_baked() {
        let overlay: AuctioneerAppConfig =
            toml::from_str("schema_version = 1\n[host.resource_limits]\ndefault_tmax_ms = 1500\n")
                .expect("a one-key overlay is valid config");

        let resolved = resolve_host(&overlay, host_default());
        assert_eq!(
            resolved.resource_limits.default_tmax_ms, 1500,
            "the one key the blob sets must win",
        );
        assert_eq!(
            resolved.resource_limits.max_tmax_ms,
            host_default().resource_limits.max_tmax_ms,
            "every key the blob omits must still come from the baked layer",
        );
        assert_eq!(
            resolved.gdpr.enabled,
            host_default().gdpr.enabled,
            "a partial blob must not blank out the baked gdpr block",
        );
        assert_eq!(
            resolved.host_cookie.ttl_days,
            host_default().host_cookie.ttl_days,
            "`host_cookie` has no overlay field at all and passes through whole",
        );

        let account = resolved_account(&overlay, account_default());
        assert_eq!(
            account.bid_rounding,
            account_default().bid_rounding,
            "an overlay that names no account key resolves to the baked default",
        );
        assert_eq!(account.targeting_prefix, account_default().targeting_prefix,);
    }
}
