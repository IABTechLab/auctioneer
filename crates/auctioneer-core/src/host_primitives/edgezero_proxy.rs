//! `EdgeZeroProxyAdapter` — bridges `EdgeZero`'s outbound `HttpClient`
//! into our `ProxyClientApi` trait so a fanout can drive it. Plus
//! `EdgeZeroFanout`, the owned `BidderFanout` the handler builds
//! per-request from `ctx.http_client()`.

use crate::host_primitives::fanout::ProxyClientApi;
use crate::host_primitives::strategy::FanoutStrategy;
use crate::outbound::wave::{WaveHost, drive_fanout};
use crate::outbound::{BoundedFetch, seal_bidder_response};
use async_trait::async_trait;
use auctioneer_bidder::{BidderHttpRequest, BidderHttpResponse, ProxyError};
use auctioneer_exchange::{
    AuctionDeadline, BidderCall, BidderCallResult, BidderFanout, FanoutInternalError,
};
use auctioneer_runtime::{Clock, DecodeCooperator, ResolvedIsolateLimits, StreamGate, Timer};
use edgezero_core::body::Body;
use edgezero_core::http::{Method, Uri};
use edgezero_core::outbound::{HttpClient, OutboundCachePolicy, OutboundRequest};
use futures::future::join_all;
use std::sync::Arc;

/// Wraps `edgezero_core::outbound::HttpClient` behind the neutral
/// `ProxyClientApi` seam so the exchange side stays adapter-agnostic.
pub struct EdgeZeroProxyAdapter {
    client: HttpClient,
}

impl EdgeZeroProxyAdapter {
    #[must_use]
    pub fn new(client: HttpClient) -> Self {
        Self { client }
    }
}

#[async_trait(?Send)]
impl ProxyClientApi for EdgeZeroProxyAdapter {
    async fn send_one(&self, req: BidderHttpRequest) -> Result<BidderHttpResponse, ProxyError> {
        let (meta, body) = req.into_parts();
        let method: Method = meta
            .method()
            .parse()
            .map_err(|err| ProxyError::Transport(format!("invalid method: {err}")))?;
        let uri: Uri = meta
            .url()
            .parse()
            .map_err(|err| ProxyError::Transport(format!("invalid url: {err}")))?;
        // A bidder call carries auction-specific state and must never
        // be served from, or written to, a platform cache: a cached
        // bid response would be both wrong and a privacy leak across
        // auctions. `PlatformDefault` is the value an unset policy
        // inherits, so the bypass is set explicitly on every call.
        let mut outbound = OutboundRequest::new(method, uri)
            .map_err(|err| ProxyError::Transport(format!("invalid outbound target: {err}")))?
            .body(Body::from_bytes(body))
            .cache_policy(OutboundCachePolicy::Bypass);
        // `BidderHttpRequest.headers` preserves order and allows
        // duplicates (e.g. multiple `Set-Cookie`, comma-joined-vs-
        // repeated `Vary`); `OutboundRequest::header` appends. A header
        // the wire cannot carry fails the whole call: sending the
        // request with that header silently removed would be a
        // different request from the one the adapter built.
        for (name, value) in meta.headers() {
            outbound = outbound.header(name, value).map_err(|err| {
                ProxyError::Transport(format!("invalid outbound header {name:?}: {err}"))
            })?;
        }

        let response = self
            .client
            .send(outbound)
            .await
            .map_err(|err| ProxyError::Transport(format!("send failed: {err}")))?;

        let (_method, status, headers, response_body, content_type_provenance) =
            response.into_parts();
        let body_bytes = response_body
            .into_bytes_bounded(usize::MAX)
            .await
            .map_err(|err| ProxyError::Transport(format!("read response body: {err}")))?;
        Ok(seal_bidder_response(
            status.as_u16(),
            &headers,
            body_bytes.to_vec(),
            content_type_provenance,
        ))
    }
}

// Compile-time assertion — surfaces Send+Sync regressions at the
// adapter boundary rather than at downstream BidderFanout impls.
const _: fn() = || {
    fn assert_send_sync<T>()
    where
        T: Send + Sync,
    {
        // Body intentionally empty: instantiating the fn is the assertion.
    }
    assert_send_sync::<EdgeZeroProxyAdapter>();
};

/// Owned `BidderFanout` the handler builds per-request when
/// `ctx.http_client()` is present. Owns the per-request client adapter
/// plus the three boot-time host primitives, so it can live behind
/// `Arc<dyn BidderFanout>`.
pub struct EdgeZeroFanout {
    adapter: EdgeZeroProxyAdapter,
    clock: Arc<dyn Clock + Send + Sync>,
    timer: Arc<dyn Timer + Send + Sync>,
    strategy: Arc<dyn FanoutStrategy + Send + Sync>,
    /// The wave scheduler's own state, present only when this fanout was
    /// built for it. Keeping the two drives on ONE type is deliberate:
    /// the handler binds `Arc<dyn BidderFanout>` once, and the per-target
    /// migration off the strategy path happens per adapter, so a target
    /// mid-migration has to be constructible either way.
    wave: Option<WaveDrive>,
}

/// Everything the wave scheduler needs that the strategy path does not.
struct WaveDrive {
    fetch: BoundedFetch,
    gate: Arc<StreamGate>,
    /// The host yield point auctioneer's own decode uses. Lives here rather
    /// than beside the timer because only the wave path performs a
    /// decode of its own; the strategy path never calls it.
    cooperator: Arc<dyn DecodeCooperator + Send + Sync>,
}

impl EdgeZeroFanout {
    #[must_use]
    pub fn new(
        client: HttpClient,
        clock: Arc<dyn Clock + Send + Sync>,
        timer: Arc<dyn Timer + Send + Sync>,
        strategy: Arc<dyn FanoutStrategy + Send + Sync>,
    ) -> Self {
        Self {
            adapter: EdgeZeroProxyAdapter::new(client),
            clock,
            timer,
            strategy,
            wave: None,
        }
    }

    /// Builds a fanout that drives the completion-driven wave scheduler.
    ///
    /// The strategy is still taken, and still installed, because the
    /// per-adapter migration off the per-call race is owned per target: a
    /// target that has not moved yet constructs through [`Self::new`] and
    /// gets the strategy path, and one that has moves here without the
    /// handler's binding changing shape.
    #[must_use]
    pub fn with_wave_scheduler(
        client: HttpClient,
        clock: Arc<dyn Clock + Send + Sync>,
        timer: Arc<dyn Timer + Send + Sync>,
        cooperator: Arc<dyn DecodeCooperator + Send + Sync>,
        strategy: Arc<dyn FanoutStrategy + Send + Sync>,
        limits: ResolvedIsolateLimits,
        gate: Arc<StreamGate>,
    ) -> Self {
        Self {
            adapter: EdgeZeroProxyAdapter::new(HttpClient::clone(&client)),
            clock,
            timer,
            strategy,
            wave: Some(WaveDrive {
                fetch: BoundedFetch::new(client, limits),
                gate,
                cooperator,
            }),
        }
    }
}

#[async_trait(?Send)]
impl BidderFanout for EdgeZeroFanout {
    async fn send_all(
        &self,
        calls: Vec<BidderCall>,
        deadline: AuctionDeadline,
    ) -> Result<Vec<BidderCallResult>, FanoutInternalError> {
        if let Some(wave) = self.wave.as_ref() {
            return drive_fanout(
                calls,
                &deadline,
                WaveHost {
                    clock: self.clock.as_ref(),
                    timer: self.timer.as_ref(),
                    cooperator: wave.cooperator.as_ref(),
                },
                &wave.fetch,
                wave.fetch.limits(),
                &wave.gate,
            )
            .await;
        }
        // `join_all` preserves input order and yields an empty vec for
        // empty input with no special-casing. Each `drive_one` is
        // self-contained — whether it races its own timers or runs the
        // single-call path is the installed strategy's business.
        let calls_in_flight = calls.into_iter().map(|call| {
            self.strategy.drive_one(
                &self.adapter,
                call,
                &deadline,
                self.clock.as_ref(),
                self.timer.as_ref(),
            )
        });
        // Always `Ok`: a per-bidder failure is an `Err` inside the
        // `BidderCallResult`, and `FanoutInternalError` is reserved for
        // a genuine orchestration fault.
        Ok(join_all(calls_in_flight).await)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::outbound_request;

    use super::{EdgeZeroProxyAdapter, ProxyClientApi as _};
    use auctioneer_bidder::{DebugBodySchema, DebugBodyStatus};
    use auctioneer_runtime::{Clock as _, FakeClock, Timer as _};
    use edgezero_core::body::Body;
    use edgezero_core::error::EdgeError;
    use edgezero_core::http::{HeaderMap, HeaderValue, StatusCode};
    use edgezero_core::outbound::{
        ContentTypeProvenance, HttpClient, IndependentOutboundProgress, OutboundBatch,
        OutboundDispatchRequest, OutboundExchangeOwner, OutboundFuture, OutboundHead,
        OutboundHeaderFidelity, OutboundHeaderInput, OutboundHttpClient, OutboundPending,
        OutboundPreparedBatch, OutboundProgressRuntime, OutboundRequest, OutboundResponse,
        OutboundSendMode, OutboundTime,
    };
    use edgezero_core::time::MonotonicClock;
    use futures::executor::block_on;
    use futures::stream;
    use serde_json::json;
    use std::rc::Rc;
    use std::sync::Arc;
    use time::OffsetDateTime;

    struct EvidenceClient {
        provenance: ContentTypeProvenance,
        mutate: bool,
        clock: Arc<FakeClock>,
    }

    impl OutboundHttpClient for EvidenceClient {
        fn begin(&self, request: OutboundDispatchRequest) -> Result<OutboundPending, EdgeError> {
            let pending_head = self.dispatch_head(request);
            Ok(OutboundPending::new(
                Box::pin(async move { pending_head.await?.finish().await }),
                OutboundExchangeOwner::new(|| {}),
            ))
        }

        fn dispatch_head(&self, request: OutboundDispatchRequest) -> OutboundFuture<OutboundHead> {
            let (parts, context) = request.into_parts();
            let mut headers = HeaderMap::new();
            headers.insert("content-type", HeaderValue::from_static("application/json"));
            let mut response = OutboundResponse::new(
                parts.method,
                StatusCode::OK,
                headers,
                Body::from_bytes(
                    serde_json::to_vec(&json!({"id":"private", "cur":"USD"})).unwrap(),
                ),
                context.clock().clone(),
                self.provenance,
            );
            if self.mutate {
                response
                    .headers_mut()
                    .insert("x-observer", HeaderValue::from_static("changed"));
            }
            // The staged adapter snapshot retains the real headers_mut()
            // invalidation before the facade receives its original-field evidence.
            let (_method, status, response_headers, body, provenance) = response.into_parts();
            let fidelity = if provenance == ContentTypeProvenance::SingleTextUnchanged {
                OutboundHeaderFidelity::AdapterVisibleFields
            } else {
                OutboundHeaderFidelity::ProviderFieldBoundariesUnknown
            };
            let head = OutboundHead::new(
                status,
                OutboundHeaderInput::new("fixture", response_headers, fidelity),
                Box::pin(stream::iter([Ok(body
                    .into_bytes()
                    .expect("buffered fixture body"))])),
                context,
                OutboundExchangeOwner::new(|| {}),
            );
            Box::pin(async move { head })
        }

        fn progress(&self) -> Rc<dyn OutboundProgressRuntime> {
            Rc::new(IndependentOutboundProgress)
        }

        fn start_batch_until(&self, _: OutboundPreparedBatch) -> OutboundBatch {
            panic!("the legacy bridge must exercise the SDK send path");
        }

        fn time(&self) -> OutboundTime {
            let read_clock = Arc::clone(&self.clock);
            let waits = Arc::clone(&self.clock);
            OutboundTime::new(
                MonotonicClock::new(move || read_clock.now_monotonic()),
                move |deadline, clock| {
                    let waiter = Arc::clone(&waits);
                    Box::pin(async move {
                        if let Some(remaining) = deadline.remaining_at(clock.now()) {
                            waiter.sleep(remaining).await;
                        }
                        Ok(())
                    })
                },
            )
        }

        fn validate_supported_options(
            &self,
            _: &OutboundRequest,
            _: OutboundSendMode,
        ) -> Result<(), EdgeError> {
            Ok(())
        }
    }

    #[test]
    fn legacy_bridge_preserves_unknown_and_invalidated_original_header_evidence() {
        for (provenance, mutate, expected) in [
            (
                ContentTypeProvenance::SingleTextUnchanged,
                false,
                DebugBodyStatus::Captured,
            ),
            (
                ContentTypeProvenance::Unknown,
                false,
                DebugBodyStatus::OmittedType,
            ),
            (
                ContentTypeProvenance::SingleTextUnchanged,
                true,
                DebugBodyStatus::OmittedType,
            ),
        ] {
            let client = EdgeZeroProxyAdapter::new(HttpClient::with_client(EvidenceClient {
                provenance,
                mutate,
                clock: Arc::new(FakeClock::new(OffsetDateTime::UNIX_EPOCH)),
            }));
            let request = outbound_request::request(
                "POST".into(),
                "https://bidder.example/auction".into(),
                vec![],
                vec![],
                vec![],
            )
            .unwrap();
            let response = block_on(client.send_one(request))
                .unwrap()
                .within_node_budget(100);
            assert_eq!(response.status(), 200);
            assert!(
                response.body_equals(
                    &serde_json::to_vec(&json!({"id":"private", "cur":"USD"})).unwrap()
                )
            );
            assert!(
                response
                    .headers()
                    .iter()
                    .any(|(name, value)| name == "content-type" && value == "application/json")
            );
            let projected = response.project_debug_body(DebugBodySchema::OpenRtb);
            assert_eq!(projected.body_status, expected);
            if expected == DebugBodyStatus::Captured {
                assert_eq!(
                    serde_json::from_str::<serde_json::Value>(&projected.body.unwrap()).unwrap(),
                    json!({"id":"[REDACTED]", "cur":"USD"})
                );
            } else {
                assert_eq!(projected.body, None);
            }
        }
    }
}
