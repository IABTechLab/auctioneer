//! Bounded request construction and borrowed wire projection for the shared fake.

use super::{FakeBidder, IdentityEnvelope, QueryMode, RequestMode};
use crate::correlation::{CorrelationError, CorrelationMedia};
use crate::{
    BidderError, BidderRequestInfo, EndpointBuilder, OutboundRequestMetaBuilder,
    OutboundRequestSink, SinkAbort,
};
use auctioneer_openrtb::types::bid_request::{BidRequest, Imp};
use serde::{
    Serialize,
    ser::{SerializeMap as _, SerializeSeq as _},
};
use std::sync::atomic::Ordering;

pub(super) fn credential_key<'a>(
    bidder: &'a FakeBidder<'_>,
    request: &BidRequest,
    info: &'a BidderRequestInfo<'_>,
) -> Result<Option<&'a str>, BidderError> {
    if let Some(observer) = bidder.credential_preflight_observer {
        observer(request, info);
    }
    if bidder.credential_reference.is_some() && bidder.semantic_error {
        return Err(BidderError::bad_input("fake parameter refusal"));
    }
    Ok(bidder.credential_reference)
}

fn metadata(
    bidder: &FakeBidder<'_>,
    info: &BidderRequestInfo<'_>,
    impressions: &Impressions<'_>,
    observation: Option<(&IdentityEnvelope<'_>, usize)>,
    meta: &mut OutboundRequestMetaBuilder<'_>,
) -> Result<(), SinkAbort> {
    bidder.metadata_writes.fetch_add(1, Ordering::Relaxed);
    meta.set_method(bidder.method)?;
    let mut endpoint = EndpointBuilder::from_request(info, bidder.endpoint_values);
    if let Some(host) = bidder.selected_host {
        endpoint
            .select_host(host)
            .map_err(|_error| SinkAbort::MetaOverCap {
                limit: u64::try_from(crate::META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
            })?;
    }
    match bidder.query {
        QueryMode::Preserve => endpoint.expand_into(meta)?,
        QueryMode::Append(pairs) => endpoint.expand_with_query_into(meta, pairs)?,
        QueryMode::Set { key, value } => {
            endpoint.expand_with_query_set_into(meta, key, value)?;
        }
    }
    meta.header("Content-Type", "application/json")?;
    meta.header("Accept", "application/json")?;
    for &(name, value) in bidder.headers {
        meta.header(name, value)?;
    }
    if let Some((identity, execution)) = observation {
        meta.header("X-Discovery-Module", identity.module)?;
        meta.header(
            "X-Discovery-Constructors",
            &identity.constructor_count.to_string(),
        )?;
        meta.header("X-Discovery-Executions", &execution.to_string())?;
    }
    for imp in impressions.selected() {
        meta.imp_id(&imp.id)?;
    }
    if let Some(header) = bidder.correlation_header {
        meta.set_correlation_header(header)?;
    }
    if bidder.record_correlation {
        for (index, imp) in impressions.selected().enumerate() {
            let ordinal =
                u16::try_from(index).map_err(|_overflow| CorrelationError::UnknownOrdinal)?;
            let token = u32::from(ordinal)
                .checked_add(1)
                .ok_or(CorrelationError::UnknownOrdinal)?;
            meta.correlate_imp(ordinal, unique_media(imp)?, token)?;
        }
    }
    Ok(())
}

pub(super) fn make_requests(
    bidder: &FakeBidder<'_>,
    request: &BidRequest,
    info: &BidderRequestInfo<'_>,
    sink: &mut OutboundRequestSink<'_>,
) -> Result<(), Vec<BidderError>> {
    if let Some(observer) = bidder.construction_observer {
        observer(info);
    }
    if let Some(observer) = bidder.request_observer {
        observer(request, info);
    }
    if bidder.semantic_error {
        return Err(vec![BidderError::bad_input("fake semantic refusal")]);
    }
    if let Some(diagnostic) = bidder.rejected_imp_diagnostic {
        for _rejected in request
            .imp
            .iter()
            .filter(|imp| bidder.reject_imp == Some(imp.id.as_str()))
        {
            sink.diagnostic(diagnostic);
        }
    }
    let observation = bidder.identity_envelope.as_ref().map(|identity| {
        (
            identity,
            identity
                .executions
                .fetch_add(1, Ordering::SeqCst)
                .saturating_add(1),
        )
    });
    if let Some(calls) = &bidder.request_coverage {
        for ids in calls {
            let _recorded = sink.push_request(
                info,
                |meta| {
                    bidder.metadata_writes.fetch_add(1, Ordering::Relaxed);
                    meta.set_method(bidder.method)?;
                    meta.set_url(info.endpoint_template)?;
                    for id in ids {
                        meta.imp_id(id)?;
                    }
                    Ok(())
                },
                |writer| {
                    bidder.body_writes.fetch_add(1, Ordering::Relaxed);
                    writer.write_json(request)
                },
            );
        }
        return Ok(());
    }
    let chunk_size = match bidder.request_mode {
        RequestMode::Aggregate => request.imp.len().max(1),
        RequestMode::Split => 1,
        RequestMode::Chunked(size) => size.get(),
    };
    for chunk in request.imp.chunks(chunk_size) {
        let impressions = Impressions {
            items: chunk,
            reject: bidder.reject_imp,
        };
        if impressions.selected().next().is_none() {
            continue;
        }
        let _recorded = sink.push_request(
            info,
            |meta| metadata(bidder, info, &impressions, observation, meta),
            |writer| {
                bidder.body_writes.fetch_add(1, Ordering::Relaxed);
                if let Some(identity) = &bidder.identity_envelope {
                    writer.write_json(&ProtocolEnvelope {
                        marker: identity.marker,
                        public: info.bidder.as_str(),
                        core: info.core_name.as_str(),
                        request_id: &request.id,
                    })
                } else {
                    writer.write_json(&Request {
                        source: request,
                        impressions: &impressions,
                    })
                }
            },
        );
    }
    Ok(())
}

#[derive(Serialize)]
struct ProtocolEnvelope<'a> {
    marker: &'a str,
    public: &'a str,
    core: &'a str,
    request_id: &'a str,
}

fn unique_media(imp: &Imp) -> Result<CorrelationMedia, CorrelationError> {
    let mut offered = [
        (CorrelationMedia::Banner, imp.banner.is_some()),
        (CorrelationMedia::Video, imp.video.is_some()),
        (CorrelationMedia::Audio, imp.audio.is_some()),
        (CorrelationMedia::Native, imp.native.is_some()),
    ]
    .into_iter()
    .filter_map(|(media, present)| present.then_some(media));
    match (offered.next(), offered.next()) {
        (Some(media), None) => Ok(media),
        _ => Err(CorrelationError::InvalidMedia),
    }
}

struct Impressions<'a> {
    items: &'a [Imp],
    reject: Option<&'a str>,
}

impl Impressions<'_> {
    fn selected(&self) -> impl Iterator<Item = &Imp> {
        self.items
            .iter()
            .filter(|imp| self.reject != Some(imp.id.as_str()))
    }
}

impl Serialize for Impressions<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        for imp in self.selected() {
            sequence.serialize_element(imp)?;
        }
        sequence.end()
    }
}

struct Request<'a> {
    source: &'a BidRequest,
    impressions: &'a Impressions<'a>,
}

impl Serialize for Request<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Keep this exhaustive so a new OpenRTB field cannot silently disappear
        // from the fake's split/chunked borrowed serialization.
        let BidRequest {
            id,
            imp: _original_impressions,
            site,
            app,
            dooh,
            device,
            user,
            regs,
            source,
            at,
            tmax,
            cur,
            test,
            ext,
            other,
        } = self.source;
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("id", id)?;
        map.serialize_entry("imp", self.impressions)?;
        if let Some(value) = site {
            map.serialize_entry("site", value)?;
        }
        if let Some(value) = app {
            map.serialize_entry("app", value)?;
        }
        if let Some(value) = dooh {
            map.serialize_entry("dooh", value)?;
        }
        if let Some(value) = device {
            map.serialize_entry("device", value)?;
        }
        if let Some(value) = user {
            map.serialize_entry("user", value)?;
        }
        if let Some(value) = regs {
            map.serialize_entry("regs", value)?;
        }
        if let Some(value) = source {
            map.serialize_entry("source", value)?;
        }
        if let Some(value) = at {
            map.serialize_entry("at", value)?;
        }
        if let Some(value) = tmax {
            map.serialize_entry("tmax", value)?;
        }
        if !cur.is_empty() {
            map.serialize_entry("cur", cur)?;
        }
        if let Some(value) = test {
            map.serialize_entry("test", value)?;
        }
        if let Some(value) = ext {
            map.serialize_entry("ext", value)?;
        }
        for (key, value) in other {
            map.serialize_entry(key, value)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::{CorrelationError, CorrelationMedia, Imp, unique_media};
    use serde_json::json;

    #[test]
    fn correlation_requires_exactly_one_supported_impression_medium() {
        for mask in 0_u8..16 {
            let mut input = json!({"id": "fake_imp"});
            for (bit, field) in [(1_u8, "banner"), (2, "video"), (4, "audio"), (8, "native")] {
                if mask & bit != 0 {
                    input[field] = json!({});
                }
            }
            let imp: Imp = serde_json::from_value(input).unwrap();
            let expected = match mask {
                1 => Ok(CorrelationMedia::Banner),
                2 => Ok(CorrelationMedia::Video),
                4 => Ok(CorrelationMedia::Audio),
                8 => Ok(CorrelationMedia::Native),
                _ => Err(CorrelationError::InvalidMedia),
            };
            assert_eq!(unique_media(&imp), expected, "media presence mask {mask}");
        }
    }
}
