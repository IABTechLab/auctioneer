//! Smoke tests for `auctioneer-config` codegen output.
//!
//! Actual baked properties are selected from independently read source definitions.
//! Feature-OFF asserts HTTPS-only metadata; feature-ON retains loopback HTTP evidence.

#[cfg(test)]
mod tests {
    use auctioneer_config::types::{BidderInfo, Compression, MediaType};
    use auctioneer_config::{account_default, bidder, bidders, eea_set, host_default};
    use std::collections::BTreeMap;
    use std::error::Error;
    use std::fs;
    use std::path::Path;

    fn source_definitions() -> Result<BTreeMap<String, toml::Value>, Box<dyn Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .with_file_name("auctioneer-bidder")
            .join("src/bidders");
        let mut definitions = BTreeMap::new();
        for directory in fs::read_dir(root)? {
            let path = directory?.path().join("bidder.toml");
            if !path.is_file() {
                continue;
            }
            let manifest: toml::Table = toml::from_str(&fs::read_to_string(path)?)?;
            match manifest
                .get("required_feature")
                .and_then(toml::Value::as_str)
            {
                None => {}
                Some("insecure-bidders") if cfg!(feature = "insecure-bidders") => {}
                Some("insecure-bidders") => continue,
                Some(_) => return Err("add explicit test feature selection".into()),
            }
            if let Some(rows) = manifest.get("definitions").and_then(toml::Value::as_table) {
                for (name, definition) in rows {
                    if definitions
                        .insert(name.clone(), definition.clone())
                        .is_some()
                    {
                        return Err("duplicate source bidder definition".into());
                    }
                }
            }
        }
        Ok(definitions)
    }

    fn property<'a>(definition: &'a toml::Value, path: &[&str]) -> Option<&'a toml::Value> {
        path.iter()
            .try_fold(definition, |value, key| value.get(*key))
    }

    fn source_bidders_with(
        predicate: impl Fn(&toml::Value) -> bool,
    ) -> Result<Vec<(&'static BidderInfo, toml::Value)>, Box<dyn Error>> {
        let selected: Vec<_> = source_definitions()?
            .into_iter()
            .filter(|(_, definition)| predicate(definition))
            .map(|(name, definition)| {
                Ok((
                    bidder(&name).ok_or("source bidder missing from baked metadata")?,
                    definition,
                ))
            })
            .collect::<Result<_, Box<dyn Error>>>()?;
        if selected.is_empty() {
            return Err("source must retain a row exercising the selected property".into());
        }
        Ok(selected)
    }

    #[test]
    fn account_default_disabled_is_false() {
        let account = account_default();
        assert!(!account.disabled);
    }

    #[test]
    fn eea_set_contains_germany() {
        assert!(eea_set().contains(&"DEU"));
    }

    #[test]
    fn baked_https_endpoint_matches_source() {
        let selected = source_bidders_with(|row| {
            row.get("endpoint")
                .and_then(toml::Value::as_str)
                .is_some_and(|endpoint| endpoint.starts_with("https://"))
        })
        .unwrap();
        for (bid, source) in selected {
            assert!(bid.uses_https);
            assert_eq!(Some(bid.endpoint.as_str()), source["endpoint"].as_str());
        }
    }

    #[test]
    fn host_default_has_required_gdpr_value() {
        let host = host_default();
        assert!(host.gdpr.default_value == "0" || host.gdpr.default_value == "1");
    }

    #[test]
    fn host_default_status_response_empty_by_default() {
        // The default is empty, which makes GET /status emit 204 No
        // Content. A non-empty operator override would flip to 200
        // with the configured body.
        let host = host_default();
        assert!(host.status_response.is_empty());
    }

    #[test]
    fn host_default_resource_limits_carries_tmax() {
        let host = host_default();
        assert!(host.resource_limits.default_tmax_ms > 0);
    }

    #[test]
    fn baked_capabilities_match_source_and_cover_site_and_app() {
        let mut site_banner = false;
        let mut app_video = false;
        for (name, source) in source_definitions().unwrap() {
            let bid = bidder(&name).unwrap();
            for (channel, actual) in [
                ("site", &bid.capabilities.site),
                ("app", &bid.capabilities.app),
                ("dooh", &bid.capabilities.dooh),
            ] {
                let expected: Vec<MediaType> =
                    property(&source, &["capabilities", channel, "mediaTypes"])
                        .cloned()
                        .unwrap_or_else(|| toml::Value::Array(Vec::new()))
                        .try_into()
                        .unwrap();
                assert_eq!(actual, &expected, "{name}/{channel}");
            }
            site_banner |= bid.capabilities.site.contains(&MediaType::Banner);
            app_video |= bid.capabilities.app.contains(&MediaType::Video);
        }
        assert!(site_banner);
        assert!(app_video);
    }

    #[test]
    fn baked_openrtb_gpp_support_from_toml() {
        let selected = source_bidders_with(|row| {
            property(row, &["openrtb", "gpp-supported"]).and_then(toml::Value::as_bool)
                == Some(true)
        })
        .unwrap();
        for (bid, _) in selected {
            assert!(bid.openrtb.gpp_supported);
        }
    }

    #[test]
    fn baked_gzip_compression_from_toml() {
        let selected = source_bidders_with(|row| {
            row.get("endpointCompression").and_then(toml::Value::as_str) == Some("gzip")
        })
        .unwrap();
        for (bid, _) in selected {
            assert_eq!(bid.endpoint_compression, Some(Compression::Gzip));
        }
    }

    #[test]
    fn baked_https_syncer_key_and_redirect_match_source() {
        let selected = source_bidders_with(|row| {
            property(row, &["userSync", "key"]).is_some()
                && property(row, &["userSync", "redirect"]).is_some()
                && row
                    .get("endpoint")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|endpoint| endpoint.starts_with("https://"))
        })
        .unwrap();
        for (bid, source) in selected {
            assert!(bid.uses_https);
            assert_eq!(
                bid.user_sync.key.as_deref(),
                property(&source, &["userSync", "key"]).and_then(toml::Value::as_str)
            );
            assert!(bid.user_sync.redirect.is_some());
            assert_eq!(
                bid.user_sync.redirect.as_ref().unwrap().url,
                property(&source, &["userSync", "redirect", "url"])
                    .and_then(toml::Value::as_str)
                    .unwrap()
            );
        }
    }

    #[test]
    fn baked_gvl_vendor_ids_and_missing_ids_match_source() {
        let mut with_id = false;
        let mut without_id = false;
        for (name, source) in source_definitions().unwrap() {
            let expected = source
                .get("gvlVendorID")
                .map(|value| u32::try_from(value.as_integer().unwrap()).unwrap());
            assert_eq!(bidder(&name).unwrap().gvl_vendor_id, expected, "{name}");
            with_id |= expected.is_some();
            without_id |= expected.is_none();
        }
        assert!(with_id);
        assert!(without_id);
    }

    #[test]
    fn active_baked_names_equal_source_definitions() {
        let names: Vec<&str> = bidders().iter().map(|info| info.name.as_str()).collect();
        let source = source_definitions().unwrap();
        assert!(!source.is_empty());
        assert_eq!(names, source.keys().map(String::as_str).collect::<Vec<_>>());
    }

    #[test]
    #[cfg(not(feature = "insecure-bidders"))]
    fn feature_off_baked_endpoints_and_syncers_are_https_only() {
        assert!(!bidders().is_empty());
        for bid in bidders() {
            assert!(bid.uses_https, "{}", bid.name);
            assert!(bid.endpoint.starts_with("https://"), "{}", bid.name);
            for endpoint in [&bid.user_sync.iframe, &bid.user_sync.redirect]
                .into_iter()
                .flatten()
            {
                assert!(endpoint.url.starts_with("https://"), "{}", bid.name);
            }
        }
    }

    #[test]
    #[cfg(feature = "insecure-bidders")]
    fn baked_loopback_http_is_present_under_feature() {
        let selected = source_bidders_with(|row| {
            row.get("endpoint")
                .and_then(toml::Value::as_str)
                .is_some_and(|endpoint| endpoint.starts_with("http://127.0.0.1:"))
        })
        .unwrap();
        for (bid, source) in selected {
            assert!(!bid.uses_https);
            assert!(bid.endpoint.contains("127.0.0.1"));
            assert_eq!(Some(bid.endpoint.as_str()), source["endpoint"].as_str());
        }
    }

    // ---- account targeting + host aliases ----------------------------

    #[test]
    fn account_default_targeting_prefix_is_hb() {
        // Phase-1 TOML pins `targeting_prefix: hb`; the auction path
        // reads this when the request omits
        // `ext.prebid.targeting.prefix`.
        let account = account_default();
        assert_eq!(account.targeting_prefix, "hb");
    }

    #[test]
    fn account_default_truncate_target_attribute_is_none_in_phase_1() {
        // Phase-1 TOML leaves the cap unset so `build_keys` uses the
        // default (`MaxKeyLength = 20`).
        let account = account_default();
        assert!(account.truncate_target_attribute.is_none());
    }

    #[test]
    fn host_defaults_aliases_is_empty_in_phase_1() {
        // Host operators declare aliases in `[baked.host.aliases]`;
        // phase 1 ships an empty map so `/info/bidders` never
        // synthesizes surprise entries out of the box.
        let host = host_default();
        assert!(host.aliases.is_empty());
    }
}
