//! Boot source-resolution suite: the `(token,
//! allow_unauthenticated_health)` decision table the adapter runs at
//! boot over its env-resolved inputs. The table itself is
//! `auctioneer_core::host_primitives::decide_health_policy` — shared by
//! every adapter — so these tests exercise the exact decision `main`
//! installs, composed with the strict flag parse and the
//! empty-secret-rejecting token constructor, WITHOUT writing the
//! process-global policy `OnceLock` (the install itself is covered by
//! auctioneer-core's per-policy-state integration binaries).

#[cfg(test)]
mod tests {
    use auctioneer_core::host_primitives::{
        BearerToken, HealthAccessPolicy, HealthBootError, decide_health_policy, read_bool_flag,
    };

    const ALLOW_UNAUTH_VAR: &str = "AUCTIONEER__RUNTIME__ALLOW_UNAUTHENTICATED_HEALTH";

    #[test]
    fn env_token_installs_token_policy() {
        let token = Some("secret").and_then(BearerToken::new);
        let allow_unauth = read_bool_flag(ALLOW_UNAUTH_VAR, None).expect("absent flag is false");
        assert!(!allow_unauth);
        let policy = decide_health_policy(token, allow_unauth).expect("token boot succeeds");
        match policy {
            HealthAccessPolicy::Token(auth) => {
                assert!(auth.verify("secret"), "the resolved token must verify");
                assert!(!auth.verify("wrong"));
            }
            HealthAccessPolicy::UnauthenticatedAcknowledged | HealthAccessPolicy::Closed => {
                panic!("a resolved token must install the Token policy")
            }
        }
    }

    #[test]
    fn env_flag_true_no_token_installs_acknowledged() {
        let allow_unauth =
            read_bool_flag(ALLOW_UNAUTH_VAR, Some("true")).expect("strict true parses");
        assert!(allow_unauth);
        let policy = decide_health_policy(None, allow_unauth).expect("acknowledged boot succeeds");
        assert!(
            matches!(policy, HealthAccessPolicy::UnauthenticatedAcknowledged),
            "flag-true with no token must install the explicit acknowledgement"
        );
        // The flag parse is strict: anything but "true"/"false" is a
        // boot error, never a silent false.
        assert!(matches!(
            read_bool_flag(ALLOW_UNAUTH_VAR, Some("yes")),
            Err(HealthBootError::NonBooleanFlag { .. })
        ));
        assert_eq!(
            read_bool_flag(ALLOW_UNAUTH_VAR, Some(" true ")).ok(),
            Some(true),
            "surrounding whitespace is trimmed before the strict parse"
        );
    }

    #[test]
    fn token_plus_flag_is_boot_error() {
        let token = BearerToken::new("secret");
        assert!(
            matches!(
                decide_health_policy(token, true),
                Err(HealthBootError::ConflictingPolicy)
            ),
            "a token AND the unauthenticated opt-in is conflicting intent - boot must fail"
        );
    }

    #[test]
    fn neither_installs_no_policy() {
        // An empty secret maps to "no token configured" first.
        let token = Some("").and_then(BearerToken::new);
        assert!(token.is_none());
        let policy = decide_health_policy(token, false).expect("fail-closed boot succeeds");
        assert!(
            matches!(policy, HealthAccessPolicy::Closed),
            "no token and no opt-in must freeze the fail-closed Closed policy"
        );
    }
}
