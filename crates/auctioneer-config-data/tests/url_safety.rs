//! Codegen-time URL safety tests for `auctioneer-config-data`'s build.rs.
//!
//! These tests use the compiled config-schema validators. The cross-layer
//! generation/render contract lives with the Core renderer's integration tests.
//!
//! Coverage:
//! - default build rejects `http://` (any host)
//! - default build rejects `https://` to loopback (any host in
//!   `127/8` + `::1` + `localhost`)
//! - default build rejects private-IP literal authority
//! - insecure build accepts `http://` ONLY to the three exact
//!   literals `127.0.0.1`, `::1`, `localhost`
//! - insecure build rejects `http://` to `127.0.0.2` (range-only
//!   would accept; literal-only required)
//! - insecure build still rejects private-IP literals
//! - syncer template rejects authority macros (e.g. `{{.Host}}`)

use auctioneer_config::url_safety as build_support;

#[cfg(test)]
mod tests {
    use super::build_support::{
        EndpointAuthorityKind, ValidationResult, classify_endpoint_authority,
        extract_endpoint_macros, is_exact_loopback_literal, is_loopback_range,
        validate_endpoint_macros, validate_endpoint_url, validate_syncer_template,
    };
    use std::slice::from_ref;

    // ----- default build (allow_insecure = false) -----

    #[test]
    fn default_build_accepts_https_public() {
        let result = validate_endpoint_url("https://bidder.example.test/openrtb2", false);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn endpoint_and_syncer_reject_root_dotted_loopback_names() {
        for host in [
            "localhost",
            "sub.localhost",
            "nip.io",
            "xip.io",
            "lvh.me",
            "localtest.me",
            "vcap.me",
            "127.0.0.1.nip.io",
            "app.xip.io",
            "app.lvh.me",
            "app.localtest.me",
            "app.vcap.me",
        ] {
            for allow_insecure in [false, true] {
                for scheme in ["http", "https"] {
                    let url = format!("{scheme}://{host}.:8443/sync?back={{{{.RedirectURL}}}}");
                    assert_ne!(
                        validate_endpoint_url(&url, allow_insecure),
                        ValidationResult::Ok,
                        "{url}, insecure={allow_insecure}"
                    );
                    assert_ne!(
                        validate_syncer_template(&url, allow_insecure),
                        ValidationResult::Ok,
                        "{url}, insecure={allow_insecure}"
                    );
                }
            }
        }
    }

    #[test]
    fn root_dot_policy_preserves_public_dns_and_normalized_ip_controls() {
        for allow_insecure in [false, true] {
            for host in [
                "sync.example.com.",
                "snip.io.",
                "nip.io.example.",
                "8.8.8.8",
                "8.8.8.8.",
                "0x08080808",
                "[2606:4700:4700::1111]",
            ] {
                let url = format!("https://{host}:8443/base%20path/sync?back={{{{.RedirectURL}}}}");
                assert_eq!(
                    validate_endpoint_url(&url, allow_insecure),
                    ValidationResult::Ok,
                    "{url}"
                );
                assert_eq!(
                    validate_syncer_template(&url, allow_insecure),
                    ValidationResult::Ok,
                    "{url}"
                );
            }
            for host in [
                "127.0.0.2.",
                "0x7f000002",
                "10.0.0.1.",
                "0x0a000001",
                "192.168.1.1.",
                "100.64.0.1.",
                "[fc00::1]",
                "[::ffff:8.8.8.8]",
            ] {
                let url = format!("https://{host}/sync?back={{{{.RedirectURL}}}}");
                assert_ne!(
                    validate_endpoint_url(&url, allow_insecure),
                    ValidationResult::Ok,
                    "{url}"
                );
                assert_ne!(
                    validate_syncer_template(&url, allow_insecure),
                    ValidationResult::Ok,
                    "{url}"
                );
            }
        }
    }

    #[test]
    fn syncer_generation_rejects_parser_repaired_url_spellings() {
        for allow_insecure in [false, true] {
            for base in [
                "https:///bidder.example.com",
                "https:////bidder.example.com",
                "https://\\bidder.example.com",
                "https://bidder.example.com\\sync",
                "https://bidder.example.com/base\\sync",
                "https://bidder.example.com/base path",
                "https://bidder.example.com/base\u{00a0}path",
            ] {
                let url = format!("{base}/sync?back={{{{.RedirectURL}}}}");
                assert_eq!(
                    validate_syncer_template(&url, allow_insecure),
                    ValidationResult::MalformedUrl,
                    "{url:?}, insecure={allow_insecure}"
                );
            }
            for base in [
                "https://bidder.example.com",
                "HTTPS://bidder.example.com/base://kept",
                "https://bidder.example.com/base%20path",
                "https://bidder.example.com//base",
            ] {
                let url = format!("{base}/sync?back={{{{.RedirectURL}}}}");
                assert_eq!(
                    validate_syncer_template(&url, allow_insecure),
                    ValidationResult::Ok,
                    "{url:?}"
                );
            }
        }
    }

    #[test]
    fn bidder_sync_template_has_an_eight_kib_ceiling() {
        let allowed = format!("https://sync.example.test/{}", "a".repeat(8_192 - 26));
        assert_eq!(
            validate_syncer_template(&allowed, false),
            ValidationResult::Ok
        );
        let oversized = format!("https://sync.example.test/{}", "a".repeat(8_193));
        assert_ne!(
            validate_syncer_template(&oversized, false),
            ValidationResult::Ok
        );
    }

    #[test]
    fn default_build_rejects_http_public() {
        let result = validate_endpoint_url("http://bidder.example.test/openrtb2", false);
        assert_eq!(result, ValidationResult::HttpInDefaultBuild);
    }

    #[test]
    fn default_build_rejects_http_loopback() {
        let result = validate_endpoint_url("http://127.0.0.1:8787/auction", false);
        assert_eq!(result, ValidationResult::HttpInDefaultBuild);
    }

    #[test]
    fn default_build_rejects_https_loopback_literal() {
        let result = validate_endpoint_url("https://127.0.0.1/openrtb2", false);
        assert_eq!(result, ValidationResult::LoopbackInDefaultBuild);
    }

    #[test]
    fn default_build_rejects_https_loopback_range() {
        // 127.0.0.2 is still loopback per RFC 1122 but a prefix-only
        // check would miss it.
        let result = validate_endpoint_url("https://127.0.0.2/openrtb2", false);
        assert_eq!(result, ValidationResult::LoopbackInDefaultBuild);
    }

    #[test]
    fn default_build_rejects_https_localhost_hostname() {
        let result = validate_endpoint_url("https://localhost/openrtb2", false);
        assert_eq!(result, ValidationResult::LoopbackInDefaultBuild);
    }

    #[test]
    fn default_build_rejects_https_ipv6_loopback() {
        let result = validate_endpoint_url("https://[::1]/openrtb2", false);
        assert_eq!(result, ValidationResult::LoopbackInDefaultBuild);
    }

    #[test]
    fn default_build_rejects_private_rfc1918() {
        let result = validate_endpoint_url("https://10.0.0.1/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_cgnat_range() {
        let result = validate_endpoint_url("https://100.64.5.5/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_v6_unique_local() {
        let result = validate_endpoint_url("https://[fc00::1]/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_unsupported_scheme() {
        let result = validate_endpoint_url("ftp://example.com/openrtb2", false);
        assert_eq!(result, ValidationResult::UnsupportedScheme);
    }

    #[test]
    fn default_build_rejects_bare_scheme_no_host() {
        // `https://` with no host slips through prefix-only matching:
        // scheme check passes, then is_loopback_range/private_ip checks
        // return false because host_str is None — so the validator
        // would return Ok. This test pins it as MalformedUrl instead.
        let result = validate_endpoint_url("https://", false);
        assert_eq!(result, ValidationResult::MalformedUrl);
    }

    #[test]
    fn default_build_rejects_https_with_invalid_percent_encoding() {
        let result = validate_endpoint_url("https://%zz/openrtb2", false);
        assert_eq!(result, ValidationResult::MalformedUrl);
    }

    #[test]
    fn default_build_rejects_https_with_broken_ipv6_bracket() {
        let result = validate_endpoint_url("https://[::1/openrtb2", false);
        assert_eq!(result, ValidationResult::MalformedUrl);
    }

    #[test]
    fn default_build_rejects_empty_string() {
        // Empty input goes through the scheme check first (neither http nor
        // https prefix), so it surfaces as UnsupportedScheme, not Malformed —
        // either rejection is acceptable; pin the actual behavior so future
        // refactors don't quietly slip it through.
        let result = validate_endpoint_url("", false);
        assert_eq!(result, ValidationResult::UnsupportedScheme);
    }

    #[test]
    fn insecure_build_rejects_bare_scheme_no_host() {
        // The same Malformed-URL gate runs in insecure mode, before the
        // exact-literal check would otherwise read an empty host string.
        let result = validate_endpoint_url("http://", true);
        assert_eq!(result, ValidationResult::MalformedUrl);
    }

    // ----- insecure build (allow_insecure = true) -----

    #[test]
    fn insecure_build_accepts_https_public() {
        let result = validate_endpoint_url("https://bidder.example.test/openrtb2", true);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn insecure_build_accepts_http_loopback_literal() {
        let result = validate_endpoint_url("http://127.0.0.1:8787/auction", true);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn insecure_build_accepts_http_localhost_hostname() {
        let result = validate_endpoint_url("http://localhost:8787/auction", true);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn insecure_build_accepts_http_ipv6_loopback() {
        let result = validate_endpoint_url("http://[::1]:8787/auction", true);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn insecure_build_rejects_http_public() {
        let result = validate_endpoint_url("http://bidder.example.test/openrtb2", true);
        assert_eq!(result, ValidationResult::NonLocalhostHttpUnderInsecure);
    }

    #[test]
    fn insecure_build_rejects_http_non_literal_loopback() {
        // The relaxation is for the three exact literals (127.0.0.1,
        // ::1, localhost), not the whole 127.0.0.0/8 range. A dedicated
        // variant covers the loopback-but-not-allow-list case so the
        // message can name the allow-list verbatim.
        let result = validate_endpoint_url("http://127.0.0.2/auction", true);
        assert_eq!(result, ValidationResult::NonLiteralLoopbackUnderInsecure);
    }

    #[test]
    fn insecure_build_rejects_https_non_literal_loopback() {
        // If is_loopback_range were only consulted when !allow_insecure,
        // HTTPS to 127.0.0.2 would slip past under the feature. This
        // test pins the dedicated NonLiteralLoopbackUnderInsecure
        // variant regardless of scheme.
        let result = validate_endpoint_url("https://127.0.0.2/auction", true);
        assert_eq!(result, ValidationResult::NonLiteralLoopbackUnderInsecure);
    }

    #[test]
    fn insecure_build_rejects_https_loopback_range_high() {
        // Same rule as the .2 test, near the top of the /8 range — proves
        // it's a range check, not a single-address blacklist.
        let result = validate_endpoint_url("https://127.255.255.255/auction", true);
        assert_eq!(result, ValidationResult::NonLiteralLoopbackUnderInsecure);
    }

    #[test]
    fn insecure_build_rejects_http_to_private_rfc1918() {
        let result = validate_endpoint_url("http://10.0.0.1/auction", true);
        assert_eq!(result, ValidationResult::NonLocalhostHttpUnderInsecure);
    }

    #[test]
    fn insecure_build_rejects_https_to_private_rfc1918() {
        let result = validate_endpoint_url("https://192.168.1.1/auction", true);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    // ----- syncer template validation -----

    #[test]
    fn syncer_template_lexical_errors_are_rejected_before_rendering() {
        for url in [
            "https://bidder.example.com/}}/sync?back={{.RedirectURL}}",
            "https://bidder.example.com/sync?back={{.RedirectURL}}}}",
            "https://bidder.example.com/sync?x=}}",
            "https://bidder.example.com/sy\tnc?back={{.RedirectURL}}",
            "https://bidder.example.com/sync?back={{.RedirectURL}}\n",
        ] {
            assert_ne!(
                validate_syncer_template(url, false),
                ValidationResult::Ok,
                "{url:?}"
            );
        }
    }

    #[test]
    fn syncer_template_accepts_path_macro() {
        // `{{.RedirectURL}}` in the path/query is fine.
        let result =
            validate_syncer_template("https://bidder.example.test/getuid?{{.RedirectURL}}", false);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn syncer_template_accepts_privacy_macros() {
        // All five privacy macros are passthrough-allowed alongside
        // {{.RedirectURL}}.
        let url = "https://bidder.example.test/sync?gdpr={{.GDPR}}&consent={{.GDPRConsent}}\
               &usp={{.USPrivacy}}&gpp={{.GPP}}&sid={{.GPPSID}}&r={{.RedirectURL}}";
        let result = validate_syncer_template(url, false);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn syncer_template_rejects_userinfo_and_fragments() {
        for url in [
            "https://user:pass@bidder.example.test/sync?r={{.RedirectURL}}",
            "https://bidder.example.test/sync?r={{.RedirectURL}}#fragment",
        ] {
            assert_ne!(
                validate_syncer_template(url, false),
                ValidationResult::Ok,
                "{url}"
            );
        }
    }

    #[test]
    fn syncer_template_rejects_external_url_macro() {
        // {{.ExternalURL}} belongs in the redirect_url template, NOT
        // in a bidder syncer URL. Build-time error.
        let result = validate_syncer_template(
            "https://bidder.example.test/sync?from={{.ExternalURL}}&r={{.RedirectURL}}",
            false,
        );
        assert_eq!(
            result,
            ValidationResult::UnknownMacro("{{.ExternalURL}}".to_owned()),
        );
    }

    #[test]
    fn syncer_template_rejects_syncer_key_macro() {
        let result =
            validate_syncer_template("https://bidder.example.test/sync?key={{.SyncerKey}}", false);
        assert_eq!(
            result,
            ValidationResult::UnknownMacro("{{.SyncerKey}}".to_owned()),
        );
    }

    #[test]
    fn syncer_template_rejects_bidder_name_macro() {
        let result =
            validate_syncer_template("https://bidder.example.test/sync?b={{.BidderName}}", false);
        assert_eq!(
            result,
            ValidationResult::UnknownMacro("{{.BidderName}}".to_owned()),
        );
    }

    #[test]
    fn syncer_template_rejects_user_macro() {
        let result =
            validate_syncer_template("https://bidder.example.test/sync?u={{.UserMacro}}", false);
        assert_eq!(
            result,
            ValidationResult::UnknownMacro("{{.UserMacro}}".to_owned()),
        );
    }

    #[test]
    fn syncer_template_rejects_arbitrary_unknown_macro() {
        let result = validate_syncer_template(
            "https://bidder.example.test/sync?x={{.NotARealMacro}}",
            false,
        );
        assert_eq!(
            result,
            ValidationResult::UnknownMacro("{{.NotARealMacro}}".to_owned()),
        );
    }

    #[test]
    fn syncer_template_rejects_authority_macro() {
        // Authority-level macros are rejected — would let the runtime
        // swap the destination via a request-derived header.
        let result = validate_syncer_template("https://{{.Host}}/sync", false);
        assert_eq!(result, ValidationResult::AuthorityMacro);
    }

    #[test]
    fn syncer_template_rejects_http_in_default_build() {
        let result = validate_syncer_template("http://example.com/sync", false);
        assert_eq!(result, ValidationResult::HttpInDefaultBuild);
    }

    #[test]
    fn syncer_template_rejects_non_literal_loopback_under_insecure() {
        let result = validate_syncer_template("http://127.0.0.2/sync", true);
        assert_eq!(result, ValidationResult::NonLiteralLoopbackUnderInsecure);
    }

    #[test]
    fn syncer_template_rejects_unclosed_macro() {
        // A `{{.RedirectURL` without a closing `}}` must be reported
        // as UnclosedMacro so the build fails, otherwise the inner
        // `split_once("}}")` returns None and the scan exits as Ok.
        let result =
            validate_syncer_template("https://bidder.example.test/sync?r={{.RedirectURL", false);
        assert!(
            matches!(result, ValidationResult::UnclosedMacro(_)),
            "expected UnclosedMacro, got {result:?}",
        );
    }

    #[test]
    fn syncer_template_rejects_localhost_alias() {
        // 127.0.0.1.nip.io always resolves to loopback. Default builds
        // reject it as a localhost alias.
        let result = validate_syncer_template("https://127.0.0.1.nip.io/sync", false);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    // ----- localhost-alias helper coverage -----

    #[test]
    fn default_build_rejects_nip_io_alias() {
        let result = validate_endpoint_url("https://127.0.0.1.nip.io/openrtb2", false);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn default_build_rejects_xip_io_alias() {
        let result = validate_endpoint_url("https://app.10.0.0.1.xip.io/openrtb2", false);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn default_build_rejects_lvh_me_alias() {
        let result = validate_endpoint_url("https://my-app.lvh.me/openrtb2", false);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn default_build_rejects_localtest_me_alias() {
        let result = validate_endpoint_url("https://demo.localtest.me/openrtb2", false);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn default_build_rejects_dot_localhost_suffix() {
        // RFC 6761 reserves the .localhost TLD for loopback. *.localhost
        // always resolves to loopback.
        let result = validate_endpoint_url("https://api.localhost/openrtb2", false);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn insecure_build_rejects_nip_io_alias() {
        // The feature-on allow-list is exact-literal only, so aliases
        // stay rejected even under insecure.
        let result = validate_endpoint_url("https://127.0.0.1.nip.io/openrtb2", true);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn insecure_build_rejects_dot_localhost_suffix() {
        let result = validate_endpoint_url("http://api.localhost/openrtb2", true);
        assert_eq!(result, ValidationResult::LocalhostAlias);
    }

    #[test]
    fn default_build_accepts_non_alias_with_localhost_substring() {
        // Guard: a hostname that only CONTAINS "localhost" as a substring
        // (e.g. "fakelocalhost.example") is not a `.localhost` suffix and
        // should not be rejected.
        let result = validate_endpoint_url("https://fakelocalhost.example/openrtb2", false);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn default_build_accepts_non_alias_with_nip_substring() {
        // Guard: "nip.io" must match as a `.<apex>` suffix, not anywhere.
        // "snip.io" is not on the alias list.
        let result = validate_endpoint_url("https://snip.io/openrtb2", false);
        assert_eq!(result, ValidationResult::Ok);
    }

    #[test]
    fn syncer_template_rejects_bare_scheme() {
        // Same prefix-only slip-through hazard the bidder-endpoint test
        // covers, but for the syncer code path.
        let result = validate_syncer_template("https://", false);
        assert_eq!(result, ValidationResult::MalformedUrl);
    }

    #[test]
    fn syncer_template_rejects_malformed_template_url() {
        // Bracket-only IPv6 placeholder + bad escape — round-trip through
        // url::Url::parse fails.
        let result = validate_syncer_template("https://[::1/sync?{{.RedirectURL}}", false);
        assert_eq!(result, ValidationResult::MalformedUrl);
    }

    // ----- helper coverage -----

    #[test]
    fn is_loopback_range_catches_full_127_8() {
        assert!(is_loopback_range("https://127.0.0.1/"));
        assert!(is_loopback_range("https://127.0.0.2/"));
        assert!(is_loopback_range("https://127.255.255.255/"));
        assert!(is_loopback_range("https://localhost/"));
        assert!(is_loopback_range("https://LOCALHOST/"));
        assert!(is_loopback_range("https://[::1]/"));
        assert!(!is_loopback_range("https://1.2.3.4/"));
    }

    #[test]
    fn is_exact_loopback_literal_is_narrow() {
        assert!(is_exact_loopback_literal("https://127.0.0.1/"));
        assert!(is_exact_loopback_literal("https://[::1]/"));
        assert!(is_exact_loopback_literal("https://localhost/"));
        assert!(is_exact_loopback_literal("https://LOCALHOST/"));
        assert!(!is_exact_loopback_literal("https://127.0.0.2/"));
        assert!(!is_exact_loopback_literal("https://127.255.255.255/"));
        assert!(!is_exact_loopback_literal("https://1.2.3.4/"));
    }

    // ----- endpoint authority classifier -----

    #[test]
    fn classify_authority_fixed_for_literal_host() {
        assert_eq!(
            classify_endpoint_authority("https://hb.example.test/openrtb"),
            EndpointAuthorityKind::Fixed,
        );
    }

    #[test]
    fn classify_authority_fixed_for_literal_host_with_port() {
        assert_eq!(
            classify_endpoint_authority("https://hb.example.test:8443/openrtb"),
            EndpointAuthorityKind::Fixed,
        );
    }

    #[test]
    fn classify_authority_host_varying_for_exact_host_macro() {
        assert_eq!(
            classify_endpoint_authority("https://{{.Host}}/openrtb"),
            EndpointAuthorityKind::HostVarying,
        );
    }

    #[test]
    fn classify_authority_unsupported_for_region_host_macro() {
        // Reviewer-cited: {{.RegionHost}} is NOT the allow-listed token.
        let result = classify_endpoint_authority("https://{{.RegionHost}}/openrtb");
        assert_eq!(
            result,
            EndpointAuthorityKind::UnsupportedMacro("{{.RegionHost}}".to_owned()),
        );
    }

    #[test]
    fn classify_authority_unsupported_for_host_plus_port_macro() {
        // Reviewer-cited: {{.Host}}{{.Port}} adds a second authority macro
        // alongside the allow-listed one — rejected.
        let result = classify_endpoint_authority("https://{{.Host}}{{.Port}}/openrtb");
        assert_eq!(
            result,
            EndpointAuthorityKind::UnsupportedMacro("{{.Host}}{{.Port}}".to_owned()),
        );
    }

    #[test]
    fn classify_authority_unsupported_for_macro_with_port_suffix() {
        // The exact-match check rejects `{{.Host}}:443` — if you need a
        // port for a host-varying endpoint, fold it into the allowlist
        // entry.
        let result = classify_endpoint_authority("https://{{.Host}}:443/openrtb");
        assert_eq!(
            result,
            EndpointAuthorityKind::UnsupportedMacro("{{.Host}}:443".to_owned()),
        );
    }

    #[test]
    fn classify_authority_unsupported_for_macro_in_host_prefix() {
        // Anchored macros in the middle of the host are also rejected.
        let result = classify_endpoint_authority("https://prefix-{{.Host}}/openrtb");
        assert_eq!(
            result,
            EndpointAuthorityKind::UnsupportedMacro("prefix-{{.Host}}".to_owned()),
        );
    }

    // ----- private-IP authority expansion -----

    #[test]
    fn default_build_rejects_ipv4_unspecified() {
        let result = validate_endpoint_url("https://0.0.0.0/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_ipv6_unspecified() {
        let result = validate_endpoint_url("https://[::]/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_ipv4_multicast() {
        let result = validate_endpoint_url("https://224.0.0.1/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_ipv6_multicast() {
        let result = validate_endpoint_url("https://[ff00::1]/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_documentation_test_net_1() {
        let result = validate_endpoint_url("https://192.0.2.1/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_documentation_test_net_2() {
        let result = validate_endpoint_url("https://198.51.100.5/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_documentation_test_net_3() {
        let result = validate_endpoint_url("https://203.0.113.5/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_benchmarking_range() {
        let result = validate_endpoint_url("https://198.18.0.1/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_class_e_reserved_range() {
        let result = validate_endpoint_url("https://240.0.0.1/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_ipv6_documentation_range() {
        let result = validate_endpoint_url("https://[2001:db8::1]/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_rejects_this_network_range() {
        // 0.x.x.x outside the unspecified literal is still "this network"
        // per RFC 1122.
        let result = validate_endpoint_url("https://0.1.2.3/openrtb2", false);
        assert_eq!(result, ValidationResult::PrivateIpAuthority);
    }

    #[test]
    fn default_build_accepts_truly_public_ipv4() {
        // Sanity: a real public IP literal still passes.
        let result = validate_endpoint_url("https://8.8.8.8/openrtb2", false);
        assert_eq!(result, ValidationResult::Ok);
    }

    // ----- endpoint_macros extraction -----

    #[test]
    fn extract_macros_empty_for_fixed_endpoint_no_macros() {
        let macros = extract_endpoint_macros("https://hb.example.test/openrtb");
        assert!(macros.is_empty(), "expected empty, got {macros:?}");
    }

    #[test]
    fn extract_macros_excludes_host_authority_macro() {
        // {{.Host}} is the authority-substitution token handled via
        // hostAllowlist, not a path/query fill — must NOT appear in
        // endpoint_macros.
        let macros = extract_endpoint_macros("https://{{.Host}}/openrtb");
        assert!(macros.is_empty(), "expected empty, got {macros:?}");
    }

    #[test]
    fn extract_macros_picks_up_single_path_macro() {
        let macros = extract_endpoint_macros("https://hb.example.test/{{.AccountID}}/openrtb");
        assert_eq!(macros, vec!["AccountID".to_owned()]);
    }

    #[test]
    fn extract_macros_dedups_and_sorts() {
        let endpoint = "https://hb.example.test/{{.PublisherID}}/openrtb?\
                    acct={{.AccountID}}&pub={{.PublisherID}}";
        let macros = extract_endpoint_macros(endpoint);
        assert_eq!(
            macros,
            vec!["AccountID".to_owned(), "PublisherID".to_owned()],
        );
    }

    #[test]
    fn extract_macros_combines_host_varying_with_path_macros() {
        // Host-varying endpoint + path macro: {{.Host}} excluded, the
        // path macro is included.
        let endpoint = "https://{{.Host}}/openrtb?acct={{.AccountID}}";
        let macros = extract_endpoint_macros(endpoint);
        assert_eq!(macros, vec!["AccountID".to_owned()]);
    }

    #[test]
    fn host_macro_outside_authority_is_flagged() {
        use super::build_support::endpoint_has_host_macro_outside_authority;
        // Authority is `{{.Host}}` → not "outside authority".
        assert!(!endpoint_has_host_macro_outside_authority(
            "https://{{.Host}}/openrtb"
        ));
        // Authority is a literal but `{{.Host}}` appears in the path.
        assert!(endpoint_has_host_macro_outside_authority(
            "https://fixed.example.test/rtb/{{.Host}}"
        ));
        // Authority AND path both have it — still flagged.
        assert!(endpoint_has_host_macro_outside_authority(
            "https://{{.Host}}/rtb/{{.Host}}"
        ));
        // Authority is a literal, path has {{.AccountID}} but no Host.
        assert!(!endpoint_has_host_macro_outside_authority(
            "https://fixed.example.test/{{.AccountID}}"
        ));
        // `{{.Host}}` in query.
        assert!(endpoint_has_host_macro_outside_authority(
            "https://fixed.example.test/rtb?h={{.Host}}"
        ));
    }

    #[test]
    fn unclosed_endpoint_macro_detected() {
        use super::build_support::endpoint_unclosed_macro_prefix;
        // Fully-closed endpoint → None.
        assert!(
            endpoint_unclosed_macro_prefix("https://example.com/openrtb?acct={{.AccountID}}")
                .is_none()
        );
        // Trailing unclosed macro → Some(prefix).
        let unclosed =
            endpoint_unclosed_macro_prefix("https://example.com/openrtb?acct={{.AccountID")
                .expect("expected unclosed-macro detection");
        assert!(
            unclosed.starts_with("{{"),
            "prefix should start with `{{{{`; got {unclosed:?}"
        );
        // Closed macro followed by unclosed one → still detected.
        let mixed = endpoint_unclosed_macro_prefix(
            "https://example.com/{{.AccountID}}/openrtb?p={{.PublisherID",
        )
        .expect("expected unclosed-macro detection for the trailing token");
        assert!(mixed.contains(".PublisherID"));
    }

    // `bidder_timeout_ms` positivity is now enforced
    // via `#[validate(range(min = 1))]` on `BidderInfoRaw::bidder_timeout_ms`
    // in build.rs (using the `validator` crate). The end-to-end check —
    // "build.rs rejects bidderTimeoutMs: 0 with a clear message" — lives
    // in tests/build_failure_harness.rs.

    // ----- Bidder-owned endpoint contracts -----

    #[test]
    fn endpoint_macro_contracts_do_not_share_names_between_owners() {
        let macros = ["RouteID".into()];
        validate_endpoint_macros("one", &macros, &["RouteID".into()]).unwrap();
        assert!(validate_endpoint_macros("two", &macros, &["Zone9".into()]).is_err());
        for name in ["AccountID", "PublisherID", "RouteID"] {
            assert!(
                validate_endpoint_macros("none", &[name.into()], &[]).is_err(),
                "{name}"
            );
        }
    }

    #[test]
    fn validate_endpoint_macros_accepts_supported_set() {
        let declared = ["AccountID".into(), "PublisherID".into(), "RouteID".into()];
        validate_endpoint_macros("ax", &[], &[]).unwrap();
        for name in &declared {
            validate_endpoint_macros("ax", from_ref(name), &declared).unwrap();
        }
    }

    #[test]
    fn validate_endpoint_macros_rejects_privacy_macro_on_endpoint() {
        let err = validate_endpoint_macros("ax", &["GDPR".into()], &["GDPR".into()])
            .expect_err("expected reject for GDPR");
        assert!(err.contains("unsupported macro"));
        assert!(!err.contains("GDPR"));
    }

    #[test]
    fn endpoint_macro_contract_refuses_invalid_names_even_when_unused() {
        for declared in [
            vec!["RouteID".into(), "RouteID".into()],
            vec!["9Route".into()],
            vec!["Route ID".into()],
            vec!["Host".into()],
            vec!["ExternalURL".into()],
            vec!["SyncerKey".into()],
            vec!["BidderName".into()],
            vec!["SyncType".into()],
            vec!["UserMacro".into()],
        ] {
            assert!(
                validate_endpoint_macros("future", &[], &declared).is_err(),
                "{declared:?}"
            );
        }
    }
}
