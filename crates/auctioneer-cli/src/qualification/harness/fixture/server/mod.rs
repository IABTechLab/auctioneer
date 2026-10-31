use super::super::{HarnessError, Result, io};
use super::{Control, Options, ProviderMode, request_id};
use bytes::Bytes;
use futures::future::{Either, pending, select};
use http::{
    Method, Request, Response, StatusCode,
    header::{CONNECTION, CONTENT_ENCODING, CONTENT_TYPE, HeaderValue},
    uri::PathAndQuery,
};
use http_body_util::{BodyExt as _, Full, LengthLimitError, Limited, combinators::BoxBody};
use hyper::{body::Incoming, server::conn::http1, service::service_fn};
use hyper_util::rt::{TokioIo, TokioTimer};
use rustls::{
    ServerConfig,
    crypto::aws_lc_rs::default_provider,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject as _},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::convert::Infallible;
use std::io::{Error as IoError, IoSlice, Result as IoResult};
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::result::Result as StdResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::{TcpListener, TcpStream},
    sync::{Semaphore, oneshot},
    task::{JoinHandle, JoinSet, yield_now},
    time::{sleep, timeout},
};
use tokio_rustls::TlsAcceptor;

mod state;
use state::{Completion, Kind, State};

type Shared = Arc<Mutex<State>>;
type Body = BoxBody<Bytes, Infallible>;

pub(super) struct Server {
    pub port: u16,
    pub lifecycle: Lifecycle,
}

pub(super) struct Lifecycle {
    state: Shared,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<()>>>,
}

impl Server {
    pub async fn start(options: Options) -> Result<Self> {
        let state = State::new(&options)?;
        let tls = tls_config(&options)?;
        let port = match options {
            Options::Native { port, .. } | Options::Provider { port, .. } => port,
        };
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .await
            .map_err(|_error| HarnessError::new("fixture_failed"))?;
        let bound_port = listener
            .local_addr()
            .map_err(|_error| HarnessError::new("fixture_failed"))?
            .port();
        let shared = Arc::new(Mutex::new(state));
        let (shutdown, cancelled) = oneshot::channel();
        let task_state = Arc::clone(&shared);
        let task = tokio::spawn(async move { listen(listener, task_state, tls, cancelled).await });
        Ok(Self {
            port: bound_port,
            lifecycle: Lifecycle {
                state: shared,
                shutdown: Some(shutdown),
                task: Some(task),
            },
        })
    }

    pub async fn command(&mut self, control: Control) -> Result<Value> {
        yield_now().await;
        self.lifecycle
            .state
            .lock()
            .map_err(|_error| HarnessError::new("fixture_failed"))?
            .command(control)
    }

    pub async fn stop(mut self) -> Result<()> {
        if let Some(shutdown) = self.lifecycle.shutdown.take() {
            let _sent = shutdown.send(());
        }
        if let Some(task) = self.lifecycle.task.take() {
            task.await
                .map_err(|_error| HarnessError::new("fixture_failed"))??;
        }
        Ok(())
    }

    pub async fn completed(&mut self) -> Result<()> {
        let task = self
            .lifecycle
            .task
            .as_mut()
            .ok_or(HarnessError::new("fixture_failed"))?;
        let result = task
            .await
            .map_err(|_error| HarnessError::new("fixture_failed"))?;
        self.lifecycle.task.take();
        result
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for Server {
    fn drop(&mut self) {
        if let Some(shutdown) = self.lifecycle.shutdown.take() {
            let _sent = shutdown.send(());
        }
        // The listener owns the JoinSet and remains responsible for draining it.
        // Explicit stop is the awaited completion fence, including after watchdog.
    }
}

fn tls_config(options: &Options) -> Result<Option<TlsAcceptor>> {
    let Options::Provider {
        key_file: Some(key_file),
        cert_file: Some(cert_file),
        ..
    } = options
    else {
        return Ok(None);
    };
    let key_bytes = io::read_bytes(key_file, 64 << 10_u32)
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    let cert_bytes = io::read_bytes(cert_file, 64 << 10_u32)
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    let key = PrivateKeyDer::from_pem_slice(&key_bytes)
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    let certs = CertificateDer::pem_slice_iter(&cert_bytes)
        .collect::<StdResult<Vec<_>, _>>()
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    if certs.is_empty() {
        return Err(HarnessError::new("fixture_failed"));
    }
    // Match the SDK's backend explicitly, independent of process-wide defaults.
    let config = ServerConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|_error| HarnessError::new("fixture_failed"))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    Ok(Some(TlsAcceptor::from(Arc::new(config))))
}

async fn listen(
    listener: TcpListener,
    state: Shared,
    tls: Option<TlsAcceptor>,
    cancelled: oneshot::Receiver<()>,
) -> Result<()> {
    let (max_sockets, watchdog) = {
        let current = state
            .lock()
            .map_err(|_error| HarnessError::new("fixture_failed"))?;
        let (kind, _body_cap, sockets) = current.limits();
        (
            sockets,
            if matches!(kind, Kind::Native) {
                1_200_000_u64
            } else {
                30_000_u64
            },
        )
    };
    let slots = Arc::new(Semaphore::new(usize::from(max_sockets)));
    let mut tasks = JoinSet::new();
    let execute = accept_connections(listener, Arc::clone(&state), tls, slots, &mut tasks);
    let stopped = select(
        Box::pin(cancelled),
        Box::pin(sleep(Duration::from_millis(watchdog))),
    );
    let outcome = match select(Box::pin(stopped), Box::pin(execute)).await {
        Either::Left((_stopped, _accepting)) => Ok(()),
        Either::Right((result, _stopped)) => result,
    };
    tasks.abort_all();
    let mut failed = match state.lock() {
        Ok(mut current) => {
            current.stop();
            false
        }
        Err(poisoned) => {
            poisoned.into_inner().stop();
            true
        }
    };
    while let Some(completion) = tasks.join_next().await {
        if completion.is_err_and(|error| !error.is_cancelled()) {
            failed = true;
        }
    }
    if failed {
        return Err(HarnessError::new("fixture_failed"));
    }
    outcome
}

async fn accept_connections(
    listener: TcpListener,
    state: Shared,
    tls: Option<TlsAcceptor>,
    slots: Arc<Semaphore>,
    tasks: &mut JoinSet<()>,
) -> Result<()> {
    loop {
        while let Some(completion) = tasks.try_join_next() {
            completion.map_err(|_error| HarnessError::new("fixture_failed"))?;
        }
        let accepted = if tasks.is_empty() {
            listener.accept().await
        } else {
            match select(Box::pin(tasks.join_next()), Box::pin(listener.accept())).await {
                Either::Left((completion, _accepting)) => {
                    if let Some(result) = completion {
                        result.map_err(|_error| HarnessError::new("fixture_failed"))?;
                    }
                    continue;
                }
                Either::Right((result, _joining)) => result,
            }
        };
        let (stream, _address) = accepted.map_err(|_error| HarnessError::new("fixture_failed"))?;
        let Ok(slot) = Arc::clone(&slots).try_acquire_owned() else {
            drop(stream);
            continue;
        };
        let connection_state = Arc::clone(&state);
        let acceptor = tls.clone();
        let connection_deadline = connection_deadline(&connection_state)?;
        tasks.spawn(async move {
            let _slot = slot;
            // Includes TLS handshake, head, body, delay, stall and output flushing.
            let _elapsed = timeout(
                connection_deadline,
                serve(stream, connection_state, acceptor),
            )
            .await;
        });
    }
}

fn connection_deadline(shared: &Shared) -> Result<Duration> {
    let state = shared
        .lock()
        .map_err(|_error| HarnessError::new("fixture_failed"))?;
    if !state.running() {
        return Err(HarnessError::new("fixture_failed"));
    }
    // Only fixtures with an installed private test barrier opt into longer holds.
    #[cfg(test)]
    if state.test_hold().is_some() {
        return Ok(Duration::from_secs(30));
    }
    Ok(Duration::from_secs(3))
}

#[cfg(test)]
async fn test_hold(connection: &Connection, native_id: Option<u32>) -> Result<()> {
    let barrier = connection
        .state
        .lock()
        .map_err(|_error| HarnessError::new("fixture_failed"))?
        .test_hold();
    if let Some(hold) = barrier {
        hold.wait(native_id).await?;
    }
    Ok(())
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

struct Connection {
    state: Shared,
    completion: Mutex<Option<Completion>>,
    consumed: Arc<AtomicBool>,
    flushed: AtomicBool,
}

struct Guard(Arc<Connection>);
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for Guard {
    fn drop(&mut self) {
        // No guard is dropped while either mutex is held by a request future.
        let completion = self
            .0
            .completion
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(record) = completion {
            self.0
                .state
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .finish(&record, self.0.flushed.load(Ordering::Acquire));
        }
    }
}

struct TrackedIo {
    stream: Box<dyn Stream>,
    connection: Arc<Connection>,
}
impl AsyncRead for TrackedIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<IoResult<()>> {
        Pin::new(&mut self.stream).poll_read(cx, buf)
    }
}
impl AsyncWrite for TrackedIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.stream).poll_write(cx, buf)
    }
    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<IoResult<usize>> {
        Pin::new(&mut self.stream).poll_write_vectored(cx, bufs)
    }
    fn is_write_vectored(&self) -> bool {
        self.stream.is_write_vectored()
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        let flushed = Pin::new(&mut self.stream).poll_flush(cx);
        if matches!(flushed, Poll::Ready(Ok(())))
            && self.connection.consumed.load(Ordering::Acquire)
        {
            self.connection.flushed.store(true, Ordering::Release);
        }
        flushed
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

async fn serve(stream: TcpStream, state: Shared, tls: Option<TlsAcceptor>) {
    let socket: Box<dyn Stream> = if let Some(acceptor) = tls {
        let Ok(secured) = acceptor.accept(stream).await else {
            return;
        };
        Box::new(secured)
    } else {
        Box::new(stream)
    };
    let head_seconds = {
        let Ok(current) = state.lock() else {
            return;
        };
        if matches!(current.limits().0, Kind::Native) {
            1
        } else {
            2
        }
    };
    let connection = Arc::new(Connection {
        state,
        completion: Mutex::new(None),
        consumed: Arc::new(AtomicBool::new(false)),
        flushed: AtomicBool::new(false),
    });
    let guard = Guard(Arc::clone(&connection));
    let tracked = TrackedIo {
        stream: socket,
        connection: Arc::clone(&connection),
    };
    let service = service_fn(move |inbound| {
        let context = Arc::clone(&connection);
        async move {
            handle(inbound, &context)
                .await
                .map_err(|_error| IoError::other("fixture_failed"))
        }
    });
    // Hyper requires an >=8192-byte parser buffer. The typed head check below
    // enforces 4096 bytes (including the request line) before body collection.
    let _closed = http1::Builder::new()
        .keep_alive(false)
        .max_headers(32)
        .max_buf_size(8192)
        .timer(TokioTimer::new())
        .header_read_timeout(Duration::from_secs(head_seconds))
        .serve_connection(TokioIo::new(tracked), service)
        .await;
    drop(guard);
}

fn bounded_head(request: &Request<Incoming>) -> bool {
    let mut size = request
        .method()
        .as_str()
        .len()
        .checked_add(request.uri().to_string().len())
        .and_then(|bytes| bytes.checked_add(14));
    for (name, value) in request.headers() {
        size = size
            .and_then(|bytes| bytes.checked_add(name.as_str().len()))
            .and_then(|bytes| bytes.checked_add(value.as_bytes().len()))
            .and_then(|bytes| bytes.checked_add(4));
    }
    request.headers().len() <= 32 && size.is_some_and(|bytes| bytes <= 4096)
}

fn reply(
    status: StatusCode,
    bytes: Bytes,
    gzip: bool,
    consumed: Arc<AtomicBool>,
) -> Response<Body> {
    let body = Full::new(bytes)
        .map_frame(move |frame| {
            consumed.store(true, Ordering::Release);
            frame
        })
        .boxed();
    let mut response = Response::new(body);
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONNECTION, HeaderValue::from_static("close"));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    if gzip {
        response
            .headers_mut()
            .insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
    }
    response
}

fn refuse(connection: &Connection, status: StatusCode) -> Result<Response<Body>> {
    connection
        .state
        .lock()
        .map_err(|_error| HarnessError::new("fixture_failed"))?
        .reject();
    Ok(reply(
        status,
        Bytes::new(),
        false,
        Arc::clone(&connection.consumed),
    ))
}

#[derive(Deserialize)]
struct Auction {
    id: String,
    imp: Vec<Value>,
}

async fn handle(inbound: Request<Incoming>, connection: &Connection) -> Result<Response<Body>> {
    let (kind, body_cap, offered) = {
        let mut current = connection
            .state
            .lock()
            .map_err(|_error| HarnessError::new("fixture_failed"))?;
        let (kind, cap, _sockets) = current.limits();
        (kind, cap, current.offer())
    };
    if !offered {
        return refuse(
            connection,
            if matches!(kind, Kind::Native) {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::NOT_FOUND
            },
        );
    }
    if !bounded_head(&inbound) {
        return refuse(connection, StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE);
    }
    if inbound.method() != Method::POST
        || inbound.uri().path_and_query().map(PathAndQuery::as_str) != Some("/openrtb2/auction")
    {
        return refuse(connection, StatusCode::NOT_FOUND);
    }
    let collected = match timeout(
        Duration::from_secs(2),
        Limited::new(inbound.into_body(), body_cap).collect(),
    )
    .await
    {
        Ok(Ok(body)) => body.to_bytes(),
        Ok(Err(error)) => {
            return refuse(
                connection,
                if error.is::<LengthLimitError>() {
                    StatusCode::PAYLOAD_TOO_LARGE
                } else {
                    StatusCode::BAD_REQUEST
                },
            );
        }
        Err(_error) => return refuse(connection, StatusCode::REQUEST_TIMEOUT),
    };
    if auctioneer_runtime::count_json_nodes(&collected, 1 << 16_u32).is_err() {
        return refuse(connection, StatusCode::BAD_REQUEST);
    }
    // Ignored fields in the typed request must not bypass serde's depth cap.
    if serde_json::from_slice::<Value>(&collected).is_err() {
        return refuse(connection, StatusCode::BAD_REQUEST);
    }
    let Ok(auction) = serde_json::from_slice::<Auction>(&collected) else {
        return refuse(connection, StatusCode::BAD_REQUEST);
    };
    match kind {
        Kind::Provider(scenario) => provider_response(connection, auction, scenario).await,
        Kind::Native => native_response(connection, auction).await,
    }
}

async fn provider_response(
    connection: &Connection,
    auction: Auction,
    scenario: ProviderMode,
) -> Result<Response<Body>> {
    let candidate_id = auction.id.strip_prefix("handler-case-").and_then(|suffix| {
        let number = suffix.parse::<u8>().ok()?;
        ((1..=14).contains(&number) && suffix == number.to_string()).then_some(number)
    });
    let Some(case_id) = candidate_id else {
        return refuse(connection, StatusCode::BAD_REQUEST);
    };
    if auction.imp.len() != 1
        || auction
            .imp
            .first()
            .and_then(|imp| imp.get("id"))
            .and_then(Value::as_str)
            != Some("fixture-imp")
    {
        return refuse(connection, StatusCode::BAD_REQUEST);
    }
    connection
        .state
        .lock()
        .map_err(|_error| HarnessError::new("fixture_failed"))?
        .dispatch(case_id);
    #[cfg(test)]
    test_hold(connection, None).await?;
    if matches!(scenario, ProviderMode::Stall) {
        sleep(Duration::from_secs(2)).await;
        return Err(HarnessError::new("fixture_failed"));
    }
    let (status, body) = if matches!(scenario, ProviderMode::Failure) {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            json!({"error":"SYNTHETIC_PROVIDER_SECRET_CANARY"}),
        )
    } else {
        let mut value = json!({"id":auction.id,"cur":"USD","seatbid":[{"seat":"fake_provider","bid":[{
            "id":"fixture-bid","impid":"fixture-imp","price":1.25_f64,"adm":"<div>fixture</div>",
            "adomain":["fixture.invalid"],"mtype":1_u8}]}]});
        if matches!(scenario, ProviderMode::Canary) {
            value
                .as_object_mut()
                .ok_or(HarnessError::new("fixture_failed"))?
                .insert(
                    "ext".to_owned(),
                    json!({"private_canary":"SYNTHETIC_PROVIDER_SECRET_CANARY"}),
                );
        }
        (StatusCode::OK, value)
    };
    Ok(reply(
        status,
        Bytes::from(io::json_bytes(&body, 16 << 10_u32)?),
        false,
        Arc::clone(&connection.consumed),
    ))
}

async fn native_response(connection: &Connection, auction: Auction) -> Result<Response<Body>> {
    let Ok(id) = request_id(&auction.id) else {
        return refuse(connection, StatusCode::BAD_REQUEST);
    };
    if !(1..=99_999_999).contains(&id) {
        return refuse(connection, StatusCode::BAD_REQUEST);
    }
    let expanded = auction
        .imp
        .iter()
        .any(|imp| imp.pointer("/banner/ext/q2_fpd").and_then(Value::as_bool) == Some(true));
    let prepared = connection
        .state
        .lock()
        .map_err(|_error| HarnessError::new("fixture_failed"))?
        .begin(id, expanded);
    let Ok((completion, plan)) = prepared else {
        return refuse(connection, StatusCode::SERVICE_UNAVAILABLE);
    };
    *connection
        .completion
        .lock()
        .map_err(|_error| HarnessError::new("fixture_failed"))? = Some(completion);
    #[cfg(test)]
    test_hold(connection, Some(id)).await?;
    if matches!(plan.mode, super::Mode::Stall) {
        return pending().await;
    }
    if let Some(released) = plan.released {
        let _released = released.await;
    }
    if plan.delay_ms != 0 {
        sleep(Duration::from_millis(plan.delay_ms)).await;
    }
    Ok(reply(
        if matches!(plan.mode, super::Mode::Error) {
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::OK
        },
        plan.body,
        matches!(plan.mode, super::Mode::Gzip),
        Arc::clone(&connection.consumed),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::super::http::{Category, RequestSpec, Response, request as owned_request};
    use super::super::super::process::Supervisor;
    use super::super::super::{HarnessError, Result};
    use super::super::{Control, Options};
    use super::super::{Mode, ProviderMode};
    use super::{Connection, Guard, Server, State, connection_deadline};
    use bytes::Bytes;
    use http::{HeaderName, HeaderValue, Method};
    use rustls::crypto::aws_lc_rs::default_provider;
    use serde_json::Value;
    use serde_json::json;
    use std::ffi::OsString;
    use std::fs;
    use std::net::Ipv4Addr;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Semaphore;
    use tokio::time::{Instant, sleep, timeout};

    // Only colocated tests can install this barrier; HTTP and operator controls
    // cannot select it. Each waiter represents a still-live, dispatched request.
    pub(super) struct Hold {
        native_id: Option<u32>,
        gate: Semaphore,
        active: AtomicU32,
    }

    struct Held<'a>(&'a Hold);
    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Held<'_> {
        fn drop(&mut self) {
            let _previous = self.0.active.fetch_sub(1, Ordering::AcqRel);
        }
    }

    impl Hold {
        pub(super) async fn wait(&self, native_id: Option<u32>) -> Result<()> {
            if self.native_id.is_some() && self.native_id != native_id {
                return Ok(());
            }
            let _previous = self.active.fetch_add(1, Ordering::AcqRel);
            let _held = Held(self);
            let _permit = self
                .gate
                .acquire()
                .await
                .map_err(|_error| HarnessError::new("fixture_failed"))?;
            Ok(())
        }

        fn active(&self) -> u32 {
            self.active.load(Ordering::Acquire)
        }

        fn release(&self) {
            self.gate.add_permits(128);
        }
    }

    fn install_hold(server: &Server, native_id: Option<u32>) -> Arc<Hold> {
        let hold = Arc::new(Hold {
            native_id,
            gate: Semaphore::new(0),
            active: AtomicU32::new(0),
        });
        server
            .lifecycle
            .state
            .lock()
            .unwrap()
            .set_test_hold(Arc::clone(&hold));
        hold
    }

    async fn wait_held(hold: &Hold, active: u32) {
        let deadline = Instant::now().checked_add(Duration::from_secs(5)).unwrap();
        while hold.active() != active {
            assert!(Instant::now() < deadline);
            sleep(Duration::from_millis(2)).await;
        }
    }

    fn provider(scenario: ProviderMode, max_requests: u8) -> Options {
        Options::Provider {
            port: 0,
            scenario,
            max_requests,
            key_file: None,
            cert_file: None,
        }
    }

    fn native(max_requests: u32, max_sockets: u16) -> Options {
        Options::Native {
            port: 0,
            body_cap: 8192,
            max_requests,
            max_sockets,
        }
    }

    fn control(command: &str, settings: Value) -> Control {
        Control {
            id: 1,
            command: command.to_owned(),
            settings,
        }
    }

    fn settings(mode: Mode, delay_ms: u64) -> Value {
        json!({"mode":mode,"encoded_bytes":1024_u64,"decoded_bytes":2048_u64,
            "seed":7_u32,"delay_ms":delay_ms,"stall_after_calls":0_u32})
    }

    fn url(port: u16) -> String {
        format!("http://127.0.0.1:{port}/openrtb2/auction")
    }

    async fn request(url: &str, spec: RequestSpec) -> Result<Response> {
        let owner = Supervisor::new();
        let response = owned_request(url, spec, &owner).await;
        owner.cleanup().await?;
        response
    }

    fn post(body: &Value) -> RequestSpec {
        RequestSpec {
            method: Method::POST,
            body: Bytes::from(serde_json::to_vec(body).unwrap()),
            timeout_ms: 2500,
            inspect: true,
            ..RequestSpec::default()
        }
    }

    fn provider_body(case_id: u8) -> Value {
        json!({"id":format!("handler-case-{case_id}"),"imp":[{"id":"fixture-imp"}]})
    }

    fn native_body(id: u32) -> Value {
        json!({"id":format!("q{id}"),"imp":[]})
    }

    async fn stats(server: &mut Server) -> super::super::Stats {
        let result = server.command(control("stats", Value::Null)).await.unwrap();
        serde_json::from_value(result["stats"].clone()).unwrap()
    }

    async fn wait_active(server: &mut Server, active: u32) -> super::super::Stats {
        let deadline = Instant::now().checked_add(Duration::from_secs(5)).unwrap();
        loop {
            let current = stats(server).await;
            if current.current_active_calls == active {
                return current;
            }
            assert!(Instant::now() < deadline);
            sleep(Duration::from_millis(2)).await;
        }
    }

    #[tokio::test]
    async fn provider_failure_stall_and_canary_are_real_http_outcomes() {
        for scenario in [
            ProviderMode::Bid,
            ProviderMode::Failure,
            ProviderMode::Stall,
            ProviderMode::Canary,
        ] {
            let mut server = Server::start(provider(scenario, 2)).await.unwrap();
            let response = if matches!(scenario, ProviderMode::Stall) {
                let hold = install_hold(&server, None);
                let target = url(server.port);
                let stalled = tokio::spawn(async move {
                    request(
                        &target,
                        RequestSpec {
                            timeout_ms: 1500,
                            ..post(&provider_body(8))
                        },
                    )
                    .await
                    .unwrap()
                });
                wait_held(&hold, 1).await;
                hold.release();
                stalled.await.unwrap()
            } else {
                request(&url(server.port), post(&provider_body(8)))
                    .await
                    .unwrap()
            };
            match scenario {
                ProviderMode::Failure => {
                    assert_eq!(response.status, 503);
                    assert_eq!(response.category, Category::Rejected);
                }
                ProviderMode::Stall => assert_eq!(response.category, Category::Timeout),
                ProviderMode::Canary => {
                    let captured: Value = serde_json::from_slice(&response.body).unwrap();
                    assert_eq!(captured["seatbid"][0]["seat"], "fake_provider");
                    assert_eq!(
                        captured["ext"]["private_canary"],
                        "SYNTHETIC_PROVIDER_SECRET_CANARY"
                    );
                    assert_eq!(captured["seatbid"][0]["bid"][0]["id"], "fixture-bid");
                    assert_eq!(
                        captured["seatbid"][0]["bid"][0]["adm"],
                        "<div>fixture</div>"
                    );
                }
                ProviderMode::Bid => {
                    assert_eq!(response.category, Category::Success);
                    let captured: Value = serde_json::from_slice(&response.body).unwrap();
                    assert_eq!(captured["seatbid"][0]["seat"], "fake_provider");
                }
            }
            assert_eq!(
                server
                    .command(control("snapshot", Value::Null))
                    .await
                    .unwrap(),
                json!({"id":1_u32,"dispatches":1_u32,"case_id":8_u8,"rejected":0_u32})
            );
            server.stop().await.unwrap();
        }
    }

    #[tokio::test]
    async fn provider_rejects_bad_methods_paths_ids_shapes_and_caps_without_dispatch() {
        let mut server = Server::start(provider(ProviderMode::Bid, 8)).await.unwrap();
        for (path, spec, status) in [
            (
                "/openrtb2/auction",
                RequestSpec {
                    timeout_ms: 2500,
                    ..RequestSpec::default()
                },
                404_u16,
            ),
            ("/snapshot", post(&provider_body(1)), 404),
            (
                "/openrtb2/auction?scenario=canary",
                post(&provider_body(1)),
                404,
            ),
            ("/openrtb2/auction", post(&provider_body(15)), 400),
            (
                "/openrtb2/auction",
                post(&json!({"id":"handler-case-01","imp":[{"id":"fixture-imp"}]})),
                400,
            ),
            (
                "/openrtb2/auction",
                post(&json!({"id":"handler-case-1","imp":[]})),
                400,
            ),
            (
                "/openrtb2/auction",
                RequestSpec {
                    body: Bytes::from_static(b"{"),
                    ..post(&provider_body(1))
                },
                400,
            ),
            (
                "/openrtb2/auction",
                RequestSpec {
                    body: Bytes::from(vec![b'x'; 0x4001]),
                    ..post(&provider_body(1))
                },
                413,
            ),
        ] {
            let target = format!("http://127.0.0.1:{}{path}", server.port);
            assert_eq!(request(&target, spec).await.unwrap().status, status);
        }
        assert_eq!(
            request(&url(server.port), post(&provider_body(1)))
                .await
                .unwrap()
                .status,
            404
        );
        assert_eq!(
            server
                .command(control("snapshot", Value::Null))
                .await
                .unwrap(),
            json!({"id":1_u32,"dispatches":0_u32,"case_id":0_u8,"rejected":9_u32})
        );
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn parked_release_is_fifo_two_then_all_and_unholds_later_calls() {
        let mut server = Server::start(native(20, 8)).await.unwrap();
        server
            .command(control("configure", settings(Mode::Park, 0)))
            .await
            .unwrap();
        let mut tasks = Vec::new();
        for id in 1_u32..=4 {
            let target = url(server.port);
            tasks.push(tokio::spawn(async move {
                request(&target, post(&native_body(id))).await.unwrap()
            }));
            wait_active(&mut server, id).await;
        }
        assert_eq!(
            server
                .command(control("configure", settings(Mode::Plain, 0)))
                .await
                .unwrap_err(),
            HarnessError::new("fixture_busy")
        );
        server
            .command(control("release", json!({"all":false})))
            .await
            .unwrap();
        let partial = wait_active(&mut server, 2).await;
        assert_eq!(partial.completed, 2);
        for task in tasks.drain(..2) {
            assert_eq!(task.await.unwrap().category, Category::Success);
        }
        assert!(!tasks.first().unwrap().is_finished());
        server
            .command(control("release", json!({"all":true})))
            .await
            .unwrap();
        for task in tasks {
            assert_eq!(task.await.unwrap().category, Category::Success);
        }
        assert_eq!(
            request(&url(server.port), post(&native_body(5)))
                .await
                .unwrap()
                .category,
            Category::Success
        );
        let final_stats = wait_active(&mut server, 0).await;
        assert_eq!(final_stats.completed, 5);
        assert_eq!(final_stats.gzip_responses, 5);
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn cancel_mode_keeps_divisible_by_three_healthy_and_disconnects_delayed_work() {
        let mut server = Server::start(native(20, 4)).await.unwrap();
        server
            .command(control("configure", settings(Mode::Cancel, 500)))
            .await
            .unwrap();
        let hold = install_hold(&server, Some(1));
        let supervisor = Supervisor::new();
        let cancellation = supervisor.clone();
        let target = url(server.port);
        let cancelled = tokio::spawn(async move {
            owned_request(
                &target,
                RequestSpec {
                    timeout_ms: 20_000,
                    ..post(&native_body(1))
                },
                &cancellation,
            )
            .await
            .unwrap()
        });
        wait_held(&hold, 1).await;
        let dispatched = wait_active(&mut server, 1).await;
        assert_eq!(dispatched.calls, 1);
        assert_eq!(dispatched.completed, 0);
        let healthy = request(&url(server.port), post(&native_body(3)))
            .await
            .unwrap();
        assert_eq!(healthy.category, Category::Success);
        assert_eq!(hold.active(), 1);
        hold.release();
        supervisor.interrupt(130);
        assert_eq!(cancelled.await.unwrap().category, Category::Interrupted);
        wait_held(&hold, 0).await;
        let observed = wait_active(&mut server, 0).await;
        assert_eq!(observed.calls, 2);
        assert_eq!(observed.completed, 1);
        assert_eq!(observed.disconnected, 1);
        assert_eq!(observed.gzip_responses, 1);
        server.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn delay_applies_to_responses_and_stop_joins_held_connections_before_returning() {
        let mut server = Server::start(native(20, 4)).await.unwrap();
        server
            .command(control("configure", settings(Mode::Gzip, 40)))
            .await
            .unwrap();
        let started = Instant::now();
        assert_eq!(
            request(&url(server.port), post(&native_body(1)))
                .await
                .unwrap()
                .category,
            Category::Success
        );
        assert!(started.elapsed() >= Duration::from_millis(40));
        wait_active(&mut server, 0).await;
        server
            .command(control("configure", settings(Mode::Park, 0)))
            .await
            .unwrap();
        let target = url(server.port);
        let held =
            tokio::spawn(
                async move { request(&target, post(&native_body(700_001))).await.unwrap() },
            );
        let observed = wait_active(&mut server, 1).await;
        assert_eq!(observed.completed, 0);
        assert_eq!(observed.max_active_debug_auctions, 1);
        let shared = Arc::clone(&server.lifecycle.state);
        let port = server.port;
        server.stop().await.unwrap();
        let stopped_observation = {
            let stopped = shared.lock().unwrap();
            (
                stopped.running(),
                stopped.stats().current_active_calls,
                stopped.stats().disconnected,
                stopped.stats().completed,
            )
        };
        assert_eq!(stopped_observation, (false, 0_u32, 1_u32, 0_u32));
        assert_ne!(held.await.unwrap().category, Category::Success);
        TcpStream::connect((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap_err();
        let rebound = TcpListener::bind((Ipv4Addr::LOCALHOST, port))
            .await
            .unwrap();
        drop(rebound);
    }

    #[tokio::test]
    async fn both_socket_populations_are_bounded_without_unbounded_waiting_tasks() {
        for provider_mode in [false, true] {
            let mut server = Server::start(if provider_mode {
                provider(ProviderMode::Stall, 32)
            } else {
                native(20, 2)
            })
            .await
            .unwrap();
            if !provider_mode {
                server
                    .command(control("configure", settings(Mode::Park, 0)))
                    .await
                    .unwrap();
            }
            let capacity = if provider_mode { 8_u32 } else { 2_u32 };
            let hold = install_hold(&server, None);
            let mut tasks = Vec::new();
            for index in 1..=capacity {
                let target = url(server.port);
                tasks.push(tokio::spawn(async move {
                    request(
                        &target,
                        RequestSpec {
                            timeout_ms: 20_000,
                            ..post(&if provider_mode {
                                provider_body(1)
                            } else {
                                native_body(index)
                            })
                        },
                    )
                    .await
                    .unwrap()
                }));
                wait_held(&hold, index).await;
                if !provider_mode {
                    wait_active(&mut server, index).await;
                }
            }
            assert_eq!(hold.active(), capacity);
            assert!(tasks.iter().all(|task| !task.is_finished()));
            let refused = request(
                &url(server.port),
                post(&if provider_mode {
                    provider_body(1)
                } else {
                    native_body(3)
                }),
            )
            .await
            .unwrap();
            assert!(matches!(
                refused.category,
                Category::NetworkError | Category::Disconnect
            ));
            assert_eq!(hold.active(), capacity);
            assert!(tasks.iter().all(|task| !task.is_finished()));
            if provider_mode {
                assert_eq!(
                    server
                        .command(control("snapshot", Value::Null))
                        .await
                        .unwrap()["dispatches"],
                    capacity
                );
            } else {
                assert_eq!(stats(&mut server).await.calls, capacity);
            }
            let shared = Arc::clone(&server.lifecycle.state);
            server.stop().await.unwrap();
            assert_eq!(hold.active(), 0);
            let stopped_observation = {
                let stopped = shared.lock().unwrap();
                (
                    stopped.running(),
                    stopped.stats().current_active_calls,
                    stopped.stats().completed,
                    stopped.stats().disconnected,
                )
            };
            assert_eq!(
                stopped_observation,
                (
                    false,
                    0_u32,
                    0_u32,
                    if provider_mode { 0_u32 } else { capacity }
                )
            );
            for task in tasks {
                assert_ne!(task.await.unwrap().category, Category::Success);
            }
        }
    }

    #[tokio::test]
    async fn hold_release_preserves_the_actual_provider_bid() {
        let mut server = Server::start(provider(ProviderMode::Bid, 2)).await.unwrap();
        assert_eq!(
            connection_deadline(&server.lifecycle.state).unwrap(),
            Duration::from_secs(3)
        );
        let hold = install_hold(&server, None);
        assert_eq!(
            connection_deadline(&server.lifecycle.state).unwrap(),
            Duration::from_secs(30)
        );
        let target = url(server.port);
        let pending = tokio::spawn(async move {
            request(
                &target,
                RequestSpec {
                    timeout_ms: 20_000,
                    ..post(&provider_body(1))
                },
            )
            .await
            .unwrap()
        });
        wait_held(&hold, 1).await;
        assert!(!pending.is_finished());
        assert_eq!(
            server
                .command(control("snapshot", Value::Null))
                .await
                .unwrap()["dispatches"],
            1_u32
        );
        hold.release();
        let response = pending.await.unwrap();
        assert_eq!(response.category, Category::Success);
        let captured: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(captured["seatbid"][0]["bid"][0]["price"], 1.25_f64);
        assert_eq!(hold.active(), 0);
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn native_request_body_and_total_caps_refuse_without_changing_valid_counts() {
        let mut server = Server::start(Options::Native {
            port: 0,
            body_cap: 128,
            max_requests: 4,
            max_sockets: 4,
        })
        .await
        .unwrap();
        assert_eq!(
            request(&url(server.port), post(&native_body(1)))
                .await
                .unwrap()
                .status,
            200
        );
        assert_eq!(
            request(&url(server.port), post(&json!({"id":"qSECRET","imp":[]})))
                .await
                .unwrap()
                .status,
            400
        );
        assert_eq!(
            request(
                &url(server.port),
                RequestSpec {
                    body: Bytes::from(vec![b'x'; 129]),
                    ..post(&native_body(1))
                }
            )
            .await
            .unwrap()
            .status,
            413
        );
        assert_eq!(
            request(&url(server.port), post(&native_body(2)))
                .await
                .unwrap()
                .status,
            200
        );
        assert_eq!(
            request(&url(server.port), post(&native_body(3)))
                .await
                .unwrap()
                .status,
            503
        );
        assert_eq!(wait_active(&mut server, 0).await.calls, 2);
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn mixed_mode_observes_gzip_http_error_and_stall_independently() {
        let mut server = Server::start(native(20, 4)).await.unwrap();
        server
            .command(control("configure", settings(Mode::Mixed, 0)))
            .await
            .unwrap();
        for (id, category, status) in [
            (3_u32, Category::Success, 200_u16),
            (4, Category::HttpError, 500),
        ] {
            let response = request(&url(server.port), post(&native_body(id)))
                .await
                .unwrap();
            assert_eq!(response.category, category);
            assert_eq!(response.status, status);
        }
        wait_active(&mut server, 0).await;
        let hold = install_hold(&server, Some(5));
        let supervisor = Supervisor::new();
        let cancellation = supervisor.clone();
        let target = url(server.port);
        let stalled = tokio::spawn(async move {
            owned_request(
                &target,
                RequestSpec {
                    timeout_ms: 20_000,
                    ..post(&native_body(5))
                },
                &cancellation,
            )
            .await
            .unwrap()
        });
        wait_held(&hold, 1).await;
        assert_eq!(wait_active(&mut server, 1).await.stalled_reads, 1);
        hold.release();
        wait_held(&hold, 0).await;
        supervisor.interrupt(130);
        assert_eq!(stalled.await.unwrap().category, Category::Interrupted);
        wait_held(&hold, 0).await;
        let observed = wait_active(&mut server, 0).await;
        assert_eq!(observed.completed, 2);
        assert_eq!(observed.gzip_responses, 1);
        assert_eq!(observed.stalled_reads, 1);
        assert_eq!(observed.disconnected, 1);
        server.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
    }

    #[test]
    fn unique_seen_ids_are_bounded_and_dropped_guards_record_disconnect_not_completion() {
        let shared = Arc::new(Mutex::new(State::new(&native(1_000_000, 2)).unwrap()));
        for id in 1_u32..=10_000 {
            let (completion, _plan) = shared.lock().unwrap().begin(id, false).unwrap();
            let connection = Arc::new(Connection {
                state: Arc::clone(&shared),
                completion: Mutex::new(Some(completion)),
                consumed: Arc::new(AtomicBool::new(true)),
                flushed: AtomicBool::new(false),
            });
            drop(Guard(connection));
        }
        let mut current = shared.lock().unwrap();
        assert_eq!(
            current.begin(10_001, false).err().unwrap(),
            HarnessError::new("fixture_failed")
        );
        assert_eq!(current.stats().calls, 10_000);
        assert_eq!(current.stats().completed, 0);
        assert_eq!(current.stats().current_active_calls, 0);
        assert_eq!(current.stats().disconnected, 10_000);
        let (existing, _existing_plan) = current.begin(1, false).unwrap();
        current.finish(&existing, false);
        current
            .command(control("configure", settings(Mode::Gzip, 0)))
            .unwrap();
        let (fresh, _fresh_plan) = current.begin(10_001, false).unwrap();
        current.finish(&fresh, true);
        assert_eq!(current.stats().calls, 1);
        assert_eq!(current.stats().completed, 1);
    }

    #[tokio::test]
    async fn malformed_options_and_tls_files_fail_closed_with_fixed_error_codes() {
        let directory = tempfile::tempdir().unwrap();
        let key_file = directory.path().join("private-key");
        let cert_file = directory.path().join("private-cert");
        fs::write(&key_file, b"PRIVATE_KEY_CANARY").unwrap();
        fs::write(&cert_file, vec![b'x'; 0x0001_0001]).unwrap();
        for options in [
            native(0, 1),
            native(1_000_001, 1),
            native(1, 0),
            native(1, 129),
            Options::Native {
                port: 0,
                body_cap: 1 << 21_u32,
                max_requests: 1,
                max_sockets: 1,
            },
            provider(ProviderMode::Bid, 0),
            provider(ProviderMode::Bid, 33),
            Options::Provider {
                port: 0,
                scenario: ProviderMode::Bid,
                max_requests: 1,
                key_file: Some(key_file.clone()),
                cert_file: None,
            },
        ] {
            assert_eq!(
                Server::start(options).await.err().unwrap(),
                HarnessError::new("invalid_input")
            );
        }
        let options = Options::Provider {
            port: 0,
            scenario: ProviderMode::Bid,
            max_requests: 1,
            key_file: Some(key_file.clone()),
            cert_file: Some(cert_file.clone()),
        };
        assert_eq!(
            Server::start(options.clone()).await.err().unwrap(),
            HarnessError::new("fixture_failed")
        );
        fs::write(&cert_file, b"PRIVATE_CERT_CANARY").unwrap();
        assert_eq!(
            Server::start(options).await.err().unwrap(),
            HarnessError::new("fixture_failed")
        );
    }

    #[tokio::test]
    async fn typed_header_bytes_and_hyper_header_count_limits_reject_before_dispatch() {
        let mut server = Server::start(provider(ProviderMode::Bid, 8)).await.unwrap();
        let mut oversized = post(&provider_body(1));
        oversized.headers.insert(
            "x-fixture",
            HeaderValue::from_str(&"x".repeat(4096)).unwrap(),
        );
        assert_eq!(
            request(&url(server.port), oversized).await.unwrap().status,
            431
        );
        let mut excessive = post(&provider_body(1));
        for index in 0_u8..33 {
            excessive.headers.insert(
                HeaderName::from_bytes(format!("x-{index}").as_bytes()).unwrap(),
                HeaderValue::from_static("x"),
            );
        }
        let response = request(&url(server.port), excessive).await.unwrap();
        assert_ne!(response.category, Category::Success);
        assert_eq!(
            server
                .command(control("snapshot", Value::Null))
                .await
                .unwrap()["dispatches"],
            0_u32
        );
        assert_eq!(
            request(&url(server.port), post(&provider_body(14)))
                .await
                .unwrap()
                .status,
            200
        );
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn header_timeout_releases_connection_capacity_without_dispatch() {
        use tokio::io::AsyncReadExt as _;
        let mut server = Server::start(native(5, 1)).await.unwrap();
        let mut idle = TcpStream::connect((Ipv4Addr::LOCALHOST, server.port))
            .await
            .unwrap();
        let mut captured = Vec::new();
        timeout(Duration::from_millis(1800), idle.read_to_end(&mut captured))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stats(&mut server).await.calls, 0);
        assert_eq!(
            request(&url(server.port), post(&native_body(1)))
                .await
                .unwrap()
                .category,
            Category::Success
        );
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn provider_tls_serves_a_verified_bid_and_rejects_a_malformed_certificate() {
        use super::super::super::process::ProcessSpec;
        use rustls::{
            ClientConfig, RootCertStore,
            pki_types::{CertificateDer, pem::PemObject as _},
        };
        let directory = tempfile::tempdir().unwrap();
        let key_file = directory.path().join("key.pem");
        let cert_file = directory.path().join("cert.pem");
        let supervisor = Supervisor::new();
        let mut generation = ProcessSpec::new("/usr/bin/openssl");
        generation.args = [
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=fixture.invalid",
            "-addext",
            "subjectAltName=IP:127.0.0.1",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-addext",
            "extendedKeyUsage=serverAuth",
            "-keyout",
        ]
        .into_iter()
        .map(OsString::from)
        .collect();
        generation.args.push(key_file.as_os_str().to_owned());
        generation.args.push(OsString::from("-out"));
        generation.args.push(cert_file.as_os_str().to_owned());
        let generated = supervisor.run(generation).await;
        supervisor.cleanup().await.unwrap();
        generated.unwrap();
        let options = Options::Provider {
            port: 0,
            scenario: ProviderMode::Bid,
            max_requests: 2,
            key_file: Some(key_file),
            cert_file: Some(cert_file.clone()),
        };
        let mut server = Server::start(options.clone()).await.unwrap();
        let cert_bytes = fs::read(&cert_file).unwrap();
        let certificate = CertificateDer::from_pem_slice(&cert_bytes).unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(certificate).unwrap();
        let client = ClientConfig::builder_with_provider(Arc::new(default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let target = format!("https://127.0.0.1:{}/openrtb2/auction", server.port);
        let response = request(
            &target,
            RequestSpec {
                tls: Some(Arc::new(client)),
                ..post(&provider_body(1))
            },
        )
        .await
        .unwrap();
        assert_eq!(response.category, Category::Success);
        let captured: Value = serde_json::from_slice(&response.body).unwrap();
        assert_eq!(captured["seatbid"][0]["bid"][0]["price"], 1.25_f64);
        assert_eq!(
            server
                .command(control("snapshot", Value::Null))
                .await
                .unwrap()["dispatches"],
            1_u32
        );
        server.stop().await.unwrap();
        fs::write(&cert_file, b"MALFORMED_CERT_CANARY").unwrap();
        assert_eq!(
            Server::start(options).await.err().unwrap(),
            HarnessError::new("fixture_failed")
        );
    }

    #[tokio::test]
    async fn native_exact_coverage_probe_fpd_and_gzip_are_observed_not_inferred() {
        use flate2::read::GzDecoder;
        use std::io::Read as _;
        let mut server = Server::start(native(20, 4)).await.unwrap();
        server
            .command(control("configure", settings(Mode::Gzip, 0)))
            .await
            .unwrap();
        for id in [1_u32, 1, 2] {
            let body = json!({"id":format!("q{id}"),"imp":[{"banner":{"ext":{"q2_fpd":true}}}]});
            let response = request(&url(server.port), post(&body)).await.unwrap();
            assert_eq!(response.headers["content-encoding"], "gzip");
            assert_eq!(response.bytes, 1024);
            let mut decoded = Vec::new();
            GzDecoder::new(response.body.as_slice())
                .read_to_end(&mut decoded)
                .unwrap();
            assert_eq!(decoded.len(), 2048);
            serde_json::from_slice::<Value>(&decoded).unwrap();
        }
        let observed = wait_active(&mut server, 0).await;
        assert_eq!(observed.calls, 3);
        assert_eq!(observed.completed, 3);
        assert_eq!(observed.fpd_expansions, 3);
        assert_eq!(observed.max_encoded_bytes, 1024);
        assert_eq!(observed.max_decoded_bytes, 2048);
        assert_eq!(observed.max_calls_per_auction, 2);
        let probed = server
            .command(control("probe", json!({"first":1_u32,"second":2_u32})))
            .await
            .unwrap();
        assert_eq!(probed["stats"]["probe_first_calls"], 2_u32);
        assert_eq!(probed["stats"]["probe_second_calls"], 1_u32);
        assert_eq!(
            server
                .command(control(
                    "coverage",
                    json!({"ids":[1_u32,2_u32,3_u32],"calls":2_u32})
                ))
                .await
                .unwrap(),
            json!({"id":1_u32,"coverage":{"auctions":3_u32,"complete_auctions":1_u32}})
        );
        server
            .command(control("configure", settings(Mode::Plain, 0)))
            .await
            .unwrap();
        assert_eq!(stats(&mut server).await.calls, 0);
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn final_wave_stalls_leave_independent_healthy_gzip_work() {
        use flate2::read::GzDecoder;
        use std::io::Read as _;
        let mut server = Server::start(native(50, 8)).await.unwrap();
        let mut configured = settings(Mode::Stall, 0);
        configured["stall_after_calls"] = json!(14_u32);
        server
            .command(control("configure", configured))
            .await
            .unwrap();
        for _call in 0_u8..14 {
            assert_eq!(
                request(&url(server.port), post(&native_body(4)))
                    .await
                    .unwrap()
                    .category,
                Category::Success
            );
        }
        wait_active(&mut server, 0).await;
        let hold = install_hold(&server, Some(4));
        let supervisor = Supervisor::new();
        let cancellation = supervisor.clone();
        let target = url(server.port);
        let stalled_request = tokio::spawn(async move {
            owned_request(
                &target,
                RequestSpec {
                    timeout_ms: 20_000,
                    ..post(&native_body(4))
                },
                &cancellation,
            )
            .await
            .unwrap()
        });
        wait_held(&hold, 1).await;
        let dispatched = wait_active(&mut server, 1).await;
        assert_eq!(dispatched.calls, 15);
        assert_eq!(dispatched.stalled_reads, 1);
        // The barrier witnesses dispatch, then the production stall must keep the
        // request alive while an independent healthy response is decoded.
        hold.release();
        wait_held(&hold, 0).await;
        let healthy = request(&url(server.port), post(&native_body(3)))
            .await
            .unwrap();
        assert_eq!(healthy.category, Category::Success);
        assert_eq!(healthy.headers["content-encoding"], "gzip");
        let mut decoded = Vec::new();
        GzDecoder::new(healthy.body.as_slice())
            .read_to_end(&mut decoded)
            .unwrap();
        assert_eq!(decoded.len(), 2048);
        serde_json::from_slice::<Value>(&decoded).unwrap();
        let overlapping = wait_active(&mut server, 1).await;
        assert_eq!(overlapping.calls, 16);
        assert_eq!(overlapping.completed, 15);
        assert!(!stalled_request.is_finished());
        supervisor.interrupt(130);
        assert_eq!(
            stalled_request.await.unwrap().category,
            Category::Interrupted
        );
        wait_held(&hold, 0).await;
        let observed = wait_active(&mut server, 0).await;
        assert_eq!(observed.calls, 16);
        assert_eq!(observed.completed, 15);
        assert_eq!(observed.stalled_reads, 1);
        assert_eq!(observed.disconnected, 1);
        server.stop().await.unwrap();
        supervisor.cleanup().await.unwrap();
    }

    #[tokio::test]
    async fn invalid_control_settings_are_strict_and_do_not_reset_populations() {
        let mut server = Server::start(native(20, 4)).await.unwrap();
        request(&url(server.port), post(&native_body(1)))
            .await
            .unwrap();
        wait_active(&mut server, 0).await;
        let mut extra = settings(Mode::Gzip, 0);
        extra["secret"] = json!(true);
        let mut excessive_stall = settings(Mode::Stall, 0);
        excessive_stall["stall_after_calls"] = json!(10_001_u32);
        for command in [
            control("configure", extra),
            control("configure", settings(Mode::Gzip, 2001)),
            control("configure", excessive_stall),
            control("release", json!({"all":false,"extra":true})),
            control("stats", json!({})),
            control("coverage", json!({"ids":[1_u32,1_u32],"calls":1_u32})),
            control("probe", json!({"first":0_u32,"second":1_u32})),
            control("snapshot", Value::Null),
            Control {
                id: 0,
                ..control("stats", Value::Null)
            },
            Control {
                id: 1_000_001,
                ..control("stats", Value::Null)
            },
        ] {
            assert_eq!(
                server.command(command).await.unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        assert_eq!(stats(&mut server).await.calls, 1);
        server.stop().await.unwrap();
    }

    #[tokio::test]
    async fn every_provider_case_has_a_real_bid_and_counters_are_fixture_local() {
        let mut first = Server::start(provider(ProviderMode::Bid, 14))
            .await
            .unwrap();
        let mut second = Server::start(provider(ProviderMode::Failure, 2))
            .await
            .unwrap();
        for case_id in 1_u8..=14 {
            let response = request(&url(first.port), post(&provider_body(case_id)))
                .await
                .unwrap();
            assert_eq!(response.category, Category::Success);
            let captured: Value = serde_json::from_slice(&response.body).unwrap();
            assert_eq!(captured["id"], format!("handler-case-{case_id}"));
            assert_eq!(captured["seatbid"][0]["bid"][0]["price"], 1.25_f64);
            assert!(captured.get("ext").is_none());
        }
        assert_eq!(
            first
                .command(control("snapshot", Value::Null))
                .await
                .unwrap(),
            json!({"id":1_u32,"dispatches":14_u32,"case_id":14_u8,"rejected":0_u32})
        );
        assert_eq!(
            second
                .command(control("snapshot", Value::Null))
                .await
                .unwrap(),
            json!({"id":1_u32,"dispatches":0_u32,"case_id":0_u8,"rejected":0_u32})
        );
        assert_eq!(
            request(&url(second.port), post(&provider_body(2)))
                .await
                .unwrap()
                .status,
            503
        );
        for invalid in [
            control("stats", Value::Null),
            control("snapshot", json!({"extra":true})),
            Control {
                id: 0,
                ..control("snapshot", Value::Null)
            },
        ] {
            assert_eq!(
                second.command(invalid).await.unwrap_err(),
                HarnessError::new("invalid_input")
            );
        }
        first.stop().await.unwrap();
        second.stop().await.unwrap();
    }

    #[tokio::test]
    async fn native_body_json_depth_and_duplicate_ids_refuse_before_dispatch() {
        let mut server = Server::start(native(10, 4)).await.unwrap();
        let deep = format!(
            "{{\"id\":\"q1\",\"imp\":[],\"ignored\":{}0{}}}",
            "[".repeat(129),
            "]".repeat(129)
        );
        for body in [
            Bytes::from(deep),
            Bytes::from_static(b"{\"id\":\"q1\",\"id\":\"q2\",\"imp\":[]}"),
        ] {
            assert_eq!(
                request(
                    &url(server.port),
                    RequestSpec {
                        body,
                        ..post(&native_body(1))
                    }
                )
                .await
                .unwrap()
                .status,
                400
            );
        }
        assert_eq!(stats(&mut server).await.calls, 0);
        assert_eq!(
            request(&url(server.port), post(&native_body(1)))
                .await
                .unwrap()
                .category,
            Category::Success
        );
        server.stop().await.unwrap();
    }
}
