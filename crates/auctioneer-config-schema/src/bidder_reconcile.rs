//! Compare complete metadata and factory declarations before feature filtering.

use crate::types::BidderMetadataDeclaration;
use std::collections::BTreeMap;

/// Reconcile all metadata/factory associations and explicit config-feature forwarding.
///
/// # Errors
/// Rejects missing factories, mismatched feature ownership, and missing forwarding edges.
pub fn validate<'a, I>(
    metadata: &[BidderMetadataDeclaration],
    factories: I,
    features: &BTreeMap<String, Vec<String>>,
) -> Result<(), String>
where
    I: IntoIterator<Item = (&'a str, Option<&'a str>)>,
{
    let factory_features: BTreeMap<_, _> = factories.into_iter().collect();
    for row in metadata {
        let Some(factory_feature) = factory_features.get(row.implementation) else {
            return Err(format!(
                "metadata implementation has no factory: {}",
                row.bidder
            ));
        };
        if *factory_feature != row.required_feature {
            return Err(format!("metadata/factory feature mismatch: {}", row.bidder));
        }
        if let Some(feature) = row.required_feature {
            let forwarding = format!("auctioneer-config/{feature}");
            if features
                .get(feature)
                .is_none_or(|edges| !edges.contains(&forwarding))
            {
                return Err(format!("missing config feature forwarding: {feature}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::types::BidderMetadataDeclaration;
    use std::collections::BTreeMap;

    fn metadata(feature: Option<&'static str>) -> [BidderMetadataDeclaration; 1] {
        [BidderMetadataDeclaration {
            bidder: "fake_fixtureAds",
            implementation: "fake_fixture_ads",
            required_feature: feature,
        }]
    }

    fn features() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([(
            "optional-bidder".into(),
            vec!["auctioneer-config/optional-bidder".into()],
        )])
    }

    #[test]
    fn same_feature_and_ungated_bindings_are_valid() {
        for feature in [None, Some("optional-bidder")] {
            assert_eq!(
                validate(
                    &metadata(feature),
                    [("fake_fixture_ads", feature)],
                    &features()
                ),
                Ok(())
            );
        }
    }

    #[test]
    fn missing_factory_is_rejected_even_for_inactive_metadata() {
        for feature in [None, Some("optional-bidder")] {
            for factories in [vec![], vec![("fake_unrelated", feature)]] {
                assert_eq!(
                    validate(&metadata(feature), factories, &features()),
                    Err("metadata implementation has no factory: fake_fixtureAds".into())
                );
            }
        }
    }

    #[test]
    fn each_feature_mismatch_direction_is_rejected_before_filtering() {
        for (metadata_feature, factory_feature) in [
            (None, Some("optional-bidder")),
            (Some("optional-bidder"), None),
            (Some("optional-bidder"), Some("different-feature")),
        ] {
            assert_eq!(
                validate(
                    &metadata(metadata_feature),
                    [("fake_fixture_ads", factory_feature)],
                    &features(),
                ),
                Err("metadata/factory feature mismatch: fake_fixtureAds".into())
            );
        }
    }

    #[test]
    fn same_feature_requires_an_explicit_config_forwarding_edge() {
        for edges in [
            vec![],
            vec!["auctioneer-config/different-feature".into()],
            vec!["auctioneer-config?/optional-bidder".into()],
            vec!["different-package/optional-bidder".into()],
            vec!["another-local-feature".into()],
        ] {
            assert_eq!(
                validate(
                    &metadata(Some("optional-bidder")),
                    [("fake_fixture_ads", Some("optional-bidder"))],
                    &BTreeMap::from([("optional-bidder".into(), edges)]),
                ),
                Err("missing config feature forwarding: optional-bidder".into())
            );
        }
        assert_eq!(
            validate(
                &metadata(Some("optional-bidder")),
                [("fake_fixture_ads", Some("optional-bidder"))],
                &BTreeMap::new(),
            ),
            Err("missing config feature forwarding: optional-bidder".into())
        );
    }

    #[test]
    fn multiple_public_bases_can_share_a_factory_without_sharing_names() {
        let mut rows = metadata(Some("optional-bidder")).to_vec();
        rows.push(BidderMetadataDeclaration {
            bidder: "fake_independent_base",
            implementation: "fake_fixture_ads",
            required_feature: Some("optional-bidder"),
        });
        assert_eq!(
            validate(
                &rows,
                [
                    ("fake_unused_factory", None),
                    ("fake_fixture_ads", Some("optional-bidder"))
                ],
                &features(),
            ),
            Ok(())
        );
        rows[1].required_feature = None;
        assert_eq!(
            validate(
                &rows,
                [("fake_fixture_ads", Some("optional-bidder"))],
                &features(),
            ),
            Err("metadata/factory feature mismatch: fake_independent_base".into())
        );
    }

    #[test]
    fn fake_unrelated_features_and_unused_factories_do_not_require_metadata() {
        assert_eq!(
            validate(
                &[],
                [("fake_unused_factory", Some("not-promoted"))],
                &BTreeMap::new(),
            ),
            Ok(())
        );
        assert_eq!(
            validate(
                &metadata(None),
                [("fake_fixture_ads", None)],
                &BTreeMap::new()
            ),
            Ok(())
        );
    }
}
