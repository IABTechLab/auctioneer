//! Final-step `prepare()` checks.
//!
//! No consent-string parsing here.
//! Consent warnings live in `auctioneer-privacy` from M3+; M1's `prepare()`
//! only does the legacy 2.5 privacy rejection and the channel-empty-name
//! rejection.

use crate::types::bid_request::BidRequest;
use crate::validate::ValidationError;

/// If `ext.prebid.channel` is present but `channel.name` is explicitly
/// empty (`""`), reject. Absent → filled with `"app"` for app requests
/// in `implicit_fill::fill_channel`. A channel object whose `name`
/// field is *missing* (`None` after deserialize) is **not** an error —
/// only the explicit empty-string case rejects.
pub(super) fn reject_empty_channel_name(req: &BidRequest) -> Result<(), ValidationError> {
    let Some(ext) = &req.ext else {
        return Ok(());
    };
    let Some(prebid) = &ext.prebid else {
        return Ok(());
    };
    let Some(channel) = &prebid.channel else {
        return Ok(());
    };
    if matches!(&channel.name, Some(name) if name.is_empty()) {
        return Err(ValidationError::EmptyChannelName);
    }
    Ok(())
}

/// Reject if any legacy 2.5 privacy field is present. The fields live
/// under `regs.ext` / `user.ext` as raw JSON (we never typed them under
/// those locations); detect by scanning the `other` map.
pub(super) fn reject_legacy_25_privacy(req: &BidRequest) -> Result<(), ValidationError> {
    if let Some(regs) = &req.regs
        && let Some(ext) = &regs.ext
    {
        if ext.other.contains_key("gdpr") {
            return Err(ValidationError::Legacy25Privacy("regs.ext.gdpr"));
        }
        if ext.other.contains_key("us_privacy") {
            return Err(ValidationError::Legacy25Privacy("regs.ext.us_privacy"));
        }
    }
    if let Some(user) = &req.user
        && let Some(ext) = &user.ext
        && ext.other.contains_key("consent")
    {
        return Err(ValidationError::Legacy25Privacy("user.ext.consent"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ext::misc::{RegsExt, UserExt};
    use crate::ext::request_prebid::{Channel, ExtRequestPrebid};
    use crate::types::bid_request::{Banner, BidRequestExt, Imp};
    use crate::types::regs::Regs;
    use crate::types::user::User;

    fn req_with_imp() -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: vec![Imp {
                id: "i".into(),
                banner: Some(Banner::default()),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn explicit_empty_channel_name_rejected() {
        let mut req = req_with_imp();
        let prebid = ExtRequestPrebid {
            channel: Some(Channel {
                name: Some(String::new()),
                ..Default::default()
            }),
            ..Default::default()
        };
        req.ext = Some(BidRequestExt {
            prebid: Some(prebid),
            other: serde_json::Map::default(),
        });
        assert_eq!(
            reject_empty_channel_name(&req),
            Err(ValidationError::EmptyChannelName),
        );
    }

    #[test]
    fn legacy_regs_ext_gdpr_rejected() {
        let mut req = req_with_imp();
        let mut ext = RegsExt::default();
        ext.other.insert("gdpr".into(), serde_json::json!(1_i32));
        req.regs = Some(Regs {
            ext: Some(ext),
            ..Default::default()
        });
        assert_eq!(
            reject_legacy_25_privacy(&req),
            Err(ValidationError::Legacy25Privacy("regs.ext.gdpr")),
        );
    }

    #[test]
    fn legacy_user_ext_consent_rejected() {
        let mut req = req_with_imp();
        let mut ext = UserExt::default();
        ext.other.insert("consent".into(), serde_json::json!("XYZ"));
        req.user = Some(User {
            ext: Some(ext),
            ..Default::default()
        });
        assert_eq!(
            reject_legacy_25_privacy(&req),
            Err(ValidationError::Legacy25Privacy("user.ext.consent")),
        );
    }

    #[test]
    fn missing_channel_name_field_is_accepted() {
        // Only the explicit empty-string case rejects. A channel object
        // whose `name` field is absent passes.
        let mut req = req_with_imp();
        let prebid = ExtRequestPrebid {
            channel: Some(Channel {
                name: None,
                ..Default::default()
            }),
            ..Default::default()
        };
        req.ext = Some(BidRequestExt {
            prebid: Some(prebid),
            other: serde_json::Map::default(),
        });
        reject_empty_channel_name(&req).unwrap();
    }

    #[test]
    fn nonempty_channel_name_is_accepted() {
        let mut req = req_with_imp();
        let prebid = ExtRequestPrebid {
            channel: Some(Channel {
                name: Some("amp".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        req.ext = Some(BidRequestExt {
            prebid: Some(prebid),
            other: serde_json::Map::default(),
        });
        reject_empty_channel_name(&req).unwrap();
    }
}
