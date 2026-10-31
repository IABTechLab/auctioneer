//! Existing user-sync sentinel policy, independent of a serving factory.

// Moved from core's codec: upstream usersync/cookie.go at revision
// d7f8515b86258688304b0d9b6668c6a0e258bc9e; its source SHA-256 is pinned in
// conformance/fixtures/usersync/sources.json. This does not qualify an adapter.
pub(super) const FAMILY: &str = "audienceNetwork";

pub(super) fn accepts_uid(uid: &str) -> bool {
    uid != "0"
}

#[cfg(test)]
mod tests {
    use super::{FAMILY, accepts_uid};

    #[test]
    fn family_and_exact_zero_sentinel_are_pinned() {
        assert_eq!(FAMILY, "audienceNetwork");
        assert!(!accepts_uid("0"));
        for uid in ["", "00", " 0", "0 ", "1", "opaque-user"] {
            assert!(accepts_uid(uid), "{uid:?}");
        }
    }

    #[test]
    fn generated_dispatch_applies_only_to_the_exact_family() {
        assert!(!crate::accepts_sync_uid(FAMILY, "0"));
        assert!(crate::accepts_sync_uid(FAMILY, ""));
        for family in ["audiencenetwork", "AudienceNetwork", "fake_bidder", ""] {
            assert!(crate::accepts_sync_uid(family, "0"), "{family:?}");
        }
    }
}
