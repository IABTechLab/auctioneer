//! Deterministic `BidResponse` → JSON bytes, bounded by the envelope's
//! `final_response_bytes`.
//!
//! The boot envelope reserves exactly `final_response_bytes` for one
//! serialized response, so serialization must not hand back more than
//! that. Two properties matter:
//!
//! - **Over-cap is an ERROR, never a truncation.** A truncated JSON body
//!   is a malformed body: the client cannot tell it from a complete one
//!   without parsing, and a bidder-facing response that silently loses
//!   bids is worse than no response. Over-cap therefore produces a
//!   500-class failure and the caller emits a refusal instead of a
//!   partial auction result.
//! - **Nothing over-cap is ever SENT.** The cap is checked before the
//!   bytes are handed to the caller, so an oversized response becomes a
//!   refusal rather than a body.
//!
//! The output buffer is reserved fallibly at the cap and never grows
//! during serialization, including when rejecting an oversized response.
//! This bounds output storage, not the response tree or all serializer
//! scratch and error metadata.
//!
//! Auxiliary JSON payloads use a counted-size reservation through
//! [`serialize_json_exact`], without imposing an additional output limit.

use auctioneer_bidder::CappedWriter;
use auctioneer_openrtb::types::bid_response::BidResponse;
use std::io::ErrorKind;

use crate::host_primitives::gates::boot_limits;

/// Why a bounded serialization did not produce bytes.
///
/// Deliberately NOT a `std::error::Error` implementor. The workspace
/// denies the `restriction` lint group, whose `missing_trait_methods`
/// arm requires every provided method of an implemented trait to be
/// written out — and `Error`'s `provide` is nightly-only while
/// `description` and `cause` are deprecated, so the impl is
/// unwriteable here without an exemption. Callers distinguish a cap
/// breach from an encoding/reservation failure, not a boxed `dyn Error`.
/// The auction handler maps both variants to a 500. [`SerializeError::message`]
/// supplies the text a log line needs.
#[derive(Debug)]
pub enum SerializeError {
    /// The serialized output exceeds its supplied reservation.
    ///
    /// Carries the CAP rather than the over-cap length: serialization
    /// stops at the cap, so the full length is not computed.
    OverCap {
        /// The envelope's `final_response_bytes`, or the counted JSON size.
        cap: usize,
    },
    /// A serializer or output-buffer reservation failure. Genuine
    /// serializer errors are preserved; reservation failures carry an
    /// I/O error with kind `OutOfMemory`.
    Encoding(serde_json::Error),
}

impl SerializeError {
    /// A log-ready description.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            SerializeError::OverCap { cap } => {
                format!("serialized response exceeds the {cap}-byte final-response cap")
            }
            SerializeError::Encoding(err) => {
                format!("response serialization failed: {err}")
            }
        }
    }
}

/// Serialize a `BidResponse` to JSON, refusing to exceed `cap` bytes.
///
/// `cap` is the envelope's `final_response_bytes`. On success the
/// returned buffer is at most `cap` bytes long; on over-cap the caller
/// gets an error and NO bytes.
///
/// Reserves `cap` bytes fallibly, then streams into a [`CappedWriter`]
/// that cannot grow the buffer. No full oversized temporary is built.
/// Reservation failure (including capacity overflow) is terminal, with
/// no retry or unbounded fallback.
///
/// # Errors
/// Returns [`SerializeError::OverCap`] when the response exceeds `cap` —
/// the caller must emit a 500 rather than a truncated body — and
/// [`SerializeError::Encoding`] for a genuine encoding or reservation failure.
pub fn serialize_response_within(
    resp: &BidResponse,
    cap: usize,
) -> Result<Vec<u8>, SerializeError> {
    serialize_within(resp, cap)
}

/// Count a borrowed payload's JSON bytes, then reserve its output once, fallibly.
///
/// The measured size is a reservation, not a configured output limit. A custom
/// serializer may change between passes; the capped writer still refuses growth
/// beyond that reservation, with no retry or unbounded fallback. This bounds only
/// output storage, not the payload tree or custom serializer scratch allocations.
pub(crate) fn serialize_json_exact<T: serde::Serialize + ?Sized>(
    value: &T,
) -> Result<Vec<u8>, SerializeError> {
    let size =
        auctioneer_runtime::serialized_json_bytes(value).map_err(SerializeError::Encoding)?;
    let cap = usize::try_from(size).map_err(|_size| {
        SerializeError::Encoding(serde_json::Error::io(ErrorKind::OutOfMemory.into()))
    })?;
    serialize_within(value, cap)
}

fn serialize_within<T: serde::Serialize + ?Sized>(
    value: &T,
    cap: usize,
) -> Result<Vec<u8>, SerializeError> {
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(cap).map_err(|_allocation| {
        SerializeError::Encoding(serde_json::Error::io(ErrorKind::OutOfMemory.into()))
    })?;
    let mut writer = CappedWriter::new(&mut bytes, cap).map_err(|_capacity| {
        SerializeError::Encoding(serde_json::Error::io(ErrorKind::OutOfMemory.into()))
    })?;
    // write_json erases serializer errors. Use the io::Write path to
    // retain them; WriteZero is this writer's cap-exhaustion signal.
    if let Err(err) = serde_json::to_writer(&mut writer, value) {
        return Err(if err.io_error_kind() == Some(ErrorKind::WriteZero) {
            SerializeError::OverCap { cap }
        } else {
            SerializeError::Encoding(err)
        });
    }
    Ok(bytes)
}

/// Serialize against the INSTALLED boot envelope's
/// `final_response_bytes`.
///
/// When no envelope is installed the response is serialized unbounded.
/// That is not a loophole left open: the cap is a memory reservation the
/// envelope makes, so with no envelope there is no reservation to
/// respect and no number to enforce. Inventing one here would apply a
/// production byte bound nobody configured. An adapter that wants the
/// bound installs an envelope at boot, and the cap becomes live for
/// every response from that moment.
///
/// This branch therefore stays on `to_vec` and is deliberately NOT
/// routed through the capped writer. A [`CappedWriter`] needs a cap to
/// pre-allocate against and a limit to refuse at; with no envelope
/// there is neither, so the only cap available would be an invented
/// one. Bounding the allocation here would mean choosing that number,
/// which is the envelope's decision rather than this function's.
///
/// # Errors
/// Returns [`SerializeError::OverCap`] when an envelope IS installed and
/// the response exceeds its `final_response_bytes`, and
/// [`SerializeError::Encoding`] for a genuine encoding or reservation failure.
pub fn serialize_response(resp: &BidResponse) -> Result<Vec<u8>, SerializeError> {
    match boot_limits() {
        Some(limits) => serialize_response_within(
            resp,
            // A cap wider than this platform's usize becomes an
            // unrepresentable reservation and fails terminally.
            usize::try_from(limits.final_response_bytes).unwrap_or(usize::MAX),
        ),
        None => serde_json::to_vec(resp).map_err(SerializeError::Encoding),
    }
}

/// Omit only the safe debug value if it cannot accompany the complete response.
/// The base response retains its original bounded-serialization failure behavior.
///
/// # Errors
/// Returns the same encoding, reservation and cap failures as the capped serializer.
pub fn serialize_response_fitting_debug_within(
    resp: &mut BidResponse,
    cap: usize,
) -> Result<Vec<u8>, SerializeError> {
    let debug = resp
        .ext
        .as_mut()
        .and_then(serde_json::Value::as_object_mut)
        .and_then(|ext| ext.remove_entry("debug"));
    let Some((key, value)) = debug else {
        return serialize_response_within(resp, cap);
    };
    let cap_bytes = u64::try_from(cap).unwrap_or(u64::MAX);
    if auctioneer_runtime::serialized_json_bytes(resp).map_err(SerializeError::Encoding)?
        > cap_bytes
    {
        return Err(SerializeError::OverCap { cap });
    }
    if let Some(ext) = resp.ext.as_mut().and_then(serde_json::Value::as_object_mut) {
        ext.insert(key, value);
    }
    if auctioneer_runtime::serialized_json_bytes(resp).map_err(SerializeError::Encoding)?
        > cap_bytes
        && let Some(ext) = resp.ext.as_mut().and_then(serde_json::Value::as_object_mut)
    {
        ext.remove("debug");
    }
    serialize_response_within(resp, cap)
}

/// Apply installed response fitting without inventing a cap for unbooted callers.
///
/// # Errors
/// Returns bounded serialization failures, or encoding failures without an envelope.
pub fn serialize_response_fitting_debug(resp: &mut BidResponse) -> Result<Vec<u8>, SerializeError> {
    match boot_limits() {
        Some(limits) => serialize_response_fitting_debug_within(
            resp,
            usize::try_from(limits.final_response_bytes).unwrap_or(usize::MAX),
        ),
        None => serialize_response(resp),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SerializeError, serialize_json_exact, serialize_response_within, serialize_within,
    };
    use auctioneer_openrtb::types::bid_response::BidResponse;
    use std::cell::Cell;
    use std::collections::BTreeMap;
    use std::io::ErrorKind;

    /// A cap large enough that nothing in these tests brushes it.
    const GENEROUS: usize = 64 * 1024;

    #[test]
    fn exact_json_reserves_only_the_encoded_length_for_borrowed_payloads() {
        #[derive(serde::Serialize)]
        struct Payload<'a> {
            text: &'a str,
            names: &'a [&'a str],
        }

        for size in [0, 1, 31, 4097, 64 * 1024 + 1] {
            let text = "a\0\n\"\\\u{00e9}".repeat(size);
            let names = ["base", text.as_str()];
            let typed = Payload {
                text: &text,
                names: &names,
            };
            let expected_typed = serde_json::to_vec(&typed).unwrap();
            let typed_bytes = serialize_json_exact(&typed).unwrap();
            assert_eq!(typed_bytes, expected_typed);
            assert_eq!(typed_bytes.capacity(), expected_typed.len());

            let value = serde_json::to_value(&typed).unwrap();
            let expected_value = serde_json::to_vec(&value).unwrap();
            let value_bytes = serialize_json_exact(&value).unwrap();
            assert_eq!(value_bytes, expected_value);
            assert_eq!(value_bytes.capacity(), expected_value.len());

            let text_bytes = serialize_json_exact(text.as_str()).unwrap();
            assert_eq!(text_bytes.len(), 2 + 15 * size, "JSON escaping is counted");
            assert_eq!(text_bytes.capacity(), text_bytes.len());
        }
        assert_eq!(serialize_json_exact(&()).unwrap(), b"null");
        assert_eq!(serialize_json_exact::<[u8]>(&[]).unwrap(), b"[]");
    }

    struct ChangingJson {
        passes: Cell<usize>,
        first: &'static str,
        second: String,
    }

    impl serde::Serialize for ChangingJson {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let pass = self.passes.get().saturating_add(1);
            self.passes.set(pass);
            serializer.serialize_str(if pass == 1 { self.first } else { &self.second })
        }
    }

    #[test]
    fn exact_json_refuses_second_pass_growth_without_retrying() {
        for size in [1, 4097, 64 * 1024 + 1] {
            let value = ChangingJson {
                passes: Cell::new(0),
                first: "",
                second: "\0".repeat(size),
            };
            assert!(matches!(
                serialize_json_exact(&value),
                Err(SerializeError::OverCap { cap: 2 })
            ));
            assert_eq!(value.passes.get(), 2, "no unbounded fallback");
        }
    }

    #[test]
    fn exact_json_does_not_require_identical_serialization_passes() {
        let value = ChangingJson {
            passes: Cell::new(0),
            first: "longer first pass",
            second: "second".into(),
        };
        let bytes = serialize_json_exact(&value).unwrap();
        assert_eq!(bytes, br#""second""#);
        assert_eq!(bytes.capacity(), value.first.len() + 2);
        assert_eq!(value.passes.get(), 2);
    }

    struct InvalidJson {
        passes: Cell<usize>,
        fail_on: usize,
    }

    impl serde::Serialize for InvalidJson {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let pass = self.passes.get().saturating_add(1);
            self.passes.set(pass);
            if pass == self.fail_on {
                serde::Serialize::serialize(&BTreeMap::from([((1_u8, 2_u8), 3_u8)]), serializer)
            } else {
                serializer.serialize_str("valid first pass")
            }
        }
    }

    #[test]
    fn exact_json_preserves_encoding_errors_from_either_pass_without_retrying() {
        let invalid = BTreeMap::from([((1_u8, 2_u8), 3_u8)]);
        let expected = serde_json::to_vec(&invalid).unwrap_err();
        for fail_on in [1, 2] {
            let value = InvalidJson {
                passes: Cell::new(0),
                fail_on,
            };
            match serialize_json_exact(&value) {
                Err(SerializeError::Encoding(err)) => {
                    assert_eq!(err.classify(), expected.classify());
                    assert_eq!(err.to_string(), expected.to_string());
                    assert_eq!(err.io_error_kind(), expected.io_error_kind());
                }
                other => panic!("expected the original encoding error, got {other:?}"),
            }
            assert_eq!(value.passes.get(), fail_on, "failed passes are terminal");
        }
    }

    #[test]
    fn an_unrepresentable_reservation_is_a_terminal_encoding_error() {
        assert!(matches!(
            serialize_within(&(), usize::MAX),
            Err(SerializeError::Encoding(err))
                if err.io_error_kind() == Some(ErrorKind::OutOfMemory)
        ));
    }

    #[test]
    fn debug_is_fitted_wholly_without_removing_bids_or_required_warnings() {
        let wire = serde_json::json!({
            "id":"r", "cur":"USD",
            "seatbid":[{"seat":"fake_safe","bid":[{"id":"b","impid":"i","price":1.23_f64}]}],
            "ext":{
                "warnings":{"prebid":[{"code":10003_i32,"message":"debug disabled for bidder"}]},
                "debug":{"httpcalls":{"fake_safe":[{"body":"\u{00e9}\\\"\0".repeat(20)}]},"truncated":false,"dropped_calls":0_i32}
            }
        });
        let original: BidResponse = serde_json::from_value(wire).unwrap();
        let mut base = original.clone();
        let removed = base
            .ext
            .as_mut()
            .unwrap()
            .as_object_mut()
            .unwrap()
            .remove("debug");
        assert!(removed.is_some());
        let base_wire = serde_json::to_vec(&base).unwrap();
        let full_wire = serde_json::to_vec(&original).unwrap();
        assert!(full_wire.len() > base_wire.len() + 1);
        for cap in [base_wire.len(), full_wire.len() - 1, full_wire.len()] {
            let mut response = original.clone();
            let actual =
                super::serialize_response_fitting_debug_within(&mut response, cap).unwrap();
            assert_eq!(
                actual,
                if cap == full_wire.len() {
                    &full_wire
                } else {
                    &base_wire
                }
                .as_slice()
            );
            assert_eq!(
                serde_json::from_slice::<serde_json::Value>(&actual).unwrap()["seatbid"],
                serde_json::to_value(&original).unwrap()["seatbid"]
            );
        }
        let mut response = original;
        assert!(matches!(
            super::serialize_response_fitting_debug_within(&mut response, base_wire.len() - 1),
            Err(SerializeError::OverCap { .. })
        ));
    }

    #[test]
    fn empty_response_serializes() {
        let bytes = serialize_response_within(&BidResponse::default(), GENEROUS)
            .expect("serialize default");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn genuine_encoding_error_keeps_serde_json_details() {
        // BidResponse currently contains only serializable values. Exercise
        // the same serializer with a map key JSON cannot represent.
        let value = BTreeMap::from([((1_u8, 2_u8), 3_u8)]);
        let expected = serde_json::to_vec(&value).expect_err("tuple keys cannot encode");
        // At cap 1 the opening brace fills the buffer, but the key
        // error is still genuine: no over-cap write was attempted.
        for cap in [1, GENEROUS] {
            let result = serialize_within(&value, cap);
            match result {
                Err(SerializeError::Encoding(err)) => {
                    assert_eq!(err.classify(), expected.classify());
                    assert_eq!(err.to_string(), expected.to_string());
                    assert_eq!(err.line(), expected.line());
                    assert_eq!(err.column(), expected.column());
                    assert_eq!(err.io_error_kind(), expected.io_error_kind());
                }
                other => panic!("expected the original serde_json error, got {other:?}"),
            }
        }
    }

    #[test]
    fn price_fallback_warnings_obey_the_final_response_cap() {
        use auctioneer_exchange::targeting::{PriceGranularityScope, parse_price_granularity};

        let mut warnings = Vec::new();
        for scope in [
            PriceGranularityScope::Global,
            PriceGranularityScope::Banner,
            PriceGranularityScope::Video,
            PriceGranularityScope::Native,
            PriceGranularityScope::Audio,
        ] {
            let _fallback = parse_price_granularity(&serde_json::Value::Null, scope, &mut warnings);
        }
        let baseline = BidResponse::default();
        let baseline_bytes = serde_json::to_vec(&baseline).unwrap();
        let response = BidResponse {
            ext: Some(serde_json::json!({"warnings": {"prebid": warnings}})),
            ..baseline
        };
        assert!(matches!(
            serialize_response_within(&response, baseline_bytes.len()),
            Err(SerializeError::OverCap { .. })
        ));
        let expected = serde_json::to_vec(&response).unwrap();
        assert_eq!(
            serialize_response_within(&response, expected.len()).unwrap(),
            expected
        );
        assert!(matches!(
            serialize_response_within(&response, expected.len() - 1_usize),
            Err(SerializeError::OverCap { .. })
        ));
    }

    /// The core bounded-serialization contract: a response that does not
    /// fit is a 500-class ERROR, and the caller receives NO bytes rather
    /// than a truncated prefix.
    #[test]
    fn final_response_over_cap_is_500_not_truncation() {
        let resp = BidResponse {
            id: "x".repeat(4096),
            ..BidResponse::default()
        };

        let full = serialize_response_within(&resp, GENEROUS).expect("fits a generous cap");
        assert!(
            full.len() > 1024,
            "the fixture must be larger than the cap under test, got {}",
            full.len(),
        );

        // One byte short of the true length is the tightest possible
        // over-cap: it proves the boundary, not merely that a tiny cap
        // fails.
        let tight = full.len().checked_sub(1).expect("non-empty response");
        let outcome = serialize_response_within(&resp, tight);

        match outcome {
            Err(SerializeError::OverCap { cap }) => {
                assert_eq!(cap, tight, "the error reports the cap it enforced");
            }
            Err(SerializeError::Encoding(err)) => {
                panic!("a cap breach must not be reported as an encoding failure: {err}")
            }
            Ok(bytes) => panic!(
                "an over-cap response must not yield bytes; got {} of them \
                 (a truncated JSON body is malformed, not a smaller response)",
                bytes.len(),
            ),
        }

        // And exactly at the cap it succeeds, byte-identical to the
        // unbounded result — the bound rejects, it does not reshape.
        let exact = serialize_response_within(&resp, full.len()).expect("the exact length fits");
        assert_eq!(exact, full, "a response at the cap is unchanged");
    }

    /// A zero cap refuses rather than returning an empty body, which
    /// would be a truncation to nothing.
    #[test]
    fn zero_cap_refuses_every_response() {
        let outcome = serialize_response_within(&BidResponse::default(), 0);
        assert!(
            matches!(outcome, Err(SerializeError::OverCap { cap: 0 })),
            "a zero cap admits no response at all",
        );
    }

    /// A response FAR over the cap is still refused rather than sent,
    /// and the caller gets no buffer back.
    ///
    /// The integration allocation guard additionally measures output
    /// storage while rejecting an oversized response.
    #[test]
    fn a_response_far_over_the_cap_is_refused_not_sent() {
        let resp = BidResponse {
            id: "y".repeat(4 * 1024 * 1024),
            ..BidResponse::default()
        };

        let outcome = serialize_response_within(&resp, 64);
        assert!(
            matches!(outcome, Err(SerializeError::OverCap { cap: 64 })),
            "a response far over the cap is refused",
        );
    }

    /// A response that exceeds the cap only because of its own encoded
    /// size is refused, with the fixture's excess established first.
    ///
    /// Named for what it checks. It does NOT observe the buffer's
    /// length or capacity, and cannot: `serialize_response_within`
    /// returns no buffer on the error path, so the filled-to-the-cap
    /// property is invisible from here. That property is measured at
    /// the allocator in `tests/serialize_allocation_bound.rs`, which is
    /// the only place the transient peak is visible — and the place a
    /// serialize-then-measure implementation, growing a separate buffer
    /// to the full response size, is caught.
    ///
    /// What this adds over its sibling above is the standalone-length
    /// guard: it pins that the fixture really does exceed `CAP`, so the
    /// refusal is the over-cap branch rather than an unexercised one.
    #[test]
    fn an_over_cap_response_is_refused_with_its_excess_established() {
        const CAP: usize = 4096;

        let resp = BidResponse {
            id: "y".repeat(4 * 1024 * 1024),
            ..BidResponse::default()
        };

        // The fixture must genuinely exceed the cap, or the assertion
        // below would be checking an unexercised branch.
        let standalone_len = serde_json::to_vec(&resp).expect("fixture encodes").len();
        assert!(
            standalone_len > CAP,
            "the fixture must exceed the cap; got {standalone_len} against {CAP}",
        );

        let outcome = serialize_response_within(&resp, CAP);
        assert!(
            matches!(outcome, Err(SerializeError::OverCap { cap: CAP })),
            "an over-cap response is refused",
        );
    }

    /// The success path allocates the reservation and not a byte more.
    ///
    /// A within-cap response comes back in a buffer whose capacity is
    /// exactly the envelope's reservation, so the bound is what was
    /// allocated rather than whatever growth doubling happened to land
    /// on.
    #[test]
    fn a_within_cap_response_allocates_exactly_the_reservation() {
        let bytes = serialize_response_within(&BidResponse::default(), GENEROUS)
            .expect("a default response fits a generous cap");
        assert_eq!(
            bytes.capacity(),
            GENEROUS,
            "the response buffer is allocated at the reservation and never grown",
        );
    }
}
