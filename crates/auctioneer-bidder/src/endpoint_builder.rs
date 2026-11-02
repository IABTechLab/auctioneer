//! `EndpointBuilder` — macro percent-encoding, path-traversal rejection,
//! `host_allowlist` lookup.

use crate::BidderRequestInfo;
use crate::meta::{META_BIDDER_REGION_BYTES, OutboundRequestMetaBuilder, SinkAbort, UrlBuffer};
use auctioneer_config::types::BidderInfo;
use percent_encoding::{AsciiSet, CONTROLS, percent_decode, utf8_percent_encode};
use std::collections::BTreeMap;
use std::fmt;
use url::form_urlencoded::byte_serialize;

/// Encode every byte that isn't an RFC 3986 unreserved character
/// (`A-Z a-z 0-9 - . _ ~`). This covers what's safe to drop verbatim
/// into a URL path or query segment without further escaping.
/// `NON_ALPHANUMERIC` would also encode `-` / `.` / `_` / `~`, which
/// produces URLs like `acct%2D42` for a plain `acct-42` input — wrong
/// per the `EndpointBuilder` test contract.
const MACRO_VALUE_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

#[derive(Debug, PartialEq, Eq)]
pub enum EndpointBuilderError {
    UnknownMacro(String),
    PathTraversal(String),
    HostNotInAllowlist(String),
    MissingMacro(String),
    MalformedTemplate(String),
    OutputTooLarge,
    /// Refused before an oversize value is decoded, encoded, or copied.
    MacroValueOverCap {
        name: String,
        len: usize,
        limit: usize,
    },
}

enum StringList<'a> {
    Owned(&'a [String]),
    Borrowed(&'a [&'a str]),
}

impl StringList<'_> {
    fn contains(&self, value: &str) -> bool {
        match self {
            Self::Owned(items) => items.iter().any(|item| item == value),
            Self::Borrowed(items) => items.contains(&value),
        }
    }
}

pub struct EndpointBuilder<'info> {
    template: &'info str,
    host_allowlist: Option<StringList<'info>>,
    /// The codegen-emitted macro allow-list (`BidderInfo.endpoint_macros`,
    /// populated by `build_support`'s `extract_endpoint_macros` scanner).
    /// `set_macro` validates `name` against this set so a typo or a
    /// new placeholder added to the template only by a bidder
    /// (without updating bidder-info) is rejected at runtime.
    allowed_macros: StringList<'info>,
    /// Owned values for the convenience API only. Production borrows a
    /// fixed-capacity slice and encodes directly while scanning the template.
    values: BTreeMap<String, String>,
    borrowed_values: &'info [(&'info str, &'info str)],
    host_selected: Option<&'info str>,
}

impl<'info> EndpointBuilder<'info> {
    /// Construct a builder over a bidder's template.
    ///
    /// # Errors
    /// Returns no errors today; the `Result` shape is the M3+-stable
    /// signature for when codegen-time template validation moves to a
    /// runtime parser.
    pub fn new(info: &'info BidderInfo) -> Result<Self, EndpointBuilderError> {
        Ok(Self {
            template: &info.endpoint,
            host_allowlist: info.host_allowlist.as_deref().map(StringList::Owned),
            allowed_macros: StringList::Owned(&info.endpoint_macros),
            values: BTreeMap::new(),
            borrowed_values: &[],
            host_selected: None,
        })
    }

    /// Production view: the macro values and configuration are borrowed.
    /// No metadata is allocated until the sink invokes its build closure.
    #[must_use]
    pub fn from_request(
        info: &'info BidderRequestInfo<'_>,
        values: &'info [(&'info str, &'info str)],
    ) -> Self {
        Self {
            template: info.endpoint_template,
            host_allowlist: info.host_allowlist.map(StringList::Borrowed),
            allowed_macros: StringList::Borrowed(info.endpoint_macros),
            values: BTreeMap::new(),
            borrowed_values: values,
            host_selected: None,
        }
    }

    /// Set the value for one of the bidder's declared endpoint macros.
    ///
    /// # Errors
    /// - `UnknownMacro` if `name` is not in the codegen-emitted
    ///   `endpoint_macros` allow-list, or refers to the authority
    ///   token `Host` (use `select_host` instead).
    /// - `PathTraversal` if the decoded value contains `..` / `.` path
    ///   segments or a NUL byte.
    pub fn set_macro(&mut self, name: &str, value: &str) -> Result<(), EndpointBuilderError> {
        // Validate against the codegen allow-list (info.endpoint_macros)
        // — NOT just "does the placeholder appear in the template."
        // `Host` is the special authority macro and is handled
        // exclusively via `select_host`; it MUST NOT be settable here.
        if name == "Host" {
            return Err(EndpointBuilderError::UnknownMacro(
                "Host is the authority token; use select_host instead".into(),
            ));
        }
        if !self.allowed_macros.contains(name) {
            return Err(EndpointBuilderError::UnknownMacro(name.into()));
        }
        // Defense in depth: also reject macros that don't appear in
        // the current template (e.g. the bidder declares a macro it
        // doesn't use; the bidder shouldn't be able to set it).
        if !self.template.split("{{.").skip(1).any(|tail| {
            tail.split_once("}}")
                .is_some_and(|(token, _)| token == name)
        }) {
            return Err(EndpointBuilderError::UnknownMacro(name.into()));
        }
        // Refuse before the convenience API copies the value. Production
        // borrows it and streams into the same bounded metadata region.
        if value.len() > META_BIDDER_REGION_BYTES {
            return Err(EndpointBuilderError::MacroValueOverCap {
                name: name.into(),
                len: value.len(),
                limit: META_BIDDER_REGION_BYTES,
            });
        }
        // Percent-decode the input recursively until convergence
        // before the segment scan. Catches double-encoded traversal
        // (`%252e%252e%252f` -> `../`) and encoded slashes (`%2f`).
        validate_value(value)?;
        self.values.insert(name.into(), value.into());
        Ok(())
    }

    /// Choose one of the host-allowlist entries for a `{{.Host}}` template.
    ///
    /// # Errors
    /// - `UnknownMacro("Host")` if the bidder is fixed-host (no allowlist).
    /// - `HostNotInAllowlist` if `host` is not a member.
    pub fn select_host(&mut self, host: &'info str) -> Result<(), EndpointBuilderError> {
        let allow = self
            .host_allowlist
            .as_ref()
            .ok_or_else(|| EndpointBuilderError::UnknownMacro("Host".into()))?;
        if !allow.contains(host) {
            return Err(EndpointBuilderError::HostNotInAllowlist(host.into()));
        }
        self.host_selected = Some(host);
        Ok(())
    }

    /// Emit the final URL with every macro substituted.
    ///
    /// # Errors
    /// - `MissingMacro` if the template still references a placeholder
    ///   the caller did not set (including `Host` for a host-varying
    ///   bidder).
    /// - `MalformedTemplate` if `{{.` appears with no matching `}}`.
    pub fn build(self) -> Result<String, EndpointBuilderError> {
        let mut url = String::new();
        self.write_to(&mut url)?;
        Ok(url)
    }

    /// Expand the template directly into a meta builder's byte pool.
    ///
    /// The URL component is written once, into the bidder region, so no
    /// free-standing `String` for the final URL outlives the call. The
    /// template scan, allowlist lookup and traversal checks are the same
    /// ones [`EndpointBuilder::build`] runs; only the output target moves
    /// into the byte pool.
    ///
    /// Callers that need to tell a template failure apart from a byte
    /// pool failure run [`EndpointBuilder::build`] themselves; this seam
    /// speaks the sink's single error language so `?` works inside a
    /// build closure.
    ///
    /// A template failure — an unset macro or a malformed placeholder —
    /// therefore reaches the sink as `MetaOverCap`, which is misleading
    /// about the cause but not about the outcome: both are bidder bugs
    /// that the exchange reports as the same per-bidder code-5 entry,
    /// and the sink's own url-parse check plus
    /// `EndpointRejectReason::TemplateUnresolved` already catch the
    /// surviving-placeholder case with its real name. Distinguishing it
    /// here would mean a `SinkAbort` variant the enumerated
    /// `OutboundBuildError` contract has no home for, so the collapse
    /// stays deliberate rather than accidental.
    ///
    /// # Errors
    /// - [`SinkAbort::MetaOverCap`] when the expanded URL does not fit
    ///   the bidder region, and for a template that failed to expand
    ///   (an unset macro or a malformed placeholder) — in both cases the
    ///   byte pool is left untouched.
    /// - [`SinkAbort::DuplicateMeta`] when the builder already has a URL.
    pub fn expand_into(self, meta: &mut OutboundRequestMetaBuilder<'_>) -> Result<(), SinkAbort> {
        meta.set_url_with(|writer| {
            self.write_to(writer)
                .map_err(|_template_error| SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
                })
        })
    }

    /// Append form-encoded query pairs while expanding into the metadata cap.
    /// Existing query pairs are preserved. No intermediate URL is allocated.
    ///
    /// # Errors
    /// Returns the same bounded metadata errors as `expand_into`. Nonempty
    /// appended pairs reject literal fragments rather than placing query data
    /// after a fragment. Failure rolls back the complete URL component.
    pub fn expand_with_query_into(
        self,
        meta: &mut OutboundRequestMetaBuilder<'_>,
        pairs: &[(&str, &str)],
    ) -> Result<(), SinkAbort> {
        if pairs.is_empty() {
            return self.expand_into(meta);
        }
        meta.set_url_with(|writer| {
            let over_cap = || SinkAbort::MetaOverCap {
                limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
            };
            if self.template.contains('#') {
                return Err(over_cap());
            }
            self.write_to(writer).map_err(|_error| over_cap())?;
            let mut separator = if self.template.contains('?') {
                if self.template.ends_with(['?', '&']) {
                    ""
                } else {
                    "&"
                }
            } else {
                "?"
            };
            for (key, value) in pairs {
                writer.write_str(separator).map_err(|_error| over_cap())?;
                write_query_pair(writer, key, value).map_err(|_error| over_cap())?;
                separator = "&";
            }
            Ok(())
        })
    }

    /// Set one form-encoded query parameter, replacing all its existing values.
    /// Keys are compared after macro expansion and one form-decoding pass.
    /// Unrelated pairs retain their order and original expanded encoding.
    /// The expanded input URL must fit before any edits; replacement does not
    /// make oversized templates admissible. No second URL buffer is allocated.
    ///
    /// # Errors
    /// Returns the same transactional metadata errors as `expand_into`.
    /// Literal fragments are rejected, as with nonempty query appends.
    pub fn expand_with_query_set_into(
        self,
        meta: &mut OutboundRequestMetaBuilder<'_>,
        key: &str,
        value: &str,
    ) -> Result<(), SinkAbort> {
        meta.set_url_buffer_with(|url| {
            let over_cap = || SinkAbort::MetaOverCap {
                limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap_or(u64::MAX),
            };
            if self.template.contains('#') {
                return Err(over_cap());
            }
            self.write_to(url.writer()).map_err(|_error| over_cap())?;
            remove_query_key(url, key)?;
            let expanded = url.as_str()?;
            let separator = if expanded.contains('?') {
                if expanded.ends_with(['?', '&']) {
                    ""
                } else {
                    "&"
                }
            } else {
                "?"
            };
            let writer = url.writer();
            writer.write_str(separator).map_err(|_error| over_cap())?;
            write_query_pair(writer, key, value).map_err(|_error| over_cap())
        })
    }

    fn write_to(&self, writer: &mut dyn fmt::Write) -> Result<(), EndpointBuilderError> {
        let mut tail = self.template;
        while let Some((literal, after_open)) = tail.split_once("{{.") {
            writer
                .write_str(literal)
                .map_err(|_write_error| EndpointBuilderError::OutputTooLarge)?;
            let (name, rest) = after_open.split_once("}}").ok_or_else(|| {
                EndpointBuilderError::MalformedTemplate("unterminated macro".into())
            })?;
            if name == "Host" {
                let host = self
                    .host_selected
                    .ok_or_else(|| EndpointBuilderError::MissingMacro("Host".into()))?;
                writer
                    .write_str(host)
                    .map_err(|_write_error| EndpointBuilderError::OutputTooLarge)?;
            } else {
                if !self.allowed_macros.contains(name) {
                    return Err(EndpointBuilderError::UnknownMacro(
                        "undeclared macro".into(),
                    ));
                }
                let value = self
                    .borrowed_values
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| *value)
                    .or_else(|| self.values.get(name).map(String::as_str))
                    .ok_or_else(|| EndpointBuilderError::MissingMacro(name.into()))?;
                validate_value(value)?;
                write!(
                    writer,
                    "{}",
                    utf8_percent_encode(value, MACRO_VALUE_ENCODE_SET)
                )
                .map_err(|_write_error| EndpointBuilderError::OutputTooLarge)?;
            }
            tail = rest;
        }
        writer
            .write_str(tail)
            .map_err(|_write_error| EndpointBuilderError::OutputTooLarge)
    }
}

fn write_query_pair(writer: &mut dyn fmt::Write, key: &str, value: &str) -> fmt::Result {
    for part in byte_serialize(key.as_bytes()) {
        writer.write_str(part)?;
    }
    writer.write_char('=')?;
    for part in byte_serialize(value.as_bytes()) {
        writer.write_str(part)?;
    }
    Ok(())
}

fn remove_query_key(url: &mut UrlBuffer<'_>, key: &str) -> Result<(), SinkAbort> {
    // Removal shifts and UTF-8 checks are quadratic in URL bytes, but both the
    // iteration count and every shift are capped by the existing metadata pool.
    let Some(query_start) = url.as_str()?.find('?').map(|index| index.saturating_add(1)) else {
        return Ok(());
    };
    let mut cursor = query_start;
    loop {
        let expanded = url.as_str()?;
        let Some(tail) = expanded.get(cursor..).filter(|tail| !tail.is_empty()) else {
            return Ok(());
        };
        let pair_len = tail.find('&').unwrap_or(tail.len());
        let end = cursor.saturating_add(pair_len);
        let pair = tail.get(..pair_len).unwrap_or_default();
        let encoded_key = pair.split_once('=').map_or(pair, |(name, _value)| name);
        let last = end == expanded.len();
        if query_key_matches(encoded_key, key) {
            if last {
                let start = if cursor == query_start {
                    cursor
                } else {
                    cursor.saturating_sub(1)
                };
                return url.remove(start..end);
            }
            url.remove(cursor..end.saturating_add(1))?;
        } else if last {
            return Ok(());
        } else {
            cursor = end.saturating_add(1);
        }
    }
}

fn query_key_matches(encoded: &str, key: &str) -> bool {
    encoded
        .as_bytes()
        .split_inclusive(|byte| *byte == b'+')
        .flat_map(|part| {
            let without_plus = part.strip_suffix(b"+");
            percent_decode(without_plus.unwrap_or(part)).chain(without_plus.map(|_prefix| b' '))
        })
        .eq(key.bytes())
}

/// Percent-decode the input recursively until convergence so chains
/// like `%2525` (encoded `%25`, which decodes to `%`) and double-encoded
/// traversal like `%252e%252e%252f` resolve to `../` before the
/// segment scan. We bound the loop at 8 passes — past that the input
/// is either pathological or fully decoded; either way the scan can
/// run on what we have.
fn validate_value(input: &str) -> Result<(), EndpointBuilderError> {
    if input.len() > META_BIDDER_REGION_BYTES {
        return Err(EndpointBuilderError::OutputTooLarge);
    }
    let mut current = [0_u8; META_BIDDER_REGION_BYTES];
    let mut len = input.len();
    for (slot, byte) in current.iter_mut().zip(input.bytes()) {
        *slot = byte;
    }
    for _ in 0..8_u8 {
        let previous = current;
        let mut written = 0_usize;
        for (slot, byte) in current.iter_mut().zip(percent_encoding::percent_decode(
            previous.get(..len).unwrap_or_default(),
        )) {
            *slot = byte;
            written = written.saturating_add(1);
        }
        if written == len {
            break;
        }
        len = written;
    }
    let decoded = current.get(..len).unwrap_or_default();
    if decoded.contains(&0)
        || decoded
            .split(|byte| matches!(*byte, b'/' | b'\\'))
            .any(|part| part == b"." || part == b"..")
    {
        return Err(EndpointBuilderError::PathTraversal(
            "invalid path segment".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{EndpointBuilder, EndpointBuilderError};
    use crate::{
        META_BIDDER_REGION_BYTES, META_BYTES_CAPACITY, OutboundRequestMetaBuilder, SinkAbort,
    };
    use auctioneer_config::types::BidderInfo;

    fn typed_info<'a>(endpoint: &'a str, macros: &'a [&'a str]) -> crate::BidderRequestInfo<'a> {
        crate::BidderRequestInfo::new(
            crate::BidderName::new("fake_requested_alias"),
            crate::BidderName::new("fake_base"),
            "account",
            None,
            endpoint,
            macros,
            None,
            None,
            crate::EntryPoint::Auction,
            &[],
        )
    }

    #[test]
    fn borrowed_pairs_encode_values_through_the_existing_capped_builder() {
        let request_info = typed_info("https://bidder.example.test/{{.RouteID}}", &["RouteID"]);
        let pairs = [("RouteID", "a b/&?=#+%")];
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
        meta.set_method("POST").unwrap();
        EndpointBuilder::from_request(&request_info, &pairs)
            .expand_into(&mut meta)
            .unwrap();
        assert_eq!(
            meta.finish().url(),
            "https://bidder.example.test/a%20b%2F%26%3F%3D%23%2B%25"
        );
    }

    #[test]
    fn borrowed_pairs_cannot_relax_template_membership_or_traversal_refusals() {
        for (macros, template, value) in [
            (&[][..], "https://bidder.example.test/{{.RouteID}}", "safe"),
            (
                &["RouteID"][..],
                "https://bidder.example.test/{{.Other}}",
                "safe",
            ),
            (
                &["RouteID"][..],
                "https://bidder.example.test/{{.RouteID}}",
                "../private",
            ),
            (
                &["RouteID"][..],
                "https://bidder.example.test/{{.RouteID}}",
                "%252e%252e%252fprivate",
            ),
        ] {
            let request_info = typed_info(template, macros);
            let pairs = [("RouteID", value)];
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            assert_eq!(
                EndpointBuilder::from_request(&request_info, &pairs).expand_into(&mut meta),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
                })
            );
            assert_eq!(meta.written(), 4);
            meta.set_url("https://recovery.example.test/").unwrap();
            assert_eq!(meta.finish().url(), "https://recovery.example.test/");
        }
    }

    fn info(endpoint: &str) -> BidderInfo {
        BidderInfo {
            endpoint: endpoint.into(),
            endpoint_macros: vec!["AccountID".into()],
            ..BidderInfo::default()
        }
    }

    #[test]
    fn query_expansion_encodes_each_pair_and_preserves_existing_query_and_macros() {
        for (endpoint, expected) in [
            (
                "https://bidder.example.test/path",
                "https://bidder.example.test/path?k%26x=a+b%26x%3D1%23%2F%25%2B&second=2",
            ),
            (
                "https://bidder.example.test/path?account={{.AccountID}}",
                "https://bidder.example.test/path?account=acct%2F42&k%26x=a+b%26x%3D1%23%2F%25%2B&second=2",
            ),
            (
                "https://bidder.example.test/path?",
                "https://bidder.example.test/path?k%26x=a+b%26x%3D1%23%2F%25%2B&second=2",
            ),
            (
                "https://bidder.example.test/path?a=1&",
                "https://bidder.example.test/path?a=1&k%26x=a+b%26x%3D1%23%2F%25%2B&second=2",
            ),
        ] {
            let info = info(endpoint);
            let mut builder = EndpointBuilder::new(&info).unwrap();
            if endpoint.contains("{{.AccountID}}") {
                builder.set_macro("AccountID", "acct/42").unwrap();
            }
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            builder
                .expand_with_query_into(&mut meta, &[("k&x", "a b&x=1#/%+"), ("second", "2")])
                .unwrap();
            assert_eq!(meta.finish().url(), expected);
        }
    }

    #[test]
    fn query_expansion_binds_the_existing_byte_cap_and_rolls_back_the_whole_url() {
        let endpoint = "https://bidder.example.test/path";
        let fitting = META_BIDDER_REGION_BYTES - 4 - endpoint.len() - "?k=".len();
        for width in [fitting - 1, fitting, fitting + 1] {
            let info = info(endpoint);
            let value = "x".repeat(width);
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let original = pool.as_ptr();
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            let result = EndpointBuilder::new(&info)
                .unwrap()
                .expand_with_query_into(&mut meta, &[("k", &value)]);
            if width <= fitting {
                result.unwrap();
                assert_eq!(meta.written(), 4 + endpoint.len() + 3 + width);
            } else {
                assert_eq!(
                    result,
                    Err(SinkAbort::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
                    })
                );
                assert_eq!(meta.written(), 4);
                meta.set_url("https://recovery.example.test/").unwrap();
            }
            drop(meta);
            assert_eq!(pool.as_ptr(), original);
            assert_eq!(pool.capacity(), META_BYTES_CAPACITY);
        }
    }

    #[test]
    fn query_expansion_refuses_fragments_without_retaining_a_partial_url() {
        let info = info("https://bidder.example.test/path#fragment");
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
        meta.set_method("POST").unwrap();
        assert_eq!(
            EndpointBuilder::new(&info)
                .unwrap()
                .expand_with_query_into(&mut meta, &[("k", "v")]),
            Err(SinkAbort::MetaOverCap {
                limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
            })
        );
        assert_eq!(meta.written(), 4);
        meta.set_url("https://recovery.example.test/").unwrap();
    }

    #[test]
    fn empty_query_pairs_and_template_failures_preserve_the_original_contract() {
        for (endpoint, valid) in [
            ("https://bidder.example.test/path?x=1", true),
            ("https://bidder.example.test/{{.AccountID}}", false),
            ("https://bidder.example.test/{{.Unknown}}", false),
            ("https://bidder.example.test/{{.AccountID", false),
        ] {
            let info = info(endpoint);
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            let result = EndpointBuilder::new(&info)
                .unwrap()
                .expand_with_query_into(&mut meta, &[]);
            if valid {
                result.unwrap();
                let duplicate = EndpointBuilder::new(&info).unwrap();
                assert!(matches!(
                    duplicate.expand_with_query_into(&mut meta, &[("x", "2")]),
                    Err(SinkAbort::DuplicateMeta { .. })
                ));
                assert_eq!(meta.finish().url(), endpoint);
            } else {
                assert_eq!(
                    result,
                    Err(SinkAbort::MetaOverCap {
                        limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap()
                    })
                );
                assert_eq!(meta.written(), 4);
            }
        }
    }

    #[test]
    fn query_set_replaces_all_decoded_keys_and_preserves_unrelated_pairs() {
        for (query, key, expected) in [
            (
                "?member_id=1&keep=a%2Bb&member%5fid=2&other=3&member_id=4",
                "member_id",
                "?keep=a%2Bb&other=3&member_id=9+%2B%26%23",
            ),
            (
                "?member_id=1&member_id=2",
                "member_id",
                "?member_id=9+%2B%26%23",
            ),
            (
                "?member_id&keep&keep=2&Member_id=3&member%255fid=4",
                "member_id",
                "?keep&keep=2&Member_id=3&member%255fid=4&member_id=9+%2B%26%23",
            ),
            ("?a+b=1&a%20b=2&a%2Bb=3", "a b", "?a%2Bb=3&a+b=9+%2B%26%23"),
            ("?a%2Bb=1&a+b=2", "a+b", "?a+b=2&a%2Bb=9+%2B%26%23"),
            ("?a%2bb=1&a+b=2", "a+b", "?a+b=2&a%2Bb=9+%2B%26%23"),
            (
                "?a++b=1&a%20%20b=2&keep=3",
                "a  b",
                "?keep=3&a++b=9+%2B%26%23",
            ),
            (
                "?%E2%98%83=1&keep=%25",
                "\u{2603}",
                "?keep=%25&%E2%98%83=9+%2B%26%23",
            ),
            ("?=1&keep=2&=3", "", "?keep=2&=9+%2B%26%23"),
            (
                "?member_id=1&&keep=2&&member_id=3&",
                "member_id",
                "?&keep=2&&member_id=9+%2B%26%23",
            ),
            ("?", "member_id", "?member_id=9+%2B%26%23"),
            ("", "member_id", "?member_id=9+%2B%26%23"),
        ] {
            let endpoint = format!("https://bidder.example.test/path{query}");
            let info = info(&endpoint);
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            meta.header("x-keep", "yes").unwrap();
            EndpointBuilder::new(&info)
                .unwrap()
                .expand_with_query_set_into(&mut meta, key, "9 +&#")
                .unwrap();
            let sent = meta.finish();
            assert_eq!(
                sent.url(),
                format!("https://bidder.example.test/path{expected}")
            );
            assert_eq!(sent.method(), "POST");
            assert_eq!(sent.headers().collect::<Vec<_>>(), [("x-keep", "yes")]);
        }
    }

    #[test]
    fn query_set_matches_keys_after_macro_expansion_and_encodes_only_the_new_pair() {
        for (template, value, expected) in [
            (
                "https://bidder.example.test/path?{{.AccountID}}=old&keep={{.AccountID}}",
                "member_id",
                "https://bidder.example.test/path?keep=member_id&member_id=42",
            ),
            (
                "https://bidder.example.test/path?%{{.AccountID}}=old&keep=1",
                "6dember_id",
                "https://bidder.example.test/path?keep=1&member_id=42",
            ),
        ] {
            let info = info(template);
            let mut builder = EndpointBuilder::new(&info).unwrap();
            builder.set_macro("AccountID", value).unwrap();
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            builder
                .expand_with_query_set_into(&mut meta, "member_id", "42")
                .unwrap();
            assert_eq!(meta.finish().url(), expected);
        }
    }

    #[test]
    fn query_set_does_not_skip_template_validation_for_replaced_values() {
        for template in [
            "https://bidder.example.test/path?member_id={{.AccountID}}",
            "https://bidder.example.test/path?member_id={{.Unknown}}",
            "https://bidder.example.test/path?member_id={{.AccountID",
            "https://bidder.example.test/path?member_id=1#fragment",
        ] {
            let info = info(template);
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            meta.header("x-keep", "yes").unwrap();
            let before = meta.written();
            assert_eq!(
                EndpointBuilder::new(&info)
                    .unwrap()
                    .expand_with_query_set_into(&mut meta, "member_id", "42"),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                })
            );
            assert_eq!(meta.written(), before);
            meta.set_url("https://recovery.example.test/").unwrap();
            let sent = meta.finish();
            assert_eq!(sent.method(), "POST");
            assert_eq!(sent.url(), "https://recovery.example.test/");
            assert_eq!(sent.headers().collect::<Vec<_>>(), [("x-keep", "yes")]);
        }
    }

    #[test]
    fn query_set_rejects_a_second_url_without_editing_the_first() {
        let info = info("https://bidder.example.test/path?member_id=1&member_id=2");
        let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
        let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
        meta.set_url(&info.endpoint).unwrap();
        let before = meta.written();
        assert_eq!(
            EndpointBuilder::new(&info)
                .unwrap()
                .expand_with_query_set_into(&mut meta, "member_id", "42"),
            Err(SinkAbort::DuplicateMeta {
                field: crate::MetaField::Url
            })
        );
        assert_eq!(meta.written(), before);
        assert_eq!(meta.finish().url(), info.endpoint);
    }

    #[test]
    fn query_set_validates_borrowed_macros_even_in_values_it_would_remove() {
        let request_info = typed_info(
            "https://bidder.example.test/path?member_id={{.RouteID}}",
            &["RouteID"],
        );
        for value in ["../private", "%252e%252e%252fprivate"] {
            let pairs = [("RouteID", value)];
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 1);
            meta.set_method("POST").unwrap();
            assert_eq!(
                EndpointBuilder::from_request(&request_info, &pairs).expand_with_query_set_into(
                    &mut meta,
                    "member_id",
                    "42"
                ),
                Err(SinkAbort::MetaOverCap {
                    limit: u64::try_from(META_BIDDER_REGION_BYTES).unwrap(),
                })
            );
            assert_eq!(meta.written(), 4);
        }
    }

    fn info_fixed_host() -> BidderInfo {
        BidderInfo {
            name: "fake_synthetic".into(),
            endpoint: "https://hb.example.test/openrtb?acct={{.AccountID}}".into(),
            uses_https: true,
            host_allowlist: None,
            endpoint_macros: vec!["AccountID".into()],
            ..BidderInfo::default()
        }
    }

    fn info_host_varying() -> BidderInfo {
        BidderInfo {
            name: "fake_synthetic".into(),
            endpoint: "https://{{.Host}}/openrtb".into(),
            uses_https: true,
            host_allowlist: Some(vec!["eu.example.test".into(), "us.example.test".into()]),
            // {{.Host}} is excluded from endpoint_macros — it's the
            // authority token, handled via select_host.
            endpoint_macros: Vec::new(),
            ..BidderInfo::default()
        }
    }

    #[test]
    fn fixed_host_substitutes_account_id() {
        let info = info_fixed_host();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        builder.set_macro("AccountID", "acct-42").unwrap();
        let url = builder.build().unwrap();
        assert_eq!(url, "https://hb.example.test/openrtb?acct=acct-42");
    }

    #[test]
    fn macro_values_are_percent_encoded() {
        let info = info_fixed_host();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        builder.set_macro("AccountID", "a b/c?d#e").unwrap();
        let url = builder.build().unwrap();
        // Path/query positions: '/', '?', '#', and ' ' all get encoded.
        assert!(url.contains("acct=a%20b%2Fc%3Fd%23e"), "got: {url}");
    }

    #[test]
    fn unknown_macro_rejected() {
        let info = info_fixed_host();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        let err = builder.set_macro("PublisherID", "1234").unwrap_err();
        assert!(
            matches!(&err, EndpointBuilderError::UnknownMacro(macro_name) if macro_name == "PublisherID")
        );
    }

    #[test]
    fn path_traversal_value_rejected() {
        let info = info_fixed_host();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        // Encoded forms must be rejected too — `%2f` (encoded `/`) and
        // `%252e%252e%252f` (double-encoded `../`) previously slipped
        // past a literal-`/` split + single-pass `%2e -> .` decode.
        for bad in [
            "..",
            "../../etc/passwd",
            "a/../b",
            "%2e%2e",
            "%2e%2e%2f",
            "..%2fevil",       // encoded slash after raw ..
            "%2e%2e%2f..",     // mixed encoded/raw traversal
            "%252e%252e%252f", // double-encoded ../
            "a%00b",           // NUL byte truncation
        ] {
            let err = builder.set_macro("AccountID", bad).unwrap_err();
            assert!(
                matches!(err, EndpointBuilderError::PathTraversal(_)),
                "value: {bad}"
            );
        }
    }

    #[test]
    fn unknown_macro_not_on_codegen_allow_list_rejected() {
        // Even if a macro appears in the template, set_macro must reject
        // it when it's not on the codegen-emitted endpoint_macros
        // allow-list. info_fixed_host's endpoint_macros (populated by
        // codegen) contains "AccountID" only.
        let info = info_fixed_host();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        let err = builder.set_macro("SyncerKey", "x").unwrap_err();
        assert!(
            matches!(&err, EndpointBuilderError::UnknownMacro(macro_name) if macro_name == "SyncerKey")
        );
    }

    #[test]
    fn host_macro_cannot_be_set_via_set_macro() {
        let info = info_host_varying();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        // Host must NEVER be settable via set_macro — it's the
        // authority token, handled exclusively through select_host.
        let err = builder.set_macro("Host", "eu.example.test").unwrap_err();
        assert!(matches!(err, EndpointBuilderError::UnknownMacro(_)));
    }

    #[test]
    fn host_varying_requires_host_selection() {
        let info = info_host_varying();
        let builder = EndpointBuilder::new(&info).unwrap();
        let err = builder.build().unwrap_err();
        assert!(
            matches!(&err, EndpointBuilderError::MissingMacro(macro_name) if macro_name == "Host")
        );
    }

    #[test]
    fn host_varying_off_allowlist_rejected() {
        let info = info_host_varying();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        let err = builder.select_host("rogue.example.test").unwrap_err();
        assert!(matches!(err, EndpointBuilderError::HostNotInAllowlist(_)));
    }

    #[test]
    fn host_varying_allowlisted_host_accepted() {
        let info = info_host_varying();
        let mut builder = EndpointBuilder::new(&info).unwrap();
        builder.select_host("eu.example.test").unwrap();
        let url = builder.build().unwrap();
        assert_eq!(url, "https://eu.example.test/openrtb");
    }

    #[test]
    fn streaming_expansion_bounds_percent_encoded_output_and_rolls_back_url() {
        let info = info_fixed_host();
        let prefix = "https://hb.example.test/openrtb?acct=";
        let fitting = "x".repeat(META_BIDDER_REGION_BYTES - 4 - prefix.len());
        for (value, success) in [
            (fitting.as_str(), true),
            ("%".repeat(fitting.len()).as_str(), false),
        ] {
            let mut pool = Vec::with_capacity(META_BYTES_CAPACITY);
            let ptr = pool.as_ptr();
            let mut meta = OutboundRequestMetaBuilder::new(&mut pool, 100);
            meta.set_method("POST").unwrap();
            let mut builder = EndpointBuilder::new(&info).unwrap();
            builder.set_macro("AccountID", value).unwrap();
            assert_eq!(builder.expand_into(&mut meta).is_ok(), success);
            if !success {
                assert_eq!(meta.written(), 4, "failed URL append rolls back");
            }
            drop(meta);
            assert_eq!(pool.capacity(), META_BYTES_CAPACITY);
            assert_eq!(pool.as_ptr(), ptr);
        }
    }
}
