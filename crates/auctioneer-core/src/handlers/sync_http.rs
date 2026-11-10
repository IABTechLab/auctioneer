//! Shared bounded HTTP inputs and endpoint-specific sync response shapes.

use std::{
    collections::BTreeMap,
    fmt,
    io::{self, Write as _},
    str::from_utf8,
};

use auctioneer_bidder::CappedWriter;
use auctioneer_config_data::usersync::{SyncResponseFormat, SyncType, UserSyncConfig};
use auctioneer_runtime::aux_work::{
    MAX_SYNC_OUTPUT_BYTES, MAX_SYNC_RESPONSE_HEADER_BYTES, MAX_SYNC_RESPONSE_HEADER_COUNT,
};
use edgezero_core::body::Body;
use edgezero_core::error::{EdgeError, RequestBodyOverflowOrigin};
use edgezero_core::http::{
    HeaderMap, HeaderName, HeaderValue, Response, StatusCode,
    header::{
        CACHE_CONTROL, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, ORIGIN, SET_COOKIE, VARY,
    },
};
use serde::Serialize;
use time::OffsetDateTime;

use crate::{
    host_primitives::error_renderer::render_error_response,
    usersync::{
        cookie::{MAX_SET_COOKIE_FIELD_BYTES, PreparedCookie, UidCookie},
        wire::{MAX_COOKIE_SYNC_BODY_BYTES, WireError},
    },
};

const PIXEL: &[u8; 86] = include_bytes!("assets/pixel.png");
const TEXT_PLAIN: HeaderValue = HeaderValue::from_static("text/plain; charset=utf-8");
const JSON: HeaderValue = HeaderValue::from_static("application/json; charset=utf-8");

/// A deployment without an explicit sync origin never reads signing keys or
/// constructs callbacks. This response grants no browser credentials policy.
pub(crate) fn unavailable() -> Response {
    setuid_error(
        StatusCode::SERVICE_UNAVAILABLE,
        "user sync is not configured",
    )
}

/// Fixed application-body ceiling; refuses output if the target cannot represent it.
#[must_use]
pub fn response_body_cap() -> usize {
    usize::try_from(MAX_SYNC_OUTPUT_BYTES).unwrap_or(0)
}

/// Fixed application-header byte ceiling; Setuid bounds its cookie separately.
/// The profile's accounting allowance may be larger. Headers added later by
/// the adapter/provider are outside this application limit.
#[must_use]
pub fn response_header_cap() -> usize {
    usize::try_from(MAX_SYNC_RESPONSE_HEADER_BYTES).unwrap_or(0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseBuildError {
    BodyLimit,
    HeaderLimit,
    Serialization,
}

pub struct SyncBrowserPolicy<'a> {
    allowed_origins: &'a [String],
}

impl<'a> SyncBrowserPolicy<'a> {
    #[must_use]
    pub fn new(snapshot: &'a UserSyncConfig) -> Self {
        Self {
            allowed_origins: &snapshot.allowed_origins,
        }
    }

    /// Applies browser policy and bounds application-owned headers only.
    /// Adapter/provider-added framing headers are not included.
    ///
    /// # Errors
    /// Returns `HeaderLimit` if the resulting header bytes/count exceed their caps or overflow.
    pub fn decorate(
        &self,
        response: Response,
        request_headers: &HeaderMap,
        max_header_bytes: usize,
    ) -> Result<Response, ResponseBuildError> {
        bound_headers(
            self.apply_policy(response, request_headers),
            max_header_bytes,
        )
    }

    /// Bounds application-owned headers with an independent Set-Cookie bound.
    /// Adapter/provider-added framing headers are not included.
    ///
    /// # Errors
    /// Returns `HeaderLimit` if the cookie field, other header bytes or total count exceed their caps.
    pub fn decorate_setuid(
        &self,
        response: Response,
        request_headers: &HeaderMap,
        max_other_header_bytes: usize,
    ) -> Result<Response, ResponseBuildError> {
        bound_setuid_headers(
            self.apply_policy(response, request_headers),
            max_other_header_bytes,
        )
    }

    fn apply_policy(&self, mut response: Response, request_headers: &HeaderMap) -> Response {
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        for name in [
            "access-control-allow-origin",
            "access-control-allow-credentials",
            "access-control-allow-methods",
            "access-control-allow-headers",
        ] {
            response.headers_mut().remove(name);
        }
        if let Some(origin) = self.allowed_origin(request_headers) {
            response
                .headers_mut()
                .insert("access-control-allow-origin", origin);
            response.headers_mut().insert(
                "access-control-allow-credentials",
                HeaderValue::from_static("true"),
            );
            merge_vary(response.headers_mut(), "Origin");
        }
        response
    }

    /// Builds a preflight response without reading cookies or the request body.
    ///
    /// # Errors
    /// Returns `HeaderLimit` if the resulting headers exceed the cap or overflow.
    pub fn preflight(
        &self,
        request_headers: &HeaderMap,
        max_header_bytes: usize,
    ) -> Result<Response, ResponseBuildError> {
        let Some(origin) = self.allowed_origin(request_headers) else {
            return bound_headers(
                response(StatusCode::FORBIDDEN, Vec::new()),
                max_header_bytes,
            );
        };
        if !requested_post(request_headers) || !content_type_header_set(request_headers) {
            return bound_headers(
                response(StatusCode::FORBIDDEN, Vec::new()),
                max_header_bytes,
            );
        }
        let mut response = response(StatusCode::NO_CONTENT, Vec::new());
        let headers = response.headers_mut();
        headers.insert("access-control-allow-origin", origin);
        headers.insert(
            "access-control-allow-credentials",
            HeaderValue::from_static("true"),
        );
        headers.insert(
            "access-control-allow-methods",
            HeaderValue::from_static("POST"),
        );
        headers.insert(
            "access-control-allow-headers",
            HeaderValue::from_static("Content-Type"),
        );
        for name in [
            "Origin",
            "Access-Control-Request-Method",
            "Access-Control-Request-Headers",
        ] {
            merge_vary(headers, name);
        }
        bound_headers(response, max_header_bytes)
    }

    fn allowed_origin(&self, request_headers: &HeaderMap) -> Option<HeaderValue> {
        let mut values = request_headers.get_all(ORIGIN).iter();
        let value = values.next()?;
        if values.next().is_some() {
            return None;
        }
        let text = value.to_str().ok()?;
        let parsed = url::Url::parse(text).ok()?;
        if !matches!(parsed.scheme(), "http" | "https")
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || text != parsed.origin().ascii_serialization()
        {
            return None;
        }
        // A request HeaderValue may share a much larger ingress backing.
        self.allowed_origins
            .iter()
            .find(|allowed| allowed.as_str() == text)
            .and_then(|allowed| HeaderValue::from_bytes(allowed.as_bytes()).ok())
    }
}

fn requested_post(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all("access-control-request-method").iter();
    let Some(value) = values.next() else {
        return false;
    };
    values.next().is_none() && value.as_bytes() == b"POST"
}

fn content_type_header_set(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all("access-control-request-headers").iter();
    let Some(value) = values.next() else {
        return true;
    };
    if values.next().is_some() || value.len() > 256 {
        return false;
    }
    let Ok(raw) = value.to_str() else {
        return false;
    };
    raw.split(',')
        .all(|name| name.trim().eq_ignore_ascii_case("content-type"))
}

fn merge_vary(headers: &mut HeaderMap, name: &'static str) {
    let present = headers
        .get_all(VARY)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|token| {
            let trimmed = token.trim();
            trimmed == "*" || trimmed.eq_ignore_ascii_case(name)
        });
    if !present {
        headers.append(VARY, HeaderValue::from_static(name));
    }
}

/// Counts serialized header bytes, including delimiters.
///
/// # Errors
/// Returns `HeaderLimit` if the byte count overflows.
pub fn serialized_header_bytes(headers: &HeaderMap) -> Result<usize, ResponseBuildError> {
    headers.iter().try_fold(2_usize, |total, (name, value)| {
        add_serialized_header_bytes(total, name.as_str(), value)
    })
}

fn add_serialized_header_bytes(
    total: usize,
    name: &str,
    value: &HeaderValue,
) -> Result<usize, ResponseBuildError> {
    total
        .checked_add(name.len())
        .and_then(|sum| sum.checked_add(2))
        .and_then(|sum| sum.checked_add(value.as_bytes().len()))
        .and_then(|sum| sum.checked_add(2))
        .ok_or(ResponseBuildError::HeaderLimit)
}

fn bound_headers(
    response: Response,
    max_header_bytes: usize,
) -> Result<Response, ResponseBuildError> {
    if response.headers().len() > usize::try_from(MAX_SYNC_RESPONSE_HEADER_COUNT).unwrap_or(0) {
        return Err(ResponseBuildError::HeaderLimit);
    }
    if serialized_header_bytes(response.headers())? > max_header_bytes {
        return Err(ResponseBuildError::HeaderLimit);
    }
    own_header_storage(response)
}

fn bound_setuid_headers(
    response: Response,
    max_other_header_bytes: usize,
) -> Result<Response, ResponseBuildError> {
    if response.headers().len() > usize::try_from(MAX_SYNC_RESPONSE_HEADER_COUNT).unwrap_or(0) {
        return Err(ResponseBuildError::HeaderLimit);
    }
    let mut saw_cookie = false;
    let other_bytes = response
        .headers()
        .iter()
        .try_fold(2_usize, |total, (name, value)| {
            if name.as_str() == SET_COOKIE.as_str() {
                if saw_cookie || value.as_bytes().len() > MAX_SET_COOKIE_FIELD_BYTES {
                    return Err(ResponseBuildError::HeaderLimit);
                }
                saw_cookie = true;
                Ok(total)
            } else {
                add_serialized_header_bytes(total, name.as_str(), value)
            }
        })?;
    if other_bytes > max_other_header_bytes {
        return Err(ResponseBuildError::HeaderLimit);
    }
    own_header_storage(response)
}

fn own_header_storage(mut response: Response) -> Result<Response, ResponseBuildError> {
    let mut headers = HeaderMap::try_with_capacity(response.headers().len())
        .map_err(|_capacity| ResponseBuildError::HeaderLimit)?;
    // Both maps coexist during construction; final values must not retain
    // an ingress slice or map capacity left over from removed fields.
    for (name, value) in response.headers() {
        let owned_name = HeaderName::from_bytes(name.as_str().as_bytes())
            .map_err(|_name| ResponseBuildError::HeaderLimit)?;
        let mut owned = HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_value| ResponseBuildError::HeaderLimit)?;
        owned.set_sensitive(value.is_sensitive());
        headers
            .try_append(owned_name, owned)
            .map_err(|_capacity| ResponseBuildError::HeaderLimit)?;
    }
    *response.headers_mut() = headers;
    Ok(response)
}

pub enum SyncReadError {
    UnsupportedEncoding(String),
    BodyTooLong,
    InvalidContentLength,
    UnreadableBody,
    RequestTimeout(EdgeError),
}

impl fmt::Debug for SyncReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedEncoding(_) => f.write_str("UnsupportedEncoding"),
            Self::BodyTooLong => f.write_str("BodyTooLong"),
            Self::InvalidContentLength => f.write_str("InvalidContentLength"),
            Self::UnreadableBody => f.write_str("UnreadableBody"),
            Self::RequestTimeout(_) => f.write_str("RequestTimeout"),
        }
    }
}

impl SyncReadError {
    #[must_use]
    pub fn response(&self) -> Response {
        match self {
            Self::UnsupportedEncoding(label) => cookie_sync_text_response(
                StatusCode::BAD_REQUEST,
                format!("Invalid request: Content-Encoding of type {label} is not supported\n"),
            ),
            Self::BodyTooLong => cookie_sync_error(
                StatusCode::BAD_REQUEST,
                "cookie_sync request size exceeded max size of 32768 bytes.",
            ),
            Self::InvalidContentLength => cookie_sync_error(
                StatusCode::BAD_REQUEST,
                "Invalid request: invalid Content-Length",
            ),
            Self::UnreadableBody => cookie_sync_error(
                StatusCode::BAD_REQUEST,
                "Invalid request: body could not be read",
            ),
            Self::RequestTimeout(error) => {
                let mut response = render_error_response(error);
                *response.status_mut() = error.status();
                response
            }
        }
    }
}

/// Reads an unencoded request body within the cookie-sync body cap.
///
/// # Errors
/// Rejects unsupported encodings, invalid lengths, oversized bodies, and read failures,
/// preserving the SDK's request-timeout category.
pub async fn read_cookie_sync_body(
    headers: &HeaderMap,
    body: Body,
) -> Result<Vec<u8>, SyncReadError> {
    for value in headers.get_all(CONTENT_ENCODING) {
        if !value.is_empty() {
            return Err(SyncReadError::UnsupportedEncoding(safe_encoding_label(
                value,
            )));
        }
    }
    let mut lengths = headers.get_all(CONTENT_LENGTH).iter();
    if let Some(length) = lengths.next() {
        if lengths.next().is_some() {
            return Err(SyncReadError::InvalidContentLength);
        }
        let raw = length.as_bytes();
        if raw.is_empty() || !raw.iter().all(u8::is_ascii_digit) {
            return Err(SyncReadError::InvalidContentLength);
        }
        let declared = from_utf8(raw)
            .map_err(|_error| SyncReadError::InvalidContentLength)?
            .parse::<u64>()
            .unwrap_or(u64::MAX);
        if usize::try_from(declared).map_or(true, |declared_bytes| {
            declared_bytes > MAX_COOKIE_SYNC_BODY_BYTES
        }) {
            return Err(SyncReadError::BodyTooLong);
        }
    }
    body.into_bytes_bounded(MAX_COOKIE_SYNC_BODY_BYTES)
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| match error {
            EdgeError::RequestTimeout { .. } => SyncReadError::RequestTimeout(error),
            // Preserve cookie-sync's public 400 size response despite the
            // SDK collector now reporting an inbound overflow as typed 413.
            EdgeError::PayloadTooLarge {
                origin: RequestBodyOverflowOrigin::Inbound,
            } => SyncReadError::BodyTooLong,
            EdgeError::BadGateway { .. }
            | EdgeError::BadRequest { .. }
            | EdgeError::ConfigOutOfDate { .. }
            | EdgeError::GatewayTimeout { .. }
            | EdgeError::Internal { .. }
            | EdgeError::MethodNotAllowed { .. }
            | EdgeError::NotFound { .. }
            | EdgeError::NotImplemented { .. }
            | EdgeError::PayloadTooLarge { .. }
            | EdgeError::RequestHeaderFieldsTooLarge { .. }
            | EdgeError::ResponseTooLarge { .. }
            | EdgeError::ServiceUnavailable { .. }
            | EdgeError::StoreExtraction { .. }
            | EdgeError::UriTooLong { .. }
            | EdgeError::Validation { .. }
            | _ => SyncReadError::UnreadableBody,
        })
}

fn safe_encoding_label(value: &HeaderValue) -> String {
    let Ok(raw_label) = value.to_str() else {
        return "<unknown>".to_owned();
    };
    let label = raw_label.trim();
    if label.is_empty()
        || label.len() > 128
        || !label.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b',' | b' ')
        })
    {
        return "<unknown>".to_owned();
    }
    label.to_owned()
}

#[derive(Serialize)]
pub struct SyncUrl<'a> {
    pub url: &'a str,
    #[serde(rename = "type")]
    pub kind: SyncType,
}

#[derive(Serialize)]
pub struct BidderSyncStatus<'a> {
    pub bidder: &'a str,
    pub no_cookie: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usersync: Option<SyncUrl<'a>>,
}

#[derive(Serialize)]
pub struct SyncDebugStatus<'a> {
    pub bidder: &'a str,
    pub error: &'a str,
}

#[derive(Serialize)]
struct CookieSyncBody<'a> {
    status: &'static str,
    bidder_status: &'a [BidderSyncStatus<'a>],
    #[serde(skip_serializing_if = "Option::is_none")]
    debug: Option<&'a [SyncDebugStatus<'a>]>,
}

/// Serializes a bounded cookie-sync response with a trailing newline.
///
/// # Errors
/// Returns `BodyLimit` if the body exceeds the cap, or `Serialization` on JSON failure.
pub fn cookie_sync_success(
    cookie: &UidCookie,
    now: OffsetDateTime,
    bidder_status: &[BidderSyncStatus<'_>],
    debug: Option<&[SyncDebugStatus<'_>]>,
    max_body_bytes: usize,
) -> Result<Response, ResponseBuildError> {
    let payload = CookieSyncBody {
        status: if cookie.has_live_syncs(now) {
            "ok"
        } else {
            "no_cookie"
        },
        bidder_status,
        debug: debug.filter(|items| !items.is_empty()),
    };
    let body = bounded_json_line(&payload, max_body_bytes, false)?;
    let mut response = response(StatusCode::OK, body);
    response.headers_mut().insert(CONTENT_TYPE, JSON);
    Ok(response)
}

#[derive(Serialize)]
struct GetUidsBody<'a> {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    buyeruids: BTreeMap<&'a str, &'a str>,
}

/// Serializes the cookie's UIDs with HTML escaping and a trailing newline.
///
/// # Errors
/// Returns `BodyLimit` if the body exceeds the cap, or `Serialization` on JSON failure.
pub fn getuids_success(
    cookie: &UidCookie,
    max_body_bytes: usize,
) -> Result<Response, ResponseBuildError> {
    let payload = GetUidsBody {
        buyeruids: cookie.uids(),
    };
    let body = bounded_json_line(&payload, max_body_bytes, true)?;
    let mut response = response(StatusCode::OK, body);
    response.headers_mut().insert(CONTENT_TYPE, TEXT_PLAIN);
    Ok(response)
}

/// Builds a callback response and attaches an already validated cookie, if present.
///
/// # Errors
/// Currently infallible; retains the shared response-builder result interface.
pub fn setuid_success(
    format: Option<SyncResponseFormat>,
    prepared_cookie: Option<PreparedCookie>,
) -> Result<Response, ResponseBuildError> {
    let mut response = match format {
        Some(SyncResponseFormat::Image) => {
            let mut response = response(StatusCode::OK, PIXEL.to_vec());
            response
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static("image/png"));
            response
                .headers_mut()
                .insert(CONTENT_LENGTH, HeaderValue::from_static("86"));
            response
        }
        Some(SyncResponseFormat::Blank) => {
            let mut response = response(StatusCode::OK, Vec::new());
            response
                .headers_mut()
                .insert(CONTENT_TYPE, HeaderValue::from_static("text/html"));
            response
        }
        None => response(StatusCode::OK, Vec::new()),
    };
    if let Some(cookie) = prepared_cookie {
        response
            .headers_mut()
            .insert(SET_COOKIE, cookie.into_header_value());
    }
    Ok(response)
}

#[must_use]
pub fn setuid_error(status: StatusCode, message: &'static str) -> Response {
    response(status, message.as_bytes().to_vec())
}

#[must_use]
pub fn setuid_wire_error(error: WireError) -> Response {
    let message = match error {
        WireError::QueryTooLong => "query exceeds 32768 bytes",
        WireError::TooManyParameters => "too many query parameters",
        WireError::InvalidPercentEncoding | WireError::InvalidUtf8 => "invalid query encoding",
        WireError::BodyTooLong | WireError::InvalidJson | WireError::JsonNodeLimit => {
            "invalid request"
        }
    };
    setuid_error(StatusCode::BAD_REQUEST, message)
}

#[must_use]
pub fn setuid_warning() -> Response {
    setuid_error(
        StatusCode::OK,
        "Warning: syncer key is not a priority, and there are only priority elements left, cookie not updated",
    )
}

#[must_use]
pub fn cpu_overloaded() -> Response {
    response(StatusCode::SERVICE_UNAVAILABLE, b"overloaded\n".to_vec())
}

#[must_use]
pub fn cpu_overloaded_with_policy(
    policy: &SyncBrowserPolicy<'_>,
    request_headers: &HeaderMap,
    max_header_bytes: usize,
) -> Response {
    policy
        .decorate(cpu_overloaded(), request_headers, max_header_bytes)
        .unwrap_or_else(|_| cpu_overloaded())
}

#[must_use]
pub fn cookie_sync_error(status: StatusCode, message: &'static str) -> Response {
    cookie_sync_text_response(status, format!("{message}\n"))
}

#[must_use]
pub fn cookie_sync_wire_error(error: WireError) -> Response {
    let message = match error {
        WireError::BodyTooLong => "cookie_sync request size exceeded max size of 32768 bytes.",
        WireError::InvalidJson => "JSON parsing failed: invalid JSON",
        WireError::JsonNodeLimit => "JSON parsing failed: resource limit exceeded",
        WireError::QueryTooLong
        | WireError::TooManyParameters
        | WireError::InvalidPercentEncoding
        | WireError::InvalidUtf8 => "invalid request",
    };
    cookie_sync_error(StatusCode::BAD_REQUEST, message)
}

fn cookie_sync_text_response(status: StatusCode, body: String) -> Response {
    let mut response = response(status, body.into_bytes());
    response.headers_mut().insert(CONTENT_TYPE, TEXT_PLAIN);
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}

fn response(status: StatusCode, body: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from_bytes(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

struct BodyCount {
    len: usize,
    max: usize,
}

#[expect(
    clippy::missing_trait_methods,
    reason = "default Write helpers delegate to the capped counter; flush has no buffered side effects"
)]
impl io::Write for BodyCount {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.max.saturating_sub(self.len) {
            return Err(io::ErrorKind::WriteZero.into());
        }
        self.len = self
            .len
            .checked_add(buf.len())
            .ok_or(io::ErrorKind::WriteZero)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn sync_json_error(error: &serde_json::Error) -> ResponseBuildError {
    if error.io_error_kind() == Some(io::ErrorKind::WriteZero) {
        ResponseBuildError::BodyLimit
    } else {
        ResponseBuildError::Serialization
    }
}

fn reserve_body(bytes: usize) -> Result<Vec<u8>, ResponseBuildError> {
    let mut body = Vec::new();
    body.try_reserve_exact(bytes)
        .map_err(|_allocation| ResponseBuildError::Serialization)?;
    Ok(body)
}

fn html_fragment(ch: char, buffer: &mut [u8; 4]) -> &[u8] {
    match ch {
        '<' => b"\\u003c",
        '>' => b"\\u003e",
        '&' => b"\\u0026",
        '\u{2028}' => b"\\u2028",
        '\u{2029}' => b"\\u2029",
        _ => ch.encode_utf8(buffer).as_bytes(),
    }
}

fn bounded_json_line(
    value: &impl Serialize,
    max: usize,
    escape_html: bool,
) -> Result<Vec<u8>, ResponseBuildError> {
    let payload_cap = max.checked_sub(1).ok_or(ResponseBuildError::BodyLimit)?;
    // Refuse during counting, before reserving either output buffer.
    let mut count = BodyCount {
        len: 0,
        max: payload_cap,
    };
    serde_json::to_writer(&mut count, value).map_err(|error| sync_json_error(&error))?;
    let raw_capacity = count
        .len
        .checked_add(usize::from(!escape_html))
        .ok_or(ResponseBuildError::BodyLimit)?;
    let mut raw = reserve_body(raw_capacity)?;
    let mut raw_writer = CappedWriter::new(&mut raw, count.len)
        .map_err(|_capacity| ResponseBuildError::Serialization)?;
    serde_json::to_writer(&mut raw_writer, value).map_err(|error| sync_json_error(&error))?;
    if raw.len() != count.len {
        return Err(ResponseBuildError::Serialization);
    }
    let mut body = if escape_html {
        let raw_text = from_utf8(&raw).map_err(|_error| ResponseBuildError::Serialization)?;
        let mut escaped_count = BodyCount {
            len: 0,
            max: payload_cap,
        };
        let mut buffer = [0; 4];
        for ch in raw_text.chars() {
            escaped_count
                .write_all(html_fragment(ch, &mut buffer))
                .map_err(|_limit| ResponseBuildError::BodyLimit)?;
        }
        // Raw and escaped backing coexist until this transformation completes.
        let escaped_capacity = escaped_count
            .len
            .checked_add(1)
            .ok_or(ResponseBuildError::BodyLimit)?;
        let mut escaped = reserve_body(escaped_capacity)?;
        let mut writer = CappedWriter::new(&mut escaped, escaped_count.len)
            .map_err(|_capacity| ResponseBuildError::Serialization)?;
        for ch in raw_text.chars() {
            writer
                .write_all(html_fragment(ch, &mut buffer))
                .map_err(|_limit| ResponseBuildError::BodyLimit)?;
        }
        escaped
    } else {
        raw
    };
    body.push(b'\n');
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::{Response, ResponseBuildError, bounded_json_line};
    use crate::test_support::config_store::configured_sync_host;
    use crate::usersync::wire::{MAX_COOKIE_SYNC_BODY_BYTES, WireError};
    use crate::{
        handlers::sync_http::{
            BidderSyncStatus, SyncBrowserPolicy, SyncUrl, cookie_sync_error, cookie_sync_success,
            cookie_sync_wire_error, getuids_success, read_cookie_sync_body,
            serialized_header_bytes, setuid_error, setuid_success, setuid_warning,
            setuid_wire_error,
        },
        usersync::{
            cookie::{CookieWriteOptions, UidCookie, prepare_set_cookie_value},
            limits::CookieLimits,
        },
    };
    use auctioneer_config_data::usersync::{SyncResponseFormat, SyncType, UserSyncConfig};
    use bytes::Bytes;
    use edgezero_core::body::Body;
    use edgezero_core::error::{EdgeError, RequestBodyOverflowOrigin};
    use futures::{executor::block_on, stream};
    use http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, SET_COOKIE},
    };
    use std::{cell::Cell, io, rc::Rc, sync::OnceLock, task::Poll};
    use time::{OffsetDateTime, format_description::well_known::Rfc3339};

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-09-30T12:00:00Z", &Rfc3339).unwrap()
    }

    fn body_bytes(response: &http::Response<Body>) -> &[u8] {
        response.body().as_bytes().unwrap()
    }

    #[test]
    fn response_cap_helpers_use_the_shipped_body_and_header_ceilings() {
        assert_eq!(super::response_body_cap(), 256 * 1024);
        assert_eq!(super::response_header_cap(), 16 * 1024);
        assert_eq!(super::MAX_SYNC_RESPONSE_HEADER_COUNT, 16);
    }

    #[test]
    fn preread_checks_all_encodings_before_length_without_polling_body() {
        let mut headers = HeaderMap::new();
        headers.append(CONTENT_ENCODING, HeaderValue::from_static(""));
        headers.append(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("32769"));
        let polls = Rc::new(Cell::new(0_usize));
        let observed = Rc::clone(&polls);
        let body = Body::from_stream(stream::poll_fn(move |_| {
            observed.set(observed.get() + 1);
            Poll::Ready(None)
        }));
        let failure = block_on(read_cookie_sync_body(&headers, body)).unwrap_err();
        let response = failure.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body_bytes(&response),
            b"Invalid request: Content-Encoding of type gzip is not supported\n"
        );
        assert_eq!(polls.get(), 0);
    }

    #[test]
    fn preread_declared_and_streaming_body_caps_are_independent() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("32769"));
        let failure = block_on(read_cookie_sync_body(&headers, Body::empty())).unwrap_err();
        assert_eq!(failure.response().status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body_bytes(&failure.response()),
            b"cookie_sync request size exceeded max size of 32768 bytes.\n"
        );

        headers.insert(CONTENT_LENGTH, HeaderValue::from_static("32768"));
        let bytes = vec![b'x'; MAX_COOKIE_SYNC_BODY_BYTES];
        assert_eq!(
            block_on(read_cookie_sync_body(
                &headers,
                Body::from_bytes(bytes.clone())
            ))
            .unwrap()
            .len(),
            MAX_COOKIE_SYNC_BODY_BYTES
        );
        let oversized_failure = block_on(read_cookie_sync_body(
            &headers,
            Body::from_bytes(vec![b'x'; MAX_COOKIE_SYNC_BODY_BYTES + 1]),
        ))
        .unwrap_err();
        assert_eq!(
            oversized_failure.response().status(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            body_bytes(&oversized_failure.response()),
            b"cookie_sync request size exceeded max size of 32768 bytes.\n"
        );
    }

    #[test]
    fn encoding_labels_are_bounded_before_reflection_in_error_responses() {
        for (raw, expected) in [
            (b" gzip ".to_vec(), "gzip".to_owned()),
            (
                b"gzip, custom_1.2-3".to_vec(),
                "gzip, custom_1.2-3".to_owned(),
            ),
            (vec![b'a'; 128], "a".repeat(128)),
            (vec![b'a'; 129], "<unknown>".to_owned()),
            (b"   ".to_vec(), "<unknown>".to_owned()),
            (
                b"gzip<script>secret</script>".to_vec(),
                "<unknown>".to_owned(),
            ),
            (vec![0xff], "<unknown>".to_owned()),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_ENCODING, HeaderValue::from_bytes(&raw).unwrap());
            let body = Body::from_stream(stream::poll_fn(|_| {
                panic!("encoding rejection must precede body reads");
            }));
            let failure = block_on(read_cookie_sync_body(&headers, body)).unwrap_err();
            let response = failure.response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                body_bytes(&response),
                format!("Invalid request: Content-Encoding of type {expected} is not supported\n")
                    .as_bytes()
            );
            assert_eq!(format!("{failure:?}"), "UnsupportedEncoding");
        }
    }

    #[test]
    fn unreadable_stream_uses_a_fixed_error_without_echoing_transport_detail() {
        let secret = "private transport detail 1829";
        let body = Body::from_stream(stream::iter(vec![Err(EdgeError::internal(
            io::Error::other(secret),
        ))]));
        let failure = block_on(read_cookie_sync_body(&HeaderMap::new(), body)).unwrap_err();
        let response = failure.response();
        assert_eq!(
            body_bytes(&response),
            b"Invalid request: body could not be read\n"
        );
        assert!(!format!("{failure:?}").contains(secret));
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    }

    #[test]
    fn unrelated_body_source_errors_are_not_cookie_sync_size_failures() {
        let secret = "private body source detail";
        for error in [
            EdgeError::bad_request(secret),
            EdgeError::payload_too_large(RequestBodyOverflowOrigin::OutboundUpload),
        ] {
            let body = Body::from_stream(stream::iter([Err(error)]));
            let failure = block_on(read_cookie_sync_body(&HeaderMap::new(), body)).unwrap_err();
            let response = failure.response();
            assert_eq!(format!("{failure:?}"), "UnreadableBody");
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(
                body_bytes(&response),
                b"Invalid request: body could not be read\n"
            );
            assert!(!format!("{failure:?}").contains(secret));
            assert_eq!(response.headers()["cache-control"], "no-store");
        }
    }

    #[test]
    fn body_read_timeout_keeps_its_status_and_redacts_transport_detail() {
        let secret = "private body timeout detail";
        let body = Body::from_stream(stream::iter([Err(EdgeError::request_timeout(secret))]));
        let failure = block_on(read_cookie_sync_body(&HeaderMap::new(), body)).unwrap_err();
        let response = failure.response();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
        assert_eq!(body_bytes(&response), b"request timeout\n");
        assert_eq!(
            response.headers()[CONTENT_TYPE],
            "text/plain; charset=utf-8"
        );
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert!(!response.headers().contains_key(SET_COOKIE));
        assert!(!format!("{failure:?}").contains(secret));
    }

    #[test]
    fn encoding_and_declared_length_errors_precede_body_timeout_without_polling() {
        for (encoding, length, expected) in [
            (
                Some("gzip"),
                "32769",
                b"Invalid request: Content-Encoding of type gzip is not supported\n".as_slice(),
            ),
            (
                None,
                "32769",
                b"cookie_sync request size exceeded max size of 32768 bytes.\n".as_slice(),
            ),
            (
                None,
                "invalid",
                b"Invalid request: invalid Content-Length\n".as_slice(),
            ),
        ] {
            let mut headers = HeaderMap::new();
            if let Some(encoding_value) = encoding {
                headers.insert(CONTENT_ENCODING, HeaderValue::from_static(encoding_value));
            }
            headers.insert(CONTENT_LENGTH, HeaderValue::from_static(length));
            let body = Body::from_stream(stream::poll_fn(|_| {
                panic!("HTTP input checks must not poll a timed-out body");
            }));
            let failure = block_on(read_cookie_sync_body(&headers, body)).unwrap_err();
            let response = failure.response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_eq!(body_bytes(&response), expected);
        }
    }

    #[test]
    fn streaming_body_overflow_precedes_a_later_timeout() {
        let chunk = Body::from_bytes(vec![b'x'; MAX_COOKIE_SYNC_BODY_BYTES + 1])
            .into_bytes()
            .unwrap();
        let body = Body::from_stream(stream::iter([
            Ok(chunk),
            Err(EdgeError::request_timeout("later body timeout")),
        ]));
        let failure = block_on(read_cookie_sync_body(&HeaderMap::new(), body)).unwrap_err();
        let response = failure.response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body_bytes(&response),
            b"cookie_sync request size exceeded max size of 32768 bytes.\n"
        );
    }

    #[test]
    fn wire_failures_have_fixed_endpoint_specific_400_shapes() {
        let cookie_error = cookie_sync_wire_error(WireError::InvalidJson);
        assert_eq!(cookie_error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(
            body_bytes(&cookie_error),
            b"JSON parsing failed: invalid JSON\n"
        );
        assert_eq!(
            cookie_error.headers().get(CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        let size_error = cookie_sync_wire_error(WireError::BodyTooLong);
        assert_eq!(
            body_bytes(&size_error),
            b"cookie_sync request size exceeded max size of 32768 bytes.\n"
        );
        let query_error = setuid_wire_error(WireError::InvalidPercentEncoding);
        assert_eq!(query_error.status(), StatusCode::BAD_REQUEST);
        assert_eq!(body_bytes(&query_error), b"invalid query encoding");
        assert!(query_error.headers().get(CONTENT_TYPE).is_none());
        assert!(query_error.headers().get(SET_COOKIE).is_none());
    }

    #[test]
    fn cookie_sync_success_has_exact_json_shape_newline_and_no_html_escaping() {
        let cookie = UidCookie::default();
        let selected = [BidderSyncStatus {
            bidder: "fake_bbb",
            no_cookie: true,
            usersync: Some(SyncUrl {
                url: "https://example.test/?x=<>&",
                kind: SyncType::Iframe,
            }),
        }];
        let response = cookie_sync_success(&cookie, now(), &selected, None, 32 * 1024).unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/json; charset=utf-8"
        );
        // Literal exemption: field order, spelling, unescaped HTML characters,
        // and the final newline are the serialized response contract.
        assert_eq!(body_bytes(&response), br#"{"status":"no_cookie","bidder_status":[{"bidder":"fake_bbb","no_cookie":true,"usersync":{"url":"https://example.test/?x=<>&","type":"iframe"}}]}"#.iter().copied().chain(*b"\n").collect::<Vec<_>>());
        assert!(response.headers().get(SET_COOKIE).is_none());
        cookie_sync_success(&cookie, now(), &selected, None, 4).unwrap_err();
    }

    #[test]
    fn cookie_sync_empty_and_live_status_have_required_array_and_newline() {
        let mut cookie = UidCookie::default();
        let empty = cookie_sync_success(&cookie, now(), &[], None, 1024).unwrap();
        assert_eq!(
            body_bytes(&empty),
            b"{\"status\":\"no_cookie\",\"bidder_status\":[]}\n"
        );
        cookie
            .set_uid("fam_a", "uid", now(), &CookieLimits::default())
            .unwrap();
        let live = cookie_sync_success(&cookie, now(), &[], None, 1024).unwrap();
        assert_eq!(
            body_bytes(&live),
            b"{\"status\":\"ok\",\"bidder_status\":[]}\n"
        );
    }

    #[test]
    fn getuids_omits_empty_map_and_retains_expired_values_with_html_escaping() {
        let mut cookie = UidCookie::default();
        let empty = getuids_success(&cookie, 1024).unwrap();
        assert_eq!(
            empty.headers().get(CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(body_bytes(&empty), b"{}\n");
        cookie
            .set_uid(
                "fam_a",
                "<>&",
                now() - time::Duration::days(30),
                &CookieLimits::default(),
            )
            .unwrap();
        let expired = getuids_success(&cookie, 1024).unwrap();
        assert_eq!(
            body_bytes(&expired),
            b"{\"buyeruids\":{\"fam_a\":\"\\u003c\\u003e\\u0026\"}}\n"
        );
        assert!(expired.headers().get(SET_COOKIE).is_none());
    }

    #[test]
    fn setuid_formats_png_blank_and_default_without_normalizing_errors() {
        let prepared = prepare_set_cookie_value("", &CookieWriteOptions::default(), now()).unwrap();
        let image = setuid_success(Some(SyncResponseFormat::Image), Some(prepared)).unwrap();
        assert_eq!(
            body_bytes(&image),
            include_bytes!(
                "../../../auctioneer-bidder/tests/conformance/fixtures/usersync/pixel.png"
            )
        );
        assert_eq!(image.headers().get(CONTENT_TYPE).unwrap(), "image/png");
        assert_eq!(image.headers().get(CONTENT_LENGTH).unwrap(), "86");
        assert!(image.headers().get(SET_COOKIE).is_some());
        let blank = setuid_success(Some(SyncResponseFormat::Blank), None).unwrap();
        assert!(body_bytes(&blank).is_empty());
        assert_eq!(blank.headers().get(CONTENT_TYPE).unwrap(), "text/html");
        let default = setuid_success(None, None).unwrap();
        assert!(body_bytes(&default).is_empty());
        assert!(default.headers().get(CONTENT_TYPE).is_none());
        let error = setuid_error(
            StatusCode::BAD_REQUEST,
            "\"bidder\" query param is required",
        );
        assert_eq!(body_bytes(&error), b"\"bidder\" query param is required");
        assert!(error.headers().get(CONTENT_TYPE).is_none());
        assert!(error.headers().get(SET_COOKIE).is_none());
        let warning = setuid_warning();
        assert_eq!(warning.status(), StatusCode::OK);
        assert_eq!(body_bytes(&warning), b"Warning: syncer key is not a priority, and there are only priority elements left, cookie not updated");
        assert!(warning.headers().get(CONTENT_TYPE).is_none());
        assert!(warning.headers().get(SET_COOKIE).is_none());
    }

    #[test]
    fn getuids_body_bound_includes_html_expansion_and_final_newline() {
        let mut cookie = UidCookie::default();
        cookie
            .set_uid("fam_a", "<>&", now(), &CookieLimits::default())
            .unwrap();
        let expected = b"{\"buyeruids\":{\"fam_a\":\"\\u003c\\u003e\\u0026\"}}\n";
        for cap in [expected.len(), expected.len() + 1] {
            assert_eq!(
                body_bytes(&getuids_success(&cookie, cap).unwrap()),
                expected
            );
        }
        assert_eq!(
            getuids_success(&cookie, expected.len() - 1).unwrap_err(),
            ResponseBuildError::BodyLimit
        );
        assert_eq!(
            getuids_success(&cookie, 0).unwrap_err(),
            ResponseBuildError::BodyLimit
        );
    }

    #[test]
    fn cookie_sync_errors_keep_http_error_shape_and_nosniff() {
        let error = cookie_sync_error(StatusCode::UNAUTHORIZED, "User has opted out");
        assert_eq!(body_bytes(&error), b"User has opted out\n");
        assert_eq!(
            error.headers().get(CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );
        assert_eq!(
            error.headers().get("x-content-type-options").unwrap(),
            "nosniff"
        );
        assert!(error.headers().get(SET_COOKIE).is_none());
    }

    fn browser_policy() -> SyncBrowserPolicy<'static> {
        static CONFIG: OnceLock<UserSyncConfig> = OnceLock::new();
        let config = CONFIG.get_or_init(|| UserSyncConfig {
            allowed_origins: vec!["https://buyer.example".to_owned()],
            ..UserSyncConfig::default()
        });
        SyncBrowserPolicy::new(config)
    }

    fn browser_responses(request: &HeaderMap) -> [Response; 3] {
        let policy = browser_policy();
        let cap = super::response_header_cap();
        [
            policy
                .decorate(
                    getuids_success(&UidCookie::default(), 1024).unwrap(),
                    request,
                    cap,
                )
                .unwrap(),
            policy
                .decorate_setuid(setuid_success(None, None).unwrap(), request, cap)
                .unwrap(),
            policy.preflight(request, cap).unwrap(),
        ]
    }

    #[test]
    fn cors_origins_own_their_bytes_without_retaining_request_backings() {
        let wire = b"https://buyer.example";
        let mut retained = Vec::new();
        for backing_size in [16 * 1024_usize, 1024 * 1024] {
            let offset = backing_size.div_euclid(2);
            let mut storage = vec![b'x'; backing_size];
            storage[offset..offset + wire.len()].copy_from_slice(wire);
            let backing = Bytes::from(storage);
            let origin =
                HeaderValue::from_maybe_shared(backing.slice(offset..offset + wire.len())).unwrap();
            let start = backing.as_ptr().addr();
            let address_range = start..start + backing.len();
            assert_eq!(origin.as_bytes(), wire);
            assert!(
                address_range.contains(&origin.as_bytes().as_ptr().addr()),
                "the request header must share the {backing_size}-byte backing"
            );
            let mut request = HeaderMap::new();
            request.insert("origin", origin);
            request.insert(
                "access-control-request-method",
                HeaderValue::from_static("POST"),
            );
            request.insert(
                "access-control-request-headers",
                HeaderValue::from_static("content-type"),
            );
            let responses = browser_responses(&request);
            assert_eq!(responses[0].status(), StatusCode::OK);
            assert_eq!(responses[1].status(), StatusCode::OK);
            assert_eq!(responses[2].status(), StatusCode::NO_CONTENT);
            for (kind, response) in ["normal", "setuid", "preflight"].into_iter().zip(responses) {
                let allowed = response
                    .headers()
                    .get("access-control-allow-origin")
                    .unwrap();
                assert_eq!(allowed.as_bytes(), wire);
                assert_eq!(
                    response.headers()["access-control-allow-credentials"],
                    "true"
                );
                if address_range.contains(&allowed.as_bytes().as_ptr().addr()) {
                    retained.push((backing_size, kind));
                }
            }
            assert_eq!(&backing[offset..offset + wire.len()], wire);
            assert_eq!(request["origin"].as_bytes().as_ptr().addr(), start + offset);
        }
        assert!(
            retained.is_empty(),
            "response origins retained request backings: {retained:?}"
        );
    }

    #[test]
    fn denied_duplicate_and_invalid_origins_stay_denied_for_every_decorator() {
        for origins in [
            vec![],
            vec![HeaderValue::from_static("https://unapproved.example")],
            vec![HeaderValue::from_static("null")],
            vec![HeaderValue::from_static("https://buyer.example/path")],
            vec![HeaderValue::from_static("https://BUYER.example")],
            vec![HeaderValue::from_bytes(b"\xff").unwrap()],
            vec![
                HeaderValue::from_static("https://buyer.example"),
                HeaderValue::from_static("https://buyer.example"),
            ],
        ] {
            let mut request = HeaderMap::new();
            for origin in origins {
                request.append("origin", origin);
            }
            request.insert(
                "access-control-request-method",
                HeaderValue::from_static("POST"),
            );
            request.insert(
                "access-control-request-headers",
                HeaderValue::from_static("content-type"),
            );
            let responses = browser_responses(&request);
            assert_eq!(responses[0].status(), StatusCode::OK);
            assert_eq!(responses[1].status(), StatusCode::OK);
            assert_eq!(responses[2].status(), StatusCode::FORBIDDEN);
            for response in responses {
                assert!(
                    !response
                        .headers()
                        .contains_key("access-control-allow-origin")
                );
                assert!(
                    !response
                        .headers()
                        .contains_key("access-control-allow-credentials")
                );
                assert_eq!(response.headers()["cache-control"], "no-store");
            }
        }
    }

    #[test]
    fn allowed_exact_origin_gets_credentials_and_vary_without_losing_existing_vary() {
        let mut request = HeaderMap::new();
        request.insert("origin", HeaderValue::from_static("https://buyer.example"));
        let mut undecorated_response =
            cookie_sync_error(StatusCode::BAD_REQUEST, "fixed application error");
        undecorated_response
            .headers_mut()
            .insert("vary", HeaderValue::from_static("Accept-Encoding"));
        let response = browser_policy()
            .decorate(undecorated_response, &request, 8 * 1024)
            .unwrap();
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "https://buyer.example"
        );
        assert_eq!(
            response
                .headers()
                .get("access-control-allow-credentials")
                .unwrap(),
            "true"
        );
        let vary: Vec<_> = response
            .headers()
            .get_all("vary")
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect();
        assert!(vary.contains(&"Accept-Encoding"));
        assert!(
            vary.iter()
                .any(|value| value.eq_ignore_ascii_case("Origin"))
        );
        assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        assert_eq!(body_bytes(&response), b"fixed application error\n");
    }

    #[test]
    fn malformed_duplicate_and_unapproved_origins_get_no_cors_permission() {
        for origin in [
            "null",
            "https://buyer.example.evil",
            "https://buyer.example/path",
            "https://BUYER.example",
        ] {
            let mut request = HeaderMap::new();
            request.insert("origin", HeaderValue::from_str(origin).unwrap());
            let response = browser_policy()
                .decorate(
                    getuids_success(&UidCookie::default(), 1024).unwrap(),
                    &request,
                    8 * 1024,
                )
                .unwrap();
            assert!(
                response
                    .headers()
                    .get("access-control-allow-origin")
                    .is_none(),
                "{origin}"
            );
            assert!(
                response
                    .headers()
                    .get("access-control-allow-credentials")
                    .is_none(),
                "{origin}"
            );
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        }
        let mut duplicate = HeaderMap::new();
        duplicate.append("origin", HeaderValue::from_static("https://buyer.example"));
        duplicate.append("origin", HeaderValue::from_static("https://buyer.example"));
        let response = browser_policy()
            .decorate(
                getuids_success(&UidCookie::default(), 1024).unwrap(),
                &duplicate,
                8 * 1024,
            )
            .unwrap();
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
    }

    #[test]
    fn policy_removes_stale_allow_headers_before_denied_origin() {
        let mut request = HeaderMap::new();
        request.insert(
            "origin",
            HeaderValue::from_static("https://unapproved.example"),
        );
        let mut stale_response = getuids_success(&UidCookie::default(), 1024).unwrap();
        stale_response.headers_mut().insert(
            "access-control-allow-origin",
            HeaderValue::from_static("https://buyer.example"),
        );
        stale_response.headers_mut().insert(
            "access-control-allow-credentials",
            HeaderValue::from_static("true"),
        );
        let response = browser_policy()
            .decorate(stale_response, &request, 8 * 1024)
            .unwrap();
        assert!(
            response
                .headers()
                .get("access-control-allow-origin")
                .is_none()
        );
        assert!(
            response
                .headers()
                .get("access-control-allow-credentials")
                .is_none()
        );
    }

    #[test]
    fn preflight_authorizes_only_post_and_content_type_for_an_allowlisted_origin() {
        let mut request = HeaderMap::new();
        request.insert("origin", HeaderValue::from_static("https://buyer.example"));
        request.insert(
            "access-control-request-method",
            HeaderValue::from_static("POST"),
        );
        request.insert(
            "access-control-request-headers",
            HeaderValue::from_static("content-type"),
        );
        let allowed = browser_policy().preflight(&request, 8 * 1024).unwrap();
        assert_eq!(allowed.status(), StatusCode::NO_CONTENT);
        assert!(body_bytes(&allowed).is_empty());
        assert_eq!(
            allowed
                .headers()
                .get("access-control-allow-origin")
                .unwrap(),
            "https://buyer.example"
        );
        assert_eq!(
            allowed
                .headers()
                .get("access-control-allow-credentials")
                .unwrap(),
            "true"
        );
        assert_eq!(
            allowed
                .headers()
                .get("access-control-allow-methods")
                .unwrap(),
            "POST"
        );
        assert_eq!(
            allowed
                .headers()
                .get("access-control-allow-headers")
                .unwrap(),
            "Content-Type"
        );
        assert_eq!(allowed.headers().get("cache-control").unwrap(), "no-store");
        assert!(allowed.headers().get(SET_COOKIE).is_none());

        for (method, headers) in [
            ("GET", "content-type"),
            ("POST", "x-token"),
            ("POST", "content-type, x-token"),
        ] {
            request.insert(
                "access-control-request-method",
                HeaderValue::from_str(method).unwrap(),
            );
            request.insert(
                "access-control-request-headers",
                HeaderValue::from_str(headers).unwrap(),
            );
            let denied = browser_policy().preflight(&request, 8 * 1024).unwrap();
            assert_eq!(denied.status(), StatusCode::FORBIDDEN);
            assert!(
                denied
                    .headers()
                    .get("access-control-allow-origin")
                    .is_none()
            );
            assert!(
                denied
                    .headers()
                    .get("access-control-allow-credentials")
                    .is_none()
            );
            assert_eq!(denied.headers().get("cache-control").unwrap(), "no-store");
        }
    }

    #[test]
    fn complete_header_bound_is_inclusive_and_failed_decoration_drops_cookie_response() {
        let mut request = HeaderMap::new();
        request.insert("origin", HeaderValue::from_static("https://buyer.example"));
        let initial_prepared =
            prepare_set_cookie_value("", &CookieWriteOptions::default(), now()).unwrap();
        let response =
            setuid_success(Some(SyncResponseFormat::Image), Some(initial_prepared)).unwrap();
        let decorated = browser_policy()
            .decorate(response, &request, 8 * 1024)
            .unwrap();
        let exact = serialized_header_bytes(decorated.headers()).unwrap();
        let at_limit_prepared =
            prepare_set_cookie_value("", &CookieWriteOptions::default(), now()).unwrap();
        browser_policy()
            .decorate(
                setuid_success(Some(SyncResponseFormat::Image), Some(at_limit_prepared)).unwrap(),
                &request,
                exact,
            )
            .unwrap();
        let over_limit_prepared =
            prepare_set_cookie_value("", &CookieWriteOptions::default(), now()).unwrap();
        browser_policy()
            .decorate(
                setuid_success(Some(SyncResponseFormat::Image), Some(over_limit_prepared)).unwrap(),
                &request,
                exact - 1,
            )
            .unwrap_err();
    }

    #[test]
    fn every_sync_builder_marks_personalized_responses_no_store() {
        let cookie = UidCookie::default();
        let responses = [
            cookie_sync_success(&cookie, now(), &[], None, 1024).unwrap(),
            cookie_sync_error(StatusCode::BAD_REQUEST, "fixed"),
            getuids_success(&cookie, 1024).unwrap(),
            setuid_success(None, None).unwrap(),
            setuid_error(StatusCode::UNAUTHORIZED, ""),
            setuid_warning(),
        ];
        for response in responses {
            assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
        }
    }

    use auctioneer_runtime::aux_work::MAX_SYNC_OUTPUT_BYTES;

    #[test]
    fn sync_json_line_reserves_only_the_complete_final_body() {
        for value in ["", "abc", "x".repeat(127).as_str(), "<>&\u{2028}\u{2029}"] {
            for escape_html in [false, true] {
                let body = bounded_json_line(&value, 4096, escape_html).unwrap();
                assert_eq!(body.capacity(), body.len(), "{value:?}, {escape_html}");
                for cap in [body.len(), body.len().checked_add(23).unwrap()] {
                    assert_eq!(bounded_json_line(&value, cap, escape_html).unwrap(), body);
                }
                assert_eq!(
                    bounded_json_line(&value, body.len().checked_sub(1).unwrap(), escape_html),
                    Err(ResponseBuildError::BodyLimit)
                );
            }
        }
    }

    #[test]
    fn sync_json_line_keeps_all_html_json_and_newline_escapes() {
        let input = "<>&\u{2028}\u{2029}\"\\\n\0\u{e9}";
        let expected = "\"\\u003c\\u003e\\u0026\\u2028\\u2029\\\"\\\\\\n\\u0000\u{e9}\"\n";
        assert_eq!(
            bounded_json_line(&input, expected.len(), true).unwrap(),
            expected.as_bytes()
        );
        assert_eq!(
            bounded_json_line(&input, expected.len().checked_sub(1).unwrap(), true),
            Err(ResponseBuildError::BodyLimit)
        );
    }

    struct ChangingString<'a> {
        first: &'a str,
        second: &'a str,
        calls: Cell<usize>,
    }

    impl serde::Serialize for ChangingString<'_> {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let call = self.calls.replace(self.calls.get().checked_add(1).unwrap());
            serializer.serialize_str(if call == 0 { self.first } else { self.second })
        }
    }

    #[test]
    fn sync_json_line_refuses_serialization_growth_and_shrinkage() {
        for escape_html in [false, true] {
            for (first, second, error) in [
                ("a", "longer", ResponseBuildError::BodyLimit),
                ("longer", "a", ResponseBuildError::Serialization),
            ] {
                let value = ChangingString {
                    first,
                    second,
                    calls: Cell::new(0),
                };
                assert_eq!(bounded_json_line(&value, 128, escape_html), Err(error));
                assert_eq!(value.calls.get(), 2);
            }
        }
    }

    struct VisitedSequence {
        entries: usize,
        visited: Cell<usize>,
    }

    impl serde::Serialize for VisitedSequence {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            use serde::ser::SerializeSeq as _;
            let mut sequence = serializer.serialize_seq(Some(self.entries))?;
            for _ in 0..self.entries {
                self.visited.set(self.visited.get().checked_add(1).unwrap());
                sequence.serialize_element("x")?;
            }
            sequence.end()
        }
    }

    #[test]
    fn sync_json_counting_stops_at_the_first_overflow_before_output_reservation() {
        use auctioneer_runtime::test_support::alloc_track::allocated;
        for escape_html in [false, true] {
            let value = VisitedSequence {
                entries: 1024,
                visited: Cell::new(0),
            };
            let before = allocated();
            let result = bounded_json_line(&value, 2048, escape_html);
            let allocated_bytes = allocated().checked_sub(before).unwrap();
            assert_eq!(result, Err(ResponseBuildError::BodyLimit));
            assert!(value.visited.get() > 0 && value.visited.get() < value.entries);
            assert!(
                allocated_bytes < 512,
                "overflow allocated {allocated_bytes} bytes"
            );

            value.visited.set(0);
            let body = bounded_json_line(&value, 8192, escape_html).unwrap();
            assert_eq!(value.visited.get(), value.entries.checked_mul(2).unwrap());
            assert_eq!(body.len(), 4098);
        }
    }

    #[test]
    fn sync_json_line_checks_newline_capacity_before_visiting_input() {
        let value = VisitedSequence {
            entries: 1,
            visited: Cell::new(0),
        };
        for escape_html in [false, true] {
            assert_eq!(
                bounded_json_line(&value, 0, escape_html),
                Err(ResponseBuildError::BodyLimit)
            );
            assert_eq!(value.visited.get(), 0);
        }
    }

    #[test]
    fn sync_html_overflow_never_reserves_the_escaped_output() {
        use auctioneer_runtime::test_support::alloc_track::allocated;
        for input_bytes in [1024, 8192] {
            let value = "<".repeat(input_bytes);
            let before = allocated();
            let result = bounded_json_line(&value, input_bytes.checked_mul(2).unwrap(), true);
            let allocated_bytes = allocated().checked_sub(before).unwrap();
            assert_eq!(result, Err(ResponseBuildError::BodyLimit));
            assert!(
                allocated_bytes
                    <= u64::try_from(input_bytes.checked_add(2 + 256).unwrap()).unwrap(),
                "only raw JSON plus fixed error scratch is allowed: {allocated_bytes}"
            );
        }
    }

    #[test]
    fn sync_json_line_keeps_genuine_serializer_errors_distinct_from_limits() {
        struct Broken;
        impl serde::Serialize for Broken {
            fn serialize<S: serde::Serializer>(&self, _serializer: S) -> Result<S::Ok, S::Error> {
                use serde::ser::Error as _;
                Err(S::Error::custom("fixed serialization failure"))
            }
        }
        for escape_html in [false, true] {
            assert_eq!(
                bounded_json_line(&Broken, 128, escape_html),
                Err(ResponseBuildError::Serialization)
            );
        }
    }

    #[test]
    fn json_output_cap_includes_newline_and_never_exposes_a_partial_body() {
        let cap = usize::try_from(MAX_SYNC_OUTPUT_BYTES).unwrap();
        let empty = serde_json::json!({"debug": ""});
        let overhead = serde_json::to_vec(&empty).unwrap().len() + 1;
        let exact = serde_json::json!({"debug": "x".repeat(cap - overhead)});
        let body = bounded_json_line(&exact, cap, false).unwrap();
        assert_eq!(body.len(), cap);
        assert_eq!(body.last(), Some(&b'\n'));
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap(),
            exact
        );

        let over = serde_json::json!({"debug": "x".repeat(cap - overhead + 1)});
        assert_eq!(
            bounded_json_line(&over, cap, false),
            Err(ResponseBuildError::BodyLimit)
        );
        let escaped = serde_json::json!({"debug": "\0".repeat(cap.div_euclid(6))});
        assert_eq!(
            bounded_json_line(&escaped, cap, false),
            Err(ResponseBuildError::BodyLimit)
        );
    }

    #[test]
    fn setuid_decorator_keeps_other_headers_bounded() {
        let host = configured_sync_host();
        let policy = SyncBrowserPolicy::new(host.user_sync.as_ref().unwrap());
        let mut response = Response::new(Body::empty());
        response
            .headers_mut()
            .insert("set-cookie", HeaderValue::from_static("uids=x"));
        response.headers_mut().insert(
            "x-extra",
            HeaderValue::from_str(&"x".repeat(16 * 1024)).unwrap(),
        );

        assert!(matches!(
            policy.decorate_setuid(response, &HeaderMap::new(), 16 * 1024),
            Err(ResponseBuildError::HeaderLimit)
        ));
    }

    #[test]
    fn sync_header_bounds_detach_backing_and_discard_removed_map_capacity() {
        for backing_bytes in [16 * 1024, 1024 * 1024] {
            for value_bytes in [17, 271] {
                for setuid in [false, true] {
                    let backing = Bytes::from(vec![b'x'; backing_bytes]);
                    let mut shared = HeaderValue::from_maybe_shared(
                        backing.slice(32..32_usize.checked_add(value_bytes).unwrap()),
                    )
                    .unwrap();
                    shared.set_sensitive(true);
                    let start = backing.as_ptr().addr();
                    let addresses = start..start.checked_add(backing.len()).unwrap();
                    assert!(addresses.contains(&shared.as_bytes().as_ptr().addr()));
                    let mut response = Response::new(Body::from_bytes(b"unchanged".to_vec()));
                    *response.status_mut() = StatusCode::ACCEPTED;
                    *response.headers_mut() = HeaderMap::with_capacity(1024);
                    response
                        .headers_mut()
                        .insert("x-removed", HeaderValue::from_static("removed"));
                    assert!(response.headers_mut().remove("x-removed").is_some());
                    for _ in 0_usize..3 {
                        response.headers_mut().append("x-shared", shared.clone());
                    }
                    response
                        .headers_mut()
                        .append("vary", HeaderValue::from_static("Origin"));
                    response
                        .headers_mut()
                        .append("vary", HeaderValue::from_static("Accept-Encoding"));
                    if setuid {
                        response.headers_mut().insert(
                            SET_COOKIE,
                            HeaderValue::from_static("uids=payload; HttpOnly"),
                        );
                    }
                    let before = response.headers().clone();
                    assert!(before.capacity() >= 1024);
                    let fresh_capacity =
                        HeaderMap::<HeaderValue>::with_capacity(before.len()).capacity();
                    let bounded = if setuid {
                        super::bound_setuid_headers(response, 4096)
                    } else {
                        super::bound_headers(response, 4096)
                    }
                    .unwrap();
                    assert_eq!(bounded.status(), StatusCode::ACCEPTED);
                    assert_eq!(body_bytes(&bounded), b"unchanged");
                    assert_eq!(bounded.headers(), &before);
                    assert!(bounded.headers().capacity() <= fresh_capacity);
                    assert_eq!(bounded.headers().get_all("x-shared").iter().count(), 3);
                    for value in bounded.headers().get_all("x-shared") {
                        assert!(value.is_sensitive());
                        assert!(!addresses.contains(&value.as_bytes().as_ptr().addr()));
                    }
                }
            }
        }
    }

    #[test]
    fn browser_policy_enforces_application_header_count_after_cors_decoration() {
        let policy = browser_policy();
        let mut request = HeaderMap::new();
        request.insert("origin", HeaderValue::from_static("https://buyer.example"));
        let cap = usize::try_from(super::MAX_SYNC_RESPONSE_HEADER_COUNT).unwrap();
        let policy_headers = policy
            .decorate(
                Response::new(Body::empty()),
                &request,
                super::response_header_cap(),
            )
            .unwrap()
            .headers()
            .len();
        for setuid in [false, true] {
            for final_count in [cap - 1, cap, cap + 1] {
                let mut response = Response::new(Body::empty());
                if setuid {
                    response
                        .headers_mut()
                        .insert(SET_COOKIE, HeaderValue::from_static("uids=x"));
                }
                for _ in 0..final_count - policy_headers - usize::from(setuid) {
                    response
                        .headers_mut()
                        .append("x-extra", HeaderValue::from_static("x"));
                }
                let result = if setuid {
                    policy.decorate_setuid(response, &request, super::response_header_cap())
                } else {
                    policy.decorate(response, &request, super::response_header_cap())
                };
                if final_count <= cap {
                    assert_eq!(result.unwrap().headers().len(), final_count);
                } else {
                    assert_eq!(result.unwrap_err(), ResponseBuildError::HeaderLimit);
                }
            }
        }
    }
}
