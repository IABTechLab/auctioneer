//! Shared bounded bidder metadata construction, called after reservation.

use crate::{
    BidderRequestInfo, EndpointBuilder, META_BIDDER_REGION_BYTES, OutboundRequestMetaBuilder,
    SinkAbort,
};
use auctioneer_openrtb::types::bid_request::BidRequest;

#[cfg(test)]
use crate::EndpointBuilderError;

/// Keep typed endpoint-policy errors available to the colocated tests.
#[cfg(test)]
fn resolve_outbound_url(
    info: &BidderRequestInfo<'_>,
    host: Option<&str>,
) -> Result<String, EndpointBuilderError> {
    let values = [
        ("AccountID", info.account_id),
        ("PublisherID", info.publisher_id.unwrap_or("")),
    ];
    let count = if info.publisher_id.is_some() { 2 } else { 1 };
    let provided_values = values.get(..count).unwrap_or_default();
    let mut builder = EndpointBuilder::from_request(info, provided_values);
    if let Some(selected_host) = host {
        builder.select_host(selected_host)?;
    }
    builder.build()
}

/// Borrow the two supported request macro values without synthesizing bidder info.
pub(super) fn write_endpoint(
    meta: &mut OutboundRequestMetaBuilder<'_>,
    info: &BidderRequestInfo<'_>,
) -> Result<(), SinkAbort> {
    let mut values = [("", ""); 2];
    let mut count = 0_usize;
    for name in info.endpoint_macros {
        let optional_value = match *name {
            "AccountID" => Some(info.account_id),
            "PublisherID" => info.publisher_id,
            _ => None,
        };
        if let Some(value) = optional_value {
            let slot = values.get_mut(count).ok_or(SinkAbort::MetaOverCap {
                limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
            })?;
            *slot = (name, value);
            count = count.saturating_add(1);
        }
    }
    EndpointBuilder::from_request(info, values.get(..count).unwrap_or_default()).expand_into(meta)
}

pub(super) fn json_request_meta(
    meta: &mut OutboundRequestMetaBuilder<'_>,
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
) -> Result<(), SinkAbort> {
    meta.set_method("POST")?;
    write_endpoint(meta, info)?;
    json_headers(meta)?;
    for imp in &request.imp {
        meta.imp_id(&imp.id)?;
    }
    Ok(())
}

/// Exact header pair for protocols that declare JSON without a charset.
pub(super) fn json_headers(meta: &mut OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort> {
    meta.header("Content-Type", "application/json")?;
    meta.header("Accept", "application/json")
}

/// Exact header pair for protocols that declare the UTF-8 charset.
pub(super) fn json_utf8_headers(
    meta: &mut OutboundRequestMetaBuilder<'_>,
) -> Result<(), SinkAbort> {
    meta.header("Content-Type", "application/json;charset=utf-8")?;
    meta.header("Accept", "application/json")
}

#[cfg(test)]
mod tests {
    use super::{json_headers, json_request_meta, json_utf8_headers, resolve_outbound_url};
    use crate::correlation::CorrelationError;
    use crate::meta::MAX_BIDDER_HEADERS;
    use crate::{META_BYTES_CAPACITY, OutboundRequestMetaBuilder, SinkAbort};
    use crate::{bidder::EntryPoint, request_info::BidderRequestInfo, types::BidderName};
    use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};

    fn info_with(
        template: &'static str,
        allowlist: Option<&'static [&'static str]>,
        macros: &'static [&'static str],
    ) -> BidderRequestInfo<'static> {
        BidderRequestInfo::new(
            BidderName::new("fake_bidder"),
            BidderName::new("fake_bidder"),
            "acct-42",
            None,
            template,
            macros,
            allowlist,
            None,
            EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn host_varying_helper_substitutes_and_gates_on_allowlist() {
        let allow: &[&str] = &["east.example.test", "west.example.test"];
        let info = info_with(
            "https://{{.Host}}/openrtb2/{{.AccountID}}",
            Some(allow),
            &["AccountID"],
        );
        let url = resolve_outbound_url(&info, Some("east.example.test")).unwrap();
        assert_eq!(url, "https://east.example.test/openrtb2/acct-42");
    }

    #[test]
    fn host_varying_helper_rejects_out_of_allowlist_host() {
        let allow: &[&str] = &["east.example.test"];
        let info = info_with("https://{{.Host}}/rtb", Some(allow), &[]);
        let err = resolve_outbound_url(&info, Some("attacker.example.test")).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("HostNotInAllowlist"),
            "expected HostNotInAllowlist SSRF guard, got {msg}",
        );
    }

    #[test]
    fn host_varying_helper_rejects_when_bidder_is_fixed_host() {
        // No host_allowlist declared → the bidder is fixed-host and
        // cannot accept a host.
        let info = info_with("https://fixed.example.test/rtb", None, &[]);
        let err = resolve_outbound_url(&info, Some("east.example.test")).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("UnknownMacro"),
            "expected UnknownMacro(\"Host\") for a fixed-host bidder, got {msg}",
        );
    }

    #[test]
    fn resolve_outbound_url_returns_missing_macro_for_host_template_without_selection() {
        // Fixed-host helper against a `{{.Host}}` template: the template
        // still references Host, but no host was selected — build()
        // returns MissingMacro("Host").
        let allow: &[&str] = &["east.example.test"];
        let info = info_with("https://{{.Host}}/rtb", Some(allow), &[]);
        let err = resolve_outbound_url(&info, None).unwrap_err();
        let msg = format!("{err:?}");
        assert!(
            msg.contains("MissingMacro") && msg.contains("Host"),
            "expected MissingMacro(\"Host\") when template needs a host but none is selected, \
             got {msg}",
        );
    }

    #[test]
    fn json_headers_preserve_exact_values_and_order_without_adding_metadata() {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
        json_headers(&mut builder).unwrap();
        let meta = builder.finish();
        assert_eq!(meta.method(), "");
        assert_eq!(meta.url(), "");
        assert_eq!(meta.imp_id_count(), 0);
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json"),
                ("Accept", "application/json")
            ]
        );
    }

    #[test]
    fn json_utf8_headers_preserve_exact_values_and_order_without_adding_metadata() {
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
        json_utf8_headers(&mut builder).unwrap();
        let meta = builder.finish();
        assert_eq!(meta.method(), "");
        assert_eq!(meta.url(), "");
        assert_eq!(meta.imp_id_count(), 0);
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json;charset=utf-8"),
                ("Accept", "application/json"),
            ]
        );
    }

    #[test]
    fn json_header_pairs_propagate_first_and_second_header_refusals_in_order() {
        for utf8 in [false, true] {
            for remaining in [0, 1] {
                let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
                let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 1);
                for _ in 0..(MAX_BIDDER_HEADERS - remaining) {
                    builder.header("X", "v").unwrap();
                }
                let result = if utf8 {
                    json_utf8_headers(&mut builder)
                } else {
                    json_headers(&mut builder)
                };
                assert_eq!(
                    result,
                    Err(SinkAbort::TooManyHeaders {
                        limit: MAX_BIDDER_HEADERS
                    })
                );
                let meta = builder.finish();
                let headers = meta.headers().collect::<Vec<_>>();
                assert_eq!(headers.len(), usize::from(MAX_BIDDER_HEADERS));
                let expected_last = if remaining == 0 {
                    ("X", "v")
                } else if utf8 {
                    ("Content-Type", "application/json;charset=utf-8")
                } else {
                    ("Content-Type", "application/json")
                };
                assert_eq!(headers.last(), Some(&expected_last));
            }
        }
    }

    #[test]
    fn json_request_metadata_preserves_endpoint_headers_and_ordered_unique_coverage() {
        let info = info_with(
            "https://fixed.example.test/rtb?account={{.AccountID}}",
            None,
            &["AccountID"],
        );
        let request = BidRequest {
            imp: ["second", "first"]
                .map(|id| Imp {
                    id: id.into(),
                    ..Imp::default()
                })
                .into(),
            ..BidRequest::default()
        };
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 2);
        json_request_meta(&mut builder, &request, &info).unwrap();
        let meta = builder.finish();
        assert_eq!(meta.method(), "POST");
        assert_eq!(meta.url(), "https://fixed.example.test/rtb?account=acct-42");
        assert_eq!(
            meta.headers().collect::<Vec<_>>(),
            [
                ("Content-Type", "application/json"),
                ("Accept", "application/json")
            ]
        );
        assert_eq!(
            meta.covered_imp_ids().collect::<Vec<_>>(),
            ["second", "first"]
        );
    }

    #[test]
    fn json_request_metadata_refuses_duplicate_coverage_without_retaining_the_duplicate() {
        let info = info_with("https://fixed.example.test/rtb", None, &[]);
        let request = BidRequest {
            imp: ["second", "first", "second"]
                .map(|id| Imp {
                    id: id.into(),
                    ..Imp::default()
                })
                .into(),
            ..BidRequest::default()
        };
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut builder = OutboundRequestMetaBuilder::new(&mut pool, 3);
        assert_eq!(
            json_request_meta(&mut builder, &request, &info),
            Err(SinkAbort::Correlation(CorrelationError::DuplicateCoverage))
        );
        let meta = builder.finish();
        assert_eq!(
            meta.covered_imp_ids().collect::<Vec<_>>(),
            ["second", "first"]
        );
    }
}
