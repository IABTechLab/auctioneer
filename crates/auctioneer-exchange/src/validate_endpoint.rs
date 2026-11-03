//! Exchange-owned outbound URL policy, shared by the sink and convenience API.

use auctioneer_bidder::{
    BidderError, BidderRequestInfo, EndpointRejectReason, EndpointValidator,
    MAX_RETAINED_SINK_ERRORS,
};
use auctioneer_config_data::types::BidderInfo;
use std::cell::RefCell;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Validate a URL using the same policy the construction sink enforces.
///
/// # Errors
/// Returns code 5 with fixed category text, never raw URL or parser details.
pub fn validate_endpoint(url: &str, info: &BidderInfo) -> Result<(), BidderError> {
    if url.contains("{{.") {
        return Err(public_error(EndpointRejectReason::TemplateUnresolved));
    }
    let parsed = url::Url::parse(url)
        .map_err(|_parse_error| public_error(EndpointRejectReason::MalformedUrl))?;
    let allow = info
        .host_allowlist
        .as_ref()
        .map(|hosts| hosts.iter().map(String::as_str).collect::<Vec<_>>());
    validate_parsed(&parsed, &info.endpoint, allow.as_deref()).map_err(public_error)
}

fn public_error(reason: EndpointRejectReason) -> BidderError {
    let message = match reason {
        EndpointRejectReason::SchemeNotAllowed => "outbound scheme not allowed",
        EndpointRejectReason::HostNotAllowed => "outbound host not allowed",
        EndpointRejectReason::IpLiteralRejected => "outbound IP literal not allowed",
        EndpointRejectReason::TemplateUnresolved => "outbound template unresolved",
        EndpointRejectReason::PortNotAllowed => "outbound port not allowed",
        EndpointRejectReason::UserinfoRejected => "outbound userinfo not allowed",
        EndpointRejectReason::AuthorityMismatch => "outbound authority mismatch",
        EndpointRejectReason::MalformedUrl => "outbound URL malformed",
        _ => "outbound endpoint rejected",
    };
    BidderError::failed_to_request_bids(message)
}

fn validate_parsed(
    parsed: &url::Url,
    template: &str,
    allow: Option<&[&str]>,
) -> Result<(), EndpointRejectReason> {
    match parsed.scheme() {
        "https" => {}
        #[cfg(feature = "insecure-bidders")]
        "http" => {}
        _ => return Err(EndpointRejectReason::SchemeNotAllowed),
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(EndpointRejectReason::UserinfoRejected);
    }
    if parsed.as_str().contains("{{.") {
        return Err(EndpointRejectReason::TemplateUnresolved);
    }
    let host = parsed
        .host_str()
        .ok_or(EndpointRejectReason::MalformedUrl)?;
    let configured = template.replace("{{.Host}}", "auctioneer-host-stand-in.invalid");
    let info_url =
        url::Url::parse(&configured).map_err(|_parse_error| EndpointRejectReason::MalformedUrl)?;
    if !parsed.scheme().eq_ignore_ascii_case(info_url.scheme()) {
        return Err(EndpointRejectReason::SchemeNotAllowed);
    }
    if let Some(hosts) = allow
        && !hosts
            .iter()
            .any(|allowed| host.eq_ignore_ascii_case(allowed))
    {
        return Err(EndpointRejectReason::HostNotAllowed);
    }
    if allow.is_none()
        && (!host.eq_ignore_ascii_case(info_url.host_str().unwrap_or_default())
            || parsed.port() != info_url.port())
    {
        return Err(EndpointRejectReason::AuthorityMismatch);
    }
    let bare_host = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .unwrap_or(host);
    #[cfg(feature = "insecure-bidders")]
    if matches!(bare_host, "127.0.0.1" | "::1") || bare_host.eq_ignore_ascii_case("localhost") {
        return Ok(());
    }
    if is_loopback_literal(bare_host) || is_localhost_alias_host(bare_host) {
        return Err(EndpointRejectReason::HostNotAllowed);
    }
    if is_private_ip_literal(bare_host) {
        return Err(EndpointRejectReason::IpLiteralRejected);
    }
    Ok(())
}

fn is_loopback_literal(host: &str) -> bool {
    host == "127.0.0.1" || host == "::1" || host.eq_ignore_ascii_case("localhost")
}

fn is_localhost_alias_host(host: &str) -> bool {
    // Same rule as `build_support::is_localhost_alias_host`. Default
    // builds reject `*.localhost` per RFC 6761 plus the well-known
    // wildcard-DNS services that always resolve to loopback.
    let lower = host.to_ascii_lowercase();
    if lower.ends_with(".localhost") {
        return true;
    }
    for apex in ["nip.io", "xip.io", "lvh.me", "localtest.me", "vcap.me"] {
        if lower == apex {
            return true;
        }
        if let Some(prefix) = lower.strip_suffix(apex)
            && prefix.ends_with('.')
        {
            return true;
        }
    }
    false
}

fn is_private_ip_literal(host: &str) -> bool {
    if let Ok(v4) = host.parse::<Ipv4Addr>() {
        return is_reserved_ipv4(v4);
    }
    if let Ok(v6) = host.parse::<Ipv6Addr>() {
        return is_reserved_ipv6(v6);
    }
    false
}

fn is_reserved_ipv4(v4: Ipv4Addr) -> bool {
    if v4.is_loopback()
        || v4.is_private()
        || v4.is_link_local()
        || v4.is_unspecified()
        || v4.is_broadcast()
        || v4.is_documentation()
        || v4.is_multicast()
    {
        return true;
    }
    let octets = v4.octets();
    // 0.0.0.0/8 — "this network" (RFC 791).
    if octets[0] == 0 {
        return true;
    }
    // 100.64.0.0/10 — carrier-grade NAT (RFC 6598).
    if octets[0] == 100 && (64..=127).contains(&octets[1]) {
        return true;
    }
    // 192.0.0.0/24 — IETF protocol assignments (RFC 6890).
    if octets[0] == 192 && octets[1] == 0 && octets[2] == 0 {
        return true;
    }
    // 198.18.0.0/15 — benchmarking (RFC 2544).
    if octets[0] == 198 && (octets[1] == 18 || octets[1] == 19) {
        return true;
    }
    // 240.0.0.0/4 — reserved (former Class E, RFC 1112).
    if octets[0] >= 240 {
        return true;
    }
    false
}

fn is_reserved_ipv6(v6: Ipv6Addr) -> bool {
    if v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local() || v6.is_multicast() {
        return true;
    }
    let segs = v6.segments();
    // Link-local fe80::/10.
    if segs[0] & 0xffc0 == 0xfe80 {
        return true;
    }
    // Documentation 2001:db8::/32 (RFC 3849).
    if segs[0] == 0x2001 && segs[1] == 0x0db8 {
        return true;
    }
    // IPv4-mapped ::ffff:0:0/96 — re-validate the embedded v4.
    if let Some(mapped) = v6.to_ipv4_mapped() {
        return is_reserved_ipv4(mapped);
    }
    false
}

/// The exchange's [`EndpointValidator`], handed to one bidder's
/// outbound sink.
///
/// Exists so the sink can refuse a URL BEFORE the body closure runs,
/// which is the only way an endpoint rejection costs nothing: a
/// pipeline that validated the URL on an already-built request had
/// already paid for the body it is about to discard.
///
/// Retained diagnostics use fixed categories, never raw authorities or
/// hosts. The auction maps sink rejections to its fixed public error;
/// bounded category diagnostics remain available to direct callers.
pub struct ExchangeEndpointValidator<'info> {
    info: &'info BidderInfo,
    /// The message from each rejection, in rejection order, BOUNDED at
    /// [`MAX_RETAINED_SINK_ERRORS`].
    ///
    /// `RefCell` because the trait takes `&self` and one sink is driven
    /// sequentially by one bidder on one thread.
    rejections: RefCell<Vec<String>>,
}

impl<'info> ExchangeEndpointValidator<'info> {
    /// Open a validator over one bidder's configured surface.
    #[must_use]
    pub const fn new(info: &'info BidderInfo) -> Self {
        Self {
            info,
            rejections: RefCell::new(Vec::new()),
        }
    }

    /// The rejection messages recorded so far, in order, leaving the
    /// validator empty.
    ///
    /// At most [`MAX_RETAINED_SINK_ERRORS`] of them, which is the same
    /// window the sink keeps its own `EndpointRejected` errors in.
    /// Keeping the FIRST N preserves construction order past the window.
    #[must_use]
    pub fn take_rejections(&self) -> Vec<String> {
        self.rejections.take()
    }
}

impl EndpointValidator for ExchangeEndpointValidator<'_> {
    fn validate(
        &self,
        url: &url::Url,
        _request_info: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        match validate_endpoint(url.as_str(), self.info) {
            Ok(()) => Ok(()),
            Err(err) => {
                // BOUNDED, like the sink's own error list and for the
                // same reason. The sink keeps at most
                // `MAX_RETAINED_SINK_ERRORS` typed errors and counts the
                // rest; this kept every message a validator ever
                // produced. A multi-attempt adapter that pushed a
                // thousand rejected calls retained a thousand messages
                // against the sink's sixteen, and the extra 984 were
                // allocated, held for the whole construction, and then
                // dropped unread — `run_make_requests` only ever draws
                // one message per RETAINED sink error.
                //
                // Latent rather than live: every stock adapter pushes
                // once per bidder. It is a construction-time bound all
                // the same, and the one input that decides how many
                // times `push_request` is called is adapter code.
                //
                // Keep-first, not keep-last, so position N on this side
                // still describes position N on the sink's. Dropping the
                // oldest would silently re-pair every message with a
                // different rejection.
                let mut rejections = self.rejections.borrow_mut();
                if rejections.len() < MAX_RETAINED_SINK_ERRORS {
                    rejections.push(err.message);
                }
                drop(rejections);
                // The deployment policy this validator enforces is
                // "the bidder's configured surface", and every check
                // inside `validate_endpoint` is a facet of that one
                // decision rather than of a distinguishable class the
                // sink would act on differently. The concrete cause
                // travels in the captured message.
                Err(EndpointRejectReason::AuthorityMismatch)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExchangeEndpointValidator, MAX_RETAINED_SINK_ERRORS};
    use auctioneer_bidder::{BidderName, BidderRequestInfo, EndpointValidator as _, EntryPoint};
    use auctioneer_config_data::types::BidderInfo;

    /// A bidder whose configured surface rejects everything this test
    /// offers, so every `validate` call is a rejection.
    fn refusing_info() -> BidderInfo {
        BidderInfo {
            name: "fake_ax".into(),
            endpoint: "https://hb.example.test/openrtb".into(),
            uses_https: true,
            host_allowlist: None,
            ..BidderInfo::default()
        }
    }

    fn request_info(currency: &[String]) -> BidderRequestInfo<'_> {
        BidderRequestInfo::new(
            BidderName::new("fake_ax"),
            BidderName::new("fake_ax"),
            "acct",
            None,
            "https://hb.example.test/openrtb",
            &[],
            None,
            None,
            EntryPoint::Auction,
            currency,
        )
    }

    /// The captured rejection messages are BOUNDED at the same window
    /// the sink keeps its own errors in.
    ///
    /// `OutboundRequestSink` retains at most
    /// [`MAX_RETAINED_SINK_ERRORS`] typed build errors and counts the
    /// rest, precisely so an adapter looping `push_request` cannot grow
    /// memory through the error list. This validator kept every message
    /// it ever produced, so the same loop retained one `String` per
    /// attempt against the sink's sixteen — allocated, held for the
    /// whole construction, then dropped unread, because
    /// `run_make_requests` draws one message per RETAINED sink error and
    /// no more.
    ///
    /// Latent today: every stock adapter pushes once per bidder. The
    /// bound is a construction-time property all the same, and what
    /// decides the attempt count is adapter code.
    ///
    /// The count driven here is deliberately far past the window, so the
    /// row fails loudly against an unbounded list rather than by one.
    #[test]
    fn captured_rejections_are_bounded_by_the_sinks_own_window() {
        const ATTEMPTS: usize = 1_000;

        let info = refusing_info();
        let validator = ExchangeEndpointValidator::new(&info);
        let currency: Vec<String> = vec!["USD".to_owned()];
        let request = request_info(&currency);
        // A URL outside the configured surface, so every call rejects.
        let url = url::Url::parse("https://evil.example.test/openrtb").expect("a valid URL");
        for _ in 0..ATTEMPTS {
            assert!(
                validator.validate(&url, &request).is_err(),
                "the fixture URL must be refused on every attempt, or the row counts \
                 acceptances and measures nothing",
            );
        }
        let captured = validator.take_rejections();
        assert_eq!(
            captured.len(),
            MAX_RETAINED_SINK_ERRORS,
            "{ATTEMPTS} rejected attempts must retain at most \
             {MAX_RETAINED_SINK_ERRORS} messages -- the window the sink keeps its own \
             EndpointRejected errors in. Retaining one per attempt allocates \
             messages that run_make_requests can never read, because it draws one \
             per retained sink error",
        );
    }

    /// The retained messages are the FIRST ones, in order.
    ///
    /// Position is load-bearing: `run_make_requests` pairs the Nth
    /// retained sink error with the Nth retained message, and the sink
    /// keeps its first N. Dropping the oldest here instead would re-pair
    /// every message with a different rejection and report each failure
    /// against the wrong URL.
    #[test]
    fn the_retained_messages_are_the_first_ones_in_order() {
        let info = refusing_info();
        let validator = ExchangeEndpointValidator::new(&info);
        let currency: Vec<String> = vec!["USD".to_owned()];
        let request = request_info(&currency);
        // Distinct fixed categories expose ordering without retaining
        // caller-controlled hosts or credentials in diagnostics.
        let cases = [
            (
                "ftp://evil.example.test/openrtb",
                "outbound scheme not allowed",
            ),
            (
                "https://user@evil.example.test/openrtb",
                "outbound userinfo not allowed",
            ),
            (
                "https://evil.example.test/openrtb",
                "outbound authority mismatch",
            ),
        ];
        for (raw, _) in cases.iter().cycle().take(MAX_RETAINED_SINK_ERRORS + 4) {
            let url = url::Url::parse(raw).expect("a valid URL");
            let _rejected = validator.validate(&url, &request);
        }
        let captured = validator.take_rejections();
        assert_eq!(captured.len(), MAX_RETAINED_SINK_ERRORS, "the window holds");
        for (index, (message, (_, expected))) in
            captured.iter().zip(cases.iter().cycle()).enumerate()
        {
            assert_eq!(
                message, expected,
                "retained slot {index} must describe attempt {index} \
                 ({expected}), because run_make_requests pairs it with the sink's \
                 own {index}th retained error. It says: {message}",
            );
        }
    }

    /// And a run inside the window keeps everything, so the bound does
    /// not silently drop messages a real auction produces.
    #[test]
    fn a_run_inside_the_window_retains_every_message() {
        let info = refusing_info();
        let validator = ExchangeEndpointValidator::new(&info);
        let currency: Vec<String> = vec!["USD".to_owned()];
        let request = request_info(&currency);
        let url = url::Url::parse("https://evil.example.test/openrtb").expect("a valid URL");
        for _ in 0..3_usize {
            let _rejected = validator.validate(&url, &request);
        }
        assert_eq!(
            validator.take_rejections().len(),
            3,
            "three rejections are inside the window and must all be retained; the \
             bound is a ceiling, not a truncation of the ordinary case",
        );
    }
}
