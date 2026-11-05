//! Shared host/runtime vocabulary for request-disclosure descriptors.

use serde::Deserialize;

/// JSON representation admitted for a reviewed field.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    String,
    Integer,
    Number,
    StringArray,
    Object,
    ObjectArray,
    Json,
}

/// Reviewed disposition, independent of generated domain closure.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    Context,
    Control,
    Id,
    Child,
    Opaque,
    PreciseGeo,
    Transform,
    Delegate,
}

/// Named finite-coordinate or explicit-address-family transformation.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Transform {
    Coordinate,
    Ipv4,
    Ipv6,
}

/// Explicit ownership exception, never a generic opaque-context exemption.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Delegate {
    UserFpd,
}

/// Fixed disclosure-domain bitmask; policy TOML cannot supply this value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Domains(u8);

impl Domains {
    pub const NONE: Self = Self(0);
    pub const ID: Self = Self(1);
    pub const GEO: Self = Self(2);
    pub const FPD: Self = Self(4);
    pub const TID: Self = Self(8);

    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

#[cfg(test)]
mod tests {
    use super::Domains;

    #[test]
    fn fixed_domain_bits_and_union_do_not_confuse_fpd_with_ids_or_tid() {
        assert_eq!(Domains::NONE.bits(), 0);
        assert_eq!(Domains::ID.bits(), 1);
        assert_eq!(Domains::GEO.bits(), 2);
        assert_eq!(Domains::FPD.bits(), 4);
        assert_eq!(Domains::TID.bits(), 8);
        assert_eq!(Domains::ID.union(Domains::GEO).bits(), 3);
        assert_eq!(Domains::from_bits(15).bits(), 15);
        assert!(!Domains::FPD.intersects(Domains::ID.union(Domains::GEO)));
        assert!(Domains::from_bits(12).intersects(Domains::TID));
    }
}
