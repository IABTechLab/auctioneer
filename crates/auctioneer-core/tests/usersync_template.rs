#[cfg(test)]
mod usersync_template_controls;
use usersync_template_controls as template_controls;

#[cfg(test)]
mod tests {
    use auctioneer_config::url_safety::{ValidationResult, validate_syncer_template};
    use auctioneer_config_data::types::{AccountConfig, HostConfig};
    use auctioneer_config_data::usersync::{
        SyncType, SyncerEndpoint, UserSyncConfig, validate_user_sync,
    };
    use auctioneer_core::usersync::template::{
        BuiltSyncUrl, SyncPrivacyMacros, SyncTemplateInput, TemplateError, build_sync_url,
    };
    use auctioneer_privacy::{
        GdprSignalIn, SyncEndpoint, SyncPrivacyInput, gcl::Gcl, gpp, gvl::GvlStore,
        resolve_sync_privacy,
    };
    use serde_json::Value;
    use time::OffsetDateTime;

    use super::template_controls::PRIVACY_MACRO_CONTROLS;

    #[test]
    fn syncer_generation_and_rendering_agree_on_authority_spellings() {
        let allow_insecure = cfg!(feature = "insecure-bidders");
        let host = UserSyncConfig::default();
        for (base, accepted) in [
            ("https:///bidder.example.com", false),
            ("https:////bidder.example.com", false),
            (r"https://\bidder.example.com", false),
            (r"https://bidder.example.com\sync", false),
            (r"https://bidder.example.com/base\sync", false),
            ("https://bidder.example.com/base path", false),
            ("https://bidder.example.com/base\u{00a0}path", false),
            ("https://localhost.", false),
            ("https://sub.localhost.", false),
            ("https://nip.io.", false),
            ("https://127.0.0.1.nip.io.", false),
            ("https://0x0a000001", false),
            ("https://127.0.0.2.", false),
            ("https://bidder.example.com", true),
            ("HTTPS://bidder.example.com/base://kept", true),
            ("https://bidder.example.com/base%20path", true),
            ("https://bidder.example.com//base", true),
            ("https://snip.io.", true),
            ("https://0x08080808", true),
            ("http://localhost:9999", allow_insecure),
            ("http:///localhost:9999", false),
        ] {
            let endpoint = SyncerEndpoint {
                url: format!(
                    "{base}/sync?back={{{{.RedirectURL}}}}&gdpr={{{{.GDPR}}}}&consent={{{{.GDPRConsent}}}}&usp={{{{.USPrivacy}}}}&gpp={{{{.GPP}}}}&sid={{{{.GPPSID}}}}"
                ),
                user_macro: Some("#FIXTURE_UID".to_owned()),
            };
            let generated =
                validate_syncer_template(&endpoint.url, allow_insecure) == ValidationResult::Ok;
            let rendered = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_fixture_base",
                bidder_name: "fake_fixture_base",
                sync_type: SyncType::Redirect,
                account: Some("publisher & account"),
                privacy: SyncPrivacyMacros {
                    gdpr: "0",
                    gdpr_consent: "",
                    us_privacy: "1YNN",
                    gpp: "",
                    gpp_sid: "-1",
                },
            });
            assert_eq!(
                generated,
                rendered.is_ok(),
                "generation/render mismatch for {base:?}"
            );
            assert_eq!(generated, accepted, "unexpected policy for {base:?}");
            if let Ok(built) = rendered {
                assert_eq!(built.user_macro, "#FIXTURE_UID");
                assert!(!built.url.contains("{{."), "{base}");
                let parsed = url::Url::parse(&built.url).unwrap();
                assert_eq!(
                    parsed
                        .query_pairs()
                        .find(|(key, _)| key == "back")
                        .unwrap()
                        .1,
                    built.callback_url,
                    "{base}"
                );
            }
        }
    }

    fn field<'a>(value: &'a Value, name: &str) -> &'a str {
        value[name]
            .as_str()
            .unwrap_or_else(|| panic!("missing {name}"))
    }

    #[test]
    fn reference_sync_urls_preserve_callback_and_bidder_uid() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../auctioneer-bidder/tests/conformance/fixtures/usersync/templates.json"
        ))
        .expect("template fixture");
        let cases = fixture["cases"].as_array().expect("cases");
        assert_eq!(cases.len(), 12);
        for case in cases {
            let input = &case["input"];
            let metadata = &input["metadata"];
            let expected = &case["expected"];
            let id = field(case, "id");
            let sync_type = match field(expected, "type") {
                "iframe" => SyncType::Iframe,
                "redirect" => SyncType::Redirect,
                other => panic!("unsupported fixture sync type {other}"),
            };
            let account = field(input, "account");
            let template_with_account = field(input, "callback_template");
            let callback_template = if account.is_empty() {
                template_with_account.to_owned()
            } else {
                // The reference generator injected account into its template.
                // Auctioneer adds it at request time from typed account selection.
                let (before, with_account) = template_with_account
                    .split_once("&account=")
                    .expect("named reference account field");
                let (_, uid) = with_account
                    .split_once("&uid=")
                    .expect("terminal reference UID field");
                format!("{before}&uid={uid}")
            };
            let config = UserSyncConfig {
                external_url: field(input, "external_url").to_owned(),
                redirect_url: Some(callback_template),
                ..UserSyncConfig::default()
            };
            let endpoint = SyncerEndpoint {
                url: field(metadata, "endpoint_url").to_owned(),
                user_macro: metadata["user_macro"].as_str().map(str::to_owned),
            };
            let privacy = &input["privacy"];
            let built = build_sync_url(SyncTemplateInput {
                host: &config,
                endpoint: &endpoint,
                callback_bidder: field(input, "bidder"),
                bidder_name: field(input, "bidder"),
                sync_type,
                account: (!account.is_empty()).then_some(account),
                privacy: SyncPrivacyMacros {
                    gdpr: field(privacy, "GDPR"),
                    gdpr_consent: field(privacy, "GDPRConsent"),
                    us_privacy: field(privacy, "USPrivacy"),
                    gpp: field(privacy, "GPP"),
                    gpp_sid: field(privacy, "GPPSID"),
                },
            })
            .unwrap_or_else(|error| panic!("{id}: {error:?}"));
            assert_eq!(built.url, field(expected, "url"), "{id}");
            assert_eq!(built.callback_url, field(expected, "callback_utf8"), "{id}");
            assert_eq!(built.user_macro, field(expected, "user_macro"), "{id}");
            assert!(!built.url.contains("{{."), "{id}");
            assert_callback_round_trip(case, &built);
        }
    }

    fn assert_callback_round_trip(case: &Value, built: &BuiltSyncUrl) {
        let input = &case["input"];
        let round_trip = &case["expected"]["round_trip"];
        let id = field(case, "id");
        let uid = field(input, "simulated_uid");
        let encoded = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("x", uid)
            .finish();
        let encoded_uid = encoded.strip_prefix("x=").expect("test encoder pair");
        let callback = if built.user_macro.is_empty() {
            format!("{}{encoded_uid}", built.callback_url)
        } else {
            built
                .callback_url
                .replacen(&built.user_macro, encoded_uid, 1)
        };
        assert_eq!(callback, field(round_trip, "callback_utf8"), "{id}");
        let parsed = url::Url::parse(&callback).expect("substituted callback URL");
        assert!(parsed.fragment().is_none(), "{id}");
        let pairs: Vec<_> = parsed.query_pairs().collect();
        assert_eq!(
            pairs.iter().filter(|(key, _)| key == "uid").count(),
            1,
            "{id}"
        );
        assert_eq!(
            pairs.iter().find(|(key, _)| key == "uid").unwrap().1,
            uid,
            "{id}"
        );
        assert_eq!(
            pairs.iter().filter(|(key, _)| key == "account").count(),
            usize::try_from(round_trip["account_value_count"].as_u64().unwrap())
                .expect("fixture account count fits usize"),
            "{id}"
        );
        let account = field(input, "account");
        if !account.is_empty() {
            assert_eq!(
                pairs.iter().find(|(key, _)| key == "account").unwrap().1,
                account,
                "{id}"
            );
        }
        assert_eq!(
            pairs.iter().find(|(key, _)| key == "bidder").unwrap().1,
            field(round_trip, "bidder"),
            "{id}"
        );
        assert_eq!(
            pairs.iter().find(|(key, _)| key == "f").unwrap().1,
            field(round_trip, "format"),
            "{id}"
        );
    }

    fn empty_privacy() -> SyncPrivacyMacros<'static> {
        SyncPrivacyMacros {
            gdpr: "",
            gdpr_consent: "",
            us_privacy: "",
            gpp: "",
            gpp_sid: "",
        }
    }

    fn emit_endpoint(host: &UserSyncConfig, raw: &str) -> Result<BuiltSyncUrl, TemplateError> {
        let endpoint = SyncerEndpoint {
            url: raw.to_owned(),
            user_macro: Some("#PMUID".to_owned()),
        };
        build_sync_url(SyncTemplateInput {
            host,
            endpoint: &endpoint,
            callback_bidder: "fake_bbb",
            bidder_name: "fake_bbb",
            sync_type: SyncType::Redirect,
            account: Some("publisher & account"),
            privacy: empty_privacy(),
        })
    }

    #[test]
    fn syncer_rendering_rejects_parser_repaired_url_spellings() {
        let host = UserSyncConfig::default();
        for base in [
            "https:///bidder.example.com",
            "https:////bidder.example.com",
            "https://\\bidder.example.com",
            "https://bidder.example.com\\sync",
            "https://bidder.example.com/base\\sync",
            "https://bidder.example.com/base path",
            "https://bidder.example.com/base\u{00a0}path",
        ] {
            let endpoint = format!("{base}/sync?back={{{{.RedirectURL}}}}");
            assert!(
                matches!(
                    emit_endpoint(&host, &endpoint),
                    Err(TemplateError::InvalidEndpoint)
                ),
                "{endpoint:?}"
            );
        }
        for base in [
            "https://bidder.example.com",
            "HTTPS://bidder.example.com/base://kept",
            "https://bidder.example.com/base%20path",
            "https://bidder.example.com//base",
        ] {
            let endpoint = format!("{base}/sync?back={{{{.RedirectURL}}}}");
            let built =
                emit_endpoint(&host, &endpoint).expect("literal endpoint and valid base path");
            assert!(built.url.starts_with(&format!("{base}/sync?back=")));
            assert_eq!(built.user_macro, "#PMUID");
            let parsed = url::Url::parse(&built.url).unwrap();
            assert_eq!(parsed.host_str(), Some("bidder.example.com"));
            assert!(parsed.fragment().is_none());
            assert_eq!(
                parsed
                    .query_pairs()
                    .find(|(key, _)| key == "back")
                    .unwrap()
                    .1,
                built.callback_url
            );
        }
    }

    #[test]
    fn rendering_rejects_root_dotted_loopback_names_on_independent_authorities() {
        let public_endpoint = "https://bidder.example.com/sync?back={{.RedirectURL}}";
        for name in [
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
            let base = format!("https://{name}.:8443");
            let external = UserSyncConfig {
                external_url: format!("{base}/base/"),
                ..UserSyncConfig::default()
            };
            let redirect = UserSyncConfig {
                redirect_url: Some(format!(
                    "{base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            for host in [external, redirect] {
                assert!(
                    matches!(
                        emit_endpoint(&host, public_endpoint),
                        Err(TemplateError::InvalidConfig)
                    ),
                    "callback {base}"
                );
            }
            let endpoint = format!("{base}/sync?back={{{{.RedirectURL}}}}");
            assert!(
                matches!(
                    emit_endpoint(&UserSyncConfig::default(), &endpoint),
                    Err(TemplateError::InvalidEndpoint)
                ),
                "bidder {base}"
            );
        }
    }

    #[test]
    fn rendering_preserves_public_dns_and_normalized_ip_controls() {
        for authority in [
            "sync.example.com.",
            "snip.io.",
            "nip.io.example.",
            "8.8.8.8",
            "8.8.8.8.",
            "0x08080808",
            "[2606:4700:4700::1111]",
        ] {
            let host = UserSyncConfig {
                external_url: format!("https://{authority}:8443/base%20path/"),
                ..UserSyncConfig::default()
            };
            let endpoint = format!("https://{authority}:8443/sync?back={{{{.RedirectURL}}}}");
            let built = emit_endpoint(&host, &endpoint).expect("public authority");
            let substituted = built.callback_url.replace("#PMUID", "fake_bbb-id");
            let callback = url::Url::parse(&substituted).unwrap();
            assert_eq!(callback.path(), "/base%20path/setuid");
            assert_eq!(
                callback
                    .query_pairs()
                    .find(|(key, _)| key == "uid")
                    .unwrap()
                    .1,
                "fake_bbb-id"
            );
        }
        for authority in [
            "127.0.0.2.",
            "0x7f000002",
            "10.0.0.1.",
            "0x0a000001",
            "192.168.1.1.",
            "100.64.0.1.",
            "[fc00::1]",
            "[::ffff:8.8.8.8]",
        ] {
            let host = UserSyncConfig {
                external_url: format!("https://{authority}/base/"),
                ..UserSyncConfig::default()
            };
            assert!(
                matches!(
                    emit_endpoint(
                        &host,
                        "https://bidder.example.com/sync?back={{.RedirectURL}}"
                    ),
                    Err(TemplateError::InvalidConfig)
                ),
                "callback {authority}"
            );
            let endpoint = format!("https://{authority}/sync?back={{{{.RedirectURL}}}}");
            assert!(
                matches!(
                    emit_endpoint(&UserSyncConfig::default(), &endpoint),
                    Err(TemplateError::InvalidEndpoint)
                ),
                "bidder {authority}"
            );
        }
    }

    #[test]
    fn rendering_keeps_exact_feature_controlled_loopback_exceptions() {
        for authority in ["localhost", "127.0.0.1", "[::1]"] {
            for scheme in ["http", "https"] {
                let base = format!("{scheme}://{authority}:9999");
                let host = UserSyncConfig {
                    external_url: format!("{base}/base/"),
                    ..UserSyncConfig::default()
                };
                let endpoint = format!("{base}/sync?back={{{{.RedirectURL}}}}");
                for (callback, bidder) in [
                    (
                        &host,
                        "https://bidder.example.com/sync?back={{.RedirectURL}}",
                    ),
                    (&UserSyncConfig::default(), endpoint.as_str()),
                ] {
                    assert_eq!(
                        emit_endpoint(callback, bidder).is_ok(),
                        cfg!(feature = "insecure-bidders"),
                        "{base}"
                    );
                }
            }
        }
    }

    fn form_value(value: &str) -> String {
        url::form_urlencoded::Serializer::new(String::new())
            .append_pair("value", value)
            .finish()
            .strip_prefix("value=")
            .unwrap()
            .to_owned()
    }

    fn assert_privacy_rendering(
        privacy: SyncPrivacyMacros<'_>,
        account: Option<&str>,
    ) -> BuiltSyncUrl {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}&usp={{.USPrivacy}}&gpp={{.GPP}}&sid={{.GPPSID}}&gdpr={{.GDPR}}&consent={{.GDPRConsent}}".to_owned(),
            user_macro: Some("#PMUID".to_owned()),
        };
        let built = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bbb",
            bidder_name: "fake_bbb",
            sync_type: SyncType::Redirect,
            account,
            privacy,
        })
        .expect("privacy values are data, not template source");
        let account_field = account
            .map(|value| format!("&account={}", form_value(value)))
            .unwrap_or_default();
        let expected_callback = format!(
            "https://sync.example.com/setuid?bidder=fake_bbb&gdpr={}&gdpr_consent={}&us_privacy={}&gpp={}&gpp_sid={}&f=i{account_field}&uid=#PMUID",
            form_value(privacy.gdpr),
            form_value(privacy.gdpr_consent),
            form_value(privacy.us_privacy),
            form_value(privacy.gpp),
            form_value(privacy.gpp_sid),
        );
        assert_eq!(built.callback_url, expected_callback);
        assert_eq!(
            built.url,
            format!(
                "https://bidder.example.com/sync?back={}&usp={}&gpp={}&sid={}&gdpr={}&consent={}",
                form_value(&expected_callback),
                form_value(privacy.us_privacy),
                form_value(privacy.gpp),
                form_value(privacy.gpp_sid),
                form_value(privacy.gdpr),
                form_value(privacy.gdpr_consent),
            )
        );
        let bidder = url::Url::parse(&built.url).unwrap();
        assert!(bidder.fragment().is_none());
        let pairs: Vec<_> = bidder.query_pairs().collect();
        assert_eq!(pairs.len(), 6);
        // Decode the outer escape once. Inner percent escapes must remain intact.
        assert_eq!(pairs[0], ("back".into(), expected_callback.into()));
        assert_eq!(pairs[1], ("usp".into(), privacy.us_privacy.into()));
        assert_eq!(pairs[2], ("gpp".into(), privacy.gpp.into()));
        assert_eq!(pairs[3], ("sid".into(), privacy.gpp_sid.into()));
        assert_eq!(pairs[4], ("gdpr".into(), privacy.gdpr.into()));
        assert_eq!(pairs[5], ("consent".into(), privacy.gdpr_consent.into()));

        let substituted = pairs[0].1.replace("#PMUID", "actual-id");
        let callback = url::Url::parse(&substituted).unwrap();
        assert!(callback.fragment().is_none());
        let callback_pairs: Vec<_> = callback.query_pairs().collect();
        assert_eq!(
            callback_pairs.len(),
            8_usize.saturating_add(usize::from(account.is_some()))
        );
        for (key, expected_value) in [
            ("bidder", "fake_bbb"),
            ("gdpr", privacy.gdpr),
            ("gdpr_consent", privacy.gdpr_consent),
            ("us_privacy", privacy.us_privacy),
            ("gpp", privacy.gpp),
            ("gpp_sid", privacy.gpp_sid),
            ("f", "i"),
            ("uid", "actual-id"),
        ] {
            let values: Vec<_> = callback_pairs
                .iter()
                .filter(|(name, _)| name == key)
                .map(|(_, value)| value.as_ref())
                .collect();
            assert_eq!(values, vec![expected_value], "{key}");
        }
        let accounts: Vec<_> = callback_pairs
            .iter()
            .filter(|(key, _)| key == "account")
            .map(|(_, value)| value.as_ref())
            .collect();
        assert_eq!(accounts, account.into_iter().collect::<Vec<_>>());
        assert!(!callback_pairs.iter().any(|(key, _)| key == "forged_param"));
        built
    }

    #[test]
    fn authored_privacy_controls_cover_every_macro_with_literal_wire_expectations() {
        assert_eq!(
            PRIVACY_MACRO_CONTROLS
                .iter()
                .map(|(name, _, _)| *name)
                .collect::<Vec<_>>(),
            ["GDPR", "GDPRConsent", "USPrivacy", "GPP", "GPPSID"]
        );
        for &(name, raw, encoded) in PRIVACY_MACRO_CONTROLS {
            assert!(!raw.is_empty() && raw.contains('&') && raw.contains('#'));
            assert_ne!(encoded, raw, "{name} must exercise escaping");
            let mut privacy = empty_privacy();
            let (callback_key, bidder_key) = match name {
                "GDPR" => {
                    privacy.gdpr = raw;
                    ("gdpr", "gdpr")
                }
                "GDPRConsent" => {
                    privacy.gdpr_consent = raw;
                    ("gdpr_consent", "consent")
                }
                "USPrivacy" => {
                    privacy.us_privacy = raw;
                    ("us_privacy", "usp")
                }
                "GPP" => {
                    privacy.gpp = raw;
                    ("gpp", "gpp")
                }
                "GPPSID" => {
                    privacy.gpp_sid = raw;
                    ("gpp_sid", "sid")
                }
                _ => panic!("unknown renderer control {name}"),
            };
            let built = assert_privacy_rendering(privacy, None);
            for (url, key) in [
                (&built.callback_url, callback_key),
                (&built.url, bidder_key),
            ] {
                let (_, query) = url.split_once('?').unwrap();
                let prefix = format!("{key}=");
                let values: Vec<_> = query
                    .split('&')
                    .filter_map(|pair| pair.strip_prefix(&prefix))
                    .collect();
                assert_eq!(values, [encoded], "{name}: {key}");
            }
        }
    }

    #[test]
    fn live_parser_privacy_inputs_vary_independently_at_both_render_stages() {
        let account = AccountConfig::default();
        let host = HostConfig::default();
        let gvl = GvlStore::for_test(vec![]);
        let gcl = Gcl::for_test(vec![], OffsetDateTime::UNIX_EPOCH);
        for raw_gpp in [
            "",
            "DBABTA~opaque&forged_param=1&uid=STOLEN",
            "DBABTA~opaque%26+{{.UserMacro}}#fragment&forged_param=1&uid=STOLEN",
        ] {
            if !raw_gpp.is_empty() {
                let envelope = gpp::parse(raw_gpp, gpp::Limits::default()).unwrap();
                assert_eq!(envelope.section(6), raw_gpp.split_once('~').map(|(_, v)| v));
            }
            for usp in ["", "1YNN", "1NNY", "1---"] {
                for sid in ["", "-1", "07,8,0007"] {
                    let context = resolve_sync_privacy(SyncPrivacyInput {
                        gdpr: GdprSignalIn::CookieNumber(0),
                        consent: None,
                        us_privacy: Some(usp),
                        gpp: Some(raw_gpp),
                        gpp_sid: Some(sid),
                        account: &account,
                        host: &host,
                        endpoint: SyncEndpoint::CookieSync,
                        gvl: &gvl,
                        gcl: &gcl,
                        now: OffsetDateTime::UNIX_EPOCH,
                    })
                    .expect("live parser accepts each independent source combination");
                    assert_eq!(context.validated_gpp().unwrap_or_default(), raw_gpp);
                    assert_eq!(context.selected_usp().unwrap_or_default(), usp);
                    assert_eq!(context.raw_gpp_sid().unwrap_or_default(), sid);
                    if sid == "07,8,0007" {
                        assert!(context.sid().contains(7));
                        assert!(context.sid().contains(8));
                    }
                    for selected_account in [None, Some("publisher & account%26+")] {
                        assert_privacy_rendering(
                            SyncPrivacyMacros {
                                gdpr: "0",
                                gdpr_consent: "",
                                us_privacy: context.selected_usp().unwrap_or_default(),
                                gpp: context.validated_gpp().unwrap_or_default(),
                                gpp_sid: context.raw_gpp_sid().unwrap_or_default(),
                            },
                            selected_account,
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn raw_gdpr_consent_macro_value_is_escaped_once_at_both_render_stages() {
        // A real TCF string is base64url and therefore form-safe, so only
        // hostile boundary data makes the consent encoding observable.
        let mut privacy = empty_privacy();
        privacy.gdpr_consent = "CPabc&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment";
        assert_privacy_rendering(privacy, None);
    }

    #[test]
    fn raw_gdpr_macro_value_is_escaped_once_at_both_render_stages() {
        // Every real GDPR signal is form-safe ("", "0", "1"), so only hostile
        // boundary data makes the encoding at either render stage observable.
        let mut privacy = empty_privacy();
        privacy.gdpr = "1&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment";
        assert_privacy_rendering(privacy, None);
    }

    #[test]
    fn direct_gdpr_macro_value_is_escaped_once_in_bidder_endpoint() {
        let host = UserSyncConfig::default();
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?gdpr={{.GDPR}}&back={{.RedirectURL}}".to_owned(),
            user_macro: Some("#PMUID".to_owned()),
        };
        let mut privacy = empty_privacy();
        privacy.gdpr = "1&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment";
        let built = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bbb",
            bidder_name: "fake_bbb",
            sync_type: SyncType::Redirect,
            account: None,
            privacy,
        })
        .expect("GDPR boundary data cannot alter the bidder endpoint");
        assert_eq!(
            built.url,
            format!(
                "https://bidder.example.com/sync?gdpr={}&back={}",
                form_value(privacy.gdpr),
                form_value(&built.callback_url),
            )
        );
        let parsed = url::Url::parse(&built.url).unwrap();
        assert!(parsed.fragment().is_none());
        let pairs: Vec<_> = parsed.query_pairs().collect();
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0], ("gdpr".into(), privacy.gdpr.into()));
        assert_eq!(pairs[1], ("back".into(), built.callback_url.into()));
    }

    #[test]
    fn raw_gpp_macro_value_is_escaped_once_at_both_render_stages() {
        let mut privacy = empty_privacy();
        privacy.gpp = "DBABTA~opaque&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment";
        assert_privacy_rendering(privacy, None);
    }

    #[test]
    fn raw_usp_macro_value_is_escaped_once_at_both_render_stages() {
        // Valid USP is all form-safe; hostile boundary data makes encoding observable.
        let mut privacy = empty_privacy();
        privacy.us_privacy = "1YNN&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment";
        assert_privacy_rendering(privacy, None);
    }

    #[test]
    fn raw_sid_macro_value_is_escaped_once_at_both_render_stages() {
        let mut privacy = empty_privacy();
        privacy.gpp_sid = "07,8&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment";
        assert_privacy_rendering(privacy, None);
    }

    #[test]
    fn bidder_b_callback_cannot_route_uid_to_hardcoded_or_duplicate_bidder() {
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("#PMUID".to_owned()),
        };
        for query in [
            "uid={{.UserMacro}}",
            "bidder=fake_aaa&uid={{.UserMacro}}",
            "bidder=fake_aaa&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
            "%62idder=fake_aaa&bidder={{.SyncerKey}}&uid={{.UserMacro}}",
        ] {
            let host = UserSyncConfig {
                redirect_url: Some(format!("{{{{.ExternalURL}}}}/setuid?{query}")),
                ..UserSyncConfig::default()
            };
            let result = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_bbb",
                bidder_name: "fake_bbb",
                sync_type: SyncType::Redirect,
                account: None,
                privacy: empty_privacy(),
            });
            assert!(
                matches!(result, Err(TemplateError::InvalidConfig)),
                "{query}"
            );
        }
    }

    #[test]
    fn callback_emission_rejects_parser_repaired_external_url() {
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: None,
        };
        for base in [
            "https:sync.example.com",
            "https:/sync.example.com",
            "https://sync.example.com ",
        ] {
            let host = UserSyncConfig {
                external_url: base.to_owned(),
                ..UserSyncConfig::default()
            };
            let result = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_bbb",
                bidder_name: "fake_bbb",
                sync_type: SyncType::Redirect,
                account: None,
                privacy: empty_privacy(),
            });
            assert!(
                matches!(result, Err(TemplateError::InvalidConfig)),
                "{base}"
            );
        }
    }

    #[test]
    fn callback_emission_rejects_embedded_scheme_delimiter_in_base() {
        assert_unanchored_callback_refused(false);
    }

    #[test]
    fn callback_emission_rejects_embedded_scheme_delimiter_in_literal_redirect() {
        assert_unanchored_callback_refused(true);
    }

    fn assert_unanchored_callback_refused(literal_redirect: bool) {
        let bidder_base = url::Url::parse("https://bidder.example/sync").unwrap();
        let endpoint = SyncerEndpoint {
            url: format!("{bidder_base}?back={{{{.RedirectURL}}}}"),
            user_macro: None,
        };
        let malformed_base = "https:sync.example.test://unused";
        let host = if literal_redirect {
            UserSyncConfig {
                redirect_url: Some(format!(
                    "{malformed_base}/setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            }
        } else {
            UserSyncConfig {
                external_url: malformed_base.to_owned(),
                ..UserSyncConfig::default()
            }
        };
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bbb",
            bidder_name: "fake_bbb",
            sync_type: SyncType::Redirect,
            account: None,
            privacy: empty_privacy(),
        });
        if let Ok(built) = &result {
            let bidder_url = url::Url::parse(&built.url).unwrap();
            let callback = bidder_url
                .query_pairs()
                .find(|(key, _)| key == "back")
                .unwrap()
                .1;
            assert_eq!(callback, built.callback_url);
            let standalone = url::Url::parse(&callback).unwrap();
            let browser_resolved = bidder_base.join(&callback).unwrap();
            assert_eq!(standalone.host_str(), Some("sync.example.test"));
            assert_eq!(browser_resolved.host_str(), Some("bidder.example"));
            panic!("unsafe callback accepted: standalone={standalone}, browser={browser_resolved}");
        }
        assert!(matches!(result, Err(TemplateError::InvalidConfig)));
    }

    #[test]
    fn anchored_callback_with_path_delimiter_resolves_to_its_own_origin() {
        let bidder_base = url::Url::parse("https://bidder.example/sync").unwrap();
        for literal_redirect in [false, true] {
            let base = "HTTPS://sync.example.test/base://kept/";
            let host = UserSyncConfig {
                external_url: base.to_owned(),
                redirect_url: literal_redirect.then(|| {
                    format!("{base}setuid?bidder={{{{.SyncerKey}}}}&uid={{{{.UserMacro}}}}")
                }),
                ..UserSyncConfig::default()
            };
            let endpoint = SyncerEndpoint {
                url: format!("{bidder_base}?back={{{{.RedirectURL}}}}"),
                user_macro: Some("#PMUID".to_owned()),
            };
            let built = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_bbb",
                bidder_name: "fake_bbb",
                sync_type: SyncType::Redirect,
                account: None,
                privacy: empty_privacy(),
            })
            .expect("anchored callback remains absolute");
            let bidder_url = url::Url::parse(&built.url).unwrap();
            let callback = bidder_url
                .query_pairs()
                .find(|(key, _)| key == "back")
                .unwrap()
                .1;
            assert_eq!(callback, built.callback_url);
            let substituted_callback = callback.replace("#PMUID", "fake_bbb-id");
            let standalone = url::Url::parse(&substituted_callback).unwrap();
            let browser_resolved = bidder_base.join(&substituted_callback).unwrap();
            assert_eq!(standalone, browser_resolved);
            assert_eq!(browser_resolved.host_str(), Some("sync.example.test"));
            assert_eq!(browser_resolved.path(), "/base://kept/setuid");
            assert!(browser_resolved.fragment().is_none());
            assert_eq!(
                browser_resolved
                    .query_pairs()
                    .find(|(key, _)| key == "uid")
                    .unwrap()
                    .1,
                "fake_bbb-id"
            );
        }
    }

    #[test]
    fn bidder_endpoint_emission_rejects_shared_nonpublic_literals() {
        let host = UserSyncConfig::default();
        for authority in [
            "0.1.2.3",
            "[fec0::1]",
            "[64:ff9b::a00:1]",
            "[100::1]",
            "[3fff::1]",
        ] {
            let endpoint = SyncerEndpoint {
                url: format!("https://{authority}/sync?back={{{{.RedirectURL}}}}"),
                user_macro: None,
            };
            let result = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_bbb",
                bidder_name: "fake_bbb",
                sync_type: SyncType::Redirect,
                account: None,
                privacy: empty_privacy(),
            });
            assert!(
                matches!(result, Err(TemplateError::InvalidEndpoint)),
                "{authority}"
            );
        }
    }

    #[test]
    fn callback_binding_preserves_basepath_and_opaque_fragment_uid_macro() {
        for binding in ["{{.SyncerKey}}", "{{.BidderName}}"] {
            let host = UserSyncConfig {
                external_url: "https://sync.example.com/base/".to_owned(),
                redirect_url: Some(format!(
                    "{{{{.ExternalURL}}}}/setuid?%62idder={binding}&uid={{{{.UserMacro}}}}"
                )),
                ..UserSyncConfig::default()
            };
            let endpoint = SyncerEndpoint {
                url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
                user_macro: Some("#PMUID".to_owned()),
            };
            let built = build_sync_url(SyncTemplateInput {
                host: &host,
                endpoint: &endpoint,
                callback_bidder: "fake_bbb",
                bidder_name: "fake_bbb",
                sync_type: SyncType::Redirect,
                account: Some("publisher & account"),
                privacy: empty_privacy(),
            })
            .expect("one bound bidder and terminal real UID macro");
            assert_eq!(
                built.callback_url,
                "https://sync.example.com/base/setuid?%62idder=fake_bbb&account=publisher+%26+account&uid=#PMUID"
            );
            let bidder_url = url::Url::parse(&built.url).unwrap();
            assert!(bidder_url.fragment().is_none());
            assert_eq!(
                bidder_url
                    .query_pairs()
                    .find(|(key, _)| key == "back")
                    .unwrap()
                    .1,
                built.callback_url
            );
            let substituted_callback = built.callback_url.replace("#PMUID", "fake_bbb-id");
            let callback = url::Url::parse(&substituted_callback).unwrap();
            assert_eq!(callback.path(), "/base/setuid");
            assert!(callback.fragment().is_none());
            assert_eq!(
                callback
                    .query_pairs()
                    .find(|(key, _)| key == "bidder")
                    .unwrap()
                    .1,
                "fake_bbb"
            );
            assert_eq!(
                callback
                    .query_pairs()
                    .find(|(key, _)| key == "uid")
                    .unwrap()
                    .1,
                "fake_bbb-id"
            );
        }
    }

    #[test]
    fn private_literal_sync_authority_is_rejected_even_with_https() {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://10.0.0.1/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("$UID".to_owned()),
        };
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bidder",
            bidder_name: "fake_bidder",
            sync_type: SyncType::Redirect,
            account: None,
            privacy: empty_privacy(),
        });
        assert!(matches!(result, Err(TemplateError::InvalidEndpoint)));
    }

    #[test]
    fn append_only_callback_requires_terminal_empty_uid() {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            redirect_url: Some("{{.ExternalURL}}/setuid?uid={{.UserMacro}}&f=i".to_owned()),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: None,
        };
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bidder",
            bidder_name: "fake_bidder",
            sync_type: SyncType::Iframe,
            account: None,
            privacy: empty_privacy(),
        });
        // Host callback validation rejects this before URL rendering.
        assert!(matches!(result, Err(TemplateError::InvalidConfig)));
    }
    #[test]
    fn replacement_callback_rejects_hardcoded_uid_value() {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            redirect_url: Some("{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&uid=$UID".to_owned()),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("$UID".to_owned()),
        };
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bidder",
            bidder_name: "fake_bidder",
            sync_type: SyncType::Redirect,
            account: None,
            privacy: empty_privacy(),
        });
        assert!(matches!(result, Err(TemplateError::InvalidConfig)));
    }

    #[test]
    fn config_rejects_encoded_duplicate_uid_before_callback_rendering() {
        let host = UserSyncConfig {
            external_url: "https://sync.example.com".to_owned(),
            redirect_url: Some(
                "{{.ExternalURL}}/setuid?bidder={{.SyncerKey}}&%75id=forged&uid={{.UserMacro}}"
                    .to_owned(),
            ),
            ..UserSyncConfig::default()
        };
        let endpoint = SyncerEndpoint {
            url: "https://bidder.example.com/sync?back={{.RedirectURL}}".to_owned(),
            user_macro: Some("$UID".to_owned()),
        };
        assert_eq!(
            validate_user_sync(&host, false),
            Err("user_sync.redirect_url must end in one uid macro field".to_owned()),
        );
        let result = build_sync_url(SyncTemplateInput {
            host: &host,
            endpoint: &endpoint,
            callback_bidder: "fake_bidder",
            bidder_name: "fake_bidder",
            sync_type: SyncType::Redirect,
            account: None,
            privacy: empty_privacy(),
        });
        assert_eq!(result.err(), Some(TemplateError::InvalidConfig));
    }
}
