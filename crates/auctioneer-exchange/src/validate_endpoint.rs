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
    let template = info
        .endpoint
        .as_deref()
        .ok_or_else(|| public_error(EndpointRejectReason::MalformedUrl))?;
    validate_endpoint_policy(url, template, info.host_allowlist.as_deref())
}

fn validate_endpoint_policy(
    url: &str,
    template: &str,
    hosts: Option<&[String]>,
) -> Result<(), BidderError> {
    if url.contains("{{.") {
        return Err(public_error(EndpointRejectReason::TemplateUnresolved));
    }
    let parsed = url::Url::parse(url)
        .map_err(|_parse_error| public_error(EndpointRejectReason::MalformedUrl))?;
    let allow =
        hosts.map(|allowed_hosts| allowed_hosts.iter().map(String::as_str).collect::<Vec<_>>());
    validate_parsed(&parsed, template, allow.as_deref()).map_err(public_error)
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
        record_validation_result(validate_endpoint(url.as_str(), self.info), &self.rejections)
    }
}

/// Borrow the exchange's immutable view after checked settings binding.
/// The bidder-supplied view passed to `validate` cannot widen this policy.
pub(crate) struct BoundEndpointValidator<'info> {
    policy: BoundEndpointPolicy<'info>,
    rejections: RefCell<Vec<String>>,
}

enum BoundEndpointPolicy<'info> {
    Metadata(&'info BidderInfo),
    Configured(&'info BidderRequestInfo<'info>),
}

impl<'info> BoundEndpointValidator<'info> {
    pub(crate) const fn new(
        info: &'info BidderInfo,
        bound: &'info BidderRequestInfo<'info>,
        has_configured_endpoint: bool,
    ) -> Self {
        Self {
            policy: if has_configured_endpoint {
                BoundEndpointPolicy::Configured(bound)
            } else {
                BoundEndpointPolicy::Metadata(info)
            },
            rejections: RefCell::new(Vec::new()),
        }
    }

    #[cfg(test)]
    fn take_rejections(&self) -> Vec<String> {
        self.rejections.take()
    }
}

impl EndpointValidator for BoundEndpointValidator<'_> {
    fn validate(
        &self,
        url: &url::Url,
        _request_info: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason> {
        let result = match self.policy {
            BoundEndpointPolicy::Metadata(info) => validate_endpoint(url.as_str(), info),
            BoundEndpointPolicy::Configured(info) => {
                validate_parsed(url, info.endpoint_template, None).map_err(public_error)
            }
        };
        record_validation_result(result, &self.rejections)
    }
}

fn record_validation_result(
    result: Result<(), BidderError>,
    rejections: &RefCell<Vec<String>>,
) -> Result<(), EndpointRejectReason> {
    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            // Match the sink's bounded, keep-first window so repeated attempts
            // cannot retain unbounded diagnostics or change rejection order.
            let mut retained = rejections.borrow_mut();
            if retained.len() < MAX_RETAINED_SINK_ERRORS {
                retained.push(err.message);
            }
            drop(retained);
            // The sink acts on one deployment-policy rejection category;
            // direct callers can inspect the fixed messages retained above.
            Err(EndpointRejectReason::AuthorityMismatch)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BoundEndpointValidator, ExchangeEndpointValidator, MAX_RETAINED_SINK_ERRORS};
    use auctioneer_bidder::{BidderName, BidderRequestInfo, EndpointValidator as _, EntryPoint};
    use auctioneer_config_data::types::BidderInfo;

    #[test]
    fn configured_endpoint_captures_authority_independently_of_request_info() {
        use auctioneer_bidder::ConfiguredEndpoint;

        let selected =
            ConfiguredEndpoint::new("https://configured.example.test:8443/auction?q=1").unwrap();
        let currency = Vec::new();
        let mut bound = request_info(&currency);
        bound.endpoint_template = selected.as_str();
        bound.host_allowlist = Some(&["configured.example.test", "forged.example.test"]);
        let info = refusing_info();
        let validator = BoundEndpointValidator::new(&info, &bound, true);
        let mut supplied = request_info(&currency);
        supplied.endpoint_template = "https://forged.example.test/auction";
        supplied.host_allowlist = Some(&["forged.example.test"]);

        let accepted = url::Url::parse("https://configured.example.test:8443/auction?q=1").unwrap();
        validator.validate(&accepted, &supplied).unwrap();
        for raw in [
            "https://forged.example.test:8443/auction",
            "https://configured.example.test/auction",
            "https://configured.example.test:8444/auction",
            "http://configured.example.test:8443/auction",
            "https://user@configured.example.test:8443/auction",
            "https://127.0.0.2:8443/auction",
        ] {
            let rejected = url::Url::parse(raw).unwrap();
            assert!(validator.validate(&rejected, &supplied).is_err(), "{raw}");
        }
        assert_eq!(validator.take_rejections().len(), 6);
    }

    #[test]
    fn configured_endpoint_rejects_before_body_construction_and_refunds_the_call() {
        use auctioneer_bidder::{CallBudget, ConfiguredEndpoint, OutboundRequestSink, SinkLimits};
        use std::cell::Cell;

        let selected = ConfiguredEndpoint::new("https://configured.example.test/auction").unwrap();
        let currency = Vec::new();
        let mut bound = request_info(&currency);
        bound.endpoint_template = selected.as_str();
        let info = refusing_info();
        let validator = BoundEndpointValidator::new(&info, &bound, true);
        let supplied = request_info(&currency);
        let budget = CallBudget::new(1);
        let body_writes = Cell::new(0_usize);
        let mut sink = OutboundRequestSink::new(SinkLimits::new(1024, 1), &budget, &validator);
        let result = sink.push_request(
            &supplied,
            |meta| {
                meta.set_method("POST")?;
                meta.set_url("https://forged.example.test/auction")?;
                meta.imp_id("i")
            },
            |writer| {
                body_writes.set(body_writes.get() + 1_usize);
                writer.write_all(b"{}")
            },
        );
        assert!(result.is_err());
        assert_eq!(body_writes.get(), 0_usize);
        assert_eq!(budget.remaining(), 1);
        assert!(sink.into_outcome().accepted.is_empty());
    }

    #[test]
    fn bound_metadata_keeps_the_declared_host_allowlist() {
        let currency = Vec::new();
        let info = BidderInfo {
            endpoint: Some("https://{{.Host}}/auction".into()),
            host_allowlist: Some(vec!["east.example.test".into(), "west.example.test".into()]),
            ..refusing_info()
        };
        let bound = request_info(&currency);
        let validator = BoundEndpointValidator::new(&info, &bound, false);
        let mut supplied = request_info(&currency);
        supplied.endpoint_template = "https://forged.example.test/auction";
        supplied.host_allowlist = Some(&["forged.example.test"]);

        for raw in [
            "https://east.example.test/auction",
            "https://west.example.test/auction",
        ] {
            validator
                .validate(&url::Url::parse(raw).unwrap(), &supplied)
                .unwrap();
        }
        for raw in [
            "https://forged.example.test/auction",
            "http://east.example.test/auction",
        ] {
            assert!(
                validator
                    .validate(&url::Url::parse(raw).unwrap(), &supplied)
                    .is_err()
            );
        }
        assert_eq!(validator.take_rejections().len(), 2);
    }

    /// A bidder whose configured surface rejects everything this test
    /// offers, so every `validate` call is a rejection.
    fn refusing_info() -> BidderInfo {
        BidderInfo {
            name: "fake_ax".into(),
            endpoint: Some("https://hb.example.test/openrtb".into()),
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
