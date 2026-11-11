//! Child binary: validate the actual generated views without installing a registry.

use auctioneer_bidder::{
    BidderName, BidderRequestInfo, CallBudget, EndpointRejectReason, EndpointValidator, EntryPoint,
    OutboundRequestSink, RegistryInputs, SinkLimits, ValidatedRegistryPlan, bidder_declarations,
    compiled_bidders,
};
use auctioneer_config_data::{
    bidder_implementation_bindings, bidder_metadata_declarations, bidders,
};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
use serde_json::{Value, json};

struct InvalidEndpointsOnly;

impl EndpointValidator for InvalidEndpointsOnly {
    fn validate(
        &self,
        url: &url::Url,
        _: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        if url.scheme() == "https"
            && url
                .host_str()
                .is_some_and(|host| host.ends_with(".invalid"))
        {
            Ok(())
        } else {
            Err(EndpointRejectReason::HostNotAllowed)
        }
    }
}

fn execute(plan: &ValidatedRegistryPlan, name: &str) -> Value {
    let definition = plan.definition(name).unwrap();
    let info = definition.info();
    let bidder = definition.construct();
    let request = BidRequest {
        id: "discovery-request".into(),
        imp: vec![Imp {
            id: "discovery-imp".into(),
            ..Imp::default()
        }],
        ..BidRequest::default()
    };
    let macros = info
        .endpoint_macros
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    let request_info = BidderRequestInfo::new(
        BidderName::new(info.name.as_str()),
        BidderName::new(definition.canonical_base()),
        "discovery-account",
        None,
        info.endpoint.as_deref().unwrap(),
        &macros,
        None,
        None,
        EntryPoint::Auction,
        &[],
    );
    let budget = CallBudget::new(1);
    let mut sink =
        OutboundRequestSink::new(SinkLimits::new(1024, 1), &budget, &InvalidEndpointsOnly);
    bidder
        .make_requests(&request, &request_info, &mut sink)
        .unwrap();
    let outcome = sink.into_outcome();
    assert!(outcome.errors.is_empty());
    assert_eq!(outcome.rejected_call_count, 0);
    assert_eq!(outcome.suppressed_error_count, 0);
    assert_eq!(outcome.accepted.len(), 1);
    let outbound = &outcome.accepted[0];
    let header = |key| outbound.headers().find(|(name, _)| *name == key).unwrap().1;
    json!({
        "name": name,
        "implementation": definition.implementation(),
        "method": outbound.method(),
        "endpoint": outbound.url(),
        "imp_ids": outbound.covered_imp_ids().collect::<Vec<_>>(),
        "content_type": header("Content-Type"),
        "module": header("X-Discovery-Module"),
        "constructors": header("X-Discovery-Constructors").parse::<usize>().unwrap(),
        "executions": header("X-Discovery-Executions").parse::<usize>().unwrap(),
        "body": serde_json::from_slice::<Value>(outbound.body()).unwrap(),
    })
}

fn main() {
    let plan = ValidatedRegistryPlan::validate(RegistryInputs {
        infos: bidders(),
        bindings: bidder_implementation_bindings(),
        metadata_declarations: bidder_metadata_declarations(),
        factory_declarations: bidder_declarations(),
        factories: compiled_bidders(),
        aliases: &[],
    })
    .unwrap();
    let lookups = [
        "fake_discovery_alpha",
        "fake_discovery_beta",
        "fake_discovery_fake_gamma",
        "fake_discovery_mirror",
        "fake_discovery_optional",
    ];
    let executions = plan
        .definitions()
        .map(|definition| execute(&plan, definition.info().name.as_str()))
        .collect::<Vec<_>>();
    println!(
        "{}",
        json!({
            "factories": compiled_bidders().iter().map(|row| row.key).collect::<Vec<_>>(),
            "plan_factories": plan.factory_keys().collect::<Vec<_>>(),
            "definitions": plan.definitions().map(|row| row.info().name.as_str()).collect::<Vec<_>>(),
            "metadata_names": bidders().iter().map(|row| row.name.as_str()).collect::<Vec<_>>(),
            "bindings": bidder_implementation_bindings().iter().map(|row| (row.bidder, row.implementation)).collect::<Vec<_>>(),
            "declarations": bidder_declarations().iter().map(|row| (row.key, row.required_feature)).collect::<Vec<_>>(),
            "metadata_declarations": bidder_metadata_declarations().iter().map(|row| (row.bidder, row.implementation, row.required_feature)).collect::<Vec<_>>(),
            "lookup": lookups.map(|name| (name, plan.definition(name).is_some())),
            "metadata_lookup": lookups.map(|name| (name, auctioneer_config_data::bidder(name).is_some())),
            "overrides": auctioneer_config_data::TEST_HARNESS_OVERRIDE_ENABLED,
            "execution_count": executions.len(),
            "executed": executions,
        })
    );
}
