//! Every commented example in `auctioneer.toml` must be valid config when
//! an operator uncomments it verbatim.
//!
//! The reference file is the only place a deployer learns these keys
//! exist, and an example that does not parse is worse than no example:
//! a rejected blob currently degrades to the baked default, which ships
//! `[baked.host.gdpr.tcf2] enabled = false`. A typo in a comment can
//! therefore turn GDPR enforcement off. This gate makes that
//! unrepresentable.
//!
//! It also pins a convention the extraction below depends on: a prose
//! comment in `auctioneer.toml` must not begin with `<ident> =`, because
//! that is indistinguishable from an uncommentable assignment.
//!
//! `auctioneer.toml` now carries THREE namespaces in one document —
//! `[baked.host]`, `[baked.account]` and `[runtime]` — so the whole
//! file no longer parses as `AuctioneerAppConfig`. The checks are split to
//! match: one case projects `[runtime]` and validates it exactly as
//! this file always has, and a second validates the `[baked.*]`
//! examples against the shapes the build script compiles them through.

#[cfg(test)]
mod tests {
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

    /// The whole document with every example enabled, parsed as the
    /// generic root. Both namespaces are opaque here; the two cases
    /// below each type the half they are about.
    fn uncommented_root() -> toml::Table {
        let document = uncommented(REFERENCE);
        toml::from_str(&document).unwrap_or_else(|err| {
            panic!("the uncommented reference document must parse as TOML: {err}\n---\n{document}")
        })
    }

    /// Pull one namespace out of the uncommented document.
    fn subtree(root: &toml::Table, path: &[&str]) -> toml::Value {
        let mut cursor = toml::Value::Table(root.clone());
        for segment in path {
            cursor = cursor
                .get(segment)
                .unwrap_or_else(|| panic!("`{}` must exist in auctioneer.toml", path.join(".")))
                .clone();
        }
        cursor
    }

    #[test]
    fn every_commented_runtime_example_in_auctioneer_toml_is_valid_config() {
        let root = uncommented_root();
        let runtime = subtree(&root, &["runtime"]);
        let rendered = toml::to_string(&runtime).expect("the runtime subtree re-serialises");

        // Guard the extraction itself: a rule that dropped everything
        // would leave an empty document, and an empty document parses.
        for required in [
            "[gdpr.tcf2]",
            "[gdpr.tcf2.purposes.2]",
            "[gdpr.tcf2.special_features.1]",
            "[aliases]",
            "[accounts.acct-42]",
            "[accounts.acct-42.gdpr.channel_enabled]",
            "[accounts.acct-42.privacy.ipv4]",
            "[ccpa]",
            "[lmt]",
            "[accounts.acct-42.ccpa.channel_enabled]",
            "[accounts.acct-42.privacy.allowactivities.transmitUfpd]",
            "[[accounts.acct-42.privacy.allowactivities.transmitUfpd.rules]]",
        ] {
            assert!(
                rendered.contains(required),
                "the extraction dropped `{required}` from `[runtime]`, so this test \
                 would prove nothing. Got:\n{rendered}",
            );
        }

        let cfg: AuctioneerAppConfig = runtime.clone().try_into().unwrap_or_else(|err| {
            panic!("every commented `[runtime]` example must parse when uncommented: {err}")
        });
        cfg.validate().unwrap_or_else(|err| {
            panic!("every commented example must also pass validation: {err}")
        });

        assert_eq!(cfg.ccpa.as_ref().and_then(|ccpa| ccpa.enforce), Some(true));
        assert_eq!(cfg.lmt.as_ref().and_then(|lmt| lmt.enforce), Some(true));

        // The two outbound overrides are the newest `[auction]` keys
        // and the pair `deny_unknown_fields` rejected until they were
        // added, so their presence in the uncommented document is what
        // proves the reference file documents the surface rather than
        // just the code carrying it.
        assert_eq!(
            (
                cfg.auction.max_outbound_raw_bytes,
                cfg.auction.max_outbound_request_bytes,
            ),
            (Some(0x0004_0000), Some(0x0004_0000)),
            "`auctioneer.toml` must document both outbound overrides with their baked \
             defaults; an undocumented key is a surface no deployer can find",
        );

        // Spot-check that the parsed values are the documented ones,
        // not silently-ignored unknown keys — `deny_unknown_fields`
        // already rejects those, so this pins the shape instead.
        assert_eq!(
            cfg.gdpr
                .as_ref()
                .and_then(|gdpr| gdpr.tcf2.as_ref())
                .and_then(|tcf2| tcf2.enabled),
            Some(true),
        );
        assert!(
            cfg.accounts["acct-42"]
                .gdpr
                .as_ref()
                .and_then(|gdpr| gdpr.channel_enabled.as_ref())
                .is_some_and(|channels| channels.web.is_some()),
            "the channel_enabled example must yield a parsed `web` tri-state",
        );
    }

    /// The `[baked.*]` half. Those tables are read by
    /// `auctioneer-config-data`'s build script, which compiles them into
    /// `host_default()` / `account_default()` — so the shipped file's
    /// baked namespaces are correct exactly when this build's baked
    /// constants match them field for field. That is a stronger check
    /// than re-parsing the tables here: it proves the build actually
    /// read THESE values.
    #[test]
    fn the_baked_examples_in_auctioneer_toml_are_what_the_build_compiled() {
        let root = uncommented_root();
        let host = subtree(&root, &["baked", "host"]);
        let account = subtree(&root, &["baked", "account"]);

        assert_eq!(
            host.get("status_response").and_then(toml::Value::as_str),
            Some(host_default().status_response.as_str()),
            "`[baked.host] status_response` must be the value this build baked",
        );
        let limits = host
            .get("resource_limits")
            .expect("`[baked.host.resource_limits]` must exist");
        assert_eq!(
            limits
                .get("default_tmax_ms")
                .and_then(toml::Value::as_integer),
            Some(host_default().resource_limits.default_tmax_ms),
            "`[baked.host.resource_limits] default_tmax_ms` must be the baked value",
        );
        assert_eq!(
            limits.get("max_tmax_ms").and_then(toml::Value::as_integer),
            Some(host_default().resource_limits.max_tmax_ms),
        );
        assert_eq!(
            host.get("gdpr")
                .and_then(|gdpr| gdpr.get("enabled"))
                .and_then(toml::Value::as_bool),
            Some(host_default().gdpr.enabled),
            "host `gdpr.enabled` is a bool here and a tri-state STRING under \
             `[baked.account]`; the namespaces are what keep them apart",
        );

        assert_eq!(
            account.get("bid_rounding").and_then(toml::Value::as_str),
            Some(account_default().bid_rounding.as_str()),
        );
        assert_eq!(
            account
                .get("targeting_prefix")
                .and_then(toml::Value::as_str),
            Some(account_default().targeting_prefix.as_str()),
        );
        assert_eq!(
            account
                .get("gdpr")
                .and_then(|gdpr| gdpr.get("enabled"))
                .and_then(toml::Value::as_str),
            Some("unset"),
            "account `gdpr.enabled` is the tri-state string, not a bool",
        );
        assert_eq!(
            account
                .get("privacy")
                .and_then(|privacy| privacy.get("ipv6"))
                .and_then(|ipv6| ipv6.get("anon_keep_bits"))
                .and_then(toml::Value::as_integer),
            Some(i64::from(account_default().privacy.ipv6.anon_keep_bits)),
        );
    }

    /// The overlay semantic the whole fold rests on: a pushed blob
    /// setting ONE key still inherits every other value from the baked
    /// layer. Making the blob total would break that — a deploy with no
    /// blob would then have no `gdpr.enabled` at all.
    #[test]
    fn partial_overlay_still_falls_through_to_baked() {
        let overlay: AuctioneerAppConfig = toml::from_str("[auction]\ndefault_tmax_ms = 1500\n")
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
