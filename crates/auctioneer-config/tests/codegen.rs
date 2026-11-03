//! Rendering tests against the real build-support library.

use auctioneer_config_schema::{build_catalog, codegen};

#[cfg(test)]
mod tests {
    #[test]
    fn bidder_manifest_accepts_owned_optional_integer_settings() {
        let parsed = toml::from_str::<super::build_catalog::Manifest>(
            "key = 'fake_probe'\nfactory = false\n[settings.fields]\nnumber = 'optional_i64'\nvideo_number = 'optional_i64'\n[provenance]\norigin = 'local'\n",
        );
        assert!(
            parsed.is_ok(),
            "bidder-owned integer declarations must parse"
        );
    }

    #[test]
    fn bidder_manifest_accepts_owned_optional_string_settings() {
        let parsed = toml::from_str::<super::build_catalog::Manifest>(
            "key = 'fake_probe'\nfactory = false\n[settings.fields]\ntoken = 'optional_string'\n[provenance]\norigin = 'local'\n",
        );
        assert!(
            parsed.is_ok(),
            "bidder-owned string declarations must parse"
        );
    }

    use super::codegen::usersync::{
        AccountCookieSyncConfig, PurposeOneTreatment, PurposeOneTreatmentOverride, SkipWhen,
        SyncResponseFormat, SyncType, UserSyncConfig,
    };
    use super::codegen::{
        AccountCcpaConfig, AccountGdprConfigRaw, AccountPrivacyConfigRaw, AccountRaw,
        BidderInfoRaw, CapabilitiesRaw, CapabilityShapeRaw, CcpaRaw, ChannelEnabledRaw, DebugRaw,
        GdprRaw, HostCookieRaw, HostDefaultRaw, LmtRaw, MaintainerRaw, OpenRtbRaw,
        PurposeEnforcementRaw, RenderError, ResourceLimitsRaw, RuntimeBootstrapRaw,
        SpecialFeatureEnforcementRaw, SyncerEndpointRaw, Tcf2ConfigRaw, UserSyncRaw,
        render_account_config, render_account_gdpr, render_account_privacy, render_bidder_info,
        render_bidder_type_imports, render_channel_enabled, render_endpoint_macros,
        render_enforce_algo, render_host_allowlist, render_host_config, render_host_vendor_id,
        render_int_i64, render_int_usize, render_media_types, render_openrtb,
        render_purpose_enforcement, render_resource_limits, render_special_feature_enforcement,
        render_string_string_btreemap, render_syncer_endpoint, render_tcf2, render_user_sync,
    };
    use auctioneer_config::types::{AccountConfig, IpKeepBits};
    use quote::ToTokens as _;
    use std::collections::BTreeMap;

    fn tokens(source: &str) -> String {
        source
            .parse::<proc_macro2::TokenStream>()
            .unwrap()
            .to_string()
    }

    fn contains_syntax(source: &str, expected: &str) -> bool {
        tokens(source).contains(&tokens(expected))
    }

    fn assert_syntax_eq(actual: impl AsRef<str>, expected: impl AsRef<str>) {
        assert_eq!(tokens(actual.as_ref()), tokens(expected.as_ref()));
    }

    fn field(source: &str, name: &str) -> String {
        let value: syn::ExprStruct = syn::parse_str(source).unwrap();
        value
            .fields
            .iter()
            .find(|field| matches!(&field.member, syn::Member::Named(member) if member == name))
            .unwrap()
            .expr
            .to_token_stream()
            .to_string()
    }

    fn struct_name(source: &str) -> String {
        syn::parse_str::<syn::ExprStruct>(source)
            .unwrap()
            .path
            .to_token_stream()
            .to_string()
    }

    #[test]
    fn baked_activity_schema_and_bounds() {
        let valid = r#"
            [allowactivities.fetchBids]
            default = false
            rules = [{allow = true, condition = {componentName = ["fake_future_alias"], componentType = ["BIDDER"]}}, {allow = false}]
        "#;
        let raw: AccountPrivacyConfigRaw = toml::from_str(valid).expect("baked activity config");
        let rendered = render_account_privacy(&raw).expect("validated render");
        assert!(contains_syntax(&rendered, "allowactivities"));
        assert!(contains_syntax(&rendered, "Some(false)"));
        assert!(contains_syntax(&rendered, "fake_future_alias"));
        for suffix in [
            "rules = [{}]".to_owned(),
            "rules = [{allow = true, condition = {componentType = ['module']}}]".to_owned(),
            "rules = [{allow = true, condition = {componentname = []}}]".to_owned(),
            format!("rules = [{}]", vec!["{allow = true}"; 129].join(",")),
            format!(
                "rules = [{{allow = true, condition = {{componentName = [{:?}]}}}}]",
                "x".repeat(129)
            ),
        ] {
            let src = format!("[allowactivities.fetchBids]\n{suffix}");
            let parsed = toml::from_str::<AccountPrivacyConfigRaw>(&src);
            assert!(parsed.is_err() || render_account_privacy(&parsed.expect("raw")).is_err());
        }
    }

    // ---- typed literals ------------------------------------------------

    #[test]
    fn baked_ccpa_accepts_partial_tristates_and_rejects_invalid_fields() {
        for (value, variant) in [
            ("enabled", "Enabled"),
            ("disabled", "Disabled"),
            ("unset", "Unset"),
        ] {
            let source = account_toml(&format!(
                "\n[ccpa]\nenabled = '{value}'\n[ccpa.channel_enabled]\nweb = '{value}'\napp = '{value}'\namp = '{value}'\nvideo = '{value}'\ndooh = '{value}'\n"
            ));
            let raw: AccountRaw = toml::from_str(&source).expect("baked ccpa");
            let rendered = render_account_config(&raw).expect("render ccpa");
            let variant_ident = syn::Ident::new(variant, proc_macro2::Span::call_site());
            let ccpa = field(&rendered, "ccpa");
            assert_eq!(struct_name(&ccpa), "AccountCcpaConfig");
            assert_syntax_eq(
                field(&ccpa, "enabled"),
                quote::quote!(TriState::#variant_ident).to_string(),
            );
            for channel in ["web", "app", "amp", "video", "dooh"] {
                assert_syntax_eq(
                    field(&field(&ccpa, "channel_enabled"), channel),
                    quote::quote!(TriState::#variant_ident).to_string(),
                );
            }
        }
        let partial: AccountRaw = toml::from_str(&account_toml(
            "\n[ccpa.channel_enabled]\napp = 'disabled'\n",
        ))
        .expect("partial CCPA");
        let rendered = render_account_config(&partial).expect("partial render");
        assert_syntax_eq(
            field(&field(&rendered, "ccpa"), "enabled"),
            "TriState::Unset",
        );
        assert!(contains_syntax(
            &rendered,
            "web: TriState::Unset, app: TriState::Disabled"
        ));
        for block in [
            "[ccpa]\nenabled = true",
            "[ccpa]\nenabled = 'ENABLED'",
            "[ccpa]\nEnabled = 'enabled'",
            "[ccpa.channel_enabled]\nAPP = 'enabled'",
            "[ccpa.channel_enabled]\napp = 'invalid'",
        ] {
            let source = account_toml(&format!("\n{block}\n"));
            let parsed = toml::from_str::<AccountRaw>(&source);
            assert!(parsed.is_err() || render_account_config(&parsed.expect("raw")).is_err());
        }
    }

    #[test]
    fn bidder_literal_has_complete_named_fields() {
        let out = render_bidder_info(
            "fake_demo",
            &minimal_bidder("https://demo.example.test/rtb"),
        )
        .unwrap();
        let literal: syn::ExprStruct = syn::parse_str(&out).unwrap();
        assert_eq!(literal.path.to_token_stream().to_string(), "BidderInfo");
        let names: Vec<_> = literal
            .fields
            .iter()
            .map(|field| field.member.to_token_stream().to_string())
            .collect();
        assert_eq!(
            names,
            [
                "name",
                "endpoint",
                "uses_https",
                "alias_of",
                "default_enabled",
                "disabled",
                "debug_allow",
                "endpoint_compression",
                "gvl_vendor_id",
                "host_allowlist",
                "bidder_timeout_ms",
                "endpoint_macros",
                "maintainer_email",
                "capabilities",
                "user_sync",
                "openrtb"
            ]
        );
    }

    #[test]
    fn literal_strings_preserve_quotes_backslashes_unicode_and_newlines() {
        let name = "fixture\"\\\n\u{1f600}";
        let out =
            render_bidder_info(name, &minimal_bidder("https://demo.example.test/rtb")).unwrap();
        let call: syn::ExprMethodCall = syn::parse_str(&field(&out, "name")).unwrap();
        assert_eq!(call.method, "to_owned");
        let syn::Expr::Lit(literal) = *call.receiver else {
            panic!("expected a string literal");
        };
        let syn::Lit::Str(value) = literal.lit else {
            panic!("expected a string literal");
        };
        assert_eq!(value.value(), name);
    }

    #[test]
    fn metadata_constructor_parser_refuses_invalid_generated_syntax() {
        for source in [
            "BidderInfo {",
            "BidderInfo { name: }",
            "BidderInfo {}; unexpected()",
        ] {
            assert_eq!(
                super::codegen::storage::constructor_heap(source),
                Err(super::codegen::storage::StorageError::Syntax)
            );
        }
    }

    // ---- integer renderers ---------------------------------------------

    #[test]
    fn render_int_i64_adds_thousands_separators_and_signs() {
        assert_eq!(render_int_i64(0), "0");
        assert_eq!(render_int_i64(750), "750");
        assert_eq!(render_int_i64(1000), "1_000");
        assert_eq!(render_int_i64(1_234_567), "1_234_567");
        assert_syntax_eq(render_int_i64(-1_234_567), "-1_234_567");
    }

    #[test]
    fn render_int_usize_renders_power_of_two_as_hex() {
        assert_eq!(render_int_usize(0), "0");
        assert_eq!(render_int_usize(50), "50");
        assert_eq!(render_int_usize(1024), "0x400");
        assert_eq!(render_int_usize(0x0004_0000), "0x40000");
        // Non-power-of-two above 1024 still uses decimal.
        assert_eq!(render_int_usize(1500), "1_500");
    }

    // ---- host_allowlist -------------------------------------------------

    #[test]
    fn render_host_allowlist_none_renders_none() {
        assert_syntax_eq(render_host_allowlist(None), "None");
    }

    #[test]
    fn render_host_allowlist_empty_slice_renders_none() {
        assert_syntax_eq(render_host_allowlist(Some(&[])), "None");
    }

    #[test]
    fn render_host_allowlist_multiple_entries_renders_owned_vec() {
        let entries = vec!["eu.example.test".to_owned(), "us.example.test".to_owned()];
        assert_syntax_eq(
            render_host_allowlist(Some(&entries)),
            "Some(vec![\"eu.example.test\".to_owned(), \"us.example.test\".to_owned()])",
        );
    }

    // ---- endpoint_macros ------------------------------------------------

    #[test]
    fn render_endpoint_macros_empty_endpoint_returns_empty_vec_literal() {
        let out =
            render_endpoint_macros("fake_x", "https://x.example/rtb", &[]).expect("no macros");
        assert_syntax_eq(out, "Vec::new()");
    }

    #[test]
    fn render_endpoint_macros_extracts_account_id_and_publisher_id() {
        let out = render_endpoint_macros(
            "fake_x",
            "https://x.example/{{.PublisherID}}/rtb?a={{.AccountID}}",
            &["AccountID".into(), "PublisherID".into()],
        )
        .expect("auction-core macros");
        assert_syntax_eq(
            out,
            "vec![\"AccountID\".to_owned(), \"PublisherID\".to_owned()]",
        );
    }

    #[test]
    fn render_endpoint_macros_rejects_unclosed_macro() {
        let err = render_endpoint_macros("fake_x", "https://x.example/rtb?a={{.AccountID", &[])
            .expect_err("unclosed");
        assert!(err.to_string().contains("unclosed template macro"));
    }

    #[test]
    fn render_endpoint_macros_rejects_host_in_path() {
        let err = render_endpoint_macros("fake_x", "https://fixed.example/rtb/{{.Host}}", &[])
            .expect_err("host");
        assert!(err.to_string().contains("path or query"));
    }

    #[test]
    fn render_endpoint_macros_rejects_unsupported_macro() {
        let err = render_endpoint_macros("fake_x", "https://x.example/rtb?g={{.GDPR}}", &[])
            .expect_err("gdpr");
        assert!(err.to_string().contains("unsupported macro"));
        assert!(!err.to_string().contains("{{.GDPR}}"));
    }

    // ---- media_types ----------------------------------------------------

    #[test]
    fn endpoint_macro_rendering_uses_owner_contract_and_only_emits_template_members() {
        let declared = ["RouteID".into(), "Zone9".into()];
        assert_syntax_eq(
            render_endpoint_macros(
                "fake_future_owner",
                "https://x.example/{{.RouteID}}",
                &declared,
            )
            .unwrap(),
            "vec![\"RouteID\".to_owned()]",
        );
        for used in ["RouteID", "AccountID", "PublisherID"] {
            let template = format!("https://x.example/{{{{.{used}}}}}");
            assert!(
                render_endpoint_macros("fake_future_owner", &template, &[]).is_err(),
                "{used}"
            );
        }
        assert_eq!(
            render_endpoint_macros(
                "fake_other_owner",
                "https://x.example/{{.RouteID}}",
                &["Zone9".into()]
            )
            .unwrap_err()
            .to_string(),
            "bidder \"fake_other_owner\" endpoint contains an unsupported macro or invalid declaration",
        );
    }

    #[test]
    fn render_media_types_missing_shape_renders_empty_vec() {
        assert_syntax_eq(render_media_types(None), "Vec::new()");
    }

    #[test]
    fn render_media_types_filters_unknown_and_keeps_order() {
        let shape = CapabilityShapeRaw {
            media_types: vec![
                "banner".into(),
                "unknown-format".into(),
                "video".into(),
                "audio".into(),
            ],
        };
        assert_syntax_eq(
            render_media_types(Some(&shape)),
            "vec![MediaType::Banner, MediaType::Video, MediaType::Audio]",
        );
    }

    // ---- user_sync + syncer_endpoint -----------------------------------

    #[test]
    fn render_syncer_endpoint_none_renders_none() {
        assert_syntax_eq(render_syncer_endpoint(None), "None");
    }

    #[test]
    fn render_syncer_endpoint_with_empty_user_macro_treats_as_none() {
        let ep = SyncerEndpointRaw {
            url: "https://sync.x/redir".into(),
            user_macro: Some(String::new()),
        };
        let out = render_syncer_endpoint(Some(&ep));
        assert!(contains_syntax(&out, "user_macro: None"));
    }

    #[test]
    fn render_user_sync_defaults_render_all_none() {
        let out = render_user_sync(&UserSyncRaw::default());
        assert_syntax_eq(
            out,
            "SyncerConfig { key: None, iframe: None, redirect: None, supports: Vec::new(), enabled: true, skip_when: SkipWhen { gdpr: false, gpp_sid: Vec::new() }, default_response: None }",
        );
    }

    #[test]
    fn render_user_sync_with_iframe_and_supports_renders_them() {
        let sync = UserSyncRaw {
            key: Some("x".into()),
            iframe: Some(SyncerEndpointRaw {
                url: "https://sync.x/iframe".into(),
                user_macro: Some("$UID".into()),
            }),
            redirect: None,
            supports: vec![SyncType::Iframe],
            enabled: Some(true),
            skip_when: SkipWhen::default(),
            default_response: None,
        };
        let out = render_user_sync(&sync);
        assert!(contains_syntax(&out, "key: Some(\"x\".to_owned())"));
        assert_syntax_eq(
            field(&out, "iframe"),
            "Some(SyncerEndpoint { url: \"https://sync.x/iframe\".to_owned(), user_macro: Some(\"$UID\".to_owned()) })",
        );
        assert!(contains_syntax(
            &out,
            "user_macro: Some(\"$UID\".to_owned())"
        ));
        assert!(contains_syntax(&out, "supports: vec![SyncType::Iframe]"));
    }

    #[test]
    fn syncer_metadata_retains_skip_rules_and_default_response() {
        let raw = UserSyncRaw {
            skip_when: SkipWhen {
                gdpr: true,
                gpp_sid: vec!["7".to_owned(), "-1".to_owned()],
            },
            default_response: Some(SyncResponseFormat::Image),
            ..UserSyncRaw::default()
        };
        let rendered = render_user_sync(&raw);
        assert!(contains_syntax(
            &rendered,
            "skip_when: SkipWhen { gdpr: true, gpp_sid: vec![\"7\".to_owned(), \"-1\".to_owned()] }"
        ));
        assert!(contains_syntax(
            &rendered,
            "default_response: Some(SyncResponseFormat::Image)"
        ));
    }

    // ---- openrtb --------------------------------------------------------

    #[test]
    fn render_openrtb_default_renders_none_version_and_false_gpp() {
        assert_syntax_eq(
            render_openrtb(&OpenRtbRaw::default()),
            "OpenRtbCapabilities { version: None, gpp_supported: false }",
        );
    }

    #[test]
    fn render_openrtb_string_version_wraps_in_some() {
        let orb = OpenRtbRaw {
            version: Some(toml::Value::String("2.6".into())),
            gpp_supported: Some(true),
        };
        let out = render_openrtb(&orb);
        assert!(contains_syntax(&out, "version: Some(\"2.6\".to_owned())"));
        assert!(contains_syntax(&out, "gpp_supported: true"));
    }

    #[test]
    fn render_openrtb_numeric_version_is_stringified() {
        let orb = OpenRtbRaw {
            version: Some(toml::Value::Integer(2)),
            gpp_supported: None,
        };
        let out = render_openrtb(&orb);
        assert!(contains_syntax(&out, "version: Some(\"2\".to_owned())"));
    }

    // ---- BidderInfo -----------------------------------------------------

    fn minimal_bidder(endpoint: &str) -> BidderInfoRaw {
        BidderInfoRaw {
            implementation: "fake_fixture_adapter".to_owned(),
            supported_endpoint_macros: vec!["AccountID".into(), "PublisherID".into()],
            required_feature: None,
            native_only: false,
            alias_of: None,
            default_enabled: None,
            endpoint: endpoint.to_owned(),
            maintainer: None,
            disabled: None,
            endpoint_compression: None,
            gvl_vendor_id: None,
            host_allowlist: None,
            bidder_timeout_ms: None,
            debug: None,
            capabilities: CapabilitiesRaw::default(),
            user_sync: UserSyncRaw::default(),
            openrtb: OpenRtbRaw::default(),
        }
    }

    #[test]
    fn render_bidder_info_minimal_yields_all_defaults() {
        let info = minimal_bidder("https://demo.example.test/openrtb");
        let out = render_bidder_info("fake_demo", &info).expect("render");
        // Field-by-field spot-checks.
        assert_eq!(struct_name(&out), "BidderInfo");
        assert!(contains_syntax(
            &out,
            "            name: \"fake_demo\".to_owned(),\n"
        ));
        assert!(contains_syntax(&out, "            uses_https: true,\n"));
        assert!(contains_syntax(&out, "            disabled: false,\n"));
        assert!(contains_syntax(&out, "            debug_allow: true,\n"));
        assert!(contains_syntax(
            &out,
            "            endpoint_compression: None,\n"
        ));
        assert!(contains_syntax(&out, "            gvl_vendor_id: None,\n"));
        assert!(contains_syntax(&out, "            host_allowlist: None,\n"));
        assert!(contains_syntax(
            &out,
            "            bidder_timeout_ms: None,\n"
        ));
        assert!(contains_syntax(
            &out,
            "            endpoint_macros: Vec::new(),\n"
        ));
        assert!(contains_syntax(
            &out,
            "            maintainer_email: None,\n"
        ));
        assert!(contains_syntax(
            &out,
            "            capabilities: Capabilities { site: Vec::new(), app: Vec::new(), dooh: Vec::new() }"
        ));
        assert_eq!(struct_name(&field(&out, "user_sync")), "SyncerConfig");
        syn::parse_str::<syn::ExprStruct>(&out).unwrap();
    }

    #[test]
    fn compiled_bidder_names_are_bounded_by_utf8_bytes() {
        let info = minimal_bidder("https://demo.example.test/openrtb");
        for name in ["a".repeat(65), "\u{1f600}".repeat(17)] {
            let error = render_bidder_info(&name, &info).expect_err("oversized bidder identity");
            assert!(error.to_string().contains("64"));
            assert!(error.to_string().len() < 128, "no input-bearing diagnostic");
        }
        for name in ["a".repeat(64), "\u{1f600}".repeat(16)] {
            render_bidder_info(&name, &info).expect("exactly 64 bytes is valid");
        }
    }

    #[test]
    fn render_bidder_info_http_endpoint_sets_uses_https_false() {
        let mut info = minimal_bidder("http://loopback.example.test/rtb");
        info.disabled = Some(true);
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(&out, "            uses_https: false,\n"));
        assert!(contains_syntax(&out, "            disabled: true,\n"));
    }

    #[test]
    fn render_bidder_info_gzip_compression_renders_some_gzip() {
        let mut info = minimal_bidder("https://demo.example.test/rtb");
        info.endpoint_compression = Some("gzip".into());
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(
            &out,
            "            endpoint_compression: Some(Compression::Gzip),\n"
        ));
    }

    #[test]
    fn render_bidder_info_host_allowlist_survives_end_to_end() {
        let mut info = minimal_bidder("https://{{.Host}}/openrtb");
        info.host_allowlist = Some(vec!["eu.example.test".into(), "us.example.test".into()]);
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(
            &out,
            "            host_allowlist: Some(vec![\"eu.example.test\".to_owned(), \"us.example.test\".to_owned()]),\n"
        ));
    }

    #[test]
    fn render_bidder_info_maintainer_email_wraps_in_some() {
        let mut info = minimal_bidder("https://demo.example.test/rtb");
        info.maintainer = Some(MaintainerRaw {
            email: Some("dev@auctioneer.test".into()),
        });
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(
            &out,
            "            maintainer_email: Some(\"dev@auctioneer.test\".to_owned()),\n"
        ));
    }

    #[test]
    fn render_bidder_info_empty_maintainer_email_stays_none() {
        let mut info = minimal_bidder("https://demo.example.test/rtb");
        info.maintainer = Some(MaintainerRaw {
            email: Some(String::new()),
        });
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(
            &out,
            "            maintainer_email: None,\n"
        ));
    }

    #[test]
    fn render_bidder_info_debug_allow_defaults_true_when_debug_missing() {
        let info = minimal_bidder("https://demo.example.test/rtb");
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(&out, "            debug_allow: true,\n"));
    }

    #[test]
    fn render_bidder_info_debug_allow_respects_explicit_false() {
        let mut info = minimal_bidder("https://demo.example.test/rtb");
        info.debug = Some(DebugRaw { allow: Some(false) });
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(&out, "            debug_allow: false,\n"));
    }

    #[test]
    fn render_bidder_info_bidder_timeout_ms_is_typed_u32_literal() {
        let mut info = minimal_bidder("https://demo.example.test/rtb");
        info.bidder_timeout_ms = Some(750);
        let out = render_bidder_info("fake_demo", &info).expect("render");
        assert!(contains_syntax(
            &out,
            "            bidder_timeout_ms: Some(750_u32),\n"
        ));
    }

    #[test]
    fn render_bidder_info_rejects_zero_timeout() {
        let mut info = minimal_bidder("https://demo.example.test/rtb");
        info.bidder_timeout_ms = Some(0);
        let err = render_bidder_info("fake_demo", &info).expect_err("must be > 0");
        assert!(err.to_string().contains("bidderTimeoutMs"));
    }

    #[test]
    fn render_bidder_info_rejects_endpoint_with_unclosed_macro() {
        let info = minimal_bidder("https://demo.example.test/rtb?a={{.AccountID");
        let err = render_bidder_info("fake_demo", &info).expect_err("unclosed");
        assert!(err.to_string().contains("unclosed template macro"));
    }

    // ---- imports --------------------------------------------------------

    #[test]
    fn render_bidder_type_imports_baseline_has_no_optional_types() {
        let bidders = vec![("fake_demo".to_owned(), minimal_bidder("https://x/rtb"))];
        let out = render_bidder_type_imports(&bidders);
        assert_syntax_eq(
            out,
            "use crate::types::{BidderInfo, Capabilities, OpenRtbCapabilities, SkipWhen, SyncerConfig};\n",
        );
    }

    #[test]
    fn render_bidder_type_imports_pulls_compression_when_any_bidder_uses_gzip() {
        let mut with_gzip = minimal_bidder("https://x/rtb");
        with_gzip.endpoint_compression = Some("gzip".into());
        let bidders = vec![("fake_demo".to_owned(), with_gzip)];
        let out = render_bidder_type_imports(&bidders);
        assert!(contains_syntax(&out, "Compression"));
    }

    #[test]
    fn render_bidder_type_imports_pulls_media_type_when_any_capability_names_one() {
        let mut with_banner = minimal_bidder("https://x/rtb");
        with_banner.capabilities.site = Some(CapabilityShapeRaw {
            media_types: vec!["banner".into()],
        });
        let bidders = vec![("fake_demo".to_owned(), with_banner)];
        let out = render_bidder_type_imports(&bidders);
        assert!(contains_syntax(&out, "MediaType"));
    }

    #[test]
    fn render_bidder_type_imports_pulls_syncer_endpoint_when_iframe_declared() {
        let mut with_sync = minimal_bidder("https://x/rtb");
        with_sync.user_sync.iframe = Some(SyncerEndpointRaw {
            url: "https://x/iframe".into(),
            user_macro: None,
        });
        let bidders = vec![("fake_demo".to_owned(), with_sync)];
        let out = render_bidder_type_imports(&bidders);
        assert!(contains_syntax(&out, "SyncerEndpoint"));
    }

    #[test]
    fn render_bidder_type_imports_no_syncer_endpoint_when_only_supports_field_set() {
        let mut with_supports = minimal_bidder("https://x/rtb");
        with_supports.user_sync.supports = vec![SyncType::Iframe];
        let bidders = vec![("fake_demo".to_owned(), with_supports)];
        let out = render_bidder_type_imports(&bidders);
        assert!(!contains_syntax(&out, "SyncerEndpoint"));
    }

    // ---- BTreeMap block ------------------------------------------------

    #[test]
    fn render_string_string_btreemap_empty_collapses_to_btreemap_new() {
        let map = BTreeMap::new();
        assert_syntax_eq(
            render_string_string_btreemap(&map).expect("empty"),
            "BTreeMap::new()",
        );
    }

    #[test]
    fn render_string_string_btreemap_nonempty_emits_inline_block() {
        let mut map = BTreeMap::new();
        map.insert("fake_alias".to_owned(), "fake_base".to_owned());
        let out = render_string_string_btreemap(&map).expect("nonempty");
        assert!(contains_syntax(&out, "let mut m = BTreeMap::new();"));
        assert!(contains_syntax(
            &out,
            "m.insert(\"fake_alias\".to_owned(), \"fake_base\".to_owned());"
        ));
        let block: syn::ExprBlock = syn::parse_str(&out).unwrap();
        assert!(
            matches!(block.block.stmts.last(), Some(syn::Stmt::Expr(syn::Expr::Path(path), None)) if path.path.is_ident("m"))
        );
    }

    // ---- resource_limits -----------------------------------------------

    #[test]
    fn outbound_request_limits_render_defaults_and_enforce_range() {
        use validator::Validate as _;

        let base = "default_tmax_ms = 750\nmax_tmax_ms = 1500\nmax_bidder_response_bytes = 262144\nmax_bidders_per_auction = 50\nmax_outbound_requests = 100\nmax_transport_bytes = 262144\nmax_decoded_bytes = 262144\nmax_parsed_nodes = 65536\n";
        let defaults: ResourceLimitsRaw = toml::from_str(base).unwrap();
        let rendered = render_resource_limits(&defaults).unwrap();
        for field in ["max_outbound_raw_bytes", "max_outbound_request_bytes"] {
            assert_syntax_eq(self::field(&rendered, field), "0x4_0000");
            for (value, literal) in [
                (1024_u64, "0x400"),
                (0x0100_0000, "0x100_0000"),
                (0x0004_0001, "262_145"),
            ] {
                let limits: ResourceLimitsRaw =
                    toml::from_str(&format!("{base}{field} = {value}\n")).unwrap();
                limits.validate().unwrap();
                assert_syntax_eq(
                    self::field(&render_resource_limits(&limits).unwrap(), field),
                    literal,
                );
            }
            for value in [0_u64, 1023, 0x0100_0001] {
                let limits: ResourceLimitsRaw =
                    toml::from_str(&format!("{base}{field} = {value}\n")).unwrap();
                assert!(limits.validate().is_err(), "{field}={value}");
            }
        }
    }

    fn minimal_limits() -> ResourceLimitsRaw {
        ResourceLimitsRaw {
            default_tmax_ms: 750,
            max_tmax_ms: 1500,
            max_bidder_response_bytes: 0x0004_0000,
            max_bidders_per_auction: 50,
            max_outbound_requests: 100,
            max_outbound_raw_bytes: 0x0004_0000,
            max_outbound_request_bytes: 0x0004_0000,
            max_transport_bytes: 0x0004_0000,
            max_decoded_bytes: 0x0004_0000,
            max_parsed_nodes: 0x0001_0000,
        }
    }

    #[test]
    fn render_resource_limits_formats_ints_with_separators_and_hex_pow2() {
        let out = render_resource_limits(&minimal_limits()).expect("render");
        assert!(contains_syntax(&out, "default_tmax_ms: 750,"));
        assert!(contains_syntax(&out, "max_tmax_ms: 1_500,"));
        assert!(contains_syntax(&out, "max_bidder_response_bytes: 0x40000,"));
        assert!(contains_syntax(&out, "max_bidders_per_auction: 50,"));
        assert!(contains_syntax(&out, "max_outbound_requests: 100,"));
        // The five envelope-funded byte/node caps. Rendered unsuffixed,
        // so the `u64` / `u32` field types give each literal its own.
        assert!(contains_syntax(&out, "max_outbound_raw_bytes: 0x4_0000,"));
        assert!(contains_syntax(
            &out,
            "max_outbound_request_bytes: 0x4_0000,"
        ));
        assert!(contains_syntax(&out, "max_transport_bytes: 0x4_0000,"));
        assert!(contains_syntax(&out, "max_decoded_bytes: 0x4_0000,"));
        assert!(contains_syntax(&out, "max_parsed_nodes: 0x1_0000,"));
    }

    /// A non-power-of-two value takes the separated-decimal arm, which
    /// is the arm the envelope-funded caps land on once an operator
    /// lowers one to something that is not a round power of two.
    #[test]
    fn render_resource_limits_separates_non_pow2_byte_caps() {
        let limits = ResourceLimitsRaw {
            max_outbound_raw_bytes: 180_000,
            max_outbound_request_bytes: 170_000,
            max_transport_bytes: 200_000,
            max_decoded_bytes: 150_000,
            max_parsed_nodes: 40_000,
            ..minimal_limits()
        };
        let out = render_resource_limits(&limits).expect("render");
        assert!(contains_syntax(&out, "max_outbound_raw_bytes: 180_000,"));
        assert!(contains_syntax(
            &out,
            "max_outbound_request_bytes: 170_000,"
        ));
        assert!(contains_syntax(&out, "max_transport_bytes: 200_000,"));
        assert!(contains_syntax(&out, "max_decoded_bytes: 150_000,"));
        assert!(contains_syntax(&out, "max_parsed_nodes: 40_000,"));
    }

    // ---- host_config end-to-end ---------------------------------------

    fn minimal_host() -> HostDefaultRaw {
        HostDefaultRaw {
            gdpr: GdprRaw {
                enabled: true,
                default_value: "0".into(),
                host_vendor_id: 0,
                tcf2: Tcf2ConfigRaw {
                    enabled: false,
                    purposes: BTreeMap::new(),
                    special_features: BTreeMap::new(),
                    purpose_one_treatment: PurposeOneTreatment::default(),
                },
                non_standard_publishers: Vec::new(),
            },
            ccpa: CcpaRaw { enforce: false },
            lmt: LmtRaw { enforce: true },
            host_cookie: HostCookieRaw {
                ttl_days: 30,
                ..HostCookieRaw::default()
            },
            user_sync: Some(UserSyncConfig::default()),
            status_response: String::new(),
            resource_limits: minimal_limits(),
            aliases: BTreeMap::new(),
        }
    }

    #[test]
    fn baked_status_banner_is_byte_bounded() {
        let mut host = minimal_host();
        host.status_response = "\u{1f600}".repeat(2_049);
        assert!(
            render_host_config(&host).is_err(),
            "baked banners must obey the byte cap"
        );
        host.status_response = "\u{1f600}".repeat(2_048);
        render_host_config(&host).expect("the exact byte boundary is valid");
    }

    #[test]
    fn render_host_config_empty_status_becomes_string_new() {
        let out = render_host_config(&minimal_host()).expect("render");
        assert!(contains_syntax(&out, "status_response: String::new(),"));
    }

    #[test]
    fn render_host_config_nonempty_status_wraps_in_owned() {
        let mut host = minimal_host();
        host.status_response = "auctioneer/build-42".into();
        let out = render_host_config(&host).expect("render");
        assert!(contains_syntax(
            &out,
            "status_response: \"auctioneer/build-42\".to_owned(),"
        ));
    }

    #[test]
    fn render_host_config_gdpr_ccpa_lmt_render_inline() {
        let out = render_host_config(&minimal_host()).expect("render");
        assert!(contains_syntax(
            &out,
            "gdpr: GdprConfig { enabled: true, default_value: \"0\".to_owned(), \
             host_vendor_id: Some(0_u32), tcf2: Tcf2Config { enabled: false, \
             purposes: BTreeMap::new(), special_features: BTreeMap::new(), \
             purpose_one_treatment: PurposeOneTreatment { enabled: true, access_allowed: true } }, \
             non_standard_publishers: Vec::new() },"
        ));
        assert!(contains_syntax(
            &out,
            "ccpa: CcpaConfig { enforce: false },"
        ));
        assert!(contains_syntax(&out, "lmt: LmtConfig { enforce: true },"));
        assert!(contains_syntax(&out, "ttl_days: 30_i64"));
    }

    #[test]
    fn render_host_config_preserves_sync_policy_and_response_caps() {
        let out = render_host_config(&minimal_host()).expect("render");
        assert!(contains_syntax(
            &out,
            "host_cookie: HostCookieConfig { cookie_name: \"auctioneer_uid\".to_owned(), \
             family: \"auctioneer\".to_owned(), domain: None, ttl_days: 30_i64, \
             max_cookie_size_bytes: 0_usize, optout_cookie: None },"
        ));
        assert!(contains_syntax(
            &out,
            "user_sync: Some(UserSyncConfig { external_url: \"https://sync.example.test\".to_owned(), \
             redirect_url: None, cooperative: CooperativeConfig { enabled_by_default: false }, \
             priority_groups: Vec::new(), allowed_origins: Vec::new() }),"
        ));
        assert!(contains_syntax(&out, "max_transport_bytes: 0x4_0000,"));
        assert!(contains_syntax(&out, "max_decoded_bytes: 0x4_0000,"));
        assert!(contains_syntax(&out, "max_parsed_nodes: 0x1_0000,"));
    }

    #[test]
    fn render_host_config_aliases_pass_through_map_renderer() {
        let mut host = minimal_host();
        host.aliases
            .insert("fake_myalias".into(), "fake_fixture_base".into());
        let out = render_host_config(&host).expect("render");
        assert!(matches!(
            syn::parse_str::<syn::Expr>(&field(&out, "aliases")).unwrap(),
            syn::Expr::Block(_)
        ));
        assert!(contains_syntax(
            &out,
            "m.insert(\"fake_myalias\".to_owned(), \"fake_fixture_base\".to_owned());"
        ));
    }

    // ---- account_config -----------------------------------------------

    fn minimal_account_gdpr() -> AccountGdprConfigRaw {
        AccountGdprConfigRaw {
            enabled: "unset".into(),
            channel_enabled: ChannelEnabledRaw {
                web: "unset".into(),
                app: "unset".into(),
                amp: "unset".into(),
                video: "unset".into(),
                dooh: "unset".into(),
            },
            purposes: BTreeMap::new(),
            special_features: BTreeMap::new(),
            basic_enforcement_vendors: Vec::new(),
            purpose_one_treatment: PurposeOneTreatmentOverride::default(),
        }
    }

    fn minimal_account() -> AccountRaw {
        AccountRaw {
            disabled: false,
            debug_allow: true,
            bid_rounding: "down".into(),
            targeting_prefix: "hb".into(),
            truncate_target_attribute: None,
            gdpr: minimal_account_gdpr(),
            cookie_sync: AccountCookieSyncConfig::default(),
            ccpa: AccountCcpaConfig::default(),
            privacy: AccountPrivacyConfigRaw::default(),
        }
    }

    #[test]
    fn render_account_config_default_uses_none_truncate() {
        let out = render_account_config(&minimal_account()).expect("render");
        assert!(contains_syntax(&out, "truncate_target_attribute: None,"));
    }

    #[test]
    fn render_account_config_with_truncate_emits_typed_usize_literal() {
        let mut acct = minimal_account();
        acct.truncate_target_attribute = Some(18);
        let out = render_account_config(&acct).expect("render");
        assert!(contains_syntax(
            &out,
            "truncate_target_attribute: Some(18_usize),"
        ));
    }

    #[test]
    fn render_account_config_string_fields_are_quoted_owned() {
        let out = render_account_config(&minimal_account()).expect("render");
        assert!(contains_syntax(&out, "bid_rounding: \"down\".to_owned(),"));
        assert!(contains_syntax(
            &out,
            "targeting_prefix: \"hb\".to_owned(),"
        ));
    }

    // ---- gdpr render helpers -------------------------------------------

    #[test]
    fn render_tcf2_renders_enabled_and_baked_maps() {
        let raw: Tcf2ConfigRaw = toml::from_str(
            r#"enabled = true

[purposes.2]
enforce_algo = "full"
enforce_purpose = "enabled"
enforce_vendors = "unset"

[special_features.1]
enforce = "enabled"
"#,
        )
        .expect("parse tcf2 raw");
        let out = render_tcf2(&raw).expect("render");
        assert_eq!(struct_name(&out), "Tcf2Config");
        assert_syntax_eq(field(&out, "enabled"), "true");
        assert!(matches!(
            syn::parse_str::<syn::Expr>(&field(&out, "purposes")).unwrap(),
            syn::Expr::Block(_)
        ));
        assert!(contains_syntax(
            &out,
            "m.insert(2_u8, PurposeEnforcement { enforce_algo: EnforceAlgo::Full, \
             enforce_purpose: TriState::Enabled, enforce_vendors: TriState::Unset, \
             vendor_exceptions: vec![] });"
        ));
        assert!(contains_syntax(
            &out,
            "m.insert(1_u8, SpecialFeatureEnforcement { enforce: TriState::Enabled, \
             vendor_exceptions: vec![] });"
        ));
    }

    #[test]
    fn render_purpose_enforcement_maps_algo_tristates_and_exceptions() {
        let purpose = PurposeEnforcementRaw {
            enforce_algo: "basic".into(),
            enforce_purpose: "enabled".into(),
            enforce_vendors: "disabled".into(),
            vendor_exceptions: vec!["fake_bidder_a".into(), "fake_bidder_b".into()],
        };
        assert_syntax_eq(
            render_purpose_enforcement(&purpose).expect("render"),
            "PurposeEnforcement { enforce_algo: EnforceAlgo::Basic, \
             enforce_purpose: TriState::Enabled, enforce_vendors: TriState::Disabled, \
             vendor_exceptions: vec![\"fake_bidder_a\".to_owned(), \"fake_bidder_b\".to_owned()] }",
        );
    }

    #[test]
    fn render_special_feature_enforcement_maps_tristate_and_exceptions() {
        let sf = SpecialFeatureEnforcementRaw {
            enforce: "disabled".into(),
            vendor_exceptions: vec!["fake_bidder_geo".into()],
        };
        assert_syntax_eq(
            render_special_feature_enforcement(&sf).expect("render"),
            "SpecialFeatureEnforcement { enforce: TriState::Disabled, \
             vendor_exceptions: vec![\"fake_bidder_geo\".to_owned()] }",
        );
    }

    #[test]
    fn render_channel_enabled_maps_all_five_channels() {
        let channels = ChannelEnabledRaw {
            web: "enabled".into(),
            app: "disabled".into(),
            amp: "unset".into(),
            video: "enabled".into(),
            dooh: "unset".into(),
        };
        assert_syntax_eq(
            render_channel_enabled(&channels).expect("render"),
            "ChannelEnabled { web: TriState::Enabled, app: TriState::Disabled, \
             amp: TriState::Unset, video: TriState::Enabled, dooh: TriState::Unset }",
        );
    }

    #[test]
    fn render_account_gdpr_renders_full_literal() {
        let raw: AccountGdprConfigRaw = toml::from_str(
            r#"enabled = "enabled"
basic_enforcement_vendors = ["fake_vendor_x"]

[channel_enabled]
web = "enabled"
app = "unset"
amp = "unset"
video = "unset"
dooh = "disabled"

[purposes.4]
enforce_algo = "basic"
enforce_purpose = "enabled"
enforce_vendors = "enabled"
vendor_exceptions = ["fake_bidder_a"]

[special_features.1]
enforce = "enabled"
"#,
        )
        .expect("parse account gdpr raw");
        let out = render_account_gdpr(&raw).expect("render");
        assert_eq!(struct_name(&out), "AccountGdprConfig");
        assert_syntax_eq(field(&out, "enabled"), "TriState::Enabled");
        assert_syntax_eq(
            field(&out, "channel_enabled"),
            "ChannelEnabled { web: TriState::Enabled, app: TriState::Unset, amp: TriState::Unset, video: TriState::Unset, dooh: TriState::Disabled }",
        );
        assert!(matches!(
            syn::parse_str::<syn::Expr>(&field(&out, "purposes")).unwrap(),
            syn::Expr::Block(_)
        ));
        assert!(contains_syntax(
            &out,
            "m.insert(4_u8, PurposeEnforcement { enforce_algo: EnforceAlgo::Basic, \
             enforce_purpose: TriState::Enabled, enforce_vendors: TriState::Enabled, \
             vendor_exceptions: vec![\"fake_bidder_a\".to_owned()] });"
        ));
        assert!(contains_syntax(
            &out,
            "m.insert(1_u8, SpecialFeatureEnforcement { enforce: TriState::Enabled, \
             vendor_exceptions: vec![] });"
        ));
        assert!(contains_syntax(
            &out,
            "basic_enforcement_vendors: vec![\"fake_vendor_x\".to_owned()]"
        ));
    }

    #[test]
    fn render_enforce_algo_rejects_unknown() {
        let err = render_enforce_algo("strict").expect_err("strict is not a valid algo");
        assert!(matches!(err, RenderError::InvalidEnforceAlgo(v) if v == "strict"));
    }

    #[test]
    fn render_enforce_algo_maps_full_and_basic() {
        assert_syntax_eq(
            render_enforce_algo("full").expect("full"),
            "EnforceAlgo::Full",
        );
        assert_syntax_eq(
            render_enforce_algo("basic").expect("basic"),
            "EnforceAlgo::Basic",
        );
    }

    #[test]
    fn render_host_vendor_id_accepts_0_and_65535_rejects_65536() {
        assert_syntax_eq(render_host_vendor_id(0).expect("zero"), "Some(0_u32)");
        assert_syntax_eq(
            render_host_vendor_id(0xFFFF).expect("u16 max"),
            "Some(65535_u32)",
        );
        let err = render_host_vendor_id(0x0001_0000).expect_err("one past u16 max");
        assert!(matches!(
            err,
            RenderError::HostVendorIdOutOfRange { value: 0x0001_0000 }
        ));
    }

    // ---- account privacy (IP-anon) -------------------------------------

    /// Minimal complete account TOML with a caller-supplied `privacy`
    /// block (possibly empty).
    fn account_toml(privacy_block: &str) -> String {
        format!(
            r#"disabled = false
debug_allow = false
bid_rounding = "down"

[gdpr]
enabled = "unset"

[gdpr.channel_enabled]
web = "unset"
app = "unset"
amp = "unset"
video = "unset"
dooh = "unset"
{privacy_block}"#
        )
    }

    #[test]
    fn account_privacy_defaults_round_trip() {
        // Runtime side: the default account carries the 24/56 family
        // defaults through `ip_keep_bits()`.
        assert_eq!(
            AccountConfig::default().ip_keep_bits(),
            IpKeepBits { v4: 24, v6: 56 }
        );
        // Codegen side: an account with no `privacy` block renders the
        // same pair.
        let no_privacy: AccountRaw = toml::from_str(&account_toml("")).expect("parse account");
        assert_syntax_eq(
            render_account_privacy(&no_privacy.privacy).expect("render"),
            "AccountPrivacyConfig { ipv4: IpAnonConfig { anon_keep_bits: 24 }, \
             ipv6: IpAnonConfig { anon_keep_bits: 56 }, allowactivities: { use std::sync::Arc; Arc::default() } }",
        );
        // And a `[privacy.ipv4]` override renders v4 = 16, v6 = 56.
        let with_v4: AccountRaw =
            toml::from_str(&account_toml("\n[privacy.ipv4]\nanon_keep_bits = 16\n"))
                .expect("parse account with ipv4 block");
        let out = render_account_privacy(&with_v4.privacy).expect("render");
        assert!(contains_syntax(
            &out,
            "ipv4: IpAnonConfig { anon_keep_bits: 16 }"
        ));
        assert!(contains_syntax(
            &out,
            "ipv6: IpAnonConfig { anon_keep_bits: 56 }"
        ));
    }

    #[test]
    fn account_privacy_missing_ipv6_defaults_to_56() {
        // A partial `[privacy]` block (only ipv4) must fall back to the
        // ipv6 FAMILY default 56 — not to `IpAnonConfigRaw`'s 24.
        let parsed: AccountRaw =
            toml::from_str(&account_toml("\n[privacy.ipv4]\nanon_keep_bits = 20\n"))
                .expect("parse account with ipv4 block");
        let out = render_account_privacy(&parsed.privacy).expect("render");
        assert!(contains_syntax(
            &out,
            "ipv4: IpAnonConfig { anon_keep_bits: 20 }"
        ));
        assert!(contains_syntax(
            &out,
            "ipv6: IpAnonConfig { anon_keep_bits: 56 }"
        ));
    }

    // -----------------------------------------------------------------
    // runtime-bootstrap.toml — the deploy-immutable policy. The build
    // bakes exactly two validated values, so the parser must be strict:
    // a misspelled key or a wrongly-typed spelling is a build failure,
    // never a silent default.
    // -----------------------------------------------------------------

    /// The full checked-in file, so a test spelling one key does not have
    /// to restate the other.
    const BOOTSTRAP_OK: &str =
        "[runtime]\nallow_missing_config = false\nmax_config_fetch_ms = 2000\n";

    #[test]
    fn runtime_bootstrap_parses_the_checked_in_file_to_the_safer_default() {
        let raw = include_str!("../data/runtime-bootstrap.toml");
        let parsed: RuntimeBootstrapRaw = toml::from_str(raw).expect("checked-in file parses");
        assert!(
            !parsed.runtime.allow_missing_config,
            "the shipped bootstrap must fail closed on a missing config",
        );
        assert!(
            parsed.runtime.max_config_fetch_ms > 0,
            "a zero fetch bound would expire every config read before it started",
        );
    }

    #[test]
    fn runtime_bootstrap_rejects_an_unknown_key() {
        let err = toml::from_str::<RuntimeBootstrapRaw>(
            "[runtime]\nallow_missing_config = false\nmax_config_fetch_ms = 2000\n\
             allow_missing_confg = true\n",
        )
        .expect_err("a misspelled key must not be ignored");
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    #[test]
    fn runtime_bootstrap_rejects_a_non_boolean_spelling() {
        for raw in [
            "[runtime]\nallow_missing_config = \"true\"\nmax_config_fetch_ms = 2000\n",
            "[runtime]\nallow_missing_config = 1\nmax_config_fetch_ms = 2000\n",
        ] {
            let err = toml::from_str::<RuntimeBootstrapRaw>(raw)
                .expect_err("a non-boolean spelling must not parse as a bool");
            assert!(err.to_string().contains("invalid type"), "{raw:?}: {err}");
        }
    }

    /// The fetch bound is strictly integral milliseconds. A quoted or
    /// fractional spelling must stop the build rather than round into a
    /// production timeout nobody wrote.
    #[test]
    fn runtime_bootstrap_rejects_a_non_integer_fetch_bound() {
        for raw in [
            "[runtime]\nallow_missing_config = false\nmax_config_fetch_ms = \"2000\"\n",
            "[runtime]\nallow_missing_config = false\nmax_config_fetch_ms = 2000.5\n",
            "[runtime]\nallow_missing_config = false\nmax_config_fetch_ms = -1\n",
        ] {
            let err = toml::from_str::<RuntimeBootstrapRaw>(raw)
                .expect_err("only a nonnegative integer parses as the fetch bound");
            let message = err.to_string();
            assert!(
                message.contains("invalid type") || message.contains("invalid value"),
                "{raw:?}: {message}",
            );
        }
    }

    #[test]
    fn runtime_bootstrap_rejects_a_missing_key() {
        for raw in [
            "[runtime]\n",
            "[runtime]\nallow_missing_config = false\n",
            "[runtime]\nmax_config_fetch_ms = 2000\n",
        ] {
            let err =
                toml::from_str::<RuntimeBootstrapRaw>(raw).expect_err("both keys are mandatory");
            assert!(err.to_string().contains("missing field"), "{raw:?}: {err}");
        }
        toml::from_str::<RuntimeBootstrapRaw>(BOOTSTRAP_OK)
            .expect("both keys present is what parses");
    }
}
