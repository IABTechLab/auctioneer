//! Published serving bindings and their bidder-owned factory inputs for real-data tests.

use super::privacy_wire::PrivacyWireContract;
use crate::Bidder;
use auctioneer_config_data::types::{BidderInfo, MediaType};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeSet, fs, path::Path, sync::OnceLock};

#[derive(Clone)]
pub struct ServingBidder {
    pub info: &'static BidderInfo,
    pub implementation: &'static str,
    pub factory: fn() -> Box<dyn Bidder>,
    pub params: Value,
    pub owner_request: Value,
    pub owner_expected_request_body: Option<Value>,
}

impl ServingBidder {
    #[must_use]
    pub fn name(&self) -> &'static str {
        &self.info.name
    }

    /// Load independent facts for the full privacy-context wire fixture.
    ///
    /// # Errors
    /// Refuses missing, escaping, malformed, or incorrectly owned facts.
    pub fn privacy_wire_contract(&self) -> Result<PrivacyWireContract, String> {
        let bytes = owned_capture(self.implementation, "privacy.json")?;
        PrivacyWireContract::parse(&bytes, self.implementation)
    }
}

#[derive(Deserialize)]
struct FactoryFixture {
    implementation: String,
    cases: Vec<FactoryCase>,
}

#[derive(Deserialize)]
struct FactoryCase {
    request: Value,
    outbound: Vec<FactoryOutbound>,
}

#[derive(Deserialize)]
struct FactoryOutbound {
    body: Value,
}

#[expect(
    clippy::unwrap_used,
    reason = "opt-in fixture setup must fail immediately for unreadable or malformed owner captures"
)]
fn owner_case(implementation: &str) -> FactoryCase {
    let fixture: FactoryFixture =
        serde_json::from_slice(&owned_capture(implementation, "factory.json").unwrap()).unwrap();
    assert_eq!(
        fixture.implementation, implementation,
        "factory fixture must identify its published implementation owner"
    );
    fixture
        .cases
        .into_iter()
        .next()
        .expect("owner fixture has a first request case")
}

fn owned_capture(implementation: &str, file: &str) -> Result<Vec<u8>, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/bidders")
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let owner = root
        .join(implementation)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if owner.parent() != Some(root.as_path()) {
        return Err("capture owner must be directly below the bidder source root".into());
    }
    let path = owner
        .join("fixtures")
        .join(file)
        .canonicalize()
        .map_err(|error| error.to_string())?;
    if !path.starts_with(&owner) {
        return Err("capture must stay below its implementation owner".into());
    }
    fs::read(path).map_err(|error| error.to_string())
}

/// Read the full published cohort and its independently owned request witnesses.
///
/// # Panics
/// Panics if metadata, factory bindings or owner fixtures are missing or inconsistent.
#[must_use]
pub fn published() -> Vec<ServingBidder> {
    let bindings = auctioneer_config_data::bidder_implementation_bindings();
    let rows: Vec<_> = bindings
        .iter()
        .map(|binding| {
            let info = auctioneer_config_data::bidder(binding.bidder)
                .expect("published binding has active metadata");
            let registration = crate::compiled_bidders()
                .iter()
                .find(|registration| registration.key == binding.implementation)
                .expect("published implementation has a compiled factory");
            let case = owner_case(binding.implementation);
            let params = case
                .request
                .pointer("/imp/0/ext/bidder")
                .filter(|value| value.is_object())
                .expect("owner first impression supplies bidder parameters")
                .clone();
            ServingBidder {
                info,
                implementation: binding.implementation,
                factory: registration.factory,
                params,
                owner_request: case.request,
                owner_expected_request_body: case.outbound.into_iter().next().map(|call| call.body),
            }
        })
        .collect();
    assert!(
        !rows.is_empty(),
        "published conformance cohort must not be empty"
    );
    let actual: BTreeSet<_> = rows.iter().map(ServingBidder::name).collect();
    let expected: BTreeSet<_> = auctioneer_config_data::bidders()
        .iter()
        .map(|info| info.name.as_str())
        .collect();
    assert_eq!(
        actual.len(),
        bindings.len(),
        "each public binding is covered once"
    );
    assert_eq!(
        actual, expected,
        "all active metadata must reach real conformance"
    );
    rows
}

/// Discover during setup, before timing or allocation baselines; retain only metadata.
///
/// # Panics
/// Panics if the published cohort has no suitable independently witnessed passthrough.
#[must_use]
pub fn passthrough_info() -> &'static BidderInfo {
    static INFO: OnceLock<&'static BidderInfo> = OnceLock::new();
    INFO.get_or_init(|| {
        published()
            .into_iter()
            .filter(|row| {
                let info = row.info;
                !info.disabled
                    && info.uses_https
                    && info.gvl_vendor_id.is_none()
                    && info.capabilities.site.contains(&MediaType::Banner)
                    // Retention tests inspect the endpoint and debug body directly.
                    && info.debug_allow
                    && info.endpoint.as_deref().is_some_and(|endpoint| !endpoint.contains("{{"))
                    && info.endpoint_compression.is_none()
                    && row.owner_expected_request_body.as_ref() == Some(&row.owner_request)
            })
            .min_by_key(|row| !default_active(row.info))
            .expect(
                "published catalog has a debug-enabled HTTPS banner passthrough without a GVL ID",
            )
            .info
    })
}

fn default_active(mut info: &BidderInfo) -> bool {
    for _ in 0..auctioneer_config_data::bidders().len() {
        if info.disabled || info.default_enabled == Some(false) {
            return false;
        }
        let Some(parent_name) = info.alias_of.as_deref() else {
            return true;
        };
        let Some(parent) = auctioneer_config_data::bidder(parent_name) else {
            return false;
        };
        info = parent;
    }
    false
}

/// Existing default-active publications for baseline handler/receipt controls.
/// Opt-in publications require their own generated-App acceptance;
/// [`published`] remains the complete catalog and is never filtered here.
///
/// # Panics
/// Panics if the complete catalog is inconsistent or the baseline is empty.
#[must_use]
pub fn default_active_publications() -> Vec<ServingBidder> {
    let rows: Vec<_> = published()
        .into_iter()
        .filter(|row| default_active(row.info))
        .collect();
    assert!(
        !rows.is_empty(),
        "default-active baseline must not disappear"
    );
    rows
}

/// Publications that require an explicit operator-configured endpoint.
/// Generated-App acceptance exercises this cohort separately from the frozen
/// default-active and native-schema cohorts, using each implementation's fixtures.
///
/// # Panics
/// Panics if catalog discovery fails or an endpoint-free publication is active by default.
#[must_use]
pub fn configured_endpoint_publications() -> Vec<ServingBidder> {
    published()
        .into_iter()
        .filter(|row| row.info.endpoint.is_none())
        .inspect(|row| {
            assert!(
                !row.info.disabled && !default_active(row.info),
                "endpoint-free publications require explicit activation"
            );
        })
        .collect()
}

/// Default-active counterpart of [`site_media`] for unfunded baseline tests.
///
/// # Panics
/// Panics if catalog discovery fails or no baseline publication supports the media.
#[must_use]
pub fn default_active_site_media(required: &[MediaType]) -> Vec<ServingBidder> {
    let rows: Vec<_> = default_active_publications()
        .into_iter()
        .filter(|row| {
            required
                .iter()
                .all(|media| row.info.capabilities.site.contains(media))
        })
        .collect();
    assert!(
        !rows.is_empty(),
        "default-active media baseline must not disappear"
    );
    rows
}

/// Select the entire published cohort supporting all required site media.
///
/// # Panics
/// Panics if the metadata or fixtures are inconsistent, or no definition supports the media.
#[must_use]
pub fn site_media(required: &[MediaType]) -> Vec<ServingBidder> {
    let rows: Vec<_> = published()
        .into_iter()
        .filter(|row| {
            required
                .iter()
                .all(|media| row.info.capabilities.site.contains(media))
        })
        .collect();
    assert!(
        !rows.is_empty(),
        "published site-media cohort must not be empty"
    );
    let actual: BTreeSet<_> = rows.iter().map(ServingBidder::name).collect();
    let expected: BTreeSet<_> = auctioneer_config_data::bidders()
        .iter()
        .filter(|info| {
            required
                .iter()
                .all(|media| info.capabilities.site.contains(media))
        })
        .map(|info| info.name.as_str())
        .collect();
    assert_eq!(
        actual, expected,
        "every capable published bidder must be exercised"
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::default_active;

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn baseline_native_and_configured_endpoint_cohorts_cover_each_publication_once() {
        use crate::schema::native_schema_required;
        use std::collections::BTreeSet;

        let rows = super::published();
        let baseline: BTreeSet<_> = super::default_active_publications()
            .into_iter()
            .map(|row| row.name())
            .collect();
        let configured: BTreeSet<_> = super::configured_endpoint_publications()
            .into_iter()
            .map(|row| row.name())
            .collect();
        let mut native = 0;
        for row in &rows {
            let controlled = native_schema_required(row.name());
            assert_eq!(
                usize::from(baseline.contains(row.name()))
                    + usize::from(controlled)
                    + usize::from(configured.contains(row.name())),
                1,
                "{} must have exactly one explicit acceptance cohort",
                row.name()
            );
            native += usize::from(controlled);
        }
        assert_eq!(native, 14, "13 new priority flows plus canonical ancestry");
        assert_eq!(
            configured.len(),
            1,
            "explicit configured-endpoint acceptance"
        );
        assert_eq!(baseline.len() + native + configured.len(), rows.len());
    }
    use auctioneer_config_data::types::BidderInfo;

    #[test]
    fn baseline_ranking_prefers_default_active_without_filtering_the_public_cohort() {
        let controlled = BidderInfo {
            name: "fake_controlled".into(),
            default_enabled: Some(false),
            ..BidderInfo::default()
        };
        let legacy = BidderInfo {
            name: "fake_legacy".into(),
            ..BidderInfo::default()
        };
        let public = [&controlled, &legacy];
        let selected = public
            .into_iter()
            .min_by_key(|info| !default_active(info))
            .unwrap();
        assert_eq!(selected.name, legacy.name);
        assert_eq!(public.len(), 2);
        let missing_base = BidderInfo {
            alias_of: Some("fake_absent_base".into()),
            ..legacy.clone()
        };
        assert!(!default_active(&missing_base));
        assert!(!default_active(&BidderInfo {
            disabled: true,
            ..legacy
        }));
    }
}
