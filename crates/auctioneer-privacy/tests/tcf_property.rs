//! Property-based coverage for the TCF v2 parser: arbitrary input
//! must never panic, and purpose/vendor payloads must round-trip
//! through the test-side encoder losslessly under both vendor-section
//! wire encodings.

use auctioneer_privacy::test_support::tc_encode;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use auctioneer_privacy::tcf::{TcfParseError, TcfParser, VendorBitSet, parse_tcf_v2};
    use proptest::collection::btree_set;
    use proptest::prelude::*;
    use proptest::string::string_regex;

    use super::tc_encode::{
        BitWriter, RangeEntrySpec, VendorSectionSpec, dv_bitfield_segment, encode_core,
        valid_core_spec, with_minimal_dv, write_range_entries,
    };

    fn range_entry(start: u16, end: u16) -> RangeEntrySpec {
        if start == end {
            RangeEntrySpec::Single(start)
        } else {
            RangeEntrySpec::Span(start, end)
        }
    }

    /// Contiguous runs of the sorted id set as range entries: len-1
    /// runs encode as `Single`, longer runs as `Span`.
    fn ids_to_range_entries(ids: &BTreeSet<u16>) -> Vec<RangeEntrySpec> {
        let mut entries = Vec::new();
        let mut run: Option<(u16, u16)> = None;
        for &id in ids {
            run = match run {
                Some((start, end)) if id == end.wrapping_add(1) => Some((start, id)),
                Some((start, end)) => {
                    entries.push(range_entry(start, end));
                    Some((id, id))
                }
                None => Some((id, id)),
            };
        }
        if let Some((start, end)) = run {
            entries.push(range_entry(start, end));
        }
        entries
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn parse_never_panics(input in string_regex(".*").expect("valid regex")) {
            // Returning at all (Ok or Err, no panic, no unbounded
            // allocation) is the property.
            let _result = parse_tcf_v2(&input);
        }

        #[test]
        fn roundtrip_preserves_purposes(
            purposes in btree_set(1_u8..=24, 0..=24_usize),
            vendors in btree_set(1_u16..=200, 0..=60_usize),
            range_encoded in any::<bool>(),
        ) {
            let mut spec = valid_core_spec();
            spec.purposes_consent = purposes.iter().copied().collect();
            spec.vendor_consent = if range_encoded {
                VendorSectionSpec::Range {
                    max_vendor_id: 200,
                    entries: ids_to_range_entries(&vendors),
                }
            } else {
                VendorSectionSpec::Bitfield {
                    max_vendor_id: 200,
                    set: vendors.iter().copied().collect(),
                }
            };
            let parsed = parse_tcf_v2(&with_minimal_dv(&encode_core(&spec)))
                .expect("generated payload is valid");
            prop_assert_eq!(&parsed.purposes_consent, &purposes);
            for vendor in 1..=200_u16 {
                prop_assert_eq!(parsed.vendor_consent.contains(vendor), vendors.contains(&vendor));
            }
        }

        #[test]
        fn dv_roundtrip_preserves_independent_membership_after_failed_parse(
            disclosed in btree_set(1_u16..=u16::MAX, 0..=24_usize),
            consent in btree_set(1_u16..=200, 0..=30_usize),
            li in btree_set(1_u16..=200, 0..=30_usize),
            range_encoded in any::<bool>(),
            reverse_ranges in any::<bool>(),
        ) {
            let max = disclosed.last().copied().unwrap_or(0);
            let dv = if range_encoded {
                let mut entries = ids_to_range_entries(&disclosed);
                if reverse_ranges {
                    entries.reverse();
                }
                let mut w = BitWriter::new();
                w.push_bits(1, 3);
                w.push_bits(u64::from(max), 16);
                w.push_bit(true);
                write_range_entries(&mut w, &entries);
                w.into_base64url()
            } else {
                dv_bitfield_segment(max, &disclosed.iter().copied().collect::<Vec<_>>())
            };
            let mut spec = valid_core_spec();
            spec.vendor_consent = VendorSectionSpec::Bitfield {
                max_vendor_id: 200,
                set: consent.iter().copied().collect(),
            };
            spec.vendor_li = VendorSectionSpec::Bitfield {
                max_vendor_id: 200,
                set: li.iter().copied().collect(),
            };
            let mut parser = TcfParser::new();
            // Fail only after an unrelated, nonempty DV was retained.
            let failed = format!("%.{}", dv_bitfield_segment(1, &[1]));
            prop_assert_eq!(parser.parse(&failed).unwrap_err(), TcfParseError::InvalidBase64);
            let raw = format!("{}.{dv}", encode_core(&spec));
            let parsed = parser.parse(&raw).expect("generated DV is valid");
            let mut expected_dv = VendorBitSet::new();
            for id in &disclosed {
                expected_dv.set(*id);
            }
            prop_assert_eq!(&parsed.disclosed_vendors, &expected_dv);
            for id in 0..=200 {
                prop_assert_eq!(parsed.vendor_consent.contains(id), consent.contains(&id));
                prop_assert_eq!(parsed.vendor_li.contains(id), li.contains(&id));
            }
            prop_assert_eq!(parser.partial_disclosed_vendors(), &VendorBitSet::new());
            let fresh = format!("{}.{}", encode_core(&valid_core_spec()), dv_bitfield_segment(0, &[]));
            let empty = parser.parse(&fresh).expect("empty DV after successful parse");
            prop_assert_eq!(empty.disclosed_vendors, VendorBitSet::new());
            prop_assert_eq!(empty.vendor_consent, VendorBitSet::new());
            prop_assert_eq!(empty.vendor_li, VendorBitSet::new());
        }
    }
}
