//! APS source conformance and capture integrity; not delivery or migration evidence.

#![cfg(not(target_arch = "wasm32"))]

mod bidder_manifest {
    // Independent test-only DTO; do not compile another crate's build source here.
    use serde::{Deserialize, Serialize};
    use std::collections::BTreeMap;

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct Manifest {
        pub key: String,
        pub factory: bool,
        pub required_feature: Option<String>,
        #[serde(default)]
        pub native_only: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub endpoint: Option<EndpointSchema>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<SettingsSchema>,
        #[serde(default)]
        pub definitions: BTreeMap<String, toml::Table>,
        pub provenance: Provenance,
    }

    #[derive(Debug, Default, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct EndpointSchema {
        #[serde(default)]
        pub fields: BTreeMap<String, String>,
    }

    #[derive(Debug, Default, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct SettingsSchema {
        #[serde(default)]
        pub fields: BTreeMap<String, SettingKind>,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum SettingKind {
        OptionalString,
        OptionalI64,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(tag = "origin", rename_all = "snake_case", deny_unknown_fields)]
    pub(crate) enum Provenance {
        #[expect(
            clippy::empty_enum_variants_with_brackets,
            reason = "Serde needs a struct variant to reject unexpected local-provenance fields"
        )]
        Local {},
        Reference {
            sources: BTreeMap<String, Source>,
            #[serde(default)]
            trees: Vec<SourceTree>,
            #[serde(default)]
            artifacts: Vec<Artifact>,
        },
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct Source {
        pub repository: String,
        pub revision: String,
    }

    #[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
    #[serde(rename_all = "snake_case")]
    pub(crate) enum Category {
        Implementation,
        Helper,
        Schema,
        Metadata,
        Protocol,
        Parameter,
        License,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct SourceTree {
        pub category: Category,
        pub source: String,
        pub source_path: String,
        pub tree: String,
        #[serde(default)]
        pub port_differences: Vec<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct Artifact {
        pub category: Category,
        pub source: String,
        pub source_path: String,
        pub source_blob: String,
        pub source_bytes: u64,
        pub source_sha256: String,
        pub retained: Option<Retained>,
        pub transformation: Option<String>,
        #[serde(default)]
        pub port_differences: Vec<String>,
    }

    /// A local artifact copy; its source path is relative to the owning bidder folder.
    #[derive(Debug, Deserialize, Serialize)]
    #[serde(deny_unknown_fields)]
    pub(crate) struct Retained {
        #[serde(rename = "source_path")]
        pub path: String,
        #[serde(rename = "source_bytes")]
        pub bytes: u64,
        #[serde(rename = "source_sha256")]
        pub sha256: String,
    }

    #[cfg(test)]
    mod tests {
        use super::Retained;

        const WITNESS: &str = "source_path = 'parameters.json'\nsource_bytes = 3778\nsource_sha256 = '20a3f193c62b1b3d6d2a0daf5dfaa3935a045c52b83af236693ebe9c3459106d'\n";

        #[test]
        fn retained_witness_round_trips_only_source_prefixed_fields() {
            let witness: Retained = toml::from_str(WITNESS).unwrap();
            let encoded = toml::to_string(&witness).unwrap();
            assert_eq!(
                toml::from_str::<toml::Value>(&encoded).unwrap(),
                toml::from_str::<toml::Value>(WITNESS).unwrap(),
            );
        }

        #[test]
        fn retained_witness_rejects_every_legacy_and_mixed_field_shape() {
            let fields = [
                ("source_path", "path"),
                ("source_bytes", "bytes"),
                ("source_sha256", "sha256"),
            ];
            for mask in 1..8_u8 {
                let mut document = WITNESS.to_owned();
                for (bit, (current, legacy)) in fields.iter().enumerate() {
                    if mask & (1 << bit) != 0 {
                        document = document.replacen(current, legacy, 1);
                    }
                }
                assert!(
                    toml::from_str::<Retained>(&document).is_err(),
                    "mask {mask}"
                );
            }
            for (_, legacy) in fields {
                let mixed = format!("{WITNESS}{legacy} = 'obsolete'\n");
                assert!(toml::from_str::<Retained>(&mixed).is_err(), "{legacy}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::bidder_manifest;
    use crate::bidders::harness::protocol_fixtures;
    use serde::{Deserialize, Serialize};
    use serde_json::{Value, json};
    use sha2::{Digest as _, Sha256};
    use std::collections::{BTreeMap, BTreeSet};
    use std::env;
    use std::fs;
    use std::iter::once;
    use std::panic::catch_unwind;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::str::from_utf8;

    const REVISION: &str = "f1e9d0d2164e76a8bcb5f10021ebb56e00dbfe9a";
    const CATALOG_REVISION: &str = "d7f8515b86258688304b0d9b6668c6a0e258bc9e";
    const PARAMETER_BLOB: &str = "74e0358851e73cad3744f600271a68abb52ba62f";
    const PARAMETER_SHA256: &str =
        "5adcf9808af402728cdce2a71a6e992b31f802bba4984139cb80437bd5f57c64";
    const PROTOCOL_PRODUCER: &str = "adapters/aps/aps_test.go:TestJsonSamples";
    const PARAMETER_PRODUCERS: [&str; 2] = [
        "adapters/aps/params_test.go:TestValidParams",
        "adapters/aps/params_test.go:TestInvalidParams",
    ];
    const PARAMETER_TRANSFORMATION: &str = "Extract TestValidParams and TestInvalidParams raw JSON spans into ordered source-test expectations with zero-based raw-byte offsets; source tests were not executed.";
    const PORT_DIFFERENCES: &str = "Pinned supplemental source for the existing APS factory and opt-in native-only publication. No source execution, protocol parity, delivery or migration is claimed. Rust SDK source/version intentionally differ from the captured prebid-server 1.0.0 identity.";
    const PUBLICATION_DIFFERENCES: &str = concat!(
        "AG's approved 2026-10-30 controlled-native publication keeps defaultEnabled=false. ",
        "The source Region authority segment is represented by complete Host substitution, ",
        "allowlisted to the existing na/eu/fe HTTPS hosts; bidder-owned parameter validation ",
        "chooses the region. Preserve site-only banner/video, gzip, vendor and maintainer facts; ",
        "no app capability or sync URL is invented. Source geoscope remains provenance, not a ",
        "routing fallback. Publication does not establish live authorization or qualification.",
    );
    const MAX_DOCUMENT_BYTES: u64 = 64 * 1024;

    fn verify_protocol_fixture(
        capture: &Capture,
        path: &str,
        source: &[u8],
    ) -> Result<(), &'static str> {
        let counts = Counts {
            aps_owned_blobs: capture.aps_sources.len(),
            shared_witnesses: capture.shared_witnesses.len(),
            protocol_fixtures: capture.protocol_cases.len(),
            parameter_cases: PARAMETER_PINS.len(),
            parameter_accepts: 5,
            parameter_rejects: 9,
            make_requests: capture
                .protocol_cases
                .iter()
                .filter(|case| case.operations.contains(&Operation::MakeRequests))
                .count(),
            make_bids: capture
                .protocol_cases
                .iter()
                .filter(|case| case.operations.contains(&Operation::MakeBids))
                .count(),
        };
        if capture.counts != counts {
            return Err("APS capture counts differ from the reviewed inventory");
        }
        let relative = format!("fixtures/protocol/{path}");
        let Some(pinned) = capture.aps_sources.iter().find(|candidate| {
            matches!(&candidate.retention, Retention::Verbatim { path: retained_path }
                if retained_path == &relative)
        }) else {
            return Err("APS protocol fixture has no pinned witness");
        };
        if source.len() != pinned.raw_bytes || sha256(source) != pinned.sha256 {
            return Err("APS protocol fixture digest mismatch");
        }
        let case = capture
            .protocol_cases
            .iter()
            .find(|case| case.path == relative)
            .ok_or("APS protocol fixture has no reviewed case")?;
        let fixture: Value =
            serde_json::from_slice(source).map_err(|_error| "APS protocol fixture must be JSON")?;
        let mut members = fixture
            .as_object()
            .ok_or("APS protocol fixture must be an object")?
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        members.sort();
        let operations = if case.http_calls > 0 {
            vec![Operation::MakeRequests, Operation::MakeBids]
        } else {
            vec![Operation::MakeRequests]
        };
        if members != case.members || operations != case.operations {
            return Err("APS protocol members or operations differ from the reviewed case");
        }
        for (field, count) in [
            ("httpCalls", case.http_calls),
            ("expectedBidResponses", case.bid_responses),
            ("expectedMakeRequestsErrors", case.request_errors),
            ("expectedMakeBidsErrors", case.bid_errors),
        ] {
            let actual = match fixture.get(field) {
                Some(value) => value
                    .as_array()
                    .ok_or("APS protocol expectations must be arrays")?
                    .len(),
                None => 0,
            };
            if actual != count {
                return Err("APS protocol counts differ from the reviewed case");
            }
        }
        Ok(())
    }

    fn verify_protocol_dispatch(manifest: &[u8], path: &str, source: &[u8]) -> Result<(), String> {
        let capture = decode_manifest(manifest)?;
        verify_protocol_fixture(&capture, path, source).map_err(String::from)
    }

    fn run_fixture(name: &str, path: &str, source: &str) {
        // Validate the reviewed manifest and supplied bytes before bidder work,
        // even when a developer runs just one filtered protocol conformance case.
        verify_protocol_dispatch(&manifest_bytes(), path, source.as_bytes()).unwrap();
        super::super::run_fixture(name, source);
    }

    protocol_fixtures!(run_fixture, "../fixtures/protocol/";
        single_banner => "exemplary/single-banner.json",
        single_video => "exemplary/single-video.json",
        multi_slot => "exemplary/multi-slot.json",
        multiformat_imp => "exemplary/multiformat-imp.json",
        banner_partial_size_overwrite => "supplemental/banner-partial-size-overwrite.json",
        defaults_currency_to_usd => "supplemental/defaults-currency-to-usd.json",
        region_na_explicit => "supplemental/region-na-explicit.json",
        rejects_mismatched_account => "supplemental/rejects-mismatched-account.json",
        rejects_unsupported_region => "supplemental/rejects-unsupported-region.json",
        requires_account => "supplemental/requires-account.json",
        resolves_eu_region => "supplemental/resolves-eu-region.json",
        status_204 => "supplemental/status-204.json",
        status_400 => "supplemental/status-400.json",
        status_500 => "supplemental/status-500.json",
        strips_sensitive_data => "supplemental/strips-sensitive-data.json",
        appends_debug_in_test_mode => "supplemental/test-mode-appends-debug.json",
    );

    #[test]
    fn original_null_extension_preserves_the_exact_owner_refusal() {
        use crate::bidders::harness::drive;
        use crate::{BidderName, BidderRequestInfo, EntryPoint};
        use auctioneer_openrtb::types::bid_request::BidRequest;
        use auctioneer_runtime::bounded_json::{JsonFieldShape, count_json_nodes_with_field};

        let wire = serde_json::to_vec(&json!({
            "id": "r", "imp": [{"id": "i", "banner": {},
                "ext": {"bidder": {"accountID": "account"}}}],
            "ext": null
        }))
        .unwrap();
        let request: BidRequest = serde_json::from_slice(&wire).unwrap();
        assert!(request.ext.is_none());
        let (_, shape) = count_json_nodes_with_field(&wire, 128, "ext").unwrap();
        assert_eq!(shape, JsonFieldShape::Null);
        let mut info = BidderRequestInfo::new(
            BidderName::new("aps"),
            BidderName::new("aps"),
            "account",
            None,
            "https://bidder.example/auction",
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        info.original_request_ext_shape = shape;
        let (semantic, calls) = drive(&super::super::super::ApsBidder, &request, &info);
        let errors = semantic.unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, 2);
        assert_eq!(
            errors[0].message,
            "aps: original request extension must be absent or an object"
        );
        assert!(calls.is_empty());
    }

    #[test]
    fn protocol_dispatch_refuses_unknown_and_changed_same_length_capture_bytes() {
        let capture = manifest();
        let path = "exemplary/single-banner.json";
        let mut bytes = read_document(&root().join(format!("fixtures/protocol/{path}")));
        assert_eq!(verify_protocol_fixture(&capture, path, &bytes), Ok(()));
        assert_eq!(
            verify_protocol_fixture(&capture, "exemplary/not-captured.json", &bytes),
            Err("APS protocol fixture has no pinned witness")
        );
        run_fixture("single_banner", path, from_utf8(&bytes).unwrap());
        // Same length and JSON meaning: ordinary protocol assertions alone
        // cannot notice this change, so only pre-dispatch integrity can refuse it.
        let newline = bytes.iter().position(|byte| *byte == b'\n').unwrap();
        bytes[newline] = b' ';
        assert_eq!(
            verify_protocol_fixture(&capture, path, &bytes),
            Err("APS protocol fixture digest mismatch")
        );
        assert!(
            catch_unwind(|| {
                run_fixture("single_banner", path, from_utf8(&bytes).unwrap());
            })
            .is_err()
        );
    }

    #[test]
    fn aps_bidder_toml_owns_all_source_witnesses_and_the_derived_parameter_capture() {
        let bytes = read_document(&root().join("bidder.toml"));
        let document: toml::Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
        assert_eq!(document["provenance"]["trees"].as_array().unwrap().len(), 3);
        let artifacts = document["provenance"]["artifacts"].as_array().unwrap();
        assert_eq!(artifacts.len(), 29); // APS 22 + shared helpers 6 + license 1.
        let parameters = artifacts
            .iter()
            .find(|row| row["source_path"].as_str() == Some("adapters/aps/params_test.go"))
            .unwrap();
        assert_eq!(parameters["category"].as_str(), Some("parameter"));
        assert_eq!(parameters["source_bytes"].as_integer(), Some(1313));
        assert!(parameters["transformation"].as_str().is_some());
        assert_eq!(
            parameters["retained"]["source_path"].as_str(),
            Some("fixtures/parameters.json")
        );
        assert_eq!(
            parameters["retained"]["source_bytes"].as_integer(),
            Some(2392)
        );
        assert_eq!(
            parameters["retained"]["source_sha256"].as_str(),
            Some(PARAMETER_SHA256)
        );
        decode_manifest(&bytes).unwrap();
    }

    #[test]
    fn aps_filtered_protocol_dispatch_requires_the_reviewed_toml_witnesses() {
        let bytes = read_document(&root().join("bidder.toml"));
        let path = "exemplary/single-banner.json";
        let fixture = read_document(&root().join(format!("fixtures/protocol/{path}")));
        assert_eq!(verify_protocol_dispatch(&bytes, path, &fixture), Ok(()));
        let mut document: toml::Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
        document["provenance"]["sources"]["supplemental"]["revision"] =
            toml::Value::String(CATALOG_REVISION.into());
        assert!(
            verify_protocol_dispatch(
                toml::to_string(&document).unwrap().as_bytes(),
                path,
                &fixture,
            )
            .is_err()
        );
        // A single filtered case must still validate unrelated helpers, license,
        // metadata, schema and the independently transformed parameter capture.
        let original: toml::Value = toml::from_str(from_utf8(&bytes).unwrap()).unwrap();
        for index in 0..29 {
            let mut changed = original.clone();
            changed["provenance"]["artifacts"][index]["source_sha256"] =
                toml::Value::String("0".repeat(64));
            assert!(
                verify_protocol_dispatch(
                    toml::to_string(&changed).unwrap().as_bytes(),
                    path,
                    &fixture,
                )
                .is_err(),
                "artifact {index}"
            );
        }
    }

    #[test]
    fn aps_protocol_dispatch_refuses_changed_expected_members_operations_and_counts() {
        let path = "exemplary/single-banner.json";
        let bytes = read_document(&root().join(format!("fixtures/protocol/{path}")));
        for field in [
            "members",
            "operations",
            "http_calls",
            "bid_responses",
            "request_errors",
            "bid_errors",
        ] {
            let mut changed = manifest();
            let case = &mut changed.protocol_cases[2];
            match field {
                "members" => {
                    case.members.pop();
                }
                "operations" => {
                    case.operations.pop();
                }
                "http_calls" => case.http_calls += 1,
                "bid_responses" => case.bid_responses += 1,
                "request_errors" => case.request_errors += 1,
                "bid_errors" => case.bid_errors += 1,
                _ => panic!("unknown fixture mutation"),
            }
            assert!(
                verify_protocol_fixture(&changed, path, &bytes).is_err(),
                "{field}"
            );
        }
        for field in [
            "aps_owned_blobs",
            "shared_witnesses",
            "protocol_fixtures",
            "parameter_cases",
            "parameter_accepts",
            "parameter_rejects",
            "make_requests",
            "make_bids",
        ] {
            let mut changed = manifest();
            let counts = &mut changed.counts;
            match field {
                "aps_owned_blobs" => counts.aps_owned_blobs += 1,
                "shared_witnesses" => counts.shared_witnesses += 1,
                "protocol_fixtures" => counts.protocol_fixtures += 1,
                "parameter_cases" => counts.parameter_cases += 1,
                "parameter_accepts" => counts.parameter_accepts += 1,
                "parameter_rejects" => counts.parameter_rejects += 1,
                "make_requests" => counts.make_requests += 1,
                "make_bids" => counts.make_bids += 1,
                _ => panic!("unknown fixture mutation"),
            }
            assert!(
                verify_protocol_fixture(&changed, path, &bytes).is_err(),
                "{field}"
            );
        }
    }

    struct ParameterPin {
        raw_bytes: usize,
        source_offset: usize,
        sha256: &'static str,
    }

    // Digests of exact raw spans in the pinned parameter-test blob, in table order.
    const PARAMETER_PINS: [ParameterPin; 14] = [
        ParameterPin {
            raw_bytes: 28,
            source_offset: 919,
            sha256: "949b09e3a2dd594a64c2d43c5498d9e0e23f2088b46275af2a01444693fc8fde",
        },
        ParameterPin {
            raw_bytes: 20,
            source_offset: 952,
            sha256: "7f13760d9ebfbe153803ddb4d87356f9a9b3599347cc5a98af9750763dc68b59",
        },
        ParameterPin {
            raw_bytes: 42,
            source_offset: 977,
            sha256: "941d4451459d375e04eb893723e80cb64aff66be6592c8ba6e8f7d4d0798ec62",
        },
        ParameterPin {
            raw_bytes: 42,
            source_offset: 1024,
            sha256: "decc6aba258c53455ea7eeccdc85c108e175655d414fbc27e19d83883cd139eb",
        },
        ParameterPin {
            raw_bytes: 42,
            source_offset: 1071,
            sha256: "1a717f0e434117c498c38bb3d4d4bd93a3ec109dc3e27f25f22c12875e090978",
        },
        ParameterPin {
            raw_bytes: 2,
            source_offset: 1151,
            sha256: "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
        },
        ParameterPin {
            raw_bytes: 17,
            source_offset: 1158,
            sha256: "f2250e245d5aec3cc88189ade5cf08e685ca6cd9503c4eaa09228454fb0e6c48",
        },
        ParameterPin {
            raw_bytes: 42,
            source_offset: 1180,
            sha256: "2beec50808687ab2e0cd15de2d54bdb39e20dbc70c0e271c77ea0703ccbf544d",
        },
        ParameterPin {
            raw_bytes: 40,
            source_offset: 1227,
            sha256: "ead274a1c16ba24b3f86043b69f680720c9c6af26429ff98de2179de6fe9d6f8",
        },
        ParameterPin {
            raw_bytes: 2,
            source_offset: 1272,
            sha256: "4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
        },
        ParameterPin {
            raw_bytes: 5,
            source_offset: 1279,
            sha256: "a68688f483205ae1a50d24a4507fbb76772e64c3905ef364dc82c61d19c81bd9",
        },
        ParameterPin {
            raw_bytes: 1,
            source_offset: 1289,
            sha256: "6b86b273ff34fce19d6b804eff5a3f5747ada4eaa22f1d49c01e52ddb7875b4b",
        },
        ParameterPin {
            raw_bytes: 4,
            source_offset: 1295,
            sha256: "b5bea41b6c623f7c09f1bf24dcae58ebab3c0cdd90ad966bc43a45b44867e12b",
        },
        ParameterPin {
            raw_bytes: 4,
            source_offset: 1304,
            sha256: "74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b",
        },
    ];

    // Immutable Git witnesses, in source-path order. No foreign programs are retained.
    type SourcePin = (
        &'static str,
        &'static str,
        usize,
        &'static str,
        Option<&'static str>,
    );
    const APS_SOURCES: [SourcePin; 22] = [
        (
            "adapters/aps/aps.go",
            "6c9b88a872c1835bf77cba7a4623cdead9d58c23",
            8436,
            "ccd42e9162b7e9fd66bfe2a9da1d1b81fe545db9f4ac332b83f6c1d98d59ea5d",
            None,
        ),
        (
            "adapters/aps/aps_test.go",
            "80f92f7524fab6ae2a37124e3022f5d4f9a1e153",
            623,
            "f365ee454a8c64f270da349573f55d3e77b920a908e6f3b6cee3930e26d9f276",
            None,
        ),
        (
            "adapters/aps/apstest/exemplary/multi-slot.json",
            "a0cfb76742fabf37b5d834a2328274432c5d955e",
            3570,
            "34054e9800ebe72bb89480f246de966a5b87dd401883879c75b124a0b3d52c9f",
            Some("fixtures/protocol/exemplary/multi-slot.json"),
        ),
        (
            "adapters/aps/apstest/exemplary/multiformat-imp.json",
            "5f8fb67d0b03e814225e94352f080ae4c0b3bcb3",
            2472,
            "e7acca7da02acb330ff95f4cc79733e2651027b69b556773845a4c4bb77dd285",
            Some("fixtures/protocol/exemplary/multiformat-imp.json"),
        ),
        (
            "adapters/aps/apstest/exemplary/single-banner.json",
            "80cebec7517c8d7f3f379addc90667161e915b75",
            2392,
            "e8b8a97dc046fe9f3a9ac29ccbfe47d3f605845445e30bb28df93df1f1a04403",
            Some("fixtures/protocol/exemplary/single-banner.json"),
        ),
        (
            "adapters/aps/apstest/exemplary/single-video.json",
            "63309a87af6b24e7270e6b7956097f2dacdfed20",
            2261,
            "2c3e291f91f4c04a2aa57edd4657632f223063cccccdcf38e1799c70054b70a0",
            Some("fixtures/protocol/exemplary/single-video.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/banner-partial-size-overwrite.json",
            "7225423f29f40809d551b90b5a48d649974fd71f",
            1525,
            "c210d9ee4e42c292f60fd9e54c8811592e12e8a3892259fd716505f5c0244115",
            Some("fixtures/protocol/supplemental/banner-partial-size-overwrite.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/defaults-currency-to-usd.json",
            "265f05598fb0b5d0365de8f43cb903f8b17660c0",
            2368,
            "f721f033b9a89f173d4f9a113e752895786d6b6f345a39201962f2dd521184c5",
            Some("fixtures/protocol/supplemental/defaults-currency-to-usd.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/region-na-explicit.json",
            "3d76aede9b8d7547ff5448cd150c255039972ae4",
            1540,
            "74d6eaa15a5b3dddfb4e0a0a10568cd09f2b2e22568a8976846ff8bb43c68ebf",
            Some("fixtures/protocol/supplemental/region-na-explicit.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/rejects-mismatched-account.json",
            "2a9b110e36cff242661bcd20dfcc5740da334a41",
            749,
            "b6c811fb21570a8e6531f4fdefcfb41ae4304824f7130ada9f3f0d166f86585b",
            Some("fixtures/protocol/supplemental/rejects-mismatched-account.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/rejects-unsupported-region.json",
            "9b1253fd9180d85a76924bc2e258ea22f90199d5",
            634,
            "0e90cb811a5f3ff133d5ed32e722bb060a2a6c25ffbe16696ffb1fc82e8aada2",
            Some("fixtures/protocol/supplemental/rejects-unsupported-region.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/requires-account.json",
            "da968534b5aaed0559fd1d890de0f3b1a14f0fc1",
            556,
            "82f491dbd585a41139368fba6ca7d890e68a956061dcdaeb42ec2faa164e2e3b",
            Some("fixtures/protocol/supplemental/requires-account.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/resolves-eu-region.json",
            "ecc3df1dbdea4dc0035a82729427b951f69b1ad9",
            1626,
            "2027b9d3cb4ee4617d537c0e8cbc7e63b2e7d3e4cdd6f7fe385c7c011496e66b",
            Some("fixtures/protocol/supplemental/resolves-eu-region.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/status-204.json",
            "0655d294f0d3a4d385575f1ebe306540326ce854",
            1140,
            "b1e7cc01b61e994814f1ed2eb5563fa29073cfed3af7abae8cbc2782259a9231",
            Some("fixtures/protocol/supplemental/status-204.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/status-400.json",
            "05db0f758c78cdd9aa602763bf68a3a3b9e5bc7c",
            1292,
            "01a5ca122d16cdc7183cf62ca2a4136a3e8162d8d3328d04d0f4c918c2daf25d",
            Some("fixtures/protocol/supplemental/status-400.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/status-500.json",
            "b120922e63204140beda296ed1d2c2526cd5a53d",
            1296,
            "e5b532014a94e8d5b7cd8c96fc66a5f8518b39f6b39d6243f7d315ce327fbdeb",
            Some("fixtures/protocol/supplemental/status-500.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/strips-sensitive-data.json",
            "8e5a3cebfd97635cf2ce6bad4cefaf6503236e15",
            2703,
            "84ed038d204aeb0b0f011e939130d2ed6dd29829b5b7d0cca1af2a4d3a7ef051",
            Some("fixtures/protocol/supplemental/strips-sensitive-data.json"),
        ),
        (
            "adapters/aps/apstest/supplemental/test-mode-appends-debug.json",
            "7fdf139c17fc081fcaf0466df7d3396e1fa48280",
            1326,
            "ce61dca306d19bc3b51e6d1e5f24b9833e7359867f4ce0aef081d45af532d360",
            Some("fixtures/protocol/supplemental/test-mode-appends-debug.json"),
        ),
        (
            "adapters/aps/params_test.go",
            PARAMETER_BLOB,
            1313,
            "96c244d0a61a4002b2228e060a019095308a59d2c2f0813017a9a620df2a0194",
            None,
        ),
        (
            "openrtb_ext/imp_aps.go",
            "f84a3e9a8fb3c2b6578923e5f3b2dc872fd4b1da",
            190,
            "856101ddab40aca82ebc21c43e890823cba5e3bfde7ffb3af88e6197faeba085",
            None,
        ),
        (
            "static/bidder-info/aps.yaml",
            "dded94f05e4cf60c327877721dc2ce96cb7abfc4",
            321,
            "2204cbfe1632647d7b63015d9a8dd314a51c28209482851db33802aef60f5d8d",
            None,
        ),
        (
            "static/bidder-params/aps.json",
            "48f5de753f4175e671f0d584ab532120c8c9cd5d",
            513,
            "64f033004924ed57417549f59b08df4f54f12b40e9562379d0276d187d406dc3",
            Some("parameters.json"),
        ),
    ];

    const SHARED_WITNESSES: [SourcePin; 6] = [
        (
            "adapters/adapterstest/test_json.go",
            "911a5b408dad2d62ab7d14d537bd304b9b74f9ab",
            20531,
            "f8e32ce7f948b1b7a5ee73cc3e5ca431a8977e8f2147ab88300871cf4f4356a0",
            None,
        ),
        (
            "adapters/bidder.go",
            "0be99b00809a1024a51ab92be9e263ef8526f44a",
            7243,
            "bf103cd4d95c9d0a42e3c61fa8970d5ec3590dc45139099e93dd0ef5e5ea1254",
            None,
        ),
        (
            "adapters/infoawarebidder.go",
            "414a9d3599bf45bef2634a3c284b35402f258c1c",
            10377,
            "d86fbf6363654210de30e84da653b13ea7115263545cba631238a6c879854da8",
            None,
        ),
        (
            "exchange/adapter_builders.go",
            "93d601431acdb09ab518599cb799c283b3462f85",
            31697,
            "2574943069d062dbfb6406f53acfe1f9acca195336efb5afd9d318db061ee374",
            None,
        ),
        (
            "exchange/bidder.go",
            "87d3eaa719a0f3732a4c45e7c6d63f2c4521c3ca",
            34671,
            "dd398f54bd4667bd8874d4a8b202dd20abde19bc94fc25f0dbe023ac163155b9",
            None,
        ),
        (
            "openrtb_ext/bidders.go",
            "f87caa3fd2eab42bf99c316239bd9a0191c18bbd",
            28110,
            "53774709e54ef283dd6128fd528a60eac16144c79c725be32028f37f9114f59c",
            None,
        ),
    ];
    const LICENSE: SourcePin = (
        "LICENSE",
        "b7fa401fa4b79b208b1dbc4679b09b8f4691adbf",
        10760,
        "9d130cc11efd232f041473f0cd62c43806b9389d63b599c8ee0862b699e8bc58",
        None,
    );

    // Independent test expectations, not a second on-disk provenance document.
    #[derive(Clone, Debug)]
    struct Capture {
        trees: Vec<Tree>,
        counts: Counts,
        aps_sources: Vec<Source>,
        shared_witnesses: Vec<Source>,
        license: Source,
        parameters: Document,
        protocol_cases: Vec<ProtocolCase>,
    }

    #[derive(Clone, Debug)]
    struct Tree {
        path: String,
        blob: String,
        recursive_blobs: usize,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct Counts {
        aps_owned_blobs: usize,
        shared_witnesses: usize,
        protocol_fixtures: usize,
        parameter_cases: usize,
        parameter_accepts: usize,
        parameter_rejects: usize,
        make_requests: usize,
        make_bids: usize,
    }

    #[derive(Clone, Debug)]
    struct Source {
        path: String,
        mode: String,
        blob: String,
        raw_bytes: usize,
        sha256: String,
        retention: Retention,
    }

    #[derive(Clone, Debug)]
    enum Retention {
        WitnessOnly,
        Verbatim { path: String },
    }

    #[derive(Clone, Debug)]
    struct Document {
        path: String,
        raw_bytes: usize,
        sha256: String,
    }

    #[derive(Clone, Debug)]
    struct ProtocolCase {
        path: String,
        members: Vec<String>,
        operations: Vec<Operation>,
        http_calls: usize,
        bid_responses: usize,
        request_errors: usize,
        bid_errors: usize,
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Operation {
        MakeRequests,
        MakeBids,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Parameters {
        schema_version: u16,
        revision: String,
        origin: String,
        source_path: String,
        source_blob: String,
        offset_unit: String,
        cases: Vec<ParameterCase>,
    }

    #[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct ParameterCase {
        group: String,
        index: usize,
        input: String,
        valid: bool,
        source_offset: usize,
    }

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders/aps")
    }

    fn read_document(path: &Path) -> Vec<u8> {
        use std::io::Read as _;

        let file = fs::File::open(path).unwrap_or_else(|error| {
            panic!(
                "APS reference capture is required: {}: {error}",
                path.display()
            )
        });
        assert!(file.metadata().unwrap().len() <= MAX_DOCUMENT_BYTES);
        let mut bytes = Vec::new();
        file.take(MAX_DOCUMENT_BYTES + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(u64::try_from(bytes.len()).unwrap() <= MAX_DOCUMENT_BYTES);
        bytes
    }

    fn sha256(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn source_pin(pin: SourcePin) -> Source {
        Source {
            path: pin.0.into(),
            mode: if pin.0 == "exchange/adapter_builders.go" {
                "100755"
            } else {
                "100644"
            }
            .into(),
            blob: pin.1.into(),
            raw_bytes: pin.2,
            sha256: pin.3.into(),
            retention: pin
                .4
                .map_or(Retention::WitnessOnly, |path| Retention::Verbatim {
                    path: path.into(),
                }),
        }
    }

    fn expected_protocol_cases() -> Vec<ProtocolCase> {
        // Counts are source fixture expectations, not executions of either adapter.
        let counts_table = [
            (1, 1, 0, 0),
            (1, 1, 0, 0),
            (1, 1, 0, 0),
            (1, 1, 0, 0),
            (1, 0, 0, 0),
            (1, 1, 0, 0),
            (1, 0, 0, 0),
            (0, 0, 1, 0),
            (0, 0, 1, 0),
            (0, 0, 1, 0),
            (1, 0, 0, 0),
            (1, 0, 0, 0),
            (1, 0, 0, 1),
            (1, 0, 0, 1),
            (1, 1, 0, 0),
            (1, 0, 0, 0),
        ];
        APS_SOURCES[2..18]
            .iter()
            .zip(counts_table)
            .enumerate()
            .map(|(index, (source, row_counts))| {
                let (http_calls, bid_responses, request_errors, bid_errors) = row_counts;
                let mut members = vec!["httpCalls", "mockBidRequest"];
                if !matches!(index, 8 | 9) {
                    members.push("expectedBidResponses");
                }
                if request_errors > 0 {
                    members.push("expectedMakeRequestsErrors");
                }
                if bid_errors > 0 {
                    members.push("expectedMakeBidsErrors");
                }
                members.sort_unstable();
                let mut operations = vec![Operation::MakeRequests];
                if http_calls > 0 {
                    operations.push(Operation::MakeBids);
                }
                ProtocolCase {
                    path: source.4.unwrap().into(),
                    members: members.into_iter().map(String::from).collect(),
                    operations,
                    http_calls,
                    bid_responses,
                    request_errors,
                    bid_errors,
                }
            })
            .collect()
    }

    fn expected_capture() -> Capture {
        Capture {
            trees: [
                (
                    "adapters/aps",
                    "a8d550dbcee0ab4ef95d6478d1aeafdf4e6404d8",
                    19,
                ),
                (
                    "adapters/aps/apstest/exemplary",
                    "bcf872996ca20ccc80a9808aac3344b4ca4b5211",
                    4,
                ),
                (
                    "adapters/aps/apstest/supplemental",
                    "9abd725a7784bd551b565cf9602b121a2939e791",
                    12,
                ),
            ]
            .map(|(path, blob, recursive_blobs)| Tree {
                path: path.into(),
                blob: blob.into(),
                recursive_blobs,
            })
            .into(),
            counts: Counts {
                aps_owned_blobs: 22,
                shared_witnesses: 6,
                protocol_fixtures: 16,
                parameter_cases: 14,
                parameter_accepts: 5,
                parameter_rejects: 9,
                make_requests: 16,
                make_bids: 13,
            },
            aps_sources: APS_SOURCES.into_iter().map(source_pin).collect(),
            shared_witnesses: SHARED_WITNESSES.into_iter().map(source_pin).collect(),
            license: source_pin(LICENSE),
            parameters: Document {
                path: "fixtures/parameters.json".into(),
                raw_bytes: 2392,
                sha256: PARAMETER_SHA256.into(),
            },
            protocol_cases: expected_protocol_cases(),
        }
    }

    fn expected_artifact(
        source: &Source,
        category: bidder_manifest::Category,
        parameters: &Document,
    ) -> bidder_manifest::Artifact {
        use bidder_manifest::{Artifact, Retained};

        let parameter = source.path == "adapters/aps/params_test.go";
        let retained = if parameter {
            Some(Retained {
                path: parameters.path.clone(),
                bytes: u64::try_from(parameters.raw_bytes).unwrap(),
                sha256: parameters.sha256.clone(),
            })
        } else if let Retention::Verbatim { path } = &source.retention {
            Some(Retained {
                path: path.clone(),
                bytes: u64::try_from(source.raw_bytes).unwrap(),
                sha256: source.sha256.clone(),
            })
        } else {
            None
        };
        Artifact {
            category,
            source: "supplemental".into(),
            source_path: source.path.clone(),
            source_blob: source.blob.clone(),
            source_bytes: u64::try_from(source.raw_bytes).unwrap(),
            source_sha256: source.sha256.clone(),
            retained,
            transformation: parameter.then(|| PARAMETER_TRANSFORMATION.into()),
            port_differences: if source.path == "static/bidder-info/aps.yaml" {
                vec![PUBLICATION_DIFFERENCES.into()]
            } else {
                Vec::new()
            },
        }
    }

    fn expected_bidder_manifest() -> bidder_manifest::Manifest {
        use bidder_manifest::{Category, Provenance, Source, SourceTree};

        let capture = expected_capture();
        let artifact = |source, category| expected_artifact(source, category, &capture.parameters);
        let artifacts = capture
            .aps_sources
            .iter()
            .map(|source| {
                let category = match source.path.as_str() {
                    "adapters/aps/params_test.go" => Category::Parameter,
                    "static/bidder-info/aps.yaml" => Category::Metadata,
                    "static/bidder-params/aps.json" => Category::Schema,
                    path if path.starts_with("adapters/aps/apstest/") => Category::Protocol,
                    _ => Category::Implementation,
                };
                artifact(source, category)
            })
            .chain(
                capture
                    .shared_witnesses
                    .iter()
                    .map(|source| artifact(source, Category::Helper)),
            )
            .chain([artifact(&capture.license, Category::License)])
            .collect();
        bidder_manifest::Manifest {
            endpoint: None,
            settings: None,
            key: "aps".into(),
            factory: true,
            required_feature: None,
            native_only: true,
            definitions: BTreeMap::from([(
                "aps".into(),
                toml::from_str(
                    r#"
                    defaultEnabled = false
                    endpoint = "https://{{.Host}}/e/pb/bid"
                    endpointCompression = "gzip"
                    gvlVendorID = 793
                    hostAllowlist = [
                        "s2s.prebid.bid-na.ads.aps.amazon-adsystem.com",
                        "s2s.prebid.bid-eu.ads.aps.amazon-adsystem.com",
                        "s2s.prebid.bid-fe.ads.aps.amazon-adsystem.com",
                    ]
                    [capabilities.site]
                    mediaTypes = ["banner", "video"]
                    [maintainer]
                    email = "aps-prebid@amazon.com"
                "#,
                )
                .unwrap(),
            )]),
            provenance: Provenance::Reference {
                sources: BTreeMap::from([(
                    "supplemental".into(),
                    Source {
                        repository: "https://github.com/prebid/prebid-server.git".into(),
                        revision: REVISION.into(),
                    },
                )]),
                trees: capture
                    .trees
                    .iter()
                    .enumerate()
                    .map(|(index, tree)| SourceTree {
                        category: if index == 0 {
                            Category::Implementation
                        } else {
                            Category::Protocol
                        },
                        source: "supplemental".into(),
                        source_path: tree.path.clone(),
                        tree: tree.blob.clone(),
                        port_differences: if index == 0 {
                            vec![PORT_DIFFERENCES.into()]
                        } else {
                            Vec::new()
                        },
                    })
                    .collect(),
                artifacts,
            },
        }
    }

    fn decode_manifest(bytes: &[u8]) -> Result<Capture, String> {
        let length = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
        if length > MAX_DOCUMENT_BYTES {
            return Err("APS manifest exceeds capture limit".into());
        }
        let source = from_utf8(bytes).map_err(|error| error.to_string())?;
        // The local strict DTO rejects unknown/duplicate fields before any
        // projection. Compare every field to independent, frozen source pins.
        let manifest: bidder_manifest::Manifest =
            toml::from_str(source).map_err(|error| error.to_string())?;
        if serde_json::to_value(&manifest).map_err(|error| error.to_string())?
            != serde_json::to_value(expected_bidder_manifest())
                .map_err(|error| error.to_string())?
        {
            return Err("APS manifest differs from the reviewed ordered source witnesses".into());
        }
        Ok(expected_capture())
    }

    fn manifest_bytes() -> Vec<u8> {
        read_document(&root().join("bidder.toml"))
    }

    fn manifest() -> Capture {
        decode_manifest(&manifest_bytes()).expect("strict APS bidder-owned source manifest")
    }

    fn decode_parameters(bytes: &[u8]) -> Result<Parameters, String> {
        let length = u64::try_from(bytes.len()).map_err(|error| error.to_string())?;
        if length > MAX_DOCUMENT_BYTES {
            return Err("APS parameter capture exceeds capture limit".into());
        }
        let shape: Value = serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        require_object(&shape)?;
        for row in shape["cases"]
            .as_array()
            .ok_or("expected APS parameter array")?
        {
            require_object(row)?;
        }
        let parameters: Parameters =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        if parameters.schema_version != 1
            || parameters.revision != REVISION
            || parameters.origin != "source-test-expectations-not-executed"
            || parameters.source_path != "adapters/aps/params_test.go"
            || parameters.source_blob != PARAMETER_BLOB
            || parameters.offset_unit != "zero-based-raw-byte"
            || parameters.cases.len() != PARAMETER_PINS.len()
        {
            return Err("APS parameter source identity or case count changed".into());
        }
        for (index, (row, pin)) in parameters.cases.iter().zip(&PARAMETER_PINS).enumerate() {
            let valid = index < 5;
            if row.group != if valid { "valid" } else { "invalid" }
                || row.index
                    != if valid {
                        index
                    } else {
                        index
                            .checked_sub(5)
                            .ok_or("invalid APS parameter group index")?
                    }
                || row.valid != valid
                || row.source_offset != pin.source_offset
                || row.input.len() != pin.raw_bytes
                || sha256(row.input.as_bytes()) != pin.sha256
            {
                return Err("APS parameter input, verdict, order or byte witness changed".into());
            }
        }
        Ok(parameters)
    }

    fn require_object(value: &Value) -> Result<(), String> {
        value
            .as_object()
            .map(|_| ())
            .ok_or_else(|| "expected APS capture object".into())
    }

    fn parameters() -> Parameters {
        let bytes = read_document(&root().join("fixtures/parameters.json"));
        assert_eq!(bytes.len(), 2392);
        assert_eq!(sha256(&bytes), PARAMETER_SHA256);
        decode_parameters(&bytes).expect("all 14 source parameter expectations")
    }

    fn files_below(root: &Path) -> Vec<String> {
        fn visit(base: &Path, path: &Path, files: &mut Vec<String>) {
            for result in fs::read_dir(path).expect("captured fixture directory") {
                let entry = result.unwrap();
                let kind = entry.file_type().unwrap();
                assert!(
                    !kind.is_symlink(),
                    "fixture symlinks are not source captures"
                );
                if kind.is_dir() {
                    visit(base, &entry.path(), files);
                } else {
                    assert!(entry.path().is_file(), "captures must be regular files");
                    files.push(
                        entry
                            .path()
                            .strip_prefix(base)
                            .unwrap()
                            .components()
                            .map(|component| component.as_os_str().to_str().unwrap())
                            .collect::<Vec<_>>()
                            .join("/"),
                    );
                }
            }
        }
        let mut files = Vec::new();
        visit(root, root, &mut files);
        files.sort();
        files
    }

    #[test]
    fn aps_manifest_freezes_exact_ordered_sources_modes_counts_and_operations() {
        let capture = manifest();
        assert_eq!(capture.aps_sources.len(), 22);
        assert_eq!(capture.protocol_cases.len(), 16);
        assert_eq!(
            capture
                .protocol_cases
                .iter()
                .map(|row| row.http_calls)
                .sum::<usize>(),
            13
        );
        assert_eq!(
            capture
                .protocol_cases
                .iter()
                .map(|row| row.bid_responses)
                .sum::<usize>(),
            6
        );
        assert_eq!(
            capture
                .protocol_cases
                .iter()
                .map(|row| row.request_errors)
                .sum::<usize>(),
            3
        );
        assert_eq!(
            capture
                .protocol_cases
                .iter()
                .map(|row| row.bid_errors)
                .sum::<usize>(),
            2
        );
    }

    fn capture_paths(capture: &Capture) -> BTreeSet<String> {
        let mut paths = BTreeSet::from([capture.parameters.path.clone()]);
        for source in capture
            .aps_sources
            .iter()
            .chain(&capture.shared_witnesses)
            .chain([&capture.license])
        {
            if let Retention::Verbatim { path } = &source.retention {
                assert!(paths.insert(path.clone()), "duplicate capture path: {path}");
            }
        }
        paths
    }

    #[test]
    fn aps_capture_has_only_declared_data_and_retains_every_raw_byte() {
        let capture = manifest();
        for source in capture
            .aps_sources
            .iter()
            .chain(&capture.shared_witnesses)
            .chain([&capture.license])
        {
            if let Retention::Verbatim { path } = &source.retention {
                let bytes = read_document(&root().join(path));
                assert_eq!(bytes.len(), source.raw_bytes, "{}", source.path);
                assert_eq!(sha256(&bytes), source.sha256, "{}", source.path);
                assert!(
                    !Path::new(path)
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("go")),
                    "foreign programs must remain witnesses only"
                );
            }
        }
        let mut paths = capture_paths(&capture);
        assert_eq!(paths.len(), 18); // 16 protocol + schema + parameter cases.
        for code in [
            "tests/allocation_conformance.rs",
            "bidder.toml",
            "tests/conformance.rs",
            "mod.rs",
            "request.rs",
            "response.rs",
            "fixtures/factory.json",
            "tests/schema/inventory-exclusion.toml",
            "tests/schema/semantics.toml",
        ] {
            assert!(
                paths.insert(code.to_owned()),
                "capture shadows code: {code}"
            );
        }
        assert_eq!(paths.len(), 27);
        assert_eq!(files_below(&root()), paths.into_iter().collect::<Vec<_>>());
    }

    #[test]
    fn aps_capture_bytes_are_not_subject_to_checkout_line_ending_conversion() {
        let paths = capture_paths(&manifest())
            .into_iter()
            .map(|path| format!("src/bidders/aps/{path}"))
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 18);
        let output = Command::new("git")
            .arg("-C")
            .arg(env!("CARGO_MANIFEST_DIR"))
            .args(["check-attr", "-z", "text", "--"])
            .args(&paths)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Git must resolve capture attributes"
        );
        let fields = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
        let (rows, trailing) = fields.as_chunks::<3>();
        assert_eq!(rows.len(), 18);
        assert_eq!(trailing, [b"".as_slice()]);
        for (row, path) in rows.iter().zip(&paths) {
            let [recorded_path, attribute, value] = *row;
            assert_eq!(recorded_path, path.as_bytes());
            assert_eq!(attribute, b"text");
            assert_eq!(value, b"unset", "{path} must retain its captured bytes");
        }
    }

    #[test]
    fn aps_protocol_capture_retains_exact_source_members_and_operation_coverage() {
        let capture = manifest();
        for case in &capture.protocol_cases {
            let fixture: Value =
                serde_json::from_slice(&read_document(&root().join(&case.path))).unwrap();
            let mut members = fixture
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            members.sort();
            assert_eq!(members, case.members, "{}", case.path);
            for (field, count) in [
                ("httpCalls", case.http_calls),
                ("expectedBidResponses", case.bid_responses),
                ("expectedMakeRequestsErrors", case.request_errors),
                ("expectedMakeBidsErrors", case.bid_errors),
            ] {
                assert_eq!(
                    fixture
                        .get(field)
                        .map_or(0, |value| value.as_array().unwrap().len()),
                    count,
                    "{} {field}",
                    case.path
                );
            }
            for call in fixture["httpCalls"].as_array().unwrap() {
                let request = call["expectedRequest"].as_object().unwrap();
                assert_eq!(
                    request.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                    BTreeSet::from(["body", "impIDs", "uri"])
                );
                // Preserve the reference identity; reviewed Auctioneer differences belong to adjacent protocol tests.
                assert_eq!(
                    request["body"]["ext"]["sdk"],
                    json!({"version": "1.0.0", "source": "prebid-server"})
                );
            }
        }
    }

    #[test]
    fn aps_parameters_preserve_five_accepts_nine_rejects_and_source_offsets() {
        let corpus = parameters();
        assert_eq!(corpus.cases.len(), 14);
        assert_eq!(corpus.cases.iter().filter(|row| row.valid).count(), 5);
        assert_eq!(corpus.cases.iter().filter(|row| !row.valid).count(), 9);
    }

    #[test]
    fn aps_package_uses_canonical_parameter_schema_without_duplicate_schema_or_license() {
        let bidder = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders/aps");
        let schema = read_document(&bidder.join("parameters.json"));
        assert_eq!(schema.len(), 513);
        assert_eq!(
            sha256(&schema),
            "64f033004924ed57417549f59b08df4f54f12b40e9562379d0276d187d406dc3"
        );
        let cases = read_document(&bidder.join("fixtures/parameters.json"));
        assert_eq!(cases.len(), 2392);
        assert_eq!(sha256(&cases), PARAMETER_SHA256);
        let corpus = decode_parameters(&cases).unwrap();
        assert_eq!(corpus.cases.len(), 14);
        assert_eq!(corpus.cases.iter().filter(|row| row.valid).count(), 5);
        assert_eq!(corpus.cases.iter().filter(|row| !row.valid).count(), 9);
        assert!(!bidder.join("fixtures/schema/aps.json").exists());
        assert!(!bidder.join("fixtures/LICENSE.reference").exists());
        assert!(!bidder.join("LICENSE.reference").exists());
        let license = read_document(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders/LICENSE.reference"),
        );
        assert_eq!(license.len(), LICENSE.2);
        assert_eq!(sha256(&license), LICENSE.3);
    }

    #[test]
    fn aps_parameter_expectations_match_captured_draft4_schema() {
        let _capture = manifest();
        let schema: Value =
            serde_json::from_slice(&read_document(&root().join("parameters.json"))).unwrap();
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft4)
            .build(&schema)
            .unwrap();
        for row in parameters().cases {
            let instance: Value = serde_json::from_str(&row.input).unwrap();
            assert_eq!(
                validator.is_valid(&instance),
                row.valid,
                "{} {}: {}",
                row.group,
                row.index,
                row.input
            );
        }
        // Schema acceptance is intentionally distinct from runtime account trimming.
        assert!(validator.is_valid(&json!({"accountID": ""})));
        assert!(validator.is_valid(&json!({"accountID": "   "})));
    }

    #[test]
    fn aps_manifest_refuses_missing_extra_duplicate_or_reordered_witnesses() {
        let original: toml::Value = toml::from_str(from_utf8(&manifest_bytes()).unwrap()).unwrap();
        decode_manifest(toml::to_string(&original).unwrap().as_bytes()).unwrap();
        for field in ["trees", "artifacts"] {
            for mutation in ["missing", "extra", "duplicate", "reorder"] {
                let mut changed = original.clone();
                let rows = changed["provenance"][field].as_array_mut().unwrap();
                match mutation {
                    "missing" => {
                        rows.remove(0);
                    }
                    "extra" => {
                        let row = rows[0].clone();
                        rows.push(row);
                    }
                    "duplicate" => rows[1] = rows[0].clone(),
                    "reorder" => rows.swap(0, 1),
                    _ => panic!("unknown fixture mutation"),
                }
                assert_eq!(
                    decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                    "APS manifest differs from the reviewed ordered source witnesses",
                    "{field} {mutation}"
                );
            }
        }
    }

    #[test]
    fn aps_manifest_refuses_each_changed_artifact_and_retained_witness() {
        let original: toml::Value = toml::from_str(from_utf8(&manifest_bytes()).unwrap()).unwrap();
        // Include shared helpers and the license, not just protocol captures.
        for index in 0..29 {
            for field in [
                "category",
                "source",
                "source_path",
                "source_blob",
                "source_bytes",
                "source_sha256",
                "transformation",
                "port_differences",
            ] {
                let mut changed = original.clone();
                let value = match field {
                    "category" => toml::Value::String(
                        if original["provenance"]["artifacts"][index][field].as_str()
                            == Some("metadata")
                        {
                            "protocol"
                        } else {
                            "metadata"
                        }
                        .into(),
                    ),
                    "source_bytes" => toml::Value::Integer(0),
                    "port_differences" => {
                        toml::Value::Array(vec![toml::Value::String("unreviewed".into())])
                    }
                    _ => toml::Value::String("altered".into()),
                };
                let row = changed["provenance"]["artifacts"][index]
                    .as_table_mut()
                    .unwrap();
                assert_ne!(row.get(field), Some(&value));
                row.insert(field.into(), value);
                assert_eq!(
                    decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                    "APS manifest differs from the reviewed ordered source witnesses",
                    "artifact {index} {field}"
                );
            }
            let Some(retained) = original["provenance"]["artifacts"][index].get("retained") else {
                continue;
            };
            for field in ["source_path", "source_bytes", "source_sha256"] {
                let mut changed = original.clone();
                changed["provenance"]["artifacts"][index]["retained"][field] =
                    if field == "source_bytes" {
                        toml::Value::Integer(0)
                    } else {
                        toml::Value::String("altered".into())
                    };
                assert_eq!(
                    decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                    "APS manifest differs from the reviewed ordered source witnesses",
                    "artifact {index} retained {field}: {retained}"
                );
            }
            let mut changed = original.clone();
            changed["provenance"]["artifacts"][index]
                .as_table_mut()
                .unwrap()
                .remove("retained");
            assert_eq!(
                decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                "APS manifest differs from the reviewed ordered source witnesses"
            );
        }
    }

    #[test]
    fn aps_manifest_refuses_each_changed_implementation_and_protocol_tree() {
        let original: toml::Value = toml::from_str(from_utf8(&manifest_bytes()).unwrap()).unwrap();
        for index in 0..3 {
            for field in [
                "category",
                "source",
                "source_path",
                "tree",
                "port_differences",
            ] {
                let mut changed = original.clone();
                changed["provenance"]["trees"][index]
                    .as_table_mut()
                    .unwrap()
                    .insert(
                        field.into(),
                        if field == "category" {
                            toml::Value::String("helper".into())
                        } else if field == "port_differences" {
                            toml::Value::Array(vec![toml::Value::String("unreviewed".into())])
                        } else {
                            toml::Value::String("altered".into())
                        },
                    );
                assert_eq!(
                    decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                    "APS manifest differs from the reviewed ordered source witnesses",
                    "tree {index} {field}"
                );
            }
        }
    }

    #[test]
    fn aps_manifest_refuses_changed_identity_source_pin_and_publication() {
        let original: toml::Value = toml::from_str(from_utf8(&manifest_bytes()).unwrap()).unwrap();
        for (field, value) in [
            ("key", toml::Value::String("unreviewed".into())),
            ("factory", toml::Value::Boolean(false)),
            ("native_only", toml::Value::Boolean(false)),
            (
                "required_feature",
                toml::Value::String("insecure-bidders".into()),
            ),
            (
                "definitions",
                toml::Value::Table(toml::Table::from_iter([(
                    "aps".into(),
                    toml::Value::Table(toml::Table::new()),
                )])),
            ),
        ] {
            let mut changed = original.clone();
            changed.as_table_mut().unwrap().insert(field.into(), value);
            assert_eq!(
                decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                "APS manifest differs from the reviewed ordered source witnesses",
                "{field}"
            );
        }
        for field in ["repository", "revision"] {
            let mut changed = original.clone();
            changed["provenance"]["sources"]["supplemental"][field] =
                toml::Value::String(if field == "revision" {
                    CATALOG_REVISION.into()
                } else {
                    "https://example.com/unreviewed.git".into()
                });
            assert_eq!(
                decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                "APS manifest differs from the reviewed ordered source witnesses",
                "source {field}"
            );
        }
        for mutation in ["missing", "extra"] {
            let mut changed = original.clone();
            let sources = changed["provenance"]["sources"].as_table_mut().unwrap();
            if mutation == "missing" {
                sources.remove("supplemental");
            } else {
                sources.insert("unreviewed".into(), sources["supplemental"].clone());
            }
            assert_eq!(
                decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err(),
                "APS manifest differs from the reviewed ordered source witnesses"
            );
        }
    }

    #[test]
    fn aps_toml_refuses_unknown_duplicate_missing_and_invalid_field_shapes() {
        let bytes = manifest_bytes();
        let original = from_utf8(&bytes).unwrap();
        decode_manifest(&bytes).unwrap();
        for prefix in ["key = 'aps'\n", "unreviewed = true\n"] {
            let error = decode_manifest(format!("{prefix}{original}").as_bytes()).unwrap_err();
            assert!(
                error.contains(if prefix.starts_with("key") {
                    "duplicate key"
                } else {
                    "unknown field"
                }),
                "{error}"
            );
        }
        let duplicate = original.replacen(
            "category = \"implementation\"",
            "category = \"implementation\"\ncategory = \"implementation\"",
            1,
        );
        assert_ne!(duplicate, original);
        let duplicate_error = decode_manifest(duplicate.as_bytes()).unwrap_err();
        assert!(
            duplicate_error.contains("duplicate key"),
            "{duplicate_error}"
        );
        let document: toml::Value = toml::from_str(original).unwrap();
        for field in ["key", "factory", "provenance"] {
            let mut changed = document.clone();
            changed.as_table_mut().unwrap().remove(field);
            let missing_error =
                decode_manifest(toml::to_string(&changed).unwrap().as_bytes()).unwrap_err();
            assert!(
                missing_error.contains("missing field") && missing_error.contains(field),
                "{missing_error}"
            );
        }
        for field in ["trees", "artifacts"] {
            for index in 0..document["provenance"][field].as_array().unwrap().len() {
                let mut unknown = document.clone();
                unknown["provenance"][field][index]
                    .as_table_mut()
                    .unwrap()
                    .insert("unreviewed".into(), toml::Value::Boolean(true));
                let unknown_error =
                    decode_manifest(toml::to_string(&unknown).unwrap().as_bytes()).unwrap_err();
                assert!(
                    unknown_error.contains("unknown field") && unknown_error.contains("unreviewed"),
                    "{unknown_error}"
                );
                let mut missing = document.clone();
                missing["provenance"][field][index]
                    .as_table_mut()
                    .unwrap()
                    .remove("source");
                let missing_error =
                    decode_manifest(toml::to_string(&missing).unwrap().as_bytes()).unwrap_err();
                assert!(
                    missing_error.contains("missing field") && missing_error.contains("source"),
                    "{missing_error}"
                );
                let mut nontable = document.clone();
                nontable["provenance"][field][index] = toml::Value::Array(Vec::new());
                let shape_error =
                    decode_manifest(toml::to_string(&nontable).unwrap().as_bytes()).unwrap_err();
                assert!(
                    shape_error.contains("invalid length 0, expected struct"),
                    "{shape_error}"
                );
            }
        }
        for source in ["null", "[]", "true", "key = ['aps']", "key = 1"] {
            decode_manifest(source.as_bytes()).unwrap_err();
        }
        assert!(decode_manifest(&[0xff]).unwrap_err().contains("utf-8"));
        let oversized_length = usize::try_from(MAX_DOCUMENT_BYTES)
            .unwrap()
            .checked_add(1)
            .unwrap();
        assert_eq!(
            decode_manifest(&vec![b' '; oversized_length]).unwrap_err(),
            "APS manifest exceeds capture limit"
        );
    }

    #[test]
    fn aps_parameter_document_refuses_unknown_duplicate_fields_and_nonobject_shapes() {
        use serde::ser::{SerializeMap as _, Serializer};

        struct Duplicate<'a>(&'a serde_json::Map<String, Value>);
        impl Serialize for Duplicate<'_> {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let mut map = serializer.serialize_map(None)?;
                for (key, value) in self.0 {
                    map.serialize_entry(key, value)?;
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
        let corpus: Value =
            serde_json::from_slice(&read_document(&root().join("fixtures/parameters.json")))
                .unwrap();
        for document in [corpus] {
            let refuses = |bytes: &[u8]| decode_parameters(bytes).is_err();
            assert!(refuses(
                &serde_json::to_vec(&Duplicate(document.as_object().unwrap())).unwrap()
            ));
            let mut unknown = document.clone();
            unknown["unreviewed"] = json!(true);
            assert!(refuses(&serde_json::to_vec(&unknown).unwrap()));
            for bytes in [b"null".as_slice(), b"[]", b"true"] {
                assert!(refuses(bytes));
            }
            let field = "cases";
            let mut missing = document.clone();
            missing[field][0]
                .as_object_mut()
                .unwrap()
                .remove("source_offset");
            assert!(refuses(&serde_json::to_vec(&missing).unwrap()));
            let mut nested = document.clone();
            nested[field][0]["unreviewed"] = json!(true);
            assert!(refuses(&serde_json::to_vec(&nested).unwrap()));
            let mut array = document.clone();
            array[field][0] = Value::Array(
                document[field][0]
                    .as_object()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect(),
            );
            assert!(refuses(&serde_json::to_vec(&array).unwrap()));
            let positional = document
                .as_object()
                .unwrap()
                .values()
                .cloned()
                .collect::<Vec<_>>();
            assert!(refuses(&serde_json::to_vec(&positional).unwrap()));
            let oversized_length = usize::try_from(MAX_DOCUMENT_BYTES)
                .unwrap()
                .checked_add(1)
                .unwrap();
            assert!(refuses(&vec![b' '; oversized_length]));
        }
    }

    #[test]
    fn aps_parameter_capture_refuses_changed_inputs_offsets_verdicts_and_membership() {
        let original = serde_json::to_value(parameters()).unwrap();
        for field in [
            "schema_version",
            "revision",
            "origin",
            "source_path",
            "source_blob",
            "offset_unit",
        ] {
            let mut changed = original.clone();
            changed[field] = if field == "schema_version" {
                json!(2_u16)
            } else {
                json!("altered")
            };
            assert!(
                decode_parameters(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{field}"
            );
        }
        for index in 0..14 {
            for field in [
                "input_digest",
                "input_length",
                "source_offset",
                "valid",
                "group",
                "index",
            ] {
                let mut changed = original.clone();
                let row = &mut changed["cases"][index];
                match field {
                    "input_digest" => {
                        row["input"] = json!("x".repeat(row["input"].as_str().unwrap().len()));
                    }
                    "input_length" => {
                        row["input"] = json!(format!("{} ", row["input"].as_str().unwrap()));
                    }
                    "source_offset" => row[field] = json!(0_usize),
                    "valid" => row[field] = json!(!row[field].as_bool().unwrap()),
                    "group" => row[field] = json!("altered"),
                    "index" => row[field] = json!(999_usize),
                    _ => panic!("unknown fixture mutation"),
                }
                assert!(
                    decode_parameters(&serde_json::to_vec(&changed).unwrap()).is_err(),
                    "case {index} {field}"
                );
            }
        }
        for mutation in ["missing", "extra", "duplicate", "reorder"] {
            let mut changed = original.clone();
            let rows = changed["cases"].as_array_mut().unwrap();
            match mutation {
                "missing" => {
                    rows.remove(0);
                }
                "extra" => {
                    let row = rows[0].clone();
                    rows.push(row);
                }
                "duplicate" => rows[1] = rows[0].clone(),
                "reorder" => rows.swap(0, 1),
                _ => panic!("unknown fixture mutation"),
            }
            assert!(
                decode_parameters(&serde_json::to_vec(&changed).unwrap()).is_err(),
                "{mutation}"
            );
        }
    }

    #[test]
    fn aps_supplemental_pin_preserves_independent_original_inventory_and_bindings() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/conformance/fixtures/reference/catalog");
        // Freeze only the original cross-bidder identities and audited bindings.
        // Complete relocated schema integrity belongs to schema qualification.
        let inventory_bytes = fs::read(root.join("source-inventory.toml")).unwrap();
        assert_eq!(
            sha256(&inventory_bytes),
            "b22f74407fd081cd7d073b8724ba8f74e1385163b9bc30c2277342adc38bb6c6"
        );
        let inventory: toml::Value = toml::from_str(from_utf8(&inventory_bytes).unwrap()).unwrap();
        assert_eq!(inventory["revision"].as_str(), Some(CATALOG_REVISION));
        assert_eq!(inventory["counts"]["identities"].as_integer(), Some(358));
        assert_eq!(inventory["counts"]["schemas"].as_integer(), Some(261));
        let binding_bytes = fs::read(root.join("source-bindings.toml")).unwrap();
        assert_eq!(
            sha256(&binding_bytes),
            "d1a0f18d99f53cbd4f62fe74c5d1e5580f8688fce95a29b1b05ee934dbe9ff0c"
        );
        let bindings: toml::Value = toml::from_str(from_utf8(&binding_bytes).unwrap()).unwrap();
        assert_eq!(bindings["revision"].as_str(), Some(CATALOG_REVISION));
        assert_eq!(bindings["bindings"].as_table().unwrap().len(), 261);
        assert!(!bindings["bindings"].as_table().unwrap().contains_key("aps"));
        assert_ne!(REVISION, CATALOG_REVISION);
    }

    #[test]
    #[ignore = "requires AUCTIONEER__BIDDER__SOURCE_DIR containing the pinned reference Git objects"]
    fn supplemental_source_matches_immutable_reference() {
        let repository = env::var_os("AUCTIONEER__BIDDER__SOURCE_DIR")
            .expect("set AUCTIONEER__BIDDER__SOURCE_DIR");
        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&repository)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "immutable Git read failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        };
        let capture = manifest();
        let sources = capture
            .aps_sources
            .iter()
            .chain(&capture.shared_witnesses)
            .chain([&capture.license])
            .collect::<Vec<_>>();
        for producer in once(PROTOCOL_PRODUCER).chain(PARAMETER_PRODUCERS) {
            let (path, function) = producer.split_once(':').unwrap();
            let source = sources.iter().find(|source| source.path == path).unwrap();
            let bytes = git(&["cat-file", "blob", &source.blob]);
            assert!(
                from_utf8(&bytes)
                    .unwrap()
                    .contains(&format!("func {function}("))
            );
        }
        for source in &sources {
            let entry = git(&["ls-tree", REVISION, "--", &source.path]);
            assert_eq!(
                String::from_utf8(entry).unwrap(),
                format!("{} blob {}\t{}\n", source.mode, source.blob, source.path)
            );
            let bytes = git(&["cat-file", "blob", &source.blob]);
            assert_eq!(bytes.len(), source.raw_bytes, "{}", source.path);
            assert_eq!(sha256(&bytes), source.sha256, "{}", source.path);
            if let Retention::Verbatim { path } = &source.retention {
                assert_eq!(read_document(&root().join(path)), bytes, "{}", source.path);
            }
        }
        for tree in &capture.trees {
            let object = format!("{REVISION}:{}", tree.path);
            assert_eq!(
                git(&["rev-parse", &object]),
                format!("{}\n", tree.blob).as_bytes()
            );
            let entries = git(&["ls-tree", "-r", &tree.blob]);
            assert_eq!(
                entries
                    .split(|byte| *byte == b'\n')
                    .count()
                    .checked_sub(1)
                    .unwrap(),
                tree.recursive_blobs
            );
        }
        assert_eq!(
            read_document(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join("src/bidders/LICENSE.reference")
            ),
            git(&["cat-file", "blob", LICENSE.1])
        );
        let bytes = git(&["cat-file", "blob", PARAMETER_BLOB]);
        assert_eq!(
            bytes
                .split(|byte| *byte == b'`')
                .count()
                .checked_sub(1)
                .unwrap(),
            28
        );
        for row in parameters().cases {
            let start = row.source_offset;
            let end = start + row.input.len();
            assert_eq!(&bytes[start..end], row.input.as_bytes());
            assert_eq!(bytes[start - 1], b'`');
            assert_eq!(&bytes[end..end + 2], b"`,");
        }
    }
}
