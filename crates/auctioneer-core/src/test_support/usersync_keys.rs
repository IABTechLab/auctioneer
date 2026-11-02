//! Public synthetic keys for tests only; never a deployment default.
use crate::usersync::{
    cookie::{DecodeOutcome, UidCookie},
    limits::CookieLimits,
    signed::{CookieKeyring, SignedCookieError},
};

// Literal exemption: shared const input to a secret provider; json! is not const.
pub const SECRET_JSON: &str = r#"{"active":"test","keys":[{"id":"test","key":"AQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE"}]}"#;

/// Parse the public, synthetic signing key.
///
/// # Panics
/// Panics if the fixed test key is not a valid keyring.
#[must_use]
pub fn keys() -> CookieKeyring {
    CookieKeyring::parse(SECRET_JSON.as_bytes()).expect("synthetic keyring")
}

/// Sign a cookie with the public test key.
///
/// # Errors
/// Returns the codec error if the payload exceeds its limits or cannot be encoded.
pub fn encode_cookie(
    cookie: &UidCookie,
    limits: &CookieLimits,
) -> Result<String, SignedCookieError> {
    keys().encode_cookie(cookie, limits)
}

/// Authenticate a cookie with the public test key.
///
/// # Errors
/// Returns the codec error for invalid signatures, malformed input or resource limits.
pub fn decode_cookie(
    encoded: &str,
    limits: &CookieLimits,
) -> Result<DecodeOutcome, SignedCookieError> {
    keys().decode_cookie(encoded, limits)
}
