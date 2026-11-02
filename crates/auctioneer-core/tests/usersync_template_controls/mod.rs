//! Rust-authored renderer controls, not captured reference-server outputs.
//! Each row varies one macro; encoded values are literal wire expectations.

pub(super) const PRIVACY_MACRO_CONTROLS: &[(&str, &str, &str)] = &[
    (
        "GDPR",
        "1&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment",
        "1%26forged_param%3D1%26uid%3DSTOLEN%2526%2B%7B%7B.UserMacro%7D%7D%23fragment",
    ),
    (
        "GDPRConsent",
        "CPabc&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment",
        "CPabc%26forged_param%3D1%26uid%3DSTOLEN%2526%2B%7B%7B.UserMacro%7D%7D%23fragment",
    ),
    (
        "USPrivacy",
        "1YNN&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment",
        "1YNN%26forged_param%3D1%26uid%3DSTOLEN%2526%2B%7B%7B.UserMacro%7D%7D%23fragment",
    ),
    (
        "GPP",
        "DBABTA~opaque&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment",
        "DBABTA%7Eopaque%26forged_param%3D1%26uid%3DSTOLEN%2526%2B%7B%7B.UserMacro%7D%7D%23fragment",
    ),
    (
        "GPPSID",
        "07,8&forged_param=1&uid=STOLEN%26+{{.UserMacro}}#fragment",
        "07%2C8%26forged_param%3D1%26uid%3DSTOLEN%2526%2B%7B%7B.UserMacro%7D%7D%23fragment",
    ),
];
