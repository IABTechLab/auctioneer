//! Bidder framework.

pub mod bidder;
pub mod bidders;
pub mod capped_writer;
pub mod catalog;
pub mod config_validation;
pub mod correlation;
pub mod currency;
pub mod debug_projection;
pub mod diagnostics;
pub mod endpoint_builder;
pub mod endpoint_validator;
pub mod finalize;
pub mod meta;
mod native_adm;
pub mod registry;
pub mod request_info;
mod response_decode;
pub mod schema;
pub mod sink;
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
pub mod types;

pub use bidder::{Bidder, BidderError, BidderErrorKind, EntryPoint};
pub use bidders::{
    BidderDeclaration, BidderRegistration, accepts_sync_uid, bidder_declarations, compiled_bidders,
};
pub use capped_writer::{BodyWriteError, CappedWriter};
pub use config_validation::{BidderConfigError, BidderConfigInput, BidderConfigValidator};
pub use currency::{FloorConversion, FloorConversionError};
pub use debug_projection::{
    DebugBodyEligibility, DebugBodyProjection, DebugBodySchema, DebugBodyStatus,
    is_debug_json_content_type, project_request_body,
};
pub use diagnostics::{
    ConstructionDiagnostic, ConstructionDiagnostics, MAX_CONSTRUCTION_DIAGNOSTICS,
};
pub use endpoint_builder::{EndpointBuilder, EndpointBuilderError};
pub use endpoint_validator::{EndpointRejectReason, EndpointValidator};
pub use finalize::{Compression, FinalizeError, TransportFinalize, finalize_for_transport};
pub use meta::{
    MAX_BIDDER_HEADERS, META_BIDDER_REGION_BYTES, META_BYTES_CAPACITY, META_FINALIZE_RESERVE_BYTES,
    MetaField, OutboundRequestMeta, OutboundRequestMetaBuilder, RESERVED_EXCHANGE_HEADER_SLOTS,
    SinkAbort, outbound_meta_bytes,
};
pub use registry::{
    BidderRegistry, RegistryAlias, RegistryBuildError, RegistryDefinition, RegistryInputs,
    ValidatedRegistryPlan,
};
pub use request_info::BidderRequestInfo;
pub use sink::{
    AcceptedRequest, CallBudget, CallReservation, MAX_RETAINED_SINK_ERRORS, OutboundRequestSink,
    SinkLimits, SinkOutcome,
};
pub use types::{
    BidderHttpRequest, BidderHttpResponse, BidderName, BidderResponse, BrowserAuctionConfig,
    OutboundBuildError, ProxyError, TypedBid,
};

/// Construct the published registry, excluding inspection-only schema builds.
///
/// # Errors
/// Refuses unqualified staging before materializing metadata or constructors,
/// then preserves the generated registry's structural failures.
pub fn configured_registry() -> Result<BidderRegistry, RegistryBuildError> {
    if schema::schema_artifact_state() == schema::SchemaArtifactState::Unqualified {
        return Err(RegistryBuildError::UnqualifiedSchemaArtifact);
    }
    bidders::configured_registry()
}
