//! The outbound-URL validation seam.
//!
//! `auctioneer-bidder` owns the trait and the typed rejection reason; the
//! exchange supplies the implementation that knows the deployment's
//! scheme, host and IP-literal policy. Keeping the reason typed here
//! means the sink can reject a request without formatting a message and
//! without depending on the exchange.

use crate::request_info::BidderRequestInfo;

/// Why an expanded outbound URL was refused.
///
/// `#[non_exhaustive]` so a new deployment policy can add a reason
/// without breaking external matches; the in-crate exhaustiveness pin
/// lives in this module's tests.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointRejectReason {
    /// The scheme is not permitted by this build.
    SchemeNotAllowed,
    /// The host is not a member of the bidder's allowlist.
    HostNotAllowed,
    /// The authority is a raw IP literal.
    IpLiteralRejected,
    /// A `{{.Macro}}` placeholder survived expansion.
    TemplateUnresolved,
    /// The explicit port is not permitted.
    PortNotAllowed,
    /// The URL did not parse.
    MalformedUrl,
    /// The authority carried userinfo.
    UserinfoRejected,
    /// The expanded authority does not match the configured one.
    AuthorityMismatch,
}

/// Validates an expanded outbound URL against a bidder's configured
/// surface, before the request is dispatched.
pub trait EndpointValidator {
    /// Accept or reject `url` for the bidder described by `info`.
    ///
    /// # Errors
    /// The [`EndpointRejectReason`] the deployment's policy assigns.
    fn validate(
        &self,
        url: &url::Url,
        info: &BidderRequestInfo<'_>,
    ) -> Result<(), EndpointRejectReason>;
}

#[cfg(test)]
mod tests {
    use super::{EndpointRejectReason, EndpointValidator};
    use crate::bidder::EntryPoint;
    use crate::request_info::BidderRequestInfo;
    use crate::types::BidderName;

    /// Test double standing in for the exchange's real validator.
    struct RejectingValidator(EndpointRejectReason);

    impl EndpointValidator for RejectingValidator {
        fn validate(
            &self,
            _url: &url::Url,
            _info: &BidderRequestInfo<'_>,
        ) -> Result<(), EndpointRejectReason> {
            Err(self.0)
        }
    }

    fn info<'a>(macros: &'a [&'a str], currency: &'a [String]) -> BidderRequestInfo<'a> {
        BidderRequestInfo::new(
            BidderName::new("fake_generic"),
            BidderName::new("fake_generic"),
            "acct-1",
            None,
            "https://example.test/openrtb",
            macros,
            None,
            None,
            EntryPoint::Auction,
            currency,
        )
    }

    /// Compile-time pin: a wildcard-free match over every reason. A new
    /// variant fails this match rather than falling silently into a
    /// catch-all arm.
    fn describe(reason: EndpointRejectReason) -> &'static str {
        match reason {
            EndpointRejectReason::SchemeNotAllowed => "scheme",
            EndpointRejectReason::HostNotAllowed => "host",
            EndpointRejectReason::IpLiteralRejected => "ip-literal",
            EndpointRejectReason::TemplateUnresolved => "template",
            EndpointRejectReason::PortNotAllowed => "port",
            EndpointRejectReason::MalformedUrl => "malformed",
            EndpointRejectReason::UserinfoRejected => "userinfo",
            EndpointRejectReason::AuthorityMismatch => "authority",
        }
    }

    #[test]
    fn endpoint_reject_reason_is_exhaustive_over_validate_failures() {
        let all = [
            EndpointRejectReason::SchemeNotAllowed,
            EndpointRejectReason::HostNotAllowed,
            EndpointRejectReason::IpLiteralRejected,
            EndpointRejectReason::TemplateUnresolved,
            EndpointRejectReason::PortNotAllowed,
            EndpointRejectReason::MalformedUrl,
            EndpointRejectReason::UserinfoRejected,
            EndpointRejectReason::AuthorityMismatch,
        ];
        let described: Vec<&str> = all.iter().copied().map(describe).collect();
        assert_eq!(described.len(), 8);

        let macros: &[&str] = &[];
        let currency: &[String] = &[];
        let request_info = info(macros, currency);
        let url = url::Url::parse("https://example.test/openrtb").unwrap();
        for reason in all {
            let validator = RejectingValidator(reason);
            assert_eq!(validator.validate(&url, &request_info), Err(reason));
        }
    }
}
