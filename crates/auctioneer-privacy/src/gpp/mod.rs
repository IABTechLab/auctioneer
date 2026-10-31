//! Bounded GPP v1 envelope decoding. Payloads remain borrowed and opaque.
//!
//! No section codecs, source selection, policy, or outbound rewriting live here.

#![forbid(unsafe_code)]

use std::{error::Error, fmt};

const CAPACITY: usize = 64;

/// Immutable, lower-only resource limits for one parse.
///
/// Defaults: 262144 raw UTF-8 bytes, 4096 header bytes, 64 raw payload slots
/// and decoded IDs, maximum ID 65535, and 524288 header operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    raw_bytes: usize,
    header_bytes: usize,
    sections: u16,
    max_id: u32,
    header_ops: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            raw_bytes: 0x0004_0000,
            header_bytes: 4096,
            sections: 64,
            max_id: 0xFFFF,
            header_ops: 0x0008_0000,
        }
    }
}

impl Limits {
    /// Construct limits no greater than the documented defaults.
    ///
    /// Zero is valid for every limit, including the operation budget. `sections`
    /// limits both raw payload slots (including empty ones) and decoded IDs.
    ///
    /// # Errors
    /// Returns [`LimitsError`] if any value exceeds its supported ceiling.
    pub fn new(
        raw_bytes: usize,
        header_bytes: usize,
        sections: u16,
        max_id: u32,
        header_ops: u64,
    ) -> Result<Self, LimitsError> {
        let ceiling = Self::default();
        if raw_bytes > ceiling.raw_bytes
            || header_bytes > ceiling.header_bytes
            || sections > ceiling.sections
            || max_id > ceiling.max_id
            || header_ops > ceiling.header_ops
        {
            return Err(LimitsError);
        }
        Ok(Self {
            raw_bytes,
            header_bytes,
            sections,
            max_id,
            header_ops,
        })
    }
}

/// Invalid caller configuration, separate from an input parsing error.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LimitsError;

impl fmt::Display for LimitsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unsupported GPP limits")
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Payload-free error uses default source and deprecated/unstable methods"
)]
impl Error for LimitsError {}

/// Closed, payload-free input failure classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Invalid framing, coding, type/version, padding, or payload count.
    Malformed,
    /// A resource limit or checked arithmetic bound was exceeded.
    LimitExceeded,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => "malformed GPP envelope",
            Self::LimitExceeded => "GPP envelope limit exceeded",
        })
    }
}

#[expect(
    clippy::missing_trait_methods,
    reason = "Payload-free error uses default source and deprecated/unstable methods"
)]
impl Error for ParseError {}

#[derive(Clone, Copy)]
struct Section<'a> {
    id: u16,
    payload: &'a str,
}

/// A validated envelope with fixed inline metadata and untouched borrowed data.
pub struct Envelope<'a> {
    raw: &'a str,
    entries: [Section<'a>; CAPACITY],
    len: u16,
}

impl fmt::Debug for Envelope<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Envelope").finish_non_exhaustive()
    }
}

impl<'a> Envelope<'a> {
    /// Look up an ID; `Some("")` is present-empty, distinct from missing.
    #[must_use]
    pub fn section(&self, id: u16) -> Option<&'a str> {
        self.sections()
            .find_map(|(candidate, payload)| (candidate == id).then_some(payload))
    }

    /// Iterate positive IDs and borrowed payloads in their original wire order.
    pub fn sections(&self) -> impl Iterator<Item = (u16, &'a str)> + '_ {
        self.entries
            .iter()
            .take(usize::from(self.len))
            .map(|entry| (entry.id, entry.payload))
    }

    /// Return the exact original input, without re-encoding or normalization.
    #[must_use]
    pub fn raw(&self) -> &'a str {
        self.raw
    }

    fn empty(raw: &'a str) -> Self {
        Self {
            raw,
            entries: [Section { id: 0, payload: "" }; CAPACITY],
            len: 0,
        }
    }

    fn append_range(
        &mut self,
        start: u64,
        end: u64,
        limits: Limits,
        budget: &mut Budget,
    ) -> Result<(), ParseError> {
        if start == 0 || end < start {
            return Err(ParseError::Malformed);
        }
        if end > u64::from(limits.max_id) {
            return Err(ParseError::LimitExceeded);
        }
        let width = budget.sub(end, start)?;
        let count = budget.add(width, 1)?;
        let total = budget.add(u64::from(self.len), count)?;
        if total > u64::from(limits.sections) {
            return Err(ParseError::LimitExceeded);
        }
        let mut id = start;
        loop {
            let wide_len = budget.add(u64::from(self.len), 1)?;
            let next_len =
                u16::try_from(wide_len).map_err(|_overflow| ParseError::LimitExceeded)?;
            let narrowed = u16::try_from(id).map_err(|_overflow| ParseError::LimitExceeded)?;
            let entry = self
                .entries
                .get_mut(usize::from(self.len))
                .ok_or(ParseError::LimitExceeded)?;
            budget.charge()?;
            entry.id = narrowed;
            self.len = next_len;
            if id == end {
                return Ok(());
            }
            id = budget.add(id, 1)?;
        }
    }
}

/// Parse a GPP v1 envelope without interpreting any section payload.
///
/// Empty input is malformed; callers handle absent-input normalization.
/// Structural caps precede semantic decoding, which stops at its first error.
///
/// # Errors
/// Returns [`ParseError::Malformed`] for invalid envelopes and
/// [`ParseError::LimitExceeded`] for bounds or arithmetic failures.
pub fn parse(raw: &str, limits: Limits) -> Result<Envelope<'_>, ParseError> {
    if raw.len() > limits.raw_bytes {
        return Err(ParseError::LimitExceeded);
    }
    let mut slots = raw.split('~');
    let header = slots.next().ok_or(ParseError::Malformed)?;
    if header.len() > limits.header_bytes {
        return Err(ParseError::LimitExceeded);
    }
    let mut slot_count = 0_u16;
    for _ in slots.clone() {
        if slot_count == limits.sections {
            return Err(ParseError::LimitExceeded);
        }
        slot_count = slot_count.checked_add(1).ok_or(ParseError::LimitExceeded)?;
    }
    let mut envelope = Envelope::empty(raw);
    let mut reader = Reader::new(header, limits.header_ops);
    reader.decode(limits, &mut envelope)?;
    if envelope.len != slot_count {
        return Err(ParseError::Malformed);
    }
    for (entry, payload) in envelope.entries.iter_mut().zip(slots) {
        entry.payload = payload;
    }
    Ok(envelope)
}

struct Budget {
    remaining: u64,
}

impl Budget {
    fn charge(&mut self) -> Result<(), ParseError> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(ParseError::LimitExceeded)?;
        Ok(())
    }

    fn add(&mut self, left: u64, right: u64) -> Result<u64, ParseError> {
        self.charge()?;
        left.checked_add(right).ok_or(ParseError::LimitExceeded)
    }

    fn sub(&mut self, left: u64, right: u64) -> Result<u64, ParseError> {
        self.charge()?;
        left.checked_sub(right).ok_or(ParseError::LimitExceeded)
    }
}

// IDs, Fibonacci weights, and counts use u64; cursor/encoded lengths use usize.
// Checked arithmetic in both domains is precharged.
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
    cached: Option<u8>,
    budget: Budget,
}

impl<'a> Reader<'a> {
    fn new(header: &'a str, operations: u64) -> Self {
        Self {
            bytes: header.as_bytes(),
            position: 0,
            cached: None,
            budget: Budget {
                remaining: operations,
            },
        }
    }

    #[expect(
        clippy::integer_division,
        clippy::integer_division_remainder_used,
        reason = "Six-bit cursor needs exact integer quotient and remainder"
    )]
    fn bit(&mut self) -> Result<u8, ParseError> {
        let offset = self.position % 6;
        if offset == 0 {
            let byte = self
                .bytes
                .get(self.position / 6)
                .ok_or(ParseError::Malformed)?;
            self.budget.charge()?;
            let value = match byte {
                b'A'..=b'Z' => {
                    self.budget.charge()?;
                    byte.checked_sub(b'A')
                }
                b'a'..=b'z' => {
                    self.budget.charge()?;
                    let index = byte.checked_sub(b'a').ok_or(ParseError::LimitExceeded)?;
                    self.budget.charge()?;
                    index.checked_add(26)
                }
                b'0'..=b'9' => {
                    self.budget.charge()?;
                    let index = byte.checked_sub(b'0').ok_or(ParseError::LimitExceeded)?;
                    self.budget.charge()?;
                    index.checked_add(52)
                }
                b'-' => Some(62),
                b'_' => Some(63),
                _ => return Err(ParseError::Malformed),
            }
            .ok_or(ParseError::LimitExceeded)?;
            // Cache only after validation; future bits in this character reuse it.
            self.cached = Some(value);
        }
        // Bit read, shift calculation, and cursor advancement are separate work.
        self.budget.charge()?;
        self.budget.charge()?;
        let shift = 5_usize
            .checked_sub(offset)
            .ok_or(ParseError::LimitExceeded)?;
        let value = self.cached.ok_or(ParseError::Malformed)?;
        let bit = (value >> shift) & 1;
        self.budget.charge()?;
        self.position = self
            .position
            .checked_add(1)
            .ok_or(ParseError::LimitExceeded)?;
        Ok(bit)
    }

    fn fixed(&mut self, width: u8) -> Result<u64, ParseError> {
        let mut value = 0_u64;
        for _ in 0..width {
            value = (value << 1_u32) | u64::from(self.bit()?);
        }
        Ok(value)
    }

    fn fibonacci(&mut self) -> Result<u64, ParseError> {
        let mut sum = 0;
        let mut previous_weight = 1;
        let mut weight = 1;
        let mut previous_bit = 0;
        let mut first = true;
        loop {
            let bit = self.bit()?;
            if previous_bit == 1 && bit == 1 {
                return if sum == 0 {
                    Err(ParseError::Malformed)
                } else {
                    Ok(sum)
                };
            }
            // Do not compute another weight for the terminator itself.
            if !first {
                let next = self.budget.add(previous_weight, weight)?;
                previous_weight = weight;
                weight = next;
            }
            if bit == 1 {
                sum = self.budget.add(sum, weight)?;
            }
            first = false;
            previous_bit = bit;
        }
    }

    fn decode(&mut self, limits: Limits, envelope: &mut Envelope<'_>) -> Result<(), ParseError> {
        if self.fixed(6)? != 3 {
            return Err(ParseError::Malformed);
        }
        if self.fixed(6)? != 1 {
            return Err(ParseError::Malformed);
        }
        let entries = self.fixed(12)?;
        if entries > u64::from(limits.sections) {
            return Err(ParseError::LimitExceeded);
        }
        let mut previous = 0;
        for _ in 0..entries {
            self.budget.charge()?;
            let range = self.bit()? == 1;
            let delta = self.fibonacci()?;
            let start = self.budget.add(previous, delta)?;
            if start > u64::from(limits.max_id) {
                return Err(ParseError::LimitExceeded);
            }
            if start <= previous {
                return Err(ParseError::Malformed);
            }
            let end = if range {
                let width = self.fibonacci()?;
                self.budget.add(start, width)?
            } else {
                start
            };
            envelope.append_range(start, end, limits, &mut self.budget)?;
            previous = end;
        }
        self.padding()
    }

    fn padding(&mut self) -> Result<(), ParseError> {
        // Length calculations consume work even when there are no tail bits.
        self.budget.charge()?;
        let canonical = self.position.div_ceil(6);
        self.budget.charge()?;
        let byte_len = self.position.div_ceil(8);
        self.budget.charge()?;
        let byte_bits = byte_len.checked_mul(8).ok_or(ParseError::LimitExceeded)?;
        self.budget.charge()?;
        let go_writer = byte_bits.div_ceil(6);
        if self.bytes.len() != canonical && self.bytes.len() != go_writer {
            return Err(ParseError::Malformed);
        }
        self.budget.charge()?;
        let bits = self
            .bytes
            .len()
            .checked_mul(6)
            .ok_or(ParseError::LimitExceeded)?;
        while self.position < bits {
            if self.bit()? != 0 {
                return Err(ParseError::Malformed);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Envelope, Limits, LimitsError, ParseError, parse};
    use proptest::collection::btree_set;
    use proptest::prelude::*;
    use std::{collections::BTreeSet, mem, str};

    mod headers;

    fn error(raw: &str, limits: Limits, expected: ParseError) {
        assert_eq!(parse(raw, limits).expect_err("invalid envelope"), expected);
    }

    // Specification-derived test encoder, not the reference writer or production code.
    fn fibonacci(mut value: u64) -> String {
        assert!(value > 0);
        let mut weights = vec![1_u64, 2];
        while *weights.last().unwrap() <= value {
            let len = weights.len();
            weights.push(
                weights[len.checked_sub(1).unwrap()]
                    .checked_add(weights[len.checked_sub(2).unwrap()])
                    .unwrap(),
            );
        }
        weights.pop();
        let mut bits = vec![b'0'; weights.len()];
        for (index, weight) in weights.iter().enumerate().rev() {
            if *weight <= value {
                bits[index] = b'1';
                value = value.checked_sub(*weight).unwrap();
            }
        }
        bits.push(b'1');
        String::from_utf8(bits).unwrap()
    }

    fn encode(bits: &str) -> String {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        bits.as_bytes()
            .chunks(6)
            .map(|chunk| {
                let value = chunk
                    .iter()
                    .fold(0_u8, |value, bit| (value << 1_u32) | u8::from(*bit == b'1'));
                char::from(
                    ALPHABET[usize::from(value << 6_usize.checked_sub(chunk.len()).unwrap())],
                )
            })
            .collect()
    }

    fn header(entries: &[(u64, u64)]) -> String {
        let mut bits = format!("000011000001{:012b}", entries.len());
        let mut previous = 0;
        for &(start, end) in entries {
            bits.push(if start == end { '0' } else { '1' });
            bits.push_str(&fibonacci(start.checked_sub(previous).unwrap()));
            if start != end {
                bits.push_str(&fibonacci(end.checked_sub(start).unwrap()));
            }
            previous = end;
        }
        encode(&bits)
    }

    fn envelope(entries: &[(u64, u64)], slots: usize) -> String {
        format!("{}{}", header(entries), "~".repeat(slots))
    }

    #[test]
    fn public_api_cumulative_singletons_and_both_padding_forms() {
        // Header-only probes: these payloads are placeholders, not TCF/USP fixtures.
        for (header, ids) in [
            ("DBABM", &[2][..]),
            ("DBABMA", &[2][..]),
            ("DBACNY", &[2, 6][..]),
            ("DBACNYA", &[2, 6][..]),
            ("DBADNb", &[2, 6, 7][..]),
            ("DBADNbA", &[2, 6, 7][..]),
        ] {
            let raw = format!("{header}{}", "~placeholder".repeat(ids.len()));
            let parsed = parse(&raw, Limits::default()).unwrap();
            assert_eq!(parsed.sections().map(|(id, _)| id).collect::<Vec<_>>(), ids);
            assert_eq!(parsed.raw(), raw);
        }
    }

    #[test]
    fn borrowed_opaque_payloads_empty_slots_and_wire_order() {
        let raw = "DBACNY~private.\u{e9}.\u{1f600}=/+~";
        let parsed = parse(raw, Limits::default()).unwrap();
        assert_eq!(parsed.section(2), Some("private.\u{e9}.\u{1f600}=/+"));
        assert_eq!(parsed.section(6), Some(""));
        assert_eq!(parsed.section(0), None);
        assert_eq!(parsed.section(7), None);
        assert_eq!(parsed.raw().as_ptr(), raw.as_ptr());
        assert_eq!(
            parsed.section(2).unwrap().as_ptr(),
            raw.get(7..).unwrap().as_ptr()
        );
        assert_eq!(
            parsed.section(6).unwrap().as_ptr(),
            raw.get(raw.len()..).unwrap().as_ptr()
        );
        assert_eq!(
            parsed.sections().collect::<Vec<_>>(),
            vec![(2, "private.\u{e9}.\u{1f600}=/+"), (6, "")]
        );
    }

    #[test]
    fn zero_envelope_is_not_absent_input() {
        assert_eq!(
            parse("DBAA", Limits::default()).unwrap().sections().count(),
            0
        );
        for raw in ["", "~", "DBAA~", "DBABM", "DBABM~~", "DBACNY~"] {
            error(raw, Limits::default(), ParseError::Malformed);
        }
    }

    #[test]
    fn unknown_ids_and_mixed_adjacent_ranges_are_opaque() {
        let raw = envelope(&[(1, 3), (4, 5), (99, 99), (101, 103)], 9);
        let parsed = parse(&raw, Limits::default()).unwrap();
        assert_eq!(
            parsed.sections().map(|(id, _)| id).collect::<Vec<_>>(),
            [1, 2, 3, 4, 5, 99, 101, 102, 103]
        );
        assert_eq!(parsed.section(99), Some(""));
    }

    #[test]
    fn spec_derived_id_boundaries_do_not_wrap() {
        let raw = envelope(&[(0xFFFF, 0xFFFF)], 1);
        assert_eq!(
            parse(&raw, Limits::default()).unwrap().section(0xFFFF),
            Some("")
        );
        for entries in [
            &[(0x0001_0000, 0x0001_0000)][..],
            &[(0xFFFF, 0x0001_0000)],
            &[(0xFFFF, 0xFFFF), (0x0001_0000, 0x0001_0000)],
            &[(1, 1), (u64::from(u32::MAX) + 1, u64::from(u32::MAX) + 1)],
        ] {
            error(
                &envelope(entries, entries.len()),
                Limits::default(),
                ParseError::LimitExceeded,
            );
        }
    }

    #[test]
    fn padding_is_only_canonical_or_go_writer_zero_tail() {
        for raw in [
            "DBABN~",
            "DBABMB~",
            "DBABMAA~",
            "DBAAA",
            "DBAAB",
            "DBACNZ~~",
            "DBACNYB~~",
            "DBACNYAA~~",
            "DBADNbB~~~",
            "DBADNbAA~~~",
        ] {
            error(raw, Limits::default(), ParseError::Malformed);
        }
    }

    #[test]
    fn fixed_field_truncation_type_version_and_alphabet() {
        for raw in [
            "D",
            "DB",
            "DBA",
            "DBAB~",
            "DBABIA~",
            "ABAA",
            "DCAA",
            "DBA=",
            "DBA+",
            "DBA/",
            "DBA ",
            "DBA\n",
            "DBA\u{e9}",
            "DBAB\0~",
        ] {
            error(raw, Limits::default(), ParseError::Malformed);
        }
    }

    #[test]
    fn diagnostics_are_fixed_and_payload_free() {
        let parsed = parse("DBABM~private-consent-marker", Limits::default()).unwrap();
        assert_eq!(format!("{parsed:?}"), "Envelope { .. }");
        assert_eq!(ParseError::Malformed.to_string(), "malformed GPP envelope");
        assert_eq!(
            ParseError::LimitExceeded.to_string(),
            "GPP envelope limit exceeded"
        );
        assert_eq!(format!("{:?}", ParseError::Malformed), "Malformed");
        assert_eq!(format!("{:?}", ParseError::LimitExceeded), "LimitExceeded");
        assert_eq!(LimitsError.to_string(), "unsupported GPP limits");
    }

    #[test]
    fn lower_only_limits_validate_separately() {
        assert_eq!(
            Limits::new(0x0004_0000, 4096, 64, 0xFFFF, 0x0008_0000).unwrap(),
            Limits::default()
        );
        for result in [
            Limits::new(0x0004_0001, 4096, 64, 0xFFFF, 0x0008_0000),
            Limits::new(0x0004_0000, 4097, 64, 0xFFFF, 0x0008_0000),
            Limits::new(0x0004_0000, 4096, 65, 0xFFFF, 0x0008_0000),
            Limits::new(0x0004_0000, 4096, 64, 0x0001_0000, 0x0008_0000),
            Limits::new(0x0004_0000, 4096, 64, 0xFFFF, 0x0008_0001),
        ] {
            assert_eq!(result, Err(LimitsError));
        }
        Limits::new(0, 0, 0, 0, 0).unwrap();
        parse("DBAA", Limits::new(4, 4, 0, 0, 0x0008_0000).unwrap()).unwrap();
        error(
            "DBAA",
            Limits::new(4, 4, 0, 0, 0).unwrap(),
            ParseError::LimitExceeded,
        );
    }

    #[test]
    fn raw_byte_cap_exact_and_over_including_unicode() {
        let raw = format!("DBABM~{}", ".".repeat(0x0003_FFFA));
        parse(&raw, Limits::default()).unwrap();
        error(
            &format!("{raw}."),
            Limits::default(),
            ParseError::LimitExceeded,
        );
        parse(
            "DBABM~\u{e9}",
            Limits::new(8, 5, 1, 2, 0x0008_0000).unwrap(),
        )
        .unwrap();
        error(
            "DBABM~\u{e9}",
            Limits::new(7, 5, 1, 2, 0x0008_0000).unwrap(),
            ParseError::LimitExceeded,
        );
    }

    #[test]
    fn header_cap_precedes_semantics() {
        error(&"!".repeat(4096), Limits::default(), ParseError::Malformed);
        error(
            &"!".repeat(4097),
            Limits::default(),
            ParseError::LimitExceeded,
        );
        parse("DBABM~", Limits::new(10, 5, 1, 2, 0x0008_0000).unwrap()).unwrap();
        error(
            "DBABMA~",
            Limits::new(10, 5, 1, 2, 0x0008_0000).unwrap(),
            ParseError::LimitExceeded,
        );
    }

    #[test]
    fn raw_slots_count_empty_slots_before_semantics() {
        error(
            &format!("!{}", "~".repeat(64)),
            Limits::default(),
            ParseError::Malformed,
        );
        error(
            &format!("!{}", "~".repeat(65)),
            Limits::default(),
            ParseError::LimitExceeded,
        );
        error(
            &format!("!{}", "~".repeat(0x0003_FFFF)),
            Limits::default(),
            ParseError::LimitExceeded,
        );
        error(
            "!~~",
            Limits::new(10, 10, 1, 10, 0).unwrap(),
            ParseError::LimitExceeded,
        );
    }

    #[test]
    fn decoded_cap_checked_before_expansion_or_slot_mismatch() {
        assert_eq!(
            parse(&envelope(&[(1, 64)], 64), Limits::default())
                .unwrap()
                .sections()
                .count(),
            64
        );
        for entries in [&[(1, 65)][..], &[(1, 0xFFFF)], &[(1, 32), (40, 72)]] {
            error(
                &envelope(entries, 0),
                Limits::default(),
                ParseError::LimitExceeded,
            );
        }
        error(
            &envelope(&[(1, 2)], 1),
            Limits::new(100, 100, 1, 0xFFFF, 0x0008_0000).unwrap(),
            ParseError::LimitExceeded,
        );
        error("DBBB", Limits::default(), ParseError::LimitExceeded); // 65 entries, no entries' bits
        error(
            "DBAB",
            Limits::new(100, 100, 0, 0xFFFF, 0x0008_0000).unwrap(),
            ParseError::LimitExceeded,
        );
    }

    #[test]
    fn id_cap_exact_and_over() {
        let limits = Limits::new(100, 100, 2, 6, 0x0008_0000).unwrap();
        parse("DBACNY~~", limits).unwrap();
        error(&envelope(&[(7, 7)], 1), limits, ParseError::LimitExceeded);
        error(&envelope(&[(1, 7)], 1), limits, ParseError::LimitExceeded);
    }

    #[test]
    fn early_semantic_error_wins_over_later_alphabet_or_work() {
        // Validation, alphabet subtraction, and six bit/shift/cursor triples.
        let limits = Limits::new(100, 100, 64, 0xFFFF, 20).unwrap();
        error("!BAA", limits, ParseError::Malformed);
        error("AAAA!", limits, ParseError::Malformed);
        error("DBAA!", limits, ParseError::LimitExceeded);
        let version_limits = Limits::new(100, 100, 64, 0xFFFF, 40).unwrap();
        error("DCAA!", version_limits, ParseError::Malformed);
    }

    #[test]
    fn fixed_width_overflow_is_distinct_from_truncation_and_zero_is_not_an_id() {
        // A long unterminated zero integer exhausts u64 weights, not the work budget.
        error(
            &format!("DBAB{}", "A".repeat(17)),
            Limits::default(),
            ParseError::LimitExceeded,
        );
        error("DBABAA", Limits::default(), ParseError::Malformed);
        error(
            "DBABAAAAAAAAAAAAAAA",
            Limits::default(),
            ParseError::Malformed,
        );
    }

    #[test]
    fn repeat_parse_has_no_retained_state() {
        for _ in 0_u8..64 {
            error("DBABN~", Limits::default(), ParseError::Malformed);
            error(
                "DBABM~",
                Limits::new(100, 100, 64, 0xFFFF, 0).unwrap(),
                ParseError::LimitExceeded,
            );
            assert_eq!(
                parse("DBABM~", Limits::default()).unwrap().section(2),
                Some("")
            );
            assert_eq!(
                parse("DBAA", Limits::default()).unwrap().sections().count(),
                0
            );
        }
    }

    #[test]
    fn result_is_fixed_inline_storage_without_drop() {
        assert!(!mem::needs_drop::<Envelope<'_>>());
        assert!(mem::size_of::<Envelope<'_>>() <= 1600);
    }

    #[test]
    fn charges_are_before_writes_and_range_caps_precede_expansion() {
        let mut parsed = Envelope::empty("");
        let mut budget = super::Budget { remaining: 7 };
        // Three count operations, first length/write, ID increment, second length.
        // The second output-ID write fails its charge without mutating the slot/length.
        assert_eq!(
            parsed.append_range(1, 2, Limits::default(), &mut budget),
            Err(ParseError::LimitExceeded)
        );
        assert_eq!(parsed.sections().collect::<Vec<_>>(), [(1, "")]);
        assert_eq!(parsed.entries[1].id, 0);
        assert_eq!(budget.remaining, 0);
        let mut oversized = Envelope::empty("");
        let mut full_budget = super::Budget {
            remaining: 0x0008_0000,
        };
        assert_eq!(
            oversized.append_range(1, 0xFFFF, Limits::default(), &mut full_budget),
            Err(ParseError::LimitExceeded)
        );
        assert_eq!(oversized.len, 0);
        assert!(oversized.entries.iter().all(|entry| entry.id == 0));
    }

    #[test]
    fn reader_validates_each_character_once_and_stops_before_mutation() {
        let mut reader = super::Reader::new("D!", 20);
        assert_eq!(reader.fixed(6), Ok(3));
        assert_eq!(reader.budget.remaining, 0);
        assert_eq!(reader.position, 6);
        assert_eq!(reader.bit(), Err(ParseError::LimitExceeded));
        assert_eq!(reader.position, 6);
        assert_eq!(reader.cached, Some(3));
        let mut invalid = super::Reader::new("!", 1);
        assert_eq!(invalid.bit(), Err(ParseError::Malformed));
        assert_eq!(invalid.position, 0);
        assert_eq!(invalid.cached, None);
        assert_eq!(invalid.budget.remaining, 0);
    }

    #[test]
    fn budget_checked_arithmetic_never_wraps_or_spends_after_exhaustion() {
        let mut budget = super::Budget { remaining: 1 };
        assert_eq!(budget.add(u64::MAX, 1), Err(ParseError::LimitExceeded));
        assert_eq!(budget.remaining, 0);
        assert_eq!(budget.add(0, 0), Err(ParseError::LimitExceeded));
        assert_eq!(budget.remaining, 0);
    }

    #[test]
    fn pinned_go_writer_and_reader_vectors() {
        assert_eq!(headers::GO_HEADERS.len(), 12);
        let mut canonical_headers = BTreeSet::new();
        for &(canonical, go_writer, ids) in headers::GO_HEADERS {
            assert!(canonical_headers.insert(canonical));
            assert!(ids.iter().all(|id| *id != 0));
            assert!(
                ids.iter()
                    .zip(ids.iter().skip(1))
                    .all(|(left, right)| left < right)
            );
            for encoded in [canonical, go_writer] {
                let raw = format!("{encoded}{}", "~".repeat(ids.len()));
                let parsed = parse(&raw, Limits::default()).unwrap();
                assert_eq!(parsed.sections().map(|(id, _)| id).collect::<Vec<_>>(), ids);
            }
            for end in 0..canonical.len() {
                let raw = format!("{}{}", canonical.get(..end).unwrap(), "~".repeat(ids.len()));
                error(&raw, Limits::default(), ParseError::Malformed);
            }
        }
    }

    #[test]
    fn operation_cap_is_exact_and_padding_is_charged() {
        // Uppercase character: validation + subtraction. Each bit: read + shift
        // subtraction + cursor addition. Padding: three ceil divisions + two
        // multiplications. DBABM/DBABMA also perform nine entry/ID operations.
        for (raw, required) in [("DBAA", 85), ("DBABM~", 114), ("DBABMA~", 134)] {
            parse(raw, Limits::new(100, 100, 64, 0xFFFF, required).unwrap()).unwrap();
            for budget in 0..required {
                error(
                    raw,
                    Limits::new(100, 100, 64, 0xFFFF, budget).unwrap(),
                    ParseError::LimitExceeded,
                );
            }
        }
    }

    #[test]
    fn zero_envelope_does_not_succeed_on_the_old_fixed_field_budget() {
        error(
            "DBAA",
            Limits::new(100, 100, 64, 0xFFFF, 28).unwrap(),
            ParseError::LimitExceeded,
        );
    }

    #[test]
    fn exhausted_budget_precedes_excess_padding_length() {
        error(
            "DBAAA",
            Limits::new(100, 100, 64, 0xFFFF, 28).unwrap(),
            ParseError::LimitExceeded,
        );
        for operations in 0..4 {
            let mut reader = super::Reader::new("DBAAA", operations);
            reader.position = 24;
            assert_eq!(reader.padding(), Err(ParseError::LimitExceeded));
            assert_eq!(reader.position, 24);
            assert_eq!(reader.budget.remaining, 0);
        }
        let mut reader = super::Reader::new("DBAAA", 4);
        reader.position = 24;
        assert_eq!(reader.padding(), Err(ParseError::Malformed));
    }

    #[test]
    fn padding_arithmetic_is_charged_even_without_tail_bits() {
        for operations in 0..5 {
            let mut reader = super::Reader::new("DBAA", operations);
            reader.position = 24;
            assert_eq!(reader.padding(), Err(ParseError::LimitExceeded));
            assert_eq!(reader.position, 24);
            assert_eq!(reader.budget.remaining, 0);
        }
        let mut reader = super::Reader::new("DBAA", 5);
        reader.position = 24;
        reader.padding().unwrap();
        assert_eq!(reader.budget.remaining, 0);
    }

    #[test]
    fn cursor_arithmetic_is_precharged_before_position_changes() {
        for operations in 0..3 {
            let mut reader = super::Reader::new("A", operations);
            reader.position = 1;
            reader.cached = Some(0);
            assert_eq!(reader.bit(), Err(ParseError::LimitExceeded));
            assert_eq!(reader.position, 1);
            assert_eq!(reader.cached, Some(0));
            assert_eq!(reader.budget.remaining, 0);
        }
        let mut reader = super::Reader::new("A", 3);
        reader.position = 1;
        reader.cached = Some(0);
        assert_eq!(reader.bit(), Ok(0));
        assert_eq!(reader.position, 2);
        assert_eq!(reader.budget.remaining, 0);
    }

    #[test]
    fn alphabet_arithmetic_is_precharged_before_cache_mutation() {
        for (raw, validation_operations) in [("A", 2), ("a", 3), ("0", 3)] {
            for operations in 0..validation_operations {
                let mut reader = super::Reader::new(raw, operations);
                assert_eq!(reader.bit(), Err(ParseError::LimitExceeded));
                assert_eq!(reader.cached, None);
                assert_eq!(reader.position, 0);
                assert_eq!(reader.budget.remaining, 0);
            }
        }
    }

    #[test]
    fn alphabet_accepts_exactly_the_url_safe_64_bytes() {
        for byte in 0_u8..=127 {
            let bytes = [byte];
            let raw = str::from_utf8(&bytes).unwrap();
            let operations = if byte.is_ascii_uppercase() {
                20
            } else if byte.is_ascii_lowercase() || byte.is_ascii_digit() {
                21
            } else {
                19
            };
            let mut reader = super::Reader::new(raw, operations);
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_') {
                reader.fixed(6).unwrap();
                assert_eq!(reader.budget.remaining, 0);
            } else {
                assert_eq!(reader.fixed(6), Err(ParseError::Malformed));
                assert_eq!(reader.position, 0);
            }
        }
    }

    #[test]
    fn u64_fibonacci_terminal_weight_and_overflows_are_platform_independent() {
        let terminal = encode(&format!("{}11", "0".repeat(91)));
        assert_eq!(
            super::Reader::new(&terminal, 1000).fibonacci(),
            Ok(12_200_160_415_121_876_738)
        );
        let sum_overflow = encode(&format!("{}1", "01".repeat(46)));
        assert_eq!(
            super::Reader::new(&sum_overflow, 1000).fibonacci(),
            Err(ParseError::LimitExceeded)
        );
        let weight_overflow = encode(&"0".repeat(96));
        assert_eq!(
            super::Reader::new(&weight_overflow, 1000).fibonacci(),
            Err(ParseError::LimitExceeded)
        );
        let truncated = encode(&"0".repeat(90));
        assert_eq!(
            super::Reader::new(&truncated, 1000).fibonacci(),
            Err(ParseError::Malformed)
        );
    }

    #[test]
    #[expect(
        clippy::print_stderr,
        reason = "Explicit layout evidence for admission accounting"
    )]
    fn per_parse_storage_measurements() {
        assert!(!mem::needs_drop::<super::Reader<'_>>());
        eprintln!(
            "GPP layouts: Envelope={} Section={} Reader={} Budget={} Limits={} Split={}",
            mem::size_of::<Envelope<'_>>(),
            mem::size_of::<super::Section<'_>>(),
            mem::size_of::<super::Reader<'_>>(),
            mem::size_of::<super::Budget>(),
            mem::size_of::<Limits>(),
            mem::size_of::<str::Split<'_, char>>()
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, ..ProptestConfig::default() })]

        #[test]
        fn arbitrary_strings_never_panic_and_results_borrow(input in any::<String>()) {
            if let Ok(parsed) = parse(&input, Limits::default()) {
                prop_assert_eq!(parsed.raw().as_ptr(), input.as_ptr());
                prop_assert!(parsed.sections().count() <= 64);
                let mut last = 0;
                for (id, payload) in parsed.sections() {
                    prop_assert!(id > last);
                    prop_assert!(input.contains(payload));
                    last = id;
                }
            }
        }

        #[test]
        fn spec_singletons_roundtrip(ids in btree_set(1_u16..=0xFFFF, 0..=64)) {
            let entries: Vec<_> = ids.iter().map(|&id| (u64::from(id), u64::from(id))).collect();
            let raw = envelope(&entries, ids.len());
            let parsed = parse(&raw, Limits::default()).unwrap();
            prop_assert_eq!(parsed.sections().map(|(id, _)| id).collect::<Vec<_>>(), ids.into_iter().collect::<Vec<_>>());
        }

        #[test]
        fn opaque_payload_remains_opaque(payload in "[^~]{0,2048}") {
            let raw = format!("DBABM~{payload}");
            prop_assert_eq!(parse(&raw, Limits::default()).unwrap().section(2), Some(payload.as_str()));
        }

        #[test]
        fn valid_prefix_arbitrary_header_and_budgets_are_bounded(tail in "[A-Za-z0-9_!?-]{0,200}", budget in 0_u64..=1024) {
            let raw = format!("DB{tail}~");
            let limits = Limits::new(1024, 256, 64, 0xFFFF, budget).unwrap();
            let first = parse(&raw, limits).map(|parsed| parsed.sections().collect::<Vec<_>>());
            let second = parse(&raw, limits).map(|parsed| parsed.sections().collect::<Vec<_>>());
            prop_assert_eq!(first, second);
        }
    }
}
