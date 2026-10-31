//! Constructors for non-fatal privacy resolution warnings.
//!
//! Code, scope, and message live here so every emitter agrees on the
//! taxonomy. Dynamic arguments are bounded numeric revision identifiers,
//! never raw request values. Source-specific malformed consent diagnostics
//! are constructed by the normalized source, without input interpolation.
//!
//! Two conditions the resolver handles emit NO warning and so have no
//! constructor here. Absent or empty `user.consent` is the "no opt-in
//! collected yet" convention rather than a defect (§7.2), and an invalid
//! `regs.gdpr` value with no `gpp_sid` is a hard `PrivacyError`
//! (§7.1) that never reaches the response.

use auctioneer_openrtb::{NonFatalWarning, WarningCode};

/// The bundled Global CMP List is past its freshness window, so no TC
/// string's CMP can be authenticated (§7). Carries no scope: this is a
/// deployment condition rather than a request one, and naming a request
/// field would point an integrator at input that is not at fault. The
/// fix is rebuilding against a fresh GVL archive.
#[must_use]
pub fn gcl_stale() -> NonFatalWarning {
    NonFatalWarning {
        code: WarningCode::GclStale,
        message: "the bundled Global CMP List is past its freshness window; CMP authenticity \
                  cannot be established, so consent is unusable"
            .to_owned(),
        scope: None,
    }
}

/// The GVL revision the TC string declared cannot be evaluated against:
/// either the bundled archive has no blob for `(spec, list)`, or the
/// blob it has disagrees with the string about the TCF policy version
/// (§7.3).
///
/// The message must not read as permissive. The behaviour is
/// enforced-but-empty, so identifiers are denied, and an operator who
/// read "permissive defaults" would draw the opposite conclusion.
#[must_use]
pub fn gvl_version_unavailable(spec: u32, list: u32) -> NonFatalWarning {
    NonFatalWarning {
        code: WarningCode::GvlVersionUnavailable,
        message: format!(
            "GVL version (spec {spec}, list {list}) is not in the bundled archive; applying \
             enforced-but-empty defaults (identifiers denied)"
        ),
        scope: Some("user.consent".to_owned()),
    }
}

/// The TC string names a CMP that the Global CMP List does not carry,
/// or one whose registration was deleted (§7). Authenticity cannot be
/// established, so consent is unusable and the request falls back to the
/// default permission row.
#[must_use]
pub fn invalid_privacy_consent() -> NonFatalWarning {
    NonFatalWarning {
        code: WarningCode::InvalidPrivacyConsent,
        message: "user.consent names a CMP that is absent from the Global CMP List or whose \
                  registration was deleted; treating as no consent"
            .to_owned(),
        scope: Some("user.consent".to_owned()),
    }
}

/// `regs.gdpr` present alongside `regs.gpp_sid` (§7.1). Diagnostic
/// only: it fires for any `regs.gdpr` value, including a legal 0/1, and
/// never changes the resolution order. An invalid `regs.gdpr` *value*
/// with an absent/empty `gpp_sid` is a hard error instead, and emits no
/// warning at all.
#[must_use]
pub fn regs_gpp_conflict() -> NonFatalWarning {
    NonFatalWarning {
        code: WarningCode::InvalidPrivacyConsent,
        message: "regs.gdpr set together with regs.gpp_sid; gpp_sid presence disambiguates when \
                  non-empty, else regs.gdpr wins"
            .to_owned(),
        scope: Some("regs.gpp_sid".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::{gcl_stale, gvl_version_unavailable, invalid_privacy_consent, regs_gpp_conflict};
    use crate::PrivacyContext;
    use auctioneer_openrtb::WarningCode;

    #[test]
    fn take_warnings_drains_the_buffer() {
        let mut ctx = PrivacyContext::allow_all_no_scrub();
        ctx.warnings.push(regs_gpp_conflict());
        ctx.warnings.push(invalid_privacy_consent());

        let drained = ctx.take_warnings();
        assert_eq!(drained.len(), 2);
        assert_eq!(
            drained.first().map(|w| w.scope.as_deref()),
            Some(Some("regs.gpp_sid"))
        );
        assert_eq!(
            drained.get(1).map(|w| w.scope.as_deref()),
            Some(Some("user.consent"))
        );
        assert!(
            drained
                .iter()
                .all(|w| w.code == WarningCode::InvalidPrivacyConsent)
        );
    }

    #[test]
    fn take_warnings_returns_empty_for_stub_context() {
        let mut ctx = PrivacyContext::allow_all_no_scrub();
        assert!(ctx.take_warnings().is_empty());
    }

    #[test]
    fn take_warnings_is_idempotent_second_call_empty() {
        let mut ctx = PrivacyContext::allow_all_no_scrub();
        ctx.warnings.push(invalid_privacy_consent());
        assert_eq!(ctx.take_warnings().len(), 1);
        assert!(ctx.take_warnings().is_empty());
    }

    #[test]
    fn deployment_warnings_have_no_request_scope() {
        // A stale GCL is a deployment condition, so no request field is
        // at fault and the scope stays absent.
        let stale = gcl_stale();
        assert_eq!(stale.code, WarningCode::GclStale);
        assert_eq!(stale.code.as_u32(), 10019_u32);
        assert!(stale.scope.is_none());
        // Every other constructor does name a field, so an absent scope
        // is a deliberate choice rather than an unset default.
        assert!(regs_gpp_conflict().scope.is_some());
        assert!(invalid_privacy_consent().scope.is_some());
    }

    #[test]
    fn gvl_version_unavailable_names_the_pair_and_denies_identifiers() {
        let warning = gvl_version_unavailable(3, 348);
        assert_eq!(warning.code, WarningCode::GvlVersionUnavailable);
        assert_eq!(warning.code.as_u32(), 10018_u32);
        assert!(warning.message.contains("spec 3, list 348"));
        assert!(warning.message.contains("identifiers denied"));
        // "Permissive" would tell an operator identifiers still flow.
        assert!(!warning.message.contains("permissive"));
        assert_eq!(warning.scope.as_deref(), Some("user.consent"));
    }
}
