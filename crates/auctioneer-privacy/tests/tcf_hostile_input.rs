//! Hostile-input regressions locking in the TCF parser's caps and
//! range validation. Each case asserts (a) the expected rejection
//! variant and (b) a deterministic operation bound — the hand-rolled
//! parser exposes its op counter, so the bounds are exact counts
//! independent of wall-clock and machine.

use auctioneer_privacy::test_support::{tc_encode, tc_encode_segments};

#[cfg(test)]
mod tests {
    #[cfg(feature = "timing-tests")]
    use std::time::{Duration, Instant};

    use auctioneer_privacy::tcf::{
        DEFAULT_OP_CEILING, MAX_INPUT_LEN, MAX_TOTAL_INPUT_LEN, MAX_TRAILING_SEGMENTS,
        RangeSection, TcfParseError, TcfParser, VendorBitSet,
    };

    use super::tc_encode::{
        BitWriter, RangeEntrySpec, VendorSectionSpec, dv_bitfield_segment, encode_core, letter,
        minimal_dv_segment, valid_core_spec, with_minimal_dv,
    };
    use super::tc_encode_segments::{dv_range_segment, publisher_tc_segment};

    /// Optional wall-clock sanity ceiling layered onto a hostile
    /// parse; not part of the default gate (flaky on shared CI).
    fn timed<T>(run: impl FnOnce() -> T) -> T {
        #[cfg(feature = "timing-tests")]
        {
            let start = Instant::now();
            let value = run();
            assert!(
                start.elapsed() <= Duration::from_millis(10),
                "hostile parse exceeded the 10 ms sanity ceiling"
            );
            value
        }
        #[cfg(not(feature = "timing-tests"))]
        {
            run()
        }
    }

    /// The documented bit-level work ceiling for a fully walked
    /// input: one op per raw byte (decode) plus one per decoded bit
    /// (6 per raw byte) stays under 8 ops per raw byte, with the
    /// slack covering the bounded per-entry word and push charges.
    fn bit_walk_ceiling(raw: &str) -> u64 {
        u64::try_from(raw.len()).unwrap_or(0).saturating_mul(8)
    }

    #[test]
    fn set_range_fills_maximal_range_in_bounded_words() {
        // Direct storage-layer proof, independent of any parser
        // counter: the O(words) fill covers the maximal range
        // completely while id 0 stays unrepresentable.
        let mut set = VendorBitSet::new();
        set.set_range(1, u16::MAX);
        assert!(!set.contains(0));
        for id in 1..=u16::MAX {
            assert!(set.contains(id), "id {id} missing from maximal fill");
        }
    }

    #[test]
    fn hand_rolled_parser_low_ceiling_rejects_before_mutating() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 65_000,
            entries: vec![
                RangeEntrySpec::Span(1, 16_000),
                RangeEntrySpec::Span(16_001, 32_000),
                RangeEntrySpec::Span(32_001, 48_000),
                RangeEntrySpec::Span(48_001, 65_000),
            ],
        };
        let raw = with_minimal_dv(&encode_core(&spec));
        let mut baseline = TcfParser::new();
        baseline
            .parse(&raw)
            .expect("fixture parses under the default ceiling");
        let full_ops = baseline.ops_used();
        // A ceiling 100 ops short of the full cost lands inside the
        // fourth range's ~266-word charge (everything after that
        // charge totals ~40 ops), so the parser must stop AT the
        // ceiling with the fourth range never written
        // (validate-before-mutate).
        let ceiling = full_ops.saturating_sub(100);
        let mut limited = TcfParser::with_op_ceiling(ceiling);
        assert_eq!(
            limited.parse(&raw).unwrap_err(),
            TcfParseError::OpBudgetExceeded
        );
        assert_eq!(limited.ops_used(), ceiling);
        let partial = limited.partial_vendor_consent();
        assert!(partial.contains(1));
        assert!(partial.contains(48_000));
        assert!(!partial.contains(48_001));
        assert!(!partial.contains(65_000));
        assert!(!partial.contains(0));
    }

    #[test]
    fn dv_ranges_charge_before_mutating_and_reset_on_reuse() {
        let core = encode_core(&valid_core_spec());
        let dv = dv_range_segment(
            u16::MAX,
            &[
                RangeEntrySpec::Single(1),
                RangeEntrySpec::Span(64, u16::MAX),
            ],
        );
        let raw = format!("{core}.{dv}");
        let mut baseline = TcfParser::new();
        let baseline_tcf = baseline.parse(&raw).expect("bounded maximal DV range");
        for id in 0..=u16::MAX {
            assert_eq!(
                baseline_tcf.disclosed_vendors.contains(id),
                id == 1 || id >= 64
            );
        }
        assert_eq!(baseline_tcf.vendor_consent, VendorBitSet::new());
        assert_eq!(baseline_tcf.vendor_li, VendorBitSet::new());
        assert!(baseline.ops_used() < DEFAULT_OP_CEILING);

        // DV header: 32 bits. First entry: 17 bits + one word + one
        // append. Second entry: 33 bits, then 1023 words + one append.
        let before_second_fill = u64::try_from(dv.len()).unwrap() + 32 + 17 + 1 + 1 + 33;
        for available in [1_022, 1_023] {
            // Fail either the word charge or the following append;
            // neither failure may write any of the second range.
            let ceiling = before_second_fill + available;
            let mut parser = TcfParser::with_op_ceiling(ceiling);
            assert_eq!(
                parser.parse(&raw).unwrap_err(),
                TcfParseError::OpBudgetExceeded
            );
            assert_eq!(parser.ops_used(), ceiling);
            let mut expected = VendorBitSet::new();
            expected.set(1);
            assert_eq!(parser.partial_disclosed_vendors(), &expected);
            assert_eq!(parser.partial_vendor_consent(), &VendorBitSet::new());
            let fresh = format!("{core}.{}", dv_bitfield_segment(2, &[2]));
            let tcf = parser.parse(&fresh).expect("reuse after budget failure");
            let mut expected_fresh = VendorBitSet::new();
            expected_fresh.set(2);
            assert_eq!(tcf.disclosed_vendors, expected_fresh);
            assert_eq!(tcf.vendor_consent, VendorBitSet::new());
            assert_eq!(tcf.vendor_li, VendorBitSet::new());
            assert_eq!(parser.partial_disclosed_vendors(), &VendorBitSet::new());
        }
    }

    #[test]
    fn dv_bitfield_charges_all_bits_before_mutating() {
        let core = encode_core(&valid_core_spec());
        let dv = dv_bitfield_segment(u16::MAX, &[1, 64, u16::MAX]);
        let raw = format!("{core}.{dv}");
        let mut baseline = TcfParser::new();
        let baseline_tcf = baseline.parse(&raw).expect("maximal DV bitfield");
        assert!(baseline_tcf.disclosed_vendors.contains(1));
        assert!(baseline_tcf.disclosed_vendors.contains(u16::MAX));
        // Header is 20 bits. One op short of the full bitfield must
        // leave even its first set bit untouched.
        let ceiling = u64::try_from(dv.len()).unwrap() + 20 + u64::from(u16::MAX) - 1;
        let mut parser = TcfParser::with_op_ceiling(ceiling);
        assert_eq!(
            parser.parse(&raw).unwrap_err(),
            TcfParseError::OpBudgetExceeded
        );
        assert_eq!(parser.ops_used(), ceiling);
        assert_eq!(parser.partial_disclosed_vendors(), &VendorBitSet::new());
        assert_eq!(parser.partial_vendor_consent(), &VendorBitSet::new());
        let fresh = format!("{core}.{}", dv_range_segment(0, &[]));
        let tcf = parser
            .parse(&fresh)
            .expect("reuse after bitfield budget failure");
        assert_eq!(tcf.disclosed_vendors, VendorBitSet::new());
        assert_eq!(tcf.vendor_consent, VendorBitSet::new());
        assert_eq!(tcf.vendor_li, VendorBitSet::new());
    }

    #[test]
    fn dv_truncated_bitfield_preserves_budget_error_precedence() {
        let mut w = BitWriter::new();
        w.push_bits(1, 3);
        w.push_bits(1_000, 16);
        w.push_bit(false);
        w.push_bit(true);
        let dv = w.into_base64url();
        let raw = format!("not-valid-core.{dv}");
        let full_charge = u64::try_from(dv.len()).unwrap() + 20 + 1_000;
        for (ceiling, expected) in [
            (full_charge - 1, TcfParseError::OpBudgetExceeded),
            (
                full_charge,
                TcfParseError::MalformedDisclosedVendorsSegment {
                    reason: "truncated segment",
                },
            ),
        ] {
            let mut parser = TcfParser::with_op_ceiling(ceiling);
            assert_eq!(parser.parse(&raw).unwrap_err(), expected);
            assert_eq!(parser.ops_used(), ceiling);
            assert_eq!(parser.partial_disclosed_vendors(), &VendorBitSet::new());
            assert_eq!(parser.partial_vendor_consent(), &VendorBitSet::new());
        }
    }

    #[test]
    fn dv_partial_state_is_reset_before_whole_input_cap() {
        let mut parser = TcfParser::new();
        let raw = format!("%.{}", dv_bitfield_segment(2, &[2]));
        assert_eq!(
            parser.parse(&raw).unwrap_err(),
            TcfParseError::InvalidBase64
        );
        assert!(parser.partial_disclosed_vendors().contains(2));
        let len = MAX_TOTAL_INPUT_LEN + 1;
        assert_eq!(
            parser.parse(&".".repeat(len)).unwrap_err(),
            TcfParseError::InputTooLong(len)
        );
        assert_eq!(parser.ops_used(), 0);
        assert_eq!(parser.partial_disclosed_vendors(), &VendorBitSet::new());
        let fresh = with_minimal_dv(&encode_core(&valid_core_spec()));
        let tcf = parser.parse(&fresh).expect("reuse after early failure");
        assert!(tcf.disclosed_vendors.contains(1));
        assert!(!tcf.disclosed_vendors.contains(2));
    }

    #[test]
    fn core_over_65536_bytes_rejected_before_base64_decode() {
        let oversize_len = MAX_INPUT_LEN.wrapping_add(1);
        let raw = format!("{}.{}", "A".repeat(oversize_len), minimal_dv_segment());
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(err, TcfParseError::InputTooLong(oversize_len));
        // Rejected before decoding: the only charged work is the DV
        // segment's framing validation (a few dozen ops); decoding
        // the 65 537-byte core would have charged that many ops on
        // its own.
        assert!(parser.ops_used() <= 64, "ops_used = {}", parser.ops_used());
    }

    #[test]
    fn oversize_trailing_segment_rejected() {
        // A valid core + valid DV + a >64 KiB type-3-leading garbage
        // segment: rejected as a TRAILING-segment error at the
        // per-segment bounded-decode cap — never a core-over-cap
        // error, and never silently discarded.
        let garbage = format!("Y{}", "A".repeat(MAX_INPUT_LEN));
        let raw = format!(
            "{}.{}.{garbage}",
            encode_core(&valid_core_spec()),
            minimal_dv_segment()
        );
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert!(matches!(
            err,
            TcfParseError::MalformedPublisherTcSegment { .. }
        ));
        // The oversize segment is classified from its first character
        // and rejected without being decoded.
        assert!(parser.ops_used() <= 64, "ops_used = {}", parser.ops_used());
    }

    #[test]
    fn too_many_trailing_segments_rejected() {
        // core + DV + Publisher TC + one more segment: the count
        // check runs FIRST, regardless of segment contents.
        // The framing-valid empty Publisher TC segment: SegmentType 3,
        // 48 zero purpose bits, NumCustomPurposes 0, zero-padded.
        let raw = format!(
            "{}.{}.{}.{}",
            encode_core(&valid_core_spec()),
            minimal_dv_segment(),
            publisher_tc_segment(0),
            minimal_dv_segment()
        );
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(err, TcfParseError::TooManyTrailingSegments { actual: 3 });
        assert_eq!(parser.ops_used(), 0, "count check precedes all charges");
    }

    #[test]
    fn range_with_start_greater_than_end_rejected() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 600,
            entries: vec![RangeEntrySpec::Span(500, 100)],
        };
        let raw = with_minimal_dv(&encode_core(&spec));
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(
            err,
            TcfParseError::RangeStartGreaterThanEnd {
                section: RangeSection::VendorConsent,
                start: 500,
                end: 100,
            }
        );
        assert!(parser.ops_used() <= bit_walk_ceiling(&raw));
    }

    #[test]
    fn overlapping_ranges_in_vendor_consent_section_rejected() {
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: 600,
            entries: vec![RangeEntrySpec::Span(10, 50), RangeEntrySpec::Single(40)],
        };
        let raw = with_minimal_dv(&encode_core(&spec));
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(
            err,
            TcfParseError::RangeEntriesOverlap {
                section: RangeSection::VendorConsent,
            }
        );
        assert!(parser.ops_used() <= bit_walk_ceiling(&raw));
    }

    #[test]
    fn consent_language_letter_out_of_alphabet_rejected() {
        // `ConsentLanguage` is two 6-bit alphabet positions; 26 and up
        // name no letter. Without the validating read this string
        // decodes into usable consent.
        let mut spec = valid_core_spec();
        spec.consent_language = (26, letter(b'N'));
        let raw = with_minimal_dv(&encode_core(&spec));
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(err, TcfParseError::InvalidBitfield);
        assert!(parser.ops_used() <= bit_walk_ceiling(&raw));
    }

    #[test]
    fn consent_language_second_letter_out_of_alphabet_rejected() {
        let mut spec = valid_core_spec();
        spec.consent_language = (letter(b'E'), 63);
        let raw = with_minimal_dv(&encode_core(&spec));
        let mut parser = TcfParser::new();
        assert_eq!(
            timed(|| parser.parse(&raw)).unwrap_err(),
            TcfParseError::InvalidBitfield
        );
    }

    #[test]
    fn valid_consent_language_still_parses() {
        let mut spec = valid_core_spec();
        spec.consent_language = (letter(b'F'), letter(b'R'));
        let raw = with_minimal_dv(&encode_core(&spec));
        let mut parser = TcfParser::new();
        parser
            .parse(&raw)
            .expect("a two-letter language code stays valid");
    }

    #[test]
    fn overlapping_disclosed_vendors_ranges_rejected() {
        // Disclosed Vendors is mandatory, so a malformed one must not
        // leave the core segment's consent usable.
        let dv = dv_range_segment(
            600,
            &[RangeEntrySpec::Span(10, 50), RangeEntrySpec::Span(40, 60)],
        );
        let raw = format!("{}.{dv}", encode_core(&valid_core_spec()));
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(
            err,
            TcfParseError::MalformedDisclosedVendorsSegment {
                reason: "range entries overlap",
            }
        );
        assert!(parser.ops_used() <= bit_walk_ceiling(&raw));
    }

    #[test]
    fn duplicate_single_vendor_in_disclosed_vendors_rejected() {
        let dv = dv_range_segment(
            600,
            &[RangeEntrySpec::Single(42), RangeEntrySpec::Single(42)],
        );
        let raw = format!("{}.{dv}", encode_core(&valid_core_spec()));
        let mut parser = TcfParser::new();
        assert_eq!(
            timed(|| parser.parse(&raw)).unwrap_err(),
            TcfParseError::MalformedDisclosedVendorsSegment {
                reason: "range entries overlap",
            }
        );
    }

    #[test]
    fn disjoint_disclosed_vendors_ranges_still_parse() {
        let dv = dv_range_segment(
            600,
            &[
                RangeEntrySpec::Span(10, 50),
                RangeEntrySpec::Single(51),
                RangeEntrySpec::Span(52, 600),
            ],
        );
        let raw = format!("{}.{dv}", encode_core(&valid_core_spec()));
        let mut parser = TcfParser::new();
        parser
            .parse(&raw)
            .expect("disjoint DV ranges remain valid framing");
    }

    /// `count` valid disjoint spans in a chosen encoding order.
    /// `order` maps entry index → the span slot it encodes, so every
    /// ordering of a given `count` carries an IDENTICAL set of ranges.
    fn spans_in_order(count: u16, order: impl Fn(u16) -> u16) -> String {
        let entries: Vec<RangeEntrySpec> = (0..count)
            .map(|n| {
                let start = order(n).saturating_mul(16).saturating_add(1);
                RangeEntrySpec::Span(start, start.saturating_add(8))
            })
            .collect();
        let mut spec = valid_core_spec();
        spec.vendor_consent = VendorSectionSpec::Range {
            max_vendor_id: u16::MAX,
            entries,
        };
        with_minimal_dv(&encode_core(&spec))
    }

    #[test]
    fn range_entry_order_does_not_decide_validity() {
        // The TCF v2 binary format imposes no ordering on range
        // entries, so ascending, descending and shuffled encodings of
        // ONE set of valid disjoint spans are three encodings of the
        // same consent. All three must parse, and the cost must not
        // depend on which one a CMP happened to emit: an order-
        // sensitive tracker charges the descending case quadratically
        // and rejects valid consent outright.
        let ascending = spans_in_order(4_000, |n| n);
        let descending = spans_in_order(4_000, |n| 3_999_u16.saturating_sub(n));
        // A stride coprime with 4 000 visits every slot exactly once,
        // so this is a genuine shuffle rather than a rotation.
        let shuffled = spans_in_order(4_000, |n| {
            u16::try_from(u32::from(n).wrapping_mul(1_237).wrapping_rem(4_000)).unwrap_or(0)
        });

        let mut ascending_parser = TcfParser::new();
        ascending_parser
            .parse(&ascending)
            .expect("ascending disjoint ranges are ordinary legal input");
        let ascending_ops = ascending_parser.ops_used();

        let mut descending_parser = TcfParser::new();
        descending_parser
            .parse(&descending)
            .expect("descending disjoint ranges encode the same valid consent");
        let descending_ops = descending_parser.ops_used();

        let mut shuffled_parser = TcfParser::new();
        shuffled_parser
            .parse(&shuffled)
            .expect("shuffled disjoint ranges encode the same valid consent");

        // Within a small constant of each other. The tracker charges
        // by entry COUNT (append, then one sort plus one pair scan at
        // section end), so the orderings differ only in base64 length,
        // not in tracker work.
        assert!(
            descending_ops <= ascending_ops.saturating_mul(2),
            "descending cost {descending_ops} must stay within 2x the ascending cost \
             {ascending_ops}; a larger gap means the charge tracks encoding order",
        );
        assert!(
            ascending_ops <= descending_ops.saturating_mul(2),
            "the bound holds in both directions: ascending {ascending_ops}, \
             descending {descending_ops}",
        );
    }

    #[test]
    fn overlap_is_rejected_whichever_order_it_is_encoded_in() {
        // Order-independence must not have been bought by dropping the
        // overlap check: the same overlapping pair rejects whether the
        // later-starting span is encoded first or second.
        for entries in [
            vec![RangeEntrySpec::Span(10, 50), RangeEntrySpec::Span(40, 60)],
            vec![RangeEntrySpec::Span(40, 60), RangeEntrySpec::Span(10, 50)],
        ] {
            let mut spec = valid_core_spec();
            spec.vendor_consent = VendorSectionSpec::Range {
                max_vendor_id: u16::MAX,
                entries,
            };
            let raw = with_minimal_dv(&encode_core(&spec));
            let mut parser = TcfParser::new();
            assert_eq!(
                timed(|| parser.parse(&raw)).unwrap_err(),
                TcfParseError::RangeEntriesOverlap {
                    section: RangeSection::VendorConsent,
                },
                "an overlap is an overlap regardless of encoding order",
            );
        }
    }

    #[test]
    fn overlap_detection_charges_its_own_work() {
        // Differencing two sections whose ONLY difference is entry
        // count isolates the per-entry cost from the fixed core-segment
        // charges, and pins it to an EXACT predicted figure rather than
        // a loose floor: a floor low enough to hold would also hold
        // with the overlap charge deleted, since the per-entry bit
        // reads alone dominate it.
        let wide = spans_in_order(4_000, |n| n);
        let narrow = spans_in_order(2_000, |n| n);

        let mut wide_parser = TcfParser::new();
        wide_parser.parse(&wide).expect("4 000 spans are legal");
        let mut narrow_parser = TcfParser::new();
        narrow_parser.parse(&narrow).expect("2 000 spans are legal");

        let delta = wide_parser
            .ops_used()
            .saturating_sub(narrow_parser.ops_used());

        // Per added entry: 33 bits read (1 is-span flag + 2 x 16-bit
        // endpoints), 1 word walk, 1 tracker append, 1 scan step — and
        // the whole section's sort charge re-scales with n. Concretely,
        // 2 000 added entries contribute 2 000 x 36 = 72 000, plus the
        // sort's 4 000 x 12 - 2 000 x 11 = 26 000, plus 11 000 of
        // base64 input bytes the longer string carries: 109 000.
        // Deleting the append + sort/scan charge drops this to 79 000,
        // so the equality is what makes the charge observable.
        assert_eq!(
            delta, 109_000,
            "doubling the entry count must add exactly the modelled ops, \
             overlap-detection charge included",
        );

        // And the charge lands BEFORE the sort runs, so a legal
        // full-width section still fits the ceiling with the charge
        // included rather than tripping it.
        assert!(
            wide_parser.ops_used() < DEFAULT_OP_CEILING,
            "a legal full-width range section must fit the default ceiling; used {}",
            wide_parser.ops_used(),
        );
    }

    #[test]
    fn dot_flood_rejected_without_scanning_every_dot() {
        // 100 000 dots. The segment count short-circuits one segment
        // past the cap, and the whole-input cap would catch anything
        // larger still, so neither the split nor the count walks the
        // flood.
        let raw = format!("{}{}", encode_core(&valid_core_spec()), ".".repeat(100_000));
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(
            err,
            TcfParseError::TooManyTrailingSegments {
                actual: MAX_TRAILING_SEGMENTS + 1,
            },
            "the count must stop one past the cap rather than report the full flood"
        );
        assert_eq!(parser.ops_used(), 0, "count check precedes all charges");
    }

    #[test]
    fn whole_input_over_the_total_cap_rejected_before_the_split() {
        // Bigger than any legal string (core + two capped segments +
        // their separators) can be, so it is rejected before the input
        // is split or walked at all.
        let oversize_len = MAX_TOTAL_INPUT_LEN.wrapping_add(1);
        let raw = ".".repeat(oversize_len);
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(err, TcfParseError::InputTooLong(oversize_len));
        assert_eq!(parser.ops_used(), 0, "the whole-input cap precedes charges");
    }

    #[test]
    fn trailing_bytes_after_pub_restrictions_rejected() {
        let mut w = super::tc_encode::BitWriter::new();
        super::tc_encode::write_core(&mut w, &valid_core_spec());
        w.push_bits(0, 5); // legal zero padding to the byte boundary
        w.push_bits(0xFF, 8); // one whole smuggled byte
        let raw = with_minimal_dv(&w.into_base64url());
        let mut parser = TcfParser::new();
        let err = timed(|| parser.parse(&raw)).unwrap_err();
        assert_eq!(err, TcfParseError::TrailingBytes);
        assert!(parser.ops_used() <= bit_walk_ceiling(&raw));
    }
}
