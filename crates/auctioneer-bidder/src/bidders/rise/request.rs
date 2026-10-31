use crate::{BidderError, BidderRequestInfo, EndpointBuilder, OutboundRequestSink};
use auctioneer_openrtb::types::{
    bid_request::{BidRequest, Imp},
    field::JsonField,
};
use serde_json::{Map, Value};

pub(super) fn make_requests(
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    let org = extract_org(request).map_err(|error| vec![error])?;
    let _recorded = sink.write_json(
        info,
        |meta| {
            meta.set_method("POST")?;
            let values = [
                ("AccountID", info.account_id),
                ("PublisherID", info.publisher_id.unwrap_or_default()),
            ];
            let count = if info.publisher_id.is_some() { 2 } else { 1 };
            EndpointBuilder::from_request(info, values.get(..count).unwrap_or_default())
                .expand_with_query_into(meta, &[("publisher_id", org)])?;
            meta.header("Content-Type", "application/json;charset=utf-8")?;
            for imp in &request.imp {
                meta.imp_id(&imp.id)?;
            }
            Ok(())
        },
        request,
    );
    Ok(())
}

fn invalid_parameters() -> BidderError {
    BidderError::unknown("extractOrg: invalid rise parameters")
}

fn parameters(imp: &Imp) -> Result<Option<&Map<String, Value>>, BidderError> {
    let ext = imp.ext.as_ref().ok_or_else(invalid_parameters)?;
    // The canonical key is typed separately; null still competes with case variants.
    let mut bidder = match &ext.bidder {
        JsonField::Missing => JsonField::Missing,
        JsonField::Null => JsonField::Null,
        JsonField::Value(value) => JsonField::Value(value),
    };
    for (key, value) in &ext.other {
        if key.eq_ignore_ascii_case("bidder") {
            if !bidder.is_missing() {
                return Err(invalid_parameters());
            }
            bidder = JsonField::Value(value);
        }
    }
    match bidder {
        JsonField::Null | JsonField::Value(Value::Null) => Ok(None),
        JsonField::Value(Value::Object(fields)) => Ok(Some(fields)),
        JsonField::Missing | JsonField::Value(_) => Err(invalid_parameters()),
    }
}

fn parameter_string<'a>(
    fields: &'a Map<String, Value>,
    name: &str,
) -> Result<&'a str, BidderError> {
    let mut matching = fields.iter().filter(|(key, _value)| {
        // Only dotted capital I can lowercase from non-ASCII into these names.
        key.chars()
            .map(|character| match character {
                '\u{0130}' => 'i',
                _ => character.to_ascii_lowercase(),
            })
            .eq(name.chars())
    });
    let value = matching.next().map(|(_key, value)| value);
    if matching.next().is_some() {
        return Err(invalid_parameters());
    }
    match value {
        None | Some(Value::Null) => Ok(""),
        Some(field) => field.as_str().ok_or_else(invalid_parameters),
    }
}

fn extract_org(request: &BidRequest) -> Result<&str, BidderError> {
    for imp in &request.imp {
        let Some(fields) = parameters(imp)? else {
            continue;
        };
        let org = parameter_string(fields, "org")?;
        let publisher = parameter_string(fields, "publisher_id")?;
        let _placement = parameter_string(fields, "placementid")?;
        if !org.is_empty() {
            return Ok(org.trim());
        }
        if !publisher.is_empty() {
            return Ok(publisher.trim());
        }
    }
    Err(BidderError::unknown(
        "extractOrg: no org or publisher_id supplied",
    ))
}

#[cfg(test)]
mod tests {
    use super::super::RiseBidder;
    use crate::bidders::harness;
    use crate::{
        BidderError, BidderName, BidderRequestInfo, EndpointRejectReason, EndpointValidator,
        EntryPoint, OutboundBuildError, SinkLimits,
    };
    use auctioneer_openrtb::types::{bid_request::BidRequest, field::JsonField};
    use serde_json::{Value, json};
    use url::Url;

    struct EndpointPolicy(bool);

    impl EndpointValidator for EndpointPolicy {
        fn validate(
            &self,
            _url: &Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if self.0 {
                Ok(())
            } else {
                Err(EndpointRejectReason::HostNotAllowed)
            }
        }
    }

    fn info(endpoint: &str) -> BidderRequestInfo<'_> {
        BidderRequestInfo::new(
            BidderName::new("rise-alias"),
            BidderName::new("rise"),
            "account",
            None,
            endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    fn request(impressions: Value) -> BidRequest {
        let mut document = json!({
            "id":"request",
            "site":{"domain":"publisher.example"},
            "user":{"keywords":"already scoped"},
            "regs":{"ext":{"gdpr":1_i32}},
            "ext":{"preserved":{"flag":true}},
        });
        document["imp"] = impressions;
        serde_json::from_value(document).unwrap()
    }

    fn minimal() -> BidRequest {
        request(json!([{"id":"i", "banner":{}, "ext":{"bidder":{"org":"org"}}}]))
    }

    fn assert_selected(impressions: Value, selected: &str) {
        let request = request(impressions);
        let before = serde_json::to_value(&request).unwrap();
        let calls = harness::drive_ok(
            &RiseBidder,
            &request,
            &info("https://bidder.example.test/pbs"),
        );
        assert_eq!(calls.len(), 1);
        let url = Url::parse(calls[0].url()).unwrap();
        assert_eq!(url.path(), "/pbs");
        assert_eq!(
            url.query_pairs().into_owned().collect::<Vec<_>>(),
            [("publisher_id".to_owned(), selected.to_owned())]
        );
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
            before
        );
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
        assert_eq!(
            calls[0].covered_imp_ids().collect::<Vec<_>>(),
            request
                .imp
                .iter()
                .map(|imp| imp.id.as_str())
                .collect::<Vec<_>>()
        );
    }

    fn assert_invalid_parameters(parameters: Value) {
        let mut impressions = json!([
            {"id":"bad", "banner":{}, "ext":{"bidder":null}},
            {"id":"good", "banner":{}, "ext":{"bidder":{"org":"valid"}}},
        ]);
        impressions[0]["ext"]["bidder"] = parameters;
        let request = request(impressions);
        let before = serde_json::to_value(&request).unwrap();
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &RiseBidder,
            &request,
            &info("https://bidder.example.test/pbs"),
            SinkLimits::new(4096, 2),
            1,
            &EndpointPolicy(true),
        );
        assert_eq!(
            semantic.unwrap_err(),
            [BidderError::unknown("extractOrg: invalid rise parameters")]
        );
        assert!(outcome.accepted.is_empty());
        assert!(outcome.errors.is_empty());
        assert!(outcome.diagnostics.iter().next().is_none());
        assert_eq!(remaining, 1);
        assert_eq!(serde_json::to_value(&request).unwrap(), before);
    }

    #[test]
    fn first_identifier_wins_in_request_order_with_org_preferred_within_a_row() {
        for (first, second, selected) in [
            (
                json!({"org":"early-org", "publisher_id":"early-publisher"}),
                json!({"org":"late-org"}),
                "early-org",
            ),
            (
                json!({"publisher_id":"early-publisher"}),
                json!({"org":"late-org"}),
                "early-publisher",
            ),
            (
                json!({"org":"", "publisher_id":"early-publisher"}),
                json!({"org":"late-org"}),
                "early-publisher",
            ),
            (json!({}), json!({"org":"late-org"}), "late-org"),
            (
                json!({"org":null, "publisher_id":null, "placementId":null}),
                json!({"publisher_id":"late-publisher"}),
                "late-publisher",
            ),
        ] {
            assert_selected(
                json!([
                    {"id":"first", "banner":{}, "ext":{"bidder":first}},
                    {"id":"second", "video":{}, "ext":{"bidder":second}},
                ]),
                selected,
            );
        }
    }

    #[test]
    fn trimming_occurs_after_selection_and_an_empty_trimmed_value_still_wins() {
        for (first, selected) in [
            (json!({"org":" \torg\r\n"}), "org"),
            (json!({"publisher_id":" \tpublisher\r\n"}), "publisher"),
            (json!({"org":" \t", "publisher_id":"ignored"}), ""),
            (json!({"publisher_id":" \t"}), ""),
            (json!({"org":"\u{0085}\u{00a0}org\u{2003}\u{3000}"}), "org"),
        ] {
            assert_selected(
                json!([
                    {"id":"first", "banner":{}, "ext":{"bidder":first}},
                    {"id":"second", "banner":{}, "ext":{"bidder":{"org":"later"}}},
                ]),
                selected,
            );
        }
    }

    #[test]
    fn rows_after_selection_are_not_decoded_but_are_sent_without_filtering() {
        for trailing in [
            json!(false),
            json!([]),
            json!({"org":7_i32}),
            json!({"placementId":false}),
            json!({"Org":"one", "ORG":"two"}),
        ] {
            assert_selected(
                json!([
                    {"id":"selected", "banner":{}, "ext":{"bidder":{"org":"first"}}},
                    {"id":"uninspected", "audio":{}, "ext":{"bidder":trailing}},
                    {"id":"missing"},
                ]),
                "first",
            );
        }
    }

    #[test]
    fn every_recognized_parameter_is_decoded_before_selecting_from_its_row() {
        for key in ["org", "publisher_id", "placementId"] {
            for malformed in [json!(true), json!(7_i32), json!([]), json!({})] {
                let mut params = json!({"org":"selected", "publisher_id":"fallback"});
                params[key] = malformed;
                assert_invalid_parameters(params);
            }
        }
        for malformed in [
            json!(false),
            json!(7_i32),
            json!("private marker"),
            json!([]),
        ] {
            assert_invalid_parameters(malformed);
        }
    }

    #[test]
    fn absent_identifiers_fail_without_allocating_a_call_or_inventing_a_fallback() {
        for impressions in [
            json!([]),
            json!([{"id":"i", "banner":{},
            "ext":{"bidder":{"org":"", "publisher_id":"", "placementId":"not-an-id"}}}]),
        ] {
            let (semantic, calls) = harness::drive(
                &RiseBidder,
                &request(impressions),
                &info("https://bidder.example.test/pbs"),
            );
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown(
                    "extractOrg: no org or publisher_id supplied"
                )]
            );
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn unavailable_bidder_parameters_before_selection_are_bidder_fatal() {
        for first in [
            json!({"id":"bad", "banner":{}}),
            json!({"id":"bad", "banner":{}, "ext":{}}),
        ] {
            let (semantic, calls) = harness::drive(
                &RiseBidder,
                &request(json!([
                    first, {"id":"good", "banner":{}, "ext":{"bidder":{"org":"valid"}}},
                ])),
                &info("https://bidder.example.test/pbs"),
            );
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown("extractOrg: invalid rise parameters")]
            );
            assert!(calls.is_empty());
        }
    }

    #[test]
    fn an_explicit_null_parameter_object_allows_scanning_to_continue() {
        let mut request = request(json!([
            {"id":"null", "banner":{}, "ext":{"bidder":{}}},
            {"id":"valid", "banner":{}, "ext":{"bidder":{"org":"later"}}},
        ]));
        request.imp[0].ext.as_mut().unwrap().bidder = JsonField::Null;
        let calls = harness::drive_ok(
            &RiseBidder,
            &request,
            &info("https://bidder.example.test/pbs"),
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].url(),
            "https://bidder.example.test/pbs?publisher_id=later"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(calls[0].body()).unwrap()["imp"][0]["ext"]["bidder"],
            Value::Null
        );
        assert_selected(
            json!([
                {"id":"null", "banner":{}, "ext":{"bidder":null}},
                {"id":"valid", "banner":{}, "ext":{"bidder":{"org":"later"}}},
            ]),
            "later",
        );
    }

    #[test]
    fn org_accepts_one_case_equivalent_key_and_rejects_each_malformed_spelling() {
        for key in ["org", "Org", "ORG"] {
            assert_selected(
                json!([{"id":"i", "banner":{}, "ext":{"bidder":{key:"selected"}}}]),
                "selected",
            );
            assert_invalid_parameters(json!({key:false}));
        }
    }

    #[test]
    fn publisher_id_uses_general_unicode_lowercase_matching() {
        for key in [
            "publisher_id",
            "Publisher_ID",
            "PUBLISHER_ID",
            "publ\u{0130}sher_id",
        ] {
            assert_selected(
                json!([{"id":"i", "banner":{}, "ext":{"bidder":{key:"selected"}}}]),
                "selected",
            );
            assert_invalid_parameters(json!({key:false}));
        }
    }

    #[test]
    fn unused_placement_id_still_uses_general_unicode_matching_and_string_decoding() {
        for key in [
            "placementId",
            "PlacementID",
            "PLACEMENTID",
            "placement\u{0130}d",
        ] {
            assert_selected(
                json!([{"id":"i", "banner":{}, "ext":{"bidder":{"org":"selected", key:"not-a-route"}}}]),
                "selected",
            );
            assert_invalid_parameters(json!({"org":"selected", key:false}));
            assert_selected(
                json!([{"id":"i", "banner":{}, "ext":{"bidder":{"org":"selected", key:null}}}]),
                "selected",
            );
        }
    }

    #[test]
    fn competing_parameter_spellings_are_fatal_even_without_canonical_or_with_equal_or_null_values()
    {
        for spellings in [
            ["org", "Org", "ORG"],
            ["publisher_id", "Publisher_ID", "PUBLISHER_ID"],
            ["placementId", "PlacementId", "PLACEMENTID"],
        ] {
            for (left, right) in [(spellings[0], spellings[1]), (spellings[1], spellings[2])] {
                for competitor in [json!("same"), json!("different"), Value::Null] {
                    let mut params = json!({left:"same", right:competitor});
                    if spellings[0] == "placementId" {
                        params["org"] = json!("selected");
                    }
                    assert_invalid_parameters(params);
                }
                assert_invalid_parameters(json!({left:null, right:null, "unrelated":true}));
            }
        }
        assert_invalid_parameters(json!({"Publisher_ID":"same", "publ\u{0130}sher_id":"same"}));
        assert_invalid_parameters(
            json!({"org":"selected", "PlacementId":"same", "placement\u{0130}d":null}),
        );
    }

    #[test]
    fn bidder_container_keys_use_ascii_matching_not_the_parameter_unicode_rule() {
        for key in ["bidder", "Bidder", "BIDDER"] {
            assert_selected(
                json!([{"id":"i", "banner":{}, "ext":{key:{"org":"selected"}}}]),
                "selected",
            );
            let (semantic, calls) = harness::drive(
                &RiseBidder,
                &request(json!([{"id":"i", "banner":{}, "ext":{key:false}}])),
                &info("https://bidder.example.test/pbs"),
            );
            assert_eq!(
                semantic.unwrap_err(),
                [BidderError::unknown("extractOrg: invalid rise parameters")]
            );
            assert!(calls.is_empty());
        }
        let (semantic, calls) = harness::drive(
            &RiseBidder,
            &request(
                json!([{"id":"i", "banner":{}, "ext":{"b\u{0130}dder":{"org":"not-recognized"}}}]),
            ),
            &info("https://bidder.example.test/pbs"),
        );
        assert_eq!(
            semantic.unwrap_err(),
            [BidderError::unknown("extractOrg: invalid rise parameters")]
        );
        assert!(calls.is_empty());
    }

    #[test]
    fn competing_bidder_containers_are_fatal_before_identifier_selection() {
        for (left, right) in [("bidder", "Bidder"), ("Bidder", "BIDDER")] {
            for competitor in [
                json!({"org":"same"}),
                json!({"org":"different"}),
                Value::Null,
            ] {
                let (semantic, calls) = harness::drive(
                    &RiseBidder,
                    &request(
                        json!([{"id":"i", "banner":{}, "ext":{left:{"org":"same"}, right:competitor}}]),
                    ),
                    &info("https://bidder.example.test/pbs"),
                );
                assert_eq!(
                    semantic.unwrap_err(),
                    [BidderError::unknown("extractOrg: invalid rise parameters")]
                );
                assert!(calls.is_empty());
            }
            for ext in [
                json!({left:null, right:{"org":"same"}}),
                json!({left:null, right:null}),
            ] {
                let (semantic, calls) = harness::drive(
                    &RiseBidder,
                    &request(json!([{"id":"i", "banner":{}, "ext":ext}])),
                    &info("https://bidder.example.test/pbs"),
                );
                assert_eq!(
                    semantic.unwrap_err(),
                    [BidderError::unknown("extractOrg: invalid rise parameters")]
                );
                assert!(calls.is_empty());
            }
        }
    }

    #[test]
    fn one_post_preserves_the_scoped_request_all_ids_and_only_the_source_header() {
        for count in [1_usize, 8, 100] {
            let impressions: Vec<_> = (0..count).map(|index| json!({
                "id":format!("imp-{index}"), "banner":{}, "ext":{"bidder":{"org":"same", "path":"ignored", "zone":"ignored"}},
            })).collect();
            let request = request(json!(impressions));
            let before = serde_json::to_value(&request).unwrap();
            let calls = harness::drive_ok(
                &RiseBidder,
                &request,
                &info("https://bidder.example.test/pbs"),
            );
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].method(), "POST");
            assert_eq!(
                calls[0].url(),
                "https://bidder.example.test/pbs?publisher_id=same"
            );
            assert_eq!(
                calls[0].headers().collect::<Vec<_>>(),
                [("Content-Type", "application/json;charset=utf-8")]
            );
            assert_eq!(
                calls[0].covered_imp_ids().collect::<Vec<_>>(),
                request
                    .imp
                    .iter()
                    .map(|imp| imp.id.as_str())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                serde_json::from_slice::<Value>(calls[0].body()).unwrap(),
                before
            );
            assert_eq!(serde_json::to_value(&request).unwrap(), before);
        }
    }

    #[test]
    fn structured_publisher_query_preserves_authority_path_existing_query_and_macros() {
        for (endpoint, expected) in [
            (
                "https://bidder.example.test/pbs",
                "https://bidder.example.test/pbs?publisher_id=a+b%26x%3D1%23%2F%25%2B",
            ),
            (
                "https://bidder.example.test/pbs?fixed=1",
                "https://bidder.example.test/pbs?fixed=1&publisher_id=a+b%26x%3D1%23%2F%25%2B",
            ),
            (
                "https://bidder.example.test/pbs?",
                "https://bidder.example.test/pbs?publisher_id=a+b%26x%3D1%23%2F%25%2B",
            ),
            (
                "https://bidder.example.test/pbs?fixed=1&",
                "https://bidder.example.test/pbs?fixed=1&publisher_id=a+b%26x%3D1%23%2F%25%2B",
            ),
            (
                "https://bidder.example.test/{{.AccountID}}/pbs?pub={{.PublisherID}}",
                "https://bidder.example.test/acct%2F42/pbs?pub=pub%26id&publisher_id=a+b%26x%3D1%23%2F%25%2B",
            ),
        ] {
            let mut info = info(endpoint);
            info.account_id = "acct/42";
            info.publisher_id = Some("pub&id");
            info.endpoint_macros = if endpoint.contains("{{.") {
                &["AccountID", "PublisherID"]
            } else {
                &[]
            };
            let request = request(
                json!([{"id":"i", "banner":{}, "ext":{"bidder":{"org":" a b&x=1#/%+ ", "path":"/not-a-route"}}}]),
            );
            let calls = harness::drive_ok(&RiseBidder, &request, &info);
            assert_eq!(calls.len(), 1);
            assert_eq!(calls[0].url(), expected);
            assert_eq!(
                Url::parse(calls[0].url()).unwrap().host_str(),
                Some("bidder.example.test")
            );
        }
    }

    #[test]
    fn raw_body_impression_call_and_endpoint_limits_bind_independently_with_refunds() {
        let request = minimal();
        let calls = harness::drive_ok(
            &RiseBidder,
            &request,
            &info("https://bidder.example.test/pbs"),
        );
        assert_eq!(calls.len(), 1);
        let bytes = calls[0].body().len();
        for (raw, ids, slots, allow, error, remaining) in [
            (bytes, 1_u16, 1_u32, true, None, 0_u32),
            (
                bytes - 1,
                1,
                1,
                true,
                Some(OutboundBuildError::OversizeRaw {
                    limit: u64::try_from(bytes - 1).unwrap(),
                }),
                1,
            ),
            (
                bytes,
                0,
                1,
                true,
                Some(OutboundBuildError::TooManyImpIds { limit: 0 }),
                1,
            ),
            (
                bytes,
                1,
                0,
                true,
                Some(OutboundBuildError::CallBudgetExhausted { max_calls: 0 }),
                0,
            ),
            (
                bytes,
                1,
                1,
                false,
                Some(OutboundBuildError::EndpointRejected {
                    reason: EndpointRejectReason::HostNotAllowed,
                }),
                1,
            ),
        ] {
            let (semantic, outcome, actual_remaining) = harness::drive_with_limits(
                &RiseBidder,
                &request,
                &info("https://bidder.example.test/pbs"),
                SinkLimits::new(raw, ids),
                slots,
                &EndpointPolicy(allow),
            );
            semantic.unwrap();
            assert_eq!(outcome.accepted.len(), usize::from(error.is_none()));
            assert_eq!(outcome.errors, error.into_iter().collect::<Vec<_>>());
            assert_eq!(actual_remaining, remaining);
        }
    }

    #[test]
    fn oversized_publisher_query_is_refused_by_metadata_cap_without_spending_a_call() {
        let request = request(
            json!([{"id":"i", "banner":{}, "ext":{"bidder":{"org":"&".repeat(crate::META_BIDDER_REGION_BYTES)}}}]),
        );
        let (semantic, outcome, remaining) = harness::drive_with_limits(
            &RiseBidder,
            &request,
            &info("https://bidder.example.test/pbs"),
            SinkLimits::new(64 * 1024, 1),
            1,
            &EndpointPolicy(true),
        );
        semantic.unwrap();
        assert!(outcome.accepted.is_empty());
        assert_eq!(
            outcome.errors,
            [OutboundBuildError::MetaOverCap {
                limit: u64::try_from(crate::META_BIDDER_REGION_BYTES).unwrap()
            }]
        );
        assert_eq!(remaining, 1);
    }
}
