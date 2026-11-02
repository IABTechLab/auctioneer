//! Synthetic metadata and declarations for registry behavior, never a serving catalog.

use auctioneer_bidder::test_support::fake_bidder::create_bidder;
use auctioneer_bidder::{BidderDeclaration, BidderRegistration, RegistryInputs};
use auctioneer_config::types::{
    BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
};
use std::sync::OnceLock;

pub const MANIFEST: &str = include_str!("../support/fake_bidder/fixtures/registry.toml");
pub const OPTIONAL_MANIFEST: &str = include_str!("../support/fake_bidder/fixtures/optional.toml");
pub const SOURCE_INVENTORY: &str =
    include_str!("../support/fake_bidder/fixtures/source-inventory.toml");
pub const SCHEMA: &[u8] = include_bytes!("../support/fake_bidder/fixtures/parameters.json");
pub const ALIAS_SCHEMA: &[u8] = include_bytes!("../support/fake_bidder/fixtures/alias.schema.json");

pub fn infos() -> &'static [BidderInfo] {
    static INFOS: OnceLock<Vec<BidderInfo>> = OnceLock::new();
    INFOS.get_or_init(|| {
        let manifest: toml::Value = toml::from_str(MANIFEST).unwrap();
        manifest["definitions"]
            .as_table()
            .unwrap()
            .iter()
            .map(|(name, definition)| BidderInfo {
                name: name.clone(),
                endpoint: definition["endpoint"].as_str().unwrap().into(),
                uses_https: true,
                ..BidderInfo::default()
            })
            .collect()
    })
}

pub fn inputs() -> RegistryInputs<'static> {
    static DECLARATIONS: &[BidderDeclaration] = &[BidderDeclaration {
        key: "fake_registry_fake",
        required_feature: None,
    }];
    static FACTORIES: &[BidderRegistration] = &[BidderRegistration {
        key: "fake_registry_fake",
        factory: create_bidder,
    }];
    static BINDINGS: &[BidderImplementationBinding] = &[
        BidderImplementationBinding {
            bidder: "fake_registry_alias",
            implementation: "fake_registry_fake",
        },
        BidderImplementationBinding {
            bidder: "fake_registry_base",
            implementation: "fake_registry_fake",
        },
        BidderImplementationBinding {
            bidder: "fake_registry_shared",
            implementation: "fake_registry_fake",
        },
    ];
    static METADATA: &[BidderMetadataDeclaration] = &[
        BidderMetadataDeclaration {
            bidder: "fake_registry_alias",
            implementation: "fake_registry_fake",
            required_feature: None,
        },
        BidderMetadataDeclaration {
            bidder: "fake_registry_base",
            implementation: "fake_registry_fake",
            required_feature: None,
        },
        BidderMetadataDeclaration {
            bidder: "fake_registry_shared",
            implementation: "fake_registry_fake",
            required_feature: None,
        },
    ];
    RegistryInputs {
        infos: infos(),
        bindings: BINDINGS,
        metadata_declarations: METADATA,
        factory_declarations: DECLARATIONS,
        factories: FACTORIES,
        aliases: &[],
    }
}
