//! Portable, atomic bidder credentials without provider I/O.

use crate::MonotonicInstant;
use crate::bounded_json::count_json_nodes;
use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use std::fmt;

/// Maximum serialized credential bundle, including JSON whitespace and escapes.
pub const MAX_BIDDER_CREDENTIAL_BYTES: usize = 4096;

/// Requested parse-heap envelope under the locked Serde/Rust implementations.
/// Includes rejected strings, diagnostic growth/boxing and unescape scratch.
/// The source derivation and independent cold observations live below.
pub const BIDDER_CREDENTIAL_PARSE_PEAK_BYTES: u64 = 86_528;

/// One sequential logical construction bank, not a provider RSS guarantee.
/// Funds parse heap, two 4096-byte read/value buffers, the 1024-byte base64
/// scratch, bounded binding ownership and fixed request/future layout reserve.
/// Allocator/provider physical readiness remains a separate target prerequisite.
pub const BIDDER_CREDENTIAL_WORKING_BYTES: u64 = 128 * 1024;

const MAX_BIDDER_CREDENTIAL_NODES: u32 = 16;
const MAX_CREDENTIAL_COMPONENT_BYTES: usize = 256;
const MAX_TRACKER_BYTES: usize = 128;
const MAX_BASIC_USERPASS_BYTES: usize = 513;
const MAX_BASIC_AUTHORIZATION_BYTES: usize = 690;

/// A credential read or validation failed, without retaining secret details.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CredentialUnavailable;

impl fmt::Display for CredentialUnavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("bidder credentials unavailable")
    }
}

/// One validated rotation, retaining no separately accessible password.
pub struct BidderCredentials {
    username: Box<str>,
    tracker: Box<str>,
    authorization: Box<str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    schema_version: u8,
    username: Box<str>,
    password: Box<str>,
    tracker: Box<str>,
}

impl BidderCredentials {
    /// Parse one bounded, strict version-1 credential bundle.
    ///
    /// At most 4096 input bytes and 16 syntactic JSON nodes are admitted before
    /// typed deserialization. Username and password must each contain 1..=256
    /// UTF-8 bytes; username cannot contain a colon. Tracker is required and may
    /// contain 0..=128 bytes. Unknown, duplicate, missing and null fields fail.
    ///
    /// # Errors
    /// Returns only [`CredentialUnavailable`] for malformed or invalid input.
    pub fn parse(input: &[u8]) -> Result<Self, CredentialUnavailable> {
        preflight(input)?;
        let wire: Wire = serde_json::from_slice(input).map_err(|_error| CredentialUnavailable)?;
        if wire.schema_version != 1
            || wire.username.is_empty()
            || wire.username.len() > MAX_CREDENTIAL_COMPONENT_BYTES
            || wire.username.contains(':')
            || wire.password.is_empty()
            || wire.password.len() > MAX_CREDENTIAL_COMPONENT_BYTES
            || wire.tracker.len() > MAX_TRACKER_BYTES
        {
            return Err(CredentialUnavailable);
        }

        let mut userpass = String::with_capacity(MAX_BASIC_USERPASS_BYTES);
        userpass.push_str(&wire.username);
        userpass.push(':');
        userpass.push_str(&wire.password);

        let mut authorization = String::with_capacity(MAX_BASIC_AUTHORIZATION_BYTES);
        authorization.push_str("Basic ");
        STANDARD.encode_string(userpass.as_bytes(), &mut authorization);

        Ok(Self {
            username: wire.username,
            tracker: wire.tracker,
            authorization: authorization.into_boxed_str(),
        })
    }

    /// Borrow the validated username.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Borrow the tracker; an empty tracker is valid.
    #[must_use]
    pub fn tracker(&self) -> &str {
        &self.tracker
    }

    /// Borrow the complete Basic Authorization header value.
    #[must_use]
    pub fn authorization(&self) -> &str {
        &self.authorization
    }
}

impl fmt::Debug for BidderCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BidderCredentials([REDACTED])")
    }
}

/// Request-bound reader of one coherent credential rotation.
///
/// Implementations must read username, password and tracker from one atomic
/// bundle, bound provider reads before parsing, and honor the original absolute
/// deadline. References come from admitted settings, not auction parameters.
/// Dropping the returned future must cancel request-bound provider work.
/// Futures need not be `Send` on single-threaded edge runtimes.
#[async_trait(?Send)]
pub trait BidderCredentialSource: Send + Sync {
    /// Load one validated bundle without exposing provider or secret diagnostics.
    ///
    /// # Errors
    /// Returns [`CredentialUnavailable`] when the bundle cannot be supplied.
    async fn load(
        &self,
        reference: &str,
        deadline: MonotonicInstant,
    ) -> Result<BidderCredentials, CredentialUnavailable>;
}

fn preflight(input: &[u8]) -> Result<(), CredentialUnavailable> {
    if input.len() > MAX_BIDDER_CREDENTIAL_BYTES
        || input.iter().find(|byte| !byte.is_ascii_whitespace()) != Some(&b'{')
    {
        // Serde struct derives also accept positional arrays; this wire format
        // admits only objects, before any typed credential allocations.
        return Err(CredentialUnavailable);
    }
    count_json_nodes(input, MAX_BIDDER_CREDENTIAL_NODES).map_err(|_error| CredentialUnavailable)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BIDDER_CREDENTIAL_PARSE_PEAK_BYTES, BIDDER_CREDENTIAL_WORKING_BYTES,
        BidderCredentialSource, BidderCredentials, CredentialUnavailable,
        MAX_BIDDER_CREDENTIAL_BYTES, MAX_BIDDER_CREDENTIAL_NODES, preflight,
    };
    use crate::{MonotonicInstant, count_json_nodes};
    use async_trait::async_trait;
    use futures::executor::block_on;
    use futures::future::ready;
    use http::HeaderValue;
    use serde_json::json;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn bundle(username: &str, password: &str, tracker: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema_version": 1_u8,
            "username": username,
            "password": password,
            "tracker": tracker,
        }))
        .unwrap()
    }

    #[test]
    fn credential_parse_and_working_envelopes_match_independent_source_terms() {
        let diagnostic = 6_u64.checked_mul(4096).unwrap().checked_add(128).unwrap();
        let heap = 3_u64
            .checked_mul(4096)
            .unwrap()
            .checked_add(3_u64.checked_mul(diagnostic).unwrap())
            .unwrap()
            .checked_add(128)
            .unwrap();
        assert_eq!(heap, 86_528);
        assert_eq!(BIDDER_CREDENTIAL_PARSE_PEAK_BYTES, 86_528);
        assert_eq!(BIDDER_CREDENTIAL_WORKING_BYTES, 128 * 1024);
        let read_and_parse = heap
            .checked_add(8192)
            .unwrap()
            .checked_add(1024)
            .unwrap()
            .checked_add(128)
            .unwrap()
            .checked_add(4096)
            .unwrap();
        assert_eq!(read_and_parse, 99_968);
        assert!(
            BIDDER_CREDENTIAL_WORKING_BYTES >= read_and_parse.checked_mul(6).unwrap().div_ceil(5)
        );
    }

    fn refused(input: &[u8]) -> CredentialUnavailable {
        let error = BidderCredentials::parse(input).unwrap_err();
        assert_eq!(error, CredentialUnavailable);
        assert_eq!(error.to_string(), "bidder credentials unavailable");
        assert_eq!(format!("{error:?}"), "CredentialUnavailable");
        error
    }

    #[test]
    fn parses_known_basic_authorization_and_borrowed_components() {
        let credentials =
            BidderCredentials::parse(&bundle("Aladdin", "open sesame", "tracking-one"))
                .expect("a complete version-1 bundle must parse");
        let username: &str = credentials.username();
        let tracker: &str = credentials.tracker();
        let authorization: &str = credentials.authorization();
        assert_eq!(username, "Aladdin");
        assert_eq!(tracker, "tracking-one");
        assert_eq!(authorization, "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==");
    }

    #[test]
    fn accepts_empty_tracker_and_colons_only_in_password() {
        let credentials = BidderCredentials::parse(&bundle("u", "p:p", "")).unwrap();
        assert_eq!(credentials.tracker(), "");
        assert_eq!(credentials.authorization(), "Basic dTpwOnA=");
        refused(&bundle("u:u", "p", ""));
    }

    #[test]
    fn rejects_each_empty_required_secret_independently() {
        refused(&bundle("", "p", "t"));
        refused(&bundle("u", "", "t"));
    }

    #[test]
    fn byte_cap_accepts_exactly_4096_and_rejects_4097_with_identical_nodes() {
        assert_eq!(MAX_BIDDER_CREDENTIAL_BYTES, 4096);
        let mut input = bundle("u", "p", "t");
        input.resize(4096, b' ');
        assert_eq!(input.len(), 4096);
        assert_eq!(count_json_nodes(&input, 16).unwrap(), 9);
        preflight(&input).unwrap();
        assert_eq!(
            BidderCredentials::parse(&input).unwrap().authorization(),
            "Basic dTpw"
        );
        input.push(b' ');
        assert_eq!(input.len(), 4097);
        assert_eq!(count_json_nodes(&input, 16).unwrap(), 9);
        assert_eq!(preflight(&input), Err(CredentialUnavailable));
        refused(&input);
    }

    #[test]
    fn node_cap_accepts_16_and_rejects_17_below_identical_byte_caps() {
        assert_eq!(MAX_BIDDER_CREDENTIAL_NODES, 16);
        for (children, nodes, admitted) in [(5, 16, true), (6, 17, false)] {
            // The extra field isolates preflight from strict DTO validation:
            // root + four key/value pairs + extra key/array + children.
            let mut input = serde_json::to_vec(&json!({
                "schema_version": 1_u8,
                "username": "u",
                "password": "p",
                "tracker": "t",
                "extra": vec![0_u8; children],
            }))
            .unwrap();
            input.resize(512, b' ');
            assert_eq!(input.len(), 512);
            assert_eq!(count_json_nodes(&input, 32).unwrap(), nodes);
            assert_eq!(preflight(&input).is_ok(), admitted);
            refused(&input);
        }
    }

    #[test]
    fn component_caps_vary_each_field_independently() {
        for (field, limit) in [("username", 256_usize), ("password", 256), ("tracker", 128)] {
            for (length, admitted) in [(limit, true), (limit.checked_add(1).unwrap(), false)] {
                let mut document = json!({
                    "schema_version": 1_u8,
                    "username": "u",
                    "password": "p",
                    "tracker": "t",
                });
                document[field] = json!("x".repeat(length));
                let input = serde_json::to_vec(&document).unwrap();
                assert!(input.len() < 4096);
                assert_eq!(count_json_nodes(&input, 16).unwrap(), 9);
                assert_eq!(
                    BidderCredentials::parse(&input).is_ok(),
                    admitted,
                    "{field}"
                );
            }
        }
    }

    #[test]
    fn component_caps_count_utf8_bytes_not_characters() {
        for (field, limit) in [("username", 256_usize), ("password", 256), ("tracker", 128)] {
            let at_limit = "\u{e9}".repeat(limit.checked_div(2).unwrap());
            assert_eq!(at_limit.len(), limit);
            let mut over_limit = at_limit.clone();
            over_limit.push('x');
            for (text, admitted) in [(at_limit, true), (over_limit, false)] {
                let mut document = json!({
                    "schema_version": 1_u8,
                    "username": "u",
                    "password": "p",
                    "tracker": "t",
                });
                document[field] = json!(text);
                let input = serde_json::to_vec(&document).unwrap();
                assert!(input.len() < 4096);
                assert_eq!(
                    BidderCredentials::parse(&input).is_ok(),
                    admitted,
                    "{field}"
                );
            }
        }
    }

    #[test]
    fn simultaneous_maximum_fields_bound_authorization_to_690_bytes() {
        let username = "u".repeat(256);
        let password = "p".repeat(256);
        let tracker = "t".repeat(128);
        let credentials =
            BidderCredentials::parse(&bundle(&username, &password, &tracker)).unwrap();
        assert_eq!(credentials.username(), username);
        assert_eq!(credentials.tracker(), tracker);
        // 513 decoded bytes, 684 base64 bytes, and the six-byte Basic prefix.
        assert_eq!(credentials.authorization().len(), 690);
        assert!(credentials.authorization().starts_with("Basic "));
        HeaderValue::from_str(credentials.authorization()).unwrap();
    }

    #[test]
    fn requires_every_field_including_tracker_and_version() {
        for field in ["schema_version", "username", "password", "tracker"] {
            let mut document = json!({
                "schema_version": 1_u8,
                "username": "u",
                "password": "p",
                "tracker": "t",
            });
            document.as_object_mut().unwrap().remove(field);
            refused(&serde_json::to_vec(&document).unwrap());
        }
    }

    #[test]
    fn typed_fields_reject_null_and_nonstring_credentials() {
        for field in ["username", "password", "tracker"] {
            for value in [json!(null), json!(true), json!(1_u8), json!([]), json!({})] {
                let mut document = json!({
                    "schema_version": 1_u8,
                    "username": "u",
                    "password": "p",
                    "tracker": "t",
                });
                document[field] = value;
                refused(&serde_json::to_vec(&document).unwrap());
            }
        }
    }

    #[test]
    fn version_guard_changes_only_version_in_an_otherwise_valid_bundle() {
        for version in [
            json!(0_u8),
            json!(2_u8),
            json!(-1_i8),
            json!(u64::MAX),
            json!(1.0_f64),
            json!("1"),
            json!(true),
            json!(null),
            json!([]),
            json!({}),
        ] {
            let mut document = json!({
                "schema_version": 1_u8,
                "username": "u",
                "password": "p",
                "tracker": "t",
            });
            BidderCredentials::parse(&serde_json::to_vec(&document).unwrap()).unwrap();
            document["schema_version"] = version;
            refused(&serde_json::to_vec(&document).unwrap());
        }
    }

    #[test]
    fn bare_version_does_not_bypass_required_typed_bundle() {
        refused(&serde_json::to_vec(&json!(1_u8)).unwrap());
        refused(&serde_json::to_vec(&json!({"schema_version": 1_u8})).unwrap());
    }

    #[test]
    fn version_guard_rejects_noninteger_lexemes_without_float_coercion() {
        // Lexical exemption: json! cannot preserve exponent spelling or an
        // integer outside u64; replacements keep the rest of the bundle valid.
        let baseline = String::from_utf8(bundle("u", "p", "t")).unwrap();
        assert!(baseline.contains("\"schema_version\":1"));
        for version in ["1e0", "1.00", "-0", "18446744073709551616", "1e400"] {
            let input = baseline.replace(
                "\"schema_version\":1",
                &format!("\"schema_version\":{version}"),
            );
            refused(input.as_bytes());
        }
    }

    #[test]
    fn strict_dto_rejects_unknown_fields_even_when_empty() {
        for field in ["extra", "authorization", "version", "secret_slot_canary"] {
            let mut document = json!({
                "schema_version": 1_u8,
                "username": "u",
                "password": "p",
                "tracker": "t",
            });
            document[field] = json!("");
            refused(&serde_json::to_vec(&document).unwrap());
        }
    }

    #[test]
    fn strict_dto_rejects_duplicate_fields_including_escaped_names() {
        // Lexical exemption: json! normalizes duplicate keys, including keys
        // whose escaped spelling decodes to the same known field name.
        for input in [
            r#"{"schema_version":1,"schema_version":1,"username":"u","password":"p","tracker":"t"}"#,
            r#"{"schema_version":1,"username":"u","username":"other","password":"p","tracker":"t"}"#,
            r#"{"schema_version":1,"username":"u","password":"p","password":"other","tracker":"t"}"#,
            r#"{"schema_version":1,"username":"u","password":"p","tracker":"t","tracker":"other"}"#,
            r#"{"schema_version":1,"username":"u","\u0075sername":"other","password":"p","tracker":"t"}"#,
        ] {
            preflight(input.as_bytes()).unwrap();
            refused(input.as_bytes());
        }
    }

    #[test]
    fn rejects_nonobject_roots_and_positional_struct_sequences() {
        for document in [
            json!(null),
            json!(true),
            json!("u:p"),
            json!([]),
            json!([1_u8, "u", "p", "t"]),
            json!({}),
        ] {
            refused(&serde_json::to_vec(&document).unwrap());
        }
    }

    #[test]
    fn rejects_malformed_json_invalid_utf8_and_trailing_documents() {
        // Lexical exemption: missing delimiters, trailing commas, and invalid
        // UTF-8 cannot be produced by json! and must reach the byte parser.
        for input in [
            b"".as_slice(),
            b" ",
            br#"{"schema_version":1,"username":"u","password":"p","tracker":"t""#,
            br#"{"schema_version":1,"username":"u","password":"p","tracker":"t",}"#,
            b"\xff",
        ] {
            refused(input);
        }
        for suffix in [b" true".as_slice(), b" {}", b"\xff"] {
            let mut input = bundle("u", "p", "t");
            input.extend_from_slice(suffix);
            refused(&input);
        }
    }

    #[test]
    fn debug_redacts_every_retained_slot_and_encoded_authorization() {
        let credentials = BidderCredentials::parse(&bundle(
            "username_slot_canary",
            "password_slot_canary",
            "tracker_slot_canary",
        ))
        .unwrap();
        for output in [format!("{credentials:?}"), format!("{credentials:#?}")] {
            assert_eq!(output, "BidderCredentials([REDACTED])");
            for secret in [
                credentials.username(),
                "password_slot_canary",
                credentials.tracker(),
                credentials.authorization(),
            ] {
                assert!(!output.contains(secret));
            }
        }
    }

    #[test]
    fn validation_errors_redact_canaries_in_every_secret_slot() {
        for field in ["username", "password", "tracker"] {
            let mut document = json!({
                "schema_version": 1_u8,
                "username": "u",
                "password": "p",
                "tracker": "t",
            });
            // Type errors can quote the supplied value; length errors can
            // accidentally retain it even after typed deserialization succeeds.
            for invalid in [
                json!(["secret_slot_canary"]),
                json!("secret_slot_canary".repeat(32)),
            ] {
                document[field] = invalid;
                let error = refused(&serde_json::to_vec(&document).unwrap());
                assert!(!error.to_string().contains("secret_slot_canary"));
                assert!(!format!("{error:?}").contains("secret_slot_canary"));
            }
        }
    }

    struct RotatingSource {
        reads: AtomicUsize,
        deadline: MonotonicInstant,
    }

    #[async_trait(?Send)]
    impl BidderCredentialSource for RotatingSource {
        async fn load(
            &self,
            reference: &str,
            deadline: MonotonicInstant,
        ) -> Result<BidderCredentials, CredentialUnavailable> {
            assert_eq!(reference, "operator/reference");
            assert_eq!(deadline, self.deadline);
            let input = match self.reads.fetch_add(1, Ordering::SeqCst) {
                0 => bundle("first", "one", "tracker-one"),
                _ => bundle("second", "two", "tracker-two"),
            };
            BidderCredentials::parse(&input)
        }
    }

    #[test]
    fn dyn_reader_returns_two_coherent_rotations_and_preserves_owned_first() {
        let deadline = MonotonicInstant::now();
        let source = RotatingSource {
            reads: AtomicUsize::new(0),
            deadline,
        };
        let reader: &dyn BidderCredentialSource = &source;
        let first = block_on(reader.load("operator/reference", deadline)).unwrap();
        assert_eq!(source.reads.load(Ordering::SeqCst), 1);
        let second = block_on(reader.load("operator/reference", deadline)).unwrap();
        assert_eq!(source.reads.load(Ordering::SeqCst), 2);
        assert_eq!(first.username(), "first");
        assert_eq!(first.tracker(), "tracker-one");
        assert_eq!(first.authorization(), "Basic Zmlyc3Q6b25l");
        assert_eq!(second.username(), "second");
        assert_eq!(second.tracker(), "tracker-two");
        assert_eq!(second.authorization(), "Basic c2Vjb25kOnR3bw==");
    }

    #[test]
    fn dyn_reader_future_need_not_be_send() {
        struct LocalSource;

        #[async_trait(?Send)]
        impl BidderCredentialSource for LocalSource {
            async fn load(
                &self,
                _reference: &str,
                _deadline: MonotonicInstant,
            ) -> Result<BidderCredentials, CredentialUnavailable> {
                let local = Rc::new(bundle("u", "p", "t"));
                ready(()).await;
                BidderCredentials::parse(local.as_slice())
            }
        }

        let reader: &dyn BidderCredentialSource = &LocalSource;
        let credentials =
            block_on(reader.load("operator/reference", MonotonicInstant::now())).unwrap();
        assert_eq!(credentials.authorization(), "Basic dTpw");
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::{
        BIDDER_CREDENTIAL_PARSE_PEAK_BYTES, BidderCredentials, CredentialUnavailable, preflight,
    };
    use crate::alloc_track::measure_on_this_thread;
    use crate::count_json_nodes;
    use serde_json::{Value, json};
    use std::hint::black_box;
    use std::io::{Write as _, stdout};
    use std::mem::size_of;
    use std::thread;

    const WIRE_BYTES: usize = 4096;

    // Analytical parse-heap envelope for independent review, not a serving
    // charge or deployment qualification. Source pins: serde_core 1.0.228
    // de/{mod,impls}.rs; serde_json 1.0.150 {de,read,error}.rs; Rust 1.98.1
    // commit 48a229ceaefd4985c50990b14116b6d856af0985, alloc/{string,raw_vec}
    // and core/fmt. Re-audit after changing those pins, Wire or source limits.
    //
    // Let W = 4096 source bytes. Admitted and rejected typed string fields can
    // together own D <= W decoded bytes before field-length checks. String's
    // Deserialize uses to_owned, then Box<str> takes that exact-sized buffer.
    // The borrowed input is not part of D or these allocator windows.
    //
    // A Rust Debug string needs at most six output bytes per decoded UTF-8
    // byte, plus two quotes: one-byte characters escape in at most six bytes;
    // two/three/four-byte characters need at most seven/eight/ten bytes.
    // DEL attains the sixfold case without requiring JSON source escaping.
    // The longest fixed Serde context here is the unknown-field diagnostic's
    // four-name expected list (85 bytes without the unknown name). Bound all
    // fixed context, including quotes and numeric type diagnostics, by 128.
    // Hence one complete diagnostic's message length M <= 6W + 128 = 24704.
    // Unknown names use Display, not Debug, but satisfy the same envelope.
    //
    // Rust String::to_string starts empty. RawVec growth is max(2*old, needed,
    // minimum_capacity). Thus a message of length <= M has capacity <= 2M,
    // with old+new <= 3M during growth or into_boxed_str shrinking. Serde's
    // string-unescape scratch has length <= W, capacity <= 2W and old+new <=
    // 3W during growth. The preflight IgnoredAny's nesting scratch also fits W.
    // Scratch growth precedes diagnostic formatting; only stable scratch can
    // coexist with the diagnostic's growth/boxing overlap.
    //
    // In the locked ErrorImpl, ErrorCode is Message(Box<str>), Io(io::Error)
    // or a unit variant, followed by two usize coordinates. Allow E = 64 per
    // error box on the supported 32/64-bit ABIs, including enum tag/alignment.
    // fix_position can overlap old/new ErrorImpl boxes while moving, not
    // copying, the message; two E terms cover that reboxing.
    //
    // Reject peak <= max(D + 3W + 2E, D + 2W + 3M + 2E)
    //             <= 3W + 3M + 2E = 86528 requested heap bytes.
    // Valid encoding separately fits 640 DTO bytes + 513 userpass bytes +
    // 690 old and 690 new authorization bytes = 2533; it cannot overlap parse
    // scratch or a diagnostic. Retained credential payload is at most 1074.
    //
    // Excluded: caller's <=4096-byte input, base64's 1024-byte stack buffer,
    // bounded capture/store-name ownership, already-funded inline request/
    // future state, allocator metadata/rounding, stack frames and process RSS.
    // Measurements below do not select the charge. They guard this independently
    // derived parse envelope and exact retained payloads, not provider RSS.
    const ATOMS: [(&str, &str); 6] = [
        ("ascii", "x"),
        ("quote", "\""),
        ("backslash", "\\"),
        ("nul", "\u{0}"),
        ("unicode_control", "\u{80}"),
        ("unicode_format", "\u{200b}"),
    ];

    fn document(maximum_fields: bool) -> Value {
        let (username, password, tracker) = if maximum_fields {
            ("u".repeat(256), "p".repeat(256), "t".repeat(128))
        } else {
            ("u".to_owned(), "p".to_owned(), "t".to_owned())
        };
        json!({
            "schema_version": 1_u8,
            "username": username,
            "password": password,
            "tracker": tracker,
        })
    }

    fn padded(mut input: Vec<u8>, length: usize) -> Vec<u8> {
        assert!(input.len() <= length);
        input.resize(length, b' ');
        input
    }

    fn encoded_atom_bytes(atom: &str) -> usize {
        // The surrounding quotes are not part of the repeated source token.
        serde_json::to_vec(&json!(atom))
            .unwrap()
            .len()
            .checked_sub(2)
            .unwrap()
    }

    fn repetitions_for(overhead: usize, atom: &str) -> usize {
        WIRE_BYTES
            .checked_sub(overhead)
            .unwrap()
            .checked_div(encoded_atom_bytes(atom))
            .unwrap()
    }

    fn with_field(input: &[u8], key: &str, value: &Value, first: bool) -> Vec<u8> {
        // Lexical exemption: source ordering and duplicate keys are measured
        // here; rebuilding the entire object with json! would normalize both.
        // Individual keys and values still use the ordinary JSON serializer.
        let encoded_key = serde_json::to_vec(key).unwrap();
        let encoded_value = serde_json::to_vec(value).unwrap();
        let mut output = Vec::new();
        if first {
            output.push(b'{');
            output.extend_from_slice(&encoded_key);
            output.push(b':');
            output.extend_from_slice(&encoded_value);
            output.push(b',');
            output.extend_from_slice(&input[1..]);
        } else {
            let closing_brace = input.len().checked_sub(1).unwrap();
            output.extend_from_slice(&input[..closing_brace]);
            output.push(b',');
            output.extend_from_slice(&encoded_key);
            output.push(b':');
            output.extend_from_slice(&encoded_value);
            output.push(b'}');
        }
        output
    }

    fn longest_field(field: &str, atom: &str, version_last: bool) -> Vec<u8> {
        let render = |text: &str| {
            let mut document = document(version_last);
            if version_last {
                document.as_object_mut().unwrap().remove("schema_version");
                with_field(
                    &serde_json::to_vec(&document).unwrap(),
                    "schema_version",
                    &json!(text),
                    false,
                )
            } else {
                document[field] = json!(text);
                serde_json::to_vec(&document).unwrap()
            }
        };
        let repetitions = repetitions_for(render("").len(), atom);
        padded(render(&atom.repeat(repetitions)), WIRE_BYTES)
    }

    fn longest_key(atom: &str, first: bool, maximum_fields: bool) -> Vec<u8> {
        let known = serde_json::to_vec(&document(maximum_fields)).unwrap();
        let overhead = with_field(&known, "", &json!(0_u8), first).len();
        let repetitions = repetitions_for(overhead, atom);
        padded(
            with_field(&known, &atom.repeat(repetitions), &json!(0_u8), first),
            WIRE_BYTES,
        )
    }

    fn observe(case: &str, input: Vec<u8>, accepted: bool) {
        // Each case's first parse runs on a fresh thread. Fixture construction,
        // thread setup, the borrowed input allocation and reporting are outside
        // the enabled window. No measured allocation crosses thread ownership.
        let (stats, payload_bytes, input_bytes, nodes) = thread::spawn(move || {
            let (result, stats) =
                measure_on_this_thread(|| BidderCredentials::parse(black_box(input.as_slice())))
                    .expect("the cold parse must have a valid same-thread measurement window");
            assert_eq!(result.is_ok(), accepted);
            let payload_bytes = match &result {
                Ok(credentials) => credentials
                    .username()
                    .len()
                    .checked_add(credentials.tracker().len())
                    .and_then(|bytes| bytes.checked_add(credentials.authorization().len()))
                    .unwrap(),
                Err(error) => {
                    assert_eq!(*error, CredentialUnavailable);
                    assert_eq!(error.to_string(), "bidder credentials unavailable");
                    assert_eq!(format!("{error:?}"), "CredentialUnavailable");
                    0
                }
            };
            assert_eq!(stats.retained_bytes, u64::try_from(payload_bytes).unwrap());
            assert!(stats.peak_bytes >= stats.retained_bytes);
            assert!(stats.peak_bytes <= BIDDER_CREDENTIAL_PARSE_PEAK_BYTES);
            assert!(payload_bytes <= 1074);
            let nodes = count_json_nodes(&input, u32::MAX).ok();
            (stats, payload_bytes, input.len(), nodes)
        })
        .join()
        .unwrap();

        // The charge is source-derived, not fitted to these observations.
        // A passing heap window is not a provider memory qualification.
        let report = json!({
            "case": case,
            "scope": "cold_parse_heap_only",
            "input_bytes": input_bytes,
            "nodes": nodes,
            "accepted": accepted,
            "peak_bytes": stats.peak_bytes,
            "retained_bytes": stats.retained_bytes,
            "credential_payload_bytes": payload_bytes,
            "credential_inline_bytes": size_of::<BidderCredentials>(),
            "wire_dto_inline_bytes": size_of::<super::Wire>(),
        });
        let mut output = stdout().lock();
        output.write_all(b"\n").unwrap();
        serde_json::to_writer(&mut output, &report).unwrap();
        output.write_all(b"\n").unwrap();
    }

    #[test]
    fn allocation_valid_maximum_fields_and_independent_wire_boundaries() {
        observe(
            "valid/minimum",
            serde_json::to_vec(&document(false)).unwrap(),
            true,
        );
        observe(
            "valid/maximum_fields",
            serde_json::to_vec(&document(true)).unwrap(),
            true,
        );
        observe(
            "valid/maximum_escaped_fields",
            serde_json::to_vec(&json!({
                "schema_version": 1_u8,
                "username": "\u{0}".repeat(256),
                "password": "\u{0}".repeat(256),
                "tracker": "\u{0}".repeat(128),
            }))
            .unwrap(),
            true,
        );
        observe(
            "valid/maximum_utf8_fields",
            serde_json::to_vec(&json!({
                "schema_version": 1_u8,
                "username": "\u{e9}".repeat(128),
                "password": "\u{e9}".repeat(128),
                "tracker": "\u{e9}".repeat(64),
            }))
            .unwrap(),
            true,
        );
        for length in [4095, 4096, 4097] {
            observe(
                &format!("wire/minimum_fields/{length}"),
                padded(serde_json::to_vec(&document(false)).unwrap(), length),
                length <= 4096,
            );
        }
        observe(
            "wire/maximum_fields/4096",
            padded(serde_json::to_vec(&document(true)).unwrap(), 4096),
            true,
        );
    }

    #[test]
    fn allocation_long_string_fields_and_version_diagnostic_matrix() {
        for (kind, atom) in ATOMS {
            for field in ["schema_version", "username", "password", "tracker"] {
                observe(
                    &format!("long_field/{field}/{kind}"),
                    longest_field(field, atom, false),
                    false,
                );
            }
            observe(
                &format!("long_field/schema_version_last_after_maximum_fields/{kind}"),
                longest_field("schema_version", atom, true),
                false,
            );
        }
    }

    #[test]
    fn allocation_long_unknown_key_diagnostic_matrix() {
        for (kind, atom) in ATOMS {
            for first in [false, true] {
                for maximum_fields in [false, true] {
                    observe(
                        &format!("long_key/{kind}/first={first}/maximum_fields={maximum_fields}"),
                        longest_key(atom, first, maximum_fields),
                        false,
                    );
                }
            }
        }
    }

    #[test]
    fn allocation_ascii_del_diagnostic_cases() {
        // Source-audit addition: DEL is one UTF-8/source byte but Debug renders
        // it as six ASCII bytes. Unlike U+0000, JSON need not escape it first.
        let atom = "\u{7f}";
        observe(
            "ascii_del/schema_version",
            longest_field("schema_version", atom, false),
            false,
        );
        observe(
            "ascii_del/schema_version_last_after_maximum_fields",
            longest_field("schema_version", atom, true),
            false,
        );
        for first in [false, true] {
            for maximum_fields in [false, true] {
                observe(
                    &format!("ascii_del/key/first={first}/maximum_fields={maximum_fields}"),
                    longest_key(atom, first, maximum_fields),
                    false,
                );
            }
        }
    }

    #[test]
    fn allocation_malformed_long_strings_keys_and_utf8_matrix() {
        for (kind, atom) in ATOMS {
            for key in [false, true] {
                // Lexical exemption: remove the closing quote, leaving an
                // unterminated long escaped string or key. json! cannot express it.
                let mut input = if key {
                    b"{".to_vec()
                } else {
                    b"{\"schema_version\":1,\"username\":".to_vec()
                };
                let overhead = input.len().checked_add(2).unwrap();
                let repetitions = repetitions_for(overhead, atom);
                input.extend_from_slice(
                    &serde_json::to_vec(&json!(atom.repeat(repetitions))).unwrap(),
                );
                input.pop();
                observe(
                    &format!("malformed/{kind}/key={key}"),
                    padded(input, WIRE_BYTES),
                    false,
                );
            }
        }
        // Lexical exemption: a byte inside a JSON string is invalid UTF-8.
        let mut invalid_utf8 = b"{\"schema_version\":\"".to_vec();
        invalid_utf8.push(0xff);
        invalid_utf8.extend_from_slice(b"\"}");
        observe(
            "malformed/utf8_string",
            padded(invalid_utf8, WIRE_BYTES),
            false,
        );
    }

    #[test]
    fn allocation_wrong_typed_fields_matrix() {
        for field in ["schema_version", "username", "password", "tracker"] {
            for (kind, value) in [
                ("null", json!(null)),
                ("boolean", json!(true)),
                ("fraction", json!(1.5_f64)),
                ("large_integer", json!(u64::MAX)),
                ("array", json!(["\u{80}".repeat(1900)])),
                ("object", json!({"inner": "\u{80}".repeat(1900)})),
            ] {
                let mut document = document(false);
                document[field] = value;
                observe(
                    &format!("wrong_type/{field}/{kind}"),
                    padded(serde_json::to_vec(&document).unwrap(), WIRE_BYTES),
                    false,
                );
            }
        }
    }

    #[test]
    fn allocation_duplicate_fields_and_independent_node_limits_matrix() {
        for field in ["schema_version", "username", "password", "tracker"] {
            let value = if field == "schema_version" {
                json!(1_u8)
            } else {
                json!("duplicate_canary")
            };
            observe(
                &format!("duplicate/{field}"),
                padded(
                    with_field(
                        &serde_json::to_vec(&document(true)).unwrap(),
                        field,
                        &value,
                        false,
                    ),
                    WIRE_BYTES,
                ),
                false,
            );
        }
        // Lexical exemption: the escaped key must collide with the decoded
        // username field, which a Value tree would normalize into one entry.
        let duplicate = with_field(
            &serde_json::to_vec(&document(true)).unwrap(),
            "username",
            &json!("duplicate_canary"),
            false,
        );
        let escaped = String::from_utf8(duplicate).unwrap().replace(
            "\"username\":\"duplicate_canary\"",
            "\"\\u0075sername\":\"duplicate_canary\"",
        );
        observe(
            "duplicate/escaped_username",
            padded(escaped.into_bytes(), WIRE_BYTES),
            false,
        );

        for nodes in [15_usize, 16, 17, 63] {
            for nested in [false, true] {
                let descendants = nodes.checked_sub(11).unwrap();
                let mut extra = json!(0_u8);
                if nested {
                    for _ in 0..descendants {
                        extra = json!([extra]);
                    }
                } else {
                    extra = json!(vec![0_u8; descendants]);
                }
                let mut document = document(false);
                document["extra"] = extra;
                let input = padded(serde_json::to_vec(&document).unwrap(), WIRE_BYTES);
                assert_eq!(
                    count_json_nodes(&input, 64).unwrap(),
                    u32::try_from(nodes).unwrap()
                );
                assert_eq!(preflight(&input).is_ok(), nodes <= 16);
                observe(&format!("node_limit/{nodes}/nested={nested}"), input, false);
            }
        }
    }
}
