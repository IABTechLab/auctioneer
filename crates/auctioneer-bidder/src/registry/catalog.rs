//! Immutable validation phase for the compiled registry join.
//!
//! This does not install a process registry or fund catalog storage. Callers
//! must fund metadata materialization and these indexes before construction.

use crate::{Bidder, BidderDeclaration, BidderRegistration};
use auctioneer_config::types::{
    BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

type Factory = fn() -> Box<dyn Bidder>;

/// Source ancestry of a flattened, compiled public alias.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryAlias {
    pub name: &'static str,
    pub source_parent: &'static str,
    pub canonical_base: &'static str,
}

/// Complete declaration views plus the actually linked active views.
#[derive(Clone, Copy)]
pub struct RegistryInputs<'a> {
    pub infos: &'static [BidderInfo],
    pub bindings: &'a [BidderImplementationBinding],
    pub metadata_declarations: &'a [BidderMetadataDeclaration],
    pub factory_declarations: &'a [BidderDeclaration],
    pub factories: &'a [BidderRegistration],
    pub aliases: &'a [RegistryAlias],
}

/// Structural failures contain public build identities, never instance values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistryBuildError {
    UnqualifiedSchemaArtifact,
    DuplicateMetadataDeclaration { bidder: &'static str },
    DuplicateFactoryDeclaration { implementation: &'static str },
    DuplicateFactory { implementation: &'static str },
    UndeclaredFactory { implementation: &'static str },
    MissingFactoryDeclaration { implementation: &'static str },
    DuplicateBidder { bidder: &'static str },
    UndeclaredBidder { bidder: &'static str },
    DuplicateBinding { bidder: &'static str },
    DanglingBinding { bidder: &'static str },
    MissingBinding { bidder: &'static str },
    BindingMismatch { bidder: &'static str },
    FeaturePredicateMismatch { bidder: &'static str },
    FeatureActivationMismatch { bidder: &'static str },
    FactoryFeatureActivationMismatch { implementation: &'static str },
    UnavailableFactory { implementation: &'static str },
    DuplicateAlias { alias: &'static str },
    DanglingAlias { alias: &'static str },
    UnknownAliasParent { alias: &'static str },
    InvalidCanonicalBase { alias: &'static str },
    AliasCycle { alias: &'static str },
    AliasImplementationMismatch { alias: &'static str },
}

impl fmt::Display for RegistryBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (reason, identity) = match *self {
            Self::UnqualifiedSchemaArtifact => ("schema artifact is unqualified", "catalog"),
            Self::DuplicateMetadataDeclaration { bidder } => {
                ("duplicate metadata declaration", bidder)
            }
            Self::DuplicateFactoryDeclaration { implementation } => {
                ("duplicate factory declaration", implementation)
            }
            Self::DuplicateFactory { implementation } => ("duplicate factory", implementation),
            Self::UndeclaredFactory { implementation } => ("undeclared factory", implementation),
            Self::MissingFactoryDeclaration { implementation } => {
                ("missing factory declaration", implementation)
            }
            Self::DuplicateBidder { bidder } => ("duplicate bidder", bidder),
            Self::UndeclaredBidder { bidder } => ("undeclared bidder", bidder),
            Self::DuplicateBinding { bidder } => ("duplicate binding", bidder),
            Self::DanglingBinding { bidder } => ("dangling binding", bidder),
            Self::MissingBinding { bidder } => ("missing binding", bidder),
            Self::BindingMismatch { bidder } => ("binding mismatch", bidder),
            Self::FeaturePredicateMismatch { bidder } => ("feature predicate mismatch", bidder),
            Self::FeatureActivationMismatch { bidder } => ("feature activation mismatch", bidder),
            Self::FactoryFeatureActivationMismatch { implementation } => {
                ("factory feature activation mismatch", implementation)
            }
            Self::UnavailableFactory { implementation } => ("unavailable factory", implementation),
            Self::DuplicateAlias { alias } => ("duplicate alias", alias),
            Self::DanglingAlias { alias } => ("dangling alias", alias),
            Self::UnknownAliasParent { alias } => ("unknown alias parent", alias),
            Self::InvalidCanonicalBase { alias } => ("invalid canonical base", alias),
            Self::AliasCycle { alias } => ("alias cycle", alias),
            Self::AliasImplementationMismatch { alias } => ("alias implementation mismatch", alias),
        };
        write!(f, "bidder registry build failed: {reason} ({identity})")
    }
}

impl Error for RegistryBuildError {}

/// A definition obtainable only after the complete input has validated.
pub struct RegistryDefinition {
    info: &'static BidderInfo,
    implementation: &'static str,
    canonical_base: &'static str,
    source_parent: Option<&'static str>,
    factory: Factory,
}

impl RegistryDefinition {
    #[must_use]
    pub const fn info(&self) -> &'static BidderInfo {
        self.info
    }

    #[must_use]
    pub const fn implementation(&self) -> &'static str {
        self.implementation
    }

    #[must_use]
    pub const fn canonical_base(&self) -> &'static str {
        self.canonical_base
    }

    #[must_use]
    pub const fn source_parent(&self) -> Option<&'static str> {
        self.source_parent
    }

    /// Construct after validation and the caller's storage reservation.
    #[must_use]
    pub fn construct(&self) -> Box<dyn Bidder> {
        (self.factory)()
    }
}

/// Read-only public-definition, base, source-alias and factory indexes.
pub struct ValidatedRegistryPlan {
    definitions: BTreeMap<&'static str, RegistryDefinition>,
    bases: BTreeMap<&'static str, &'static BidderInfo>,
    aliases: BTreeMap<&'static str, RegistryAlias>,
    factories: BTreeMap<&'static str, Factory>,
}

impl fmt::Debug for ValidatedRegistryPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedRegistryPlan")
            .field("definitions", &self.definitions.len())
            .field("bases", &self.bases.len())
            .field("aliases", &self.aliases.len())
            .field("factories", &self.factories.len())
            .finish()
    }
}

impl ValidatedRegistryPlan {
    /// Validate all declarations, active membership and alias ancestry.
    /// No bidder factory is called, including on successful validation.
    ///
    /// # Errors
    /// Returns a typed structural refusal without exposing a partial plan.
    pub fn validate(inputs: RegistryInputs<'_>) -> Result<Self, RegistryBuildError> {
        let declarations = DeclarationIndexes::validate(&inputs)?;
        let active = ActiveIndexes::validate(&inputs, &declarations)?;
        let ancestry = AliasIndexes::validate(inputs.aliases, &declarations)?;
        let mut definitions = BTreeMap::new();
        let mut bases = BTreeMap::new();
        for (name, info) in active.infos {
            let implementation = *active
                .bindings
                .get(name)
                .ok_or(RegistryBuildError::MissingBinding { bidder: name })?;
            let factory = *declarations
                .factories
                .get(implementation)
                .ok_or(RegistryBuildError::FeatureActivationMismatch { bidder: name })?;
            let canonical_base = *ancestry
                .canonical
                .get(name)
                .ok_or(RegistryBuildError::InvalidCanonicalBase { alias: name })?;
            let source_parent = ancestry.aliases.get(name).map(|alias| alias.source_parent);
            if source_parent.is_none() {
                bases.insert(name, info);
            }
            definitions.insert(
                name,
                RegistryDefinition {
                    info,
                    implementation,
                    canonical_base,
                    source_parent,
                    factory,
                },
            );
        }
        let aliases = ancestry
            .aliases
            .into_iter()
            .filter(|(name, _)| definitions.contains_key(*name))
            .collect();
        Ok(Self {
            definitions,
            bases,
            aliases,
            factories: declarations.factories,
        })
    }

    #[must_use]
    pub fn definition(&self, name: &str) -> Option<&RegistryDefinition> {
        self.definitions.get(name)
    }

    #[must_use]
    pub fn definitions(&self) -> impl ExactSizeIterator<Item = &RegistryDefinition> {
        self.definitions.values()
    }

    #[must_use]
    pub fn base_names(&self) -> impl ExactSizeIterator<Item = &'static str> {
        self.bases.keys().copied()
    }

    #[must_use]
    pub fn aliases(&self) -> impl ExactSizeIterator<Item = &RegistryAlias> {
        self.aliases.values()
    }

    #[must_use]
    pub fn factory_keys(&self) -> impl ExactSizeIterator<Item = &'static str> {
        self.factories.keys().copied()
    }
}

struct DeclarationIndexes {
    metadata: BTreeMap<&'static str, BidderMetadataDeclaration>,
    factories: BTreeMap<&'static str, Factory>,
}

impl DeclarationIndexes {
    fn validate(inputs: &RegistryInputs<'_>) -> Result<Self, RegistryBuildError> {
        let mut metadata = BTreeMap::new();
        let mut folded_names = BTreeSet::new();
        for row in inputs.metadata_declarations {
            if !folded_names.insert(row.bidder.to_ascii_lowercase()) {
                return Err(RegistryBuildError::DuplicateMetadataDeclaration {
                    bidder: row.bidder,
                });
            }
            metadata.insert(row.bidder, *row);
        }
        let mut factory_features = BTreeMap::new();
        for row in inputs.factory_declarations {
            if factory_features
                .insert(row.key, row.required_feature)
                .is_some()
            {
                return Err(RegistryBuildError::DuplicateFactoryDeclaration {
                    implementation: row.key,
                });
            }
        }
        let mut factories = BTreeMap::new();
        for row in inputs.factories {
            if !factory_features.contains_key(row.key) {
                return Err(RegistryBuildError::UndeclaredFactory {
                    implementation: row.key,
                });
            }
            if factories.insert(row.key, row.factory).is_some() {
                return Err(RegistryBuildError::DuplicateFactory {
                    implementation: row.key,
                });
            }
        }
        for row in inputs.metadata_declarations {
            let feature = factory_features.get(row.implementation).ok_or(
                RegistryBuildError::MissingFactoryDeclaration {
                    implementation: row.implementation,
                },
            )?;
            if *feature != row.required_feature {
                return Err(RegistryBuildError::FeaturePredicateMismatch { bidder: row.bidder });
            }
        }
        for (implementation, feature) in factory_features {
            if feature.is_none() && !factories.contains_key(implementation) {
                return Err(RegistryBuildError::UnavailableFactory { implementation });
            }
        }
        let mut active_features = BTreeMap::new();
        for row in inputs.factory_declarations {
            if let Some(feature) = row.required_feature {
                let active = factories.contains_key(row.key);
                if let Some(previous) = active_features.insert(feature, active)
                    && previous != active
                {
                    return Err(RegistryBuildError::FactoryFeatureActivationMismatch {
                        implementation: row.key,
                    });
                }
            }
        }
        Ok(Self {
            metadata,
            factories,
        })
    }
}

struct ActiveIndexes {
    infos: BTreeMap<&'static str, &'static BidderInfo>,
    bindings: BTreeMap<&'static str, &'static str>,
}

impl ActiveIndexes {
    fn validate(
        inputs: &RegistryInputs<'_>,
        declarations: &DeclarationIndexes,
    ) -> Result<Self, RegistryBuildError> {
        let mut infos = BTreeMap::new();
        let mut folded_names = BTreeSet::new();
        for info in inputs.infos {
            let name = info.name.as_str();
            if !folded_names.insert(name.to_ascii_lowercase()) {
                return Err(RegistryBuildError::DuplicateBidder { bidder: name });
            }
            if !declarations.metadata.contains_key(name) {
                return Err(RegistryBuildError::UndeclaredBidder { bidder: name });
            }
            infos.insert(name, info);
        }
        let mut bindings = BTreeMap::new();
        for row in inputs.bindings {
            if bindings.insert(row.bidder, row.implementation).is_some() {
                return Err(RegistryBuildError::DuplicateBinding { bidder: row.bidder });
            }
            if !infos.contains_key(row.bidder) {
                return Err(RegistryBuildError::DanglingBinding { bidder: row.bidder });
            }
            let declared = declarations
                .metadata
                .get(row.bidder)
                .ok_or(RegistryBuildError::UndeclaredBidder { bidder: row.bidder })?;
            if declared.implementation != row.implementation {
                return Err(RegistryBuildError::BindingMismatch { bidder: row.bidder });
            }
        }
        for name in infos.keys() {
            if !bindings.contains_key(name) {
                return Err(RegistryBuildError::MissingBinding { bidder: name });
            }
        }
        for row in inputs.metadata_declarations {
            let active = infos.contains_key(row.bidder);
            let compiled = declarations.factories.contains_key(row.implementation);
            if active != compiled || (row.required_feature.is_none() && !active) {
                return Err(RegistryBuildError::FeatureActivationMismatch { bidder: row.bidder });
            }
        }
        Ok(Self { infos, bindings })
    }
}

struct AliasIndexes {
    aliases: BTreeMap<&'static str, RegistryAlias>,
    canonical: BTreeMap<&'static str, &'static str>,
}

impl AliasIndexes {
    fn validate(
        rows: &[RegistryAlias],
        declarations: &DeclarationIndexes,
    ) -> Result<Self, RegistryBuildError> {
        let mut aliases = BTreeMap::new();
        for row in rows {
            if aliases.insert(row.name, *row).is_some() {
                return Err(RegistryBuildError::DuplicateAlias { alias: row.name });
            }
            if !declarations.metadata.contains_key(row.name) {
                return Err(RegistryBuildError::DanglingAlias { alias: row.name });
            }
        }
        for row in rows {
            if !declarations.metadata.contains_key(row.source_parent) {
                return Err(RegistryBuildError::UnknownAliasParent { alias: row.name });
            }
            if !declarations.metadata.contains_key(row.canonical_base)
                || aliases.contains_key(row.canonical_base)
            {
                return Err(RegistryBuildError::InvalidCanonicalBase { alias: row.name });
            }
        }
        let canonical = declarations
            .metadata
            .keys()
            .filter(|name| !aliases.contains_key(*name))
            .map(|name| (*name, *name))
            .collect();
        let mut indexes = Self { aliases, canonical };
        for row in rows {
            indexes.resolve_chain(row.name, declarations)?;
        }
        Ok(indexes)
    }

    fn resolve_chain(
        &mut self,
        start: &'static str,
        declarations: &DeclarationIndexes,
    ) -> Result<(), RegistryBuildError> {
        let mut path = Vec::new();
        let mut visited = BTreeSet::new();
        let mut cursor = start;
        let canonical = loop {
            if let Some(base) = self.canonical.get(cursor) {
                break *base;
            }
            if !visited.insert(cursor) {
                return Err(RegistryBuildError::AliasCycle { alias: cursor });
            }
            let row = *self
                .aliases
                .get(cursor)
                .ok_or(RegistryBuildError::UnknownAliasParent { alias: start })?;
            path.push(row);
            cursor = row.source_parent;
        };
        let base = declarations
            .metadata
            .get(canonical)
            .ok_or(RegistryBuildError::InvalidCanonicalBase { alias: start })?;
        for row in path.into_iter().rev() {
            if row.canonical_base != canonical {
                return Err(RegistryBuildError::InvalidCanonicalBase { alias: row.name });
            }
            let declared = declarations
                .metadata
                .get(row.name)
                .ok_or(RegistryBuildError::DanglingAlias { alias: row.name })?;
            if declared.implementation != base.implementation {
                return Err(RegistryBuildError::AliasImplementationMismatch { alias: row.name });
            }
            self.canonical.insert(row.name, canonical);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{RegistryAlias, RegistryBuildError, RegistryInputs, ValidatedRegistryPlan};
    use crate::{
        Bidder, BidderDeclaration, BidderError, BidderHttpResponse, BidderRegistration,
        BidderRequestInfo, BidderResponse, DebugBodySchema, OutboundRequestMeta,
        OutboundRequestSink,
    };
    use auctioneer_config::types::{
        BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
    };
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use std::cell::RefCell;
    use std::ptr;

    thread_local! {
        static CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }

    struct ProbeBidder(usize);

    impl Bidder for ProbeBidder {
        fn debug_body_schema(&self) -> DebugBodySchema {
            DebugBodySchema::Unsupported
        }

        fn make_requests(
            &self,
            _request: &BidRequest,
            _info: &BidderRequestInfo<'_>,
            _sink: &mut OutboundRequestSink<'_>,
        ) -> Result<(), Vec<BidderError>> {
            Err(vec![BidderError::unknown(self.0.to_string())])
        }

        fn make_bids(
            &self,
            _request: &BidRequest,
            _sent: &OutboundRequestMeta,
            _response: &BidderHttpResponse,
        ) -> Result<BidderResponse, Vec<BidderError>> {
            Err(vec![])
        }
    }

    fn shared() -> Box<dyn Bidder> {
        let ordinal = CALLS.with_borrow_mut(|calls| {
            calls.push("fake_shared");
            calls.len()
        });
        Box::new(ProbeBidder(ordinal))
    }

    fn other() -> Box<dyn Bidder> {
        let ordinal = CALLS.with_borrow_mut(|calls| {
            calls.push("fake_other");
            calls.len()
        });
        Box::new(ProbeBidder(ordinal))
    }

    struct Fixture {
        infos: &'static [BidderInfo],
        bindings: Vec<BidderImplementationBinding>,
        metadata: Vec<BidderMetadataDeclaration>,
        declarations: Vec<BidderDeclaration>,
        factories: Vec<BidderRegistration>,
        aliases: Vec<RegistryAlias>,
    }

    impl Fixture {
        fn new() -> Self {
            CALLS.with_borrow_mut(Vec::clear);
            let infos = Box::leak(
                [
                    "fake_zIndependent",
                    "fake_base",
                    "fake_child",
                    "fake_nested",
                ]
                .map(|name| BidderInfo {
                    name: name.into(),
                    endpoint: format!("https://{name}.example.test/bid"),
                    gvl_vendor_id: Some(if name == "fake_base" { 7 } else { 9 }),
                    disabled: name == "fake_child",
                    ..BidderInfo::default()
                })
                .to_vec()
                .into_boxed_slice(),
            );
            let bindings: Vec<_> = [
                "fake_zIndependent",
                "fake_base",
                "fake_child",
                "fake_nested",
            ]
            .into_iter()
            .map(|bidder| BidderImplementationBinding {
                bidder,
                implementation: "fake_shared",
            })
            .collect();
            let metadata = bindings
                .iter()
                .map(|binding| BidderMetadataDeclaration {
                    bidder: binding.bidder,
                    implementation: binding.implementation,
                    required_feature: None,
                })
                .collect();
            Self {
                infos,
                bindings,
                metadata,
                declarations: vec![
                    BidderDeclaration {
                        key: "fake_shared",
                        required_feature: None,
                    },
                    BidderDeclaration {
                        key: "fake_unused",
                        required_feature: Some("unused-feature"),
                    },
                ],
                factories: vec![BidderRegistration {
                    key: "fake_shared",
                    factory: shared,
                }],
                aliases: vec![
                    RegistryAlias {
                        name: "fake_nested",
                        source_parent: "fake_child",
                        canonical_base: "fake_base",
                    },
                    RegistryAlias {
                        name: "fake_child",
                        source_parent: "fake_base",
                        canonical_base: "fake_base",
                    },
                ],
            }
        }

        fn validate(&self) -> Result<ValidatedRegistryPlan, RegistryBuildError> {
            ValidatedRegistryPlan::validate(RegistryInputs {
                infos: self.infos,
                bindings: &self.bindings,
                metadata_declarations: &self.metadata,
                factory_declarations: &self.declarations,
                factories: &self.factories,
                aliases: &self.aliases,
            })
        }

        fn refusal(&self, expected: RegistryBuildError) {
            assert_eq!(self.validate().unwrap_err(), expected);
            CALLS.with_borrow(|calls| assert!(calls.is_empty(), "invalid input called a factory"));
        }
    }

    #[test]
    fn validated_definitions_borrow_metadata_and_keep_independent_public_bases() {
        let fixture = Fixture::new();
        let plan = fixture.validate().unwrap();
        assert_eq!(plan.definitions().len(), 4);
        assert_eq!(
            plan.base_names().collect::<Vec<_>>(),
            ["fake_base", "fake_zIndependent"]
        );
        assert_eq!(
            plan.aliases().map(|row| row.name).collect::<Vec<_>>(),
            ["fake_child", "fake_nested"]
        );
        assert_eq!(plan.factory_keys().collect::<Vec<_>>(), ["fake_shared"]);
        assert!(plan.definition("fake_unknown").is_none());
        for info in fixture.infos {
            let definition = plan.definition(&info.name).unwrap();
            assert!(ptr::eq(definition.info(), info));
            assert_eq!(definition.implementation(), "fake_shared");
            assert_eq!(
                definition.canonical_base(),
                if info.name == "fake_zIndependent" {
                    "fake_zIndependent"
                } else {
                    "fake_base"
                }
            );
        }
        assert_eq!(
            plan.definition("fake_nested").unwrap().source_parent(),
            Some("fake_child")
        );
        assert_eq!(plan.definition("fake_base").unwrap().source_parent(), None);
        CALLS.with_borrow(|calls| assert!(calls.is_empty()));
        let instances: Vec<_> = plan
            .definitions()
            .map(super::RegistryDefinition::construct)
            .collect();
        assert!(!ptr::addr_eq(
            ptr::from_ref(instances[0].as_ref()),
            ptr::from_ref(instances[1].as_ref())
        ));
        CALLS.with_borrow(|calls| assert_eq!(calls.as_slice(), ["fake_shared"; 4]));
    }

    #[test]
    fn duplicate_declarations_and_factories_fail_without_construction() {
        let mut fixture = Fixture::new();
        fixture.metadata.push(fixture.metadata[0]);
        fixture.refusal(RegistryBuildError::DuplicateMetadataDeclaration {
            bidder: "fake_zIndependent",
        });
        fixture.metadata.pop().unwrap();
        fixture.declarations.push(BidderDeclaration {
            key: "fake_shared",
            required_feature: None,
        });
        fixture.refusal(RegistryBuildError::DuplicateFactoryDeclaration {
            implementation: "fake_shared",
        });
        fixture.declarations.pop().unwrap();
        fixture.factories.push(BidderRegistration {
            key: "fake_shared",
            factory: other,
        });
        fixture.refusal(RegistryBuildError::DuplicateFactory {
            implementation: "fake_shared",
        });
    }

    #[test]
    fn ascii_folded_metadata_collisions_fail_even_when_inactive() {
        let mut fixture = Fixture::new();
        fixture.metadata.push(BidderMetadataDeclaration {
            bidder: "fake_BASE",
            implementation: "fake_shared",
            required_feature: Some("disabled-feature"),
        });
        fixture.refusal(RegistryBuildError::DuplicateMetadataDeclaration {
            bidder: "fake_BASE",
        });
    }

    #[test]
    fn incomplete_or_inconsistent_active_bindings_fail_without_construction() {
        let mut fixture = Fixture::new();
        fixture.bindings.pop().unwrap();
        fixture.refusal(RegistryBuildError::MissingBinding {
            bidder: "fake_nested",
        });
        fixture.bindings.push(BidderImplementationBinding {
            bidder: "fake_nested",
            implementation: "fake_other",
        });
        fixture.refusal(RegistryBuildError::BindingMismatch {
            bidder: "fake_nested",
        });
        fixture.bindings.pop().unwrap();
        fixture.bindings.push(BidderImplementationBinding {
            bidder: "fake_nested",
            implementation: "fake_shared",
        });
        fixture.bindings.push(fixture.bindings[0]);
        fixture.refusal(RegistryBuildError::DuplicateBinding {
            bidder: "fake_zIndependent",
        });
        fixture.bindings.pop().unwrap();
        fixture.bindings.push(BidderImplementationBinding {
            bidder: "fake_absent",
            implementation: "fake_shared",
        });
        fixture.refusal(RegistryBuildError::DanglingBinding {
            bidder: "fake_absent",
        });
    }

    #[test]
    fn undeclared_factory_and_missing_factory_declaration_fail_closed() {
        let mut fixture = Fixture::new();
        fixture.factories.push(BidderRegistration {
            key: "fake_other",
            factory: other,
        });
        fixture.refusal(RegistryBuildError::UndeclaredFactory {
            implementation: "fake_other",
        });
        fixture.factories.pop().unwrap();
        fixture.metadata[0].implementation = "fake_missing";
        fixture.refusal(RegistryBuildError::MissingFactoryDeclaration {
            implementation: "fake_missing",
        });
    }

    #[test]
    fn matching_predicates_still_require_identical_actual_feature_membership() {
        let mut fixture = Fixture::new();
        fixture.declarations[0].required_feature = Some("optional");
        fixture.refusal(RegistryBuildError::FeaturePredicateMismatch {
            bidder: "fake_zIndependent",
        });
        for row in &mut fixture.metadata {
            row.required_feature = Some("optional");
        }
        fixture.factories.clear();
        fixture.refusal(RegistryBuildError::FeatureActivationMismatch {
            bidder: "fake_zIndependent",
        });
        fixture.factories.push(BidderRegistration {
            key: "fake_shared",
            factory: shared,
        });
        fixture.infos = &[];
        fixture.bindings.clear();
        fixture.refusal(RegistryBuildError::FeatureActivationMismatch {
            bidder: "fake_zIndependent",
        });
    }

    #[test]
    fn malformed_alias_ancestry_fails_even_for_disabled_definitions() {
        let mut fixture = Fixture::new();
        fixture.aliases[1].source_parent = "fake_unknown";
        fixture.refusal(RegistryBuildError::UnknownAliasParent {
            alias: "fake_child",
        });
        fixture.aliases[1].source_parent = "fake_base";
        fixture.aliases[1].canonical_base = "fake_zIndependent";
        fixture.refusal(RegistryBuildError::InvalidCanonicalBase {
            alias: "fake_child",
        });
        fixture.aliases[1].canonical_base = "fake_base";
        fixture.aliases[1].source_parent = "fake_nested";
        fixture.refusal(RegistryBuildError::AliasCycle {
            alias: "fake_nested",
        });
    }

    #[test]
    fn active_metadata_refuses_exact_and_folded_duplicates_and_undeclared_names() {
        for name in ["fake_base", "fake_BASE", "fake_undeclared"] {
            let mut fixture = Fixture::new();
            let mut infos = fixture.infos.to_vec();
            infos.push(BidderInfo {
                name: name.into(),
                ..BidderInfo::default()
            });
            fixture.infos = Box::leak(infos.into_boxed_slice());
            fixture.refusal(if name == "fake_undeclared" {
                RegistryBuildError::UndeclaredBidder { bidder: name }
            } else {
                RegistryBuildError::DuplicateBidder { bidder: name }
            });
        }
    }

    #[test]
    fn unconditional_factories_must_exist_even_without_public_definitions() {
        let mut fixture = Fixture::new();
        fixture.factories.clear();
        fixture.refusal(RegistryBuildError::UnavailableFactory {
            implementation: "fake_shared",
        });
        fixture.factories.push(BidderRegistration {
            key: "fake_shared",
            factory: shared,
        });
        fixture.declarations[1].required_feature = None;
        fixture.refusal(RegistryBuildError::UnavailableFactory {
            implementation: "fake_unused",
        });
        fixture.factories.push(BidderRegistration {
            key: "fake_unused",
            factory: other,
        });
        let plan = fixture.validate().unwrap();
        assert_eq!(
            plan.factory_keys().collect::<Vec<_>>(),
            ["fake_shared", "fake_unused"]
        );
        assert_eq!(plan.definitions().len(), 4);
        CALLS.with_borrow(|calls| assert!(calls.is_empty()));
    }

    #[test]
    fn matching_inactive_feature_views_validate_their_aliases_without_materializing_entries() {
        let mut fixture = Fixture::new();
        fixture.declarations[0].required_feature = Some("optional");
        for row in &mut fixture.metadata {
            row.required_feature = Some("optional");
        }
        fixture.factories.clear();
        fixture.infos = &[];
        fixture.bindings.clear();
        let plan = fixture.validate().unwrap();
        assert_eq!(plan.definitions().len(), 0);
        assert_eq!(plan.base_names().len(), 0);
        assert_eq!(plan.aliases().len(), 0);
        assert_eq!(plan.factory_keys().len(), 0);
        fixture.aliases[1].source_parent = "fake_missing";
        fixture.refusal(RegistryBuildError::UnknownAliasParent {
            alias: "fake_child",
        });
    }

    #[test]
    fn missing_one_active_identity_cannot_hide_behind_a_shared_live_factory() {
        let mut fixture = Fixture::new();
        fixture.infos = &fixture.infos[1..];
        fixture.bindings.remove(0);
        fixture.refusal(RegistryBuildError::FeatureActivationMismatch {
            bidder: "fake_zIndependent",
        });
    }

    #[test]
    fn aliases_refuse_duplicate_or_dangling_declarations() {
        let mut fixture = Fixture::new();
        fixture.aliases.push(fixture.aliases[0]);
        fixture.refusal(RegistryBuildError::DuplicateAlias {
            alias: "fake_nested",
        });
        fixture.aliases.pop().unwrap();
        fixture.aliases.push(RegistryAlias {
            name: "fake_missing",
            source_parent: "fake_base",
            canonical_base: "fake_base",
        });
        fixture.refusal(RegistryBuildError::DanglingAlias {
            alias: "fake_missing",
        });
    }

    #[test]
    fn aliases_refuse_missing_or_noncanonical_bases_and_self_cycles() {
        let mut fixture = Fixture::new();
        for canonical_base in ["fake_missing", "fake_nested"] {
            fixture.aliases[1].canonical_base = canonical_base;
            fixture.refusal(RegistryBuildError::InvalidCanonicalBase {
                alias: "fake_child",
            });
        }
        fixture.aliases[1].canonical_base = "fake_base";
        fixture.aliases[1].source_parent = "fake_child";
        fixture.refusal(RegistryBuildError::AliasCycle {
            alias: "fake_child",
        });
    }

    #[test]
    fn aliases_cannot_switch_implementation_while_preserving_a_valid_parent() {
        let mut fixture = Fixture::new();
        fixture.declarations.push(BidderDeclaration {
            key: "fake_other",
            required_feature: None,
        });
        fixture.factories.push(BidderRegistration {
            key: "fake_other",
            factory: other,
        });
        fixture.metadata[2].implementation = "fake_other";
        fixture.bindings[2].implementation = "fake_other";
        fixture.refusal(RegistryBuildError::AliasImplementationMismatch {
            alias: "fake_child",
        });
    }

    #[test]
    fn declaration_and_alias_order_do_not_change_validated_public_views() {
        for reversed in [false, true] {
            let mut fixture = Fixture::new();
            if reversed {
                fixture.metadata.reverse();
                fixture.bindings.reverse();
                fixture.declarations.reverse();
                fixture.aliases.reverse();
            }
            let plan = fixture.validate().unwrap();
            assert_eq!(
                plan.definitions()
                    .map(|row| row.info().name.as_str())
                    .collect::<Vec<_>>(),
                [
                    "fake_base",
                    "fake_child",
                    "fake_nested",
                    "fake_zIndependent"
                ]
            );
            assert_eq!(
                plan.definition("fake_nested").unwrap().canonical_base(),
                "fake_base"
            );
            assert_eq!(
                plan.definition("fake_nested").unwrap().source_parent(),
                Some("fake_child")
            );
            assert_eq!(
                plan.definition("fake_zIndependent")
                    .unwrap()
                    .canonical_base(),
                "fake_zIndependent"
            );
            CALLS.with_borrow(|calls| assert!(calls.is_empty()));
        }
    }

    #[test]
    fn one_feature_cannot_activate_only_some_of_its_factories_even_when_unused() {
        for (shared_active, unused_active) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let mut fixture = Fixture::new();
            for row in &mut fixture.declarations {
                row.required_feature = Some("optional");
            }
            for row in &mut fixture.metadata {
                row.required_feature = Some("optional");
            }
            if !shared_active {
                fixture.factories.clear();
                fixture.infos = &[];
                fixture.bindings.clear();
            }
            if unused_active {
                fixture.factories.push(BidderRegistration {
                    key: "fake_unused",
                    factory: other,
                });
            }
            if shared_active == unused_active {
                let plan = fixture.validate().unwrap();
                assert_eq!(plan.definitions().len(), if shared_active { 4 } else { 0 });
                assert_eq!(plan.factory_keys().len(), if shared_active { 2 } else { 0 });
                CALLS.with_borrow(|calls| assert!(calls.is_empty()));
            } else {
                fixture.refusal(RegistryBuildError::FactoryFeatureActivationMismatch {
                    implementation: "fake_unused",
                });
            }
        }
    }
}
