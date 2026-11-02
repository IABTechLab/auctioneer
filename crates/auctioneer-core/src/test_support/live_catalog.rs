//! Roles for tests that dispatch the real App or linked serving binary.
//! This reads generated metadata only; it never installs a replacement registry.

use auctioneer_config::{
    types::{BidderInfo, BidderMetadataDeclaration},
    usersync::{SyncResponseFormat, SyncType, SyncerEndpoint},
};
use url::{Url, form_urlencoded};

#[derive(Clone, Copy)]
pub struct SyncRole {
    pub info: &'static BidderInfo,
    pub endpoint: &'static SyncerEndpoint,
    pub kind: SyncType,
}

impl SyncRole {
    #[must_use]
    pub fn name(self) -> &'static str {
        &self.info.name
    }

    #[must_use]
    pub fn family(self) -> &'static str {
        self.info.user_sync.key.as_deref().unwrap_or(self.name())
    }

    #[must_use]
    pub fn user_macro(self) -> &'static str {
        self.endpoint.user_macro.as_deref().unwrap_or_default()
    }

    #[must_use]
    pub fn setuid(self, query: &str) -> String {
        format!("/setuid?bidder={}&{query}", self.name())
    }

    /// Decode the callback's declared query slot, independently of the renderer.
    /// Compare the outer destination before following a response-provided URL.
    ///
    /// # Panics
    /// Panics for an invalid URL, unexpected destination, or missing/ambiguous callback.
    #[must_use]
    pub fn callback(self, rendered: &str) -> Url {
        let outer = Url::parse(rendered).expect("emitted sync URL");
        let template = Url::parse(&self.endpoint.url).expect("metadata sync URL");
        assert_eq!(
            outer.origin(),
            template.origin(),
            "sync authority must match metadata"
        );
        assert_eq!(
            outer.path(),
            template.path(),
            "sync path must match metadata"
        );
        assert!(
            outer.fragment().is_none(),
            "UID macro must not become a fragment"
        );
        let callback = if self.endpoint.url.split_once('?').map(|(_, query)| query)
            == Some("{{.RedirectURL}}")
        {
            form_urlencoded::parse(
                format!("callback={}", outer.query().expect("callback query")).as_bytes(),
            )
            .next()
            .expect("one encoded callback")
            .1
            .into_owned()
        } else {
            let slot = template
                .query_pairs()
                .find(|(_, value)| value == "{{.RedirectURL}}")
                .expect("metadata declares a complete callback query value")
                .0;
            let values: Vec<_> = outer
                .query_pairs()
                .filter(|(key, _)| key == &slot)
                .collect();
            assert_eq!(values.len(), 1, "exactly one callback query slot");
            values
                .into_iter()
                .next()
                .expect("one callback slot")
                .1
                .to_string()
        };
        let callback_url = Url::parse(&callback).expect("decoded callback URL");
        assert_eq!(
            callback_url.path(),
            "/setuid",
            "callback must target the UID handler"
        );
        callback_url
    }

    /// Substitute the captured UID macro and return the handler callback target.
    ///
    /// # Panics
    /// Panics if the callback is invalid or does not preserve its UID slot.
    #[must_use]
    pub fn callback_uri(self, rendered: &str, uid: &str) -> String {
        let callback = self.callback(rendered);
        let template = callback.as_str();
        let completed = if self.user_macro().is_empty() {
            assert!(
                template.ends_with("&uid="),
                "append-UID callback: {template}"
            );
            format!("{template}{uid}")
        } else {
            assert!(
                template.contains(self.user_macro()),
                "callback must retain UID macro"
            );
            template.replace(self.user_macro(), uid)
        };
        let completed_url = Url::parse(&completed).expect("completed callback");
        assert!(
            completed_url.fragment().is_none(),
            "UID must stay in the query"
        );
        format!(
            "{}?{}",
            completed_url.path(),
            completed_url.query().expect("completed callback query")
        )
    }
}

#[expect(
    clippy::panic,
    reason = "a missing live-catalog coverage role must fail the test"
)]
fn select_sync(obligation: &str, kind: SyncType, predicate: impl Fn(SyncRole) -> bool) -> SyncRole {
    auctioneer_config::bidders()
        .iter()
        .filter(|info| {
            !info.disabled
                && info.user_sync.enabled
                && !info.user_sync.skip_when.gdpr
                && info.user_sync.skip_when.gpp_sid.is_empty()
                && (info.user_sync.supports.is_empty() || info.user_sync.supports.contains(&kind))
        })
        .filter_map(|info| {
            let endpoint = match kind {
                SyncType::Iframe => info.user_sync.iframe.as_ref(),
                SyncType::Redirect => info.user_sync.redirect.as_ref(),
            }?;
            Some(SyncRole {
                info,
                endpoint,
                kind,
            })
        })
        .filter(|role| predicate(*role))
        .min_by_key(|role| role.name())
        .unwrap_or_else(|| panic!("live catalog lacks required usersync coverage: {obligation}"))
}

/// Redirect-only macro substitution, distinct storage family, image callback,
/// and no endpoint-level consent expansion (the negative URL-cap control).
///
/// # Panics
/// Panics if the serving catalog has no matching redirect role.
#[must_use]
pub fn redirect() -> SyncRole {
    select_sync(
        "redirect-only UID macro with distinct family, image response and TCF vendor",
        SyncType::Redirect,
        |role| {
            role.info.user_sync.iframe.is_none()
                && role.family() != role.name()
                && role
                    .info
                    .gvl_vendor_id
                    .is_some_and(|vendor| vendor > 0 && u16::try_from(vendor).is_ok())
                && !role.user_macro().is_empty()
                && role.endpoint.url.ends_with("?{{.RedirectURL}}")
                && !role.endpoint.url.contains("{{.GDPRConsent}}")
                && role
                    .info
                    .user_sync
                    .default_response
                    .is_none_or(|format| format == SyncResponseFormat::Image)
        },
    )
}

/// Default iframe with partner-appended UID, named callback query value,
/// endpoint-level consent expansion, and an independent storage family.
///
/// # Panics
/// Panics if the serving catalog lacks either required independent sync family.
#[must_use]
pub fn iframe() -> SyncRole {
    let other_family = redirect().family();
    select_sync(
        "iframe append-UID callback with consent expansion and blank response",
        SyncType::Iframe,
        |role| {
            role.user_macro().is_empty()
                && role.family() != other_family
                && role.endpoint.url.contains("={{.RedirectURL}}")
                && role.endpoint.url.contains("{{.GDPRConsent}}")
                && role
                    .info
                    .user_sync
                    .default_response
                    .is_none_or(|format| format == SyncResponseFormat::Blank)
        },
    )
}

/// Select a live identity without sync endpoints for negative coverage.
///
/// # Panics
/// Panics if every live identity has a sync endpoint.
#[must_use]
pub fn unsyncable() -> &'static BidderInfo {
    auctioneer_config::bidders()
        .iter()
        .filter(|info| {
            !info.disabled && info.user_sync.iframe.is_none() && info.user_sync.redirect.is_none()
        })
        .min_by_key(|info| &info.name)
        .expect("live catalog requires an unsyncable identity for negative endpoint coverage")
}

/// Collect the generated declarations requiring a feature.
///
/// # Panics
/// Panics if the feature has no declarations, preventing vacuous membership coverage.
#[must_use]
pub fn feature_declarations(feature: &str) -> Vec<&'static BidderMetadataDeclaration> {
    let rows: Vec<_> = auctioneer_config::bidder_metadata_declarations()
        .iter()
        .filter(|row| row.required_feature == Some(feature))
        .collect();
    assert!(
        !rows.is_empty(),
        "feature-membership coverage requires declarations for {feature}"
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::SyncRole;
    use auctioneer_config::{
        types::BidderInfo,
        usersync::{SyncType, SyncerEndpoint},
    };
    use url::{Url, form_urlencoded};

    fn role(url: &str, user_macro: &str) -> SyncRole {
        SyncRole {
            info: Box::leak(Box::new(BidderInfo {
                name: "fake_fixture_bidder".to_owned(),
                ..BidderInfo::default()
            })),
            endpoint: Box::leak(Box::new(SyncerEndpoint {
                url: url.to_owned(),
                user_macro: Some(user_macro.to_owned()),
            })),
            kind: SyncType::Redirect,
        }
    }

    #[test]
    fn callback_extraction_handles_bare_query_and_named_query_value() {
        let callback = "https://callback.example/setuid?bidder=fake_fixture_bidder&uid=$PARTNER";
        let bare = role(
            "https://sync.partner.example/get?{{.RedirectURL}}",
            "$PARTNER",
        );
        let encoded: String = form_urlencoded::byte_serialize(callback.as_bytes()).collect();
        assert_eq!(
            bare.callback_uri(
                &format!("https://sync.partner.example/get?{encoded}"),
                "partner-user"
            ),
            "/setuid?bidder=fake_fixture_bidder&uid=partner-user"
        );

        let named = role(
            "https://sync.partner.example/get?back={{.RedirectURL}}&mode=1",
            "",
        );
        let mut outer = Url::parse("https://sync.partner.example/get?mode=1").unwrap();
        outer.query_pairs_mut().append_pair(
            "back",
            "https://callback.example/setuid?bidder=fake_fixture_bidder&uid=",
        );
        assert_eq!(
            named.callback_uri(outer.as_str(), "appended-user"),
            "/setuid?bidder=fake_fixture_bidder&uid=appended-user"
        );
    }

    #[test]
    #[should_panic(expected = "assertion `left == right` failed")]
    fn callback_extraction_rejects_a_changed_partner_destination() {
        let selected = role(
            "https://sync.partner.example/get?{{.RedirectURL}}",
            "$PARTNER",
        );
        drop(selected.callback(
            "https://different.partner.example/get?https%3A%2F%2Fcallback.example%2Fsetuid",
        ));
    }
}
