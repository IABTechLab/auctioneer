//! Strict, allocation-free validation of a USP v1 value.
//!
//! This module does not select consent sources, emit warnings, or enforce
//! CCPA. It only validates the value and exposes its sale-opt-out flag.

use std::fmt;

/// An immutable, validated four-byte USP v1 value.
///
/// Storage is private so every value satisfies the grammar accepted by
/// [`Self::parse`]. Debug output omits the consent bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Usp([u8; 4]);

impl fmt::Debug for Usp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Usp").finish_non_exhaustive()
    }
}

/// A nonempty input was not a valid USP v1 value.
///
/// This fixed diagnostic never retains or formats the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid USP value")]
pub struct UspParseError;

impl Usp {
    /// Parse exactly `1` followed by three ASCII `Y`, `N`, or `-` flags.
    ///
    /// Absent and empty inputs return `Ok(None)`. No trimming, case folding,
    /// prefix acceptance, or allocation is performed.
    ///
    /// # Errors
    ///
    /// Returns [`UspParseError`] for every nonempty input outside this grammar.
    pub fn parse(input: Option<&str>) -> Result<Option<Self>, UspParseError> {
        let value = match input {
            None | Some("") => return Ok(None),
            Some(present) => present,
        };
        let bytes: [u8; 4] = value
            .as_bytes()
            .try_into()
            .map_err(|_wrong_length| UspParseError)?;
        if !matches!(
            bytes,
            [
                b'1',
                b'Y' | b'N' | b'-',
                b'Y' | b'N' | b'-',
                b'Y' | b'N' | b'-'
            ]
        ) {
            return Err(UspParseError);
        }
        Ok(Some(Self(bytes)))
    }

    /// Whether the sale-opt-out flag (byte index 2) is `Y`.
    ///
    /// This reports the flag only; it does not determine CCPA applicability.
    #[must_use]
    pub fn sale_opt_out(&self) -> bool {
        matches!(self.0, [_, _, b'Y', _])
    }

    /// The validated bytes, unchanged from the input, for later projection.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::mem;
    use std::str;

    use proptest::prelude::*;

    use crate::{Usp, UspParseError};

    #[test]
    fn absent_and_empty_are_absent() {
        assert_eq!(Usp::parse(None), Ok(None));
        assert_eq!(Usp::parse(Some("")), Ok(None));
    }

    #[test]
    fn all_27_flag_combinations_preserve_bytes_and_sale_opt_out() {
        for notice in *b"YN-" {
            for sale in *b"YN-" {
                for lspa in *b"YN-" {
                    let bytes = [b'1', notice, sale, lspa];
                    let input = str::from_utf8(&bytes).expect("ASCII fixture");
                    let usp = Usp::parse(Some(input))
                        .expect("valid flags")
                        .expect("present value");

                    assert_eq!(usp.as_bytes(), &bytes);
                    assert_eq!(usp.sale_opt_out(), sale == b'Y');
                }
            }
        }
    }

    #[test]
    fn only_the_third_byte_controls_sale_opt_out() {
        for (input, expected) in [("1NYN", true), ("1YNY", false), ("1Y-Y", false)] {
            let usp = Usp::parse(Some(input))
                .expect("valid fixture")
                .expect("present value");
            assert_eq!(usp.sale_opt_out(), expected, "{input}");
        }
    }

    #[test]
    fn every_invalid_ascii_flag_byte_is_rejected_in_each_slot() {
        for slot in 1..=3 {
            for byte in 0_u8..=127 {
                if b"YN-".contains(&byte) {
                    continue;
                }
                let mut bytes = *b"1NNN";
                bytes[slot] = byte;
                let input = str::from_utf8(&bytes).expect("ASCII fixture");

                assert_eq!(
                    Usp::parse(Some(input)),
                    Err(UspParseError),
                    "slot {slot}, byte {byte}"
                );
            }
        }
    }

    #[test]
    fn every_other_ascii_version_is_rejected() {
        for version in 0_u8..=127 {
            if version == b'1' {
                continue;
            }
            let bytes = [version, b'Y', b'N', b'-'];
            let input = str::from_utf8(&bytes).expect("ASCII fixture");
            assert_eq!(Usp::parse(Some(input)), Err(UspParseError));
        }
    }

    #[test]
    fn rejects_nonempty_wrong_lengths_prefixes_and_suffixes() {
        for input in [
            "1", "1Y", "1YN", "1YNNN", "11YNN", "x1YNN", "1YNNx", "1YNN1NNN", "1YNN\0", "\x001YNN",
        ] {
            assert_eq!(Usp::parse(Some(input)), Err(UspParseError), "{input:?}");
        }

        let long_suffix = format!("1YNN{}", "N".repeat(0x1_0000));
        assert_eq!(Usp::parse(Some(&long_suffix)), Err(UspParseError));
    }

    #[test]
    fn rejects_lowercase_whitespace_and_unicode_without_normalizing() {
        for input in [
            "1ynn",
            "1yNN",
            "1YnN",
            "1YNn",
            " ",
            "\t\r\n",
            " 1YNN",
            "1YNN ",
            "\t1YNN",
            "1YNN\n",
            "1 NN",
            "1Y\tN",
            "1YN\r",
            "\u{ff11}YNN",
            "1\u{ff39}NN",
            "1Y\u{ff2e}N",
            "1YN\u{2013}",
            "1\u{e9}N",
            "1\u{4e2d}",
            "\u{1f600}",
            "\u{feff}1YNN",
            "1YNN\u{a0}",
        ] {
            assert_eq!(Usp::parse(Some(input)), Err(UspParseError), "{input:?}");
        }
    }

    #[test]
    fn parse_error_diagnostics_are_fixed_and_do_not_echo_input() {
        for input in ["private-consent-payload", "1YNN.sensitive-suffix", "2YYY"] {
            let error = Usp::parse(Some(input)).expect_err("invalid fixture");
            assert_eq!(error, UspParseError);
            assert_eq!(error.to_string(), "invalid USP value");
            assert_eq!(format!("{error:?}"), "UspParseError");
        }
    }

    #[test]
    fn value_debug_does_not_expose_consent_bytes() {
        for input in ["1NYN", "1YNY", "1Y-Y"] {
            let usp = Usp::parse(Some(input))
                .expect("valid fixture")
                .expect("present value");
            assert_eq!(format!("{usp:?}"), "Usp { .. }");
        }
    }

    #[test]
    fn value_and_error_have_fixed_inline_storage() {
        assert_eq!(mem::size_of::<Usp>(), 4);
        assert_eq!(mem::size_of::<UspParseError>(), 0);
        assert!(!mem::needs_drop::<Usp>());
        assert!(!mem::needs_drop::<UspParseError>());
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 256,
            failure_persistence: None,
            ..ProptestConfig::default()
        })]

        #[test]
        fn arbitrary_strings_follow_exact_grammar_without_panicking(input in any::<String>()) {
            let result = Usp::parse(Some(&input));
            let bytes = input.as_bytes();
            let valid = bytes.len() == 4
                && bytes.first() == Some(&b'1')
                && bytes.iter().skip(1).all(|byte| b"YN-".contains(byte));

            if input.is_empty() {
                prop_assert_eq!(result, Ok(None));
            } else if valid {
                let usp = result.expect("valid grammar").expect("nonempty value");
                prop_assert_eq!(usp.as_bytes().as_slice(), bytes);
                prop_assert_eq!(usp.sale_opt_out(), bytes.get(2) == Some(&b'Y'));
            } else {
                prop_assert_eq!(result, Err(UspParseError));
            }
        }
    }
}
