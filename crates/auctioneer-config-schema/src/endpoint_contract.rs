//! Build-time vocabulary shared by manifest admission and template rendering.

use std::collections::BTreeSet;

/// Check that placeholder names are unique identifiers outside the reserved vocabulary.
#[must_use]
pub fn valid_placeholders<'a, I>(names: I) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = BTreeSet::new();
    names.into_iter().all(|name| {
        name.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
            && !matches!(
                name,
                "Host"
                    | "GDPR"
                    | "GDPRConsent"
                    | "USPrivacy"
                    | "GPP"
                    | "GPPSID"
                    | "RedirectURL"
                    | "ExternalURL"
                    | "SyncerKey"
                    | "BidderName"
                    | "SyncType"
                    | "UserMacro"
            )
            && seen.insert(name)
    })
}
