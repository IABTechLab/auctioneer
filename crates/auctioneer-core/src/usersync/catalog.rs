//! Request-scoped syncer view over the compiled bidder registry.

use std::cmp::Ordering;

use auctioneer_bidder::BidderRegistry;
use auctioneer_config_data::types::{BidderInfo, HostConfig};
use auctioneer_config_data::usersync::validate_catalog_size;

use crate::{
    bidder_registry::registry_handle,
    config::{AuctioneerAppConfig, resolve::sync_bidder_enabled_in_catalog},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogError {
    Registry(auctioneer_bidder::RegistryBuildError),
    UnknownAliasBase,
    TooManyNames,
}

#[derive(Clone, Copy, Debug)]
struct Entry<'a> {
    catalog_name: &'a str,
    family: &'static str,
    bidder: &'static BidderInfo,
    configured: bool,
    enabled: bool,
}

impl<'a> Entry<'a> {
    fn new<F>(
        name: &'a str,
        bidder: &'static BidderInfo,
        app: &AuctioneerAppConfig,
        lookup: F,
    ) -> Self
    where
        F: Fn(&str) -> Option<&'static BidderInfo>,
    {
        let syncer = &bidder.user_sync;
        Self {
            catalog_name: name,
            family: syncer.key.as_deref().unwrap_or(&bidder.name),
            bidder,
            configured: syncer.iframe.is_some() || syncer.redirect.is_some(),
            enabled: sync_bidder_enabled_in_catalog(app, name, bidder, lookup),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Catalog<'a> {
    entries: Vec<Entry<'a>>,
}

#[derive(Clone, Debug)]
pub struct Candidate<'a> {
    pub requested_name: String,
    pub catalog_name: &'a str,
    pub normalized_name: String,
    pub base_name: &'a str,
    pub family: &'a str,
    pub bidder: &'static BidderInfo,
    pub configured: bool,
    pub enabled: bool,
}

fn folded_cmp(left: &str, right: &str) -> Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_lowercase()))
}

impl<'a> Catalog<'a> {
    #[cfg(test)]
    pub(crate) fn for_test_bidder(name: &'a str, bidder: BidderInfo) -> Self {
        let leaked_bidder = Box::leak(Box::new(bidder));
        let entry = Entry::new(name, leaked_bidder, &AuctioneerAppConfig::default(), |_| {
            None
        });
        let entries = vec![entry];
        Self { entries }
    }

    /// Build after the runtime loader has validated the merged alias namespace.
    /// An unknown alias base still fails closed if this is called directly.
    ///
    /// # Errors
    /// Returns a distinct `Registry` failure if initialization fails,
    /// `UnknownAliasBase` for an unregistered base, or `TooManyNames`
    /// before allocating an oversized catalog.
    pub fn new(host: &'a HostConfig, app: &AuctioneerAppConfig) -> Result<Self, CatalogError> {
        Self::from_registry(
            host,
            app,
            registry_handle().map_err(CatalogError::Registry)?,
        )
    }

    /// Build the same bounded view over an explicitly supplied registry.
    /// Metadata is borrowed; no factory is executed by this constructor.
    ///
    /// # Errors
    /// Returns the same catalog-size and alias-base errors as [`Self::new`].
    pub fn from_registry(
        host: &'a HostConfig,
        app: &AuctioneerAppConfig,
        registry: &BidderRegistry,
    ) -> Result<Self, CatalogError> {
        let capacity = registry
            .infos()
            .len()
            .checked_add(host.aliases.len())
            .ok_or(CatalogError::TooManyNames)?;
        validate_catalog_size(capacity, 0).map_err(|_error| CatalogError::TooManyNames)?;
        let mut entries = Vec::with_capacity(capacity);
        let lookup = |name: &str| registry.get(name).map(|(info, _)| info);
        for bidder in registry.infos() {
            entries.push(Entry::new(&bidder.name, bidder, app, lookup));
        }
        entries.sort_by(|left, right| folded_cmp(left.catalog_name, right.catalog_name));
        let compiled_len = entries.len();
        // Host aliases replace exact base names. They are never allowed to
        // silently fall back if their declared base is unavailable.
        for (alias, base) in &host.aliases {
            let (bidder, _) = registry.get(base).ok_or(CatalogError::UnknownAliasBase)?;
            let mut entry = Entry::new(alias, bidder, app, lookup);
            // The immutable sorted prefix retains compiled vetoes even when a
            // differently cased host alias replaces their public identity.
            let compiled = entries
                .get(..compiled_len)
                .ok_or(CatalogError::TooManyNames)?;
            let start = compiled.partition_point(|row| folded_cmp(row.catalog_name, alias).is_lt());
            entry.enabled &= compiled
                .iter()
                .skip(start)
                .take_while(|row| row.catalog_name.eq_ignore_ascii_case(alias))
                .all(|row| row.enabled);
            entries.push(entry);
        }
        // Stable ordering retains map-insertion precedence: later host aliases
        // replace earlier entries with the same ASCII-folded name.
        entries.sort_by(|left, right| folded_cmp(left.catalog_name, right.catalog_name));
        entries.dedup_by(|later, earlier| {
            if later
                .catalog_name
                .eq_ignore_ascii_case(earlier.catalog_name)
            {
                *earlier = *later;
                true
            } else {
                false
            }
        });
        Ok(Self { entries })
    }

    #[must_use]
    pub fn lookup(&self, requested: &str) -> Option<Candidate<'_>> {
        let entry = self.find_entry(requested)?;
        Some(Candidate {
            requested_name: requested.to_owned(),
            catalog_name: entry.catalog_name,
            normalized_name: requested.to_ascii_lowercase(),
            base_name: &entry.bidder.name,
            family: entry.family,
            bidder: entry.bidder,
            configured: entry.configured,
            enabled: entry.enabled,
        })
    }

    fn find_entry(&self, requested: &str) -> Option<&Entry<'a>> {
        let index = self
            .entries
            .binary_search_by(|entry| folded_cmp(entry.catalog_name, requested))
            .ok()?;
        self.entries.get(index)
    }

    pub(super) fn sync_enabled(&self, requested: &str) -> Option<bool> {
        self.find_entry(requested).map(|entry| entry.enabled)
    }

    /// Borrow configured identity/family pairs, including disabled syncers.
    pub fn configured_families(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .filter(|entry| entry.configured)
            .map(|entry| (entry.catalog_name, entry.family))
    }

    /// Resolve a configured family without copying either name.
    #[must_use]
    pub fn configured_family(&self, requested: &str) -> Option<&'static str> {
        self.find_entry(requested)
            .filter(|entry| entry.configured)
            .map(|entry| entry.family)
    }

    /// Configured names only; disabled entries remain available so chooser
    /// stage ordering and requested-name diagnostics can observe them.
    #[must_use]
    pub fn available_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self
            .configured_families()
            .map(|(name, _family)| name)
            .collect();
        names.sort_unstable();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::{Catalog, CatalogError, folded_cmp};
    use crate::bidder_fixture::registry;
    use crate::config::AuctioneerAppConfig;
    use auctioneer_config_data::types::HostConfig;
    use std::collections::BTreeMap;

    #[test]
    fn borrowed_comparison_matches_owned_ascii_normalization() {
        let names = [
            "",
            "fake_a",
            "fake_A",
            "fake_Z",
            "fake_abc-09",
            "fake_ABC-09",
            "fake_aBC_09",
            "fake_prefix",
            "fake_prefix-more",
            "fake_\u{00dc}ser",
            "fake_\u{00fc}ser",
            "fake_\u{03a9}A",
        ];
        for left in names {
            for right in names {
                assert_eq!(
                    folded_cmp(left, right),
                    left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase()),
                    "{left:?} versus {right:?}"
                );
            }
        }
    }

    #[test]
    fn compact_index_preserves_last_ascii_equivalent_alias_precedence() {
        let host = HostConfig {
            aliases: BTreeMap::from([
                ("fake_MY_ALIAS".to_owned(), "fake_aaa".to_owned()),
                ("fake_my_alias".to_owned(), "fake_bbb".to_owned()),
            ]),
            ..HostConfig::default()
        };
        let catalog =
            Catalog::from_registry(&host, &AuctioneerAppConfig::default(), registry()).unwrap();
        let candidate = catalog.lookup("fake_My_Alias").unwrap();
        assert_eq!(candidate.catalog_name, "fake_my_alias");
        assert_eq!(candidate.requested_name, "fake_My_Alias");
        assert_eq!(candidate.normalized_name, "fake_my_alias");
        assert_eq!(candidate.base_name, "fake_bbb");
        let available = catalog.available_names();
        assert_eq!(
            available
                .iter()
                .filter(|name| **name == "fake_my_alias")
                .count(),
            1
        );
        assert!(!available.contains(&"fake_MY_ALIAS"));
    }

    #[test]
    fn direct_construction_rejects_an_overfull_catalog_before_alias_lookup() {
        let aliases = (0_usize..513)
            .map(|index| {
                (
                    format!("fake_alias_{index:03}"),
                    "fake_missing_base".to_owned(),
                )
            })
            .collect();
        let host = HostConfig {
            aliases,
            ..HostConfig::default()
        };
        assert!(matches!(
            Catalog::from_registry(&host, &AuctioneerAppConfig::default(), registry()),
            Err(CatalogError::TooManyNames)
        ));
    }

    #[test]
    fn sync_catalog_uses_snapshot_activation_and_compiled_ancestry_for_host_aliases() {
        use crate::bidder_fixture::fake_bidder::FakeBidder;
        use auctioneer_bidder::BidderRegistry;
        use auctioneer_config_data::types::BidderInfo;

        let mut registry = BidderRegistry::new();
        for (name, parent) in [
            ("fake_controlled", None),
            ("fake_compiled_alias", Some("fake_controlled")),
        ] {
            let mut info = BidderInfo {
                name: name.into(),
                alias_of: parent.map(str::to_owned),
                default_enabled: Some(false),
                ..BidderInfo::default()
            };
            info.user_sync.enabled = true;
            registry
                .register(Box::leak(Box::new(info)), Box::<FakeBidder>::default())
                .unwrap();
        }
        let host = HostConfig {
            aliases: BTreeMap::from([("fake_host_alias".into(), "fake_compiled_alias".into())]),
            ..HostConfig::default()
        };
        for (overlay, expected) in [
            ("schema_version = 1\n", false),
            (
                "schema_version = 1\n[bidders.fake_compiled_alias]\nenabled = true\n[bidders.fake_host_alias]\nenabled = true\n",
                false,
            ),
            (
                "schema_version = 1\n[bidders.fake_controlled]\nenabled = true\n[bidders.fake_compiled_alias]\nenabled = true\n",
                true,
            ),
            (
                "schema_version = 1\n[bidders.fake_controlled]\nenabled = false\n[bidders.fake_compiled_alias]\nenabled = true\n",
                false,
            ),
            (
                "schema_version = 1\n[bidders.fake_controlled]\nenabled = true\n[bidders.fake_compiled_alias]\nenabled = true\n",
                true,
            ),
        ] {
            let app: AuctioneerAppConfig = toml::from_str(overlay).unwrap();
            let catalog = Catalog::from_registry(&host, &app, &registry).unwrap();
            assert_eq!(catalog.lookup("fake_host_alias").unwrap().enabled, expected);
            assert_eq!(
                catalog.lookup("fake_compiled_alias").unwrap().enabled,
                expected
            );
            assert_eq!(registry.infos().len(), 2);
        }
    }

    #[test]
    fn replacing_a_compiled_name_retains_original_and_replacement_sync_vetoes() {
        use crate::bidder_fixture::fake_bidder::FakeBidder;
        use auctioneer_bidder::BidderRegistry;
        use auctioneer_config_data::types::BidderInfo;

        for (baked_veto, overlay, expected) in [
            (false, "schema_version = 1\n", false),
            (
                false,
                "schema_version = 1\n[bidders.fake_alias]\nenabled = true\n",
                false,
            ),
            (
                false,
                "schema_version = 1\n[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                true,
            ),
            (
                false,
                "schema_version = 1\n[bidders.fake_base]\nenabled = false\n[bidders.fake_alias]\nenabled = true\n",
                false,
            ),
            (
                false,
                "schema_version = 1\n[bidders.fake_base]\nenabled = true\nusersync_enabled = false\n[bidders.fake_alias]\nenabled = true\n",
                false,
            ),
            (
                false,
                "schema_version = 1\n[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\nusersync_enabled = false\n",
                false,
            ),
            (
                false,
                "schema_version = 1\n[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n[bidders.fake_replacement]\nenabled = false\n",
                false,
            ),
            (
                false,
                "schema_version = 1\n[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n[bidders.fake_replacement]\nusersync_enabled = false\n",
                false,
            ),
            (
                true,
                "schema_version = 1\n[bidders.fake_base]\nenabled = true\n[bidders.fake_alias]\nenabled = true\n",
                false,
            ),
        ] {
            let mut registry = BidderRegistry::new();
            for (name, parent, enabled) in [
                ("fake_base", None, false),
                ("fake_alias", Some("fake_base"), false),
                ("fake_replacement", None, true),
            ] {
                let mut info = BidderInfo {
                    name: name.into(),
                    alias_of: parent.map(str::to_owned),
                    default_enabled: Some(enabled),
                    disabled: baked_veto && name == "fake_base",
                    ..BidderInfo::default()
                };
                info.user_sync.enabled = true;
                registry
                    .register(Box::leak(Box::new(info)), Box::<FakeBidder>::default())
                    .unwrap();
            }
            let app: AuctioneerAppConfig = toml::from_str(overlay).unwrap();
            for alias in ["fake_alias", "FAKE_ALIAS", "fAkE_aLiAs"] {
                let host = HostConfig {
                    aliases: BTreeMap::from([(alias.into(), "fake_replacement".into())]),
                    ..HostConfig::default()
                };
                let catalog = Catalog::from_registry(&host, &app, &registry).unwrap();
                let candidate = catalog.lookup("fake_alias").unwrap();
                assert_eq!(candidate.base_name, "fake_replacement");
                assert_eq!(
                    candidate.enabled, expected,
                    "alias={alias}, baked_veto={baked_veto}, overlay={overlay}"
                );
            }
        }
    }

    #[test]
    fn replacing_a_default_active_legacy_name_preserves_alias_precedence() {
        let host = HostConfig {
            aliases: BTreeMap::from([("fake_aaa".into(), "fake_bbb".into())]),
            ..HostConfig::default()
        };
        let app = AuctioneerAppConfig::default();
        let catalog = Catalog::from_registry(&host, &app, registry()).unwrap();
        let candidate = catalog.lookup("fake_aaa").unwrap();
        assert_eq!(candidate.base_name, "fake_bbb");
        assert_eq!(
            candidate.enabled,
            catalog.lookup("fake_bbb").unwrap().enabled
        );
    }
}
