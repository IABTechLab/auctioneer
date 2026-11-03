//! Test-only local browser configuration through the existing envelope builder.

use std::env;

use anyhow::{Context as _, Result, ensure};
use auctioneer_config_data::{
    types::BidderInfo,
    usersync::{SyncResponseFormat, SyncType, SyncerEndpoint},
};
use auctioneer_core::config::{AuctioneerAppConfig, validate_effective_defaults};
use edgezero_core::blob_envelope::BlobEnvelope;
use serde_json::{Value, json};
use url::Url;

const FIXTURE_BIDDER: &str = "fake_browser";

fn browser_syncer(bidders: &[BidderInfo]) -> Result<(&BidderInfo, &SyncerEndpoint)> {
    bidders
        .iter()
        .filter(|info| {
            let sync = &info.user_sync;
            !info.disabled
                && sync.enabled
                && sync.iframe.is_none()
                && !sync.skip_when.gdpr
                && sync.skip_when.gpp_sid.is_empty()
                && (sync.supports.is_empty() || sync.supports.contains(&SyncType::Redirect))
                && sync
                    .key
                    .as_deref()
                    .is_some_and(|key| !key.is_empty() && key != info.name && key != FIXTURE_BIDDER)
                && sync
                    .default_response
                    .is_none_or(|format| format == SyncResponseFormat::Image)
        })
        .filter_map(|info| Some((info, info.user_sync.redirect.as_ref()?)))
        .filter(|(_, endpoint)| {
            endpoint
                .user_macro
                .as_deref()
                .is_some_and(|value| !value.is_empty())
                && endpoint
                    .url
                    .strip_suffix("?{{.RedirectURL}}")
                    .is_some_and(|prefix| !prefix.contains("{{"))
        })
        .min_by_key(|(info, _)| &info.name)
        .context(
            "published metadata lacks a redirect-only syncer with a distinct family and UID macro",
        )
}

fn browser_bundle(publisher: &str, sync: &str) -> Result<Value> {
    let (info, endpoint) = browser_syncer(auctioneer_config_data::bidders())?;
    let url = Url::parse(&endpoint.url).context("parse selected sync metadata")?;
    let config: AuctioneerAppConfig = serde_json::from_value(json!({
        "user_sync": {
            "external_url": sync,
            "allowed_origins": [publisher],
        },
        "aliases": {(FIXTURE_BIDDER): info.name},
        "accounts": {"publisher": {}},
    }))
    .context("construct local browser config")?;
    validate_effective_defaults(&config).map_err(anyhow::Error::msg)?;
    let envelope = BlobEnvelope::new(
        serde_json::to_value(config)?,
        "1970-01-01T00:00:00Z".to_owned(),
    );
    Ok(json!({
        "envelope": envelope,
        "fixture": {
            "bidder": FIXTURE_BIDDER,
            "family": info.user_sync.key,
            "endpoint_host": url.host_str().context("sync metadata requires a host")?,
            "endpoint_origin": url.origin().ascii_serialization(),
            "endpoint_path": url.path(),
            "endpoint_template": endpoint.url,
            "user_macro": endpoint.user_macro,
            "sync_type": SyncType::Redirect,
        },
    }))
}

#[expect(
    clippy::print_stdout,
    reason = "this fixture command emits its envelope and metadata descriptor"
)]
fn main() -> Result<()> {
    ensure!(
        cfg!(feature = "insecure-bidders"),
        "build the local browser fixture with --features insecure-bidders"
    );
    let args: Vec<String> = env::args().skip(1).collect();
    let [publisher, sync] = args.as_slice() else {
        anyhow::bail!("expected publisher origin and sync origin");
    };
    println!(
        "{}",
        serde_json::to_string(&browser_bundle(publisher, sync)?)?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use auctioneer_config_data::{
        types::BidderInfo,
        usersync::{SyncResponseFormat, SyncType, SyncerConfig, SyncerEndpoint},
    };
    use serde_json::Value;

    fn metadata() -> BidderInfo {
        BidderInfo {
            name: "fake_source".to_owned(),
            user_sync: SyncerConfig {
                key: Some("fake_family".to_owned()),
                redirect: Some(SyncerEndpoint {
                    url: "https://fake-sync.invalid/redirect?{{.RedirectURL}}".to_owned(),
                    user_macro: Some("$FAKE_UID".to_owned()),
                }),
                ..SyncerConfig::default()
            },
            ..BidderInfo::default()
        }
    }

    #[test]
    fn browser_role_is_selected_by_properties_without_an_identity_selector() {
        let mut first = metadata();
        first.name = "fake_first".to_owned();
        let mut last = first.clone();
        last.name = "fake_last".to_owned();
        let rows = [last.clone(), first];
        assert_eq!(browser_syncer(&rows).unwrap().0.name, "fake_first");
        for mutation in 0_u8..14 {
            let mut row = metadata();
            match mutation {
                0 => row.disabled = true,
                1 => row.user_sync.enabled = false,
                2 => row.user_sync.iframe = row.user_sync.redirect.clone(),
                3 => row.user_sync.redirect = None,
                4 => row.user_sync.supports = vec![SyncType::Iframe],
                5 => row.user_sync.skip_when.gdpr = true,
                6 => row.user_sync.skip_when.gpp_sid = vec!["fake_section".to_owned()],
                7 => row.user_sync.key = Some(row.name.clone()),
                8 => row.user_sync.redirect.as_mut().unwrap().user_macro = None,
                9 => row.user_sync.default_response = Some(SyncResponseFormat::Blank),
                10 => row.user_sync.key = None,
                11 => row.user_sync.key = Some(String::new()),
                12 => row.user_sync.key = Some(FIXTURE_BIDDER.to_owned()),
                _ => row.user_sync.redirect.as_mut().unwrap().user_macro = Some(String::new()),
            }
            assert!(
                browser_syncer(&[row.clone()]).is_err(),
                "mutation {mutation}"
            );
            let rows = [row, last.clone()];
            assert_eq!(browser_syncer(&rows).unwrap().0.name, last.name);
        }
        let mut named_callback = metadata();
        named_callback.user_sync.redirect.as_mut().unwrap().url =
            "https://fake-sync.invalid/redirect?callback={{.RedirectURL}}".to_owned();
        assert!(browser_syncer(&[named_callback]).is_err());
        assert!(browser_syncer(&[]).is_err());
    }

    #[test]
    fn bundle_uses_a_fixture_alias_and_exact_published_metadata_without_rewriting_it() {
        let before = serde_json::to_value(auctioneer_config_data::bidders()).unwrap();
        let bundle = browser_bundle("http://127.0.0.1:42001", "http://127.0.0.1:42002").unwrap();
        let (info, endpoint) = browser_syncer(auctioneer_config_data::bidders()).unwrap();
        let url = Url::parse(&endpoint.url).unwrap();
        assert_eq!(
            bundle["fixture"],
            json!({
                "bidder": "fake_browser",
                "family": info.user_sync.key.as_deref().unwrap(),
                "endpoint_host": url.host_str().unwrap(),
                "endpoint_origin": url.origin().ascii_serialization(),
                "endpoint_path": url.path(),
                "endpoint_template": endpoint.url,
                "user_macro": endpoint.user_macro.as_deref().unwrap(),
                "sync_type": "redirect",
            })
        );
        let envelope: BlobEnvelope = serde_json::from_value(bundle["envelope"].clone()).unwrap();
        envelope.verify().unwrap();
        let config: AuctioneerAppConfig = serde_json::from_value(envelope.data).unwrap();
        validate_effective_defaults(&config).unwrap();
        assert_eq!(config.aliases.as_ref().unwrap().len(), 1);
        assert_eq!(config.aliases.as_ref().unwrap()["fake_browser"], info.name);
        let aliases = &bundle["envelope"]["data"]["aliases"];
        assert_eq!(aliases, &json!({"fake_browser": info.name}));
        assert_eq!(
            bundle["envelope"]["data"]["user_sync"]["external_url"],
            "http://127.0.0.1:42002"
        );
        assert_eq!(
            serde_json::to_value(auctioneer_config_data::bidders()).unwrap(),
            before
        );
        assert_eq!(
            bundle
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["envelope", "fixture"]
        );
        assert_ne!(
            bundle["fixture"]["family"],
            Value::String("fake_browser".to_owned())
        );
    }
}
