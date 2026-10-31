//! `RequestHeaders` — HTTP-header signals consumed by `prepare()`.

/// HTTP-header signals `prepare()` needs for implicit-field fill.
/// Trusted-server callers without an HTTP request set every field to `None`.
#[derive(Clone, Copy, Debug, Default)]
pub struct RequestHeaders<'a> {
    pub referer: Option<&'a str>,
    pub user_agent: Option<&'a str>,
    pub dnt: Option<&'a str>,
    pub sec_gpc: Option<&'a str>,
    pub true_client_ip: Option<&'a str>,
    pub x_forwarded_for: Option<&'a str>,
    pub x_real_ip: Option<&'a str>,
    pub remote_addr: Option<&'a str>,
    /// `Content-Encoding` is rejected by the caller before reaching
    /// `prepare()`, but trusted-server callers may pass it through for
    /// parity tests.
    pub content_encoding: Option<&'a str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all_none() {
        let headers = RequestHeaders::default();
        assert!(headers.referer.is_none());
        assert!(headers.user_agent.is_none());
        assert!(headers.remote_addr.is_none());
    }
}
