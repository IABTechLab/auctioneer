//! Native HTTP coverage through the production Axum binary and generated App.

#[cfg(test)]
mod tests {
    use auctioneer_core::test_support::live_catalog;
    use serde_json::{Value, json};
    use std::env::{temp_dir, var_os};
    use std::fmt::Write as _;
    use std::fs::{self, DirBuilder, File};
    use std::io::{ErrorKind, Read as _, Write as _};
    use std::iter::repeat_with;
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command, id};
    use std::str::from_utf8;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::thread::{panicking, sleep};
    use std::time::{Duration, Instant};
    use tokio::net::TcpSocket;
    use tokio::time::timeout;

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);
    const ALLOWED_ORIGIN: &str = "https://publisher.example";
    // Literal exemption: a const &str cannot hold json!'s Value; retain this
    // prehashed configuration envelope as a static transport fixture.
    const ALLOWED_CONFIG: &str = r#"{"data":{"user_sync":{"allowed_origins":["https://publisher.example"]}},"generated_at":"1970-01-01T00:00:00Z","sha256":"dda348ccc1d3ff9add8c99b9c68039aa04727b3650fb23e5acac0a47d7fbbb27","version":1}"#;
    // Literal exemption: the feature-enabled test also needs a const &str
    // envelope with its pinned configuration digest, not a runtime Value.
    const LOOPBACK_CONFIG: &str = r#"{"data":{"user_sync":{"external_url":"http://127.0.0.1:8999"}},"generated_at":"1970-01-01T00:00:00Z","sha256":"f78c7ccaa2e4add76a52440a56b35c2201f18e105bd1037d8a35abbd4ab27818","version":1}"#;

    struct Server {
        child: Option<Child>,
        addr: SocketAddr,
        dir: PathBuf,
    }

    impl Server {
        fn start(config_blob: Option<&str>) -> Self {
            Self::start_binary(
                config_blob,
                env!("CARGO_BIN_EXE_auctioneer-adapter-axum").as_ref(),
            )
        }

        fn start_binary(config_blob: Option<&str>, binary: &Path) -> Self {
            for _ in 0_usize..5_usize {
                let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback port");
                let addr = listener.local_addr().expect("reserved address");
                drop(listener);
                let dir = temp_dir().join(format!(
                    "auctioneer-usersync-transport-{}-{}",
                    id(),
                    NEXT_DIR.fetch_add(1, Ordering::Relaxed)
                ));
                DirBuilder::new()
                    .create(&dir)
                    .expect("private child directory");
                if let Some(blob) = config_blob {
                    DirBuilder::new()
                        .create(dir.join(".edgezero"))
                        .expect("private config directory");
                    fs::write(
                        dir.join(".edgezero/local-config-auctioneer_runtime.json"),
                        serde_json::to_vec(&json!({"auctioneer_runtime": blob}))
                            .expect("serialize config-store fixture"),
                    )
                    .expect("private config blob");
                }
                let log = File::create(dir.join("server.log")).expect("child log file");
                let child = Command::new(binary)
                    .current_dir(&dir)
                    .env_clear()
                    .env("EDGEZERO__ADAPTER__HOST", "127.0.0.1")
                    .env("EDGEZERO__ADAPTER__PORT", addr.port().to_string())
                    .env("EDGEZERO__SECRETS__USERSYNC_COOKIE_KEYRING", serde_json::to_string(&json!({
                        "active":"test", "keys":[{"id":"test","key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]
                    })).expect("serialize public synthetic signing key"))
                    .stdout(log.try_clone().expect("clone child log"))
                    .stderr(log)
                    .spawn()
                    .expect("spawn production Axum binary");
                let mut server = Self {
                    child: Some(child),
                    addr,
                    dir,
                };
                let deadline = Instant::now()
                    .checked_add(Duration::from_secs(10))
                    .expect("startup deadline");
                loop {
                    if let Some(status) = server
                        .child
                        .as_mut()
                        .expect("child exists")
                        .try_wait()
                        .expect("poll child")
                    {
                        let startup_log = server.log();
                        if startup_log.contains("failed to bind dev server") {
                            break;
                        }
                        panic!("Axum child exited during bootstrap ({status}): {startup_log}");
                    }
                    if TcpStream::connect_timeout(&addr, Duration::from_millis(100)).is_ok() {
                        return server;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "Axum readiness timed out: {}",
                        server.log()
                    );
                    sleep(Duration::from_millis(25));
                }
            }
            panic!("Axum could not claim a reserved loopback port after five attempts");
        }

        fn log(&self) -> String {
            fs::read_to_string(self.dir.join("server.log")).unwrap_or_default()
        }

        fn aux_slots(&self) -> usize {
            let output = Command::new(env!("CARGO_BIN_EXE_auctioneer-adapter-axum"))
                .current_dir(&self.dir)
                .env_clear()
                .arg("--resource-profile")
                .output()
                .expect("inspect the serving binary's compiled resource profile");
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            let profile: Value =
                serde_json::from_slice(&output.stdout).expect("resource profile JSON");
            let slots = usize::try_from(
                profile["resources"]["limits"]["aux_route_permits"]
                    .as_u64()
                    .expect("compiled aux permits"),
            )
            .expect("aux permits fit usize");
            assert!(
                slots > 0,
                "the disconnect fixture needs an enabled aux class"
            );
            slots
        }

        fn hold_streaming_body(&self) -> TcpStream {
            let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_secs(2))
                .expect("connect held request");
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .expect("held request write timeout");
            // Axum buffers application/json before EdgeZero admission. text/plain
            // leaves this body streaming, so the aux permit is held while it waits.
            stream
                .write_all(
                    b"POST /cookie_sync HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: text/plain\r\nContent-Length: 128\r\n\r\n{",
                )
                .expect("send held request head and partial body");
            stream
        }

        fn wait_for_status(&self, expected: u16) -> WireResponse {
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(3))
                .expect("status deadline");
            loop {
                let response = self.request("GET", "/getuids", &[], b"");
                if response.status == expected {
                    return response;
                }
                assert!(
                    Instant::now() < deadline,
                    "expected {expected}, last status was {}",
                    response.status
                );
                sleep(Duration::from_millis(20));
            }
        }

        fn request(
            &self,
            method: &str,
            target: &str,
            headers: &[(&str, &str)],
            body: &[u8],
        ) -> WireResponse {
            let mut stream = TcpStream::connect_timeout(&self.addr, Duration::from_secs(2))
                .expect("connect to Axum child");
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .expect("write timeout");
            let mut request =
                format!("{method} {target} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n");
            for (name, value) in headers {
                write!(request, "{name}: {value}\r\n").expect("append request header");
            }
            if !body.is_empty()
                && !headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("content-length"))
            {
                write!(request, "Content-Length: {}\r\n", body.len())
                    .expect("append content length");
            }
            request.push_str("\r\n");
            stream
                .write_all(request.as_bytes())
                .expect("send request head");
            stream.write_all(body).expect("send request body");
            let mut raw = Vec::new();
            let mut chunk = [0_u8; 8 * 1024];
            let deadline = Instant::now()
                .checked_add(Duration::from_secs(5))
                .expect("response deadline");
            loop {
                let remaining = deadline.saturating_duration_since(Instant::now());
                assert!(
                    !remaining.is_zero(),
                    "HTTP response exceeded five-second deadline"
                );
                stream
                    .set_read_timeout(Some(
                        remaining
                            .min(Duration::from_millis(500))
                            .max(Duration::from_millis(1)),
                    ))
                    .expect("bounded read timeout");
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(count) => {
                        assert!(
                            raw.len()
                                .checked_add(count)
                                .is_some_and(|total| total <= 1024 * 1024),
                            "HTTP response exceeded one-megabyte test cap"
                        );
                        raw.extend_from_slice(&chunk[..count]);
                        if WireResponse::is_complete(&raw) {
                            break;
                        }
                    }
                    Err(error) if error.kind() == ErrorKind::ConnectionReset && !raw.is_empty() => {
                        break;
                    }
                    Err(error)
                        if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                    Err(error) => {
                        panic!("read HTTP response: {error}; received {} bytes", raw.len())
                    }
                }
            }
            WireResponse::parse(&raw)
        }
    }

    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop::pin_drop is unstable and mutually exclusive with Drop::drop"
    )]
    impl Drop for Server {
        fn drop(&mut self) {
            let mut cleanup_errors = Vec::new();
            let mut timed_out = false;
            if let Some(mut child) = self.child.take() {
                if let Some(error) = child.kill().err().filter(|error| {
                    !matches!(error.kind(), ErrorKind::InvalidInput | ErrorKind::NotFound)
                }) {
                    cleanup_errors.push(format!("kill Axum child: {error}"));
                }
                let deadline = Instant::now()
                    .checked_add(Duration::from_secs(3))
                    .expect("child reap deadline");
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => break,
                        Ok(None) if Instant::now() < deadline => {
                            sleep(Duration::from_millis(10));
                        }
                        Ok(None) => {
                            timed_out = true;
                            if let Err(error) = child.kill() {
                                cleanup_errors.push(format!("kill timed-out Axum child: {error}"));
                            }
                            if let Err(error) = child.wait() {
                                cleanup_errors.push(format!("reap timed-out Axum child: {error}"));
                            }
                            break;
                        }
                        Err(error) => {
                            cleanup_errors.push(format!("poll Axum child: {error}"));
                            if let Err(wait_error) = child.wait() {
                                cleanup_errors.push(format!("reap Axum child: {wait_error}"));
                            }
                            break;
                        }
                    }
                }
            }
            if let Err(error) = fs::remove_dir_all(&self.dir) {
                cleanup_errors.push(format!("remove Axum child directory: {error}"));
            }
            if !panicking() {
                assert!(!timed_out, "Axum child did not reap within three seconds");
                assert!(cleanup_errors.is_empty(), "{}", cleanup_errors.join("; "));
            }
        }
    }

    struct WireResponse {
        status: u16,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl WireResponse {
        fn is_complete(raw: &[u8]) -> bool {
            let Some(split) = raw.windows(4).position(|window| window == b"\r\n\r\n") else {
                return false;
            };
            let head = from_utf8(&raw[..split]).expect("ASCII response head");
            let mut lines = head.split("\r\n");
            let status = lines
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .expect("response status");
            if matches!(status, "204" | "304") {
                return true;
            }
            lines
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length").then(|| {
                        value
                            .trim()
                            .parse::<usize>()
                            .expect("response content length")
                    })
                })
                .is_some_and(|length| {
                    let body_start = split.checked_add(4).expect("response head length");
                    raw.len()
                        .checked_sub(body_start)
                        .is_some_and(|body| body >= length)
                })
        }

        fn parse(raw: &[u8]) -> Self {
            let split = raw
                .windows(4)
                .position(|window| window == b"\r\n\r\n")
                .expect("HTTP response head");
            let head = from_utf8(&raw[..split]).expect("ASCII response head");
            let mut lines = head.split("\r\n");
            let status = lines
                .next()
                .expect("status line")
                .split_whitespace()
                .nth(1)
                .expect("status code")
                .parse()
                .expect("numeric status");
            let headers = lines
                .map(|line| {
                    let (name, value) = line.split_once(':').expect("header field");
                    (name.to_ascii_lowercase(), value.trim().to_owned())
                })
                .collect::<Vec<_>>();
            let mut response = Self {
                status,
                headers,
                body: raw[split.checked_add(4).expect("response head length")..].to_vec(),
            };
            if response.header("transfer-encoding") == Some("chunked") {
                response.body = decode_chunked(&response.body);
            }
            response
        }

        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value.as_str())
        }

        fn text(&self) -> &str {
            from_utf8(&self.body).expect("UTF-8 response body")
        }
    }

    fn decode_chunked(mut raw: &[u8]) -> Vec<u8> {
        let mut body = Vec::new();
        loop {
            let end = raw
                .windows(2)
                .position(|window| window == b"\r\n")
                .expect("chunk size line");
            let size = from_utf8(&raw[..end])
                .expect("ASCII chunk size")
                .split(';')
                .next()
                .and_then(|size| usize::from_str_radix(size, 16).ok())
                .expect("hex chunk size");
            raw = &raw[end.checked_add(2).expect("chunk size line length")..];
            if size == 0 {
                return body;
            }
            body.extend_from_slice(raw.get(..size).expect("complete chunk"));
            raw = raw
                .get(size.checked_add(2).expect("chunk length")..)
                .expect("chunk terminator");
        }
    }

    fn assert_requests_waiting(streams: &[TcpStream]) {
        for stream in streams {
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .expect("held response peek timeout");
            let mut byte = [0_u8; 1];
            match stream.peek(&mut byte) {
                Err(error)
                    if matches!(error.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Ok(0) => panic!("a recovered aux slot closed while its body was held"),
                Ok(_) => panic!("a recovered aux slot was refused before its body completed"),
                Err(error) => panic!("peek held request: {error}"),
            }
        }
    }

    #[test]
    fn response_completion_waits_for_declared_bytes() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        for end in 0..response.len() {
            assert!(!WireResponse::is_complete(&response[..end]));
        }
        assert!(WireResponse::is_complete(response));
        assert!(WireResponse::is_complete(
            b"HTTP/1.1 204 No Content\r\n\r\n"
        ));
        assert!(WireResponse::is_complete(
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
        ));
    }

    #[test]
    fn real_binary_preflight_sync_callback_and_readback() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let preflight = server.request(
            "OPTIONS",
            "/cookie_sync",
            &[
                ("Origin", ALLOWED_ORIGIN),
                ("Access-Control-Request-Method", "POST"),
                ("Access-Control-Request-Headers", "content-type"),
            ],
            b"",
        );
        assert_eq!(preflight.status, 204);
        assert_eq!(
            preflight.header("access-control-allow-origin"),
            Some(ALLOWED_ORIGIN)
        );
        assert_eq!(
            preflight.header("access-control-allow-credentials"),
            Some("true")
        );
        assert_eq!(preflight.header("cache-control"), Some("no-store"));

        let sync = server.request(
            "POST",
            "/cookie_sync",
            &[
                ("Content-Type", "application/json"),
                ("Origin", ALLOWED_ORIGIN),
            ],
            &serde_json::to_vec(
                &json!({"bidders": [live_catalog::redirect().name()], "gdpr": 0_i32}),
            )
            .expect("serialize cookie-sync request"),
        );
        assert_eq!(sync.status, 200, "{}", sync.text());
        assert_eq!(
            sync.header("access-control-allow-origin"),
            Some(ALLOWED_ORIGIN)
        );
        let sync_body: Value = serde_json::from_slice(&sync.body).expect("sync response JSON");
        let statuses = sync_body["bidder_status"]
            .as_array()
            .expect("sync statuses");
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0]["bidder"], live_catalog::redirect().name());
        assert_eq!(statuses[0]["usersync"]["type"], "redirect");
        let emitted_callback = live_catalog::redirect()
            .callback(statuses[0]["usersync"]["url"].as_str().expect("sync URL"));
        assert_eq!(
            emitted_callback
                .query_pairs()
                .find(|(key, _)| key == "bidder")
                .expect("callback identity")
                .1,
            live_catalog::redirect().name()
        );
        assert!(!sync.headers.iter().any(|(name, _)| name == "set-cookie"));

        let callback = server.request(
            "GET",
            &live_catalog::redirect().setuid("uid=buyer-transport&gdpr=0&f=b"),
            &[],
            b"",
        );
        assert_eq!(callback.status, 200, "{}", callback.text());
        assert_eq!(callback.header("cache-control"), Some("no-store"));
        assert_eq!(callback.header("content-type"), Some("text/html"));
        let cookie = callback
            .header("set-cookie")
            .expect("successful callback writes cookie")
            .split(';')
            .next()
            .expect("cookie pair");
        let readback = server.request("GET", "/getuids", &[("Cookie", cookie)], b"");
        assert_eq!(readback.status, 200);
        assert_eq!(
            readback.text(),
            format!(
                "{{\"buyeruids\":{{{}:\"buyer-transport\"}}}}\n",
                serde_json::to_string(live_catalog::redirect().family()).expect("family JSON")
            )
        );
        assert_eq!(readback.header("cache-control"), Some("no-store"));
        assert!(readback.header("set-cookie").is_none());

        let denied = server.request(
            "OPTIONS",
            "/cookie_sync",
            &[
                ("Origin", "https://other.example"),
                ("Access-Control-Request-Method", "POST"),
            ],
            b"",
        );
        assert_eq!(denied.status, 403);
        assert!(denied.header("access-control-allow-origin").is_none());
        assert_eq!(denied.header("cache-control"), Some("no-store"));
        assert!(!server.log().is_empty(), "child logging must be captured");
        assert!(!server.log().contains("buyer-transport"));
    }

    #[test]
    fn real_binary_logs_do_not_expose_sync_inputs_or_cookie_values() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let consent = "private-consent-token";
        let gdpr = "private-gdpr-token";
        let query = "private-query-token";
        let cookie = "private-cookie-token";
        let uid = "private-uid-token";
        let private_url = "https://private.example/private-url-token?uid=private-url-user";
        let malformed_body = serde_json::to_vec(&serde_json::json!({
            "bidders": [live_catalog::redirect().name()],
            "gdpr": gdpr,
            "gdpr_consent": consent,
        }))
        .expect("malformed GDPR fixture serialization");

        let malformed = server.request(
            "POST",
            "/cookie_sync",
            &[("Content-Type", "application/json"), ("Cookie", cookie)],
            &malformed_body,
        );
        assert_eq!(malformed.status, 400);
        let url_body = serde_json::to_vec(&serde_json::json!({
            "bidders": [live_catalog::redirect().name()],
            "gdpr": 0_i32,
            "gdpr_consent": private_url,
            "coopSync": false,
        }))
        .expect("URL-bearing consent fixture serialization");
        let url_response = server.request(
            "POST",
            "/cookie_sync",
            &[("Content-Type", "application/json")],
            &url_body,
        );
        assert_eq!(url_response.status, 200, "{}", url_response.text());
        assert!(url_response.text().contains("private-url-token"));
        assert!(url_response.header("set-cookie").is_none());
        let callback = server.request(
            "GET",
            &live_catalog::redirect().setuid(&format!("uid={uid}&gdpr=0&f=b&debug={query}")),
            &[],
            b"",
        );
        assert_eq!(callback.status, 200, "{}", callback.text());
        let encoded = callback.header("set-cookie").expect("callback cookie");
        let readback = server.request("GET", "/getuids", &[("Cookie", encoded)], b"");
        assert_eq!(readback.status, 200);

        let logs = server.log();
        for private in [
            consent,
            gdpr,
            query,
            cookie,
            uid,
            encoded,
            private_url,
            "private-url-token",
            "private-url-user",
            "AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE",
        ] {
            assert!(
                !logs.contains(private),
                "sync input appeared in server logs"
            );
        }
    }

    #[test]
    fn real_binary_remains_available_after_partial_body_disconnect() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let mut slow = TcpStream::connect_timeout(&server.addr, Duration::from_secs(2))
            .expect("connect slow request");
        slow.write_all(
            b"POST /cookie_sync HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{",
        )
        .expect("send partial body");
        sleep(Duration::from_millis(50));
        drop(slow);
        let next = server.request("GET", "/getuids", &[], b"");
        assert_eq!(next.status, 200, "{}", next.text());
        assert_eq!(next.text(), "{}\n");
    }

    #[test]
    fn real_binary_recovers_all_aux_slots_after_stream_disconnects() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let aux_slots = server.aux_slots();
        let first_wave = repeat_with(|| server.hold_streaming_body())
            .take(aux_slots)
            .collect::<Vec<_>>();
        let saturated = server.wait_for_status(503);
        assert_eq!(saturated.header("cache-control"), Some("no-store"));
        let probe = server.request("GET", "/status", &[], b"");
        assert_eq!(
            probe.status, 204,
            "probe must remain available at aux saturation"
        );
        assert_requests_waiting(&first_wave);

        drop(first_wave);
        server.wait_for_status(200);

        let second_wave = repeat_with(|| server.hold_streaming_body())
            .take(aux_slots)
            .collect::<Vec<_>>();
        server.wait_for_status(503);
        assert_requests_waiting(&second_wave);
        drop(second_wave);
        server.wait_for_status(200);
    }

    #[tokio::test]
    async fn real_binary_recovers_all_aux_slots_after_slow_response_disconnects() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let aux_slots = server.aux_slots();
        let consent = "<".repeat(4_000);
        let body = serde_json::to_string(&json!({
            "bidders": [live_catalog::redirect().name(), live_catalog::iframe().name()],
            "gdpr": 0_i32,
            "gdpr_consent": consent,
            "coopSync": false,
        }))
        .expect("serialize sync request");
        let baseline = server.request(
            "POST",
            "/cookie_sync",
            &[("Content-Type", "application/json")],
            body.as_bytes(),
        );
        assert_eq!(baseline.status, 200, "{}", baseline.text());
        assert!(baseline.body.len() > 32 * 1_024, "large response fixture");
        let request = format!(
            "POST /cookie_sync HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let pipelined = request.repeat(64);
        let mut receivers = Vec::new();
        for _ in 0_usize..aux_slots {
            let socket = TcpSocket::new_v4().expect("slow receiver socket");
            socket
                .set_recv_buffer_size(1_024)
                .expect("small receiver window");
            let connected = timeout(Duration::from_secs(2), socket.connect(server.addr))
                .await
                .expect("slow receiver connection deadline")
                .expect("connect slow receiver");
            let mut stream = connected.into_std().expect("blocking receiver socket");
            stream
                .set_nonblocking(false)
                .expect("blocking receiver writes");
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .expect("pipeline write deadline");
            // Every request body is complete; only response reads are withheld.
            stream
                .write_all(pipelined.as_bytes())
                .expect("send complete pipelined requests");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("response head deadline");
            let mut head_buffer = [0_u8; 1_024];
            let deadline = Instant::now() + Duration::from_secs(2);
            let head = loop {
                let count = stream
                    .peek(&mut head_buffer)
                    .expect("peek slow response head");
                assert!(count > 0, "slow response closed before its head arrived");
                if let Some(end) = head_buffer[..count]
                    .windows(4)
                    .position(|part| part == b"\r\n\r\n")
                {
                    break from_utf8(&head_buffer[..end])
                        .expect("ASCII response head")
                        .to_owned();
                }
                assert!(Instant::now() < deadline, "incomplete slow response head");
                sleep(Duration::from_millis(10));
            };
            assert!(head.starts_with("HTTP/1.1 200 "), "{head}");
            assert!(head.contains("cache-control: no-store"), "{head}");
            assert!(!head.contains("set-cookie:"), "{head}");
            receivers.push(stream);
        }

        sleep(Duration::from_millis(100));
        assert_eq!(server.request("GET", "/status", &[], b"").status, 204);

        drop(receivers);
        server.wait_for_status(200);
        // HostHandoff may release a response before the slow client drains it.
        // Streaming bodies below prove all aux slots are reusable after disconnect.
        let held = repeat_with(|| server.hold_streaming_body())
            .take(aux_slots)
            .collect::<Vec<_>>();
        server.wait_for_status(503);
        assert_requests_waiting(&held);
        drop(held);
        server.wait_for_status(200);
    }

    #[test]
    fn real_binary_enforces_setuid_query_limit_and_parameter_count() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let prefix = format!(
            "bidder={}&uid=x&gdpr=0&f=b&x=",
            live_catalog::redirect().name()
        );
        let at_limit = format!(
            "/setuid?{prefix}{}",
            "a".repeat(
                0x8000_usize
                    .checked_sub(prefix.len())
                    .expect("query prefix fits")
            )
        );
        let accepted = server.request("GET", &at_limit, &[], b"");
        assert_eq!(accepted.status, 200, "{}", accepted.text());
        let too_long = format!("{at_limit}a");
        let rejected = server.request("GET", &too_long, &[], b"");
        assert_eq!(rejected.status, 400);
        assert_eq!(rejected.text(), "query exceeds 32768 bytes");

        let mut too_many = live_catalog::redirect().setuid("uid=x&gdpr=0&f=b");
        for _ in 0_usize..29_usize {
            too_many.push_str("&x=1");
        }
        let too_many_rejected = server.request("GET", &too_many, &[], b"");
        assert_eq!(too_many_rejected.status, 400);
        assert_eq!(too_many_rejected.text(), "too many query parameters");
    }

    #[test]
    fn real_binary_enforces_cookie_sync_body_limit() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let oversized_body = vec![b' '; 0x8001];
        let rejected = server.request(
            "POST",
            "/cookie_sync",
            &[("Content-Type", "application/json")],
            &oversized_body,
        );
        assert_eq!(rejected.status, 400);
        assert_eq!(
            rejected.text(),
            "cookie_sync request size exceeded max size of 32768 bytes.\n"
        );
    }

    #[test]
    fn real_binary_rejects_encoding_before_waiting_for_oversized_body() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let started = Instant::now();
        let rejected = server.request(
            "POST",
            "/cookie_sync",
            &[
                ("Content-Type", "application/json"),
                ("Content-Encoding", "gzip"),
                ("Content-Length", "32769"),
            ],
            b"",
        );
        assert_eq!(rejected.status, 400);
        assert_eq!(
            rejected.text(),
            "Invalid request: Content-Encoding of type gzip is not supported\n"
        );
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn real_binary_enforces_header_count_and_bytes() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let mut many = Vec::new();
        for i in 0_usize..130_usize {
            many.push((format!("X-Field-{i}"), "x".to_owned()));
        }
        let borrowed = many
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect::<Vec<_>>();
        let rejected = server.request("GET", "/getuids", &borrowed, b"");
        assert_eq!(rejected.status, 431);

        let padding = "x".repeat(17 * 1024);
        let padding_rejected = server.request("GET", "/getuids", &[("X-Padding", &padding)], b"");
        assert_eq!(padding_rejected.status, 431);
    }

    #[test]
    fn real_binary_keeps_application_and_config_errors_separate() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let bad_json = server.request(
            "POST",
            "/cookie_sync",
            &[
                ("Content-Type", "application/json"),
                ("Origin", ALLOWED_ORIGIN),
            ],
            b"{",
        );
        assert_eq!(bad_json.status, 400);
        assert_eq!(
            bad_json.header("access-control-allow-origin"),
            Some(ALLOWED_ORIGIN)
        );
        assert_eq!(bad_json.header("cache-control"), Some("no-store"));
        assert!(bad_json.header("set-cookie").is_none());

        let duplicate_origin = server.request(
            "POST",
            "/cookie_sync",
            &[
                ("Content-Type", "application/json"),
                ("Origin", ALLOWED_ORIGIN),
                ("Origin", "https://other.example"),
            ],
            &serde_json::to_vec(&json!({"bidders": [], "gdpr": 0_i32}))
                .expect("serialize duplicate-origin request"),
        );
        assert_eq!(duplicate_origin.status, 200);
        assert!(
            duplicate_origin
                .header("access-control-allow-origin")
                .is_none()
        );

        let unsyncable = server.request(
            "GET",
            &format!(
                "/setuid?bidder={}&uid=blocked&gdpr=0",
                live_catalog::unsyncable().name
            ),
            &[],
            b"",
        );
        assert_eq!(unsyncable.status, 400);
        assert!(unsyncable.header("set-cookie").is_none());

        let unknown = server.request("GET", "/missing-sync-route", &[], b"");
        assert_eq!(unknown.status, 404);
        assert!(unknown.header("access-control-allow-origin").is_none());

        let broken = Server::start(Some("not an envelope"));
        let config_error = broken.request("GET", "/getuids", &[("Origin", ALLOWED_ORIGIN)], b"");
        assert_eq!(config_error.status, 500);
        assert!(config_error.header("access-control-allow-origin").is_none());
        assert_eq!(config_error.header("cache-control"), Some("no-store"));
    }

    #[test]
    fn linked_binary_exposes_bidder_and_loopback_callback() {
        let server = Server::start(Some(ALLOWED_CONFIG));
        let bidders = server.request("GET", "/info/bidders", &[], b"");
        assert_eq!(bidders.status, 200);
        let names: Vec<String> = serde_json::from_slice(&bidders.body).expect("bidder list");
        for row in live_catalog::feature_declarations("insecure-bidders") {
            assert!(
                names.iter().any(|name| name == row.bidder),
                "missing feature-gated identity {}",
                row.bidder
            );
        }

        let local = Server::start(Some(LOOPBACK_CONFIG));
        let sync = local.request(
            "POST",
            "/cookie_sync",
            &[("Content-Type", "application/json")],
            &serde_json::to_vec(
                &json!({"bidders": [live_catalog::redirect().name()], "gdpr": 0_i32}),
            )
            .expect("serialize loopback cookie-sync request"),
        );
        assert_eq!(sync.status, 200, "{}", sync.text());
        assert!(sync.text().contains("127.0.0.1"), "{}", sync.text());
    }

    #[test]
    #[ignore = "driven by scripts/check-secure-usersync.sh with a separately built production binary"]
    fn secure_production_binary_rejects_loopback_callback_config() {
        let binary = var_os("AUCTIONEER__TEST__SECURE_AXUM_BINARY")
            .expect("the secure-default check must supply its production binary");
        let server = Server::start_binary(Some(LOOPBACK_CONFIG), binary.as_ref());
        let rejected = server.request("GET", "/getuids", &[], b"");
        assert_eq!(rejected.status, 500);
        assert_eq!(rejected.header("cache-control"), Some("no-store"));
        let defaults = Server::start_binary(Some(ALLOWED_CONFIG), binary.as_ref());
        let bidders = defaults.request("GET", "/info/bidders", &[], b"");
        assert_eq!(bidders.status, 200);
        let names: Vec<String> = serde_json::from_slice(&bidders.body).expect("secure bidder list");
        for row in live_catalog::feature_declarations("insecure-bidders") {
            assert!(
                !names.iter().any(|name| name == row.bidder),
                "secure binary exposes gated identity {}",
                row.bidder
            );
        }
    }
}
