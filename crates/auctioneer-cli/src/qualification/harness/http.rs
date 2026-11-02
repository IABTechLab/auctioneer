use super::{
    HarnessError, Result,
    process::{ManagedTask, Supervisor},
};
use bytes::Bytes;
use futures::future::{Either, pending, select};
use http::{
    HeaderMap, Method, Request,
    header::{CONTENT_LENGTH, CONTENT_TYPE, HOST},
};
use http_body_util::{BodyExt as _, Full};
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use rustls::{ClientConfig, pki_types::ServerName};
use serde::Serialize;
use std::fmt;
use std::io::{IoSlice, Result as IoResult};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
    time::{Instant, sleep, timeout},
};
use tokio_rustls::TlsConnector;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Success,
    Rejected,
    HttpError,
    NetworkError,
    BodyLimit,
    Disconnect,
    Timeout,
    Interrupted,
}

#[derive(Clone, Default)]
pub struct Progress {
    pub sent: Arc<AtomicBool>,
    pub headers: Arc<AtomicBool>,
}

pub struct RequestSpec {
    pub method: Method,
    pub body: Bytes,
    pub headers: HeaderMap,
    pub timeout_ms: u64,
    pub body_cap: usize,
    pub disconnect_ms: u64,
    pub slow_read_ms: u64,
    pub inspect: bool,
    pub progress: Progress,
    pub tls: Option<Arc<ClientConfig>>,
}

impl Default for RequestSpec {
    fn default() -> Self {
        Self {
            method: Method::GET,
            body: Bytes::new(),
            headers: HeaderMap::new(),
            timeout_ms: 1000,
            body_cap: 1 << 20,
            disconnect_ms: 0,
            slow_read_ms: 0,
            inspect: false,
            progress: Progress::default(),
            tls: None,
        }
    }
}

pub struct Response {
    pub category: Category,
    pub status: u16,
    pub bytes: usize,
    pub latency_us: u64,
    pub slow_reads: u32,
    pub body: Vec<u8>,
    pub headers: HeaderMap,
}

impl Response {
    fn empty(category: Category) -> Self {
        Self {
            category,
            status: 0,
            bytes: 0,
            latency_us: 1,
            slow_reads: 0,
            body: Vec::new(),
            headers: HeaderMap::new(),
        }
    }
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Response")
            .field("category", &self.category)
            .field("status", &self.status)
            .field("bytes", &self.bytes)
            .finish_non_exhaustive()
    }
}

trait Stream: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Stream for T {}

// A sent witness requires the HTTP engine to consume the complete Full body
// and then flush its output. Starting a request alone is not a wire witness.
struct TrackedIo {
    stream: Box<dyn Stream>,
    consumed: Arc<AtomicBool>,
    flushed: Arc<AtomicBool>,
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
        let result = Pin::new(&mut self.stream).poll_flush(cx);
        if matches!(result, Poll::Ready(Ok(()))) && self.consumed.load(Ordering::Acquire) {
            self.flushed.store(true, Ordering::Release);
        }
        result
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<IoResult<()>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

struct Driver {
    task: ManagedTask,
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
)]
impl Drop for Driver {
    fn drop(&mut self) {
        self.task.abort();
    }
}

fn target(url: &str, spec: &RequestSpec) -> Result<Url> {
    let target = Url::parse(url).map_err(|_error| HarnessError::new("invalid_input"))?;
    let valid_host = target
        .host_str()
        .is_some_and(|host| host == "127.0.0.1" || host == "[::1]");
    if !valid_host
        || !matches!(target.scheme(), "http" | "https")
        || !target.username().is_empty()
        || target.password().is_some()
        || target.fragment().is_some()
        || !(1..=120_000).contains(&spec.timeout_ms)
        || !(1..=16 << 20_u32).contains(&spec.body_cap)
        || spec.body.len() > 1 << 20_u32
        || spec.disconnect_ms > 120_000
        || spec.slow_read_ms > 120_000
        || (target.scheme() == "https" && spec.tls.is_none())
    {
        return Err(HarnessError::new("invalid_input"));
    }
    Ok(target)
}

pub async fn request(url: &str, spec: RequestSpec, supervisor: &Supervisor) -> Result<Response> {
    let target = target(url, &spec)?;
    let start = Instant::now();
    let mut response = Response::empty(Category::NetworkError);
    let mut driver = None;
    let terminal = {
        let execute = exchange(&target, &spec, &mut response, &mut driver, supervisor);
        let disconnect = async {
            if spec.disconnect_ms == 0 {
                pending::<()>().await;
            } else {
                sleep(Duration::from_millis(spec.disconnect_ms)).await;
            }
        };
        let cancelled = async {
            let _reason = supervisor.cancelled().await;
        };
        match select(
            Box::pin(select(Box::pin(cancelled), Box::pin(disconnect))),
            Box::pin(timeout(Duration::from_millis(spec.timeout_ms), execute)),
        )
        .await
        {
            Either::Left((Either::Left(((), _pending)), _request)) => Category::Interrupted,
            Either::Left((Either::Right(((), _pending)), _request)) => Category::Disconnect,
            Either::Right((result, _pending)) => match result {
                Ok(category) => category,
                Err(_error) => Category::Timeout,
            },
        }
    };
    response.category = terminal;
    if let Some(connection) = driver {
        connection.task.abort();
        supervisor.join_task(&connection.task).await?;
    }
    response.latency_us = u64::try_from(start.elapsed().as_micros())
        .unwrap_or(u64::MAX)
        .max(1);
    Ok(response)
}

async fn connection_socket(target: &Url, spec: &RequestSpec) -> Option<Box<dyn Stream>> {
    let host = if target.host_str() == Some("[::1]") {
        "::1"
    } else {
        "127.0.0.1"
    };
    let port = target.port_or_known_default()?;
    let stream = TcpStream::connect((host, port)).await.ok()?;
    if target.scheme() == "https" {
        let config = spec.tls.as_ref()?;
        let name = ServerName::try_from(host.to_owned()).ok()?;
        let tls = TlsConnector::from(Arc::clone(config))
            .connect(name, stream)
            .await
            .ok()?;
        Some(Box::new(tls))
    } else {
        Some(Box::new(stream))
    }
}

async fn exchange(
    target: &Url,
    spec: &RequestSpec,
    response: &mut Response,
    driver: &mut Option<Driver>,
    supervisor: &Supervisor,
) -> Category {
    let Some(socket) = connection_socket(target, spec).await else {
        return Category::NetworkError;
    };
    let consumed = Arc::new(AtomicBool::new(spec.body.is_empty()));
    let tracked = TrackedIo {
        stream: socket,
        consumed: Arc::clone(&consumed),
        flushed: Arc::clone(&spec.progress.sent),
    };
    let Ok((mut sender, connection)) = http1::handshake(TokioIo::new(tracked)).await else {
        return Category::NetworkError;
    };
    let Ok(task) = supervisor
        .task(async move {
            let _closed = connection.await;
        })
        .await
    else {
        return Category::Interrupted;
    };
    *driver = Some(Driver { task });
    let mut path = target.path().to_owned();
    if let Some(query) = target.query() {
        path.push('?');
        path.push_str(query);
    }
    let body = Full::new(spec.body.clone()).map_frame(move |frame| {
        consumed.store(true, Ordering::Release);
        frame
    });
    let mut builder = Request::builder()
        .method(spec.method.clone())
        .uri(path)
        .header(HOST, target.authority())
        .header(CONTENT_LENGTH, spec.body.len());
    if !spec.body.is_empty() {
        builder = builder.header(CONTENT_TYPE, "application/json");
    }
    let Ok(mut outbound) = builder.body(body) else {
        return Category::NetworkError;
    };
    for (name, value) in &spec.headers {
        outbound.headers_mut().insert(name, value.clone());
    }
    let Ok(inbound) = sender.send_request(outbound).await else {
        return Category::NetworkError;
    };
    response.status = inbound.status().as_u16();
    response.headers = inbound.headers().clone();
    spec.progress.headers.store(true, Ordering::Release);
    let mut incoming = inbound.into_body();
    while let Some(frame) = incoming.frame().await {
        let Ok(received) = frame else {
            return Category::Disconnect;
        };
        if let Ok(data) = received.into_data() {
            let Some(next) = response.bytes.checked_add(data.len()) else {
                return Category::BodyLimit;
            };
            response.bytes = next;
            if next > spec.body_cap {
                return Category::BodyLimit;
            }
            if spec.inspect {
                response.body.extend_from_slice(&data);
            }
            if spec.slow_read_ms != 0 {
                response.slow_reads = response.slow_reads.saturating_add(1);
                sleep(Duration::from_millis(spec.slow_read_ms)).await;
            }
        }
    }
    if matches!(response.status, 429 | 503) {
        Category::Rejected
    } else if (200..300).contains(&response.status) {
        Category::Success
    } else {
        Category::HttpError
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::Response as HttpResponse;
    use http::StatusCode;
    use http_body_util::Full;
    use hyper::{server::conn::http1, service::service_fn};
    use hyper_util::rt::TokioIo;
    use std::convert::Infallible;
    use tokio::{net::TcpListener, time::sleep};

    async fn completed_request(url: &str, spec: RequestSpec) -> Result<Response> {
        let owner = Supervisor::new();
        let response = request(url, spec, &owner).await;
        owner.cleanup().await?;
        response
    }

    async fn server(status: StatusCode, body: &'static [u8], wait_ms: u64) -> (String, Supervisor) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let owner = Supervisor::new();
        let _task = owner
            .task(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let service = service_fn(move |_request| async move {
                    sleep(Duration::from_millis(wait_ms)).await;
                    Ok::<_, Infallible>(
                        HttpResponse::builder()
                            .status(status)
                            .body(Full::new(Bytes::from_static(body)))
                            .unwrap(),
                    )
                });
                let _closed = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            })
            .await
            .unwrap();
        (format!("http://{address}/test"), owner)
    }

    #[tokio::test]
    async fn actual_http_classifies_status_before_inspection_and_observes_flush_and_headers() {
        for (status, expected) in [
            (StatusCode::OK, Category::Success),
            (StatusCode::TOO_MANY_REQUESTS, Category::Rejected),
            (StatusCode::SERVICE_UNAVAILABLE, Category::Rejected),
            (StatusCode::BAD_REQUEST, Category::HttpError),
        ] {
            let (url, server_owner) = server(status, b"{}", 0).await;
            let progress = Progress::default();
            let response = completed_request(
                &url,
                RequestSpec {
                    inspect: true,
                    progress: progress.clone(),
                    ..RequestSpec::default()
                },
            )
            .await;
            server_owner.cleanup().await.unwrap();
            let observed = response.unwrap();
            assert_eq!(observed.category, expected);
            assert_eq!(observed.status, status.as_u16());
            assert_eq!(observed.bytes, 2);
            assert_eq!(observed.body, b"{}");
            assert!(observed.latency_us > 0);
            assert!(progress.sent.load(Ordering::Acquire));
            assert!(progress.headers.load(Ordering::Acquire));
        }
    }

    #[tokio::test]
    async fn actual_http_caps_bodies_and_preserves_timeout_disconnect_and_slow_read_categories() {
        for (cap, timeout_ms, disconnect_ms, slow_read_ms, expected) in [
            (3_usize, 1000_u64, 0_u64, 0_u64, Category::BodyLimit),
            (4, 5, 0, 0, Category::Timeout),
            (4, 1000, 5, 0, Category::Disconnect),
            (4, 1000, 0, 5, Category::Success),
        ] {
            let (url, server_owner) = server(StatusCode::OK, b"abcd", 20).await;
            let response = completed_request(
                &url,
                RequestSpec {
                    body_cap: cap,
                    timeout_ms,
                    disconnect_ms,
                    slow_read_ms,
                    inspect: true,
                    ..RequestSpec::default()
                },
            )
            .await;
            server_owner.cleanup().await.unwrap();
            let observed = response.unwrap();
            assert_eq!(observed.category, expected);
            assert!(observed.body.len() <= cap);
            if slow_read_ms != 0 {
                assert!(observed.slow_reads > 0);
            }
        }
    }

    #[tokio::test]
    async fn unrequested_bodies_are_counted_not_retained_and_nonloopback_urls_refuse() {
        let (url, server_owner) = server(StatusCode::OK, b"SECRET_BODY", 0).await;
        let response = completed_request(&url, RequestSpec::default()).await;
        server_owner.cleanup().await.unwrap();
        let observed = response.unwrap();
        assert_eq!(observed.category, Category::Success);
        assert_eq!(observed.bytes, 11);
        assert!(observed.body.is_empty());
        for invalid_url in [
            "https://publisher.example/a",
            "http://localhost/a",
            "file:///tmp/a",
            "http://user:SECRET@127.0.0.1/a",
            "http://127.0.0.1/a#SECRET",
        ] {
            assert!(matches!(
                completed_request(invalid_url, RequestSpec::default()).await,
                Err(HarnessError {
                    code: "invalid_input",
                    ..
                })
            ));
        }
    }

    #[tokio::test]
    async fn dropping_connected_request_retains_its_driver_until_external_cleanup() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/test", listener.local_addr().unwrap());
        let received = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&received);
        let server_owner = Supervisor::new();
        let _server = server_owner
            .task(async move {
                let (stream, _) = listener.accept().await.unwrap();
                let service = service_fn(move |_request| {
                    observed.store(true, Ordering::Release);
                    async {
                        pending::<()>().await;
                        Ok::<_, Infallible>(HttpResponse::new(Full::new(Bytes::new())))
                    }
                });
                let _closed = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await;
            })
            .await
            .unwrap();
        let owner = Supervisor::new();
        let mut inflight = Box::pin(request(
            &url,
            RequestSpec {
                timeout_ms: 10_000,
                ..RequestSpec::default()
            },
            &owner,
        ));
        let reached_service = matches!(
            timeout(
                Duration::from_secs(2),
                select(
                    inflight.as_mut(),
                    Box::pin(async {
                        while !received.load(Ordering::Acquire) {
                            sleep(Duration::from_millis(1)).await;
                        }
                    })
                )
            )
            .await,
            Ok(Either::Right(((), _request)))
        );
        let registered = owner.registered_task_count().await;
        drop(inflight);
        let retained = owner.registered_task_count().await;
        let cleaned = owner.cleanup().await;
        let remaining = owner.registered_task_count().await;
        let server_cleaned = server_owner.cleanup().await;
        cleaned.unwrap();
        server_cleaned.unwrap();
        assert!(reached_service);
        assert_eq!(registered, 1_usize);
        assert_eq!(retained, 1_usize);
        assert_eq!(remaining, 0_usize);
    }
}
