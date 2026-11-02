//! Fixed, nonfatal construction diagnostics, independent of sink build errors.

use serde::ser::SerializeStruct as _;
use serde::{Serialize, Serializer};

/// Maximum number of nonfatal construction diagnostics retained per bidder.
pub const MAX_CONSTRUCTION_DIAGNOSTICS: usize = 32;

/// A closed diagnostic vocabulary with no request data or arbitrary messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ConstructionDiagnostic {
    InvalidImpressionParameters,
    UnsupportedMediaType,
    InvalidImpression,
    AdditionalDiagnostics,
}

impl ConstructionDiagnostic {
    /// Public invalid-input code shared by every nonfatal diagnostic.
    #[must_use]
    pub const fn code(&self) -> u32 {
        2
    }

    /// Safe static text, never derived from request or validator contents.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::InvalidImpressionParameters => "invalid impression parameters",
            Self::UnsupportedMediaType => "unsupported media type",
            Self::InvalidImpression => "invalid impression",
            Self::AdditionalDiagnostics => "additional construction diagnostics suppressed",
        }
    }
}

impl Serialize for ConstructionDiagnostic {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut diagnostic = serializer.serialize_struct("ConstructionDiagnostic", 2)?;
        diagnostic.serialize_field("code", &self.code())?;
        diagnostic.serialize_field("message", self.message())?;
        diagnostic.end()
    }
}

/// Inline storage for the first 32 diagnostics, with a final overflow marker.
///
/// On the 33rd push, only the last retained slot becomes
/// [`ConstructionDiagnostic::AdditionalDiagnostics`]. Further pushes cannot
/// change the retained prefix or grow either storage or serialized output.
#[derive(Clone, Debug)]
pub struct ConstructionDiagnostics {
    entries: [ConstructionDiagnostic; MAX_CONSTRUCTION_DIAGNOSTICS],
    len: u8,
}

impl Default for ConstructionDiagnostics {
    fn default() -> Self {
        Self {
            entries: [ConstructionDiagnostic::AdditionalDiagnostics; MAX_CONSTRUCTION_DIAGNOSTICS],
            len: 0,
        }
    }
}

impl ConstructionDiagnostics {
    /// Retain one diagnostic without allocation, marking overflow in place.
    pub fn push(&mut self, diagnostic: ConstructionDiagnostic) {
        if let Some(slot) = self.entries.get_mut(usize::from(self.len)) {
            *slot = diagnostic;
            self.len = self.len.saturating_add(1);
            return;
        }
        if let Some(last) = self.entries.last_mut() {
            *last = ConstructionDiagnostic::AdditionalDiagnostics;
        }
    }

    /// Number of retained entries, including any overflow marker.
    #[must_use]
    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    /// Whether no diagnostics have been retained.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Borrow the retained entries in push order, without allocation.
    pub fn iter(&self) -> impl Iterator<Item = &ConstructionDiagnostic> {
        self.entries.iter().take(self.len())
    }
}

impl Serialize for ConstructionDiagnostics {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(self.iter())
    }
}

// The separate diagnostic retention charge covers native and guest layouts.
const _: () = assert!(
    size_of::<ConstructionDiagnostics>() <= 40,
    "inline construction diagnostics outgrew their funded envelope"
);

#[cfg(test)]
mod tests {
    use super::{ConstructionDiagnostic, ConstructionDiagnostics, MAX_CONSTRUCTION_DIAGNOSTICS};
    use std::mem::{align_of, needs_drop};
    use std::ptr;

    #[test]
    fn diagnostics_have_only_fixed_safe_code_two_messages() {
        for (diagnostic, message) in [
            (
                ConstructionDiagnostic::InvalidImpressionParameters,
                "invalid impression parameters",
            ),
            (
                ConstructionDiagnostic::UnsupportedMediaType,
                "unsupported media type",
            ),
            (
                ConstructionDiagnostic::InvalidImpression,
                "invalid impression",
            ),
            (
                ConstructionDiagnostic::AdditionalDiagnostics,
                "additional construction diagnostics suppressed",
            ),
        ] {
            let static_message: &'static str = diagnostic.message();
            assert_eq!(static_message, message);
            assert!(static_message.is_ascii());
            assert!(static_message.len() <= 64);
            assert_eq!(diagnostic.code(), 2);
            assert_eq!(
                serde_json::to_value(diagnostic).unwrap(),
                serde_json::json!({ "code": 2_u32, "message": message })
            );
        }
    }

    #[test]
    fn default_diagnostics_are_empty_and_serialize_as_an_empty_sequence() {
        let diagnostics = ConstructionDiagnostics::default();
        assert!(diagnostics.is_empty());
        assert_eq!(diagnostics.len(), 0);
        assert_eq!(diagnostics.iter().count(), 0);
        assert_eq!(serde_json::to_string(&diagnostics).unwrap(), "[]");
    }

    #[test]
    fn exact_capacity_retains_every_entry_and_one_over_replaces_only_the_last() {
        let mut expected = [ConstructionDiagnostic::InvalidImpressionParameters; 32];
        expected[30] = ConstructionDiagnostic::UnsupportedMediaType;
        expected[31] = ConstructionDiagnostic::InvalidImpression;
        let mut diagnostics = ConstructionDiagnostics::default();
        for diagnostic in &expected[..31] {
            diagnostics.push(*diagnostic);
        }
        assert_eq!(diagnostics.len(), 31);
        assert!(diagnostics.iter().eq(expected[..31].iter()));

        diagnostics.push(expected[31]);
        assert_eq!(diagnostics.len(), 32);
        assert!(!diagnostics.is_empty());
        assert!(diagnostics.iter().eq(expected.iter()));
        assert_eq!(
            serde_json::to_value(&diagnostics).unwrap(),
            serde_json::to_value(expected).unwrap()
        );

        diagnostics.push(ConstructionDiagnostic::UnsupportedMediaType);
        expected[31] = ConstructionDiagnostic::AdditionalDiagnostics;
        assert_eq!(diagnostics.len(), 32);
        assert!(diagnostics.iter().eq(expected.iter()));
        assert_eq!(
            serde_json::to_value(&diagnostics).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }

    #[test]
    fn repeated_overflow_keeps_the_first_thirty_one_and_one_marker() {
        let mut diagnostics = ConstructionDiagnostics::default();
        for _ in 0..MAX_CONSTRUCTION_DIAGNOSTICS {
            diagnostics.push(ConstructionDiagnostic::InvalidImpression);
        }
        for _ in 0_usize..1_024 {
            diagnostics.push(ConstructionDiagnostic::UnsupportedMediaType);
        }
        let mut expected = [ConstructionDiagnostic::InvalidImpression; 32];
        expected[31] = ConstructionDiagnostic::AdditionalDiagnostics;
        assert_eq!(diagnostics.len(), 32);
        assert!(diagnostics.iter().eq(expected.iter()));
        assert_eq!(
            serde_json::to_value(&diagnostics).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }

    #[test]
    fn clone_is_independent_and_iteration_borrows_the_inline_entries() {
        let mut original = ConstructionDiagnostics::default();
        original.push(ConstructionDiagnostic::InvalidImpressionParameters);
        let mut cloned = original.clone();
        cloned.push(ConstructionDiagnostic::UnsupportedMediaType);
        assert_eq!(original.len(), 1);
        assert_eq!(cloned.len(), 2);
        assert!(
            original
                .iter()
                .zip(original.entries.iter())
                .all(|(borrowed, stored)| ptr::eq(borrowed, stored))
        );
        assert!(format!("{cloned:?}").contains("UnsupportedMediaType"));
    }

    #[test]
    fn inline_diagnostic_layout_is_pointer_width_independent() {
        assert_eq!(MAX_CONSTRUCTION_DIAGNOSTICS, 32);
        assert_eq!(size_of::<ConstructionDiagnostic>(), 1);
        assert_eq!(align_of::<ConstructionDiagnostic>(), 1);
        assert_eq!(size_of::<ConstructionDiagnostics>(), 33);
        assert!(size_of::<ConstructionDiagnostics>() <= 40);
        assert_eq!(align_of::<ConstructionDiagnostics>(), 1);
        assert!(!needs_drop::<ConstructionDiagnostics>());
    }
}
