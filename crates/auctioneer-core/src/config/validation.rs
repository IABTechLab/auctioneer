//! Shared effective policy validation and bidder-owned subvalidation.
//! Validation does not install state or admit a serving snapshot.

use super::{BidderOverrides, MAX_BIDDER_NAME_BYTES, validate_host_alias_overlay};
use auctioneer_bidder::BidderSettings;
use auctioneer_bidder::{
    BidderConfigError, BidderConfigInput, RegistryDefinition, ValidatedRegistryPlan,
};
use auctioneer_config::usersync::validate_catalog_size;
use auctioneer_config::{
    override_value::FieldOverride,
    source::AuctionConfig,
    types::{AccountConfig, HostConfig, ResourceLimits},
    usersync::{validate_host_cookie, validate_priority_groups, validate_user_sync},
};
use auctioneer_runtime::ResolvedIsolateLimits;
use std::collections::{BTreeMap, BTreeSet};
use validator::Validate as _;

/// Explicit, borrowed inputs for non-installing policy validation.
/// Product defaults and the linked catalog belong to the caller; the capacity
/// row must already be normalized and resource-validated by startup. This does
/// not establish input/storage funding or construct an admitted snapshot.
pub struct ConfigValidationContext<'a> {
    pub product_host: &'a HostConfig,
    pub product_account: &'a AccountConfig,
    pub catalog: &'a ValidatedRegistryPlan,
    pub limits: &'a ResolvedIsolateLimits,
    pub now_unix_seconds: i64,
}

/// Fixed policy refusal categories; no operator values or names are retained.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PolicyValidationError {
    #[error("operator policy structure invalid")]
    Structure,
    #[error("operator policy resource limits invalid")]
    ResourceLimits,
    #[error("operator policy host cookie invalid")]
    HostCookie,
    #[error("operator policy user sync invalid")]
    UserSync,
    #[error("operator policy account sync invalid")]
    AccountSync,
    #[error("operator policy floor currency expired")]
    CurrencyExpired,
    #[error("operator policy bidder activation invalid")]
    Activation,
    #[error(transparent)]
    Bidder(#[from] BidderConfigurationError),
}

/// Preserve reviewed policy categories until the SDK builds safe diagnostics.
#[derive(Debug)]
pub(super) enum CompiledPolicyError {
    Registry(auctioneer_bidder::RegistryBuildError),
    Policy(PolicyValidationError),
}

/// Validate a complete policy without installing it or reading ambient state.
/// Callers must first bound and fund the source, merged views, namespace and
/// validation temporaries. Success is not snapshot or startup admission.
///
/// # Errors
/// Returns a fixed shared category or static bidder-owned field/reason pair.
pub fn validate_operator_policy(
    policy: &super::policy::OperatorPolicy,
    context: &ConfigValidationContext<'_>,
) -> Result<(), PolicyValidationError> {
    validate_policy(
        policy,
        context.product_host,
        context.product_account,
        context.catalog,
        Some((context.limits, context.now_unix_seconds)),
    )
}

/// Validate structure and effective policy against this build's compiled join.
/// This is the common push/SDK path. It constructs no bidder, reads no clock,
/// applies no startup capacity and retains no catalog indexes after returning.
///
/// # Errors
/// Preserves immutable registry failures separately from malformed policy.
pub fn validate_compiled_operator_policy(
    policy: &super::policy::OperatorPolicy,
    product_host: &HostConfig,
    product_account: &AccountConfig,
) -> Result<(), super::EffectiveConfigError> {
    validate_compiled_policy(policy, product_host, product_account).map_err(|error| match error {
        CompiledPolicyError::Registry(registry_error) => {
            super::EffectiveConfigError::Registry(registry_error)
        }
        CompiledPolicyError::Policy(policy_error) => {
            super::EffectiveConfigError::Malformed(policy_error.to_string())
        }
    })
}

pub(super) fn validate_compiled_policy(
    policy: &super::policy::OperatorPolicy,
    product_host: &HostConfig,
    product_account: &AccountConfig,
) -> Result<(), CompiledPolicyError> {
    let catalog = ValidatedRegistryPlan::compiled().map_err(CompiledPolicyError::Registry)?;
    validate_effective_operator_policy(policy, product_host, product_account, &catalog)
        .map_err(CompiledPolicyError::Policy)
}

/// Validate structure, merged semantics, settings bindings and bidder callbacks.
/// Uses only the supplied product defaults and catalog. Floor-currency shape
/// is validated here; freshness remains the auction request clock's decision.
/// This is not storage, capacity or snapshot admission.
///
/// # Errors
/// Returns fixed shared categories or static bidder-owned field/reason IDs.
pub fn validate_effective_operator_policy(
    policy: &super::policy::OperatorPolicy,
    product_host: &HostConfig,
    product_account: &AccountConfig,
    catalog: &ValidatedRegistryPlan,
) -> Result<(), PolicyValidationError> {
    validate_policy(policy, product_host, product_account, catalog, None)
}

fn validate_policy(
    policy: &super::policy::OperatorPolicy,
    product_host: &HostConfig,
    product_account: &AccountConfig,
    catalog: &ValidatedRegistryPlan,
    admission: Option<(&ResolvedIsolateLimits, i64)>,
) -> Result<(), PolicyValidationError> {
    policy
        .validate()
        .map_err(|_error| PolicyValidationError::Structure)?;
    let mut host = super::policy_host::requested_policy_host(product_host, &policy.host);
    validate_requested_resources(&host.resource_limits)?;
    validate_host_cookie(&host.host_cookie).map_err(|_error| PolicyValidationError::HostCookie)?;
    if let Some(sync) = &host.user_sync {
        validate_user_sync(sync, cfg!(feature = "insecure-bidders"))
            .map_err(|_error| PolicyValidationError::UserSync)?;
    }
    // Sync priorities replace whole lists. Validate the effective default once
    // and each supplied replacement; inheriting accounts need no copied state.
    let defaults = match policy
        .account_defaults
        .cookie_sync
        .as_ref()
        .map(|sync| &sync.priority_groups)
    {
        Some(FieldOverride::Value(groups)) => Some(groups.as_slice()),
        Some(FieldOverride::Clear) => None,
        None | Some(FieldOverride::Inherit) => {
            product_account.cookie_sync.priority_groups.as_deref()
        }
    };
    if let Some(groups) = defaults {
        validate_priority_groups(groups).map_err(|_error| PolicyValidationError::AccountSync)?;
    }
    for overlay in policy.accounts.values() {
        if let Some(sync) = &overlay.cookie_sync
            && let FieldOverride::Value(groups) = &sync.priority_groups
        {
            validate_priority_groups(groups)
                .map_err(|_error| PolicyValidationError::AccountSync)?;
        }
    }
    if let Some((funded, now_unix_seconds)) = admission {
        if policy
            .floor_currency
            .as_ref()
            .is_some_and(|snapshot| now_unix_seconds >= snapshot.expires_at)
        {
            return Err(PolicyValidationError::CurrencyExpired);
        }
        // Validate requests before clamping. Above-cap requests retain existing
        // clamp semantics; a zero funded cap is never an executable row.
        super::resolve::clamp_host_to_boot(&mut host, funded);
        let limits = &host.resource_limits;
        if limits.max_bidders_per_auction == 0
            || limits.max_outbound_requests == 0
            || limits.max_bidder_response_bytes == 0
            || limits.max_outbound_raw_bytes == 0
            || limits.max_outbound_request_bytes == 0
            || limits.max_transport_bytes == 0
            || limits.max_decoded_bytes == 0
            || limits.max_parsed_nodes == 0
        {
            return Err(PolicyValidationError::ResourceLimits);
        }
    }
    for (name, entry) in &policy.bidders {
        if entry.enabled == Some(true) {
            let target = host.aliases.get(name).map_or(name.as_str(), String::as_str);
            if immutable_disabled(name, catalog) || immutable_disabled(target, catalog) {
                return Err(PolicyValidationError::Activation);
            }
        }
    }
    // The root already validated every supplied settings object. Reuse the same
    // semantic dispatcher without repeating nested structural validation.
    validate_bidder_context(&policy.bidders, &host.aliases, catalog)?;
    Ok(())
}

fn validate_requested_resources(limits: &ResourceLimits) -> Result<(), PolicyValidationError> {
    AuctionConfig {
        default_tmax_ms: Some(limits.default_tmax_ms),
        max_tmax_ms: Some(limits.max_tmax_ms),
        max_bidders_per_auction: Some(limits.max_bidders_per_auction),
        max_outbound_requests: Some(limits.max_outbound_requests),
        max_bidder_response_bytes: Some(limits.max_bidder_response_bytes),
        max_outbound_raw_bytes: Some(limits.max_outbound_raw_bytes),
        max_outbound_request_bytes: Some(limits.max_outbound_request_bytes),
        max_transport_bytes: Some(limits.max_transport_bytes),
        max_decoded_bytes: Some(limits.max_decoded_bytes),
        max_parsed_nodes: Some(limits.max_parsed_nodes),
    }
    .validate()
    .map_err(|_error| PolicyValidationError::ResourceLimits)?;
    if limits.default_tmax_ms > limits.max_tmax_ms {
        return Err(PolicyValidationError::ResourceLimits);
    }
    Ok(())
}

fn immutable_disabled(name: &str, catalog: &ValidatedRegistryPlan) -> bool {
    let Some(mut definition) = catalog.definition(name) else {
        return false;
    };
    for _ in 0..catalog.definitions().len() {
        if definition.info().disabled {
            return true;
        }
        let Some(parent) = definition
            .source_parent()
            .and_then(|parent_name| catalog.definition(parent_name))
        else {
            return false;
        };
        definition = parent;
    }
    true
}

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

/// Pure bidder subvalidator, not complete `OperatorPolicy` validation
/// or snapshot admission. `aliases` must already be the effective host map.
/// No defaults, accounts, globals, factories, I/O or secrets are resolved here.
///
/// Callers must supply bounded, caller-owned input and fund temporary namespace
/// folding (map nodes and folded names), borrowed identity-set nodes (at most
/// catalog definitions + host aliases + explicit settings), and shared/bidder
/// validation-error peaks before activation. This API supplies neither
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
    for entry in bidders.values() {
        entry
            .validate()
            .map_err(|_error| BidderConfigurationError::Structure)?;
    }
    validate_bidder_context(bidders, aliases, catalog)
}

fn validate_bidder_context(
    bidders: &BTreeMap<String, BidderOverrides>,
    aliases: &BTreeMap<String, String>,
    catalog: &ValidatedRegistryPlan,
) -> Result<(), BidderConfigurationError> {
    validate_catalog_size(catalog.definitions().len(), aliases.len())
        .map_err(|_error| BidderConfigurationError::Structure)?;
    // The caller validated the supplied overlay. This map also includes
    // product aliases, so its population is bounded by the complete catalog,
    // not the smaller per-publication overlay allowance.
    if aliases.iter().any(|(name, base)| {
        name.is_empty() || name.len() > MAX_BIDDER_NAME_BYTES || base.len() > MAX_BIDDER_NAME_BYTES
    }) || bidders
        .keys()
        .any(|name| name.len() > MAX_BIDDER_NAME_BYTES)
    {
        return Err(BidderConfigurationError::Structure);
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
    // Walk the same activation gates as serving through validated source
    // ancestry, without cloning a whole application model.
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
    use crate::config::{policy::OperatorPolicy, policy_host::resolve_policy_host};
    use crate::test_support::boot_parity_fixture::profile_for_adapter;
    use auctioneer_bidder::{
        Bidder, BidderConfigError, BidderConfigInput, BidderDeclaration, BidderRegistration,
        RegistryAlias, RegistryInputs, ValidatedRegistryPlan,
    };
    use auctioneer_config_data::types::{
        AccountConfig, BidderImplementationBinding, BidderInfo, BidderMetadataDeclaration,
        HostConfig,
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

    fn context<'a>(
        host: &'a HostConfig,
        account: &'a AccountConfig,
        catalog: &'a ValidatedRegistryPlan,
        limits: &'a auctioneer_runtime::ResolvedIsolateLimits,
    ) -> super::ConfigValidationContext<'a> {
        super::ConfigValidationContext {
            product_host: host,
            product_account: account,
            catalog,
            limits,
            now_unix_seconds: 1_000,
        }
    }

    fn policy(document: serde_json::Value) -> OperatorPolicy {
        serde_json::from_value(document).expect("fake policy")
    }

    fn limits() -> auctioneer_runtime::ResolvedIsolateLimits {
        profile_for_adapter("unequal_explicit_caps", "axum").limits
    }

    #[test]
    fn effective_policy_validation_has_no_operation_clock_or_capacity_dependency() {
        let catalog = catalog(infos());
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let policy = policy(json!({"schema_version": 1_i32, "floor_currency": {
            "expires_at": 1_i64, "usd_per_unit": {"QAA": 1.25_f64}
        }}));
        assert_eq!(
            super::validate_effective_operator_policy(&policy, host, account, &catalog),
            Ok(())
        );
        OBSERVATIONS.with_borrow(|seen| assert_eq!(seen.len(), 4));
        OBSERVATIONS.with_borrow_mut(Vec::clear);
        let limits = limits();
        let mut explicit = context(host, account, &catalog, &limits);
        explicit.now_unix_seconds = 1;
        assert_eq!(
            super::validate_operator_policy(&policy, &explicit),
            Err(super::PolicyValidationError::CurrencyExpired)
        );
        no_callbacks();
        assert_eq!(
            super::validate_effective_operator_policy(&policy, host, account, &catalog),
            Ok(())
        );
        OBSERVATIONS.with_borrow(|seen| assert_eq!(seen.len(), 4));
    }

    #[test]
    fn effective_policy_validation_reuses_structure_semantics_activation_and_dispatch() {
        use super::{PolicyValidationError, validate_effective_operator_policy};
        let mut rows = infos();
        rows[0].disabled = true;
        let catalog = catalog(rows);
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        for (document, expected) in [
            (
                json!({"schema_version": 1_i32, "host": {"resource_limits": {"default_tmax_ms": 0_i32}}}),
                PolicyValidationError::Structure,
            ),
            (
                json!({"schema_version": 1_i32, "host": {"resource_limits": {"default_tmax_ms": 31_i32, "max_tmax_ms": 30_i32}}}),
                PolicyValidationError::ResourceLimits,
            ),
            (
                json!({"schema_version": 1_i32, "host": {"host_cookie": {"cookie_name": "fake;private"}}}),
                PolicyValidationError::HostCookie,
            ),
            (
                json!({"schema_version": 1_i32, "account_defaults": {"cookie_sync": {"priority_groups": [["fake\nprivate"]]}}}),
                PolicyValidationError::AccountSync,
            ),
            (
                json!({"schema_version": 1_i32, "bidders": {"fake_nested": {"enabled": true}}}),
                PolicyValidationError::Activation,
            ),
            (
                json!({"schema_version": 1_i32, "host": {"aliases": {"fake_alias": "fake_missing"}}}),
                PolicyValidationError::Bidder(BidderConfigurationError::InvalidAliasBase),
            ),
        ] {
            assert_eq!(
                validate_effective_operator_policy(&policy(document), host, account, &catalog),
                Err(expected)
            );
            no_callbacks();
        }
        REFUSE.set(true);
        assert_eq!(
            validate_effective_operator_policy(&OperatorPolicy::default(), host, account, &catalog),
            Err(PolicyValidationError::Bidder(
                BidderConfigurationError::Bidder(BidderConfigError {
                    field: "fake_field",
                    reason: "fake_required"
                })
            ))
        );
        OBSERVATIONS.with_borrow(|seen| assert_eq!(seen.len(), 1));
    }

    #[test]
    fn validation_uses_its_context_without_boot_installation() {
        use super::{BidderConfigurationError, PolicyValidationError, validate_operator_policy};
        let first = catalog(infos());
        let second = ValidatedRegistryPlan::validate(RegistryInputs {
            infos: &[],
            bindings: &[],
            metadata_declarations: &[],
            factory_declarations: &[],
            factories: &[],
            aliases: &[],
        })
        .unwrap();
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let limits = limits();
        let policy = policy(json!({"schema_version": 1_u64, "host": {
            "aliases": {"fake_host": "fake_base"}
        }}));
        assert_eq!(
            validate_operator_policy(&policy, &context(host, account, &first, &limits)),
            Ok(())
        );
        assert_eq!(
            validate_operator_policy(&policy, &context(host, account, &second, &limits)),
            Err(PolicyValidationError::Bidder(
                BidderConfigurationError::InvalidAliasBase
            ))
        );
        assert_eq!(
            validate_operator_policy(&policy, &context(host, account, &first, &limits)),
            Ok(())
        );
    }

    #[test]
    fn policy_rejects_invalid_requested_values_before_capacity_clamping() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let mut limits = limits();
        limits.max_transport_bytes = 1024;
        let context = context(
            auctioneer_config_data::product_host_default(),
            auctioneer_config_data::product_account_default(),
            &catalog,
            &limits,
        );
        for document in [
            json!({"schema_version": 1_u64, "host": {"resource_limits": {"max_transport_bytes": 104_857_601_u64}}}),
            json!({"schema_version": 1_u64, "account_defaults": {"max_decoded_bytes": 0_u64}}),
            json!({"schema_version": 1_u64, "accounts": {"fake_account": {"max_transport_bytes": 0_u64}}}),
        ] {
            assert_eq!(
                validate_operator_policy(&policy(document), &context),
                Err(PolicyValidationError::Structure)
            );
            no_callbacks();
        }
    }

    #[test]
    fn policy_checks_effective_deadline_order_across_product_and_overlay() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let mut host = auctioneer_config_data::product_host_default().clone();
        host.resource_limits.default_tmax_ms = 31;
        host.resource_limits.max_tmax_ms = 37;
        let limits = limits();
        let context = context(
            &host,
            auctioneer_config_data::product_account_default(),
            &catalog,
            &limits,
        );
        let bad = policy(
            json!({"schema_version": 1_u64, "host": {"resource_limits": {"max_tmax_ms": 30_i64}}}),
        );
        assert_eq!(
            validate_operator_policy(&bad, &context),
            Err(PolicyValidationError::ResourceLimits)
        );
        no_callbacks();
        let equal = policy(
            json!({"schema_version": 1_u64, "host": {"resource_limits": {"max_tmax_ms": 31_i64}}}),
        );
        assert_eq!(validate_operator_policy(&equal, &context), Ok(()));
    }

    #[test]
    fn policy_cookie_and_sync_checks_use_effective_values_without_raw_diagnostics() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let limits = limits();
        let context = context(host, account, &catalog, &limits);
        for (document, expected) in [
            (
                json!({"schema_version": 1_u64, "host": {"host_cookie": {"cookie_name": "fake_private_canary;cookie"}}}),
                PolicyValidationError::HostCookie,
            ),
            (
                json!({"schema_version": 1_u64, "host": {"user_sync": {"external_url": "https://fake_private_canary@sync.example.test"}}}),
                PolicyValidationError::UserSync,
            ),
        ] {
            let error = validate_operator_policy(&policy(document), &context).unwrap_err();
            assert_eq!(error, expected);
            assert!(!format!("{error:?} {error}").contains("fake_private_canary"));
            no_callbacks();
        }
        let mut bad_product = host.clone();
        bad_product.host_cookie.cookie_name = "fake_private_canary;cookie".into();
        assert_eq!(
            validate_operator_policy(
                &OperatorPolicy::default(),
                &super::ConfigValidationContext {
                    product_host: &bad_product,
                    ..context
                }
            ),
            Err(PolicyValidationError::HostCookie)
        );
    }

    #[test]
    fn policy_account_sync_validation_covers_defaults_named_and_inherited_values() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let limits = limits();
        let context = context(host, account, &catalog, &limits);
        for scope in ["account_defaults", "accounts"] {
            let row = json!({"cookie_sync": {"priority_groups": [["fake_private_canary\n"]]}});
            let document = if scope == "accounts" {
                json!({"schema_version": 1_u64, "accounts": {"fake_private_account": row}})
            } else {
                json!({"schema_version": 1_u64, "account_defaults": row})
            };
            assert_eq!(
                validate_operator_policy(&policy(document), &context),
                Err(PolicyValidationError::AccountSync)
            );
            no_callbacks();
        }
        let mut inherited = account.clone();
        inherited.cookie_sync.priority_groups = Some(vec![vec!["fake_private_canary\n".into()]]);
        assert_eq!(
            validate_operator_policy(
                &OperatorPolicy::default(),
                &super::ConfigValidationContext {
                    product_account: &inherited,
                    ..context
                }
            ),
            Err(PolicyValidationError::AccountSync)
        );
    }

    #[test]
    fn policy_floor_currency_expiry_uses_only_the_supplied_time() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let limits = limits();
        let mut context = context(
            auctioneer_config_data::product_host_default(),
            auctioneer_config_data::product_account_default(),
            &catalog,
            &limits,
        );
        let policy = policy(json!({"schema_version": 1_u64, "floor_currency": {
            "expires_at": 1001_i64, "usd_per_unit": {"QAA": 1.25_f64}
        }}));
        assert_eq!(validate_operator_policy(&policy, &context), Ok(()));
        context.now_unix_seconds = 1001;
        assert_eq!(
            validate_operator_policy(&policy, &context),
            Err(PolicyValidationError::CurrencyExpired)
        );
        context.now_unix_seconds = 1002;
        assert_eq!(
            validate_operator_policy(&policy, &context),
            Err(PolicyValidationError::CurrencyExpired)
        );
        context.now_unix_seconds = 1000;
        assert_eq!(validate_operator_policy(&policy, &context), Ok(()));
    }

    #[test]
    fn policy_refuses_enabling_an_immutable_disabled_definition() {
        use super::{PolicyValidationError, validate_operator_policy};
        let mut rows = infos();
        rows[0].disabled = true;
        let catalog = catalog(rows);
        let limits = limits();
        let context = context(
            auctioneer_config_data::product_host_default(),
            auctioneer_config_data::product_account_default(),
            &catalog,
            &limits,
        );
        for name in ["fake_base", "fake_child", "fake_host"] {
            let policy = policy(json!({"schema_version": 1_u64,
                "host": {"aliases": {"fake_host": "fake_base"}},
                "bidders": {name: {"enabled": true}}
            }));
            assert_eq!(
                validate_operator_policy(&policy, &context),
                Err(PolicyValidationError::Activation)
            );
            no_callbacks();
        }
    }

    #[test]
    fn policy_runs_the_same_bidder_dispatcher_once_and_propagates_fixed_refusal() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let limits = limits();
        let context = context(
            auctioneer_config_data::product_host_default(),
            auctioneer_config_data::product_account_default(),
            &catalog,
            &limits,
        );
        assert_eq!(
            validate_operator_policy(&OperatorPolicy::default(), &context),
            Ok(())
        );
        OBSERVATIONS.with_borrow(|seen| assert_eq!(seen.len(), 4));
        OBSERVATIONS.with_borrow_mut(Vec::clear);
        REFUSE.set(true);
        assert_eq!(
            validate_operator_policy(&OperatorPolicy::default(), &context),
            Err(PolicyValidationError::Bidder(
                BidderConfigurationError::Bidder(BidderConfigError {
                    field: "fake_field",
                    reason: "fake_required"
                })
            ))
        );
        OBSERVATIONS.with_borrow(|seen| assert_eq!(seen.len(), 1));
    }

    #[test]
    fn policy_capacity_validation_keeps_valid_clamps_and_refuses_zero_funding() {
        use super::{PolicyValidationError, validate_operator_policy};
        let catalog = catalog(infos());
        let host = auctioneer_config_data::product_host_default();
        let account = auctioneer_config_data::product_account_default();
        let policy = policy(json!({"schema_version": 1_u64, "host": {
            "resource_limits": {"max_transport_bytes": 0x0001_0000_u64}
        }}));
        let mut limits = limits();
        limits.max_transport_bytes = 1024;
        assert_eq!(
            validate_operator_policy(&policy, &context(host, account, &catalog, &limits)),
            Ok(())
        );
        let resolved = resolve_policy_host(host, &policy, &limits);
        assert_eq!(resolved.resource_limits.max_transport_bytes, 1024);
        let axes: [fn(&mut auctioneer_runtime::ResolvedIsolateLimits); 7] = [
            |row| row.max_bidders_per_auction = 0,
            |row| row.max_outbound_requests = 0,
            |row| row.max_outbound_raw_bytes = 0,
            |row| row.max_outbound_request_bytes = 0,
            |row| row.max_transport_bytes = 0,
            |row| row.max_decoded_bytes = 0,
            |row| row.max_parsed_nodes = 0,
        ];
        for axis in axes {
            OBSERVATIONS.with_borrow_mut(Vec::clear);
            let mut bad = limits;
            axis(&mut bad);
            assert_eq!(
                validate_operator_policy(&policy, &context(host, account, &catalog, &bad)),
                Err(PolicyValidationError::ResourceLimits)
            );
            no_callbacks();
        }
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
