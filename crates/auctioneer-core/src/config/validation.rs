//! STAGED pure bidder-policy subvalidation, not complete operator-policy or
//! snapshot admission. This module does not install serving configuration.

use super::{BidderOverrides, MAX_BIDDER_NAME_BYTES, validate_host_alias_overlay};
use auctioneer_bidder::{
    BidderConfigError, BidderConfigInput, RegistryDefinition, ValidatedRegistryPlan,
};
use auctioneer_config::usersync::validate_catalog_size;
use auctioneer_config::{BidderSettings, bidder_settings::ensure_limits};
use std::collections::{BTreeMap, BTreeSet};
use validator::Validate as _;

/// Fixed shared refusals or reviewed static bidder-owned identifiers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum BidderConfigurationError {
    /// A shared name, map, override or settings bound is invalid.
    #[error("bidder configuration structure invalid")]
    Structure,
    /// Distinct identities differ only in ASCII case.
    #[error("bidder configuration namespace collision")]
    NamespaceCollision,
    /// A host alias does not target an eligible compiled base.
    #[error("bidder configuration alias base invalid")]
    InvalidAliasBase,
    /// Settings do not bind to a linked settings-owning implementation.
    #[error("bidder configuration settings binding invalid")]
    SettingsBinding,
    /// The owner refused supplied values or effective requiredness.
    #[error(transparent)]
    Bidder(#[from] BidderConfigError),
}

/// STAGED pure bidder subvalidator, not complete `OperatorPolicy` validation
/// or snapshot admission. `aliases` must already be the effective host map.
/// No defaults, accounts, globals, factories, I/O or secrets are resolved here.
///
/// Callers must supply bounded, caller-owned input and fund temporary namespace
/// folding (map nodes and folded names), borrowed identity-set nodes (at most
/// catalog definitions + host aliases + explicit settings), and shared/bidder
/// validation-error peaks before activation. This staged API supplies neither
/// storage admission nor a new numeric budget. No settings or account is cloned.
///
/// # Errors
/// Returns only fixed shared categories or static bidder field/reason IDs.
pub fn validate_bidder_configuration(
    bidders: &BTreeMap<String, BidderOverrides>,
    aliases: &BTreeMap<String, String>,
    catalog: &ValidatedRegistryPlan,
) -> Result<(), BidderConfigurationError> {
    validate_host_alias_overlay(aliases).map_err(|_error| BidderConfigurationError::Structure)?;
    validate_catalog_size(catalog.definitions().len(), aliases.len())
        .map_err(|_error| BidderConfigurationError::Structure)?;
    if aliases.keys().any(String::is_empty)
        || bidders
            .keys()
            .any(|name| name.len() > MAX_BIDDER_NAME_BYTES)
    {
        return Err(BidderConfigurationError::Structure);
    }
    for entry in bidders.values() {
        if let Some(settings) = &entry.settings {
            ensure_limits(settings).map_err(|_error| BidderConfigurationError::Structure)?;
        }
        entry
            .validate()
            .map_err(|_error| BidderConfigurationError::Structure)?;
    }
    validate_namespace(bidders, aliases, catalog)?;
    for base in aliases.values() {
        // Serving BidderRegistry::base_names includes compiled aliases and
        // disabled definitions; plan.base_names is a narrower ancestry index.
        if catalog.definition(base).is_none() {
            return Err(BidderConfigurationError::InvalidAliasBase);
        }
    }

    let mut identities = BTreeSet::new();
    for definition in catalog.definitions() {
        if compiled_enabled(bidders, definition, catalog) {
            identities.insert(definition.info().name.as_str());
        }
    }
    identities.extend(aliases.keys().map(String::as_str));
    identities.extend(
        bidders
            .iter()
            .filter_map(|(name, entry)| entry.settings.as_ref().map(|_| name.as_str())),
    );

    // Complete binding admission for the entire union before any owner runs,
    // including disabled, shadowed and future request-local supplied entries.
    for name in &identities {
        let (implementation, input) = identity_context(name, bidders, aliases, catalog);
        for settings in input.requested.into_iter().chain(input.base) {
            if Some(settings.implementation_key()) != implementation
                || catalog
                    .config_validator(settings.implementation_key())
                    .is_none()
            {
                return Err(BidderConfigurationError::SettingsBinding);
            }
        }
    }
    for name in identities {
        let (implementation, input) = identity_context(name, bidders, aliases, catalog);
        if let Some(callback) = implementation.and_then(|key| catalog.config_validator(key)) {
            callback(input)?;
        }
    }
    Ok(())
}

fn validate_namespace(
    bidders: &BTreeMap<String, BidderOverrides>,
    aliases: &BTreeMap<String, String>,
    catalog: &ValidatedRegistryPlan,
) -> Result<(), BidderConfigurationError> {
    let mut folded = BTreeMap::new();
    for name in catalog
        .definitions()
        .map(|definition| definition.info().name.as_str())
        .chain(aliases.keys().map(String::as_str))
        .chain(bidders.keys().map(String::as_str))
    {
        if folded
            .insert(name.to_ascii_lowercase(), name)
            .is_some_and(|previous| previous != name)
        {
            return Err(BidderConfigurationError::NamespaceCollision);
        }
    }
    Ok(())
}

fn identity_context<'a>(
    name: &str,
    bidders: &'a BTreeMap<String, BidderOverrides>,
    aliases: &BTreeMap<String, String>,
    catalog: &'a ValidatedRegistryPlan,
) -> (Option<&'static str>, BidderConfigInput<'a>) {
    // The host mapping wins even over an exact compiled name. Like exchange's
    // pbr.base, its target is a catalog name, not another host-alias lookup.
    let target = aliases.get(name).map_or(name, String::as_str);
    let definition = catalog.definition(target);
    let requested = bidders.get(name).and_then(|entry| entry.settings.as_ref());
    let implementation = definition
        .map(RegistryDefinition::implementation)
        .or(requested.map(BidderSettings::implementation_key));
    let enabled = definition.is_some_and(|base| compiled_enabled(bidders, base, catalog))
        && catalog
            .definition(name)
            .is_none_or(|own| compiled_enabled(bidders, own, catalog))
        && bidders
            .get(name)
            .and_then(|entry| entry.enabled)
            .unwrap_or(true);
    let input = BidderConfigInput {
        requested,
        // Direct compiled aliases use their own wire name, not canonical-base
        // settings. Only a configured host alias selects a different base name.
        base: definition.and_then(|_| bidders.get(target)?.settings.as_ref()),
        definition: definition.map(RegistryDefinition::info),
        enabled,
    };
    (implementation, input)
}

fn compiled_enabled<'a>(
    bidders: &BTreeMap<String, BidderOverrides>,
    mut definition: &'a RegistryDefinition,
    catalog: &'a ValidatedRegistryPlan,
) -> bool {
    // resolve's helper takes the legacy whole application DTO. Walk the same
    // gates through validated source ancestry without cloning that DTO or
    // changing the old serving helper's semantics.
    for _ in 0..catalog.definitions().len() {
        let info = definition.info();
        if info.disabled
            || !bidders
                .get(&info.name)
                .and_then(|entry| entry.enabled)
                .or(info.default_enabled)
                .unwrap_or(true)
        {
            return false;
        }
        let Some(parent_name) = definition.source_parent() else {
            return true;
        };
        let Some(parent) = catalog.definition(parent_name) else {
            return false;
        };
        definition = parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::{BidderConfigurationError, BidderOverrides, validate_bidder_configuration};
    use auctioneer_bidder::{
        Bidder, BidderConfigError, BidderConfigInput, BidderDeclaration, BidderRegistration,
        RegistryAlias, RegistryInputs, ValidatedRegistryPlan,
    };
    use auctioneer_config::types::{
        BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
    };
    use serde_json::json;
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    #[derive(Debug, Eq, PartialEq)]
    struct Observation {
        owner: &'static str,
        definition: Option<String>,
        enabled: bool,
    }

    thread_local! {
        static OBSERVATIONS: RefCell<Vec<Observation>> = const { RefCell::new(Vec::new()) };
        static REFUSE: Cell<bool> = const { Cell::new(false) };
    }

    fn observe(owner: &'static str, input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
        if input.requested.is_some() || input.base.is_some() {
            return Err(BidderConfigError {
                field: "fake_context",
                reason: "unexpected_supplied_settings",
            });
        }
        OBSERVATIONS.with_borrow_mut(|seen| {
            seen.push(Observation {
                owner,
                definition: input.definition.map(|info| info.name.clone()),
                enabled: input.enabled,
            });
        });
        if REFUSE.get() {
            Err(BidderConfigError {
                field: "fake_field",
                reason: "fake_required",
            })
        } else {
            Ok(())
        }
    }

    fn validate_fake(input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
        observe("fake_owner", input)
    }

    fn validate_other(input: BidderConfigInput<'_>) -> Result<(), BidderConfigError> {
        observe("fake_other_owner", input)
    }

    fn factory() -> Box<dyn Bidder> {
        panic!("configuration validation must never construct bidders")
    }

    fn infos() -> Vec<BidderInfo> {
        [
            ("fake_base", None),
            ("fake_child", Some("fake_base")),
            ("fake_nested", Some("fake_child")),
            ("fake_other", None),
            ("fake_plain", None),
        ]
        .map(|(name, parent)| BidderInfo {
            name: name.into(),
            alias_of: parent.map(str::to_owned),
            ..BidderInfo::default()
        })
        .to_vec()
    }

    fn catalog(rows: Vec<BidderInfo>) -> ValidatedRegistryPlan {
        OBSERVATIONS.with_borrow_mut(Vec::clear);
        REFUSE.set(false);
        let infos = Box::leak(rows.into_boxed_slice());
        let bindings: Vec<_> = infos
            .iter()
            .map(|info| BidderImplementationBinding {
                bidder: info.name.as_str(),
                implementation: match info.name.as_str() {
                    "fake_plain" => "fake_plain_owner",
                    "fake_other" => "fake_other_owner",
                    _ => "fake_owner",
                },
            })
            .collect();
        let metadata: Vec<_> = bindings
            .iter()
            .map(|binding| BidderMetadataDeclaration {
                bidder: binding.bidder,
                implementation: binding.implementation,
                required_feature: None,
            })
            .collect();
        let declarations = [
            BidderDeclaration {
                key: "fake_owner",
                required_feature: None,
                has_settings: true,
            },
            BidderDeclaration {
                key: "fake_other_owner",
                required_feature: None,
                has_settings: true,
            },
            BidderDeclaration {
                key: "fake_plain_owner",
                required_feature: None,
                has_settings: false,
            },
        ];
        let factories = [
            BidderRegistration {
                key: "fake_owner",
                factory,
                config_validator: Some(validate_fake),
            },
            BidderRegistration {
                key: "fake_other_owner",
                factory,
                config_validator: Some(validate_other),
            },
            BidderRegistration {
                key: "fake_plain_owner",
                factory,
                config_validator: None,
            },
        ];
        let aliases = [
            RegistryAlias {
                name: "fake_child",
                source_parent: "fake_base",
                canonical_base: "fake_base",
            },
            RegistryAlias {
                name: "fake_nested",
                source_parent: "fake_child",
                canonical_base: "fake_base",
            },
        ];
        ValidatedRegistryPlan::validate(RegistryInputs {
            infos,
            bindings: &bindings,
            metadata_declarations: &metadata,
            factory_declarations: &declarations,
            factories: &factories,
            aliases: &aliases,
        })
        .expect("fake catalog is valid")
    }

    fn bidders(value: serde_json::Value) -> BTreeMap<String, BidderOverrides> {
        serde_json::from_value(value).expect("fake bidder overrides are typed")
    }

    fn aliases(rows: &[(&str, &str)]) -> BTreeMap<String, String> {
        rows.iter()
            .map(|(name, base)| ((*name).into(), (*base).into()))
            .collect()
    }

    fn no_callbacks() {
        OBSERVATIONS.with_borrow(|seen| assert!(seen.is_empty(), "{seen:?}"));
    }

    #[test]
    fn enabled_definitions_validate_absent_settings_without_factories() {
        let catalog = catalog(infos());
        validate_bidder_configuration(&BTreeMap::new(), &BTreeMap::new(), &catalog).unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 4);
            assert!(
                seen.iter()
                    .all(|item| item.enabled && item.definition.is_some())
            );
        });
    }

    #[test]
    fn callback_refusal_preserves_only_static_identifiers() {
        let catalog = catalog(infos());
        REFUSE.set(true);
        let error = validate_bidder_configuration(&BTreeMap::new(), &BTreeMap::new(), &catalog)
            .unwrap_err();
        assert_eq!(
            error,
            BidderConfigurationError::Bidder(BidderConfigError {
                field: "fake_field",
                reason: "fake_required",
            })
        );
        assert_eq!(
            error.to_string(),
            "bidder configuration invalid: fake_field (fake_required)"
        );
        OBSERVATIONS.with_borrow(|seen| assert_eq!(seen.len(), 1));
    }

    #[test]
    fn identity_union_deduplicates_known_alias_and_override() {
        let catalog = catalog(infos());
        let bidders =
            bidders(json!({"fake_other": {"enabled": true}, "fake_host": {"enabled": true}}));
        let aliases = aliases(&[("fake_other", "fake_base"), ("fake_host", "fake_base")]);
        validate_bidder_configuration(&bidders, &aliases, &catalog).unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 5);
            assert_eq!(
                seen.iter()
                    .filter(|item| item.owner == "fake_other_owner")
                    .count(),
                0
            );
            assert_eq!(
                seen.iter()
                    .filter(|item| item.definition.as_deref() == Some("fake_base"))
                    .count(),
                3
            );
        });
    }

    #[test]
    fn disabled_known_without_settings_needs_no_callback_but_configured_alias_does() {
        let catalog = catalog(infos());
        let bidders =
            bidders(json!({"fake_base": {"enabled": false}, "fake_host": {"enabled": true}}));
        validate_bidder_configuration(&bidders, &aliases(&[("fake_host", "fake_base")]), &catalog)
            .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 2);
            assert!(
                seen.iter()
                    .any(|item| item.definition.as_deref() == Some("fake_base") && !item.enabled)
            );
            assert!(
                seen.iter()
                    .any(|item| item.owner == "fake_other_owner" && item.enabled)
            );
        });
    }

    #[test]
    fn compiled_ancestry_default_enablement_cannot_be_bypassed() {
        let mut infos = infos();
        infos[0].default_enabled = Some(false);
        let catalog = catalog(infos);
        let bidders =
            bidders(json!({"fake_child": {"enabled": true}, "fake_nested": {"enabled": true}}));
        validate_bidder_configuration(&bidders, &aliases(&[("fake_host", "fake_base")]), &catalog)
            .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 2);
            assert!(
                seen.iter()
                    .all(|item| item.owner == "fake_other_owner" || !item.enabled)
            );
        });
    }

    #[test]
    fn explicit_activation_restores_default_disabled_base_and_descendants() {
        let mut infos = infos();
        infos[0].default_enabled = Some(false);
        let catalog = catalog(infos);
        validate_bidder_configuration(
            &bidders(json!({"fake_base": {"enabled": true}})),
            &BTreeMap::new(),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 4);
            assert!(seen.iter().all(|item| item.enabled));
        });
    }

    #[test]
    fn immutable_disabled_definition_cannot_be_activated() {
        let mut infos = infos();
        infos[1].disabled = true;
        let catalog = catalog(infos);
        validate_bidder_configuration(
            &bidders(json!({"fake_child": {"enabled": true}, "fake_nested": {"enabled": true}})),
            &BTreeMap::new(),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 2);
            assert!(
                seen.iter()
                    .all(|item| item.definition.as_deref() != Some("fake_child")
                        && item.definition.as_deref() != Some("fake_nested"))
            );
        });
    }

    #[test]
    fn shadowed_compiled_identity_keeps_its_immutable_activation_gate() {
        let mut infos = infos();
        infos[3].disabled = true;
        let catalog = catalog(infos);
        validate_bidder_configuration(
            &bidders(json!({"fake_other": {"enabled": true}})),
            &aliases(&[("fake_other", "fake_base")]),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 4);
            assert_eq!(seen.iter().filter(|item| !item.enabled).count(), 1);
            assert!(seen.iter().all(|item| item.owner == "fake_owner"));
        });
    }

    #[test]
    fn host_alias_cannot_target_unknown_base() {
        let catalog = catalog(infos());
        assert_eq!(
            validate_bidder_configuration(
                &BTreeMap::new(),
                &aliases(&[("fake_host", "fake_missing")]),
                &catalog,
            ),
            Err(BidderConfigurationError::InvalidAliasBase)
        );
        no_callbacks();
    }

    #[test]
    fn host_alias_can_target_compiled_alias_without_bypassing_ancestry() {
        let mut infos = infos();
        infos[1].default_enabled = Some(false);
        let catalog = catalog(infos);
        validate_bidder_configuration(
            &bidders(json!({"fake_host": {"enabled": true}, "fake_nested": {"enabled": true}})),
            &aliases(&[("fake_host", "fake_nested")]),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 3);
            assert!(
                seen.iter()
                    .any(|item| item.definition.as_deref() == Some("fake_nested") && !item.enabled)
            );
        });
    }

    #[test]
    fn host_alias_targets_are_not_recursively_resolved() {
        let catalog = catalog(infos());
        validate_bidder_configuration(
            &BTreeMap::new(),
            &aliases(&[("fake_other", "fake_base"), ("fake_host", "fake_other")]),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 5);
            assert_eq!(
                seen.iter()
                    .filter(|item| item.owner == "fake_other_owner")
                    .count(),
                1
            );
            assert!(
                seen.iter()
                    .any(|item| item.definition.as_deref() == Some("fake_other"))
            );
        });
    }

    #[test]
    fn immutable_disabled_host_base_keeps_configured_alias_disabled() {
        let mut infos = infos();
        infos[0].disabled = true;
        let catalog = catalog(infos);
        validate_bidder_configuration(
            &bidders(json!({"fake_base": {"enabled": true}, "fake_host": {"enabled": true}})),
            &aliases(&[("fake_host", "fake_base")]),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 2);
            assert!(
                seen.iter()
                    .any(|item| item.definition.as_deref() == Some("fake_base") && !item.enabled)
            );
        });
    }

    #[test]
    fn namespace_collisions_are_rejected_before_any_callback() {
        let catalog = catalog(infos());
        for (bidders, aliases) in [
            (
                bidders(json!({"fake_unknown": {}, "FAKE_UNKNOWN": {}})),
                BTreeMap::new(),
            ),
            (
                BTreeMap::new(),
                aliases(&[("fake_host", "fake_base"), ("FAKE_HOST", "fake_base")]),
            ),
            (
                bidders(json!({"FAKE_HOST": {}})),
                aliases(&[("fake_host", "fake_base")]),
            ),
            (bidders(json!({"FAKE_CHILD": {}})), BTreeMap::new()),
            (BTreeMap::new(), aliases(&[("FAKE_CHILD", "fake_base")])),
        ] {
            assert_eq!(
                validate_bidder_configuration(&bidders, &aliases, &catalog),
                Err(BidderConfigurationError::NamespaceCollision)
            );
            no_callbacks();
        }
    }

    #[test]
    fn invalid_disabled_override_is_sanitized_before_any_callback() {
        let catalog = catalog(infos());
        let bidders =
            bidders(json!({"fake_secret_canary": {"enabled": false, "timeout_ms": 0_u32}}));
        let error =
            validate_bidder_configuration(&bidders, &BTreeMap::new(), &catalog).unwrap_err();
        assert_eq!(error, BidderConfigurationError::Structure);
        assert_eq!(error.to_string(), "bidder configuration structure invalid");
        assert!(!format!("{error:?}").contains("fake_secret_canary"));
        no_callbacks();
    }

    #[test]
    fn name_and_alias_bounds_are_checked_before_callbacks() {
        let catalog = catalog(infos());
        let oversized = "x".repeat(super::super::MAX_BIDDER_NAME_BYTES + 1);
        for (bidders, aliases) in [
            (
                BTreeMap::from([(oversized.clone(), BidderOverrides::default())]),
                BTreeMap::new(),
            ),
            (
                BTreeMap::new(),
                BTreeMap::from([(oversized.clone(), "fake_base".into())]),
            ),
            (
                BTreeMap::new(),
                BTreeMap::from([("fake_host".into(), oversized)]),
            ),
            (BTreeMap::new(), aliases(&[("", "fake_base")])),
            (
                BTreeMap::new(),
                (0..=super::super::MAX_ALIASES)
                    .map(|i| (format!("fake_host_{i}"), "fake_base".into()))
                    .collect(),
            ),
        ] {
            assert_eq!(
                validate_bidder_configuration(&bidders, &aliases, &catalog),
                Err(BidderConfigurationError::Structure)
            );
            no_callbacks();
        }
    }

    #[test]
    fn unknown_settings_free_overrides_do_not_become_resolved_identities() {
        let catalog = catalog(infos());
        validate_bidder_configuration(
            &bidders(json!({"fake_future": {"enabled": true}})),
            &BTreeMap::new(),
            &catalog,
        )
        .unwrap();
        OBSERVATIONS.with_borrow(|seen| {
            assert_eq!(seen.len(), 4);
            assert!(seen.iter().all(|item| item.definition.is_some()));
        });
    }
}
