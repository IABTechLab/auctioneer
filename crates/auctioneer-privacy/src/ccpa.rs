//! Validated CCPA exceptions. Consent selection and enablement are independent.

use std::{collections::BTreeSet, fmt};

use crate::PrivacyError;

/// Exact requested bidder identities exempted from CCPA, not other policies.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct NoSale {
    all: bool,
    names: BTreeSet<String>,
}

impl fmt::Debug for NoSale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NoSale")
            .field("all", &self.all)
            .field("name_count", &self.names.len())
            .finish_non_exhaustive()
    }
}

impl NoSale {
    /// Validate against the exchange's full registered/resolved namespace.
    ///
    /// `is_known` must include valid disabled identities, not only participating
    /// bidders. No core-name fallback or Unicode folding is applied here.
    /// Call independently of consent validity and CCPA enablement.
    ///
    /// # Errors
    /// Returns [`PrivacyError::InvalidNoSale`] for unknown names or any wildcard
    /// list other than the singleton `["*"]`. Names are never included in errors.
    pub fn validate<Known>(names: &[String], is_known: Known) -> Result<Self, PrivacyError>
    where
        Known: Fn(&str) -> bool,
    {
        if names.len() == 1 && names.first().is_some_and(|name| name == "*") {
            return Ok(Self {
                all: true,
                names: BTreeSet::new(),
            });
        }
        // Reject the whole list before retaining any names. Repeated names do
        // not cause repeated owned-string allocations during deduplication.
        if names.iter().any(|name| name == "*" || !is_known(name)) {
            return Err(PrivacyError::InvalidNoSale);
        }
        let mut result = Self::default();
        for name in names {
            if !result.names.contains(name) {
                result.names.insert(name.clone());
            }
        }
        Ok(result)
    }

    /// Whether this exact requested bidder/alias is exempt from CCPA scrubbing.
    #[must_use]
    pub fn exempts(&self, bidder: &str) -> bool {
        self.all || self.names.contains(bidder)
    }
}
