//! Bidder-local legacy privacy signals, applied after policy scrubbing.

use auctioneer_openrtb::types::bid_request::BidRequest;
use auctioneer_openrtb::types::regs::Regs;
use auctioneer_openrtb::types::user::User;
use auctioneer_privacy::sources::LegacyProjection;

/// Sanitize legacy USP for every bidder, then downgrade only non-GPP bidders.
/// Source selection and validation belong to normalization, not this mutation.
pub(crate) fn apply_legacy_projection(
    request: &mut BidRequest,
    projection: LegacyProjection<'_>,
    gpp_supported: bool,
) {
    if projection.clear_legacy_usp
        && let Some(regs) = request.regs.as_mut()
    {
        regs.us_privacy = None;
    }
    if gpp_supported {
        return;
    }

    if let Some(gdpr) = projection.gdpr {
        let regs = request.regs.get_or_insert_with(Regs::default);
        if regs.gdpr.is_none() {
            regs.gdpr = Some(gdpr);
        }
    }
    if let Some(consent) = projection.consent {
        let user = request.user.get_or_insert_with(User::default);
        if user.consent.as_deref().is_none_or(str::is_empty) {
            user.consent = Some(consent.to_owned());
        }
    }
    if let Some(usp) = projection.usp {
        let regs = request.regs.get_or_insert_with(Regs::default);
        if regs.us_privacy.as_deref().is_none_or(str::is_empty) {
            // Usp guarantees four ASCII bytes, so no fallible UTF-8 conversion is needed.
            regs.us_privacy = Some(usp.as_bytes().iter().copied().map(char::from).collect());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::apply_legacy_projection;
    use auctioneer_openrtb::types::bid_request::BidRequest;
    use auctioneer_privacy::gpp::Limits;
    use auctioneer_privacy::sources::{LegacyProjection, normalize};
    use serde_json::{Value, json};

    fn project(value: Value, gpp_supported: bool) -> Value {
        let original: BidRequest = serde_json::from_value(value).unwrap();
        let sources = normalize(&original, Limits::default()).unwrap();
        let projection = sources.projection();
        let mut projected = original.clone();
        apply_legacy_projection(&mut projected, projection, gpp_supported);
        let first = serde_json::to_value(&projected).unwrap();
        apply_legacy_projection(&mut projected, projection, gpp_supported);
        assert_eq!(
            serde_json::to_value(projected).unwrap(),
            first,
            "idempotent projection"
        );
        first
    }

    #[test]
    fn invalid_legacy_usp_is_cleared_before_metadata_specific_backfill() {
        for gpp_supported in [false, true] {
            let projected = project(
                json!({
                    "id": "projection",
                    "regs": {"gpp": "DBABTA~1YYN", "gpp_sid": [6_i32],
                        "us_privacy": "invalid-legacy", "ext": {"gpc": "1"}}
                }),
                gpp_supported,
            );
            assert_eq!(projected["regs"]["gpp"], "DBABTA~1YYN");
            assert_eq!(projected["regs"]["gpp_sid"], json!([6_i32]));
            assert_eq!(projected["regs"]["ext"]["gpc"], "1");
            assert!(projected.get("user").is_none());
            if gpp_supported {
                assert!(projected["regs"].get("us_privacy").is_none());
                assert!(projected["regs"].get("gdpr").is_none());
            } else {
                assert_eq!(projected["regs"]["us_privacy"], "1YYN");
                assert_eq!(projected["regs"]["gdpr"], 0_i32);
            }
        }
    }

    #[test]
    fn populated_legacy_and_original_gpp_sid_are_untouched() {
        let original = json!({
            "id": "projection",
            "regs": {"gpp": "DBACNYA~selected-tcf~1YNN", "gpp_sid": [6_i32,2_i32,6_i32,999_i32],
                "gdpr": 0_i32, "us_privacy": "1YYN", "coppa": 1_i32},
            "user": {"consent": "populated-legacy-tcf", "geo": {}, "ext": {"context": "keep"}},
            "source": {"ext": {"context": "keep"}}
        });
        for gpp_supported in [false, true] {
            assert_eq!(project(original.clone(), gpp_supported), original);
        }
    }

    #[test]
    fn missing_and_empty_legacy_fields_are_backfilled_only_for_non_gpp_bidders() {
        for empty in [false, true] {
            let mut original = json!({
                "id": "projection",
                "regs": {"gpp": "DBACNY~opaque-tcf~1YNN", "gpp_sid": [2_i32,6_i32]}
            });
            if empty {
                original["user"] = json!({"consent": ""});
                original["regs"]["us_privacy"] = json!("");
            }
            assert_eq!(project(original.clone(), true), original);
            let projected = project(original.clone(), false);
            assert_eq!(projected["user"], json!({"consent": "opaque-tcf"}));
            assert_eq!(projected["regs"]["gdpr"], 1_i32);
            assert_eq!(projected["regs"]["us_privacy"], "1YNN");
            assert_eq!(projected["regs"]["gpp"], original["regs"]["gpp"]);
            assert_eq!(projected["regs"]["gpp_sid"], original["regs"]["gpp_sid"]);
        }
    }

    #[test]
    fn no_backfill_inputs_do_not_create_missing_objects_even_during_sanitation() {
        let projection = LegacyProjection {
            gdpr: None,
            consent: None,
            usp: None,
            clear_legacy_usp: true,
        };
        for gpp_supported in [false, true] {
            let mut request = BidRequest::default();
            apply_legacy_projection(&mut request, projection, gpp_supported);
            assert!(request.user.is_none());
            assert!(request.regs.is_none());
        }
    }

    #[test]
    fn empty_active_tcf_is_copied_but_empty_or_invalid_usp_is_not() {
        for selected_usp in ["", "invalid-selected-usp"] {
            let projected = project(
                json!({
                    "id": "projection",
                    "regs": {"gpp": format!("DBACNY~~{selected_usp}"), "gpp_sid": [2_i32,6_i32]}
                }),
                false,
            );
            assert_eq!(projected["user"], json!({"consent": ""}));
            assert_eq!(projected["regs"]["gdpr"], 1_i32);
            assert!(projected["regs"].get("us_privacy").is_none());
        }
    }
}
