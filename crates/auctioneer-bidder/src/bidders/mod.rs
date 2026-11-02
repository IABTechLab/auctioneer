//! Bidder implementations.
//!
//! `url::json_request_meta` resolves endpoint templates through
//! `EndpointBuilder` against the codegen-emitted `endpoint_macros`
//! allow-list, writing the result straight into the sink's meta byte pool.

mod media_type;
mod status;
mod url;
mod wire_fields;

include!(concat!(env!("OUT_DIR"), "/discovery.rs"));
include!(concat!(env!("OUT_DIR"), "/registry.rs"));
include!(concat!(env!("OUT_DIR"), "/sync_uid_policies.rs"));

/// A discovered implementation, including implementations gated off in this build.
pub struct BidderDeclaration {
    /// The implementation directory's validated key.
    pub key: &'static str,
    /// The Cargo feature required to compile this implementation, if any.
    pub required_feature: Option<&'static str>,
}

/// A compiled implementation and its lazy, stateless constructor.
pub struct BidderRegistration {
    /// The implementation directory's validated key.
    pub key: &'static str,
    /// Constructs a bidder synchronously and infallibly, without I/O.
    pub factory: fn() -> Box<dyn crate::Bidder>,
}

/// Borrow all implementation declarations, regardless of enabled features.
#[must_use]
pub fn bidder_declarations() -> &'static [BidderDeclaration] {
    ALL_DECLARATIONS
}

/// Driving a bidder through a real sink, for the bidder unit tests.
///
/// A bidder no longer returns its requests, so a test that wants to
/// assert on one has to supply the sink the bidder pushes into. Doing
/// that through the REAL sink rather than a recording double is the
/// point: the sink owns the reserve-validate-write transaction, so a
/// test driven this way exercises the same ordering production does.
#[cfg(test)]
pub(crate) mod harness {

    use crate::bidder::{Bidder, BidderError};
    use crate::endpoint_validator::{EndpointRejectReason, EndpointValidator};
    use crate::meta::{META_BYTES_CAPACITY, OutboundRequestMeta, OutboundRequestMetaBuilder};
    use crate::request_info::BidderRequestInfo;
    use crate::sink::{CallBudget, OutboundRequestSink, SinkLimits, SinkOutcome};
    use crate::types::{BidderHttpRequest, BidderHttpResponse, BidderResponse};
    use auctioneer_openrtb::types::bid_request::BidRequest;

    // Paths resolve at the invocation, so each bidder keeps its fixtures adjacent.
    // The callback owns protocol-specific assertions and approved differences.
    #[cfg(not(target_arch = "wasm32"))]
    macro_rules! protocol_fixtures {
        ($runner:path, $root:literal; $($test:ident => $path:literal),+ $(,)?) => {
            $(
                #[test]
                fn $test() {
                    $runner(stringify!($test), $path, include_str!(concat!($root, $path)));
                }
            )+
        };
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) use protocol_fixtures;

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) use crate::test_support::factory_contract::assert_factory_contract;

    /// Validate frozen adjacent provenance and supplied capture bytes before dispatch.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn verify_protocol_capture(
        manifest: &str,
        manifest_sha256: &str,
        path: &str,
        source: &str,
    ) -> Result<(), &'static str> {
        use sha2::{Digest as _, Sha256};
        let digest = |bytes: &str| format!("{:x}", Sha256::digest(bytes.as_bytes()));
        if digest(manifest) != manifest_sha256 {
            return Err("protocol manifest digest mismatch");
        }
        let document: toml::Value =
            toml::from_str(manifest).map_err(|_error| "invalid protocol manifest")?;
        let artifacts = document
            .get("provenance")
            .and_then(|value| value.get("artifacts"))
            .and_then(toml::Value::as_array)
            .ok_or("invalid protocol manifest")?;
        let mut matches = artifacts.iter().filter(|artifact| {
            artifact.get("category").and_then(toml::Value::as_str) == Some("protocol")
                && artifact
                    .get("retained")
                    .and_then(|value| value.get("source_path"))
                    .and_then(toml::Value::as_str)
                    == Some(path)
        });
        let artifact = matches
            .next()
            .ok_or("protocol capture has no unique witness")?;
        if matches.next().is_some() {
            return Err("protocol capture has no unique witness");
        }
        let retained = artifact
            .get("retained")
            .ok_or("invalid protocol manifest")?;
        let length = retained
            .get("source_bytes")
            .and_then(toml::Value::as_integer)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or("invalid protocol manifest")?;
        let expected_digest = retained
            .get("source_sha256")
            .and_then(toml::Value::as_str)
            .ok_or("invalid protocol manifest")?;
        if artifact.get("transformation").is_some()
            || artifact
                .get("source_bytes")
                .and_then(toml::Value::as_integer)
                .and_then(|bytes| u64::try_from(bytes).ok())
                != Some(length)
            || artifact.get("source_sha256").and_then(toml::Value::as_str) != Some(expected_digest)
        {
            return Err("protocol capture must retain its source bytes verbatim");
        }
        if u64::try_from(source.len()).ok() != Some(length) || digest(source) != expected_digest {
            return Err("protocol capture digest mismatch");
        }
        Ok(())
    }

    /// Accepts every URL, so a bidder test's subject stays the
    /// bidder. Endpoint policy has its own tests in the exchange.
    struct AllowAllEndpoints;

    impl EndpointValidator for AllowAllEndpoints {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Ok(())
        }
    }

    /// Budgets generous enough that nothing a bidder test pushes is
    /// refused for want of room — the caps have their own tests in
    /// `sink` and `capped_writer`.
    const TEST_LIMITS: SinkLimits = SinkLimits::new(256 * 1024, 100);

    /// A minimal `OutboundRequestMeta` for a `make_bids` test.
    ///
    /// Built through the real builder rather than by constructing the
    /// struct, so the offsets a parser reads back are the ones the sink
    /// would have produced.
    pub(super) fn sent_meta(url: &str) -> OutboundRequestMeta {
        sent_meta_with_ids(url, [])
    }

    pub(super) fn sent_meta_with_ids<'a>(
        url: &str,
        ids: impl IntoIterator<Item = &'a str>,
    ) -> OutboundRequestMeta {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 100);
        builder.set_method("POST").expect("fits");
        builder.set_url(url).expect("fits");
        for id in ids {
            builder.imp_id(id).unwrap();
        }
        builder.finish()
    }

    /// A node budget wide enough that no fixture a decode test writes
    /// is refused for want of nodes.
    ///
    /// A row that wants the budget to BIND names its own narrow figure,
    /// so a test asserting a refusal cannot be reading this one.
    const TEST_NODE_BUDGET: u32 = 1_000_000;

    /// A 200 response carrying `body`, budgeted so a bidder decode
    /// test is testing the bidder and not the budget.
    ///
    /// Built through `within_node_budget` rather than a struct literal
    /// because the body is sealed — the same seam the exchange uses.
    pub(crate) fn budgeted_response(status: u16, body: Vec<u8>) -> BidderHttpResponse {
        BidderHttpResponse::new(status, vec![], body).within_node_budget(TEST_NODE_BUDGET)
    }

    pub(super) fn decode_bid(
        bidder: &dyn Bidder,
        impressions: serde_json::Value,
        fields: serde_json::Value,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        let mut seats = serde_json::json!([{"seat": "bidder", "bid": []}]);
        seats[0]["bid"] = serde_json::Value::Array(vec![fields]);
        decode_bids(bidder, impressions, seats)
    }

    pub(super) fn decode_bids(
        bidder: &dyn Bidder,
        impressions: serde_json::Value,
        seats: serde_json::Value,
    ) -> Result<BidderResponse, Vec<BidderError>> {
        let (request, response) = bid_fixtures(impressions, seats);
        bidder.make_bids(
            &request,
            &sent_meta_with_ids(
                "https://bidder.example.test/openrtb",
                request.imp.iter().map(|imp| imp.id.as_str()),
            ),
            &response,
        )
    }

    pub(super) fn assert_bad_server_response_errors(errors: &[BidderError], expected: usize) {
        assert_eq!(errors.len(), expected);
        assert!(errors.iter().all(|error| error.code == 4));
    }

    // Malformed fixtures panic independently of the bidder's semantic Result.
    fn bid_fixtures(
        impressions: serde_json::Value,
        mut seats: serde_json::Value,
    ) -> (BidRequest, BidderHttpResponse) {
        for seat in seats
            .as_array_mut()
            .expect("seat fixtures must be an array")
        {
            for fields in seat["bid"]
                .as_array_mut()
                .expect("bid fixtures must be an array")
            {
                let serde_json::Value::Object(extra_fields) = fields.take() else {
                    panic!("bid fields must be an object");
                };
                let mut bid = serde_json::json!({"id": "bid", "impid": "i", "price": 1.25_f64});
                bid.as_object_mut()
                    .expect("bid object")
                    .extend(extra_fields);
                *fields = bid;
            }
        }
        let request = BidRequest {
            id: "request".into(),
            imp: serde_json::from_value(impressions).expect("valid impression fixtures"),
            ..BidRequest::default()
        };
        let mut response = serde_json::json!({"id": "response", "cur": "USD"});
        response["seatbid"] = seats;
        let body = serde_json::to_vec(&response).expect("valid response fixture");
        (request, budgeted_response(200, body))
    }

    /// Run `bidder.make_requests` through a sink and hand back both
    /// channels: the semantic result, and calls eligible for dispatch.
    pub(super) fn drive<A>(
        bidder: &A,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> (Result<(), Vec<BidderError>>, Vec<BidderHttpRequest>)
    where
        A: Bidder + ?Sized,
    {
        let (semantic, outcome, _remaining) =
            drive_with_limits(bidder, request, info, TEST_LIMITS, 16, &AllowAllEndpoints);
        (semantic, outcome.accepted)
    }

    pub(super) fn drive_with_limits<A: Bidder + ?Sized>(
        bidder: &A,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
        limits: SinkLimits,
        max_calls: u32,
        validator: &dyn EndpointValidator,
    ) -> (Result<(), Vec<BidderError>>, SinkOutcome, u32) {
        let budget = CallBudget::new(max_calls);
        let mut sink = OutboundRequestSink::new(limits, &budget, validator);
        let semantic = bidder.make_requests(request, info, &mut sink);
        let outcome = if semantic.is_ok() {
            sink.into_outcome()
        } else {
            sink.into_outcome_discarding_accepted()
        };
        (semantic, outcome, budget.remaining())
    }

    /// `drive`, asserting the semantic channel succeeded, for the tests
    /// whose subject is the request the bidder built.
    pub(crate) fn drive_ok<A>(
        bidder: &A,
        request: &BidRequest,
        info: &BidderRequestInfo<'_>,
    ) -> Vec<BidderHttpRequest>
    where
        A: Bidder + ?Sized,
    {
        let (semantic, accepted) = drive(bidder, request, info);
        semantic.expect("bidder reports no semantic failure");
        accepted
    }

    #[cfg(test)]
    mod construction {
        use super::*;
        use crate::{
            BidderName, ConstructionDiagnostic, EntryPoint, OutboundBuildError,
            debug_projection::DebugBodySchema,
        };
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct ProbeBidder {
            fail: bool,
            accepted_before_finish: AtomicUsize,
        }

        impl Bidder for ProbeBidder {
            fn debug_body_schema(&self) -> DebugBodySchema {
                DebugBodySchema::Unsupported
            }

            fn make_requests(
                &self,
                _request: &BidRequest,
                info: &BidderRequestInfo<'_>,
                sink: &mut OutboundRequestSink<'_>,
            ) -> Result<(), Vec<BidderError>> {
                sink.diagnostic(ConstructionDiagnostic::InvalidImpressionParameters);
                for body in [b"x".as_slice(), b"xx", b"x", b"x", b"x"] {
                    let _recorded = sink.push_request(
                        info,
                        |meta| {
                            meta.set_method("POST")?;
                            meta.set_url(info.endpoint_template)
                        },
                        |writer| writer.write_all(body),
                    );
                }
                self.accepted_before_finish
                    .store(sink.accepted_count(), Ordering::Relaxed);
                if self.fail {
                    Err(vec![BidderError::bad_input("construction failed")])
                } else {
                    Ok(())
                }
            }

            fn make_bids(
                &self,
                _request: &BidRequest,
                _sent: &OutboundRequestMeta,
                _response: &BidderHttpResponse,
            ) -> Result<BidderResponse, Vec<BidderError>> {
                Err(vec![BidderError::bad_server_response(
                    "construction-only probe",
                )])
            }
        }

        #[test]
        fn driver_commits_only_success_and_refunds_semantic_failure() {
            let info = BidderRequestInfo::new(
                BidderName::new("probe"),
                BidderName::new("probe"),
                "account",
                None,
                "https://bidder.example.test/",
                &[],
                None,
                None,
                EntryPoint::Auction,
                &[],
            );
            for fail in [false, true] {
                let bidder = ProbeBidder {
                    fail,
                    accepted_before_finish: AtomicUsize::new(0),
                };
                let (semantic, outcome, remaining) = drive_with_limits(
                    &bidder,
                    &BidRequest::default(),
                    &info,
                    SinkLimits::new(1, 1),
                    2,
                    &AllowAllEndpoints,
                );
                assert_eq!(bidder.accepted_before_finish.load(Ordering::Relaxed), 2);
                assert_eq!(
                    outcome.errors,
                    [
                        OutboundBuildError::OversizeRaw { limit: 1 },
                        OutboundBuildError::CallBudgetExhausted { max_calls: 2 },
                    ]
                );
                assert_eq!(outcome.rejected_call_count, 2);
                assert_eq!(outcome.suppressed_error_count, 0);
                assert_eq!(
                    outcome.diagnostics.iter().copied().collect::<Vec<_>>(),
                    [ConstructionDiagnostic::InvalidImpressionParameters]
                );
                if fail {
                    let errors = semantic.unwrap_err();
                    assert_eq!(errors.len(), 1);
                    assert_eq!(errors[0].code, 2);
                    assert_eq!(errors[0].message, "construction failed");
                    assert!(outcome.accepted.is_empty());
                    assert_eq!(remaining, 2);
                } else {
                    semantic.unwrap();
                    assert_eq!(outcome.accepted.len(), 2);
                    assert!(outcome.accepted.iter().all(|call| call.body() == b"x"));
                    assert_eq!(remaining, 0);
                }
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[cfg(test)]
    mod capture_integrity {
        use super::verify_protocol_capture;
        use serde_json::json;
        use sha2::{Digest as _, Sha256};

        #[derive(Clone, Copy, Debug)]
        enum WitnessChange {
            Transformed,
            Nonprotocol,
            Duplicate,
            Length,
            Digest,
        }

        fn sha256(source: &str) -> String {
            format!("{:x}", Sha256::digest(source.as_bytes()))
        }

        fn fixture() -> (String, String, String) {
            let source = serde_json::to_string(&json!({"id": "fixture"})).unwrap();
            let digest = sha256(&source);
            let manifest = toml::to_string(&json!({
                "key": "fixture", "factory": true,
                "provenance": {
                    "origin": "reference",
                    "sources": {"upstream": {"repository": "https://example.test/reference.git", "revision": "a".repeat(40)}},
                    "artifacts": [{
                        "category": "protocol", "source": "upstream",
                        "source_path": "fixtures/source.json", "source_blob": "b".repeat(40),
                        "source_bytes": source.len(), "source_sha256": digest,
                        "retained": {"source_path": "fixtures/protocol/source.json", "source_bytes": source.len(), "source_sha256": digest},
                    }],
                },
            })).unwrap();
            let manifest_digest = sha256(&manifest);
            (manifest, manifest_digest, source)
        }

        #[test]
        fn verifies_the_frozen_manifest_and_exact_supplied_capture() {
            let (manifest, digest, source) = fixture();
            assert_eq!(
                verify_protocol_capture(
                    &manifest,
                    &digest,
                    "fixtures/protocol/source.json",
                    &source
                ),
                Ok(())
            );
        }

        #[test]
        fn sent_fixture_metadata_carries_exact_ordered_coverage_and_no_implicit_ids() {
            let meta =
                super::sent_meta_with_ids("https://bidder.example.test/", ["second", "first"]);
            assert_eq!(meta.method(), "POST");
            assert_eq!(meta.url(), "https://bidder.example.test/");
            assert_eq!(
                meta.covered_imp_ids().collect::<Vec<_>>(),
                ["second", "first"]
            );
            assert_eq!(
                super::sent_meta("https://bidder.example.test/").imp_id_count(),
                0
            );
        }

        #[test]
        fn refuses_manifest_changes_unknown_paths_and_same_length_capture_edits() {
            let (manifest, digest, source) = fixture();
            assert_eq!(
                verify_protocol_capture(
                    &manifest,
                    &"0".repeat(64),
                    "fixtures/protocol/source.json",
                    &source
                ),
                Err("protocol manifest digest mismatch")
            );
            assert_eq!(
                verify_protocol_capture(
                    &manifest,
                    &digest,
                    "fixtures/protocol/not-captured.json",
                    &source
                ),
                Err("protocol capture has no unique witness")
            );
            let changed = source.replace("fixture", "changed");
            assert_eq!(changed.len(), source.len());
            assert_eq!(
                verify_protocol_capture(
                    &manifest,
                    &digest,
                    "fixtures/protocol/source.json",
                    &changed
                ),
                Err("protocol capture digest mismatch")
            );
        }

        #[test]
        fn refuses_transformed_nonprotocol_duplicate_and_inconsistent_verbatim_witnesses() {
            let (manifest, _digest, source) = fixture();
            let original: toml::Value = toml::from_str(&manifest).unwrap();
            for kind in [
                WitnessChange::Transformed,
                WitnessChange::Nonprotocol,
                WitnessChange::Duplicate,
                WitnessChange::Length,
                WitnessChange::Digest,
            ] {
                let mut changed = original.clone();
                let rows = changed["provenance"]["artifacts"].as_array_mut().unwrap();
                match kind {
                    WitnessChange::Transformed => {
                        rows[0].as_table_mut().unwrap().insert(
                            "transformation".into(),
                            toml::Value::String("changed source".into()),
                        );
                    }
                    WitnessChange::Nonprotocol => {
                        rows[0]["category"] = toml::Value::String("metadata".into());
                    }
                    WitnessChange::Duplicate => {
                        rows.push(rows[0].clone());
                    }
                    WitnessChange::Length => {
                        rows[0]["source_bytes"] = toml::Value::Integer(1);
                    }
                    WitnessChange::Digest => {
                        rows[0]["source_sha256"] = toml::Value::String("0".repeat(64));
                    }
                }
                let bytes = toml::to_string(&changed).unwrap();
                let expected =
                    if matches!(kind, WitnessChange::Nonprotocol | WitnessChange::Duplicate) {
                        "protocol capture has no unique witness"
                    } else {
                        "protocol capture must retain its source bytes verbatim"
                    };
                assert_eq!(
                    verify_protocol_capture(
                        &bytes,
                        &sha256(&bytes),
                        "fixtures/protocol/source.json",
                        &source
                    ),
                    Err(expected),
                    "{kind:?}"
                );
            }
        }
    }
}

#[cfg(test)]
#[cfg(not(target_arch = "wasm32"))]
mod tests {
    use crate::{BidderDeclaration, BidderRegistration, bidder_declarations, compiled_bidders};
    use serde::Deserialize;
    use std::{collections::BTreeMap, fs, path::Path, ptr};

    #[derive(Deserialize)]
    struct Manifest {
        key: String,
        factory: bool,
        required_feature: Option<String>,
    }

    fn expected_declarations() -> Vec<(String, Option<String>)> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders");
        let mut expected = BTreeMap::new();
        for entry in fs::read_dir(root).unwrap() {
            let directory = entry.unwrap().path();
            let manifest_path = directory.join("bidder.toml");
            if !manifest_path.is_file() {
                continue;
            }
            let manifest: Manifest =
                toml::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
            assert_eq!(
                directory.file_name().unwrap().to_str().unwrap(),
                manifest.key
            );
            if !manifest.factory {
                continue;
            }
            let fixture: serde_json::Value = serde_json::from_str(
                &fs::read_to_string(directory.join("fixtures/factory.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                fixture["implementation"].as_str(),
                Some(manifest.key.as_str())
            );
            assert!(!fixture["cases"].as_array().unwrap().is_empty());
            assert!(
                expected
                    .insert(manifest.key, manifest.required_feature)
                    .is_none()
            );
        }
        assert!(
            !expected.is_empty(),
            "independent factory scan cannot be empty"
        );
        expected.into_iter().collect()
    }

    #[test]
    fn compiled_bidders_are_sorted_and_feature_filtered() {
        let expected: Vec<_> = expected_declarations()
            .into_iter()
            .filter(|(_key, feature)| match feature.as_deref() {
                None => true,
                Some("insecure-bidders") => cfg!(feature = "insecure-bidders"),
                Some(unknown) => panic!("unknown required feature: {unknown}"),
            })
            .map(|(key, _feature)| key)
            .collect();
        let actual: Vec<_> = compiled_bidders()
            .iter()
            .map(|row| row.key.to_owned())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn bidder_declarations_include_sorted_feature_gated_rows() {
        let actual: Vec<_> = bidder_declarations()
            .iter()
            .map(|row| (row.key.to_owned(), row.required_feature.map(str::to_owned)))
            .collect();
        assert_eq!(actual, expected_declarations());
    }

    #[test]
    fn discovery_views_borrow_static_slices() {
        let declarations: &'static [BidderDeclaration] = bidder_declarations();
        let registrations: &'static [BidderRegistration] = compiled_bidders();
        assert!(!declarations.is_empty());
        assert!(!registrations.is_empty());
        for _ in 0..3_usize {
            assert!(ptr::eq(declarations, bidder_declarations()));
            assert!(ptr::eq(registrations, compiled_bidders()));
        }
    }
}
