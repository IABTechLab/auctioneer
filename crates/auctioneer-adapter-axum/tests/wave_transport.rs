//! The native adapter's wave drive over a real socket — the path a served request takes.
//!
//! The sibling `cancellation.rs` drives real TCP through
//! [`EdgeZeroFanout::new`]. That constructor leaves `wave: None`, so
//! `send_all` takes the per-call strategy branch — the arm
//! `handlers::auction::fanout_from_context` reaches only when no boot
//! envelope is installed, and whose own comment calls that unreachable
//! for anything the ingress policy admitted. All four adapters'
//! `envelope.rs` document [`EdgeZeroFanout::with_wave_scheduler`] as
//! the preferred constructor, and that is the one the handler returns
//! first.
//!
//! The wave drive was covered only by canned clients: the conformance
//! suite's `BoundedWave` arm wraps an in-memory `TestProxyClient`, and
//! `auctioneer-core`'s oversize and mid-flight-timeout tests script an
//! `OutboundBatch` directly. None of them opens a socket. So the three
//! behaviours that are ABOUT the transport — per-call cancellation,
//! the per-call deadline, and the response body cap — were unverified
//! on the drive production runs.
//!
//! What a canned client cannot show, and each case here does:
//!
//!   - CANCELLATION: whether a losing race actually drops the in-flight
//!     transport future, which is only observable from the other end of
//!     a connection.
//!   - DEADLINE: whether the ONE absolute instant `compose_all` built
//!     reaches the wire as a deadline the transport enforces. The wave
//!     drive is the only drive that propagates it; the per-call bridge
//!     sends a request carrying no deadline at all.
//!   - BODY CAP: whether `max_transport_bytes` stops a real streaming
//!     body. A scripted batch hands back a ready-made
//!     `EdgeError::ResponseTooLarge`, which proves the classifier and
//!     nothing about enforcement.
//!
//! # Which rows can tell the two drives apart
//!
//! Stated because it decides what each row is evidence FOR. Swapping
//! `with_wave_scheduler`'s wave for `None` — the per-call bridge, under
//! the wave constructor's name — fails the original plain-body cap
//! rows: only the wave drive sets a response-body policy, so the bridge
//! answers `Other("transport error")` where the caps should fire.
//!
//! The single-call cancellation and deadline rows pass against BOTH drives, because
//! both cancel a losing send and both end a call on its budget. They
//! are still worth having — nothing else drove the wave path over a
//! socket at all, so neither behaviour was verified on it — but they
//! are not evidence about WHICH drive ran, and the cap rows are.
//!
//! Additional rows exercise concurrent sibling deadlines and slot ordering,
//! exact and over-limit compressed/decoded caps, and a late gzip EOF.
//! Each checks permit recovery by sending another full-width healthy wave.
//!
//! Raw TCP with no HTTP framework in the loop, for the same reason the
//! sibling gives: a server's state machine can swallow the half-close
//! that is the cancellation signal under test.

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use auctioneer_bidder::{
        BidderName, BidderRequestInfo, CallBudget, EndpointRejectReason, EndpointValidator,
        EntryPoint, OutboundRequestSink, SinkLimits,
    };
    use auctioneer_core::host_primitives::{EdgeZeroFanout, SystemClock, axum_production_strategy};
    use auctioneer_exchange::{
        AuctionDeadline, BidderCall, BidderFanout as _, FanoutError, OversizeOrigin, TimeoutCause,
        TmaxSource,
    };
    use auctioneer_runtime::{
        Clock, MAX_GVL_PARSED_BYTES, MIN_INBOUND_PARSE_FACTOR, MIN_RESPONSE_NODE_PEAK_BYTES,
        MIN_RESPONSE_PARSE_FACTOR, MonotonicInstant, NoopDecodeCooperator, ResolvedIsolateLimits,
        StreamGate, Timer,
    };
    use edgezero_adapter_axum::outbound::AxumOutboundClient;
    use edgezero_core::config_store::DEFAULT_CONFIG_JSON_NODES;
    use edgezero_core::outbound::HttpClient;
    use futures::future::{Either, select};
    use std::future::{Future, poll_fn};
    use std::io::ErrorKind;
    use std::iter::repeat_with;
    use std::str::from_utf8;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::task::Poll;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::oneshot;
    use tokio::task::{JoinHandle, LocalSet};
    use tokio::time::{sleep, timeout};

    /// How long the server waits after answering before it concludes
    /// the client is idle rather than gone.
    const PEEK_WINDOW: Duration = Duration::from_millis(500);

    /// Every fixture read is bounded, so a client that connects and
    /// then stalls cannot hang the suite.
    const READ_TIMEOUT: Duration = Duration::from_secs(5);

    /// The per-bidder budget the cancellation and deadline rows race.
    const CALL_TIMEOUT: Duration = Duration::from_millis(50);

    /// Well past anything these rows exercise: only the per-bidder arm
    /// may win, never the auction arm.
    const GENEROUS_DEADLINE: Duration = Duration::from_secs(30);

    /// The transport cap the body-cap row breaches.
    ///
    /// Small enough that the fixture can overshoot it quickly, and well
    /// under every other cap in [`wave_limits`] so the refusal can only
    /// be this one.
    const TRANSPORT_CAP: u64 = 4 * 1024;

    /// How many concurrent body streams the gate admits.
    ///
    /// Wider than any row's call count, so the gate never splits a
    /// row's wave into several — a split would change what a row
    /// observes without changing what it claims.
    const WAVE_GATE_CAPACITY: u32 = 8;

    // Raw-string `const`: `serde_json::json!` yields a `Value`, which no
    // `const` can hold. Converting means turning this into a function
    // returning a `String` and re-rendering it at every use.
    const RESPONSE_BODY: &str = r#"{"id":"wave","seatbid":[]}"#;

    // A gzip member containing RESPONSE_BODY followed by 8192 spaces.
    // Fixed wire bytes avoid adding a compression dependency to this fixture.
    const GZIP_PADDING_BYTES: usize = 8192;
    const GZIP_BODY: &[u8] = &[
        0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0xed, 0xc8, 0xb1, 0x09, 0x80,
        0x30, 0x00, 0x04, 0xc0, 0x55, 0xe4, 0x6b, 0x27, 0x70, 0x15, 0xb1, 0x48, 0x30, 0x45, 0x6a,
        0x45, 0x0b, 0x71, 0x77, 0x71, 0x8c, 0xc0, 0x5d, 0x79, 0x4f, 0xfa, 0x9e, 0x25, 0x77, 0xb9,
        0x5a, 0xe6, 0x1c, 0xad, 0x9c, 0xf5, 0x8f, 0x75, 0x7b, 0x27, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x60, 0x78, 0x1f, 0x72, 0xdb, 0x23, 0x05, 0x1a, 0x20, 0x00, 0x00,
    ];

    /// What the server saw on one connection after it answered.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum ConnOutcome {
        /// The client stayed connected for the whole peek window.
        Responded,
        /// The client went away — a half-close or a reset.
        ClientDrop,
        /// Unexpected extra bytes. With the request body already
        /// drained this is a real diagnostic, not a late body chunk.
        ContinuedReading,
    }

    /// Whether the fixture completes its response, withholds it, or
    /// overruns the transport cap.
    #[derive(Clone)]
    enum ResponseMode {
        /// Exact response bytes, including non-text or repeated header values.
        RawComplete(Vec<u8>),
        /// Headers with a `content-length` whose body never arrives, so
        /// the client keeps reading until something cancels it.
        PromisedBodyNeverArrives,
        /// A complete, immediately readable response.
        Complete,
        /// Keep the healthy sibling pending until the short call is cancelled.
        CompleteAfterDrop(Arc<Mutex<Vec<ConnOutcome>>>),
        /// A complete gzip member, without a declared encoded length.
        GzipChunkedBody,
        /// Send the entire deflate payload, then terminate HTTP without the
        /// gzip trailer only when the test has checked the pending read.
        GzipLateEof {
            prefix_sent: Arc<AtomicBool>,
            release_eof: Arc<AtomicBool>,
        },
        /// A chunked body streamed well past the transport cap.
        ///
        /// Chunked on purpose: a `content-length` over the cap could be
        /// refused from the header alone, which would prove a header
        /// check rather than enforcement during the body read.
        OversizeChunkedBody,
    }

    struct Fixture {
        port: u16,
        outcomes: Arc<Mutex<Vec<ConnOutcome>>>,
        requests_seen: Arc<AtomicUsize>,
    }

    /// One owned task covers both accept and the withheld response.
    struct CancellationFixture {
        fixture: Fixture,
        headers_ready: oneshot::Receiver<()>,
        headers_established: bool,
        shutdown: Option<oneshot::Sender<()>>,
        task: Option<JoinHandle<()>>,
    }

    impl CancellationFixture {
        async fn wait_for_headers(&mut self) -> Result<(), &'static str> {
            timeout(READ_TIMEOUT, &mut self.headers_ready)
                .await
                .map_err(|_elapsed| "response headers did not arrive")?
                .map_err(|_closed| "fixture exited before flushing response headers")?;
            self.headers_established = true;
            Ok(())
        }

        async fn finish(mut self) -> (bool, Vec<ConnOutcome>) {
            if let Some(shutdown) = self.shutdown.take() {
                let _closed = shutdown.send(());
            }
            let task = self.task.as_mut().expect("the fixture owns its task");
            let joined = timeout(READ_TIMEOUT, &mut *task).await;
            if joined.is_err() {
                task.abort();
                let _cancelled = task.await;
                panic!("the cancellation fixture did not finish within {READ_TIMEOUT:?}");
            }
            joined
                .expect("fixture finished before its watchdog")
                .expect("the cancellation fixture task must not panic");
            let headers_established =
                self.headers_established || self.headers_ready.try_recv().is_ok();
            let outcomes = self
                .fixture
                .outcomes
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            (headers_established, outcomes)
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for CancellationFixture {
        fn drop(&mut self) {
            if let Some(task) = &self.task {
                task.abort();
            }
        }
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

    /// Stream chunked body bytes until the client gives up.
    ///
    /// Returns once a write fails, which is what a client that stopped
    /// reading after its cap looks like from here. The total is bounded
    /// so a client that keeps reading cannot make this loop forever.
    async fn stream_past_the_cap(stream: &mut TcpStream) {
        let chunk = vec![b'x'; 1024];
        let ceiling = usize::try_from(TRANSPORT_CAP)
            .unwrap_or(usize::MAX)
            .saturating_mul(8);
        let mut sent = 0_usize;
        while sent < ceiling {
            let framed = format!("{:x}\r\n", chunk.len());
            if stream.write_all(framed.as_bytes()).await.is_err()
                || stream.write_all(&chunk).await.is_err()
                || stream.write_all(b"\r\n").await.is_err()
            {
                return;
            }
            if stream.flush().await.is_err() {
                return;
            }
            sent = sent.saturating_add(chunk.len());
        }
    }

    /// Read the request head and drain exactly the promised body.
    ///
    /// The body drain is the subtle part: one `read` can deliver the
    /// headers AND some or all of the body, so the remainder loop reads
    /// only `content_length - already_buffered` more bytes. A fresh
    /// `read_exact(content_length)` would block forever on bytes that
    /// already arrived, and skipping the drain entirely would let the
    /// first body byte surface in the peek race as a bogus
    /// `ContinuedReading`.
    async fn drain_request(stream: &mut TcpStream) -> bool {
        let mut buf: Vec<u8> = Vec::new();
        let header_terminator = loop {
            let mut chunk = [0_u8; 1024];
            let read = timeout(READ_TIMEOUT, stream.read(&mut chunk))
                .await
                .expect("request-header read timed out")
                .unwrap_or_else(|err| {
                    if matches!(
                        err.kind(),
                        ErrorKind::UnexpectedEof
                            | ErrorKind::ConnectionReset
                            | ErrorKind::BrokenPipe,
                    ) {
                        0
                    } else {
                        panic!("request header reads: {err}");
                    }
                });
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
            return false;
        };
        // The fixture controls both ends, so a missing or malformed
        // length is a bug in the test, never a case to tolerate:
        // reading it as a zero-length body would leak the real body
        // bytes into the peek race.
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
                .unwrap_or_else(|err| {
                    if matches!(
                        err.kind(),
                        ErrorKind::UnexpectedEof
                            | ErrorKind::ConnectionReset
                            | ErrorKind::BrokenPipe,
                    ) {
                        0
                    } else {
                        panic!("request body reads: {err}");
                    }
                });
            // EOF mid-body: the client left before finishing the
            // request. That is itself a drop — answering as though the
            // request had completed would report a truncated frame as a
            // normal one.
            if read == 0 {
                return false;
            }
            remaining = remaining.saturating_sub(read);
        }
        true
    }

    async fn write_gzip_body(stream: &mut TcpStream, mode: &ResponseMode) {
        let body = if matches!(mode, ResponseMode::GzipLateEof { .. }) {
            GZIP_BODY
                .get(..GZIP_BODY.len().saturating_sub(8))
                .expect("gzip payload without its trailer")
        } else {
            GZIP_BODY
        };
        stream
            .write_all(format!("{:x}\r\n", body.len()).as_bytes())
            .await
            .expect("gzip chunk size writes");
        stream.write_all(body).await.expect("gzip payload writes");
        stream.write_all(b"\r\n").await.expect("chunk ends");
        stream.flush().await.expect("gzip payload flushes");
        if let ResponseMode::GzipLateEof {
            prefix_sent,
            release_eof,
        } = mode
        {
            prefix_sent.store(true, Ordering::SeqCst);
            wait_for_flag(release_eof).await;
        }
        // HTTP completes cleanly, so the late EOF is a codec failure,
        // not a truncated chunked-transfer frame.
        stream
            .write_all(b"0\r\n\r\n")
            .await
            .expect("HTTP body ends");
        stream.flush().await.expect("HTTP EOF flushes");
    }

    /// Answer a fully drained request, then watch for cancellation.
    async fn serve_one(
        mut stream: TcpStream,
        mode: ResponseMode,
        outcomes: &Mutex<Vec<ConnOutcome>>,
        requests_seen: &AtomicUsize,
        headers_ready: Option<oneshot::Sender<()>>,
    ) {
        if !drain_request(&mut stream).await {
            record(outcomes, ConnOutcome::ClientDrop);
            return;
        }
        requests_seen.fetch_add(1, Ordering::SeqCst);

        if let ResponseMode::CompleteAfterDrop(short_outcomes) = &mode {
            assert_eq!(
                wait_for_record(short_outcomes, READ_TIMEOUT).await,
                vec![ConnOutcome::ClientDrop],
                "the healthy response waits for the short sibling's disconnect",
            );
        }

        let head = match &mode {
            ResponseMode::RawComplete(_) => String::new(),
            ResponseMode::PromisedBodyNeverArrives => {
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 500\r\n\r\n"
                    .to_owned()
            }
            ResponseMode::Complete => format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{RESPONSE_BODY}",
                RESPONSE_BODY.len(),
            ),
            ResponseMode::CompleteAfterDrop(_) => {
                let body = healthy_sibling_body();
                let json = from_utf8(&body).expect("serialized JSON is UTF-8");
                format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\n\r\n{json}",
                    body.len(),
                )
            }
            ResponseMode::GzipChunkedBody | ResponseMode::GzipLateEof { .. } => {
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                 content-encoding: gzip\r\ntransfer-encoding: chunked\r\n\r\n"
                    .to_owned()
            }
            ResponseMode::OversizeChunkedBody => {
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                 transfer-encoding: chunked\r\n\r\n"
                    .to_owned()
            }
        };
        let wire_head = if let ResponseMode::RawComplete(bytes) = &mode {
            bytes.as_slice()
        } else {
            head.as_bytes()
        };
        let write_head = async {
            stream.write_all(wire_head).await?;
            stream.flush().await
        }
        .await;
        if let Err(err) = write_head {
            if headers_ready.is_some()
                && matches!(
                    err.kind(),
                    ErrorKind::UnexpectedEof | ErrorKind::ConnectionReset | ErrorKind::BrokenPipe,
                )
            {
                record(outcomes, ConnOutcome::ClientDrop);
                return;
            }
            panic!("response head writes and flushes: {err}");
        }
        if let Some(header_notifier) = headers_ready {
            let _closed = header_notifier.send(());
        }

        if matches!(
            mode,
            ResponseMode::GzipChunkedBody | ResponseMode::GzipLateEof { .. }
        ) {
            write_gzip_body(&mut stream, &mode).await;
        }

        if matches!(mode, ResponseMode::OversizeChunkedBody) {
            stream_past_the_cap(&mut stream).await;
            // The client stopped reading at its cap, so the connection
            // is already going away; recording a drop here would race
            // the write failure above rather than observe anything.
            record(outcomes, ConnOutcome::ClientDrop);
            return;
        }

        // Request fully consumed, response head sent: any further bytes
        // on this socket are unexpected.
        let mut peek = [0_u8; 1];
        // A withheld body must outlive the call budget; the idle window
        // is only appropriate after a complete response.
        let peek_window = if matches!(mode, ResponseMode::PromisedBodyNeverArrives) {
            READ_TIMEOUT
        } else {
            PEEK_WINDOW
        };
        let outcome = match timeout(peek_window, stream.read(&mut peek)).await {
            // Neither a byte nor a close inside the window: the client
            // is still holding the connection open.
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
            // With the request body already drained, any further byte
            // is genuinely unexpected data rather than a late chunk.
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
        let requests_seen = Arc::new(AtomicUsize::new(0));
        let accept_recorder = Arc::clone(&outcomes);
        let accept_requests = Arc::clone(&requests_seen);
        tokio::spawn(async move {
            while let Ok((stream, _peer)) = listener.accept().await {
                let conn_recorder = Arc::clone(&accept_recorder);
                let conn_requests = Arc::clone(&accept_requests);
                let conn_mode = mode.clone();
                tokio::spawn(async move {
                    serve_one(stream, conn_mode, &conn_recorder, &conn_requests, None).await;
                });
            }
        });
        Fixture {
            port,
            outcomes,
            requests_seen,
        }
    }

    async fn start_cancellation_fixture() -> CancellationFixture {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind an ephemeral port");
        let port = listener.local_addr().expect("local addr").port();
        let outcomes = Arc::new(Mutex::new(Vec::new()));
        let requests_seen = Arc::new(AtomicUsize::new(0));
        let recorder = Arc::clone(&outcomes);
        let requests = Arc::clone(&requests_seen);
        let (headers_ready, headers_received) = oneshot::channel();
        let (shutdown, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            let (stream, _peer) = match select(Box::pin(listener.accept()), stopped).await {
                Either::Left((accepted, _shutdown)) => {
                    accepted.expect("accept the cancellation connection")
                }
                Either::Right((_shutdown, _accept)) => return,
            };
            drop(listener);
            serve_one(
                stream,
                ResponseMode::PromisedBodyNeverArrives,
                &recorder,
                &requests,
                Some(headers_ready),
            )
            .await;
        });
        CancellationFixture {
            fixture: Fixture {
                port,
                outcomes,
                requests_seen,
            },
            headers_ready: headers_received,
            headers_established: false,
            shutdown: Some(shutdown),
            task: Some(task),
        }
    }

    fn pre_header_cleanup_is_valid(requests_seen: usize, outcomes: &[ConnOutcome]) -> bool {
        (requests_seen == 0 || !outcomes.is_empty())
            && outcomes
                .iter()
                .all(|outcome| *outcome == ConnOutcome::ClientDrop)
    }

    #[test]
    fn pre_header_cleanup_requires_a_record_after_a_drained_request() {
        assert!(!pre_header_cleanup_is_valid(1, &[]));
        assert!(pre_header_cleanup_is_valid(0, &[]));
        for requests_seen in [0, 1] {
            assert!(pre_header_cleanup_is_valid(
                requests_seen,
                &[ConnOutcome::ClientDrop],
            ));
            for outcome in [ConnOutcome::Responded, ConnOutcome::ContinuedReading] {
                assert!(!pre_header_cleanup_is_valid(requests_seen, &[outcome]));
                assert!(!pre_header_cleanup_is_valid(
                    requests_seen,
                    &[ConnOutcome::ClientDrop, outcome],
                ));
            }
        }
    }

    async fn assert_deadline_connection_closed(mut fixture: CancellationFixture) {
        fixture.headers_established = fixture.headers_ready.try_recv().is_ok();
        let seen = if fixture.headers_established {
            Some(wait_for_outcome(&fixture.fixture, PEEK_WINDOW).await)
        } else {
            None
        };
        let requests_seen = Arc::clone(&fixture.fixture.requests_seen);
        let (headers_established, outcomes) = fixture.finish().await;
        let drained_requests = requests_seen.load(Ordering::SeqCst);
        if let Some(observed) = seen {
            assert_eq!(
                observed,
                vec![ConnOutcome::ClientDrop],
                "deadline expiry closes an established connection within {PEEK_WINDOW:?}",
            );
        }
        if headers_established {
            assert_eq!(outcomes, vec![ConnOutcome::ClientDrop]);
        } else {
            assert!(
                pre_header_cleanup_is_valid(drained_requests, &outcomes),
                "pre-header cleanup must record ClientDrop after a drained request: \
                 requests_seen={drained_requests}, outcomes={outcomes:?}",
            );
        }
    }

    /// An empty record alone does not distinguish pre-connect expiry
    /// from a connection that has not yet reported an outcome.
    async fn wait_for_outcome(fixture: &Fixture, budget: Duration) -> Vec<ConnOutcome> {
        wait_for_record(&fixture.outcomes, budget).await
    }

    async fn wait_for_record(
        outcomes: &Mutex<Vec<ConnOutcome>>,
        budget: Duration,
    ) -> Vec<ConnOutcome> {
        let step = Duration::from_millis(5);
        let mut waited = Duration::ZERO;
        while waited < budget {
            {
                let seen = outcomes.lock().unwrap_or_else(PoisonError::into_inner);
                if !seen.is_empty() {
                    return seen.clone();
                }
            }
            sleep(step).await;
            waited = waited.saturating_add(step);
        }
        Vec::new()
    }

    async fn wait_for_flag(flag: &AtomicBool) {
        timeout(READ_TIMEOUT, async {
            while !flag.load(Ordering::SeqCst) {
                sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("fixture signal arrives before the read timeout");
    }

    /// The limits the wave drive charges against.
    ///
    /// Every cap but `max_transport_bytes` is wide enough that no row
    /// trips it, so an `OversizeResponse` here can only be the
    /// transport cap the body-cap row targets.
    fn wave_limits() -> ResolvedIsolateLimits {
        ResolvedIsolateLimits {
            isolate_total_bytes: 512 * 1024 * 1024,
            baseline_bytes: 0,
            inbound_request_bytes: 256 * 1024,
            inbound_parse_factor: MIN_INBOUND_PARSE_FACTOR,
            max_outbound_request_bytes: 256 * 1024,
            max_outbound_raw_bytes: 256 * 1024,
            max_transport_bytes: TRANSPORT_CAP,
            max_decoded_bytes: 256 * 1024,
            response_parse_factor: MIN_RESPONSE_PARSE_FACTOR,
            response_node_peak_bytes: MIN_RESPONSE_NODE_PEAK_BYTES,
            final_response_bytes: 256 * 1024,
            egress_permits: 4,
            response_write_deadline_ms: 10_000,
            guest_decoder_bound_bytes: 0,
            aux_route_permits: 2,
            aux_route_peak_bytes: 1024,
            max_brotli_wbits: 22,
            max_brotli_layers: 1,
            gvl_cache_entries: 1,
            gvl_parsed_bytes: MAX_GVL_PARSED_BYTES,
            max_bidders_per_auction: 8,
            max_outbound_requests: 8,
            max_concurrent_auctions: 4,
            max_active_body_streams: WAVE_GATE_CAPACITY,
            max_runtime_config_bytes: 1024,
            max_runtime_config_blob_bytes: 1024,
            max_parsed_nodes: 1024,
            max_copied_header_bytes: 1024,
            max_admitted_header_bytes: 8192,
            max_admitted_header_count: 64,
            max_impressions_per_auction: 100,
            max_config_json_nodes: DEFAULT_CONFIG_JSON_NODES,
            terminal_response_permits: 2,
            refusal_body_bytes: 1024,
            transmit_state_bytes: 1024,
        }
    }

    /// The fanout under test: the SAME constructor
    /// `handlers::auction::fanout_from_context` returns on a served
    /// request, over a real `reqwest`-backed outbound client.
    fn wave_fanout() -> EdgeZeroFanout {
        wave_fanout_with_gate(StreamGate::new(WAVE_GATE_CAPACITY))
    }

    fn wave_fanout_with_gate(gate: Arc<StreamGate>) -> EdgeZeroFanout {
        let client = AxumOutboundClient::try_new().expect("reqwest client init");
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(SystemClock::new());
        let timer: Arc<dyn Timer + Send + Sync> = Arc::new(TransportTestTimer);
        EdgeZeroFanout::with_wave_scheduler(
            HttpClient::with_client(client),
            clock,
            timer,
            Arc::new(NoopDecodeCooperator),
            axum_production_strategy(),
            wave_limits(),
            gate,
        )
    }

    fn assert_gate_recovered(gate: &StreamGate) {
        assert_eq!(gate.free(), gate.capacity(), "all stream weight returned");
        assert_eq!(gate.in_flight(), 0, "no live wave permit remains");
        assert_eq!(gate.granted(), 0, "no grant remains parked");
        assert_eq!(gate.waiting_len(), 0, "no acquisition remains queued");
        assert_eq!(gate.node_count(), 0, "no waiter storage remains");
    }

    async fn assert_healthy_wave_reuses_gate(fanout: &EdgeZeroFanout, gate: &StreamGate) {
        assert_gate_recovered(gate);
        let fixture = start_fixture(ResponseMode::Complete).await;
        let width = usize::try_from(gate.capacity()).expect("capacity fits");
        let calls = repeat_with(|| call_to(fixture.port, Duration::from_secs(3)))
            .take(width)
            .collect();
        let results = timeout(READ_TIMEOUT, fanout.send_all(calls, deadline()))
            .await
            .expect("the recovered gate admits another full-width wave")
            .expect("the recovery fanout succeeds");
        assert_eq!(results.len(), width);
        for row in results {
            assert_eq!(
                row.result.expect("the recovery call succeeds").into_body(),
                RESPONSE_BODY.as_bytes(),
            );
        }
        assert_gate_recovered(gate);
    }

    /// The fallback drive, for the rows that contrast the two.
    fn bridge_fanout() -> EdgeZeroFanout {
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

    fn call_to(port: u16, call_timeout: Duration) -> BidderCall {
        call_capped(port, call_timeout, 1 << 20)
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

    /// A call carrying its own `max_decoded_bytes`.
    ///
    /// Separate from [`call_to`] because only one row varies this: the
    /// per-call cap is the knob `narrow_to_call` applies, and that
    /// narrowing exists ONLY on the wave drive.
    fn call_capped(port: u16, call_timeout: Duration, max_decoded_bytes: u64) -> BidderCall {
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
        let body = br#"{"id":"wave"}"#;
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
            timeout: call_timeout,
            max_transport_bytes: 1 << 20,
            max_decoded_bytes,
        }
    }

    fn healthy_sibling_body() -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({
            "id": "healthy-sibling",
            "seatbid": [],
        }))
        .expect("serialize the healthy sibling response")
    }

    /// The transport tests run on the real host clock, so the generous
    /// deadline is composed against a live instant rather than a fake
    /// axis origin.
    fn deadline() -> AuctionDeadline {
        AuctionDeadline {
            deadline_at: MonotonicInstant::now()
                .checked_add(GENEROUS_DEADLINE)
                .expect("no overflow"),
            source: TmaxSource::Request,
        }
    }

    // SDK batches eagerly schedule local tasks, matching the production Axum runner.
    async fn with_local_scheduler(work: impl Future<Output = ()>) {
        LocalSet::new().run_until(work).await;
    }

    /// Deadline expiry may precede connection establishment. Once headers
    /// were sent, expiry must also close the established connection.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_cancels_a_lost_race_over_a_real_socket() {
        with_local_scheduler(async {
            let fixture = start_cancellation_fixture().await;
            let fanout = wave_fanout();

            let completed = fanout
                .send_all(
                    vec![call_to(fixture.fixture.port, CALL_TIMEOUT)],
                    deadline(),
                )
                .await;
            assert_deadline_connection_closed(fixture).await;
            let results = completed.expect("send_all itself succeeds");
            assert_eq!(results.len(), 1, "one call in, one result out");
            assert!(
                matches!(
                    results.first().map(|row| &row.result),
                    Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
                ),
                "the per-bidder budget ran out first, got {:?}",
                results.first().map(|row| &row.result),
            );

            drop(fanout);
        })
        .await;
    }

    async fn assert_post_headers_cancellation(fanout: &EdgeZeroFanout) {
        let mut fixture = start_cancellation_fixture().await;
        // This row tests explicit cancellation, not deadline enforcement.
        // Establishment has its own READ_TIMEOUT watchdog; CALL_TIMEOUT
        // stays unchanged in the separate deadline rows.
        let mut send = Box::pin(fanout.send_all(
            vec![call_to(fixture.fixture.port, GENEROUS_DEADLINE)],
            deadline(),
        ));
        let ready = match select(send.as_mut(), Box::pin(fixture.wait_for_headers())).await {
            Either::Left((result, _headers)) => Err(format!(
                "the withheld body completed before cancellation: {result:?}"
            )),
            Either::Right((acknowledged, _send)) => acknowledged.map_err(str::to_owned),
        };
        let send_pending = if ready.is_ok() {
            poll_fn(|context| Poll::Ready(send.as_mut().poll(context).is_pending())).await
        } else {
            false
        };
        let connection_pending = fixture
            .fixture
            .outcomes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_empty();
        let requests_seen = fixture.fixture.requests_seen.load(Ordering::SeqCst);

        drop(send);
        // The borrowed fanout (including its pooled client) stays alive
        // throughout observation and fixture teardown.
        let seen = wait_for_outcome(&fixture.fixture, PEEK_WINDOW).await;
        let (headers_established, outcomes) = fixture.finish().await;
        ready.expect("the request reaches flushed response headers before cancellation");
        assert!(
            send_pending,
            "the send remains Pending after the header acknowledgement"
        );
        assert!(
            connection_pending,
            "the connection is pending before explicit cancellation"
        );
        assert_eq!(requests_seen, 1);
        assert!(headers_established);
        assert_eq!(
            seen,
            vec![ConnOutcome::ClientDrop],
            "dropping the send must close the established socket within {PEEK_WINDOW:?}: {outcomes:?}",
        );
        assert_eq!(outcomes, vec![ConnOutcome::ClientDrop]);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_cancels_an_established_body_read_when_dropped() {
        with_local_scheduler(async {
            let gate = StreamGate::new(1);
            let fanout = wave_fanout_with_gate(Arc::clone(&gate));
            assert_post_headers_cancellation(&fanout).await;
            assert_healthy_wave_reuses_gate(&fanout, &gate).await;
        })
        .await;
    }

    /// The mirror image, so the wave drive's classifier is shown to
    /// distinguish the two outcomes rather than reporting a drop
    /// unconditionally.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_publishes_a_fast_upstream_and_records_no_drop() {
        with_local_scheduler(async {
            let fixture = start_fixture(ResponseMode::Complete).await;
            let fanout = wave_fanout();

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
        })
        .await;
    }

    /// THE BODY CAP, enforced during a real streaming read.
    ///
    /// The fixture answers with a chunked body and keeps writing well
    /// past `max_transport_bytes`. The call must be refused as
    /// `OversizeResponse` carrying the ENCODED-BODY origin — upstream's
    /// name for the on-the-wire cap, before any decode — which is the
    /// one cap narrowed in [`wave_limits`].
    ///
    /// Chunked rather than a declared `content-length` over the cap: a
    /// length header could be refused before a single body byte is
    /// read, and that would prove a header check instead of enforcement
    /// while the body streams.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_refuses_a_body_past_the_transport_cap_on_the_wire() {
        with_local_scheduler(async {
            let fixture = start_fixture(ResponseMode::OversizeChunkedBody).await;
            let fanout = wave_fanout();

            let results = fanout
                .send_all(
                    // A budget long enough that a timeout cannot be what
                    // ends this call: the refusal has to be the cap.
                    vec![call_to(fixture.port, Duration::from_secs(10))],
                    deadline(),
                )
                .await
                .expect("send_all itself succeeds");
            assert_eq!(results.len(), 1, "one call in, one result out");
            let outcome = results.first().map(|row| &row.result);
            assert!(
                matches!(
                    outcome,
                    Some(Err(FanoutError::OversizeResponse(
                        OversizeOrigin::EncodedBody
                    ))),
                ),
                "a body streamed past max_transport_bytes must be refused as an oversize \
             response with the ENCODED-BODY origin — the wire cap, applied before any \
             decode. A Timeout means the cap did not fire and the budget ended the \
             call instead. Got {outcome:?}",
            );
            drop(fanout);
        })
        .await;
    }

    /// THE PER-CALL DEADLINE, as the two drives differ on the wire.
    ///
    /// `to_outbound` sets ONE absolute instant on each outbound request
    /// and the wave drive is the only drive that does so; the per-call
    /// bridge sends a request carrying no deadline. Both still answer
    /// `Timeout(Bidder)` — the bridge's own timer arm sees to that — so
    /// the per-drive difference is not visible in the verdict, and
    /// asserting only the verdict would pass against a drive that had
    /// stopped propagating anything.
    ///
    /// This row gives the call 50 ms against a 30-second auction deadline
    /// and requires cancellation cleanup. The one-call batch cutoff equals
    /// that call's deadline, so this row alone cannot distinguish transport
    /// request deadlines from batch-cutoff enforcement. The distinct-deadline
    /// sibling test proves individual slots expire while others remain live.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_carries_its_composed_deadline_to_the_wire() {
        with_local_scheduler(async {
            let fixture = start_cancellation_fixture().await;
            let fanout = wave_fanout();
            let started = MonotonicInstant::now();

            let completed = fanout
                .send_all(
                    vec![call_to(fixture.fixture.port, CALL_TIMEOUT)],
                    deadline(),
                )
                .await;
            let elapsed = MonotonicInstant::now().saturating_duration_since(started);
            assert_deadline_connection_closed(fixture).await;
            let results = completed.expect("send_all itself succeeds");

            assert!(
                matches!(
                    results.first().map(|row| &row.result),
                    Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
                ),
                "the per-call budget is the earlier of the two, so it selects Bidder; got {:?}",
                results.first().map(|row| &row.result),
            );
            // This generous bound separates call/batch expiry from the 30-second
            // auction cutoff; it is not a measurement of exact 50 ms enforcement.
            assert!(
                elapsed < GENEROUS_DEADLINE / 2,
                "the call returned after {elapsed:?}, which is not the per-call budget of \
             {CALL_TIMEOUT:?} — the composed per-call deadline did not reach the wire",
            );
            assert_eq!(results.len(), 1, "one call in, one result out");
            drop(fanout);
        })
        .await;
    }

    /// THE PER-CALL CAP, which only the wave drive applies.
    ///
    /// `narrow_to_call` takes the tighter of the envelope's decoded cap
    /// and this call's own `max_decoded_bytes`. The per-call bridge
    /// sends a request carrying no response cap at all, so this row
    /// fails against it: with the wave drive swapped for the bridge the
    /// same call answers `Other("transport error")` instead.
    ///
    /// The cap here is BELOW the envelope's transport cap, so the
    /// narrowed decoded cap has to be what refuses the body — if the
    /// transport cap bound first the origin would be `EncodedBody`, and
    /// the assertion names the origin rather than just "refused".
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_narrows_the_response_cap_to_the_call_on_the_wire() {
        with_local_scheduler(async {
            let fixture = start_fixture(ResponseMode::OversizeChunkedBody).await;
            let fanout = wave_fanout();

            let results = fanout
                .send_all(
                    vec![call_capped(fixture.port, Duration::from_secs(10), 1024)],
                    deadline(),
                )
                .await
                .expect("send_all itself succeeds");
            let outcome = results.first().map(|row| &row.result);
            assert!(
                matches!(
                    outcome,
                    Some(Err(FanoutError::OversizeResponse(
                        OversizeOrigin::DecodedBody
                    ))),
                ),
                "a per-call cap of 1024 bytes, tighter than the envelope's, must narrow the \
             decoded cap and refuse this body with the DECODED-BODY origin. An \
             EncodedBody origin means the envelope's transport cap bound first and the \
             per-call narrowing was not applied; a transport error means the per-call \
             bridge ran instead of the wave drive. Got {outcome:?}",
            );
            drop(fanout);
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_preserves_sibling_deadlines_and_original_slots_on_the_wire() {
        with_local_scheduler(async {
            let short = start_fixture(ResponseMode::PromisedBodyNeverArrives).await;
            let healthy =
                start_fixture(ResponseMode::CompleteAfterDrop(Arc::clone(&short.outcomes))).await;
            let cutoff_anchor = start_fixture(ResponseMode::Complete).await;
            let gate = StreamGate::new(3);
            let fanout = wave_fanout_with_gate(Arc::clone(&gate));
            let short_timeout = Duration::from_secs(1);
            let healthy_timeout = Duration::from_secs(3);
            let cutoff_timeout = Duration::from_secs(4);
            assert!(short_timeout < healthy_timeout && healthy_timeout < cutoff_timeout);

            // The third slot sets the batch cutoff strictly after the healthy
            // slot's deadline, without keeping another slow connection alive.
            // Arrival order is anchor, short timeout, healthy, not input order.
            let send = timeout(
                READ_TIMEOUT,
                fanout.send_all(
                    vec![
                        call_to(healthy.port, healthy_timeout),
                        call_to(short.port, short_timeout),
                        call_to(cutoff_anchor.port, cutoff_timeout),
                    ],
                    deadline(),
                ),
            );
            let observe_overlap = async {
                timeout(short_timeout, async {
                    while [&healthy, &short, &cutoff_anchor]
                        .iter()
                        .any(|fixture| fixture.requests_seen.load(Ordering::SeqCst) == 0)
                    {
                        sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .expect("all siblings reach TCP before the short deadline");
                assert!(
                    short
                        .outcomes
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner)
                        .is_empty(),
                    "the short connection is still pending when the healthy request arrives",
                );
                assert_eq!(
                    gate.in_flight(),
                    3,
                    "one full concurrent wave holds the gate"
                );
                assert_eq!(gate.free(), 0, "the wave saturates the stream gate");
            };
            let (completed, ()) = tokio::join!(send, observe_overlap);
            let results = completed
                .expect("the wave returns before its observation cutoff")
                .expect("send_all itself succeeds");
            assert_eq!(results.len(), 3, "every original slot is present");
            let mut slots = results.into_iter();
            let healthy_slot = slots.next().expect("healthy slot at original index zero");
            let short_slot = slots.next().expect("short slot at original index one");
            let anchor_slot = slots.next().expect("anchor slot at original index two");
            assert!(
                matches!(
                    short_slot.result,
                    Err(FanoutError::Timeout(TimeoutCause::Bidder))
                ),
                "only the short slot expires at its own deadline: {:?}",
                short_slot.result,
            );
            assert_eq!(
                healthy_slot
                    .result
                    .expect("the healthy sibling survives the short deadline")
                    .into_body(),
                healthy_sibling_body(),
                "the late healthy response remains at its original index",
            );
            assert_eq!(
                anchor_slot
                    .result
                    .expect("the fast anchor succeeds")
                    .into_body(),
                RESPONSE_BODY.as_bytes(),
                "the first completion remains at its original index",
            );
            assert_eq!(
                wait_for_outcome(&short, READ_TIMEOUT).await,
                vec![ConnOutcome::ClientDrop],
                "short-slot expiry cancels its real transport while the sibling succeeds",
            );
            assert_healthy_wave_reuses_gate(&fanout, &gate).await;
        })
        .await;
    }

    fn gzip_call_to(port: u16, max_transport_bytes: u64, max_decoded_bytes: u64) -> BidderCall {
        let mut call = call_capped(port, Duration::from_secs(3), max_decoded_bytes);
        call.max_transport_bytes = max_transport_bytes;
        call
    }

    fn gzip_decoded_body() -> Vec<u8> {
        let mut body = RESPONSE_BODY.as_bytes().to_vec();
        body.resize(RESPONSE_BODY.len().saturating_add(GZIP_PADDING_BYTES), b' ');
        body
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_decodes_gzip_at_both_exact_caps_on_the_wire() {
        with_local_scheduler(async {
            let fixture = start_fixture(ResponseMode::GzipChunkedBody).await;
            let gate = StreamGate::new(1);
            let fanout = wave_fanout_with_gate(Arc::clone(&gate));
            let expected = gzip_decoded_body();
            let results = timeout(
                READ_TIMEOUT,
                fanout.send_all(
                    vec![gzip_call_to(
                        fixture.port,
                        u64::try_from(GZIP_BODY.len()).expect("encoded length fits"),
                        u64::try_from(expected.len()).expect("decoded length fits"),
                    )],
                    deadline(),
                ),
            )
            .await
            .expect("the gzip read finishes")
            .expect("send_all itself succeeds");
            assert_eq!(results.len(), 1);
            let response = results
                .into_iter()
                .next()
                .expect("one gzip response")
                .result
                .expect("both exact caps admit the complete gzip member");
            assert!(
                response
                    .headers()
                    .iter()
                    .all(|(name, _value)| !name.eq_ignore_ascii_case("content-encoding")),
                "the successfully decoded response is no longer marked gzip",
            );
            assert_eq!(response.into_body(), expected);
            assert_healthy_wave_reuses_gate(&fanout, &gate).await;
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_caps_compressed_bytes_before_decoding_on_the_wire() {
        with_local_scheduler(async {
        let fixture = start_fixture(ResponseMode::GzipChunkedBody).await;
        let gate = StreamGate::new(1);
        let fanout = wave_fanout_with_gate(Arc::clone(&gate));
        let results = timeout(
            READ_TIMEOUT,
            fanout.send_all(
                vec![gzip_call_to(
                    fixture.port,
                    u64::try_from(GZIP_BODY.len() - 1).expect("encoded cap fits"),
                    u64::try_from(gzip_decoded_body().len()).expect("decoded cap fits"),
                )],
                deadline(),
            ),
        )
        .await
        .expect("the compressed cap stops the read")
        .expect("send_all itself succeeds");
        assert_eq!(results.len(), 1);
        assert!(
            matches!(
                results.first().map(|row| &row.result),
                Some(Err(FanoutError::OversizeResponse(
                    OversizeOrigin::EncodedBody
                ))),
            ),
            "a chunked gzip member one encoded byte over cap is refused before decode: {results:?}",
        );
        assert_healthy_wave_reuses_gate(&fanout, &gate).await;
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_caps_gzip_expansion_on_the_wire() {
        with_local_scheduler(async {
            let fixture = start_fixture(ResponseMode::GzipChunkedBody).await;
            let gate = StreamGate::new(1);
            let fanout = wave_fanout_with_gate(Arc::clone(&gate));
            let decoded_cap =
                u64::try_from(gzip_decoded_body().len() - 1).expect("decoded cap fits");
            assert!(u64::try_from(GZIP_BODY.len()).expect("encoded length fits") < decoded_cap);
            let results = timeout(
                READ_TIMEOUT,
                fanout.send_all(
                    vec![gzip_call_to(fixture.port, TRANSPORT_CAP, decoded_cap)],
                    deadline(),
                ),
            )
            .await
            .expect("the decoded cap stops expansion")
            .expect("send_all itself succeeds");
            assert_eq!(results.len(), 1);
            assert!(
                matches!(
                    results.first().map(|row| &row.result),
                    Some(Err(FanoutError::OversizeResponse(
                        OversizeOrigin::DecodedBody
                    ))),
                ),
                "a small gzip member expanding one decoded byte over cap is refused: {results:?}",
            );
            assert_healthy_wave_reuses_gate(&fanout, &gate).await;
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_wave_drive_waits_for_gzip_eof_and_refuses_a_missing_trailer_on_the_wire() {
        with_local_scheduler(async {
            let prefix_sent = Arc::new(AtomicBool::new(false));
            let release_eof = Arc::new(AtomicBool::new(false));
            let fixture = start_fixture(ResponseMode::GzipLateEof {
                prefix_sent: Arc::clone(&prefix_sent),
                release_eof: Arc::clone(&release_eof),
            })
            .await;
            let healthy = start_fixture(ResponseMode::Complete).await;
            let gate = StreamGate::new(2);
            let fanout = wave_fanout_with_gate(Arc::clone(&gate));
            let published = AtomicBool::new(false);
            let send = async {
                let results = timeout(
                    READ_TIMEOUT,
                    fanout.send_all(
                        vec![
                            call_to(fixture.port, Duration::from_secs(3)),
                            call_to(healthy.port, Duration::from_secs(4)),
                        ],
                        deadline(),
                    ),
                )
                .await
                .expect("the late EOF releases the read")
                .expect("a codec failure remains positional");
                published.store(true, Ordering::SeqCst);
                results
            };
            let observe_pending_eof = async {
                wait_for_flag(&prefix_sent).await;
                sleep(Duration::from_millis(100)).await;
                assert!(
                    !published.load(Ordering::SeqCst),
                    "decoded payload and a healthy sibling cannot publish before codec EOF",
                );
                assert_eq!(gate.in_flight(), 2, "the pending EOF keeps the wave lease");
                assert_eq!(gate.free(), 0);
                release_eof.store(true, Ordering::SeqCst);
            };
            let (results, ()) = tokio::join!(send, observe_pending_eof);
            assert_eq!(results.len(), 2);
            let mut slots = results.into_iter();
            let failed = slots.next().expect("the truncated gzip slot");
            assert!(
                matches!(failed.result, Err(FanoutError::BadServerResponse)),
                "clean HTTP EOF without a gzip trailer refuses all partial decoded bytes: {:?}",
                failed.result,
            );
            assert_eq!(
                slots
                    .next()
                    .expect("the healthy sibling slot")
                    .result
                    .expect("the codec failure does not rewrite its healthy sibling")
                    .into_body(),
                RESPONSE_BODY.as_bytes(),
            );
            assert_healthy_wave_reuses_gate(&fanout, &gate).await;
        })
        .await;
    }

    /// The fallback drive still behaves, so the rows above are not the
    /// only evidence that this fixture works.
    ///
    /// Kept because `EdgeZeroFanout::new` IS reachable — the handler's
    /// no-boot-envelope arm — and a change that broke only the bridge
    /// would otherwise go unnoticed here.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_per_call_bridge_still_cancels_over_the_same_fixture() {
        with_local_scheduler(async {
            let fixture = start_cancellation_fixture().await;
            let fanout = bridge_fanout();

            let completed = fanout
                .send_all(
                    vec![call_to(fixture.fixture.port, CALL_TIMEOUT)],
                    deadline(),
                )
                .await;
            assert_deadline_connection_closed(fixture).await;
            let results = completed.expect("send_all itself succeeds");
            assert!(
                matches!(
                    results.first().map(|row| &row.result),
                    Some(Err(FanoutError::Timeout(TimeoutCause::Bidder))),
                ),
                "the bridge classifies the same way, got {:?}",
                results.first().map(|row| &row.result),
            );

            assert_post_headers_cancellation(&fanout).await;
            drop(fanout);
        })
        .await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn original_content_type_controls_safe_projection_over_real_transports() {
        with_local_scheduler(async {
            use auctioneer_bidder::{DebugBodySchema, DebugBodyStatus};
            use serde_json::{Value, json};

            let body = serde_json::to_vec(&json!({"id":"private-wire-id", "seatbid":[]})).unwrap();
            let cases: &[(&[&[u8]], DebugBodyStatus)] = &[
                (&[b"application/json"], DebugBodyStatus::Captured),
                (
                    &[b"Application/JSON; charset=utf-8"],
                    DebugBodyStatus::Captured,
                ),
                (&[], DebugBodyStatus::OmittedType),
                (
                    &[b"application/json", b"application/json"],
                    DebugBodyStatus::OmittedType,
                ),
                (
                    &[b"application/json", b"text/plain"],
                    DebugBodyStatus::OmittedType,
                ),
                (
                    &[b"application/json", b"\xff"],
                    DebugBodyStatus::OmittedType,
                ),
                (
                    &[b"application/json, application/json"],
                    DebugBodyStatus::OmittedType,
                ),
                (&[b"text/html"], DebugBodyStatus::OmittedType),
                (
                    &[b"application/json; note=\"\xc3\xa9\""],
                    DebugBodyStatus::OmittedType,
                ),
            ];
            for (drive, fanout) in [("wave", wave_fanout()), ("bridge", bridge_fanout())] {
                for (values, expected) in cases {
                    let mut wire = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\n", body.len())
                        .into_bytes();
                    for value in *values {
                        wire.extend_from_slice(b"content-type: ");
                        wire.extend_from_slice(value);
                        wire.extend_from_slice(b"\r\n");
                    }
                    wire.extend_from_slice(b"\r\n");
                    wire.extend_from_slice(&body);
                    let fixture = start_fixture(ResponseMode::RawComplete(wire)).await;
                    let mut results = timeout(
                        READ_TIMEOUT,
                        fanout.send_all(
                            vec![call_to(fixture.port, Duration::from_secs(3))],
                            deadline(),
                        ),
                    )
                    .await
                    .unwrap()
                    .unwrap();
                    assert_eq!(results.len(), 1, "{drive}");
                    let response = results
                        .pop()
                        .unwrap()
                        .result
                        .unwrap()
                        .within_node_budget(wave_limits().max_parsed_nodes);
                    assert!(
                        response.body_equals(&body),
                        "{drive}: ordinary body delivery unchanged"
                    );
                    let projection = response.project_debug_body(DebugBodySchema::OpenRtb);
                    assert_eq!(
                        projection.body_status,
                        *expected,
                        "{drive}, {} fields",
                        values.len()
                    );
                    if *expected == DebugBodyStatus::Captured {
                        assert_eq!(
                            serde_json::from_str::<Value>(&projection.body.unwrap()).unwrap(),
                            json!({"id":"[REDACTED]"}),
                        );
                    } else {
                        assert_eq!(projection.body, None, "{drive}");
                    }
                    assert_eq!(fixture.requests_seen.load(Ordering::SeqCst), 1, "{drive}");
                }
            }
        })
        .await;
    }
}
