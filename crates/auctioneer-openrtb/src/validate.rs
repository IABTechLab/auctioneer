//! Structural validation.
//!
//! `Display` strings deliberately omit the `Invalid request: ` prefix; that
//! prefix is added by the auction endpoint when serializing wire errors.

use crate::types::bid_request::BidRequest;
use std::collections::HashSet;
use thiserror::Error;

/// All structural validation failures.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("imp[{0}] must have a unique id")]
    DuplicateImpId(String),
    #[error("channel must have a name")]
    EmptyChannelName,
    #[error("imp[{0}] must declare at least one media type")]
    ImpHasNoMediaType(String),
    #[error("exactly one of site / app / dooh must be present")]
    InventoryShape,
    #[error("request carries legacy 2.5 privacy field {0}; auctioneer is 2.6 only")]
    Legacy25Privacy(&'static str),
    #[error("tmax must be >= 0")]
    NegativeTmax,
    #[error("imp[] must contain at least one entry")]
    NoImps,
    #[error("imp[] has {got} entries, exceeding the {max} limit")]
    TooManyImpressions { got: u32, max: u32 },
}

impl BidRequest {
    /// Structural validation. Returns the first failure.
    ///
    /// # Errors
    /// Returns [`ValidationError`] for any structural issue listed in that enum.
    #[inline]
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.imp.is_empty() {
            return Err(ValidationError::NoImps);
        }
        let mut seen = HashSet::with_capacity(self.imp.len());
        for imp in &self.imp {
            if !seen.insert(imp.id.clone()) {
                return Err(ValidationError::DuplicateImpId(imp.id.clone()));
            }
            if imp.banner.is_none()
                && imp.video.is_none()
                && imp.native.is_none()
                && imp.audio.is_none()
            {
                return Err(ValidationError::ImpHasNoMediaType(imp.id.clone()));
            }
        }
        let inventory_count = [self.site.is_some(), self.app.is_some(), self.dooh.is_some()]
            .iter()
            .filter(|present| **present)
            .count();
        if inventory_count != 1 {
            return Err(ValidationError::InventoryShape);
        }
        if let Some(tmax) = self.tmax
            && tmax < 0_i64
        {
            return Err(ValidationError::NegativeTmax);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::bid_request::{Banner, Imp};
    use crate::types::inventory::{App, Dooh, Site};

    fn ok_req() -> BidRequest {
        BidRequest {
            id: "r".into(),
            imp: vec![Imp {
                id: "i1".into(),
                banner: Some(Banner {
                    w: Some(300_i32),
                    h: Some(250_i32),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            site: Some(Site::default()),
            ..Default::default()
        }
    }

    #[test]
    fn duplicate_imp_ids_fail() {
        let mut req = ok_req();
        req.imp.push(req.imp[0].clone());
        assert_eq!(
            req.validate(),
            Err(ValidationError::DuplicateImpId("i1".into())),
        );
    }

    #[test]
    fn imp_without_media_type_fails() {
        let mut req = ok_req();
        req.imp[0].banner = None;
        assert!(matches!(
            req.validate(),
            Err(ValidationError::ImpHasNoMediaType(_))
        ));
    }

    #[test]
    fn site_plus_app_fails_inventory_shape() {
        let mut req = ok_req();
        req.app = Some(App::default());
        assert_eq!(req.validate(), Err(ValidationError::InventoryShape));
    }

    #[test]
    fn no_inventory_fails_inventory_shape() {
        let mut req = ok_req();
        req.site = None;
        assert_eq!(req.validate(), Err(ValidationError::InventoryShape));
    }

    #[test]
    fn dooh_only_passes_inventory_shape() {
        let mut req = ok_req();
        req.site = None;
        req.dooh = Some(Dooh::default());
        req.validate().unwrap();
    }

    #[test]
    fn negative_tmax_fails() {
        let mut req = ok_req();
        req.tmax = Some(-1_i64);
        assert_eq!(req.validate(), Err(ValidationError::NegativeTmax));
    }

    #[test]
    fn no_imps_fails() {
        let mut req = ok_req();
        req.imp.clear();
        assert_eq!(req.validate(), Err(ValidationError::NoImps));
    }

    #[test]
    fn valid_request_passes() {
        ok_req().validate().unwrap();
    }
}
