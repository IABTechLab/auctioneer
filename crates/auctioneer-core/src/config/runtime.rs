//! The single runtime-config resolution owner.
//!
//! Every config-consuming route (`/openrtb2/auction`, `/status`,
//! `/info/bidders`, `/info/bidders/<name>`) goes through
//! [`resolve_runtime_config`] rather than calling the `EdgeZero`
//! `AppConfig` extractor itself, so the load policy — and the wire shape
//! of a load failure — is defined in exactly one place.
//!
//! That policy is fail-closed: a missing, corrupt or untrusted blob is a 500,
//! so is a missing binding or a store key the backend refuses, and a store outage is a 503,
//! because serving any of them from the baked default would silently
//! turn GDPR enforcement off. See [`resolve_runtime_config`] for the
//! per-class reasoning.
//!
//! Fail-closed is the default, not a universal rule. It buys exactly one
//! thing — no privacy decision made from config nobody can vouch for — so
//! a route that makes NO privacy decision pays its availability cost for
//! nothing. Such a route calls [`RuntimeConfigError::degrade_to_default`]
//! instead of [`RuntimeConfigError::into_response`]. Both dispositions
//! live here so a reader sees which routes take which, and why, in one
//! place rather than reconstructing it from four handlers.

use crate::bidder_registry::{cached_registry_error, registry_handle};
use crate::config::{AuctioneerAppConfig, SdkAuctioneerAppConfig};
use crate::configure::extraction_limits_for;
use crate::host_primitives::boot_gates;
use edgezero_core::body::Body;
use edgezero_core::config_store::ConfigExtractionLimits;
use edgezero_core::context::RequestContext;
use edgezero_core::error::{EdgeError, StoreExtractionReason};
use edgezero_core::extractor::AppConfig;
use edgezero_core::http::{Response, StatusCode, response_builder};
use std::time::Duration;

/// The logical config-store id the deploy binds `auctioneer.toml` to.
const CONFIG_STORE_ID: &str = "auctioneer_runtime";

/// Fixed bound for the SDK's root-config and secret extraction, independent of operator policy.
pub const CONFIG_FETCH_TIMEOUT: Duration = Duration::from_secs(2);

/// `Retry-After` for a transient config-store outage. The pinned
/// `EdgeZero` `ServiceUnavailable` carries no backend hint, so this is
/// the no-hint default the fail-loud contract specifies.
const CONFIG_STORE_RETRY_AFTER: Duration = Duration::from_secs(5);

/// Response-header name carrying [`ConfigStatusCode`]'s verdict on every
/// `/status` answer. It is the MACHINE channel: a monitoring rule alerts
/// on `degraded` without parsing a body, and it works on the empty-body
/// shape, which has nothing else to carry the signal.
pub const CONFIG_STATUS_HEADER: &str = "x-auctioneer-config-status";

/// The PUBLIC vocabulary for a config fault, closed and carrying no
/// value-derived text by construction.
///
/// `/status` is unauthenticated, so it publishes this and nothing else.
/// [`RuntimeConfigError`]'s `detail` never reaches the wire:
/// `validate_alias_bases` can interpolate store-controlled alias and base names,
/// so publishing it would hand any caller fragments of the deploy's
/// configuration chosen by whoever can write the config store. `detail`
/// goes to the log line, which an operator reads with deploy credentials.
///
/// Each variant names a different operator action: fix the blob, fix the
/// key binding, wait for the backend, or rebuild the App after funded boot.
/// Finer granularity would push toward naming WHICH FIELD was wrong,
/// which is the same leak in another form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigStatusCode {
    /// The compiled registry cannot be constructed; rebuilding is required.
    RegistryBuildFailed,
    /// The blob is unparseable, untrusted or invalid.
    MalformedConfig,
    /// The backend refused the bound key, so no push can ever land where
    /// this deploy looks — a different fix from a corrupt blob, which is
    /// why the two do not share a code.
    StoreKeyRejected,
    /// A transient backend failure. The only code whose answer is to
    /// wait.
    StoreUnavailable,
    /// The captured extractor caps have no installed boot funding.
    UnfundedExtraction,
}

impl ConfigStatusCode {
    /// The wire string, fixed here rather than taken from the `Debug`
    /// impl so the published contract survives a variant rename.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RegistryBuildFailed => "registry_build_failed",
            Self::MalformedConfig => "malformed_config",
            Self::StoreKeyRejected => "store_key_rejected",
            Self::StoreUnavailable => "store_unavailable",
            Self::UnfundedExtraction => "unfunded_extraction",
        }
    }
}

/// Closed reasons for refusing extraction before any store work begins.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnfundedExtractionReason {
    /// The complete boot gates have not been installed.
    BootMissing,
    /// At least one captured extraction cap exceeds its installed funding.
    CapturedCapsAboveFunding,
}

impl UnfundedExtractionReason {
    const fn as_str(self) -> &'static str {
        match self {
            Self::BootMissing => "boot_missing",
            Self::CapturedCapsAboveFunding => "captured_caps_above_funding",
        }
    }
}

/// A core-owned error type rather than `EdgeError`: the pinned `EdgeZero`
/// serialises `EdgeError` as JSON and offers no `Retry-After` control,
/// while the fail-loud contract this type exists to serve is a
/// plain-text 500 / 503 plus `Retry-After: {n}s`. Callers pair
/// [`resolve_runtime_config`] with [`RuntimeConfigError::into_response`]
/// to produce those exact wire bytes.
#[derive(Debug)]
pub enum RuntimeConfigError {
    /// Fatal immutable build failure, never a degradable runtime blob error.
    RegistryBuildFailed {
        error: auctioneer_bidder::RegistryBuildError,
    },
    /// Extraction was refused before a read because boot did not fund
    /// the context's captured caps. Carries no context or store values.
    UnfundedExtraction { reason: UnfundedExtractionReason },
    /// The store holds a blob this build cannot trust: the envelope
    /// failed to parse, its recorded sha did not match the canonical
    /// hash of its own `data`, or it declares an envelope version this
    /// build does not understand — what `EdgeZero` calls "a
    /// corrupt/tampered store entry".
    ///
    /// Not a fresh deploy that will heal itself, and serving a request
    /// from the baked default instead would silently drop GDPR
    /// enforcement on a deployment whose config is broken.
    MalformedHost { detail: String },
    /// The store rejected the key the binding told it to read, so no
    /// blob can ever be pushed where this deploy looks for one.
    ///
    /// Its DISPOSITION is [`Self::MalformedHost`]'s, unchanged: 500,
    /// fail-closed, no `Retry-After`, because retrying a key the backend
    /// will never accept cannot fix it. The variant exists so `/status`
    /// can name the fault distinctly — a refused key and a corrupt blob
    /// have different fixes, and collapsing them leaves an operator
    /// unable to tell which one they are looking at. That makes this a
    /// reporting split rather than a policy change.
    StoreKeyRejected { detail: String },
    /// The config-store backend is transiently unable to answer. A
    /// transient failure is not a fresh deploy, so it must not fall back
    /// to the baked default either.
    ConfigStoreUnavailable {
        detail: String,
        retry_after: Duration,
    },
}

impl From<auctioneer_bidder::RegistryBuildError> for RuntimeConfigError {
    fn from(error: auctioneer_bidder::RegistryBuildError) -> Self {
        Self::RegistryBuildFailed { error }
    }
}

impl From<super::EffectiveConfigError> for RuntimeConfigError {
    fn from(error: super::EffectiveConfigError) -> Self {
        match error {
            super::EffectiveConfigError::Registry(cause) => Self::from(cause),
            super::EffectiveConfigError::Malformed(detail) => Self::MalformedHost { detail },
        }
    }
}

impl RuntimeConfigError {
    /// The public code for this fault: a TOTAL match on the error KIND,
    /// never on `detail`, whose text can carry store-controlled values.
    #[must_use]
    pub const fn status_code(&self) -> ConfigStatusCode {
        match *self {
            Self::RegistryBuildFailed { .. } => ConfigStatusCode::RegistryBuildFailed,
            Self::MalformedHost { .. } => ConfigStatusCode::MalformedConfig,
            Self::StoreKeyRejected { .. } => ConfigStatusCode::StoreKeyRejected,
            Self::ConfigStoreUnavailable { .. } => ConfigStatusCode::StoreUnavailable,
            Self::UnfundedExtraction { .. } => ConfigStatusCode::UnfundedExtraction,
        }
    }

    /// Take the PERMISSIVE disposition for runtime-config faults only:
    /// log the failure and carry on from the baked default. Immutable
    /// registry failures remain fatal.
    ///
    /// Fail-closed ([`RuntimeConfigError::into_response`]) exists to stop
    /// one specific harm — a request answered under config nobody can
    /// vouch for, silently forwarding identifiers because the baked
    /// default leaves GDPR enforcement off. A route that reaches no
    /// privacy decision cannot cause that harm, so failing closed there
    /// buys nothing and costs availability.
    ///
    /// `/status` is that route. It is a liveness probe: a probe that 500s
    /// through a config-store outage turns a degraded-but-still-serving
    /// fleet into a restart loop, and restarting cannot fix the store.
    /// The load POLICY still lives in one place — only the disposition
    /// differs, and it is named here rather than hand-rolled per handler.
    ///
    /// Every other config-consuming route keeps fail-closed. `/info` in
    /// particular is not a liveness surface, and a config it cannot trust
    /// makes the bidder list it advertises untrustworthy too.
    ///
    /// # Errors
    /// Immutable registry failures remain fatal and cannot use baked defaults.
    pub fn degrade_to_default(self) -> Result<AuctioneerAppConfig, Self> {
        let reason = match self {
            error @ Self::RegistryBuildFailed { .. } => return Err(error),
            Self::UnfundedExtraction { reason } => {
                format!("unfunded config extraction: {}", reason.as_str())
            }
            Self::MalformedHost { detail } => format!("malformed host config: {detail}"),
            Self::StoreKeyRejected { detail } => {
                format!("config store rejected the bound key: {detail}")
            }
            Self::ConfigStoreUnavailable { detail, .. } => {
                format!("config store unavailable: {detail}")
            }
        };
        log::warn!(
            "{reason}. Continuing from AuctioneerAppConfig::default(): this route reaches no \
             privacy decision, so failing closed would cost availability without protecting \
             anything."
        );
        Ok(AuctioneerAppConfig::default())
    }

    /// Render the error as the response the route returns verbatim — the
    /// FAIL-CLOSED disposition, and the default for every route that
    /// reaches a privacy decision.
    ///
    /// # Errors
    /// Returns [`EdgeError`] when the response builder rejects the
    /// status / header / body combination.
    pub fn into_response(self) -> Result<Response, EdgeError> {
        match self {
            Self::RegistryBuildFailed { error } => {
                log::error!("{error}");
                response_builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("cache-control", "no-store")
                    .header(
                        CONFIG_STATUS_HEADER,
                        ConfigStatusCode::RegistryBuildFailed.as_str(),
                    )
                    .body(Body::from_bytes(
                        b"Bidder registry initialization failed".to_vec(),
                    ))
                    .map_err(EdgeError::internal)
            }
            Self::UnfundedExtraction { reason } => {
                log::error!("unfunded config extraction: {}", reason.as_str());
                response_builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("cache-control", "no-store")
                    .body(Body::from_bytes(
                        b"Unfunded configuration extraction".to_vec(),
                    ))
                    .map_err(EdgeError::internal)
            }
            Self::MalformedHost { detail } => {
                log::error!("malformed host config: {detail}");
                response_builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("cache-control", "no-store")
                    .body(Body::from_bytes(b"Malformed host configuration".to_vec()))
                    .map_err(EdgeError::internal)
            }
            // The same wire bytes as a corrupt blob: on every route that
            // reaches a privacy or auction decision, a deploy whose key
            // is refused is as unusable as one whose blob is. Only the
            // log line and `/status`'s public code tell them apart.
            Self::StoreKeyRejected { detail } => {
                log::error!("config store rejected the bound key: {detail}");
                response_builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("cache-control", "no-store")
                    .body(Body::from_bytes(b"Malformed host configuration".to_vec()))
                    .map_err(EdgeError::internal)
            }
            Self::ConfigStoreUnavailable {
                detail,
                retry_after,
            } => {
                log::warn!("config store unavailable: {detail}");
                let secs = retry_after.as_secs();
                let body = format!("Config store unavailable, retry after {secs}s");
                response_builder()
                    .status(StatusCode::SERVICE_UNAVAILABLE)
                    .header("content-type", "text/plain; charset=utf-8")
                    .header("cache-control", "no-store")
                    .header("retry-after", secs.to_string())
                    .body(Body::from_bytes(body.into_bytes()))
                    .map_err(EdgeError::internal)
            }
        }
    }
}

/// Resolve the runtime app-config for a request.
///
/// A runtime publication is required: absent bindings and missing blobs fail
/// closed just like malformed publications. There is no operator-configurable
/// missing-config fallback. The SDK performs typed and full semantic validation;
/// successful extraction is not validated a second time here.
///
/// Cached registry failures take precedence, and cold registry construction and
/// store reads require the context's complete captured extraction tuple to fit
/// the funded host declaration. Backend/secret outages and deadline expiry stay
/// transient (503 with `Retry-After`); invalid keys retain their distinct refusal
/// code and permanent 500 disposition. The bound key comes from the selected
/// store binding, not [`CONFIG_STORE_ID`], which only selects the binding.
///
/// `/status` alone may explicitly take [`RuntimeConfigError::degrade_to_default`]
/// after recording the closed fault code, because it makes no privacy decision.
///
/// # Errors
/// Returns [`RuntimeConfigError::MalformedHost`] when the stored blob
/// fails its integrity, envelope, deserialization or validation check,
/// or when the store reports a reason this build does not recognise,
/// [`RuntimeConfigError::StoreKeyRejected`] when the backend rejects the
/// binding's key, and
/// [`RuntimeConfigError::ConfigStoreUnavailable`] when the backend, its
/// secret backend, or the read deadline is transiently exhausted. The
/// full map over the store-extraction taxonomy is
/// [`classify_store_extraction`]. Returns [`RuntimeConfigError::UnfundedExtraction`]
/// before any store read if the installed boot gates do not fund the
/// context's captured extraction caps. Missing bindings always fail closed.
pub async fn resolve_runtime_config(
    ctx: &RequestContext,
) -> Result<AuctioneerAppConfig, RuntimeConfigError> {
    resolve_with_registry(
        ctx,
        cached_registry_error,
        || {
            let gates = boot_gates().ok_or(RuntimeConfigError::UnfundedExtraction {
                reason: UnfundedExtractionReason::BootMissing,
            })?;
            check_extraction_caps(
                ctx.config_extraction_limits(),
                extraction_limits_for(Some(&gates.limits)),
            )
        },
        || registry_handle().map(|_registry| ()),
    )
    .await
}

async fn resolve_with_registry(
    ctx: &RequestContext,
    cached_registry_error: impl FnOnce() -> Option<auctioneer_bidder::RegistryBuildError>,
    check_funding: impl FnOnce() -> Result<(), RuntimeConfigError>,
    initialize_registry: impl FnOnce() -> Result<(), auctioneer_bidder::RegistryBuildError>,
) -> Result<AuctioneerAppConfig, RuntimeConfigError> {
    if let Some(error) = cached_registry_error() {
        return Err(error.into());
    }
    if ctx.config_store_binding(CONFIG_STORE_ID).is_none() {
        return Err(missing_config("no config store binding", None));
    }
    check_funding()?;
    initialize_registry()?;
    AppConfig::<SdkAuctioneerAppConfig>::from_store(ctx, CONFIG_STORE_ID, None)
        .await
        .map(|config| config.0)
        .map_err(classify_load_error)
}

// The complete SDK tuple is bounded independently by the host's funded declaration.
fn check_extraction_caps(
    captured: ConfigExtractionLimits,
    funded: ConfigExtractionLimits,
) -> Result<(), RuntimeConfigError> {
    let ConfigExtractionLimits {
        max_backend_bytes,
        max_blob_bytes,
        max_json_depth,
        max_json_nodes,
        max_secret_bytes,
        max_total_bytes,
        timeout,
    } = captured;
    if max_backend_bytes > funded.max_backend_bytes
        || max_blob_bytes > funded.max_blob_bytes
        || max_json_depth > funded.max_json_depth
        || max_json_nodes > funded.max_json_nodes
        || max_secret_bytes > funded.max_secret_bytes
        || max_total_bytes > funded.max_total_bytes
        || timeout > funded.timeout
    {
        return Err(RuntimeConfigError::UnfundedExtraction {
            reason: UnfundedExtractionReason::CapturedCapsAboveFunding,
        });
    }
    Ok(())
}

/// The pure classification of a `from_store` error, split from the
/// async resolver so every arm — including the `#[non_exhaustive]`
/// remainder — is unit-testable with constructed errors.
#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "All residual SDK errors, including future variants, intentionally share this value-free fail-closed category."
)]
fn classify_load_error(error: EdgeError) -> RuntimeConfigError {
    match error {
        EdgeError::StoreExtraction {
            reason, field_path, ..
        } => classify_store_extraction(reason, field_path.as_deref()),
        // The residual untyped arms carry no upstream text either: an
        // `Internal` source or a backend message can quote stored
        // values, and `detail` reaches the log line.
        EdgeError::Internal { .. } => RuntimeConfigError::MalformedHost {
            detail: "config store extraction failed: internal error".to_owned(),
        },
        EdgeError::ServiceUnavailable { .. } => RuntimeConfigError::ConfigStoreUnavailable {
            detail: "config store unavailable".to_owned(),
            retry_after: CONFIG_STORE_RETRY_AFTER,
        },
        EdgeError::BadRequest { .. } => RuntimeConfigError::StoreKeyRejected {
            detail: "config store rejected the bound key".to_owned(),
        },
        // `EdgeError` is `#[non_exhaustive]`, so this arm cannot be
        // spelled out variant by variant — and a variant this build has
        // never seen is a config it cannot vouch for. It fails closed
        // with fixed text: the error's `Display` can quote stored
        // values, so it is neither logged nor rendered.
        _unclassified => RuntimeConfigError::MalformedHost {
            detail: "config store extraction failed: unclassified error".to_owned(),
        },
    }
}

/// The one rule for "there is no config to load" — whether because no
/// store is bound or because the bound store holds no blob. Always fails closed.
/// An established registry failure remains fatal without initializing a registry here.
fn missing_config(
    what: &'static str,
    registry_error: Option<auctioneer_bidder::RegistryBuildError>,
) -> RuntimeConfigError {
    if let Some(error) = registry_error {
        return error.into();
    }
    RuntimeConfigError::MalformedHost {
        detail: format!(
            "config store extraction failed: {what} (runtime configuration is required)"
        ),
    }
}

/// The total map over upstream's `#[non_exhaustive]` store-extraction
/// taxonomy. `detail` carries the reason discriminant and the field path
/// only: deserialization, validation and secret failures can embed stored
/// values or secret identifiers, so the upstream message never reaches a
/// log line or the wire.
fn classify_store_extraction(
    reason: StoreExtractionReason,
    field_path: Option<&str>,
) -> RuntimeConfigError {
    use StoreExtractionReason as Reason;
    let detail = match field_path {
        Some(path) => format!("config store extraction failed: {reason:?} at {path}"),
        None => format!("config store extraction failed: {reason:?}"),
    };
    match reason {
        // Missing and deleted publications are both unconfigured, never permissive.
        Reason::MissingBlob => missing_config("MissingBlob", cached_registry_error()),
        // Transient on the backend side, the secret backend and the read
        // deadline included: a retry can succeed, so the caller answers
        // 503 with `Retry-After` rather than a permanent 500.
        Reason::BackendUnavailable
        | Reason::SecretBackendUnavailable
        | Reason::DeadlineExceeded => RuntimeConfigError::ConfigStoreUnavailable {
            detail,
            retry_after: CONFIG_STORE_RETRY_AFTER,
        },
        // The typed extractor's route to the same fault the untyped
        // `BadRequest` arm carries. It leaves the malformed family so
        // `/status` reports it distinctly; its disposition does not
        // change.
        Reason::InvalidKey => RuntimeConfigError::StoreKeyRejected { detail },
        // `StructuralLimit` is the stored blob exceeding the extractor's
        // JSON depth or node cap before any of it is materialized. It
        // belongs with `Deserialization`, `IntegrityMismatch` and
        // `MalformedEnvelope` rather than with the transient arm: the
        // caps are fixed for the life of the process, so the identical
        // blob fails identically on every retry — there is nothing for a
        // `Retry-After` to wait for. It is not `MissingBlob` either; it
        // can only arise AFTER a read returned a value.
        Reason::BackendFailure
        | Reason::Deserialization
        | Reason::IntegrityMismatch
        | Reason::InvalidSecretValue
        | Reason::MalformedEnvelope
        | Reason::MissingRegistry
        | Reason::MissingSecret
        | Reason::StructuralLimit
        | Reason::UnknownStore
        | Reason::UnsupportedVersion
        | Reason::Validation
        | Reason::ValueTooLarge => RuntimeConfigError::MalformedHost { detail },
        // A reason this build has never seen is a config it cannot trust.
        unknown => RuntimeConfigError::MalformedHost {
            detail: format!("config store extraction failed: unrecognised reason {unknown:?}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuctioneerAppConfig, CONFIG_STORE_RETRY_AFTER, ConfigStatusCode, RuntimeConfigError,
        StatusCode, missing_config,
    };
    use crate::config::{EffectiveConfigError, validate_effective_defaults};
    use edgezero_core::config_store::ConfigExtractionLimits;
    use edgezero_core::context::RequestContext;
    use edgezero_core::error::{EdgeError, StoreExtractionReason};
    use futures::executor::block_on;
    use std::{cell::Cell, sync::Arc, time::Duration};

    // Preserve pure effective-validation controls without repeating validation
    // after the SDK's production extractor has already accepted a publication.
    fn classify_load(
        loaded: Result<AuctioneerAppConfig, EdgeError>,
    ) -> Result<AuctioneerAppConfig, RuntimeConfigError> {
        let config = loaded.map_err(super::classify_load_error)?;
        validate_effective_defaults(&config)?;
        Ok(config)
    }

    #[test]
    fn cached_failure_precedes_funding_but_cold_initialization_requires_funding() {
        use super::UnfundedExtractionReason;
        let cached_error = auctioneer_bidder::RegistryBuildError::MissingBinding {
            bidder: "fake_cached",
        };
        let cold_error = auctioneer_bidder::RegistryBuildError::UndeclaredBidder {
            bidder: "fake_cold",
        };
        for funding_error in [
            None,
            Some(UnfundedExtractionReason::BootMissing),
            Some(UnfundedExtractionReason::CapturedCapsAboveFunding),
        ] {
            for cached in [None, Some(cached_error)] {
                let funding_calls = Cell::new(0_u32);
                let builder_calls = Cell::new(0_u32);
                let result = block_on(super::resolve_with_registry(
                    &bound_context(),
                    || cached,
                    || {
                        funding_calls.set(funding_calls.get() + 1);
                        funding_error.map_or(Ok(()), |reason| {
                            Err(RuntimeConfigError::UnfundedExtraction { reason })
                        })
                    },
                    || {
                        builder_calls.set(builder_calls.get() + 1);
                        Err(cold_error)
                    },
                ));
                if let Some(expected) = cached {
                    assert!(
                        matches!(result, Err(RuntimeConfigError::RegistryBuildFailed { error }) if error == expected)
                    );
                    assert_eq!(funding_calls.get(), 0);
                    assert_eq!(builder_calls.get(), 0);
                } else if let Some(expected) = funding_error {
                    assert!(
                        matches!(result, Err(RuntimeConfigError::UnfundedExtraction { reason }) if reason == expected)
                    );
                    assert_eq!(funding_calls.get(), 1);
                    assert_eq!(builder_calls.get(), 0);
                } else {
                    assert!(
                        matches!(result, Err(RuntimeConfigError::RegistryBuildFailed { error }) if error == cold_error)
                    );
                    assert_eq!(funding_calls.get(), 1);
                    assert_eq!(builder_calls.get(), 1);
                }
            }
        }
    }

    #[test]
    fn missing_config_does_not_absorb_an_established_registry_failure() {
        let source = auctioneer_bidder::RegistryBuildError::MissingBinding {
            bidder: "fake_missing",
        };
        for absence in ["no config store binding", "MissingBlob"] {
            let result = super::missing_config(absence, Some(source));
            assert!(
                matches!(result, RuntimeConfigError::RegistryBuildFailed { error } if error == source)
            );
        }
    }

    #[test]
    fn absent_binding_fails_closed_without_funding_registry_or_store_work() {
        use edgezero_core::{body::Body, http::request_builder, params::PathParams};

        let request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let context = RequestContext::new(request, PathParams::default());
        let funding_calls = Cell::new(0_u32);
        let registry_calls = Cell::new(0_u32);
        let error = block_on(super::resolve_with_registry(
            &context,
            || None,
            || {
                funding_calls.set(funding_calls.get() + 1);
                Ok(())
            },
            || {
                registry_calls.set(registry_calls.get() + 1);
                Ok(())
            },
        ))
        .unwrap_err();
        assert_eq!(error.status_code(), ConfigStatusCode::MalformedConfig);
        let response = error.into_response().unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key("retry-after"));
        assert_eq!(funding_calls.get(), 0);
        assert_eq!(registry_calls.get(), 0);
    }

    #[test]
    fn unfunded_context_refuses_before_registry_initialization() {
        use super::UnfundedExtractionReason;
        for reason in [
            UnfundedExtractionReason::BootMissing,
            UnfundedExtractionReason::CapturedCapsAboveFunding,
        ] {
            let calls = Cell::new(0_u32);
            let source = auctioneer_bidder::RegistryBuildError::MissingBinding {
                bidder: "fake_missing",
            };
            let initialize = || {
                calls.set(calls.get() + 1);
                Err(source)
            };
            let result = block_on(super::resolve_with_registry(
                &bound_context(),
                || None,
                || Err(RuntimeConfigError::UnfundedExtraction { reason }),
                initialize,
            ));
            assert!(
                matches!(result, Err(RuntimeConfigError::UnfundedExtraction { reason: actual }) if actual == reason)
            );
            assert_eq!(calls.get(), 0);

            let funded = block_on(super::resolve_with_registry(
                &bound_context(),
                || None,
                || Ok(()),
                initialize,
            ));
            assert!(
                matches!(funded, Err(RuntimeConfigError::RegistryBuildFailed { error }) if error == source)
            );
            assert_eq!(
                calls.get(),
                1,
                "funding must reach the same builder after refusal"
            );
        }
    }

    #[test]
    fn funded_context_propagates_registry_failure_before_store_read() {
        let initialized = Cell::new(false);
        let source = auctioneer_bidder::RegistryBuildError::MissingBinding {
            bidder: "fake_missing",
        };
        let result = block_on(super::resolve_with_registry(
            &bound_context(),
            || None,
            || Ok(()),
            || {
                initialized.set(true);
                Err(source)
            },
        ));
        assert!(
            matches!(result, Err(RuntimeConfigError::RegistryBuildFailed { error }) if error == source)
        );
        assert!(initialized.get());
    }

    fn bound_context() -> RequestContext {
        use edgezero_core::{
            body::Body,
            config_store::{
                BoundedStoreRead, ConfigStore, ConfigStoreError, ConfigStoreHandle, ConfigValue,
            },
            http::request_builder,
            params::PathParams,
            store_registry::{ConfigRegistry, ConfigStoreBinding, StoreRegistry},
            time::{Deadline, MonotonicClock},
        };
        struct NoReads;
        #[async_trait::async_trait(?Send)]
        impl ConfigStore for NoReads {
            async fn get(&self, _key: &str) -> Result<Option<ConfigValue>, ConfigStoreError> {
                panic!("funding and registry refusals must precede store reads");
            }

            async fn get_bounded(
                &self,
                _key: &str,
                _clock: &MonotonicClock,
                _deadline: Deadline,
                _max_backend_bytes: u64,
                _max_value_bytes: u64,
            ) -> Result<BoundedStoreRead<ConfigValue>, ConfigStoreError> {
                panic!("funding and registry refusals must precede store reads");
            }
        }
        let mut request = request_builder()
            .uri("/status")
            .body(Body::empty())
            .unwrap();
        let stores: ConfigRegistry = StoreRegistry::single_id(
            super::CONFIG_STORE_ID.to_owned(),
            ConfigStoreBinding {
                default_key: super::CONFIG_STORE_ID.to_owned(),
                handle: ConfigStoreHandle::new(Arc::new(NoReads)),
            },
        );
        request.extensions_mut().insert(stores);
        RequestContext::new(request, PathParams::default())
    }

    #[test]
    fn registry_failure_is_fatal_not_malformed_or_degradable() {
        let source = auctioneer_bidder::RegistryBuildError::MissingBinding {
            bidder: "fake_missing",
        };
        let error = RuntimeConfigError::from(EffectiveConfigError::Registry(source));
        assert_eq!(
            error.status_code(),
            super::ConfigStatusCode::RegistryBuildFailed
        );
        let refused = error
            .degrade_to_default()
            .expect_err("registry failures cannot degrade");
        assert!(
            matches!(refused, RuntimeConfigError::RegistryBuildFailed { error: cause } if cause == source)
        );
        let response = refused.into_response().unwrap();
        assert_eq!(response.status(), http::StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.body().as_bytes().unwrap(),
            b"Bidder registry initialization failed"
        );
        assert!(!response.headers().contains_key("retry-after"));
        assert_eq!(
            response.headers()[super::CONFIG_STATUS_HEADER],
            "registry_build_failed"
        );
    }

    #[test]
    fn malformed_effective_config_remains_degradable() {
        let error =
            RuntimeConfigError::from(EffectiveConfigError::Malformed("fake_invalid".to_owned()));
        assert_eq!(
            error.status_code(),
            super::ConfigStatusCode::MalformedConfig
        );
        error.degrade_to_default().unwrap();
    }

    #[test]
    fn extraction_caps_accept_equality_and_independently_narrower_contexts() {
        let equal = ConfigExtractionLimits {
            max_backend_bytes: 7919,
            max_blob_bytes: 3571,
            max_json_depth: 17,
            max_json_nodes: 127,
            max_secret_bytes: 521,
            max_total_bytes: 12_347,
            timeout: Duration::from_millis(1973),
        };
        for captured in [
            equal,
            ConfigExtractionLimits {
                max_backend_bytes: equal.max_backend_bytes - 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_blob_bytes: equal.max_blob_bytes - 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_json_depth: equal.max_json_depth - 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_json_nodes: equal.max_json_nodes - 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_secret_bytes: equal.max_secret_bytes - 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_total_bytes: equal.max_total_bytes - 1,
                ..equal
            },
            ConfigExtractionLimits {
                timeout: equal
                    .timeout
                    .checked_sub(Duration::from_nanos(1))
                    .expect("one-nanosecond narrowing stays within the positive timeout"),
                ..equal
            },
        ] {
            assert_eq!(captured.validate().unwrap(), captured);
            super::check_extraction_caps(captured, equal).unwrap();
        }
    }

    #[test]
    fn each_extraction_cap_independently_refuses_one_unit_above_funding() {
        let equal = ConfigExtractionLimits {
            max_backend_bytes: 7919,
            max_blob_bytes: 3571,
            max_json_depth: 17,
            max_json_nodes: 127,
            max_secret_bytes: 521,
            max_total_bytes: 12_347,
            timeout: Duration::from_millis(1973),
        };
        for captured in [
            ConfigExtractionLimits {
                max_backend_bytes: equal.max_backend_bytes + 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_blob_bytes: equal.max_blob_bytes + 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_json_depth: equal.max_json_depth + 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_json_nodes: equal.max_json_nodes + 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_secret_bytes: equal.max_secret_bytes + 1,
                ..equal
            },
            ConfigExtractionLimits {
                max_total_bytes: equal.max_total_bytes + 1,
                ..equal
            },
            ConfigExtractionLimits {
                timeout: equal.timeout + Duration::from_nanos(1),
                ..equal
            },
        ] {
            assert_eq!(captured.validate().unwrap(), captured);
            let error = super::check_extraction_caps(captured, equal).unwrap_err();
            assert!(matches!(
                error,
                RuntimeConfigError::UnfundedExtraction {
                    reason: super::UnfundedExtractionReason::CapturedCapsAboveFunding,
                }
            ));
        }
    }

    #[test]
    fn unfunded_extraction_has_fixed_reasons_and_bounded_nonretryable_wire_bytes() {
        use super::UnfundedExtractionReason as Reason;
        for (reason, fixed) in [
            (Reason::BootMissing, "boot_missing"),
            (
                Reason::CapturedCapsAboveFunding,
                "captured_caps_above_funding",
            ),
        ] {
            assert_eq!(reason.as_str(), fixed);
            let error = RuntimeConfigError::UnfundedExtraction { reason };
            assert_eq!(error.status_code(), ConfigStatusCode::UnfundedExtraction);
            assert_eq!(error.status_code().as_str(), "unfunded_extraction");
            let response = error.into_response().unwrap();
            assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(response.headers()["cache-control"], "no-store");
            assert_eq!(
                response.headers()["content-type"],
                "text/plain; charset=utf-8"
            );
            assert!(!response.headers().contains_key("retry-after"));
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-origin")
            );
            assert_eq!(
                response.body().as_bytes().unwrap(),
                b"Unfunded configuration extraction"
            );
            RuntimeConfigError::UnfundedExtraction { reason }
                .degrade_to_default()
                .expect("liveness remains available without beginning an unfunded extraction");
        }
    }

    /// Every variant of the upstream taxonomy this build knows, with the
    /// wire class each must produce. A variant added upstream lands in
    /// the fail-closed wildcard until it is placed here on purpose.
    const TRANSIENT: [StoreExtractionReason; 3] = [
        StoreExtractionReason::BackendUnavailable,
        StoreExtractionReason::SecretBackendUnavailable,
        StoreExtractionReason::DeadlineExceeded,
    ];
    const MALFORMED: [StoreExtractionReason; 12] = [
        StoreExtractionReason::BackendFailure,
        StoreExtractionReason::Deserialization,
        StoreExtractionReason::IntegrityMismatch,
        StoreExtractionReason::InvalidSecretValue,
        StoreExtractionReason::MalformedEnvelope,
        StoreExtractionReason::MissingRegistry,
        StoreExtractionReason::MissingSecret,
        StoreExtractionReason::StructuralLimit,
        StoreExtractionReason::UnknownStore,
        StoreExtractionReason::UnsupportedVersion,
        StoreExtractionReason::Validation,
        StoreExtractionReason::ValueTooLarge,
    ];

    #[test]
    fn transient_reasons_map_to_503_with_retry_after_and_discriminant_only_detail() {
        for reason in TRANSIENT {
            let err = super::classify_store_extraction(reason, Some("host.gdpr.tcf2"));
            match &err {
                RuntimeConfigError::ConfigStoreUnavailable { detail, .. } => assert_eq!(
                    detail,
                    &format!("config store extraction failed: {reason:?} at host.gdpr.tcf2"),
                ),
                RuntimeConfigError::RegistryBuildFailed { .. }
                | RuntimeConfigError::UnfundedExtraction { .. }
                | RuntimeConfigError::MalformedHost { .. }
                | RuntimeConfigError::StoreKeyRejected { .. } => {
                    panic!("{reason:?} must be ConfigStoreUnavailable, got {err:?}")
                }
            }
            let response = err.into_response().expect("response builds");
            assert_eq!(
                response.status(),
                StatusCode::SERVICE_UNAVAILABLE,
                "{reason:?}"
            );
            assert!(response.headers().contains_key("retry-after"), "{reason:?}");
        }
    }

    /// Transient errors stay closed on privacy routes; only liveness may degrade.
    #[test]
    fn transient_reasons_remain_errors_and_liveness_can_degrade() {
        for reason in TRANSIENT {
            for field_path in [None, Some("host.gdpr")] {
                let err = super::classify_store_extraction(reason, field_path);
                match &err {
                    RuntimeConfigError::ConfigStoreUnavailable { detail, .. } => {
                        let suffix = field_path.map_or(String::new(), |path| format!(" at {path}"));
                        assert_eq!(
                            detail,
                            &format!("config store extraction failed: {reason:?}{suffix}"),
                        );
                    }
                    RuntimeConfigError::RegistryBuildFailed { .. }
                    | RuntimeConfigError::UnfundedExtraction { .. }
                    | RuntimeConfigError::MalformedHost { .. }
                    | RuntimeConfigError::StoreKeyRejected { .. } => {
                        panic!("{reason:?} must stay transient, got {err:?}")
                    }
                }
                let response = err.into_response().expect("response builds");
                assert_eq!(
                    response.status(),
                    StatusCode::SERVICE_UNAVAILABLE,
                    "{reason:?} / {field_path:?}",
                );
                assert!(
                    response.headers().contains_key("retry-after"),
                    "{reason:?} / {field_path:?}",
                );
                let degraded = super::classify_store_extraction(reason, field_path)
                    .degrade_to_default()
                    .expect("liveness can degrade a transient config fault");
                assert!(degraded.host.aliases.is_none());
            }
        }
    }

    #[test]
    fn malformed_family_maps_to_500_with_discriminant_only_detail() {
        for reason in MALFORMED {
            let err = super::classify_store_extraction(reason, None);
            match &err {
                RuntimeConfigError::MalformedHost { detail } => assert_eq!(
                    detail,
                    &format!("config store extraction failed: {reason:?}"),
                ),
                RuntimeConfigError::RegistryBuildFailed { .. }
                | RuntimeConfigError::UnfundedExtraction { .. }
                | RuntimeConfigError::ConfigStoreUnavailable { .. }
                | RuntimeConfigError::StoreKeyRejected { .. } => {
                    panic!("{reason:?} must be MalformedHost, got {err:?}")
                }
            }
            let response = err.into_response().expect("response builds");
            assert_eq!(
                response.status(),
                StatusCode::INTERNAL_SERVER_ERROR,
                "{reason:?}"
            );
        }
    }

    /// Malformed publications remain permanent faults, with explicit liveness degradation.
    #[test]
    fn malformed_reasons_remain_errors_and_liveness_can_degrade() {
        for reason in MALFORMED {
            for field_path in [None, Some("host.gdpr")] {
                let err = super::classify_store_extraction(reason, field_path);
                match &err {
                    RuntimeConfigError::MalformedHost { detail } => {
                        let suffix = field_path.map_or(String::new(), |path| format!(" at {path}"));
                        assert_eq!(
                            detail,
                            &format!("config store extraction failed: {reason:?}{suffix}")
                        );
                    }
                    RuntimeConfigError::RegistryBuildFailed { .. }
                    | RuntimeConfigError::UnfundedExtraction { .. }
                    | RuntimeConfigError::ConfigStoreUnavailable { .. }
                    | RuntimeConfigError::StoreKeyRejected { .. } => {
                        panic!("{reason:?} must stay malformed, got {err:?}")
                    }
                }
                let response = err.into_response().expect("response builds");
                assert_eq!(
                    response.status(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "{reason:?} / {field_path:?}",
                );
                assert!(!response.headers().contains_key("retry-after"));
                let degraded = super::classify_store_extraction(reason, field_path)
                    .degrade_to_default()
                    .expect("liveness can degrade a malformed publication");
                assert!(degraded.host.aliases.is_none());
            }
        }
    }

    /// The rejected-key reason has to travel the REAL extractor path.
    /// bounded fanout reaches it as `StoreExtractionReason::InvalidKey`, which the
    /// classifier used to group with the malformed family — a test
    /// building an `EdgeError::BadRequest` by hand would pass while the
    /// shipped path still reported `malformed_config`.
    #[test]
    fn invalid_key_reason_reports_store_key_rejected() {
        let err = super::classify_store_extraction(StoreExtractionReason::InvalidKey, None);
        assert!(
            matches!(err, RuntimeConfigError::StoreKeyRejected { .. }),
            "a refused key is its own fault, not a malformed blob: {err:?}",
        );
        assert_eq!(err.status_code(), ConfigStatusCode::StoreKeyRejected);
        assert_eq!(err.status_code().as_str(), "store_key_rejected");
    }

    /// `EdgeError::BadRequest` is the untyped path to the same fault, and
    /// it has to land on the same variant — otherwise the code an
    /// operator sees depends on which extractor the deploy happened to
    /// take.
    #[test]
    fn bad_request_reports_store_key_rejected() {
        let err = classify_load(Err(EdgeError::bad_request("rejected")))
            .expect_err("a refused key never yields a config");
        assert!(matches!(err, RuntimeConfigError::StoreKeyRejected { .. }));
        assert_eq!(err.status_code(), ConfigStatusCode::StoreKeyRejected);
    }

    /// The split is a REPORTING split, not a policy change: outside
    /// `/status`, a refused key fails closed exactly as a corrupt blob
    /// does — 500, no `Retry-After` — so retrying cannot paper over a
    /// deploy where no push can ever land.
    #[test]
    fn store_key_rejected_keeps_the_malformed_disposition() {
        let malformed = RuntimeConfigError::MalformedHost {
            detail: "corrupt blob".to_owned(),
        }
        .into_response()
        .expect("response builds");
        let rejected = RuntimeConfigError::StoreKeyRejected {
            detail: "refused key".to_owned(),
        }
        .into_response()
        .expect("response builds");
        assert_eq!(rejected.status(), malformed.status());
        assert_eq!(rejected.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(
            !rejected.headers().contains_key("retry-after"),
            "a key the backend will never accept is not worth retrying",
        );
    }

    /// The public vocabulary is derived from the error KIND by a total
    /// match, and its wire strings are fixed here rather than taken from
    /// a `Debug` impl a later refactor could rename.
    #[test]
    fn config_status_codes_are_fixed_and_kind_derived() {
        let rows: [(RuntimeConfigError, ConfigStatusCode, &str); 3] = [
            (
                RuntimeConfigError::MalformedHost {
                    detail: "canary".to_owned(),
                },
                ConfigStatusCode::MalformedConfig,
                "malformed_config",
            ),
            (
                RuntimeConfigError::StoreKeyRejected {
                    detail: "canary".to_owned(),
                },
                ConfigStatusCode::StoreKeyRejected,
                "store_key_rejected",
            ),
            (
                RuntimeConfigError::ConfigStoreUnavailable {
                    detail: "canary".to_owned(),
                    retry_after: CONFIG_STORE_RETRY_AFTER,
                },
                ConfigStatusCode::StoreUnavailable,
                "store_unavailable",
            ),
        ];
        for (err, expected, wire) in rows {
            assert_eq!(err.status_code(), expected);
            assert_eq!(expected.as_str(), wire);
            assert!(
                !wire.contains("canary"),
                "the public code is never derived from `detail`",
            );
        }
    }

    #[test]
    fn missing_blob_is_always_a_closed_runtime_fault() {
        let refused = super::classify_store_extraction(StoreExtractionReason::MissingBlob, None);
        match &refused {
            RuntimeConfigError::MalformedHost { detail } => {
                assert!(detail.starts_with("config store extraction failed: MissingBlob"));
            }
            RuntimeConfigError::RegistryBuildFailed { .. }
            | RuntimeConfigError::UnfundedExtraction { .. }
            | RuntimeConfigError::ConfigStoreUnavailable { .. }
            | RuntimeConfigError::StoreKeyRejected { .. } => {
                panic!("must be MalformedHost, got {refused:?}")
            }
        }
        assert_eq!(
            refused.into_response().expect("response builds").status(),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }

    /// The `#[non_exhaustive]` remainder and the residual untyped arms
    /// fail closed with FIXED text: a constructed error carrying a
    /// canary in its message never reaches `detail` — the only string
    /// that is logged or rendered.
    #[test]
    fn outer_edge_errors_fail_closed_without_upstream_text() {
        let canary = "secret-value-that-must-never-appear";
        let outer: [EdgeError; 5] = [
            EdgeError::not_found(canary),
            EdgeError::validation(canary),
            EdgeError::internal(anyhow::anyhow!("{canary}")),
            EdgeError::bad_request(canary),
            EdgeError::service_unavailable(canary),
        ];
        for err in outer {
            let classified =
                classify_load(Err(err)).expect_err("outer errors never yield a config");
            let detail = match &classified {
                RuntimeConfigError::MalformedHost { detail }
                | RuntimeConfigError::StoreKeyRejected { detail }
                | RuntimeConfigError::ConfigStoreUnavailable { detail, .. } => detail.clone(),
                RuntimeConfigError::RegistryBuildFailed { .. }
                | RuntimeConfigError::UnfundedExtraction { .. } => {
                    panic!("store classification cannot refuse funding")
                }
            };
            assert!(
                !detail.contains(canary),
                "upstream text leaked into detail: {detail}"
            );
            let status = classified
                .into_response()
                .expect("response builds")
                .status();
            assert!(
                matches!(
                    status,
                    StatusCode::INTERNAL_SERVER_ERROR | StatusCode::SERVICE_UNAVAILABLE
                ),
                "every outer error fails closed, got {status}",
            );
        }
    }

    #[test]
    fn missing_config_is_one_rule_for_absent_binding_and_missing_blob() {
        for what in ["no config store binding", "MissingBlob"] {
            let refused = missing_config(what, None);
            match refused {
                RuntimeConfigError::MalformedHost { detail } => {
                    assert_eq!(
                        detail,
                        format!(
                            "config store extraction failed: {what} (runtime configuration is required)"
                        ),
                    );
                }
                RuntimeConfigError::RegistryBuildFailed { .. }
                | RuntimeConfigError::UnfundedExtraction { .. }
                | RuntimeConfigError::ConfigStoreUnavailable { .. }
                | RuntimeConfigError::StoreKeyRejected { .. } => {
                    panic!("a missing config is never transient")
                }
            }
            let degraded = missing_config(what, None)
                .degrade_to_default()
                .expect("only the explicit liveness disposition can degrade a missing config");
            assert!(degraded.host.aliases.is_none());
        }
    }

    /// The privacy-safe logging rule, stated as a contract on `detail`:
    /// for every reason it is exactly the discriminant plus the optional
    /// field path — never the upstream message, which can quote stored
    /// values or secret identifiers. The classifier cannot even receive
    /// that message; this test pins the exact strings so a future
    /// "helpful" `{message}` interpolation fails here.
    #[test]
    fn config_error_logs_structured_allowlist_no_raw_text() {
        let canary = "secret-value-that-must-never-appear";
        for reason in TRANSIENT
            .into_iter()
            .chain(MALFORMED)
            .chain([StoreExtractionReason::InvalidKey])
        {
            let err = super::classify_store_extraction(reason, Some(canary));
            let detail = match err {
                RuntimeConfigError::MalformedHost { detail }
                | RuntimeConfigError::StoreKeyRejected { detail }
                | RuntimeConfigError::ConfigStoreUnavailable { detail, .. } => detail,
                RuntimeConfigError::RegistryBuildFailed { .. }
                | RuntimeConfigError::UnfundedExtraction { .. } => {
                    panic!("store classification cannot refuse funding")
                }
            };
            assert_eq!(
                detail,
                format!("config store extraction failed: {reason:?} at {canary}"),
                "detail is the discriminant and the field path, nothing else",
            );
        }
    }

    #[test]
    fn malformed_host_renders_a_plain_text_500() {
        let response = RuntimeConfigError::MalformedHost {
            detail: "bad gdpr block".to_owned(),
        }
        .into_response()
        .expect("response builds");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok()),
            Some("text/plain; charset=utf-8"),
        );
    }

    #[test]
    fn config_store_unavailable_renders_503_with_retry_after() {
        let response = RuntimeConfigError::ConfigStoreUnavailable {
            detail: "kv down".to_owned(),
            retry_after: Duration::from_secs(30),
        }
        .into_response()
        .expect("response builds");
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok()),
            Some("30"),
        );
    }

    #[test]
    fn fail_closed_config_responses_are_not_cacheable_or_cors_decorated() {
        for error in [
            RuntimeConfigError::MalformedHost {
                detail: "invalid".to_owned(),
            },
            RuntimeConfigError::StoreKeyRejected {
                detail: "invalid".to_owned(),
            },
            RuntimeConfigError::ConfigStoreUnavailable {
                detail: "offline".to_owned(),
                retry_after: Duration::from_secs(5),
            },
        ] {
            let response = error.into_response().expect("response builds");
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
            assert!(
                !response
                    .headers()
                    .contains_key("access-control-allow-origin")
            );
        }
    }

    #[test]
    fn loaded_sync_override_is_validated_after_merging() {
        let invalid: AuctioneerAppConfig =
            toml::from_str("schema_version = 1\n[host.host_cookie]\ncookie_name = 'uids'\n")
                .unwrap();
        let sdk_invalid = super::SdkAuctioneerAppConfig(invalid);
        assert!(
            validator::Validate::validate(&sdk_invalid).is_err(),
            "the SDK boundary must apply semantic validation"
        );
        let err = classify_load(Ok(sdk_invalid.0)).expect_err("merged invalid config fails closed");
        assert!(matches!(err, RuntimeConfigError::MalformedHost { .. }));

        let valid: AuctioneerAppConfig = toml::from_str(
            "schema_version = 1\n[host.host_cookie]\nmax_cookie_size_bytes = 0\n\
             [host.user_sync]\nexternal_url = 'https://sync.example.test'\npriority_groups = []\n",
        )
        .unwrap();
        let sdk_valid = super::SdkAuctioneerAppConfig(valid);
        validator::Validate::validate(&sdk_valid)
            .expect("SDK validation accepts explicit zero and empty priority");
        classify_load(Ok(sdk_valid.0)).expect("explicit zero and empty priority are valid");
    }
}
