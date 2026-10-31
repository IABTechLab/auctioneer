//! Extra trailing-segment builders for consumers that exercise the
//! full segment framing surface. Available alongside the canonical
//! core encoder in unit-test builds and with the `test-support` feature.

use super::tc_encode::{BitWriter, RangeEntrySpec, write_range_entries};

/// Disclosed Vendors segment, range encoding.
#[must_use]
pub fn dv_range_segment(max_vendor_id: u16, entries: &[RangeEntrySpec]) -> String {
    let mut w = BitWriter::new();
    w.push_bits(1, 3);
    w.push_bits(u64::from(max_vendor_id), 16);
    w.push_bit(true);
    write_range_entries(&mut w, entries);
    w.into_base64url()
}

/// Publisher TC segment with all-zero purpose bits and
/// `num_custom_purposes` zeroed custom-purpose bit pairs.
#[must_use]
pub fn publisher_tc_segment(num_custom_purposes: u8) -> String {
    let mut w = BitWriter::new();
    w.push_bits(3, 3);
    w.push_bits(0, 24);
    w.push_bits(0, 24);
    w.push_bits(u64::from(num_custom_purposes), 6);
    for _ in 0..num_custom_purposes {
        w.push_bit(false);
        w.push_bit(false);
    }
    w.into_base64url()
}
