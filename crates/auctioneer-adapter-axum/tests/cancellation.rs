//! Losing the race must CANCEL the outbound call, not orphan it.
//!
//! A canned proxy client only proves orchestration: it says nothing
//! about whether a real in-flight transport future is dropped when a
//! timer arm wins. So this fixture is a real TCP server on a real
//! ephemeral port, driven through the same `edgezero-adapter-axum`
//! proxy client the Axum binary uses, and it observes the connection
//! from the server side.
//!
//! Raw TCP on purpose, with no HTTP framework in the loop: an HTTP
//! server's state machine can swallow the half-close that is exactly
//! the signal under test.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_bidder::{
        BidderName, BidderRequestInfo, CallBudget, EndpointRejectReason, EndpointValidator,
        EntryPoint, OutboundRequestSink, SinkLimits,
    };
    use auctioneer_core::host_primitives::{EdgeZeroFanout, SystemClock, axum_production_strategy};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderFanout as _, FanoutError, TimeoutCause, TmaxSource,
    };
    use auctioneer_runtime::{Clock, MonotonicInstant, Timer};
    use edgezero_adapter_axum::outbound::AxumOutboundClient;
    use edgezero_core::outbound::HttpClient;
    use std::io::ErrorKind;
    use std::str::from_utf8;
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::time::{sleep, timeout};

    /// How long the server waits after answering before it concludes the
    /// client is simply idle rather than gone.
    const PEEK_WINDOW: Duration = Duration::from_millis(500);

    /// Every fixture read is bounded, so a client that connects and then
    /// stalls cannot hang the suite.
    const READ_TIMEOUT: Duration = Duration::from_secs(5);

    /// The per-bidder budget the cancellation row races against.
    const CALL_TIMEOUT: Duration = Duration::from_millis(50);

    /// Well past anything these rows exercise: only the per-bidder arm
    /// may win, never the auction arm.
    const GENEROUS_DEADLINE: Duration = Duration::from_secs(30);

    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const RESPONSE_BODY: &str = r#"{"id":"cancellation","seatbid":[]}"#;

    /// What the server saw on one connection after it answered.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ConnOutcome {
        /// The client stayed connected for the whole peek window.
        Responded,
        /// The client went away — a half-close or a reset.
        ClientDrop,
        /// Unexpected extra bytes. With the request body already
        /// drained this is a real diagnostic, not the body arriving
        /// late.
        ContinuedReading,
    }

    /// Whether the fixture completes its response or only promises one.
    #[derive(Clone, Copy)]
    enum ResponseMode {
        /// Headers with a `content-length` whose body never arrives, so
        /// the client keeps reading until something cancels it.
        PromisedBodyNeverArrives,
        /// A complete, immediately readable response.
        Complete,
    }

    struct Fixture {
        port: u16,
        outcomes: Arc<Mutex<Vec<ConnOutcome>>>,
    }

    /// Tokio-backed `Timer` for the fixture. The server's `TokioTimer` is
    /// private to its binary, so integration tests use this local implementation.
    struct TransportTestTimer;

    #[async_trait(?Send)]
    impl Timer for TransportTestTimer {
        async fn sleep(&self, duration: Duration) {
            sleep(duration).await;
        }
    }

    fn record(outcomes: &Mutex<Vec<ConnOutcome>>, outcome: ConnOutcome) {
        outcomes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(outcome);
    }

    fn find_crlf_crlf(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn parse_content_length(header_block: &[u8]) -> Option<usize> {
        let text = from_utf8(header_block).ok()?;
        text.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.trim()
                    .eq_ignore_ascii_case("content-length")
                    .then(|| value.trim())
            })?
            .parse()
            .ok()
    }

    /// Read the request head, drain exactly the promised body, answer,
    /// then watch the connection.
    ///
    /// The body drain is the subtle part: one `read` can deliver the
    /// headers AND some or all of the body, so the remainder loop reads
    /// only `content_length - already_buffered` more bytes. A fresh
    /// `read_exact(content_length)` would block forever on bytes that
    /// already arrived, and skipping the drain entirely would let the
    /// first body byte surface in the peek race as a bogus
    /// `ContinuedReading`.
    async fn serve_one(
        mut stream: TcpStream,
        mode: ResponseMode,
        outcomes: &Mutex<Vec<ConnOutcome>>,
    ) {
        let mut buf: Vec<u8> = Vec::new();
        let header_terminator = loop {
            let mut chunk = [0_u8; 1024];
            let read = timeout(READ_TIMEOUT, stream.read(&mut chunk))
                .await
                .expect("request-header read timed out")
                .expect("request header reads");
            if read == 0 {
                break None;
            }
            buf.extend_from_slice(chunk.get(..read).unwrap_or_default());
            if let Some(at) = find_crlf_crlf(&buf) {
                break Some(at.saturating_add(4));
            }
            assert!(buf.len() < 64 * 1024, "request headers too large");
        };
        let Some(header_end) = header_terminator else {
            record(outcomes, ConnOutcome::ClientDrop);
            return;
        };
        // The fixture controls both ends, so a missing or malformed
        // length is a bug in the test, never a case to tolerate: reading
        // it as a zero-length body would leak the real body bytes into
        // the peek race.
        let content_length = parse_content_length(buf.get(..header_end).unwrap_or_default())
            .expect("the bidder POST must carry a valid Content-Length");
        let already = buf.len().saturating_sub(header_end);
        assert!(
            already <= content_length,
            "client sent {already} body bytes, exceeding Content-Length {content_length}",
        );
        let mut remaining = content_length.saturating_sub(already);
        while remaining > 0 {
            let mut chunk = vec![0_u8; remaining.min(8 * 1024)];
            let read = timeout(READ_TIMEOUT, stream.read(&mut chunk))
                .await
                .expect("request-body read timed out")
                .expect("request body reads");
            // EOF mid-body: the client left before finishing the
            // request. That is itself a drop — answering as though the
            // request had completed would report a truncated frame as a
            // normal one.
            if read == 0 {
                record(outcomes, ConnOutcome::ClientDrop);
                return;
            }
            remaining = remaining.saturating_sub(read);
        }

        let head = match mode {
            ResponseMode::PromisedBodyNeverArrives => {
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 500\r\n\r\n"
                    .to_owned()
            }
            ResponseMode::Complete => format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{RESPONSE_BODY}",
                RESPONSE_BODY.len(),
            ),
        };
        stream
            .write_all(head.as_bytes())
            .await
            .expect("response head writes");
        stream.flush().await.expect("response head flushes");

        // Body fully consumed, response sent: the only further bytes on
        // this socket are a disconnect.
        let mut peek = [0_u8; 1];
        let outcome = match timeout(PEEK_WINDOW, stream.read(&mut peek)).await {
            // Neither a byte nor a close inside the window: the client is
            // still holding the connection open.
            Err(_elapsed) => ConnOutcome::Responded,
            Ok(Ok(0)) => ConnOutcome::ClientDrop,
            Ok(Err(err))
                if matches!(
                    err.kind(),
                    ErrorKind::UnexpectedEof | ErrorKind::ConnectionReset | ErrorKind::BrokenPipe,
                ) =>
            {
                ConnOutcome::ClientDrop
            }
            // With the request body already drained, any further byte is
            // genuinely unexpected data rather than a late body chunk.
            Ok(_) => ConnOutcome::ContinuedReading,
        };
        record(outcomes, outcome);
    }

    async fn start_fixture(mode: ResponseMode) -> Fixture {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind an ephemeral port");
        let port = listener.local_addr().expect("local addr").port();
        let outcomes: Arc<Mutex<Vec<ConnOutcome>>> = Arc::new(Mutex::new(Vec::new()));
        let accept_recorder = Arc::clone(&outcomes);
        tokio::spawn(async move {
            while let Ok((stream, _peer)) = listener.accept().await {
                let conn_recorder = Arc::clone(&accept_recorder);
                tokio::spawn(async move { serve_one(stream, mode, &conn_recorder).await });
            }
        });
        Fixture { port, outcomes }
    }

    /// Poll the shared outcomes vector on a bounded spin. An outcome
    /// that never arrives means the future was orphaned rather than
    /// cancelled, so the caller fails on the empty vector.
    async fn wait_for_outcome(fixture: &Fixture, budget: Duration) -> Vec<ConnOutcome> {
        let step = Duration::from_millis(5);
        let mut waited = Duration::ZERO;
        while waited < budget {
            {
                let seen = fixture
                    .outcomes
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner);
                if !seen.is_empty() {
                    return seen.clone();
                }
            }
            sleep(step).await;
            waited = waited.saturating_add(step);
        }
        Vec::new()
    }

    fn edgezero_fanout() -> EdgeZeroFanout {
        let client = AxumOutboundClient::try_new().expect("reqwest client init");
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock::new());
        let timer: Arc<dyn Timer + Send + Sync> = Arc::new(TransportTestTimer);
        EdgeZeroFanout::new(
            HttpClient::with_client(client),
            clock,
            timer,
            axum_production_strategy(),
        )
    }

    // These tests exercise a real local transport, not publisher endpoint policy.
    struct FixtureEndpoint(u16);

    impl EndpointValidator for FixtureEndpoint {
        fn validate(
            &self,
            url: &url::Url,
            _: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            if url.as_str() == format!("http://127.0.0.1:{}/rtb", self.0) {
                Ok(())
            } else {
                Err(EndpointRejectReason::AuthorityMismatch)
            }
        }
    }

    fn call_to(port: u16, timeout: Duration) -> BidderCall {
        let endpoint = format!("http://127.0.0.1:{port}/rtb");
        let info = BidderRequestInfo::new(
            BidderName::new("fake_fixture"),
            BidderName::new("fake_fixture"),
            "account",
            None,
            &endpoint,
            &[],
            None,
            None,
            EntryPoint::Auction,
            &[],
        );
        let budget = CallBudget::new(1);
        let validator = FixtureEndpoint(port);
        let body = br#"{"id":"cancellation"}"#;
        let mut sink =
            OutboundRequestSink::new(SinkLimits::new(body.len(), 1), &budget, &validator);
        sink.push_request(
            &info,
            |meta| {
                meta.set_method("POST")?;
                meta.set_url(&endpoint)?;
                meta.header("content-type", "application/json")?;
                meta.imp_id("imp-1")
            },
            |writer| writer.write_all(body),
        )
        .expect("sink-built local fixture");
        BidderCall {
            request: sink
                .into_outcome()
                .accepted
                .pop()
                .expect("one fixture request"),
            timeout,
            max_transport_bytes: 1 << 20,
            max_decoded_bytes: 1 << 20,
        }
    }

    /// The transport tests run on the real host clock, so the
    /// generous deadline is composed against a live instant rather
    /// than a fake axis origin.
    fn deadline() -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: MonotonicInstant::now()
                .checked_add(GENEROUS_DEADLINE)
                .expect("no overflow"),
            source: TmaxSource::Request,
        }
    }

    /// The per-bidder budget elapses while the upstream is still
    /// withholding the body it promised. The server must SEE the client
    /// go away, which only happens if the losing send arm was dropped.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_lost_race_cancels_the_in_flight_transport_future() {
        let fixture = start_fixture(ResponseMode::PromisedBodyNeverArrives).await;
        let fanout = edgezero_fanout();

        let results = fanout
            .send_all(vec![call_to(fixture.port, CALL_TIMEOUT)], deadline())
            .await
            .expect("send_all itself succeeds");
        assert_eq!(results.len(), 1, "one call in, one result out");
        assert!(
            matches!(
                results.first().map(|row| &row.result),
                Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
            ),
            "the per-bidder budget ran out first, got {:?}",
            results.first().map(|row| &row.result),
        );

        let seen = wait_for_outcome(&fixture, Duration::from_millis(250)).await;
        assert_eq!(
            seen,
            vec![ConnOutcome::ClientDrop],
            "the server must observe the connection go away right after the timeout; an empty \
             record means the transport future was orphaned rather than cancelled",
        );
        drop(fanout);
    }

    /// The mirror image, so the classifier is shown to distinguish the
    /// two outcomes rather than reporting a drop unconditionally.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_fast_upstream_completes_and_the_server_records_no_drop() {
        let fixture = start_fixture(ResponseMode::Complete).await;
        let fanout = edgezero_fanout();

        let results = fanout
            .send_all(
                vec![call_to(fixture.port, Duration::from_secs(5))],
                deadline(),
            )
            .await
            .expect("send_all itself succeeds");
        assert_eq!(results.len(), 1, "one call in, one result out");
        assert!(
            results.first().is_some_and(|row| row.result.is_ok()),
            "an upstream that answers inside the budget succeeds, got {:?}",
            results.first().map(|row| &row.result),
        );

        // The fanout — and with it the pooled connection — stays alive
        // until the server's peek window closes, so a `Responded` here
        // is the server outliving the call rather than a lucky race.
        let seen = wait_for_outcome(&fixture, Duration::from_secs(3)).await;
        assert_eq!(
            seen,
            vec![ConnOutcome::Responded],
            "a completed call leaves the connection in place for the whole peek window",
        );
        drop(fanout);
    }

    /// Headers and body in a SINGLE write, which is the interleaving a
    /// naive `read_exact(content_length)` would deadlock on: the body
    /// bytes are already in the fixture's buffer, so there is nothing
    /// left to read.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_fixture_drains_a_request_delivered_in_one_write() {
        let fixture = start_fixture(ResponseMode::Complete).await;
        // Raw string, not `serde_json::json!`: this fixture's own `len()` is
        // read below, so the exact rendered text is part of what the row
        // asserts. `json!` renders its own spacing.
        let body = r#"{"id":"single-write"}"#;
        let request = format!(
            "POST /rtb HTTP/1.1\r\nhost: 127.0.0.1\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{body}",
            body.len(),
        );
        let mut stream = TcpStream::connect(("127.0.0.1", fixture.port))
            .await
            .expect("connect to the fixture");
        stream
            .write_all(request.as_bytes())
            .await
            .expect("the whole request writes at once");
        stream.flush().await.expect("request flushes");

        // The fixture answers only AFTER draining the body, so reading
        // its status line back is the proof that the remainder loop was
        // skipped instead of blocking.
        let mut head = [0_u8; 12];
        timeout(Duration::from_secs(2), stream.read_exact(&mut head))
            .await
            .expect("the fixture must not block when headers and body arrive together")
            .expect("the response head reads");
        assert_eq!(
            &head, b"HTTP/1.1 200",
            "the fixture answered the single-write request",
        );

        drop(stream);
        let seen = wait_for_outcome(&fixture, Duration::from_secs(2)).await;
        assert_eq!(
            seen,
            vec![ConnOutcome::ClientDrop],
            "and it still reports the outcome once the client goes away",
        );
    }
}
